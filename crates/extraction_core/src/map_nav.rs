//! The Zone's ground navigation grid, built from a map: terrain, building
//! footprints, tree trunks, big rocks, forest drag and bridge decks.
//!
//! The walkers path on it, the simulation bots path on it, and the map
//! lint (`map_lint`) checks reachability and routes on it, so all three
//! agree on where you can walk.

use crate::geom::polyline_at;
use crate::map::{MapDef, RoadKind, prefab_footprint, prefab_walls};
use crate::nav::{NavGrid, Obstacle};
use crate::terrain::Heightmap;

/// Cell size of the Zone's navigation grid, metres.
pub const CELL: f32 = 2.0;
/// The steepest ground a walker paths over, degrees.
pub const MAX_SLOPE: f32 = 32.0;

/// The navigation grid for ground robots (and bots): `max_slope` is the
/// steepest ground that counts as walkable ([`MAX_SLOPE`] for the walkers;
/// the lint also builds a stricter one for comfortable walking).
pub fn zone_grid(map: &MapDef, hm: &Heightmap, max_slope: f32) -> NavGrid {
    let mut g = NavGrid::from_heightmap(hm, CELL, max_slope);
    for p in map
        .placements
        .iter()
        .filter(|p| !crate::underground::is_site(&p.prefab))
    {
        let f = prefab_footprint(&p.prefab);
        // Anything thinner than a cell (a garden fence's 0.2 m) needs the
        // clearance to reach half a cell, or the grid's cell centres fall
        // either side of it and a path goes straight through.
        let thin = f[0].min(f[2]) * 0.5;
        g.block(
            Obstacle::Rect {
                at: p.at,
                half: [f[0] * 0.5, f[2] * 0.5],
                yaw: p.rot.to_radians(),
            },
            (CELL * 0.5 - thin).max(0.6),
        );
        let (sn, c) = p.rot.to_radians().sin_cos();
        for (o, half) in prefab_walls(&p.prefab) {
            g.block(
                Obstacle::Rect {
                    at: [
                        p.at[0] + o[0] * c - o[1] * sn,
                        p.at[1] + o[0] * sn + o[1] * c,
                    ],
                    half: *half,
                    yaw: p.rot.to_radians(),
                },
                0.6,
            );
        }
    }
    // Underground sites' kiosks stand on the surface like placements (the
    // shells: their aprons are open ground).
    for (at, rot) in crate::underground::kiosks(map) {
        g.block(
            Obstacle::Rect {
                at,
                half: crate::underground::SHELL_HALF,
                yaw: rot.to_radians(),
            },
            0.6,
        );
    }
    for t in crate::forest::scatter_trees(map, hm) {
        g.block(
            Obstacle::Circle {
                at: [t.pos[0], t.pos[2]],
                radius: 0.3 * t.scale,
            },
            0.5,
        );
    }
    for r in crate::rocks::scatter_rocks(map, hm) {
        if r.radius >= crate::rocks::SOLID_RADIUS {
            g.block(
                Obstacle::Circle {
                    at: [r.pos[0], r.pos[2]],
                    radius: r.radius * 0.9,
                },
                0.5,
            );
        }
    }
    for f in &map.forests {
        g.scale_cost_in(&f.polygon, 1.6);
    }
    // Bridges are walkable even though the water under them isn't; the
    // bank beneath and beside a deck isn't (no headroom under it, rails
    // along it): the way is along the deck, following its curve.
    for rl in &hm.road_lines {
        let half = if matches!(rl.kind, RoadKind::Path) {
            0.9
        } else {
            rl.width * 0.5 + 0.4
        };
        for (a, b) in &rl.bridges {
            let at = |s: f32| polyline_at(&rl.points, s).0;
            let mut s = *a + 1.0;
            while s < *b - 1.0 {
                g.block(
                    Obstacle::Circle {
                        at: at(s),
                        radius: half,
                    },
                    0.6,
                );
                s += 1.0;
            }
            // Waypoints follow the deck's own line.
            let mut lane = Vec::new();
            let mut s = *a - 3.0;
            while s <= *b + 3.0 {
                lane.push(at(s));
                s += 1.0;
            }
            g.lanes.push(lane);
            // Out past where the blocking reaches, onto the road.
            let mut s = *a - 5.0;
            while s < *b + 5.0 {
                g.open_line(at(s), at((s + 2.0).min(*b + 5.0)), 1);
                s += 2.0;
            }
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Placement, TRAINING_YARD};

    /// A garden fence (0.2 m thick, thinner than a cell) is a wall to the
    /// walkers at any angle: a path from one side to the other goes round
    /// its end, never through.
    #[test]
    fn a_thin_fence_is_walked_round_at_any_angle() {
        for rot in [0.0f32, 20.0, 45.0, 70.0, 90.0, 135.0] {
            let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
            let at = [-45.0, 30.0];
            map.placements.push(Placement {
                prefab: "fence_garden".into(),
                at,
                rot,
                y: 0.0,
                absolute: false,
                tag: String::new(),
            });
            let hm = Heightmap::build(&map);
            let g = zone_grid(&map, &hm, MAX_SLOPE);
            // Its face is local -Z: 2.5 m out either side of its middle.
            let r = rot.to_radians();
            let n = [-r.sin(), r.cos()];
            let a = [at[0] - n[0] * 2.5, at[1] - n[1] * 2.5];
            let b = [at[0] + n[0] * 2.5, at[1] + n[1] * 2.5];
            let path = g.path(a, b).expect("a way round");
            let len: f32 = path.windows(2).map(|w| crate::geom::dist(w[0], w[1])).sum();
            assert!(len > 8.0, "rot {rot}: {len:.1} m, straight through");
        }
    }
}
