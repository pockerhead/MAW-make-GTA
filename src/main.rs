mod camera;
#[cfg(feature = "debug")]
mod debug;
mod input;
#[cfg(feature = "dev")]
mod remote;
mod visuals;

use bevy::{asset::io::file::FileAssetReader, prelude::*};
use camera::{CAMERA_CONFIG, CameraConfig, CameraPlugin};
use gta_sim::{
    compose_sim,
    config::{ConfigRoot, load_config},
};
use input::PlayerInputPlugin;
use visuals::{RENDER_CONFIG, RenderConfig, VisualsPlugin};

fn main() -> AppExit {
    let root = ConfigRoot(FileAssetReader::get_base_path().join("assets"));
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "GTA-like".into(),
            ..default()
        }),
        ..default()
    }));
    if let Err(error) = compose_sim(&mut app, root.clone()) {
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
        ));
    #[cfg(feature = "dev")]
    app.add_plugins(remote::QaRemotePlugin);
    #[cfg(feature = "debug")]
    app.add_plugins(debug::DebugToolsPlugin);
    app.run()
}
