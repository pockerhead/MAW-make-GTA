//! Walkers reach a sidewalk waypoint that lies under a standing car (TASK-036 item A).
//!
//! The cause (TASK-036 stage 0, `scratch/stage0/walkers.txt`): a car left over a sidewalk node covers
//! the node's lane target; `around_cars` never returns a target inside a car, so the walker never came
//! within `arrive_radius` of it and orbited the car's corners, and every walker bound for that node
//! pressed against the others there (13 walkers stalled up to 94 s, 11 up to 118 s; 0 of 6361 stalled
//! samples had a counter-flowing neighbour). The rule: a waypoint under a standing car counts as
//! reached beside that car (`civilian::arrive`).
//!
//! - A1 (floor, correctness of the arrival rule): a three-arm hub whose centre node lies under a
//!   parked car; six walkers bound for the hub each take their next edge there within
//!   `(d_start + ring / 2) / walk_speed + ARRIVAL_SLACK` s (the far start 9.6 m, half the corner ring
//!   round the car 9.68 m, at 1.8 m/s: 10.7 s), and none stands (< 0.7 m in 10 s) before.
//! - A2 (seed-1 city, the TASK-037 QA scene R1 spot B; liveness as the player sees it): a car parked
//!   over a sidewalk node, headings 0 and 90; no Wander/Flee civilian within 8 m of it stands (moves
//!   < 0.7 m over 10 s) longer than `MAX_STALL`. Derivation: without the car 0 walkers stall >= 10 s,
//!   with it (unfixed) 70-118 s; the bound is two metric windows.
//!
//! - A3 (floor, correctness of the corner shift): a walker heading round a parked car to its far
//!   corner and one leaving it the other way on the line through that corner pass each other. The
//!   case is the Linux A2 heading-90 stall the arrival rule exposed (two walkers 89 s face to face at
//!   a corner, intents exactly opposite): `around_cars` corners carried no `keep_right`, lane targets
//!   do. The civilian caller moves a corner target `keep_right` across the travel, away from the car
//!   (never towards it: 0.8 - 0.5 m would put the capsule against the body).
//!
//! Flips: the covered branch of `civilian::reached` returns `false` (A1 and A2 RED); the corner shift
//! is 0 (`off_corner` with `keep_right` 0: A3 RED, the walkers stand face to face).

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{LocomotionConfig, MoveIntent},
    civilian::{Civilian, CivilianState},
    navigation::{GraphWalker, NavigationConfig, SidewalkGraph, lane_target},
    occupancy::{Footprint, RoadOccupancy},
    traffic::FlatRect,
    vehicle::VehicleConfig,
};
use std::collections::HashMap;
use traffic_support::*;

const HZ: u32 = 64;
/// Timing slack of A1 over the geometric bound, s: the start from rest and the turns at the corners.
/// Measured arrivals 1.5-4.9 s (3 runs, no spread), all under the geometric 10.7 s alone.
const ARRIVAL_SLACK: f32 = 1.0;
/// A walker that moves less than this over `STALL_WINDOW` stands, m.
const STALL_MOVE: f32 = 0.7;
const STALL_WINDOW: f32 = 10.0;
/// A2 bound: two stall windows.
const MAX_STALL: f32 = 2.0 * STALL_WINDOW;

/// `p` lies inside `rect` grown by `grow` on both axes.
fn inside(rect: &FlatRect, p: Vec3, grow: f32) -> bool {
    let d = Vec2::new(p.x, p.z) - rect.centre;
    d.dot(rect.axis).abs() <= rect.half.x + grow
        && d.dot(rect.axis.perp()).abs() <= rect.half.y + grow
}

fn walker_of(app: &App, e: Entity) -> GraphWalker {
    *app.world()
        .get::<GraphWalker>(e)
        .expect("GATE BROKEN: walker missing")
}

/// A1: the hub centre C = (24, 0, 0), arms to (24, 0, 12), (24, 0, -12), (12, 0, 0); a car parked
/// with its centre 0.5 m from C (as spot B).
#[test]
fn a1_walkers_reach_a_node_under_a_car() {
    let (lanes, connectors) = loop_lanes(6.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let hub = Vec3::new(24.0, 0.0, 0.0);
    let arms = [
        Vec3::new(24.0, 0.0, 12.0),
        Vec3::new(24.0, 0.0, -12.0),
        Vec3::new(12.0, 0.0, 0.0),
    ];
    // Clear of every test-area block (the nearest, the 1.5 m block at (16, 10), spans x <= 16.75) and
    // of the loop lanes (|x|, |z| >= 34).
    for p in arms.iter().chain([&hub]) {
        assert!(
            p.x > 17.0 && p.x < 33.0 || p.z.abs() < 8.0 && p.x > 11.0 && p.x < 33.0,
            "GATE BROKEN: hub point {p} near the test area or the loop"
        );
    }
    let mut nodes = vec![hub];
    nodes.extend(arms);
    test_graph(&mut app, nodes, &[(0, 1), (0, 2), (0, 3)]);
    let car = park_car(&mut app, hub + Vec3::X * 0.5, Vec3::X);
    run_ticks(&mut app, HZ);
    let parked = position_of(&app, car);
    let loco = app.world().resource::<LocomotionConfig>().clone();
    let nav = app.world().resource::<NavigationConfig>().clone();
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let body = app
        .world()
        .resource::<RoadOccupancy>()
        .body(car)
        .copied()
        .expect("GATE BROKEN: the parked car is not in the road occupancy");
    let Footprint::Rect(rect) = body.shape else {
        panic!("GATE BROKEN: the car's footprint is not a rect");
    };
    assert!(
        body.standing > 0.0,
        "GATE BROKEN: the parked car does not stand"
    );
    let graph = app.world().resource::<SidewalkGraph>();
    for k in 1..=3u32 {
        let target = lane_target(graph, GraphWalker { from: k, to: 0 }, nav.keep_right);
        assert!(
            inside(&rect, target, loco.capsule_radius),
            "GATE BROKEN: the lane target {target} of arm {k} is not under the car"
        );
    }
    let mut walkers = Vec::new();
    for k in 1..=3u32 {
        for t in [0.2, 0.6] {
            walkers.push(spawn_civilian(
                &mut app,
                GraphWalker { from: k, to: 0 },
                t,
                calm(),
            ));
        }
    }
    // The far start, and half the ring of corners `corner` m out round the car.
    let corner = loco.capsule_radius + nav.arrive_radius;
    let d_start = 12.0 * (1.0 - 0.2);
    let ring = 2.0 * (2.0 * (half.x + corner) + 2.0 * (half.z + corner));
    let bound = (d_start + ring / 2.0) / loco.walk_speed + ARRIVAL_SLACK;
    let mut arrived: HashMap<Entity, f32> = HashMap::new();
    let mut anchors: HashMap<Entity, (Vec3, u32)> = HashMap::new();
    let mut stood: Vec<(Entity, f32, Vec3)> = Vec::new();
    for tick in 0..(bound * HZ as f32).ceil() as u32 {
        run_ticks(&mut app, 1);
        assert!(
            (position_of(&app, car) - parked).with_y(0.0).length() < 0.05,
            "GATE BROKEN: the parked car moved"
        );
        for &w in &walkers {
            if arrived.contains_key(&w) {
                continue;
            }
            if walker_of(&app, w).from == 0 {
                arrived.insert(w, tick as f32 / HZ as f32);
                continue;
            }
            let p = position_of(&app, w);
            let anchor = anchors.entry(w).or_insert((p, tick));
            if (p - anchor.0).with_y(0.0).length() >= STALL_MOVE {
                *anchor = (p, tick);
            } else if (tick - anchor.1) as f32 / HZ as f32 >= STALL_WINDOW {
                stood.push((w, tick as f32 / HZ as f32, p));
                *anchor = (p, tick);
            }
        }
    }
    let mut times: Vec<f32> = arrived.values().copied().collect();
    times.sort_by(f32::total_cmp);
    eprintln!(
        "A1: arrivals at the covered node {times:?} s of {} walkers, bound {bound:.2} s \
         (geometric {:.2} s + slack {ARRIVAL_SLACK} s); stands before arrival {stood:?}",
        walkers.len(),
        bound - ARRIVAL_SLACK
    );
    let mut failures = Vec::new();
    let missing: Vec<(Entity, Vec3)> = walkers
        .iter()
        .filter(|w| !arrived.contains_key(w))
        .map(|&w| (w, position_of(&app, w)))
        .collect();
    if !missing.is_empty() {
        failures.push(format!(
            "walkers never reached the node under the car in {bound:.2} s: {missing:?}"
        ));
    }
    if !stood.is_empty() {
        failures.push(format!(
            "walkers stood (< {STALL_MOVE} m in {STALL_WINDOW} s) before arriving: {stood:?}"
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

/// A3 (floor, correctness of the corner shift): walker A goes round a parked car to its far corner
/// while walker B, leaving it the other way, heads for a lane target on the line through that corner,
/// the two walking at each other on one line (the Linux A2 heading-90 stall: 89 s, intents exactly
/// opposite, capsules touching). The car stands at (24.5, 0, 0) facing +X; A bound east (edge
/// (18, 0) -> (32, 0)) starts at (21.9, -2.0), past the car's south-west corner; B bound west (edge
/// (32, -1.5) -> (18, -1.5), lane target (18, -2.0)) starts at (26.3, -2.0). Both must pass each other
/// (each beyond the other's start) within twice the time to walk the gap between their starts (4.9 s;
/// measured 3.3 s; with the corner unshifted they stand face to face).
#[test]
fn a3_head_on_walkers_pass_at_a_car_corner() {
    let (lanes, connectors) = loop_lanes(6.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let nodes = vec![
        Vec3::new(18.0, 0.0, 0.0),
        Vec3::new(32.0, 0.0, 0.0),
        Vec3::new(32.0, 0.0, -1.5),
        Vec3::new(18.0, 0.0, -1.5),
    ];
    test_graph(&mut app, nodes, &[(0, 1), (2, 3)]);
    let car = park_car(&mut app, Vec3::new(24.5, 0.0, 0.0), Vec3::X);
    run_ticks(&mut app, HZ);
    let parked = position_of(&app, car);
    let loco = app.world().resource::<LocomotionConfig>().clone();
    let nav = app.world().resource::<NavigationConfig>().clone();
    let rect = footprint(&app, car);
    let corner = loco.capsule_radius + nav.arrive_radius;
    let far = rect.half + Vec2::splat(corner);
    // The south-east corner (largest x - z) of the ring `around_cars` steers by.
    let k = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
        .map(|(u, v): (f32, f32)| {
            rect.centre + rect.axis * (u * far.x) + rect.axis.perp() * (v * far.y)
        })
        .into_iter()
        .max_by(|a, b| (a.x - a.y).total_cmp(&(b.x - b.y)))
        .expect("four corners");
    let graph = app.world().resource::<SidewalkGraph>();
    let b_target = lane_target(graph, GraphWalker { from: 2, to: 3 }, nav.keep_right);
    let a_target = lane_target(graph, GraphWalker { from: 0, to: 1 }, nav.keep_right);
    let (a_start, b_start) = (Vec3::new(21.9, 0.0, -2.0), Vec3::new(26.3, 0.0, -2.0));
    assert!(
        (k.y - b_target.z).abs() < 0.01 && (k.y - a_start.z).abs() < 0.01,
        "GATE BROKEN: A3: the corner {k} is not on B's line (target {b_target}, starts at z {})",
        a_start.z
    );
    assert!(
        (0..=100).any(|k| inside(
            &rect,
            a_start.lerp(a_target, k as f32 / 100.0),
            loco.capsule_radius
        )),
        "GATE BROKEN: A3: A's way to its target {a_target} does not cross the car"
    );
    let a = spawn_civilian(&mut app, GraphWalker { from: 0, to: 1 }, 0.2, calm());
    let b = spawn_civilian(&mut app, GraphWalker { from: 2, to: 3 }, 0.3, calm());
    for (w, at) in [(a, a_start), (b, b_start)] {
        let y = position_of(&app, w).y;
        teleport(&mut app, w, at.with_y(y));
    }
    run_ticks(&mut app, 1);
    let heading = |app: &App, w: Entity| {
        let yaw = app.world().get::<MoveIntent>(w).expect("intent").yaw;
        Vec2::new(-yaw.sin(), -yaw.cos())
    };
    assert!(
        heading(&app, a).dot(Vec2::X) > 10f32.to_radians().cos()
            && heading(&app, b).dot(Vec2::NEG_X) > 0.999,
        "GATE BROKEN: A3: A heads {} (east, round the car), B heads {} (west)",
        heading(&app, a),
        heading(&app, b)
    );
    let bound = 2.0 * (b_start.x - a_start.x) / loco.walk_speed;
    let mut passed = None;
    let (mut a_x, mut b_x) = (a_start.x, b_start.x);
    for tick in 0..(bound * HZ as f32).ceil() as u32 {
        run_ticks(&mut app, 1);
        assert!(
            (position_of(&app, car) - parked).with_y(0.0).length() < 0.05,
            "GATE BROKEN: A3: the parked car moved"
        );
        (a_x, b_x) = (position_of(&app, a).x, position_of(&app, b).x);
        if a_x >= b_start.x && b_x <= a_start.x {
            passed = Some(tick as f32 / HZ as f32);
            break;
        }
    }
    eprintln!(
        "A3: passed at {passed:?} s (bound {bound:.2} s); A at {}, B at {}",
        position_of(&app, a),
        position_of(&app, b)
    );
    assert!(
        passed.is_some(),
        "A3: the walkers did not pass each other at the car corner within {bound:.2} s (A x {a_x:.2}, B x {b_x:.2})"
    );
}

/// The QA scene: seed 1, the player at the R1 stand point looking at the junction, a car parked at
/// spot B with `heading_deg`, 150 s. Returns the longest stall per walker near the car.
fn spot_b(heading_deg: f32) -> Vec<(Entity, f32, Vec3)> {
    let mut app = city_app(1);
    let junction = Vec3::new(7.9, 0.0, -81.3);
    let stand = Vec2::new(7.2, -50.0);
    stand_player(&mut app, stand, junction - Vec3::new(stand.x, 0.0, stand.y));
    run_ticks(&mut app, 2 * HZ);
    let spot = Vec3::new(5.3, 0.0, -84.5);
    clear_spot(&mut app, spot, 6.0);
    let yaw = heading_deg.to_radians();
    let car = park_car(&mut app, spot, Vec3::new(-yaw.sin(), 0.0, -yaw.cos()));
    let parked = position_of(&app, car);
    let rect = footprint(&app, car);
    let loco = app.world().resource::<LocomotionConfig>().clone();
    let nav = app.world().resource::<NavigationConfig>().clone();
    let graph = app.world().resource::<SidewalkGraph>();
    let covered: Vec<u32> = (0..graph.nodes().len() as u32)
        .filter(|&n| {
            graph.neighbors(n).iter().any(|&m| {
                let target = lane_target(graph, GraphWalker { from: m, to: n }, nav.keep_right);
                inside(&rect, target, loco.capsule_radius)
            })
        })
        .collect();
    assert!(
        !covered.is_empty(),
        "GATE BROKEN: heading {heading_deg}: no sidewalk node's lane target under the car"
    );
    let mut targeted = false;
    let mut anchors: HashMap<Entity, (Vec3, u32)> = HashMap::new();
    let mut worst: HashMap<Entity, (f32, Vec3)> = HashMap::new();
    for tick in 0..150 * HZ {
        run_ticks(&mut app, 1);
        if tick % 16 != 0 {
            continue;
        }
        let at = app
            .world()
            .get::<Position>(car)
            .map(|p| p.0)
            .expect("GATE BROKEN: the parked car is gone");
        assert!(
            (at - parked).with_y(0.0).length() < 0.05,
            "GATE BROKEN: heading {heading_deg}: the parked car moved"
        );
        let walkers: Vec<(Entity, CivilianState, Vec3, GraphWalker)> = app
            .world_mut()
            .query::<(Entity, &Civilian, &Position, &GraphWalker)>()
            .iter(app.world())
            .map(|(e, c, p, w)| (e, c.state, p.0, *w))
            .collect();
        for (e, state, p, w) in walkers {
            targeted |= covered.contains(&w.to);
            let moving = matches!(state, CivilianState::Wander | CivilianState::Flee { .. });
            if !moving || (p - spot).with_y(0.0).length() > 8.0 {
                anchors.remove(&e);
                continue;
            }
            let anchor = anchors.entry(e).or_insert((p, tick));
            if (p - anchor.0).with_y(0.0).length() > STALL_MOVE {
                *anchor = (p, tick);
                continue;
            }
            let secs = (tick - anchor.1) as f32 / HZ as f32;
            if secs >= STALL_WINDOW && worst.get(&e).is_none_or(|w| w.0 < secs) {
                worst.insert(e, (secs, p));
            }
        }
    }
    assert!(
        targeted,
        "GATE BROKEN: heading {heading_deg}: no walker headed for a node under the car ({covered:?})"
    );
    let mut rows: Vec<(Entity, f32, Vec3)> =
        worst.into_iter().map(|(e, (s, p))| (e, s, p)).collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    eprintln!(
        "A2 heading {heading_deg}: covered nodes {covered:?}; walkers stalled >= {STALL_WINDOW} s \
         within 8 m of the car (longest per walker): {rows:?}"
    );
    rows
}

fn a2(heading_deg: f32) {
    let rows = spot_b(heading_deg);
    let over: Vec<_> = rows.iter().filter(|r| r.1 > MAX_STALL).collect();
    assert!(
        over.is_empty(),
        "heading {heading_deg}: walkers stood beside the car longer than {MAX_STALL} s: {over:?}"
    );
}

#[test]
fn a2_spot_b_heading_0() {
    a2(0.0);
}

#[test]
fn a2_spot_b_heading_90() {
    a2(90.0);
}
