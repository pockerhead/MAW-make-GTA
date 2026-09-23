//! Kenney glTF humanoid on every character body, animated from `AnimState`.

use super::character_config::{CharacterClips, CharacterVisualConfig};
use avian3d::prelude::LinearVelocity;
use bevy::{
    animation::AnimationTargetId, gltf::GltfMeshName, prelude::*,
    world_serialization::WorldInstanceReady,
};
use gta_sim::{
    character::{AnimState, CharacterBody},
    combat::{Loadout, ShotFired, Weapon},
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
            .add_systems(Update, drive_character_animation);
    }
}

/// Animation graph shared by every character model; node `i` plays the clip of `AnimState` `i`.
#[derive(Resource)]
pub(super) struct CharacterAnimations {
    pub(super) graph: Handle<AnimationGraph>,
    /// Locomotion on every joint (unarmed).
    pub(super) nodes: [AnimationNodeIndex; 6],
    /// The same locomotion without the arms (armed: the arm layer owns them).
    pub(super) legs: [AnimationNodeIndex; 6],
    /// Arm-only clips per `ArmPose`.
    pub(super) hold: [AnimationNodeIndex; 2],
    pub(super) shoot: [AnimationNodeIndex; 2],
}

impl FromWorld for CharacterAnimations {
    fn from_world(world: &mut World) -> Self {
        let model = world.resource::<CharacterVisualConfig>().model.clone();
        let clips = *world.resource::<CharacterClips>();
        let asset_server = world.resource::<AssetServer>().clone();
        let clip = |index: usize| {
            asset_server.load(GltfAssetLabel::Animation(index).from_asset(model.clone()))
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
        let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        Self {
            graph,
            nodes,
            legs,
            hold,
            shoot,
        }
    }
}

/// Root of the glTF model instance under a character body.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct CharacterModel;

/// On the model's `AnimationPlayer`: which character it follows and which state it shows.
#[derive(Component)]
pub(super) struct CharacterAnimator {
    pub(super) character: Entity,
    pub(super) shown: AnimState,
    /// Whether `shown` plays on the `legs` nodes (a gun is held).
    pub(super) armed: bool,
    /// The arm-layer node playing, if any.
    pub(super) arms: Option<AnimationNodeIndex>,
}

/// Model feet (y = 0) on the ground: the body centre floats `float_height` above it.
pub(super) fn model_transform(float_height: f32, scale: f32) -> Transform {
    Transform::from_xyz(0.0, -float_height, 0.0)
        .with_rotation(Quat::from_rotation_y(MODEL_YAW))
        .with_scale(Vec3::splat(scale))
}

fn spawn_character_model(
    event: On<Add, CharacterBody>,
    bodies: Query<&CharacterBody>,
    config: Res<CharacterVisualConfig>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(body) = bodies.get(event.entity) else {
        return;
    };
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(config.model.clone()));
    let transform = model_transform(body.float_height, config.scale());
    commands
        .entity(event.entity)
        .insert(Visibility::default())
        .with_children(|parent| {
            parent
                .spawn((CharacterModel, WorldAssetRoot(scene), transform))
                .observe(on_model_ready);
        });
}

#[allow(clippy::too_many_arguments)]
fn on_model_ready(
    ready: On<WorldInstanceReady>,
    models: Query<&ChildOf, With<CharacterModel>>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
    meshes: Query<(&GltfMeshName, &MeshMaterial3d<StandardMaterial>)>,
    targets: Query<(&Name, &AnimationTargetId)>,
    animations: Res<CharacterAnimations>,
    config: Res<CharacterVisualConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    let Ok(child_of) = models.get(ready.entity) else {
        return;
    };
    let character = child_of.parent();
    let mut wired = 0;
    for entity in children.iter_descendants(ready.entity) {
        if let Ok(mut player) = players.get_mut(entity) {
            wire_player(entity, &mut player, character, &animations, &mut commands);
            wired += 1;
        }
        if let Ok((name, material)) = meshes.get(entity) {
            tint_mesh(
                entity,
                name,
                material,
                &config,
                &mut materials,
                &mut commands,
            );
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
    assign_mask_groups(&animations.graph, &groups, &mut graphs);
}

/// Adds the model's animated nodes to the shared graph's mask groups (no-op once every node is known).
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
        AnimationGraphHandle(animations.graph.clone()),
        transitions,
        CharacterAnimator {
            character,
            shown: AnimState::Idle,
            armed: false,
            arms: None,
        },
    ));
}

/// Multiplies the base colour of `tinted_mesh` by `tint` on a clone, so meshes sharing the material keep it.
fn tint_mesh(
    entity: Entity,
    name: &GltfMeshName,
    material: &MeshMaterial3d<StandardMaterial>,
    config: &CharacterVisualConfig,
    materials: &mut Assets<StandardMaterial>,
    commands: &mut Commands,
) {
    let (r, g, b) = config.tint;
    if (r, g, b) == (1.0, 1.0, 1.0) || name.0 != config.tinted_mesh {
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

fn drive_character_animation(
    characters: Query<(&AnimState, &LinearVelocity, Option<&Loadout>)>,
    mut shots: MessageReader<ShotFired>,
    mut animators: Query<(
        &mut CharacterAnimator,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
    animations: Res<CharacterAnimations>,
    config: Res<CharacterVisualConfig>,
) {
    let shooters = shots.read().map(|shot| shot.shooter).collect::<Vec<_>>();
    for (mut animator, mut player, mut transitions) in &mut animators {
        let Ok((state, velocity, loadout)) = characters.get(animator.character) else {
            continue;
        };
        let pose = arm_pose(loadout.and_then(|loadout| loadout.held));
        let locomotion = if pose.is_some() {
            &animations.legs
        } else {
            &animations.nodes
        };
        let node = locomotion[*state as usize];
        if (*state, pose.is_some()) != (animator.shown, animator.armed) {
            transitions
                .play(
                    &mut player,
                    node,
                    Duration::from_secs_f32(config.blend_seconds),
                )
                .repeat();
            animator.shown = *state;
            animator.armed = pose.is_some();
        }
        let fired = shooters.contains(&animator.character);
        drive_arms(&mut animator, &mut player, &animations, pose, fired);
        let Some(active) = player.animation_mut(node) else {
            continue;
        };
        let horizontal = Vec2::new(velocity.x, velocity.z).length();
        active.set_speed(playback_rate(*state, horizontal, &config));
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
