//! Car physics on the test floor (GDD §5, T14), production composition: rest, tunnelling, crash
//! damage and stall, rollover and steering signs.

mod common;
mod vehicle_support;

use avian3d::prelude::*;
use bevy::{ecs::message::MessageCursor, prelude::*};
use common::*;
use gta_sim::{
    vehicle::{GRAVITY, Vehicle, VehicleImpact},
    world::CityBlock,
};
use vehicle_support::*;

// ---------------------------------------------------------------- G5 rest

fn rest_run(driven: bool) {
    let mut app = headless_app();
    settle(&mut app);
    let car = spawn_car(&mut app, Vec2::new(-25.0, 0.0), 0.0);
    // A spawned car has no previous compression: no damper kick on its first tick.
    let hop = velocity_of(&app, car).y;
    assert!(hop.abs() < 0.1, "spawn kick: {hop:.3} m/s vertical");
    if driven {
        drive_in(&mut app, car);
    }
    let rest = vehicle_cfg(&app).rest_height();
    // A drop (0.5 m/s down, 0.053 m amplitude, ζ 0.4 → ×0.02 in 1 s) and a creep (0.02 m/s):
    // the damper and the hold force must both do work for the bounds to hold.
    app.world_mut().get_mut::<LinearVelocity>(car).unwrap().0 = Vec3::NEG_Y * 0.5;
    run_ticks(&mut app, 64);
    kick(&mut app, car, 0.02);
    let start = position_of(&app, car);
    let start_yaw = yaw_of(&app, car);
    for tick in 0..640 {
        run_ticks(&mut app, 1);
        let at = position_of(&app, car);
        assert!(
            (at.y - rest).abs() < 0.02,
            "tick {tick}: centre y {} off rest height {rest}",
            at.y
        );
    }
    let drift = flat(position_of(&app, car) - start).length();
    let turned = (yaw_of(&app, car) - start_yaw).to_degrees().abs();
    assert!(drift < 0.02, "drifted {drift:.4} m in 640 ticks");
    assert!(turned < 0.2, "turned {turned:.3} deg in 640 ticks");
    if !driven {
        assert!(
            app.world().get::<Sleeping>(car).is_some(),
            "parked car not asleep after 640 ticks"
        );
    }
}

#[test]
fn driven_car_at_rest_does_not_drift() {
    rest_run(true);
}

#[test]
fn parked_car_at_rest_does_not_drift_and_sleeps() {
    rest_run(false);
}

// ---------------------------------------------------------------- G1 / G2 tunnelling

/// Drives `car` at max speed with full throttle for 128 ticks; `inside` flags a centre past the
/// obstacle. Returns (top speed before contact, final speed).
fn ram(app: &mut App, car: Entity, inside: impl Fn(Vec3) -> bool, contact: impl Fn(Vec3) -> bool) {
    let max = vehicle_cfg(app).max_speed;
    drive_in(app, car);
    run_ticks(app, 32);
    kick(app, car, max);
    set_drive(app, |d| d.throttle = 1.0);
    let mut top: f32 = 0.0;
    let mut touched = false;
    for tick in 0..128 {
        run_ticks(app, 1);
        let at = position_of(app, car);
        assert!(!inside(at), "tick {tick}: centre {at} passed the obstacle");
        assert!(at.y < 4.0, "tick {tick}: centre {at} climbed");
        touched |= contact(at);
        if !touched {
            top = top.max(velocity_of(app, car).length());
        }
    }
    assert!(
        top >= 0.95 * max,
        "liveness: top speed before contact {top:.2} < {:.2}",
        0.95 * max
    );
    let end = velocity_of(app, car).length();
    assert!(end <= 3.0, "still moving at {end:.2} m/s after the hit");
}

/// Wall 12 x 4 x 0.5 at (0, 2, 14): near face z 13.75. A filter without `World` drives through it;
/// a margin flip stays green (speculative contacts stop a 0.44 m/tick box either way).
#[test]
fn car_at_max_speed_does_not_tunnel_the_wall() {
    let mut app = headless_app();
    settle(&mut app);
    let car = spawn_car(&mut app, Vec2::new(0.0, -10.0), 180.0);
    let half = vehicle_cfg(&app).half_extents().z;
    let limit = 13.75 - half + 0.1;
    ram(&mut app, car, |at| at.z > limit, |at| at.z > limit - 0.2);
}

#[test]
fn car_at_max_speed_does_not_tunnel_a_corner() {
    let mut app = headless_app();
    settle(&mut app);
    spawn_wall(&mut app, Vec3::new(33.0, 5.0, -33.0), Vec3::splat(10.0));
    let car = spawn_car(&mut app, Vec2::new(17.4, -17.4), -45.0);
    let door = door_of(&app, car);
    assert!(
        flat(door).distance(Vec2::new(16.41, -18.81)) < 0.02,
        "GATE BROKEN: door {door}"
    );
    let half = vehicle_cfg(&app).half_extents().z;
    // The front corner reaches the box when the centre is `half` short of the corner diagonal.
    ram(
        &mut app,
        car,
        |at| at.x > 28.0 && at.z < -28.0,
        |at| at.x > 28.0 - half * 0.75,
    );
}

// ---------------------------------------------------------------- G6 crash damage and stall

/// A driven car with its front 1.0 m short of the test wall (near face z 13.75), health set to
/// `health`, coasting into the wall at `speed`; returns after 64 ticks.
fn crash(speed: f32, health: f32) -> (App, Entity) {
    let mut app = headless_app();
    settle(&mut app);
    let half = vehicle_cfg(&app).half_extents().z;
    let car = spawn_car(&mut app, Vec2::new(0.0, 13.75 - half - 1.0), 180.0);
    drive_in(&mut app, car);
    run_ticks(&mut app, 32);
    set_car_health(&mut app, car, health);
    kick(&mut app, car, speed);
    run_ticks(&mut app, 64);
    let front = position_of(&app, car).z + half;
    assert!(
        front > 13.75 - 0.1,
        "GATE BROKEN: the car at {speed} m/s did not reach the wall (front z {front})"
    );
    (app, car)
}

#[test]
fn slow_crash_does_not_hurt_the_car() {
    let (app, car) = crash(4.0, 1000.0);
    assert_eq!(car_health(&app, car), 1000.0);
}

/// (closing − 5)·40 with closing 10 minus ≤ 0.2 m/s of coasting: 192..200.
#[test]
fn crash_at_10_mps_costs_the_formula() {
    let (app, car) = crash(10.0, 1000.0);
    let loss = 1000.0 - car_health(&app, car);
    assert!((192.0..=200.0).contains(&loss), "car lost {loss}");
}

/// A car at 0 health ignores the throttle (reverse, away from the wall).
#[test]
fn wrecked_car_stalls() {
    let (mut app, car) = crash(10.0, 150.0);
    assert_eq!(car_health(&app, car), 0.0, "150 - 196 must clamp to 0");
    let from = position_of(&app, car);
    set_drive(&mut app, |d| d.throttle = -1.0);
    run_ticks(&mut app, 64);
    let speed = velocity_of(&app, car).length();
    let moved = flat(position_of(&app, car) - from).length();
    assert!(speed < 0.3, "stalled car moves at {speed:.2} m/s");
    assert!(moved < 0.1, "stalled car moved {moved:.3} m");
}

/// Two cars meet at 10 m/s: both lose (closing − 5)·40 and both get a `VehicleImpact` at one point.
/// `parked_rams` puts the driven car on the second side of avian's pair (the moving body comes
/// first), the side the old code dropped. Flip: write the impact for the first car only.
fn car_car_crash(parked_rams: bool) {
    let mut app = headless_app();
    settle(&mut app);
    // Faces 5.92 m apart; the parked car faces the driven one.
    let parked = spawn_car(&mut app, Vec2::new(-25.0, -10.0), 180.0);
    let mine = spawn_car(&mut app, Vec2::new(-25.0, 0.0), 0.0);
    drive_in(&mut app, mine);
    run_ticks(&mut app, 32);
    let mut starts: MessageCursor<CollisionStart> = app
        .world()
        .resource::<Messages<CollisionStart>>()
        .get_cursor_current();
    let mut impacts: MessageCursor<VehicleImpact> = app
        .world()
        .resource::<Messages<VehicleImpact>>()
        .get_cursor_current();
    let rammer = if parked_rams { parked } else { mine };
    kick(&mut app, rammer, 10.0);
    let (mut pair, mut log) = (Vec::new(), Vec::new());
    let mut contact_speed = None;
    // The crash tick's `CollisionStart` is read on the next tick: keep two ticks of speed.
    let mut now = velocity_of(&app, rammer).length();
    for _ in 0..64 {
        let before = now;
        now = velocity_of(&app, rammer).length();
        run_ticks(&mut app, 1);
        let world = app.world();
        pair.extend(
            starts
                .read(world.resource::<Messages<CollisionStart>>())
                .filter(|s| s.body1 == Some(mine) || s.body2 == Some(mine))
                .map(|s| (s.body1, s.body2)),
        );
        let fresh: Vec<VehicleImpact> = impacts
            .read(world.resource::<Messages<VehicleImpact>>())
            .copied()
            .collect();
        if !fresh.is_empty() && contact_speed.is_none() {
            contact_speed = Some(before);
        }
        log.extend(fresh);
    }
    let second = (Some(parked), Some(mine));
    if parked_rams {
        assert!(
            pair.contains(&second),
            "GATE BROKEN: the driven car is not the second body: {pair:?}"
        );
    }
    let speed = contact_speed.expect("GATE BROKEN: no crash");
    assert!(speed > 9.0, "GATE BROKEN: contact at {speed:.2} m/s");
    // Speed before the crash step; that step coasts ≤ 1/64 m/s more.
    let expected = (speed - 5.0) * 40.0;
    let max = damage_cfg(&app).vehicle.max_health;
    for (name, car) in [("driven", mine), ("parked", parked)] {
        let loss = max - car_health(&app, car);
        assert!(
            (loss - expected).abs() <= 3.0,
            "{name} car lost {loss}, expected {expected:.1}"
        );
    }
    let of = |car: Entity| log.iter().filter(|i| i.vehicle == car).collect::<Vec<_>>();
    let (own, theirs) = (of(mine), of(parked));
    assert!(
        own.iter().any(|i| i.speed >= 9.0),
        "no crash impact for the driven car: {log:?}"
    );
    assert!(
        own.iter().any(|i| theirs
            .iter()
            .any(|t| t.point == i.point && t.speed == i.speed)),
        "the two impacts differ: {log:?}"
    );
}

#[test]
fn driven_car_rams_a_parked_one() {
    car_car_crash(false);
}

#[test]
fn parked_car_rolls_into_the_driven_one() {
    car_car_crash(true);
}

// ---------------------------------------------------------------- G7 rollover and steering signs

/// Full throttle and full right lock from 28 m/s on a 400 m floor: the tyres slide, the car stays
/// on its wheels.
#[test]
fn hard_turn_at_max_speed_does_not_roll() {
    let mut app = headless_app();
    settle(&mut app);
    spawn_wall(
        &mut app,
        Vec3::new(500.0, -0.5, 500.0),
        Vec3::new(400.0, 1.0, 400.0),
    );
    let car = spawn_car(&mut app, Vec2::new(500.0, 600.0), 0.0);
    drive_in(&mut app, car);
    run_ticks(&mut app, 32);
    let max = vehicle_cfg(&app).max_speed;
    kick(&mut app, car, max);
    set_drive(&mut app, |d| {
        d.throttle = 1.0;
        d.steer = 1.0;
    });
    let mut lowest: f32 = 1.0;
    for _ in 0..192 {
        run_ticks(&mut app, 1);
        lowest = lowest.min((rotation_of(&app, car) * Vec3::Y).y);
    }
    assert!(lowest >= 0.5, "car tipped: lowest up.y {lowest:.3}");
    let speed = velocity_of(&app, car).length();
    assert!(
        speed >= 8.0,
        "liveness: {speed:.2} m/s at the end of the turn"
    );
    let turned = yaw_of(&app, car).to_degrees();
    assert!(turned < -30.0, "liveness: turned only {turned:.1} deg");
}

/// From rest, full throttle for 64 ticks: `(yaw, steer) -> (displacement, yaw change)`.
fn drive_from_rest(yaw: f32, steer: f32) -> (Vec3, f32) {
    let mut app = headless_app();
    settle(&mut app);
    let car = spawn_car(&mut app, Vec2::new(-25.0, 0.0), yaw);
    drive_in(&mut app, car);
    run_ticks(&mut app, 32);
    let from = position_of(&app, car);
    let yaw0 = yaw_of(&app, car);
    set_drive(&mut app, |d| {
        d.throttle = 1.0;
        d.steer = steer;
    });
    run_ticks(&mut app, 64);
    let turn = (yaw_of(&app, car) - yaw0).to_degrees();
    (position_of(&app, car) - from, turn)
}

#[test]
fn drive_and_steer_signs() {
    let (d, _) = drive_from_rest(0.0, 0.0);
    assert!(d.z < -1.0, "yaw 0 drove {d}");
    let (d, _) = drive_from_rest(90.0, 0.0);
    assert!(d.x < -1.0, "yaw 90 drove {d}");
    let (d, _) = drive_from_rest(180.0, 0.0);
    assert!(d.z > 1.0, "yaw 180 drove {d}");
    let (d, turn) = drive_from_rest(0.0, 1.0);
    assert!(d.x > 0.2, "right steer drove {d}");
    assert!(turn < -5.0, "right steer turned {turn:.1} deg");
}

// ---------------------------------------------------------------- G13 curbs and the underbody

/// Far floor (top y 0) and a city-style curb: a convex `CityBlock` prism, face at z 580, top
/// 0.15 (`city.ron` curb_height), sidewalk z 480..580.
fn curb_app() -> App {
    let mut app = headless_app();
    settle(&mut app);
    spawn_wall(
        &mut app,
        Vec3::new(500.0, -0.5, 500.0),
        Vec3::new(400.0, 1.0, 400.0),
    );
    let points: Vec<Vec3> = [
        (350.0, 480.0),
        (650.0, 480.0),
        (650.0, 580.0),
        (350.0, 580.0),
    ]
    .into_iter()
    .flat_map(|(x, z)| [Vec3::new(x, -1.0, z), Vec3::new(x, 0.15, z)])
    .collect();
    app.world_mut().spawn((
        CityBlock,
        RigidBody::Static,
        Collider::convex_hull(points).expect("GATE BROKEN: curb prism"),
        Transform::IDENTITY,
    ));
    app
}

/// Full throttle at 20 m/s into the curb, `yaw_deg` off square, nose 8 m short of it: no damage,
/// never thrown back or caught, and the car ends on the sidewalk. No wheel pushes harder than a
/// fully compressed spring plus the damper at its rate cap (k·travel + c·max_damper_speed = 9.1 kN);
/// the step jumps the compression within one tick, and without the cap that is a ~28 kN kick.
/// Flip: no clamp on the rate in `spring_force`.
fn climb_curb(yaw_deg: f32) {
    let mut app = curb_app();
    let yaw = yaw_deg.to_radians();
    let back = 8.0 + vehicle_cfg(&app).half_extents().z;
    let car = spawn_car(
        &mut app,
        Vec2::new(500.0 + back * yaw.sin(), 580.0 + back * yaw.cos()),
        yaw_deg,
    );
    drive_in(&mut app, car);
    run_ticks(&mut app, 32);
    let forward = forward_of(&app, car);
    kick(&mut app, car, 20.0);
    set_drive(&mut app, |d| d.throttle = 1.0);
    let mut slowest = f32::MAX;
    let mut peak = 0f32;
    for _ in 0..96 {
        run_ticks(&mut app, 1);
        slowest = slowest.min(velocity_of(&app, car).dot(forward));
        let wheels = app.world().get::<Vehicle>(car).unwrap().wheels;
        peak = wheels.iter().fold(peak, |peak, w| peak.max(w.force));
    }
    let cfg = vehicle_cfg(&app);
    let ceiling = cfg.spring_rate() * cfg.suspension.travel
        + cfg.damper_rate() * cfg.suspension.max_damper_speed;
    assert!(
        peak <= ceiling,
        "{yaw_deg} deg: a wheel pushed {peak:.0} N at the curb, over {ceiling:.0} N"
    );
    let static_load = cfg.mass / 4.0 * GRAVITY;
    assert!(
        peak > 2.0 * static_load,
        "GATE BROKEN: peak {peak:.0} N, the wheels never took the step"
    );
    let loss = damage_cfg(&app).vehicle.max_health - car_health(&app, car);
    assert_eq!(loss, 0.0, "{yaw_deg} deg: the curb cost {loss}");
    assert!(
        slowest >= 15.0,
        "{yaw_deg} deg: caught or thrown back, slowest {slowest:.2} m/s along the heading"
    );
    let at = position_of(&app, car);
    let on_sidewalk = vehicle_cfg(&app).rest_height() + 0.15;
    assert!(
        at.z < 580.0 - 5.0 && (at.y - on_sidewalk).abs() < 0.05,
        "{yaw_deg} deg: ended at {at}, not on the sidewalk (centre y {on_sidewalk:.3})"
    );
}

/// A city curb at speed is climbed by the wheels (owner report: the nose caught it, 470+ damage
/// and a bounce). Flip: `underbody` lift 0 and chamfer 0 in `sedan.ron`.
#[test]
fn curb_at_20_mps_is_climbed_square() {
    climb_curb(0.0);
}

#[test]
fn curb_at_20_mps_is_climbed_at_30_deg() {
    climb_curb(30.0);
}

/// Dropped flat at 8 m/s, the springs bottom out and the underbody hits the floor: a scrape, not a
/// crash. Flip: skip the `scrape_normal` check in `apply_impacts`.
#[test]
fn flat_landing_is_not_a_crash() {
    let mut app = headless_app();
    settle(&mut app);
    let car = spawn_car(&mut app, Vec2::new(-25.0, 0.0), 0.0);
    run_ticks(&mut app, 32);
    let mut starts: MessageCursor<CollisionStart> = app
        .world()
        .resource::<Messages<CollisionStart>>()
        .get_cursor_current();
    app.world_mut().get_mut::<LinearVelocity>(car).unwrap().0 = Vec3::NEG_Y * 8.0;
    let mut touched = 0;
    for _ in 0..32 {
        run_ticks(&mut app, 1);
        touched += starts
            .read(app.world().resource::<Messages<CollisionStart>>())
            .filter(|s| s.body1 == Some(car) || s.body2 == Some(car))
            .count();
    }
    assert!(
        touched > 0,
        "GATE BROKEN: the underbody never touched the floor"
    );
    let loss = damage_cfg(&app).vehicle.max_health - car_health(&app, car);
    assert_eq!(loss, 0.0, "a flat landing cost {loss}");
}
