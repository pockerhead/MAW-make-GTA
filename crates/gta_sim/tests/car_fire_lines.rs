//! Cars in the line of fire (O1, TASK-015 note, T15): a cop or a gunman whose segment to the target
//! passes a non-target car holds fire and repositions, like for any spared body; a car beside the
//! segment (no spread cone for cars), behind the target, or the car the target drives never stops it.
//! Test floor, production composition; the player at the origin, the shooter 20 m along −Z.

mod common;
mod police_support;
mod vehicle_support;
mod wanted_support;

use bevy::{ecs::message::MessageCursor, prelude::*};
use common::*;
use gta_sim::{
    combat::{BulletHitVehicle, Weapon, WeaponsConfig},
    police::{CopState, EscalationConfig, UnitKind},
};
use police_support::*;
use std::f32::consts::PI;
use vehicle_support::*;

const FIGHT_TICKS: u32 = 1920;
const SHOOTER: Vec3 = Vec3::new(0.0, 0.0, -20.0);

#[derive(Clone, Copy)]
enum Role {
    Cop,
    Gunman,
}

struct Fight {
    shots: usize,
    hits_on_car: usize,
    player_hits: usize,
    /// Farthest the shooter got sideways (across the line), m.
    moved: f32,
}

/// A broadside car (long axis along x) centred at `(x, z)`, or the player driving it at the origin.
fn fight(role: Role, car_at: Option<Vec2>, drive: bool) -> Fight {
    let mut app = gang_floor_default(TurfLayout::WholeFloor);
    set_player_armor(&mut app, 1.0e6);
    if drive {
        // The player steps aside for the car to spawn, then gets in.
        let float = app
            .world()
            .resource::<gta_sim::character::LocomotionConfig>()
            .float_height;
        place_player(&mut app, Vec3::new(6.0, float, 6.0));
        run_ticks(&mut app, 2);
    }
    let car = car_at.map(|at| spawn_car(&mut app, at, 90.0));
    if let (true, Some(car)) = (drive, car) {
        drive_in(&mut app, car);
    }
    let shooter = match role {
        Role::Cop => {
            raise_heat(&mut app, 180);
            let cop = spawn_unit(&mut app, UnitKind::Patrol, SHOOTER, PI);
            set_cop_state(&mut app, cop, CopState::Attack);
            cop
        }
        Role::Gunman => {
            let m = spawn_member(&mut app, 0, SHOOTER, Weapon::Pistol);
            provoke(&mut app, m);
            m
        }
    };
    let start = position_of(&app, shooter);
    let me = player(&mut app);
    let mut shots = Shots::new(&app);
    let mut hits: MessageCursor<BulletHitVehicle> = app
        .world()
        .resource::<Messages<BulletHitVehicle>>()
        .get_cursor_current();
    let mut hits_on_car = 0;
    let mut moved: f32 = 0.0;
    for _ in 0..FIGHT_TICKS {
        shots.run(&mut app, 1);
        let messages = app.world().resource::<Messages<BulletHitVehicle>>();
        hits_on_car += hits
            .read(messages)
            .filter(|h| Some(h.vehicle) == car && h.shooter == shooter)
            .count();
        moved = moved.max((position_of(&app, shooter).x - start.x).abs());
    }
    Fight {
        shots: shots.shots.iter().filter(|s| s.shooter == shooter).count(),
        hits_on_car,
        player_hits: shots
            .dealt_log
            .iter()
            .filter(|d| d.shooter == shooter && d.target == me)
            .count(),
        moved,
    }
}

/// Centre x of a broadside car whose near corner is `off` m beside the line x = 0.
fn corner_off(off: f32, side: f32) -> f32 {
    side * (2.04 + off)
}

/// Car hits over the six corner cases with the car rule off (empty car list), measured in TASK-016:
/// 24 of 252 shots. Cars block only the segment with plain clearance (no spread cone, PLAN R7), so a
/// repositioned line 0.5 m clear of a corner still loses some pellets to spread: the gate holds the
/// waste under half of the unruled count, not at zero.
const UNRULED_CAR_HITS: usize = 24;
/// The same six cases for a gang gunman (aim error 4°), rule off: 17 of 144 shots.
const UNRULED_GANG_CAR_HITS: usize = 17;

#[test]
fn a_car_corner_in_the_line_is_not_shot_at() {
    let mut car_hits = 0;
    for side in [1.0, -1.0] {
        for z in [-6.0, -10.0, -14.0] {
            let at = Vec2::new(corner_off(0.4, side), z);
            let f = fight(Role::Cop, Some(at), false);
            println!(
                "corner at {at}: shots {}, hits on the car {}, hits on the player {}, moved {:.1}",
                f.shots, f.hits_on_car, f.player_hits, f.moved
            );
            assert!(
                f.player_hits >= 1,
                "car at {at}: the cop never hit the player"
            );
            car_hits += f.hits_on_car;
        }
    }
    assert!(
        car_hits * 2 <= UNRULED_CAR_HITS,
        "{car_hits} pellets into the cars (unruled: {UNRULED_CAR_HITS})"
    );
}

#[test]
fn a_gunman_does_not_shoot_a_car_in_the_line() {
    let mut car_hits = 0;
    for side in [1.0, -1.0] {
        for z in [-6.0, -10.0, -14.0] {
            let at = Vec2::new(corner_off(0.4, side), z);
            let f = fight(Role::Gunman, Some(at), false);
            println!(
                "gunman, corner at {at}: shots {}, hits on the car {}, hits on the player {}",
                f.shots, f.hits_on_car, f.player_hits
            );
            assert!(
                f.player_hits >= 1,
                "car at {at}: the gunman never hit the player"
            );
            car_hits += f.hits_on_car;
        }
    }
    println!("gunman: {car_hits} pellets into the cars");
    assert!(
        car_hits * 2 <= UNRULED_GANG_CAR_HITS,
        "{car_hits} pellets into the cars (unruled: {UNRULED_GANG_CAR_HITS})"
    );
}

#[test]
fn the_car_the_target_drives_is_shot() {
    // The player sits in a broadside car at the origin: its body is the target's cover, not a
    // bystander's car.
    let f = fight(Role::Cop, Some(Vec2::new(0.0, 0.0)), true);
    println!(
        "driven car: shots {}, hits on it {}",
        f.shots, f.hits_on_car
    );
    assert!(
        f.hits_on_car >= 1,
        "the cop never fired at the player's car"
    );
}

#[test]
fn a_car_beside_the_segment_does_not_block() {
    let esc = {
        let app = headless_app();
        app.world().resource::<EscalationConfig>().clone()
    };
    let weapons = {
        let app = headless_app();
        app.world().resource::<WeaponsConfig>().clone()
    };
    // The band where the cone rule would block and the segment rule does not.
    let clearance = 0.3 + esc.combat.fire_line_margin;
    let spread = weapons.pistol.spread.base_deg + weapons.pistol.spread.max_bloom_deg;
    let cone = (esc.combat.aim_error_deg + spread).to_radians().tan();
    let off = 1.0;
    assert!(
        clearance < off && off < clearance + 10.0 * cone,
        "GATE BROKEN: empty band ({clearance} < {off} < {})",
        clearance + 10.0 * cone
    );
    let f = fight(
        Role::Cop,
        Some(Vec2::new(corner_off(off, 1.0), -10.0)),
        false,
    );
    println!("beside: shots {}, moved {:.2}", f.shots, f.moved);
    assert!(f.shots >= 6, "the cop held fire for a car beside its line");
    assert!(f.moved < 0.5, "the cop repositioned ({} m)", f.moved);
}

#[test]
fn a_car_behind_the_target_does_not_block() {
    let f = fight(Role::Cop, Some(Vec2::new(0.0, 3.0 + 1.2)), false);
    println!("behind: shots {}, moved {:.2}", f.shots, f.moved);
    assert!(
        f.shots >= 6,
        "the cop held fire for a car behind the player"
    );
    assert!(
        f.moved < 0.5,
        "the cop repositioned for a car behind the player ({} m)",
        f.moved
    );
}
