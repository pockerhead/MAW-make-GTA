//! Queries over the road snapshot: the first body along a strip (sensing, lane choice) and the first
//! body or claim overlapping a rectangle (lane starts, grants, spawns, manoeuvre clearance).

use super::{BodyKind, Claim, Footprint, RoadBody, RoadOccupancy};
use crate::layers::GameLayer;
use crate::traffic::{FlatRect, swept_circle_hits_rect, swept_rect_hits_rect};
use avian3d::prelude::*;
use bevy::prelude::*;

/// A band `half_width` either side of the line from `origin` along the unit `dir`, `length` m long.
#[derive(Clone, Copy, Debug)]
pub struct Strip {
    pub origin: Vec2,
    pub dir: Vec2,
    pub length: f32,
    pub half_width: f32,
}

/// The nearest body (or opposite claim) along a strip.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub entity: Entity,
    /// From the strip origin to the body's near side, m (0 when it straddles the origin).
    pub gap: f32,
    /// Its velocity along the strip, m/s.
    pub speed_along: f32,
    pub standing: f32,
    pub kind: BodyKind,
    pub dynamic: bool,
    /// A pass claim rather than a body.
    pub claim: bool,
}

/// Which claims an overlap query sees.
#[derive(Clone, Copy, Debug)]
pub enum ClaimFilter {
    None,
    All,
    Except(Entity),
    /// Claims of cars travelling against this direction.
    Against(Vec2),
}

fn corners(rect: &FlatRect) -> [Vec2; 4] {
    let (x, y) = (rect.axis * rect.half.x, rect.axis.perp() * rect.half.y);
    [
        rect.centre + x + y,
        rect.centre + x - y,
        rect.centre - x - y,
        rect.centre - x + y,
    ]
}

/// Keeps the part of `polygon` with `sign × c <= limit` (one Sutherland-Hodgman pass on a band line).
fn clip(polygon: &[Vec2], sign: f32, limit: f32) -> Vec<Vec2> {
    let inside = |p: Vec2| sign * p.y <= limit;
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for (k, &p) in polygon.iter().enumerate() {
        let q = polygon[(k + 1) % polygon.len()];
        if inside(p) {
            out.push(p);
        }
        if inside(p) != inside(q) {
            let t = (limit - sign * p.y) / (sign * (q.y - p.y));
            out.push(p + (q - p) * t);
        }
    }
    out
}

impl Strip {
    /// `p` as (along, across) the strip.
    fn local(&self, p: Vec2) -> Vec2 {
        let d = p - self.origin;
        Vec2::new(d.dot(self.dir), d.dot(self.dir.perp()))
    }

    /// Interval of `shape` along the strip inside the band, if it reaches into the band.
    fn span(&self, shape: &Footprint) -> Option<(f32, f32)> {
        match *shape {
            Footprint::Rect(rect) => {
                let local = corners(&rect).map(|p| self.local(p));
                let clipped = clip(&clip(&local, 1.0, self.half_width), -1.0, self.half_width);
                let (lo, hi) = clipped
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                        (lo.min(p.x), hi.max(p.x))
                    });
                (lo <= hi).then_some((lo, hi))
            }
            Footprint::Circle { centre, radius } => {
                let c = self.local(centre);
                let out = (c.y.abs() - self.half_width).max(0.0);
                (out < radius).then(|| {
                    let reach = (radius * radius - out * out).sqrt();
                    (c.x - reach, c.x + reach)
                })
            }
        }
    }

    /// Gap to a shape whose along-interval is `(lo, hi)`, if it lies on the strip.
    fn gap(&self, (lo, hi): (f32, f32)) -> Option<f32> {
        (hi >= 0.0 && lo <= self.length).then(|| lo.max(0.0))
    }
}

fn overlaps(shape: &Footprint, rect: &FlatRect) -> bool {
    let shrunk = FlatRect {
        half: (rect.half - Vec2::splat(0.01)).max(Vec2::ZERO),
        ..*rect
    };
    match *shape {
        Footprint::Rect(r) => {
            let r = FlatRect {
                half: (r.half - Vec2::splat(0.01)).max(Vec2::ZERO),
                ..r
            };
            swept_rect_hits_rect(&r, Vec2::ZERO, &shrunk)
        }
        Footprint::Circle { centre, radius } => {
            swept_circle_hits_rect(centre, (radius - 0.01).max(0.0), Vec2::ZERO, &shrunk)
        }
    }
}

impl RoadOccupancy {
    /// The nearest body along `strip` that `skip` does not drop, or the nearest claim of a car
    /// travelling against the strip.
    pub fn first_along(&self, strip: &Strip, skip: impl Fn(&RoadBody) -> bool) -> Option<Hit> {
        let reach = strip.length + strip.half_width;
        let mut best: Option<Hit> = None;
        let mut keep = |hit: Hit| {
            if best.is_none_or(|b| hit.gap < b.gap) {
                best = Some(hit);
            }
        };
        for body in &self.bodies {
            let centre = match body.shape {
                Footprint::Rect(r) => r.centre,
                Footprint::Circle { centre, .. } => centre,
            };
            if centre.distance(strip.origin) > reach + 4.0 || skip(body) {
                continue;
            }
            let Some(gap) = strip.span(&body.shape).and_then(|s| strip.gap(s)) else {
                continue;
            };
            keep(Hit {
                entity: body.entity,
                gap,
                speed_along: body.velocity.dot(strip.dir),
                standing: body.standing,
                kind: body.kind,
                dynamic: body.dynamic,
                claim: false,
            });
        }
        // A car already inside an oncoming claim drives out of it (the passer waits for that).
        for claim in self.claims.iter().filter(|c| c.dir.dot(strip.dir) < 0.0) {
            let Some(gap) = strip
                .span(&Footprint::Rect(claim.rect))
                .filter(|s| s.0 > 0.0)
                .and_then(|s| strip.gap(s))
            else {
                continue;
            };
            keep(Hit {
                entity: claim.owner,
                gap,
                speed_along: 0.0,
                standing: f32::MAX,
                kind: BodyKind::Traffic,
                dynamic: false,
                claim: true,
            });
        }
        best
    }

    /// The first body (not dropped by `skip`) or claim (by `claims`) overlapping `rect`.
    pub fn blocked(
        &self,
        rect: &FlatRect,
        skip: impl Fn(&RoadBody) -> bool,
        claims: ClaimFilter,
    ) -> Option<Entity> {
        let reach = rect.half.length();
        let body = self.bodies.iter().find(|b| {
            let (centre, size) = match b.shape {
                Footprint::Rect(r) => (r.centre, r.half.length()),
                Footprint::Circle { centre, radius } => (centre, radius),
            };
            centre.distance(rect.centre) <= reach + size && !skip(b) && overlaps(&b.shape, rect)
        });
        if let Some(b) = body {
            return Some(b.entity);
        }
        self.claims
            .iter()
            .filter(|c| match claims {
                ClaimFilter::None => false,
                ClaimFilter::All => true,
                ClaimFilter::Except(e) => c.owner != e,
                ClaimFilter::Against(dir) => c.dir.dot(dir) < 0.0,
            })
            .find(|c| overlaps(&Footprint::Rect(c.rect), rect))
            .map(|c| c.owner)
    }

    pub fn body(&self, entity: Entity) -> Option<&RoadBody> {
        self.index.get(&entity).map(|&k| &self.bodies[k])
    }

    pub fn bodies(&self) -> &[RoadBody] {
        &self.bodies
    }

    pub fn claims(&self) -> &[Claim] {
        &self.claims
    }

    /// Police cars with the sirens on.
    pub fn sirens(&self) -> impl Iterator<Item = &RoadBody> {
        self.bodies.iter().filter(|b| b.siren)
    }

    /// A claim committed this tick (visible at once to the rest of the tick).
    pub fn insert_claim(&mut self, claim: Claim) {
        self.claims.push(claim);
    }

    pub fn claims_extend(&mut self, claims: impl IntoIterator<Item = Claim>) {
        self.claims.extend(claims);
    }
}

/// No static world geometry (walls, curbs, the raised sidewalk) inside `rect` between `bottom` and
/// `top`: the road edge stays an avian query.
pub fn world_clear(spatial: &SpatialQuery, rect: &FlatRect, bottom: f32, top: f32) -> bool {
    let rotation = Quat::from_rotation_arc(Vec3::X, Vec3::new(rect.axis.x, 0.0, rect.axis.y));
    let centre = Vec3::new(rect.centre.x, (bottom + top) / 2.0, rect.centre.y);
    spatial
        .shape_intersections(
            &Collider::cuboid(2.0 * rect.half.x, top - bottom, 2.0 * rect.half.y),
            centre,
            rotation,
            &SpatialQueryFilter::from_mask(GameLayer::World),
        )
        .is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::occupancy::Footprint;

    /// A car facing -Z (strip direction (0, -1) in flat x/z) at `centre`.
    fn car(centre: Vec2) -> Footprint {
        Footprint::Rect(FlatRect::of(
            Vec3::new(centre.x, 0.0, centre.y),
            Quat::IDENTITY,
            Vec2::new(1.2, 2.04),
        ))
    }

    fn road(bodies: Vec<Footprint>, claims: Vec<Claim>) -> RoadOccupancy {
        let mut road = RoadOccupancy::default();
        for (k, shape) in bodies.into_iter().enumerate() {
            road.index.insert(Entity::from_bits(k as u64 + 1), k);
            road.bodies.push(RoadBody {
                entity: Entity::from_bits(k as u64 + 1),
                kind: BodyKind::Vehicle,
                shape,
                velocity: Vec2::ZERO,
                dynamic: true,
                standing: 0.0,
                siren: false,
            });
        }
        road.claims = claims;
        road
    }

    /// From the origin along -Z, 25 m, half width 1.2.
    fn strip() -> Strip {
        Strip {
            origin: Vec2::ZERO,
            dir: Vec2::NEG_Y,
            length: 25.0,
            half_width: 1.2,
        }
    }

    fn gap(road: &RoadOccupancy) -> Option<f32> {
        road.first_along(&strip(), |_| false).map(|h| h.gap)
    }

    #[test]
    fn strip_rows() {
        // Straight ahead: rear 10 m ahead.
        let g = gap(&road(vec![car(Vec2::new(0.0, -12.04))], vec![]));
        assert!((g.unwrap() - 10.0).abs() < 1e-4, "{g:?}");
        // Straddling the band edge (centre 2.2 m aside, 0.2 m inside the band).
        let g = gap(&road(vec![car(Vec2::new(2.2, -12.04))], vec![]));
        assert!((g.unwrap() - 10.0).abs() < 1e-4, "{g:?}");
        // Beside the band (2.5 m aside: 0.1 m clear).
        assert_eq!(gap(&road(vec![car(Vec2::new(2.5, -12.04))], vec![])), None);
        // Behind the origin: excluded.
        assert_eq!(gap(&road(vec![car(Vec2::new(0.0, 3.0))], vec![])), None);
        // Straddling the origin: gap 0.
        assert_eq!(
            gap(&road(vec![car(Vec2::new(0.0, -1.0))], vec![])),
            Some(0.0)
        );
        // Past the end.
        assert_eq!(gap(&road(vec![car(Vec2::new(0.0, -28.0))], vec![])), None);
        // A walker at the band edge beside the bumper (the traffic_pedestrian flank case: centre 1.14 m
        // behind the nose, |c| = 1.5, r 0.3): excluded.
        let walker = Footprint::Circle {
            centre: Vec2::new(1.5, 1.14),
            radius: 0.3,
        };
        assert_eq!(gap(&road(vec![walker], vec![])), None);
        // A walker 5 m ahead, 1.4 m aside (0.1 m into the band).
        let walker = Footprint::Circle {
            centre: Vec2::new(1.4, -5.0),
            radius: 0.3,
        };
        let g = gap(&road(vec![walker], vec![])).unwrap();
        assert!((g - (5.0 - (0.09f32 - 0.04).sqrt())).abs() < 1e-4, "{g}");
    }

    #[test]
    fn claim_rows() {
        let rect = FlatRect::of(
            Vec3::new(0.0, 0.0, -15.0),
            Quat::IDENTITY,
            Vec2::new(1.7, 8.0),
        );
        let claim = |dir: Vec2| Claim {
            owner: Entity::from_bits(99),
            rect,
            dir,
        };
        // Same direction: ignored by the strip (a follower sees the passer's body, not its claim).
        assert_eq!(gap(&road(vec![], vec![claim(Vec2::NEG_Y)])), None);
        // Opposite: seen at its near end.
        let hit = road(vec![], vec![claim(Vec2::Y)])
            .first_along(&strip(), |_| false)
            .unwrap();
        assert!(hit.claim && (hit.gap - 7.0).abs() < 1e-4, "{hit:?}");
        // Overlap queries see claims by filter.
        let r = road(vec![], vec![claim(Vec2::Y)]);
        let probe = FlatRect::of(
            Vec3::new(0.0, 0.0, -10.0),
            Quat::IDENTITY,
            Vec2::new(1.2, 2.0),
        );
        assert!(r.blocked(&probe, |_| false, ClaimFilter::All).is_some());
        assert!(r.blocked(&probe, |_| false, ClaimFilter::None).is_none());
        assert!(
            r.blocked(
                &probe,
                |_| false,
                ClaimFilter::Except(Entity::from_bits(99))
            )
            .is_none()
        );
    }

    #[test]
    fn overlap_rows() {
        let r = road(vec![car(Vec2::ZERO)], vec![]);
        let at =
            |x: f32| FlatRect::of(Vec3::new(x, 0.0, 0.0), Quat::IDENTITY, Vec2::new(1.2, 2.04));
        // Touching side by side (2.4 m apart) is not an overlap; 2.3 m is.
        assert!(r.blocked(&at(2.4), |_| false, ClaimFilter::None).is_none());
        assert!(r.blocked(&at(2.3), |_| false, ClaimFilter::None).is_some());
        assert!(r.blocked(&at(2.3), |_| true, ClaimFilter::None).is_none());
    }
}
