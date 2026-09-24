//! Game audio (GDD §8): procedural shots, siren and ambience, Kenney CC0 impacts, stingers and UI
//! sounds, spatial emitters heard by the camera, a per-class voice budget.

mod config;
mod cues;
#[cfg(test)]
mod event_gate;
#[cfg(test)]
mod gate;
mod loops;
mod synth;

pub use config::{MIX_CONFIG, MixConfig};

use bevy::{audio::AddAudioSource, prelude::*};

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        // `SoundCuesPlugin` creates `Synth` assets while it builds.
        app.add_audio_source::<synth::Synth>()
            .add_plugins((cues::SoundCuesPlugin, loops::SoundLoopsPlugin));
    }
}
