//! G2 (TASK-032): traffic goes around a standing body in its lane. Production city (seeds 1, 2, 7,
//! 42), the longest street / avenue lane with 70 m to its stop line; a car left 40 m before the stop
//! line (or a dummy standing there); the player on the sidewalk 25 m past it, looking back up the
//! lane. A named fixture feeds 8 AI cars at the lane start, one every 4 s while its first 20 m are free
//! (natural traffic counts too).
//!
//! - Liveness + correctness: the first 8 AI cars that queue behind the body pass it (rear past its
//!   front) within `PASS_S` of joining the queue.
//! - No AI car in the bubble stands longer than `STAND_S`; no `Dynamic` one either (the M1 class).
//! - No `CollisionStart` between a passing car and any vehicle; the G1 oracle stays clean.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::{ecs::message::MessageCursor, prelude::*};
use common::*;
use gta_sim::{
    traffic::{Manoeuvre, Segment, TrafficCar, TrafficConfig},
    vehicle::{Vehicle, VehicleConfig},
};
use std::collections::HashMap;
use traffic_support::*;

const HZ: u32 = 64;
const SECONDS: u32 = 150;
/// 8 cars at about 7.5 s each (TASK-032 spec, a floor).
const PASS_S: f32 = 60.0;
const STAND_S: f32 = 30.0;
const QUEUED: usize = 8;

#[derive(Clone, Copy, PartialEq)]
enum Body {
    Car,
    Dummy,
}

struct Watch {
    /// car -> (joined tick, passed tick)
    queue: HashMap<Entity, (u32, Option<u32>)>,
    order: Vec<Entity>,
    clock: StandClock,
    oracle: Footprints,
    contacts: Vec<(u32, Entity, Entity)>,
    passes: u32,
}

fn run(seed: u64, avenue: bool, body: Body) {
    let label = format!(
        "seed {seed} {} {}",
        if avenue { "avenue" } else { "street" },
        if body == Body::Car { "car" } else { "dummy" }
    );
    let mut app = city_app(seed);
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let v0 = if avenue {
        cfg.desired_speed.avenue
    } else {
        cfg.desired_speed.street
    };
    let lane = longest_lane(&app, v0, 70.0);
    let l = graph(&app).lane(lane).clone();
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let obstacle_s = l.stop - 40.0;
    let at = l.from + l.dir * obstacle_s;
    let beside = at + l.dir * 25.0 + right_of(l.dir) * if avenue { 6.5 } else { 3.2 };
    stand_player(&mut app, Vec2::new(beside.x, beside.z), -l.dir);
    run_ticks(&mut app, 2 * HZ);
    clear_spot(&mut app, at, 8.0);
    let (front, placed) = match body {
        Body::Car => (obstacle_s + half.z, park_car(&mut app, at, l.dir)),
        Body::Dummy => {
            let y = ground_at(&mut app, Vec2::new(at.x, at.z));
            let radius = app
                .world()
                .resource::<gta_sim::character::LocomotionConfig>()
                .capsule_radius;
            (obstacle_s + radius, spawn_dummy(&mut app, at.with_y(y)))
        }
    };
    let mut w = Watch {
        queue: HashMap::new(),
        order: Vec::new(),
        clock: StandClock::default(),
        oracle: Footprints::new(&app),
        contacts: Vec::new(),
        passes: 0,
    };
    let mut cursor: MessageCursor<CollisionStart> = app
        .world()
        .resource::<Messages<CollisionStart>>()
        .get_cursor_current();
    let mut feeder = Feeder::new(lane, 4 * HZ, QUEUED);
    let right = right_of(l.dir);
    for tick in 0..SECONDS * HZ {
        feeder.tick(&mut app, tick);
        run_ticks(&mut app, 1);
        w.clock.record(&mut app);
        w.oracle.record(&mut app, tick);
        let cars: Vec<(Entity, TrafficCar, Vec3, f32)> = app
            .world_mut()
            .query::<(Entity, &TrafficCar, &Position, &LinearVelocity)>()
            .iter(app.world())
            .filter(|(_, c, ..)| c.is_ai())
            .map(|(e, c, p, v)| (e, *c, p.0, v.0.length()))
            .collect();
        for &(e, car, p, v) in &cars {
            if matches!(car.manoeuvre, Manoeuvre::Pass { .. }) {
                w.passes += 1;
            }
            let d = (p - l.from).with_y(0.0);
            let (a, c) = (d.dot(l.dir), d.dot(right));
            let on_lane = car.segment == Segment::Lane(lane);
            if on_lane && a < obstacle_s && c.abs() < 1.7 && v < 0.5 && !w.queue.contains_key(&e) {
                w.queue.insert(e, (tick, None));
                w.order.push(e);
            }
            if let Some(entry) = w.queue.get_mut(&e)
                && entry.1.is_none()
                && (a - half.z > front || !on_lane)
            {
                entry.1 = Some(tick);
            }
        }
        let passing: Vec<Entity> = cars
            .iter()
            .filter(|c| matches!(c.1.manoeuvre, Manoeuvre::Pass { go: true, .. }))
            .map(|c| c.0)
            .collect();
        let world = app.world();
        for start in cursor.read(world.resource::<Messages<CollisionStart>>()) {
            let a = start.body1.unwrap_or(start.collider1);
            let b = start.body2.unwrap_or(start.collider2);
            for (p, o) in [(a, b), (b, a)] {
                if passing.contains(&p) && world.get::<Vehicle>(o).is_some() {
                    w.contacts.push((tick, p, o));
                }
            }
        }
    }
    let _ = placed;
    report(&label, &w);
}

fn report(label: &str, w: &Watch) {
    let secs = |t: u32| t as f32 / HZ as f32;
    let first: Vec<(Entity, f32, Option<f32>)> = w
        .order
        .iter()
        .take(QUEUED)
        .map(|e| {
            let (joined, passed) = w.queue[e];
            (*e, secs(joined), passed.map(|p| secs(p - joined)))
        })
        .collect();
    let (worst, at) = w.clock.worst();
    eprintln!(
        "{label}: queued {} (first {QUEUED}: joined s / waited s {:?}), pass ticks {}, worst stand \
         {worst:.1} s at ({:.1}, {:.1}), worst dynamic stand {:.1} s, G1 max depth {:.3}",
        w.order.len(),
        first
            .iter()
            .map(|f| (f.1.round(), f.2.map(|x| (x * 10.0).round() / 10.0)))
            .collect::<Vec<_>>(),
        w.passes,
        at.x,
        at.z,
        w.clock.worst_dynamic(),
        w.oracle.max_depth()
    );
    assert!(
        first.len() == QUEUED,
        "GATE BROKEN: {label}: only {} cars queued",
        first.len()
    );
    let mut failures = Vec::new();
    let late: Vec<_> = first
        .iter()
        .filter(|f| f.2.is_none_or(|t| t > PASS_S))
        .collect();
    if !late.is_empty() {
        failures.push(format!("not past within {PASS_S} s of queuing: {late:?}"));
    }
    let long = w.clock.longer_than(STAND_S);
    if !long.is_empty() {
        failures.push(format!("AI cars stood > {STAND_S} s: {long:?}"));
    }
    failures.extend(w.clock.dynamic_violation());
    if !w.contacts.is_empty() {
        failures.push(format!("passing cars touched vehicles: {:?}", w.contacts));
    }
    if !w.oracle.violations.is_empty() {
        failures.push(format!("G1: {:?}", w.oracle.summary()));
    }
    assert!(failures.is_empty(), "{label}: {}", failures.join("; "));
}

#[test]
fn street_seed_1() {
    run(1, false, Body::Car);
}

#[test]
fn street_seed_2() {
    run(2, false, Body::Car);
}

#[test]
fn street_seed_7() {
    run(7, false, Body::Car);
}

#[test]
fn street_seed_42() {
    run(42, false, Body::Car);
}

#[test]
fn avenue_seed_1() {
    run(1, true, Body::Car);
}

#[test]
fn avenue_seed_2() {
    run(2, true, Body::Car);
}

#[test]
fn avenue_seed_7() {
    run(7, true, Body::Car);
}

#[test]
fn avenue_seed_42() {
    run(42, true, Body::Car);
}

#[test]
fn dummy_street_seed_1() {
    run(1, false, Body::Dummy);
}

#[test]
fn dummy_street_seed_7() {
    run(7, false, Body::Dummy);
}
