mod common;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::character::LocomotionConfig;

struct Attempt {
    on_top: bool,
    settle_ticks: Option<u32>,
    buried_ticks: u32,
}

fn attempt(height: f32, max_height: Option<f32>) -> Attempt {
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
        Collider::cuboid(4.0, height, 20.0),
        Transform::from_xyz(25.0, height / 2.0, -12.0),
    ));
    let entity = player(&mut app);
    let start = Vec3::new(25.0, cfg.float_height, 3.0);
    app.world_mut().get_mut::<Position>(entity).unwrap().0 = start;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .unwrap()
        .translation = start;
    run_ticks(&mut app, 8);
    set_intent(&mut app, |intent| intent.axis = Vec2::Y);
    let mut jumped = false;
    let mut cleared = None;
    let mut settled = None;
    let mut buried_ticks = 0;
    for tick in 0..150 {
        let before = position(&mut app);
        if !jumped && before.z < -0.8 {
            set_intent(&mut app, |intent| {
                intent.jump_requested = true;
                intent.jump_held = true;
            });
            jumped = true;
        }
        run_ticks(&mut app, 1);
        let at = position(&mut app);
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
    let end = position(&mut app);
    Attempt {
        on_top: end.z < -2.3 && (end.y - (height + cfg.float_height)).abs() < 0.1,
        settle_ticks: cleared.zip(settled).map(|(start, end)| end - start),
        buried_ticks,
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
