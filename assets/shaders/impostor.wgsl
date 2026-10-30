// A far tree's foliage: the captured picture on one quad that turns to
// face the eye, standard
// PBR, alpha-tested (or alpha to coverage: `trees::foliage_alpha`). Within the near cut of the tree's base (carried in
// UV1; horizontal distance, as the cards are cut) the real cards are drawn
// instead, the two dithered against each other across a band
// (`impostors::LOD_BAND`).

#import bevy_pbr::{
    mesh_functions,
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> cut: vec4<f32>;

// bevy_pbr's mesh vertex shader, except that a tree within the near cut
// collapses to a point: its quads make no fragments at all (discarding
// them in the fragment stage shaded every one first; in a wood that was
// a third of the frame's fragments).
@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex.instance_index);
#endif
#ifdef VERTEX_POSITIONS
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );
#ifdef VERTEX_UVS_A
#ifdef VERTEX_UVS_B
    // One quad a tree, turned about the tree's axis (its base, UV1) to
    // face this eye (`impostors::face`); the normal tipped toward it.
    let base = vertex.uv_b;
    let half = length(out.world_position.xz - base);
    var to_eye = view.world_position.xz - base;
    to_eye = to_eye / max(length(to_eye), 1e-4);
    let right = vec2<f32>(to_eye.y, -to_eye.x);
    let p = base + right * (vertex.uv.x * 2.0 - 1.0) * half;
    out.world_position = vec4<f32>(p.x, out.world_position.y, p.y, 1.0);
#ifdef VERTEX_NORMALS
    out.world_normal = normalize(vec3<f32>(to_eye.x * 0.35, 0.65, to_eye.y * 0.35));
#endif
#endif
#endif
    out.position = position_world_to_clip(out.world_position.xyz);
#endif
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
    if (distance(view.world_position.xz, vertex.uv_b) < cut.x - cut.y) {
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index
    );
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif
    return out;
}

// A fixed pattern over the screen's pixels (interleaved gradient noise):
// the impostor's pixels where it's under the fade, the cards' the rest.
fn lod_dither(px: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(floor(px), vec2<f32>(0.06711056, 0.00583715))));
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
#ifdef VERTEX_UVS_B
    // Across the band, dithered in as the cards dither out (the same
    // pattern, the other side of it: `tree_sway.wgsl`).
    let f = smoothstep(cut.x - cut.y, cut.x + cut.y, distance(view.world_position.xz, in.uv_b));
    if (lod_dither(in.position.xy) >= f) {
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
