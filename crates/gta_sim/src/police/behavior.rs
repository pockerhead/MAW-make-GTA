//! Cop systems: the hostility alert, the cop FSM and death.

use super::cars::{CrewOf, PoliceCar, PoliceCarState, nearest_door};
use super::fsm::{CopSenses, next_state};
use super::{
    CopState, EscalationConfig, PoliceAlert, PoliceCombatConfig, PoliceRng, PoliceUnit, roll,
};
use crate::character::{
    ActionIntent, AimIntent, Character, CharacterBody, Dead, Gait, Health, LocomotionConfig,
    MoveIntent,
};
use crate::combat::{
    AimConfig, BulletTrace, DamageDealt, DamageScale, GunSlot, Loadout, WeaponsConfig, cone_sample,
    dropped_gun,
};
use crate::gang::Faction;
use crate::gang::fsm::{Move, band_move};
use crate::navigation::{NavigationConfig, Route, RouteLoad, SidewalkGraph, flat_distance, steer};
use crate::perception::{AiClock, Perception, PerceptionConfig, wall_blocked};
use crate::player::Player;
use crate::population::{PopulationConfig, corpse_components, spawn_points};
use crate::tactics::{
    Aim, CarRect, Ctx, FireLine, Motion, Seek, Shooter, apply_motion, around_cars, hold_fire,
    nearby_cars, overreach, select,
};
use crate::vehicle::{Driving, Vehicle, VehicleConfig, door_point};
use crate::wanted::{WantedConfig, WantedLevel, cop_sees, eye};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Distance from `p` to the segment `a`-`b`.
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// A player hit on a cop, or a player bullet passing within `arrest.near_miss_distance` of a live
/// cop, makes arrest-row cops shoot for `arrest.hostile_seconds`; attacks on anyone else do not.
pub(super) fn police_alert(
    esc: Res<EscalationConfig>,
    time: Res<Time<Fixed>>,
    mut alert: ResMut<PoliceAlert>,
    mut traces: MessageReader<BulletTrace>,
    mut dealt: MessageReader<DamageDealt>,
    player: Query<Entity, With<Player>>,
    cops: Query<(&Position, &PoliceUnit)>,
) {
    alert.hostile_left = (alert.hostile_left - time.delta_secs()).max(0.0);
    let player = player.single().ok();
    let by_player = |e: Entity| player == Some(e);
    // Every message is read (a stopped iterator would leave the rest for the next tick).
    let cop_hit = dealt
        .read()
        .filter(|d| by_player(d.shooter) && cops.contains(d.target))
        .count()
        > 0;
    let near = esc.arrest.near_miss_distance;
    let shot_at = traces
        .read()
        .filter(|t| {
            by_player(t.shooter)
                && cops.iter().any(|(p, unit)| {
                    unit.state != CopState::Dead && segment_distance(p.0, t.from, t.to) <= near
                })
        })
        .count()
        > 0;
    if cop_hit || shot_at {
        alert.hostile_left = esc.arrest.hostile_seconds;
    }
}

/// Pulls the trigger; the aim direction gets the police error cone.
fn pull(
    unit: &mut PoliceUnit,
    aim: &mut AimIntent,
    action: &mut ActionIntent,
    rng: &mut PoliceRng,
    c: &PoliceCombatConfig,
) {
    unit.trigger_left = roll(rng, c.trigger_seconds);
    action.fire_requested = true;
    let Ok(dir) = Dir3::new(aim.direction) else {
        return;
    };
    let (u, v) = (rng.unit(), rng.unit());
    aim.direction = cone_sample(dir, c.aim_error_deg.to_radians(), u, v).as_vec3();
}

/// A point of the search circle around `centre` (chest height): a random sidewalk spawn point within
/// `radius`, else a random point of the disc.
fn search_point(
    graph: &SidewalkGraph,
    spacing: f32,
    centre: Vec3,
    radius: f32,
    float_height: f32,
    rng: &mut PoliceRng,
) -> Vec3 {
    let points: Vec<Vec3> = spawn_points(graph, spacing)
        .map(|p| p.at)
        .filter(|&p| flat_distance(p, centre) <= radius)
        .collect();
    if points.is_empty() {
        let (r, a) = (
            radius * rng.unit().sqrt(),
            std::f32::consts::TAU * rng.unit(),
        );
        return centre + Vec3::new(r * a.cos(), 0.0, r * a.sin());
    }
    points[rng.next_u32() as usize % points.len()] + Vec3::Y * float_height
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn police_fsm(
    configs: (
        Res<EscalationConfig>,
        Res<WantedConfig>,
        Res<PerceptionConfig>,
        Res<LocomotionConfig>,
        Res<NavigationConfig>,
        Res<WeaponsConfig>,
        Res<PopulationConfig>,
        Res<AimConfig>,
        Res<VehicleConfig>,
    ),
    state: (
        Res<AiClock>,
        Res<WantedLevel>,
        Res<PoliceAlert>,
        Res<SidewalkGraph>,
    ),
    mut rng: ResMut<PoliceRng>,
    mut load: ResMut<RouteLoad>,
    time: Res<Time<Fixed>>,
    spatial: SpatialQuery,
    mut units: Query<(
        Entity,
        &mut PoliceUnit,
        &mut Route,
        &Perception,
        (&Position, &Rotation),
        &Loadout,
        &mut MoveIntent,
        &mut AimIntent,
        &mut ActionIntent,
        Option<&CrewOf>,
        &mut DamageScale,
    )>,
    player: Query<
        (Entity, &Position, Has<Dead>, Option<&Driving>),
        (With<Player>, Without<PoliceUnit>),
    >,
    characters: Query<(Entity, &Position, &Health, Option<&Faction>), With<Character>>,
    cars: Query<
        (Entity, &Position, &Rotation, Option<&PoliceCar>),
        (With<Vehicle>, Without<PoliceUnit>),
    >,
) {
    let (esc, wanted_cfg, perception, loco, nav, weapons, population, aim_cfg, vehicle) = configs;
    let (clock, wanted, alert, graph) = state;
    let c = &esc.combat;
    let d = c.discipline();
    let ctx = Ctx {
        nav: &nav,
        graph: &graph,
        spatial: &spatial,
        dt: time.delta_secs(),
    };
    let slot = (clock.tick % u64::from(perception.slots)) as u8;
    let live_player = player
        .single()
        .ok()
        .filter(|p| !p.2)
        .map(|(e, p, ..)| (e, p.0));
    // The door of the player's car: arresting cops walk up to it.
    let player_door = player
        .single()
        .ok()
        .and_then(|p| p.3)
        .and_then(|d| cars.get(d.vehicle).ok())
        .map(|(_, p, r, _)| door_point(vehicle.door(), p.0, r.0));
    let living: Vec<(Entity, Vec3, Option<Faction>)> = characters
        .iter()
        .filter(|(.., health, _)| health.current > 0.0)
        .map(|(e, p, _, f)| (e, p.0, f.copied()))
        .collect();
    let radius = loco.capsule_radius;
    let clearance = radius + d.fire_line_margin;
    let overreach = overreach(&aim_cfg, &loco);
    let shooters: Vec<Shooter> = live_player
        .map(|(target, to)| {
            units
                .iter()
                .filter(|(_, u, ..)| u.state == CopState::Attack && u.sees)
                .filter_map(|(entity, u, _, _, (position, _), loadout, ..)| {
                    let gun = esc.spec(u.kind).gun;
                    (loadout.held == Some(gun)).then(|| Shooter {
                        entity,
                        faction: Faction::Police,
                        chest: position.0,
                        target,
                        to,
                        line: FireLine::of(
                            d.aim_error_deg,
                            &weapons,
                            gun,
                            loadout,
                            clearance,
                            overreach,
                            d.overshoot_margin,
                        ),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let player_car = player.single().ok().and_then(|p| p.3).map(|d| d.vehicle);
    let half = vehicle.half_extents();
    let near_cars = nearby_cars(
        &shooters,
        cars.iter().map(|(e, p, r, _)| (e, p.0, r.0)),
        player_car,
        Vec2::new(half.x, half.z),
    );
    // Cars around the arrest point (the driver's door, or the player): an arresting cop walks around
    // them (the wall avoidance does not see cars).
    let arrest_cars: Vec<CarRect> = player_door
        .or(live_player.map(|(_, at)| at))
        .map(|centre| {
            let reach = nav.direct_seek_distance + Vec2::new(half.x, half.z).length();
            cars.iter()
                .filter(|(_, p, ..)| flat_distance(p.0, centre) <= reach)
                .map(|(_, p, r, _)| CarRect::of(p.0, r.0, Vec2::new(half.x, half.z)))
                .collect()
        })
        .unwrap_or_default();
    let stars = wanted.stars;
    let row = (stars >= 1).then(|| &esc.stars[usize::from(stars) - 1]);
    let hostile = alert.hostile_left > 0.0;
    for (
        me,
        mut unit,
        mut route,
        perception_slot,
        (position, rotation),
        loadout,
        mut intent,
        mut aim,
        mut action,
        crew_of,
        mut scale,
    ) in &mut units
    {
        if unit.state == CopState::Dead {
            continue;
        }
        if let Some(row) = row
            && scale.0 != row.damage_scale
        {
            scale.0 = row.damage_scale;
        }
        let unit = &mut *unit;
        let chest = position.0;
        let feet = chest - Vec3::Y * loco.float_height;
        let eyes = feet + Vec3::Y * loco.head_height;
        let on_slot = perception_slot.slot == slot;
        let spec = esc.spec(unit.kind);

        // 1. Senses on the AI slot; a dead or absent player is never seen.
        if on_slot {
            unit.sees = live_player.is_some_and(|(_, p)| {
                cop_sees(
                    &spatial,
                    eyes,
                    rotation.0 * Vec3::NEG_Z,
                    eye(p, &loco),
                    &wanted_cfg,
                    wanted_cfg.cop_view_distance,
                )
            });
            unit.dest_clear = unit.dest.is_some_and(|d| !wall_blocked(&spatial, eyes, d));
        }
        if live_player.is_none() {
            unit.sees = false;
        }

        // 2. Transition, every tick.
        let goal = match unit.state {
            CopState::Respond => wanted.last_known,
            CopState::Search => unit.search_point,
            _ => None,
        };
        let senses = CopSenses {
            sees: unit.sees,
            hostile,
            arrest_row: row.is_some_and(|r| r.arrest),
            stars,
            at_goal: goal.is_some_and(|g| flat_distance(chest, g) <= esc.search_arrive_distance),
            near_driver: player_door
                .is_some_and(|door| flat_distance(chest, door) <= esc.arrest.approach_distance),
        };
        let before = unit.state;
        unit.state = next_state(before, &senses);
        if unit.state == CopState::Attack {
            if before != CopState::Attack {
                unit.trigger_left = roll(&mut rng, c.trigger_seconds);
            }
            unit.trigger_left = (unit.trigger_left - ctx.dt).max(0.0);
        }

        // 3. The search point: a new one on entering Search and on arriving at the current one.
        if unit.state == CopState::Search {
            let arrived = unit
                .search_point
                .is_none_or(|p| flat_distance(chest, p) <= esc.search_arrive_distance);
            if let (true, Some(centre)) = (before != CopState::Search || arrived, wanted.last_known)
            {
                let radius = wanted_cfg.stars[usize::from(stars.max(1)) - 1].search_radius;
                unit.search_point = Some(search_point(
                    &graph,
                    population.spawn_point_spacing,
                    centre,
                    radius,
                    loco.float_height,
                    &mut rng,
                ));
            }
        }

        // 3b. A cop of a parked police car walks back to its door once the player drove off.
        let door = crew_of
            .and_then(|c| cars.get(c.car).ok())
            .filter(|(_, p, _, car)| {
                car.is_some_and(|c| {
                    c.state == PoliceCarState::Dismounted
                        && live_player.is_some_and(|(_, at)| {
                            c.driven_off(&esc.car, player_door.is_some(), flat_distance(at, p.0))
                        })
                })
            })
            .map(|(_, p, r, _)| nearest_door(&vehicle, p.0, r.0, chest));
        if let Some(door) = door {
            aim.aiming = false;
            unit.dest = None;
            unit.reposition = None;
            select(&mut action, loadout, Some(spec.gun));
            let direct = flat_distance(chest, door) <= nav.direct_seek_distance;
            let motion = Motion::Seek(Seek {
                dest: door,
                gait: c.chase_gait,
                direct,
            });
            apply_motion(
                &ctx,
                motion,
                chest,
                on_slot,
                &mut unit.avoid,
                &mut route,
                &mut load,
                &mut intent,
            );
            continue;
        }

        // 4. Intents; `fire_requested` is only ever set here, the weapon systems take it.
        let aim_from_eyes = |at: Vec3| AimIntent {
            origin: eyes,
            direction: at - eyes,
            aiming: true,
        };
        let seek = |dest: Vec3, gait: Gait, direct: bool| Motion::Seek(Seek { dest, gait, direct });
        let plan = unit.reposition.take();
        let (want, dest, motion) = match (unit.state, live_player) {
            (CopState::Respond, _) => {
                aim.aiming = false;
                let dest = wanted.last_known;
                let motion = dest.map_or(Motion::Stand, |to| {
                    let direct =
                        unit.dest_clear && flat_distance(chest, to) <= nav.direct_seek_distance;
                    seek(to, c.chase_gait, direct)
                });
                (Some(spec.gun), dest, motion)
            }
            (CopState::Search, _) => {
                aim.aiming = false;
                let dest = unit.search_point;
                let motion = dest.map_or(Motion::Stand, |to| {
                    let direct =
                        unit.dest_clear && flat_distance(chest, to) <= nav.direct_seek_distance;
                    seek(to, c.search_gait, direct)
                });
                (Some(spec.gun), dest, motion)
            }
            (CopState::Arrest, Some((_, at))) => {
                // A driver is arrested at the car's door: the cop faces it, where he comes out.
                let at = player_door.unwrap_or(at);
                *aim = aim_from_eyes(at);
                let distance = flat_distance(chest, at);
                let motion = if distance > esc.arrest.stand_distance {
                    // Unseen near a driver's door: straight on while no wall is in the way.
                    let direct =
                        (unit.sees || unit.dest_clear) && distance <= nav.direct_seek_distance;
                    let via = if direct {
                        around_cars(chest, at, &arrest_cars, radius, radius + nav.arrive_radius)
                    } else {
                        at
                    };
                    seek(via, c.chase_gait, direct)
                } else {
                    Motion::Stand
                };
                (Some(spec.gun), Some(at), motion)
            }
            (CopState::Attack, Some((target, at))) => {
                *aim = aim_from_eyes(at);
                let distance = flat_distance(chest, at);
                let in_range = distance <= weapons.stats(spec.gun).range;
                let line = FireLine::of(
                    d.aim_error_deg,
                    &weapons,
                    spec.gun,
                    loadout,
                    clearance,
                    overreach,
                    d.overshoot_margin,
                );
                let mut line_blocked = false;
                let mut clearing = None;
                if unit.sees && in_range {
                    let hold = hold_fire(
                        &ctx,
                        me,
                        chest,
                        Aim {
                            entity: target,
                            chest: at,
                        },
                        line,
                        &living,
                        &shooters,
                        &near_cars,
                        |f| f != Some(Faction::Player),
                        plan,
                        on_slot,
                        &d,
                        radius,
                        &mut load.rays,
                    );
                    line_blocked = hold.line_blocked;
                    clearing = hold.clearing;
                    unit.reposition = hold.kept_spot;
                }
                let armed = unit.trigger_left == 0.0
                    && unit.sees
                    && in_range
                    && loadout.held == Some(spec.gun);
                if armed && !line_blocked {
                    pull(unit, &mut aim, &mut action, &mut rng, c);
                }
                let motion =
                    clearing.unwrap_or_else(|| match band_move(distance, spec.keep_distance) {
                        Move::Approach => seek(
                            at,
                            c.chase_gait,
                            unit.sees && distance <= nav.direct_seek_distance,
                        ),
                        Move::BackOff => Motion::Yaw(steer(at, chest), c.reposition_gait),
                        Move::Hold => Motion::Stand,
                    });
                (Some(spec.gun), Some(at), motion)
            }
            (CopState::Leave, player) => {
                aim.aiming = false;
                let motion = player.map_or(Motion::Stand, |(_, at)| {
                    Motion::Yaw(steer(at, chest), c.leave_gait)
                });
                (None, None, motion)
            }
            // Arrest or Attack with the player gone this tick: the transition catches up next tick.
            _ => {
                aim.aiming = false;
                (Some(spec.gun), None, Motion::Stand)
            }
        };
        unit.dest = dest;
        select(&mut action, loadout, want);
        apply_motion(
            &ctx,
            motion,
            chest,
            on_slot,
            &mut unit.avoid,
            &mut route,
            &mut load,
            &mut intent,
        );
    }
}

/// A cop at 0 health becomes a corpse and drops its gun.
#[allow(clippy::type_complexity)]
pub(super) fn police_death(
    mut commands: Commands,
    esc: Res<EscalationConfig>,
    weapons: Res<WeaponsConfig>,
    mut units: Query<(
        Entity,
        &Health,
        &Position,
        &CharacterBody,
        &mut PoliceUnit,
        &mut MoveIntent,
        &mut AimIntent,
        &mut ActionIntent,
        &mut Loadout,
    )>,
) {
    for (entity, health, position, body, mut unit, mut intent, mut aim, mut action, mut loadout) in
        &mut units
    {
        if unit.state == CopState::Dead || health.current > 0.0 {
            continue;
        }
        unit.state = CopState::Dead;
        intent.axis = Vec2::ZERO;
        aim.aiming = false;
        action.fire_requested = false;
        let gun = esc.spec(unit.kind).gun;
        loadout.held = None;
        loadout.guns[gun.index()] = GunSlot::default();
        loadout.reload_left = 0.0;
        commands.entity(entity).insert(corpse_components());
        let feet = position.0 - Vec3::Y * body.float_height;
        commands.spawn(dropped_gun(gun, feet, &weapons));
    }
}
