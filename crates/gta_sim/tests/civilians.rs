//! Civilian perception, reactions, death and corpses on the flat test floor (production composition).

mod common;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use bevy_tnua::TnuaToggle;
use common::*;
use gta_sim::{
    character::{Health, LocomotionConfig},
    civilian::{CivilianConfig, CivilianState, Reaction, Temperament, choose_reaction},
    combat::{AimConfig, GunSlot, Weapon, WeaponsConfig, muzzle},
    layers::GameLayer,
    navigation::GraphWalker,
    perception::{AiClock, Perception, PerceptionConfig, Threat, ThreatKind},
    population::{Corpse, PopulationConfig},
};

/// Square loop of half-size `h` around the origin: nodes 0..4 = (-h,-h), (h,-h), (h,h), (-h,h);
/// plus a far segment 4-5 from (-34, 0, -30) to (-26, 0, -30).
fn square(app: &mut App, h: f32) {
    test_graph(
        app,
        vec![
            Vec3::new(-h, 0.0, -h),
            Vec3::new(h, 0.0, -h),
            Vec3::new(h, 0.0, h),
            Vec3::new(-h, 0.0, h),
            Vec3::new(-34.0, 0.0, -30.0),
            Vec3::new(-26.0, 0.0, -30.0),
        ],
        &[(0, 1), (1, 2), (2, 3), (3, 0), (4, 5)],
    );
}

/// Side midpoints of the square: (0,-h), (h,0), (0,h), (-h,0).
const SIDES: [GraphWalker; 4] = [
    GraphWalker { from: 0, to: 1 },
    GraphWalker { from: 1, to: 2 },
    GraphWalker { from: 2, to: 3 },
    GraphWalker { from: 3, to: 0 },
];

/// Test floor with the player settled at the origin holding a pistol, a square of half-size `h`.
fn armed_app(h: f32) -> App {
    let mut app = headless_app();
    square(&mut app, h);
    settle(&mut app);
    let size = app.world().resource::<WeaponsConfig>().pistol.magazine;
    set_loadout(&mut app, |l| {
        l.held = Some(Weapon::Pistol);
        l.guns[Weapon::Pistol.index()] = GunSlot {
            owned: true,
            magazine: size,
            reserve: 0,
            ..default()
        };
    });
    app
}

fn perception_cfg(app: &App) -> PerceptionConfig {
    app.world().resource::<PerceptionConfig>().clone()
}

fn civilian_cfg(app: &App) -> CivilianConfig {
    app.world().resource::<CivilianConfig>().clone()
}

fn clock(app: &App) -> u64 {
    app.world().resource::<AiClock>().tick
}

fn slot(app: &App, entity: Entity) -> u8 {
    app.world()
        .get::<Perception>(entity)
        .expect("GATE BROKEN: civilian missing Perception")
        .slot
}

fn calm_state(state: CivilianState) -> bool {
    matches!(state, CivilianState::Wander | CivilianState::Idle { .. })
}

/// Muzzle of a shot fired from the player's body along `direction`.
fn muzzle_of(app: &mut App, direction: Vec3) -> Vec3 {
    let offset = app.world().resource::<AimConfig>().muzzle_offset();
    muzzle(position(app), direction, offset)
}

/// Pulls the trigger aiming straight up; returns the AI tick of the shot.
fn shoot_into_the_air(app: &mut App) -> u64 {
    let eye = position(app);
    set_aim(app, eye, eye + Vec3::Y);
    set_action(app, |a| a.fire_requested = true);
    let mut shots = Shots::new(app);
    for _ in 0..4 {
        shots.run(app, 1);
        if !shots.shots.is_empty() {
            return clock(app);
        }
    }
    panic!("GATE BROKEN: the pistol did not fire");
}

/// Call progress of a civilian in `Report`, whatever it is about.
fn call_progress(state: CivilianState) -> Option<f32> {
    match state {
        CivilianState::Report { progress, .. } => Some(progress),
        _ => None,
    }
}

fn hold_idle(app: &mut App, entity: Entity) {
    set_civilian_state(app, entity, CivilianState::Idle { left: 1.0e6 });
}

#[test]
fn gunshot_at_20m_flee_or_cower_within_one_cycle() {
    let mut app = armed_app(20.0);
    let slots = u64::from(perception_cfg(&app).slots);
    let hearing = perception_cfg(&app).hearing_radius;
    let report_min = civilian_cfg(&app).reaction.report_min_distance;
    let near = SIDES.map(|walker| spawn_civilian(&mut app, walker, 0.5, calm()));
    let control = spawn_civilian(&mut app, GraphWalker { from: 4, to: 5 }, 0.5, calm());
    for (k, &e) in near.iter().enumerate() {
        assert_eq!(
            u64::from(slot(&app, e)),
            k as u64 % slots,
            "GATE BROKEN: slot order"
        );
    }
    run_ticks(&mut app, 16);
    for &e in near.iter().chain([&control]) {
        assert_eq!(civilian_state(&app, e), CivilianState::Wander);
    }
    let shot_muzzle = muzzle_of(&mut app, Vec3::Y);
    for &e in &near {
        let d = shot_muzzle.distance(position_of(&app, e));
        assert!(
            d < hearing && d < report_min,
            "GATE BROKEN: civilian at {d} m"
        );
    }
    assert!(
        shot_muzzle.distance(position_of(&app, control)) > hearing + 1.0,
        "GATE BROKEN: control within hearing"
    );
    let t = shoot_into_the_air(&mut app);
    let mut reacted: [Option<u64>; 4] = [None; 4];
    for step in 0..(3 * slots) {
        if step > 0 {
            run_ticks(&mut app, 1);
        }
        for (k, &e) in near.iter().enumerate() {
            if reacted[k].is_none() && !calm_state(civilian_state(&app, e)) {
                let state = civilian_state(&app, e);
                assert!(
                    matches!(
                        state,
                        CivilianState::Flee { .. } | CivilianState::Cower { .. }
                    ),
                    "civilian {k}: {state:?}"
                );
                reacted[k] = Some(clock(&app));
            }
        }
    }
    let ticks = reacted.map(|r| r.expect("a civilian within hearing never reacted"));
    for tick in ticks {
        assert!(
            (t..t + slots).contains(&tick),
            "reaction at {tick}, shot at {t}: {ticks:?}"
        );
    }
    let mut sorted = ticks.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 4, "reactions not time-sliced: {ticks:?}");
    assert_eq!(
        civilian_state(&app, control),
        CivilianState::Wander,
        "a civilian out of hearing reacted"
    );
}

#[test]
fn hurt_reacts_same_tick() {
    let mut app = armed_app(10.0);
    let slots = u64::from(perception_cfg(&app).slots);
    let near = SIDES.map(|walker| spawn_civilian(&mut app, walker, 0.5, calm()));
    for &e in &near {
        hold_idle(&mut app, e);
    }
    run_ticks(&mut app, 16);
    let next = (clock(&app) + 1) % slots;
    let target = *near
        .iter()
        .find(|&&e| u64::from(slot(&app, e)) != next)
        .expect("GATE BROKEN: every civilian perceives next tick");
    let origin = position(&mut app);
    let chest = position_of(&app, target);
    set_aim(&mut app, origin, chest);
    set_action(&mut app, |a| a.fire_requested = true);
    let mut shots = Shots::new(&app);
    shots.run(&mut app, 1);
    let hit = shots
        .dealt_log
        .iter()
        .find(|hit| hit.target == target)
        .unwrap_or_else(|| panic!("GATE BROKEN: shot missed: {:?}", shots.trace_log));
    assert!(!hit.killed, "GATE BROKEN: the first hit killed");
    let state = civilian_state(&app, target);
    assert!(
        matches!(
            state,
            CivilianState::Flee { .. } | CivilianState::Cower { .. }
        ),
        "hit civilian (slot {} vs tick slot {next}) did not react in the hit tick: {state:?}",
        slot(&app, target)
    );
}

#[test]
fn fatal_hit_kills_in_the_same_tick() {
    let mut app = armed_app(5.0);
    let target = spawn_civilian(&mut app, SIDES[0], 0.5, calm());
    hold_idle(&mut app, target);
    run_ticks(&mut app, 16);
    let head_height = app.world().resource::<LocomotionConfig>().head_height;
    let float_height = app.world().resource::<LocomotionConfig>().float_height;
    let magazine = app.world().resource::<WeaponsConfig>().pistol.magazine;
    for _ in 0..magazine {
        let head = position_of(&app, target) + Vec3::Y * (head_height - float_height);
        let origin = position(&mut app);
        set_aim(&mut app, origin, head);
        set_action(&mut app, |a| a.fire_requested = true);
        let mut shots = Shots::new(&app);
        shots.run(&mut app, 1);
        if !shots
            .dealt_log
            .iter()
            .any(|hit| hit.target == target && hit.killed)
        {
            run_ticks(&mut app, 24);
            continue;
        }
        assert_eq!(civilian_state(&app, target), CivilianState::Dead);
        let world = app.world();
        assert!(
            world.get::<Corpse>(target).is_some(),
            "no Corpse in the kill tick"
        );
        assert_eq!(world.get::<TnuaToggle>(target), Some(&TnuaToggle::Disabled));
        assert_eq!(world.get::<Perception>(target).unwrap().pending, None);
        return;
    }
    panic!("GATE BROKEN: no killing hit within one magazine");
}

/// A calm civilian at the square's (0, -30) midpoint with the report-prone temperament of row 4.
fn witness_app() -> (App, Entity) {
    let mut app = armed_app(30.0);
    let witness = spawn_civilian(
        &mut app,
        SIDES[0],
        0.5,
        Temperament {
            flee: 0.7,
            cower: 1.0,
            report: 1.5,
        },
    );
    run_ticks(&mut app, 16);
    let cfg = civilian_cfg(&app);
    let d = muzzle_of(&mut app, Vec3::Y).distance(position_of(&app, witness));
    assert!(
        d >= cfg.reaction.report_min_distance && d < perception_cfg(&app).hearing_radius,
        "GATE BROKEN: witness at {d} m"
    );
    (app, witness)
}

/// Runs up to one perception cycle after a shot; returns the AI tick of the first non-calm state.
fn await_reaction(app: &mut App, witness: Entity, from: CivilianState) -> u64 {
    let slots = perception_cfg(app).slots;
    for step in 0..slots {
        if step > 0 {
            run_ticks(app, 1);
        }
        if civilian_state(app, witness) != from && !calm_state(civilian_state(app, witness)) {
            return clock(app);
        }
    }
    panic!(
        "no reaction within one cycle: {:?}",
        civilian_state(app, witness)
    );
}

#[test]
fn report_is_interrupted_by_a_new_threat() {
    let (mut app, witness) = witness_app();
    shoot_into_the_air(&mut app);
    await_reaction(&mut app, witness, CivilianState::Wander);
    assert!(
        matches!(civilian_state(&app, witness), CivilianState::Report { .. }),
        "{:?}",
        civilian_state(&app, witness)
    );
    run_ticks(&mut app, 24);
    assert!(matches!(
        civilian_state(&app, witness),
        CivilianState::Report { .. }
    ));
    let calling = civilian_state(&app, witness);
    shoot_into_the_air(&mut app);
    let slots = perception_cfg(&app).slots;
    let mut state = civilian_state(&app, witness);
    for _ in 0..slots {
        if !matches!(state, CivilianState::Report { .. }) {
            break;
        }
        run_ticks(&mut app, 1);
        state = civilian_state(&app, witness);
    }
    assert!(
        matches!(state, CivilianState::Flee { .. }),
        "call {calling:?} not interrupted: {state:?}"
    );
}

#[test]
fn report_completes_after_call_seconds() {
    let (mut app, witness) = witness_app();
    let call_seconds = civilian_cfg(&app).call_seconds;
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let ticks = (call_seconds / step).round() as u32;
    assert_eq!(
        ticks as f32 * step,
        call_seconds,
        "GATE BROKEN: call not a whole tick count"
    );
    shoot_into_the_air(&mut app);
    await_reaction(&mut app, witness, CivilianState::Wander);
    assert_eq!(call_progress(civilian_state(&app, witness)), Some(0.0));
    let per_tick = step / call_seconds;
    for k in 1..ticks {
        run_ticks(&mut app, 1);
        assert_eq!(
            call_progress(civilian_state(&app, witness)),
            Some(k as f32 * per_tick),
            "tick {k}"
        );
    }
    run_ticks(&mut app, 1);
    let state = civilian_state(&app, witness);
    assert!(
        calm_state(state),
        "call not completed after {ticks} ticks: {state:?}"
    );
}

/// A report-prone witness at (0, -10) that saw a body die 6 m away (row 6) and started a call.
/// The corpse is nearer than the player's muzzle, so a later shot competes with it for "nearest".
fn corpse_witness_app() -> (App, Entity) {
    let mut app = armed_app(10.0);
    let temperament = Temperament {
        flee: 0.6,
        cower: 1.0,
        report: 1.4,
    };
    let witness = spawn_civilian(&mut app, SIDES[0], 0.5, temperament);
    let victim = spawn_civilian(&mut app, SIDES[0], 0.8, calm());
    hold_idle(&mut app, witness);
    hold_idle(&mut app, victim);
    run_ticks(&mut app, 16);
    let body = position_of(&app, victim);
    let d = position_of(&app, witness).distance(body);
    let threat = Threat {
        kind: ThreatKind::Corpse,
        at: body,
        distance: d,
        cause: None,
    };
    let cfg = civilian_cfg(&app);
    assert!(
        d <= perception_cfg(&app).corpse_sight
            && choose_reaction(&threat, &temperament, &cfg.reaction, true) == Reaction::Report,
        "GATE BROKEN: corpse at {d} m does not make a caller"
    );
    let shot = muzzle_of(&mut app, Vec3::Y).distance(position_of(&app, witness));
    assert!(
        d < shot && shot < perception_cfg(&app).hearing_radius,
        "GATE BROKEN: corpse {d} m, shot {shot} m"
    );
    app.world_mut().get_mut::<Health>(victim).unwrap().current = 0.0;
    let slots = perception_cfg(&app).slots;
    for _ in 0..=slots {
        run_ticks(&mut app, 1);
        if call_progress(civilian_state(&app, witness)) == Some(0.0) {
            return (app, witness);
        }
    }
    panic!(
        "GATE BROKEN: the witness did not start a call: {:?}",
        civilian_state(&app, witness)
    );
}

#[test]
fn corpse_in_sight_does_not_cancel_its_own_call() {
    let (mut app, witness) = corpse_witness_app();
    let call_seconds = civilian_cfg(&app).call_seconds;
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let ticks = (call_seconds / step).round() as u32;
    let per_tick = step / call_seconds;
    for k in 1..ticks {
        run_ticks(&mut app, 1);
        assert_eq!(
            call_progress(civilian_state(&app, witness)),
            Some(k as f32 * per_tick),
            "tick {k}"
        );
    }
    run_ticks(&mut app, 1);
    let state = civilian_state(&app, witness);
    assert!(
        calm_state(state),
        "call not completed after {ticks} ticks: {state:?}"
    );
}

#[test]
fn a_shot_still_interrupts_a_corpse_call() {
    let (mut app, witness) = corpse_witness_app();
    run_ticks(&mut app, 24);
    assert!(matches!(
        civilian_state(&app, witness),
        CivilianState::Report { .. }
    ));
    shoot_into_the_air(&mut app);
    let slots = perception_cfg(&app).slots;
    let mut state = civilian_state(&app, witness);
    for _ in 0..slots {
        if !matches!(state, CivilianState::Report { .. }) {
            break;
        }
        run_ticks(&mut app, 1);
        state = civilian_state(&app, witness);
    }
    assert!(
        matches!(
            state,
            CivilianState::Flee { .. } | CivilianState::Cower { .. }
        ),
        "call not interrupted by the shot: {state:?}"
    );
}

/// Hitscan ray at chest height towards `-Z` from 5 m in front of `at`.
fn chest_ray_hits(app: &mut App, at: Vec3) -> Option<Entity> {
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            let filter = SpatialQueryFilter::from_mask([
                GameLayer::World,
                GameLayer::Character,
                GameLayer::Hitbox,
            ]);
            spatial
                .cast_ray(at + Vec3::Z * 5.0, Dir3::NEG_Z, 10.0, true, &filter)
                .map(|hit| hit.entity)
        })
        .expect("GATE BROKEN: ray system failed")
}

#[test]
fn death_makes_a_corpse() {
    let mut app = armed_app(10.0);
    let victim = spawn_civilian(&mut app, SIDES[0], 0.5, calm());
    hold_idle(&mut app, victim);
    run_ticks(&mut app, 32);
    let before = position_of(&app, victim);
    assert_eq!(
        chest_ray_hits(&mut app, before),
        Some(victim),
        "GATE BROKEN: the chest ray misses the live civilian"
    );
    app.world_mut().get_mut::<Health>(victim).unwrap().current = 0.0;
    run_ticks(&mut app, 1);
    assert_eq!(civilian_state(&app, victim), CivilianState::Dead);
    let world = app.world();
    assert!(world.get::<gta_sim::character::Dead>(victim).is_some());
    assert!(world.get::<Corpse>(victim).is_some());
    assert_eq!(world.get::<TnuaToggle>(victim), Some(&TnuaToggle::Disabled));
    assert_eq!(world.get::<RigidBody>(victim), Some(&RigidBody::Static));
    let dead_at = position_of(&app, victim);
    run_ticks(&mut app, 64);
    assert!(
        position_of(&app, victim).distance(dead_at) < 1e-4,
        "corpse moved"
    );
    assert_ne!(
        chest_ray_hits(&mut app, dead_at),
        Some(victim),
        "a corpse still stops bullets"
    );
}

#[test]
fn corpse_limit_and_lifetime() {
    let mut app = armed_app(30.0);
    let limit = app.world().resource::<PopulationConfig>().corpse_limit as usize;
    let seconds = app.world().resource::<PopulationConfig>().corpse_seconds;
    let step = app
        .world()
        .resource::<Time<Fixed>>()
        .timestep()
        .as_secs_f32();
    let lifetime = (seconds / step).round() as u32;
    assert_eq!(
        lifetime as f32 * step,
        seconds,
        "GATE BROKEN: lifetime not whole ticks"
    );
    let per_side = limit.div_ceil(4) + 1;
    let victims = (0..=limit)
        .map(|i| {
            let t = (i / 4) as f32 / per_side as f32 + 0.05;
            spawn_civilian(&mut app, SIDES[i % 4], t, calm())
        })
        .collect::<Vec<_>>();
    run_ticks(&mut app, 8);
    for &victim in &victims {
        app.world_mut().get_mut::<Health>(victim).unwrap().current = 0.0;
        run_ticks(&mut app, 1);
    }
    run_ticks(&mut app, 1);
    assert_eq!(count::<With<Corpse>>(&mut app), limit);
    assert!(
        app.world().get_entity(victims[0]).is_err(),
        "the oldest corpse survived the limit"
    );
    assert!(app.world().get_entity(victims[1]).is_ok());
    // victims[1] died `limit` ticks before the last kill, which was 1 tick ago.
    let elapsed = limit as u32;
    run_ticks(&mut app, lifetime - 1 - elapsed);
    assert!(
        app.world().get_entity(victims[1]).is_ok(),
        "corpse removed before corpse_seconds"
    );
    run_ticks(&mut app, 1);
    assert!(
        app.world().get_entity(victims[1]).is_err(),
        "corpse outlived corpse_seconds"
    );
}

#[test]
fn npcs_live_through_wasted() {
    use gta_sim::flow::GameState;
    let mut app = headless_app();
    let c = Vec3::new(-25.0, 0.0, 25.0);
    let h = 1.5;
    test_graph(
        &mut app,
        vec![
            c + Vec3::new(-h, 0.0, -h),
            c + Vec3::new(h, 0.0, -h),
            c + Vec3::new(h, 0.0, h),
            c + Vec3::new(-h, 0.0, h),
        ],
        &[(0, 1), (1, 2), (2, 3), (3, 0)],
    );
    settle(&mut app);
    let walker = spawn_civilian(&mut app, SIDES[0], 0.5, calm());
    run_ticks(&mut app, 8);
    let tolerance = 1.0;
    let check = |app: &mut App| -> u32 {
        let w = *app.world().get::<GraphWalker>(walker).unwrap();
        let graph = app.world().resource::<gta_sim::navigation::SidewalkGraph>();
        assert!(graph.is_edge(w.from, w.to), "{w:?}");
        let (a, b) = (graph.node(w.from).xz(), graph.node(w.to).xz());
        let p = position_of(app, walker).xz();
        let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
        let off = p.distance(a + (b - a) * t);
        assert!(off <= tolerance, "walker {off} m off {w:?} during Wasted");
        w.to
    };
    write_damage(&mut app, 1000.0);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        game_state(&app),
        GameState::Wasted,
        "GATE BROKEN: not wasted"
    );
    let mut to = check(&mut app);
    let mut changes = 0;
    let mut updates = 0;
    while game_state(&app) == GameState::Wasted {
        app.update();
        updates += 1;
        assert!(updates < 5000, "GATE BROKEN: Wasted never ended");
        let now = check(&mut app);
        if now != to {
            changes += 1;
            to = now;
        }
    }
    assert!(
        changes >= 2,
        "the walker stopped following the graph during Wasted: {changes}"
    );
}
