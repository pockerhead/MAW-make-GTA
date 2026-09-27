//! The junction box path check drives the requester's real body along its connector from where it
//! stands (TASK-036 item B), and a siren yield ends in the box (item C).
//!
//! B. `box_rules::connector_clear` tested the connector's centre line +- (half width + margin/2) with no
//! body length; a right-turn body pivots almost in place and its rear swung 0.87-0.91 m past that band,
//! into a kinematic car standing on a conflicting connector (a demoted holder) that the conflict table
//! does not know. TASK-036 stage 0 (`scratch/stage0/b_fixture.txt`): G1 0.533 m (+ floor, Y at 6 m/s),
//! 0.683 m (seed 1 node 69), and from rest a mutual wait (Y granted, stopped by its sensing at s 0.47
//! while X waited for Y's grant, >= 17.75 s).
//! - B1 (+ floor, Y at speed), B3 (seed 1 node 69, Y from rest): correctness, the G1 oracle
//!   (`traffic_support::Footprints`, unmargined, 0.02 m kinematic tolerance = the two 0.01 m shrinks of
//!   `RoadOccupancy::blocked`) clean, Y never granted while X stands at its fixture pose, both cars out of
//!   the box within `LEAVE_WITHIN`.
//! - B2 (+ floor, Y from rest): liveness (no mutual wait), same assertions.
//! - B4 (+ floor, regression guard, TASK-033 lease): a holder H on its straight connector at s 3.5, held
//!   by a walker dummy, its lane follower F standing the jam gap behind its rear (F's nose within half a
//!   car length of the connector start, so F lies in a sweep from s 0) and a waiter W for a conflicting
//!   connector, queued for it for as long as H holds its grant (else the lease is never contested and
//!   the flip cannot go RED): H keeps its grant for `reservation_timeout + 0.5` s (only a walker holds it).
//! - B5 (+ floor, regression guard): the queue head of the opposite lane at its stop line is not a body on
//!   a left turner's path: the turner (queued first) is granted.
//! - Queue order (B1-B3): Y's waiting stamp is pre-set to 0 before the first shared tick; X is stamped
//!   with the current tick (>= 1). Who is granted first is the rule under test, never the precondition.
//!
//! C. A siren yield that reaches its full offset at or past the lane end took the car into its
//! connector, where the yield stop halted it with its grant for good (`sirens::update` ended a yield only
//! on a lane). TASK-036 stage 0 (`scratch/stage0/c_fixture.txt`): + floor s 0.75, seed 1 connector 1091
//! s 0.35. Ending the yield in the box let a car at the avenue curb offset (3.25 m) drive on into a
//! co-granted car (App sweep, G1 up to 0.48 m), so the curb lane is taken only outside the yield's reach
//! of the stop line (`sirens::yield_reach`). Every C row starts the yield through `sirens::update` with a
//! real siren car (a responding police car at rest behind, removed once the yield starts) and names the
//! offset it chose.
//! - C1 (+ floor, the slack yield begun 1 m before the stop line), C2 (seed 1 curb lane 437, within the
//!   reach: the curb refused, the slack chosen): liveness, the car is on its connector in its yield
//!   for a tick (precondition), then its rear leaves the connector within `LEAVE_WITHIN` and it holds
//!   no grant of that connector afterwards; G1 clean.
//! - C3 (seed 1 curb lane 437, 1 m outside the reach, granted, parked cars on its run removed): the curb
//!   taken, the car stands at its offset before the stop line; G1 clean.
//!
//! Flips (stage summaries of TASK-036): B1-B3: the old capless band in `connector_clear` (B1/B3 RED on
//! G1, B2 on liveness); B4: holders' `from_s` = 0 (H demoted at the lease); B5: the body grown by
//! `conflict_margin / 2` in `drive.rs` (Y refused for Q); C1/C2: `sirens::update` returns `None` off a
//! lane again (both stand in the box); C2: no reach check (the curb taken 0.37 m before the stop line);
//! C3: the reach check always refuses (the slack taken).

mod common;
mod police_support;
mod traffic_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    traffic::{Manoeuvre, Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections},
    vehicle::{Vehicle, VehicleConfig},
};
use police_support::spawn_police_car;
use traffic_support::*;

const HZ: u32 = 64;
/// The cars' rears leave their connectors within this, s: the first green runs left by 7.9-8.8 s (B:
/// Y waits for room behind X on the shared exit lane, then starts from rest) and 2.5-5.8 s (C); about
/// twice the longest.
const LEAVE_WITHIN: f32 = 16.0;

/// The + intersection of `traffic_intersection.rs` (centre (-22, 22), arms 12 m): approaches 0..3
/// (from north, east, south, west), exits 4..7 (to north, east, south, west).
fn plus_floor() -> App {
    let centre = Vec3::new(-22.0, 0.0, 22.0);
    let dirs = [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X];
    let mut lanes = Vec::new();
    for &d in &dirs {
        let side = right_of(-d) * 1.625;
        lanes.push((centre + d * 15.25 + side, centre + d * 3.25 + side, 6.0, 0));
    }
    for (k, &d) in dirs.iter().enumerate() {
        let side = right_of(d) * 1.625;
        lanes.push((
            centre + d * 3.25 + side,
            centre + d * 15.25 + side,
            6.0,
            10 + k as u32,
        ));
    }
    let mut connectors = Vec::new();
    for i in 0..4u32 {
        for j in 0..4u32 {
            if i != j {
                connectors.push((i, 4 + j, 0));
            }
        }
    }
    for j in 0..4u32 {
        connectors.push((4 + j, j, 10 + j));
    }
    traffic_floor(lanes, &connectors, &[])
}

fn connector(g: &TrafficGraph, from: u32, to: u32) -> u32 {
    (0..g.connectors().len() as u32)
        .find(|&c| g.connector(c).from_lane == from && g.connector(c).to_lane == to)
        .unwrap_or_else(|| panic!("GATE BROKEN: no connector {from} -> {to}"))
}

/// The junction tick (`Time<Fixed>` elapsed / timestep), as `traffic::drive` computes it.
fn tick_now(app: &App) -> u64 {
    let time = app.world().resource::<Time<Fixed>>();
    (time.elapsed().as_nanos() / time.timestep().as_nanos().max(1)) as u64
}

fn granted(app: &App, node: u32, car: Entity) -> Option<u32> {
    app.world()
        .resource::<TrafficIntersections>()
        .0
        .get(&node)
        .and_then(|j| j.occupants.iter().find(|o| o.1 == car).map(|o| o.0))
}

fn waiter_stamp(app: &App, node: u32, car: Entity) -> Option<u64> {
    app.world()
        .resource::<TrafficIntersections>()
        .0
        .get(&node)
        .and_then(|j| j.waiters.iter().find(|w| w.1 == car).map(|w| w.0))
}

/// Turn of connector `c`, degrees (+ right).
fn turn_deg(g: &TrafficGraph, c: u32) -> f32 {
    let k = g.connector(c);
    let a = g.lane(k.from_lane).dir;
    let b = g.lane(k.to_lane).dir;
    let (a, b) = (Vec2::new(a.x, a.z), Vec2::new(b.x, b.z));
    a.perp_dot(b).atan2(a.dot(b)).to_degrees()
}

/// Tracks when a car's rear first leaves connector `c` (onto its exit lane).
struct Leave {
    car: Entity,
    c: u32,
    at: Option<f32>,
}

impl Leave {
    fn record(&mut self, app: &App, g: &TrafficGraph, half: f32, t: f32) {
        if self.at.is_some() {
            return;
        }
        let Some(car) = app.world().get::<TrafficCar>(self.car) else {
            return;
        };
        if car.segment == Segment::Lane(g.connector(self.c).to_lane) && car.s - half >= 0.0 {
            self.at = Some(t);
        }
    }
}

/// B1-B3: X standing kinematic on `c1` at `s1` with no grant (a demoted holder), Y on `c2`'s source
/// lane (at 6 m/s 8 m before its stop line, or from rest at it), `c2` conflicting with `c1`.
fn standoff(mut app: App, (c1, s1): (u32, f32), c2: u32, y_speed: f32, label: &str) -> Vec<String> {
    let g = graph(&app);
    let h = app.world().resource::<VehicleConfig>().half_extents();
    let (k1, k2) = (g.connector(c1).clone(), g.connector(c2).clone());
    assert!(
        k2.conflicts.contains(&c1),
        "GATE BROKEN: {label}: connector {c2} does not conflict with {c1}"
    );
    let lane = g.lane(k2.from_lane).clone();
    let y_s = if y_speed > 0.0 {
        (lane.stop - h.z - 8.0).max(2.5)
    } else {
        lane.stop - h.z - 0.3
    };
    let x = spawn_traffic_car(&mut app, Segment::Connector(c1), s1, 0.0);
    set_car(&mut app, x, |t| t.next = Some(c1));
    let y = spawn_traffic_car(&mut app, Segment::Lane(k2.from_lane), y_s, y_speed);
    set_car(&mut app, y, |t| {
        t.next = Some(c2);
        t.waiting = Some(0);
    });
    assert!(
        tick_now(&app) >= 1 && traffic_car(&app, x).waiting.is_none(),
        "GATE BROKEN: {label}: X would not be stamped after Y (tick {})",
        tick_now(&app)
    );
    let node = k1.node;
    let mut oracle = Footprints::new(&app);
    let mut leaves = [
        Leave {
            car: x,
            c: c1,
            at: None,
        },
        Leave {
            car: y,
            c: c2,
            at: None,
        },
    ];
    let (mut ordered, mut early) = (false, Vec::new());
    for tick in 0..20 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let t = tick as f32 / HZ as f32;
        if !ordered {
            let y_granted = granted(&app, node, y).is_some();
            let y_stamp = waiter_stamp(&app, node, y);
            if y_granted || y_stamp.is_some() {
                assert!(
                    y_granted || y_stamp == Some(0),
                    "GATE BROKEN: {label}: Y queued with stamp {y_stamp:?}, not 0"
                );
                ordered = true;
            }
        }
        // Past the box the bubble may despawn a car (the city rows keep no traffic).
        let x_at_pose = app
            .world()
            .get::<TrafficCar>(x)
            .is_some_and(|t| t.segment == Segment::Connector(c1) && t.s <= s1 + 0.05);
        if x_at_pose && granted(&app, node, y).is_some() {
            early.push(t);
        }
        for leave in &mut leaves {
            leave.record(&app, &g, h.z, t);
        }
    }
    assert!(ordered, "GATE BROKEN: {label}: Y never queued");
    eprintln!(
        "{label}: X left the box at {:?} s, Y at {:?} s; Y granted while X stood at its pose from {:?} s; \
         G1 max depth {:.3} m",
        leaves[0].at,
        leaves[1].at,
        early.first(),
        oracle.max_depth()
    );
    let mut failures = Vec::new();
    if !oracle.violations.is_empty() {
        failures.push(format!("{label}: G1 {:?}", oracle.summary()));
    }
    if let Some(t) = early.first() {
        failures.push(format!(
            "{label}: Y was granted at {t:.2} s while X stood on its path"
        ));
    }
    for (name, leave) in ["X", "Y"].iter().zip(&leaves) {
        if leave.at.is_none_or(|t| t > LEAVE_WITHIN) {
            failures.push(format!(
                "{label}: {name} left the box at {:?} s (bound {LEAVE_WITHIN} s)",
                leave.at
            ));
        }
    }
    failures
}

fn plus_standoff(y_speed: f32, label: &str) -> Vec<String> {
    let app = plus_floor();
    let g = graph(&app);
    // X on the straight east -> west, Y's right turn north -> west (the same exit lane).
    let (c1, c2) = (connector(&g, 1, 7), connector(&g, 0, 7));
    assert!(
        turn_deg(&g, c1).abs() < 1.0 && turn_deg(&g, c2) > 45.0,
        "GATE BROKEN: {label}: not a straight and a right turn"
    );
    standoff(app, (c1, 1.5), c2, y_speed, label)
}

#[test]
fn b1_right_turn_at_speed_meets_no_standing_car() {
    let failures = plus_standoff(6.0, "B1");
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn b2_right_turn_from_rest_no_mutual_wait() {
    let failures = plus_standoff(0.0, "B2");
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

/// The seed-N city with no traffic but the fixture's, the player watching node `node` from 25 m.
fn quiet(app: &mut App, node: u32) {
    set_traffic(app, |t| t.bubble.max_cars = 0);
    let g = graph(app);
    let points: Vec<Vec3> = g
        .connectors()
        .iter()
        .filter(|k| k.node == node)
        .flat_map(|k| k.points.clone())
        .collect();
    let hub = points.iter().copied().sum::<Vec3>() / points.len().max(1) as f32;
    let feet = sidewalk_at(app, hub, 25.0);
    stand_player(app, Vec2::new(feet.x, feet.z), hub - feet);
    for e in cars(app) {
        app.world_mut()
            .resource_mut::<TrafficIntersections>()
            .release(e);
        app.world_mut().despawn(e);
    }
    run_ticks(app, 2);
}

#[test]
fn b3_seed_1_node_69_right_turn_from_rest() {
    let (c1, c2) = (590, 587);
    let mut app = city_app(1);
    quiet(&mut app, 69);
    let g = graph(&app);
    assert!(
        g.connector(c1).node == 69 && turn_deg(&g, c2) > 45.0,
        "GATE BROKEN: B3: seed 1 connector {c2} is not a right turn at node 69"
    );
    let failures = standoff(app, (c1, 1.47), c2, 0.0, "B3");
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

/// B4: a holder waiting for a walker on its connector keeps its lease over its own lane follower.
#[test]
fn b4_holder_keeps_its_lease_over_its_follower() {
    let mut app = plus_floor();
    let g = graph(&app);
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let h = app.world().resource::<VehicleConfig>().half_extents();
    let c1 = connector(&g, 0, 6);
    let c5 = connector(&g, 1, 7);
    let window = cfg.reservation_timeout + 0.5;
    assert!(
        window < cfg.pass.character_seconds && g.connector(c1).conflicts.contains(&c5),
        "GATE BROKEN: B4: window {window} s vs character_seconds {}, or c{c5} does not contest c{c1}",
        cfg.pass.character_seconds
    );
    let s_h = 3.5;
    let holder = spawn_traffic_car(&mut app, Segment::Connector(c1), s_h, 0.0);
    set_car(&mut app, holder, |t| t.next = Some(c1));
    app.world_mut()
        .resource_mut::<TrafficIntersections>()
        .0
        .entry(0)
        .or_default()
        .occupants
        .push((c1, holder));
    let (nose, tangent) = g.pose(Segment::Connector(c1), s_h + h.z + 0.6);
    let dummy = spawn_dummy(&mut app, nose);
    let lane0 = g.lane(0).clone();
    // F's nose the jam gap behind H's rear (the connector is straight).
    let f_nose = s_h - h.z - cfg.idm.min_gap;
    assert!(
        (-h.z..0.0).contains(&f_nose),
        "GATE BROKEN: B4: F's nose {f_nose:.2} m is not within half a car of the connector start"
    );
    let follower = spawn_traffic_car(&mut app, Segment::Lane(0), lane0.length + f_nose - h.z, 0.0);
    set_car(&mut app, follower, |t| t.next = Some(c1));
    let lane1 = g.lane(1).clone();
    let w = spawn_traffic_car(&mut app, Segment::Lane(1), lane1.stop - h.z, 0.0);
    set_car(&mut app, w, |t| t.next = Some(c5));
    let exit = g.lane(g.connector(c1).to_lane).length;
    assert!(
        2.0 * (2.0 * h.z + cfg.idm.min_gap) > exit,
        "GATE BROKEN: B4: exit lane {exit} m has room for F behind H"
    );
    let mut oracle = Footprints::new(&app);
    let (mut lost, mut f_granted, mut moved) = (None, false, 0.0_f32);
    let mut uncontested = None;
    let start = position_of(&app, holder);
    for tick in 0..(window * HZ as f32).ceil() as u32 {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        if lost.is_none() && granted(&app, 0, holder).is_none() {
            lost = Some(tick as f32 / HZ as f32);
        }
        // W contests H's lease (a waiter for c5) for as long as H holds it.
        let w_waits = app.world().resource::<TrafficIntersections>().0[&0]
            .waiters
            .iter()
            .any(|x| x.1 == w && x.2 == c5);
        if lost.is_none() && !w_waits {
            uncontested.get_or_insert(tick as f32 / HZ as f32);
        }
        f_granted |= granted(&app, 0, follower).is_some();
        moved = moved.max((position_of(&app, holder) - start).with_y(0.0).length());
    }
    assert!(
        !f_granted && moved < 0.3,
        "GATE BROKEN: B4: F granted ({f_granted}) or H did not stand (moved {moved:.2} m)"
    );
    assert!(
        uncontested.is_none(),
        "GATE BROKEN: B4: W was not a waiter for c{c5} while H held its grant (from {uncontested:?} s)"
    );
    let dummy_at = position_of(&app, dummy);
    assert!(
        (dummy_at - nose).with_y(0.0).dot(tangent) > -0.5,
        "GATE BROKEN: B4: the dummy left H's nose"
    );
    eprintln!(
        "B4: H lost its grant at {lost:?} s over a {window} s window; H moved {moved:.2} m; G1 max depth {:.3}",
        oracle.max_depth()
    );
    oracle.assert_clean("B4");
    assert!(
        lost.is_none(),
        "B4: the holder waiting for a walker lost its grant at {lost:?} s (its follower is not on its path)"
    );
}

/// B5: the opposite queue head at its stop line does not block a left turner's grant.
#[test]
fn b5_opposite_queue_head_is_not_on_the_path() {
    let mut app = plus_floor();
    let g = graph(&app);
    let h = app.world().resource::<VehicleConfig>().half_extents();
    let (c0, c6) = (connector(&g, 0, 5), connector(&g, 2, 4));
    assert!(
        turn_deg(&g, c0) < -45.0 && g.connector(c0).conflicts.contains(&c6),
        "GATE BROKEN: B5: c{c0} is not a left turn conflicting with c{c6}"
    );
    let y = spawn_traffic_car(&mut app, Segment::Lane(0), g.lane(0).stop - h.z, 0.0);
    set_car(&mut app, y, |t| {
        t.next = Some(c0);
        t.waiting = Some(0);
    });
    let q = spawn_traffic_car(&mut app, Segment::Lane(2), g.lane(2).stop - h.z, 0.0);
    set_car(&mut app, q, |t| t.next = Some(c6));
    assert!(
        granted(&app, 0, q).is_none() && tick_now(&app) >= 1,
        "GATE BROKEN: B5: Q granted before the first tick"
    );
    run_ticks(&mut app, 1);
    assert!(
        waiter_stamp(&app, 0, q).is_some() || granted(&app, 0, q).is_some(),
        "GATE BROKEN: B5: Q did not queue in the first tick"
    );
    assert!(
        granted(&app, 0, y).is_some(),
        "B5: the left turner (queued first) was not granted: the opposite queue head at its stop line \
         counted as a body on its path (Q granted: {:?})",
        granted(&app, 0, q)
    );
}

/// A responding police car (sirens on) `behind` m behind `car` on its lane, at rest: runs ticks until
/// `sirens::update` starts `car`'s yield, then takes the siren car away (the yield is never passed and
/// runs to the box or its timeout). Returns the chosen offset.
fn siren_start(app: &mut App, car: Entity, behind: f32, label: &str) -> f32 {
    let g = graph(app);
    let t = traffic_car(app, car);
    let (at, dir) = g.pose(t.segment, t.s - behind);
    let police = spawn_police_car(app, at, dir, 0.0, vec![]);
    let mut offset = None;
    for _ in 0..4 {
        run_ticks(app, 1);
        if let Manoeuvre::Yield { offset: o, .. } = traffic_car(app, car).manoeuvre {
            offset = Some(o);
            break;
        }
    }
    app.world_mut().despawn(police);
    offset.unwrap_or_else(|| {
        panic!("GATE BROKEN: {label}: no yield started for a siren car {behind:.2} m behind")
    })
}

/// C: `car` drives into connector `c` in its yield; its rear leaves `c` within `LEAVE_WITHIN` and it
/// then holds no grant of `c`.
fn yield_into_box(mut app: App, car: Entity, c: u32, label: &str) {
    let g = graph(&app);
    let h = app.world().resource::<VehicleConfig>().half_extents();
    let node = g.connector(c).node;
    let mut oracle = Footprints::new(&app);
    let mut leave = Leave { car, c, at: None };
    let (mut yielding_in_box, mut held_after, mut end) = (false, None, None);
    for tick in 0..20 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let t = tick as f32 / HZ as f32;
        // Past the box the bubble may despawn the car (the city row keeps no traffic).
        let now = app.world().get::<TrafficCar>(car).copied();
        yielding_in_box |= now.is_some_and(|n| {
            n.segment == Segment::Connector(c) && matches!(n.manoeuvre, Manoeuvre::Yield { .. })
        });
        end = now.map(|n| (n.segment, n.s));
        leave.record(&app, &g, h.z, t);
        if leave.at.is_some_and(|l| t > l) && granted(&app, node, car) == Some(c) {
            held_after.get_or_insert(t);
        }
    }
    eprintln!(
        "{label}: rear left connector {c} at {:?} s; end (segment, s) {:?}; held its grant after leaving from {held_after:?}; \
         G1 max depth {:.3}",
        leave.at,
        end,
        oracle.max_depth()
    );
    assert!(
        yielding_in_box,
        "GATE BROKEN: {label}: the car was never on connector {c} in its yield"
    );
    oracle.assert_clean(label);
    assert!(
        leave.at.is_some_and(|t| t <= LEAVE_WITHIN) && held_after.is_none(),
        "{label}: the yielding car stayed in the box (left at {:?} s, bound {LEAVE_WITHIN} s; end (segment, s) {:?}) \
         or held its grant after leaving ({held_after:?})",
        leave.at,
        end
    );
}

/// A kinematic car on `lane` at `speed` with its centre `before_end` m before the lane end, bound for
/// `c`, granted (asserted after one tick).
fn granted_approach(
    app: &mut App,
    lane: u32,
    c: u32,
    (before_end, speed): (f32, f32),
    label: &str,
) -> Entity {
    let g = graph(app);
    let length = g.lane(lane).length;
    let a = spawn_traffic_car(app, Segment::Lane(lane), length - before_end, speed);
    set_car(app, a, |t| t.next = Some(c));
    run_ticks(app, 1);
    assert!(
        granted(app, g.connector(c).node, a) == Some(c),
        "GATE BROKEN: {label}: A was not granted before its yield"
    );
    a
}

#[test]
fn c1_slack_yield_at_the_stop_line_drives_out_of_the_box() {
    let mut app = plus_floor();
    let g = graph(&app);
    let h = app.world().resource::<VehicleConfig>().half_extents();
    let c1 = connector(&g, 0, 6);
    let lane = g.lane(0).clone();
    // The nose 1 m before the stop line (+ floor: at the lane end).
    let before_end = lane.length - lane.stop + h.z + 1.2;
    let a = granted_approach(&mut app, 0, c1, (before_end, 6.0), "C1");
    let offset = siren_start(&mut app, a, 2.0 * h.z + 1.0, "C1");
    let slack = lane
        .left_gap
        .expect("GATE BROKEN: C1: lane 0 has no left gap")
        / 2.0
        - h.x;
    assert!(
        (offset - slack).abs() < 1e-4,
        "GATE BROKEN: C1: the yield took offset {offset}, not the slack {slack}"
    );
    yield_into_box(app, a, c1, "C1");
}

/// Seed 1 curb lane 437 into connector 1091 (the TASK-036 stage 0 curb lock), the box quiet.
fn curb_scene() -> (App, u32, u32) {
    let (lane_id, c) = (437, 1091);
    let mut app = city_app(1);
    let g = graph(&app);
    let lane = g.lane(lane_id).clone();
    assert!(
        g.connector(c).from_lane == lane_id
            && lane.curb_lane
            && lane.left_gap.is_some_and(|p| (p - 3.25).abs() < 0.01),
        "GATE BROKEN: seed 1 lane {lane_id} is not a curb lane with left gap 3.25 into connector {c}"
    );
    quiet(&mut app, g.connector(c).node);
    (app, lane_id, c)
}

/// The curb yield's reach from `v` (the shift at the yield's top speed, then its stop from
/// `pass.speed`), as `sirens::yield_reach` derives it.
fn curb_reach(app: &App, pitch: f32, v: f32) -> f32 {
    let cfg = app.world().resource::<TrafficConfig>();
    let top = v.max(cfg.pass.speed);
    pitch * top / cfg.lateral.rate(top)
        + cfg.pass.speed * cfg.pass.speed / (2.0 * cfg.idm.comfortable_deceleration)
}

/// C2: within the curb yield's reach of the stop line the curb lane is refused (the slack instead: a
/// car entering the box a lane over met a co-granted car, C-G1 0.479 m), and the slack yield that
/// reaches the box ends there.
#[test]
fn c2_seed_1_curb_lane_near_the_box_yields_to_the_slack_and_drives_out() {
    let (mut app, lane_id, c) = curb_scene();
    let g = graph(&app);
    let lane = g.lane(lane_id).clone();
    let h = app.world().resource::<VehicleConfig>().half_extents();
    let a = granted_approach(&mut app, lane_id, c, (2.5, 6.0), "C2");
    let t = traffic_car(&app, a);
    let room = lane.stop - (t.s + h.z);
    let pitch = lane.left_gap.unwrap_or(3.25);
    assert!(
        room < curb_reach(&app, pitch, t.speed),
        "GATE BROKEN: C2: room {room:.2} m is outside the curb reach"
    );
    let offset = siren_start(&mut app, a, 2.0 * h.z + 1.0, "C2");
    let slack = pitch / 2.0 - h.x;
    assert!(
        (offset - slack).abs() < 1e-4,
        "C2: {room:.2} m before the stop line the curb yield took offset {offset} (the slack is {slack})"
    );
    yield_into_box(app, a, c, "C2");
}

/// C3: outside the reach the curb lane is taken, and the granted car stands at its curb offset before
/// the stop line.
#[test]
fn c3_seed_1_curb_yield_outside_its_reach_stands_before_the_stop_line() {
    let (mut app, lane_id, c) = curb_scene();
    let g = graph(&app);
    let lane = g.lane(lane_id).clone();
    let h = app.world().resource::<VehicleConfig>().half_extents();
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let pitch = lane.left_gap.unwrap_or(3.25);
    let room = curb_reach(&app, pitch, 6.0) + 1.0;
    let s = lane.stop - h.z - room - 0.1;
    let behind = 2.0 * h.z + 1.0;
    assert!(
        s - behind - h.z >= 0.0 && s - h.z >= 2.0 * h.z + cfg.idm.min_gap,
        "GATE BROKEN: C3: lane {lane_id} ({:.1} m to its stop line) is too short for a start at s {s:.2}",
        lane.stop
    );
    // The player past the box (a body beside the curb lane would stop the car first).
    let node = g.connector(c).node;
    let hub = g.connector(c).points[0];
    let feet = sidewalk_at(&app, hub + lane.dir * 25.0, 0.0);
    stand_player(&mut app, Vec2::new(feet.x, feet.z), hub - feet);
    // Named mutation: no parked car on the curb lane beside the run (the curb yield would stop for it).
    let run = |p: Vec3| {
        let d = (p - lane.from).with_y(0.0);
        let along = d.dot(lane.dir);
        (s - behind..=lane.stop).contains(&along) && d.dot(right_of(lane.dir)).abs() <= 2.0 * pitch
    };
    let parked: Vec<Entity> = app
        .world_mut()
        .query_filtered::<(Entity, &Transform), (With<Vehicle>, Without<TrafficCar>)>()
        .iter(app.world())
        .filter(|(_, t)| run(t.translation))
        .map(|(e, _)| e)
        .collect();
    for e in parked {
        app.world_mut().despawn(e);
    }
    let a = spawn_traffic_car(&mut app, Segment::Lane(lane_id), s, 6.0);
    set_car(&mut app, a, |t| t.next = Some(c));
    app.world_mut()
        .resource_mut::<TrafficIntersections>()
        .0
        .entry(node)
        .or_default()
        .occupants
        .push((c, a));
    let offset = siren_start(&mut app, a, behind, "C3");
    assert!(
        (offset - pitch).abs() < 1e-4,
        "C3: {room:.2} m before the stop line the curb yield took offset {offset}, not the curb {pitch}"
    );
    let mut oracle = Footprints::new(&app);
    let (mut stood, mut worst_nose) = (None, f32::NEG_INFINITY);
    for tick in 0..12 * HZ {
        run_ticks(&mut app, 1);
        oracle.record(&mut app, tick);
        let t = traffic_car(&app, a);
        if !matches!(t.manoeuvre, Manoeuvre::Yield { .. }) {
            break;
        }
        assert!(
            granted(&app, node, a) == Some(c),
            "GATE BROKEN: C3: A lost its grant in its yield"
        );
        worst_nose = worst_nose.max(match t.segment {
            Segment::Lane(l) if l == lane_id => t.s + h.z - lane.stop,
            _ => f32::INFINITY,
        });
        let shifted = (t.lateral - pitch).abs() < 0.05;
        assert!(
            t.speed > 0.0 || shifted,
            "GATE BROKEN: C3: A stood at lateral {:.2} before its shift (not the yield stop)",
            t.lateral
        );
        if t.speed == 0.0 {
            stood.get_or_insert(tick as f32 / HZ as f32);
        }
    }
    eprintln!(
        "C3: room {room:.2} m; stood at the curb offset at {stood:?} s; nose past the stop line by at most {worst_nose:.2} m; \
         G1 max depth {:.3}",
        oracle.max_depth()
    );
    oracle.assert_clean("C3");
    assert!(
        stood.is_some() && worst_nose <= 0.0,
        "C3: the curb yield did not stand before the stop line (stood {stood:?}, nose past it by {worst_nose:.2} m)"
    );
}
