//! Cop systems: the hostility alert, the cop FSM and death.

use super::fsm::{CopSenses, next_state};
use super::{
    CopState, EscalationConfig, PoliceAlert, PoliceCombatConfig, PoliceRng, PoliceUnit, roll,
};
use crate::character::{
    ActionIntent, AimIntent, Character, CharacterBody, Dead, Gait, Health, LocomotionConfig,
    MoveIntent,
};
use crate::combat::{
    AimConfig, DamageDealt, GunSlot, Loadout, MeleeHit, ShotFired, WeaponsConfig, cone_sample,
    dropped_gun,
};
use crate::gang::Faction;
use crate::gang::fsm::{Move, band_move};
use crate::navigation::{NavigationConfig, Route, RouteLoad, SidewalkGraph, flat_distance, steer};
use crate::perception::{AiClock, Perception, PerceptionConfig, sight_blocked};
use crate::player::Player;
use crate::population::{PopulationConfig, corpse_components, spawn_points};
use crate::tactics::{
    Aim, Ctx, FireLine, Motion, Seek, Shooter, apply_motion, hold_fire, overreach, select,
};
use crate::wanted::{WantedConfig, WantedLevel, cop_sees, eye, witnesses};
use avian3d::prelude::*;
use bevy::prelude::*;

/// A player attack witnessed by a live cop, or any player hit on a cop, makes arrest-row cops shoot
/// for `arrest.hostile_seconds`.
#[allow(clippy::too_many_arguments)]
pub(super) fn police_alert(
    configs: (
        Res<EscalationConfig>,
        Res<WantedConfig>,
        Res<LocomotionConfig>,
    ),
    time: Res<Time<Fixed>>,
    spatial: SpatialQuery,
    mut alert: ResMut<PoliceAlert>,
    mut shots: MessageReader<ShotFired>,
    mut hits: MessageReader<MeleeHit>,
    mut dealt: MessageReader<DamageDealt>,
    player: Query<(Entity, &Position), With<Player>>,
    cops: Query<(&Position, &PoliceUnit)>,
) {
    let (esc, wanted_cfg, loco) = configs;
    alert.hostile_left = (alert.hostile_left - time.delta_secs()).max(0.0);
    let player = player.single().ok();
    let by_player = |e: Entity| player.is_some_and(|(p, _)| p == e);
    // Every message is read (a stopped iterator would leave the rest for the next tick).
    let attacks = shots.read().filter(|s| by_player(s.shooter)).count()
        + hits.read().filter(|h| by_player(h.attacker)).count();
    let cop_hit = dealt
        .read()
        .filter(|d| by_player(d.shooter) && cops.contains(d.target))
        .count()
        > 0;
    let Some((_, at)) = player else {
        return;
    };
    let offender = eye(at.0, &loco);
    let witnessed = attacks > 0
        && cops.iter().any(|(p, unit)| {
            unit.state != CopState::Dead
                && witnesses(&spatial, eye(p.0, &loco), offender, &wanted_cfg)
        });
    if cop_hit || witnessed {
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
    )>,
    player: Query<(Entity, &Position, Has<Dead>), (With<Player>, Without<PoliceUnit>)>,
    characters: Query<(Entity, &Position, &Health, Option<&Faction>), With<Character>>,
) {
    let (esc, wanted_cfg, perception, loco, nav, weapons, population, aim_cfg) = configs;
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
        .map(|(e, p, _)| (e, p.0));
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
                        ),
                    })
                })
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
    ) in &mut units
    {
        if unit.state == CopState::Dead {
            continue;
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
                )
            });
            unit.dest_clear = unit.dest.is_some_and(|d| !sight_blocked(&spatial, eyes, d));
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
                *aim = aim_from_eyes(at);
                let distance = flat_distance(chest, at);
                let motion = if distance > esc.arrest.stand_distance {
                    let direct = unit.sees && distance <= nav.direct_seek_distance;
                    seek(at, c.chase_gait, direct)
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
