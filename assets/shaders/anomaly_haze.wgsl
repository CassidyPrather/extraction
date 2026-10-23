// Anomaly haze: refraction of the view behind it. The material is a
// StandardMaterial with specular_transmission, which puts it in Bevy's
// transmissive pass and gives it the view as it stood before (the view
// transmission texture); this shader samples that itself, refracted
// through a normal (Bevy's own fetch blurs, rejects by depth and turned
// rejected taps into black specks at the cards' edges). A camera-facing
// card whose normal is wobbled by a moving noise field shifts what you see
// through it a few centimetres, and nothing at its edges, so the card
// itself never shows. A well uses a real sphere instead: its round normals
// make a lens that turns the world over at its heart.
//
// haze.shape.x: 0 a card facing you, 1 a column card turning only about
// the vertical (heat over a burner, a lift's draught), 2 the mesh as it is
// (the well's lens).
// haze.shape.y: lift of the card's centre above the anomaly, metres.
// haze.shape.z: index of refraction, w: thickness (how far behind the
// surface the bent ray is followed, metres).
// haze.wave: x amplitude, y scale (1/m), z speed, w swirl (0 rising heat,
// 1 circling).
// haze.tint: rgb added glow at the heart, a: how much.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    mesh_view_bindings::{view, globals, view_transmission_texture, view_transmission_sampler},
    view_transformations::position_world_to_clip,
}

struct Haze {
    shape: vec4<f32>,
    wave: vec4<f32>,
    tint: vec4<f32>,
    // x: colour drained from what's seen through it, y: how much dimmed,
    // z: rim brightening, w: rim pulse (rad/s).
    drain: vec4<f32>,
    // Light of its own, so it reads in the dark too: x: the rim's (breathing
    // with it), y: a glint sweeping across the disc now and then (a mirror),
    // z: seconds between glints, w: this one's beat (0..1, from its spot).
    glow: vec4<f32>,
    // x: a step its rim takes away from what's behind (darker over light:
    // snow, fog; lighter over dark: night), y: the glint too, rather than
    // adding light.
    edge: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> haze: Haze;

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
                mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y);
    let b = mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
                mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y);
    return mix(a, b, u.z);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let mode = i32(haze.shape.x + 0.5);
    if (mode == 2) {
        out.world_position = mesh_functions::mesh_position_local_to_world(
            world_from_local, vec4<f32>(vertex.position, 1.0));
        out.world_normal = mesh_functions::mesh_normal_local_to_world(
            vertex.normal, vertex.instance_index);
    } else {
        // Size from the entity's scale; the card from the camera's axes.
        let sx = length(world_from_local[0].xyz);
        let sy = length(world_from_local[1].xyz);
        let centre = (world_from_local * vec4<f32>(0.0, 0.0, 0.0, 1.0)).xyz
            + vec3<f32>(0.0, haze.shape.y, 0.0);
        let to_eye = view.world_position - centre;
        var right: vec3<f32>;
        var up: vec3<f32>;
        if (mode == 1) {
            up = vec3<f32>(0.0, 1.0, 0.0);
            right = normalize(cross(up, to_eye) + vec3<f32>(1e-4, 0.0, 0.0));
        } else {
            right = view.world_from_view[0].xyz;
            up = view.world_from_view[1].xyz;
        }
        let p = centre + right * vertex.position.x * sx + up * vertex.position.y * sy;
        out.world_position = vec4<f32>(p, 1.0);
        out.world_normal = normalize(view.world_position - p);
    }
    out.position = position_world_to_clip(out.world_position.xyz);
#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif
    return out;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let t = globals.time;
    let mode = i32(haze.shape.x + 0.5);
    var uv = vec2<f32>(0.5, 0.5);
#ifdef VERTEX_UVS_A
    uv = in.uv;
#endif
    let q = uv * 2.0 - 1.0;
    // How much this spot bends: nothing at the card's edges.
    var edge: f32;
    if (mode == 1) {
        edge = (1.0 - smoothstep(0.35, 1.0, abs(q.x))) * (1.0 - smoothstep(0.55, 1.0, abs(q.y)))
            * smoothstep(-1.0, -0.6, q.y);
    } else if (mode == 2) {
        edge = 1.0;
    } else {
        edge = 1.0 - smoothstep(0.45, 1.0, length(q));
    }
    let wp = in.world_position.xyz;
    let s = haze.wave.y;
    let sp = haze.wave.z * t;
    // Rising heat: the field streams upward. Circling: it turns about the
    // vertical, quicker toward the middle.
    let ang = haze.wave.w * sp * (1.0 + 1.5 * (1.0 - length(q)));
    let rot = vec2<f32>(q.x * cos(ang) - q.y * sin(ang), q.x * sin(ang) + q.y * cos(ang));
    let field = vec3<f32>(rot * 1.5 + wp.xz * (1.0 - haze.wave.w) * 0.2, 0.0)
        + vec3<f32>(0.0, -sp * (1.0 - haze.wave.w), sp * 0.3);
    let e = 0.15;
    let n0 = vnoise(field * s * 2.0) + 0.5 * vnoise(field * s * 5.0 + vec3<f32>(3.1));
    let nx = vnoise((field + vec3<f32>(e, 0.0, 0.0)) * s * 2.0)
        + 0.5 * vnoise((field + vec3<f32>(e, 0.0, 0.0)) * s * 5.0 + vec3<f32>(3.1)) - n0;
    let ny = vnoise((field + vec3<f32>(0.0, e, 0.0)) * s * 2.0)
        + 0.5 * vnoise((field + vec3<f32>(0.0, e, 0.0)) * s * 5.0 + vec3<f32>(3.1)) - n0;
    let right = view.world_from_view[0].xyz;
    let up = view.world_from_view[1].xyz;
    let amp = haze.wave.x * edge;
    var n = normalize(in.world_normal);
    if (!is_front) {
        n = -n;
    }
    n = normalize(n + (right * nx + up * ny) / e * amp);
    // Through the surface and out the far side, `thickness` on: where
    // that lands on screen is what shows through here.
    let v = normalize(view.world_position - wp);
    let eta = 1.0 / max(haze.shape.z, 1.0);
    let tdir = refract(-v, n, eta);
    let exit = wp + tdir * haze.shape.w;
    let clip = view.clip_from_world * vec4<f32>(exit, 1.0);
    let sub = (clip.xy / clip.w) * vec2<f32>(0.5, -0.5) + 0.5;
    let full = vec2<f32>(textureDimensions(view_transmission_texture));
    let st = clamp((sub * view.viewport.zw + view.viewport.xy) / full, vec2<f32>(0.0), vec2<f32>(1.0));
    var behind = textureSampleLevel(view_transmission_texture, view_transmission_sampler, st, 0.0).rgb;
    // A Hush: the colour goes out of the world seen through it.
    let grey = dot(behind, vec3<f32>(0.2126, 0.7152, 0.0722));
    // How much of the world seen through it is touched: most at the heart,
    // fading out long before the card's edge, so no outline ever shows
    // (a card's rim is a circle on the screen, not a thing in the world);
    // a lens (the mesh as it is) by how squarely it faces you.
    var disc = 1.0 - smoothstep(0.25, 0.95, length(q));
    if (mode == 2) {
        disc = smoothstep(0.15, 0.7, abs(dot(normalize(in.world_normal), v)));
    }
    behind = mix(behind, vec3<f32>(grey), haze.drain.x * disc)
        * (1.0 - haze.drain.y * disc);
    // A glint: a bright bar crossing the disc in the first third of each
    // period, then gone (each anomaly on its own beat).
    if (haze.glow.y > 0.0) {
        let phase = fract(t / max(haze.glow.z, 0.1) + haze.glow.w);
        let at = -1.6 + 3.2 * clamp(phase * 3.0, 0.0, 1.0);
        let across = (q.x + q.y) * 0.7071 - at;
        let bar = exp(-(across * across) / 0.0225);
        let lit = bar * (1.0 - smoothstep(0.8, 0.95, length(q)));
        if (haze.edge.y > 0.5) {
            let away_g = select(-1.0, 1.0, grey < 0.5);
            behind = max(behind + vec3<f32>(away_g * haze.glow.y * 0.5 * lit), vec3<f32>(0.0));
        } else {
            behind += vec3<f32>(haze.glow.y * lit);
        }
    }
    // The heart's glow: a soft spot, never the card's outline.
    let glow = haze.tint.a * pow(max(0.0, 1.0 - length(q)), 3.0) * select(1.0, 0.0, mode == 2);
    behind += haze.tint.rgb * glow * (0.85 + 0.15 * sin(t * 7.0));
    var out: FragmentOutput;
    out.color = vec4<f32>(behind, 1.0);
    return out;
}
