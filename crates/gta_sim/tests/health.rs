mod common;

use bevy::{
    ecs::reflect::ReflectMessage,
    prelude::*,
    reflect::{TypeRegistry, structs::DynamicStruct},
};
use common::*;
use gta_sim::{
    character::{Dead, HealthConfig, LocomotionConfig, MoveIntent},
    combat::{Pickup, PickupKind},
    player::DebugDamage,
    world::HospitalSpawn,
};

fn float_height(app: &App) -> f32 {
    app.world().resource::<LocomotionConfig>().float_height
}

fn pickup_at(app: &mut App, kind: PickupKind) -> (Entity, Vec3) {
    app.world_mut()
        .query::<(Entity, &Pickup, &Transform)>()
        .iter(app.world())
        .find(|(_, p, _)| p.kind == kind)
        .map(|(e, _, t)| (e, t.translation))
        .unwrap_or_else(|| panic!("GATE BROKEN: no {kind:?} pickup"))
}

fn cooldown(app: &App, pickup: Entity) -> f32 {
    app.world().get::<Pickup>(pickup).unwrap().cooldown
}

fn stand_at(app: &mut App, ground: Vec3) {
    let fh = float_height(app);
    place_player(app, ground + Vec3::Y * fh);
}

#[test]
fn armor_absorbs_then_health() {
    let mut app = headless_app();
    settle(&mut app);
    set_health(&mut app, |h| h.armor = 50.0);
    for expected in [(100.0, 20.0), (90.0, 0.0), (60.0, 0.0)] {
        write_damage(&mut app, 30.0);
        run_ticks(&mut app, 1);
        let h = health(&mut app);
        assert_eq!((h.current, h.armor), expected);
    }
}

#[test]
fn regen_waits_then_stops_at_cap() {
    let mut app = headless_app();
    settle(&mut app);
    write_damage(&mut app, 70.0);
    run_ticks(&mut app, 1);
    // The damage tick zeroes since_damage and the regen step of the same tick adds dt.
    let h = health(&mut app);
    assert_eq!(h.current, 30.0);
    assert_eq!(h.since_damage, 1.0 / 64.0);
    run_ticks(&mut app, 318);
    assert_eq!(health(&mut app).current, 30.0, "since = 319/64 < 5 s");
    run_ticks(&mut app, 1);
    let current = health(&mut app).current;
    assert!(
        (current - 30.078125).abs() < 1e-4,
        "since = 320/64 = 5 s: one regen step of 5/64, got {current}"
    );
    let mut max = current;
    for _ in 0..640 {
        run_ticks(&mut app, 1);
        max = max.max(health(&mut app).current);
    }
    assert_eq!(health(&mut app).current, 50.0);
    assert!(max <= 50.0, "regen overshot the cap: {max}");

    set_health(&mut app, |h| {
        h.current = 100.0;
        h.since_damage = 0.0;
    });
    write_damage(&mut app, 40.0);
    run_ticks(&mut app, 1);
    assert_eq!(health(&mut app).current, 60.0);
    run_ticks(&mut app, 640);
    assert_eq!(health(&mut app).current, 60.0, "no regen above the cap");

    // 70 -> 30 reaches 50 in exact 5/64 steps; 49.5 does not, so only the clamp stops at 50.
    set_health(&mut app, |h| h.current = 100.0);
    write_damage(&mut app, 50.5);
    run_ticks(&mut app, 1);
    assert_eq!(health(&mut app).current, 49.5);
    let mut max: f32 = 49.5;
    for _ in 0..400 {
        run_ticks(&mut app, 1);
        max = max.max(health(&mut app).current);
    }
    assert!(max <= 50.0, "regen overshot the cap from 49.5: {max}");
    assert_eq!(health(&mut app).current, 50.0);
}

#[test]
fn pickups_heal_armor_and_respawn() {
    let mut app = headless_app();
    settle(&mut app);
    let cfg = app.world().resource::<HealthConfig>().clone();
    let (medkit, medkit_at) = pickup_at(&mut app, PickupKind::Health);
    let (_, armor_at) = pickup_at(&mut app, PickupKind::Armor);
    let spawn = *app.world().resource::<HospitalSpawn>();
    assert_eq!(medkit_at, spawn.point + spawn.along * cfg.pickups.spacing);
    assert_eq!(armor_at, spawn.point - spawn.along * cfg.pickups.spacing);

    write_damage(&mut app, 70.0);
    run_ticks(&mut app, 1);
    assert_eq!(health(&mut app).current, 30.0);
    stand_at(&mut app, medkit_at);
    run_ticks(&mut app, 2);
    assert_eq!(health(&mut app).current, 80.0);
    assert!(cooldown(&app, medkit) > 0.0, "a taken medkit cools down");

    stand_at(&mut app, spawn.point);
    run_ticks(&mut app, 2);
    stand_at(&mut app, medkit_at);
    run_ticks(&mut app, 2);
    assert_eq!(
        health(&mut app).current,
        80.0,
        "a cooling medkit must not heal"
    );

    stand_at(&mut app, armor_at);
    run_ticks(&mut app, 2);
    assert_eq!(health(&mut app).armor, 50.0);

    stand_at(&mut app, spawn.point);
    run_ticks(&mut app, (cfg.pickups.respawn * 64.0) as u32 + 2);
    assert_eq!(cooldown(&app, medkit), 0.0, "medkit is back after respawn");

    set_health(&mut app, |h| h.current = cfg.max_health);
    stand_at(&mut app, medkit_at);
    run_ticks(&mut app, 2);
    assert_eq!(
        cooldown(&app, medkit),
        0.0,
        "a medkit is not spent at full health"
    );
}

#[test]
fn respawn_point_is_clear_of_pickups() {
    let mut app = headless_app();
    settle(&mut app);
    let radius = app.world().resource::<HealthConfig>().pickups.radius;
    let point = app.world().resource::<HospitalSpawn>().point;
    assert_eq!(count::<With<Pickup>>(&mut app), 2);
    for kind in [PickupKind::Health, PickupKind::Armor] {
        let (_, at) = pickup_at(&mut app, kind);
        assert!(
            point.distance(at) > radius,
            "{kind:?} pickup at {at} is within reach of the respawn point {point}"
        );
    }
}

#[test]
fn debug_damage_through_reflection() {
    let mut app = headless_app();
    settle(&mut app);
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let registry: &TypeRegistry = &registry.read();
    let reflect = registry
        .get_type_data::<ReflectMessage>(std::any::TypeId::of::<DebugDamage>())
        .expect("DebugDamage has no ReflectMessage: BRP world.write_message cannot reach it")
        .clone();
    let mut message = DynamicStruct::default();
    message.insert("amount", 20.0_f32);
    reflect.write_message(app.world_mut(), &message, registry);
    run_ticks(&mut app, 1);
    assert_eq!(health(&mut app).current, 80.0);
}

#[test]
fn dead_player_cannot_walk() {
    let mut app = headless_app();
    settle(&mut app);
    let start = position(&mut app);
    let entity = player(&mut app);
    app.world_mut().entity_mut(entity).insert(Dead);
    set_intent(&mut app, |intent| {
        intent.axis = Vec2::Y;
        intent.jump_requested = true;
    });
    let mut highest = start.y;
    for _ in 0..64 {
        run_ticks(&mut app, 1);
        highest = highest.max(position(&mut app).y);
    }
    let end = position(&mut app);
    let shift = Vec2::new(end.x - start.x, end.z - start.z).length();
    assert!(shift < 0.1, "a dead player walked {shift} m");
    assert!(
        !app.world()
            .get::<MoveIntent>(entity)
            .unwrap()
            .jump_requested,
        "a jump request made while dead must be dropped"
    );
    assert!(
        (highest - start.y).abs() < 0.05 && (end.y - start.y).abs() < 0.05,
        "a dead player left the ground: highest {highest}, end {}, rest {}",
        end.y,
        start.y
    );
}
