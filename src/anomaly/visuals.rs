//! How anomalies look, made to read from 20 m in fog and rain:
//!
//! - **Particles on the GPU** ([`ParticleMaterial`],
//!   `shaders/anomaly_particles.wgsl`): each system (a lift's leaves, a
//!   burner's embers and smoke, a snare's sparks, a well's grit, a
//!   mirror's glints) is one mesh of quads whose motion is a function of
//!   time and seed in the vertex shader: one draw each, however many.
//! - **Refraction** ([`HazeMaterial`], `shaders/anomaly_haze.wgsl`): the
//!   view behind shimmers through screen-space transmission: heat over a
//!   burner, a lift's draught, a mirror's fold, and a well's lens, which
//!   turns the world over at its heart.
//! - **Light in step**: one light each, and the flicker that drives it
//!   drives the particles' brightness and the haze's glow too, so a
//!   burner's embers surge with its light. Things thrown in (see
//!   `detector::Touched`) light an anomaly up for a few seconds.
//! - A snare's arcs are one mesh, rebuilt as they re-strike, and reach out
//!   to whatever it discharged into (metal in your hand first).
//!
//! Plus the glass eye's x-ray, which shows robots through walls while you
//! hold it to your eye, and the looks of the anomaly items (bolt,
//! detector, lodestone, mirror shard).

use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::{NoAutoAabb, VisibilityRange};
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPipeline, MaterialPipelineKey,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError, TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;
use extraction_core::anomaly::{AnomalyKind, WELL_HEIGHT};

use super::artifacts::ArtifactEffects;
use super::detector::{DetectorClick, REVEAL_TIME, Touched};
use super::mirrors::{Blinked, MIRROR_HEIGHT};
use super::{Anomaly, AnomalyAct, AnomalyEvent, ArcStrike};
use crate::items::{Catalog, CustomVisual, Item};
use crate::rig::{LocalPlayer, RigHead};
use crate::robots::RobotPart;

/// A tell's colour (`render::tells`, where it's checked against colour
/// blindness).
fn tell(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

pub struct AnomalyVisualsPlugin;

impl Plugin for AnomalyVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialPlugin::<XrayMaterial>::default(),
            MaterialPlugin::<ParticleMaterial>::default(),
            MaterialPlugin::<HazeMaterial>::default(),
        ))
        .add_observer(claim_item_looks)
        .add_systems(Startup, init_assets)
        .add_systems(
            Update,
            (
                (dress_anomalies, dress_scars).run_if(|| !hidden()),
                (react_to_events, pulse, snare_arcs).chain(),
                blink_flashes,
                dress_items,
                detector_lamps,
                stone_lamps,
                bear_screens,
                xray_parts,
                glass_eye,
            ),
        );
    }
}

/// `EXTRACTION_HIDE_ANOMALIES` set: the anomalies keep working but aren't
/// dressed, so a shot of the same view with and without them measures what
/// their looks add (`scripts/anomaly_legibility.py`).
fn hidden() -> bool {
    static HIDE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HIDE.get_or_init(|| std::env::var_os("EXTRACTION_HIDE_ANOMALIES").is_some())
}

// --- Materials -------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct ParticleParams {
    /// x: mode (see the shader), y: radius, z: reach, w: glow (additive).
    pub mode: Vec4,
    /// x: charge, y: revealed 0..1, z: flash 0..1, w: base size.
    pub state: Vec4,
    /// rgb: colour, a: opacity.
    pub color: Vec4,
}

/// One anomaly particle system: a mesh of quads moved in the shader.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct ParticleMaterial {
    #[uniform(0)]
    pub params: ParticleParams,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
    /// Additive (glowing) or blended.
    pub additive: bool,
}

impl Material for ParticleMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/anomaly_particles.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/anomaly_particles.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        if self.additive {
            AlphaMode::Add
        } else {
            AlphaMode::Blend
        }
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_1.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex];
        descriptor.primitive.cull_mode = None;
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct HazeParams {
    /// x: 0 card, 1 column, 2 the mesh as it is; y: card centre's lift;
    /// z: index of refraction; w: thickness.
    pub shape: Vec4,
    /// x: amplitude, y: scale (1/m), z: speed, w: swirl (0 rising, 1 circling).
    pub wave: Vec4,
    /// rgb: glow at the heart, a: how much.
    pub tint: Vec4,
    /// x: how much colour it drains from what's seen through it (a Hush's
    /// dead air), y: how much it dims it.
    pub drain: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct HazeExt {
    #[uniform(100)]
    pub haze: HazeParams,
}

/// Refraction: Bevy's screen-space transmission, bent by a moving normal.
pub type HazeMaterial = ExtendedMaterial<StandardMaterial, HazeExt>;

impl MaterialExtension for HazeExt {
    fn vertex_shader() -> ShaderRef {
        "shaders/anomaly_haze.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/anomaly_haze.wgsl".into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // A card in the air must not hide what's behind it from the
        // particles drawn after it.
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_write_enabled = Some(false);
        }
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

fn haze_material(ior: f32, thickness: f32, mut haze: HazeParams) -> HazeMaterial {
    haze.shape.z = ior;
    haze.shape.w = thickness;
    ExtendedMaterial {
        // Only what puts it in the transmissive pass; the shader does the
        // rest.
        base: StandardMaterial {
            base_color: Color::WHITE,
            specular_transmission: 1.0,
            ior,
            thickness,
            ..default()
        },
        extension: HazeExt { haze },
    }
}

// --- Assets ----------------------------------------------------------------

#[derive(Resource)]
struct AnomalyAssets {
    card: Handle<Mesh>,
    lens: Handle<Mesh>,
    core: Handle<Mesh>,
    leaf: Handle<Image>,
    radial: Handle<Image>,
    dot: Handle<Image>,
    scorch: Handle<StandardMaterial>,
    flattened: Handle<StandardMaterial>,
    glassy: Handle<StandardMaterial>,
    crushed: Handle<StandardMaterial>,
    hushed: Handle<StandardMaterial>,
    hush_ring: Handle<StandardMaterial>,
    dragged: Handle<StandardMaterial>,
    spark: Handle<StandardMaterial>,
    void: Handle<StandardMaterial>,
}

/// A soft round alpha falloff, for ground marks and soft particles.
fn radial_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let n = 128u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            // Ragged edge: a few lobes.
            let a = v.atan2(u);
            let edge = 0.82 + 0.08 * (a * 5.0).sin() + 0.05 * (a * 11.0 + 1.3).sin();
            let alpha = (1.0 - ((r - edge * 0.55) / (edge * 0.45)).clamp(0.0, 1.0)).powf(1.6);
            data.extend_from_slice(&[255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
    images.add(Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

/// A ring, soft both sides, round the outer tenth: the edge of a Hush
/// drawn on the ground.
fn ring_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let n = 128u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let alpha = (1.0 - ((r - 0.91) / 0.05).abs()).clamp(0.0, 1.0).powf(0.7);
            data.extend_from_slice(&[255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
    images.add(Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

/// A soft round dot (no lobes), for glowing particles.
fn dot_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let n = 64u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let r2 = u * u + v * v;
            // A hot centre in a soft halo.
            let a = ((-r2 * 9.0).exp() + 0.35 * (-r2 * 2.5).exp()) * (1.0 - r2).max(0.0);
            data.extend_from_slice(&[255, 255, 255, (a.min(1.0) * 255.0) as u8]);
        }
    }
    images.add(Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

fn init_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    server: Res<AssetServer>,
) {
    let leaf: Handle<Image> = server
        .load_builder()
        .with_settings(|s: &mut bevy::image::ImageLoaderSettings| s.is_srgb = true)
        .load("textures/foliage/birch_leaves/albedo.ktx2");
    let radial = radial_texture(&mut images);
    let dot = dot_texture(&mut images);
    let mut ground = |c: Color, emissive: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: c,
            base_color_texture: Some(radial.clone()),
            emissive,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.95,
            depth_bias: 50.0,
            ..default()
        })
    };
    let scorch = ground(
        Color::srgba(0.02, 0.018, 0.015, 0.92),
        LinearRgba::rgb(0.14, 0.02, 0.0),
    );
    let flattened = ground(Color::srgba(0.08, 0.07, 0.05, 0.6), LinearRgba::BLACK);
    let glassy = ground(
        Color::srgba(0.22, 0.26, 0.32, 0.55),
        LinearRgba::rgb(0.0, 0.02, 0.06),
    );
    let hushed = ground(Color::srgba(0.7, 0.7, 0.68, 0.6), LinearRgba::BLACK);
    // Torn turf, the dark earth under it showing.
    let dragged = ground(Color::srgba(0.11, 0.08, 0.05, 0.8), LinearRgba::BLACK);
    let crushed = ground(
        Color::srgba(0.03, 0.025, 0.035, 0.85),
        LinearRgba::rgb(0.02, 0.0, 0.05),
    );
    // Its edge, pale and firm: a line on dark wet ground you can step up to.
    let ring = ring_texture(&mut images);
    let hush_ring = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.85, 0.82, 0.85),
        base_color_texture: Some(ring),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.95,
        depth_bias: 60.0,
        ..default()
    });
    let a = AnomalyAssets {
        card: meshes.add(Rectangle::new(1.0, 1.0)),
        lens: meshes.add(Sphere::new(1.0).mesh().uv(32, 18)),
        core: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        leaf,
        radial,
        dot,
        scorch,
        flattened,
        glassy,
        crushed,
        hushed,
        hush_ring,
        dragged,
        spark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.8, 1.0),
            emissive: LinearRgba::rgb(10.0, 22.0, 60.0),
            unlit: true,
            cull_mode: None,
            ..default()
        }),
        void: materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(0.15, 0.04, 0.35),
            perceptual_roughness: 0.2,
            reflectance: 0.9,
            ..default()
        }),
    };
    commands.insert_resource(a);
}

/// A mesh of `n` quads for the particle shader: corners in POSITION, the
/// quad's UVs in UV_0, its seeds in UV_1.
fn particle_mesh(n: usize, rng: &mut extraction_core::noise::Rng) -> Mesh {
    let mut pos = Vec::with_capacity(n * 4);
    let mut uv = Vec::with_capacity(n * 4);
    let mut seed = Vec::with_capacity(n * 4);
    let mut idx = Vec::with_capacity(n * 6);
    for i in 0..n {
        let s = [rng.f32(), rng.f32()];
        for (x, y) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
            pos.push([x, y, 0.0]);
            uv.push([x + 0.5, 0.5 - y]);
            seed.push(s);
        }
        let k = (i * 4) as u32;
        idx.extend_from_slice(&[k, k + 1, k + 2, k, k + 2, k + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, seed)
    .with_inserted_indices(Indices::U32(idx))
}

/// A ground mark draped over the terrain round `centre` (the anomaly's
/// position): a grid whose every vertex sits just above the ground, so a
/// slope can't bury half a flat disc and cut it off in a hard line.
fn draped_disc(zone: &crate::world::Zone, centre: Vec3, half: f32) -> Mesh {
    let n = 12;
    let below = crate::world::underground::floor_under(zone, centre).is_some();
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut nrm = Vec::new();
    let mut idx = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            let (x, z) = ((u * 2.0 - 1.0) * half, (v * 2.0 - 1.0) * half);
            // Underground, on the site's floor.
            let y = if below {
                0.04
            } else {
                zone.ground(centre.x + x, centre.z + z) - centre.y + 0.04
            };
            pos.push([x, y, z]);
            uv.push([u, v]);
            nrm.push([0.0, 1.0, 0.0]);
        }
    }
    for j in 0..n {
        for i in 0..n {
            let k = (j * (n + 1) + i) as u32;
            let r = (n + 1) as u32;
            idx.extend_from_slice(&[k, k + r, k + 1, k + 1, k + r, k + r + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

// --- Dressing --------------------------------------------------------------

/// A drag scar: a smear draped over the ground from where the anomaly sat
/// last cycle to where it sits now, as wide as it is, soft at the edges.
fn dress_scars(
    mut commands: Commands,
    added: Query<(Entity, &super::DragScar, &Transform), Added<super::DragScar>>,
    a: Res<AnomalyAssets>,
    zone: Res<crate::world::Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (e, scar, at) in &added {
        let mesh = draped_strip(&zone, at.translation, scar.from, scar.radius * 0.8);
        commands.entity(e).insert((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(a.dragged.clone()),
            NotShadowCaster,
            VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: 75.0..90.0,
                use_aabb: false,
            },
        ));
    }
}

/// A strip draped over the terrain from `from` to `to` (the entity's
/// position), `half` wide either side, reaching `half` past both ends.
fn draped_strip(zone: &crate::world::Zone, to: Vec3, from: Vec3, half: f32) -> Mesh {
    let (along, across) = (16, 4);
    let back = Vec2::new(from.x - to.x, from.z - to.z);
    let len = back.length().max(0.01);
    let dir = back / len;
    let side = Vec2::new(-dir.y, dir.x);
    let mut pos = Vec::new();
    let mut uv = Vec::new();
    let mut nrm = Vec::new();
    let mut idx = Vec::new();
    for j in 0..=along {
        for i in 0..=across {
            let (u, v) = (i as f32 / across as f32, j as f32 / along as f32);
            let p = dir * (v * (len + half * 2.0) - half) + side * (u * 2.0 - 1.0) * half;
            let y = zone.ground(to.x + p.x, to.z + p.y) - to.y + 0.05;
            pos.push([p.x, y, p.y]);
            uv.push([u, v]);
            nrm.push([0.0, 1.0, 0.0]);
        }
    }
    for j in 0..along {
        for i in 0..across {
            let k = (j * (across + 1) + i) as u32;
            let r = (across + 1) as u32;
            idx.extend_from_slice(&[k, k + r, k + 1, k + 1, k + r, k + r + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

/// How an anomaly looks this frame: what its light, particles and haze
/// all follow.
#[derive(Component, Debug, Default)]
pub struct Look {
    /// The kind's own flicker, 0..1 about a mean of ~0.6.
    pub flicker: f32,
    /// A burst (a discharge, a pop, a blink), decaying to 0.
    pub flash: f32,
    /// Shown up by something thrown in, 0..1.
    pub reveal: f32,
}

/// A particle system of an anomaly: its colour and opacity at rest.
#[derive(Component)]
struct Particles {
    color: Vec3,
    alpha: f32,
}

/// An anomaly's light, at rest.
#[derive(Component)]
struct Glow {
    base: f32,
    /// How much a flash adds, times `base` (or absolute for a dark one).
    burst: f32,
}

/// An anomaly's haze, at rest.
#[derive(Component)]
struct Haze {
    amp: f32,
    /// Extra amplitude at full charge (a lift winding up).
    charged: f32,
    tint: f32,
}

/// A snare's arcs: one mesh, re-struck every few frames.
#[derive(Component, Default)]
struct Arcs {
    timer: f32,
    /// Arcs out to something it struck: target (local), seconds left.
    reaching: Vec<(Vec3, f32)>,
}

const ARC_SEGMENTS: usize = 7;
const ARCS: usize = 5;

struct System {
    mode: f32,
    count: usize,
    size: f32,
    texture: Handle<Image>,
    additive: bool,
    color: Vec3,
    alpha: f32,
}

#[allow(clippy::too_many_arguments)]
fn dress_anomalies(
    mut commands: Commands,
    added: Query<(Entity, &Anomaly, &Transform), Added<Anomaly>>,
    a: Res<AnomalyAssets>,
    zone: Res<crate::world::Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut particles: ResMut<Assets<ParticleMaterial>>,
    mut hazes: ResMut<Assets<HazeMaterial>>,
) {
    let near = |end: f32| VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: end - 15.0..end,
        use_aabb: false,
    };
    for (e, anomaly, at) in &added {
        let r = anomaly.radius;
        let kind = anomaly.kind;
        let reach = kind.reach(r);
        let mut rng = extraction_core::noise::Rng::new(e.to_bits());
        let leaf = |count, size| System {
            mode: 0.0,
            count,
            size,
            texture: a.leaf.clone(),
            additive: false,
            color: Vec3::new(0.85, 0.7, 0.45),
            alpha: 1.0,
        };
        let soft = |mode, count, size, color: Vec3, alpha| System {
            mode,
            count,
            size,
            texture: a.radial.clone(),
            additive: false,
            color,
            alpha,
        };
        let glow = |mode, count, size, color: Vec3| System {
            mode,
            count,
            size,
            texture: a.dot.clone(),
            additive: true,
            color,
            alpha: 1.0,
        };
        let (mark, systems, light, haze) = match kind {
            AnomalyKind::Lift => (
                Some(&a.flattened),
                vec![
                    leaf(90, 0.2),
                    soft(6.0, 26, 0.7, Vec3::new(0.32, 0.27, 0.2), 0.22),
                ],
                (tell(crate::render::tells::LIFT), 0.0, 60_000.0, 1.6),
                Some((
                    1.15,
                    0.6,
                    HazeParams {
                        shape: Vec4::new(1.0, 2.0, 0.0, 0.0),
                        wave: Vec4::new(0.06, 1.0, 0.8, 1.0),
                        tint: Vec4::ZERO,
                        drain: Vec4::ZERO,
                    },
                    Vec3::new(r * 2.2, 4.5, 1.0),
                    0.4,
                )),
            ),
            AnomalyKind::Burner => (
                Some(&a.scorch),
                vec![
                    glow(1.0, 70, 0.055, Vec3::new(16.0, 4.0, 0.7)),
                    soft(2.0, 14, 1.0, Vec3::new(0.12, 0.11, 0.1), 0.32),
                ],
                (tell(crate::render::tells::BURNER), 35_000.0, 3.0, 0.5),
                Some((
                    1.2,
                    0.6,
                    HazeParams {
                        shape: Vec4::new(1.0, 2.4, 0.0, 0.0),
                        wave: Vec4::new(0.5, 1.6, 1.4, 0.0),
                        tint: Vec4::new(0.6, 0.15, 0.02, 0.0),
                        drain: Vec4::ZERO,
                    },
                    Vec3::new(r * 2.2, 5.0, 1.0),
                    0.0,
                )),
            ),
            AnomalyKind::Snare => (
                Some(&a.glassy),
                vec![glow(3.0, 30, 0.03, Vec3::new(5.0, 9.0, 20.0))],
                (tell(crate::render::tells::SNARE), 12_000.0, 12.0, 0.2),
                Some((
                    1.1,
                    0.4,
                    HazeParams {
                        shape: Vec4::new(0.0, 0.8, 0.0, 0.0),
                        wave: Vec4::new(0.08, 2.5, 3.0, 0.0),
                        tint: Vec4::new(0.1, 0.2, 0.6, 0.0),
                        drain: Vec4::ZERO,
                    },
                    Vec3::new(r * 1.4, r * 1.4, 1.0),
                    0.0,
                )),
            ),
            AnomalyKind::Well => (
                Some(&a.crushed),
                vec![
                    soft(4.0, 140, 0.1, Vec3::new(0.09, 0.08, 0.085), 1.0),
                    // Motes of light falling in with the grit: what reads
                    // through fog and rain.
                    glow(4.0, 36, 0.035, Vec3::new(5.0, 1.8, 12.0)),
                    soft(6.0, 20, 0.6, Vec3::new(0.2, 0.18, 0.2), 0.18),
                ],
                (tell(crate::render::tells::WELL), 4_000.0, 4.0, 1.0),
                Some((
                    1.33,
                    r * 0.7,
                    HazeParams {
                        shape: Vec4::new(2.0, 0.0, 0.0, 0.0),
                        wave: Vec4::new(0.04, 1.5, 0.5, 1.0),
                        tint: Vec4::ZERO,
                        drain: Vec4::ZERO,
                    },
                    Vec3::splat(r * 0.45),
                    0.0,
                )),
            ),
            AnomalyKind::Mirror => (
                None,
                vec![glow(5.0, 40, 0.035, Vec3::new(6.0, 10.0, 14.0))],
                (tell(crate::render::tells::MIRROR), 0.0, 40_000.0, 0.0),
                Some((
                    1.33,
                    0.8,
                    HazeParams {
                        shape: Vec4::new(0.0, MIRROR_HEIGHT, 0.0, 0.0),
                        wave: Vec4::new(0.3, 0.9, 0.6, 1.0),
                        tint: Vec4::new(0.3, 0.5, 0.7, 0.0),
                        drain: Vec4::ZERO,
                    },
                    Vec3::new(r * 2.2, r * 2.2, 1.0),
                    0.0,
                )),
            ),
            // Dead air: pale dust hanging still, a ground drained of colour,
            // and no light of its own.
            AnomalyKind::Hush => (
                Some(&a.hushed),
                vec![soft(7.0, 60, 0.1, Vec3::new(0.85, 0.85, 0.82), 0.9)],
                (Color::srgb(0.8, 0.8, 0.85), 0.0, 25_000.0, 1.0),
                // Seen through, the world inside goes dim (and greyer),
                // inside a slow-pulsing bright rim: luminance and motion,
                // which carry in fog and to colour-blind eyes.
                Some((
                    1.02,
                    0.2,
                    HazeParams {
                        shape: Vec4::new(0.0, 1.2, 0.0, 0.0),
                        wave: Vec4::new(0.02, 0.5, 0.1, 1.0),
                        tint: Vec4::ZERO,
                        drain: Vec4::new(0.6, 0.5, 1.2, 1.1),
                    },
                    Vec3::new(r * 2.3, r * 2.3, 1.0),
                    0.0,
                )),
            ),
        };
        commands
            .entity(e)
            .insert(Look::default())
            .with_children(|c| {
                if let Some(mark) = mark {
                    let disc = meshes.add(draped_disc(&zone, at.translation, r * 1.1));
                    c.spawn((
                        Mesh3d(disc),
                        MeshMaterial3d(mark.clone()),
                        Transform::IDENTITY,
                        NotShadowCaster,
                        near(90.0),
                    ));
                }
                if kind == AnomalyKind::Hush {
                    let disc = meshes.add(draped_disc(&zone, at.translation, r * 1.1));
                    c.spawn((
                        Mesh3d(disc),
                        MeshMaterial3d(a.hush_ring.clone()),
                        Transform::IDENTITY,
                        NotShadowCaster,
                        near(90.0),
                    ));
                }
                for s in systems {
                    let mesh = meshes.add(particle_mesh(s.count, &mut rng));
                    let ext = reach * 1.9 + s.size * 3.0 + 2.0;
                    c.spawn((
                        Particles {
                            color: s.color,
                            alpha: s.alpha,
                        },
                        Mesh3d(mesh),
                        MeshMaterial3d(particles.add(ParticleMaterial {
                            params: ParticleParams {
                                mode: Vec4::new(s.mode, r, reach, s.additive as u8 as f32),
                                state: Vec4::new(anomaly.charge, 0.0, 0.0, s.size),
                                color: s.color.extend(s.alpha),
                            },
                            texture: s.texture,
                            additive: s.additive,
                        })),
                        Transform::IDENTITY,
                        Aabb::from_min_max(Vec3::new(-ext, -0.5, -ext), Vec3::new(ext, 7.0, ext)),
                        NoAutoAabb,
                        NotShadowCaster,
                        near(95.0),
                    ));
                }
                let (color, base, burst, height): (Color, f32, f32, f32) = light;
                c.spawn((
                    Glow { base, burst },
                    PointLight {
                        color,
                        intensity: base,
                        range: r * 4.0 + 5.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, height.max(0.4), 0.0),
                    if base > 0.0 {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                ));
                if let Some((ior, thickness, params, scale, charged)) = haze {
                    let mesh = if params.shape.x > 1.5 {
                        a.lens.clone()
                    } else {
                        a.card.clone()
                    };
                    let at = if params.shape.x > 1.5 {
                        Vec3::Y * WELL_HEIGHT
                    } else {
                        Vec3::ZERO
                    };
                    let ext = scale.x.max(scale.y) + params.shape.y + 1.0;
                    c.spawn((
                        Haze {
                            amp: params.wave.x,
                            charged,
                            tint: params.tint.w,
                        },
                        Mesh3d(mesh),
                        MeshMaterial3d(hazes.add(haze_material(ior, thickness, params))),
                        Transform::from_translation(at).with_scale(scale),
                        Aabb::from_min_max(Vec3::splat(-ext) / scale, Vec3::splat(ext) / scale),
                        NoAutoAabb,
                        NotShadowCaster,
                        near(55.0),
                    ));
                }
                match kind {
                    AnomalyKind::Well => {
                        // The heart: a dark knot that drinks the light.
                        c.spawn((
                            Mesh3d(a.core.clone()),
                            MeshMaterial3d(a.void.clone()),
                            Transform::from_xyz(0.0, WELL_HEIGHT, 0.0)
                                .with_scale(Vec3::splat(0.05 + r * 0.015)),
                            NotShadowCaster,
                            near(90.0),
                        ));
                    }
                    AnomalyKind::Snare => {
                        let mesh = meshes.add(arc_mesh(&[]));
                        let ext = r + super::SNARE_METAL_REACH + 1.0;
                        c.spawn((
                            Arcs::default(),
                            Mesh3d(mesh),
                            MeshMaterial3d(a.spark.clone()),
                            Transform::from_xyz(0.0, 0.8, 0.0),
                            Aabb::from_min_max(Vec3::splat(-ext), Vec3::splat(ext)),
                            NoAutoAabb,
                            NotShadowCaster,
                            near(90.0),
                        ));
                    }
                    _ => {}
                }
            });
    }
}

/// Where the eye is (the flat camera, else the local player's head).
fn eye_position(
    heads: &Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: &Query<(), With<LocalPlayer>>,
    cams: &Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
) -> Option<Vec3> {
    cams.iter().next().map(|g| g.translation()).or_else(|| {
        heads
            .iter()
            .find(|(_, p)| locals.contains(p.parent()))
            .map(|(g, _)| g.translation())
    })
}

/// A kind's flicker at time `t` (0..1, about 0.6 on average): what its
/// light, embers and haze all follow.
pub fn flicker(kind: AnomalyKind, t: f32, charge: f32, seed: f32) -> f32 {
    let t = t + seed * 17.0;
    let v = match kind {
        // Coals: slow surges with quick gutters on top.
        AnomalyKind::Burner => {
            0.5 + 0.3 * (t * 2.3).sin() * (t * 0.9).sin()
                + 0.15 * (t * 13.0).sin() * (t * 7.3).sin()
                + 0.05 * (t * 31.0).sin()
        }
        // Steady, with a crackle now and then.
        AnomalyKind::Snare => {
            let tick = (t * 12.0).floor();
            let crack = ((tick * 12.9898).sin() * 43758.545).fract().abs();
            0.55 + 0.3 * charge + if crack > 0.93 { 0.4 } else { 0.0 }
        }
        // A slow, heavy throb.
        AnomalyKind::Well => 0.35 + 0.65 * (0.5 + 0.5 * (t * 1.1).sin()).powi(4),
        AnomalyKind::Lift => 0.3 + 0.7 * charge,
        AnomalyKind::Mirror => 0.5 + 0.1 * (t * 0.7).sin(),
        AnomalyKind::Hush => 0.5,
    };
    v.clamp(0.0, 1.0)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn pulse(
    mut anomalies: Query<(
        Entity,
        &Anomaly,
        &Touched,
        &mut Look,
        &GlobalTransform,
        &Children,
    )>,
    mut glows: Query<(&Glow, &mut PointLight, &mut Visibility)>,
    systems: Query<(&Particles, &MeshMaterial3d<ParticleMaterial>)>,
    hazes: Query<(&Haze, &MeshMaterial3d<HazeMaterial>)>,
    mut particle_mats: ResMut<Assets<ParticleMaterial>>,
    mut haze_mats: ResMut<Assets<HazeMaterial>>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    agitation: Option<Res<super::surge::Agitation>>,
    time: Res<Time>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let eye = eye_position(&heads, &locals, &cams);
    let stirred = agitation.map_or(0.0, |a| a.0);
    for (e, a, touched, mut look, gt, children) in &mut anomalies {
        look.flash = (look.flash - dt * 2.5).max(0.0);
        look.reveal = (touched.revealed / REVEAL_TIME).clamp(0.0, 1.0);
        let seed = (e.index_u32() % 97) as f32 * 0.37;
        look.flicker = flicker(a.kind, t, a.charge, seed);
        // With the Surge coming they flare, each on its own beat, harder
        // and more often as it nears.
        if stirred > 0.0 {
            let beat = (t * (1.5 + 2.0 * stirred) + seed * 3.0).sin();
            look.flash = look.flash.max(stirred * 0.6 * beat.max(0.0).powi(3));
        }
        // Only the near ones get their materials touched.
        if eye.is_some_and(|p| p.distance(gt.translation()) > 100.0) {
            continue;
        }
        let lift = look.flicker + 2.5 * look.flash + 1.5 * look.reveal;
        for c in children.iter() {
            if let Ok((g, mut light, mut vis)) = glows.get_mut(c) {
                let i = if g.base > 0.0 {
                    g.base * (0.4 + 0.6 * look.flicker) * (1.0 + g.burst * look.flash)
                        + g.base * 2.0 * look.reveal
                } else {
                    g.burst * (look.flash + 0.5 * look.reveal)
                };
                light.intensity = i;
                let want = if i > 1.0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                vis.set_if_neq(want);
            }
            if let Ok((p, m)) = systems.get(c)
                && let Some(mut mat) = particle_mats.get_mut(&m.0)
            {
                mat.params.state.x = a.charge;
                // The Surge swells them.
                mat.params.mode.y = a.radius;
                mat.params.mode.z = a.kind.reach(a.radius);
                mat.params.state.y = look.reveal;
                mat.params.state.z = look.flash;
                let b = 0.45 + 0.55 * lift;
                let glowing = mat.params.mode.w > 0.5;
                mat.params.color = if glowing {
                    (p.color * b).extend(p.alpha)
                } else {
                    p.color.extend((p.alpha * (1.0 + look.reveal)).min(1.0))
                };
            }
            if let Ok((h, m)) = hazes.get(c)
                && let Some(mut mat) = haze_mats.get_mut(&m.0)
            {
                let amp = h.amp + h.charged * a.charge + 0.25 * look.flash + 0.3 * look.reveal;
                mat.extension.haze.wave.x = amp;
                mat.extension.haze.tint.w = h.tint * lift + 0.3 * look.reveal;
            }
        }
    }
}

fn react_to_events(
    mut events: MessageReader<AnomalyEvent>,
    mut strikes: MessageReader<ArcStrike>,
    mut looks: Query<(&mut Look, &GlobalTransform, &Children)>,
    mut arcs: Query<&mut Arcs>,
) {
    for ev in events.read() {
        let Ok((mut look, _, kids)) = looks.get_mut(ev.anomaly) else {
            continue;
        };
        look.flash = look.flash.max(match ev.what {
            AnomalyAct::Discharge | AnomalyAct::Pop | AnomalyAct::Blink => 1.0,
            AnomalyAct::Bloom => 0.8,
            AnomalyAct::Flare | AnomalyAct::Crush => 0.6,
            AnomalyAct::Reveal => 0.4,
        });
        if ev.what == AnomalyAct::Discharge {
            for c in kids.iter() {
                if let Ok(mut a) = arcs.get_mut(c) {
                    a.timer = 0.0;
                }
            }
        }
    }
    for s in strikes.read() {
        let Ok((_, gt, kids)) = looks.get(s.anomaly) else {
            continue;
        };
        for c in kids.iter() {
            if let Ok(mut a) = arcs.get_mut(c) {
                // Arcs' local space: 0.8 m over the anomaly.
                let local = s.to - gt.translation() - Vec3::Y * 0.8;
                a.reaching.push((local, 0.35));
                a.timer = 0.0;
            }
        }
    }
}

/// Ribbons along polylines, each segment two crossed quads so it reads
/// from any side without knowing where the eye is.
fn arc_mesh(paths: &[(Vec<Vec3>, f32)]) -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for (path, w) in paths {
        for seg in path.windows(2) {
            let (p0, p1) = (seg[0], seg[1]);
            let d = (p1 - p0).normalize_or(Vec3::Y);
            let s1 = d.any_orthonormal_vector() * *w;
            let s2 = d.cross(s1).normalize_or_zero() * *w;
            for side in [s1, s2] {
                let k = pos.len() as u32;
                for p in [p0 - side, p0 + side, p1 + side, p1 - side] {
                    pos.push(p.to_array());
                    nrm.push([0.0, 1.0, 0.0]);
                    uv.push([0.0, 0.0]);
                }
                idx.extend_from_slice(&[k, k + 1, k + 2, k, k + 2, k + 3]);
            }
        }
    }
    if pos.is_empty() {
        // Something to hold the buffers' shape.
        pos.extend([[0.0; 3]; 3]);
        nrm.extend([[0.0, 1.0, 0.0]; 3]);
        uv.extend([[0.0; 2]; 3]);
        idx.extend([0, 1, 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

/// One jagged polyline from `from` to `to`.
fn jag(rng: &mut extraction_core::noise::Rng, from: Vec3, to: Vec3, rough: f32) -> Vec<Vec3> {
    let len = from.distance(to);
    (0..=ARC_SEGMENTS)
        .map(|k| {
            let f = k as f32 / ARC_SEGMENTS as f32;
            let j = if k == 0 || k == ARC_SEGMENTS {
                Vec3::ZERO
            } else {
                Vec3::new(
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                    rng.range(-1.0, 1.0),
                ) * len
                    * rough
            };
            from.lerp(to, f) + j
        })
        .collect()
}

/// Arcs crawl about the snare's core, re-striking every few frames; when
/// it discharges they reach out to its full radius, and to whatever it
/// struck.
#[allow(clippy::too_many_arguments)]
fn snare_arcs(
    mut arcs: Query<(&mut Arcs, &Mesh3d, &ChildOf, &GlobalTransform)>,
    anomalies: Query<(&Anomaly, &Look)>,
    mut meshes: ResMut<Assets<Mesh>>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    time: Res<Time>,
    mut seed: Local<u64>,
) {
    let dt = time.delta_secs();
    let eye = eye_position(&heads, &locals, &cams);
    for (mut a, mesh, parent, gt) in &mut arcs {
        let Ok((anomaly, look)) = anomalies.get(parent.parent()) else {
            continue;
        };
        for r in &mut a.reaching {
            r.1 -= dt;
        }
        a.reaching.retain(|r| r.1 > 0.0);
        a.timer -= dt;
        if a.timer > 0.0 {
            continue;
        }
        a.timer = 0.05;
        *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut rng = extraction_core::noise::Rng::new(*seed);
        let hot = anomaly.charge.max(look.flash).max(look.reveal);
        let reach = anomaly.radius * (0.3 + 0.7 * look.flash) * (0.4 + 0.6 * hot);
        let mut paths = Vec::new();
        // Thicker far off: an arc a few millimetres across is gone in a
        // pixel by 20 m.
        let far = eye.map_or(1.0, |e| {
            (e.distance(gt.translation()) / 6.0).clamp(1.0, 4.0)
        });
        let width = 0.006 * (1.0 + 2.5 * look.flash) * far;
        for _ in 0..ARCS {
            if !rng.chance(0.25 + 0.75 * hot) {
                continue;
            }
            let dir = Vec3::new(
                rng.range(-1.0, 1.0),
                rng.range(-0.6, 1.0),
                rng.range(-1.0, 1.0),
            )
            .normalize_or(Vec3::Y);
            let main = jag(&mut rng, Vec3::ZERO, dir * reach, 0.12);
            // A fork off the middle now and then.
            if rng.chance(0.4) {
                let from = main[ARC_SEGMENTS / 2];
                let fork = from + (dir + Vec3::Y * 0.5).normalize() * reach * 0.4;
                paths.push((jag(&mut rng, from, fork, 0.15), width * 0.6));
            }
            paths.push((main, width));
        }
        for (to, _) in a.reaching.clone() {
            paths.push((jag(&mut rng, Vec3::ZERO, to, 0.1), 0.012 * far));
            paths.push((jag(&mut rng, Vec3::ZERO, to, 0.16), 0.007 * far));
        }
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            *m = arc_mesh(&paths);
        }
    }
}

/// A blink leaves a flash where it took something and where it set it
/// down.
#[derive(Component)]
struct BlinkFlash(f32);

fn blink_flashes(
    mut commands: Commands,
    mut blinks: MessageReader<Blinked>,
    mut flashes: Query<(Entity, &mut BlinkFlash, &mut PointLight)>,
    time: Res<Time>,
) {
    for b in blinks.read() {
        for at in [b.from, b.to] {
            commands.spawn((
                BlinkFlash(0.45),
                PointLight {
                    color: Color::srgb(0.6, 0.85, 1.0),
                    intensity: 60_000.0,
                    range: 8.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(at + Vec3::Y * 0.8),
            ));
        }
    }
    let dt = time.delta_secs();
    for (e, mut f, mut light) in &mut flashes {
        f.0 -= dt;
        light.intensity = 60_000.0 * (f.0 / 0.45).max(0.0).powi(2);
        if f.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

// --- The anomaly items' looks ------------------------------------------------

/// The items whose looks are built here rather than loaded.
const OWN_LOOKS: [&str; 8] = [
    "bolt",
    "detector",
    "art_lodestone",
    "art_shard",
    "art_stormglass",
    "detector_sweep",
    "detector_bear",
    "art_hushstone",
];

/// Claims the look of an anomaly item as it's spawned, before the item
/// placeholder system gives it a box.
fn claim_item_looks(
    add: On<Add, Item>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut commands: Commands,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    if OWN_LOOKS.contains(&catalog.get(item.kind()).id.as_str()) {
        commands.entity(add.entity).insert(CustomVisual);
    }
}

/// A detector's lamp, lit by each click.
#[derive(Component)]
struct Lamp {
    material: Handle<StandardMaterial>,
    lit: f32,
}

fn hex_prism(radius: f32, height: f32) -> Mesh {
    Cylinder::new(radius, height).mesh().resolution(6).build()
}

/// A thin wedge of glass: a triangle, extruded.
fn shard_mesh() -> Mesh {
    let tri = [
        Vec2::new(0.0, 0.055),
        Vec2::new(-0.022, -0.05),
        Vec2::new(0.025, -0.045),
    ];
    let t = 0.008;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for (z, n) in [(t, 1.0), (-t, -1.0)] {
        let k = pos.len() as u32;
        for p in tri {
            pos.push([p.x, p.y, z * 0.5]);
            nrm.push([0.0, 0.0, n]);
        }
        if n > 0.0 {
            idx.extend([k, k + 1, k + 2]);
        } else {
            idx.extend([k, k + 2, k + 1]);
        }
    }
    for i in 0..3 {
        let (a, b) = (tri[i], tri[(i + 1) % 3]);
        let d = b - a;
        let n = Vec3::new(d.y, -d.x, 0.0).normalize();
        let k = pos.len() as u32;
        for (p, z) in [(a, t), (b, t), (b, -t), (a, -t)] {
            pos.push([p.x, p.y, z * 0.5]);
            nrm.push(n.to_array());
        }
        idx.extend([k, k + 2, k + 1, k, k + 3, k + 2]);
    }
    let uv = vec![[0.0f32, 0.0]; pos.len()];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

#[allow(clippy::type_complexity)]
fn dress_items(
    mut commands: Commands,
    added: Query<(Entity, &Item), (Added<CustomVisual>, With<Item>)>,
    catalog: Res<Catalog>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, item) in &added {
        let id = catalog.get(item.kind()).id.as_str();
        let mut mat = |c: Color, metal: f32, rough: f32, emissive: LinearRgba| {
            materials.add(StandardMaterial {
                base_color: c,
                metallic: metal,
                perceptual_roughness: rough,
                emissive,
                ..default()
            })
        };
        let mut parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)> = Vec::new();
        let mut lamp = None;
        let mut stone_lamp = None;
        let mut screen = false;
        match id {
            "bolt" => {
                let steel = mat(Color::srgb(0.36, 0.3, 0.25), 0.7, 0.65, LinearRgba::BLACK);
                let along = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
                parts.push((
                    meshes.add(Cylinder::new(0.005, 0.06).mesh().resolution(10)),
                    steel.clone(),
                    Transform::from_xyz(0.0, 0.0, 0.004).with_rotation(along),
                ));
                parts.push((
                    meshes.add(hex_prism(0.0085, 0.007)),
                    steel.clone(),
                    Transform::from_xyz(0.0, 0.0, -0.0315).with_rotation(along),
                ));
                parts.push((
                    meshes.add(hex_prism(0.0085, 0.008)),
                    steel,
                    Transform::from_xyz(0.0, 0.0, 0.018).with_rotation(along),
                ));
            }
            "detector" | "detector_sweep" | "detector_bear" => {
                // The Authority's Bear is grey; the survey sets olive.
                let body = if id == "detector_bear" {
                    Color::srgb(0.32, 0.33, 0.34)
                } else {
                    Color::srgb(0.36, 0.4, 0.24)
                };
                let olive = mat(body, 0.15, 0.6, LinearRgba::BLACK);
                let face = mat(Color::srgb(0.85, 0.82, 0.7), 0.0, 0.4, LinearRgba::BLACK);
                let black = mat(Color::srgb(0.04, 0.04, 0.04), 0.3, 0.5, LinearRgba::BLACK);
                let glass = mat(
                    Color::srgb(0.15, 0.03, 0.02),
                    0.0,
                    0.2,
                    LinearRgba::rgb(0.2, 0.02, 0.0),
                );
                parts.push((
                    meshes.add(Cuboid::new(0.07, 0.04, 0.11)),
                    olive,
                    Transform::from_xyz(0.0, 0.0, 0.025),
                ));
                // A dial and a grille on top, a carrying handle along the
                // side.
                // (The Bear's screen takes the dial's place.)
                if id != "detector_bear" {
                    parts.push((
                        meshes.add(Cylinder::new(0.016, 0.003).mesh().resolution(20)),
                        face,
                        Transform::from_xyz(0.012, 0.021, 0.05),
                    ));
                }
                parts.push((
                    meshes.add(Cuboid::new(0.03, 0.004, 0.03)),
                    black.clone(),
                    Transform::from_xyz(-0.015, 0.021, 0.005),
                ));
                parts.push((
                    meshes.add(Cuboid::new(0.012, 0.012, 0.09)),
                    black.clone(),
                    Transform::from_xyz(-0.041, 0.0, 0.025),
                ));
                parts.push((
                    meshes.add(Cylinder::new(0.004, 0.05).mesh().resolution(8)),
                    black.clone(),
                    Transform::from_xyz(0.018, 0.0, -0.055)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ));
                parts.push((
                    meshes.add(Sphere::new(0.006).mesh().ico(1).unwrap()),
                    black.clone(),
                    Transform::from_xyz(0.018, 0.0, -0.078),
                ));
                parts.push((
                    meshes.add(Sphere::new(0.005).mesh().ico(1).unwrap()),
                    glass.clone(),
                    Transform::from_xyz(-0.02, 0.021, -0.01),
                ));
                lamp = Some(glass);
                if id == "detector_sweep" {
                    // A long whip antenna, and a green lamp for stones.
                    parts.push((
                        meshes.add(Cylinder::new(0.0025, 0.16).mesh().resolution(6)),
                        black.clone(),
                        Transform::from_xyz(0.018, 0.0, -0.16)
                            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ));
                    let green = mat(
                        Color::srgb(0.02, 0.12, 0.03),
                        0.0,
                        0.2,
                        LinearRgba::rgb(0.0, 0.1, 0.02),
                    );
                    parts.push((
                        meshes.add(Sphere::new(0.005).mesh().ico(1).unwrap()),
                        green.clone(),
                        Transform::from_xyz(-0.03, 0.021, -0.01),
                    ));
                    stone_lamp = Some(green);
                }
                if id == "detector_bear" {
                    // A round screen on top, facing up and back at you.
                    parts.push((
                        meshes.add(Cylinder::new(0.03, 0.004).mesh().resolution(32)),
                        mat(
                            Color::srgb(0.01, 0.03, 0.015),
                            0.0,
                            0.15,
                            LinearRgba::rgb(0.0, 0.02, 0.005),
                        ),
                        Transform::from_xyz(0.0, 0.022, 0.035),
                    ));
                    screen = true;
                }
            }
            "art_lodestone" => {
                let iron = mat(
                    Color::srgb(0.16, 0.15, 0.17),
                    0.9,
                    0.5,
                    LinearRgba::rgb(0.004, 0.0, 0.01),
                );
                let mut m = Sphere::new(1.0).mesh().ico(2).unwrap();
                // Knobbled, like a lump of slag.
                if let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
                    m.attribute_mut(Mesh::ATTRIBUTE_POSITION)
                {
                    for v in p.iter_mut() {
                        let q = Vec3::from_array(*v);
                        let k = 1.0
                            + 0.12 * (q.x * 7.0 + q.y * 3.0).sin() * (q.z * 5.0).cos()
                            + 0.06 * (q.y * 13.0).sin();
                        *v = (q * k).to_array();
                    }
                }
                m.compute_smooth_normals();
                parts.push((
                    meshes.add(m),
                    iron,
                    Transform::from_scale(Vec3::new(0.028, 0.023, 0.026)),
                ));
            }
            "art_shard" => {
                let glass = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.85, 0.9, 0.97),
                    metallic: 1.0,
                    perceptual_roughness: 0.04,
                    emissive: LinearRgba::rgb(0.03, 0.06, 0.12),
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                });
                parts.push((meshes.add(shard_mesh()), glass, Transform::IDENTITY));
            }
            "art_hushstone" => {
                // A flat grey pebble, matte as felt.
                let felt = mat(Color::srgb(0.42, 0.42, 0.4), 0.0, 0.98, LinearRgba::BLACK);
                parts.push((
                    meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
                    felt,
                    Transform::from_scale(Vec3::new(0.025, 0.012, 0.02)),
                ));
            }
            "art_stormglass" => {
                // A clouded bead: milky glass with a cold light in it.
                let glass = materials.add(StandardMaterial {
                    base_color: Color::srgba(0.82, 0.86, 0.92, 0.85),
                    perceptual_roughness: 0.25,
                    emissive: LinearRgba::rgb(0.05, 0.08, 0.14),
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                });
                parts.push((
                    meshes.add(Sphere::new(0.02).mesh().ico(3).unwrap()),
                    glass,
                    Transform::IDENTITY,
                ));
            }
            _ => continue,
        }
        commands.entity(e).insert(Visibility::default());
        if let Some(material) = lamp {
            commands.entity(e).insert(Lamp { material, lit: 0.0 });
        }
        if let Some(material) = stone_lamp {
            commands.entity(e).insert(StoneLamp { material, lit: 0.0 });
        }
        if screen {
            // A pool of blips on the screen, placed by `bear_screens`.
            let dot = meshes.add(Cylinder::new(0.0022, 0.001).mesh().resolution(8));
            let stone = materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: LinearRgba::rgb(2.0, 8.0, 2.5),
                unlit: true,
                ..default()
            });
            let ring = materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: LinearRgba::rgb(0.3, 1.6, 0.5),
                unlit: true,
                ..default()
            });
            let mut blips = Vec::new();
            commands.entity(e).with_children(|c| {
                for i in 0..BEAR_BLIPS {
                    let m = if i < BEAR_BLIPS / 2 { &stone } else { &ring };
                    blips.push(
                        c.spawn((
                            Mesh3d(dot.clone()),
                            MeshMaterial3d(m.clone()),
                            Transform::from_xyz(0.0, 0.025, 0.035),
                            Visibility::Hidden,
                        ))
                        .id(),
                    );
                }
            });
            commands.entity(e).insert(BearBlips(blips));
        }
        commands.entity(e).with_children(|c| {
            for (mesh, material, tf) in parts {
                c.spawn((Mesh3d(mesh), MeshMaterial3d(material), tf));
            }
        });
    }
}

/// A sweep's green lamp, lit by each stone ping.
#[derive(Component)]
struct StoneLamp {
    material: Handle<StandardMaterial>,
    lit: f32,
}

/// Blips on a Bear's screen: the first half stones, the rest anomalies.
#[derive(Component)]
struct BearBlips(Vec<Entity>);

const BEAR_BLIPS: usize = 24;
/// The screen's radius and centre, item space.
const SCREEN_R: f32 = 0.027;
const SCREEN_AT: Vec3 = Vec3::new(0.0, 0.0245, 0.035);

/// Places a held Bear's blips: stones as bright dots, anomalies as dim
/// ones sized by their reach. Up the screen is the way you face (-Z).
fn bear_screens(
    screens: Query<(&super::detector::BearScreen, &BearBlips)>,
    mut blips: Query<(&mut Transform, &mut Visibility)>,
) {
    for (screen, pool) in &screens {
        let half = pool.0.len() / 2;
        let place = |i: usize,
                     at: Option<([f32; 2], f32)>,
                     blips: &mut Query<(&mut Transform, &mut Visibility)>| {
            if let Ok((mut tf, mut vis)) = blips.get_mut(pool.0[i]) {
                match at {
                    Some((p, size)) => {
                        tf.translation =
                            SCREEN_AT + Vec3::new(p[0] * SCREEN_R, 0.0, -p[1] * SCREEN_R);
                        tf.scale = Vec3::splat(size);
                        vis.set_if_neq(Visibility::Inherited);
                    }
                    None => {
                        vis.set_if_neq(Visibility::Hidden);
                    }
                }
            }
        };
        for i in 0..half {
            place(i, screen.stones.get(i).map(|p| (*p, 1.0)), &mut blips);
        }
        for i in 0..pool.0.len() - half {
            let a = screen
                .anomalies
                .get(i)
                .map(|(p, r)| (*p, (1.0 + r * 8.0).min(4.0)));
            place(half + i, a, &mut blips);
        }
    }
}

/// A sweep's green lamp flashes with each stone ping.
fn stone_lamps(
    mut pings: MessageReader<super::detector::StonePing>,
    mut lamps: Query<&mut StoneLamp>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for p in pings.read() {
        if let Ok(mut l) = lamps.get_mut(p.detector) {
            l.lit = 1.0;
        }
    }
    let dt = time.delta_secs();
    for mut l in &mut lamps {
        if l.lit <= 0.0 {
            continue;
        }
        l.lit = (l.lit - dt * 8.0).max(0.0);
        if let Some(mut m) = materials.get_mut(&l.material) {
            m.emissive = LinearRgba::rgb(0.0, 0.1 + 20.0 * l.lit, 0.02 + 4.0 * l.lit);
        }
    }
}

/// A detector's lamp blinks red with each click.
fn detector_lamps(
    mut clicks: MessageReader<DetectorClick>,
    mut lamps: Query<&mut Lamp>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for c in clicks.read() {
        if let Ok(mut l) = lamps.get_mut(c.detector) {
            l.lit = 1.0;
        }
    }
    let dt = time.delta_secs();
    for mut l in &mut lamps {
        if l.lit <= 0.0 {
            continue;
        }
        l.lit = (l.lit - dt * 12.0).max(0.0);
        if let Some(mut m) = materials.get_mut(&l.material) {
            m.emissive = LinearRgba::rgb(0.2 + 30.0 * l.lit, 0.02 + 3.0 * l.lit, 0.0);
        }
    }
}

// --- The glass eye ---------------------------------------------------------

/// Unlit, additive, drawn on top of everything: robots seen through walls.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct XrayMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Material for XrayMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/xray.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_compare = Some(CompareFunction::Always);
            ds.depth_write_enabled = Some(false);
        }
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Component)]
struct XrayGhost;

/// Every robot part gets a hidden x-ray ghost the size of its collider.
fn xray_parts(
    mut commands: Commands,
    parts: Query<(Entity, &avian3d::prelude::Collider), Added<RobotPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<XrayMaterial>>,
    mut mat: Local<Option<Handle<XrayMaterial>>>,
) {
    for (e, collider) in &parts {
        let m = mat
            .get_or_insert_with(|| {
                materials.add(XrayMaterial {
                    color: LinearRgba::rgb(0.9, 0.25, 0.2),
                })
            })
            .clone();
        let aabb = avian3d::prelude::SimpleCollider::aabb(collider, Vec3::ZERO, Quat::IDENTITY);
        let size = (aabb.max - aabb.min).max(Vec3::splat(0.05));
        commands.entity(e).with_child((
            XrayGhost,
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(m),
            Transform::from_translation((aabb.max + aabb.min) * 0.5),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }
}

/// Holding the glass eye up to your face shows every ghost; robots the
/// camera drone has spotted show theirs anyway.
#[allow(clippy::too_many_arguments)]
fn glass_eye(
    players: Query<&ArtifactEffects, With<LocalPlayer>>,
    eyes: Query<&GlobalTransform>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut ghosts: Query<(&mut Visibility, &ChildOf), With<XrayGhost>>,
    parents: Query<&ChildOf, Without<XrayGhost>>,
    spotted: Query<(), With<crate::gadgets::camera_drone::Spotted>>,
) {
    let head = eye_position(&heads, &locals, &cams);
    let on = players
        .iter()
        .next()
        .and_then(|fx| fx.glass_eye)
        .is_some_and(|eye| {
            let (Ok(eg), Some(h)) = (eyes.get(eye), head) else {
                return false;
            };
            eg.translation().distance(h) < 0.22
        });
    for (mut v, parent) in &mut ghosts {
        // Up from the part to its robot.
        let mut e = parent.parent();
        let mut marked = false;
        for _ in 0..4 {
            if spotted.contains(e) {
                marked = true;
                break;
            }
            match parents.get(e) {
                Ok(p) => e = p.parent(),
                Err(_) => break,
            }
        }
        let want = if on || marked {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
}
