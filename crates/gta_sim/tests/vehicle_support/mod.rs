//! Shared fixtures of the car gates (`vehicle*.rs`).
#![allow(dead_code)]

use crate::common::*;
use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use gta_sim::{
    character::LocomotionConfig,
    combat::aim_yaw,
    layers::GameLayer,
    traffic::{TrafficGraph, TrafficLane},
    vehicle::{
        DamageConfig, DriveIntent, Driving, VehicleConfig, VehicleHealth, pursuit_steer,
        vehicle_bundle,
    },
};

pub fn vehicle_cfg(app: &App) -> VehicleConfig {
    app.world().resource::<VehicleConfig>().clone()
}

pub fn damage_cfg(app: &App) -> DamageConfig {
    app.world().resource::<DamageConfig>().clone()
}

/// A production car standing at rest height, centre above `feet_xz`, facing yaw `yaw_deg`.
pub fn spawn_car(app: &mut App, feet_xz: Vec2, yaw_deg: f32) -> Entity {
    let cfg = vehicle_cfg(app);
    let dmg = damage_cfg(app);
    let centre = Vec3::new(feet_xz.x, cfg.rest_height(), feet_xz.y);
    let rotation = Quat::from_rotation_y(yaw_deg.to_radians());
    let h = cfg.half_extents();
    let overlaps = app
        .world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            spatial.shape_intersections(
                &Collider::cuboid(2.0 * h.x, 2.0 * h.y, 2.0 * h.z),
                centre,
                rotation,
                &SpatialQueryFilter::from_mask([
                    GameLayer::World,
                    GameLayer::Character,
                    GameLayer::Vehicle,
                ]),
            )
        })
        .expect("GATE BROKEN: overlap query failed");
    assert!(
        overlaps.is_empty(),
        "GATE BROKEN: fixture overlaps the test area at {centre}: {overlaps:?}"
    );
    let car = app
        .world_mut()
        .spawn(vehicle_bundle(
            &cfg,
            &dmg,
            Transform::from_translation(centre).with_rotation(rotation),
        ))
        .id();
    run_ticks(app, 1);
    let at = position_of(app, car);
    assert!(
        Vec2::new(at.x - centre.x, at.z - centre.z).length() < 0.01,
        "GATE BROKEN: car spawned at {at}, expected {centre}"
    );
    car
}

pub fn rotation_of(app: &App, entity: Entity) -> Quat {
    app.world().get::<Rotation>(entity).unwrap().0
}

pub fn forward_of(app: &App, car: Entity) -> Vec3 {
    rotation_of(app, car) * Vec3::NEG_Z
}

pub fn velocity_of(app: &App, entity: Entity) -> Vec3 {
    app.world().get::<LinearVelocity>(entity).unwrap().0
}

pub fn door_of(app: &App, car: Entity) -> Vec3 {
    let cfg = vehicle_cfg(app);
    position_of(app, car) + rotation_of(app, car) * cfg.door()
}

pub fn driving(app: &mut App) -> Option<Entity> {
    let me = player(app);
    app.world().get::<Driving>(me).map(|d| d.vehicle)
}

pub fn request_vehicle(app: &mut App) {
    set_action(app, |a| a.vehicle_requested = true);
}

/// Puts the player at the car's door and gets in.
pub fn drive_in(app: &mut App, car: Entity) {
    let float = app.world().resource::<LocomotionConfig>().float_height;
    let door = door_of(app, car);
    place_player(app, Vec3::new(door.x, float, door.z));
    request_vehicle(app);
    run_ticks(app, 1);
    assert_eq!(driving(app), Some(car), "GATE BROKEN: could not get in");
}

pub fn kick(app: &mut App, car: Entity, speed: f32) {
    let v = forward_of(app, car) * speed;
    app.world_mut().get_mut::<LinearVelocity>(car).unwrap().0 = v;
}

pub fn set_drive(app: &mut App, update: impl FnOnce(&mut DriveIntent)) {
    let me = player(app);
    update(app.world_mut().get_mut::<DriveIntent>(me).unwrap().as_mut());
}

pub fn yaw_of(app: &App, car: Entity) -> f32 {
    aim_yaw(forward_of(app, car))
}

pub fn flat(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

pub fn car_health(app: &App, car: Entity) -> f32 {
    app.world().get::<VehicleHealth>(car).unwrap().current
}

pub fn set_car_health(app: &mut App, car: Entity, current: f32) {
    app.world_mut()
        .get_mut::<VehicleHealth>(car)
        .unwrap()
        .current = current;
}

pub fn float_height_of(app: &App) -> f32 {
    app.world().resource::<LocomotionConfig>().float_height
}

/// Steers the player's car along the lane ahead at `speed` m/s (pure pursuit 8 m ahead).
pub fn cruise(app: &mut App, car: Entity, speed: f32) {
    let cfg = app.world().resource::<VehicleConfig>().clone();
    let graph = app.world().resource::<TrafficGraph>().clone();
    let (p, f) = (position_of(app, car), forward_of(app, car));
    let v = velocity_of(app, car).dot(f);
    let lane = graph
        .lanes()
        .iter()
        .filter(|l| l.dir.dot(f) > 0.7)
        .min_by(|a, b| {
            let d = |l: &TrafficLane| {
                let s = (p - l.from).dot(l.dir).clamp(0.0, l.length);
                (l.from + l.dir * s - p).with_y(0.0).length()
            };
            d(a).total_cmp(&d(b))
        })
        .cloned();
    let steer = lane.map_or(0.0, |l| {
        let s = (p - l.from).dot(l.dir) + 8.0;
        let target = if s <= l.length {
            l.from + l.dir * s
        } else {
            let next = l
                .out
                .iter()
                .map(|&c| graph.lane(graph.connector(c).to_lane))
                .max_by(|a, b| a.dir.dot(l.dir).total_cmp(&b.dir.dot(l.dir)))
                .expect("GATE BROKEN: dead-end lane");
            next.from + next.dir * (s - l.length).min(next.length)
        };
        pursuit_steer(&cfg, p, f, v, target)
    });
    set_drive(app, |d| {
        d.throttle = ((speed - v) * 0.5).clamp(-1.0, 1.0);
        d.steer = steer;
    });
}
