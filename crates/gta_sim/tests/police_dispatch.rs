//! `PoliceDispatcher` on the test floor (production composition): the unit count follows the star
//! row, spawns stay out of the camera view, lost units come back after `reinforce_seconds`, cleared
//! wanted sends everyone away, and units sent away never rejoin.

mod common;
mod police_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::police::{CopState, PoliceDispatcher};
use police_support::*;
use std::collections::HashMap;
use wanted_support::*;

const FEET: Vec3 = Vec3::new(-30.0, 0.0, -30.0);

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a - b).with_y(0.0).length()
}

/// The player at (-30, 0, -30) behind two sidewalk edges, A (30, 0, -35)-(30, 0, 35) and
/// B (-35, 0, 35)-(25, 0, 35): 19 spawn points 60-89 m away, all inside the 40-90 m ring; the camera
/// looks away from them; no civilians.
fn dispatch_floor() -> App {
    let mut app = headless_app();
    test_graph(
        &mut app,
        vec![
            Vec3::new(30.0, 0.0, -35.0),
            Vec3::new(30.0, 0.0, 35.0),
            Vec3::new(-35.0, 0.0, 35.0),
            Vec3::new(25.0, 0.0, 35.0),
        ],
        &[(0, 1), (2, 3)],
    );
    set_population(&mut app, |p| p.max_civilians = 0);
    settle(&mut app);
    let at = chest(&app, FEET);
    place_player(&mut app, at);
    run_ticks(&mut app, 32);
    assert!(
        flat(position(&mut app), FEET) < 0.05,
        "GATE BROKEN: the player is not at {FEET}"
    );
    assert_shipped_police(&app);
    set_player_armor(&mut app, 1.0e6);
    set_view(
        &mut app,
        Some(chase_view(FEET, Vec3::new(-1.0, 0.0, -1.0).normalize())),
    );
    app
}

fn dispatcher(app: &App) -> PoliceDispatcher {
    *app.world().resource::<PoliceDispatcher>()
}

/// Row `k`: never more than the row, and the row full by tick 64.
fn units_follow_row(k: usize) {
    let mut app = dispatch_floor();
    let row = esc(&app).stars[k].clone();
    let heat = wanted_cfg(&app).stars[k].heat;
    set_heat(&mut app, heat);
    let mut full = None;
    for tick in 1..=320 {
        run_ticks(&mut app, 1);
        let (units, swat) = active(&mut app);
        assert!(
            units <= row.units && swat <= row.swat,
            "row {k} tick {tick}: {units} units, {swat} SWAT"
        );
        let d = dispatcher(&app);
        assert_eq!(
            (d.units, d.swat),
            (units, swat),
            "tick {tick}: dispatcher count"
        );
        if (units, swat) == (row.units, row.swat) {
            full.get_or_insert(tick);
        }
    }
    let full = full.unwrap_or_else(|| panic!("row {k} never full: {:?}", active(&mut app)));
    assert!(full <= 64, "row {k} full only at tick {full}");
}

#[test]
fn units_follow_row_1() {
    units_follow_row(0);
}

#[test]
fn units_follow_row_2() {
    units_follow_row(1);
}

#[test]
fn units_follow_row_3() {
    units_follow_row(2);
}

#[test]
fn units_follow_row_4() {
    units_follow_row(3);
}

#[test]
fn units_follow_row_5() {
    units_follow_row(4);
}

#[test]
fn spawns_stay_off_frame() {
    let mut app = dispatch_floor();
    set_view(
        &mut app,
        Some(chase_view(FEET, Vec3::new(60.0, 0.0, 30.0).normalize())),
    );
    set_heat(&mut app, 40);
    let visible = [
        Vec3::new(30.0, 0.0, -35.0),
        Vec3::new(30.0, 0.0, -27.0),
        Vec3::new(30.0, 0.0, -19.0),
    ];
    let mut first: HashMap<Entity, Vec3> = HashMap::new();
    for _ in 0..64 {
        run_ticks(&mut app, 1);
        for (e, _) in units(&mut app) {
            let at = position_of(&app, e);
            first.entry(e).or_insert(at);
        }
    }
    assert_eq!(first.len(), 2, "GATE BROKEN: {} units spawned", first.len());
    for (e, at) in &first {
        println!("{e} spawned at {at}");
        for p in visible {
            assert!(flat(*at, p) > 0.5, "{e} spawned in view at {at}");
        }
    }
}

#[test]
fn lost_unit_is_replaced_after_reinforce_seconds() {
    let mut app = dispatch_floor();
    let heat = wanted_cfg(&app).stars[2].heat;
    set_heat(&mut app, heat);
    let reinforce = esc(&app).stars[2].reinforce_seconds;
    let wait = ticks_in(&app, reinforce);
    for _ in 0..64 {
        run_ticks(&mut app, 1);
    }
    assert_eq!(active(&mut app), (6, 0), "GATE BROKEN: row 3 not full");
    let (victim, _) = units(&mut app)[0].clone();
    set_health_of(&mut app, victim, |h| h.current = 0.0);
    // Tick T: `police_death` marks it dead, the dispatcher sees `Added<Dead>` and waits `wait` ticks.
    for k in 0..wait {
        run_ticks(&mut app, 1);
        assert_eq!(cop(&app, victim).state, CopState::Dead, "GATE BROKEN");
        assert_eq!(active(&mut app).0, 5, "tick T+{k}: replaced early");
    }
    run_ticks(&mut app, 1);
    assert_eq!(active(&mut app).0, 6, "not replaced at tick T+{wait}");
}

#[test]
fn cleared_wanted_sends_units_away() {
    let mut app = dispatch_floor();
    set_heat(&mut app, 40);
    for _ in 0..64 {
        run_ticks(&mut app, 1);
    }
    assert_eq!(active(&mut app), (2, 0), "GATE BROKEN: row 1 not full");
    set_heat(&mut app, 0);
    run_ticks(&mut app, 2);
    for (e, u) in units(&mut app) {
        assert_eq!(u.state, CopState::Leave, "{e} did not leave");
    }
    for _ in 0..130 {
        run_ticks(&mut app, 1);
    }
    assert!(
        units(&mut app).is_empty(),
        "leaving units stayed: {:?}",
        units(&mut app)
    );
}

#[test]
fn reset_units_do_not_rejoin() {
    let mut app = dispatch_floor();
    let heat = wanted_cfg(&app).stars[4].heat;
    set_heat(&mut app, heat);
    for _ in 0..64 {
        run_ticks(&mut app, 1);
    }
    assert_eq!(active(&mut app), (12, 12), "GATE BROKEN: row 5 not full");
    let old: Vec<Entity> = units(&mut app).into_iter().map(|(e, _)| e).collect();
    set_view(
        &mut app,
        Some(chase_view(FEET, Vec3::new(1.0, 0.0, 1.0).normalize())),
    );
    set_heat(&mut app, 0);
    run_ticks(&mut app, 2);
    assert!(
        units(&mut app)
            .iter()
            .all(|(_, u)| u.state == CopState::Leave),
        "GATE BROKEN: not all units leave"
    );
    set_heat(&mut app, 40);
    for tick in 0..320 {
        run_ticks(&mut app, 1);
        let all = units(&mut app);
        for e in &old {
            assert!(
                all.iter().any(|(o, _)| o == e),
                "GATE BROKEN: old unit {e} despawned at tick {tick}"
            );
        }
        let back: Vec<_> = all
            .iter()
            .filter(|(_, u)| u.state != CopState::Leave)
            .collect();
        assert!(back.len() <= 2, "tick {tick}: {} units engaged", back.len());
        assert!(
            back.iter()
                .all(|(_, u)| u.kind == gta_sim::police::UnitKind::Patrol),
            "tick {tick}: SWAT engaged at 1 star"
        );
    }
}
