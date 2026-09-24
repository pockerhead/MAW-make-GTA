//! Vendored bevy-tnua-avian3d motor loop (ADR-001): a disabled controller (a corpse) must not
//! switch off the motors of characters iterated after it.

mod common;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_tnua::{TnuaMotor, TnuaToggle};
use common::*;
use gta_sim::{navigation::GraphWalker, population::Corpse};

/// Moves the player into an archetype table created after every existing one.
#[derive(Component)]
struct LateTable;

/// 64 ticks of run from rest: 4.5 m/s, 0.15 s to run speed → 4.5·(1 − 0.075) = 4.16 m.
const MIN_RUN: f32 = 3.5;

fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn run_forward(app: &mut App) -> f32 {
    let start = position(app);
    set_intent(app, |i| i.axis = Vec2::Y);
    run_ticks(app, 64);
    flat_distance(position(app), start)
}

fn kill_civilian(app: &mut App) -> Entity {
    test_graph(
        app,
        vec![Vec3::new(-25.0, 0.0, 10.0), Vec3::new(-25.0, 0.0, 20.0)],
        &[(0, 1)],
    );
    let civilian = spawn_civilian(app, GraphWalker { from: 0, to: 1 }, 0.0, calm());
    run_ticks(app, 1);
    set_health_of(app, civilian, |h| h.current = 0.0);
    for _ in 0..8 {
        run_ticks(app, 1);
        let world = app.world();
        if world.get::<Corpse>(civilian).is_some()
            && world.get::<TnuaToggle>(civilian) == Some(&TnuaToggle::Disabled)
        {
            return civilian;
        }
    }
    panic!("GATE BROKEN: civilian did not become a corpse within 8 ticks");
}

#[test]
fn walker_after_a_corpse_still_moves() {
    let mut app = headless_app();
    settle(&mut app);
    // A long-lived state, like the motor system's: tables are appended in the order they are first
    // seen. A fresh `world.query` sees all of them at once in the component index's hash order.
    let mut motors = app.world_mut().query::<(Entity, &TnuaMotor, Forces)>();
    let corpse = kill_civilian(&mut app);
    motors.iter_mut(app.world_mut()).for_each(drop);
    let walker = player(&mut app);
    app.world_mut().entity_mut(walker).insert(LateTable);

    let order: Vec<Entity> = motors
        .iter_mut(app.world_mut())
        .map(|(entity, ..)| entity)
        .collect();
    let corpse_at = order.iter().position(|&e| e == corpse);
    let walker_at = order.iter().position(|&e| e == walker);
    assert!(
        matches!((corpse_at, walker_at), (Some(c), Some(w)) if c < w),
        "GATE BROKEN: corpse not in the motor loop before the walker ({corpse_at:?}, {walker_at:?})"
    );

    let moved = run_forward(&mut app);
    assert!(
        moved >= MIN_RUN,
        "walker after a corpse moved {moved:.2} m in 64 ticks, expected >= {MIN_RUN}"
    );
}

#[test]
fn walker_without_a_corpse_moves() {
    let mut app = headless_app();
    settle(&mut app);
    let moved = run_forward(&mut app);
    assert!(
        moved >= MIN_RUN,
        "walker moved {moved:.2} m in 64 ticks, expected >= {MIN_RUN}"
    );
}
