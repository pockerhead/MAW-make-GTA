//! `TrafficGraph`: the inner (slot 0) lanes of the city with Bézier connectors across the
//! intersections and a conflict table per intersection.

use super::TrafficConfig;
use crate::vehicle::VehicleConfig;
use crate::world::{City, CityLayout, CityParams, CityParamsRes, RoadClass};
use bevy::prelude::*;

/// A place on the traffic graph: a lane or an intersection connector.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Segment {
    Lane(u32),
    Connector(u32),
}

#[derive(Clone, Debug)]
pub struct TrafficLane {
    pub from: Vec3,
    pub to: Vec3,
    /// Unit direction.
    pub dir: Vec3,
    pub length: f32,
    /// Where a car without a grant stops its nose, m along the lane (the lane end, or before the
    /// pedestrian crossing there).
    pub stop: f32,
    /// Desired speed, m/s.
    pub v0: f32,
    /// Intersection at the lane end.
    pub end_node: u32,
    /// Connectors leaving the lane end.
    pub out: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct TrafficConnector {
    pub from_lane: u32,
    pub to_lane: u32,
    pub node: u32,
    /// Quadratic Bézier control points.
    bezier: [Vec3; 3],
    pub points: Vec<Vec3>,
    /// Bézier parameter of each point.
    params: Vec<f32>,
    /// Arc length at each point, m.
    pub cumulative: Vec<f32>,
    pub length: f32,
    /// Connectors of the same intersection a car on this one must not share the box with.
    pub conflicts: Vec<u32>,
}

/// Lane graph of the traffic (a `Resource` built once per city).
#[derive(Resource, Clone, Debug)]
pub struct TrafficGraph {
    lanes: Vec<TrafficLane>,
    connectors: Vec<TrafficConnector>,
    spawn_points: Vec<(u32, f32)>,
    /// Flat bounds (min, max) of the connector curves of each intersection.
    boxes: Vec<(Vec2, Vec2)>,
}

fn flat(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}

/// Minimum distance between two flat segments.
fn segment_distance(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> f32 {
    let point = |p: Vec2, s0: Vec2, s1: Vec2| {
        let d = s1 - s0;
        let t = ((p - s0).dot(d) / d.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
        p.distance(s0 + d * t)
    };
    let (da, db) = (a1 - a0, b1 - b0);
    let denom = da.perp_dot(db);
    if denom.abs() > f32::EPSILON {
        let t = (b0 - a0).perp_dot(db) / denom;
        let u = (b0 - a0).perp_dot(da) / denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            return 0.0;
        }
    }
    point(a0, b0, b1)
        .min(point(a1, b0, b1))
        .min(point(b0, a0, a1))
        .min(point(b1, a0, a1))
}

fn polyline_distance(a: &[Vec3], b: &[Vec3]) -> f32 {
    let mut best = f32::INFINITY;
    for sa in a.windows(2) {
        for sb in b.windows(2) {
            best = best.min(segment_distance(
                flat(sa[0]),
                flat(sa[1]),
                flat(sb[0]),
                flat(sb[1]),
            ));
        }
    }
    best
}

/// Control point of the turn from `p0` along `d_in` into `p2` along `d_out`: where the two lane lines
/// meet, or the midpoint when they are parallel or meet behind either end.
pub fn control_point(p0: Vec3, d_in: Vec3, p2: Vec3, d_out: Vec3) -> Vec3 {
    let (a, b, r) = (flat(d_in), flat(d_out), flat(p2 - p0));
    let cross = a.perp_dot(b);
    let mid = (p0 + p2) / 2.0;
    if cross.abs() < 1e-3 {
        return mid;
    }
    let t = r.perp_dot(b) / cross;
    let u = -r.perp_dot(a) / cross;
    if t <= 0.0 || u <= 0.0 {
        return mid;
    }
    let c = flat(p0) + a * t;
    Vec3::new(c.x, p0.y, c.y)
}

fn bezier(b: &[Vec3; 3], t: f32) -> Vec3 {
    let s = 1.0 - t;
    b[0] * s * s + b[1] * 2.0 * s * t + b[2] * t * t
}

fn bezier_tangent(b: &[Vec3; 3], t: f32) -> Vec3 {
    ((b[1] - b[0]) * 2.0 * (1.0 - t) + (b[2] - b[1]) * 2.0 * t).normalize_or_zero()
}

impl TrafficConnector {
    fn new(from_lane: u32, to_lane: u32, node: u32, lanes: &[TrafficLane], samples: u32) -> Self {
        let (a, b) = (&lanes[from_lane as usize], &lanes[to_lane as usize]);
        let control = control_point(a.to, a.dir, b.from, b.dir);
        let bezier = [a.to, control, b.from];
        let params: Vec<f32> = (0..samples)
            .map(|k| k as f32 / (samples - 1) as f32)
            .collect();
        let points: Vec<Vec3> = params.iter().map(|&t| self::bezier(&bezier, t)).collect();
        let mut cumulative = vec![0.0];
        for w in points.windows(2) {
            let last = *cumulative.last().unwrap_or(&0.0);
            cumulative.push(last + w[0].distance(w[1]));
        }
        let length = *cumulative.last().unwrap_or(&0.0);
        Self {
            from_lane,
            to_lane,
            node,
            bezier,
            points,
            params,
            cumulative,
            length,
            conflicts: Vec::new(),
        }
    }

    /// Arc length of the polyline point nearest to `p` (flat).
    fn project(&self, p: Vec3) -> f32 {
        let q = flat(p);
        let mut best = (f32::INFINITY, 0.0);
        for (k, w) in self.points.windows(2).enumerate() {
            let (a, b) = (flat(w[0]), flat(w[1]));
            let d = b - a;
            let t = ((q - a).dot(d) / d.length_squared().max(f32::EPSILON)).clamp(0.0, 1.0);
            let distance = q.distance(a + d * t);
            if distance < best.0 {
                best = (distance, self.cumulative[k] + t * d.length());
            }
        }
        best.1
    }

    fn pose(&self, s: f32) -> (Vec3, Vec3) {
        let s = s.clamp(0.0, self.length);
        let k = self
            .cumulative
            .partition_point(|&c| c <= s)
            .clamp(1, self.points.len() - 1);
        let (c0, c1) = (self.cumulative[k - 1], self.cumulative[k]);
        let f = if c1 > c0 { (s - c0) / (c1 - c0) } else { 0.0 };
        let point = self.points[k - 1].lerp(self.points[k], f);
        let t = self.params[k - 1] + (self.params[k] - self.params[k - 1]) * f;
        (point, bezier_tangent(&self.bezier, t))
    }
}

impl TrafficGraph {
    /// `lanes`: (from, to, v0, end node); `connectors`: (from lane, to lane, node). `half_width`: car
    /// half width, m.
    pub fn new(
        lanes: Vec<(Vec3, Vec3, f32, u32)>,
        connectors: &[(u32, u32, u32)],
        cfg: &TrafficConfig,
        half_width: f32,
    ) -> Result<Self, String> {
        let mut built = Vec::with_capacity(lanes.len());
        for (k, (from, to, v0, end_node)) in lanes.into_iter().enumerate() {
            let length = from.distance(to);
            if !(length.is_finite() && length > 0.0 && from.is_finite() && to.is_finite()) {
                return Err(format!("lane {k}: degenerate ({from} -> {to})"));
            }
            built.push(TrafficLane {
                from,
                to,
                dir: (to - from) / length,
                length,
                stop: length,
                v0,
                end_node,
                out: Vec::new(),
            });
        }
        let mut conns = Vec::with_capacity(connectors.len());
        for (id, &(a, b, node)) in connectors.iter().enumerate() {
            if a as usize >= built.len() || b as usize >= built.len() {
                return Err(format!("connector {id}: lane out of range"));
            }
            conns.push(TrafficConnector::new(
                a,
                b,
                node,
                &built,
                cfg.connector_samples,
            ));
            built[a as usize].out.push(id as u32);
        }
        if let Some(k) = built.iter().position(|l| l.out.is_empty()) {
            return Err(format!(
                "lane {k} ({} -> {}) has no outgoing connector (a dead end parks cars forever)",
                built[k].from, built[k].to
            ));
        }
        let clear = 2.0 * half_width + cfg.conflict_margin;
        for i in 0..conns.len() {
            let conflicts = (0..conns.len())
                .filter(|&j| j != i && conns[j].node == conns[i].node)
                .filter(|&j| {
                    let (a, b) = (&conns[i], &conns[j]);
                    a.from_lane == b.from_lane
                        || a.to_lane == b.to_lane
                        || polyline_distance(&a.points, &b.points) < clear
                })
                .map(|j| j as u32)
                .collect();
            conns[i].conflicts = conflicts;
        }
        let spacing = cfg.bubble.spawn_spacing;
        let spawn_points = built
            .iter()
            .enumerate()
            .flat_map(|(k, lane)| {
                let count = ((lane.length - spacing / 2.0) / spacing).ceil().max(0.0) as u32;
                (0..count).map(move |i| (k as u32, spacing / 2.0 + i as f32 * spacing))
            })
            .filter(|&(k, s)| s < built[k as usize].length)
            .collect();
        let mut bounds: std::collections::BTreeMap<u32, (Vec2, Vec2)> = Default::default();
        for conn in &conns {
            for &p in &conn.points {
                let b = bounds.entry(conn.node).or_insert((flat(p), flat(p)));
                *b = (b.0.min(flat(p)), b.1.max(flat(p)));
            }
        }
        Ok(Self {
            lanes: built,
            connectors: conns,
            spawn_points,
            boxes: bounds.into_values().collect(),
        })
    }

    /// The inner lane of every road edge in both directions (slot 0), at road-top height 0.
    pub fn from_layout(
        layout: &CityLayout,
        params: &CityParams,
        cfg: &TrafficConfig,
        half_width: f32,
    ) -> Result<Self, String> {
        let roads = &layout.roads;
        let width = params.roads.lane_width;
        let mut index = vec![None; layout.lanes.lanes.len()];
        let mut lanes = Vec::new();
        for (k, lane) in layout.lanes.lanes.iter().enumerate() {
            let edge = roads.edges[lane.edge as usize];
            let (a, b) = (roads.nodes[edge.a as usize], roads.nodes[edge.b as usize]);
            let d = (b - a).normalize();
            if lane_slot(d.perp_dot(lane.from - a).abs(), width) != 0 {
                continue;
            }
            let v0 = match edge.class {
                RoadClass::Avenue => cfg.desired_speed.avenue,
                RoadClass::Street => cfg.desired_speed.street,
                RoadClass::Alley => continue,
            };
            let end_node = if lane.to.distance(b) < lane.to.distance(a) {
                edge.b
            } else {
                edge.a
            };
            index[k] = Some(lanes.len() as u32);
            lanes.push((
                Vec3::new(lane.from.x, 0.0, lane.from.y),
                Vec3::new(lane.to.x, 0.0, lane.to.y),
                v0,
                end_node,
            ));
        }
        let connectors: Vec<(u32, u32, u32)> = layout
            .lanes
            .connectors
            .iter()
            .filter_map(|c| {
                Some((
                    index[c.from as usize]?,
                    index[c.to as usize]?,
                    c.intersection,
                ))
            })
            .collect();
        let mut graph = Self::new(lanes, &connectors, cfg, half_width)?;
        let walks = &layout.sidewalks;
        for lane in &mut graph.lanes {
            let (a, r) = (flat(lane.from), flat(lane.to - lane.from));
            for &(i, j) in &walks.edges {
                let p = walks.nodes[i as usize];
                let Some(t) = crossing(a, r, p, walks.nodes[j as usize] - p) else {
                    continue;
                };
                // A waiting car must not stand on the crosswalk: walkers press into its nose and it
                // waits for them.
                if t > 0.5 {
                    lane.stop = lane
                        .stop
                        .min((t * lane.length - cfg.crossing_clearance).max(0.0));
                }
            }
        }
        Ok(graph)
    }

    pub fn lanes(&self) -> &[TrafficLane] {
        &self.lanes
    }

    pub fn connectors(&self) -> &[TrafficConnector] {
        &self.connectors
    }

    pub fn lane(&self, id: u32) -> &TrafficLane {
        &self.lanes[id as usize]
    }

    pub fn connector(&self, id: u32) -> &TrafficConnector {
        &self.connectors[id as usize]
    }

    /// (lane, s) spawn candidates every `bubble.spawn_spacing` m.
    pub fn spawn_points(&self) -> &[(u32, f32)] {
        &self.spawn_points
    }

    /// Point (y = 0) and unit tangent at `s` m along `seg` (clamped to the segment).
    pub fn pose(&self, seg: Segment, s: f32) -> (Vec3, Vec3) {
        match seg {
            Segment::Lane(k) => {
                let lane = &self.lanes[k as usize];
                (lane.from + lane.dir * s.clamp(0.0, lane.length), lane.dir)
            }
            Segment::Connector(k) => self.connectors[k as usize].pose(s),
        }
    }

    /// Position along `seg` nearest to `p` (flat), clamped to the segment.
    pub fn project(&self, seg: Segment, p: Vec3) -> f32 {
        match seg {
            Segment::Lane(k) => {
                let lane = &self.lanes[k as usize];
                (p - lane.from).with_y(0.0).dot(lane.dir)
            }
            Segment::Connector(k) => self.connectors[k as usize].project(p),
        }
    }

    pub fn length(&self, seg: Segment) -> f32 {
        match seg {
            Segment::Lane(k) => self.lanes[k as usize].length,
            Segment::Connector(k) => self.connectors[k as usize].length,
        }
    }

    /// `p` lies within `margin` (flat) of an intersection's connector curves' bounds.
    pub fn in_junction(&self, p: Vec3, margin: f32) -> bool {
        let q = flat(p);
        self.boxes.iter().any(|&(lo, hi)| {
            q.cmpge(lo - Vec2::splat(margin)).all() && q.cmple(hi + Vec2::splat(margin)).all()
        })
    }

    /// Nearest lane point to `p` (flat): (lane, s, distance).
    pub fn nearest(&self, p: Vec3) -> Option<(Segment, f32, f32)> {
        self.lanes
            .iter()
            .enumerate()
            .map(|(k, lane)| {
                let s = (p - lane.from).dot(lane.dir).clamp(0.0, lane.length);
                let at = lane.from + lane.dir * s;
                (Segment::Lane(k as u32), s, flat(p).distance(flat(at)))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
    }
}

/// Parameter along `r` where the segment `a + r·t` crosses `p + d·u` (both within their ends).
fn crossing(a: Vec2, r: Vec2, p: Vec2, d: Vec2) -> Option<f32> {
    let den = r.perp_dot(d);
    if den.abs() < f32::EPSILON {
        return None;
    }
    let t = (p - a).perp_dot(d) / den;
    let u = (p - a).perp_dot(r) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// Lane slot from the lateral offset of a lane to its road's centre line: 0 = inner.
pub fn lane_slot(offset: f32, lane_width: f32) -> i32 {
    (offset / lane_width - 0.5).round() as i32
}

pub(super) fn build_traffic_graph(
    mut commands: Commands,
    city: Res<City>,
    params: Res<CityParamsRes>,
    cfg: Res<TrafficConfig>,
    vehicle: Res<VehicleConfig>,
    mut exit: MessageWriter<AppExit>,
) {
    match TrafficGraph::from_layout(&city.0, &params.0, &cfg, vehicle.half_extents().x) {
        Ok(graph) => commands.insert_resource(graph),
        Err(e) => {
            error!("traffic graph invalid: {e}");
            exit.write(AppExit::error());
        }
    }
}

pub(super) fn drop_traffic_graph(mut commands: Commands) {
    commands.remove_resource::<TrafficGraph>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigRoot, load_config};
    use crate::traffic::TRAFFIC_CONFIG;

    fn shipped() -> TrafficConfig {
        let root = ConfigRoot(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").into());
        load_config(&root, TRAFFIC_CONFIG).expect("GATE BROKEN: traffic.ron")
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        a.abs_diff_eq(b, 1e-4)
    }

    #[test]
    fn slot_rows() {
        let w = 3.25;
        assert_eq!(lane_slot(1.625, w), 0, "inner avenue lane");
        assert_eq!(lane_slot(4.875, w), 1, "curb avenue lane");
        assert_eq!(lane_slot(1.625, w), 0, "street lane");
        // Both directions: the offset is measured unsigned from the edge line.
        let (a, b) = (Vec2::ZERO, Vec2::new(100.0, 0.0));
        let d = (b - a).normalize();
        let forward = Vec2::new(10.0, -1.625);
        let backward = Vec2::new(90.0, 1.625);
        let curb_back = Vec2::new(90.0, 4.875);
        assert_eq!(lane_slot(d.perp_dot(forward - a).abs(), w), 0);
        assert_eq!(lane_slot(d.perp_dot(backward - a).abs(), w), 0);
        assert_eq!(lane_slot(d.perp_dot(curb_back - a).abs(), w), 1);
    }

    #[test]
    fn right_angle_connector() {
        let cfg = shipped();
        let p0 = Vec3::ZERO;
        let p2 = Vec3::new(5.0, 0.0, -5.0);
        let c = control_point(p0, Vec3::NEG_Z, p2, Vec3::X);
        assert!(close(c, Vec3::new(0.0, 0.0, -5.0)), "{c}");
        let b = [p0, c, p2];
        assert!(close(bezier(&b, 0.5), Vec3::new(1.25, 0.0, -3.75)));
        let graph = TrafficGraph::new(
            vec![
                (Vec3::new(0.0, 0.0, 20.0), p0, 12.0, 0),
                (p2, Vec3::new(30.0, 0.0, -5.0), 12.0, 1),
                (
                    Vec3::new(30.0, 0.0, -5.0),
                    Vec3::new(0.0, 0.0, 20.0),
                    12.0,
                    2,
                ),
            ],
            &[(0, 1, 0), (1, 2, 1), (2, 0, 2)],
            &cfg,
            1.2,
        )
        .expect("loop graph");
        let seg = Segment::Connector(0);
        let (start, t0) = graph.pose(seg, 0.0);
        let (end, t1) = graph.pose(seg, graph.length(seg));
        assert!(close(start, p0) && close(end, p2));
        assert!(close(t0, Vec3::NEG_Z) && close(t1, Vec3::X), "{t0} {t1}");
    }

    /// A + intersection at the origin, arms of 20 m, inner lanes 1.625 m right of each centre line.
    fn plus() -> TrafficGraph {
        let cfg = shipped();
        let (o, arm, trim) = (1.625, 20.0, 3.25);
        // Directions N(−Z), E(+X), S(+Z), W(−X); lane k in = coming from direction k's arm.
        let dirs = [Vec3::NEG_Z, Vec3::X, Vec3::Z, Vec3::NEG_X];
        let right = |d: Vec3| Vec3::new(-d.z, 0.0, d.x);
        let mut lanes = Vec::new();
        // In lanes 0..4 (travel towards the centre), out lanes 4..8 (away from it).
        for &d in &dirs {
            let travel = -d;
            let from = d * arm + right(travel) * o;
            let to = d * trim + right(travel) * o;
            lanes.push((from, to, 12.0, 0));
        }
        for &d in &dirs {
            let from = d * trim + right(d) * o;
            let to = d * arm + right(d) * o;
            lanes.push((from, to, 12.0, 1));
        }
        let mut connectors = Vec::new();
        for i in 0..4u32 {
            for j in 0..4u32 {
                if i != j {
                    connectors.push((i, 4 + j, 0));
                }
            }
        }
        // Out lanes loop back into their own in lane through a far node (no conflicts there).
        for j in 0..4u32 {
            connectors.push((4 + j, j, 10 + j));
        }
        TrafficGraph::new(lanes, &connectors, &cfg, 1.2).expect("plus graph")
    }

    fn find(g: &TrafficGraph, from: u32, to: u32) -> u32 {
        g.connectors()
            .iter()
            .position(|c| c.from_lane == from && c.to_lane == to)
            .expect("connector") as u32
    }

    #[test]
    fn plus_conflicts() {
        let g = plus();
        assert_eq!(g.connectors().iter().filter(|c| c.node == 0).count(), 12);
        // Straight north arm -> south arm (in 0 -> out 6) and east -> west (in 1 -> out 7).
        let ns = find(&g, 0, 6);
        let ew = find(&g, 1, 7);
        assert!(g.connector(ns).conflicts.contains(&ew));
        assert!(!g.connector(ns).conflicts.contains(&ns));
        // Same source.
        let n_turn = find(&g, 0, 5);
        assert!(g.connector(ns).conflicts.contains(&n_turn));
        // Same destination.
        let w_to_s = find(&g, 3, 6);
        assert!(g.connector(ns).conflicts.contains(&w_to_s));
        // Opposite straights pass each other 3.25 m apart.
        let sn = find(&g, 2, 4);
        assert!(!g.connector(ns).conflicts.contains(&sn));
    }
}
