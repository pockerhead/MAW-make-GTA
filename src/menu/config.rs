use bevy::prelude::*;
use serde::Deserialize;

pub const UI_CONFIG: &str = "ui/strings.ron";

type Rgb = (f32, f32, f32);
type Rgba = (f32, f32, f32, f32);

/// On-screen texts, fonts and HUD layout (Q1: HUD numbers live here, no separate hud.ron).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct UiConfig {
    /// Asset path of the body font (a manifest file).
    pub font: String,
    /// Asset path of the title font (a manifest file).
    pub title_font: String,
    /// Loading screen text; `{seed}` is replaced with the city seed.
    pub loading: String,
    pub loading_size: f32,
    pub wasted: String,
    pub wasted_size: f32,
    pub wasted_color: Rgb,
    pub wasted_backdrop: Rgba,
    /// Colour grading post-saturation of the 3D view while wasted.
    pub wasted_saturation: f32,
    /// Label of a headshot damage number; `{damage}` is replaced with the value.
    pub damage_crit: String,
    pub hud: HudLayout,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HudLayout {
    pub margin: f32,
    pub bar_width: f32,
    pub bar_height: f32,
    pub bar_gap: f32,
    pub health_color: Rgb,
    pub armor_color: Rgb,
    pub back_color: Rgba,
    pub ammo_size: f32,
    pub ammo_color: Rgb,
    /// Crosshair dot edge, arm length and thickness, px.
    pub crosshair_dot: f32,
    pub crosshair_arm: f32,
    pub crosshair_thickness: f32,
    pub crosshair_color: Rgba,
    pub hit_marker_size: f32,
    /// Real seconds the hit marker stays on screen (GDD §7).
    pub hit_marker_seconds: f32,
    pub hit_marker_color: Rgb,
    pub kill_marker_color: Rgb,
    pub witness_bar: WitnessBarConfig,
    pub stars: StarsConfig,
}

/// Progress bar over a civilian in `Report` (GDD §6.2).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct WitnessBarConfig {
    /// Size of the full bar, px.
    pub width: f32,
    pub height: f32,
    /// Bar anchor above the model's head top, m.
    pub head_offset: f32,
    pub fill_color: Rgb,
    pub back_color: Rgba,
}

/// Wanted stars under the ammo counter (GDD §6.4, §7).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct StarsConfig {
    /// Text of one star; the font must have the glyph.
    pub glyph: String,
    /// Font size, px.
    pub size: f32,
    /// Gap between stars, px.
    pub gap: f32,
    /// An earned star while a cop sees the player.
    pub lit_color: Rgb,
    /// The bright phase of an earned star's blink while no cop sees the player.
    pub gray_color: Rgb,
    /// The dark phase of that blink: still an earned star, never the empty slot's look.
    pub dim_color: Rgb,
    /// A star not earned.
    pub off_color: Rgba,
    /// Drop shadow of an earned star (every phase), px down-right; empty slots have none.
    pub shadow_offset: f32,
    pub shadow_color: Rgba,
    /// Real seconds of one blink phase.
    pub blink_seconds: f32,
}

fn positive(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{field} must be a finite number > 0, got {value}"))
    }
}

fn unit(field: &str, values: &[f32]) -> Result<(), String> {
    if values.iter().all(|v| (0.0..=1.0).contains(v)) {
        Ok(())
    } else {
        Err(format!(
            "{field} components must be in [0, 1], got {values:?}"
        ))
    }
}

impl UiConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("font", &self.font),
            ("title_font", &self.title_font),
            ("loading", &self.loading),
            ("wasted", &self.wasted),
            ("damage_crit", &self.damage_crit),
        ] {
            if value.is_empty() {
                return Err(format!("{field} is empty"));
            }
        }
        if !self.loading.contains("{seed}") {
            return Err("loading must contain {seed}".into());
        }
        if !self.damage_crit.contains("{damage}") {
            return Err("damage_crit must contain {damage}".into());
        }
        let hud = &self.hud;
        positive("loading_size", self.loading_size)?;
        positive("wasted_size", self.wasted_size)?;
        positive("hud.margin", hud.margin)?;
        positive("hud.bar_width", hud.bar_width)?;
        positive("hud.bar_height", hud.bar_height)?;
        positive("hud.bar_gap", hud.bar_gap)?;
        positive("hud.ammo_size", hud.ammo_size)?;
        positive("hud.crosshair_dot", hud.crosshair_dot)?;
        positive("hud.crosshair_arm", hud.crosshair_arm)?;
        positive("hud.crosshair_thickness", hud.crosshair_thickness)?;
        positive("hud.hit_marker_size", hud.hit_marker_size)?;
        positive("hud.hit_marker_seconds", hud.hit_marker_seconds)?;
        let witness = &hud.witness_bar;
        positive("hud.witness_bar.width", witness.width)?;
        positive("hud.witness_bar.height", witness.height)?;
        if !(witness.head_offset.is_finite() && witness.head_offset >= 0.0) {
            return Err(format!(
                "hud.witness_bar.head_offset must be a finite number >= 0, got {}",
                witness.head_offset
            ));
        }
        let stars = &hud.stars;
        if stars.glyph.is_empty() {
            return Err("hud.stars.glyph is empty".into());
        }
        positive("hud.stars.size", stars.size)?;
        positive("hud.stars.blink_seconds", stars.blink_seconds)?;
        if !(stars.gap.is_finite() && stars.gap >= 0.0) {
            return Err(format!(
                "hud.stars.gap must be a finite number >= 0, got {}",
                stars.gap
            ));
        }
        for (field, (r, g, b)) in [
            ("hud.stars.lit_color", stars.lit_color),
            ("hud.stars.gray_color", stars.gray_color),
            ("hud.stars.dim_color", stars.dim_color),
        ] {
            unit(field, &[r, g, b])?;
        }
        let (r, g, b, a) = stars.off_color;
        unit("hud.stars.off_color", &[r, g, b, a])?;
        if !(stars.shadow_offset.is_finite() && stars.shadow_offset >= 0.0) {
            return Err(format!(
                "hud.stars.shadow_offset must be a finite number >= 0, got {}",
                stars.shadow_offset
            ));
        }
        let (r, g, b, a) = stars.shadow_color;
        unit("hud.stars.shadow_color", &[r, g, b, a])?;
        let (r, g, b) = witness.fill_color;
        unit("hud.witness_bar.fill_color", &[r, g, b])?;
        let (r, g, b, a) = witness.back_color;
        unit("hud.witness_bar.back_color", &[r, g, b, a])?;
        if !(self.wasted_saturation.is_finite() && self.wasted_saturation >= 0.0) {
            return Err(format!(
                "wasted_saturation must be a finite number >= 0, got {}",
                self.wasted_saturation
            ));
        }
        let (r, g, b) = self.wasted_color;
        unit("wasted_color", &[r, g, b])?;
        let (r, g, b, a) = self.wasted_backdrop;
        unit("wasted_backdrop", &[r, g, b, a])?;
        let (r, g, b) = hud.health_color;
        unit("hud.health_color", &[r, g, b])?;
        let (r, g, b) = hud.armor_color;
        unit("hud.armor_color", &[r, g, b])?;
        let (r, g, b, a) = hud.back_color;
        unit("hud.back_color", &[r, g, b, a])?;
        let (r, g, b, a) = hud.crosshair_color;
        unit("hud.crosshair_color", &[r, g, b, a])?;
        for (field, (r, g, b)) in [
            ("hud.ammo_color", hud.ammo_color),
            ("hud.hit_marker_color", hud.hit_marker_color),
            ("hud.kill_marker_color", hud.kill_marker_color),
        ] {
            unit(field, &[r, g, b])?;
        }
        Ok(())
    }

    /// Font asset paths, each of which must be a third-party manifest file.
    pub fn font_paths(&self) -> [&str; 2] {
        [&self.font, &self.title_font]
    }
}
