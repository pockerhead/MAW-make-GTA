//! AC4 (correctness, GDD §5.2): a kinematic traffic car turns dynamic before a dynamic body touches
//! it (a kinematic body pushes with infinite mass), and never for bodies that only pass by.
//!
//! Floor: lane 0 runs +X along z 30 from x −30 to 10 into a 5 m destination lane, so its connector is
//! never granted (the destination never has the 6.08 m of room): a car on lane 0 stops at its stop
//! line, centre x ≈ 5.96. The opposite inner lane would run along z 26.75.

mod common;
mod traffic_support;
mod vehicle_support;

use avian3d::prelude::*;
use bevy::{ecs::message::MessageCursor, prelude::*};
use common::*;
use gta_sim::{
    character::Gait,
    traffic::{Segment, TrafficMode, TrafficStats},
};
use traffic_support::*;
use vehicle_support::*;

const LANE_Z: f32 = 30.0;
const OPPOSITE_Z: f32 = 26.75;

fn floor(sidewalk: &[(Vec3, Vec3)]) -> App {
    let at = |x: f32| Vec3::new(x, 0.0, LANE_Z);
    traffic_floor(
        vec![
            (at(-30.0), at(10.0), 12.0, 0),
            (at(20.0), at(25.0), 12.0, 1),
        ],
        &[(0, 1, 0), (1, 0, 1)],
        sidewalk,
    )
}

/// First tick (1-based, counted by the caller) a `CollisionStart` of `a` and `b` is read.
struct Contacts {
    cursor: MessageCursor<CollisionStart>,
}

impl Contacts {
    fn new(app: &App) -> Self {
        Self {
            cursor: app
                .world()
                .resource::<Messages<CollisionStart>>()
                .get_cursor_current(),
        }
    }

    fn touched(&mut self, app: &App, a: Entity, b: Entity) -> bool {
        let messages = app.world().resource::<Messages<CollisionStart>>();
        self.cursor.read(messages).any(|m| {
            let (x, y) = (
                m.body1.unwrap_or(m.collider1),
                m.body2.unwrap_or(m.collider2),
            );
            (x == a && y == b) || (x == b && y == a)
        })
    }
}

fn dynamic(app: &App, car: Entity) -> bool {
    app.world()
        .get::<RigidBody>(car)
        .is_some_and(|b| b.is_dynamic())
}

/// A traffic car held at the stop line of lane 0.
fn stopped_car(app: &mut App) -> Entity {
    let car = spawn_traffic_car(app, Segment::Lane(0), 30.0, 0.0);
    run_ticks(app, 640);
    let t = traffic_car(app, car);
    assert!(
        t.speed == 0.0 && t.mode == TrafficMode::Kinematic && t.segment == Segment::Lane(0),
        "GATE BROKEN: the car is not held at the stop line: {t:?}"
    );
    car
}

/// Runs until the pair touches (at most `limit` ticks): (tick of the switch, tick of the contact).
fn switch_and_contact(app: &mut App, car: Entity, other: Entity, limit: u32) -> (Option<u32>, u32) {
    let mut contacts = Contacts::new(app);
    let mut switched = None;
    for tick in 1..=limit {
        run_ticks(app, 1);
        if switched.is_none() && dynamic(app, car) {
            switched = Some(tick);
        }
        if contacts.touched(app, car, other) {
            return (switched, tick);
        }
    }
    panic!("GATE BROKEN: no contact in {limit} ticks");
}

#[test]
fn rear_end_switches_before_contact() {
    let mut app = floor(&[]);
    let car = stopped_car(&mut app);
    let x = position_of(&app, car).x;
    assert!((x - 6.0).abs() < 0.2, "GATE BROKEN: stop point x {x}");
    let player_car = spawn_car(&mut app, Vec2::new(-12.0, LANE_Z), -90.0);
    drive_in(&mut app, player_car);
    set_drive(&mut app, |d| d.throttle = 1.0);
    let mut contacts = Contacts::new(&app);
    let mut switched = None;
    let mut contact = None;
    let mut min_forward = f32::INFINITY;
    for tick in 1..=640u32 {
        run_ticks(&mut app, 1);
        if switched.is_none() && dynamic(&app, car) {
            switched = Some(tick);
        }
        if contact.is_none() && contacts.touched(&app, car, player_car) {
            contact = Some(tick);
            set_drive(&mut app, |d| d.throttle = 0.0);
        }
        if contact.is_some() {
            min_forward =
                min_forward.min(velocity_of(&app, player_car).dot(forward_of(&app, player_car)));
        }
        if contact.is_some_and(|c| tick == c + 16) {
            let speed = velocity_of(&app, car).length();
            assert!(
                speed >= 1.5,
                "traffic car speed {speed} 16 ticks after the hit"
            );
            break;
        }
    }
    let contact = contact.expect("GATE BROKEN: the player's car never reached the traffic car");
    let switched = switched.expect("the traffic car never turned dynamic");
    assert!(
        switched < contact,
        "switch at tick {switched}, contact at {contact}"
    );
    assert!(
        min_forward >= 0.0,
        "the player's car bounced back: forward {min_forward}"
    );
    let cause = app.world().resource::<TrafficStats>().switches_by_cause;
    eprintln!("switch tick {switched}, contact tick {contact}, causes {cause:?}");
    assert_eq!(cause[1], 1, "switched by the vehicle rule: {cause:?}");
}

#[test]
fn walking_into_the_side_switches_before_contact() {
    let mut app = floor(&[]);
    let car = stopped_car(&mut app);
    let at = position_of(&app, car);
    let float = float_height_of(&app);
    place_player(&mut app, Vec3::new(at.x, float, LANE_Z + 5.0));
    let me = player(&mut app);
    set_intent(&mut app, |i| {
        i.axis = Vec2::Y;
        i.yaw = 0.0;
        i.gait = Gait::Walk;
    });
    let (switched, contact) = switch_and_contact(&mut app, car, me, 640);
    let switched = switched.expect("the traffic car never turned dynamic");
    assert!(
        switched < contact,
        "switch at tick {switched}, contact at {contact}"
    );
}

#[test]
fn a_sleeping_parked_car_beside_the_lane_never_switches() {
    let mut app = floor(&[]);
    // 0.85 m clear of a car on lane 0 (half widths 1.2).
    let parked = spawn_car(&mut app, Vec2::new(-10.0, LANE_Z - 2.4 - 0.85), -90.0);
    run_ticks(&mut app, 320);
    assert!(
        app.world().get::<Sleeping>(parked).is_some(),
        "GATE BROKEN: the parked car does not sleep"
    );
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 2.04, 12.0);
    let mut passed = false;
    for _ in 0..640 {
        run_ticks(&mut app, 1);
        assert!(!dynamic(&app, car), "switched while passing a parked car");
        passed |= position_of(&app, car).x > -10.0 + 4.08;
    }
    assert!(
        passed,
        "GATE BROKEN: the traffic car never passed the parked car"
    );
}

#[test]
fn an_oncoming_car_in_the_opposite_lane_never_switches() {
    let mut app = floor(&[]);
    let player_car = spawn_car(&mut app, Vec2::new(30.0, OPPOSITE_Z), 90.0);
    drive_in(&mut app, player_car);
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 2.04, 12.0);
    set_drive(&mut app, |d| d.throttle = 1.0);
    let mut closest = f32::INFINITY;
    let mut crossed = false;
    for _ in 0..640 {
        run_ticks(&mut app, 1);
        assert!(!dynamic(&app, car), "switched for an oncoming car");
        let (a, b) = (position_of(&app, car), position_of(&app, player_car));
        assert!(
            (b.z - OPPOSITE_Z).abs() < 0.3,
            "GATE BROKEN: the player's car left its lane (z {})",
            b.z
        );
        closest = closest.min((a - b).with_y(0.0).length());
        if a.x > b.x {
            crossed = true;
            break;
        }
    }
    assert!(crossed, "GATE BROKEN: the cars never met");
    let speed = velocity_of(&app, player_car).length();
    eprintln!("closest {closest:.2} m, player car at {speed:.1} m/s");
    assert!(
        speed > 8.0,
        "GATE BROKEN: the player's car was slow ({speed} m/s)"
    );
}

#[test]
fn a_pedestrian_crossing_in_front_never_switches() {
    // A walker heads for the point `keep_right` (0.5 m) right of its next node (−X when walking +Z),
    // so halfway along the run it is 0.25 m right of the edge line: along x 9.56 it crosses the lane
    // 1.2 m in front of the stopped car's bumper (x ≈ 8.11).
    let line = 6.07 + 2.04 + 1.2 + 0.25;
    let run = (
        Vec3::new(line, 0.0, LANE_Z - 8.0),
        Vec3::new(line, 0.0, LANE_Z + 8.0),
    );
    let mut app = floor(&[run]);
    let car = stopped_car(&mut app);
    let bumper = position_of(&app, car).x + 2.04;
    let walker = spawn_civilian(
        &mut app,
        gta_sim::navigation::GraphWalker { from: 0, to: 1 },
        0.0,
        calm(),
    );
    let mut crossing_x = None;
    for _ in 0..1280 {
        run_ticks(&mut app, 1);
        let p = position_of(&app, walker);
        assert!(
            !dynamic(&app, car),
            "switched for a pedestrian crossing in front: walker at {p}, car at {}, causes {:?}",
            position_of(&app, car),
            app.world().resource::<TrafficStats>().switches_by_cause
        );
        if (p.z - LANE_Z).abs() < 0.2 {
            crossing_x = Some(p.x);
        }
        if p.z > LANE_Z + 4.0 {
            break;
        }
    }
    let x = crossing_x.expect("GATE BROKEN: the civilian never crossed the lane");
    assert!(
        (x - bumper - 1.2).abs() < 0.2,
        "GATE BROKEN: crossed at x {x}, not 1.2 m in front of the bumper"
    );
}

#[test]
fn a_sleeping_car_ahead_switches_before_contact() {
    let mut app = floor(&[]);
    // The forward cast off: only the switch stands between the two cars (named mutation).
    set_traffic(&mut app, |t| t.sense_distance = 0.01);
    let parked = spawn_car(
        &mut app,
        Vec2::new(-30.0 + 2.04 + 4.08 + 20.0, LANE_Z),
        -90.0,
    );
    run_ticks(&mut app, 320);
    assert!(
        app.world().get::<Sleeping>(parked).is_some(),
        "GATE BROKEN: the parked car does not sleep"
    );
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 2.04, 12.0);
    let (switched, contact) = switch_and_contact(&mut app, car, parked, 640);
    let switched = switched.expect("the traffic car never turned dynamic");
    assert!(
        switched < contact,
        "switch at tick {switched}, contact at {contact}"
    );
}
