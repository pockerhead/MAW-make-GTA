mod common;

use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use citygen::dist_point_segment;
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    flow::GameState,
    world::{
        City, CityBlock, CityBuilding, CityEdgeWall, CityGround, CityLandmarks, CityLayoutHash,
        CitySeed, RoadClass, WorldSource, centroid,
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
        .map(|(body, collider, transform)| (*body, collider.clone(), *transform))
        .collect::<Vec<_>>();
    let buildings = &app.world().resource::<City>().0.buildings;
    assert!(!buildings.is_empty());
    assert_eq!(bodies.len(), buildings.len());
    for b in buildings {
        let center = Vec3::new(b.center.x, b.height / 2.0, b.center.y);
        let (body, collider, transform) = bodies
            .iter()
            .find(|(_, _, t)| (t.translation - center).length() < 1e-3)
            .unwrap_or_else(|| panic!("no collider at building center {center}"));
        assert!(body.is_static(), "building at {center} is {body:?}");
        check_building_shape(b, collider, center);
        let facade = transform.rotation * Vec3::X;
        assert!(
            (facade - Vec3::new(b.axis.x, 0.0, b.axis.y)).length() < 1e-3,
            "building at {center}: local X maps to {facade}, axis {}",
            b.axis
        );
    }
    assert_eq!(count::<With<CityGround>>(&mut app), 1);
}

/// Expected (half extents, local centre y) of the base and every setback tier.
fn expected_slabs(b: &citygen::Building) -> Vec<(Vec3, f32)> {
    let mut bottoms = vec![(0.0, b.half_extents)];
    bottoms.extend(b.upper_tiers.iter().map(|t| (t.bottom, t.half_extents)));
    (0..bottoms.len())
        .map(|k| {
            let (bottom, half) = bottoms[k];
            let top = bottoms.get(k + 1).map_or(b.height, |n| n.0);
            (
                Vec3::new(half.x, (top - bottom) / 2.0, half.y),
                (bottom + top) / 2.0 - b.height / 2.0,
            )
        })
        .collect()
}

fn check_building_shape(b: &citygen::Building, collider: &Collider, center: Vec3) {
    let expected = expected_slabs(b);
    let parts =
        if b.upper_tiers.is_empty() {
            let cuboid = collider.shape().as_cuboid().unwrap_or_else(|| {
                panic!("building at {center}: plain box collider is not a cuboid")
            });
            let h = cuboid.half_extents;
            vec![(Vec3::new(h.x, h.y, h.z), 0.0)]
        } else {
            let compound = collider.shape().as_compound().unwrap_or_else(|| {
                panic!("building at {center}: tiered collider is not a compound")
            });
            compound
                .shapes()
                .iter()
                .map(|(pose, shape)| {
                    let h = shape
                        .as_cuboid()
                        .unwrap_or_else(|| {
                            panic!("building at {center}: compound part is not a cuboid")
                        })
                        .half_extents;
                    (Vec3::new(h.x, h.y, h.z), pose.translation.y)
                })
                .collect()
        };
    assert_eq!(
        parts.len(),
        expected.len(),
        "building at {center}: {} collider parts, expected {} (1 + tiers)",
        parts.len(),
        expected.len()
    );
    for (k, ((half, y), (want_half, want_y))) in parts.iter().zip(&expected).enumerate() {
        assert!(
            (*half - *want_half).length() < 1e-3 && (y - want_y).abs() < 1e-3,
            "building at {center} part {k}: half {half} y {y}, expected {want_half} y {want_y}"
        );
    }
}

#[test]
fn landmarks_resource_matches_layout() {
    let mut app = city_app(1);
    let blocks = count::<With<CityBlock>>(&mut app);
    let lm = app.world().resource::<CityLandmarks>();
    let city = &app.world().resource::<City>().0;
    let curb = city_params(&app).roads.curb_height;
    let tallest = city.buildings.iter().map(|b| b.height).fold(0.0, f32::max);
    let tower = &city.buildings[city.landmarks.tower as usize];
    assert!(
        (lm.tower_roof.y - tallest).abs() < 1e-4,
        "tower roof {} is not the tallest height {tallest}",
        lm.tower_roof
    );
    assert!(
        (Vec2::new(lm.tower_roof.x, lm.tower_roof.z) - tower.center).length() < 1e-4,
        "tower roof {} not over the tower centre {}",
        lm.tower_roof,
        tower.center
    );
    let park = centroid(&city.blocks[city.landmarks.park as usize].inner);
    assert!(
        (lm.park_center - Vec3::new(park.x, curb, park.y)).length() < 1e-4,
        "park centre {} expected {park} at y {curb}",
        lm.park_center
    );
    let expected_blocks = city.blocks.iter().filter(|b| b.curb.len() >= 3).count();
    assert!(expected_blocks > 0);
    assert_eq!(
        blocks, expected_blocks,
        "one CityBlock per block with a curb"
    );
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
