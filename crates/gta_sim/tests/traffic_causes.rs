//! Causal repros of the TASK-031 M1 freeze (TASK-032 stage 1): each cause in isolation in the
//! production city, the player standing on the sidewalk off the carriageway facing the scene.
//!
//! - (a) a car the player left mid-lane on a street: no AI car behind it stands longer than 30 s
//!   (stands elsewhere within 60 m, e.g. junction approaches held for room, are printed);
//! - (b) a traffic car bumped into `Dynamic` (rear nudge, shoved to the curb, a dummy pressed at its
//!   bumper, yawed 45 deg): after 60 s it is not `Dynamic` and standing; with the dummy pressed at its
//!   bumper (a standing body on its path) it waits, never given up, and drives on once the dummy goes;
//! - (c) a character standing in a street lane: the car behind it does not stand longer than 30 s;
//! - (d) a car left on a connector path with an AI car on that connector behind it, out of view: no
//!   stand over `bubble.stuck_despawn_seconds` + 1 s, and the left car is gone (the stuck cheat) or
//!   out of the box (pushed);
//! - R1 (headless, the TASK-031 repro): a car left in the middle of the junction box next to the
//!   spawn, the player on the spawn sidewalk looking at it: no traffic car within 45 m of the box
//!   stands longer than 30 s over 150 s, G1, third bodies and relaxations clean (both seeds since
//!   TASK-039: the progress rule ends the class D stand of seed 1 and the class E queue of seed 7);
//! - the stuck cheat's in-view rule (fallback R-B): a car in the box the player looks at from nearby
//!   is never popped; seen from past `bubble.stuck_in_view_distance` it is cleared.
//!
//! Every row holds the TASK-032 bound city-wide: no AI car stands longer than 30 s in `Dynamic` (walkers
//! pinned at the nose of a `Dynamic` grant holder broke it in (c) and rb until TASK-037: civilians now
//! go around standing cars). Each row asserts the fixed behaviour; on the pre-TASK-032 code the
//! reproduced rows were RED.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    traffic::{Segment, TrafficCar, TrafficConfig, TrafficIntersections, TrafficMode},
    vehicle::VehicleConfig,
    world::PlayerSpawn,
};
use traffic_support::*;

const HZ: u32 = 64;
/// How long a dummy stands pressed against a bumped car, s (under the 30 s `Dynamic` stand bound).
const PRESSED_S: u32 = 20;
/// When the spot C queue is filled up to 8, s: the approach queue has formed by then (4 cars at 20 s).
const FILL_AT_S: u32 = 25;

struct Scene {
    app: App,
    lane: u32,
    obstacle: Vec3,
    dir: Vec3,
}

/// The longest street lane of `seed` with 75 m to its stop line; the obstacle spot 45 m along it;
/// the player on the right sidewalk 20 m past it, looking back up the lane.
fn street_scene(seed: u64) -> Scene {
    let mut app = city_app(seed);
    let street = app.world().resource::<TrafficConfig>().desired_speed.street;
    let lane = longest_lane(&app, street, 75.0);
    let l = graph(&app).lane(lane).clone();
    let obstacle = l.from + l.dir * 45.0;
    let beside = l.from + l.dir * 65.0 + right_of(l.dir) * 3.2;
    stand_player(&mut app, Vec2::new(beside.x, beside.z), -l.dir);
    run_ticks(&mut app, 2 * HZ);
    clear_spot(&mut app, obstacle, 8.0);
    Scene {
        app,
        lane,
        obstacle,
        dir: l.dir,
    }
}

/// Runs `seconds` with the feeder on the scene lane; returns the stand clock.
fn watch(scene: &mut Scene, seconds: u32, feed: bool) -> StandClock {
    let mut clock = StandClock::default();
    let mut feeder = Feeder::new(scene.lane, 4 * HZ, 8);
    for tick in 0..seconds * HZ {
        if feed {
            feeder.tick(&mut scene.app, tick);
        }
        run_ticks(&mut scene.app, 1);
        clock.record(&mut scene.app);
    }
    clock
}

/// Stands over `limit` of cars in the scene lane behind the obstacle (the queue it holds).
fn stands_behind(scene: &Scene, clock: &StandClock, limit: f32) -> Vec<(f32, Vec3)> {
    let l = graph(&scene.app).lane(scene.lane).clone();
    let right = right_of(l.dir);
    let obstacle = (scene.obstacle - l.from).with_y(0.0).dot(l.dir);
    clock
        .longer_than(limit)
        .into_iter()
        .filter(|(_, _, p)| {
            let d = (*p - l.from).with_y(0.0);
            (-5.0..obstacle).contains(&d.dot(l.dir)) && d.dot(right).abs() < 1.7
        })
        .map(|(_, s, p)| (s, p))
        .collect()
}

fn stands_near(clock: &StandClock, at: Vec3, radius: f32, limit: f32) -> Vec<(f32, Vec3)> {
    clock
        .longer_than(limit)
        .into_iter()
        .filter(|(_, _, p)| (*p - at).with_y(0.0).length() <= radius)
        .map(|(_, s, p)| (s, p))
        .collect()
}

fn abandoned_car(seed: u64) {
    let mut scene = street_scene(seed);
    let at = scene.obstacle;
    park_car(&mut scene.app, at, scene.dir);
    let clock = watch(&mut scene, 120, true);
    let stood = stands_behind(&scene, &clock, 30.0);
    eprintln!(
        "(a) seed {seed}: worst stand {:?}, behind the car over 30 s: {stood:?}; within 60 m over 30 s: {:?}",
        clock.worst(),
        stands_near(&clock, at, 60.0, 30.0)
    );
    let mut failures: Vec<String> = clock.dynamic_violation().into_iter().collect();
    if !stood.is_empty() {
        failures.push(format!(
            "AI cars stood > 30 s behind the left car: {stood:?}"
        ));
    }
    assert!(failures.is_empty(), "seed {seed}: {failures:?}");
}

#[test]
fn a_left_car_seed_1() {
    abandoned_car(1);
}

#[test]
fn a_left_car_seed_7() {
    abandoned_car(7);
}

/// Bumps a traffic car standing at the obstacle spot into `Dynamic` with a car kicked at 2 m/s into
/// its rear, then `after` perturbs it; 60 s later it must not be `Dynamic` and standing. A body
/// `after` returns stands in the car's way for `pressed_s` and is then removed. With such a body the
/// car stays `Dynamic` and is never given up for `progress.wait_seconds`, is never given up in the
/// run, stands at most 30 s, drives past the body (its rear beyond it) before it is removed (the
/// progress rule), and the body is never hit or knocked down.
fn bumped(label: &str, pressed_s: u32, after: impl FnOnce(&mut Scene, Entity) -> Option<Entity>) {
    let mut scene = street_scene(1);
    let half = scene.app.world().resource::<VehicleConfig>().half_extents();
    let lane = scene.lane;
    let s = (scene.obstacle - graph(&scene.app).lane(lane).from).dot(scene.dir);
    let car = spawn_traffic_car(&mut scene.app, Segment::Lane(lane), s, 0.0);
    let behind = scene.obstacle - scene.dir * (2.0 * half.z + 1.0);
    clear_spot(&mut scene.app, behind, 1.0);
    let pusher = park_car(&mut scene.app, behind, scene.dir);
    let mut switched = false;
    // The pusher is held at 2 m/s until the contact (an unmanned car brakes on its own).
    for _ in 0..2 * HZ {
        scene
            .app
            .world_mut()
            .get_mut::<LinearVelocity>(pusher)
            .unwrap()
            .0 = scene.dir * 2.0;
        run_ticks(&mut scene.app, 1);
        if traffic_car(&scene.app, car).mode == TrafficMode::Dynamic {
            switched = true;
            break;
        }
    }
    assert!(
        switched,
        "GATE BROKEN: {label}: the nudge did not switch the car"
    );
    let blocker = after(&mut scene, car);
    let wait = scene
        .app
        .world()
        .resource::<TrafficConfig>()
        .progress
        .wait_seconds as u32;
    let mut given_up_while_pressed = None;
    let mut stood = 0.0;
    let mut gone = None;
    let mut clock = StandClock::default();
    let mut given_up = None;
    let mut rear_past = None;
    let mut hurt = Vec::new();
    let mut hits = scene
        .app
        .world()
        .resource::<Messages<gta_sim::vehicle::VehicleHit>>()
        .get_cursor_current();
    for tick in 0..60 * HZ {
        if tick == pressed_s * HZ
            && let Some(body) = blocker
        {
            scene.app.world_mut().despawn(body);
        }
        run_ticks(&mut scene.app, 1);
        clock.record(&mut scene.app);
        let now = scene.app.world().get::<TrafficCar>(car).map(|c| c.mode);
        if blocker.is_some()
            && tick < wait * HZ
            && given_up_while_pressed.is_none()
            && !matches!(now, Some(TrafficMode::Dynamic))
        {
            given_up_while_pressed = Some((tick as f32 / HZ as f32, now));
        }
        if let (Some(body), true) = (blocker, tick < pressed_s * HZ) {
            let world = scene.app.world();
            let at = |e: Entity| world.get::<Position>(e).map(|p| p.0);
            if let (Some(c), Some(b)) = (at(car), at(body))
                && rear_past.is_none()
                && (c - scene.dir * half.z - b).dot(scene.dir) > 0.0
            {
                rear_past = Some(tick as f32 / HZ as f32);
            }
            let hit = hits
                .read(world.resource::<Messages<gta_sim::vehicle::VehicleHit>>())
                .any(|h| h.target == body);
            let reaction = world.get::<gta_sim::combat::HitReaction>(body).copied();
            if (hit || reaction.is_some_and(|r| r.is_active())) && hurt.len() < 3 {
                hurt.push((tick as f32 / HZ as f32, hit, reaction));
            }
        }
        if blocker.is_some()
            && given_up.is_none()
            && matches!(
                now,
                Some(TrafficMode::Abandoned | TrafficMode::Bailing { .. })
            )
        {
            given_up = Some((tick as f32 / HZ as f32, now));
        }
        // Gone: it drove out of the bubble (not standing).
        let Some(v) = scene.app.world().get::<LinearVelocity>(car) else {
            gone = Some(tick as f32 / HZ as f32);
            stood = 0.0;
            break;
        };
        stood = if v.0.length() < 0.5 {
            stood + 1.0 / HZ as f32
        } else {
            0.0
        };
    }
    let mode = scene.app.world().get::<TrafficCar>(car).map(|c| c.mode);
    let at = scene.app.world().get::<Position>(car).map(|p| p.0);
    eprintln!(
        "(b) {label}: after 60 s mode {mode:?} at {at:?}, standing {stood:.1} s, despawned at {gone:?} s; worst dynamic stand {:.1} s",
        clock.worst_dynamic()
    );
    let mut failures: Vec<String> = clock.dynamic_violation().into_iter().collect();
    if let Some((t, mode)) = given_up_while_pressed {
        failures.push(format!(
            "left Dynamic at {t:.2} s ({mode:?}) while a body stood in its way on its path"
        ));
    }
    if mode == Some(TrafficMode::Dynamic) && stood > 30.0 {
        failures.push(format!(
            "still Dynamic and standing {stood:.1} s after 60 s"
        ));
    }
    if blocker.is_some() {
        eprintln!(
            "(b) {label}: rear past the body at {rear_past:?} s (removed at {pressed_s} s), longest stand {:.1} s",
            clock.of(car)
        );
        if let Some(g) = given_up {
            failures.push(format!("given up at {g:?}"));
        }
        if clock.of(car) > 30.0 {
            failures.push(format!("stood {:.1} s", clock.of(car)));
        }
        if rear_past.is_none() {
            failures.push("never drove past the body before it was removed".into());
        }
        if !hurt.is_empty() {
            failures.push(format!("the body was hit: {hurt:?}"));
        }
    }
    assert!(failures.is_empty(), "{label}: {failures:?}");
}

#[test]
fn b1_rear_nudge() {
    bumped("rear nudge", PRESSED_S, |_, _| None);
}

#[test]
fn b2_shoved_to_the_curb() {
    bumped(
        "shoved 0.6 m to the curb, a dummy on the sidewalk ahead",
        PRESSED_S,
        |scene, car| {
            let right = right_of(scene.dir);
            let p = scene.app.world().get::<Position>(car).unwrap().0 + right * 0.6;
            scene.app.world_mut().get_mut::<Position>(car).unwrap().0 = p;
            scene
                .app
                .world_mut()
                .get_mut::<Transform>(car)
                .unwrap()
                .translation = p;
            let walker = scene.obstacle + scene.dir * 10.0 + right * 3.0;
            let y = ground_at(&mut scene.app, Vec2::new(walker.x, walker.z));
            spawn_dummy(&mut scene.app, walker.with_y(y));
            None
        },
    );
}

#[test]
fn b3_dummy_pressed_at_the_bumper() {
    // Pressed past the progress wait: the car squeezes past the dummy (TASK-039).
    let wait = {
        let app = headless_app();
        app.world()
            .resource::<TrafficConfig>()
            .progress
            .wait_seconds as u32
    };
    bumped(
        "a dummy pressed at the front bumper",
        wait + 12,
        |scene, car| {
            let half = scene.app.world().resource::<VehicleConfig>().half_extents();
            let nose =
                scene.app.world().get::<Position>(car).unwrap().0 + scene.dir * (half.z + 0.35);
            let y = ground_at(&mut scene.app, Vec2::new(nose.x, nose.z));
            Some(spawn_dummy(&mut scene.app, nose.with_y(y)))
        },
    );
}

#[test]
fn b4_yawed_45_deg() {
    bumped("yawed 45 deg", PRESSED_S, |scene, car| {
        let r = Quat::from_rotation_y(45f32.to_radians())
            * scene.app.world().get::<Rotation>(car).unwrap().0;
        scene.app.world_mut().get_mut::<Rotation>(car).unwrap().0 = r;
        scene
            .app
            .world_mut()
            .get_mut::<Transform>(car)
            .unwrap()
            .rotation = r;
        None
    });
}

#[test]
fn c_character_in_the_lane() {
    let mut scene = street_scene(1);
    let at = scene.obstacle;
    let y = ground_at(&mut scene.app, Vec2::new(at.x, at.z));
    spawn_dummy(&mut scene.app, at.with_y(y));
    let clock = watch(&mut scene, 120, true);
    let stood = stands_behind(&scene, &clock, 30.0);
    eprintln!(
        "(c): worst stand {:?}, behind the character over 30 s: {stood:?}; within 60 m over 30 s: {:?}",
        clock.worst(),
        stands_near(&clock, at, 60.0, 30.0)
    );
    let mut failures: Vec<String> = clock.dynamic_violation().into_iter().collect();
    if !stood.is_empty() {
        failures.push(format!(
            "AI cars stood > 30 s behind a character in the lane: {stood:?}"
        ));
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// A car left on a connector path of the junction nearest to the spawn, an AI car on that connector
/// behind it; the player 12 m from the connector entry, facing away from the spot. Returns the stand
/// clock, the AI car, the left car, the spot and the app.
fn car_left_on_a_connector() -> (StandClock, Entity, Entity, Vec3, App) {
    let mut app = city_app(1);
    run_ticks(&mut app, 2 * HZ);
    let g = graph(&app);
    let feet = position(&mut app);
    // A connector of 10 m or more of the junction nearest to the spawn.
    let (c, conn) = g
        .connectors()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.length >= 10.0)
        .min_by(|a, b| {
            let d = |c: &gta_sim::traffic::TrafficConnector| (c.points[0] - feet).length();
            d(a.1).total_cmp(&d(b.1))
        })
        .map(|(k, c)| (k as u32, c.clone()))
        .expect("GATE BROKEN: no connector of 10 m");
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let (spot, tangent) = g.pose(Segment::Connector(c), 2.0 * half.z + 3.5);
    let (entry, _) = g.pose(Segment::Connector(c), 0.5);
    let side = conn.points[0] + (conn.points[0] - spot).normalize() * 12.0;
    stand_player(&mut app, Vec2::new(side.x, side.z), side - spot);
    clear_spot(&mut app, spot, 9.0);
    clear_spot(&mut app, entry, 6.0);
    let obstacle = park_car(&mut app, spot, tangent);
    let car = spawn_traffic_car(&mut app, Segment::Connector(c), 0.5, 0.0);
    set_car(&mut app, car, |t| t.next = Some(c));
    app.world_mut()
        .resource_mut::<TrafficIntersections>()
        .0
        .entry(conn.node)
        .or_default()
        .occupants
        .push((c, car));
    let mut clock = StandClock::default();
    for _ in 0..120 * HZ {
        run_ticks(&mut app, 1);
        clock.record(&mut app);
    }
    (clock, car, obstacle, spot, app)
}

/// Out of view nothing near the box stands longer than `bubble.stuck_despawn_seconds` (+ 1 s), and
/// the left car is gone (the stuck cheat) or out of the box (pushed).
#[test]
fn d_car_left_on_a_connector_out_of_view() {
    let (clock, car, obstacle, spot, app) = car_left_on_a_connector();
    let limit = app
        .world()
        .resource::<TrafficConfig>()
        .bubble
        .stuck_despawn_seconds
        + 1.0;
    let stood = clock.of(car);
    let worst = stands_near(&clock, spot, 45.0, limit);
    let left = app.world().get::<Position>(obstacle).map(|p| p.0);
    let in_box = left.is_some_and(|p| graph(&app).in_junction(p, 0.0));
    eprintln!(
        "(d) out of view: the connector car stood {stood:.1} s; stands > {limit} s near: {worst:?}; left car at {left:?} (in the box: {in_box}); worst {:?}, worst dynamic {:.1} s",
        clock.worst(),
        clock.worst_dynamic()
    );
    let mut violations: Vec<String> = clock.dynamic_violation().into_iter().collect();
    if stood > limit || !worst.is_empty() {
        violations.push(format!(
            "stands over {limit} s: the connector car {stood:.1} s, near the box {worst:?}"
        ));
    }
    if in_box {
        violations.push("the left car still stands in the box".to_string());
    }
    assert!(violations.is_empty(), "{violations:#?}");
}

/// What a run with a car left in the middle of the box nearest to the spawn saw.
struct LeftInBox {
    hub: Vec3,
    clock: StandClock,
    oracle: Footprints,
    /// Where the left car ended, `None` when the cheat despawned it.
    left: Option<Vec3>,
}

/// The car the player drove into the junction next to the spawn, left in the middle of the box; the
/// player on the sidewalk `back` m from it (`sidewalk_at`), looking at it. Returns the app, the node,
/// its hub and the left car.
fn box_scene(seed: u64, back: f32) -> (App, u32, Vec3, Entity) {
    let mut app = city_app(seed);
    let spawn = app.world().resource::<PlayerSpawn>().0;
    let (node, hub) = nearest_box(&app, spawn);
    let feet = sidewalk_at(&app, hub, back);
    let dir = (hub - feet).with_y(0.0).normalize();
    stand_player(&mut app, Vec2::new(feet.x, feet.z), dir);
    run_ticks(&mut app, 2 * HZ);
    clear_spot(&mut app, hub, 6.0);
    let left = park_car(&mut app, hub, dir);
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let seen = app
        .world()
        .resource::<gta_sim::population::CameraView>()
        .0
        .as_ref()
        .is_some_and(|v| {
            gta_sim::traffic::in_frame(v, hub, Quat::IDENTITY, half, feet, f32::INFINITY)
        });
    assert!(
        seen,
        "GATE BROKEN: seed {seed}: the box {node} at {hub} is not in frame"
    );
    (app, node, hub, left)
}

/// A car left in the middle of the box next to the spawn (`box_scene`), watched for 150 s.
fn left_in_the_box(seed: u64, back: f32) -> LeftInBox {
    let (mut app, node, hub, left) = box_scene(seed, back);
    let mut clock = StandClock::default();
    let mut oracle = Footprints::new(&app).with_third_bodies();
    for tick in 0..150 * HZ {
        run_ticks(&mut app, 1);
        clock.record(&mut app);
        oracle.record(&mut app, tick);
    }
    let distance = (hub - position(&mut app)).with_y(0.0).length();
    let at = app.world().get::<Position>(left).map(|p| p.0);
    eprintln!(
        "seed {seed}: box {node} at ({:.1}, {:.1}), the player {distance:.1} m from it; stands > 30 s \
         within 45 m: {:?}; worst {:?}, worst dynamic {:.1} s; left car now at {at:?}; G1 max depth {:.3}, relaxed max depth {:.2}, relaxations {}",
        hub.x,
        hub.z,
        stands_near(&clock, hub, 45.0, 30.0),
        clock.worst(),
        clock.worst_dynamic(),
        oracle.max_depth(),
        oracle.relaxed_max_depth(),
        stats(&app).progress_relaxations
    );
    LeftInBox {
        hub,
        clock,
        oracle,
        left: at,
    }
}

/// R1 headless (the TASK-031 repro: the player 31 m from the box): no traffic car within 45 m of the
/// box stands longer than 30 s over 150 s, none in `Dynamic` either, G1 and third bodies clean. The
/// stuck cheat does not reach a car the player looks at from within `bubble.stuck_in_view_distance`;
/// the progress rule (TASK-039) does: the car behind the left car squeezes past it.
fn r1(seed: u64) {
    let run = left_in_the_box(seed, 31.0);
    let stood = stands_near(&run.clock, run.hub, 45.0, 30.0);
    let mut failures: Vec<String> = run.clock.dynamic_violation().into_iter().collect();
    if !stood.is_empty() {
        failures.push(format!("traffic stood > 30 s near the box: {stood:?}"));
    }
    if !run.oracle.violations.is_empty() {
        failures.push(format!("G1: {:?}", run.oracle.summary()));
    }
    if !run.oracle.third.violations.is_empty() {
        failures.push(format!("third bodies: {:?}", run.oracle.third_summary()));
    }
    failures.extend(run.oracle.relax_failure());
    assert!(failures.is_empty(), "seed {seed}: {failures:#?}");
}

#[test]
fn r1_car_left_in_the_box_seed_1() {
    r1(1);
}

#[test]
fn r1_car_left_in_the_box_seed_7() {
    r1(7);
}

/// Fallback R-B, near: the stuck cheat never pops a car in the box the player looks at from 25 m
/// (within `bubble.stuck_in_view_distance`).
#[test]
fn rb_a_box_car_seen_from_nearby_stays() {
    let run = left_in_the_box(1, 25.0);
    assert!(
        run.left.is_some(),
        "the stuck cheat despawned the car in the box the player looks at from nearby"
    );
}

/// Fallback R-B, far: seen from 60 m (past `bubble.stuck_in_view_distance`) the car in the box counts
/// as out of frame and is despawned; no AI car stands longer than 30 s in `Dynamic`, G1 clean. The
/// stands near the box are printed, not asserted: cars bumped at a lane end are given up where no pass
/// reaches (FIX_SUMMARY.md).
#[test]
fn rb_a_box_car_seen_from_afar_is_cleared() {
    let run = left_in_the_box(1, 60.0);
    let mut failures: Vec<String> = run.clock.dynamic_violation().into_iter().collect();
    if run.left.is_some() {
        failures.push(format!("the car in the box is still there: {:?}", run.left));
    }
    if !run.oracle.violations.is_empty() {
        failures.push(format!("G1: {:?}", run.oracle.summary()));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Spot C of the TASK-032 QA R1 runtime (seed 1, the car left in the box, the player 31 m away): QA saw
/// the south approach queue given up car by car, each switched by a walker pressed against it (13
/// switches by characters) and unable to recover, each given-up car the next obstacle. Here the
/// queue on the approach lane through (-4.5, -92) is filled up to 8 standing cars at the jam gap
/// behind the last AI car on the lane (the lane start is past the bubble's reach, a feeder there is
/// despawned); from
/// then on every queued car gets a dummy pressed at its right side for `PRESSED_S`: none of the cars
/// that stood in that queue is given up.
#[test]
fn spot_c_queue_behind_a_box_car_is_never_given_up() {
    let (mut app, _, hub, _) = box_scene(1, 31.0);
    let g = graph(&app);
    let probe = Vec3::new(-4.5, 0.0, -92.0);
    let (lane, off) = g
        .lanes()
        .iter()
        .enumerate()
        .filter(|(_, l)| l.dir.z > 0.9)
        .filter_map(|(k, l)| {
            let along = (probe - l.from).with_y(0.0).dot(l.dir);
            (0.0..=l.length).contains(&along).then(|| {
                (
                    k as u32,
                    (probe - l.from - l.dir * along).with_y(0.0).length(),
                )
            })
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("GATE BROKEN: no northbound lane through the spot C queue");
    assert!(
        off < 1.0,
        "GATE BROKEN: the spot C lane is {off:.2} m off (-4.5, -92)"
    );
    let l = g.lane(lane).clone();
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let radius = app
        .world()
        .resource::<gta_sim::character::LocomotionConfig>()
        .capsule_radius;
    let side = right_of(l.dir) * (half.x + radius + 0.05);
    // The queue: AI cars standing on the lane's line, from 60 m before its start to its end.
    let in_queue = |at: Vec3| {
        let d = (at - l.from).with_y(0.0);
        let along = d.dot(l.dir);
        (-60.0..=l.length).contains(&along) && (d - l.dir * along).length() < 1.7
    };
    let jam = 2.0 * half.z + app.world().resource::<TrafficConfig>().idm.min_gap;
    let mut press_from = None;
    let mut oracle = Footprints::new(&app);
    let mut clock = StandClock::default();
    let mut members: std::collections::HashSet<Entity> = Default::default();
    let mut pressed: Vec<(Entity, Entity, u32)> = Vec::new();
    let (mut longest, mut switched) = (0, std::collections::HashSet::new());
    let mut given_up: Vec<(Entity, f32, Vec3, TrafficMode)> = Vec::new();
    for tick in 0..90 * HZ {
        for &(_, dummy, until) in &pressed {
            if until == tick {
                app.world_mut().despawn(dummy);
            }
        }
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        clock.record(&mut app);
        let cars: Vec<(Entity, TrafficCar, Vec3, f32)> = app
            .world_mut()
            .query::<(Entity, &TrafficCar, &Position, &LinearVelocity)>()
            .iter(app.world())
            .map(|(e, c, p, v)| (e, *c, p.0, v.0.length()))
            .collect();
        let queue: Vec<&(Entity, TrafficCar, Vec3, f32)> = cars
            .iter()
            .filter(|c| c.1.is_ai() && c.3 < 0.5 && in_queue(c.2))
            .collect();
        longest = longest.max(queue.len());
        if tick == FILL_AT_S * HZ {
            // Behind the last AI car on the lane, moving or standing: a car still rolling up to the
            // queue holds its spot too.
            let tail = cars
                .iter()
                .filter(|c| c.1.is_ai() && c.1.segment == Segment::Lane(lane))
                .map(|c| c.1.s)
                .fold(f32::INFINITY, f32::min);
            for k in 1..=8usize.saturating_sub(queue.len()) {
                let s = tail - k as f32 * jam;
                if s > half.z + 1.0 {
                    spawn_traffic_car(&mut app, Segment::Lane(lane), s, 0.0);
                }
            }
        }
        if queue.len() >= 8 && press_from.is_none() {
            press_from = Some(tick);
        }
        for &&(e, car, at, _) in &queue {
            members.insert(e);
            if car.mode == TrafficMode::Dynamic {
                switched.insert(e);
            }
            let pressing = press_from.is_some_and(|t| tick < t + 10 * HZ);
            if pressing && !pressed.iter().any(|p| p.0 == e) {
                let feet = at + side;
                let y = ground_at(&mut app, Vec2::new(feet.x, feet.z));
                let dummy = spawn_dummy(&mut app, feet.with_y(y));
                pressed.push((e, dummy, tick + PRESSED_S * HZ));
            }
        }
        for (e, car, at, _) in &cars {
            let out = matches!(
                car.mode,
                TrafficMode::Abandoned | TrafficMode::Bailing { .. }
            );
            if out && members.contains(e) && !given_up.iter().any(|g| g.0 == *e) {
                given_up.push((*e, tick as f32 / HZ as f32, *at, car.mode));
            }
        }
    }
    eprintln!(
        "spot C: lane {lane} (box at ({:.1}, {:.1})), longest queue {longest} (pressing from {:?} s), {} cars stood in it, {} pressed, {} switched; given up: {given_up:?}; worst dynamic {:.1} s; G1 max depth {:.3}",
        hub.x,
        hub.z,
        press_from.map(|t| t as f32 / HZ as f32),
        members.len(),
        pressed.len(),
        switched.len(),
        clock.worst_dynamic(),
        oracle.max_depth()
    );
    assert!(
        longest >= 8,
        "GATE BROKEN: the spot C queue reached only {longest} cars"
    );
    assert!(
        switched.len() >= 4,
        "GATE BROKEN: the pressed dummies switched only {} queued cars to Dynamic",
        switched.len()
    );
    oracle.assert_clean("spot C queue");
    assert!(
        given_up.is_empty(),
        "cars of the queue behind the box car were given up: {given_up:?}"
    );
}
