//! Traffic yielding to a siren car (GDD §5.3, TASK-032): a car a responding or chasing police car
//! comes up behind, or drives at, pulls to the curb side and stops until it has passed.

use super::drive::Snap;
use super::lateral::right_of;
use super::{FlatRect, IdmConfig, Manoeuvre, Segment, TrafficConfig, TrafficGraph};
use crate::occupancy::{ClaimFilter, Footprint, RoadOccupancy, flat, world_clear};
use crate::vehicle::VehicleConfig;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Gap to the virtual standing obstacle a yielding car brakes for: its comfortable stop from `v`. It
/// shrinks to 0 with the speed, so the car comes to a stop (a gap ending at the jam gap is IDM's rest
/// point: the car would creep towards it forever).
pub(super) fn yield_gap(v: f32, idm: &IdmConfig) -> f32 {
    v * v / (2.0 * idm.comfortable_deceleration)
}

/// A siren car relative to a car on its lane: (along, across, heading . tangent).
fn relative(shape: &Footprint, at: Vec2, tangent: Vec2) -> Option<(f32, f32, f32)> {
    let Footprint::Rect(r) = shape else {
        return None;
    };
    let d = r.centre - at;
    let forward = -r.axis.perp();
    Some((d.dot(tangent), d.dot(tangent.perp()), forward.dot(tangent)))
}

/// The yield of an idle kinematic car on a lane, or the end of its current yield (with the seconds
/// to ignore sirens after a timeout); `None`: no change.
#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    road: &RoadOccupancy,
    spatial: &SpatialQuery,
    graph: &TrafficGraph,
    snap: &Snap,
    idle: bool,
    tick: u64,
    dt: f32,
    configs: (&TrafficConfig, &VehicleConfig),
) -> Option<(Manoeuvre, f32)> {
    let (cfg, vehicle) = configs;
    let car = &snap.car;
    let Segment::Lane(l) = car.segment else {
        return None;
    };
    let lane = graph.lane(l);
    let half = vehicle.half_extents();
    let (point, tangent) = graph.pose(car.segment, car.s);
    let (at, t) = (flat(point), flat(tangent).normalize_or_zero());
    let pitch = lane.left_gap.unwrap_or(2.0 * half.x);
    let cos45 = std::f32::consts::FRAC_1_SQRT_2;
    let timeout = (cfg.sirens.timeout_seconds / dt).round() as u64;
    if let Manoeuvre::Yield { siren, since, .. } = car.manoeuvre {
        // Past: its centre two half lengths beyond ours along the lane (ahead if it came from
        // behind, behind if it drove at us).
        let passed = road
            .body(siren)
            .and_then(|b| relative(&b.shape, at, t))
            .is_some_and(|(along, _, heading)| {
                if heading > 0.0 {
                    along > 2.0 * half.z
                } else {
                    along < -2.0 * half.z
                }
            });
        let timed_out = tick.saturating_sub(since) >= timeout;
        return (passed || timed_out).then(|| {
            let deaf = if timed_out {
                cfg.sirens.timeout_seconds
            } else {
                0.0
            };
            (Manoeuvre::Rejoin, deaf)
        });
    }
    // A car that would stop within a car length and a jam gap of its lane start leaves a siren car
    // behind it in the box, where it cannot change lanes: it drives on.
    if !idle || car.deaf > 0.0 || car.s - half.z < 2.0 * half.z + cfg.idm.min_gap {
        return None;
    }
    let reach = cfg.sirens.yield_distance;
    let siren = road.sirens().find(|b| {
        relative(&b.shape, at, t).is_some_and(|(along, across, heading)| {
            let behind =
                heading > cos45 && (-reach..=-half.z).contains(&along) && across.abs() <= pitch;
            let ahead = heading < -cos45
                && (half.z..=reach).contains(&along)
                && across.abs() <= 1.5 * pitch;
            behind || ahead
        })
    })?;
    // The curb lane of an avenue when free, else the slack of its own lane.
    let right = right_of(tangent);
    let curb = FlatRect {
        centre: flat(point + right * pitch),
        axis: flat(right).normalize_or(Vec2::X),
        half: Vec2::new(half.x, half.z + cfg.idm.min_gap),
    };
    let curb_free = lane.curb_lane
        && road.blocked(&curb, |_| false, ClaimFilter::All).is_none()
        && world_clear(spatial, &curb, 0.05, vehicle.rest_height() + half.y);
    // With no curb lane free and no opposite lane the siren car cannot get by: stopping only blocks it.
    let offset = if curb_free {
        pitch
    } else if lane.left_gap.is_some() {
        (pitch / 2.0 - half.x).max(0.0)
    } else {
        return None;
    };
    Some((
        Manoeuvre::Yield {
            siren: siren.entity,
            since: tick,
            offset,
        },
        0.0,
    ))
}
