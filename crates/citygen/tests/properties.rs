mod common;

use citygen::{
    BuildingKind, CityLayout, CityParams, DistrictKind, RoadClass, Vec2, contains_convex,
    convex_overlap, dist_point_segment, generate,
};
use common::{layouts, shipped_params};

fn reached(count: usize, start: usize, next: &[Vec<usize>]) -> Vec<bool> {
    let mut seen = vec![false; count];
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(at) = stack.pop() {
        for &n in &next[at] {
            if !seen[n] {
                seen[n] = true;
                stack.push(n);
            }
        }
    }
    seen
}

fn undirected(count: usize, edges: impl Iterator<Item = (u32, u32)>) -> Vec<Vec<usize>> {
    let mut next = vec![Vec::new(); count];
    for (a, b) in edges {
        next[a as usize].push(b as usize);
        next[b as usize].push(a as usize);
    }
    next
}

fn aabb(poly: &[Vec2]) -> (Vec2, Vec2) {
    poly.iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(lo, hi), &p| (lo.min(p), hi.max(p)),
    )
}

fn district_of_building(layout: &CityLayout, building: usize) -> u32 {
    let lot = layout.buildings[building].lot as usize;
    layout.blocks[layout.lots[lot].block as usize].district
}

#[test]
fn road_graph_connected() {
    for (seed, layout) in layouts() {
        let roads = &layout.roads;
        assert_eq!(
            roads.nodes[roads.center as usize],
            Vec2::ZERO,
            "seed {seed}: centre node"
        );
        let next = undirected(roads.nodes.len(), roads.edges.iter().map(|e| (e.a, e.b)));
        let seen = reached(roads.nodes.len(), roads.center as usize, &next);
        let missing = seen.iter().filter(|s| !**s).count();
        assert_eq!(missing, 0, "seed {seed}: {missing} road nodes unreachable");
    }
}

#[test]
fn lane_graph_strongly_connected() {
    for (seed, layout) in layouts() {
        let lanes = &layout.lanes;
        let n = lanes.lanes.len();
        assert!(n > 0, "seed {seed}: no lanes");
        let mut forward = vec![Vec::new(); n];
        let mut backward = vec![Vec::new(); n];
        for c in &lanes.connectors {
            forward[c.from as usize].push(c.to as usize);
            backward[c.to as usize].push(c.from as usize);
        }
        for (name, next) in [("forward", &forward), ("backward", &backward)] {
            let missing = reached(n, 0, next).iter().filter(|s| !**s).count();
            assert_eq!(
                missing, 0,
                "seed {seed}: {missing} lanes unreachable {name} from lane 0"
            );
        }
    }
}

#[test]
fn sidewalk_graph_connected() {
    for (seed, layout) in layouts() {
        let walk = &layout.sidewalks;
        assert!(!walk.nodes.is_empty(), "seed {seed}: no sidewalk nodes");
        let next = undirected(walk.nodes.len(), walk.edges.iter().copied());
        let missing = reached(walk.nodes.len(), 0, &next)
            .iter()
            .filter(|s| !**s)
            .count();
        assert_eq!(
            missing, 0,
            "seed {seed}: {missing} sidewalk nodes unreachable"
        );
    }
}

#[test]
fn lots_do_not_overlap() {
    for (seed, layout) in layouts() {
        let boxes = layout
            .lots
            .iter()
            .map(|l| aabb(&l.polygon))
            .collect::<Vec<_>>();
        for (i, lot) in layout.lots.iter().enumerate() {
            let inner = &layout.blocks[lot.block as usize].inner;
            for &p in &lot.polygon {
                assert!(
                    contains_convex(inner, p, 1e-3),
                    "seed {seed}: lot {i} leaves its block at {p}"
                );
            }
            for j in i + 1..layout.lots.len() {
                let (a, b) = (boxes[i], boxes[j]);
                if a.1.x <= b.0.x || b.1.x <= a.0.x || a.1.y <= b.0.y || b.1.y <= a.0.y {
                    continue;
                }
                let depth = convex_overlap(&lot.polygon, &layout.lots[j].polygon);
                assert!(
                    depth <= 1e-3,
                    "seed {seed}: lots {i} and {j} overlap by {depth} m"
                );
            }
        }
    }
}

fn street_sides(layout: &CityLayout, block: usize) -> Vec<(Vec2, Vec2, RoadClass)> {
    let b = &layout.blocks[block];
    let n = b.inner.len();
    (0..n)
        .map(|k| {
            (
                b.inner[k],
                b.inner[(k + 1) % n],
                layout.roads.edges[b.sides[k] as usize].class,
            )
        })
        .collect()
}

#[test]
fn lots_face_a_street() {
    let params = shipped_params();
    for (seed, layout) in layouts() {
        for (id, block) in layout.blocks.iter().enumerate() {
            if block.is_park || block.inner.is_empty() {
                continue;
            }
            assert_eq!(
                block.inner.len(),
                block.sides.len(),
                "seed {seed}: block {id} inner/sides"
            );
            for (k, &side) in block.sides.iter().enumerate() {
                assert!(
                    (side as usize) < layout.roads.edges.len(),
                    "seed {seed}: block {id} side {k} dead edge"
                );
                let edge = layout.roads.edges[side as usize];
                let (p, q) = (
                    layout.roads.nodes[edge.a as usize],
                    layout.roads.nodes[edge.b as usize],
                );
                let dir = (q - p).normalize();
                let (s0, s1) = (block.inner[k], block.inner[(k + 1) % block.inner.len()]);
                let parallel = (s1 - s0).normalize().perp_dot(dir).abs();
                assert!(
                    parallel <= 1e-3,
                    "seed {seed}: block {id} side {k} not parallel ({parallel})"
                );
                let expected = params.half_carriageway(edge.class) + params.sidewalk(edge.class);
                for s in [s0, s1] {
                    let distance = (s - p).perp_dot(dir).abs();
                    assert!(
                        (distance - expected).abs() <= 1e-2,
                        "seed {seed}: block {id} side {k} at {distance} m from its road, expected {expected}"
                    );
                }
            }
        }
        for (i, lot) in layout.lots.iter().enumerate() {
            let block = &layout.blocks[lot.block as usize];
            let min_frontage = params
                .district(layout.districts[block.district as usize].kind)
                .min_frontage;
            let streets = street_sides(layout, lot.block as usize);
            let n = lot.polygon.len();
            let faces = (0..n).any(|k| {
                let (a, b) = (lot.polygon[k], lot.polygon[(k + 1) % n]);
                (b - a).length() >= min_frontage - 1e-3
                    && streets.iter().any(|&(s0, s1, class)| {
                        class != RoadClass::Alley
                            && dist_point_segment(a, s0, s1) <= 1e-3
                            && dist_point_segment(b, s0, s1) <= 1e-3
                    })
            });
            assert!(
                faces,
                "seed {seed}: lot {i} has no frontage of {min_frontage} m on a street"
            );
        }
    }
}

#[test]
fn buildings_inside_lots() {
    for (seed, layout) in layouts() {
        assert!(!layout.buildings.is_empty(), "seed {seed}: no buildings");
        for (i, b) in layout.buildings.iter().enumerate() {
            assert!(
                b.height > 0.0,
                "seed {seed}: building {i} height {}",
                b.height
            );
            let (u, v) = (b.axis, b.axis.perp());
            let lot = &layout.lots[b.lot as usize].polygon;
            for (su, sv) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                let corner = b.center + u * (su * b.half_extents.x) + v * (sv * b.half_extents.y);
                assert!(
                    contains_convex(lot, corner, 1e-3),
                    "seed {seed}: building {i} corner {corner} outside lot"
                );
            }
        }
    }
}

fn check_pois(seed: u64, params: &CityParams, layout: &CityLayout) {
    let count = |kind: BuildingKind| layout.buildings.iter().filter(|b| b.kind == kind).count();
    assert_eq!(count(BuildingKind::Hospital), 1, "seed {seed}: hospitals");
    assert_eq!(
        count(BuildingKind::PoliceStation),
        params.pois.police_stations as usize,
        "seed {seed}: police stations"
    );
    let [g0, g1] = layout.gang_districts;
    assert_ne!(g0, g1, "seed {seed}: gang districts must differ");
    for (index, district) in [(0u8, g0), (1u8, g1)] {
        let hqs = (0..layout.buildings.len())
            .filter(|&b| layout.buildings[b].kind == BuildingKind::GangHq(index))
            .collect::<Vec<_>>();
        assert_eq!(hqs.len(), 1, "seed {seed}: gang {index} headquarters");
        assert_eq!(
            district_of_building(layout, hqs[0]),
            district,
            "seed {seed}: gang {index} HQ district"
        );
        let kind = layout.districts[district as usize].kind;
        assert!(
            matches!(kind, DistrictKind::Residential | DistrictKind::Industrial),
            "seed {seed}: gang {index} district is {kind:?}"
        );
    }
}

#[test]
fn pois_exist() {
    let params = shipped_params();
    for (seed, layout) in layouts() {
        check_pois(*seed, &params, layout);
    }
}

#[test]
fn player_spawn_on_sidewalk() {
    let params = shipped_params();
    for (seed, layout) in layouts() {
        let spawn = layout.player_spawn;
        let on_sidewalk = layout.roads.edges.iter().any(|e| {
            let half = params.half_carriageway(e.class);
            let (p, q) = (
                layout.roads.nodes[e.a as usize],
                layout.roads.nodes[e.b as usize],
            );
            let d = dist_point_segment(spawn, p, q);
            e.class != RoadClass::Alley && d >= half && d <= half + params.sidewalk(e.class)
        });
        assert!(
            on_sidewalk,
            "seed {seed}: spawn {spawn} is not on a sidewalk"
        );
        for (i, lot) in layout.lots.iter().enumerate() {
            assert!(
                !contains_convex(&lot.polygon, spawn, 0.0),
                "seed {seed}: spawn inside lot {i}"
            );
        }
    }
}

#[test]
fn gang_guarantee_recolors() {
    let mut params = shipped_params();
    params.districts.weights.commercial = 1.0;
    params.districts.weights.residential = 0.0;
    params.districts.weights.industrial = 0.0;
    let layout = generate(1, &params).unwrap_or_else(|e| panic!("commercial-only weights: {e}"));
    check_pois(1, &params, &layout);
}
