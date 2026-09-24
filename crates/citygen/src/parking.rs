use crate::{
    CityParams, ParkingSpot, RoadClass, RoadGraph,
    rng::{PARKING, chance, stream},
};

/// Parked-car spots on the outer lane of every avenue, both directions (right-hand traffic).
pub(crate) fn spots(seed: u64, params: &CityParams, roads: &RoadGraph) -> Vec<ParkingSpot> {
    let cfg = &params.parking;
    let from_centre = params.half_carriageway(RoadClass::Avenue) - cfg.curb_offset;
    let mut spots = Vec::new();
    for (e, edge) in roads.edges.iter().enumerate() {
        if edge.class != RoadClass::Avenue {
            continue;
        }
        let mut rng = stream(seed, PARKING, e as u64);
        for (from, to) in [(edge.a, edge.b), (edge.b, edge.a)] {
            let (p, q) = (roads.nodes[from as usize], roads.nodes[to as usize]);
            let len = (q - p).length();
            let d = (q - p) / len;
            // Same frame as `graphs::lanes`: d.perp() is the driver's right.
            let right = d.perp();
            let mut s = cfg.end_margin;
            while s <= len - cfg.end_margin {
                if chance(&mut rng, cfg.chance) {
                    spots.push(ParkingSpot {
                        position: p + d * s + right * from_centre,
                        heading: d,
                    });
                }
                s += cfg.spacing;
            }
        }
    }
    spots
}
