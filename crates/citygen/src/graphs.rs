use crate::{
    Block, CityLayout, CityParams, Connector, Lane, LaneGraph, RoadClass, RoadGraph, Vec2,
    WalkGraph, geom,
};

/// Builds the sidewalk graph, the directed lane graph and the player spawn point.
pub(crate) fn build(
    params: &CityParams,
    roads: &RoadGraph,
    blocks: &[Block],
) -> (WalkGraph, LaneGraph, Vec2) {
    (
        sidewalks(params, roads, blocks),
        lanes(params, roads),
        player_spawn(params, roads, blocks),
    )
}

fn road_polygon(roads: &RoadGraph, block: &Block) -> Vec<Vec2> {
    block
        .nodes
        .iter()
        .map(|&n| roads.nodes[n as usize])
        .collect()
}

// Sidewalk centre line; alley sides have no sidewalk, so the walk runs along the curb there.
fn walk_offset(params: &CityParams, class: RoadClass) -> f32 {
    params.half_carriageway(class) + params.sidewalk(class) / 2.0
}

fn sidewalks(params: &CityParams, roads: &RoadGraph, blocks: &[Block]) -> WalkGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut corners: Vec<Vec<u32>> = Vec::with_capacity(blocks.len());
    for block in blocks {
        let offsets = block
            .sides
            .iter()
            .map(|&s| walk_offset(params, roads.edges[s as usize].class))
            .collect::<Vec<_>>();
        let ring = geom::inset(&road_polygon(roads, block), &offsets).unwrap_or_default();
        let first = nodes.len() as u32;
        let ids = (0..ring.len() as u32)
            .map(|k| first + k)
            .collect::<Vec<_>>();
        nodes.extend(&ring);
        for k in 0..ids.len() {
            edges.push((ids[k], ids[(k + 1) % ids.len()]));
        }
        if block.is_park && !ring.is_empty() {
            let center = nodes.len() as u32;
            nodes.push(geom::centroid(&ring));
            edges.extend(ids.iter().map(|&id| (center, id)));
        }
        corners.push(ids);
    }
    let mut sides_of_edge = vec![Vec::new(); roads.edges.len()];
    for (b, block) in blocks.iter().enumerate() {
        for &s in &block.sides {
            sides_of_edge[s as usize].push(b);
        }
    }
    // Crossings: at both ends of a road edge, link the corners of the two blocks it separates.
    let corner_at = |b: usize, node: u32| {
        let k = blocks[b].nodes.iter().position(|&n| n == node)?;
        corners[b].get(k).copied()
    };
    for (e, pair) in sides_of_edge.iter().enumerate() {
        let &[first, second] = pair.as_slice() else {
            continue;
        };
        let edge = roads.edges[e];
        for node in [edge.a, edge.b] {
            if let (Some(p), Some(q)) = (corner_at(first, node), corner_at(second, node)) {
                edges.push((p, q));
            }
        }
    }
    WalkGraph { nodes, edges }
}

fn lanes(params: &CityParams, roads: &RoadGraph) -> LaneGraph {
    let mut trim = vec![0.0_f32; roads.nodes.len()];
    for edge in &roads.edges {
        let half = params.half_carriageway(edge.class);
        trim[edge.a as usize] = trim[edge.a as usize].max(half);
        trim[edge.b as usize] = trim[edge.b as usize].max(half);
    }
    let mut lanes = Vec::new();
    let mut ends = Vec::new();
    for (e, edge) in roads.edges.iter().enumerate() {
        let count = match edge.class {
            RoadClass::Avenue => params.roads.avenue.lanes_per_direction,
            RoadClass::Street => params.roads.street.lanes_per_direction,
            RoadClass::Alley => continue,
        };
        for (from, to) in [(edge.a, edge.b), (edge.b, edge.a)] {
            let (p, q) = (roads.nodes[from as usize], roads.nodes[to as usize]);
            let d = (q - p).normalize();
            // Right-hand traffic: with world (x, z) mapped to (x, y), d.perp() is the driver's right.
            let right = d.perp();
            for i in 0..count {
                let shift = right * ((i as f32 + 0.5) * params.roads.lane_width);
                lanes.push(Lane {
                    edge: e as u32,
                    from: p + d * trim[from as usize] + shift,
                    to: q - d * trim[to as usize] + shift,
                });
                ends.push((from, to));
            }
        }
    }
    let mut incoming = vec![Vec::new(); roads.nodes.len()];
    let mut outgoing = vec![Vec::new(); roads.nodes.len()];
    for (id, &(from, to)) in ends.iter().enumerate() {
        outgoing[from as usize].push(id as u32);
        incoming[to as usize].push(id as u32);
    }
    let mut connectors = Vec::new();
    for node in 0..roads.nodes.len() {
        for &a in &incoming[node] {
            for &b in &outgoing[node] {
                if lanes[a as usize].edge != lanes[b as usize].edge {
                    connectors.push(Connector {
                        from: a,
                        to: b,
                        intersection: node as u32,
                    });
                }
            }
        }
    }
    LaneGraph { lanes, connectors }
}

// The sidewalk midpoint of the north-south street side closest to the city centre.
fn player_spawn(params: &CityParams, roads: &RoadGraph, blocks: &[Block]) -> Vec2 {
    let mut best: Option<Vec2> = None;
    for block in blocks.iter().filter(|b| !b.inner.is_empty()) {
        let poly = road_polygon(roads, block);
        for (k, &side) in block.sides.iter().enumerate() {
            let class = roads.edges[side as usize].class;
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            let d = b - a;
            if class == RoadClass::Alley || d.y.abs() <= d.x.abs() {
                continue;
            }
            let point = (a + b) / 2.0 + d.normalize().perp() * walk_offset(params, class);
            if best.is_none_or(|p| point.length_squared() < p.length_squared()) {
                best = Some(point);
            }
        }
    }
    best.expect("a city with buildable blocks has north-south streets")
}

/// Point on the sidewalk centre line of the building's block facing `building`, and the unit sidewalk
/// direction there. The point is at least `margin` from both ends of its sidewalk side, so
/// `point ± dir * margin` stays on the same side. `None` for a bad index or no side long enough.
pub fn sidewalk_anchor(
    layout: &CityLayout,
    params: &CityParams,
    building: usize,
    margin: f32,
) -> Option<(Vec2, Vec2)> {
    let b = layout.buildings.get(building)?;
    let lot = layout.lots.get(b.lot as usize)?;
    let block = layout.blocks.get(lot.block as usize)?;
    let classes = block
        .sides
        .iter()
        .map(|&s| Some(layout.roads.edges.get(s as usize)?.class))
        .collect::<Option<Vec<_>>>()?;
    let offsets = classes
        .iter()
        .map(|&class| walk_offset(params, class))
        .collect::<Vec<_>>();
    let polygon = block
        .nodes
        .iter()
        .map(|&n| layout.roads.nodes.get(n as usize).copied())
        .collect::<Option<Vec<_>>>()?;
    // Side k of the ring lies on road side k moved inward to the sidewalk centre line.
    let ring = geom::inset(&polygon, &offsets)?;
    let n = ring.len();
    classes
        .iter()
        .enumerate()
        .filter(|&(_, &class)| class != RoadClass::Alley)
        .filter_map(|(k, _)| anchor_on_segment(ring[k], ring[(k + 1) % n], b.center, margin))
        .min_by(|p, q| {
            p.0.distance_squared(b.center)
                .total_cmp(&q.0.distance_squared(b.center))
        })
}

/// Closest point to `target` on segment `a → c`, kept `margin` away from both ends; `None` when the
/// segment is shorter than `2 * margin`.
fn anchor_on_segment(a: Vec2, c: Vec2, target: Vec2, margin: f32) -> Option<(Vec2, Vec2)> {
    let len = a.distance(c);
    // Written so that a NaN length or margin also yields `None` (and `clamp` never panics).
    let long_enough = len >= 2.0 * margin && len > 0.0;
    if !long_enough {
        return None;
    }
    let d = (c - a) / len;
    let t = (target - a).dot(d).clamp(margin, len - margin);
    Some((a + d * t, d))
}

#[cfg(test)]
mod tests {
    use super::anchor_on_segment;
    use crate::Vec2;

    #[test]
    fn anchor_on_segment_examples() {
        let v = Vec2::new;
        let cases = [
            (
                v(-50.0, 0.0),
                v(50.0, 0.0),
                v(35.0, 10.0),
                Some((v(35.0, 0.0), v(1.0, 0.0))),
            ),
            (
                v(50.0, 0.0),
                v(-50.0, 0.0),
                v(35.0, 10.0),
                Some((v(35.0, 0.0), v(-1.0, 0.0))),
            ),
            (
                v(0.0, -50.0),
                v(0.0, 50.0),
                v(-10.0, 49.0),
                Some((v(0.0, 44.0), v(0.0, 1.0))),
            ),
            (
                v(-50.0, 0.0),
                v(50.0, 0.0),
                v(-60.0, 5.0),
                Some((v(-44.0, 0.0), v(1.0, 0.0))),
            ),
            (v(0.0, 0.0), v(10.0, 0.0), v(5.0, 1.0), None),
        ];
        for (i, (a, c, target, expected)) in cases.into_iter().enumerate() {
            let got = anchor_on_segment(a, c, target, 6.0);
            match (got, expected) {
                (None, None) => {}
                (Some((p, d)), Some((ep, ed))) => {
                    assert!(
                        p.distance(ep) < 1e-4,
                        "case {}: point {p}, expected {ep}",
                        i + 1
                    );
                    assert!(
                        d.distance(ed) < 1e-4,
                        "case {}: dir {d}, expected {ed}",
                        i + 1
                    );
                }
                _ => panic!("case {}: got {got:?}, expected {expected:?}", i + 1),
            }
        }
    }
}
