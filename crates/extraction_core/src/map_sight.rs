//! The sightline audit: can you find your way by what you see?
//!
//! From every 10 m of walkable ground inside the fence, the nearest
//! landmark (a structure at least `map_lint::LANDMARK_HEIGHT` tall, as
//! `map_landmarks` sees it: the top of any of its tall parts) whose top
//! shows over the land; and from each insertion, rendezvous and
//! hilltop, which landmarks show in fog and in ordinary weather. Ground
//! with nothing in sight within fog range is where a player gets lost.
//!
//! `cargo test -p extraction_core map_sight -- --nocapture` prints each
//! valley's audit; `map-dump` writes it for `map_preview.py --sight`.

use crate::geom::{P2, dist};
use crate::map::{MapDef, MarkerKind, prefab_footprint};
use crate::map_lint::{CLEAR_SIGHT, FOG_SIGHT, inside_fence, sees};
use crate::terrain::Heightmap;
use std::fmt::Write as _;

/// Spacing of the audit's grid, metres.
pub const STEP: f32 = 10.0;
/// A hilltop: the highest ground within this radius, and this much above
/// the lowest ground on that circle.
pub const HILL_RADIUS: f32 = 50.0;
pub const HILL_RISE: f32 = 4.0;

/// Somewhere a player stands and looks round.
#[derive(Clone, Debug)]
pub struct Viewpoint {
    pub name: String,
    pub at: P2,
    /// Landmarks in sight in fog, and in ordinary weather.
    pub fog: Vec<String>,
    pub clear: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Audit {
    pub map: String,
    /// Grid side and origin: cell `(i, j)` is at `origin + (i, j) * STEP`.
    pub n: usize,
    pub origin: f32,
    /// Distance to the nearest landmark in sight (within `CLEAR_SIGHT`),
    /// per cell; `INFINITY` where none is, `NaN` where you can't stand.
    pub nearest: Vec<f32>,
    pub viewpoints: Vec<Viewpoint>,
    /// Per region: share of its walkable ground with no landmark in sight
    /// in fog, and in ordinary weather.
    pub regions: Vec<(String, f32, f32)>,
    /// Share of all walkable ground blind in fog, and in ordinary weather.
    pub blind_fog: f32,
    pub blind_clear: f32,
}

impl Audit {
    pub fn text(&self) -> String {
        let mut s = format!(
            "== {} sightlines ==\n  blind in fog {:.0}% of the valley, in drizzle {:.0}%\n",
            self.map,
            self.blind_fog * 100.0,
            self.blind_clear * 100.0
        );
        for (name, fog, clear) in &self.regions {
            let _ = writeln!(
                s,
                "  region {name}: blind {:.0}% in fog, {:.0}% in drizzle",
                fog * 100.0,
                clear * 100.0
            );
        }
        for v in &self.viewpoints {
            let _ = writeln!(
                s,
                "  from {} ({:.0}, {:.0}): fog [{}], drizzle [{}]",
                v.name,
                v.at[0],
                v.at[1],
                v.fog.join(", "),
                v.clear.join(", ")
            );
        }
        s
    }

    /// The grid as JSON for the preview: `{n, origin, step, nearest: [...]}`
    /// (`null` where you can't stand, `-1` where nothing is in sight).
    pub fn json(&self) -> String {
        let cells: Vec<String> = self
            .nearest
            .iter()
            .map(|d| {
                if d.is_nan() {
                    "null".into()
                } else if d.is_infinite() {
                    "-1".into()
                } else {
                    format!("{d:.0}")
                }
            })
            .collect();
        format!(
            "{{\"n\":{},\"origin\":{},\"step\":{},\"fog\":{},\"clear\":{},\"nearest\":[{}]}}",
            self.n,
            self.origin,
            STEP,
            FOG_SIGHT,
            CLEAR_SIGHT,
            cells.join(",")
        )
    }
}

fn standable(map: &MapDef, hm: &Heightmap, p: P2) -> bool {
    inside_fence(map, p) && hm.water_level(p).is_none() && hm.slope_deg(p) < 32.0
}

/// Local summits inside the fence: the highest ground within
/// [`HILL_RADIUS`] and [`HILL_RISE`] above its surroundings.
pub fn hilltops(map: &MapDef, hm: &Heightmap) -> Vec<P2> {
    let half = map.half() - map.fence_inset;
    let mut out: Vec<P2> = Vec::new();
    let mut z = -half;
    while z <= half {
        let mut x = -half;
        while x <= half {
            let p = [x, z];
            let h = hm.height(p);
            let ring: Vec<f32> = (0..16)
                .map(|k| {
                    let a = k as f32 / 16.0 * std::f32::consts::TAU;
                    [x + a.cos() * HILL_RADIUS, z + a.sin() * HILL_RADIUS]
                })
                .map(|q| hm.height(q))
                .collect();
            let inner_higher = (1..=4).any(|r| {
                let rr = r as f32 * HILL_RADIUS / 4.0;
                (0..12).any(|k| {
                    let a = k as f32 / 12.0 * std::f32::consts::TAU;
                    hm.height([x + a.cos() * rr, z + a.sin() * rr]) > h + 0.2
                })
            });
            let low = ring.iter().cloned().fold(f32::INFINITY, f32::min);
            if !inner_higher
                && h - low >= HILL_RISE
                && standable(map, hm, p)
                && out.iter().all(|q| dist(*q, p) > HILL_RADIUS)
            {
                out.push(p);
            }
            x += 10.0;
        }
        z += 10.0;
    }
    out
}

/// The audit of a map as built from `hm`.
pub fn audit(map: &MapDef, hm: &Heightmap) -> Audit {
    let marks = crate::map_landmarks::landmarks(map, hm);
    let half = map.half() - map.fence_inset;
    let n = (2.0 * half / STEP).floor() as usize + 1;
    let origin = -half;
    let mut nearest = vec![f32::NAN; n * n];
    let seen_from = |p: P2, range: f32| crate::map_landmarks::seen_from(&marks, hm, p, range);
    for j in 0..n {
        for i in 0..n {
            let p = [origin + i as f32 * STEP, origin + j as f32 * STEP];
            if !standable(map, hm, p) {
                continue;
            }
            nearest[j * n + i] = seen_from(p, CLEAR_SIGHT)
                .first()
                .map_or(f32::INFINITY, |s| s.1);
        }
    }
    let cells: Vec<(P2, f32)> = (0..n * n)
        .filter(|k| !nearest[*k].is_nan())
        .map(|k| {
            (
                [
                    origin + (k % n) as f32 * STEP,
                    origin + (k / n) as f32 * STEP,
                ],
                nearest[k],
            )
        })
        .collect();
    let blind = |cs: &[&(P2, f32)], range: f32| {
        cs.iter().filter(|c| c.1 > range).count() as f32 / cs.len().max(1) as f32
    };
    let all: Vec<&(P2, f32)> = cells.iter().collect();
    let regions = map
        .regions
        .iter()
        .map(|r| {
            let cs: Vec<&(P2, f32)> = cells
                .iter()
                .filter(|c| crate::geom::point_in_polygon(&r.polygon, c.0))
                .collect();
            (
                r.name.clone(),
                blind(&cs, FOG_SIGHT),
                blind(&cs, CLEAR_SIGHT),
            )
        })
        .collect();
    let mut viewpoints = Vec::new();
    let names = |v: Vec<(String, f32)>| {
        v.into_iter()
            .map(|(name, d)| format!("{name} {d:.0} m"))
            .collect::<Vec<_>>()
    };
    for m in &map.markers {
        let label = match m.kind {
            MarkerKind::Insertion => "insertion",
            MarkerKind::Rendezvous => "rendezvous",
            _ => continue,
        };
        viewpoints.push(Viewpoint {
            name: format!("{label} {}", m.name),
            at: m.at,
            fog: names(seen_from(m.at, FOG_SIGHT)),
            clear: names(seen_from(m.at, CLEAR_SIGHT)),
        });
    }
    for h in hilltops(map, hm) {
        viewpoints.push(Viewpoint {
            name: format!("hilltop {:.0} m", hm.height(h)),
            at: h,
            fog: names(seen_from(h, FOG_SIGHT)),
            clear: names(seen_from(h, CLEAR_SIGHT)),
        });
    }
    Audit {
        map: map.name.clone(),
        n,
        origin,
        nearest,
        viewpoints,
        regions,
        blind_fog: blind(&all, FOG_SIGHT),
        blind_clear: blind(&all, CLEAR_SIGHT),
    }
}

/// Where a new landmark `height` tall would do the most good: candidate
/// spots every [`SITE_STEP`] metres on open, standable ground (clear of
/// buildings, roads and water), ranked greedily by how much ground that is
/// blind in fog would see it within `FOG_SIGHT`. Returns up to `k` sites
/// with the share of the valley's walkable ground each one newly covers.
pub fn hero_sites(
    map: &MapDef,
    hm: &Heightmap,
    a: &Audit,
    height: f32,
    k: usize,
) -> Vec<(P2, f32)> {
    let total = a.nearest.iter().filter(|d| !d.is_nan()).count().max(1) as f32;
    let mut blind: Vec<P2> = (0..a.n * a.n)
        .filter(|i| {
            let d = a.nearest[*i];
            !d.is_nan() && d > FOG_SIGHT
        })
        .map(|i| {
            [
                a.origin + (i % a.n) as f32 * STEP,
                a.origin + (i / a.n) as f32 * STEP,
            ]
        })
        .collect();
    let half = map.half() - map.fence_inset;
    let mut candidates = Vec::new();
    let mut z = -half;
    while z <= half {
        let mut x = -half;
        while x <= half {
            let p = [x, z];
            let clear = standable(map, hm, p)
                && hm.road_at(p).kind.is_none()
                && map.placements.iter().all(|q| {
                    let f = prefab_footprint(&q.prefab);
                    dist(q.at, p) > f[0].max(f[2]) * 0.6 + 8.0
                });
            if clear {
                candidates.push(p);
            }
            x += SITE_STEP;
        }
        z += SITE_STEP;
    }
    let mut out = Vec::new();
    for _ in 0..k {
        let best = candidates
            .iter()
            .map(|c| {
                let n = blind
                    .iter()
                    .filter(|b| dist(**b, *c) <= FOG_SIGHT && sees(hm, **b, *c, height))
                    .count();
                (n, *c)
            })
            .max_by_key(|x| x.0);
        let Some((n, c)) = best else { break };
        if n == 0 {
            break;
        }
        out.push((c, n as f32 / total));
        blind.retain(|b| !(dist(*b, c) <= FOG_SIGHT && sees(hm, *b, c, height)));
    }
    out
}

/// Spacing of candidate landmark sites, metres.
pub const SITE_STEP: f32 = 20.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints each valley's audit; holds only that the audit runs and
    /// finds its viewpoints (the thresholds come when every valley has its
    /// hero landmark).
    #[test]
    fn sightline_audit() {
        for text in crate::map::VALLEYS.map(|v| v.1) {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let a = audit(&map, &hm);
            println!("{}", a.text());
            for (c, share) in hero_sites(&map, &hm, &a, 20.0, 3) {
                println!(
                    "  a 20 m landmark at ({:.0}, {:.0}) would end the fog blindness of {:.0}% of the valley",
                    c[0],
                    c[1],
                    share * 100.0
                );
            }
            assert!(a.viewpoints.len() >= 4, "{}", a.text());
            assert!(a.blind_fog >= a.blind_clear);
        }
    }

    #[test]
    fn a_tower_on_a_plain_is_seen_from_round_it() {
        let mut map = MapDef::from_ron(crate::map::TRAINING_YARD).unwrap();
        map.placements.push(crate::map::Placement {
            prefab: "radio_mast".into(),
            at: [0.0, 0.0],
            rot: 0.0,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
        let hm = Heightmap::build(&map);
        let a = audit(&map, &hm);
        let k = |x: f32, z: f32| {
            let i = ((x - a.origin) / STEP).round() as usize;
            let j = ((z - a.origin) / STEP).round() as usize;
            a.nearest[j * a.n + i]
        };
        assert!(k(30.0, 0.0) < 40.0, "{}", k(30.0, 0.0));
        assert!(a.blind_fog < 0.2, "{}", a.text());
    }
}
