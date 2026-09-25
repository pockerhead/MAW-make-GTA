//! Drivable cars (GDD §5, slice T14): one dynamic box body with four raycast wheels.

mod autopilot;
mod chassis;
mod config;
mod impact;
mod seat;

pub use autopilot::{Autopilot, follow_speed, pursuit_steer, speed_throttle};
pub(crate) use seat::{ROOF_EXIT, exit_spots, pull_out};

pub use chassis::{drive_force, lateral_force, spring_force, steer_limit, wheel_forward};
pub use config::{
    AutopilotConfig, CabinConfig, DAMAGE_CONFIG, DamageConfig, GRAVITY, GripConfig,
    PedestrianDamage, SteerConfig, SuspensionConfig, UnderbodyConfig, VEHICLE_CONFIG,
    VehicleConfig, VehicleDamage, WheelsConfig,
};
pub use impact::{bullet_damage, pedestrian_damage, vehicle_damage};

use crate::character::{Character, HealthSystems};
use crate::combat::aim_yaw;
use crate::flow::{GameState, NEW_CITY, PlayingSystems};
use crate::layers::GameLayer;
use crate::police::PoliceSystems;
use crate::world::{City, CityScoped};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Wheel corners as (x sign, z sign) in the body frame: front-left, front-right, back-left,
/// back-right (front = −Z).
pub const WHEELS: [(f32, f32); 4] = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];

#[derive(Reflect, Default, Clone, Copy, Debug)]
pub struct WheelState {
    /// Suspension compression, m (0 = fully extended).
    pub compression: f32,
    pub grounded: bool,
    /// Suspension force applied this tick, N.
    pub force: f32,
}

#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(VehicleHealth, PreStepVelocity, CityScoped)]
pub struct Vehicle {
    pub driver: Option<Entity>,
    /// Front wheel steer angle, rad (+ = right).
    pub steer: f32,
    /// A grounded wheel stands on a city block (sidewalk).
    pub on_sidewalk: bool,
    /// Somebody has driven it before (the first entry is a theft).
    pub taken: bool,
    pub wheels: [WheelState; 4],
}

/// Car health; at zero the car stalls.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct VehicleHealth {
    pub current: f32,
}

/// On the player while it drives `vehicle`.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Driving {
    pub vehicle: Entity,
}

/// Driver input; the client writes it, the car of the driver reads it.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct DriveIntent {
    /// −1 (brake / reverse) .. 1 (forward).
    pub throttle: f32,
    /// −1 (left) .. 1 (right).
    pub steer: f32,
    pub handbrake: bool,
}

/// `LinearVelocity` before the current physics step: crash damage uses the closing speed before
/// the solver separated the bodies.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct PreStepVelocity(pub Vec3);

/// Work of the car systems in the current tick.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct VehicleLoad {
    pub awake: u32,
    pub rays: u32,
}

/// The player got into `vehicle`; `first` = nobody drove it before.
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct VehicleEntered {
    pub vehicle: Entity,
    pub driver: Entity,
    pub attack: u32,
    pub first: bool,
}

/// A car hit a character; `speed` is the closing speed, m/s.
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct VehicleHit {
    pub vehicle: Entity,
    pub driver: Option<Entity>,
    pub target: Entity,
    pub attack: u32,
    pub speed: f32,
}

/// A car hit the world, a person it hurt, or another car (one message per car); `speed` is the
/// closing speed, m/s.
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct VehicleImpact {
    pub vehicle: Entity,
    pub point: Vec3,
    pub speed: f32,
}

/// A pellet hit a car inside its cabin zone (`VehicleConfig::cabin`).
#[derive(Message, Reflect, Clone, Copy, Debug)]
#[reflect(Message)]
pub struct CabinHit {
    pub shooter: Entity,
    pub attack: u32,
    pub vehicle: Entity,
    pub point: Vec3,
    /// Base damage of the pellet (before `bullet_scale`).
    pub damage: f32,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum VehicleSystems {
    Enter,
    Bullets,
    Impact,
    Drive,
    Record,
    Seat,
}

/// World door point of a car at `position` / `rotation` (`door` in the body frame).
pub fn door_point(door: Vec3, position: Vec3, rotation: Quat) -> Vec3 {
    position + rotation * door
}

pub fn vehicle_bundle(
    cfg: &VehicleConfig,
    dmg: &DamageConfig,
    transform: Transform,
) -> impl Bundle {
    let chassis = Collider::convex_hull(cfg.chassis_points())
        .expect("validated chassis corners span a volume");
    (
        Vehicle::default(),
        VehicleHealth {
            current: dmg.vehicle.max_health,
        },
        Name::new("Vehicle"),
        transform,
        RigidBody::Dynamic,
        chassis,
        ColliderDensity(cfg.chassis_density()),
        CenterOfMass(cfg.center_of_mass()),
        CollisionLayers::new(
            GameLayer::Vehicle,
            [GameLayer::World, GameLayer::Character, GameLayer::Vehicle],
        ),
        CollisionEventsEnabled,
    )
}

fn spawn_parked_cars(
    mut commands: Commands,
    city: Res<City>,
    cfg: Res<VehicleConfig>,
    dmg: Res<DamageConfig>,
) {
    let height = cfg.rest_height();
    for spot in &city.0.parking {
        let heading = Vec3::new(spot.heading.x, 0.0, spot.heading.y);
        let transform = Transform::from_xyz(spot.position.x, height, spot.position.y)
            .with_rotation(Quat::from_rotation_y(aim_yaw(heading)));
        commands.spawn(vehicle_bundle(&cfg, &dmg, transform));
    }
}

fn clear_vehicle_messages(
    mut entered: ResMut<Messages<VehicleEntered>>,
    mut hits: ResMut<Messages<VehicleHit>>,
    mut impacts: ResMut<Messages<VehicleImpact>>,
    mut cabin: ResMut<Messages<CabinHit>>,
) {
    entered.clear();
    hits.clear();
    impacts.clear();
    cabin.clear();
}

pub struct VehiclePlugin;

impl Plugin for VehiclePlugin {
    fn build(&self, app: &mut App) {
        app.register_required_components::<Character, DriveIntent>()
            .register_required_components::<Character, PreStepVelocity>()
            .init_resource::<VehicleLoad>()
            .add_message::<VehicleEntered>()
            .add_message::<VehicleHit>()
            .add_message::<VehicleImpact>()
            .add_message::<CabinHit>()
            .register_type::<CabinHit>()
            .register_type::<Autopilot>()
            .register_type::<Vehicle>()
            .register_type::<WheelState>()
            .register_type::<VehicleHealth>()
            .register_type::<Driving>()
            .register_type::<DriveIntent>()
            .register_type::<PreStepVelocity>()
            .register_type::<VehicleLoad>()
            .register_type::<VehicleEntered>()
            .register_type::<VehicleHit>()
            .register_type::<VehicleImpact>()
            .configure_sets(
                FixedUpdate,
                (
                    VehicleSystems::Enter
                        .in_set(PlayingSystems)
                        .before(HealthSystems::Damage),
                    VehicleSystems::Impact.in_set(HealthSystems::Damage),
                    VehicleSystems::Bullets
                        .after(HealthSystems::Damage)
                        .before(HealthSystems::Death),
                    VehicleSystems::Drive
                        .after(VehicleSystems::Enter)
                        .after(VehicleSystems::Bullets)
                        .after(VehicleSystems::Impact)
                        .after(PoliceSystems),
                ),
            )
            .configure_sets(
                FixedPostUpdate,
                (
                    VehicleSystems::Record.before(PhysicsSystems::First),
                    VehicleSystems::Seat.after(PhysicsSystems::Last),
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    seat::enter_exit.in_set(VehicleSystems::Enter),
                    impact::apply_impacts.in_set(VehicleSystems::Impact),
                    impact::apply_bullet_hits.in_set(VehicleSystems::Bullets),
                    (autopilot::steer_autopilots, chassis::drive_vehicles)
                        .chain()
                        .in_set(VehicleSystems::Drive),
                ),
            )
            .add_systems(
                FixedPostUpdate,
                (
                    impact::record_pre_step.in_set(VehicleSystems::Record),
                    seat::sync_seats.in_set(VehicleSystems::Seat),
                ),
            )
            .add_systems(OnEnter(GameState::Wasted), seat::eject_all)
            .add_systems(OnEnter(GameState::Busted), seat::eject_all)
            .add_systems(NEW_CITY, clear_vehicle_messages)
            .add_systems(
                OnTransition {
                    exited: GameState::Loading,
                    entered: GameState::Playing,
                },
                spawn_parked_cars.run_if(resource_exists::<City>),
            );
    }
}
