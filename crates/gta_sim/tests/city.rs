mod common;

use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use citygen::dist_point_segment;
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    flow::GameState,
    world::{
        City, CityBuilding, CityEdgeWall, CityGround, CityLayoutHash, CitySeed, RoadClass,
        WorldSource,
    },
};
use std::time::{Duration, Instant};

fn count<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), F>()
        .iter(app.world())
        .count()
}

#[test]
fn runtime_hash_matches_golden() {
    for seed in [1, 2] {
        let app = city_app(seed);
        assert_eq!(app.world().resource::<CitySeed>().0, seed);
        let hash = app.world().resource::<CityLayoutHash>().0;
        assert_eq!(
            hash,
            golden(seed),
            "seed {seed}: runtime layout hash {hash:#018x}"
        );
    }
}

#[test]
fn one_static_collider_per_building() {
    let mut app = city_app(1);
    let bodies = app
        .world_mut()
        .query_filtered::<(&RigidBody, &Collider, &Transform), With<CityBuilding>>()
        .iter(app.world())
        .map(|(body, collider, transform)| {
            let cuboid = collider
                .shape()
                .as_cuboid()
                .expect("building collider is a cuboid");
            let half = cuboid.half_extents;
            (*body, Vec3::new(half.x, half.y, half.z), *transform)
        })
        .collect::<Vec<_>>();
    let buildings = &app.world().resource::<City>().0.buildings;
    assert!(!buildings.is_empty());
    assert_eq!(bodies.len(), buildings.len());
    for b in buildings {
        let center = Vec3::new(b.center.x, b.height / 2.0, b.center.y);
        let (body, half, transform) = bodies
            .iter()
            .find(|(_, _, t)| (t.translation - center).length() < 1e-3)
            .unwrap_or_else(|| panic!("no collider at building center {center}"));
        assert!(body.is_static(), "building at {center} is {body:?}");
        let expected = Vec3::new(b.half_extents.x, b.height / 2.0, b.half_extents.y);
        assert!(
            (*half - expected).length() < 1e-3,
            "building at {center}: collider half extents {half}, expected {expected}"
        );
        let facade = transform.rotation * Vec3::X;
        assert!(
            (facade - Vec3::new(b.axis.x, 0.0, b.axis.y)).length() < 1e-3,
            "building at {center}: local X maps to {facade}, axis {}",
            b.axis
        );
    }
    assert_eq!(count::<With<CityGround>>(&mut app), 1);
}

#[test]
fn edge_walls_at_ground_edge() {
    let mut app = city_app(1);
    let g = app.world().resource::<City>().0.ground_size;
    let wall = &city_params(&app).edge_wall;
    let (h, t) = (wall.height, wall.thickness);
    let offset = g / 2.0 + t / 2.0;
    let expected = [
        Vec3::new(offset, h / 2.0, 0.0),
        Vec3::new(-offset, h / 2.0, 0.0),
        Vec3::new(0.0, h / 2.0, offset),
        Vec3::new(0.0, h / 2.0, -offset),
    ];
    let walls = app
        .world_mut()
        .query_filtered::<&Transform, With<CityEdgeWall>>()
        .iter(app.world())
        .map(|t| t.translation)
        .collect::<Vec<_>>();
    assert_eq!(walls.len(), 4, "edge walls: {walls:?}");
    for e in expected {
        assert!(
            walls.iter().any(|w| (*w - e).length() < 1e-3),
            "no wall at {e}: {walls:?}"
        );
    }
}

#[test]
fn edge_wall_stops_player() {
    let mut app = city_app(1);
    settle(&mut app);
    let g = app.world().resource::<City>().0.ground_size;
    let radius = app.world().resource::<LocomotionConfig>().capsule_radius;
    place_player(&mut app, Vec3::new(g / 2.0 - 5.0, 1.05, 0.0));
    set_intent(&mut app, |intent| {
        intent.axis = Vec2::X;
        intent.yaw = 0.0;
    });
    run_ticks(&mut app, 200);
    let at = position(&mut app);
    assert!(
        at.x < g / 2.0 - radius + 0.05,
        "player passed the edge wall: {at:?}"
    );
    assert!(at.y > 0.5, "player fell off the ground: {at:?}");
}

#[test]
fn player_spawns_on_sidewalk_and_stands() {
    let mut app = city_app(1);
    settle(&mut app);
    let at = position(&mut app);
    let p = Vec2::new(at.x, at.z);
    let city = &app.world().resource::<City>().0;
    let params = city_params(&app);
    let nearest = city
        .roads
        .edges
        .iter()
        .filter(|e| e.class != RoadClass::Alley)
        .map(|e| {
            let (a, b) = (
                city.roads.nodes[e.a as usize],
                city.roads.nodes[e.b as usize],
            );
            (dist_point_segment(p, a, b), e.class)
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .expect("city has streets");
    let (distance, class) = nearest;
    let half = params.half_carriageway(class);
    let sidewalk = params.sidewalk(class);
    assert!(
        distance >= half && distance <= half + sidewalk,
        "player at {at:?} is {distance} m from the nearest street axis, sidewalk is [{half}, {}]",
        half + sidewalk
    );
    for b in &city.buildings {
        let local = p - b.center;
        let (du, dv) = (local.dot(b.axis).abs(), local.dot(b.axis.perp()).abs());
        assert!(
            du > b.half_extents.x || dv > b.half_extents.y,
            "player at {at:?} is inside a building footprint"
        );
    }
}

// Run in QA: cargo test -p gta_sim --release --test city -- --ignored --nocapture
#[test]
#[ignore]
fn city_startup_budget() {
    let mut app = composed_app(WorldSource::City { seed: 1 });
    let start = Instant::now();
    let mut slowest = Duration::ZERO;
    while *app.world().resource::<State<GameState>>().get() != GameState::Playing {
        let frame = Instant::now();
        app.update();
        slowest = slowest.max(frame.elapsed());
        assert!(app.should_exit().is_none(), "city generation failed");
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "city generation did not finish"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let total = start.elapsed();
    println!("time to Playing: {total:?}; slowest update (apply frame): {slowest:?}");
    assert!(total < Duration::from_secs(2), "time to Playing {total:?}");
}
