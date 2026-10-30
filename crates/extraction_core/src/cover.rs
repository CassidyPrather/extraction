//! Ground cover: grass tufts in the open, ferns under the trees, sedge on
//! wet ground. Decoration only (nothing collides with it), but pure and
//! deterministic per patch of ground so the game can stream it in and out
//! round the player and get the same plants back each time.

use crate::geom::{self, P2};
use crate::map::{MapDef, prefab_footprint};
use crate::noise::{self, Rng};
use crate::terrain::Heightmap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoverKind {
    Grass,
    /// Taller, darker grass on wet ground.
    Sedge,
    Fern,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoverSpot {
    /// Base on the ground.
    pub pos: [f32; 3],
    pub kind: CoverKind,
    /// About 1: height and spread multiply by it.
    pub scale: f32,
    pub yaw: f32,
    /// A free random number per plant, for the mesh's own variation.
    pub seed: u32,
    /// 0..1, uniform: the game keeps only the low ranks further out.
    pub rank: f32,
}

/// What a map's cover needs to know besides the heightmap: the forests'
/// outlines and the buildings' floors, worked out once.
pub struct CoverMap {
    seed: u32,
    /// A burnt valley (`MapDef::ash`): drier, thinner cover.
    pub burnt: bool,
    /// Trees by 4 m cell: ferns grow in their shade, not in the open.
    trees: std::collections::HashMap<(i32, i32), Vec<P2>>,
    forests: Vec<(Vec<P2>, (P2, P2), f32)>,
    /// Footprints: centre, sin/cos of the yaw, half width and depth.
    floors: Vec<(P2, (f32, f32), f32, f32)>,
}

impl CoverMap {
    pub fn new(map: &MapDef, hm: &Heightmap) -> Self {
        let mut trees: std::collections::HashMap<(i32, i32), Vec<P2>> = Default::default();
        for t in crate::forest::scatter_trees(map, hm) {
            let p = [t.pos[0], t.pos[2]];
            trees
                .entry(((p[0] / 4.0).floor() as i32, (p[1] / 4.0).floor() as i32))
                .or_default()
                .push(p);
        }
        Self {
            seed: map.noise.seed ^ 0xC0FE,
            burnt: map.ash > 0.0,
            trees,
            forests: map
                .forests
                .iter()
                .map(|f| (f.polygon.clone(), geom::bounds(&f.polygon), f.undergrowth))
                .collect(),
            floors: map
                .placements
                .iter()
                .map(|p| {
                    let [w, _, d] = prefab_footprint(&p.prefab);
                    (
                        p.at,
                        p.rot.to_radians().sin_cos(),
                        w * 0.5 + 0.4,
                        d * 0.5 + 0.4,
                    )
                })
                .collect(),
        }
    }

    /// The undergrowth of the forest over `p`, if one is.
    fn forest(&self, p: P2) -> Option<f32> {
        self.forests.iter().find_map(|(poly, (lo, hi), u)| {
            (p[0] >= lo[0]
                && p[0] <= hi[0]
                && p[1] >= lo[1]
                && p[1] <= hi[1]
                && geom::point_in_polygon(poly, p))
            .then_some(*u)
        })
    }

    /// Whether a tree stands within `r` metres.
    fn shaded(&self, p: P2, r: f32) -> bool {
        let (ci, cj) = ((p[0] / 4.0).floor() as i32, (p[1] / 4.0).floor() as i32);
        let n = (r / 4.0).ceil() as i32;
        (-n..=n).any(|di| {
            (-n..=n).any(|dj| {
                self.trees
                    .get(&(ci + di, cj + dj))
                    .is_some_and(|v| v.iter().any(|t| geom::dist(*t, p) < r))
            })
        })
    }

    fn on_a_floor(&self, p: P2) -> bool {
        self.floors.iter().any(|(at, (s, c), hw, hd)| {
            let (dx, dz) = (p[0] - at[0], p[1] - at[1]);
            if dx.abs() > hw + hd || dz.abs() > hw + hd {
                return false;
            }
            let lx = dx * c + dz * s;
            let lz = -dx * s + dz * c;
            lx.abs() < *hw && lz.abs() < *hd
        })
    }

    /// The plants on the square patch `[x, z]..[x + size, z + size]`.
    pub fn patch(&self, hm: &Heightmap, lo: P2, size: f32) -> Vec<CoverSpot> {
        let mut out = Vec::new();
        let key = ((lo[0] / size).floor() as i64, (lo[1] / size).floor() as i64);
        let mut rng = Rng::new(
            u64::from(self.seed)
                ^ (key.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (key.1 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F),
        );
        // The ground ends at the map's edge; past it a height is the edge's,
        // so a plant there would hang in the air beyond the last of the land.
        let (min, max) = (hm.origin, hm.origin + (hm.n - 1) as f32 * hm.cell);
        let cell = 0.55;
        let n = (size / cell).round() as i32;
        let step = size / n as f32;
        for j in 0..n {
            for i in 0..n {
                // Jittered past the cell's edges (and kept in the patch), so
                // the plants don't line up on a grid.
                let p = [
                    (lo[0] + (i as f32 + rng.range(-0.6, 1.6)) * step)
                        .clamp(lo[0], lo[0] + size - 1e-3),
                    (lo[1] + (j as f32 + rng.range(-0.6, 1.6)) * step)
                        .clamp(lo[1], lo[1] + size - 1e-3),
                ];
                if p[0] <= min || p[0] >= max || p[1] <= min || p[1] >= max {
                    continue;
                }
                let (roll, scale, yaw, seed, rank) = (
                    rng.f32(),
                    rng.range(0.7, 1.3),
                    rng.range(0.0, std::f32::consts::TAU),
                    rng.next_u64() as u32,
                    rng.f32(),
                );
                if hm.water_level(p).is_some()
                    || hm.road_at(p).weight > 40
                    || hm.slope_deg(p) > 38.0
                {
                    continue;
                }
                // Nothing under (or poking through) a bridge deck.
                if hm.road_lines.iter().any(|rl| {
                    !rl.bridges.is_empty() && {
                        let (d, s, _) = geom::polyline_closest(&rl.points, p);
                        d < rl.width * 0.5 + 2.0 && rl.on_bridge(s)
                    }
                }) {
                    continue;
                }
                // Meadows come and go: patches thick with grass, patches
                // short and bare.
                let meadow = geom::smoothstep(
                    -0.35,
                    0.35,
                    noise::fbm(p[0] / 38.0, p[1] / 38.0, self.seed, 3, 0.5),
                );
                let wet = hm.wetness(p);
                let shade = self.forest(p).filter(|_| self.shaded(p, 6.0));
                let fern_share = if self.burnt { 0.3 } else { 1.0 };
                let (kind, chance) = match shade {
                    // Ferns are big: fewer to the square metre than grass,
                    // and only in a tree's shade.
                    Some(u) if roll < (0.08 * u + 0.025) * fern_share => (CoverKind::Fern, 1.0),
                    Some(_) => (CoverKind::Grass, 0.15 + 0.25 * meadow),
                    None if wet > 0.35 => (CoverKind::Sedge, 0.35),
                    None => (CoverKind::Grass, 0.45 + 0.5 * meadow),
                };
                let chance = if self.burnt { chance * 0.55 } else { chance };
                // A second roll for the chance so it isn't tied to the fern pick.
                if kind != CoverKind::Fern && (roll * 7.31).fract() > chance {
                    continue;
                }
                // Thin out by the road's shoulders.
                if hm.road_at(p).weight > 0 && rng.f32() < 0.7 {
                    continue;
                }
                if self.on_a_floor(p) {
                    continue;
                }
                out.push(CoverSpot {
                    pos: [p[0], hm.height(p) - 0.03, p[1]],
                    kind,
                    scale: match kind {
                        CoverKind::Grass => scale * (0.75 + 0.5 * meadow),
                        _ => scale,
                    },
                    yaw,
                    seed,
                    rank,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::MARROW_CREEK;

    /// Patches over the map's edge keep their plants on the land: past
    /// the edge there is no ground to stand on.
    #[test]
    fn no_cover_past_the_edge_of_the_map() {
        let map = MapDef::from_ron(MARROW_CREEK).unwrap();
        let hm = Heightmap::build(&map);
        let cover = CoverMap::new(&map, &hm);
        let half = map.half();
        let mut kept = 0;
        for lo in [
            [half - 8.0, 0.0],
            [-half - 8.0, 0.0],
            [0.0, half - 8.0],
            [0.0, -half - 8.0],
        ] {
            for c in cover.patch(&hm, lo, 16.0) {
                assert!(
                    c.pos[0].abs() <= half && c.pos[2].abs() <= half,
                    "a plant at {:?}, past the edge at {half}",
                    c.pos
                );
                kept += 1;
            }
        }
        assert!(kept > 0, "the inner half of each patch keeps its plants");
    }

    #[test]
    fn cover_grows_in_the_open_and_under_trees_but_not_on_roads_or_floors() {
        let map = MapDef::from_ron(MARROW_CREEK).unwrap();
        let hm = Heightmap::build(&map);
        let cover = CoverMap::new(&map, &hm);
        let size = 24.0;
        let mut counts = std::collections::HashMap::new();
        let mut patches = 0;
        // The middle three quarters of the valley, whatever its size.
        let reach = map.size * 0.375;
        let mut z = -reach;
        while z < reach {
            let mut x = -reach;
            while x < reach {
                let a = cover.patch(&hm, [x, z], size);
                assert_eq!(a, cover.patch(&hm, [x, z], size), "same patch, same plants");
                for s in &a {
                    let p = [s.pos[0], s.pos[2]];
                    assert!(p[0] >= x && p[0] < x + size && p[1] >= z && p[1] < z + size);
                    assert!(hm.road_at(p).weight <= 40, "a plant on the road at {p:?}");
                    assert!(hm.water_level(p).is_none());
                    assert!(!cover.on_a_floor(p));
                    if s.kind == CoverKind::Fern {
                        assert!(cover.shaded(p, 6.0), "a fern in the open at {p:?}");
                    }
                    *counts.entry(s.kind).or_insert(0usize) += 1;
                }
                patches += 1;
                x += size * 3.0;
            }
            z += size * 3.0;
        }
        let per_patch = counts.values().sum::<usize>() as f32 / patches as f32;
        assert!(
            per_patch > 600.0,
            "{per_patch} plants per patch: {counts:?}"
        );
        for k in [CoverKind::Grass, CoverKind::Fern] {
            assert!(
                counts.get(&k).copied().unwrap_or(0) > 100,
                "too little {k:?}: {counts:?}"
            );
        }
    }
}
