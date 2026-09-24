//! Headless gates of the civilian looks: every civilian model animates from its own glTF clips (real
//! GLBs), per-model graphs are built from their own model, and dead/cowering characters select their
//! full-body clips. The GLB harness here is shared with `gang_gate`.

use super::{
    CHARACTER_VISUAL_CONFIG, CharacterClips, CharacterVisualConfig,
    character::{
        CharacterAnimations, CharacterAnimator, CharacterModel, CharacterVisualsPlugin, ModelKey,
        ShownAction,
    },
};
use bevy::{
    animation::{AnimationPlugin, graph::AnimationNodeType},
    asset::AssetPlugin,
    gltf::GltfPlugin,
    image::ImagePlugin,
    mesh::MeshPlugin,
    prelude::*,
    state::app::StatesPlugin,
    time::TimeUpdateStrategy,
    world_serialization::{WorldAsset, WorldSerializationPlugin},
};
use gta_sim::{
    character::{AnimState, CharacterControlConfig, Dead, HealthConfig, LocomotionConfig},
    civilian::{Civilian, CivilianConfig, CivilianState, Temperament, civilian_bundle},
    compose_sim,
    config::{
        ConfigRoot, load_config,
        manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest},
    },
    navigation::{GraphWalker, SidewalkGraph},
    player::Player,
    population::Appearance,
    world::WorldSource,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn assets_root() -> ConfigRoot {
    ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

pub(super) fn visual_config() -> CharacterVisualConfig {
    load_config::<CharacterVisualConfig>(&assets_root(), CHARACTER_VISUAL_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
}

pub(super) fn manifest() -> ThirdPartyManifest {
    load_config::<ThirdPartyManifest>(&assets_root(), THIRD_PARTY_MANIFEST)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
}

fn shipped_clips() -> CharacterClips {
    visual_config()
        .resolve(&manifest())
        .unwrap_or_else(|e| panic!("GATE BROKEN: {CHARACTER_VISUAL_CONFIG}: {e:?}"))
}

/// Model paths by `ModelKey`.
pub(super) fn models() -> Vec<String> {
    let config = visual_config();
    std::iter::once(config.model.clone())
        .chain(config.civilian_models.clone())
        .chain(config.gang_models.clone())
        .chain(config.police_models.clone())
        .collect()
}

fn require_glbs() {
    for model in models() {
        let path = assets_root().path(&model);
        assert!(
            path.is_file(),
            "GATE BROKEN: missing third-party asset {}; run python tools/fetch_assets.py",
            path.display()
        );
    }
}

/// A 40 m straight sidewalk on the test floor.
fn line_graph() -> SidewalkGraph {
    SidewalkGraph::new(
        vec![Vec3::new(-20.0, 0.0, -30.0), Vec3::new(20.0, 0.0, -30.0)],
        &[(0, 1)],
    )
    .unwrap()
}

fn calm() -> Temperament {
    Temperament {
        flee: 1.0,
        cower: 1.0,
        report: 1.0,
    }
}

fn spawn_civilian(app: &mut App, graph: &SidewalkGraph, t: f32, appearance: u32) -> Entity {
    let world = app.world();
    let loco = world.resource::<LocomotionConfig>().clone();
    let health = world.resource::<HealthConfig>().clone();
    let handle = world.resource::<CharacterControlConfig>().0.clone();
    let bundle = civilian_bundle(
        &loco,
        handle,
        &health,
        graph,
        GraphWalker { from: 0, to: 1 },
        t,
        calm(),
        Appearance(appearance),
    );
    app.world_mut().spawn(bundle).id()
}

/// The `CharacterModel` above `entity`, if any.
pub(super) fn model_of(app: &App, mut entity: Entity) -> Option<Entity> {
    loop {
        if app.world().get::<CharacterModel>(entity).is_some() {
            return Some(entity);
        }
        entity = app.world().get::<ChildOf>(entity)?.parent();
    }
}

/// `leg-left` rotation per model key.
fn leg_rotations(app: &mut App) -> Vec<(usize, Quat)> {
    let legs = app
        .world_mut()
        .query::<(Entity, &Name, &Transform)>()
        .iter(app.world())
        .filter(|(_, name, _)| name.as_str() == "leg-left")
        .map(|(e, _, t)| (e, t.rotation))
        .collect::<Vec<_>>();
    legs.into_iter()
        .filter_map(|(e, rotation)| {
            let model = model_of(app, e)?;
            Some((app.world().get::<ModelKey>(model)?.0, rotation))
        })
        .collect()
}

/// Sim composition (test area) plus the production `CharacterVisualsPlugin` loading the real GLBs;
/// the caller finishes it.
pub(super) fn glb_app() -> App {
    require_glbs();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin {
            file_path: assets_root().0.to_string_lossy().into_owned(),
            ..default()
        },
        StatesPlugin,
        ImagePlugin::default(),
        MeshPlugin,
        AnimationPlugin,
        WorldSerializationPlugin,
        GltfPlugin::default(),
    ))
    .init_asset::<StandardMaterial>()
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    compose_sim(&mut app, assets_root(), WorldSource::TestArea)
        .unwrap_or_else(|e| panic!("GATE BROKEN: compose_sim: {e}"));
    app.insert_resource(visual_config())
        .insert_resource(shipped_clips())
        .add_plugins(CharacterVisualsPlugin);
    app
}

/// Updates until `count` character models exist and each is wired to an animator.
pub(super) fn wait_wired(app: &mut App, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        app.update();
        let models = app
            .world_mut()
            .query_filtered::<(), With<CharacterModel>>()
            .iter(app.world())
            .count();
        let wired = app
            .world_mut()
            .query::<&CharacterAnimator>()
            .iter(app.world())
            .count();
        if models == count && wired == models {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "GATE BROKEN: {wired} of {models} models wired after 30 s"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Model key and graph handle of the animator following each of `characters`.
pub(super) fn animator_graphs(
    app: &mut App,
    characters: &[Entity],
) -> Vec<(usize, Handle<AnimationGraph>)> {
    let animators = app
        .world_mut()
        .query::<(&CharacterAnimator, &ModelKey, &AnimationGraphHandle)>()
        .iter(app.world())
        .map(|(a, k, g)| (a.character, k.0, g.0.clone()))
        .collect::<Vec<_>>();
    characters
        .iter()
        .map(|&c| {
            let (_, key, graph) = animators
                .iter()
                .find(|(owner, _, _)| *owner == c)
                .unwrap_or_else(|| panic!("{c} has no animator"));
            (*key, graph.clone())
        })
        .collect()
}

/// Largest `leg-left` turn from its first pose over 32 updates, per model key: two samples of a
/// swinging leg can land on the same angle, and the clip phase depends on asset load timing.
pub(super) fn leg_turns(app: &mut App, keys: usize) -> (Vec<(usize, Quat)>, Vec<f32>) {
    let before = leg_rotations(app);
    let mut turns = vec![0.0_f32; keys];
    for _ in 0..32 {
        app.update();
        for (key, rotation) in leg_rotations(app) {
            let Some(&(_, first)) = before.iter().find(|(k, _)| *k == key) else {
                continue;
            };
            turns[key] = turns[key].max(first.angle_between(rotation));
        }
    }
    (before, turns)
}

#[test]
fn every_civilian_model_animates_from_its_own_clips() {
    let mut app = glb_app();
    app.finish();
    app.cleanup();
    // A stop would show the idle clip during the sample window.
    app.world_mut().resource_mut::<CivilianConfig>().idle_chance = 0.0;
    let graph = line_graph();
    let n = visual_config().civilian_models.len();
    let civilians = (0..n)
        .map(|k| spawn_civilian(&mut app, &graph, (k as f32 + 0.5) / n as f32, k as u32))
        .collect::<Vec<_>>();
    app.world_mut().insert_resource(graph);
    wait_wired(&mut app, n + 1);
    let graphs = app.world().resource::<CharacterAnimations>().graphs.clone();
    for (k, (key, graph)) in animator_graphs(&mut app, &civilians)
        .into_iter()
        .enumerate()
    {
        assert_eq!(key, 1 + k % n, "civilian {k} got the wrong model");
        assert_eq!(
            graph, graphs[key],
            "civilian {k} animates with another model's graph"
        );
    }
    let (before, turns) = leg_turns(&mut app, n + 1);
    for (key, &turned) in turns.iter().enumerate().skip(1) {
        assert!(
            before.iter().any(|(k, _)| *k == key),
            "model key {key} has no leg-left joint"
        );
        assert!(
            turned > 0.1,
            "model {} ({key}): leg-left turned {turned} rad while walking (T-pose)",
            models()[key]
        );
    }
}

/// Sim composition (test area) plus the production `CharacterVisualsPlugin` with stand-in asset types.
fn stand_in_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<Image>()
    .init_asset::<WorldAsset>()
    .init_asset::<AnimationClip>()
    .init_asset::<AnimationGraph>();
    compose_sim(&mut app, assets_root(), WorldSource::TestArea)
        .unwrap_or_else(|e| panic!("GATE BROKEN: compose_sim: {e}"));
    app.insert_resource(visual_config())
        .insert_resource(shipped_clips())
        .add_plugins(CharacterVisualsPlugin);
    app.finish();
    app.cleanup();
    for _ in 0..10 {
        app.update();
    }
    app
}

#[test]
fn graph_clips_come_from_their_own_model() {
    let app = stand_in_app();
    let animations = app.world().resource::<CharacterAnimations>();
    let graphs = app.world().resource::<Assets<AnimationGraph>>();
    let models = models();
    assert_eq!(animations.graphs.len(), models.len());
    assert_eq!(animations.scenes.len(), models.len());
    let clip_paths = |key: usize| {
        let graph = graphs
            .get(&animations.graphs[key])
            .expect("graph not in assets");
        graph
            .nodes()
            .filter_map(|node| match &graph.get(node)?.node_type {
                AnimationNodeType::Clip(handle) => Some((node, handle.path()?.to_string())),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let reference = clip_paths(0);
    assert!(
        !reference.is_empty(),
        "GATE BROKEN: player graph has no clips"
    );
    for (key, model) in models.iter().enumerate() {
        let paths = clip_paths(key);
        assert_eq!(paths.len(), reference.len(), "{model}: node count differs");
        for ((node, path), (ref_node, ref_path)) in paths.iter().zip(&reference) {
            assert_eq!(node, ref_node, "{model}: node order differs");
            let (source, label) = path.split_once('#').expect("clip path without a label");
            assert_eq!(
                source, model,
                "{model}: node {node:?} plays a clip of {source}"
            );
            assert_eq!(Some(label), ref_path.split_once('#').map(|(_, l)| l));
        }
        let scene = animations.scenes[key]
            .path()
            .expect("scene handle without a path");
        assert_eq!(scene.to_string(), format!("{model}#Scene0"));
    }
}

fn spawn_animator(app: &mut App, character: Entity) -> Entity {
    app.world_mut()
        .spawn((
            AnimationPlayer::default(),
            AnimationTransitions::new(),
            CharacterAnimator {
                character,
                shown: AnimState::Idle,
                armed: false,
                arms: None,
                action: None,
            },
        ))
        .id()
}

fn main_node(app: &App, animator: Entity) -> Option<AnimationNodeIndex> {
    app.world()
        .get::<AnimationTransitions>(animator)
        .unwrap()
        .get_main_animation()
}

fn active(app: &App, animator: Entity, node: AnimationNodeIndex) -> bool {
    app.world()
        .get::<AnimationPlayer>(animator)
        .unwrap()
        .animation(node)
        .is_some()
}

#[test]
fn dead_and_cower_select_their_nodes() {
    let mut app = stand_in_app();
    let (death, cower, idle) = {
        let a = app.world().resource::<CharacterAnimations>();
        (a.death, a.cower, a.nodes[AnimState::Idle as usize])
    };
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .expect("GATE BROKEN: no player");
    let dying = spawn_animator(&mut app, player);
    app.update();
    app.world_mut().entity_mut(player).insert(Dead);
    for _ in 0..8 {
        app.update();
        assert_eq!(
            main_node(&app, dying),
            Some(death),
            "dead body left the death clip"
        );
        assert!(active(&app, dying, death));
    }
    assert_eq!(
        app.world().get::<CharacterAnimator>(dying).unwrap().action,
        Some(ShownAction::Death)
    );
    app.world_mut().entity_mut(player).remove::<Dead>();
    app.update();
    assert_eq!(
        main_node(&app, dying),
        Some(idle),
        "revived body not back to locomotion"
    );

    let civilian = spawn_civilian(&mut app, &line_graph(), 0.5, 0);
    let crouching = spawn_animator(&mut app, civilian);
    app.update();
    app.world_mut().get_mut::<Civilian>(civilian).unwrap().state = CivilianState::Cower {
        from: Vec3::ZERO,
        left: 100.0,
        about: None,
    };
    app.update();
    assert_eq!(main_node(&app, crouching), Some(cower));
    assert!(active(&app, crouching, cower));
    app.world_mut().get_mut::<Civilian>(civilian).unwrap().state = CivilianState::Wander;
    app.update();
    let main = main_node(&app, crouching);
    let locomotion = app.world().resource::<CharacterAnimations>().nodes;
    assert!(
        main.is_some_and(|m| locomotion.contains(&m)),
        "calm civilian not back to locomotion: {main:?}"
    );
}

#[test]
fn civilian_models_are_validated() {
    let manifest = manifest();
    let mut config = visual_config();
    config
        .civilian_models
        .push("third_party/inter/Inter-Regular.ttf".into());
    let errors = config.resolve(&manifest).unwrap_err();
    assert!(
        errors.iter().any(|e| e.contains("Inter-Regular.ttf")),
        "{errors:?}"
    );
    let mut config = visual_config();
    config.civilian_models.clear();
    let error = config.validate().unwrap_err();
    assert!(error.contains("civilian_models"), "{error}");
    let mut config = visual_config();
    config.civilian_tints.clear();
    let error = config.validate().unwrap_err();
    assert!(error.contains("civilian_tints"), "{error}");
}
