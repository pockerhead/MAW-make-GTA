//! Search circle, cop sight and the wanted lifecycle (GDD §6.4, T10) on the test floor.

mod common;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    civilian::CivilianState, combat::aim_yaw, flow::GameState, perception::Cause,
    wanted::WantedLevel,
};
use std::f32::consts::PI;
use wanted_support::*;

/// Test floor, player settled at the origin, shipped numbers checked.
fn floor() -> App {
    let mut app = headless_app();
    settle(&mut app);
    assert_shipped(&app);
    app
}

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a - b).xz().length()
}

/// Named mutation `heat = 40` with the player at the origin; one tick fixes the circle there.
fn one_star_at_origin() -> (App, Vec3) {
    let mut app = floor();
    set_heat(&mut app, 40);
    run_ticks(&mut app, 1);
    let w = wanted(&app);
    let centre = w.last_known.expect("liveness: no search circle");
    assert!(flat(centre, Vec3::ZERO) < 0.1, "{w:?}");
    assert_eq!((w.stars, w.hidden), (1, 0.0));
    (app, centre)
}

/// Runs `ticks` ticks unseen at `feet`, asserting each tick the player is still outside the
/// circle and `hidden` grew by exactly one step from `start`.
fn hide_outside(app: &mut App, centre: Vec3, start: u32, ticks: u32) {
    let radius = wanted_cfg(app).stars[0].search_radius;
    for k in start + 1..=start + ticks {
        run_ticks(app, 1);
        let at = position(app);
        assert!(
            flat(at, centre) > radius,
            "GATE BROKEN: drifted into the circle: {at}"
        );
        let w = wanted(app);
        if w.heat == 0 {
            return;
        }
        assert_eq!(w.hidden, k as f32 / 64.0, "tick {k}: {w:?}");
    }
}

#[test]
fn stars_follow_a_heat_mutation() {
    let mut app = floor();
    let three = wanted_cfg(&app).stars[2].heat;
    set_heat(&mut app, three);
    run_ticks(&mut app, 1);
    let w = wanted(&app);
    assert_eq!(w.stars, 3, "{w:?}");
    // Physics steps after the wanted systems; the body floats by micrometres.
    let centre = w.last_known.expect("no circle after a heat mutation");
    assert!(centre.distance(position(&mut app)) < 1e-3, "{w:?}");
}

#[test]
fn hiding_outside_the_circle_clears() {
    let (mut app, centre) = one_star_at_origin();
    let away = chest(&app, Vec3::new(30.0, 0.0, 30.0));
    place_player(&mut app, away);
    hide_outside(&mut app, centre, 0, 639);
    assert_eq!(wanted(&app).stars, 1, "cleared before 640 ticks");
    run_ticks(&mut app, 1);
    let w = wanted(&app);
    assert_eq!(w, WantedLevel::default(), "not cleared at tick 640");

    // Control: inside the circle the timer never runs.
    let (mut app, _) = one_star_at_origin();
    let inside = chest(&app, Vec3::new(20.0, 0.0, 0.0));
    place_player(&mut app, inside);
    run_ticks(&mut app, 1280);
    let w = wanted(&app);
    assert_eq!((w.stars, w.hidden), (1, 0.0), "{w:?}");

    // Variant: a tick back inside restarts the timer.
    let (mut app, centre) = one_star_at_origin();
    place_player(&mut app, away);
    hide_outside(&mut app, centre, 0, 320);
    place_player(&mut app, inside);
    run_ticks(&mut app, 1);
    assert_eq!(wanted(&app).hidden, 0.0);
    place_player(&mut app, away);
    hide_outside(&mut app, centre, 0, 320);
    let w = wanted(&app);
    assert_eq!(w.stars, 1, "the timer did not restart: {w:?}");
}

#[test]
fn spotted_resets_timer_and_moves_circle() {
    let (mut app, centre) = one_star_at_origin();
    let away = chest(&app, Vec3::new(30.0, 0.0, 30.0));
    place_player(&mut app, away);
    hide_outside(&mut app, centre, 0, 320);
    assert_eq!(wanted(&app).hidden, 5.0);
    let cop_chest = chest(&app, Vec3::new(30.0, 0.0, 10.0));
    let cop = spawn_cop(&mut app, cop_chest, PI);
    run_ticks(&mut app, 1);
    let w = wanted(&app);
    let player_at = position(&mut app);
    assert!(w.seen && w.hidden == 0.0, "not spotted: {w:?}");
    let new_centre = w.last_known.unwrap();
    assert!(flat(new_centre, player_at) < 0.1, "circle not moved: {w:?}");
    app.world_mut().despawn(cop);
    run_ticks(&mut app, 1280);
    let w = wanted(&app);
    assert_eq!((w.stars, w.hidden, w.seen), (1, 0.0, false), "{w:?}");
    let far = chest(&app, Vec3::new(-30.0, 0.0, -30.0));
    place_player(&mut app, far);
    hide_outside(&mut app, new_centre, 0, 639);
    assert_eq!(wanted(&app).stars, 1);
    run_ticks(&mut app, 1);
    assert_eq!(wanted(&app), WantedLevel::default());

    // A cop facing away does not see the player.
    let (mut app, centre) = one_star_at_origin();
    place_player(&mut app, away);
    spawn_cop(&mut app, cop_chest, 0.0);
    hide_outside(&mut app, centre, 0, 320);
    let w = wanted(&app);
    assert!(!w.seen && w.hidden == 5.0, "{w:?}");
}

/// `seen` of a one-star player at (0,0,8) watched by a cop at `cop_feet` looking along `yaw`.
fn seen_through_the_production_system(cop_feet: Vec3, yaw: f32) -> bool {
    let mut app = floor();
    let player_at = chest(&app, Vec3::new(0.0, 0.0, 8.0));
    place_player(&mut app, player_at);
    let cop = chest(&app, cop_feet);
    spawn_cop(&mut app, cop, yaw);
    set_heat(&mut app, 40);
    run_ticks(&mut app, 1);
    let w = wanted(&app);
    assert_eq!(w.stars, 1, "GATE BROKEN: {w:?}");
    w.seen
}

#[test]
fn walls_block_cop_sight() {
    assert!(
        !seen_through_the_production_system(Vec3::new(0.0, 0.0, 20.0), 0.0),
        "a cop saw through the wall"
    );
    assert!(
        seen_through_the_production_system(
            Vec3::new(-15.0, 0.0, 20.0),
            aim_yaw(Vec3::new(15.0, 0.0, -12.0))
        ),
        "a cop past the wall's end did not see the player"
    );
}

#[test]
fn wasted_forgets_the_crimes() {
    let (mut app, witness, victim) = setup_w(&[]);
    let mut probe = Probe::new(&app);
    kill_with_one_shot(&mut app, &mut probe, victim);
    await_report(&mut app, &mut probe, witness, Cause::Body(victim));
    assert!(
        !incidents(&app).is_empty(),
        "liveness: no incident to forget"
    );
    write_damage(&mut app, 1000.0);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        game_state(&app),
        GameState::Wasted,
        "GATE BROKEN: not wasted"
    );
    let mut updates = 0;
    while game_state(&app) == GameState::Wasted {
        app.update();
        updates += 1;
        assert!(updates < 5000, "GATE BROKEN: Wasted never ended");
    }
    assert_eq!(game_state(&app), GameState::Playing, "GATE BROKEN");
    // Named mutation: a call started before the death completes after the respawn.
    set_civilian_state(
        &mut app,
        witness,
        CivilianState::Report {
            progress: 0.0,
            about: Some(Cause::Body(victim)),
        },
    );
    let mut probe = Probe::new(&app);
    probe.run(&mut app, 256 + 16);
    assert!(
        probe
            .call_log
            .iter()
            .any(|c| c.about == Cause::Body(victim)),
        "liveness: the stale call never completed: {:?}",
        civilian_state(&app, witness)
    );
    let w = wanted(&app);
    assert_eq!(w.heat, 0, "a pre-death crime survived Wasted: {w:?}");
    assert!(incidents(&app).is_empty());
}
