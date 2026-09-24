//! A kill in front of people on a seed-1 street gets a star: witnesses call, directly or after they
//! fled (GDD §6.2, §6.4; TASK-026). Production population and composition; no cop until the star.

mod common;
mod wanted_support;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    civilian::{Civilian, CivilianConfig, CivilianState},
    combat::Weapon,
    navigation::{GraphWalker, SidewalkGraph},
    perception::{PerceptionConfig, sight_blocked},
    police::PoliceUnit,
    population::NpcRng,
};
use std::collections::HashMap;
use wanted_support::*;

const RUNS: u64 = 100;
const NEEDED: usize = 85;
const STAR_WITHIN_S: f32 = 15.0;
const FILL_S: f32 = 6.0;
const MIN_OTHERS: usize = 3;
/// Player to victim along the victim's sidewalk, m.
const SHOT_RANGE: f32 = 6.0;

/// One kill: time to the first star (s) or `None`, the others within hearing of the victim, and
/// whether the first call came from a witness who had fled or crouched.
#[derive(Debug)]
struct Run {
    star_s: Option<f32>,
    others: usize,
    first_caller_fled: Option<bool>,
}

fn live_civilians(app: &mut App) -> Vec<(Entity, Vec3, GraphWalker)> {
    app.world_mut()
        .query::<(Entity, &Civilian, &Position, &GraphWalker)>()
        .iter(app.world())
        .filter(|(_, c, ..)| matches!(c.state, CivilianState::Wander | CivilianState::Idle { .. }))
        .map(|(e, _, p, w)| (e, p.0, *w))
        .collect()
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn blocked(app: &mut App, from: Vec3, to: Vec3) -> bool {
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| sight_blocked(&spatial, from, to))
        .expect("GATE BROKEN: ray system failed")
}

/// Victim with the fewest (at least `MIN_OTHERS`) calm civilians within `radius` and a clear shot from `SHOT_RANGE` m along
/// its sidewalk: `(victim, player chest, others within radius)`.
fn pick_victim(app: &mut App, radius: f32) -> Option<(Entity, Vec3, usize)> {
    let people = live_civilians(app);
    let mut ranked: Vec<_> = people
        .iter()
        .map(|&(e, at, walker)| {
            let others = people
                .iter()
                .filter(|&&(o, p, _)| o != e && p.distance(at) <= radius)
                .count();
            (others, e, at, walker)
        })
        .filter(|&(others, ..)| others >= MIN_OTHERS)
        .collect();
    // The sparsest qualifying street: a crowd would call under any rules.
    ranked.sort_by_key(|r| r.0);
    for (others, victim, at, walker) in ranked {
        let graph = app.world().resource::<SidewalkGraph>();
        let along = (graph.node(walker.to) - graph.node(walker.from))
            .with_y(0.0)
            .normalize_or_zero();
        for spot in [at + along * SHOT_RANGE, at - along * SHOT_RANGE] {
            let in_line = people
                .iter()
                .any(|&(o, p, _)| o != victim && segment_distance(p, spot, at) < 1.5);
            if !in_line && !blocked(app, spot, at) {
                return Some((victim, spot, others));
            }
        }
    }
    None
}

fn trial(seed: u64, old_rules: bool) -> Run {
    let mut app = city_app(1);
    settle(&mut app);
    if old_rules {
        // Named test mutation: the TASK-011 witness rules.
        let mut cfg = app.world_mut().resource_mut::<CivilianConfig>();
        cfg.call_after_flee = 0.0;
        cfg.reaction.report = 0.8;
    }
    *app.world_mut().resource_mut::<NpcRng>() = NpcRng::seeded(seed);
    arm(&mut app, Weapon::Pistol);
    let hearing = app.world().resource::<PerceptionConfig>().hearing_radius;
    let float = app.world().resource::<LocomotionConfig>().float_height;
    let start = position(&mut app) - Vec3::Y * float;
    let (dir, _) = open_street(&mut app, start);
    set_view(&mut app, Some(chase_view(start, dir)));
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let fill = ticks_in(&app, FILL_S);
    run_ticks(&mut app, fill);
    let (victim, chest, others) = pick_victim(&mut app, hearing)
        .unwrap_or_else(|| panic!("GATE BROKEN: seed {seed}: no victim with {MIN_OTHERS} others"));
    place_player(&mut app, chest);
    let to_victim = (position_of(&app, victim) - chest).with_y(0.0).normalize();
    set_view(
        &mut app,
        Some(chase_view(chest - Vec3::Y * float, to_victim)),
    );
    run_ticks(&mut app, 2);
    assert_eq!(count::<With<PoliceUnit>>(&mut app), 0, "GATE BROKEN: a cop");
    let mut probe = Probe::new(&app);
    kill_with_one_shot(&mut app, &mut probe, victim);
    let mut before: HashMap<Entity, CivilianState> = HashMap::new();
    let mut first_caller_fled = None;
    for tick in 1..=ticks_in(&app, STAR_WITHIN_S) {
        let states: Vec<(Entity, CivilianState)> = app
            .world_mut()
            .query::<(Entity, &Civilian)>()
            .iter(app.world())
            .map(|(e, c)| (e, c.state))
            .collect();
        for (e, state) in states {
            if !matches!(state, CivilianState::Report { .. }) {
                before.insert(e, state);
            }
        }
        probe.run(&mut app, 1);
        if first_caller_fled.is_none()
            && let Some(call) = probe.call_log.first()
        {
            first_caller_fled = Some(matches!(
                before.get(&call.caller),
                Some(CivilianState::Flee { .. } | CivilianState::Cower { .. })
            ));
        }
        if wanted(&app).stars >= 1 {
            assert!(
                !probe.call_log.is_empty(),
                "seed {seed}: a star without a civilian call"
            );
            return Run {
                star_s: Some(tick as f32 * step),
                others,
                first_caller_fled,
            };
        }
    }
    Run {
        star_s: None,
        others,
        first_caller_fled,
    }
}

/// Runs every seed on 4 threads, in seed order.
fn trials(old_rules: bool) -> Vec<Run> {
    let seeds: Vec<u64> = (0..RUNS).map(|k| 1000 + k).collect();
    let mut runs: Vec<(u64, Run)> = std::thread::scope(|scope| {
        let handles: Vec<_> = seeds
            .chunks(RUNS as usize / 4)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&s| (s, trial(s, old_rules)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a trial panicked"))
            .collect()
    });
    runs.sort_by_key(|(s, _)| *s);
    for (seed, run) in &runs {
        println!("seed {seed}: {run:?}");
    }
    runs.into_iter().map(|(_, run)| run).collect()
}

fn starred(runs: &[Run]) -> usize {
    runs.iter().filter(|r| r.star_s.is_some()).count()
}

#[test]
fn street_kill_gets_a_star() {
    let runs = trials(false);
    let mut times: Vec<f32> = runs.iter().filter_map(|r| r.star_s).collect();
    times.sort_by(f32::total_cmp);
    let fled = runs
        .iter()
        .filter(|r| r.first_caller_fled == Some(true))
        .count();
    let fewest = runs.iter().map(|r| r.others).min().unwrap_or(0);
    println!(
        "time to star, s (sorted): {times:?}; first call after a flight: {fled}; fewest others within hearing: {fewest}"
    );
    assert!(
        starred(&runs) >= NEEDED,
        "{} of {RUNS} kills got a star within {STAR_WITHIN_S} s",
        starred(&runs)
    );
}

/// Flip: the same kills under the old rules (`call_after_flee` 0, `report` 0.8).
#[test]
#[ignore = "flip-RED of street_kill_gets_a_star; run by hand"]
fn street_kill_under_old_rules_misses() {
    let runs = trials(true);
    assert!(
        starred(&runs) < NEEDED,
        "the old rules pass too: {} of {RUNS}",
        starred(&runs)
    );
}
