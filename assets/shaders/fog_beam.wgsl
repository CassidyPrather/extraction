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
    // 0..1; z: the ground's height where it lands (world y); w: how fast
    // the light spreads down the throw (0: a searchlight's 0.0012; a
    // lamp's or torch's 0.08, thin a few metres out).
    air: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> b: FogBeam;

const MARCH: i32 = 12;
// Metres of beam a sample covers before another is added, and the fewest.
const STEP: f32 = 1.5;
const FEWEST: i32 = 5;

// Where a ray from `o` along `d` (unit), already in the cone at `t0`,
// leaves it: its wall, or its end. The cone's true surface, not the
// mesh's facets, so the march is the same length whichever face drew it.
fn exit_at(o: vec3<f32>, d: vec3<f32>, t0: f32) -> f32 {
    let v = b.axis.xyz;
    let len = b.axis.w;
    let co = o - b.apex.xyz;
    let dv = dot(d, v);
    let cv = dot(co, v);
    let c2 = 1.0 / (1.0 + b.apex.w * b.apex.w);
    let qa = dv * dv - c2;
    let qb = 2.0 * (dv * cv - c2 * dot(d, co));
    let qc = cv * cv - c2 * dot(co, co);
    var best = t0 + 2.0 * (len * b.apex.w + 1.0);
    // Its end.
    if (dv > 1e-4) {
        let tc = (len - cv) / dv;
        if (tc > t0) {
            best = min(best, tc);
        }
    }
    // Its wall (on the lamp's side of the apex, past where it starts).
    if (abs(qa) > 1e-6) {
        let disc = qb * qb - 4.0 * qa * qc;
        if (disc >= 0.0) {
            let s = sqrt(disc);
            let r1 = (-qb - s) / (2.0 * qa);
            let r2 = (-qb + s) / (2.0 * qa);
            for (var k = 0; k < 2; k = k + 1) {
                let r = select(r2, r1, k == 0);
                if (r > t0 + 0.02 && cv + r * dv > 0.0) {
                    best = min(best, r);
                }
            }
        }
    }
    return best;
}

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
    let spread = 1.0 / (1.0 + along * along * max(b.air.w, 0.0012));
    // Not in the eye's own few metres: a torch's beam starts at the hand,
    // and a beam over you lights the air round you, not the inside of
    // your eyes.
    let by_eye = smoothstep(0.4, 3.0, distance(p, view.world_position));
    // (Only a long throw's: a torch's or a drone's lamp is at the eye or
    // over it, and its few metres are all near the lamp.)
    let long = smoothstep(20.0, 40.0, len);
    let core = 1.0 + (1.2 + 2.0 * b.air.y) * exp(-along / 12.0) * long;
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
    return soft * spread * core * start * ground * end * air * by_eye;
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
    var span = exit_at(eye, rd, t0) - t0;
    if (inside) {
        span = min(span, dist);
    }
    span = max(span, 0.05);
    let n = clamp(i32(ceil(span / STEP)), FEWEST, MARCH);
    let ds = span / f32(n);
    // A jitter on where the samples fall (no banding), fixed in the world
    // and smooth across it: it doesn't crawl from frame to frame, nor
    // show as blocks.
    let jit = noise3(at * 3.0);
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
    let cap = mix(0.1, 1.0, smoothstep(2.0, 20.0, near));
    let day = b.air.y;
    // (By day it must still show over a lit field: more of the haze.)
    // (At night it's the brightest thing in view: a lower ceiling, so the
    // billows show in it and it doesn't read as a solid cone.)
    let a = min(1.0 - exp(-k * mix(0.07, 0.16, day)), cap * mix(0.32, 0.5, day));
    let glow = mix(1.7, 1.5, day);
    // Premultiplied: the blended haze, plus a little light added on top,
    // so a thin stretch lightens what's behind it and never greys it like
    // smoke.
    // (Capped near the eye too: a torch's beam fills the view from the
    // hand.)
    let add = min(k * 0.02, 0.12 * cap) * mix(1.0, 0.6, day);
    return vec4<f32>(b.light.rgb * (glow * a + add), a);
}
