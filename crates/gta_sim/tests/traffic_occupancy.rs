//! G8 (TASK-032): the traffic consumers moved onto the shared road occupancy, one row per input.
//!
//! - Sensing sees (a) a kinematic AI car off its path line, (b) an abandoned traffic car, a taken car
//!   and a police car with the sirens off, (c) a character; a body's standing time restarts when it
//!   is put somewhere else.
//! - Lane start: a grant into a lane waits while an off-path kinematic car stands on its start.
//! - Claims: only a car the AI drives claims road; a car hijacked in the middle of a pass drops its
//!   claim, and the oncoming lane drives on past the spot; a car bailing out in the middle of a pass
//!   drops its claim and stays where it is (no sideways jump back).
//! - City facts the go-around reads: the opposite lane on the left of every lane, the curb lane of
//!   avenue lanes.
//!
//! Floors use same-direction lanes only (no `left_gap`), so no pass can start, except the claim rows
//! (a pass set by the fixture). The G1 oracle runs every tick of every floor row.

mod common;
mod traffic_support;
mod vehicle_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    occupancy::RoadOccupancy,
    police::{PoliceCar, PoliceCarState},
    traffic::{Manoeuvre, Segment, TrafficConfig, TrafficIntersections, TrafficMode},
    vehicle::{Autopilot, DriveIntent, VehicleConfig},
};
use traffic_support::*;
use vehicle_support::drive_in;

const HZ: u32 = 64;

fn at(x: f32, z: f32) -> Vec3 {
    Vec3::new(x, 0.0, z)
}

/// L1 +X along z 30, L2 +X along z 26.75 (3.25 m to L1's left), each looping back to itself.
fn parallel_floor() -> App {
    traffic_floor(
        vec![
            (at(-35.0, 30.0), at(35.0, 30.0), 12.0, 0),
            (at(-35.0, 26.75), at(35.0, 26.75), 12.0, 1),
        ],
        &[(0, 0, 0), (1, 1, 1)],
        &[],
    )
}

fn cfgs(app: &App) -> (TrafficConfig, VehicleConfig) {
    (
        app.world().resource::<TrafficConfig>().clone(),
        app.world().resource::<VehicleConfig>().clone(),
    )
}

fn switches(app: &App) -> u32 {
    stats(app).switches_by_cause.iter().sum()
}

/// Holds a traffic car where it stands, `lateral` m off its path (named fixture: a yield to an absent
/// siren car that never times out).
fn hold_off_path(app: &mut App, car: Entity, lateral: f32) {
    let v = cfgs(app).1;
    let seg_s = traffic_car(app, car);
    let (point, tangent) = graph(app).pose(seg_s.segment, seg_s.s);
    let right = right_of(tangent);
    set_car(app, car, |c| {
        c.lateral = lateral;
        c.speed = 0.0;
        c.manoeuvre = Manoeuvre::Yield {
            siren: Entity::PLACEHOLDER,
            since: u64::MAX,
            offset: lateral,
        };
    });
    teleport(
        app,
        car,
        point + right * lateral + Vec3::Y * v.rest_height(),
    );
}

/// A car at 12 m/s on L1 from s 10 comes up to `obstacle` (flat front at `front_x` on L1): it stops
/// kinematic, with no switch, its bumper at least `min_gap - 0.1` short of the obstacle. Watched for
/// 1 s less than `progress.wait_seconds`: past that the progress rule (TASK-039) squeezes it past.
fn stops_short(app: &mut App, front_x: f32, label: &str) {
    let (traffic, vehicle) = cfgs(app);
    let half = vehicle.half_extents().z;
    let car = spawn_traffic_car(app, Segment::Lane(0), 10.0, 12.0);
    let before = (switches(app), stats(app).casts);
    let mut oracle = Footprints::new(app);
    let mut stood = 0;
    let seconds = (traffic.progress.wait_seconds as u32).min(21) - 1;
    for tick in 0..seconds * HZ {
        run_ticks(app, 1);
        oracle.record(app, tick);
        stood = if traffic_car(app, car).speed == 0.0 {
            stood + 1
        } else {
            0
        };
    }
    assert_traffic_ran(app, before.1, 1, seconds * HZ);
    let gap = front_x - (position_of(app, car).x + half);
    let mode = traffic_car(app, car).mode;
    let mut failures = Vec::new();
    if switches(app) != before.0 || mode != TrafficMode::Kinematic {
        failures.push(format!("the car switched ({mode:?}): it hit the {label}"));
    }
    if stood < HZ || gap < traffic.idm.min_gap - 0.1 {
        failures.push(format!(
            "not held short of the {label}: bumper gap {gap:.2} m, stood {stood} ticks"
        ));
    }
    oracle.assert_clean(label);
    assert!(failures.is_empty(), "{label}: {}", failures.join("; "));
}

/// (a) Car A stands on L1 1.4 m to the left (reaching 0.55 m into L2); car B comes up L2 at 12 m/s
/// from 40 m behind. B stops behind A's rear.
#[test]
fn sensing_sees_a_kinematic_car_off_its_path() {
    let mut app = parallel_floor();
    let (traffic, vehicle) = cfgs(&app);
    let half = vehicle.half_extents().z;
    let a = spawn_traffic_car(&mut app, Segment::Lane(0), 50.0, 0.0);
    hold_off_path(&mut app, a, -1.4);
    let b = spawn_traffic_car(&mut app, Segment::Lane(1), 10.0, 12.0);
    let casts = stats(&app).casts;
    let mut oracle = Footprints::new(&app);
    // Until B has stood 1 s (inside the yield timeout, so A is still held).
    let (mut stood, mut ticks) = (0, 0);
    while stood < HZ && ticks < 20 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, ticks);
        ticks += 1;
        stood = if traffic_car(&app, b).speed == 0.0 {
            stood + 1
        } else {
            0
        };
    }
    eprintln!("B held after {:.2} s", ticks as f32 / HZ as f32);
    assert_traffic_ran(&app, casts, 2, ticks);
    let pa = position_of(&app, a);
    assert!(
        (pa.z - 28.6).abs() < 0.05 && (pa.x - 15.0).abs() < 0.05,
        "GATE BROKEN: car A drifted from its held spot: {pa}"
    );
    let gap = (pa.x - half) - (position_of(&app, b).x + half);
    oracle.assert_clean("L2 car against the off-path L1 car");
    assert!(
        traffic_car(&app, b).speed == 0.0 && gap >= traffic.idm.min_gap - 0.1,
        "car B did not hold behind the off-path car: speed {}, bumper gap {gap:.2} m",
        traffic_car(&app, b).speed
    );
}

/// (b) An abandoned traffic car, a taken car and a police car leaving (sirens off) on L1.
#[test]
fn sensing_sees_abandoned_taken_and_police_cars() {
    for kind in ["abandoned", "taken", "police"] {
        let mut app = parallel_floor();
        let half = cfgs(&app).1.half_extents().z;
        let obstacle = spawn_traffic_car(&mut app, Segment::Lane(0), 50.0, 0.0);
        let mut body = app.world_mut().entity_mut(obstacle);
        body.remove::<(Autopilot, DriveIntent)>();
        body.insert(RigidBody::Dynamic);
        match kind {
            "abandoned" => set_car(&mut app, obstacle, |c| c.mode = TrafficMode::Abandoned),
            "taken" => set_car(&mut app, obstacle, |c| c.mode = TrafficMode::Taken),
            _ => {
                let mut body = app.world_mut().entity_mut(obstacle);
                body.remove::<gta_sim::traffic::TrafficCar>();
                body.insert(PoliceCar {
                    state: PoliceCarState::Leave,
                    crew: vec![],
                    stopped: 0.0,
                    moving: 0.0,
                    blocked: 0.0,
                    reboard_left: 0.0,
                });
            }
        }
        run_ticks(&mut app, 2);
        stops_short(&mut app, 15.0 - half, kind);
    }
}

/// (c) A dummy standing on L1.
#[test]
fn sensing_sees_a_character() {
    let mut app = parallel_floor();
    let radius = app
        .world()
        .resource::<gta_sim::character::LocomotionConfig>()
        .capsule_radius;
    spawn_dummy(&mut app, at(15.0, 30.0));
    run_ticks(&mut app, 2);
    stops_short(&mut app, 15.0 - radius, "dummy");
}

/// Standing time restarts when a body is put somewhere else (a teleport or a respawn keeps its
/// velocity at 0): a dummy that stood 2 s on L1 and is moved 10 m along it counts from zero (else a
/// car behind it would go around a person who just arrived, t15 hijack on seed 1).
#[test]
fn standing_restarts_after_a_teleport() {
    let mut app = parallel_floor();
    let dummy = spawn_dummy(&mut app, at(15.0, 30.0));
    run_ticks(&mut app, 2 * HZ);
    let stood = |app: &App| {
        app.world()
            .resource::<gta_sim::occupancy::RoadOccupancy>()
            .body(dummy)
            .map_or(0.0, |b| b.standing)
    };
    assert!(
        stood(&app) > 1.5,
        "GATE BROKEN: the dummy has not stood ({})",
        stood(&app)
    );
    let to = position_of(&app, dummy) + Vec3::X * 10.0;
    teleport(&mut app, dummy, to);
    run_ticks(&mut app, 2);
    let after = stood(&app);
    assert!(
        after <= 2.0 / HZ as f32 + 1e-4,
        "standing {after} s right after a teleport"
    );
}

/// Lane start: A +X ends at x -2, B +X starts at x 2 (A -> B straight), C -X along z 26.75. Car X
/// belongs to C but stands 3.25 m to its left, on B's start (a passer on the opposite lane); car W
/// waits at A's end for the connector into B. W is never granted while X stands there.
#[test]
fn lane_start_sees_an_off_path_car() {
    let mut app = traffic_floor(
        vec![
            (at(-35.0, 30.0), at(-2.0, 30.0), 12.0, 0),
            (at(2.0, 30.0), at(35.0, 30.0), 12.0, 1),
            (at(35.0, 26.75), at(-35.0, 26.75), 12.0, 2),
        ],
        &[(0, 1, 0), (1, 2, 1), (2, 0, 2)],
        &[],
    );
    let (traffic, vehicle) = cfgs(&app);
    let half = vehicle.half_extents().z;
    let x = spawn_traffic_car(&mut app, Segment::Lane(2), 29.0, 0.0);
    hold_off_path(&mut app, x, -3.25);
    let w = spawn_traffic_car(
        &mut app,
        Segment::Lane(0),
        33.0 - half - traffic.idm.min_gap,
        0.0,
    );
    let c = graph(&app).lane(0).out[0];
    let mut oracle = Footprints::new(&app);
    let (mut asked, mut granted) = (false, false);
    for tick in 0..3 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let junctions = app.world().resource::<TrafficIntersections>();
        granted |= junctions.granted(0, c, w);
        asked |= granted
            || junctions
                .0
                .get(&0)
                .is_some_and(|j| j.waiters.iter().any(|w2| w2.1 == w));
    }
    assert!(asked, "GATE BROKEN: car W never asked for its connector");
    let px = position_of(&app, x);
    assert!(
        (px.z - 30.0).abs() < 0.05,
        "GATE BROKEN: car X is not on B's start: {px}"
    );
    oracle.assert_clean("lane start");
    assert!(
        !granted,
        "car W was granted into a lane start an off-path car stands on"
    );
}

/// Seed 1: every street and avenue lane has the opposite inner lane 3.25 m to its left; avenue lanes
/// (and only they) have a curb lane.
#[test]
fn city_lane_facts() {
    let app = city_app(1);
    let (traffic, _) = cfgs(&app);
    let g = graph(&app);
    let mut failures = Vec::new();
    let (mut streets, mut avenues) = (0, 0);
    for (k, lane) in g.lanes().iter().enumerate() {
        let avenue = (lane.v0 - traffic.desired_speed.avenue).abs() < 1e-3;
        if avenue {
            avenues += 1;
        } else {
            streets += 1;
        }
        let gap_ok = lane.left_gap.is_some_and(|gap| (gap - 3.25).abs() < 0.01);
        if !gap_ok || lane.curb_lane != avenue {
            failures.push(format!(
                "lane {k} (v0 {}): left_gap {:?}, curb {}",
                lane.v0, lane.left_gap, lane.curb_lane
            ));
        }
    }
    assert!(
        streets > 50 && avenues > 20,
        "GATE BROKEN: {streets} street and {avenues} avenue lanes"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Spawner: lane 0 +X along z 30 with lane 1 -X along z 26.75 to its left, lane 2 +X along z 20. The
/// player stands at (0, 5) looking -Z, so the lanes are off frame and the off-frame band (15-25 m)
/// holds the spawn points x -10, 0, 10 of lanes 1 and 2. Car P on lane 0 is set in a published pass
/// (named fixture: a pass whose claim runs over lane 1 from x -12 to 18, held back by a dummy standing
/// in it at x -8): no car spawns in the claim, cars do spawn on lane 2.
#[test]
fn spawner_keeps_off_a_pass_claim() {
    let mut app = traffic_floor(
        vec![
            (at(-35.0, 30.0), at(35.0, 30.0), 12.0, 0),
            (at(35.0, 26.75), at(-35.0, 26.75), 12.0, 1),
            (at(-35.0, 20.0), at(35.0, 20.0), 12.0, 2),
        ],
        &[(0, 1, 0), (1, 0, 1), (2, 2, 2)],
        &[],
    );
    let float = app
        .world()
        .resource::<gta_sim::character::LocomotionConfig>()
        .float_height;
    place_player(&mut app, Vec3::new(0.0, float, 5.0));
    run_ticks(&mut app, 8);
    let obstacle = park_car(&mut app, at(-4.0, 30.0), Vec3::X);
    let p = spawn_traffic_car(&mut app, Segment::Lane(0), 25.0, 0.0);
    spawn_dummy(&mut app, at(-8.0, 26.75));
    set_car(&mut app, p, |c| {
        c.manoeuvre = Manoeuvre::Pass {
            obstacle,
            offset: -3.25,
            need: 2.9,
            hold_s: 25.0,
            merge_s: 40.0,
            end_s: 51.0,
            go: false,
        }
    });
    set_view(&mut app, Some(chase_view(at(0.0, 5.0), Vec3::NEG_Z)));
    *app.world_mut()
        .resource_mut::<gta_sim::traffic::TrafficPhase>() = gta_sim::traffic::TrafficPhase::Steady;
    let before: Vec<Entity> = cars(&mut app);
    let mut oracle = Footprints::new(&app);
    for tick in 0..HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        assert!(
            matches!(
                traffic_car(&app, p).manoeuvre,
                Manoeuvre::Pass { go: false, .. }
            ),
            "GATE BROKEN: the fixture pass started or ended: {:?}",
            traffic_car(&app, p).manoeuvre
        );
    }
    let spawned: Vec<Vec3> = cars(&mut app)
        .into_iter()
        .filter(|e| !before.contains(e))
        .map(|e| position_of(&app, e))
        .collect();
    let in_claim: Vec<&Vec3> = spawned
        .iter()
        .filter(|q| (q.z - 26.75).abs() < 1.0 && (-12.0..18.0).contains(&q.x))
        .collect();
    let elsewhere = spawned.iter().filter(|q| (q.z - 20.0).abs() < 1.0).count();
    assert!(
        elsewhere > 0,
        "GATE BROKEN: nothing spawned on lane 2 ({spawned:?})"
    );
    oracle.assert_clean("spawner");
    assert!(
        in_claim.is_empty(),
        "cars spawned inside a pass claim: {in_claim:?}"
    );
}

/// Junction grants: the + intersection of `traffic_intersection.rs` (centre (-22, 22), arms 12 m). A
/// car with no driver stands in the middle of the box on the straight connector north -> south; the
/// car queued on the north arm with that exit is not granted it while the body is fresh (before
/// `pass.vehicle_seconds`, after which it may enter alone to go around).
#[test]
fn junction_grants_see_a_body_on_the_path() {
    let centre = Vec3::new(-22.0, 0.0, 22.0);
    let dirs = [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X];
    let mut lanes = Vec::new();
    for &d in &dirs {
        let side = right_of(-d) * 1.625;
        lanes.push((centre + d * 15.25 + side, centre + d * 3.25 + side, 6.0, 0));
    }
    for (k, &d) in dirs.iter().enumerate() {
        let side = right_of(d) * 1.625;
        lanes.push((
            centre + d * 3.25 + side,
            centre + d * 15.25 + side,
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
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let g = graph(&app);
    let straight = (0..g.connectors().len() as u32)
        .find(|&c| g.connector(c).from_lane == 0 && g.connector(c).to_lane == 6)
        .expect("GATE BROKEN: no north -> south connector");
    let seg = Segment::Connector(straight);
    let (mid, tangent) = g.pose(seg, g.length(seg) / 2.0);
    park_car(&mut app, mid, tangent);
    let half = cfgs(&app).1.half_extents().z;
    let w = spawn_traffic_car(&mut app, Segment::Lane(0), g.lane(0).stop - half - 2.0, 0.0);
    set_car(&mut app, w, |c| c.next = Some(straight));
    let fresh = app
        .world()
        .resource::<gta_sim::traffic::TrafficConfig>()
        .pass
        .vehicle_seconds;
    let mut oracle = Footprints::new(&app);
    let (mut asked, mut into_body) = (false, false);
    for tick in 0..((fresh - 0.5) * HZ as f32) as u32 {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let junctions = app.world().resource::<TrafficIntersections>();
        let Some(j) = junctions.0.get(&0) else {
            continue;
        };
        into_body |= j.occupants.iter().any(|o| o.0 == straight);
        asked |= j.waiters.iter().any(|x| x.1 == w) || j.occupants.iter().any(|o| o.1 == w);
    }
    assert!(asked, "GATE BROKEN: the north car never queued");
    oracle.assert_clean("junction grant");
    assert!(
        !into_body,
        "the connector with a body on its path was granted"
    );
}

/// A car on L0 (+X, z 30) in a published pass around a car left at x -4, its claim over the
/// opposite lane L1 (-X, z 26.75), is hijacked: the AI state goes with the driver and no claim of
/// it is left in the road occupancy; an oncoming car on L1 then drives past the spot. Second part
/// (named mutation): the taken car's fields are set back to the pass; a car the AI does not drive
/// still claims nothing.
#[test]
fn a_hijacked_passer_claims_nothing() {
    let mut app = traffic_floor(
        vec![
            (at(-35.0, 30.0), at(35.0, 30.0), 12.0, 0),
            (at(35.0, 26.75), at(-35.0, 26.75), 12.0, 1),
        ],
        &[(0, 1, 0), (1, 0, 1)],
        &[],
    );
    let obstacle = park_car(&mut app, at(-4.0, 30.0), Vec3::X);
    let p = spawn_traffic_car(&mut app, Segment::Lane(0), 25.0, 0.0);
    let pass = Manoeuvre::Pass {
        obstacle,
        offset: -3.25,
        need: 2.9,
        hold_s: 25.0,
        merge_s: 40.0,
        end_s: 51.0,
        go: false,
    };
    set_car(&mut app, p, |c| c.manoeuvre = pass);
    run_ticks(&mut app, 1);
    let claims_of = |app: &App| {
        app.world()
            .resource::<RoadOccupancy>()
            .claims()
            .iter()
            .filter(|c| c.owner == p)
            .count()
    };
    assert!(
        claims_of(&app) > 0,
        "GATE BROKEN: the fixture pass publishes no claim"
    );
    drive_in(&mut app, p);
    run_ticks(&mut app, 1);
    let taken = traffic_car(&app, p);
    let mut failures = Vec::new();
    if taken.mode != TrafficMode::Taken {
        failures.push(format!(
            "GATE BROKEN: the car is not taken: {:?}",
            taken.mode
        ));
    }
    if taken.manoeuvre != Manoeuvre::None {
        failures.push(format!("the taken car kept {:?}", taken.manoeuvre));
    }
    if claims_of(&app) > 0 {
        failures.push("the taken car still claims the opposite lane".into());
    }
    let oncoming = spawn_traffic_car(&mut app, Segment::Lane(1), 5.0, 12.0);
    let mut oracle = Footprints::new(&app);
    let mut reached = f32::INFINITY;
    for tick in 0..8 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        if traffic_car(&app, oncoming).segment == Segment::Lane(1) {
            reached = reached.min(position_of(&app, oncoming).x);
        }
    }
    // The claim ran over L1 from x 18 down to x -12.
    if reached > -13.0 {
        failures.push(format!(
            "the oncoming car got no further than x {reached:.1} on the opposite lane"
        ));
    }
    set_car(&mut app, p, |c| c.manoeuvre = pass);
    run_ticks(&mut app, 1);
    if claims_of(&app) > 0 {
        failures.push("a car the AI does not drive claims road from its fields".into());
    }
    oracle.assert_clean("hijacked passer");
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A passer out in the opposite lane (lateral -3.25) whose driver bails out (shot or scared): the
/// claim goes the next tick, while the car is still bailing, and the car brakes where it is instead of
/// jumping back to its lane line. Before, a bailing car kept its claim, and with no free door it
/// stands forever (QA, TASK-032 round 1).
#[test]
fn a_bailing_passer_claims_nothing() {
    let mut app = traffic_floor(
        vec![
            (at(-35.0, 30.0), at(35.0, 30.0), 12.0, 0),
            (at(35.0, 26.75), at(-35.0, 26.75), 12.0, 1),
        ],
        &[(0, 1, 0), (1, 0, 1)],
        &[],
    );
    let obstacle = park_car(&mut app, at(-4.0, 30.0), Vec3::X);
    let p = spawn_traffic_car(&mut app, Segment::Lane(0), 25.0, 6.0);
    set_car(&mut app, p, |c| {
        c.lateral = -3.25;
        c.manoeuvre = Manoeuvre::Pass {
            obstacle,
            offset: -3.25,
            need: 2.9,
            hold_s: 25.0,
            merge_s: 40.0,
            end_s: 51.0,
            go: true,
        };
    });
    run_ticks(&mut app, 1);
    let claims_of = |app: &App| {
        app.world()
            .resource::<RoadOccupancy>()
            .claims()
            .iter()
            .filter(|c| c.owner == p)
            .count()
    };
    assert!(
        claims_of(&app) > 0,
        "GATE BROKEN: the fixture pass publishes no claim"
    );
    set_car(&mut app, p, |c| {
        c.mode = TrafficMode::Bailing {
            attack: None,
            shooter: None,
        }
    });
    let z0 = position_of(&app, p).z;
    let mut oracle = Footprints::new(&app);
    let mut failures = Vec::new();
    for tick in 0..HZ / 4 {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let car = traffic_car(&app, p);
        if !matches!(car.mode, TrafficMode::Bailing { .. }) {
            failures.push(format!(
                "GATE BROKEN: tick {tick}: the car is no longer bailing: {:?}",
                car.mode
            ));
            break;
        }
        if tick >= 1 && claims_of(&app) > 0 {
            failures.push(format!("tick {tick}: the bailing car still claims road"));
            break;
        }
    }
    let dz = (position_of(&app, p).z - z0).abs();
    if dz > 0.1 {
        failures.push(format!("the bailing car moved {dz:.2} m sideways"));
    }
    oracle.assert_clean("bailing passer");
    assert!(failures.is_empty(), "{failures:#?}");
}
