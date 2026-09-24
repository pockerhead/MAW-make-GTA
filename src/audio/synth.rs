//! Procedural sounds (GDD §8): gunshots (noise + low body), the siren wail, the city bed and park
//! birds. Loops are endless decoders played `Once`: rodio's `Loop` would buffer a source that never ends.

use super::config::{AmbienceConfig, BirdsConfig, ShotSoundConfig, SirenConfig};
use bevy::{
    audio::{ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
};
use std::{f32::consts::TAU, time::Duration};

const SAMPLE_RATE: u32 = 44_100;
const NOISE_SEED: u32 = 0x9E37_79B9;

#[derive(Asset, TypePath)]
pub enum Synth {
    Shot(ShotSynth),
    Siren(SirenSynth),
    City(CitySynth),
    Birds(BirdSynth),
}

#[derive(Clone, Copy)]
pub struct ShotSynth {
    seconds: f32,
    decay: f32,
    body_hz: f32,
    body_decay: f32,
    body_level: f32,
}

#[derive(Clone, Copy)]
pub struct SirenSynth {
    lo_hz: f32,
    hi_hz: f32,
    period: f32,
    /// Seconds into the sweep of the first sample.
    start: f32,
}

#[derive(Clone, Copy)]
pub struct CitySynth {
    lowpass_hz: f32,
}

#[derive(Clone, Copy)]
pub struct BirdSynth {
    chirps_per_s: f32,
    lo_hz: f32,
    hi_hz: f32,
    chirp_seconds: f32,
}

impl Synth {
    pub fn shot(cfg: &ShotSoundConfig) -> Self {
        Self::Shot(ShotSynth {
            seconds: cfg.seconds,
            decay: cfg.decay,
            body_hz: cfg.body_hz,
            body_decay: cfg.body_decay,
            body_level: cfg.body_level,
        })
    }

    pub fn siren(cfg: &SirenConfig, start: f32) -> Self {
        Self::Siren(SirenSynth {
            lo_hz: cfg.lo_hz,
            hi_hz: cfg.hi_hz,
            period: cfg.period,
            start: start % cfg.period,
        })
    }

    pub fn city(cfg: &AmbienceConfig) -> Self {
        Self::City(CitySynth {
            lowpass_hz: cfg.city_lowpass_hz,
        })
    }

    pub fn birds(cfg: &BirdsConfig) -> Self {
        Self::Birds(BirdSynth {
            chirps_per_s: cfg.chirps_per_s,
            lo_hz: cfg.lo_hz,
            hi_hz: cfg.hi_hz,
            chirp_seconds: cfg.chirp_seconds,
        })
    }

    /// Siren, city and birds never end.
    #[cfg(test)]
    pub fn is_endless(&self) -> bool {
        !matches!(self, Self::Shot(_))
    }
}

/// xorshift32 white noise in [-1, 1].
#[derive(Clone, Copy)]
pub struct Noise(u32);

impl Noise {
    fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    fn unit(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

pub enum SynthDecoder {
    Shot {
        synth: ShotSynth,
        sample: u32,
        samples: u32,
        noise: Noise,
    },
    Siren {
        synth: SirenSynth,
        t: f32,
        phase: f32,
    },
    City {
        a: f32,
        y: f32,
        noise: Noise,
    },
    Birds {
        synth: BirdSynth,
        noise: Noise,
        /// Seconds into the current chirp; `None` between chirps.
        chirp: Option<f32>,
        phase: f32,
    },
}

impl Decodable for Synth {
    type Decoder = SynthDecoder;

    fn decoder(&self) -> SynthDecoder {
        let noise = Noise(NOISE_SEED);
        match *self {
            Self::Shot(synth) => SynthDecoder::Shot {
                synth,
                sample: 0,
                samples: (synth.seconds * SAMPLE_RATE as f32) as u32,
                noise,
            },
            Self::Siren(synth) => SynthDecoder::Siren {
                synth,
                t: synth.start,
                phase: 0.0,
            },
            Self::City(synth) => SynthDecoder::City {
                a: 1.0 - (-TAU * synth.lowpass_hz / SAMPLE_RATE as f32).exp(),
                y: 0.0,
                noise,
            },
            Self::Birds(synth) => SynthDecoder::Birds {
                synth,
                noise,
                chirp: None,
                phase: 0.0,
            },
        }
    }
}

const DT: f32 = 1.0 / SAMPLE_RATE as f32;

impl Iterator for SynthDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        match self {
            Self::Shot {
                synth,
                sample,
                samples,
                noise,
            } => {
                if *sample >= *samples {
                    return None;
                }
                let t = *sample as f32 * DT;
                *sample += 1;
                let hiss = noise.signed() * (-synth.decay * t).exp();
                let body = synth.body_level
                    * (TAU * synth.body_hz * t).sin()
                    * (-synth.body_decay * t).exp();
                Some((hiss + body) / (1.0 + synth.body_level))
            }
            Self::Siren { synth, t, phase } => {
                let sweep = 0.5 - 0.5 * (TAU * *t / synth.period).cos();
                let f = synth.lo_hz + (synth.hi_hz - synth.lo_hz) * sweep;
                *phase = (*phase + TAU * f * DT) % TAU;
                *t = (*t + DT) % synth.period;
                Some(phase.sin())
            }
            Self::City { a, y, noise } => {
                *y += *a * (noise.signed() - *y);
                Some(*y)
            }
            Self::Birds {
                synth,
                noise,
                chirp,
                phase,
            } => {
                let start = noise.unit() < synth.chirps_per_s * DT;
                let Some(tau) = chirp.or(start.then_some(0.0)) else {
                    return Some(0.0);
                };
                let u = tau / synth.chirp_seconds;
                let f = synth.hi_hz + (synth.lo_hz - synth.hi_hz) * u;
                *phase = (*phase + TAU * f * DT) % TAU;
                let amplitude = 0.5 - 0.5 * (TAU * u).cos();
                *chirp = (tau + DT < synth.chirp_seconds).then_some(tau + DT);
                Some(amplitude * phase.sin())
            }
        }
    }
}

impl Source for SynthDecoder {
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
        match self {
            Self::Shot { samples, .. } => Some(Duration::from_secs_f32(
                *samples as f32 / SAMPLE_RATE as f32,
            )),
            _ => None,
        }
    }
}
