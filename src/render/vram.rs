//! VRAM: what a valley holds on the GPU, against what the playtest's
//! laptop has (an RTX 3060 laptop, 6 GB; the ceiling leaves the driver,
//! the compositor and SteamVR their share). Counted from the assets and
//! the views, without a GPU:
//! - textures: every image, by its format, size, mips and layers;
//! - meshes: every mesh's vertex and index buffers;
//! - shadow maps: the sun's cascades, and shadowed lights' maps;
//! - render targets: both eyes at the runtime's full size (holding the
//!   frame rate, they're made that big), with their MSAA.
//!
//! See docs/PERFORMANCE.md, "VRAM".

#![cfg(test)]

use bevy::prelude::*;

/// The ceiling, bytes: the 3060 laptop's 6 GB less the driver's,
/// SteamVR's and the compositor's share.
pub(crate) const VRAM_CEILING: u64 = 4_500 * 1024 * 1024;

/// What a valley may hold, under the ceiling: below the 2.0-2.2 GB they
/// held with every texture RGBA8. The colour and data maps are BC7 now:
/// 1.75-1.99 GB.
pub(crate) const VRAM_BUDGET: u64 = 2_048 * 1024 * 1024;

pub(crate) const MB: f64 = 1024.0 * 1024.0;

/// The eyes' width the targets are made at when holding the frame rate
/// (`render::hold_rate`): the playtest's Beyond 2's recommended size.
const HELD_EYE_PX: u64 = 2980;

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

/// Counts what `world` (a valley's client, loaded) holds, prints it by
/// kind and folder, and gives the total. `BUDGET_EYE_PX` sets the eyes'
/// width; holding the frame rate (the default), their targets are the
/// runtime's full size, the playtest's Beyond 2's 2980.
pub(crate) fn report(map: &str, world: &mut World) -> u64 {
    let px = std::env::var("BUDGET_EYE_PX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(HELD_EYE_PX);
    let v = measure(world, px);
    println!(
        "    VRAM: {:.0} MB of {:.0} (budget {:.0}): textures {:.0}, meshes {:.0}, shadows {:.0}, eye targets {:.0} (2 x {px}²)",
        v.total() as f64 / MB,
        VRAM_CEILING as f64 / MB,
        VRAM_BUDGET as f64 / MB,
        v.textures as f64 / MB,
        v.meshes as f64 / MB,
        v.shadows as f64 / MB,
        v.targets as f64 / MB
    );
    for (f, b) in v.by_folder.iter().take(6) {
        println!("        {f}: {:.0} MB", *b as f64 / MB);
    }
    let _ = map;
    v.total()
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
