// Terrain: blends ground layers on top of the standard PBR pipeline.
//
// Vertex colour carries splat weights: r = mud/wet, g = rock (slope),
// b = road, a = forest floor. uv_b.x is the road kind (1 asphalt, 2 gravel,
// 3 rail ballast, 4 dirt, 5 path), uv_b.y the wetness from water.
// Layers in the arrays: 0 grass, 1 forest floor, 2 mud, 3 rock, 4 gravel,
// 5 asphalt, 6 dirt road.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{view, globals},
}
#endif
#ifdef DISTANCE_FOG
#import bevy_pbr::mesh_view_bindings::fog
#endif

struct TerrainParams {
    textured: u32,
    wetness: f32,
    tile: f32,
    burnt: f32,
    // How full the puddles are (world::puddles::fill), and rain falling.
    puddles: f32,
    rain: f32,
    // Snow lying (world::snow), above this height.
    snow: f32,
    snow_line: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> terrain: TerrainParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var albedo_tex: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var albedo_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var normal_tex: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var normal_smp: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var arm_tex: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var arm_smp: sampler;

// Lanes packed through the snow (world::snow::Packed): per lane its ends
// (a.xz, b.xz), and its freshness (x) and half width (y).
struct Lanes {
    ends: array<vec4<f32>, 16>,
    extra: array<vec4<f32>, 16>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var<uniform> lanes: Lanes;

// How packed the snow is here, 0..1: inside a fresh lane, soft-edged.
fn packed_at(p: vec2<f32>) -> f32 {
    var k = 0.0;
    for (var i = 0u; i < 16u; i = i + 1u) {
        let fresh = lanes.extra[i].x;
        if (fresh <= 0.0) {
            continue;
        }
        let a = lanes.ends[i].xy;
        let b = lanes.ends[i].zw;
        let ab = b - a;
        let t = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-4), 0.0, 1.0);
        let d = length(p - (a + ab * t));
        let w = lanes.extra[i].y;
        k = max(k, fresh * (1.0 - smoothstep(w * 0.8, w, d)));
    }
    return k;
}

const LAYER_COUNT: u32 = 7u;

// ---------------------------------------------------------------------
// Noise for breakup and the procedural fallback.
// ---------------------------------------------------------------------

fn hash2(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var s = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 4; i = i + 1) {
        s = s + a * vnoise(q);
        q = q * 2.03 + vec2<f32>(17.0, 9.0);
        a = a * 0.5;
    }
    return s;
}

// ---------------------------------------------------------------------
// Layer sampling
// ---------------------------------------------------------------------

struct Layer {
    albedo: vec3<f32>,
    // Tangent-space normal, z up.
    normal: vec3<f32>,
    ao: f32,
    rough: f32,
    // A height-ish value for height blending (albedo luminance or noise).
    height: f32,
}

fn layer_tint(i: u32) -> vec3<f32> {
    switch i {
        case 0u: { return vec3<f32>(0.20, 0.24, 0.12); } // grass, olive and tired
        case 1u: { return vec3<f32>(0.17, 0.13, 0.09); } // needles and moss
        case 2u: { return vec3<f32>(0.13, 0.10, 0.07); } // mud
        case 3u: { return vec3<f32>(0.30, 0.30, 0.29); } // rock
        case 4u: { return vec3<f32>(0.33, 0.31, 0.28); } // gravel
        case 5u: { return vec3<f32>(0.11, 0.11, 0.115); } // asphalt
        default: { return vec3<f32>(0.22, 0.17, 0.12); } // dirt road
    }
}

fn layer_scale(i: u32) -> f32 {
    switch i {
        case 3u: { return 0.6; }  // rock is bigger
        case 5u: { return 1.2; }
        default: { return 1.0; }
    }
}

fn procedural_layer(i: u32, wp: vec2<f32>) -> Layer {
    var l: Layer;
    let n1 = fbm(wp * 0.35 + vec2<f32>(f32(i) * 13.0, 0.0));
    let n2 = fbm(wp * 3.1 + vec2<f32>(0.0, f32(i) * 7.0));
    let tint = layer_tint(i);
    var v = 0.75 + 0.5 * n1 + 0.25 * (n2 - 0.5);
    if (i == 0u) {
        // Grass: patches of yellower dead grass.
        let dead = smoothstep(0.55, 0.7, fbm(wp * 0.08));
        l.albedo = mix(tint, vec3<f32>(0.30, 0.27, 0.15), dead * 0.6) * v;
    } else {
        l.albedo = tint * v;
    }
    let e = 0.15;
    let hx = fbm((wp + vec2<f32>(e, 0.0)) * 3.1 + vec2<f32>(0.0, f32(i) * 7.0)) - n2;
    let hz = fbm((wp + vec2<f32>(0.0, e)) * 3.1 + vec2<f32>(0.0, f32(i) * 7.0)) - n2;
    let bump = select(1.2, 3.0, i == 3u || i == 4u);
    l.normal = normalize(vec3<f32>(-hx * bump, -hz * bump, 1.0));
    l.ao = 0.85 + 0.15 * n2;
    l.rough = select(0.92, 0.75, i == 5u);
    l.height = n2;
    return l;
}

// Hex-tiling (after Mikkelsen, "Practical Real-Time Hex-Tiling"): the
// plane is cut into a triangle grid; each grid vertex carries its own
// random offset and rotation, and a pixel blends the three samples of the
// triangle it falls in. No two neighbouring patches of ground are the same
// part of the texture, at any scale, so a field doesn't read as tiles.
fn hash22(p: vec2<f32>) -> vec2<f32> {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(q) * 43758.5453);
}

struct HexCell {
    weights: vec3<f32>,
    v1: vec2<f32>,
    v2: vec2<f32>,
    v3: vec2<f32>,
}

fn hex_cell(uv: vec2<f32>) -> HexCell {
    // About one texture tile per hex.
    let s = uv * 1.7320508;
    let skewed = vec2<f32>(s.x - 0.57735027 * s.y, 1.15470054 * s.y);
    let base = floor(skewed);
    let f = fract(skewed);
    let z = 1.0 - f.x - f.y;
    var c: HexCell;
    if (z > 0.0) {
        c.weights = vec3<f32>(z, f.y, f.x);
        c.v1 = base;
        c.v2 = base + vec2<f32>(0.0, 1.0);
        c.v3 = base + vec2<f32>(1.0, 0.0);
    } else {
        c.weights = vec3<f32>(-z, 1.0 - f.y, 1.0 - f.x);
        c.v1 = base + vec2<f32>(1.0, 1.0);
        c.v2 = base + vec2<f32>(1.0, 0.0);
        c.v3 = base + vec2<f32>(0.0, 1.0);
    }
    return c;
}

struct Tap {
    albedo: vec3<f32>,
    normal: vec3<f32>,
    arm: vec3<f32>,
}

// One of the three samples: the vertex's offset and rotation applied to
// the coordinates (and their derivatives), the normal turned back.
fn hex_tap(i: u32, uv: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>, vertex: vec2<f32>) -> Tap {
    let r = hash22(vertex + vec2<f32>(f32(i) * 7.31, 0.0));
    let a = r.x * 6.2831853;
    let cs = cos(a);
    let sn = sin(a);
    let rot = mat2x2<f32>(cs, sn, -sn, cs);
    let off = hash22(vertex * 1.31 + vec2<f32>(3.7, f32(i)));
    let u = rot * uv + off;
    let ddx = rot * dx;
    let ddy = rot * dy;
    var t: Tap;
    t.albedo = textureSampleGrad(albedo_tex, albedo_smp, u, i32(i), ddx, ddy).rgb;
    var n = textureSampleGrad(normal_tex, normal_smp, u, i32(i), ddx, ddy).xyz * 2.0 - 1.0;
    // Undo the rotation on the normal's tangent-plane part.
    n = vec3<f32>(cs * n.x - sn * n.y, sn * n.x + cs * n.y, n.z);
    t.normal = n;
    t.arm = textureSampleGrad(arm_tex, arm_smp, u, i32(i), ddx, ddy).rgb;
    return t;
}

fn sample_layer(i: u32, wp: vec2<f32>) -> Layer {
    if (terrain.textured == 0u) {
        return procedural_layer(i, wp);
    }
    let uv = wp / (terrain.tile / layer_scale(i));
    let dx = dpdx(uv);
    let dy = dpdy(uv);
    let c = hex_cell(uv);
    let t1 = hex_tap(i, uv, dx, dy, c.v1);
    let t2 = hex_tap(i, uv, dx, dy, c.v2);
    let t3 = hex_tap(i, uv, dx, dy, c.v3);
    // Sharpen the blend toward the brighter (higher) sample so the seams
    // between cells read as one surface, not a cross-fade.
    let lum = vec3<f32>(
        dot(t1.albedo, vec3<f32>(0.3, 0.59, 0.11)),
        dot(t2.albedo, vec3<f32>(0.3, 0.59, 0.11)),
        dot(t3.albedo, vec3<f32>(0.3, 0.59, 0.11)),
    );
    var w = pow(c.weights, vec3<f32>(5.0)) * (1.0 + 2.0 * lum);
    w = w / max(w.x + w.y + w.z, 1e-5);
    var l: Layer;
    // Large-scale brightness variation so fields don't read as wallpaper.
    let macro_v = 0.85 + 0.3 * fbm(wp * 0.02 + vec2<f32>(f32(i) * 3.0, 1.0));
    l.albedo = (t1.albedo * w.x + t2.albedo * w.y + t3.albedo * w.z) * macro_v;
    l.normal = normalize(t1.normal * w.x + t2.normal * w.y + t3.normal * w.z);
    let m = t1.arm * w.x + t2.arm * w.y + t3.arm * w.z;
    l.ao = m.r;
    l.rough = m.g;
    l.height = dot(l.albedo, vec3<f32>(0.3, 0.59, 0.11));
    return l;
}

// Puddles: the mask is `world::puddles::mask` (keep the two in step).
fn puddle_mask(p: vec2<f32>, basin: f32, road: f32, up: f32, fill: f32) -> f32 {
    if (fill <= 0.0) {
        return 0.0;
    }
    let depth = max(basin, road * 0.5) * 0.9
        + (vnoise(p * 0.45) - 0.5) * 0.5
        + (vnoise(p * 1.7 + vec2<f32>(13.0, 7.0)) - 0.5) * 0.15;
    let level = 1.05 - 0.55 * fill;
    return smoothstep(level, level + 0.07, depth) * smoothstep(0.985, 0.997, up);
}

#ifndef PREPASS_PIPELINE
// Rain on a puddle: a ring spreading from each drop, one drop a cell, two
// sizes of cell. Returns the ripples' slope (x, z).
fn ripples(p: vec2<f32>, t: f32) -> vec2<f32> {
    var g = vec2<f32>(0.0);
    for (var layer = 0; layer < 2; layer = layer + 1) {
        let scale = select(3.0, 5.3, layer == 1);
        let cell = floor(p * scale);
        let h = hash22(cell + vec2<f32>(f32(layer) * 17.0, 3.0));
        let centre = (cell + 0.2 + 0.6 * h) / scale;
        let phase = fract(t * (1.1 + 0.4 * h.y) + h.x * 7.0);
        let d = p - centre;
        let r = length(d);
        let ring = exp(-pow((r - phase * 0.14) * 60.0, 2.0)) * (1.0 - phase) * (1.0 - phase);
        g = g + d / max(r, 1e-3) * ring;
    }
    return g;
}
#endif

// Rock projected from three axes so cliffs don't stretch.
fn sample_rock_triplanar(wp: vec3<f32>, n: vec3<f32>) -> Layer {
    var w = abs(n);
    w = pow(w, vec3<f32>(4.0));
    w = w / (w.x + w.y + w.z);
    let lx = sample_layer(3u, wp.zy);
    let ly = sample_layer(3u, wp.xz);
    let lz = sample_layer(3u, wp.xy);
    var l: Layer;
    l.albedo = lx.albedo * w.x + ly.albedo * w.y + lz.albedo * w.z;
    l.ao = lx.ao * w.x + ly.ao * w.y + lz.ao * w.z;
    l.rough = lx.rough * w.x + ly.rough * w.y + lz.rough * w.z;
    l.height = lx.height * w.x + ly.height * w.y + lz.height * w.z;
    // Normals: just the top projection's detail, bent toward the surface.
    l.normal = normalize(mix(vec3<f32>(0.0, 0.0, 1.0), ly.normal, w.y) + vec3<f32>(0.0, 0.0, 0.3));
    return l;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let wp = in.world_position.xyz;
    let n_geo = normalize(in.world_normal);
#ifdef VERTEX_COLORS
    let splat = in.color;
#else
    let splat = vec4<f32>(0.0);
#endif
#ifdef VERTEX_UVS_B
    let road_kind = u32(round(in.uv_b.x));
    let water_wet = in.uv_b.y;
#else
    let road_kind = 0u;
    let water_wet = 0.0;
#endif

    // Breakup noise so layer edges are organic, not vertex-smooth.
    let breakup = fbm(wp.xz * 0.6) - 0.5;
    let fine = fbm(wp.xz * 4.0) - 0.5;

    var w: array<f32, 7>;
    let forest = clamp(splat.a + breakup * 0.6, 0.0, 1.0);
    let mud = clamp(splat.r * 1.4 + breakup * 0.5 + fine * 0.2 - 0.15, 0.0, 1.0);
    // Rock shows through in patches on the steep: grass holds on in tongues
    // up a moderate slope, bare rock only where it's steepest.
    let patches = fbm(wp.xz * 0.05 + vec2<f32>(11.0, 7.0)) - 0.5;
    let rock = clamp(splat.g * 1.4 + breakup * 0.6 + patches * 0.9 - 0.2, 0.0, 1.0);
    let road = clamp(splat.b * 1.15 + fine * 0.35, 0.0, 1.0);
    // Patches of bare earth in open grass.
    let bare = smoothstep(0.62, 0.75, fbm(wp.xz * 0.11 + vec2<f32>(5.0, 3.0))) * (1.0 - forest);

    w[0] = (1.0 - forest) * (1.0 - bare);
    w[1] = forest;
    w[2] = mud + bare * 0.6;
    w[3] = rock;
    var road_layer = 6u;
    if (road_kind == 1u) { road_layer = 5u; }
    if (road_kind == 2u || road_kind == 3u) { road_layer = 4u; }
    w[4] = 0.0;
    w[5] = 0.0;
    w[6] = 0.0;
    w[road_layer] = road;
    // Paths are worn dirt, faint.
    if (road_kind == 5u) { w[6] = road * 0.8; }
    // Asphalt crumbles at the edges into gravel.
    if (road_kind == 1u) { w[4] = road * (1.0 - road) * 2.0; }

    // Composite top-down: base (grass/forest), then mud, gravel/road, rock.
    var albedo = vec3<f32>(0.0);
    var tn = vec3<f32>(0.0, 0.0, 1.0);
    var ao = 1.0;
    var rough = 0.9;
    var acc = 0.0;
    for (var i = 0u; i < LAYER_COUNT; i = i + 1u) {
        if (w[i] < 0.01) { continue; }
        var l: Layer;
        if (i == 3u) {
            l = sample_rock_triplanar(wp, n_geo);
            // Wet, dark, and mossy where it faces up.
            let moss = smoothstep(0.55, 0.85, n_geo.y) * smoothstep(0.35, 0.6, fbm(wp.xz * 0.4));
            // Weathered in tones, warm like the boulders, not one grey sheet
            // (no level bands: on patchy ground they read as terraces).
            let tone = fbm(wp.xz * 0.15 + vec2<f32>(3.0, 9.0));
            let tint = mix(vec3<f32>(1.05, 0.9, 0.72), vec3<f32>(0.72, 0.74, 0.76), tone);
            // Rills: rain cuts a bank into grooves straight down the fall
            // line (stretched noise across the slope, long down it).
            let side = normalize(vec2<f32>(n_geo.x, n_geo.z) + vec2<f32>(1e-4, 0.0));
            let across = dot(wp.xz, vec2<f32>(-side.y, side.x));
            let rill = fbm(vec2<f32>(across * 1.3, wp.y * 0.12));
            let steep = smoothstep(0.9, 0.6, n_geo.y);
            let rills = mix(1.0, mix(0.55, 1.15, rill), steep);
            l.albedo = mix(l.albedo * 0.6 * tint * rills, vec3<f32>(0.10, 0.13, 0.06), moss * 0.7);
            // Weathered rock is matt; the scan's gloss turned a bank to a
            // sheen of sky at a glancing angle.
            l.rough = max(l.rough, 0.85);
        } else {
            l = sample_layer(i, wp.xz);
            // The forest floor sits in the trees' shade, lit by the sky
            // alone: at the scan's own albedo it went near black in a
            // headset. Lighter needle litter keeps it readable.
            if (i == 1u) {
                l.albedo = l.albedo * 1.4;
            }
        }
        // Height-aware blending: each layer covers the ones below it where
        // its own height wins.
        let cover = select(
            clamp(w[i] * 1.6 + (l.height - 0.5) * 0.8 - 0.2, 0.0, 1.0),
            1.0,
            acc == 0.0
        );
        albedo = mix(albedo, l.albedo, cover);
        tn = normalize(mix(tn, l.normal, cover));
        ao = mix(ao, l.ao, cover);
        rough = mix(rough, l.rough, cover);
        acc = acc + cover;
    }

    // Macro colour, tens of metres across: a meadow isn't one green.
    // Straw-dry patches, lusher green ones, moss through the forest floor.
    // (Two value-noise taps; the textures' own variation is all under a
    // few metres, so without this a field read as one flat olive.)
    let m1 = vnoise(wp.xz * 0.024 + vec2<f32>(31.0, 17.0));
    let m2 = vnoise(wp.xz * 0.07 + vec2<f32>(7.0, 43.0));
    // (The road's own splat, strictly: its noisy edge let the tint onto
    // the asphalt.)
    let off_road = 1.0 - clamp(splat.b * 2.0, 0.0, 1.0);
    let open_ground = (1.0 - forest) * (1.0 - rock) * off_road;
    let straw = smoothstep(0.45, 0.75, m1 * 0.65 + m2 * 0.35);
    let lush = smoothstep(0.5, 0.8, 1.0 - (m1 * 0.55 + m2 * 0.45));
    var grass_tint = mix(vec3<f32>(1.0), vec3<f32>(1.25, 1.08, 0.74), straw * 0.9);
    grass_tint = mix(grass_tint, vec3<f32>(0.86, 1.02, 0.78), lush * 0.8);
    let moss = smoothstep(0.45, 0.75, m2) * forest * (1.0 - rock) * off_road;
    albedo = albedo * mix(vec3<f32>(1.0), grass_tint, open_ground);
    albedo = mix(albedo, albedo * vec3<f32>(0.88, 1.1, 0.8), moss * 0.7);

    // A burnt valley: the ground goes to ash and charcoal, with rusty
    // burnt soil showing through where the fire went hottest (roads keep
    // their own colour). Brightness comes from the layers' own detail, so
    // the ground keeps its texture.
    if (terrain.burnt > 0.5) {
        let lum = dot(albedo, vec3<f32>(0.3, 0.59, 0.11));
        let pale = smoothstep(0.35, 0.7, fbm(wp.xz * 0.025 + vec2<f32>(4.0, 2.0)));
        let charcoal = vec3<f32>(lum) * vec3<f32>(0.62, 0.58, 0.54);
        let ash = vec3<f32>(lum) * vec3<f32>(2.1, 2.0, 1.85);
        let rust = vec3<f32>(lum) * vec3<f32>(1.75, 1.05, 0.62);
        var burnt = mix(charcoal, ash, pale);
        burnt = mix(burnt, rust, smoothstep(0.62, 0.85, m2) * (1.0 - pale) * 0.8);
        albedo = mix(albedo, burnt, 0.9 * (1.0 - road));
    }

    // Wetness: darker, glossier, especially near water and on roads.
    let wet = clamp(terrain.wetness * (0.5 + 0.5 * fbm(wp.xz * 0.05)) + water_wet * 0.8, 0.0, 1.0);
    let porosity = 1.0 - w[5] * 0.5;
    albedo = albedo * mix(1.0, 0.55, wet * porosity);
    // (Not mirror-wet: soaked ground should read as damp earth, not as
    // more water.)
    // Only roads and mud go glossy: wet grass and rock stay matt (a sheen
    // across a whole slope read as plaster under the overcast sky).
    rough = mix(rough, 0.42, wet * (0.15 + 0.6 * w[5] + 0.4 * mud));
    rough = max(rough, mix(0.8, 0.42, clamp(road + mud * wet, 0.0, 1.0)));

    // Standing water: dark over the mud, smooth, rain rings on it.
    var puddle = 0.0;
#ifndef PREPASS_PIPELINE
    puddle = puddle_mask(wp.xz, splat.r, road, n_geo.y, terrain.puddles);
    if (puddle > 0.01) {
        albedo = mix(albedo, albedo * 0.3, puddle);
        rough = mix(rough, 0.05, puddle);
        let rg = ripples(wp.xz, globals.time) * terrain.rain;
        tn = normalize(mix(tn, vec3<f32>(-rg.x * 0.18, -rg.y * 0.18, 1.0), puddle));
    }
#endif

    // Snow (world::snow): above the line, with a ragged edge; thin first,
    // in patches, the grass and stones through it, then lying all over the
    // flatter ground (roads too). It fills the puddles.
    if (terrain.snow > 0.002) {
        let edge = (fbm(wp.xz * 0.03) - 0.5) * 9.0;
        let above = smoothstep(terrain.snow_line - 3.0, terrain.snow_line + 4.0, wp.y + edge);
        let flat = smoothstep(0.55, 0.88, n_geo.y);
        let patches = fbm(wp.xz * 0.9) * 0.55 + fbm(wp.xz * 0.12 + vec2<f32>(4.0, 9.0)) * 0.45;
        let lie = clamp((terrain.snow * 1.6 - patches * 0.75 - (1.0 - flat) * 1.1) * 3.0, 0.0, 1.0);
        var s = above * lie;
        // A packed lane: the snow pressed flat and grey, the plough's
        // tread lines along it.
        let packed = packed_at(wp.xz) * s;
        if (s > 0.0) {
            // Blue in its hollows, a little uneven.
            let tone = vec3<f32>(0.74, 0.76, 0.8) * (0.94 + 0.08 * fine);
            albedo = mix(albedo, tone, s);
            rough = mix(rough, 0.78, s);
            // (Some of the ground's relief left: smoothed all over, a
            // snowfield in this light lost its shape.)
            tn = normalize(mix(tn, vec3<f32>(0.0, 0.0, 1.0), s * 0.55));
            ao = mix(ao, 1.0, s * 0.5);
            puddle = puddle * (1.0 - s);
            if (packed > 0.0) {
                albedo = mix(albedo, vec3<f32>(0.56, 0.59, 0.64) * (0.95 + 0.1 * fine), packed);
                rough = mix(rough, 0.55, packed);
                tn = normalize(mix(tn, vec3<f32>(0.0, 0.0, 1.0), packed));
            }
        }
    }

    // Tangent frame for an XZ projection: T = +X, B = +Z.
    let t = normalize(vec3<f32>(1.0, 0.0, 0.0) - n_geo * n_geo.x);
    // OpenGL-convention normal maps: green points up the image (-V).
    let b = cross(n_geo, t);
    let n = normalize(t * tn.x + b * tn.y + n_geo * tn.z);

    pbr_input.material.base_color = vec4<f32>(albedo, 1.0);
    pbr_input.material.perceptual_roughness = clamp(rough, 0.05, 1.0);
    pbr_input.material.metallic = 0.0;
    pbr_input.diffuse_occlusion = vec3<f32>(ao);
    pbr_input.N = n;
    // A puddle's specular is its own reflection, below.
    pbr_input.material.reflectance = pbr_input.material.reflectance * (1.0 - puddle);

    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    // The terrain wets itself (above): no sky openness for the wet patch.
    pbr_input.material.base_color.a = 0.0;

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    if (puddle > 0.01) {
        // The sky in the puddle (as the water's: the fog's colour at the
        // horizon, darker overhead), by Schlick's fresnel.
        let v = normalize(view.world_position - wp);
        let r = reflect(-v, n);
#ifdef DISTANCE_FOG
        let horizon = fog.base_color.rgb;
#else
        let horizon = vec3<f32>(0.2);
#endif
        let refl = mix(horizon * 0.9, horizon * 0.7, smoothstep(0.1, 0.7, r.y));
        let f = 0.02 + 0.98 * pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 5.0);
        out.color = vec4<f32>(mix(out.color.rgb, refl, f * puddle * 0.9), out.color.a);
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
