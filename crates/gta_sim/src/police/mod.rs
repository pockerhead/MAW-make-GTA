//! Police on foot (GDD §6.4): the dispatcher keeps the unit count at the wanted row, the cop FSM,
//! the arrest.

mod arrest;
mod behavior;
mod car_dispatch;
mod car_route;
mod cars;
mod dispatch;
pub mod fsm;

pub use arrest::ArrestAttempt;
pub use car_route::find_lane_route;
pub use cars::{
    CarSenses, CrewOf, PoliceCar, PoliceCarRng, PoliceCarRoute, PoliceCarState, next_car_state,
};

use crate::character::{
    Character, CharacterSchemeConfig, Gait, Health, HealthConfig, HealthSystems, LocomotionConfig,
    character_components,
};
use crate::combat::{Loadout, Weapon, WeaponsConfig, acquire, unit_f32};
use crate::flow::{GameState, NEW_CITY, NpcSystems, PlayingSystems};
use crate::gang::Faction;
use crate::navigation::Route;
use crate::perception::{AiSystems, Perception};
use crate::population::{Appearance, Offscreen, PopulationSystems};
use crate::tactics::Discipline;
use crate::traffic::TrafficGraph;
use crate::wanted::{STARS, WantedSystems};
use crate::world::CitySeed;
use bevy::prelude::*;
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};
use serde::Deserialize;

/// Path of the police config, relative to the assets root.
pub const POLICE_CONFIG: &str = "police/escalation.ron";

/// Police tuning (GDD §6.4).
#[derive(Resource, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EscalationConfig {
    /// Index = wanted stars − 1.
    pub stars: [EscalationRow; STARS],
    pub patrol: UnitSpec,
    pub swat: UnitSpec,
    /// Flat distance of a spawn point from the player (inner, outer), m.
    pub spawn_ring: (f32, f32),
    pub spawns_per_tick: u32,
    pub car: PoliceCarConfig,
    pub arrest: ArrestConfig,
    /// A cop this close (flat) to its goal has arrived, m.
    pub search_arrive_distance: f32,
    pub combat: PoliceCombatConfig,
}

/// Foot units of one wanted star.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EscalationRow {
    /// Live units at most.
    pub units: u32,
    /// Of them SWAT.
    pub swat: u32,
    /// Seconds before a lost unit is replaced.
    pub reinforce_seconds: f32,
    /// Cops try to arrest instead of shooting.
    pub arrest: bool,
    /// New units come in from other sides of the last known position.
    pub surround: bool,
    /// Police cars at most.
    pub cars: u32,
}

/// Police cars (GDD §5.3).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PoliceCarConfig {
    /// Cops aboard a new car; they count in the row's units.
    pub crew: u32,
    /// Flat distance of a car spawn point from the player (inner, outer), m.
    pub spawn_ring: (f32, f32),
    pub spawns_per_tick: u32,
    /// Speed on lanes, m/s.
    pub pursuit_speed: f32,
    /// Speed on intersection connectors, m/s.
    pub turn_speed: f32,
    /// The crew gets out this close to the player, m.
    pub dismount_distance: f32,
    /// A seen driver this close is chased straight, m.
    pub direct_chase_distance: f32,
    /// Seconds a driver stays stopped before the crew gets out.
    pub stopped_seconds: f32,
    /// The crew re-boards when the player drives farther than this from the car, m.
    pub reboard_distance: f32,
    /// Seconds after which cops that have not re-boarded are left behind.
    pub reboard_timeout_seconds: f32,
    pub route_refresh_seconds: f32,
    /// A* searches per tick for all police cars.
    pub routes_per_tick: u32,
    /// Seconds a responding car may crawl (held up in traffic) before its crew goes on foot.
    pub blocked_seconds: f32,
    /// A route ends on any lane within this of the lane nearest to the target (beyond that lane's own
    /// distance), m: the far side of the target's street counts, no block is looped for the near side.
    pub goal_margin: f32,
    /// A stopping car pulls over this far right of its lane where that spot is free (0: never), m.
    pub pull_over: f32,
    /// Seconds the player's car must move (above exit speed) before the crew counts it as driven off.
    pub moving_seconds: f32,
    /// A car held up inside an intersection lets its crew out there after `blocked_seconds` times this.
    pub junction_factor: f32,
}

/// Gear and distance band of one unit kind.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct UnitSpec {
    pub gun: Weapon,
    pub armor: f32,
    /// Reserve rounds at spawn (capped by the weapon's `max_reserve`).
    pub reserve: u32,
    /// Distance band an armed unit keeps (min, max), m.
    pub keep_distance: (f32, f32),
}

/// Arrest rules (GDD §6.4).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ArrestConfig {
    /// A cop this close holds the player, m.
    pub distance: f32,
    /// An arresting cop stops this close, m.
    pub stand_distance: f32,
    /// Seconds the player stays passive within `distance` before BUSTED.
    pub seconds: f32,
    /// The player this far from the arresting cop broke free, m.
    pub break_free_distance: f32,
    /// Seconds a witnessed attack by the player makes arrest-row cops shoot.
    pub hostile_seconds: f32,
    /// Seconds a cop at the door of a stopped car takes to pull the driver out.
    pub pull_out_seconds: f32,
    /// Seconds more the pull waits for a blocked left door; then the driver is pulled out at any clear
    /// exit and the arrest starts over on foot.
    pub pull_give_up_seconds: f32,
    /// A cop this close to the door of the driver's car goes for the door even without sight of the
    /// driver (walking around the cars in the way), m.
    pub approach_distance: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PoliceCombatConfig {
    /// Cone added on top of the weapon spread, degrees.
    pub aim_error_deg: f32,
    /// Seconds between trigger pulls (min, max).
    pub trigger_seconds: (f32, f32),
    /// Clearance kept between the line of fire and a body that must not be hit, on top of the body
    /// radius and the spread cone at that distance, m.
    pub fire_line_margin: f32,
    /// A spared body this close to the target yields the target to the fight, m.
    pub pressed_distance: f32,
    /// Sideways offsets of the spots a cop with a blocked line tries, m.
    pub reposition_offsets: Vec<f32>,
    /// Step back or forward along the line, m.
    pub reposition_step: f32,
    pub chase_gait: Gait,
    /// Gait to a clear spot and when backing off.
    pub reposition_gait: Gait,
    pub search_gait: Gait,
    pub leave_gait: Gait,
}

impl PoliceCombatConfig {
    pub(crate) fn discipline(&self) -> Discipline<'_> {
        Discipline {
            aim_error_deg: self.aim_error_deg,
            fire_line_margin: self.fire_line_margin,
            pressed_distance: self.pressed_distance,
            reposition_offsets: &self.reposition_offsets,
            reposition_step: self.reposition_step,
            reposition_gait: self.reposition_gait,
            chase_gait: self.chase_gait,
        }
    }
}

fn positive(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{field} {value} must be finite and > 0"))
    }
}

fn not_negative(field: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(format!("{field} {value} must be finite and >= 0"))
    }
}

fn band(field: &str, (lo, hi): (f32, f32)) -> Result<(), String> {
    if lo.is_finite() && hi.is_finite() && 0.0 < lo && lo < hi {
        Ok(())
    } else {
        Err(format!("{field} ({lo}, {hi}) must satisfy 0 < lo < hi"))
    }
}

impl EscalationConfig {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_rows()?;
        for (name, spec) in [("patrol", &self.patrol), ("swat", &self.swat)] {
            not_negative(&format!("{name}.armor"), spec.armor)?;
            band(&format!("{name}.keep_distance"), spec.keep_distance)?;
        }
        band("spawn_ring", self.spawn_ring)?;
        if self.spawns_per_tick < 1 {
            return Err("spawns_per_tick must be >= 1".into());
        }
        self.validate_arrest()?;
        self.validate_cars()?;
        positive("search_arrive_distance", self.search_arrive_distance)?;
        self.validate_combat()
    }

    fn validate_cars(&self) -> Result<(), String> {
        let c = &self.car;
        if c.crew < 1 {
            return Err("car.crew must be >= 1".into());
        }
        if c.spawns_per_tick < 1 {
            return Err("car.spawns_per_tick must be >= 1".into());
        }
        if c.routes_per_tick < 1 {
            return Err("car.routes_per_tick must be >= 1".into());
        }
        band("car.spawn_ring", c.spawn_ring)?;
        for (field, value) in [
            ("car.pursuit_speed", c.pursuit_speed),
            ("car.turn_speed", c.turn_speed),
            ("car.dismount_distance", c.dismount_distance),
            ("car.direct_chase_distance", c.direct_chase_distance),
            ("car.stopped_seconds", c.stopped_seconds),
            ("car.reboard_distance", c.reboard_distance),
            ("car.reboard_timeout_seconds", c.reboard_timeout_seconds),
            ("car.route_refresh_seconds", c.route_refresh_seconds),
            ("car.blocked_seconds", c.blocked_seconds),
            ("car.goal_margin", c.goal_margin),
            ("car.moving_seconds", c.moving_seconds),
        ] {
            positive(field, value)?;
        }
        not_negative("car.pull_over", c.pull_over)?;
        if !(c.junction_factor.is_finite() && c.junction_factor >= 1.0) {
            return Err(format!(
                "car.junction_factor {} must be finite and >= 1",
                c.junction_factor
            ));
        }
        if c.dismount_distance >= c.direct_chase_distance {
            return Err(format!(
                "car.dismount_distance {} must be < car.direct_chase_distance {}",
                c.dismount_distance, c.direct_chase_distance
            ));
        }
        Ok(())
    }

    fn validate_rows(&self) -> Result<(), String> {
        if self.stars[0].units < 1 {
            return Err("stars[0].units must be >= 1".into());
        }
        for (i, row) in self.stars.iter().enumerate() {
            if row.swat > row.units {
                return Err(format!(
                    "stars[{i}].swat {} must be <= stars[{i}].units {}",
                    row.swat, row.units
                ));
            }
            not_negative(
                &format!("stars[{i}].reinforce_seconds"),
                row.reinforce_seconds,
            )?;
            if row.cars < 1 {
                return Err(format!("stars[{i}].cars must be >= 1"));
            }
            let Some(previous) = i.checked_sub(1).map(|p| &self.stars[p]) else {
                continue;
            };
            if row.units < previous.units {
                return Err(format!(
                    "stars[{i}].units {} must be >= stars[{}].units {}",
                    row.units,
                    i - 1,
                    previous.units
                ));
            }
            if row.cars < previous.cars {
                return Err(format!(
                    "stars[{i}].cars {} must be >= stars[{}].cars {}",
                    row.cars,
                    i - 1,
                    previous.cars
                ));
            }
            if row.swat < previous.swat {
                return Err(format!(
                    "stars[{i}].swat {} must be >= stars[{}].swat {}",
                    row.swat,
                    i - 1,
                    previous.swat
                ));
            }
        }
        Ok(())
    }

    fn validate_arrest(&self) -> Result<(), String> {
        let a = &self.arrest;
        positive("arrest.stand_distance", a.stand_distance)?;
        positive("arrest.seconds", a.seconds)?;
        positive("arrest.pull_out_seconds", a.pull_out_seconds)?;
        positive("arrest.pull_give_up_seconds", a.pull_give_up_seconds)?;
        positive("arrest.approach_distance", a.approach_distance)?;
        not_negative("arrest.hostile_seconds", a.hostile_seconds)?;
        if a.distance <= a.stand_distance {
            return Err(format!(
                "arrest.distance {} must be > arrest.stand_distance {}",
                a.distance, a.stand_distance
            ));
        }
        if !(a.distance < a.break_free_distance && a.break_free_distance.is_finite()) {
            return Err(format!(
                "arrest.break_free_distance {} must be finite and > arrest.distance {}",
                a.break_free_distance, a.distance
            ));
        }
        Ok(())
    }

    fn validate_combat(&self) -> Result<(), String> {
        let c = &self.combat;
        if !(c.aim_error_deg.is_finite() && (0.0..90.0).contains(&c.aim_error_deg)) {
            return Err(format!(
                "combat.aim_error_deg {} must be in [0, 90)",
                c.aim_error_deg
            ));
        }
        let (t0, t1) = c.trigger_seconds;
        if !(t0.is_finite() && t1.is_finite() && 0.0 < t0 && t0 <= t1) {
            return Err(format!(
                "combat.trigger_seconds {:?} must satisfy 0 < lo <= hi",
                c.trigger_seconds
            ));
        }
        not_negative("combat.fire_line_margin", c.fire_line_margin)?;
        positive("combat.pressed_distance", c.pressed_distance)?;
        if c.reposition_offsets.is_empty()
            || !c
                .reposition_offsets
                .iter()
                .all(|o| o.is_finite() && *o > 0.0)
        {
            return Err(format!(
                "combat.reposition_offsets {:?} must list at least one finite offset > 0",
                c.reposition_offsets
            ));
        }
        positive("combat.reposition_step", c.reposition_step)
    }

    /// Units and cars spawn inside the population bubble: past `despawn_distance` they would vanish at once.
    pub fn validate_ring(&self, despawn_distance: f32) -> Result<(), String> {
        if self.spawn_ring.1 >= despawn_distance {
            return Err(format!(
                "spawn_ring {:?} must end below the population despawn_distance {despawn_distance}",
                self.spawn_ring
            ));
        }
        if self.car.spawn_ring.1 >= despawn_distance {
            return Err(format!(
                "car.spawn_ring {:?} must end below the population despawn_distance {despawn_distance}",
                self.car.spawn_ring
            ));
        }
        Ok(())
    }

    pub fn spec(&self, kind: UnitKind) -> &UnitSpec {
        match kind {
            UnitKind::Patrol => &self.patrol,
            UnitKind::Swat => &self.swat,
        }
    }
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Patrol,
    Swat,
}

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopState {
    /// Heading for the last known position.
    Respond,
    /// Walking up to the player to arrest (arrest rows only).
    Arrest,
    Attack,
    /// Walking between points of the search circle.
    Search,
    /// Wanted cleared: walking away until despawned; never comes back.
    Leave,
    Dead,
}

#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
#[require(Character, Perception, Offscreen, Route)]
pub struct PoliceUnit {
    pub kind: UnitKind,
    pub state: CopState,
    /// The cop sees the player (view cone, distance, clear line), refreshed on its AI slot.
    pub sees: bool,
    /// Where it heads this tick (chest height): the last known position, the player, a search point.
    pub dest: Option<Vec3>,
    /// Clear line from the eyes to `dest`, refreshed on the AI slot.
    pub dest_clear: bool,
    /// Yaw offset of the current direct seek chosen by `avoid_offset`, refreshed on the AI slot.
    pub avoid: f32,
    /// Seconds until the next trigger pull may happen (clamped at 0).
    pub trigger_left: f32,
    /// Spot (chest) with a clear line of fire it walks to while its own line is blocked.
    pub reposition: Option<Vec3>,
    /// Current point (chest height) of the search circle.
    pub search_point: Option<Vec3>,
}

/// Sim-owned RNG of police rolls; its own stream of the combat seed.
#[derive(Resource)]
pub struct PoliceRng(pub ChaCha8Rng);

impl PoliceRng {
    pub fn seeded(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        rng.set_stream(3);
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

/// Uniform in `[lo, hi)`.
fn roll(rng: &mut PoliceRng, (lo, hi): (f32, f32)) -> f32 {
    lo + (hi - lo) * rng.unit()
}

/// Active units (not dead, not leaving; on foot plus crews aboard) and active police cars the
/// dispatchers counted in their last run; read by QA.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct PoliceDispatcher {
    pub units: u32,
    pub swat: u32,
    /// Seconds until a lost unit may be replaced.
    pub reinforce_left: f32,
    /// Police cars responding, chasing or with their crew out.
    pub cars: u32,
}

/// A witnessed attack by the player: arrest-row cops shoot while `hostile_left > 0`.
#[derive(Resource, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct PoliceAlert {
    pub hostile_left: f32,
}

/// Police set that runs after the population and the wanted level of the tick.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PoliceSystems;

/// A foot cop standing at `feet`, facing `facing_yaw`, gun and reserve of its kind.
#[allow(clippy::too_many_arguments)]
pub fn police_unit_bundle(
    loco: &LocomotionConfig,
    handle: Handle<CharacterSchemeConfig>,
    health: &HealthConfig,
    weapons: &WeaponsConfig,
    spec: &UnitSpec,
    kind: UnitKind,
    feet: Vec3,
    facing_yaw: f32,
    appearance: Appearance,
) -> impl Bundle {
    let mut loadout = Loadout::default();
    let stats = weapons.stats(spec.gun);
    let slot = &mut loadout.guns[spec.gun.index()];
    acquire(slot, stats, true);
    slot.reserve = spec.reserve.min(stats.max_reserve);
    (
        PoliceUnit {
            kind,
            state: CopState::Respond,
            sees: false,
            dest: None,
            dest_clear: false,
            avoid: 0.0,
            trigger_left: 0.0,
            reposition: None,
            search_point: None,
        },
        Faction::Police,
        appearance,
        Name::new(match kind {
            UnitKind::Patrol => "Cop",
            UnitKind::Swat => "SWAT",
        }),
        Transform::from_translation(feet + Vec3::Y * loco.float_height)
            .with_rotation(Quat::from_rotation_y(facing_yaw)),
        character_components(loco, handle),
        Health {
            armor: spec.armor,
            ..Health::full(health)
        },
        loadout,
    )
}

pub struct PolicePlugin {
    /// Seed of `PoliceRng` (the combat seed).
    pub seed: u64,
}

impl Plugin for PolicePlugin {
    fn build(&self, app: &mut App) {
        let lanes = resource_exists::<TrafficGraph>;
        app.insert_resource(PoliceRng::seeded(self.seed))
            .insert_resource(PoliceCarRng::seeded(self.seed))
            .init_resource::<PoliceDispatcher>()
            .init_resource::<ArrestAttempt>()
            .init_resource::<PoliceAlert>()
            .register_type::<UnitKind>()
            .register_type::<CopState>()
            .register_type::<PoliceUnit>()
            .register_type::<PoliceDispatcher>()
            .register_type::<ArrestAttempt>()
            .register_type::<PoliceAlert>()
            .register_type::<PoliceCar>()
            .register_type::<PoliceCarState>()
            .register_type::<PoliceCarRoute>()
            .register_type::<CrewOf>()
            // After the population (it resets the shared ray budget) and the wanted level (fresh stars).
            .configure_sets(
                FixedUpdate,
                PoliceSystems
                    .after(PopulationSystems)
                    .after(WantedSystems)
                    .in_set(NpcSystems),
            )
            .add_systems(
                FixedUpdate,
                (
                    behavior::police_death
                        .in_set(HealthSystems::Death)
                        .in_set(NpcSystems),
                    behavior::police_alert.in_set(AiSystems::Perceive),
                    (behavior::police_fsm, cars::board_police_cars.run_if(lanes))
                        .chain()
                        .in_set(AiSystems::Decide),
                    (
                        cars::on_police_car_entered.run_if(lanes),
                        car_dispatch::despawn_police_cars.run_if(lanes),
                        car_dispatch::dispatch_police_cars
                            .in_set(PlayingSystems)
                            .run_if(lanes),
                        car_route::drive_police_cars.run_if(lanes),
                        dispatch::despawn_police,
                        dispatch::dispatch_police.in_set(PlayingSystems),
                    )
                        .chain()
                        .in_set(PoliceSystems),
                    (arrest::pull_out_driver, arrest::arrest_player)
                        .chain()
                        .after(AiSystems::Decide)
                        .before(WantedSystems)
                        .in_set(PlayingSystems),
                ),
            )
            .add_systems(OnEnter(GameState::Wasted), arrest::reset_arrest)
            .add_systems(OnExit(GameState::Busted), arrest::reset_arrest)
            .add_systems(NEW_CITY, (arrest::reset_arrest, reset_dispatcher))
            .add_systems(
                OnEnter(GameState::Loading),
                reseed_police.run_if(resource_exists::<CitySeed>),
            );
    }
}

fn reset_dispatcher(mut dispatcher: ResMut<PoliceDispatcher>) {
    *dispatcher = PoliceDispatcher::default();
}

fn reseed_police(seed: Res<CitySeed>, mut rng: ResMut<PoliceRng>, mut cars: ResMut<PoliceCarRng>) {
    *rng = PoliceRng::seeded(seed.0);
    *cars = PoliceCarRng::seeded(seed.0);
}
