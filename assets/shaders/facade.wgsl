// City material: StandardMaterial with vertex colours; facade walls (uv >= 0) get a window grid,
// one cell per bay (u) and floor (v).
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct FacadeParams {
    glass: vec4<f32>,
    // width, height, sill (cell fractions), glass roughness
    window: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> facade: FacadeParams;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef VERTEX_UVS_A
    if (in.uv.x >= 0.0) {
        let cell = fract(in.uv);
        let inside = abs(cell.x - 0.5) < 0.5 * facade.window.x
            && cell.y > facade.window.z && cell.y < facade.window.z + facade.window.y;
        if (inside) {
            pbr_input.material.base_color = vec4<f32>(facade.glass.rgb, 1.0);
            pbr_input.material.perceptual_roughness = facade.window.w;
        }
    }
#endif

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
