//! Gang hostility on the flat test floor (production composition): neutrality outside the turf,
//! warning, group aggro, `GangHeat`, the faction matrix (GDD §6.3).

mod common;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{ActionIntent, AimIntent, LocomotionConfig},
    combat::{DamageDealt, Melee, Weapon},
    flow::GameState,
    gang::{Faction, GangState, GangTerritories, PlayerTerritory},
    perception::PerceptionConfig,
};

const TICK_HZ: f32 = 64.0;

fn loco(app: &App) -> LocomotionConfig {
    app.world().resource::<LocomotionConfig>().clone()
}

fn slots(app: &App) -> u32 {
    u32::from(app.world().resource::<PerceptionConfig>().slots)
}

/// Player feet at `feet`.
fn put_player(app: &mut App, feet: Vec3) {
    let float = loco(app).float_height;
    place_player(app, feet + Vec3::Y * float);
}

fn eyes_of(app: &App, chest: Vec3) -> Vec3 {
    let l = loco(app);
    chest - Vec3::Y * l.float_height + Vec3::Y * l.head_height
}

fn player_eyes(app: &mut App) -> Vec3 {
    let chest = position(app);
    eyes_of(app, chest)
}

/// The player aims from its eyes at `target` with `aiming` (RMB) set as given.
fn player_aims(app: &mut App, target: Vec3, aiming: bool) {
    let eyes = player_eyes(app);
    set_aim(app, eyes, target);
    let entity = player(app);
    app.world_mut().get_mut::<AimIntent>(entity).unwrap().aiming = aiming;
}

fn player_territory(app: &App) -> Option<u8> {
    app.world().resource::<PlayerTerritory>().0
}

fn fire(app: &mut App) {
    set_action(app, |a| a.fire_requested = true);
}

/// Ticks one at a time until `found` matches a new message of `shots`, at most `limit` ticks.
fn tick_until(
    app: &mut App,
    shots: &mut Shots,
    limit: u32,
    found: impl Fn(&Shots) -> bool,
) -> Option<u32> {
    (1..=limit).find(|_| {
        shots.run(app, 1);
        found(shots)
    })
}

#[test]
fn outside_the_territory_members_stay_neutral() {
    let mut app = gang_floor_default(TurfLayout::WestHalf);
    let a = spawn_member(&mut app, 0, Vec3::new(-4.0, 0.0, -10.0), Weapon::Pistol);
    let b = spawn_member(&mut app, 0, Vec3::new(-2.0, 0.0, -12.0), Weapon::Pistol);
    let hostility = gang_cfg(&app).hostility;
    put_player(&mut app, Vec3::new(3.0, 0.0, -10.0));
    run_ticks(&mut app, 1);
    let a_chest = position_of(&app, a);
    assert!(
        (position(&mut app) - a_chest).with_y(0.0).length() < hostility.warn_distance,
        "GATE BROKEN: the player must stand within warn_distance of A"
    );
    // (a) Outside the turf: close, aiming at A, then a shot into the air.
    player_aims(&mut app, a_chest, true);
    let ticks = (hostility.warn_seconds * TICK_HZ) as u32 + slots(&app) + 8;
    for k in 1..=ticks {
        run_ticks(&mut app, 1);
        assert_eq!(player_territory(&app), None);
        assert_eq!(gang_state(&app, a), GangState::Idle, "A at tick {k}");
        assert_eq!(gang_state(&app, b), GangState::Idle, "B at tick {k}");
    }
    assert_eq!((heat(&app, 0), heat(&app, 1)), (0.0, 0.0));
    let up = player_eyes(&mut app) + Vec3::Y * 10.0;
    player_aims(&mut app, up, true);
    fire(&mut app);
    let mut shots = Shots::new(&app);
    shots.run(&mut app, 4);
    assert_eq!(
        shots.shots.len(),
        1,
        "GATE BROKEN: the shot into the air did not fire"
    );
    let muzzle = shots.shots[0].muzzle;
    assert_eq!(
        app.world()
            .resource::<GangTerritories>()
            .territory_at(muzzle),
        None,
        "GATE BROKEN: the muzzle must be outside the turf"
    );
    for m in [a, b] {
        assert!(position_of(&app, m).distance(muzzle) <= hostility.shot_radius);
        assert_eq!(
            gang_state(&app, m),
            GangState::Idle,
            "shot outside the turf"
        );
    }
    assert_eq!(heat(&app, 0), 0.0);

    // (b) Positive control: inside the turf, not aiming.
    put_player(&mut app, Vec3::new(-6.0, 0.0, -4.0));
    let up = player_eyes(&mut app) + Vec3::Y * 10.0;
    player_aims(&mut app, up, false);
    let warn_tick = (hostility.warn_seconds * TICK_HZ) as u32;
    run_ticks(&mut app, warn_tick - 1);
    assert_eq!(player_territory(&app), Some(0));
    assert_eq!(
        gang_state(&app, a),
        GangState::Idle,
        "tick {}",
        warn_tick - 1
    );
    run_ticks(&mut app, 1);
    assert_eq!(gang_state(&app, a), GangState::Warn, "tick {warn_tick}");
    assert_eq!(gang_state(&app, b), GangState::Idle, "B is 8.94 m away");
    fire(&mut app);
    let mut shots = Shots::new(&app);
    tick_until(&mut app, &mut shots, 4, |s| !s.shots.is_empty())
        .expect("GATE BROKEN: the second shot did not fire");
    let target = player(&mut app);
    assert_eq!(gang_state(&app, a), GangState::Attack { target });
    assert_eq!(gang_state(&app, b), GangState::Attack { target });
    assert_eq!(heat(&app, 0), hostility.heat_seconds);
}

#[test]
fn aiming_at_a_member_warns_within_one_cycle() {
    for aiming in [true, false] {
        let mut app = gang_floor_default(TurfLayout::WholeFloor);
        let a = spawn_member(&mut app, 0, Vec3::new(0.0, 0.0, -10.0), Weapon::Pistol);
        run_ticks(&mut app, 1);
        let chest = position_of(&app, a);
        player_aims(&mut app, chest, aiming);
        if aiming {
            let limit = slots(&app);
            let warned = (1..=limit).find(|_| {
                run_ticks(&mut app, 1);
                gang_state(&app, a) == GangState::Warn
            });
            assert!(warned.is_some(), "no Warn within {limit} ticks of aiming");
        } else {
            run_ticks(&mut app, 64);
            assert_eq!(gang_state(&app, a), GangState::Idle, "control: not aiming");
        }
    }
}

/// Player at (3,0,-10) outside the turf; gang 0: V (-2,0,-10), M1 (-4,0,-6), M2 (-2,0,19),
/// C (-2,0,21); gang 1: R (-4,0,-12).
#[test]
fn attack_on_a_member_aggroes_the_group_within_30m() {
    let mut app = gang_floor_default(TurfLayout::WestHalf);
    set_player_armor(&mut app, 1.0e6);
    let v = spawn_member(&mut app, 0, Vec3::new(-2.0, 0.0, -10.0), Weapon::Pistol);
    let m1 = spawn_member(&mut app, 0, Vec3::new(-4.0, 0.0, -6.0), Weapon::Pistol);
    let m2 = spawn_member(&mut app, 0, Vec3::new(-2.0, 0.0, 19.0), Weapon::Pistol);
    let c = spawn_member(&mut app, 0, Vec3::new(-2.0, 0.0, 21.0), Weapon::Pistol);
    let r = spawn_member(&mut app, 1, Vec3::new(-4.0, 0.0, -12.0), Weapon::Pistol);
    put_player(&mut app, Vec3::new(3.0, 0.0, -10.0));
    run_ticks(&mut app, 1);
    let radius = gang_cfg(&app).hostility.group_radius;
    let at = |app: &App, e| position_of(app, e);
    let (dv_m2, dv_c) = (
        (at(&app, m2) - at(&app, v)).with_y(0.0).length(),
        (at(&app, c) - at(&app, v)).with_y(0.0).length(),
    );
    assert!(
        dv_m2 < radius && radius < dv_c,
        "GATE BROKEN: |M2-V| {dv_m2} < group_radius {radius} < |C-V| {dv_c} must hold"
    );
    let v_chest = at(&app, v);
    player_aims(&mut app, v_chest, false);
    fire(&mut app);
    let shooter = player(&mut app);
    let mut shots = Shots::new(&app);
    let hit = |s: &Shots| {
        s.dealt_log
            .iter()
            .any(|d: &DamageDealt| d.shooter == shooter && d.target == v)
    };
    tick_until(&mut app, &mut shots, 4, hit).expect("GATE BROKEN: the shot did not hit V");
    assert_eq!(
        player_territory(&app),
        None,
        "GATE BROKEN: shooter in the turf"
    );
    let muzzle = shots.shots[0].muzzle;
    assert_eq!(
        app.world()
            .resource::<GangTerritories>()
            .territory_at(muzzle),
        None,
        "GATE BROKEN: the muzzle must be outside the turf"
    );
    let attack = GangState::Attack { target: shooter };
    for (name, e) in [("V", v), ("M1", m1), ("M2", m2)] {
        assert_eq!(gang_state(&app, e), attack, "{name} in the hit tick");
    }
    assert_eq!(
        gang_state(&app, c),
        GangState::Idle,
        "C is beyond group_radius"
    );
    assert_eq!(gang_state(&app, r), GangState::Idle, "R is another gang");
    assert_eq!(heat(&app, 0), gang_cfg(&app).hostility.heat_seconds);
    assert_eq!(heat(&app, 1), 0.0);
    for _ in 0..slots(&app) {
        run_ticks(&mut app, 1);
        assert_eq!(gang_state(&app, c), GangState::Idle);
        assert_eq!(gang_state(&app, r), GangState::Idle);
    }
}

#[test]
fn hit_outside_the_territory_still_provokes() {
    let mut app = gang_floor_default(TurfLayout::WestHalf);
    set_player_armor(&mut app, 1.0e6);
    let a = spawn_member(&mut app, 0, Vec3::new(-4.0, 0.0, -10.0), Weapon::Pistol);
    put_player(&mut app, Vec3::new(3.0, 0.0, -10.0));
    run_ticks(&mut app, 1);
    let a_chest = position_of(&app, a);
    player_aims(&mut app, a_chest, false);
    fire(&mut app);
    let shooter = player(&mut app);
    let mut shots = Shots::new(&app);
    tick_until(&mut app, &mut shots, 4, |s| {
        s.dealt_log.iter().any(|d| d.target == a)
    })
    .expect("GATE BROKEN: the shot did not hit A");
    assert_eq!(player_territory(&app), None);
    assert_eq!(gang_state(&app, a), GangState::Attack { target: shooter });
    assert_eq!(heat(&app, 0), gang_cfg(&app).hostility.heat_seconds);
}

/// Shoots V (0,0,-10) from the player at the origin (tick T), then despawns V (named mutation).
fn shoot_and_remove(app: &mut App) {
    set_player_armor(app, 1.0e6);
    let v = spawn_member(app, 0, Vec3::new(0.0, 0.0, -10.0), Weapon::Pistol);
    run_ticks(app, 1);
    let v_chest = position_of(app, v);
    player_aims(app, v_chest, false);
    fire(app);
    let mut shots = Shots::new(app);
    tick_until(app, &mut shots, 4, |s| {
        s.dealt_log.iter().any(|d| d.target == v)
    })
    .expect("GATE BROKEN: the shot did not hit V");
    app.world_mut().despawn(v);
}

/// Runs to tick `to` counted from T.
fn advance(app: &mut App, t: &mut u32, to: u32) {
    run_ticks(app, to - *t);
    *t = to;
}

#[test]
fn gang_heat_decays_in_120s() {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    shoot_and_remove(&mut app);
    let seconds = gang_cfg(&app).hostility.heat_seconds;
    assert_eq!(
        seconds, 120.0,
        "GATE BROKEN: the exact values below assume 120 s"
    );
    assert_eq!(heat(&app, 0), 120.0, "tick T");
    let spot = Vec3::new(0.0, 0.0, -20.0);
    let mut t = 0;
    advance(&mut app, &mut t, 1);
    assert_eq!(heat(&app, 0), 120.0 - 1.0 / 64.0);
    advance(&mut app, &mut t, 3840);
    assert_eq!(heat(&app, 0), 60.0);
    advance(&mut app, &mut t, 7000);
    assert_eq!(heat(&app, 0), 10.625);
    let f = spawn_member(&mut app, 0, spot, Weapon::Pistol);
    let limit = slots(&app);
    let attacked = (1..=limit).find(|_| {
        run_ticks(&mut app, 1);
        matches!(gang_state(&app, f), GangState::Attack { .. })
    });
    assert!(
        attacked.is_some(),
        "heated gang: F attacks on sight within {limit} ticks"
    );
    t += attacked.unwrap();
    app.world_mut().despawn(f);
    advance(&mut app, &mut t, 7679);
    assert_eq!(heat(&app, 0), 1.0 / 64.0);
    advance(&mut app, &mut t, 7680);
    assert_eq!(heat(&app, 0), 0.0);
    advance(&mut app, &mut t, 7681);
    assert_eq!(heat(&app, 0), 0.0);
    let g = spawn_member(&mut app, 0, spot, Weapon::Pistol);
    for k in 0..64 {
        run_ticks(&mut app, 1);
        assert!(
            !matches!(gang_state(&app, g), GangState::Attack { .. }),
            "cooled gang: G attacked at tick {k}"
        );
    }
}

fn fixed_ticks(app: &App) -> u128 {
    let time = app.world().resource::<Time<Fixed>>();
    time.elapsed().as_nanos() / time.timestep().as_nanos()
}

#[test]
fn gang_heat_keeps_decaying_while_wasted() {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    shoot_and_remove(&mut app);
    set_player_armor(&mut app, 0.0);
    write_damage(&mut app, 1000.0);
    let entered = (0..3).find(|_| {
        app.update();
        game_state(&app) == GameState::Wasted
    });
    assert!(entered.is_some(), "GATE BROKEN: no Wasted within 3 updates");
    let heat0 = heat(&app, 0);
    let ticks0 = fixed_ticks(&app);
    let mut updates = 0;
    while updates < 40 {
        app.update();
        if game_state(&app) != GameState::Wasted {
            break;
        }
        updates += 1;
    }
    assert!(
        updates >= 20,
        "GATE BROKEN: Wasted lasted only {updates} updates"
    );
    let n = fixed_ticks(&app) - ticks0;
    assert!(n > 0, "GATE BROKEN: no fixed tick ran during Wasted");
    assert!(heat0 > 0.0, "GATE BROKEN: heat already gone");
    assert_eq!(
        heat(&app, 0),
        heat0 - n as f32 / 64.0,
        "{n} fixed ticks while wasted"
    );
}

/// One half of the matrix gate in a fresh app: A (gang 0) is about to punch B (gang 1) 1.2 m away,
/// or A (gang 1) to shoot into the air 2 m from B (gang 0); returns (app, A, B).
fn matrix_half(punch: bool, hostile: bool) -> (App, Entity, Entity) {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    put_player(&mut app, Vec3::new(20.0, 0.0, 20.0));
    set_matrix(&mut app, Faction::Gang(0), Faction::Gang(1), hostile);
    let (a, b) = if punch {
        (
            spawn_member(&mut app, 0, Vec3::new(0.0, 0.0, -10.0), Weapon::Pistol),
            spawn_member(&mut app, 1, Vec3::new(0.0, 0.0, -11.2), Weapon::Pistol),
        )
    } else {
        (
            spawn_member(&mut app, 1, Vec3::new(0.0, 0.0, -10.0), Weapon::Pistol),
            spawn_member(&mut app, 0, Vec3::new(0.0, 0.0, -12.0), Weapon::Pistol),
        )
    };
    run_ticks(&mut app, 1);
    let (pa, pb) = (position_of(&app, a), position_of(&app, b));
    let eyes = eyes_of(&app, pa);
    let direction = if punch { pb - pa } else { Vec3::Y };
    *app.world_mut().get_mut::<AimIntent>(a).unwrap() = AimIntent {
        origin: eyes,
        direction,
        aiming: false,
    };
    if !punch {
        app.world_mut()
            .get_mut::<gta_sim::combat::Loadout>(a)
            .unwrap()
            .held = Some(Weapon::Pistol);
    }
    app.world_mut()
        .get_mut::<ActionIntent>(a)
        .unwrap()
        .fire_requested = true;
    (app, a, b)
}

#[test]
fn disabled_matrix_gangs_do_not_attack_each_other() {
    // 1. Punch, matrix off: the fist meets B and does no harm (spared at impact), B stays calm.
    let (mut app, a, b) = matrix_half(true, false);
    let mut shots = Shots::new(&app);
    let mut landed = false;
    for _ in 0..30 {
        shots.run(&mut app, 1);
        let melee = app.world().get::<Melee>(a).unwrap();
        landed |= melee.swing.is_some_and(|s| s.landed);
    }
    assert!(landed, "GATE BROKEN: A's punch met no body");
    assert!(
        !shots.dealt_log.iter().any(|d| d.shooter == a),
        "punch, matrix off: B took damage {:?}",
        shots.dealt_log
    );
    for k in 0..64 {
        for e in [a, b] {
            assert!(
                !matches!(gang_state(&app, e), GangState::Attack { .. }),
                "punch, matrix off: {e} attacks at tick {k}"
            );
        }
        run_ticks(&mut app, 1);
    }
    assert_eq!((heat(&app, 0), heat(&app, 1)), (0.0, 0.0));

    // 2. Punch, matrix on (positive control).
    let (mut app, a, b) = matrix_half(true, true);
    let mut shots = Shots::new(&app);
    tick_until(&mut app, &mut shots, 30, |s| {
        s.dealt_log.iter().any(|d| d.shooter == a && d.target == b)
    })
    .expect("GATE BROKEN: A's punch did not land on B");
    assert_eq!(gang_state(&app, b), GangState::Attack { target: a });
    // Only B's gang answers: the attacker's own gang is never set on it.
    assert_eq!(gang_state(&app, a), GangState::Idle, "A is not B's gang");

    // 3. Shot, matrix off: A (gang 1) fires into the air beside B (gang 0) on gang 0 turf.
    let (mut app, a, b) = matrix_half(false, false);
    let mut shots = Shots::new(&app);
    tick_until(&mut app, &mut shots, 4, |s| {
        s.shots.iter().any(|f| f.shooter == a)
    })
    .expect("GATE BROKEN: A's shot did not fire");
    assert_eq!(shots.shots.len(), 1, "GATE BROKEN: one shot expected");
    let muzzle = shots.shots[0].muzzle;
    assert_eq!(
        app.world()
            .resource::<GangTerritories>()
            .territory_at(muzzle),
        Some(0),
        "GATE BROKEN: the shot rule must be live (muzzle on B's turf)"
    );
    assert!(
        position_of(&app, b).distance(muzzle) <= gang_cfg(&app).hostility.shot_radius,
        "GATE BROKEN: B must be within shot_radius"
    );
    for k in 0..64 {
        assert_eq!(
            gang_state(&app, b),
            GangState::Idle,
            "shot, matrix off: tick {k}"
        );
        run_ticks(&mut app, 1);
    }

    // 4. Shot, matrix on (positive control).
    let (mut app, a, b) = matrix_half(false, true);
    let mut shots = Shots::new(&app);
    tick_until(&mut app, &mut shots, 4, |s| {
        s.shots.iter().any(|f| f.shooter == a)
    })
    .expect("GATE BROKEN: A's shot did not fire");
    assert_eq!(gang_state(&app, b), GangState::Attack { target: a });
}
