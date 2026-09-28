//! The progress rule (TASK-039): one universal way out for an AI car stuck behind a standing body.
//!
//! Every tick each standing AI car records at most one reason it stands, an edge of the wait-for
//! record: `Follow` (a queue leader, moving traffic a pass waits for, room past the box), `Grant` (a
//! conflicting grant) or `Body` (a body on its path). The edges form a functional graph; a pointer
//! walk finds the sink of each chain. When a sink has stood `progress.wait_seconds`, the `Body` edges
//! of its chain whose waiter (and intermediate target) stood `progress.grace_seconds` are candidates,
//! taken in order of least overlap with their blocker (ties: the lower entity). A car being passed is
//! never moved. The chosen car gets `TrafficCar.relaxed`: for that pair only,
//! sensing, path checks and recovery ignore the blocker, the contact hook drops their contacts (no
//! push, no `CollisionStart`, no damage, no switch), their wheel rays skip each other
//! (`vehicle::PassingThrough`) and a relaxed person does not stand on the car (`TnuaNotPlatform`). The
//! planning part ends when the car is past the body, the body moved or left, the car left AI, or after
//! `max_seconds`; the physics exemption lasts until the two separate, and a car left standing in its
//! blocker's way plans again.

use super::box_rules::connector_clear;
use super::contact::{past, penetration, rim, touches};
use super::drive::{Snap, ahead as path_ahead};
use super::junction::REPICK_WITHIN;
use super::lateral::{CORRIDOR_LATERAL_STEP, effective_lateral, offset_pose, right_of};
use super::manoeuvre::{Sensed, holds_offset};
use super::{
    Manoeuvre, Relax, Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections,
    TrafficMode, TrafficStats,
};
use crate::character::{Character, LocomotionConfig};
use crate::occupancy::{BodyKind, Footprint, RoadOccupancy, flat};
use crate::traffic::FlatRect;
use crate::vehicle::{PassingThrough, Vehicle, VehicleConfig};
use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_tnua::TnuaNotPlatform;
use std::collections::{HashMap, HashSet};

/// The app's collision hooks: a relaxed (car, blocker) pair gets no contacts.
#[derive(SystemParam)]
pub struct TrafficHooks<'w, 's> {
    cars: Query<'w, 's, &'static TrafficCar>,
}

impl TrafficHooks<'_, '_> {
    fn exempt(&self, car: Entity, other: Entity) -> bool {
        self.cars.get(car).is_ok_and(|c| exempt_pair(c, other))
    }
}

impl CollisionHooks for TrafficHooks<'_, '_> {
    fn modify_contacts(&self, contacts: &mut ContactPair, _commands: &mut Commands) -> bool {
        let a = contacts.body1.unwrap_or(contacts.collider1);
        let b = contacts.body2.unwrap_or(contacts.collider2);
        !(self.exempt(a, b) || self.exempt(b, a))
    }
}

/// `car` is relaxed against `other` (either phase: a physics consumer).
pub(super) fn exempt_pair(car: &TrafficCar, other: Entity) -> bool {
    car.relaxed.is_some_and(|r| r.exempts(other))
}

/// The blocker of `car`'s relaxation while it still plans (path checks, recovery).
pub(super) fn planning_blocker(car: &TrafficCar) -> Option<Entity> {
    car.relaxed.filter(|r| !r.physics_only).map(|r| r.blocker)
}

/// The blocker sensing (and the path leader, and the squeeze speed cap) ignores while planning. A
/// `Dynamic` car too: its autopilot drives it through the blocker (a car pinned between the blocker
/// and a body behind it could never recover first).
pub(super) fn sensing_blocker(snap: &Snap) -> Option<Entity> {
    planning_blocker(&snap.car)
}

/// Sensing and the path leader skip `other` while the car plans: its blocker, or the trailing body it
/// is still inside (it can only drive out of it forward).
pub(super) fn sensing_skips(snap: &Snap, other: Entity) -> bool {
    snap.car
        .relaxed
        .is_some_and(|r| !r.physics_only && r.exempts(other))
}

/// End rules and markers of the relaxations (before `advance_traffic` every tick).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn upkeep(
    mut commands: Commands,
    configs: (
        Res<TrafficConfig>,
        Res<VehicleConfig>,
        Res<LocomotionConfig>,
    ),
    time: Res<Time<Fixed>>,
    road: Res<RoadOccupancy>,
    mut cars: Query<(
        Entity,
        &mut TrafficCar,
        &Position,
        &Rotation,
        Option<&PassingThrough>,
        Has<TnuaNotPlatform>,
    )>,
    others: Query<(&Position, &Rotation, Has<Vehicle>, Has<Character>), Without<TrafficCar>>,
) {
    let (cfg, vcfg, loco) = configs;
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let marked = cars.iter().any(|(_, c, _, _, through, platform)| {
        c.relaxed.is_some() || through.is_some() || platform
    });
    if !marked {
        return;
    }
    let step = time.timestep().as_nanos().max(1);
    let tick = (time.elapsed().as_nanos() / step) as u64;
    let half = vcfg.half_extents();
    let half = Vec2::new(half.x, half.z);
    let max_ticks = (cfg.progress.max_seconds / dt).ceil() as u64;
    let traffic: HashMap<Entity, FlatRect> = cars
        .iter()
        .map(|(e, _, p, r, ..)| (e, FlatRect::of(p.0, r.0, half)))
        .collect();
    // A body's footprint, and whether it is a person.
    let shape_of = |e: Entity| {
        if let Some(rect) = traffic.get(&e) {
            return Some((Footprint::Rect(*rect), false));
        }
        let (p, rot, vehicle, character) = others.get(e).ok()?;
        match (vehicle, character) {
            (true, _) => Some((Footprint::Rect(FlatRect::of(p.0, rot.0, half)), false)),
            (_, true) => Some((
                Footprint::Circle {
                    centre: flat(p.0),
                    radius: loco.capsule_radius,
                },
                true,
            )),
            _ => None,
        }
    };
    for (entity, mut car, position, rotation, through, not_platform) in &mut cars {
        let own = FlatRect::of(position.0, rotation.0, half);
        let grown = FlatRect {
            half: half + Vec2::splat(cfg.recover.skin),
            ..own
        };
        let target = car.relaxed.and_then(|r| shape_of(r.blocker));
        let trailing = car
            .relaxed
            .and_then(|r| r.trailing)
            .and_then(|t| Some((t, shape_of(t)?)))
            .filter(|(_, (shape, _))| touches(&grown, shape));
        let main = car.relaxed.and_then(|mut r| {
            let (shape, _) = target?;
            if !r.physics_only {
                let ai = matches!(car.mode, TrafficMode::Kinematic | TrafficMode::Dynamic);
                let moved = road.body(r.blocker).is_some_and(|b| b.standing == 0.0);
                let beyond = past(&own, &shape);
                let stale = tick.saturating_sub(r.since) >= max_ticks;
                r.physics_only = !ai || moved || beyond || stale;
            }
            (!r.physics_only || touches(&grown, &shape)).then_some(r)
        });
        let relaxed = match (main, trailing) {
            (Some(r), t) => Some(Relax {
                trailing: t.map(|t| t.0),
                ..r
            }),
            (None, Some((t, _))) => car.relaxed.map(|r| Relax {
                blocker: t,
                physics_only: true,
                trailing: None,
                ..r
            }),
            (None, None) => None,
        };
        if car.relaxed != relaxed {
            car.relaxed = relaxed;
        }
        let marker = relaxed.map(|r| PassingThrough {
            blocker: r.blocker,
            trailing: r.trailing,
        });
        if through.copied() != marker {
            match marker {
                Some(m) => commands.entity(entity).try_insert(m),
                None => commands.entity(entity).try_remove::<PassingThrough>(),
            };
        }
        let person = |t: Option<(Footprint, bool)>| t.is_some_and(|t| t.1);
        let platform_off = relaxed
            .is_some_and(|r| person(shape_of(r.blocker)) || person(r.trailing.and_then(shape_of)));
        if platform_off != not_platform {
            if platform_off {
                commands.entity(entity).try_insert(TnuaNotPlatform);
            } else {
                commands.entity(entity).try_remove::<TnuaNotPlatform>();
            }
        }
    }
}

/// Why a car stands: a queue or room ahead, a conflicting grant, a body on its path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EdgeKind {
    Follow,
    Grant,
    Body,
}

impl EdgeKind {
    /// Tie order of two reasons at the same gap: a body, then a grant, then a queue.
    fn rank(self) -> u8 {
        match self {
            EdgeKind::Body => 0,
            EdgeKind::Grant => 1,
            EdgeKind::Follow => 2,
        }
    }
}

/// One edge of the wait-for record: the car stands for `target`, `gap` m ahead (the nearest wins).
#[derive(Clone, Copy, Debug)]
pub(super) struct Edge {
    pub target: Entity,
    pub kind: EdgeKind,
    pub gap: f32,
    /// The car's path leader within the look-ahead, whatever its gap (one squeezer per blocker).
    pub leader: Option<Entity>,
}

/// What `edge_of` reads for one car.
pub(super) struct Reasons<'a> {
    /// The path leader: gap, entity.
    pub leader: Option<(f32, Entity)>,
    pub sensed: &'a Sensed,
    /// Its reason as an ungranted waiter (`junction::update`).
    pub junction: Option<(Entity, EdgeKind)>,
    /// The body that keeps a `Dynamic` car from recovering.
    pub stay: Option<Entity>,
    /// The moving car (or passer's claim) a published pass waits for (`pass::waits_for_traffic`).
    pub pass_wait: Option<Entity>,
}

/// The reason a standing AI car stands (at most one edge).
pub(super) fn edge_of(
    snap: &Snap,
    graph: &TrafficGraph,
    reasons: &Reasons,
    cfg: &TrafficConfig,
    half_length: f32,
) -> Option<Edge> {
    let car = &snap.car;
    let leader = reasons.leader.map(|l| l.1);
    let edge = |target, kind, gap| Edge {
        target,
        kind,
        gap,
        leader,
    };
    if snap.dynamic {
        if let Some(b) = reasons.stay {
            return Some(edge(b, EdgeKind::Body, 0.0));
        }
        return reasons
            .leader
            .filter(|l| l.0 <= 2.0 * cfg.idm.min_gap)
            .map(|l| edge(l.1, EdgeKind::Follow, l.0));
    }
    let reach = cfg.pass.trigger_gap;
    let mut options: Vec<Edge> = Vec::new();
    if let Some((gap, e)) = reasons.leader.filter(|l| l.0 <= reach) {
        options.push(edge(e, EdgeKind::Follow, gap));
    }
    let hits = reasons
        .sensed
        .ahead
        .iter()
        .chain(reasons.sensed.beside.iter());
    for hit in hits.filter(|h| h.gap <= reach) {
        let kind = if hit.claim {
            EdgeKind::Follow
        } else {
            EdgeKind::Body
        };
        options.push(edge(hit.entity, kind, hit.gap));
    }
    // A published pass waiting for moving traffic waits for that traffic, not for its obstacle.
    if let Manoeuvre::Pass {
        go: false,
        obstacle,
        hold_s,
        ..
    } = car.manoeuvre
    {
        let wait = match reasons.pass_wait {
            Some(traffic) => edge(traffic, EdgeKind::Follow, hold_s - car.s),
            None => edge(obstacle, EdgeKind::Body, hold_s - car.s),
        };
        options.push(wait);
    }
    if let Some((target, kind)) = reasons.junction {
        let gap = match car.segment {
            Segment::Lane(l) => graph.lane(l).stop - (car.s + half_length),
            Segment::Connector(_) => 0.0,
        };
        options.push(edge(target, kind, gap.max(0.0)));
    }
    options.into_iter().min_by(|a, b| {
        a.gap
            .total_cmp(&b.gap)
            .then(a.kind.rank().cmp(&b.kind.rank()))
    })
}

/// The chain root of each car: its sink (a body with no edge), `None` on a cycle.
fn roots(snaps: &[Snap], edges: &[Option<Edge>]) -> Vec<Option<Entity>> {
    let index: HashMap<Entity, usize> = snaps
        .iter()
        .enumerate()
        .map(|(k, s)| (s.entity, k))
        .collect();
    let mut root: Vec<Option<Option<Entity>>> = vec![None; snaps.len()];
    for start in 0..snaps.len() {
        let mut stack: Vec<usize> = Vec::new();
        let mut at = start;
        let found = loop {
            if let Some(r) = root[at] {
                break r;
            }
            if stack.contains(&at) {
                break None;
            }
            stack.push(at);
            let Some(edge) = edges[at] else {
                break Some(snaps[at].entity);
            };
            match index.get(&edge.target) {
                Some(&next) => at = next,
                None => break Some(edge.target),
            }
        };
        for k in stack {
            root[k] = Some(found);
        }
    }
    root.into_iter().map(Option::flatten).collect()
}

/// Everything `detect` reads about the road.
pub(super) struct World<'a> {
    pub road: &'a RoadOccupancy,
    pub graph: &'a TrafficGraph,
    pub junctions: &'a TrafficIntersections,
    pub cfg: &'a TrafficConfig,
    pub half: Vec3,
    pub tick: u64,
}

/// How deep the car's body driven on along its path (at its current offset and on the path line)
/// reaches into `blocker` until its rear is past it; for a head that may still take another exit, the
/// least over its exits clear of every other body.
fn overlap(w: &World, snap: &Snap, blocker: Entity) -> f32 {
    let Some(body) = w.road.body(blocker) else {
        return f32::INFINITY;
    };
    let car = &snap.car;
    let half = Vec2::new(w.half.x, w.half.z);
    let (point, tangent) = w.graph.pose(car.segment, car.s);
    let current = if snap.dynamic {
        (snap.position - point).with_y(0.0).dot(right_of(tangent))
    } else {
        effective_lateral(w.graph, car.segment, car.s, car.lateral, holds_offset(car))
    };
    let forward = flat(tangent).normalize_or_zero();
    let reach = rim(&body.shape, Vec2::X)
        .iter()
        .map(|&q| (q - flat(snap.position)).dot(forward))
        .fold(0.0, f32::max)
        + half.y;
    let depth = |seg: Segment, s: f32, next: Option<u32>| {
        let mut worst: f32 = 0.0;
        let mut d = 0.0;
        while d <= reach {
            let (seg, s) = path_ahead(w.graph, seg, s, next, d);
            for lateral in [current, 0.0] {
                let (p, t) = offset_pose(w.graph, seg, s, lateral);
                let rect = FlatRect {
                    centre: flat(p),
                    axis: flat(right_of(t)).normalize_or(Vec2::X),
                    half,
                };
                worst = worst.max(penetration(&rect, &body.shape));
            }
            d += CORRIDOR_LATERAL_STEP;
        }
        worst
    };
    let exits = repick_exits(w, snap, blocker);
    if exits.is_empty() {
        return depth(car.segment, car.s, car.next);
    }
    exits
        .into_iter()
        .map(|(seg, s, next)| depth(seg, s, next))
        .fold(f32::INFINITY, f32::min)
}

/// For a head at its stop line or at the very start of its connector: its exits clear of every body but
/// `blocker` as (segment, s, next) starts; empty otherwise.
fn repick_exits(w: &World, snap: &Snap, blocker: Entity) -> Vec<(Segment, f32, Option<u32>)> {
    let car = &snap.car;
    let (lane, from_s, on_connector) = match car.segment {
        Segment::Lane(l) if !snap.dynamic => {
            let lane = w.graph.lane(l);
            if lane.stop - (car.s + w.half.z) > 2.0 * w.cfg.idm.min_gap {
                return Vec::new();
            }
            (l, 0.0, false)
        }
        Segment::Connector(c) if !snap.dynamic && car.s <= REPICK_WITHIN => {
            (w.graph.connector(c).from_lane, car.s, true)
        }
        _ => return Vec::new(),
    };
    let half = Vec2::new(w.half.x, w.half.z);
    w.graph
        .lane(lane)
        .out
        .iter()
        .copied()
        .filter(|&c| {
            let here = w.junctions.0.get(&w.graph.connector(c).node);
            connector_clear(
                w.road,
                w.graph,
                here,
                c,
                from_s,
                snap.entity,
                half,
                Some(blocker),
            )
            .is_none()
        })
        .map(|c| {
            if on_connector {
                (Segment::Connector(c), from_s, None)
            } else {
                (car.segment, car.s, Some(c))
            }
        })
        .collect()
}

fn ai(s: &Snap) -> bool {
    !s.abandon && matches!(s.car.mode, TrafficMode::Kinematic | TrafficMode::Dynamic)
}

/// The wait-for detector: roots every chain, and relaxes the `Body` edges of chains whose sink has
/// stood `progress.wait_seconds` (their waiter, and an intermediate target, `grace_seconds`), least
/// overlap first; never a car that is being passed.
pub(super) fn detect(
    w: &World,
    snaps: &mut [Snap],
    edges: &[Option<Edge>],
    stats: &mut TrafficStats,
) {
    let p = &w.cfg.progress;
    let standing = |e: Entity| w.road.body(e).map_or(0.0, |b| b.standing);
    let edges = settle(w, snaps, edges);
    let roots = roots(snaps, &edges);
    let long = |s: &Snap| ai(s) && standing(s.entity) > p.wait_seconds;
    stats.unexplained = snaps
        .iter()
        .zip(&edges)
        .filter(|(s, e)| long(s) && s.car.relaxed.is_none() && e.is_none())
        .count() as u32;
    stats.stalled_relaxed = snaps
        .iter()
        .filter(|s| long(s) && s.car.relaxed.is_some())
        .count() as u32;
    let candidates = candidates(w, snaps, &edges, &roots);
    accept(w, snaps, &edges, candidates, stats);
}

/// A `Follow` edge to a body that stood the whole wait with no reason of its own and is not driven by
/// the AI (a bailing car, a body with no edge) becomes `Body`: it is the body the queue waits for.
fn settle(w: &World, snaps: &[Snap], edges: &[Option<Edge>]) -> Vec<Option<Edge>> {
    let standing = |e: Entity| w.road.body(e).map_or(0.0, |b| b.standing);
    // A driven car with no recorded reason is never passed through: it counts as unexplained.
    let accounted: HashSet<Entity> = snaps
        .iter()
        .zip(edges)
        .filter(|(s, e)| e.is_some() || ai(s))
        .map(|(s, _)| s.entity)
        .collect();
    edges
        .iter()
        .map(|e| {
            e.map(|mut e| {
                if e.kind == EdgeKind::Follow
                    && !accounted.contains(&e.target)
                    && standing(e.target) >= w.cfg.progress.wait_seconds
                {
                    e.kind = EdgeKind::Body;
                }
                e
            })
        })
        .collect()
}

/// Due `Body` edges as (overlap, waiter index, blocker), least overlap first.
fn candidates(
    w: &World,
    snaps: &[Snap],
    edges: &[Option<Edge>],
    roots: &[Option<Entity>],
) -> Vec<(f32, usize, Entity)> {
    let p = &w.cfg.progress;
    let standing = |e: Entity| w.road.body(e).map_or(0.0, |b| b.standing);
    let person = |e: Entity| {
        w.road
            .body(e)
            .is_some_and(|b| b.kind == BodyKind::Character)
    };
    let mut due: Vec<(usize, Edge)> = Vec::new();
    for (k, snap) in snaps.iter().enumerate() {
        let (Some(edge), Some(sink)) = (edges[k], roots[k]) else {
            continue;
        };
        // A new squeeze, one back in its blocker's way (the blocker moved and stood again, or the
        // squeeze went stale) planning again, or one held by the next body ahead (after its blocker,
        // or while still inside it): the old blocker stays exempt until the two separate.
        let free = snap.car.relaxed.is_none_or(|r| {
            let inside = w
                .road
                .body(r.blocker)
                .is_some_and(|b| penetration(&snap_rect(w, snap), &b.shape) > 0.0);
            r.physics_only || (r.blocker != edge.target && inside)
        });
        // The sink's clock orders a chain; a car that has itself waited the whole time behind
        // vehicles goes once its sink stood the grace (a nudge restarts a vehicle's clock, not the
        // wait). A person keeps his own clock.
        let (w_stood, s_stood) = (standing(snap.entity), standing(sink));
        let own_clock = !person(edge.target) && !person(sink);
        let clock = s_stood >= p.wait_seconds
            || (own_clock && w_stood >= p.wait_seconds && s_stood >= p.grace_seconds);
        if edge.kind == EdgeKind::Body
            && ai(snap)
            && free
            && clock
            && w_stood >= p.grace_seconds
            && (edge.target == sink || standing(edge.target) >= p.grace_seconds)
        {
            due.push((k, edge));
        }
    }
    let index: HashMap<Entity, usize> = snaps
        .iter()
        .enumerate()
        .map(|(k, s)| (s.entity, k))
        .collect();
    let rect = |s: &Snap| snap_rect(w, s);
    let mut out: Vec<(f32, usize, Entity)> = Vec::new();
    for (k, edge) in due {
        let (snap, b) = (&snaps[k], edge.target);
        let Some(body) = w.road.body(b) else {
            continue;
        };
        // The squeezer is the car nearest the blocker on its path.
        let nearer = edge
            .leader
            .filter(|&l| l != b)
            .and_then(|l| index.get(&l))
            .is_some_and(|&j| {
                let leader = &snaps[j];
                exempt_pair(&leader.car, b) || penetration(&rect(leader), &body.shape) > 0.0
            });
        if nearer {
            continue;
        }
        out.push((overlap(w, snap, b), k, b));
    }
    out.sort_by(|a, b| {
        a.0.total_cmp(&b.0).then(
            snaps[a.1]
                .entity
                .to_bits()
                .cmp(&snaps[b.1].entity.to_bits()),
        )
    });
    out
}

fn snap_rect(w: &World, s: &Snap) -> FlatRect {
    FlatRect::of(s.position, s.rotation, Vec2::new(w.half.x, w.half.z))
}

/// Accepts the candidates in order: never a waiter that is being passed, never through a blocker
/// that is squeezing on itself.
fn accept(
    w: &World,
    snaps: &mut [Snap],
    edges: &[Option<Edge>],
    candidates: Vec<(f32, usize, Entity)>,
    stats: &mut TrafficStats,
) {
    // Squeezes under way; one whose car stands for anything (a grant too) is not passing now.
    let mut active: Vec<(Entity, Entity)> = snaps
        .iter()
        .zip(edges)
        .filter_map(|(s, e)| {
            let b = planning_blocker(&s.car)?;
            e.is_none().then_some((s.entity, b))
        })
        .collect();
    for (_, k, blocker) in candidates {
        let me = snaps[k].entity;
        let passed = active
            .iter()
            .any(|&(waiter, b)| b == me || waiter == blocker);
        if passed {
            continue;
        }
        let car = &mut snaps[k].car;
        let trailing = car.relaxed.map(|r| r.blocker).filter(|&b| b != blocker);
        car.relaxed = Some(Relax {
            blocker,
            since: w.tick,
            physics_only: false,
            trailing,
        });
        // A pass around the blocker (published, or a box pass under way) gives way to the squeeze:
        // its claim would hold oncoming cars that the car itself then waits for.
        if matches!(car.manoeuvre, Manoeuvre::Pass { obstacle, .. } if obstacle == blocker) {
            car.manoeuvre = Manoeuvre::None;
        }
        active.push((me, blocker));
        stats.progress_relaxations += 1;
    }
}
