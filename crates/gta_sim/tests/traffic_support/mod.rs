//! Shared fixtures of the traffic gates (`traffic_*.rs`).
#![allow(dead_code)]

pub mod progress;
mod third_body;
pub use third_body::*;

use crate::common::*;
use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use gta_sim::{
    layers::GameLayer,
    traffic::{
        FlatRect, Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficStats,
        spawn_traffic_car as spawn_car_with, swept_rect_hits_rect,
    },
    vehicle::{DamageConfig, VehicleConfig},
};

/// A lane (from, to, v0, end node) at road-top height.
pub type LaneSpec = (Vec3, Vec3, f32, u32);

/// The far sidewalk stub of a traffic floor (no fleeing civilian runs into the lanes).
pub const STUB: (Vec3, Vec3) = (Vec3::new(30.0, 0.0, -30.0), Vec3::new(35.0, 0.0, -30.0));

/// Test floor with the player settled at the origin, a sidewalk graph of `sidewalk_runs` (default
/// `STUB`) and the synthetic traffic graph; no camera view (the bubble never spawns or despawns).
pub fn traffic_floor(
    lanes: Vec<LaneSpec>,
    connectors: &[(u32, u32, u32)],
    sidewalk_runs: &[(Vec3, Vec3)],
) -> App {
    let mut app = headless_app();
    let runs = if sidewalk_runs.is_empty() {
        &[STUB][..]
    } else {
        sidewalk_runs
    };
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for &(a, b) in runs {
        let first = nodes.len() as u32;
        nodes.extend([a, b]);
        edges.push((first, first + 1));
    }
    test_graph(&mut app, nodes, &edges);
    settle(&mut app);
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let graph = TrafficGraph::new(lanes, connectors, &cfg, Vec2::new(half.x, half.z))
        .unwrap_or_else(|e| panic!("GATE BROKEN: test traffic graph: {e}"));
    app.world_mut().insert_resource(graph);
    app
}

/// A closed loop around the test floor (x, z within ±37, clear of every test-area block): four lanes
/// with rounded corners of radius 3 m (one connector in, one out per corner, no conflicts), `v0` on
/// every lane.
pub fn loop_lanes(v0: f32) -> (Vec<LaneSpec>, Vec<(u32, u32, u32)>) {
    let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    (
        vec![
            (at(-34.0, 37.0), at(34.0, 37.0), v0, 0),
            (at(37.0, 34.0), at(37.0, -34.0), v0, 1),
            (at(34.0, -37.0), at(-34.0, -37.0), v0, 2),
            (at(-37.0, -34.0), at(-37.0, 34.0), v0, 3),
        ],
        vec![(0, 1, 0), (1, 2, 1), (2, 3, 2), (3, 0, 3)],
    )
}

/// The path of a car running a loop from lane 0: lane, connector, lane, ... (8 segments).
pub fn loop_path(graph: &TrafficGraph) -> Vec<Segment> {
    let mut segs = Vec::new();
    for lane in 0..4u32 {
        segs.push(Segment::Lane(lane));
        segs.push(Segment::Connector(graph.lane(lane).out[0]));
    }
    segs
}

/// `(segment, s)` at `distance` m along `path` (wrapping).
pub fn place_on(graph: &TrafficGraph, path: &[Segment], distance: f32) -> (Segment, f32) {
    let total: f32 = path.iter().map(|&s| graph.length(s)).sum();
    let mut d = distance.rem_euclid(total);
    for &seg in path {
        let length = graph.length(seg);
        if d < length {
            return (seg, d);
        }
        d -= length;
    }
    (path[0], 0.0)
}

pub fn graph(app: &App) -> TrafficGraph {
    app.world().resource::<TrafficGraph>().clone()
}

/// A traffic car through the production spawn path, then one tick; `GATE BROKEN` if its chassis
/// starts inside a test-area block.
pub fn spawn_traffic_car(app: &mut App, seg: Segment, s: f32, speed: f32) -> Entity {
    let car = app
        .world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  graph: Res<TrafficGraph>,
                  cfg: Res<VehicleConfig>,
                  dmg: Res<DamageConfig>| {
                spawn_car_with(&mut commands, &graph, &cfg, &dmg, seg, s, speed, 0)
            },
        )
        .expect("GATE BROKEN: spawn system failed");
    let cfg = app.world().resource::<VehicleConfig>().clone();
    let at = position_of_or_pose(app, car, seg, s, &cfg);
    let h = cfg.half_extents();
    let rotation = Quat::from_rotation_y(gta_sim::combat::aim_yaw(graph(app).pose(seg, s).1));
    let blocks = app
        .world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            spatial.shape_intersections(
                // The chassis box above its 0.2 m underbody lift (clear of the floor at rest).
                &Collider::cuboid(2.0 * h.x, 2.0 * h.y - 0.2, 2.0 * h.z),
                at + Vec3::Y * 0.1,
                rotation,
                &SpatialQueryFilter::from_mask(GameLayer::World),
            )
        })
        .expect("GATE BROKEN: overlap query failed");
    assert!(
        blocks.is_empty(),
        "GATE BROKEN: traffic car at {at} overlaps the test area: {blocks:?}"
    );
    car
}

fn position_of_or_pose(app: &App, car: Entity, seg: Segment, s: f32, cfg: &VehicleConfig) -> Vec3 {
    app.world()
        .get::<Transform>(car)
        .map(|t| t.translation)
        .unwrap_or_else(|| graph(app).pose(seg, s).0 + Vec3::Y * cfg.rest_height())
}

pub fn traffic_car(app: &App, car: Entity) -> TrafficCar {
    *app.world()
        .get::<TrafficCar>(car)
        .expect("GATE BROKEN: traffic car missing")
}

pub fn cars(app: &mut App) -> Vec<Entity> {
    let mut cars: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<TrafficCar>>()
        .iter(app.world())
        .collect();
    cars.sort_by_key(|e| e.to_bits());
    cars
}

pub fn stats(app: &App) -> TrafficStats {
    *app.world().resource::<TrafficStats>()
}

pub fn set_traffic(app: &mut App, update: impl FnOnce(&mut TrafficConfig)) {
    update(app.world_mut().resource_mut::<TrafficConfig>().as_mut());
}

/// Chassis footprint of a car body from its `Position` / `Rotation`.
pub fn footprint(app: &App, car: Entity) -> FlatRect {
    let half = app.world().resource::<VehicleConfig>().half_extents();
    FlatRect::of(
        position_of(app, car),
        app.world().get::<Rotation>(car).unwrap().0,
        Vec2::new(half.x, half.z),
    )
}

/// The chassis footprints of two cars overlap (2D SAT, shrunk 1 cm against touching).
pub fn obb_overlap(a: &FlatRect, b: &FlatRect) -> bool {
    let shrink = |r: &FlatRect| FlatRect {
        half: r.half - Vec2::splat(0.01),
        ..*r
    };
    swept_rect_hits_rect(&shrink(a), Vec2::ZERO, &shrink(b))
}

/// Liveness: the drive system cast at least `cars × ticks / 2` times since `before`.
pub fn assert_traffic_ran(app: &App, before: u32, cars: u32, ticks: u32) {
    let casts = stats(app).casts.wrapping_sub(before);
    assert!(
        casts >= cars * ticks / 2,
        "GATE BROKEN: traffic systems did not run ({casts} casts for {cars} cars in {ticks} ticks)"
    );
}

/// One interpenetration the footprint oracle saw.
#[derive(Clone, Debug)]
pub struct Violation {
    pub tick: u32,
    pub pair: (Entity, Entity),
    /// Which side of the pair was kinematic.
    pub kinematic: (bool, bool),
    pub depth: f32,
    pub at: (Vec2, Vec2),
}

/// A flat oriented rectangle: centre, unit right axis, unit forward axis, half extents.
#[derive(Clone, Copy, Debug)]
struct Rect {
    centre: Vec2,
    right: Vec2,
    forward: Vec2,
    half: Vec2,
}

impl Rect {
    fn of(position: Vec3, rotation: Quat, half: Vec2) -> Self {
        let r = rotation * Vec3::X;
        let f = rotation * Vec3::NEG_Z;
        Self {
            centre: Vec2::new(position.x, position.z),
            right: Vec2::new(r.x, r.z).normalize_or(Vec2::X),
            forward: Vec2::new(f.x, f.z).normalize_or(Vec2::NEG_Y),
            half,
        }
    }

    fn radius(&self) -> f32 {
        self.half.length()
    }

    fn project(&self, n: Vec2) -> (f32, f32) {
        let c = self.centre.dot(n);
        let r = self.half.x * self.right.dot(n).abs() + self.half.y * self.forward.dot(n).abs();
        (c - r, c + r)
    }
}

/// Penetration depth of two rectangles (minimum overlap over the four face normals; <= 0: apart).
fn penetration(a: &Rect, b: &Rect) -> f32 {
    [a.right, a.forward, b.right, b.forward]
        .into_iter()
        .map(|n| {
            let (a0, a1) = a.project(n);
            let (b0, b1) = b.project(n);
            a1.min(b1) - a0.max(b0)
        })
        .fold(f32::INFINITY, f32::min)
}

/// Distinct interpenetrating pair: (pair, worst depth, first tick, kinematic sides, centres).
pub type PairSummary = ((Entity, Entity), f32, u32, (bool, bool), (Vec2, Vec2));

/// G1 oracle: every tick, no two vehicle footprints interpenetrate while at least one of them is
/// kinematic. Its own SAT on `Position` / `Rotation`, independent of the occupancy code. Tolerances:
/// kinematic x kinematic 0.02 m (1 cm per side, the `obb_overlap` shrink; such pairs get no solver
/// response), kinematic x dynamic `max_overlap_solve_speed x dt` (what the solver pushes apart in one
/// step at its maximum push-out speed).
pub struct Footprints {
    pub kinematic_tolerance: f32,
    pub dynamic_tolerance: f32,
    pub violations: Vec<Violation>,
    pub max_depth: f32,
    pub pairs: u64,
    /// Relaxed pairs and third bodies (TASK-039, `third_body.rs`).
    pub third: Third,
}

impl Footprints {
    pub fn new(app: &App) -> Self {
        let world = app.world();
        let solver = world.resource::<avian3d::dynamics::solver::SolverConfig>();
        let unit = world.resource::<PhysicsLengthUnit>().0;
        let dt = world.resource::<Time<Fixed>>().timestep().as_secs_f32();
        Self {
            kinematic_tolerance: 0.02,
            dynamic_tolerance: solver.max_overlap_solve_speed * unit * dt,
            violations: Vec::new(),
            max_depth: 0.0,
            pairs: 0,
            third: Third::default(),
        }
    }

    /// Checks every vehicle within 150 m of the player (all without a player).
    pub fn record(&mut self, app: &mut App, tick: u32) {
        let half = app.world().resource::<VehicleConfig>().half_extents();
        let half = Vec2::new(half.x, half.z);
        let centre = app
            .world_mut()
            .query_filtered::<&Position, With<gta_sim::player::Player>>()
            .iter(app.world())
            .next()
            .map(|p| p.0);
        let bodies: Vec<(Entity, Rect, bool)> = app
            .world_mut()
            .query_filtered::<(Entity, &Position, &Rotation, &RigidBody), With<gta_sim::vehicle::Vehicle>>()
            .iter(app.world())
            .filter(|(_, p, ..)| {
                centre.is_none_or(|c| Vec2::new(p.0.x - c.x, p.0.z - c.z).length() <= 150.0)
            })
            .map(|(e, p, r, b)| (e, Rect::of(p.0, r.0, half), b.is_kinematic()))
            .collect();
        let relaxed = relaxed_pairs(app);
        for i in 0..bodies.len() {
            for j in i + 1..bodies.len() {
                let (a, b) = (&bodies[i], &bodies[j]);
                if !(a.2 || b.2) || a.1.centre.distance(b.1.centre) > a.1.radius() + b.1.radius() {
                    continue;
                }
                self.pairs += 1;
                let depth = penetration(&a.1, &b.1);
                if self.third.exempt(&relaxed, (a.0, b.0), depth) {
                    continue;
                }
                self.max_depth = self.max_depth.max(depth);
                let tolerance = if a.2 && b.2 {
                    self.kinematic_tolerance
                } else {
                    self.dynamic_tolerance
                };
                if depth > tolerance {
                    self.violations.push(Violation {
                        tick,
                        pair: (a.0, b.0),
                        kinematic: (a.2, b.2),
                        depth,
                        at: (a.1.centre, b.1.centre),
                    });
                }
            }
        }
        self.record_third(app, tick, &bodies, &relaxed);
    }

    pub fn max_depth(&self) -> f32 {
        self.max_depth
    }

    pub fn summary(&self) -> Vec<PairSummary> {
        let mut out: Vec<PairSummary> = Vec::new();
        for v in &self.violations {
            match out.iter_mut().find(|o| o.0 == v.pair) {
                Some(o) => o.1 = o.1.max(v.depth),
                None => out.push((v.pair, v.depth, v.tick, v.kinematic, v.at)),
            }
        }
        out
    }

    pub fn assert_clean(&self, label: &str) {
        self.assert_third_clean(label);
        assert!(
            self.violations.is_empty(),
            "{label}: {} kinematic interpenetration ticks over {} close pairs (tolerance kin {:.3} / \
             dyn {:.4} m), distinct pairs (pair, depth, first tick, kinematic, centres): {:?}",
            self.violations.len(),
            self.pairs,
            self.kinematic_tolerance,
            self.dynamic_tolerance,
            self.summary()
        );
    }
}

/// No AI car stands longer than this in `Dynamic` in any city gate of TASK-032 (spec G3), s.
pub const DYNAMIC_STAND_S: f32 = 30.0;

/// Per AI car: the longest continuous stand below `hold_speed`, and the longest one in `Dynamic`.
#[derive(Default)]
pub struct StandClock {
    now: f32,
    /// entity -> (stand start, dynamic stand start)
    current: std::collections::HashMap<Entity, (Option<f32>, Option<f32>)>,
    /// entity -> (longest stand, longest dynamic stand, where the longest stand is, where the
    /// longest dynamic stand is)
    longest: std::collections::HashMap<Entity, (f32, f32, Vec3, Vec3)>,
}

impl StandClock {
    pub fn record(&mut self, app: &mut App) {
        let dt = app
            .world()
            .resource::<Time<Fixed>>()
            .timestep()
            .as_secs_f32();
        let hold = app.world().resource::<VehicleConfig>().hold_speed;
        self.now += dt;
        let now = self.now;
        let cars: Vec<(Entity, bool, bool, Vec3)> = app
            .world_mut()
            .query::<(Entity, &TrafficCar, &LinearVelocity, &Position)>()
            .iter(app.world())
            .filter(|(_, c, ..)| c.is_ai())
            .map(|(e, c, v, p)| {
                (
                    e,
                    v.0.with_y(0.0).length() < hold,
                    c.mode == gta_sim::traffic::TrafficMode::Dynamic,
                    p.0,
                )
            })
            .collect();
        self.current.retain(|e, _| cars.iter().any(|c| c.0 == *e));
        for (e, stands, dynamic, at) in cars {
            let entry = self.current.entry(e).or_insert((None, None));
            if !stands {
                *entry = (None, None);
                continue;
            }
            let since = *entry.0.get_or_insert(now - dt);
            if !dynamic {
                entry.1 = None;
            }
            let dyn_since = dynamic.then(|| *entry.1.get_or_insert(now - dt));
            let best = self.longest.entry(e).or_insert((0.0, 0.0, at, at));
            if now - since > best.0 {
                best.0 = now - since;
                best.2 = at;
            }
            if let Some(d) = dyn_since
                && now - d > best.1
            {
                best.1 = now - d;
                best.3 = at;
            }
        }
    }

    /// Longest stand of any car, s, and where.
    pub fn worst(&self) -> (f32, Vec3) {
        self.longest
            .values()
            .map(|v| (v.0, v.2))
            .fold((0.0, Vec3::ZERO), |a, b| if b.0 > a.0 { b } else { a })
    }

    /// Longest stand in `Dynamic`, s.
    pub fn worst_dynamic(&self) -> f32 {
        self.longest.values().map(|v| v.1).fold(0.0, f32::max)
    }

    /// The TASK-032 bound every city gate carries (the TASK-031 M1 class: bumped cars left `Dynamic`
    /// stood 87-155 s): a violation when an AI car stood in `Dynamic` longer than `DYNAMIC_STAND_S`.
    pub fn dynamic_violation(&self) -> Option<String> {
        let worst = self.worst_dynamic();
        (worst > DYNAMIC_STAND_S)
            .then(|| format!("an AI car stood {worst:.1} s in Dynamic (> {DYNAMIC_STAND_S} s)"))
    }

    /// Cars whose longest `Dynamic` stand is over `seconds`: (car, stand, where), longest first.
    pub fn dynamic_longer_than(&self, seconds: f32) -> Vec<(Entity, f32, Vec3)> {
        let mut out: Vec<_> = self
            .longest
            .iter()
            .filter(|(_, v)| v.1 > seconds)
            .map(|(e, v)| (*e, v.1, v.3))
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }

    pub fn longer_than(&self, seconds: f32) -> Vec<(Entity, f32, Vec3)> {
        let mut out: Vec<_> = self
            .longest
            .iter()
            .filter(|(_, v)| v.0 > seconds)
            .map(|(e, v)| (*e, v.0, v.2))
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }

    /// Longest stand of `car`, s.
    pub fn of(&self, car: Entity) -> f32 {
        self.longest.get(&car).map_or(0.0, |v| v.0)
    }
}

pub fn set_car(app: &mut App, car: Entity, update: impl FnOnce(&mut TrafficCar)) {
    update(
        app.world_mut()
            .get_mut::<TrafficCar>(car)
            .expect("GATE BROKEN: traffic car missing")
            .as_mut(),
    );
}

/// Moves a body to `at` (Position and Transform, so avian does not snap it back).
pub fn teleport(app: &mut App, body: Entity, at: Vec3) {
    app.world_mut().get_mut::<Position>(body).unwrap().0 = at;
    app.world_mut()
        .get_mut::<Transform>(body)
        .unwrap()
        .translation = at;
}

/// Right of a flat direction.
pub fn right_of(d: Vec3) -> Vec3 {
    Vec3::new(-d.z, 0.0, d.x)
}

/// Height of the ground (road or sidewalk top) under `xz`.
pub fn ground_at(app: &mut App, xz: Vec2) -> f32 {
    let from = Vec3::new(xz.x, 30.0, xz.y);
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            spatial
                .cast_ray(
                    from,
                    Dir3::NEG_Y,
                    40.0,
                    true,
                    &SpatialQueryFilter::from_mask(GameLayer::World),
                )
                .map(|hit| from.y - hit.distance)
        })
        .expect("GATE BROKEN: ray system failed")
        .unwrap_or_else(|| panic!("GATE BROKEN: no ground under {xz}"))
}

/// Stands the player on the ground at `xz` and points the camera view along the flat `dir`.
pub fn stand_player(app: &mut App, xz: Vec2, dir: Vec3) -> Vec3 {
    let y = ground_at(app, xz);
    let float = app
        .world()
        .resource::<gta_sim::character::LocomotionConfig>()
        .float_height;
    let feet = Vec3::new(xz.x, y, xz.y);
    place_player(app, feet + Vec3::Y * float);
    set_view(app, Some(chase_view(feet, dir.with_y(0.0).normalize())));
    feet
}

/// The longest lane with desired speed `v0` whose stop line lies at least `min_stop` m along it.
pub fn longest_lane(app: &App, v0: f32, min_stop: f32) -> u32 {
    let graph = app.world().resource::<TrafficGraph>();
    graph
        .lanes()
        .iter()
        .enumerate()
        .filter(|(_, l)| (l.v0 - v0).abs() < 1e-3 && l.stop >= min_stop)
        .max_by(|a, b| a.1.stop.total_cmp(&b.1.stop))
        .map(|(k, _)| k as u32)
        .unwrap_or_else(|| {
            panic!("GATE BROKEN: no lane of v0 {v0} with {min_stop} m to its stop line")
        })
}

/// Despawns every traffic car whose centre lies within `radius` (flat) of `at` (a named fixture
/// mutation: the spot for a placed body).
pub fn clear_spot(app: &mut App, at: Vec3, radius: f32) {
    let near: Vec<Entity> = app
        .world_mut()
        .query_filtered::<(Entity, &Position), With<TrafficCar>>()
        .iter(app.world())
        .filter(|(_, p)| (p.0 - at).with_y(0.0).length() < radius)
        .map(|(e, _)| e)
        .collect();
    for e in near {
        app.world_mut()
            .resource_mut::<gta_sim::traffic::TrafficIntersections>()
            .release(e);
        app.world_mut().despawn(e);
    }
}

/// A car body with no AI (a car the player left), production bundle, at rest at the flat point `at`
/// facing `dir`; one tick.
pub fn park_car(app: &mut App, at: Vec3, dir: Vec3) -> Entity {
    let cfg = app.world().resource::<VehicleConfig>().clone();
    let dmg = app.world().resource::<DamageConfig>().clone();
    let centre = Vec3::new(at.x, at.y + cfg.rest_height(), at.z);
    let rotation = Quat::from_rotation_y(gta_sim::combat::aim_yaw(dir));
    let car = app
        .world_mut()
        .spawn(gta_sim::vehicle::vehicle_bundle(
            &cfg,
            &dmg,
            Transform::from_translation(centre).with_rotation(rotation),
        ))
        .id();
    run_ticks(app, 1);
    car
}

/// Feeds AI cars through the production spawn path at the start of `lane` every `every` ticks while
/// the first 20 m of the lane are free, up to `count` cars.
pub struct Feeder {
    pub lane: u32,
    pub every: u32,
    pub count: usize,
    pub spawned: Vec<Entity>,
    next: u32,
}

impl Feeder {
    pub fn new(lane: u32, every: u32, count: usize) -> Self {
        Self {
            lane,
            every,
            count,
            spawned: Vec::new(),
            next: 0,
        }
    }

    pub fn tick(&mut self, app: &mut App, tick: u32) {
        if tick < self.next || self.spawned.len() >= self.count {
            return;
        }
        let graph = graph(app);
        let lane = graph.lane(self.lane).clone();
        let s = app.world().resource::<VehicleConfig>().half_extents().z + 1.0;
        let busy = app
            .world_mut()
            .query_filtered::<&Position, Or<(
                With<gta_sim::vehicle::Vehicle>,
                With<gta_sim::character::Character>,
            )>>()
            .iter(app.world())
            .any(|p| {
                let d = (p.0 - lane.from).with_y(0.0);
                let along = d.dot(lane.dir);
                (s - 6.0..=s + 20.0).contains(&along) && (d - lane.dir * along).length() < 2.5
            });
        if busy {
            return;
        }
        let car = spawn_traffic_car(app, Segment::Lane(self.lane), s, 6.0);
        self.spawned.push(car);
        self.next = tick + self.every;
    }
}

/// A straight two-way street along X (clear of every test-area block): lane 0 heads +X at z = 30,
/// lane 1 heads -X at z = 26.75 (3.25 m to lane 0's left); U connectors at both ends.
pub fn two_way_street(length: f32) -> (Vec<LaneSpec>, Vec<(u32, u32, u32)>) {
    let x = length / 2.0;
    assert!(
        x <= 37.0,
        "GATE BROKEN: a {length} m street leaves the 80 m test floor"
    );
    let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    (
        vec![
            (at(-x, 30.0), at(x, 30.0), 12.0, 0),
            (at(x, 26.75), at(-x, 26.75), 12.0, 1),
        ],
        vec![(0, 1, 0), (1, 0, 1)],
    )
}

/// Named mutation of the driving gates: the road ahead of the player's car is kept clear (he drives on
/// whatever is in front; the subject is what happens behind him): traffic cars less than 60 m ahead
/// and 2.5 m to the side of its heading line despawn.
pub fn clear_ahead(app: &mut App, car: Entity) {
    let me = app.world().get::<Position>(car).unwrap().0;
    let forward = (app.world().get::<Rotation>(car).unwrap().0 * Vec3::NEG_Z)
        .with_y(0.0)
        .normalize();
    let ahead: Vec<Entity> = app
        .world_mut()
        .query_filtered::<(Entity, &Position), With<TrafficCar>>()
        .iter(app.world())
        .filter(|(_, p)| {
            let d = (p.0 - me).with_y(0.0);
            let along = d.dot(forward);
            along > 0.0 && along < 60.0 && (d - forward * along).length() < 2.5
        })
        .map(|(e, _)| e)
        .collect();
    for e in ahead {
        app.world_mut()
            .resource_mut::<gta_sim::traffic::TrafficIntersections>()
            .release(e);
        app.world_mut().despawn(e);
    }
}

/// From the two-way lane of desired speed `v0` with the longest straight run ahead: that lane and the
/// connectors and lanes straight on after it (at most 8 more lanes).
pub fn straightest_road(g: &TrafficGraph, v0: f32) -> Vec<Segment> {
    let chain = |mut lane: u32| {
        let mut segs = vec![Segment::Lane(lane)];
        for _ in 0..8 {
            let l = g.lane(lane);
            let Some(c) = l
                .out
                .iter()
                .copied()
                .find(|&c| g.lane(g.connector(c).to_lane).dir.dot(l.dir) > 0.99)
            else {
                break;
            };
            lane = g.connector(c).to_lane;
            segs.extend([Segment::Connector(c), Segment::Lane(lane)]);
        }
        segs
    };
    let length = |segs: &Vec<Segment>| segs.iter().map(|&s| g.length(s)).sum::<f32>();
    g.lanes()
        .iter()
        .enumerate()
        .filter(|(_, l)| (l.v0 - v0).abs() < 1e-3 && l.left_gap.is_some())
        .map(|(k, _)| chain(k as u32))
        .max_by(|a, b| length(a).total_cmp(&length(b)))
        .expect("GATE BROKEN: no two-way lane")
}

/// The junction nearest to `feet` (by the centre of its connector points), and that centre.
pub fn nearest_box(app: &App, feet: Vec3) -> (u32, Vec3) {
    let g = graph(app);
    let mut hubs: std::collections::HashMap<u32, (Vec3, u32)> = Default::default();
    for c in g.connectors() {
        let hub = hubs.entry(c.node).or_insert((Vec3::ZERO, 0));
        for &p in &c.points {
            hub.0 += p;
            hub.1 += 1;
        }
    }
    hubs.into_iter()
        .map(|(node, (sum, n))| (node, sum / n as f32))
        .min_by(|a, b| {
            let d = |h: Vec3| (h - feet).with_y(0.0).length();
            d(a.1).total_cmp(&d(b.1))
        })
        .expect("GATE BROKEN: no junction")
}

/// The sidewalk point (off the carriageway: a point of a sidewalk graph edge, 1 m apart) whose flat
/// distance to `at` is nearest to `distance` m.
pub fn sidewalk_at(app: &App, at: Vec3, distance: f32) -> Vec3 {
    let walks = app.world().resource::<gta_sim::navigation::SidewalkGraph>();
    walks
        .edges()
        .iter()
        .flat_map(|&(i, j)| {
            let (a, b) = (walks.node(i), walks.node(j));
            let steps = a.distance(b).ceil().max(1.0) as u32;
            (0..=steps).map(move |k| a.lerp(b, k as f32 / steps as f32))
        })
        .min_by(|a, b| {
            let off = |p: Vec3| ((p - at).with_y(0.0).length() - distance).abs();
            off(*a).total_cmp(&off(*b))
        })
        .expect("GATE BROKEN: no sidewalk edge")
}
