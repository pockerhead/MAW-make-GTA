//! Traffic (GDD §5.2, slice T15): kinematic cars on the inner lanes with IDM, intersection
//! reservations, the Vermeij bubble, a time-to-contact switch to a dynamic body, hijacking and the
//! driver bailing out of a shot-at car.

mod bail;
mod config;
mod contact;
mod drive;
mod graph;
mod hijack;
pub mod idm;
mod junction;
mod spawn;

pub use config::{
    Band, BubbleConfig, DesiredSpeed, IdmConfig, LostConfig, SwitchConfig, TRAFFIC_CONFIG,
    TrafficConfig,
};
pub use contact::{FlatRect, swept_circle_hits_rect, swept_rect_hits_rect};
pub use graph::{Segment, TrafficConnector, TrafficGraph, TrafficLane, control_point, lane_slot};
pub use spawn::{in_frame, spawn_traffic_car};

use crate::combat::unit_f32;
use crate::flow::{GameState, NEW_CITY, NpcSystems};
use crate::population::Offscreen;
use crate::vehicle::{Autopilot, DriveIntent, VehicleSystems};
use crate::wanted::WantedSystems;
use crate::world::{City, CitySeed};
use avian3d::prelude::*;
use bevy::prelude::*;
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};
use std::collections::HashMap;

/// What drives a traffic car.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficMode {
    /// AI on the lane path, moved by velocity (the data driver is aboard).
    Kinematic,
    /// AI on the lane path through the autopilot after a contact.
    Dynamic,
    /// The driver brakes to a stop and gets out: scared by a cabin shot (`attack` by `shooter`) or the
    /// car is wrecked (both `None`).
    Bailing {
        attack: Option<u32>,
        shooter: Option<Entity>,
    },
    /// The player sits in it: never despawned, not counted in the cap.
    Taken,
    /// No AI: a dynamic body under the bubble despawn rule.
    Abandoned,
}

/// A car of the traffic bubble.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
#[require(Offscreen)]
pub struct TrafficCar {
    pub segment: Segment,
    /// Position of the car centre along `segment`, m.
    pub s: f32,
    /// Speed along the path, m/s.
    pub speed: f32,
    /// Connector chosen at the end of the current lane.
    pub next: Option<u32>,
    pub mode: TrafficMode,
    /// Fixed tick the car joined its intersection's queue.
    pub waiting: Option<u64>,
}

impl TrafficCar {
    /// Driven by the traffic AI (occupies the lanes and the intersections).
    pub fn is_ai(&self) -> bool {
        matches!(
            self.mode,
            TrafficMode::Kinematic | TrafficMode::Dynamic | TrafficMode::Bailing { .. }
        )
    }
}

/// Sim-owned RNG of traffic rolls; its own stream of the city seed.
#[derive(Resource)]
pub struct TrafficRng(pub ChaCha8Rng);

impl TrafficRng {
    pub fn seeded(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        rng.set_stream(4);
        Self(rng)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        unit_f32(&mut self.0)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }
}

/// Reservations of one intersection.
#[derive(Default, Clone, Debug)]
pub struct Junction {
    /// Granted connectors and their cars.
    pub occupants: Vec<(u32, Entity)>,
    /// (tick joined, car, connector) of the cars waiting at a stop line.
    pub waiters: Vec<(u64, Entity, u32)>,
}

/// Intersection reservations by node.
#[derive(Resource, Default, Debug)]
pub struct TrafficIntersections(pub HashMap<u32, Junction>);

impl TrafficIntersections {
    /// Drops every reservation and queue place of `car`.
    pub fn release(&mut self, car: Entity) {
        for junction in self.0.values_mut() {
            junction.occupants.retain(|o| o.1 != car);
            junction.waiters.retain(|w| w.1 != car);
        }
    }

    pub fn granted(&self, node: u32, connector: u32, car: Entity) -> bool {
        self.0
            .get(&node)
            .is_some_and(|j| j.occupants.contains(&(connector, car)))
    }
}

/// Switch causes, index of `TrafficStats::switches_by_cause`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchCause {
    Character = 0,
    Vehicle = 1,
    Backstop = 2,
    Hijack = 3,
}

/// Counts of the current traffic, read by QA and the bench.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Resource)]
pub struct TrafficStats {
    /// Every non-taken traffic car.
    pub cars: u32,
    pub kinematic: u32,
    pub dynamic: u32,
    pub bailing: u32,
    pub abandoned: u32,
    pub taken: u32,
    pub spawned: u32,
    pub despawned: u32,
    /// Forward casts so far (liveness of the drive system).
    pub casts: u32,
    /// Kinematic → dynamic switches by `SwitchCause`.
    pub switches_by_cause: [u32; 4],
}

/// Spawning mode: the one-shot fill at load, then the Vermeij bands.
#[derive(Resource, Reflect, Default, PartialEq, Eq, Debug, Clone, Copy)]
#[reflect(Resource)]
pub enum TrafficPhase {
    #[default]
    InitialFill,
    Steady,
}

/// A cabin shot scared a traffic driver: the shooter did a shooting (GDD §6.4 Q-Б).
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct DriverScared {
    pub shooter: Entity,
    pub attack: u32,
    pub vehicle: Entity,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrafficSystems {
    Hijack,
    Bail,
    Drive,
    Bubble,
}

/// Gives the car up: a dynamic body with no AI and no reservations.
pub(crate) fn abandon(
    commands: &mut Commands,
    junctions: &mut TrafficIntersections,
    entity: Entity,
    car: &mut TrafficCar,
) {
    commands.entity(entity).try_insert(RigidBody::Dynamic);
    commands
        .entity(entity)
        .try_remove::<(Autopilot, DriveIntent, SleepingDisabled)>();
    junctions.release(entity);
    car.mode = TrafficMode::Abandoned;
    car.next = None;
    car.waiting = None;
}

pub struct TrafficPlugin;

impl Plugin for TrafficPlugin {
    fn build(&self, app: &mut App) {
        let running = resource_exists::<TrafficGraph>;
        app.insert_resource(TrafficRng::seeded(0))
            .init_resource::<TrafficIntersections>()
            .init_resource::<TrafficStats>()
            .init_resource::<TrafficPhase>()
            .add_message::<DriverScared>()
            .register_type::<Segment>()
            .register_type::<TrafficMode>()
            .register_type::<TrafficCar>()
            .register_type::<TrafficStats>()
            .register_type::<TrafficPhase>()
            .register_type::<DriverScared>()
            .configure_sets(
                FixedUpdate,
                (
                    TrafficSystems::Hijack.after(VehicleSystems::Enter),
                    TrafficSystems::Bail
                        .after(VehicleSystems::Bullets)
                        .before(WantedSystems),
                    TrafficSystems::Drive
                        .after(TrafficSystems::Hijack)
                        .after(TrafficSystems::Bail)
                        .before(VehicleSystems::Drive),
                    TrafficSystems::Bubble.after(TrafficSystems::Drive),
                )
                    .in_set(NpcSystems)
                    .run_if(running),
            )
            .add_systems(
                FixedUpdate,
                (
                    (hijack::on_hijack, hijack::on_leave)
                        .chain()
                        .in_set(TrafficSystems::Hijack),
                    bail::bail_out.in_set(TrafficSystems::Bail),
                    drive::advance_traffic.in_set(TrafficSystems::Drive),
                    (spawn::despawn_traffic, spawn::spawn_traffic)
                        .chain()
                        .in_set(TrafficSystems::Bubble),
                ),
            )
            // No state gate: FixedPostUpdate does not run while paused; traffic moves while wasted.
            .add_systems(
                FixedPostUpdate,
                contact::switch_to_dynamic
                    .before(PhysicsSystems::First)
                    .run_if(running),
            )
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                graph::build_traffic_graph.run_if(resource_exists::<City>),
            )
            .add_systems(NEW_CITY, (graph::drop_traffic_graph, reset_traffic))
            .add_systems(
                OnEnter(GameState::Loading),
                reseed_traffic.run_if(resource_exists::<CitySeed>),
            );
    }
}

fn reset_traffic(
    mut junctions: ResMut<TrafficIntersections>,
    mut stats: ResMut<TrafficStats>,
    mut phase: ResMut<TrafficPhase>,
    mut scared: ResMut<Messages<DriverScared>>,
) {
    junctions.0.clear();
    *stats = TrafficStats::default();
    *phase = TrafficPhase::InitialFill;
    scared.clear();
}

fn reseed_traffic(seed: Res<CitySeed>, mut rng: ResMut<TrafficRng>) {
    *rng = TrafficRng::seeded(seed.0);
}
