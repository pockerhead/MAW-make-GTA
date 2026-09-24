use super::config::{Rgb, Rgba, positive, unit};
use serde::Deserialize;

/// Main menu, pause menu and settings screen: texts, layout and colours (GDD §7).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MenuConfig {
    pub title: String,
    pub new_game: String,
    pub seed_label: String,
    pub seed_hint: String,
    pub quit: String,
    pub paused: String,
    /// Seed line of the pause menu; `{seed}` is replaced with the city seed.
    pub current_seed: String,
    pub resume: String,
    pub new_city: String,
    pub settings: String,
    pub back: String,
    pub sensitivity: String,
    pub volume: String,
    pub invert_y: String,
    pub reduce_shake: String,
    pub reduce_camera_motion: String,
    pub no_flashes: String,
    pub on: String,
    pub off: String,
    /// Labels of the settings buttons: one step down, one step up, flip a toggle.
    pub step_down: String,
    pub step_up: String,
    pub toggle: String,
    /// Value texts of the settings screen; `{value}` is replaced with the number.
    pub sensitivity_value: String,
    pub volume_value: String,
    /// Font sizes and spacing, px.
    pub title_size: f32,
    pub item_size: f32,
    pub item_gap: f32,
    pub button_width: f32,
    pub button_height: f32,
    /// Backdrop of the pause menu over the frozen frame.
    pub backdrop: Rgba,
    /// Backdrop of the main menu (no city behind it).
    pub menu_backdrop: Rgba,
    pub button_color: Rgba,
    pub button_hover_color: Rgba,
    pub text_color: Rgb,
    pub field_color: Rgba,
    /// Mouse sensitivity multiplier: (min, max, step of one button press).
    pub sensitivity_range: (f32, f32, f32),
    /// Volume change of one button press (volume is 0..1).
    pub volume_step: f32,
}

impl MenuConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("menu.title", &self.title),
            ("menu.new_game", &self.new_game),
            ("menu.seed_label", &self.seed_label),
            ("menu.seed_hint", &self.seed_hint),
            ("menu.quit", &self.quit),
            ("menu.paused", &self.paused),
            ("menu.current_seed", &self.current_seed),
            ("menu.resume", &self.resume),
            ("menu.new_city", &self.new_city),
            ("menu.settings", &self.settings),
            ("menu.back", &self.back),
            ("menu.sensitivity", &self.sensitivity),
            ("menu.volume", &self.volume),
            ("menu.invert_y", &self.invert_y),
            ("menu.reduce_shake", &self.reduce_shake),
            ("menu.reduce_camera_motion", &self.reduce_camera_motion),
            ("menu.no_flashes", &self.no_flashes),
            ("menu.on", &self.on),
            ("menu.off", &self.off),
            ("menu.step_down", &self.step_down),
            ("menu.step_up", &self.step_up),
            ("menu.toggle", &self.toggle),
        ] {
            if value.is_empty() {
                return Err(format!("{field} is empty"));
            }
        }
        if !self.current_seed.contains("{seed}") {
            return Err("menu.current_seed must contain {seed}".into());
        }
        for (field, value) in [
            ("menu.sensitivity_value", &self.sensitivity_value),
            ("menu.volume_value", &self.volume_value),
        ] {
            if !value.contains("{value}") {
                return Err(format!("{field} must contain {{value}}"));
            }
        }
        positive("menu.title_size", self.title_size)?;
        positive("menu.item_size", self.item_size)?;
        positive("menu.item_gap", self.item_gap)?;
        positive("menu.button_width", self.button_width)?;
        positive("menu.button_height", self.button_height)?;
        for (field, (r, g, b, a)) in [
            ("menu.backdrop", self.backdrop),
            ("menu.menu_backdrop", self.menu_backdrop),
            ("menu.button_color", self.button_color),
            ("menu.button_hover_color", self.button_hover_color),
            ("menu.field_color", self.field_color),
        ] {
            unit(field, &[r, g, b, a])?;
        }
        let (r, g, b) = self.text_color;
        unit("menu.text_color", &[r, g, b])?;
        let (min, max, step) = self.sensitivity_range;
        positive("menu.sensitivity_range min", min)?;
        positive("menu.sensitivity_range step", step)?;
        if !(max.is_finite() && max > min) {
            return Err(format!(
                "menu.sensitivity_range max {max} must be finite and > min {min}"
            ));
        }
        positive("menu.volume_step", self.volume_step)?;
        if self.volume_step > 1.0 {
            return Err(format!(
                "menu.volume_step must be <= 1, got {}",
                self.volume_step
            ));
        }
        Ok(())
    }
}
