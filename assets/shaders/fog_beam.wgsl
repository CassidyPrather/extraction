// Light in fog (`render::fog_beam`): a lamp's beam as light scattered in
// hazy air, not a solid cone. One shared look for every lit beam (the
// searchlights first).
//
// The cone mesh only bounds the volume. Each pixel it covers marches a
// short stretch of the view ray through the cone (single scattering,
// MARCH samples), and adds what the air there scatters toward the eye:
//
// - dense near the lamp and thinning with distance (the light spreads);
// - soft toward the cone's wall, not a hard edge;
// - fading out softly where the beam meets the ground it lands on, not a
//   line where two polygons cross;
// - dust drifting slowly through it, as in a dusty room's sunbeam;
// - brighter looking up the beam toward the lamp (forward scattering);
// - as dense as the air is: the world's fog and weather (`air`).
//
// One face a pixel: seen from outside, the front faces draw it; from
// inside (FOG_BEAM_INSIDE, picked on the CPU) the back faces, and the
// march starts at the eye. 3 to 12 samples, by how much beam the ray
// crosses: a ray grazing the edge needs few. Near the eye
// it's capped: a beam over you lights the air round you, it doesn't white
// out the view.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, globals},
}

struct FogBeam {
    // rgb: the light; w: its strength (0 off).
    light: vec4<f32>,
    // xyz: the lamp (world); w: tan of the half-angle.
    apex: vec4<f32>,
    // xyz: the way it throws (world, unit); w: how far it goes (m).
    axis: vec4<f32>,
    // x: how dense the air is (0.2 clear .. 1.2 thick fog); y: daylight
    // 0..1; z: the ground's height where it lands (world y); w: unused.
    air: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> b: FogBeam;

const MARCH: i32 = 12;
// Metres of beam a sample covers before another is added.
const STEP: f32 = 1.5;

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

// Smooth value noise: the haze's slow billows.
fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n000 = hash3(i);
    let n100 = hash3(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash3(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash3(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash3(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash3(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash3(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash3(i + vec3<f32>(1.0, 1.0, 1.0));
    let x00 = mix(n000, n100, u.x);
    let x10 = mix(n010, n110, u.x);
    let x01 = mix(n001, n101, u.x);
    let x11 = mix(n011, n111, u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// How much light the air scatters at `p` (before the phase).
fn density(p: vec3<f32>) -> f32 {
    let rel = p - b.apex.xyz;
    let along = dot(rel, b.axis.xyz);
    let len = b.axis.w;
    if (along <= 0.0 || along > len + 2.0) {
        return 0.0;
    }
    let off = length(rel - b.axis.xyz * along);
    let edge = off / max(along * b.apex.w, 0.05);
    // Soft to the wall: most light down the middle.
    let soft = 1.0 - smoothstep(0.35, 1.0, edge);
    if (soft <= 0.0) {
        return 0.0;
    }
    // The light spreads: bright at the lamp, thinning down the throw (a
    // tight beam holds more than a torch's); a dense core near the lamp,
    // more of it by day, so it reads against a lit sky.
    let spread = 1.0 / (1.0 + along * along * 0.0012);
    let core = 1.0 + (1.2 + 2.0 * b.air.y) * exp(-along / 12.0);
    let start = smoothstep(0.0, 1.5, along);
    // Where it lands: a soft fade into the ground, and past its end.
    let ground = smoothstep(0.0, 1.2, p.y - b.air.z);
    let end = 1.0 - smoothstep(len - 1.0, len + 2.0, along);
    // The air: large billows drifting slowly, fixed in the world. (No
    // fine grain: in a headset it shimmers between the eyes and frames.)
    let t = globals.time;
    let drift = vec3<f32>(t * 0.08, t * -0.02, t * 0.05);
    let billow = noise3(p * 0.09 + drift);
    let air = 0.2 + 1.2 * billow * billow;
    return soft * spread * core * start * ground * end * air;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if (b.light.w <= 0.0) {
        discard;
    }
    let eye = view.world_position;
    let at = in.world_position.xyz;
    let to = at - eye;
    let dist = length(to);
    let rd = to / max(dist, 1e-4);
#ifdef FOG_BEAM_INSIDE
    let inside = true;
#else
    let inside = false;
#endif
    // The stretch of the ray through the beam: from where it enters (or
    // the eye) on through as wide as the cone is here, crossing at a slant.
    var t0 = dist;
    if (inside) {
        t0 = 0.0;
    }
    let p0 = eye + rd * t0;
    let along0 = max(dot(p0 - b.apex.xyz, b.axis.xyz), 0.5);
    let width = 2.0 * along0 * b.apex.w + 2.0;
    let slant = sqrt(max(1.0 - pow(dot(rd, b.axis.xyz), 2.0), 0.02));
    var span = min(width / slant, b.axis.w + 4.0);
    if (inside) {
        span = dist;
    }
    let n = clamp(i32(ceil(span / STEP)), 3, MARCH);
    let ds = span / f32(n);
    // A per-pixel jitter on where the samples fall (no banding), fixed in
    // the world: it doesn't crawl from frame to frame.
    let jit = hash3(floor(at * 7.0));
    var sum = 0.0;
    for (var i = 0; i < n; i = i + 1) {
        let p = p0 + rd * (ds * (f32(i) + jit));
        sum = sum + density(p) * ds;
    }
    // Forward scattering: looking up the beam toward the lamp, brighter.
    let toward = max(dot(-rd, b.axis.xyz), 0.0);
    let phase = 0.55 + 1.6 * pow(toward, 6.0);
    let k = sum * phase * b.air.x * b.light.w;
    // Near the eye, capped: light round you, not a white-out.
    let near = select(dist, 0.0, inside);
    let cap = mix(0.18, 1.0, smoothstep(2.0, 20.0, near));
    let day = b.air.y;
    // (By day it must still show over a lit field: more of the haze.)
    // (At night it's the brightest thing in view: a lower ceiling, so the
    // billows show in it and it doesn't read as a solid cone.)
    let a = min(1.0 - exp(-k * mix(0.07, 0.16, day)), cap * mix(0.32, 0.5, day));
    let glow = mix(1.7, 1.5, day);
    // Premultiplied: the blended haze, plus a little light added on top,
    // so a thin stretch lightens what's behind it and never greys it like
    // smoke.
    let add = min(k * 0.02, 0.12) * mix(1.0, 0.6, day);
    return vec4<f32>(b.light.rgb * (glow * a + add), a);
}
