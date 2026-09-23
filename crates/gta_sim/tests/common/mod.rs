#![allow(dead_code)]

use avian3d::prelude::Position;
use bevy::{
    asset::AssetPlugin, ecs::query::QueryFilter, prelude::*, state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use gta_sim::{
    character::{Health, LocomotionConfig, MoveIntent},
    compose_sim,
    config::ConfigRoot,
    flow::{GameState, WastedPhase},
    player::{DebugDamage, Player},
    world::{CityParams, CityParamsRes, PlayerSpawn, WorldSource},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub fn assets_root() -> ConfigRoot {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    ConfigRoot(
        path.canonicalize()
            .unwrap_or_else(|_| panic!("GATE BROKEN: assets root not found at {}", path.display())),
    )
}

pub fn composed_app(source: WorldSource) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    compose_sim(&mut app, assets_root(), source).expect("GATE BROKEN: compose_sim failed");
    app.finish();
    app.cleanup();
    app
}

pub fn headless_app() -> App {
    composed_app(WorldSource::TestArea)
}

/// Headless app with a generated city, updated until `GameState::Playing`.
pub fn city_app(seed: u64) -> App {
    let mut app = composed_app(WorldSource::City { seed });
    let deadline = Instant::now() + Duration::from_secs(120);
    while *app.world().resource::<State<GameState>>().get() != GameState::Playing {
        app.update();
        if let Some(exit) = app.should_exit() {
            panic!("city generation failed: {exit:?}");
        }
        assert!(
            Instant::now() < deadline,
            "city generation did not finish in 120 s"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    app
}

/// Golden layout hash of `seed` from citygen's `golden_hashes.txt`; line-ending independent.
pub fn golden(seed: u64) -> u64 {
    include_str!("../../../citygen/tests/golden_hashes.txt")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(' '))
        .find(|(s, _)| s.parse() == Ok(seed))
        .map(|(_, hash)| {
            u64::from_str_radix(hash.trim().trim_start_matches("0x"), 16)
                .unwrap_or_else(|_| panic!("GATE BROKEN: bad golden hash for seed {seed}"))
        })
        .unwrap_or_else(|| panic!("GATE BROKEN: no golden for seed {seed}"))
}

pub fn city_params(app: &App) -> &CityParams {
    &app.world().resource::<CityParamsRes>().0
}

pub fn place_player(app: &mut App, at: Vec3) {
    let entity = player(app);
    app.world_mut().get_mut::<Position>(entity).unwrap().0 = at;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .unwrap()
        .translation = at;
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
    let expected = app.world().resource::<PlayerSpawn>().0.y
        + app.world().resource::<LocomotionConfig>().float_height;
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

pub fn health(app: &mut App) -> Health {
    let entity = player(app);
    *app.world()
        .get::<Health>(entity)
        .expect("GATE BROKEN: player missing Health")
}

pub fn set_health(app: &mut App, update: impl FnOnce(&mut Health)) {
    let entity = player(app);
    update(
        app.world_mut()
            .get_mut::<Health>(entity)
            .expect("GATE BROKEN: player missing Health")
            .as_mut(),
    );
}

pub fn write_damage(app: &mut App, amount: f32) {
    app.world_mut().write_message(DebugDamage { amount });
}

pub fn game_state(app: &App) -> GameState {
    app.world().resource::<State<GameState>>().get().clone()
}

pub fn wasted_phase(app: &App) -> Option<WastedPhase> {
    app.world()
        .get_resource::<State<WastedPhase>>()
        .map(|s| s.get().clone())
}

pub fn count<F: QueryFilter>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), F>()
        .iter(app.world())
        .count()
}
