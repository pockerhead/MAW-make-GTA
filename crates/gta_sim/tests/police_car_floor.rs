//! Police cars on synthetic lanes (GDD §5.3, TASK-016 fixer): a car never stops or lets its crew out
//! inside an intersection unless it is stuck there, a crew never gets out onto the roof (a cop without
//! a free door waits and gets out once one clears; with no free door at all the car stays on the job),
//! a car with no lane route to the player lets its crew out where it stands instead of driving straight
//! at the player, a car pulls away from rest behind another at the IDM rate, not at a crawl, and a
//! car never spawns behind AI traffic on its lane route to the player.

mod common;
mod police_support;
mod traffic_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    combat::aim_yaw,
    police::{CrewOf, PoliceCar, PoliceCarRoute, PoliceCarState, PoliceUnit, UnitKind},
    traffic::{Segment, TrafficConfig, TrafficGraph},
    vehicle::{Autopilot, DriveIntent, vehicle_bundle},
};
use police_support::*;
use traffic_support::*;
use vehicle_support::*;
use wanted_support::*;

/// A four-lane loop of half size `h` around `(cx, cz)` (clockwise seen from +Y, rounded corners of
/// 3 m), its corner nodes `node..node + 4`; lane indices start at `first`.
fn square_loop(
    cx: f32,
    cz: f32,
    h: f32,
    first: u32,
    node: u32,
) -> (Vec<LaneSpec>, Vec<(u32, u32, u32)>) {
    rect_loop(cx, cz, (h, h), first, node)
}

/// `square_loop` with half sizes `(hx, hz)`.
fn rect_loop(
    cx: f32,
    cz: f32,
    (hx, hz): (f32, f32),
    first: u32,
    node: u32,
) -> (Vec<LaneSpec>, Vec<(u32, u32, u32)>) {
    let at = |x: f32, z: f32| Vec3::new(cx + x, 0.0, cz + z);
    let r = 3.0;
    (
        vec![
            (at(-hx + r, hz), at(hx - r, hz), 12.0, node),
            (at(hx, hz - r), at(hx, -hz + r), 12.0, node + 1),
            (at(hx - r, -hz), at(-hx + r, -hz), 12.0, node + 2),
            (at(-hx, -hz + r), at(-hx, hz - r), 12.0, node + 3),
        ],
        (0..4)
            .map(|k| (first + k, first + (k + 1) % 4, node + k))
            .collect(),
    )
}

/// Loop A around (22, 28) and loop B around (-22, 28), both of half size 8, not connected.
fn two_loops() -> (Vec<LaneSpec>, Vec<(u32, u32, u32)>) {
    let (mut lanes, mut connectors) = square_loop(22.0, 28.0, 8.0, 0, 0);
    let (b_lanes, b_connectors) = square_loop(-22.0, 28.0, 8.0, 4, 4);
    lanes.extend(b_lanes);
    connectors.extend(b_connectors);
    (lanes, connectors)
}

/// The player on foot with feet at `feet`, 2 stars that the search never clears, armour so nobody
/// dies.
fn wanted_player(app: &mut App, feet: Vec3) {
    let float = float_height_of(app);
    place_player(app, feet + Vec3::Y * float);
    run_ticks(app, 16);
    let settled = position(app);
    assert!(
        (settled - (feet + Vec3::Y * float)).length() < 0.1,
        "GATE BROKEN: player put at {feet} stands at {settled}"
    );
    set_player_armor(app, 1.0e6);
    let heat = wanted_cfg(app).stars[1].heat;
    raise_heat(app, heat);
}

fn hold_two_stars(app: &mut App) {
    let w = wanted(app);
    assert_eq!(
        w.stars, 2,
        "GATE BROKEN: the wanted level left 2 stars: {w:?}"
    );
    app.world_mut()
        .resource_mut::<gta_sim::wanted::WantedLevel>()
        .hidden = 0.0;
}

/// A responding police car (the production component set of the car dispatcher) on the road at `at`
/// heading `dir` at `speed`.
fn spawn_police_car(app: &mut App, at: Vec3, dir: Vec3, speed: f32, crew: Vec<UnitKind>) -> Entity {
    let cfg = vehicle_cfg(app);
    let dmg = damage_cfg(app);
    let body = Vec3::new(at.x, cfg.rest_height(), at.z);
    let transform =
        Transform::from_translation(body).with_rotation(Quat::from_rotation_y(aim_yaw(dir)));
    let car = app
        .world_mut()
        .spawn((
            vehicle_bundle(&cfg, &dmg, transform),
            PoliceCar {
                state: PoliceCarState::Respond,
                crew,
                stopped: 0.0,
                moving: 0.0,
                blocked: 0.0,
                reboard_left: 10.0,
            },
            PoliceCarRoute::default(),
            Autopilot {
                target: body + dir * cfg.autopilot.lookahead_min,
                ..default()
            },
            DriveIntent::default(),
            SleepingDisabled,
        ))
        .id();
    app.world_mut()
        .entity_mut(car)
        .insert((Name::new("Police car"), LinearVelocity(dir * speed)));
    car
}

fn police_car(app: &App, car: Entity) -> PoliceCar {
    app.world()
        .get::<PoliceCar>(car)
        .cloned()
        .expect("GATE BROKEN: the police car is gone")
}

/// Live cops of `car` outside it, with their feet height.
fn crew_out(app: &mut App, car: Entity) -> Vec<(Entity, f32)> {
    let float = float_height_of(app);
    app.world_mut()
        .query::<(Entity, &CrewOf, &PoliceUnit, &Position)>()
        .iter(app.world())
        .filter(|(_, c, ..)| c.car == car)
        .map(|(e, _, _, p)| (e, p.0.y - float))
        .collect()
}

// ------------------------------------------------------------------ intersections

const CENTRE: Vec3 = Vec3::new(-22.0, 0.0, 22.0);
const HALF_BOX: f32 = 3.25;
const ARM: f32 = 12.0;
const OFFSET: f32 = 1.625;

/// The AC2 + intersection (`traffic_intersection.rs`): in lanes 0..4 from N, E, S, W, out lanes 4..8.
fn plus() -> (Vec<LaneSpec>, Vec<(u32, u32, u32)>) {
    let right = |d: Vec3| Vec3::new(-d.z, 0.0, d.x);
    let dirs = [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X];
    let mut lanes = Vec::new();
    for &d in &dirs {
        let side = right(-d) * OFFSET;
        lanes.push((
            CENTRE + d * (HALF_BOX + ARM) + side,
            CENTRE + d * HALF_BOX + side,
            6.0,
            0,
        ));
    }
    for (k, &d) in dirs.iter().enumerate() {
        let side = right(d) * OFFSET;
        lanes.push((
            CENTRE + d * HALF_BOX + side,
            CENTRE + d * (HALF_BOX + ARM) + side,
            6.0,
            10 + k as u32,
        ));
    }
    let mut connectors = Vec::new();
    for i in 0..4u32 {
        for j in 0..4u32 {
            if i != j {
                connectors.push((i, 4 + j, 0));
            }
        }
    }
    for j in 0..4u32 {
        connectors.push((4 + j, j, 10 + j));
    }
    (lanes, connectors)
}

/// A police car crossing the box (north to south, 8 m/s) with the player on foot 13 m away: the
/// "hold" rule wants it stopped at once, yet it neither stands nor lets its crew out inside the box.
#[test]
fn police_car_never_stops_in_an_intersection() {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    wanted_player(&mut app, Vec3::new(-12.0, 0.0, 28.0));
    let graph: TrafficGraph = graph(&app);
    let half = vehicle_cfg(&app).half_extents();
    let straight = (0..graph.connectors().len() as u32)
        .find(|&c| graph.connector(c).from_lane == 0 && graph.connector(c).to_lane == 6)
        .expect("GATE BROKEN: no north-south connector");
    let seg = Segment::Connector(straight);
    let (at, tangent) = graph.pose(seg, graph.length(seg) / 2.0);
    assert!(
        graph.in_junction(at, half.z),
        "GATE BROKEN: the car does not start in the box"
    );
    let car = spawn_police_car(&mut app, at, tangent, 8.0, vec![UnitKind::Patrol; 2]);
    let dismount = police_support::esc(&app).car.dismount_distance;
    let mut standing = 0;
    let mut dismounted = None;
    for tick in 0..640 {
        run_ticks(&mut app, 1);
        hold_two_stars(&mut app);
        let p = position_of(&app, car);
        let v = velocity_of(&app, car).length();
        let state = police_car(&app, car).state;
        let inside = graph.in_junction(p, half.z);
        standing = if inside && v < 0.5 { standing + 1 } else { 0 };
        assert!(
            standing < 32,
            "tick {tick}: the police car stands in the box at {p} ({state:?})"
        );
        if state == PoliceCarState::Dismounted {
            assert!(!inside, "tick {tick}: the crew got out in the box at {p}");
            dismounted = Some((tick, p));
            break;
        }
    }
    let (tick, p) = dismounted.expect("the police car never dismounted in 10 s");
    let d = (p - position(&mut app)).with_y(0.0).length();
    eprintln!("dismounted after {tick} ticks at {p}, {d:.1} m from the player");
    assert!(d <= dismount + 6.0, "dismounted {d} m from the player");
    run_ticks(&mut app, 2);
    assert!(
        !crew_out(&mut app, car).is_empty(),
        "no crew out of the dismounted car"
    );
}

// ------------------------------------------------------------------ the roof

/// A car parked on loop A with a tall wall over its right door and the player on foot 15 m off its
/// left: one cop gets out at the left door, the other stays aboard (never onto the roof) and gets
/// out there once the first one has left the door.
#[test]
fn crew_never_gets_out_on_the_roof() {
    let (lanes, connectors) = two_loops();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    // Loop A's west lane runs +Z along x = 14; the car heads +Z, its right door faces -X.
    let at = Vec3::new(14.0, 0.0, 28.0);
    spawn_wall(
        &mut app,
        Vec3::new(at.x - 2.0, 2.0, at.z),
        Vec3::new(1.4, 4.0, 6.0),
    );
    wanted_player(&mut app, Vec3::new(26.0, 0.0, 37.0));
    let car = spawn_police_car(&mut app, at, Vec3::Z, 0.0, vec![UnitKind::Patrol; 2]);
    let ground_step = 0.5;
    let mut first = None;
    for tick in 0..128 {
        run_ticks(&mut app, 1);
        hold_two_stars(&mut app);
        for (cop, feet) in crew_out(&mut app, car) {
            assert!(
                feet < ground_step,
                "tick {tick}: cop {cop} got out with its feet at {feet} (the roof)"
            );
        }
        let out = crew_out(&mut app, car);
        if police_car(&app, car).state == PoliceCarState::Dismounted && !out.is_empty() {
            first.get_or_insert(out[0].0);
        }
    }
    let first = first.expect("GATE BROKEN: nobody got out of the car");
    assert_eq!(
        crew_out(&mut app, car).len(),
        1,
        "GATE BROKEN: two cops out of a car with one free door"
    );
    assert_eq!(
        police_car(&app, car).crew.len(),
        1,
        "the cop without a free door did not stay aboard"
    );
    // The first cop leaves the door: the one aboard gets out there.
    let away = position_of(&app, first) + Vec3::X * 6.0;
    app.world_mut().get_mut::<Position>(first).unwrap().0 = away;
    app.world_mut()
        .get_mut::<Transform>(first)
        .unwrap()
        .translation = away;
    let mut both = false;
    for tick in 0..16 {
        run_ticks(&mut app, 1);
        hold_two_stars(&mut app);
        let out = crew_out(&mut app, car);
        for (cop, feet) in &out {
            assert!(
                *feet < ground_step,
                "tick {tick}: cop {cop} got out with its feet at {feet} (the roof)"
            );
        }
        if out.len() == 2 {
            both = true;
            break;
        }
    }
    assert!(both, "the cop aboard never got out at the freed door");
    assert!(police_car(&app, car).crew.is_empty());
}

/// Tall walls over both doors: nobody can get out, so the car stays on the job (Respond, crew aboard)
/// instead of flipping between Dismounted and Respond every tick; once a door clears the crew gets out.
#[test]
fn crew_waits_aboard_when_no_door_is_free() {
    let (lanes, connectors) = two_loops();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let at = Vec3::new(14.0, 0.0, 28.0);
    spawn_wall(
        &mut app,
        Vec3::new(at.x - 2.0, 2.0, at.z),
        Vec3::new(1.4, 4.0, 6.0),
    );
    let left = spawn_wall(
        &mut app,
        Vec3::new(at.x + 2.0, 2.0, at.z),
        Vec3::new(1.4, 4.0, 6.0),
    );
    wanted_player(&mut app, Vec3::new(26.0, 0.0, 37.0));
    let car = spawn_police_car(&mut app, at, Vec3::Z, 0.0, vec![UnitKind::Patrol; 2]);
    for tick in 0..128 {
        run_ticks(&mut app, 1);
        hold_two_stars(&mut app);
        let c = police_car(&app, car);
        assert_eq!(
            c.state,
            PoliceCarState::Respond,
            "tick {tick}: a car nobody can leave is {:?}",
            c.state
        );
        assert_eq!(
            c.crew.len(),
            2,
            "tick {tick}: crew left a car with no free door"
        );
        assert!(
            crew_out(&mut app, car).is_empty(),
            "tick {tick}: a cop got out"
        );
    }
    app.world_mut().despawn(left);
    let mut out = false;
    for _ in 0..16 {
        run_ticks(&mut app, 1);
        hold_two_stars(&mut app);
        if police_car(&app, car).state == PoliceCarState::Dismounted
            && !crew_out(&mut app, car).is_empty()
        {
            out = true;
            break;
        }
    }
    assert!(out, "the crew did not get out once the left door cleared");
    for (cop, feet) in crew_out(&mut app, car) {
        assert!(feet < 0.5, "cop {cop} got out with its feet at {feet}");
    }
}

// ------------------------------------------------------------------ no route

/// A car on loop A, the player on foot inside loop B (no connector joins them): the car has no lane
/// route, so its crew gets out at once where it stands; it never drives straight at the player.
#[test]
fn car_without_a_route_lets_its_crew_out() {
    let (lanes, connectors) = two_loops();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let player = Vec3::new(-22.0, 0.0, 28.0);
    wanted_player(&mut app, player);
    let at = Vec3::new(14.0, 0.0, 28.0);
    let car = spawn_police_car(&mut app, at, Vec3::Z, 0.0, vec![UnitKind::Patrol; 2]);
    let start = position_of(&app, car);
    let mut dismounted = None;
    for tick in 0..256 {
        run_ticks(&mut app, 1);
        hold_two_stars(&mut app);
        let moved = (position_of(&app, car) - start).with_y(0.0).length();
        assert!(
            moved < 1.0,
            "tick {tick}: the car without a route moved {moved} m (towards the player?)"
        );
        if dismounted.is_none() && police_car(&app, car).state == PoliceCarState::Dismounted {
            dismounted = Some(tick);
        }
    }
    let tick = dismounted.expect("the crew of a car without a route never got out");
    assert!(
        tick < 16,
        "the crew got out only after {tick} ticks (held up, not for the missing route)"
    );
    assert!(
        !crew_out(&mut app, car).is_empty(),
        "no crew out of the car without a route"
    );
}

// ------------------------------------------------------------------ pulling away

/// QA round 2: a police car at rest behind any body in its forward cast crawled (IDM target `v + a·dt`
/// makes the autopilot throttle `a·dt·gain`: 0.3 m in 4 s). A responding car on a long lane with a
/// traffic car pulling away 15 m ahead reaches half the IDM free-road rate within 4 s.
#[test]
fn police_car_pulls_away_behind_a_leader() {
    let (lanes, connectors) = rect_loop(0.0, 29.0, (30.0, 6.0), 0, 0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    wanted_player(&mut app, Vec3::new(26.0, 0.0, 29.0));
    let graph: TrafficGraph = graph(&app);
    let (at, dir) = graph.pose(Segment::Lane(0), 2.0);
    // Named mutation: a car slower than exit speed for `blocked_seconds` lets its crew out, which ends
    // the drive under test; here it never does.
    app.world_mut()
        .resource_mut::<gta_sim::police::EscalationConfig>()
        .car
        .blocked_seconds = 1.0e6;
    let car = spawn_police_car(&mut app, at, dir, 0.0, vec![UnitKind::Patrol; 2]);
    let leader = spawn_traffic_car(&mut app, Segment::Lane(0), 17.0, 0.0);
    let idm = app.world().resource::<TrafficConfig>().idm.clone();
    let seconds = 4.0;
    let wanted_speed = 0.5 * idm.acceleration * seconds;
    let sense = app.world().resource::<TrafficConfig>().sense_distance;
    let half = vehicle_cfg(&app).half_extents().z;
    let mut top = 0.0f32;
    for _ in 0..(seconds * 64.0) as u32 {
        run_ticks(&mut app, 1);
        hold_two_stars(&mut app);
        top = top.max(velocity_of(&app, car).dot(dir));
        let gap = (position_of(&app, leader) - position_of(&app, car)).length() - 2.0 * half;
        assert!(
            gap < sense,
            "GATE BROKEN: the leader left the forward cast ({gap:.1} m)"
        );
        assert_eq!(
            police_car(&app, car).state,
            PoliceCarState::Respond,
            "GATE BROKEN: the car stopped responding"
        );
    }
    eprintln!("police car top speed {top:.2} m/s in {seconds} s (wanted >= {wanted_speed:.2})");
    assert!(
        top >= wanted_speed,
        "the police car crawled: {top:.2} m/s after {seconds} s behind a leader (wanted >= {wanted_speed:.2})"
    );
}

// ------------------------------------------------------------------ stuck in the box

/// QA round 2 (seed 3 plaza, 5 stars): a police car held up inside an intersection stood there 34 s
/// with its crew aboard. Held in the box by a car stopped on its exit lane, it lets its crew out there
/// after `blocked_seconds × junction_factor`, not before.
#[test]
fn police_car_stuck_in_an_intersection_lets_its_crew_out() {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    wanted_player(&mut app, Vec3::new(-12.0, 0.0, 28.0));
    let graph: TrafficGraph = graph(&app);
    let cfg = vehicle_cfg(&app);
    let half = cfg.half_extents();
    let straight = (0..graph.connectors().len() as u32)
        .find(|&c| graph.connector(c).from_lane == 0 && graph.connector(c).to_lane == 6)
        .expect("GATE BROKEN: no north-south connector");
    let seg = Segment::Connector(straight);
    let (at, tangent) = graph.pose(seg, graph.length(seg) / 2.0);
    // A car stopped just past the box on the exit lane, its tail 1.5 m ahead of the police car's nose.
    let ahead = at + tangent * (2.0 * half.z + 1.5);
    spawn_car(
        &mut app,
        Vec2::new(ahead.x, ahead.z),
        aim_yaw(tangent).to_degrees(),
    );
    let car = spawn_police_car(&mut app, at, tangent, 0.0, vec![UnitKind::Patrol; 2]);
    let esc = police_support::esc(&app);
    let stuck = (esc.car.blocked_seconds * esc.car.junction_factor * 64.0) as u32;
    let mut dismounted = None;
    for tick in 0..stuck + 128 {
        run_ticks(&mut app, 1);
        hold_two_stars(&mut app);
        assert!(
            graph.in_junction(position_of(&app, car), half.z),
            "GATE BROKEN: the police car left the box at tick {tick}"
        );
        if police_car(&app, car).state == PoliceCarState::Dismounted {
            dismounted = Some(tick);
            break;
        }
    }
    let tick = dismounted.expect("the police car stuck in the box never let its crew out");
    eprintln!("dismounted in the box after {tick} ticks (stuck rule {stuck})");
    assert!(
        tick + 2 >= stuck,
        "the crew got out in the box at tick {tick}, before the stuck rule ({stuck})"
    );
    run_ticks(&mut app, 2);
    assert!(
        !crew_out(&mut app, car).is_empty(),
        "no crew out of the car stuck in the box"
    );
}

// ------------------------------------------------------------------ spawn behind a queue

fn police_cars_at(app: &mut App) -> Vec<(Entity, Vec3)> {
    app.world_mut()
        .query_filtered::<(Entity, &Position), With<PoliceCar>>()
        .iter(app.world())
        .map(|(e, p)| (e, p.0))
        .collect()
}

/// Fixer round 4 (t15 chase): police cars spawned in the traffic queue behind a driving player never
/// closed in (they cannot pass traffic). The player drives lane 0 of the loop (+X) at 8 m/s, a traffic
/// car follows him 10 m behind; every ring point heading to him (lane 0 behind, lane 3 into lane 0)
/// lies behind that car, so no police car spawns. Once the traffic car is gone a car spawns at once
/// (the queue, nothing else, held the dispatcher back).
#[test]
fn police_car_never_spawns_behind_a_traffic_queue() {
    let (lanes, connectors) = loop_lanes(12.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    // Named mutation: the queue car fills the traffic bubble, the traffic spawner adds none.
    set_traffic(&mut app, |t| t.bubble.max_cars = 1);
    let car = spawn_car(&mut app, Vec2::new(10.0, 37.0), -90.0);
    drive_in(&mut app, car);
    set_player_armor(&mut app, 1.0e6);
    let heat = wanted_cfg(&app).stars[1].heat;
    raise_heat(&mut app, heat);
    let queue = spawn_traffic_car(&mut app, Segment::Lane(0), 34.0, 0.0);
    let exit_speed = vehicle_cfg(&app).exit_max_speed;
    let speed = 8.0;
    kick(&mut app, car, speed);
    let hold = |app: &mut App| {
        let v = velocity_of(app, car).dot(forward_of(app, car));
        set_drive(app, |d| d.throttle = ((speed - v) * 0.5).clamp(-1.0, 1.0));
        let at = position_of(app, car);
        // The camera looks ahead: every ring point behind the player is hidden.
        set_view(app, Some(chase_view(at, Vec3::X)));
        run_ticks(app, 1);
        hold_two_stars(app);
    };
    for tick in 0..64 {
        hold(&mut app);
        let v = velocity_of(&app, car).length();
        assert!(
            v > exit_speed,
            "GATE BROKEN: tick {tick}: the player's car slowed to {v:.2} m/s (no pursuit)"
        );
        let queue_x = position_of(&app, queue).x;
        let cars = police_cars_at(&mut app);
        assert!(
            cars.is_empty(),
            "tick {tick}: police car spawned behind the traffic car at x = {queue_x}: {cars:?}"
        );
    }
    app.world_mut().entity_mut(queue).despawn();
    // Named mutation: no traffic car replaces it.
    set_traffic(&mut app, |t| t.bubble.max_cars = 0);
    let mut spawned = None;
    for tick in 0..16 {
        hold(&mut app);
        if !police_cars_at(&mut app).is_empty() {
            spawned = Some(tick);
            break;
        }
    }
    assert!(
        spawned.is_some(),
        "GATE BROKEN: no police car spawned with the lane clear (the queue did not hold it back)"
    );
}
