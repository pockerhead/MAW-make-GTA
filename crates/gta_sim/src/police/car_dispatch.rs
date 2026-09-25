//! Police car dispatch (GDD §5.3, §6.4): `row.cars` cars on the job, crews counted in the row's units,
//! spawned on hidden lane points of the car ring; the car bubble.

use super::car_route::{approach_clear, lane_costs_to};
use super::cars::{PoliceCar, PoliceCarRoute, PoliceCarState};
use super::fsm::{pick_spawn, spawn_kind};
use super::{CopState, EscalationConfig, PoliceDispatcher, PoliceUnit, UnitKind};
use crate::combat::aim_yaw;
use crate::layers::GameLayer;
use crate::navigation::flat_distance;
use crate::player::Player;
use crate::population::{
    CameraView, OCCLUSION_RAYS_PER_POINT, Offscreen, PopulationConfig, PopulationLoad, occluded,
};
use crate::traffic::{Segment, TrafficCar, TrafficConfig, TrafficGraph, in_frame};
use crate::vehicle::{
    Autopilot, DamageConfig, DriveIntent, Driving, Vehicle, VehicleConfig, vehicle_bundle,
};
use crate::wanted::WantedLevel;
use avian3d::prelude::*;
use bevy::prelude::*;

/// Leaving and abandoned cars follow the traffic bubble rule; engaged cars go once off frame and past
/// the population bubble like foot cops; a car the player left is abandoned.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn despawn_police_cars(
    mut commands: Commands,
    configs: (
        Res<TrafficConfig>,
        Res<PopulationConfig>,
        Res<VehicleConfig>,
    ),
    view: Res<CameraView>,
    time: Res<Time<Fixed>>,
    player: Query<&Position, With<Player>>,
    mut cars: Query<(
        Entity,
        &mut PoliceCar,
        &Vehicle,
        &Position,
        &Rotation,
        &mut Offscreen,
    )>,
) {
    let (traffic, population, vehicle_cfg) = configs;
    for (_, mut car, vehicle, ..) in &mut cars {
        if car.state == PoliceCarState::Taken && vehicle.driver.is_none() {
            car.state = PoliceCarState::Abandoned;
        }
    }
    let (Some(view), Ok(player)) = (view.0, player.single()) else {
        return;
    };
    let bubble = &traffic.bubble;
    let half = vehicle_cfg.half_extents();
    let dt = time.delta_secs();
    for (entity, car, _, position, rotation, mut offscreen) in &mut cars {
        if car.state == PoliceCarState::Taken {
            offscreen.0 = 0.0;
            continue;
        }
        let framed = in_frame(
            &view,
            position.0,
            rotation.0,
            half,
            player.0,
            bubble.in_view.despawn,
        );
        offscreen.0 = if framed { 0.0 } else { offscreen.0 + dt };
        let distance = flat_distance(position.0, player.0);
        let gone = if car.active() {
            offscreen.0 >= population.despawn_offscreen_seconds
                && distance > population.despawn_distance
        } else {
            offscreen.0 >= bubble.offscreen_seconds && distance > bubble.off_view.despawn
        };
        if gone {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Keeps `row.cars` active police cars while the row's units (foot plus aboard) allow one more cop;
/// a car spawns on a lane point of the car ring hidden from the camera. While the player drives a
/// moving car (a pursuit) the point also needs no AI traffic ahead on its lane route to him: a police
/// car cannot pass traffic (one reverse route search per tick, only while a car is missing).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn dispatch_police_cars(
    mut commands: Commands,
    configs: (
        Res<EscalationConfig>,
        Res<PopulationConfig>,
        Res<VehicleConfig>,
        Res<DamageConfig>,
    ),
    state: (Res<WantedLevel>, Res<CameraView>, Res<TrafficGraph>),
    mut dispatcher: ResMut<PoliceDispatcher>,
    mut load: ResMut<PopulationLoad>,
    spatial: SpatialQuery,
    player: Query<(&Position, Option<&Driving>), With<Player>>,
    speeds: Query<&LinearVelocity>,
    units: Query<(&Position, &PoliceUnit)>,
    cars: Query<(&Position, &PoliceCar)>,
    traffic: Query<&TrafficCar>,
) {
    let (esc, population, vehicle, dmg) = configs;
    let (wanted, view, graph) = state;
    let foot: Vec<(Vec3, UnitKind)> = units
        .iter()
        .filter(|(_, u)| !matches!(u.state, CopState::Dead | CopState::Leave))
        .map(|(p, u)| (p.0, u.kind))
        .collect();
    let mut out: Vec<Vec3> = foot.iter().map(|f| f.0).collect();
    let mut kinds: Vec<UnitKind> = foot.iter().map(|f| f.1).collect();
    let mut active = 0u32;
    for (position, car) in cars.iter().filter(|(_, c)| c.active()) {
        active += 1;
        out.push(position.0);
        kinds.extend(car.crew.iter().copied());
    }
    dispatcher.cars = active;
    let row = (wanted.stars >= 1).then(|| &esc.stars[usize::from(wanted.stars) - 1]);
    let (Some(row), Some(view), Some(centre), Ok((player, driving))) =
        (row, view.0, wanted.last_known, player.single())
    else {
        return;
    };
    let pursuit = driving
        .and_then(|d| speeds.get(d.vehicle).ok())
        .is_some_and(|v| v.0.length() > vehicle.exit_max_speed);
    let c = &esc.car;
    let half = vehicle.half_extents();
    let (inner, outer) = c.spawn_ring;
    let chassis = Collider::cuboid(2.0 * half.x, 2.0 * half.y, 2.0 * half.z);
    let filter = SpatialQueryFilter::from_mask([GameLayer::Character, GameLayer::Vehicle]);
    let margin = population.spawn_view_margin_deg.to_radians();
    let height = vehicle.rest_height();
    // Only lanes heading towards the target: without U-turns a car facing away loops a block first.
    let mut candidates: Vec<(Vec3, Vec3, (u32, f32))> = graph
        .spawn_points()
        .iter()
        .map(|&(lane, s)| {
            let (p, t) = graph.pose(Segment::Lane(lane), s);
            (p, t, (lane, s))
        })
        .filter(|(p, t, _)| {
            (inner..=outer).contains(&flat_distance(*p, player.0)) && t.dot(centre - *p) > 0.0
        })
        .collect();
    let queue: Vec<(Segment, f32)> = traffic
        .iter()
        .filter(|t| t.is_ai())
        .map(|t| (t.segment, t.s))
        .collect();
    let radius = graph
        .nearest(player.0)
        .map_or(0.0, |(_, _, d)| d + c.goal_margin);
    let mut costs: Option<Vec<u32>> = None;
    let mut spawned = 0;
    while spawned < c.spawns_per_tick && active < row.cars {
        let units = kinds.len() as u32;
        if units + 1 > row.units {
            break;
        }
        let points: Vec<Vec3> = candidates.iter().map(|c| c.0).collect();
        let Some(k) = pick_spawn(&points, centre, &out, row.surround) else {
            break;
        };
        let (at, tangent, spot) = candidates.swap_remove(k);
        if pursuit {
            let costs = costs.get_or_insert_with(|| lane_costs_to(&graph, player.0, radius));
            if !approach_clear(&graph, costs, &queue, spot, player.0, radius) {
                continue;
            }
        }
        let rotation = Quat::from_rotation_y(aim_yaw(tangent));
        let body = at + Vec3::Y * height;
        let corners = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .map(|(x, z)| body + rotation * Vec3::new(x * half.x, half.y, z * half.z));
        let hidden = corners.iter().all(|&p| !view.contains(p, margin))
            || (load.rays + OCCLUSION_RAYS_PER_POINT <= population.occlusion_rays_per_tick
                && occluded(&spatial, &view, at, height + half.y, half.x, &mut load.rays));
        if !hidden
            || !spatial
                .shape_intersections(&chassis, body, rotation, &filter)
                .is_empty()
        {
            continue;
        }
        let size = c.crew.min(row.units - units);
        let mut crew = Vec::with_capacity(size as usize);
        for _ in 0..size {
            let swat = kinds.iter().filter(|k| **k == UnitKind::Swat).count() as u32;
            let Some(kind) = spawn_kind(row, kinds.len() as u32, swat) else {
                break;
            };
            crew.push(kind);
            kinds.push(kind);
        }
        let transform = Transform::from_translation(body).with_rotation(rotation);
        // `vehicle_bundle` holds a `Name`: a duplicate in one bundle panics, so it is replaced after.
        commands
            .spawn((
                vehicle_bundle(&vehicle, &dmg, transform),
                PoliceCar {
                    state: PoliceCarState::Respond,
                    crew,
                    stopped: 0.0,
                    moving: 0.0,
                    blocked: 0.0,
                    reboard_left: c.reboard_timeout_seconds,
                },
                PoliceCarRoute::default(),
                Autopilot {
                    target: body + tangent * vehicle.autopilot.lookahead_min,
                    ..default()
                },
                DriveIntent::default(),
                SleepingDisabled,
            ))
            .insert(Name::new("Police car"));
        out.push(body);
        active += 1;
        spawned += 1;
    }
    dispatcher.cars = active;
}
