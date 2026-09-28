//! Traffic progress (TASK-039), floor rows: the checker and the mechanism of the relaxed pass
//! (`traffic::progress`); the city rows are in `traffic_progress.rs`.
//!
//! - The third-body oracle (correctness of the checker): a kinematic car driven through a standing
//!   character or into a wall is a violation; the pair a relaxation names is exempt, no other; a
//!   relaxation that starts against a moving body or inside it, or outlives its bound, is a violation.
//! - Plumbing (mechanism, relaxation set by hand): a relaxed car drives through a standing player
//!   unhurt and unlifted, the exemption lasts until the pair separates, the contact hook keeps a
//!   parked car still, the wheel rays of an awake parked car skip the passer.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    traffic::{Relax, Segment, TrafficMode},
    vehicle::VehicleConfig,
};
use traffic_support::{progress::*, *};

const HZ: u32 = PROGRESS_HZ;

// ---------------------------------------------------------------------------------------------
// The third-body oracle.

enum OracleCase {
    Plain,
    RelaxedAgainstDummy,
    RelaxedAgainstOther,
    Wall,
}

/// A traffic car set `Taken` with a stand-in driver (named mutation: it stays kinematic, the switch
/// ignores it and nobody abandons it) driven by
/// hand at 2 m/s through a dummy held on its line (named mutation: the dummy is put back each tick),
/// or into the test-area wall; the relaxation re-set by hand each tick.
fn oracle_run(case: OracleCase) -> Footprints {
    let (lanes, connectors) = two_way_street(60.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 10.0, 0.0);
    set_car(&mut app, car, |c| c.mode = TrafficMode::Taken);
    let driver = app
        .world_mut()
        .spawn(gta_sim::vehicle::Driving { vehicle: car })
        .id();
    app.world_mut()
        .get_mut::<gta_sim::vehicle::Vehicle>(car)
        .unwrap()
        .driver = Some(driver);
    let feet = Vec3::new(-10.0, 0.0, 30.0);
    let dummy = spawn_dummy(&mut app, feet);
    let other = spawn_dummy(&mut app, Vec3::new(-10.0, 0.0, 40.0));
    run_ticks(&mut app, 2);
    let float = app.world().resource::<LocomotionConfig>().float_height;
    let rest = app.world().resource::<VehicleConfig>().rest_height();
    let (start, dir) = match case {
        OracleCase::Wall => (Vec3::new(0.0, rest, 22.0), Vec3::NEG_Z),
        _ => (Vec3::new(-20.0, rest, 30.0), Vec3::X),
    };
    let rotation = Quat::from_rotation_y(gta_sim::combat::aim_yaw(dir));
    app.world_mut().get_mut::<Rotation>(car).unwrap().0 = rotation;
    app.world_mut().get_mut::<Transform>(car).unwrap().rotation = rotation;
    let mut oracle = Footprints::new(&app).with_third_bodies();
    for tick in 0..6 * HZ {
        let at = start + dir * 2.0 * tick as f32 / HZ as f32;
        teleport(&mut app, car, at);
        teleport(&mut app, dummy, feet + Vec3::Y * float);
        let blocker = match case {
            OracleCase::RelaxedAgainstDummy => Some(dummy),
            OracleCase::RelaxedAgainstOther => Some(other),
            _ => None,
        };
        set_car(&mut app, car, |c| {
            c.relaxed = blocker.map(|blocker| Relax {
                blocker,
                since: 0,
                physics_only: false,
                trailing: None,
            })
        });
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
    }
    oracle
}

#[test]
fn oracle_sees_a_car_through_a_dummy() {
    let plain = oracle_run(OracleCase::Plain);
    let relaxed = oracle_run(OracleCase::RelaxedAgainstDummy);
    let other = oracle_run(OracleCase::RelaxedAgainstOther);
    let wall = oracle_run(OracleCase::Wall);
    eprintln!(
        "oracle: plain {:?}; relaxed depth {:.2} m, {:?}; relaxed against another {:?}; wall {:?}",
        plain.third_summary(),
        relaxed.relaxed_max_depth(),
        relaxed.third_summary(),
        other.third_summary(),
        wall.third_summary()
    );
    let mut failures = Vec::new();
    if plain.third.violations.is_empty() {
        failures.push("a car through a dummy is no violation");
    }
    if !relaxed.third.violations.is_empty() || relaxed.relaxed_max_depth() <= 0.0 {
        failures.push("the relaxed pair is not exempt (or never overlapped)");
    }
    if other.third.violations.is_empty() {
        failures.push("a relaxation naming another body exempts the dummy");
    }
    if wall.third.violations.is_empty() {
        failures.push("a car in the wall is no violation");
    }
    assert!(failures.is_empty(), "{failures:?}");
}

// ---------------------------------------------------------------------------------------------
// Plumbing: the relaxation set by hand (named mutation: the detector is not the subject).

/// The loop floor (its lanes have no neighbour: no lane pass, no box pass, so a relaxed car drives
/// through its blocker) and a kinematic traffic car on lane 0 (z 37, x -34..34, clear of every
/// test-area block) at s 10 and `speed`.
fn loop_floor(speed: f32) -> (App, Entity) {
    let v0 = {
        let app = headless_app();
        app.world()
            .resource::<gta_sim::traffic::TrafficConfig>()
            .desired_speed
            .street
    };
    let (lanes, connectors) = loop_lanes(v0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 10.0, speed);
    (app, car)
}

/// Lane 0 of the loop floor at `s` (flat, y 0).
fn on_lane0(s: f32) -> Vec3 {
    Vec3::new(-34.0 + s, 0.0, 37.0)
}

/// A kinematic car at 5 m/s relaxed against the player standing on its line 12 m ahead drives
/// through him at most at the pass speed: no hit, no damage, no stagger, not lifted, stays kinematic, and
/// the relaxation ends once it is past.
#[test]
fn relaxed_car_drives_through_a_standing_player() {
    let (mut app, car) = loop_floor(5.0);
    let at = on_lane0(22.0);
    // No camera view: the bubble never despawns the car once it is past him.
    let float = app.world().resource::<LocomotionConfig>().float_height;
    place_player(&mut app, at + Vec3::Y * float);
    run_ticks(&mut app, 2);
    let player = player(&mut app);
    let health = health_of(&app, player);
    relax(&mut app, car, player);
    let mut unhurt = Unhurt::new(&mut app, player);
    let mut contacts = Contacts::new(&app);
    let (mut touched, mut switched, mut past_at, mut ended_at) = (false, false, None, None);
    let mut fastest: f32 = 0.0;
    for tick in 0..10 * HZ {
        run_ticks(&mut app, 1);
        unhurt.check(&app, tick);
        touched |= contacts.touched(&app, car, player);
        switched |= !kinematic(&app, car);
        let x = position_of(&app, car).x;
        let t = traffic_car(&app, car);
        if (x - at.x).abs() < 3.0 && t.relaxed.is_some_and(|r| !r.physics_only) {
            fastest = fastest.max(t.speed);
        }
        if past_at.is_none() && x > at.x {
            past_at = Some(tick);
        }
        if ended_at.is_none() && past_at.is_some() && traffic_car(&app, car).relaxed.is_none() {
            ended_at = Some(tick);
        }
    }
    let mut failures = unhurt.failures("the player");
    let now = health_of(&app, player);
    if now.current != health.current {
        failures.push(format!("health {health:?} -> {now:?}"));
    }
    let squeeze = app
        .world()
        .resource::<gta_sim::traffic::TrafficConfig>()
        .pass
        .speed;
    for (bad, what) in [
        (touched, "a CollisionStart of the pair"),
        (switched, "the car left kinematic"),
        (past_at.is_none(), "the car never passed the player"),
        (ended_at.is_none(), "the relaxation never ended"),
        (
            fastest > squeeze + 1e-3,
            "faster than the pass speed while relaxed",
        ),
    ] {
        if bad {
            failures.push(what.into());
        }
    }
    eprintln!(
        "through the player: past at {past_at:?}, relaxation ended at {ended_at:?}, speed by him {fastest:.2} m/s, height change {:.3} m",
        unhurt.lift
    );
    assert!(failures.is_empty(), "{failures:?}");
}

/// As above with a dummy: when the car's centre is within 0.3 m of it along the lane, the dummy is put
/// 0.3 m aside ("the body moved", named mutation). Planning ends at once (physics only: sensing sees
/// the dummy again, behind the nose), and the pair keeps its exemption while the car drives out of it
/// (no contact, no damage, no switch, the dummy not pushed); the relaxation ends only once the two
/// footprints are apart.
#[test]
fn relaxed_pair_ends_only_when_separated() {
    let (mut app, car) = loop_floor(5.0);
    let feet = on_lane0(22.0);
    let dummy = spawn_dummy(&mut app, feet);
    run_ticks(&mut app, 2);
    relax(&mut app, car, dummy);
    let mut unhurt = Unhurt::new(&mut app, dummy);
    let mut contacts = Contacts::new(&app);
    let loco = app.world().resource::<LocomotionConfig>().clone();
    let mut moved_at = None;
    let mut expected = position_of(&app, dummy);
    let (mut drift, mut touched, mut switched) = (0.0_f32, false, false);
    let (mut planning_after_move, mut ended_overlapped, mut ended_at) = (false, false, None);
    let mut overlapped_after_move = 0;
    for tick in 0..8 * HZ {
        let x = position_of(&app, car).x;
        if moved_at.is_none() && (x - feet.x).abs() < 0.3 {
            expected = feet + Vec3::new(0.0, loco.float_height, 0.3);
            teleport(&mut app, dummy, expected);
            moved_at = Some(tick);
        }
        run_ticks(&mut app, 1);
        unhurt.check(&app, tick);
        let now = position_of(&app, dummy);
        drift = drift.max((now - expected).with_y(0.0).length());
        touched |= contacts.touched(&app, car, dummy);
        switched |= !kinematic(&app, car);
        let overlap = gta_sim::traffic::swept_circle_hits_rect(
            Vec2::new(now.x, now.z),
            loco.capsule_radius,
            Vec2::ZERO,
            &footprint(&app, car),
        );
        let relaxed = traffic_car(&app, car).relaxed;
        if moved_at.is_some_and(|t| tick >= t + 2) {
            planning_after_move |= relaxed.is_some_and(|r| !r.physics_only);
            overlapped_after_move += u32::from(overlap);
            if relaxed.is_none() && ended_at.is_none() {
                ended_at = Some(tick);
                ended_overlapped = overlap;
            }
        }
    }
    let mut failures = unhurt.failures("the dummy");
    for (bad, what) in [
        (
            moved_at.is_none(),
            "the car never reached the dummy (GATE BROKEN)",
        ),
        (
            overlapped_after_move == 0,
            "the pair never overlapped after the move (GATE BROKEN)",
        ),
        (planning_after_move, "still planning after the dummy moved"),
        (touched, "a CollisionStart of the pair"),
        (switched, "the car left kinematic"),
        (drift > 0.05, "the dummy was pushed"),
        (ended_at.is_none(), "the relaxation outlived the separation"),
        (
            ended_overlapped,
            "the relaxation ended while the pair overlapped",
        ),
    ] {
        if bad {
            failures.push(format!("{what} (drift {drift:.3} m)"));
        }
    }
    eprintln!(
        "separation: moved at {moved_at:?}, overlapped {overlapped_after_move} ticks after, ended at {ended_at:?}, dummy drift {drift:.3} m"
    );
    assert!(failures.is_empty(), "{failures:?}");
}

// A kinematic car relaxed against a parked car on its line 15 m ahead drives through it: no
/// `CollisionStart` of the pair, the parked car moves < 0.02 m and tilts < 1 deg, the car stays
/// kinematic, the relaxation ends after. `awake`: the parked car kept awake (named mutation: as when it
/// touches a `Dynamic` traffic car), so its wheel rays run; else put to sleep.
fn through_a_parked_car(awake: bool) {
    let (mut app, car) = loop_floor(5.0);
    let parked = park_car(&mut app, on_lane0(25.0), Vec3::X);
    if awake {
        app.world_mut().entity_mut(parked).insert(SleepingDisabled);
    } else {
        app.world_mut().entity_mut(parked).insert(Sleeping);
    }
    run_ticks(&mut app, 2);
    assert!(
        awake || app.world().get::<Sleeping>(parked).is_some(),
        "GATE BROKEN: the parked car is not asleep"
    );
    let start = position_of(&app, parked);
    relax(&mut app, car, parked);
    let mut contacts = Contacts::new(&app);
    let (mut drift, mut tilt, mut touched, mut switched) = (0.0_f32, 0.0_f32, false, false);
    let mut overlapped = 0;
    for _ in 0..12 * HZ {
        run_ticks(&mut app, 1);
        touched |= contacts.touched(&app, car, parked);
        switched |= !kinematic(&app, car);
        drift = drift.max((position_of(&app, parked) - start).length());
        let up = app.world().get::<Rotation>(parked).unwrap().0 * Vec3::Y;
        tilt = tilt.max(up.angle_between(Vec3::Y).to_degrees());
        overlapped += u32::from(obb_overlap(&footprint(&app, car), &footprint(&app, parked)));
    }
    let relaxed = traffic_car(&app, car).relaxed;
    eprintln!(
        "parked (awake {awake}): overlapped {overlapped} ticks, drift {drift:.4} m, tilt {tilt:.2} deg, touched {touched}, switched {switched}, relaxed now {relaxed:?}"
    );
    let mut failures = Vec::new();
    for (bad, what) in [
        (
            overlapped == 0,
            "the car never drove through the parked car (GATE BROKEN)",
        ),
        (touched, "a CollisionStart of the pair"),
        (switched, "the car left kinematic"),
        (drift >= 0.02, "the parked car moved"),
        (tilt >= 1.0, "the parked car tilted"),
        (relaxed.is_some(), "the relaxation never ended"),
    ] {
        if bad {
            failures.push(what);
        }
    }
    assert!(failures.is_empty(), "awake {awake}: {failures:?}");
}

#[test]
fn relaxed_pair_has_no_contact() {
    through_a_parked_car(false);
}

#[test]
fn relaxed_car_passes_an_awake_parked_car() {
    through_a_parked_car(true);
}

// ---------------------------------------------------------------------------------------------
// The oracle's own checks of a relaxation (independent of the rule that set it).

enum RelaxCase {
    /// Against a traffic car driving by on the other lane.
    Moving,
    /// Against a dummy inside the car.
    Inside,
    /// Against a standing dummy ahead, held 2 s with the limit shrunk to 1 s (named mutation).
    Long,
    /// Against a standing dummy ahead, held 0.5 s: no violation.
    Control,
}

/// A traffic car set `Taken` with a stand-in driver (it stays kinematic and nobody else sets its
/// relaxation), standing at (-20, 30) heading +x on the two-way street; the relaxation set by hand
/// after each tick while held (where the detector sets one: the oracle reads it before the next
/// upkeep). Returns the oracle's relaxation violations.
fn relax_case(case: RelaxCase) -> Vec<String> {
    let (lanes, connectors) = two_way_street(60.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let car = spawn_traffic_car(&mut app, Segment::Lane(0), 10.0, 0.0);
    set_car(&mut app, car, |c| c.mode = TrafficMode::Taken);
    let driver = app
        .world_mut()
        .spawn(gta_sim::vehicle::Driving { vehicle: car })
        .id();
    app.world_mut()
        .get_mut::<gta_sim::vehicle::Vehicle>(car)
        .unwrap()
        .driver = Some(driver);
    let rest = app.world().resource::<VehicleConfig>().rest_height();
    let rotation = Quat::from_rotation_y(gta_sim::combat::aim_yaw(Vec3::X));
    app.world_mut().get_mut::<Rotation>(car).unwrap().0 = rotation;
    app.world_mut().get_mut::<Transform>(car).unwrap().rotation = rotation;
    teleport(&mut app, car, Vec3::new(-20.0, rest, 30.0));
    let blocker = match case {
        RelaxCase::Moving => spawn_traffic_car(&mut app, Segment::Lane(1), 20.0, 8.0),
        RelaxCase::Inside => spawn_dummy(&mut app, Vec3::new(-20.0, 0.0, 30.0)),
        RelaxCase::Long | RelaxCase::Control => spawn_dummy(&mut app, Vec3::new(-14.0, 0.0, 30.0)),
    };
    run_ticks(&mut app, HZ);
    let mut oracle = Footprints::new(&app);
    if matches!(case, RelaxCase::Long) {
        oracle.third.relax_limit = HZ;
    }
    let held = match case {
        RelaxCase::Long => 2 * HZ,
        RelaxCase::Control => HZ / 2,
        _ => 4,
    };
    for tick in 0..held + HZ {
        run_ticks(&mut app, 1);
        if tick < held {
            set_car(&mut app, car, |c| {
                c.relaxed = Some(Relax {
                    blocker,
                    since: 0,
                    physics_only: false,
                    trailing: None,
                })
            });
        }
        oracle.record(&mut app, tick);
    }
    oracle.third.relax_violations
}

#[test]
fn oracle_checks_the_start_and_end_of_a_relaxation() {
    let moving = relax_case(RelaxCase::Moving);
    let inside = relax_case(RelaxCase::Inside);
    let long = relax_case(RelaxCase::Long);
    let control = relax_case(RelaxCase::Control);
    eprintln!(
        "relaxation checks: moving {moving:?}; inside {inside:?}; long {long:?}; control {control:?}"
    );
    let mut failures = Vec::new();
    for (bad, what) in [
        (
            moving.is_empty(),
            "a relaxation against a moving car is no violation",
        ),
        (
            inside.is_empty(),
            "a relaxation started inside its body is no violation",
        ),
        (
            long.is_empty(),
            "a relaxation held past its limit is no violation",
        ),
        (
            !control.is_empty(),
            "a short relaxation against a standing body apart is a violation",
        ),
    ] {
        if bad {
            failures.push(what);
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

// ---------------------------------------------------------------------------------------------
// C1: a `Dynamic` car pinned between a body behind and one ahead; a squeeze left standing in the way
// of its blocker.

/// Local copy of `traffic_recovery.rs` `switch_by_hand` (named mutation: the switch is not the subject).
fn switch_by_hand(app: &mut App, car: Entity) {
    let at = position_of(app, car);
    let forward = app.world().get::<Rotation>(car).unwrap().0 * Vec3::NEG_Z;
    app.world_mut().entity_mut(car).insert((
        RigidBody::Dynamic,
        SleepingDisabled,
        gta_sim::vehicle::Autopilot {
            target: at + forward * 4.0,
            ..default()
        },
        gta_sim::vehicle::DriveIntent::default(),
    ));
    set_car(app, car, |c| c.mode = TrafficMode::Dynamic);
}

/// A `Dynamic` car at rest at s 10 of the loop's lane 0 between a parked car 0.05 m behind its rear
/// bumper and, ahead of its nose, a parked car 0.05 m away (`person` false) or a dummy 0.35 m away:
/// both inside its recovery skins, so it cannot recover on its own. `behind_first`: the body behind
/// is spawned first (it comes first in the road occupancy). Returns the failures: it must squeeze
/// through the body ahead (never relaxed against the one behind) and have its rear past it within
/// 30 s, standing in `Dynamic` no longer than that, the body behind unmoved, the oracle clean, a dummy
/// unhurt.
fn pinned_dynamic_car(person: bool, behind_first: bool) -> Vec<String> {
    let (mut app, car) = loop_floor(0.0);
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let hold = app.world().resource::<VehicleConfig>().hold_speed;
    let radius = app.world().resource::<LocomotionConfig>().capsule_radius;
    let spawn_behind = |app: &mut App| park_car(app, on_lane0(10.0 - 2.0 * half.z - 0.05), Vec3::X);
    let spawn_ahead = |app: &mut App| {
        if person {
            spawn_dummy(app, on_lane0(10.0 + half.z + 0.35 + radius))
        } else {
            park_car(app, on_lane0(10.0 + 2.0 * half.z + 0.05), Vec3::X)
        }
    };
    let (behind, ahead) = if behind_first {
        let b = spawn_behind(&mut app);
        (b, spawn_ahead(&mut app))
    } else {
        let a = spawn_ahead(&mut app);
        (spawn_behind(&mut app), a)
    };
    run_ticks(&mut app, 2);
    switch_by_hand(&mut app, car);
    let start = position_of(&app, behind);
    let far = position_of(&app, ahead).x + if person { radius } else { half.z };
    let mut unhurt = person.then(|| Unhurt::new(&mut app, ahead));
    let mut oracle = Footprints::new(&app).with_third_bodies();
    let (mut moved_at, mut past_at, mut drift) = (None, None, 0.0_f32);
    let (mut stand, mut worst_stand) = (0u32, 0u32);
    let mut against: Vec<Entity> = Vec::new();
    let label = format!("person {person}, behind first {behind_first}");
    for tick in 0..40 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        if let Some(u) = unhurt.as_mut() {
            u.check(&app, tick);
        }
        drift = drift.max((position_of(&app, behind) - start).length());
        let t = traffic_car(&app, car);
        if let Some(r) = t.relaxed
            && !against.contains(&r.blocker)
        {
            against.push(r.blocker);
        }
        let speed = app
            .world()
            .get::<LinearVelocity>(car)
            .map_or(0.0, |v| v.0.length());
        stand = if t.mode == TrafficMode::Dynamic && speed < hold {
            stand + 1
        } else {
            0
        };
        worst_stand = worst_stand.max(stand);
        if moved_at.is_none() && speed >= hold {
            moved_at = Some(tick);
        }
        if past_at.is_none() && position_of(&app, car).x - half.z > far {
            past_at = Some(tick);
        }
    }
    let mut failures: Vec<String> = unhurt.map(|u| u.failures(&label)).unwrap_or_default();
    let s = |t: Option<u32>| t.map(|t| t as f32 / HZ as f32);
    let worst = worst_stand as f32 / HZ as f32;
    eprintln!(
        "pinned ({label}): moved at {:?} s, rear past the body ahead at {:?} s, worst Dynamic stand \
         {worst:.1} s, now {:?}, relaxed against {against:?} (ahead {ahead}, behind {behind}), behind \
         drift {drift:.3} m",
        s(moved_at),
        s(past_at),
        traffic_car(&app, car).mode
    );
    if s(moved_at).is_some_and(|t| t < 5.0) {
        failures.push(format!(
            "{label}: GATE BROKEN: moved at {:?} s, not pinned",
            s(moved_at)
        ));
    }
    if s(past_at).is_none_or(|t| t > 30.0) || worst > 30.0 {
        failures.push(format!(
            "{label}: past the body ahead at {:?} s, Dynamic stand {worst:.1} s (bound 30)",
            s(past_at)
        ));
    }
    if against.contains(&behind) {
        failures.push(format!("{label}: relaxed against the body behind"));
    }
    if drift > 0.05 {
        failures.push(format!("{label}: the body behind moved {drift:.3} m"));
    }
    let bad = [
        (!oracle.violations.is_empty()).then(|| format!("G1 {:?}", oracle.summary())),
        (!oracle.third.violations.is_empty())
            .then(|| format!("third bodies {:?}", oracle.third_summary())),
        (!oracle.third.relax_violations.is_empty())
            .then(|| format!("relaxations {:?}", oracle.third.relax_violations)),
    ];
    failures.extend(bad.into_iter().flatten().map(|b| format!("{label}: {b}")));
    failures
}

#[test]
fn dynamic_car_pinned_between_two_parked_cars() {
    let failures = [true, false]
        .map(|first| pinned_dynamic_car(false, first))
        .concat();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn dynamic_car_pinned_behind_a_dummy() {
    let failures = [true, false]
        .map(|first| pinned_dynamic_car(true, first))
        .concat();
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A squeeze left standing in the way of its blocker plans again: a kinematic car at rest at s 10 with
/// a dummy standing 0.2 m inside its nose, relaxed against it by hand in the physics-only phase (named
/// mutation: as when the squeeze went stale, or its blocker moved and stopped again on the nose). It
/// senses the dummy and stands; once the dummy has stood `wait_seconds` and the car the grace, the
/// detector plans the squeeze again: the car is past the dummy and the relaxation over within 30 s,
/// with no contact of the pair, the dummy unhurt.
#[test]
fn squeeze_plans_again_when_left_in_the_way() {
    let (mut app, car) = loop_floor(0.0);
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let radius = app.world().resource::<LocomotionConfig>().capsule_radius;
    let feet = on_lane0(10.0 + half.z + radius - 0.2);
    let dummy = spawn_dummy(&mut app, feet);
    let since = fixed_tick(&app);
    set_car(&mut app, car, |c| {
        c.relaxed = Some(Relax {
            blocker: dummy,
            since,
            physics_only: true,
            trailing: None,
        })
    });
    let mut unhurt = Unhurt::new(&mut app, dummy);
    let mut contacts = Contacts::new(&app);
    let (mut touched, mut replanned, mut past_at, mut ended_at) = (false, None, None, None);
    for tick in 0..40 * HZ {
        run_ticks(&mut app, 1);
        unhurt.check(&app, tick);
        touched |= contacts.touched(&app, car, dummy);
        let t = traffic_car(&app, car);
        if replanned.is_none() && t.relaxed.is_some_and(|r| !r.physics_only) {
            replanned = Some(tick);
        }
        let rear = position_of(&app, car).x - half.z;
        if past_at.is_none() && rear > position_of(&app, dummy).x + radius {
            past_at = Some(tick);
        }
        if ended_at.is_none() && past_at.is_some() && t.relaxed.is_none() {
            ended_at = Some(tick);
        }
    }
    let s = |t: Option<u32>| t.map(|t| t as f32 / HZ as f32);
    eprintln!(
        "left in the way: planned again at {:?} s, past at {:?} s, ended at {:?} s",
        s(replanned),
        s(past_at),
        s(ended_at)
    );
    let mut failures = unhurt.failures("the dummy");
    for (bad, what) in [
        (touched, "a CollisionStart of the pair"),
        (replanned.is_none(), "the squeeze never planned again"),
        (
            s(past_at).is_none_or(|t| t > 30.0),
            "not past the dummy within 30 s",
        ),
        (
            s(ended_at).is_none_or(|t| t > 30.0),
            "the relaxation not over within 30 s",
        ),
    ] {
        if bad {
            failures.push(what.into());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

/// M2 (review): a person keeps his own clock. A kinematic car at rest at s 10 of the loop's lane 0
/// stands 12 s behind a dummy 2 m ahead; then that dummy steps away (named mutation: put 7 m aside)
/// and a second one takes its place. The car has waited far longer than `wait_seconds` by then, yet it
/// squeezes past the second dummy only once that one has stood `wait_seconds` itself (a person pausing
/// on the crosswalk is not squeezed after the grace); and it does squeeze then.
#[test]
fn a_person_keeps_his_own_clock() {
    let (mut app, car) = loop_floor(0.0);
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let loco = app.world().resource::<LocomotionConfig>().clone();
    let wait = app
        .world()
        .resource::<gta_sim::traffic::TrafficConfig>()
        .progress
        .wait_seconds;
    let feet = on_lane0(10.0 + half.z + 2.0 + loco.capsule_radius);
    let first = spawn_dummy(&mut app, feet);
    let swap = 12 * HZ;
    let mut second = None;
    let mut relaxed_at = None;
    for tick in 0..40 * HZ {
        if tick == swap {
            teleport(
                &mut app,
                first,
                feet + Vec3::new(0.0, loco.float_height, -7.0),
            );
            second = Some(spawn_dummy(&mut app, feet));
        }
        run_ticks(&mut app, 1);
        let t = traffic_car(&app, car);
        assert!(
            tick >= swap || t.relaxed.is_none(),
            "GATE BROKEN: the car squeezed past the first dummy at {} s",
            tick as f32 / HZ as f32
        );
        if tick < swap {
            assert!(t.speed == 0.0, "GATE BROKEN: the car moved before the swap");
        }
        if relaxed_at.is_none() && second.is_some() && t.relaxed.map(|r| r.blocker) == second {
            relaxed_at = Some(tick);
        }
    }
    let after = relaxed_at.map(|t| (t - swap) as f32 / HZ as f32);
    eprintln!(
        "own clock: squeezed past the second dummy {after:?} s after it stepped in (wait {wait} s)"
    );
    assert!(
        after.is_some_and(|s| s >= wait - 0.5 && s <= wait + 2.0),
        "squeezed past the second dummy {after:?} s after it stepped in (wait {wait} s)"
    );
}
