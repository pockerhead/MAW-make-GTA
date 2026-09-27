//! AC2 (correctness, GDD §5.2): at a + intersection one car at a time holds any conflict point of two
//! crossing connector centre lines; the points are computed by the test from the graph poses, and cars
//! are fed into all four approaches for 6 seeds x 6400 ticks (turns from `TrafficRng`).
//! "Don't block the box" with a non-AI car: an abandoned car standing just past the box never lets a
//! car stop inside it.
//! Sensing on a connector (TASK-037) sweeps the car's body along its path: a parked car beside the
//! curve (in the straight strip, clear of the sweep) never stops a turn; one on the sweep holds the car
//! at the jam gap measured along the path; a body at the outer rear flank (behind the nose) never holds
//! it; a car held on a crossing connector without a grant does (no grant keeps others off it).

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::traffic::{
    FlatRect, Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections, TrafficMode,
    TrafficRng,
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

/// Lease: the north head holds its grant while a walker stands before its bumper, short of the stop
/// line; the east head waits for a crossing connector. The grant lapses exactly `reservation_timeout`
/// after it was given (one tick of slack), the east car gets it and the north car queues again.
#[test]
fn contested_lease_lapses_to_the_waiter() {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let graph = graph(&app);
    let half = app
        .world()
        .resource::<gta_sim::vehicle::VehicleConfig>()
        .half_extents()
        .z;
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let timeout = app
        .world()
        .resource::<gta_sim::traffic::TrafficConfig>()
        .reservation_timeout;
    let lease = (timeout / step).ceil() as u32;
    let find = |from: u32, to: u32| {
        (0..graph.connectors().len() as u32)
            .find(|&c| graph.connector(c).from_lane == from && graph.connector(c).to_lane == to)
            .expect("GATE BROKEN: missing connector")
    };
    let (north, east) = (find(0, 6), find(1, 7));
    assert!(
        graph.connector(north).conflicts.contains(&east),
        "GATE BROKEN: the straight connectors do not cross"
    );
    // Nose 3 m before the stop line (inside the request distance at rest), a walker 1.8 m before it.
    let s = graph.lane(0).stop - half - 3.0;
    let holder = spawn_traffic_car(&mut app, Segment::Lane(0), s, 0.0);
    app.world_mut().get_mut::<TrafficCar>(holder).unwrap().next = Some(north);
    let walker_at = graph.pose(Segment::Lane(0), s + half + 1.8).0 + Vec3::Y * 0.05;
    let walker = spawn_dummy(&mut app, walker_at);
    let granted = |app: &App, c: u32, e: Entity| {
        app.world()
            .resource::<TrafficIntersections>()
            .granted(0, c, e)
    };
    let waits = |app: &App, e: Entity| {
        app.world()
            .resource::<TrafficIntersections>()
            .0
            .values()
            .any(|j| j.waiters.iter().any(|w| w.1 == e))
    };
    run_ticks(&mut app, 1);
    assert!(
        granted(&app, north, holder),
        "GATE BROKEN: the north car was not granted on its first tick"
    );
    let home = position_of(&app, walker);
    let waiter = spawn_traffic_car(&mut app, Segment::Lane(1), s, 0.0);
    app.world_mut().get_mut::<TrafficCar>(waiter).unwrap().next = Some(east);
    let before = stats(&app).casts;
    let mut lapsed = None;
    for tick in 1..=2 * lease {
        run_ticks(&mut app, 1);
        let car = traffic_car(&app, holder);
        assert!(
            car.segment == Segment::Lane(0)
                && car.s + half <= graph.lane(0).stop
                && car.speed < 0.5,
            "GATE BROKEN: tick {tick}: the north car is not standing before its stop line ({car:?})"
        );
        assert!(
            (position_of(&app, walker) - home).with_y(0.0).length() < 0.5,
            "GATE BROKEN: tick {tick}: the walker moved"
        );
        if tick == 64 {
            assert_traffic_ran(&app, before, 2, 64);
            assert!(
                waits(&app, waiter),
                "GATE BROKEN: the east car does not wait for its connector"
            );
        }
        if !granted(&app, north, holder) {
            lapsed = Some(tick);
            break;
        }
    }
    // Granted on the tick before the loop: the lease runs out on loop tick `lease`.
    let lapsed =
        lapsed.unwrap_or_else(|| panic!("the north car kept its grant {} ticks", 2 * lease));
    eprintln!("lease {lease} ticks, the north car lost its grant on tick {lapsed}");
    assert!(
        (lease..=lease + 1).contains(&lapsed),
        "the grant lapsed on tick {lapsed}, the lease is {lease} ticks"
    );
    assert!(
        granted(&app, east, waiter),
        "the east car did not get the lapsed grant"
    );
    assert!(waits(&app, holder), "the north car did not queue again");
}

/// The node-0 connector from `from` to `to`.
fn turn(graph: &TrafficGraph, from: u32, to: u32) -> u32 {
    (0..graph.connectors().len() as u32)
        .find(|&c| graph.connector(c).from_lane == from && graph.connector(c).to_lane == to)
        .expect("GATE BROKEN: missing connector")
}

/// Point and tangent `d` m along connector `c`, onto its exit lane past the end.
fn pose_at(graph: &TrafficGraph, c: u32, d: f32) -> (Vec3, Vec3) {
    let length = graph.length(Segment::Connector(c));
    if d <= length {
        graph.pose(Segment::Connector(c), d)
    } else {
        graph.pose(Segment::Lane(graph.connector(c).to_lane), d - length)
    }
}

/// The car body (unmargined) posed `d` m along connector `c`, onto its exit lane past the end.
fn body_at(graph: &TrafficGraph, c: u32, d: f32, half: Vec3) -> FlatRect {
    let (point, tangent) = pose_at(graph, c, d);
    FlatRect::of(
        point,
        Quat::from_rotation_y(gta_sim::combat::aim_yaw(tangent)),
        Vec2::new(half.x, half.z),
    )
}

/// Smallest growth of `a` (0.01 m steps, up to 3 m) that overlaps `b`.
fn clearance(a: &FlatRect, b: &FlatRect) -> f32 {
    (0..=300)
        .map(|k| k as f32 * 0.01)
        .find(|&g| {
            obb_overlap(
                &FlatRect {
                    half: a.half + Vec2::splat(g),
                    ..*a
                },
                b,
            )
        })
        .unwrap_or(3.0)
}

/// Travel from `s` along connector `c` (0.01 m steps) before the body first overlaps `other`.
fn travel_gap(graph: &TrafficGraph, c: u32, s: f32, half: Vec3, other: &FlatRect) -> f32 {
    (0..2000)
        .map(|k| k as f32 * 0.01)
        .find(|&d| obb_overlap(&body_at(graph, c, s + d + 0.01, half), other))
        .unwrap_or(20.0)
}

/// A car standing at `s0` on connector `c`, granted (in the node's occupants), `next` set.
fn granted_on(app: &mut App, c: u32, s0: f32) -> Entity {
    let node = graph(app).connector(c).node;
    let car = spawn_traffic_car(app, Segment::Connector(c), s0, 0.0);
    app.world_mut().get_mut::<TrafficCar>(car).unwrap().next = Some(c);
    app.world_mut()
        .resource_mut::<TrafficIntersections>()
        .0
        .entry(node)
        .or_default()
        .occupants
        .push((c, car));
    car
}

const S0: f32 = 0.5;

/// A parked car ahead of the right turn from the north, on the straight line of the strip the old
/// sensing cast from the nose at `S0` (6 m, half width), its footprint at least 0.35 m clear of the
/// car body swept along the turn: the car turns without stopping and never comes nearer to it than
/// the switch's straight-line drift allows.
#[test]
fn a_car_beside_the_curve_does_not_stop_a_turn() {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let g = graph(&app);
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let half = app
        .world()
        .resource::<gta_sim::vehicle::VehicleConfig>()
        .half_extents();
    let c = turn(&g, 0, 7);
    let length = g.length(Segment::Connector(c));
    let (point, tangent) = g.pose(Segment::Connector(c), S0);
    let nose = point + tangent * half.z;
    let strip = FlatRect::of(
        nose + tangent * (cfg.turn_sense_distance / 2.0),
        Quat::from_rotation_y(gta_sim::combat::aim_yaw(tangent)),
        Vec2::new(half.x, cfg.turn_sense_distance / 2.0),
    );
    let sweep: Vec<FlatRect> = (0..=((length + cfg.turn_sense_distance) / 0.05) as u32)
        .map(|k| body_at(&g, c, k as f32 * 0.05, half))
        .collect();
    let along_x = Quat::from_rotation_y(gta_sim::combat::aim_yaw(Vec3::X));
    let parked_rect = |at: Vec3| FlatRect::of(at, along_x, Vec2::new(half.x, half.z));
    let clear_of_sweep = |r: &FlatRect| sweep.iter().map(|b| clearance(r, b)).fold(3.0, f32::min);
    let at = (0..120)
        .map(|k| nose + tangent * (half.x + k as f32 * 0.05))
        .find(|&at| clear_of_sweep(&parked_rect(at)) >= 0.40)
        .expect("GATE BROKEN: no spot clear of the sweep ahead of the turn");
    let rect = parked_rect(at);
    assert!(
        obb_overlap(&rect, &strip),
        "GATE BROKEN: the parked car at {at} is outside the old straight strip"
    );
    let parked = park_car(&mut app, at, Vec3::X);
    app.world_mut().entity_mut(parked).insert(Sleeping);
    let car = granted_on(&mut app, c, S0);
    let switches = stats(&app).switches_by_cause;
    let mut oracle = Footprints::new(&app);
    let limit = ((length / cfg.turn_speed + 2.0) * 64.0) as u32;
    let (mut moved, mut stopped, mut out, mut closest) = (false, None, None, f32::INFINITY);
    for tick in 0..limit {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let t = traffic_car(&app, car);
        closest = closest.min(clearance(&footprint(&app, car), &footprint(&app, parked)));
        if t.segment == Segment::Connector(c) {
            moved |= t.speed > 0.0;
            if moved && t.speed == 0.0 && stopped.is_none() {
                stopped = Some((tick, t.s));
            }
        } else {
            out = Some(tick);
            break;
        }
    }
    let drift = cfg.switch.skin
        + (cfg.turn_speed * cfg.switch.horizon_seconds).powi(2) / (2.0 * 0.707 * 1.625);
    eprintln!(
        "beside the curve: parked at {at} ({:.2} m clear of the sweep); stopped {stopped:?}, out on tick {out:?} (limit {limit}); closest {closest:.2} m (drift bound {drift:.2}); switches {:?} -> {:?}",
        clear_of_sweep(&rect),
        switches,
        stats(&app).switches_by_cause
    );
    oracle.assert_clean("beside the curve");
    let mut failures = Vec::new();
    if stopped.is_some() {
        failures.push(format!(
            "the turning car stopped on the connector: {stopped:?}"
        ));
    }
    if out.is_none() {
        failures.push(format!("not on the exit lane within {limit} ticks"));
    }
    if stats(&app).switches_by_cause != switches {
        failures.push("a switch to Dynamic happened".to_string());
    }
    if closest <= drift {
        failures.push(format!("closest approach {closest:.2} m <= {drift:.2} m"));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Named fixture mutation: `pass.vehicle_seconds` 1000 s (no demotion, no box pass while the car
/// settles). A parked car on the left turn's sweep 3.5 m of travel ahead of the car at `S0`: once the
/// car has stood 0.5 s, the travel gap measured by the test along the path is the jam gap.
#[test]
fn a_car_on_the_curve_stops_at_the_jam_gap() {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    set_traffic(&mut app, |c| c.pass.vehicle_seconds = 1000.0);
    let g = graph(&app);
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let half = app
        .world()
        .resource::<gta_sim::vehicle::VehicleConfig>()
        .half_extents();
    let c = turn(&g, 0, 5);
    // Parked along the path where the travel gap from S0 is 3.5 m (0.01 m search).
    let spot = (0..1000)
        .map(|k| S0 + 2.0 * half.z + k as f32 * 0.01)
        .find(|&d| travel_gap(&g, c, S0, half, &body_at(&g, c, d, half)) >= 3.5)
        .expect("GATE BROKEN: no spot 3.5 m ahead");
    let (at, dir) = pose_at(&g, c, spot);
    let parked = park_car(&mut app, at, dir);
    app.world_mut().entity_mut(parked).insert(Sleeping);
    let rect = footprint(&app, parked);
    let start_gap = travel_gap(&g, c, S0, half, &rect);
    let car = granted_on(&mut app, c, S0);
    let mut oracle = Footprints::new(&app);
    let (mut moved, mut still, mut rest) = (false, 0, None);
    for tick in 0..10 * 64 {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let t = traffic_car(&app, car);
        moved |= t.speed > 0.0;
        still = if moved && t.speed == 0.0 {
            still + 1
        } else {
            0
        };
        if still >= 32 {
            rest = Some((t.segment, t.s));
            break;
        }
    }
    oracle.assert_clean("on the curve");
    let (seg, s) = rest.expect("the car never came to rest behind the parked car");
    assert_eq!(
        seg,
        Segment::Connector(c),
        "GATE BROKEN: it came to rest off the connector"
    );
    let gap = travel_gap(&g, c, s, half, &rect);
    eprintln!(
        "on the curve: parked at {spot:.2} m along (start gap {start_gap:.2}); rest at s {s:.3}, travel gap {gap:.3} (jam gap {})",
        cfg.idm.min_gap
    );
    assert!(
        (gap - cfg.idm.min_gap).abs() <= JAM_TOLERANCE,
        "rest travel gap {gap:.3} m, jam gap {} ± {JAM_TOLERANCE}",
        cfg.idm.min_gap
    );
}

/// Rest travel gap 2.000 m with the bisected gap, 1.790 m with the gap at the hit sample (`gap = d_k`):
/// 0.1 m leaves 0.10 m and 0.11 m on either side.
const JAM_TOLERANCE: f32 = 0.1;

/// The car at rest at `S0` of the right turn with a standing dummy 0.15 m off its outer (left) flank,
/// 1.5 m behind its centre: behind the nose, but inside the body swept over the first 0.6 m of the
/// turn (the rear swings out). The car turns away (it may be switched `Dynamic` by the contact).
#[test]
fn a_body_at_the_outer_rear_flank_does_not_hold_a_turn() {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let g = graph(&app);
    let half = app
        .world()
        .resource::<gta_sim::vehicle::VehicleConfig>()
        .half_extents();
    let radius = app
        .world()
        .resource::<gta_sim::character::LocomotionConfig>()
        .capsule_radius;
    let c = turn(&g, 0, 7);
    let (point, tangent) = g.pose(Segment::Connector(c), S0);
    let left = -right(tangent);
    let feet = point + left * (half.x + 0.15 + radius) - tangent * 1.5;
    let nose = point + tangent * half.z;
    let ahead = (feet - nose).with_y(0.0).dot(tangent) + radius;
    let swept = (0..=12).any(|k| {
        let b = body_at(&g, c, S0 + k as f32 * 0.05, half);
        let q = Vec2::new(feet.x, feet.z);
        let local = q - b.centre;
        let (x, y) = (local.dot(b.axis).abs(), local.dot(b.axis.perp()).abs());
        let out = Vec2::new((x - b.half.x).max(0.0), (y - b.half.y).max(0.0));
        out.length() < radius
    });
    assert!(
        ahead <= 0.0 && swept,
        "GATE BROKEN: the dummy is ahead of the nose ({ahead:.2}) or outside the first 0.6 m of the sweep ({swept})"
    );
    let dummy = spawn_dummy(&mut app, feet + Vec3::Y * 0.05);
    let car = granted_on(&mut app, c, S0);
    let start = position_of(&app, car);
    let mut oracle = Footprints::new(&app);
    let mut dynamic = false;
    let mut travelled = 0.0;
    for tick in 0..3 * 64 {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        dynamic |= traffic_car(&app, car).mode == TrafficMode::Dynamic;
        travelled = (position_of(&app, car) - start).with_y(0.0).length();
        if travelled >= 1.5 {
            break;
        }
    }
    eprintln!(
        "outer rear flank: dummy {:.2} m behind the nose line; travelled {travelled:.2} m, switched Dynamic {dynamic}; dummy moved {:.2} m",
        -ahead,
        (position_of(&app, dummy) - feet).with_y(0.0).length()
    );
    oracle.assert_clean("outer rear flank");
    assert!(
        travelled >= 1.5,
        "the dummy at the outer rear flank held the turn: {travelled:.2} m in 3 s"
    );
}

/// A kinematic car held on the east straight without a grant (recovered there: on its path line, an
/// AI car the plain sensing skips), its body on the sweep of the granted north left turn: the turning
/// car senses it and stops short (no grant keeps it off that body). Kinematic bodies pass through
/// each other, so a missed one is a G1 interpenetration.
#[test]
fn a_crossing_car_stops_for_a_car_held_in_the_box() {
    let (lanes, connectors) = plus();
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let g = graph(&app);
    let half = app
        .world()
        .resource::<gta_sim::vehicle::VehicleConfig>()
        .half_extents();
    let (y, x) = (turn(&g, 0, 5), turn(&g, 1, 7));
    let own = body_at(&g, y, S0, half);
    let sweep: Vec<FlatRect> = (0..=120)
        .map(|k| body_at(&g, y, S0 + k as f32 * 0.05, half))
        .collect();
    let s_x = (0..=60)
        .map(|k| k as f32 * 0.1)
        .find(|&s| {
            let b = body_at(&g, x, s, half);
            clearance(&b, &own) > 0.5 && sweep.iter().any(|r| obb_overlap(&b, r))
        })
        .expect("GATE BROKEN: no spot on the east straight across the left turn's sweep");
    let crossing = granted_on(&mut app, y, S0);
    let held = spawn_traffic_car(&mut app, Segment::Connector(x), s_x, 0.0);
    app.world_mut().get_mut::<TrafficCar>(held).unwrap().next = Some(x);
    let mut oracle = Footprints::new(&app);
    let (mut moved, mut scene) = (false, 0);
    // The scene lasts while the held car stands ungranted on its path line (the lease hands it the
    // box after a while).
    for tick in 0..4 * 64 {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let h = traffic_car(&app, held);
        let granted = app
            .world()
            .resource::<TrafficIntersections>()
            .granted(0, x, held);
        if granted || h.mode != TrafficMode::Kinematic || h.lateral != 0.0 || h.speed > 0.0 {
            break;
        }
        scene = tick + 1;
        moved |= traffic_car(&app, crossing).speed > 0.0;
    }
    let gap = travel_gap(
        &g,
        y,
        traffic_car(&app, crossing).s,
        half,
        &footprint(&app, held),
    );
    eprintln!(
        "held in the box at s {s_x:.1} for {scene} ticks: the crossing car rests {gap:.2} m of travel short"
    );
    assert!(
        moved && scene >= 2 * 64,
        "GATE BROKEN: the crossing car never moved, or the held car left within {scene} ticks"
    );
    oracle.assert_clean("held in the box");
}
