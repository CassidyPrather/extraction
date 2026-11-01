//! The sky in every reflection. The game had no environment light: Bevy's
//! ambient is one flat colour from every direction, so a metal (all
//! specular) reflected the same grey whichever way it faced. Blued steel
//! came out near black and a curved slide had nothing to grade across.
//!
//! No cubemaps. The specular half of Bevy's ambient term
//! (`bevy_pbr::ambient::ambient_light`) is patched to read an analytic
//! sky in the reflected direction, the one you can see:
//! - The horizon is the fog's colour, which is the sky dome's horizon and
//!   follows the valley, the hour, the weather and the Surge.
//! - The zenith is the dome's darker top ([`ZENITH`] of the horizon).
//! - Below the horizon is the ground's bounce ([`GROUND`] of the ambient).
//!
//! A rough surface reads it blurred toward the half-sky its normal sees.
//! The diffuse half is untouched: the flat ambient every gauge is tuned to.
//!
//! Indoors it follows the room (`grade::dim_indoors`). The fog and ambient
//! it is made from are the room's, dimmed, and past the room the shader
//! gives the sky's light back (`render::fog`'s `extraction_room`). Near
//! the eye in a room, metal reflects the room's walls ([`WALLS`] of its
//! ambient), not open sky.
//!
//! A source edit against a pinned Bevy, like `render::fog`: if the anchors
//! stop matching, it warns once and the ambient stays flat.

use bevy::prelude::*;
use bevy::shader::{Shader, ShaderImport, Source};

pub struct SkyLightPlugin;

impl Plugin for SkyLightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, patch_ambient);
    }
}

/// The dome's zenith against its horizon, per channel (linear): the
/// default sky's (`world::sky::Weather`), darker and bluer overhead.
pub const ZENITH: [f32; 3] = [0.44, 0.47, 0.54];
/// The ground's light against the sky's ambient: a share of it bounced
/// back up off earth and grass (albedo about a quarter).
pub const GROUND: f32 = 0.25;
/// A room's walls against its ambient, for metal near the eye indoors.
pub const WALLS: f32 = 0.35;

const MARKER: &str = "// EXTRACTION_SKY_LIGHT";
const IMPORT_ANCHOR: &str = "    mesh_view_bindings::lights,\n";
const IMPORT_PATCHED: &str = "    mesh_view_bindings::{lights, fog, view},\n";
const RETURN_ANCHOR: &str = "    return (diffuse_ambient + specular_ambient * specular_occlusion) * lights.ambient_color.rgb * occlusion;\n";

fn sky_wgsl() -> String {
    format!(
        r#"{MARKER}
// (The fog's binding exists only with DISTANCE_FOG: so do these.)
#ifdef DISTANCE_FOG
const XS_ZENITH: vec3<f32> = vec3<f32>({z0:.3}, {z1:.3}, {z2:.3});
const XS_GROUND: f32 = {GROUND:.3};
const XS_WALLS: f32 = {WALLS:.3};

// The sky seen along `d`, in the lights' units (the fog's colour is the
// dome's horizon as drawn, after the exposure).
fn extraction_sky(d: vec3<f32>, horizon: vec3<f32>, ground: vec3<f32>) -> vec3<f32> {{
    let up = mix(horizon, horizon * XS_ZENITH, sqrt(clamp(d.y, 0.0, 1.0)));
    return mix(ground, up, smoothstep(-0.12, 0.0, d.y));
}}

// What a surface's specular reflects: the sky along the reflection,
// toward the half-sky its normal sees as it roughens; near the eye in a
// room, the room's walls.
fn extraction_env(at: vec3<f32>, N: vec3<f32>, V: vec3<f32>, roughness: f32) -> vec3<f32> {{
    let horizon = fog.base_color.rgb / max(view.exposure, 1e-9);
    let ground = lights.ambient_color.rgb * XS_GROUND;
    let a = roughness * roughness;
    let R = reflect(-V, N);
    let sharp = extraction_sky(normalize(mix(R, N, a)), horizon, ground);
    let sky_mean = horizon * (vec3<f32>(1.0) + XS_ZENITH) * 0.5;
    let wide = mix(ground, sky_mean, 0.5 + 0.5 * N.y);
    var env = mix(sharp, wide, a);
    // The room the eye is in (`grade::room_code` on the ambient's alpha).
    let code = lights.ambient_color.a;
    if (code < 0.0) {{
        let reach = floor(-code / 4.0);
        let inside = 1.0 - smoothstep(reach, reach + 3.0, distance(at, view.world_position.xyz));
        env = mix(env, lights.ambient_color.rgb * XS_WALLS, inside * 0.9);
    }}
    return env;
}}
#endif

"#,
        z0 = ZENITH[0],
        z1 = ZENITH[1],
        z2 = ZENITH[2],
    )
}

fn return_patched() -> String {
    format!(
        "#ifdef DISTANCE_FOG\n    let env = extraction_env(world_position.xyz, world_normal, V, perceptual_roughness);\n    return (diffuse_ambient * lights.ambient_color.rgb + specular_ambient * specular_occlusion * env) * occlusion;\n#else\n{RETURN_ANCHOR}#endif\n"
    )
}

/// `bevy_pbr::ambient`'s source with the sky in its specular, or None if
/// its anchors aren't there (or it's patched already).
fn patch(src: &str) -> Option<String> {
    if src.contains(MARKER) || !src.contains(IMPORT_ANCHOR) || !src.contains(RETURN_ANCHOR) {
        return None;
    }
    let fn_at = src.find("fn ambient_light(")?;
    let mut out = String::with_capacity(src.len() + 2048);
    out.push_str(&src[..fn_at]);
    out.push_str(&sky_wgsl());
    out.push_str(&src[fn_at..]);
    Some(out.replacen(IMPORT_ANCHOR, IMPORT_PATCHED, 1).replacen(
        RETURN_ANCHOR,
        &return_patched(),
        1,
    ))
}

fn patch_ambient(
    mut events: MessageReader<AssetEvent<Shader>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut warned: Local<bool>,
) {
    let wanted = ShaderImport::Custom("bevy_pbr::ambient".into());
    for ev in events.read() {
        let (AssetEvent::Added { id } | AssetEvent::Modified { id }) = ev else {
            continue;
        };
        let Some(shader) = shaders.get(*id) else {
            continue;
        };
        if shader.import_path != wanted {
            continue;
        }
        let Source::Wgsl(src) = &shader.source else {
            continue;
        };
        if src.contains(MARKER) {
            continue;
        }
        let Some(new_src) = patch(src) else {
            if !*warned {
                warn!("sky light: pbr_ambient.wgsl anchors not found; the ambient stays flat");
                *warned = true;
            }
            continue;
        };
        let mut replacement = Shader::from_wgsl(new_src, shader.path.clone());
        replacement.shader_defs = shader.shader_defs.clone();
        replacement.file_dependencies = shader.file_dependencies.clone();
        replacement.additional_imports = shader.additional_imports.clone();
        if let Some(mut s) = shaders.get_mut(*id) {
            *s = replacement;
        }
    }
}

/// The CPU twin of `extraction_sky` (for the tests and the gauges).
pub fn sky(d: Vec3, horizon: Vec3, ground: Vec3) -> Vec3 {
    let s = |e0: f32, e1: f32, x: f32| {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let up = horizon.lerp(horizon * Vec3::from(ZENITH), d.y.clamp(0.0, 1.0).sqrt());
    ground.lerp(up, s(-0.12, 0.0, d.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bevy 0.19.1's `pbr_ambient.wgsl`, as far as the patch reads it.
    const SAMPLE: &str = "#define_import_path bevy_pbr::ambient\n\n#import bevy_pbr::{\n    lighting::{EnvBRDFApprox, F_AB},\n    mesh_view_bindings::lights,\n}\n\nfn ambient_light(\n    world_position: vec4<f32>,\n) -> vec3<f32> {\n    return (diffuse_ambient + specular_ambient * specular_occlusion) * lights.ambient_color.rgb * occlusion;\n}\n";

    #[test]
    fn the_patch_takes_once_and_keeps_the_flat_ambient_without_fog() {
        let out = patch(SAMPLE).expect("anchors");
        assert!(out.contains(MARKER) && out.contains(IMPORT_PATCHED));
        assert!(out.find("fn extraction_env").unwrap() < out.find("fn ambient_light").unwrap());
        // A view without fog (the impostors' capture) keeps Bevy's own line.
        assert!(out.contains(&format!("#else\n{RETURN_ANCHOR}#endif")));
        assert!(patch(&out).is_none(), "patched twice");
    }

    /// Against the pinned Bevy's real source: the anchors are there.
    #[test]
    fn the_anchors_are_in_bevys_ambient() {
        let home = std::env::var("CARGO_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".cargo")
            });
        let Some(src) = std::fs::read_dir(home.join("registry/src"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|d| d.path().join("bevy_pbr-0.19.1/src/render/pbr_ambient.wgsl"))
            .find_map(|p| std::fs::read_to_string(p).ok())
        else {
            eprintln!("bevy_pbr 0.19.1 source not found: skipped");
            return;
        };
        assert!(
            patch(&src).is_some(),
            "pbr_ambient.wgsl changed: re-anchor the sky light"
        );
    }

    /// Metal reflects the sky you see. Under the default sky at the
    /// game's exposure, the reflection is graded: the horizon (the dome's
    /// own, through the fog's colour) half as bright again as the zenith, the
    /// ground darker still. Its level is that of the flat ambient it
    /// replaces (a half to one and a half of it, sideways), so metal
    /// neither went dark nor glows.
    #[test]
    fn metal_reflects_a_graded_sky_at_the_ambients_level() {
        let w = crate::world::sky::Weather::default();
        let exposure = 1.0 / (1.2 * 2f32.powf(10.6));
        let horizon = w.horizon.to_linear().to_vec3() / exposure;
        let ambient = w.ambient_color.to_linear().to_vec3() * w.ambient;
        let ground = ambient * GROUND;
        let luma = |v: Vec3| v.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let side = luma(sky(Vec3::new(1.0, 0.02, 0.0).normalize(), horizon, ground));
        let top = luma(sky(Vec3::Y, horizon, ground));
        let down = luma(sky(-Vec3::Y, horizon, ground));
        assert!(side > 1.5 * top, "horizon {side} vs zenith {top}");
        assert!(top > down, "zenith {top} vs ground {down}");
        let flat = luma(ambient);
        assert!(
            (0.5..1.5).contains(&(side / flat)),
            "horizon {side} against the flat ambient {flat}"
        );
    }
}
