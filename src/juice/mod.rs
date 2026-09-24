mod config;
mod damage_arc;
mod damage_numbers;
#[cfg(test)]
mod damage_numbers_gate;
#[cfg(test)]
mod feedback_gate;
mod hit_stop;
mod shake;
mod vignette;

pub use config::{JUICE_CONFIG, JuiceConfig, StarPulseConfig};
pub use damage_numbers::DamageNumbersPlugin;
pub use hit_stop::{HitStop, HitStopPlugin, HitStopSystems};
pub use shake::CameraShake;

use crate::settings::GameSettings;
use bevy::prelude::*;
use gta_sim::{
    character::Health, combat::ShotFired, flow::GameState, player::Player, wanted::WantedLevel,
};

/// Visual camera kick in radians of pitch; the aim ray ignores it.
#[derive(Resource, Default)]
pub struct CameraRecoil {
    pub pitch: f32,
}

/// The player's health plus armour fell since the last frame.
#[derive(Message, Clone, Copy, Debug)]
pub struct PlayerHurt;

/// The wanted level rose.
#[derive(Message, Clone, Copy, Debug)]
pub struct StarsRaised;

/// Recoil, floating damage numbers, melee hit-stop, camera shake, the hurt vignette and the
/// damage-direction arc.
pub struct JuicePlugin;

impl Plugin for JuicePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraRecoil>()
            .init_resource::<CameraShake>()
            .register_type::<CameraShake>()
            .add_message::<PlayerHurt>()
            .add_message::<StarsRaised>()
            .add_plugins((
                DamageNumbersPlugin,
                HitStopPlugin,
                vignette::VignettePlugin,
                damage_arc::DamageArcPlugin,
            ))
            .add_systems(
                Update,
                (
                    kick_camera,
                    (
                        detect_player_hurt,
                        detect_stars_raised,
                        shake::add_trauma,
                        shake::shake_camera,
                    )
                        .chain(),
                    vignette::update_vignette.after(detect_player_hurt),
                ),
            );
    }
}

fn kick_camera(
    mut shots: MessageReader<ShotFired>,
    players: Query<(), With<Player>>,
    juice: Res<JuiceConfig>,
    settings: Res<GameSettings>,
    real: Res<Time<Real>>,
    mut recoil: ResMut<CameraRecoil>,
) {
    let scale = if settings.reduce_camera_motion {
        juice.camera_motion_reduced_scale
    } else {
        1.0
    };
    for shot in shots.read() {
        if players.contains(shot.shooter) {
            recoil.pitch += juice.recoil_deg.get(shot.weapon).to_radians() * scale;
        }
    }
    let decay = std::f32::consts::LN_2 / juice.recoil_half_life;
    recoil.pitch.smooth_nudge(&0.0, decay, real.delta_secs());
}

/// Health plus armour of the same player entity fell while playing. Outside `Playing` the pool is
/// forgotten: a respawn heals the same entity and drops its armour (Busted with armour > 0).
fn detect_player_hurt(
    state: Res<State<GameState>>,
    players: Query<(Entity, &Health), With<Player>>,
    mut last: Local<Option<(Entity, f32)>>,
    mut hurt: MessageWriter<PlayerHurt>,
) {
    if *state.get() != GameState::Playing {
        *last = None;
        return;
    }
    let Ok((player, health)) = players.single() else {
        return;
    };
    let pool = health.current + health.armor;
    if let Some((previous, before)) = *last
        && previous == player
        && pool < before
    {
        hurt.write(PlayerHurt);
    }
    *last = Some((player, pool));
}

fn detect_stars_raised(
    wanted: Res<WantedLevel>,
    mut last: Local<u8>,
    mut raised: MessageWriter<StarsRaised>,
) {
    if wanted.stars > *last {
        raised.write(StarsRaised);
    }
    *last = wanted.stars;
}
