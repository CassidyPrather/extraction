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
    // x: deck heaviness (1 the reference overcast), y: scud amount,
    // z: the moon's lit face (0 not up; + waxing, - waning; sun_dir is
    // then toward it), w: how much night sky (stars).
    cloud: vec4<f32>,
    // A low sun's glow on its side of the horizon: rgb, strength in w.
    glow: vec4<f32>,
    // Lightning (`world::storm`): xyz toward the strike, w how bright
    // (against a day's horizon).
    flash: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> sky: SkyParams;

fn hash3(p: vec3<f32>) -> vec3<f32> {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yxz + 33.33);
    return fract((q.xxy + q.yzz) * q.zyx);
}

// Stars: one chance per cell of a grid on the sky. `density` (0..1) is the
// chance a cell holds one. Each is a soft point a little over a pixel wide
// in a headset (a hard one would shimmer), brighter ones rarer.
fn stars(dir: vec3<f32>, cells: f32, density: f32, time: f32, twinkle: f32) -> f32 {
    let p = dir * cells;
    let c = floor(p);
    let h = hash3(c);
    if (h.x > density) {
        return 0.0;
    }
    let centre = c + 0.25 + 0.5 * hash3(c + 17.0);
    let d = length(p - centre);
    let size = 0.11 + 0.06 * h.y;
    let core = 1.0 - smoothstep(0.0, size, d);
    // Rare bright ones; most are faint.
    let mag = 0.12 + 0.88 * pow(h.z, 6.0);
    // Low down, seen through more air, they flicker.
    let flick = 1.0 - twinkle * 0.7 * (0.5 + 0.5 * sin(time * (7.0 + 9.0 * h.y) + h.z * 40.0));
    return core * mag * flick;
}

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
    if (sky.glow.w > 0.0 && sky.cloud.w <= 0.0) {
        let sun_h = normalize(sky.sun_dir.xz + vec2<f32>(1e-4, 0.0));
        let dir_h = normalize(dir.xz + vec2<f32>(1e-4, 0.0));
        let side = pow(max(dot(dir_h, sun_h), 0.0), 2.0);
        let low = 1.0 - smoothstep(0.0, 0.45, up);
        let g = side * low * sky.glow.w * (0.55 + 0.45 * (1.0 - belly));
        col = mix(col, col * sky.glow.rgb * 1.9, clamp(g, 0.0, 1.0));
    }

    // Night: the stars and the Milky Way where the deck breaks, and the
    // moon, its phase, and the cloud's edges round it lit silver.
    let night = sky.cloud.w;
    if (night > 0.0) {
        // How much open sky there is here: the gaps, not the bellies; none
        // at all under a full overcast.
        let open = (1.0 - belly) * (1.0 - scud * 0.8 * sky.cloud.y)
            * (1.0 - smoothstep(0.55, 1.05, heavy)) * detail;
        // The Milky Way: a band tilted across the sky, lumpy and dusty.
        let pole = normalize(vec3<f32>(0.42, 0.55, -0.72));
        let band_d = dot(dir, pole);
        let band = exp(-band_d * band_d * 22.0);
        let lumps = fbm2(dir.xz * 3.0 / max(abs(dir.y) + 0.35, 0.35) + vec2<f32>(dir.y * 4.0, 0.0), 1.0);
        let dust = smoothstep(0.35, 0.75, lumps);
        let milk = band * (0.35 + 0.65 * dust) * (1.0 - 0.6 * smoothstep(0.45, 0.6, fbm2(dir.xz * 9.0 + 4.0, 1.0)));
        let low = 1.0 - smoothstep(0.02, 0.35, up);
        let star = stars(dir, 140.0, 0.16 + 0.3 * band, time, low)
            + stars(dir, 70.0, 0.05, time, low) * 1.6;
        // The moon washes the faint ones out.
        let moon_lit = abs(sky.cloud.z);
        // How much of the moon's light gets through to the cloud and air
        // (none under murk: an overcast night is black).
        let moon_air = sky.glow.w;
        // Unlit by a moon, the cloud is only what the valley's sky lights.
        col = col * mix(0.65, 1.0, moon_air) ;
        // The gaps are space: darker than the cloud's undersides, which
        // catch what light the valley and the moon give them.
        col = mix(col, zenith * 0.5, open * detail * night * (1.0 - 0.45 * moon_lit));
        let wash = 1.0 - 0.6 * moon_lit;
        let fade = smoothstep(0.0, 0.12, up) * open * night * wash;
        col = col + vec3<f32>(0.75, 0.82, 1.0) * star * 0.7 * fade;
        col = col + vec3<f32>(0.55, 0.6, 0.75) * milk * 0.03 * fade;

        if (moon_lit > 0.0) {
            let m = normalize(sky.sun_dir.xyz);
            let cos_r = dot(dir, m);
            // Its disc: half a degree is a dot in a headset; drawn twice
            // that, the face can be read.
            let radius = 0.021;
            let r = sqrt(max(2.0 * (1.0 - cos_r), 0.0)) / radius;
            // Its face, a frame on the disc: x across toward the west
            // (where the sun went), y up.
            let east = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), m) + vec3<f32>(1e-4, 0.0, 0.0));
            let north = cross(m, east);
            let fx = dot(dir - m * cos_r, east) / radius;
            let fy = dot(dir - m * cos_r, north) / radius;
            let fz = sqrt(max(1.0 - fx * fx - fy * fy, 0.0));
            // The sun lights it from its own side: waxing from the west
            // (-x), waning from the east; the face at the phase angle.
            let a = acos(clamp(1.0 - 2.0 * moon_lit, -1.0, 1.0));
            let side = select(1.0, -1.0, sky.cloud.z > 0.0);
            let sunlit = smoothstep(-0.04, 0.04, fx * side * sin(a) + fz * cos(a));
            let maria = 0.78 + 0.22 * smoothstep(0.4, 0.65, fbm2(vec2<f32>(fx, fy) * 2.2 + 3.0, 1.0));
            let disc = 1.0 - smoothstep(0.9, 1.05, r);
            // Earthshine on the dark side, just there.
            let face = (sunlit * maria + 0.02) * disc;
            // Through the cloud: clear in a gap, a blurred smear behind a
            // belly, gone under a thick deck.
            let thick = smoothstep(0.75, 1.3, heavy);
            let through = mix(1.0, 0.15, belly) * (1.0 - thick)
                * clamp(moon_air / max(moon_lit, 0.05), 0.0, 1.0);
            col = mix(col, vec3<f32>(0.92, 0.95, 1.0) * 3.0, face * through);
            // The halo in the air round it, and the cloud's edges near it
            // lit: the thin cloud between belly and seam catches it most.
            let edge = belly * (1.0 - belly) * 4.0;
            let near = pow(max(cos_r, 0.0), 60.0);
            let wide = pow(max(cos_r, 0.0), 6.0);
            let light = moon_air;
            col = col + vec3<f32>(0.6, 0.68, 0.85) * light * (0.05 * near + 0.012 * wide);
            col = col + vec3<f32>(0.7, 0.78, 0.95) * light * edge * (0.18 * near + 0.05 * wide) * detail;
        }
    }

    // Lightning: the deck lit from inside, the bellies most, brightest
    // round the strike and the whole sky a little.
    if (sky.flash.w > 0.0) {
        let c = max(dot(dir, normalize(sky.flash.xyz)), 0.0);
        // (As bright as the ground it lights: a day's horizon, several
        // times over; less where the deck is thin.)
        // (Scaling the deck keeps its shape: the bellies, thickest, glow
        // most. Added flat, it went to a featureless sheet.)
        let lit = sky.flash.w * (0.45 + 1.2 * pow(c, 6.0) + 3.0 * pow(c, 48.0));
        col = col * (1.0 + lit * (0.5 + 2.2 * belly) * mix(0.5, 1.0, detail))
            + vec3<f32>(0.72, 0.78, 1.0) * 0.06 * lit;
    }

    // Toward and below the horizon the sky is fog.
    let fog_t = 1.0 - smoothstep(-0.02, 0.14, up);
    // In a flash the fog is lit as `world::storm` lights the fog itself
    // (else the fogged tree line stood out pale against a darker band).
    let air = vec3<f32>(0.72, 0.78, 1.0) * 0.216 * max(sky.flash.w, 0.0);
    col = mix(col, horizon + air, fog_t);
    return vec4<f32>(col, 1.0);
}
