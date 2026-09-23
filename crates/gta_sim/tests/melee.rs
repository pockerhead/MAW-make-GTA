//! Melee gates (GDD §4.2): attack windows, the fist combo, knockback, stagger and the bat.
//! Tick numbers are derived from `advance_swing` at 1/64 s (see its unit table): a swing clicked
//! before T0 has its window on T0+8..T0+14 and ends on T0+23; queued swings follow on the end tick.
mod common;

use avian3d::prelude::*;
use bevy::{ecs::message::MessageCursor, prelude::*};
use bevy_tnua::prelude::*;
use common::*;
use gta_sim::{
    character::{ActionIntent, AimIntent, CharacterScheme, LocomotionConfig, WeaponRequest},
    combat::{
        BatPickup, DamageDealt, GunSlot, HitReaction, Loadout, Melee, MeleeHit, MeleeWeapon,
        Weapon, knock_back,
    },
    flow::GameState,
};

/// Attacker's feet; dummies stand at `A + d · distance`.
const A: Vec3 = Vec3::new(-20.0, 0.0, 21.0);

fn float_height(app: &App) -> f32 {
    app.world().resource::<LocomotionConfig>().float_height
}

/// Test area with the player at `A` and a dummy `distance` m along `d`, the player aiming at its chest.
fn setup(d: Vec3, distance: f32) -> (App, Entity) {
    setup_with(d, distance, |_| {})
}

/// `setup` with `extra` bodies spawned before the settling ticks.
fn setup_with(d: Vec3, distance: f32, extra: impl FnOnce(&mut App)) -> (App, Entity) {
    let mut app = headless_app();
    settle(&mut app);
    let fh = float_height(&app);
    place_player(&mut app, A + Vec3::Y * fh);
    let dummy = spawn_dummy(&mut app, A + d * distance);
    extra(&mut app);
    run_ticks(&mut app, 8);
    aim_at(&mut app, A + d * distance);
    (app, dummy)
}

/// Aims the player from its body centre at the chest (feet + 1 m) of a body with feet at `feet`.
fn aim_at(app: &mut App, feet: Vec3) {
    let origin = position(app);
    set_aim(app, origin, feet + Vec3::Y);
}

fn click(app: &mut App) {
    set_action(app, |a| a.fire_requested = true);
}

fn reaction(app: &App, entity: Entity) -> HitReaction {
    *app.world()
        .get::<HitReaction>(entity)
        .expect("GATE BROKEN: missing HitReaction")
}

fn melee(app: &mut App) -> Melee {
    let entity = player(app);
    app.world()
        .get::<Melee>(entity)
        .expect("GATE BROKEN: player missing Melee")
        .clone()
}

fn body_position(app: &App, entity: Entity) -> Vec3 {
    app.world()
        .get::<Position>(entity)
        .expect("GATE BROKEN: missing Position")
        .0
}

fn teleport(app: &mut App, entity: Entity, feet: Vec3) {
    let at = feet + Vec3::Y * float_height(app);
    app.world_mut().get_mut::<Position>(entity).unwrap().0 = at;
    app.world_mut()
        .get_mut::<Transform>(entity)
        .unwrap()
        .translation = at;
}

fn give_pistol(app: &mut App) {
    set_loadout(app, |l| {
        l.held = Some(Weapon::Pistol);
        l.guns[Weapon::Pistol.index()] = GunSlot {
            owned: true,
            magazine: 10,
            ..default()
        };
    });
}

/// `DamageDealt` and `MeleeHit` per fixed tick; tick 0 is the first `step` (T0).
struct Hits {
    dealt: MessageCursor<DamageDealt>,
    melee: MessageCursor<MeleeHit>,
    tick: u32,
    dealt_log: Vec<(u32, DamageDealt)>,
    melee_log: Vec<(u32, MeleeHit)>,
}

impl Hits {
    fn new(app: &App) -> Self {
        let world = app.world();
        Self {
            dealt: world
                .resource::<Messages<DamageDealt>>()
                .get_cursor_current(),
            melee: world.resource::<Messages<MeleeHit>>().get_cursor_current(),
            tick: 0,
            dealt_log: Vec::new(),
            melee_log: Vec::new(),
        }
    }

    /// Runs one fixed tick and records its messages; returns the tick's index.
    fn step(&mut self, app: &mut App) -> u32 {
        run_ticks(app, 1);
        let world = app.world();
        let tick = self.tick;
        self.dealt_log.extend(
            self.dealt
                .read(world.resource::<Messages<DamageDealt>>())
                .map(|m| (tick, *m)),
        );
        self.melee_log.extend(
            self.melee
                .read(world.resource::<Messages<MeleeHit>>())
                .map(|m| (tick, *m)),
        );
        self.tick += 1;
        tick
    }

    /// Steps until tick `last` inclusive.
    fn run_to(&mut self, app: &mut App, last: u32) {
        while self.tick <= last {
            self.step(app);
        }
    }

    fn ticks(&self) -> Vec<u32> {
        self.dealt_log.iter().map(|(t, _)| *t).collect()
    }
}

#[test]
fn hit_lands_only_in_window() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    let before = health_of(&app, dummy).current;
    let attacker = player(&mut app);
    let mut hits = Hits::new(&app);
    click(&mut app);
    hits.step(&mut app);
    let attack = melee(&mut app)
        .swing
        .expect("the click did not start a swing")
        .attack;
    hits.run_to(&mut app, 23);
    assert_eq!(hits.ticks(), [8], "{:?}", hits.dealt_log);
    let hit = hits.dealt_log[0].1;
    assert_eq!(
        (hit.shooter, hit.target, hit.damage, hit.headshot, hit.shot),
        (attacker, dummy, 10, false, attack)
    );
    assert_eq!(before - health_of(&app, dummy).current, 10.0);
    assert_eq!(hits.melee_log.len(), 1);

    // A target stepping into reach after the window has closed is not hit (0.234 s > 0.22 s).
    for (teleport_after, expected) in [(15, 0), (9, 1)] {
        let (mut app, dummy) = setup(Vec3::NEG_Z, 2.0);
        let mut hits = Hits::new(&app);
        click(&mut app);
        hits.run_to(&mut app, teleport_after);
        assert!(
            hits.dealt_log.is_empty(),
            "hit from 2 m: {:?}",
            hits.dealt_log
        );
        teleport(&mut app, dummy, A + Vec3::NEG_Z);
        hits.run_to(&mut app, 23);
        assert_eq!(
            hits.dealt_log.len(),
            expected,
            "teleported after T0+{teleport_after}: {:?}",
            hits.dealt_log
        );
        if expected == 1 {
            // One tick of slack for the broad phase after the teleport.
            assert!(hits.ticks()[0] <= 14, "{:?}", hits.dealt_log);
        }
    }
}

/// Clicks before T0, T0+10 and T0+`third`; returns the reaction after every tick 0..=last.
fn combo(app: &mut App, dummy: Entity, third: u32, last: u32) -> (Hits, Vec<HitReaction>) {
    let mut hits = Hits::new(app);
    let mut reactions = Vec::new();
    click(app);
    while hits.tick <= last {
        if [10, third].contains(&hits.tick) {
            click(app);
        }
        hits.step(app);
        reactions.push(reaction(app, dummy));
    }
    (hits, reactions)
}

#[test]
fn third_combo_hit_knocks_down() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    let before = health_of(&app, dummy).current;
    let (hits, reactions) = combo(&mut app, dummy, 30, 131);
    assert_eq!(hits.ticks(), [8, 31, 54], "{:?}", hits.dealt_log);
    assert!(matches!(reactions[8], HitReaction::Staggered { .. }));
    assert!(matches!(reactions[31], HitReaction::Staggered { .. }));
    assert!(reactions[54].is_knocked_down(), "{:?}", reactions[54]);
    assert!(reactions[130].is_knocked_down(), "{:?}", reactions[130]);
    assert_eq!(reactions[131], HitReaction::Steady);
    assert_eq!(before - health_of(&app, dummy).current, 40.0);
    let damages: Vec<u32> = hits.dealt_log.iter().map(|(_, h)| h.damage).collect();
    assert_eq!(damages, [10, 10, 20]);
    let mut shots: Vec<u32> = hits.dealt_log.iter().map(|(_, h)| h.shot).collect();
    shots.dedup();
    assert_eq!(shots.len(), 3, "{:?}", hits.dealt_log);
    let knockdowns: Vec<bool> = hits.melee_log.iter().map(|(_, h)| h.knockdown).collect();
    assert_eq!(knockdowns, [false, false, true]);
}

#[test]
fn combo_resets_after_window() {
    // Swing 2 ends at T0+46 with nothing queued; a click 27 idle ticks later (> 0.4 s · 64 = 25.6)
    // starts step 0 again, which lands at T0+81 as a jab.
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    let (hits, reactions) = combo(&mut app, dummy, 73, 100);
    assert_eq!(hits.ticks(), [8, 31, 81], "{:?}", hits.dealt_log);
    assert!(matches!(reactions[81], HitReaction::Staggered { .. }));
    assert!(!reactions.iter().any(HitReaction::is_knocked_down));
}

#[test]
fn knockback_pushes_along_the_blow() {
    for d in [Vec3::NEG_Z, Vec3::X, Vec3::Z] {
        let (mut app, dummy) = setup(d, 1.0);
        let before = body_position(&app, dummy);
        let mut hits = Hits::new(&app);
        click(&mut app);
        hits.run_to(&mut app, 8);
        assert_eq!(hits.ticks(), [8], "{d}: {:?}", hits.dealt_log);
        hits.run_to(&mut app, 8 + 32);
        let delta = body_position(&app, dummy) - before;
        let along = delta.dot(d);
        let lateral = (delta - along * d).length();
        println!("{d}: along {along:.3} m, lateral {lateral:.4} m");
        assert!(along >= 0.1, "{d}: pushed {along} m along the blow");
        assert!(lateral < 0.02, "{d}: {lateral} m sideways");
    }
}

#[test]
fn second_shove_during_knockback_is_applied() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    let before = body_position(&app, dummy);
    let shove = |app: &mut App, speed: f32| {
        let mut controller = app
            .world_mut()
            .get_mut::<TnuaController<CharacterScheme>>(dummy)
            .unwrap();
        knock_back(&mut controller, Vec3::NEG_Z * speed);
    };
    shove(&mut app, 3.0);
    run_ticks(&mut app, 4);
    shove(&mut app, 5.0);
    run_ticks(&mut app, 128);
    let along = (body_position(&app, dummy) - before).dot(Vec3::NEG_Z);
    println!("two shoves: {along:.3} m");
    assert!(along > 0.8, "the second shove was dropped: {along} m");
}

#[test]
fn real_strike_during_knockback_shoves_again() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    let mut hits = Hits::new(&app);
    click(&mut app);
    hits.run_to(&mut app, 3);
    {
        let mut controller = app
            .world_mut()
            .get_mut::<TnuaController<CharacterScheme>>(dummy)
            .unwrap();
        knock_back(&mut controller, Vec3::NEG_Z * 2.0);
    }
    hits.run_to(&mut app, 7);
    let speed = |app: &App| -app.world().get::<LinearVelocity>(dummy).unwrap().z;
    let v_before = speed(&app);
    hits.run_to(&mut app, 8);
    assert_eq!(hits.ticks(), [8], "GATE BROKEN: {:?}", hits.dealt_log);
    let v_after = speed(&app);
    println!("speed along the blow: {v_before:.3} -> {v_after:.3}");
    assert!(
        v_after - v_before > 0.6,
        "the jab did not shove a knocked-back dummy: {v_before} -> {v_after}"
    );
}

/// A dummy at `A − Z` that swings at the player when `fire_requested` is raised.
fn dummy_attacker(app: &mut App, dummy: Entity) {
    let chest = position(app) - Vec3::Y * 0.05;
    let origin = body_position(app, dummy);
    app.world_mut().entity_mut(dummy).insert((
        Loadout::default(),
        AimIntent {
            origin,
            direction: (chest - origin).normalize(),
            aiming: false,
        },
    ));
}

fn raise_dummy_click(app: &mut App, dummy: Entity) {
    app.world_mut()
        .get_mut::<ActionIntent>(dummy)
        .unwrap()
        .fire_requested = true;
}

#[test]
fn stagger_interrupts_the_victims_swing() {
    for player_clicks in [true, false] {
        let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
        dummy_attacker(&mut app, dummy);
        let player = player(&mut app);
        let mut hits = Hits::new(&app);
        if player_clicks {
            click(&mut app);
        }
        hits.step(&mut app);
        raise_dummy_click(&mut app, dummy);
        hits.run_to(&mut app, 40);
        let on_player: Vec<(u32, u32)> = hits
            .dealt_log
            .iter()
            .filter(|(_, h)| h.target == player)
            .map(|(t, h)| (*t, h.damage))
            .collect();
        if player_clicks {
            assert!(on_player.is_empty(), "a staggered dummy hit: {on_player:?}");
            assert!(
                hits.dealt_log
                    .iter()
                    .any(|(t, h)| *t == 8 && h.target == dummy),
                "{:?}",
                hits.dealt_log
            );
        } else {
            assert_eq!(on_player, [(9, 10)], "control: the dummy's own jab");
        }
    }
}

#[test]
fn stagger_blocks_gun_fire() {
    let (mut app, _) = setup(Vec3::NEG_Z, 3.0);
    give_pistol(&mut app);
    run_ticks(&mut app, 1);
    let entity = player(&mut app);
    *app.world_mut().get_mut::<HitReaction>(entity).unwrap() = HitReaction::Staggered { left: 0.3 };
    click(&mut app);
    let mut shots = Shots::new(&app);
    shots.run(&mut app, 1);
    assert!(shots.shots.is_empty(), "a staggered player fired");
    let action = app.world().get::<ActionIntent>(entity).unwrap().clone();
    assert!(!action.fire_requested, "the click stayed buffered");
    *app.world_mut().get_mut::<HitReaction>(entity).unwrap() = HitReaction::Steady;
    click(&mut app);
    shots.run(&mut app, 1);
    assert_eq!(shots.shots.len(), 1, "control: a steady player fires");
}

#[test]
fn bat_hit_knocks_down_at_once() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    app.world_mut()
        .spawn((BatPickup::default(), Transform::from_translation(A)));
    run_ticks(&mut app, 1);
    let l = loadout(&mut app);
    assert_eq!((l.has_bat, l.melee), (true, MeleeWeapon::Bat));
    let before = health_of(&app, dummy).current;
    let mut hits = Hits::new(&app);
    click(&mut app);
    hits.run_to(&mut app, 39);
    assert_eq!(hits.ticks(), [13], "{:?}", hits.dealt_log);
    assert_eq!(hits.dealt_log[0].1.damage, 25);
    assert_eq!(before - health_of(&app, dummy).current, 25.0);
    assert!(reaction(&app, dummy).is_knocked_down());

    for expected in [MeleeWeapon::Fists, MeleeWeapon::Bat] {
        set_action(&mut app, |a| a.select = Some(WeaponRequest::Unarmed));
        run_ticks(&mut app, 1);
        assert_eq!(loadout(&mut app).melee, expected, "key 1 while unarmed");
    }
    give_pistol(&mut app);
    set_action(&mut app, |a| a.select = Some(WeaponRequest::Unarmed));
    run_ticks(&mut app, 1);
    let l = loadout(&mut app);
    assert_eq!(
        (l.held, l.melee),
        (None, MeleeWeapon::Bat),
        "key 1 with a gun"
    );
}

#[test]
fn wall_blocks_the_punch() {
    for wall in [true, false] {
        let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
        if wall {
            spawn_wall(
                &mut app,
                A + Vec3::NEG_Z * 0.5 + Vec3::Y,
                Vec3::new(2.0, 2.0, 0.2),
            );
            run_ticks(&mut app, 2);
        }
        let mut hits = Hits::new(&app);
        click(&mut app);
        hits.run_to(&mut app, 8);
        let landed = melee(&mut app).swing.is_some_and(|s| s.landed);
        hits.run_to(&mut app, 23);
        let on_dummy = hits.dealt_log.iter().filter(|(_, h)| h.target == dummy);
        if wall {
            assert_eq!(on_dummy.count(), 0, "punched through the wall");
            assert!(landed, "the swing was not spent on the wall");
        } else {
            assert_eq!(on_dummy.count(), 1, "control: no wall");
        }
    }
}

/// Ticks on which `target` was hit by one click at T0.
fn punch_ticks_on(app: &mut App, target: Entity) -> Vec<u32> {
    let mut hits = Hits::new(app);
    click(app);
    hits.run_to(app, 23);
    hits.dealt_log
        .iter()
        .filter(|(_, h)| h.target == target)
        .map(|(t, _)| *t)
        .collect()
}

// The sweep sphere (r 0.35) is wider than the capsule (r 0.3): a body touching the attacker's back
// or side overlaps it at the start and must not take or block the punch aimed ahead.
#[test]
fn wall_behind_does_not_block_the_punch() {
    for gap in [0.0, 0.02, 0.04] {
        let (mut app, dummy) = setup_with(Vec3::NEG_Z, 1.0, |app| {
            spawn_wall(
                app,
                A + Vec3::Z * (0.3 + gap + 0.1) + Vec3::Y,
                Vec3::new(3.0, 2.0, 0.2),
            );
        });
        assert_eq!(punch_ticks_on(&mut app, dummy), [8], "wall {gap} m behind");
    }
}

#[test]
fn wall_alongside_does_not_block_the_punch() {
    let mut app = headless_app();
    settle(&mut app);
    let fh = float_height(&app);
    place_player(&mut app, A + Vec3::Y * fh);
    spawn_wall(
        &mut app,
        A + Vec3::X * 0.7 + Vec3::Y,
        Vec3::new(0.2, 2.0, 4.0),
    );
    run_ticks(&mut app, 4);
    // Walk sideways into the wall until it stops the capsule flush against it.
    set_intent(&mut app, |i| {
        i.axis = Vec2::X;
        i.yaw = 0.0;
    });
    run_ticks(&mut app, 64);
    set_intent(&mut app, |i| i.axis = Vec2::ZERO);
    run_ticks(&mut app, 64);
    let feet = position(&mut app) - Vec3::Y * fh;
    let gap = A.x + 0.6 - (feet.x + 0.3);
    assert!(gap < 0.05, "GATE BROKEN: capsule {gap} m from the wall");
    let dummy = spawn_dummy(&mut app, feet + Vec3::NEG_Z);
    run_ticks(&mut app, 8);
    aim_at(&mut app, feet + Vec3::NEG_Z);
    assert_eq!(punch_ticks_on(&mut app, dummy), [8]);
}

// Yaws at which the contact normal of a flush wall is off exact perpendicular by float noise
// (the swing slides along the wall, yet the raw dot product came out negative).
#[test]
fn rotated_wall_alongside_does_not_block_the_punch() {
    for deg in [37.0_f32, 83.0] {
        let turn = Quat::from_rotation_y(deg.to_radians());
        let (along, side) = (turn * Vec3::NEG_Z, turn * Vec3::X);
        let (mut app, dummy) = setup_with(along, 1.0, |app| {
            app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(0.2, 2.0, 4.0),
                Transform::from_translation(A + side * 0.4 + Vec3::Y).with_rotation(turn),
            ));
        });
        assert_eq!(punch_ticks_on(&mut app, dummy), [8], "wall at {deg} deg");
    }
}

#[test]
fn bystander_behind_is_not_punched() {
    let mut bystander = Entity::PLACEHOLDER;
    let (mut app, dummy) = setup_with(Vec3::NEG_Z, 1.0, |app| {
        bystander = spawn_dummy(app, A + Vec3::Z * 0.62);
    });
    let apart = (body_position(&app, bystander) - position(&mut app))
        .with_y(0.0)
        .length();
    assert!(
        apart < 0.65,
        "GATE BROKEN: bystander {apart} m away, outside the sphere"
    );
    let mut hits = Hits::new(&app);
    click(&mut app);
    hits.run_to(&mut app, 23);
    let targets: Vec<(u32, Entity)> = hits.dealt_log.iter().map(|(t, h)| (*t, h.target)).collect();
    assert_eq!(
        targets,
        [(8, dummy)],
        "bystander {bystander}, dummy ahead {dummy}"
    );
    assert_eq!(reaction(&app, bystander), HitReaction::Steady);
}

#[test]
fn body_touching_the_front_is_punched() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 0.6);
    let apart = (body_position(&app, dummy) - position(&mut app))
        .with_y(0.0)
        .length();
    assert!(
        apart < 0.65,
        "GATE BROKEN: dummy {apart} m away, outside the sphere"
    );
    assert_eq!(punch_ticks_on(&mut app, dummy), [8]);
}

#[test]
fn knocked_down_head_is_no_headshot() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    let (_, reactions) = combo(&mut app, dummy, 30, 54 + 32);
    assert!(reactions[54 + 32].is_knocked_down(), "GATE BROKEN");
    let feet = body_position(&app, dummy) - Vec3::Y * float_height(&app);
    let fh = float_height(&app);
    place_player(&mut app, feet + Vec3::Z * 5.0 + Vec3::Y * fh);
    give_pistol(&mut app);
    run_ticks(&mut app, 8);
    assert!(reaction(&app, dummy).is_knocked_down(), "GATE BROKEN");
    let shoot = |app: &mut App| {
        let origin = position(app);
        set_aim(app, origin, feet + Vec3::Y * 1.65);
        click(app);
        let mut shots = Shots::new(app);
        shots.run(app, 1);
        shots.dealt_log
    };
    let dealt = shoot(&mut app);
    assert_eq!(dealt.len(), 1, "{dealt:?}");
    assert!(
        !dealt[0].headshot,
        "the head of a knocked-down dummy counted"
    );
    while reaction(&app, dummy).is_active() {
        run_ticks(&mut app, 1);
    }
    let dealt = shoot(&mut app);
    assert_eq!(dealt.len(), 1, "{dealt:?}");
    assert!(dealt[0].headshot, "control: a steady head");
}

#[test]
fn armour_absorbs_punch_first() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    set_health_of(&mut app, dummy, |h| h.armor = 50.0);
    let mut hits = Hits::new(&app);
    click(&mut app);
    hits.run_to(&mut app, 8);
    assert_eq!(hits.dealt_log.len(), 1);
    assert_eq!(hits.dealt_log[0].1.damage, 10);
    let health = health_of(&app, dummy);
    assert_eq!((health.current, health.armor), (100.0, 40.0));
}

#[test]
fn gun_shot_and_punch_have_distinct_shot_ids() {
    let (mut app, dummy) = setup(Vec3::NEG_Z, 1.0);
    give_pistol(&mut app);
    let mut hits = Hits::new(&app);
    click(&mut app);
    hits.step(&mut app);
    assert_eq!(hits.dealt_log.len(), 1, "GATE BROKEN: the pistol missed");
    set_loadout(&mut app, |l| l.held = None);
    click(&mut app);
    hits.run_to(&mut app, 30);
    let targets: Vec<Entity> = hits.dealt_log.iter().map(|(_, h)| h.target).collect();
    assert_eq!(targets, [dummy, dummy], "{:?}", hits.dealt_log);
    let (gun, punch) = (hits.dealt_log[0].1.shot, hits.dealt_log[1].1.shot);
    assert_ne!(gun, punch, "gun and fist share an attack id");
}

#[test]
fn melee_state_is_reset_by_respawn() {
    let (mut app, _) = setup(Vec3::NEG_Z, 3.0);
    click(&mut app);
    run_ticks(&mut app, 2);
    let entity = player(&mut app);
    assert!(melee(&mut app).swing.is_some(), "GATE BROKEN: no swing");
    app.world_mut().get_mut::<Melee>(entity).unwrap().queued = true;
    *app.world_mut().get_mut::<HitReaction>(entity).unwrap() =
        HitReaction::KnockedDown { left: 1.0 };
    set_loadout(&mut app, |l| l.has_bat = true);
    write_damage(&mut app, 1000.0);
    let mut updates = 0;
    while game_state(&app) != GameState::Wasted {
        app.update();
        updates += 1;
        assert!(updates < 4, "GATE BROKEN: no Wasted");
    }
    while game_state(&app) != GameState::Playing {
        app.update();
        updates += 1;
        assert!(updates < 2000, "GATE BROKEN: Wasted never ended");
    }
    let m = melee(&mut app);
    assert_eq!(reaction(&app, entity), HitReaction::Steady);
    assert_eq!((m.swing, m.queued), (None, false));
    assert!(loadout(&mut app).has_bat, "the bat is lost on death");
}
