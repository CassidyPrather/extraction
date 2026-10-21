//! Where the trees go: scatters every forest polygon of a map.
//!
//! Pure and deterministic per map, so the game (colliders, meshes, the
//! robots' nav grid), the headless sims and the Unity exporter all see the
//! same forest.

use crate::geom::{self, P2};
use crate::map::{MapDef, TreeSpecies, prefab_clearance};
use crate::noise::Rng;
use crate::terrain::Heightmap;

/// Mesh variants the game builds per species.
pub const TREE_VARIANTS: usize = 3;

/// How far a trunk's origin sits below the ground, so it never shows a gap
/// on a slope.
pub const TREE_SINK: f32 = 0.15;

/// One tree, in map space.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeSpot {
    pub species: TreeSpecies,
    /// `0..TREE_VARIANTS`.
    pub variant: usize,
    /// `[x, y, z]`: map `x`/`z`, `y` the ground height less [`TREE_SINK`].
    pub pos: [f32; 3],
    /// Radians, counter-clockwise looking down (a rotation about +Y in the
    /// game's right-handed frame).
    pub yaw: f32,
    /// Uniform scale, 0.8..1.2.
    pub scale: f32,
}

/// Scatters trees through every forest polygon. Deterministic per map.
pub fn scatter_trees(map: &MapDef, hm: &Heightmap) -> Vec<TreeSpot> {
    let mut out = Vec::new();
    // Keep clear of structures and markers.
    let clear: Vec<(P2, f32)> = map
        .placements
        .iter()
        // (A site's kiosks are cleared after: see the end.)
        .filter(|p| !crate::underground::is_site(&p.prefab))
        .map(|p| (p.at, prefab_clearance(&p.prefab)))
        .chain(map.markers.iter().map(|m| (m.at, 8.0)))
        .collect();
    for forest in &map.forests {
        let mut rng = Rng::new(0x7EE5 ^ u64::from(forest.seed));
        let spacing = (100.0 / forest.density.max(0.01)).sqrt();
        let (lo, hi) = geom::bounds(&forest.polygon);
        let weights: Vec<f32> = forest.mix.iter().map(|(_, w)| *w).collect();
        let mut z = lo[1];
        while z < hi[1] {
            let mut x = lo[0];
            while x < hi[0] {
                let p = [
                    x + rng.range(-0.45, 0.45) * spacing,
                    z + rng.range(-0.45, 0.45) * spacing,
                ];
                x += spacing;
                if !geom::point_in_polygon(&forest.polygon, p) {
                    continue;
                }
                // Thin out toward the forest edge for a ragged treeline.
                let edge = geom::polygon_edge_distance(&forest.polygon, p);
                if edge < 12.0 && rng.f32() > edge / 12.0 {
                    continue;
                }
                if hm.road_at(p).weight > 0 || hm.water_level(p).is_some() {
                    continue;
                }
                if hm.wetness(p) > 0.7 || hm.slope_deg(p) > 38.0 {
                    continue;
                }
                if hm
                    .road_lines
                    .iter()
                    .any(|rl| geom::polyline_closest(&rl.points, p).0 < rl.width * 0.5 + 2.5)
                {
                    continue;
                }
                if clear.iter().any(|(c, r)| geom::dist(*c, p) < *r) {
                    continue;
                }
                let Some(k) = rng.weighted(&weights) else {
                    continue;
                };
                let species = forest.mix[k].0;
                let y = hm.height(p);
                out.push(TreeSpot {
                    species,
                    variant: rng.below(TREE_VARIANTS),
                    pos: [p[0], y - TREE_SINK, p[1]],
                    yaw: rng.range(0.0, std::f32::consts::TAU),
                    scale: rng.range(0.8, 1.2),
                });
            }
            z += spacing;
        }
    }
    // Off every site kiosk, the exits' too, after the scatter (as rocks).
    let kiosks = crate::underground::kiosk_clearings(map);
    out.retain(|t| {
        !kiosks
            .iter()
            .any(|(c, r)| geom::dist(*c, [t.pos[0], t.pos[2]]) < *r)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{KETTLE_PASS, MARROW_CREEK};

    #[test]
    fn forests_are_full_and_clear_of_roads_and_water() {
        for src in [MARROW_CREEK, KETTLE_PASS] {
            let map = MapDef::from_ron(src).unwrap();
            let hm = Heightmap::build(&map);
            let a = scatter_trees(&map, &hm);
            assert!(a.len() > 1000, "{}: {} trees", map.name, a.len());
            for t in &a {
                let p = [t.pos[0], t.pos[2]];
                assert!(hm.water_level(p).is_none());
                assert_eq!(hm.road_at(p).weight, 0);
                assert!((0.8..1.2).contains(&t.scale));
                assert!(t.variant < TREE_VARIANTS);
                assert!((t.pos[1] + TREE_SINK - hm.height(p)).abs() < 1e-4);
            }
        }
    }
}
