mod common;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{Dead, Gait, HeadHitbox, LocomotionConfig, WeaponRequest, head_hitbox},
    combat::{
        AimConfig, BulletTrace, CombatRng, DamageDealt, GunSlot, Loadout, TraceHit, Weapon,
        WeaponPickup, WeaponsConfig, muzzle, roll_damage,
    },
    world::WorldSource,
};
use rand_chacha::rand_core::Rng;

const DUMMY_FEET: Vec3 = Vec3::new(-20.0, 0.0, 20.0);
const SHOOTER_FEET: Vec3 = Vec3::new(-20.0, 0.0, 30.0);
const CHEST: Vec3 = Vec3::new(-20.0, 1.0, 20.0);

fn weapons(app: &App) -> WeaponsConfig {
    app.world().resource::<WeaponsConfig>().clone()
}

fn float_height(app: &App) -> f32 {
    app.world().resource::<LocomotionConfig>().float_height
}

/// Test area with the player standing at `feet`, holding `weapon` with a full magazine.
fn armed_app(feet: Vec3, weapon: Weapon) -> App {
    let mut app = headless_app();
    settle(&mut app);
    let fh = float_height(&app);
    place_player(&mut app, feet + Vec3::Y * fh);
    give(&mut app, weapon, None);
    run_ticks(&mut app, 8);
    app
}

/// Holds `weapon` with a full magazine (or `magazine`) and an empty reserve.
fn give(app: &mut App, weapon: Weapon, magazine: Option<u32>) {
    let size = weapons(app).stats(weapon).magazine;
    set_loadout(app, |l| {
        l.held = Some(weapon);
        l.guns[weapon.index()] = GunSlot {
            owned: true,
            magazine: magazine.unwrap_or(size),
            reserve: 0,
            ..default()
        };
    });
}

fn body_centre(app: &mut App) -> Vec3 {
    position(app)
}

/// Aims from the body centre at `target` and pulls the trigger once; returns the tick's messages.
fn shoot_at(app: &mut App, target: Vec3) -> Shots {
    let origin = body_centre(app);
    shoot_from(app, origin, target)
}

fn shoot_from(app: &mut App, origin: Vec3, target: Vec3) -> Shots {
    set_aim(app, origin, target);
    set_action(app, |a| a.fire_requested = true);
    let mut shots = Shots::new(app);
    shots.run(app, 1);
    shots
}

fn band(app: &App, weapon: Weapon, base: f32) -> std::ops::RangeInclusive<u32> {
    let variance = weapons(app).stats(weapon).damage_variance;
    roll_damage(base, variance, 0.0)..=roll_damage(base, variance, 1.0)
}

fn drop_of(app: &App, dummy: Entity, before: f32) -> f32 {
    before - health_of(app, dummy).current
}

#[test]
fn pistol_hits_dummy_at_10m_for_table_damage() {
    let mut app = armed_app(SHOOTER_FEET, Weapon::Pistol);
    let dummy = spawn_dummy(&mut app, DUMMY_FEET);
    run_ticks(&mut app, 8);
    let before = health_of(&app, dummy).current;
    let shots = shoot_at(&mut app, CHEST);
    let cfg = weapons(&app);
    let expected = band(&app, Weapon::Pistol, cfg.pistol.damage);
    assert_eq!(shots.dealt_log.len(), 1, "{:?}", shots.trace_log);
    let hit = shots.dealt_log[0];
    assert_eq!(hit.target, dummy);
    assert!(!hit.headshot && !hit.killed, "{hit:?}");
    assert!(
        expected.contains(&hit.damage),
        "{} not in {expected:?}",
        hit.damage
    );
    assert_eq!(
        drop_of(&app, dummy, before),
        hit.damage as f32,
        "applied != shown"
    );
    assert_eq!(loadout(&mut app).guns[0].magazine, cfg.pistol.magazine - 1);
    assert_eq!(shots.trace_log.len(), 1);
    assert_eq!(shots.trace_log[0].hit, TraceHit::Body);
}

#[test]
fn wall_between_muzzle_and_target_blocks() {
    let mut app = armed_app(SHOOTER_FEET, Weapon::Pistol);
    let dummy = spawn_dummy(&mut app, DUMMY_FEET);
    let wall = spawn_wall(
        &mut app,
        Vec3::new(-20.0, 0.8, 29.0),
        Vec3::new(2.0, 1.6, 0.2),
    );
    run_ticks(&mut app, 8);
    let over_the_wall = Vec3::new(-20.0, 2.5, 33.0);
    let shots = shoot_from(&mut app, over_the_wall, CHEST);
    assert!(shots.dealt_log.is_empty(), "{:?}", shots.dealt_log);
    assert_eq!(health_of(&app, dummy).current, 100.0);
    assert_eq!(shots.trace_log.len(), 1);
    assert_eq!(shots.trace_log[0].hit, TraceHit::World);
    let to = shots.trace_log[0].to;
    assert!(
        (to.z - 29.1).abs() < 0.05,
        "tracer stopped at {to}, not on the wall"
    );

    // Positive control: the same shot without the wall hits.
    app.world_mut().despawn(wall);
    run_ticks(&mut app, 20);
    let shots = shoot_from(&mut app, over_the_wall, CHEST);
    let damage = weapons(&app).pistol.damage;
    assert_eq!(shots.dealt_log.len(), 1, "{:?}", shots.trace_log);
    assert!(band(&app, Weapon::Pistol, damage).contains(&shots.dealt_log[0].damage));
}

#[test]
fn head_sensor_doubles_damage() {
    let mut app = armed_app(SHOOTER_FEET, Weapon::Pistol);
    let dummy = spawn_dummy(&mut app, DUMMY_FEET);
    run_ticks(&mut app, 8);
    let before = health_of(&app, dummy).current;
    let shots = shoot_at(&mut app, Vec3::new(-20.0, 1.65, 20.0));
    let cfg = weapons(&app);
    let expected = band(
        &app,
        Weapon::Pistol,
        cfg.pistol.damage * cfg.headshot_multiplier,
    );
    assert_eq!(shots.dealt_log.len(), 1, "{:?}", shots.trace_log);
    let hit = shots.dealt_log[0];
    assert!(hit.headshot, "{hit:?}");
    assert!(
        expected.contains(&hit.damage),
        "{} not in {expected:?}",
        hit.damage
    );
    assert_eq!(drop_of(&app, dummy, before), hit.damage as f32);
}

#[test]
fn reload_takes_configured_time() {
    let mut app = armed_app(SHOOTER_FEET, Weapon::Pistol);
    set_loadout(&mut app, |l| {
        l.guns[0].magazine = 3;
        l.guns[0].reserve = 20;
    });
    let cfg = weapons(&app);
    set_action(&mut app, |a| a.reload_requested = true);
    run_ticks(&mut app, 1);
    let ticks = (cfg.pistol.reload * 64.0).ceil() as u32;
    set_aim(
        &mut app,
        Vec3::new(-20.0, 1.05, 30.0),
        Vec3::new(-20.0, 1.05, 0.0),
    );
    set_action(&mut app, |a| a.fire_requested = true);
    let mut shots = Shots::new(&app);
    shots.run(&mut app, ticks - 1);
    let slot = loadout(&mut app).guns[0];
    assert_eq!((slot.magazine, slot.reserve), (3, 20), "reloaded early");
    assert!(shots.shots.is_empty(), "fired while reloading");
    run_ticks(&mut app, 1);
    let slot = loadout(&mut app).guns[0];
    assert_eq!(
        (slot.magazine, slot.reserve),
        (cfg.pistol.magazine, 20 - (cfg.pistol.magazine - 3)),
        "not reloaded after {ticks} ticks"
    );
}

#[test]
fn shotgun_fires_ten_pellets() {
    let mut app = armed_app(Vec3::new(-20.0, 0.0, 22.0), Weapon::Shotgun);
    let dummy = spawn_dummy(&mut app, DUMMY_FEET);
    run_ticks(&mut app, 8);
    let before = health_of(&app, dummy).current;
    let shots = shoot_at(&mut app, CHEST);
    let cfg = weapons(&app);
    let pellets = cfg.shotgun.pellets as usize;
    let expected = band(&app, Weapon::Shotgun, cfg.shotgun.damage);
    assert_eq!(
        pellets, 10,
        "GATE BROKEN: the shotgun ships with 10 pellets"
    );
    assert_eq!(shots.shots.len(), 1);
    assert_eq!(shots.trace_log.len(), pellets);
    assert_eq!(shots.dealt_log.len(), pellets, "{:?}", shots.trace_log);
    let sum: u32 = shots.dealt_log.iter().map(|d| d.damage).sum();
    for hit in &shots.dealt_log {
        assert!(
            expected.contains(&hit.damage),
            "{} not in {expected:?}",
            hit.damage
        );
    }
    assert_eq!(drop_of(&app, dummy, before), sum as f32);
    assert!(health_of(&app, dummy).current > 0.0);
    let shot = shots.dealt_log[0].shot;
    assert!(
        shots.dealt_log.iter().all(|d| d.shot == shot),
        "pellets of one blast carry different shot ids: {:?}",
        shots.dealt_log
    );
    assert!(
        shots
            .trace_log
            .iter()
            .all(|t| t.attack == shots.shots[0].attack),
        "pellet traces do not carry the blast's attack id: {:?}",
        shots.trace_log
    );
}

#[test]
fn spread_grows_in_series_and_recovers() {
    let mut app = armed_app(SHOOTER_FEET, Weapon::Smg);
    let spread = weapons(&app).smg.spread;
    set_aim(
        &mut app,
        Vec3::new(-20.0, 1.05, 30.0),
        Vec3::new(-20.0, 1.05, 0.0),
    );
    set_action(&mut app, |a| a.fire_held = true);
    let mut shots = Shots::new(&app);
    let mut blooms = Vec::new();
    while blooms.len() < 15 {
        let before = shots.shots.len();
        shots.run(&mut app, 1);
        if shots.shots.len() > before {
            let l = loadout(&mut app);
            blooms.push((l.guns[Weapon::Smg.index()].bloom_deg, l.spread_deg));
        }
        assert!(shots.shots.len() < 20, "GATE BROKEN: runaway fire");
    }
    set_action(&mut app, |a| a.fire_held = false);
    for (n, &(bloom, spread_deg)) in blooms.iter().enumerate() {
        let expected = (spread.per_shot_deg * (n + 1) as f32).min(spread.max_bloom_deg);
        assert!(
            (bloom - expected).abs() < 1e-4,
            "shot {}: bloom {bloom}",
            n + 1
        );
        assert!(
            (spread_deg - (spread.base_deg + bloom)).abs() < 0.01,
            "shot {}: spread {spread_deg}",
            n + 1
        );
    }
    assert!(blooms[9].0 > blooms[8].0 && blooms[8].0 > blooms[0].0);

    // Tick k after the last shot: recovery starts once k/64 >= recovery_delay.
    let first = (spread.recovery_delay * 64.0).ceil() as u32;
    let per_tick = spread.recovery_deg_per_s / 64.0;
    let last = first + (spread.max_bloom_deg / per_tick).ceil() as u32 - 1;
    let mut k = 0;
    let mut bloom_at = |app: &mut App, tick: u32| {
        run_ticks(app, tick - k);
        k = tick;
        loadout(app).guns[Weapon::Smg.index()].bloom_deg
    };
    let max = spread.max_bloom_deg;
    assert_eq!(
        bloom_at(&mut app, first - 1),
        max,
        "recovered before the delay"
    );
    let b = bloom_at(&mut app, first);
    assert!(
        (b - (max - per_tick)).abs() < 1e-4,
        "first recovery tick: {b}"
    );
    assert!(bloom_at(&mut app, last - 1) > 0.0);
    assert_eq!(bloom_at(&mut app, last), 0.0);
}

/// Rest height of a body dropped onto a 1 m box carrying `head` as a child.
fn rest_on_box_with(head: impl Bundle) -> f32 {
    let mut app = headless_app();
    settle(&mut app);
    let center = Vec3::new(-20.0, 1.3, 25.0);
    let block = spawn_wall(&mut app, center, Vec3::new(2.0, 1.0, 2.0));
    app.world_mut().spawn((head, ChildOf(block)));
    place_player(&mut app, center + Vec3::Y * 2.0);
    run_ticks(&mut app, 128);
    position(&mut app).y
}

#[test]
fn head_hitbox_is_not_ground() {
    let app = headless_app();
    let cfg = app.world().resource::<LocomotionConfig>().clone();
    drop(app);
    let box_top = 1.8;
    let y = rest_on_box_with(head_hitbox(&cfg));
    assert!(
        (y - (box_top + cfg.float_height)).abs() < 0.05,
        "Tnua stands on the head sensor: y={y}"
    );
    // Control: a plain sphere in the same place is ground, so the fixture can see the difference.
    let plain = rest_on_box_with((
        Collider::sphere(cfg.head_radius),
        Transform::from_xyz(0.0, cfg.head_height - cfg.float_height, 0.0),
    ));
    let head_top = 1.3 + cfg.head_height - cfg.float_height + cfg.head_radius;
    assert!(
        plain > box_top + cfg.float_height + 0.2,
        "GATE BROKEN: plain sphere (top {head_top}) not seen as ground: y={plain}"
    );
}

#[test]
fn aiming_turns_body_and_caps_speed() {
    let mut app = headless_app();
    settle(&mut app);
    let start = position(&mut app);
    set_intent(&mut app, |i| i.axis = Vec2::Y);
    let entity = player(&mut app);
    app.world_mut()
        .get_mut::<gta_sim::character::AimIntent>(entity)
        .unwrap()
        .aiming = true;
    set_aim(&mut app, start, start + Vec3::NEG_X);
    run_ticks(&mut app, 64);
    let forward = *app.world().get::<Rotation>(entity).unwrap() * Vec3::NEG_Z;
    assert!(
        forward.dot(Vec3::NEG_X) > 0.99,
        "body faces {forward}, not the aim"
    );
    let moved = position(&mut app) - start;
    assert!(
        moved.z < -2.0 && moved.x.abs() < 0.3,
        "strafe moved {moved}"
    );

    let run_speed = app.world().resource::<LocomotionConfig>().run_speed;
    let speed = |aiming: bool| {
        let mut app = headless_app();
        settle(&mut app);
        let entity = player(&mut app);
        set_intent(&mut app, |i| {
            i.axis = Vec2::Y;
            i.gait = Gait::Sprint;
        });
        let at = position(&mut app);
        set_aim(&mut app, at, at + Vec3::NEG_Z);
        app.world_mut()
            .get_mut::<gta_sim::character::AimIntent>(entity)
            .unwrap()
            .aiming = aiming;
        run_ticks(&mut app, 128);
        let v = app.world().get::<LinearVelocity>(entity).unwrap().0;
        Vec2::new(v.x, v.z).length()
    };
    let aimed = speed(true);
    assert!(aimed <= run_speed + 0.05, "aiming sprint at {aimed} m/s");
    let free = speed(false);
    assert!(
        free > run_speed + 0.5,
        "GATE BROKEN: sprint without aim only {free} m/s"
    );
}

fn spawn_pickup(app: &mut App, weapon: Weapon, ammo_only: bool, at: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            WeaponPickup {
                weapon,
                ammo_only,
                cooldown: 0.0,
            },
            Transform::from_translation(at),
        ))
        .id()
}

#[test]
fn weapon_and_ammo_pickups() {
    let mut app = headless_app();
    settle(&mut app);
    let cfg = weapons(&app);
    let fh = float_height(&app);
    let gun = spawn_pickup(&mut app, Weapon::Pistol, false, Vec3::new(-20.0, 0.0, 35.0));
    let ammo = spawn_pickup(&mut app, Weapon::Pistol, true, Vec3::new(-20.0, 0.0, 38.0));
    assert_eq!(loadout(&mut app).held, None, "the player starts unarmed");
    place_player(&mut app, Vec3::new(-20.0, fh, 35.0));
    run_ticks(&mut app, 2);
    let l = loadout(&mut app);
    let mag = cfg.pistol.magazine.min(cfg.pistol.pickup_ammo);
    assert_eq!(
        l.guns[0],
        GunSlot {
            owned: true,
            magazine: mag,
            reserve: cfg.pistol.pickup_ammo - mag,
            ..default()
        }
    );
    assert_eq!(l.held, Some(Weapon::Pistol));
    assert!(app.world().get::<WeaponPickup>(gun).unwrap().cooldown > 0.0);

    place_player(&mut app, Vec3::new(-20.0, fh, 38.0));
    run_ticks(&mut app, 2);
    let reserve = loadout(&mut app).guns[0].reserve;
    assert_eq!(
        reserve,
        cfg.pistol.pickup_ammo - mag + cfg.pistol.pickup_ammo
    );

    place_player(&mut app, Vec3::new(-20.0, fh, 30.0));
    run_ticks(&mut app, (cfg.pickups.respawn * 64.0) as u32 + 2);
    for pickup in [gun, ammo] {
        assert!(app.world().get::<WeaponPickup>(pickup).unwrap().available());
    }
}

#[test]
fn dummy_dies_and_resets() {
    // Muzzle and head at the same x, 1.5 m apart: the shot line passes 0.24 m from the head
    // centre (inside r 0.35) and 0.33 m from the capsule top (outside r 0.3).
    let mut app = armed_app(Vec3::new(-20.0, 0.0, 21.95), Weapon::Pistol);
    let offset = app.world().resource::<AimConfig>().muzzle_offset();
    let origin = muzzle(body_centre(&mut app), Vec3::NEG_Z, offset);
    let dummy = spawn_dummy(&mut app, Vec3::new(origin.x, 0.0, 20.0));
    run_ticks(&mut app, 8);
    let head = Vec3::new(origin.x, 1.85, 20.0);

    set_health_of(&mut app, dummy, |h| h.current = 0.0);
    run_ticks(&mut app, 1);
    assert!(
        app.world().get::<Dead>(dummy).is_some(),
        "zero health did not kill"
    );
    let shots = shoot_from(&mut app, origin, head);
    assert!(shots.dealt_log.is_empty(), "{:?}", shots.dealt_log);
    let BulletTrace { from, to, .. } = shots.trace_log[0];
    assert!(
        to.z < 19.5,
        "the shot stopped at {to} (from {from}): the dead head still blocks"
    );

    let reset = (weapons(&app).range.dummy_reset * 64.0).ceil() as u32;
    run_ticks(&mut app, reset);
    assert!(
        app.world().get::<Dead>(dummy).is_none(),
        "not back after {reset} ticks"
    );
    assert_eq!(health_of(&app, dummy).current, 100.0);
    let shots = shoot_from(&mut app, origin, head);
    assert_eq!(shots.dealt_log.len(), 1, "{:?}", shots.trace_log);
    assert!(shots.dealt_log[0].headshot);
    let head_count = count::<With<HeadHitbox>>(&mut app);
    assert_eq!(head_count, 2, "player and dummy each carry one head");
}

#[test]
fn semi_auto_vs_automatic() {
    let mut app = armed_app(SHOOTER_FEET, Weapon::Pistol);
    set_aim(
        &mut app,
        Vec3::new(-20.0, 1.05, 30.0),
        Vec3::new(-20.0, 1.05, 0.0),
    );
    set_action(&mut app, |a| {
        a.fire_held = true;
        a.fire_requested = true;
    });
    let mut shots = Shots::new(&app);
    shots.run(&mut app, 5);
    set_action(&mut app, |a| a.fire_requested = true);
    shots.run(&mut app, 59);
    assert_eq!(
        shots.shots.len(),
        1,
        "semi-auto fired from a held trigger or a buffered press"
    );
    set_action(&mut app, |a| a.fire_requested = true);
    shots.run(&mut app, 1);
    assert_eq!(shots.shots.len(), 2);

    let mut app = armed_app(SHOOTER_FEET, Weapon::Smg);
    set_aim(
        &mut app,
        Vec3::new(-20.0, 1.05, 30.0),
        Vec3::new(-20.0, 1.05, 0.0),
    );
    set_action(&mut app, |a| a.fire_held = true);
    let mut shots = Shots::new(&app);
    shots.run(&mut app, 64);
    let interval = (weapons(&app).smg.fire_interval * 64.0).ceil() as usize;
    assert_eq!(shots.shots.len(), 1 + 63 / interval);
}

#[test]
fn damage_variance_stays_in_band_and_varies() {
    let mut app = armed_app(SHOOTER_FEET, Weapon::Pistol);
    let dummy = spawn_dummy(&mut app, DUMMY_FEET);
    run_ticks(&mut app, 8);
    let cfg = weapons(&app);
    let body = band(&app, Weapon::Pistol, cfg.pistol.damage);
    // Every shot at base spread (the D1 geometry): wait until the bloom of the previous one is gone.
    let s = cfg.pistol.spread;
    let settle_bloom =
        ((s.recovery_delay + s.per_shot_deg / s.recovery_deg_per_s) * 64.0).ceil() as u32 + 1;
    let mut seen = Vec::new();
    for shot in 0..24 {
        give(&mut app, Weapon::Pistol, None);
        set_health_of(&mut app, dummy, |h| h.current = 100.0);
        let shots = shoot_at(&mut app, CHEST);
        assert_eq!(
            shots.dealt_log.len(),
            1,
            "shot {shot}: {:?}",
            shots.trace_log
        );
        let damage = shots.dealt_log[0].damage;
        assert!(
            body.contains(&damage),
            "shot {shot}: {damage} not in {body:?}"
        );
        assert_eq!(drop_of(&app, dummy, 100.0), damage as f32, "shot {shot}");
        seen.push(damage);
        run_ticks(&mut app, settle_bloom);
        assert_eq!(
            loadout(&mut app).guns[Weapon::Pistol.index()].bloom_deg,
            0.0,
            "GATE BROKEN: bloom left over"
        );
    }
    seen.sort();
    seen.dedup();
    assert!(seen.len() >= 2, "every pistol hit rolled {seen:?}");

    let mut app = armed_app(Vec3::new(-20.0, 0.0, 22.0), Weapon::Shotgun);
    let dummy = spawn_dummy(&mut app, DUMMY_FEET);
    run_ticks(&mut app, 8);
    let pellet = band(&app, Weapon::Shotgun, cfg.shotgun.damage);
    let s = cfg.shotgun.spread;
    let wait =
        ((s.recovery_delay + s.per_shot_deg / s.recovery_deg_per_s) * 64.0).ceil() as u32 + 1;
    let mut seen = Vec::new();
    let mut blast_ids = Vec::new();
    for blast in 0..3 {
        set_health_of(&mut app, dummy, |h| h.current = 100.0);
        let shots = shoot_at(&mut app, CHEST);
        assert_eq!(
            shots.dealt_log.len(),
            cfg.shotgun.pellets as usize,
            "blast {blast}"
        );
        blast_ids.push(shots.dealt_log[0].shot);
        seen.extend(shots.dealt_log.iter().map(|d: &DamageDealt| d.damage));
        run_ticks(&mut app, wait);
    }
    assert!(
        seen.iter().all(|d| pellet.contains(d)),
        "{seen:?} not in {pellet:?}"
    );
    seen.sort();
    seen.dedup();
    assert!(seen.len() >= 2, "every pellet rolled {seen:?}");
    blast_ids.dedup();
    assert_eq!(blast_ids.len(), 3, "blasts share a shot id: {blast_ids:?}");
}

/// The city range (seed 1) stands on flat ground with a clear line from the pickups to each dummy.
#[test]
fn city_range_is_clear_on_seed_1() {
    use bevy::ecs::system::RunSystemOnce;
    let mut app = city_app(1);
    settle(&mut app);
    let centre = app
        .world()
        .resource::<gta_sim::world::CityLandmarks>()
        .park_center;
    let fh = float_height(&app);
    let dummies: Vec<(Entity, Vec3)> = app
        .world_mut()
        .query_filtered::<(Entity, &Position), With<gta_sim::combat::Dummy>>()
        .iter(app.world())
        .map(|(e, p)| (e, p.0))
        .collect();
    assert_eq!(dummies.len(), weapons(&app).range.dummies as usize);
    for (dummy, at) in dummies {
        assert!(
            (at.y - (centre.y + fh)).abs() < 0.05,
            "dummy {dummy} not standing at park level: {at}"
        );
        let from = centre + Vec3::Y * 1.4;
        let target = at - Vec3::Y * 0.05;
        let hit = app
            .world_mut()
            .run_system_once(move |q: SpatialQuery, of: Query<&ColliderOf>| {
                let dir = Dir3::new(target - from).unwrap();
                q.cast_ray(from, dir, 30.0, true, &SpatialQueryFilter::default())
                    .map(|h| of.get(h.entity).map_or(h.entity, |c| c.body))
            })
            .unwrap();
        assert_eq!(
            hit,
            Some(dummy),
            "line from the park centre to dummy {dummy} is blocked"
        );
    }
}

#[test]
fn player_starts_unarmed_with_loadout() {
    let mut app = headless_app();
    settle(&mut app);
    let l: Loadout = loadout(&mut app);
    assert_eq!(l.held, None);
    assert!(l.guns.iter().all(|g| !g.owned));
}

/// L1: cooldown and bloom belong to the gun. A pistol drawn right after a shotgun blast fires at
/// once with its base spread, and the shotgun keeps its own cooldown and bloom while holstered.
#[test]
fn cooldown_and_bloom_stay_with_their_gun() {
    let mut app = armed_app(SHOOTER_FEET, Weapon::Shotgun);
    let cfg = weapons(&app);
    set_loadout(&mut app, |l| {
        l.guns[Weapon::Pistol.index()] = GunSlot {
            owned: true,
            magazine: cfg.pistol.magazine,
            ..default()
        };
    });
    let origin = body_centre(&mut app);
    let ahead = origin + Vec3::NEG_Z * 30.0;
    let blast = shoot_from(&mut app, origin, ahead);
    assert_eq!(
        blast.shots.len(),
        1,
        "GATE BROKEN: the shotgun did not fire"
    );

    set_action(&mut app, |a| {
        a.select = Some(WeaponRequest::Gun(Weapon::Pistol))
    });
    run_ticks(&mut app, 1);
    let l = loadout(&mut app);
    assert_eq!(l.held, Some(Weapon::Pistol));
    assert!(
        (l.spread_deg - cfg.pistol.spread.base_deg).abs() < 0.01,
        "pistol drawn with spread {} (base {})",
        l.spread_deg,
        cfg.pistol.spread.base_deg
    );
    let shot = shoot_from(&mut app, origin, ahead);
    assert_eq!(
        shot.shots.iter().map(|s| s.weapon).collect::<Vec<_>>(),
        vec![Weapon::Pistol],
        "the pistol did not fire right after the shotgun blast"
    );

    set_action(&mut app, |a| {
        a.select = Some(WeaponRequest::Gun(Weapon::Shotgun))
    });
    run_ticks(&mut app, 1);
    let shotgun = loadout(&mut app).guns[Weapon::Shotgun.index()];
    assert!(
        shotgun.bloom_deg > 0.0 && shotgun.cooldown > 0.0,
        "the shotgun lost its bloom/cooldown while holstered: {shotgun:?}"
    );
    let again = shoot_from(&mut app, origin, ahead);
    assert!(
        again.shots.is_empty(),
        "switching guns skipped the shotgun cooldown"
    );
}

fn first_rolls(source: WorldSource) -> [u32; 4] {
    let mut app = composed_app(source);
    let mut rng = app.world_mut().resource_mut::<CombatRng>();
    std::array::from_fn(|_| rng.0.next_u32())
}

/// L4: the combat RNG follows the city seed (same seed, same rolls; another seed, other rolls);
/// the test level always starts from the same sequence.
#[test]
fn combat_rng_follows_city_seed() {
    let one = first_rolls(WorldSource::City { seed: 1 });
    assert_eq!(one, first_rolls(WorldSource::City { seed: 1 }));
    assert_ne!(one, first_rolls(WorldSource::City { seed: 2 }));
    assert_eq!(
        first_rolls(WorldSource::TestArea),
        first_rolls(WorldSource::TestArea)
    );
}
