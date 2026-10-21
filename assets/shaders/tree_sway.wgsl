// A near tree's foliage cards in the wind: bevy_pbr's mesh vertex shader,
// each vertex pushed by how high up the tree and how far out along a
// branch it is (the cards are one mesh per tree variant, so this comes
// from the local position, and each tree's phase from where it stands).
// A card close to the eye shrinks to its centre (`trees::card_scale`):
// whole from 2 m, gone by 1.2 m, smoothly.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::{globals, view},
}

struct Sway {
    // xy: the wind's direction (world x/z), z: strength 0..1, w: gustiness.
    wind: vec4<f32>,
    // xyz: the walker's feet, w: how far round them plants lean away.
    pusher: vec4<f32>,
    // The walker's hands, up in the branches: xyz where, w how far round
    // them the foliage parts (0: not pushing).
    hand_a: vec4<f32>,
    hand_b: vec4<f32>,
    // Robots pushing through low branches: xyz where, w how far round.
    robot_a: vec4<f32>,
    robot_b: vec4<f32>,
    robot_c: vec4<f32>,
    robot_d: vec4<f32>,
}

// Branch ends pushed aside by a hand, more the nearer and the freer.
fn parted(world: vec3<f32>, hand: vec4<f32>, free: f32) -> vec3<f32> {
    if (hand.w <= 0.0) {
        return vec3<f32>(0.0);
    }
    let away = world - hand.xyz;
    let near = 1.0 - smoothstep(0.05, hand.w, length(away));
    return normalize(away + vec3<f32>(0.0, 1e-3, 0.0)) * near * 0.3 * (0.3 + 0.7 * free);
}

// Branches pushed aside by a robot walking under them: its spot is low
// (at its feet), so it's a column, by distance across the ground, from
// its feet up to about its height.
fn shouldered(world: vec3<f32>, robot: vec4<f32>, free: f32) -> vec3<f32> {
    if (robot.w <= 0.0 || world.y < robot.y - 0.3 || world.y > robot.y + 2.5) {
        return vec3<f32>(0.0);
    }
    let away = world.xz - robot.xz;
    let near = 1.0 - smoothstep(0.05, robot.w, length(away));
    let out2 = away / max(length(away), 1e-3);
    return vec3<f32>(out2.x, 0.15, out2.y) * near * 0.3 * (0.3 + 0.7 * free);
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> sway: Sway;

fn offset(world: vec3<f32>, local: vec3<f32>, origin: vec3<f32>) -> vec3<f32> {
    let t = globals.time;
    let dir = normalize(vec2<f32>(sway.wind.x, sway.wind.y) + vec2<f32>(1e-4, 0.0));
    // How free this part of the tree is: nothing at the foot, most at the
    // top and the ends of the branches.
    let up = clamp((local.y - 2.0) / 14.0, 0.0, 1.0);
    let out = clamp(length(local.xz) / 3.0, 0.0, 1.0);
    let weight = up * up * (0.5 + 0.5 * out);
    // The crown leans with the gusts; branch ends flutter on their own.
    let phase = dot(origin.xz, vec2<f32>(0.173, 0.091));
    let along = dot(world.xz, dir);
    let gust = 0.5 + 0.5 * sin(along * 0.05 - t * 0.9 + phase) * sway.wind.w;
    let flutter = sin(t * 2.3 + phase * 7.0 + length(local.xz) * 1.7) * 0.25 * out;
    let push = sway.wind.z * (0.7 * gust + flutter);
    return vec3<f32>(dir.x, 0.0, dir.y) * push * 0.35 * weight
        + parted(world, sway.hand_a, out)
        + parted(world, sway.hand_b, out)
        + shouldered(world, sway.robot_a, out)
        + shouldered(world, sway.robot_b, out)
        + shouldered(world, sway.robot_c, out)
        + shouldered(world, sway.robot_d, out);
}

@vertex
fn vertex(
    vertex: Vertex,
#ifdef CARD_CENTRE
    @location(8) card_centre: vec3<f32>,
#endif
) -> VertexOutput {
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
    out.world_position = vec4<f32>(
        out.world_position.xyz + offset(out.world_position.xyz, vertex.position, world_from_local[3].xyz),
        1.0
    );
#ifdef CARD_CENTRE
    // The card's centre, swayed as its corners are, and how much of the
    // card is left at its distance from the eye.
    var centre = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(card_centre, 1.0)
    ).xyz;
    centre = centre + offset(centre, card_centre, world_from_local[3].xyz);
    let left = smoothstep(1.2, 2.0, distance(centre, view.world_position.xyz));
    out.world_position = vec4<f32>(mix(centre, out.world_position.xyz, left), 1.0);
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
