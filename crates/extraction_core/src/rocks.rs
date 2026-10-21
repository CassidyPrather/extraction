//! Boulders and outcrops: rock that breaks up the open ground.
//!
//! Pure and deterministic per map, like the forests, so the game (meshes,
//! colliders), the robots' nav grid and the sims all see the same rocks.
//! Where the ground is steep rock gathers (outcrops on hillsides and the
//! tops of rises), in clusters a few metres across, with a thin sprinkling
//! over the flats; rock fields come and go with a slow noise. Nothing lies
//! on roads or paths, in water, by buildings and markers, or round a tree.

use crate::geom::{self, P2};
use crate::map::{MapDef, MarkerKind, prefab_clearance};
use crate::noise::Rng;
use crate::terrain::Heightmap;

/// Mesh variants the game builds.
pub const ROCK_VARIANTS: usize = 6;

/// Rocks at least this big block the way (nav, colliders); smaller ones
/// are stepped over.
pub const SOLID_RADIUS: f32 = 0.45;

#[derive(Clone, Debug, PartialEq)]
pub struct RockSpot {
    /// `[x, y, z]`: map `x`/`z`; `y` the rock's centre, partly buried.
    pub pos: [f32; 3],
    /// Horizontal radius, metres.
    pub radius: f32,
    /// Height over width (its half-height is `radius * squash`): boulders
    /// sit low.
    pub squash: f32,
    /// Length over breadth: its local x is `radius * stretch` across, its
    /// z `radius / stretch`.
    pub stretch: f32,
    pub yaw: f32,
    /// `0..ROCK_VARIANTS`.
    pub variant: usize,
}

/// A deterministic hash of the map's name, so each map rolls its own.
fn seed(map: &MapDef) -> u64 {
    map.name.bytes().fold(0xB01D_E25u64, |h, b| {
        h.wrapping_mul(31).wrapping_add(u64::from(b))
    })
}

/// Smooth value noise in 0..1 at `p / scale` (for rock fields).
fn field(rng_seed: u64, p: P2, scale: f32) -> f32 {
    let hash = |i: i64, j: i64| {
        let mut r = Rng::new(
            rng_seed ^ (i as u64).wrapping_mul(0x9E37_79B9) ^ (j as u64).wrapping_mul(0x85EB_CA6B),
        );
        r.f32()
    };
    let (x, z) = (p[0] / scale, p[1] / scale);
    let (i, j) = (x.floor() as i64, z.floor() as i64);
    let (fx, fz) = (x - x.floor(), z - z.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v) = (s(fx), s(fz));
    let a = hash(i, j) + (hash(i + 1, j) - hash(i, j)) * u;
    let b = hash(i, j + 1) + (hash(i + 1, j + 1) - hash(i, j + 1)) * u;
    a + (b - a) * v
}

pub fn scatter_rocks(map: &MapDef, hm: &Heightmap) -> Vec<RockSpot> {
    let mut out = Vec::new();
    let base = seed(map);
    let mut rng = Rng::new(base);
    let clear: Vec<(P2, f32)> = map
        .placements
        .iter()
        .map(|p| (p.at, prefab_clearance(&p.prefab) + 2.0))
        // Round a rendezvous, the whole way in to the truck's tailgate.
        .chain(map.markers.iter().map(|m| {
            let r = match m.kind {
                MarkerKind::Rendezvous => crate::map_lint::TRUCK_APPROACH + 4.0,
                _ => 7.0,
            };
            (m.at, r)
        }))
        .collect();
    // Trees by 4 m cell, so a boulder doesn't grow round a trunk.
    let mut trees: std::collections::HashMap<(i32, i32), Vec<P2>> = Default::default();
    for t in crate::forest::scatter_trees(map, hm) {
        let p = [t.pos[0], t.pos[2]];
        trees
            .entry(((p[0] / 4.0).floor() as i32, (p[1] / 4.0).floor() as i32))
            .or_default()
            .push(p);
    }
    let near_tree = |p: P2, r: f32| {
        let (ci, cj) = ((p[0] / 4.0).floor() as i32, (p[1] / 4.0).floor() as i32);
        (-1..=1).any(|di| {
            (-1..=1).any(|dj| {
                trees
                    .get(&(ci + di, cj + dj))
                    .is_some_and(|v| v.iter().any(|t| geom::dist(*t, p) < r + 0.7))
            })
        })
    };
    let ok = |p: P2, r: f32| {
        !near_tree(p, r)
            && hm.road_at(p).weight == 0
            && hm.water_level(p).is_none()
            && hm.wetness(p) < 0.6
            && !hm
                .road_lines
                .iter()
                .any(|rl| geom::polyline_closest(&rl.points, p).0 < rl.width * 0.5 + 1.5 + r)
            && !clear.iter().any(|(c, cr)| geom::dist(*c, p) < *cr + r)
    };
    // A rock at `p`: its centre set so it rests in the ground. Its base
    // (`-0.55` of its half-height below the centre) goes below the lowest
    // ground round its rim, so no side hangs over a slope; `sink` buries
    // it further, as a share of its height. None if it would vanish
    // into the slope on the uphill side.
    let place = |rng: &mut Rng, p: P2, r: f32, squash: f32, sink: f32| -> Option<RockSpot> {
        let stretch = rng.range(0.75, 1.3);
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let h = r * squash;
        let reach = r * stretch.max(1.0 / stretch) * 0.85;
        let ring: Vec<f32> = (0..8)
            .map(|k| {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                hm.height([p[0] + a.cos() * reach, p[1] + a.sin() * reach])
            })
            .collect();
        let low = ring.iter().cloned().fold(f32::INFINITY, f32::min);
        let high = ring.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let y = (hm.height(p) + h - 2.0 * h * sink).min(low + 0.55 * h - 0.06);
        if y + 0.8 * h < high + 0.05 {
            return None;
        }
        Some(RockSpot {
            pos: [p[0], y, p[1]],
            radius: r,
            squash,
            stretch,
            yaw,
            variant: rng.below(ROCK_VARIANTS),
        })
    };
    let half = map.half() - map.fence_inset.max(10.0) - 4.0;
    let cell = 9.0;
    let mut z = -half;
    while z < half {
        let mut x = -half;
        while x < half {
            let c = [x + rng.range(0.0, cell), z + rng.range(0.0, cell)];
            x += cell;
            let slope = hm.slope_deg(c);
            if slope > 45.0 {
                continue;
            }
            // Steeper ground is rockier; a slow noise makes rock fields.
            let steep = ((slope - 7.0) / 22.0).clamp(0.0, 1.0);
            let fields = ((field(base, c, 90.0) - 0.45) * 3.0).clamp(0.0, 1.0);
            // Now and then a lone erratic: a boulder the ice left, big
            // enough to steer by.
            if rng.f32() < 0.012 {
                let r = rng.range(1.8, 3.2);
                if ok(c, r) {
                    let squash = rng.range(0.5, 0.75);
                    if let Some(rock) = place(&mut rng, c, r, squash, 0.3) {
                        out.push(rock);
                    }
                }
                continue;
            }
            // What's been placed in this cell, so a cluster's rocks lie
            // beside each other rather than through or on top.
            let mut here: Vec<(P2, f32)> = Vec::new();
            let apart = |here: &[(P2, f32)], p: P2, r: f32| {
                here.iter()
                    .all(|(q, rq)| geom::dist(*q, p) > 0.9 * (r + rq))
            };
            // Field stones: a few small ones, mostly in the rock fields and
            // on stony slopes, so the ground there isn't bare texture and
            // the meadows aren't peppered evenly.
            if rng.f32() < 0.06 + 0.45 * fields + 0.25 * steep {
                let n = 1 + rng.below(4);
                for _ in 0..n {
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    let d = rng.range(0.0, 4.0);
                    let p = [c[0] + 3.0 + a.cos() * d, c[1] + 3.0 + a.sin() * d];
                    let r = rng.range(0.1, 0.38);
                    if !ok(p, r) || !apart(&here, p, r) {
                        continue;
                    }
                    let squash = rng.range(0.35, 0.65);
                    if let Some(rock) = place(&mut rng, p, r, squash, 0.4) {
                        here.push((p, r));
                        out.push(rock);
                    }
                }
            }
            let chance = 0.18 + 0.5 * steep * steep + 0.45 * fields;
            if rng.f32() > chance {
                continue;
            }
            let n = 1 + rng.below(if steep > 0.4 || fields > 0.5 { 6 } else { 3 });
            let big = 0.8 + 1.8 * steep.max(fields * 0.7);
            for k in 0..n {
                let r = if k == 0 {
                    rng.range(0.5, 1.0) * big
                } else {
                    rng.range(0.2, 0.55) * big.min(1.4)
                };
                let off = if k == 0 {
                    [0.0, 0.0]
                } else {
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    let d = rng.range(0.8, 3.5) + r;
                    [a.cos() * d, a.sin() * d]
                };
                let p = [c[0] + off[0], c[1] + off[1]];
                if !ok(p, r) || !apart(&here, p, r) {
                    continue;
                }
                let squash = rng.range(0.45, 0.8);
                // Buried by a quarter of its height, more on a slope.
                if let Some(rock) = place(&mut rng, p, r, squash, 0.25 + 0.3 * steep) {
                    here.push((p, r));
                    out.push(rock);
                }
            }
        }
        z += cell;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rocks_break_up_the_ground_and_keep_off_roads() {
        for src in crate::map::VALLEYS.map(|v| v.1) {
            let map = MapDef::from_ron(src).unwrap();
            let hm = Heightmap::build(&map);
            let rocks = scatter_rocks(&map, &hm);
            assert!(
                (1000..20000).contains(&rocks.len()),
                "{}: {} rocks",
                map.name,
                rocks.len()
            );
            for r in &rocks {
                let p = [r.pos[0], r.pos[2]];
                assert_eq!(hm.road_at(p).weight, 0, "a rock on a road at {p:?}");
                assert!(hm.water_level(p).is_none());
                assert!(r.variant < ROCK_VARIANTS);
                // Partly buried: its top above ground, its bottom below.
                let g = hm.height(p);
                let half_h = r.radius * r.squash;
                assert!(
                    r.pos[1] - half_h < g && r.pos[1] + half_h > g,
                    "{r:?} at ground {g}"
                );
                // Seated: its base under the ground all round its rim, so
                // no side overhangs a slope.
                let reach = r.radius * r.stretch.max(1.0 / r.stretch) * 0.85;
                let low = (0..8)
                    .map(|k| {
                        let a = k as f32 * std::f32::consts::TAU / 8.0;
                        hm.height([p[0] + a.cos() * reach, p[1] + a.sin() * reach])
                    })
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    r.pos[1] - 0.55 * half_h < low,
                    "{r:?} overhangs: rim ground {low}"
                );
            }
            // Same map, same rocks.
            assert_eq!(rocks, scatter_rocks(&map, &hm));
        }
    }
}
