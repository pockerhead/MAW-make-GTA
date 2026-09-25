//! Lines of fire: a shooter holds fire while a groupmate or a bystander is in its line and walks to a
//! nearby spot whose own line to the target is clear (the line pivots on the target, so every spot is
//! tested on its own line, not as a parallel shift).

use super::{Ctx, Discipline, Motion, Seek};
use crate::character::LocomotionConfig;
use crate::combat::{AimConfig, Loadout, Weapon, WeaponsConfig};
use crate::gang::Faction;
use crate::navigation::{flat_distance, steer};
use crate::perception::sight_blocked;
use avian3d::prelude::*;
use bevy::prelude::*;

fn flat2(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

/// How far past the guarded end of the line, along it from the shooter's centre, a bullet can still
/// touch a body: the ray starts at the muzzle (at most its flat offset ahead of the centre) and stops on
/// the body's surface (at most the widest hitbox radius before its centre).
pub(crate) fn overreach(aim: &AimConfig, loco: &LocomotionConfig) -> f32 {
    flat2(aim.muzzle_offset()).length() + loco.capsule_radius.max(loco.head_radius)
}

/// Line-of-fire test of one member: the guarded reach (the weapon range, cut to `overshoot` past the
/// target, plus `overreach`), spread half-angle (rad), clearance around the line.
#[derive(Clone, Copy)]
pub(crate) struct FireLine {
    range: f32,
    overreach: f32,
    /// Metres past the target a miss is still guarded against.
    overshoot: f32,
    cone: f32,
    clearance: f32,
}

impl FireLine {
    /// The role's error cone on top of the widest spread of the shooter's gun (both half-angles; the
    /// error cone is applied to the aim, the weapon cone around it in `fire_weapons`).
    pub(crate) fn of(
        aim_error_deg: f32,
        weapons: &WeaponsConfig,
        gun: Weapon,
        loadout: &Loadout,
        clearance: f32,
        overreach: f32,
        overshoot: f32,
    ) -> Self {
        let stats = weapons.stats(gun);
        let spread = (stats.spread.base_deg + stats.spread.max_bloom_deg).max(loadout.spread_deg);
        Self {
            range: stats.range,
            overreach,
            overshoot,
            cone: (aim_error_deg + spread).to_radians(),
            clearance,
        }
    }

    /// The full weapon reach from the shooter's centre; the car filter must not depend on the target.
    pub(crate) fn reach(&self) -> f32 {
        self.range + self.overreach
    }

    /// A body of `bodies` sits in the line `from` -> `to`: ahead within the guarded reach (a miss is
    /// guarded against up to `overshoot` past the target) and inside the line widened by `clearance`
    /// plus the spread cone at its distance.
    pub(crate) fn blocked(&self, from: Vec3, to: Vec3, bodies: &[Vec3]) -> bool {
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
        let limit = self.range.min(flat2(to - from).length() + self.overshoot) + self.overreach;
        let (clearance, widen) = (self.clearance, self.cone.tan());
        bodies.iter().enumerate().filter_map(move |(k, &p)| {
            let dir = dir?;
            let rel = flat2(p - from);
            let along = rel.dot(dir);
            (along > 0.0 && along < limit && rel.perp_dot(dir).abs() <= clearance + along * widen)
                .then_some(k)
        })
    }
}

/// Flat footprint of a car: centre, unit right axis, half extents (right, forward).
#[derive(Clone, Copy, Debug)]
pub(crate) struct CarRect {
    pub(crate) centre: Vec2,
    pub(crate) axis: Vec2,
    pub(crate) half: Vec2,
}

impl CarRect {
    pub(crate) fn of(position: Vec3, rotation: Quat, half: Vec2) -> Self {
        let right = rotation * Vec3::X;
        Self {
            centre: flat2(position),
            axis: Vec2::new(right.x, right.z).normalize_or(Vec2::X),
            half,
        }
    }
}

/// The flat segment `from` -> `to` passes within `clearance` of `car` (no spread cone, nothing past
/// the target: a car only stops the bullets that would reach the target through it).
pub(crate) fn car_blocks(from: Vec3, to: Vec3, car: &CarRect, clearance: f32) -> bool {
    car_entry(from, to, car, clearance).is_some()
}

/// Where along the flat segment `from` -> `to` (0..1) it first comes within `clearance` of `car`.
fn car_entry(from: Vec3, to: Vec3, car: &CarRect, clearance: f32) -> Option<f32> {
    let local = |p: Vec3| {
        let d = flat2(p) - car.centre;
        Vec2::new(d.dot(car.axis), d.dot(car.axis.perp()))
    };
    let (a, b) = (local(from), local(to));
    let half = car.half + Vec2::splat(clearance);
    let dir = b - a;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    for (p, q, h) in [(a.x, dir.x, half.x), (a.y, dir.y, half.y)] {
        if q.abs() < f32::EPSILON {
            if p.abs() > h {
                return None;
            }
            continue;
        }
        let (e0, e1) = ((-h - p) / q, (h - p) / q);
        t0 = t0.max(e0.min(e1));
        t1 = t1.min(e0.max(e1));
    }
    (t0 <= t1).then_some(t0)
}

/// Where a walker at `from` heads to reach `to` around parked or queued cars (their bodies are not
/// in the wall avoidance): `to` while no car lies within `clearance` of the straight line; else the
/// corner, `corner` m out, of the first car in the way that is in plain view and shortest to go via.
pub(crate) fn around_cars(
    from: Vec3,
    to: Vec3,
    cars: &[CarRect],
    clearance: f32,
    corner: f32,
) -> Vec3 {
    let first = cars
        .iter()
        .filter_map(|car| car_entry(from, to, car, clearance).map(|t| (t, car)))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, car)) = first else {
        return to;
    };
    let half = car.half + Vec2::splat(corner);
    let via = |p: Vec3| flat_distance(from, p) + flat_distance(p, to);
    [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
        .into_iter()
        .map(|(u, v)| {
            let c = car.centre + car.axis * (u * half.x) + car.axis.perp() * (v * half.y);
            Vec3::new(c.x, to.y, c.y)
        })
        .filter(|&c| {
            cars.iter()
                .all(|other| car_entry(from, c, other, 0.0).is_none())
        })
        .min_by(|a, b| via(*a).total_cmp(&via(*b)))
        .unwrap_or(to)
}

/// Cars a shooter of `shooters` could fire through (centre within its reach plus the car's half
/// diagonal), except `exclude` (the car the target drives).
pub(crate) fn nearby_cars(
    shooters: &[Shooter],
    cars: impl Iterator<Item = (Entity, Vec3, Quat)>,
    exclude: Option<Entity>,
    half: Vec2,
) -> Vec<CarRect> {
    let diagonal = half.length();
    cars.filter(|&(car, ..)| Some(car) != exclude)
        .filter(|&(_, p, _)| {
            shooters
                .iter()
                .any(|s| flat_distance(s.chest, p) <= s.line.reach() + diagonal)
        })
        .map(|(_, p, r)| CarRect::of(p, r, half))
        .collect()
}

/// A shooter with its gun out and a live target: the line two shooters may block for each other.
pub(crate) struct Shooter {
    pub(crate) entity: Entity,
    pub(crate) faction: Faction,
    pub(crate) chest: Vec3,
    pub(crate) target: Entity,
    pub(crate) to: Vec3,
    pub(crate) line: FireLine,
}

/// A shooter whose line of fire to `to` is blocked this tick.
pub(crate) struct Blocked<'a> {
    pub(crate) chest: Vec3,
    pub(crate) to: Vec3,
    pub(crate) line: FireLine,
    /// Bodies it must not hit.
    pub(crate) shields: &'a [Vec3],
    /// Per shield: it is pressed against the target (within `pressed_distance`: a brawling groupmate, a
    /// human shield), so closing in cannot clear a line past it.
    pub(crate) yielding: &'a [bool],
    /// Every other living body: a spot is not walked to through one.
    pub(crate) bodies: &'a [Vec3],
    /// Cars near the shooter: the segment to the target must not cross one.
    pub(crate) cars: &'a [CarRect],
}

impl FireLine {
    pub(crate) fn clearance(&self) -> f32 {
        self.clearance
    }
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
    if b.line.blocked(spot, b.to, b.shields)
        || b.bodies.iter().any(|&p| bumps(p))
        || b.cars
            .iter()
            .any(|car| car_blocks(spot, b.to, car, b.line.clearance))
    {
        return false;
    }
    *rays += 1;
    let way = (spot - b.chest).normalize_or_zero();
    !sight_blocked(spatial, b.chest, spot + way * radius)
}

/// Candidate spots around the member, nearest first: both sides of its line at each
/// `reposition_offsets`, one `reposition_step` back and forward.
fn spots(b: &Blocked, d: &Discipline) -> Vec<Vec3> {
    let ahead = (b.to - b.chest).with_y(0.0).normalize_or_zero();
    let side = Vec3::new(-ahead.z, 0.0, ahead.x);
    let mut offsets: Vec<Vec3> = d
        .reposition_offsets
        .iter()
        .flat_map(|&o| [side * o, -side * o])
        .chain([-ahead * d.reposition_step, ahead * d.reposition_step])
        .collect();
    offsets.sort_by(|p, q| p.length_squared().total_cmp(&q.length_squared()));
    offsets.into_iter().map(|o| b.chest + o).collect()
}

/// The nearest usable candidate spot.
fn clear_spot(
    spatial: &SpatialQuery,
    b: &Blocked,
    d: &Discipline,
    radius: f32,
    rays: &mut u32,
) -> Option<Vec3> {
    spots(b, d)
        .into_iter()
        .find(|&spot| usable(spatial, b, spot, radius, rays))
}

/// Beside the nearest blocker that is not pressed against the target, shoulder to shoulder with it: in a
/// passage too narrow for the candidate spots the rear shooter queues up level with the front one.
fn queue_slot(spatial: &SpatialQuery, b: &Blocked, radius: f32, rays: &mut u32) -> Option<Vec3> {
    let front = b
        .line
        .blockers(b.chest, b.to, b.shields)
        .filter(|&k| !b.yielding[k])
        .map(|k| b.shields[k])
        .min_by(|p, q| flat_distance(*p, b.chest).total_cmp(&flat_distance(*q, b.chest)))?;
    let ahead = (b.to - front).with_y(0.0).normalize_or_zero();
    let side = Vec3::new(-ahead.z, 0.0, ahead.x) * (radius + b.line.clearance);
    let (left, right) = (front + side, front - side);
    let order = if flat_distance(right, b.chest) < flat_distance(left, b.chest) {
        [right, left]
    } else {
        [left, right]
    };
    order
        .into_iter()
        .find(|&slot| usable(spatial, b, slot, radius, rays))
}

/// Every body blocking the member's line, or the line of any candidate spot, gives the target to the
/// fight: nearer lines pass it all the same (a brawler 1.56 m from the target closes every line past
/// ~2.5 m), so closing in would only walk the member into the scrum.
fn pinned(b: &Blocked, d: &Discipline) -> bool {
    std::iter::once(b.chest).chain(spots(b, d)).all(|from| {
        b.line
            .blockers(from, b.to, b.shields)
            .all(|k| b.yielding[k])
            && !b
                .cars
                .iter()
                .any(|car| car_blocks(from, b.to, car, b.line.clearance))
    })
}

/// Motion of a member with a blocked line: to its clear spot, re-checked on its AI slot and kept while
/// still usable (re-picking every slot flipped a member between two sides in place). With none, a
/// `pinned` member keeps its `keep_distance` band (motion `None`) and fires once a line clears; a
/// member held up by a shooter ahead in a narrow passage takes the queue slot beside it (looked for on
/// its AI slot, like the spots); otherwise it
/// closes in on the target (nearer, the same spots turn the line further). Returns the
/// spot kept.
pub(crate) fn unblock(
    ctx: &Ctx,
    b: &Blocked,
    plan: Option<Vec3>,
    on_slot: bool,
    d: &Discipline,
    radius: f32,
    rays: &mut u32,
) -> (Option<Vec3>, Option<Motion>) {
    let plan = match plan {
        _ if !on_slot => plan,
        Some(spot) if usable(ctx.spatial, b, spot, radius, rays) => Some(spot),
        _ => clear_spot(ctx.spatial, b, d, radius, rays),
    };
    let Some(spot) = plan else {
        if pinned(b, d) {
            return (None, None);
        }
        let queue = on_slot.then(|| queue_slot(ctx.spatial, b, radius, rays));
        if let Some(slot) = queue.flatten() {
            return (
                Some(slot),
                Some(Motion::Yaw(steer(b.chest, slot), d.reposition_gait)),
            );
        }
        let direct = flat_distance(b.chest, b.to) <= ctx.nav.direct_seek_distance;
        let seek = Seek {
            dest: b.to,
            gait: d.chase_gait,
            direct,
        };
        return (None, Some(Motion::Seek(seek)));
    };
    (
        Some(spot),
        Some(Motion::Yaw(steer(b.chest, spot), d.reposition_gait)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A car at the origin along Z (half 1.2 x 2.04): a walker behind it heading for a point past its
    /// left side goes via the near left corner; a clear line goes straight.
    #[test]
    fn around_cars_rows() {
        let car = CarRect::of(Vec3::ZERO, Quat::IDENTITY, Vec2::new(1.2, 2.04));
        let (from, to) = (Vec3::new(0.0, 0.0, 3.0), Vec3::new(-1.7, 0.0, -8.0));
        let via = around_cars(from, to, &[car], 0.3, 0.8);
        assert!((via - Vec3::new(-2.0, 0.0, 2.84)).length() < 1e-4, "{via}");
        // From that corner the line along the side is clear.
        assert_eq!(around_cars(via, to, &[car], 0.3, 0.8), to);
        let clear = Vec3::new(-3.0, 0.0, -8.0);
        assert_eq!(
            around_cars(Vec3::new(-3.0, 0.0, 3.0), clear, &[car], 0.3, 0.8),
            clear
        );
    }

    /// Range 45, overreach 0.865, cone 11 deg, clearance 0.5; target 11 m ahead unless stated. The
    /// guarded reach is min(range, D + overshoot) + overreach along the line.
    #[test]
    fn overshoot_limits_the_guarded_zone() {
        let line = |overshoot: f32| FireLine {
            range: 45.0,
            overreach: 0.865,
            overshoot,
            cone: 11.0_f32.to_radians(),
            clearance: 0.5,
        };
        let from = Vec3::ZERO;
        let near = Vec3::new(0.0, 0.0, -11.0);
        let far = Vec3::new(0.0, 0.0, -40.0);
        let body = |z: f32| [Vec3::new(0.0, 0.0, z)];
        // 20.5 > 11 + 8 + 0.865 = 19.865: past the zone.
        assert!(!line(8.0).blocked(from, near, &body(-20.5)));
        assert!(line(8.0).blocked(from, near, &body(-19.5)));
        // Along 15, lateral 3 <= 0.5 + 15 tan 11 deg = 3.416.
        assert!(line(8.0).blocked(from, near, &[Vec3::new(3.0, 0.0, -15.0)]));
        // The range caps it: min(45, 48) + 0.865 = 45.865.
        assert!(line(8.0).blocked(from, far, &body(-45.5)));
        assert!(!line(8.0).blocked(from, far, &body(-46.0)));
        // Overshoot 60 >= range: the whole weapon reach, as before the rule.
        assert!(line(60.0).blocked(from, near, &body(-40.0)));
        assert!((line(8.0).reach() - 45.865).abs() < 1e-5);
    }
}
