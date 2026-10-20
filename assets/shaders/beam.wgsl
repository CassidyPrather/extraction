// A torch's beam: the lit air in its cone (`world::beams`). One additive
// cone, both faces drawn. A ray through the cone crosses more lit air the
// more squarely it meets the surface, and the most looking down the
// throw, so that's the brightness; the light thins along it (spreading,
// and the air taking it), dust drifts in it close to, and rain streaks.
//
// uv.x runs round the cone, uv.y along it (0 at the lamp, 1 at the end of
// the unit cone; the mesh is scaled to the beam's length).

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, globals},
}

struct BeamParams {
    // rgb: the light's colour; w: the lit air's radiance (0 off).
    color: vec4<f32>,
    // x: how much the air scatters; y: rain; z: tan of the half-angle;
    // w: the beam's length (m).
    air: vec4<f32>,
    // xyz: the way it throws.
    axis: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> beam: BeamParams;

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let to_eye = view.world_position - in.world_position.xyz;
    let dist = length(to_eye);
    let v = to_eye / max(dist, 1e-3);
    let n = normalize(in.world_normal);
    let along = in.uv.y;
    // How much lit air the ray crosses here: squarely through the cone
    // a lot, along its edge none; looking down the throw, all of it.
    // Its silhouette always goes soft (a hard edge reads as a tube).
    let square = abs(dot(n, v));
    let down = pow(abs(dot(v, beam.axis.xyz)), 4.0);
    let path = smoothstep(0.0, 0.5, square) * (0.6 + 0.8 * down);
    // The light thins as it spreads and the air takes it; it starts just
    // past the lens, and stops softly where the beam ends.
    let len = beam.air.w;
    let d = along * len;
    let spread = 1.0 / (1.0 + d * d * 0.08);
    let ends = smoothstep(0.0, 0.08, along) * pow(1.0 - along, 2.0);
    // Close to the eye a face would fill the view: let it go.
    let near = smoothstep(0.25, 1.2, dist);
    var k = path * spread * ends * near * beam.air.x;
    // Dust: motes on a drifting grid, near only (far, they'd shimmer).
    let t = globals.time;
    let p = in.world_position.xyz * 9.0 + vec3<f32>(t * 0.07, -t * 0.11, t * 0.05);
    let c = floor(p);
    let h = hash3(c);
    let off = p - c - vec3<f32>(hash3(c + 7.0), hash3(c + 13.0), hash3(c + 29.0));
    let mote = select(0.0, 1.0 - smoothstep(0.05, 0.16, length(off)), h < 0.06);
    let twinkle = 0.6 + 0.4 * sin(t * 3.0 + h * 60.0);
    k = k + mote * twinkle * 3.0 * spread * ends * (1.0 - smoothstep(3.0, 7.0, dist)) * (0.3 + 0.7 * beam.air.x);
    // Rain: streaks falling through the light.
    let r = beam.air.y;
    if (r > 0.0) {
        let q = in.world_position.xyz * vec3<f32>(14.0, 0.9, 14.0) + vec3<f32>(0.0, t * 9.0, 0.0);
        let rc = floor(q);
        let rh = hash3(rc);
        let streak = select(0.0, smoothstep(0.5, 0.0, abs(fract(q.x) - 0.5) * 6.0), rh < 0.12 * r);
        k = k + streak * 1.5 * spread * ends * (1.0 - smoothstep(4.0, 9.0, dist));
    }
    // Alpha 0: the blend is premultiplied, so this adds.
    return vec4<f32>(beam.color.rgb * beam.color.w * k * 5.0 * view.exposure, 0.0);
}
