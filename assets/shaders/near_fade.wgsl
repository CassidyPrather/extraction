// Standard PBR that dissolves (dithered) close to the camera: the merged
// far tree crowns, which give way to detailed near trees.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::view,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> fade: vec4<f32>;

fn bayer4(p: vec2<u32>) -> f32 {
    let m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    return (m[(p.y % 4u) * 4u + (p.x % 4u)] + 0.5) / 16.0;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let d = length(in.world_position.xyz - view.world_position);
    let t = clamp((d - fade.x) / max(fade.y - fade.x, 0.001), 0.0, 1.0);
    if (t < bayer4(vec2<u32>(in.position.xy))) {
        discard;
    }
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
