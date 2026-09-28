//! Per-car manoeuvre decisions of `advance_traffic`: what the car senses ahead (the strips), and
//! when it starts, lets go of or drops a pass around a standing body (on its lane or in the box).

use super::box_rules::{path_pose, plan_box_pass, shift_pass};
use super::drive::Snap;
use super::lateral::{
    CORRIDOR_LATERAL_STEP, CORRIDOR_YAW_STEP_DEG, effective_lateral, right_of, target_lateral,
};
use super::pass::{may_go, passable, plan_pass};
use super::progress::{sensing_blocker, sensing_skips};
use super::sirens;
use super::{
    FlatRect, Manoeuvre, Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections,
    TrafficMode,
};
use crate::occupancy::{BodyKind, Claim, Footprint, Hit, RoadBody, RoadOccupancy, Strip, flat};
use crate::vehicle::VehicleConfig;
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashSet;

/// `me` holds the whole box of connector `c`'s node.
fn holds_box(junctions: &TrafficIntersections, graph: &TrafficGraph, c: u32, me: Entity) -> bool {
    junctions
        .0
        .get(&graph.connector(c).node)
        .is_some_and(|j| j.whole.is_some_and(|w| w.0 == me))
}

/// Nobody but `me` holds a grant at connector `c`'s node.
fn alone_in_box(
    junctions: &TrafficIntersections,
    graph: &TrafficGraph,
    c: u32,
    me: Entity,
) -> bool {
    junctions
        .0
        .get(&graph.connector(c).node)
        .is_none_or(|j| j.occupants.iter().all(|o| o.1 == me) && j.whole.is_none_or(|w| w.0 == me))
}

pub(super) fn passing(car: &TrafficCar) -> bool {
    matches!(car.manoeuvre, Manoeuvre::Pass { .. })
}

/// An offset the lateral law steps on a connector instead of decaying it (a pass, a rejoin).
pub(super) fn holds_offset(car: &TrafficCar) -> bool {
    passing(car) || car.manoeuvre == Manoeuvre::Rejoin
}

/// The first body or oncoming claim the car's body meets driving `reach` m on along connector `c`
/// from `s`, `lateral` m off the line (onto the exit lane past the end). Only a body with a part ahead
/// of the nose counts: one at the flank or swept by the rear on a curve never holds the car. The gap
/// is the free travel to first contact, bisected to 0.01 m.
fn sweep_ahead(
    graph: &TrafficGraph,
    road: &RoadOccupancy,
    (c, s): (u32, f32),
    lateral: f32,
    reach: f32,
    half: Vec3,
    skip: impl Fn(&RoadBody) -> bool,
) -> Option<Hit> {
    let pose = |d: f32| {
        let (point, tangent) = path_pose(graph, c, s + d);
        let right = right_of(tangent);
        let body = FlatRect {
            centre: flat(point + right * lateral),
            axis: flat(right).normalize_or(Vec2::X),
            half: Vec2::new(half.x, half.z),
        };
        (body, flat(tangent).normalize_or_zero())
    };
    let (body, fwd) = pose(0.0);
    let nose = body.centre + fwd * half.z;
    let behind_nose = |b: &RoadBody| match b.shape {
        Footprint::Circle { centre, radius } => (centre - nose).dot(fwd) + radius <= 0.0,
        Footprint::Rect(r) => {
            (r.centre - nose).dot(fwd)
                + r.half.x * r.axis.dot(fwd).abs()
                + r.half.y * r.axis.perp().dot(fwd).abs()
                <= 0.0
        }
    };
    let skip = |b: &RoadBody| skip(b) || behind_nose(b);
    // The recovery corridor's step law bounds rotation and travel between samples; a body slipping
    // between two samples is met on a later tick (the no-touch guarantee is the unmargined oracle's).
    let mut travels = vec![0.0];
    let mut d = 0.0;
    while d < reach {
        let step = CORRIDOR_LATERAL_STEP.min(reach - d);
        let turn = pose(d).1.angle_to(pose(d + step).1).abs();
        let n = (turn / CORRIDOR_YAW_STEP_DEG.to_radians()).ceil().max(1.0);
        travels.extend((1..=n as u32).map(|k| d + step * k as f32 / n));
        d += step;
    }
    let (rects, dirs): (Vec<FlatRect>, Vec<Vec2>) = travels.iter().map(|&d| pose(d)).unzip();
    let (k, mut hit) = road.first_in(&rects, &dirs, skip)?;
    if k == 0 {
        return Some(hit);
    }
    let (mut free, mut met) = (travels[k - 1], travels[k]);
    while met - free > 0.01 {
        let mid = (free + met) / 2.0;
        let (rect, dir) = pose(mid);
        match road.first_in(&[rects[0], rect], &[dirs[0], dir], skip) {
            Some((_, h)) => {
                met = mid;
                hit = h;
            }
            None => free = mid,
        }
    }
    hit.gap = free;
    Some(hit)
}

/// What a car senses ahead: the nearest body at its target offset (`ahead`), at its current offset
/// while the two differ (`beside`), and the relaxed blocker on the target strip or sweep, which the
/// other two skip.
pub(super) struct Sensed {
    pub ahead: Option<Hit>,
    pub beside: Option<Hit>,
    pub relaxed_hit: Option<Hit>,
}

/// Bodies ahead from the nose: at the target offset over the sensing reach (on a lane a strip, on a
/// connector the body swept along the path), and on a strip at the current offset over the rest of
/// the lateral move plus twice the jam gap while the two differ. A car on its path line with no
/// manoeuvre skips AI cars on their path lines (the path occupancy holds those; a strip into a box
/// must not brake for crossing cars), except one `held` on a connector without a grant: no grant
/// keeps crossing cars off it. A relaxed kinematic car skips its blocker (`relaxed_hit` alone sees it).
pub(super) fn sense(
    graph: &TrafficGraph,
    road: &RoadOccupancy,
    cfg: &TrafficConfig,
    snap: &Snap,
    half: Vec3,
    held: &HashSet<Entity>,
) -> Sensed {
    let car = &snap.car;
    let (point, tangent) = graph.pose(car.segment, car.s);
    let right = right_of(tangent);
    let current = if snap.dynamic {
        (snap.position - point).with_y(0.0).dot(right)
    } else {
        effective_lateral(graph, car.segment, car.s, car.lateral, holds_offset(car))
    };
    let (target, reach) = match car.segment {
        Segment::Lane(_) => (target_lateral(car, car.s - half.z), cfg.sense_distance),
        Segment::Connector(_) if holds_offset(car) => {
            (target_lateral(car, car.s - half.z), cfg.turn_sense_distance)
        }
        Segment::Connector(_) => (current, cfg.turn_sense_distance),
    };
    let plain = car.lateral == 0.0 && target == 0.0 && car.manoeuvre == Manoeuvre::None;
    let me = snap.entity;
    let relaxed = sensing_blocker(snap);
    let skip = |b: &RoadBody| {
        b.entity == me
            || sensing_skips(snap, b.entity)
            || (plain && b.kind == BodyKind::OnPathTraffic && !held.contains(&b.entity))
    };
    let strip = |lateral: f32, length: f32| Strip {
        origin: flat(point + right * lateral + tangent * half.z),
        dir: flat(tangent).normalize_or_zero(),
        length,
        half_width: half.x,
    };
    let along = |skip: &dyn Fn(&RoadBody) -> bool| match car.segment {
        Segment::Connector(c) => sweep_ahead(graph, road, (c, car.s), target, reach, half, skip),
        Segment::Lane(_) => road.first_along(&strip(target, reach), skip),
    };
    let ahead = along(&skip);
    let relaxed_hit = relaxed
        .and_then(|b| along(&|x: &RoadBody| x.entity != b).filter(|h| h.entity == b && !h.claim));
    let beside = ((target - current).abs() > 1e-4).then(|| {
        let v = car.speed;
        let to_go = v * (target - current).abs() / cfg.lateral.rate(v);
        let length = (to_go + 2.0 * cfg.idm.min_gap).min(reach);
        road.first_along(&strip(current, length), skip)
    });
    Sensed {
        ahead,
        beside: beside.flatten(),
        relaxed_hit,
    }
}

/// A new manoeuvre for a car, the claim it publishes, the connector whose whole box it takes, and
/// seconds to ignore sirens.
pub(super) struct Update {
    pub manoeuvre: Manoeuvre,
    pub claim: Option<Claim>,
    pub whole_box: Option<u32>,
    pub deaf: f32,
}

/// The manoeuvre change of `snap` this tick, given the nearest body `ahead` on its strip and, for a
/// relaxed car, its blocker there (`relaxed_hit`).
pub(super) fn plan(
    world: (
        &RoadOccupancy,
        &SpatialQuery,
        &TrafficGraph,
        &TrafficIntersections,
    ),
    snap: &Snap,
    (ahead, relaxed_hit): (Option<Hit>, Option<Hit>),
    clock: (u64, f32),
    configs: (&TrafficConfig, &VehicleConfig),
) -> Option<Update> {
    let (road, spatial, graph, _) = world;
    let (cfg, vcfg) = configs;
    let car = &snap.car;
    let idle = !snap.dynamic
        && car.mode == TrafficMode::Kinematic
        && car.manoeuvre == Manoeuvre::None
        && car.lateral == 0.0;
    // Sirens first: a yielding car neither passes nor goes on.
    if let Some((manoeuvre, deaf)) =
        sirens::update(road, spatial, graph, snap, idle, clock.0, clock.1, configs)
    {
        return Some(Update {
            manoeuvre,
            claim: None,
            whole_box: None,
            deaf,
        });
    }
    // A published pass: out once its claim is empty, dropped if the obstacle moved off.
    if let Manoeuvre::Pass {
        go: false,
        obstacle,
        ..
    } = car.manoeuvre
    {
        let mut next = car.manoeuvre;
        if road.body(obstacle).is_none_or(|b| b.standing == 0.0) {
            next = Manoeuvre::None;
        } else if let (Manoeuvre::Pass { go, .. }, true) =
            (&mut next, may_go(road, graph, snap, cfg, vcfg))
        {
            *go = true;
        }
        return Some(Update {
            manoeuvre: next,
            claim: None,
            whole_box: None,
            deaf: 0.0,
        });
    }
    // A relaxed car takes a clean lane pass that can go at once; else it drives through its blocker on
    // its own line (in the box never an offset: the conflict table keeps co-granted cars apart only on
    // their lines, and a whole-box pass lapses under the lease).
    if let Some(hit) = relaxed_hit.filter(|_| idle) {
        if let Some((mut manoeuvre, claim)) = plan_pass(road, spatial, graph, snap, &hit, cfg, vcfg)
        {
            let probe = Snap {
                car: TrafficCar {
                    manoeuvre,
                    ..snap.car
                },
                ..*snap
            };
            if let (Manoeuvre::Pass { go, .. }, true) =
                (&mut manoeuvre, may_go(road, graph, &probe, cfg, vcfg))
            {
                *go = true;
                return Some(Update {
                    manoeuvre,
                    claim: Some(claim),
                    whole_box: None,
                    deaf: 0.0,
                });
            }
        }
        return None;
    }
    let hit = ahead.filter(|hit| idle && passable(hit, cfg))?;
    if let Some((manoeuvre, claim)) = plan_pass(road, spatial, graph, snap, &hit, cfg, vcfg) {
        return Some(Update {
            manoeuvre,
            claim: Some(claim),
            whole_box: None,
            deaf: 0.0,
        });
    }
    box_pass(world, snap, &hit, configs)
}

/// Standing behind a body in the box, alone there: around it, holding the whole box.
fn box_pass(
    world: (
        &RoadOccupancy,
        &SpatialQuery,
        &TrafficGraph,
        &TrafficIntersections,
    ),
    snap: &Snap,
    hit: &Hit,
    configs: (&TrafficConfig, &VehicleConfig),
) -> Option<Update> {
    let (road, spatial, graph, junctions) = world;
    let (cfg, vcfg) = configs;
    let car = &snap.car;
    let me = snap.entity;
    let (c, at, origin) = match car.segment {
        Segment::Connector(c) => (c, car.s, 0.0),
        // At its stop line with the whole box granted (its every exit blocked).
        Segment::Lane(l) => {
            let c = car.next.filter(|&c| holds_box(junctions, graph, c, me))?;
            let length = graph.lane(l).length;
            (c, car.s - length, length)
        }
    };
    if hit.kind == BodyKind::Character || !alone_in_box(junctions, graph, c, me) {
        return None;
    }
    let mut manoeuvre = plan_box_pass(road, spatial, graph, me, (c, at), hit.entity, cfg, vcfg)?;
    shift_pass(&mut manoeuvre, origin);
    Some(Update {
        manoeuvre,
        claim: None,
        whole_box: Some(c),
        deaf: 0.0,
    })
}
