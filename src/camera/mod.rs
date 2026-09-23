mod config;
pub use config::{CAMERA_CONFIG, CameraConfig};

use crate::input::CursorCaptured;
use crate::juice::CameraRecoil;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy_enhanced_input::prelude::*;
use gta_sim::{
    character::{AimIntent, CharacterBody},
    flow::GameState,
    layers::GameLayer,
    player::Player,
};

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub pivot: Option<Vec3>,
    /// 0 = free camera, 1 = aim camera; moves at `1 / aim_transition` per second.
    pub aim_blend: f32,
}

impl OrbitCamera {
    /// Eased aim blend (ease-out quadratic).
    pub fn aim_weight(&self) -> f32 {
        1.0 - (1.0 - self.aim_blend).powi(2)
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<OrbitCamera>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, apply_mouse_look)
            .add_systems(OnExit(GameState::Wasted), reset_pivot)
            .add_systems(
                PostUpdate,
                follow_player.before(TransformSystems::Propagate),
            )
            .add_observer(enable_player_interpolation);
    }
}

fn spawn_camera(mut commands: Commands, config: Res<CameraConfig>) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: config.fov_deg.to_radians(),
            ..default()
        }),
        OrbitCamera {
            yaw: 0.0,
            pitch: 0.0,
            distance: config.distance,
            pivot: None,
            aim_blend: 0.0,
        },
        Transform::default(),
    ));
}

pub fn apply_mouse_look(
    captured: Res<CursorCaptured>,
    config: Res<CameraConfig>,
    look: Single<&Action<crate::input::Look>>,
    mut camera: Single<&mut OrbitCamera>,
) {
    if !captured.0 {
        return;
    }
    let delta = **look;
    let scale = 1.0 + (config.aim_sensitivity_scale - 1.0) * camera.aim_weight();
    let sensitivity = config.mouse_sensitivity_deg.to_radians() * scale;
    camera.yaw -= delta.x * sensitivity;
    camera.pitch = (camera.pitch - delta.y * sensitivity).clamp(
        config.pitch_min_deg.to_radians(),
        config.pitch_max_deg.to_radians(),
    );
}

#[allow(clippy::type_complexity)]
pub fn follow_player(
    time: Res<Time>,
    real: Res<Time<Real>>,
    config: Res<CameraConfig>,
    recoil: Res<CameraRecoil>,
    spatial: SpatialQuery,
    player: Single<(Entity, &Transform, &CharacterBody, &mut AimIntent), With<Player>>,
    mut camera: Single<(&mut OrbitCamera, &mut Transform, &mut Projection), Without<Player>>,
) {
    let (entity, transform, body, mut aim) = player.into_inner();
    let (orbit, camera_transform, projection) = &mut *camera;
    let dt = time.delta_secs();
    let target_blend = if aim.aiming { 1.0 } else { 0.0 };
    let step = real.delta_secs() / config.aim_transition;
    orbit.aim_blend += (target_blend - orbit.aim_blend).clamp(-step, step);
    let weight = orbit.aim_weight();
    let lerp = |free: f32, aimed: f32| free + (aimed - free) * weight;
    if let Projection::Perspective(perspective) = &mut **projection {
        perspective.fov = lerp(config.fov_deg, config.aim_fov_deg).to_radians();
    }
    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    let back = rotation * Vec3::Z;
    let right = rotation * Vec3::X;
    let feet = transform.translation - Vec3::Y * body.float_height;
    let head = feet + Vec3::Y * config.pivot_height;
    let mut pivot = orbit.pivot.unwrap_or(head);
    pivot.smooth_nudge(&head, std::f32::consts::LN_2 / config.follow_half_life, dt);
    orbit.pivot = Some(pivot);
    let shape = Collider::sphere(config.collision_radius);
    // World only: the pivot sits inside the player's head hitbox, and characters never push the camera.
    let filter = SpatialQueryFilter::from_mask(GameLayer::World).with_excluded_entities([entity]);
    let shoulder_length = lerp(config.shoulder_offset, config.aim_shoulder_offset);
    let shoulder_hit = spatial.cast_shape(
        &shape,
        pivot,
        Quat::IDENTITY,
        Dir3::new(right).unwrap(),
        &ShapeCastConfig::from_max_distance(shoulder_length),
        &filter,
    );
    let shoulder = pivot
        + right * shoulder_hit.map_or(shoulder_length, |hit| hit.distance.min(shoulder_length));
    let distance = lerp(config.distance, config.aim_distance);
    let distance_hit = spatial.cast_shape(
        &shape,
        shoulder,
        Quat::IDENTITY,
        Dir3::new(back).unwrap(),
        &ShapeCastConfig::from_max_distance(distance),
        &filter,
    );
    let target = distance_hit.map_or(distance, |hit| hit.distance.min(distance));
    if target < orbit.distance {
        orbit.distance = target;
    } else {
        orbit.distance.smooth_nudge(
            &target,
            std::f32::consts::LN_2 / config.collision_release_half_life,
            dt,
        );
    }
    camera_transform.translation = shoulder + back * orbit.distance;
    // The aim ray is the camera ray without recoil: the kick is visual only (GDD §4.1).
    aim.origin = camera_transform.translation;
    aim.direction = rotation * Vec3::NEG_Z;
    camera_transform.rotation = rotation * Quat::from_rotation_x(recoil.pitch);
}

/// After the respawn teleport the pivot jumps to the new head instead of flying across the city.
fn reset_pivot(mut camera: Single<&mut OrbitCamera>) {
    camera.pivot = None;
}

fn enable_player_interpolation(event: On<Add, Player>, mut commands: Commands) {
    commands.entity(event.entity).insert(TransformInterpolation);
}
