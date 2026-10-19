// A far tree's foliage: the captured picture on crossed quads, standard
// PBR, alpha to coverage. Within the near cut of the tree's base (carried in
// UV1; horizontal distance, as the cards are cut) the real cards are drawn
// instead: the whole tree swaps at once.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::view,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> cut: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
#ifdef VERTEX_UVS_B
    if (distance(view.world_position.xz, in.uv_b) < cut.x) {
        discard;
    }
#endif
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
