mod config;
mod damage_numbers;
#[cfg(test)]
mod damage_numbers_gate;

pub use config::{JUICE_CONFIG, JuiceConfig};
pub use damage_numbers::DamageNumbersPlugin;

use bevy::prelude::*;
use gta_sim::{combat::ShotFired, player::Player};

/// Visual camera kick in radians of pitch; the aim ray ignores it.
#[derive(Resource, Default)]
pub struct CameraRecoil {
    pub pitch: f32,
}

/// Recoil and floating damage numbers.
pub struct JuicePlugin;

impl Plugin for JuicePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraRecoil>()
            .add_plugins(DamageNumbersPlugin)
            .add_systems(Update, kick_camera);
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
