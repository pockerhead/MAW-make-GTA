//! Trauma camera shake (GDD §8): a melee hit the player lands or takes adds trauma; the camera
//! rotates by `max · trauma² · noise` and trauma decays on real time. Rotation only: the aim ray is
//! written before the shake is applied.

use super::{JuiceConfig, config::ShakeConfig};
use bevy::prelude::*;
use gta_sim::{combat::MeleeHit, player::Player};

#[derive(Resource, Default)]
pub struct CameraShake {
    pub trauma: f32,
    /// Applied after the camera's own rotation.
    pub rotation: Quat,
}

pub(super) fn add_melee_trauma(
    mut hits: MessageReader<MeleeHit>,
    players: Query<(), With<Player>>,
    juice: Res<JuiceConfig>,
    mut shake: ResMut<CameraShake>,
) {
    for hit in hits.read() {
        if players.contains(hit.attacker) || players.contains(hit.target) {
            shake.trauma = (shake.trauma + juice.shake.melee_trauma).min(1.0);
        }
    }
}

pub(super) fn shake_camera(
    real: Res<Time<Real>>,
    juice: Res<JuiceConfig>,
    mut shake: ResMut<CameraShake>,
) {
    let cfg = &juice.shake;
    shake.trauma = (shake.trauma - cfg.decay_per_s * real.delta_secs()).max(0.0);
    // Wrapped (1 h) so f32 noise time keeps its precision in long sessions.
    let t = real.elapsed_secs_wrapped() * cfg.noise_hz;
    shake.rotation = shake_rotation(shake.trauma, t, cfg);
}

/// Camera shake rotation at `trauma` and noise time `t` (lattice steps).
pub fn shake_rotation(trauma: f32, t: f32, cfg: &ShakeConfig) -> Quat {
    let s = trauma * trauma;
    Quat::from_euler(
        EulerRot::YXZ,
        cfg.max_yaw_deg.to_radians() * s * smooth_noise(0, t),
        cfg.max_pitch_deg.to_radians() * s * smooth_noise(1, t),
        cfg.max_roll_deg.to_radians() * s * smooth_noise(2, t),
    )
}

/// Value noise in [-1, 1]: hashed lattice values at integer `t`, smoothstep between them.
pub fn smooth_noise(channel: u32, t: f32) -> f32 {
    let i = t.floor();
    let f = t - i;
    let (a, b) = (lattice(channel, i as i32), lattice(channel, i as i32 + 1));
    a + (b - a) * f * f * (3.0 - 2.0 * f)
}

/// Integer hash of (channel, i) mapped to [-1, 1].
fn lattice(channel: u32, i: i32) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x9E37_79B1) ^ channel.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / 8_388_607.5 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ShakeConfig {
        ShakeConfig {
            melee_trauma: 0.25,
            decay_per_s: 1.2,
            max_yaw_deg: 3.0,
            max_pitch_deg: 3.0,
            max_roll_deg: 5.0,
            noise_hz: 15.0,
        }
    }

    fn samples() -> impl Iterator<Item = f32> {
        (0..10_000).map(|i| i as f32 * 0.0137 - 40.0)
    }

    #[test]
    fn noise_stays_in_range_and_is_continuous() {
        for channel in 0..3 {
            for t in samples() {
                let n = smooth_noise(channel, t);
                assert!((-1.0..=1.0).contains(&n), "n({channel}, {t}) = {n}");
                let step = (smooth_noise(channel, t + 1e-3) - n).abs();
                assert!(step < 0.01, "jump {step} at {t}");
            }
        }
    }

    #[test]
    fn channels_differ() {
        let t = 3.3;
        let [a, b, c] = [0, 1, 2].map(|channel| smooth_noise(channel, t));
        assert!(a != b && b != c && a != c, "{a} {b} {c}");
    }

    #[test]
    fn no_trauma_no_shake() {
        for t in samples().step_by(97) {
            assert_eq!(shake_rotation(0.0, t, &cfg()), Quat::IDENTITY);
        }
    }

    #[test]
    fn full_trauma_stays_within_max_angles() {
        let cfg = cfg();
        for t in samples().step_by(7) {
            let (yaw, pitch, roll) = shake_rotation(1.0, t, &cfg).to_euler(EulerRot::YXZ);
            for (angle, max) in [
                (yaw, cfg.max_yaw_deg),
                (pitch, cfg.max_pitch_deg),
                (roll, cfg.max_roll_deg),
            ] {
                assert!(
                    angle.abs() <= max.to_radians() + 1e-5,
                    "{angle} > {max} deg at {t}"
                );
            }
        }
    }
}
