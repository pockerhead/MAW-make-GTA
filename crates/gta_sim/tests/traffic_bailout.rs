//! A shot into a traffic car's cabin (correctness, GDD §5.2 Q-Б): the car brakes, one driver gets
//! out and flees about that shot, the shot is a shooting incident, and the driver's call about it
//! raises the heat. A hood hit scares nobody; a second cabin hit brings no second driver.
//!
//! Floor: the perimeter loop; lane 0 runs +X along z 37. The player stands 8 m to the car's left
//! (z 29) with a pistol, no person near; a dead-end sidewalk run at z 33, x 24..34 lies beside the
//! stop point (TASK-026: a fleer needs a way to run for its flight to end).

mod common;
mod traffic_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    civilian::{Civilian, CivilianConfig, CivilianState},
    combat::{TraceHit, Weapon},
    perception::Cause,
    traffic::{Segment, TrafficMode},
    vehicle::VehicleConfig,
    wanted::Crime,
};
use traffic_support::*;
use vehicle_support::*;
use wanted_support::*;

const LANE_Z: f32 = 37.0;
const RUN: (Vec3, Vec3) = (Vec3::new(24.0, 0.0, 33.0), Vec3::new(34.0, 0.0, 33.0));

fn setup() -> (App, Entity) {
    let (lanes, connectors) = loop_lanes(12.0);
    let mut app = traffic_floor(lanes, &connectors, &[RUN]);
    let float = float_height_of(&app);
    place_player(&mut app, Vec3::new(18.0, float, LANE_Z - 8.0));
    arm(&mut app, Weapon::Pistol);
    run_ticks(&mut app, 1);
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 2.04, 12.0);
    (app, car)
}

/// Runs until the car's centre passes `x`.
fn until_x(app: &mut App, car: Entity, x: f32) {
    for _ in 0..640 {
        if position_of(app, car).x >= x {
            return;
        }
        run_ticks(app, 1);
    }
    panic!("GATE BROKEN: the car never reached x {x}");
}

/// Fires at the body-frame point `local` of the car; returns (attack, car position at the shot).
fn shoot(app: &mut App, probe: &mut Probe, car: Entity, local: Vec3) -> (u32, Vec3) {
    let at = position_of(app, car);
    let target = at + rotation_of(app, car) * local;
    let attack = fire_at(app, probe, target);
    (attack, at)
}

fn civilians_of(app: &mut App) -> Vec<(Vec3, CivilianState)> {
    app.world_mut()
        .query::<(&Position, &Civilian)>()
        .iter(app.world())
        .map(|(p, c)| (p.0, c.state))
        .collect()
}

/// Left side window, cabin (body frame).
const WINDOW: Vec3 = Vec3::new(-1.2, 0.5, 0.1);
/// Left front fender: outside the cabin zone (z −1.8 − 0.1 beyond ±1.0).
const FENDER: Vec3 = Vec3::new(-1.2, 0.3, -1.8);

#[test]
fn a_cabin_shot_makes_the_driver_bail_out_and_call() {
    let (mut app, car) = setup();
    app.world_mut()
        .resource_mut::<CivilianConfig>()
        .call_after_flee = 1.0; // named mutation: the fleeing driver always calls (F8)
    let mut probe = Probe::new(&app);
    until_x(&mut app, car, 15.0);
    let (attack, at) = shoot(&mut app, &mut probe, car, WINDOW);
    let mode = traffic_car(&app, car).mode;
    assert!(
        matches!(mode, TrafficMode::Bailing { attack: Some(a), .. } if a == attack),
        "mode after the cabin shot {mode:?}"
    );
    // Brakes at max_deceleration 8 from 12 m/s: 9 m, plus a metre.
    let mut stop = None;
    for _ in 0..640 {
        probe.run(&mut app, 1);
        if traffic_car(&app, car).mode == TrafficMode::Abandoned {
            stop = Some(position_of(&app, car));
            break;
        }
    }
    let stop = stop.expect("the driver never got out");
    assert!(
        stop.x - at.x <= 10.0,
        "stopped {} m past the hit",
        stop.x - at.x
    );
    assert!(app.world().get::<RigidBody>(car).unwrap().is_dynamic());
    let cfg = app.world().resource::<VehicleConfig>().clone();
    let rotation = rotation_of(&app, car);
    let door = cfg.door();
    let doors = [door, Vec3::new(-door.x, door.y, door.z)].map(|d| stop + rotation * d);
    let people = civilians_of(&mut app);
    assert_eq!(people.len(), 1, "exactly one driver: {people:?}");
    let (p, state) = people[0];
    assert!(
        doors.iter().any(|d| (p - *d).with_y(0.0).length() <= 1.0),
        "the driver at {p} is not at a door exit spot {doors:?}"
    );
    assert!(
        matches!(state, CivilianState::Flee { about: Some(Cause::Attack(a)), .. } if a == attack),
        "driver state {state:?}"
    );
    assert!(
        incidents(&app)
            .iter()
            .any(|i| i.crime == Crime::Shooting && !i.reported),
        "no shooting incident: {:?}",
        incidents(&app)
    );
    let heat = wanted(&app).heat;
    let limit = longest_call_ticks(&app);
    probe.run_until_calls(&mut app, 1, limit);
    assert_eq!(probe.call_log[0].about, Cause::Attack(attack));
    run_ticks(&mut app, 1);
    let rise = wanted(&app).heat - heat;
    assert_eq!(
        rise,
        wanted_cfg(&app).heat.shooting_near_people,
        "the call about the cabin shot"
    );
}

#[test]
fn a_hood_hit_scares_nobody() {
    let (mut app, car) = setup();
    let mut probe = Probe::new(&app);
    until_x(&mut app, car, 15.0);
    let (attack, _) = shoot(&mut app, &mut probe, car, FENDER);
    let trace = *probe
        .shots
        .trace_log
        .iter()
        .find(|t| t.attack == attack)
        .expect("GATE BROKEN: no trace of the shot");
    // The pellet stopped on the car's front (the car moved at most 0.2 m since the shot).
    let local = rotation_of(&app, car).inverse() * (trace.to - position_of(&app, car));
    assert!(
        !matches!(trace.hit, TraceHit::Nothing | TraceHit::Body)
            && local.z < -1.2
            && local.x < -1.0,
        "GATE BROKEN: the shot did not hit the car's front: {trace:?} (local {local})"
    );
    probe.run(&mut app, 320);
    assert!(
        matches!(traffic_car(&app, car).mode, TrafficMode::Kinematic),
        "mode {:?}",
        traffic_car(&app, car).mode
    );
    assert!(civilians_of(&mut app).is_empty(), "a driver got out");
}

#[test]
fn a_second_cabin_hit_brings_no_second_driver() {
    let (mut app, car) = setup();
    let mut probe = Probe::new(&app);
    until_x(&mut app, car, 15.0);
    shoot(&mut app, &mut probe, car, WINDOW);
    probe.run(&mut app, 24);
    assert!(
        matches!(traffic_car(&app, car).mode, TrafficMode::Bailing { .. }),
        "GATE BROKEN: first hit did not scare the driver"
    );
    shoot(&mut app, &mut probe, car, WINDOW);
    probe.run(&mut app, 640);
    assert_eq!(traffic_car(&app, car).mode, TrafficMode::Abandoned);
    assert_eq!(civilians_of(&mut app).len(), 1);
}
