//! G6 (TASK-032): police close in on a fleeing driver through traffic. Production city, population and
//! traffic, seeds 1..=10: the player's car cruises at 12 m/s (street v0) along the two-way avenue (or
//! street) with the longest straight run ahead, three AI cars queued 8 m apart behind it, a responding police car
//! 40 m behind the last one. Named mutations: no dispatched police cars (the subject is the one behind
//! the queue), the followers go straight on at every junction like the player (the queue stays
//! between them), the road ahead of the player is kept clear, the camera looks back over the column
//! (the bubble keeps what is in frame), the player is reported all the time (the subject is closing
//! in, not the search), armour so nobody dies.
//!
//! - Liveness: on avenues that police car or its crew comes within `PRESSURE_M` of the player's car
//!   within `CHASE_S` in at least `SEEDS_NEEDED` of the 10 seeds (t15 numbers; TASK-016 baseline
//!   3/5). Streets are reported only (orchestrator fallback chain, see the street test).
//! - The G1 oracle stays clean in every run.

mod common;
mod police_support;
mod traffic_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    combat::aim_yaw,
    police::{CrewOf, UnitKind},
    traffic::{Segment, TrafficCar, TrafficConfig},
    vehicle::VehicleConfig,
    wanted::WantedLevel,
};
use police_support::*;
use traffic_support::*;
use vehicle_support::*;
use wanted_support::*;

const HZ: u32 = 64;
/// t15 `PRESSURE_M` and `CHASE_S`.
const PRESSURE_M: f32 = 18.0;
const CHASE_S: u32 = 25;
const SPEED: f32 = 12.0;
const SEEDS_NEEDED: usize = 8;

/// Closest approach of the police car or its crew to the player's car within `CHASE_S`, and the
/// second it first came within `PRESSURE_M`.
fn chase(seed: u64, avenue: bool) -> (f32, Option<f32>, f32) {
    let mut app = city_app(seed);
    settle(&mut app);
    no_police_cars(&mut app);
    set_player_armor(&mut app, 1.0e6);
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let g = graph(&app);
    let v0 = if avenue {
        cfg.desired_speed.avenue
    } else {
        cfg.desired_speed.street
    };
    let chain = straightest_road(&g, v0);
    let run: f32 = chain.iter().map(|&seg| g.length(seg)).sum();
    let half = app.world().resource::<VehicleConfig>().half_extents();
    // Followers queued at 8 m behind the player (they open up to the IDM spacing on their own), the
    // police car 40 m behind the last one: 64 m behind the player, inside the 90 m bubble around him
    // (a car beyond it is the bubble's to drop). A follower that would start on a connector (no
    // grant) starts at the beginning of the next lane.
    let player_d = 70.0;
    let followers_d = [8.0, 16.0, 24.0].map(|k| player_d - k);
    let police_d = followers_d[2] - 40.0;
    let pose = |d: f32| {
        let (seg, s) = place_on(&g, &chain, d);
        g.pose(seg, s)
    };
    for d in [police_d, player_d].into_iter().chain(followers_d) {
        clear_spot(&mut app, pose(d).0, 6.0);
    }
    let (at, dir) = pose(player_d);
    let car = spawn_car(&mut app, Vec2::new(at.x, at.z), aim_yaw(dir).to_degrees());
    drive_in(&mut app, car);
    kick(&mut app, car, SPEED);
    let followers = followers_d.map(|d| {
        let (seg, s) = match place_on(&g, &chain, d) {
            (Segment::Connector(c), _) => (Segment::Lane(g.connector(c).to_lane), half.z + 0.5),
            lane => lane,
        };
        spawn_traffic_car(&mut app, seg, s, SPEED)
    });
    let (p, dir) = pose(police_d);
    let police = spawn_police_car(&mut app, p, dir, SPEED, vec![UnitKind::Patrol; 2]);
    let heat = wanted_cfg(&app).stars[1].heat;
    raise_heat(&mut app, heat);
    let mut oracle = Footprints::new(&app);
    let mut closest = f32::INFINITY;
    let mut pressure = None;
    eprintln!(
        "seed {seed}: {run:.0} m straight ahead ({} segments)",
        chain.len()
    );
    for tick in 0..CHASE_S * HZ {
        cruise(&mut app, car, SPEED);
        keep_straight(&mut app, &followers);
        clear_ahead(&mut app, car);
        let me = position_of(&app, car);
        // Named mutation: the camera looks back over the column (the bubble keeps what is in frame;
        // off frame it drops cars 25 m behind a driver, and the queue would vanish).
        let view = chase_view(me, -forward_of(&app, car));
        set_view(&mut app, Some(view));
        {
            let mut w = app.world_mut().resource_mut::<WantedLevel>();
            w.last_known = Some(me);
            w.hidden = 0.0;
        }
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let me = position_of(&app, car);
        let crew: Vec<Vec3> = app
            .world_mut()
            .query::<(&CrewOf, &Position)>()
            .iter(app.world())
            .filter(|(c, _)| c.car == police)
            .map(|(_, p)| p.0)
            .collect();
        let d = app
            .world()
            .get::<Position>(police)
            .map(|p| p.0)
            .into_iter()
            .chain(crew)
            .map(|p| (p - me).with_y(0.0).length())
            .fold(f32::INFINITY, f32::min);
        closest = closest.min(d);
        if d <= PRESSURE_M && pressure.is_none() {
            pressure = Some(tick as f32 / HZ as f32);
        }
    }
    oracle.assert_clean(&format!("close in seed {seed}"));
    (closest, pressure, oracle.max_depth())
}

/// Named mutation: the followers take the straight exit at every junction (they stay between the
/// police car and the player, who cruises straight on).
fn keep_straight(app: &mut App, cars: &[Entity]) {
    let g = graph(app);
    for &car in cars {
        let Some(t) = app.world().get::<TrafficCar>(car).copied() else {
            continue;
        };
        let Segment::Lane(l) = t.segment else {
            continue;
        };
        if t.next.is_some() || !t.is_ai() {
            continue;
        }
        let dir = g.lane(l).dir;
        let straight = g
            .lane(l)
            .out
            .iter()
            .copied()
            .find(|&c| g.lane(g.connector(c).to_lane).dir.dot(dir) > 0.99);
        if let Some(c) = straight {
            set_car(app, car, |t| t.next = Some(c));
        }
    }
}

fn seeds(avenue: bool) -> Vec<u64> {
    let mut hits = Vec::new();
    for seed in 1..=10 {
        let (closest, pressure, depth) = chase(seed, avenue);
        eprintln!(
            "seed {seed}: closest {closest:.1} m, within {PRESSURE_M} m at {pressure:?} s, G1 depth {depth:.3}"
        );
        if pressure.is_some() {
            hits.push(seed);
        }
    }
    eprintln!("pressure in {} / 10 seeds: {hits:?}", hits.len());
    hits
}

#[test]
#[ignore = "TASK-032 stop rule: 2/10 on avenues, 0/10 on streets (REDESIGN_NOTE.md)"]
fn police_close_in_on_avenues() {
    let hits = seeds(true);
    assert!(
        hits.len() >= SEEDS_NEEDED,
        "police closed in within {CHASE_S} s in {} / 10 seeds ({hits:?}), need {SEEDS_NEEDED}",
        hits.len()
    );
}

/// Streets are reported, not asserted (orchestrator fallback): a street is 6.5 m of asphalt, so a
/// police car (2.4 m) fits between a car yielded to one curb and an oncoming car yielded to the other
/// (1.7 m apart) only where they are staggered. The G1 oracle is still asserted.
#[test]
#[ignore = "report only (TASK-032 REDESIGN_NOTE.md)"]
fn police_close_in_on_streets_numbers() {
    seeds(false);
}
