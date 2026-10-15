//! Far trees as impostors: each tree variant's foliage (its cards and
//! core) is drawn once, at start, from the side into a texture (albedo:
//! unlit, so the game's own light and weather fall on it later), read back
//! and given a proper mip chain. Past [`NEAR`] a tree's foliage is three
//! crossed quads with that picture on them, merged per forest chunk; inside
//! it the real cards. The two swap per tree, all at once, at the same
//! distance (no dithered cross-fade to shimmer in a headset).

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{
    Extent3d, PrimitiveTopology, TextureDimension, TextureFormat, TextureUsages,
};

/// Foliage switches between cards and impostor this far from the tree.
pub const NEAR: f32 = 68.0;
/// The capture rigs' render layer (nothing else is on it).
const LAYER: usize = 9;
/// Capture width, texels (a multiple of 64: readback rows stay unpadded).
const WIDTH: u32 = 256;

pub struct ImpostorPlugin;

impl Plugin for ImpostorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ImpostorMaterial>::default())
            .add_systems(Update, read_back);
    }
}

/// What a variant's impostor is: the foliage's bounds (tree-local) and the
/// texture its picture ends up in.
#[derive(Clone, Debug)]
pub struct Impostor {
    pub min: Vec3,
    pub max: Vec3,
    pub image: Handle<Image>,
}

/// A capture in flight.
#[derive(Component)]
struct Capture {
    target: Handle<Image>,
    out: Handle<Image>,
    size: UVec2,
    frames: u32,
    rig: Vec<Entity>,
    reading: bool,
    tries: u32,
    /// Textures the rig needs loaded before its picture is worth taking.
    needs: Vec<Handle<Image>>,
}

/// Lays out a capture of one variant's foliage and returns its impostor;
/// the texture is a transparent placeholder until the capture lands.
#[allow(clippy::too_many_arguments)]
pub fn capture(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    slot: usize,
    cards: (Handle<Mesh>, &StandardMaterial),
    core: Option<(Handle<Mesh>, &StandardMaterial, Vec3)>,
    min: Vec3,
    max: Vec3,
) -> Impostor {
    // Square in plan: the picture is the same from every side.
    let r = min
        .x
        .abs()
        .max(max.x.abs())
        .max(min.z.abs())
        .max(max.z.abs())
        + 0.2;
    let (w, h) = (2.0 * r, max.y - min.y + 0.4);
    let height = ((WIDTH as f32 * h / w / 64.0).ceil() as u32 * 64).clamp(64, 1024);
    let size = UVec2::new(WIDTH, height);
    let (min, max) = (Vec3::new(-r, min.y - 0.2, -r), Vec3::new(r, max.y + 0.2, r));
    let needs: Vec<Handle<Image>> = [Some(cards.1), core.as_ref().map(|c| c.1)]
        .into_iter()
        .flatten()
        .filter_map(|m| m.base_color_texture.clone())
        .collect();
    let target = images.add(target_image(size));
    let out = images.add(placeholder());
    // Far below the world, one rig per variant, out of each other's view.
    let origin = Vec3::new(slot as f32 * 60.0, -4000.0, 0.0);
    let layer = RenderLayers::layer(LAYER);
    let unlit = |m: &StandardMaterial| StandardMaterial {
        unlit: true,
        ..m.clone()
    };
    let mut rig = vec![
        commands
            .spawn((
                Mesh3d(cards.0),
                MeshMaterial3d(materials.add(unlit(cards.1))),
                Transform::from_translation(origin),
                layer.clone(),
            ))
            .id(),
    ];
    if let Some((mesh, mat, scale)) = core {
        rig.push(
            commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(materials.add(unlit(mat))),
                    Transform::from_translation(origin).with_scale(scale),
                    layer.clone(),
                ))
                .id(),
        );
    }
    let centre = origin + Vec3::new(0.0, (min.y + max.y) * 0.5, 0.0);
    rig.push(
        commands
            .spawn((
                Camera3d::default(),
                Camera {
                    order: -30 - slot as isize,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(target.clone().into()),
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: ScalingMode::Fixed {
                        width: w,
                        height: max.y - min.y,
                    },
                    near: 0.0,
                    far: 4.0 * r + 10.0,
                    ..OrthographicProjection::default_3d()
                }),
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                Msaa::Off,
                Transform::from_translation(centre + Vec3::Z * (r + 5.0))
                    .looking_at(centre, Vec3::Y),
                layer,
            ))
            .id(),
    );
    commands.spawn(Capture {
        target,
        out: out.clone(),
        size,
        frames: 0,
        rig,
        reading: false,
        tries: 0,
        needs,
    });
    Impostor {
        min,
        max,
        image: out,
    }
}

fn target_image(size: UVec2) -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_SRC
        | TextureUsages::COPY_DST
        | TextureUsages::RENDER_ATTACHMENT;
    image
}

fn placeholder() -> Image {
    Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// A few frames after a rig is laid out, read its picture back; when it
/// arrives, build the mips, fill the impostor's texture and pack up.
fn read_back(
    mut commands: Commands,
    mut captures: Query<(Entity, &mut Capture)>,
    server: Res<AssetServer>,
) {
    for (e, mut c) in &mut captures {
        // Nothing is drawn with a texture still loading.
        if !c
            .needs
            .iter()
            .all(|h| server.is_loaded_with_dependencies(h))
        {
            continue;
        }
        c.frames += 1;
        if c.frames < 4 || c.reading {
            continue;
        }
        c.reading = true;
        let (out, size, rig) = (c.out.clone(), c.size, c.rig.clone());
        commands.spawn(Readback::texture(c.target.clone())).observe(
            move |done: On<ReadbackComplete>,
                  mut images: ResMut<Assets<Image>>,
                  mut materials: ResMut<Assets<ImpostorMaterial>>,
                  mut captures: Query<&mut Capture>,
                  mut commands: Commands| {
                commands.entity(done.entity).despawn();
                let expected = (size.x * size.y * 4) as usize;
                if let Some(dir) = std::env::var_os("EXTRACTION_IMPOSTOR_DUMP") {
                    let path = std::path::Path::new(&dir).join(format!(
                        "impostor_{}x{}_{}.rgba",
                        size.x,
                        size.y,
                        e.index_u32()
                    ));
                    let _ = std::fs::write(path, &done.data);
                }
                // An empty picture: the rig wasn't drawn yet (its pipeline
                // still compiling). Take it again in a while.
                let empty = done.data.len() < expected
                    || !done.data[..expected].chunks_exact(4).any(|p| p[3] > 0);
                if empty {
                    if let Ok(mut c) = captures.get_mut(e) {
                        c.reading = false;
                        c.frames = 0;
                        c.tries += 1;
                        if c.tries < 30 {
                            return;
                        }
                        warn!("impostor capture stayed empty; far trees of a variant won't show");
                    }
                } else {
                    info!("impostor captured ({}x{})", size.x, size.y);
                    // A new texture, pointed to by the materials that showed
                    // the placeholder (swapping an image's contents under a
                    // material doesn't reach its bind group).
                    let image = images.add(with_mips(&done.data[..expected], size));
                    let ids: Vec<_> = materials
                        .iter()
                        .filter(|(_, m)| m.base.base_color_texture.as_ref() == Some(&out))
                        .map(|(id, _)| id)
                        .collect();
                    for id in ids {
                        if let Some(mut m) = materials.get_mut(id) {
                            m.base.base_color_texture = Some(image.clone());
                        }
                    }
                }
                for r in &rig {
                    commands.entity(*r).despawn();
                }
                commands.entity(e).despawn();
            },
        );
    }
}

/// sRGB byte to linear and back.
fn to_lin(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

/// The material's alpha cut-off (see [`material`]).
const CUTOFF: f32 = 0.5;

/// The captured picture with a full mip chain: colour bled into the empty
/// texels (no dark fringes when filtered), colour averaged in linear light
/// weighted by alpha, and each level's alpha scaled to keep the coverage
/// the full-size picture has at the cut-off (or far trees thin to nothing).
pub fn with_mips(rgba: &[u8], size: UVec2) -> Image {
    let (w0, h0) = (size.x as usize, size.y as usize);
    let mut px: Vec<[f32; 4]> = rgba
        .chunks_exact(4)
        .map(|p| {
            [
                to_lin(p[0]),
                to_lin(p[1]),
                to_lin(p[2]),
                p[3] as f32 / 255.0,
            ]
        })
        .collect();
    bleed(&mut px, w0, h0);
    let coverage = |px: &[[f32; 4]], scale: f32| {
        px.iter().filter(|p| p[3] * scale >= CUTOFF).count() as f32 / px.len() as f32
    };
    let want = coverage(&px, 1.0);
    let mut levels = vec![px];
    let (mut w, mut h) = (w0, h0);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let src = levels.last().unwrap();
        let mut dst = vec![[0.0f32; 4]; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0.0f32; 4];
                let mut wsum = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                    let p = src[sy * w + sx];
                    let a = p[3].max(1e-3);
                    for c in 0..3 {
                        acc[c] += p[c] * a;
                    }
                    acc[3] += p[3];
                    wsum += a;
                }
                dst[y * nw + x] = [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum, acc[3] / 4.0];
            }
        }
        // Scale alpha so this level covers what the top one does.
        let (mut lo, mut hi) = (0.5f32, 4.0f32);
        for _ in 0..16 {
            let mid = (lo + hi) * 0.5;
            if coverage(&dst, mid) < want {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let s = (lo + hi) * 0.5;
        for p in &mut dst {
            p[3] = (p[3] * s).min(1.0);
        }
        levels.push(dst);
        (w, h) = (nw, nh);
    }
    let mut data = Vec::new();
    for level in &levels {
        for p in level {
            data.extend_from_slice(&[
                to_srgb(p[0]),
                to_srgb(p[1]),
                to_srgb(p[2]),
                (p[3] * 255.0 + 0.5) as u8,
            ]);
        }
    }
    // Made with the top level (as `Image::new` checks), then the chain.
    let top = data[..w0 * h0 * 4].to_vec();
    let mut image = Image::new(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        top,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels.len() as u32;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::ClampToEdge,
        address_mode_v: bevy::image::ImageAddressMode::ClampToEdge,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// Empty texels take the colour of filled ones near them (a few passes of
/// growing the edge outwards, then the overall average for the rest).
fn bleed(px: &mut [[f32; 4]], w: usize, h: usize) {
    let mut known: Vec<bool> = px.iter().map(|p| p[3] > 0.01).collect();
    for _ in 0..8 {
        let mut next = known.clone();
        let snapshot: Vec<[f32; 4]> = px.to_vec();
        for y in 0..h {
            for x in 0..w {
                let k = y * w + x;
                if known[k] {
                    continue;
                }
                let mut acc = [0.0f32; 3];
                let mut n = 0.0;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if known[j] {
                        for c in 0..3 {
                            acc[c] += snapshot[j][c];
                        }
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    px[k] = [acc[0] / n, acc[1] / n, acc[2] / n, px[k][3]];
                    next[k] = true;
                }
            }
        }
        known = next;
    }
    let filled: Vec<&[f32; 4]> = px.iter().filter(|p| p[3] > 0.01).collect();
    if filled.is_empty() {
        return;
    }
    let mut mean = [0.0f32; 3];
    for p in &filled {
        for c in 0..3 {
            mean[c] += p[c] / filled.len() as f32;
        }
    }
    for (p, k) in px.iter_mut().zip(known) {
        if !k {
            p[0] = mean[0];
            p[1] = mean[1];
            p[2] = mean[2];
        }
    }
}

/// Three quads crossed at 60 degrees round a tree's axis, both faces each
/// (their own normals, mostly up: lit like a crown, not like a wall),
/// transformed to the tree. UV1 carries the tree's base (x, z), for the
/// near cut: the horizontal distance `trees::cut_near_detail` measures for
/// the cards.
pub fn quads(imp: &Impostor, t: &Transform, out: &mut Builder) {
    let base = t.translation;
    let r = imp.max.x;
    for k in 0..3 {
        let a = k as f32 * std::f32::consts::PI / 3.0;
        let across = Vec3::new(a.cos(), 0.0, a.sin());
        let facing = across.cross(Vec3::Y);
        for side in [1.0f32, -1.0] {
            let n = (facing * side * 0.35 + Vec3::Y * 0.65).normalize();
            let first = out.pos.len() as u32;
            for (u, v) in [(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)] {
                let local = across * (u * 2.0 - 1.0) * r
                    + Vec3::Y * (imp.min.y + (1.0 - v) * (imp.max.y - imp.min.y));
                out.pos.push(t.transform_point(local).to_array());
                out.nrm.push((t.rotation * n).to_array());
                out.uv.push([if side > 0.0 { u } else { 1.0 - u }, v]);
                out.base.push([base.x, base.z]);
            }
            let tri = if side > 0.0 {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            out.idx.extend(tri.iter().map(|i| first + i));
        }
    }
}

/// Accumulates impostor quads for one chunk.
#[derive(Default)]
pub struct Builder {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    base: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

impl Builder {
    pub fn is_empty(&self) -> bool {
        self.idx.is_empty()
    }

    pub fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.base)
        .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// Impostor material: the picture, alpha-tested, lit, gone within
/// [`NEAR`] of the tree (by its base, carried in UV1).
pub type ImpostorMaterial = bevy::pbr::ExtendedMaterial<StandardMaterial, ImpostorCut>;

#[derive(Asset, bevy::render::render_resource::AsBindGroup, Reflect, Debug, Clone)]
pub struct ImpostorCut {
    /// x: the near cut, metres.
    #[uniform(100)]
    pub cut: Vec4,
}

impl bevy::pbr::MaterialExtension for ImpostorCut {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        "shaders/impostor.wgsl".into()
    }
}

pub fn material(image: Handle<Image>, tint: Color) -> ImpostorMaterial {
    ImpostorMaterial {
        base: StandardMaterial {
            base_color: tint,
            base_color_texture: Some(image),
            alpha_mode: AlphaMode::Mask(CUTOFF),
            perceptual_roughness: 0.85,
            reflectance: 0.2,
            diffuse_transmission: 0.2,
            ..default()
        },
        extension: ImpostorCut {
            cut: Vec4::new(NEAR, 0.0, 0.0, 0.0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sparse picture keeps its coverage down the mip chain, and the
    /// empty texels round a leaf take its colour.
    #[test]
    fn mips_keep_coverage_and_colour() {
        let size = UVec2::new(64, 64);
        let mut rgba = vec![0u8; 64 * 64 * 4];
        // Clumps of leaves: a quarter of 8-texel blocks.
        for y in 0..64 {
            for x in 0..64 {
                if (x / 8) % 2 == 0 && (y / 8) % 2 == 0 {
                    let k = (y * 64 + x) * 4;
                    rgba[k..k + 4].copy_from_slice(&[40, 120, 40, 255]);
                }
            }
        }
        let image = with_mips(&rgba, size);
        assert_eq!(image.texture_descriptor.mip_level_count, 7);
        let data = image.data.as_ref().unwrap();
        // Level 2 (16x16) starts after 64*64 + 32*32 texels.
        let off = (64 * 64 + 32 * 32) * 4;
        let l2 = &data[off..off + 16 * 16 * 4];
        let covered = l2
            .chunks(4)
            .filter(|p| p[3] as f32 / 255.0 >= CUTOFF)
            .count();
        let share = covered as f32 / 256.0;
        assert!((share - 0.25).abs() < 0.1, "level 2 covers {share}");
        // No black fringe: every texel is green-ish.
        assert!(l2.chunks(4).all(|p| p[1] > p[0] && p[1] > 60));
    }
}
