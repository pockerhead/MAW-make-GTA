//! G4 (TASK-032): a car left inside a busy junction box does not lock it. The seed-N city with the
//! production population, the player standing on the sidewalk `WATCH_M` from the junction nearest to
//! the spawn, looking at it; at 20 s a car with no driver is placed on the middle of the connector of
//! that box most used so far. The body stays in frame within `bubble.stuck_in_view_distance` for the
//! whole run, so the stuck cheat never hides a late lock (`GATE BROKEN` otherwise).
//!
//! - Lease (correctness): no grant holder whose connector runs through the body and that has not
//!   moved keeps its grant against a waiter for a conflicting connector longer than the shipped
//!   `reservation_timeout` plus 1 s, whether it stands before its stop line, past it or on its
//!   connector (a holder waiting for walkers keeps its lease: TASK-033).
//! - G1 oracle clean.
//! - Liveness, asserted only in the `_liveness` rows, ignored: the in-view box lock is open
//!   (TASK-037). The player watching the box from nearby keeps the car there (the stuck cheat's
//!   in-view rule) and nothing moves it, so the cars at the box entries stand in `Dynamic`.
//!   - No AI car on the box's approaches (within 50 m of it) stands longer than `MAX_STOP` (the
//!     TASK-033 saturation bound); stands elsewhere are printed.
//!   - No AI car stands longer than 30 s in `Dynamic` (spec G3).
//!   - The extra row: an AI car already on the blocked connector right behind the body (it takes
//!     another exit from the start of its connector) stands at most `MAX_STOP`.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    traffic::{
        Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections, TrafficMode,
        in_frame,
    },
    vehicle::{Vehicle, VehicleConfig},
    world::PlayerSpawn,
};
use std::collections::HashMap;
use traffic_support::*;

const HZ: u32 = 64;
const SECONDS: u32 = 120;
const PLACE_AT: u32 = 20;
const MAX_STOP: f32 = 40.0;
/// Where the player watches the box from, m: inside `bubble.stuck_in_view_distance` (40 m).
const WATCH_M: f32 = 25.0;

struct Run {
    worst_stale: f32,
    stand: (f32, Vec3),
    longest: Vec<(Entity, f32, Vec3)>,
    extra: Option<f32>,
    oracle: Footprints,
    grants_at: u32,
    dynamic: Option<String>,
}

/// Grants of each connector of `node` over the first `PLACE_AT` s.
fn connector_use(app: &mut App, node: u32) -> HashMap<u32, u32> {
    let mut per_connector: HashMap<u32, u32> = HashMap::new();
    let mut seen: Vec<(u32, Entity)> = Vec::new();
    for _ in 0..PLACE_AT * HZ {
        run_ticks(app, 1);
        let junctions = app.world().resource::<TrafficIntersections>();
        let Some(j) = junctions.0.get(&node) else {
            seen.clear();
            continue;
        };
        for &o in &j.occupants {
            if !seen.contains(&o) {
                *per_connector.entry(o.0).or_default() += 1;
            }
        }
        seen = j.occupants.clone();
    }
    assert!(
        !per_connector.is_empty(),
        "GATE BROKEN: no grants at the box in {PLACE_AT} s"
    );
    per_connector
}

fn run(seed: u64, extra: bool) -> Run {
    let mut app = city_app(seed);
    let spawn = app.world().resource::<PlayerSpawn>().0;
    let (node, hub) = nearest_box(&app, spawn);
    let feet = sidewalk_at(&app, hub, WATCH_M);
    stand_player(&mut app, Vec2::new(feet.x, feet.z), hub - feet);
    let graph: TrafficGraph = graph(&app);
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let near = app
        .world()
        .resource::<TrafficConfig>()
        .bubble
        .stuck_in_view_distance;
    let used = connector_use(&mut app, node);
    let mut connectors: Vec<u32> = (0..graph.connectors().len() as u32)
        .filter(|&c| graph.connector(c).node == node && graph.connector(c).length >= 9.0)
        .collect();
    connectors.sort_by_key(|c| (u32::MAX - used.get(c).copied().unwrap_or(0), *c));
    let mut placed = None;
    'find: for _ in 0..5 * HZ {
        for &c in &connectors {
            let s = if extra {
                2.0 * half.z + 3.5
            } else {
                graph.length(Segment::Connector(c)) / 2.0
            };
            let (spot, tangent) = graph.pose(Segment::Connector(c), s);
            let near = app
                .world_mut()
                .query_filtered::<&Position, Or<(With<Vehicle>, With<gta_sim::character::Character>)>>()
                .iter(app.world())
                .any(|p| (p.0 - spot).with_y(0.0).length() < 2.0 * half.z + 2.0);
            if !near {
                placed = Some((c, spot, tangent));
                break 'find;
            }
        }
        run_ticks(&mut app, 1);
    }
    let (c, spot, tangent) =
        placed.expect("GATE BROKEN: no free spot on a connector of the busiest box in 5 s");
    let body = park_car(&mut app, spot, tangent);
    let follower = extra.then(|| {
        clear_spot(&mut app, graph.pose(Segment::Connector(c), 0.5).0, 6.0);
        let car = spawn_traffic_car(&mut app, Segment::Connector(c), 0.5, 0.0);
        set_car(&mut app, car, |t| t.next = Some(c));
        app.world_mut()
            .resource_mut::<TrafficIntersections>()
            .0
            .entry(graph.connector(c).node)
            .or_default()
            .occupants
            .push((c, car));
        car
    });
    // Connectors whose path runs through the body (footprint against the connector's car band).
    let body_rect = footprint(&app, body);
    let through: Vec<u32> = (0..graph.connectors().len() as u32)
        .filter(|&k| {
            graph.connector(k).points.windows(2).any(|w| {
                let d = (w[1] - w[0]).with_y(0.0);
                let length = d.length().max(1e-4);
                let band = gta_sim::traffic::FlatRect {
                    centre: Vec2::new((w[0].x + w[1].x) / 2.0, (w[0].z + w[1].z) / 2.0),
                    axis: Vec2::new(-d.z, d.x) / length,
                    half: Vec2::new(half.x, length / 2.0),
                };
                obb_overlap(&band, &body_rect)
            })
        })
        .collect();
    assert!(
        through.contains(&c),
        "GATE BROKEN: the body is not on its own connector's path"
    );
    let mut clock = StandClock::default();
    let mut oracle = Footprints::new(&app);
    let mut unseen = 0;
    let mut stale: HashMap<(u32, Entity), u32> = HashMap::new();
    let mut worst_stale = 0;
    let mut grants_at = 0;
    for tick in 0..(SECONDS - PLACE_AT) * HZ {
        run_ticks(&mut app, 1);
        clock.record(&mut app);
        oracle.record(&mut app, tick);
        let at = app.world().get::<Position>(body).map(|p| p.0);
        let watched = at.is_some_and(|p| {
            (p - feet).with_y(0.0).length() < near
                && app
                    .world()
                    .resource::<gta_sim::population::CameraView>()
                    .0
                    .as_ref()
                    .is_some_and(|v| in_frame(v, p, Quat::IDENTITY, half, feet, f32::INFINITY))
        });
        unseen += u32::from(!watched);
        let cars: HashMap<Entity, (TrafficCar, bool)> = app
            .world_mut()
            .query::<(Entity, &TrafficCar, &LinearVelocity)>()
            .iter(app.world())
            .filter(|(_, c, _)| matches!(c.mode, TrafficMode::Kinematic | TrafficMode::Dynamic))
            .map(|(e, c, v)| (e, (*c, v.0.with_y(0.0).length() < 0.5)))
            .collect();
        let mut live = HashMap::new();
        for (n, j) in &app.world().resource::<TrafficIntersections>().0 {
            for &(k, e) in &j.occupants {
                if *n == node {
                    grants_at += 1;
                }
                let Some(&(car, stands)) = cars.get(&e) else {
                    continue;
                };
                let conn = graph.connector(k);
                let holding = (car.segment == Segment::Connector(k)
                    || car.segment == Segment::Lane(conn.from_lane))
                    && through.contains(&k);
                let contested = j.waiters.iter().any(|w| conn.conflicts.contains(&w.2));
                let held = if stands && holding && contested {
                    stale.get(&(k, e)).copied().unwrap_or(0) + 1
                } else {
                    0
                };
                worst_stale = worst_stale.max(held);
                live.insert((k, e), held);
            }
        }
        stale = live;
    }
    assert!(
        unseen == 0,
        "GATE BROKEN: seed {seed}: the body was out of frame, past {near} m or gone for {unseen} ticks"
    );
    // The box's approaches: stands within 50 m of the node's connector centroid.
    let points: Vec<Vec3> = graph
        .connectors()
        .iter()
        .filter(|k| k.node == node)
        .flat_map(|k| k.points.clone())
        .collect();
    let hub = points.iter().copied().sum::<Vec3>() / points.len().max(1) as f32;
    let (near, far): (Vec<_>, Vec<_>) = clock
        .longer_than(MAX_STOP)
        .into_iter()
        .partition(|(_, _, p)| (*p - hub).with_y(0.0).length() <= 50.0);
    eprintln!("stands > {MAX_STOP} s away from the box (reported): {far:?}");
    Run {
        worst_stale: worst_stale as f32 / HZ as f32,
        stand: clock.worst(),
        longest: near,
        extra: follower.map(|f| clock.of(f)),
        oracle,
        grants_at,
        dynamic: clock.dynamic_violation(),
    }
}

/// `liveness`: the approach, `Dynamic` and extra-car stands are asserted too (else printed only;
/// see the module doc).
fn check(seed: u64, extra: bool, liveness: bool) {
    let timeout = {
        let app = headless_app();
        app.world().resource::<TrafficConfig>().reservation_timeout
    };
    let run = run(seed, extra);
    eprintln!(
        "seed {seed}{}: worst stale holder {:.2} s, worst stand {:.1} s at ({:.1}, {:.1}), extra car \
         stood {:?} s, {} grant ticks at the box, G1 max depth {:.3}",
        if extra { " (extra)" } else { "" },
        run.worst_stale,
        run.stand.0,
        run.stand.1.x,
        run.stand.1.z,
        run.extra,
        run.grants_at,
        run.oracle.max_depth()
    );
    assert!(
        run.grants_at > 0,
        "GATE BROKEN: seed {seed}: no grants at the box after the car was placed"
    );
    let mut failures = Vec::new();
    if run.worst_stale > timeout + 1.0 {
        failures.push(format!(
            "a standing holder kept a contested grant {:.2} s (lease {timeout} s)",
            run.worst_stale
        ));
    }
    if liveness && !run.longest.is_empty() {
        failures.push(format!("AI cars stood > {MAX_STOP} s: {:?}", run.longest));
    }
    if liveness && run.extra.is_some_and(|s| s > MAX_STOP) {
        failures.push(format!(
            "the car behind the body on its connector stood {:?} s",
            run.extra
        ));
    }
    if !run.oracle.violations.is_empty() {
        failures.push(format!("G1: {:?}", run.oracle.summary()));
    }
    match run.dynamic {
        Some(dynamic) if liveness => failures.push(dynamic),
        Some(dynamic) => eprintln!("seed {seed} (reported, TASK-037): {dynamic}"),
        None => {}
    }
    assert!(failures.is_empty(), "seed {seed}: {}", failures.join("; "));
}

#[test]
fn seed_1_box_keeps_moving() {
    check(1, false, false);
}

#[test]
fn seed_7_box_keeps_moving() {
    check(7, false, false);
}

#[test]
fn seed_1_car_behind_the_body_leaves() {
    check(1, true, false);
}

#[test]
#[ignore = "TASK-037: the in-view junction box lock (a car left in a box the player watches from within stuck_in_view_distance)"]
fn seed_1_box_keeps_moving_liveness() {
    check(1, false, true);
}

#[test]
#[ignore = "TASK-037: the in-view junction box lock (a car left in a box the player watches from within stuck_in_view_distance)"]
fn seed_7_box_keeps_moving_liveness() {
    check(7, false, true);
}

#[test]
#[ignore = "TASK-037: the in-view junction box lock (a car left in a box the player watches from within stuck_in_view_distance)"]
fn seed_1_car_behind_the_body_leaves_liveness() {
    check(1, true, true);
}
