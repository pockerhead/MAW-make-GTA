//! Hijacking a traffic car (correctness, GDD §5.2): the data driver is thrown out as one fleeing
//! civilian at the player's feet, the theft is a crime, the car is the player's; once left it is an
//! abandoned car that stays put, and getting back in throws nobody out again.

mod common;
mod traffic_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::Gait,
    civilian::{Civilian, CivilianState},
    combat::dummy_bundle,
    perception::Cause,
    traffic::{Segment, SwitchCause, TrafficMode, TrafficStats},
    vehicle::{Autopilot, DriveIntent, VehicleEntered},
    wanted::Crime,
};
use traffic_support::*;
use vehicle_support::*;
use wanted_support::incidents;

const LANE_Z: f32 = 30.0;

fn floor() -> App {
    let at = |x: f32| Vec3::new(x, 0.0, LANE_Z);
    traffic_floor(
        vec![
            (at(-30.0), at(10.0), 12.0, 0),
            (at(20.0), at(25.0), 12.0, 1),
        ],
        &[(0, 1, 0), (1, 0, 1)],
        &[],
    )
}

fn spawn_dummy_at(app: &mut App, feet: Vec3) -> Entity {
    let world = app.world();
    let bundle = dummy_bundle(
        world.resource::<gta_sim::character::LocomotionConfig>(),
        world
            .resource::<gta_sim::character::CharacterControlConfig>()
            .0
            .clone(),
        world.resource::<gta_sim::character::HealthConfig>(),
        feet,
    );
    app.world_mut().spawn(bundle).id()
}

/// A traffic car standing behind a dummy 10 m ahead of its bumper in lane 0; returns both.
fn held_car(app: &mut App) -> (Entity, Entity) {
    let car = spawn_traffic_car(app, Segment::Lane(0), 10.0, 0.0);
    let dummy = spawn_dummy_at(app, Vec3::new(-30.0 + 10.0 + 2.04 + 10.0, 0.0, LANE_Z));
    // IDM closes the gap to the jam distance and stops.
    for _ in 0..1920 {
        run_ticks(app, 1);
        if traffic_car(app, car).speed == 0.0 {
            break;
        }
    }
    let t = traffic_car(app, car);
    assert!(
        t.speed < 0.05 && t.mode == TrafficMode::Kinematic,
        "GATE BROKEN: the car is not held by the dummy: {t:?}"
    );
    (car, dummy)
}

/// Stands the player 1.5 m out from the car's door point.
fn stand_at_door(app: &mut App, car: Entity) -> Vec3 {
    let door = door_of(app, car);
    let out = (door - position_of(app, car)).with_y(0.0).normalize();
    let feet = Vec3::new(door.x, 0.0, door.z) + out * 1.5;
    place_player(app, feet + Vec3::Y * float_height_of(app));
    run_ticks(app, 1);
    feet
}

fn civilians_of(app: &mut App) -> Vec<(Entity, Vec3, CivilianState)> {
    app.world_mut()
        .query::<(Entity, &Position, &Civilian)>()
        .iter(app.world())
        .map(|(e, p, c)| (e, p.0, c.state))
        .collect()
}

#[test]
fn hijack_throws_the_driver_out_once() {
    let mut app = floor();
    let (car, _) = held_car(&mut app);
    let feet = stand_at_door(&mut app, car);
    let mut entered = app
        .world()
        .resource::<Messages<VehicleEntered>>()
        .get_cursor_current();
    let before = civilians_of(&mut app).len();
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    let entry = *entered
        .read(app.world().resource::<Messages<VehicleEntered>>())
        .next()
        .expect("GATE BROKEN: no VehicleEntered");
    assert_eq!(driving(&mut app), Some(car));
    assert!(app.world().get::<RigidBody>(car).unwrap().is_dynamic());
    assert_eq!(traffic_car(&app, car).mode, TrafficMode::Taken);
    assert!(app.world().get::<Autopilot>(car).is_none());
    assert!(app.world().get::<DriveIntent>(car).is_none());
    let people = civilians_of(&mut app);
    assert_eq!(people.len(), before + 1, "one driver out");
    let (_, at, state) = people.last().copied().unwrap();
    let float = float_height_of(&app);
    assert!(
        (at - Vec3::Y * float - feet).with_y(0.0).length() < 1.0,
        "the driver stands at {at}, the player got in from {feet}"
    );
    assert!(
        matches!(state, CivilianState::Flee { about: Some(Cause::Attack(a)), .. } if a == entry.attack),
        "driver state {state:?}, entry attack {}",
        entry.attack
    );
    assert!(
        incidents(&app)
            .iter()
            .any(|i| i.crime == Crime::CarTheft && i.victim == Some(car)),
        "no car theft recorded"
    );
    assert_eq!(
        app.world().resource::<TrafficStats>().switches_by_cause[SwitchCause::Hijack as usize],
        1
    );

    // F out: an abandoned car that stays where it stopped.
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_eq!(driving(&mut app), None, "GATE BROKEN: could not get out");
    run_ticks(&mut app, 1);
    assert_eq!(traffic_car(&app, car).mode, TrafficMode::Abandoned);
    let parked = position_of(&app, car);
    run_ticks(&mut app, 128);
    let drift = (position_of(&app, car) - parked).with_y(0.0).length();
    assert!(drift < 0.2, "the empty car drifted {drift} m");

    // Back in: nobody is thrown out again.
    let count = civilians_of(&mut app).len();
    drive_in(&mut app, car);
    run_ticks(&mut app, 1);
    assert_eq!(traffic_car(&app, car).mode, TrafficMode::Taken);
    assert_eq!(
        civilians_of(&mut app).len(),
        count,
        "a second driver came out"
    );
}

#[test]
fn a_hijacked_dynamic_car_stays_put_once_left() {
    let mut app = floor();
    let (car, dummy) = held_car(&mut app);
    // Walk into the car's side until it turns dynamic (its autopilot then creeps it on).
    let at = position_of(&app, car);
    let float = float_height_of(&app);
    place_player(&mut app, Vec3::new(at.x, float, LANE_Z - 5.0));
    set_intent(&mut app, |i| {
        i.axis = Vec2::Y;
        i.yaw = std::f32::consts::PI;
        i.gait = Gait::Walk;
    });
    for _ in 0..320 {
        run_ticks(&mut app, 1);
        if traffic_car(&app, car).mode == TrafficMode::Dynamic {
            break;
        }
    }
    set_intent(&mut app, |i| i.axis = Vec2::ZERO);
    assert_eq!(
        traffic_car(&app, car).mode,
        TrafficMode::Dynamic,
        "GATE BROKEN: the car never turned dynamic"
    );
    // The way ahead clears: the autopilot drives on.
    app.world_mut().despawn(dummy);
    run_ticks(&mut app, 96);
    let throttle = app
        .world()
        .get::<DriveIntent>(car)
        .map_or(0.0, |d| d.throttle);
    assert!(
        throttle > 0.0,
        "GATE BROKEN: the autopilot does not drive the car (throttle {throttle})"
    );
    stand_at_door(&mut app, car);
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_eq!(
        driving(&mut app),
        Some(car),
        "GATE BROKEN: could not get in"
    );
    assert!(
        app.world().get::<Autopilot>(car).is_none()
            && app.world().get::<DriveIntent>(car).is_none(),
        "the AI driver is still aboard the player's car"
    );
    run_ticks(&mut app, 128);
    request_vehicle(&mut app);
    run_ticks(&mut app, 2);
    assert_eq!(driving(&mut app), None, "GATE BROKEN: could not get out");
    let parked = position_of(&app, car);
    run_ticks(&mut app, 128);
    let drift = (position_of(&app, car) - parked).with_y(0.0).length();
    assert!(drift < 0.2, "the empty car drove off {drift} m");
}
