//! Traffic with nobody playing (TASK-031 F1): the seed-N city with the production population, the
//! player standing at the spawn looking north (the playtest baseline pose) for 120 s.
//!
//! - Liveness (no gridlock): no AI car stands (< 0.5 m/s) longer than `MAX_STOP`. The unfixed code
//!   gridlocked on all four seeds (15-20 cars standing to the end, stops of 87-117 s). Stops over
//!   20 s (queues at a saturated junction) are printed, not asserted.
//! - Lease (correctness): no junction grant stays with a car standing before its stop line longer
//!   than the SHIPPED `reservation_timeout` (plus one tick) while a car waits at that node for a
//!   conflicting connector. Uncontested, or with the nose past the stop line, the holder keeps it.
//! - Stop lines (correctness, geometry): on the seed-1 graph every stop line sits more than a walker's
//!   reach (navigation `keep_right` + capsule radius) before the crossing at its lane end.

mod common;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    navigation::{NavigationConfig, SidewalkGraph},
    traffic::{
        Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections, TrafficMode,
        TrafficStats,
    },
    vehicle::VehicleConfig,
    world::PlayerSpawn,
};
use std::collections::HashMap;

const SECONDS: u32 = 120;
const STOP_SPEED: f32 = 0.5;
const MAX_STOP: f32 = 40.0;
const REPORT_STOP: f32 = 20.0;

struct Run {
    /// Longest stand of any AI car, s, and where.
    worst_stop: (f32, Vec3),
    /// Stands over `REPORT_STOP`, s.
    long_stops: Vec<f32>,
    /// Longest grant held by a car standing before its stop line while contested, ticks.
    worst_stale: u32,
    grants: u32,
    mean_cars: f32,
    casts: u32,
}

fn run(seed: u64) -> (Run, f32, u32) {
    let mut app = city_app(seed);
    let feet = app.world().resource::<PlayerSpawn>().0;
    set_view(&mut app, Some(chase_view(feet, Vec3::NEG_Z)));
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let hz = (1.0 / step).round() as u32;
    let graph = app.world().resource::<TrafficGraph>().clone();
    let half = app.world().resource::<VehicleConfig>().half_extents().z;
    let timeout = app.world().resource::<TrafficConfig>().reservation_timeout;
    let spawn = position(&mut app);
    let casts = app.world().resource::<TrafficStats>().casts;
    let mut standing_since: HashMap<Entity, u32> = HashMap::new();
    let mut worst_of: HashMap<Entity, u32> = HashMap::new();
    let mut stale: HashMap<(u32, Entity), u32> = HashMap::new();
    let mut result = Run {
        worst_stop: (0.0, Vec3::ZERO),
        long_stops: Vec::new(),
        worst_stale: 0,
        grants: 0,
        mean_cars: 0.0,
        casts: 0,
    };
    let mut car_ticks = 0;
    for tick in 0..SECONDS * hz {
        run_ticks(&mut app, 1);
        let cars: HashMap<Entity, (TrafficCar, bool, Vec3)> = app
            .world_mut()
            .query::<(Entity, &TrafficCar, &LinearVelocity, &Position)>()
            .iter(app.world())
            .filter(|(_, c, ..)| matches!(c.mode, TrafficMode::Kinematic | TrafficMode::Dynamic))
            .map(|(e, c, v, p)| (e, (*c, v.0.with_y(0.0).length() < STOP_SPEED, p.0)))
            .collect();
        car_ticks += cars.len();
        standing_since.retain(|e, _| cars.get(e).is_some_and(|c| c.1));
        for (&e, &(_, stands, p)) in &cars {
            if !stands {
                continue;
            }
            let stood = tick - *standing_since.entry(e).or_insert(tick);
            let worst = worst_of.entry(e).or_default();
            *worst = (*worst).max(stood);
            if stood as f32 * step > result.worst_stop.0 {
                result.worst_stop = (stood as f32 * step, p);
            }
        }
        let mut live = HashMap::new();
        for j in app.world().resource::<TrafficIntersections>().0.values() {
            for &(c, e) in &j.occupants {
                if !stale.contains_key(&(c, e)) {
                    result.grants += 1;
                }
                let Some(&(car, stands, _)) = cars.get(&e) else {
                    live.insert((c, e), 0);
                    continue;
                };
                let conn = graph.connector(c);
                let from = conn.from_lane;
                let before =
                    car.segment == Segment::Lane(from) && car.s + half <= graph.lane(from).stop;
                let contested = j.waiters.iter().any(|w| conn.conflicts.contains(&w.2));
                let held = if stands && before && contested {
                    stale.get(&(c, e)).copied().unwrap_or(0) + 1
                } else {
                    0
                };
                result.worst_stale = result.worst_stale.max(held);
                live.insert((c, e), held);
            }
        }
        stale = live;
    }
    // Walkers jostle the standing player; the view stays on the spawn.
    let moved = (position(&mut app) - spawn).with_y(0.0).length();
    assert!(
        moved < 5.0,
        "GATE BROKEN: the player drifted {moved:.2} m from the spawn"
    );
    result.long_stops = worst_of
        .values()
        .map(|&t| t as f32 * step)
        .filter(|&s| s > REPORT_STOP)
        .collect();
    result.mean_cars = car_ticks as f32 / (SECONDS * hz) as f32;
    result.casts = app.world().resource::<TrafficStats>().casts - casts;
    (result, timeout, hz)
}

fn nobody_playing(seed: u64) {
    let (run, timeout, hz) = run(seed);
    let (stop, at) = run.worst_stop;
    eprintln!(
        "seed {seed}: {:.1} AI cars on average, {} grants, worst stand {stop:.1} s at ({:.1}, {:.1}), \
         stands > {REPORT_STOP} s: {:?}, worst stale grant {:.2} s",
        run.mean_cars,
        run.grants,
        at.x,
        at.z,
        run.long_stops,
        run.worst_stale as f32 / hz as f32
    );
    assert!(
        run.mean_cars >= 10.0 && run.grants >= 40 && run.casts > 0,
        "GATE BROKEN: seed {seed}: thin traffic ({:.1} cars, {} grants, {} casts)",
        run.mean_cars,
        run.grants,
        run.casts
    );
    eprintln!(
        "seed {seed}: stand margin {:.1} s under {MAX_STOP} s",
        MAX_STOP - stop
    );
    let mut failures = Vec::new();
    if stop > MAX_STOP {
        failures.push(format!(
            "a car stood {stop:.1} s at ({:.1}, {:.1}) (> {MAX_STOP} s: gridlock)",
            at.x, at.z
        ));
    }
    let lease = (timeout * hz as f32).ceil() as u32 + 1;
    if run.worst_stale > lease {
        failures.push(format!(
            "a car standing before its stop line held a contested grant {:.2} s (lease {timeout} s)",
            run.worst_stale as f32 / hz as f32
        ));
    }
    assert!(failures.is_empty(), "seed {seed}: {}", failures.join("; "));
}

#[test]
fn seed_1_keeps_flowing() {
    nobody_playing(1);
}

#[test]
fn seed_2_keeps_flowing() {
    nobody_playing(2);
}

#[test]
fn seed_7_keeps_flowing() {
    nobody_playing(7);
}

#[test]
fn seed_42_keeps_flowing() {
    nobody_playing(42);
}

/// Parameter along `a -> b` where it crosses `p -> q`, both segments within their ends.
fn crossing(a: Vec2, b: Vec2, p: Vec2, q: Vec2) -> Option<f32> {
    let (r, d) = (b - a, q - p);
    let den = r.perp_dot(d);
    if den.abs() < 1e-6 {
        return None;
    }
    let t = (p - a).perp_dot(d) / den;
    let u = (p - a).perp_dot(r) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

#[test]
fn stop_lines_leave_the_crosswalk_free() {
    let app = city_app(1);
    let world = app.world();
    let graph = world.resource::<TrafficGraph>();
    let walks = world.resource::<SidewalkGraph>();
    let reach = world.resource::<NavigationConfig>().keep_right
        + world.resource::<LocomotionConfig>().capsule_radius;
    let flat = |v: Vec3| Vec2::new(v.x, v.z);
    let mut checked = 0;
    for (k, lane) in graph.lanes().iter().enumerate() {
        for &(i, j) in walks.edges() {
            let Some(t) = crossing(
                flat(lane.from),
                flat(lane.to),
                flat(walks.node(i)),
                flat(walks.node(j)),
            ) else {
                continue;
            };
            if t <= 0.5 {
                continue;
            }
            checked += 1;
            let walker = t * lane.length - reach;
            assert!(
                lane.stop < walker,
                "lane {k}: stop line at {:.2} m, a walker on the crossing reaches back to {walker:.2} m",
                lane.stop
            );
        }
    }
    assert!(
        checked >= 100,
        "GATE BROKEN: only {checked} lane ends with a crossing"
    );
}
