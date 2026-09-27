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
//!   nothing is ahead: given up after `give_up_seconds`;
//! - (g) held in a queue on its line with a parked car at rest 0.25 m beside it (inside `recover.skin`,
//!   outside the rest skin: `switch.skin` plus the rejoin's own move, nothing on its line): recovers
//!   and is not switched again; a standing dummy there keeps it `Dynamic` (a walker keeps the full
//!   skin);
//! - (f) on a connector without its grant: a car shoved 0.10 m off the line (inside the conflict
//!   table's body band, `half.x + conflict_margin / 2`) recovers where it stands, slides back onto the
//!   line without leaving the band, is granted and drives on; shoved 0.25 m (out of the band), or on
//!   the line 0.3 m behind the connector start, it stays `Dynamic`.
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
/// without its grant, so it holds) with nothing ahead: out of the conflict table's band it never
/// recovers on the connector and is given up after `give_up_seconds` of standing, where it stands.
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
            "GATE BROKEN: it recovered out of the table band on a connector"
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

/// Table-band reach of `car` across the path line of its segment, and the band limit.
fn band_reach(app: &App, car: Entity) -> (f32, f32) {
    let (h, c) = (half(app), cfg(app));
    let seg = traffic_car(app, car).segment;
    let at = position_of(app, car);
    let g = graph(app);
    let (point, tangent) = g.pose(seg, g.project(seg, at));
    let across = right_of(tangent);
    let across = Vec2::new(across.x, across.z).normalize();
    let rect = gta_sim::traffic::FlatRect::of(
        at,
        app.world().get::<Rotation>(car).unwrap().0,
        Vec2::new(h.x, h.z),
    );
    let reach = (rect.centre - Vec2::new(point.x, point.z))
        .dot(across)
        .abs()
        + rect.half.x * rect.axis.dot(across).abs()
        + rect.half.y * rect.axis.perp().dot(across).abs();
    (reach, h.x + c.conflict_margin / 2.0)
}

/// (f) A car on the U connector of the two-way street at `s`, switched by hand, its grant released
/// (it holds), shoved `shove` m (right, along) off its pose at the path yaw.
fn connector_shove(s: f32, shove: Vec2) -> (App, Entity) {
    let (lanes, connectors) = two_way_street(70.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let car = spawn_traffic_car(&mut app, Segment::Connector(0), s, 0.0);
    set_car(&mut app, car, |t| t.next = Some(0));
    switch_by_hand(&mut app, car);
    app.world_mut()
        .resource_mut::<TrafficIntersections>()
        .release(car);
    let (point, tangent) = graph(&app).pose(Segment::Connector(0), s);
    let at =
        point.with_y(position_of(&app, car).y) + right_of(tangent) * shove.x + tangent * shove.y;
    let yaw = Quat::from_rotation_y(gta_sim::combat::aim_yaw(tangent));
    teleport(&mut app, car, at);
    app.world_mut().get_mut::<Rotation>(car).unwrap().0 = yaw;
    app.world_mut().get_mut::<Transform>(car).unwrap().rotation = yaw;
    run_ticks(&mut app, 1);
    (app, car)
}

/// (f) Inside the table band: kinematic where it stands within `recover.seconds + 1 s`, the rejoin
/// slides back onto the line in place without the body leaving the band, then it is granted (a waiter
/// on an empty node) and reaches lane 1.
#[test]
fn f_connector_car_recovers_in_the_table_band() {
    let (mut app, car) = connector_shove(1.0, Vec2::new(0.10, 0.0));
    let c = cfg(&app);
    let (reach, limit) = band_reach(&app, car);
    assert!(
        reach < limit - 0.02,
        "GATE BROKEN: the shoved car reaches {reach:.3} m across the line (band {limit:.3})"
    );
    let mut oracle = Footprints::new(&app);
    let mut failures = Vec::new();
    let (mut recovered, mut rejoined, mut on_lane) = (None, None, None);
    let mut lateral_at_recovery = 0.0;
    let mut widest: f32 = 0.0;
    for tick in 0..12 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let t = tick as f32 / HZ as f32;
        let now = traffic_car(&app, car);
        if recovered.is_none() && now.mode == TrafficMode::Kinematic {
            recovered = Some(t);
            lateral_at_recovery = now.lateral;
            if now.segment != Segment::Connector(0) || (now.s - 1.0).abs() >= 0.05 {
                failures.push(format!(
                    "recovered at {:?} s {:.3}, not where it stood",
                    now.segment, now.s
                ));
            }
        }
        if recovered.is_some() && rejoined.is_none() {
            widest = widest.max(band_reach(&app, car).0);
            if now.manoeuvre == Manoeuvre::None && now.lateral.abs() < 0.01 {
                rejoined = Some((t, now.segment));
            }
        }
        if now.segment == Segment::Lane(1) {
            on_lane = Some(t);
            break;
        }
    }
    eprintln!(
        "(f) in band: reach {reach:.3} (band {limit:.3}); kinematic after {recovered:?} s, lateral {lateral_at_recovery:.3}; \
         rejoined {rejoined:?}, widest reach while rejoining {widest:.3}; on lane 1 after {on_lane:?} s; G1 max depth {:.3}",
        oracle.max_depth()
    );
    oracle.assert_clean("connector recovery");
    let Some(back) = recovered.filter(|&t| t <= c.recover.seconds + 1.0) else {
        panic!(
            "not kinematic within {} s on the connector: {recovered:?}",
            c.recover.seconds + 1.0
        );
    };
    if widest > limit {
        failures.push(format!(
            "the body left the table band while rejoining: {widest:.3} > {limit:.3}"
        ));
    }
    let bound = lateral_at_recovery.abs() / c.lateral.rate_at_rest + 0.1;
    match rejoined {
        Some((t, Segment::Connector(0))) if t - back <= bound => {}
        other => failures.push(format!(
            "the rejoin did not end on the connector within {bound:.3} s: {other:?} (recovered at {back:.3} s)"
        )),
    }
    if on_lane.is_none_or(|t| t - back > 5.0) {
        failures.push(format!(
            "not on lane 1 within 5 s of the recovery: {on_lane:?}"
        ));
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// (f) Out of the table band: 0.25 m across, or on the line 0.5 m back from s 0.2 (0.3 m behind the
/// connector start, where it projects onto s 0 and would jump there): never kinematic on the
/// connector within `recover.seconds + 2 s`.
#[test]
fn f_connector_car_out_of_the_table_band_stays_dynamic() {
    f_out_of_band(1.0, Vec2::new(0.25, 0.0));
    f_out_of_band(0.2, Vec2::new(0.0, -0.5));
}

fn f_out_of_band(s: f32, shove: Vec2) {
    let (mut app, car) = connector_shove(s, shove);
    let c = cfg(&app);
    let (reach, limit) = band_reach(&app, car);
    let g = graph(&app);
    let at = position_of(&app, car);
    let (point, tangent) = g.pose(Segment::Connector(0), g.project(Segment::Connector(0), at));
    let along = (at - point).with_y(0.0).dot(tangent).abs();
    let margin = c.conflict_margin / 2.0;
    assert!(
        reach > limit + 0.05 || (reach < limit - 0.02 && along > margin + 0.05),
        "GATE BROKEN: the shoved car reaches {reach:.3} m across (band {limit:.3}), {along:.3} m along"
    );
    let mut oracle = Footprints::new(&app);
    let mut kinematic = None;
    for tick in 0..((c.recover.seconds + 2.0) * HZ as f32) as u32 {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        if kinematic.is_none() && mode(&app, car) == Some(TrafficMode::Kinematic) {
            kinematic = Some(tick as f32 / HZ as f32);
        }
    }
    eprintln!(
        "(f) out of band: reach {reach:.3} (band {limit:.3}), along {along:.3}, kinematic at {kinematic:?}"
    );
    oracle.assert_clean("connector, out of band");
    assert!(
        kinematic.is_none() && mode(&app, car) == Some(TrafficMode::Dynamic),
        "a car out of the table band recovered on the connector at {kinematic:?} s ({:?})",
        mode(&app, car)
    );
}

/// (g) On the two-way street, car T standing behind a held car is switched by hand; `body` (a parked
/// sleeping car or a standing dummy) stands 0.25 m right of its footprint. Returns (first kinematic
/// tick, re-switches after it) over `recover.seconds + 3 s`.
fn beside_at_rest(dummy: bool) -> (Option<f32>, u32) {
    let (lanes, connectors) = two_way_street(70.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let (h, c) = (half(&app), cfg(&app));
    let _lead = held_car(&mut app, 40.0);
    let car = spawn_traffic_car(
        &mut app,
        Segment::Lane(0),
        40.0 - 2.0 * h.z - c.idm.min_gap,
        0.0,
    );
    run_ticks(&mut app, 4);
    switch_by_hand(&mut app, car);
    let at = position_of(&app, car);
    // On its line: the rest skin is the switch skin alone.
    let rest = c.switch.skin;
    assert!(
        rest + 0.05 < 0.25 && 0.25 < c.recover.skin - 0.05,
        "GATE BROKEN: 0.25 m is not strictly between the rest skin {rest} and recover.skin {}",
        c.recover.skin
    );
    if dummy {
        let radius = app
            .world()
            .resource::<gta_sim::character::LocomotionConfig>()
            .capsule_radius;
        spawn_dummy(&mut app, Vec3::new(at.x, 0.05, 30.0 + h.x + 0.25 + radius));
    } else {
        let parked = park_car(
            &mut app,
            Vec3::new(at.x, 0.0, 30.0 + 2.0 * h.x + 0.25),
            Vec3::X,
        );
        app.world_mut().entity_mut(parked).insert(Sleeping);
    }
    let mut oracle = Footprints::new(&app);
    let (mut back, mut re_switches, mut last) = (None, 0, mode(&app, car));
    for tick in 0..((c.recover.seconds + 3.0) * HZ as f32) as u32 {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let now = mode(&app, car);
        if back.is_none() && now == Some(TrafficMode::Kinematic) {
            back = Some(tick as f32 / HZ as f32);
        }
        if now == Some(TrafficMode::Dynamic) && last == Some(TrafficMode::Kinematic) {
            re_switches += 1;
        }
        last = now;
    }
    eprintln!(
        "(g) {} 0.25 m beside (rest skin {rest:.2}, recover.skin {}): kinematic after {back:?} s, {re_switches} re-switches, final {:?}",
        if dummy { "dummy" } else { "parked car" },
        c.recover.skin,
        mode(&app, car)
    );
    oracle.assert_clean("beside at rest");
    (back, re_switches)
}

#[test]
fn g_car_at_rest_inside_the_skin_lets_it_recover() {
    let limit = {
        let app = floor();
        cfg(&app).recover.seconds + 1.0
    };
    let (back, re_switches) = beside_at_rest(false);
    assert!(
        back.is_some_and(|t| t <= limit) && re_switches == 0,
        "not kinematic within {limit} s beside a car at rest, or switched again: {back:?}, {re_switches}"
    );
}

#[test]
fn g_walker_at_rest_inside_the_skin_keeps_it_dynamic() {
    let (back, _) = beside_at_rest(true);
    assert!(
        back.is_none(),
        "recovered with a walker 0.25 m beside it after {back:?} s"
    );
}
