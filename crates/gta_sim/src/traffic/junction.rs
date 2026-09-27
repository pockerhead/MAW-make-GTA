//! Intersection reservations: a car near its stop line queues for its connector; a connector is
//! granted first come, first served when no granted or earlier queued connector conflicts with it and
//! its destination lane has room ("don't block the box"): behind the last AI car there, and no other
//! vehicle (abandoned, taken, police) standing on that stretch. Only the first car of a lane queues,
//! and a grant is a lease: it lapses when its holder stands before the stop line for
//! `reservation_timeout` while a car waits for a conflicting connector, so a holder that cannot reach
//! its connector never locks the node.
//!
//! The box (TASK-032): a grant also needs its connector path clear of bodies the conflict table does
//! not know (`box_rules`; the path check drives the requester's body on from where it stands); a
//! holder on its connector or past its stop line whose path such a body has blocked, and that has not
//! moved for the lease while contested, is demoted to a waiter where it stands; a queue head whose
//! path a standing body blocks takes another exit (a car at the very start of its connector too).

use super::box_rules::{connector_clear, repick};
use super::drive::{Occupancy, Snap};
use super::{
    IdmConfig, Junction, Segment, TrafficGraph, TrafficIntersections, TrafficMode, TrafficRng,
};
use crate::occupancy::RoadOccupancy;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// Distance before the stop line at which a car at `speed` asks for its connector, m: its braking
/// distance to the stop line plus the jam gap IDM keeps before it, and one more jam gap so a car that
/// came to rest behind the line has asked already.
pub(super) fn request_distance(speed: f32, idm: &IdmConfig, half_length: f32) -> f32 {
    speed * speed / (2.0 * idm.comfortable_deceleration) + 2.0 * idm.min_gap + half_length
}

/// The grant of `car`'s chosen connector exists.
pub(super) fn has_grant(
    graph: &TrafficGraph,
    junctions: &TrafficIntersections,
    snap: &Snap,
) -> bool {
    snap.car
        .next
        .is_some_and(|c| junctions.granted(graph.connector(c).node, c, snap.entity))
}

/// What the box rules read.
pub(super) struct BoxInputs<'a> {
    pub road: &'a RoadOccupancy,
    /// Car half extents (across, along), m: the body the path check drives along a connector.
    pub body: Vec2,
    /// A body standing this long on a chosen path makes the queue head take another exit, s.
    pub stuck_seconds: f32,
}

/// A car this far along its connector may still switch to another exit of its lane, m. Geometry: all
/// exits start at the lane end along the lane and part as s²/2r (a tight right turn against a left
/// one); 1 m along they lie at most 0.42 m apart (seeds 1 and 7, every lane end), so the switch moves
/// the car sideways by no more than the 0.425 m its lane leaves beside it (w/2 − half width).
const REPICK_WITHIN: f32 = 1.0;

/// Where the car stands along connector `c`: its `s` on it, else 0 (on its source lane).
fn from_s(snap: &Snap, c: u32) -> f32 {
    if snap.car.segment == Segment::Connector(c) {
        snap.car.s
    } else {
        0.0
    }
}

/// Room left at the start of `lane` behind its last car, m.
fn room(graph: &TrafficGraph, occupancy: &Occupancy, lane: u32, half_length: f32) -> f32 {
    let length = graph.lane(lane).length;
    occupancy
        .get(&Segment::Lane(lane))
        .and_then(|cars| cars.first())
        .map_or(length, |&(s, _)| (s - half_length).min(length))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    graph: &TrafficGraph,
    junctions: &mut TrafficIntersections,
    snaps: &mut [Snap],
    occupancy: &Occupancy,
    idm: &IdmConfig,
    half_length: f32,
    tick: u64,
    lease: (u64, f32),
    rng: &mut TrafficRng,
    lane_start_free: &dyn Fn(u32, f32) -> bool,
    boxes: &BoxInputs,
) {
    let (lease_ticks, hold_speed) = lease;
    let index: HashMap<Entity, usize> = snaps
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.abandon)
        .map(|(k, s)| (s.entity, k))
        .collect();
    // Only the first car of a lane queues: a car behind it cannot reach the box first.
    let heads: HashSet<Entity> = occupancy
        .iter()
        .filter_map(|(&seg, cars)| {
            let Segment::Lane(l) = seg else {
                return None;
            };
            let length = graph.lane(l).length;
            cars.iter()
                .rev()
                .find(|c| c.0 <= length)
                .map(|c| snaps[c.1].entity)
        })
        .collect();
    // Release: the rear has left the connector, the car is gone or no longer AI, or its lease lapsed
    // in favour of a waiter for a conflicting connector (a holder past its stop line or on its
    // connector is demoted to a waiter where it stands).
    // Holders whose path a body the conflict table does not know has blocked for a while (walkers
    // are not such bodies: a car waiting for them on the crosswalk keeps its lease, TASK-033).
    let mut body_blocked: HashSet<(u32, Entity)> = HashSet::new();
    for junction in junctions.0.values() {
        for &(c, e) in &junction.occupants {
            // Read below only for a holder on its source lane or connector: on the exit lane its
            // path is behind it.
            let Some(&k) = index.get(&e) else {
                continue;
            };
            if snaps[k].car.segment == Segment::Lane(graph.connector(c).to_lane) {
                continue;
            }
            let from = from_s(&snaps[k], c);
            let blocker =
                connector_clear(boxes.road, graph, Some(junction), c, from, e, boxes.body)
                    .and_then(|b| boxes.road.body(b));
            if blocker.is_some_and(|b| b.standing >= boxes.stuck_seconds) {
                body_blocked.insert((c, e));
            }
        }
    }
    let mut demoted_all = Vec::new();
    for junction in junctions.0.values_mut() {
        let Junction {
            occupants,
            waiters,
            moved,
            whole,
        } = junction;
        waiters.retain(|&(_, e, c)| {
            let Some(&k) = index.get(&e) else {
                return false;
            };
            let car = &snaps[k].car;
            let from = Segment::Lane(graph.connector(c).from_lane);
            !matches!(car.mode, TrafficMode::Bailing { .. })
                && car.next == Some(c)
                && ((heads.contains(&e) && car.segment == from)
                    || car.segment == Segment::Connector(c))
        });
        let mut demoted = Vec::new();
        occupants.retain(|&(c, e)| {
            let Some(&k) = index.get(&e) else {
                return false;
            };
            let car = &snaps[k].car;
            let conn = graph.connector(c);
            let last = moved.entry(e).or_insert(tick);
            if car.speed >= hold_speed {
                *last = tick;
            }
            let contested = waiters.iter().any(|w| conn.conflicts.contains(&w.2));
            let stale = contested && tick - *last >= lease_ticks && whole.is_none_or(|w| w.0 != e);
            let stuck = stale && body_blocked.contains(&(c, e));
            match car.segment {
                Segment::Lane(l) if l == conn.from_lane => {
                    let past = car.s + half_length > graph.lane(l).stop;
                    if past && stuck && car.next == Some(c) {
                        demoted.push((tick, e, c));
                    }
                    car.next == Some(c) && !(stale && (!past || stuck))
                }
                Segment::Connector(k) if k == c => {
                    if stuck {
                        demoted.push((tick, e, c));
                    }
                    !stuck
                }
                Segment::Lane(l) if l == conn.to_lane => car.s - half_length < 0.0,
                _ => false,
            }
        });
        moved.retain(|e, _| occupants.iter().any(|o| o.1 == *e));
        demoted_all.extend(demoted.iter().map(|d| d.1));
        waiters.extend(demoted);
        // A whole-box grant (a connector pass) ends when its car is out of the box, and lapses with
        // its grant when the car has not moved for the lease (no way around was found).
        if let Some((e, to_lane)) = *whole {
            let idle = moved.get(&e).is_none_or(|&last| tick - last >= lease_ticks);
            let keep = index.get(&e).is_some_and(|&k| {
                let car = &snaps[k].car;
                let out = car.segment == Segment::Lane(to_lane) && car.s - half_length >= 0.0;
                !(out || matches!(car.mode, TrafficMode::Bailing { .. }))
            });
            if !keep || idle {
                *whole = None;
            }
            if idle {
                occupants.retain(|o| o.1 != e);
                moved.remove(&e);
            }
        }
    }
    for e in demoted_all {
        if let Some(&k) = index.get(&e) {
            snaps[k].car.waiting = Some(tick);
        }
    }
    // Requests near the stop line; a queue head whose path a standing body blocks takes another exit.
    for snap in snaps.iter_mut() {
        if snap.abandon || !matches!(snap.car.mode, TrafficMode::Kinematic | TrafficMode::Dynamic) {
            snap.car.waiting = None;
            continue;
        }
        // A kinematic car on a connector queues for it where it stands (a demoted holder, a car
        // placed there); at its very start it may still switch to another exit.
        let (lane, on_connector) = match snap.car.segment {
            Segment::Lane(lane) => (lane, false),
            Segment::Connector(c) if !snap.dynamic => (graph.connector(c).from_lane, true),
            Segment::Connector(_) => continue,
        };
        let at_start = on_connector && snap.car.s <= REPICK_WITHIN;
        if !on_connector
            && graph.lane(lane).stop - snap.car.s
                > request_distance(snap.car.speed, idm, half_length)
        {
            continue;
        }
        let out = &graph.lane(lane).out;
        let mut c = match snap.car.segment {
            Segment::Connector(k) => *snap.car.next.insert(k),
            Segment::Lane(_) => *snap
                .car
                .next
                .get_or_insert_with(|| out[rng.next_u32() as usize % out.len()]),
        };
        let node = graph.connector(c).node;
        if at_start || (!on_connector && heads.contains(&snap.entity)) {
            let here = junctions.0.get(&node);
            let from = from_s(snap, c);
            let stuck = connector_clear(boxes.road, graph, here, c, from, snap.entity, boxes.body)
                .and_then(|b| boxes.road.body(b))
                .is_some_and(|b| b.standing >= boxes.stuck_seconds);
            let other = if stuck {
                repick(
                    boxes.road,
                    graph,
                    here,
                    lane,
                    c,
                    from,
                    snap.entity,
                    boxes.body,
                )
            } else {
                None
            };
            if let Some(other) = other {
                c = other;
                snap.car.next = Some(c);
                if at_start {
                    // Same start pose: the car switches connectors and queues for the new one.
                    snap.car.segment = Segment::Connector(c);
                    junctions.release(snap.entity);
                }
                if let Some(j) = junctions.0.get_mut(&node) {
                    j.waiters.retain(|w| w.1 != snap.entity);
                }
            }
        }
        if on_connector && snap.car.segment != Segment::Connector(c) {
            continue;
        }
        let junction = junctions.0.entry(node).or_default();
        if junction.occupants.contains(&(c, snap.entity))
            || !(heads.contains(&snap.entity) || on_connector)
        {
            snap.car.waiting = None;
            continue;
        }
        let stamp = *snap.car.waiting.get_or_insert(tick);
        if !junction.waiters.iter().any(|w| w.1 == snap.entity) {
            junction.waiters.push((stamp, snap.entity, c));
        }
    }
    // Grants, first come first served; a queued connector blocks the later conflicting ones unless it
    // waits only for room past the box (a car stuck behind a standing one must not lock the node).
    let spacing = 2.0 * half_length + idm.min_gap;
    let nodes: Vec<u32> = junctions.0.keys().copied().collect();
    let mut into_lane: HashMap<u32, u32> = HashMap::new();
    for junction in junctions.0.values() {
        for &(c, _) in &junction.occupants {
            *into_lane.entry(graph.connector(c).to_lane).or_default() += 1;
        }
    }
    // Path checks against this tick's grants, before any grant changes them.
    // Per waiter: its path is clear (None), or the standing time of the body on it.
    let mut blocked: HashMap<(Entity, u32), Option<f32>> = HashMap::new();
    for junction in junctions.0.values() {
        for &(_, e, c) in &junction.waiters {
            let from = index.get(&e).map_or(0.0, |&k| from_s(&snaps[k], c));
            let body = connector_clear(boxes.road, graph, Some(junction), c, from, e, boxes.body);
            let standing = body.map(|b| boxes.road.body(b).map_or(0.0, |b| b.standing));
            blocked.insert((e, c), standing);
        }
    }
    for node in nodes {
        let Some(junction) = junctions.0.get_mut(&node) else {
            continue;
        };
        // A connector pass holds the whole box.
        if junction.whole.is_some() {
            continue;
        }
        junction
            .waiters
            .sort_by_key(|&(stamp, e, _)| (stamp, e.to_bits()));
        let mut blocking: Vec<u32> = junction.occupants.iter().map(|o| o.0).collect();
        let mut granted = Vec::new();
        for &(_, e, c) in &junction.waiters {
            let conn = graph.connector(c);
            if blocking.iter().any(|b| conn.conflicts.contains(b)) {
                blocking.push(c);
                continue;
            }
            let queued = into_lane.get(&conn.to_lane).copied().unwrap_or(0);
            let need = (queued + 1) as f32 * spacing;
            // Waiting for room past the box or for a body on its path: not blocking the node.
            if room(graph, occupancy, conn.to_lane, half_length) < need
                || !lane_start_free(conn.to_lane, need)
            {
                continue;
            }
            if let Some(standing) = blocked.get(&(e, c)).copied().flatten() {
                // A body standing on every way out: the car enters alone and goes around it.
                if standing >= boxes.stuck_seconds
                    && junction.occupants.is_empty()
                    && granted.is_empty()
                {
                    granted.push((c, e));
                    junction.whole = Some((e, conn.to_lane));
                    break;
                }
                continue;
            }
            blocking.push(c);
            *into_lane.entry(conn.to_lane).or_default() += 1;
            granted.push((c, e));
        }
        junction.waiters.retain(|w| !granted.contains(&(w.2, w.1)));
        junction.occupants.extend(granted.iter().copied());
        junction
            .moved
            .extend(granted.iter().map(|&(_, e)| (e, tick)));
        for (_, e) in granted {
            if let Some(&k) = index.get(&e) {
                snaps[k].car.waiting = None;
            }
        }
    }
}
