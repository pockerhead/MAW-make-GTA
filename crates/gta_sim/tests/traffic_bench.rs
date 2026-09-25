//! Frame budget with traffic at 5 stars (GDD §11: physics + AI <= 4 ms per tick): the seed-1 city,
//! 40 civilians, gangs, the player driving an avenue with 12 units and 5 police cars after it and a
//! full traffic bubble. Liveness plus an order-of-magnitude mean (per-tick max has OS spikes).

mod common;
mod police_support;
mod traffic_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    navigation::RouteLoad,
    police::PoliceCar,
    traffic::{TrafficGraph, TrafficStats},
    vehicle::{Vehicle, VehicleConfig, VehicleLoad, pursuit_steer},
    world::{City, CityParamsRes},
};
use std::time::{Duration, Instant};
use vehicle_support::*;
use wanted_support::*;

/// Probe mean of this bench on the implementer's machine was 1.86 ms (TASK-016, 21 traffic cars, 5 police
/// cars, 12 units, 40 civilians); ten times that, rounded up.
const MEAN_LIMIT: Duration = Duration::from_millis(19);
const OBSERVED_TICKS: u32 = 640;

/// Steers the player's car along the lane ahead at `speed` m/s (pure pursuit 8 m ahead).
fn cruise(app: &mut App, car: Entity, speed: f32) {
    let cfg = app.world().resource::<VehicleConfig>().clone();
    let graph = app.world().resource::<TrafficGraph>().clone();
    let (p, f) = (position_of(app, car), forward_of(app, car));
    let v = velocity_of(app, car).dot(f);
    let lane = graph
        .lanes()
        .iter()
        .filter(|l| l.dir.dot(f) > 0.7)
        .min_by(|a, b| {
            let d = |l: &gta_sim::traffic::TrafficLane| {
                let s = (p - l.from).dot(l.dir).clamp(0.0, l.length);
                (l.from + l.dir * s - p).with_y(0.0).length()
            };
            d(a).total_cmp(&d(b))
        })
        .cloned();
    let steer = lane.map_or(0.0, |l| {
        let s = (p - l.from).dot(l.dir) + 8.0;
        let target = if s <= l.length {
            l.from + l.dir * s
        } else {
            let next = l
                .out
                .iter()
                .map(|&c| graph.lane(graph.connector(c).to_lane))
                .max_by(|a, b| a.dir.dot(l.dir).total_cmp(&b.dir.dot(l.dir)))
                .expect("GATE BROKEN: dead-end lane");
            next.from + next.dir * (s - l.length).min(next.length)
        };
        pursuit_steer(&cfg, p, f, v, target)
    });
    set_drive(app, |d| {
        d.throttle = ((speed - v) * 0.5).clamp(-1.0, 1.0);
        d.steer = steer;
    });
}

#[test]
fn traffic_bench() {
    let mut app = city_app(1);
    settle(&mut app);
    set_population(&mut app, |p| p.max_civilians = 40);
    set_player_armor(&mut app, 1.0e6);
    let (spot, curb, offset) = {
        let world = app.world();
        let params = &world.resource::<CityParamsRes>().0;
        let spot = *world
            .resource::<City>()
            .0
            .parking
            .iter()
            .filter(|s| s.heading.dot(-s.position) > 0.0)
            .min_by(|a, b| a.position.length().total_cmp(&b.position.length()))
            .expect("GATE BROKEN: no parked car facing the centre");
        (
            spot,
            params.roads.curb_height,
            params.parking.curb_offset + params.roads.avenue.sidewalk / 2.0,
        )
    };
    let heading = Vec3::new(spot.heading.x, 0.0, spot.heading.y);
    let right = Vec3::new(-heading.z, 0.0, heading.x);
    let feet = Vec3::new(spot.position.x, curb, spot.position.y) + right * offset;
    let float = float_height(&app);
    place_player(&mut app, feet + Vec3::Y * float);
    run_ticks(&mut app, 16);
    let at = Vec3::new(spot.position.x, 0.0, spot.position.y);
    let car = app
        .world_mut()
        .query_filtered::<(Entity, &Position), (With<Vehicle>, Without<PoliceCar>)>()
        .iter(app.world())
        .min_by(|a, b| (a.1.0 - at).length().total_cmp(&(b.1.0 - at).length()))
        .map(|(e, _)| e)
        .expect("GATE BROKEN: no parked car");
    drive_in(&mut app, car);
    let heat = wanted_cfg(&app).stars[4].heat;
    set_heat(&mut app, heat);
    let mut waited = 0;
    loop {
        let me = position(&mut app);
        let view_dir = forward_of(&app, car).with_y(0.0).normalize();
        set_view(&mut app, Some(chase_view(me - Vec3::Y * float, view_dir)));
        cruise(&mut app, car, 6.0);
        {
            let mut w = app
                .world_mut()
                .resource_mut::<gta_sim::wanted::WantedLevel>();
            w.hidden = 0.0;
            w.last_known = Some(me);
        }
        run_ticks(&mut app, 1);
        waited += 1;
        let cars = app.world().resource::<TrafficStats>().cars;
        let police = app
            .world_mut()
            .query::<&PoliceCar>()
            .iter(app.world())
            .filter(|c| c.active())
            .count();
        if cars >= 20 && police == 5 {
            break;
        }
        assert!(
            waited <= 3840,
            "GATE BROKEN: traffic {cars} cars, {police} police cars after {waited} ticks"
        );
    }
    let mut times = Vec::with_capacity(OBSERVED_TICKS as usize);
    for _ in 0..OBSERVED_TICKS {
        let me = position(&mut app);
        let view_dir = forward_of(&app, car).with_y(0.0).normalize();
        set_view(&mut app, Some(chase_view(me - Vec3::Y * float, view_dir)));
        cruise(&mut app, car, 6.0);
        app.world_mut()
            .resource_mut::<gta_sim::wanted::WantedLevel>()
            .hidden = 0.0;
        let start = Instant::now();
        run_ticks(&mut app, 1);
        times.push(start.elapsed());
    }
    let mean = times.iter().sum::<Duration>() / OBSERVED_TICKS;
    times.sort();
    let pick = |q: f32| times[((times.len() - 1) as f32 * q) as usize];
    println!(
        "traffic bench x {OBSERVED_TICKS} ticks (ready after {waited}): mean {mean:?} p50 {:?} p95 {:?} \
         max {:?}; {:?}; {:?}; {:?}",
        pick(0.5),
        pick(0.95),
        times.last().unwrap(),
        app.world().resource::<TrafficStats>(),
        app.world().resource::<VehicleLoad>(),
        app.world().resource::<RouteLoad>(),
    );
    assert!(mean < MEAN_LIMIT, "mean tick {mean:?} >= {MEAN_LIMIT:?}");
}
