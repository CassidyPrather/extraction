//! Ground navigation: a cost grid over the map and A* across it.
//!
//! Walking robots and simulation bots path on this. Cells are blocked by
//! steep ground, deep water, building footprints and tree trunks; roads are
//! cheap, mud and forest dear. Paths are smoothed by line-of-sight so they
//! don't zig-zag along the grid.

use crate::geom::P2;
use crate::terrain::Heightmap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Cost of a blocked cell.
pub const BLOCKED: u8 = 0;

#[derive(Clone, Debug)]
pub struct NavGrid {
    pub n: usize,
    pub cell: f32,
    pub origin: f32,
    /// 0 = blocked, otherwise relative cost to cross (1 = road).
    pub cost: Vec<u8>,
}

/// Something that blocks walking: a circle (trunks, posts) or an oriented
/// rectangle (buildings), in map space.
#[derive(Clone, Copy, Debug)]
pub enum Obstacle {
    Circle {
        at: P2,
        radius: f32,
    },
    /// Centre, half extents (x, z) and yaw in radians.
    Rect {
        at: P2,
        half: P2,
        yaw: f32,
    },
}

impl NavGrid {
    /// Builds the grid from terrain alone; add obstacles after.
    pub fn from_heightmap(hm: &Heightmap, cell: f32, max_slope_deg: f32) -> Self {
        let size = (hm.n - 1) as f32 * hm.cell;
        let n = (size / cell).floor() as usize;
        let mut g = Self {
            n,
            cell,
            origin: hm.origin,
            cost: vec![1; n * n],
        };
        for j in 0..n {
            for i in 0..n {
                let p = g.centre(i, j);
                let slope = hm.slope_deg(p);
                let k = j * n + i;
                if slope > max_slope_deg {
                    g.cost[k] = BLOCKED;
                    continue;
                }
                if let Some(level) = hm.water_level(p) {
                    // Wadeable shallows are slow; deep water is out.
                    g.cost[k] = if level - hm.height(p) > 0.9 {
                        BLOCKED
                    } else {
                        12
                    };
                    continue;
                }
                let road = hm.road_at(p);
                let mut c = 3.0;
                if road.weight > 100 {
                    c = 1.0;
                }
                c += slope / 10.0;
                c += hm.wetness(p) * 4.0;
                g.cost[k] = c.round().clamp(1.0, 250.0) as u8;
            }
        }
        g
    }

    pub fn centre(&self, i: usize, j: usize) -> P2 {
        [
            self.origin + (i as f32 + 0.5) * self.cell,
            self.origin + (j as f32 + 0.5) * self.cell,
        ]
    }

    pub fn cell_of(&self, p: P2) -> Option<(usize, usize)> {
        let i = ((p[0] - self.origin) / self.cell).floor();
        let j = ((p[1] - self.origin) / self.cell).floor();
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.n && (j as usize) < self.n)
            .then_some((i as usize, j as usize))
    }

    pub fn walkable(&self, p: P2) -> bool {
        self.cell_of(p)
            .is_some_and(|(i, j)| self.cost[j * self.n + i] != BLOCKED)
    }

    /// Multiplies the cost of cells inside a polygon (forest: slower going).
    pub fn scale_cost_in(&mut self, poly: &[P2], factor: f32) {
        let (lo, hi) = crate::geom::bounds(poly);
        let (Some((i0, j0)), Some((i1, j1))) = (
            self.cell_of(clamp_to(self, lo)),
            self.cell_of(clamp_to(self, hi)),
        ) else {
            return;
        };
        for j in j0..=j1 {
            for i in i0..=i1 {
                let k = j * self.n + i;
                if self.cost[k] != BLOCKED && crate::geom::point_in_polygon(poly, self.centre(i, j))
                {
                    self.cost[k] = (self.cost[k] as f32 * factor).round().clamp(1.0, 250.0) as u8;
                }
            }
        }
    }

    /// Marks an obstacle's cells blocked, grown by `clearance` (the walker's radius).
    pub fn block(&mut self, ob: Obstacle, clearance: f32) {
        let (centre, reach) = match ob {
            Obstacle::Circle { at, radius } => (at, radius + clearance),
            Obstacle::Rect { at, half, .. } => (at, half[0].hypot(half[1]) + clearance),
        };
        let lo = clamp_to(self, [centre[0] - reach, centre[1] - reach]);
        let hi = clamp_to(self, [centre[0] + reach, centre[1] + reach]);
        let (Some((i0, j0)), Some((i1, j1))) = (self.cell_of(lo), self.cell_of(hi)) else {
            return;
        };
        for j in j0..=j1 {
            for i in i0..=i1 {
                let p = self.centre(i, j);
                let inside = match ob {
                    Obstacle::Circle { at, radius } => {
                        crate::geom::dist(p, at) <= radius + clearance
                    }
                    Obstacle::Rect { at, half, yaw } => {
                        let d = crate::geom::sub(p, at);
                        let (s, c) = yaw.sin_cos();
                        let lx = d[0] * c + d[1] * s;
                        let lz = -d[0] * s + d[1] * c;
                        lx.abs() <= half[0] + clearance && lz.abs() <= half[1] + clearance
                    }
                };
                if inside {
                    self.cost[j * self.n + i] = BLOCKED;
                }
            }
        }
    }

    /// Opens a corridor (doorways, bridges) that something else blocked.
    pub fn open_line(&mut self, a: P2, b: P2, cost: u8) {
        let steps = (crate::geom::dist(a, b) / (self.cell * 0.5)).ceil() as usize + 1;
        let mut last: Option<(usize, usize)> = None;
        for s in 0..=steps {
            let p = crate::geom::lerp(a, b, s as f32 / steps as f32);
            if let Some((i, j)) = self.cell_of(p) {
                self.cost[j * self.n + i] = cost.max(1);
                // A diagonal step would be a corner the pathfinder won't
                // cut: open the cell beside it so the line stays walkable.
                if let Some((li, lj)) = last {
                    if li != i && lj != j {
                        self.cost[lj * self.n + i] = cost.max(1);
                    }
                }
                last = Some((i, j));
            }
        }
    }

    /// Nearest walkable cell centre to `p`, searching outward.
    pub fn nearest_walkable(&self, p: P2) -> Option<P2> {
        let (ci, cj) = self.cell_of(clamp_to(self, p))?;
        for r in 0..(self.n as i64) {
            let mut best: Option<(f32, P2)> = None;
            for dj in -r..=r {
                for di in -r..=r {
                    if di.abs() != r && dj.abs() != r {
                        continue;
                    }
                    let (i, j) = (ci as i64 + di, cj as i64 + dj);
                    if i < 0 || j < 0 || i >= self.n as i64 || j >= self.n as i64 {
                        continue;
                    }
                    if self.cost[j as usize * self.n + i as usize] != BLOCKED {
                        let c = self.centre(i as usize, j as usize);
                        let d = crate::geom::dist(c, p);
                        if best.is_none_or(|b| d < b.0) {
                            best = Some((d, c));
                        }
                    }
                }
            }
            if let Some((_, c)) = best {
                return Some(c);
            }
            if r > 64 {
                break;
            }
        }
        None
    }

    /// Whether a straight walk from `a` to `b` stays on walkable cells.
    pub fn line_clear(&self, a: P2, b: P2) -> bool {
        let steps = (crate::geom::dist(a, b) / (self.cell * 0.5)).ceil() as usize + 1;
        (0..=steps).all(|s| self.walkable(crate::geom::lerp(a, b, s as f32 / steps as f32)))
    }

    /// A* from `from` to `to`. Returns smoothed waypoints (including both
    /// ends, snapped to walkable ground) or `None` if unreachable.
    pub fn path(&self, from: P2, to: P2) -> Option<Vec<P2>> {
        let start_p = if self.walkable(from) {
            from
        } else {
            self.nearest_walkable(from)?
        };
        let goal_p = if self.walkable(to) {
            to
        } else {
            self.nearest_walkable(to)?
        };
        let (si, sj) = self.cell_of(start_p)?;
        let (gi, gj) = self.cell_of(goal_p)?;
        let n = self.n;
        let start = sj * n + si;
        let goal = gj * n + gi;
        let mut g = vec![f32::INFINITY; n * n];
        let mut came = vec![usize::MAX; n * n];
        let mut open = BinaryHeap::new();
        g[start] = 0.0;
        let h = |k: usize| {
            let (i, j) = (k % n, k / n);
            let dx = (i as f32 - gi as f32).abs();
            let dz = (j as f32 - gj as f32).abs();
            // Octile distance at the cheapest cost.
            (dx.max(dz) + (std::f32::consts::SQRT_2 - 1.0) * dx.min(dz)) * self.cell
        };
        open.push(Node {
            f: h(start),
            k: start,
        });
        while let Some(Node { k, .. }) = open.pop() {
            if k == goal {
                break;
            }
            let (i, j) = ((k % n) as i64, (k / n) as i64);
            for (di, dj) in [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (1, -1),
                (-1, 1),
                (1, 1),
            ] {
                let (ni, nj) = (i + di, j + dj);
                if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                    continue;
                }
                let nk = nj as usize * n + ni as usize;
                let c = self.cost[nk];
                if c == BLOCKED {
                    continue;
                }
                // No corner cutting past blocked cells.
                if di != 0 && dj != 0 {
                    let a = j as usize * n + ni as usize;
                    let b = nj as usize * n + i as usize;
                    if self.cost[a] == BLOCKED || self.cost[b] == BLOCKED {
                        continue;
                    }
                }
                let step = if di != 0 && dj != 0 {
                    std::f32::consts::SQRT_2
                } else {
                    1.0
                } * self.cell;
                let ng = g[k] + step * c as f32;
                if ng < g[nk] {
                    g[nk] = ng;
                    came[nk] = k;
                    open.push(Node {
                        f: ng + h(nk),
                        k: nk,
                    });
                }
            }
        }
        if came[goal] == usize::MAX && goal != start {
            return None;
        }
        let mut cells = vec![goal];
        let mut k = goal;
        while k != start {
            k = came[k];
            cells.push(k);
        }
        cells.reverse();
        let mut raw: Vec<P2> = cells.iter().map(|&k| self.centre(k % n, k / n)).collect();
        raw[0] = start_p;
        if let Some(last) = raw.last_mut() {
            *last = goal_p;
        }
        Some(self.smooth(&raw))
    }

    /// String-pulls a cell path: skip waypoints while the straight line
    /// stays walkable and doesn't cross notably dearer ground.
    fn smooth(&self, raw: &[P2]) -> Vec<P2> {
        if raw.len() <= 2 {
            return raw.to_vec();
        }
        let mut out = vec![raw[0]];
        let mut anchor = 0;
        let mut i = 2;
        while i < raw.len() {
            if !self.line_clear(raw[anchor], raw[i]) {
                out.push(raw[i - 1]);
                anchor = i - 1;
            }
            i += 1;
        }
        out.push(*raw.last().unwrap());
        out
    }
}

fn clamp_to(g: &NavGrid, p: P2) -> P2 {
    let lo = g.origin + 1e-3;
    let hi = g.origin + g.n as f32 * g.cell - 1e-3;
    [p[0].clamp(lo, hi), p[1].clamp(lo, hi)]
}

#[derive(PartialEq)]
struct Node {
    f: f32,
    k: usize,
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f
            .total_cmp(&self.f)
            .then_with(|| self.k.cmp(&other.k))
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MARROW_CREEK, MapDef};

    fn open_grid() -> NavGrid {
        NavGrid {
            n: 50,
            cell: 1.0,
            origin: -25.0,
            cost: vec![3; 2500],
        }
    }

    #[test]
    fn straight_line_on_open_ground() {
        let g = open_grid();
        let p = g.path([-20.0, 0.0], [20.0, 0.0]).unwrap();
        assert_eq!(p.len(), 2, "{p:?}");
    }

    #[test]
    fn goes_around_a_wall() {
        let mut g = open_grid();
        g.block(
            Obstacle::Rect {
                at: [0.0, 0.0],
                half: [1.0, 15.0],
                yaw: 0.0,
            },
            0.5,
        );
        assert!(!g.walkable([0.0, 0.0]));
        let p = g.path([-10.0, 0.0], [10.0, 0.0]).unwrap();
        assert!(p.len() > 2);
        for w in p.windows(2) {
            assert!(g.line_clear(w[0], w[1]), "segment {w:?} crosses the wall");
        }
    }

    #[test]
    fn unreachable_inside_a_closed_ring() {
        let mut g = open_grid();
        for k in 0..64 {
            let a = k as f32 / 64.0 * std::f32::consts::TAU;
            g.block(
                Obstacle::Circle {
                    at: [8.0 * a.cos(), 8.0 * a.sin()],
                    radius: 1.0,
                },
                0.0,
            );
        }
        assert!(g.path([-20.0, -20.0], [0.0, 0.0]).is_none());
    }

    #[test]
    fn marrow_creek_insertion_reaches_every_rendezvous() {
        let map = MapDef::from_ron(MARROW_CREEK).unwrap();
        let hm = Heightmap::build(&map);
        let g = NavGrid::from_heightmap(&hm, 2.0, 35.0);
        let ins = map
            .markers_of(&crate::map::MarkerKind::Insertion)
            .next()
            .unwrap();
        for rv in map.markers_of(&crate::map::MarkerKind::Rendezvous) {
            let p = g.path(ins.at, rv.at);
            assert!(p.is_some(), "no path from insertion to {}", rv.name);
        }
    }
}
