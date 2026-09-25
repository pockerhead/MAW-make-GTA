//! Kinematic → dynamic switch before a contact: a kinematic body would push a dynamic one with
//! infinite mass (probe Q4), so the car turns dynamic while the other body is still one sweep away.

use super::{SwitchCause, TrafficCar, TrafficConfig, TrafficMode, TrafficStats};
use crate::character::{Character, LocomotionConfig};
use crate::layers::GameLayer;
use crate::vehicle::{Autopilot, DriveIntent, Vehicle, VehicleConfig};
use avian3d::prelude::*;
use bevy::prelude::*;

/// A flat oriented rectangle: centre, unit right axis, half extents (right, forward).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlatRect {
    pub centre: Vec2,
    pub axis: Vec2,
    pub half: Vec2,
}

impl FlatRect {
    /// Footprint of a body at `position` / `rotation` with body-frame half extents x / z `half`.
    pub fn of(position: Vec3, rotation: Quat, half: Vec2) -> Self {
        let right = rotation * Vec3::X;
        Self {
            centre: Vec2::new(position.x, position.z),
            axis: Vec2::new(right.x, right.z).normalize_or(Vec2::X),
            half,
        }
    }

    fn axes(&self) -> [Vec2; 2] {
        [self.axis, self.axis.perp()]
    }

    /// Projection interval on `n`.
    fn span(&self, n: Vec2) -> (f32, f32) {
        let c = self.centre.dot(n);
        let r = self.half.x * self.axis.dot(n).abs() + self.half.y * self.axis.perp().dot(n).abs();
        (c - r, c + r)
    }

    /// `p` in the rectangle's frame.
    fn local(&self, p: Vec2) -> Vec2 {
        let d = p - self.centre;
        Vec2::new(d.dot(self.axis), d.dot(self.axis.perp()))
    }
}

fn point_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let d = b - a;
    let t = ((p - a).dot(d) / d.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
    p.distance(a + d * t)
}

/// Distance from a point to an axis-aligned box of half extents `half` centred at the origin.
fn point_box(p: Vec2, half: Vec2) -> f32 {
    (p.abs() - half).max(Vec2::ZERO).length()
}

/// A circle of `radius` moving from `centre` by `d` touches `rect` at some point of the sweep.
pub fn swept_circle_hits_rect(centre: Vec2, radius: f32, d: Vec2, rect: &FlatRect) -> bool {
    let (a, b) = (rect.local(centre), rect.local(centre + d));
    let half = rect.half;
    // The segment enters the box (slab clipping) or one of the closest-pair vertices is near.
    let inside = |p: Vec2| p.x.abs() <= half.x && p.y.abs() <= half.y;
    if inside(a) || inside(b) {
        return true;
    }
    let dir = b - a;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    let mut crosses = true;
    for (p, q, h) in [(a.x, dir.x, half.x), (a.y, dir.y, half.y)] {
        if q.abs() < f32::EPSILON {
            if p.abs() > h {
                crosses = false;
            }
            continue;
        }
        let (e0, e1) = ((-h - p) / q, (h - p) / q);
        t0 = t0.max(e0.min(e1));
        t1 = t1.min(e0.max(e1));
    }
    if crosses && t0 <= t1 {
        return true;
    }
    let corners = [
        Vec2::new(half.x, half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, -half.y),
        Vec2::new(-half.x, half.y),
    ];
    let nearest = point_box(a, half).min(point_box(b, half)).min(
        corners
            .iter()
            .map(|&c| point_segment(c, a, b))
            .fold(f32::INFINITY, f32::min),
    );
    nearest <= radius
}

/// `other` moving by `d` touches `rect` at some point of the sweep (SAT on the swept hull).
pub fn swept_rect_hits_rect(other: &FlatRect, d: Vec2, rect: &FlatRect) -> bool {
    let mut axes: Vec<Vec2> = rect.axes().into_iter().chain(other.axes()).collect();
    if let Some(n) = d.perp().try_normalize() {
        axes.push(n);
    }
    axes.into_iter().all(|n| {
        let (a0, a1) = rect.span(n);
        let (b0, b1) = other.span(n);
        let shift = d.dot(n);
        let (s0, s1) = (b0.min(b0 + shift), b1.max(b1 + shift));
        s0 <= a1 && a0 <= s1
    })
}

fn flat(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn switch_to_dynamic(
    mut commands: Commands,
    spatial: SpatialQuery,
    configs: (
        Res<TrafficConfig>,
        Res<VehicleConfig>,
        Res<LocomotionConfig>,
    ),
    mut stats: ResMut<TrafficStats>,
    mut starts: MessageReader<CollisionStart>,
    mut cars: Query<(
        Entity,
        &mut TrafficCar,
        &Position,
        &Rotation,
        &LinearVelocity,
        &RigidBody,
    )>,
    bodies: Query<(
        &RigidBody,
        &Position,
        &Rotation,
        &LinearVelocity,
        Has<RigidBodyDisabled>,
        Has<Sleeping>,
        Has<Character>,
        Has<Vehicle>,
    )>,
) {
    let (cfg, vehicle, loco) = configs;
    let half = vehicle.half_extents();
    let reach = cfg.switch.reach;
    let broad = Collider::cuboid(
        2.0 * (half.x + reach),
        2.0 * (half.y + reach),
        2.0 * (half.z + reach),
    );
    let mask = SpatialQueryFilter::from_mask([GameLayer::Character, GameLayer::Vehicle]);
    let is_kinematic_ai = |car: &TrafficCar, body: &RigidBody| {
        body.is_kinematic()
            && matches!(
                car.mode,
                TrafficMode::Kinematic | TrafficMode::Bailing { .. }
            )
    };
    let mut switch: Vec<(Entity, SwitchCause)> = Vec::new();
    for (entity, car, position, rotation, velocity, body) in &cars {
        if !is_kinematic_ai(car, body) {
            continue;
        }
        let own = FlatRect::of(
            position.0,
            rotation.0,
            Vec2::new(half.x + cfg.switch.skin, half.z + cfg.switch.skin),
        );
        let filter = mask.clone().with_excluded_entities([entity]);
        let hits = spatial.shape_intersections(&broad, position.0, rotation.0, &filter);
        let cause = hits.into_iter().find_map(|other| {
            let (rb, p, r, v, disabled, sleeping, character, is_car) = bodies.get(other).ok()?;
            if !rb.is_dynamic() || disabled {
                return None;
            }
            let v_other = if sleeping { Vec3::ZERO } else { v.0 };
            let d = flat(v_other - velocity.0) * cfg.switch.horizon_seconds;
            if character {
                swept_circle_hits_rect(flat(p.0), loco.capsule_radius, d, &own)
                    .then_some(SwitchCause::Character)
            } else if is_car {
                let rect = FlatRect::of(p.0, r.0, Vec2::new(half.x, half.z));
                swept_rect_hits_rect(&rect, d, &own).then_some(SwitchCause::Vehicle)
            } else {
                None
            }
        });
        if let Some(cause) = cause {
            switch.push((entity, cause));
        }
    }
    for start in starts.read() {
        let e1 = start.body1.unwrap_or(start.collider1);
        let e2 = start.body2.unwrap_or(start.collider2);
        for (car, other) in [(e1, e2), (e2, e1)] {
            let Ok((_, traffic, .., body)) = cars.get(car) else {
                continue;
            };
            let dynamic_other = bodies
                .get(other)
                .is_ok_and(|(rb, _, _, _, disabled, ..)| rb.is_dynamic() && !disabled);
            if is_kinematic_ai(traffic, body) && dynamic_other && !switch.iter().any(|s| s.0 == car)
            {
                switch.push((car, SwitchCause::Backstop));
            }
        }
    }
    for (entity, cause) in switch {
        let Ok((_, mut car, position, rotation, velocity, _)) = cars.get_mut(entity) else {
            continue;
        };
        let forward = rotation.0 * Vec3::NEG_Z;
        commands.entity(entity).try_insert((
            RigidBody::Dynamic,
            SleepingDisabled,
            Autopilot {
                target: position.0 + forward * vehicle.autopilot.lookahead_min,
                speed: velocity.0.dot(forward).max(0.0),
                stuck: 0.0,
                reverse_left: 0.0,
            },
            DriveIntent::default(),
        ));
        if car.mode == TrafficMode::Kinematic {
            car.mode = TrafficMode::Dynamic;
        }
        stats.switches_by_cause[cause as usize] += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A car footprint facing −Z at `centre` (half 1.2 x 2.04, grown by `skin`).
    fn car(centre: Vec2, skin: f32) -> FlatRect {
        FlatRect::of(
            Vec3::new(centre.x, 0.0, centre.y),
            Quat::IDENTITY,
            Vec2::new(1.2 + skin, 2.04 + skin),
        )
    }

    #[test]
    fn rear_end_row() {
        // A stopped car at the origin; the player's car 0.5 m behind its rear bumper (+Z side)
        // closing at 10 m/s: sweep 1.0 m.
        let own = car(Vec2::ZERO, 0.1);
        let player = car(Vec2::new(0.0, 2.04 + 0.5 + 2.04), 0.0);
        assert!(swept_rect_hits_rect(&player, Vec2::new(0.0, -1.0), &own));
        // 1.2 m gap: the sweep does not reach yet.
        let far = car(Vec2::new(0.0, 2.04 + 1.2 + 2.04), 0.0);
        assert!(!swept_rect_hits_rect(&far, Vec2::new(0.0, -1.0), &own));
    }

    #[test]
    fn pedestrian_crossing_row() {
        // 1.2 m ahead of the front bumper (−Z), walking across at 1.4 m/s: sweep 0.14 m sideways.
        let own = car(Vec2::ZERO, 0.1);
        let at = Vec2::new(-0.5, -(2.04 + 1.2));
        assert!(!swept_circle_hits_rect(at, 0.3, Vec2::new(0.14, 0.0), &own));
        // Walking into the bumper instead.
        assert!(swept_circle_hits_rect(
            Vec2::new(0.0, -(2.04 + 0.35)),
            0.3,
            Vec2::new(0.0, 0.14),
            &own
        ));
    }

    #[test]
    fn oncoming_row() {
        // Opposite inner lane, 3.25 m centre to centre (0.85 m clear), closing 28 m/s: sweep 2.8 m.
        let own = car(Vec2::ZERO, 0.1);
        let other = car(Vec2::new(3.25, -6.0), 0.0);
        assert!(!swept_rect_hits_rect(&other, Vec2::new(0.0, 2.8), &own));
    }

    #[test]
    fn parked_beside_row() {
        // A sleeping parked car 0.85 m beside a passing car at 16 m/s (relative sweep 1.6 m back).
        let own = car(Vec2::ZERO, 0.1);
        let parked = car(Vec2::new(-(2.4 + 0.85), 0.0), 0.0);
        assert!(!swept_rect_hits_rect(&parked, Vec2::new(0.0, 1.6), &own));
    }

    #[test]
    fn sleeping_ahead_row() {
        // A sleeping car 1.0 m ahead of a kinematic car at 12 m/s: relative sweep 1.2 m towards it.
        let own = car(Vec2::ZERO, 0.1);
        let ahead = car(Vec2::new(0.0, -(4.08 + 1.0)), 0.0);
        assert!(swept_rect_hits_rect(&ahead, Vec2::new(0.0, 1.2), &own));
    }

    #[test]
    fn rotated_rect_sat() {
        // A car at 45° whose corner points at the own car: the corner decides, not the AABB.
        let own = car(Vec2::ZERO, 0.0);
        let r = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        let diagonal = FlatRect::of(Vec3::new(4.0, 0.0, 0.0), r, Vec2::new(1.2, 2.04));
        // Its corner reaches x = 4 − 2.29 = 1.71: a 0.3 m sweep stays 0.21 m off, 1.5 m overlaps.
        assert!(!swept_rect_hits_rect(&diagonal, Vec2::new(-0.3, 0.0), &own));
        assert!(swept_rect_hits_rect(&diagonal, Vec2::new(-1.5, 0.0), &own));
    }
}
