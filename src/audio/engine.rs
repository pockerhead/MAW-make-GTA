//! Engine hum of the player's car (GDD §8): one endless procedural emitter on the car the player
//! drives; its pitch follows the revs, its volume the revs or the throttle.

use super::{
    MixConfig,
    config::EngineConfig,
    cues::{SoundBank, SoundClass, SoundStats, spawn_sound},
};
use avian3d::prelude::LinearVelocity;
use bevy::{
    audio::{GlobalVolume, SpatialScale, Volume},
    prelude::*,
};
use gta_sim::{
    flow::GameState,
    player::Player,
    vehicle::{DriveIntent, Driving, VehicleConfig},
};

/// The engine sound, parented to the player's car.
#[derive(Component)]
pub struct EngineEmitter;

pub(super) struct EngineSoundPlugin;

impl Plugin for EngineSoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_engine_emitter, drive_engine_voice).chain().run_if(
                in_state(GameState::Playing)
                    .or_else(in_state(GameState::Wasted))
                    .or_else(in_state(GameState::Busted)),
            ),
        );
    }
}

/// `(playback speed, volume)` at `speed_ratio` (forward speed / max speed) and `throttle`.
pub fn engine_voice(speed_ratio: f32, throttle: f32, cfg: &EngineConfig) -> (f32, f32) {
    let rpm = speed_ratio
        .abs()
        .max(throttle.abs() * cfg.rev_share)
        .clamp(0.0, 1.0);
    let load = rpm.max(throttle.abs()).min(1.0);
    (
        cfg.idle_pitch.lerp(cfg.max_pitch, rpm),
        cfg.idle_volume.lerp(cfg.max_volume, load),
    )
}

/// One emitter on the car the player drives, none otherwise.
fn sync_engine_emitter(
    mut commands: Commands,
    mix: Res<MixConfig>,
    player: Query<&Driving, With<Player>>,
    emitters: Query<(Entity, &ChildOf), With<EngineEmitter>>,
    mut bank: ResMut<SoundBank>,
    mut stats: ResMut<SoundStats>,
) {
    let car = player.single().ok().map(|d| d.vehicle);
    let mut kept = false;
    for (emitter, parent) in &emitters {
        if Some(parent.parent()) == car && !kept {
            kept = true;
            continue;
        }
        commands.entity(emitter).try_despawn();
    }
    let Some(car) = car.filter(|_| !kept) else {
        return;
    };
    let settings = PlaybackSettings::ONCE
        .with_spatial(true)
        .with_spatial_scale(SpatialScale::new(1.0 / mix.engine.ref_distance))
        .with_volume(Volume::Linear(mix.engine.idle_volume));
    let source = AudioPlayer(bank.engine.clone());
    spawn_sound(
        &mut commands,
        &mut bank,
        &mut stats,
        SoundClass::Engine,
        (
            EngineEmitter,
            ChildOf(car),
            Transform::default(),
            source,
            settings,
        ),
    );
}

fn drive_engine_voice(
    mix: Res<MixConfig>,
    cfg: Res<VehicleConfig>,
    global: Res<GlobalVolume>,
    player: Query<&DriveIntent, With<Player>>,
    cars: Query<(&LinearVelocity, &GlobalTransform)>,
    mut emitters: Query<(&ChildOf, &mut SpatialAudioSink), With<EngineEmitter>>,
) {
    let throttle = player.single().map_or(0.0, |intent| intent.throttle);
    for (parent, mut sink) in &mut emitters {
        let Ok((velocity, transform)) = cars.get(parent.parent()) else {
            continue;
        };
        let forward = velocity.dot(*transform.forward());
        let (pitch, gain) = engine_voice(forward / cfg.max_speed, throttle, &mix.engine);
        sink.set_speed(pitch);
        // `set_volume` replaces the sink volume, so the global factor is applied here again.
        sink.set_volume(Volume::Linear(gain) * global.volume);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> EngineConfig {
        EngineConfig {
            base_hz: 45.0,
            harmonics: 6,
            noise: 0.15,
            idle_pitch: 0.8,
            max_pitch: 2.4,
            rev_share: 0.35,
            idle_volume: 0.25,
            max_volume: 0.6,
            ref_distance: 8.0,
        }
    }

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5
    }

    #[test]
    fn engine_voice_rows() {
        let c = cfg();
        assert!(close(engine_voice(0.0, 0.0, &c), (0.8, 0.25)));
        // Revving a standing car: rpm = rev_share, full throttle volume.
        assert!(close(engine_voice(0.0, 1.0, &c), (0.8 + 1.6 * 0.35, 0.6)));
        assert!(close(engine_voice(1.0, 0.0, &c), (2.4, 0.6)));
        // Reversing: |speed| and |throttle|.
        assert!(close(engine_voice(-0.2, -1.0, &c), (0.8 + 1.6 * 0.35, 0.6)));
    }
}
