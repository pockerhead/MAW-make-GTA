// Release builds on Windows open no console window; their log goes to a file (`log_layer`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod bench;
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
use bevy::{
    asset::io::file::FileAssetReader,
    log::{
        BoxedFmtLayer, LogPlugin,
        tracing_subscriber::{Layer as _, fmt},
    },
    prelude::*,
    window::WindowResolution,
};
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
use std::{
    fs::File,
    io::Write as _,
    path::PathBuf,
    sync::{Arc, OnceLock},
};
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
        .chain(render_config.vehicle_asset_paths())
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

/// Release log: `gta_like.log` next to the exe, or in the temp dir when that folder is not writable.
static LOG_FILE: OnceLock<(PathBuf, Arc<File>)> = OnceLock::new();
const LOG_NAME: &str = "gta_like.log";

/// Creates (truncates) the release log before anything can fail, so a stale log never outlives a failed start.
fn open_log_file() {
    let beside_exe = std::env::current_exe()
        .ok()
        .map(|exe| exe.with_file_name(LOG_NAME));
    for path in beside_exe
        .into_iter()
        .chain([std::env::temp_dir().join(LOG_NAME)])
    {
        let Ok(file) = File::create(&path) else {
            continue;
        };
        let _ = LOG_FILE.set((path, Arc::new(file)));
        return;
    }
}

fn write_log(text: &str) {
    let Some((_, file)) = LOG_FILE.get() else {
        return;
    };
    let _ = writeln!(file.as_ref(), "{text}");
}

#[cfg(all(windows, not(debug_assertions)))]
fn error_dialog(text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    // The log keeps every line; a list of missing assets would not fit on the screen.
    const DIALOG_LINES: usize = 12;
    let mut shown = text
        .lines()
        .take(DIALOG_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    if text.lines().count() > DIALOG_LINES {
        shown.push_str("\n...");
    }
    let log = LOG_FILE
        .get()
        .map_or_else(|| LOG_NAME.into(), |(path, _)| path.display().to_string());
    let body = format!("{shown}\n\nЛог: {log}");
    let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (body, caption) = (wide(&body), wide("GTA-like"));
    // SAFETY: both buffers are NUL-terminated UTF-16 and outlive the call; a null owner window is allowed.
    unsafe { MessageBoxW(0, body.as_ptr(), caption.as_ptr(), MB_OK | MB_ICONERROR) };
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn error_dialog(_text: &str) {}

/// A start-up error: stderr, the log file and (Windows release, no console) a dialog.
fn fatal(error: impl std::fmt::Display) -> AppExit {
    let text = error.to_string();
    eprintln!("{text}");
    for line in text.lines() {
        write_log(&format!("ERROR {line}"));
    }
    error_dialog(&format!(
        "Игра не запустилась:\n\n{text}\n\nЕсли exe запущен прямо из zip, распакуйте архив целиком."
    ));
    AppExit::error()
}

/// Panics go to the log too; one on the main thread also shows the dialog (the window just vanished).
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>");
        let text = format!("thread '{name}' {info}");
        write_log(&text);
        if name == "main" {
            error_dialog(&text);
        }
    }));
}

/// Release builds log to the file first (plain) and then to stderr; debug builds keep the default stderr layer.
fn log_layer(_app: &mut App) -> Option<BoxedFmtLayer> {
    let (_, file) = LOG_FILE.get()?;
    let file = fmt::Layer::default()
        .with_ansi(false)
        .with_writer(file.clone());
    let console = fmt::Layer::default().with_writer(std::io::stderr);
    Some(Box::new(file.and_then(console)))
}

fn main() -> AppExit {
    if cfg!(not(debug_assertions)) {
        open_log_file();
        install_panic_hook();
    }
    let cli = match cli_seed() {
        Ok(seed) => seed,
        Err(error) => return fatal(error),
    };
    let seed = cli.unwrap_or_else(clock_seed);
    let settings_id = match flag_value("--settings-id") {
        Ok(id) => id.unwrap_or_else(|| SETTINGS_APP_ID.into()),
        Err(error) => return fatal(error),
    };
    let title = match flag_value("--window-title") {
        Ok(title) => title.unwrap_or_else(|| "GTA-like".into()),
        Err(error) => return fatal(error),
    };
    let bench = std::env::args().any(|a| a == "--bench-scene");
    let root = ConfigRoot(FileAssetReader::get_base_path().join("assets"));
    let render_config = match load_config::<RenderConfig>(&root, RENDER_CONFIG) {
        Ok(config) => config,
        Err(error) => return fatal(error),
    };
    let mut window = Window { title, ..default() };
    if bench {
        let (width, height) = render_config.bench().resolution;
        window.resolution = WindowResolution::new(width, height).with_scale_factor_override(1.0);
    }
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(window),
                ..default()
            })
            .set(LogPlugin {
                fmt_layer: log_layer,
                ..default()
            }),
    );
    if let Some((path, _)) = LOG_FILE.get()
        && std::env::current_exe().is_ok_and(|exe| exe.with_file_name(LOG_NAME) != *path)
    {
        warn!(
            "the exe folder is not writable; log file: {}",
            path.display()
        );
    }
    info!("city seed {seed}");
    if let Err(error) = compose_sim(&mut app, root.clone(), WorldSource::City { seed }) {
        return fatal(error);
    }
    if cli.is_none() && !bench {
        app.insert_state(GameState::MainMenu);
    }
    let camera_config = match load_config::<CameraConfig>(&root, CAMERA_CONFIG) {
        Ok(config) => config,
        Err(error) => return fatal(error),
    };
    let character_config =
        match load_config::<CharacterVisualConfig>(&root, CHARACTER_VISUAL_CONFIG) {
            Ok(config) => config,
            Err(error) => return fatal(error),
        };
    let ui_config = match load_config::<UiConfig>(&root, UI_CONFIG) {
        Ok(config) => config,
        Err(error) => return fatal(error),
    };
    let juice_config = match load_config::<JuiceConfig>(&root, JUICE_CONFIG) {
        Ok(config) => config,
        Err(error) => return fatal(error),
    };
    let mix_config = match load_config::<MixConfig>(&root, MIX_CONFIG) {
        Ok(config) => config,
        Err(error) => return fatal(error),
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
        Err(errors) => return fatal(errors.join("\n")),
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
    if bench {
        app.add_plugins(bench::BenchScenePlugin);
    }
    #[cfg(feature = "dev")]
    app.add_plugins(remote::QaRemotePlugin);
    #[cfg(feature = "debug")]
    app.add_plugins(debug::DebugToolsPlugin);
    app.run()
}
