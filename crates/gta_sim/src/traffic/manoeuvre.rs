//! Per-car manoeuvre decisions of `advance_traffic`: what the car senses ahead (the strips), and
//! when it starts, lets go of or drops a pass around a standing body (on its lane or in the box).

use super::box_rules::{plan_box_pass, shift_pass};
use super::drive::Snap;
use super::lateral::{effective_lateral, right_of, target_lateral};
use super::pass::{may_go, passable, plan_pass};
use super::sirens;
use super::{
    Manoeuvre, Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections, TrafficMode,
};
use crate::occupancy::{BodyKind, Claim, Hit, RoadOccupancy, Strip, flat};
use crate::vehicle::VehicleConfig;
use avian3d::prelude::*;
use bevy::prelude::*;

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

/// Bodies ahead on strips from the nose: at the target offset over the
/// sensing reach, and at the current offset over the rest of the lateral move plus twice the jam gap
/// while the two differ. A car on its path line with no manoeuvre skips AI cars on their path lines
/// (the path occupancy holds those; a straight strip into a box must not brake for crossing cars).
pub(super) fn sense(
    graph: &TrafficGraph,
    road: &RoadOccupancy,
    cfg: &TrafficConfig,
    snap: &Snap,
    half: Vec3,
) -> (Option<Hit>, Option<Hit>) {
    let car = &snap.car;
    let (point, tangent) = graph.pose(car.segment, car.s);
    let right = right_of(tangent);
    let current = if snap.dynamic {
        (snap.position - point).with_y(0.0).dot(right)
    } else {
        effective_lateral(graph, car.segment, car.s, car.lateral, passing(car))
    };
    let (target, reach) = match car.segment {
        Segment::Lane(_) => (target_lateral(car, car.s - half.z), cfg.sense_distance),
        Segment::Connector(_) if passing(car) => {
            (target_lateral(car, car.s - half.z), cfg.turn_sense_distance)
        }
        Segment::Connector(_) => (current, cfg.turn_sense_distance),
    };
    let plain = car.lateral == 0.0 && target == 0.0 && car.manoeuvre == Manoeuvre::None;
    let me = snap.entity;
    let skip = |b: &crate::occupancy::RoadBody| {
        b.entity == me || (plain && b.kind == BodyKind::OnPathTraffic)
    };
    let strip = |lateral: f32, length: f32| Strip {
        origin: flat(point + right * lateral + tangent * half.z),
        dir: flat(tangent).normalize_or_zero(),
        length,
        half_width: half.x,
    };
    let ahead = road.first_along(&strip(target, reach), skip);
    if (target - current).abs() <= 1e-4 {
        return (ahead, None);
    }
    let v = car.speed;
    let to_go = v * (target - current).abs() / cfg.lateral.rate(v);
    let length = (to_go + 2.0 * cfg.idm.min_gap).min(reach);
    (ahead, road.first_along(&strip(current, length), skip))
}

/// A new manoeuvre for a car, the claim it publishes, the connector whose whole box it takes, and
/// seconds to ignore sirens.
pub(super) struct Update {
    pub manoeuvre: Manoeuvre,
    pub claim: Option<Claim>,
    pub whole_box: Option<u32>,
    pub deaf: f32,
}

/// The manoeuvre change of `snap` this tick, given the nearest body `ahead` on its strip.
pub(super) fn plan(
    world: (
        &RoadOccupancy,
        &SpatialQuery,
        &TrafficGraph,
        &TrafficIntersections,
    ),
    snap: &Snap,
    ahead: Option<Hit>,
    clock: (u64, f32),
    configs: (&TrafficConfig, &VehicleConfig),
) -> Option<Update> {
    let (road, spatial, graph, junctions) = world;
    let (cfg, vcfg) = configs;
    let car = &snap.car;
    let me = snap.entity;
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
    let hit = ahead.filter(|hit| idle && passable(hit, cfg))?;
    if let Some((manoeuvre, claim)) = plan_pass(road, spatial, graph, snap, &hit, cfg, vcfg) {
        return Some(Update {
            manoeuvre,
            claim: Some(claim),
            whole_box: None,
            deaf: 0.0,
        });
    }
    // Standing behind a body in the box, alone there: around it, holding the whole box.
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
