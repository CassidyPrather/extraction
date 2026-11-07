//! The danger gradient on the walked ways: where a way goes from quiet
//! ground (a region of danger 0 or 1, or none) into hot (2 or 3), what the
//! world says about it on the way up to the line, from the last
//! [`APPROACH`] metres on the quiet side: a sign, a wreck, a gate or guard
//! booth, a sentry, an anomaly, a red-lit landmark over the hot ground.
//! A crossing with none of them is a newcomer walking from quiet into hot
//! without being told. A probe for now (`probe_gradient`), not a rule.

use crate::geom::{P2, dist};
use crate::map::{MapDef, MarkerKind, prefab_footprint};
use crate::map_lint::{Route, Solid, routed_ways, sees, solids};
use crate::nav::NavGrid;
use crate::terrain::Heightmap;

/// How far back along the way from the line the approach is (m).
pub const APPROACH: f32 = 60.0;
/// The danger from which ground is hot.
pub const HOT: u8 = 2;
/// A tell reads when it spans this much of the eye (radians), as a cue
/// does off the way (`map_breadcrumbs::READ_ANGLE`); not the skyline's
/// smaller allowance: a tell is found in the frame without a caption.
pub const READ_ANGLE: f32 = crate::map_breadcrumbs::READ_ANGLE;
/// It must read from the last stretch before the line, this far short of
/// it (m): what a newcomer sees coming up to it, the approach frame's
/// 20 m among it; not from far back along the way, past something else.
pub const SHORT: f32 = 8.0;
pub const READ_FROM: f32 = 30.0;
/// How near the line a thing outside the hot ground still tells of it (m).
pub const BY_THE_LINE: f32 = 40.0;

#[derive(Clone, Debug)]
pub struct Crossing {
    pub way: String,
    pub at: P2,
    pub from: (String, u8),
    pub into: (String, u8),
    /// What tells of it on the approach: kind, name, how far from the
    /// nearest approach point that sees it.
    pub signals: Vec<(&'static str, String, f32)>,
    /// The way's point [`BACK`] metres back from the line, and the way's
    /// bearing there (a unit vector, toward the line): where a tell goes.
    pub back: (P2, P2),
}

/// How far back from the line a tell's spot is suggested (m).
pub const BACK: f32 = 15.0;

fn danger(map: &MapDef, p: P2) -> (String, u8) {
    map.region_at(p)
        .map_or(("open ground".into(), 0), |r| (r.name.clone(), r.danger))
}

/// Whether something `height` tall at `to` is in sight from `from`, over
/// the ground and past the buildings between (not its own).
fn in_sight(hm: &Heightmap, solids: &[Solid], from: P2, to: P2, height: f32) -> bool {
    if !sees(hm, from, to, height) {
        return false;
    }
    let d = dist(from, to);
    let n = d.ceil() as usize;
    let eye = hm.height(from) + 1.7;
    let top = hm.height(to) + height;
    solids
        .iter()
        .filter(|s| s.rect.sdf(to) > 3.0 && s.rect.sdf(from) > 0.5)
        .all(|s| {
            let h = prefab_footprint(&s.prefab)[1];
            (1..n).all(|i| {
                let t = i as f32 / n as f32;
                let p = crate::geom::lerp(from, to, t);
                s.rect.sdf(p) > 0.0 || hm.height(p) + h < eye + (top - eye) * t
            })
        })
}

pub fn crossings(map: &MapDef, hm: &Heightmap, comfy: &NavGrid, routes: &[Route]) -> Vec<Crossing> {
    let solids = solids(map);
    let marks = crate::map_landmarks::landmarks(map, hm);
    // Every thing that can tell: kind, name, where, how tall, how far it
    // carries, and how big it looks (the square root of its height times
    // its narrowest breadth, as `map_breadcrumbs` reads a cue).
    let mut tells: Vec<(&'static str, String, P2, f32, f32, f32)> = Vec::new();
    for m in &map.markers {
        match &m.kind {
            MarkerKind::Sign { text } => tells.push((
                "sign",
                text.lines().next().unwrap_or("").chars().take(40).collect(),
                m.at,
                1.5,
                30.0,
                1.1,
            )),
            MarkerKind::Sentry { .. } => {
                tells.push(("sentry", m.name.clone(), m.at, 2.0, 90.0, 1.6))
            }
            MarkerKind::RobotGate => {
                tells.push(("robot gate", m.name.clone(), m.at, 3.0, 120.0, 3.0))
            }
            MarkerKind::Anomaly { kind, .. } => {
                tells.push(("anomaly", kind.clone(), m.at, 1.0, 40.0, 2.0))
            }
            _ => {}
        }
    }
    for p in &map.placements {
        let h = prefab_footprint(&p.prefab)[1];
        let kind = if p.prefab.starts_with("wreck_") {
            "wreck"
        } else if matches!(
            p.prefab.as_str(),
            "robot_gate" | "quarantine_gate" | "guard_booth"
        ) {
            "gate"
        } else {
            continue;
        };
        let f = prefab_footprint(&p.prefab);
        tells.push((
            kind,
            p.prefab.clone(),
            p.at,
            h.max(1.5),
            80.0,
            (h * f[0].min(f[2])).sqrt(),
        ));
    }
    // A landmark's lamp over hot ground: red, far off.
    for l in &marks {
        for (top, y) in &l.tops {
            if danger(map, *top).1 >= HOT {
                let h = y - hm.height(*top);
                tells.push(("red lamp over it", l.name.clone(), *top, h, 250.0, 1.0));
            }
        }
    }
    let mut out = Vec::new();
    for (way, pts) in routed_ways(map, comfy, routes) {
        for (i, w) in pts.windows(2).enumerate() {
            let (a, b) = (danger(map, w[0]), danger(map, w[1]));
            if !(a.1 < HOT && b.1 >= HOT) {
                continue;
            }
            // The approach: back along the way from the line.
            let mut approach = Vec::new();
            let mut s = 0.0;
            let mut j = i + 1;
            while j > 0 && s <= APPROACH {
                approach.push(pts[j]);
                s += dist(pts[j], pts[j - 1]);
                j -= 1;
            }
            let back = {
                let mut k = 0;
                while k + 1 < approach.len() && dist(approach[k + 1], w[1]) < BACK {
                    k += 1;
                }
                let at = approach[(k + 1).min(approach.len() - 1)];
                let to = approach[k];
                let d = dist(at, to).max(0.1);
                (at, [(to[0] - at[0]) / d, (to[1] - at[1]) / d])
            };
            let approach: Vec<P2> = approach.into_iter().step_by(3).collect();
            let mut signals = Vec::new();
            for (kind, name, at, h, reach, size) in &tells {
                // Its own: in the hot ground being entered, or by the line.
                let here = danger(map, *at).0 == b.0 || dist(*at, w[1]) <= BY_THE_LINE;
                if !here {
                    continue;
                }
                let near = approach
                    .iter()
                    .map(|p| (dist(*p, *at), *p))
                    .filter(|(d, _)| d <= reach)
                    // Found in the frame without a caption: big enough at
                    // its distance (the lamps aside: they glow), from a
                    // point on the approach short of the line.
                    .filter(|(d, p)| {
                        *kind == "red lamp over it"
                            || (*size / d.max(1.0) >= READ_ANGLE
                                && (SHORT..=READ_FROM).contains(&dist(*p, w[1])))
                    })
                    .filter(|(_, p)| in_sight(hm, &solids, *p, *at, *h))
                    .map(|(d, _)| d)
                    .fold(f32::INFINITY, f32::min);
                if near.is_finite() {
                    signals.push((*kind, name.clone(), near));
                }
            }
            signals.sort_by(|x, y| x.2.total_cmp(&y.2));
            signals.dedup_by(|x, y| x.0 == y.0 && x.1 == y.1);
            out.push(Crossing {
                way: way.clone(),
                at: w[1],
                from: a,
                into: b,
                signals,
                back,
            });
        }
    }
    // One line per place a way crosses, the nearest crossings merged.
    out.dedup_by(|x, y| x.into.0 == y.into.0 && dist(x.at, y.at) < 25.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints every crossing from quiet into hot on the walked ways and
    /// what tells of it (`MAP_ID` for one valley).
    #[test]
    #[ignore]
    fn probe_gradient() {
        let only = std::env::var("MAP_ID").ok();
        for (id, text) in crate::map::VALLEYS {
            if only.as_deref().is_some_and(|o| o != id) {
                continue;
            }
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let comfy = crate::map_nav::zone_grid(&map, &hm, crate::map_lint::COMFORT_SLOPE);
            let r = crate::map_lint::lint(&map, &hm);
            let cs = crossings(&map, &hm, &comfy, &r.routes);
            let mut seen: Vec<(String, (i32, i32))> = Vec::new();
            println!("== {id}: {} crossings", cs.len());
            for c in &cs {
                let key = (
                    c.into.0.clone(),
                    ((c.at[0] / 20.0) as i32, (c.at[1] / 20.0) as i32),
                );
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                let strong: Vec<_> = c
                    .signals
                    .iter()
                    .filter(|s| s.0 != "red lamp over it")
                    .collect();
                let lamps: Vec<_> = c
                    .signals
                    .iter()
                    .filter(|s| s.0 == "red lamp over it")
                    .map(|s| format!("{} ({:.0} m)", s.1, s.2))
                    .collect();
                let tell = if strong.is_empty() {
                    "NOTHING TELLS".to_string()
                } else {
                    strong
                        .iter()
                        .map(|(k, n, d)| format!("{k} {n} ({d:.0} m)"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let (p, d) = c.back;
                let side = |k: f32| [p[0] - d[1] * k, p[1] + d[0] * k];
                let (l, r) = (side(4.0), side(-4.0));
                let tell = format!(
                    "{tell}  | {BACK:.0} m back ({:.1}, {:.1}): beside ({:.1}, {:.1}) or ({:.1}, {:.1}), facing {:.0}",
                    p[0],
                    p[1],
                    l[0],
                    l[1],
                    r[0],
                    r[1],
                    d[0].atan2(-d[1]).to_degrees().rem_euclid(360.0)
                );
                let tell = format!(
                    "{tell}  | its lamps: {}",
                    if lamps.is_empty() {
                        "none".into()
                    } else {
                        lamps.join(", ")
                    }
                );
                println!(
                    "  {} ({}) -> {} ({}) at ({:.0}, {:.0}) on {}: {tell}",
                    c.from.0, c.from.1, c.into.0, c.into.1, c.at[0], c.at[1], c.way
                );
            }
        }
    }
}

#[cfg(test)]
mod edges {
    use crate::map::{MapDef, MarkerKind};

    /// Where the valley's own edges stand: insertion, truck stops,
    /// rendezvous and first jobs, each with its region and danger.
    #[test]
    #[ignore]
    fn probe_edges() {
        for (id, text) in crate::map::VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let d = |p: [f32; 2]| {
                map.region_at(p).map_or("open ground (0)".to_string(), |r| {
                    format!("{} ({})", r.name, r.danger)
                })
            };
            let signs = map
                .markers
                .iter()
                .filter(|m| matches!(m.kind, MarkerKind::Sign { .. }))
                .count();
            println!("== {id}: {signs} signs");
            for m in &map.markers {
                let k = match m.kind {
                    MarkerKind::Insertion => "insertion",
                    MarkerKind::TruckStop => "truck stop",
                    MarkerKind::Rendezvous => "rendezvous",
                    _ => continue,
                };
                println!("  {k} {}: {}", m.name, d(m.at));
            }
            for (what, at) in crate::map_lint::first_jobs(&map, id) {
                println!("  first job {what}: {}", d(at));
            }
        }
    }
}
