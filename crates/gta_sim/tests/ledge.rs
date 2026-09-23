mod common;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::character::LocomotionConfig;

struct Attempt {
    on_top: bool,
    settle_ticks: Option<u32>,
    buried_ticks: u32,
    ceiling_overlap: bool,
    reached_wall: bool,
}

fn attempt(height: f32, max_height: Option<f32>) -> Attempt {
    attempt_with_obstacle(height, max_height, 0.0, false, false)
}

fn attempt_with_obstacle(
    height: f32,
    max_height: Option<f32>,
    angle_deg: f32,
    ceiling: bool,
    angle_gate: bool,
) -> Attempt {
    let mut app = headless_app();
    settle(&mut app);
    if let Some(max_height) = max_height {
        app.world_mut()
            .resource_mut::<LocomotionConfig>()
            .ledge_assist_max_height = max_height;
    }
    let cfg = app.world().resource::<LocomotionConfig>().clone();
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(100.0, height, 20.0),
        Transform::from_xyz(25.0, height / 2.0, -12.0),
    ));
    if ceiling {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(4.0, 0.25, 4.0),
            Transform::from_xyz(25.0, height + 1.375, -4.0),
        ));
    }
    let entity = player(&mut app);
    let start = Vec3::new(25.0, cfg.float_height, if angle_gate { 4.0 } else { 3.0 });
    app.world_mut().get_mut::<Position>(entity).unwrap().0 = start;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .unwrap()
        .translation = start;
    run_ticks(&mut app, 8);
    set_intent(&mut app, |intent| {
        intent.axis = Vec2::Y;
        intent.yaw = angle_deg.to_radians();
    });
    let mut jumped = false;
    let mut cleared = None;
    let mut settled = None;
    let mut buried_ticks = 0;
    let mut ceiling_overlap = false;
    let mut reached_wall = false;
    let mut on_top = false;
    let ticks = if angle_gate { 400 } else { 300 };
    let jump_z = if angle_gate { -1.0 } else { -0.8 };
    for tick in 0..ticks {
        let before = position(&mut app);
        if !jumped && before.z < jump_z {
            set_intent(&mut app, |intent| {
                intent.jump_requested = true;
                intent.jump_held = true;
            });
            jumped = true;
        }
        run_ticks(&mut app, 1);
        let at = position(&mut app);
        reached_wall |= at.z < -1.6;
        on_top |= at.z < -2.3 && (at.y - (height + cfg.float_height)).abs() < 0.15;
        if ceiling && (at.x - 25.0).abs() < 1.7 && (-6.0..=-2.0).contains(&at.z) {
            ceiling_overlap |= at.y + cfg.capsule_height / 2.0 > height + 1.25
                && at.y - cfg.capsule_height / 2.0 < height + 1.5;
        }
        if at.z < -2.0 - cfg.capsule_radius && cleared.is_none() {
            cleared = Some(tick);
        }
        if cleared.is_some() && at.y - cfg.float_height < height - 0.05 {
            buried_ticks += 1;
        }
        if cleared.is_some()
            && settled.is_none()
            && (at.y - (height + cfg.float_height)).abs() < 0.05
        {
            settled = Some(tick);
        }
    }
    Attempt {
        on_top,
        settle_ticks: cleared.zip(settled).map(|(start, end)| end - start),
        buried_ticks,
        ceiling_overlap,
        reached_wall,
    }
}

#[test]
fn configured_reach_climbs_and_above_reach_blocks() {
    let cfg = headless_app()
        .world()
        .resource::<LocomotionConfig>()
        .clone();
    let reachable = attempt(cfg.ledge_assist_max_height, None);
    assert!(reachable.on_top, "configured reach was not climbed");
    let too_high = attempt(cfg.ledge_assist_max_height + 0.3, None);
    assert!(!too_high.on_top, "ledge above configured reach was climbed");
    let reduced_reach = attempt(cfg.ledge_assist_max_height, Some(cfg.jump_height + 0.1));
    assert!(
        !reduced_reach.on_top,
        "changing the configured max height did not restrict the same ledge"
    );
}

#[test]
fn pull_up_settles_quickly_without_burying_feet() {
    let cfg = headless_app()
        .world()
        .resource::<LocomotionConfig>()
        .clone();
    let result = attempt(cfg.ledge_assist_max_height, None);
    assert!(result.on_top, "pull-up did not finish");
    assert!(
        result.settle_ticks.is_some_and(|ticks| ticks <= 19),
        "pull-up took {:?} ticks after clearing the edge",
        result.settle_ticks
    );
    assert!(
        result.buried_ticks <= 6,
        "feet were inside the ledge for {} ticks",
        result.buried_ticks
    );
}

#[test]
fn configured_limit_blocks_oblique_approaches() {
    let mut climbed = Vec::new();
    for angle in [0.0, 45.0, 62.0, 75.0] {
        let result = attempt_with_obstacle(1.3, Some(1.2), angle, false, true);
        assert!(
            result.reached_wall,
            "{angle} degree approach missed the wall"
        );
        if result.on_top {
            climbed.push(angle);
        }
    }
    assert!(
        climbed.is_empty(),
        "above-limit climbs at {climbed:?} degrees"
    );
}

#[test]
fn low_ceiling_blocks_pull_up_snap() {
    let result = attempt_with_obstacle(1.4, None, 0.0, true, false);
    assert!(!result.ceiling_overlap, "capsule snapped into low ceiling");
}

#[test]
fn landing_on_crate_allows_next_jump_over_short_step() {
    let mut app = headless_app();
    settle(&mut app);
    let cfg = app.world().resource::<LocomotionConfig>().clone();
    for (height, depth, z) in [(0.8, 2.0, -2.0), (1.5, 97.0, -51.5)] {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(100.0, height, depth),
            Transform::from_xyz(25.0, height / 2.0, z),
        ));
    }
    let entity = player(&mut app);
    let start = Vec3::new(25.0, cfg.float_height, 1.0);
    app.world_mut().get_mut::<Position>(entity).unwrap().0 = start;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .unwrap()
        .translation = start;
    run_ticks(&mut app, 16);
    set_intent(&mut app, |intent| {
        intent.axis = Vec2::Y;
        intent.jump_requested = true;
        intent.jump_held = true;
    });
    let mut cleared = false;
    for tick in 0..180 {
        if tick > 60 && tick % 40 == 0 {
            set_intent(&mut app, |intent| intent.jump_held = false);
        }
        if tick > 60 && tick % 40 == 5 {
            set_intent(&mut app, |intent| {
                intent.jump_requested = true;
                intent.jump_held = true;
            });
        }
        run_ticks(&mut app, 1);
        let at = position(&mut app);
        cleared |= at.z < -3.3 && at.y > 1.5 + cfg.float_height - 0.2;
    }
    assert!(cleared, "second jump could not clear 0.7 m step");
}

fn crate_to_wall_with_one_jump(buffered_tap: bool) -> Option<u32> {
    let mut app = headless_app();
    settle(&mut app);
    let cfg = app.world().resource::<LocomotionConfig>().clone();
    for (height, depth, z) in [(0.8, 2.0, -2.0), (1.5, 97.0, -51.5)] {
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(100.0, height, depth),
            Transform::from_xyz(25.0, height / 2.0, z),
        ));
    }
    let entity = player(&mut app);
    let start = Vec3::new(25.0, cfg.float_height, 1.0);
    app.world_mut().get_mut::<Position>(entity).unwrap().0 = start;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .unwrap()
        .translation = start;
    run_ticks(&mut app, 16);
    set_intent(&mut app, |intent| {
        intent.axis = Vec2::Y;
        intent.jump_requested = true;
        intent.jump_held = true;
    });
    for tick in 0..180 {
        if tick == 20 || (buffered_tap && tick == 43) {
            set_intent(&mut app, |intent| intent.jump_held = false);
        }
        if buffered_tap && tick == 28 {
            set_intent(&mut app, |intent| {
                intent.jump_requested = true;
                intent.jump_held = true;
            });
        }
        run_ticks(&mut app, 1);
        let at = position(&mut app);
        if at.z < -3.3 && at.y > 1.5 + cfg.float_height - 0.2 {
            return Some(tick);
        }
    }
    None
}

#[test]
fn walking_from_crate_clears_short_step() {
    assert!(
        crate_to_wall_with_one_jump(false).is_some(),
        "walking from the 0.8 m crate could not clear the 0.7 m step"
    );
}

#[test]
fn buffered_jump_from_crate_clears_short_step() {
    assert!(
        crate_to_wall_with_one_jump(true).is_some(),
        "buffered jump from the 0.8 m crate could not clear the 0.7 m step"
    );
}
