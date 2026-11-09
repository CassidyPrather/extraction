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
/// It must read from the approach, from this far short of the line (m)
/// back to [`READ_FROM`]: what a newcomer sees coming up to it, not only
/// once they're at it.
pub const SHORT: f32 = 8.0;
pub const READ_FROM: f32 = APPROACH;
/// And it must be ahead, within this of the way's heading where it's
/// read from (degrees): in the frame walking on, not over a shoulder. A
/// tell beside the way short of the line reads from further back, where
/// it's ahead; the heading, not a narrow window, keeps out what's only
/// passed.
pub const AHEAD: f32 = 35.0;
/// How much quiet ground a way must have crossed before the line for it
/// to be a way out of the quiet (m): the approach frame's 20 m. Less, and
/// it's a gap between hot places, walked hot to hot.
pub const QUIET_RUN: f32 = 20.0;
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

/// How far a sentry's day marker reads (m): its full sight
/// (`rules::SIGHT`), against a slag heap or a grey building, as Robot
/// bodies measured it; dark when it has no power.
pub const SENTRY_READ: f32 = 55.0;

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
    crossings_with(map, hm, comfy, routes, false)
}

/// As [`crossings`], after dark: a tell counts only if it gives its own
/// light (a sentry's searchlight, a steady anomaly's glow, a robot gate's
/// red lamp), read as a glow within its reach, or if a light to steer by
/// (`map_night::lights`) stands within [`LIT_BY`] of it, read as by day
/// but no further than [`LIT_READ`].
pub fn crossings_at_night(
    map: &MapDef,
    hm: &Heightmap,
    comfy: &NavGrid,
    routes: &[Route],
) -> Vec<Crossing> {
    crossings_with(map, hm, comfy, routes, true)
}

/// How near the line a landmark's red lamp must stand, outside the hot
/// ground it tells of, to be that place's own at night (m): a lamp on the
/// skyline beyond is how a walker finds home, not a warning (coordinator).
pub const OWN_LAMP: f32 = 25.0;
/// How far from the reading point a place's own red lamp still tells of
/// it at night (m): further, it is skyline, whatever ground it stands in
/// (coordinator): the lamps far off are how a walker finds home.
pub const OWN_LAMP_READ: f32 = 100.0;
/// The placed props that light themselves after dark, as the game draws
/// them: prefab, and how far its light reads (m). A prop counts as a glow
/// at night only when it is here, and whoever lights it in the renderer
/// adds it here with a render test holding the two together (as
/// `map_night::LAMP_SEATS` is held), so the gauge can't count a lamp the
/// game doesn't light. `machine_marker`: its orange lens and held halo
/// (`render::way_lamps`), read from 25 m in a downpour.
pub const SELF_LIT: &[(&str, f32)] = &[("machine_marker", 25.0)];
/// How near a light an unlit tell must stand to be seen by it at night (m).
pub const LIT_BY: f32 = 8.0;
/// How far a tell seen only by a lamp beside it reads at night (m).
pub const LIT_READ: f32 = 25.0;
/// The anomalies that glow steady after dark (`anomaly::visuals`); the
/// rest show only when something is thrown in.
pub const GLOWING: &[&str] = &["Burner", "Snare", "Well", "Frost"];

fn crossings_with(
    map: &MapDef,
    hm: &Heightmap,
    comfy: &NavGrid,
    routes: &[Route],
    night: bool,
) -> Vec<Crossing> {
    let solids = solids(map);
    let marks = crate::map_landmarks::landmarks(map, hm);
    // Every thing that can tell: kind, name, where, how tall, how far it
    // carries, and how big it looks (the square root of its height times
    // its narrowest breadth, as `map_breadcrumbs` reads a cue).
    let mut tells: Vec<(&'static str, String, P2, f32, f32, f32)> = Vec::new();
    // At night, the ones that give their own light (read as a glow).
    let mut glows: Vec<P2> = Vec::new();
    for m in &map.markers {
        match &m.kind {
            MarkerKind::Sign { text, .. } => tells.push((
                "sign",
                text.lines().next().unwrap_or("").chars().take(40).collect(),
                m.at,
                1.5,
                30.0,
                1.1,
            )),
            MarkerKind::Sentry { .. } => {
                if night {
                    glows.push(m.at);
                }
                let reach = if night {
                    crate::sentry::BEAM_REACH
                } else {
                    90.0
                };
                tells.push(("sentry", m.name.clone(), m.at, 2.0, reach, 1.6))
            }
            MarkerKind::RobotGate => {
                if night {
                    glows.push(m.at);
                }
                let reach = if night { 25.0 } else { 120.0 };
                tells.push(("robot gate", m.name.clone(), m.at, 3.0, reach, 3.0))
            }
            MarkerKind::Anomaly { kind, .. } => {
                if night && GLOWING.contains(&kind.as_str()) {
                    glows.push(m.at);
                }
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
        } else if p.prefab == "machine_marker" {
            // The machines' own boundary stake: small by day, its amber
            // lens a glow after dark once it is lit ([`SELF_LIT`]).
            if night && SELF_LIT.iter().any(|(id, _)| *id == p.prefab) {
                glows.push(p.at);
            }
            "machine marker"
        } else {
            continue;
        };
        let f = prefab_footprint(&p.prefab);
        let reach = match SELF_LIT.iter().find(|(id, _)| *id == p.prefab) {
            Some((_, r)) if night => *r,
            _ => 80.0,
        };
        tells.push((
            kind,
            p.prefab.clone(),
            p.at,
            h.max(1.5),
            reach,
            (h * f[0].min(f[2])).sqrt(),
        ));
    }
    // After dark, an unlit tell needs a lamp beside it, and reads only as
    // far as that shows it; one that gives its own light glows.
    if night {
        let lights = crate::map_night::lights(map, hm);
        tells.retain_mut(|t| {
            if glows.contains(&t.2) {
                return true;
            }
            t.4 = t.4.min(LIT_READ);
            lights.iter().any(|l| dist(l.at, t.2) <= LIT_BY)
        });
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
            // The approach: back along the way from the line, on quiet
            // ground (it ends where the way was last in hot ground).
            let mut approach = Vec::new();
            let mut s = 0.0;
            let mut j = i + 1;
            while j > 0 && s <= APPROACH {
                approach.push(pts[j]);
                if danger(map, pts[j - 1]).1 >= HOT {
                    break;
                }
                s += dist(pts[j], pts[j - 1]);
                j -= 1;
            }
            // A gap of quiet ground narrower than the approach frame
            // between hot places isn't a way out of the quiet: no crossing.
            if s < QUIET_RUN {
                continue;
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
            // Each reading point with the way's heading there (toward the
            // line: the point before it in `approach`, or the line itself).
            let approach: Vec<(P2, P2)> = (0..approach.len())
                .step_by(3)
                .map(|k| {
                    let at = approach[k];
                    let to = if k == 0 { w[1] } else { approach[k - 1] };
                    let to = if dist(at, to) < 0.1 { w[1] } else { to };
                    let d = dist(at, to).max(0.1);
                    (at, [(to[0] - at[0]) / d, (to[1] - at[1]) / d])
                })
                .collect();
            let ahead = |p: P2, h: P2, at: P2| {
                let d = dist(p, at);
                d < 1.0
                    || (h[0] * (at[0] - p[0]) + h[1] * (at[1] - p[1])) / d
                        >= AHEAD.to_radians().cos()
            };
            let mut signals = Vec::new();
            for (kind, name, at, h, reach, size) in &tells {
                // Its own: in the hot ground being entered, or by the line.
                let here = danger(map, *at).0 == b.0 || dist(*at, w[1]) <= BY_THE_LINE;
                if !here {
                    continue;
                }
                let near = approach
                    .iter()
                    .filter(|(p, h)| ahead(*p, *h, *at))
                    .map(|(p, _)| (dist(*p, *at), *p))
                    .filter(|(d, _)| d <= reach)
                    // Found in the frame without a caption: big enough at
                    // its distance (the lamps aside: they glow), from a
                    // point on the approach short of the line.
                    .filter(|(d, p)| {
                        *kind == "red lamp over it"
                            || glows.contains(at)
                            || (*size / d.max(1.0) >= READ_ANGLE
                                && (SHORT..=READ_FROM).contains(&dist(*p, w[1])))
                    })
                    .filter(|(_, p)| in_sight(hm, &solids, *p, *at, *h))
                    .map(|(d, _)| d)
                    .fold(f32::INFINITY, f32::min);
                if near.is_finite() {
                    // After dark a landmark's red lamp is the place's own
                    // tell when its landmark is part of the hot place: in
                    // the ground being entered, or by the line.
                    let own = night
                        && *kind == "red lamp over it"
                        && near <= OWN_LAMP_READ
                        && (danger(map, *at).0 == b.0 || dist(*at, w[1]) <= OWN_LAMP);
                    let kind = if own { "its own red lamp" } else { *kind };
                    signals.push((kind, name.clone(), near));
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
            let cs = if std::env::var("NIGHT").is_ok() {
                crossings_at_night(&map, &hm, &comfy, &r.routes)
            } else {
                crossings(&map, &hm, &comfy, &r.routes)
            };
            // One line per place, cell and way (two ways crossing in one
            // cell from different directions each get a line).
            let mut seen: Vec<(String, (i32, i32), String)> = Vec::new();
            println!("== {id}: {} crossings", cs.len());
            for c in &cs {
                let key = (
                    c.into.0.clone(),
                    ((c.at[0] / 20.0) as i32, (c.at[1] / 20.0) as i32),
                    c.way.clone(),
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

/// Where a walked way first comes into a sentry's sight with its lamp
/// not read first: not seen from that point or one before it on the way,
/// ahead (within [`AHEAD`] of the way's heading), within [`SENTRY_READ`]
/// and in sight. A machine must be readable at least as far out as it can
/// see you. At a way's start (a job, a stop) the walker stands and looks
/// round, so any heading reads there. The sightline is the sentry's own
/// the other way, its head to the walker's eye: the lamp is on its head.
#[derive(Clone, Debug)]
pub struct Unread {
    pub sentry: String,
    pub way: String,
    pub at: P2,
    /// How far off it stands (m), and how far off the walker's heading
    /// (degrees).
    pub off: f32,
    pub bearing: f32,
}

pub fn unread_sentries(
    map: &MapDef,
    hm: &Heightmap,
    comfy: &NavGrid,
    routes: &[Route],
) -> Vec<Unread> {
    unread_sentries_at(map, hm, comfy, routes, crate::sentry::SIGHT, SENTRY_READ)
}

/// As [`unread_sentries`] at another hour: it sees no further than `sight`
/// (`world::weather::Conditions::sight` on its 55 m: about 37 m at dawn,
/// 33 m at dusk, 22 m at night, any weather) and its lamp reads to `read`
/// (its searchlight's [`crate::sentry::BEAM_REACH`] from dusk to dawn).
pub fn unread_sentries_at(
    map: &MapDef,
    hm: &Heightmap,
    comfy: &NavGrid,
    routes: &[Route],
    sight: f32,
    read: f32,
) -> Vec<Unread> {
    let blocks = crate::map_sentry::blocks(map, hm);
    let ways = routed_ways(map, comfy, routes);
    let heading = |pts: &[P2], k: usize| {
        let (q, m) = (pts[k], pts[(k + 3).min(pts.len() - 1)]);
        let d = dist(q, m).max(0.1);
        [(m[0] - q[0]) / d, (m[1] - q[1]) / d]
    };
    let mut out = Vec::new();
    for s in crate::map_sentry::sentries(map) {
        let lamp = (s.at, s.head(hm));
        let reads = |q: P2, h: P2, start: bool| {
            let d = dist(q, s.at);
            d <= read
                && (d < 1.0
                    || start
                    || (h[0] * (s.at[0] - q[0]) + h[1] * (s.at[1] - q[1])) / d
                        >= AHEAD.to_radians().cos())
                && crate::map_sentry::clear(
                    hm,
                    &crate::map_sentry::near_line(&blocks, s.at, q),
                    lamp,
                    (q, hm.height(q) + 1.7),
                )
        };
        for (way, pts) in &ways {
            let mut seen = false;
            for i in 0..pts.len().saturating_sub(1) {
                let now = s.sees(map, hm, pts[i]) && dist(pts[i], s.at) <= sight;
                if now && !seen && !(0..=i).any(|k| reads(pts[k], heading(pts, k), k == 0)) {
                    let (p, h) = (pts[i], heading(pts, i));
                    let off = dist(p, s.at).max(0.1);
                    let c = (h[0] * (s.at[0] - p[0]) + h[1] * (s.at[1] - p[1])) / off;
                    out.push(Unread {
                        sentry: s.name.clone(),
                        way: way.clone(),
                        at: p,
                        off,
                        bearing: c.clamp(-1.0, 1.0).acos().to_degrees(),
                    });
                }
                seen = now;
            }
        }
    }
    out
}

#[cfg(test)]
mod sentries {
    use super::*;

    /// Every sentry's lamp is read before it first sees you on a walked
    /// way, in every valley.
    #[test]
    fn every_sentry_reads_before_it_sees_you() {
        let bad: Vec<String> = std::thread::scope(|sc| {
            let jobs: Vec<_> = crate::map::VALLEYS
                .iter()
                .map(|(id, text)| {
                    sc.spawn(move || {
                        let map = MapDef::from_ron(text).unwrap();
                        let hm = Heightmap::build(&map);
                        let comfy = crate::map_nav::zone_grid(
                            &map,
                            &hm,
                            crate::map_lint::COMFORT_SLOPE,
                        );
                        let r = crate::map_lint::lint(&map, &hm);
                        let at = |sight, read| unread_sentries_at(&map, &hm, &comfy, &r.routes, sight, read);
                        let mut all = at(crate::sentry::SIGHT, SENTRY_READ);
                        for (hour, sight) in [("dawn", 36.9), ("dusk", 33.0), ("night", 22.0)] {
                            for mut u in at(sight, crate::sentry::BEAM_REACH) {
                                u.way = format!("{} (at {hour})", u.way);
                                all.push(u);
                            }
                        }
                        all.into_iter()
                            .map(|u| {
                                format!(
                                    "{id}: {} first sees {} at ({:.0}, {:.0}), {:.0} m off and {:.0} deg off the heading, its lamp not read first",
                                    u.sentry, u.way, u.at[0], u.at[1], u.off, u.bearing
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
        });
        assert!(
            bad.is_empty(),
            "sentries that see you before you can see them:\n{}",
            bad.join("\n")
        );
    }
}

#[cfg(test)]
mod shift_probe {
    use crate::geom::{P2, dist};
    use crate::map::MapDef;
    use crate::terrain::Heightmap;

    fn off(p: P2, line: &[P2]) -> f32 {
        crate::geom::polyline_closest(line, p).0
    }

    /// How far each walked way moves with the valley's wrecks taken out,
    /// at its worst point, and the wreck nearest there: a wreck placed as
    /// a tell should leave the way where it was.
    #[test]
    #[ignore]
    fn probe_way_shift() {
        for (id, text) in crate::map::VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let mut bare = map.clone();
            bare.placements.retain(|p| !p.prefab.starts_with("wreck_"));
            let ways = |m: &MapDef| {
                let hm = Heightmap::build(m);
                let comfy = crate::map_nav::zone_grid(m, &hm, crate::map_lint::COMFORT_SLOPE);
                let r = crate::map_lint::lint(m, &hm);
                crate::map_lint::routed_ways(m, &comfy, &r.routes)
            };
            let (with, without) = (ways(&map), ways(&bare));
            let mut moved = 0;
            for (way, pts) in &with {
                let Some((_, b)) = without.iter().find(|(w, _)| w == way) else {
                    continue;
                };
                let Some((d, at)) = pts
                    .iter()
                    .map(|p| (off(*p, b), *p))
                    .max_by(|a, c| a.0.total_cmp(&c.0))
                else {
                    continue;
                };
                if d > 2.0 {
                    moved += 1;
                    let near = map
                        .placements
                        .iter()
                        .filter(|p| p.prefab.starts_with("wreck_"))
                        .min_by(|a, c| dist(a.at, at).total_cmp(&dist(c.at, at)))
                        .map(|p| {
                            format!(
                                "{} ({:.1}, {:.1}) {:.0} m off",
                                p.prefab,
                                p.at[0],
                                p.at[1],
                                dist(p.at, at)
                            )
                        })
                        .unwrap_or_default();
                    println!(
                        "  {id} {way}: {d:.1} m at ({:.0}, {:.0}); nearest wreck {near}",
                        at[0], at[1]
                    );
                }
            }
            println!("== {id}: {} ways, {moved} moved over 2 m", with.len());
        }
    }
}

#[cfg(test)]
mod night {
    use super::*;

    /// Every way from quiet ground into hot is told by its place after dark
    /// as well as by day (`crossings_at_night`): a light of its own (a
    /// sentry's searchlight, a glowing anomaly, a lit machine stake), a
    /// lamp beside an unlit tell, or the place's own red lamp within
    /// [`OWN_LAMP_READ`]. Without it a way moved a few metres leaves a
    /// crossing dark and nothing says so. Known gaps are named here till
    /// they're closed (valley, the place, near, part of the way's name,
    /// whose), and this fails once one is: take it off.
    #[test]
    fn every_quiet_to_hot_crossing_is_told_at_night() {
        const GAPS: &[(&str, &str, P2, &str, &str)] = &[];
        let found: Vec<(Vec<String>, Vec<String>)> = std::thread::scope(|sc| {
            let jobs: Vec<_> = crate::map::VALLEYS
                .iter()
                .map(|(id, text)| {
                    sc.spawn(move || {
                        let map = MapDef::from_ron(text).unwrap();
                        let hm = Heightmap::build(&map);
                        let comfy =
                            crate::map_nav::zone_grid(&map, &hm, crate::map_lint::COMFORT_SLOPE);
                        let r = crate::map_lint::lint(&map, &hm);
                        let (mut untold, mut closed) = (Vec::new(), Vec::new());
                        for c in crossings_at_night(&map, &hm, &comfy, &r.routes) {
                            let told = c.signals.iter().any(|s| s.0 != "red lamp over it");
                            let gap = GAPS.iter().find(|g| {
                                g.0 == *id
                                    && g.1 == c.into.0
                                    && c.way.contains(g.3)
                                    && dist(g.2, c.at) < 25.0
                            });
                            match (told, gap) {
                                (false, None) => untold.push(format!(
                                    "{id}: {} -> {} at ({:.0}, {:.0}) on {}",
                                    c.from.0, c.into.0, c.at[0], c.at[1], c.way
                                )),
                                (true, Some(g)) => {
                                    closed.push(format!("{id} {} on {} ({})", g.1, g.3, g.4))
                                }
                                _ => {}
                            }
                        }
                        (untold, closed)
                    })
                })
                .collect();
            jobs.into_iter().map(|j| j.join().unwrap()).collect()
        });
        let untold: Vec<_> = found.iter().flat_map(|f| f.0.clone()).collect();
        let mut closed: Vec<_> = found.iter().flat_map(|f| f.1.clone()).collect();
        closed.dedup();
        assert!(
            untold.is_empty(),
            "crossings nothing tells of after dark: {untold:#?}"
        );
        assert!(
            closed.is_empty(),
            "told after dark now, take it off GAPS: {closed:?}"
        );
    }
}
