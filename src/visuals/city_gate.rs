//! Headless gate: the city is drawn as one merged mesh per render chunk, never per building.

use super::{
    RENDER_CONFIG, RenderConfig,
    city::{CityChunk, CityMeshTask, CityVisualsPlugin, PendingCitySpawn},
    facade::FacadeMaterial,
    props::CityProp,
};
use bevy::{
    asset::AssetPlugin, camera::visibility::VisibilityRange, ecs::query::QueryFilter, prelude::*,
    state::app::StatesPlugin, time::TimeUpdateStrategy,
};
use gta_sim::{
    compose_sim,
    config::{
        ConfigRoot, load_config,
        manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest},
    },
    flow::GameState,
    player::{DebugDamage, Player},
    world::{City, CityBuilding, CityParamsRes, WorldSource},
};
use std::{
    collections::{BTreeSet, HashSet},
    path::Path,
    time::{Duration, Instant},
};

fn assets_root() -> ConfigRoot {
    ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

fn render_config() -> RenderConfig {
    let config = load_config::<RenderConfig>(&assets_root(), RENDER_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    config
        .validate()
        .unwrap_or_else(|e| panic!("GATE BROKEN: {RENDER_CONFIG}: {e}"));
    config
}

/// Sim composition plus the production `CityVisualsPlugin`, updated until the city is spawned.
fn city_visuals_app(seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    // Stand-ins for the render plugins that normally register these asset types.
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<Image>()
    .init_asset::<FacadeMaterial>();
    compose_sim(&mut app, assets_root(), WorldSource::City { seed })
        .unwrap_or_else(|e| panic!("GATE BROKEN: compose_sim: {e}"));
    app.insert_resource(render_config())
        .add_plugins(CityVisualsPlugin);
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        app.update();
        if let Some(exit) = app.should_exit() {
            panic!("GATE BROKEN: app exited: {exit:?}");
        }
        let world = app.world();
        let done = *world.resource::<State<GameState>>().get() == GameState::Playing
            && !world.contains_resource::<CityMeshTask>()
            && !world.contains_resource::<PendingCitySpawn>()
            && world.contains_resource::<City>();
        if done {
            return app;
        }
        assert!(
            Instant::now() < deadline,
            "GATE BROKEN: city visuals not spawned in 120 s"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn count<F: QueryFilter>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), F>()
        .iter(app.world())
        .count()
}

fn quantize(p: [f32; 3]) -> [i64; 3] {
    p.map(|v| (v * 1000.0).round() as i64)
}

#[test]
fn city_meshes_are_merged_per_chunk() {
    let mut app = city_visuals_app(1);
    let render = render_config();
    let size = app.world().resource::<CityParamsRes>().0.size;
    // Worked example: size 1200 m, chunk 128 m -> ceil(9.375) = 10 -> 100 chunks.
    let n = (size / render.chunk_size).ceil() as u32;
    let expected = (n * n) as usize;

    assert_eq!(
        count::<(With<Mesh3d>, With<CityBuilding>)>(&mut app),
        0,
        "buildings carry their own Mesh3d"
    );
    let chunks = app
        .world_mut()
        .query_filtered::<(&CityChunk, &Mesh3d), With<Mesh3d>>()
        .iter(app.world())
        .map(|(chunk, mesh)| (chunk.coord, mesh.0.clone()))
        .collect::<Vec<_>>();
    assert_eq!(chunks.len(), expected, "chunk entities with Mesh3d");
    let coords = chunks
        .iter()
        .map(|c| (c.0.x, c.0.y))
        .collect::<BTreeSet<_>>();
    let all = (0..n)
        .flat_map(|x| (0..n).map(move |z| (x, z)))
        .collect::<BTreeSet<_>>();
    assert_eq!(coords, all, "chunk coords must be exactly [0, {n})²");
    assert_eq!(
        count::<(With<Mesh3d>, Without<CityProp>)>(&mut app),
        expected,
        "a mesh outside the chunk grid (ground, per-block surface or building) was spawned"
    );

    let meshes = app.world().resource::<Assets<Mesh>>();
    let mut vertices = std::collections::HashMap::new();
    for (coord, handle) in &chunks {
        let mesh = meshes
            .get(handle)
            .unwrap_or_else(|| panic!("chunk {coord} mesh not in Assets<Mesh>"));
        let indices = mesh.indices().map_or(0, |i| i.len());
        assert!(
            indices > 0 && indices.is_multiple_of(3),
            "chunk {coord}: {indices} indices"
        );
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|a| a.as_float3())
            .unwrap_or_else(|| panic!("chunk {coord}: no float3 positions"));
        vertices.insert(
            *coord,
            positions
                .iter()
                .map(|p| quantize(*p))
                .collect::<HashSet<_>>(),
        );
    }
    let layout = &app.world().resource::<City>().0;
    let chunk = render.chunk_size;
    let cell = |x: f32| (((x + size / 2.0) / chunk).floor() as i64).clamp(0, n as i64 - 1) as u32;
    for (i, b) in layout.buildings.iter().enumerate() {
        let c = UVec2::new(cell(b.center.x), cell(b.center.y));
        let h = b
            .upper_tiers
            .last()
            .map_or(b.half_extents, |t| t.half_extents);
        let corner = b.center + b.axis * h.x + b.axis.perp() * h.y;
        let roof = quantize([corner.x, b.height, corner.y]);
        assert!(
            vertices[&c].contains(&roof),
            "building {i}: roof corner {corner} at {} missing from chunk {c}",
            b.height
        );
    }

    let props = app
        .world_mut()
        .query_filtered::<Option<&VisibilityRange>, With<CityProp>>()
        .iter(app.world())
        .map(|range| range.cloned())
        .collect::<Vec<_>>();
    assert!(!props.is_empty(), "no props spawned");
    let range = render.props.visibility_range;
    let fade_start = range - render.props.fade;
    for prop in props {
        let prop = prop.expect("CityProp without VisibilityRange");
        assert_eq!(prop.end_margin, fade_start..range, "prop visibility range");
    }
}

#[test]
fn render_props_reference_manifest_files() {
    let manifest = load_config::<ThirdPartyManifest>(&assets_root(), THIRD_PARTY_MANIFEST)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    let config = render_config();
    for path in config.prop_asset_paths() {
        assert!(
            manifest.contains_asset(path),
            "{RENDER_CONFIG}: {path} is not listed in {THIRD_PARTY_MANIFEST}"
        );
    }
}

fn city_entities<F: QueryFilter>(app: &mut App) -> BTreeSet<Entity> {
    app.world_mut()
        .query_filtered::<Entity, F>()
        .iter(app.world())
        .collect()
}

#[test]
fn city_is_built_once_across_respawn() {
    let mut app = city_visuals_app(1);
    let chunks = city_entities::<(With<CityChunk>, With<Mesh3d>)>(&mut app);
    let props = city_entities::<With<CityProp>>(&mut app);
    assert!(
        !chunks.is_empty() && !props.is_empty(),
        "GATE BROKEN: no city"
    );
    let mut rebuilt = Vec::new();
    let mut step = |app: &mut App, label: &str| {
        app.update();
        let world = app.world();
        if world.contains_resource::<CityMeshTask>()
            || world.contains_resource::<PendingCitySpawn>()
        {
            rebuilt.push(label.to_string());
        }
    };
    let state = |app: &App| app.world().resource::<State<GameState>>().get().clone();

    app.world_mut()
        .write_message(DebugDamage { amount: 1000.0 });
    for k in 0.. {
        assert!(k < 3, "lethal damage did not enter Wasted");
        step(&mut app, "entering Wasted");
        if state(&app) == GameState::Wasted {
            break;
        }
    }
    for k in 0.. {
        assert!(k < 300, "Wasted did not end within 300 updates");
        step(&mut app, "Wasted");
        if state(&app) == GameState::Playing {
            break;
        }
    }
    for _ in 0..30 {
        step(&mut app, "Playing after respawn");
    }

    assert!(
        rebuilt.is_empty(),
        "the city mesh build ran again: {:?}",
        rebuilt.first()
    );
    assert_eq!(
        city_entities::<(With<CityChunk>, With<Mesh3d>)>(&mut app),
        chunks,
        "chunk entities changed across respawn"
    );
    assert_eq!(
        city_entities::<With<CityProp>>(&mut app),
        props,
        "prop entities changed across respawn"
    );
    assert_eq!(count::<With<Player>>(&mut app), 1);
}
