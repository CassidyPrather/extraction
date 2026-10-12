// Water: animated ripple normals, a fresnel blend toward the sky colour,
// rain-ring-ish fine detail. Forward only (it's transparent).

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{view, globals},
}

struct WaterParams {
    sky: vec4<f32>,
    flow: vec2<f32>,
    ripple: f32,
    _pad: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> water: WaterParams;

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2<f32>(1.0, 0.0)), u.x),
               mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}

fn height(p: vec2<f32>, t: f32) -> f32 {
    let q = p - water.flow * t;
    return vnoise(q * 1.7) * 0.5 + vnoise(q * 4.3 + vec2<f32>(t * 0.4, 0.0)) * 0.3
        + vnoise(p * 11.0 + vec2<f32>(0.0, t * 1.3)) * 0.15;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let p = in.world_position.xz;
    let t = globals.time;
    let e = 0.05;
    let h0 = height(p, t);
    let hx = height(p + vec2<f32>(e, 0.0), t) - h0;
    let hz = height(p + vec2<f32>(0.0, e), t) - h0;
    let strength = 0.6 * water.ripple;
    let n = normalize(vec3<f32>(-hx / e * strength * 0.05, 1.0, -hz / e * strength * 0.05));
    pbr_input.N = n;

    let v = normalize(view.world_position - in.world_position.xyz);
    let fresnel = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 4.0);

    var out: FragmentOutput;
    var lit = apply_pbr_lighting(pbr_input);
    // Fake sky reflection: the overcast is the only thing up there.
    let refl = water.sky.rgb * (0.6 + 0.4 * (1.0 - h0));
    lit = vec4<f32>(mix(lit.rgb, refl, fresnel * 0.85), mix(pbr_input.material.base_color.a, 1.0, fresnel));
    out.color = main_pass_post_lighting_processing(pbr_input, lit);
    return out;
}
