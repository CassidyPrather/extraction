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

// The most a halo adds as the tonemapper gets it and keeps its colour: in
// its brightest channel, and to the eye (a green, nearly all luminance,
// whitens at a third of a red's level). `render::grade::LAMP_CHANNEL_CAP`
// and `LAMP_LUMA_CAP` hold the same two.
const HUE_CAP: f32 = 1.0;
const LUMA_CAP: f32 = 0.3;

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
    // Through fog, the core goes first and the halo stays longest. The
    // scene's own fog (`FogFalloff::from_visibility_squared`: 5% left at
    // the visibility), so a lamp carries as far as what it stands on.
    let far = dist / max(halo.air.y, 1.0);
    let fog = exp(-3.0 * far * far);
    let k = (core * fog + glow * sqrt(fog)) * edge;
    let rad = halo.color.rgb * halo.color.w * k * view.exposure;
    // Its brightest channel eased toward HUE_CAP and its luminance toward
    // LUMA_CAP, all channels together (hue kept): past those the
    // tonemapper (AgX) runs a red or green core to white, and a red
    // landmark and a green way out read the same. Eased as it will reach
    // the tonemapper: the grade's exposure (the eyes' adaptation, up to
    // three stops at night) comes after.
    let gain = exp2(view.color_grading.exposure);
    let m = max(rad.r, max(rad.g, rad.b)) * gain;
    let y = dot(rad, vec3<f32>(0.2126, 0.7152, 0.0722)) * gain;
    let by_m = (m / (1.0 + m / HUE_CAP)) / max(m, 1e-6);
    let by_y = (y / (1.0 + y / LUMA_CAP)) / max(y, 1e-6);
    return vec4<f32>(rad * min(by_m, by_y), 0.0);
}
