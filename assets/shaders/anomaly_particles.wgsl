// Anomaly particles, moved on the GPU: one mesh of N quads per anomaly
// system, every quad's motion a pure function of time and its seed (UV_1),
// so an anomaly's leaves, embers, sparks or grit are a single draw.
//
// Modes (params.mode.x):
//   0 lift leaves     circling columns; drawn in as it winds up to pop
//   1 burner embers   rising from the patch
//   2 burner smoke    big soft puffs, rising and drifting off
//   3 snare sparks    blinking round the core, flung out on a discharge
//   4 well grit       an accretion disc spiralling into the heart
//   5 mirror glints   twinkles on the fold's surface, bright when revealed
//   6 lift dust       a low swirl over the flattened ring
//   7 hush dust       hanging dead still in a dome
//
// Fog is applied here through bevy_pbr's apply_fog, which render/fog.rs
// patches to add the ground-fog banks and Kettle Pass's cloud cap, so a
// leaf in a bank is as lost as the ground under it; glowing things lose
// a little less, so embers and sparks carry a little further.

#import bevy_pbr::{
    mesh_functions,
    mesh_view_bindings::{view, globals, fog},
    view_transformations::position_world_to_clip,
}
#ifdef DISTANCE_FOG
#import bevy_pbr::pbr_functions::apply_fog
#endif

struct Params {
    // x: mode, y: radius, z: reach (well), w: glow (0 lit-ish, 1 emissive)
    mode: vec4<f32>,
    // x: charge, y: revealed 0..1, z: flash 0..1, w: base size
    state: vec4<f32>,
    // rgb: colour (linear, may exceed 1 for glow), a: opacity
    color: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: Params;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var tex_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) seed: vec2<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
    @location(2) world: vec3<f32>,
    @location(3) heat: f32,
};

const TAU: f32 = 6.2831853;
const PI: f32 = 3.1415927;

fn h(s: f32, k: f32) -> f32 {
    return fract(sin(s * k + 0.17) * 43758.5453);
}

struct P {
    pos: vec3<f32>,
    size: f32,
    alpha: f32,
    spin: f32,
    // 1: leaf-like, tumbling (squash the quad across)
    tumble: f32,
    heat: f32,
};

fn particle(mode: i32, s: f32, s2: f32, t: f32) -> P {
    let R = params.mode.y;
    let charge = params.state.x;
    let reveal = params.state.y;
    let flash = params.state.z;
    var p: P;
    p.size = params.state.w * (0.6 + 0.8 * h(s, 3.1));
    p.alpha = 1.0;
    p.spin = s * TAU + t * (0.5 + h(s, 9.7));
    p.tumble = 0.0;
    p.heat = 0.0;
    if (mode == 0) {
        let windup = clamp((charge - 0.5) * 2.0, 0.0, 1.0);
        let speed = (0.5 + 0.7 * h(s, 1.7)) * (1.0 + 1.6 * charge);
        let a = s * TAU + t * speed;
        if (h(s, 4.4) < 0.35) {
            // Inflow: slides in along the ground from beyond the ring and
            // climbs into the column, only while it winds up.
            let life = fract(t * (0.35 + 0.2 * h(s, 5.5)) + h(s, 6.6));
            let r = R * 1.8 * (1.0 - life) + R * 0.3 * life;
            p.pos = vec3<f32>(cos(a * 0.3) * r, 0.05 + life * life * 2.0, sin(a * 0.3) * r);
            p.alpha = windup * smoothstep(0.0, 0.1, life);
        } else {
            let r = R * (0.15 + 0.75 * h(s, 7.1)) * (1.0 - 0.45 * charge);
            let y = 0.2 + 2.6 * (0.5 + 0.5 * sin(t * 0.35 + s * 11.0)) * (0.4 + 0.6 * h(s, 3.3))
                * (1.0 + 0.5 * charge);
            p.pos = vec3<f32>(cos(a) * r, y, sin(a) * r);
        }
        p.spin = t * (1.3 + charge * 3.0) + s * 20.0;
        p.tumble = 1.0;
    } else if (mode == 1) {
        let life = fract(t * (0.3 + 0.25 * h(s, 2.9)) * (1.0 + flash) + h(s, 5.3));
        let a = s * TAU;
        let r = R * 0.8 * sqrt(h(s, 4.7)) * (1.0 - 0.3 * life);
        let wob = vec3<f32>(sin(t * 2.0 + s * 9.0), 0.0, cos(t * 1.7 + s * 7.0)) * 0.15 * life;
        p.pos = vec3<f32>(cos(a) * r, life * 3.4 * (1.0 + 0.6 * flash), sin(a) * r) + wob;
        p.alpha = sin(life * PI) * (0.7 + 0.3 * sin(t * 13.0 + s * 50.0));
        p.heat = 1.0;
    } else if (mode == 2) {
        let life = fract(t / (8.0 + 4.0 * h(s, 1.9)) + h(s, 6.1));
        let a = s * TAU;
        let start = vec3<f32>(cos(a), 0.0, sin(a)) * R * 0.5 * h(s, 3.7);
        let drift = vec3<f32>(0.8, 0.0, 0.5) * life * life * 2.5;
        p.pos = start + drift + vec3<f32>(0.0, 0.4 + life * 5.0, 0.0);
        p.size = params.state.w * (0.6 + life * 2.2) * sqrt(1.0 - life);
        p.alpha = sqrt(1.0 - life) * smoothstep(0.0, 0.08, life);
        p.spin = s * 7.0 + t * 0.1;
    } else if (mode == 3) {
        let phase = fract(t * (3.0 + 4.0 * h(s, 1.3)) + s);
        let a = s * TAU + t * 0.4;
        let b = h(s, 8.9) * PI - PI * 0.5;
        let r = R * 0.35 * (0.6 + 0.4 * phase) * (1.0 + 2.2 * flash);
        p.pos = vec3<f32>(cos(a) * cos(b) * r, 0.8 + sin(b) * r, sin(a) * cos(b) * r);
        let lit = step(h(s + floor(t * 9.0), 2.3), 0.3 + 0.7 * max(charge, flash));
        p.alpha = lit * (1.0 - phase * 0.6);
        p.size = p.size * (1.0 + 1.5 * flash);
        p.heat = 1.0;
    } else if (mode == 4) {
        let reach = params.mode.z;
        let life = fract(t * (0.07 + 0.08 * h(s, 1.1)) + h(s, 2.2));
        // Most of it close in, circling the heart; a thin trail out to
        // the reach.
        let rr = max(reach * pow(1.0 - life, 1.6), 0.08);
        // Faster nearer the heart, as anything falling in would.
        let a = s * TAU + t * (1.2 / sqrt(rr));
        let tilt = (h(s, 3.3) - 0.5) * 0.25;
        p.pos = vec3<f32>(cos(a) * rr, 1.3 + sin(a) * rr * tilt, sin(a) * rr);
        p.size = p.size * (0.35 + 0.65 * rr / reach);
        p.alpha = smoothstep(0.0, 0.1, life) * (1.0 - smoothstep(0.85, 1.0, life));
        // Stretched toward the heart's glow as it falls in.
        p.heat = smoothstep(0.5, 1.0, life);
        p.tumble = 0.6;
        p.spin = a;
    } else if (mode == 5) {
        let a = s * TAU;
        let b = acos(1.0 - 2.0 * h(s, 2.7));
        let r = R * (0.95 + 0.05 * sin(t * 2.0 + s * 30.0));
        p.pos = vec3<f32>(sin(b) * cos(a) * r, 0.9 + cos(b) * r * 0.8, sin(b) * sin(a) * r);
        let tw = pow(max(0.0, sin(t * (1.0 + 2.5 * h(s, 4.1)) + s * 40.0)), 18.0);
        p.alpha = tw * (0.25 + 0.75 * max(reveal, flash));
        p.heat = 1.0;
    } else if (mode == 7) {
        // Hush: dust hanging dead still in a dome, barely drifting.
        let a = s * TAU;
        let rr = R * sqrt(h(s, 2.3));
        let y = 0.15 + 2.4 * h(s, 4.9) * (1.0 - 0.5 * rr / R);
        let drift = vec3<f32>(sin(t * 0.05 + s * 7.0), 0.0, cos(t * 0.04 + s * 5.0)) * 0.05;
        p.pos = vec3<f32>(cos(a) * rr, y, sin(a) * rr) + drift;
        p.alpha = 0.6 + 0.4 * reveal;
        p.spin = s * 3.0;
    } else {
        let a = s * TAU + t * (0.3 + 0.9 * charge) * (0.6 + 0.4 * h(s, 1.4));
        let r = R * (0.3 + 0.75 * h(s, 2.1));
        p.pos = vec3<f32>(cos(a) * r, 0.08 + 0.35 * h(s, 3.9) * (1.0 + 2.0 * charge), sin(a) * r);
        p.size = params.state.w * (0.6 + 0.8 * h(s, 5.1));
        p.alpha = 0.3 + 0.7 * charge;
        p.spin = a;
    }
    return p;
}

@vertex
fn vertex(v: Vertex) -> VOut {
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    let centre = (world_from_local * vec4<f32>(0.0, 0.0, 0.0, 1.0)).xyz;
    let t = globals.time;
    let p = particle(i32(params.mode.x + 0.5), v.seed.x, v.seed.y, t);
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    let c = cos(p.spin);
    let sn = sin(p.spin);
    var corner = vec2<f32>(v.position.x * c - v.position.y * sn, v.position.x * sn + v.position.y * c);
    // A leaf seen edge-on as it tumbles.
    corner.x *= mix(1.0, abs(cos(p.spin * 0.7)) + 0.15, p.tumble);
    // Thickened with distance (more for glowing things), so a spark or an
    // ember keeps a pixel or two at 20 m instead of flickering out.
    let far = distance(centre + p.pos, view.world_position);
    let grow = clamp(far / mix(15.0, 7.0, params.mode.w), 1.0, mix(2.0, 2.5, params.mode.w));
    let world = centre + p.pos + (right * corner.x + up * corner.y) * p.size * grow;
    var out: VOut;
    out.clip = position_world_to_clip(world);
    out.uv = v.uv;
    let d = distance(world, view.world_position);
    out.alpha = p.alpha * (1.0 - smoothstep(70.0, 95.0, d));
    out.world = world;
    out.heat = p.heat;
    return out;
}

// The Zone's things show through its murk: fog takes an anomaly's
// particles as if they were this share as far off.
const FOG_REACH: f32 = 0.5;

@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    let fog_at = view.world_position + (in.world - view.world_position) * FOG_REACH;
    let texel = textureSample(tex, tex_sampler, in.uv);
    let glow = params.mode.w;
    var rgb = params.color.rgb * mix(texel.rgb, vec3<f32>(1.0), glow);
    // Grit falling into a well warms toward its violet glow.
    rgb = mix(rgb, vec3<f32>(2.0, 0.8, 4.0), in.heat * (1.0 - glow) * 0.6);
    let a = texel.a * params.color.a * in.alpha;
    if (glow > 0.5) {
        // Additive (Bevy's premultiplied blend with no alpha): fog only
        // dims it. The fog is mix(x, F, f), so f is 1 - (fog(1) - fog(0)).
        var keep = vec3<f32>(1.0);
#ifdef DISTANCE_FOG
        keep = apply_fog(fog, vec4<f32>(1.0), fog_at, view.world_position, in.clip.xy).rgb
            - apply_fog(fog, vec4<f32>(0.0, 0.0, 0.0, 1.0), fog_at, view.world_position, in.clip.xy).rgb;
#endif
        let dim = mix(vec3<f32>(1.0), keep, 0.75);
        return vec4<f32>(rgb * a * dim, 0.0);
    }
    if (a < 0.02) {
        discard;
    }
#ifdef DISTANCE_FOG
    rgb = apply_fog(fog, vec4<f32>(rgb, 1.0), fog_at, view.world_position, in.clip.xy).rgb;
#endif
    return vec4<f32>(rgb, a);
}
