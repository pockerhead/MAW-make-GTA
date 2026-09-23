//! Kenney glTF humanoid on every character body, animated from `AnimState`.

use super::character_config::{CharacterClips, CharacterVisualConfig};
use avian3d::prelude::LinearVelocity;
use bevy::{gltf::GltfMeshName, prelude::*, world_serialization::WorldInstanceReady};
use gta_sim::character::{AnimState, CharacterBody};
use std::time::Duration;

/// glTF faces +Z, gameplay bodies face -Z.
const MODEL_YAW: f32 = std::f32::consts::PI;

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
    pub(super) nodes: [AnimationNodeIndex; 6],
}

impl FromWorld for CharacterAnimations {
    fn from_world(world: &mut World) -> Self {
        let model = world.resource::<CharacterVisualConfig>().model.clone();
        let clips = world.resource::<CharacterClips>().0;
        let asset_server = world.resource::<AssetServer>().clone();
        let mut graph = AnimationGraph::new();
        let root = graph.root;
        let nodes = clips.map(|index| {
            graph.add_clip(
                asset_server.load(GltfAssetLabel::Animation(index).from_asset(model.clone())),
                1.0,
                root,
            )
        });
        let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        Self { graph, nodes }
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
    animations: Res<CharacterAnimations>,
    config: Res<CharacterVisualConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    characters: Query<(&AnimState, &LinearVelocity)>,
    mut animators: Query<(
        &mut CharacterAnimator,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
    animations: Res<CharacterAnimations>,
    config: Res<CharacterVisualConfig>,
) {
    for (mut animator, mut player, mut transitions) in &mut animators {
        let Ok((state, velocity)) = characters.get(animator.character) else {
            continue;
        };
        let node = animations.nodes[*state as usize];
        if *state != animator.shown {
            transitions
                .play(
                    &mut player,
                    node,
                    Duration::from_secs_f32(config.blend_seconds),
                )
                .repeat();
            animator.shown = *state;
        }
        let Some(active) = player.animation_mut(node) else {
            continue;
        };
        let horizontal = Vec2::new(velocity.x, velocity.z).length();
        active.set_speed(playback_rate(*state, horizontal, &config));
    }
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
