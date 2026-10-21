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

/// The paint a timber house's boards were last given, long faded: blue,
/// green, ochre, oxblood, or none (bare grey boards), from its seed: a
/// multiplier on the boards' colours.
pub fn paint(seed: u64) -> Vec3 {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x9a17);
    let colour = match rng.usize(..6) {
        0 => Vec3::new(0.62, 0.78, 1.0),
        1 => Vec3::new(0.72, 0.92, 0.7),
        2 => Vec3::new(1.0, 0.86, 0.55),
        3 => Vec3::new(0.95, 0.62, 0.55),
        _ => return Vec3::ONE,
    };
    Vec3::ONE.lerp(colour, 0.55 + 0.3 * rng.f32())
}

/// Tint a mesh's baked colours by `tint` (its AO and sky kept).
pub fn tint(mesh: &mut Mesh, tint: Vec3) {
    if tint == Vec3::ONE {
        return;
    }
    if let Some(VertexAttributeValues::Float32x4(col)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
        for c in col.iter_mut() {
            c[0] *= tint.x;
            c[1] *= tint.y;
            c[2] *= tint.z;
        }
    }
}

/// The snow a building carries in a valley where it lies all winter
/// (`snow::snow_line`, `MapDef::snow`): a load on every top that faces
/// up and is open to the sky (its roof's slopes, a flat roof, the sills,
/// the steps), thinner as the slope steepens and none past 55 degrees,
/// with a cornice lipping over a sloped roof's eaves. Visual only: pieces
/// to bake with the building's own (they block nothing).
pub fn snow_load(geo: &Geo, occ: &Occluders) -> Vec<super::buildings::Piece> {
    let mut out = Vec::new();
    for p in &geo.pieces {
        let Shape::Box(s) = p.shape else { continue };
        if matches!(p.mat, Mat::Glass | Mat::Snow) {
            continue;
        }
        let s = s * p.at.scale;
        let up = p.at.rotation * Vec3::Y;
        let area = s.x * s.z;
        if up.y < 0.57 || area < 0.08 || s.x.min(s.z) < 0.08 {
            continue;
        }
        let top = p.at.translation + up * s.y * 0.5;
        // Open to the sky over its middle and its ends.
        let (ax, az) = (p.at.rotation * Vec3::X, p.at.rotation * Vec3::Z);
        let open = [
            Vec3::ZERO,
            ax * 0.4 * s.x,
            -ax * 0.4 * s.x,
            az * 0.4 * s.z,
            -az * 0.4 * s.z,
        ]
        .iter()
        .filter(|o| occ.sky(top + **o + up * 0.05, up) > 0.5)
        .count();
        if open < 4 {
            continue;
        }
        // Deep on the flat, a skin on a steep pitch; thin on a sill.
        let pitch = ((up.y - 0.57) / 0.43).clamp(0.0, 1.0);
        let full = if area < 0.5 { 0.07 } else { 0.24 };
        let depth = full * (0.35 + 0.65 * pitch);
        let inset = 0.03;
        let rot = p.at.rotation;
        // Sunk a centimetre into what it lies on.
        let lift = depth * 0.5 - 0.01;
        out.push(super::buildings::Piece {
            shape: Shape::Box(Vec3::new(s.x - inset * 2.0, depth, s.z - inset * 2.0)),
            at: Transform::from_translation(top + up * lift).with_rotation(rot),
            mat: Mat::Snow,
            collide: false,
        });
        // The cornice at a slope's foot: the load creeping over the eaves.
        if up.y < 0.985 && area >= 0.5 {
            let downhill = (up * up.y - Vec3::Y).normalize();
            let local = rot.inverse() * downhill;
            let (half, across, along_x) = if local.x.abs() > local.z.abs() {
                (s.x * 0.5, s.z, true)
            } else {
                (s.z * 0.5, s.x, false)
            };
            let lip = 0.2;
            let size = if along_x {
                Vec3::new(lip, depth * 1.35, across - inset * 2.0)
            } else {
                Vec3::new(across - inset * 2.0, depth * 1.35, lip)
            };
            let at = top + downhill * (half - lip * 0.5 + 0.06) + up * (depth * 0.675 - 0.01);
            out.push(super::buildings::Piece {
                shape: Shape::Box(size),
                at: Transform::from_translation(at).with_rotation(rot),
                mat: Mat::Snow,
                collide: false,
            });
        }
    }
    out
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

/// How far an old house's roof has sagged at the middle of its ridge, m
/// (0 for most: about a third sag), from its seed.
pub fn sagging(seed: u64) -> f32 {
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x5a99);
    if rng.f32() < 0.32 {
        0.07 + 0.15 * rng.f32()
    } else {
        0.0
    }
}

/// Where a roof is, from its merged meshes: their bounds (building-local).
pub fn roof_bounds<'a>(meshes: impl IntoIterator<Item = &'a Mesh>) -> Option<(Vec3, Vec3)> {
    let mut b: Option<(Vec3, Vec3)> = None;
    for m in meshes {
        let Some(VertexAttributeValues::Float32x3(pos)) = m.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        for p in pos {
            let p = Vec3::from(*p);
            b = Some(b.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))));
        }
    }
    b
}

/// Bow what's over a pitched roof's eaves (`roof`: its bounds; its ridge
/// along z, building-local): down by `amount` at the middle of the
/// ridge, nothing at the gables' ends (the end walls hold it) or along
/// the eaves (the side walls do). The roof, its ridge board and the
/// gables' tops alike, so nothing stands off it. Drawn only: the roof's
/// colliders stay where they were.
pub fn sag(mesh: &mut Mesh, roof: (Vec3, Vec3), amount: f32) {
    if amount <= 0.0 {
        return;
    }
    let (lo, hi) = roof;
    let over = lo.y + 0.02;
    // A box's face is two triangles: cut what's over the eaves finer to
    // bend it.
    subdivide(mesh, 0.6, over);
    let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    let (c, half) = ((lo + hi) * 0.5, ((hi - lo) * 0.5).max(Vec3::splat(0.01)));
    for p in pos.iter_mut().filter(|p| p[1] > over) {
        let tz = ((p[2] - lo.z) / (2.0 * half.z)).clamp(0.0, 1.0);
        let xn = ((p[0] - c.x) / half.x).abs().min(1.0);
        p[1] -= amount * (std::f32::consts::PI * tz).sin() * (1.0 - xn);
    }
}

/// Split a triangle list's edges over `above` (both their ends) until
/// none is longer than `max`, every attribute interpolated. An edge splits
/// the same in both its triangles (by its length and height alone), so the
/// cut mesh has no cracks to open.
fn subdivide(mesh: &mut Mesh, max: f32, above: f32) {
    let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    let n = pos.len();
    let mut idx: Vec<u32> = match mesh.indices() {
        Some(Indices::U32(i)) => i.clone(),
        Some(Indices::U16(i)) => i.iter().map(|&k| k as u32).collect(),
        None => (0..n as u32).collect(),
    };
    // Every float attribute, flattened: (id, components, data).
    let mut attrs: Vec<(bevy::mesh::MeshVertexAttribute, usize, Vec<f32>)> = Vec::new();
    for (a, v) in mesh.attributes() {
        let (k, data): (usize, Vec<f32>) = match v {
            VertexAttributeValues::Float32x2(d) => (2, d.iter().flatten().copied().collect()),
            VertexAttributeValues::Float32x3(d) => (3, d.iter().flatten().copied().collect()),
            VertexAttributeValues::Float32x4(d) => (4, d.iter().flatten().copied().collect()),
            _ => return,
        };
        attrs.push((a.clone(), k, data));
    }
    let p = |attrs: &[(bevy::mesh::MeshVertexAttribute, usize, Vec<f32>)], i: u32| {
        let d = &attrs
            .iter()
            .find(|(a, _, _)| a.id == Mesh::ATTRIBUTE_POSITION.id)
            .unwrap()
            .2;
        Vec3::from_slice(&d[i as usize * 3..i as usize * 3 + 3])
    };
    for _ in 0..8 {
        let mut mid: std::collections::HashMap<(u32, u32), u32> = Default::default();
        let mut next = Vec::with_capacity(idx.len() * 2);
        let mut split = false;
        for t in idx.chunks_exact(3) {
            let v = [t[0], t[1], t[2]];
            let mut m = [None; 3];
            for e in 0..3 {
                let (a, b) = (v[e], v[(e + 1) % 3]);
                let (pa, pb) = (p(&attrs, a), p(&attrs, b));
                if pa.distance(pb) <= max || pa.y <= above || pb.y <= above {
                    continue;
                }
                split = true;
                let key = (a.min(b), a.max(b));
                let k = *mid.entry(key).or_insert_with(|| {
                    let at = attrs[0].2.len() / attrs[0].1;
                    for (_, c, d) in attrs.iter_mut() {
                        for j in 0..*c {
                            let x = (d[a as usize * *c + j] + d[b as usize * *c + j]) * 0.5;
                            d.push(x);
                        }
                    }
                    at as u32
                });
                m[e] = Some(k);
            }
            // Edge e runs v[e] -> v[e+1], its middle m[e].
            match m {
                [None, None, None] => next.extend_from_slice(&v),
                [Some(a), Some(b), Some(c)] => {
                    next.extend_from_slice(&[v[0], a, c, a, v[1], b, c, b, v[2], a, b, c])
                }
                _ => {
                    // One or two long edges: fan from the first one's
                    // middle round the cut outline (from a corner, the fan
                    // would run along an uncut edge).
                    let mut ring = Vec::with_capacity(5);
                    for e in 0..3 {
                        ring.push(v[e]);
                        if let Some(k) = m[e] {
                            ring.push(k);
                        }
                    }
                    let first = m.iter().flatten().next().copied().unwrap();
                    let start = ring.iter().position(|&x| x == first).unwrap();
                    ring.rotate_left(start);
                    for w in 1..ring.len() - 1 {
                        next.extend_from_slice(&[ring[0], ring[w], ring[w + 1]]);
                    }
                }
            }
        }
        idx = next;
        if !split {
            break;
        }
    }
    for (a, c, d) in attrs {
        let v = match c {
            2 => {
                VertexAttributeValues::Float32x2(d.chunks_exact(2).map(|x| [x[0], x[1]]).collect())
            }
            3 => VertexAttributeValues::Float32x3(
                d.chunks_exact(3).map(|x| [x[0], x[1], x[2]]).collect(),
            ),
            _ => VertexAttributeValues::Float32x4(
                d.chunks_exact(4)
                    .map(|x| [x[0], x[1], x[2], x[3]])
                    .collect(),
            ),
        };
        mesh.insert_attribute(a, v);
    }
    mesh.insert_indices(Indices::U32(idx));
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
    /// Where a reservoir stood on a drowned house's walls: a hard dark
    /// line, a film of mud under it. Its sides don't fade (the bands
    /// meet along a wall).
    Tide,
}

pub const STAINS: [Stain; 4] = [Stain::Water, Stain::Rust, Stain::Soot, Stain::Tide];

impl Stain {
    fn tint(self) -> Vec3 {
        match self {
            Stain::Water => Vec3::new(0.62, 0.64, 0.6),
            Stain::Rust => Vec3::new(0.62, 0.42, 0.28),
            Stain::Soot => Vec3::new(0.18, 0.17, 0.16),
            Stain::Tide => Vec3::new(0.3, 0.29, 0.2),
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
                if self == Stain::Tide {
                    // The line, a few centimetres of near black; under it
                    // the mud's film, even but for a mottle, paling a
                    // little downwards.
                    let line = 1.0 - smooth(0.03, 0.07, t);
                    let film =
                        0.52 + 0.06 * (u * 37.0 + t * 11.0).sin() * (u * 13.0).cos() - 0.12 * t;
                    s = line.max(film);
                } else {
                    // A band along the source edge, and soft sides.
                    s = s.max((1.0 - t * 6.0).max(0.0) * 0.7);
                    s *= smooth(0.0, 0.12, u) * smooth(0.0, 0.12, 1.0 - u);
                }
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
    tide(geo, occ, &mut out);
    out
}

/// How far over a drowned house's silt the water stood at its lowest
/// for years, m.
const TIDE: f32 = 1.0;

/// A drowned house's tide line (`Ruin::Drowned`: found by its silt): on
/// every face of its walls, in and out, the line `TIDE` over the silt and
/// the film down to the silt (outside, a little further).
fn tide(geo: &Geo, occ: &Occluders, out: &mut Vec<Decal>) {
    let Some(silt) = geo
        .pieces
        .iter()
        .filter(|p| p.mat == Mat::Silt && p.collide)
        .filter_map(|p| match p.shape {
            Shape::Box(s) if s.x * s.z > 4.0 => Some(p.at.translation.y + s.y * 0.5),
            _ => None,
        })
        .reduce(f32::max)
    else {
        return;
    };
    let line = silt + TIDE;
    const PROUD: f32 = 0.008;
    for p in &geo.pieces {
        let Shape::Box(s) = p.shape else { continue };
        let walls = matches!(
            p.mat,
            Mat::Brick | Mat::Planks | Mat::Sodden | Mat::Concrete | Mat::DressedStone
        );
        let (c, h) = (p.at.translation, s * 0.5);
        if !p.collide || !walls || c.y - h.y > line - 0.05 || c.y + h.y < line + 0.02 {
            continue;
        }
        let r = p.at.rotation;
        for (ln, lu, hn, hu) in [
            (Vec3::X, Vec3::Z, h.x, h.z),
            (Vec3::NEG_X, Vec3::Z, h.x, h.z),
            (Vec3::Z, Vec3::X, h.z, h.x),
            (Vec3::NEG_Z, Vec3::X, h.z, h.x),
        ] {
            let (n, u) = (r * ln, r * lu);
            if hu < 0.04 || n.y.abs() > 0.3 {
                continue;
            }
            let face = Vec3::new(c.x, line, c.z) + n * hn;
            if occ.inside(face + n * 0.04) {
                continue;
            }
            let outside = Vec2::new(face.x, face.z).dot(Vec2::new(n.x, n.z)) > 0.0
                && !occ.inside(face + n * 0.3 - Vec3::Y * TIDE);
            let foot = (c.y - h.y).max(if outside { silt - 0.6 } else { silt });
            out.push(Decal {
                kind: Stain::Tide,
                top: face + n * PROUD,
                n,
                u,
                width: hu * 2.0,
                length: (line - foot).max(0.1),
            });
        }
    }
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
pub struct StainAssets(pub [Handle<StandardMaterial>; 4]);

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
/// from the building's seed, and every boarded one's (`boarded_sashes`:
/// the glass went first). Pure, and the same headless: Destruction's
/// panes start broken where this says so, and `dress_buildings` swaps
/// those sashes for `window_sash_broken`.
pub fn weathered_sashes(prefab: &str, at: &Transform) -> Vec<bool> {
    super::buildings::prefab(prefab).map_or_else(Vec::new, |geo| weathered(&geo, prefab, at))
}

/// Which of its sashes someone boarded up, the same way: a quarter of a
/// damaged house's, one in twelve of a whole one's, none of a burnt or
/// drowned one's, nor of anything but the common houses. Destruction nails three `window_board`s across each, outside.
pub fn boarded_sashes(prefab: &str, at: &Transform) -> Vec<bool> {
    super::buildings::prefab(prefab).map_or_else(Vec::new, |geo| boarded(&geo, prefab, at))
}

fn sashes(geo: &Geo) -> usize {
    geo.props
        .iter()
        .flat_map(|d| d.models.iter())
        .filter(|(name, _)| *name == "window_sash")
        .count()
}

/// [`boarded_sashes`] for a prefab's geometry in hand.
pub(super) fn boarded(geo: &Geo, prefab: &str, at: &Transform) -> Vec<bool> {
    let xz = Vec2::new(at.translation.x, at.translation.z);
    let mut rng = fastrand::Rng::with_seed(super::overgrowth::seed(xz, prefab) ^ 0xb0a2d);
    // Only the common houses (their walls all 0.18 m, as the boards are
    // set for).
    let house = matches!(
        extraction_core::map::prefab_base(prefab),
        "house_plank" | "farmhouse" | "house_brick"
    );
    let share = if !house {
        0.0
    } else if prefab.ends_with("_damaged") {
        0.25
    } else if extraction_core::map::prefab_base(prefab) != prefab {
        0.0
    } else {
        1.0 / 12.0
    };
    (0..sashes(geo)).map(|_| rng.f32() < share).collect()
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
    boarded(geo, prefab, at)
        .into_iter()
        .map(|b| (rng.f32() < share) || b)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Snow lies on a house's roof slopes, with a cornice at each eave,
    /// and nowhere under its roof: every load is open to the sky.
    #[test]
    fn snow_lies_on_the_roofs_and_under_nothing() {
        for id in ["house_plank", "house_brick", "cabin_log", "shed", "chapel"] {
            let geo = crate::world::buildings::prefab(id).unwrap();
            let occ = crate::world::buildings::occluders(&geo);
            let load = snow_load(&geo, &occ);
            assert!(load.len() >= 4, "{id}: {} loads", load.len());
            assert!(
                load.iter().all(|p| p.mat == Mat::Snow && !p.collide),
                "{id}"
            );
            // On the roof: some of it well up, and sloped.
            assert!(
                load.iter()
                    .any(|p| p.at.translation.y > 2.5 && (p.at.rotation * Vec3::Y).y < 0.98),
                "{id}: no snow on a pitched roof"
            );
            for p in &load {
                let Shape::Box(s) = p.shape else { panic!() };
                let up = p.at.rotation * Vec3::Y;
                let top = p.at.translation + up * (s.y * 0.5 + 0.05);
                assert!(occ.sky(top, up) > 0.5, "{id}: snow under cover at {top:?}");
            }
            assert_eq!(snow_load(&geo, &occ).len(), load.len(), "{id}");
        }
    }

    /// The timber houses come in several faded paints and bare boards,
    /// each the same every time, none brighter than the boards.
    #[test]
    fn houses_are_painted_variously() {
        let paints: Vec<Vec3> = (0..64).map(paint).collect();
        assert_eq!(paints, (0..64).map(paint).collect::<Vec<_>>());
        assert!(
            paints
                .iter()
                .all(|p| p.max_element() <= 1.0 && p.min_element() > 0.5)
        );
        assert!(paints.contains(&Vec3::ONE), "no bare boards");
        // Fading keeps a paint's direction away from white: its colour.
        let kinds: Vec<_> = paints
            .iter()
            .filter(|p| **p != Vec3::ONE)
            .map(|p| {
                ((Vec3::ONE - *p).normalize() * 10.0)
                    .round()
                    .as_ivec3()
                    .to_array()
            })
            .collect();
        assert!(kinds.len() >= 4, "too few paints: {kinds:?}");
    }

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

    /// A drowned house has its tide line round its walls, in and out,
    /// and nothing else hangs on it but where the line runs.
    #[test]
    fn drowned_houses_have_a_tide_line() {
        for id in [
            "house_plank_drowned",
            "farmhouse_drowned",
            "house_brick_drowned",
            "chapel_drowned",
        ] {
            let geo = crate::world::buildings::prefab(id).unwrap();
            let occ = crate::world::buildings::occluders(&geo);
            let tide: Vec<_> = stains(&geo, &occ, 7, false)
                .into_iter()
                .filter(|d| d.kind == Stain::Tide)
                .collect();
            assert!(tide.len() >= 16, "{id}: {} tide decals", tide.len());
            let y = tide[0].top.y;
            assert!(tide.iter().all(|d| (d.top.y - y).abs() < 1e-4), "{id}");
            assert!(geo.doors.is_empty(), "{id}: a door left");
            assert!(
                !geo.pieces.iter().any(|p| p.mat == Mat::Glass),
                "{id}: glass"
            );
        }
        // And a dry house has none.
        let geo = crate::world::buildings::prefab("house_brick").unwrap();
        let occ = crate::world::buildings::occluders(&geo);
        assert!(
            !stains(&geo, &occ, 7, false)
                .iter()
                .any(|d| d.kind == Stain::Tide)
        );
    }

    /// A bowed roof: two pitched slabs, cut fine (their area kept), the
    /// middle of the ridge down by the bow, the gables' ends and the eaves
    /// where they were.
    #[test]
    fn a_roof_sags_in_the_middle() {
        let slab = |side: f32| {
            let pitch = 0.5f32;
            let run = 3.0;
            crate::render::materials::metric_box(Vec3::new(run / pitch.cos(), 0.1, 8.0))
                .transformed_by(
                    Transform::from_xyz(side * run * 0.5, 4.0 + run * 0.5 * pitch.tan(), 0.0)
                        .with_rotation(Quat::from_rotation_z(-side * pitch)),
                )
        };
        let mut m = slab(1.0);
        m.merge(&slab(-1.0)).unwrap();
        let positions = |m: &Mesh| match m.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(p)) => p.clone(),
            _ => panic!(),
        };
        let area = |m: &Mesh| {
            let p = positions(m);
            let Some(Indices::U32(i)) = m.indices() else {
                panic!()
            };
            i.chunks_exact(3)
                .map(|t| {
                    let [a, b, c] = [t[0], t[1], t[2]].map(|k| Vec3::from(p[k as usize]));
                    (b - a).cross(c - a).length() * 0.5
                })
                .sum::<f32>()
        };
        let roof = roof_bounds([&m]).unwrap();
        let mut flat = m.clone();
        flat.insert_indices(Indices::U32(match m.indices() {
            Some(Indices::U16(i)) => i.iter().map(|&k| k as u32).collect(),
            Some(Indices::U32(i)) => i.clone(),
            None => panic!(),
        }));
        let whole = area(&flat);
        subdivide(&mut flat, 0.6, roof.0.y + 0.02);
        // Cut, not torn: the same surface.
        assert!((area(&flat) - whole).abs() / whole < 1e-4);
        sag(&mut m, roof, 0.2);
        let (p, q) = (positions(&m), positions(&flat));
        assert_eq!(p.len(), q.len());
        assert!(p.len() > 200, "{} vertices", p.len());
        let drop: Vec<f32> = p.iter().zip(&q).map(|(a, b)| b[1] - a[1]).collect();
        assert!(drop.iter().all(|d| (-1e-4..=0.2 + 1e-4).contains(d)));
        // Most at the ridge's middle, none at the ends or the eaves.
        let most = p
            .iter()
            .zip(&drop)
            .filter(|(v, _)| v[0].abs() < 0.3 && v[2].abs() < 0.3)
            .map(|(_, d)| *d)
            .fold(0.0, f32::max);
        assert!(most > 0.15, "the ridge's middle down {most}");
        assert!(
            p.iter()
                .zip(&drop)
                .filter(|(v, _)| v[2].abs() > 3.99)
                .all(|(_, d)| d.abs() < 1e-4)
        );
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
        // A boarded one's glass went first; a burnt house has none to
        // board.
        for (x, z) in [(-18.0, 95.0), (22.0, 85.0), (4.0, -30.0), (60.0, 12.0)] {
            let at = Transform::from_xyz(x, 0.0, z);
            for id in ["house_plank", "house_brick_damaged", "farmhouse"] {
                let (w, b) = (weathered_sashes(id, &at), boarded_sashes(id, &at));
                assert_eq!(w.len(), b.len());
                assert!(w.iter().zip(&b).all(|(w, b)| *w || !*b), "{id}");
                assert_eq!(b, boarded_sashes(id, &at));
            }
            assert!(!boarded_sashes("house_plank_burnt", &at).iter().any(|b| *b));
        }
    }
}
