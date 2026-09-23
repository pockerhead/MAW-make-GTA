mod city;
mod test_area;

pub use city::{
    City, CityBuilding, CityEdgeWall, CityGround, CityLayoutHash, CityParamsRes, CitySeed,
};
pub use citygen::{BuildingKind, CityLayout, CityParams, DistrictKind, RoadClass};

use crate::flow::GameState;
use bevy::prelude::*;
use city::{apply_city_generation, start_city_generation};
use test_area::spawn_test_area;

/// Path of the city generator parameters, relative to the assets root.
pub const CITY_CONFIG: &str = "world/city.ron";

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Block {
    pub size: Vec3,
}

#[derive(Resource)]
pub struct PlayerSpawn(pub Vec3);

/// Which world the simulation builds while in `GameState::Loading`.
#[derive(Clone, Copy, Debug)]
pub enum WorldSource {
    /// The fixed fixture level of the headless character gates.
    TestArea,
    /// A generated city; `compose_sim` loads its parameters.
    City { seed: u64 },
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum WorldSystems {
    Generation,
}

pub struct WorldPlugin {
    pub source: WorldSource,
}

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlayerSpawn(Vec3::ZERO))
            .register_type::<Block>();
        match self.source {
            WorldSource::TestArea => {
                app.add_systems(
                    OnEnter(GameState::Loading),
                    (spawn_test_area, finish_loading),
                );
            }
            WorldSource::City { seed } => {
                app.insert_resource(CitySeed(seed))
                    .register_type::<CitySeed>()
                    .register_type::<CityLayoutHash>()
                    .configure_sets(
                        Update,
                        WorldSystems::Generation.run_if(in_state(GameState::Loading)),
                    )
                    .add_systems(OnEnter(GameState::Loading), start_city_generation)
                    .add_systems(
                        Update,
                        apply_city_generation.in_set(WorldSystems::Generation),
                    );
            }
        }
    }
}

fn finish_loading(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Playing);
}
