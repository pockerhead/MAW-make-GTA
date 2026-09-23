mod city;
#[cfg(test)]
mod city_gate;
mod city_mesh;
mod config;
mod facade;
mod props;
mod sky;

pub use config::{RENDER_CONFIG, RenderConfig};

use bevy::{
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, GlobalAmbientLight},
    prelude::*,
};
use facade::FacadeMaterial;
use gta_sim::{character::CharacterBody, world::Block};

pub struct VisualsPlugin;

impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        let config = app.world().resource::<RenderConfig>();
        let brightness = config.ambient_brightness;
        let shadow_map = config.shadows.map_size;
        let (r, g, b) = config.fog.color;
        app.insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness,
            ..default()
        })
        .insert_resource(DirectionalLightShadowMap { size: shadow_map })
        .insert_resource(ClearColor(Color::srgb(r, g, b)))
        .add_systems(Startup, spawn_light)
        .add_plugins((
            MaterialPlugin::<FacadeMaterial>::default(),
            sky::SkyPlugin,
            city::CityVisualsPlugin,
        ))
        .add_observer(visualize_block)
        .add_observer(visualize_character);
    }
}

fn spawn_light(mut commands: Commands, config: Res<RenderConfig>) {
    let shadows = &config.shadows;
    commands.spawn((
        CascadeShadowConfigBuilder {
            num_cascades: shadows.cascades,
            first_cascade_far_bound: shadows.first_cascade_far_bound,
            maximum_distance: shadows.maximum_distance,
            ..default()
        }
        .build(),
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: config.sun_illuminance,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            config.sun_pitch_deg.to_radians(),
            config.sun_yaw_deg.to_radians(),
            0.0,
        )),
    ));
}

fn visualize_block(
    event: On<Add, Block>,
    blocks: Query<&Block>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(block) = blocks.get(event.entity) else {
        return;
    };
    commands.entity(event.entity).insert((
        Mesh3d(meshes.add(Cuboid::from_size(block.size))),
        MeshMaterial3d(materials.add(Color::srgb(0.55, 0.58, 0.62))),
    ));
}

fn visualize_character(
    event: On<Add, CharacterBody>,
    bodies: Query<&CharacterBody>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(body) = bodies.get(event.entity) else {
        return;
    };
    let visible_height = body.float_height + body.height / 2.0;
    let center_offset = (body.float_height - body.height / 2.0) / 2.0;
    let material = materials.add(Color::srgb(0.2, 0.5, 0.9));
    commands
        .entity(event.entity)
        .insert(Visibility::default())
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Capsule3d {
                    radius: body.radius,
                    half_length: visible_height / 2.0 - body.radius,
                })),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(0.0, -center_offset, 0.0),
            ));
            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.16, 0.12, 0.3))),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, 0.25, -body.radius),
            ));
        });
}
