mod common;

use bevy::prelude::*;
use common::*;
use gta_sim::character::{Gait, LocomotionConfig};

fn run_forward(yaw: f32, gait: Gait) -> (Vec3, f32) {
    let mut app = headless_app();
    settle(&mut app);
    let start = position(&mut app);
    set_intent(&mut app, |intent| {
        intent.axis = Vec2::Y;
        intent.yaw = yaw.to_radians();
        intent.gait = gait;
    });
    run_ticks(&mut app, 64);
    let end = position(&mut app);
    let config = app.world().resource::<LocomotionConfig>();
    let speed = match gait {
        Gait::Walk => config.walk_speed,
        Gait::Run => config.run_speed,
        Gait::Sprint => config.sprint_speed,
    };
    (end - start, speed)
}

fn assert_forward(distance: f32, drift: f32, speed: f32) {
    assert!(
        (0.8 * speed..=speed).contains(&distance),
        "distance={distance}, speed={speed}"
    );
    assert!(drift.abs() < 0.1, "side drift={drift}");
}

#[test]
fn forward_yaw_0_moves_neg_z() {
    let (delta, speed) = run_forward(0.0, Gait::Run);
    assert_forward(-delta.z, delta.x, speed);
}

#[test]
fn forward_yaw_90_moves_neg_x() {
    let (delta, speed) = run_forward(90.0, Gait::Run);
    assert_forward(-delta.x, delta.z, speed);
}

#[test]
fn forward_yaw_180_moves_pos_z() {
    let (delta, speed) = run_forward(180.0, Gait::Run);
    assert_forward(delta.z, delta.x, speed);
}

#[test]
fn sprint_uses_sprint_speed() {
    let (delta, speed) = run_forward(0.0, Gait::Sprint);
    assert_forward(-delta.z, delta.x, speed);
}
