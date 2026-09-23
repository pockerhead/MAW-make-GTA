use bevy::{light::GlobalAmbientLight, prelude::*};
use gta_sim::{character::CharacterBody, world::Block};
use serde::Deserialize;

pub const RENDER_CONFIG: &str = "world/render.ron";

#[derive(Resource, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderConfig {
    ambient_brightness: f32,
    sun_illuminance: f32,
    sun_pitch_deg: f32,
    sun_yaw_deg: f32,
}

pub struct VisualsPlugin;

impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        let config = app.world().resource::<RenderConfig>();
        let brightness = config.ambient_brightness;
        app.insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness,
            ..default()
        })
        .add_systems(Startup, spawn_light)
        .add_observer(visualize_block)
        .add_observer(visualize_character);
    }
}

fn spawn_light(mut commands: Commands, config: Res<RenderConfig>) {
    commands.spawn((
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
