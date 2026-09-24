mod config;
pub use config::{CAMERA_CONFIG, CameraConfig};

use crate::input::CursorCaptured;
use crate::juice::{CameraRecoil, CameraShake};
use crate::settings::GameSettings;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy_enhanced_input::prelude::*;
use gta_sim::{
    character::{AimIntent, CharacterBody},
    combat::aim_yaw,
    flow::{GameState, NEW_CITY},
    layers::GameLayer,
    player::Player,
    population::{CameraView, ViewCone},
    vehicle::{Driving, Vehicle},
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
    /// Real seconds since the last mouse look; the car camera swings back behind the car after
    /// `car_look_return`.
    pub look_idle: f32,
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
            .add_systems(OnExit(GameState::Busted), reset_pivot)
            .add_systems(NEW_CITY, reset_pivot)
            .add_systems(
                PostUpdate,
                (
                    follow_player.before(TransformSystems::Propagate),
                    publish_camera_view
                        .after(follow_player)
                        .run_if(any_with_component::<Player>),
                ),
            )
            .add_observer(enable_player_interpolation)
            .add_observer(enable_vehicle_interpolation);
    }
}

/// Signed shortest turn from angle `from` to angle `to`, rad, in (−π, π].
pub fn shortest_arc(from: f32, to: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let delta = (to - from).rem_euclid(TAU);
    if delta > PI { delta - TAU } else { delta }
}

/// `current` moved towards `target` along the shortest arc, halving the gap every `half_life`.
fn approach_angle(current: f32, target: f32, half_life: f32, dt: f32) -> f32 {
    let keep = 0.5f32.powf(dt / half_life);
    current + shortest_arc(current, target) * (1.0 - keep)
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
            look_idle: 0.0,
        },
        Transform::default(),
    ));
}

pub fn apply_mouse_look(
    captured: Res<CursorCaptured>,
    config: Res<CameraConfig>,
    settings: Res<GameSettings>,
    look: Single<&Action<crate::input::Look>>,
    drive_look: Single<&Action<crate::input::DriveLook>>,
    mut camera: Single<&mut OrbitCamera>,
) {
    if !captured.0 {
        return;
    }
    // The inactive context's action reads zero.
    let delta = ***look + ***drive_look;
    if delta != Vec2::ZERO {
        camera.look_idle = 0.0;
    }
    let scale = 1.0 + (config.aim_sensitivity_scale - 1.0) * camera.aim_weight();
    let sensitivity =
        config.mouse_sensitivity_deg.to_radians() * scale * settings.mouse_sensitivity;
    let pitch_sign = if settings.invert_y { -1.0 } else { 1.0 };
    camera.yaw -= delta.x * sensitivity;
    camera.pitch = (camera.pitch - pitch_sign * delta.y * sensitivity).clamp(
        config.pitch_min_deg.to_radians(),
        config.pitch_max_deg.to_radians(),
    );
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn follow_player(
    time: Res<Time>,
    real: Res<Time<Real>>,
    config: Res<CameraConfig>,
    recoil: Res<CameraRecoil>,
    shake: Res<CameraShake>,
    spatial: SpatialQuery,
    player: Single<
        (
            Entity,
            &Transform,
            &CharacterBody,
            &mut AimIntent,
            Option<&Driving>,
        ),
        With<Player>,
    >,
    cars: Query<&Transform, (With<Vehicle>, Without<OrbitCamera>, Without<Player>)>,
    mut camera: Single<(&mut OrbitCamera, &mut Transform, &mut Projection), Without<Player>>,
) {
    let (entity, transform, body, mut aim, driving) = player.into_inner();
    let (orbit, camera_transform, projection) = &mut *camera;
    let dt = time.delta_secs();
    let car = driving.and_then(|d| cars.get(d.vehicle).ok());
    orbit.look_idle += real.delta_secs();
    if let Some(car) = car
        && orbit.look_idle >= config.car_look_return
    {
        let behind = aim_yaw(car.rotation * Vec3::NEG_Z);
        let half_life = config.car_yaw_half_life;
        orbit.yaw = approach_angle(orbit.yaw, behind, half_life, real.delta_secs());
        orbit.pitch = approach_angle(
            orbit.pitch,
            config.car_pitch_deg.to_radians(),
            half_life,
            real.delta_secs(),
        );
    }
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
    let head = match car {
        Some(car) => car.translation + Vec3::Y * config.car_pivot_height,
        None => feet + Vec3::Y * config.pivot_height,
    };
    let mut pivot = orbit.pivot.unwrap_or(head);
    pivot.smooth_nudge(&head, std::f32::consts::LN_2 / config.follow_half_life, dt);
    orbit.pivot = Some(pivot);
    let shape = Collider::sphere(config.collision_radius);
    // World only: the pivot sits inside the player's head hitbox, and characters never push the camera.
    let filter = SpatialQueryFilter::from_mask(GameLayer::World).with_excluded_entities([entity]);
    let shoulder_length = match car {
        Some(_) => 0.0,
        None => lerp(config.shoulder_offset, config.aim_shoulder_offset),
    };
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
    let distance = match car {
        Some(_) => config.car_distance,
        None => lerp(config.distance, config.aim_distance),
    };
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
    // The aim ray is the camera ray without recoil or shake: both are visual only (GDD §4.1).
    aim.origin = camera_transform.translation;
    aim.direction = rotation * Vec3::NEG_Z;
    camera_transform.rotation = rotation * Quat::from_rotation_x(recoil.pitch) * shake.rotation;
}

/// Hands the sim the camera's view cone (final pose, recoil and shake included) for off-screen spawning.
fn publish_camera_view(
    camera: Single<(&Transform, &Projection), With<OrbitCamera>>,
    mut view: ResMut<CameraView>,
) {
    let (transform, projection) = *camera;
    let Projection::Perspective(perspective) = projection else {
        return;
    };
    view.0 = Some(ViewCone::from_perspective(
        transform.translation,
        transform.forward(),
        perspective.fov,
        perspective.aspect_ratio,
    ));
}

/// After the respawn teleport the pivot jumps to the new head instead of flying across the city.
fn reset_pivot(mut camera: Single<&mut OrbitCamera>) {
    camera.pivot = None;
}

fn enable_player_interpolation(event: On<Add, Player>, mut commands: Commands) {
    commands.entity(event.entity).insert(TransformInterpolation);
}

fn enable_vehicle_interpolation(event: On<Add, Vehicle>, mut commands: Commands) {
    commands.entity(event.entity).insert(TransformInterpolation);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_arc_rows() {
        let deg = |d: f32| d.to_radians();
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert!(close(shortest_arc(deg(170.0), deg(-170.0)), deg(20.0)));
        assert!(close(shortest_arc(deg(10.0), deg(-10.0)), deg(-20.0)));
        assert!(shortest_arc(0.0, deg(180.0)).abs() <= deg(180.0) + 1e-4);
        assert!(close(shortest_arc(deg(-179.0), deg(179.0)), deg(-2.0)));
        // Yaw of a car: −Z → 0, −X → 90°, +Z → 180°.
        assert!(close(aim_yaw(Vec3::NEG_Z), 0.0));
        assert!(close(aim_yaw(Vec3::NEG_X), deg(90.0)));
        assert!(close(aim_yaw(Vec3::Z).abs(), deg(180.0)));
    }

    #[test]
    fn approach_angle_halves_the_gap_per_half_life() {
        let next = approach_angle(0.0, 1.0, 0.2, 0.2);
        assert!((next - 0.5).abs() < 1e-5);
        // Across the ±π seam it turns the short way.
        let deg = |d: f32| d.to_radians();
        let next = approach_angle(deg(170.0), deg(-170.0), 0.2, 0.2);
        assert!((next - deg(180.0)).abs() < 1e-4);
    }
}
