//! Trauma camera shake (GDD §8): the player's shots, melee hits the player lands or takes, hits
//! that hurt the player and kills nearby add trauma; the camera rotates by `max · trauma² · noise`
//! and trauma decays on real time. Rotation only: the aim ray is written before the shake is applied.

use super::{JuiceConfig, PlayerHurt, config::ShakeConfig};
use crate::settings::GameSettings;
use bevy::prelude::*;
use gta_sim::{
    combat::{DamageDealt, MeleeHit, ShotFired},
    player::Player,
    vehicle::{Driving, VehicleImpact},
};

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct CameraShake {
    pub trauma: f32,
    /// Applied after the camera's own rotation.
    pub rotation: Quat,
}

/// Trauma of a crash of the player's car at `speed` m/s.
pub fn crash_trauma(speed: f32, cfg: &ShakeConfig) -> f32 {
    if speed < cfg.crash_min_speed {
        return 0.0;
    }
    (speed * cfg.crash_trauma_per_mps).min(1.0)
}

/// Each row adds its trauma, clamped at 1; rows stack (a punch taken is melee + hurt).
#[allow(clippy::too_many_arguments)]
pub(super) fn add_trauma(
    mut shots: MessageReader<ShotFired>,
    mut hits: MessageReader<MeleeHit>,
    mut hurts: MessageReader<PlayerHurt>,
    mut dealt: MessageReader<DamageDealt>,
    mut crashes: MessageReader<VehicleImpact>,
    players: Query<(Entity, &Transform, Option<&Driving>), With<Player>>,
    juice: Res<JuiceConfig>,
    mut shake: ResMut<CameraShake>,
) {
    let cfg = &juice.shake;
    let car = players
        .single()
        .ok()
        .and_then(|(.., driving)| driving.map(|d| d.vehicle));
    let player = players
        .single()
        .ok()
        .map(|(entity, transform, _)| (entity, transform));
    let is_player = |entity: Entity| player.is_some_and(|(p, _)| p == entity);
    let mut rows = Vec::new();
    rows.extend(
        shots
            .read()
            .filter(|shot| is_player(shot.shooter))
            .map(|_| cfg.shot_trauma),
    );
    rows.extend(
        hits.read()
            .filter(|hit| is_player(hit.attacker) || is_player(hit.target))
            .map(|_| cfg.melee_trauma),
    );
    rows.extend(hurts.read().map(|_| cfg.hurt_trauma));
    rows.extend(
        crashes
            .read()
            .filter(|crash| Some(crash.vehicle) == car)
            .map(|crash| crash_trauma(crash.speed, cfg)),
    );
    for hit in dealt.read() {
        let Some((body, transform)) = player else {
            continue;
        };
        if hit.killed
            && hit.target != body
            && hit.point.distance(transform.translation) <= cfg.death_radius
        {
            rows.push(cfg.death_trauma);
        }
    }
    for row in rows {
        shake.trauma = (shake.trauma + row).min(1.0);
    }
}

pub(super) fn shake_camera(
    real: Res<Time<Real>>,
    juice: Res<JuiceConfig>,
    settings: Res<GameSettings>,
    mut shake: ResMut<CameraShake>,
) {
    let cfg = &juice.shake;
    shake.trauma = (shake.trauma - cfg.decay_per_s * real.delta_secs()).max(0.0);
    // Wrapped (1 h) so f32 noise time keeps its precision in long sessions.
    let t = real.elapsed_secs_wrapped() * cfg.noise_hz;
    let scale = if settings.reduce_shake {
        cfg.reduced_scale
    } else {
        1.0
    };
    shake.rotation = Quat::IDENTITY.slerp(shake_rotation(shake.trauma, t, cfg), scale);
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
            reduced_scale: 0.3,
            shot_trauma: 0.1,
            hurt_trauma: 0.2,
            death_trauma: 0.4,
            death_radius: 12.0,
            crash_trauma_per_mps: 0.02,
            crash_min_speed: 5.0,
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
    fn crash_trauma_rows() {
        let cfg = cfg();
        assert_eq!(crash_trauma(4.9, &cfg), 0.0);
        assert!((crash_trauma(10.0, &cfg) - 0.2).abs() < 1e-6);
        assert_eq!(crash_trauma(80.0, &cfg), 1.0);
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
