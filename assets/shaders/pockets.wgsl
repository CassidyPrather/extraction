// Pocket spheres (`render::pockets`, `interact::pockets`): one material
// for all of them and their icons, each one's look in its `MeshTag`, so
// they draw together. The tag: bits 0-7 alpha, 8-15 grey, bit 16 an icon
// (flat, no rim, in its parts' own colours).
//
// A sphere is a soap-bubble: faint in its middle, stronger at its rim,
// so it reads as a round thing in the air and doesn't fog what's in it.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_functions,
    mesh_view_bindings::view,
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let tag = mesh_functions::get_tag(in.instance_index);
    let alpha = f32(tag & 0xffu) / 255.0;
    let grey = f32((tag >> 8u) & 0xffu) / 255.0;
    let icon = ((tag >> 16u) & 1u) == 1u;
    let color = vec3<f32>(grey, grey, min(grey * 1.03, 1.0));
    if icon {
        // In its own colours, a little dimmed (`render::pockets`): a white
        // copy read as a bug (playtest 2026-11-22).
#ifdef VERTEX_COLORS
        return vec4<f32>(in.color.rgb * 0.8, alpha);
#else
        return vec4<f32>(color, alpha);
#endif
    }
    let v = normalize(view.world_position - in.world_position.xyz);
    let n = normalize(in.world_normal);
    let facing = abs(dot(n, v));
    let rim = pow(1.0 - facing, 2.0);
    return vec4<f32>(color, clamp(alpha * (0.55 + 1.2 * rim), 0.0, 1.0));
}
