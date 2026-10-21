// Rain sheets: a handful of tall veils of heavier rain drifting across
// the valley with the wind, 40-200 m out (`world::storm`). One quad each,
// placed, sized and moved here; the CPU never touches them. They're more
// of the fog's own colour (rain is more air to see through), streaked
// with falling water, ragged at the sides, thinning into the cloud above,
// and they gust.

#import bevy_pbr::{
    mesh_view_bindings::{view, globals, fog},
    view_transformations::position_world_to_clip,
}

struct SheetParams {
    // x: how much rain (0: none), y: 1 in a storm (gusts), zw: wind m/s.
    rain: vec4<f32>,
    // x: brightness against a day's.
    light: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> sheet: SheetParams;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fade: f32,
    @location(2) seed: vec3<f32>,
    @location(3) size: vec2<f32>,
};

// The ring the sheets wrap round in (m), and where they show.
const BOX: f32 = 420.0;
const NEAR0: f32 = 40.0;
const NEAR1: f32 = 80.0;
const FAR0: f32 = 160.0;
const FAR1: f32 = 205.0;

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let seed = v.position;
    let t = globals.time;
    let cam = view.world_position.xyz;
    let wind = sheet.rain.zw * (0.8 + 0.4 * seed.y);
    var p = seed.xz * BOX + wind * t;
    var rel = (fract((p - cam.xz) / BOX + 0.5) - 0.5) * BOX;
    let d = max(length(rel), 1.0);
    let width = 60.0 + 70.0 * seed.z;
    // To about the far hills' line: higher up they stood in front of the
    // sky as pale pillars.
    let height = 30.0 + 0.12 * d;
    // Face the eye about the vertical.
    let right = vec3<f32>(-rel.y, 0.0, rel.x) / d;
    let centre = vec3<f32>(cam.x + rel.x, cam.y - 30.0, cam.z + rel.y);
    let pos = centre + right * (v.uv.x - 0.5) * width + vec3<f32>(0.0, v.uv.y * height, 0.0);
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(pos);
    out.uv = v.uv;
    out.seed = seed;
    out.size = vec2<f32>(width, height);
    out.fade = smoothstep(NEAR0, NEAR1, d) * (1.0 - smoothstep(FAR0, FAR1, d));
    if (sheet.rain.x <= 0.0 || out.fade <= 0.0) {
        out.clip_position = vec4<f32>(0.0, 0.0, -10.0, 1.0);
    }
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = globals.time;
    let s = in.seed.x * 37.0 + in.seed.z * 11.0;
    let x = in.uv.x * in.size.x;
    let y = in.uv.y * in.size.y;
    // The veil's shape: ragged at the sides, in a few denser columns, and
    // thinning up into the cloud.
    let cols = vnoise(vec2<f32>(x * 0.035 + s, t * 0.05)) * 0.7 + vnoise(vec2<f32>(x * 0.11 + s, 3.0)) * 0.3;
    let sides = smoothstep(0.0, 0.25, in.uv.x) * smoothstep(0.0, 0.25, 1.0 - in.uv.x);
    let veil = sides * smoothstep(0.3, 0.75, cols) * (1.0 - smoothstep(0.35, 1.0, in.uv.y));
    // Falling water: fine vertical streaks running down.
    let streak = vnoise(vec2<f32>(x * 1.3, y * 0.05 + t * 2.4 + s));
    // Gusts: a storm's sheets come and go.
    let gust = mix(0.85, 0.45 + 0.55 * (0.5 + 0.5 * sin(t * 0.37 + s)), sheet.rain.y);
    let a = sheet.rain.x * 0.42 * veil * (0.6 + 0.4 * streak) * gust * in.fade;
    // The fog's own colour (its alpha carries the wetness code; not ours).
    return vec4<f32>(fog.base_color.rgb * 1.05, clamp(a, 0.0, 0.6));
}
