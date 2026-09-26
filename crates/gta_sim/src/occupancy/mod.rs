//! What occupies the road (TASK-032): one flat snapshot per fixed tick of every vehicle body and every
//! enabled character near the player, with velocity, standing time, siren flag, and the pass claims
//! of traffic cars going around a standing body. Traffic sensing, lane starts, junction grants, the
//! traffic spawner and the police lane choice query it (`query.rs`); the claims make a passer visible
//! to oncoming cars before it gets there.

mod query;

pub use query::{ClaimFilter, Hit, Strip, world_clear};

use crate::character::{Character, LocomotionConfig};
use crate::flow::{NEW_CITY, NpcSystems};
use crate::navigation::flat_distance;
use crate::player::Player;
use crate::police::{PoliceCar, sirens_on};
use crate::traffic::{
    FlatRect, Manoeuvre, TrafficCar, TrafficConfig, TrafficGraph, TrafficMode, TrafficSystems,
    derived_claim,
};
use crate::vehicle::{Vehicle, VehicleConfig};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

/// How a road body is seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// A kinematic AI car on its path line with no manoeuvre: exactly the cars the path occupancy of
    /// `advance_traffic` holds.
    OnPathTraffic,
    /// Any other AI car (off its path line, dynamic, bailing dynamic).
    Traffic,
    /// Any other vehicle (abandoned or taken traffic, parked, the player's car, police).
    Vehicle,
    Character,
}

#[derive(Clone, Copy, Debug)]
pub enum Footprint {
    Rect(FlatRect),
    Circle { centre: Vec2, radius: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct RoadBody {
    pub entity: Entity,
    pub kind: BodyKind,
    pub shape: Footprint,
    pub velocity: Vec2,
    pub dynamic: bool,
    /// Seconds at or below the vehicle `hold_speed`.
    pub standing: f32,
    pub siren: bool,
}

/// The road stretch a passer will drive through (`dir`: its travel direction).
#[derive(Clone, Copy, Debug)]
pub struct Claim {
    pub owner: Entity,
    pub rect: FlatRect,
    pub dir: Vec2,
}

/// The road as this fixed tick sees it (built before traffic and police drive).
#[derive(Resource, Default)]
pub struct RoadOccupancy {
    bodies: Vec<RoadBody>,
    index: HashMap<Entity, usize>,
    claims: Vec<Claim>,
    /// Standing seconds and flat position of each body last tick.
    standing: HashMap<Entity, (f32, Vec2)>,
}

/// Builds the snapshot: after the hijack and bail sets, before traffic drives or spawns.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct OccupancySystems;

pub fn flat(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

fn kind_of(car: Option<&TrafficCar>, body: &RigidBody, vehicle: bool) -> BodyKind {
    let Some(car) = car.filter(|c| c.is_ai()) else {
        return if vehicle {
            BodyKind::Vehicle
        } else {
            BodyKind::Character
        };
    };
    let on_path = body.is_kinematic()
        && matches!(
            car.mode,
            TrafficMode::Kinematic | TrafficMode::Bailing { .. }
        )
        && car.lateral == 0.0
        && car.manoeuvre == Manoeuvre::None;
    if on_path {
        BodyKind::OnPathTraffic
    } else {
        BodyKind::Traffic
    }
}

#[allow(clippy::type_complexity)]
fn snapshot_road(
    configs: (
        Res<VehicleConfig>,
        Res<LocomotionConfig>,
        Res<TrafficConfig>,
    ),
    graph: Res<TrafficGraph>,
    time: Res<Time<Fixed>>,
    mut occupancy: ResMut<RoadOccupancy>,
    player: Query<&Position, With<Player>>,
    bodies: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &LinearVelocity,
            &RigidBody,
            Option<&TrafficCar>,
            Option<&PoliceCar>,
            Has<Vehicle>,
            Has<ColliderDisabled>,
            Has<RigidBodyDisabled>,
        ),
        Or<(With<Vehicle>, With<Character>)>,
    >,
) {
    let (vehicle, loco, traffic) = configs;
    let dt = time.delta_secs();
    let half = vehicle.half_extents();
    let radius = traffic.bubble.in_view.despawn + traffic.look_ahead;
    let centre = player.single().ok().map(|p| p.0);
    let occupancy = &mut *occupancy;
    occupancy.bodies.clear();
    occupancy.index.clear();
    occupancy.claims.clear();
    let mut standing = HashMap::with_capacity(occupancy.standing.len());
    for (entity, position, rotation, velocity, body, car, police, is_vehicle, off, disabled) in
        &bodies
    {
        // Seated drivers and the player in a car: avian queries skip them too.
        if off || disabled || centre.is_some_and(|c| flat_distance(c, position.0) > radius) {
            continue;
        }
        let v = flat(velocity.0);
        let at = flat(position.0);
        // Moving, or put somewhere else since last tick (a teleport, a respawn): not standing.
        let still = v.length() <= vehicle.hold_speed;
        let stood = match occupancy.standing.get(&entity) {
            Some(&(t, last)) if still && at.distance(last) <= vehicle.hold_speed * dt => t + dt,
            None if still => dt,
            _ => 0.0,
        };
        standing.insert(entity, (stood, at));
        let shape = if is_vehicle {
            Footprint::Rect(FlatRect::of(
                position.0,
                rotation.0,
                Vec2::new(half.x, half.z),
            ))
        } else {
            Footprint::Circle {
                centre: flat(position.0),
                radius: loco.capsule_radius,
            }
        };
        // Only a car the AI still drives on claims road: a hijacked or abandoned one keeps its fields,
        // and a bailing one stops where it is (it keeps its offset, so the manoeuvre stays).
        if let Some(car) =
            car.filter(|c| matches!(c.mode, TrafficMode::Kinematic | TrafficMode::Dynamic))
        {
            let claim = derived_claim(&graph, car, entity, &vehicle, &traffic);
            occupancy.claims.extend(claim);
        }
        occupancy.index.insert(entity, occupancy.bodies.len());
        occupancy.bodies.push(RoadBody {
            entity,
            kind: kind_of(car, body, is_vehicle),
            shape,
            velocity: v,
            dynamic: body.is_dynamic(),
            standing: stood,
            siren: police.is_some_and(|p| sirens_on(p.state)),
        });
    }
    occupancy.standing = standing;
}

fn reset_occupancy(mut occupancy: ResMut<RoadOccupancy>) {
    *occupancy = RoadOccupancy::default();
}

pub struct OccupancyPlugin;

impl Plugin for OccupancyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RoadOccupancy>()
            .configure_sets(
                FixedUpdate,
                OccupancySystems
                    .after(TrafficSystems::Hijack)
                    .after(TrafficSystems::Bail)
                    .in_set(NpcSystems)
                    .run_if(resource_exists::<TrafficGraph>),
            )
            .add_systems(FixedUpdate, snapshot_road.in_set(OccupancySystems))
            .add_systems(NEW_CITY, reset_occupancy);
    }
}
