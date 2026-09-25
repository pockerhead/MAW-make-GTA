//! AC1 (correctness, GDD §5.2): IDM on a closed loop of 10 kinematic cars never gives a negative
//! speed, never lets two chassis overlap and keeps every car on its path heading, 8 seeds x 6400 ticks.

mod common;
mod traffic_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::combat::aim_yaw;
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};
use traffic_support::*;

const CARS: usize = 10;
const TICKS: u32 = 6400;

fn wrap(a: f32) -> f32 {
    let a = (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
    a - std::f32::consts::PI
}

/// Bumper gaps of one seed in [1, 8] m, the first one exactly 1.0 m (below s0: IDM brakes at rest).
fn gaps(seed: u64) -> Vec<f32> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut gaps: Vec<f32> = (0..CARS)
        .map(|_| 1.0 + 7.0 * (rng.next_u32() >> 8) as f32 / 16_777_216.0)
        .collect();
    gaps[0] = 1.0;
    gaps
}

fn run_seed(seed: u64) {
    let (lanes, connectors) = loop_lanes(12.0);
    let mut app = traffic_floor(lanes, &connectors, &[]);
    let graph = graph(&app);
    let path = loop_path(&graph);
    let total: f32 = path.iter().map(|&s| graph.length(s)).sum();
    let length = 2.0
        * app
            .world()
            .resource::<gta_sim::vehicle::VehicleConfig>()
            .half_extents()
            .z;
    let mut at = 0.0;
    let mut placed = Vec::new();
    for gap in gaps(seed) {
        let (seg, s) = place_on(&graph, &path, at);
        placed.push(spawn_traffic_car(&mut app, seg, s, 0.0));
        at += length + gap;
    }
    assert!(
        at < total,
        "GATE BROKEN: {CARS} cars do not fit the {total} m loop"
    );
    let before = stats(&app).casts;
    let mut travelled = [0.0_f32; CARS];
    let mut last: Vec<Vec3> = placed.iter().map(|&c| position_of(&app, c)).collect();
    for tick in 1..=TICKS {
        run_ticks(&mut app, 1);
        if tick == 64 {
            assert_traffic_ran(&app, before, CARS as u32, 64);
        }
        let now = cars(&mut app);
        assert_eq!(
            now.len(),
            CARS,
            "seed {seed} tick {tick}: car count changed"
        );
        let rects: Vec<_> = placed.iter().map(|&c| footprint(&app, c)).collect();
        for (k, &car) in placed.iter().enumerate() {
            let t = traffic_car(&app, car);
            assert!(
                t.speed >= 0.0,
                "seed {seed} tick {tick}: car {k} speed {}",
                t.speed
            );
            let (_, tangent) = graph.pose(t.segment, t.s);
            let velocity = app.world().get::<LinearVelocity>(car).unwrap().0;
            assert!(
                velocity.dot(tangent) >= -1e-4,
                "seed {seed} tick {tick}: car {k} backs up at {velocity}"
            );
            let forward = app.world().get::<Rotation>(car).unwrap().0 * Vec3::NEG_Z;
            let off = wrap(aim_yaw(forward) - aim_yaw(tangent)).abs().to_degrees();
            assert!(
                off < 2.0,
                "seed {seed} tick {tick}: car {k} heading off by {off} deg"
            );
            let p = position_of(&app, car);
            travelled[k] += (p - last[k]).with_y(0.0).length();
            last[k] = p;
            for (j, other) in rects.iter().enumerate().skip(k + 1) {
                assert!(
                    !obb_overlap(&rects[k], other),
                    "seed {seed} tick {tick}: cars {k} and {j} overlap"
                );
            }
        }
    }
    for (k, d) in travelled.iter().enumerate() {
        assert!(
            *d >= total,
            "GATE BROKEN: seed {seed}: car {k} travelled {d} m < one loop {total} m"
        );
    }
    eprintln!(
        "seed {seed}: loop {total:.1} m, travelled min {:.0} m max {:.0} m",
        travelled.iter().copied().fold(f32::INFINITY, f32::min),
        travelled.iter().copied().fold(0.0, f32::max)
    );
}

#[test]
fn idm_loop_never_reverses_or_overlaps() {
    for seed in 1..=8 {
        run_seed(seed);
    }
}
