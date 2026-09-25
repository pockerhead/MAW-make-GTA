//! Shared fixtures of the traffic gates (`traffic_*.rs`).
#![allow(dead_code)]

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
    let half = app.world().resource::<VehicleConfig>().half_extents().x;
    let graph = TrafficGraph::new(lanes, connectors, &cfg, half)
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
