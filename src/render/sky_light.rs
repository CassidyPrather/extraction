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
//! - Below the horizon is far ground in the air (the horizon's colour),
//!   giving way to the ground's bounce straight down ([`GROUND`] of the
//!   ambient).
//!
//! A rough surface reads it blurred toward the half-sky its normal sees.
//! Rough enough ([`ROUGH_FLAT`]: foliage, bark, plaster, most of a forest's
//! fragments) it skips the sky and keeps the flat ambient, which is close
//! to the blurred sky there and costs nothing. The diffuse half is
//! untouched: the flat ambient every gauge is tuned to.
//!
//! Indoors it follows the room (`grade::dim_indoors`). The fog and ambient
//! it is made from are the room's, dimmed, and past the room the shader
//! gives the sky's light back (`render::fog`'s `extraction_room`). Near
//! the eye in a room, metal reflects the room's walls ([`WALLS`] of its
//! ambient), not open sky.
//!
//! `EXTRACTION_FLAT_AMBIENT=1` leaves Bevy's ambient as it was (the A/B
//! for frame cost, `scripts/bench.sh`).
//!
//! A source edit against a pinned Bevy, like `render::fog`: if the anchors
//! stop matching, it warns once and the ambient stays flat.
//!
//! On a Bevy upgrade, `tests::bevys_ambient_is_the_one_we_patch` fails
//! until this is redone:
//! 1. Read the new `bevy_pbr/src/render/pbr_ambient.wgsl`.
//! 2. If `ambient_light` still multiplies one flat `lights.ambient_color`
//!    into both its diffuse and specular halves, move the anchors
//!    (`IMPORT_ANCHOR`, `RETURN_ANCHOR`) to the new lines, and check that
//!    `fog`, `view` and `lights` are still in `mesh_view_bindings`, with
//!    `fog` under `DISTANCE_FOG`.
//! 3. If Bevy now has environment light of its own here (light probes on
//!    by default), weigh dropping this patch for it.
//! 4. Paste the new file into `UPSTREAM` and shoot
//!    `docs/shots/visual-design/envlight/shots.txt` against the old build.

use bevy::prelude::*;
use bevy::shader::{Shader, ShaderImport, Source};

pub struct SkyLightPlugin;

impl Plugin for SkyLightPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("EXTRACTION_FLAT_AMBIENT").is_some_and(|v| v != "0") {
            return;
        }
        app.add_systems(Update, patch_ambient);
    }
}

/// The sky overhead against its horizon, per channel (linear): darker and
/// a little bluer. (The dome's own zenith colour is darker still, but its
/// cloud lifts it: at 0.44-0.54 a sphere's top came out darker than the
/// sky behind it.)
pub const ZENITH: [f32; 3] = [0.68, 0.70, 0.76];
/// The ground's light against the sky's ambient: what earth and grass
/// (albedo about a fifth) give back of the sky's light and the sun's,
/// about as much again under cloud.
pub const GROUND: f32 = 0.4;
/// A room's walls against its ambient, for metal near the eye indoors.
pub const WALLS: f32 = 0.35;
/// From this perceptual roughness the sky fades into the flat ambient,
/// and from [`ROUGH_FLAT`] it isn't looked up at all.
pub const ROUGH_FADE: f32 = 0.55;
pub const ROUGH_FLAT: f32 = 0.7;
/// In a room, what's in your hands keeps some of the light the room cuts
/// (as a body's own bounce would give it): within [`NEAR_IN`] of the eye
/// the cut is given back by [`NEAR_SHARE`] of its stops, fading out by
/// [`NEAR_OUT`]. Out of doors (no room) nothing changes. On the Rime Fell
/// mill's ground floor the PM at 30 cm all but vanished against the room.
pub const NEAR_IN: f32 = 0.55;
pub const NEAR_OUT: f32 = 0.9;
pub const NEAR_SHARE: f32 = 0.75;
/// Under this much ambient (summed, cd/m²) there is no sky to reflect (deep
/// in the workings, `world::underground::go_dark`): not looked up.
pub const DARK: f32 = 0.5;

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
const XS_NEAR_IN: f32 = {NEAR_IN:.3};
const XS_NEAR_OUT: f32 = {NEAR_OUT:.3};
const XS_NEAR_SHARE: f32 = {NEAR_SHARE:.3};

// What's in your hands in a room keeps some of the light the room cuts
// (`NEAR_SHARE` of its stops): 1 out of doors, or past arm's length.
fn extraction_near(at: vec3<f32>) -> f32 {{
    let code = lights.ambient_color.a;
    if (code >= 0.0) {{
        return 1.0;
    }}
    let reach = floor(-code / 4.0);
    let k = max(-code - reach * 4.0, 0.05);
    let near = 1.0 - smoothstep(XS_NEAR_IN, XS_NEAR_OUT, distance(at, view.world_position.xyz));
    return mix(1.0, pow(1.0 / k, XS_NEAR_SHARE), near);
}}

// The sky seen along `d`, in the lights' units (the fog's colour is the
// dome's horizon as drawn, after the exposure).
fn extraction_sky(d: vec3<f32>, horizon: vec3<f32>, ground: vec3<f32>) -> vec3<f32> {{
    let up = mix(horizon, horizon * XS_ZENITH, sqrt(clamp(d.y, 0.0, 1.0)));
    // Just under the horizon is far ground, in the air: the horizon's
    // colour; straight down, the ground at your feet.
    let down = mix(horizon, ground, smoothstep(0.0, 0.5, -d.y));
    return select(down, up, d.y >= 0.0);
}}

// What a surface's specular reflects: the sky along the reflection,
// toward the half-sky its normal sees as it roughens; near the eye in a
// room, the room's walls.
fn extraction_env(at: vec3<f32>, N: vec3<f32>, V: vec3<f32>, roughness: f32) -> vec3<f32> {{
    // The room the eye is in (`grade::room_code` on the ambient's alpha):
    // inside it, the walls, and no sky to look up at all.
    let walls = lights.ambient_color.rgb * XS_WALLS;
    var inside = 0.0;
    let code = lights.ambient_color.a;
    if (code < 0.0) {{
        let reach = floor(-code / 4.0);
        inside = (1.0 - smoothstep(reach, reach + 3.0, distance(at, view.world_position.xyz))) * 0.9;
        if (inside >= 0.899) {{
            return walls;
        }}
    }}
    let horizon = fog.base_color.rgb / max(view.exposure, 1e-9);
    let ground = lights.ambient_color.rgb * XS_GROUND;
    let a = roughness * roughness;
    let R = reflect(-V, N);
    let sharp = extraction_sky(normalize(mix(R, N, a)), horizon, ground);
    let sky_mean = horizon * (vec3<f32>(1.0) + XS_ZENITH) * 0.5;
    let wide = mix(ground, sky_mean, 0.5 + 0.5 * N.y);
    return mix(mix(sharp, wide, a), walls, inside);
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
        "#ifdef DISTANCE_FOG\n    var env = lights.ambient_color.rgb;\n    if (perceptual_roughness < {ROUGH_FLAT:.3} && dot(env, vec3<f32>(1.0)) > {DARK:.3}) {{\n        env = mix(extraction_env(world_position.xyz, world_normal, V, perceptual_roughness), env, smoothstep({ROUGH_FADE:.3}, {ROUGH_FLAT:.3}, perceptual_roughness));\n    }}\n    return (diffuse_ambient * lights.ambient_color.rgb + specular_ambient * specular_occlusion * env) * occlusion * extraction_near(world_position.xyz);\n#else\n{RETURN_ANCHOR}#endif\n"
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

/// The CPU twin of `extraction_near`: the ambient's multiplier `dist` m
/// from the eye, in a room cutting it to `k` (None: out of doors).
pub fn near(dist: f32, k: Option<f32>) -> f32 {
    let Some(k) = k else { return 1.0 };
    let t = ((dist - NEAR_IN) / (NEAR_OUT - NEAR_IN)).clamp(0.0, 1.0);
    let near = 1.0 - t * t * (3.0 - 2.0 * t);
    1.0 + ((1.0 / k.max(0.05)).powf(NEAR_SHARE) - 1.0) * near
}

/// The CPU twin of `extraction_sky` (for the tests and the gauges).
pub fn sky(d: Vec3, horizon: Vec3, ground: Vec3) -> Vec3 {
    let s = |e0: f32, e1: f32, x: f32| {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let up = horizon.lerp(horizon * Vec3::from(ZENITH), d.y.clamp(0.0, 1.0).sqrt());
    let down = horizon.lerp(ground, s(0.0, 0.5, -d.y));
    if d.y >= 0.0 { up } else { down }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bevy 0.19.1's `pbr_ambient.wgsl`, whole: the shape the patch is
    /// written against.
    const UPSTREAM: &str = r#"#define_import_path bevy_pbr::ambient

#import bevy_pbr::{
    lighting::{EnvBRDFApprox, F_AB},
    mesh_view_bindings::lights,
}

// A precomputed `NdotV` is provided because it is computed regardless,
// but `world_normal` and the view vector `V` are provided separately for more advanced uses.
fn ambient_light(
    world_position: vec4<f32>,
    world_normal: vec3<f32>,
    V: vec3<f32>,
    NdotV: f32,
    diffuse_color: vec3<f32>,
    specular_color: vec3<f32>,
    perceptual_roughness: f32,
    occlusion: vec3<f32>,
) -> vec3<f32> {
    let diffuse_ambient = EnvBRDFApprox(diffuse_color, F_AB(1.0, NdotV));
    let specular_ambient = EnvBRDFApprox(specular_color, F_AB(perceptual_roughness, NdotV));

    // No real world material has specular values under 0.02, so we use this range as a
    // "pre-baked specular occlusion" that extinguishes the fresnel term, for artistic control.
    // See: https://google.github.io/filament/Filament.md.html#specularocclusion
    let specular_occlusion = saturate(dot(specular_color, vec3(50.0 * 0.33)));

    return (diffuse_ambient + specular_ambient * specular_occlusion) * lights.ambient_color.rgb * occlusion;
}
"#;

    #[test]
    fn the_patch_takes_once_and_keeps_the_flat_ambient_without_fog() {
        let out = patch(UPSTREAM).expect("anchors");
        assert!(out.contains(MARKER) && out.contains(IMPORT_PATCHED));
        assert!(out.find("fn extraction_env").unwrap() < out.find("fn ambient_light").unwrap());
        // A view without fog (the impostors' capture) keeps Bevy's own line.
        assert!(out.contains(&format!("#else\n{RETURN_ANCHOR}#endif")));
        assert!(patch(&out).is_none(), "patched twice");
    }

    /// Bevy's real `pbr_ambient.wgsl` is the one pinned in `UPSTREAM`:
    /// an upgrade that changes it fails here (see the module's notes).
    #[test]
    fn bevys_ambient_is_the_one_we_patch() {
        let home = std::env::var("CARGO_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".cargo")
            });
        let found: Vec<(std::path::PathBuf, String)> = std::fs::read_dir(home.join("registry/src"))
            .into_iter()
            .flatten()
            .flatten()
            .flat_map(|d| std::fs::read_dir(d.path()).into_iter().flatten().flatten())
            .filter(|d| d.file_name().to_string_lossy().starts_with("bevy_pbr-"))
            .filter_map(|d| {
                let p = d.path().join("src/render/pbr_ambient.wgsl");
                std::fs::read_to_string(&p).ok().map(|s| (d.path(), s))
            })
            .collect();
        let Some((dir, src)) = found
            .iter()
            .find(|(d, _)| d.ends_with(format!("bevy_pbr-{}", bevy_pbr_version())))
        else {
            eprintln!("bevy_pbr {} source not found: skipped", bevy_pbr_version());
            return;
        };
        assert_eq!(
            src.as_str(),
            UPSTREAM,
            "{} changed: redo the sky light (src/render/sky_light.rs's notes)",
            dir.display()
        );
    }

    /// The bevy_pbr the game builds against, from Cargo.lock.
    fn bevy_pbr_version() -> String {
        let lock =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock")).unwrap();
        let at = lock
            .find("name = \"bevy_pbr\"\nversion = \"")
            .expect("bevy_pbr in Cargo.lock");
        let rest = &lock[at + "name = \"bevy_pbr\"\nversion = \"".len()..];
        rest[..rest.find('"').unwrap()].to_owned()
    }

    /// In a room, a gun at 30 cm gets twice the room's light or more; the
    /// room past arm's length nothing; out of doors (the yard) nothing at
    /// all.
    #[test]
    fn in_a_room_what_you_hold_keeps_its_light_and_out_of_doors_nothing_changes() {
        let k = crate::world::weather::INTERIOR_LIGHT;
        assert!(near(0.3, Some(k)) >= 2.0, "{}", near(0.3, Some(k)));
        assert_eq!(near(1.5, Some(k)), 1.0);
        for d in [0.1, 0.3, 0.6, 2.0] {
            assert_eq!(near(d, None), 1.0);
        }
        // Never brighter than the room's light uncut.
        assert!(near(0.0, Some(k)) <= 1.0 / k);
    }

    /// Metal reflects the sky you see. Under the default sky at the
    /// game's exposure, the reflection is graded: the horizon (the dome's
    /// own, through the fog's colour) a fifth again as bright as the zenith, the
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
        assert!(side > 1.2 * top, "horizon {side} vs zenith {top}");
        assert!(top > down, "zenith {top} vs ground {down}");
        let flat = luma(ambient);
        assert!(
            (0.5..1.5).contains(&(side / flat)),
            "horizon {side} against the flat ambient {flat}"
        );
    }
}
