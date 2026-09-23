//! Headless gameplay simulation.

pub mod character;
pub mod config;
pub mod flow;
pub mod player;
pub mod world;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua_avian3d::prelude::*;
use character::{CharacterPlugin, LOCOMOTION_CONFIG, LocomotionConfig};
use config::{ConfigError, ConfigRoot, load_config};
use flow::FlowPlugin;
use player::PlayerPlugin;
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
    if let WorldSource::City { .. } = source {
        let params = load_config::<CityParams>(&root, CITY_CONFIG)?;
        params.validate().map_err(|message| ConfigError {
            path: root.path(CITY_CONFIG),
            message,
        })?;
        app.insert_resource(CityParamsRes(params));
    }
    app.insert_resource(root).insert_resource(cfg).add_plugins((
        FlowPlugin,
        PhysicsPlugins::default(),
        TnuaAvian3dPlugin::new(FixedUpdate),
        CharacterPlugin,
        WorldPlugin { source },
        PlayerPlugin,
    ));
    Ok(())
}
