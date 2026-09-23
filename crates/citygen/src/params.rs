use crate::RoadClass;
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CityParams {
    pub size: f32,
    pub ground_margin: f32,
    pub edge_wall: EdgeWallParams,
    pub grid: GridParams,
    pub roads: RoadParams,
    pub superblock_chance: f32,
    pub alley_chance: f32,
    pub districts: DistrictsParams,
    pub split_jitter: f32,
    pub min_building_area: f32,
    pub pois: PoiParams,
    pub massing: MassingParams,
    pub landmarks: LandmarkParams,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EdgeWallParams {
    pub height: f32,
    pub thickness: f32,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GridParams {
    pub core_radius: f32,
    pub core_block: (f32, f32),
    pub outer_block: (f32, f32),
    pub node_jitter: f32,
    pub avenue_every: u32,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RoadParams {
    pub lane_width: f32,
    pub avenue: RoadClassParams,
    pub street: RoadClassParams,
    pub alley_width: f32,
    pub curb_height: f32,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RoadClassParams {
    pub lanes_per_direction: u32,
    pub sidewalk: f32,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DistrictsParams {
    pub grid: u32,
    pub seed_jitter: f32,
    pub weights: DistrictWeights,
    pub downtown: DistrictParams,
    pub commercial: DistrictParams,
    pub residential: DistrictParams,
    pub industrial: DistrictParams,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DistrictWeights {
    pub commercial: f32,
    pub residential: f32,
    pub industrial: f32,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DistrictParams {
    pub floors: (u32, u32),
    pub floor_height: f32,
    pub park_share: f32,
    pub lot_area: (f32, f32),
    pub min_frontage: f32,
    pub setback: f32,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PoiParams {
    pub hospital_districts: Vec<DistrictKind>,
    pub police_districts: Vec<DistrictKind>,
    pub police_stations: u32,
}
/// Setback tiers of tall buildings: a tier every `setback_tier_floors` floors from
/// `setback_min_floors` up, each inset by `setback_inset` until a half extent drops below `setback_min_half`.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct MassingParams {
    pub setback_min_floors: u32,
    pub setback_tier_floors: u32,
    pub setback_inset: f32,
    pub setback_min_half: f32,
}
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LandmarkParams {
    pub tower_floors: u32,
    pub tower_footprint: f32,
    pub park_radius: f32,
}
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistrictKind {
    Downtown,
    Commercial,
    Residential,
    Industrial,
}

fn positive(name: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be finite and positive"))
    }
}
fn probability(name: &str, value: f32) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("{name} must be in [0, 1]"))
    }
}
impl CityParams {
    pub fn district(&self, kind: DistrictKind) -> &DistrictParams {
        match kind {
            DistrictKind::Downtown => &self.districts.downtown,
            DistrictKind::Commercial => &self.districts.commercial,
            DistrictKind::Residential => &self.districts.residential,
            DistrictKind::Industrial => &self.districts.industrial,
        }
    }
    pub fn half_carriageway(&self, class: RoadClass) -> f32 {
        match class {
            RoadClass::Avenue => {
                self.roads.avenue.lanes_per_direction as f32 * self.roads.lane_width
            }
            RoadClass::Street => {
                self.roads.street.lanes_per_direction as f32 * self.roads.lane_width
            }
            RoadClass::Alley => self.roads.alley_width / 2.0,
        }
    }
    pub fn sidewalk(&self, class: RoadClass) -> f32 {
        match class {
            RoadClass::Avenue => self.roads.avenue.sidewalk,
            RoadClass::Street => self.roads.street.sidewalk,
            RoadClass::Alley => 0.0,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        positive("size", self.size)?;
        positive("ground_margin", self.ground_margin)?;
        positive("edge_wall.height", self.edge_wall.height)?;
        positive("edge_wall.thickness", self.edge_wall.thickness)?;
        positive("grid.core_radius", self.grid.core_radius)?;
        if self.grid.core_radius >= self.size / 2.0 {
            return Err("grid.core_radius must be smaller than size/2".into());
        }
        for (name, pair) in [
            ("grid.core_block", self.grid.core_block),
            ("grid.outer_block", self.grid.outer_block),
        ] {
            positive(&format!("{name}.0"), pair.0)?;
            positive(&format!("{name}.1"), pair.1)?;
            if pair.0 > pair.1 {
                return Err(format!("{name} min exceeds max"));
            }
        }
        if !crate::grid::split_feasible(
            self.grid.core_radius,
            self.grid.core_block.0,
            self.grid.core_block.1,
        ) {
            return Err("grid.core_radius cannot be split into core_block range".into());
        }
        if !crate::grid::split_feasible(
            self.size / 2.0 - self.grid.core_radius,
            self.grid.outer_block.0,
            self.grid.outer_block.1,
        ) {
            return Err("grid.outer_block cannot split outskirts interval".into());
        }
        if !self.grid.node_jitter.is_finite()
            || self.grid.node_jitter < 0.0
            || 4.0 * self.grid.node_jitter >= self.grid.core_block.0.min(self.grid.outer_block.0)
        {
            return Err("grid.node_jitter too large".into());
        }
        if self.grid.avenue_every == 0 {
            return Err("grid.avenue_every must be positive".into());
        }
        positive("roads.lane_width", self.roads.lane_width)?;
        positive("roads.alley_width", self.roads.alley_width)?;
        positive("roads.curb_height", self.roads.curb_height)?;
        for (name, road) in [
            ("avenue", &self.roads.avenue),
            ("street", &self.roads.street),
        ] {
            if road.lanes_per_direction == 0 {
                return Err(format!("roads.{name}.lanes_per_direction must be positive"));
            }
            positive(&format!("roads.{name}.sidewalk"), road.sidewalk)?;
        }
        if self.grid.core_block.0.min(self.grid.outer_block.0)
            <= 2.0 * (self.half_carriageway(RoadClass::Avenue) + self.sidewalk(RoadClass::Avenue))
        {
            return Err("grid block minimum must exceed avenue width and sidewalks".into());
        }
        probability("superblock_chance", self.superblock_chance)?;
        probability("alley_chance", self.alley_chance)?;
        if self.superblock_chance + self.alley_chance > 1.0 {
            return Err("superblock_chance + alley_chance exceeds 1".into());
        }
        probability("split_jitter", self.split_jitter)?;
        if self.split_jitter >= 0.5 {
            return Err("split_jitter must be < 0.5".into());
        }
        if self.districts.grid < 3 || self.districts.grid.is_multiple_of(2) {
            return Err("districts.grid must be odd and at least 3".into());
        }
        if !self.districts.seed_jitter.is_finite()
            || !(0.0..0.5).contains(&self.districts.seed_jitter)
        {
            return Err("districts.seed_jitter must be in [0, 0.5)".into());
        }
        let w = &self.districts.weights;
        if [w.commercial, w.residential, w.industrial]
            .into_iter()
            .any(|x| !x.is_finite() || x < 0.0)
            || w.commercial + w.residential + w.industrial <= 0.0
        {
            return Err("districts.weights must be nonnegative with positive sum".into());
        }
        for kind in [
            DistrictKind::Downtown,
            DistrictKind::Commercial,
            DistrictKind::Residential,
            DistrictKind::Industrial,
        ] {
            let d = self.district(kind);
            if d.floors.0 == 0 || d.floors.0 > d.floors.1 {
                return Err(format!("{kind:?}.floors invalid"));
            }
            positive(&format!("{kind:?}.floor_height"), d.floor_height)?;
            probability(&format!("{kind:?}.park_share"), d.park_share)?;
            positive(&format!("{kind:?}.lot_area.0"), d.lot_area.0)?;
            positive(&format!("{kind:?}.lot_area.1"), d.lot_area.1)?;
            if d.lot_area.0 > d.lot_area.1 {
                return Err(format!("{kind:?}.lot_area invalid"));
            }
            positive(&format!("{kind:?}.min_frontage"), d.min_frontage)?;
            if !d.setback.is_finite() || d.setback < 0.0 {
                return Err(format!("{kind:?}.setback invalid"));
            }
        }
        positive("min_building_area", self.min_building_area)?;
        if self.pois.hospital_districts.is_empty() || self.pois.police_districts.is_empty() {
            return Err("pois district preferences must be nonempty".into());
        }
        if !(1..=2).contains(&self.pois.police_stations) {
            return Err("pois.police_stations must be 1 or 2".into());
        }
        let m = &self.massing;
        if m.setback_min_floors == 0 || m.setback_tier_floors == 0 {
            return Err(
                "massing.setback_min_floors and setback_tier_floors must be at least 1".into(),
            );
        }
        positive("massing.setback_inset", m.setback_inset)?;
        positive("massing.setback_min_half", m.setback_min_half)?;
        let l = &self.landmarks;
        positive("landmarks.tower_footprint", l.tower_footprint)?;
        positive("landmarks.park_radius", l.park_radius)?;
        let tallest = [
            DistrictKind::Downtown,
            DistrictKind::Commercial,
            DistrictKind::Residential,
            DistrictKind::Industrial,
        ]
        .into_iter()
        .map(|kind| {
            let d = self.district(kind);
            d.floors.1 as f32 * d.floor_height
        })
        .fold(0.0, f32::max);
        if l.tower_floors as f32 * self.districts.downtown.floor_height <= tallest {
            return Err("landmarks.tower_floors must make the tower the tallest building".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> CityParams {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/world/city.ron");
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("GATE BROKEN: cannot read {path}: {e}"));
        let params: CityParams =
            ron::from_str(&text).unwrap_or_else(|e| panic!("GATE BROKEN: {path}: {e}"));
        params
            .validate()
            .unwrap_or_else(|e| panic!("GATE BROKEN: {path}: {e}"));
        params
    }

    #[test]
    fn validate_rejects_bad_params() {
        let mut p = shipped();
        p.grid.core_radius = 91.0;
        assert!(p.validate().unwrap_err().contains("grid.core_radius"));

        let mut p = shipped();
        p.superblock_chance = 0.6;
        p.alley_chance = 0.6;
        assert!(
            p.validate()
                .unwrap_err()
                .contains("superblock_chance + alley_chance")
        );

        let mut p = shipped();
        p.districts.grid = 4;
        assert!(p.validate().unwrap_err().contains("districts.grid"));

        let mut p = shipped();
        p.landmarks.tower_floors = 30;
        assert!(p.validate().unwrap_err().contains("landmarks.tower_floors"));
    }
}
