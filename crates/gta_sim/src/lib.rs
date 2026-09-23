//! Headless gameplay simulation.

pub mod character;
pub mod combat;
pub mod config;
pub mod flow;
pub mod player;
pub mod wanted;
pub mod world;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua_avian3d::prelude::*;
use character::{
    CharacterPlugin, HEALTH_CONFIG, HealthConfig, LOCOMOTION_CONFIG, LocomotionConfig,
};
use combat::CombatPlugin;
use config::{ConfigError, ConfigRoot, load_config};
use flow::{FlowPlugin, RESPAWN_CONFIG, RespawnConfig};
use player::PlayerPlugin;
use wanted::WantedPlugin;
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
    if let WorldSource::City { .. } = source {
        let params = load_config::<CityParams>(&root, CITY_CONFIG)?;
        params.validate().map_err(|message| ConfigError {
            path: root.path(CITY_CONFIG),
            message,
        })?;
        app.insert_resource(CityParamsRes(params));
    }
    app.insert_resource(root)
        .insert_resource(cfg)
        .insert_resource(health)
        .insert_resource(respawn)
        .add_plugins((
            FlowPlugin,
            PhysicsPlugins::default(),
            TnuaAvian3dPlugin::new(FixedUpdate),
            CharacterPlugin,
            WorldPlugin { source },
            PlayerPlugin,
            CombatPlugin,
            WantedPlugin,
        ));
    Ok(())
}
