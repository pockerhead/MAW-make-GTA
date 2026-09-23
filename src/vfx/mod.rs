//! Muzzle flash and bullet tracers: short-lived meshes sharing one mesh and material each.
//! Both start at the visible barrel of the shooter's held gun; gameplay rays keep the sim muzzle.

use crate::{juice::JuiceConfig, visuals::HeldGun};
use bevy::{light::NotShadowCaster, prelude::*};
use gta_sim::combat::{BulletTrace, ShotFired};

type Guns<'w, 's> = Query<
    'w,
    's,
    (
        &'static HeldGun,
        &'static GlobalTransform,
        &'static InheritedVisibility,
    ),
>;

/// World position of the shooter's visible barrel end, if it holds a shown gun.
fn barrel_end(guns: &Guns, shooter: Entity) -> Option<Vec3> {
    guns.iter()
        .find(|(gun, _, visible)| gun.owner == shooter && visible.get())
        .map(|(gun, global, _)| global.transform_point(gun.barrel))
}

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VfxAssets>()
            .add_systems(Update, (spawn_flashes, spawn_tracers, expire));
    }
}

#[derive(Resource)]
struct VfxAssets {
    flash_mesh: Handle<Mesh>,
    flash_material: Handle<StandardMaterial>,
    tracer_mesh: Handle<Mesh>,
    tracer_material: Handle<StandardMaterial>,
}

fn unlit(color: (f32, f32, f32)) -> StandardMaterial {
    let (r, g, b) = color;
    StandardMaterial {
        base_color: Color::srgb(r, g, b),
        unlit: true,
        ..default()
    }
}

impl FromWorld for VfxAssets {
    fn from_world(world: &mut World) -> Self {
        let juice = world.resource::<JuiceConfig>().clone();
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let flash_mesh = meshes.add(Sphere::new(juice.flash.size / 2.0));
        let tracer_mesh = meshes.add(Cuboid::from_length(1.0));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self {
            flash_mesh,
            flash_material: materials.add(unlit(juice.flash.color)),
            tracer_mesh,
            tracer_material: materials.add(unlit(juice.tracer.color)),
        }
    }
}

/// Real seconds until the entity despawns.
#[derive(Component)]
struct Lifetime(f32);

fn spawn_flashes(
    mut commands: Commands,
    mut shots: MessageReader<ShotFired>,
    juice: Res<JuiceConfig>,
    assets: Res<VfxAssets>,
    guns: Guns,
) {
    let flash = &juice.flash;
    let (r, g, b) = flash.color;
    for shot in shots.read() {
        commands.spawn((
            Name::new("Muzzle flash"),
            Mesh3d(assets.flash_mesh.clone()),
            MeshMaterial3d(assets.flash_material.clone()),
            Transform::from_translation(barrel_end(&guns, shot.shooter).unwrap_or(shot.muzzle)),
            NotShadowCaster,
            PointLight {
                color: Color::srgb(r, g, b),
                intensity: flash.light_intensity,
                range: flash.light_range,
                shadow_maps_enabled: false,
                ..default()
            },
            Lifetime(flash.seconds),
        ));
    }
}

fn spawn_tracers(
    mut commands: Commands,
    mut traces: MessageReader<BulletTrace>,
    juice: Res<JuiceConfig>,
    assets: Res<VfxAssets>,
    guns: Guns,
) {
    let tracer = &juice.tracer;
    for trace in traces.read() {
        let from = barrel_end(&guns, trace.shooter).unwrap_or(trace.from);
        let path = trace.to - from;
        let length = path.length();
        let Ok(direction) = Dir3::new(path) else {
            continue;
        };
        commands.spawn((
            Name::new("Tracer"),
            Mesh3d(assets.tracer_mesh.clone()),
            MeshMaterial3d(assets.tracer_material.clone()),
            Transform::from_translation(from + path / 2.0)
                .looking_to(direction, Vec3::Y)
                .with_scale(Vec3::new(tracer.width, tracer.width, length)),
            NotShadowCaster,
            Lifetime(tracer.seconds),
        ));
    }
}

fn expire(
    mut commands: Commands,
    real: Res<Time<Real>>,
    mut effects: Query<(Entity, &mut Lifetime)>,
) {
    for (entity, mut lifetime) in &mut effects {
        lifetime.0 -= real.delta_secs();
        if lifetime.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}
