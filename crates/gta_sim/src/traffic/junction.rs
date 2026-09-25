//! Intersection reservations: a car near its stop line queues for its connector; a connector is
//! granted first come, first served when no granted or earlier queued connector conflicts with it and
//! its destination lane has room ("don't block the box"): behind the last AI car there, and no other
//! vehicle (abandoned, taken, police) standing on that stretch.

use super::drive::{Occupancy, Snap};
use super::{IdmConfig, Segment, TrafficGraph, TrafficIntersections, TrafficMode, TrafficRng};
use bevy::prelude::*;
use std::collections::HashMap;

/// Distance before the lane end at which a car at `speed` asks for its connector, m: its braking
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
    rng: &mut TrafficRng,
    lane_start_free: &dyn Fn(u32, f32) -> bool,
) {
    let index: HashMap<Entity, usize> = snaps
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.abandon)
        .map(|(k, s)| (s.entity, k))
        .collect();
    // Release: the rear has left the connector, or the car is gone or no longer AI.
    for junction in junctions.0.values_mut() {
        junction.occupants.retain(|&(c, e)| {
            let Some(&k) = index.get(&e) else {
                return false;
            };
            let car = &snaps[k].car;
            let conn = graph.connector(c);
            match car.segment {
                Segment::Lane(l) if l == conn.from_lane => car.next == Some(c),
                Segment::Connector(k) => k == c,
                Segment::Lane(l) if l == conn.to_lane => car.s - half_length < 0.0,
                Segment::Lane(_) => false,
            }
        });
        junction.waiters.retain(|&(_, e, c)| {
            let Some(&k) = index.get(&e) else {
                return false;
            };
            let car = &snaps[k].car;
            !matches!(car.mode, TrafficMode::Bailing { .. })
                && car.next == Some(c)
                && car.segment == Segment::Lane(graph.connector(c).from_lane)
        });
    }
    // Requests near the stop line.
    for snap in snaps.iter_mut() {
        let Segment::Lane(lane) = snap.car.segment else {
            continue;
        };
        if snap.abandon || !matches!(snap.car.mode, TrafficMode::Kinematic | TrafficMode::Dynamic) {
            snap.car.waiting = None;
            continue;
        }
        let length = graph.lane(lane).length;
        if length - snap.car.s > request_distance(snap.car.speed, idm, half_length) {
            continue;
        }
        let out = &graph.lane(lane).out;
        let c = *snap
            .car
            .next
            .get_or_insert_with(|| out[rng.next_u32() as usize % out.len()]);
        let node = graph.connector(c).node;
        let junction = junctions.0.entry(node).or_default();
        if junction.occupants.contains(&(c, snap.entity)) {
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
    for node in nodes {
        let Some(junction) = junctions.0.get_mut(&node) else {
            continue;
        };
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
            if room(graph, occupancy, conn.to_lane, half_length) < need
                || !lane_start_free(conn.to_lane, need)
            {
                continue;
            }
            blocking.push(c);
            *into_lane.entry(conn.to_lane).or_default() += 1;
            granted.push((c, e));
        }
        junction.waiters.retain(|w| !granted.contains(&(w.2, w.1)));
        junction.occupants.extend(granted.iter().copied());
        for (_, e) in granted {
            if let Some(&k) = index.get(&e) {
                snaps[k].car.waiting = None;
            }
        }
    }
}
