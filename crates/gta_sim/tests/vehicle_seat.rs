//! Getting in and out of a car (GDD §5, T14), production composition: who controls what, no
//! latched fire, and the flow (Wasted, Busted, arrest, a vanished car) while driving.

mod common;
mod police_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::{ecs::message::MessageCursor, prelude::*};
use bevy_tnua::TnuaToggle;
use common::*;
use gta_sim::{
    character::{Dead, HeadHitbox},
    combat::{BulletHitVehicle, ShotFired, Weapon, WeaponsConfig, acquire},
    flow::GameState,
    police::{CopState, UnitKind},
    vehicle::{Driving, Vehicle},
    world::{HospitalSpawn, PoliceStationSpawn},
};
use police_support::*;
use vehicle_support::*;
use wanted_support::*;

fn head_of(app: &mut App) -> Entity {
    let me = player(app);
    app.world()
        .get::<Children>(me)
        .expect("GATE BROKEN: player has no children")
        .iter()
        .find(|c| app.world().get::<HeadHitbox>(*c).is_some())
        .expect("GATE BROKEN: player has no head hitbox")
}

/// The player is on foot: no link, all four seat components gone.
fn assert_on_foot(app: &mut App, context: &str) {
    let me = player(app);
    let head = head_of(app);
    let world = app.world();
    assert!(
        world.get::<Driving>(me).is_none(),
        "{context}: still Driving"
    );
    assert!(
        world.get::<RigidBodyDisabled>(me).is_none(),
        "{context}: body still disabled"
    );
    assert!(
        world.get::<ColliderDisabled>(me).is_none(),
        "{context}: collider still disabled"
    );
    assert!(
        world.get::<ColliderDisabled>(head).is_none(),
        "{context}: head collider still disabled"
    );
    assert!(
        world.get::<TnuaToggle>(me).is_none(),
        "{context}: TnuaToggle still set"
    );
}

fn driver_of(app: &App, car: Entity) -> Option<Entity> {
    app.world().get::<Vehicle>(car).unwrap().driver
}

fn car_app() -> (App, Entity) {
    let mut app = headless_app();
    settle(&mut app);
    let car = spawn_car(&mut app, Vec2::new(-25.0, 0.0), 0.0);
    (app, car)
}

// ---------------------------------------------------------------- G3 control ownership

#[test]
fn on_foot_throttle_does_not_move_the_car() {
    let (mut app, car) = car_app();
    let from = position_of(&app, car);
    set_drive(&mut app, |d| d.throttle = 1.0);
    run_ticks(&mut app, 64);
    let moved = flat(position_of(&app, car) - from).length();
    assert!(moved < 0.05, "a car without a driver moved {moved:.3} m");
}

#[test]
fn enter_drive_exit_hands_control_over() {
    let (mut app, car) = car_app();
    let me = player(&mut app);
    drive_in(&mut app, car);
    let head = head_of(&mut app);
    {
        let world = app.world();
        assert_eq!(driver_of(&app, car), Some(me));
        assert!(world.get::<RigidBodyDisabled>(me).is_some());
        assert_eq!(world.get::<TnuaToggle>(me), Some(&TnuaToggle::Disabled));
        assert!(world.get::<ColliderDisabled>(me).is_some());
        assert!(world.get::<ColliderDisabled>(head).is_some());
    }

    // Driving: the car answers the throttle, the walk intent does nothing.
    let from = position_of(&app, car);
    set_drive(&mut app, |d| d.throttle = 1.0);
    set_intent(&mut app, |i| i.axis = Vec2::Y);
    run_ticks(&mut app, 64);
    let moved = position_of(&app, car) - from;
    assert!(
        moved.z <= -1.5,
        "driven car moved {moved} (expected >= 1.5 m along -Z)"
    );
    assert!(moved.x.abs() < 0.3, "driven car drifted sideways {moved}");
    let cfg = vehicle_cfg(&app);
    let seat = position_of(&app, car) + rotation_of(&app, car) * cfg.seat();
    let at = position(&mut app);
    assert!(at.distance(seat) < 0.01, "driver at {at}, seat at {seat}");

    // Brake below exit_max_speed and get out on the left.
    set_intent(&mut app, |i| i.axis = Vec2::ZERO);
    set_drive(&mut app, |d| d.throttle = -1.0);
    for _ in 0..64 {
        if velocity_of(&app, car).length() < cfg.exit_max_speed {
            break;
        }
        run_ticks(&mut app, 1);
    }
    let speed = velocity_of(&app, car).length();
    assert!(
        speed > 0.5,
        "GATE BROKEN: exit row wants a rolling car, speed {speed}"
    );
    set_drive(&mut app, |d| d.throttle = 0.0);
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_on_foot(&mut app, "after exit");
    assert_eq!(driver_of(&app, car), None);
    let right = rotation_of(&app, car) * Vec3::X;
    let offset = position(&mut app) - position_of(&app, car);
    assert!(
        offset.dot(right) < -1.0,
        "exited at {offset} from the car, not on the left"
    );

    // On foot again: the throttle no longer drives the car, the walk intent moves the player.
    run_ticks(&mut app, 256);
    let car_from = position_of(&app, car);
    let me_from = position(&mut app);
    set_drive(&mut app, |d| d.throttle = 1.0);
    set_intent(&mut app, |i| i.axis = Vec2::Y);
    run_ticks(&mut app, 64);
    let car_moved = flat(position_of(&app, car) - car_from).length();
    let me_moved = flat(position(&mut app) - me_from).length();
    assert!(car_moved < 0.05, "the left car moved {car_moved:.3} m");
    assert!(me_moved >= 1.0, "the player walked only {me_moved:.2} m");
}

#[test]
fn no_exit_at_speed() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    run_ticks(&mut app, 16);
    kick(&mut app, car, 10.0);
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_eq!(driving(&mut app), Some(car), "left a car at 10 m/s");
}

#[test]
fn no_entry_beyond_the_enter_radius() {
    let (mut app, car) = car_app();
    let door = door_of(&app, car);
    let radius = vehicle_cfg(&app).enter_radius;
    // 3.0 m from the door, straight out from the left side.
    let right = rotation_of(&app, car) * Vec3::X;
    let at = door - right * (radius + 0.5);
    let float = float_height_of(&app);
    place_player(&mut app, Vec3::new(at.x, float, at.z));
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_eq!(
        driving(&mut app),
        None,
        "entered from {:.2} m",
        radius + 0.5
    );
    assert_eq!(driver_of(&app, car), None);
}

/// A driverless car rolling past at 6 m/s with its door at the player: no entry (the exit limit
/// holds both ways). Flip: drop the speed filter in `enter_exit`.
#[test]
fn no_entry_into_a_rolling_car() {
    let (mut app, car) = car_app();
    let limit = vehicle_cfg(&app).exit_max_speed;
    kick(&mut app, car, 2.0 * limit);
    let door = door_of(&app, car);
    let feet = Vec3::new(door.x, float_height_of(&app), door.z);
    place_player(&mut app, feet);
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert!(
        velocity_of(&app, car).length() > limit,
        "GATE BROKEN: the car slowed below the limit"
    );
    assert_eq!(
        driving(&mut app),
        None,
        "got into a car at {} m/s",
        2.0 * limit
    );
    assert_eq!(driver_of(&app, car), None);
}

/// A parked car that has fallen asleep answers the throttle once entered. Flip: skip the
/// `SleepingDisabled` insert in `enter`.
#[test]
fn a_sleeping_car_drives_off() {
    let (mut app, car) = car_app();
    for _ in 0..20 {
        run_ticks(&mut app, 32);
        if app.world().get::<Sleeping>(car).is_some() {
            break;
        }
    }
    assert!(
        app.world().get::<Sleeping>(car).is_some(),
        "GATE BROKEN: the parked car never slept"
    );
    drive_in(&mut app, car);
    let from = position_of(&app, car);
    set_drive(&mut app, |d| d.throttle = 1.0);
    run_ticks(&mut app, 64);
    let moved = flat(position_of(&app, car) - from).length();
    assert!(moved >= 1.5, "the woken car moved {moved:.3} m");
}

/// Walls 4 m tall (above the feet ray's start) 0.1 m off the left and right sides and a block
/// over the roof, placed after the car so its spawn check passes.
fn wall_in(app: &mut App, right: bool, roof: bool) {
    spawn_wall(app, Vec3::new(-27.0, 2.0, 0.0), Vec3::new(1.4, 4.0, 6.0));
    if right {
        spawn_wall(app, Vec3::new(-23.0, 2.0, 0.0), Vec3::new(1.4, 4.0, 6.0));
    }
    if roof {
        spawn_wall(app, Vec3::new(-25.0, 4.6, 0.0), Vec3::new(3.0, 4.8, 5.0));
    }
}

/// Left door blocked: the player gets out on the right. Flip: try only the left door.
#[test]
fn blocked_left_door_exits_on_the_right() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    wall_in(&mut app, false, false);
    run_ticks(&mut app, 16);
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_on_foot(&mut app, "left door blocked");
    let right = rotation_of(&app, car) * Vec3::X;
    let offset = position(&mut app) - position_of(&app, car);
    assert!(
        offset.dot(right) > 1.0 && offset.y.abs() < 0.2,
        "exited at {offset} from the car, not at the right door"
    );
}

/// A thin fence between the car's left side (x -26.2) and the left door point (x -26.7) that the
/// door capsule does not touch: the player must not pass through it, and gets out on the right.
/// Flip: drop the seat-to-spot path cast in `exit_spot` (QA TASK-015 B2).
#[test]
fn thin_fence_beside_the_door_is_not_walked_through() {
    for (centre_x, thickness) in [(-26.325, 0.05), (-26.40, 0.1)] {
        let (mut app, car) = car_app();
        drive_in(&mut app, car);
        spawn_wall(
            &mut app,
            Vec3::new(centre_x, 0.6, 0.0),
            Vec3::new(thickness, 1.2, 6.0),
        );
        run_ticks(&mut app, 16);
        request_vehicle(&mut app);
        run_ticks(&mut app, 1);
        assert_on_foot(&mut app, &format!("fence at x {centre_x}"));
        run_ticks(&mut app, 32);
        let right = rotation_of(&app, car) * Vec3::X;
        let offset = position(&mut app) - position_of(&app, car);
        assert!(
            offset.dot(right) > 1.0,
            "fence at x {centre_x}: exited at {offset} from the car, not at the right door"
        );
    }
}

/// A wall lower than the feet ray's start (car centre + 2 m) next to the left door: its top is no
/// exit, the player gets out on the right at ground level. Flip: no ground-level check on the
/// door candidates.
#[test]
fn low_wall_at_the_door_is_not_an_exit() {
    for height in [1.0, 2.0, 3.0] {
        let (mut app, car) = car_app();
        drive_in(&mut app, car);
        spawn_wall(
            &mut app,
            Vec3::new(-27.0, height / 2.0, 0.0),
            Vec3::new(1.4, height, 6.0),
        );
        run_ticks(&mut app, 16);
        request_vehicle(&mut app);
        run_ticks(&mut app, 1);
        assert_on_foot(&mut app, &format!("{height} m wall"));
        let right = rotation_of(&app, car) * Vec3::X;
        let at = position(&mut app);
        let offset = at - position_of(&app, car);
        let standing = float_height_of(&app);
        assert!(
            offset.dot(right) > 1.0 && (at.y - standing).abs() < 0.05,
            "{height} m wall: exited at {at} ({offset} from the car), not at the right door on \
             the ground (centre y {standing})"
        );
    }
}

/// Both doors walled in, the roof open: the player climbs out onto the car's top (the roof
/// candidate is checked against the car's top, not the ground).
#[test]
fn walled_doors_exit_onto_the_roof() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    wall_in(&mut app, true, false);
    run_ticks(&mut app, 16);
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_on_foot(&mut app, "doors walled");
    let cfg = vehicle_cfg(&app);
    let car_at = position_of(&app, car);
    let at = position(&mut app);
    let standing = car_at.y + cfg.half_extents().y + float_height_of(&app);
    assert!(
        flat(at - car_at).length() < 0.1 && (at.y - standing).abs() < 0.05,
        "exited at {at}, not on the roof of the car at {car_at} (centre y {standing})"
    );
}

/// Both doors and the roof blocked: the player stays in the seat.
#[test]
fn boxed_in_driver_stays_seated() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    wall_in(&mut app, true, true);
    run_ticks(&mut app, 16);
    request_vehicle(&mut app);
    run_ticks(&mut app, 1);
    assert_eq!(driving(&mut app), Some(car), "left a boxed-in car");
}

/// Fire held (SMG, automatic) and F in the same tick at the door: the seat must not fire.
#[test]
fn held_fire_does_not_carry_into_the_seat() {
    let (mut app, car) = car_app();
    let stats = app
        .world()
        .resource::<WeaponsConfig>()
        .stats(Weapon::Smg)
        .clone();
    set_loadout(&mut app, |l| {
        acquire(&mut l.guns[Weapon::Smg.index()], &stats, true);
        l.held = Some(Weapon::Smg);
    });
    let door = door_of(&app, car);
    let feet = Vec3::new(door.x, float_height_of(&app), door.z);
    place_player(&mut app, feet);
    let target = position_of(&app, car);
    set_aim(&mut app, feet, target);
    let me = player(&mut app);
    let world = app.world();
    let mut fired: MessageCursor<ShotFired> =
        world.resource::<Messages<ShotFired>>().get_cursor_current();
    let mut hits: MessageCursor<BulletHitVehicle> = world
        .resource::<Messages<BulletHitVehicle>>()
        .get_cursor_current();
    set_action(&mut app, |a| {
        a.fire_held = true;
        a.vehicle_requested = true;
    });
    let mut shots = 0;
    let mut car_hits = 0;
    for _ in 0..32 {
        run_ticks(&mut app, 1);
        let world = app.world();
        shots += fired
            .read(world.resource::<Messages<ShotFired>>())
            .filter(|s| s.shooter == me)
            .count();
        car_hits += hits
            .read(world.resource::<Messages<BulletHitVehicle>>())
            .count();
    }
    assert_eq!(driving(&mut app), Some(car), "GATE BROKEN: did not get in");
    assert_eq!(shots, 0, "the driver fired {shots} shots");
    assert_eq!(car_hits, 0, "{car_hits} bullets hit the car");
    assert_eq!(car_health(&app, car), damage_cfg(&app).vehicle.max_health);
}

// ---------------------------------------------------------------- G8 flow while driving

fn until_playing_again(app: &mut App) {
    for _ in 0..1000 {
        app.update();
        if game_state(app) == GameState::Playing {
            return;
        }
    }
    panic!("not back to Playing within 1000 updates");
}

#[test]
fn wasted_while_driving_ejects_and_respawns_on_foot() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    write_damage(&mut app, 1000.0);
    for _ in 0..3 {
        app.update();
        if game_state(&app) == GameState::Wasted {
            break;
        }
    }
    assert_eq!(
        game_state(&app),
        GameState::Wasted,
        "GATE BROKEN: not wasted"
    );
    let me = player(&mut app);
    assert!(app.world().get::<Dead>(me).is_some());
    assert_on_foot(&mut app, "in Wasted");
    assert_eq!(driver_of(&app, car), None);
    until_playing_again(&mut app);
    let hospital = app.world().resource::<HospitalSpawn>().point;
    let at = position(&mut app);
    assert!(
        flat(at - hospital).length() < 0.1,
        "respawned at {at}, hospital {hospital}"
    );
    assert_on_foot(&mut app, "after respawn");
    run_ticks(&mut app, 16);
    let from = position(&mut app);
    set_intent(&mut app, |i| i.axis = Vec2::Y);
    run_ticks(&mut app, 64);
    let moved = flat(position(&mut app) - from).length();
    assert!(moved >= 1.0, "the respawned player walked {moved:.2} m");
}

/// A driver is never arrested in a moving car (above `exit_max_speed`) and never off an arrest row:
/// a cop held in `Arrest` at the driver's door neither binds an arrest attempt nor pulls him out.
/// (O2 allows only the 1-star pull-out of a stopped car: `police_pull_out.rs`.)
#[test]
fn no_arrest_in_a_car() {
    // (heat, the car's speed): 1 star in a car kept at 5 m/s; 2 stars in a car at rest.
    for (heat, speed) in [(40, 5.0), (180, 0.0)] {
        let mut app = graph_app(10.0, &[]);
        set_player_armor(&mut app, 1.0e6);
        raise_heat(&mut app, heat);
        let car = spawn_car(&mut app, Vec2::new(-5.0, 0.0), 0.0);
        drive_in(&mut app, car);
        let start = position_of(&app, car);
        // At the driver's door: left face x -6.2, capsule 0.3, 0.2 m of air; 1.25 m from the seat.
        let feet = Vec3::new(-6.7, 0.0, 0.1);
        let unit = spawn_unit(&mut app, UnitKind::Patrol, feet, 0.0);
        let chest = feet + Vec3::Y * float_height_of(&app);
        let arrest = esc(&app).arrest;
        let seconds = arrest.pull_out_seconds + arrest.pull_give_up_seconds + arrest.seconds + 1.0;
        for tick in 0..(seconds * 64.0) as u32 {
            // Named mutation: the cop is held at the door in `Arrest`, the car keeps `speed` in place.
            set_cop_state(&mut app, unit, CopState::Arrest);
            app.world_mut().get_mut::<Position>(unit).unwrap().0 = chest;
            app.world_mut()
                .get_mut::<Transform>(unit)
                .unwrap()
                .translation = chest;
            let forward = forward_of(&app, car);
            app.world_mut().get_mut::<Position>(car).unwrap().0 = start;
            app.world_mut().get_mut::<LinearVelocity>(car).unwrap().0 = forward * speed;
            run_ticks(&mut app, 1);
            assert_eq!(
                game_state(&app),
                GameState::Playing,
                "heat {heat}: busted in a car"
            );
            assert_eq!(
                driving(&mut app),
                Some(car),
                "heat {heat}: pulled out at tick {tick}"
            );
            assert_eq!(
                attempt(&app).cop,
                None,
                "heat {heat}: an arrest attempt on a driver at tick {tick}"
            );
        }
        let gap = flat(position_of(&app, unit) - position(&mut app)).length();
        assert!(
            gap <= arrest.distance,
            "GATE BROKEN: the cop stands {gap:.2} m from the driver, arrest distance {}",
            arrest.distance
        );
    }
}

#[test]
fn busted_while_driving_ejects_and_respawns_at_the_station() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Busted);
    app.update();
    assert_eq!(
        game_state(&app),
        GameState::Busted,
        "GATE BROKEN: not busted"
    );
    assert_on_foot(&mut app, "in Busted");
    assert_eq!(driver_of(&app, car), None);
    until_playing_again(&mut app);
    let station = app.world().resource::<PoliceStationSpawn>().point;
    let at = position(&mut app);
    assert!(
        flat(at - station).length() < 0.1,
        "respawned at {at}, station {station}"
    );
    assert_on_foot(&mut app, "after respawn");
}

#[test]
fn vanished_car_puts_the_driver_on_foot() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    app.world_mut().entity_mut(car).despawn();
    run_ticks(&mut app, 1);
    assert_on_foot(&mut app, "after the car vanished");
}

#[test]
fn car_mass_and_yaw_inertia() {
    let (app, car) = car_app();
    let world = app.world();
    let mass = world.get::<ComputedMass>(car).unwrap().value();
    assert!((mass - 1200.0).abs() <= 1.0, "mass {mass}");
    // Raised box m/12·(w² + l²) minus the two chamfer wedges, ρ = 1200 / 15.699 m³: 2 181.9 kg·m².
    let tensor = world.get::<ComputedAngularInertia>(car).unwrap().tensor();
    let yaw = tensor.mul_vec3(Vec3::Y).y;
    assert!((yaw - 2181.9).abs() <= 0.01 * 2181.9, "yaw inertia {yaw}");
}

/// Forced eject (Busted) from a car with a 1 m wall along the left side (over the left door point), a
/// 4 m wall along the right side and a slab 2.5 m above the roof: no door stands on the ground and
/// the roof ray starts inside the slab, so the player goes onto the car's top, never onto the 1 m
/// wall. Flip: the old forced fallback (the left door's feet ray) lands on the wall top.
#[test]
fn forced_eject_never_on_a_wall_top() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    spawn_wall(
        &mut app,
        Vec3::new(-27.0, 0.5, 0.0),
        Vec3::new(1.4, 1.0, 6.0),
    );
    spawn_wall(
        &mut app,
        Vec3::new(-23.0, 2.0, 0.0),
        Vec3::new(1.4, 4.0, 6.0),
    );
    let cfg = vehicle_cfg(&app);
    let top = position_of(&app, car).y + cfg.half_extents().y;
    spawn_wall(
        &mut app,
        Vec3::new(-25.0, top + 2.5 + 0.5, 0.0),
        Vec3::new(3.0, 1.0, 5.0),
    );
    run_ticks(&mut app, 16);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Busted);
    app.update();
    assert_eq!(
        game_state(&app),
        GameState::Busted,
        "GATE BROKEN: not busted"
    );
    assert_on_foot(&mut app, "forced eject");
    let feet = position(&mut app).y - float_height_of(&app);
    assert!(
        (feet - top).abs() < 0.05,
        "feet at {feet}: not on the car top {top} (a 1 m wall top is at 1.0)"
    );
}

/// A dummy at the right door, a 1 m wall over the left door and a slab over the roof: nothing is
/// clear, the forced eject takes the right door at ground level (the body is in the way, the wall
/// top is no floor).
#[test]
fn forced_eject_takes_a_level_door_despite_a_body() {
    let (mut app, car) = car_app();
    drive_in(&mut app, car);
    spawn_wall(
        &mut app,
        Vec3::new(-27.0, 0.5, 0.0),
        Vec3::new(1.4, 1.0, 6.0),
    );
    let top = position_of(&app, car).y + vehicle_cfg(&app).half_extents().y;
    spawn_wall(
        &mut app,
        Vec3::new(-25.0, top + 2.5 + 0.5, 0.0),
        Vec3::new(3.0, 1.0, 5.0),
    );
    let right = position_of(&app, car) + rotation_of(&app, car) * Vec3::new(1.7, 0.0, -0.3);
    spawn_dummy(&mut app, Vec3::new(right.x, 0.0, right.z));
    run_ticks(&mut app, 16);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Busted);
    app.update();
    assert_eq!(
        game_state(&app),
        GameState::Busted,
        "GATE BROKEN: not busted"
    );
    let at = position(&mut app);
    let feet = at.y - float_height_of(&app);
    assert!(feet.abs() < 0.05, "feet at {feet}, not on the ground");
    assert!(
        flat(at - right).length() < 0.3,
        "ejected at {at}, not at the right door {right}"
    );
}
