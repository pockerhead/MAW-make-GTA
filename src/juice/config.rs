use bevy::prelude::Resource;
use gta_sim::combat::Weapon;
use serde::Deserialize;

pub const JUICE_CONFIG: &str = "juice/juice.ron";

type Rgb = (f32, f32, f32);

/// Shot feedback: recoil, muzzle flash, tracer and floating damage numbers (GDD §8).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct JuiceConfig {
    pub recoil_deg: PerWeapon,
    /// Seconds for the camera kick to halve.
    pub recoil_half_life: f32,
    /// Real seconds the attacker's and target's animations freeze on a melee hit.
    pub hit_stop_seconds: f32,
    pub shake: ShakeConfig,
    pub flash: FlashConfig,
    pub tracer: TracerConfig,
    pub damage_numbers: DamageNumbersConfig,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct PerWeapon {
    pub pistol: f32,
    pub smg: f32,
    pub shotgun: f32,
}

impl PerWeapon {
    pub fn get(&self, weapon: Weapon) -> f32 {
        match weapon {
            Weapon::Pistol => self.pistol,
            Weapon::Smg => self.smg,
            Weapon::Shotgun => self.shotgun,
        }
    }
}

/// Trauma camera shake: rotation `max · trauma² · noise`, trauma decays on real time.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShakeConfig {
    /// Trauma added by a melee hit the player lands or takes (0..1].
    pub melee_trauma: f32,
    /// Trauma lost per real second.
    pub decay_per_s: f32,
    pub max_yaw_deg: f32,
    pub max_pitch_deg: f32,
    pub max_roll_deg: f32,
    /// Noise lattice steps per second.
    pub noise_hz: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct FlashConfig {
    pub seconds: f32,
    /// Diameter of the flash sphere, m.
    pub size: f32,
    pub color: Rgb,
    /// Point light intensity, lumens.
    pub light_intensity: f32,
    pub light_range: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct TracerConfig {
    pub seconds: f32,
    /// Thickness, m.
    pub width: f32,
    pub color: Rgb,
}

/// Floating damage numbers; times in real seconds, distances in logical pixels.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct DamageNumbersConfig {
    pub lifetime: f32,
    /// Seconds the scale eases from `pop_start_scale` to 1.
    pub pop_seconds: f32,
    pub pop_start_scale: f32,
    /// Total upward travel (ease-out cubic).
    pub rise_px: f32,
    /// Largest sideways travel (linear).
    pub drift_px: f32,
    /// Fraction of the lifetime after which the alpha falls to 0.
    pub fade_start: f32,
    /// Font size of a body hit.
    pub size: f32,
    /// Font size of a headshot.
    pub crit_size: f32,
    pub color: Rgb,
    pub crit_color: Rgb,
}

fn positive(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{field} must be a finite number > 0, got {value}"))
    }
}

fn non_negative(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(format!("{field} must be a finite number >= 0, got {value}"))
    }
}

fn unit_rgb(field: &str, (r, g, b): Rgb) -> Result<(), String> {
    if [r, g, b].iter().all(|v| (0.0..=1.0).contains(v)) {
        Ok(())
    } else {
        Err(format!("{field} components must be in [0, 1]"))
    }
}

impl JuiceConfig {
    pub fn validate(&self) -> Result<(), String> {
        let r = &self.recoil_deg;
        non_negative("recoil_deg.pistol", r.pistol)?;
        non_negative("recoil_deg.smg", r.smg)?;
        non_negative("recoil_deg.shotgun", r.shotgun)?;
        positive("recoil_half_life", self.recoil_half_life)?;
        positive("hit_stop_seconds", self.hit_stop_seconds)?;
        self.shake.validate()?;
        let f = &self.flash;
        positive("flash.seconds", f.seconds)?;
        positive("flash.size", f.size)?;
        unit_rgb("flash.color", f.color)?;
        non_negative("flash.light_intensity", f.light_intensity)?;
        positive("flash.light_range", f.light_range)?;
        let t = &self.tracer;
        positive("tracer.seconds", t.seconds)?;
        positive("tracer.width", t.width)?;
        unit_rgb("tracer.color", t.color)?;
        self.damage_numbers.validate()
    }
}

impl ShakeConfig {
    fn validate(&self) -> Result<(), String> {
        positive("shake.melee_trauma", self.melee_trauma)?;
        if self.melee_trauma > 1.0 {
            return Err(format!(
                "shake.melee_trauma must be <= 1, got {}",
                self.melee_trauma
            ));
        }
        positive("shake.decay_per_s", self.decay_per_s)?;
        non_negative("shake.max_yaw_deg", self.max_yaw_deg)?;
        non_negative("shake.max_pitch_deg", self.max_pitch_deg)?;
        non_negative("shake.max_roll_deg", self.max_roll_deg)?;
        positive("shake.noise_hz", self.noise_hz)
    }
}

impl DamageNumbersConfig {
    fn validate(&self) -> Result<(), String> {
        positive("damage_numbers.lifetime", self.lifetime)?;
        positive("damage_numbers.pop_seconds", self.pop_seconds)?;
        if self.pop_seconds >= self.lifetime {
            return Err("damage_numbers.pop_seconds must be < lifetime".into());
        }
        positive("damage_numbers.pop_start_scale", self.pop_start_scale)?;
        non_negative("damage_numbers.rise_px", self.rise_px)?;
        non_negative("damage_numbers.drift_px", self.drift_px)?;
        if !(0.0..1.0).contains(&self.fade_start) {
            return Err("damage_numbers.fade_start must be in [0, 1)".into());
        }
        positive("damage_numbers.size", self.size)?;
        positive("damage_numbers.crit_size", self.crit_size)?;
        if self.crit_size < self.size {
            return Err("damage_numbers.crit_size must be >= size".into());
        }
        unit_rgb("damage_numbers.color", self.color)?;
        unit_rgb("damage_numbers.crit_color", self.crit_color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gta_sim::config::{ConfigRoot, load_config};
    use std::path::Path;

    fn shipped() -> JuiceConfig {
        let root = ConfigRoot(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
        load_config::<JuiceConfig>(&root, JUICE_CONFIG)
            .unwrap_or_else(|e| panic!("GATE BROKEN: {e}"))
    }

    #[test]
    fn shipped_juice_validates() {
        shipped().validate().unwrap();
    }

    #[test]
    fn hit_stop_must_be_positive() {
        let mut cfg = shipped();
        cfg.hit_stop_seconds = 0.0;
        let error = cfg.validate().unwrap_err();
        assert!(error.contains("hit_stop_seconds"), "{error}");
    }
}
