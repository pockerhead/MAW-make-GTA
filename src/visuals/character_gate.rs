//! Headless gate: the production character plugin puts the manifest-resolved glTF humanoid under the
//! player and drives its clips from `AnimState`. Loading the real GLB is covered by tools/qa/scenarios/t4.py.

use super::{
    CHARACTER_VISUAL_CONFIG, CharacterClips, CharacterVisualConfig,
    character::{
        CharacterAnimations, CharacterAnimator, CharacterModel, CharacterVisualsPlugin,
        model_transform, playback_rate,
    },
};
use avian3d::prelude::LinearVelocity;
use bevy::{
    asset::AssetPlugin, prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy,
    world_serialization::WorldAsset,
};
use gta_sim::{
    character::{AnimState, Gait, MoveIntent, move_direction},
    compose_sim,
    config::{
        ConfigRoot, load_config,
        manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest},
    },
    player::Player,
    world::WorldSource,
};
use std::path::Path;

fn assets_root() -> ConfigRoot {
    ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

fn visual_config() -> CharacterVisualConfig {
    load_config::<CharacterVisualConfig>(&assets_root(), CHARACTER_VISUAL_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
}

fn manifest() -> ThirdPartyManifest {
    load_config::<ThirdPartyManifest>(&assets_root(), THIRD_PARTY_MANIFEST)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
}

fn shipped_clips() -> CharacterClips {
    visual_config()
        .resolve(&manifest())
        .unwrap_or_else(|e| panic!("GATE BROKEN: {CHARACTER_VISUAL_CONFIG}: {e:?}"))
}

/// Sim composition (test area) plus the production `CharacterVisualsPlugin`, updated until the player exists.
fn character_visuals_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    // Stand-ins for the render, glTF and animation plugins that normally register these asset types.
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
        if find_player(&mut app).is_some() {
            return app;
        }
    }
    panic!("GATE BROKEN: no Player after 10 updates");
}

fn find_player(app: &mut App) -> Option<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .ok()
}

fn player(app: &mut App) -> Entity {
    find_player(app).expect("GATE BROKEN: expected exactly one Player")
}

fn anim_state(app: &App, player: Entity) -> AnimState {
    *app.world()
        .get::<AnimState>(player)
        .expect("GATE BROKEN: player missing AnimState")
}

#[test]
fn character_visuals_reference_manifest_rig() {
    let config = visual_config();
    config.validate().unwrap();
    // idle, walk, sprint (run), sprint, jump, fall in the glTF animation order of the pinned archive.
    assert_eq!(
        config.resolve(&manifest()).unwrap(),
        CharacterClips([1, 2, 3, 3, 4, 5])
    );
    let order = [
        AnimState::Idle,
        AnimState::Walk,
        AnimState::Run,
        AnimState::Sprint,
        AnimState::Jump,
        AnimState::Fall,
    ];
    for (index, state) in order.into_iter().enumerate() {
        assert_eq!(state as usize, index, "{state:?}");
    }
}

#[test]
fn visual_config_rejects_values_that_break_derived_numbers() {
    visual_config().validate().unwrap();
    type Break = fn(&mut CharacterVisualConfig);
    let cases: [(&str, Break, &str); 4] = [
        (
            "Duration overflow",
            |c| c.blend_seconds = 2e19,
            "blend_seconds",
        ),
        ("infinite scale", |c| c.model_height = 1e-40, "model_height"),
        (
            "zero scale",
            |c| {
                c.height = 1e-45;
                c.model_height = 1e30;
            },
            "model_height",
        ),
        (
            "infinite clip speed",
            |c| c.walk.native_speed = 3e38,
            "walk.native_speed",
        ),
    ];
    for (name, break_config, field) in cases {
        let mut config = visual_config();
        break_config(&mut config);
        let error = config.validate().expect_err(name);
        assert!(error.contains(field), "{name}: {error}");
    }
}

#[test]
fn model_faces_body_forward() {
    for yaw_deg in [0.0_f32, 90.0, 180.0] {
        let yaw = yaw_deg.to_radians();
        let face = Quat::from_rotation_y(yaw) * model_transform(1.05, 2.68).rotation * Vec3::Z;
        let forward = move_direction(Vec2::Y, yaw);
        assert!(
            face.abs_diff_eq(forward, 1e-5),
            "yaw {yaw_deg}: model faces {face}, body moves {forward}"
        );
    }
}

#[test]
fn model_feet_on_ground() {
    let transform = model_transform(1.05, 2.5);
    assert_eq!(transform.translation, Vec3::new(0.0, -1.05, 0.0));
    assert_eq!(transform.scale, Vec3::splat(2.5));
}

#[test]
fn playback_rate_worked_example() {
    let config = visual_config();
    // scale 1.8 / 0.67132 = 2.68128; walk 1.28 * 2.68128 = 3.4320 m/s, sprint 2.66 * 2.68128 = 7.1322 m/s.
    for (state, speed, expected) in [
        (AnimState::Walk, 1.8, 0.5245),
        (AnimState::Run, 4.5, 0.6309),
        (AnimState::Sprint, 6.8, 0.9534),
        (AnimState::Idle, 0.0, 1.0),
        (AnimState::Jump, 4.5, 1.0),
        (AnimState::Fall, 4.5, 1.0),
    ] {
        let rate = playback_rate(state, speed, &config);
        assert!(
            (rate - expected).abs() < 1e-3,
            "{state:?} at {speed}: rate {rate}, expected {expected}"
        );
    }
}

#[test]
fn character_model_spawns_under_player() {
    let mut app = character_visuals_app();
    let player = player(&mut app);
    let models = app
        .world_mut()
        .query_filtered::<(Entity, &ChildOf, &WorldAssetRoot, &Transform), With<CharacterModel>>()
        .iter(app.world())
        .map(|(entity, child_of, root, transform)| {
            (
                entity,
                child_of.parent(),
                root.0.path().map(ToString::to_string),
                *transform,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(models.len(), 1, "CharacterModel entities");
    let (_, parent, path, transform) = &models[0];
    assert_eq!(*parent, player);
    assert_eq!(
        path.as_deref(),
        Some("third_party/mini-characters/character-male-a.glb#Scene0")
    );
    assert_eq!(*transform, model_transform(1.05, 1.8 / 0.67132));
    let children = app
        .world()
        .get::<Children>(player)
        .map(|c| c.to_vec())
        .unwrap_or_default();
    assert!(
        children
            .iter()
            .all(|child| app.world().get::<Mesh3d>(*child).is_none()),
        "placeholder mesh still under the player"
    );
}

#[test]
fn graph_nodes_follow_manifest_clips() {
    let app = character_visuals_app();
    let animations = app.world().resource::<CharacterAnimations>();
    let graphs = app.world().resource::<Assets<AnimationGraph>>();
    let graph = graphs
        .get(&animations.graph)
        .expect("CharacterAnimations graph is not in Assets<AnimationGraph>");
    let clips = shipped_clips().0;
    for (node, clip) in animations.nodes.iter().zip(clips) {
        let Some(AnimationNodeType::Clip(handle)) = graph.get(*node).map(|n| &n.node_type) else {
            panic!("node {node:?} is not a clip node");
        };
        assert_eq!(
            handle.path().map(ToString::to_string),
            Some(format!(
                "third_party/mini-characters/character-male-a.glb#Animation{clip}"
            ))
        );
    }
}

/// Updates until the player shows `state` (at most 64 updates), then once more.
fn update_until(app: &mut App, player: Entity, state: AnimState) {
    for _ in 0..64 {
        if anim_state(app, player) == state {
            app.update();
            return;
        }
        app.update();
    }
    panic!(
        "GATE BROKEN: player never reached {state:?}, shows {:?}",
        anim_state(app, player)
    );
}

#[test]
fn animator_follows_anim_state() {
    let mut app = character_visuals_app();
    for _ in 0..64 {
        app.update();
    }
    let player = player(&mut app);
    let animator = app
        .world_mut()
        .spawn((
            AnimationPlayer::default(),
            AnimationTransitions::new(),
            CharacterAnimator {
                character: player,
                shown: AnimState::Idle,
            },
        ))
        .id();
    let nodes = app.world().resource::<CharacterAnimations>().nodes;
    let config = visual_config();
    for gait in [Gait::Run, Gait::Sprint] {
        let state = if gait == Gait::Run {
            AnimState::Run
        } else {
            AnimState::Sprint
        };
        *app.world_mut().get_mut::<MoveIntent>(player).unwrap() = MoveIntent {
            axis: Vec2::Y,
            gait,
            ..default()
        };
        update_until(&mut app, player, state);
        assert_eq!(
            anim_state(&app, player),
            state,
            "state changed on the extra update"
        );
        let velocity = app.world().get::<LinearVelocity>(player).unwrap().0;
        let horizontal = Vec2::new(velocity.x, velocity.z).length();
        let world = app.world();
        let transitions = world.get::<AnimationTransitions>(animator).unwrap();
        let animation_player = world.get::<AnimationPlayer>(animator).unwrap();
        let node = nodes[state as usize];
        assert_eq!(transitions.get_main_animation(), Some(node), "{state:?}");
        let speed = animation_player
            .animation(node)
            .unwrap_or_else(|| panic!("{state:?} node is not active"))
            .speed();
        let expected = playback_rate(state, horizontal, &config);
        assert!(
            (speed - expected).abs() < 1e-4,
            "{state:?}: clip speed {speed}, expected {expected} at {horizontal} m/s"
        );
    }
}
