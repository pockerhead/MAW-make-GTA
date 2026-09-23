mod config;
mod damage_numbers;
#[cfg(test)]
mod damage_numbers_gate;
mod hit_stop;
mod shake;

pub use config::{JUICE_CONFIG, JuiceConfig};
pub use damage_numbers::DamageNumbersPlugin;
pub use hit_stop::{HitStop, HitStopPlugin, HitStopSystems};
pub use shake::CameraShake;

use bevy::prelude::*;
use gta_sim::{combat::ShotFired, player::Player};

/// Visual camera kick in radians of pitch; the aim ray ignores it.
#[derive(Resource, Default)]
pub struct CameraRecoil {
    pub pitch: f32,
}

/// Recoil, floating damage numbers, melee hit-stop and camera shake.
pub struct JuicePlugin;

impl Plugin for JuicePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraRecoil>()
            .init_resource::<CameraShake>()
            .add_plugins((DamageNumbersPlugin, HitStopPlugin))
            .add_systems(
                Update,
                (
                    kick_camera,
                    (shake::add_melee_trauma, shake::shake_camera).chain(),
                ),
            );
    }
}

fn kick_camera(
    mut shots: MessageReader<ShotFired>,
    players: Query<(), With<Player>>,
    juice: Res<JuiceConfig>,
    real: Res<Time<Real>>,
    mut recoil: ResMut<CameraRecoil>,
) {
    for shot in shots.read() {
        if players.contains(shot.shooter) {
            recoil.pitch += juice.recoil_deg.get(shot.weapon).to_radians();
        }
    }
    let decay = std::f32::consts::LN_2 / juice.recoil_half_life;
    recoil.pitch.smooth_nudge(&0.0, decay, real.delta_secs());
}
