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
    /// Share of the recoil kick left with the "reduce camera motion" setting, [0, 1].
    pub camera_motion_reduced_scale: f32,
    pub vignette: VignetteConfig,
    pub damage_arc: DamageArcConfig,
    pub star_pulse: StarPulseConfig,
    pub smoke: SmokeConfig,
}

/// Smoke from the hood of a wrecked car (health 0); real seconds.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct SmokeConfig {
    /// Seconds between puffs of one car.
    pub interval: f32,
    /// Lifetime of a puff; it rises and fades over it.
    pub seconds: f32,
    /// Diameter of a new puff, m.
    pub size: f32,
    /// Height a puff rises over its lifetime, m.
    pub rise: f32,
    /// sRGB colour and starting alpha.
    pub color: (f32, f32, f32, f32),
    /// Puff origin in the car body frame, m.
    pub hood: (f32, f32, f32),
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
    /// Share of the shake left with the "reduce shake" setting, [0, 1].
    pub reduced_scale: f32,
    /// Trauma added by a shot the player fires (0..1].
    pub shot_trauma: f32,
    /// Trauma added by each hit that lowers the player's health or armour (0..1].
    pub hurt_trauma: f32,
    /// Trauma added by a kill within `death_radius` of the player (0..1].
    pub death_trauma: f32,
    /// m.
    pub death_radius: f32,
    /// Trauma per m/s of a crash of the player's car (clamped at 1).
    pub crash_trauma_per_mps: f32,
    /// Crashes slower than this add no trauma, m/s.
    pub crash_min_speed: f32,
}

/// Red screen-edge vignette when the player is hurt; real seconds.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct VignetteConfig {
    pub color: Rgb,
    /// Intensity added per hurt.
    pub per_hurt: f32,
    /// Intensity cap, (0, 1].
    pub max: f32,
    /// Intensity lost per real second.
    pub decay_per_s: f32,
    /// Size of the clear centre (bevy `Vignette::radius`).
    pub radius: f32,
    /// Softness of the edge (bevy `Vignette::smoothness`).
    pub smoothness: f32,
}

/// Screen arc pointing at whoever hurt the player; real seconds, logical pixels.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct DamageArcConfig {
    /// Lifetime; the arc fades out over it.
    pub seconds: f32,
    /// Radius of the ring the arc sits on.
    pub radius_px: f32,
    pub thickness_px: f32,
    pub color: Rgb,
}

/// Wanted stars pulse when the level rises.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct StarPulseConfig {
    /// Scale at the start of the pulse, > 1.
    pub scale: f32,
    /// Real seconds back to scale 1.
    pub seconds: f32,
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
        self.damage_numbers.validate()?;
        if !(0.0..=1.0).contains(&self.camera_motion_reduced_scale) {
            return Err(format!(
                "camera_motion_reduced_scale must be in [0, 1], got {}",
                self.camera_motion_reduced_scale
            ));
        }
        let v = &self.vignette;
        unit_rgb("vignette.color", v.color)?;
        positive("vignette.per_hurt", v.per_hurt)?;
        trauma("vignette.max", v.max)?;
        positive("vignette.decay_per_s", v.decay_per_s)?;
        positive("vignette.radius", v.radius)?;
        positive("vignette.smoothness", v.smoothness)?;
        let a = &self.damage_arc;
        positive("damage_arc.seconds", a.seconds)?;
        positive("damage_arc.radius_px", a.radius_px)?;
        positive("damage_arc.thickness_px", a.thickness_px)?;
        unit_rgb("damage_arc.color", a.color)?;
        let p = &self.star_pulse;
        if !(p.scale.is_finite() && p.scale > 1.0) {
            return Err(format!("star_pulse.scale must be > 1, got {}", p.scale));
        }
        positive("star_pulse.seconds", p.seconds)?;
        let s = &self.smoke;
        positive("smoke.interval", s.interval)?;
        positive("smoke.seconds", s.seconds)?;
        positive("smoke.size", s.size)?;
        non_negative("smoke.rise", s.rise)?;
        let (r, g, b, a) = s.color;
        unit_rgb("smoke.color", (r, g, b))?;
        trauma("smoke.color alpha", a)?;
        let (x, y, z) = s.hood;
        if ![x, y, z].iter().all(|v| v.is_finite()) {
            return Err("smoke.hood must be finite".into());
        }
        Ok(())
    }
}

/// A finite value in (0, 1].
fn trauma(field: &str, value: f32) -> Result<(), String> {
    positive(field, value)?;
    if value > 1.0 {
        return Err(format!("{field} must be <= 1, got {value}"));
    }
    Ok(())
}

impl ShakeConfig {
    fn validate(&self) -> Result<(), String> {
        trauma("shake.melee_trauma", self.melee_trauma)?;
        trauma("shake.shot_trauma", self.shot_trauma)?;
        trauma("shake.hurt_trauma", self.hurt_trauma)?;
        trauma("shake.death_trauma", self.death_trauma)?;
        positive("shake.death_radius", self.death_radius)?;
        positive("shake.crash_trauma_per_mps", self.crash_trauma_per_mps)?;
        non_negative("shake.crash_min_speed", self.crash_min_speed)?;
        positive("shake.decay_per_s", self.decay_per_s)?;
        non_negative("shake.max_yaw_deg", self.max_yaw_deg)?;
        non_negative("shake.max_pitch_deg", self.max_pitch_deg)?;
        non_negative("shake.max_roll_deg", self.max_roll_deg)?;
        positive("shake.noise_hz", self.noise_hz)?;
        if !(0.0..=1.0).contains(&self.reduced_scale) {
            return Err(format!(
                "shake.reduced_scale must be in [0, 1], got {}",
                self.reduced_scale
            ));
        }
        Ok(())
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

    #[test]
    fn sabotaged_feedback_values_fail() {
        let fails = |keyword: &str, sabotage: fn(&mut JuiceConfig)| {
            let mut cfg = shipped();
            sabotage(&mut cfg);
            let error = cfg.validate().unwrap_err();
            assert!(error.contains(keyword), "{keyword}: {error}");
        };
        fails("vignette.max", |c| c.vignette.max = 1.5);
        fails("star_pulse.scale", |c| c.star_pulse.scale = 0.9);
        fails("shake.death_radius", |c| c.shake.death_radius = -1.0);
        fails("shake.crash_trauma_per_mps", |c| {
            c.shake.crash_trauma_per_mps = 0.0
        });
        fails("smoke.interval", |c| c.smoke.interval = 0.0);
        fails("smoke.color alpha", |c| c.smoke.color.3 = 1.5);
    }
}
