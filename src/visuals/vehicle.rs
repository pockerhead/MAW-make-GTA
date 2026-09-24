//! Kenney Car Kit sedan on every car body: the front wheels steer, all wheels spin and follow the
//! suspension; the driver's model is hidden; a wrecked car smokes from the hood.

use super::{RenderConfig, character::CharacterModel};
use crate::juice::JuiceConfig;
use avian3d::prelude::LinearVelocity;
use bevy::{
    light::NotShadowCaster,
    prelude::*,
    world_serialization::{WorldAsset, WorldInstanceReady},
};
use gta_sim::{
    player::Player,
    vehicle::{Driving, GRAVITY, Vehicle, VehicleConfig, VehicleHealth},
};

/// glTF faces +Z, car bodies face −Z.
const MODEL_YAW: f32 = std::f32::consts::PI;

pub(super) struct VehicleVisualsPlugin;

impl Plugin for VehicleVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleVisualAssets>()
            .add_observer(spawn_vehicle_model)
            .add_systems(
                Update,
                (animate_wheels, hide_driver_model, emit_smoke, drift_smoke),
            );
    }
}

#[derive(Resource)]
struct VehicleVisualAssets {
    scene: Handle<WorldAsset>,
    puff_mesh: Handle<Mesh>,
}

impl FromWorld for VehicleVisualAssets {
    fn from_world(world: &mut World) -> Self {
        let model = world.resource::<RenderConfig>().vehicle.model.clone();
        let scene = world
            .resource::<AssetServer>()
            .load(GltfAssetLabel::Scene(0).from_asset(model));
        let puff_mesh = world.resource_mut::<Assets<Mesh>>().add(Sphere::new(0.5));
        Self { scene, puff_mesh }
    }
}

/// Root of the glTF instance under a car; the wheel nodes once the scene is ready.
#[derive(Component)]
struct VehicleModel {
    /// Front-left, front-right, back-left, back-right node and its rest translation.
    wheels: [Option<(Entity, Vec3)>; 4],
    /// Wheel roll angle, rad.
    spin: f32,
}

fn spawn_vehicle_model(
    event: On<Add, Vehicle>,
    config: Res<RenderConfig>,
    assets: Res<VehicleVisualAssets>,
    mut commands: Commands,
) {
    let v = &config.vehicle;
    let transform = Transform::from_xyz(v.offset.0, v.offset.1, v.offset.2)
        .with_rotation(Quat::from_rotation_y(MODEL_YAW))
        .with_scale(Vec3::splat(v.scale));
    commands
        .entity(event.entity)
        .insert(Visibility::default())
        .with_children(|parent| {
            parent
                .spawn((
                    VehicleModel {
                        wheels: [None; 4],
                        spin: 0.0,
                    },
                    WorldAssetRoot(assets.scene.clone()),
                    transform,
                ))
                .observe(on_vehicle_ready);
        });
}

fn on_vehicle_ready(
    ready: On<WorldInstanceReady>,
    config: Res<RenderConfig>,
    children: Query<&Children>,
    nodes: Query<(&Name, &Transform)>,
    mut models: Query<&mut VehicleModel>,
) {
    let Ok(mut model) = models.get_mut(ready.entity) else {
        return;
    };
    for entity in children.iter_descendants(ready.entity) {
        let Ok((name, transform)) = nodes.get(entity) else {
            continue;
        };
        if let Some(i) = config
            .vehicle
            .wheels
            .iter()
            .position(|w| w == name.as_str())
        {
            model.wheels[i] = Some((entity, transform.translation));
        }
    }
    if model.wheels.iter().any(Option::is_none) {
        error!("car model {}: wheel nodes missing", ready.entity);
    }
}

/// Front wheels turn by the steer angle (the model's 180° yaw commutes with it), every wheel rolls
/// by `v·dt/r` and its hub follows the suspension relative to the rest compression.
fn animate_wheels(
    time: Res<Time>,
    render: Res<RenderConfig>,
    cfg: Res<VehicleConfig>,
    cars: Query<(&Vehicle, &Transform, &LinearVelocity, &Children)>,
    mut models: Query<&mut VehicleModel>,
    mut transforms: Query<&mut Transform, Without<Vehicle>>,
) {
    let rest = GRAVITY / (std::f32::consts::TAU * cfg.suspension.frequency_hz).powi(2);
    let scale = render.vehicle.scale;
    for (vehicle, car, velocity, children) in &cars {
        let Some(root) = children.iter().find(|c| models.contains(*c)) else {
            continue;
        };
        let Ok(mut model) = models.get_mut(root) else {
            continue;
        };
        let forward_speed = velocity.dot(car.rotation * Vec3::NEG_Z);
        model.spin += forward_speed * time.delta_secs() / cfg.wheels.radius;
        let spin = Quat::from_rotation_x(model.spin);
        for (i, wheel) in model.wheels.iter().enumerate() {
            let Some((node, base)) = *wheel else {
                continue;
            };
            let Ok(mut transform) = transforms.get_mut(node) else {
                continue;
            };
            let state = vehicle.wheels[i];
            let lift = if state.grounded {
                state.compression - rest
            } else {
                -rest
            };
            let yaw = if i < 2 { -vehicle.steer } else { 0.0 };
            transform.translation = base + Vec3::Y * lift / scale;
            transform.rotation = Quat::from_rotation_y(yaw) * spin;
        }
    }
}

/// The driver sits inside the car: its body model is hidden while it drives.
fn hide_driver_model(
    players: Query<(Has<Driving>, &Children), With<Player>>,
    mut models: Query<&mut Visibility, With<CharacterModel>>,
) {
    for (driving, children) in &players {
        let wanted = if driving {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for child in children.iter() {
            if let Ok(mut visibility) = models.get_mut(child) {
                visibility.set_if_neq(wanted);
            }
        }
    }
}

/// One smoke puff: its age in real seconds; hidden puffs are free for reuse.
#[derive(Component)]
struct SmokePuff {
    age: f32,
    from: Vec3,
}

/// Every `interval` real seconds each wrecked car emits a puff at its hood, reusing a free one.
#[allow(clippy::too_many_arguments)]
fn emit_smoke(
    real: Res<Time<Real>>,
    juice: Res<JuiceConfig>,
    assets: Res<VehicleVisualAssets>,
    cars: Query<(&Transform, &VehicleHealth), With<Vehicle>>,
    mut puffs: Query<(&mut SmokePuff, &mut Visibility), Without<Vehicle>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut clock: Local<f32>,
    mut commands: Commands,
) {
    let cfg = &juice.smoke;
    *clock += real.delta_secs();
    if *clock < cfg.interval {
        return;
    }
    *clock = 0.0;
    let mut free = puffs
        .iter_mut()
        .filter(|(_, visibility)| **visibility == Visibility::Hidden);
    let (x, y, z) = cfg.hood;
    for (car, health) in &cars {
        if health.current > 0.0 {
            continue;
        }
        let from = car.transform_point(Vec3::new(x, y, z));
        if let Some((mut puff, mut visibility)) = free.next() {
            *puff = SmokePuff { age: 0.0, from };
            *visibility = Visibility::Inherited;
            continue;
        }
        let (r, g, b, a) = cfg.color;
        commands.spawn((
            SmokePuff { age: 0.0, from },
            Mesh3d(assets.puff_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(r, g, b, a),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_translation(from).with_scale(Vec3::splat(cfg.size)),
            Visibility::Inherited,
            NotShadowCaster,
        ));
    }
}

/// Puffs rise, grow and fade over `seconds`, then hide.
fn drift_smoke(
    real: Res<Time<Real>>,
    juice: Res<JuiceConfig>,
    mut puffs: Query<(
        &mut SmokePuff,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cfg = &juice.smoke;
    for (mut puff, mut transform, mut visibility, material) in &mut puffs {
        if *visibility == Visibility::Hidden {
            continue;
        }
        puff.age += real.delta_secs();
        let t = (puff.age / cfg.seconds).min(1.0);
        if t >= 1.0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        transform.translation = puff.from + Vec3::Y * cfg.rise * t;
        transform.scale = Vec3::splat(cfg.size * (1.0 + t));
        if let Some(mut material) = materials.get_mut(&material.0) {
            let (r, g, b, a) = cfg.color;
            material.base_color = Color::srgba(r, g, b, a * (1.0 - t));
        }
    }
}
