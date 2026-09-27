//! A bumped traffic car (`Dynamic` after a contact) goes back to kinematic driving once it is safe to:
//! upright, on its path within the `lost` thresholds (on a connector only inside the conflict table's
//! body band), at rest, no dynamic body about to reach it (a wider sweep than the switch's, so the two
//! never flip each other), and a clear corridor from where it stands back onto its path line. A
//! vehicle at rest is kept only the rest skin away, plus what it closes over the recovery horizon
//! (`standing` also counts a body creeping at up to `hold_speed`). One that cannot is given up only when it is out of its lane with
//! nothing ahead of it: a car in a queue or behind a standing body waits (a given-up car there is one
//! more standing body, and the car behind it the next to be bumped).

use super::drive::Snap;
use super::lateral::{CORRIDOR_LATERAL_STEP, CORRIDOR_YAW_STEP_DEG, right_of};
use super::{
    FlatRect, Segment, TrafficConfig, TrafficGraph, swept_circle_hits_rect, swept_rect_hits_rect,
};
use crate::combat::aim_yaw;
use crate::occupancy::{ClaimFilter, Footprint, RoadBody, RoadOccupancy, Strip, flat};
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

fn wrap(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// A vehicle body at rest (a walker at rest reaches walk speed within ticks and keeps the full skin).
fn resting_vehicle(b: &RoadBody) -> bool {
    matches!(b.shape, Footprint::Rect(_)) && b.standing > 0.0
}

/// A vehicle body at rest, moved on by its own velocity over `horizon`, touches `tight` (the car's
/// corridor sample grown by the rest skin): one creeping towards it closes that skin.
fn resting_in(b: &RoadBody, tight: &FlatRect, horizon: f32) -> bool {
    let Footprint::Rect(rect) = b.shape else {
        return false;
    };
    b.standing > 0.0 && swept_rect_hits_rect(&rect, b.velocity * horizon, tight)
}

/// The rejoin from the current pose onto the path line at the same `s`: line point, tangent, the
/// offset (+ right) and the current yaw and the turn to the path yaw, rad.
fn rejoin(graph: &TrafficGraph, snap: &Snap) -> (Vec3, Vec3, f32, f32, f32) {
    let (point, tangent) = graph.pose(snap.car.segment, snap.car.s);
    let lateral = (snap.position - point).with_y(0.0).dot(right_of(tangent));
    let yaw = aim_yaw(snap.rotation * Vec3::NEG_Z);
    (point, tangent, lateral, yaw, wrap(aim_yaw(tangent) - yaw))
}

/// A rejoin on a connector slides: its heading stays on the path tangent.
fn slides(snap: &Snap) -> bool {
    matches!(snap.car.segment, Segment::Connector(_))
}

/// Skin against a vehicle at rest: the switch skin plus the farthest the car's own rejoin moves any
/// part of it (the offset, and the corner arc of its turn and, unless it `slides`, of the heading
/// swing the lateral law adds at rest), so the switch cannot fire on that body when the rejoin ends;
/// at most `recover.skin`.
fn rest_skin(cfg: &TrafficConfig, half: Vec2, lateral: f32, turn: f32, slides: bool) -> f32 {
    let l = &cfg.lateral;
    let swing = if slides {
        0.0
    } else {
        l.rate_at_rest
            .atan()
            .min(l.yaw_rate_deg.to_radians() * lateral.abs() / l.rate_at_rest)
    };
    (cfg.switch.skin + lateral.abs() + half.length() * (turn.abs() + swing)).min(cfg.recover.skin)
}

/// No dynamic body whose relative sweep over the horizon reaches the car's footprint grown by the
/// recovery skin (the rest skin for a vehicle at rest).
fn nobody_coming(
    road: &RoadOccupancy,
    snap: &Snap,
    cfg: &TrafficConfig,
    half: Vec2,
    rest: f32,
) -> bool {
    let r = &cfg.recover;
    let own = FlatRect::of(snap.position, snap.rotation, half + Vec2::splat(r.skin));
    let near = FlatRect::of(snap.position, snap.rotation, half + Vec2::splat(rest));
    let v = flat(snap.velocity);
    let reach = own.half.length() + 4.0;
    road.bodies()
        .iter()
        .filter(|b| b.dynamic && b.entity != snap.entity)
        .all(|b| {
            let d = (b.velocity - v) * r.horizon_seconds;
            match b.shape {
                Footprint::Rect(rect) => {
                    let own = if resting_vehicle(b) { &near } else { &own };
                    rect.centre.distance(own.centre) > reach + d.length()
                        || !swept_rect_hits_rect(&rect, d, own)
                }
                Footprint::Circle { centre, radius } => {
                    centre.distance(own.centre) > reach + d.length()
                        || !swept_circle_hits_rect(centre, radius, d, &own)
                }
            }
        })
}

/// The motion from the current pose onto the lane line (same `s`, path yaw) sampled in rectangles
/// grown by the skin (the rest skin against vehicles at rest); `Some(lateral)` when every sample is
/// free of bodies and claims.
fn corridor_clear(
    road: &RoadOccupancy,
    graph: &TrafficGraph,
    snap: &Snap,
    cfg: &TrafficConfig,
    half: Vec2,
) -> Option<f32> {
    let (point, tangent, lateral, yaw, turn) = rejoin(graph, snap);
    let steps = (turn.abs() / CORRIDOR_YAW_STEP_DEG.to_radians())
        .max(lateral.abs() / CORRIDOR_LATERAL_STEP)
        .ceil()
        .max(1.0) as u32;
    let grown = half + Vec2::splat(cfg.recover.skin);
    let near = half + Vec2::splat(rest_skin(cfg, half, lateral, turn, slides(snap)));
    let me = snap.entity;
    let horizon = cfg.recover.horizon_seconds;
    (0..=steps)
        .all(|k| {
            let f = k as f32 / steps as f32;
            let centre = point + right_of(tangent) * lateral * (1.0 - f);
            let rotation = Quat::from_rotation_y(yaw + turn * f);
            let rect = FlatRect::of(centre, rotation, grown);
            let tight = FlatRect::of(centre, rotation, near);
            road.blocked(
                &rect,
                |b| b.entity == me || resting_vehicle(b),
                ClaimFilter::All,
            )
            .is_none()
                && !road
                    .bodies()
                    .iter()
                    .any(|b| b.entity != me && resting_in(b, &tight, horizon))
        })
        .then_some(lateral)
}

/// How far the footprint reaches across the path line at the car's `s`, m.
fn reach_across(graph: &TrafficGraph, snap: &Snap, half: Vec2) -> f32 {
    let (point, tangent) = graph.pose(snap.car.segment, snap.car.s);
    let across = flat(right_of(tangent)).normalize_or_zero();
    let rect = FlatRect::of(snap.position, snap.rotation, half);
    (rect.centre - flat(point)).dot(across).abs()
        + rect.half.x * rect.axis.dot(across).abs()
        + rect.half.y * rect.axis.perp().dot(across).abs()
}

/// The footprint reaches out of the lane band, half the lane pitch either side of the path line (the
/// lane slack of a car on its line). A lane with no neighbour has no known pitch: the car's own width.
fn off_lane(graph: &TrafficGraph, snap: &Snap, half: Vec2) -> bool {
    let lane = match snap.car.segment {
        Segment::Lane(l) => l,
        Segment::Connector(c) => graph.connector(c).from_lane,
    };
    let pitch = graph.lane(lane).left_gap.unwrap_or(2.0 * half.x);
    reach_across(graph, snap, half) > pitch / 2.0
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
    // On a connector only where the conflict table's body model (half width + half the margin) holds,
    // across and along (a car pushed back past the connector start projects onto s 0 and would jump
    // there in one tick, outside the corridor).
    let on_path = match snap.car.segment {
        Segment::Lane(_) => true,
        Segment::Connector(_) => {
            let (point, tangent) = graph.pose(snap.car.segment, snap.car.s);
            let along = (snap.position - point).with_y(0.0).dot(tangent).abs();
            reach_across(graph, snap, half) <= half.x + cfg.conflict_margin / 2.0
                && along <= cfg.conflict_margin / 2.0
        }
    };
    let (_, _, lateral, _, turn) = rejoin(graph, snap);
    let rest = rest_skin(cfg, half, lateral, turn, slides(snap));
    let clear = (upright && at_rest && on_path && nobody_coming(road, snap, cfg, half, rest))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::occupancy::BodyKind;

    /// A parked car standing 1 s, its near side 0.25 m right of the car at the origin (half 1.2 x
    /// 2.04): 0.15 m outside that footprint grown by a 0.1 m rest skin.
    fn body(velocity: Vec2) -> RoadBody {
        let at = Vec3::X * (2.0 * 1.2 + 0.25);
        RoadBody {
            entity: Entity::PLACEHOLDER,
            kind: BodyKind::Vehicle,
            shape: Footprint::Rect(FlatRect::of(at, Quat::IDENTITY, Vec2::new(1.2, 2.04))),
            velocity,
            dynamic: true,
            standing: 1.0,
            siren: false,
        }
    }

    /// Over the 0.5 s horizon a body still or creeping away stays out of the 0.1 m rest skin (0.15 m
    /// left); one creeping in at 0.4 m/s (0.2 m) closes it; a walker is never a resting vehicle.
    #[test]
    fn resting_in_rows() {
        let tight = FlatRect::of(Vec3::ZERO, Quat::IDENTITY, Vec2::new(1.3, 2.14));
        assert!(!resting_in(&body(Vec2::ZERO), &tight, 0.5));
        assert!(!resting_in(&body(Vec2::X * 0.4), &tight, 0.5));
        assert!(resting_in(&body(Vec2::NEG_X * 0.4), &tight, 0.5));
        let walker = RoadBody {
            shape: Footprint::Circle {
                centre: Vec2::X * 1.65,
                radius: 0.3,
            },
            ..body(Vec2::ZERO)
        };
        assert!(!resting_vehicle(&walker) && !resting_in(&walker, &tight, 0.5));
    }
}
