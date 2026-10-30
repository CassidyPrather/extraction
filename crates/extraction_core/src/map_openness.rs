//! How open a valley is, and how close together its places sit: the
//! playtest of 28 October (`docs/playtests/2026-10-28/TRANSCRIPT.md`, P1)
//! found Marrow Creek "a couple of points in an otherwise empty plane",
//! its places "too far apart", seen from too far, and a fight at one
//! drawing every machine (see `docs/MAP_DESIGN.md`, "Tight, broken,
//! quiet between").
//!
//! - [`openness`]: from every [`STEP`] of walkable ground inside the
//!   fence, how far a standing figure is seen along sixteen bearings,
//!   over the land and past what stands on it (trunks, rocks, buildings:
//!   `anomaly::sight_blockers`), out to [`SEEN_MAX`].
//! - [`walks`]: each point of interest's walk to its nearest neighbour.
//! - [`pull`]: for each hot zone (where the machines work, watch or come
//!   in), how many of the others would hear a fight there: a gunshot
//!   carries [`GUNSHOT`] (`robots::senses::GUNSHOT`), [`OCCLUDED`] as far
//!   past a hill or a wall standing [`HEARD_OVER`] over its line.

use crate::geom::{P2, dist};
use crate::map::{MapDef, MarkerKind};
use crate::map_lint::inside_fence;
use crate::nav::NavGrid;
use crate::terrain::Heightmap;

/// Spacing of the openness grid, metres.
pub const STEP: f32 = 10.0;
/// How far a sightline is followed, metres (beyond it, fog and range).
pub const SEEN_MAX: f32 = 150.0;
/// A standing figure's height, and the eye that looks for it (m).
pub const FIGURE: f32 = 1.75;
pub const EYE: f32 = 1.7;
/// Ground seen into this far along most bearings is open: the sight of
/// the longest-sighted machine on the ground (a drone's or a pylon's,
/// `robots::drone`, `robots::pylon`), metres.
pub const OPEN_REACH: f32 = 70.0;
/// A gunshot's reach to a machine's ears in the open, m: the one number
/// the game's `robots::senses` and this lint share (a 5.45's; a pistol
/// carries about 105 m, a Mosin 170)...
pub const GUNSHOT: f32 = 150.0;
/// ...and the share of it past a hill or a wall in the way
/// (`robots::senses::OCCLUDED`) standing this high over the line
/// (`robots::senses::HEARD_OVER`).
pub const OCCLUDED: f32 = 0.45;
pub const HEARD_OVER: f32 = 1.2;
/// A walking pace for walk times, m/s (`player::WALK_SPEED`).
pub const WALK: f32 = 2.4;

/// The valleys held to the rules below (by `MapDef::name`): Marrow
/// Creek, the prototype; the others join as they're reworked
/// (`docs/MAP_DESIGN.md`, "Tight, broken, quiet between").
pub const TIGHT: &[&str] = &["Marrow Creek"];
/// No more than this share of the open ground is seen past a walker's
/// sight along half its bearings ([`OPEN_REACH`])...
pub const OPEN_MAX: f32 = 0.16;
/// ...no quiet place (danger 1 or less) is heard by more than this many
/// hot spots...
pub const QUIET_PULL: usize = 2;
/// ...a fight at a hot spot is heard by this many others at most, on
/// average...
pub const PULL_MEAN: f32 = 1.5;
/// ...and no walk between neighbouring places runs this far with nothing
/// to stop at ([`STOP_NEAR`]), metres.
pub const EMPTY_MAX: f32 = 100.0;
/// Every hot region (danger 2 or more) caps how far its machines notice
/// you at this or less (`Region::notice`, metres; Robot minds' senses).
pub const NOTICE_MAX: f32 = 50.0;

/// How far into a wood (at a density of 1) a line of sight goes, metres:
/// one trunk hides little, a wood's depth hides everything.
pub const WOOD_DEPTH: f32 = 30.0;

/// How far, along a bearing from `p`, a standing figure stays in sight:
/// `solid` stops it, `wood` (a wood's density there) wears it down.
fn reach(
    hm: &Heightmap,
    solid: &dyn Fn(P2) -> bool,
    wood: &dyn Fn(P2) -> f32,
    p: P2,
    dir: P2,
) -> f32 {
    let eye = hm.height(p) + EYE;
    let mut steepest = f32::NEG_INFINITY;
    let mut seen = 0.0;
    let mut through = 0.0;
    let mut d = 2.0;
    while d <= SEEN_MAX {
        let q = [p[0] + dir[0] * d, p[1] + dir[1] * d];
        through += 2.0 * wood(q);
        if solid(q) || through > WOOD_DEPTH {
            break;
        }
        let ground = (hm.height(q) - eye) / d;
        let figure = (hm.height(q) + FIGURE - eye) / d;
        if figure >= steepest {
            seen = d;
        }
        steepest = steepest.max(ground);
        d += 2.0;
    }
    seen
}

/// The openness of a valley.
#[derive(Clone, Debug, Default)]
pub struct Openness {
    /// Points sampled.
    pub points: usize,
    /// Median and 90th percentile of each point's median reach (m).
    pub median: f32,
    pub p90: f32,
    /// Share of points seen into past [`OPEN_REACH`] along at least half
    /// their bearings.
    pub open_share: f32,
}

pub fn openness(map: &MapDef, hm: &Heightmap, walk: &NavGrid) -> Openness {
    // What stands in the way, rasterised at 2 m.
    let cell = 2.0f32;
    let half = map.size * 0.5;
    let n = (map.size / cell).ceil() as usize + 1;
    let mut block = vec![false; n * n];
    // Buildings and rocks block; the woods are their depth.
    let rocks = crate::rocks::scatter_rocks(map, hm).into_iter().map(|r| {
        let long = r.radius * r.stretch.max(1.0 / r.stretch);
        ([r.pos[0], r.pos[2]], long + 0.3)
    });
    let built = crate::map_lint::solids(map)
        .into_iter()
        .filter(|s| crate::map::prefab_footprint(&s.prefab)[1] >= FIGURE || s.kiosk())
        .map(|s| (s.rect.at, s.rect.half[0].min(s.rect.half[1]).max(0.5)));
    for (at, r) in rocks.chain(built) {
        let (i0, j0) = (
            ((at[0] - r + half) / cell).floor(),
            ((at[1] - r + half) / cell).floor(),
        );
        let (i1, j1) = (
            ((at[0] + r + half) / cell).ceil(),
            ((at[1] + r + half) / cell).ceil(),
        );
        for j in j0.max(0.0) as usize..=(j1 as usize).min(n - 1) {
            for i in i0.max(0.0) as usize..=(i1 as usize).min(n - 1) {
                let c = [i as f32 * cell - half, j as f32 * cell - half];
                if dist(c, at) <= r {
                    block[j * n + i] = true;
                }
            }
        }
    }
    let solid = |q: P2| {
        let (i, j) = (
            ((q[0] + half) / cell).round(),
            ((q[1] + half) / cell).round(),
        );
        i >= 0.0
            && j >= 0.0
            && (i as usize) < n
            && (j as usize) < n
            && block[j as usize * n + i as usize]
    };
    let mut woods = vec![0.0f32; n * n];
    for f in &map.forests {
        let (lo, hi) = crate::geom::bounds(&f.polygon);
        for j in (((lo[1] + half) / cell).floor().max(0.0) as usize)
            ..=(((hi[1] + half) / cell).ceil() as usize).min(n - 1)
        {
            for i in (((lo[0] + half) / cell).floor().max(0.0) as usize)
                ..=(((hi[0] + half) / cell).ceil() as usize).min(n - 1)
            {
                let c = [i as f32 * cell - half, j as f32 * cell - half];
                if crate::geom::point_in_polygon(&f.polygon, c) {
                    woods[j * n + i] = woods[j * n + i].max(f.density);
                }
            }
        }
    }
    let wood = |q: P2| {
        let (i, j) = (
            ((q[0] + half) / cell).round(),
            ((q[1] + half) / cell).round(),
        );
        if i >= 0.0 && j >= 0.0 && (i as usize) < n && (j as usize) < n {
            woods[j as usize * n + i as usize]
        } else {
            0.0
        }
    };
    let dirs: Vec<P2> = (0..16)
        .map(|k| {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            [a.cos(), a.sin()]
        })
        .collect();
    let mut medians = Vec::new();
    let mut open = 0usize;
    let mut z = -half;
    while z <= half {
        let mut x = -half;
        while x <= half {
            let p = [x, z];
            if inside_fence(map, p) && walk.walkable(p) && !solid(p) && wood(p) == 0.0 {
                let mut r: Vec<f32> = dirs
                    .iter()
                    .map(|d| reach(hm, &solid, &wood, p, *d))
                    .collect();
                r.sort_by(f32::total_cmp);
                medians.push(r[r.len() / 2]);
                if r[r.len() / 2] >= OPEN_REACH {
                    open += 1;
                    if std::env::var_os("OPEN_DEBUG").is_some() {
                        eprintln!("OPEN {} {} {:.0}", p[0], p[1], r[r.len() / 2]);
                    }
                }
            }
            x += STEP;
        }
        z += STEP;
    }
    medians.sort_by(f32::total_cmp);
    let at = |q: f32| {
        medians
            .get(((medians.len() as f32 - 1.0) * q) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    Openness {
        points: medians.len(),
        median: at(0.5),
        p90: at(0.9),
        open_share: open as f32 / medians.len().max(1) as f32,
    }
}

/// Each point of interest's walk on comfortable ground to its nearest
/// neighbour: (it, the neighbour, metres).
pub fn walks(map: &MapDef, comfy: &NavGrid) -> Vec<(String, String, f32)> {
    let pois: Vec<(&str, P2)> = map
        .markers_of(&MarkerKind::Poi)
        .map(|m| (m.name.as_str(), m.at))
        .collect();
    let mut out = Vec::new();
    for (a, pa) in &pois {
        let best = pois
            .iter()
            .filter(|(b, _)| b != a)
            .filter_map(|(b, pb)| {
                let path = comfy.path(*pa, *pb)?;
                let len: f32 = path.windows(2).map(|w| dist(w[0], w[1])).sum();
                Some((b.to_string(), len))
            })
            .min_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((b, len)) = best {
            out.push((a.to_string(), b, len));
        }
    }
    out
}

/// Somewhere a walk stops: a roof to go into, a kiosk, loot.
pub fn stops(map: &MapDef) -> Vec<P2> {
    crate::map_lint::solids(map)
        .into_iter()
        .filter(|s| s.kiosk() || crate::map_surge::roofed(&s.prefab))
        .map(|s| s.rect.at)
        .chain(
            map.markers
                .iter()
                .filter(|m| matches!(m.kind, MarkerKind::Loot { .. }))
                .map(|m| m.at),
        )
        .collect()
}

/// A stop counts along a walk within this, metres.
pub const STOP_NEAR: f32 = 40.0;

/// For each point of interest's walk to its nearest neighbour, the
/// longest stretch with nothing to stop at within [`STOP_NEAR`]: the
/// tester's "too far apart" is empty walking, not distance.
pub fn empty_walks(map: &MapDef, comfy: &NavGrid) -> Vec<(String, String, f32, P2)> {
    let stops = stops(map);
    let pois: Vec<(&str, P2)> = map
        .markers_of(&MarkerKind::Poi)
        .map(|m| (m.name.as_str(), m.at))
        .collect();
    let mut out = Vec::new();
    for (a, b, _) in walks(map, comfy) {
        let (Some(pa), Some(pb)) = (
            pois.iter().find(|p| p.0 == a).map(|p| p.1),
            pois.iter().find(|p| p.0 == b).map(|p| p.1),
        ) else {
            continue;
        };
        let Some(path) = comfy.path(pa, pb) else {
            continue;
        };
        let pts = crate::map_lint::resample(&path);
        let (mut run, mut best, mut at) = (0.0f32, 0.0f32, pa);
        for (i, p) in pts.iter().enumerate() {
            let near = stops.iter().any(|s| dist(*s, *p) <= STOP_NEAR);
            if near {
                run = 0.0;
            } else if i > 0 {
                run += dist(pts[i - 1], *p);
                if run > best {
                    (best, at) = (run, *p);
                }
            }
        }
        out.push((a, b, best, at));
    }
    out
}

/// Where the machines are: their work sites, sentries, spotters and gate.
pub fn hot_spots(map: &MapDef) -> Vec<(String, P2)> {
    map.markers
        .iter()
        .filter_map(|m| {
            let what = match &m.kind {
                MarkerKind::WorkSite => "work site",
                MarkerKind::Sentry { .. } => "sentry",
                MarkerKind::Spotter => "spotter",
                MarkerKind::RobotGate => "gate",
                _ => return None,
            };
            let name = if m.name.is_empty() {
                what.to_string()
            } else {
                format!("{what} {}", m.name)
            };
            Some((name, m.at))
        })
        .collect()
}

/// Whether a fight at `a` is heard at `b` (the land's rises in between
/// cut its reach to [`OCCLUDED`]).
pub fn heard(hm: &Heightmap, a: P2, b: P2) -> bool {
    let d = dist(a, b);
    if d > GUNSHOT {
        return false;
    }
    let (ha, hb) = (hm.height(a) + EYE, hm.height(b) + EYE);
    let n = (d / 4.0).ceil().max(1.0) as usize;
    let blocked = (1..n).any(|k| {
        let t = k as f32 / n as f32;
        let q = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        hm.height(q) > ha + (hb - ha) * t + HEARD_OVER
    });
    d <= if blocked { GUNSHOT * OCCLUDED } else { GUNSHOT }
}

/// Hot spots within this of one another are one hot zone, metres: a
/// zone hearing its own fight isn't the valley coming to it.
pub const ZONE: f32 = 60.0;

/// Each hot spot's zone: an index shared by spots within [`ZONE`] of one
/// another (chained).
fn zones(spots: &[(String, P2)]) -> Vec<usize> {
    let mut zone: Vec<usize> = (0..spots.len()).collect();
    fn root(z: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while z[i] != i {
            z[i] = z[z[i]];
            i = z[i];
        }
        i
    }
    for i in 0..spots.len() {
        for j in i + 1..spots.len() {
            if dist(spots[i].1, spots[j].1) <= ZONE {
                let (a, b) = (root(&mut zone, i), root(&mut zone, j));
                zone[a] = b;
            }
        }
    }
    (0..spots.len()).map(|i| root(&mut zone, i)).collect()
}

/// For each point of interest, how many hot zones would hear a fight
/// there (the tester's "the entirety of the robots to your front door").
pub fn pull_at_places(map: &MapDef, hm: &Heightmap) -> Vec<(String, P2, usize)> {
    let spots = hot_spots(map);
    let zone = zones(&spots);
    map.markers_of(&MarkerKind::Poi)
        .map(|m| {
            let mut heard_by: Vec<usize> = spots
                .iter()
                .enumerate()
                .filter(|(_, (_, b))| heard(hm, m.at, *b))
                .map(|(i, _)| zone[i])
                .collect();
            heard_by.sort_unstable();
            heard_by.dedup();
            (m.name.clone(), m.at, heard_by.len())
        })
        .collect()
}

/// For each hot spot, how many other hot zones would hear a fight there.
pub fn pull(map: &MapDef, hm: &Heightmap) -> Vec<(String, P2, usize)> {
    let spots = hot_spots(map);
    let zone = zones(&spots);
    spots
        .iter()
        .enumerate()
        .map(|(i, (name, at))| {
            let mut heard_by: Vec<usize> = spots
                .iter()
                .enumerate()
                .filter(|(j, (_, b))| zone[*j] != zone[i] && heard(hm, *at, *b))
                .map(|(j, _)| zone[j])
                .collect();
            heard_by.sort_unstable();
            heard_by.dedup();
            (name.clone(), *at, heard_by.len())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each valley's openness, walks and pull, printed.
    #[test]
    #[ignore]
    fn print_openness() {
        for (id, text) in crate::map::VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let walk = crate::map_nav::zone_grid(&map, &hm, crate::map_nav::MAX_SLOPE);
            let comfy = crate::map_nav::zone_grid(&map, &hm, crate::map_lint::COMFORT_SLOPE);
            let o = openness(&map, &hm, &walk);
            let w = walks(&map, &comfy);
            let mean = w.iter().map(|x| x.2).sum::<f32>() / w.len().max(1) as f32;
            let worst = w.iter().max_by(|a, b| a.2.total_cmp(&b.2));
            let p = pull(&map, &hm);
            let pmax = p.iter().map(|x| x.2).max().unwrap_or(0);
            let pmean = p.iter().map(|x| x.2).sum::<usize>() as f32 / p.len().max(1) as f32;
            eprintln!(
                "{id}: seen median {:.0} m, p90 {:.0} m, open {:.0}%; walk to the nearest place mean {:.0} m ({:.0} s), longest {:?}; pull mean {pmean:.1}, most {pmax} of {}",
                o.median,
                o.p90,
                o.open_share * 100.0,
                mean,
                mean / WALK,
                worst,
                p.len()
            );
            let e = empty_walks(&map, &comfy);
            let emax = e.iter().max_by(|a, b| a.2.total_cmp(&b.2));
            eprintln!("   longest empty walk: {emax:?}");
            if std::env::var("EMPTY").is_ok() {
                for x in &e {
                    eprintln!(
                        "     {} -> {}: {:.0} m empty, worst at ({:.0}, {:.0})",
                        x.0, x.1, x.2, x.3[0], x.3[1]
                    );
                }
            }
            let q = pull_at_places(&map, &hm);
            eprintln!(
                "   places heard by: {}",
                q.iter()
                    .map(|(n, _, k)| format!("{n} {k}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if let Ok(place) = std::env::var("WHO_HEARS") {
                if let Some(m) = map.markers_of(&MarkerKind::Poi).find(|m| m.name == place) {
                    for (n, at) in hot_spots(&map) {
                        if heard(&hm, m.at, at) {
                            eprintln!("   {place} heard at {n} ({:.0} m)", dist(m.at, at));
                        }
                    }
                }
            }
            if std::env::var("PULL").is_ok() {
                for (n, at, k) in &p {
                    eprintln!("   {n} ({:.0}, {:.0}): {k}", at[0], at[1]);
                }
            }
        }
    }
}

#[cfg(test)]
mod probe_file {
    use super::*;
    /// The measures and the lint of a map read from MAP_FILE.
    #[test]
    #[ignore]
    fn probe_file() {
        let text = std::fs::read_to_string(std::env::var("MAP_FILE").unwrap()).unwrap();
        let map = MapDef::from_ron(&text).unwrap();
        let hm = Heightmap::build(&map);
        let walk = crate::map_nav::zone_grid(&map, &hm, crate::map_nav::MAX_SLOPE);
        let comfy = crate::map_nav::zone_grid(&map, &hm, crate::map_lint::COMFORT_SLOPE);
        let o = openness(&map, &hm, &walk);
        let w = walks(&map, &comfy);
        let mean = w.iter().map(|x| x.2).sum::<f32>() / w.len().max(1) as f32;
        let p = pull(&map, &hm);
        let pmean = p.iter().map(|x| x.2).sum::<usize>() as f32 / p.len().max(1) as f32;
        eprintln!(
            "FILE: seen p90 {:.0} m, open {:.0}%; nearest-place walk mean {mean:.0} m ({:.0} s); pull {pmean:.1}",
            o.p90,
            o.open_share * 100.0,
            mean / WALK
        );
        for (a, b, l) in &w {
            eprintln!("  walk {a} -> {b}: {l:.0} m");
        }
        eprintln!(
            "  places heard by: {}",
            pull_at_places(&map, &hm)
                .iter()
                .map(|(n, _, k)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        if let Ok(place) = std::env::var("WHO_HEARS") {
            if let Some(m) = map.markers_of(&MarkerKind::Poi).find(|m| m.name == place) {
                for (n, at) in hot_spots(&map) {
                    if heard(&hm, m.at, at) {
                        eprintln!("  {place} heard at {n} ({:.0} m)", dist(m.at, at));
                    }
                }
            }
        }
        for (n, at, k) in &p {
            eprintln!("  pull {n} ({:.0}, {:.0}): {k}", at[0], at[1]);
        }
        let r = crate::map_lint::lint(&map, &hm);
        for f in &r.findings {
            eprintln!(
                "  FINDING {:?}: {} at ({:.0}, {:.0})",
                f.rule, f.what, f.at[0], f.at[1]
            );
        }
    }
}

#[cfg(test)]
mod probe_grid {
    use super::*;
    #[test]
    #[ignore]
    fn probe_grid() {
        let text = std::fs::read_to_string(std::env::var("MAP_FILE").unwrap()).unwrap();
        let map = MapDef::from_ron(&text).unwrap();
        let hm = Heightmap::build(&map);
        let v: Vec<f32> = std::env::var("GRID").unwrap().split(',').map(|s| s.parse().unwrap()).collect();
        let (x0, x1, z0, z1, st) = (v[0], v[1], v[2], v[3], v[4]);
        let mut z = z0;
        while z <= z1 {
            let mut row = format!("{z:5.0}:");
            let mut x = x0;
            while x <= x1 {
                row += &format!(" {:5.1}", hm.height([x, z]));
                x += st;
            }
            eprintln!("{row}");
            z += st;
        }
    }
}
