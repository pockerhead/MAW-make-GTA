//! G3 (TASK-032): a bumped traffic car (switched to `Dynamic` by a contact) goes back to kinematic
//! driving on its lane, or is given up; nothing stays `Dynamic` and standing forever.
//!
//! - (a) a low-speed nudge: kinematic and moving along its lane within `recover.seconds + 1 s` after
//!   it came to rest;
//! - (c) the player's car kept pressed against it: no switch/recover flip-flop (it never turns
//!   kinematic while pressed: one switch, no recovery); with a car ahead in its lane it waits;
//! - (d) upside down: abandoned;
//! - (e) shoved out of its lane behind a standing car (a queue), the rejoin corridor blocked by a
//!   parked car that is clear of the car itself: no recovery, no re-switch, and it waits (never given
//!   up); with the parked car clear: recovers. Shoved out of its path where it cannot drive on and
//!   nothing is ahead: given up after `give_up_seconds`.
//!
//! Floors: the loop lanes (lane 0 +X along z 37) and, for (e), the two-way street (lane 0 +X along
//! z 30); a car "held" ahead is a kinematic traffic car in a
//! yield with offset 0 (it stands), released by clearing its manoeuvre. The G1 oracle runs every tick.

mod common;
mod traffic_support;
mod vehicle_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    traffic::{Manoeuvre, Segment, TrafficConfig, TrafficIntersections, TrafficMode},
    vehicle::{Autopilot, DriveIntent, VehicleConfig},
};
use traffic_support::*;
use vehicle_support::{drive_in, set_drive};

const HZ: u32 = 64;

fn floor() -> App {
    let (lanes, connectors) = loop_lanes(12.0);
    traffic_floor(lanes, &connectors, &[])
}

fn cfg(app: &App) -> TrafficConfig {
    app.world().resource::<TrafficConfig>().clone()
}

fn half(app: &App) -> Vec3 {
    app.world().resource::<VehicleConfig>().half_extents()
}

/// A traffic car standing at `s` on lane 0 (held: a yield with offset 0).
fn held_car(app: &mut App, s: f32) -> Entity {
    let car = spawn_traffic_car(app, Segment::Lane(0), s, 0.0);
    set_car(app, car, |c| {
        c.manoeuvre = Manoeuvre::Yield {
            siren: Entity::PLACEHOLDER,
            since: u64::MAX,
            offset: 0.0,
        }
    });
    car
}

fn release(app: &mut App, car: Entity) {
    set_car(app, car, |c| c.manoeuvre = Manoeuvre::None);
}

/// Switches a traffic car to `Dynamic` the way `switch_to_dynamic` does (no contact).
fn switch_by_hand(app: &mut App, car: Entity) {
    let at = position_of(app, car);
    let forward = app.world().get::<Rotation>(car).unwrap().0 * Vec3::NEG_Z;
    app.world_mut().entity_mut(car).insert((
        RigidBody::Dynamic,
        SleepingDisabled,
        Autopilot {
            target: at + forward * 4.0,
            ..default()
        },
        DriveIntent::default(),
    ));
    set_car(app, car, |c| c.mode = TrafficMode::Dynamic);
}

fn mode(app: &App, car: Entity) -> Option<TrafficMode> {
    app.world()
        .get::<gta_sim::traffic::TrafficCar>(car)
        .map(|c| c.mode)
}

fn speed(app: &App, car: Entity) -> f32 {
    app.world()
        .get::<LinearVelocity>(car)
        .map_or(0.0, |v| v.0.length())
}

/// The car `behind` is pushed at 2 m/s into the rear of `car` until the switch; `None` if none came.
fn nudge(app: &mut App, car: Entity, pusher: Entity, oracle: &mut Footprints) -> Option<u32> {
    for tick in 0..2 * HZ {
        app.world_mut().get_mut::<LinearVelocity>(pusher).unwrap().0 = Vec3::X * 2.0;
        run_ticks(app, 1);
        oracle.record(app, tick);
        if mode(app, car) == Some(TrafficMode::Dynamic) {
            return Some(tick);
        }
    }
    None
}

/// (a) Car T stands behind a held car L; a car 1 m behind is pushed into T's rear at 2 m/s. L drives
/// off as soon as T is kinematic again; T is kinematic and moving within `recover.seconds + 1 s` after
/// it came to rest.
#[test]
fn a_nudged_car_recovers() {
    let mut app = floor();
    let (h, c) = (half(&app), cfg(&app));
    let lead = held_car(&mut app, 40.0);
    let car = spawn_traffic_car(
        &mut app,
        Segment::Lane(0),
        40.0 - 2.0 * h.z - c.idm.min_gap,
        0.0,
    );
    set_car(&mut app, car, |t| t.speed = 0.0);
    run_ticks(&mut app, 4);
    let behind = position_of(&app, car) - Vec3::X * (2.0 * h.z + 1.0);
    let pusher = park_car(&mut app, behind, Vec3::X);
    let mut oracle = Footprints::new(&app);
    assert!(
        nudge(&mut app, car, pusher, &mut oracle).is_some(),
        "GATE BROKEN: the nudge did not switch the car"
    );
    let hold = app.world().resource::<VehicleConfig>().hold_speed;
    let (mut rest_at, mut back_at) = (None, None);
    for tick in 0..20 * HZ {
        // A tap: the nudging car backs off at 1 m/s for 0.5 s after the contact (left touching, it
        // would switch the car again the tick it turned kinematic, so it is given up instead).
        if tick < HZ / 2 {
            app.world_mut().get_mut::<LinearVelocity>(pusher).unwrap().0 = Vec3::NEG_X;
        }
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        if rest_at.is_none() && speed(&app, car) <= hold {
            rest_at = Some(tick);
        }
        // The held car drives off once T is kinematic again (a dynamic T would follow it at once).
        if mode(&app, car) == Some(TrafficMode::Kinematic) {
            release(&mut app, lead);
        }
        if rest_at.is_some()
            && mode(&app, car) == Some(TrafficMode::Kinematic)
            && traffic_car(&app, car).speed > 0.0
        {
            back_at = Some(tick);
            break;
        }
    }
    let rest = rest_at.expect("GATE BROKEN: the nudged car never came to rest");
    oracle.assert_clean("nudge");
    let limit = ((c.recover.seconds + 1.0) * HZ as f32) as u32;
    let took = back_at.map(|b| b - rest);
    eprintln!(
        "recovered {:?} s after rest (limit {:.2} s); lateral {:.3}",
        took.map(|t| t as f32 / HZ as f32),
        limit as f32 / HZ as f32,
        traffic_car(&app, car).lateral
    );
    assert!(
        took.is_some_and(|t| t <= limit),
        "the nudged car was not kinematic and moving {:.2} s after rest: {:?} ({:?})",
        limit as f32 / HZ as f32,
        took,
        mode(&app, car)
    );
}

/// (c) The player drives into the rear of a car standing behind a held car and keeps the throttle
/// open: one switch and no recovery while pressed.
#[test]
fn c_pressed_car_does_not_flip_flop() {
    let mut app = floor();
    let (h, c) = (half(&app), cfg(&app));
    let _lead = held_car(&mut app, 40.0);
    let car = spawn_traffic_car(
        &mut app,
        Segment::Lane(0),
        40.0 - 2.0 * h.z - c.idm.min_gap,
        0.0,
    );
    run_ticks(&mut app, 4);
    let behind = position_of(&app, car) - Vec3::X * (2.0 * h.z + 1.5);
    let mine = park_car(&mut app, behind, Vec3::X);
    drive_in(&mut app, mine);
    set_drive(&mut app, |d| d.throttle = 0.4);
    let mut oracle = Footprints::new(&app);
    let (mut switches, mut recoveries) = (Vec::new(), 0);
    let mut last = mode(&app, car);
    let mut first_stand = None;
    let mut counted = stats(&app).switches_by_cause.iter().sum::<u32>();
    for tick in 0..14 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let now = mode(&app, car);
        // A recovery and the switch back can fall in one tick: count the switch counter too.
        let total = stats(&app).switches_by_cause.iter().sum::<u32>();
        if total > counted {
            switches.push(tick);
            counted = total;
        }
        if now == Some(TrafficMode::Kinematic) && last == Some(TrafficMode::Dynamic) {
            recoveries += 1;
        }
        if first_stand.is_none() && now == Some(TrafficMode::Dynamic) && speed(&app, car) < 0.5 {
            first_stand = Some(tick);
        }
        if matches!(
            now,
            Some(TrafficMode::Bailing { .. } | TrafficMode::Abandoned) | None
        ) && !matches!(
            last,
            Some(TrafficMode::Bailing { .. } | TrafficMode::Abandoned) | None
        ) {
            eprintln!(
                "given up at {:.2} s (standing since {:?})",
                tick as f32 / HZ as f32,
                first_stand.map(|t| t as f32 / HZ as f32)
            );
        }
        last = now;
    }
    eprintln!(
        "switches at {:?}, recoveries {recoveries}, final {:?}",
        switches
            .iter()
            .map(|t| *t as f32 / HZ as f32)
            .collect::<Vec<_>>(),
        mode(&app, car)
    );
    assert!(
        !switches.is_empty(),
        "GATE BROKEN: the player's car never switched it"
    );
    oracle.assert_clean("pressed");
    // Pressed, it never turns kinematic (it would be switched again the next tick).
    assert!(
        recoveries == 0 && switches.len() == 1,
        "switch/recover flip-flop while pressed: {recoveries} recoveries, switches at {switches:?} (recover.seconds {})",
        c.recover.seconds
    );
}

/// (d) A car switched and turned upside down is given up.
#[test]
fn d_upside_down_is_abandoned() {
    let mut app = floor();
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 30.0, 0.0);
    run_ticks(&mut app, 2);
    switch_by_hand(&mut app, car);
    let upside =
        Quat::from_rotation_z(std::f32::consts::PI) * app.world().get::<Rotation>(car).unwrap().0;
    app.world_mut().get_mut::<Rotation>(car).unwrap().0 = upside;
    app.world_mut().get_mut::<Transform>(car).unwrap().rotation = upside;
    run_ticks(&mut app, 2);
    assert_eq!(mode(&app, car), Some(TrafficMode::Abandoned));
}

struct Corridor {
    recovered_after: Option<f32>,
    switches: u32,
    final_mode: Option<TrafficMode>,
}

/// (e) On the two-way street, car T standing behind a held car is switched by hand, shoved 1.5 m left
/// (towards the opposite lane) and yawed 30 deg. A parked, sleeping car stands with its flat centre at
/// `parked_z`, right of the lane line (z 30).
fn corridor(parked_z: f32) -> Corridor {
    let (lanes, connectors) = two_way_street(70.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let (h, c) = (half(&app), cfg(&app));
    let _lead = held_car(&mut app, 40.0);
    let s = 40.0 - 2.0 * h.z - c.idm.min_gap;
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), s, 0.0);
    run_ticks(&mut app, 4);
    switch_by_hand(&mut app, car);
    let at = position_of(&app, car) - Vec3::Z * 1.5;
    let yawed =
        Quat::from_rotation_y(30f32.to_radians()) * app.world().get::<Rotation>(car).unwrap().0;
    teleport(&mut app, car, at);
    app.world_mut().get_mut::<Rotation>(car).unwrap().0 = yawed;
    app.world_mut().get_mut::<Transform>(car).unwrap().rotation = yawed;
    let skin = c.recover.skin;
    let along_x = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
    let rect = |centre: Vec3, rotation: Quat, grow: f32| {
        gta_sim::traffic::FlatRect::of(centre, rotation, Vec2::new(h.x + grow, h.z + grow))
    };
    let parked_at = Vec3::new(at.x, 0.0, parked_z);
    let parked_rect = rect(parked_at, along_x, 0.0);
    let final_rect = rect(Vec3::new(at.x, 0.0, 30.0), along_x, skin);
    assert!(
        !obb_overlap(&rect(at, yawed, skin), &parked_rect),
        "GATE BROKEN: the parked car is within the skin of the shoved car"
    );
    let blocks = obb_overlap(&final_rect, &parked_rect);
    let parked = park_car(&mut app, parked_at, Vec3::X);
    app.world_mut().entity_mut(parked).insert(Sleeping);
    let switches = stats(&app).switches_by_cause.iter().sum::<u32>();
    let mut oracle = Footprints::new(&app);
    let mut recovered_after = None;
    let mut last = mode(&app, car);
    let mut re_switches = 0;
    for tick in 0..14 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let now = mode(&app, car);
        if recovered_after.is_none() && now == Some(TrafficMode::Kinematic) {
            recovered_after = Some(tick as f32 / HZ as f32);
        }
        if now == Some(TrafficMode::Dynamic) && last == Some(TrafficMode::Kinematic) {
            re_switches += 1;
        }
        last = now;
    }
    oracle.assert_clean("corridor");
    let counted = stats(&app).switches_by_cause.iter().sum::<u32>() - switches;
    eprintln!(
        "parked z {parked_z} (blocks the corridor end: {blocks}): recovered after {recovered_after:?} s, {re_switches} re-switches ({counted} counted), final {:?} at {:?} (shoved to {at:?})",
        mode(&app, car),
        position_of(&app, car)
    );
    assert!(
        blocks == (parked_z < 33.0),
        "GATE BROKEN: parked car at z {parked_z} blocks the corridor end: {blocks}"
    );
    Corridor {
        recovered_after,
        switches: re_switches.max(counted),
        final_mode: mode(&app, car),
    }
}

/// The blocked car stands behind a car in its lane within the jam gap: given up there it would be one
/// more standing body, and the car behind it the next to be bumped (TASK-032 QA, spot C).
#[test]
fn e_blocked_corridor_waits_in_the_queue_and_clear_corridor_recovers() {
    let limit = {
        let app = floor();
        let c = cfg(&app);
        c.recover.seconds + 1.0
    };
    let blocked = corridor(32.6);
    let clear = corridor(34.0);
    let mut failures = Vec::new();
    if blocked.recovered_after.is_some() || blocked.switches > 0 {
        failures.push(format!(
            "blocked corridor: recovered after {:?} s with {} re-switches",
            blocked.recovered_after, blocked.switches
        ));
    }
    if blocked.final_mode != Some(TrafficMode::Dynamic) {
        failures.push(format!(
            "blocked corridor: a car standing behind a car in its lane was given up: {:?}",
            blocked.final_mode
        ));
    }
    if clear.recovered_after.is_none_or(|t| t > limit) {
        failures.push(format!(
            "clear corridor: recovered after {:?} s (limit {limit} s)",
            clear.recovered_after
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

/// (b) On the two-way street, a car switched and shoved beyond `lost` (1.0 m to the curb side, yawed
/// 65 deg > 60 deg) is abandoned where it stands, and a car coming up behind it goes around it via
/// the opposite lane (its rear past the wreck's front within 30 s).
#[test]
fn b_shoved_beyond_lost_is_abandoned_and_passed() {
    let (lanes, connectors) = two_way_street(70.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let wreck = spawn_traffic_car(&mut app, Segment::Lane(0), 35.0, 0.0);
    run_ticks(&mut app, 2);
    switch_by_hand(&mut app, wreck);
    let yawed =
        Quat::from_rotation_y(65f32.to_radians()) * app.world().get::<Rotation>(wreck).unwrap().0;
    let at = position_of(&app, wreck) + Vec3::Z;
    teleport(&mut app, wreck, at);
    app.world_mut().get_mut::<Rotation>(wreck).unwrap().0 = yawed;
    app.world_mut()
        .get_mut::<Transform>(wreck)
        .unwrap()
        .rotation = yawed;
    run_ticks(&mut app, 2);
    assert_eq!(
        mode(&app, wreck),
        Some(TrafficMode::Abandoned),
        "the shoved car is not abandoned"
    );
    let h = half(&app);
    let front = at.x + h.z * 65f32.to_radians().cos() + h.x * 65f32.to_radians().sin();
    let follower = spawn_traffic_car(&mut app, Segment::Lane(0), 5.0, 8.0);
    let mut oracle = Footprints::new(&app);
    let mut passed = None;
    for tick in 0..30 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        if position_of(&app, follower).x - h.z > front {
            passed = Some(tick as f32 / HZ as f32);
            break;
        }
    }
    eprintln!("follower past the wreck after {passed:?} s");
    oracle.assert_clean("pass a wreck");
    assert!(
        passed.is_some(),
        "the follower did not go around the wreck in 30 s"
    );
}

/// (e) Shoved out of its path where it cannot drive on (on the U connector of the two-way street
/// without its grant, so it holds) with nothing ahead: it never recovers on a connector and is given up
/// after `give_up_seconds` of standing, where it stands.
#[test]
fn e_off_path_car_with_nothing_ahead_gives_up() {
    let (lanes, connectors) = two_way_street(70.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let c = cfg(&app);
    let car = spawn_traffic_car(&mut app, Segment::Connector(0), 1.0, 0.0);
    set_car(&mut app, car, |t| t.next = Some(0));
    switch_by_hand(&mut app, car);
    app.world_mut()
        .resource_mut::<TrafficIntersections>()
        .release(car);
    let (point, tangent) = graph(&app).pose(Segment::Connector(0), 1.0);
    let at = position_of(&app, car) - right_of(tangent) * 1.5;
    let yawed =
        Quat::from_rotation_y(30f32.to_radians()) * app.world().get::<Rotation>(car).unwrap().0;
    teleport(&mut app, car, at);
    app.world_mut().get_mut::<Rotation>(car).unwrap().0 = yawed;
    app.world_mut().get_mut::<Transform>(car).unwrap().rotation = yawed;
    let mut oracle = Footprints::new(&app);
    let mut given_up = None;
    for tick in 0..(c.recover.give_up_seconds as u32 + 4) * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let now = mode(&app, car);
        assert_ne!(
            now,
            Some(TrafficMode::Kinematic),
            "GATE BROKEN: it recovered on the connector"
        );
        if now != Some(TrafficMode::Dynamic) {
            given_up = Some(tick as f32 / HZ as f32);
            break;
        }
    }
    let moved = position_of(&app, car).with_y(0.0).distance(at.with_y(0.0));
    eprintln!(
        "off path {:.2} m from the connector line, given up after {given_up:?} s (give_up_seconds {}), moved {moved:.2} m",
        (at - point).with_y(0.0).length(),
        c.recover.give_up_seconds
    );
    oracle.assert_clean("off path, nothing ahead");
    assert!(
        moved < 0.5,
        "GATE BROKEN: the held car drove {moved:.2} m (not standing)"
    );
    assert!(
        given_up.is_some_and(|t| t >= c.recover.give_up_seconds - 2.0 / HZ as f32),
        "a car out of its path with nothing ahead was not given up after {} s: {given_up:?}",
        c.recover.give_up_seconds
    );
}
