//! Kenney glTF humanoid on every character body, animated from `AnimState`.

use super::character_config::{CharacterClips, CharacterVisualConfig};
use crate::juice::{HitStop, HitStopSystems};
use avian3d::prelude::LinearVelocity;
use bevy::{
    animation::AnimationTargetId,
    gltf::GltfMeshName,
    prelude::*,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
use gta_sim::{
    character::{AnimState, CharacterBody, Dead},
    civilian::{Civilian, CivilianState},
    combat::{HitReaction, Loadout, Melee, MeleeWeapon, ShotFired, Swing, Weapon},
    gang::{GangConfig, GangMember},
    population::Appearance,
};
use std::time::Duration;

/// glTF faces +Z, gameplay bodies face -Z.
const MODEL_YAW: f32 = std::f32::consts::PI;
/// Animation mask groups: the arm joints, and every other animated node of the model.
pub(super) const ARMS_GROUP: u32 = 0;
pub(super) const BODY_GROUP: u32 = 1;

/// Arm clip set of a held gun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArmPose {
    OneHand,
    TwoHands,
}

/// Arm pose for the held weapon; `None` leaves the arms to locomotion.
pub(super) fn arm_pose(held: Option<Weapon>) -> Option<ArmPose> {
    match held? {
        Weapon::Pistol => Some(ArmPose::OneHand),
        Weapon::Smg | Weapon::Shotgun => Some(ArmPose::TwoHands),
    }
}

pub struct CharacterVisualsPlugin;

impl Plugin for CharacterVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CharacterModel>()
            .init_resource::<CharacterAnimations>()
            .add_observer(spawn_character_model)
            .add_systems(
                Update,
                (
                    drive_character_animation,
                    apply_hit_stop
                        .after(drive_character_animation)
                        .after(HitStopSystems),
                ),
            );
    }
}

/// One animation graph per character model, built from that model's own clips (a clip drives only
/// the model whose glTF root name it carries). Every graph has the same node indices; node `i`
/// plays the clip of `AnimState` `i`.
#[derive(Resource)]
pub(super) struct CharacterAnimations {
    /// Indexed by `ModelKey`: 0 = `model` (player, dummies), 1..=C = `civilian_models`, C+1.. =
    /// `gang_models`.
    pub(super) graphs: Vec<Handle<AnimationGraph>>,
    /// Scene of each model, by `ModelKey`; loaded up front so civilians never wait on asset IO.
    pub(super) scenes: Vec<Handle<WorldAsset>>,
    /// Locomotion on every joint (unarmed).
    pub(super) nodes: [AnimationNodeIndex; 6],
    /// The same locomotion without the arms (armed: the arm layer owns them).
    pub(super) legs: [AnimationNodeIndex; 6],
    /// Arm-only clips per `ArmPose`.
    pub(super) hold: [AnimationNodeIndex; 2],
    pub(super) shoot: [AnimationNodeIndex; 2],
    /// Full-body melee clips: fist combo steps, bat swing, knockdown.
    pub(super) fists: [AnimationNodeIndex; 3],
    pub(super) bat: AnimationNodeIndex,
    pub(super) knockdown: AnimationNodeIndex,
    /// Death (played once) and cower (looped) clips.
    pub(super) death: AnimationNodeIndex,
    pub(super) cower: AnimationNodeIndex,
    /// The death clip is the knockdown clip: a knocked-down body that dies keeps lying, no re-fall.
    pub(super) death_is_knockdown: bool,
    /// Rest pose under every other clip, at the configured weight.
    pub(super) rest: AnimationNodeIndex,
    /// Clip assets of the swings, for their durations.
    pub(super) fist_clips: [Handle<AnimationClip>; 3],
    pub(super) bat_clip: Handle<AnimationClip>,
}

/// Node indices of one per-model graph; equal for every model (same build order).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct GraphNodes {
    nodes: [AnimationNodeIndex; 6],
    legs: [AnimationNodeIndex; 6],
    hold: [AnimationNodeIndex; 2],
    shoot: [AnimationNodeIndex; 2],
    fists: [AnimationNodeIndex; 3],
    bat: AnimationNodeIndex,
    knockdown: AnimationNodeIndex,
    death: AnimationNodeIndex,
    cower: AnimationNodeIndex,
    rest: AnimationNodeIndex,
}

/// The character graph of `model`, every clip loaded from `model` itself.
fn build_graph(
    asset_server: &AssetServer,
    model: &str,
    clips: &CharacterClips,
    rest_weight: f32,
) -> (AnimationGraph, GraphNodes) {
    let clip = |index: usize| {
        asset_server.load(GltfAssetLabel::Animation(index).from_asset(model.to_owned()))
    };
    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let nodes = clips.locomotion.map(|i| graph.add_clip(clip(i), 1.0, root));
    let legs = clips
        .locomotion
        .map(|i| graph.add_clip_with_mask(clip(i), 1 << ARMS_GROUP, 1.0, root));
    let hold = clips
        .hold
        .map(|i| graph.add_clip_with_mask(clip(i), 1 << BODY_GROUP, 1.0, root));
    let shoot = clips
        .shoot
        .map(|i| graph.add_clip_with_mask(clip(i), 1 << BODY_GROUP, 1.0, root));
    let fists = clips.melee.map(|i| graph.add_clip(clip(i), 1.0, root));
    let bat = graph.add_clip(clip(clips.bat), 1.0, root);
    let knockdown = graph.add_clip(clip(clips.knockdown), 1.0, root);
    let rest = graph.add_clip(clip(clips.rest), rest_weight, root);
    let death = graph.add_clip(clip(clips.death), 1.0, root);
    let cower = graph.add_clip(clip(clips.cower), 1.0, root);
    let nodes = GraphNodes {
        nodes,
        legs,
        hold,
        shoot,
        fists,
        bat,
        knockdown,
        death,
        cower,
        rest,
    };
    (graph, nodes)
}

impl FromWorld for CharacterAnimations {
    fn from_world(world: &mut World) -> Self {
        let config = world.resource::<CharacterVisualConfig>().clone();
        let clips = *world.resource::<CharacterClips>();
        let asset_server = world.resource::<AssetServer>().clone();
        let models = std::iter::once(&config.model)
            .chain(&config.civilian_models)
            .chain(&config.gang_models)
            .collect::<Vec<_>>();
        let mut graphs = Vec::with_capacity(models.len());
        let mut scenes = Vec::with_capacity(models.len());
        let mut first: Option<GraphNodes> = None;
        for model in models {
            let (graph, nodes) = build_graph(&asset_server, model, &clips, config.rest.weight);
            debug_assert!(first.is_none_or(|first| first == nodes));
            first.get_or_insert(nodes);
            graphs.push(world.resource_mut::<Assets<AnimationGraph>>().add(graph));
            scenes.push(asset_server.load(GltfAssetLabel::Scene(0).from_asset(model.clone())));
        }
        let n = first.expect("the player model is always first");
        let clip = |index: usize| {
            asset_server.load(GltfAssetLabel::Animation(index).from_asset(config.model.clone()))
        };
        Self {
            graphs,
            scenes,
            nodes: n.nodes,
            legs: n.legs,
            hold: n.hold,
            shoot: n.shoot,
            fists: n.fists,
            bat: n.bat,
            knockdown: n.knockdown,
            death: n.death,
            cower: n.cower,
            death_is_knockdown: clips.death == clips.knockdown,
            rest: n.rest,
            fist_clips: clips.melee.map(clip),
            bat_clip: clip(clips.bat),
        }
    }
}

/// Root of the glTF model instance under a character body.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct CharacterModel;

/// Which model (index into `CharacterAnimations::graphs`) a `CharacterModel` instance and its
/// animator use.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ModelKey(pub(super) usize);

/// On the model's `AnimationPlayer`: which character it follows and which state it shows.
#[derive(Component)]
pub(super) struct CharacterAnimator {
    pub(super) character: Entity,
    pub(super) shown: AnimState,
    /// Whether `shown` plays on the `legs` nodes (a gun is held).
    pub(super) armed: bool,
    /// The arm-layer node playing, if any.
    pub(super) arms: Option<AnimationNodeIndex>,
    /// The full-body melee clip shown instead of locomotion, if any.
    pub(super) action: Option<ShownAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShownAction {
    /// A swing, by its attack id.
    Swing(u32),
    Knockdown,
    Death,
    Cower,
}

/// Model feet (y = 0) on the ground: the body centre floats `float_height` above it.
pub(super) fn model_transform(float_height: f32, scale: f32) -> Transform {
    Transform::from_xyz(0.0, -float_height, 0.0)
        .with_rotation(Quat::from_rotation_y(MODEL_YAW))
        .with_scale(Vec3::splat(scale))
}

/// Model key and tint of a body: a civilian's `Appearance` picks a civilian model and tint, a gang
/// member's a gang model under its gang's tint; everyone else is model 0 under `tint`.
pub(super) fn body_look(
    appearance: Option<Appearance>,
    civilian: bool,
    gang: Option<u8>,
    config: &CharacterVisualConfig,
    gangs: &GangConfig,
) -> (usize, (f32, f32, f32)) {
    let Some(a) = appearance.map(|a| a.0 as usize) else {
        return (0, config.tint);
    };
    let c = config.civilian_models.len();
    if civilian {
        let tint = config.civilian_tints[(a / c) % config.civilian_tints.len()];
        return (1 + a % c, tint);
    }
    let Some(spec) = gang.and_then(|g| gangs.gangs.get(g as usize)) else {
        return (0, config.tint);
    };
    (1 + c + a % config.gang_models.len(), spec.tint)
}

/// Look inputs of a body: appearance, civilian, gang.
type LookQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Appearance>,
        Has<Civilian>,
        Option<&'static GangMember>,
    ),
>;

fn look_of(
    looks: &LookQuery,
    entity: Entity,
    config: &CharacterVisualConfig,
    gangs: &GangConfig,
) -> (usize, (f32, f32, f32)) {
    let Ok((appearance, civilian, member)) = looks.get(entity) else {
        return (0, config.tint);
    };
    body_look(
        appearance.copied(),
        civilian,
        member.map(|m| m.gang),
        config,
        gangs,
    )
}

fn spawn_character_model(
    event: On<Add, CharacterBody>,
    bodies: Query<&CharacterBody>,
    looks: LookQuery,
    config: Res<CharacterVisualConfig>,
    gangs: Res<GangConfig>,
    animations: Res<CharacterAnimations>,
    mut commands: Commands,
) {
    let Ok(body) = bodies.get(event.entity) else {
        return;
    };
    let (key, _) = look_of(&looks, event.entity, &config, &gangs);
    let scene = animations.scenes[key].clone();
    let transform = model_transform(body.float_height, config.scale());
    commands
        .entity(event.entity)
        .insert(Visibility::default())
        .with_children(|parent| {
            parent
                .spawn((
                    CharacterModel,
                    ModelKey(key),
                    WorldAssetRoot(scene),
                    transform,
                ))
                .observe(on_model_ready);
        });
}

#[allow(clippy::too_many_arguments)]
fn on_model_ready(
    ready: On<WorldInstanceReady>,
    models: Query<(&ChildOf, &ModelKey), With<CharacterModel>>,
    looks: LookQuery,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    meshes: Query<(&GltfMeshName, &MeshMaterial3d<StandardMaterial>)>,
    targets: Query<(&Name, &AnimationTargetId)>,
    animations: Res<CharacterAnimations>,
    config: Res<CharacterVisualConfig>,
    gangs: Res<GangConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    let Ok((child_of, &key)) = models.get(ready.entity) else {
        return;
    };
    let character = child_of.parent();
    let (_, tint) = look_of(&looks, character, &config, &gangs);
    let mut wired = 0;
    for entity in children.iter_descendants(ready.entity) {
        if let Ok(mut player) = players.get_mut(entity) {
            wire_player(
                entity,
                &mut player,
                character,
                key,
                &animations,
                &mut commands,
            );
            wired += 1;
        }
        if let Ok((name, material)) = meshes.get(entity)
            && name.0 == config.tinted_mesh
        {
            tint_mesh(entity, name, material, tint, &mut materials, &mut commands);
        }
    }
    if wired == 0 {
        error!(
            "gltf character model {}: no AnimationPlayer in the scene",
            ready.entity
        );
    }
    let groups = children
        .iter_descendants(ready.entity)
        .filter_map(|entity| targets.get(entity).ok())
        .map(|(name, target)| {
            let arm = config.arm_joints.iter().any(|joint| joint == name.as_str());
            (*target, if arm { ARMS_GROUP } else { BODY_GROUP })
        })
        .collect::<Vec<_>>();
    assign_mask_groups(&animations.graphs[key.0], &groups, &mut graphs);
}

/// Adds the model's animated nodes to its graph's mask groups (no-op once every node is known).
fn assign_mask_groups(
    graph: &Handle<AnimationGraph>,
    groups: &[(AnimationTargetId, u32)],
    graphs: &mut Assets<AnimationGraph>,
) {
    let known = graphs.get(graph).is_some_and(|graph| {
        groups
            .iter()
            .all(|(target, group)| graph.mask_groups.get(target) == Some(&(1 << group)))
    });
    if known {
        return;
    }
    let Some(mut graph) = graphs.get_mut(graph) else {
        return;
    };
    for (target, group) in groups {
        graph.add_target_to_mask_group(*target, *group);
    }
}

fn wire_player(
    entity: Entity,
    player: &mut AnimationPlayer,
    character: Entity,
    key: ModelKey,
    animations: &CharacterAnimations,
    commands: &mut Commands,
) {
    let mut transitions = AnimationTransitions::new();
    transitions
        .play(
            player,
            animations.nodes[AnimState::Idle as usize],
            Duration::ZERO,
        )
        .repeat();
    commands.entity(entity).insert((
        AnimationGraphHandle(animations.graphs[key.0].clone()),
        key,
        transitions,
        CharacterAnimator {
            character,
            shown: AnimState::Idle,
            armed: false,
            arms: None,
            action: None,
        },
    ));
}

/// Multiplies the base colour of the tinted mesh by `tint` on a clone, so meshes sharing the material keep it.
fn tint_mesh(
    entity: Entity,
    name: &GltfMeshName,
    material: &MeshMaterial3d<StandardMaterial>,
    (r, g, b): (f32, f32, f32),
    materials: &mut Assets<StandardMaterial>,
    commands: &mut Commands,
) {
    if (r, g, b) == (1.0, 1.0, 1.0) {
        return;
    }
    let Some(mut tinted) = materials.get(&material.0).cloned() else {
        error!(
            "gltf character model: material of {} not loaded, tint skipped",
            name.0
        );
        return;
    };
    let base = tinted.base_color.to_linear();
    tinted.base_color = Color::linear_rgba(base.red * r, base.green * g, base.blue * b, base.alpha);
    commands
        .entity(entity)
        .insert(MeshMaterial3d(materials.add(tinted)));
}

#[allow(clippy::type_complexity)]
fn drive_character_animation(
    characters: Query<(
        &AnimState,
        &LinearVelocity,
        Option<&Loadout>,
        (&HitReaction, &Melee),
        Has<Dead>,
        Option<&Civilian>,
    )>,
    mut shots: MessageReader<ShotFired>,
    mut animators: Query<(
        &mut CharacterAnimator,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
    animations: Res<CharacterAnimations>,
    clips: Res<Assets<AnimationClip>>,
    config: Res<CharacterVisualConfig>,
) {
    let shooters = shots.read().map(|shot| shot.shooter).collect::<Vec<_>>();
    let blend = Duration::from_secs_f32(config.blend_seconds);
    for (mut animator, mut player, mut transitions) in &mut animators {
        let Ok((state, velocity, loadout, (reaction, melee), dead, civilian)) =
            characters.get(animator.character)
        else {
            continue;
        };
        if player.animation(animations.rest).is_none() {
            player.start(animations.rest).repeat();
        }
        let knocked_down = reaction.is_knocked_down();
        let cowering = civilian.is_some_and(|c| matches!(c.state, CivilianState::Cower { .. }));
        let action = if dead {
            Some(ShownAction::Death)
        } else if knocked_down {
            Some(ShownAction::Knockdown)
        } else if cowering {
            Some(ShownAction::Cower)
        } else {
            melee.swing.map(|swing| ShownAction::Swing(swing.attack))
        };
        let action_ended = action.is_none() && animator.action.is_some();
        if action != animator.action {
            match (action, melee.swing) {
                (Some(ShownAction::Death), _) => {
                    let lying = animator.action == Some(ShownAction::Knockdown)
                        && animations.death_is_knockdown;
                    if !lying {
                        transitions.play(&mut player, animations.death, blend);
                    }
                }
                (Some(ShownAction::Cower), _) => {
                    transitions
                        .play(&mut player, animations.cower, blend)
                        .repeat();
                }
                (Some(ShownAction::Knockdown), _) => {
                    transitions.play(&mut player, animations.knockdown, blend);
                }
                (Some(ShownAction::Swing(_)), Some(swing)) => {
                    transitions.play(&mut player, swing_node(&animations, swing), blend);
                }
                _ => {}
            }
            animator.action = action;
        }
        let pose = if knocked_down || dead {
            None
        } else {
            arm_pose(loadout.and_then(|loadout| loadout.held))
        };
        let fired = shooters.contains(&animator.character);
        drive_arms(&mut animator, &mut player, &animations, pose, fired);
        if let Some(swing) = melee
            .swing
            .filter(|_| matches!(animator.action, Some(ShownAction::Swing(_))))
        {
            // Stretches the clip over the sim's swing; 1.0 until the clip asset is loaded.
            let speed = clips
                .get(swing_clip(&animations, swing))
                .map_or(1.0, |clip| clip.duration() / swing.duration);
            if let Some(active) = player.animation_mut(swing_node(&animations, swing)) {
                active.set_speed(speed);
            }
        }
        if animator.action.is_some() {
            continue;
        }
        let locomotion = if pose.is_some() {
            &animations.legs
        } else {
            &animations.nodes
        };
        let node = locomotion[*state as usize];
        if action_ended || (*state, pose.is_some()) != (animator.shown, animator.armed) {
            transitions.play(&mut player, node, blend).repeat();
            animator.shown = *state;
            animator.armed = pose.is_some();
        }
        let Some(active) = player.animation_mut(node) else {
            continue;
        };
        let horizontal = Vec2::new(velocity.x, velocity.z).length();
        active.set_speed(playback_rate(*state, horizontal, &config));
    }
}

fn swing_node(animations: &CharacterAnimations, swing: Swing) -> AnimationNodeIndex {
    match swing.weapon {
        MeleeWeapon::Fists => animations.fists[swing.step as usize % 3],
        MeleeWeapon::Bat => animations.bat,
    }
}

fn swing_clip(animations: &CharacterAnimations, swing: Swing) -> &Handle<AnimationClip> {
    match swing.weapon {
        MeleeWeapon::Fists => &animations.fist_clips[swing.step as usize % 3],
        MeleeWeapon::Bat => &animations.bat_clip,
    }
}

/// Freezes every clip of a character in hit-stop; `Option`: the juice plugin may be absent.
fn apply_hit_stop(
    stops: Query<Option<&HitStop>>,
    mut animators: Query<(&CharacterAnimator, &mut AnimationPlayer)>,
) {
    for (animator, mut player) in &mut animators {
        let frozen = stops
            .get(animator.character)
            .ok()
            .flatten()
            .is_some_and(|stop| stop.left > 0.0);
        if frozen {
            player.pause_all();
        } else {
            player.resume_all();
        }
    }
}

/// Arm layer: the hold clip loops; a shot (re)starts the shoot clip, which hands back to hold when it ends.
fn drive_arms(
    animator: &mut CharacterAnimator,
    player: &mut AnimationPlayer,
    animations: &CharacterAnimations,
    pose: Option<ArmPose>,
    fired: bool,
) {
    let wanted = pose.map(|pose| {
        let shoot = animations.shoot[pose as usize];
        let shooting = fired || player.animation(shoot).is_some_and(|a| !a.is_finished());
        let node = if shooting {
            shoot
        } else {
            animations.hold[pose as usize]
        };
        (node, shooting)
    });
    let node = wanted.map(|(node, _)| node);
    if node != animator.arms
        && let Some(old) = animator.arms
    {
        player.stop(old);
    }
    if let Some((new, shooting)) = wanted
        && (node != animator.arms || fired)
    {
        let active = player.start(new);
        if !shooting {
            active.repeat();
        }
    }
    animator.arms = node;
}

/// Clip rate that keeps the feet planted: body speed over the clip's native speed in metres.
pub(super) fn playback_rate(
    state: AnimState,
    horizontal_speed: f32,
    config: &CharacterVisualConfig,
) -> f32 {
    let clip = match state {
        AnimState::Walk => &config.walk,
        AnimState::Run => &config.run,
        AnimState::Sprint => &config.sprint,
        AnimState::Idle | AnimState::Jump | AnimState::Fall => return 1.0,
    };
    horizontal_speed / (clip.native_speed * config.scale())
}
