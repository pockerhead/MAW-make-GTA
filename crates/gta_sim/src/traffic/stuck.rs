//! The stuck cheat (TASK-032): a traffic car that has stood `bubble.stuck_despawn_seconds` out of
//! frame despawns, and so does a car nobody drives standing that long in (or at) a junction box out
//! of frame, farther than `bubble.stuck_in_view_distance` or with only a corner in frame; the bubble
//! refills the street. A car at a box the player looks at from nearby never pops, and there the lock
//! remains.

use super::{TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections, TrafficMode};
use super::{TrafficStats, in_frame};
use crate::navigation::flat_distance;
use crate::occupancy::RoadOccupancy;
use crate::player::Player;
use crate::police::PoliceCar;
use crate::population::{CameraView, ViewCone};
use crate::vehicle::{Vehicle, VehicleConfig};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn despawn_stuck(
    mut commands: Commands,
    configs: (Res<TrafficConfig>, Res<VehicleConfig>),
    world: (Res<CameraView>, Res<TrafficGraph>, Res<RoadOccupancy>),
    time: Res<Time<Fixed>>,
    mut junctions: ResMut<TrafficIntersections>,
    mut stats: ResMut<TrafficStats>,
    mut hidden: Local<HashMap<Entity, f32>>,
    player: Query<&Position, With<Player>>,
    cars: Query<(Entity, &Vehicle, &Position, &Rotation, Option<&TrafficCar>), Without<PoliceCar>>,
) {
    let (cfg, vehicle) = configs;
    let (view, graph, road) = world;
    let (Some(view), Ok(player)) = (view.0, player.single()) else {
        return;
    };
    let limit = cfg.bubble.stuck_despawn_seconds;
    let dt = time.delta_secs();
    let half = vehicle.half_extents();
    let mut seen = HashMap::with_capacity(hidden.len());
    for (entity, car, position, rotation, traffic) in &cars {
        // Standing in the road snapshot first: it holds only bodies near the player.
        if road.body(entity).is_none_or(|b| b.standing <= 0.0) {
            continue;
        }
        let driverless = match traffic {
            Some(t) => t.mode == TrafficMode::Abandoned,
            None => car.driver.is_none(),
        };
        // In a box or within a car length of it: at a lane end no pass reaches (`pass.rs`).
        let seen_now = if driverless && graph.in_junction(position.0, 2.0 * half.z) {
            flat_distance(position.0, player.0) <= cfg.bubble.stuck_in_view_distance
                && middle_in_frame(&view, position.0, half)
        } else if traffic.is_some_and(|t| t.mode != TrafficMode::Taken) {
            // In frame past the in-view despawn distance a traffic car is the bubble's.
            let distance = cfg.bubble.in_view.despawn;
            in_frame(&view, position.0, rotation.0, half, player.0, distance)
        } else {
            continue;
        };
        if seen_now {
            continue;
        }
        let out = hidden.get(&entity).copied().unwrap_or(0.0) + dt;
        if out < limit {
            seen.insert(entity, out);
            continue;
        }
        junctions.release(entity);
        commands.entity(entity).try_despawn();
        if traffic.is_some() {
            stats.despawned += 1;
        }
    }
    *hidden = seen;
}

/// The car's centre line (not only a corner) is inside the view cone.
fn middle_in_frame(view: &ViewCone, position: Vec3, half: Vec3) -> bool {
    [
        Vec3::new(position.x, 0.1, position.z),
        position + Vec3::Y * half.y,
    ]
    .into_iter()
    .any(|p| view.contains(p, 0.0))
}
