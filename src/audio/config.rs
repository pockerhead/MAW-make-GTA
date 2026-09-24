//! `audio/mix.ron`: synth shapes, volumes, voice budget and the Kenney CC0 files of every cue (GDD §8).

use bevy::prelude::Resource;
use gta_sim::{
    combat::Weapon,
    config::manifest::{THIRD_PARTY_MANIFEST, ThirdPartyManifest},
};
use serde::Deserialize;

pub const MIX_CONFIG: &str = "audio/mix.ron";

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MixConfig {
    /// Distance between the camera listener's ears, m.
    pub listener_ear_gap: f32,
    /// Spatial one-shots quieter than this at the listener are not spawned, [0, 1].
    pub min_gain: f32,
    pub voices: Voices,
    pub shot: ShotMix,
    pub impacts: ImpactMix,
    pub hurt: HurtMix,
    pub interface: InterfaceMix,
    pub stinger: StingerMix,
    pub ambience: AmbienceConfig,
    pub siren: SirenConfig,
}

/// Alive one-shots per class; the oldest is stolen.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct Voices {
    pub shot: usize,
    pub impact: usize,
    pub hurt: usize,
    pub ui: usize,
    pub stinger: usize,
    pub death_sting: usize,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShotMix {
    /// Playback speeds, cycled by attack id.
    pub pitch_variants: Vec<f32>,
    /// Full volume around an NPC muzzle, m; `(ref/d)²` beyond.
    pub npc_ref_distance: f32,
    pub pistol: ShotSoundConfig,
    pub smg: ShotSoundConfig,
    pub shotgun: ShotSoundConfig,
}

impl ShotMix {
    pub fn sound(&self, weapon: Weapon) -> ShotSoundConfig {
        match weapon {
            Weapon::Pistol => self.pistol,
            Weapon::Smg => self.smg,
            Weapon::Shotgun => self.shotgun,
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShotSoundConfig {
    /// Burst length, s.
    pub seconds: f32,
    /// Noise envelope decay rate, 1/s.
    pub decay: f32,
    /// Linear volume.
    pub volume: f32,
    /// Low sine body, Hz.
    pub body_hz: f32,
    /// Body envelope decay rate, 1/s.
    pub body_decay: f32,
    /// Body level relative to the noise.
    pub body_level: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ImpactMix {
    pub volume: f32,
    /// Full volume around the impact, m; `(ref/d)²` beyond.
    pub ref_distance: f32,
    pub bullet_body: Vec<String>,
    pub bullet_world: Vec<String>,
    pub punch: Vec<String>,
    pub heavy: Vec<String>,
    pub death: Vec<String>,
}

/// The player's own body taking damage: non-spatial.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HurtMix {
    pub volume: f32,
    /// Real seconds after a hurt cue during which further hits play no cue.
    pub min_interval: f32,
    pub pool: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct InterfaceMix {
    pub volume: f32,
    pub press: String,
    pub pause: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct StingerMix {
    pub volume: f32,
    /// Wanted level rises.
    pub wanted: String,
    /// The player dies.
    pub death: String,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct AmbienceConfig {
    pub city_volume: f32,
    pub park_volume: f32,
    /// Share of the city bed removed at park weight 1, [0, 1].
    pub city_duck_in_park: f32,
    /// Distance outside a park over which the park weight falls 1 → 0, m.
    pub park_fade: f32,
    /// One-pole low-pass of the city noise, Hz.
    pub city_lowpass_hz: f32,
    pub birds: BirdsConfig,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct BirdsConfig {
    /// Mean chirp starts per second.
    pub chirps_per_s: f32,
    /// A chirp sweeps from `hi_hz` down to `lo_hz`.
    pub lo_hz: f32,
    pub hi_hz: f32,
    pub chirp_seconds: f32,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct SirenConfig {
    pub volume: f32,
    /// Full volume around the cop, m; `(ref/d)²` beyond.
    pub ref_distance: f32,
    /// Cops that carry a siren at once.
    pub max_emitters: usize,
    /// Farther cops from the listener carry no siren, m.
    pub audible: f32,
    /// Real seconds between re-choosing which cops carry sirens.
    pub repick_seconds: f32,
    /// A siren moves to another cop only if that cop is closer to the listener by more than this, m.
    pub switch_margin: f32,
    /// Above the cop's body centre, m.
    pub height: f32,
    /// The wail sweeps between these, Hz.
    pub lo_hz: f32,
    pub hi_hz: f32,
    /// One up-and-down sweep, s.
    pub period: f32,
}

fn positive(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{field} must be a finite number > 0, got {value}"))
    }
}

fn unit(field: &str, value: f32) -> Result<(), String> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("{field} must be in [0, 1], got {value}"))
    }
}

fn volume(field: &str, value: f32) -> Result<(), String> {
    positive(field, value)?;
    if value > 1.0 {
        return Err(format!("{field} must be <= 1, got {value}"));
    }
    Ok(())
}

fn below(lo_field: &str, lo: f32, hi_field: &str, hi: f32) -> Result<(), String> {
    if lo < hi {
        Ok(())
    } else {
        Err(format!("{lo_field} must be < {hi_field}, got {lo} >= {hi}"))
    }
}

impl MixConfig {
    pub fn validate(&self) -> Result<(), String> {
        positive("listener_ear_gap", self.listener_ear_gap)?;
        unit("min_gain", self.min_gain)?;
        let v = &self.voices;
        for (field, count) in [
            ("voices.shot", v.shot),
            ("voices.impact", v.impact),
            ("voices.hurt", v.hurt),
            ("voices.ui", v.ui),
            ("voices.stinger", v.stinger),
            ("voices.death_sting", v.death_sting),
        ] {
            if count < 1 {
                return Err(format!("{field} must be >= 1, got {count}"));
            }
        }
        let shot = &self.shot;
        if shot.pitch_variants.is_empty() {
            return Err("shot.pitch_variants must not be empty".into());
        }
        for speed in &shot.pitch_variants {
            if !(speed.is_finite() && *speed > 0.5 && *speed <= 2.0) {
                return Err(format!(
                    "shot.pitch_variants must be in (0.5, 2], got {speed}"
                ));
            }
        }
        positive("shot.npc_ref_distance", shot.npc_ref_distance)?;
        for (name, s) in [
            ("shot.pistol", shot.pistol),
            ("shot.smg", shot.smg),
            ("shot.shotgun", shot.shotgun),
        ] {
            for (field, value) in [
                ("seconds", s.seconds),
                ("decay", s.decay),
                ("body_hz", s.body_hz),
                ("body_decay", s.body_decay),
                ("body_level", s.body_level),
            ] {
                positive(&format!("{name}.{field}"), value)?;
            }
            volume(&format!("{name}.volume"), s.volume)?;
        }
        volume("impacts.volume", self.impacts.volume)?;
        positive("impacts.ref_distance", self.impacts.ref_distance)?;
        volume("hurt.volume", self.hurt.volume)?;
        positive("hurt.min_interval", self.hurt.min_interval)?;
        volume("interface.volume", self.interface.volume)?;
        volume("stinger.volume", self.stinger.volume)?;
        let a = &self.ambience;
        volume("ambience.city_volume", a.city_volume)?;
        volume("ambience.park_volume", a.park_volume)?;
        unit("ambience.city_duck_in_park", a.city_duck_in_park)?;
        positive("ambience.park_fade", a.park_fade)?;
        positive("ambience.city_lowpass_hz", a.city_lowpass_hz)?;
        let b = &a.birds;
        positive("ambience.birds.chirps_per_s", b.chirps_per_s)?;
        positive("ambience.birds.lo_hz", b.lo_hz)?;
        positive("ambience.birds.hi_hz", b.hi_hz)?;
        below("ambience.birds.lo_hz", b.lo_hz, "hi_hz", b.hi_hz)?;
        positive("ambience.birds.chirp_seconds", b.chirp_seconds)?;
        let s = &self.siren;
        volume("siren.volume", s.volume)?;
        positive("siren.ref_distance", s.ref_distance)?;
        if s.max_emitters < 1 {
            return Err(format!(
                "siren.max_emitters must be >= 1, got {}",
                s.max_emitters
            ));
        }
        positive("siren.audible", s.audible)?;
        positive("siren.repick_seconds", s.repick_seconds)?;
        positive("siren.switch_margin", s.switch_margin)?;
        if !(s.height.is_finite() && s.height >= 0.0) {
            return Err(format!(
                "siren.height must be a finite number >= 0, got {}",
                s.height
            ));
        }
        positive("siren.lo_hz", s.lo_hz)?;
        positive("siren.hi_hz", s.hi_hz)?;
        below("siren.lo_hz", s.lo_hz, "hi_hz", s.hi_hz)?;
        positive("siren.period", s.period)
    }

    /// Every sound file the mix plays, relative to the assets root.
    #[cfg(test)]
    pub fn sound_paths(&self) -> impl Iterator<Item = &str> {
        self.pools()
            .into_iter()
            .flat_map(|(_, pool)| pool.iter())
            .chain(self.singles().into_iter().map(|(_, path)| path))
            .map(String::as_str)
    }

    fn pools(&self) -> [(&'static str, &Vec<String>); 6] {
        let i = &self.impacts;
        [
            ("impacts.bullet_body", &i.bullet_body),
            ("impacts.bullet_world", &i.bullet_world),
            ("impacts.punch", &i.punch),
            ("impacts.heavy", &i.heavy),
            ("impacts.death", &i.death),
            ("hurt.pool", &self.hurt.pool),
        ]
    }

    fn singles(&self) -> [(&'static str, &String); 4] {
        [
            ("interface.press", &self.interface.press),
            ("interface.pause", &self.interface.pause),
            ("stinger.wanted", &self.stinger.wanted),
            ("stinger.death", &self.stinger.death),
        ]
    }

    /// Every sound is a non-empty pool of `.ogg` files listed in the third-party manifest.
    pub fn check_sounds(&self, manifest: &ThirdPartyManifest) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut check = |field: &str, path: &str| {
            if !path.ends_with(".ogg") {
                errors.push(format!("{MIX_CONFIG}: {field} {path} must be an .ogg file"));
            } else if !manifest.contains_asset(path) {
                errors.push(format!(
                    "{MIX_CONFIG}: {field} {path} is not listed in {THIRD_PARTY_MANIFEST}"
                ));
            }
        };
        for (field, pool) in self.pools() {
            for path in pool {
                check(field, path);
            }
        }
        for (field, path) in self.singles() {
            check(field, path);
        }
        for (field, pool) in self.pools() {
            if pool.is_empty() {
                errors.push(format!("{MIX_CONFIG}: {field} pool is empty"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
