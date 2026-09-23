//! Headless gameplay simulation.

pub mod character;
pub mod config;
pub mod player;
pub mod world;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua_avian3d::prelude::*;
use character::{CharacterPlugin, LOCOMOTION_CONFIG, LocomotionConfig};
use config::{ConfigError, ConfigRoot, load_config};
use player::PlayerPlugin;
use world::WorldPlugin;

pub fn compose_sim(app: &mut App, root: ConfigRoot) -> Result<(), ConfigError> {
    let cfg = load_config::<LocomotionConfig>(&root, LOCOMOTION_CONFIG)?;
    app.insert_resource(root).insert_resource(cfg).add_plugins((
        PhysicsPlugins::default(),
        TnuaAvian3dPlugin::new(FixedUpdate),
        CharacterPlugin,
        WorldPlugin,
        PlayerPlugin,
    ));
    Ok(())
}
