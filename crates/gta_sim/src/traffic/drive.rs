//! `advance_traffic`: one bounded pass over the AI cars per tick in entity order — path upkeep,
//! intersections, IDM on the nearest obstacle (leader, stop line, forward cast), kinematic motion
//! or autopilot targets, and the driver getting out of a stopped bailing car.

use super::box_rules::shift_pass;
use super::hijack::spawn_driver;
use super::idm::{ballistic_step, idm_acceleration};
use super::junction::{self, has_grant};
use super::lateral::{
    effective_lateral, heading_yaw, manoeuvre_band, offset_pose, step_lateral, target_lateral,
    turn_towards, yaw_cap,
};
use super::manoeuvre::{holds_offset, passing, plan, sense};
use super::pass::pass_done;
use super::recover::{Recovery, recover_dynamic};
use super::sirens::yield_gap;
use super::{
    FlatRect, Manoeuvre, Segment, TrafficCar, TrafficConfig, TrafficGraph, TrafficIntersections,
    TrafficMode, TrafficRng, TrafficStats, abandon,
};
use crate::character::{CharacterControlConfig, HealthConfig, LocomotionConfig};
use crate::civilian::CivilianConfig;
use crate::combat::aim_yaw;
use crate::navigation::SidewalkGraph;
use crate::occupancy::{BodyKind, ClaimFilter, RoadOccupancy};
use crate::perception::Cause;
use crate::vehicle::{
    Autopilot, DriveIntent, Vehicle, VehicleConfig, VehicleHealth, WheelState, exit_spots,
    follow_speed,
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

/// A car on a connector without its grant or the whole box (demoted, recovered there, or switched to
/// another exit): it holds.
fn held_in_box(graph: &TrafficGraph, junctions: &TrafficIntersections, snap: &Snap) -> bool {
    let Segment::Connector(c) = snap.car.segment else {
        return false;
    };
    let node = graph.connector(c).node;
    !junctions.granted(node, c, snap.entity)
        && junctions
            .0
            .get(&node)
            .is_none_or(|j| j.whole.map(|w| w.0) != Some(snap.entity))
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
    mut road: ResMut<RoadOccupancy>,
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
    let mut recovered: Vec<(Entity, f32)> = Vec::new();
    for snap in &mut snaps {
        if snap.dynamic {
            let forward = snap.rotation * Vec3::NEG_Z;
            snap.car.speed = snap.velocity.dot(forward).max(0.0);
            snap.abandon = reproject(&graph, &junctions, snap, &cfg);
        }
        if snap.dynamic && !snap.abandon && snap.car.mode == TrafficMode::Dynamic {
            match recover_dynamic(snap, &road, &graph, &cfg, &vcfg, dt) {
                Recovery::Stay => {}
                Recovery::Recover { lateral } => recovered.push((snap.entity, lateral)),
                // A car with no door free is given up at once: bailing, it would stand forever.
                Recovery::GiveUp => {
                    let car = (snap.entity, snap.position, snap.rotation);
                    if exit_spots(&spatial, &vcfg, &loco, car, &[]).is_empty() {
                        snap.abandon = true;
                    } else {
                        snap.car.mode = TrafficMode::Bailing {
                            attack: None,
                            shooter: None,
                        };
                    }
                }
            }
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
    // Bodies outside the path occupancy (not AI cars on their path, not dynamic AI cars: `room` counts
    // those) or a pass claim on the first `length` m of `lane`.
    let lane_start_free = |lane: u32, length: f32| {
        let l = graph.lane(lane);
        let length = length.min(l.length);
        let centre = l.from + l.dir * (length / 2.0);
        let rect = FlatRect::of(
            centre,
            Quat::from_rotation_y(aim_yaw(l.dir)),
            Vec2::new(half.x, length / 2.0),
        );
        let skip = |b: &crate::occupancy::RoadBody| {
            b.kind == BodyKind::Character
                || b.kind == BodyKind::OnPathTraffic
                || (b.kind == BodyKind::Traffic && b.dynamic)
        };
        road.blocked(&rect, skip, ClaimFilter::All).is_none()
    };
    let lease_ticks = (cfg.reservation_timeout / dt).ceil() as u64;
    junction::update(
        &graph,
        &mut junctions,
        &mut snaps,
        &occupied,
        idm,
        half_length,
        tick,
        (lease_ticks, vcfg.hold_speed),
        &mut rng,
        &lane_start_free,
        &junction::BoxInputs {
            road: &road,
            body: Vec2::new(half.x, half.z),
            stuck_seconds: cfg.pass.vehicle_seconds,
        },
    );

    // 5. Obstacles and 6. acceleration.
    let mut accelerations = vec![0.0; snaps.len()];
    let held_cars: HashSet<Entity> = snaps
        .iter()
        .filter(|s| !s.abandon && held_in_box(&graph, &junctions, s))
        .map(|s| s.entity)
        .collect();
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
        let v0 = match car.manoeuvre {
            Manoeuvre::Pass { .. } | Manoeuvre::Yield { .. } => v0.min(cfg.pass.speed),
            _ => v0,
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
            obstacle(graph.lane(l).stop - (car.s + half_length), 0.0);
        }
        match car.manoeuvre {
            // Out of the way first (at most at the manoeuvre speed), then a stop.
            Manoeuvre::Yield { offset, .. } if (car.lateral - offset).abs() < 0.05 => {
                obstacle(yield_gap(v, idm), 0.0)
            }
            // Behind the committed obstacle until the car is out beside it.
            Manoeuvre::Pass { need, hold_s, .. } if car.lateral.abs() < need && car.s <= hold_s => {
                obstacle(hold_s - car.s, 0.0)
            }
            _ => {}
        }
        stats.casts = stats.casts.wrapping_add(1);
        counts.casts = stats.casts;
        let (ahead, beside) = sense(&graph, &road, &cfg, snap, half, &held_cars);
        for hit in ahead.iter().chain(beside.iter()) {
            obstacle(hit.gap, hit.speed_along);
        }
        let update = plan(
            (&road, &spatial, &graph, &junctions),
            snap,
            ahead,
            (tick, dt),
            (&cfg, &vcfg),
        );
        accelerations[k] = a;
        let Some(update) = update else {
            continue;
        };
        snaps[k].car.manoeuvre = update.manoeuvre;
        snaps[k].car.deaf = snaps[k].car.deaf.max(update.deaf);
        road.claims_extend(update.claim);
        if let Some(c) = update.whole_box {
            let conn = graph.connector(c);
            let me = snaps[k].entity;
            let junction = junctions.0.entry(conn.node).or_default();
            junction.whole = Some((me, conn.to_lane));
            // The whole box is held through the car's own grant (a demoted car has none): without it
            // the lease finds no move of the car and lapses the next tick.
            junction.waiters.retain(|w| w.1 != me);
            if !junction.occupants.contains(&(c, me)) {
                junction.occupants.push((c, me));
            }
            junction.moved.insert(me, tick);
            snaps[k].car.waiting = None;
        }
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
        let held = held_in_box(&graph, &junctions, snap);
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
            let side = match seg {
                Segment::Lane(_) => target_lateral(&snap.car, snap.car.s - half_length),
                Segment::Connector(_) => 0.0,
            };
            if let Some(mut pilot) = pilot {
                pilot.target = offset_pose(&graph, seg, s, side).0;
                pilot.speed = if held {
                    0.0
                } else {
                    follow_speed(&vcfg, v, a, dt)
                };
            }
            continue;
        }
        let car = &mut snap.car;
        car.deaf = (car.deaf - dt).max(0.0);
        let before = effective_lateral(&graph, car.segment, car.s, car.lateral, holds_offset(car));
        let (travel, mut v) = if held {
            (0.0, 0.0)
        } else {
            ballistic_step(0.0, car.speed, a, dt)
        };
        let mut s = car.s + travel;
        let mut seg = car.segment;
        for _ in 0..3 {
            let length = graph.length(seg);
            match seg {
                Segment::Lane(l) => {
                    if let (true, Some(c)) = (granted, car.next)
                        && s > length
                    {
                        s -= length;
                        seg = Segment::Connector(c);
                        shift_pass(&mut car.manoeuvre, -length);
                        continue;
                    }
                    // A driver never runs the stop line.
                    let stop = graph.lane(l).stop - half_length;
                    if !granted && s > stop {
                        s = stop.max(car.s);
                        v = 0.0;
                    }
                }
                Segment::Connector(c) if s > length => {
                    s -= length;
                    seg = Segment::Lane(graph.connector(c).to_lane);
                    car.next = None;
                    car.waiting = None;
                    // A pass in the box goes on along the exit lane (its marks move with the
                    // origin), so does a rejoin still offset; any other offset has decayed to 0.
                    if passing(car) {
                        shift_pass(&mut car.manoeuvre, -length);
                    } else if car.manoeuvre != Manoeuvre::Rejoin {
                        car.lateral = 0.0;
                    }
                    continue;
                }
                Segment::Connector(_) => {}
            }
            break;
        }
        if let Manoeuvre::Pass { need, hold_s, .. } = car.manoeuvre
            && car.lateral.abs() < need
            && car.s <= hold_s
            && s > hold_s
        {
            s = hold_s.max(car.s);
            v = 0.0;
        }
        car.segment = seg;
        car.s = s;
        car.speed = v;
        let manoeuvring = car.lateral != 0.0 || car.manoeuvre != Manoeuvre::None;
        if manoeuvring && (matches!(seg, Segment::Lane(_)) || holds_offset(car)) {
            let target = target_lateral(car, s - half_length);
            car.lateral = step_lateral(car.lateral, target, v, dt, &cfg.lateral);
        }
        let lateral = effective_lateral(&graph, seg, s, car.lateral, holds_offset(car));
        let (point, tangent) = offset_pose(&graph, seg, s, lateral);
        let target = Vec3::new(point.x, rest, point.z);
        velocity.0 = (target - snap.position) / dt;
        let yaw = aim_yaw(snap.rotation * Vec3::NEG_Z);
        let path_error = wrap(aim_yaw(tangent) - yaw);
        // Off the path line the heading follows the sideways move at a limited turn rate; on it the
        // yaw snaps to the path in one tick. A rejoin in the box slides: a yaw swing takes the body
        // out of the conflict table's band.
        let slide = matches!(seg, Segment::Connector(_)) && car.manoeuvre == Manoeuvre::Rejoin;
        angular.0 = if manoeuvring {
            let rate = if slide { 0.0 } else { (lateral - before) / dt };
            let mut heading = heading_yaw(tangent, v, rate);
            // On a lane the swing is capped to the manoeuvre's band, at the offset now and where it
            // will be by the time the yaw can turn back (the turn rate lags the cap).
            if let Segment::Lane(l) = seg {
                let pitch = graph.lane(l).left_gap.unwrap_or(2.0 * half.x);
                let band = manoeuvre_band(car.manoeuvre, pitch, half.x, cfg.pass.clearance);
                let turn_back = path_error.abs() / cfg.lateral.yaw_rate_deg.to_radians();
                let target = target_lateral(car, s - half_length);
                let ahead = step_lateral(lateral, target, v, turn_back, &cfg.lateral);
                let cap = yaw_cap(band, (lateral, ahead), Vec2::new(half.x, half.z));
                let aim = aim_yaw(tangent);
                heading = aim + wrap(heading - aim).clamp(-cap, cap);
            }
            turn_towards(snap.rotation, heading, dt, &cfg.lateral)
        } else {
            Vec3::new(0.0, path_error / dt, 0.0)
        };
        if pass_done(car, half_length) {
            car.manoeuvre = Manoeuvre::None;
        }
        if car.manoeuvre == Manoeuvre::Rejoin
            && car.lateral.abs() < 0.01
            && path_error.abs() < 1f32.to_radians()
        {
            car.lateral = 0.0;
            car.manoeuvre = Manoeuvre::None;
        }
        let yaw_rate = angular.0.y;
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
        if let Some(&(_, lateral)) = recovered.iter().find(|r| r.0 == snap.entity)
            && !snap.abandon
        {
            commands
                .entity(snap.entity)
                .try_insert(RigidBody::Kinematic);
            commands
                .entity(snap.entity)
                .try_remove::<(Autopilot, DriveIntent, SleepingDisabled)>();
            let car = &mut snap.car;
            car.mode = TrafficMode::Kinematic;
            car.lateral = lateral;
            car.manoeuvre = Manoeuvre::Rejoin;
            car.calm = 0.0;
            car.stood = 0.0;
            car.speed = 0.0;
        }
        if let Ok((_, mut car, ..)) = cars.get_mut(snap.entity) {
            *car = snap.car;
        }
    }
    *stats = counts;
}
