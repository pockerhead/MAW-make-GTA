//! Pulling a driver out (O2, GDD §6.4 T15): at 1 star a cop in `Arrest` at the door of the player's
//! stopped car pulls the player out through the left door after `pull_out_seconds`, and the arrest
//! then runs on foot to BUSTED. Never while the car moves, off an arrest row or from 2 m. The
//! arresting cops never block the door; a door blocked by anything else holds the pull for
//! `pull_give_up_seconds`, then the driver comes out at another exit and the arrest starts over on
//! foot (a cop at the far door must not count as a break-free: +1 star).

mod common;
mod police_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::prelude::*;
use common::*;
use gta_sim::{
    flow::GameState,
    police::{CopState, UnitKind},
};
use police_support::*;
use vehicle_support::*;
use wanted_support::*;

/// The player sits in a car at (−25, 0) facing −X (left door towards +Z) on `graph_app(10)` at
/// `heat`; a patrol cop stands `gap` m out from the left door point facing it. Returns (app, car, cop).
fn setup(heat: u32, gap: f32) -> (App, Entity, Entity) {
    let mut app = graph_app(10.0, &[]);
    assert_shipped_police(&app);
    set_player_armor(&mut app, 1.0e6);
    let car = spawn_car(&mut app, Vec2::new(-25.0, 0.0), 90.0);
    drive_in(&mut app, car);
    raise_heat(&mut app, heat);
    let door = door_of(&app, car);
    let cop = spawn_unit(
        &mut app,
        UnitKind::Patrol,
        Vec3::new(door.x, 0.0, door.z + gap),
        0.0,
    );
    (app, car, cop)
}

fn pulled_out(app: &mut App) -> bool {
    driving(app).is_none()
}

#[test]
fn a_stopped_driver_is_pulled_out_and_busted() {
    let (mut app, car, unit) = setup(40, 1.2);
    let esc = esc(&app);
    // The cop goes for the arrest; the pull starts on the first tick it holds the door.
    let mut ticks = 0;
    while attempt(&app).pull == 0.0 {
        ticks += 1;
        assert!(
            ticks <= 8,
            "GATE BROKEN: no pull started: {:?}",
            cop(&app, unit)
        );
        run_ticks(&mut app, 1);
    }
    assert_eq!(cop(&app, unit).state, CopState::Arrest);
    let pull_ticks = (esc.arrest.pull_out_seconds * 64.0) as u32;
    run_ticks(&mut app, pull_ticks - 2);
    assert!(!pulled_out(&mut app), "pulled out a tick early");
    run_ticks(&mut app, 1);
    assert!(
        pulled_out(&mut app),
        "not pulled out after {} s",
        esc.arrest.pull_out_seconds
    );
    run_ticks(&mut app, 1);
    let door = door_of(&app, car);
    let at = position(&mut app);
    let feet = at.y - float_height_of(&app);
    assert!(feet.abs() < 0.05, "feet at {feet} after the pull");
    assert!(
        (at - door).with_y(0.0).length() < 0.4,
        "the player stands at {at}, the left door is {door}"
    );
    let arrest_ticks = (esc.arrest.seconds * 64.0) as u32;
    for _ in 0..arrest_ticks + 2 {
        if game_state(&app) == GameState::Busted {
            break;
        }
        run_ticks(&mut app, 1);
    }
    assert_eq!(
        game_state(&app),
        GameState::Busted,
        "no BUSTED after the pull"
    );
}

fn never_pulled(mut app: App, each_tick: impl Fn(&mut App)) {
    let heat = wanted(&app).heat;
    for tick in 0..640 {
        each_tick(&mut app);
        run_ticks(&mut app, 1);
        assert!(!pulled_out(&mut app), "pulled out at tick {tick}");
    }
    assert_eq!(wanted(&app).heat, heat, "the heat changed (broke free?)");
}

#[test]
fn a_moving_car_is_not_pulled() {
    let (app, car, _) = setup(40, 1.2);
    let start = position_of(&app, car);
    // Named mutation: the car keeps 5 m/s in place (its speed is what the rule reads).
    never_pulled(app, move |app| {
        let forward = forward_of(app, car);
        app.world_mut().get_mut::<Position>(car).unwrap().0 = start;
        app.world_mut().get_mut::<LinearVelocity>(car).unwrap().0 = forward * 5.0;
    });
}

#[test]
fn no_pull_at_two_stars() {
    let (app, _, _) = setup(180, 1.2);
    never_pulled(app, |_| {});
}

#[test]
fn no_pull_from_two_metres() {
    let (app, car, cop) = setup(40, 2.0);
    let door = door_of(&app, car);
    let float = float_height_of(&app);
    // Named mutation: the cop is held 2.0 m from the door (it would walk up to 1.0 m).
    never_pulled(app, move |app| {
        let at = Vec3::new(door.x, float, door.z + 2.0);
        app.world_mut().get_mut::<Position>(cop).unwrap().0 = at;
        app.world_mut()
            .get_mut::<Transform>(cop)
            .unwrap()
            .translation = at;
    });
}

/// A blocked left door holds the pull for `pull_give_up_seconds`; then the driver is pulled out at
/// another exit and the arrest starts over on foot: never a break-free from the cop at the far door.
#[test]
fn a_blocked_left_door_gives_up_to_another_exit() {
    let (mut app, car, _) = setup(40, 1.2);
    let esc = esc(&app);
    let door = door_of(&app, car);
    // A 1 m wall 0.1 m out from the door point, along the car: the cop sees over it, no capsule fits.
    spawn_wall(
        &mut app,
        Vec3::new(door.x, 0.5, door.z + 0.1 + 0.1),
        Vec3::new(6.0, 1.0, 0.2),
    );
    let heat = wanted(&app).heat;
    let hold = ((esc.arrest.pull_out_seconds + esc.arrest.pull_give_up_seconds) * 64.0) as u32;
    let mut out = None;
    for tick in 0..hold + 64 * 20 {
        run_ticks(&mut app, 1);
        assert_eq!(
            wanted(&app).heat,
            heat,
            "the heat changed at tick {tick} (broke free?)"
        );
        if out.is_none() && pulled_out(&mut app) {
            out = Some(tick);
            let at = position(&mut app);
            assert!(
                (at - door).with_y(0.0).length() > 1.0,
                "pulled out through the blocked left door: {at}"
            );
        }
    }
    let out = out.expect("never pulled out through another exit");
    eprintln!("pulled out at tick {out} (give-up at {hold})");
    assert!(
        out + 2 >= hold,
        "pulled out at tick {out}, before the give-up at {hold}"
    );
    assert!(
        out <= hold + 2,
        "pulled out at tick {out}, long after the give-up at {hold}"
    );
    // What follows is the ordinary foot arrest (here the cop loses sight of the player behind the car
    // and searches); the 20 s window above proves only that nothing breaks free.
    assert_eq!(
        attempt(&app).cop,
        None,
        "the attempt still binds the cop at the far door"
    );
}

/// Both doors blocked (1 m walls, the cop sees over them): the give-up never pulls the driver out onto
/// the roof, the one exit left (a crew never gets out there either); he stays in the car and nothing
/// breaks free.
#[test]
fn a_give_up_never_pulls_onto_the_roof() {
    let (mut app, car, _) = setup(40, 1.2);
    let esc = esc(&app);
    let door = door_of(&app, car);
    let centre = position_of(&app, car);
    let right = Vec3::new(door.x, 0.0, 2.0 * centre.z - door.z);
    for (at, out) in [(door, 1.0), (right, -1.0)] {
        spawn_wall(
            &mut app,
            Vec3::new(at.x, 0.5, at.z + out * 0.2),
            Vec3::new(6.0, 1.0, 0.2),
        );
    }
    let heat = wanted(&app).heat;
    let give_up = ((esc.arrest.pull_out_seconds + esc.arrest.pull_give_up_seconds) * 64.0) as u32;
    let mut longest = 0.0f32;
    for tick in 0..give_up + 64 * 10 {
        run_ticks(&mut app, 1);
        longest = longest.max(attempt(&app).pull);
        assert!(
            !pulled_out(&mut app),
            "pulled out at tick {tick}, feet at {:.2} (give-up at {give_up})",
            position(&mut app).y - float_height_of(&app)
        );
        assert_eq!(wanted(&app).heat, heat, "the heat changed at tick {tick}");
    }
    let limit = esc.arrest.pull_out_seconds + esc.arrest.pull_give_up_seconds - 1.5 / 64.0;
    assert!(
        longest >= limit,
        "GATE BROKEN: the pull never reached the give-up ({longest:.2} s of {limit:.2} s)"
    );
}

/// The city case (QA bug 2): the whole crew is in `Arrest` and the second cop stands on the
/// left-door exit spot. The arresting cops never block the door they pull at.
#[test]
fn a_second_arresting_cop_at_the_door_does_not_block_the_pull() {
    let (mut app, car, first) = setup(40, 1.2);
    let esc = esc(&app);
    let door = door_of(&app, car);
    let float = float_height_of(&app);
    // The second cop stands 0.44 m out from the door point (QA measurement), held there.
    let second = spawn_unit(
        &mut app,
        UnitKind::Patrol,
        Vec3::new(door.x + 0.3, 0.0, door.z + 0.44),
        0.0,
    );
    let hold_second = move |app: &mut App| {
        let at = Vec3::new(door.x + 0.3, float, door.z + 0.44);
        app.world_mut().get_mut::<Position>(second).unwrap().0 = at;
        app.world_mut()
            .get_mut::<Transform>(second)
            .unwrap()
            .translation = at;
    };
    let mut ticks = 0;
    while attempt(&app).pull == 0.0 {
        ticks += 1;
        assert!(
            ticks <= 8,
            "GATE BROKEN: no pull started: {:?}",
            cop(&app, first)
        );
        hold_second(&mut app);
        run_ticks(&mut app, 1);
    }
    assert_eq!(
        cop(&app, second).state,
        CopState::Arrest,
        "GATE BROKEN: the second cop is not arresting"
    );
    let pull_ticks = (esc.arrest.pull_out_seconds * 64.0) as u32;
    for _ in 0..pull_ticks + 1 {
        hold_second(&mut app);
        run_ticks(&mut app, 1);
    }
    assert!(
        pulled_out(&mut app),
        "not pulled out after {} s with a second cop at the door",
        esc.arrest.pull_out_seconds
    );
    run_ticks(&mut app, 1);
    let at = position(&mut app);
    assert!(
        (at - door).with_y(0.0).length() < 1.0,
        "the player stands at {at}, not at the left door {door}"
    );
    let arrest_ticks = (esc.arrest.seconds * 64.0) as u32;
    for _ in 0..arrest_ticks + 64 {
        if game_state(&app) == GameState::Busted {
            break;
        }
        run_ticks(&mut app, 1);
    }
    assert_eq!(
        game_state(&app),
        GameState::Busted,
        "no BUSTED after the pull"
    );
}
