// Light shafts: the sun through a window into a dark room, or through a
// gap in the trees at dawn and dusk, as a few crossed additive cards along
// the light (`world::shafts`). No raymarch: each card is soft at its edges,
// fades out as it turns edge-on (so the cards never show as planes), is
// brightest looking toward the sun (light scatters forward in haze), and
// thins with distance.
//
// uv.x runs across a card (0..1), uv.y along the light (0 where it comes
// in, 1 where it ends).

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
}

struct ShaftParams {
    // rgb: the sun's colour; w: how strong the shafts are (0: off).
    color: vec4<f32>,
    // xyz: the way the light travels; w: how far they reach (m).
    sun: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> shaft: ShaftParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let to_eye = view.world_position - in.world_position.xyz;
    let dist = length(to_eye);
    let v = to_eye / max(dist, 1e-3);
    // Soft across, fading in where the light enters and out along it.
    let across = in.uv.x;
    let along = in.uv.y;
    let edge = smoothstep(0.0, 0.35, across) * smoothstep(1.0, 0.65, across);
    let length_fade = smoothstep(0.0, 0.08, along) * pow(1.0 - along, 1.5);
    // Edge-on, a card is a line: gone before it gets there.
    let facing = smoothstep(0.15, 0.55, abs(dot(v, normalize(in.world_normal))));
    // Looking toward the sun (against the light's travel), brighter.
    let toward = max(dot(-v, -shaft.sun.xyz), 0.0);
    let forward = 0.35 + 0.65 * pow(toward, 3.0);
    let far = 1.0 - smoothstep(shaft.sun.w * 0.6, shaft.sun.w, dist);
    // Close to, a card would fill the view: fade it as you walk into it.
    let near = smoothstep(0.6, 2.5, dist);
    let k = edge * length_fade * facing * forward * far * near * shaft.color.w;
    // Alpha 0: the blend is premultiplied, so this adds.
    return vec4<f32>(shaft.color.rgb * k * view.exposure, 0.0);
}
