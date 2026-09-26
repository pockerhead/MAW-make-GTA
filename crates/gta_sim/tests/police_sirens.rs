//! G5 (TASK-032): traffic yields to a siren car. Production city (seeds 1 and 7), the longest street
//! lane with 75 m to its stop line and a straight lane after it; six AI cars at rest (jam gap apart,
//! the head 10 m before the stop line), a police car 12 m behind the last one, the player on foot on
//! the exit lane's sidewalk looking back, past the head by the dismount distance + the police stopping
//! distance + one car length (the crew gets out past the queue).
//! Named mutations: no traffic bubble (`max_cars = 0`: the yield mechanism, not oncoming flow) and no
//! dispatched police cars.
//!
//! - Responding (sirens on): every queued car yields (shifts over 0.3 m toward its curb and stops),
//!   the police car passes all six without being held up (`blocked` under `blocked_seconds`), and each
//!   yielded car is kinematic on its lane line and moving within `sirens.timeout_seconds` after the
//!   police car's rear passed its nose.
//! - Leaving (sirens off): nobody yields.
//! - The G1 oracle stays clean in every row, and no AI car stands longer than 30 s in `Dynamic`.

mod common;
mod police_support;
mod traffic_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    police::{EscalationConfig, PoliceCar, PoliceCarState, UnitKind},
    traffic::{Manoeuvre, Segment, TrafficCar, TrafficConfig, TrafficMode},
    vehicle::VehicleConfig,
    wanted::WantedLevel,
};
use police_support::*;
use traffic_support::*;
use wanted_support::*;

const HZ: u32 = 64;
const SECONDS: u32 = 30;
const QUEUE: usize = 6;

/// A street lane with 75 m to its stop line and a straight lane of 30 m or more after it: (lane,
/// the straight lane after it).
fn street_with_straight_exit(app: &App) -> (u32, u32) {
    let street = app.world().resource::<TrafficConfig>().desired_speed.street;
    let g = graph(app);
    g.lanes()
        .iter()
        .enumerate()
        .filter(|(_, l)| (l.v0 - street).abs() < 1e-3 && l.stop >= 75.0 && l.left_gap.is_some())
        .filter_map(|(k, l)| {
            let exit = l.out.iter().map(|&c| g.connector(c).to_lane).find(|&t| {
                let next = g.lane(t);
                next.dir.dot(l.dir) > 0.99 && next.length >= 30.0
            })?;
            Some((k as u32, exit, l.stop))
        })
        .max_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(k, exit, _)| (k, exit))
        .expect("GATE BROKEN: no street lane of 75 m with a straight exit")
}

struct Scene {
    app: App,
    lane: u32,
    queue: Vec<Entity>,
    police: Entity,
}

fn scene(seed: u64, state: PoliceCarState) -> Scene {
    let mut app = city_app(seed);
    set_traffic(&mut app, |t| t.bubble.max_cars = 0);
    no_police_cars(&mut app);
    let (lane, exit) = street_with_straight_exit(&app);
    let (l, e) = (
        graph(&app).lane(lane).clone(),
        graph(&app).lane(exit).clone(),
    );
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let gap = app.world().resource::<TrafficConfig>().idm.min_gap;
    let spacing = 2.0 * half.z + gap;
    // The queue head 10 m before the stop line, the police car 12 m behind the last car (a leaving car
    // 8 m: it follows the traffic bubble rule, so it starts inside the 90 m in-frame radius).
    let head = l.stop - 10.0 - half.z;
    let behind = if state == PoliceCarState::Leave {
        8.0
    } else {
        12.0
    };
    let police_s = head - spacing * (QUEUE - 1) as f32 - behind;
    // The player past the head's nose by the dismount distance, the police stopping distance at
    // pursuit speed and one car length, on the exit lane's sidewalk looking back at the queue.
    let car = app.world().resource::<EscalationConfig>().car.clone();
    let brake = app.world().resource::<TrafficConfig>().idm.max_deceleration;
    let beyond = car.dismount_distance
        + car.pursuit_speed * car.pursuit_speed / (2.0 * brake)
        + 2.0 * half.z;
    let nose = l.from + l.dir * (head + half.z);
    let x = (beyond - (e.from - nose).with_y(0.0).dot(l.dir)).max(5.0);
    assert!(
        x <= e.length,
        "GATE BROKEN: the exit lane is {} m, the player needs {x} m",
        e.length
    );
    let player = e.from + e.dir * x + right_of(e.dir) * 3.2;
    stand_player(&mut app, Vec2::new(player.x, player.z), -e.dir);
    set_player_armor(&mut app, 1.0e6);
    let heat = wanted_cfg(&app).stars[1].heat;
    raise_heat(&mut app, heat);
    let traffic: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<TrafficCar>>()
        .iter(app.world())
        .collect();
    for car in traffic {
        app.world_mut()
            .resource_mut::<gta_sim::traffic::TrafficIntersections>()
            .release(car);
        app.world_mut().despawn(car);
    }
    let queue: Vec<Entity> = (0..QUEUE)
        .map(|k| {
            let s = head - spacing * (QUEUE - 1 - k) as f32;
            spawn_traffic_car(&mut app, Segment::Lane(lane), s, 0.0)
        })
        .collect();
    let (at, dir) = graph(&app).pose(Segment::Lane(lane), police_s);
    let police = spawn_police_car(&mut app, at, dir, 0.0, vec![UnitKind::Patrol; 2]);
    app.world_mut().get_mut::<PoliceCar>(police).unwrap().state = state;
    assert_eq!(l.v0, e.v0, "GATE BROKEN: the exit is not a street");
    Scene {
        app,
        lane,
        queue,
        police,
    }
}

fn hold_stars(app: &mut App) {
    let mut w = app.world_mut().resource_mut::<WantedLevel>();
    assert!(w.stars >= 1, "GATE BROKEN: the wanted level dropped to 0");
    w.hidden = 0.0;
}

#[derive(Default, Clone, Copy, Debug)]
struct Track {
    yielded: bool,
    /// Largest curb shift while yielding, m.
    shift: f32,
    /// Stood still while yielding.
    stopped: bool,
    /// Tick the police car's rear passed the car's nose.
    passed: Option<u32>,
    /// Tick it was back on its lane line (kinematic, moving) after that.
    resumed: Option<u32>,
}

struct Outcome {
    tracks: Vec<Track>,
    worst_blocked: f32,
    oracle: Footprints,
    clock: StandClock,
}

fn watch(scene: &mut Scene) -> Outcome {
    let app = &mut scene.app;
    let l = graph(app).lane(scene.lane).clone();
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let along = |p: Vec3| (p - l.from).with_y(0.0).dot(l.dir);
    let mut tracks = vec![Track::default(); QUEUE];
    let mut worst_blocked = 0.0f32;
    let mut oracle = Footprints::new(app);
    let mut clock = StandClock::default();
    for tick in 0..SECONDS * HZ {
        run_ticks(app, 1);
        hold_stars(app);
        oracle.record(app, tick);
        clock.record(app);
        let police = app.world().get::<PoliceCar>(scene.police).cloned();
        let tail = along(position_of(app, scene.police)) - half.z;
        let all_passed = tracks.iter().all(|t| t.passed.is_some());
        if let Some(p) = &police
            && !all_passed
        {
            worst_blocked = worst_blocked.max(p.blocked);
        }
        for (k, &car) in scene.queue.iter().enumerate() {
            let Some(c) = app.world().get::<TrafficCar>(car).copied() else {
                continue;
            };
            let t = &mut tracks[k];
            if matches!(c.manoeuvre, Manoeuvre::Yield { .. }) {
                t.yielded = true;
                t.shift = t.shift.max(c.lateral);
                t.stopped |= c.speed == 0.0;
            }
            let nose = along(position_of(app, car)) + half.z;
            if t.passed.is_none() && tail > nose {
                t.passed = Some(tick);
            }
            let back = c.mode == TrafficMode::Kinematic
                && c.lateral.abs() < 0.05
                && c.manoeuvre == Manoeuvre::None
                && c.speed > 0.0;
            if t.passed.is_some() && t.resumed.is_none() && back {
                t.resumed = Some(tick);
            }
        }
    }
    Outcome {
        tracks,
        worst_blocked,
        oracle,
        clock,
    }
}

fn responding(seed: u64) {
    let mut scene = scene(seed, PoliceCarState::Respond);
    let out = watch(&mut scene);
    let cfg = scene.app.world().resource::<TrafficConfig>().clone();
    let blocked = scene
        .app
        .world()
        .resource::<EscalationConfig>()
        .car
        .blocked_seconds;
    let timeout = (cfg.sirens.timeout_seconds * HZ as f32) as u32;
    eprintln!(
        "seed {seed}: worst blocked {:.2} s, G1 max depth {:.3}",
        out.worst_blocked,
        out.oracle.max_depth()
    );
    let mut violations = Vec::new();
    for (k, t) in out.tracks.iter().enumerate() {
        let resume = t.passed.zip(t.resumed).map(|(p, r)| r - p);
        eprintln!(
            "  car {k}: yielded {}, shift {:.3} m, stopped {}, passed at {:?}, resumed after {:?} ticks",
            t.yielded, t.shift, t.stopped, t.passed, resume
        );
        if !(t.yielded && t.shift > 0.3 && t.stopped) {
            violations.push(format!("car {k} did not yield to the curb and stop: {t:?}"));
        }
        if t.passed.is_none() {
            violations.push(format!("the police car never passed car {k}"));
        }
        if resume.is_none_or(|r| r > timeout) {
            violations.push(format!(
                "car {k} not back on its lane within {} s of the pass: {t:?}",
                cfg.sirens.timeout_seconds
            ));
        }
    }
    if out.worst_blocked >= blocked {
        violations.push(format!(
            "the police car was held up {:.2} s before passing the queue",
            out.worst_blocked
        ));
    }
    violations.extend(out.clock.dynamic_violation());
    assert!(violations.is_empty(), "seed {seed}: {violations:#?}");
    out.oracle.assert_clean(&format!("sirens seed {seed}"));
}

fn leaving(seed: u64) {
    let mut scene = scene(seed, PoliceCarState::Leave);
    let out = watch(&mut scene);
    let yielded: Vec<usize> = (0..QUEUE).filter(|&k| out.tracks[k].yielded).collect();
    eprintln!("seed {seed} (sirens off): yielded {yielded:?}");
    assert!(
        yielded.is_empty(),
        "seed {seed}: cars {yielded:?} yielded to a car with its sirens off"
    );
    if let Some(v) = out.clock.dynamic_violation() {
        panic!("seed {seed} (sirens off): {v}");
    }
    out.oracle.assert_clean(&format!("sirens off seed {seed}"));
}

#[test]
fn seed_1_queue_yields_to_sirens() {
    responding(1);
}

#[test]
fn seed_7_queue_yields_to_sirens() {
    responding(7);
}

#[test]
fn seed_1_nobody_yields_with_sirens_off() {
    leaving(1);
}

#[test]
fn seed_7_nobody_yields_with_sirens_off() {
    leaving(7);
}
