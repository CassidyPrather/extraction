//! The Surge, as the map lint sees it (`docs/MAP_DESIGN.md`, "The
//! Surge"): from wherever a raid goes there is a roof in reach once the
//! radio says "One minute. Get out, or get under something.", and the
//! Surge's own anomalies never open against a wall or in a doorway.
//!
//! Distances are walked on the comfort grid (`map_lint::COMFORT_SLOPE`),
//! eight ways and round every footprint, from the shelters' doors: the
//! way a player who isn't scrambling up a bank would go. Grid steps run a
//! few per cent longer than a straight line, so the numbers err long.

use crate::geom::{self, P2, dist};
use crate::map::{MapDef, prefab_base, prefab_entrances};
use crate::map_lint::{ENTERABLE, Finding, Rect, Report, Route, Rule, Solid, inside_fence, solids};
use crate::nav::{BLOCKED, NavGrid};
use crate::terrain::Heightmap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

// ---- Shelter in reach --------------------------------------------------------

/// Seconds between the radio's "One minute" and the Surge
/// (`raid::update`'s line at 60 s to go).
pub const SURGE_CALL: f32 = 60.0;
/// How fast a body walks and sprints, m/s (`player::WALK_SPEED`,
/// `SPRINT_SPEED`), and how long full stamina lasts at a sprint and takes
/// to come back walking, seconds (`player.rs`: drained over 12 s, back
/// over 8).
pub const WALK: f32 = 2.4;
pub const SPRINT: f32 = 4.6;
pub const SPRINT_FOR: f32 = 12.0;
pub const RECOVER_IN: f32 = 8.0;

/// How far a body gets in `secs`, sprinting whenever it has the breath,
/// from fresh: full stamina sprinted off, then sprint and walk in the
/// share stamina allows (3 parts in 5 sprinting). A minute is 234 m, not
/// the 276 m a sprint would make it: nobody sprints a minute.
pub const fn dash(secs: f32) -> f32 {
    // Sprinting `s` of the `secs` and walking the rest uses
    // s / SPRINT_FOR of stamina and gives back (secs - s) / RECOVER_IN:
    // fresh, the budget is one. Solve for s.
    let s = (1.0 + secs / RECOVER_IN) / (1.0 / SPRINT_FOR + 1.0 / RECOVER_IN);
    let s = if s > secs { secs } else { s };
    s * SPRINT + (secs - s) * WALK
}

/// From every point of every main route, a roof within the minute's walk
/// after the call (144 m): a first-timer on the way out hears it and
/// walks in, no running.
pub const ROUTE_ROOF: f32 = SURGE_CALL * WALK;
/// From everywhere a raid can go on comfortable ground, a roof within the
/// minute at the best pace a body keeps up ([`dash`], 234 m): off the
/// routes you were warned four minutes earlier, and a minute's run is
/// still enough.
pub const ANYWHERE_ROOF: f32 = dash(SURGE_CALL);
/// How far the whole warning takes you at a walk (five minutes, 720 m):
/// measured for the record (`raid::SURGE_WARNING`).
pub const SURGE_WARNING: f32 = 300.0;
/// Where a door is reached from: this far out from its face.
pub const DOOR_STEP: f32 = 1.5;
/// A patch of open ground smaller than this, square metres, is noted in
/// the measures but not a finding (a cell or two on a bank).
pub const PATCH_MIN: f32 = 16.0;

// ---- Anomalies off the walls --------------------------------------------------

/// The Surge layouts checked against the walls: seeds, and the cycles of
/// each (a cycle re-rolls the fields).
pub const SURGE_SEEDS: u64 = 8;
pub const SURGE_CYCLES: u32 = 4;
/// An anomaly's reach (`AnomalyKind::danger_radius`) stays at least this
/// far off any wall (a building's footprint, a kiosk's with its apron):
/// room to walk round a building without walking into one.
pub const WALL_CLEAR: f32 = 2.0;
/// And out of the strip in front of every door: this far out from its
/// face, as wide as the door and half a metre either side.
pub const DOOR_STRIP: f32 = 4.0;

/// A way under a roof: what it is, and where you stand to go in.
#[derive(Clone, Debug)]
pub struct Shelter {
    pub name: String,
    pub doors: Vec<P2>,
}

/// Whether a placement keeps the Surge off: a walk-in building with its
/// roof on (a burnt house is open to the sky behind its front, a drowned
/// one has no roof at all), or anything with a door you walk through (a
/// boxcar, a signal box, a chapel).
pub fn roofed(prefab: &str) -> bool {
    if prefab.ends_with("_burnt") || prefab.ends_with("_drowned") {
        return false;
    }
    ENTERABLE.contains(&prefab_base(prefab)) || !prefab_entrances(prefab).is_empty()
}

/// A door's middle on the outside face, and the way out, in the map.
fn doors_of(s: &Solid) -> Vec<(P2, P2, f32)> {
    if let Some((at, out)) = s.kiosk_door() {
        return vec![(at, out, 2.0 * crate::underground::STAIR_HALF)];
    }
    prefab_entrances(&s.prefab)
        .iter()
        .map(|e| {
            let at = s.rect.world(e.at);
            let out = geom::sub(s.rect.world(geom::add(e.at, e.out)), at);
            (at, out, e.width)
        })
        .collect()
}

/// Every door's strip on the map, the kiosks' too: what it is, and the
/// strip [`DOOR_STRIP`] out from the face with a shoulder either side (as
/// [`anomaly_room`] keeps anomalies off them).
pub(crate) fn door_strips(map: &MapDef) -> Vec<(String, Rect)> {
    let mut out = Vec::new();
    for s in solids(map) {
        for (at, out_dir, width) in doors_of(&s) {
            out.push((
                format!("the {}'s door", s.name),
                Rect {
                    at: geom::add(at, geom::scale(out_dir, DOOR_STRIP * 0.5)),
                    half: [width * 0.5 + 0.5, DOOR_STRIP * 0.5],
                    yaw: (-out_dir[0]).atan2(out_dir[1]),
                },
            ));
        }
    }
    out
}

/// Every shelter on the map: roofed buildings by their doors (an open
/// shed with no doors, the sawmill, from anywhere round it), and every
/// kiosk by its door.
pub fn shelters(map: &MapDef) -> Vec<Shelter> {
    solids(map)
        .iter()
        .filter(|s| s.kiosk() || roofed(&s.prefab))
        .map(|s| {
            let doors = doors_of(s);
            let doors = if doors.is_empty() {
                s.rect
                    .outline(2.0)
                    .into_iter()
                    .map(|q| {
                        let away = geom::sub(q, s.rect.at);
                        let l = geom::len(away).max(1e-3);
                        geom::add(q, geom::scale(away, DOOR_STEP / l))
                    })
                    .collect()
            } else {
                doors
                    .into_iter()
                    .map(|(at, out, _)| geom::add(at, geom::scale(out, DOOR_STEP)))
                    .collect()
            };
            Shelter {
                name: s.name.clone(),
                doors,
            }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq)]
struct Open(f32, usize);

impl Eq for Open {}

impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0)
    }
}

impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Metres walked from the nearest of `from` to every cell of `g`
/// (infinite where none can be reached), stepping as the pathfinder does:
/// eight ways, never cutting a blocked corner. A source on a blocked cell
/// starts from the nearest open cell within two.
pub fn walked(g: &NavGrid, from: &[P2]) -> Vec<f32> {
    let n = g.n;
    let mut d = vec![f32::INFINITY; n * n];
    let mut open = BinaryHeap::new();
    for p in from {
        let Some((ci, cj)) = g.cell_of(*p) else {
            continue;
        };
        let mut best: Option<(f32, usize)> = None;
        for j in cj.saturating_sub(2)..=(cj + 2).min(n - 1) {
            for i in ci.saturating_sub(2)..=(ci + 2).min(n - 1) {
                let k = j * n + i;
                let e = dist(g.centre(i, j), *p);
                if g.cost[k] != BLOCKED && best.is_none_or(|b| e < b.0) {
                    best = Some((e, k));
                }
            }
        }
        if let Some((e, k)) = best
            && e < d[k]
        {
            d[k] = e;
            open.push(Open(e, k));
        }
    }
    let diag = g.cell * std::f32::consts::SQRT_2;
    while let Some(Open(dk, k)) = open.pop() {
        if dk > d[k] {
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
            if g.cost[nk] == BLOCKED {
                continue;
            }
            let step = if di != 0 && dj != 0 {
                let a = j as usize * n + ni as usize;
                let b = nj as usize * n + i as usize;
                if g.cost[a] == BLOCKED || g.cost[b] == BLOCKED {
                    continue;
                }
                diag
            } else {
                g.cell
            };
            let e = dk + step;
            if e < d[nk] {
                d[nk] = e;
                open.push(Open(e, nk));
            }
        }
    }
    d
}

/// The walk from `p` to the nearest roof, from the best cell within
/// `near` of it (a route point can sit on a bridge lane's edge, a target
/// in a crater you edge up to).
fn roof_at(g: &NavGrid, d: &[f32], p: P2, near: f32) -> f32 {
    let Some((ci, cj)) = g.cell_of(p) else {
        return f32::INFINITY;
    };
    let k = (near / g.cell).ceil().max(1.0) as usize;
    let mut best = f32::INFINITY;
    for j in cj.saturating_sub(k)..=(cj + k).min(g.n - 1) {
        for i in ci.saturating_sub(k)..=(ci + k).min(g.n - 1) {
            let e = dist(g.centre(i, j), p);
            if e <= near.max(g.cell * 1.5) {
                best = best.min(d[j * g.n + i] + e);
            }
        }
    }
    best
}

/// The `q`th quantile of `v` (sorted in place).
fn quantile(v: &mut [f32], q: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f32 * q).round() as usize]
}

/// The walk to the nearest roof from every cell of the comfort grid, and
/// that grid, for `map-dump` to draw: `(grid, metres)`.
pub fn roof_field(map: &MapDef, hm: &Heightmap) -> (NavGrid, Vec<f32>) {
    let g = crate::map_nav::zone_grid(map, hm, crate::map_lint::COMFORT_SLOPE);
    let doors: Vec<P2> = shelters(map).into_iter().flat_map(|s| s.doors).collect();
    let d = walked(&g, &doors);
    (g, d)
}

/// How much room an anomaly has at `p`: its reach must stay within this
/// (negative: `p` is in a wall or a doorway already), and what's nearest.
/// Every wall on the surface keeps [`WALL_CLEAR`] (a kiosk's apron
/// counts as its wall: the hole's edge is under it), and every door the
/// [`DOOR_STRIP`] in front of it.
pub fn anomaly_room(map: &MapDef, p: P2) -> (f32, String) {
    let mut best = (f32::INFINITY, String::new());
    for s in solids(map) {
        if s.prefab == "anomaly_core" {
            continue;
        }
        // Only what's near can be nearest.
        if dist(s.rect.at, p) - geom::len(s.rect.half) - DOOR_STRIP > best.0 + WALL_CLEAR {
            continue;
        }
        let wall = s.rect.sdf(p) - WALL_CLEAR;
        if wall < best.0 {
            best = (wall, format!("the {}'s wall", s.name));
        }
        for (at, out, width) in doors_of(&s) {
            let strip = Rect {
                at: geom::add(at, geom::scale(out, DOOR_STRIP * 0.5)),
                half: [width * 0.5 + 0.5, DOOR_STRIP * 0.5],
                yaw: (-out[0]).atan2(out[1]),
            };
            let e = strip.sdf(p);
            if e < best.0 {
                best = (e, format!("the {}'s door", s.name));
            }
        }
    }
    best
}

/// The Surge's checks, into `r`: shelter in reach (from the main routes,
/// and from everywhere comfortable), and its anomalies off walls and
/// doors. `comfy` is the comfort grid and `seen` what of it the insertion
/// reaches; `targets` are the lint's (name, where, how near counts).
pub fn check(
    map: &MapDef,
    hm: &Heightmap,
    comfy: &NavGrid,
    seen: &[bool],
    routes: &[Route],
    targets: &[(String, P2, f32)],
    r: &mut Report,
) {
    let find = |r: &mut Report, what: String, at: P2| {
        r.findings.push(Finding {
            rule: Rule::Surge,
            what,
            at,
        });
    };
    let shelters = shelters(map);
    let doors: Vec<P2> = shelters.iter().flat_map(|s| s.doors.clone()).collect();
    let d = walked(comfy, &doors);
    let secs = |m: f32| m / WALK;

    // From the routes: a minute's walk.
    for route in routes {
        let along: Vec<f32> = route
            .points
            .iter()
            .map(|p| roof_at(comfy, &d, *p, comfy.cell))
            .collect();
        let worst = along.iter().copied().fold(0.0f32, f32::max);
        let p95 = quantile(&mut along.clone(), 0.95);
        r.measures.push(format!(
            "route to {}: a roof within {worst:.0} m everywhere ({:.0} s at a walk), 95% within {p95:.0} m",
            route.to,
            secs(worst)
        ));
        // Each stretch out of reach, by its worst point.
        let mut k = 0;
        while k < along.len() {
            if along[k] <= ROUTE_ROOF {
                k += 1;
                continue;
            }
            let start = k;
            let mut top = k;
            while k < along.len() && along[k] > ROUTE_ROOF {
                if along[k] > along[top] {
                    top = k;
                }
                k += 1;
            }
            let long: f32 = route.points[start..k]
                .windows(2)
                .map(|w| dist(w[0], w[1]))
                .sum();
            find(
                r,
                format!(
                    "{long:.0} m of the way to {} is over {ROUTE_ROOF:.0} m from a roof (a minute's walk); here {:.0} m ({:.0} s)",
                    route.to,
                    along[top],
                    secs(along[top])
                ),
                route.points[top],
            );
        }
    }

    // From anywhere comfortable inside the fence: a minute at a run.
    let n = comfy.n;
    let open: Vec<usize> = (0..n * n)
        .filter(|&k| seen[k] && inside_fence(map, comfy.centre(k % n, k / n)))
        .collect();
    let mut all: Vec<f32> = open.iter().map(|&k| d[k]).collect();
    let worst_k = open.iter().copied().max_by(|a, b| d[*a].total_cmp(&d[*b]));
    let cell_area = comfy.cell * comfy.cell;
    let over = |lim: f32| all.iter().filter(|x| **x > lim).count() as f32 * cell_area;
    let (o_any, o_route, o_warn) = (
        over(ANYWHERE_ROOF),
        over(ROUTE_ROOF),
        over(SURGE_WARNING * WALK),
    );
    let p95 = quantile(&mut all, 0.95);
    let p99 = quantile(&mut all, 0.99);
    if let Some(wk) = worst_k {
        r.measures.push(format!(
            "surge: {} roofs; anywhere a roof within {:.0} m (at {:.0}, {:.0}), 95% within {p95:.0} m, 99% {p99:.0} m; \
             ground over {ROUTE_ROOF:.0} m {o_route:.0} m², over {ANYWHERE_ROOF:.0} m {o_any:.0} m², over {:.0} m {o_warn:.0} m²",
            shelters.len(),
            d[wk],
            comfy.centre(wk % n, wk / n)[0],
            comfy.centre(wk % n, wk / n)[1],
            SURGE_WARNING * WALK
        ));
    }
    // Patches out of reach, one finding each at its worst cell.
    let mut done = vec![false; n * n];
    let bad =
        |k: usize| seen[k] && d[k] > ANYWHERE_ROOF && inside_fence(map, comfy.centre(k % n, k / n));
    let mut small = 0.0;
    for &k0 in &open {
        if done[k0] || !bad(k0) {
            continue;
        }
        let mut stack = vec![k0];
        done[k0] = true;
        let (mut cells, mut top) = (0usize, k0);
        while let Some(k) = stack.pop() {
            cells += 1;
            if d[k] > d[top] {
                top = k;
            }
            let (i, j) = ((k % n) as i64, (k / n) as i64);
            for dj in -1..=1i64 {
                for di in -1..=1i64 {
                    let (ni, nj) = (i + di, j + dj);
                    if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                        continue;
                    }
                    let nk = nj as usize * n + ni as usize;
                    if !done[nk] && bad(nk) {
                        done[nk] = true;
                        stack.push(nk);
                    }
                }
            }
        }
        let area = cells as f32 * cell_area;
        if area < PATCH_MIN {
            small += area;
            continue;
        }
        let at = comfy.centre(top % n, top / n);
        let what = if d[top].is_finite() {
            format!("{:.0} m ({:.0} s at a walk)", d[top], secs(d[top]))
        } else {
            "no way".into()
        };
        find(
            r,
            format!(
                "{area:.0} m² of open ground over {ANYWHERE_ROOF:.0} m from a roof (a minute at a run); here {what}"
            ),
            at,
        );
    }
    let mut far: Vec<(f32, &str)> = targets
        .iter()
        .map(|(name, at, near)| (roof_at(comfy, &d, *at, *near), name.as_str()))
        .collect();
    far.sort_by(|a, b| b.0.total_cmp(&a.0));
    if let Some((m, name)) = far.first() {
        r.measures.push(format!(
            "surge: farthest target from a roof {name}, {m:.0} m; slivers over {ANYWHERE_ROOF:.0} m {small:.0} m²"
        ));
    }

    // The Surge's anomalies off walls and doors.
    let mut seen_at: Vec<P2> = Vec::new();
    let (mut total, mut against) = (0, 0);
    for seed in 0..SURGE_SEEDS {
        for cycle in 0..SURGE_CYCLES {
            for a in crate::anomaly::layout(map, hm, seed, cycle) {
                total += 1;
                let reach = a.kind.danger_radius(a.radius);
                let (room, what) = anomaly_room(map, a.at);
                if room >= reach {
                    continue;
                }
                against += 1;
                // A standing anomaly is in every layout: once is enough.
                if seen_at.iter().any(|q| dist(*q, a.at) < 0.5) {
                    continue;
                }
                seen_at.push(a.at);
                let gap = room - reach;
                let to = if what.ends_with("door") {
                    format!("{:.1} m into the strip before {what}", -gap)
                } else {
                    format!("{:.1} m from {what}", (gap + WALL_CLEAR).max(0.0))
                };
                find(
                    r,
                    format!(
                        "a {:?} (reach {reach:.1} m) at ({:.1}, {:.1}) reaches {to}",
                        a.kind, a.at[0], a.at[1]
                    ),
                    a.at,
                );
            }
        }
    }
    r.measures.push(format!(
        "surge anomalies over {SURGE_SEEDS} seeds x {SURGE_CYCLES} cycles: {total}, {against} against a wall or a door"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{CINDER_WORKS, Placement};
    use crate::map_lint::lint;

    fn placed(prefab: &str, at: P2, rot: f32) -> Placement {
        Placement {
            prefab: prefab.into(),
            at,
            rot,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        }
    }

    #[test]
    fn a_minute_at_a_run_is_not_a_minute_sprinting() {
        let m = dash(SURGE_CALL);
        assert!((m - 233.8).abs() < 1.0, "{m}");
        assert!(
            (dash(10.0) - 46.0).abs() < 1e-3,
            "fresh, ten seconds is all sprint"
        );
        assert!((ROUTE_ROOF - 144.0).abs() < 1e-3);
    }

    #[test]
    fn doors_turn_with_their_building() {
        // A shed turned a quarter: its door turns with it, as
        // `underground::turn` (the game's yaw) turns (x, z).
        let map = MapDef {
            placements: vec![placed("shed", [10.0, 20.0], 90.0)],
            ..MapDef::from_ron(crate::map::TRAINING_YARD).unwrap()
        };
        let s = shelters(&map)
            .into_iter()
            .find(|s| s.name == "shed")
            .unwrap();
        let door = s.doors[0];
        let want = crate::underground::turn([-1.0, -1.55 - DOOR_STEP], 90.0);
        assert!(
            dist(door, [10.0 + want[0], 20.0 + want[1]]) < 1e-3,
            "{door:?}"
        );
        assert!(roofed("barn") && roofed("boxcar") && !roofed("house_plank_burnt"));
        assert!(!roofed("radio_mast") && !roofed("house_brick_drowned"));
    }

    #[test]
    fn a_long_walk_from_any_roof_is_found() {
        let mut map = MapDef::from_ron(CINDER_WORKS).unwrap();
        // Every roof gone but the farthest from the insertion's side.
        map.placements.retain(|p| !roofed(&p.prefab));
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Surge && f.what.contains("of the way to")),
            "{}",
            r.text()
        );
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Surge && f.what.contains("of open ground")),
            "{}",
            r.text()
        );
    }

    #[test]
    fn an_anomaly_against_a_wall_or_in_a_doorway_is_found() {
        let mut map = MapDef::from_ron(CINDER_WORKS).unwrap();
        let hall = map
            .placements
            .iter()
            .find(|p| p.prefab == "factory_hall")
            .unwrap()
            .clone();
        let rc = Rect::of(&hall.prefab, hall.at, hall.rot);
        // A snare reaching to a metre off its front wall (between its two
        // big doors), a mirror on its back door's step.
        let side = rc.world([0.0, -(rc.half[1] + 1.0 + 1.8)]);
        let door = rc.world([12.6, rc.half[1] + 3.5]);
        for (kind, at, radius) in [("Snare", side, 1.8), ("Mirror", door, 1.2)] {
            map.markers.push(crate::map::Marker {
                kind: crate::map::MarkerKind::Anomaly {
                    kind: kind.into(),
                    radius,
                },
                at,
                rot: 0.0,
                name: format!("test {kind}"),
                y: 0.0,
            });
        }
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        let hits: Vec<&Finding> = r
            .findings
            .iter()
            .filter(|f| f.rule == Rule::Surge && f.what.contains("factory_hall"))
            .collect();
        assert!(
            hits.iter()
                .any(|f| f.what.contains("Snare") && f.what.contains("wall")),
            "{}",
            r.text()
        );
        assert!(
            hits.iter()
                .any(|f| f.what.contains("Mirror") && f.what.contains("door")),
            "{}",
            r.text()
        );
    }
}
