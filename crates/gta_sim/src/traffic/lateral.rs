//! Sideways offset of a kinematic traffic car from its path line: one rate-limited law for the rejoin
//! after a recovery, the go-around and the siren yield. On a lane the offset steps towards its target;
//! on a connector it decays linearly to 0 at the connector end unless the car passes or rejoins.

use super::{LateralConfig, Manoeuvre, Segment, TrafficCar, TrafficGraph};
use crate::combat::aim_yaw;
use bevy::prelude::*;

/// Geometry law: corridor samples at most 7.5 deg and 0.3 m apart keep the corner chord between two
/// samples (2.37 m x sin 7.5 deg = 0.31 m) under the recovery skin.
pub(super) const CORRIDOR_YAW_STEP_DEG: f32 = 7.5;
pub(super) const CORRIDOR_LATERAL_STEP: f32 = 0.3;

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
/// entry) decays to 0 at the end, unless the car steps it there (`holds`: it goes around a body in
/// the box or rejoins its path).
pub fn effective_lateral(
    graph: &TrafficGraph,
    seg: Segment,
    s: f32,
    lateral: f32,
    holds: bool,
) -> f32 {
    match seg {
        Segment::Connector(_) if !holds => {
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

/// The band across the lane line (`lo < 0 < hi`, + right) a manoeuvring lane car's body may use: half
/// the lane `pitch` either side, widened on a pass's side to its claim and on a yield's side to the car
/// at its offset.
pub fn manoeuvre_band(manoeuvre: Manoeuvre, pitch: f32, half_x: f32, clearance: f32) -> (f32, f32) {
    let edge = pitch / 2.0;
    let (offset, reach) = match manoeuvre {
        Manoeuvre::Pass { offset, .. } => (offset, offset.abs() + half_x + clearance),
        Manoeuvre::Yield { offset, .. } => (offset, offset.abs() + half_x),
        _ => (0.0, edge),
    };
    let wide = reach.max(edge);
    if offset > 0.0 {
        (-edge, wide)
    } else if offset < 0.0 {
        (-wide, edge)
    } else {
        (-edge, edge)
    }
}

/// Largest yaw off the path tangent, rad, that keeps a body of half extents `half` (across, along)
/// inside `band` at every offset of `laterals`: at yaw error θ it reaches `|half| sin(θ + φ)` across
/// either side (the nose one way, the rear the other), φ = atan2(half.x, half.y).
pub fn yaw_cap(band: (f32, f32), laterals: (f32, f32), half: Vec2) -> f32 {
    let room = (band.1 - laterals.0.max(laterals.1)).min(laterals.0.min(laterals.1) - band.0);
    let r = half.length();
    ((room / r).clamp(-1.0, 1.0).asin() - half.x.atan2(half.y)).max(0.0)
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

    /// Sedan half (1.2, 2.04), pitch 3.25: on the line the swing is capped at asin(1.625 / 2.367) -
    /// 30.47 deg = 12.9 deg; a curb pass at +3.25 widens the right side to 3.25 + 1.2 + 0.5.
    #[test]
    fn yaw_cap_rows() {
        let half = Vec2::new(1.2, 2.04);
        let lane = manoeuvre_band(Manoeuvre::None, 3.25, 1.2, 0.5);
        assert_eq!(lane, (-1.625, 1.625));
        let cap = yaw_cap(lane, (0.0, 0.0), half).to_degrees();
        assert!((cap - 12.9).abs() < 0.05, "{cap}");
        // The room is the worst of the two offsets; no room left caps the yaw at 0.
        assert_eq!(
            yaw_cap(lane, (0.0, 0.3), half),
            yaw_cap(lane, (0.3, 0.0), half)
        );
        assert!(yaw_cap(lane, (0.0, 0.3), half) < yaw_cap(lane, (0.0, 0.0), half));
        assert_eq!(yaw_cap(lane, (0.5, 0.5), half), 0.0);
        let pass = Manoeuvre::Pass {
            obstacle: Entity::PLACEHOLDER,
            offset: 3.25,
            need: 3.25,
            hold_s: 0.0,
            merge_s: 0.0,
            end_s: 0.0,
            go: true,
        };
        assert_eq!(manoeuvre_band(pass, 3.25, 1.2, 0.5), (-1.625, 4.95));
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
