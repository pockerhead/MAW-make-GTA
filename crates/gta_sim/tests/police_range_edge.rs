//! Cops on opposite sides of the player at the edge of weapon range (TASK-012 QA round 1, Bug 1): a
//! bullet starts at the muzzle and stops on a body's surface, so a cop whose centre is just past the
//! range along the line is still in reach and must count as a shield (shared `tactics` fire line).

mod common;
mod police_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use common::*;
use gta_sim::{
    character::Cuffed,
    combat::{AimConfig, Weapon, WeaponsConfig, muzzle},
    police::{CopState, UnitKind},
};
use police_support::*;
use std::f32::consts::FRAC_PI_2;

/// 5 s of fixed ticks: several trigger rolls (`trigger_seconds` 0.4..0.9 s).
const PAIR_TICKS: u32 = 320;

/// 40 s of fixed ticks (the QA probe's window).
const SURROUND_TICKS: u32 = 2560;

/// Whether a straight shot of `range` from `shooter`'s muzzle, aimed at `victim`'s centre at muzzle
/// height, touches `victim`'s body or head (the hitscan's layers; every other body is ignored).
fn bullet_reaches(app: &mut App, shooter: Entity, victim: Entity, range: f32) -> bool {
    let from = position_of(app, shooter);
    let to = position_of(app, victim);
    let offset = app.world().resource::<AimConfig>().muzzle_offset();
    let start = muzzle(from, (to - from).with_y(0.0), offset);
    let aim = to.with_y(start.y);
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery, owners: Query<&ColliderOf>| {
            let filter = SpatialQueryFilter::from_mask([
                gta_sim::layers::GameLayer::World,
                gta_sim::layers::GameLayer::Character,
                gta_sim::layers::GameLayer::Hitbox,
            ]);
            let only_victim = |e: Entity| owners.get(e).is_ok_and(|of| of.body == victim);
            let dir = Dir3::new(aim - start).expect("GATE BROKEN: zero aim");
            spatial
                .cast_ray_predicate(start, dir, range, true, &filter, &only_victim)
                .is_some()
        })
        .expect("GATE BROKEN: ray system failed")
}

/// Two SWAT (SMG) facing each other across the player at the origin along x, `gap` m apart centre to
/// centre, both in `Attack` and held in place by a named test mutation (`Cuffed`: the walk is zeroed,
/// so the pair stays at the boundary). Returns the shots each fired in `PAIR_TICKS` and whether a
/// straight SMG shot from the west cop reaches the east one.
fn pair_across_the_player(gap: f32) -> ([usize; 2], bool) {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    assert_shipped_police(&app);
    set_player_armor(&mut app, 1.0e6);
    raise_heat(&mut app, 180);
    let range = app.world().resource::<WeaponsConfig>().smg.range;
    assert_eq!(range, 45.0, "GATE BROKEN: shipped SMG range moved");
    assert_eq!(
        esc(&app).spec(UnitKind::Swat).gun,
        Weapon::Smg,
        "GATE BROKEN: SWAT gun moved"
    );
    let west = spawn_unit(
        &mut app,
        UnitKind::Swat,
        Vec3::new(-22.65, 0.0, 0.0),
        -FRAC_PI_2,
    );
    let east = spawn_unit(
        &mut app,
        UnitKind::Swat,
        Vec3::new(gap - 22.65, 0.0, 0.0),
        FRAC_PI_2,
    );
    for cop in [west, east] {
        app.world_mut().entity_mut(cop).insert(Cuffed);
        set_cop_state(&mut app, cop, CopState::Attack);
    }
    let reaches = bullet_reaches(&mut app, west, east, range);
    let mut shots = Shots::new(&app);
    shots.run(&mut app, PAIR_TICKS);
    for cop in [west, east] {
        let at = position_of(&app, cop);
        assert!(
            cop_state(&app, cop) == CopState::Attack && cop_sees_player(&app, cop),
            "GATE BROKEN: cop at {at} left Attack or lost the player"
        );
    }
    let fired = [west, east].map(|c| shots.shots.iter().filter(|s| s.shooter == c).count());
    (fired, reaches)
}

fn cop_state(app: &App, entity: Entity) -> CopState {
    cop(app, entity).state
}

fn cop_sees_player(app: &App, entity: Entity) -> bool {
    cop(app, entity).sees
}

/// Deterministic boundary pair. 45.3 m: each centre is past the SMG range along the line, but a shot
/// from the muzzle (0.45 m ahead) reaches the capsule surface (0.3 m before the centre), so neither
/// cop may fire. 46.2 m: out of reach (45 + 0.45 + 0.35 head < 46.2), both fire (liveness: the hold
/// above is the boundary, not a cop that never shoots).
#[test]
fn cops_hold_fire_at_the_range_edge_across_the_player() {
    let (fired, reaches) = pair_across_the_player(45.3);
    assert!(
        reaches,
        "GATE BROKEN: at 45.3 m a straight SMG shot must reach the far cop"
    );
    assert_eq!(
        fired,
        [0, 0],
        "cops 45.3 m apart across the player fired (a miss past the player hits the other cop)"
    );

    let (fired, reaches) = pair_across_the_player(46.2);
    assert!(
        !reaches,
        "GATE BROKEN: at 46.2 m a straight SMG shot must fall short of the far cop"
    );
    assert!(
        fired.iter().all(|&n| n >= 1),
        "cops out of each other's reach did not fire: {fired:?}"
    );
}

/// QA round 1 probe, committed: 12 SWAT from both ends of a street of `width` m (walls 78 m long),
/// player armoured at (0, 0, 32). Returns (cop-to-cop hits, cops that fired, hits on the player).
fn swat_street(width: f32, attack: bool) -> (usize, usize, usize) {
    let mut app = gang_floor(
        TurfLayout::WholeFloor,
        vec![Vec3::new(30.0, 0.0, -30.0), Vec3::new(35.0, 0.0, -30.0)],
        &[(0, 1)],
    );
    assert_shipped_police(&app);
    set_player_armor(&mut app, 1.0e6);
    place_player(&mut app, Vec3::new(0.0, 0.0, 32.0));
    run_ticks(&mut app, 8);
    raise_heat(&mut app, 2400);
    let h = width / 2.0 + 0.25;
    for z in [32.0 - h, 32.0 + h] {
        spawn_wall(&mut app, Vec3::new(0.0, 2.0, z), Vec3::new(78.0, 4.0, 0.5));
    }
    let mut cops = Vec::new();
    for k in 0..6 {
        let x = 22.0 + 3.0 * k as f32;
        let dz = if k % 2 == 0 { -1.0 } else { 1.0 };
        let east = Vec3::new(x, 0.0, 32.0 + dz);
        let west = Vec3::new(-x, 0.0, 32.0 - dz);
        cops.push(spawn_unit(&mut app, UnitKind::Swat, east, FRAC_PI_2));
        cops.push(spawn_unit(&mut app, UnitKind::Swat, west, -FRAC_PI_2));
    }
    if attack {
        for &c in &cops {
            set_cop_state(&mut app, c, CopState::Attack);
        }
    }
    let me = player(&mut app);
    let mut shots = Shots::new(&app);
    shots.run(&mut app, SURROUND_TICKS);
    let friendly: Vec<_> = shots
        .dealt_log
        .iter()
        .filter(|d| cops.contains(&d.shooter) && cops.contains(&d.target))
        .collect();
    for d in &friendly {
        println!(
            "street {width} m: {:?} hit cop {:?} at {} for {}",
            d.shooter, d.target, d.point, d.damage
        );
    }
    let firing = cops
        .iter()
        .filter(|&&c| shots.shots.iter().any(|s| s.shooter == c))
        .count();
    let on_player = shots.dealt_log.iter().filter(|d| d.target == me).count();
    println!(
        "12 SWAT, street {width} m, start {}: friendly {}, firing {firing}/12, hits on player {on_player}",
        if attack { "Attack" } else { "Respond" },
        friendly.len()
    );
    (friendly.len(), firing, on_player)
}

/// Cops of the 12 that fire at least once in 40 s (liveness: zero cop-to-cop hits must not come from
/// a squad that stopped shooting). Measured 10-11 at every width here (QA before the fix: 11); the
/// idle rest is the accepted "single file" risk at this scale (QA round 1, Bug 3).
const MIN_FIRING: usize = 9;

fn assert_no_cop_on_cop(width: f32, attack: bool) {
    let (friendly, firing, on_player) = swat_street(width, attack);
    assert_eq!(friendly, 0, "street {width} m: cop-to-cop hits");
    assert!(
        firing >= MIN_FIRING && on_player >= 1,
        "street {width} m: the squad barely fought ({firing}/12 fired, {on_player} hits on the player)"
    );
}

// Clean before the fix too (QA probe and the flip: 0 hits): a safety case only.
#[test]
fn surrounding_swat_never_hit_each_other_street_8() {
    assert_no_cop_on_cop(8.0, true);
}

#[test]
fn surrounding_swat_never_hit_each_other_street_10() {
    assert_no_cop_on_cop(10.0, true);
}

#[test]
fn surrounding_swat_never_hit_each_other_street_11() {
    assert_no_cop_on_cop(11.0, true);
}

#[test]
fn surrounding_swat_never_hit_each_other_street_14() {
    assert_no_cop_on_cop(14.0, true);
}

#[test]
fn surrounding_swat_never_hit_each_other_street_11_from_respond() {
    assert_no_cop_on_cop(11.0, false);
}
