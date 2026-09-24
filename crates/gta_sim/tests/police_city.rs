//! Police in the generated seed-1 city: cops dispatched at 2 stars reach the player along the
//! sidewalk graph (navmesh evidence, liveness only), and the police station respawn exists.

mod common;
mod police_support;
mod wanted_support;

use bevy::prelude::*;
use common::*;
use gta_sim::{
    character::HealthConfig,
    police::CopState,
    world::{BuildingKind, City, CityLandmarks, HospitalSpawn, PoliceStationSpawn},
};
use police_support::*;
use std::collections::BTreeMap;
use wanted_support::*;

const SPOT_TICKS: u32 = 2560;

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a - b).with_y(0.0).length()
}

/// 40 s at 2 stars with the player standing at `feet`; returns per unit the first tick it saw the
/// player or came within its keep band, else its closest distance.
fn chase(name: &str, feet: impl Fn(&App) -> Vec3) -> BTreeMap<Entity, Result<u32, f32>> {
    let mut app = city_app(1);
    settle(&mut app);
    set_population(&mut app, |p| {
        p.max_civilians = 0;
        p.max_gang_members = 0;
    });
    let feet = feet(&app);
    let at = chest(&app, feet);
    place_player(&mut app, at);
    run_ticks(&mut app, 16);
    let settled = position(&mut app);
    assert!(
        settled.distance(at) < 0.1,
        "GATE BROKEN: {name}: player put at {at} stands at {settled} (inside a building?)"
    );
    set_player_armor(&mut app, 1.0e6);
    set_view(&mut app, Some(chase_view(feet, Vec3::NEG_Z)));
    set_heat(&mut app, 180);
    let keep = esc(&app).patrol.keep_distance.1;
    let mut reach: BTreeMap<Entity, Result<u32, f32>> = BTreeMap::new();
    for tick in 0..SPOT_TICKS {
        run_ticks(&mut app, 1);
        let player = position(&mut app);
        for (e, unit) in units(&mut app) {
            if unit.state == CopState::Dead {
                continue;
            }
            let d = flat(position_of(&app, e), player);
            let entry = reach.entry(e).or_insert(Err(f32::INFINITY));
            if let Err(closest) = *entry {
                *entry = if unit.sees || d <= keep {
                    Ok(tick)
                } else {
                    Err(closest.min(d))
                };
            }
        }
    }
    for (e, r) in &reach {
        match r {
            Ok(tick) => println!("{name}: {e} reached in {:.2} s", *tick as f32 / 64.0),
            Err(d) => println!("{name}: {e} stuck at {d:.1} m"),
        }
    }
    reach
}

/// Feet of a sidewalk spot of the generated city.
type Spot = fn(&App) -> Vec3;

#[test]
fn cops_reach_the_player_in_the_city() {
    let spots: [(&str, Spot); 3] = [
        ("hospital", |app| {
            app.world().resource::<HospitalSpawn>().point
        }),
        ("plaza", |app| {
            // The plaza block's centre is the tower's: stand on the sidewalk in front of it.
            let layout = &app.world().resource::<City>().0;
            let margin = app.world().resource::<HealthConfig>().pickups.spacing;
            let tower = layout.landmarks.tower as usize;
            let (anchor, _) = citygen::sidewalk_anchor(layout, city_params(app), tower, margin)
                .expect("GATE BROKEN: tower without a sidewalk anchor");
            Vec3::new(anchor.x, city_params(app).roads.curb_height, anchor.y)
        }),
        ("park", |app| {
            app.world().resource::<CityLandmarks>().park_center
        }),
    ];
    for (name, feet) in spots {
        let reach = chase(name, feet);
        assert!(!reach.is_empty(), "{name}: no unit dispatched in 40 s");
        assert!(
            reach.values().any(Result::is_ok),
            "{name}: no cop reached the player in 40 s: {reach:?}"
        );
    }
}

/// The station respawn is the sidewalk anchor of the first police station with the hospital margin
/// (the anchor itself is swept over all seeds in `citygen/tests/properties.rs`).
#[test]
fn station_spawn_exists() {
    let app = city_app(1);
    let layout = &app.world().resource::<City>().0;
    let station = layout
        .buildings
        .iter()
        .position(|b| b.kind == BuildingKind::PoliceStation)
        .expect("GATE BROKEN: no police station in seed 1");
    let margin = app.world().resource::<HealthConfig>().pickups.spacing;
    let (anchor, _) = citygen::sidewalk_anchor(layout, city_params(&app), station, margin)
        .expect("GATE BROKEN: station without a sidewalk anchor");
    let spawn = app.world().resource::<PoliceStationSpawn>().point;
    let expected = Vec3::new(anchor.x, city_params(&app).roads.curb_height, anchor.y);
    assert!(
        spawn.distance(expected) < 1e-3,
        "station spawn {spawn}, anchor {expected}"
    );
    let hospital = app.world().resource::<HospitalSpawn>().point;
    assert!(
        flat(spawn, hospital) > 10.0,
        "GATE BROKEN: station and hospital spawns coincide"
    );
}
