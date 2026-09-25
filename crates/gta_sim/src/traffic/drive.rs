//! `advance_traffic`: one bounded pass over the AI cars per tick in entity order — path upkeep,
//! intersections, IDM on the nearest obstacle (leader, stop line, forward cast), kinematic motion
//! or autopilot targets, and the driver getting out of a stopped bailing car.

use super::hijack::spawn_driver;
use super::idm::{ballistic_step, idm_acceleration};
use super::junction::{self, has_grant};
use super::{
    Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections, TrafficMode,
    TrafficRng, TrafficStats, abandon,
};
use crate::character::{CharacterControlConfig, HealthConfig, LocomotionConfig};
use crate::civilian::CivilianConfig;
use crate::combat::aim_yaw;
use crate::layers::GameLayer;
use crate::navigation::SidewalkGraph;
use crate::perception::Cause;
use crate::vehicle::{
    Autopilot, Vehicle, VehicleConfig, VehicleHealth, WheelState, exit_spots, follow_speed,
};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// A traffic AI car as this tick sees it.
pub(super) struct Snap {
    pub(super) entity: Entity,
    pub(super) car: TrafficCar,
    pub(super) position: Vec3,
    pub(super) rotation: Quat,
    pub(super) velocity: Vec3,
    pub(super) dynamic: bool,
    pub(super) health: f32,
    /// Given up this tick.
    pub(super) abandon: bool,
}

/// Cars per segment, sorted by `s`; a car on a connector also sits on its source lane past the lane
/// end (its rear may still be there).
pub(super) type Occupancy = HashMap<Segment, Vec<(f32, usize)>>;

fn occupancy(graph: &TrafficGraph, snaps: &[Snap]) -> Occupancy {
    let mut map: Occupancy = HashMap::new();
    for (k, snap) in snaps.iter().enumerate().filter(|(_, s)| !s.abandon) {
        map.entry(snap.car.segment)
            .or_default()
            .push((snap.car.s, k));
        if let Segment::Connector(c) = snap.car.segment {
            let lane = graph.connector(c).from_lane;
            let s = graph.lane(lane).length + snap.car.s;
            map.entry(Segment::Lane(lane)).or_default().push((s, k));
        }
    }
    for cars in map.values_mut() {
        cars.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    map
}

/// The segment after `seg` on a car's path (`next`: its chosen connector at a lane end).
fn successor(graph: &TrafficGraph, seg: Segment, next: Option<u32>) -> Option<Segment> {
    match seg {
        Segment::Lane(_) => next.map(Segment::Connector),
        Segment::Connector(c) => Some(Segment::Lane(graph.connector(c).to_lane)),
    }
}

/// The place `distance` m ahead of `(seg, s)` along the path; stops at the end of the known path.
fn ahead(
    graph: &TrafficGraph,
    seg: Segment,
    s: f32,
    next: Option<u32>,
    distance: f32,
) -> (Segment, f32) {
    let (mut seg, mut s) = (seg, s + distance);
    for _ in 0..3 {
        let length = graph.length(seg);
        if s <= length {
            break;
        }
        let Some(after) = successor(graph, seg, next) else {
            return (seg, length);
        };
        s -= length;
        seg = after;
    }
    (seg, s)
}

/// Bumper gap to the nearest car ahead on the path within `look_ahead`, and its speed.
fn leader(
    graph: &TrafficGraph,
    occupancy: &Occupancy,
    snaps: &[Snap],
    me: usize,
    half_length: f32,
    look_ahead: f32,
) -> Option<(f32, f32)> {
    let car = &snaps[me].car;
    let (mut seg, mut offset) = (car.segment, -car.s);
    for _ in 0..3 {
        let found = occupancy.get(&seg).and_then(|cars| {
            cars.iter()
                .filter(|&&(_, k)| k != me)
                .map(|&(s, k)| (offset + s, k))
                .find(|&(d, _)| d > 0.0)
        });
        if let Some((d, k)) = found {
            return (d <= look_ahead).then(|| (d - 2.0 * half_length, snaps[k].car.speed));
        }
        offset += graph.length(seg);
        if offset > look_ahead {
            return None;
        }
        seg = successor(graph, seg, car.next)?;
    }
    None
}

/// Wrapped yaw difference in `(-PI, PI]`.
fn wrap(angle: f32) -> f32 {
    let a = (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
    a - std::f32::consts::PI
}

/// Re-projects a dynamic car onto its own path; `true` when it is lost.
fn reproject(
    graph: &TrafficGraph,
    junctions: &TrafficIntersections,
    snap: &mut Snap,
    cfg: &TrafficConfig,
) -> bool {
    for _ in 0..3 {
        let s = graph.project(snap.car.segment, snap.position);
        snap.car.s = s.max(0.0);
        let length = graph.length(snap.car.segment);
        match snap.car.segment {
            Segment::Lane(_) if s >= length && has_grant(graph, junctions, snap) => {
                let Some(c) = snap.car.next else {
                    break;
                };
                snap.car.segment = Segment::Connector(c);
            }
            Segment::Connector(c) if s >= length - 1e-3 => {
                snap.car.segment = Segment::Lane(graph.connector(c).to_lane);
                snap.car.next = None;
                snap.car.waiting = None;
            }
            _ => break,
        }
    }
    let (point, tangent) = graph.pose(snap.car.segment, snap.car.s);
    let off = (snap.position - point).with_y(0.0).length();
    let forward = (snap.rotation * Vec3::NEG_Z)
        .with_y(0.0)
        .normalize_or_zero();
    let heading = forward.angle_between(tangent);
    let upside_down = (snap.rotation * Vec3::Y).y < 0.0;
    off > cfg.lost.distance || heading > cfg.lost.angle_deg.to_radians() || upside_down
}

#[allow(clippy::type_complexity)]
type CarItem<'a> = (
    Entity,
    &'a mut TrafficCar,
    &'a Position,
    &'a Rotation,
    &'a mut LinearVelocity,
    &'a mut AngularVelocity,
    &'a mut Vehicle,
    &'a VehicleHealth,
    &'a RigidBody,
    Option<&'a mut Autopilot>,
);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn advance_traffic(
    mut commands: Commands,
    spatial: SpatialQuery,
    configs: (
        Res<TrafficConfig>,
        Res<VehicleConfig>,
        Res<LocomotionConfig>,
    ),
    drivers: (
        Res<HealthConfig>,
        Res<CharacterControlConfig>,
        Res<CivilianConfig>,
    ),
    graphs: (Res<TrafficGraph>, Res<SidewalkGraph>),
    time: Res<Time<Fixed>>,
    mut junctions: ResMut<TrafficIntersections>,
    mut rng: ResMut<TrafficRng>,
    mut stats: ResMut<TrafficStats>,
    mut cars: Query<CarItem>,
    others: Query<(&Position, &LinearVelocity), Without<TrafficCar>>,
) {
    let (cfg, vcfg, loco) = configs;
    let (health_cfg, handle, civilian) = drivers;
    let (graph, sidewalks) = graphs;
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let step = time.timestep().as_nanos().max(1);
    let tick = (time.elapsed().as_nanos() / step) as u64;
    let half = vcfg.half_extents();
    let half_length = half.z;
    let idm = &cfg.idm;

    let mut counts = TrafficStats {
        spawned: stats.spawned,
        despawned: stats.despawned,
        casts: stats.casts,
        switches_by_cause: stats.switches_by_cause,
        ..default()
    };
    let mut snaps: Vec<Snap> = Vec::new();
    for (entity, car, position, rotation, velocity, _, _, health, body, _) in &cars {
        match car.mode {
            TrafficMode::Kinematic => counts.kinematic += 1,
            TrafficMode::Dynamic => counts.dynamic += 1,
            TrafficMode::Bailing { .. } => counts.bailing += 1,
            TrafficMode::Taken => counts.taken += 1,
            TrafficMode::Abandoned => counts.abandoned += 1,
        }
        if !car.is_ai() {
            continue;
        }
        snaps.push(Snap {
            entity,
            car: *car,
            position: position.0,
            rotation: rotation.0,
            velocity: velocity.0,
            dynamic: body.is_dynamic(),
            health: health.current,
            abandon: false,
        });
    }
    counts.cars = counts.kinematic + counts.dynamic + counts.bailing + counts.abandoned;
    snaps.sort_by_key(|s| s.entity.to_bits());

    // 1. Dynamic cars: back onto their path, or given up. 2. Wrecks bail out.
    for snap in &mut snaps {
        if snap.dynamic {
            let forward = snap.rotation * Vec3::NEG_Z;
            snap.car.speed = snap.velocity.dot(forward).max(0.0);
            snap.abandon = reproject(&graph, &junctions, snap, &cfg);
        }
        if snap.health <= 0.0
            && matches!(snap.car.mode, TrafficMode::Kinematic | TrafficMode::Dynamic)
        {
            snap.car.mode = TrafficMode::Bailing {
                attack: None,
                shooter: None,
            };
        }
    }

    // 3. Occupancy, 4. intersections.
    let occupied = occupancy(&graph, &snaps);
    let rest = vcfg.rest_height();
    // Vehicles outside the occupancy (abandoned, taken, police) on the first `length` m of `lane`.
    let others_on_lane = SpatialQueryFilter::from_mask(GameLayer::Vehicle)
        .with_excluded_entities(snaps.iter().filter(|s| !s.abandon).map(|s| s.entity));
    let lane_start_free = |lane: u32, length: f32| {
        let l = graph.lane(lane);
        let length = length.min(l.length);
        let centre = l.from + l.dir * (length / 2.0) + Vec3::Y * rest;
        spatial
            .shape_intersections(
                &Collider::cuboid(2.0 * half.x, 2.0 * half.y, length),
                centre,
                Quat::from_rotation_y(aim_yaw(l.dir)),
                &others_on_lane,
            )
            .is_empty()
    };
    junction::update(
        &graph,
        &mut junctions,
        &mut snaps,
        &occupied,
        idm,
        half_length,
        tick,
        &mut rng,
        &lane_start_free,
    );

    // 5. Obstacles and 6. acceleration.
    let kinematic: HashSet<Entity> = snaps
        .iter()
        .filter(|s| !s.dynamic)
        .map(|s| s.entity)
        .collect();
    let slab = Collider::cuboid(2.0 * half.x, 2.0 * half.y, 0.1);
    // The cast starts at the nose (slab front face on the bumper): a body pressed against the flank is
    // beside the car, not in its way, and must not hold it (a walker and a car waiting on each other).
    let nose = half_length - 0.05;
    let filter = SpatialQueryFilter::from_mask([GameLayer::Character, GameLayer::Vehicle]);
    let speed_of = |e: Entity| {
        cars.get(e)
            .map(|c| c.4.0)
            .ok()
            .or_else(|| others.get(e).ok().map(|o| o.1.0))
            .unwrap_or(Vec3::ZERO)
    };
    let mut accelerations = vec![0.0; snaps.len()];
    for k in 0..snaps.len() {
        let snap = &snaps[k];
        if snap.abandon {
            continue;
        }
        if matches!(snap.car.mode, TrafficMode::Bailing { .. }) {
            accelerations[k] = -idm.max_deceleration;
            continue;
        }
        let car = &snap.car;
        let v = car.speed;
        let granted = has_grant(&graph, &junctions, snap);
        let v0 = match car.segment {
            Segment::Lane(l) => {
                let lane = graph.lane(l);
                // A chosen turn caps the speed so that the connector is reached at `turn_speed`.
                let to_end = (lane.length - car.s - half_length).max(0.0);
                let approach = (cfg.turn_speed * cfg.turn_speed
                    + 2.0 * idm.comfortable_deceleration * to_end)
                    .sqrt();
                if car.next.is_some() {
                    lane.v0.min(approach)
                } else {
                    lane.v0
                }
            }
            Segment::Connector(_) => cfg.turn_speed,
        };
        let mut a = idm_acceleration(v, v0, None, idm);
        let mut obstacle = |gap: f32, other: f32| {
            a = a.min(idm_acceleration(v, v0, Some((gap, v - other)), idm));
        };
        if let Some((gap, other)) =
            leader(&graph, &occupied, &snaps, k, half_length, cfg.look_ahead)
        {
            obstacle(gap, other);
        }
        if let (Segment::Lane(l), false) = (car.segment, granted) {
            obstacle(graph.lane(l).length - (car.s + half_length), 0.0);
        }
        let (_, tangent) = graph.pose(car.segment, car.s);
        let reach = match car.segment {
            Segment::Lane(_) => cfg.sense_distance,
            Segment::Connector(_) => cfg.turn_sense_distance,
        };
        if let Ok(direction) = Dir3::new(tangent) {
            stats.casts = stats.casts.wrapping_add(1);
            counts.casts = stats.casts;
            let me = snap.entity;
            let config = ShapeCastConfig::from_max_distance(half_length + reach - nose);
            let hit = spatial.cast_shape_predicate(
                &slab,
                snap.position + tangent * nose,
                Quat::from_rotation_y(aim_yaw(tangent)),
                direction,
                &config,
                &filter,
                &|e| e != me && !kinematic.contains(&e),
            );
            if let Some(hit) = hit {
                obstacle(
                    nose + hit.distance - half_length,
                    speed_of(hit.entity).dot(tangent),
                );
            }
        }
        accelerations[k] = a;
    }

    // Motion: kinematic cars step along the path, dynamic cars get an autopilot target.
    let rest_wheels = [WheelState {
        compression: vcfg.static_compression(),
        grounded: true,
        force: vcfg.mass / 4.0 * crate::vehicle::GRAVITY,
    }; 4];
    for (k, snap) in snaps.iter_mut().enumerate() {
        if snap.abandon {
            continue;
        }
        let a = accelerations[k];
        let granted = has_grant(&graph, &junctions, snap);
        let Ok((_, _, _, _, mut velocity, mut angular, mut vehicle, _, _, pilot)) =
            cars.get_mut(snap.entity)
        else {
            continue;
        };
        if snap.dynamic {
            let v = snap.car.speed;
            let lookahead = vcfg
                .autopilot
                .lookahead_min
                .max(vcfg.autopilot.lookahead_per_mps * v);
            let next = granted.then_some(snap.car.next).flatten();
            let (seg, s) = ahead(&graph, snap.car.segment, snap.car.s, next, lookahead);
            if let Some(mut pilot) = pilot {
                pilot.target = graph.pose(seg, s).0;
                pilot.speed = follow_speed(&vcfg, v, a, dt);
            }
            continue;
        }
        let car = &mut snap.car;
        let (travel, mut v) = ballistic_step(0.0, car.speed, a, dt);
        let mut s = car.s + travel;
        let mut seg = car.segment;
        for _ in 0..3 {
            let length = graph.length(seg);
            match seg {
                Segment::Lane(_) => {
                    if let (true, Some(c)) = (granted, car.next)
                        && s > length
                    {
                        s -= length;
                        seg = Segment::Connector(c);
                        continue;
                    }
                    // A driver never runs the stop line.
                    if !granted && s > length - half_length {
                        s = (length - half_length).max(car.s);
                        v = 0.0;
                    }
                }
                Segment::Connector(c) if s > length => {
                    s -= length;
                    seg = Segment::Lane(graph.connector(c).to_lane);
                    car.next = None;
                    car.waiting = None;
                    continue;
                }
                Segment::Connector(_) => {}
            }
            break;
        }
        car.segment = seg;
        car.s = s;
        car.speed = v;
        let (point, tangent) = graph.pose(seg, s);
        let target = Vec3::new(point.x, rest, point.z);
        velocity.0 = (target - snap.position) / dt;
        let yaw = aim_yaw(snap.rotation * Vec3::NEG_Z);
        let yaw_rate = wrap(aim_yaw(tangent) - yaw) / dt;
        angular.0 = Vec3::new(0.0, yaw_rate, 0.0);
        let limit = vcfg.steer.max_deg.to_radians();
        // Bicycle model, visual only: yaw rate = v / wheelbase · tan(steer).
        vehicle.steer = (2.0 * vcfg.wheels.half_wheelbase * yaw_rate / v.max(1.0))
            .atan()
            .clamp(-limit, limit);
        vehicle.wheels = rest_wheels;
    }

    // 8. A stopped bailing car: the driver gets out at the first clear exit and runs.
    for snap in &mut snaps {
        let TrafficMode::Bailing { attack, shooter } = snap.car.mode else {
            continue;
        };
        if snap.abandon {
            continue;
        }
        let stands = if snap.dynamic {
            snap.velocity.length() <= vcfg.hold_speed
        } else {
            snap.car.speed == 0.0
        };
        if !stands {
            continue;
        }
        let spots = exit_spots(
            &spatial,
            &vcfg,
            &loco,
            (snap.entity, snap.position, snap.rotation),
            &[],
        );
        let Some((_, spot)) = spots.into_iter().next() else {
            continue;
        };
        let from = shooter
            .and_then(|e| others.get(e).ok())
            .map_or(snap.position, |(p, _)| p.0);
        spawn_driver(
            &mut commands,
            &sidewalks,
            (&loco, &health_cfg, &handle, &civilian),
            &mut rng,
            spot.centre - Vec3::Y * loco.float_height,
            from,
            attack.map(Cause::Attack),
        );
        snap.abandon = true;
    }

    for snap in &mut snaps {
        if snap.abandon {
            abandon(&mut commands, &mut junctions, snap.entity, &mut snap.car);
        }
        if let Ok((_, mut car, ..)) = cars.get_mut(snap.entity) {
            *car = snap.car;
        }
    }
    *stats = counts;
}
