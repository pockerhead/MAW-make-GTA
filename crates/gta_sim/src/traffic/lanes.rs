//! Road facts per traffic lane that the go-around and the siren yield read: the opposite inner lane
//! beside it (a street or avenue centre line) and whether a curb lane runs on its right (an avenue).

use super::TrafficLane;
use super::graph::lane_slot;
use crate::world::CityLayout;
use bevy::prelude::*;

/// Lateral distance to the nearest antiparallel lane whose line lies on the left of each lane at more
/// than one and at most two car widths and overlaps it along the lane, m.
pub(super) fn left_gaps(lanes: &[TrafficLane], half_width: f32) -> Vec<Option<f32>> {
    lanes
        .iter()
        .map(|lane| {
            let left = Vec3::new(lane.dir.z, 0.0, -lane.dir.x);
            lanes
                .iter()
                .filter(|other| other.dir.dot(lane.dir) < -0.99)
                .filter_map(|other| {
                    let gap = (other.from - lane.from).dot(left);
                    let (a, b) = (
                        (other.from - lane.from).dot(lane.dir),
                        (other.to - lane.from).dot(lane.dir),
                    );
                    let overlap = a.max(b).min(lane.length) - a.min(b).max(0.0);
                    (2.0 * half_width < gap && gap <= 4.0 * half_width && overlap > 0.0)
                        .then_some(gap)
                })
                .min_by(f32::total_cmp)
        })
        .collect()
}

/// For each layout lane kept as traffic lane `index[k]`: a same-direction slot-1 lane (a curb lane)
/// runs on the same road edge.
pub(super) fn curb_lanes(layout: &CityLayout, lane_width: f32, index: &[Option<u32>]) -> Vec<u32> {
    let roads = &layout.roads;
    let slot = |k: usize| {
        let lane = &layout.lanes.lanes[k];
        let edge = roads.edges[lane.edge as usize];
        let (a, b) = (roads.nodes[edge.a as usize], roads.nodes[edge.b as usize]);
        lane_slot(
            (b - a).normalize().perp_dot(lane.from - a).abs(),
            lane_width,
        )
    };
    let dir = |k: usize| {
        let lane = &layout.lanes.lanes[k];
        (lane.to - lane.from).normalize_or_zero()
    };
    let all = &layout.lanes.lanes;
    index
        .iter()
        .enumerate()
        .filter_map(|(k, kept)| {
            let kept = (*kept)?;
            all.iter()
                .enumerate()
                .any(|(j, other)| {
                    j != k && other.edge == all[k].edge && dir(j).dot(dir(k)) > 0.99 && slot(j) == 1
                })
                .then_some(kept)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigRoot, load_config};
    use crate::traffic::{TRAFFIC_CONFIG, TrafficConfig, TrafficGraph};

    fn cfg() -> TrafficConfig {
        let root = ConfigRoot(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into());
        load_config(&root, TRAFFIC_CONFIG).expect("GATE BROKEN: traffic.ron")
    }

    fn at(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, z)
    }

    /// A two-way edge along X: lane 0 +X (its left is -Z), lane 1 -X 3.25 m to its left; a one-way pair
    /// 3.25 m apart in the same direction.
    #[test]
    fn left_gap_rows() {
        let graph = TrafficGraph::new(
            vec![
                (at(0.0, 0.0), at(50.0, 0.0), 12.0, 0),
                (at(50.0, -3.25), at(0.0, -3.25), 12.0, 1),
                (at(0.0, 20.0), at(50.0, 20.0), 12.0, 2),
                (at(0.0, 23.25), at(50.0, 23.25), 12.0, 3),
            ],
            &[(0, 1, 0), (1, 0, 1), (2, 3, 2), (3, 2, 3)],
            &cfg(),
            1.2,
        )
        .expect("GATE BROKEN: graph");
        let gap = |k: usize| graph.lanes()[k].left_gap;
        assert_eq!(gap(0), Some(3.25), "street edge, +X lane");
        assert_eq!(gap(1), Some(3.25), "street edge, -X lane");
        assert_eq!(gap(2), None, "one-way pair: same direction");
        assert_eq!(gap(3), None);
        assert!(
            graph.lanes().iter().all(|l| !l.curb_lane),
            "a synthetic graph has no curb lanes"
        );
    }
}
