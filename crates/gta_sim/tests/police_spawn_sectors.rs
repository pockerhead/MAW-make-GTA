//! G7 (TASK-032): police cars spawn ahead of and beside a fleeing driver, not only behind. Production
//! city seed 1, the player's car cruising at 12 m/s along the two-way street with the longest straight
//! run, the camera behind it looking ahead (updated every tick), 2 and 3 stars. Named mutations: the
//! player is reported all the time and the search never clears (the subject is the dispatcher, not the
//! search), the road ahead of him is kept clear (he keeps moving, so the dispatcher sees a pursuit),
//! armour so nobody dies.
//!
//! - Sector rows (each police car and its crew despawned one tick after it appears, to sample the
//!   dispatcher many times): the spawns the test classifies itself by the bearing from the driver's
//!   heading (the data sector bounds) cover ahead, beside and behind, each at least its fixed floor
//!   (`FLOORS`, half the shipped shares) of the pursuit spawns; picks that fell back to another sector
//!   when one had no hidden point are reported.
//! - Every spawn is hidden (every top corner of the chassis outside the view cone, or every ray from the
//!   camera to them blocked by world geometry) and overlaps no vehicle or character.
//! - Cap row (no despawn, 60 s): the police cars with sirens on never exceed the row's `cars`.

mod common;
mod police_support;
mod traffic_support;
mod vehicle_support;
mod wanted_support;

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use common::*;
use gta_sim::{
    combat::aim_yaw,
    layers::GameLayer,
    police::{CrewOf, EscalationConfig, PoliceCar, sirens_on},
    population::ViewCone,
    traffic::{FlatRect, TrafficConfig},
    vehicle::{Vehicle, VehicleConfig},
    wanted::WantedLevel,
};
use std::collections::HashSet;
use traffic_support::*;
use vehicle_support::*;
use wanted_support::*;

const HZ: u32 = 64;
const SPEED: f32 = 12.0;
const SECTORS: [&str; 3] = ["ahead", "beside", "behind"];
/// Fixed floors, share of the pursuit spawns per sector: half the shipped shares (0.3 / 0.3 / 0.4),
/// frozen so a changed share cannot move its own floor. Measured at 2 stars, 30 s: ahead 475, beside
/// 612, behind 833 of 1920 (1405 dispatcher picks fell back to another sector).
const FLOORS: [f32; 3] = [0.15, 0.15, 0.2];

struct Spawn {
    sector: usize,
    hidden: bool,
    overlaps: Vec<Entity>,
}

/// The driver's car cruising the straightest street, `stars` held.
fn chase_scene(stars: u8) -> (App, Entity) {
    let mut app = city_app(1);
    settle(&mut app);
    set_player_armor(&mut app, 1.0e6);
    let cfg = app.world().resource::<TrafficConfig>().clone();
    let g = graph(&app);
    let chain = straightest_road(&g, cfg.desired_speed.street);
    let (seg, s) = place_on(&g, &chain, 10.0);
    let (at, dir) = g.pose(seg, s);
    clear_spot(&mut app, at, 6.0);
    let car = spawn_car(&mut app, Vec2::new(at.x, at.z), aim_yaw(dir).to_degrees());
    drive_in(&mut app, car);
    kick(&mut app, car, SPEED);
    let heat = wanted_cfg(&app).stars[usize::from(stars) - 1].heat;
    police_support::raise_heat(&mut app, heat);
    (app, car)
}

/// One tick of the chase: steer, keep the road clear, camera behind the car, the player reported.
fn chase_tick(app: &mut App, car: Entity) -> ViewCone {
    cruise(app, car, SPEED);
    clear_ahead(app, car);
    let me = position_of(app, car);
    let view = chase_view(me, forward_of(app, car));
    set_view(app, Some(view));
    {
        let mut w = app.world_mut().resource_mut::<WantedLevel>();
        w.last_known = Some(me);
        w.hidden = 0.0;
    }
    run_ticks(app, 1);
    view
}

/// Every top corner of the car at `body` is outside `view`, or world geometry blocks every ray from
/// the camera to them.
fn hidden(app: &mut App, view: ViewCone, position: Vec3, rotation: Quat) -> bool {
    let half = app.world().resource::<VehicleConfig>().half_extents();
    let corners = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
        .map(|(x, z)| position + rotation * Vec3::new(x * half.x, half.y, z * half.z));
    if corners.iter().all(|&p| !view.contains(p, 0.0)) {
        return true;
    }
    app.world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            let filter = SpatialQueryFilter::from_mask(GameLayer::World);
            corners.iter().all(|&p| {
                let d = p - view.origin;
                let Ok(dir) = Dir3::new(d) else {
                    return false;
                };
                spatial
                    .cast_ray(view.origin, dir, d.length() - 0.05, true, &filter)
                    .is_some()
            })
        })
        .expect("GATE BROKEN: ray query failed")
}

/// Vehicles and characters overlapping the chassis of `car` (flat, 1 cm shrink like the oracle).
fn overlaps(app: &mut App, car: Entity) -> Vec<Entity> {
    let me = footprint(app, car);
    let radius = app
        .world()
        .resource::<gta_sim::character::LocomotionConfig>()
        .capsule_radius;
    let vehicles: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<Vehicle>>()
        .iter(app.world())
        .filter(|&e| e != car)
        .collect();
    let mut hits: Vec<Entity> = vehicles
        .into_iter()
        .filter(|&e| obb_overlap(&me, &footprint(app, e)))
        .collect();
    let characters: Vec<(Entity, Vec2)> = app
        .world_mut()
        .query_filtered::<(Entity, &Position), (
            With<gta_sim::character::Character>,
            Without<ColliderDisabled>,
        )>()
        .iter(app.world())
        .map(|(e, p)| (e, Vec2::new(p.0.x, p.0.z)))
        .collect();
    hits.extend(
        characters
            .into_iter()
            .filter(|&(_, c)| circle_in_rect(c, radius, &me))
            .map(|(e, _)| e),
    );
    hits
}

fn circle_in_rect(c: Vec2, r: f32, rect: &FlatRect) -> bool {
    let d = c - rect.centre;
    let (ax, az) = (rect.axis, rect.axis.perp());
    let local = Vec2::new(d.dot(ax), d.dot(az));
    let q = local.abs() - rect.half;
    q.max(Vec2::ZERO).length() < r - 0.01
}

/// Police cars that appeared this tick.
fn new_police(app: &mut App, seen: &mut HashSet<Entity>) -> Vec<Entity> {
    let now: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<PoliceCar>>()
        .iter(app.world())
        .collect();
    now.into_iter().filter(|&e| seen.insert(e)).collect()
}

fn sampled_spawns(stars: u8, seconds: u32) -> Vec<Spawn> {
    let (mut app, car) = chase_scene(stars);
    let sectors = app
        .world()
        .resource::<EscalationConfig>()
        .car
        .spawn_sectors
        .clone();
    let exit = app.world().resource::<VehicleConfig>().exit_max_speed;
    let mut seen = HashSet::new();
    let mut spawns = Vec::new();
    for _ in 0..seconds * HZ {
        let view = chase_tick(&mut app, car);
        let velocity = velocity_of(&app, car).with_y(0.0);
        let me = position_of(&app, car);
        for police in new_police(&mut app, &mut seen) {
            let p = position_of(&app, police);
            let r = rotation_of(&app, police);
            let hidden = hidden(&mut app, view, p, r);
            let overlaps = overlaps(&mut app, police);
            // Only spawns in a pursuit count for the sectors (the driver moving above exit speed).
            if velocity.length() > exit {
                let angle = (p - me).with_y(0.0).angle_between(velocity).to_degrees();
                let sector = if angle <= sectors.ahead_deg {
                    0
                } else if angle > sectors.behind_deg {
                    2
                } else {
                    1
                };
                spawns.push(Spawn {
                    sector,
                    hidden,
                    overlaps,
                });
            } else {
                assert!(
                    hidden && overlaps.is_empty(),
                    "spawn not hidden or overlapping"
                );
            }
            // Named mutation: the car and its crew go, so the dispatcher spawns again.
            let crew: Vec<Entity> = app
                .world_mut()
                .query::<(Entity, &CrewOf)>()
                .iter(app.world())
                .filter(|(_, c)| c.car == police)
                .map(|(e, _)| e)
                .collect();
            for e in crew.into_iter().chain([police]) {
                app.world_mut().despawn(e);
            }
        }
    }
    let fallbacks = app
        .world()
        .resource::<gta_sim::police::PoliceDispatcher>()
        .sector_fallbacks;
    eprintln!(
        "{stars} stars: {} pursuit spawns, dispatcher fallbacks {fallbacks}",
        spawns.len()
    );
    spawns
}

fn sector_rows(stars: u8) {
    let spawns = sampled_spawns(stars, 30);
    let n = spawns.len();
    assert!(n >= 12, "GATE BROKEN: only {n} pursuit spawns sampled");
    let mut violations = Vec::new();
    for (k, name) in SECTORS.iter().enumerate() {
        let count = spawns.iter().filter(|s| s.sector == k).count();
        let floor = (FLOORS[k] * n as f32).ceil() as usize;
        eprintln!("  {name}: {count} of {n} (floor {floor})");
        if count < floor {
            violations.push(format!("{name}: {count} of {n} spawns, floor {floor}"));
        }
    }
    for (i, s) in spawns.iter().enumerate() {
        if !s.hidden {
            violations.push(format!("spawn {i} ({}) in view", SECTORS[s.sector]));
        }
        if !s.overlaps.is_empty() {
            violations.push(format!("spawn {i} overlaps {:?}", s.overlaps));
        }
    }
    assert!(violations.is_empty(), "{stars} stars: {violations:#?}");
}

/// 2 stars: with the despawn mutation the row cap never binds, so more stars sample the same.
#[test]
fn two_stars_cover_every_sector() {
    sector_rows(2);
}

/// No despawn mutation: over 60 s the cars with sirens on never exceed the row's cap.
#[test]
fn cars_stay_within_the_row_cap() {
    for stars in [2u8, 3] {
        let (mut app, car) = chase_scene(stars);
        let cap = app.world().resource::<EscalationConfig>().stars[usize::from(stars) - 1].cars;
        let mut worst = 0;
        for tick in 0..60 * HZ {
            chase_tick(&mut app, car);
            let active = app
                .world_mut()
                .query::<&PoliceCar>()
                .iter(app.world())
                .filter(|c| sirens_on(c.state))
                .count() as u32;
            worst = worst.max(active);
            assert!(
                active <= cap,
                "{stars} stars, tick {tick}: {active} cars with sirens on, cap {cap}"
            );
        }
        eprintln!("{stars} stars: at most {worst} cars with sirens on (cap {cap})");
        assert!(worst > 0, "GATE BROKEN: {stars} stars dispatched no car");
    }
}
