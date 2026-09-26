//! A bumped traffic car (`Dynamic` after a contact) goes back to kinematic driving once it is safe to:
//! upright, on its lane within the `lost` thresholds, at rest, no dynamic body about to reach it (a
//! wider sweep than the switch's, so the two never flip each other), and a clear corridor from where
//! it stands back onto its lane line. One that cannot is given up only when it is out of its lane
//! with nothing ahead of it: a car in a queue or behind a standing body waits (a given-up car there
//! is one more standing body, and the car behind it the next to be bumped).

use super::drive::Snap;
use super::lateral::right_of;
use super::{
    FlatRect, Segment, TrafficConfig, TrafficGraph, swept_circle_hits_rect, swept_rect_hits_rect,
};
use crate::combat::aim_yaw;
use crate::occupancy::{ClaimFilter, Footprint, RoadOccupancy, Strip, flat};
use crate::vehicle::VehicleConfig;
use bevy::prelude::*;

pub(super) enum Recovery {
    Stay,
    /// Back to kinematic, `lateral` m off the lane line (+ right).
    Recover {
        lateral: f32,
    },
    GiveUp,
}

/// Geometry law: corridor samples at most 7.5 deg and 0.3 m apart keep the corner chord between two
/// samples (2.37 m x sin 7.5 deg = 0.31 m) under the recovery skin.
const CORRIDOR_YAW_STEP_DEG: f32 = 7.5;
const CORRIDOR_LATERAL_STEP: f32 = 0.3;

fn wrap(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// No dynamic body whose relative sweep over the horizon reaches the car's footprint grown by the
/// recovery skin.
fn nobody_coming(road: &RoadOccupancy, snap: &Snap, cfg: &TrafficConfig, half: Vec2) -> bool {
    let r = &cfg.recover;
    let own = FlatRect::of(snap.position, snap.rotation, half + Vec2::splat(r.skin));
    let v = flat(snap.velocity);
    let reach = own.half.length() + 4.0;
    road.bodies()
        .iter()
        .filter(|b| b.dynamic && b.entity != snap.entity)
        .all(|b| {
            let d = (b.velocity - v) * r.horizon_seconds;
            match b.shape {
                Footprint::Rect(rect) => {
                    rect.centre.distance(own.centre) > reach + d.length()
                        || !swept_rect_hits_rect(&rect, d, &own)
                }
                Footprint::Circle { centre, radius } => {
                    centre.distance(own.centre) > reach + d.length()
                        || !swept_circle_hits_rect(centre, radius, d, &own)
                }
            }
        })
}

/// The motion from the current pose onto the lane line (same `s`, path yaw) sampled in rectangles
/// grown by the skin; `Some(lateral)` when every sample is free of bodies and claims.
fn corridor_clear(
    road: &RoadOccupancy,
    graph: &TrafficGraph,
    snap: &Snap,
    cfg: &TrafficConfig,
    half: Vec2,
) -> Option<f32> {
    let (point, tangent) = graph.pose(snap.car.segment, snap.car.s);
    let lateral = (snap.position - point).with_y(0.0).dot(right_of(tangent));
    let yaw = aim_yaw(snap.rotation * Vec3::NEG_Z);
    let turn = wrap(aim_yaw(tangent) - yaw);
    let steps = (turn.abs() / CORRIDOR_YAW_STEP_DEG.to_radians())
        .max(lateral.abs() / CORRIDOR_LATERAL_STEP)
        .ceil()
        .max(1.0) as u32;
    let grown = half + Vec2::splat(cfg.recover.skin);
    let me = snap.entity;
    (0..=steps)
        .all(|k| {
            let f = k as f32 / steps as f32;
            let centre = point + right_of(tangent) * lateral * (1.0 - f);
            let rotation = Quat::from_rotation_y(yaw + turn * f);
            let rect = FlatRect::of(centre, rotation, grown);
            road.blocked(&rect, |b| b.entity == me, ClaimFilter::All)
                .is_none()
        })
        .then_some(lateral)
}

/// The footprint reaches out of the lane band, half the lane pitch either side of the path line (the
/// lane slack of a car on its line). A lane with no neighbour has no known pitch: the car's own width.
fn off_lane(graph: &TrafficGraph, snap: &Snap, half: Vec2) -> bool {
    let lane = match snap.car.segment {
        Segment::Lane(l) => l,
        Segment::Connector(c) => graph.connector(c).from_lane,
    };
    let pitch = graph.lane(lane).left_gap.unwrap_or(2.0 * half.x);
    let (point, tangent) = graph.pose(snap.car.segment, snap.car.s);
    let across = flat(right_of(tangent)).normalize_or_zero();
    let rect = FlatRect::of(snap.position, snap.rotation, half);
    let reach = (rect.centre - flat(point)).dot(across).abs()
        + rect.half.x * rect.axis.dot(across).abs()
        + rect.half.y * rect.axis.perp().dot(across).abs();
    reach > pitch / 2.0
}

/// A body or claim on the lane line within twice the jam gap of the nose (a car standing in a queue
/// rests at the jam gap: a strip of exactly that length can miss its leader).
fn led(
    road: &RoadOccupancy,
    graph: &TrafficGraph,
    snap: &Snap,
    cfg: &TrafficConfig,
    half: Vec2,
) -> bool {
    let (point, tangent) = graph.pose(snap.car.segment, snap.car.s);
    let dir = flat(tangent).normalize_or_zero();
    let strip = Strip {
        origin: flat(point) + dir * half.y,
        dir,
        length: 2.0 * cfg.idm.min_gap,
        half_width: half.x,
    };
    road.first_along(&strip, |b| b.entity == snap.entity)
        .is_some()
}

/// One tick of the recovery of a `Dynamic` car that is not lost; updates its calm time.
pub(super) fn recover_dynamic(
    snap: &mut Snap,
    road: &RoadOccupancy,
    graph: &TrafficGraph,
    cfg: &TrafficConfig,
    vehicle: &VehicleConfig,
    dt: f32,
) -> Recovery {
    let r = &cfg.recover;
    let half = vehicle.half_extents();
    let half = Vec2::new(half.x, half.z);
    let upright = (snap.rotation * Vec3::Y).y >= r.max_tilt_deg.to_radians().cos();
    let at_rest = snap.velocity.with_y(0.0).length() <= vehicle.hold_speed;
    let on_lane = matches!(snap.car.segment, Segment::Lane(_));
    let clear = (upright && at_rest && on_lane && nobody_coming(road, snap, cfg, half))
        .then(|| corridor_clear(road, graph, snap, cfg, half))
        .flatten();
    snap.car.calm = if clear.is_some() {
        snap.car.calm + dt
    } else {
        0.0
    };
    if let Some(lateral) = clear.filter(|_| snap.car.calm >= r.seconds) {
        return Recovery::Recover { lateral };
    }
    let may_give_up = at_rest && off_lane(graph, snap, half) && !led(road, graph, snap, cfg, half);
    snap.car.stood = if may_give_up {
        snap.car.stood + dt
    } else {
        0.0
    };
    if snap.car.stood >= r.give_up_seconds {
        Recovery::GiveUp
    } else {
        Recovery::Stay
    }
}
