//! Which file a texture loads from. `scripts/bc7_textures.py` bakes a
//! BC7 copy (`<name>.bc7.ktx2`) of the colour and data maps beside each
//! RGBA8 original: the GPU samples it as it is (nothing decoded as it
//! loads) at a quarter of the bytes. Normal maps get a BC5 copy
//! (`normal.bc5.ktx2`: X and Y, Z rebuilt in the shader; Bevy's own
//! materials see the format and rebuild it, terrain.wgsl always does). It's loaded wherever the GPU has BC
//! formats (every desktop GPU) and the copy exists; otherwise, and with no
//! GPU at all (the tests), the original. See docs/PERFORMANCE.md,
//! "Textures".

use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

static BC: AtomicBool = AtomicBool::new(false);

pub struct TexturesPlugin;

impl Plugin for TexturesPlugin {
    fn build(&self, _app: &mut App) {}

    /// After the renderer's own `finish`, which made the device.
    fn finish(&self, app: &mut App) {
        // The device's formats; with none (the tests), the image loader's
        // (a GPU-less test client says it takes BC, so it measures what
        // ships: the copies loaded as they are, never sampled).
        let device = app
            .world()
            .get_resource::<bevy::render::renderer::RenderDevice>()
            .map(|d| {
                d.features()
                    .contains(bevy::render::render_resource::WgpuFeatures::TEXTURE_COMPRESSION_BC)
            });
        let loader = app
            .world()
            .get_resource::<bevy::image::CompressedImageFormatSupport>()
            .is_some_and(|s| s.0.contains(bevy::image::CompressedImageFormats::BC));
        let bc = device.unwrap_or(loader)
            // `EXTRACTION_BC=0`: the originals (to compare the look).
            && std::env::var("EXTRACTION_BC").map_or(true, |v| v != "0");
        BC.store(bc, Ordering::Relaxed);
        if app
            .world()
            .contains_resource::<bevy::render::renderer::RenderDevice>()
        {
            crate::report::log_fact(
                "textures",
                if bc {
                    "BC7 where baked"
                } else {
                    "RGBA8 (no BC formats on this GPU, or EXTRACTION_BC=0)"
                },
            );
        }
    }
}

/// The path to load for `path` (an asset path ending `.ktx2`): its BC7
/// (or a normal map's BC5) copy where the GPU takes it and the copy's
/// there, else `path`.
pub fn ktx2(path: &str) -> String {
    if !BC.load(Ordering::Relaxed) {
        return path.to_string();
    }
    let Some(stem) = path.strip_suffix(".ktx2") else {
        return path.to_string();
    };
    let bc = if stem.ends_with("normal") {
        format!("{stem}.bc5.ktx2")
    } else {
        format!("{stem}.bc7.ktx2")
    };
    if crate::asset_root_path().join(&bc).exists() {
        bc
    } else {
        path.to_string()
    }
}

/// The textures under `assets/textures` with no baked copy beside them
/// (`scripts/bc7_textures.py` makes them; it needs etcpak). The load and
/// VRAM gauges measure what ships, the copies: without them they'd
/// measure the RGBA8 originals (a valley's VRAM 2.2 GB, not 2.0, and its
/// load a third slower) and blame the valley.
#[cfg(test)]
pub(crate) fn unbaked() -> Vec<String> {
    let root = crate::asset_root_path();
    let mut out = Vec::new();
    let mut stack = vec![root.join("textures")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = p.to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".ktx2") else {
                continue;
            };
            if stem.ends_with(".bc7") || stem.ends_with(".bc5") {
                continue;
            }
            let copy = if stem.ends_with("normal") {
                format!("{stem}.bc5.ktx2")
            } else {
                format!("{stem}.bc7.ktx2")
            };
            if !std::path::Path::new(&copy).exists() {
                out.push(
                    p.strip_prefix(&root)
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With no GPU (or none with BC), every texture is its original; with
    /// one, the baked copy where there is one.
    #[test]
    fn the_copy_only_where_it_can_be_used() {
        let albedo = "textures/materials/boulder/albedo.ktx2";
        let normal = "textures/materials/boulder/normal.ktx2";
        // (A test client in this process may have said BC already.)
        BC.store(false, Ordering::Relaxed);
        assert_eq!(ktx2(albedo), albedo);
        BC.store(true, Ordering::Relaxed);
        let baked = crate::asset_root_path()
            .join("textures/materials/boulder/albedo.bc7.ktx2")
            .exists();
        assert_eq!(
            ktx2(albedo),
            if baked {
                "textures/materials/boulder/albedo.bc7.ktx2"
            } else {
                albedo
            }
        );
        let baked = crate::asset_root_path()
            .join("textures/materials/boulder/normal.bc5.ktx2")
            .exists();
        assert_eq!(
            ktx2(normal),
            if baked {
                "textures/materials/boulder/normal.bc5.ktx2"
            } else {
                normal
            },
            "a normal map's copy is BC5"
        );
        BC.store(false, Ordering::Relaxed);
    }
}
