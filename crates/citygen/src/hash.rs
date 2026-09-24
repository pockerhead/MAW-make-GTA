use crate::{BuildingKind, CityLayout, DistrictKind, RoadClass, Vec2, rng::fnv1a64};

// Bumped on any change of the canonical encoding below.
const HASH_SCHEMA_VERSION: u32 = 3;
// Floats are hashed at 1 mm resolution so sub-millimetre noise does not count as a layout change.
const QUANTUM_PER_METRE: f32 = 1000.0;

fn quantize_mm(v: f32) -> i64 {
    (v * QUANTUM_PER_METRE).round() as i64
}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&quantize_mm(v).to_le_bytes());
    }
    fn vec2(&mut self, v: Vec2) {
        self.f32(v.x);
        self.f32(v.y);
    }
    fn len(&mut self, n: usize) {
        self.u32(n as u32);
    }
    fn vec2s(&mut self, vs: &[Vec2]) {
        self.len(vs.len());
        vs.iter().for_each(|&v| self.vec2(v));
    }
    fn u32s(&mut self, vs: &[u32]) {
        self.len(vs.len());
        vs.iter().for_each(|&v| self.u32(v));
    }
}

fn road_class(class: RoadClass) -> u8 {
    match class {
        RoadClass::Avenue => 0,
        RoadClass::Street => 1,
        RoadClass::Alley => 2,
    }
}

fn district_kind(kind: DistrictKind) -> u8 {
    match kind {
        DistrictKind::Downtown => 0,
        DistrictKind::Commercial => 1,
        DistrictKind::Residential => 2,
        DistrictKind::Industrial => 3,
    }
}

fn building_kind(kind: BuildingKind) -> (u8, u8) {
    match kind {
        BuildingKind::Generic => (0, 0),
        BuildingKind::Hospital => (1, 0),
        BuildingKind::PoliceStation => (2, 0),
        BuildingKind::GangHq(i) => (3, i),
        BuildingKind::Tower => (4, 0),
    }
}

/// Stable FNV-1a 64 hash of the canonical layout encoding (floats quantised to 1 mm).
pub fn layout_hash(layout: &CityLayout) -> u64 {
    let mut w = Writer::default();
    w.u32(HASH_SCHEMA_VERSION);
    w.u64(layout.seed);
    w.f32(layout.size);
    w.f32(layout.ground_size);

    w.u32(layout.roads.center);
    w.vec2s(&layout.roads.nodes);
    w.len(layout.roads.edges.len());
    for e in &layout.roads.edges {
        w.u32(e.a);
        w.u32(e.b);
        w.u8(road_class(e.class));
    }

    w.len(layout.districts.len());
    for d in &layout.districts {
        w.u8(district_kind(d.kind));
        w.vec2(d.seed_point);
    }

    w.len(layout.blocks.len());
    for b in &layout.blocks {
        w.u32(b.district);
        w.u8(u8::from(b.is_park));
        w.u32s(&b.nodes);
        w.u32s(&b.sides);
        w.vec2s(&b.curb);
        w.vec2s(&b.inner);
    }

    w.len(layout.lots.len());
    for lot in &layout.lots {
        w.u32(lot.block);
        w.vec2s(&lot.polygon);
    }

    w.len(layout.buildings.len());
    for b in &layout.buildings {
        w.u32(b.lot);
        w.vec2(b.center);
        w.vec2(b.axis);
        w.vec2(b.half_extents);
        w.f32(b.height);
        w.len(b.upper_tiers.len());
        for tier in &b.upper_tiers {
            w.f32(tier.bottom);
            w.vec2(tier.half_extents);
        }
        let (code, gang) = building_kind(b.kind);
        w.u8(code);
        w.u8(gang);
    }

    w.u32(layout.gang_districts[0]);
    w.u32(layout.gang_districts[1]);
    w.u32(layout.landmarks.plaza);
    w.u32(layout.landmarks.park);
    w.u32(layout.landmarks.tower);

    w.vec2s(&layout.sidewalks.nodes);
    w.len(layout.sidewalks.edges.len());
    for &(a, b) in &layout.sidewalks.edges {
        w.u32(a);
        w.u32(b);
    }

    w.len(layout.lanes.lanes.len());
    for lane in &layout.lanes.lanes {
        w.u32(lane.edge);
        w.vec2(lane.from);
        w.vec2(lane.to);
    }
    w.len(layout.lanes.connectors.len());
    for c in &layout.lanes.connectors {
        w.u32(c.from);
        w.u32(c.to);
        w.u32(c.intersection);
    }

    w.vec2(layout.player_spawn);

    w.len(layout.parking.len());
    for spot in &layout.parking {
        w.vec2(spot.position);
        w.vec2(spot.heading);
    }
    fnv1a64(&w.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_and_quantum_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
        assert_eq!(quantize_mm(1.2345), 1235);
        assert_eq!(quantize_mm(0.0005), 1);
        assert_eq!(quantize_mm(-0.0005), -1);
        assert_eq!(fnv1a64(&1235i64.to_le_bytes()), 0x08ce64bd5cc50a22);
    }
}
