//! Going around a standing body (GDD §5.2, TASK-032): the queue head behind a car left in its lane, a
//! wreck, a dismounted police car or a person who stood long enough passes it on the curb lane
//! (avenues) or the opposite lane. Its claim (the road from its rear to the end of the pass) goes into
//! the road occupancy at the commit, while the car still waits on its line: oncoming cars stop before
//! it, the spawner and lane starts keep off it, and the car moves out once nobody is left in it.

use super::drive::Snap;
use super::lateral::right_of;
use super::{FlatRect, Manoeuvre, Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficLane};
use crate::occupancy::{
    BodyKind, Claim, ClaimFilter, Footprint, Hit, RoadBody, RoadOccupancy, flat, world_clear,
};
use crate::vehicle::VehicleConfig;
use avian3d::prelude::*;
use bevy::prelude::*;

/// The road along `lane` from `s0` to `s1` m, `lateral` m right of its line, `half_width` either side.
fn lane_rect(lane: &TrafficLane, s0: f32, s1: f32, lateral: f32, half_width: f32) -> FlatRect {
    let right = right_of(lane.dir);
    FlatRect {
        centre: flat(lane.from + lane.dir * ((s0 + s1) / 2.0) + right * lateral),
        axis: flat(right).normalize_or(Vec2::X),
        half: Vec2::new(half_width, ((s1 - s0) / 2.0).max(0.0)),
    }
}

/// The nearest hit ahead is a body to go around: close, standing long enough, not moving traffic.
pub(super) fn passable(hit: &Hit, cfg: &TrafficConfig) -> bool {
    let p = &cfg.pass;
    if hit.claim || hit.gap > p.trigger_gap {
        return false;
    }
    match hit.kind {
        BodyKind::Vehicle => hit.standing >= p.vehicle_seconds,
        BodyKind::Traffic => hit.dynamic && hit.standing >= p.vehicle_seconds,
        BodyKind::Character => hit.standing >= p.character_seconds,
        BodyKind::OnPathTraffic => false,
    }
}

/// The claim of a car in a pass: from its rear to the end of the pass, on the pass side.
pub fn derived_claim(
    graph: &TrafficGraph,
    car: &TrafficCar,
    entity: Entity,
    vehicle: &VehicleConfig,
    cfg: &TrafficConfig,
) -> Option<Claim> {
    let Manoeuvre::Pass { offset, end_s, .. } = car.manoeuvre else {
        return None;
    };
    let Segment::Lane(l) = car.segment else {
        return None;
    };
    let half = vehicle.half_extents();
    let lane = graph.lane(l);
    let (rear, end) = (car.s - half.z, end_s + half.z);
    (end > rear).then(|| Claim {
        owner: entity,
        rect: lane_rect(lane, rear, end, offset, half.x + cfg.pass.clearance),
        dir: flat(lane.dir),
    })
}

/// A body (other than `me` and passers ahead the same way) or an oncoming claim lies in `rect`; with
/// `oncoming_ok`, cars driving against `dir` are let through (they leave it or stop before it).
fn taken(
    road: &RoadOccupancy,
    rect: &FlatRect,
    me: Entity,
    dir: Vec2,
    oncoming_ok: bool,
    hold_speed: f32,
) -> bool {
    let same_way = |e: Entity| {
        road.claims()
            .iter()
            .any(|c| c.owner == e && c.dir.dot(dir) > 0.0)
    };
    let passing = |b: &RoadBody| {
        oncoming_ok
            && matches!(b.kind, BodyKind::OnPathTraffic | BodyKind::Traffic)
            && b.velocity.dot(dir) < -hold_speed
    };
    road.blocked(
        rect,
        |b| b.entity == me || same_way(b.entity) || passing(b),
        ClaimFilter::Against(dir),
    )
    .is_some()
}

/// Corners (or four rim points) of a footprint, flat at y 0.
fn outline(shape: &Footprint, lane: &TrafficLane) -> Vec<Vec3> {
    match *shape {
        Footprint::Rect(r) => {
            let (x, y) = (r.axis * r.half.x, r.axis.perp() * r.half.y);
            [x + y, x - y, -x - y, -x + y]
                .map(|d| Vec3::new(r.centre.x + d.x, 0.0, r.centre.y + d.y))
                .to_vec()
        }
        Footprint::Circle { centre, radius } => {
            let c = Vec3::new(centre.x, 0.0, centre.y);
            let right = right_of(lane.dir);
            vec![
                c + lane.dir * radius,
                c - lane.dir * radius,
                c + right * radius,
                c - right * radius,
            ]
        }
    }
}

/// (min, max) of `f` over the points.
fn extent(points: &[Vec3], f: impl Fn(Vec3) -> f32) -> (f32, f32) {
    points
        .iter()
        .map(|&q| f(q))
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        })
}

/// An oncoming car beyond the claim end (m along `lane`) still too close to stop hard before it.
fn cannot_stop(
    road: &RoadOccupancy,
    lane: &TrafficLane,
    claim_end: f32,
    offset: f32,
    cfg: &TrafficConfig,
    vehicle: &VehicleConfig,
) -> bool {
    let dir = flat(lane.dir);
    // Farthest an oncoming traffic car at the top desired speed can be and still not stop hard.
    let v0 = cfg.desired_speed.avenue.max(cfg.desired_speed.street);
    let reach = v0 * v0 / (2.0 * cfg.idm.max_deceleration) + cfg.idm.min_gap;
    let beyond = lane_rect(
        lane,
        claim_end,
        claim_end + reach,
        offset,
        vehicle.half_extents().x,
    );
    road.bodies().iter().any(|b| {
        let v = -b.velocity.dot(dir);
        if !matches!(b.kind, BodyKind::OnPathTraffic | BodyKind::Traffic) || v <= vehicle.hold_speed
        {
            return false;
        }
        let points = outline(&b.shape, lane);
        let near = extent(&points, |q| (q - lane.from).with_y(0.0).dot(lane.dir)).0;
        let stop = v * v / (2.0 * cfg.idm.max_deceleration) + cfg.idm.min_gap;
        near - claim_end < stop
            && road
                .blocked(&beyond, |o| o.entity != b.entity, ClaimFilter::None)
                .is_some()
    })
}

/// A car with a published pass claim may move out: nobody is left in the claim and no oncoming car is
/// too close to stop before it.
pub(super) fn may_go(
    road: &RoadOccupancy,
    graph: &TrafficGraph,
    snap: &Snap,
    cfg: &TrafficConfig,
    vehicle: &VehicleConfig,
) -> bool {
    let (Manoeuvre::Pass { offset, end_s, .. }, Segment::Lane(l)) =
        (snap.car.manoeuvre, snap.car.segment)
    else {
        return false;
    };
    let Some(claim) = derived_claim(graph, &snap.car, snap.entity, vehicle, cfg) else {
        return false;
    };
    let end = end_s + vehicle.half_extents().z;
    !taken(
        road,
        &claim.rect,
        snap.entity,
        claim.dir,
        false,
        vehicle.hold_speed,
    ) && !cannot_stop(road, graph.lane(l), end, offset, cfg, vehicle)
}

/// The pass around the body `hit` found ahead of `snap` on its lane, and its claim, if one side is
/// clear: the curb lane first (avenues), then the opposite lane.
pub(super) fn plan_pass(
    road: &RoadOccupancy,
    spatial: &SpatialQuery,
    graph: &TrafficGraph,
    snap: &Snap,
    hit: &Hit,
    cfg: &TrafficConfig,
    vehicle: &VehicleConfig,
) -> Option<(Manoeuvre, Claim)> {
    let Segment::Lane(l) = snap.car.segment else {
        return None;
    };
    let lane = graph.lane(l);
    let pitch = lane.left_gap?;
    let body = road.body(hit.entity)?;
    let half = vehicle.half_extents();
    let p = &cfg.pass;
    let right = right_of(lane.dir);
    let points = outline(&body.shape, lane);
    let (rear, front) = extent(&points, |q| (q - lane.from).with_y(0.0).dot(lane.dir));
    let (left, right_edge) = extent(&points, |q| (q - lane.from).with_y(0.0).dot(right));
    let hold_s = rear - half.z - cfg.idm.min_gap;
    let merge_s = front + p.clearance + half.z;
    let end_s = merge_s + half.z + pitch * p.speed / cfg.lateral.rate(p.speed);
    // A pass reaching the lane end would enter the junction off its line.
    if end_s + half.z > lane.stop {
        return None;
    }
    let slack = pitch / 2.0 - half.x;
    let dir = flat(lane.dir);
    let me = snap.entity;
    let top = vehicle.rest_height() + half.y;
    let stretch = lane.v0 * lane.v0 / (2.0 * cfg.idm.comfortable_deceleration) + cfg.idm.min_gap;
    for side in [1.0_f32, -1.0] {
        if side > 0.0 && !lane.curb_lane {
            continue;
        }
        let extent = if side > 0.0 { right_edge } else { -left };
        let need = extent + p.clearance + half.x;
        let shift = pitch.max(need);
        if shift > pitch + slack {
            continue;
        }
        let offset = side * shift;
        let claim = lane_rect(
            lane,
            snap.car.s - half.z,
            end_s + half.z,
            offset,
            half.x + p.clearance,
        );
        // Oncoming cars already in the claim drive out of it; nothing else may stand there.
        let free = !taken(road, &claim, me, dir, side < 0.0, vehicle.hold_speed);
        let clear = if side > 0.0 {
            // The car itself stays off the raised sidewalk (the claim's clearance may reach over it).
            let body = lane_rect(lane, snap.car.s - half.z, end_s + half.z, offset, half.x);
            world_clear(spatial, &body, 0.05, top)
        } else {
            // Turns with the oncoming flow: no pass starts while an oncoming car waits beyond it.
            let beyond = lane_rect(
                lane,
                end_s + half.z,
                end_s + half.z + stretch,
                offset,
                half.x,
            );
            let waiting = |b: &RoadBody| {
                matches!(b.kind, BodyKind::OnPathTraffic | BodyKind::Traffic) && b.standing > 0.0
            };
            road.blocked(&beyond, |b| !waiting(b), ClaimFilter::None)
                .is_none()
        };
        if free && clear {
            let pass = Manoeuvre::Pass {
                obstacle: hit.entity,
                offset,
                need,
                hold_s,
                merge_s,
                end_s,
                go: false,
            };
            return Some((
                pass,
                Claim {
                    owner: me,
                    rect: claim,
                    dir,
                },
            ));
        }
    }
    None
}

/// The pass is over: the rear is past the merge point and the car is back on its line.
pub(super) fn pass_done(car: &TrafficCar, half_length: f32) -> bool {
    matches!(car.manoeuvre, Manoeuvre::Pass { merge_s, .. }
        if car.s - half_length >= merge_s && car.lateral == 0.0)
}
