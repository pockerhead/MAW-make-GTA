mod character;
mod character_config;
#[cfg(test)]
mod character_gate;
mod city;
#[cfg(test)]
mod city_gate;
mod city_mesh;
mod config;
mod facade;
mod pickups;
mod props;
mod sky;
mod weapons;

pub use character_config::{CHARACTER_VISUAL_CONFIG, CharacterClips, CharacterVisualConfig};
pub use config::{RENDER_CONFIG, RenderConfig};
pub use weapons::HeldGun;

use bevy::{
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, GlobalAmbientLight},
    prelude::*,
};
use facade::FacadeMaterial;
use gta_sim::world::Block;

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
            character::CharacterVisualsPlugin,
        ))
        .init_resource::<weapons::WeaponVisualAssets>()
        .add_systems(
            Update,
            (
                pickups::show_available_pickups,
                weapons::show_available_weapon_pickups,
                weapons::show_available_bat_pickups,
                weapons::attach_held_gun,
                weapons::show_held_gun,
            ),
        )
        .add_observer(visualize_block)
        .add_observer(pickups::visualize_pickup)
        .add_observer(weapons::visualize_weapon_pickup)
        .add_observer(weapons::visualize_bat_pickup);
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
