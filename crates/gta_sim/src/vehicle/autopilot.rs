//! AI driving of a car body: pure pursuit to a target point (Coulter 1992) and a speed controller,
//! written into the car's own `DriveIntent`. Traffic and police cars set the target and the speed.

use super::{DriveIntent, VehicleConfig, steer_limit};
use crate::vehicle::AutopilotConfig;
use avian3d::prelude::*;
use bevy::prelude::*;

/// The AI driver of a car: where to steer and how fast to go.
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component, Default)]
pub struct Autopilot {
    pub target: Vec3,
    /// Wanted forward speed, m/s.
    pub speed: f32,
    /// Seconds stuck with the throttle open.
    pub stuck: f32,
    /// Seconds left of backing up.
    pub reverse_left: f32,
}

/// Steer input (−1..1, + = right) that brings a car at `position` facing `forward` onto `target`.
pub fn pursuit_steer(
    cfg: &VehicleConfig,
    position: Vec3,
    forward: Vec3,
    forward_speed: f32,
    target: Vec3,
) -> f32 {
    let right = forward.cross(Vec3::Y);
    let d = (target - position).with_y(0.0);
    let alpha = d.dot(right).atan2(d.dot(forward));
    let lookahead = d.length().max(cfg.autopilot.lookahead_min);
    let curvature = 2.0 * alpha.sin() / lookahead;
    let angle = (curvature * 2.0 * cfg.wheels.half_wheelbase).atan();
    (angle / steer_limit(cfg, forward_speed)).clamp(-1.0, 1.0)
}

/// Throttle (−1..1) towards `speed`: open while slower, brake while faster, hold at rest; never a
/// reverse by accident (`drive_force` reverses on a negative throttle below `hold_speed`).
pub fn speed_throttle(
    cfg: &VehicleConfig,
    ap: &AutopilotConfig,
    speed: f32,
    forward_speed: f32,
) -> f32 {
    let error = speed - forward_speed;
    if error > 0.0 {
        return (error * ap.speed_gain).min(1.0);
    }
    if forward_speed > cfg.hold_speed {
        return (error * ap.speed_gain).max(-1.0);
    }
    0.0
}

/// Autopilot speed for a car at forward speed `v` that should accelerate at `a` (IDM): the throttle
/// `gain · (speed − v)` opens by `a / acceleration`, so the car speeds up at the IDM rate, not at
/// `a·dt·gain` (a crawl from rest). Braking: any shortfall brakes.
pub fn follow_speed(cfg: &VehicleConfig, v: f32, a: f32, dt: f32) -> f32 {
    if a > 0.0 {
        v + a / (cfg.acceleration * cfg.autopilot.speed_gain)
    } else {
        (v + a * dt).max(0.0)
    }
}

pub(super) fn steer_autopilots(
    cfg: Res<VehicleConfig>,
    time: Res<Time<Fixed>>,
    mut cars: Query<(
        &mut Autopilot,
        &mut DriveIntent,
        &Position,
        &Rotation,
        &LinearVelocity,
        &RigidBody,
    )>,
) {
    let dt = time.delta_secs();
    let ap = &cfg.autopilot;
    for (mut pilot, mut intent, position, rotation, velocity, body) in &mut cars {
        if !body.is_dynamic() {
            continue;
        }
        let forward = rotation.0 * Vec3::NEG_Z;
        let forward_speed = velocity.0.dot(forward);
        let steer = pursuit_steer(&cfg, position.0, forward, forward_speed, pilot.target);
        let throttle = speed_throttle(&cfg, ap, pilot.speed, forward_speed);
        if throttle > 0.5 && velocity.0.length() < ap.stuck_speed {
            pilot.stuck += dt;
        } else {
            pilot.stuck = 0.0;
        }
        if pilot.stuck >= ap.stuck_seconds {
            pilot.stuck = 0.0;
            pilot.reverse_left = ap.reverse_seconds;
        }
        *intent = if pilot.reverse_left > 0.0 {
            pilot.reverse_left = (pilot.reverse_left - dt).max(0.0);
            DriveIntent {
                throttle: -1.0,
                steer: -steer,
                handbrake: false,
            }
        } else {
            DriveIntent {
                throttle,
                steer,
                handbrake: false,
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigRoot, load_config};
    use crate::vehicle::VEHICLE_CONFIG;

    fn shipped() -> VehicleConfig {
        let root = ConfigRoot(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into());
        load_config(&root, VEHICLE_CONFIG).expect("GATE BROKEN: sedan.ron")
    }

    #[test]
    fn pursuit_directions() {
        let cfg = shipped();
        // Car at the origin, target 10 m ahead and 5 m to the right, per heading.
        for (yaw_deg, right_target, left_target) in [
            (0.0, Vec3::new(5.0, 0.0, -10.0), Vec3::new(-5.0, 0.0, -10.0)),
            (
                90.0,
                Vec3::new(-10.0, 0.0, -5.0),
                Vec3::new(-10.0, 0.0, 5.0),
            ),
            (180.0, Vec3::new(-5.0, 0.0, 10.0), Vec3::new(5.0, 0.0, 10.0)),
        ] {
            let forward = Quat::from_rotation_y(f32::to_radians(yaw_deg)) * Vec3::NEG_Z;
            let right = pursuit_steer(&cfg, Vec3::ZERO, forward, 5.0, right_target);
            let left = pursuit_steer(&cfg, Vec3::ZERO, forward, 5.0, left_target);
            assert!(right > 0.0, "yaw {yaw_deg}: {right}");
            assert!(left < 0.0, "yaw {yaw_deg}: {left}");
            assert!((right + left).abs() < 1e-5, "yaw {yaw_deg}: symmetric");
        }
        // Straight ahead: no steer.
        let ahead = pursuit_steer(
            &cfg,
            Vec3::ZERO,
            Vec3::NEG_Z,
            5.0,
            Vec3::new(0.0, 0.0, -10.0),
        );
        assert!(ahead.abs() < 1e-6);
        // A wheel angle of +δ points +x (right) in the body frame, as `chassis.rs` steers.
        assert!(crate::vehicle::wheel_forward(0.2).x > 0.0);
    }

    #[test]
    fn throttle_rows() {
        let cfg = shipped();
        let ap = &cfg.autopilot;
        assert_eq!(speed_throttle(&cfg, ap, 10.0, 0.0), 1.0);
        assert!((speed_throttle(&cfg, ap, 10.0, 9.0) - 0.5).abs() < 1e-6);
        assert!((speed_throttle(&cfg, ap, 8.0, 9.0) + 0.5).abs() < 1e-6);
        assert_eq!(speed_throttle(&cfg, ap, 0.0, 20.0), -1.0);
        // At rest with no wanted speed: hold, never reverse.
        assert_eq!(speed_throttle(&cfg, ap, 0.0, 0.3), 0.0);
        assert_eq!(speed_throttle(&cfg, ap, 0.0, 0.0), 0.0);
    }
}
