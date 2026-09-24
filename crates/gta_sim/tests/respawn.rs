mod common;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{Dead, HealthConfig, LocomotionConfig, WeaponRequest},
    combat::{Dummy, GunSlot, Loadout, Pickup, Weapon, WeaponPickup, WeaponsConfig},
    flow::{GameState, RespawnConfig, WastedPhase},
    player::Player,
    wanted::{WantedConfig, WantedLevel},
    world::{CityBlock, CityBuilding, CityEdgeWall, CityGround, HospitalSpawn},
};

fn fixed_ticks(app: &App) -> u128 {
    let time = app.world().resource::<Time<Fixed>>();
    time.elapsed().as_nanos() / time.timestep().as_nanos()
}

fn speed(app: &App) -> f32 {
    app.world().resource::<Time<Virtual>>().relative_speed()
}

fn horizontal(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// Writes lethal damage and updates until `Wasted` is visible; returns the fixed ticks of the
/// update in which `Wasted` became visible (U1).
fn kill(app: &mut App) -> u128 {
    write_damage(app, 1000.0);
    for _ in 0..3 {
        let before = fixed_ticks(app);
        app.update();
        if game_state(app) == GameState::Wasted {
            return fixed_ticks(app) - before;
        }
    }
    panic!("lethal damage did not enter Wasted within 3 updates");
}

#[derive(Debug, Default)]
struct WastedRun {
    first_screen: Option<u32>,
    playing_at: u32,
    slowmo_ticks: u128,
    screen_ticks: u128,
}

/// Updates one by one from U1 (k = 0) until `Playing`, attributing each update's fixed ticks to
/// the phase visible after it. `run_ticks` cannot be used here: slow motion stalls it.
fn run_wasted(app: &mut App, u1_ticks: u128) -> WastedRun {
    let scale = app.world().resource::<RespawnConfig>().wasted_time_scale;
    let mut run = WastedRun {
        slowmo_ticks: u1_ticks,
        ..default()
    };
    for k in 1..=600 {
        let before = fixed_ticks(app);
        app.update();
        let delta = fixed_ticks(app) - before;
        if game_state(app) == GameState::Playing {
            run.playing_at = k;
            return run;
        }
        match wasted_phase(app) {
            Some(WastedPhase::SlowMo) => {
                assert_eq!(speed(app), scale, "k={k}: slow-mo speed");
                run.slowmo_ticks += delta;
            }
            Some(WastedPhase::Screen) => {
                run.first_screen.get_or_insert(k);
                assert_eq!(speed(app), 1.0, "k={k}: the screen phase runs at 1.0");
                run.screen_ticks += delta;
            }
            None => panic!("k={k}: Wasted without a WastedPhase"),
        }
    }
    panic!("Wasted did not end within 600 updates");
}

#[test]
fn death_wasted_respawn_at_hospital() {
    let mut app = city_app(1);
    settle(&mut app);
    let entity = player(&mut app);
    let mut armed = Loadout {
        held: Some(Weapon::Pistol),
        ..default()
    };
    armed.guns[Weapon::Pistol.index()] = GunSlot {
        owned: true,
        magazine: 7,
        reserve: 20,
        ..default()
    };
    app.world_mut().entity_mut(entity).insert(armed.clone());
    let three_stars = app.world().resource::<WantedConfig>().stars[2].heat;
    app.world_mut().resource_mut::<WantedLevel>().heat = three_stars;
    let scale = app.world().resource::<RespawnConfig>().wasted_time_scale;

    let u1_ticks = kill(&mut app);
    assert_eq!(
        speed(&app),
        scale,
        "k=0: slow motion starts on entering Wasted"
    );
    assert!(app.world().get::<Dead>(entity).is_some());
    let wanted = *app.world().resource::<WantedLevel>();
    assert_eq!((wanted.heat, wanted.stars), (0, 0), "{wanted:?}");
    assert_eq!(wasted_phase(&app), Some(WastedPhase::SlowMo));

    let run = run_wasted(&mut app, u1_ticks);
    eprintln!("wasted run: {run:?}");
    let screen = run.first_screen.expect("the Screen phase never came");
    assert!((95..=97).contains(&screen), "Screen first at k={screen}");
    assert!(
        (287..=289).contains(&run.playing_at),
        "Playing at k={} (Time<Real> expects 288)",
        run.playing_at
    );
    assert!(
        (27..=31).contains(&run.slowmo_ticks),
        "fixed ticks in SlowMo: {}",
        run.slowmo_ticks
    );
    assert!(
        (189..=193).contains(&run.screen_ticks),
        "fixed ticks in Screen: {}",
        run.screen_ticks
    );

    assert_eq!(speed(&app), 1.0);
    assert_eq!(player(&mut app), entity, "respawn keeps the player entity");
    let kept = app.world().get::<Loadout>(entity).unwrap();
    assert_eq!(
        (kept.held, kept.guns),
        (armed.held, armed.guns),
        "the loadout survives death"
    );
    assert!(app.world().get::<Dead>(entity).is_none());
    let health = health(&mut app);
    let max = app.world().resource::<HealthConfig>().max_health;
    assert_eq!((health.current, health.armor), (max, 0.0));
    let spawn = *app.world().resource::<HospitalSpawn>();
    let at = position(&mut app);
    assert!(
        horizontal(at, spawn.point) < 0.05,
        "respawned at {at}, hospital {}",
        spawn.point
    );

    run_ticks(&mut app, 64);
    let fh = app.world().resource::<LocomotionConfig>().float_height;
    let rest = position(&mut app);
    assert!(
        (rest.y - (spawn.point.y + fh)).abs() < 0.05,
        "not standing on the sidewalk: y={}",
        rest.y
    );
    assert!(
        horizontal(rest, at) < 0.1,
        "drifted after respawn to {rest}"
    );
    let radius = app.world().resource::<HealthConfig>().pickups.radius;
    let pickups = app
        .world_mut()
        .query::<(&Pickup, &Transform)>()
        .iter(app.world())
        .map(|(p, t)| (p.cooldown, t.translation))
        .collect::<Vec<_>>();
    assert_eq!(pickups.len(), 2);
    for (cooldown, at) in pickups {
        assert!(
            spawn.point.distance(at) > radius,
            "pickup at {at} within reach"
        );
        assert_eq!(cooldown, 0.0, "a pickup was taken on respawn");
    }
}

#[test]
fn wasted_abort_from_slowmo_restores_time() {
    let mut app = city_app(1);
    settle(&mut app);
    let scale = app.world().resource::<RespawnConfig>().wasted_time_scale;
    kill(&mut app);
    assert_eq!(wasted_phase(&app), Some(WastedPhase::SlowMo));
    assert_eq!(speed(&app), scale);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    app.update();
    assert_eq!(game_state(&app), GameState::Playing);
    assert_eq!(speed(&app), 1.0, "leaving Wasted from SlowMo restores time");
    let spawn = app.world().resource::<HospitalSpawn>().point;
    let at = position(&mut app);
    assert!(horizontal(at, spawn) < 0.05, "respawned at {at}");
}

fn world_counts(app: &mut App) -> [usize; 8] {
    [
        count::<With<Player>>(app),
        count::<With<CityBuilding>>(app),
        count::<With<CityBlock>>(app),
        count::<With<CityGround>>(app),
        count::<With<CityEdgeWall>>(app),
        count::<With<Pickup>>(app),
        count::<With<Dummy>>(app),
        count::<With<WeaponPickup>>(app),
    ]
}

#[test]
fn respawn_keeps_world_one_shot() {
    let mut app = city_app(1);
    settle(&mut app);
    let before = world_counts(&mut app);
    assert_eq!((before[0], before[5]), (1, 2));
    let range = app.world().resource::<WeaponsConfig>().range;
    assert_eq!((before[6], before[7]), (range.dummies as usize, 6));
    let u1_ticks = kill(&mut app);
    run_wasted(&mut app, u1_ticks);
    run_ticks(&mut app, 64);
    assert_eq!(
        world_counts(&mut app),
        before,
        "[Player, CityBuilding, CityBlock, CityGround, CityEdgeWall, Pickup, Dummy, WeaponPickup] \
         changed across respawn"
    );
}

/// B1: a damage message written in the last frame of `Wasted` must not reach the respawned player.
#[test]
fn damage_queued_during_wasted_is_dropped() {
    let mut app = headless_app();
    settle(&mut app);
    kill(&mut app);
    // The clock is deterministic: a second app on the same timeline gives the number of updates.
    let mut probe = headless_app();
    settle(&mut probe);
    let probe_u1 = kill(&mut probe);
    let updates = run_wasted(&mut probe, probe_u1).playing_at;
    assert!(updates > 1, "GATE BROKEN: Wasted lasted {updates} updates");
    for k in 1..updates {
        assert_eq!(
            game_state(&app),
            GameState::Wasted,
            "k={k}: left Wasted early"
        );
        app.update();
    }
    assert_eq!(
        game_state(&app),
        GameState::Wasted,
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
    let h = health(&mut app);
    assert_eq!((h.current, h.armor), (max, 0.0), "right after respawn");
    run_ticks(&mut app, 4);
    let h = health(&mut app);
    assert_eq!(
        (h.current, h.armor),
        (max, 0.0),
        "queued damage hit the respawned player"
    );
}

/// A trigger pull, reload or weapon switch given on the Wasted screen must not act after respawn.
#[test]
fn input_raised_during_wasted_is_dropped() {
    let mut app = headless_app();
    settle(&mut app);
    set_loadout(&mut app, |l| {
        l.held = Some(Weapon::Pistol);
        l.guns[Weapon::Pistol.index()] = GunSlot {
            owned: true,
            magazine: 10,
            reserve: 20,
            ..default()
        };
    });
    kill(&mut app);
    let origin = position(&mut app);
    set_aim(&mut app, origin, origin + Vec3::NEG_Z * 10.0);
    set_action(&mut app, |a| {
        a.fire_requested = true;
        a.reload_requested = true;
        a.select = Some(WeaponRequest::Unarmed);
    });
    let mut updates = 0;
    while game_state(&app) != GameState::Playing {
        app.update();
        updates += 1;
        assert!(updates < 2000, "GATE BROKEN: Wasted never ended");
    }
    run_ticks(&mut app, 8);
    let l = loadout(&mut app);
    let slot = l.guns[Weapon::Pistol.index()];
    assert_eq!(
        (l.held, slot.magazine, slot.reserve, l.reload_left),
        (Some(Weapon::Pistol), 10, 20, 0.0),
        "input from the Wasted screen acted after respawn"
    );
}
