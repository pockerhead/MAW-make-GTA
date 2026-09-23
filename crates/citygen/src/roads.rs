use crate::{Block, CityParams, RoadClass, RoadEdge, RoadGraph, Vec2, geom, grid::GridLines, rng};
use std::collections::HashMap;

fn pair(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

fn boundary(poly: &[u32], skip: (u32, u32)) -> Vec<(u32, u32)> {
    (0..poly.len())
        .filter_map(|i| {
            let edge = (poly[i], poly[(i + 1) % poly.len()]);
            (pair(edge.0, edge.1) != skip).then_some(edge)
        })
        .collect()
}

fn merged(a: &[u32], b: &[u32], skip: (u32, u32)) -> Vec<u32> {
    let mut sides = boundary(a, skip);
    sides.extend(boundary(b, skip));
    let boundary = sides;
    let start = boundary[0].0;
    let mut poly = vec![start];
    let mut at = start;
    while let Some((_, next)) = boundary.iter().find(|(from, _)| *from == at) {
        if *next == start {
            break;
        }
        poly.push(*next);
        at = *next;
        if poly.len() > boundary.len() {
            break;
        }
    }
    poly
}

pub(crate) fn build(seed: u64, params: &CityParams, grid: &GridLines) -> (RoadGraph, Vec<Block>) {
    let nx = grid.x.len();
    let nz = grid.z.len();
    let id = |i: usize, j: usize| (i * nz + j) as u32;
    let mut nodes = Vec::with_capacity(nx * nz);
    for i in 0..nx {
        for j in 0..nz {
            let mut p = Vec2::new(grid.x[i], grid.z[j]);
            if (i, j) != (grid.cx, grid.cz) {
                let mut r = rng::stream(seed, rng::JITTER, i as u64 * 1024 + j as u64);
                if i != 0 && i + 1 != nx {
                    p.x +=
                        rng::range_f32(&mut r, -params.grid.node_jitter, params.grid.node_jitter);
                }
                if j != 0 && j + 1 != nz {
                    p.y +=
                        rng::range_f32(&mut r, -params.grid.node_jitter, params.grid.node_jitter);
                }
            }
            nodes.push(p);
        }
    }
    let avenue =
        |line: usize, center: usize| line.abs_diff(center).is_multiple_of(params.grid.avenue_every as usize);
    let mut edges = Vec::new();
    let mut lookup = HashMap::new();
    for i in 0..nx {
        for j in 0..nz {
            if i + 1 < nx {
                let a = id(i, j);
                let b = id(i + 1, j);
                lookup.insert(pair(a, b), edges.len() as u32);
                edges.push(RoadEdge {
                    a,
                    b,
                    class: if avenue(j, grid.cz) {
                        RoadClass::Avenue
                    } else {
                        RoadClass::Street
                    },
                });
            }
            if j + 1 < nz {
                let a = id(i, j);
                let b = id(i, j + 1);
                lookup.insert(pair(a, b), edges.len() as u32);
                edges.push(RoadEdge {
                    a,
                    b,
                    class: if avenue(i, grid.cx) {
                        RoadClass::Avenue
                    } else {
                        RoadClass::Street
                    },
                });
            }
        }
    }
    let mut cells = Vec::new();
    for i in 0..nx - 1 {
        for j in 0..nz - 1 {
            cells.push(vec![id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)]);
        }
    }
    let mut adjacent = vec![Vec::new(); edges.len()];
    for (cell, poly) in cells.iter().enumerate() {
        for k in 0..poly.len() {
            adjacent[lookup[&pair(poly[k], poly[(k + 1) % poly.len()])] as usize].push(cell);
        }
    }
    let mut used = vec![false; cells.len()];
    let mut removed = vec![false; edges.len()];
    let mut replacement = vec![None; cells.len()];
    for (edge_id, touching) in adjacent.iter().enumerate() {
        if touching.len() != 2 {
            continue;
        }
        let mut r = rng::stream(seed, rng::EDGES, edge_id as u64);
        let value = rng::unit_f32(&mut r);
        let (a, b) = (touching[0], touching[1]);
        if used[a] || used[b] {
            continue;
        }
        if value < params.superblock_chance {
            let e = edges[edge_id];
            let joined = merged(&cells[a], &cells[b], pair(e.a, e.b));
            let poly = joined
                .iter()
                .map(|&n| nodes[n as usize])
                .collect::<Vec<_>>();
            if geom::is_convex(&poly, 1e-4) {
                used[a] = true;
                used[b] = true;
                removed[edge_id] = true;
                replacement[a] = Some(joined);
                replacement[b] = Some(Vec::new());
            }
        } else if value < params.superblock_chance + params.alley_chance {
            edges[edge_id].class = RoadClass::Alley;
            used[a] = true;
            used[b] = true;
        }
    }
    let mut remap = vec![u32::MAX; edges.len()];
    let mut next_id = 0;
    let live = edges
        .into_iter()
        .enumerate()
        .filter_map(|(i, e)| {
            if removed[i] {
                None
            } else {
                remap[i] = next_id;
                next_id += 1;
                Some(e)
            }
        })
        .collect::<Vec<_>>();
    let mut blocks = Vec::new();
    for (cell, original) in cells.into_iter().enumerate() {
        let poly = match replacement[cell].take() {
            Some(p) if p.is_empty() => continue,
            Some(p) => p,
            None => original,
        };
        let sides = (0..poly.len())
            .map(|k| remap[lookup[&pair(poly[k], poly[(k + 1) % poly.len()])] as usize])
            .collect::<Vec<_>>();
        let polygon = poly.iter().map(|&n| nodes[n as usize]).collect::<Vec<_>>();
        let curb_offsets = sides
            .iter()
            .map(|&s| params.half_carriageway(live[s as usize].class))
            .collect::<Vec<_>>();
        let inner_offsets = sides
            .iter()
            .map(|&s| {
                let class = live[s as usize].class;
                params.half_carriageway(class) + params.sidewalk(class)
            })
            .collect::<Vec<_>>();
        let curb = geom::inset(&polygon, &curb_offsets).unwrap_or_default();
        let inner = if curb.is_empty() {
            Vec::new()
        } else {
            geom::inset(&polygon, &inner_offsets).unwrap_or_default()
        };
        let is_park = inner.is_empty();
        blocks.push(Block {
            district: 0,
            nodes: poly,
            sides,
            curb,
            inner,
            is_park,
        });
    }
    (
        RoadGraph {
            nodes,
            edges: live,
            center: id(grid.cx, grid.cz),
        },
        blocks,
    )
}
