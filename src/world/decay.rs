//! The Zone's years on its buildings, besides the ivy (`overgrowth`):
//! moss and lichen in the buildings' own colours, and stains run down
//! their walls. Visual only; `buildings::dress_buildings` applies both.
//!
//! - [`moss`] tints a building's shaded, baked colours: roofs and ledges
//!   open to the rain, north faces, the feet of walls, in patches. The
//!   wet-weather shading reads the same sky the bake carries, so moss on a
//!   roof goes darker in the rain like everything else out there.
//! - [`stains`] lays decals on the walls' outsides: water stains under the
//!   windows and down from the eaves, rust under iron (a corrugated roof's
//!   edge, a metal fixture on a wall), soot above a burnt house's
//!   openings. Each is a quad with a streak texture drawn by multiplying
//!   (the wall shows through, only darker), a few millimetres proud.
//!
//! All from the building's seed (`overgrowth::seed`): the same stains on
//! the same building every time.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use super::ao::Occluders;
use super::buildings::{Geo, Shape};
use crate::render::materials::Mat;

/// Moss's colour (as a multiplier: green, darker).
const MOSS: Vec3 = Vec3::new(0.62, 0.74, 0.42);
/// Lichen's: a dry yellow-grey.
const LICHEN: Vec3 = Vec3::new(0.86, 0.85, 0.66);

/// A cheap smooth-ish value noise in 0..1, by position.
fn noise(p: Vec3, seed: u64) -> f32 {
    let s = (seed % 1000) as f32 * 0.37;
    let v = (p.x * 1.7 + s).sin() * (p.z * 1.3 - s * 0.5).cos() * 0.5
        + (p.y * 2.3 + p.x * 0.9 + s * 1.3).sin() * 0.3
        + (p.z * 3.1 - p.y * 1.1).cos() * 0.2;
    (v * 0.5 + 0.5).clamp(0.0, 1.0)
}

/// How mossy a building is, 0..1, from its seed.
pub fn mossiness(seed: u64) -> f32 {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x0b55);
    0.25 + 0.75 * rng.f32()
}

/// Tint a building's merged mesh's baked colours with moss and lichen:
/// `north` is building-local, `seed` the building's.
pub fn moss(mesh: &mut Mesh, north: Vec3, seed: u64) {
    let amount = mossiness(seed);
    let Some(VertexAttributeValues::Float32x3(pos)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION).cloned()
    else {
        return;
    };
    let Some(VertexAttributeValues::Float32x3(nrm)) =
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL).cloned()
    else {
        return;
    };
    let Some(VertexAttributeValues::Float32x4(col)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
    else {
        return;
    };
    for ((c, p), n) in col.iter_mut().zip(&pos).zip(&nrm) {
        let (p, n) = (Vec3::from_array(*p), Vec3::from_array(*n));
        // The sky the bake left in alpha: rain falls here.
        let sky = c[3];
        let up = n.y.max(0.0);
        let north_w = n.dot(north).max(0.0) * (1.0 - n.y.abs());
        let foot = (1.0 - (p.y / 1.2).clamp(0.0, 1.0)) * (1.0 - n.y.abs());
        let shade = 1.0 - c[0];
        let patch = noise(p, seed);
        let m = ((up * sky * 0.9 + north_w * 0.7 + foot * 0.5 + shade * 0.4)
            * smooth(0.35, 0.75, patch)
            * amount)
            .clamp(0.0, 0.85);
        // Lichen on what's open and dry-ish: roofs' tops, in its own
        // patches.
        let l =
            (up * sky * smooth(0.6, 0.85, noise(p * 2.7, seed ^ 7)) * amount * 0.7).clamp(0.0, 0.7);
        let tint = Vec3::ONE.lerp(MOSS, m) * Vec3::ONE.lerp(LICHEN, l * (1.0 - m));
        c[0] *= tint.x;
        c[1] *= tint.y;
        c[2] *= tint.z;
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// --- Stains ---------------------------------------------------------------------------------

/// The kinds of stain, each its own texture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stain {
    /// Water run down a wall: grey-green, darker at the top.
    Water,
    /// Rust run from iron: orange-brown.
    Rust,
    /// Soot over an opening a fire came out of: black, darker at the
    /// bottom, rising.
    Soot,
}

pub const STAINS: [Stain; 3] = [Stain::Water, Stain::Rust, Stain::Soot];

impl Stain {
    fn tint(self) -> Vec3 {
        match self {
            Stain::Water => Vec3::new(0.62, 0.64, 0.6),
            Stain::Rust => Vec3::new(0.62, 0.42, 0.28),
            Stain::Soot => Vec3::new(0.18, 0.17, 0.16),
        }
    }

    /// Its streak texture, made here: the tint where the streaks are,
    /// white (no change) elsewhere; streaks hang from the top (soot rises
    /// from the bottom).
    pub fn image(self) -> Image {
        let (w, h) = (64u32, 128u32);
        let mut rng = fastrand::Rng::with_seed(0x57a1 + self as u64);
        // A row of streaks: x centre, width, length, strength.
        let streaks: Vec<(f32, f32, f32, f32)> = (0..14)
            .map(|_| {
                (
                    rng.f32(),
                    0.02 + rng.f32() * 0.08,
                    0.35 + rng.f32() * 0.65,
                    0.4 + rng.f32() * 0.6,
                )
            })
            .collect();
        let tint = self.tint();
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            // t: 0 at the source edge, 1 at the far end.
            let t = if self == Stain::Soot {
                1.0 - y as f32 / (h - 1) as f32
            } else {
                y as f32 / (h - 1) as f32
            };
            for x in 0..w {
                let u = x as f32 / (w - 1) as f32;
                let mut s: f32 = 0.0;
                for &(cx, sw, len, k) in &streaks {
                    let d = ((u - cx) / sw).abs();
                    if d < 1.0 && t < len {
                        s = s.max(k * (1.0 - d * d) * (1.0 - t / len).powf(0.8));
                    }
                }
                // A band along the source edge, and soft sides.
                s = s.max((1.0 - t * 6.0).max(0.0) * 0.7);
                s *= smooth(0.0, 0.12, u) * smooth(0.0, 0.12, 1.0 - u);
                let c = Vec3::ONE.lerp(tint, s.clamp(0.0, 1.0));
                data.extend_from_slice(&[
                    (c.x * 255.0) as u8,
                    (c.y * 255.0) as u8,
                    (c.z * 255.0) as u8,
                    255,
                ]);
            }
        }
        Image::new(
            bevy::render::render_resource::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            data,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        )
    }
}

/// A stain quad: on a wall face at `top` (the middle of its source edge),
/// spanning `width` along `u`, running `length` down (soot: up) the
/// face, facing out along `n`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decal {
    pub kind: Stain,
    pub top: Vec3,
    pub n: Vec3,
    pub u: Vec3,
    pub width: f32,
    pub length: f32,
}

/// A pane's outward normal (its thin axis, pointing away from the
/// building's middle, as `shafts` finds windows), the wall's run along it,
/// and its width along that.
fn outward(at: &Transform, size: Vec3) -> Option<(Vec3, Vec3, f32)> {
    let r = at.rotation;
    let axes = [r * Vec3::X, r * Vec3::Y, r * Vec3::Z];
    let (k, _) = size
        .to_array()
        .into_iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let mut n = axes[k];
    if n.y.abs() > 0.5 {
        return None;
    }
    let c = at.translation;
    if Vec2::new(c.x, c.z).dot(Vec2::new(n.x, n.z)) < 0.0 {
        n = -n;
    }
    let u = Vec3::Y.cross(n).normalize_or_zero();
    let w = (u.dot(axes[0]).abs() * size.x + u.dot(axes[2]).abs() * size.z).max(0.1);
    Some((n, u, w))
}

/// Out from a point inside a wall along `n` to its face (within 0.5 m).
fn face_out(occ: &Occluders, p: Vec3, n: Vec3) -> Option<Vec3> {
    if !occ.inside(p) {
        return None;
    }
    (1..=50)
        .map(|k| p + n * (k as f32 * 0.01))
        .find(|q| !occ.inside(*q))
}

/// The stains on a building: under its windows (water), down from its
/// eaves (water, or rust under iron sheet), under iron fixed to its walls
/// (rust), above its openings if it burnt (soot).
pub fn stains(geo: &Geo, occ: &Occluders, seed: u64, burnt: bool) -> Vec<Decal> {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x5a1d);
    let mut out = Vec::new();
    const PROUD: f32 = 0.008;
    let iron_roof = geo
        .pieces
        .iter()
        .any(|p| matches!(p.mat, Mat::Corrugated | Mat::RustyMetal) && p.at.translation.y > 2.0);
    for p in &geo.pieces {
        let Shape::Box(s) = p.shape else { continue };
        let c = p.at.translation;
        match p.mat {
            // A window: water down from under its sill, soot up from over
            // its head (each on the wall's face, found from inside it).
            Mat::Glass => {
                let Some((n, u, w)) = outward(&p.at, s) else {
                    continue;
                };
                let half_h = s.y * 0.5;
                let back = -n * 0.03;
                if let Some(f) = face_out(occ, c + back - Vec3::Y * (half_h + 0.2), n) {
                    if rng.f32() < 0.75 {
                        out.push(Decal {
                            kind: Stain::Water,
                            top: f + Vec3::Y * 0.12 + n * PROUD,
                            n,
                            u,
                            width: w * (0.8 + rng.f32() * 0.3),
                            length: 0.6 + rng.f32() * 0.8,
                        });
                    }
                }
                if burnt {
                    if let Some(f) = face_out(occ, c + back + Vec3::Y * (half_h + 0.2), n) {
                        out.push(Decal {
                            kind: Stain::Soot,
                            top: f - Vec3::Y * 0.12 + n * PROUD,
                            n,
                            u,
                            width: w + 0.4,
                            length: 0.9 + rng.f32() * 0.5,
                        });
                    }
                }
            }
            // A burnt house's openings have no glass left: find them by
            // their charred lintels (the 8 cm beam over each), soot rising
            // over them.
            Mat::Charred if burnt && (s.y - 0.08).abs() < 0.005 && s.x.max(s.z) > 0.8 => {
                let (along, thick) = if s.x > s.z {
                    (Vec3::X, s.z)
                } else {
                    (Vec3::Z, s.x)
                };
                let r = p.at.rotation;
                let across = r * if along == Vec3::X { Vec3::Z } else { Vec3::X };
                let mut n = across;
                if Vec2::new(c.x, c.z).dot(Vec2::new(n.x, n.z)) < 0.0 {
                    n = -n;
                }
                let u = r * along;
                if let Some(f) = face_out(occ, c - n * (thick * 0.25) + Vec3::Y * 0.2, n) {
                    out.push(Decal {
                        kind: Stain::Soot,
                        top: f - Vec3::Y * 0.16 + n * PROUD,
                        n,
                        u,
                        width: s.x.max(s.z) + 0.4,
                        length: 0.9 + rng.f32() * 0.5,
                    });
                }
            }
            // Iron fixed to a wall (a box, a bracket): rust run below it.
            Mat::DarkMetal | Mat::RustyMetal if s.max_element() < 1.2 && p.collide => {
                let r = p.at.rotation;
                for (ln, lu) in [
                    (Vec3::X, Vec3::Z),
                    (Vec3::NEG_X, Vec3::Z),
                    (Vec3::Z, Vec3::X),
                    (Vec3::NEG_Z, Vec3::X),
                ] {
                    let (n, u) = (r * ln, r * lu);
                    let depth = (ln.abs() * s).max_element() * 0.5;
                    let behind = c - n * (depth + 0.05) - Vec3::Y * (s.y * 0.5 + 0.05);
                    if let Some(f) = face_out(occ, behind, n) {
                        out.push(Decal {
                            kind: Stain::Rust,
                            top: f + n * PROUD,
                            n,
                            u,
                            width: (lu.abs() * s).max_element().max(0.15) + 0.1,
                            length: 0.5 + rng.f32() * 0.7,
                        });
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    // Down from the eaves: the tops of the walls' outsides under a roof's
    // edge, in runs.
    for p in &geo.pieces {
        let Shape::Box(s) = p.shape else { continue };
        let walls = matches!(
            p.mat,
            Mat::Planks
                | Mat::Brick
                | Mat::Plaster
                | Mat::Concrete
                | Mat::LogWall
                | Mat::SootBrick
                | Mat::Charred
        );
        if !p.collide || !walls {
            continue;
        }
        let r = p.at.rotation;
        let h = s * 0.5;
        for (ln, lu, hn, hu) in [
            (Vec3::X, Vec3::Z, h.x, h.z),
            (Vec3::NEG_X, Vec3::Z, h.x, h.z),
            (Vec3::Z, Vec3::X, h.z, h.x),
            (Vec3::NEG_Z, Vec3::X, h.z, h.x),
        ] {
            let (n, u) = (r * ln, r * lu);
            if hu < 0.6 || h.y < 0.8 || n.y.abs() > 0.3 {
                continue;
            }
            let top_c = p.at.translation + n * hn + r * Vec3::Y * (h.y - 0.02);
            // Outside (away from the middle), open in front, a roof's
            // edge over it.
            if Vec2::new(top_c.x, top_c.z).dot(Vec2::new(n.x, n.z)) < 0.0
                || occ.inside(top_c + n * 0.04)
                || occ.sky(top_c + n * 0.15, Vec3::Y) > 0.5
            {
                continue;
            }
            let runs = ((hu * 2.0) / 2.5).ceil() as usize;
            for k in 0..runs {
                if rng.f32() < 0.35 {
                    continue;
                }
                let a = -hu + (k as f32 + 0.2 + rng.f32() * 0.6) * (2.0 * hu / runs as f32);
                let width = (0.5 + rng.f32() * 1.2).min(2.0 * hu);
                out.push(Decal {
                    kind: if iron_roof { Stain::Rust } else { Stain::Water },
                    top: top_c + u * a + n * PROUD,
                    n,
                    u,
                    width,
                    length: (0.5 + rng.f32() * 1.3).min(h.y * 2.0 - 0.3),
                });
            }
        }
    }
    out
}

/// One stain kind's decals as a mesh (building-local).
pub fn decal_mesh(decals: &[Decal], kind: Stain) -> Option<Mesh> {
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for d in decals.iter().filter(|d| d.kind == kind) {
        let down = if kind == Stain::Soot {
            Vec3::Y
        } else {
            Vec3::NEG_Y
        };
        let half = d.u * d.width * 0.5;
        let far = d.top + down * d.length;
        let base = pos.len() as u32;
        // Soot's texture rises from its bottom row (v 1); the others hang
        // from their top (v 0).
        let (v_src, v_far) = if kind == Stain::Soot {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        for (p, t) in [
            (d.top - half, [0.0, v_src]),
            (d.top + half, [1.0, v_src]),
            (far + half, [1.0, v_far]),
            (far - half, [0.0, v_far]),
        ] {
            pos.push(p.to_array());
            nrm.push(d.n.to_array());
            uv.push(t);
        }
        // Facing out along n.
        let cross = (d.u).cross(down).dot(d.n);
        if cross > 0.0 {
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            idx.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    if idx.is_empty() {
        return None;
    }
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx)),
    )
}

/// The stains' materials: their streak textures, multiplied over what's
/// behind them, unlit.
#[derive(Resource)]
pub struct StainAssets(pub [Handle<StandardMaterial>; 3]);

pub fn setup_stains(
    mut commands: Commands,
    existing: Option<Res<StainAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if existing.is_some() {
        return;
    }
    let mk = |k: Stain, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>| {
        materials.add(StandardMaterial {
            base_color_texture: Some(images.add(k.image())),
            alpha_mode: AlphaMode::Multiply,
            unlit: true,
            depth_bias: 4.0,
            fog_enabled: false,
            ..default()
        })
    };
    let handles = STAINS.map(|k| mk(k, &mut images, &mut materials));
    commands.insert_resource(StainAssets(handles));
}

/// Which of the building `prefab` at `at`'s window sashes time has
/// smashed: one bool per `window_sash` model, in `Geo::props` order and
/// each prop's models in order. All of a ruin's, 15 to 60% of a house's,
/// from the building's seed. Pure, and the same headless: Destruction's
/// panes start broken where this says so, and `dress_buildings` swaps
/// those sashes for `window_sash_broken`.
pub fn weathered_sashes(prefab: &str, at: &Transform) -> Vec<bool> {
    super::buildings::prefab(prefab).map_or_else(Vec::new, |geo| weathered(&geo, prefab, at))
}

/// [`weathered_sashes`] for a prefab's geometry in hand.
pub(super) fn weathered(geo: &Geo, prefab: &str, at: &Transform) -> Vec<bool> {
    let xz = Vec2::new(at.translation.x, at.translation.z);
    let mut rng = fastrand::Rng::with_seed(super::overgrowth::seed(xz, prefab) ^ 0x9a5e);
    let share = if extraction_core::map::prefab_base(prefab) != prefab {
        1.0
    } else {
        0.15 + rng.f32() * 0.45
    };
    geo.props
        .iter()
        .flat_map(|d| d.models.iter())
        .filter(|(name, _)| *name == "window_sash")
        .map(|_| rng.f32() < share)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same building stains the same, and its stains lie on its walls'
    /// outsides.
    #[test]
    fn stains_are_deterministic() {
        for id in ["house_plank", "house_brick_burnt", "shed", "factory_hall"] {
            let geo = crate::world::buildings::prefab(id).unwrap();
            let occ = crate::world::buildings::occluders(&geo);
            let a = stains(&geo, &occ, 42, id.ends_with("_burnt"));
            let b = stains(&geo, &occ, 42, id.ends_with("_burnt"));
            assert_eq!(a, b, "{id}");
            assert!(!a.is_empty(), "{id}: no stains");
            if id.ends_with("_burnt") {
                assert!(a.iter().any(|d| d.kind == Stain::Soot), "{id}: no soot");
            }
        }
    }

    /// Weathered sashes: one per sash, the same every time, all of a
    /// ruin's and only some of a house's.
    #[test]
    fn weathered_sashes_are_steady() {
        let at = Transform::from_xyz(-18.0, 0.0, 95.0);
        let house = weathered_sashes("house_plank", &at);
        assert!(!house.is_empty());
        assert_eq!(house, weathered_sashes("house_plank", &at));
        assert!(house.iter().any(|b| !b), "every pane of a house smashed");
        let ruin = weathered_sashes("house_plank_damaged", &at);
        assert!(!ruin.is_empty() && ruin.iter().all(|b| *b), "{ruin:?}");
        assert!(weathered_sashes("no_such_prefab", &at).is_empty());
    }
}
