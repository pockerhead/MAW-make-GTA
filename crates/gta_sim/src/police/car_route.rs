//! Police car driving (GDD §5.3): A* over the traffic lanes to the lane nearest the target, the car
//! state table each tick, the crew getting out, the re-boarding timeout.

use super::cars::{
    CarSenses, CrewOf, PoliceCar, PoliceCarRng, PoliceCarRoute, PoliceCarState, crews_outside,
    dismount, next_car_state,
};
use super::{EscalationConfig, PoliceUnit};
use crate::character::{CharacterControlConfig, Dead, HealthConfig, LocomotionConfig};
use crate::combat::{WeaponsConfig, aim_yaw};
use crate::layers::GameLayer;
use crate::navigation::flat_distance;
use crate::player::Player;
use crate::traffic::{Segment, TrafficConfig, TrafficGraph, idm::idm_acceleration};
use crate::vehicle::{Autopilot, Driving, VehicleConfig, follow_speed};
use crate::wanted::{WantedConfig, WantedLevel, cop_sees, eye};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Integer centimetres: `astar` needs an `Ord` cost.
fn cm(metres: f32) -> u32 {
    (metres.max(0.0) * 100.0) as u32
}

/// Flat distance from `p` to the nearest point of `lane` at or past `from_s` m along it.
fn lane_distance(graph: &TrafficGraph, lane: u32, p: Vec3, from_s: f32) -> f32 {
    let l = graph.lane(lane);
    let s = (p - l.from)
        .dot(l.dir)
        .clamp(from_s.min(l.length), l.length);
    flat_distance(p, l.from + l.dir * s)
}

/// Lanes from `from` (the car `from_s` m along it) to the first lane that passes within `radius` of
/// `goal_point` (A* over lane ends; cost = connector + next lane, heuristic = straight line from the
/// lane end to `goal_point`): a car need not loop a block to reach the one nearest lane when another
/// one passes close enough. On `from` only the stretch ahead of the car counts. `None` when
/// unreachable.
pub fn find_lane_route(
    graph: &TrafficGraph,
    from: u32,
    from_s: f32,
    goal_point: Vec3,
    radius: f32,
) -> Option<Vec<u32>> {
    pathfinding::prelude::astar(
        &from,
        |&l| {
            graph.lane(l).out.iter().map(|&c| {
                let conn = graph.connector(c);
                (
                    conn.to_lane,
                    cm(conn.length + graph.lane(conn.to_lane).length),
                )
            })
        },
        |&l| cm(flat_distance(graph.lane(l).to, goal_point) - radius),
        |&l| {
            let behind = if l == from { from_s } else { 0.0 };
            lane_distance(graph, l, goal_point, behind) <= radius
        },
    )
    .map(|(path, _)| path)
}

/// Cost of a lane step in `find_lane_route`: the connector and the lane it leads to.
fn step_cost(graph: &TrafficGraph, connector: u32) -> u32 {
    let conn = graph.connector(connector);
    cm(conn.length + graph.lane(conn.to_lane).length)
}

/// Route cost from every lane to the goal of `find_lane_route` (one reverse Dijkstra over the lane
/// graph, same costs): 0 on a lane passing within `radius` of `goal_point`, `u32::MAX` if unreachable.
pub(super) fn lane_costs_to(graph: &TrafficGraph, goal_point: Vec3, radius: f32) -> Vec<u32> {
    let n = graph.lanes().len();
    let mut cost = vec![u32::MAX; n];
    let mut incoming: Vec<Vec<(u32, u32)>> = vec![Vec::new(); n];
    for (k, conn) in graph.connectors().iter().enumerate() {
        incoming[conn.to_lane as usize].push((conn.from_lane, step_cost(graph, k as u32)));
    }
    let mut heap = BinaryHeap::new();
    for lane in 0..n as u32 {
        if lane_distance(graph, lane, goal_point, 0.0) <= radius {
            cost[lane as usize] = 0;
            heap.push(Reverse((0u32, lane)));
        }
    }
    while let Some(Reverse((at, lane))) = heap.pop() {
        if at > cost[lane as usize] {
            continue;
        }
        for &(from, step) in &incoming[lane as usize] {
            let via = at.saturating_add(step);
            if via < cost[from as usize] {
                cost[from as usize] = via;
                heap.push(Reverse((via, from)));
            }
        }
    }
    cost
}

/// Whether no AI traffic car (`traffic`: segment and s) stands ahead of a car `s` m along `lane` on its
/// cheapest lane route (`costs` from `lane_costs_to`) to `goal_point`, up to the goal's projection on
/// the last lane. Police cars cannot pass traffic: one spawned behind a queue never closes in.
pub(super) fn approach_clear(
    graph: &TrafficGraph,
    costs: &[u32],
    traffic: &[(Segment, f32)],
    (lane, s): (u32, f32),
    goal_point: Vec3,
    radius: f32,
) -> bool {
    let queued = |seg: Segment, from: f32, to: f32| {
        traffic
            .iter()
            .any(|&(at, ts)| at == seg && (from..=to).contains(&ts))
    };
    let (mut lane, mut from) = (lane, s);
    for _ in 0..graph.lanes().len() {
        let l = graph.lane(lane);
        if lane_distance(graph, lane, goal_point, from) <= radius {
            let goal_s = (goal_point - l.from).dot(l.dir).clamp(from, l.length);
            return !queued(Segment::Lane(lane), from, goal_s);
        }
        if queued(Segment::Lane(lane), from, f32::INFINITY) {
            return false;
        }
        let next = l.out.iter().copied().min_by_key(|&c| {
            costs[graph.connector(c).to_lane as usize].saturating_add(step_cost(graph, c))
        });
        let Some(c) = next else {
            return false;
        };
        let to = graph.connector(c).to_lane;
        if costs[to as usize] == u32::MAX || queued(Segment::Connector(c), 0.0, f32::INFINITY) {
            return false;
        }
        (lane, from) = (to, 0.0);
    }
    false
}

/// The lane under a car at `position` heading along `forward`: the nearest lane not pointing against it.
fn start_lane(graph: &TrafficGraph, position: Vec3, forward: Vec3) -> Option<u32> {
    graph
        .lanes()
        .iter()
        .enumerate()
        .filter(|(_, lane)| lane.dir.dot(forward) > 0.0)
        .map(|(k, lane)| {
            let s = (position - lane.from).dot(lane.dir).clamp(0.0, lane.length);
            (k as u32, flat_distance(position, lane.from + lane.dir * s))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k)
}

fn connector_between(graph: &TrafficGraph, a: u32, b: u32) -> Option<u32> {
    graph
        .lane(a)
        .out
        .iter()
        .copied()
        .find(|&c| graph.connector(c).to_lane == b)
}

/// Where a car on its route steers to and how far it still has to go.
struct Walk {
    target: Vec3,
    remaining: f32,
    turning: bool,
    /// Path length to the next connector (the next turn), if one follows.
    to_turn: Option<f32>,
}

/// Follows the route from the car at `position`: advances `route.next` past a lane the car has left,
/// the target `lookahead` m ahead, the path left to `end` m along the last lane.
fn follow(
    graph: &TrafficGraph,
    route: &mut PoliceCarRoute,
    position: Vec3,
    end: f32,
    lookahead: f32,
) -> Option<Walk> {
    let build = |lanes: &[u32]| -> Vec<Segment> {
        let mut segs = Vec::new();
        for (k, &lane) in lanes.iter().enumerate() {
            segs.push(Segment::Lane(lane));
            let Some(&next) = lanes.get(k + 1) else {
                break;
            };
            let Some(c) = connector_between(graph, lane, next) else {
                break;
            };
            segs.push(Segment::Connector(c));
        }
        segs
    };
    let mut segs = build(route.lanes.get(route.next..).unwrap_or_default());
    if segs.is_empty() {
        return None;
    }
    let length = |segs: &[Segment], k: usize| {
        if k + 1 == segs.len() {
            end.min(graph.length(segs[k]))
        } else {
            graph.length(segs[k])
        }
    };
    let place = |seg: Segment| {
        let s = graph.project(seg, position).clamp(0.0, graph.length(seg));
        (s, flat_distance(position, graph.pose(seg, s).0))
    };
    let (mut index, mut s) = (0, place(segs[0]).0);
    let mut best = f32::INFINITY;
    for (k, &seg) in segs.iter().enumerate().take(3) {
        let (sk, d) = place(seg);
        if d < best - 1e-3 {
            (index, s, best) = (k, sk, d);
        }
    }
    if index == 2 {
        route.next += 1;
        segs = build(route.lanes.get(route.next..).unwrap_or_default());
        index = 0;
    }
    let mut remaining = length(&segs, index) - s;
    for k in index + 1..segs.len() {
        remaining += length(&segs, k);
    }
    let (mut k, mut at) = (index, s + lookahead);
    while at > length(&segs, k) && k + 1 < segs.len() {
        at -= length(&segs, k);
        k += 1;
    }
    let at = at.min(length(&segs, k));
    let to_turn = match (segs[index], segs.get(index + 1)) {
        (Segment::Lane(_), Some(Segment::Connector(_))) => Some(length(&segs, index) - s),
        _ => None,
    };
    Some(Walk {
        target: graph.pose(segs[k], at).0,
        // Past the goal point (it lies behind the car on its last lane): arrived.
        remaining: remaining.max(0.0),
        turning: matches!(segs[index], Segment::Connector(_)),
        to_turn,
    })
}

/// The offset to the curb side for a car stopping from speed `v` at `brake`: `offset` to its right when
/// a chassis there at the stop point is free of cars, walls and the raised sidewalk.
#[allow(clippy::too_many_arguments)]
fn pull_over(
    spatial: &SpatialQuery,
    vehicle: &VehicleConfig,
    offset: f32,
    brake: f32,
    entity: Entity,
    position: Vec3,
    rotation: Quat,
    v: f32,
) -> Option<Vec3> {
    if offset <= 0.0 {
        return None;
    }
    let half = vehicle.half_extents();
    let forward = (rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let side = forward.cross(Vec3::Y) * offset;
    let stop = v.max(0.0).powi(2) / (2.0 * brake);
    let top = vehicle.rest_height() + half.y;
    let low = 0.05;
    let centre = (position + forward * stop + side).with_y((top + low) / 2.0);
    let chassis = Collider::cuboid(2.0 * half.x, top - low, 2.0 * half.z);
    let filter = SpatialQueryFilter::from_mask([GameLayer::World, GameLayer::Vehicle])
        .with_excluded_entities([entity]);
    let yaw = Quat::from_rotation_y(aim_yaw(forward));
    spatial
        .shape_intersections(&chassis, centre, yaw, &filter)
        .is_empty()
        .then_some(side)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn drive_police_cars(
    mut commands: Commands,
    spatial: SpatialQuery,
    configs: (
        Res<EscalationConfig>,
        Res<WantedConfig>,
        Res<VehicleConfig>,
        Res<TrafficConfig>,
        Res<LocomotionConfig>,
    ),
    crew_cfg: (
        Res<HealthConfig>,
        Res<CharacterControlConfig>,
        Res<WeaponsConfig>,
    ),
    state: (Res<TrafficGraph>, Res<WantedLevel>, Res<Time<Fixed>>),
    mut rng: ResMut<PoliceCarRng>,
    player: Query<(Entity, &Position, Option<&Driving>, Has<Dead>), With<Player>>,
    speeds: Query<&LinearVelocity>,
    mut cars: Query<(
        Entity,
        &mut PoliceCar,
        &mut PoliceCarRoute,
        &mut Autopilot,
        &Position,
        &Rotation,
    )>,
    crews: Query<(Entity, &CrewOf, &PoliceUnit)>,
) {
    let (esc, wanted_cfg, vehicle, traffic, loco) = configs;
    let (health, handle, weapons) = crew_cfg;
    let (graph, wanted, time) = state;
    let dt = time.delta_secs();
    let c = &esc.car;
    let half = vehicle.half_extents();
    let outside = crews_outside(crews.iter().map(|(_, crew_of, unit)| (crew_of, unit)));
    let player = player.single().ok();
    let driving = player.and_then(|p| p.2.map(|d| d.vehicle));
    let player_car_speed = driving
        .and_then(|car| speeds.get(car).ok())
        .map_or(0.0, |v| v.0.length());
    let live = player.filter(|p| !p.3).map(|p| p.1.0);
    let mut order: Vec<Entity> = cars.iter().map(|c| c.0).collect();
    order.sort_by_key(|e| e.to_bits());
    let mut searches = 0;
    let slab = Collider::cuboid(2.0 * half.x, 2.0 * half.y, 0.1);
    for entity in order {
        let Ok((_, mut car, mut route, mut pilot, position, rotation)) = cars.get_mut(entity)
        else {
            continue;
        };
        let (position, rotation) = (position.0, rotation.0);
        let forward = rotation * Vec3::NEG_Z;
        let v = speeds.get(entity).map_or(0.0, |v| v.0.dot(forward));
        route.age += dt;
        let at = live.unwrap_or(position);
        let distance = flat_distance(position, at);
        let sees = !car.crew.is_empty()
            && live.is_some_and(|p| {
                cop_sees(
                    &spatial,
                    position + rotation * vehicle.seat(),
                    forward,
                    eye(p, &loco),
                    &wanted_cfg,
                    wanted_cfg.cop_car_view_distance,
                )
            });
        car.blocked = if car.state == PoliceCarState::Respond && v.abs() <= vehicle.exit_max_speed {
            car.blocked + dt
        } else {
            0.0
        };
        car.stopped = if driving.is_some() && player_car_speed <= vehicle.exit_max_speed {
            car.stopped + dt
        } else {
            0.0
        };
        car.moving = if driving.is_some() && player_car_speed > vehicle.exit_max_speed {
            car.moving + dt
        } else {
            0.0
        };
        let mut crew_outside = outside.get(&entity).copied().unwrap_or(0);
        if car.state == PoliceCarState::Dismounted {
            if car.driven_off(c, driving.is_some(), distance) {
                car.reboard_left = (car.reboard_left - dt).max(0.0);
            } else {
                car.reboard_left = c.reboard_timeout_seconds;
            }
            if car.reboard_left <= 0.0 {
                for (cop, crew_of, _) in &crews {
                    if crew_of.car == entity {
                        commands.entity(cop).try_remove::<CrewOf>();
                    }
                }
                crew_outside = 0;
            }
        }
        // Where the route leads: the player while seen, else the last known position.
        let goal_point = if sees { live } else { wanted.last_known }.unwrap_or(at);
        let lookahead = vehicle
            .autopilot
            .lookahead_min
            .max(vehicle.autopilot.lookahead_per_mps * v.max(0.0));
        // A car never stops in an intersection: it neither parks nor lets its crew out there.
        let in_box = graph.in_junction(position, half.z);
        let mut no_route = false;
        let walk = match (car.state, graph.nearest(goal_point)) {
            (PoliceCarState::Respond, Some((_, _, nearest))) => {
                // Any lane passing within the margin of the nearest one will do: the far side of the
                // target's street counts.
                let radius = nearest + c.goal_margin;
                let stale = route.lanes.is_empty() || route.age >= c.route_refresh_seconds;
                if stale && searches < c.routes_per_tick {
                    searches += 1;
                    let from = route
                        .lanes
                        .get(route.next)
                        .copied()
                        .filter(|&l| {
                            let lane = graph.lane(l);
                            let s = (position - lane.from).dot(lane.dir).clamp(0.0, lane.length);
                            flat_distance(position, lane.from + lane.dir * s)
                                <= traffic.lost.distance
                        })
                        .or_else(|| start_lane(&graph, position, forward));
                    let lanes = from
                        .and_then(|f| {
                            let s = graph.project(Segment::Lane(f), position);
                            find_lane_route(&graph, f, s, goal_point, radius)
                        })
                        .unwrap_or_default();
                    no_route = lanes.is_empty();
                    *route = PoliceCarRoute {
                        goal: lanes.last().copied(),
                        lanes,
                        next: 0,
                        age: 0.0,
                    };
                }
                // The route ends with the whole car on its last lane, clear of both intersections.
                let end = route.goal.map_or(0.0, |g| {
                    let length = graph.lane(g).length;
                    let clear = (half.z + traffic.idm.min_gap).min(length / 2.0);
                    graph
                        .project(Segment::Lane(g), goal_point)
                        .clamp(clear, length - clear)
                });
                follow(&graph, &mut route, position, end, lookahead)
            }
            _ => None,
        };
        let senses = CarSenses {
            stars: wanted.stars,
            driving: driving.is_some(),
            sees,
            distance,
            speed: v.abs(),
            player_stopped: car.stopped,
            crew_aboard: car.crew.len() as u32,
            crew_outside,
            reboard_left: car.reboard_left,
            // Near the goal: a car held up behind another there (a parked police car) gets out too.
            route_done: walk
                .as_ref()
                .is_some_and(|w| w.remaining <= c.dismount_distance),
            blocked: car.blocked >= c.blocked_seconds,
            no_route,
            in_junction: in_box,
            stuck_in_junction: car.blocked >= c.blocked_seconds * c.junction_factor,
            driven_off: car.driven_off(c, driving.is_some(), distance),
        };
        let before = car.state;
        car.state = next_car_state(before, &senses, c, vehicle.exit_max_speed);
        if car.state == PoliceCarState::Dismounted && before != PoliceCarState::Dismounted {
            let mut crew = std::mem::take(&mut car.crew);
            let aboard = crew.len();
            dismount(
                &mut commands,
                &spatial,
                (&esc, &vehicle, &loco, &health, &handle, &weapons),
                &mut rng,
                (entity, position, rotation),
                &mut crew,
            );
            if aboard > 0 && crew.len() == aboard {
                // No free door: the crew waits aboard (asked again next tick), the car stays on the job.
                car.state = before;
            } else {
                car.reboard_left = c.reboard_timeout_seconds;
            }
            car.crew = crew;
        } else if car.state == PoliceCarState::Dismounted
            && !car.crew.is_empty()
            && !car.driven_off(c, driving.is_some(), distance)
        {
            // A cop left aboard (no clear door) gets out once a door is free.
            let mut crew = std::mem::take(&mut car.crew);
            dismount(
                &mut commands,
                &spatial,
                (&esc, &vehicle, &loco, &health, &handle, &weapons),
                &mut rng,
                (entity, position, rotation),
                &mut crew,
            );
            car.crew = crew;
        }
        if before == PoliceCarState::Dismounted && car.state == PoliceCarState::Respond {
            *route = PoliceCarRoute::default();
        }
        let turning = walk.as_ref().is_some_and(|w| w.turning);
        let brake = traffic.idm.max_deceleration;
        let (target, mut speed, ignore) = match car.state {
            PoliceCarState::Respond => {
                let hold = distance <= c.dismount_distance
                    && (driving.is_none() || car.stopped >= c.stopped_seconds);
                let (target, limit) = match &walk {
                    Some(w) => (
                        w.target,
                        // Police brake hard: slow to `turn_speed` by the next turn and to 0 at the goal.
                        if w.turning {
                            c.turn_speed
                        } else {
                            c.pursuit_speed
                        }
                        .min((2.0 * brake * w.remaining).sqrt())
                        .min(w.to_turn.map_or(f32::INFINITY, |d| {
                            (c.turn_speed * c.turn_speed + 2.0 * brake * d.max(0.0)).sqrt()
                        })),
                    ),
                    // No route (yet): wait where it is, never straight through the blocks.
                    None => (position + forward * lookahead, 0.0),
                };
                let speed = if hold { 0.0 } else { limit };
                if in_box {
                    // Through the intersection at turn speed before any stop.
                    (target, speed.max(c.turn_speed), None)
                } else if speed < c.turn_speed
                    && let Some(side) = pull_over(
                        &spatial,
                        &vehicle,
                        c.pull_over,
                        brake,
                        entity,
                        position,
                        rotation,
                        v,
                    )
                {
                    (target + side, speed, None)
                } else {
                    (target, speed, None)
                }
            }
            PoliceCarState::Chase => (at, c.pursuit_speed, driving),
            PoliceCarState::Leave => {
                if route.lanes.is_empty() {
                    route.lanes.extend(start_lane(&graph, position, forward));
                    route.next = 0;
                }
                // Wander: the route grows by a random connector at the end of its last lane.
                if let Some(&last) = route.lanes.last()
                    && route.lanes.len() < route.next + 3
                {
                    let out = &graph.lane(last).out;
                    let c = out[rng.next_u32() as usize % out.len()];
                    route.lanes.push(graph.connector(c).to_lane);
                }
                let end = route.lanes.last().map_or(0.0, |&l| graph.lane(l).length);
                match follow(&graph, &mut route, position, end, lookahead) {
                    Some(w) => {
                        let v0 = route
                            .lanes
                            .get(route.next)
                            .map_or(c.turn_speed, |&l| graph.lane(l).v0);
                        (w.target, if w.turning { c.turn_speed } else { v0 }, None)
                    }
                    None => (position + forward * lookahead, 0.0, None),
                }
            }
            _ => (position + forward * lookahead, 0.0, None),
        };
        // IDM on whatever stands ahead (the player's car is rammed in a chase).
        if speed > 0.0
            && let Ok(direction) = Dir3::new(forward.with_y(0.0))
        {
            let filter = SpatialQueryFilter::from_mask([GameLayer::Character, GameLayer::Vehicle]);
            // Turning, a straight cast would see the cars waiting at the other stop lines.
            let sense = if turning {
                traffic.turn_sense_distance
            } else {
                traffic.sense_distance
            };
            let config = ShapeCastConfig::from_max_distance(half.z + sense);
            let hit = spatial.cast_shape_predicate(
                &slab,
                position,
                Quat::from_rotation_y(aim_yaw(forward)),
                direction,
                &config,
                &filter,
                &|e| e != entity && Some(e) != ignore,
            );
            if let Some(hit) = hit {
                let other = speeds.get(hit.entity).map_or(0.0, |o| o.0.dot(forward));
                let gap = hit.distance - half.z;
                let a = idm_acceleration(v.max(0.0), speed, Some((gap, v - other)), &traffic.idm);
                speed = speed.min(follow_speed(&vehicle, v.max(0.0), a, dt));
            }
        }
        pilot.target = target;
        pilot.speed = speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigRoot, load_config};
    use crate::traffic::TRAFFIC_CONFIG;

    /// A square loop of side 80 (lanes 0..4 clockwise from the north side, heading +X, -Z, -X, +Z).
    fn square() -> TrafficGraph {
        let root = ConfigRoot(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into());
        let cfg: TrafficConfig =
            load_config(&root, TRAFFIC_CONFIG).expect("GATE BROKEN: traffic.ron");
        let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
        let lanes = vec![
            (at(-37.0, 40.0), at(37.0, 40.0), 12.0, 0),
            (at(40.0, 37.0), at(40.0, -37.0), 12.0, 1),
            (at(37.0, -40.0), at(-37.0, -40.0), 12.0, 2),
            (at(-40.0, -37.0), at(-40.0, 37.0), 12.0, 3),
        ];
        TrafficGraph::new(
            lanes,
            &[(0, 1, 0), (1, 2, 1), (2, 3, 2), (3, 0, 3)],
            &cfg,
            1.0,
        )
        .expect("GATE BROKEN: square graph")
    }

    /// Start 10 m along lane 0, the goal by the middle of lane 2 (x = 0): the route runs lane 0,
    /// connector 0, lane 1, connector 1, lane 2 up to s = 37.
    #[test]
    fn approach_clear_rows() {
        let graph = square();
        let goal = Vec3::new(0.0, 0.0, -44.0);
        let radius = 4.0 + 10.0;
        let costs = lane_costs_to(&graph, goal, radius);
        assert_eq!(costs[2], 0);
        assert!(costs[1] < costs[0] && costs[0] < costs[3], "{costs:?}");
        let c0 = graph.lane(0).out[0];
        let rows: [(&[(Segment, f32)], bool); 7] = [
            (&[], true),
            (&[(Segment::Lane(0), 5.0)], true),
            (&[(Segment::Lane(0), 30.0)], false),
            (&[(Segment::Connector(c0), 1.0)], false),
            (&[(Segment::Lane(1), 60.0)], false),
            (&[(Segment::Lane(2), 20.0)], false),
            (&[(Segment::Lane(2), 50.0), (Segment::Lane(3), 5.0)], true),
        ];
        for (traffic, clear) in rows {
            assert_eq!(
                approach_clear(&graph, &costs, traffic, (0, 10.0), goal, radius),
                clear,
                "traffic {traffic:?}"
            );
        }
    }
}
