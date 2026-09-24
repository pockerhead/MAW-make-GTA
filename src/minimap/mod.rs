//! Round minimap bottom-left (GDD §7): the city raster rotated with the camera, gang territories,
//! the search circle, cop view cones, the player arrow and marks for cops, gangs, pickups, the
//! hospital and the police station.

mod config;
mod markers;
mod material;

pub use config::MinimapConfig;
pub use markers::{MarkerKind, MinimapMarker};

use crate::camera::{OrbitCamera, follow_player};
use crate::menu::{UiConfig, UiFonts};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
    ui::UiSystems,
};
use gta_sim::{
    character::Dead,
    flow::{GameState, NEW_CITY},
    gang::GangConfig,
    player::Player,
    police::PoliceUnit,
    wanted::{WantedConfig, WantedLevel},
    world::{City, CityScoped, Raster, RasterStyle, heading_on_map, rasterize},
};
use material::{MAX_CONES, MinimapMaterial, MinimapUniform};

/// The minimap root node; read by QA over BRP.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Minimap;

#[derive(Resource)]
struct MinimapRasterTask(Task<Raster>);

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<MinimapMaterial>::default())
            .register_type::<Minimap>()
            .register_type::<MinimapMarker>()
            .register_type::<MarkerKind>()
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                (spawn_minimap, start_raster),
            )
            .add_systems(NEW_CITY, drop_raster_task)
            .add_systems(
                Update,
                (
                    poll_raster.run_if(resource_exists::<MinimapRasterTask>),
                    markers::sync_markers,
                ),
            )
            .add_systems(
                PostUpdate,
                (update_minimap, markers::place_markers)
                    .after(follow_player)
                    .before(UiSystems::Layout),
            );
    }
}

/// sRGB bytes of a unit colour.
fn srgb8((r, g, b): (f32, f32, f32), alpha: f32) -> [u8; 4] {
    [r, g, b, alpha].map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn linear((r, g, b, a): (f32, f32, f32, f32)) -> Vec4 {
    Color::srgba(r, g, b, a).to_linear().to_vec4()
}

fn spawn_minimap(mut commands: Commands, ui: Res<UiConfig>, fonts: Res<UiFonts>) {
    let hud = &ui.hud;
    let cfg = &hud.minimap;
    // Hidden until the raster is ready.
    let root = commands
        .spawn((
            Name::new("Minimap"),
            Minimap,
            CityScoped,
            Node {
                position_type: PositionType::Absolute,
                left: px(hud.margin),
                bottom: px(hud.margin),
                width: px(cfg.size),
                height: px(cfg.size),
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    markers::spawn_landmarks(&mut commands, root, cfg, &fonts);
}

fn start_raster(
    mut commands: Commands,
    city: Res<City>,
    ui: Res<UiConfig>,
    gangs: Res<GangConfig>,
) {
    let cfg = &ui.hud.minimap;
    let tint = |g: usize| {
        let tint = gangs.gangs.get(g).map_or((1.0, 1.0, 1.0), |spec| spec.tint);
        srgb8(tint, cfg.territory_alpha)
    };
    let style = RasterStyle {
        px_per_m: cfg.raster_px_per_m,
        road: srgb8(cfg.road, 1.0),
        sidewalk: srgb8(cfg.sidewalk, 1.0),
        block: srgb8(cfg.block, 1.0),
        park: srgb8(cfg.park, 1.0),
        building: srgb8(cfg.building, 1.0),
        territory: [tint(0), tint(1)],
    };
    let layout = city.0.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move { rasterize(&layout, &style) });
    commands.insert_resource(MinimapRasterTask(task));
}

fn poll_raster(
    mut commands: Commands,
    mut task: ResMut<MinimapRasterTask>,
    ui: Res<UiConfig>,
    wanted: Res<WantedConfig>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<MinimapMaterial>>,
    root: Single<(Entity, &mut Visibility), With<Minimap>>,
) {
    let Some(raster) = block_on(poll_once(&mut task.0)) else {
        return;
    };
    commands.remove_resource::<MinimapRasterTask>();
    let cfg = &ui.hud.minimap;
    let size = Vec2::new(raster.width as f32, raster.height as f32) / raster.px_per_m;
    let origin = raster.origin;
    let mut image = Image::new(
        Extent3d {
            width: raster.width,
            height: raster.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        raster.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    let (r, g, b) = cfg.arrow_color;
    let data = MinimapUniform {
        raster_origin: origin,
        raster_size: size,
        view_radius: cfg.view_radius,
        rim_px: cfg.rim_px,
        arrow_px: cfg.arrow_px,
        cone: Vec4::new(
            wanted.cop_view_distance,
            (wanted.cop_view_cone_deg / 2.0).to_radians().cos(),
            0.0,
            cfg.search_ring_px,
        ),
        outside: linear(cfg.outside),
        rim: linear(cfg.rim_color),
        search_fill: linear(cfg.search_fill),
        search_ring: linear(cfg.search_ring),
        cone_color: linear(cfg.cone_color),
        arrow_color: linear((r, g, b, 1.0)),
        ..default()
    };
    let material = materials.add(MinimapMaterial {
        data,
        map: images.add(image),
    });
    let (entity, mut visibility) = root.into_inner();
    commands.entity(entity).insert(MaterialNode(material));
    *visibility = Visibility::Inherited;
}

fn drop_raster_task(mut commands: Commands) {
    commands.remove_resource::<MinimapRasterTask>();
}

/// Feeds the shader the frame's centre, camera yaw, arrow, search circle and nearest cops.
#[allow(clippy::too_many_arguments)]
fn update_minimap(
    ui: Res<UiConfig>,
    wanted_cfg: Res<WantedConfig>,
    wanted: Res<WantedLevel>,
    player: Single<&Transform, With<Player>>,
    camera: Single<&OrbitCamera>,
    cops: Query<&Transform, (With<PoliceUnit>, Without<Dead>)>,
    root: Single<&MaterialNode<MinimapMaterial>, With<Minimap>>,
    mut materials: ResMut<Assets<MinimapMaterial>>,
) {
    let Some(mut material) = materials.get_mut(&root.0) else {
        return;
    };
    let cfg = &ui.hud.minimap;
    let centre = player.translation.xz();
    let yaw = camera.yaw;
    let facing = player.rotation.to_euler(EulerRot::YXZ).0;
    let data = &mut material.data;
    data.center = centre;
    data.heading = Vec2::new(yaw.sin(), yaw.cos());
    data.arrow_heading = heading_on_map(facing, yaw);
    data.search = wanted
        .search_circle(&wanted_cfg.stars)
        .map_or(Vec4::ZERO, |(at, radius)| {
            Vec4::new(at.x, at.z, radius, 1.0)
        });
    let reach = cfg.view_radius + wanted_cfg.cop_view_distance;
    let mut near: Vec<(f32, Vec4)> = cops
        .iter()
        .filter_map(|t| {
            let at = t.translation.xz();
            let distance = at.distance(centre);
            let forward = (t.rotation * Vec3::NEG_Z).xz().normalize_or_zero();
            (distance <= reach).then(|| (distance, Vec4::new(at.x, at.y, forward.x, forward.y)))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near.truncate(MAX_CONES);
    data.cone.z = near.len() as f32;
    for (slot, (_, cop)) in data.cops.iter_mut().zip(&near) {
        *slot = *cop;
    }
}
