//! Patterned wear on guns: burnt powder gathered at the muzzle and the
//! ejection port, and the edges (bevels and roundings, where the hand and
//! the holster rub) worn bright, then rusted through near failure.
//!
//! A whole-material tint (`weapons::weather`) can't place wear, and a mask
//! texture per gun would cost memory for every gun. So Bevy's
//! `bevy_pbr::pbr_fragment` is patched when it loads, as `fog` patches
//! `pbr_functions`: at the end of `pbr_input_from_standard_material`, a
//! material marked by a negative `thickness` gets the wear. The edges come
//! from how fast the shading normal turns across the pixel (bevels turn
//! fast, flats not at all); the muzzle and port are points in the mesh's
//! own space, so a moving part (the slide) takes its soot with it and
//! nothing is written per frame.
//!
//! This is our own code editing Bevy's shader *asset* as it loads (the
//! `Shader` in `Assets<Shader>` with import path `bevy_pbr::pbr_fragment`),
//! as `render::fog` does for `pbr_functions`. The bevy_pbr crate, its
//! source and Cargo.toml are untouched.
//!
//! ## Borrowed fields
//!
//! A gun's own material copy (`weapons::weather`) carries the wear in four
//! `StandardMaterial` fields that only transmission (glass, liquids)
//! reads:
//!
//! | Field | Holds | Read by Bevy only when |
//! | - | - | - |
//! | `thickness` | the marker (below 0) and the amounts ([`code`]) | `specular_transmission` or `diffuse_transmission` > 0 |
//! | `attenuation_color` | the muzzle (rgb) and the port's x (alpha) | the same, and only with a finite `attenuation_distance` |
//! | `clearcoat_perceptual_roughness` | the port's y | `clearcoat` > 0 |
//! | `ior` | the port's z | transmission, as above |
//!
//! Why it is safe:
//! - [`encode`] refuses any material with transmission or clearcoat, and
//!   never turns either on, so Bevy never reads the fields as what they
//!   are. (`attenuation_distance` is `render::held_light`'s, and is left
//!   alone.)
//! - None of the four is in `StandardMaterialKey`, so no new pipelines.
//!   Both are tested (`wear_never_turns_on_transmission_or_a_new_pipeline`).
//! - A material without the marker (`thickness` >= 0, Bevy's default 0)
//!   skips the wear after one uniform read.
//! - If a later Bevy reads one of these fields elsewhere, or moves the
//!   anchor, this is where to look: the anchor test patches Bevy's own
//!   source from the registry.
//!
//! If the anchor stops matching a new Bevy, it warns and guns keep the
//! flat tint.

use bevy::prelude::*;
use bevy::shader::{Shader, ShaderImport, Source};

pub struct GunWearPlugin;

impl Plugin for GunWearPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, patch_pbr_fragment);
    }
}

const MARKER: &str = "// extraction: gun wear";

/// The end of `pbr_input_from_standard_material`, where the wear goes.
const ANCHOR_END: &str = "    return pbr_input;\n}";

/// How a material ages, as the shader's class: 0 rusts, 1 wears to bare
/// alloy, 2 fades (wood, Bakelite, polymer).
pub const CLASS_RUSTS: u32 = 0;
pub const CLASS_BARES: u32 = 1;
pub const CLASS_FADES: u32 = 2;

fn q(x: f32) -> f32 {
    (x.clamp(0.0, 1.0) * 99.0).round()
}

/// The marker and the amounts, as one negative number: class, then worn,
/// late (the last stretch to failure) and soot, each in 0..99. Under
/// 2^24, so exact in an f32.
pub fn code(class: u32, worn: f32, late: f32, soot: f32) -> f32 {
    -(1.0 + class as f32 * 1_000_000.0 + q(worn) * 10_000.0 + q(late) * 100.0 + q(soot))
}

/// Writes the wear into a material (its muzzle and port in the mesh's own
/// space). False, and nothing written, for a material that transmits
/// light (glass), which needs those fields.
pub fn encode(
    m: &mut StandardMaterial,
    class: u32,
    worn: f32,
    late: f32,
    soot: f32,
    muzzle: Vec3,
    port: Vec3,
) -> bool {
    if m.specular_transmission > 0.0 || m.diffuse_transmission > 0.0 || m.clearcoat > 0.0 {
        return false;
    }
    m.thickness = code(class, worn, late, soot);
    m.attenuation_color = Color::LinearRgba(LinearRgba::new(muzzle.x, muzzle.y, muzzle.z, port.x));
    m.clearcoat_perceptual_roughness = port.y;
    m.ior = port.z;
    true
}

/// Reads one field of the material, bound or bindless.
fn field(name: &str) -> String {
    format!(
        "#ifdef BINDLESS\n        let xw_{name} = pbr_bindings::material_array[material_indices[slot].material].{name};\n#else\n        let xw_{name} = pbr_bindings::material.{name};\n#endif\n"
    )
}

fn wear_wgsl() -> String {
    let mut s = String::new();
    s.push_str(MARKER);
    s.push_str("\n#ifndef PREPASS_PIPELINE\n#ifndef MESHLET_MESH_MATERIAL_PASS\n    {\n");
    s.push_str(&field("thickness"));
    s.push_str("        if (xw_thickness < 0.0) {\n");
    s.push_str(&field("attenuation_color"));
    s.push_str(&field("clearcoat_perceptual_roughness"));
    s.push_str(&field("ior"));
    s.push_str(WEAR_BODY);
    s.push_str("        }\n    }\n#endif\n#endif\n");
    s
}

/// The wear itself, given the fields. `in`, `slot` and `pbr_input` are
/// `pbr_input_from_standard_material`'s.
const WEAR_BODY: &str = r#"
            let xw_code = -xw_thickness - 1.0;
            let xw_class = floor(xw_code / 1000000.0);
            var xw_r = xw_code - xw_class * 1000000.0;
            let xw_worn = floor(xw_r / 10000.0);
            xw_r = xw_r - xw_worn * 10000.0;
            let xw_late = floor(xw_r / 100.0);
            let xw_soot = (xw_r - xw_late * 100.0) / 99.0;
            let xw_w = xw_worn / 99.0;
            let xw_l = xw_late / 99.0;
            // The muzzle and port, from the mesh's space to the world.
            let xw_a = mesh[in.instance_index].world_from_local;
            let xw_m4 = vec4<f32>(xw_attenuation_color.xyz, 1.0);
            let xw_p4 = vec4<f32>(xw_attenuation_color.w, xw_clearcoat_perceptual_roughness, xw_ior, 1.0);
            let xw_muzzle = vec3<f32>(dot(xw_a[0], xw_m4), dot(xw_a[1], xw_m4), dot(xw_a[2], xw_m4));
            let xw_port = vec3<f32>(dot(xw_a[0], xw_p4), dot(xw_a[1], xw_p4), dot(xw_a[2], xw_p4));
            let xw_P = pbr_input.world_position.xyz;
            // Burnt powder: black at the muzzle's face and the port's
            // edges, thinning over a few centimetres.
            let xw_near = max(
                1.0 - smoothstep(0.008, 0.05, distance(xw_P, xw_muzzle)),
                1.0 - smoothstep(0.012, 0.06, distance(xw_P, xw_port)),
            );
            let xw_s = xw_soot * xw_near;
            // Edges: the shading normal turns fast across a bevel (about
            // 500 per metre), slowly over a barrel (100), not on a flat.
            let xw_curve = length(fwidth(pbr_input.world_normal)) / max(length(fwidth(xw_P)), 1e-6);
            let xw_e = smoothstep(150.0, 600.0, xw_curve) * xw_w;
            var xw_col = pbr_input.material.base_color.rgb;
            var xw_met = pbr_input.material.metallic;
            var xw_rough = pbr_input.material.perceptual_roughness;
            if (xw_class < 0.5) {
                // Steel: rubbed bright, then near failure rusted through
                // and pitted.
                let xw_to = mix(vec3<f32>(0.5, 0.5, 0.51), vec3<f32>(0.34, 0.12, 0.04), xw_l);
                xw_col = mix(xw_col, xw_to, xw_e * 0.85);
                xw_met = mix(xw_met, mix(1.0, 0.05, xw_l), xw_e);
                xw_rough = mix(xw_rough, mix(0.32, 0.95, xw_l), xw_e);
            } else if (xw_class < 1.5) {
                // Anodised alloy: worn through to bright metal.
                xw_col = mix(xw_col, vec3<f32>(0.62, 0.63, 0.64), xw_e * 0.8);
                xw_met = mix(xw_met, 1.0, xw_e);
                xw_rough = mix(xw_rough, 0.35, xw_e);
            } else {
                // Wood, Bakelite, polymer: rubbed pale and dry.
                xw_col = mix(xw_col, xw_col * 1.5 + vec3<f32>(0.06), xw_e * 0.7);
                xw_rough = mix(xw_rough, 0.9, xw_e);
            }
            xw_col = mix(xw_col, vec3<f32>(0.025, 0.025, 0.023), xw_s * 0.9);
            xw_met = xw_met * (1.0 - xw_s);
            xw_rough = mix(xw_rough, 0.9, xw_s);
            pbr_input.material.base_color = vec4<f32>(xw_col, pbr_input.material.base_color.a);
            pbr_input.material.metallic = xw_met;
            pbr_input.material.perceptual_roughness = xw_rough;
"#;

/// The patched source, or None if the anchor is gone.
fn patch(src: &str) -> Option<String> {
    if src.contains(MARKER) {
        return None;
    }
    let at = src.rfind(ANCHOR_END)?;
    // It must be the end of pbr_input_from_standard_material, the last
    // function in the file.
    if !src[..at].contains("fn pbr_input_from_standard_material(") {
        return None;
    }
    let mut out = String::with_capacity(src.len() + 4096);
    out.push_str(&src[..at]);
    out.push_str(&wear_wgsl());
    out.push_str(&src[at..]);
    Some(out)
}

fn patch_pbr_fragment(
    mut events: MessageReader<AssetEvent<Shader>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut warned: Local<bool>,
) {
    let wanted = ShaderImport::Custom("bevy_pbr::pbr_fragment".into());
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
        // Our own edit fires Modified too: then the marker is there.
        if src.contains(MARKER) {
            continue;
        }
        let Some(new_src) = patch(src) else {
            if !*warned {
                warn!("gun wear: pbr_fragment.wgsl anchor not found; guns keep the flat tint");
                *warned = true;
            }
            continue;
        };
        let mut replacement = Shader::from_wgsl(new_src, shader.path.clone());
        replacement.shader_defs = shader.shader_defs.clone();
        replacement.file_dependencies = shader.file_dependencies.clone();
        replacement.additional_imports = shader.additional_imports.clone();
        let id = *id;
        if let Some(mut s) = shaders.get_mut(id) {
            *s = replacement;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bevy's own pbr_fragment.wgsl from the cargo registry, if it's there.
    fn bevy_source() -> Option<String> {
        let home = std::env::var("CARGO_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| std::path::Path::new(&h).join(".cargo")))
            .ok()?;
        for reg in std::fs::read_dir(home.join("registry/src")).ok()?.flatten() {
            let f = reg
                .path()
                .join("bevy_pbr-0.19.1/src/render/pbr_fragment.wgsl");
            if let Ok(s) = std::fs::read_to_string(f) {
                return Some(s);
            }
        }
        None
    }

    #[test]
    fn the_anchor_is_in_bevys_shader() {
        let Some(src) = bevy_source() else {
            return;
        };
        let out = patch(&src).expect("anchor");
        assert!(out.contains(MARKER));
        assert!(out.trim_end().ends_with("return pbr_input;\n}"));
        assert!(patch(&out).is_none(), "patches once");
    }

    #[test]
    fn the_code_reads_back_exactly() {
        // As the shader unpacks it.
        let c = code(CLASS_FADES, 0.85, 1.0, 0.37);
        let n = -c - 1.0;
        let class = (n / 1_000_000.0).floor();
        let mut r = n - class * 1_000_000.0;
        let worn = (r / 10_000.0).floor();
        r -= worn * 10_000.0;
        let late = (r / 100.0).floor();
        let soot = r - late * 100.0;
        assert_eq!(class, 2.0);
        assert_eq!(worn, q(0.85));
        assert_eq!(late, 99.0);
        assert_eq!(soot, q(0.37));
        assert!(
            code(CLASS_RUSTS, 0.0, 0.0, 0.0) < 0.0,
            "a clean gun is still marked"
        );
    }

    #[test]
    fn glass_is_left_alone() {
        let mut glass = StandardMaterial {
            specular_transmission: 0.9,
            thickness: 0.01,
            ..default()
        };
        assert!(!encode(
            &mut glass,
            0,
            1.0,
            1.0,
            1.0,
            Vec3::ZERO,
            Vec3::ZERO
        ));
        assert_eq!(glass.thickness, 0.01);
    }

    #[test]
    fn wear_never_turns_on_transmission_or_a_new_pipeline() {
        use bevy::pbr::StandardMaterialKey;
        // Each finish a gun has: bluing, alloy, wood, and a held copy
        // (held_light's finite attenuation_distance).
        let finishes = [
            StandardMaterial {
                metallic: 0.9,
                perceptual_roughness: 0.3,
                ..default()
            },
            StandardMaterial {
                metallic: 0.0,
                ..default()
            },
            StandardMaterial {
                attenuation_distance: 3e33,
                ..default()
            },
        ];
        for orig in finishes {
            for (worn, late, soot) in [(0.0, 0.0, 0.0), (1.0, 1.0, 1.0), (0.4, 0.0, 0.7)] {
                let mut m = orig.clone();
                assert!(encode(
                    &mut m,
                    CLASS_FADES,
                    worn,
                    late,
                    soot,
                    Vec3::new(0.0, 0.06, -0.12),
                    Vec3::new(0.012, 0.068, -0.04),
                ));
                // Transmission and clearcoat stay off, so the borrowed
                // fields are never read as what they are...
                assert_eq!(m.specular_transmission, 0.0);
                assert_eq!(m.diffuse_transmission, 0.0);
                assert_eq!(m.clearcoat, 0.0);
                assert_eq!(m.attenuation_distance, orig.attenuation_distance);
                // ...and the pipeline key is the one it had.
                assert!(StandardMaterialKey::from(&m) == StandardMaterialKey::from(&orig));
            }
        }
    }
}
