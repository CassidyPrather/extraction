// The glass eye's view of robots: a hot rim on a faint fill, drawn through
// everything (the pipeline turns the depth test off).
// No fog: a sight through walls (the glass eye's), not part of the air.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, globals},
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> color: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let v = normalize(view.world_position.xyz - in.world_position.xyz);
    let n = normalize(in.world_normal);
    let rim = pow(1.0 - abs(dot(n, v)), 2.0);
    let pulse = 0.85 + 0.15 * sin(globals.time * 6.0);
    let c = color.rgb * (0.15 + 1.6 * rim) * pulse;
    return vec4<f32>(c, 1.0);
}
