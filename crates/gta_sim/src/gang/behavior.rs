//! Gang systems: heat, the player's territory, provocation and group aggro, the member FSM, death.

mod fire_line;

use super::fsm::{Focus, Move, Senses, Tactic, band_move, choose_tactic, next_state};
use super::{
    Faction, GangConfig, GangHeat, GangMember, GangRng, GangState, GangTerritories,
    PlayerTerritory, roll,
};
use crate::character::{
    ActionIntent, AimIntent, Character, CharacterBody, Dead, Gait, Health, HealthConfig,
    LocomotionConfig, MoveIntent, WeaponRequest,
};
use crate::combat::{
    DamageDealt, GunSlot, Loadout, ShotFired, Weapon, WeaponsConfig, cone_sample, dropped_gun,
};
use crate::navigation::{
    NavigationConfig, Route, RouteLoad, SidewalkGraph, avoid_offset, flat_distance, nearest_node,
    plan_route, route_point, steer,
};
use crate::perception::{AiClock, Perception, PerceptionConfig, sight_blocked};
use crate::player::Player;
use crate::population::corpse_components;
use avian3d::prelude::*;
use bevy::prelude::*;
use fire_line::{Blocked, FireLine, Shooter, unblock};

pub(super) fn decay_gang_heat(time: Res<Time<Fixed>>, mut heat: ResMut<GangHeat>) {
    let dt = time.delta_secs();
    for left in &mut heat.left {
        *left = (*left - dt).max(0.0);
    }
}

pub(super) fn locate_player(
    territories: Res<GangTerritories>,
    player: Query<&Position, (With<Player>, Without<Dead>)>,
    mut territory: ResMut<PlayerTerritory>,
) {
    let turf = player
        .single()
        .ok()
        .and_then(|p| territories.territory_at(p.0));
    if territory.0 != turf {
        territory.0 = turf;
    }
}

/// A hostile hit on a member, or a hostile shot near a member inside its territory, puts every idle
/// or warning member of that gang within `group_radius` of the provoked one into `Attack`.
#[allow(clippy::too_many_arguments)]
pub(super) fn provoke_gangs(
    cfg: Res<GangConfig>,
    territories: Res<GangTerritories>,
    mut heat: ResMut<GangHeat>,
    mut rng: ResMut<GangRng>,
    mut shots: MessageReader<ShotFired>,
    mut damage: MessageReader<DamageDealt>,
    factions: Query<&Faction>,
    positions: Query<&Position>,
    mut members: Query<(&Position, &mut GangMember)>,
) {
    let h = &cfg.hostility;
    // (gang, provoked at, attacker)
    let mut provocations: Vec<(u8, Vec3, Entity)> = Vec::new();
    for hit in damage.read() {
        let Ok((position, member)) = members.get(hit.target) else {
            continue;
        };
        let Ok(&attacker) = factions.get(hit.shooter) else {
            continue;
        };
        if cfg.hostile(Faction::Gang(member.gang), attacker) {
            provocations.push((member.gang, position.0, hit.shooter));
        }
    }
    for shot in shots.read() {
        let Ok(&shooter) = factions.get(shot.shooter) else {
            continue;
        };
        let turf = territories.territory_at(shot.muzzle);
        for (position, member) in &members {
            if member.state == GangState::Dead
                || turf != Some(member.gang)
                || position.0.distance(shot.muzzle) > h.shot_radius
                || !cfg.hostile(Faction::Gang(member.gang), shooter)
            {
                continue;
            }
            provocations.push((member.gang, position.0, shot.shooter));
        }
    }
    for (gang, at, attacker) in provocations {
        if factions.get(attacker) == Ok(&Faction::Player) {
            heat.left[gang as usize] = h.heat_seconds;
        }
        let seen = positions.get(attacker).map_or(at, |p| p.0);
        for (position, mut member) in &mut members {
            if member.gang != gang
                || !matches!(member.state, GangState::Idle | GangState::Warn)
                || flat_distance(position.0, at) > h.group_radius
            {
                continue;
            }
            member.state = GangState::Attack { target: attacker };
            member.last_seen = seen;
            member.sees = false;
            member.sees_focus = None;
            member.trigger_left = roll(&mut rng, cfg.combat.trigger_seconds);
        }
    }
}

/// A character a member may focus on: chest position and whether it is alive.
#[derive(Clone, Copy)]
struct Body {
    chest: Vec3,
    alive: bool,
}

/// Shared read-only inputs of one `gang_fsm` run.
struct Ctx<'a, 'w, 's> {
    nav: &'a NavigationConfig,
    graph: &'a SidewalkGraph,
    spatial: &'a SpatialQuery<'w, 's>,
    dt: f32,
}

/// Where a member walks this tick: `dest` straight (with wall avoidance) or along a graph route.
struct Seek {
    dest: Vec3,
    gait: Gait,
    direct: bool,
}

/// How a member moves this tick.
enum Motion {
    Stand,
    /// Walk along a move yaw (`None`: stand).
    Yaw(Option<f32>, Gait),
    Seek(Seek),
}

/// Move yaw towards `seek.dest`: direct seek plus the avoidance offset refreshed on the slot, or the
/// next point of a route re-planned within the tick's search budget; direct when no route is usable.
fn head_for(
    ctx: &Ctx,
    seek: &Seek,
    chest: Vec3,
    on_slot: bool,
    member: &mut GangMember,
    route: &mut Route,
    load: &mut RouteLoad,
) -> Option<f32> {
    route.age += ctx.dt;
    let straight = steer(chest, seek.dest)?;
    if seek.direct {
        route.nodes.clear();
        if on_slot {
            member.avoid = avoid_offset(ctx.spatial, chest, straight, ctx.nav, &mut load.rays);
        }
        return Some(straight + member.avoid);
    }
    member.avoid = 0.0;
    let Some(goal) = nearest_node(ctx.graph, seek.dest) else {
        return Some(straight);
    };
    let stale = route.nodes.is_empty()
        || route.goal != Some(goal)
        || route.age >= ctx.nav.route_refresh_seconds;
    if stale && load.searches < ctx.nav.route_requests_per_tick {
        load.searches += 1;
        plan_route(ctx.graph, route, chest, goal);
    }
    if route.nodes.is_empty() {
        return Some(straight);
    }
    let point = route_point(ctx.graph, route, chest, seek.dest, ctx.nav.arrive_radius);
    steer(chest, point).or(Some(straight))
}

fn walk(intent: &mut MoveIntent, yaw: Option<f32>, gait: Gait) {
    let Some(yaw) = yaw else {
        intent.axis = Vec2::ZERO;
        return;
    };
    intent.axis = Vec2::Y;
    intent.yaw = yaw;
    intent.gait = gait;
}

/// Pulls the trigger (or punches) when the cooldown is over; the aim direction gets the gang's error cone.
fn pull(
    member: &mut GangMember,
    aim: &mut AimIntent,
    action: &mut ActionIntent,
    rng: &mut GangRng,
    cfg: &GangConfig,
) {
    member.trigger_left = roll(rng, cfg.combat.trigger_seconds);
    action.fire_requested = true;
    let Ok(dir) = Dir3::new(aim.direction) else {
        return;
    };
    let (u, v) = (rng.unit(), rng.unit());
    aim.direction = cone_sample(dir, cfg.combat.aim_error_deg.to_radians(), u, v).as_vec3();
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn gang_fsm(
    configs: (
        Res<GangConfig>,
        Res<PerceptionConfig>,
        Res<LocomotionConfig>,
        Res<NavigationConfig>,
        Res<WeaponsConfig>,
        Res<HealthConfig>,
    ),
    state: (
        Res<AiClock>,
        Res<GangHeat>,
        Res<PlayerTerritory>,
        Res<SidewalkGraph>,
    ),
    mut rng: ResMut<GangRng>,
    mut load: ResMut<RouteLoad>,
    time: Res<Time<Fixed>>,
    spatial: SpatialQuery,
    mut members: Query<(
        Entity,
        &mut GangMember,
        &mut Route,
        &Perception,
        &Position,
        &Health,
        &Loadout,
        &mut MoveIntent,
        &mut AimIntent,
        &mut ActionIntent,
    )>,
    player: Query<
        (Entity, &Position, &AimIntent, &Loadout, Has<Dead>),
        (With<Player>, Without<GangMember>),
    >,
    bodies: Query<(&Position, Has<Dead>), Without<GangMember>>,
    characters: Query<(Entity, &Position, &Health, Option<&Faction>), With<Character>>,
) {
    let (cfg, perception, loco, nav, weapons, health_cfg) = configs;
    let (clock, heat, territory, graph) = state;
    let h = &cfg.hostility;
    let c = &cfg.combat;
    let ctx = Ctx {
        nav: &nav,
        graph: &graph,
        spatial: &spatial,
        dt: time.delta_secs(),
    };
    let slot = (clock.tick % u64::from(perception.slots)) as u8;
    let aim_cone = perception.aimed_cone_deg.to_radians();
    let others: Vec<(Entity, Body)> = members
        .iter()
        .map(|(e, m, _, _, p, ..)| {
            (
                e,
                Body {
                    chest: p.0,
                    alive: m.state != GangState::Dead,
                },
            )
        })
        .collect();
    let body = |e: Entity| -> Option<Body> {
        if let Some(&(_, b)) = others.iter().find(|(o, _)| *o == e) {
            return Some(b);
        }
        bodies.get(e).ok().map(|(p, dead)| Body {
            chest: p.0,
            alive: !dead,
        })
    };
    let live_player = player.single().ok().filter(|p| !p.4);
    let living: Vec<(Entity, Vec3, Option<Faction>)> = characters
        .iter()
        .filter(|(.., health, _)| health.current > 0.0)
        .map(|(e, p, _, f)| (e, p.0, f.copied()))
        .collect();
    let radius = loco.capsule_radius;
    let clearance = radius + c.fire_line_margin;
    let shooters: Vec<Shooter> = members
        .iter()
        .filter_map(|(entity, m, _, _, position, _, loadout, ..)| {
            let (GangState::Attack { target } | GangState::Retreat { from: target }) = m.state
            else {
                return None;
            };
            let to = body(target).filter(|b| b.alive)?.chest;
            (m.sees && loadout.held == Some(m.gun)).then(|| Shooter {
                entity,
                gang: m.gang,
                chest: position.0,
                target,
                to,
                line: FireLine::of(c, &weapons, m.gun, loadout, clearance),
            })
        })
        .collect();
    for (
        me,
        mut member,
        mut route,
        perception_slot,
        position,
        health,
        loadout,
        mut intent,
        mut aim,
        mut action,
    ) in &mut members
    {
        if member.state == GangState::Dead {
            continue;
        }
        let member = &mut *member;
        let chest = position.0;
        let feet = chest - Vec3::Y * loco.float_height;
        let eyes = feet + Vec3::Y * loco.head_height;
        let on_slot = perception_slot.slot == slot;
        let in_turf = territory.0 == Some(member.gang);

        // 1. Dwell of the player close by inside the territory.
        let near_player = live_player
            .filter(|p| in_turf && flat_distance(p.1.0, chest) <= h.warn_distance)
            .is_some();
        member.dwell = if near_player {
            member.dwell + ctx.dt
        } else {
            0.0
        };

        // 2. Senses: line of sight to the focus on the slot (or at once for a new focus).
        let focus_entity = match member.state {
            GangState::Attack { target } => Some(target),
            GangState::Retreat { from } => Some(from),
            _ => live_player.map(|p| p.0),
        };
        let focus = focus_entity.and_then(|e| body(e).filter(|b| b.alive).map(|b| (e, b)));
        let mut aimed_at = false;
        if on_slot || member.sees_focus != focus_entity {
            member.sees_focus = focus_entity;
            member.sees = focus.is_some_and(|(_, b)| {
                flat_distance(chest, b.chest) <= h.sight_distance
                    && !sight_blocked(&spatial, eyes, b.chest)
            });
            if let (true, Some((_, b))) = (member.sees, focus) {
                member.last_seen = b.chest;
            }
        }
        if on_slot {
            aimed_at = live_player.is_some_and(|(_, p, player_aim, player_loadout, _)| {
                player_aim.aiming
                    && player_loadout.held.is_some()
                    && flat_distance(p.0, chest) <= perception.aimed_distance
                    && player_aim
                        .direction
                        .angle_between(chest - player_aim.origin)
                        <= aim_cone
                    && member.sees
                    && member.sees_focus == live_player.map(|p| p.0)
            });
            if flat_distance(feet, member.spot) > nav.arrive_radius {
                member.home_clear =
                    !sight_blocked(&spatial, chest, member.spot + Vec3::Y * loco.float_height);
            }
        }

        // 3. Tactic (meaningful only in Attack).
        let target = focus.filter(|_| {
            matches!(
                member.state,
                GangState::Attack { .. } | GangState::Retreat { .. }
            )
        });
        let distance = target.map_or(f32::INFINITY, |(_, b)| flat_distance(chest, b.chest));
        let slot_ammo: &GunSlot = &loadout.guns[member.gun.index()];
        let has_ammo = slot_ammo.magazine + slot_ammo.reserve > 0;
        let in_range = distance <= weapons.stats(member.gun).range;
        let tactic = choose_tactic(
            health.current / health_cfg.max_health,
            distance,
            member.sees,
            has_ammo,
            loadout.held.is_none(),
            in_range,
            c,
        );

        // 4. Transition.
        let focus_of = |at: Vec3, visible: bool| Focus {
            distance: flat_distance(chest, at),
            from_post: flat_distance(at, member.spot),
            visible,
        };
        let player_focus = live_player
            .map(|(e, p, ..)| focus_of(p.0, member.sees && member.sees_focus == Some(e)));
        let senses = Senses {
            heat: heat.left[member.gang as usize],
            player: player_focus,
            player_in_turf: in_turf,
            dwell: member.dwell,
            aimed_at,
            target: target.map(|(_, b)| focus_of(b.chest, member.sees)),
            target_is_player: target.is_some_and(|(e, _)| Some(e) == live_player.map(|p| p.0)),
            tactic,
        };
        let before = member.state;
        member.state = next_state(before, live_player.map(|p| p.0), &senses, h);
        if matches!(member.state, GangState::Attack { .. })
            && !matches!(before, GangState::Attack { .. })
        {
            member.trigger_left = roll(&mut rng, c.trigger_seconds);
        }

        // 5. Trigger cooldown.
        if matches!(
            member.state,
            GangState::Attack { .. } | GangState::Retreat { .. }
        ) {
            member.trigger_left = (member.trigger_left - ctx.dt).max(0.0);
        }

        // 6. Intents; `fire_requested` is only ever set here, the weapon systems take it.
        let aim_from_eyes = |at: Vec3| AimIntent {
            origin: eyes,
            direction: at - eyes,
            aiming: true,
        };
        let seek = |dest: Vec3, gait: Gait, direct: bool| Motion::Seek(Seek { dest, gait, direct });
        let armed_pull = member.trigger_left == 0.0
            && member.sees
            && in_range
            && loadout.held == Some(member.gun);

        // Hold fire while a groupmate or a bystander is in the line (a miss flies on to the weapon range)
        // and move to clear it.
        let line = FireLine::of(c, &weapons, member.gun, loadout, clearance);
        let gang = Faction::Gang(member.gang);
        let shooting = match member.state {
            GangState::Attack { .. } => matches!(tactic, Tactic::Shoot | Tactic::Retreat),
            GangState::Retreat { .. } => distance >= c.retreat_distance,
            _ => false,
        };
        let plan = member.reposition.take();
        // `line_blocked`: hold fire; `clearing`: how to move instead (`None`: the band below).
        let mut line_blocked = false;
        let mut clearing = None;
        if let (true, Some((target_entity, at))) = (shooting && member.sees && in_range, target) {
            let others: Vec<(Entity, Vec3, Option<Faction>)> =
                living.iter().filter(|b| b.0 != me).copied().collect();
            // A spared body pressed against the target (a brawling groupmate, a human shield) yields
            // the target to the fight: no line past it opens by closing in.
            let (shields, yielding): (Vec<Vec3>, Vec<bool>) = others
                .iter()
                .filter(|&&(e, _, f)| e != target_entity && cfg.spares(Some(gang), f))
                .map(|&(_, p, _)| (p, flat_distance(p, at.chest) <= c.melee_distance.1))
                .unzip();
            if line.blocked(chest, at.chest, &shields) {
                line_blocked = true;
                // Two members blocking each other: the lower index moves, this one holds.
                let yields = shooters.iter().any(|s| {
                    s.entity.index_u32() < me.index_u32()
                        && s.target != me
                        && s.entity != target_entity
                        && cfg.spares(Some(gang), Some(Faction::Gang(s.gang)))
                        && line.blocked(chest, at.chest, &[s.chest])
                        && s.line.blocked(s.chest, s.to, &[chest])
                });
                clearing = if yields {
                    Some(Motion::Stand)
                } else {
                    let all: Vec<Vec3> = others.iter().map(|b| b.1).collect();
                    let blocked = Blocked {
                        chest,
                        to: at.chest,
                        line,
                        shields: &shields,
                        yielding: &yielding,
                        bodies: &all,
                    };
                    let (spot, motion) =
                        unblock(&ctx, &blocked, plan, on_slot, c, radius, &mut load.rays);
                    member.reposition = spot;
                    motion
                };
            }
        }
        let (want, motion) = match (member.state, target) {
            (GangState::Warn, _) => {
                let Some((_, p, ..)) = live_player else {
                    continue;
                };
                *aim = aim_from_eyes(p.0);
                let d = flat_distance(chest, p.0);
                let motion = if d > h.warn_keep_distance {
                    seek(
                        p.0,
                        c.warn_gait,
                        member.sees && d <= nav.direct_seek_distance,
                    )
                } else {
                    Motion::Stand
                };
                (Some(member.gun), motion)
            }
            (GangState::Attack { .. }, Some((_, at))) => match tactic {
                Tactic::Shoot | Tactic::Retreat => {
                    *aim = aim_from_eyes(at.chest);
                    if armed_pull && !line_blocked {
                        pull(member, &mut aim, &mut action, &mut rng, &cfg);
                    }
                    let motion =
                        clearing.unwrap_or_else(|| match band_move(distance, c.keep_distance) {
                            Move::Approach => seek(
                                at.chest,
                                c.chase_gait,
                                member.sees && distance <= nav.direct_seek_distance,
                            ),
                            Move::BackOff => Motion::Yaw(steer(at.chest, chest), c.back_off_gait),
                            Move::Hold => Motion::Stand,
                        });
                    (Some(member.gun), motion)
                }
                Tactic::Melee => {
                    *aim = AimIntent {
                        direction: (at.chest - eyes).with_y(0.0),
                        ..aim_from_eyes(at.chest)
                    };
                    if member.trigger_left == 0.0 && loadout.held.is_none() {
                        pull(member, &mut aim, &mut action, &mut rng, &cfg);
                    }
                    let motion = if distance > c.melee_distance.0 {
                        seek(at.chest, c.chase_gait, true)
                    } else {
                        Motion::Stand
                    };
                    (None, motion)
                }
                Tactic::Chase => {
                    let last = member.last_seen;
                    *aim = aim_from_eyes(last);
                    let d = flat_distance(chest, last);
                    let motion = if d > nav.arrive_radius {
                        seek(
                            last,
                            c.chase_gait,
                            member.sees && d <= nav.direct_seek_distance,
                        )
                    } else {
                        Motion::Stand
                    };
                    (has_ammo.then_some(member.gun), motion)
                }
            },
            (GangState::Retreat { .. }, Some((_, at))) => {
                let motion = if distance < c.retreat_distance {
                    aim.aiming = false;
                    let yaw = steer(at.chest, chest).map(|yaw| {
                        if on_slot {
                            member.avoid = avoid_offset(&spatial, chest, yaw, &nav, &mut load.rays);
                        }
                        yaw + member.avoid
                    });
                    Motion::Yaw(yaw, c.retreat_gait)
                } else {
                    *aim = aim_from_eyes(at.chest);
                    if armed_pull && !line_blocked {
                        pull(member, &mut aim, &mut action, &mut rng, &cfg);
                    }
                    clearing.unwrap_or(Motion::Stand)
                };
                (Some(member.gun), motion)
            }
            // Idle, or a fight whose target is gone this tick.
            _ => {
                aim.aiming = false;
                let home = flat_distance(feet, member.spot);
                let motion = if home > nav.arrive_radius {
                    seek(
                        member.spot + Vec3::Y * loco.float_height,
                        c.warn_gait,
                        member.home_clear && home <= nav.direct_seek_distance,
                    )
                } else {
                    Motion::Stand
                };
                (None, motion)
            }
        };
        select(&mut action, loadout, want);
        match motion {
            Motion::Stand => intent.axis = Vec2::ZERO,
            Motion::Yaw(yaw, gait) => walk(&mut intent, yaw, gait),
            Motion::Seek(s) => {
                let yaw = head_for(&ctx, &s, chest, on_slot, member, &mut route, &mut load);
                walk(&mut intent, yaw, s.gait);
            }
        }
    }
}

/// Requests the wanted weapon only when it differs from the held one (never `Unarmed` while unarmed).
fn select(action: &mut ActionIntent, loadout: &Loadout, want: Option<Weapon>) {
    if loadout.held == want {
        return;
    }
    action.select = Some(match want {
        Some(gun) => WeaponRequest::Gun(gun),
        None => WeaponRequest::Unarmed,
    });
}

/// A member at 0 health becomes a corpse and drops its gun (GDD §6.3).
#[allow(clippy::type_complexity)]
pub(super) fn gang_death(
    mut commands: Commands,
    weapons: Res<WeaponsConfig>,
    mut members: Query<(
        Entity,
        &Health,
        &Position,
        &CharacterBody,
        &mut GangMember,
        &mut MoveIntent,
        &mut AimIntent,
        &mut ActionIntent,
        &mut Loadout,
    )>,
) {
    for (
        entity,
        health,
        position,
        body,
        mut member,
        mut intent,
        mut aim,
        mut action,
        mut loadout,
    ) in &mut members
    {
        if member.state == GangState::Dead || health.current > 0.0 {
            continue;
        }
        member.state = GangState::Dead;
        intent.axis = Vec2::ZERO;
        aim.aiming = false;
        action.fire_requested = false;
        let gun = member.gun;
        loadout.held = None;
        loadout.guns[gun.index()] = GunSlot::default();
        loadout.reload_left = 0.0;
        commands.entity(entity).insert(corpse_components());
        let feet = position.0 - Vec3::Y * body.float_height;
        commands.spawn(dropped_gun(gun, feet, &weapons));
    }
}
