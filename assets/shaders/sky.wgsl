// Overcast sky dome: a low stratocumulus deck, heavy darker bellies with
// paler seams between them, a faster ragged scud layer under it, a
// brighter band along the horizon where far-off gaps let light through,
// and a faint smear where the sun is trying to break through.
//
// Cost: 2D value noise only (the deck is a plane overhead), eight octaves
// in all, on the sky pixels nothing else covers. Detail fades out toward
// the horizon, where the projected plane's frequency would otherwise run
// past a pixel and crawl in a headset.

#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::{view, globals}}

struct SkyParams {
    horizon: vec4<f32>,
    zenith: vec4<f32>,
    sun_dir: vec4<f32>,
    // x: deck heaviness (1 the reference overcast), y: scud amount.
    cloud: vec4<f32>,
    // A low sun's glow on its side of the horizon: rgb, strength in w.
    glow: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> sky: SkyParams;

fn hash2(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2(i), hash2(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y);
}

// `detail` (0..1) scales the upper octaves away toward the horizon.
fn fbm2(p: vec2<f32>, detail: f32) -> f32 {
    var s = 0.0;
    var a = 0.5;
    var q = p;
    let rot = mat2x2<f32>(0.8, 0.6, -0.6, 0.8);
    for (var i = 0; i < 5; i = i + 1) {
        let w = select(1.0, detail, i >= 2);
        s = s + a * w * (noise2(q) - 0.5);
        q = rot * q * 2.03 + vec2<f32>(13.1, 7.7);
        a = a * 0.5;
    }
    return s + 0.5;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let dir = normalize(in.world_position.xyz - view.world_position);
    let up = clamp(dir.y, -1.0, 1.0);
    let horizon = sky.horizon.rgb;
    let zenith = sky.zenith.rgb;

    // Gradient: a lighter band low down, darker overhead.
    let t = pow(clamp(up, 0.0, 1.0), 0.5);
    var col = mix(horizon * 1.04, zenith, t);

    // The deck: a plane overhead, so cells shrink toward the horizon.
    let h = max(up, 0.04);
    let plane = dir.xz / h;
    let detail = smoothstep(0.03, 0.22, up);
    let time = globals.time;
    let drift = vec2<f32>(time * 0.0035, time * 0.0012);
    // Domain warp: billows, not blobs.
    let warp = vec2<f32>(
        noise2(plane * 0.35 + drift * 0.5 + vec2<f32>(3.0, 1.0)),
        noise2(plane * 0.35 + drift * 0.5 + vec2<f32>(8.0, 5.0)),
    ) - 0.5;
    let n = fbm2(plane * 1.1 + drift + warp * 1.2, detail);
    // Overcast: mostly cloud, the seams are where it thins.
    // Heavier weather closes the seams and darkens the bellies; a clearing
    // breaks the deck up into paler cloud with wide gaps.
    let heavy = sky.cloud.x;
    let shift = (heavy - 1.0) * 0.3;
    let belly = smoothstep(0.42 - shift, 0.66 - shift, n);
    let seam = 1.0 - smoothstep(0.3 - shift, 0.48 - shift, n);
    let dark = clamp(0.7 - (heavy - 1.0) * 0.25, 0.5, 0.92);
    col = col * mix(1.06, dark, belly * mix(0.5, 1.0, detail));
    col = col + horizon * 0.2 * seam * detail * clamp(2.0 - heavy, 0.3, 1.6);

    // Scud: ragged low cloud, darker, moving faster and the other way.
    let scud_p = plane * 2.2 + vec2<f32>(-time * 0.011, time * 0.006);
    let scud = smoothstep(0.55, 0.8, fbm2(scud_p, detail * detail));
    col = mix(col, zenith * 0.78, clamp(scud * 0.4 * sky.cloud.y, 0.0, 0.8) * detail);

    // The sun, smeared through the deck (w: how much gets through).
    let s = max(dot(dir, normalize(sky.sun_dir.xyz)), 0.0);
    let glow = sky.sun_dir.w;
    let thin = 1.0 - belly * 0.7;
    col = col + vec3<f32>(0.30, 0.28, 0.24) * pow(s, 24.0) * thin * 0.6 * glow;
    col = col + vec3<f32>(0.08, 0.075, 0.06) * pow(s, 4.0) * glow;

    // A low sun lights the horizon on its side, and the cloud's
    // underside there, warm; the seams take the most of it.
    if (sky.glow.w > 0.0) {
        let sun_h = normalize(sky.sun_dir.xz + vec2<f32>(1e-4, 0.0));
        let dir_h = normalize(dir.xz + vec2<f32>(1e-4, 0.0));
        let side = pow(max(dot(dir_h, sun_h), 0.0), 2.0);
        let low = 1.0 - smoothstep(0.0, 0.45, up);
        let g = side * low * sky.glow.w * (0.55 + 0.45 * (1.0 - belly));
        col = mix(col, col * sky.glow.rgb * 1.9, clamp(g, 0.0, 1.0));
    }

    // Toward and below the horizon the sky is fog.
    let fog_t = 1.0 - smoothstep(-0.02, 0.14, up);
    col = mix(col, horizon, fog_t);
    return vec4<f32>(col, 1.0);
}
