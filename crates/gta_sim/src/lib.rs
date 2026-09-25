//! Headless gameplay simulation.

pub mod character;
pub mod civilian;
pub mod combat;
pub mod config;
pub mod flow;
pub mod gang;
pub mod layers;
pub mod navigation;
pub mod perception;
pub mod player;
pub mod police;
pub mod population;
pub(crate) mod tactics;
pub mod traffic;
pub mod vehicle;
pub mod wanted;
pub mod world;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua_avian3d::prelude::*;
use character::{
    CharacterPlugin, HEALTH_CONFIG, HealthConfig, LOCOMOTION_CONFIG, LocomotionConfig,
};
use civilian::{CIVILIAN_CONFIG, CivilianConfig, CivilianPlugin};
use combat::{
    AIM_CONFIG, AimConfig, CombatPlugin, MELEE_CONFIG, MeleeConfig, WEAPONS_CONFIG, WeaponsConfig,
};
use config::{ConfigError, ConfigRoot, load_config};
use flow::{FlowPlugin, RESPAWN_CONFIG, RespawnConfig};
use gang::{GANG_CONFIG, GangConfig, GangPlugin};
use navigation::{NAVIGATION_CONFIG, NavigationConfig, NavigationPlugin};
use perception::{PERCEPTION_CONFIG, PerceptionConfig, PerceptionPlugin};
use player::PlayerPlugin;
use police::{EscalationConfig, POLICE_CONFIG, PolicePlugin};
use population::{POPULATION_CONFIG, PopulationConfig, PopulationPlugin};
use traffic::{TRAFFIC_CONFIG, TrafficConfig, TrafficPlugin};
use vehicle::{DAMAGE_CONFIG, DamageConfig, VEHICLE_CONFIG, VehicleConfig, VehiclePlugin};
use wanted::{WANTED_CONFIG, WantedConfig, WantedPlugin};
use world::{CITY_CONFIG, CityParams, CityParamsRes, WorldPlugin, WorldSource};

/// Adds the headless simulation. The caller adds `StatesPlugin` first (`DefaultPlugins` has it).
pub fn compose_sim(
    app: &mut App,
    root: ConfigRoot,
    source: WorldSource,
) -> Result<(), ConfigError> {
    let cfg = load_config::<LocomotionConfig>(&root, LOCOMOTION_CONFIG)?;
    cfg.validate().map_err(|message| ConfigError {
        path: root.path(LOCOMOTION_CONFIG),
        message,
    })?;
    let health = load_config::<HealthConfig>(&root, HEALTH_CONFIG)?;
    health.validate().map_err(|message| ConfigError {
        path: root.path(HEALTH_CONFIG),
        message,
    })?;
    let respawn = load_config::<RespawnConfig>(&root, RESPAWN_CONFIG)?;
    respawn.validate().map_err(|message| ConfigError {
        path: root.path(RESPAWN_CONFIG),
        message,
    })?;
    let weapons = load_config::<WeaponsConfig>(&root, WEAPONS_CONFIG)?;
    weapons.validate().map_err(|message| ConfigError {
        path: root.path(WEAPONS_CONFIG),
        message,
    })?;
    let aim = load_config::<AimConfig>(&root, AIM_CONFIG)?;
    aim.validate().map_err(|message| ConfigError {
        path: root.path(AIM_CONFIG),
        message,
    })?;
    let melee = load_config::<MeleeConfig>(&root, MELEE_CONFIG)?;
    melee.validate().map_err(|message| ConfigError {
        path: root.path(MELEE_CONFIG),
        message,
    })?;
    let population = load_config::<PopulationConfig>(&root, POPULATION_CONFIG)?;
    population.validate().map_err(|message| ConfigError {
        path: root.path(POPULATION_CONFIG),
        message,
    })?;
    let perception = load_config::<PerceptionConfig>(&root, PERCEPTION_CONFIG)?;
    perception.validate().map_err(|message| ConfigError {
        path: root.path(PERCEPTION_CONFIG),
        message,
    })?;
    let navigation = load_config::<NavigationConfig>(&root, NAVIGATION_CONFIG)?;
    navigation.validate().map_err(|message| ConfigError {
        path: root.path(NAVIGATION_CONFIG),
        message,
    })?;
    let civilian = load_config::<CivilianConfig>(&root, CIVILIAN_CONFIG)?;
    civilian.validate().map_err(|message| ConfigError {
        path: root.path(CIVILIAN_CONFIG),
        message,
    })?;
    civilian
        .validate_fight_hearing(perception.fight_hearing_radius)
        .map_err(|message| ConfigError {
            path: root.path(CIVILIAN_CONFIG),
            message,
        })?;
    let gangs = load_config::<GangConfig>(&root, GANG_CONFIG)?;
    gangs.validate().map_err(|message| ConfigError {
        path: root.path(GANG_CONFIG),
        message,
    })?;
    let wanted = load_config::<WantedConfig>(&root, WANTED_CONFIG)?;
    wanted.validate().map_err(|message| ConfigError {
        path: root.path(WANTED_CONFIG),
        message,
    })?;
    let police = load_config::<EscalationConfig>(&root, POLICE_CONFIG)?;
    police
        .validate()
        .and_then(|()| police.validate_ring(population.despawn_distance))
        .and_then(|()| police.validate_overshoot(&weapons))
        .map_err(|message| ConfigError {
            path: root.path(POLICE_CONFIG),
            message,
        })?;
    let vehicle = load_config::<VehicleConfig>(&root, VEHICLE_CONFIG)?;
    vehicle.validate().map_err(|message| ConfigError {
        path: root.path(VEHICLE_CONFIG),
        message,
    })?;
    let vehicle_damage = load_config::<DamageConfig>(&root, DAMAGE_CONFIG)?;
    vehicle_damage.validate().map_err(|message| ConfigError {
        path: root.path(DAMAGE_CONFIG),
        message,
    })?;
    // A witness perceives within `slots` ticks of the stimulus (Bevy's default fixed tick, never overridden).
    let tick = Time::<Fixed>::default().timestep().as_secs_f32();
    let traffic = load_config::<TrafficConfig>(&root, TRAFFIC_CONFIG)?;
    traffic
        .validate(tick)
        .and_then(|()| traffic.validate_cross(vehicle.max_speed, population.despawn_distance))
        .map_err(|message| ConfigError {
            path: root.path(TRAFFIC_CONFIG),
            message,
        })?;
    let call_delay = civilian.longest_call_delay(
        population.corpse_seconds,
        f32::from(perception.slots) * tick,
        cfg.speed(civilian.flee_gait),
    );
    wanted
        .validate_call_delay(call_delay)
        .map_err(|message| ConfigError {
            path: root.path(WANTED_CONFIG),
            message,
        })?;
    // A city run rolls from `CitySeed` (reseeded at every load); the fixed test level always rolls the
    // same sequence.
    let combat_seed = match source {
        WorldSource::City { seed } => seed,
        WorldSource::TestArea => 0,
    };
    if let WorldSource::City { .. } = source {
        let params = load_config::<CityParams>(&root, CITY_CONFIG)?;
        params
            .validate()
            .and_then(|()| parking_fits(&params, &vehicle))
            .map_err(|message| ConfigError {
                path: root.path(CITY_CONFIG),
                message,
            })?;
        app.insert_resource(CityParamsRes(params));
    }
    app.insert_resource(root)
        .insert_resource(cfg)
        .insert_resource(health)
        .insert_resource(respawn)
        .insert_resource(weapons)
        .insert_resource(aim)
        .insert_resource(melee)
        .insert_resource(population)
        .insert_resource(perception)
        .insert_resource(navigation)
        .insert_resource(civilian)
        .insert_resource(gangs)
        .insert_resource(wanted)
        .insert_resource(police)
        .insert_resource(vehicle)
        .insert_resource(vehicle_damage)
        .insert_resource(traffic)
        .add_plugins((
            FlowPlugin,
            PhysicsPlugins::default(),
            TnuaAvian3dPlugin::new(FixedUpdate),
            CharacterPlugin,
            WorldPlugin { source },
            PlayerPlugin,
            CombatPlugin { seed: combat_seed },
            NavigationPlugin,
            PerceptionPlugin,
            PopulationPlugin { seed: combat_seed },
            CivilianPlugin,
            GangPlugin { seed: combat_seed },
            PolicePlugin { seed: combat_seed },
            WantedPlugin,
        ));
    // Outside the tuple: `Plugins` is implemented for tuples of at most 15.
    app.add_plugins((VehiclePlugin, TrafficPlugin));
    Ok(())
}

/// A parked car stays inside its curb lane: the centre at least half a car width from the curb,
/// the far side inside the lane.
fn parking_fits(params: &CityParams, car: &VehicleConfig) -> Result<(), String> {
    let half_width = car.half_extents().x;
    let offset = params.parking.curb_offset;
    if offset < half_width || offset + half_width > params.roads.lane_width {
        return Err(format!(
            "parking.curb_offset {offset} must keep a car (half width {half_width}) inside the curb lane \
             ({half_width} <= curb_offset <= roads.lane_width - {half_width})"
        ));
    }
    Ok(())
}
