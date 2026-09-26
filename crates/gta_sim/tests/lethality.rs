//! Lethality balance (TASK-035), production composition on the test floor: 1-star cops shoot only a
//! player who attacked police (GDD §6.4 table, row 1), and the time a standing unarmoured player lasts
//! against two patrol cops and a gang trio.

mod common;
mod police_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::{ActionIntent, AimIntent, Dead},
    combat::{
        AimConfig, CombatRng, DamageScale, Loadout, MeleeHit, Weapon, WeaponsConfig, acquire,
        muzzle,
    },
    gang::GangRng,
    police::{CopState, EscalationConfig, PoliceAlert, PoliceRng, UnitKind},
};
use police_support::*;
use std::f32::consts::PI;
use wanted_support::*;

const SEEDS: [u64; 7] = [1, 2, 3, 4, 5, 6, 7];
/// Floor of the police time-to-kill targets (task decision 2), s.
const POLICE_TTK: f32 = 8.0;
/// Floor of the gang time-to-kill target (task decision 2), s.
const GANG_TTK: f32 = 6.0;
/// Ceiling: a standing unarmoured player still dies to any of these groups within this, s.
const TTK_CEILING: f32 = 30.0;
/// A run stops here; a player alive at the cap counts as surviving the cap.
const TTK_CAP: f32 = 40.0;

/// Two patrol cops 12.4 m in front of the player, facing him, in the clear part of the test floor.
const COP_SPOTS: [Vec3; 2] = [Vec3::new(-3.0, 0.0, -12.0), Vec3::new(3.0, 0.0, -12.0)];

/// Twelve spots in two rows 12 and 15 m in front of the player, clear of the ramp (x < -8) and the
/// steps and box (x > 8.5); the full 2-star row takes the four in the middle of the first row.
const ROW_SPOTS: [Vec3; 12] = [
    Vec3::new(-4.5, 0.0, -12.0),
    Vec3::new(-1.5, 0.0, -12.0),
    Vec3::new(1.5, 0.0, -12.0),
    Vec3::new(4.5, 0.0, -12.0),
    Vec3::new(-7.5, 0.0, -12.0),
    Vec3::new(7.5, 0.0, -12.0),
    Vec3::new(-6.0, 0.0, -15.0),
    Vec3::new(-3.6, 0.0, -15.0),
    Vec3::new(-1.2, 0.0, -15.0),
    Vec3::new(1.2, 0.0, -15.0),
    Vec3::new(3.6, 0.0, -15.0),
    Vec3::new(6.0, 0.0, -15.0),
];

fn reseed(app: &mut App, seed: u64) {
    app.world_mut().insert_resource(PoliceRng::seeded(seed));
    app.world_mut().insert_resource(CombatRng::seeded(seed));
    app.world_mut().insert_resource(GangRng::seeded(seed));
}

fn police_floor(heat: u32, seed: u64) -> (App, Vec<Entity>) {
    police_group(heat, seed, UnitKind::Patrol, &COP_SPOTS)
}

fn police_group(heat: u32, seed: u64, kind: UnitKind, spots: &[Vec3]) -> (App, Vec<Entity>) {
    let mut app = graph_app(10.0, &[]);
    assert_shipped_police(&app);
    reseed(&mut app, seed);
    raise_heat(&mut app, heat);
    let cops = spots
        .iter()
        .map(|&spot| spawn_unit(&mut app, kind, spot, PI))
        .collect();
    (app, cops)
}

/// Seconds from the first shot of `shooters` to the player's death (`TTK_CAP` if he lives that long
/// after it); `hold` runs before every tick (a named mutation that keeps the fight going).
fn time_to_kill(app: &mut App, shooters: &[Entity], hold: impl Fn(&mut App)) -> f32 {
    assert_eq!(
        health(app).armor,
        0.0,
        "GATE BROKEN: the player wears armour"
    );
    let me = player(app);
    let mut shots = Shots::new(app);
    let mut first = None;
    let cap = (TTK_CAP * 64.0) as u32;
    for tick in 0..cap + 20 * 64 {
        hold(app);
        shots.run(app, 1);
        if first.is_none() && shots.shots.iter().any(|s| shooters.contains(&s.shooter)) {
            first = Some(tick);
        }
        let Some(first) = first else {
            continue;
        };
        if app.world().get::<Dead>(me).is_some() {
            return (tick - first) as f32 / 64.0;
        }
        if tick - first >= cap {
            return TTK_CAP;
        }
    }
    panic!("GATE BROKEN: nobody opened fire in 20 s");
}

fn median(mut values: Vec<f32>) -> f32 {
    values.sort_by(f32::total_cmp);
    values[values.len() / 2]
}

fn police_ttk(heat: u32, held_hostile: bool) -> Vec<f32> {
    group_ttk(heat, held_hostile, UnitKind::Patrol, &COP_SPOTS)
}

fn group_ttk(heat: u32, held_hostile: bool, kind: UnitKind, spots: &[Vec3]) -> Vec<f32> {
    SEEDS
        .iter()
        .map(|&seed| {
            let (mut app, cops) = police_group(heat, seed, kind, spots);
            time_to_kill(&mut app, &cops, |app| {
                if held_hostile {
                    // Named mutation: the player attacked police just now (the alert never lapses).
                    app.world_mut().resource_mut::<PoliceAlert>().hostile_left = 1.0e6;
                }
            })
        })
        .collect()
}

fn assert_ttk(name: &str, ttk: Vec<f32>, floor: f32) {
    let m = median(ttk.clone());
    println!("{name}: median TTK {m:.2} s over seeds {SEEDS:?}: {ttk:.2?}");
    assert!(
        m >= floor,
        "{name}: median TTK {m:.2} s < {floor} s ({ttk:.2?})"
    );
    assert!(
        m <= TTK_CEILING,
        "{name}: median TTK {m:.2} s > {TTK_CEILING} s ({ttk:.2?})"
    );
}

#[test]
fn two_one_star_cops_after_an_attack_take_8_s() {
    assert_ttk(
        "1-star patrol pair, hostile",
        police_ttk(40, true),
        POLICE_TTK,
    );
}

#[test]
fn two_two_star_cops_take_8_s() {
    assert_ttk("2-star patrol pair", police_ttk(180, false), POLICE_TTK);
}

/// The full 2-star row (4 patrol cops) and the 5-star row (12 SWAT, SMG; unit counts pinned by
/// `assert_shipped_police`) through the real AI: only the ceiling, the floors are the 2-cop gates.
#[test]
fn the_full_two_star_row_kills_within_30_s() {
    let ttk = group_ttk(180, false, UnitKind::Patrol, &ROW_SPOTS[..4]);
    assert_ttk("full 2-star row (4 patrol)", ttk, 0.0);
}

#[test]
fn the_full_five_star_row_kills_within_30_s() {
    let ttk = group_ttk(2400, false, UnitKind::Swat, &ROW_SPOTS);
    assert_ttk("full 5-star row (12 SWAT)", ttk, 0.0);
}

/// Three members of `gang` 1.5 m apart, 10 m in front of the player, provoked; the guns cycle through
/// the gang's arsenal starting at `seed`.
fn gang_ttk(gang: u8) -> Vec<f32> {
    SEEDS
        .iter()
        .map(|&seed| {
            let mut app = gang_floor_default(TurfLayout::WholeFloor);
            reseed(&mut app, seed);
            let arsenal = gang_cfg(&app).gangs[usize::from(gang)].weapons.clone();
            let members: Vec<Entity> = (0..3)
                .map(|i| {
                    let gun = arsenal[(seed as usize + i) % arsenal.len()];
                    let spot = Vec3::new(-1.5 + 1.5 * i as f32, 0.0, -10.0);
                    spawn_member(&mut app, gang, spot, gun)
                })
                .collect();
            run_ticks(&mut app, 1);
            for &m in &members {
                provoke(&mut app, m);
            }
            time_to_kill(&mut app, &members, |_| {})
        })
        .collect()
}

#[test]
fn a_gang_trio_at_10_m_takes_6_s() {
    for gang in 0..2 {
        assert_ttk(
            &format!("gang {gang} trio at 10 m"),
            gang_ttk(gang),
            GANG_TTK,
        );
    }
}

// ---------------------------------------------------------------- 1-star fire rule

/// `police_floor` at 1 star, the player armed, a civilian held idle behind him at (0, 0, 10); both
/// cops go for the arrest.
fn one_star_scene() -> (App, Vec<Entity>, Entity) {
    let (mut app, cops) = police_floor(40, 0);
    arm(&mut app, Weapon::Pistol);
    let civilian = spawn_civilian(&mut app, SIDES[2], 0.5, calm());
    hold_idle(&mut app, civilian);
    for _ in 0..8 {
        run_ticks(&mut app, 1);
        if cops.iter().all(|&c| cop(&app, c).state == CopState::Arrest) {
            return (app, cops, civilian);
        }
    }
    panic!("GATE BROKEN: the cops did not go for the arrest");
}

fn police_shots(probe: &Probe, cops: &[Entity]) -> usize {
    probe
        .shots
        .shots
        .iter()
        .filter(|s| cops.contains(&s.shooter))
        .count()
}

#[test]
fn one_star_cops_arrest_a_civilian_shooter() {
    let (mut app, cops, civilian) = one_star_scene();
    let mut probe = Probe::new(&app);
    let at = position_of(&app, civilian);
    let attack = fire_at(&mut app, &mut probe, at);
    assert!(
        probe
            .shots
            .dealt_log
            .iter()
            .any(|d| d.shot == attack && d.target == civilian),
        "GATE BROKEN: the shot missed the civilian"
    );
    let mut attempted = None;
    for tick in 0..640 {
        probe.run(&mut app, 1);
        assert_eq!(
            wanted(&app).stars,
            1,
            "GATE BROKEN: left the 1-star row: {:?}",
            wanted(&app)
        );
        for &c in &cops {
            assert_ne!(
                cop(&app, c).state,
                CopState::Attack,
                "tick {tick}: a 1-star cop attacks a civilian shooter"
            );
        }
        if attempt(&app).cop.is_some() {
            attempted = Some(tick);
            break;
        }
    }
    let attempted = attempted.expect("no arrest attempt within 10 s of the shot");
    println!("arrest attempt {attempted} ticks after the shot");
    assert_eq!(
        police_shots(&probe, &cops),
        0,
        "1-star cops fired at a player who shot a civilian"
    );
}

/// Runs until a cop fires, at most 2 s; returns the tick.
fn await_police_fire(app: &mut App, probe: &mut Probe, cops: &[Entity]) -> Option<u32> {
    (0..128).find(|_| {
        probe.run(app, 1);
        police_shots(probe, cops) > 0
    })
}

#[test]
fn one_star_cops_fire_at_a_player_who_shoots_a_cop() {
    let (mut app, cops, _) = one_star_scene();
    set_player_armor(&mut app, 1.0e6);
    let mut probe = Probe::new(&app);
    let at = position_of(&app, cops[0]);
    let attack = fire_at(&mut app, &mut probe, at);
    assert!(
        probe
            .shots
            .dealt_log
            .iter()
            .any(|d| d.shot == attack && d.target == cops[0]),
        "GATE BROKEN: the shot missed the cop"
    );
    let fired = await_police_fire(&mut app, &mut probe, &cops);
    assert!(fired.is_some(), "1-star cops did not fire at a cop shooter");
}

#[test]
fn one_star_cops_fire_at_a_player_who_shoots_past_a_cop() {
    let (mut app, cops, _) = one_star_scene();
    set_player_armor(&mut app, 1.0e6);
    let mut probe = Probe::new(&app);
    // 1.0 m beside the cop's chest, away from the other cop: a miss inside `near_miss_distance`.
    let past = position_of(&app, cops[0]) - Vec3::X;
    let attack = fire_at(&mut app, &mut probe, past);
    assert!(
        !probe.shots.dealt_log.iter().any(|d| d.shot == attack),
        "GATE BROKEN: the near miss hit somebody"
    );
    let fired = await_police_fire(&mut app, &mut probe, &cops);
    assert!(
        fired.is_some(),
        "1-star cops did not fire at a player shooting at them"
    );
}

#[test]
fn one_star_cops_fire_at_a_player_who_punches_a_cop() {
    let (mut app, cops, _) = one_star_scene();
    set_player_armor(&mut app, 1.0e6);
    set_loadout(&mut app, |l| l.held = None);
    let mut probe = Probe::new(&app);
    let near = (0..640)
        .find_map(|_| {
            probe.run(&mut app, 1);
            let me = position(&mut app);
            cops.iter()
                .copied()
                .find(|&c| (position_of(&app, c) - me).with_y(0.0).length() <= 1.4)
        })
        .expect("GATE BROKEN: no cop came within punching reach");
    let origin = position(&mut app);
    let target = position_of(&app, near);
    set_aim(&mut app, origin, target);
    let mut hits = app
        .world()
        .resource::<Messages<MeleeHit>>()
        .get_cursor_current();
    set_action(&mut app, |a| a.fire_requested = true);
    let landed = (0..24).any(|_| {
        probe.run(&mut app, 1);
        let messages = app.world().resource::<Messages<MeleeHit>>();
        hits.read(messages).any(|h| h.target == near)
    });
    assert!(landed, "GATE BROKEN: the punch never landed");
    assert_eq!(
        police_shots(&probe, &cops),
        0,
        "GATE BROKEN: police fired before the punch"
    );
    let fired = await_police_fire(&mut app, &mut probe, &cops);
    assert!(fired.is_some(), "1-star cops did not fire at a cop puncher");
}

/// A civilian stands 2.5 m in front of a cop, on the line from the player: the shot stops on the
/// civilian, more than `arrest.near_miss_distance` short of the cop, so it is not an attack on police.
#[test]
fn one_star_cops_arrest_a_player_who_shoots_a_civilian_in_front_of_them() {
    let (mut app, cops, _) = one_star_scene();
    let me = position(&mut app);
    let cop_at = position_of(&app, cops[0]);
    let toward = (me - cop_at).with_y(0.0).normalize();
    let shield = spawn_dummy(&mut app, (cop_at + toward * 2.5).with_y(0.0));
    let mut probe = Probe::new(&app);
    probe.run(&mut app, 1);
    let at = position_of(&app, shield);
    let gap = (position_of(&app, cops[0]) - at).with_y(0.0).length();
    assert!(
        gap >= 2.0,
        "GATE BROKEN: the civilian stands {gap:.2} m from the cop"
    );
    let attack = fire_at(&mut app, &mut probe, at);
    assert!(
        probe
            .shots
            .dealt_log
            .iter()
            .any(|d| d.shot == attack && d.target == shield),
        "GATE BROKEN: the shot missed the civilian"
    );
    let end = probe
        .shots
        .trace_log
        .iter()
        .find(|t| t.attack == attack)
        .expect("GATE BROKEN: no trace")
        .to;
    println!(
        "trace ends {:.2} m from the cop",
        end.distance(position_of(&app, cops[0]))
    );
    for tick in 0..128 {
        probe.run(&mut app, 1);
        for &c in &cops {
            assert_ne!(
                cop(&app, c).state,
                CopState::Attack,
                "tick {tick}: a 1-star cop attacks a player who shot the civilian in front of him"
            );
        }
    }
    assert_eq!(
        police_shots(&probe, &cops),
        0,
        "1-star cops fired at a player who shot a civilian in front of them"
    );
}

// ---------------------------------------------------------------- victim scope of DamageScale

/// A dummy with a pistol and the 2-star police `DamageScale` fires one shot at `target`.
fn dummy_fires_at(app: &mut App, shooter: Entity, target: Vec3) -> Shots {
    let offset = app.world().resource::<AimConfig>().muzzle_offset();
    let body = position_of(app, shooter);
    let (mut from, mut dir) = (body, Vec3::NEG_Z);
    for _ in 0..4 {
        dir = (target - from).normalize();
        from = muzzle(body, dir, offset);
    }
    let mut aim = app.world_mut().get_mut::<AimIntent>(shooter).unwrap();
    aim.origin = from;
    aim.direction = dir;
    aim.aiming = true;
    let mut shots = Shots::new(app);
    app.world_mut()
        .get_mut::<ActionIntent>(shooter)
        .unwrap()
        .fire_requested = true;
    shots.run(app, 4);
    // Past the pistol cooldown before the next shot.
    run_ticks(app, 32);
    shots
}

/// NPC lethality is scaled by victim: the police `DamageScale` cuts a bullet on the player, a stray on
/// a civilian keeps the gun's full damage.
#[test]
fn police_damage_scale_applies_only_to_the_player() {
    let mut app = graph_app(10.0, &[]);
    let scale = app.world().resource::<EscalationConfig>().stars[1].damage_scale;
    let pistol = app.world().resource::<WeaponsConfig>().pistol.clone();
    let shooter = spawn_dummy(&mut app, Vec3::new(0.0, 0.0, -8.0));
    let bystander = spawn_dummy(&mut app, Vec3::new(4.0, 0.0, -8.0));
    let mut loadout = Loadout::default();
    acquire(&mut loadout.guns[Weapon::Pistol.index()], &pistol, true);
    loadout.held = Some(Weapon::Pistol);
    app.world_mut()
        .entity_mut(shooter)
        .insert((loadout, DamageScale(scale)));
    run_ticks(&mut app, 32);
    let me = player(&mut app);
    let dealt = |shots: &Shots, target: Entity| {
        let hits: Vec<u32> = shots
            .dealt_log
            .iter()
            .filter(|d| d.shooter == shooter && d.target == target)
            .map(|d| d.damage)
            .collect();
        assert_eq!(hits.len(), 1, "GATE BROKEN: hits on {target}: {hits:?}");
        hits[0]
    };
    let at = position_of(&app, bystander);
    let on_civilian = dealt(&dummy_fires_at(&mut app, shooter, at), bystander);
    let at = position(&mut app);
    let on_player = dealt(&dummy_fires_at(&mut app, shooter, at), me);
    println!("scale {scale}: civilian took {on_civilian}, player took {on_player}");
    let v = pistol.damage_variance;
    assert!(
        on_civilian as f32 >= (pistol.damage * (1.0 - v)).floor(),
        "a stray on a civilian was scaled: {on_civilian}"
    );
    assert!(
        on_player as f32 <= (pistol.damage * scale * (1.0 + v)).ceil(),
        "a police bullet on the player was not scaled: {on_player}"
    );
}
