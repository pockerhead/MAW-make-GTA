use super::RenderConfig;
use bevy::{asset::RenderAssetUsages, mesh::Indices, mesh::PrimitiveTopology, prelude::*};
use gta_sim::{
    flow::GameState,
    world::{BuildingKind, City, CityBuilding, DistrictKind},
};

pub struct CityVisualsPlugin;

impl Plugin for CityVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_city_palette)
            .add_systems(OnEnter(GameState::Playing), spawn_city_surfaces)
            .add_observer(visualize_city_building);
    }
}

/// One shared material per city colour.
#[derive(Resource)]
struct CityPalette {
    downtown: Handle<StandardMaterial>,
    commercial: Handle<StandardMaterial>,
    residential: Handle<StandardMaterial>,
    industrial: Handle<StandardMaterial>,
    hospital: Handle<StandardMaterial>,
    police: Handle<StandardMaterial>,
    gang_hq: Handle<StandardMaterial>,
    road: Handle<StandardMaterial>,
    sidewalk: Handle<StandardMaterial>,
    park: Handle<StandardMaterial>,
}

fn setup_city_palette(
    mut commands: Commands,
    config: Res<RenderConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut add = |(r, g, b): (f32, f32, f32)| materials.add(Color::srgb(r, g, b));
    let colors = &config.district_colors;
    commands.insert_resource(CityPalette {
        downtown: add(colors.downtown),
        commercial: add(colors.commercial),
        residential: add(colors.residential),
        industrial: add(colors.industrial),
        hospital: add(config.hospital_color),
        police: add(config.police_color),
        gang_hq: add(config.gang_hq_color),
        road: add(config.road_color),
        sidewalk: add(config.sidewalk_color),
        park: add(config.park_color),
    });
}

fn visualize_city_building(
    event: On<Add, CityBuilding>,
    buildings: Query<&CityBuilding>,
    palette: Res<CityPalette>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok(building) = buildings.get(event.entity) else {
        return;
    };
    let material = match (building.kind, building.district) {
        (BuildingKind::Hospital, _) => &palette.hospital,
        (BuildingKind::PoliceStation, _) => &palette.police,
        (BuildingKind::GangHq(_), _) => &palette.gang_hq,
        (BuildingKind::Generic, DistrictKind::Downtown) => &palette.downtown,
        (BuildingKind::Generic, DistrictKind::Commercial) => &palette.commercial,
        (BuildingKind::Generic, DistrictKind::Residential) => &palette.residential,
        (BuildingKind::Generic, DistrictKind::Industrial) => &palette.industrial,
    };
    commands.entity(event.entity).insert((
        Mesh3d(meshes.add(Cuboid::from_size(building.size))),
        MeshMaterial3d(material.clone()),
    ));
}

fn spawn_city_surfaces(
    mut commands: Commands,
    city: Res<City>,
    config: Res<RenderConfig>,
    palette: Res<CityPalette>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let layout = &city.0;
    let half = layout.ground_size / 2.0;
    let ground = [
        Vec2::new(-half, -half),
        Vec2::new(half, -half),
        Vec2::new(half, half),
        Vec2::new(-half, half),
    ];
    let step = config.surface_layer_step;
    let curbs = layout.blocks.iter().map(|b| b.curb.as_slice());
    let parks = layout.blocks.iter().filter(|b| b.is_park).map(|b| {
        if b.inner.is_empty() {
            b.curb.as_slice()
        } else {
            b.inner.as_slice()
        }
    });
    for (mesh, material) in [
        (flat_mesh(std::iter::once(&ground[..]), 0.0), &palette.road),
        (flat_mesh(curbs, step), &palette.sidewalk),
        (flat_mesh(parks, 2.0 * step), &palette.park),
    ] {
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone())));
    }
}

/// Horizontal mesh of counter-clockwise (positive-area in (x, z)) convex polygons at height `y`.
fn flat_mesh<'a>(polys: impl Iterator<Item = &'a [Vec2]>, y: f32) -> Mesh {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for poly in polys.filter(|p| p.len() >= 3) {
        let base = positions.len() as u32;
        positions.extend(poly.iter().map(|p| [p.x, y, p.y]));
        // Positive area in (x, z) is clockwise seen from +Y, so the fan is (0, i + 1, i).
        for i in 1..poly.len() as u32 - 1 {
            indices.extend([base, base + i + 1, base + i]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}
