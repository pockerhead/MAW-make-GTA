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

use bevy::prelude::*;
use common::*;
use gta_sim::traffic::Segment;
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
