use super::RenderConfig;
use bevy::{
    color::ColorToComponents,
    light::NotShadowCaster,
    pbr::{DistanceFog, FogFalloff},
    prelude::*,
};

/// Gradient sky dome that follows the camera, and linear distance fog on the 3D camera.
pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(add_fog)
            .add_systems(Startup, spawn_sky_dome)
            .add_systems(Update, follow_camera);
    }
}

#[derive(Component)]
struct SkyDome;

fn srgb((r, g, b): (f32, f32, f32)) -> Color {
    Color::srgb(r, g, b)
}

fn add_fog(event: On<Add, Camera3d>, config: Res<RenderConfig>, mut commands: Commands) {
    let fog = &config.fog;
    let (r, g, b, a) = fog.sun_glow;
    commands.entity(event.entity).insert(DistanceFog {
        color: srgb(fog.color),
        directional_light_color: Color::srgba(r, g, b, a),
        directional_light_exponent: fog.sun_glow_exponent,
        falloff: FogFalloff::Linear {
            start: fog.start,
            end: fog.end,
        },
    });
}

fn spawn_sky_dome(
    mut commands: Commands,
    config: Res<RenderConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sky = &config.sky;
    let mut mesh = Sphere::new(sky.radius).mesh().uv(sky.sectors, sky.stacks);
    let horizon = srgb(config.fog.color).to_linear().to_vec3();
    let zenith = srgb(sky.zenith).to_linear().to_vec3();
    let colors = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|positions| positions.as_float3())
        .expect("sphere mesh has float3 positions")
        .iter()
        .map(|p| {
            let t = (p[1] / sky.radius)
                .clamp(0.0, 1.0)
                .powf(sky.gradient_exponent);
            horizon.lerp(zenith, t).extend(1.0).to_array()
        })
        .collect::<Vec<_>>();
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    commands.spawn((
        SkyDome,
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            fog_enabled: false,
            cull_mode: None,
            ..default()
        })),
        NotShadowCaster,
        Transform::default(),
    ));
}

fn follow_camera(
    camera: Single<&Transform, (With<Camera3d>, Without<SkyDome>)>,
    mut dome: Single<&mut Transform, With<SkyDome>>,
) {
    dome.translation = camera.translation;
}
