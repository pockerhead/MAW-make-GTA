use super::config::{RenderConfig, Rgb};
use bevy::{
    asset::RenderAssetUsages,
    color::ColorToComponents,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use gta_sim::world::{BuildingKind, CityLayout, CityParams, DistrictKind, RoadClass, centroid};

// UV of every non-facade vertex; the facade shader draws windows only where uv >= 0.
const NO_UV: [f32; 2] = [-1.0, -1.0];

/// Merged city geometry of one render chunk, in world coordinates.
pub(super) struct ChunkMesh {
    pub coord: UVec2,
    pub mesh: Mesh,
}

/// Square chunk grid over the city; the outer chunks stretch to the ground edge.
pub(super) struct ChunkGrid {
    pub n: u32,
    chunk: f32,
    size: f32,
    ground: f32,
}

impl ChunkGrid {
    pub(super) fn new(size: f32, chunk: f32, ground: f32) -> Self {
        Self {
            n: (size / chunk).ceil() as u32,
            chunk,
            size,
            ground,
        }
    }

    fn cell(&self, x: f32) -> u32 {
        let i = ((x + self.size / 2.0) / self.chunk).floor() as i64;
        i.clamp(0, i64::from(self.n) - 1) as u32
    }

    pub(super) fn chunk_of(&self, p: Vec2) -> UVec2 {
        UVec2::new(self.cell(p.x), self.cell(p.y))
    }

    /// Asphalt extent `[lo, hi]` of chunk row/column `i`.
    pub(super) fn bounds(&self, i: u32) -> (f32, f32) {
        let mut lo = -self.size / 2.0 + i as f32 * self.chunk;
        let mut hi = lo + self.chunk;
        if i == 0 {
            lo = -self.ground / 2.0;
        }
        if i == self.n - 1 {
            hi = self.ground / 2.0;
        }
        (lo, hi)
    }

    fn index(&self, c: UVec2) -> usize {
        (c.x * self.n + c.y) as usize
    }
}

fn linear((r, g, b): Rgb) -> [f32; 4] {
    Color::srgb(r, g, b).to_linear().to_f32_array()
}

#[derive(Default)]
struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    fn vertex(&mut self, p: Vec3, normal: Vec3, uv: [f32; 2], color: [f32; 4]) {
        self.positions.push(p.to_array());
        self.normals.push(normal.to_array());
        self.uvs.push(uv);
        self.colors.push(color);
    }

    /// Counter-clockwise (positive-area in layout x, y) convex polygon at height `y`, facing +Y.
    fn flat(&mut self, poly: &[Vec2], y: f32, color: [f32; 4]) {
        let base = self.positions.len() as u32;
        for p in poly {
            self.vertex(Vec3::new(p.x, y, p.y), Vec3::Y, NO_UV, color);
        }
        // Positive area in (x, z) is clockwise seen from +Y, so the fan is (0, i + 1, i).
        for i in 1..poly.len() as u32 - 1 {
            self.indices.extend([base, base + i + 1, base + i]);
        }
    }

    /// Vertical quad on edge `a -> b` of a counter-clockwise outline, facing outwards.
    /// `uv_scale` = (bays along the edge, floor height) gives facade UVs; `None` gives `NO_UV`.
    fn wall(
        &mut self,
        a: Vec2,
        b: Vec2,
        y: (f32, f32),
        color: [f32; 4],
        uv_scale: Option<(f32, f32)>,
    ) {
        let out = -(b - a).perp().normalize();
        let normal = Vec3::new(out.x, 0.0, out.y);
        let base = self.positions.len() as u32;
        let uv = |u: f32, height: f32| {
            uv_scale.map_or(NO_UV, |(bays, floor)| [u * bays, height / floor])
        };
        self.vertex(Vec3::new(a.x, y.0, a.y), normal, uv(0.0, y.0), color);
        self.vertex(Vec3::new(b.x, y.0, b.y), normal, uv(1.0, y.0), color);
        self.vertex(Vec3::new(b.x, y.1, b.y), normal, uv(1.0, y.1), color);
        self.vertex(Vec3::new(a.x, y.1, a.y), normal, uv(0.0, y.1), color);
        // A0, B1, B0, A0, A1, B1 with A0, B0, B1, A1 = base + 0, 1, 2, 3.
        self.indices
            .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    /// Flat strip from `start` to `end` of the given width at height `y`.
    fn strip(&mut self, start: Vec2, end: Vec2, width: f32, y: f32, color: [f32; 4]) {
        let n = (end - start).normalize().perp() * (width / 2.0);
        self.flat(&[start - n, end - n, end + n, start + n], y, color);
    }

    fn into_mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

struct Chunks {
    grid: ChunkGrid,
    builders: Vec<MeshBuilder>,
}

impl Chunks {
    fn at(&mut self, p: Vec2) -> &mut MeshBuilder {
        let index = self.grid.index(self.grid.chunk_of(p));
        &mut self.builders[index]
    }
}

fn district_color(colors: &super::config::DistrictColors, kind: DistrictKind) -> &Rgb {
    match kind {
        DistrictKind::Downtown => &colors.downtown,
        DistrictKind::Commercial => &colors.commercial,
        DistrictKind::Residential => &colors.residential,
        DistrictKind::Industrial => &colors.industrial,
    }
}

/// Builds the merged city meshes: exactly `n²` chunks sorted by coordinate (x-major).
pub(super) fn build_city_meshes(
    layout: &CityLayout,
    params: &CityParams,
    config: &RenderConfig,
) -> Vec<ChunkMesh> {
    let grid = ChunkGrid::new(params.size, config.chunk_size, layout.ground_size);
    let n = grid.n;
    let mut chunks = Chunks {
        builders: (0..n * n).map(|_| MeshBuilder::default()).collect(),
        grid,
    };
    let road = linear(config.road_color);
    for x in 0..n {
        for z in 0..n {
            let ((x0, x1), (z0, z1)) = (chunks.grid.bounds(x), chunks.grid.bounds(z));
            let quad = [
                Vec2::new(x0, z0),
                Vec2::new(x1, z0),
                Vec2::new(x1, z1),
                Vec2::new(x0, z1),
            ];
            let index = chunks.grid.index(UVec2::new(x, z));
            chunks.builders[index].flat(&quad, 0.0, road);
        }
    }
    add_blocks(&mut chunks, layout, params, config);
    add_buildings(&mut chunks, layout, params, config);
    add_markings(&mut chunks, layout, params, config);
    (0..n * n)
        .zip(chunks.builders)
        .map(|(i, builder)| ChunkMesh {
            coord: UVec2::new(i / n, i % n),
            mesh: builder.into_mesh(),
        })
        .collect()
}

fn add_blocks(
    chunks: &mut Chunks,
    layout: &CityLayout,
    params: &CityParams,
    config: &RenderConfig,
) {
    let curb_h = params.roads.curb_height;
    let (curb_color, sidewalk) = (linear(config.curb_color), linear(config.sidewalk_color));
    for (id, block) in layout.blocks.iter().enumerate() {
        if block.curb.len() < 3 {
            continue;
        }
        let builder = chunks.at(centroid(&block.curb));
        let n = block.curb.len();
        for k in 0..n {
            builder.wall(
                block.curb[k],
                block.curb[(k + 1) % n],
                (0.0, curb_h),
                curb_color,
                None,
            );
        }
        if block.inner.is_empty() {
            builder.flat(&block.curb, curb_h, sidewalk);
            continue;
        }
        for k in 0..n {
            let next = (k + 1) % n;
            let ring = [
                block.curb[k],
                block.curb[next],
                block.inner[next],
                block.inner[k],
            ];
            builder.flat(&ring, curb_h, sidewalk);
        }
        let inner = if block.is_park {
            config.park_color
        } else if id as u32 == layout.landmarks.plaza {
            config.plaza_color
        } else {
            let kind = layout.districts[block.district as usize].kind;
            *district_color(&config.lot_colors, kind)
        };
        builder.flat(&block.inner, curb_h, linear(inner));
    }
}

fn add_buildings(
    chunks: &mut Chunks,
    layout: &CityLayout,
    params: &CityParams,
    config: &RenderConfig,
) {
    for b in &layout.buildings {
        let block = layout.lots[b.lot as usize].block as usize;
        let district = layout.districts[layout.blocks[block].district as usize].kind;
        let floor_h = if b.kind == BuildingKind::Tower {
            params.districts.downtown.floor_height
        } else {
            params.district(district).floor_height
        };
        let color = linear(match b.kind {
            BuildingKind::Tower => config.tower_color,
            BuildingKind::Hospital => config.hospital_color,
            BuildingKind::PoliceStation => config.police_color,
            BuildingKind::GangHq(_) => config.gang_hq_color,
            BuildingKind::Generic => *district_color(&config.district_colors, district),
        });
        let mut slabs = vec![(0.0, b.half_extents)];
        slabs.extend(b.upper_tiers.iter().map(|t| (t.bottom, t.half_extents)));
        let builder = chunks.at(b.center);
        let (u, v) = (b.axis, b.axis.perp());
        for (k, &(bottom, h)) in slabs.iter().enumerate() {
            let top = slabs.get(k + 1).map_or(b.height, |next| next.0);
            let c = b.center;
            let corners = [
                c - u * h.x - v * h.y,
                c + u * h.x - v * h.y,
                c + u * h.x + v * h.y,
                c - u * h.x + v * h.y,
            ];
            for i in 0..4 {
                let (a, e) = (corners[i], corners[(i + 1) % 4]);
                let bays = ((e - a).length() / config.facade.bay_width)
                    .round()
                    .max(1.0);
                builder.wall(a, e, (bottom, top), color, Some((bays, floor_h)));
            }
            builder.flat(&corners, top, color);
        }
    }
}

/// Centred run of dashes of length `dash` separated by `gap` over `[s0, s1]`; empty if none fits.
fn dashes(s0: f32, s1: f32, dash: f32, gap: f32) -> Vec<(f32, f32)> {
    let usable = s1 - s0;
    let count = ((usable + gap) / (dash + gap)).floor().max(0.0) as u32;
    if count == 0 {
        return Vec::new();
    }
    let total = count as f32 * dash + (count - 1) as f32 * gap;
    let start = s0 + (usable - total) / 2.0;
    (0..count)
        .map(|i| {
            let t = start + i as f32 * (dash + gap);
            (t, t + dash)
        })
        .collect()
}

/// Centre lines, lane dividers and crosswalks of every non-alley road, at `surface_layer_step`.
fn add_markings(
    chunks: &mut Chunks,
    layout: &CityLayout,
    params: &CityParams,
    config: &RenderConfig,
) {
    let m = &config.markings;
    let y = config.surface_layer_step;
    let (color, center_color) = (linear(m.color), linear(m.center_color));
    let roads = &layout.roads;
    let mut trim = vec![0.0_f32; roads.nodes.len()];
    for edge in &roads.edges {
        let half = params.half_carriageway(edge.class);
        trim[edge.a as usize] = trim[edge.a as usize].max(half);
        trim[edge.b as usize] = trim[edge.b as usize].max(half);
    }
    let strip = |chunks: &mut Chunks, (from, to): (Vec2, Vec2), width: f32, color| {
        chunks
            .at((from + to) / 2.0)
            .strip(from, to, width, y, color);
    };
    for edge in &roads.edges {
        let lanes = match edge.class {
            RoadClass::Avenue => params.roads.avenue.lanes_per_direction,
            RoadClass::Street => params.roads.street.lanes_per_direction,
            RoadClass::Alley => continue,
        };
        let (p, q) = (roads.nodes[edge.a as usize], roads.nodes[edge.b as usize]);
        let length = (q - p).length();
        let d = (q - p) / length;
        let side = d.perp();
        let (trim_a, trim_b) = (trim[edge.a as usize], trim[edge.b as usize]);
        let (s0, s1) = (
            trim_a + m.crosswalk_depth,
            length - trim_b - m.crosswalk_depth,
        );
        if s1 <= s0 {
            continue;
        }
        let at = |t: f32, offset: f32| p + d * t + side * offset;
        if edge.class == RoadClass::Avenue {
            strip(chunks, (at(s0, 0.0), at(s1, 0.0)), m.width, center_color);
        } else {
            for (t0, t1) in dashes(s0, s1, m.dash, m.gap) {
                strip(chunks, (at(t0, 0.0), at(t1, 0.0)), m.width, color);
            }
        }
        for k in 1..lanes {
            for offset in [1.0, -1.0].map(|s| s * k as f32 * params.roads.lane_width) {
                for (t0, t1) in dashes(s0, s1, m.dash, m.gap) {
                    strip(chunks, (at(t0, offset), at(t1, offset)), m.width, color);
                }
            }
        }
        let half = params.half_carriageway(edge.class);
        let stripes = dashes(-half, half, m.crosswalk_stripe, m.crosswalk_gap);
        for (t0, t1) in [
            (trim_a, s0),
            (length - trim_b - m.crosswalk_depth, length - trim_b),
        ] {
            for &(o0, o1) in &stripes {
                let offset = (o0 + o1) / 2.0;
                strip(
                    chunks,
                    (at(t0, offset), at(t1, offset)),
                    m.crosswalk_stripe,
                    color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashes_are_centred() {
        assert_eq!(
            dashes(0.0, 21.0, 3.0, 6.0),
            [(0.0, 3.0), (9.0, 12.0), (18.0, 21.0)]
        );
        assert_eq!(
            dashes(0.0, 23.0, 3.0, 6.0),
            [(1.0, 4.0), (10.0, 13.0), (19.0, 22.0)]
        );
        assert!(dashes(0.0, 2.0, 3.0, 6.0).is_empty());
    }

    #[test]
    fn grid_covers_ground() {
        let grid = ChunkGrid::new(1200.0, 128.0, 1400.0);
        assert_eq!(grid.n, 10);
        assert_eq!(grid.chunk_of(Vec2::ZERO), UVec2::new(4, 4));
        assert_eq!(grid.chunk_of(Vec2::splat(-600.0)), UVec2::ZERO);
        assert_eq!(grid.chunk_of(Vec2::new(620.0, 0.0)).x, 9);
        assert_eq!(grid.chunk_of(Vec2::new(700.0, 0.0)).x, 9);
        assert_eq!(grid.bounds(0), (-700.0, -472.0));
        assert_eq!(grid.bounds(9), (552.0, 700.0));
        for i in 0..grid.n - 1 {
            assert_eq!(
                grid.bounds(i).1,
                grid.bounds(i + 1).0,
                "gap after chunk {i}"
            );
        }
    }
}
