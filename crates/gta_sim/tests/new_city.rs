//! Pause and "Новый город" (GDD §7, T12): the pause freezes the fixed clock, the main menu builds
//! nothing and reseeds every RNG from `CitySeed`, and a new city leaves nothing of the old one.

mod common;
mod police_support;
mod wanted_support;

use bevy::{
    asset::AssetPlugin, ecs::query::Or, prelude::*, state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use common::*;
use gta_sim::{
    character::{Character, Gait},
    civilian::{Civilian, PoliceCall},
    combat::{
        BatPickup, BulletTrace, CombatRng, DamageDealt, Dummy, MeleeHit, Pickup, ShotFired, Weapon,
        WeaponPickup, WeaponsConfig, dropped_gun,
    },
    compose_sim,
    flow::{GameState, pause_request},
    gang::{GangHeat, GangRng, GangTerritories, PlayerTerritory},
    navigation::{GraphWalker, SidewalkGraph},
    perception::{Cause, StimulusLog, ThreatKind},
    player::{DebugDamage, Player},
    police::{ArrestAttempt, PoliceAlert, PoliceDispatcher, PoliceRng, UnitKind},
    population::{CameraView, NpcRng, PopulationPhase},
    wanted::{Crimes, WantedLevel},
    world::{
        City, CityBlock, CityBuilding, CityEdgeWall, CityGround, CityLandmarks, CityLayoutHash,
        CitySeed, WorldSource,
    },
};
use police_support::*;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use wanted_support::*;

fn state(app: &App) -> GameState {
    app.world().resource::<State<GameState>>().get().clone()
}

/// Esc through the sim rule; returns once `State == Paused`. The transition update may still run one
/// fixed tick (the virtual delta was taken in `First`): record baselines after this returns.
fn pause(app: &mut App) {
    let target = pause_request(
        app.world().resource::<State<GameState>>().get(),
        app.world().resource::<NextState<GameState>>(),
    );
    assert_eq!(target, Some(GameState::Paused), "GATE BROKEN: cannot pause");
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Paused);
    for _ in 0..3 {
        app.update();
        if state(app) == GameState::Paused {
            return;
        }
    }
    panic!("GATE BROKEN: not paused after 3 updates");
}

fn new_city(app: &mut App, seed: u64) {
    app.world_mut().resource_mut::<CitySeed>().0 = seed;
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Loading);
    app.update();
}

fn until_playing(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while state(app) != GameState::Playing {
        app.update();
        if let Some(exit) = app.should_exit() {
            panic!("city generation failed: {exit:?}");
        }
        assert!(
            Instant::now() < deadline,
            "city generation did not finish in 120 s"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn entities<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> HashSet<Entity> {
    app.world_mut()
        .query_filtered::<Entity, F>()
        .iter(app.world())
        .collect()
}

/// City app plus every entity with a `Transform` after its first update (before the city exists).
fn city_app_with_baseline(seed: u64) -> (App, HashSet<Entity>) {
    let mut app = composed_app(WorldSource::City { seed });
    app.update();
    let b0 = entities::<With<Transform>>(&mut app);
    until_playing(&mut app);
    (app, b0)
}

#[test]
fn pause_freezes_the_fixed_clock() {
    let mut app = headless_app();
    settle(&mut app);
    set_intent(&mut app, |intent| {
        intent.axis = Vec2::Y;
        intent.gait = Gait::Run;
    });
    pause(&mut app);
    let elapsed = app.world().resource::<Time<Fixed>>().elapsed();
    let at = position(&mut app);
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<Time<Fixed>>().elapsed(),
        elapsed,
        "the fixed clock advanced while paused"
    );
    assert_eq!(position(&mut app), at, "the player moved while paused");
    assert!(app.world().resource::<Time<Virtual>>().is_paused());

    let resume = pause_request(
        app.world().resource::<State<GameState>>().get(),
        app.world().resource::<NextState<GameState>>(),
    );
    assert_eq!(resume, Some(GameState::Playing));
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    for _ in 0..30 {
        app.update();
    }
    let step = app.world().resource::<Time<Fixed>>().timestep();
    let grown = app.world().resource::<Time<Fixed>>().elapsed() - elapsed;
    assert!(
        grown >= step * 28,
        "fixed clock grew {grown:?} after resume"
    );
    // >= 28 ticks (0.44 s) at run speed 4.5 m/s after 0.15 s of acceleration: >= 1.3 m.
    let moved = (position(&mut app) - at).with_y(0.0).length();
    assert!(moved >= 1.0, "player moved {moved} m after resume");
}

#[test]
fn main_menu_waits_for_a_seed() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        StatesPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    compose_sim(&mut app, assets_root(), WorldSource::City { seed: 7 })
        .expect("GATE BROKEN: compose_sim failed");
    app.insert_state(GameState::MainMenu);
    app.finish();
    app.cleanup();
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(state(&app), GameState::MainMenu);
    assert!(!app.world().contains_resource::<City>(), "a city was built");
    assert!(!app.world().contains_resource::<CityLayoutHash>());
    assert_eq!(count::<With<Player>>(&mut app), 0, "a player spawned");

    app.world_mut().resource_mut::<CitySeed>().0 = 2;
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Loading);
    app.update();
    let world = app.world();
    assert!(
        world.resource::<CombatRng>().0 == CombatRng::seeded(2).0,
        "CombatRng not reseeded"
    );
    assert!(
        world.resource::<NpcRng>().0 == NpcRng::seeded(2).0,
        "NpcRng not reseeded"
    );
    assert!(
        world.resource::<GangRng>().0 == GangRng::seeded(2).0,
        "GangRng not reseeded"
    );
    assert!(
        world.resource::<PoliceRng>().0 == PoliceRng::seeded(2).0,
        "PoliceRng not reseeded"
    );
    until_playing(&mut app);
    assert_eq!(app.world().resource::<CityLayoutHash>().0, golden(2));
}

type CitySnapshot = Or<(
    With<Character>,
    With<Pickup>,
    With<WeaponPickup>,
    With<BatPickup>,
    With<CityBuilding>,
    With<CityBlock>,
    With<CityGround>,
    With<CityEdgeWall>,
)>;

/// Sidewalk nodes 40..120 m (flat) from `feet`: never inside a building, outside arrest reach.
fn fixture_nodes(app: &App, feet: Vec3) -> Vec<u32> {
    let graph = app.world().resource::<SidewalkGraph>();
    (0..graph.nodes().len() as u32)
        .filter(|&n| (40.0..=120.0).contains(&(graph.node(n) - feet).with_y(0.0).length()))
        .filter(|&n| !graph.neighbors(n).is_empty())
        .collect()
}

#[test]
fn new_city_replaces_the_city() {
    let (mut app, b0) = city_app_with_baseline(1);
    settle(&mut app);
    raise_heat(&mut app, 200);

    let feet = position(&mut app);
    let nodes = fixture_nodes(&app, feet);
    assert!(
        nodes.len() >= 5,
        "GATE BROKEN: {} sidewalk nodes 40..120 m out",
        nodes.len()
    );
    let (node, to) = {
        let graph = app.world().resource::<SidewalkGraph>();
        (
            nodes.iter().map(|&n| graph.node(n)).collect::<Vec<_>>(),
            nodes
                .iter()
                .map(|&n| graph.neighbors(n)[0])
                .collect::<Vec<_>>(),
        )
    };
    for i in 0..2 {
        let walker = GraphWalker {
            from: nodes[i],
            to: to[i],
        };
        spawn_civilian(&mut app, walker, 0.0, calm());
    }
    spawn_member(&mut app, 0, node[2], Weapon::Pistol);
    let unit = spawn_unit(&mut app, UnitKind::Patrol, node[3], 0.0);
    let weapons = app.world().resource::<WeaponsConfig>().clone();
    app.world_mut()
        .spawn(dropped_gun(Weapon::Pistol, node[4], &weapons));
    set_view(&mut app, Some(chase_view(feet, Vec3::NEG_Z)));
    run_ticks(&mut app, 60);

    assert_eq!(state(&app), GameState::Playing, "GATE BROKEN: left Playing");
    assert!(
        count::<With<Civilian>>(&mut app) >= 1,
        "GATE BROKEN: no civilian"
    );
    assert!(
        count::<With<gta_sim::gang::GangMember>>(&mut app) >= 1,
        "GATE BROKEN: no gang member"
    );
    assert!(
        count::<With<gta_sim::police::PoliceUnit>>(&mut app) >= 1,
        "GATE BROKEN: no cop"
    );
    assert!(
        count::<With<gta_sim::combat::Dropped>>(&mut app) >= 1,
        "GATE BROKEN: no dropped gun"
    );
    assert_eq!(wanted(&app).heat, 200, "GATE BROKEN: heat changed");
    for (name, present) in [
        ("City", app.world().contains_resource::<City>()),
        (
            "CityLayoutHash",
            app.world().contains_resource::<CityLayoutHash>(),
        ),
        (
            "CityLandmarks",
            app.world().contains_resource::<CityLandmarks>(),
        ),
        (
            "SidewalkGraph",
            app.world().contains_resource::<SidewalkGraph>(),
        ),
        (
            "GangTerritories",
            app.world().contains_resource::<GangTerritories>(),
        ),
    ] {
        assert!(present, "GATE BROKEN: {name} missing before the new city");
    }
    let old = entities::<CitySnapshot>(&mut app);

    pause(&mut app);
    {
        let world = app.world_mut();
        world.resource_mut::<GangHeat>().left[0] = 5.0;
        *world.resource_mut::<PlayerTerritory>() = PlayerTerritory(Some(0));
        *world.resource_mut::<PoliceDispatcher>() = PoliceDispatcher {
            units: 3,
            swat: 1,
            reinforce_left: 2.0,
        };
        *world.resource_mut::<ArrestAttempt>() = ArrestAttempt {
            cop: Some(unit),
            hold: 1.0,
        };
        *world.resource_mut::<PoliceAlert>() = PoliceAlert { hostile_left: 5.0 };
        *world.resource_mut::<PopulationPhase>() = PopulationPhase::Steady;
        world
            .resource_mut::<StimulusLog>()
            .0
            .push((1, ThreatKind::Gunshot, Vec3::ZERO, 1));
        let nobody = Entity::PLACEHOLDER;
        world.write_message(ShotFired {
            shooter: nobody,
            weapon: Weapon::Pistol,
            muzzle: Vec3::ZERO,
            attack: 1,
        });
        world.write_message(BulletTrace {
            shooter: nobody,
            from: Vec3::ZERO,
            to: Vec3::X,
        });
        world.write_message(DamageDealt {
            shooter: nobody,
            shot: 1,
            target: nobody,
            point: Vec3::ZERO,
            damage: 1,
            headshot: false,
            killed: false,
        });
        world.write_message(MeleeHit {
            attacker: nobody,
            target: nobody,
            point: Vec3::ZERO,
            knockdown: false,
            attack: 1,
        });
        world.write_message(PoliceCall {
            caller: nobody,
            about: Cause::Attack(1),
        });
        world.write_message(DebugDamage { amount: 1.0 });
    }
    let world = app.world();
    assert_eq!(world.resource::<GangHeat>().left[0], 5.0, "GATE BROKEN");
    assert_eq!(
        world.resource::<PlayerTerritory>().0,
        Some(0),
        "GATE BROKEN"
    );
    assert_eq!(world.resource::<PoliceDispatcher>().units, 3, "GATE BROKEN");
    assert_eq!(world.resource::<ArrestAttempt>().hold, 1.0, "GATE BROKEN");
    assert_eq!(
        world.resource::<PoliceAlert>().hostile_left,
        5.0,
        "GATE BROKEN"
    );
    assert_eq!(
        *world.resource::<PopulationPhase>(),
        PopulationPhase::Steady
    );
    assert!(
        world.resource::<CameraView>().0.is_some(),
        "GATE BROKEN: no view"
    );
    assert!(!world.resource::<StimulusLog>().0.is_empty(), "GATE BROKEN");
    assert_ne!(*world.resource::<WantedLevel>(), WantedLevel::default());
    assert!(
        !world.resource::<Messages<ShotFired>>().is_empty(),
        "GATE BROKEN"
    );
    assert!(
        !world.resource::<Messages<BulletTrace>>().is_empty(),
        "GATE BROKEN"
    );
    assert!(
        !world.resource::<Messages<DamageDealt>>().is_empty(),
        "GATE BROKEN"
    );
    assert!(
        !world.resource::<Messages<MeleeHit>>().is_empty(),
        "GATE BROKEN"
    );
    assert!(
        !world.resource::<Messages<PoliceCall>>().is_empty(),
        "GATE BROKEN"
    );
    assert!(
        !world.resource::<Messages<DebugDamage>>().is_empty(),
        "GATE BROKEN"
    );

    new_city(&mut app, 2);
    assert_eq!(state(&app), GameState::Loading, "GATE BROKEN: not loading");
    // Case A: right after the transition frame.
    let leaked: Vec<Entity> = entities::<With<Transform>>(&mut app)
        .difference(&b0)
        .copied()
        .collect();
    assert!(
        leaked.is_empty(),
        "A1: {} entities of the old city left",
        leaked.len()
    );
    let world = app.world();
    assert!(!world.contains_resource::<City>(), "A2: City");
    assert!(
        !world.contains_resource::<CityLayoutHash>(),
        "A3: CityLayoutHash"
    );
    assert!(
        !world.contains_resource::<CityLandmarks>(),
        "A4: CityLandmarks"
    );
    assert!(
        !world.contains_resource::<SidewalkGraph>(),
        "A5: SidewalkGraph"
    );
    assert!(
        !world.contains_resource::<GangTerritories>(),
        "A6: GangTerritories"
    );
    assert_eq!(
        world.resource::<GangHeat>().left,
        vec![0.0; 2],
        "A7: GangHeat"
    );
    assert_eq!(
        world.resource::<PlayerTerritory>().0,
        None,
        "A8: PlayerTerritory"
    );
    let dispatcher = *world.resource::<PoliceDispatcher>();
    assert!(
        dispatcher.units == 0 && dispatcher.swat == 0 && dispatcher.reinforce_left == 0.0,
        "A9: {dispatcher:?}"
    );
    assert_eq!(
        *world.resource::<ArrestAttempt>(),
        ArrestAttempt::default(),
        "A10: ArrestAttempt"
    );
    assert_eq!(
        world.resource::<PoliceAlert>().hostile_left,
        0.0,
        "A11: PoliceAlert"
    );
    assert_eq!(
        *world.resource::<PopulationPhase>(),
        PopulationPhase::InitialFill,
        "A12: PopulationPhase"
    );
    assert!(
        world.resource::<CameraView>().0.is_none(),
        "A13: CameraView"
    );
    assert!(
        world.resource::<StimulusLog>().0.is_empty(),
        "A14: StimulusLog"
    );
    assert_eq!(
        *world.resource::<WantedLevel>(),
        WantedLevel::default(),
        "A15: WantedLevel"
    );
    assert!(
        world.resource::<Crimes>().incidents().is_empty(),
        "A16: Crimes"
    );
    assert_eq!(
        world.resource::<Messages<ShotFired>>().len(),
        0,
        "A17: ShotFired"
    );
    assert_eq!(
        world.resource::<Messages<BulletTrace>>().len(),
        0,
        "A18: BulletTrace"
    );
    assert_eq!(
        world.resource::<Messages<DamageDealt>>().len(),
        0,
        "A19: DamageDealt"
    );
    assert_eq!(
        world.resource::<Messages<MeleeHit>>().len(),
        0,
        "A20: MeleeHit"
    );
    assert_eq!(
        world.resource::<Messages<PoliceCall>>().len(),
        0,
        "A21: PoliceCall"
    );
    assert_eq!(
        world.resource::<Messages<DebugDamage>>().len(),
        0,
        "A22: DebugDamage"
    );
    assert!(
        !world.resource::<Time<Virtual>>().is_paused(),
        "A23: still paused"
    );

    // Case B: the new city is complete and alive.
    until_playing(&mut app);
    settle(&mut app);
    let world = app.world();
    assert_eq!(world.resource::<CitySeed>().0, 2);
    assert_eq!(world.resource::<CityLayoutHash>().0, golden(2));
    let layout = &world.resource::<City>().0;
    let buildings = layout.buildings.len();
    let blocks = layout.blocks.iter().filter(|b| b.curb.len() >= 3).count();
    let dummies = world.resource::<WeaponsConfig>().range.dummies as usize;
    assert_eq!(count::<With<Player>>(&mut app), 1, "players");
    assert_eq!(
        count::<With<CityBuilding>>(&mut app),
        buildings,
        "buildings"
    );
    assert_eq!(count::<With<CityGround>>(&mut app), 1, "ground");
    assert_eq!(count::<With<CityEdgeWall>>(&mut app), 4, "edge walls");
    assert_eq!(count::<With<CityBlock>>(&mut app), blocks, "blocks");
    assert_eq!(
        count::<With<Pickup>>(&mut app),
        2,
        "health and armour pickups"
    );
    assert_eq!(count::<With<Dummy>>(&mut app), dummies, "dummies");
    let survivors = old
        .iter()
        .filter(|&&e| app.world().get_entity(e).is_ok())
        .count();
    assert_eq!(survivors, 0, "entities of the old city alive");
    let feet = position(&mut app);
    set_view(&mut app, Some(chase_view(feet, Vec3::NEG_Z)));
    let before = app.world().resource::<Time<Fixed>>().elapsed();
    run_ticks(&mut app, 200);
    assert!(app.world().resource::<Time<Fixed>>().elapsed() > before);
    assert!(
        count::<With<Civilian>>(&mut app) >= 1,
        "no civilian spawned in the new city"
    );
}
