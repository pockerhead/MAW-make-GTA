//! AC2 (correctness, GDD §5.2): at a + intersection one car at a time holds any conflict point of two
//! crossing connector centre lines; the points are computed by the test from the graph poses, and cars
//! are fed into all four approaches for 6 seeds x 6400 ticks (turns from `TrafficRng`).
//! "Don't block the box" with a non-AI car: an abandoned car standing just past the box never lets a
//! car stop inside it.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::traffic::{
    Segment, TrafficCar, TrafficGraph, TrafficIntersections, TrafficMode, TrafficRng,
};
use traffic_support::*;

const TICKS: u32 = 6400;
/// Intersection centre: clear of every test-area block, arms of 12 m.
const CENTRE: Vec3 = Vec3::new(-22.0, 0.0, 22.0);
const HALF_BOX: f32 = 3.25;
const ARM: f32 = 12.0;
const OFFSET: f32 = 1.625;
const V0: f32 = 6.0;

fn right(d: Vec3) -> Vec3 {
    Vec3::new(-d.z, 0.0, d.x)
}

/// In lanes 0..4 (towards the centre from N, E, S, W), out lanes 4..8; 12 turning connectors at node
/// 0 plus one far connector per out lane back to its own in lane (never reached: the test removes
/// cars on the out lanes).
fn plus() -> (Vec<LaneSpec>, Vec<(u32, u32, u32)>) {
    let dirs = [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X];
    let mut lanes = Vec::new();
    for &d in &dirs {
        let travel = -d;
        let side = right(travel) * OFFSET;
        lanes.push((
            CENTRE + d * (HALF_BOX + ARM) + side,
            CENTRE + d * HALF_BOX + side,
            V0,
            0,
        ));
    }
    for (k, &d) in dirs.iter().enumerate() {
        let side = right(d) * OFFSET;
        lanes.push((
            CENTRE + d * HALF_BOX + side,
            CENTRE + d * (HALF_BOX + ARM) + side,
            V0,
            10 + k as u32,
        ));
    }
    let mut connectors = Vec::new();
    for i in 0..4u32 {
        for j in 0..4u32 {
            if i != j {
                connectors.push((i, 4 + j, 0));
            }
        }
    }
    for j in 0..4u32 {
        connectors.push((4 + j, j, 10 + j));
    }
    (lanes, connectors)
}

/// Points where the centre lines of two node-0 connectors with different source and destination
/// cross (sampled every 0.1 m, closest approach < 0.05 m).
fn conflict_points(graph: &TrafficGraph) -> Vec<Vec3> {
    let samples = |c: usize| -> Vec<Vec3> {
        let seg = Segment::Connector(c as u32);
        let length = graph.length(seg);
        let n = (length / 0.1).ceil() as usize;
        (0..=n)
            .map(|i| graph.pose(seg, length * i as f32 / n as f32).0)
            .collect()
    };
    let conns = graph.connectors();
    let mut points = Vec::new();
    for a in 0..conns.len() {
        for b in a + 1..conns.len() {
            let (ca, cb) = (&conns[a], &conns[b]);
            if ca.node != 0
                || cb.node != 0
                || ca.from_lane == cb.from_lane
                || ca.to_lane == cb.to_lane
            {
                continue;
            }
            let (sa, sb) = (samples(a), samples(b));
            let mut best = (f32::INFINITY, Vec3::ZERO);
            for &p in &sa {
                for &q in &sb {
                    let d = p.distance(q);
                    if d < best.0 {
                        best = (d, (p + q) / 2.0);
                    }
                }
            }
            if best.0 < 0.05 {
                points.push(best.1);
            }
        }
    }
    points
}

fn run_seed(seed: u64) -> [u32; 4] {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    app.world_mut().insert_resource(TrafficRng::seeded(seed));
    let graph = graph(&app);
    let points = conflict_points(&graph);
    assert!(
        points.len() >= 8,
        "GATE BROKEN: only {} conflict points",
        points.len()
    );
    let half = app
        .world()
        .resource::<gta_sim::vehicle::VehicleConfig>()
        .half_extents();
    let mut covered = vec![false; points.len()];
    let mut delivered = [0u32; 4];
    let mut origin = std::collections::HashMap::new();
    let before = stats(&app).casts;
    for tick in 1..=TICKS {
        // Feed every approach whose start is free; take cars off the far end of the out lanes.
        let now = cars(&mut app);
        let snapshot: Vec<_> = now.iter().map(|&c| (c, traffic_car(&app, c))).collect();
        for lane in 0..4u32 {
            let free = snapshot
                .iter()
                .all(|(_, t)| t.segment != Segment::Lane(lane) || t.s >= 4.0 * half.z + 2.0);
            if free {
                let car = spawn_traffic_car(&mut app, Segment::Lane(lane), half.z, V0);
                origin.insert(car, lane);
            }
        }
        for (car, t) in &snapshot {
            if let Segment::Lane(4..8) = t.segment
                && t.s > half.z + 2.0
            {
                app.world_mut().despawn(*car);
                delivered[origin[car] as usize] += 1;
            }
        }
        run_ticks(&mut app, 1);
        if tick == 64 {
            assert_traffic_ran(&app, before, 1, 64);
        }
        let rects: Vec<_> = cars(&mut app).iter().map(|&c| footprint(&app, c)).collect();
        for (k, &p) in points.iter().enumerate() {
            let holders = rects
                .iter()
                .filter(|r| {
                    let d = Vec2::new(p.x, p.z) - r.centre;
                    d.dot(r.axis).abs() <= r.half.x && d.dot(r.axis.perp()).abs() <= r.half.y
                })
                .count();
            assert!(
                holders <= 1,
                "seed {seed} tick {tick}: {holders} cars on conflict point {k} at {p}"
            );
            covered[k] |= holders == 1;
        }
    }
    for (k, c) in covered.iter().enumerate() {
        assert!(
            c,
            "GATE BROKEN: seed {seed}: conflict point {k} at {} never covered",
            points[k]
        );
    }
    delivered
}

#[test]
fn one_car_per_conflict_point() {
    for seed in 1..=6 {
        let delivered = run_seed(seed);
        eprintln!("seed {seed}: delivered per approach {delivered:?}");
        for (k, d) in delivered.iter().enumerate() {
            assert!(
                *d >= 3,
                "GATE BROKEN: seed {seed}: approach {k} delivered only {d} cars"
            );
        }
    }
}

/// Out lane of the south arm (index 4 + 2).
const BLOCKED_LANE: u32 = 6;
const BOX_TICKS: u32 = 6400;

/// One seed: an abandoned car 1 m into the south out lane, a car at the north stop line bound for
/// that lane, traffic fed into the other approaches. Every tick no AI car stands on a connector.
/// Returns the cars that drove a connector crossing the stuck car's one (the node is not locked).
fn run_blocked_box(seed: u64) -> u32 {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    app.world_mut().insert_resource(TrafficRng::seeded(seed));
    let graph = graph(&app);
    let half = app
        .world()
        .resource::<gta_sim::vehicle::VehicleConfig>()
        .half_extents();
    // Its rear 1 m past the box edge: an abandoned car, a dynamic body outside the AI occupancy.
    let obstacle = spawn_traffic_car(&mut app, Segment::Lane(BLOCKED_LANE), half.z + 1.0, 0.0);
    app.world_mut()
        .get_mut::<TrafficCar>(obstacle)
        .unwrap()
        .mode = TrafficMode::Abandoned;
    app.world_mut()
        .entity_mut(obstacle)
        .insert(RigidBody::Dynamic);
    let north_south = (0..graph.connectors().len() as u32)
        .find(|&c| graph.connector(c).from_lane == 0 && graph.connector(c).to_lane == BLOCKED_LANE)
        .expect("GATE BROKEN: no north-south connector");
    let crossing = graph.connector(north_south).conflicts.clone();
    let stuck = spawn_traffic_car(&mut app, Segment::Lane(0), ARM - half.z - 3.0, 0.0);
    app.world_mut().get_mut::<TrafficCar>(stuck).unwrap().next = Some(north_south);
    let mut crossed = std::collections::HashSet::new();
    let before = stats(&app).casts;
    for tick in 1..=BOX_TICKS {
        let snapshot: Vec<_> = cars(&mut app)
            .into_iter()
            .filter(|&c| c != obstacle && c != stuck)
            .map(|c| (c, traffic_car(&app, c)))
            .collect();
        for lane in 1..4u32 {
            let free = snapshot
                .iter()
                .all(|(_, t)| t.segment != Segment::Lane(lane) || t.s >= 4.0 * half.z + 2.0);
            if free {
                spawn_traffic_car(&mut app, Segment::Lane(lane), half.z, V0);
            }
        }
        for (car, t) in &snapshot {
            if let Segment::Lane(4..8) = t.segment
                && t.s > half.z + 2.0
            {
                app.world_mut().despawn(*car);
            }
        }
        run_ticks(&mut app, 1);
        if tick == 64 {
            assert_traffic_ran(&app, before, 1, 64);
            let junctions = app.world().resource::<TrafficIntersections>();
            assert!(
                junctions
                    .0
                    .values()
                    .any(|j| j.waiters.iter().any(|w| w.1 == stuck && w.2 == north_south)),
                "GATE BROKEN: seed {seed}: the north car does not wait for the blocked lane"
            );
        }
        for car in cars(&mut app) {
            let t = traffic_car(&app, car);
            if let Segment::Connector(c) = t.segment {
                assert!(
                    t.speed > 0.0,
                    "seed {seed} tick {tick}: {car} stands on connector {c} ({t:?})"
                );
                if crossing.contains(&c) {
                    crossed.insert(car);
                }
            }
        }
        assert!(
            app.world().get_entity(obstacle).is_ok() && app.world().get_entity(stuck).is_ok(),
            "GATE BROKEN: seed {seed}: a fixture car is gone"
        );
    }
    crossed.len() as u32
}

#[test]
fn abandoned_car_past_the_box_holds_no_car_inside() {
    for seed in 1..=3 {
        let crossed = run_blocked_box(seed);
        eprintln!("seed {seed}: {crossed} cars crossed the stuck car's path");
        // Measured over 100 s: 6..12 cars cross; a locked node lets only the 2 that were already
        // queued before the stuck car go.
        assert!(
            crossed >= 4,
            "seed {seed}: the node is locked, only {crossed} cars crossed the stuck car's path"
        );
    }
}
