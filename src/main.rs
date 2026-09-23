mod camera;
#[cfg(feature = "debug")]
mod debug;
mod input;
mod menu;
#[cfg(feature = "dev")]
mod remote;
mod visuals;

use bevy::{asset::io::file::FileAssetReader, prelude::*};
use camera::{CAMERA_CONFIG, CameraConfig, CameraPlugin};
use gta_sim::{
    compose_sim,
    config::{ConfigRoot, load_config},
    world::WorldSource,
};
use input::PlayerInputPlugin;
use menu::MenuPlugin;
use std::time::{SystemTime, UNIX_EPOCH};
use visuals::{RENDER_CONFIG, RenderConfig, VisualsPlugin};

/// `--seed N` from the command line; without the flag a seed is taken from the clock.
fn parse_seed() -> Result<u64, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg != "--seed" {
            continue;
        }
        let value = args.next().ok_or("--seed needs a value")?;
        return value
            .parse()
            .map_err(|_| format!("--seed expects an unsigned integer, got {value:?}"));
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    Ok(nanos as u64)
}

fn main() -> AppExit {
    let seed = match parse_seed() {
        Ok(seed) => seed,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    let root = ConfigRoot(FileAssetReader::get_base_path().join("assets"));
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "GTA-like".into(),
            ..default()
        }),
        ..default()
    }));
    info!("city seed {seed}");
    if let Err(error) = compose_sim(&mut app, root.clone(), WorldSource::City { seed }) {
        eprintln!("{error}");
        return AppExit::error();
    }
    let camera_config = match load_config::<CameraConfig>(&root, CAMERA_CONFIG) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    let render_config = match load_config::<RenderConfig>(&root, RENDER_CONFIG) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    app.insert_resource(camera_config)
        .insert_resource(render_config)
        .add_plugins((
            bevy_enhanced_input::prelude::EnhancedInputPlugin,
            PlayerInputPlugin,
            CameraPlugin,
            VisualsPlugin,
            MenuPlugin,
        ));
    #[cfg(feature = "dev")]
    app.add_plugins(remote::QaRemotePlugin);
    #[cfg(feature = "debug")]
    app.add_plugins(debug::DebugToolsPlugin);
    app.run()
}
