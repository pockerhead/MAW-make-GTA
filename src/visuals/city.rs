use super::{
    RenderConfig,
    city_mesh::{ChunkMesh, build_city_meshes},
    facade::{FacadeMaterial, facade_material},
    props::{CityProp, PropAssets, PropPlacement, load_prop_assets, place_props, prop_bundle},
};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use gta_sim::{
    flow::GameState,
    world::{City, CityParamsRes},
};

/// Owns all city geometry on the render side: merged chunk meshes and props.
pub struct CityVisualsPlugin;

impl Plugin for CityVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CityChunk>()
            .register_type::<CityProp>()
            .add_systems(Startup, (create_facade_material, load_prop_assets))
            // Once per session: returning from `Wasted` must not build the city a second time.
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                start_city_mesh_build,
            )
            .add_systems(
                Update,
                (
                    poll_city_mesh_build.run_if(resource_exists::<CityMeshTask>),
                    apply_city_spawn.run_if(resource_exists::<PendingCitySpawn>),
                )
                    .chain(),
            );
    }
}

/// One merged mesh of city geometry (ground, blocks, buildings, markings) per render chunk.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct CityChunk {
    pub coord: UVec2,
}

#[derive(Resource)]
pub(super) struct CityMeshTask(Task<(Vec<ChunkMesh>, Vec<PropPlacement>)>);

/// Built geometry waiting to be spawned within the per-frame budget.
#[derive(Resource)]
pub(super) struct PendingCitySpawn {
    chunks: Vec<ChunkMesh>,
    props: Vec<PropPlacement>,
}

#[derive(Resource)]
struct CityFacade(Handle<FacadeMaterial>);

fn create_facade_material(
    mut commands: Commands,
    config: Res<RenderConfig>,
    mut materials: ResMut<Assets<FacadeMaterial>>,
) {
    commands.insert_resource(CityFacade(materials.add(facade_material(&config))));
}

fn start_city_mesh_build(
    mut commands: Commands,
    city: Res<City>,
    params: Res<CityParamsRes>,
    config: Res<RenderConfig>,
) {
    let (layout, params, config) = (city.0.clone(), params.0.clone(), config.clone());
    let task = AsyncComputeTaskPool::get().spawn(async move {
        (
            build_city_meshes(&layout, &params, &config),
            place_props(&layout, &params, &config),
        )
    });
    commands.insert_resource(CityMeshTask(task));
}

fn poll_city_mesh_build(mut commands: Commands, mut task: ResMut<CityMeshTask>) {
    let Some((chunks, props)) = block_on(poll_once(&mut task.0)) else {
        return;
    };
    commands.remove_resource::<CityMeshTask>();
    commands.insert_resource(PendingCitySpawn { chunks, props });
}

fn apply_city_spawn(
    mut commands: Commands,
    mut pending: ResMut<PendingCitySpawn>,
    config: Res<RenderConfig>,
    facade: Res<CityFacade>,
    props: Res<PropAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let budget = &config.spawn_budget;
    let take = pending.chunks.len().saturating_sub(budget.chunks_per_frame);
    for chunk in pending.chunks.drain(take..) {
        commands.spawn((
            CityChunk { coord: chunk.coord },
            Mesh3d(meshes.add(chunk.mesh)),
            MeshMaterial3d(facade.0.clone()),
            Transform::IDENTITY,
        ));
    }
    let take = pending.props.len().saturating_sub(budget.props_per_frame);
    for prop in pending.props.drain(take..) {
        commands.spawn(prop_bundle(&props, &config, prop));
    }
    if pending.chunks.is_empty() && pending.props.is_empty() {
        commands.remove_resource::<PendingCitySpawn>();
    }
}
