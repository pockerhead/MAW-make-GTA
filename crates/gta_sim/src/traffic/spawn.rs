//! The traffic bubble (GDD §5.2, Vermeij): in frame cars spawn at 70-90 m and go past 90 m, off
//! frame they spawn at 15-25 m and go past 25 m, and never before 2 s off frame.

use super::{
    Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections, TrafficMode,
    TrafficPhase, TrafficRng, TrafficStats,
};
use crate::combat::aim_yaw;
use crate::layers::GameLayer;
use crate::navigation::flat_distance;
use crate::player::Player;
use crate::population::{Appearance, CameraView, Offscreen, ViewCone};
use crate::vehicle::{DamageConfig, VehicleConfig, vehicle_bundle};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

/// A car body at `position` / `rotation` (chassis half extents `half`) is in frame: a top corner of
/// the chassis or its centre line is inside the view cone, within `distance` (flat) of the player.
pub fn in_frame(
    view: &ViewCone,
    position: Vec3,
    rotation: Quat,
    half: Vec3,
    player: Vec3,
    distance: f32,
) -> bool {
    if flat_distance(position, player) > distance {
        return false;
    }
    let corners = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
        .map(|(x, z)| position + rotation * Vec3::new(x * half.x, half.y, z * half.z));
    let line = [
        Vec3::new(position.x, 0.1, position.z),
        position + Vec3::Y * half.y,
    ];
    corners
        .into_iter()
        .chain(line)
        .any(|p| view.contains(p, 0.0))
}

/// Spawns a kinematic traffic car centred on `(seg, s)` moving at `speed`; the production spawn path.
#[allow(clippy::too_many_arguments)]
pub fn spawn_traffic_car(
    commands: &mut Commands,
    graph: &TrafficGraph,
    cfg: &VehicleConfig,
    dmg: &DamageConfig,
    seg: Segment,
    s: f32,
    speed: f32,
    appearance: u32,
) -> Entity {
    let (point, tangent) = graph.pose(seg, s);
    let transform = Transform::from_xyz(point.x, cfg.rest_height(), point.z)
        .with_rotation(Quat::from_rotation_y(aim_yaw(tangent)));
    // `vehicle_bundle` holds `RigidBody::Dynamic` and `Name`: a duplicate in one bundle panics, so
    // both are replaced after the spawn (the client observer of `Add<Vehicle>` sees the markers).
    commands
        .spawn((
            vehicle_bundle(cfg, dmg, transform),
            TrafficCar {
                segment: seg,
                s,
                speed,
                next: None,
                mode: TrafficMode::Kinematic,
                waiting: None,
            },
            Appearance(appearance),
        ))
        .insert((RigidBody::Kinematic, Name::new("Traffic car")))
        .id()
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn despawn_traffic(
    mut commands: Commands,
    configs: (Res<TrafficConfig>, Res<VehicleConfig>),
    view: Res<CameraView>,
    time: Res<Time<Fixed>>,
    mut junctions: ResMut<TrafficIntersections>,
    mut stats: ResMut<TrafficStats>,
    player: Query<&Position, With<Player>>,
    mut cars: Query<(Entity, &TrafficCar, &Position, &Rotation, &mut Offscreen)>,
) {
    let (cfg, vehicle) = configs;
    let (Some(view), Ok(player)) = (view.0, player.single()) else {
        return;
    };
    let bubble = &cfg.bubble;
    let dt = time.delta_secs();
    let half = vehicle.half_extents();
    for (entity, car, position, rotation, mut offscreen) in &mut cars {
        if car.mode == TrafficMode::Taken {
            offscreen.0 = 0.0;
            continue;
        }
        if in_frame(
            &view,
            position.0,
            rotation.0,
            half,
            player.0,
            bubble.in_view.despawn,
        ) {
            offscreen.0 = 0.0;
            continue;
        }
        offscreen.0 += dt;
        if offscreen.0 >= bubble.offscreen_seconds
            && flat_distance(position.0, player.0) > bubble.off_view.despawn
        {
            junctions.release(entity);
            commands.entity(entity).try_despawn();
            stats.despawned += 1;
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn spawn_traffic(
    mut commands: Commands,
    spatial: SpatialQuery,
    configs: (Res<TrafficConfig>, Res<VehicleConfig>, Res<DamageConfig>),
    graph: Res<TrafficGraph>,
    view: Res<CameraView>,
    mut phase: ResMut<TrafficPhase>,
    mut rng: ResMut<TrafficRng>,
    mut stats: ResMut<TrafficStats>,
    player: Query<&Position, With<Player>>,
    cars: Query<&TrafficCar>,
) {
    let (cfg, vehicle, dmg) = configs;
    let (Some(view), Ok(player)) = (view.0, player.single()) else {
        return;
    };
    let bubble = &cfg.bubble;
    let idm = &cfg.idm;
    let alive = cars.iter().filter(|c| c.mode != TrafficMode::Taken).count() as u32;
    let deficit = bubble.max_cars.saturating_sub(alive);
    if deficit == 0 {
        *phase = TrafficPhase::Steady;
        return;
    }
    let initial = *phase == TrafficPhase::InitialFill;
    let budget = if initial {
        bubble.initial_spawns_per_tick
    } else {
        bubble.spawns_per_tick
    }
    .min(deficit);
    let half = vehicle.half_extents();
    let half_length = half.z;
    // Distance from a car centred at `s` to where IDM stops it before the stop line (bumper s0 short).
    let to_stop_line = |stop: f32, s: f32| stop - s - half_length - idm.min_gap;
    let mut on_lane: HashMap<u32, Vec<f32>> = HashMap::new();
    for car in cars.iter().filter(|c| c.is_ai()) {
        if let Segment::Lane(l) = car.segment {
            on_lane.entry(l).or_default().push(car.s);
        }
    }
    let mut candidates: Vec<(u32, f32)> = graph
        .spawn_points()
        .iter()
        .copied()
        .filter(|&(lane, s)| {
            let l = graph.lane(lane);
            let (point, tangent) = graph.pose(Segment::Lane(lane), s);
            let d = flat_distance(point, player.0);
            let band = if initial {
                (bubble.off_view.spawn, bubble.in_view.despawn)
            } else {
                let rotation = Quat::from_rotation_y(aim_yaw(tangent));
                let centre = point + Vec3::Y * vehicle.rest_height();
                if in_frame(&view, centre, rotation, half, player.0, f32::INFINITY) {
                    (bubble.in_view.spawn, bubble.in_view.despawn)
                } else {
                    (bubble.off_view.spawn, bubble.off_view.despawn)
                }
            };
            if !(band.0 <= d && d < band.1) {
                return false;
            }
            // Room before the stop line, and a free stretch around the spot.
            let clear = idm.min_gap + l.v0 * idm.time_headway + 2.0 * half_length;
            to_stop_line(l.stop, s) > 0.0
                && on_lane
                    .get(&lane)
                    .is_none_or(|cars| cars.iter().all(|&c| (c - s).abs() > clear))
        })
        .collect();
    let chassis = Collider::cuboid(2.0 * half.x, 2.0 * half.y, 2.0 * half.z);
    let filter = SpatialQueryFilter::from_mask([GameLayer::Character, GameLayer::Vehicle]);
    let mut spawned: Vec<(u32, f32)> = Vec::new();
    let mut checks = 0;
    while (spawned.len() as u32) < budget && !candidates.is_empty() && checks < 4 * budget {
        let (lane, s) = candidates.swap_remove(rng.next_u32() as usize % candidates.len());
        let clear = idm.min_gap + graph.lane(lane).v0 * idm.time_headway + 2.0 * half_length;
        if spawned
            .iter()
            .any(|&(l, t)| l == lane && (t - s).abs() <= clear)
        {
            continue;
        }
        checks += 1;
        let (point, tangent) = graph.pose(Segment::Lane(lane), s);
        let centre = point + Vec3::Y * vehicle.rest_height();
        let rotation = Quat::from_rotation_y(aim_yaw(tangent));
        if !spatial
            .shape_intersections(&chassis, centre, rotation, &filter)
            .is_empty()
        {
            continue;
        }
        // A car near the lane end starts slow enough to stop comfortably at the line.
        let l = graph.lane(lane);
        let speed =
            l.v0.min((2.0 * idm.comfortable_deceleration * to_stop_line(l.stop, s)).sqrt());
        spawn_traffic_car(
            &mut commands,
            &graph,
            &vehicle,
            &dmg,
            Segment::Lane(lane),
            s,
            speed,
            rng.next_u32(),
        );
        stats.spawned += 1;
        spawned.push((lane, s));
    }
    if initial && ((spawned.len() as u32) < budget || spawned.len() as u32 == deficit) {
        *phase = TrafficPhase::Steady;
    }
}
