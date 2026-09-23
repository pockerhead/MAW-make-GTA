//! Placeholder gunshot: a decaying noise burst per weapon (GDD §8; real sounds come in T13).

use bevy::{
    audio::{AddAudioSource, ChannelCount, Decodable, SampleRate, Source, Volume},
    prelude::*,
};
use gta_sim::combat::{ShotFired, Weapon};
use serde::Deserialize;
use std::time::Duration;

pub const MIX_CONFIG: &str = "audio/mix.ron";

const SAMPLE_RATE: u32 = 44_100;

#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MixConfig {
    pub shot: ShotMix,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShotMix {
    pub pistol: ShotSoundConfig,
    pub smg: ShotSoundConfig,
    pub shotgun: ShotSoundConfig,
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShotSoundConfig {
    /// Burst length, s.
    pub seconds: f32,
    /// Envelope decay rate, 1/s.
    pub decay: f32,
    /// Linear volume.
    pub volume: f32,
}

impl MixConfig {
    pub fn validate(&self) -> Result<(), String> {
        for (name, s) in [
            ("shot.pistol", self.shot.pistol),
            ("shot.smg", self.shot.smg),
            ("shot.shotgun", self.shot.shotgun),
        ] {
            for (field, value) in [
                ("seconds", s.seconds),
                ("decay", s.decay),
                ("volume", s.volume),
            ] {
                if !(value.is_finite() && value > 0.0) {
                    return Err(format!(
                        "{name}.{field} must be a finite number > 0, got {value}"
                    ));
                }
            }
        }
        Ok(())
    }

    fn sound(&self, weapon: Weapon) -> ShotSoundConfig {
        match weapon {
            Weapon::Pistol => self.shot.pistol,
            Weapon::Smg => self.shot.smg,
            Weapon::Shotgun => self.shot.shotgun,
        }
    }
}

#[derive(Asset, TypePath)]
struct ShotSound {
    seconds: f32,
    decay: f32,
}

/// White noise (xorshift32) under an exponential envelope, mono.
struct NoiseBurstDecoder {
    sample: u32,
    samples: u32,
    decay: f32,
    state: u32,
}

impl Iterator for NoiseBurstDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.sample >= self.samples {
            return None;
        }
        let t = self.sample as f32 / SAMPLE_RATE as f32;
        self.sample += 1;
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        let noise = self.state as f32 / u32::MAX as f32 * 2.0 - 1.0;
        Some(noise * (-self.decay * t).exp())
    }
}

impl Source for NoiseBurstDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.samples as f32 / SAMPLE_RATE as f32,
        ))
    }
}

impl Decodable for ShotSound {
    type Decoder = NoiseBurstDecoder;

    fn decoder(&self) -> NoiseBurstDecoder {
        NoiseBurstDecoder {
            sample: 0,
            samples: (self.seconds * SAMPLE_RATE as f32) as u32,
            decay: self.decay,
            state: 0x9E37_79B9,
        }
    }
}

#[derive(Resource)]
struct ShotSounds([Handle<ShotSound>; 3]);

pub struct ShotAudioPlugin;

impl Plugin for ShotAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<ShotSound>()
            .add_systems(Startup, create_sounds)
            .add_systems(Update, play_shots);
    }
}

fn create_sounds(
    mut commands: Commands,
    mix: Res<MixConfig>,
    mut sounds: ResMut<Assets<ShotSound>>,
) {
    let handles = Weapon::ALL.map(|weapon| {
        let s = mix.sound(weapon);
        sounds.add(ShotSound {
            seconds: s.seconds,
            decay: s.decay,
        })
    });
    commands.insert_resource(ShotSounds(handles));
}

fn play_shots(
    mut commands: Commands,
    mut shots: MessageReader<ShotFired>,
    mix: Res<MixConfig>,
    sounds: Option<Res<ShotSounds>>,
) {
    let Some(sounds) = sounds else {
        return;
    };
    for shot in shots.read() {
        let volume = mix.sound(shot.weapon).volume;
        commands.spawn((
            AudioPlayer(sounds.0[shot.weapon.index()].clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
        ));
    }
}
