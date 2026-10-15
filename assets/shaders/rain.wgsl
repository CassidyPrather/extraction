// Rain: thousands of streaks in a box that wraps around the camera, all
// moved here so the CPU never touches them. Each streak is a quad whose
// four corners share a base point (position) and differ in uv: x picks the
// side, y the end. Streaks face the camera about their own (wind-slanted)
// axis.

#import bevy_pbr::{
    mesh_view_bindings::{view, globals},
    view_transformations::position_world_to_clip,
}

struct RainParams {
    // x: intensity 0..1, y: fall speed m/s, z: streak length m, w: box size m
    params: vec4<f32>,
    // xz: wind, m/s; y: 1 for ash (pale flakes that tumble); w: width m
    wind: vec4<f32>,
}

// The nearest roofs: per roof, (centre x, centre z, bottom y, top y) then
// (half width, half depth, cos yaw, sin yaw). Nothing falls inside one.
struct Shelters {
    boxes: array<vec4<f32>, 16>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> rain: RainParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> shelters: Shelters;

fn sheltered(p: vec3<f32>) -> bool {
    for (var i = 0u; i < 8u; i = i + 1u) {
        let a = shelters.boxes[2u * i];
        let b = shelters.boxes[2u * i + 1u];
        if (b.x <= 0.0) {
            continue;
        }
        let d = p.xz - a.xy;
        // Into the building's frame (the game's yaw is about -Y).
        let lx = d.x * b.z + d.y * b.w;
        let lz = -d.x * b.w + d.y * b.z;
        if (abs(lx) < b.x && abs(lz) < b.y && p.y > a.z && p.y < a.w) {
            return true;
        }
    }
    return false;
}

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fade: f32,
    @location(2) ash: f32,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let box_size = rain.params.w;
    let speed = rain.params.y;
    let t = globals.time;
    let cam = view.world_position.xyz;
    let fall = vec3<f32>(rain.wind.x, -speed, rain.wind.z);
    // Base point drifts with the fall, wrapped into the box round the camera.
    var p = v.position + fall * t;
    if (rain.wind.y > 0.5) {
        // Ash tumbles as it drifts.
        let ph = v.position.x * 3.1 + v.position.z * 1.7;
        p = p + vec3<f32>(sin(t * 0.8 + ph), 0.0, cos(t * 0.6 + ph * 1.3)) * 0.35;
    }
    p = p - cam;
    p = (fract(p / box_size + 0.5) - 0.5) * box_size;
    let centre = cam + p;
    let axis = normalize(-fall);
    let to_cam = normalize(cam - centre);
    let side = normalize(cross(axis, to_cam));
    let width = select(0.006, rain.wind.w, rain.wind.w > 0.0);
    let pos = centre + side * (v.uv.x - 0.5) * width + axis * v.uv.y * rain.params.z;
    var out: VertexOutput;
    out.clip_position = position_world_to_clip(pos);
    out.uv = v.uv;
    out.ash = rain.wind.y;
    // Fade near the box edge (no popping) and right at the eye.
    let d = length(p) / (box_size * 0.5);
    out.fade = (1.0 - smoothstep(0.6, 1.0, d)) * smoothstep(0.4, 1.5, length(p));
    // Thin the field out with intensity: drop a fraction of streaks.
    let keep = fract(sin(dot(v.position.xz, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    if (keep > rain.params.x || sheltered(centre)) {
        out.clip_position = vec4<f32>(0.0, 0.0, -10.0, 1.0);
    }
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let edge = 1.0 - abs(in.uv.x - 0.5) * 2.0;
    if (in.ash > 0.5) {
        // A soft grey flake.
        let r = length(in.uv - vec2<f32>(0.5, 0.5)) * 2.0;
        let a = 0.55 * in.fade * (1.0 - smoothstep(0.5, 1.0, r));
        return vec4<f32>(0.6, 0.58, 0.55, a);
    }
    let a = 0.22 * in.fade * edge * (0.4 + 0.6 * in.uv.y);
    return vec4<f32>(0.62, 0.66, 0.70, a);
}
