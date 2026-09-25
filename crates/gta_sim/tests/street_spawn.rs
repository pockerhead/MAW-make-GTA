//! Spawning where the player cannot see it, ahead of the view first; seed-1 city (GDD §6.1).

mod common;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use common::*;
use gta_sim::{
    character::{Gait, Health, LocomotionConfig},
    civilian::{Civilian, CivilianState},
    navigation::{SidewalkGraph, flat_distance},
    perception::sight_blocked,
    population::{PopulationConfig, PopulationLoad, PopulationPhase, ViewCone, outside_cone},
};

const TICK_HZ: u32 = 64;

fn population_cfg(app: &App) -> PopulationConfig {
    app.world().resource::<PopulationConfig>().clone()
}

fn loco(app: &App) -> LocomotionConfig {
    app.world().resource::<LocomotionConfig>().clone()
}

fn feet(app: &mut App) -> Vec3 {
    position(app) - Vec3::Y * loco(app).float_height
}

fn blocked(app: &mut App, from: Vec3, to: Vec3) -> bool {
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| sight_blocked(&spatial, from, to))
        .expect("GATE BROKEN: ray system failed")
}

/// The top of the body's width at `feet` seen from `origin`: (left, centre, right).
fn head_line(app: &App, origin: Vec3, feet: Vec3) -> [Vec3; 3] {
    let head = feet + Vec3::Y * population_cfg(app).occlusion_ray_height;
    let side = (feet - origin).with_y(0.0).cross(Vec3::Y).normalize() * loco(app).capsule_radius;
    [head + side, head, head - side]
}

/// The centre head ray and the feet ray from `origin` to a body at `feet` hit world geometry.
fn centre_hidden(app: &mut App, origin: Vec3, feet: Vec3) -> bool {
    let [_, head, _] = head_line(app, origin, feet);
    blocked(app, origin, head) && blocked(app, origin, feet + Vec3::Y * 0.1)
}

/// The centre rays and both side head rays (collider half-width) hit world geometry.
fn hidden_behind_geometry(app: &mut App, origin: Vec3, feet: Vec3) -> bool {
    let [left, _, right] = head_line(app, origin, feet);
    centre_hidden(app, origin, feet) && blocked(app, origin, left) && blocked(app, origin, right)
}

/// Alive civilians: (entity, chest position).
fn alive(app: &mut App) -> Vec<(Entity, Vec3)> {
    app.world_mut()
        .query::<(Entity, &Civilian, &Position)>()
        .iter(app.world())
        .filter(|(_, c, _)| c.state != CivilianState::Dead)
        .map(|(e, _, p)| (e, p.0))
        .collect()
}

/// The QA probe's measure (`qa_street_life.py`): alive civilians inside the cone within `range` m
/// of the camera, occluded or not.
fn in_view(app: &mut App, view: &ViewCone, range: f32) -> usize {
    alive(app)
        .into_iter()
        .filter(|&(_, p)| view.contains(p, 0.0) && p.distance(view.origin) <= range)
        .count()
}

/// Horizontal distance from `p` to the nearest sidewalk edge.
fn off_graph(app: &App, p: Vec3) -> f32 {
    let graph = app
        .world()
        .get_resource::<SidewalkGraph>()
        .expect("GATE BROKEN: no SidewalkGraph");
    graph
        .edges()
        .iter()
        .map(|&(a, b)| {
            let (p, a, b) = (p.xz(), graph.node(a).xz(), graph.node(b).xz());
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        })
        .fold(f32::INFINITY, f32::min)
}

/// Runs one fixed tick under `view`; returns the feet of every civilian that appeared in it.
fn tick_spawns(app: &mut App, view: ViewCone, known: &mut Vec<Entity>) -> Vec<Vec3> {
    set_view(app, Some(view));
    run_ticks(app, 1);
    let float = loco(app).float_height;
    let mut fresh = Vec::new();
    for e in civilians(app) {
        if known.contains(&e) {
            continue;
        }
        known.push(e);
        let feet = position_of(app, e) - Vec3::Y * float;
        let off = off_graph(app, feet);
        assert!(off <= 1e-3, "{e} spawned {off} m off the sidewalk graph");
        fresh.push(feet);
    }
    fresh
}

#[test]
fn occluded_in_cone_accepted_clear_in_cone_rejected() {
    let mut app = city_app(1);
    *app.world_mut().resource_mut::<PopulationPhase>() = PopulationPhase::Steady;
    // Random order: this gate is about the occlusion rule, the forward order has its own gate.
    set_population(&mut app, |p| p.spawn_forward_weight = 0.0);
    let cfg = population_cfg(&app);
    let (inner, outer) = cfg.spawn_ring;
    let player_feet = feet(&mut app);
    // A cone that contains every point: nothing is off-frame, only a building can hide a spawn.
    let origin = player_feet + Vec3::Y * 1.55 + Vec3::Z * 3.8;
    let view = ViewCone {
        origin,
        forward: Vec3::NEG_Z,
        half_angle: std::f32::consts::PI,
    };
    let ring = {
        let graph = app.world().resource::<SidewalkGraph>();
        (0..graph.nodes().len() as u32)
            .filter(|&n| !graph.neighbors(n).is_empty())
            .map(|n| graph.node(n))
            .filter(|&p| (inner..=outer).contains(&flat_distance(p, player_feet)))
            .collect::<Vec<_>>()
    };
    let (mut occluded, mut clear) = (0, 0);
    for p in ring {
        if hidden_behind_geometry(&mut app, origin, p) {
            occluded += 1;
        } else {
            clear += 1;
        }
    }
    println!("ring nodes in the cone: {occluded} behind buildings, {clear} in clear view");
    assert!(
        occluded > 0 && clear > 0,
        "GATE BROKEN: the ring needs both occluded and clear nodes"
    );
    let mut known = civilians(&mut app);
    let mut spawned = Vec::new();
    for _ in 0..64 {
        spawned.extend(tick_spawns(&mut app, view, &mut known));
    }
    assert!(
        !spawned.is_empty(),
        "nothing spawned behind a building inside the cone"
    );
    for &p in &spawned {
        let d = flat_distance(p, player_feet);
        assert!(
            (inner..=outer).contains(&d),
            "spawned at {d} m, off the ring"
        );
        assert!(
            hidden_behind_geometry(&mut app, origin, p),
            "spawned in clear view inside the cone at {p}"
        );
    }
    println!("{} spawns, every one behind a building", spawned.len());
}

#[test]
fn corner_peek_in_cone_rejected() {
    let mut app = city_app(1);
    *app.world_mut().resource_mut::<PopulationPhase>() = PopulationPhase::Steady;
    let cfg = population_cfg(&app);
    let (inner, outer) = cfg.spawn_ring;
    let apart = 2.0 * cfg.spawn_min_separation;
    let player_feet = feet(&mut app);
    let taken = alive(&mut app)
        .into_iter()
        .map(|(_, p)| p)
        .collect::<Vec<_>>();
    // The spawner's points: nodes and every `spawn_point_spacing` m inside each edge.
    let ring = {
        let graph = app.world().resource::<SidewalkGraph>();
        let spacing = cfg.spawn_point_spacing;
        let nodes = (0..graph.nodes().len() as u32)
            .filter(|&n| !graph.neighbors(n).is_empty())
            .map(|n| graph.node(n));
        let inner_points = graph.edges().iter().flat_map(|&(a, b)| {
            let (pa, pb) = (graph.node(a), graph.node(b));
            let length = flat_distance(pa, pb);
            let count = (length / spacing - 0.5).floor().max(0.0) as u32;
            (1..=count).map(move |k| pa.lerp(pb, k as f32 * spacing / length))
        });
        nodes
            .chain(inner_points)
            .filter(|&p| (inner + 1.0..=outer - 1.0).contains(&flat_distance(p, player_feet)))
            .filter(|&p| taken.iter().all(|&t| flat_distance(t, p) >= apart))
            .collect::<Vec<_>>()
    };
    // Real sidewalk points, seen from the chase camera around the player: hidden at the centre with
    // a side of the body in clear view, or hidden whole.
    let (mut peeks, mut hidden) = (Vec::new(), Vec::new());
    for deg in (0..360).step_by(10) {
        let yaw = (deg as f32).to_radians();
        let origin = chase_view(player_feet, Vec3::new(-yaw.sin(), 0.0, -yaw.cos())).origin;
        for &p in &ring {
            if !centre_hidden(&mut app, origin, p) {
                continue;
            }
            if hidden_behind_geometry(&mut app, origin, p) {
                hidden.push((origin, p));
            } else {
                peeks.push((origin, p));
            }
        }
    }
    println!(
        "ring points hidden at the centre over 36 camera positions: {} whole body, {} with a side in clear view",
        hidden.len(),
        peeks.len()
    );
    let Some(&(origin, peek)) = peeks.first() else {
        panic!("GATE BROKEN: no ring point with only a side of the body in clear view");
    };
    let Some(&(_, whole)) = hidden
        .iter()
        .find(|&&(o, p)| o == origin && flat_distance(p, peek) >= apart)
    else {
        panic!("GATE BROKEN: no fully hidden ring point away from {peek}");
    };
    // The node straight ahead on a thin ring through it is the first candidate of the order.
    set_population(&mut app, |p| p.spawn_forward_weight = 1.0e6);
    let spawns_at = |app: &mut App, node: Vec3| {
        let d = flat_distance(node, player_feet);
        set_population(app, |p| p.spawn_ring = (d - 0.25, d + 0.25));
        let view = ViewCone {
            origin,
            forward: (node + Vec3::Y - origin).normalize(),
            half_angle: 10f32.to_radians(),
        };
        let mut known = civilians(app);
        tick_spawns(app, view, &mut known)
            .iter()
            .any(|&f| flat_distance(f, node) < 0.5)
    };
    assert!(
        !spawns_at(&mut app, peek),
        "spawned at a corner with a side of the body in clear view: {peek}"
    );
    assert!(
        spawns_at(&mut app, whole),
        "GATE BROKEN: nobody spawned at the fully hidden point {whole}"
    );
}

#[test]
fn standing_still_does_not_churn() {
    // Standing through the fill: recycling takes calm civilians from behind, a budget per tick.
    const STAND_S: u32 = 20;
    let mut app = city_app(1);
    settle(&mut app);
    let cfg = population_cfg(&app);
    let start = feet(&mut app);
    let (dir, _) = open_street(&mut app, start);
    let mut prev = alive(&mut app);
    let mut spawns = [0; (STAND_S / 10) as usize];
    let mut max_recycled = 0;
    for tick in 0..STAND_S * TICK_HZ {
        let player = position(&mut app);
        let view = chase_view(feet(&mut app), dir);
        set_view(&mut app, Some(view));
        run_ticks(&mut app, 1);
        let present = civilians(&mut app);
        let recycled = prev
            .iter()
            .filter(|&&(e, p)| {
                !present.contains(&e) && flat_distance(p, player) <= cfg.despawn_distance
            })
            .count();
        max_recycled = max_recycled.max(recycled);
        spawns[(tick / (10 * TICK_HZ)) as usize] += present
            .iter()
            .filter(|&&e| prev.iter().all(|&(p, _)| p != e))
            .count();
        prev = alive(&mut app);
    }
    println!(
        "standing: spawns per 10 s {spawns:?}, max {max_recycled} recycled in one tick (budget {})",
        cfg.recycles_per_tick
    );
    assert!(
        spawns[0] >= cfg.max_civilians as usize,
        "GATE BROKEN: the initial fill spawned only {}",
        spawns[0]
    );
    assert!(
        max_recycled <= cfg.recycles_per_tick as usize,
        "{max_recycled} recycled in one tick, budget {}",
        cfg.recycles_per_tick
    );
    assert!(
        spawns[1] <= (cfg.max_civilians / 2) as usize,
        "standing still churns: {} spawns in the second 10 s",
        spawns[1]
    );
}

#[test]
fn no_spawn_in_clear_view_over_60s_turning() {
    let mut app = city_app(1);
    // The subject is the civilian spawner: a traffic car nudging a civilian spawned on a crosswalk in
    // its first tick moved it 0.067 m off the graph (CI, TASK-016).
    app.world_mut()
        .resource_mut::<gta_sim::traffic::TrafficConfig>()
        .bubble
        .max_cars = 0;
    let cfg = population_cfg(&app);
    let head = loco(&app).head_height;
    let margin = cfg.spawn_view_margin_deg.to_radians();
    let mut known = Vec::new();
    let (mut spawns, mut in_cone, mut max_rays) = (0, 0, 0);
    for tick in 0..60 * TICK_HZ {
        // A full turn every 8 s; a civilian dies every second so the spawner keeps working.
        let yaw = tick as f32 / (8 * TICK_HZ) as f32 * std::f32::consts::TAU;
        let view = chase_view(feet(&mut app), Vec3::new(-yaw.sin(), 0.0, -yaw.cos()));
        if tick % TICK_HZ == TICK_HZ - 1
            && let Some(&(victim, _)) = alive(&mut app).first()
        {
            app.world_mut().get_mut::<Health>(victim).unwrap().current = 0.0;
        }
        let fresh = tick_spawns(&mut app, view, &mut known);
        let rays = app.world().resource::<PopulationLoad>().rays;
        assert!(
            rays <= cfg.occlusion_rays_per_tick,
            "tick {tick}: {rays} occlusion rays"
        );
        max_rays = max_rays.max(rays);
        for p in fresh {
            spawns += 1;
            if outside_cone(&view, p, head, margin) {
                continue;
            }
            in_cone += 1;
            assert!(
                hidden_behind_geometry(&mut app, view.origin, p),
                "tick {tick}: spawned in the cone in clear view at {p}"
            );
        }
    }
    println!(
        "{spawns} spawns in 60 s, {in_cone} inside the cone behind buildings; \
         max {max_rays} occlusion rays per tick (budget {})",
        cfg.occlusion_rays_per_tick
    );
    assert!(spawns > 60, "GATE BROKEN: only {spawns} spawns");
    assert!(
        in_cone > 0,
        "GATE BROKEN: no in-cone spawn exercised the ray"
    );
}

#[test]
fn street_ahead_stays_populated() {
    // Standing through the initial fill, then running along the street; recycling keeps the cap moving ahead.
    const START_S: u32 = 15;
    const WALK_S: u32 = 60;
    const WINDOW_S: usize = 20;
    const SAMPLES_PER_S: u32 = 4;
    const RANGE: f32 = 60.0;
    let mut app = city_app(1);
    settle(&mut app);
    // The subject is the civilian bubble: a traffic car queued across the run stopped the player
    // at 130 m of 270 (TASK-016).
    app.world_mut()
        .resource_mut::<gta_sim::traffic::TrafficConfig>()
        .bubble
        .max_cars = 0;
    let speed = loco(&app).run_speed;
    let start = feet(&mut app);
    let (dir, length) = open_street(&mut app, start);
    let needed = speed * WALK_S as f32 * 1.2;
    assert!(
        length >= needed,
        "GATE BROKEN: longest open street from the spawn is {length} m, need {needed}"
    );
    let mut samples = Vec::new();
    let mut per_second = Vec::new();
    let mut known = civilians(&mut app);
    let (mut run_spawns, mut ahead) = (0, 0);
    for tick in 0..(START_S + WALK_S) * TICK_HZ {
        if tick == START_S * TICK_HZ {
            set_intent(&mut app, |i| {
                i.axis = Vec2::Y;
                i.yaw = f32::atan2(-dir.x, -dir.z);
                i.gait = Gait::Run;
            });
        }
        let player = feet(&mut app);
        let view = chase_view(player, dir);
        let fresh = tick_spawns(&mut app, view, &mut known);
        if tick >= START_S * TICK_HZ {
            run_spawns += fresh.len();
            ahead += fresh
                .iter()
                .filter(|&&f| (f - player).with_y(0.0).dot(dir) > 0.0)
                .count();
        }
        if tick % TICK_HZ == 0 {
            per_second.push(in_view(&mut app, &view, RANGE));
        }
        if tick >= START_S * TICK_HZ && tick % (TICK_HZ / SAMPLES_PER_S) == 0 {
            samples.push(in_view(&mut app, &view, RANGE));
        }
    }
    let travelled = flat_distance(feet(&mut app), start);
    let expected = speed * WALK_S as f32;
    assert!(
        travelled >= 0.8 * expected,
        "GATE BROKEN: the player ran only {travelled} m of {expected}"
    );
    let mean_of = |s: &[usize]| s.iter().sum::<usize>() as f32 / s.len() as f32;
    let mean = mean_of(&samples);
    let window = WINDOW_S * SAMPLES_PER_S as usize;
    let thirds = samples.chunks(window).map(mean_of).collect::<Vec<_>>();
    let sliding_min = samples
        .windows(window)
        .map(mean_of)
        .fold(f32::INFINITY, f32::min);
    let ahead_share = ahead as f32 / run_spawns.max(1) as f32;
    println!("in view within {RANGE} m, each second from t=0: {per_second:?}");
    println!("run spawns: {run_spawns}, {ahead} ahead of the player ({ahead_share:.2})");
    println!(
        "street life: {WALK_S} s run from t={START_S} s, mean {mean:.2} civilians in view within \
         {RANGE} m; 20 s windows {thirds:.2?}, sliding 20 s min {sliding_min:.2}; \
         {travelled:.0} m along {dir}"
    );
    // Random order measured 0.39 ahead (spawn_forward_weight 0), the shipped order 1.00.
    assert!(
        run_spawns >= 20,
        "GATE BROKEN: only {run_spawns} spawns during the run"
    );
    assert!(
        ahead_share >= 0.75,
        "only {ahead_share:.2} of the run spawns ahead of the player: no forward preference"
    );
    assert!(
        sliding_min >= 4.0,
        "a 20 s window has mean in view {sliding_min} < 4"
    );
    assert!(mean >= 5.0, "mean in view {mean} < 5");
}
