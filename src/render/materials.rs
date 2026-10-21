//! The material library: CC0 PBR sets from `assets/textures/materials/`
//! (see `docs/ASSETS.md`), with flat-colour fallbacks when they haven't
//! been fetched. Meshes built here use UVs in metres; each material scales
//! them to its own tile size, so texel density is the same everywhere.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::math::Affine2;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use std::collections::HashMap;

use super::terrain_material::repeat_sampler;

/// Surface kinds the world is built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mat {
    Planks,
    LogWall,
    Brick,
    Plaster,
    Concrete,
    RustyMetal,
    PaintedMetal,
    Corrugated,
    Shingles,
    WoodFloor,
    Canvas,
    BarkSpruce,
    BarkBirch,
    BarkPine,
    RobotShell,
    GunMetal,
    GunWood,
    Polymer,
    /// Window glass.
    Glass,
    /// Untextured dark metal (fittings, wire).
    DarkMetal,
    /// Untextured raw timber (furniture, beams).
    Timber,
    /// Road asphalt (bridge decks).
    Asphalt,
    /// Weathered rock: boulders and outcrops.
    Boulder,
    /// Burnt timber: a ruin's planks, rafters and floor.
    Charred,
    /// Brick blackened by a fire.
    SootBrick,
    /// Plaster blackened by a fire.
    SootPlaster,
    /// Dressed stone in courses (ashlar): a dam's face, a masonry tower.
    DressedStone,
    /// A reservoir's mud, dried at the top: what fills a drowned house.
    Silt,
    /// Timber sixty years under water: black-brown, slimed.
    Sodden,
}

impl Mat {
    /// (texture dir, metres per tile, fallback colour, roughness).
    pub(crate) fn info(self) -> (Option<&'static str>, f32, Color, f32) {
        match self {
            Mat::Planks => (
                Some("weathered_planks"),
                2.0,
                Color::srgb(0.42, 0.40, 0.36),
                0.85,
            ),
            Mat::LogWall => (Some("log_wall"), 2.5, Color::srgb(0.33, 0.25, 0.18), 0.85),
            Mat::Brick => (Some("red_brick"), 2.0, Color::srgb(0.45, 0.25, 0.2), 0.85),
            Mat::Plaster => (
                Some("plaster_wall_dirty"),
                3.0,
                Color::srgb(0.62, 0.6, 0.55),
                0.9,
            ),
            Mat::Concrete => (
                Some("concrete_wet"),
                3.0,
                Color::srgb(0.45, 0.45, 0.43),
                0.8,
            ),
            Mat::RustyMetal => (Some("rusty_metal"), 2.0, Color::srgb(0.4, 0.25, 0.15), 0.7),
            Mat::PaintedMetal => (
                Some("painted_metal_peeling"),
                2.0,
                Color::srgb(0.3, 0.38, 0.35),
                0.6,
            ),
            Mat::Corrugated => (
                Some("corrugated_iron"),
                2.0,
                Color::srgb(0.4, 0.3, 0.25),
                0.6,
            ),
            Mat::Shingles => (
                Some("roof_tiles_or_shingles"),
                2.0,
                Color::srgb(0.2, 0.2, 0.2),
                0.85,
            ),
            Mat::WoodFloor => (
                Some("wood_floor_old"),
                2.0,
                Color::srgb(0.35, 0.27, 0.2),
                0.7,
            ),
            Mat::Canvas => (
                Some("fabric_canvas"),
                1.0,
                Color::srgb(0.3, 0.31, 0.21),
                0.95,
            ),
            Mat::BarkSpruce => (
                Some("bark_spruce"),
                1.2,
                Color::srgb(0.22, 0.17, 0.13),
                0.95,
            ),
            Mat::BarkBirch => (Some("bark_birch"), 1.2, Color::srgb(0.75, 0.73, 0.7), 0.85),
            Mat::BarkPine => (Some("bark_pine"), 1.2, Color::srgb(0.4, 0.26, 0.16), 0.95),
            Mat::RobotShell => (
                Some("robot_shell"),
                1.0,
                Color::srgb(0.86, 0.87, 0.88),
                0.35,
            ),
            Mat::GunMetal => (Some("gun_metal"), 0.4, Color::srgb(0.08, 0.08, 0.09), 0.4),
            Mat::GunWood => (Some("gun_wood"), 0.5, Color::srgb(0.36, 0.17, 0.08), 0.5),
            Mat::Polymer => (
                Some("polymer_plum"),
                0.4,
                Color::srgb(0.27, 0.1, 0.11),
                0.55,
            ),
            Mat::Glass => (None, 1.0, Color::srgba(0.12, 0.14, 0.15, 0.55), 0.05),
            Mat::DarkMetal => (None, 1.0, Color::srgb(0.12, 0.12, 0.12), 0.55),
            Mat::Timber => (Some("timber"), 1.5, Color::srgb(0.9, 0.85, 0.8), 0.8),
            Mat::Asphalt => (Some("asphalt"), 4.0, Color::srgb(0.25, 0.25, 0.24), 0.75),
            Mat::Boulder => (Some("boulder"), 2.4, Color::srgb(0.62, 0.56, 0.47), 0.85),
            Mat::Charred => (
                Some("weathered_planks"),
                2.0,
                Color::srgb(0.07, 0.06, 0.055),
                0.95,
            ),
            Mat::SootBrick => (Some("red_brick"), 2.0, Color::srgb(0.2, 0.13, 0.11), 0.9),
            Mat::SootPlaster => (
                Some("plaster_wall_dirty"),
                3.0,
                Color::srgb(0.18, 0.17, 0.16),
                0.95,
            ),
            // Blocks about 0.6 m high, six courses to a tile.
            Mat::DressedStone => (
                Some("dressed_stone"),
                3.6,
                Color::srgb(0.5, 0.49, 0.46),
                0.85,
            ),
            Mat::Silt => (Some("concrete_wet"), 4.0, Color::srgb(0.33, 0.3, 0.25), 0.9),
            Mat::Sodden => (
                Some("weathered_planks"),
                2.0,
                Color::srgb(0.12, 0.11, 0.08),
                0.7,
            ),
        }
    }
}

/// Lazily built materials, shared everywhere.
#[derive(Resource, Default)]
pub struct MaterialLibrary {
    cache: HashMap<Mat, Handle<StandardMaterial>>,
}

impl MaterialLibrary {
    pub fn get(
        &mut self,
        mat: Mat,
        server: &AssetServer,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        if let Some(h) = self.cache.get(&mat) {
            return h.clone();
        }
        let (dir, tile, color, rough) = mat.info();
        let root = crate::asset_root_path().join("textures/materials");
        let textured = dir.is_some_and(|d| root.join(d).join("albedo.ktx2").exists());
        let mut m = StandardMaterial {
            base_color: color,
            perceptual_roughness: rough,
            uv_transform: Affine2::from_scale(Vec2::splat(1.0 / tile)),
            ..default()
        };
        if matches!(mat, Mat::DarkMetal | Mat::GunMetal) {
            m.metallic = 0.8;
        }
        if mat == Mat::Glass {
            m.alpha_mode = AlphaMode::Blend;
            m.reflectance = 0.8;
        }
        if textured {
            let d = dir.unwrap();
            let load = |name: &str, srgb: bool| -> Handle<Image> {
                server
                    .load_builder()
                    .with_settings(move |s: &mut ImageLoaderSettings| {
                        s.is_srgb = srgb;
                        s.sampler = ImageSampler::Descriptor(repeat_sampler());
                    })
                    .load(format!("textures/materials/{d}/{name}.ktx2"))
            };
            // A few scans want grading to sit in the same gloom.
            m.base_color = match mat {
                Mat::Plaster => Color::srgb(0.78, 0.74, 0.66),
                Mat::Concrete => Color::srgb(0.85, 0.85, 0.83),
                // The rock scan is a pale, cool granite: warmer and darker
                // to sit in the valleys' soil (rocks vary it per rock).
                Mat::Boulder => color,
                // Burnt: the planks' grain under soot, the brick's courses
                // blackened.
                Mat::Charred => Color::srgb(0.3, 0.26, 0.23),
                Mat::SootBrick => Color::srgb(0.45, 0.4, 0.38),
                Mat::SootPlaster => Color::srgb(0.3, 0.28, 0.26),
                // The concrete's pores as dried mud; the planks' grain
                // under slime.
                Mat::Silt => Color::srgb(0.62, 0.55, 0.43),
                Mat::Sodden => Color::srgb(0.52, 0.5, 0.4),
                _ => Color::WHITE,
            };
            m.base_color_texture = Some(load("albedo", true));
            m.normal_map_texture = Some(load("normal", false));
            let arm = load("arm", false);
            m.occlusion_texture = Some(arm.clone());
            if mat == Mat::Boulder {
                // The scan's roughness glints like wet plastic on a big
                // rock face: weathered rock is matt.
                m.perceptual_roughness = 0.9;
                m.metallic = 0.0;
            } else {
                m.metallic_roughness_texture = Some(arm);
                m.perceptual_roughness = 1.0;
                m.metallic = 1.0;
            }
        }
        let h = materials.add(m);
        self.cache.insert(mat, h.clone());
        h
    }
}

/// A box with UVs in metres on every face, and tangents for normal maps.
pub fn metric_box(size: Vec3) -> Mesh {
    let h = size * 0.5;
    // (normal, u axis, v axis) per face; v runs down the face.
    let faces = [
        (Vec3::X, Vec3::NEG_Z, Vec3::NEG_Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::NEG_Y),
        (Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::Z, Vec3::X, Vec3::NEG_Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::NEG_Y),
    ];
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    let mut idx = Vec::new();
    for (n, u, v) in faces {
        let base = pos.len() as u32;
        let hu = (u * h).abs().max_element();
        let hv = (v * h).abs().max_element();
        let hn = (n * h).abs().max_element();
        for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let p = n * hn + u * hu * su + v * hv * sv;
            pos.push(p.to_array());
            nrm.push(n.to_array());
            uv.push([hu * su + hu, hv * sv + hv]);
        }
        // Wind counter-clockwise seen from outside.
        idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx));
    let _ = mesh.generate_tangents();
    mesh
}

/// A triangular prism (a gable end): base `width` along X, `height` up,
/// `depth` along Z, apex centred.
pub fn gable_prism(width: f32, height: f32, depth: f32) -> Mesh {
    let (w, d) = (width * 0.5, depth * 0.5);
    let a = Vec3::new(-w, 0.0, 0.0);
    let b = Vec3::new(w, 0.0, 0.0);
    let c = Vec3::new(0.0, height, 0.0);
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for (z, n) in [(d, Vec3::Z), (-d, Vec3::NEG_Z)] {
        let base = pos.len() as u32;
        for p in [a, b, c] {
            let q = p + Vec3::Z * z;
            pos.push(q.to_array());
            nrm.push(n.to_array());
            uv.push([q.x + w, height - q.y]);
        }
        if n.z > 0.0 {
            idx.extend_from_slice(&[base, base + 1, base + 2]);
        } else {
            idx.extend_from_slice(&[base, base + 2, base + 1]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx));
    let _ = mesh.generate_tangents();
    mesh
}
