//! Shared NPC gunfight layer: walking to a point, weapon selection and the fire-line discipline of
//! armed NPCs (gangs, police).

mod fire_line;

pub(crate) use fire_line::{Blocked, FireLine, Shooter, overreach, unblock};

use crate::character::{ActionIntent, Gait, MoveIntent, WeaponRequest};
use crate::combat::{Loadout, Weapon};
use crate::gang::Faction;
use crate::navigation::{
    NavigationConfig, Route, RouteLoad, SidewalkGraph, avoid_offset, flat_distance, nearest_node,
    plan_route, route_point, steer,
};
use avian3d::prelude::*;
use bevy::prelude::*;

/// Shared read-only inputs of one NPC decision run.
pub(crate) struct Ctx<'a, 'w, 's> {
    pub(crate) nav: &'a NavigationConfig,
    pub(crate) graph: &'a SidewalkGraph,
    pub(crate) spatial: &'a SpatialQuery<'w, 's>,
    pub(crate) dt: f32,
}

/// Where an NPC walks this tick: `dest` straight (with wall avoidance) or along a graph route.
pub(crate) struct Seek {
    pub(crate) dest: Vec3,
    pub(crate) gait: Gait,
    pub(crate) direct: bool,
}

/// How an NPC moves this tick.
pub(crate) enum Motion {
    Stand,
    /// Walk along a move yaw (`None`: stand).
    Yaw(Option<f32>, Gait),
    Seek(Seek),
}

/// Fire-line tuning of one armed role.
pub(crate) struct Discipline<'a> {
    /// Cone added on top of the weapon spread, degrees.
    pub(crate) aim_error_deg: f32,
    pub(crate) fire_line_margin: f32,
    /// A spared body this close to the target yields the target to the fight, m.
    pub(crate) pressed_distance: f32,
    pub(crate) reposition_offsets: &'a [f32],
    pub(crate) reposition_step: f32,
    pub(crate) reposition_gait: Gait,
    pub(crate) chase_gait: Gait,
}

/// Move yaw towards `seek.dest`: direct seek plus the avoidance offset refreshed on the slot, or the
/// next point of a route re-planned within the tick's search budget, then straight on with avoidance;
/// direct when no route is usable.
pub(crate) fn head_for(
    ctx: &Ctx,
    seek: &Seek,
    chest: Vec3,
    on_slot: bool,
    avoid: &mut f32,
    route: &mut Route,
    load: &mut RouteLoad,
) -> Option<f32> {
    route.age += ctx.dt;
    let straight = steer(chest, seek.dest)?;
    if seek.direct {
        route.nodes.clear();
        if on_slot {
            *avoid = avoid_offset(ctx.spatial, chest, straight, ctx.nav, &mut load.rays);
        }
        return Some(straight + *avoid);
    }
    let Some(goal) = nearest_node(ctx.graph, seek.dest) else {
        *avoid = 0.0;
        return Some(straight);
    };
    // A route walked to its end is not re-planned by age while the walker is still nearest to its goal
    // node: from there it heads straight on, and a re-plan would send it back to that node every
    // refresh. A walker pushed out of the goal node's region (a sidestep, a retreat) re-plans again.
    let refresh = route.age >= ctx.nav.route_refresh_seconds
        && !(route.next >= route.nodes.len() && nearest_node(ctx.graph, chest) == Some(goal));
    let stale = route.nodes.is_empty() || route.goal != Some(goal) || refresh;
    if stale && load.searches < ctx.nav.route_requests_per_tick {
        load.searches += 1;
        plan_route(ctx.graph, route, chest, goal);
    }
    if route.nodes.is_empty() {
        *avoid = 0.0;
        return Some(straight);
    }
    let point = route_point(ctx.graph, route, chest, seek.dest, ctx.nav.arrive_radius);
    if route.next < route.nodes.len() {
        *avoid = 0.0;
        return steer(chest, point).or(Some(straight));
    }
    // Past the last node the walker heads straight for `dest`, with the wall avoidance of a direct seek.
    if on_slot {
        *avoid = avoid_offset(ctx.spatial, chest, straight, ctx.nav, &mut load.rays);
    }
    Some(straight + *avoid)
}

pub(crate) fn walk(intent: &mut MoveIntent, yaw: Option<f32>, gait: Gait) {
    let Some(yaw) = yaw else {
        intent.axis = Vec2::ZERO;
        return;
    };
    intent.axis = Vec2::Y;
    intent.yaw = yaw;
    intent.gait = gait;
}

/// Turns `motion` into the move intent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_motion(
    ctx: &Ctx,
    motion: Motion,
    chest: Vec3,
    on_slot: bool,
    avoid: &mut f32,
    route: &mut Route,
    load: &mut RouteLoad,
    intent: &mut MoveIntent,
) {
    match motion {
        Motion::Stand => intent.axis = Vec2::ZERO,
        Motion::Yaw(yaw, gait) => walk(intent, yaw, gait),
        Motion::Seek(s) => {
            let yaw = head_for(ctx, &s, chest, on_slot, avoid, route, load);
            walk(intent, yaw, s.gait);
        }
    }
}

/// Requests the wanted weapon only when it differs from the held one (never `Unarmed` while unarmed).
pub(crate) fn select(action: &mut ActionIntent, loadout: &Loadout, want: Option<Weapon>) {
    if loadout.held == want {
        return;
    }
    action.select = Some(match want {
        Some(gun) => WeaponRequest::Gun(gun),
        None => WeaponRequest::Unarmed,
    });
}

/// The shooter's target this tick: entity and chest.
#[derive(Clone, Copy)]
pub(crate) struct Aim {
    pub(crate) entity: Entity,
    pub(crate) chest: Vec3,
}

/// Outcome of `hold_fire`.
pub(crate) struct Hold {
    /// Hold fire this tick.
    pub(crate) line_blocked: bool,
    /// The clear spot to keep walking to (`None` while the line is clear or it yields).
    pub(crate) kept_spot: Option<Vec3>,
    /// How to move instead of the role's own motion (`None`: the role's own).
    pub(crate) clearing: Option<Motion>,
}

/// Hold fire while a spared body is in the line (a miss flies on to the weapon range) and move to
/// clear it. `living`: every live character; `spares(f)`: a body of faction `f` must not be hit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hold_fire(
    ctx: &Ctx,
    me: Entity,
    chest: Vec3,
    at: Aim,
    line: FireLine,
    living: &[(Entity, Vec3, Option<Faction>)],
    shooters: &[Shooter],
    spares: impl Fn(Option<Faction>) -> bool,
    plan: Option<Vec3>,
    on_slot: bool,
    d: &Discipline,
    radius: f32,
    rays: &mut u32,
) -> Hold {
    let mut hold = Hold {
        line_blocked: false,
        kept_spot: None,
        clearing: None,
    };
    let others: Vec<(Entity, Vec3, Option<Faction>)> =
        living.iter().filter(|b| b.0 != me).copied().collect();
    // A spared body pressed against the target (a brawling groupmate, a human shield) yields the
    // target to the fight: no line past it opens by closing in.
    let (shields, yielding): (Vec<Vec3>, Vec<bool>) = others
        .iter()
        .filter(|&&(e, _, f)| e != at.entity && spares(f))
        .map(|&(_, p, _)| (p, flat_distance(p, at.chest) <= d.pressed_distance))
        .unzip();
    if !line.blocked(chest, at.chest, &shields) {
        return hold;
    }
    hold.line_blocked = true;
    // Two shooters blocking each other: the lower index moves, this one holds.
    let yields = shooters.iter().any(|s| {
        s.entity.index_u32() < me.index_u32()
            && s.target != me
            && s.entity != at.entity
            && spares(Some(s.faction))
            && line.blocked(chest, at.chest, &[s.chest])
            && s.line.blocked(s.chest, s.to, &[chest])
    });
    if yields {
        hold.clearing = Some(Motion::Stand);
        return hold;
    }
    let all: Vec<Vec3> = others.iter().map(|b| b.1).collect();
    let blocked = Blocked {
        chest,
        to: at.chest,
        line,
        shields: &shields,
        yielding: &yielding,
        bodies: &all,
    };
    let (spot, motion) = unblock(ctx, &blocked, plan, on_slot, d, radius, rays);
    hold.kept_spot = spot;
    hold.clearing = motion;
    hold
}
