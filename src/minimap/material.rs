use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};

/// Length of the cop array in the shader (law, >= the escalation maximum of 12 units); the WGSL
/// array length in `shaders/minimap.wgsl` must equal it.
pub const MAX_CONES: usize = 16;

/// Per-frame minimap state; field order matches `MinimapUniform` in `shaders/minimap.wgsl`.
#[derive(ShaderType, Clone, Debug, Default)]
pub struct MinimapUniform {
    /// Player (x, z), the map centre.
    pub center: Vec2,
    /// (sin, cos) of the camera yaw.
    pub heading: Vec2,
    pub raster_origin: Vec2,
    /// Raster extent, m.
    pub raster_size: Vec2,
    pub view_radius: f32,
    /// Player arrow, radians counter-clockwise from map-up.
    pub arrow_heading: f32,
    pub rim_px: f32,
    pub arrow_px: f32,
    /// Search circle (x, z, radius, on 0/1).
    pub search: Vec4,
    /// (cone range m, cos of the half angle, cop count, search ring px).
    pub cone: Vec4,
    /// Linear RGBA colours.
    pub outside: Vec4,
    pub rim: Vec4,
    pub search_fill: Vec4,
    pub search_ring: Vec4,
    pub cone_color: Vec4,
    pub arrow_color: Vec4,
    /// Cops (x, z, forward x, forward z); the first `cone.z` are live.
    pub cops: [Vec4; MAX_CONES],
}

/// The whole round map (raster, search circle, cop cones, player arrow, rim) as one UI material:
/// the node stays unrotated, the shader rotates the lookup.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct MinimapMaterial {
    #[uniform(0)]
    pub data: MinimapUniform,
    #[texture(1)]
    #[sampler(2)]
    pub map: Handle<Image>,
}

impl UiMaterial for MinimapMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/minimap.wgsl".into()
    }
}
