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

/// Two stars: the second row (70 m, 15 s) governs the search, not the first (QA TASK-011).
#[test]
fn second_star_row_governs_search() {
    let mut app = floor();
    let cfg = wanted_cfg(&app);
    assert_eq!(
        (
            cfg.stars[1].heat,
            cfg.stars[1].search_radius,
            cfg.stars[1].clear_seconds
        ),
        (180, 70.0, 15.0),
        "GATE BROKEN: second star row"
    );
    let corner = chest(&app, Vec3::new(-35.0, 0.0, -35.0));
    place_player(&mut app, corner);
    run_ticks(&mut app, 2);
    set_heat(&mut app, 180);
    run_ticks(&mut app, 1);
    let w = wanted(&app);
    assert_eq!(w.stars, 2, "{w:?}");
    let centre = w.last_known.unwrap();
    // Outside the 1-star circle (40 m), inside the 2-star one (70 m): never clears.
    let mid = chest(&app, Vec3::new(15.0, 0.0, -35.0));
    place_player(&mut app, mid);
    run_ticks(&mut app, 1);
    let d = flat(position(&mut app), centre);
    assert!(d > 40.0 && d < 70.0, "GATE BROKEN: {d}");
    run_ticks(&mut app, 2000);
    let w = wanted(&app);
    assert_eq!((w.stars, w.hidden), (2, 0.0), "{w:?}");
    // Outside 70 m: clears after exactly 15 s = 960 ticks, not 640.
    let far = chest(&app, Vec3::new(35.0, 0.0, 35.0));
    place_player(&mut app, far);
    run_ticks(&mut app, 959);
    let w = wanted(&app);
    assert!(
        flat(position(&mut app), centre) > 70.0,
        "GATE BROKEN: drifted"
    );
    assert_eq!(w.stars, 2, "cleared before 960 ticks: {w:?}");
    run_ticks(&mut app, 1);
    assert_eq!(wanted(&app), WantedLevel::default());
}

/// Named mutation: heat of star row `row` and the search circle centred `distance` m west of the player.
fn search_from(app: &mut App, row: usize, distance: f32) -> Vec3 {
    let heat = wanted_cfg(app).stars[row].heat;
    let centre = position(app) - Vec3::X * distance;
    let mut w = app.world_mut().resource_mut::<WantedLevel>();
    w.heat = heat;
    w.last_known = Some(centre);
    centre
}

/// Stars 3..5 (the floor is too small for their circles, so the centre moves, not the player): each
/// row's own radius and clear time govern the search.
#[test]
fn higher_star_rows_govern_search() {
    let shipped = [(550, 100.0, 20.0), (1200, 140.0, 25.0), (2400, 180.0, 30.0)];
    for (row, expected) in (2..5).zip(shipped) {
        let mut app = floor();
        let rows = wanted_cfg(&app).stars;
        let r = &rows[row];
        assert_eq!(
            (r.heat, r.search_radius, r.clear_seconds),
            expected,
            "GATE BROKEN: star row {row}"
        );
        let stars = row as u8 + 1;
        let clear = ticks_in(&app, r.clear_seconds);
        // Outside the previous row's circle, inside this row's: never clears.
        let inside = (rows[row - 1].search_radius + r.search_radius) / 2.0;
        let centre = search_from(&mut app, row, inside);
        run_ticks(&mut app, clear + 64);
        let w = wanted(&app);
        let d = flat(position(&mut app), centre);
        assert!(
            d > rows[row - 1].search_radius && d < r.search_radius,
            "GATE BROKEN: {d}"
        );
        assert_eq!((w.stars, w.hidden), (stars, 0.0), "row {row}: {w:?}");
        // Outside this row's circle: clears after exactly its clear time.
        let centre = search_from(&mut app, row, r.search_radius + 10.0);
        run_ticks(&mut app, clear - 1);
        let w = wanted(&app);
        assert!(
            flat(position(&mut app), centre) > r.search_radius,
            "GATE BROKEN: drifted"
        );
        assert_eq!(
            w.stars, stars,
            "row {row} cleared before {clear} ticks: {w:?}"
        );
        run_ticks(&mut app, 1);
        assert_eq!(
            wanted(&app),
            WantedLevel::default(),
            "row {row} not cleared at tick {clear}"
        );
    }
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
