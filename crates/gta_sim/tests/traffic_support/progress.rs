//! Shared helpers of the progress gates (`traffic_progress*.rs`, TASK-039): city-row watching and
//! reporting, the hurt check of a relaxed person, `CollisionStart` of a pair, hand-set relaxations.

use super::*;
use crate::common::*;
use avian3d::prelude::*;
use bevy::prelude::*;
use gta_sim::{
    combat::{DamageDealt, HitReaction},
    traffic::{Relax, Segment, TrafficCar, TrafficGraph, TrafficIntersections},
    vehicle::VehicleHit,
};
use std::collections::HashMap;

pub const PROGRESS_HZ: u32 = 64;
const HZ: u32 = PROGRESS_HZ;
/// The stand bound of every city row, s.
pub const STAND_S: f32 = 30.0;
/// Stands are asserted within this radius of the scene, m.
pub const SCENE_M: f32 = 45.0;

/// Per second, every AI car standing over 5 s within 45 m of `at` (env `TRAFFIC_TRACE=1`).
pub fn trace(app: &mut App, at: Vec3, tick: u32, clock: &StandClock) {
    if !tick.is_multiple_of(HZ) || std::env::var_os("TRAFFIC_TRACE").is_none() {
        return;
    }
    let junctions = app.world().resource::<TrafficIntersections>();
    let grants: HashMap<Entity, u32> = junctions
        .0
        .values()
        .flat_map(|j| j.occupants.iter().map(|o| (o.1, o.0)))
        .collect();
    let rows: Vec<String> = app
        .world_mut()
        .query::<(Entity, &TrafficCar, &Position, &LinearVelocity)>()
        .iter(app.world())
        .filter(|(e, c, p, _)| {
            c.is_ai() && (p.0 - at).with_y(0.0).length() < SCENE_M && clock.current(*e) > 5.0
        })
        .map(|(e, c, p, v)| {
            format!(
                "  {e} {:?} s {:.2} {:?} {:?} lat {:.2} next {:?} wait {:?} grant {:?} relaxed {:?} v {:.2} stood {:.1} at ({:.1}, {:.1})",
                c.segment,
                c.s,
                c.mode,
                c.manoeuvre,
                c.lateral,
                c.next,
                c.waiting,
                grants.get(&e),
                c.relaxed,
                v.0.length(),
                clock.current(e),
                p.0.x,
                p.0.z
            )
        })
        .collect();
    eprintln!("t {:.0} s:\n{}", tick as f32 / HZ as f32, rows.join("\n"));
}

/// What a city row saw.
pub struct Seen {
    pub clock: StandClock,
    pub oracle: Footprints,
    /// First relaxation of each car: (car, blocker, tick).
    pub relaxations: Vec<(Entity, Entity, u32)>,
    pub max_unexplained: u32,
    pub max_stalled_relaxed: u32,
}

/// Runs `seconds` with `each` before every tick; stands, G1 with third bodies, relaxations and the
/// trace around `at`.
pub fn watch(app: &mut App, at: Vec3, seconds: u32, mut each: impl FnMut(&mut App, u32)) -> Seen {
    let mut clock = StandClock::default();
    let mut oracle = Footprints::new(app).with_third_bodies();
    let mut relaxations: Vec<(Entity, Entity, u32)> = Vec::new();
    let (mut max_unexplained, mut max_stalled_relaxed) = (0, 0);
    for tick in 0..seconds * HZ {
        each(app, tick);
        run_ticks(app, 1);
        clock.record(app);
        oracle.record(app, tick);
        for (car, blocker) in relaxed_pairs(app) {
            if !relaxations.iter().any(|r| r.0 == car && r.1 == blocker) {
                relaxations.push((car, blocker, tick));
            }
        }
        let stats = stats(app);
        max_unexplained = max_unexplained.max(stats.unexplained);
        max_stalled_relaxed = max_stalled_relaxed.max(stats.stalled_relaxed);
        trace(app, at, tick, &clock);
    }
    Seen {
        clock,
        oracle,
        relaxations,
        max_unexplained,
        max_stalled_relaxed,
    }
}

/// Stands over the bound within `SCENE_M` of `at`, the `Dynamic` bound, G1 and the third bodies.
pub fn scene_failures(seen: &Seen, at: Vec3) -> Vec<String> {
    let mut failures: Vec<String> = seen.clock.dynamic_violation().into_iter().collect();
    let stood: Vec<(f32, Vec3)> = seen
        .clock
        .longer_than(STAND_S)
        .into_iter()
        .filter(|(_, _, p)| (*p - at).with_y(0.0).length() <= SCENE_M)
        .map(|(_, s, p)| (s, p))
        .collect();
    if !stood.is_empty() {
        failures.push(format!(
            "AI cars stood > {STAND_S} s near the scene: {stood:?}"
        ));
    }
    if !seen.oracle.violations.is_empty() {
        failures.push(format!("G1: {:?}", seen.oracle.summary()));
    }
    if !seen.oracle.third.violations.is_empty() {
        failures.push(format!("third bodies: {:?}", seen.oracle.third_summary()));
    }
    if !seen.oracle.third.relax_violations.is_empty() {
        failures.push(format!(
            "relaxations: {:?}",
            seen.oracle.third.relax_violations
        ));
    }
    failures
}

pub fn report(label: &str, app: &App, seen: &Seen) {
    let stats = stats(app);
    eprintln!(
        "{label}: worst stand {:?}, worst dynamic {:.1} s, relaxed max depth {:.2} m, progress {:?}, \
         recoveries {}, max unexplained {}, max stalled relaxed {}, relaxations (car, blocker, s) {:?}",
        seen.clock.worst(),
        seen.clock.worst_dynamic(),
        seen.oracle.relaxed_max_depth(),
        stats.progress_relaxations,
        stats.progress_recoveries,
        seen.max_unexplained,
        seen.max_stalled_relaxed,
        seen.relaxations
            .iter()
            .map(|r| (r.0, r.1, r.2 as f32 / HZ as f32))
            .collect::<Vec<_>>()
    );
}

pub fn lane_of(g: &TrafficGraph, p: Vec3) -> u32 {
    match g.nearest(p) {
        Some((Segment::Lane(l), _, d)) if d < 1.5 => l,
        other => panic!("GATE BROKEN: no lane at {p}: {other:?}"),
    }
}

/// Where the lane `lane` passes x = `x`, `lateral` m right of its line.
pub fn lane_point(g: &TrafficGraph, lane: u32, x: f32, lateral: f32) -> Vec3 {
    let l = g.lane(lane);
    let s = (x - l.from.x) / l.dir.x;
    l.from + l.dir * s + right_of(l.dir) * lateral
}

/// Messages and reactions that hurt `who` (hits, damage, a stagger or knock-down), and its height change.
pub struct Unhurt {
    who: Entity,
    y: f32,
    hits: bevy::ecs::message::MessageCursor<VehicleHit>,
    damage: bevy::ecs::message::MessageCursor<DamageDealt>,
    hurt: Vec<String>,
    pub lift: f32,
}

impl Unhurt {
    pub fn new(app: &mut App, who: Entity) -> Self {
        let world = app.world();
        Self {
            who,
            y: world.get::<Position>(who).unwrap().0.y,
            hits: world
                .resource::<Messages<VehicleHit>>()
                .get_cursor_current(),
            damage: world
                .resource::<Messages<DamageDealt>>()
                .get_cursor_current(),
            hurt: Vec::new(),
            lift: 0.0,
        }
    }

    pub fn check(&mut self, app: &App, tick: u32) {
        let world = app.world();
        let who = self.who;
        let hit = self
            .hits
            .read(world.resource::<Messages<VehicleHit>>())
            .any(|h| h.target == who);
        let dealt = self
            .damage
            .read(world.resource::<Messages<DamageDealt>>())
            .any(|d| d.target == who);
        let reaction = world.get::<HitReaction>(who).copied();
        if (hit || dealt || reaction.is_some_and(|r| r.is_active())) && self.hurt.len() < 5 {
            self.hurt.push(format!(
                "t {:.2}: hit {hit}, damage {dealt}, reaction {reaction:?}",
                tick as f32 / HZ as f32
            ));
        }
        if let Some(p) = world.get::<Position>(who) {
            self.lift = self.lift.max((p.0.y - self.y).abs());
        }
    }

    pub fn failures(&self, label: &str) -> Vec<String> {
        let mut out = Vec::new();
        if !self.hurt.is_empty() {
            out.push(format!("{label} was hurt: {:?}", self.hurt));
        }
        if self.lift > 0.1 {
            out.push(format!("{label} moved {:.2} m in height", self.lift));
        }
        out
    }
}

pub fn fixed_tick(app: &App) -> u64 {
    let time = app.world().resource::<Time<Fixed>>();
    (time.elapsed().as_nanos() / time.timestep().as_nanos().max(1)) as u64
}

pub fn relax(app: &mut App, car: Entity, blocker: Entity) {
    let since = fixed_tick(app);
    set_car(app, car, |c| {
        c.relaxed = Some(Relax {
            blocker,
            since,
            physics_only: false,
            trailing: None,
        })
    });
}

/// `CollisionStart` messages of one pair.
pub struct Contacts(bevy::ecs::message::MessageCursor<CollisionStart>);

impl Contacts {
    pub fn new(app: &App) -> Self {
        Self(
            app.world()
                .resource::<Messages<CollisionStart>>()
                .get_cursor_current(),
        )
    }

    pub fn touched(&mut self, app: &App, a: Entity, b: Entity) -> bool {
        let messages = app.world().resource::<Messages<CollisionStart>>();
        self.0.read(messages).any(|m| {
            let pair = (
                m.body1.unwrap_or(m.collider1),
                m.body2.unwrap_or(m.collider2),
            );
            pair == (a, b) || pair == (b, a)
        })
    }
}

pub fn kinematic(app: &App, car: Entity) -> bool {
    app.world()
        .get::<RigidBody>(car)
        .is_some_and(|b| b.is_kinematic())
}
