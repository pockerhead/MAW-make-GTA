use crate::{Block, CityParams, District, DistrictKind, GenError, RoadGraph, Vec2, rng};

pub(crate) fn assign(
    seed: u64,
    params: &CityParams,
    roads: &RoadGraph,
    blocks: &mut [Block],
) -> Result<Vec<District>, GenError> {
    let n = params.districts.grid as usize;
    let cell = params.size / n as f32;
    let mut districts = Vec::with_capacity(n * n);
    let weights = &params.districts.weights;
    let sum = weights.commercial + weights.residential + weights.industrial;
    for i in 0..n {
        for j in 0..n {
            let center = i == n / 2 && j == n / 2;
            let mut r = rng::stream(seed, rng::DISTRICTS, (i * n + j) as u64);
            let point = if center {
                Vec2::ZERO
            } else {
                Vec2::new(
                    -params.size / 2.0
                        + (i as f32 + 0.5) * cell
                        + rng::range_f32(
                            &mut r,
                            -params.districts.seed_jitter * cell,
                            params.districts.seed_jitter * cell,
                        ),
                    -params.size / 2.0
                        + (j as f32 + 0.5) * cell
                        + rng::range_f32(
                            &mut r,
                            -params.districts.seed_jitter * cell,
                            params.districts.seed_jitter * cell,
                        ),
                )
            };
            let roll = rng::unit_f32(&mut r) * sum;
            let kind = if center {
                DistrictKind::Downtown
            } else if roll < weights.commercial {
                DistrictKind::Commercial
            } else if roll < weights.commercial + weights.residential {
                DistrictKind::Residential
            } else {
                DistrictKind::Industrial
            };
            districts.push(District {
                kind,
                seed_point: point,
            });
        }
    }
    for block in blocks.iter_mut() {
        let point = crate::geom::centroid(
            &block
                .nodes
                .iter()
                .map(|&id| roads.nodes[id as usize])
                .collect::<Vec<_>>(),
        );
        block.district = districts
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let da = (a.1.seed_point - point).length_squared();
                let db = (b.1.seed_point - point).length_squared();
                da.total_cmp(&db).then(a.0.cmp(&b.0))
            })
            .unwrap()
            .0 as u32;
    }
    loop {
        let gang = districts
            .iter()
            .enumerate()
            .filter(|(i, district)| {
                matches!(
                    district.kind,
                    DistrictKind::Residential | DistrictKind::Industrial
                ) && blocks
                    .iter()
                    .any(|b| b.district as usize == *i && !b.inner.is_empty())
            })
            .count();
        if gang >= 2 {
            break;
        }
        let candidate = districts
            .iter()
            .enumerate()
            .filter(|(i, d)| {
                d.kind != DistrictKind::Downtown
                    && !matches!(d.kind, DistrictKind::Residential | DistrictKind::Industrial)
                    && blocks
                        .iter()
                        .any(|b| b.district as usize == *i && !b.inner.is_empty())
            })
            .map(|(i, _)| {
                (
                    i,
                    blocks
                        .iter()
                        .filter(|b| b.district as usize == i && !b.inner.is_empty())
                        .count(),
                )
            })
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
        let Some((index, _)) = candidate else {
            return Err(GenError::NotEnoughGangDistricts { found: gang as u32 });
        };
        districts[index].kind = DistrictKind::Residential;
    }
    Ok(districts)
}
