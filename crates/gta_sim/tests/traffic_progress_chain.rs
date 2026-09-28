//! Traffic progress (TASK-039), two standing bodies in a row (QA B1, liveness + correctness): a car
//! relaxed against the first body stops inside it before the second; it re-targets to the second while
//! the first stays exempt until the two separate (`traffic::progress`, `Relax::trailing`).
//!
//! Floors: the loop (no neighbour lane: no lane pass, no box pass) and the two-way street with a feeder
//! on the oncoming lane every 3 s (a lane pass rarely fits). The first body is a parked car, the second
//! a parked car or a dummy, 1 or 3 m past it. Each run collects all violations before panicking.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    traffic::{Relax, Segment, TrafficCar, TrafficConfig},
    vehicle::VehicleConfig,
};
use std::collections::HashMap;
use traffic_support::{progress::*, *};

const HZ: u32 = PROGRESS_HZ;

#[derive(Clone, Copy, Debug)]
enum Floor {
    Loop,
    BusyStreet,
}

/// Runs one scene; returns its failures.
fn two_in_a_row(floor: Floor, gap: f32, person_second: bool) -> Vec<String> {
    let label = format!("{floor:?} gap {gap} person_second {person_second}");
    let v0 = headless_app()
        .world()
        .resource::<TrafficConfig>()
        .desired_speed
        .street;
    // Lane 0 at `s` (flat), the watched car's start and the first body's centre, s.
    let (lanes, connectors, on_lane0, car_s, first_s): (_, _, fn(f32) -> Vec3, f32, f32) =
        match floor {
            Floor::Loop => {
                let (l, c) = loop_lanes(v0);
                (l, c, |s| Vec3::new(-34.0 + s, 0.0, 37.0), 10.0, 30.0)
            }
            Floor::BusyStreet => {
                let (l, c) = two_way_street(60.0);
                (l, c, |s| Vec3::new(-30.0 + s, 0.0, 30.0), 2.0, 22.0)
            }
        };
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let radius = app.world().resource::<LocomotionConfig>().capsule_radius;
    let cfg = app.world().resource::<TrafficConfig>().clone();
    spawn_traffic_car(&mut app, Segment::Lane(0), car_s, 0.0);
    let first = park_car(&mut app, on_lane0(first_s), Vec3::X);
    let (second, second_half) = if person_second {
        let at = on_lane0(first_s + half.z + gap + radius);
        (spawn_dummy(&mut app, at), radius)
    } else {
        let at = on_lane0(first_s + 2.0 * half.z + gap);
        (park_car(&mut app, at, Vec3::X), half.z)
    };
    run_ticks(&mut app, 2);
    let mut failures: Vec<String> = Vec::new();
    for (who, want, e) in [
        ("first", on_lane0(first_s), first),
        (
            "second",
            on_lane0(first_s + half.z + gap + second_half),
            second,
        ),
    ] {
        let off = (position_of(&app, e) - want).with_y(0.0).length();
        if off > 0.3 {
            failures.push(format!(
                "GATE BROKEN: {label}: the {who} body is {off:.2} m off its spot"
            ));
        }
    }
    let (first_rear, second_rear) = (
        position_of(&app, first).x - half.z,
        position_of(&app, second).x - second_half,
    );
    let far = position_of(&app, second).x + second_half;
    // Derived, from a car's relaxation against the first body: it drives from `min_gap` behind the
    // first body to `min_gap` before the second (from rest to rest), stands `grace_seconds`, re-targets
    // and drives until its rear is past the second (from rest). A leg of `d` m at most at the pass
    // speed `v` (the squeeze cap) takes at most `d / v + v / a` to start and `v / b` to stop.
    let (v, idm) = (cfg.pass.speed, &cfg.idm);
    let leg = |d: f32| d / v + v / idm.acceleration;
    let bound = leg(second_rear - first_rear)
        + v / idm.comfortable_deceleration
        + cfg.progress.grace_seconds
        + leg(far - second_rear + idm.min_gap + 2.0 * half.z);
    let mut feeder = matches!(floor, Floor::BusyStreet).then(|| Feeder::new(1, 3 * HZ, 30));
    let seconds = match floor {
        Floor::Loop => 90,
        Floor::BusyStreet => 150,
    };
    // Per car relaxed against the first body: (that tick, the tick its rear is past the second), s.
    let mut squeezes: HashMap<Entity, (f32, Option<f32>)> = HashMap::new();
    let at = on_lane0(first_s);
    let lane_z = at.z;
    let seen = watch(&mut app, at, seconds, |app, tick| {
        if let Some(f) = feeder.as_mut() {
            f.tick(app, tick);
        }
        let now = tick as f32 / HZ as f32;
        let cars: Vec<(Entity, Option<Relax>, Vec3)> = app
            .world_mut()
            .query::<(Entity, &TrafficCar, &Position)>()
            .iter(app.world())
            .map(|(e, c, p)| (e, c.relaxed, p.0))
            .collect();
        for (e, relaxed, p) in cars {
            if relaxed.is_some_and(|r| r.blocker == first) {
                squeezes.entry(e).or_insert((now, None));
            }
            let past = (0.0..20.0).contains(&(p.x - half.z - far)) && (p.z - lane_z).abs() < 1.0;
            if let Some(squeeze) = squeezes.get_mut(&e).filter(|s| past && s.1.is_none()) {
                squeeze.1 = Some(now);
            }
        }
    });
    report(&label, &app, &seen);
    eprintln!(
        "{label}: squeezes (relaxed, past) {squeezes:?}, bound {bound:.1} s after the relaxation"
    );
    failures.extend(
        scene_failures(&seen, at)
            .into_iter()
            .map(|f| format!("{label}: {f}")),
    );
    if squeezes.is_empty() {
        failures.push(format!(
            "{label}: no car was relaxed against the first body"
        ));
    }
    let end = seconds as f32;
    for (e, (relaxed, past)) in &squeezes {
        let late = past.map_or(relaxed + bound < end, |t| t - relaxed > bound);
        if late {
            failures.push(format!(
                "{label}: {e} relaxed against the first body at {relaxed:.1} s, rear past the second at \
                 {past:?} s, bound {bound:.1} s after the relaxation"
            ));
        }
    }
    failures
}

/// A car stopped behind two standing bodies 1-3 m apart gets past both within the derived bound, no
/// AI car stands > 30 s near them, no third body is driven through, every relaxation starts apart from
/// a standing body and ends within its bound.
#[test]
fn car_gets_past_two_standing_bodies_in_a_row() {
    let mut failures: Vec<String> = Vec::new();
    for (floor, gap, person) in [
        (Floor::Loop, 1.0, false),
        (Floor::Loop, 3.0, false),
        (Floor::Loop, 1.0, true),
        (Floor::Loop, 3.0, true),
        (Floor::BusyStreet, 1.0, false),
        (Floor::BusyStreet, 3.0, true),
    ] {
        failures.extend(two_in_a_row(floor, gap, person));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
