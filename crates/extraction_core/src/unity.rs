//! Helpers for exporting a map to a Unity scene (the VRChat port): the
//! coordinate conversion, terrain resampling and the splat (alpha) maps.
//! The `unity-export` binary does the file writing; this is the pure part.
//!
//! # Coordinates
//!
//! The game is right-handed, `+Y` up, metres, origin at the map centre,
//! `x` east and `+z` **south** (north is `-z`). Unity is left-handed, `+Y`
//! up, and its top view has `+z` pointing up the screen, so we make Unity
//! `+z` north:
//!
//! ```text
//! unity.x =  game.x      (east)
//! unity.y =  game.y      (up; absolute heights are kept as they are)
//! unity.z = -game.z      (north)
//! ```
//!
//! Flipping one axis turns a right-handed frame into a left-handed one
//! without mirroring the world, so the map comes out the right way round.
//! Rotations about `Y` reverse their sign under the flip: a map placement's
//! `rot` (degrees clockwise from north, looking down) is already Unity's
//! convention and passes through unchanged, and a game yaw in radians
//! (counter-clockwise looking down, Bevy's `from_rotation_y`) becomes
//! `-yaw`. See [`placement_yaw_to_unity`] and [`yaw_to_unity`].
//!
//! # Terrain
//!
//! A Unity Terrain sits at its minimum corner: we put it at
//! `(-size/2, min_height, -size/2)` with size `(size, max - min, size)`, so
//! it covers the same square as the game map and Unity world heights equal
//! game heights. Unity height samples are indexed `[row, column]` with row
//! 0 at the terrain's local `z = 0` (the south edge), column 0 at the west
//! edge.

use crate::geom::{self, P2};
use crate::map::{MapDef, RoadKind};
use crate::terrain::Heightmap;

/// Unity terrain heightmap resolution (must be `2^n + 1`).
pub const TERRAIN_RES: usize = 1025;
/// Unity terrain alphamap (splat) resolution (a power of two).
pub const ALPHA_RES: usize = 1024;

/// Game point `[x, y, z]` to Unity world `[x, y, z]`.
pub fn to_unity(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[1], -p[2]]
}

/// Game map point `[x, z]` to Unity `[x, z]`.
pub fn to_unity_2d(p: P2) -> [f32; 2] {
    [p[0], -p[1]]
}

/// Unity world `[x, y, z]` back to the game's `[x, y, z]`.
pub fn from_unity(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[1], -p[2]]
}

/// A map placement's `rot` (degrees clockwise from north, looking down)
/// as a Unity Y Euler angle in degrees, for an object that faces the same
/// way in both engines once converted with the `z` flip. Unity's positive
/// Y rotation is clockwise looking down, so this is the identity
/// (normalised to `0..360`).
pub fn placement_yaw_to_unity(rot_deg: f32) -> f32 {
    rot_deg.rem_euclid(360.0)
}

/// A game yaw in radians (counter-clockwise looking down, a rotation about
/// `+Y` in the right-handed frame) as a Unity yaw in radians (clockwise
/// looking down), normalised to `0..TAU`.
pub fn yaw_to_unity(yaw: f32) -> f32 {
    (-yaw).rem_euclid(std::f32::consts::TAU)
}

/// Bilinearly resamples a square grid of `n_src`² samples to `n_dst`²
/// samples spanning the same extent (corner samples land on corner
/// samples). Row-major, row `j` then column `i`.
pub fn resample_bilinear(src: &[f32], n_src: usize, n_dst: usize) -> Vec<f32> {
    assert!(n_src >= 2 && n_dst >= 2 && src.len() == n_src * n_src);
    let scale = (n_src - 1) as f64 / (n_dst - 1) as f64;
    // Per destination index: the source cell and the fraction across it.
    let taps: Vec<(usize, f32)> = (0..n_dst)
        .map(|d| {
            let g = d as f64 * scale;
            let i = (g.floor() as usize).min(n_src - 2);
            (i, (g - i as f64) as f32)
        })
        .collect();
    let mut out = Vec::with_capacity(n_dst * n_dst);
    for &(j, fz) in &taps {
        for &(i, fx) in &taps {
            let h00 = src[j * n_src + i];
            let h10 = src[j * n_src + i + 1];
            let h01 = src[(j + 1) * n_src + i];
            let h11 = src[(j + 1) * n_src + i + 1];
            let a = h00 + (h10 - h00) * fx;
            let b = h01 + (h11 - h01) * fx;
            out.push(a + (b - a) * fz);
        }
    }
    out
}

/// Reverses the row order of a row-major grid `width` wide. The game's
/// grids run north to south (row 0 at `z = -size/2`); Unity's heights run
/// south to north.
pub fn flip_rows<T: Clone>(grid: &[T], width: usize) -> Vec<T> {
    grid.chunks(width).rev().flatten().cloned().collect()
}

/// Smallest and largest value.
pub fn min_max(data: &[f32]) -> (f32, f32) {
    data.iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(*v), hi.max(*v))
        })
}

/// Heights to 16-bit samples: `lo` maps to 0, `hi` to 65535.
pub fn quantize_u16(data: &[f32], lo: f32, hi: f32) -> Vec<u16> {
    let range = (hi - lo).max(1e-6);
    data.iter()
        .map(|h| (((h - lo) / range).clamp(0.0, 1.0) * 65535.0).round() as u16)
        .collect()
}

/// The game heightmap resampled for a Unity terrain: `res`² samples, rows
/// south to north (Unity's `[row, column]` order), absolute heights.
pub fn unity_heights(hm: &Heightmap, res: usize) -> Vec<f32> {
    flip_rows(&resample_bilinear(&hm.heights, hm.n, res), res)
}

/// The terrain layers we paint, in alphamap order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplatLayer {
    Grass,
    /// Needles and leaf litter under the trees.
    Forest,
    /// Dirt tracks and footpaths.
    Dirt,
    /// Gravel roads and rail ballast.
    Gravel,
    Asphalt,
    /// Steep ground.
    Rock,
    /// Wet banks and stream beds.
    Mud,
}

impl SplatLayer {
    pub const ALL: [Self; 7] = [
        Self::Grass,
        Self::Forest,
        Self::Dirt,
        Self::Gravel,
        Self::Asphalt,
        Self::Rock,
        Self::Mud,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Grass => "grass",
            Self::Forest => "forest",
            Self::Dirt => "dirt",
            Self::Gravel => "gravel",
            Self::Asphalt => "asphalt",
            Self::Rock => "rock",
            Self::Mud => "mud",
        }
    }

    /// The layer a road kind paints.
    pub fn of_road(kind: RoadKind) -> Self {
        match kind {
            RoadKind::Asphalt => Self::Asphalt,
            RoadKind::Gravel | RoadKind::Rail => Self::Gravel,
            RoadKind::Dirt | RoadKind::Path => Self::Dirt,
        }
    }
}

/// Lays `a` of `layer` over the weights so far (they keep summing to 1).
fn over(w: &mut [f32; 7], layer: SplatLayer, a: f32) {
    let a = a.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    for v in w.iter_mut() {
        *v *= 1.0 - a;
    }
    w[layer as usize] += a;
}

/// Splat weights for every layer at `res`² texel centres, one grid per
/// layer, rows **north to south** (image order: the top of a PNG is
/// north). Weights at a texel sum to 1. The order of painting, each over
/// the last: grass, forest floor (soft polygon edges), mud from wetness,
/// rock from slope, roads by kind, mud under water.
pub fn splat_weights(map: &MapDef, hm: &Heightmap, res: usize) -> Vec<Vec<f32>> {
    let layers = SplatLayer::ALL.len();
    let mut out = vec![vec![0.0f32; res * res]; layers];
    let texel = map.size / res as f32;
    let half = map.half();
    let forests: Vec<(&[P2], P2, P2)> = map
        .forests
        .iter()
        .map(|f| {
            let (lo, hi) = geom::bounds(&f.polygon);
            (f.polygon.as_slice(), lo, hi)
        })
        .collect();
    let fade = 6.0;
    for row in 0..res {
        let z = -half + (row as f32 + 0.5) * texel;
        for col in 0..res {
            let x = -half + (col as f32 + 0.5) * texel;
            let p = [x, z];
            let mut w = [0.0f32; 7];
            w[SplatLayer::Grass as usize] = 1.0;
            let mut forest = 0.0f32;
            for (poly, lo, hi) in &forests {
                if x < lo[0] - fade || x > hi[0] + fade || z < lo[1] - fade || z > hi[1] + fade {
                    continue;
                }
                let d = geom::polygon_edge_distance(poly, p);
                let t = geom::smoothstep(0.0, fade, d) * 0.5;
                let f = if geom::point_in_polygon(poly, p) {
                    0.5 + t
                } else {
                    0.5 - t
                };
                forest = forest.max(f);
            }
            over(&mut w, SplatLayer::Forest, forest);
            over(
                &mut w,
                SplatLayer::Mud,
                geom::smoothstep(0.35, 0.8, hm.wetness(p)),
            );
            over(
                &mut w,
                SplatLayer::Rock,
                geom::smoothstep(32.0, 42.0, hm.slope_deg(p)),
            );
            for (layer, a) in road_paint(hm, p) {
                over(&mut w, layer, a);
            }
            if hm.water_level(p).is_some() {
                over(&mut w, SplatLayer::Mud, 1.0);
            }
            let k = row * res + col;
            for (l, v) in w.iter().enumerate() {
                out[l][k] = *v;
            }
        }
    }
    out
}

/// Road paint at a point, bilinear across the four cells around it, per
/// layer (0..=1), weakest first so the strongest is laid on top.
fn road_paint(hm: &Heightmap, p: P2) -> Vec<(SplatLayer, f32)> {
    let max = (hm.n - 1) as f32;
    let gx = ((p[0] - hm.origin) / hm.cell).clamp(0.0, max);
    let gz = ((p[1] - hm.origin) / hm.cell).clamp(0.0, max);
    let i = (gx.floor() as usize).min(hm.n - 2);
    let j = (gz.floor() as usize).min(hm.n - 2);
    let (fx, fz) = (gx - i as f32, gz - j as f32);
    let mut acc: Vec<(SplatLayer, f32)> = Vec::new();
    for (di, dj, f) in [
        (0, 0, (1.0 - fx) * (1.0 - fz)),
        (1, 0, fx * (1.0 - fz)),
        (0, 1, (1.0 - fx) * fz),
        (1, 1, fx * fz),
    ] {
        let paint = hm.roads[hm.idx(i + di, j + dj)];
        let Some(kind) = paint.kind else { continue };
        let layer = SplatLayer::of_road(kind);
        let a = f * paint.weight as f32 / 255.0;
        match acc.iter_mut().find(|(l, _)| *l == layer) {
            Some(e) => e.1 += a,
            None => acc.push((layer, a)),
        }
    }
    acc.sort_by(|a, b| a.1.total_cmp(&b.1));
    acc
}

/// Where the game stands a placement (see `Heightmap::footing`).
pub fn placement_ground(hm: &Heightmap, prefab: &str, at: P2, rot_deg: f32) -> f32 {
    hm.footing(prefab, at, rot_deg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_a_plane_is_exact_and_keeps_the_corners() {
        // h = 2x + 3z on a 5x5 grid; resampling a plane bilinearly at any
        // resolution gives the same plane.
        let n = 5;
        let src: Vec<f32> = (0..n * n)
            .map(|k| 2.0 * (k % n) as f32 + 3.0 * (k / n) as f32)
            .collect();
        let m = 17;
        let dst = resample_bilinear(&src, n, m);
        assert_eq!(dst.len(), m * m);
        let s = (n - 1) as f32 / (m - 1) as f32;
        for j in 0..m {
            for i in 0..m {
                let want = 2.0 * i as f32 * s + 3.0 * j as f32 * s;
                assert!((dst[j * m + i] - want).abs() < 1e-4, "{i},{j}");
            }
        }
        assert_eq!(dst[0], src[0]);
        assert_eq!(dst[m * m - 1], src[n * n - 1]);
        assert_eq!(dst[m - 1], src[n - 1]);
        assert_eq!(dst[(m - 1) * m], src[(n - 1) * n]);
    }

    #[test]
    fn resampling_to_the_same_size_is_the_identity() {
        let src: Vec<f32> = (0..36).map(|k| ((k * 7919) % 13) as f32).collect();
        assert_eq!(resample_bilinear(&src, 6, 6), src);
    }

    #[test]
    fn resampling_interpolates_between_samples() {
        // A 2x2 grid upsampled to 3x3: the middle is the mean.
        let dst = resample_bilinear(&[0.0, 4.0, 8.0, 12.0], 2, 3);
        assert_eq!(dst, vec![0.0, 2.0, 4.0, 4.0, 6.0, 8.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn unity_heights_match_the_game_heightmap() {
        let map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let hm = Heightmap::build(&map);
        let res = 129;
        let h = unity_heights(&hm, res);
        let step = map.size / (res - 1) as f32;
        for (row, col) in [(0, 0), (0, 128), (128, 0), (64, 64), (17, 101)] {
            // Unity row 0 is the south edge: game z = +size/2.
            let x = -map.half() + col as f32 * step;
            let z = map.half() - row as f32 * step;
            let want = hm.height([x, z]);
            let got = h[row * res + col];
            assert!((got - want).abs() < 1e-3, "{row},{col}: {got} vs {want}");
        }
    }

    #[test]
    fn quantizing_spans_the_full_range() {
        let q = quantize_u16(&[10.0, 15.0, 20.0, 25.0], 10.0, 20.0);
        assert_eq!(q, vec![0, 32768, 65535, 65535]);
        assert_eq!(min_max(&[3.0, -1.0, 7.5]), (-1.0, 7.5));
    }

    #[test]
    fn flipping_rows_puts_the_south_edge_first() {
        let g = [1, 2, 3, 4, 5, 6];
        assert_eq!(flip_rows(&g, 2), vec![5, 6, 3, 4, 1, 2]);
    }

    #[test]
    fn coordinates_flip_z_and_round_trip() {
        // North in the game (-z) is +z in Unity; east stays east.
        assert_eq!(to_unity([10.0, 2.0, -50.0]), [10.0, 2.0, 50.0]);
        assert_eq!(to_unity_2d([-3.0, 4.0]), [-3.0, -4.0]);
        let p = [12.5, -3.0, 99.0];
        assert_eq!(from_unity(to_unity(p)), p);
    }

    #[test]
    fn yaws_convert_to_the_same_facing() {
        use std::f32::consts::{FRAC_PI_2, TAU};
        // A placement facing east (90° clockwise from north) faces east in
        // Unity too, which is a +90° Euler Y.
        assert_eq!(placement_yaw_to_unity(90.0), 90.0);
        assert_eq!(placement_yaw_to_unity(-90.0), 270.0);
        // Rotate the game's "north" (-z) by a game yaw, convert the result,
        // and compare with Unity's "north" (+z) rotated by the Unity yaw.
        for yaw in [0.3f32, FRAC_PI_2, 2.0, 5.5] {
            // Bevy: from_rotation_y(yaw) * (0, 0, -1).
            let game = [-yaw.sin(), 0.0, -yaw.cos()];
            let u = yaw_to_unity(yaw);
            // Unity: Quaternion.Euler(0, u deg, 0) * (0, 0, 1).
            let unity = [u.sin(), 0.0, u.cos()];
            let conv = to_unity(game);
            for k in 0..3 {
                assert!(
                    (conv[k] - unity[k]).abs() < 1e-5,
                    "{yaw}: {conv:?} {unity:?}"
                );
            }
            assert!((0.0..TAU).contains(&u));
        }
        // Same check for a placement: the game turns a prefab with
        // from_rotation_y(-rot).
        for rot in [0.0f32, 37.0, 90.0, 200.0] {
            let r = -rot.to_radians();
            let game = [-r.sin(), 0.0, -r.cos()];
            let u = placement_yaw_to_unity(rot).to_radians();
            let unity = [u.sin(), 0.0, u.cos()];
            let conv = to_unity(game);
            for k in 0..3 {
                assert!(
                    (conv[k] - unity[k]).abs() < 1e-5,
                    "{rot}: {conv:?} {unity:?}"
                );
            }
        }
    }

    #[test]
    fn splats_sum_to_one_and_paint_roads_and_water() {
        let map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let hm = Heightmap::build(&map);
        let res = 64;
        let w = splat_weights(&map, &hm, res);
        assert_eq!(w.len(), SplatLayer::ALL.len());
        for k in 0..res * res {
            let s: f32 = w.iter().map(|l| l[k]).sum();
            assert!((s - 1.0).abs() < 1e-4, "texel {k} sums to {s}");
        }
        // Somewhere is road, somewhere is forest, somewhere is mud.
        for layer in [SplatLayer::Forest, SplatLayer::Mud, SplatLayer::Grass] {
            assert!(
                w[layer as usize].iter().any(|v| *v > 0.9),
                "{} never dominates",
                layer.name()
            );
        }
        let road = [SplatLayer::Asphalt, SplatLayer::Gravel, SplatLayer::Dirt];
        assert!(road.iter().any(|l| w[*l as usize].iter().any(|v| *v > 0.5)));
    }
}
