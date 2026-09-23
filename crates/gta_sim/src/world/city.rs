use super::PlayerSpawn;
use crate::flow::GameState;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use citygen::{
    BuildingKind, CityLayout, CityParams, DistrictKind, GenError, generate, layout_hash,
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
    let spawn = layout.player_spawn;
    commands.insert_resource(PlayerSpawn(Vec3::new(spawn.x, 0.0, spawn.y)));
    spawn_ground_and_walls(&mut commands, &params.0, layout.ground_size);
    spawn_buildings(&mut commands, &layout);
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
            Collider::cuboid(size.x, size.y, size.z),
            Transform::from_xyz(c.x, size.y / 2.0, c.y).with_rotation(rotation),
        ));
    }
}
