//! Player settings (GDD §7): neutral multipliers and flags over the base numbers in the RON configs,
//! persisted by `bevy::settings::SettingsPlugin` in the user's preferences directory.

use crate::menu::UiConfig;
use bevy::{
    audio::{GlobalVolume, Volume},
    prelude::*,
    settings::{ReflectSettingsGroup, SaveSettingsSync, SettingsGroup, SettingsPlugin},
    window::ExitSystems,
};

/// Reverse-domain id of the settings directory (identity, not tuning).
pub const SETTINGS_APP_ID: &str = "com.github.pockerhead.maw-make-gta";

#[derive(Resource, SettingsGroup, Reflect, Clone, Debug, PartialEq)]
#[reflect(Resource, SettingsGroup, Default)]
#[settings_group(group = "game")]
pub struct GameSettings {
    /// Multiplier of `camera.ron` mouse sensitivity.
    pub mouse_sensitivity: f32,
    /// Global volume, 0..1.
    pub volume: f32,
    pub invert_y: bool,
    /// Camera shake scaled by `juice.ron` `shake.reduced_scale`.
    pub reduce_shake: bool,
    /// No muzzle flashes (tracers stay) and no hurt vignette.
    pub no_flashes: bool,
    /// Recoil kick scaled by `juice.ron` `camera_motion_reduced_scale`.
    pub reduce_camera_motion: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 1.0,
            volume: 1.0,
            invert_y: false,
            reduce_shake: false,
            no_flashes: false,
            reduce_camera_motion: false,
        }
    }
}

pub struct GameSettingsPlugin {
    /// Settings directory id: `SETTINGS_APP_ID` unless `--settings-id` overrides it (QA, showcase).
    pub app_id: String,
}

impl Plugin for GameSettingsPlugin {
    fn build(&self, app: &mut App) {
        // `SettingsPlugin::build` scans the type registry for settings groups.
        app.register_type::<GameSettings>()
            .add_plugins(SettingsPlugin::new(&self.app_id))
            .add_systems(Startup, sanitize_settings)
            .add_systems(
                Update,
                apply_volume.run_if(resource_changed::<GameSettings>),
            )
            .add_systems(Last, save_on_exit.after(ExitSystems));
    }
}

/// A hand-edited file with bad values is clamped on load: non-finite values fall back to the
/// default, sensitivity to `[min, max]`, volume to `[0, 1]`.
pub fn sanitize(s: &GameSettings, (min, max, _): (f32, f32, f32)) -> GameSettings {
    let default = GameSettings::default();
    let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
    GameSettings {
        mouse_sensitivity: finite(s.mouse_sensitivity, default.mouse_sensitivity).clamp(min, max),
        volume: finite(s.volume, default.volume).clamp(0.0, 1.0),
        ..s.clone()
    }
}

// Written only when a value changes, so an untouched file is not rewritten on exit.
fn sanitize_settings(ui: Res<UiConfig>, mut settings: ResMut<GameSettings>) {
    let clean = sanitize(&settings, ui.menu.sensitivity_range);
    if clean != *settings {
        *settings = clean;
    }
}

// `GlobalVolume` applies when a sink is created: new sounds only (shots are one-shots).
fn apply_volume(settings: Res<GameSettings>, mut global: ResMut<GlobalVolume>) {
    global.volume = Volume::Linear(settings.volume);
}

fn save_on_exit(mut exits: MessageReader<AppExit>, mut commands: Commands) {
    if exits.read().next().is_some() {
        commands.queue(SaveSettingsSync::IfChanged);
    }
}

/// `value` moved by `steps` steps on the grid of `step`, clamped to `[min, max]`; snapping to the
/// grid keeps 0.7 from becoming 0.70000005 in the settings file.
pub fn step_value(value: f32, steps: i32, min: f32, max: f32, step: f32) -> f32 {
    (((value / step).round() + steps as f32) * step).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANGE: (f32, f32, f32) = (0.25, 3.0, 0.25);

    fn with(mouse_sensitivity: f32, volume: f32) -> GameSettings {
        GameSettings {
            mouse_sensitivity,
            volume,
            invert_y: true,
            ..default()
        }
    }

    #[test]
    fn sanitize_table() {
        let clean = |s: f32, v: f32| {
            let out = sanitize(&with(s, v), RANGE);
            assert!(out.invert_y, "flags are kept");
            (out.mouse_sensitivity, out.volume)
        };
        assert_eq!(clean(f32::NAN, 0.5).0, 1.0, "NaN sensitivity");
        assert_eq!(clean(1.5, f32::NAN).1, 1.0, "NaN volume");
        assert_eq!(clean(-1.0, 0.5).0, 0.25, "negative sensitivity");
        assert_eq!(clean(1.5, -1.0).1, 0.0, "negative volume");
        assert_eq!(clean(99.0, 0.5).0, 3.0, "huge sensitivity");
        assert_eq!(clean(1.5, 99.0).1, 1.0, "huge volume");
        assert_eq!(clean(1.5, 0.5).0, 1.5, "in-range sensitivity");
        assert_eq!(clean(1.5, 0.5).1, 0.5, "in-range volume");
    }

    #[test]
    fn step_value_table() {
        assert_eq!(step_value(1.0, 1, 0.25, 3.0, 0.25), 1.25, "up one");
        assert_eq!(
            step_value(0.5, -3, 0.25, 3.0, 0.25),
            0.25,
            "down to the min clamp"
        );
        assert_eq!(
            step_value(2.75, 3, 0.25, 3.0, 0.25),
            3.0,
            "up to the max clamp"
        );
        assert_eq!(step_value(0.33, 1, 0.25, 3.0, 0.25), 0.5, "off-grid start");
    }
}
