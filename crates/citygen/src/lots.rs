use crate::{
    Block, Building, BuildingKind, CityParams, District, DistrictParams, GenError, Landmarks, Lot,
    MassingParams, RoadClass, RoadGraph, Tier, Vec2, geom, landmarks, rng,
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

/// Marks parks and landmarks, subdivides the remaining blocks into lots and fits one box building
/// per lot; the plaza block is a single lot holding the tower.
pub(crate) fn build(
    seed: u64,
    params: &CityParams,
    blocks: &mut [Block],
    districts: &[District],
    roads: &RoadGraph,
) -> Result<(Vec<Lot>, Vec<Building>, Landmarks), GenError> {
    let protected = protected_blocks(blocks, districts.len());
    mark_parks(seed, params, blocks, districts, &protected);
    let (plaza, park) = landmarks::choose(params, districts, roads, blocks, &protected)?;
    blocks[park].is_park = true;
    blocks[plaza].is_park = false;
    let mut lots = Vec::new();
    let mut buildings = Vec::new();
    let mut tower = None;
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
        if block_id == plaza {
            let lot_id = lots.len() as u32;
            let building = fit_tower(params, lot_id, &root)
                .ok_or(GenError::NoLandmarkCandidate { landmark: "tower" })?;
            tower = Some(buildings.len() as u32);
            buildings.push(building);
            lots.push(Lot {
                block: block_id as u32,
                polygon: root.polygon,
            });
            continue;
        }
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
    let tower = tower.ok_or(GenError::NoLandmarkCandidate { landmark: "tower" })?;
    let landmarks = Landmarks {
        plaza: plaza as u32,
        park: park as u32,
        tower,
    };
    Ok((lots, buildings, landmarks))
}

/// The lowest-id buildable block of every district, which never becomes a park.
pub(crate) fn protected_blocks(blocks: &[Block], district_count: usize) -> Vec<Option<usize>> {
    let mut protected = vec![None; district_count];
    for (id, block) in blocks.iter().enumerate() {
        let slot = &mut protected[block.district as usize];
        if !block.inner.is_empty() && slot.is_none() {
            *slot = Some(id);
        }
    }
    protected
}

fn mark_parks(
    seed: u64,
    params: &CityParams,
    blocks: &mut [Block],
    districts: &[District],
    protected: &[Option<usize>],
) {
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

/// Shrinks `half` until the rectangle at `center` with axes `u`, `u.perp()` fits inside `shrunk`.
fn shrink_to_fit(shrunk: &[Vec2], center: Vec2, u: Vec2, half: Vec2) -> Option<Vec2> {
    let v = u.perp();
    let corners_inside = |half: Vec2| {
        [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
            .into_iter()
            .all(|(su, sv)| {
                let corner = center + u * (su * half.x) + v * (sv * half.y);
                geom::contains_convex(shrunk, corner, FIT_EPS)
            })
    };
    let mut half = half;
    let mut steps = 0;
    while !corners_inside(half) {
        if steps == FIT_STEPS {
            return None;
        }
        half *= FIT_SHRINK;
        steps += 1;
    }
    Some(half)
}

/// Setback tiers of a building with `floors` floors and base half extents `base`.
fn tiers(m: &MassingParams, floors: u32, floor_h: f32, base: Vec2) -> Vec<Tier> {
    let mut out = Vec::new();
    if floors < m.setback_min_floors {
        return out;
    }
    let mut prev = base;
    let mut k = 1;
    while k * m.setback_tier_floors < floors {
        let next = prev - Vec2::splat(m.setback_inset);
        if next.x < m.setback_min_half || next.y < m.setback_min_half {
            break;
        }
        out.push(Tier {
            bottom: (k * m.setback_tier_floors) as f32 * floor_h,
            half_extents: next,
        });
        prev = next;
        k += 1;
    }
    out
}

fn fit_tower(params: &CityParams, lot: u32, shape: &LotShape) -> Option<Building> {
    let downtown = &params.districts.downtown;
    let (a, b) = longest_frontage(shape)?;
    let u = (b - a).normalize();
    let offsets = vec![downtown.setback; shape.polygon.len()];
    let shrunk = geom::inset(&shape.polygon, &offsets)?;
    let center = geom::centroid(&shrunk);
    let footprint = Vec2::splat(params.landmarks.tower_footprint / 2.0);
    let half = shrink_to_fit(&shrunk, center, u, footprint)?;
    let floors = params.landmarks.tower_floors;
    Some(Building {
        lot,
        center,
        axis: u,
        half_extents: half,
        height: floors as f32 * downtown.floor_height,
        kind: BuildingKind::Tower,
        upper_tiers: tiers(&params.massing, floors, downtown.floor_height, half),
    })
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
    let half = shrink_to_fit(&shrunk, center, u, (hi - lo) / 2.0)?;
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
        upper_tiers: tiers(&params.massing, floors, district.floor_height, half),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn massing() -> MassingParams {
        MassingParams {
            setback_min_floors: 12,
            setback_tier_floors: 8,
            setback_inset: 3.0,
            setback_min_half: 6.0,
        }
    }

    #[test]
    fn tiers_worked_examples() {
        let t = tiers(&massing(), 30, 3.9, Vec2::new(15.0, 12.0));
        assert_eq!(t.len(), 2);
        assert!((t[0].bottom - 31.2).abs() < 1e-4 && t[0].half_extents == Vec2::new(12.0, 9.0));
        assert!((t[1].bottom - 62.4).abs() < 1e-4 && t[1].half_extents == Vec2::new(9.0, 6.0));
        let tower = tiers(&massing(), 40, 3.9, Vec2::splat(16.0));
        let halves = tower.iter().map(|t| t.half_extents.x).collect::<Vec<_>>();
        assert_eq!(halves, [13.0, 10.0, 7.0]);
        assert!((tower[2].bottom - 93.6).abs() < 1e-4);
        assert!(tiers(&massing(), 8, 3.9, Vec2::splat(20.0)).is_empty());
    }
}
