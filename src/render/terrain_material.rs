//! The terrain material: StandardMaterial lighting with a custom fragment
//! stage that blends ground layers (grass, forest floor, mud, rock, gravel,
//! asphalt, dirt road) from splat weights in the vertex colours.
//!
//! The layer textures are texture arrays built by `scripts/fetch_assets.py`
//! from CC0 scans. Until they exist (a fresh checkout, CI) the shader paints
//! procedural stand-ins so the game still looks like a place.

use bevy::image::{
    ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExt>;

const SHADER: &str = "shaders/terrain.wgsl";

/// Ground layers, in texture-array order.
pub const LAYERS: [&str; 7] = [
    "grass",
    "forest_floor",
    "mud",
    "rock",
    "gravel",
    "asphalt",
    "dirt_road",
];

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct TerrainParams {
    /// 1 when the texture arrays are real, 0 for procedural stand-ins.
    pub textured: u32,
    /// Global wetness (rain), 0..=1.
    pub wetness: f32,
    /// Metres per texture repeat for the base layers.
    pub tile: f32,
    /// 1 in a burnt valley: the ground goes to ash and scorched earth.
    pub burnt: f32,
    /// How full the puddles are, 0..1 (`world::puddles::fill`).
    pub puddles: f32,
    /// Rain falling, 0..1: rings on the puddles.
    pub rain: f32,
    /// Snow lying, 0..1, above `snow_line` (m; `world::snow`).
    pub snow: f32,
    pub snow_line: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TerrainExt {
    #[uniform(100)]
    pub params: TerrainParams,
    #[texture(101, dimension = "2d_array")]
    #[sampler(102)]
    pub albedo: Option<Handle<Image>>,
    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    pub normal: Option<Handle<Image>>,
    #[texture(105, dimension = "2d_array")]
    #[sampler(106)]
    pub arm: Option<Handle<Image>>,
}

impl MaterialExtension for TerrainExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default());
    }
}

/// Where the processed arrays live (relative to `assets/`).
const ARRAY_DIR: &str = "textures/terrain";

fn arrays_present() -> bool {
    let root = crate::asset_root_path();
    ["albedo", "normal", "arm"]
        .iter()
        .all(|m| root.join(format!("{ARRAY_DIR}/{m}.ktx2")).exists())
}

pub fn terrain_material(asset_server: &AssetServer) -> TerrainMaterial {
    let textured = arrays_present();
    let load = |name: &str, srgb: bool| -> Option<Handle<Image>> {
        textured.then(|| {
            asset_server
                .load_builder()
                .with_settings(move |s: &mut ImageLoaderSettings| {
                    s.is_srgb = srgb;
                    s.sampler = ImageSampler::Descriptor(repeat_sampler());
                })
                .load(format!("{ARRAY_DIR}/{name}.ktx2"))
        })
    };
    if !textured {
        info!(
            "terrain textures not fetched; using procedural ground (run scripts/fetch_assets.py)"
        );
    }
    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.9,
            reflectance: 0.3,
            ..default()
        },
        extension: TerrainExt {
            params: TerrainParams {
                textured: u32::from(textured),
                wetness: 0.35,
                tile: 3.0,
                burnt: 0.0,
                puddles: 0.0,
                rain: 0.0,
                snow: 0.0,
                snow_line: crate::render::fog::NO_SNOW,
            },
            albedo: load("albedo", true),
            normal: load("normal", false),
            arm: load("arm", false),
        },
    }
}

pub fn repeat_sampler() -> ImageSamplerDescriptor {
    ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    }
}
