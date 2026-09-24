//! Crimes on cops (GDD §6.4, T11): reported always, the cop is the witness. The test floor has no
//! sidewalk graph, so `NpcSystems` (cop FSM, cop death) are off and the cop stands still;
//! `record_crimes` is `PlayingSystems` and runs. A named config mutation shrinks the witness distance
//! below every eye distance here, so only the "always" rule can report.

mod common;
mod police_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    combat::{MeleeHit, Weapon},
    police::UnitKind,
    wanted::{Crime, WantedConfig},
};
use police_support::*;
use std::f32::consts::PI;
use wanted_support::*;

/// Test floor, player settled at the origin, a patrol cop at `feet` facing +Z.
fn crime_floor(feet: Vec3, blind: bool) -> (App, Entity) {
    let mut app = headless_app();
    settle(&mut app);
    assert_shipped(&app);
    let heat = wanted_cfg(&app).heat;
    assert_eq!(
        (heat.punch_cop, heat.wound_cop, heat.kill_cop),
        (45, 80, 150),
        "GATE BROKEN: cop heat"
    );
    if blind {
        app.world_mut()
            .resource_mut::<WantedConfig>()
            .cop_witness_distance = 0.5;
    }
    let cop = spawn_unit(&mut app, UnitKind::Patrol, feet, PI);
    (app, cop)
}

fn reported(app: &App) -> Vec<(Crime, bool)> {
    incidents(app)
        .iter()
        .map(|i| (i.crime, i.reported))
        .collect()
}

#[test]
fn punching_a_cop_is_reported_always() {
    let (mut app, cop) = crime_floor(Vec3::new(0.0, 0.0, -1.2), true);
    let mut probe = Probe::new(&app);
    let origin = position(&mut app);
    let target = position_of(&app, cop);
    set_aim(&mut app, origin, target);
    let mut hits = app
        .world()
        .resource::<Messages<MeleeHit>>()
        .get_cursor_current();
    set_action(&mut app, |a| a.fire_requested = true);
    let mut landed = false;
    for _ in 0..24 {
        probe.run(&mut app, 1);
        let messages = app.world().resource::<Messages<MeleeHit>>();
        if hits.read(messages).any(|h| h.target == cop) {
            landed = true;
            break;
        }
    }
    assert!(landed, "GATE BROKEN: the punch never landed");
    assert_eq!(wanted(&app).heat, 45, "{:?}", reported(&app));
    assert_eq!(reported(&app), vec![(Crime::PunchCop, true)]);
}

#[test]
fn wounding_a_cop_is_reported_always() {
    let (mut app, cop) = crime_floor(Vec3::new(0.0, 0.0, -10.0), true);
    arm(&mut app, Weapon::Pistol);
    let mut probe = Probe::new(&app);
    let target = position_of(&app, cop);
    let attack = fire_at(&mut app, &mut probe, target);
    assert!(
        probe
            .shots
            .dealt_log
            .iter()
            .any(|h| h.shot == attack && h.target == cop && !h.killed),
        "GATE BROKEN: no wounding hit"
    );
    let mut crimes = reported(&app);
    crimes.sort_by_key(|&(c, _)| c as u8);
    assert_eq!(wanted(&app).heat, 80, "{crimes:?}");
    assert_eq!(
        crimes,
        vec![(Crime::Shooting, false), (Crime::WoundCop, true)]
    );
}

#[test]
fn killing_a_cop_is_reported_always() {
    let (mut app, cop) = crime_floor(Vec3::new(0.0, 0.0, -10.0), true);
    arm(&mut app, Weapon::Pistol);
    let mut probe = Probe::new(&app);
    kill_with_one_shot(&mut app, &mut probe, cop);
    let mut crimes = reported(&app);
    crimes.sort_by_key(|&(c, _)| c as u8);
    assert_eq!(wanted(&app).heat, 150, "{crimes:?}");
    assert_eq!(
        crimes,
        vec![(Crime::Shooting, false), (Crime::KillCop, true)]
    );
}

#[test]
fn cop_witnesses_shooting_near_itself() {
    let (mut app, _) = crime_floor(Vec3::new(0.0, 0.0, -8.0), false);
    arm(&mut app, Weapon::Pistol);
    let mut probe = Probe::new(&app);
    shoot_into_the_air(&mut app, &mut probe);
    assert_eq!(wanted(&app).heat, 10, "{:?}", reported(&app));
    assert_eq!(reported(&app), vec![(Crime::Shooting, true)]);
}
