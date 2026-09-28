//! Traffic progress (TASK-039), city rows: a car stuck behind a standing body with no way around
//! squeezes past it with collision relaxed against that body only (`traffic::progress`); the checker
//! and mechanism rows are in `traffic_progress_plumbing.rs`.
//!
//! - M1 (liveness + correctness, TASK-040): the player's car left on a street and a traffic car bumped
//!   onto the centre line block both lanes (seed 1).
//! - N1 (liveness + correctness, TASK-040): the player standing on the crosswalk at the east
//!   approach's lane end of box 83 (seed 1); the approach head stops before him.
//!
//! City rows: the production city with a stationary player facing the scene, the production
//! population, no input; each collects all violations before panicking.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    traffic::{FlatRect, Segment, TrafficCar, TrafficMode},
    vehicle::VehicleConfig,
};
use std::collections::HashSet;
use traffic_support::{progress::*, *};

const HZ: u32 = PROGRESS_HZ;

// ---------------------------------------------------------------------------------------------
// M1.

/// M1 (TASK-040): east of the park on seed 1, the player's car P parked 0.6 m right of the eastbound
/// line near (34.9, -76.9) and a traffic car D bumped into `Dynamic` and put 1.85 m left of that line
/// near (34.0, -79.2) (0.05 m from P): no way past on either lane. The player on the north sidewalk
/// looks at them; feeders on both lanes.
struct M1 {
    app: App,
    east: u32,
    west: u32,
    d: Entity,
    p: Entity,
}

const M1_AT: Vec3 = Vec3::new(34.5, 0.0, -78.5);

fn m1_scene() -> M1 {
    let mut app = city_app(1);
    let g = graph(&app);
    let east = lane_of(&g, Vec3::new(34.4, 0.0, -77.5));
    let west = lane_of(&g, Vec3::new(40.2, 0.0, -80.4));
    let (le, lw) = (g.lane(east).clone(), g.lane(west).clone());
    assert!(
        le.dir.dot(lw.dir) < -0.99 && le.dir.x > 0.99,
        "GATE BROKEN: M1 lanes {east} / {west} are not an eastbound / westbound pair"
    );
    let feet = sidewalk_at(&app, Vec3::new(35.0, 0.0, -70.5), 0.0);
    stand_player(&mut app, Vec2::new(feet.x, feet.z), M1_AT - feet);
    run_ticks(&mut app, 2 * HZ);
    clear_spot(&mut app, M1_AT, 10.0);
    let half = app.world().resource::<VehicleConfig>().half_extents();
    // D: spawned on the eastbound line, switched by a real contact (a parked car kicked into its rear).
    let d_line = lane_point(&g, east, 34.0, 0.0);
    let s = (d_line - le.from).with_y(0.0).dot(le.dir);
    let d = spawn_traffic_car(&mut app, Segment::Lane(east), s, 0.0);
    let pusher = park_car(&mut app, d_line - le.dir * (2.0 * half.z + 1.0), le.dir);
    let mut switched = false;
    for _ in 0..2 * HZ {
        app.world_mut().get_mut::<LinearVelocity>(pusher).unwrap().0 = le.dir * 2.0;
        run_ticks(&mut app, 1);
        if traffic_car(&app, d).mode == TrafficMode::Dynamic {
            switched = true;
            break;
        }
    }
    assert!(switched, "GATE BROKEN: M1: the nudge did not switch D");
    app.world_mut().despawn(pusher);
    // Named mutation: D put across the centre line (TASK-040 bumped it there), heading kept.
    let rest = app.world().resource::<VehicleConfig>().rest_height();
    teleport(&mut app, d, lane_point(&g, east, 34.0, -1.85).with_y(rest));
    app.world_mut().get_mut::<LinearVelocity>(d).unwrap().0 = Vec3::ZERO;
    app.world_mut().get_mut::<AngularVelocity>(d).unwrap().0 = Vec3::ZERO;
    let p = park_car(&mut app, lane_point(&g, east, 34.9, 0.6), le.dir);
    run_ticks(&mut app, 2 * HZ);
    let (fd, fp) = (footprint(&app, d), footprint(&app, p));
    let band = |lane: u32, x: f32| {
        FlatRect::of(
            lane_point(&g, lane, x, 0.0),
            Quat::from_rotation_y(gta_sim::combat::aim_yaw(g.lane(lane).dir)),
            Vec2::new(half.x, half.z),
        )
    };
    let broken = [
        (traffic_car(&app, d).mode != TrafficMode::Dynamic).then_some("D is not Dynamic"),
        ((fd.centre - Vec2::new(34.0, -79.2)).length() > 0.5).then_some("D is off (34.0, -79.2)"),
        ((fp.centre - Vec2::new(34.9, -76.9)).length() > 0.5).then_some("P is off (34.9, -76.9)"),
        obb_overlap(&fd, &fp).then_some("D overlaps P"),
        (!obb_overlap(&band(east, 34.9), &fp)).then_some("P leaves the eastbound line free"),
        (!obb_overlap(&band(west, 34.0), &fd)).then_some("D leaves the westbound line free"),
    ];
    let broken: Vec<&str> = broken.into_iter().flatten().collect();
    assert!(
        broken.is_empty(),
        "GATE BROKEN: M1 fixture: {broken:?} (D at {}, P at {})",
        fd.centre,
        fp.centre
    );
    M1 {
        app,
        east,
        west,
        d,
        p,
    }
}

#[test]
fn m1_two_bodies_block_both_lanes_seed_1() {
    let mut m1 = m1_scene();
    let mut feeders = [
        Feeder::new(m1.east, 4 * HZ, 8),
        Feeder::new(m1.west, 4 * HZ, 8),
    ];
    let g = graph(&m1.app);
    let half = m1.app.world().resource::<VehicleConfig>().half_extents();
    // Cars on each lane seen behind the scene (clear of both bodies) and later past it (the rear
    // beyond the far body).
    let mut behind: [HashSet<Entity>; 2] = Default::default();
    let mut past: [HashSet<Entity>; 2] = Default::default();
    let (east, west) = (m1.east, m1.west);
    let seen = watch(&mut m1.app, M1_AT, 120, |app, tick| {
        for f in &mut feeders {
            f.tick(app, tick);
        }
        for (e, c, p) in app
            .world_mut()
            .query::<(Entity, &TrafficCar, &Position)>()
            .iter(app.world())
        {
            let (k, lane) = match c.segment {
                Segment::Lane(l) if l == east => (0, l),
                Segment::Lane(l) if l == west => (1, l),
                _ => continue,
            };
            let dir = g.lane(lane).dir;
            let (front, rear) = ((p.0 + dir * half.z).x, (p.0 - dir * half.z).x);
            if !c.is_ai() {
                continue;
            }
            if (k == 0 && front < 31.0) || (k == 1 && front > 38.0) {
                behind[k].insert(e);
            }
            if ((k == 0 && rear > 37.0) || (k == 1 && rear < 31.0)) && behind[k].contains(&e) {
                past[k].insert(e);
            }
        }
    });
    let mut failures = scene_failures(&seen, M1_AT);
    let passed = past.each_ref().map(|p| p.len());
    let fed = feeders.each_ref().map(|f| f.spawned.len());
    if passed.iter().any(|&n| n < 4) {
        failures.push(format!(
            "cars from behind the scene past it per lane (east, west) {passed:?} < 4 (fed {fed:?})"
        ));
    }
    if stats(&m1.app).progress_relaxations < 1 {
        failures.push("the progress rule never fired".into());
    }
    report("M1", &m1.app, &seen);
    eprintln!(
        "M1: D {:?} at {:?}, P at {:?}, fed {fed:?}, past {passed:?}, first relaxations {:?}",
        m1.app.world().get::<TrafficCar>(m1.d).map(|c| c.mode),
        m1.app.world().get::<Position>(m1.d).map(|p| p.0),
        m1.app.world().get::<Position>(m1.p).map(|p| p.0),
        seen.relaxations.iter().take(3).collect::<Vec<_>>()
    );
    assert!(failures.is_empty(), "M1: {failures:#?}");
}

// ---------------------------------------------------------------------------------------------
// N1.

/// N1 (TASK-040): the player standing still at (7.9, -81.3) on seed 1, 100 s, no input. That point is
/// the end of box 83's east approach (lane 297 on seed 1: 0.5 m before its stop line, by the
/// crosswalk), not the box: the approach head stands before him on its lane. `GATE BROKEN` unless the
/// capsule lies on the body strip of exactly one approach's end (6 m before its stop line to 3 m
/// past it).
#[test]
fn n1_player_standing_on_the_crosswalk_seed_1() {
    let mut app = city_app(1);
    let at = Vec3::new(7.9, 0.0, -81.3);
    let (node, hub) = nearest_box(&app, at);
    let g = graph(&app);
    let loco = app.world().resource::<LocomotionConfig>().clone();
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let on_end = |lane: &gta_sim::traffic::TrafficLane| {
        let centre = lane.from + lane.dir * (lane.stop - 1.5);
        let strip = FlatRect::of(
            centre,
            Quat::from_rotation_y(gta_sim::combat::aim_yaw(lane.dir)),
            Vec2::new(half.x, 4.5),
        );
        gta_sim::traffic::swept_circle_hits_rect(
            Vec2::new(at.x, at.z),
            loco.capsule_radius,
            Vec2::ZERO,
            &strip,
        )
    };
    let approaches: Vec<u32> = (0..g.lanes().len() as u32)
        .filter(|&l| g.lane(l).end_node == node && on_end(g.lane(l)))
        .collect();
    let [approach] = approaches[..] else {
        panic!(
            "GATE BROKEN: N1: the player at {at} stands on the lane end of {approaches:?} of box {node}"
        );
    };
    let dir = -g.lane(approach).dir;
    stand_player(&mut app, Vec2::new(at.x, at.z), dir);
    let player = player(&mut app);
    let health = health_of(&app, player);
    let mut unhurt = Unhurt::new(&mut app, player);
    // The approach's cars that drove past the player: on a connector from it, 3 m in.
    let mut passed: HashSet<Entity> = HashSet::new();
    let seen = watch(&mut app, hub, 100, |app, tick| {
        unhurt.check(app, tick);
        for (e, c) in app
            .world_mut()
            .query::<(Entity, &TrafficCar)>()
            .iter(app.world())
        {
            if let Segment::Connector(k) = c.segment
                && g.connector(k).from_lane == approach
                && c.s > 3.0
            {
                passed.insert(e);
            }
        }
    });
    let mut failures = scene_failures(&seen, hub);
    failures.extend(unhurt.failures("the player"));
    let now = health_of(&app, player);
    if now.current != health.current || now.armor != health.armor {
        failures.push(format!("the player's health went {health:?} -> {now:?}"));
    }
    let against = seen.relaxations.iter().filter(|r| r.1 == player).count();
    if against == 0 {
        failures.push("no relaxation against the player".into());
    }
    report("N1", &app, &seen);
    eprintln!(
        "N1: box {node} at ({:.1}, {:.1}), approach lane {approach}, {} of its cars drove past the player, \
         {against} relaxations against him, height change {:.3} m",
        hub.x,
        hub.z,
        passed.len(),
        unhurt.lift
    );
    assert!(failures.is_empty(), "N1: {failures:#?}");
}
