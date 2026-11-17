// A searchlight's beam and the pool it throws on the ground
// (`robots::searchlight_look`). One material, two shapes:
//
// - the beam: the unit cone from `world::beams` (uv.x round it, uv.y along
//   it, 0 at the lamp), scaled to the throw. Lit haze, blended (not
//   added): by day a pale shaft over the ground and the trees behind it,
//   by night a bright one. It thins slowly along a long throw (a
//   searchlight's beam is tight), and goes soft at its edges.
// - the pool: a flat disc (uv 0..1 across it) where the beam lands: a pale
//   middle with a warning rim (orange-red), so what's being watched reads
//   as a threat on the ground by day as well as night.
//
// Fog: the beam is the lit air itself, so it isn't fogged; it thins with
// the distance to the eye instead, as the air takes it.

#import bevy_pbr::{
    forward_io::VertexOutput,
    mesh_view_bindings::{view, globals},
}

struct SearchParams {
    // rgb: the light; w: how strong (0 off).
    light: vec4<f32>,
    // rgb: the warning rim; w: how much of it.
    rim: vec4<f32>,
    // x: 0 beam, 1 pool; y: the beam's length (m); z: daylight 0..1;
    // w: how much the air shows (weather).
    shape: vec4<f32>,
    // xyz: the way it throws (world).
    axis: vec4<f32>,
    // xyz: the lamp (world); w: tan of the half-angle.
    origin: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> p: SearchParams;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let to_eye = view.world_position - in.world_position.xyz;
    let dist = length(to_eye);
    let v = to_eye / max(dist, 1e-3);
    let day = p.shape.z;
    // Far off the haze goes into the air like everything else.
    let far = 1.0 / (1.0 + dist * dist * 0.00004);
    if (p.shape.x < 0.5) {
        let n = normalize(in.world_normal);
        let along = in.uv.y;
        let square = abs(dot(n, v));
        // Looking down the throw (it's coming at you): all of it at once.
        let down = pow(abs(dot(v, p.axis.xyz)), 6.0);
        let path = smoothstep(0.0, 0.6, square) * (0.5 + 1.5 * down);
        let ends = smoothstep(0.0, 0.03, along) * (1.0 - smoothstep(0.75, 1.0, along));
        // A tight beam thins slowly: still a shaft where it lands.
        let thin = 1.0 - 0.55 * along;
        let near = smoothstep(0.5, 4.0, dist);
        // Standing in it: from inside, a beam is glare round you, not a
        // wall of haze across the view.
        let cam = view.world_position - p.origin.xyz;
        let c_along = dot(cam, p.axis.xyz);
        let c_off = length(cam - p.axis.xyz * c_along);
        let inside = select(1.0, 0.3, c_along > 0.0 && c_off < c_along * p.origin.w * 1.1);
        let k = clamp(path * ends * thin * near * far * inside * p.light.w * p.shape.w, 0.0, 1.0);
        // By day a pale, milky shaft (it must show over a lit field); by
        // night less of it is needed, and it's brighter.
        let a = k * mix(0.42, 0.22, day);
        // Display brightness, not scene light: exposure is the scene's,
        // and a beam by day must still read lighter than what's behind it.
        let glow = mix(2.4, 1.6, day);
        return vec4<f32>(p.light.rgb * glow, a);
    }
    // The pool: soft middle, a warning rim near the edge.
    let r = length(in.uv * 2.0 - vec2<f32>(1.0, 1.0));
    let middle = 1.0 - smoothstep(0.15, 0.85, r);
    let rim = smoothstep(0.62, 0.8, r) * (1.0 - smoothstep(0.86, 1.0, r));
    // A slow crawl round the rim: it's watching.
    let a_round = atan2(in.uv.y - 0.5, in.uv.x - 0.5);
    let crawl = 0.75 + 0.25 * sin(a_round * 6.0 - globals.time * 1.5);
    let lit = middle * mix(0.55, 0.35, day) * p.light.w;
    let warn = rim * crawl * p.rim.w * mix(0.9, 0.75, day);
    let alpha = clamp(lit + warn, 0.0, 0.95) * far;
    let col = (p.light.rgb * lit * mix(2.6, 1.7, day) + p.rim.rgb * warn * mix(3.0, 2.0, day))
        / max(lit + warn, 1e-3);
    return vec4<f32>(col, alpha);
}
