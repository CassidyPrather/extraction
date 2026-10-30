// A lightning bolt (`world::storm`): its channel as a ribbon facing the
// eye, built per strike on the CPU (a few dozen segments). Additive: a
// hot white core with a blue-violet glow round it, only for the flash's
// frames, and fading with distance into the rain.
// No fog: additive; it fades with distance into the rain itself.

#import bevy_pbr::{
    mesh_view_bindings::view,
    view_transformations::position_world_to_clip,
}

struct BoltParams {
    // rgb: colour; w: how bright now (the flash, and how far off it is).
    color: vec4<f32>,
    // x: half width of the glow (m).
    size: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> bolt: BoltParams;

struct Vertex {
    @location(0) position: vec3<f32>,
    // The segment's direction (unit): the ribbon turns about it.
    @location(1) axis: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let to_eye = normalize(view.world_position.xyz - v.position);
    var side = cross(v.axis, to_eye);
    if (length(side) < 1e-4) {
        side = vec3<f32>(1.0, 0.0, 0.0);
    }
    // uv.y: 1 on the main channel, less on a fork (thinner).
    let w = bolt.size.x * mix(0.5, 1.0, v.uv.y);
    let p = v.position + normalize(side) * (v.uv.x - 0.5) * 2.0 * w;
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(p);
    out.uv = v.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let x = (in.uv.x - 0.5) * 2.0;
    let core = exp(-x * x * 30.0);
    let glow = exp(-x * x * 3.0) * 0.6;
    let c = mix(bolt.color.rgb * vec3<f32>(0.7, 0.75, 1.0), vec3<f32>(1.0), core);
    // Alpha 0: Bevy's additive blend is premultiplied (src + dst * (1 - a)).
    return vec4<f32>(c * (core * 6.0 + glow) * bolt.color.w * mix(0.6, 1.0, in.uv.y), 0.0);
}
