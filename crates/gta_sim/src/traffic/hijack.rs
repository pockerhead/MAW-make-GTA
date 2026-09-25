//! The player takes a traffic car: the data driver is thrown out as a fleeing civilian (GDD §5.2);
//! the car becomes the player's, and an abandoned car once the player leaves it.

use super::{
    SwitchCause, TrafficCar, TrafficIntersections, TrafficMode, TrafficRng, TrafficStats, abandon,
};
use crate::character::{CharacterControlConfig, HealthConfig, LocomotionConfig};
use crate::civilian::{Civilian, CivilianConfig, CivilianState, civilian_bundle, roll_temperament};
use crate::navigation::{GraphWalker, SidewalkGraph, flee_start, nearest_node};
use crate::perception::Cause;
use crate::population::Appearance;
use crate::vehicle::{Autopilot, DriveIntent, Vehicle, VehicleEntered};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Configs a driver civilian is made from.
pub(crate) type DriverConfigs<'a> = (
    &'a LocomotionConfig,
    &'a HealthConfig,
    &'a CharacterControlConfig,
    &'a CivilianConfig,
);

/// A civilian standing at `feet`, fleeing from `from` about `about`; rolls come from `TrafficRng`.
pub(crate) fn spawn_driver(
    commands: &mut Commands,
    graph: &SidewalkGraph,
    configs: DriverConfigs,
    rng: &mut TrafficRng,
    feet: Vec3,
    from: Vec3,
    about: Option<Cause>,
) -> Option<Entity> {
    let (loco, health, handle, civilian) = configs;
    let node = nearest_node(graph, feet)?;
    let walker = GraphWalker {
        from: node,
        to: *graph.neighbors(node).first()?,
    };
    let temperament = roll_temperament(&mut rng.0, civilian.reaction.temperament_spread);
    let appearance = Appearance(rng.next_u32());
    let (lo, hi) = civilian.flee_distance;
    let left = lo + (hi - lo) * rng.unit();
    let entity = commands
        .spawn(civilian_bundle(
            loco,
            handle.0.clone(),
            health,
            graph,
            walker,
            0.0,
            temperament,
            appearance,
        ))
        .insert((
            // A Transform, not only a Position: avian copies GlobalTransform into Position.
            Transform::from_translation(feet + Vec3::Y * loco.float_height),
            Civilian {
                state: CivilianState::Flee { from, left, about },
                temperament,
            },
            flee_start(graph, walker, from),
        ))
        .id();
    Some(entity)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn on_hijack(
    mut commands: Commands,
    configs: (
        Res<LocomotionConfig>,
        Res<HealthConfig>,
        Res<CharacterControlConfig>,
        Res<CivilianConfig>,
    ),
    graph: Res<SidewalkGraph>,
    mut rng: ResMut<TrafficRng>,
    mut junctions: ResMut<TrafficIntersections>,
    mut stats: ResMut<TrafficStats>,
    mut entered: MessageReader<VehicleEntered>,
    mut cars: Query<(&mut TrafficCar, &RigidBody)>,
    drivers: Query<&Position>,
) {
    let (loco, health, handle, civilian) = &configs;
    for entry in entered.read() {
        let Ok((mut car, body)) = cars.get_mut(entry.vehicle) else {
            continue;
        };
        if car.is_ai() {
            // The player still stands where it got in: `sync_seats` seats it after the physics step.
            if let Ok(at) = drivers.get(entry.driver) {
                let feet = at.0 - Vec3::Y * loco.float_height;
                spawn_driver(
                    &mut commands,
                    &graph,
                    (loco, health, handle, civilian),
                    &mut rng,
                    feet,
                    at.0,
                    Some(Cause::Attack(entry.attack)),
                );
            }
            if body.is_kinematic() {
                stats.switches_by_cause[SwitchCause::Hijack as usize] += 1;
            }
        }
        commands
            .entity(entry.vehicle)
            .try_insert(RigidBody::Dynamic);
        commands
            .entity(entry.vehicle)
            .try_remove::<(Autopilot, DriveIntent)>();
        junctions.release(entry.vehicle);
        car.mode = TrafficMode::Taken;
        car.next = None;
        car.waiting = None;
    }
}

/// A taken car the player left (F out, a forced eject) is abandoned.
pub(super) fn on_leave(
    mut commands: Commands,
    mut junctions: ResMut<TrafficIntersections>,
    mut cars: Query<(Entity, &mut TrafficCar, &Vehicle)>,
) {
    for (entity, mut car, vehicle) in &mut cars {
        if car.mode == TrafficMode::Taken && vehicle.driver.is_none() {
            abandon(&mut commands, &mut junctions, entity, &mut car);
        }
    }
}
