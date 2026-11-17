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
// Seen from outside, the front faces draw it; from inside, the back faces
// (the camera is in the beam: the march starts at the eye). Near the eye
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
    // tight beam holds more than a torch's).
    let spread = 1.0 / (1.0 + along * along * 0.0012);
    let start = smoothstep(0.0, 1.5, along);
    // Where it lands: a soft fade into the ground, and past its end.
    let ground = smoothstep(0.0, 1.2, p.y - b.air.z);
    let end = 1.0 - smoothstep(len - 1.0, len + 2.0, along);
    // The air: billows drifting slowly, and fine dust in it.
    let t = globals.time;
    let drift = vec3<f32>(t * 0.21, t * -0.05, t * 0.13);
    let billow = noise3(p * 0.18 + drift * 0.6);
    let dust = noise3(p * 1.3 + drift * 1.7);
    // (Billows vary it a lot: light in moving air, not an even tube.)
    let air = 0.2 + 1.1 * billow * billow + 0.45 * dust * dust;
    return soft * spread * start * ground * end * air;
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
    // Inside the cone (between the lamp and the ground, within its
    // angle)? Then only the back faces draw, marching from the eye.
    let rel = eye - b.apex.xyz;
    let c_along = dot(rel, b.axis.xyz);
    let c_off = length(rel - b.axis.xyz * c_along);
    let inside = c_along > 0.0 && c_along < b.axis.w && c_off < c_along * b.apex.w;
    let n = normalize(in.world_normal);
    let facing = dot(n, -rd) > 0.0;
    if (inside == facing) {
        discard;
    }
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
    let ds = span / f32(MARCH);
    // A per-pixel jitter on where the samples fall (no banding).
    let jit = hash3(at * 13.7 + vec3<f32>(globals.time));
    var sum = 0.0;
    for (var i = 0; i < MARCH; i = i + 1) {
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
    let a = min(1.0 - exp(-k * mix(0.07, 0.16, day)), cap * mix(0.45, 0.5, day));
    let glow = mix(2.2, 1.5, day);
    return vec4<f32>(b.light.rgb * glow, a);
}
