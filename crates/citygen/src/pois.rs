use crate::{Block, Building, BuildingKind, District, DistrictKind, GenError, Lot, PoiParams, rng};

/// Assigns gang headquarters, the hospital and police stations to existing buildings.
/// Returns the two gang districts.
pub(crate) fn assign(
    seed: u64,
    pois: &PoiParams,
    blocks: &[Block],
    lots: &[Lot],
    buildings: &mut [Building],
    districts: &[District],
) -> Result<[u32; 2], GenError> {
    let district_of = |b: &Building| blocks[lots[b.lot as usize].block as usize].district;
    let footprint = |b: &Building| 4.0 * b.half_extents.x * b.half_extents.y;
    let mut order = (0..buildings.len()).collect::<Vec<_>>();
    order.sort_by(|&a, &b| {
        footprint(&buildings[b])
            .total_cmp(&footprint(&buildings[a]))
            .then(a.cmp(&b))
    });
    let owner = order
        .iter()
        .map(|&i| district_of(&buildings[i]))
        .collect::<Vec<_>>();

    let mut gangs = (0..districts.len() as u32)
        .filter(|&d| {
            matches!(
                districts[d as usize].kind,
                DistrictKind::Residential | DistrictKind::Industrial
            )
        })
        .filter(|d| owner.contains(d))
        .collect::<Vec<_>>();
    let mut r = rng::stream(seed, rng::GANGS, 0);
    for i in (1..gangs.len()).rev() {
        let j = rng::range_u32(&mut r, 0, i as u32) as usize;
        gangs.swap(i, j);
    }
    if gangs.len() < 2 {
        return Err(GenError::NotEnoughGangDistricts {
            found: gangs.len() as u32,
        });
    }
    let gang_districts = [gangs[0], gangs[1]];
    for (index, &district) in gang_districts.iter().enumerate() {
        let slot = owner
            .iter()
            .position(|&d| d == district)
            .expect("gang district has a building");
        buildings[order[slot]].kind = BuildingKind::GangHq(index as u8);
    }

    let kind_of = |slot: usize| districts[owner[slot] as usize].kind;
    let mut place = |preferred: &[DistrictKind], kind: BuildingKind, poi: &'static str| {
        let free = |slot: &usize| buildings[order[*slot]].kind == BuildingKind::Generic;
        let slots = || (0..order.len()).filter(free);
        let slot = slots()
            .find(|&s| preferred.contains(&kind_of(s)))
            .or_else(|| slots().find(|&s| kind_of(s) != DistrictKind::Downtown))
            .or_else(|| slots().next())
            .ok_or(GenError::NoPoiCandidate { poi })?;
        buildings[order[slot]].kind = kind;
        Ok(())
    };
    place(&pois.hospital_districts, BuildingKind::Hospital, "hospital")?;
    for _ in 0..pois.police_stations {
        place(
            &pois.police_districts,
            BuildingKind::PoliceStation,
            "police",
        )?;
    }
    Ok(gang_districts)
}
