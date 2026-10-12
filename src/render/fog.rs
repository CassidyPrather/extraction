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
}

impl Default for HeightFog {
    fn default() -> Self {
        Self {
            density: 0.011,
            base: 16.0,
            falloff: 0.2,
            patchiness: 0.75,
            drift: 0.6,
        }
    }
}

const MARKER: &str = "// EXTRACTION_HEIGHT_FOG";
const CALL_ANCHOR: &str =
    "return bevy_pbr::fog::exponential_squared_fog(fog_params, input_color, distance, scattering);";
const FN_ANCHOR: &str = "fn apply_fog(";

fn height_fog_wgsl(f: &HeightFog) -> String {
    format!(
        r#"{MARKER}
const HF_DENSITY: f32 = {density:.6};
const HF_BASE: f32 = {base:.3};
const HF_FALLOFF: f32 = {falloff:.5};
const HF_PATCHY: f32 = {patchy:.3};
const HF_DRIFT: f32 = {drift:.4};

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
}

fn patched_call() -> String {
    "return extraction_height_fog(fog_params, bevy_pbr::fog::exponential_squared_fog(fog_params, input_color, distance, scattering), fragment_world_position, view_world_position, scattering);".to_owned()
}

fn patch(src: &str, fog: &HeightFog) -> Option<String> {
    let base = unpatched(src);
    if !base.contains(CALL_ANCHOR) || !base.contains(FN_ANCHOR) {
        return None;
    }
    let with_fns = base.replacen(
        FN_ANCHOR,
        &format!("{}{FN_ANCHOR}", height_fog_wgsl(fog)),
        1,
    );
    Some(with_fns.replacen(CALL_ANCHOR, &patched_call(), 1))
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
}
