mod config;
pub use config::{CAMERA_CONFIG, CameraConfig};

use crate::input::CursorCaptured;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy_enhanced_input::prelude::*;
use gta_sim::{character::CharacterBody, player::Player};

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub pivot: Option<Vec3>,
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<OrbitCamera>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, apply_mouse_look)
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
    let sensitivity = config.mouse_sensitivity_deg.to_radians();
    camera.yaw -= delta.x * sensitivity;
    camera.pitch = (camera.pitch - delta.y * sensitivity).clamp(
        config.pitch_min_deg.to_radians(),
        config.pitch_max_deg.to_radians(),
    );
}

fn follow_player(
    time: Res<Time>,
    config: Res<CameraConfig>,
    spatial: SpatialQuery,
    player: Single<(Entity, &Transform, &CharacterBody), With<Player>>,
    mut camera: Single<(&mut OrbitCamera, &mut Transform), Without<Player>>,
) {
    let (entity, transform, body) = *player;
    let (orbit, camera_transform) = &mut *camera;
    let dt = time.delta_secs();
    let rotation = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    let back = rotation * Vec3::Z;
    let right = rotation * Vec3::X;
    let feet = transform.translation - Vec3::Y * body.float_height;
    let head = feet + Vec3::Y * config.pivot_height;
    let mut pivot = orbit.pivot.unwrap_or(head);
    pivot.smooth_nudge(&head, std::f32::consts::LN_2 / config.follow_half_life, dt);
    orbit.pivot = Some(pivot);
    let shape = Collider::sphere(config.collision_radius);
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let shoulder_length = config.shoulder_offset;
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
    let distance_hit = spatial.cast_shape(
        &shape,
        shoulder,
        Quat::IDENTITY,
        Dir3::new(back).unwrap(),
        &ShapeCastConfig::from_max_distance(config.distance),
        &filter,
    );
    let target = distance_hit.map_or(config.distance, |hit| hit.distance.min(config.distance));
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
    camera_transform.rotation = rotation;
}

fn enable_player_interpolation(event: On<Add, Player>, mut commands: Commands) {
    commands.entity(event.entity).insert(TransformInterpolation);
}
