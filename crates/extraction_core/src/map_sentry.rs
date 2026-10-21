//! Sentries in the valleys: where a map may stand one (`MarkerKind::Sentry`)
//! so that it is a puzzle and not a wall. The machine's own rules are
//! [`crate::sentry`]; this is the lint's side (`Rule::Sentry`, see
//! `docs/MAP_DESIGN.md`, "Sentries").
//!
//! A sentry earns its place when:
//!
//! - **its field of fire is a field**: from its head it sees most of the
//!   ground its sweep can reach ([`FIELD_SHARE`] within [`sentry::SIGHT`]),
//!   not the back of a hill or a wall;
//! - **it guards something** within [`GUARD_REACH`]: a Gate, a key, a point
//!   of interest, a work site, loot, an underground site;
//! - **there is a way round**: from the insertion, a walk that never enters
//!   its field reaches within [`WAY_ROUND`] of its mast (behind it), and
//!   its junction box lies on that unseen ground, [`BOX_NEAR`] to
//!   [`BOX_FAR`] from the mast and inside the fence;
//! - **it never covers the way in or out**: not the insertion, not a
//!   rendezvous, not the truck's tailgate or the approach to it;
//! - **you see it before it sees you**: where a main route walks into its
//!   field, its head is in sight from [`WARN_RUN`] metres back along the
//!   route, so the chirp is never the first you know of it.
//!
//! Line of sight runs through the land and the buildings (a placement's
//! footprint to its height); trees don't stop rounds and don't count.

use crate::geom::{P2, dist};
use crate::map::{MapDef, MarkerKind, prefab_footprint};
use crate::nav::{BLOCKED, NavGrid};
use crate::sentry;
use crate::terrain::Heightmap;

/// Its head's height over the ground at its foot (the game's
/// `robots::sentry::MAST`).
pub const HEAD: f32 = 4.6;
/// What it has to see of you: a chest, standing.
pub const BODY: f32 = 1.3;
/// The share of the walkable ground its sweep reaches (within
/// [`sentry::SIGHT`]) that its head must actually see.
pub const FIELD_SHARE: f32 = 0.6;
/// Something worth guarding within this of the mast.
pub const GUARD_REACH: f32 = 40.0;
/// An unseen walk from the insertion gets this close to the mast.
pub const WAY_ROUND: f32 = 8.0;
/// The junction box: this far from the mast at least and at most.
pub const BOX_NEAR: f32 = 8.0;
pub const BOX_FAR: f32 = 25.0;
/// Where a main route walks into a field, the head is in sight from this
/// far back along it (about ten seconds at a walk).
pub const WARN_RUN: f32 = 25.0;
/// Survey drones circle a point of interest this far out (the game's
/// `robots::drone`, Working) and fly straight between it and the Gate.
pub const SURVEY_ORBIT: f32 = 14.0;
/// Lanes near the Gate are the robots' own doorstep: a sentry there
/// guards it, and that's the danger you chose by going.
pub const LANE_HOME: f32 = 40.0;

/// The truck's tailgate and the bots' approach behind a rendezvous
/// (the lint's `TAILGATE`, `TRUCK_APPROACH`).
const TRUCK_SPOTS: [f32; 3] = [0.0, 3.6, 8.0];

/// A sentry as a map places it.
#[derive(Clone, Debug)]
pub struct Sentry {
    pub name: String,
    pub at: P2,
    /// Middle of its arc, as [`sentry::bearing`] measures (see
    /// [`Sentry::of`]), and its full width, degrees.
    pub centre: f32,
    pub arc: f32,
    pub power: P2,
}

/// How far behind the mast its box stands when the map doesn't say (the
/// game's `robots::sentry::POWER_BEHIND`).
pub const POWER_BEHIND: f32 = 12.0;
/// The widest arc a sentry may sweep: past this, with its eye's own
/// field, there's no behind to get round to.
pub const ARC_MAX: f32 = 260.0;

impl Sentry {
    /// A sentry marker as the game stands it (`robots::sentry::from_marker`):
    /// `rot` faces the way a sign's board would (0 toward +Z, 90 toward
    /// +X), so its arc's middle is `180 - rot` as [`sentry::bearing`]
    /// measures; the box defaults to [`POWER_BEHIND`] straight behind.
    pub fn of(m: &crate::map::Marker) -> Option<Sentry> {
        let MarkerKind::Sentry { arc, power } = &m.kind else {
            return None;
        };
        let r = m.rot.to_radians();
        let facing = [r.sin(), r.cos()];
        Some(Sentry {
            name: if m.name.is_empty() {
                "a sentry".into()
            } else {
                m.name.clone()
            },
            at: m.at,
            centre: sentry::wrap_deg(180.0 - m.rot),
            arc: *arc,
            power: power.unwrap_or([
                m.at[0] - facing[0] * POWER_BEHIND,
                m.at[1] - facing[1] * POWER_BEHIND,
            ]),
        })
    }
}

/// The map's sentries.
pub fn sentries(map: &MapDef) -> Vec<Sentry> {
    map.markers.iter().filter_map(Sentry::of).collect()
}

/// A placement as a block that stops a line of sight.
struct Block {
    at: P2,
    half: P2,
    yaw: f32,
    reach: f32,
    top: f32,
}

impl Block {
    fn inside(&self, p: P2) -> bool {
        let d = [p[0] - self.at[0], p[1] - self.at[1]];
        let (s, c) = self.yaw.sin_cos();
        let l = [d[0] * c + d[1] * s, -d[0] * s + d[1] * c];
        l[0].abs() <= self.half[0] && l[1].abs() <= self.half[1]
    }
}

fn blocks(map: &MapDef, hm: &Heightmap) -> Vec<Block> {
    let mut out: Vec<Block> = map
        .placements
        .iter()
        .filter(|p| !crate::underground::is_site(&p.prefab) && p.prefab != "anomaly_core")
        .map(|p| {
            let f = prefab_footprint(&p.prefab);
            let base = if p.absolute {
                p.y
            } else {
                hm.height(p.at) + p.y
            };
            Block {
                at: p.at,
                half: [f[0] * 0.5, f[2] * 0.5],
                yaw: p.rot.to_radians(),
                reach: (f[0] * f[0] + f[2] * f[2]).sqrt() * 0.5,
                top: base + f[1],
            }
        })
        .collect();
    // Every underground kiosk, the entrances' and the exits', by its shell
    // (its apron is ground): a kiosk in front of a sentry hides what's
    // behind it like any shed.
    for s in crate::map_lint::solids(map) {
        if let Some(shell) = s.shell {
            out.push(Block {
                at: shell.at,
                half: shell.half,
                yaw: shell.yaw,
                reach: crate::geom::len(shell.half),
                top: hm.height(shell.at) + crate::underground::FOOTPRINT[1],
            });
        }
    }
    out
}

/// What one sentry sees, cell by cell of a walk grid.
pub struct Field {
    /// The cells it can ever see you on.
    pub seen: Vec<bool>,
    /// Walkable cells its sweep reaches, and of those how many it sees.
    pub reachable: usize,
    pub visible: usize,
}

/// Whether the line from `a` to `b` (heights above sea level) is clear of
/// the land and the buildings. The last metre at either end is free (the
/// mast's own pad, the ground you stand on).
fn clear(hm: &Heightmap, blocks: &[&Block], a: (P2, f32), b: (P2, f32)) -> bool {
    let d = dist(a.0, b.0);
    let n = (d / 1.0).ceil().max(1.0) as usize;
    (1..n).all(|i| {
        let t = i as f32 / n as f32;
        if d * t < 1.0 || d * (1.0 - t) < 1.0 {
            return true;
        }
        let p = crate::geom::lerp(a.0, b.0, t);
        let h = a.1 + (b.1 - a.1) * t;
        hm.height(p) < h - 0.1 && !blocks.iter().any(|k| h < k.top && k.inside(p))
    })
}

fn near_line<'a>(blocks: &'a [Block], a: P2, b: P2) -> Vec<&'a Block> {
    blocks
        .iter()
        .filter(|k| crate::geom::polyline_closest(&[a, b], k.at).0 <= k.reach)
        .collect()
}

impl Sentry {
    pub fn head(&self, hm: &Heightmap) -> f32 {
        hm.height(self.at) + HEAD
    }

    /// Whether its sweep can ever turn onto `p` (in range and arc).
    pub fn sweeps(&self, p: P2) -> bool {
        let d = dist(self.at, p);
        d <= sentry::SIGHT
            && d > 0.5
            && sentry::can_ever_see(sentry::bearing(self.at, p), self.centre, self.arc)
    }

    fn sees_with(&self, hm: &Heightmap, blocks: &[Block], p: P2) -> bool {
        self.sweeps(p)
            && clear(
                hm,
                &near_line(blocks, self.at, p),
                (self.at, self.head(hm)),
                (p, hm.height(p) + BODY),
            )
    }

    /// Whether it can ever see someone standing at `p`.
    pub fn sees(&self, map: &MapDef, hm: &Heightmap, p: P2) -> bool {
        self.sees_with(hm, &blocks(map, hm), p)
    }

    /// Its field on a walk grid.
    pub fn field(&self, map: &MapDef, hm: &Heightmap, g: &NavGrid) -> Field {
        let blocks = blocks(map, hm);
        let mut seen = vec![false; g.n * g.n];
        let (mut reachable, mut visible) = (0, 0);
        let lo = g.cell_of([self.at[0] - sentry::SIGHT, self.at[1] - sentry::SIGHT]);
        let hi = g.cell_of([self.at[0] + sentry::SIGHT, self.at[1] + sentry::SIGHT]);
        let (lo, hi) = (lo.unwrap_or((0, 0)), hi.unwrap_or((g.n - 1, g.n - 1)));
        for j in lo.1..=hi.1 {
            for i in lo.0..=hi.0 {
                let c = g.centre(i, j);
                if !self.sweeps(c) {
                    continue;
                }
                let k = j * g.n + i;
                let sees = self.sees_with(hm, &blocks, c);
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

    /// Whether the head is in sight from eye height at `p`, wherever it's
    /// looking.
    fn in_sight_from(&self, hm: &Heightmap, blocks: &[Block], p: P2) -> bool {
        clear(
            hm,
            &near_line(blocks, p, self.at),
            (p, hm.height(p) + 1.7),
            (self.at, self.head(hm)),
        )
    }
}

/// Cells reached from `from` on `g` without stepping onto `avoid`.
pub fn flood_avoiding(g: &NavGrid, from: P2, avoid: &[bool]) -> Vec<bool> {
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
    let k0 = j0 * n + i0;
    if avoid[k0] {
        return seen;
    }
    seen[k0] = true;
    let mut stack = vec![k0];
    let open = |k: usize| g.cost[k] != BLOCKED && !avoid[k];
    while let Some(k) = stack.pop() {
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
            if seen[nk] || !open(nk) {
                continue;
            }
            if di != 0 && dj != 0 {
                let a = j as usize * n + ni as usize;
                let b = nj as usize * n + i as usize;
                if !open(a) || !open(b) {
                    continue;
                }
            }
            seen[nk] = true;
            stack.push(nk);
        }
    }
    seen
}

/// Whether any reached cell lies within `r` of `p`.
fn reached_within(g: &NavGrid, seen: &[bool], p: P2, r: f32) -> bool {
    let k = (r / g.cell).ceil() as i64;
    let Some((ci, cj)) = g.cell_of(p) else {
        return false;
    };
    for j in cj as i64 - k..=cj as i64 + k {
        for i in ci as i64 - k..=ci as i64 + k {
            if i < 0 || j < 0 || i >= g.n as i64 || j >= g.n as i64 {
                continue;
            }
            let (i, j) = (i as usize, j as usize);
            if seen[j * g.n + i] && dist(g.centre(i, j), p) <= r {
                return true;
            }
        }
    }
    false
}

/// What a sentry check found: findings (what, where) and measures.
#[derive(Default)]
pub struct Checked {
    pub findings: Vec<(String, P2)>,
    pub measures: Vec<String>,
}

/// Every sentry against the rules above. `walk` is the walkers' grid,
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
    let blocks = blocks(map, hm);
    let guarded: Vec<(String, P2)> = map
        .markers
        .iter()
        .filter(|m| {
            matches!(
                m.kind,
                MarkerKind::RobotGate
                    | MarkerKind::ValleyKey { .. }
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
    let rvs: Vec<(&str, P2)> = map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| (m.name.as_str(), m.at))
        .collect();
    for s in sentries(map) {
        let f = s.field(map, hm, walk);
        let share = f.visible as f32 / f.reachable.max(1) as f32;
        let unseen = flood_avoiding(walk, ins, &f.seen);
        let way_round = reached_within(walk, &unseen, s.at, WAY_ROUND);
        let box_d = dist(s.at, s.power);
        let box_unseen = reached_within(walk, &unseen, s.power, 2.5);
        out.measures.push(format!(
            "sentry {}: sees {:.0}% of its sweep's ground, way round {}, box {:.0} m off{}",
            s.name,
            share * 100.0,
            if way_round { "yes" } else { "no" },
            box_d,
            if box_unseen {
                " unseen"
            } else {
                " in its field"
            },
        ));
        if s.arc > ARC_MAX {
            out.findings.push((
                format!("{} sweeps {:.0}° (at most {ARC_MAX})", s.name, s.arc),
                s.at,
            ));
        }
        if share < FIELD_SHARE {
            out.findings.push((
                format!(
                    "{} sees only {:.0}% of the ground its sweep reaches",
                    s.name,
                    share * 100.0
                ),
                s.at,
            ));
        }
        if !guarded.iter().any(|(_, at)| dist(*at, s.at) <= GUARD_REACH) {
            out.findings.push((
                format!("{} guards nothing within {GUARD_REACH} m", s.name),
                s.at,
            ));
        }
        if !way_round {
            out.findings.push((
                format!("no way to {} from the insertion out of its sight", s.name),
                s.at,
            ));
        }
        if !(BOX_NEAR..=BOX_FAR).contains(&box_d) || !inside_fence(s.power) {
            out.findings.push((
                format!(
                    "{}'s junction box is {box_d:.0} m from it (wants {BOX_NEAR}-{BOX_FAR}, inside the fence)",
                    s.name
                ),
                s.power,
            ));
        } else if !box_unseen {
            out.findings.push((
                format!("{}'s junction box can't be got to out of its sight", s.name),
                s.power,
            ));
        }
        let mut covers: Vec<(String, P2)> = vec![("the insertion".into(), ins)];
        for (name, at) in &rvs {
            let l = (at[0] * at[0] + at[1] * at[1]).sqrt().max(1e-3);
            let back = [-at[0] / l, -at[1] / l];
            for d in TRUCK_SPOTS {
                covers.push((
                    format!("the truck at {name}"),
                    [at[0] + back[0] * d, at[1] + back[1] * d],
                ));
            }
        }
        if let Some((what, at)) = covers.iter().find(|(_, at)| {
            // The spot or the ground round it (a gate may stand on the
            // spot itself).
            (0..9).any(|k| {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let r = if k == 8 { 0.0 } else { 3.0 };
                s.sees_with(hm, &blocks, [at[0] + a.cos() * r, at[1] + a.sin() * r])
            })
        }) {
            out.findings
                .push((format!("{} covers {what}", s.name), *at));
        }
        // Two machines on one spot: a survey drone's orbit or lane (past
        // the Gate's doorstep) through the sentry's sweep. The sims found
        // raiders who lived through one dying to the other in seconds, but
        // only where raiders go (Marrow's Depot, by its road east): every
        // Gate sentry sweeps the lanes leaving its Gate and every key
        // sentry a surveyed place, and those were quiet. So it's measured,
        // for reading against the sims, not held.
        let gate = map.markers_of(&MarkerKind::RobotGate).next().map(|m| m.at);
        let mut lanes = Vec::new();
        for poi in map.markers_of(&MarkerKind::Poi) {
            let ring = (0..24).map(|k| {
                let a = k as f32 / 24.0 * std::f32::consts::TAU;
                [
                    poi.at[0] + a.cos() * SURVEY_ORBIT,
                    poi.at[1] + a.sin() * SURVEY_ORBIT,
                ]
            });
            let path: Vec<P2> = match gate {
                Some(g) => {
                    let n = (dist(g, poi.at) / 2.0).ceil() as usize;
                    (0..=n)
                        .map(|i| crate::geom::lerp(g, poi.at, i as f32 / n.max(1) as f32))
                        .filter(|p| dist(*p, g) > LANE_HOME)
                        .collect()
                }
                None => Vec::new(),
            };
            let n = ring.chain(path).filter(|p| s.sweeps(*p)).count();
            if n > 0 {
                lanes.push(format!("{} ({} m)", poi.name, n * 2));
            }
        }
        out.measures.push(format!(
            "sentry {}: sweeps survey drones' ways to {}",
            s.name,
            if lanes.is_empty() {
                "none".to_string()
            } else {
                lanes.join(", ")
            }
        ));
        // Warning: where a route walks into its field, the head is in sight
        // from WARN_RUN back.
        for (to, pts) in routes {
            let Some(enter) = pts.iter().position(|p| {
                walk.cell_of(*p)
                    .is_some_and(|(i, j)| f.seen[j * walk.n + i])
            }) else {
                continue;
            };
            let mut back = 0.0;
            let mut k = enter;
            while k > 0 && back < WARN_RUN {
                back += dist(pts[k], pts[k - 1]);
                k -= 1;
            }
            let warned = back >= WARN_RUN - 0.5 && s.in_sight_from(hm, &blocks, pts[k]);
            out.measures.push(format!(
                "sentry {}: the route to {to} walks into its field{}",
                s.name,
                if warned { ", seen coming" } else { "" }
            ));
            if !warned {
                out.findings.push((
                    format!(
                        "the route to {to} walks into {}'s field without seeing it {WARN_RUN} m off",
                        s.name
                    ),
                    pts[enter],
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Marker, TRAINING_YARD};

    fn yard_with(s: Marker) -> (MapDef, Heightmap) {
        let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
        map.markers.push(s);
        let hm = Heightmap::build(&map);
        (map, hm)
    }

    /// A sentry facing `rot` the way the maps write it (0 toward +Z).
    fn sentry_at(at: P2, rot: f32, arc: f32, power: P2) -> Marker {
        Marker {
            kind: MarkerKind::Sentry {
                arc,
                power: Some(power),
            },
            at,
            rot,
            name: "Test sentry".into(),
            y: 0.0,
        }
    }

    #[test]
    fn behind_it_is_never_seen_and_in_front_is() {
        let (map, _) = yard_with(sentry_at([0.0, 0.0], 180.0, 90.0, [0.0, 15.0]));
        let s = &sentries(&map)[0];
        // Facing -Z: 20 m ahead it can look; 20 m behind, never.
        assert!(s.sweeps([0.0, -20.0]));
        assert!(!s.sweeps([0.0, 20.0]));
        assert!(!s.sweeps([0.0, -80.0]), "past its sight");
    }

    #[test]
    fn a_wall_hides_what_is_behind_it() {
        let (mut map, _) = yard_with(sentry_at([0.0, 0.0], 180.0, 90.0, [0.0, 15.0]));
        let p = [0.0, -30.0];
        let hm = Heightmap::build(&map);
        let open = sentries(&map)[0].sees(&map, &hm, p);
        map.placements.push(crate::map::Placement {
            prefab: "barn".into(),
            at: [0.0, -18.0],
            rot: 0.0,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
        let hm = Heightmap::build(&map);
        let hidden = !sentries(&map)[0].sees(&map, &hm, p);
        assert!(open && hidden, "open {open}, hidden {hidden}");
    }

    #[test]
    fn a_sentry_over_the_way_in_is_found() {
        let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
        let ins = map.markers_of(&MarkerKind::Insertion).next().unwrap().at;
        // Beside the insertion, looking at it, its box too far.
        let at = [ins[0] + 20.0, ins[1]];
        map.markers
            .push(sentry_at(at, 270.0, 120.0, [at[0] + 40.0, at[1]]));
        let hm = Heightmap::build(&map);
        let walk = crate::map_nav::zone_grid(&map, &hm, crate::map_nav::MAX_SLOPE);
        let c = check(&map, &hm, &walk, ins, &[], |_| true);
        let text: Vec<&str> = c.findings.iter().map(|f| f.0.as_str()).collect();
        assert!(
            text.iter().any(|t| t.contains("covers the insertion")),
            "{text:?}"
        );
        assert!(text.iter().any(|t| t.contains("junction box")), "{text:?}");
    }
}

/// Where a sentry could stand over a thing: `SCOUT_SENTRY=map:x:z cargo
/// test -p extraction_core scout_sentry -- --ignored --nocapture` tries
/// spots round (x, z), every facing and two arc widths, a box behind, and
/// prints the ones that pass, best field first.
#[cfg(test)]
mod scout {
    use super::*;

    #[test]
    #[ignore]
    fn scout_sentry() {
        let arg = std::env::var("SCOUT_SENTRY").unwrap();
        let v: Vec<&str> = arg.split(':').collect();
        let text = crate::map::VALLEYS
            .iter()
            .find(|m| m.0 == v[0])
            .expect("a valley")
            .1;
        let target = [v[1].parse::<f32>().unwrap(), v[2].parse::<f32>().unwrap()];
        let base = MapDef::from_ron(text).unwrap();
        let hm = Heightmap::build(&base);
        let walk = crate::map_nav::zone_grid(&base, &hm, crate::map_nav::MAX_SLOPE);
        let ins = base.markers_of(&MarkerKind::Insertion).next().unwrap().at;
        let routes: Vec<(String, Vec<P2>)> = crate::map_lint::lint(&base, &hm)
            .routes
            .into_iter()
            .map(|r| (r.to, r.points))
            .collect();
        let mut good = Vec::new();
        for ring in [6.0f32, 12.0, 18.0, 26.0] {
            for k in 0..12 {
                let a = k as f32 * 30f32.to_radians();
                let at = [target[0] + a.cos() * ring, target[1] + a.sin() * ring];
                if !walk.walkable(at) || hm.water_level(at).is_some() {
                    continue;
                }
                for c in 0..24 {
                    let centre = sentry::wrap_deg(c as f32 * 15.0);
                    for arc in [90.0f32, 140.0] {
                        // The box behind it, off to one side.
                        for (off, d) in [(150.0f32, 14.0f32), (210.0, 14.0), (180.0, 18.0)] {
                            let b = (centre + off).to_radians();
                            let power = [at[0] + b.sin() * d, at[1] - b.cos() * d];
                            let mut map = base.clone();
                            map.markers.push(crate::map::Marker {
                                kind: MarkerKind::Sentry {
                                    arc,
                                    power: Some(power),
                                },
                                at,
                                rot: sentry::wrap_deg(180.0 - centre),
                                name: "S".into(),
                                y: 0.0,
                            });
                            let c = check(&map, &hm, &walk, ins, &routes, |p| {
                                crate::map_lint::inside_fence(&map, p)
                            });
                            if c.findings.is_empty() {
                                let s = &sentries(&map)[0];
                                let f = s.field(&map, &hm, &walk);
                                let share = f.visible as f32 / f.reachable.max(1) as f32;
                                good.push((share, f.visible, at, centre, arc, power));
                            }
                        }
                    }
                }
            }
        }
        // Facing the way people come (toward the insertion), then the
        // biggest field.
        let facing = |g: &(f32, usize, P2, f32, f32, P2)| {
            let want = sentry::bearing(g.2, ins);
            (sentry::wrap_deg(g.3 - want).abs() / 30.0).round() as i32
        };
        good.sort_by(|a, b| facing(a).cmp(&facing(b)).then(b.1.cmp(&a.1)));
        for g in good.iter().take(15) {
            println!(
                "at ({:.1}, {:.1}) rot {:.0} arc {:.0} power ({:.1}, {:.1}): sees {:.0}% ({} cells)",
                g.2[0],
                g.2[1],
                sentry::wrap_deg(180.0 - g.3),
                g.4,
                g.5[0],
                g.5[1],
                g.0 * 100.0,
                g.1
            );
        }
        println!("{} passing", good.len());
    }
}
