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
    config::{
        ConfigRoot, load_config,
        manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest},
    },
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

/// Checks the render config and that every third-party asset it names is listed and present.
fn preflight(root: &ConfigRoot, render_config: &RenderConfig) -> Result<(), Vec<String>> {
    render_config
        .validate()
        .map_err(|message| vec![format!("{}: {message}", root.path(RENDER_CONFIG).display())])?;
    let manifest = load_config::<ThirdPartyManifest>(root, THIRD_PARTY_MANIFEST)
        .map_err(|error| vec![error.to_string()])?;
    manifest.validate().map_err(|message| {
        vec![format!(
            "{}: {message}",
            root.path(THIRD_PARTY_MANIFEST).display()
        )]
    })?;
    let unlisted = render_config
        .prop_asset_paths()
        .filter(|path| !manifest.contains_asset(path))
        .map(|path| {
            format!("{RENDER_CONFIG}: prop asset {path} is not listed in {THIRD_PARTY_MANIFEST}")
        })
        .collect::<Vec<_>>();
    if !unlisted.is_empty() {
        return Err(unlisted);
    }
    let missing = manifest.missing_files(root);
    if !missing.is_empty() {
        return Err(missing
            .iter()
            .map(|path| {
                format!(
                    "missing third-party asset {}; run `python tools/fetch_assets.py`",
                    path.display()
                )
            })
            .collect());
    }
    Ok(())
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
    if let Err(errors) = preflight(&root, &render_config) {
        for error in errors {
            eprintln!("{error}");
        }
        return AppExit::error();
    }
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
