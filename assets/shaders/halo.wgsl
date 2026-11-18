// A lamp's halo (`world::halos`): drawn on the near side of a sphere round
// the lamp, in front of the lamp's own bulb. How close the view ray
// passes to the lamp, as a share of the sphere's radius, falls out of the
// face's normal: t = sin(angle(normal, view)). A
// tight bright core, a soft wide halo that spreads more in damp air, and
// the fog takes it with distance.
//
// Light in fog (#110, as `render::fog_beam`'s beams): in wet air the lamp's
// light hangs round it as a wider aureole, drifting with the same slow
// billows fixed in the world, fading out well inside its sphere (no disc's
// edge). The sphere is drawn `air.z` times its size for it (the vertex
// stage), and the core and halo above are worked in its own size, so they
// are as they were.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
}

struct HaloParams {
    // rgb: the lamp's colour; w: its core's radiance.
    color: vec4<f32>,
    // x: how much the air spreads it; y: how far you see (m); z: how much
    // wider the aureole's sphere is than the halo's (`halos::spread`, 1 in
    // dry air).
    air: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> halo: HaloParams;

fn spread() -> f32 {
    return max(halo.air.z, 1.0);
}

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
        vec4<f32>(vertex.position * spread(), 1.0),
    );
    out.position = position_world_to_clip(out.world_position.xyz);
#endif
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index,
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
        vertex.instance_index,
        world_from_local[3],
    );
#endif
    return out;
}

// The beams' haze (`fog_beam.wgsl`'s): smooth value noise.
fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

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
    // (Of the aureole's sphere; `t2` of the halo's own, as it was.)
    let ta2 = clamp(1.0 - c * c, 0.0, 1.0);
    let s = spread();
    let t2 = min(ta2 * s * s, 1.0);
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
    // The aureole: the lamp's light in the wet air round it, the beams'
    // billows drifting through it (lumps a metre or two, fixed in the
    // world: no grain to shimmer), gone before the sphere's edge. Only in
    // wet air: none in dry.
    let wet = clamp((air - 0.2) / 0.6, 0.0, 1.0);
    let t = globals.time;
    let drift = vec3<f32>(t * 0.08, t * -0.02, t * 0.05);
    let n3 = noise3(in.world_position.xyz * 0.6 + drift);
    let billow = 0.4 + 1.2 * n3 * n3;
    let aureole = exp(-ta2 * 3.5) * (1.0 - smoothstep(0.45, 1.0, ta2)) * 0.05 * wet * billow;
    let k = (core * fog + glow * sqrt(fog)) * edge + aureole * sqrt(fog);
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
