//! A walker pressed against the flank of a traffic car does not hold the car (TASK-016 QA bug 4: in
//! the city, cars stood 3-13 s inside intersections with a civilian pushing into their side, each
//! waiting for the other). The forward cast starts at the nose: only bodies ahead of the bumper stop
//! a car. A car switched to dynamic by the walker then pulls away at the IDM rate (it used to crawl
//! at a·dt·gain of throttle, 0.3 m in 5 s).
//!
//! Floor: lane 0 runs +X along z 30 from x −30 to 10. A dummy 10 m ahead of the bumper of a car
//! spawned at s 10 (centre x −20) holds it: IDM stops it `min_gap` (2 m) short of the dummy's
//! capsule, dummy centre x −7.96 → surface −8.26 → bumper −10.26 → centre −12.30. A sidewalk run
//! crosses the lane at x −11.15; a walker keeps 0.5 m right of its next node, so it walks into the
//! car's left flank 0.9 m ahead of the centre (the QA sample: ahead 0.9, side −1.5).

mod common;
mod traffic_support;

use avian3d::prelude::LinearVelocity;
use bevy::{ecs::message::MessageCursor, prelude::*};
use common::*;
use gta_sim::{
    character::{Dead, HealthConfig, LocomotionConfig},
    combat::HitReaction,
    config::load_config,
    traffic::{Segment, TRAFFIC_CONFIG, TrafficConfig},
    vehicle::VehicleHit,
};
use traffic_support::*;

const LANE_Z: f32 = 30.0;
const HELD_X: f32 = -12.30;
const RUN_X: f32 = HELD_X + 0.9 + 0.25;

#[test]
fn a_walker_at_the_flank_does_not_hold_the_car() {
    let at = |x: f32| Vec3::new(x, 0.0, LANE_Z);
    let run = (
        Vec3::new(RUN_X, 0.0, LANE_Z - 8.0),
        Vec3::new(RUN_X, 0.0, LANE_Z + 8.0),
    );
    let mut app = traffic_floor(
        vec![
            (at(-30.0), at(10.0), 12.0, 0),
            (at(20.0), at(25.0), 12.0, 1),
        ],
        &[(0, 1, 0), (1, 0, 1)],
        &[run],
    );
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 10.0, 0.0);
    let dummy = spawn_dummy(&mut app, Vec3::new(-7.96, 0.0, LANE_Z));
    for _ in 0..1920 {
        run_ticks(&mut app, 1);
        if traffic_car(&app, car).speed == 0.0 {
            break;
        }
    }
    let held = position_of(&app, car);
    assert!(
        traffic_car(&app, car).speed == 0.0 && (held.x - HELD_X).abs() < 0.3,
        "GATE BROKEN: the car is not held by the dummy at x {HELD_X}: {held}"
    );
    let walker = spawn_civilian(
        &mut app,
        gta_sim::navigation::GraphWalker { from: 0, to: 1 },
        0.0,
        calm(),
    );
    let health = health_of(&app, walker).current;
    // The walker reaches the flank and pushes into it.
    let mut pressed = 0;
    for _ in 0..1280 {
        run_ticks(&mut app, 1);
        let p = position_of(&app, walker) - position_of(&app, car);
        let beside = p.z.abs() < 1.2 + 0.3 + 0.1 && p.x.abs() < 2.04;
        pressed = if beside { pressed + 1 } else { 0 };
        if pressed >= 64 {
            break;
        }
    }
    let p = position_of(&app, walker) - position_of(&app, car);
    assert!(
        pressed >= 64,
        "GATE BROKEN: the walker never pressed against the flank: offset {p}"
    );
    eprintln!("walker at {p} from the car centre");
    // The way ahead clears: the car drives off, the walker at its flank notwithstanding.
    app.world_mut().despawn(dummy);
    let start = position_of(&app, car);
    run_ticks(&mut app, 320);
    let moved = (position_of(&app, car) - start).with_y(0.0).length();
    eprintln!("moved {moved:.2} m in 5 s");
    assert!(
        moved > 3.0,
        "the car stood {moved:.2} m in 5 s with a walker at its flank (offset {p})"
    );
    assert_eq!(
        health_of(&app, walker).current,
        health,
        "the walker was hurt by the car"
    );
}

/// A walker standing just ahead of the bumper's corner still stops the car.
#[test]
fn a_walker_ahead_of_the_bumper_still_holds_the_car() {
    let at = |x: f32| Vec3::new(x, 0.0, LANE_Z);
    let mut app = traffic_floor(
        vec![
            (at(-30.0), at(10.0), 12.0, 0),
            (at(20.0), at(25.0), 12.0, 1),
        ],
        &[(0, 1, 0), (1, 0, 1)],
        &[],
    );
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 10.0, 0.0);
    // 0.3 m ahead of the bumper (x −17.96 + 0.3 + radius), 1.0 m left of the centre line.
    spawn_dummy(&mut app, Vec3::new(-20.0 + 2.04 + 0.6, 0.0, LANE_Z - 1.0));
    let start = position_of(&app, car);
    run_ticks(&mut app, 640);
    let moved = (position_of(&app, car) - start).with_y(0.0).length();
    assert!(
        moved < 0.1,
        "the car moved {moved:.2} m into a walker ahead of its bumper"
    );
}

/// A traffic car cruising at the fastest lane speed (`traffic.ron` v0) meets the full-health player who
/// stepped onto its lane 1 m ahead of the bumper: IDM brakes, the car still hits at about the cruise
/// speed, and the player is hurt and knocked down, not killed (TASK-035).
#[test]
fn a_cruising_car_does_not_kill_a_full_health_player() {
    cruise_hit(false);
}

/// The same car meets the player running at `run_speed` head-on into it from 4 m ahead of the bumper:
/// the closing speed is cruise plus run, and the player still lives (TASK-035, review m3).
#[test]
fn a_cruising_car_does_not_kill_a_player_running_into_it() {
    cruise_hit(true);
}

fn cruise_hit(running: bool) {
    let at = |x: f32| Vec3::new(x, 0.0, LANE_Z);
    let v0 = load_config::<TrafficConfig>(&assets_root(), TRAFFIC_CONFIG)
        .expect("GATE BROKEN: shipped traffic config")
        .desired_speed;
    let cruise = v0.avenue.max(v0.street);
    let mut app = traffic_floor(
        vec![
            (at(-30.0), at(10.0), cruise, 0),
            (at(20.0), at(25.0), cruise, 1),
        ],
        &[(0, 1, 0), (1, 0, 1)],
        &[],
    );
    let max = app.world().resource::<HealthConfig>().max_health;
    assert_eq!(
        (health(&mut app).current, health(&mut app).armor),
        (max, 0.0),
        "GATE BROKEN: the player is not at full health without armour"
    );
    let loco = app.world().resource::<LocomotionConfig>().clone();
    let lead = if running { 4.0 } else { 1.0 };
    // Car centre at s 2 (x −28) moves 0.25 m in its spawn tick; bumper 2.04 ahead, capsule radius 0.3.
    place_player(
        &mut app,
        Vec3::new(-28.0 + 0.25 + 2.04 + lead + 0.3, loco.float_height, LANE_Z),
    );
    if running {
        // Run gait towards −X, into the oncoming car.
        set_intent(&mut app, |i| {
            i.axis = Vec2::Y;
            i.yaw = std::f32::consts::FRAC_PI_2;
        });
    }
    let run = if running { loco.run_speed } else { 0.0 };
    let me = player(&mut app);
    let mut hits: MessageCursor<VehicleHit> = app
        .world()
        .resource::<Messages<VehicleHit>>()
        .get_cursor_current();
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 2.0, cruise);
    let mut log = Vec::new();
    let mut knocked_down = false;
    // The player's speed towards the car (−X) before each tick until the first hit; the last sample
    // is already the contact push, the one before it is the approach.
    let mut approach = Vec::new();
    for _ in 0..192 {
        if log.is_empty() {
            approach.push(-app.world().get::<LinearVelocity>(me).unwrap().x);
        }
        run_ticks(&mut app, 1);
        log.extend(
            hits.read(app.world().resource::<Messages<VehicleHit>>())
                .filter(|h| h.target == me && h.vehicle == car)
                .map(|h| h.speed),
        );
        knocked_down |= app
            .world()
            .get::<HitReaction>(me)
            .is_some_and(|r| r.is_knocked_down());
    }
    let health = health(&mut app).current;
    let toward = approach[approach.len().saturating_sub(2)];
    println!(
        "{cruise} m/s lane, player {toward:.2} m/s towards the car: hit speeds {log:.2?}, health {health}"
    );
    let first = *log
        .first()
        .expect("GATE BROKEN: the car never hit the player");
    assert!(
        toward >= 0.9 * run,
        "GATE BROKEN: the player met the car at {toward:.2} m/s, not running ({run})"
    );
    // IDM brakes for a body closing on the bumper: the car itself stays within 2 m/s of cruise.
    assert!(
        first - toward >= cruise - 2.0,
        "GATE BROKEN: the car hit at {:.2} m/s of its own, not at the cruise speed {cruise}",
        first - toward
    );
    assert!(
        app.world().get::<Dead>(me).is_none() && health > 0.0,
        "a traffic car at {first:.2} m/s killed a full-health player (hit speeds {log:.2?})"
    );
    assert!(health < max, "the hit did not hurt");
    assert!(knocked_down, "not knocked down at {first:.2} m/s");
}
