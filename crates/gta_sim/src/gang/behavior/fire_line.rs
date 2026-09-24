//! Lines of fire: a member holds fire while a groupmate or a bystander is in its line and walks to a
//! nearby spot whose own line to the target is clear (the line pivots on the target, so every spot is
//! tested on its own line, not as a parallel shift).

use super::{Ctx, Motion, Seek};
use crate::combat::{Loadout, Weapon, WeaponsConfig};
use crate::gang::GangCombatConfig;
use crate::navigation::{flat_distance, steer};
use crate::perception::sight_blocked;
use avian3d::prelude::*;
use bevy::prelude::*;

fn flat2(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

/// Line-of-fire test of one member: weapon range, spread half-angle (rad), clearance around the line.
#[derive(Clone, Copy)]
pub(super) struct FireLine {
    range: f32,
    cone: f32,
    clearance: f32,
}

impl FireLine {
    /// The gang error cone on top of the widest spread of the member's gun (both half-angles; the error
    /// cone is applied to the aim, the weapon cone around it in `fire_weapons`).
    pub(super) fn of(
        c: &GangCombatConfig,
        weapons: &WeaponsConfig,
        gun: Weapon,
        loadout: &Loadout,
        clearance: f32,
    ) -> Self {
        let stats = weapons.stats(gun);
        let spread = (stats.spread.base_deg + stats.spread.max_bloom_deg).max(loadout.spread_deg);
        Self {
            range: stats.range,
            cone: (c.aim_error_deg + spread).to_radians(),
            clearance,
        }
    }

    /// A body of `bodies` sits in the line `from` -> `to`: ahead within `range` (a miss flies past the
    /// target) and inside the line widened by `clearance` plus the spread cone at its distance.
    pub(super) fn blocked(&self, from: Vec3, to: Vec3, bodies: &[Vec3]) -> bool {
        self.blockers(from, to, bodies).next().is_some()
    }

    /// Indices of the `bodies` that block the line `from` -> `to` (see `blocked`).
    fn blockers<'b>(
        &self,
        from: Vec3,
        to: Vec3,
        bodies: &'b [Vec3],
    ) -> impl Iterator<Item = usize> + 'b {
        let dir = flat2(to - from).try_normalize();
        let (range, clearance, widen) = (self.range, self.clearance, self.cone.tan());
        bodies.iter().enumerate().filter_map(move |(k, &p)| {
            let dir = dir?;
            let rel = flat2(p - from);
            let along = rel.dot(dir);
            (along > 0.0 && along < range && rel.perp_dot(dir).abs() <= clearance + along * widen)
                .then_some(k)
        })
    }
}

/// A member with its gun out and a live target: the line two members may block for each other.
pub(super) struct Shooter {
    pub(super) entity: Entity,
    pub(super) gang: u8,
    pub(super) chest: Vec3,
    pub(super) target: Entity,
    pub(super) to: Vec3,
    pub(super) line: FireLine,
}

/// A member whose line of fire to `to` is blocked this tick.
pub(super) struct Blocked<'a> {
    pub(super) chest: Vec3,
    pub(super) to: Vec3,
    pub(super) line: FireLine,
    /// Bodies it must not hit.
    pub(super) shields: &'a [Vec3],
    /// Per shield: it is pressed against the target (within `melee_distance.1`: a brawling groupmate, a
    /// human shield), so closing in cannot clear a line past it.
    pub(super) yielding: &'a [bool],
    /// Every other living body: a spot is not walked to through one.
    pub(super) bodies: &'a [Vec3],
}

/// Distance from `p` to the segment `a`-`b` in the ground plane.
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let (p, a, b) = (flat2(p), flat2(a), flat2(b));
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// `spot` has a clear line to the target and the member can walk straight to it: the walk brings it
/// within two radii of no body it is not already moving away from, and no wall stands in the way (one
/// World-mask ray, counted in `rays`).
fn usable(spatial: &SpatialQuery, b: &Blocked, spot: Vec3, radius: f32, rays: &mut u32) -> bool {
    let bumps = |p: Vec3| {
        let pass = segment_distance(p, b.chest, spot);
        pass < 2.0 * radius && pass < flat_distance(p, b.chest) - 1e-3
    };
    if b.line.blocked(spot, b.to, b.shields) || b.bodies.iter().any(|&p| bumps(p)) {
        return false;
    }
    *rays += 1;
    let way = (spot - b.chest).normalize_or_zero();
    !sight_blocked(spatial, b.chest, spot + way * radius)
}

/// Candidate spots around the member, nearest first: both sides of its line at each
/// `reposition_offsets`, one `reposition_step` back and forward.
fn spots(b: &Blocked, c: &GangCombatConfig) -> Vec<Vec3> {
    let ahead = (b.to - b.chest).with_y(0.0).normalize_or_zero();
    let side = Vec3::new(-ahead.z, 0.0, ahead.x);
    let mut offsets: Vec<Vec3> = c
        .reposition_offsets
        .iter()
        .flat_map(|&o| [side * o, -side * o])
        .chain([-ahead * c.reposition_step, ahead * c.reposition_step])
        .collect();
    offsets.sort_by(|p, q| p.length_squared().total_cmp(&q.length_squared()));
    offsets.into_iter().map(|o| b.chest + o).collect()
}

/// The nearest usable candidate spot.
fn clear_spot(
    spatial: &SpatialQuery,
    b: &Blocked,
    c: &GangCombatConfig,
    radius: f32,
    rays: &mut u32,
) -> Option<Vec3> {
    spots(b, c)
        .into_iter()
        .find(|&spot| usable(spatial, b, spot, radius, rays))
}

/// Every body blocking the member's line, or the line of any candidate spot, gives the target to the
/// fight: nearer lines pass it all the same (a brawler 1.56 m from the target closes every line past
/// ~2.5 m), so closing in would only walk the member into the scrum.
fn pinned(b: &Blocked, c: &GangCombatConfig) -> bool {
    std::iter::once(b.chest).chain(spots(b, c)).all(|from| {
        b.line
            .blockers(from, b.to, b.shields)
            .all(|k| b.yielding[k])
    })
}

/// Motion of a member with a blocked line: to its clear spot, re-checked on its AI slot and kept while
/// still usable (re-picking every slot flipped a member between two sides in place). With none, a
/// `pinned` member keeps its `keep_distance` band (motion `None`) and fires once a line clears;
/// otherwise it closes in on the target (nearer, the same spots turn the line further). Returns the
/// spot kept.
pub(super) fn unblock(
    ctx: &Ctx,
    b: &Blocked,
    plan: Option<Vec3>,
    on_slot: bool,
    c: &GangCombatConfig,
    radius: f32,
    rays: &mut u32,
) -> (Option<Vec3>, Option<Motion>) {
    let plan = match plan {
        _ if !on_slot => plan,
        Some(spot) if usable(ctx.spatial, b, spot, radius, rays) => Some(spot),
        _ => clear_spot(ctx.spatial, b, c, radius, rays),
    };
    let Some(spot) = plan else {
        if pinned(b, c) {
            return (None, None);
        }
        let direct = flat_distance(b.chest, b.to) <= ctx.nav.direct_seek_distance;
        let seek = Seek {
            dest: b.to,
            gait: c.chase_gait,
            direct,
        };
        return (None, Some(Motion::Seek(seek)));
    };
    (
        Some(spot),
        Some(Motion::Yaw(steer(b.chest, spot), c.reposition_gait)),
    )
}
