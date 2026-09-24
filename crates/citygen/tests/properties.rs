mod common;

use citygen::{
    BuildingKind, CityLayout, CityParams, DistrictKind, RoadClass, Vec2, centroid, contains_convex,
    convex_overlap, dist_point_segment, generate, sidewalk_anchor,
};
use common::{layouts, shipped_params};
use serde::Deserialize;

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

#[derive(Deserialize)]
struct HealthFile {
    pickups: PickupsFile,
}

#[derive(Deserialize)]
struct PickupsFile {
    spacing: f32,
}

/// `pickups.spacing` of the shipped `character/health.ron`: the sidewalk margin of the hospital anchor.
fn pickup_spacing() -> f32 {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/character/health.ron"
    );
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("GATE BROKEN: cannot read {path}: {e}"));
    let file: HealthFile =
        ron::from_str(&text).unwrap_or_else(|e| panic!("GATE BROKEN: cannot parse {path}: {e}"));
    file.pickups.spacing
}

/// Building `idx` has a sidewalk anchor: a unit direction and three points on a sidewalk, off every
/// carriageway and lot, within `margin` along the sidewalk from the building centre.
fn assert_anchor_on_sidewalk(
    seed: u64,
    layout: &CityLayout,
    params: &CityParams,
    idx: usize,
    margin: f32,
    what: &str,
) {
    let (point, along) = sidewalk_anchor(layout, params, idx, margin)
        .unwrap_or_else(|| panic!("seed {seed}: {what} {idx} has no sidewalk anchor"));
    assert!(
        (along.length() - 1.0).abs() < 1e-4,
        "seed {seed}: direction {along} is not unit"
    );
    for p in [point, point + along * margin, point - along * margin] {
        let on_sidewalk = layout.roads.edges.iter().any(|e| {
            let half = params.half_carriageway(e.class);
            let (a, b) = (
                layout.roads.nodes[e.a as usize],
                layout.roads.nodes[e.b as usize],
            );
            let d = dist_point_segment(p, a, b);
            e.class != RoadClass::Alley && d >= half && d <= half + params.sidewalk(e.class)
        });
        assert!(on_sidewalk, "seed {seed}: {p} is not on a sidewalk");
        for (i, e) in layout.roads.edges.iter().enumerate() {
            let (a, b) = (
                layout.roads.nodes[e.a as usize],
                layout.roads.nodes[e.b as usize],
            );
            let d = dist_point_segment(p, a, b);
            assert!(
                d >= params.half_carriageway(e.class) - 1e-3,
                "seed {seed}: {p} is on the carriageway of edge {i} ({d} m from its axis)"
            );
        }
        for (i, lot) in layout.lots.iter().enumerate() {
            assert!(
                !contains_convex(&lot.polygon, p, 0.0),
                "seed {seed}: {p} inside lot {i}"
            );
        }
    }
    let center = layout.buildings[idx].center;
    let off = (center - point).dot(along).abs();
    assert!(
        off <= margin + 1e-3,
        "seed {seed}: anchor {point} is {off} m along the sidewalk from {what} {center}"
    );
}

#[test]
fn hospital_anchor_on_sidewalk() {
    let params = shipped_params();
    let margin = pickup_spacing();
    for (seed, layout) in layouts() {
        let hospitals = layout
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BuildingKind::Hospital)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        let &[idx] = hospitals.as_slice() else {
            panic!("seed {seed}: expected one hospital, found {hospitals:?}");
        };
        assert_anchor_on_sidewalk(*seed, layout, &params, idx, margin, "hospital");
    }
}

/// The police station respawn (`world::station_spawn`) uses the first station with the hospital margin.
#[test]
fn police_station_anchor_on_sidewalk() {
    let params = shipped_params();
    let margin = pickup_spacing();
    for (seed, layout) in layouts() {
        let stations = layout
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BuildingKind::PoliceStation)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        assert!(!stations.is_empty(), "seed {seed}: no police station");
        for idx in stations {
            assert_anchor_on_sidewalk(*seed, layout, &params, idx, margin, "police station");
        }
    }
}

#[test]
fn sidewalk_anchor_rejects_bad_indices() {
    let params = shipped_params();
    let margin = pickup_spacing();
    let (seed, layout) = &layouts()[0];
    let idx = layout
        .buildings
        .iter()
        .position(|b| b.kind == BuildingKind::Hospital)
        .unwrap_or_else(|| panic!("seed {seed}: no hospital"));
    assert!(sidewalk_anchor(layout, &params, layout.buildings.len(), margin).is_none());
    let mut broken = layout.clone();
    let block = broken.lots[broken.buildings[idx].lot as usize].block as usize;
    broken.blocks[block].nodes[0] = u32::MAX;
    assert!(
        sidewalk_anchor(&broken, &params, idx, margin).is_none(),
        "seed {seed}: an out-of-range block node must give None"
    );
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

#[test]
fn landmarks_exist() {
    let params = shipped_params();
    for (seed, layout) in layouts() {
        let lm = layout.landmarks;
        let tower = &layout.buildings[lm.tower as usize];
        assert_eq!(tower.kind, BuildingKind::Tower, "seed {seed}: tower kind");
        let towers = layout
            .buildings
            .iter()
            .filter(|b| b.kind == BuildingKind::Tower)
            .count();
        assert_eq!(towers, 1, "seed {seed}: tower count");
        for (i, b) in layout.buildings.iter().enumerate() {
            assert!(
                i == lm.tower as usize || b.height < tower.height,
                "seed {seed}: building {i} height {} not below tower {}",
                b.height,
                tower.height
            );
        }
        let plaza_lots = (0..layout.lots.len())
            .filter(|&l| layout.lots[l].block == lm.plaza)
            .collect::<Vec<_>>();
        assert_eq!(
            plaza_lots.len(),
            1,
            "seed {seed}: plaza lots {plaza_lots:?}"
        );
        let on_plaza = (0..layout.buildings.len())
            .filter(|&b| layout.buildings[b].lot as usize == plaza_lots[0])
            .collect::<Vec<_>>();
        assert_eq!(
            on_plaza,
            [lm.tower as usize],
            "seed {seed}: plaza buildings"
        );
        let park = &layout.blocks[lm.park as usize];
        assert!(park.is_park, "seed {seed}: park block not a park");
        assert_ne!(lm.park, lm.plaza, "seed {seed}: park == plaza");
        let r = centroid(&park.inner).length();
        assert!(
            r <= params.landmarks.park_radius,
            "seed {seed}: park at {r} m from the centre"
        );
    }
}

#[test]
fn setback_tiers_nested() {
    let params = shipped_params();
    let m = &params.massing;
    let mut tiered_non_tower_on_seed_1 = false;
    for (seed, layout) in layouts() {
        for (i, b) in layout.buildings.iter().enumerate() {
            let floor_h = if b.kind == BuildingKind::Tower {
                params.districts.downtown.floor_height
            } else {
                params
                    .district(layout.districts[district_of_building(layout, i) as usize].kind)
                    .floor_height
            };
            if ((b.height / floor_h).round() as u32) < m.setback_min_floors {
                assert!(
                    b.upper_tiers.is_empty(),
                    "seed {seed}: low building {i} has tiers"
                );
                continue;
            }
            let (mut prev_bottom, mut prev_half) = (0.0, b.half_extents);
            for (k, tier) in b.upper_tiers.iter().enumerate() {
                assert!(
                    tier.bottom > prev_bottom && tier.bottom < b.height,
                    "seed {seed}: building {i} tier {k} bottom {}",
                    tier.bottom
                );
                let floors = tier.bottom / floor_h;
                assert!(
                    (floors - floors.round()).abs() < 1e-3,
                    "seed {seed}: building {i} tier {k} bottom {} is not on a floor",
                    tier.bottom
                );
                let expected = prev_half - Vec2::splat(m.setback_inset);
                assert!(
                    (tier.half_extents - expected).length() < 1e-4,
                    "seed {seed}: building {i} tier {k} half {} expected {expected}",
                    tier.half_extents
                );
                assert!(
                    tier.half_extents.min_element() >= m.setback_min_half,
                    "seed {seed}: building {i} tier {k} too thin"
                );
                (prev_bottom, prev_half) = (tier.bottom, tier.half_extents);
            }
            if *seed == 1 && b.kind != BuildingKind::Tower && !b.upper_tiers.is_empty() {
                tiered_non_tower_on_seed_1 = true;
            }
        }
    }
    assert!(
        tiered_non_tower_on_seed_1,
        "seed 1: no building besides the tower has setback tiers"
    );
}

/// The avenue edge and direction (`true` = a→b) a spot lies on, if it sits on an avenue curb lane.
fn spot_lane(
    params: &CityParams,
    layout: &CityLayout,
    spot: &citygen::ParkingSpot,
) -> Option<(usize, bool)> {
    let cfg = &params.parking;
    let from_centre = params.half_carriageway(RoadClass::Avenue) - cfg.curb_offset;
    layout.roads.edges.iter().enumerate().find_map(|(e, edge)| {
        if edge.class != RoadClass::Avenue {
            return None;
        }
        let (a, b) = (
            layout.roads.nodes[edge.a as usize],
            layout.roads.nodes[edge.b as usize],
        );
        let len = (b - a).length();
        let d = (b - a) / len;
        if d.perp_dot(spot.heading).abs() >= 1e-4 {
            return None;
        }
        let forward = spot.heading.dot(d) > 0.0;
        let (start, dir) = if forward { (a, d) } else { (b, -d) };
        let rel = spot.position - start;
        let along = rel.dot(dir);
        let side = rel.dot(dir.perp());
        let on_lane = (side - from_centre).abs() < 0.01
            && along >= cfg.end_margin - 0.01
            && along <= len - cfg.end_margin + 0.01;
        on_lane.then_some((e, forward))
    })
}

#[test]
fn parking_spots_on_avenue_curb_lanes() {
    let params = shipped_params();
    let cfg = &params.parking;
    let mut counts = Vec::new();
    for (seed, layout) in layouts() {
        assert!(!layout.parking.is_empty(), "seed {seed}: no parking spots");
        let mut by_lane: std::collections::HashMap<(usize, bool), Vec<Vec2>> = Default::default();
        for (i, spot) in layout.parking.iter().enumerate() {
            let lane = spot_lane(&params, layout, spot).unwrap_or_else(|| {
                panic!(
                    "seed {seed}: spot {i} at {} (heading {}) is not on an avenue curb lane",
                    spot.position, spot.heading
                )
            });
            for (b, block) in layout.blocks.iter().enumerate() {
                assert!(
                    !contains_convex(&block.curb, spot.position, 0.0),
                    "seed {seed}: spot {i} inside block {b}"
                );
            }
            by_lane.entry(lane).or_default().push(spot.position);
        }
        for ((edge, forward), spots) in by_lane {
            for (i, a) in spots.iter().enumerate() {
                for b in &spots[i + 1..] {
                    assert!(
                        a.distance(*b) >= cfg.spacing - 0.01,
                        "seed {seed}: edge {edge} ({forward}) spots {a} and {b} closer than spacing"
                    );
                }
            }
        }
        counts.push((*seed, layout.parking.len()));
    }
    println!("parking spots per seed: {counts:?}");
}
