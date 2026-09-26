//! The arrest (GDD §6.4, T11) on the test floor, production composition: a passive player next to a
//! 1-star cop is busted after exactly `arrest.seconds`, loses guns and wanted level and respawns at
//! the police station; breaking free, attacking, a knockdown and a same-tick death change that.

mod common;
mod police_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{Cuffed, Dead, Gait, HealthConfig, MoveIntent},
    combat::{GunSlot, HitReaction, Pickup, Weapon, WeaponPickup, WeaponsConfig},
    flow::{BustedPhase, GameState},
    police::{ArrestAttempt, CopState, UnitKind},
    wanted::{Crimes, WantedLevel},
    world::{CityBlock, CityBuilding, CityEdgeWall, CityGround, PoliceStationSpawn},
};
use police_support::*;
use std::f32::consts::PI;
use wanted_support::*;

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a - b).with_y(0.0).length()
}

/// `graph_app(10)` at 1 star; the player holds a loaded pistol and owns a bat; a patrol cop stands
/// at (0, 0, -10) facing the player at the origin.
fn arrest_setup() -> (App, Entity) {
    let mut app = graph_app(10.0, &[]);
    assert_shipped_police(&app);
    raise_heat(&mut app, 40);
    let stats = app
        .world()
        .resource::<WeaponsConfig>()
        .stats(Weapon::Pistol)
        .clone();
    set_loadout(&mut app, |l| {
        l.held = Some(Weapon::Pistol);
        l.guns[Weapon::Pistol.index()] = GunSlot {
            owned: true,
            magazine: stats.magazine,
            reserve: stats.pickup_ammo,
            ..default()
        };
        l.has_bat = true;
    });
    let unit = spawn_unit(&mut app, UnitKind::Patrol, Vec3::new(0.0, 0.0, -10.0), PI);
    let mut ticks = 0;
    while cop(&app, unit).state != CopState::Arrest {
        ticks += 1;
        assert!(
            ticks <= 4,
            "the cop did not go for the arrest in 4 ticks: {:?}",
            cop(&app, unit)
        );
        run_ticks(&mut app, 1);
    }
    (app, unit)
}

/// Runs one tick at a time until the arrest attempt starts (tick `s`); the hold is then one tick.
fn until_hold(app: &mut App) {
    for _ in 0..640 {
        run_ticks(app, 1);
        if attempt(app).cop.is_some() {
            assert_eq!(attempt(app).hold, 1.0 / 64.0, "hold after the start tick");
            return;
        }
    }
    panic!("no arrest attempt within 10 s");
}

/// From the update in which `Busted` became visible (k = 0), updates one by one until `Playing`;
/// returns (first update showing `Screen`, update showing `Playing`).
fn run_busted(app: &mut App) -> (u32, u32) {
    let mut screen = None;
    for k in 1..=800 {
        app.update();
        if game_state(app) == GameState::Playing {
            return (screen.expect("the Screen phase never came"), k);
        }
        match busted_phase(app) {
            Some(BustedPhase::Arrest) => assert!(screen.is_none(), "k={k}: back to Arrest"),
            Some(BustedPhase::Screen) => {
                screen.get_or_insert(k);
            }
            None => panic!("k={k}: Busted without a BustedPhase"),
        }
    }
    panic!("Busted did not end within 800 updates");
}

/// Runs the 94 ticks after `s` and the tick `s + 95` that completes the hold.
fn hold_to_busted(app: &mut App) {
    run_ticks(app, 94);
    assert_eq!(attempt(app).hold, 95.0 / 64.0, "hold after tick s+94");
    assert_eq!(game_state(app), GameState::Playing, "busted before s+95");
    run_ticks(app, 1);
    assert_eq!(
        game_state(app),
        GameState::Playing,
        "NextState applies one update later"
    );
    app.update();
    assert_eq!(game_state(app), GameState::Busted);
    assert_eq!(busted_phase(app), Some(BustedPhase::Arrest));
}

#[test]
fn passive_player_is_busted_and_disarmed() {
    let (mut app, _) = arrest_setup();
    until_hold(&mut app);
    hold_to_busted(&mut app);
    let me = player(&mut app);
    assert!(
        app.world().get::<Cuffed>(me).is_some(),
        "the busted player is not cuffed"
    );
    let (screen, playing) = run_busted(&mut app);
    assert_eq!(
        (screen, playing),
        (128, 320),
        "Screen at 2 s, Playing at 5 s of real time"
    );
    assert_confiscated(&loadout(&mut app));
    let station = app.world().resource::<PoliceStationSpawn>().point;
    let at = position(&mut app);
    assert!(
        flat(at, station) < 0.05,
        "respawned at {at}, station {station}"
    );
    assert_eq!(wanted(&app), WantedLevel::default());
    assert!(app.world().resource::<Crimes>().incidents().is_empty());
    assert!(
        app.world().get::<Cuffed>(me).is_none(),
        "still cuffed after the respawn"
    );
    assert_eq!(attempt(&app), ArrestAttempt::default());
}

#[test]
fn cuffed_player_cannot_move() {
    let (mut app, _) = arrest_setup();
    until_hold(&mut app);
    hold_to_busted(&mut app);
    let from = position(&mut app);
    set_intent(&mut app, |i| {
        i.axis = Vec2::Y;
        i.gait = Gait::Run;
    });
    for _ in 0..64 {
        app.update();
    }
    assert_eq!(
        busted_phase(&app),
        Some(BustedPhase::Arrest),
        "GATE BROKEN: left the arrest phase"
    );
    let moved = flat(position(&mut app), from);
    assert!(moved < 0.05, "the cuffed player moved {moved} m");
}

#[test]
fn breaking_free_adds_a_star() {
    let (mut app, unit) = arrest_setup();
    until_hold(&mut app);
    set_intent(&mut app, |i: &mut MoveIntent| {
        i.axis = Vec2::Y;
        i.yaw = PI;
        i.gait = Gait::Sprint;
    });
    let z0 = position(&mut app).z;
    run_ticks(&mut app, 16);
    assert!(
        position(&mut app).z > z0 + 0.5,
        "GATE BROKEN: the player does not run +Z"
    );
    let mut free = None;
    for tick in 16..128 {
        run_ticks(&mut app, 1);
        assert_ne!(
            game_state(&app),
            GameState::Busted,
            "busted while running off"
        );
        if wanted(&app).heat != 40 {
            free = Some(tick);
            break;
        }
    }
    let free = free.expect("the player never broke free in 2 s");
    let w = wanted(&app);
    assert_eq!((w.heat, w.stars), (180, 2), "tick {free}: {w:?}");
    assert_eq!(attempt(&app).cop, None);
    println!("broke free at tick {free}");
    let mut attack = false;
    for _ in 0..4 {
        run_ticks(&mut app, 1);
        attack |= cop(&app, unit).state == CopState::Attack;
    }
    assert!(attack, "the cop did not open fire: {:?}", cop(&app, unit));
    run_ticks(&mut app, 64);
    assert_ne!(game_state(&app), GameState::Busted);
}

#[test]
fn attacking_player_is_shot_not_arrested() {
    let (mut app, unit) = arrest_setup();
    set_player_armor(&mut app, 1.0e6);
    for _ in 0..320 {
        if flat(position_of(&app, unit), position(&mut app)) <= 5.0 {
            break;
        }
        run_ticks(&mut app, 1);
    }
    let gap = flat(position_of(&app, unit), position(&mut app));
    assert!(
        (4.0..=5.0).contains(&gap),
        "GATE BROKEN: the cop is {gap} m away"
    );
    assert_eq!(
        cop(&app, unit).state,
        CopState::Arrest,
        "GATE BROKEN: not arresting"
    );
    let mut probe = Probe::new(&app);
    // A shot 1 m beside the cop's chest: an attack on police (`arrest.near_miss_distance`) that hits nobody.
    let (me, at) = (position(&mut app), position_of(&app, unit));
    let across = (at - me).with_y(0.0).normalize().cross(Vec3::Y);
    let attack = fire_at(&mut app, &mut probe, at + across);
    assert!(
        !probe.shots.dealt_log.iter().any(|d| d.shot == attack),
        "GATE BROKEN: the near miss hit somebody"
    );
    assert_eq!(
        wanted(&app).stars,
        1,
        "GATE BROKEN: the shot raised the stars: {:?}",
        wanted(&app)
    );
    let mut attacked_at = None;
    for tick in 0..4 {
        if cop(&app, unit).state == CopState::Attack {
            attacked_at = Some(tick);
            break;
        }
        probe.run(&mut app, 1);
    }
    assert!(
        attacked_at.is_some(),
        "the cop kept arresting an attacker: {:?}",
        cop(&app, unit)
    );
    let mut fired = false;
    for _ in 0..128 {
        probe.run(&mut app, 1);
        fired |= probe.shots.shots.iter().any(|s| s.shooter == unit);
        assert_ne!(game_state(&app), GameState::Busted, "busted while hostile");
    }
    assert!(fired, "the cop never fired at the attacking player");
    let mut back = None;
    for tick in 0..320 {
        probe.run(&mut app, 1);
        if cop(&app, unit).state == CopState::Arrest {
            back = Some(tick);
            break;
        }
    }
    let back = back.expect("the cop never went back to the arrest");
    println!("back to Arrest {back} ticks after the 128-tick window");
    assert!(
        back + 128 + 4 >= 316,
        "back to Arrest {} ticks after the shot, before hostile_seconds ran out",
        back + 128
    );
}

#[test]
fn knocked_down_player_is_taken_at_once() {
    let (mut app, _) = arrest_setup();
    until_hold(&mut app);
    let me = player(&mut app);
    *app.world_mut().get_mut::<HitReaction>(me).unwrap() = HitReaction::KnockedDown { left: 1.2 };
    run_ticks(&mut app, 1);
    assert_eq!(
        game_state(&app),
        GameState::Playing,
        "NextState applies one update later"
    );
    assert!(
        attempt(&app).hold < 3.0 / 64.0,
        "the hold kept running: {:?}",
        attempt(&app)
    );
    app.update();
    assert_eq!(
        game_state(&app),
        GameState::Busted,
        "a knocked-down player was not taken at once"
    );
}

/// From `Busted` entered by a named mutation, the number of updates until `Playing`.
fn bust(app: &mut App) -> u32 {
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Busted);
    app.update();
    assert_eq!(
        game_state(app),
        GameState::Busted,
        "GATE BROKEN: not busted"
    );
    run_busted(app).1
}

#[test]
fn busted_drops_queued_damage() {
    let mut app = headless_app();
    settle(&mut app);
    let mut probe = headless_app();
    settle(&mut probe);
    let updates = bust(&mut probe);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Busted);
    app.update();
    for k in 1..updates {
        assert_eq!(
            game_state(&app),
            GameState::Busted,
            "k={k}: left Busted early"
        );
        app.update();
    }
    assert_eq!(
        game_state(&app),
        GameState::Busted,
        "GATE BROKEN: timeline drifted"
    );
    write_damage(&mut app, 30.0);
    app.update();
    assert_eq!(
        game_state(&app),
        GameState::Playing,
        "GATE BROKEN: not back in Playing"
    );
    let max = app.world().resource::<HealthConfig>().max_health;
    assert_eq!(health(&mut app).current, max, "right after respawn");
    run_ticks(&mut app, 4);
    assert_eq!(
        health(&mut app).current,
        max,
        "queued damage hit the respawned player"
    );
}

fn world_counts(app: &mut App) -> [usize; 8] {
    [
        count::<With<gta_sim::player::Player>>(app),
        count::<With<CityBuilding>>(app),
        count::<With<CityBlock>>(app),
        count::<With<CityGround>>(app),
        count::<With<CityEdgeWall>>(app),
        count::<With<Pickup>>(app),
        count::<With<gta_sim::combat::Dummy>>(app),
        count::<With<WeaponPickup>>(app),
    ]
}

#[test]
fn busted_keeps_world_one_shot() {
    let mut app = city_app(1);
    settle(&mut app);
    let before = world_counts(&mut app);
    assert_eq!((before[0], before[5]), (1, 2), "GATE BROKEN: {before:?}");
    bust(&mut app);
    run_ticks(&mut app, 64);
    assert_eq!(
        world_counts(&mut app),
        before,
        "[Player, CityBuilding, CityBlock, CityGround, CityEdgeWall, Pickup, Dummy, WeaponPickup] \
         changed across the arrest"
    );
}

/// P9. Guards: `Without<Dead>` in `arrest_player` and the FSM's dead-player filter (a dead player is not
/// seen, so the cop leaves `Arrest` in Decide); either suffices, so the flip removes both.
#[test]
fn death_beats_arrest_in_the_same_tick() {
    let (mut app, _) = arrest_setup();
    until_hold(&mut app);
    run_ticks(&mut app, 94);
    assert_eq!(
        attempt(&app).hold,
        95.0 / 64.0,
        "GATE BROKEN: hold after tick s+94"
    );
    set_health(&mut app, |h| h.current = 0.0);
    run_ticks(&mut app, 1);
    let me = player(&mut app);
    assert!(
        app.world().get::<Dead>(me).is_some(),
        "GATE BROKEN: the player did not die"
    );
    app.update();
    assert_eq!(
        game_state(&app),
        GameState::Wasted,
        "the arrest beat the death"
    );
    for _ in 0..8 {
        app.update();
        assert_ne!(game_state(&app), GameState::Busted);
    }
}

/// P6: the player vanishes behind the 4 m wall at z = 14; the cop walks to the last known position,
/// then searches points of the circle around it.
#[test]
fn lost_player_is_searched_at_last_known() {
    let mut app = headless_app();
    test_graph(
        &mut app,
        vec![
            Vec3::new(-6.0, 0.0, -10.0),
            Vec3::new(6.0, 0.0, -10.0),
            Vec3::new(6.0, 0.0, 0.0),
            Vec3::new(-6.0, 0.0, 0.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (3, 0)],
    );
    settle(&mut app);
    assert_shipped_police(&app);
    set_player_armor(&mut app, 1.0e6);
    raise_heat(&mut app, 180);
    let unit = spawn_unit(
        &mut app,
        UnitKind::Patrol,
        Vec3::new(-12.0, 0.0, 0.0),
        -std::f32::consts::FRAC_PI_2,
    );
    for _ in 0..8 {
        run_ticks(&mut app, 1);
    }
    assert_eq!(
        cop(&app, unit).state,
        CopState::Attack,
        "GATE BROKEN: no fight"
    );
    assert!(
        wanted(&app).seen,
        "GATE BROKEN: the cop does not see the player"
    );
    let last = position(&mut app);
    let hidden = chest(&app, Vec3::new(0.0, 0.0, 22.0));
    place_player(&mut app, hidden);
    let mut lost = false;
    for _ in 0..8 {
        run_ticks(&mut app, 1);
        let w = wanted(&app);
        if !w.seen && cop(&app, unit).state == CopState::Respond {
            let known = w.last_known.expect("no last known position");
            assert!(
                flat(known, last) < 0.05,
                "last known {known}, player was at {last}"
            );
            lost = true;
            break;
        }
    }
    assert!(
        lost,
        "the cop did not lose the player: {:?}",
        cop(&app, unit)
    );
    let mut arrived = None;
    for tick in 0..320 {
        let c = cop(&app, unit);
        if c.state == CopState::Search {
            arrived = Some(tick);
            break;
        }
        assert_eq!(c.state, CopState::Respond, "tick {tick}");
        assert_eq!(
            c.dest,
            wanted(&app).last_known,
            "tick {tick}: Respond heads elsewhere"
        );
        run_ticks(&mut app, 1);
    }
    let arrived = arrived.expect("the cop never reached the last known position in 5 s");
    assert!(
        flat(position_of(&app, unit), last) <= 3.0 + 0.1,
        "Search started {} m from the last known position",
        flat(position_of(&app, unit), last)
    );
    println!("reached the last known position in {arrived} ticks");
    let radius = wanted_cfg(&app).stars[1].search_radius;
    let mut points: Vec<Vec3> = Vec::new();
    for tick in 0..1280 {
        assert!(
            !wanted(&app).seen,
            "GATE BROKEN: the player was seen again at tick {tick}"
        );
        let c = cop(&app, unit);
        assert_eq!(c.state, CopState::Search, "tick {tick}");
        let point = c.search_point.expect("Search without a point");
        assert!(
            flat(point, last) <= radius,
            "search point {point} outside the circle"
        );
        if !points.iter().any(|p| flat(*p, point) < 0.01) {
            points.push(point);
        }
        if points.len() >= 2 && tick > 0 {
            println!("second search point after {tick} ticks");
            break;
        }
        run_ticks(&mut app, 1);
    }
    assert!(points.len() >= 2, "one search point in 20 s: {points:?}");
}
