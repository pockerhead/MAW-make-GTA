//! Sideways offset of a kinematic traffic car from its path line: one rate-limited law for the rejoin
//! after a recovery, the go-around and the siren yield. On a lane the offset steps towards its target;
//! on a connector it decays linearly to 0 at the connector end.

use super::{LateralConfig, Manoeuvre, Segment, TrafficCar, TrafficGraph};
use crate::combat::aim_yaw;
use bevy::prelude::*;

/// Right of a flat tangent.
pub fn right_of(tangent: Vec3) -> Vec3 {
    Vec3::new(-tangent.z, 0.0, tangent.x)
}

/// The offset `car` steers to: a pass keeps its side until the rear is past `merge_s`, a yield its
/// curb offset, anything else the path line. `rear_s`: the car's rear along its lane, m.
pub fn target_lateral(car: &TrafficCar, rear_s: f32) -> f32 {
    match car.manoeuvre {
        Manoeuvre::Pass {
            offset,
            merge_s,
            go: true,
            ..
        } if rear_s < merge_s => offset,
        Manoeuvre::Yield { offset, .. } => offset,
        _ => 0.0,
    }
}

/// One tick of the offset towards `target` at the rate of speed `v`.
pub fn step_lateral(lateral: f32, target: f32, v: f32, dt: f32, cfg: &LateralConfig) -> f32 {
    let max = cfg.rate(v) * dt;
    if (target - lateral).abs() <= max {
        target
    } else {
        lateral + (target - lateral).signum() * max
    }
}

/// The offset in effect at `s` along `seg`: on a connector `lateral` (its value at the connector
/// entry) decays to 0 at the end, unless the car goes around a body in the box (`passing`).
pub fn effective_lateral(
    graph: &TrafficGraph,
    seg: Segment,
    s: f32,
    lateral: f32,
    passing: bool,
) -> f32 {
    match seg {
        Segment::Connector(_) if !passing => {
            let length = graph.length(seg).max(f32::EPSILON);
            lateral * (1.0 - s / length).clamp(0.0, 1.0)
        }
        _ => lateral,
    }
}

/// Point (y = 0) and unit tangent `lateral` m right of the path at `s` along `seg`.
pub fn offset_pose(graph: &TrafficGraph, seg: Segment, s: f32, lateral: f32) -> (Vec3, Vec3) {
    let (point, tangent) = graph.pose(seg, s);
    (point + right_of(tangent) * lateral, tangent)
}

/// Heading of a car moving along `tangent` at `v` while its offset changes at `rate` m/s (+ right).
pub fn heading_yaw(tangent: Vec3, v: f32, rate: f32) -> f32 {
    aim_yaw(tangent * v.max(1.0) + right_of(tangent) * rate)
}

/// Angular velocity that turns `rotation` towards the upright heading `yaw` in one tick of `dt`, at
/// most `yaw_rate_deg` per second (also levels a residual roll or pitch).
pub fn turn_towards(rotation: Quat, yaw: f32, dt: f32, cfg: &LateralConfig) -> Vec3 {
    let delta = Quat::from_rotation_y(yaw) * rotation.inverse();
    let (axis, angle) = delta.to_axis_angle();
    let angle = if angle > std::f32::consts::PI {
        angle - std::f32::consts::TAU
    } else {
        angle
    };
    (axis * angle / dt).clamp_length_max(cfg.yaw_rate_deg.to_radians())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> LateralConfig {
        LateralConfig {
            rate_at_rest: 0.8,
            slope: 0.15,
            yaw_rate_deg: 60.0,
        }
    }

    #[test]
    fn step_is_rate_limited_and_lands_on_target() {
        let c = cfg();
        let dt = 1.0 / 64.0;
        // At rest 0.8 m/s: 0.0125 m per tick, off-grid start so the clamp decides the last step.
        let mut l: f32 = 0.003;
        let mut ticks = 0;
        while (l - 2.9).abs() > 1e-6 {
            let next = step_lateral(l, 2.9, 0.0, dt, &c);
            assert!((next - l).abs() <= 0.8 * dt + 1e-6);
            l = next;
            ticks += 1;
        }
        assert!(l == 2.9 && ticks == 232, "{ticks}");
    }

    /// Right of the tangent on three headings, and a car shifting right turns its nose right.
    #[test]
    fn offset_sign_rows() {
        assert_eq!(right_of(Vec3::Z), Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(right_of(Vec3::X), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(right_of(Vec3::NEG_Z), Vec3::new(1.0, 0.0, 0.0));
        let yaw = heading_yaw(Vec3::NEG_Z, 6.0, 1.7);
        let forward = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
        assert!(
            forward.x > 0.0,
            "shifting right from -Z heads +X: {forward}"
        );
        assert!((forward.x.atan2(-forward.z).to_degrees() - 15.83).abs() < 0.1);
    }

    #[test]
    fn turn_is_clamped_and_levels() {
        let c = cfg();
        let dt = 1.0 / 64.0;
        let w = turn_towards(Quat::IDENTITY, 1.0, dt, &c);
        assert!(
            (w.length() - 60f32.to_radians()).abs() < 1e-4 && w.y > 0.0,
            "{w}"
        );
        let rolled = Quat::from_rotation_z(0.01);
        let w = turn_towards(rolled, 0.0, dt, &c);
        assert!(w.z < 0.0 && w.length() > 0.0, "levels the roll: {w}");
    }
}
