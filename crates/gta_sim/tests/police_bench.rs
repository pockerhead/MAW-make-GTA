//! Frame budget at 5 stars: 12 SWAT units fighting the player in the seed-1 city with 40 civilians.
//! Prints tick times; asserts only an order-of-magnitude mean (per-tick max has OS spikes).

mod common;
mod police_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use police_support::*;
use std::time::{Duration, Instant};
use wanted_support::*;

/// Probe mean with 12 SWAT and 40 civilians in this city was 1.05 ms (runs 0.95-1.05); ten times that,
/// rounded up.
const MEAN_LIMIT: Duration = Duration::from_millis(11);
const OBSERVED_TICKS: u32 = 640;

#[test]
fn police_bench() {
    let mut app = city_app(1);
    settle(&mut app);
    set_population(&mut app, |p| p.max_civilians = 40);
    set_player_armor(&mut app, 1.0e6);
    let feet = position(&mut app) - Vec3::Y * float_height(&app);
    set_view(&mut app, Some(chase_view(feet, Vec3::NEG_Z)));
    let heat = wanted_cfg(&app).stars[4].heat;
    set_heat(&mut app, heat);
    let row = esc(&app).stars[4].clone();
    let mut waited = 0;
    while active(&mut app) != (row.units, row.swat) {
        waited += 1;
        assert!(
            waited <= 640,
            "GATE BROKEN: row 5 not full: {:?}",
            active(&mut app)
        );
        run_ticks(&mut app, 1);
    }
    let mut times = Vec::with_capacity(OBSERVED_TICKS as usize);
    for _ in 0..OBSERVED_TICKS {
        let start = Instant::now();
        run_ticks(&mut app, 1);
        times.push(start.elapsed());
    }
    let mean = times.iter().sum::<Duration>() / OBSERVED_TICKS;
    times.sort();
    let pick = |q: f32| times[((times.len() - 1) as f32 * q) as usize];
    println!(
        "12 SWAT + 40 civilians x {OBSERVED_TICKS} ticks (full after {waited}): mean {mean:?} p50 {:?} \
         p95 {:?} max {:?}, active {:?}",
        pick(0.5),
        pick(0.95),
        times.last().unwrap(),
        active(&mut app)
    );
    assert!(mean < MEAN_LIMIT, "mean tick {mean:?} >= {MEAN_LIMIT:?}");
}
