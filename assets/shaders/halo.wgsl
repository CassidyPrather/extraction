// A lamp's halo (`world::halos`): drawn on the near side of a sphere round
// the lamp, in front of the lamp's own bulb. How close the view ray
// passes to the lamp, as a share of the sphere's radius, falls out of the
// face's normal: t = sin(angle(normal, view)). A
// tight bright core, a soft wide halo that spreads more in damp air, and
// the fog takes it with distance.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::view,
}

struct HaloParams {
    // rgb: the lamp's colour; w: its core's radiance.
    color: vec4<f32>,
    // x: how much the air spreads it; y: how far you see (m).
    air: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> halo: HaloParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let to_eye = view.world_position - in.world_position.xyz;
    let dist = length(to_eye);
    let v = to_eye / max(dist, 1e-3);
    let n = normalize(in.world_normal);
    let c = dot(n, v);
    let t2 = clamp(1.0 - c * c, 0.0, 1.0);
    let air = halo.air.x;
    let core = exp(-t2 * 90.0);
    let glow = exp(-t2 * mix(9.0, 4.0, clamp(air, 0.0, 1.0))) * (0.08 + 0.15 * air);
    // The edge of the sphere: nothing left.
    let edge = 1.0 - smoothstep(0.75, 1.0, t2);
    // Through fog, the core goes first and the halo stays longest.
    let fog = exp(-dist / max(halo.air.y * 0.35, 1.0));
    let k = (core * fog + glow * sqrt(fog)) * edge;
    return vec4<f32>(halo.color.rgb * halo.color.w * k * view.exposure, 0.0);
}
