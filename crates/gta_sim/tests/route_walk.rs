//! Walking past the end of a graph route (`tactics::head_for`), with a cop in `Respond` as the walker:
//! the last known position lies 45 m past the goal node (beyond `direct_seek_distance`, so the cop
//! walks the route, not a direct seek). Test floor, graph X (-30, -34) - Y (-20, -34) - Z (-20, -10) -
//! G (-30, -10); G is the node nearest to the player at (-30, 35).

mod common;
mod police_support;
mod wanted_support;

use avian3d::prelude::Position;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    navigation::{NavigationConfig, Route},
    police::{CopState, UnitKind},
};
use police_support::*;
use std::f32::consts::PI;
use wanted_support::*;

const GOAL: Vec3 = Vec3::new(-30.0, 0.0, -10.0);
const PLAYER: Vec3 = Vec3::new(-30.0, 0.0, 35.0);
/// Reached: within this of the player, or the cop sees it (`cop_view_distance` 35 m).
const REACH: f32 = 20.0;

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a - b).with_y(0.0).length()
}

/// The floor with the graph and `walls`, the player at `PLAYER` as the 2-star last known position and
/// one patrol cop at the goal node G facing the player.
fn walk_setup(walls: &[(Vec3, Vec3)]) -> (App, Entity) {
    let mut app = headless_app();
    test_graph(
        &mut app,
        vec![
            Vec3::new(-30.0, 0.0, -34.0),
            Vec3::new(-20.0, 0.0, -34.0),
            Vec3::new(-20.0, 0.0, -10.0),
            GOAL,
        ],
        &[(0, 1), (1, 2), (2, 3)],
    );
    for &(center, size) in walls {
        spawn_wall(&mut app, center, size);
    }
    settle(&mut app);
    assert_shipped_police(&app);
    let nav = app.world().resource::<NavigationConfig>();
    assert_eq!(
        (
            nav.direct_seek_distance,
            nav.route_refresh_seconds,
            nav.avoid_distance
        ),
        (25.0, 1.0, 2.0),
        "GATE BROKEN: navigation"
    );
    let at = chest(&app, PLAYER);
    place_player(&mut app, at);
    set_player_armor(&mut app, 1.0e6);
    raise_heat(&mut app, 180);
    let known = wanted(&app).last_known.expect("GATE BROKEN: no last known");
    assert!(
        flat(known, PLAYER) < 0.05,
        "GATE BROKEN: last known {known}"
    );
    let unit = spawn_unit(&mut app, UnitKind::Patrol, GOAL, PI);
    assert_eq!(active(&mut app), (1, 0), "GATE BROKEN: dispatched units");
    (app, unit)
}

fn route_of(app: &App, unit: Entity) -> Route {
    app.world()
        .get::<Route>(unit)
        .expect("GATE BROKEN: no route")
        .clone()
}

/// Ticks until the cop sees the player or comes within `REACH`, at most `limit`; prints the closest
/// distance otherwise.
fn ticks_to_reach(app: &mut App, unit: Entity, limit: u32) -> Option<u32> {
    let mut closest = f32::INFINITY;
    for tick in 0..limit {
        run_ticks(app, 1);
        let c = cop(app, unit);
        let d = flat(position_of(app, unit), PLAYER);
        closest = closest.min(d);
        if c.sees || d <= REACH {
            return Some(tick);
        }
    }
    println!("closest {closest:.1} m, cop {:?}", cop(app, unit));
    None
}

/// From the goal node the cop heads straight on to the last known position and is not pulled back
/// to the node on every route refresh: 10 m to the 35 m view range at 4.5 m/s ≈ 2.3 s.
#[test]
fn walker_goes_on_past_the_goal_node() {
    let (mut app, unit) = walk_setup(&[]);
    let reached = ticks_to_reach(&mut app, unit, 640);
    assert!(
        reached.is_some(),
        "the cop never got past the goal node in 10 s"
    );
}

/// A walker on the leg past its goal node is pushed into a dead-end pocket (a retreat, a knockback;
/// here a named teleport) whose nearest node is X, not G: it re-plans along the graph and comes out.
/// Path D → X → Y → Z → G → 10 m past G ≈ 64 m, ≈ 14 s at 4.5 m/s.
#[test]
fn displaced_walker_replans() {
    // Pocket around D (-30, -24), open to -Z: back wall towards the player, side walls 1.5 m away.
    let walls = [
        (Vec3::new(-30.0, 2.0, -22.25), Vec3::new(4.0, 4.0, 0.5)),
        (Vec3::new(-31.75, 2.0, -25.0), Vec3::new(0.5, 4.0, 6.0)),
        (Vec3::new(-28.25, 2.0, -25.0), Vec3::new(0.5, 4.0, 6.0)),
    ];
    let (mut app, unit) = walk_setup(&walls);
    run_ticks(&mut app, 64);
    let route = route_of(&app, unit);
    assert!(
        route.goal == Some(3) && route.next >= route.nodes.len(),
        "GATE BROKEN: the cop is not past its goal node: {route:?}"
    );
    assert_eq!(
        cop(&app, unit).state,
        CopState::Respond,
        "GATE BROKEN: the cop left Respond"
    );
    let pocket = chest(&app, Vec3::new(-30.0, 0.0, -24.0));
    let world = app.world_mut();
    world.get_mut::<Position>(unit).unwrap().0 = pocket;
    world.get_mut::<Transform>(unit).unwrap().translation = pocket;
    run_ticks(&mut app, 1);
    assert!(
        flat(position_of(&app, unit), pocket) < 0.2,
        "GATE BROKEN: the teleport did not hold"
    );
    let reached = ticks_to_reach(&mut app, unit, 1920);
    assert!(
        reached.is_some(),
        "the displaced cop is stuck in the pocket for 30 s"
    );
}

/// A wall across the leg past the goal node: the cop walks around it (avoidance), not into it; it
/// stays nearest to G all the way, so no re-plan helps. Wall 6 m wide, 4 m high at z = 0.
#[test]
fn walker_past_the_goal_node_avoids_walls() {
    let walls = [(Vec3::new(-30.0, 2.0, 0.0), Vec3::new(6.0, 4.0, 0.5))];
    let (mut app, unit) = walk_setup(&walls);
    let reached = ticks_to_reach(&mut app, unit, 1280);
    assert!(
        reached.is_some(),
        "the cop pressed into the wall past its goal node for 20 s"
    );
}
