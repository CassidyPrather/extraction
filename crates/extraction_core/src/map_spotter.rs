//! Spotters in the valleys: where a map may fly one (`MarkerKind::Spotter`,
//! a winch at `marker.at`; `rot` is unused) so that it makes you use the
//! ground's cover rather than ending the raid. The machine's own rules are
//! [`crate::spotter`]; this is the lint's side (`Rule::Spotter`, see
//! `docs/MAP_DESIGN.md`, "Spotters").
//!
//! A spotter is an alarm, not a gun: it sees far ([`spotter::SIGHT`]) from
//! high up ([`spotter::TETHER`]), turns a slow circle, takes
//! [`spotter::FIX`] seconds to fix you and then calls a drone onto where
//! you stood. It earns its place when:
//!
//! - **its winch stands on good ground**: walkable, dry, no steeper than
//!   `map_lint::COMFORT_SLOPE`, inside the fence: cutting the tether is
//!   the answer, so the winch must be somewhere you can stand;
//! - **it watches something**: a key, a point of interest, a work site,
//!   loot or an underground site within its sight;
//! - **there is a way in under it**: from the insertion, a walk reaches
//!   within [`WAY_IN`] of the winch, through the blind ring under its
//!   level eye ([`spotter::blind_radius`]), crossing no more of the ground
//!   it sees than you can walk while its eye is turned away
//!   ([`WAY_SEEN`]). (A sentry wants a walk it never sees; a spotter turns
//!   its back on you most of the time, and in open ground, which is where
//!   one belongs, there is no such walk: timing its turn is the way in.);
//! - **it never watches the way in or out**: not the insertion, not a
//!   rendezvous, not the truck's tailgate or the approach to it;
//! - **it doesn't stack on a sentry**: the ground it sees covers no more
//!   than [`STACK_SHARE`] of any sentry's field. A spotter calling a drone
//!   onto a turret's ground is the deadly pairing the sims found in
//!   Marrow: deaths cluster where two machines cover one spot.
//!
//! Line of sight runs from the envelope (the winch's ground plus
//! [`spotter::TETHER`]) to a standing body ([`map_sentry::BODY`]) through
//! the land, the buildings and the kiosks (`map_sentry`'s blocks). Trees
//! don't count, so its field is an upper bound: woods are cover the lint
//! doesn't credit.

use crate::geom::{P2, dist};
use crate::map::{MapDef, MarkerKind};
use crate::map_sentry::{self, BODY, Block, Checked, Field};
use crate::nav::{BLOCKED, NavGrid};
use crate::spotter;
use crate::terrain::Heightmap;

/// The way in from the insertion gets this close to the winch.
pub const WAY_IN: f32 = 3.0;
/// The most ground it sees that the way in may cross, m: a walk
/// (`map_surge::WALK`) for as long as its eye is turned from any one way,
/// [`spotter::PAN_PERIOD`] less the share its [`spotter::FOV`] covers
/// (about 69 m: more than its whole ring on flat ground, so it bites
/// where the ground round the winch rises and there's no blind ring).
pub const WAY_SEEN: f32 =
    crate::map_surge::WALK * spotter::PAN_PERIOD * (1.0 - spotter::FOV / 360.0);
/// The most of a sentry's field a spotter's seen ground may cover.
pub const STACK_SHARE: f32 = 0.5;

/// A spotter as a map places it.
#[derive(Clone, Debug)]
pub struct Spotter {
    pub name: String,
    /// Its winch.
    pub at: P2,
    /// How far it sees: its own sight, or less where its winch stands in a
    /// region that caps how far machines notice (`Region::notice`).
    pub sight: f32,
}

impl Spotter {
    pub fn of(m: &crate::map::Marker) -> Option<Spotter> {
        if m.kind != MarkerKind::Spotter {
            return None;
        }
        Some(Spotter {
            name: if m.name.is_empty() {
                "a spotter".into()
            } else {
                m.name.clone()
            },
            at: m.at,
            sight: spotter::SIGHT,
        })
    }

    /// The envelope's height above sea level.
    pub fn eye(&self, hm: &Heightmap) -> f32 {
        hm.height(self.at) + spotter::TETHER
    }

    /// Whether its turning eye ever looks at a body standing at `p`: within
    /// its sight, and not so far below its level eye that it's under the
    /// bottom of its view (the blind ring).
    pub fn in_view(&self, hm: &Heightmap, p: P2) -> bool {
        let d = dist(self.at, p);
        let drop = self.eye(hm) - (hm.height(p) + BODY);
        d > 0.5
            && d.hypot(drop) <= self.sight
            && drop.atan2(d).abs() <= (spotter::FOV * 0.5).to_radians()
    }

    fn sees_with(&self, hm: &Heightmap, blocks: &[Block], p: P2) -> bool {
        self.in_view(hm, p)
            && map_sentry::clear(
                hm,
                &map_sentry::near_line(blocks, self.at, p),
                (self.at, self.eye(hm)),
                (p, hm.height(p) + BODY),
            )
    }

    /// Whether it can ever see someone standing at `p`.
    pub fn sees(&self, map: &MapDef, hm: &Heightmap, p: P2) -> bool {
        self.sees_with(hm, &map_sentry::blocks(map, hm), p)
    }

    /// Its field on a walk grid: `reachable` the walkable cells in its
    /// view, `visible` those it sees.
    pub fn field(&self, map: &MapDef, hm: &Heightmap, g: &NavGrid) -> Field {
        field_with(self, hm, &map_sentry::blocks(map, hm), g)
    }
}

fn field_with(s: &Spotter, hm: &Heightmap, blocks: &[Block], g: &NavGrid) -> Field {
    let r = s.sight;
    let mut seen = vec![false; g.n * g.n];
    let (mut reachable, mut visible) = (0, 0);
    let lo = g.cell_of([s.at[0] - r, s.at[1] - r]).unwrap_or((0, 0));
    let hi = g
        .cell_of([s.at[0] + r, s.at[1] + r])
        .unwrap_or((g.n - 1, g.n - 1));
    for j in lo.1..=hi.1 {
        for i in lo.0..=hi.0 {
            let c = g.centre(i, j);
            if !s.in_view(hm, c) {
                continue;
            }
            let k = j * g.n + i;
            let sees = s.sees_with(hm, blocks, c);
            seen[k] = sees;
            if g.cost[k] != BLOCKED {
                reachable += 1;
                visible += sees as usize;
            }
        }
    }
    Field {
        seen,
        reachable,
        visible,
    }
}

/// The map's spotters.
pub fn spotters(map: &MapDef) -> Vec<Spotter> {
    map.markers
        .iter()
        .filter_map(Spotter::of)
        .map(|mut s| {
            for reg in &map.regions {
                if let Some(n) = reg.notice {
                    if crate::geom::point_in_polygon(&reg.polygon, s.at) {
                        s.sight = s.sight.min(n);
                    }
                }
            }
            s
        })
        .collect()
}

/// Metres of a route (points every `map_lint::STEP`) on seen cells, and
/// the longest seen run.
fn route_seen(g: &NavGrid, seen: &[bool], pts: &[P2]) -> (f32, f32) {
    let (mut total, mut run, mut longest) = (0.0f32, 0.0f32, 0.0f32);
    for w in pts.windows(2) {
        let on = g.cell_of(w[1]).is_some_and(|(i, j)| seen[j * g.n + i]);
        if on {
            let d = dist(w[0], w[1]);
            total += d;
            run += d;
            longest = longest.max(run);
        } else {
            run = 0.0;
        }
    }
    (total, longest)
}

/// For every cell, the least metres of `seen` ground a walk from `from`
/// crosses to get there (infinite where no walk does).
fn seen_crossing(g: &NavGrid, from: P2, seen: &[bool]) -> Vec<f32> {
    #[derive(PartialEq)]
    struct Node(f32, usize);
    impl Eq for Node {}
    impl Ord for Node {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            o.0.total_cmp(&self.0)
        }
    }
    impl PartialOrd for Node {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    let n = g.n;
    let mut cost = vec![f32::INFINITY; n * n];
    let start = if g.walkable(from) {
        Some(from)
    } else {
        g.nearest_walkable(from)
    };
    let Some((i0, j0)) = start.and_then(|p| g.cell_of(p)) else {
        return cost;
    };
    let k0 = j0 * n + i0;
    cost[k0] = 0.0;
    let mut heap = std::collections::BinaryHeap::from([Node(0.0, k0)]);
    let open = |k: usize| g.cost[k] != BLOCKED;
    while let Some(Node(c0, k)) = heap.pop() {
        if c0 > cost[k] {
            continue;
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
            if !open(nk) {
                continue;
            }
            let diagonal = di != 0 && dj != 0;
            if diagonal {
                let a = j as usize * n + ni as usize;
                let b = nj as usize * n + i as usize;
                if !open(a) || !open(b) {
                    continue;
                }
            }
            // Half the step in each cell's sight.
            let step = g.cell
                * if diagonal {
                    std::f32::consts::SQRT_2
                } else {
                    1.0
                };
            let c = c0 + step * 0.5 * (seen[k] as u8 + seen[nk] as u8) as f32;
            if c < cost[nk] {
                cost[nk] = c;
                heap.push(Node(c, nk));
            }
        }
    }
    cost
}

/// The least of `v` over cells within `r` of `p`.
fn least_within(g: &NavGrid, v: &[f32], p: P2, r: f32) -> f32 {
    let k = (r / g.cell).ceil() as i64;
    let Some((ci, cj)) = g.cell_of(p) else {
        return f32::INFINITY;
    };
    let mut best = f32::INFINITY;
    for j in cj as i64 - k..=cj as i64 + k {
        for i in ci as i64 - k..=ci as i64 + k {
            if i < 0 || j < 0 || i >= g.n as i64 || j >= g.n as i64 {
                continue;
            }
            let (i, j) = (i as usize, j as usize);
            if dist(g.centre(i, j), p) <= r {
                best = best.min(v[j * g.n + i]);
            }
        }
    }
    best
}

/// Every spotter against the rules above. `walk` is the walkers' grid,
/// `routes` the main routes' points, `inside_fence` the lint's fence test.
pub fn check(
    map: &MapDef,
    hm: &Heightmap,
    walk: &NavGrid,
    ins: P2,
    routes: &[(String, Vec<P2>)],
    inside_fence: impl Fn(P2) -> bool,
) -> Checked {
    let mut out = Checked::default();
    let all = spotters(map);
    if all.is_empty() {
        return out;
    }
    let blocks = map_sentry::blocks(map, hm);
    let watched: Vec<(String, P2)> = map
        .markers
        .iter()
        .filter(|m| {
            matches!(
                m.kind,
                MarkerKind::ValleyKey { .. }
                    | MarkerKind::Poi
                    | MarkerKind::WorkSite
                    | MarkerKind::Loot { .. }
                    | MarkerKind::ArtifactField
            )
        })
        .map(|m| (m.name.clone(), m.at))
        .chain(
            map.placements
                .iter()
                .filter(|p| crate::underground::is_site(&p.prefab))
                .map(|p| (p.prefab.clone(), p.at)),
        )
        .collect();
    let ways = map_sentry::ways_out(map, ins);
    let drones = map_sentry::drone_ways(map);
    let sentries = map_sentry::sentries(map);
    let gate = map.markers_of(&MarkerKind::RobotGate).next().map(|m| m.at);
    let reach = spotter::SIGHT + crate::sentry::SIGHT;
    let mut sentry_fields: Vec<Option<Field>> = sentries.iter().map(|_| None).collect();
    for s in &all {
        let f = field_with(s, hm, &blocks, walk);
        let share = f.visible as f32 / f.reachable.max(1) as f32;
        let unseen = map_sentry::flood_avoiding(walk, ins, &f.seen);
        let hidden_way = map_sentry::reached_within(walk, &unseen, s.at, WAY_IN);
        let crossed = seen_crossing(walk, ins, &f.seen);
        let in_seen = least_within(walk, &crossed, s.at, WAY_IN);
        let way_in = in_seen <= WAY_SEEN;
        let slope = hm.slope_deg(s.at);
        let area = f.visible as f32 * walk.cell * walk.cell;
        out.measures.push(format!(
            "spotter {}: sees {:.0}% of the ground in its view ({:.0} m²), blind within {:.0} m, way in under it crossing {} of its ground{}, winch on {:.0}°, Gate {}",
            s.name,
            share * 100.0,
            area,
            spotter::blind_radius(spotter::TETHER),
            if in_seen.is_finite() {
                format!("{in_seen:.0} m")
            } else {
                "none: no way".into()
            },
            if hidden_way { " (and one unseen)" } else { "" },
            slope,
            gate.map_or("none".into(), |g| format!("{:.0} m off", dist(g, s.at))),
        ));
        // The winch.
        let footing = if !walk.walkable(s.at) {
            Some("isn't on walkable ground")
        } else if hm.water_level(s.at).is_some() {
            Some("is in the water")
        } else if slope > crate::map_lint::COMFORT_SLOPE {
            Some("is on ground too steep to stand and cut")
        } else if !inside_fence(s.at) {
            Some("is outside the fence")
        } else {
            None
        };
        if let Some(why) = footing {
            out.findings
                .push((format!("{}'s winch {why}", s.name), s.at));
        }
        if !watched
            .iter()
            .any(|(_, at)| dist(*at, s.at) <= spotter::SIGHT)
        {
            out.findings.push((
                format!("{} watches nothing within {} m", s.name, spotter::SIGHT),
                s.at,
            ));
        }
        if !way_in {
            out.findings.push((
                format!(
                    "the way in under {} crosses {in_seen:.0} m of the ground it sees (at most {WAY_SEEN:.0})",
                    s.name
                ),
                s.at,
            ));
        }
        if let Some((what, at)) = map_sentry::covered(&ways, |p| s.sees_with(hm, &blocks, p)) {
            out.findings
                .push((format!("{} watches {what}", s.name), at));
        }
        // The main routes across its ground: what it makes you go round.
        let crossed: Vec<String> = routes
            .iter()
            .filter_map(|(to, pts)| {
                let (m, longest) = route_seen(walk, &f.seen, pts);
                (m > 0.0).then(|| format!("to {to} {m:.0} m (longest {longest:.0} m)"))
            })
            .collect();
        out.measures.push(format!(
            "spotter {}: main routes in its sight: {}",
            s.name,
            if crossed.is_empty() {
                "none".into()
            } else {
                crossed.join(", ")
            }
        ));
        // Two machines on one spot: a sentry's field, a survey drone's way.
        let mut stacked = Vec::new();
        for (k, t) in sentries.iter().enumerate() {
            if dist(t.at, s.at) > reach {
                continue;
            }
            let tf = sentry_fields[k].get_or_insert_with(|| t.field(map, hm, walk));
            let both = (0..walk.n * walk.n)
                .filter(|&c| walk.cost[c] != BLOCKED && tf.seen[c] && f.seen[c])
                .count();
            let of = both as f32 / tf.visible.max(1) as f32;
            stacked.push(format!("{} {:.0}%", t.name, of * 100.0));
            if of > STACK_SHARE {
                out.findings.push((
                    format!(
                        "{} sees {:.0}% of {}'s field (at most {:.0}%)",
                        s.name,
                        of * 100.0,
                        t.name,
                        STACK_SHARE * 100.0
                    ),
                    t.at,
                ));
            }
        }
        let lanes: Vec<String> = drones
            .iter()
            .filter_map(|(poi, pts)| {
                let n = pts.iter().filter(|p| s.sees_with(hm, &blocks, **p)).count();
                (n > 0).then(|| format!("{poi} ({} m)", n * 2))
            })
            .collect();
        // Work sites: a pylon put up there watches as far, and calls too.
        let pylons: Vec<String> = map
            .markers_of(&MarkerKind::WorkSite)
            .filter(|m| dist(m.at, s.at) <= spotter::SIGHT)
            .map(|m| format!("{} {:.0} m off", m.name, dist(m.at, s.at)))
            .collect();
        out.measures.push(format!(
            "spotter {}: work sites (pylons) within its sight: {}",
            s.name,
            if pylons.is_empty() {
                "none".into()
            } else {
                pylons.join(", ")
            }
        ));
        out.measures.push(format!(
            "spotter {}: sentries' fields it sees: {}; survey drones' ways it sees: {}",
            s.name,
            if stacked.is_empty() {
                "none".into()
            } else {
                stacked.join(", ")
            },
            if lanes.is_empty() {
                "none".into()
            } else {
                lanes.join(", ")
            }
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Marker, TRAINING_YARD};

    fn spotter_at(at: P2) -> Marker {
        Marker {
            kind: MarkerKind::Spotter,
            at,
            rot: 0.0,
            name: "Test spotter".into(),
            y: 0.0,
        }
    }

    /// Under it is blind; out in its ring it sees; past its sight it
    /// doesn't; and a barn hides what's behind it.
    #[test]
    fn blind_under_it_and_a_barn_hides() {
        let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
        map.markers.push(spotter_at([0.0, 0.0]));
        let hm = Heightmap::build(&map);
        let s = &spotters(&map)[0];
        let flat = |p: P2| (hm.height(p) - hm.height(s.at)).abs() < 2.0;
        let under = [5.0, 0.0];
        let ring = [0.0, -50.0];
        assert!(flat(under) && flat(ring), "the yard isn't flat here");
        assert!(!s.sees(&map, &hm, under), "under it is blind");
        assert!(s.sees(&map, &hm, ring));
        assert!(!s.in_view(&hm, [0.0, -100.0]), "past its sight");
        map.placements.push(crate::map::Placement {
            prefab: "barn".into(),
            at: [0.0, -44.0],
            rot: 0.0,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
        let hm = Heightmap::build(&map);
        assert!(!spotters(&map)[0].sees(&map, &hm, [0.0, -49.0]));
    }

    /// A spotter over the insertion is found.
    #[test]
    fn a_spotter_over_the_way_in_is_found() {
        let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
        let ins = map.markers_of(&MarkerKind::Insertion).next().unwrap().at;
        map.markers.push(spotter_at([ins[0] + 40.0, ins[1]]));
        let hm = Heightmap::build(&map);
        let walk = crate::map_nav::zone_grid(&map, &hm, crate::map_nav::MAX_SLOPE);
        let c = check(&map, &hm, &walk, ins, &[], |_| true);
        let text: Vec<&str> = c.findings.iter().map(|f| f.0.as_str()).collect();
        assert!(
            text.iter().any(|t| t.contains("watches the insertion")),
            "{text:?}"
        );
    }
}

/// Where a spotter could fly over a place: `SCOUT_SPOTTER=map:x:z cargo
/// test -p extraction_core scout_spotter -- --ignored --nocapture` tries
/// winches round (x, z) out to 60 m and prints the ones that pass, the
/// most main route in sight first (what it makes you go round), then the
/// least sentry ground.
#[cfg(test)]
mod scout {
    use super::*;

    #[test]
    #[ignore]
    fn scout_spotter() {
        let arg = std::env::var("SCOUT_SPOTTER").unwrap();
        let v: Vec<&str> = arg.split(':').collect();
        let text = crate::map::VALLEYS
            .iter()
            .find(|m| m.0 == v[0])
            .expect("a valley")
            .1;
        let target = [v[1].parse::<f32>().unwrap(), v[2].parse::<f32>().unwrap()];
        let base = MapDef::from_ron(text).unwrap();
        // Without the map's own spotters: scouting a replacement.
        let base = MapDef {
            markers: base
                .markers
                .iter()
                .filter(|m| m.kind != MarkerKind::Spotter)
                .cloned()
                .collect(),
            ..base
        };
        let hm = Heightmap::build(&base);
        let walk = crate::map_nav::zone_grid(&base, &hm, crate::map_nav::MAX_SLOPE);
        let ins = base.markers_of(&MarkerKind::Insertion).next().unwrap().at;
        let routes: Vec<(String, Vec<P2>)> = crate::map_lint::lint(&base, &hm)
            .routes
            .into_iter()
            .map(|r| (r.to, r.points))
            .collect();
        let mut good = Vec::new();
        for ring in [0.0f32, 15.0, 30.0, 45.0, 60.0] {
            let n = if ring == 0.0 { 1 } else { 12 };
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                let at = [target[0] + a.cos() * ring, target[1] + a.sin() * ring];
                let mut map = base.clone();
                map.markers.push(crate::map::Marker {
                    kind: MarkerKind::Spotter,
                    at,
                    rot: 0.0,
                    name: "S".into(),
                    y: 0.0,
                });
                let c = check(&map, &hm, &walk, ins, &routes, |p| {
                    crate::map_lint::inside_fence(&map, p)
                });
                if !c.findings.is_empty() {
                    let why: Vec<&str> = c.findings.iter().map(|f| f.0.as_str()).collect();
                    println!("  ({:.0}, {:.0}) fails: {}", at[0], at[1], why.join("; "));
                    continue;
                }
                let s = &spotters(&map)[0];
                let f = s.field(&map, &hm, &walk);
                let on_routes: f32 = routes
                    .iter()
                    .map(|(_, pts)| route_seen(&walk, &f.seen, pts).0)
                    .sum();
                good.push((on_routes, f.visible, at, c.measures));
            }
        }
        good.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cmp(&a.1)));
        for g in good.iter().take(12) {
            println!(
                "at ({:.1}, {:.1}): {:.0} m of main route in sight, {} cells",
                g.2[0], g.2[1], g.0, g.1
            );
            for m in &g.3 {
                println!("    {m}");
            }
        }
        println!("{} passing", good.len());
    }
}
