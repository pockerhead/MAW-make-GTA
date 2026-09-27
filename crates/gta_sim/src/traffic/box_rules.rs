//! Junction box rules (TASK-032): a grant also needs its connector path free of bodies the conflict
//! table does not know (a car left in the box, a demoted holder, a passer); a queue head whose path is
//! blocked by a standing body takes another exit, or enters alone and goes around it (the whole box).

use super::lateral::right_of;
use super::{FlatRect, Junction, Manoeuvre, Segment, TrafficConfig, TrafficGraph};
use crate::occupancy::{BodyKind, ClaimFilter, Footprint, RoadOccupancy, flat, world_clear};
use crate::vehicle::VehicleConfig;
use avian3d::prelude::*;
use bevy::prelude::*;

/// The connector's polyline as rectangles (no end caps), `half_width` either side.
pub(super) fn connector_rects(graph: &TrafficGraph, c: u32, half_width: f32) -> Vec<FlatRect> {
    graph
        .connector(c)
        .points
        .windows(2)
        .filter_map(|w| {
            let d = (w[1] - w[0]).with_y(0.0);
            let length = d.length();
            (length > 1e-4).then(|| FlatRect {
                centre: flat((w[0] + w[1]) / 2.0),
                axis: flat(right_of(d / length)).normalize_or(Vec2::X),
                half: Vec2::new(half_width, length / 2.0),
            })
        })
        .collect()
}

/// The first body or claim on connector `c`'s path, other than `requester`, walkers (the strips
/// handle them) and AI cars granted at the node (the conflict table covers those).
pub(super) fn connector_clear(
    road: &RoadOccupancy,
    graph: &TrafficGraph,
    junction: Option<&Junction>,
    c: u32,
    requester: Entity,
    half_width: f32,
) -> Option<Entity> {
    let granted = |e: Entity| junction.is_some_and(|j| j.occupants.iter().any(|o| o.1 == e));
    let skip = |b: &crate::occupancy::RoadBody| {
        b.entity == requester
            || b.kind == BodyKind::Character
            || (matches!(b.kind, BodyKind::OnPathTraffic | BodyKind::Traffic) && granted(b.entity))
    };
    connector_rects(graph, c, half_width)
        .iter()
        .find_map(|r| road.blocked(r, skip, ClaimFilter::Except(requester)))
}

/// Another exit of `lane` after `current` (in `out` order) whose path is clear; no random draw.
pub(super) fn repick(
    road: &RoadOccupancy,
    graph: &TrafficGraph,
    junction: Option<&Junction>,
    lane: u32,
    current: u32,
    requester: Entity,
    half_width: f32,
) -> Option<u32> {
    let out = &graph.lane(lane).out;
    let k = out.iter().position(|&c| c == current).unwrap_or(0);
    (1..out.len())
        .map(|i| out[(k + i) % out.len()])
        .find(|&c| connector_clear(road, graph, junction, c, requester, half_width).is_none())
}

/// Point and tangent `s` m along connector `c` (before 0: on its source lane, past its end: on its
/// exit lane).
pub(super) fn path_pose(graph: &TrafficGraph, c: u32, s: f32) -> (Vec3, Vec3) {
    let conn = graph.connector(c);
    let length = graph.length(Segment::Connector(c));
    if s < 0.0 {
        let from = graph.lane(conn.from_lane);
        graph.pose(Segment::Lane(conn.from_lane), from.length + s)
    } else if s <= length {
        graph.pose(Segment::Connector(c), s)
    } else {
        graph.pose(Segment::Lane(conn.to_lane), s - length)
    }
}

/// The marks of a pass moved by `by` m (a new segment origin along the same path).
pub(super) fn shift_pass(manoeuvre: &mut Manoeuvre, by: f32) {
    if let Manoeuvre::Pass {
        hold_s,
        merge_s,
        end_s,
        ..
    } = manoeuvre
    {
        *hold_s += by;
        *merge_s += by;
        *end_s += by;
    }
}

/// A car standing behind a body on its connector (or at its stop line, holding the whole box) goes
/// around it inside the box (it then holds the whole box); `s` m along the connector (negative: on the
/// source lane before it): offset to one side of the connector line past the body, back on the line on its exit
/// lane. The path (the line and the offset band) must hold no body but the obstacle, and the offset
/// band no wall or raised sidewalk. `None`: no side fits.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_box_pass(
    road: &RoadOccupancy,
    spatial: &SpatialQuery,
    graph: &TrafficGraph,
    me: Entity,
    (c, s): (u32, f32),
    obstacle: Entity,
    cfg: &TrafficConfig,
    vehicle: &VehicleConfig,
) -> Option<Manoeuvre> {
    let body = road.body(obstacle)?;
    let half = vehicle.half_extents();
    let p = &cfg.pass;
    let length = graph.length(Segment::Connector(c));
    let exit = graph.lane(graph.connector(c).to_lane);
    let pitch = exit.left_gap?;
    let corners: Vec<Vec3> = match body.shape {
        Footprint::Rect(r) => {
            let (x, y) = (r.axis * r.half.x, r.axis.perp() * r.half.y);
            [x + y, x - y, -x - y, -x + y]
                .map(|d| Vec3::new(r.centre.x + d.x, 0.0, r.centre.y + d.y))
                .to_vec()
        }
        Footprint::Circle { centre, radius } => {
            let q = Vec3::new(centre.x, 0.0, centre.y);
            vec![
                q + Vec3::X * radius,
                q - Vec3::X * radius,
                q + Vec3::Z * radius,
                q - Vec3::Z * radius,
            ]
        }
    };
    // Along the path and across its line at that point, for each corner.
    let local: Vec<(f32, f32)> = corners
        .iter()
        .map(|&q| {
            let a = graph.project(Segment::Connector(c), q);
            let a = if a >= length - 1e-3 {
                length
                    + graph
                        .project(Segment::Lane(graph.connector(c).to_lane), q)
                        .max(0.0)
            } else {
                a
            };
            let (point, tangent) = path_pose(graph, c, a);
            (a, (q - point).with_y(0.0).dot(right_of(tangent)))
        })
        .collect();
    let (rear, front) = local
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), l| {
            (lo.min(l.0), hi.max(l.0))
        });
    let (left, right_edge) = local
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), l| {
            (lo.min(l.1), hi.max(l.1))
        });
    let merge_s = front + p.clearance + half.z;
    let end_s = merge_s + half.z + pitch * cfg.turn_speed / cfg.lateral.rate(cfg.turn_speed);
    if end_s + half.z > length + exit.stop {
        return None;
    }
    let slack = pitch / 2.0 - half.x;
    let top = vehicle.rest_height() + half.y;
    let step = 1.0;
    let samples: Vec<f32> = (0..)
        .map(|k| s - half.z + k as f32 * step)
        .take_while(|&a| a <= end_s + half.z)
        .collect();
    let band = |a: f32, lateral: f32, half_width: f32| {
        let (point, tangent) = path_pose(graph, c, a);
        FlatRect {
            centre: flat(point + right_of(tangent) * lateral),
            axis: flat(right_of(tangent)).normalize_or(Vec2::X),
            half: Vec2::new(half_width, step / 2.0 + 0.05),
        }
    };
    let free = |rect: &FlatRect| {
        road.blocked(
            rect,
            |b| b.entity == me || b.entity == obstacle,
            ClaimFilter::Except(me),
        )
        .is_none()
    };
    for side in [-1.0_f32, 1.0] {
        let extent = if side > 0.0 { right_edge } else { -left };
        let need = extent + p.clearance + half.x;
        let shift = pitch.max(need);
        if shift > pitch + slack {
            continue;
        }
        let offset = side * shift;
        let clear = samples.iter().all(|&a| {
            let wide = band(a, offset, half.x + p.clearance);
            free(&band(a, 0.0, half.x))
                && free(&band(a, offset / 2.0, half.x))
                && free(&wide)
                && world_clear(spatial, &band(a, offset, half.x), 0.05, top)
        });
        if clear {
            return Some(Manoeuvre::Pass {
                obstacle,
                offset,
                need,
                hold_s: rear - half.z - cfg.idm.min_gap,
                merge_s,
                end_s,
                go: true,
            });
        }
    }
    None
}
