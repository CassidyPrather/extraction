// Ground cover in the wind: bevy_pbr's mesh vertex shader, with each
// vertex pushed by the wind and pushed aside by whoever walks through.
//
// UV_1 carries, per vertex, how free it is to move (0 at a blade's root,
// 1 at its tip; squared by the mesh builder) and the plant's own phase, so
// a meadow doesn't move in step.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
}

struct Sway {
    // xy: the wind's direction (world x/z), z: strength 0..1, w: gustiness.
    wind: vec4<f32>,
    // xyz: the walker's feet, w: how far round them plants lean away.
    pusher: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> sway: Sway;

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
    var o = vec3<f32>(dir.x, 0.0, dir.y) * push * 0.18 * weight;
    // Lean away from the walker's legs.
    let away = world.xz - sway.pusher.xz;
    let d = length(away);
    let near = 1.0 - smoothstep(0.15, sway.pusher.w, d);
    if (near > 0.0 && abs(world.y - sway.pusher.y) < 1.5) {
        let out2 = away / max(d, 1e-3);
        o += vec3<f32>(out2.x, -0.5, out2.y) * near * 0.25 * weight;
    }
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
    out.world_position = vec4<f32>(
        out.world_position.xyz + offset(out.world_position.xyz, vertex.uv_b.x, vertex.uv_b.y),
        1.0
    );
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
