//! Headless gate: the production character plugin puts the manifest-resolved glTF humanoid under the
//! player and drives its clips from `AnimState`. Loading the real GLB is covered by tools/qa/scenarios/t4.py.

use super::{
    CHARACTER_VISUAL_CONFIG, CharacterClips, CharacterVisualConfig,
    character::{
        ARMS_GROUP, ArmPose, BODY_GROUP, CharacterAnimations, CharacterAnimator, CharacterModel,
        CharacterVisualsPlugin, arm_pose, model_transform, playback_rate,
    },
};
use crate::juice::{HitStopPlugin, JUICE_CONFIG, JuiceConfig};
use avian3d::prelude::{LinearVelocity, Position};
use bevy::{
    asset::AssetPlugin, ecs::message::MessageCursor, prelude::*, state::app::StatesPlugin,
    time::TimeUpdateStrategy, world_serialization::WorldAsset,
};
use gta_sim::{
    character::{
        ActionIntent, AimIntent, AnimState, CharacterControlConfig, Gait, HealthConfig,
        LocomotionConfig, MoveIntent, move_direction,
    },
    combat::{HitReaction, Loadout, MeleeHit, ShotFired, Weapon, dummy_bundle},
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
    character_visuals_app_with(|_| {})
}

/// `character_visuals_app` with `extra` applied before `finish`/`cleanup`.
fn character_visuals_app_with(extra: impl FnOnce(&mut App)) -> App {
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
    extra(&mut app);
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
    // idle, walk, sprint (run), sprint, jump, fall; holding-right/-both and their -shoot clips,
    // in the glTF animation order of the pinned archive.
    assert_eq!(
        config.resolve(&manifest()).unwrap(),
        CharacterClips {
            locomotion: [1, 2, 3, 3, 4, 5],
            hold: [13, 15],
            shoot: [16, 18],
            melee: [19, 20, 21],
            bat: 19,
            death: 9,
            cower: 6,
            knockdown: 9,
            rest: 0,
        }
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
        .get(&animations.graphs[0])
        .expect("CharacterAnimations graph is not in Assets<AnimationGraph>");
    let clips = shipped_clips();
    let arms = 1 << ARMS_GROUP;
    let body = 1 << BODY_GROUP;
    let expected = animations
        .nodes
        .iter()
        .zip(clips.locomotion)
        .map(|(node, clip)| (*node, clip, 0))
        .chain(
            animations
                .legs
                .iter()
                .zip(clips.locomotion)
                .map(|(node, clip)| (*node, clip, arms)),
        )
        .chain(
            animations
                .hold
                .iter()
                .chain(&animations.shoot)
                .zip(clips.hold.into_iter().chain(clips.shoot))
                .map(|(node, clip)| (*node, clip, body)),
        )
        .chain(
            animations
                .fists
                .iter()
                .chain([&animations.bat, &animations.knockdown, &animations.rest])
                .zip(
                    clips
                        .melee
                        .into_iter()
                        .chain([clips.bat, clips.knockdown, clips.rest]),
                )
                .map(|(node, clip)| (*node, clip, 0)),
        );
    for (node, clip, mask) in expected {
        let Some(graph_node) = graph.get(node) else {
            panic!("node {node:?} is not in the graph");
        };
        let AnimationNodeType::Clip(handle) = &graph_node.node_type else {
            panic!("node {node:?} is not a clip node");
        };
        assert_eq!(
            handle.path().map(ToString::to_string),
            Some(format!(
                "third_party/mini-characters/character-male-a.glb#Animation{clip}"
            ))
        );
        assert_eq!(graph_node.mask, mask, "mask of node {node:?} (clip {clip})");
    }
}

#[test]
fn unknown_arm_joints_are_rejected() {
    let mut config = visual_config();
    config.hand_joint = "hand-right".into();
    config.arm_joints.push("elbow-left".into());
    let errors = config
        .resolve(&manifest())
        .expect_err("unknown joints accepted");
    for joint in ["hand-right", "elbow-left"] {
        assert!(
            errors.iter().any(|e| e.contains(joint)),
            "{joint} not reported: {errors:?}"
        );
    }
}

#[test]
fn arm_pose_per_weapon() {
    for (held, pose) in [
        (None, None),
        (Some(Weapon::Pistol), Some(ArmPose::OneHand)),
        (Some(Weapon::Smg), Some(ArmPose::TwoHands)),
        (Some(Weapon::Shotgun), Some(ArmPose::TwoHands)),
    ] {
        assert_eq!(arm_pose(held), pose, "{held:?}");
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
                armed: false,
                arms: None,
                action: None,
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

fn active_nodes(app: &App, animator: Entity) -> Vec<AnimationNodeIndex> {
    let mut nodes = app
        .world()
        .get::<AnimationPlayer>(animator)
        .unwrap()
        .playing_animations()
        .map(|(node, _)| *node)
        .collect::<Vec<_>>();
    nodes.sort();
    nodes
}

/// Armed: locomotion moves to the arm-less nodes, the arms play the weapon's hold clip, a shot its
/// shoot clip. (Returning from shoot to hold needs the clip to advance: runtime, t6 screenshots.)
#[test]
fn armed_animator_layers_arm_clips() {
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
            // Not the player's state, so the first update starts a locomotion clip.
            CharacterAnimator {
                character: player,
                shown: AnimState::Fall,
                armed: false,
                arms: None,
                action: None,
            },
        ))
        .id();
    app.update();
    let (nodes, legs, hold, shoot) = {
        let a = app.world().resource::<CharacterAnimations>();
        (a.nodes, a.legs, a.hold, a.shoot)
    };
    let idle = AnimState::Idle as usize;
    let main = |app: &App| {
        app.world()
            .get::<AnimationTransitions>(animator)
            .unwrap()
            .get_main_animation()
    };
    assert_eq!(anim_state(&app, player), AnimState::Idle);
    assert_eq!(
        main(&app),
        Some(nodes[idle]),
        "unarmed: full-body locomotion"
    );
    assert!(
        !active_nodes(&app, animator)
            .iter()
            .any(|n| hold.contains(n) || shoot.contains(n)),
        "unarmed: no arm clip"
    );
    for (weapon, pose) in [(Weapon::Pistol, 0), (Weapon::Smg, 1)] {
        app.world_mut().get_mut::<Loadout>(player).unwrap().held = Some(weapon);
        app.update();
        assert_eq!(
            main(&app),
            Some(legs[idle]),
            "{weapon:?}: arm-less locomotion"
        );
        assert!(
            active_nodes(&app, animator).contains(&hold[pose]),
            "{weapon:?}: hold clip not playing"
        );
        app.world_mut().write_message(ShotFired {
            shooter: player,
            weapon,
            muzzle: Vec3::ZERO,
            attack: 0,
        });
        app.update();
        let active = active_nodes(&app, animator);
        assert!(
            active.contains(&shoot[pose]),
            "{weapon:?}: shoot clip not playing"
        );
        assert!(
            !active.contains(&hold[pose]),
            "{weapon:?}: hold still playing"
        );
    }
    app.world_mut().get_mut::<Loadout>(player).unwrap().held = None;
    app.update();
    assert_eq!(main(&app), Some(nodes[idle]), "unarmed again");
    assert!(
        !active_nodes(&app, animator)
            .iter()
            .any(|n| hold.contains(n) || shoot.contains(n)),
        "unarmed again: arm clip still playing"
    );
}

/// Attacker's feet in the test area (open floor).
const A: Vec3 = Vec3::new(-20.0, 0.0, 21.0);

fn juice() -> JuiceConfig {
    let cfg = load_config::<JuiceConfig>(&assets_root(), JUICE_CONFIG)
        .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"));
    cfg.validate()
        .unwrap_or_else(|e| panic!("GATE BROKEN: {JUICE_CONFIG}: {e}"));
    cfg
}

/// The visuals app plus the production hit-stop plugin, as `JuicePlugin` adds it.
fn hit_stop_app() -> App {
    character_visuals_app_with(|app| {
        app.insert_resource(juice()).add_plugins(HitStopPlugin);
    })
}

fn spawn_dummy(app: &mut App, feet: Vec3) -> Entity {
    let world = app.world();
    let loco = world.resource::<LocomotionConfig>().clone();
    let health = world.resource::<HealthConfig>().clone();
    let handle = world.resource::<CharacterControlConfig>().0.clone();
    app.world_mut()
        .spawn(dummy_bundle(&loco, handle, &health, feet))
        .id()
}

/// A hand-built animator of `character` with the idle node already playing.
fn spawn_animator(app: &mut App, character: Entity) -> Entity {
    let idle = app.world().resource::<CharacterAnimations>().nodes[AnimState::Idle as usize];
    let mut player = AnimationPlayer::default();
    let mut transitions = AnimationTransitions::new();
    transitions
        .play(&mut player, idle, std::time::Duration::ZERO)
        .repeat();
    app.world_mut()
        .spawn((
            player,
            transitions,
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

/// Player at `A`, a target dummy at `A - Z`, a bystander 4 m away; one animator each.
fn melee_scene(app: &mut App) -> [(Entity, Entity); 3] {
    let player = player(app);
    let at = A + Vec3::Y * app.world().resource::<LocomotionConfig>().float_height;
    app.world_mut().get_mut::<Position>(player).unwrap().0 = at;
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = at;
    let target = spawn_dummy(app, A + Vec3::NEG_Z);
    let bystander = spawn_dummy(app, A + Vec3::X * 4.0);
    for _ in 0..8 {
        app.update();
    }
    let origin = app.world().get::<Position>(player).unwrap().0;
    let mut aim = app.world_mut().get_mut::<AimIntent>(player).unwrap();
    aim.origin = origin;
    aim.direction = (A + Vec3::NEG_Z + Vec3::Y - origin).normalize();
    [player, target, bystander].map(|character| (character, spawn_animator(app, character)))
}

fn click(app: &mut App, player: Entity) {
    app.world_mut()
        .get_mut::<ActionIntent>(player)
        .unwrap()
        .fire_requested = true;
}

/// (active animations, how many of them are paused).
fn paused(app: &App, animator: Entity) -> (usize, usize) {
    let player = app.world().get::<AnimationPlayer>(animator).unwrap();
    let all = player.playing_animations().count();
    let paused = player
        .playing_animations()
        .filter(|(_, a)| a.is_paused())
        .count();
    (all, paused)
}

type Frame = (bool, (usize, usize), (usize, usize), (usize, usize));

/// Hit-stop freezes the animations of the attacker and the target for `hit_stop_seconds` of real
/// time and nothing else: `Time<Virtual>` and the fixed tick run on unchanged.
#[test]
fn hit_stop_freezes_only_the_pair_and_not_time() {
    let mut app = hit_stop_app();
    let [(player, attacker), (_, target), (_, bystander)] = melee_scene(&mut app);
    let mut hits: MessageCursor<MeleeHit> = app
        .world()
        .resource::<Messages<MeleeHit>>()
        .get_cursor_current();
    let step = app.world().resource::<Time<Fixed>>().timestep();
    click(&mut app, player);
    // Per update: (MeleeHit seen, attacker, target, bystander) as (active, paused).
    let mut log: Vec<Frame> = Vec::new();
    for _ in 0..40 {
        let fixed = app.world().resource::<Time<Fixed>>().elapsed();
        app.update();
        let world = app.world();
        let virt = world.resource::<Time<Virtual>>();
        assert_eq!(virt.relative_speed(), 1.0, "hit-stop scaled Time<Virtual>");
        assert!(!virt.is_paused(), "hit-stop paused Time<Virtual>");
        assert_eq!(
            world.resource::<Time<Fixed>>().elapsed() - fixed,
            step,
            "the fixed tick did not advance by exactly one step"
        );
        let hit = hits.read(world.resource::<Messages<MeleeHit>>()).count() > 0;
        log.push((
            hit,
            paused(&app, attacker),
            paused(&app, target),
            paused(&app, bystander),
        ));
        if log.len() > 5 && log[log.len() - 6].0 {
            break;
        }
    }
    let u = log
        .iter()
        .position(|(hit, ..)| *hit)
        .expect("GATE BROKEN: the punch never landed");
    assert!(u >= 1 && log.len() >= u + 5, "GATE BROKEN: {log:?}");
    for (k, (_, attacker, target, bystander)) in log.iter().enumerate().skip(u - 1).take(6) {
        let frozen = (u..u + 4).contains(&k);
        for (name, (all, paused)) in [("attacker", attacker), ("target", target)] {
            assert!(*all > 0, "GATE BROKEN: {name} has no active animation");
            let expected = if frozen { *all } else { 0 };
            assert_eq!(
                *paused,
                expected,
                "{name} at U{:+}: {log:?}",
                k as i64 - u as i64
            );
        }
        assert!(
            bystander.0 > 0,
            "GATE BROKEN: bystander has no active animation"
        );
        assert_eq!(bystander.1, 0, "the bystander froze: {log:?}");
    }
}

/// Liveness: a swing and a knockdown replace locomotion with their full-body clips, and
/// locomotion comes back when they end.
#[test]
fn melee_actions_drive_full_body_clips() {
    let mut app = hit_stop_app();
    let [(player, attacker), _, (bystander, watcher)] = melee_scene(&mut app);
    let (nodes, fists, knockdown) = {
        let a = app.world().resource::<CharacterAnimations>();
        (a.nodes, a.fists, a.knockdown)
    };
    let main = |app: &App, animator: Entity| {
        app.world()
            .get::<AnimationTransitions>(animator)
            .unwrap()
            .get_main_animation()
    };
    click(&mut app, player);
    app.update();
    assert_eq!(main(&app, attacker), Some(fists[0]), "swing clip");
    *app.world_mut().get_mut::<HitReaction>(bystander).unwrap() =
        HitReaction::KnockedDown { left: 1.0 };
    app.update();
    assert_eq!(main(&app, watcher), Some(knockdown), "knockdown clip");
    *app.world_mut().get_mut::<HitReaction>(bystander).unwrap() = HitReaction::Steady;
    app.update();
    assert_eq!(
        main(&app, watcher),
        Some(nodes[AnimState::Idle as usize]),
        "locomotion after the knockdown"
    );
}
