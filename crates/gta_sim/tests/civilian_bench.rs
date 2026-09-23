//! Stress bench: 64 Tnua civilians in the seed-1 city for 640 fixed ticks. Prints tick times; asserts
//! only an order-of-magnitude mean and bounded perception work per tick.

mod common;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{AimIntent, Dead, Health},
    civilian::{Civilian, CivilianState},
    combat::{Loadout, ShotFired, Weapon},
    navigation::{GraphWalker, SidewalkGraph, flat_distance},
    perception::{PerceptionConfig, PerceptionLoad},
    population::ViewCone,
};
use std::time::{Duration, Instant};

/// Probe mean with 64 Tnua walkers in this city was 0.756 ms; ten times that, rounded up.
const MEAN_LIMIT: Duration = Duration::from_millis(8);
const OBSERVED_TICKS: u32 = 640;

struct Report {
    mean: Duration,
    p50: Duration,
    p95: Duration,
    max: Duration,
    alive: (usize, usize),
    max_agents: u32,
    max_rays: u32,
}

fn alive(app: &mut App) -> usize {
    app.world_mut()
        .query::<&Civilian>()
        .iter(app.world())
        .filter(|c| c.state != CivilianState::Dead)
        .count()
}

fn aimers(app: &mut App) -> u32 {
    app.world_mut()
        .query_filtered::<(&AimIntent, &Loadout), Without<Dead>>()
        .iter(app.world())
        .filter(|(aim, loadout)| aim.aiming && loadout.held.is_some())
        .count() as u32
}

fn bench(count: usize) -> Report {
    let mut app = city_app(1);
    set_population(&mut app, |p| p.max_civilians = count as u32);
    let slots = u32::from(app.world().resource::<PerceptionConfig>().slots);
    let player_feet = position(&mut app);
    let eye = player_feet + Vec3::Y;
    set_view(
        &mut app,
        Some(ViewCone::from_perspective(
            eye,
            Dir3::Y,
            70f32.to_radians(),
            16.0 / 9.0,
        )),
    );
    let mut placed: Vec<(GraphWalker, f32, Vec3)> = Vec::new();
    {
        let graph = app.world().resource::<SidewalkGraph>();
        'edges: for &(a, b) in graph.edges() {
            for t in [0.1, 0.3, 0.5, 0.7, 0.9] {
                let at = graph.node(a).lerp(graph.node(b), t);
                if flat_distance(at, player_feet) > 100.0
                    || placed.iter().any(|(_, _, p)| flat_distance(*p, at) < 3.0)
                {
                    continue;
                }
                placed.push((GraphWalker { from: a, to: b }, t, at));
                if placed.len() == count {
                    break 'edges;
                }
            }
        }
    }
    assert_eq!(
        placed.len(),
        count,
        "GATE BROKEN: not enough room within 100 m"
    );
    let civilians = placed
        .iter()
        .map(|&(walker, t, _)| spawn_civilian(&mut app, walker, t, calm()))
        .collect::<Vec<_>>();
    for &victim in &civilians[..4] {
        app.world_mut().get_mut::<Health>(victim).unwrap().current = 0.0;
    }
    run_ticks(&mut app, 64);
    let shooter = player(&mut app);
    let mut times = Vec::with_capacity(OBSERVED_TICKS as usize);
    let (mut min_alive, mut max_alive) = (usize::MAX, 0);
    let (mut max_agents, mut max_rays) = (0, 0);
    for tick in 0..OBSERVED_TICKS {
        if tick % 64 == 0 {
            let target = civilians[4 + (tick / 64) as usize % (count - 4)];
            if let Some(at) = app.world().get::<avian3d::prelude::Position>(target) {
                let muzzle = at.0;
                app.world_mut().write_message(ShotFired {
                    shooter,
                    weapon: Weapon::Pistol,
                    muzzle,
                });
            }
        }
        let before = alive(&mut app) as u32;
        let aim = aimers(&mut app);
        let start = Instant::now();
        run_ticks(&mut app, 1);
        times.push(start.elapsed());
        let now = alive(&mut app);
        min_alive = min_alive.min(now);
        max_alive = max_alive.max(now);
        let load = *app.world().resource::<PerceptionLoad>();
        let bound = before.max(now as u32).div_ceil(slots) + 1;
        assert!(
            load.agents <= bound,
            "tick {tick}: {} agents perceived, bound {bound}",
            load.agents
        );
        assert!(
            load.rays <= load.agents * (1 + aim),
            "tick {tick}: {} rays for {} agents",
            load.rays,
            load.agents
        );
        max_agents = max_agents.max(load.agents);
        max_rays = max_rays.max(load.rays);
    }
    let mean = times.iter().sum::<Duration>() / OBSERVED_TICKS;
    times.sort();
    let pick = |q: f32| times[((times.len() - 1) as f32 * q) as usize];
    Report {
        mean,
        p50: pick(0.5),
        p95: pick(0.95),
        max: *times.last().unwrap(),
        alive: (min_alive, max_alive),
        max_agents,
        max_rays,
    }
}

#[test]
fn civilian_bench() {
    let full = bench(64);
    println!(
        "64 civilians x {OBSERVED_TICKS} ticks: mean {:?} p50 {:?} p95 {:?} max {:?}, alive {:?}, \
         max perception agents {} rays {}",
        full.mean, full.p50, full.p95, full.max, full.alive, full.max_agents, full.max_rays
    );
    let half = bench(32);
    println!(
        "32 civilians: mean {:?} (64: {:?}), comparison only",
        half.mean, full.mean
    );
    assert!(
        full.mean < MEAN_LIMIT,
        "mean tick {:?} >= {MEAN_LIMIT:?}",
        full.mean
    );
    assert!(full.alive.0 > 0, "no civilian alive during the bench");
}
