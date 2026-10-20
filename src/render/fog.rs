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

const WET_WGSL: &str = r#"
fn extraction_wet(p: pbr_types::PbrInput) -> pbr_types::PbrInput {
    var o = p;
    let alpha_mode = p.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if (alpha_mode != pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE) {
        return o;
    }
    o.material.base_color.a = 1.0;
#ifdef DISTANCE_FOG
    let w = clamp((1.0 - view_bindings::fog.base_color.a) * 500.0, 0.0, 1.0);
#else
    // A view with no fog (the impostors' capture) has no weather either.
    let w = 0.0;
#endif
    if (w < 0.002) {
        return o;
    }
    // Rain falls on what faces up; walls take the run-off, a little.
    let open = clamp(p.material.base_color.a, 0.0, 1.0);
    let up = clamp(p.world_normal.y, 0.0, 1.0);
    let k = w * open * (0.3 + 0.7 * up);
    // Darker (soaked), and glossy, metal less so.
    let soak = mix(0.62, 0.85, p.material.metallic);
    o.material.base_color = vec4<f32>(p.material.base_color.rgb * mix(1.0, soak, k), 1.0);
    o.material.perceptual_roughness = mix(
        p.material.perceptual_roughness,
        min(p.material.perceptual_roughness, 0.32),
        k * 0.85,
    );
    return o;
}
"#;

fn height_fog_wgsl(f: &HeightFog) -> String {
    format!(
        r#"{MARKER}
const HF_DENSITY: f32 = {density:.6};
const HF_BASE: f32 = {base:.3};
const HF_FALLOFF: f32 = {falloff:.5};
const HF_PATCHY: f32 = {patchy:.3};
const HF_DRIFT: f32 = {drift:.4};
const HF_CAP: f32 = {cap:.6};
const HF_CAP_BASE: f32 = {cap_base:.3};
const HF_CAP_RISE: f32 = {cap_rise:.5};

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
    let fog_color = fog_params.base_color.rgb * 1.06
        + scattering * fog_params.directional_light_color.rgb * fog_params.directional_light_color.a;
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
