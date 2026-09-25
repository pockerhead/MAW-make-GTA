//! `--bench-scene` (GDD §11/§13 T16): the worst scene of the performance budget, reproducible for a
//! trace. The player boards a parked car near the Downtown centre, 5 stars are pinned, and the car
//! drives a block loop (always the rightmost turn) while traffic, civilians and the police chase fill
//! the frame. QA-only: the game proper never adds this plugin. Its cheats (`pin`, `drive`): 5 stars
//! pinned, the player and the car at full health, the car a ghost on rails at `bench.speed`.

use std::f32::consts::{PI, TAU};

use crate::visuals::RenderConfig;
use avian3d::prelude::{
    AngularVelocity, CollisionLayers, LayerMask, LinearVelocity, Position, RigidBody, Rotation,
};
use bevy::prelude::*;
use gta_sim::{
    character::{ActionIntent, Health, HealthConfig, LocomotionConfig},
    combat::aim_yaw,
    flow::PlayingSystems,
    layers::GameLayer,
    player::Player,
    police::PoliceCar,
    traffic::{Segment, TrafficCar, TrafficGraph, TrafficLane},
    vehicle::{
        DamageConfig, Driving, Vehicle, VehicleConfig, VehicleHealth, VehicleSystems, door_point,
    },
    wanted::{WantedConfig, WantedLevel},
    world::City,
};

/// Seconds the bench may try to board before it reports the scene broken.
const BOARD_GIVE_UP_S: f32 = 5.0;

pub struct BenchScenePlugin;

impl Plugin for BenchScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BenchPhase>()
            .init_resource::<BenchFrames>()
            .register_type::<BenchFrames>()
            .add_systems(
                FixedUpdate,
                (
                    board
                        .run_if(resource_equals(BenchPhase::Board))
                        .before(VehicleSystems::Enter),
                    pin,
                    drive
                        .run_if(resource_equals(BenchPhase::Chase))
                        .after(VehicleSystems::Enter)
                        .before(VehicleSystems::Drive),
                )
                    .in_set(PlayingSystems),
            )
            .add_systems(Last, record_frames);
    }
}

#[derive(Resource, Default, PartialEq, Eq)]
enum BenchPhase {
    #[default]
    Board,
    Chase,
}

/// Real frame times while `recording` (t16 switches it on over BRP for its measuring window).
#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct BenchFrames {
    pub recording: bool,
    pub frame_ms: Vec<f32>,
}

/// Puts the player at the door of the parked car nearest the parking spot closest to the centre that
/// faces it, and asks to get in; retried every tick until the player drives.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn board(
    mut phase: ResMut<BenchPhase>,
    mut waited: Local<f32>,
    mut reported: Local<bool>,
    time: Res<Time<Fixed>>,
    city: Res<City>,
    cfg: Res<VehicleConfig>,
    loco: Res<LocomotionConfig>,
    mut player: Single<
        (
            &mut Position,
            &mut Transform,
            &mut ActionIntent,
            Has<Driving>,
        ),
        With<Player>,
    >,
    cars: Query<
        (&Vehicle, &Position, &Rotation),
        (Without<Player>, Without<TrafficCar>, Without<PoliceCar>),
    >,
) {
    let (position, transform, action, driving) = &mut *player;
    if *driving {
        *phase = BenchPhase::Chase;
        return;
    }
    *waited += time.delta_secs();
    if *waited > BOARD_GIVE_UP_S && !*reported {
        *reported = true;
        error!("bench: could not board a parked car in {BOARD_GIVE_UP_S} s");
    }
    let Some(spot) = city
        .0
        .parking
        .iter()
        .filter(|s| s.heading.dot(-s.position) > 0.0)
        .min_by(|a, b| a.position.length().total_cmp(&b.position.length()))
    else {
        return;
    };
    let at = Vec3::new(spot.position.x, 0.0, spot.position.y);
    let flat = |v: Vec3| v.with_y(0.0);
    let Some((_, car_position, car_rotation)) = cars
        .iter()
        .filter(|(vehicle, ..)| vehicle.driver.is_none())
        .min_by(|a, b| {
            flat(a.1.0 - at)
                .length()
                .total_cmp(&flat(b.1.0 - at).length())
        })
    else {
        return;
    };
    position.0 =
        door_point(cfg.door(), car_position.0, car_rotation.0) + Vec3::Y * loco.float_height;
    transform.translation = position.0;
    action.vehicle_requested = true;
}

/// Bench cheats (bench-only, they keep the §11 worst scene alive): 5 stars with the player in sight,
/// the player and the driven car at full health.
#[allow(clippy::type_complexity)]
fn pin(
    wanted_cfg: Res<WantedConfig>,
    health_cfg: Res<HealthConfig>,
    damage: Res<DamageConfig>,
    mut wanted: ResMut<WantedLevel>,
    mut player: Single<(&Position, &mut Health, Option<&Driving>), With<Player>>,
    mut cars: Query<&mut VehicleHealth>,
) {
    let (position, health, driving) = &mut *player;
    wanted.heat = wanted_cfg.stars[wanted_cfg.stars.len() - 1].heat;
    wanted.hidden = 0.0;
    wanted.last_known = Some(position.0);
    health.current = health_cfg.max_health;
    health.armor = health_cfg.max_armor;
    let Some(driving) = driving else {
        return;
    };
    if let Ok(mut car) = cars.get_mut(driving.vehicle) {
        car.current = damage.vehicle.max_health;
    }
}

/// Bench cheat, a ghost on rails: the driven car leaves physics and follows its lane route
/// kinematically at `bench.speed`, taking the rightmost turn at every junction. It collides with
/// nothing, so traffic and the police cannot box it in, but it stays on the vehicle layer: traffic
/// casts still brake for it and the police still see and chase it.
#[allow(clippy::too_many_arguments)]
fn drive(
    mut commands: Commands,
    render: Res<RenderConfig>,
    cfg: Res<VehicleConfig>,
    graph: Option<Res<TrafficGraph>>,
    time: Res<Time<Fixed>>,
    mut ride: Local<Option<Ride>>,
    player: Single<&Driving, With<Player>>,
    mut cars: Query<(
        &Position,
        &Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
    )>,
) {
    let dt = time.delta_secs();
    let Some(graph) = graph else {
        return;
    };
    let car = player.vehicle;
    let Ok((position, rotation, mut velocity, mut angular)) = cars.get_mut(car) else {
        return;
    };
    if dt <= 0.0 {
        return;
    }
    let forward = (rotation.0 * Vec3::NEG_Z)
        .with_y(0.0)
        .normalize_or(Vec3::NEG_Z);
    let current = match *ride {
        Some(current) => current,
        None => {
            let Some(lane) = current_lane(graph.lanes(), position.0, forward) else {
                return;
            };
            commands.entity(car).try_insert((
                RigidBody::Kinematic,
                CollisionLayers::new(GameLayer::Vehicle, LayerMask::NONE),
            ));
            let segment = Segment::Lane(lane);
            let s = graph
                .project(segment, position.0)
                .clamp(0.0, graph.length(segment));
            Ride { segment, s }
        }
    };
    let next = advance(&graph, current, render.bench().speed * dt);
    *ride = Some(next);
    let (point, tangent) = graph.pose(next.segment, next.s);
    velocity.0 = (point.with_y(cfg.rest_height()) - position.0) / dt;
    let turn = wrap_angle(aim_yaw(tangent) - aim_yaw(forward));
    angular.0 = Vec3::Y * turn / dt;
}

fn record_frames(time: Res<Time<Real>>, mut frames: ResMut<BenchFrames>) {
    if frames.recording {
        frames.frame_ms.push(time.delta_secs() * 1000.0);
    }
}

/// Where the bench car is on its route: `s` m along a lane or a junction connector.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ride {
    segment: Segment,
    s: f32,
}

/// `ride` moved `travel` m on: past a lane end onto its rightmost connector, past a connector end onto
/// the lane it leads to.
fn advance(graph: &TrafficGraph, ride: Ride, travel: f32) -> Ride {
    let Ride { mut segment, mut s } = ride;
    s += travel;
    while s > graph.length(segment) {
        s -= graph.length(segment);
        segment = match segment {
            Segment::Lane(id) => {
                let lane = graph.lane(id);
                let outs = lane
                    .out
                    .iter()
                    .map(|&c| (c, graph.lane(graph.connector(c).to_lane).dir));
                let Some(connector) = rightmost(lane.dir, outs) else {
                    return Ride {
                        segment,
                        s: lane.length,
                    };
                };
                Segment::Connector(connector)
            }
            Segment::Connector(id) => Segment::Lane(graph.connector(id).to_lane),
        };
    }
    Ride { segment, s }
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Id of the lane running along `forward` (within ~45 deg) nearest to `position`.
fn current_lane(lanes: &[TrafficLane], position: Vec3, forward: Vec3) -> Option<u32> {
    let distance = |l: &TrafficLane| {
        let s = (position - l.from).dot(l.dir).clamp(0.0, l.length);
        (l.from + l.dir * s - position).with_y(0.0).length()
    };
    (0..lanes.len())
        .filter(|&k| lanes[k].dir.dot(forward) > 0.7)
        .min_by(|&a, &b| distance(&lanes[a]).total_cmp(&distance(&lanes[b])))
        .map(|k| k as u32)
}

/// Of the connectors `(id, direction of the lane it leads to)`, the one turning most to the right of
/// `dir` (Bevy: forward −Z, right = `dir × Y`); straight on ranks above a left turn.
fn rightmost(dir: Vec3, outs: impl Iterator<Item = (u32, Vec3)>) -> Option<u32> {
    let right = dir.cross(Vec3::Y);
    outs.max_by(|a, b| a.1.dot(right).total_cmp(&b.1.dot(right)))
        .map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gta_sim::{
        config::{ConfigRoot, load_config},
        traffic::{TRAFFIC_CONFIG, TrafficConfig},
    };

    #[test]
    fn rightmost_turn_rows() {
        let (north, east, south, west) = (Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X);
        // Heading north (−Z) the right is +X (east); heading east the right is +Z; heading south −X.
        let four = |dir: Vec3| [(1, dir), (2, dir.cross(Vec3::Y)), (3, -dir.cross(Vec3::Y))];
        for (dir, right) in [(north, east), (east, south), (south, west)] {
            assert_eq!(dir.cross(Vec3::Y), right);
            assert_eq!(rightmost(dir, four(dir).into_iter()), Some(2), "{dir}");
        }
        // A T-junction without a right turn: straight on (dot 0) beats the left (dot −1).
        assert_eq!(
            rightmost(north, [(7, west), (8, north)].into_iter()),
            Some(8)
        );
        assert_eq!(rightmost(north, std::iter::empty()), None);
    }

    /// Lane 0 runs north into a junction with a straight-on connector (id 0, onto lane 2) and a right
    /// turn (id 1, onto lane 1, east); lanes 1 and 2 lead back to lane 0 (the graph has no dead ends).
    fn junction() -> TrafficGraph {
        let root = ConfigRoot(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"));
        let cfg =
            load_config::<TrafficConfig>(&root, TRAFFIC_CONFIG).expect("GATE BROKEN: traffic.ron");
        let lanes = vec![
            (Vec3::ZERO, Vec3::new(0.0, 0.0, -50.0), 12.0, 0),
            (
                Vec3::new(5.0, 0.0, -55.0),
                Vec3::new(55.0, 0.0, -55.0),
                12.0,
                1,
            ),
            (
                Vec3::new(0.0, 0.0, -60.0),
                Vec3::new(0.0, 0.0, -110.0),
                12.0,
                2,
            ),
        ];
        TrafficGraph::new(
            lanes,
            &[(0, 2, 0), (0, 1, 0), (1, 0, 1), (2, 0, 2)],
            &cfg,
            1.0,
        )
        .expect("GATE BROKEN: test junction")
    }

    #[test]
    fn the_ride_takes_the_right_turn_and_the_lane_after_it() {
        let graph = junction();
        let (lane0, turn) = (Segment::Lane(0), Segment::Connector(1));
        assert_eq!(
            advance(
                &graph,
                Ride {
                    segment: lane0,
                    s: 10.0
                },
                5.0
            ),
            Ride {
                segment: lane0,
                s: 15.0
            }
        );
        let into_turn = advance(
            &graph,
            Ride {
                segment: lane0,
                s: 48.0,
            },
            5.0,
        );
        assert_eq!(
            into_turn.segment, turn,
            "the rightmost connector, not straight on"
        );
        assert!((into_turn.s - 3.0).abs() < 1e-4, "{into_turn:?}");
        let length = graph.length(turn);
        assert!(length > 3.0, "GATE BROKEN: connector {length} m");
        let out = advance(
            &graph,
            Ride {
                segment: turn,
                s: length - 1.0,
            },
            3.0,
        );
        assert_eq!(out.segment, Segment::Lane(1));
        assert!((out.s - 2.0).abs() < 1e-4, "{out:?}");
        let at = Vec3::new(0.3, 0.0, -10.0);
        assert_eq!(current_lane(graph.lanes(), at, Vec3::NEG_Z), Some(0));
        assert_eq!(current_lane(graph.lanes(), at, Vec3::X), Some(1));
    }
}
