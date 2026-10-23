// What an anomaly does to the ground under it: standard PBR, but fogged as
// if half as far off, as its particles are (`anomaly_particles.wgsl`): the
// Zone's things show through its murk, so a scorch or a scoured patch has
// an edge you can still see a few metres off in a pea-souper. (The base
// material's own fog is off, so it's fogged once, here.)

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{view, fog},
}
#ifdef DISTANCE_FOG
#import bevy_pbr::pbr_functions::apply_fog
#endif

// x: the share of its distance the fog takes it at.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> reach: vec4<f32>;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    var c = apply_pbr_lighting(pbr_input);
#ifdef DISTANCE_FOG
    let at = view.world_position + (in.world_position.xyz - view.world_position) * reach.x;
    c = apply_fog(fog, c, at, view.world_position, in.position.xy);
#endif
    out.color = main_pass_post_lighting_processing(pbr_input, c);
    return out;
}
