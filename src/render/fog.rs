//! Ground fog: analytic exponential height fog, layered on the distance fog
//! for every PBR surface (StandardMaterial, glTF models, and our own
//! terrain and water shaders, which all finish in Bevy's
//! `main_pass_post_lighting_processing`).
//!
//! Bevy has no hook for this, so we patch its `bevy_pbr::pbr_functions`
//! shader source when it loads: the exponential-squared branch of
//! `apply_fog` gets our height term applied on top. Density thins
//! exponentially with altitude above [`HeightFog::base`], so fog pools in the
//! creek valley and low fields and drifts in slow banks (a little value
//! noise over the ground, scrolled by time). The integral along the view
//! ray has a closed form, so it costs a handful of ALU per pixel — cheap
//! enough for VR, unlike ray-marched volumes.
//!
//! The patch is a source edit against a pinned Bevy version; if the anchor
//! strings stop matching, it logs a warning and the game falls back to
//! plain distance fog.

use bevy::prelude::*;
use bevy::shader::{Shader, ShaderImport, Source};

pub struct HeightFogPlugin;

impl Plugin for HeightFogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeightFog>()
            .add_systems(Update, patch_pbr_functions);
    }
}

/// Ground-fog parameters. Changing them re-patches the shader (a pipeline
/// recompile), so this is for weather changes, not per-frame animation.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct HeightFog {
    /// Extinction per metre at `base` height.
    pub density: f32,
    /// World height (m) where `density` applies.
    pub base: f32,
    /// How fast it thins with height (1/m): 1/falloff is the e-folding height.
    pub falloff: f32,
    /// 0 = uniform layer, 1 = strongly patchy banks.
    pub patchiness: f32,
    /// Bank drift speed (m/s).
    pub drift: f32,
    /// Cloud on the tops: extinction per metre at `cap_base`, thickening
    /// upward (0: none). A valley's high ground goes into the cloud.
    pub cap_density: f32,
    /// World height (m) where `cap_density` applies.
    pub cap_base: f32,
    /// How fast the cap thickens going up (1/m).
    pub cap_rise: f32,
    /// Where snow can lie (m; `world::snow::snow_line`), far overhead in a
    /// valley without one.
    pub snow_line: f32,
}

impl Default for HeightFog {
    fn default() -> Self {
        Self {
            density: 0.011,
            base: 16.0,
            falloff: 0.2,
            patchiness: 0.75,
            drift: 0.6,
            cap_density: 0.0,
            cap_base: 100.0,
            cap_rise: 0.08,
            snow_line: NO_SNOW,
        }
    }
}

const MARKER: &str = "// EXTRACTION_HEIGHT_FOG";
const CALL_ANCHOR: &str =
    "return bevy_pbr::fog::exponential_squared_fog(fog_params, input_color, distance, scattering);";
const FN_ANCHOR: &str = "fn apply_fog(";

// Wet surfaces: every PBR surface's lighting first goes through
// `extraction_wet`, which reads how wet the world is from the fog colour's
// alpha (`1 - wetness * WET_CODE`: a fog weaker by at most 0.2%, which no
// one sees) and the surface's sky openness from its own alpha (opaque
// surfaces ignore alpha: buildings bake it, `world::ao`; everything else
// is 1, open; the terrain sets 0 and wets itself). Bevy forces an opaque
// surface's alpha to 1 in `alpha_discard`; the patch keeps it until the
// lighting has read it, and then puts the 1 back.
const LIGHT_ANCHOR: &str =
    "fn apply_pbr_lighting(\n    in: pbr_types::PbrInput,\n) -> vec4<f32> {\n";
const LIGHT_PATCHED: &str = "fn apply_pbr_lighting(\n    in_dry: pbr_types::PbrInput,\n) -> vec4<f32> {\n    let in = extraction_wet(in_dry); // EXTRACTION_WET\n";

/// The patched lighting entry, with `extraction_wet` just above it (where
/// it's in scope whatever the shader defs; next to `apply_fog` it wasn't).
fn light_patched() -> String {
    format!("{WET_WGSL}{LIGHT_PATCHED}")
}
const OPAQUE_ANCHOR: &str = "        // NOTE: If rendering as opaque, alpha should be ignored so set to 1.0\n        color.a = 1.0;\n";
const OPAQUE_PATCHED: &str = "        // NOTE: If rendering as opaque, alpha should be ignored so set to 1.0\n        // EXTRACTION_WET: kept (the sky openness) until the lighting reads it.\n        color.a = clamp(color.a, 0.0, 1.0);\n";
/// Wetness rides the fog colour's alpha as `1 - wetness * WET_CODE`.
pub const WET_CODE: f32 = 0.002;
/// Lying snow rides the fog's lobe exponent: a whole number, plus
/// `snow * SNOW_CODE` (a lobe a hundredth sharper at most: unseen).
pub const SNOW_CODE: f32 = 0.01;
/// A snow line no ground reaches.
pub const NO_SNOW: f32 = 1.0e5;

const WET_WGSL: &str = r#"
// The room the eye is in (`render::grade::dim_indoors`): how far its walls
// cut the sky's light (1 out of doors) and how far it reaches (m), off the
// ambient's alpha, which the lighting never reads.
fn extraction_room() -> vec2<f32> {
    let a = view_bindings::lights.ambient_color.a;
    if (a >= 0.0) {
        return vec2<f32>(1.0, 0.0);
    }
    let reach = floor(-a / 4.0);
    return vec2<f32>(max(-a - reach * 4.0, 0.05), reach);
}

// How much of the room's cut past it (`at`, from the eye) is given back:
// the outdoors seen from a room keeps the sky's light.
fn extraction_outdoors(room: vec2<f32>, at: vec3<f32>) -> f32 {
    return smoothstep(room.y, room.y + 3.0, distance(at, view_bindings::view.world_position.xyz));
}

fn extraction_wet(p: pbr_types::PbrInput) -> pbr_types::PbrInput {
    var o = p;
    let alpha_mode = p.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    let opaque = alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE;
    let mask = alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_MASK
        || alpha_mode == pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_ALPHA_TO_COVERAGE;
    if (!opaque && !mask) {
        return o;
    }
#ifdef DISTANCE_FOG
    let w = select(0.0, clamp((1.0 - view_bindings::fog.base_color.a) * 500.0, 0.0, 1.0), opaque);
    let snow = clamp(fract(view_bindings::fog.directional_light_exponent + 1e-5) * 100.0 - 1e-3, 0.0, 1.0);
#else
    // A view with no fog (the impostors' capture) has no weather either.
    let w = 0.0;
    let snow = 0.0;
#endif
    // A masked surface's alpha is its cutout (a tree's cards): open sky.
    let open = select(1.0, clamp(p.material.base_color.a, 0.0, 1.0), opaque);
    if (opaque) {
        o.material.base_color.a = 1.0;
    }
    // From a room, what's under the sky past its walls keeps the sky's
    // light (the ambient is the room's: `grade::dim_indoors`). What's under
    // a roof stays the room's dark, this one's or the next house's.
    let room = extraction_room();
    if (room.x < 0.999) {
        // (The terrain's alpha is below zero: under the sky, wet already.)
        let sky = select(open, 1.0, opaque && p.material.base_color.a < -0.5);
        let back = extraction_outdoors(room, p.world_position.xyz) * smoothstep(0.8, 0.95, sky);
        o.diffuse_occlusion = p.diffuse_occlusion * mix(1.0, 1.0 / room.x, back);
    }
#ifdef DISTANCE_FOG
    if (snow > 0.002 && HF_SNOW_LINE < 1.0e4) {
        o = extraction_snow(o, snow, open, mask);
    }
#endif
    if (w < 0.002) {
        return o;
    }
    // Rain falls on what faces up; walls take the run-off, a little.
    let up = clamp(p.world_normal.y, 0.0, 1.0);
    let k = w * open * (0.3 + 0.7 * up);
    // Darker (soaked), and glossy, metal less so.
    let soak = mix(0.62, 0.85, p.material.metallic);
    // (Not where snow lies on it: snow is matt.)
    o.material.base_color = vec4<f32>(o.material.base_color.rgb * mix(1.0, soak, k), o.material.base_color.a);
    o.material.perceptual_roughness = mix(
        o.material.perceptual_roughness,
        min(o.material.perceptual_roughness, 0.32),
        k * 0.85,
    );
    return o;
}

// (The height fog's noise and the valley's snow line live with
// `apply_fog`, under DISTANCE_FOG: so does this.)
#ifdef DISTANCE_FOG
// Snow (`world::snow`): it lies on what faces up, out in the open, above
// the valley's line (a few metres' ragged edge), thin first in patches and
// then all over; on a slope only where it isn't steep. Matt and white.
fn extraction_snow(p: pbr_types::PbrInput, snow: f32, open: f32, cards: bool) -> pbr_types::PbrInput {
    var o = p;
    let wp = p.world_position.xyz;
    let edge = (hf_noise(wp.xz * 0.06) - 0.5) * 7.0;
    let above = smoothstep(HF_SNOW_LINE - 3.0, HF_SNOW_LINE + 4.0, wp.y + edge);
    // Cards (foliage, ivy) have rounded normals, up over a whole crown:
    // only their tops take it, and never all over (the green shows under
    // the load; white all through, a spruce was a ghost of one).
    let flat = select(
        smoothstep(0.35, 0.8, p.world_normal.y),
        smoothstep(0.6, 0.95, p.world_normal.y) * 0.45,
        cards,
    );
    let patches = hf_noise(wp.xz * 1.3) * 0.6 + hf_noise(wp.xz * 0.21) * 0.4;
    // (Steepness scales it: subtracted, a wall still took snow at full.)
    let lie = clamp((snow * 1.6 - patches * 0.7) * 3.0, 0.0, 1.0) * flat;
    let s = open * above * lie;
    if (s <= 0.0) {
        return o;
    }
    o.material.base_color = vec4<f32>(
        mix(p.material.base_color.rgb, vec3<f32>(0.74, 0.76, 0.8), s),
        p.material.base_color.a,
    );
    o.material.perceptual_roughness = mix(p.material.perceptual_roughness, 0.75, s);
    o.material.metallic = mix(p.material.metallic, 0.0, s);
    o.material.emissive = p.material.emissive * (1.0 - s);
    // Snow smooths over the surface's own relief.
    o.N = normalize(mix(p.N, p.world_normal, s * 0.6));
    return o;
}
#endif
"#;

fn height_fog_wgsl(f: &HeightFog) -> String {
    format!(
        r#"{MARKER}
const HF_DENSITY: f32 = {density:.6};
const HF_AIR: f32 = 0.6;
const HF_BASE: f32 = {base:.3};
const HF_FALLOFF: f32 = {falloff:.5};
const HF_PATCHY: f32 = {patchy:.3};
const HF_DRIFT: f32 = {drift:.4};
const HF_CAP: f32 = {cap:.6};
const HF_CAP_BASE: f32 = {cap_base:.3};
const HF_CAP_RISE: f32 = {cap_rise:.5};
const HF_SNOW_LINE: f32 = {snow_line:.2};

fn hf_hash(p: vec2<f32>) -> f32 {{
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}}

fn hf_noise(p: vec2<f32>) -> f32 {{
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hf_hash(i);
    let b = hf_hash(i + vec2<f32>(1.0, 0.0));
    let c = hf_hash(i + vec2<f32>(0.0, 1.0));
    let d = hf_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}}

fn extraction_height_fog(
    fog_params: mesh_view_types::Fog,
    color: vec4<f32>,
    frag: vec3<f32>,
    cam: vec3<f32>,
    scattering: vec3<f32>,
) -> vec4<f32> {{
    let ray = frag - cam;
    let len = length(ray);
    // Banks: low-frequency noise over the ground, sampled along the ray's
    // far half so near and far fog breathe differently.
    let t = globals.time * HF_DRIFT;
    let at = mix(cam.xz, frag.xz, 0.65);
    let n = hf_noise(at * 0.011 + vec2<f32>(t * 0.011, t * 0.004)) * 0.7
        + hf_noise(at * 0.037 - vec2<f32>(t * 0.02, -t * 0.013)) * 0.3;
    let density = HF_DENSITY * mix(1.0, n * 2.0, HF_PATCHY);
    // Optical depth of density * exp(-falloff * (y - base)) along the ray.
    let start = density * exp(clamp(-HF_FALLOFF * (cam.y - HF_BASE), -30.0, 8.0));
    let k = clamp(HF_FALLOFF * ray.y, -30.0, 30.0);
    var depth = start * len;
    if (abs(k) > 1e-3) {{
        depth = start * len * (1.0 - exp(-k)) / k;
    }}
    // One atmosphere: a ray climbing out of the ground mist still crosses
    // at least HF_AIR of the air at the eye. Without it a tall thing's top
    // (Marrow's chimney, 30 m up) came out of the fog crisp while the trees
    // round its foot were half gone: it read as drawn through the fog.
    depth = max(depth, start * len * HF_AIR);
    // The cap: density * exp(rise * (y - cap base)), the same integral
    // the other way up (a few ALU; nothing when the valley has none).
    if (HF_CAP > 0.0) {{
        let cs = HF_CAP * exp(clamp(HF_CAP_RISE * (cam.y - HF_CAP_BASE), -30.0, 8.0));
        let ck = clamp(HF_CAP_RISE * ray.y, -30.0, 30.0);
        var cd = cs * len;
        if (abs(ck) > 1e-3) {{
            cd = cs * len * (exp(ck) - 1.0) / ck;
        }}
        depth = depth + cd;
    }}
    let amount = 1.0 - exp(-max(depth, 0.0));
    // The air in the room is the room's (dimmed with it); the air out past
    // it, the sky's.
    let room = extraction_room();
    let lit = mix(1.0, 1.0 / room.x, extraction_outdoors(room, frag));
    let fog_color = (fog_params.base_color.rgb * 1.06
        + scattering * fog_params.directional_light_color.rgb * fog_params.directional_light_color.a) * lit;
    return vec4<f32>(mix(color.rgb, fog_color, amount), color.a);
}}

"#,
        density = f.density,
        base = f.base,
        falloff = f.falloff,
        patchy = f.patchiness.clamp(0.0, 1.0),
        drift = f.drift,
        cap = f.cap_density.max(0.0),
        cap_base = f.cap_base,
        cap_rise = f.cap_rise,
        snow_line = f.snow_line,
    )
}

/// Strips an earlier patch, so parameters can change.
fn unpatched(src: &str) -> String {
    let Some(start) = src.find(MARKER) else {
        return src.to_owned();
    };
    let end = src[start..]
        .find(FN_ANCHOR)
        .map_or(src.len(), |e| start + e);
    let mut out = String::with_capacity(src.len());
    out.push_str(&src[..start]);
    out.push_str(&src[end..]);
    out.replace(&patched_call(), CALL_ANCHOR)
        .replace(&light_patched(), LIGHT_ANCHOR)
        .replace(OPAQUE_PATCHED, OPAQUE_ANCHOR)
}

fn patched_call() -> String {
    "return extraction_height_fog(fog_params, bevy_pbr::fog::exponential_squared_fog(fog_params, input_color, distance, scattering), fragment_world_position, view_world_position, scattering);".to_owned()
}

fn patch(src: &str, fog: &HeightFog) -> Option<String> {
    let base = unpatched(src);
    if !base.contains(CALL_ANCHOR) || !base.contains(FN_ANCHOR) {
        return None;
    }
    // Wet surfaces only where both anchors are there (else dry, as before).
    let wet = base.contains(LIGHT_ANCHOR) && base.contains(OPAQUE_ANCHOR);
    let with_fns = base.replacen(
        FN_ANCHOR,
        &format!("{}{FN_ANCHOR}", height_fog_wgsl(fog)),
        1,
    );
    let mut out = with_fns.replacen(CALL_ANCHOR, &patched_call(), 1);
    if wet {
        out = out.replacen(LIGHT_ANCHOR, &light_patched(), 1).replacen(
            OPAQUE_ANCHOR,
            OPAQUE_PATCHED,
            1,
        );
    }
    Some(out)
}

fn patch_pbr_functions(
    mut events: MessageReader<AssetEvent<Shader>>,
    mut shaders: ResMut<Assets<Shader>>,
    fog: Res<HeightFog>,
    mut target: Local<Option<AssetId<Shader>>>,
    mut applied: Local<Option<HeightFog>>,
    mut warned: Local<bool>,
) {
    let wanted = ShaderImport::Custom("bevy_pbr::pbr_functions".into());
    for ev in events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = ev {
            if shaders.get(*id).is_some_and(|s| s.import_path == wanted) {
                *target = Some(*id);
                // Our own edit fires Modified too; only re-patch if the
                // source lost its patch (a reload).
                let fresh = shaders.get(*id).is_some_and(|s| match &s.source {
                    Source::Wgsl(src) => !src.contains(MARKER),
                    _ => false,
                });
                if fresh {
                    *applied = None;
                }
            }
        }
    }
    let Some(id) = *target else {
        return;
    };
    if applied.as_ref() == Some(&*fog) {
        return;
    }
    let Some(shader) = shaders.get(id) else {
        return;
    };
    let Source::Wgsl(src) = &shader.source else {
        return;
    };
    let Some(new_src) = patch(src, &fog) else {
        if !*warned {
            warn!("height fog: pbr_functions.wgsl anchors not found; distance fog only");
            *warned = true;
        }
        *applied = Some(*fog);
        return;
    };
    let path = shader.path.clone();
    let mut replacement = Shader::from_wgsl(new_src, path);
    replacement.shader_defs = shader.shader_defs.clone();
    replacement.file_dependencies = shader.file_dependencies.clone();
    replacement.additional_imports = shader.additional_imports.clone();
    if let Some(mut s) = shaders.get_mut(id) {
        *s = replacement;
    }
    *applied = Some(*fog);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every material out in the world takes the fog: the world reads as
    /// one atmosphere (a building once stood crisp in the fog, playtest
    /// 2026-10-28). A material that turns fog off must be additive or
    /// multiplied (fog's colour added onto, or multiplied into, everything
    /// behind it is wrong), or say why within 20 lines above (a fog of its
    /// own, an overlay at the eye); a fragment shader of our own must fog
    /// or say why not.
    #[test]
    fn every_material_takes_the_fog_or_says_why() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut bad = Vec::new();
        let mut stack = vec![root.join("src")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap();
                let lines: Vec<&str> = text.lines().collect();
                for (n, l) in lines.iter().enumerate() {
                    if !l.contains("fog_enabled: false") || l.trim_start().starts_with("//") {
                        continue;
                    }
                    // The material's own lines, and the 20 above it.
                    let from = n.saturating_sub(20);
                    let near = &lines[from..(n + 6).min(lines.len())];
                    let blended = near
                        .iter()
                        .any(|l| l.contains("AlphaMode::Add") || l.contains("AlphaMode::Multiply"));
                    let said = lines[from..n].iter().any(|l| {
                        let t = l.trim_start();
                        t.starts_with("//") && t.to_lowercase().contains("fog")
                    });
                    if !blended && !said {
                        bad.push(format!(
                            "{}:{}",
                            p.strip_prefix(root).unwrap().display(),
                            n + 1
                        ));
                    }
                }
            }
        }
        for e in std::fs::read_dir(root.join("assets/shaders"))
            .unwrap()
            .flatten()
        {
            let p = e.path();
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            if !text.contains("fn fragment") {
                continue;
            }
            let fogs = [
                "fog",
                "main_pass_post_lighting_processing",
                "apply_pbr_lighting",
            ]
            .iter()
            .any(|w| text.to_lowercase().contains(w));
            if !fogs {
                bad.push(p.strip_prefix(root).unwrap().display().to_string());
            }
        }
        assert!(bad.is_empty(), "fog off with no reason given: {bad:#?}");
    }

    const SAMPLE: &str = "fn other() {}\nfn apply_fog(a: f32) {\n    if x {\n        return bevy_pbr::fog::exponential_squared_fog(fog_params, input_color, distance, scattering);\n    }\n}\n";

    #[test]
    fn patch_round_trips() {
        let a = patch(SAMPLE, &HeightFog::default()).unwrap();
        assert!(a.contains(MARKER));
        assert!(a.contains("extraction_height_fog(fog_params"));
        assert_eq!(unpatched(&a), SAMPLE);
        let b = patch(
            &a,
            &HeightFog {
                density: 0.5,
                ..default()
            },
        )
        .unwrap();
        assert_eq!(b.matches(MARKER).count(), 1);
        assert!(b.contains("0.500000"));
    }

    #[test]
    fn wet_patch_round_trips() {
        let src = format!(
            "{SAMPLE}{LIGHT_ANCHOR}    return in.x;\n}}\nfn alpha_discard() {{\n{OPAQUE_ANCHOR}}}\n"
        );
        let a = patch(&src, &HeightFog::default()).unwrap();
        assert!(a.contains("let in = extraction_wet(in_dry);"));
        assert!(a.contains("fn extraction_wet("));
        assert!(!a.contains(OPAQUE_ANCHOR));
        assert_eq!(unpatched(&a), src);
        // Patched twice, still once.
        let b = patch(&a, &HeightFog::default()).unwrap();
        assert_eq!(b.matches("fn extraction_wet(").count(), 1);
        assert_eq!(unpatched(&b), src);
    }
}
