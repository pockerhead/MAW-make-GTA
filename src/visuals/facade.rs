use super::RenderConfig;
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};

const FACADE_SHADER: &str = "shaders/facade.wgsl";

/// Vertex-coloured city material; UV >= 0 marks facade walls where the shader draws windows.
pub type FacadeMaterial = ExtendedMaterial<StandardMaterial, FacadeExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FacadeExtension {
    #[uniform(100)]
    pub params: FacadeParams,
}

#[derive(ShaderType, Reflect, Debug, Clone)]
pub struct FacadeParams {
    /// Linear glass colour, w = 1.
    pub glass: Vec4,
    /// Window width, height, sill (fractions of a cell) and glass roughness.
    pub window: Vec4,
}

impl MaterialExtension for FacadeExtension {
    fn fragment_shader() -> ShaderRef {
        FACADE_SHADER.into()
    }
}

pub fn facade_material(config: &RenderConfig) -> FacadeMaterial {
    let f = &config.facade;
    let (r, g, b) = f.glass_color;
    let glass = Color::srgb(r, g, b).to_linear();
    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: f.wall_roughness,
            ..default()
        },
        extension: FacadeExtension {
            params: FacadeParams {
                glass: Vec4::new(glass.red, glass.green, glass.blue, 1.0),
                window: Vec4::new(
                    f.window_width,
                    f.window_height,
                    f.window_sill,
                    f.glass_roughness,
                ),
            },
        },
    }
}
