//! VRAM: what a valley holds on the GPU, against what the playtest's
//! laptop has (an RTX 3060 laptop, 6 GB; the ceiling leaves the driver,
//! the compositor and SteamVR their share). Counted from the assets and
//! the views, without a GPU:
//! - textures: every image, by its format, size, mips and layers;
//! - meshes: every mesh's vertex and index buffers;
//! - shadow maps: the sun's cascades, and shadowed lights' maps;
//! - render targets: both eyes at the auto eye size, with their MSAA.
//!
//! See docs/PERFORMANCE.md, "VRAM".

#![cfg(test)]

use bevy::prelude::*;

/// The ceiling, bytes: the 3060 laptop's 6 GB less the driver's,
/// SteamVR's and the compositor's share.
const VRAM_CEILING: u64 = 4_500 * 1024 * 1024;

const MB: f64 = 1024.0 * 1024.0;

/// An image's bytes on the GPU (all its mips and layers).
pub fn image_bytes(image: &Image) -> u64 {
    let d = &image.texture_descriptor;
    let fmt = d.format;
    let (bw, bh) = fmt.block_dimensions();
    let block = fmt
        .block_copy_size(Some(bevy::render::render_resource::TextureAspect::All))
        .or_else(|| {
            fmt.block_copy_size(Some(
                bevy::render::render_resource::TextureAspect::DepthOnly,
            ))
        })
        .unwrap_or(4) as u64;
    let layers = d.size.depth_or_array_layers.max(1) as u64;
    let mut total = 0;
    for mip in 0..d.mip_level_count.max(1) {
        let w = (d.size.width >> mip).max(1);
        let h = (d.size.height >> mip).max(1);
        let bx = w.div_ceil(bw) as u64;
        let by = h.div_ceil(bh) as u64;
        total += bx * by * block * layers;
    }
    total
}

/// A mesh's bytes on the GPU (its vertex data and indices).
pub fn mesh_bytes(mesh: &Mesh) -> u64 {
    let verts: u64 = mesh
        .attributes()
        .map(|(a, _)| a.format.size() * mesh.count_vertices() as u64)
        .sum();
    let idx = match mesh.indices() {
        Some(bevy::mesh::Indices::U16(i)) => i.len() as u64 * 2,
        Some(bevy::mesh::Indices::U32(i)) => i.len() as u64 * 4,
        None => 0,
    };
    verts + idx
}

/// Both eyes' targets at `px` across, with `samples` MSAA: an HDR colour
/// target (Rgba16Float) and depth (Depth32) at the samples, the resolved
/// colour, the tonemapped output, and the swapchain image.
pub fn eye_targets_bytes(px: u64, samples: u64) -> u64 {
    let pixels = px * px;
    let per_eye = pixels * 8 * samples // HDR colour, multisampled
        + pixels * 4 * samples // depth, multisampled
        + pixels * 8 // resolved HDR
        + pixels * 4 // tonemapped
        + pixels * 4; // the swapchain's image
    per_eye * 2
}

#[derive(Default, Debug)]
pub struct Vram {
    pub textures: u64,
    pub meshes: u64,
    pub shadows: u64,
    pub targets: u64,
    /// Textures by folder, biggest first.
    pub by_folder: Vec<(String, u64)>,
}

impl Vram {
    pub fn total(&self) -> u64 {
        self.textures + self.meshes + self.shadows + self.targets
    }
}

pub fn measure(world: &mut World, eye_px: u64) -> Vram {
    let mut v = Vram::default();
    let server = world.resource::<AssetServer>().clone();
    let mut folders: std::collections::HashMap<String, u64> = Default::default();
    for (id, image) in world.resource::<Assets<Image>>().iter() {
        let b = image_bytes(image);
        v.textures += b;
        let folder = server
            .get_path(id)
            .map(|p| {
                let s = p.path().to_string_lossy().to_string();
                let parts: Vec<&str> = s.split('/').collect();
                parts[..parts.len().saturating_sub(1).min(3)].join("/")
            })
            .unwrap_or_else(|| "(made in game)".into());
        *folders.entry(folder).or_default() += b;
    }
    let mut by: Vec<(String, u64)> = folders.into_iter().collect();
    by.sort_by(|a, b| b.1.cmp(&a.1));
    v.by_folder = by;
    v.meshes = world
        .resource::<Assets<Mesh>>()
        .iter()
        .map(|(_, m)| mesh_bytes(m))
        .sum();
    // The sun: a depth layer a cascade, and cascades are per view (both
    // eyes have their own); each shadowed point light a cube, shared.
    let map = world
        .get_resource::<bevy::light::DirectionalLightShadowMap>()
        .map_or(2048, |m| m.size) as u64;
    let cascades = {
        let mut q = world.query::<&bevy::light::CascadeShadowConfig>();
        q.iter(world).map(|c| c.bounds.len()).max().unwrap_or(0) as u64
    };
    let point = world
        .get_resource::<bevy::light::PointLightShadowMap>()
        .map_or(1024, |m| m.size) as u64;
    let shadowed_points = {
        let mut q = world.query::<&PointLight>();
        q.iter(world).filter(|l| l.shadow_maps_enabled).count() as u64
    };
    v.shadows = map * map * 4 * cascades * 2 + shadowed_points * point * point * 4 * 6;
    let samples = {
        let mut q = world.query_filtered::<&Msaa, With<Camera3d>>();
        q.iter(world).map(|m| m.samples() as u64).max().unwrap_or(4)
    };
    v.targets = eye_targets_bytes(eye_px, samples);
    v
}

fn holds(map: &str) {
    let _one_at_a_time = super::budget::tests::heavy();
    let mut app = super::budget::tests::client_threads(map, "clearing", None);
    super::budget::tests::settle(&mut app);
    // What loads at start loads in the background: until it has landed.
    for _ in 0..2000 {
        let loading = {
            let server = app.world().resource::<AssetServer>();
            let models = app.world().resource::<crate::models::Models>();
            models.handles().any(|h| {
                matches!(
                    server.get_load_state(h),
                    Some(bevy::asset::LoadState::Loading)
                )
            })
        };
        if !loading {
            break;
        }
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    for _ in 0..60 {
        app.update();
    }
    let px = std::env::var("BUDGET_EYE_PX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(crate::rig::comfort::Comfort::AUTO_EYE_PX as u64);
    let v = measure(app.world_mut(), px);
    println!(
        "{map}: {:.0} MB of {:.0}: textures {:.0}, meshes {:.0}, shadows {:.0}, eye targets {:.0} (2 x {px}²)",
        v.total() as f64 / MB,
        VRAM_CEILING as f64 / MB,
        v.textures as f64 / MB,
        v.meshes as f64 / MB,
        v.shadows as f64 / MB,
        v.targets as f64 / MB
    );
    for (f, b) in v.by_folder.iter().take(12) {
        println!("    {f}: {:.0} MB", *b as f64 / MB);
    }
    assert!(
        v.total() <= VRAM_CEILING,
        "{map} holds {:.0} MB on the GPU, over {:.0}",
        v.total() as f64 / MB,
        VRAM_CEILING as f64 / MB
    );
}

#[test]
#[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
fn gauge_every_valley_holds_its_vram() {
    for (map, _) in extraction_core::map::VALLEYS {
        holds(map);
    }
}

/// The sums are right for what's easy to count by hand.
#[test]
fn vram_counts_add_up() {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let img = Image::new_fill(
        Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );
    assert_eq!(image_bytes(&img), 256 * 256 * 4);
    let mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
    assert!(mesh_bytes(&mesh) > 24 * 12);
    assert_eq!(
        eye_targets_bytes(100, 4),
        2 * 100 * 100 * (32 + 16 + 8 + 4 + 4)
    );
}
