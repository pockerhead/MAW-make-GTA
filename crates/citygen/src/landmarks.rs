use crate::{Block, CityParams, District, GenError, RoadClass, RoadGraph, geom};

/// Picks the plaza block (closest to the centre with a street frontage of its district's
/// `min_frontage`) and the park block (largest unprotected block within `park_radius`).
pub(crate) fn choose(
    params: &CityParams,
    districts: &[District],
    roads: &RoadGraph,
    blocks: &[Block],
    protected: &[Option<usize>],
) -> Result<(usize, usize), GenError> {
    let has_frontage = |block: &Block| {
        let min = params
            .district(districts[block.district as usize].kind)
            .min_frontage;
        let n = block.inner.len();
        (0..n).any(|k| {
            roads.edges[block.sides[k] as usize].class != RoadClass::Alley
                && (block.inner[(k + 1) % n] - block.inner[k]).length() >= min
        })
    };
    let plaza = blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| !b.inner.is_empty() && has_frontage(b))
        .map(|(id, b)| (id, geom::centroid(&b.inner).length()))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        .map(|(id, _)| id)
        .ok_or(GenError::NoLandmarkCandidate { landmark: "plaza" })?;
    let park = blocks
        .iter()
        .enumerate()
        .filter(|&(id, b)| {
            !b.inner.is_empty()
                && id != plaza
                && protected[b.district as usize] != Some(id)
                && geom::centroid(&b.inner).length() <= params.landmarks.park_radius
        })
        .map(|(id, b)| (id, geom::signed_area(&b.inner)))
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(id, _)| id)
        .ok_or(GenError::NoLandmarkCandidate { landmark: "park" })?;
    Ok((plaza, park))
}
