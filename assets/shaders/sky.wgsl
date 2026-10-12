// Overcast sky dome: a grey gradient with slow, heavy cloud structure and a
// faint bright patch where the sun is trying to get through.

#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::{view, globals}}

struct SkyParams {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    sun_dir: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> sky: SkyParams;

fn hash3(p: vec3<f32>) -> f32 {
    let q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.2, 0.3));
    let r = q * 17.0;
    return fract(r.x * r.y * r.z * (r.x + r.y + r.z));
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y),
        mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y),
        u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    var s = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i = i + 1) {
        s = s + a * noise3(q);
        q = q * 2.02;
        a = a * 0.5;
    }
    return s;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position.xyz - view.world_position);
    let up = clamp(dir.y, -1.0, 1.0);
    // Gradient: horizon band, darker overhead.
    let t = pow(clamp(up, 0.0, 1.0), 0.6);
    var col = mix(sky.horizon.rgb, sky.zenith.rgb, t);

    // Cloud base: project onto a plane overhead so structure shrinks toward
    // the horizon like a real ceiling, and drift it slowly.
    let h = max(up, 0.03);
    let plane = dir.xz / h * 0.6;
    let drift = vec2<f32>(globals.time * 0.004, globals.time * 0.0015);
    let c = fbm3(vec3<f32>(plane + drift, globals.time * 0.003));
    let c2 = fbm3(vec3<f32>(plane * 3.0 + drift * 2.0, 4.0));
    let cloud = smoothstep(0.35, 0.8, c * 0.75 + c2 * 0.35);
    // Darker cloud bellies, lighter gaps.
    col = col * mix(1.08, 0.78, cloud) ;

    // The sun, smeared through the deck (w: how much gets through).
    let s = max(dot(dir, normalize(sky.sun_dir.xyz)), 0.0);
    let glow = sky.sun_dir.w;
    col = col + vec3<f32>(0.35, 0.33, 0.28) * pow(s, 24.0) * (1.0 - cloud * 0.7) * 0.6 * glow;
    col = col + vec3<f32>(0.08, 0.075, 0.06) * pow(s, 4.0) * glow;

    // Toward and below the horizon the sky is fog.
    let fog_t = 1.0 - smoothstep(-0.02, 0.18, up);
    col = mix(col, sky.horizon.rgb, fog_t);
    return vec4<f32>(col, 1.0);
}
