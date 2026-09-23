mod audio;
mod camera;
#[cfg(feature = "debug")]
mod debug;
mod hud;
mod input;
mod juice;
mod menu;
#[cfg(feature = "dev")]
mod remote;
mod vfx;
mod visuals;

use audio::{MIX_CONFIG, MixConfig, ShotAudioPlugin};
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
use juice::{JUICE_CONFIG, JuiceConfig, JuicePlugin};
use menu::{MenuPlugin, UI_CONFIG, UiConfig};
use std::time::{SystemTime, UNIX_EPOCH};
use visuals::{
    CHARACTER_VISUAL_CONFIG, CharacterClips, CharacterVisualConfig, RENDER_CONFIG, RenderConfig,
    VisualsPlugin,
};

/// The value after `flag` on the command line, `None` without the flag.
fn flag_value(flag: &str) -> Result<Option<String>, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg != flag {
            continue;
        }
        return args
            .next()
            .map(Some)
            .ok_or_else(|| format!("{flag} needs a value"));
    }
    Ok(None)
}

/// `--seed N` from the command line; without the flag a seed is taken from the clock.
fn parse_seed() -> Result<u64, String> {
    if let Some(value) = flag_value("--seed")? {
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

/// Checks the render, character and UI configs, that every third-party asset they name is listed
/// and present, and resolves the character clips against the manifest rig.
fn preflight(
    root: &ConfigRoot,
    render_config: &RenderConfig,
    character_config: &CharacterVisualConfig,
    ui_config: &UiConfig,
    feedback: (&CameraConfig, &JuiceConfig, &MixConfig),
) -> Result<CharacterClips, Vec<String>> {
    let (camera_config, juice_config, mix_config) = feedback;
    for (path, result) in [
        (CAMERA_CONFIG, camera_config.validate()),
        (JUICE_CONFIG, juice_config.validate()),
        (MIX_CONFIG, mix_config.validate()),
    ] {
        result.map_err(|message| vec![format!("{}: {message}", root.path(path).display())])?;
    }
    render_config
        .validate()
        .map_err(|message| vec![format!("{}: {message}", root.path(RENDER_CONFIG).display())])?;
    character_config.validate().map_err(|message| {
        vec![format!(
            "{}: {message}",
            root.path(CHARACTER_VISUAL_CONFIG).display()
        )]
    })?;
    ui_config
        .validate()
        .map_err(|message| vec![format!("{}: {message}", root.path(UI_CONFIG).display())])?;
    let manifest = load_config::<ThirdPartyManifest>(root, THIRD_PARTY_MANIFEST)
        .map_err(|error| vec![error.to_string()])?;
    manifest.validate().map_err(|message| {
        vec![format!(
            "{}: {message}",
            root.path(THIRD_PARTY_MANIFEST).display()
        )]
    })?;
    let mut unlisted = render_config
        .prop_asset_paths()
        .filter(|path| !manifest.contains_asset(path))
        .map(|path| {
            format!("{RENDER_CONFIG}: prop asset {path} is not listed in {THIRD_PARTY_MANIFEST}")
        })
        .collect::<Vec<_>>();
    let model = &character_config.model;
    if !manifest.contains_asset(model) {
        unlisted.push(format!(
            "{CHARACTER_VISUAL_CONFIG}: model {model} is not listed in {THIRD_PARTY_MANIFEST}"
        ));
    }
    for civilian in &character_config.civilian_models {
        if !manifest.contains_asset(civilian) {
            unlisted.push(format!(
                "{CHARACTER_VISUAL_CONFIG}: civilian model {civilian} is not listed in {THIRD_PARTY_MANIFEST}"
            ));
        }
    }
    for font in ui_config.font_paths() {
        if !manifest.contains_asset(font) {
            unlisted.push(format!(
                "{UI_CONFIG}: font {font} is not listed in {THIRD_PARTY_MANIFEST}"
            ));
        }
    }
    if !unlisted.is_empty() {
        return Err(unlisted);
    }
    let clips = character_config.resolve(&manifest).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| format!("{CHARACTER_VISUAL_CONFIG}: {error}"))
            .collect::<Vec<_>>()
    })?;
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
    Ok(clips)
}

fn main() -> AppExit {
    let seed = match parse_seed() {
        Ok(seed) => seed,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    let title = match flag_value("--window-title") {
        Ok(title) => title.unwrap_or_else(|| "GTA-like".into()),
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    let root = ConfigRoot(FileAssetReader::get_base_path().join("assets"));
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window { title, ..default() }),
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
    let character_config =
        match load_config::<CharacterVisualConfig>(&root, CHARACTER_VISUAL_CONFIG) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("{error}");
                return AppExit::error();
            }
        };
    let ui_config = match load_config::<UiConfig>(&root, UI_CONFIG) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    let juice_config = match load_config::<JuiceConfig>(&root, JUICE_CONFIG) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    let mix_config = match load_config::<MixConfig>(&root, MIX_CONFIG) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    let feedback = (&camera_config, &juice_config, &mix_config);
    let clips = match preflight(
        &root,
        &render_config,
        &character_config,
        &ui_config,
        feedback,
    ) {
        Ok(clips) => clips,
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            return AppExit::error();
        }
    };
    // CharacterAnimations (VisualsPlugin) and UiFonts (MenuPlugin) read their configs while the plugins build.
    app.insert_resource(camera_config)
        .insert_resource(render_config)
        .insert_resource(character_config)
        .insert_resource(clips)
        .insert_resource(ui_config)
        .insert_resource(juice_config)
        .insert_resource(mix_config)
        .add_plugins((
            bevy_enhanced_input::prelude::EnhancedInputPlugin,
            PlayerInputPlugin,
            CameraPlugin,
            VisualsPlugin,
            MenuPlugin,
            hud::HudPlugin,
            JuicePlugin,
            vfx::VfxPlugin,
            ShotAudioPlugin,
        ));
    #[cfg(feature = "dev")]
    app.add_plugins(remote::QaRemotePlugin);
    #[cfg(feature = "debug")]
    app.add_plugins(debug::DebugToolsPlugin);
    app.run()
}
