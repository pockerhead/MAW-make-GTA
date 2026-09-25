//! Traffic graph over real cities (GDD §5.2, T15): built from citygen without an App for seeds 1..=8.
//! Correctness: inner lanes only, every lane leads on, every intersection is connected, and no car
//! following a connector clips a city block (a kinematic car does not collide with static geometry).

mod common;

use bevy::prelude::*;
use citygen::{RoadClass, convex_overlap, generate};
use common::assets_root;
use gta_sim::{
    config::load_config,
    traffic::{Segment, TRAFFIC_CONFIG, TrafficConfig, TrafficGraph},
    vehicle::{VEHICLE_CONFIG, VehicleConfig},
    world::{CITY_CONFIG, CityParams},
};

fn configs() -> (CityParams, TrafficConfig, VehicleConfig) {
    let root = assets_root();
    (
        load_config(&root, CITY_CONFIG).expect("GATE BROKEN: city.ron"),
        load_config(&root, TRAFFIC_CONFIG).expect("GATE BROKEN: traffic.ron"),
        load_config(&root, VEHICLE_CONFIG).expect("GATE BROKEN: sedan.ron"),
    )
}

/// Flat corners of a car centred at `p` heading along `tangent`.
fn footprint(p: Vec3, tangent: Vec3, half: Vec3) -> Vec<citygen::Vec2> {
    let f = Vec2::new(tangent.x, tangent.z).normalize() * half.z;
    let r = Vec2::new(-tangent.z, tangent.x).normalize() * half.x;
    let c = Vec2::new(p.x, p.z);
    [c + f + r, c + f - r, c - f - r, c - f + r]
        .into_iter()
        .map(|v| citygen::Vec2::new(v.x, v.y))
        .collect()
}

#[test]
fn city_graphs_are_sound() {
    let (params, cfg, vehicle) = configs();
    let half = vehicle.half_extents();
    let curb_lane = params.half_carriageway(RoadClass::Avenue) - params.parking.curb_offset;
    for seed in 1..=8u64 {
        let layout = generate(seed, &params).expect("GATE BROKEN: city generation");
        let graph = TrafficGraph::from_layout(&layout, &params, &cfg, half.x)
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        assert!(
            !graph.lanes().is_empty(),
            "GATE BROKEN: seed {seed} has no lanes"
        );
        let roads = &layout.roads;
        for (k, lane) in graph.lanes().iter().enumerate() {
            assert!(!lane.out.is_empty(), "seed {seed}: lane {k} is a dead end");
            // Lateral offset to the nearest road centre line through its end node.
            let offset = roads
                .edges
                .iter()
                .filter(|e| e.class != RoadClass::Alley)
                .filter_map(|e| {
                    let (a, b) = (roads.nodes[e.a as usize], roads.nodes[e.b as usize]);
                    let d = (b - a).normalize();
                    let from = citygen::Vec2::new(lane.from.x, lane.from.z);
                    let to = citygen::Vec2::new(lane.to.x, lane.to.z);
                    let along = (to - from).normalize();
                    let parallel = d.dot(along).abs() > 0.999;
                    let off = d.perp_dot(from - a).abs();
                    let inside =
                        (from - a).dot(d) > -1.0 && (from - a).dot(d) < (b - a).length() + 1.0;
                    (parallel && inside).then_some(off)
                })
                .fold(f32::INFINITY, f32::min);
            assert!(
                (offset - curb_lane).abs() > 0.5,
                "seed {seed}: lane {k} runs on the curb lane (offset {offset})"
            );
        }
        // Every intersection with at least two lane roads has connectors.
        let mut degree = vec![0u32; roads.nodes.len()];
        for e in roads.edges.iter().filter(|e| e.class != RoadClass::Alley) {
            degree[e.a as usize] += 1;
            degree[e.b as usize] += 1;
        }
        for (node, &d) in degree.iter().enumerate() {
            if d < 2 {
                continue;
            }
            assert!(
                graph.connectors().iter().any(|c| c.node == node as u32),
                "seed {seed}: intersection {node} with {d} roads has no connector"
            );
        }
        // A car following any connector stays off every block (curb prism).
        for (k, conn) in graph.connectors().iter().enumerate() {
            let seg = Segment::Connector(k as u32);
            let start = citygen::Vec2::new(conn.points[0].x, conn.points[0].z);
            let near: Vec<(usize, &citygen::Block)> = layout
                .blocks
                .iter()
                .enumerate()
                .filter(|(_, b)| b.curb.len() >= 3)
                .filter(|(_, b)| {
                    (0..b.curb.len()).any(|i| {
                        citygen::dist_point_segment(
                            start,
                            b.curb[i],
                            b.curb[(i + 1) % b.curb.len()],
                        ) < 40.0
                    })
                })
                .collect();
            let steps = (conn.length / 0.25).ceil() as u32;
            for i in 0..=steps {
                let s = conn.length * i as f32 / steps as f32;
                let (p, tangent) = graph.pose(seg, s);
                let car = footprint(p, tangent, half);
                for &(b, block) in &near {
                    let depth = convex_overlap(&car, &block.curb);
                    assert!(
                        depth <= 0.0,
                        "seed {seed}: connector {k} at s {s:.2} ({p}) clips block {b} by {depth:.3} m"
                    );
                }
            }
        }
    }
}
