//! Which file a texture loads from. `scripts/bc7_textures.py` bakes a
//! BC7 copy (`<name>.bc7.ktx2`) of the colour and data maps beside each
//! RGBA8 original: the GPU samples it as it is (nothing decoded as it
//! loads) at a quarter of the bytes. It's loaded wherever the GPU has BC
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
/// copy where the GPU takes it and the copy's there, else `path`.
pub fn ktx2(path: &str) -> String {
    if !BC.load(Ordering::Relaxed) {
        return path.to_string();
    }
    let Some(stem) = path.strip_suffix(".ktx2") else {
        return path.to_string();
    };
    let bc7 = format!("{stem}.bc7.ktx2");
    if crate::asset_root_path().join(&bc7).exists() {
        bc7
    } else {
        path.to_string()
    }
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
        assert_eq!(ktx2(normal), normal, "normal maps aren't baked");
        BC.store(false, Ordering::Relaxed);
    }
}
