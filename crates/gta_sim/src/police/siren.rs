//! Sirens (GDD §5.3, TASK-032): a police car responding or chasing has them on; traffic ahead yields
//! to it and it may take any lane (the opposite one included) where the road ahead is clearer.

use super::cars::PoliceCarState;
use crate::occupancy::{RoadBody, RoadOccupancy, Strip, flat};
use crate::traffic::{TrafficGraph, lateral::right_of};
use bevy::prelude::*;
use serde::Deserialize;

/// Lane choice of a car with its sirens on.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SirenConfig {
    /// Lateral positions it may take, in lane pitches right of its own lane line (-1: the opposite
    /// lane).
    pub lane_offsets: Vec<f32>,
    /// Seconds a lane is kept before the next change.
    pub lane_hold_seconds: f32,
    /// Leaving its own lane needs this much more clear road ahead, m.
    pub lane_gain: f32,
}

impl SirenConfig {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(o) = self
            .lane_offsets
            .iter()
            .find(|o| !(o.is_finite() && o.abs() <= 1.0))
        {
            return Err(format!(
                "car.sirens.lane_offsets {o} must be finite, |o| <= 1"
            ));
        }
        for (field, value) in [
            ("car.sirens.lane_hold_seconds", self.lane_hold_seconds),
            ("car.sirens.lane_gain", self.lane_gain),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field} {value} must be finite and >= 0"));
            }
        }
        Ok(())
    }
}

/// The lateral position a siren car drives at (lane pitches right of its lane line) and how long it
/// has kept it.
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component, Default)]
pub struct SirenLane {
    pub offset: f32,
    pub held: f32,
}

/// Sirens are on while the car responds or chases (the audio emitter pick is client presentation).
pub fn sirens_on(state: PoliceCarState) -> bool {
    matches!(state, PoliceCarState::Respond | PoliceCarState::Chase)
}

/// The lane a car at `position` heading `forward` drives along (its line, direction and the pitch to
/// the opposite lane): the nearest lane pointing its way within one and a half pitches (so a car out
/// in the opposite lane keeps its own), out of the boxes.
pub(super) fn lane_frame(
    graph: &TrafficGraph,
    position: Vec3,
    forward: Vec3,
) -> Option<(Vec3, Vec3, f32)> {
    if graph.in_junction(position, 0.0) {
        return None;
    }
    graph
        .lanes()
        .iter()
        .filter(|lane| lane.dir.dot(forward) > std::f32::consts::FRAC_1_SQRT_2)
        .filter_map(|lane| {
            let pitch = lane.left_gap?;
            let s = (position - lane.from).dot(lane.dir).clamp(0.0, lane.length);
            let at = lane.from + lane.dir * s;
            let d = flat(position).distance(flat(at));
            (d <= 1.5 * pitch).then_some((d, (at, lane.dir, pitch)))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, frame)| frame)
}

/// Clear road ahead of the nose at `offset` pitches, m (the strip length when nothing is on it).
fn clear_ahead(road: &RoadOccupancy, strip: &Strip, skip: &impl Fn(&RoadBody) -> bool) -> f32 {
    road.first_along(strip, skip)
        .map_or(strip.length, |h| h.gap)
}

/// The strip ahead of a car's nose at `offset` pitches right of the lane line.
pub(super) fn strip_at(
    frame: (Vec3, Vec3, f32),
    nose: Vec3,
    offset: f32,
    length: f32,
    half_width: f32,
) -> Strip {
    let (line, dir, pitch) = frame;
    let along = (nose - line).with_y(0.0).dot(dir);
    Strip {
        origin: flat(line + dir * along + right_of(dir) * offset * pitch),
        dir: flat(dir).normalize_or_zero(),
        length,
        half_width,
    }
}

/// Picks the lateral position: leaving its own lane needs `lane_gain` more clear road there, coming
/// back only as much clear road as where it is; either after `lane_hold_seconds`.
pub(super) fn choose_lane(
    road: &RoadOccupancy,
    lane: &mut SirenLane,
    cfg: &SirenConfig,
    strip: impl Fn(f32) -> Strip,
    skip: impl Fn(&RoadBody) -> bool,
    dt: f32,
) {
    lane.held += dt;
    if lane.held < cfg.lane_hold_seconds {
        return;
    }
    let clear = |o: f32| clear_ahead(road, &strip(o), &skip);
    let home = clear(0.0);
    let pick = if lane.offset == 0.0 {
        cfg.lane_offsets
            .iter()
            .copied()
            .map(|o| (o, clear(o)))
            .filter(|&(_, c)| c - home > cfg.lane_gain)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(o, _)| o)
    } else {
        (home >= clear(lane.offset)).then_some(0.0)
    };
    if let Some(o) = pick.filter(|&o| o != lane.offset) {
        lane.offset = o;
        lane.held = 0.0;
    }
}

/// The nearest body in the chosen corridor: (gap, speed along) for IDM.
pub(super) fn corridor_obstacle(
    road: &RoadOccupancy,
    strip: &Strip,
    skip: impl Fn(&RoadBody) -> bool,
) -> Option<(f32, f32)> {
    road.first_along(strip, skip)
        .map(|h| (h.gap, h.speed_along))
}
