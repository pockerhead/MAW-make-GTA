use crate::{
    Block, CityParams, Connector, Lane, LaneGraph, RoadClass, RoadGraph, Vec2, WalkGraph, geom,
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
