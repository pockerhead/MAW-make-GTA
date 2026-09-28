//! TASK-039 extensions of the G1 oracle: the pair a progress relaxation exempts (reported by depth, never
//! a violation), checked on its own terms (it starts against a standing body, apart from it, and ends
//! within its bound), and, opt in, third bodies a kinematic car must not drive into: characters and
//! the static world.

use super::*;
use gta_sim::character::{Character, LocomotionConfig};
use gta_sim::vehicle::Vehicle;
use std::collections::HashMap;

/// Longest a relaxed pair may still touch after the squeeze's `max_seconds` (it drives out of the
/// body at the pass speed), s.
pub const SEPARATION_S: f32 = 10.0;

/// Distinct third-body pair: (pair, worst depth, first tick, centres).
pub type ThirdSummary = ((Entity, Entity), f32, u32, (Vec2, Vec2));

/// This tick's relaxed pairs: (car, its blocker or trailing body), both phases.
pub type Relaxed = Vec<(Entity, Entity)>;

pub fn relaxed_pairs(app: &mut App) -> Relaxed {
    app.world_mut()
        .query::<(Entity, &TrafficCar)>()
        .iter(app.world())
        .filter_map(|(e, c)| c.relaxed.map(|r| (e, r)))
        .flat_map(|(e, r)| {
            [Some(r.blocker), r.trailing]
                .into_iter()
                .flatten()
                .map(move |b| (e, b))
        })
        .collect()
}

/// Relaxed pairs and third-body state of a `Footprints`.
#[derive(Default)]
pub struct Third {
    /// Worst depth of a pair exempt by a relaxation.
    pub relaxed_max_depth: f32,
    enabled: bool,
    pub violations: Vec<Violation>,
    /// Relaxed pairs under way: (car, blocker) -> first tick.
    since: HashMap<(Entity, Entity), u32>,
    /// Ground speed of every vehicle and character at the previous record (before this tick's step).
    speeds: HashMap<Entity, f32>,
    /// Longest a pair may stay relaxed, ticks (`max_seconds` + `SEPARATION_S`).
    pub relax_limit: u32,
    /// Relaxations that started against a moving body or inside it, or outlived `relax_limit`.
    pub relax_violations: Vec<String>,
}

impl Third {
    /// `pair` is a relaxed (car, blocker) pair: its depth is recorded as the relaxed depth.
    pub fn exempt(&mut self, relaxed: &Relaxed, pair: (Entity, Entity), depth: f32) -> bool {
        let (a, b) = pair;
        let hit = relaxed.iter().any(|&p| p == (a, b) || p == (b, a));
        if hit {
            self.relaxed_max_depth = self.relaxed_max_depth.max(depth);
        }
        hit
    }
}

/// Signed distance from `p` to a rectangle (negative inside).
fn signed_distance(p: Vec2, rect: &Rect) -> f32 {
    let d = p - rect.centre;
    let q = Vec2::new(d.dot(rect.right).abs(), d.dot(rect.forward).abs()) - rect.half;
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0)
}

impl Footprints {
    /// Also checks every tick: a kinematic vehicle against every character near the player (depth: the
    /// capsule radius minus the circle-to-footprint distance, `dynamic_tolerance`) and against the static
    /// world (the chassis box above its underbody lift, shrunk 0.02 m per side: any hit).
    pub fn with_third_bodies(mut self) -> Self {
        self.third.enabled = true;
        self
    }

    pub fn relaxed_max_depth(&self) -> f32 {
        self.third.relaxed_max_depth
    }

    /// The relaxation checks, as one failure line.
    pub fn relax_failure(&self) -> Option<String> {
        let v = &self.third.relax_violations;
        (!v.is_empty()).then(|| format!("relaxations: {v:?}"))
    }

    /// Every relaxation starts against a body standing (at most `hold_speed` when the tick that set it
    /// began) and apart from it (at most `dynamic_tolerance` deep), and the pair leaves the relaxed set
    /// within `relax_limit`. Read from the bodies' own pose and velocity, not from the relaxation.
    fn record_relaxations(&mut self, app: &mut App, tick: u32, relaxed: &Relaxed) {
        let speeds: HashMap<Entity, f32> = app
            .world_mut()
            .query_filtered::<(Entity, &LinearVelocity), Or<(With<Vehicle>, With<Character>)>>()
            .iter(app.world())
            .map(|(e, v)| (e, v.0.with_y(0.0).length()))
            .collect();
        let before = std::mem::replace(&mut self.third.speeds, speeds);
        let world = app.world();
        let (half, hold) = {
            let v = world.resource::<VehicleConfig>();
            let h = v.half_extents();
            (Vec2::new(h.x, h.z), v.hold_speed)
        };
        let radius = world.resource::<LocomotionConfig>().capsule_radius;
        let limit = if self.third.relax_limit == 0 {
            let dt = world.resource::<Time<Fixed>>().timestep().as_secs_f32();
            let max = world.resource::<TrafficConfig>().progress.max_seconds;
            ((max + SEPARATION_S) / dt).ceil() as u32
        } else {
            self.third.relax_limit
        };
        self.third.since.retain(|pair, _| relaxed.contains(pair));
        for &(car, blocker) in relaxed {
            let Some(&start) = self.third.since.get(&(car, blocker)) else {
                self.third.since.insert((car, blocker), tick);
                let speed = before
                    .get(&blocker)
                    .or(self.third.speeds.get(&blocker))
                    .copied()
                    .unwrap_or(0.0);
                let (Some(p), Some(r), Some(b)) = (
                    world.get::<Position>(car),
                    world.get::<Rotation>(car),
                    world.get::<Position>(blocker),
                ) else {
                    continue;
                };
                let rect = Rect::of(p.0, r.0, half);
                let depth: f32 = match (
                    world.get::<Rotation>(blocker),
                    world.get::<Character>(blocker),
                ) {
                    (Some(q), None) => penetration(&rect, &Rect::of(b.0, q.0, half)),
                    _ => radius - signed_distance(Vec2::new(b.0.x, b.0.z), &rect),
                }
                .max(0.0);
                if speed > hold || depth > self.dynamic_tolerance {
                    self.third.relax_violations.push(format!(
                        "t {tick}: {car} relaxed against {blocker} moving {speed:.2} m/s, {depth:.3} m deep"
                    ));
                }
                continue;
            };
            if tick - start == limit + 1 {
                self.third.relax_violations.push(format!(
                    "t {tick}: {car} still relaxed against {blocker} {} ticks after tick {start}",
                    limit + 1
                ));
            }
        }
    }

    pub(super) fn record_third(
        &mut self,
        app: &mut App,
        tick: u32,
        bodies: &[(Entity, Rect, bool)],
        relaxed: &Relaxed,
    ) {
        self.record_relaxations(app, tick, relaxed);
        if !self.third.enabled {
            return;
        }
        let radius = app.world().resource::<LocomotionConfig>().capsule_radius;
        let characters: Vec<(Entity, Vec2)> = app
            .world_mut()
            .query_filtered::<(Entity, &Position), (
                With<Character>,
                Without<ColliderDisabled>,
                Without<RigidBodyDisabled>,
            )>()
            .iter(app.world())
            .map(|(e, p)| (e, Vec2::new(p.0.x, p.0.z)))
            .collect();
        for (car, rect, kinematic) in bodies.iter().filter(|b| b.2) {
            for &(who, at) in &characters {
                if at.distance(rect.centre) > rect.radius() + radius {
                    continue;
                }
                let depth = radius - signed_distance(at, rect);
                if depth <= 0.0 || self.third.exempt(relaxed, (*car, who), depth) {
                    continue;
                }
                if depth > self.dynamic_tolerance {
                    self.third.violations.push(Violation {
                        tick,
                        pair: (*car, who),
                        kinematic: (*kinematic, false),
                        depth,
                        at: (rect.centre, at),
                    });
                }
            }
        }
        let cars: Vec<(Entity, Vec3, Quat)> = bodies
            .iter()
            .filter(|b| b.2)
            .filter_map(|b| {
                let world = app.world();
                Some((
                    b.0,
                    world.get::<Position>(b.0)?.0,
                    world.get::<Rotation>(b.0)?.0,
                ))
            })
            .collect();
        let h = app.world().resource::<VehicleConfig>().half_extents();
        let walls = app
            .world_mut()
            .run_system_once(move |spatial: SpatialQuery| {
                let shape =
                    Collider::cuboid(2.0 * h.x - 0.04, 2.0 * h.y - 0.2 - 0.04, 2.0 * h.z - 0.04);
                let filter = SpatialQueryFilter::from_mask(GameLayer::World);
                cars.iter()
                    .filter_map(|&(car, p, r)| {
                        let hits =
                            spatial.shape_intersections(&shape, p + Vec3::Y * 0.1, r, &filter);
                        hits.first().map(|&wall| (car, wall, Vec2::new(p.x, p.z)))
                    })
                    .collect::<Vec<_>>()
            })
            .expect("GATE BROKEN: overlap query failed");
        for (car, wall, at) in walls {
            self.third.violations.push(Violation {
                tick,
                pair: (car, wall),
                kinematic: (true, false),
                depth: 0.02,
                at: (at, at),
            });
        }
    }

    /// Distinct third-body pairs: (pair, worst depth, first tick, centres).
    pub fn third_summary(&self) -> Vec<ThirdSummary> {
        let mut out: Vec<ThirdSummary> = Vec::new();
        for v in &self.third.violations {
            match out.iter_mut().find(|o| o.0 == v.pair) {
                Some(o) => o.1 = o.1.max(v.depth),
                None => out.push((v.pair, v.depth, v.tick, v.at)),
            }
        }
        out
    }

    pub(super) fn assert_third_clean(&self, label: &str) {
        assert!(
            self.third.relax_violations.is_empty(),
            "{label}: relaxations {:?}",
            self.third.relax_violations
        );
        assert!(
            self.third.violations.is_empty(),
            "{label}: {} third-body violation ticks, distinct (pair, depth, first tick, centres): {:?}",
            self.third.violations.len(),
            self.third_summary()
        );
    }
}

impl StandClock {
    /// How long `car` has stood so far (0 when it moves or is unknown), s.
    pub fn current(&self, car: Entity) -> f32 {
        self.current
            .get(&car)
            .and_then(|c| c.0)
            .map_or(0.0, |since| self.now - since)
    }
}
