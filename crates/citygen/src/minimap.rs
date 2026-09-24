//! Minimap raster of a layout and the world-to-map projection shared by the client and its shader.

use crate::rng::fnv1a64;
use crate::{CityLayout, Vec2};

/// Colours of the raster layers, sRGB RGBA8; territory alpha is `territory[g][3]`.
#[derive(Clone, Debug)]
pub struct RasterStyle {
    pub px_per_m: f32,
    pub road: [u8; 4],
    pub sidewalk: [u8; 4],
    pub block: [u8; 4],
    pub park: [u8; 4],
    pub building: [u8; 4],
    pub territory: [[u8; 4]; 2],
}

/// Top-down picture of the ground square `[-g/2, g/2]²`, `g = layout.ground_size`, with layout
/// (x, y) = world (x, z). `origin = (-g/2, -g/2)`; texel `(col, row)` covers
/// x ∈ `origin.x + [col, col + 1) / px_per_m` and z ∈ `origin.y + [row, row + 1) / px_per_m`, so
/// row 0 is the smallest z (north: forward is −Z). `width = height = ceil(g · px_per_m)`; each texel
/// holds the layer under its centre. `rgba` is row-major, 4 bytes per texel.
#[derive(Clone, Debug)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub origin: Vec2,
    pub px_per_m: f32,
    pub rgba: Vec<u8>,
}

impl Raster {
    /// FNV-1a over the size, origin (mm), scale and pixels.
    pub fn hash(&self) -> u64 {
        let mut bytes = Vec::with_capacity(28 + self.rgba.len());
        bytes.extend_from_slice(&self.width.to_le_bytes());
        bytes.extend_from_slice(&self.height.to_le_bytes());
        for v in [self.origin.x, self.origin.y] {
            bytes.extend_from_slice(&((v * 1000.0).round() as i64).to_le_bytes());
        }
        bytes.extend_from_slice(&self.px_per_m.to_bits().to_le_bytes());
        bytes.extend_from_slice(&self.rgba);
        fnv1a64(&bytes)
    }

    fn texel_centre(&self, col: u32, row: u32) -> Vec2 {
        self.origin + (Vec2::new(col as f32, row as f32) + 0.5) / self.px_per_m
    }

    /// Texel columns and rows whose cells overlap the box `[lo, hi]`.
    fn span(&self, lo: Vec2, hi: Vec2) -> (std::ops::Range<u32>, std::ops::Range<u32>) {
        let cell =
            |v: f32, o: f32, n: u32| ((v - o) * self.px_per_m).floor().clamp(0.0, n as f32) as u32;
        let cols = cell(lo.x, self.origin.x, self.width)
            ..(cell(hi.x, self.origin.x, self.width) + 1).min(self.width);
        let rows = cell(lo.y, self.origin.y, self.height)
            ..(cell(hi.y, self.origin.y, self.height) + 1).min(self.height);
        (cols, rows)
    }

    /// Calls `paint` with the bytes of every texel whose centre `inside` accepts within the box.
    fn scan(
        &mut self,
        lo: Vec2,
        hi: Vec2,
        inside: impl Fn(Vec2) -> bool,
        paint: impl Fn(&mut [u8]),
    ) {
        let (cols, rows) = self.span(lo, hi);
        for row in rows {
            for col in cols.clone() {
                if !inside(self.texel_centre(col, row)) {
                    continue;
                }
                let at = (row as usize * self.width as usize + col as usize) * 4;
                paint(&mut self.rgba[at..at + 4]);
            }
        }
    }
}

/// A convex polygon with its edges precomputed for point tests.
struct Convex {
    edges: Vec<(Vec2, Vec2)>,
    lo: Vec2,
    hi: Vec2,
}

impl Convex {
    fn new(poly: &[Vec2]) -> Self {
        let edges = (0..poly.len())
            .map(|i| (poly[i], (poly[(i + 1) % poly.len()] - poly[i]).normalize()))
            .collect();
        let lo = poly
            .iter()
            .copied()
            .fold(Vec2::splat(f32::INFINITY), Vec2::min);
        let hi = poly
            .iter()
            .copied()
            .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
        Self { edges, lo, hi }
    }

    fn contains(&self, p: Vec2) -> bool {
        self.edges.iter().all(|&(a, d)| d.perp_dot(p - a) >= 0.0)
    }
}

fn fill(raster: &mut Raster, poly: &[Vec2], color: [u8; 4]) {
    let shape = Convex::new(poly);
    raster.scan(
        shape.lo,
        shape.hi,
        |p| shape.contains(p),
        |px| px.copy_from_slice(&color),
    );
}

fn blend(dst: &mut [u8], src: [u8; 4]) {
    let a = u32::from(src[3]);
    for (d, s) in dst[..3].iter_mut().zip(src) {
        *d = ((u32::from(*d) * (255 - a) + u32::from(s) * a + 127) / 255) as u8;
    }
    dst[3] = 255;
}

/// Rasterizes `layout`: road everywhere, then each block's curb (sidewalk) and inner area (park or
/// block), building base footprints, and gang territories blended over their blocks.
pub fn rasterize(layout: &CityLayout, style: &RasterStyle) -> Raster {
    let g = layout.ground_size;
    let size = (g * style.px_per_m).ceil() as u32;
    let mut raster = Raster {
        width: size,
        height: size,
        origin: Vec2::splat(-g / 2.0),
        px_per_m: style.px_per_m,
        rgba: style.road.repeat(size as usize * size as usize),
    };
    for block in layout.blocks.iter().filter(|b| b.curb.len() >= 3) {
        fill(&mut raster, &block.curb, style.sidewalk);
        if block.inner.len() >= 3 {
            let color = if block.is_park {
                style.park
            } else {
                style.block
            };
            fill(&mut raster, &block.inner, color);
        }
    }
    for b in &layout.buildings {
        let (u, v, h) = (b.axis, b.axis.perp(), b.half_extents);
        let reach = Vec2::new(
            u.x.abs() * h.x + v.x.abs() * h.y,
            u.y.abs() * h.x + v.y.abs() * h.y,
        );
        let inside = |p: Vec2| {
            let d = p - b.center;
            d.dot(u).abs() <= h.x && d.dot(v).abs() <= h.y
        };
        raster.scan(b.center - reach, b.center + reach, inside, |px| {
            px.copy_from_slice(&style.building)
        });
    }
    for (gang, &district) in layout.gang_districts.iter().enumerate() {
        let tint = style.territory[gang];
        for block in layout
            .blocks
            .iter()
            .filter(|b| b.district == district && b.curb.len() >= 3)
        {
            let shape = Convex::new(&block.curb);
            raster.scan(
                shape.lo,
                shape.hi,
                |p| shape.contains(p),
                |px| blend(px, tint),
            );
        }
    }
    raster
}

/// `point` in the view of a camera at `center` with `yaw` (about +Y, forward = −Z at 0), metres:
/// x to the right of the view, y ahead.
pub fn project(point: Vec2, center: Vec2, yaw: f32) -> Vec2 {
    let d = point - center;
    let (s, c) = yaw.sin_cos();
    Vec2::new(d.x * c - d.y * s, -d.x * s - d.y * c)
}

/// UI pixel offset from the map centre (y down) of `point` on a map rotated with the camera.
pub fn map_px(point: Vec2, center: Vec2, yaw: f32, px_per_m: f32) -> Vec2 {
    let m = project(point, center, yaw);
    Vec2::new(m.x, -m.y) * px_per_m
}

/// Heading of a body facing `facing_yaw` on the map of a camera at `camera_yaw`, radians
/// counter-clockwise from map-up.
pub fn heading_on_map(facing_yaw: f32, camera_yaw: f32) -> f32 {
    facing_yaw - camera_yaw
}
