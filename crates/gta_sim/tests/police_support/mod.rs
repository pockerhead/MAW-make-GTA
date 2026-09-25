//! Shared fixtures of the police gates (`police_*.rs`).
#![allow(dead_code)]

use crate::common::*;
use crate::wanted_support::*;
use bevy::prelude::*;
use gta_sim::{
    character::{CharacterControlConfig, Gait, HealthConfig, LocomotionConfig},
    combat::{Loadout, WeaponsConfig},
    flow::{BustedPhase, RespawnConfig},
    navigation::SidewalkGraph,
    perception::PerceptionConfig,
    police::{ArrestAttempt, CopState, EscalationConfig, PoliceUnit, UnitKind, police_unit_bundle},
    population::{Appearance, PopulationConfig},
    wanted::stars_for,
};

/// Writes `heat` and runs one tick so `track_search` recomputes the stars: a unit whose first FSM
/// run sees 0 stars leaves for good.
pub fn raise_heat(app: &mut App, heat: u32) {
    set_heat(app, heat);
    run_ticks(app, 1);
    let expected = stars_for(heat, &wanted_cfg(app).stars);
    assert_eq!(
        wanted(app).stars,
        expected,
        "GATE BROKEN: heat {heat} gave {:?}",
        wanted(app)
    );
}

pub fn esc(app: &App) -> EscalationConfig {
    app.world().resource::<EscalationConfig>().clone()
}

/// A unit of `kind` (shipped spec) standing at `feet`, facing `Quat::from_rotation_y(yaw) · −Z`
/// (`PI` = +Z, `-FRAC_PI_2` = +X); runs one tick and checks it stands where it was put.
pub fn spawn_unit(app: &mut App, kind: UnitKind, feet: Vec3, yaw: f32) -> Entity {
    if app.world().contains_resource::<SidewalkGraph>() {
        assert!(
            wanted(app).stars >= 1,
            "GATE BROKEN: raise_heat before spawn_unit"
        );
    }
    let world = app.world();
    let loco = world.resource::<LocomotionConfig>().clone();
    let health = world.resource::<HealthConfig>().clone();
    let weapons = world.resource::<WeaponsConfig>().clone();
    let handle = world.resource::<CharacterControlConfig>().0.clone();
    let esc = esc(app);
    let bundle = police_unit_bundle(
        &loco,
        handle,
        &health,
        &weapons,
        esc.spec(kind),
        kind,
        feet,
        yaw,
        Appearance(0),
    );
    let entity = app.world_mut().spawn(bundle).id();
    run_ticks(app, 1);
    let at = position_of(app, entity);
    assert!(
        (at - feet).with_y(0.0).length() < 0.05,
        "GATE BROKEN: unit spawned at {feet} stands at {at}"
    );
    entity
}

pub fn cop(app: &App, entity: Entity) -> PoliceUnit {
    app.world()
        .get::<PoliceUnit>(entity)
        .expect("GATE BROKEN: police unit missing")
        .clone()
}

/// Named test mutation of a unit's state.
pub fn set_cop_state(app: &mut App, entity: Entity, state: CopState) {
    app.world_mut()
        .get_mut::<PoliceUnit>(entity)
        .expect("GATE BROKEN: police unit missing")
        .state = state;
}

pub fn units(app: &mut App) -> Vec<(Entity, PoliceUnit)> {
    app.world_mut()
        .query::<(Entity, &PoliceUnit)>()
        .iter(app.world())
        .map(|(e, u)| (e, u.clone()))
        .collect()
}

/// Active units (not dead, not leaving) and of them SWAT.
pub fn active(app: &mut App) -> (u32, u32) {
    let units = units(app);
    let live = units
        .iter()
        .filter(|(_, u)| !matches!(u.state, CopState::Dead | CopState::Leave));
    let (mut all, mut swat) = (0, 0);
    for (_, u) in live {
        all += 1;
        swat += u32::from(u.kind == UnitKind::Swat);
    }
    (all, swat)
}

pub fn attempt(app: &App) -> ArrestAttempt {
    *app.world().resource::<ArrestAttempt>()
}

pub fn busted_phase(app: &App) -> Option<BustedPhase> {
    app.world()
        .get_resource::<State<BustedPhase>>()
        .map(|s| s.get().clone())
}

/// The confiscation of an arrest: no gun owned, nothing held, no bat.
pub fn assert_confiscated(loadout: &Loadout) {
    assert!(
        loadout.guns.iter().all(|g| !g.owned),
        "a gun is still owned: {:?}",
        loadout.guns
    );
    assert_eq!(loadout.held, None, "a gun is still held");
    assert!(!loadout.has_bat, "the bat was kept");
}

/// The shipped numbers every worked example of the police gates assumes.
pub fn assert_shipped_police(app: &App) {
    let e = esc(app);
    let rows: Vec<(u32, u32, f32, bool, bool)> = e
        .stars
        .iter()
        .map(|r| (r.units, r.swat, r.reinforce_seconds, r.arrest, r.surround))
        .collect();
    assert_eq!(
        rows,
        [
            (2, 0, 20.0, true, false),
            (4, 0, 15.0, false, false),
            (6, 0, 10.0, false, true),
            (8, 4, 8.0, false, true),
            (12, 12, 3.0, false, true),
        ],
        "GATE BROKEN: escalation rows"
    );
    let a = &e.arrest;
    assert_eq!(
        (
            a.distance,
            a.stand_distance,
            a.seconds,
            a.break_free_distance,
            a.hostile_seconds
        ),
        (1.5, 1.0, 1.5, 3.0, 5.0),
        "GATE BROKEN: arrest block"
    );
    assert_eq!(
        (
            e.combat.chase_gait,
            e.combat.search_gait,
            e.spawn_ring,
            e.spawns_per_tick,
            e.search_arrive_distance
        ),
        (Gait::Run, Gait::Walk, (40.0, 90.0), 1, 3.0),
        "GATE BROKEN: police movement"
    );
    let loco = app.world().resource::<LocomotionConfig>();
    assert_eq!(
        (loco.run_speed, loco.sprint_speed),
        (4.5, 6.8),
        "GATE BROKEN: locomotion"
    );
    assert_eq!(
        app.world().resource::<PerceptionConfig>().slots,
        4,
        "GATE BROKEN: perception slots"
    );
    let p = app.world().resource::<PopulationConfig>();
    assert_eq!(
        (p.despawn_offscreen_seconds, p.spawn_point_spacing),
        (2.0, 8.0),
        "GATE BROKEN: population"
    );
    let w = wanted_cfg(app);
    assert_eq!(
        w.stars.each_ref().map(|r| r.heat),
        [40, 180, 550, 1200, 2400],
        "GATE BROKEN: star heats"
    );
    assert_eq!(
        (w.stars[0].search_radius, w.stars[1].search_radius),
        (40.0, 70.0),
        "GATE BROKEN: search radii"
    );
    let r = app.world().resource::<RespawnConfig>();
    assert_eq!(
        (r.busted_arrest, r.busted_screen),
        (2.0, 3.0),
        "GATE BROKEN: busted timing"
    );
}

/// Named mutation: no police cars in any row (the row's units all come on foot; with `cars = 0` the
/// foot dispatcher reserves no seats).
pub fn no_police_cars(app: &mut App) {
    for row in app
        .world_mut()
        .resource_mut::<EscalationConfig>()
        .stars
        .iter_mut()
    {
        row.cars = 0;
    }
}
