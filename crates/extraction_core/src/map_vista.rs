//! The first frame: off the truck, before a step is taken, the first job's
//! place or a landmark on the way to it stands in the frame, in sight and
//! big enough to read. A newcomer's first look should hold somewhere to go
//! (walk review, `docs/craft/WALK_REVIEW.md`; Kettle's Gate woods hid the
//! Logging camp 70 m off the truck until a ride was cut through them).
//!
//! The frame is the way to the first job's heading at the truck, `AHEAD`
//! either side (as `map_gradient` reads a tell). Something counts when it
//! is within that, in sight over the ground, past the buildings and through
//! the trees (`map_landmarks::Canopy`, at the crowns' lowest breadth:
//! `LOW_CROWN`), and big enough for its distance: the
//! square root of its height times its narrowest breadth over the
//! distance, at least `READ_ANGLE`.
//!
//! Weather is not in it: Marrow's farm stands clear in the frame and is
//! faint in the drizzle. That is the sky's to fix, not the map's.

use crate::geom::{P2, dist};
use crate::map::{MapDef, MarkerKind, prefab_footprint};
use crate::map_gradient::{AHEAD, READ_ANGLE};
use crate::map_lint::{Solid, sees, solids};
use crate::terrain::Heightmap;

/// How far along the way to the first job its heading is taken (m): the
/// way's first steps, not the truck's own facing.
pub const HEADING_ALONG: f32 = 20.0;
/// A place's buildings: those standing this near its point (m).
pub const PLACE_REACH: f32 = 30.0;
/// A landmark is on the way when it stands this near it (m).
pub const ON_THE_WAY: f32 = 60.0;
/// Too small to be a building you'd steer for (m tall).
const BUILDING_HEIGHT: f32 = 2.0;
/// A spruce's crown at its widest, its lowest boughs, each way (m), narrowing
/// to nothing at its top (a cone). A first frame looks low, through the
/// bottoms of the crowns, where a tree is wider than `map_landmarks`' mean
/// crown: Kettle's Gate woods hid the sawmill behind four spruces 1.6-2.3 m
/// off the line, which the mean crown (2.2 m) took for a glimpse past one.
/// Higher up a crown is narrow: Marrow's ranger tower lamp shows between the
/// pines' tops, which a crown 3 m wide all the way up would have hidden.
pub const LOW_CROWN: f32 = 3.0;
/// The line passes through this many crowns: hidden.
pub const CROWNS_HIDE: usize = 2;

/// What the first frame holds, one valley.
#[derive(Clone, Debug)]
pub struct FirstFrame {
    pub insertion: String,
    pub at: P2,
    /// The way's heading off the truck (degrees, 0 north, clockwise).
    pub heading: f32,
    pub job: String,
    /// Everything that might be steered for, and why each does or doesn't
    /// read: (what, distance, degrees off the heading, verdict).
    pub seen: Vec<(String, f32, f32, &'static str)>,
}

impl FirstFrame {
    pub fn reads(&self) -> bool {
        self.seen.iter().any(|s| s.3 == "reads")
    }
}

fn bearing(from: P2, to: P2) -> f32 {
    (to[0] - from[0])
        .atan2(-(to[1] - from[1]))
        .to_degrees()
        .rem_euclid(360.0)
}

fn off(a: f32, b: f32) -> f32 {
    ((a - b + 540.0).rem_euclid(360.0) - 180.0).abs()
}

/// The point `along` metres down a polyline.
fn down(path: &[P2], along: f32) -> P2 {
    let mut left = along;
    for w in path.windows(2) {
        let l = dist(w[0], w[1]);
        if l >= left {
            return crate::geom::lerp(w[0], w[1], left / l.max(1e-6));
        }
        left -= l;
    }
    *path.last().unwrap_or(&[0.0, 0.0])
}

/// Whether a top `top` metres above sea at `to` is in sight from eye height
/// at `from`: over the ground, past every building but those at `to`'s
/// own place, and through the trees.
fn in_sight(
    hm: &Heightmap,
    solids: &[Solid],
    canopy: &crate::map_landmarks::Canopy,
    own: &[P2],
    from: P2,
    to: P2,
    top: f32,
) -> bool {
    let height = top - hm.height(to);
    if !sees(hm, from, to, height)
        || canopy.crowns(hm, from, to, top, LOW_CROWN, true, CROWNS_HIDE) >= CROWNS_HIDE
    {
        return false;
    }
    let d = dist(from, to);
    let n = d.ceil() as usize;
    let eye = hm.height(from) + 1.7;
    solids
        .iter()
        .filter(|s| s.rect.sdf(to) > 3.0 && s.rect.sdf(from) > 0.5)
        .filter(|s| own.iter().all(|o| s.rect.sdf(*o) > 0.5))
        .all(|s| {
            let h = prefab_footprint(&s.prefab)[1];
            (1..n).all(|i| {
                let t = i as f32 / n as f32;
                let p = crate::geom::lerp(from, to, t);
                s.rect.sdf(p) > 0.0 || hm.height(p) + h < eye + (top - eye) * t
            })
        })
}

/// The truck, the way's heading off it, the first job's place and the way
/// there, every 2 m.
struct Start {
    insertion: String,
    at: P2,
    heading: f32,
    job: P2,
    place: String,
    way: Vec<P2>,
}

fn start(id: &str, map: &MapDef, hm: &Heightmap) -> Option<Start> {
    let ins = map.markers_of(&MarkerKind::Insertion).next()?;
    let Some(crate::progression::Job::Scout { place, at: job, .. }) =
        crate::progression::first_job(id, map)
    else {
        return None;
    };
    let comfy = crate::map_nav::zone_grid(map, hm, crate::map_lint::COMFORT_SLOPE);
    let corners = comfy.path(ins.at, job)?;
    let heading = bearing(ins.at, down(&corners, HEADING_ALONG));
    // Every 2 m along it: a straight road is two corners a long way apart.
    let mut way = Vec::new();
    for w in corners.windows(2) {
        let n = (dist(w[0], w[1]) / 2.0).ceil().max(1.0) as usize;
        way.extend((0..n).map(|i| crate::geom::lerp(w[0], w[1], i as f32 / n as f32)));
    }
    way.extend(corners.last());
    Some(Start {
        insertion: ins.name.clone(),
        at: ins.at,
        heading,
        job,
        place,
        way,
    })
}

/// The first frame of a valley: from its truck, toward its first job.
pub fn first_frame(id: &str, map: &MapDef, hm: &Heightmap) -> Option<FirstFrame> {
    let Start {
        insertion,
        at,
        heading,
        job,
        place,
        way,
    } = start(id, map, hm)?;
    let solids = solids(map);
    let canopy = crate::map_landmarks::Canopy::new(map, hm);

    // (what, where its tops are, how big it looks)
    let mut things: Vec<(String, Vec<(P2, f32)>, f32)> = Vec::new();
    for p in &map.placements {
        let [w, h, l] = prefab_footprint(&p.prefab);
        if h < BUILDING_HEIGHT || dist(p.at, job) > PLACE_REACH {
            continue;
        }
        let top = hm.footing(&p.prefab, p.at, p.rot) + p.y + h;
        things.push((
            format!("{place}: {}", p.prefab),
            vec![(p.at, top)],
            (h * w.min(l)).sqrt(),
        ));
    }
    for m in crate::map_landmarks::landmarks(map, hm) {
        let near_way = m
            .tops
            .iter()
            .any(|(at, _)| way.iter().any(|q| dist(*q, *at) <= ON_THE_WAY));
        if !near_way {
            continue;
        }
        let size = m
            .pieces
            .iter()
            .map(|&i| {
                let [w, h, l] = prefab_footprint(&map.placements[i].prefab);
                (h * w.min(l)).sqrt()
            })
            .fold(0.0, f32::max);
        things.push((format!("landmark {}", m.name), m.tops.clone(), size));
    }

    let mut seen = Vec::new();
    for (what, tops, size) in things {
        let own: Vec<P2> = tops.iter().map(|t| t.0).collect();
        // Its nearest top that reads, else why the nearest doesn't.
        let mut best: Option<(f32, f32, &'static str)> = None;
        for (to, top) in &tops {
            let d = dist(at, *to);
            let o = off(bearing(at, *to), heading);
            let verdict = if o > AHEAD {
                "out of frame"
            } else if size / d.max(1.0) < READ_ANGLE {
                "too small"
            } else if !in_sight(hm, &solids, &canopy, &own, at, *to, *top) {
                "hidden"
            } else {
                "reads"
            };
            let better = match best {
                None => true,
                Some((bd, _, bv)) => {
                    (verdict == "reads" && bv != "reads") || (verdict == bv && d < bd)
                }
            };
            if better {
                best = Some((d, o, verdict));
            }
        }
        if let Some((d, o, v)) = best {
            seen.push((what, d, o, v));
        }
    }
    seen.sort_by(|a, b| a.1.total_cmp(&b.1));
    Some(FirstFrame {
        insertion,
        at,
        heading,
        job: place,
        seen,
    })
}

/// The lights that say somewhere to go after dark: a yard lamp, a lit
/// window, a landmark's red lamp. Not a machines' stake or a warning (they
/// say danger), nor a rendezvous's beacon (the way out, not the way in).
pub const STEER_BY: &[&str] = &["yard lamp", "lit window", "landmark lamp"];

/// The first frame after dark: the lights of `STEER_BY` within `AHEAD` of
/// the way's heading off the truck, within their reach, and in sight over
/// the ground, past the buildings (not their own) and through the trees.
pub fn first_frame_at_night(id: &str, map: &MapDef, hm: &Heightmap) -> Option<FirstFrame> {
    let Start {
        insertion,
        at,
        heading,
        place,
        ..
    } = start(id, map, hm)?;
    let solids = solids(map);
    let canopy = crate::map_landmarks::Canopy::new(map, hm);
    let mut seen = Vec::new();
    for l in crate::map_night::lights(map, hm) {
        if !STEER_BY.contains(&l.kind) {
            continue;
        }
        let d = dist(at, l.at);
        let o = off(bearing(at, l.at), heading);
        let mut own = l.on.clone();
        own.push(l.at);
        let verdict = if o > AHEAD {
            "out of frame"
        } else if d > l.reach {
            "too far"
        } else if !in_sight(
            hm,
            &solids,
            &canopy,
            &own,
            at,
            l.at,
            hm.height(l.at) + l.height,
        ) {
            "hidden"
        } else {
            "reads"
        };
        seen.push((format!("{} {}", l.kind, l.name), d, o, verdict));
    }
    seen.sort_by(|a, b| a.1.total_cmp(&b.1));
    Some(FirstFrame {
        insertion,
        at,
        heading,
        job: place,
        seen,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Valleys whose first frame is known not to read, each with whose it
    /// is and why. Off the list in the same commit as the fix.
    const GAPS: &[(&str, &str, &str)] = &[];
    /// The same, after dark.
    const NIGHT_GAPS: &[(&str, &str, &str)] = &[];

    #[test]
    fn every_first_frame_holds_somewhere_to_go() {
        let mut bad = Vec::new();
        let mut fixed = Vec::new();
        for (id, text) in crate::map::VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            for (when, frame, gaps) in [
                ("by day", first_frame(id, &map, &hm), GAPS),
                (
                    "after dark",
                    first_frame_at_night(id, &map, &hm),
                    NIGHT_GAPS,
                ),
            ] {
                let Some(f) = frame else {
                    continue;
                };
                let gap = gaps.iter().find(|g| g.0 == id);
                match (f.reads(), gap) {
                    (false, None) => bad.push(format!(
                        "{id} {when}: from {} toward {} (heading {:.0}): {}",
                        f.insertion,
                        f.job,
                        f.heading,
                        f.seen
                            .iter()
                            .map(|s| format!("{} {:.0} m {:.0} deg off, {}", s.0, s.1, s.2, s.3))
                            .collect::<Vec<_>>()
                            .join("; ")
                    )),
                    (true, Some(g)) => {
                        fixed.push(format!("{id} {when}: reads now, off its GAPS ({})", g.1))
                    }
                    _ => {}
                }
            }
        }
        assert!(
            bad.is_empty() && fixed.is_empty(),
            "{}",
            [bad, fixed].concat().join("\n")
        );
    }

    /// Prints each valley's first frame:
    ///   cargo test -p extraction_core probe_first_frame -- --ignored --nocapture
    #[test]
    #[ignore = "probe"]
    fn probe_first_frame() {
        let only = std::env::var("MAP_ID").ok();
        for (id, text) in crate::map::VALLEYS {
            if only.as_deref().is_some_and(|o| o != id) {
                continue;
            }
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            for (when, frame) in [
                ("by day", first_frame(id, &map, &hm)),
                ("after dark", first_frame_at_night(id, &map, &hm)),
            ] {
                let Some(f) = frame else {
                    println!("{id}: no first job");
                    continue;
                };
                println!(
                    "{id} {when}: from {} {:?} toward {}, heading {:.0}: {}",
                    f.insertion,
                    f.at,
                    f.job,
                    f.heading,
                    if f.reads() { "READS" } else { "NOTHING READS" }
                );
                for s in &f.seen {
                    println!("    {:<40} {:5.0} m {:4.0} deg off  {}", s.0, s.1, s.2, s.3);
                }
            }
        }
    }
}
