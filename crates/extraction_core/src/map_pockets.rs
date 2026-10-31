//! Walkable pockets: ground the walkers' grid calls reachable that a body
//! can't walk into, or out of. The grid judges each 2 m cell by the slope
//! at its middle, so a steep band between two cells' middles (a road
//! cut's bank, a plateau's or a pad's rim) is invisible to it: it joins
//! the cells, and a bot or a walker that plans across meets a bank it
//! can't climb (`Rule::Pocket`, see `docs/MAP_DESIGN.md`).
//!
//! Every step between two walkable neighbours is walked at
//! [`SAMPLE`] intervals; one that crosses ground steeper than the grid's
//! own limit is a false step. Flooding from the insertion with and
//! without false steps, the ground reached only by them is a pocket.

use crate::geom::{self, P2};
use crate::nav::{BLOCKED, NavGrid};
use crate::terrain::Heightmap;

/// How finely a step between two cells' middles is walked, metres.
pub const SAMPLE: f32 = 0.25;
/// A pocket at least this big (m²) is a finding; smaller is a sliver.
pub const POCKET_MIN: f32 = 16.0;

const NEIGHBOURS: [(i64, i64); 8] = [
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
    (1, 1),
];

/// The steepest ground the body stands on (the game's `player::walk_over`
/// takes a ground normal over 0.65 up: 49.5 degrees), and the step it
/// climbs onto anything steeper (`player::STEP_HEIGHT`).
pub const BODY_SLOPE: f32 = 49.5;
pub const BODY_STEP: f32 = 0.4;

/// Whether the step from cell `a` to its neighbour `b` is one the body
/// can't take: a stretch steeper than [`BODY_SLOPE`] climbing (or
/// dropping) more than [`BODY_STEP`] in all, or water too deep to wade.
/// (`max_slope`, the grid's own limit, is kept for the measure.)
pub fn false_step(
    g: &NavGrid,
    hm: &Heightmap,
    a: (usize, usize),
    b: (usize, usize),
    _max_slope: f32,
) -> bool {
    let (pa, pb) = (g.centre(a.0, a.1), g.centre(b.0, b.1));
    let len = geom::dist(pa, pb);
    let n = (len / SAMPLE).ceil().max(1.0) as usize;
    let run = len / n as f32;
    let steep = BODY_SLOPE.to_radians().tan() * run;
    let mut climb = 0.0f32;
    // Where you walk: a bridge's deck over the water, else the ground.
    let on_deck = |p: P2| (crate::map_lint::walk_height(hm, p) - hm.height(p)).abs() > 0.05;
    let mut prev = crate::map_lint::walk_height(hm, pa);
    for k in 1..=n {
        let p = geom::lerp(pa, pb, k as f32 / n as f32);
        if !on_deck(p) && hm.water_level(p).is_some_and(|w| w - hm.height(p) > 0.9) {
            return true;
        }
        let h = crate::map_lint::walk_height(hm, p);
        let dh = h - prev;
        prev = h;
        if dh.abs() > steep {
            // Steeper than it stands on: a step, carried on while it lasts
            // (the same way: a lip, not a ripple).
            climb = if climb * dh > 0.0 { climb + dh } else { dh };
            if climb.abs() > BODY_STEP {
                return true;
            }
        } else {
            climb = 0.0;
        }
    }
    false
}

/// Cells reached from `from`, stepping as the pathfinder does; with
/// `strict`, never over a false step.
fn flood(g: &NavGrid, hm: &Heightmap, from: P2, max_slope: f32, strict: bool) -> Vec<bool> {
    let n = g.n;
    let mut seen = vec![false; n * n];
    let start = if g.walkable(from) {
        Some(from)
    } else {
        g.nearest_walkable(from)
    };
    let Some((i0, j0)) = start.and_then(|p| g.cell_of(p)) else {
        return seen;
    };
    seen[j0 * n + i0] = true;
    let mut stack = vec![(i0, j0)];
    while let Some((i, j)) = stack.pop() {
        for (di, dj) in NEIGHBOURS {
            let (ni, nj) = (i as i64 + di, j as i64 + dj);
            if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                continue;
            }
            let (ni, nj) = (ni as usize, nj as usize);
            let k = nj * n + ni;
            if seen[k] || g.cost[k] == BLOCKED {
                continue;
            }
            if di != 0 && dj != 0 {
                let a = j * n + ni;
                let b = nj * n + i;
                if g.cost[a] == BLOCKED || g.cost[b] == BLOCKED {
                    continue;
                }
            }
            if strict && false_step(g, hm, (i, j), (ni, nj), max_slope) {
                continue;
            }
            seen[k] = true;
            stack.push((ni, nj));
        }
    }
    seen
}

/// What the pocket check found.
#[derive(Default)]
pub struct Pockets {
    /// Each pocket: its size (m²) and a cell in it.
    pub pockets: Vec<(f32, P2)>,
    /// Steps between walkable cells the grid takes and a body can't.
    pub false_steps: usize,
}

/// The pockets of `g` (built with `max_slope`) as seen from `from`, on
/// ground `inside` says is in play (inside the quarantine fence: the grid
/// doesn't know the fence, the game's collider does).
pub fn pockets(
    g: &NavGrid,
    hm: &Heightmap,
    from: P2,
    max_slope: f32,
    inside: impl Fn(P2) -> bool,
) -> Pockets {
    let loose = flood(g, hm, from, max_slope, false);
    let strict = flood(g, hm, from, max_slope, true);
    let n = g.n;
    let mut out = Pockets::default();
    for j in 0..n {
        for i in 0..n {
            if g.cost[j * n + i] == BLOCKED {
                continue;
            }
            for (di, dj) in NEIGHBOURS[..4].iter().copied() {
                let (ni, nj) = (i as i64 + di, j as i64 + dj);
                if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 || (di, dj) < (0, 0) {
                    continue;
                }
                let (ni, nj) = (ni as usize, nj as usize);
                if g.cost[nj * n + ni] != BLOCKED && false_step(g, hm, (i, j), (ni, nj), max_slope)
                {
                    out.false_steps += 1;
                }
            }
        }
    }
    // Group the cells reached only over false steps into pockets.
    let mut done = vec![false; n * n];
    for j in 0..n {
        for i in 0..n {
            let k = j * n + i;
            if !loose[k] || strict[k] || done[k] || !inside(g.centre(i, j)) {
                continue;
            }
            let mut cells = 0usize;
            let mut stack = vec![(i, j)];
            done[k] = true;
            while let Some((ci, cj)) = stack.pop() {
                cells += 1;
                for (di, dj) in NEIGHBOURS[..4].iter().copied() {
                    let (ni, nj) = (ci as i64 + di, cj as i64 + dj);
                    if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                        continue;
                    }
                    let nk = nj as usize * n + ni as usize;
                    if loose[nk]
                        && !strict[nk]
                        && !done[nk]
                        && inside(g.centre(ni as usize, nj as usize))
                    {
                        done[nk] = true;
                        stack.push((ni as usize, nj as usize));
                    }
                }
            }
            out.pockets
                .push((cells as f32 * g.cell * g.cell, g.centre(i, j)));
        }
    }
    out.pockets.sort_by(|a, b| b.0.total_cmp(&a.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{CINDER_WORKS, MapDef, MarkerKind};

    fn built(text: &str) -> (MapDef, Heightmap, NavGrid) {
        let map = MapDef::from_ron(text).unwrap();
        let hm = Heightmap::build(&map);
        let g = crate::map_nav::zone_grid(&map, &hm, crate::map_nav::MAX_SLOPE);
        (map, hm, g)
    }

    /// Over Cinder's canal by the works bridge, the step is taken on the
    /// deck, not the water under it.
    #[test]
    fn a_step_on_a_bridge_deck_is_walkable() {
        let (_, hm, g) = built(CINDER_WORKS);
        let (a, b) = (
            g.cell_of([21.0, 61.0]).unwrap(),
            g.cell_of([21.0, 59.0]).unwrap(),
        );
        assert!(!false_step(&g, &hm, a, b, crate::map_nav::MAX_SLOPE));
    }

    /// Every bridge is got onto at both ends: its deck ends on its line
    /// with line beyond to come from, the ground under its last metres is
    /// its abutment (no gap a body stands under), and a body coming along
    /// the line, or from beside the deck's end round onto the line, meets
    /// no step or slope it can't take, up to the deck and onto it. Cinder's
    /// lock path started 4 m out along its deck (the pull-in shortened the
    /// path, not the deck): no abutment, a 0.5 m step up out of the cut
    /// under it, and the smoke's raiders stuck at both ends.
    #[test]
    fn every_bridge_is_got_onto_at_both_ends() {
        // Line beyond a deck's end to come from, and how far beside it a
        // body comes round from.
        const APPROACH: f32 = 3.0;
        const BESIDE: f32 = 2.0;
        // The abutment: this far in from the deck's end (`Heightmap::road`
        // builds it to `terrain::ABUTMENT`, read here short of its edge).
        const ABUTMENT: f32 = 1.5;
        let yard = [("training_yard", crate::map::TRAINING_YARD)];
        let mut bad = Vec::new();
        for (name, text) in crate::map::VALLEYS.into_iter().chain(yard) {
            let (map, hm, _) = built(text);
            for rl in hm.road_lines.iter().filter(|rl| !rl.bridges.is_empty()) {
                let road = &map.roads[rl.road].name;
                let mut arc = vec![0.0f32];
                for w in rl.points.windows(2) {
                    arc.push(arc.last().unwrap() + geom::dist(w[0], w[1]));
                }
                let len = *arc.last().unwrap();
                let deck = |s: f32| {
                    let k = arc.partition_point(|a| *a < s).clamp(1, arc.len() - 1);
                    let t = ((s - arc[k - 1]) / (arc[k] - arc[k - 1]).max(1e-3)).clamp(0.0, 1.0);
                    rl.heights[k - 1] + (rl.heights[k] - rl.heights[k - 1]) * t
                };
                let at = |s: f32| geom::polyline_at(&rl.points, s);
                for &(a, b) in &rl.bridges {
                    for (end, out) in [(a, -1.0f32), (b, 1.0)] {
                        let tag = format!(
                            "{name} {road}: deck end at ({:.1}, {:.1})",
                            at(end.clamp(0.0, len)).0[0],
                            at(end.clamp(0.0, len)).0[1]
                        );
                        let from = end + out * APPROACH;
                        if from < 0.0 || from > len {
                            bad.push(format!(
                                "{tag}: {:.1} m of line beyond it",
                                (len - end).min(end).max(-end)
                            ));
                            continue;
                        }
                        // Under its last metres, the abutment.
                        let mut s = end - out * ABUTMENT;
                        while (s - end) * -out >= 0.0 {
                            let gap = deck(s) - hm.height(at(s).0);
                            if gap > BODY_STEP {
                                bad.push(format!(
                                    "{tag}: {gap:.2} m under the deck {:.1} m in",
                                    (s - end).abs()
                                ));
                                break;
                            }
                            s += out * 0.5;
                        }
                        // Walked in: along the line from beyond the end,
                        // and from either side of it round onto the line,
                        // then onto the deck.
                        let (p, dir) = at(from);
                        let side = [-dir[1], dir[0]];
                        let reach = rl.width * 0.5 + BESIDE;
                        let onto = at(end - out * 1.0).0;
                        // Where the body stands: the deck over its span,
                        // the ground elsewhere.
                        let ground = |q: P2| {
                            let (_, s, _) = geom::polyline_closest(&rl.points, q);
                            let under = (s - a) * (b - s) >= 0.0
                                && geom::dist(q, at(s).0) <= rl.width * 0.5;
                            if under { deck(s) } else { hm.height(q) }
                        };
                        // A climb the body can't take: a step over its
                        // step height, or ground steeper than it stands
                        // on for more than one sample.
                        let steep = SAMPLE * BODY_SLOPE.to_radians().tan();
                        for (k, from_where) in [
                            (0.0f32, "the line"),
                            (-1.0, "one side"),
                            (1.0, "the other side"),
                        ] {
                            let start = [p[0] + side[0] * reach * k, p[1] + side[1] * reach * k];
                            let mut was_steep = false;
                            'walk: for (u, v) in [(start, p), (p, onto)] {
                                let n = (geom::dist(u, v) / SAMPLE).ceil().max(1.0) as usize;
                                for i in 0..n {
                                    let q0 = geom::lerp(u, v, i as f32 / n as f32);
                                    let q1 = geom::lerp(u, v, (i + 1) as f32 / n as f32);
                                    let rise = ground(q1) - ground(q0);
                                    let is_steep = rise > steep;
                                    if rise > BODY_STEP || (is_steep && was_steep) {
                                        bad.push(format!(
                                            "{tag}: from {from_where}, a {rise:.2} m rise at ({:.1}, {:.1})",
                                            q1[0], q1[1]
                                        ));
                                        break 'walk;
                                    }
                                    was_steep = is_steep;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(bad.is_empty(), "bridges not got onto:\n{}", bad.join("\n"));
    }

    /// A bridge is walked at its deck: Cinder's canal bridges are the only
    /// ways north, and read off the water under them, the whole north of
    /// the valley came out a pocket.
    #[test]
    fn bridges_are_walked_at_the_deck() {
        let (map, hm, g) = built(CINDER_WORKS);
        let ins = map.markers_of(&MarkerKind::Insertion).next().unwrap().at;
        let p = pockets(&g, &hm, ins, crate::map_nav::MAX_SLOPE, |_| true);
        let big: f32 = p.pockets.iter().map(|q| q.0).sum();
        assert!(
            big < POCKET_MIN,
            "{:?}",
            &p.pockets[..p.pockets.len().min(4)]
        );
    }
}

#[cfg(test)]
mod probe {
    use super::*;
    #[test]
    #[ignore]
    fn probe_pockets() {
        let name = std::env::var("PROBE_MAP").unwrap();
        let text = crate::map::VALLEYS.iter().find(|v| v.0 == name).unwrap().1;
        let map = crate::map::MapDef::from_ron(text).unwrap();
        let hm = Heightmap::build(&map);
        let g = crate::map_nav::zone_grid(&map, &hm, crate::map_nav::MAX_SLOPE);
        let ins = map
            .markers_of(&crate::map::MarkerKind::Insertion)
            .next()
            .unwrap()
            .at;
        let strict = flood(&g, &hm, ins, crate::map_nav::MAX_SLOPE, true);
        let loose = flood(&g, &hm, ins, crate::map_nav::MAX_SLOPE, false);
        eprintln!(
            "strict {} loose {}",
            strict.iter().filter(|b| **b).count(),
            loose.iter().filter(|b| **b).count()
        );
        let n = g.n;
        for j in 0..n {
            for i in 0..n {
                if !strict[j * n + i] {
                    continue;
                }
                for (di, dj) in NEIGHBOURS {
                    let (ni, nj) = (i as i64 + di, j as i64 + dj);
                    if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                        continue;
                    }
                    let (ni, nj) = (ni as usize, nj as usize);
                    if !strict[nj * n + ni] && loose[nj * n + ni] {
                        let a = g.centre(i, j);
                        let b = g.centre(ni, nj);
                        let worst = (1..8)
                            .map(|k| hm.slope_deg(geom::lerp(a, b, k as f32 / 8.0)))
                            .fold(0.0f32, f32::max);
                        eprintln!("edge {:?} -> {:?} worst slope {worst:.0}", a, b);
                    }
                }
            }
        }
    }
}
