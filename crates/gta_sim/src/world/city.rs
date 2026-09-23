use super::PlayerSpawn;
use crate::flow::GameState;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use citygen::{
    BuildingKind, CityLayout, CityParams, DistrictKind, GenError, Vec2 as LayoutVec2, centroid,
    generate, layout_hash,
};

// The ground is a 1 m thick static slab with its top face at y = 0 (collision geometry law).
const GROUND_SLAB_THICKNESS: f32 = 1.0;

/// Seed of the generated city; read by QA over BRP.
#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct CitySeed(pub u64);

/// `citygen::layout_hash` of the generated city; read by QA over BRP.
#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct CityLayoutHash(pub u64);

/// The generated city layout; present from the frame generation finished.
#[derive(Resource)]
pub struct City(pub CityLayout);

/// Validated generator parameters loaded from `world/city.ron`.
#[derive(Resource)]
pub struct CityParamsRes(pub CityParams);

#[derive(Component)]
pub struct CityBuilding {
    pub size: Vec3,
    pub district: DistrictKind,
    pub kind: BuildingKind,
}

#[derive(Component)]
pub struct CityGround;

#[derive(Component)]
pub struct CityEdgeWall;

/// Raised curb-and-sidewalk prism of one city block (one convex static collider).
#[derive(Component)]
pub struct CityBlock;

/// Landmark points of the generated city; read by QA over BRP.
#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct CityLandmarks {
    pub tower_roof: Vec3,
    pub plaza_center: Vec3,
    pub park_center: Vec3,
}

#[derive(Resource)]
pub(super) struct CityGenTask(Task<Result<(CityLayout, u64), GenError>>);

pub(super) fn start_city_generation(
    mut commands: Commands,
    seed: Res<CitySeed>,
    params: Res<CityParamsRes>,
) {
    let seed = seed.0;
    let params = params.0.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let layout = generate(seed, &params)?;
        let hash = layout_hash(&layout);
        Ok((layout, hash))
    });
    commands.insert_resource(CityGenTask(task));
}

pub(super) fn apply_city_generation(
    mut commands: Commands,
    mut task: ResMut<CityGenTask>,
    params: Res<CityParamsRes>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(result) = block_on(poll_once(&mut task.0)) else {
        return;
    };
    commands.remove_resource::<CityGenTask>();
    let (layout, hash) = match result {
        Ok(generated) => generated,
        Err(err) => {
            error!("city generation failed: {err}");
            exit.write(AppExit::error());
            return;
        }
    };
    let curb = params.0.roads.curb_height;
    let spawn = layout.player_spawn;
    commands.insert_resource(PlayerSpawn(Vec3::new(spawn.x, curb, spawn.y)));
    spawn_ground_and_walls(&mut commands, &params.0, layout.ground_size);
    if let Err(err) = spawn_blocks(&mut commands, &layout, curb) {
        error!("city generation failed: {err}");
        exit.write(AppExit::error());
        return;
    }
    spawn_buildings(&mut commands, &layout);
    commands.insert_resource(landmarks(&layout, curb));
    commands.insert_resource(CityLayoutHash(hash));
    commands.insert_resource(City(layout));
    next.set(GameState::Playing);
}

fn spawn_ground_and_walls(commands: &mut Commands, params: &CityParams, ground: f32) {
    commands.spawn((
        CityGround,
        RigidBody::Static,
        Collider::cuboid(ground, GROUND_SLAB_THICKNESS, ground),
        Transform::from_xyz(0.0, -GROUND_SLAB_THICKNESS / 2.0, 0.0),
    ));
    let (h, t) = (params.edge_wall.height, params.edge_wall.thickness);
    let length = ground + 2.0 * t;
    let offset = ground / 2.0 + t / 2.0;
    for (size, center) in [
        (Vec3::new(t, h, length), Vec3::new(offset, h / 2.0, 0.0)),
        (Vec3::new(t, h, length), Vec3::new(-offset, h / 2.0, 0.0)),
        (Vec3::new(length, h, t), Vec3::new(0.0, h / 2.0, offset)),
        (Vec3::new(length, h, t), Vec3::new(0.0, h / 2.0, -offset)),
    ] {
        commands.spawn((
            CityEdgeWall,
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Transform::from_translation(center),
        ));
    }
}

fn landmarks(layout: &CityLayout, curb: f32) -> CityLandmarks {
    let lm = layout.landmarks;
    let tower = &layout.buildings[lm.tower as usize];
    let flat = |p: LayoutVec2| Vec3::new(p.x, curb, p.y);
    CityLandmarks {
        tower_roof: Vec3::new(tower.center.x, tower.height, tower.center.y),
        plaza_center: flat(centroid(&layout.blocks[lm.plaza as usize].inner)),
        park_center: flat(centroid(&layout.blocks[lm.park as usize].inner)),
    }
}

/// One convex prism per block from the ground slab bottom to the curb top.
fn spawn_blocks(commands: &mut Commands, layout: &CityLayout, curb: f32) -> Result<(), String> {
    for (id, block) in layout.blocks.iter().enumerate() {
        if block.curb.len() < 3 {
            continue;
        }
        let points = block
            .curb
            .iter()
            .flat_map(|p| {
                [
                    Vec3::new(p.x, -GROUND_SLAB_THICKNESS, p.y),
                    Vec3::new(p.x, curb, p.y),
                ]
            })
            .collect();
        let collider = Collider::convex_hull(points)
            .ok_or_else(|| format!("block {id}: degenerate curb polygon"))?;
        commands.spawn((CityBlock, RigidBody::Static, collider, Transform::IDENTITY));
    }
    Ok(())
}

fn spawn_buildings(commands: &mut Commands, layout: &CityLayout) {
    for building in &layout.buildings {
        let block = layout.lots[building.lot as usize].block;
        let district = layout.districts[layout.blocks[block as usize].district as usize].kind;
        let size = Vec3::new(
            2.0 * building.half_extents.x,
            building.height,
            2.0 * building.half_extents.y,
        );
        let (c, u) = (building.center, building.axis);
        // Local X goes along the frontage axis u, local Z along u.perp(); layout (x, y) is world (x, z).
        let rotation = Quat::from_rotation_y(f32::atan2(-u.y, u.x));
        commands.spawn((
            CityBuilding {
                size,
                district,
                kind: building.kind,
            },
            RigidBody::Static,
            building_collider(building),
            Transform::from_xyz(c.x, size.y / 2.0, c.y).with_rotation(rotation),
        ));
    }
}

/// A cuboid for a plain box; a compound of the base and every setback tier otherwise. Parts are
/// placed relative to the building centre at `height / 2`.
fn building_collider(building: &citygen::Building) -> Collider {
    let height = building.height;
    let base = building.half_extents;
    if building.upper_tiers.is_empty() {
        return Collider::cuboid(2.0 * base.x, height, 2.0 * base.y);
    }
    let mut slabs = vec![(0.0, base)];
    slabs.extend(
        building
            .upper_tiers
            .iter()
            .map(|t| (t.bottom, t.half_extents)),
    );
    let parts = slabs
        .iter()
        .enumerate()
        .map(|(k, &(bottom, half))| {
            let top = slabs.get(k + 1).map_or(height, |next| next.0);
            let mid = (bottom + top) / 2.0;
            (
                Vec3::new(0.0, mid - height / 2.0, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(2.0 * half.x, top - bottom, 2.0 * half.y),
            )
        })
        .collect();
    Collider::compound(parts)
}
