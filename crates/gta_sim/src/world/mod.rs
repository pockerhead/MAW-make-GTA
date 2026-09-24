mod city;
mod test_area;

pub use city::{
    City, CityBlock, CityBuilding, CityEdgeWall, CityGround, CityLandmarks, CityLayoutHash,
    CityParamsRes, CitySeed,
};
pub use citygen::minimap::{Raster, RasterStyle, heading_on_map, map_px, project, rasterize};
pub use citygen::{
    BuildingKind, CityLayout, CityParams, DistrictKind, Landmarks, RoadClass, Tier, centroid,
    contains_convex,
};

use crate::flow::{GameState, NEW_CITY};
use bevy::prelude::*;
use city::{apply_city_generation, start_city_generation};
use test_area::spawn_test_area;

/// Path of the city generator parameters, relative to the assets root.
pub const CITY_CONFIG: &str = "world/city.ron";

/// Lives as long as the current city: despawned on `flow::NEW_CITY`.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct CityScoped;

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

/// Respawn point on the sidewalk in front of the (first) police station and the unit sidewalk
/// direction there. Read by QA over BRP.
#[derive(Resource, Reflect, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct PoliceStationSpawn {
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
            .insert_resource(PoliceStationSpawn {
                point: test_area::STATION_SPAWN,
                along: Vec3::X,
            })
            .register_type::<HospitalSpawn>()
            .register_type::<PoliceStationSpawn>()
            .register_type::<Block>()
            .register_type::<CityScoped>();
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
                    )
                    .add_systems(NEW_CITY, (despawn_city, drop_city));
            }
        }
    }
}

// A scoped child of a scoped parent is already gone with its parent: `try_despawn`.
fn despawn_city(mut commands: Commands, scoped: Query<Entity, With<CityScoped>>) {
    for entity in &scoped {
        commands.entity(entity).try_despawn();
    }
}

/// QA and menus never read the old city's hash while the new one loads.
fn drop_city(mut commands: Commands) {
    commands.remove_resource::<City>();
    commands.remove_resource::<CityLayoutHash>();
    commands.remove_resource::<CityLandmarks>();
}

fn finish_loading(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Playing);
}
