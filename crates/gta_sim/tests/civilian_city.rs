//! Civilians in the generated city (seed 1): graph, wandering, spawning and despawning.

mod common;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use common::*;
use gta_sim::{
    character::LocomotionConfig,
    civilian::{Civilian, CivilianState},
    navigation::{GraphWalker, SidewalkGraph, flat_distance},
    perception::sight_blocked,
    population::{PopulationConfig, PopulationPhase, ViewCone, outside_cone},
    world::City,
};
use std::collections::HashMap;

fn graph(app: &App) -> &SidewalkGraph {
    app.world()
        .get_resource::<SidewalkGraph>()
        .expect("GATE BROKEN: no SidewalkGraph in the city")
}

fn loco(app: &App) -> LocomotionConfig {
    app.world().resource::<LocomotionConfig>().clone()
}

fn population_cfg(app: &App) -> PopulationConfig {
    app.world().resource::<PopulationConfig>().clone()
}

/// Player feet and camera-like eye.
fn player_eye(app: &mut App) -> (Vec3, Vec3) {
    let l = loco(app);
    let feet = position(app) - Vec3::Y * l.float_height;
    (feet, feet + Vec3::Y * l.head_height)
}

/// Horizontal distance from `p` to segment `a`-`b`.
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let (p, a, b) = (p.xz(), a.xz(), b.xz());
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn nearest_node(graph: &SidewalkGraph, p: Vec3) -> f32 {
    graph
        .nodes()
        .iter()
        .map(|&n| flat_distance(n, p))
        .fold(f32::INFINITY, f32::min)
}

fn feet_of(app: &App, entity: Entity) -> Vec3 {
    position_of(app, entity) - Vec3::Y * app.world().resource::<LocomotionConfig>().float_height
}

fn camera_behind(app: &mut App) -> ViewCone {
    let (_, eye) = player_eye(app);
    ViewCone::from_perspective(
        eye + Vec3::Z * 4.0,
        Dir3::NEG_Z,
        70f32.to_radians(),
        16.0 / 9.0,
    )
}

fn blocked(app: &mut App, from: Vec3, to: Vec3) -> bool {
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| sight_blocked(&spatial, from, to))
        .expect("GATE BROKEN: ray system failed")
}

#[test]
fn graph_matches_city() {
    let app = city_app(1);
    let city_edges = app.world().resource::<City>().0.sidewalks.edges.clone();
    let graph = graph(&app);
    assert_eq!(graph.edges().len(), city_edges.len());
    for (a, b) in city_edges {
        assert!(
            graph.is_edge(a, b) && graph.is_edge(b, a),
            "edge ({a}, {b})"
        );
    }
}

#[test]
fn wander_stays_on_the_graph() {
    let mut app = city_app(1);
    set_population(&mut app, |p| p.max_civilians = 0);
    let tolerance = city_params(&app).roads.street.sidewalk / 2.0;
    let arrive = app
        .world()
        .resource::<gta_sim::navigation::NavigationConfig>()
        .arrive_radius;
    assert!(tolerance > arrive, "GATE BROKEN: tolerance {tolerance}");
    let (player_feet, _) = player_eye(&mut app);
    let mut chosen: Vec<(GraphWalker, f32, Vec3)> = Vec::new();
    // Each walker starts at most 8 m (or 4 m) before its target node, so it reaches a node early in the run.
    let mut candidates = Vec::new();
    for &(a, b) in graph(&app).edges() {
        let len = flat_distance(graph(&app).node(a), graph(&app).node(b));
        for t in [(1.0 - 8.0 / len).max(0.5), (1.0 - 4.0 / len).max(0.5)] {
            candidates.push((GraphWalker { from: a, to: b }, t));
            candidates.push((GraphWalker { from: b, to: a }, t));
        }
    }
    for (walker, t) in candidates {
        let at = graph(&app)
            .node(walker.from)
            .lerp(graph(&app).node(walker.to), t);
        let d = flat_distance(at, player_feet);
        if !(3.0..=40.0).contains(&d) || chosen.iter().any(|(_, _, p)| flat_distance(*p, at) < 6.0)
        {
            continue;
        }
        chosen.push((walker, t, at));
        if chosen.len() == 12 {
            break;
        }
    }
    assert_eq!(
        chosen.len(),
        12,
        "GATE BROKEN: not enough edge points near the spawn"
    );
    let walkers = chosen
        .iter()
        .map(|&(walker, t, _)| spawn_civilian(&mut app, walker, t, calm()))
        .collect::<Vec<_>>();
    let mut changes: HashMap<Entity, (u32, u32, f32, Vec3)> = walkers
        .iter()
        .map(|&e| {
            let to = app.world().get::<GraphWalker>(e).unwrap().to;
            (e, (to, 0, 0.0, feet_of(&app, e)))
        })
        .collect();
    for sample in 0..240 {
        run_ticks(&mut app, 8);
        for &e in &walkers {
            let state = civilian_state(&app, e);
            assert!(
                matches!(state, CivilianState::Wander | CivilianState::Idle { .. }),
                "sample {sample}: {state:?}"
            );
            let walker = *app.world().get::<GraphWalker>(e).unwrap();
            let g = graph(&app);
            assert!(
                g.is_edge(walker.from, walker.to),
                "{walker:?} is not an edge"
            );
            let feet = feet_of(&app, e);
            let off = segment_distance(feet, g.node(walker.from), g.node(walker.to));
            assert!(
                off <= tolerance,
                "sample {sample}: {e} is {off} m off edge {walker:?} ({state:?})"
            );
            let entry = changes.get_mut(&e).unwrap();
            if walker.to != entry.0 {
                entry.0 = walker.to;
                entry.1 += 1;
            }
            entry.2 += flat_distance(feet, entry.3);
            entry.3 = feet;
        }
    }
    for (e, (_, changed, travelled, _)) in changes {
        assert!(
            changed >= 1 && travelled >= 20.0,
            "{e}: {changed} node changes, {travelled} m"
        );
    }
}

#[test]
fn no_spawn_before_the_view_is_known() {
    let mut app = city_app(1);
    run_ticks(&mut app, 128);
    assert_eq!(
        count::<With<Civilian>>(&mut app),
        0,
        "spawned without a view"
    );
    let (_, eye) = player_eye(&mut app);
    set_view(
        &mut app,
        Some(ViewCone::from_perspective(
            eye,
            Dir3::Y,
            70f32.to_radians(),
            16.0 / 9.0,
        )),
    );
    run_ticks(&mut app, 2);
    assert!(
        count::<With<Civilian>>(&mut app) >= 1,
        "no spawn with a view"
    );
}

/// Runs `ticks` fixed ticks with `view`, recording every new civilian's feet on its first tick.
fn spawns_under(app: &mut App, view: ViewCone, ticks: u32) -> Vec<(Entity, Vec3)> {
    set_view(app, Some(view));
    let mut seen: Vec<(Entity, Vec3)> = Vec::new();
    for _ in 0..ticks {
        run_ticks(app, 1);
        let fresh = civilians(app)
            .into_iter()
            .filter(|e| seen.iter().all(|(s, _)| s != e))
            .collect::<Vec<_>>();
        let separation = population_cfg(app).spawn_min_separation;
        for &e in &fresh {
            let feet = feet_of(app, e);
            for other in civilians(app).into_iter().filter(|&o| o != e) {
                let d = flat_distance(feet, feet_of(app, other));
                assert!(d >= separation - 0.1, "{e} spawned {d} m from {other}");
            }
            seen.push((e, feet));
        }
    }
    seen
}

#[test]
fn initial_fill_closer_than_the_ring() {
    let mut app = city_app(1);
    let cfg = population_cfg(&app);
    let head = loco(&app).head_height;
    let margin = cfg.spawn_view_margin_deg.to_radians();
    let view = camera_behind(&mut app);
    let (player_feet, _) = player_eye(&mut app);
    let nodes = graph(&app).nodes().to_vec();
    let in_range = |n: &Vec3| {
        let d = flat_distance(*n, player_feet);
        cfg.initial_inner_radius <= d && d <= cfg.spawn_ring.1
    };
    assert!(
        nodes.iter().any(|n| in_range(n)
            && flat_distance(*n, player_feet) < cfg.spawn_ring.0
            && outside_cone(&view, *n, head, margin)),
        "GATE BROKEN: no node closer than the ring outside the cone"
    );
    let visible_nodes = nodes
        .iter()
        .filter(|n| in_range(n) && !outside_cone(&view, **n, head, margin))
        .copied()
        .collect::<Vec<_>>();
    assert!(
        visible_nodes.iter().any(|&n| {
            !(blocked(&mut app, view.origin, n + Vec3::Y * 0.1)
                && blocked(&mut app, view.origin, n + Vec3::Y * head))
        }),
        "GATE BROKEN: every in-range node inside the cone is occluded"
    );
    let spawned = spawns_under(&mut app, view, 8);
    assert!(!spawned.is_empty(), "the initial fill spawned nobody");
    let mut closer = 0;
    for &(e, feet) in &spawned {
        assert!(
            nearest_node(graph(&app), feet) <= 1e-3,
            "{e} not on a node: {feet}"
        );
        let d = flat_distance(feet, player_feet);
        assert!(
            cfg.initial_inner_radius <= d && d <= cfg.spawn_ring.1,
            "{e} at {d} m"
        );
        let hidden = outside_cone(&view, feet, head, margin)
            || (blocked(&mut app, view.origin, feet + Vec3::Y * 0.1)
                && blocked(&mut app, view.origin, feet + Vec3::Y * head));
        assert!(hidden, "{e} spawned in view at {feet}");
        if d < cfg.spawn_ring.0 {
            closer += 1;
        }
    }
    assert!(
        closer >= 1,
        "no initial spawn closer than {} m",
        cfg.spawn_ring.0
    );
}

#[test]
fn steady_spawns_in_the_ring_off_frame() {
    let mut app = city_app(1);
    *app.world_mut().resource_mut::<PopulationPhase>() = PopulationPhase::Steady;
    let cfg = population_cfg(&app);
    let head = loco(&app).head_height;
    let margin = cfg.spawn_view_margin_deg.to_radians();
    let view = camera_behind(&mut app);
    let (player_feet, _) = player_eye(&mut app);
    assert!(
        graph(&app).nodes().iter().any(|&n| {
            let d = flat_distance(n, player_feet);
            cfg.spawn_ring.0 <= d && d <= cfg.spawn_ring.1 && !outside_cone(&view, n, head, margin)
        }),
        "GATE BROKEN: no ring node inside the cone"
    );
    let spawned = spawns_under(&mut app, view, 64);
    assert!(!spawned.is_empty(), "steady spawning spawned nobody");
    for &(e, feet) in &spawned {
        assert!(
            nearest_node(graph(&app), feet) <= 1e-3,
            "{e} not on a node: {feet}"
        );
        let d = flat_distance(feet, player_feet);
        assert!(
            cfg.spawn_ring.0 <= d && d <= cfg.spawn_ring.1,
            "{e} at {d} m"
        );
        assert!(
            outside_cone(&view, feet, head, margin),
            "{e} spawned in the cone"
        );
    }
}

/// A node whose horizontal distance to `from` lies in `range`, away from `avoid` by more than `angle` (rad).
fn node_at(app: &App, from: Vec3, range: std::ops::Range<f32>, avoid: Option<(Vec3, f32)>) -> u32 {
    let g = graph(app);
    (0..g.nodes().len() as u32)
        .find(|&n| {
            let p = g.node(n);
            let d = flat_distance(p, from);
            range.contains(&d)
                && !g.neighbors(n).is_empty()
                && avoid.is_none_or(|(dir, angle)| (p - from).xz().angle_to(dir.xz()).abs() > angle)
        })
        .unwrap_or_else(|| panic!("GATE BROKEN: no node in {range:?}"))
}

#[test]
fn despawn_after_2s_offscreen_beyond_150m() {
    let mut app = city_app(1);
    set_population(&mut app, |p| p.max_civilians = 0);
    let cfg = population_cfg(&app);
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let grace = (cfg.despawn_offscreen_seconds / step).round() as u32;
    assert_eq!(
        grace as f32 * step,
        cfg.despawn_offscreen_seconds,
        "GATE BROKEN"
    );
    let (player_feet, eye) = player_eye(&mut app);
    let far = cfg.despawn_distance;
    let c = node_at(&app, player_feet, far + 5.0..far + 25.0, None);
    let c_pos = graph(&app).node(c);
    let a = node_at(
        &app,
        player_feet,
        far + 5.0..far + 25.0,
        Some((c_pos - player_feet, 100f32.to_radians())),
    );
    let a_pos = graph(&app).node(a);
    let b = node_at(&app, player_feet, far - 20.0..far - 5.0, None);
    let spawn_at = |app: &mut App, n: u32| {
        let to = graph(app).neighbors(n)[0];
        let e = spawn_civilian(app, GraphWalker { from: n, to }, 0.0, calm());
        set_civilian_state(app, e, CivilianState::Idle { left: 1.0e6 });
        e
    };
    let (a, b, c) = (
        spawn_at(&mut app, a),
        spawn_at(&mut app, b),
        spawn_at(&mut app, c),
    );
    let at_c = ViewCone::from_perspective(
        eye,
        Dir3::new(c_pos + Vec3::Y - eye).unwrap(),
        70f32.to_radians(),
        16.0 / 9.0,
    );
    set_view(&mut app, Some(at_c));
    let head = loco(&app).head_height;
    assert!(
        outside_cone(&at_c, a_pos, head, 0.0) && !outside_cone(&at_c, c_pos, head, 0.0),
        "GATE BROKEN: A must be off-frame and C in frame"
    );
    run_ticks(&mut app, grace - 1);
    assert!(
        app.world().get_entity(a).is_ok(),
        "A despawned before the grace"
    );
    run_ticks(&mut app, 1);
    assert!(
        app.world().get_entity(a).is_err(),
        "A not despawned after the grace"
    );
    run_ticks(&mut app, grace);
    assert!(
        app.world().get_entity(b).is_ok(),
        "B (inside 150 m) despawned"
    );
    assert!(app.world().get_entity(c).is_ok(), "C (in view) despawned");
    set_view(
        &mut app,
        Some(ViewCone::from_perspective(
            eye,
            Dir3::Y,
            70f32.to_radians(),
            16.0 / 9.0,
        )),
    );
    run_ticks(&mut app, grace - 1);
    assert!(
        app.world().get_entity(c).is_ok(),
        "C despawned before the grace"
    );
    run_ticks(&mut app, 1);
    assert!(
        app.world().get_entity(c).is_err(),
        "C not despawned after leaving the view"
    );
    assert!(
        app.world().get_entity(b).is_ok(),
        "B (inside 150 m) despawned"
    );
}
