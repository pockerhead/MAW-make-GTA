//! Deterministic, renderer-independent city layout generation.

mod districts;
mod geom;
mod graphs;
mod grid;
mod hash;
mod landmarks;
mod layout;
mod lots;
pub mod minimap;
mod params;
mod pois;
mod rng;
mod roads;

pub use geom::{centroid, contains_convex, convex_overlap, dist_point_segment};
pub use glam::Vec2;
pub use graphs::sidewalk_anchor;
pub use hash::layout_hash;
pub use layout::*;
pub use params::*;

/// Generates a city layout. `params` must have passed `CityParams::validate` (the loader does it);
/// the result is a pure function of `seed` and `params`.
pub fn generate(seed: u64, params: &CityParams) -> Result<CityLayout, GenError> {
    let lines = grid::build(seed, params);
    let (roads, mut blocks) = roads::build(seed, params, &lines);
    let districts = districts::assign(seed, params, &roads, &mut blocks)?;
    let (lots, mut buildings, landmarks) =
        lots::build(seed, params, &mut blocks, &districts, &roads)?;
    let gang_districts = pois::assign(
        seed,
        &params.pois,
        &blocks,
        &lots,
        &mut buildings,
        &districts,
    )?;
    let (sidewalks, lanes, player_spawn) = graphs::build(params, &roads, &blocks);
    Ok(CityLayout {
        seed,
        size: params.size,
        ground_size: params.size + 2.0 * params.ground_margin,
        roads,
        districts,
        blocks,
        lots,
        buildings,
        gang_districts,
        sidewalks,
        lanes,
        player_spawn,
        landmarks,
    })
}
