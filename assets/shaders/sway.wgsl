// Ground cover in the wind: bevy_pbr's mesh vertex shader, with each
// vertex pushed by the wind and pushed aside by whoever walks through.
//
// UV_1 carries, per vertex, how free it is to move (0 at a blade's root,
// 1 at its tip; squared by the mesh builder) and the plant's own phase, so
// a meadow doesn't move in step.
//
// Blades thicken with distance, about half a pixel a side in the headset:
// a 2 cm blade is a pixel wide at 10 m and a fraction of one past that,
// and a sub-pixel blade flickers in and out of the MSAA samples as the
// head moves. Thickened, it holds its coverage and the meadow reads as a
// soft mass instead of crawling.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::{globals, view},
}

// World metres a side per metre of distance: half a pixel at about
// 1800 px across 100 degrees.
const WIDEN: f32 = 0.0005;

struct Sway {
    // xy: the wind's direction (world x/z), z: strength 0..1, w: gustiness.
    wind: vec4<f32>,
    // xyz: the walker's feet, w: how far round them plants lean away.
    pusher: vec4<f32>,
    // The walker's hands, down among the plants: xyz where, w how far round
    // them plants are parted (0: not pushing).
    hand_a: vec4<f32>,
    hand_b: vec4<f32>,
    // Snow lying (world::snow): x how much, y the line (m).
    snow: vec4<f32>,
    // Robots among the plants: xyz where, w how far round (0: none).
    robot_a: vec4<f32>,
    robot_b: vec4<f32>,
    robot_c: vec4<f32>,
    robot_d: vec4<f32>,
    // The nearest anomalies (`anomaly::visuals::grass_anomalies`): xyz the
    // centre, w its radius (0: none); `kinds` the kind of each (1 lift,
    // 2 burner, 3 snare, 4 well, 5 mirror, 6 hush, 7 frost).
    anomaly_a: vec4<f32>,
    anomaly_b: vec4<f32>,
    anomaly_c: vec4<f32>,
    anomaly_d: vec4<f32>,
    kinds: vec4<f32>,
}

// How far a plant here sinks into lying snow (m): with the snow's depth,
// above the line, by a little noise of its own; one plant in ten stands
// out of it as a dry stalk. (Standing through it, the grass made a
// snowfield read as a frosted lawn.)
fn buried(world: vec3<f32>, phase: f32) -> f32 {
    if (sway.snow.x <= 0.002) {
        return 0.0;
    }
    let above = smoothstep(sway.snow.y - 3.0, sway.snow.y + 4.0, world.y);
    let depth = sway.snow.x * above * (0.7 + 0.5 * fract(phase * 7.31));
    return select(depth, depth * 0.35, fract(phase * 13.7) < 0.1);
}

// Parted round a hand: leaning away from it, more the nearer.
fn parted(world: vec3<f32>, hand: vec4<f32>, weight: f32) -> vec3<f32> {
    if (hand.w <= 0.0) {
        return vec3<f32>(0.0);
    }
    let away = world - hand.xyz;
    let d = length(away);
    let near = 1.0 - smoothstep(0.05, hand.w, d);
    let out2 = away.xz / max(length(away.xz), 1e-3);
    return vec3<f32>(out2.x, -0.3, out2.y) * near * 0.2 * weight;
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> sway: Sway;

// What an anomaly does to the plants in it, out to its edge and no
// further (the edge firm within 15 cm, so where the grass changes is where
// it starts): `still` how much of the wind it takes away, `o` how it moves
// them, `sink` how far it presses them down.
struct InAnomaly {
    still: f32,
    o: vec3<f32>,
    sink: f32,
}

fn in_anomaly(world: vec3<f32>, a: vec4<f32>, kind: f32, weight: f32, phase: f32) -> InAnomaly {
    var r: InAnomaly;
    r.still = 0.0;
    r.o = vec3<f32>(0.0);
    r.sink = 0.0;
    if (a.w <= 0.0) {
        return r;
    }
    let rel = world.xz - a.xz;
    let d = length(rel);
    let inside = 1.0 - smoothstep(a.w - 0.15, a.w, d);
    if (inside <= 0.0 || abs(world.y - a.y) > 3.0) {
        return r;
    }
    let t = globals.time;
    let k = i32(kind + 0.5);
    let toward = -rel / max(d, 1e-3);
    if (k == 1) {
        // A lift: torn up and carried off, the ground under it swept bare.
        r.still = 1.0;
        r.sink = 0.5;
    } else if (k == 2) {
        // A burner: burnt to the ground.
        r.still = 1.0;
        r.sink = 0.5;
    } else if (k == 3) {
        // A snare: standing on end, quivering.
        r.still = 1.0;
        r.o = vec3<f32>(sin(t * 37.0 + phase * 40.0), 0.4, cos(t * 41.0 + phase * 31.0))
            * vec3<f32>(0.015, 0.1, 0.015) * weight;
    } else if (k == 4) {
        // A well: laid flat toward its heart.
        r.still = 1.0;
        r.o = vec3<f32>(toward.x, -0.6, toward.y) * 0.3 * weight;
    } else if (k == 5) {
        // A mirror: wiped flat by what's blinked through it.
        r.still = 1.0;
        r.o = vec3<f32>(toward.x, -0.7, toward.y) * 0.2 * weight;
        r.sink = 0.06;
    } else if (k == 6) {
        // A hush: dead still whatever the wind, pressed down into the
        // dust settled on it.
        r.still = 1.0;
        r.sink = 0.12;
    } else if (k == 7) {
        // A frost: stiff, bowed under the rime.
        r.still = 1.0;
        r.o = vec3<f32>(0.0, -0.08, 0.0) * weight;
        r.sink = 0.03;
    }
    r.still *= inside;
    r.o *= inside;
    r.sink *= inside;
    return r;
}

fn anomalies(world: vec3<f32>, weight: f32, phase: f32) -> InAnomaly {
    var r = in_anomaly(world, sway.anomaly_a, sway.kinds.x, weight, phase);
    for (var i = 1; i < 4; i++) {
        var a = sway.anomaly_b;
        var k = sway.kinds.y;
        if (i == 2) {
            a = sway.anomaly_c;
            k = sway.kinds.z;
        } else if (i == 3) {
            a = sway.anomaly_d;
            k = sway.kinds.w;
        }
        let n = in_anomaly(world, a, k, weight, phase);
        r.still = max(r.still, n.still);
        r.o += n.o;
        r.sink = max(r.sink, n.sink);
    }
    return r;
}

fn offset(world: vec3<f32>, weight: f32, phase: f32) -> vec3<f32> {
    let t = globals.time;
    let dir = normalize(vec2<f32>(sway.wind.x, sway.wind.y) + vec2<f32>(1e-4, 0.0));
    let along = dot(world.xz, dir);
    // Gusts roll across the field (a slow wave along the wind), and each
    // plant flutters on its own.
    let gust = 0.5 + 0.5 * sin(along * 0.12 - t * 1.3) * sway.wind.w;
    let flutter = sin(t * 3.1 + phase * 6.283 + along * 0.7) * 0.35
        + sin(t * 5.3 + phase * 11.0) * 0.15;
    let push = sway.wind.z * (0.6 * gust + flutter * (0.4 + 0.6 * gust));
    let held = anomalies(world, weight, phase);
    var o = vec3<f32>(dir.x, 0.0, dir.y) * push * 0.18 * weight * (1.0 - held.still) + held.o;
    // Lean away from the walker's legs.
    let away = world.xz - sway.pusher.xz;
    let d = length(away);
    let near = 1.0 - smoothstep(0.15, sway.pusher.w, d);
    if (near > 0.0 && abs(world.y - sway.pusher.y) < 1.5) {
        let out2 = away / max(d, 1e-3);
        o += vec3<f32>(out2.x, -0.5, out2.y) * near * 0.25 * weight;
    }
    o += parted(world, sway.hand_a, weight) + parted(world, sway.hand_b, weight);
    o += parted(world, sway.robot_a, weight) + parted(world, sway.robot_b, weight)
        + parted(world, sway.robot_c, weight) + parted(world, sway.robot_d, weight);
    return o;
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index
    );
#endif

#ifdef VERTEX_POSITIONS
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );
#ifdef VERTEX_UVS_B
    let sunk = max(
        buried(out.world_position.xyz, vertex.uv_b.y),
        anomalies(out.world_position.xyz, vertex.uv_b.x, vertex.uv_b.y).sink
    );
    out.world_position = vec4<f32>(
        out.world_position.xyz + offset(out.world_position.xyz, vertex.uv_b.x, vertex.uv_b.y)
            - vec3<f32>(0.0, sunk, 0.0),
        1.0
    );
#endif
#ifdef VERTEX_UVS_A
#ifdef VERTEX_NORMALS
    // A blade's two edges are its strip's u = 0 and u = 1; its side runs
    // across its lean (the normal leans out along the blade). Fern fronds
    // sit inside an atlas, never at exactly 0 or 1.
    if (vertex.uv.x == 0.0 || vertex.uv.x == 1.0) {
        let lean = out.world_normal.xz + vec2<f32>(1e-4, 0.0);
        let o = normalize(lean);
        let side = vec3<f32>(-o.y, 0.0, o.x);
        let dist = distance(view.world_position, out.world_position.xyz);
        let edge = vertex.uv.x * 2.0 - 1.0;
        out.world_position = vec4<f32>(
            out.world_position.xyz + side * edge * WIDEN * dist,
            1.0
        );
    }
#endif
#endif
    out.position = position_world_to_clip(out.world_position.xyz);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
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
