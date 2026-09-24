mod audio;
mod camera;
#[cfg(feature = "debug")]
mod debug;
mod hud;
mod input;
mod juice;
mod menu;
mod minimap;
#[cfg(feature = "dev")]
mod remote;
mod settings;
mod vfx;
mod visuals;

use audio::{GameAudioPlugin, MIX_CONFIG, MixConfig};
use bevy::{asset::io::file::FileAssetReader, prelude::*};
use camera::{CAMERA_CONFIG, CameraConfig, CameraPlugin};
use gta_sim::{
    compose_sim,
    config::{
        ConfigRoot, load_config,
        manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest},
    },
    flow::GameState,
    world::WorldSource,
};
use input::PlayerInputPlugin;
use juice::{JUICE_CONFIG, JuiceConfig, JuicePlugin};
use menu::{MenuPlugin, UI_CONFIG, UiConfig, clock_seed};
use settings::{GameSettingsPlugin, SETTINGS_APP_ID};
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

/// `--seed N` from the command line; `None` without the flag (the game starts in the main menu).
fn cli_seed() -> Result<Option<u64>, String> {
    let Some(value) = flag_value("--seed")? else {
        return Ok(None);
    };
    value
        .parse()
        .map(Some)
        .map_err(|_| format!("--seed expects an unsigned integer, got {value:?}"))
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
    for gang in &character_config.gang_models {
        if !manifest.contains_asset(gang) {
            unlisted.push(format!(
                "{CHARACTER_VISUAL_CONFIG}: gang model {gang} is not listed in {THIRD_PARTY_MANIFEST}"
            ));
        }
    }
    for police in &character_config.police_models {
        if !manifest.contains_asset(police) {
            unlisted.push(format!(
                "{CHARACTER_VISUAL_CONFIG}: police model {police} is not listed in {THIRD_PARTY_MANIFEST}"
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
    if let Err(errors) = mix_config.check_sounds(&manifest) {
        unlisted.extend(errors);
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
    let cli = match cli_seed() {
        Ok(seed) => seed,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::error();
        }
    };
    let seed = cli.unwrap_or_else(clock_seed);
    let settings_id = match flag_value("--settings-id") {
        Ok(id) => id.unwrap_or_else(|| SETTINGS_APP_ID.into()),
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
    if cli.is_none() {
        app.insert_state(GameState::MainMenu);
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
            GameSettingsPlugin {
                app_id: settings_id,
            },
            bevy_enhanced_input::prelude::EnhancedInputPlugin,
            PlayerInputPlugin,
            CameraPlugin,
            VisualsPlugin,
            MenuPlugin,
            hud::HudPlugin,
            JuicePlugin,
            vfx::VfxPlugin,
            GameAudioPlugin,
            minimap::MinimapPlugin,
        ));
    #[cfg(feature = "dev")]
    app.add_plugins(remote::QaRemotePlugin);
    #[cfg(feature = "debug")]
    app.add_plugins(debug::DebugToolsPlugin);
    app.run()
}
