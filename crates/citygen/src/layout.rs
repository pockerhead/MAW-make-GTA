use crate::{DistrictKind, Vec2};
use std::fmt;

#[derive(Clone, Debug)]
pub struct CityLayout {
    pub seed: u64,
    pub size: f32,
    pub ground_size: f32,
    pub roads: RoadGraph,
    pub districts: Vec<District>,
    pub blocks: Vec<Block>,
    pub lots: Vec<Lot>,
    pub buildings: Vec<Building>,
    pub gang_districts: [u32; 2],
    pub sidewalks: WalkGraph,
    pub lanes: LaneGraph,
    pub player_spawn: Vec2,
}

#[derive(Clone, Debug)]
pub struct RoadGraph {
    pub nodes: Vec<Vec2>,
    pub edges: Vec<RoadEdge>,
    pub center: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct RoadEdge {
    pub a: u32,
    pub b: u32,
    pub class: RoadClass,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadClass {
    Avenue,
    Street,
    Alley,
}
#[derive(Clone, Debug)]
pub struct District {
    pub kind: DistrictKind,
    pub seed_point: Vec2,
}
#[derive(Clone, Debug)]
pub struct Block {
    pub district: u32,
    pub nodes: Vec<u32>,
    pub sides: Vec<u32>,
    pub curb: Vec<Vec2>,
    pub inner: Vec<Vec2>,
    pub is_park: bool,
}
#[derive(Clone, Debug)]
pub struct Lot {
    pub block: u32,
    pub polygon: Vec<Vec2>,
}
#[derive(Clone, Debug)]
pub struct Building {
    pub lot: u32,
    pub center: Vec2,
    pub axis: Vec2,
    pub half_extents: Vec2,
    pub height: f32,
    pub kind: BuildingKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingKind {
    Generic,
    Hospital,
    PoliceStation,
    GangHq(u8),
}
#[derive(Clone, Debug)]
pub struct WalkGraph {
    pub nodes: Vec<Vec2>,
    pub edges: Vec<(u32, u32)>,
}
#[derive(Clone, Debug)]
pub struct LaneGraph {
    pub lanes: Vec<Lane>,
    pub connectors: Vec<Connector>,
}
#[derive(Clone, Copy, Debug)]
pub struct Lane {
    pub edge: u32,
    pub from: Vec2,
    pub to: Vec2,
}
#[derive(Clone, Copy, Debug)]
pub struct Connector {
    pub from: u32,
    pub to: u32,
    pub intersection: u32,
}

#[derive(Debug)]
pub enum GenError {
    NotEnoughGangDistricts { found: u32 },
    NoPoiCandidate { poi: &'static str },
}
impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnoughGangDistricts { found } => {
                write!(f, "need two gang districts with buildings; found {found}")
            }
            Self::NoPoiCandidate { poi } => write!(f, "no building candidate for {poi}"),
        }
    }
}
impl std::error::Error for GenError {}
