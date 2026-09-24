//! Minimap raster and projection gates (GDD §7, T12): the raster is deterministic per seed and
//! oriented by its contract; a point ahead of the camera is straight up on the map.

mod common;

use citygen::minimap::{Raster, RasterStyle, heading_on_map, map_px, project, rasterize};
use citygen::{CityLayout, Vec2, contains_convex};
use common::{SEEDS, layouts};
use glam::{Quat, Vec3, Vec3Swizzles};

const BLESS: &str =
    "cargo test -p citygen --test minimap -- --ignored bless_print_minimap_golden --nocapture";

/// Independent of `strings.ron`: colour tuning never re-blesses the golden.
fn style() -> RasterStyle {
    RasterStyle {
        px_per_m: 1.0,
        road: [40, 40, 44, 255],
        sidewalk: [150, 150, 150, 255],
        block: [90, 96, 110, 255],
        park: [60, 140, 60, 255],
        building: [210, 200, 190, 255],
        territory: [[220, 40, 40, 96], [40, 60, 220, 160]],
    }
}

fn layout(seed: u64) -> &'static CityLayout {
    &layouts()
        .iter()
        .find(|(s, _)| *s == seed)
        .unwrap_or_else(|| panic!("GATE BROKEN: no layout for seed {seed}"))
        .1
}

fn golden_minimap() -> Vec<(u64, u64)> {
    include_str!("golden_minimap.txt")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (seed, hash) = line
                .split_once(' ')
                .unwrap_or_else(|| panic!("GATE BROKEN: bad golden line {line:?}"));
            (
                seed.parse()
                    .unwrap_or_else(|_| panic!("GATE BROKEN: bad seed in {line:?}")),
                u64::from_str_radix(hash.trim().trim_start_matches("0x"), 16)
                    .unwrap_or_else(|_| panic!("GATE BROKEN: bad hash in {line:?}")),
            )
        })
        .collect()
}

#[test]
fn raster_hashes_match_golden() {
    let golden = golden_minimap();
    assert_eq!(
        golden.iter().map(|g| g.0).collect::<Vec<_>>(),
        SEEDS.to_vec(),
        "GATE BROKEN: golden_minimap.txt must list seeds {SEEDS:?}"
    );
    for (seed, expected) in golden {
        let actual = rasterize(layout(seed), &style()).hash();
        assert_eq!(
            actual, expected,
            "seed {seed}: minimap hash {actual:#018x}, golden {expected:#018x}; if the change is intended, bless: {BLESS}"
        );
    }
}

#[test]
#[ignore]
fn bless_print_minimap_golden() {
    for seed in SEEDS {
        println!("{seed} {:#018x}", rasterize(layout(seed), &style()).hash());
    }
}

#[test]
fn raster_differs_by_seed() {
    let [a, b, c] = SEEDS.map(|s| rasterize(layout(s), &style()).hash());
    assert!(a != b && b != c && a != c, "{a:#x} {b:#x} {c:#x}");
}

/// Reads the raster by its documented contract only, never through a raster helper.
struct Contract<'a> {
    raster: &'a Raster,
    origin: Vec2,
    ppm: f32,
}

impl<'a> Contract<'a> {
    fn new(layout: &CityLayout, raster: &'a Raster) -> Self {
        let g = layout.ground_size;
        let ppm = style().px_per_m;
        let size = (g * ppm).ceil() as u32;
        assert_eq!(
            (raster.width, raster.height),
            (size, size),
            "raster size is not ceil(ground_size * px_per_m)"
        );
        assert_eq!(raster.rgba.len(), (size * size * 4) as usize);
        Self {
            raster,
            origin: Vec2::splat(-g / 2.0),
            ppm,
        }
    }

    fn cell(&self, p: Vec2) -> (u32, u32) {
        let c = ((p - self.origin) * self.ppm).floor();
        (c.x as u32, c.y as u32)
    }

    fn centre(&self, (col, row): (u32, u32)) -> Vec2 {
        self.origin + (Vec2::new(col as f32, row as f32) + 0.5) / self.ppm
    }

    fn mirrored(&self, (col, row): (u32, u32)) -> (u32, u32) {
        (col, self.raster.height - 1 - row)
    }

    fn texel(&self, (col, row): (u32, u32)) -> [u8; 4] {
        let at = ((row * self.raster.width + col) * 4) as usize;
        self.raster.rgba[at..at + 4].try_into().unwrap()
    }
}

fn in_building(layout: &CityLayout, p: Vec2) -> bool {
    layout.buildings.iter().any(|b| {
        let d = p - b.center;
        d.dot(b.axis).abs() <= b.half_extents.x && d.dot(b.axis.perp()).abs() <= b.half_extents.y
    })
}

fn in_gang_block(layout: &CityLayout, p: Vec2) -> bool {
    layout
        .blocks
        .iter()
        .any(|b| layout.gang_districts.contains(&b.district) && contains_convex(&b.curb, p, 0.0))
}

fn in_any_curb(layout: &CityLayout, p: Vec2) -> bool {
    layout
        .blocks
        .iter()
        .any(|b| b.curb.len() >= 3 && contains_convex(&b.curb, p, 0.0))
}

fn blend(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let a = u32::from(src[3]);
    let mix = |d: u8, s: u8| ((u32::from(d) * (255 - a) + u32::from(s) * a + 127) / 255) as u8;
    [
        mix(dst[0], src[0]),
        mix(dst[1], src[1]),
        mix(dst[2], src[2]),
        255,
    ]
}

/// Midpoint of `curb[0]` and `inner[0]` of the first block of `pick` whose texel centre lies on the
/// sidewalk ring, outside every building.
fn sidewalk_probe(
    layout: &CityLayout,
    map: &Contract,
    pick: impl Fn(&citygen::Block) -> bool,
) -> Option<(u32, u32)> {
    layout.blocks.iter().filter(|b| pick(b)).find_map(|b| {
        if b.curb.len() < 3 || b.inner.len() < 3 {
            return None;
        }
        let cell = map.cell((b.curb[0] + b.inner[0]) / 2.0);
        let c = map.centre(cell);
        let ring = contains_convex(&b.curb, c, 0.0) && !contains_convex(&b.inner, c, 0.0);
        (ring && !in_building(layout, c)).then_some(cell)
    })
}

#[test]
fn raster_marks_known_places() {
    let layout = layout(1);
    let raster = rasterize(layout, &style());
    let map = Contract::new(layout, &raster);
    let s = style();
    let gang_block = |b: &citygen::Block| layout.gang_districts.contains(&b.district);

    // (a) road: a road edge midpoint off every block.
    let road = layout
        .roads
        .edges
        .iter()
        .map(|e| {
            map.cell((layout.roads.nodes[e.a as usize] + layout.roads.nodes[e.b as usize]) / 2.0)
        })
        .find(|&cell| !in_any_curb(layout, map.centre(cell)))
        .expect("GATE BROKEN: no road edge midpoint off the blocks");
    assert_eq!(map.texel(road), s.road, "(a) road texel {road:?}");

    // (b) building whose row-mirrored texel is neither a building nor a territory.
    let mut buildings: Vec<usize> = (0..layout.buildings.len()).collect();
    buildings.sort_by_key(|&i| layout.buildings[i].kind != citygen::BuildingKind::Hospital);
    let building = buildings
        .into_iter()
        .map(|i| map.cell(layout.buildings[i].center))
        .find(|&cell| {
            let (c, m) = (map.centre(cell), map.centre(map.mirrored(cell)));
            in_building(layout, c)
                && !in_gang_block(layout, c)
                && !in_building(layout, m)
                && !in_gang_block(layout, m)
        })
        .expect("GATE BROKEN: no building with a free mirrored texel");
    assert_eq!(
        map.texel(building),
        s.building,
        "(b) building texel {building:?}"
    );

    // (c) park centroid.
    let park = &layout.blocks[layout.landmarks.park as usize];
    assert!(!gang_block(park), "GATE BROKEN: the park is a gang block");
    let park_cell = map.cell(citygen::centroid(&park.inner));
    let c = map.centre(park_cell);
    assert!(
        contains_convex(&park.inner, c, 0.0) && !in_building(layout, c),
        "GATE BROKEN: park centroid texel is not open park"
    );
    assert_eq!(map.texel(park_cell), s.park, "(c) park texel {park_cell:?}");

    // (d) plain block: 1 m inside an inner vertex, no building there.
    let block = layout
        .blocks
        .iter()
        .filter(|b| !b.is_park && !gang_block(b) && b.inner.len() >= 3)
        .flat_map(|b| {
            let centre = citygen::centroid(&b.inner);
            b.inner
                .iter()
                .map(move |&v| (b, v + (centre - v).normalize() * 1.0))
        })
        .map(|(b, p)| (b, map.cell(p)))
        .find(|&(b, cell)| {
            let c = map.centre(cell);
            contains_convex(&b.inner, c, 0.0) && !in_building(layout, c)
        })
        .map(|(_, cell)| cell)
        .expect("GATE BROKEN: no open block corner");
    assert_eq!(map.texel(block), s.block, "(d) block texel {block:?}");

    // (e) sidewalk of a non-gang block.
    let sidewalk = sidewalk_probe(layout, &map, |b| !gang_block(b))
        .expect("GATE BROKEN: no sidewalk probe outside the gangs");
    assert_eq!(
        map.texel(sidewalk),
        s.sidewalk,
        "(e) sidewalk texel {sidewalk:?}"
    );

    // (f) sidewalk of a gang-0 block, tinted.
    let turf = sidewalk_probe(layout, &map, |b| b.district == layout.gang_districts[0])
        .expect("GATE BROKEN: no sidewalk probe in gang 0");
    assert_eq!(
        map.texel(turf),
        blend(s.sidewalk, s.territory[0]),
        "(f) territory texel {turf:?}"
    );

    // (g) a mirrored raster would put the building there.
    assert_ne!(
        map.texel(map.mirrored(building)),
        s.building,
        "(g) mirrored texel of {building:?} is a building"
    );
}

fn forward(yaw: f32) -> Vec2 {
    (Quat::from_rotation_y(yaw) * Vec3::NEG_Z).xz()
}

fn right(yaw: f32) -> Vec2 {
    (Quat::from_rotation_y(yaw) * Vec3::X).xz()
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).abs().max_element() < 1e-4
}

#[test]
fn camera_ahead_is_straight_up() {
    let centre = Vec2::new(123.0, -45.0);
    for deg in [0.0_f32, 90.0, 180.0] {
        let yaw = deg.to_radians();
        let ahead = centre + 10.0 * forward(yaw);
        let m = project(ahead, centre, yaw);
        assert!(
            close(m, Vec2::new(0.0, 10.0)),
            "yaw {deg}: 10 m ahead projects to {m}"
        );
        let px = map_px(ahead, centre, yaw, 2.0);
        assert!(
            close(px, Vec2::new(0.0, -20.0)),
            "yaw {deg}: 10 m ahead maps to {px} px"
        );
        let side = project(centre + 10.0 * right(yaw), centre, yaw);
        assert!(
            close(side, Vec2::new(10.0, 0.0)),
            "yaw {deg}: 10 m right projects to {side}"
        );
    }
}

#[test]
fn arrow_points_along_facing() {
    let centre = Vec2::new(-30.0, 17.0);
    for (camera, facing) in [(0.0_f32, 90.0_f32), (90.0, 90.0), (90.0, 0.0)] {
        let (cam, f) = (camera.to_radians(), facing.to_radians());
        let h = heading_on_map(f, cam);
        let expected = Vec2::new(-h.sin(), h.cos());
        let actual = project(centre + forward(f), centre, cam).normalize();
        assert!(
            close(expected, actual),
            "camera {camera}, facing {facing}: arrow {expected}, body {actual}"
        );
    }
}
