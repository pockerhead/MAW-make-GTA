use crate::Vec2;

// Numeric tolerances prevent almost parallel street corners from producing unbounded insets.
const PARALLEL_EPS: f32 = 1e-6;
const AREA_EPS: f32 = 1e-4;

pub(crate) fn signed_area(poly: &[Vec2]) -> f32 {
    poly.iter()
        .enumerate()
        .map(|(i, &a)| a.perp_dot(poly[(i + 1) % poly.len()]))
        .sum::<f32>()
        / 2.0
}

pub(crate) fn centroid(poly: &[Vec2]) -> Vec2 {
    poly.iter().copied().sum::<Vec2>() / poly.len() as f32
}

pub(crate) fn is_convex(poly: &[Vec2], eps: f32) -> bool {
    poly.len() >= 3
        && signed_area(poly) > eps
        && (0..poly.len()).all(|i| {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            let c = poly[(i + 2) % poly.len()];
            (b - a).perp_dot(c - b) >= -eps
        })
}

pub(crate) fn inset(poly: &[Vec2], offsets: &[f32]) -> Option<Vec<Vec2>> {
    if poly.len() != offsets.len() || !is_convex(poly, AREA_EPS) {
        return None;
    }
    let mut lines = Vec::with_capacity(poly.len());
    for i in 0..poly.len() {
        let dir = (poly[(i + 1) % poly.len()] - poly[i]).normalize();
        lines.push((poly[i] + dir.perp() * offsets[i], dir));
    }
    let mut out = Vec::with_capacity(poly.len());
    for i in 0..poly.len() {
        let (p, d) = lines[(i + poly.len() - 1) % poly.len()];
        let (q, e) = lines[i];
        let cross = d.perp_dot(e);
        out.push(if cross.abs() < PARALLEL_EPS {
            q
        } else {
            p + d * ((q - p).perp_dot(e) / cross)
        });
    }
    if !is_convex(&out, AREA_EPS) {
        return None;
    }
    for (i, &(p, d)) in lines.iter().enumerate() {
        let normal = d.perp();
        if out.iter().any(|v| (*v - p).dot(normal) < -0.01) || offsets[i] < 0.0 {
            return None;
        }
    }
    Some(out)
}

/// Keeps the part of `poly` where `(p - point)·normal >= 0`; `flags[i]` belongs to side `i -> i+1`,
/// sides created by the cut get `false`.
pub(crate) fn clip_half_plane(
    poly: &[Vec2],
    flags: &[bool],
    point: Vec2,
    normal: Vec2,
) -> (Vec<Vec2>, Vec<bool>) {
    let mut out = Vec::new();
    let mut out_flags = Vec::new();
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        let da = (a - point).dot(normal);
        let db = (b - point).dot(normal);
        if da >= 0.0 {
            out.push(a);
            out_flags.push(flags[i]);
        }
        if (da >= 0.0) != (db >= 0.0) {
            out.push(a + (b - a) * (da / (da - db)));
            out_flags.push(da < 0.0 && flags[i]);
        }
    }
    dedup_ring(&mut out, &mut out_flags);
    (out, out_flags)
}

// A zero-length side is dropped together with its flag; the previous side keeps its own flag.
fn dedup_ring(poly: &mut Vec<Vec2>, flags: &mut Vec<bool>) {
    let mut i = 0;
    while poly.len() > 1 && i < poly.len() {
        let next = (i + 1) % poly.len();
        if (poly[next] - poly[i]).length_squared() < PARALLEL_EPS {
            poly.remove(i);
            flags.remove(i);
        } else {
            i += 1;
        }
    }
}

pub(crate) fn min_area_obb(poly: &[Vec2]) -> (Vec2, Vec2, Vec2) {
    let mut best = (Vec2::X, Vec2::ZERO, Vec2::ZERO);
    let mut best_area = f32::INFINITY;
    for i in 0..poly.len() {
        let u = (poly[(i + 1) % poly.len()] - poly[i]).normalize();
        let v = u.perp();
        let (mut min, mut max) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
        for p in poly {
            let q = Vec2::new(p.dot(u), p.dot(v));
            min = min.min(q);
            max = max.max(q);
        }
        let area = (max.x - min.x) * (max.y - min.y);
        if area < best_area {
            best = (u, min, max);
            best_area = area;
        }
    }
    best
}

/// True when `point` lies inside the counter-clockwise convex `poly` or within `eps` metres outside it.
pub fn contains_convex(poly: &[Vec2], point: Vec2, eps: f32) -> bool {
    poly.len() >= 3
        && (0..poly.len()).all(|i| {
            (poly[(i + 1) % poly.len()] - poly[i])
                .normalize()
                .perp_dot(point - poly[i])
                >= -eps
        })
}

/// Penetration depth of two convex polygons along the best separating axis; touching gives 0.
pub fn convex_overlap(a: &[Vec2], b: &[Vec2]) -> f32 {
    if a.len() < 3 || b.len() < 3 {
        return 0.0;
    }
    let mut depth = f32::INFINITY;
    for poly in [a, b] {
        for i in 0..poly.len() {
            let axis = (poly[(i + 1) % poly.len()] - poly[i]).perp().normalize();
            let project = |shape: &[Vec2]| -> (f32, f32) {
                shape
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                        (lo.min(p.dot(axis)), hi.max(p.dot(axis)))
                    })
            };
            let (al, ah) = project(a);
            let (bl, bh) = project(b);
            depth = depth.min(ah.min(bh) - al.max(bl));
            if depth <= 0.0 {
                return 0.0;
            }
        }
    }
    depth
}

/// Distance from `p` to the segment `a`-`b`.
pub fn dist_point_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let d = b - a;
    let t = ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
    (p - a - d * t).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(side: f32) -> [Vec2; 4] {
        [
            Vec2::ZERO,
            Vec2::new(side, 0.0),
            Vec2::splat(side),
            Vec2::new(0.0, side),
        ]
    }

    #[test]
    fn inset_square() {
        let inner = inset(&square(10.0), &[1.0; 4]).unwrap();
        assert!((signed_area(&inner) - 64.0).abs() < 1e-3);
        assert!(inset(&square(2.0), &[1.5; 4]).is_none());
    }

    #[test]
    fn inward_normals_of_unit_square() {
        let s = square(1.0);
        let normal = |i: usize| (s[(i + 1) % 4] - s[i]).perp();
        assert_eq!(normal(0), Vec2::new(0.0, 1.0));
        assert_eq!(normal(1), Vec2::new(-1.0, 0.0));
        assert_eq!(normal(3), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn clip_square_by_line() {
        let s = square(10.0);
        let flags = [true; 4];
        let at = Vec2::new(4.0, 0.0);
        let (left, left_flags) = clip_half_plane(&s, &flags, at, Vec2::NEG_X);
        let (right, right_flags) = clip_half_plane(&s, &flags, at, Vec2::X);
        assert!((signed_area(&left) - 40.0).abs() < 1e-3);
        assert!((signed_area(&right) - 60.0).abs() < 1e-3);
        for (poly, flags) in [(&left, &left_flags), (&right, &right_flags)] {
            assert_eq!(poly.len(), 4);
            let cut = (0..4).filter(|&i| {
                let (a, b) = (poly[i], poly[(i + 1) % 4]);
                (a.x - 4.0).abs() < 1e-4 && (b.x - 4.0).abs() < 1e-4
            });
            for i in cut {
                assert!(!flags[i]);
            }
            assert_eq!(flags.iter().filter(|f| !**f).count(), 1);
        }
    }

    #[test]
    fn overlap_depth() {
        let s = square(10.0);
        let touching = s.map(|p| p + Vec2::new(10.0, 0.0));
        assert_eq!(convex_overlap(&s, &touching), 0.0);
        let shifted = s.map(|p| p + Vec2::new(9.5, 0.0));
        assert!((convex_overlap(&s, &shifted) - 0.5).abs() < 1e-3);
    }
}
