//! Tnua sensor entities die with their character on every production despawn path: the set of ALL
//! entities (reflected or not) returns to its baseline. bevy-tnua 0.32 sensors carry only
//! `TnuaProximitySensor` once orphaned, so a query over reflected or `Transform` components misses them.

mod common;
mod police_support;
mod wanted_support;

use bevy::{ecs::resource::IsResource, prelude::*};
use bevy_tnua::TnuaSensorsSet;
use common::*;
use gta_sim::{
    character::Health,
    civilian::Civilian,
    flow::GameState,
    navigation::GraphWalker,
    police::{CopState, UnitKind},
    population::{Corpse, ViewCone},
    world::{CitySeed, WorldSource},
};
use police_support::*;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use wanted_support::*;

/// Allowed rise of the 90-120 s maximum over the 30-60 s one: characters mid-respawn (measured spread 1).
const SLACK: usize = 10;

const SIDES: [GraphWalker; 4] = [
    GraphWalker { from: 0, to: 1 },
    GraphWalker { from: 1, to: 2 },
    GraphWalker { from: 2, to: 3 },
    GraphWalker { from: 3, to: 0 },
];

/// Every entity except resources (a resource inserted during play is state, not a spawned thing).
fn all_entities(app: &App) -> HashSet<Entity> {
    app.world()
        .iter_entities()
        .filter(|e| !e.contains::<IsResource>())
        .map(|e| e.id())
        .collect()
}

fn sensors_of(app: &App, entity: Entity) -> Vec<Entity> {
    app.world()
        .get::<TnuaSensorsSet>(entity)
        .map(|set| set.iter().collect())
        .unwrap_or_default()
}

/// Sensors of `characters`, each of which must already own at least one.
fn owned_sensors(app: &App, characters: &[Entity]) -> Vec<Entity> {
    characters
        .iter()
        .flat_map(|&c| {
            let sensors = sensors_of(app, c);
            assert!(!sensors.is_empty(), "GATE BROKEN: {c} has no Tnua sensor");
            sensors
        })
        .collect()
}

fn assert_back_to(app: &App, baseline: &HashSet<Entity>, sensors: &[Entity], path: &str) {
    let world = app.world();
    let alive = sensors
        .iter()
        .filter(|&&s| world.get_entity(s).is_ok())
        .count();
    let extra: Vec<Entity> = all_entities(app).difference(baseline).copied().collect();
    let described: Vec<String> = extra
        .iter()
        .take(5)
        .map(|&e| {
            let names: Vec<String> = world
                .inspect_entity(e)
                .map(|infos| infos.map(|i| i.name().to_string()).collect())
                .unwrap_or_default();
            format!("{e}: {names:?}")
        })
        .collect();
    assert!(
        extra.is_empty() && alive == 0,
        "{path}: {alive}/{} sensors alive, {} entities above the baseline of {} (total {}): {described:#?}",
        sensors.len(),
        extra.len(),
        baseline.len(),
        world.entity_count(),
    );
}

/// Test floor with a square sidewalk of half-size 20 m, no civilian spawning, the player settled.
fn square_floor() -> App {
    let mut app = headless_app();
    let h = 20.0;
    test_graph(
        &mut app,
        vec![
            Vec3::new(-h, 0.0, -h),
            Vec3::new(h, 0.0, -h),
            Vec3::new(h, 0.0, h),
            Vec3::new(-h, 0.0, h),
        ],
        &[(0, 1), (1, 2), (2, 3), (3, 0)],
    );
    set_population(&mut app, |p| p.max_civilians = 0);
    settle(&mut app);
    app
}

/// Looking straight up: every character is off-frame, and none is "ahead" of the view.
fn look_up(app: &mut App) {
    let eye = position(app) + Vec3::Y;
    set_view(
        app,
        Some(ViewCone::from_perspective(
            eye,
            Dir3::Y,
            70f32.to_radians(),
            16.0 / 9.0,
        )),
    );
}

fn grace_ticks(app: &App) -> u32 {
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let cfg = app
        .world()
        .resource::<gta_sim::population::PopulationConfig>();
    (cfg.despawn_offscreen_seconds / step).ceil() as u32
}

fn spawn_civilians(app: &mut App) -> Vec<Entity> {
    let civilians: Vec<Entity> = (0..8)
        .map(|i| spawn_civilian(app, SIDES[i % 4], 0.2 + 0.15 * (i / 4) as f32, calm()))
        .collect();
    run_ticks(app, 4);
    civilians
}

#[test]
fn recycled_civilians_leave_no_entity() {
    let mut app = square_floor();
    let baseline = all_entities(&app);
    let civilians = spawn_civilians(&mut app);
    let sensors = owned_sensors(&app, &civilians);
    // At the cap (0) every calm civilian beyond 1 m, off-frame and not ahead, is recyclable.
    set_population(&mut app, |p| {
        p.recycle_distance = 1.0;
        p.recycles_per_tick = 8;
    });
    look_up(&mut app);
    let grace = grace_ticks(&app);
    run_ticks(&mut app, grace + 4);
    assert!(
        civilians
            .iter()
            .all(|&c| app.world().get_entity(c).is_err()),
        "GATE BROKEN: civilians not recycled"
    );
    assert_back_to(&app, &baseline, &sensors, "recycle");
}

#[test]
fn expired_corpses_leave_no_entity() {
    let mut app = square_floor();
    let baseline = all_entities(&app);
    let civilians = spawn_civilians(&mut app);
    let sensors = owned_sensors(&app, &civilians);
    for &c in &civilians {
        app.world_mut().get_mut::<Health>(c).unwrap().current = 0.0;
    }
    run_ticks(&mut app, 1);
    assert_eq!(
        count::<With<Corpse>>(&mut app),
        civilians.len(),
        "GATE BROKEN: not all corpses"
    );
    set_population(&mut app, |p| p.corpse_seconds = 0.5);
    run_ticks(&mut app, 40);
    assert_eq!(
        count::<With<Civilian>>(&mut app),
        0,
        "GATE BROKEN: corpses not expired"
    );
    assert_back_to(&app, &baseline, &sensors, "corpse expiry");
}

#[test]
fn leaving_police_leave_no_entity() {
    // The sidewalk graph (the police FSM needs one) lies inside the spawn ring's inner edge: the
    // dispatcher finds no spawn point, only the units put here exist.
    let mut app = headless_app();
    test_graph(
        &mut app,
        vec![Vec3::new(5.0, 0.0, 5.0), Vec3::new(8.0, 0.0, 5.0)],
        &[(0, 1)],
    );
    set_population(&mut app, |p| p.max_civilians = 0);
    settle(&mut app);
    let baseline = all_entities(&app);
    raise_heat(&mut app, 40);
    let spots = [
        Vec3::new(-25.0, 0.0, 25.0),
        Vec3::new(25.0, 0.0, 25.0),
        Vec3::new(25.0, 0.0, -30.0),
        Vec3::new(-25.0, 0.0, -30.0),
    ];
    let units: Vec<Entity> = spots
        .iter()
        .map(|&spot| spawn_unit(&mut app, UnitKind::Patrol, spot, 0.0))
        .collect();
    let sensors = owned_sensors(&app, &units);
    set_heat(&mut app, 0);
    // The FSM is time-sliced: each unit decides on its own slot tick.
    for _ in 0..60 {
        if units.iter().all(|&u| cop(&app, u).state == CopState::Leave) {
            break;
        }
        run_ticks(&mut app, 1);
    }
    for &u in &units {
        assert_eq!(
            cop(&app, u).state,
            CopState::Leave,
            "GATE BROKEN: {u} does not leave at 0 stars"
        );
    }
    look_up(&mut app);
    let grace = grace_ticks(&app);
    run_ticks(&mut app, grace + 4);
    assert!(
        units.iter().all(|&u| app.world().get_entity(u).is_err()),
        "GATE BROKEN: leaving units not despawned"
    );
    assert_back_to(&app, &baseline, &sensors, "police Leave");
}

fn state(app: &App) -> GameState {
    app.world().resource::<State<GameState>>().get().clone()
}

fn set_state(app: &mut App, to: GameState) {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(to.clone());
    for _ in 0..3 {
        app.update();
        if state(app) == to {
            return;
        }
    }
    panic!("GATE BROKEN: not {to:?} after 3 updates");
}

#[test]
fn new_city_leaves_no_entity() {
    let mut app = composed_app(WorldSource::City { seed: 1 });
    app.update();
    let baseline = all_entities(&app);
    let deadline = Instant::now() + Duration::from_secs(120);
    while state(&app) != GameState::Playing {
        app.update();
        assert!(Instant::now() < deadline, "GATE BROKEN: city not ready");
        std::thread::sleep(Duration::from_millis(1));
    }
    settle(&mut app);
    let feet = position(&mut app);
    set_view(&mut app, Some(chase_view(feet, Vec3::NEG_Z)));
    run_ticks(&mut app, 120);
    let characters: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<gta_sim::character::Character>>()
        .iter(app.world())
        .collect();
    assert!(
        characters.len() >= 10,
        "GATE BROKEN: {} characters in the city",
        characters.len()
    );
    let sensors = owned_sensors(&app, &characters);
    // "Новый город" is the Paused -> Loading transition (`NEW_CITY`).
    set_state(&mut app, GameState::Paused);
    app.world_mut().resource_mut::<CitySeed>().0 = 2;
    set_state(&mut app, GameState::Loading);
    assert_back_to(&app, &baseline, &sensors, "new city");
}

fn characters(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<gta_sim::character::Character>>()
        .iter(app.world())
        .collect()
}

/// Seed-1 city, player idle behind a fixed chase view, production population caps: over 120 s of
/// civilian recycling the total entity count stays flat after the initial fill.
#[test]
fn idle_city_entity_count_stays_bounded() {
    let mut app = city_app(1);
    settle(&mut app);
    let feet = position(&mut app);
    set_view(&mut app, Some(chase_view(feet, Vec3::NEG_Z)));
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let per_second = (1.0 / step).round() as u32;
    let mut seen = HashSet::new();
    let mut samples = Vec::new();
    for _ in 0..120 {
        run_ticks(&mut app, per_second);
        seen.extend(characters(&mut app));
        samples.push(all_entities(&app).len());
    }
    let alive = characters(&mut app);
    let despawned = seen.len() - alive.len();
    let (early, late) = (&samples[30..60], &samples[90..120]);
    let range = |w: &[usize]| (*w.iter().min().unwrap(), *w.iter().max().unwrap());
    println!(
        "idle 120 s: {despawned} characters despawned, {} alive; entities min/max all {:?}, 30-60 s {:?}, 90-120 s {:?}",
        alive.len(),
        range(&samples),
        range(early),
        range(late),
    );
    assert!(
        despawned >= 30,
        "GATE BROKEN: only {despawned} characters despawned in 120 s idle"
    );
    assert!(
        range(late).1 <= range(early).1 + SLACK,
        "entity count grows: 30-60 s {:?}, 90-120 s {:?} ({despawned} despawned)",
        range(early),
        range(late),
    );
}
