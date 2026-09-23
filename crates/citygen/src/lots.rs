use crate::{
    Block, Building, BuildingKind, CityParams, District, DistrictParams, Lot, RoadClass, RoadGraph,
    Vec2, geom, rng,
};
use rand_chacha::ChaCha8Rng;

// Recursion bound of the lot subdivision (algorithm law, not tuning).
const MAX_SPLIT_DEPTH: u32 = 16;
// Building fitting: shrink factor and step count (algorithm law, not tuning).
const FIT_SHRINK: f32 = 0.9;
const FIT_STEPS: u32 = 24;
// Containment tolerance (m) for the fitted rectangle corners.
const FIT_EPS: f32 = 1e-4;

struct LotShape {
    polygon: Vec<Vec2>,
    frontage: Vec<bool>,
}

/// Marks parks, subdivides the remaining blocks into lots and fits one box building per lot.
pub(crate) fn build(
    seed: u64,
    params: &CityParams,
    blocks: &mut [Block],
    districts: &[District],
    roads: &RoadGraph,
) -> (Vec<Lot>, Vec<Building>) {
    mark_parks(seed, params, blocks, districts);
    let mut lots = Vec::new();
    let mut buildings = Vec::new();
    for (block_id, block) in blocks.iter().enumerate() {
        if block.is_park {
            continue;
        }
        let district = params.district(districts[block.district as usize].kind);
        let frontage = block
            .sides
            .iter()
            .map(|&s| roads.edges[s as usize].class != RoadClass::Alley)
            .collect();
        let root = LotShape {
            polygon: block.inner.clone(),
            frontage,
        };
        let mut r = rng::stream(seed, rng::LOTS, block_id as u64);
        let mut leaves = Vec::new();
        subdivide(root, 0, params.split_jitter, district, &mut r, &mut leaves);
        for shape in leaves {
            let lot_id = lots.len() as u32;
            if let Some(building) = fit_building(seed, params, district, lot_id, &shape) {
                buildings.push(building);
            }
            lots.push(Lot {
                block: block_id as u32,
                polygon: shape.polygon,
            });
        }
    }
    (lots, buildings)
}

// The lowest-id buildable block of every district never becomes a park.
fn mark_parks(seed: u64, params: &CityParams, blocks: &mut [Block], districts: &[District]) {
    let mut protected = vec![None; districts.len()];
    for (id, block) in blocks.iter().enumerate() {
        let slot = &mut protected[block.district as usize];
        if !block.inner.is_empty() && slot.is_none() {
            *slot = Some(id);
        }
    }
    for (id, block) in blocks.iter_mut().enumerate() {
        if block.inner.is_empty() || protected[block.district as usize] == Some(id) {
            continue;
        }
        let share = params
            .district(districts[block.district as usize].kind)
            .park_share;
        let mut r = rng::stream(seed, rng::PARKS, id as u64);
        block.is_park = rng::chance(&mut r, share);
    }
}

fn longest_frontage(shape: &LotShape) -> Option<(Vec2, Vec2)> {
    let n = shape.polygon.len();
    (0..n)
        .filter(|&i| shape.frontage[i])
        .map(|i| (shape.polygon[i], shape.polygon[(i + 1) % n]))
        .max_by(|a, b| {
            (a.1 - a.0)
                .length_squared()
                .total_cmp(&(b.1 - b.0).length_squared())
        })
}

fn acceptable(shape: &LotShape, district: &DistrictParams) -> bool {
    let frontage = longest_frontage(shape).map_or(0.0, |(a, b)| (b - a).length());
    geom::signed_area(&shape.polygon) >= district.lot_area.0 && frontage >= district.min_frontage
}

fn subdivide(
    shape: LotShape,
    depth: u32,
    split_jitter: f32,
    district: &DistrictParams,
    r: &mut ChaCha8Rng,
    out: &mut Vec<LotShape>,
) {
    if depth >= MAX_SPLIT_DEPTH || geom::signed_area(&shape.polygon) <= district.lot_area.1 {
        out.push(shape);
        return;
    }
    let (u, min, max) = geom::min_area_obb(&shape.polygon);
    let jitter = rng::range_f32(r, -split_jitter, split_jitter);
    let along_u = (u, min.x, max.x);
    let along_v = (u.perp(), min.y, max.y);
    let (long, short) = if max.x - min.x >= max.y - min.y {
        (along_u, along_v)
    } else {
        (along_v, along_u)
    };
    for (axis, lo, hi) in [long, short] {
        let point = axis * ((lo + hi) / 2.0 + jitter * (hi - lo));
        let (a, fa) = geom::clip_half_plane(&shape.polygon, &shape.frontage, point, axis);
        let (b, fb) = geom::clip_half_plane(&shape.polygon, &shape.frontage, point, -axis);
        let first = LotShape {
            polygon: a,
            frontage: fa,
        };
        let second = LotShape {
            polygon: b,
            frontage: fb,
        };
        if acceptable(&first, district) && acceptable(&second, district) {
            subdivide(first, depth + 1, split_jitter, district, r, out);
            subdivide(second, depth + 1, split_jitter, district, r, out);
            return;
        }
    }
    out.push(shape);
}

fn fit_building(
    seed: u64,
    params: &CityParams,
    district: &DistrictParams,
    lot: u32,
    shape: &LotShape,
) -> Option<Building> {
    let (a, b) = longest_frontage(shape)?;
    let u = (b - a).normalize();
    let v = u.perp();
    let offsets = vec![district.setback; shape.polygon.len()];
    let shrunk = geom::inset(&shape.polygon, &offsets)?;
    let (mut lo, mut hi) = (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY));
    for p in &shrunk {
        let q = Vec2::new(p.dot(u), p.dot(v));
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let center = geom::centroid(&shrunk);
    let mut half = (hi - lo) / 2.0;
    let corners_inside = |half: Vec2| {
        [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
            .into_iter()
            .all(|(su, sv)| {
                let corner = center + u * (su * half.x) + v * (sv * half.y);
                geom::contains_convex(&shrunk, corner, FIT_EPS)
            })
    };
    let mut steps = 0;
    while !corners_inside(half) {
        if steps == FIT_STEPS {
            return None;
        }
        half *= FIT_SHRINK;
        steps += 1;
    }
    if 4.0 * half.x * half.y < params.min_building_area {
        return None;
    }
    let mut r = rng::stream(seed, rng::BUILDINGS, u64::from(lot));
    let floors = rng::range_u32(&mut r, district.floors.0, district.floors.1);
    Some(Building {
        lot,
        center,
        axis: u,
        half_extents: half,
        height: floors as f32 * district.floor_height,
        kind: BuildingKind::Generic,
    })
}
