mod city;
mod test_area;

pub use city::{
    City, CityBlock, CityBuilding, CityEdgeWall, CityGround, CityLandmarks, CityLayoutHash,
    CityParamsRes, CitySeed,
};
pub use citygen::{
    BuildingKind, CityLayout, CityParams, DistrictKind, Landmarks, RoadClass, Tier, centroid,
    contains_convex,
};

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

/// Respawn point on the sidewalk in front of the hospital and the unit sidewalk direction there;
/// the pickups stand along `along` on both sides. Read by QA over BRP.
#[derive(Resource, Reflect, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct HospitalSpawn {
    pub point: Vec3,
    pub along: Vec3,
}

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
            .insert_resource(HospitalSpawn {
                point: Vec3::ZERO,
                along: Vec3::X,
            })
            .register_type::<HospitalSpawn>()
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
                    .register_type::<CityLandmarks>()
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
