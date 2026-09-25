//! Police cars (GDD §5.3, slice T15): the car state table, the crew getting out and back in, a
//! hijacked police car. Dispatch and despawn: `car_dispatch.rs`; driving: `car_route.rs`.

use super::{
    CopState, EscalationConfig, PoliceCarConfig, PoliceUnit, UnitKind, police_unit_bundle,
};
use crate::character::{CharacterControlConfig, Dead, HealthConfig, LocomotionConfig};
use crate::combat::{WeaponsConfig, aim_yaw, unit_f32};
use crate::navigation::flat_distance;
use crate::player::Player;
use crate::population::{Appearance, Offscreen};
use crate::vehicle::{
    Autopilot, DriveIntent, Driving, ROOF_EXIT, VehicleConfig, VehicleEntered, door_point,
    exit_spots,
};
use avian3d::prelude::*;
use bevy::prelude::*;
use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};
use std::collections::HashMap;

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoliceCarState {
    /// Driving over the lanes to the player or the last known position.
    Respond,
    /// Driving straight at a seen driver.
    Chase,
    /// Parked, the crew out.
    Dismounted,
    /// Wanted cleared: driving the lanes until despawned.
    Leave,
    /// The player drives it.
    Taken,
    /// Nobody left: a parked car under the bubble despawn rule.
    Abandoned,
}

/// A police car and the cops aboard it.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
#[require(Offscreen)]
pub struct PoliceCar {
    pub state: PoliceCarState,
    /// Kinds of the cops aboard.
    pub crew: Vec<UnitKind>,
    /// Seconds the player's car has stood (at most exit speed).
    pub stopped: f32,
    /// Seconds the player's car has moved (above exit speed).
    pub moving: f32,
    /// Seconds this car has crawled (at most exit speed) while responding: held up in traffic.
    pub blocked: f32,
    /// Seconds the crew still has to re-board once the player drove off.
    pub reboard_left: f32,
}

impl PoliceCar {
    /// Counted by the dispatcher (responding, chasing, crew out).
    pub fn active(&self) -> bool {
        matches!(
            self.state,
            PoliceCarState::Respond | PoliceCarState::Chase | PoliceCarState::Dismounted
        )
    }

    /// Active with a cop aboard: it sees and witnesses.
    pub fn crewed(&self) -> bool {
        self.active() && !self.crew.is_empty()
    }

    /// The player drives away from this car (`distance` m off): nobody gets out, a crew that is out
    /// walks back and re-boards. Only a car moving for `moving_seconds` is driven off: a stop-and-go
    /// driver never is (the crew would hop out and in).
    pub fn driven_off(&self, cfg: &PoliceCarConfig, driving: bool, distance: f32) -> bool {
        driving && self.moving >= cfg.moving_seconds && distance > cfg.reboard_distance
    }
}

/// The lane route of a police car: lanes to drive, the index of the current one, the goal lane.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct PoliceCarRoute {
    pub lanes: Vec<u32>,
    pub next: usize,
    pub goal: Option<u32>,
    /// Seconds since the route was planned.
    pub age: f32,
}

/// A cop that got out of `car` and may get back in.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct CrewOf {
    pub car: Entity,
}

/// Sim-owned RNG of police car rolls; its own stream of the city seed.
#[derive(Resource)]
pub struct PoliceCarRng(pub ChaCha8Rng);

impl PoliceCarRng {
    pub fn seeded(seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        rng.set_stream(5);
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

/// What a police car knows this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct CarSenses {
    pub stars: u8,
    /// The player drives a car.
    pub driving: bool,
    /// The crew sees the player.
    pub sees: bool,
    /// Flat distance to the player, m.
    pub distance: f32,
    /// This car's speed, m/s.
    pub speed: f32,
    /// Seconds the player's car has stood.
    pub player_stopped: f32,
    pub crew_aboard: u32,
    /// Live cops of this car outside it.
    pub crew_outside: u32,
    pub reboard_left: f32,
    /// The route to the last known position is driven to its end.
    pub route_done: bool,
    /// Held up (in a traffic queue) long enough: the crew goes on on foot.
    pub blocked: bool,
    /// No lane route leads to the target: the crew goes on on foot.
    pub no_route: bool,
    /// The car is in an intersection: it does not stop there.
    pub in_junction: bool,
    /// Held up in the intersection for `blocked_seconds × junction_factor`: the crew gets out there.
    pub stuck_in_junction: bool,
    /// `PoliceCar::driven_off`: a crew that got out would get back in.
    pub driven_off: bool,
}

/// Next police car state; Leave, Taken and Abandoned are terminal.
pub fn next_car_state(
    state: PoliceCarState,
    s: &CarSenses,
    cfg: &PoliceCarConfig,
    exit_max_speed: f32,
) -> PoliceCarState {
    use PoliceCarState::*;
    let stopped_driver = s.driving && s.player_stopped >= cfg.stopped_seconds;
    let chase = s.driving && s.sees && s.distance <= cfg.direct_chase_distance && !stopped_driver;
    match state {
        Leave | Taken | Abandoned => state,
        Respond | Chase | Dismounted if s.stars == 0 => Leave,
        Respond if chase => Chase,
        Respond => {
            // A driver must have stopped for `stopped_seconds`, whatever the reason to get out.
            let may_stop = !s.driving || stopped_driver;
            let near = s.distance <= cfg.dismount_distance;
            let arrived = may_stop && (near || s.route_done || s.blocked || s.no_route);
            let placed = !s.in_junction || s.stuck_in_junction;
            if s.speed <= exit_max_speed && arrived && placed && !s.driven_off {
                Dismounted
            } else {
                Respond
            }
        }
        Chase if chase => Chase,
        Chase => Respond,
        Dismounted if s.crew_aboard == 0 && s.crew_outside == 0 => Abandoned,
        Dismounted if s.crew_aboard >= 1 && (s.crew_outside == 0 || s.reboard_left <= 0.0) => {
            Respond
        }
        Dismounted => Dismounted,
    }
}

/// Configs a dismounting crew is made from.
pub(super) type CrewConfigs<'a> = (
    &'a EscalationConfig,
    &'a VehicleConfig,
    &'a LocomotionConfig,
    &'a HealthConfig,
    &'a CharacterControlConfig,
    &'a WeaponsConfig,
);

/// The crew aboard gets out at the clear doors of the car, one cop per door; the rest stays aboard
/// (never onto the roof).
pub(super) fn dismount(
    commands: &mut Commands,
    spatial: &SpatialQuery,
    configs: CrewConfigs,
    rng: &mut PoliceCarRng,
    car: (Entity, Vec3, Quat),
    crew: &mut Vec<UnitKind>,
) {
    let (esc, vehicle, loco, health, handle, weapons) = configs;
    let spots: Vec<_> = exit_spots(spatial, vehicle, loco, car, &[])
        .into_iter()
        .filter(|(k, _)| *k != ROOF_EXIT)
        .collect();
    let out = crew.len().min(spots.len());
    for (kind, (_, spot)) in crew.drain(..out).zip(spots) {
        let feet = spot.centre - Vec3::Y * loco.float_height;
        commands
            .spawn(police_unit_bundle(
                loco,
                handle.0.clone(),
                health,
                weapons,
                esc.spec(kind),
                kind,
                feet,
                aim_yaw(car.2 * Vec3::NEG_Z),
                Appearance(rng.next_u32()),
            ))
            .insert(CrewOf { car: car.0 });
    }
}

/// The player got into a police car: its crew gets out at once; the car is the player's.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn on_police_car_entered(
    mut commands: Commands,
    spatial: SpatialQuery,
    configs: (
        Res<EscalationConfig>,
        Res<VehicleConfig>,
        Res<LocomotionConfig>,
        Res<HealthConfig>,
        Res<CharacterControlConfig>,
        Res<WeaponsConfig>,
    ),
    mut rng: ResMut<PoliceCarRng>,
    mut entered: MessageReader<VehicleEntered>,
    mut cars: Query<(&mut PoliceCar, &Position, &Rotation)>,
) {
    let (esc, vehicle, loco, health, handle, weapons) = &configs;
    for entry in entered.read() {
        let Ok((mut car, position, rotation)) = cars.get_mut(entry.vehicle) else {
            continue;
        };
        if !matches!(car.state, PoliceCarState::Taken | PoliceCarState::Abandoned) {
            let mut crew = std::mem::take(&mut car.crew);
            dismount(
                &mut commands,
                &spatial,
                (esc, vehicle, loco, health, handle, weapons),
                &mut rng,
                (entry.vehicle, position.0, rotation.0),
                &mut crew,
            );
            // A cop with no clear exit is dropped: nobody stays aboard the player's car.
        }
        car.state = PoliceCarState::Taken;
        commands
            .entity(entry.vehicle)
            .try_remove::<(Autopilot, DriveIntent, PoliceCarRoute)>();
    }
}

/// A cop of a parked car walks back to its nearer door while the player drives away (`police_fsm`) and
/// gets in there; a cop whose car is gone or taken stays an ordinary foot cop.
#[allow(clippy::type_complexity)]
pub(super) fn board_police_cars(
    mut commands: Commands,
    configs: (Res<VehicleConfig>, Res<EscalationConfig>),
    player: Query<(&Position, Has<Driving>), With<Player>>,
    cops: Query<(Entity, &CrewOf, &Position, &PoliceUnit), Without<Dead>>,
    mut cars: Query<(&mut PoliceCar, &Position, &Rotation), Without<PoliceUnit>>,
) {
    let (vehicle, esc) = configs;
    let away = player.single().ok().filter(|p| p.1).map(|p| p.0.0);
    for (cop, crew_of, position, unit) in &cops {
        let Ok((mut car, car_pos, car_rot)) = cars.get_mut(crew_of.car) else {
            commands.entity(cop).try_remove::<CrewOf>();
            continue;
        };
        if matches!(
            car.state,
            PoliceCarState::Taken | PoliceCarState::Abandoned | PoliceCarState::Leave
        ) {
            commands.entity(cop).try_remove::<CrewOf>();
            continue;
        }
        let driven_off =
            away.is_some_and(|p| car.driven_off(&esc.car, true, flat_distance(p, car_pos.0)));
        let door = nearest_door(&vehicle, car_pos.0, car_rot.0, position.0);
        if car.state == PoliceCarState::Dismounted
            && driven_off
            && flat_distance(position.0, door) <= vehicle.enter_radius
        {
            car.crew.push(unit.kind);
            commands.entity(cop).try_despawn();
        }
    }
}

/// The door of a car at `position` / `rotation` nearest to `from`: a cop gets in on its own side.
pub(super) fn nearest_door(
    vehicle: &VehicleConfig,
    position: Vec3,
    rotation: Quat,
    from: Vec3,
) -> Vec3 {
    let d = vehicle.door();
    let (left, right) = (
        door_point(d, position, rotation),
        door_point(Vec3::new(-d.x, d.y, d.z), position, rotation),
    );
    if flat_distance(from, left) <= flat_distance(from, right) {
        left
    } else {
        right
    }
}

/// Live cops outside each car.
pub(super) fn crews_outside<'a>(
    cops: impl Iterator<Item = (&'a CrewOf, &'a PoliceUnit)>,
) -> HashMap<Entity, u32> {
    let mut outside = HashMap::new();
    for (crew_of, unit) in cops {
        if unit.state != CopState::Dead {
            *outside.entry(crew_of.car).or_default() += 1;
        }
    }
    outside
}

#[cfg(test)]
mod tests {
    use super::*;
    use PoliceCarState::*;

    fn cfg() -> PoliceCarConfig {
        let esc: EscalationConfig =
            ron::from_str(include_str!("../../../../assets/police/escalation.ron"))
                .unwrap_or_else(|e| panic!("GATE BROKEN: police/escalation.ron: {e}"));
        let c = esc.car;
        assert_eq!(
            (
                c.dismount_distance,
                c.direct_chase_distance,
                c.stopped_seconds
            ),
            (20.0, 40.0, 1.0),
            "GATE BROKEN: shipped car block changed"
        );
        c
    }

    fn senses() -> CarSenses {
        CarSenses {
            stars: 2,
            distance: 80.0,
            speed: 15.0,
            crew_aboard: 2,
            reboard_left: 10.0,
            ..default()
        }
    }

    #[test]
    fn next_car_state_table() {
        let c = cfg();
        let exit = 3.0;
        let step = |state, s: CarSenses| next_car_state(state, &s, &c, exit);
        // Any active state at 0 stars leaves.
        for state in [Respond, Chase, Dismounted] {
            assert_eq!(
                step(
                    state,
                    CarSenses {
                        stars: 0,
                        ..senses()
                    }
                ),
                Leave
            );
        }
        for state in [Leave, Taken, Abandoned] {
            assert_eq!(step(state, senses()), state, "{state:?} terminal");
            assert_eq!(
                step(
                    state,
                    CarSenses {
                        stars: 0,
                        ..senses()
                    }
                ),
                state
            );
        }
        // Respond -> Chase: a seen driver within the chase distance.
        let chase = CarSenses {
            driving: true,
            sees: true,
            distance: 39.0,
            ..senses()
        };
        assert_eq!(step(Respond, chase), Chase);
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    sees: false,
                    ..chase
                }
            ),
            Respond
        );
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    distance: 41.0,
                    ..chase
                }
            ),
            Respond
        );
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    driving: false,
                    ..chase
                }
            ),
            Respond
        );
        // Chase -> Respond when that ends, incl. a driver that has stopped.
        assert_eq!(step(Chase, chase), Chase);
        assert_eq!(
            step(
                Chase,
                CarSenses {
                    sees: false,
                    ..chase
                }
            ),
            Respond
        );
        assert_eq!(
            step(
                Chase,
                CarSenses {
                    player_stopped: 1.0,
                    ..chase
                }
            ),
            Respond
        );
        // Respond -> Dismounted: slow and (on foot near, stopped driver near, route done).
        let on_foot = CarSenses {
            distance: 19.0,
            speed: 2.0,
            ..senses()
        };
        assert_eq!(step(Respond, on_foot), Dismounted);
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    speed: 3.5,
                    ..on_foot
                }
            ),
            Respond
        );
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    distance: 21.0,
                    ..on_foot
                }
            ),
            Respond
        );
        let stopped = CarSenses {
            driving: true,
            player_stopped: 1.0,
            ..on_foot
        };
        assert_eq!(step(Respond, stopped), Dismounted);
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    player_stopped: 0.5,
                    ..stopped
                }
            ),
            Respond
        );
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    distance: 90.0,
                    route_done: true,
                    ..on_foot
                }
            ),
            Dismounted
        );
        // Held up in traffic far away: the crew goes on foot; still only once slow.
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    distance: 90.0,
                    blocked: true,
                    ..on_foot
                }
            ),
            Dismounted
        );
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    distance: 90.0,
                    blocked: true,
                    speed: 5.0,
                    ..on_foot
                }
            ),
            Respond
        );
        // No route to the target: the crew goes on foot, once slow.
        let lost = CarSenses {
            distance: 90.0,
            no_route: true,
            ..on_foot
        };
        assert_eq!(step(Respond, lost), Dismounted);
        assert_eq!(step(Respond, CarSenses { speed: 5.0, ..lost }), Respond);
        // Never for a driver that is already gone (the crew would get back in the same tick).
        for reason in [
            lost,
            CarSenses {
                distance: 90.0,
                route_done: true,
                ..on_foot
            },
        ] {
            assert_eq!(
                step(
                    Respond,
                    CarSenses {
                        driving: true,
                        driven_off: true,
                        ..reason
                    }
                ),
                Respond,
                "{reason:?}"
            );
        }
        // Never in an intersection, whatever the reason to stop.
        for reason in [
            on_foot,
            stopped,
            lost,
            CarSenses {
                distance: 90.0,
                route_done: true,
                ..on_foot
            },
            CarSenses {
                distance: 90.0,
                blocked: true,
                ..on_foot
            },
        ] {
            assert_eq!(
                step(
                    Respond,
                    CarSenses {
                        in_junction: true,
                        ..reason
                    }
                ),
                Respond,
                "{reason:?}"
            );
        }
        // Held up inside an intersection for blocked_seconds x junction_factor: out there after all.
        assert_eq!(
            step(
                Respond,
                CarSenses {
                    distance: 90.0,
                    blocked: true,
                    in_junction: true,
                    stuck_in_junction: true,
                    ..on_foot
                }
            ),
            Dismounted
        );
        // A driver: nobody gets out, for any reason, until his car has stood stopped_seconds.
        for reason in [
            CarSenses {
                distance: 90.0,
                blocked: true,
                ..on_foot
            },
            CarSenses {
                distance: 90.0,
                route_done: true,
                ..on_foot
            },
            lost,
            on_foot,
        ] {
            let driver = CarSenses {
                driving: true,
                player_stopped: 0.5,
                ..reason
            };
            assert_eq!(step(Respond, driver), Respond, "{driver:?}");
            let stood = CarSenses {
                player_stopped: 1.0,
                ..driver
            };
            assert_eq!(step(Respond, stood), Dismounted, "{stood:?}");
        }
        // Dismounted: back to Respond with crew aboard once nobody is outside or the timeout ran out.
        let out = CarSenses {
            crew_aboard: 0,
            crew_outside: 2,
            ..senses()
        };
        assert_eq!(step(Dismounted, out), Dismounted);
        assert_eq!(
            step(
                Dismounted,
                CarSenses {
                    crew_aboard: 1,
                    crew_outside: 1,
                    ..out
                }
            ),
            Dismounted
        );
        assert_eq!(
            step(
                Dismounted,
                CarSenses {
                    crew_aboard: 2,
                    crew_outside: 0,
                    ..out
                }
            ),
            Respond
        );
        assert_eq!(
            step(
                Dismounted,
                CarSenses {
                    crew_aboard: 1,
                    crew_outside: 1,
                    reboard_left: 0.0,
                    ..out
                }
            ),
            Respond
        );
        // Nobody aboard and nobody alive outside: abandoned.
        assert_eq!(
            step(
                Dismounted,
                CarSenses {
                    crew_aboard: 0,
                    crew_outside: 0,
                    ..out
                }
            ),
            Abandoned
        );
    }

    /// Driven off only after the player's car moved `moving_seconds`, and only beyond
    /// `reboard_distance`; a stopped (or stop-and-go) driver never is.
    #[test]
    fn driven_off_needs_moving_seconds() {
        let c = cfg();
        let car = |moving: f32| PoliceCar {
            state: Dismounted,
            crew: vec![],
            stopped: 0.0,
            moving,
            blocked: 0.0,
            reboard_left: 10.0,
        };
        let far = c.reboard_distance + 1.0;
        assert!(car(c.moving_seconds).driven_off(&c, true, far));
        assert!(!car(c.moving_seconds * 0.9).driven_off(&c, true, far));
        assert!(!car(c.moving_seconds).driven_off(&c, true, c.reboard_distance - 1.0));
        assert!(!car(c.moving_seconds).driven_off(&c, false, far));
    }
}
