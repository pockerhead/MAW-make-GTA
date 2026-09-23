#![allow(dead_code)]

use avian3d::prelude::Position;
use bevy::{asset::AssetPlugin, prelude::*, time::TimeUpdateStrategy};
use gta_sim::{
    character::{LocomotionConfig, MoveIntent},
    compose_sim,
    config::ConfigRoot,
    player::Player,
};
use std::path::Path;

pub fn assets_root() -> ConfigRoot {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    ConfigRoot(
        path.canonicalize()
            .unwrap_or_else(|_| panic!("GATE BROKEN: assets root not found at {}", path.display())),
    )
}

pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, AssetPlugin::default()))
        .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    compose_sim(&mut app, assets_root()).expect("GATE BROKEN: compose_sim failed");
    app.finish();
    app.cleanup();
    app
}

pub fn run_ticks(app: &mut App, count: u32) {
    let initial = app.world().resource::<Time<Fixed>>().elapsed();
    let step = app.world().resource::<Time<Fixed>>().timestep();
    for _ in 0..count + 4 {
        if app.world().resource::<Time<Fixed>>().elapsed() - initial >= step * count {
            return;
        }
        app.update();
    }
    panic!("GATE BROKEN: fixed loop stalled");
}

pub fn player(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .expect("GATE BROKEN: expected exactly one Player")
}

pub fn position(app: &mut App) -> Vec3 {
    let entity = player(app);
    app.world()
        .get::<Position>(entity)
        .expect("GATE BROKEN: player missing Position")
        .0
}

pub fn settle(app: &mut App) {
    run_ticks(app, 64);
    let y = position(app).y;
    let expected = app.world().resource::<LocomotionConfig>().float_height;
    assert!(
        (y - expected).abs() < 0.05,
        "GATE BROKEN: player not resting on floor at spawn, y={y}"
    );
}

pub fn set_intent(app: &mut App, update: impl FnOnce(&mut MoveIntent)) {
    let entity = player(app);
    update(
        app.world_mut()
            .get_mut::<MoveIntent>(entity)
            .expect("GATE BROKEN: missing MoveIntent")
            .as_mut(),
    );
}
