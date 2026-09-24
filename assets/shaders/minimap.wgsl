// Round minimap (GDD §7): raster lookup rotated with the camera, search circle, cop view cones,
// player arrow and rim, drawn over one unrotated UI node.
//
// Spaces:
//   p  node space: centre (0, 0), rim at length 1, y DOWN (UI uv origin is top-left).
//   m  metres in the camera view: x right, y AHEAD (= up on the map).
//   w  world (x, z); forward of yaw 0 is -Z.
// The projection is citygen::minimap::project (m from w); here it is inverted (w from m).

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct MinimapUniform {
    center: vec2<f32>,
    heading: vec2<f32>,
    raster_origin: vec2<f32>,
    raster_size: vec2<f32>,
    view_radius: f32,
    arrow_heading: f32,
    rim_px: f32,
    arrow_px: f32,
    search: vec4<f32>,
    cone: vec4<f32>,
    outside: vec4<f32>,
    rim: vec4<f32>,
    search_fill: vec4<f32>,
    search_ring: vec4<f32>,
    cone_color: vec4<f32>,
    arrow_color: vec4<f32>,
    // Length must equal MAX_CONES in src/minimap/material.rs.
    cops: array<vec4<f32>, 16>,
}

@group(1) @binding(0) var<uniform> data: MinimapUniform;
@group(1) @binding(1) var map: texture_2d<f32>;
@group(1) @binding(2) var map_sampler: sampler;

fn over(dst: vec4<f32>, src: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(mix(dst.rgb, src.rgb, src.a), max(dst.a, src.a));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = (in.uv - 0.5) * 2.0;
    let r = length(p);
    if r > 1.0 {
        return vec4<f32>(0.0);
    }
    let half_px = in.size.x * 0.5;
    let m_per_px = data.view_radius / half_px;
    let m = vec2<f32>(p.x, -p.y) * data.view_radius;
    let s = data.heading.x;
    let c = data.heading.y;
    let w = data.center + vec2<f32>(c, -s) * m.x + vec2<f32>(-s, -c) * m.y;

    // Sampled before any branch on data: explicit level, no derivatives.
    let uv = (w - data.raster_origin) / data.raster_size;
    let texel = textureSampleLevel(map, map_sampler, uv, 0.0);
    var color = data.outside;
    if all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)) {
        color = texel;
    }

    if data.search.w > 0.5 {
        let d = distance(w, data.search.xy);
        if d < data.search.z {
            color = over(color, data.search_fill);
        }
        if abs(d - data.search.z) < 0.5 * data.cone.w * m_per_px {
            color = over(color, data.search_ring);
        }
    }

    let count = u32(data.cone.z);
    for (var i = 0u; i < count; i++) {
        let cop = data.cops[i];
        let v = w - cop.xy;
        let d = length(v);
        if d > 0.001 && d < data.cone.x && dot(v / d, cop.zw) >= data.cone.y {
            color = over(color, data.cone_color);
        }
    }

    // Arrow in m space scaled to px: apex along (-sin h, cos h), y up.
    let q = vec2<f32>(p.x, -p.y) * half_px;
    let h = data.arrow_heading;
    let dir = vec2<f32>(-sin(h), cos(h));
    let side = vec2<f32>(dir.y, -dir.x);
    let a = dot(q, dir);
    let b = dot(q, side);
    let len = data.arrow_px;
    if a >= -0.4 * len && abs(b) <= 0.35 * (0.6 * len - a) {
        color = over(color, data.arrow_color);
    }

    if r > 1.0 - data.rim_px / half_px {
        color = over(color, data.rim);
    }
    return color;
}
