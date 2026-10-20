//! How things break: a flat piece (a pane, a board) cut into convex
//! pieces, ahead of time and deterministically, so the game only has to
//! throw them. Pure geometry in a piece's own plane (metres, centred on
//! the piece); the game extrudes the polygons by the thickness.
//!
//! Glass breaks round where it was struck: a Voronoi cut of the pane with
//! its seeds crowded near the impact, so the shards there are small and
//! the ones out by the frame big; the frame keeps jagged teeth of the
//! border cells. Wood breaks along its grain: a board splits across in
//! long oblique cuts (pointed ends) and now and then along its length.

use crate::noise::Rng;

pub type P2 = [f32; 2];

/// One convex piece: its outline (counter-clockwise) about its own
/// centroid, and where that centroid sits on the whole piece.
#[derive(Clone, Debug, PartialEq)]
pub struct Shard {
    pub poly: Vec<P2>,
    pub at: P2,
}

impl Shard {
    fn from_world(poly: Vec<P2>) -> Self {
        let c = centroid(&poly);
        Self {
            poly: poly.iter().map(|p| [p[0] - c[0], p[1] - c[1]]).collect(),
            at: c,
        }
    }

    pub fn area(&self) -> f32 {
        area(&self.poly)
    }

    /// Half the outline's extent along each axis (a box that holds it).
    pub fn half_extents(&self) -> P2 {
        let mut h = [0.0f32; 2];
        for p in &self.poly {
            h[0] = h[0].max(p[0].abs());
            h[1] = h[1].max(p[1].abs());
        }
        h
    }
}

/// A broken pane: what stays in the frame and what falls out of it.
#[derive(Clone, Debug, Default)]
pub struct Shatter {
    pub kept: Vec<Shard>,
    pub falling: Vec<Shard>,
}

/// Signed area (positive counter-clockwise).
pub fn area(poly: &[P2]) -> f32 {
    let n = poly.len();
    let mut a = 0.0;
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a * 0.5
}

pub fn centroid(poly: &[P2]) -> P2 {
    let n = poly.len();
    let a = area(poly);
    if a.abs() < 1e-9 {
        let s = poly
            .iter()
            .fold([0.0, 0.0], |s, p| [s[0] + p[0], s[1] + p[1]]);
        return [s[0] / n.max(1) as f32, s[1] / n.max(1) as f32];
    }
    let (mut cx, mut cy) = (0.0, 0.0);
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let k = p[0] * q[1] - q[0] * p[1];
        cx += (p[0] + q[0]) * k;
        cy += (p[1] + q[1]) * k;
    }
    [cx / (6.0 * a), cy / (6.0 * a)]
}

/// The part of a convex polygon where `p . n <= d`.
pub fn clip(poly: &[P2], n: P2, d: f32) -> Vec<P2> {
    let side = |p: P2| p[0] * n[0] + p[1] * n[1] - d;
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        let (sp, sq) = (side(p), side(q));
        if sp <= 0.0 {
            out.push(p);
        }
        if (sp <= 0.0) != (sq <= 0.0) {
            let t = sp / (sp - sq);
            out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
        }
    }
    out
}

fn rect(half: P2) -> Vec<P2> {
    vec![
        [-half[0], -half[1]],
        [half[0], -half[1]],
        [half[0], half[1]],
        [-half[0], half[1]],
    ]
}

/// The Voronoi cells of `seeds` within the rectangle `half`: each seed's
/// cell is the rectangle clipped by the bisector with every other seed.
pub fn voronoi(half: P2, seeds: &[P2]) -> Vec<Vec<P2>> {
    seeds
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut cell = rect(half);
            for (j, o) in seeds.iter().enumerate() {
                if i == j || cell.len() < 3 {
                    continue;
                }
                let n = [o[0] - s[0], o[1] - s[1]];
                let mid = [(o[0] + s[0]) * 0.5, (o[1] + s[1]) * 0.5];
                let d = n[0] * mid[0] + n[1] * mid[1];
                cell = clip(&cell, n, d);
            }
            cell
        })
        .filter(|c| c.len() >= 3 && area(c) > 1e-7)
        .collect()
}

/// Smallest piece worth a body, square metres (anything smaller is
/// dust and glitter, left to the impact effects).
pub const MIN_SHARD: f32 = 0.0004;

/// Shatter a pane of half extents `half` struck at `impact` (in the
/// pane's plane, clamped into it) into about `cells` pieces.
pub fn shatter(half: P2, impact: P2, seed: u64, cells: usize) -> Shatter {
    let mut rng = Rng::new(seed ^ 0x6_1A55);
    let impact = [
        impact[0].clamp(-half[0], half[0]),
        impact[1].clamp(-half[1], half[1]),
    ];
    let reach = (half[0] * half[0] + half[1] * half[1]).sqrt() * 2.0;
    let mut seeds = Vec::with_capacity(cells);
    // Crowded round the impact: radius ~ u^2 puts most of them close.
    let mut tries = 0;
    while seeds.len() < cells && tries < cells * 8 {
        tries += 1;
        let r = reach * rng.f32().powf(2.0);
        let a = rng.range(0.0, std::f32::consts::TAU);
        let p = [impact[0] + r * a.cos(), impact[1] + r * a.sin()];
        if p[0].abs() < half[0] && p[1].abs() < half[1] {
            seeds.push(p);
        }
    }
    if seeds.len() < 2 {
        seeds = vec![[-half[0] * 0.5, 0.0], [half[0] * 0.5, 0.0]];
    }
    let mut out = Shatter::default();
    let eps = 1e-4;
    for cell in voronoi(half, &seeds) {
        // Which borders it touches: x-, x+, y-, y+.
        let touches = |axis: usize, sign: f32| {
            cell.iter()
                .any(|p| (p[axis] * sign - half[axis]).abs() < eps)
        };
        let borders: Vec<(usize, f32)> = [(0, -1.0), (0, 1.0), (1, -1.0), (1, 1.0)]
            .into_iter()
            .filter(|(a, s)| touches(*a, *s))
            .collect();
        // A border cell leaves a tooth in the frame: cut it with a slanted
        // line and keep the border's side.
        if let Some(&(axis, sign)) = borders.first() {
            if rng.chance(0.85) {
                // Inward from that border, swung up to 50 degrees.
                let inward = if axis == 0 {
                    [-sign, 0.0]
                } else {
                    [0.0, -sign]
                };
                let swing = rng.range(-0.9, 0.9);
                let (s, c) = swing.sin_cos();
                let n = [inward[0] * c - inward[1] * s, inward[0] * s + inward[1] * c];
                // Through a point a share of the way in from the border.
                let depth = half[axis] * rng.range(0.08, 0.3);
                let mut at = [0.0f32; 2];
                at[axis] = sign * (half[axis] - depth);
                let other = 1 - axis;
                let span = cell.iter().map(|p| p[other]);
                let (lo, hi) = span.fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(v), h.max(v)));
                at[other] = (lo + hi) * 0.5;
                // Keep the side away from `n` (toward the border).
                let d = n[0] * at[0] + n[1] * at[1];
                let kept = clip(&cell, n, d);
                let rest = clip(&cell, [-n[0], -n[1]], -d);
                if kept.len() >= 3 && area(&kept) > MIN_SHARD * 0.25 {
                    out.kept.push(Shard::from_world(kept));
                }
                if rest.len() >= 3 && area(&rest) >= MIN_SHARD {
                    out.falling.push(Shard::from_world(rest));
                }
                continue;
            }
        }
        if area(&cell) >= MIN_SHARD {
            out.falling.push(Shard::from_world(cell));
        }
    }
    out
}

/// Split a board (`half`: half its length along x, half its width along
/// y) into splinters: oblique cuts across the grain, an occasional split
/// along it. About `pieces` pieces, every one convex.
pub fn splinter(half: P2, seed: u64, pieces: usize) -> Vec<Shard> {
    let mut rng = Rng::new(seed ^ 0x5_B117);
    let mut polys = vec![rect(half)];
    // Lengthwise splits first (the grain), then across.
    let along = (pieces / 4).clamp(0, 2);
    let across = pieces.saturating_sub(1).max(1) / (along + 1);
    let mut cut = |polys: Vec<Vec<P2>>, n: P2, d: f32| -> Vec<Vec<P2>> {
        polys
            .into_iter()
            .flat_map(|p| {
                let a = clip(&p, n, d);
                let b = clip(&p, [-n[0], -n[1]], -d);
                [a, b]
            })
            .filter(|p| p.len() >= 3 && area(p) > 1e-7)
            .collect()
    };
    for _ in 0..along {
        let y = rng.range(-half[1] * 0.6, half[1] * 0.6);
        // A split follows the grain, a few degrees off it.
        let a = rng.range(-0.03, 0.03);
        let n = [a.sin(), a.cos()];
        polys = cut(polys, n, n[1] * y);
    }
    for k in 0..across {
        // Spread along the length, each cut steep (a pointed break).
        let x = -half[0] + 2.0 * half[0] * (k as f32 + rng.range(0.3, 0.7)) / across as f32;
        let a = rng.range(0.25, 0.5) * if rng.chance(0.5) { 1.0 } else { -1.0 };
        let n = [a.cos(), a.sin()];
        polys = cut(polys, n, n[0] * x);
    }
    polys.into_iter().map(Shard::from_world).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(shards: &[Shard]) -> f32 {
        shards.iter().map(|s| s.area()).sum()
    }

    fn convex(poly: &[P2]) -> bool {
        let n = poly.len();
        (0..n).all(|i| {
            let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
            (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]) >= -1e-6
        })
    }

    #[test]
    fn voronoi_cells_tile_the_rectangle() {
        let half = [0.4, 0.5];
        let seeds = [
            [0.0, 0.0],
            [0.2, 0.3],
            [-0.3, -0.2],
            [0.35, -0.45],
            [-0.1, 0.4],
        ];
        let cells = voronoi(half, &seeds);
        assert_eq!(cells.len(), seeds.len());
        let sum: f32 = cells.iter().map(|c| area(c)).sum();
        assert!((sum - 0.8).abs() < 1e-4, "cells cover the pane ({sum})");
        assert!(cells.iter().all(|c| convex(c)));
    }

    #[test]
    fn a_shattered_pane_is_all_there_and_crowded_at_the_impact() {
        let half = [0.21, 0.24];
        let impact = [0.08, -0.1];
        for seed in 0..20 {
            let s = shatter(half, impact, seed, 18);
            let whole = 4.0 * half[0] * half[1];
            let got = total(&s.kept) + total(&s.falling);
            // Only crumbs below MIN_SHARD are let go.
            assert!(
                got <= whole + 1e-4 && got > whole * 0.9,
                "seed {seed}: {got} of {whole}"
            );
            assert!(!s.falling.is_empty() && !s.kept.is_empty(), "seed {seed}");
            for sh in s.kept.iter().chain(&s.falling) {
                assert!(convex(&sh.poly), "seed {seed}: a shard isn't convex");
                assert!(sh.area() > 0.0, "wound counter-clockwise");
            }
            // The pieces near the impact are smaller than those far off.
            let near: Vec<f32> = s
                .falling
                .iter()
                .filter(|f| (f.at[0] - impact[0]).hypot(f.at[1] - impact[1]) < 0.1)
                .map(|f| f.area())
                .collect();
            let far: Vec<f32> = s
                .falling
                .iter()
                .filter(|f| (f.at[0] - impact[0]).hypot(f.at[1] - impact[1]) > 0.2)
                .map(|f| f.area())
                .collect();
            if !near.is_empty() && !far.is_empty() {
                let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
                assert!(mean(&near) < mean(&far), "seed {seed}");
            }
        }
    }

    #[test]
    fn teeth_stay_by_the_frame() {
        let half = [0.3, 0.3];
        let s = shatter(half, [0.0, 0.0], 7, 20);
        for k in &s.kept {
            // Some vertex on the border.
            let on = k.poly.iter().any(|p| {
                let w = [p[0] + k.at[0], p[1] + k.at[1]];
                (w[0].abs() - half[0]).abs() < 1e-3 || (w[1].abs() - half[1]).abs() < 1e-3
            });
            assert!(on, "a kept shard hangs from the frame");
        }
    }

    #[test]
    fn the_same_break_twice_is_the_same() {
        assert_eq!(
            shatter([0.3, 0.4], [0.1, 0.0], 99, 16).falling,
            shatter([0.3, 0.4], [0.1, 0.0], 99, 16).falling
        );
    }

    #[test]
    fn a_board_splinters_into_long_convex_pieces() {
        let half = [0.9, 0.075];
        for seed in 0..20 {
            let s = splinter(half, seed, 5);
            assert!(s.len() >= 3, "seed {seed}: {} pieces", s.len());
            assert!((total(&s) - 4.0 * half[0] * half[1]).abs() < 1e-4);
            assert!(s.iter().all(|p| convex(&p.poly)));
            // Splinters, not chips: the longest piece runs along the grain.
            let longest = s
                .iter()
                .map(|p| p.half_extents())
                .max_by(|a, b| a[0].total_cmp(&b[0]))
                .unwrap();
            assert!(longest[0] > longest[1] * 2.0, "seed {seed}");
        }
    }
}
