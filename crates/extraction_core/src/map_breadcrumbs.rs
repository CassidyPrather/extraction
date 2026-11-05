//! Breadcrumbs: every reward off the ways a raid walks is cued from them.
//!
//! A reward (a cache, a note, a terminal, the valley's key, an offering,
//! an artifact field, a place a job sends you, a staged scene) that lies
//! off the walked ways (the main routes, the first jobs' walks there and
//! home, each place's way home: `map_lint::routed_ways`) needs something a
//! walker on them sees or reads: its building's silhouette, a lamp, a
//! trail leaving the way toward it, a sign, the anomaly an artifact field
//! lies in. `docs/MAP_DESIGN.md`, "Breadcrumbs".
//!
//! What the map can't say isn't counted: smoke and sound are the game's.
//! Sight is over the ground only (`map_lint::sees`), not through woods or
//! fog: the stills from the route judge the worst.

use crate::geom::polyline_closest;
use crate::geom::{P2, dist};
use crate::map::{MapDef, MarkerKind, prefab_footprint};
use crate::map_lint::{Route, Solid, routed_ways, sees, solids};
use crate::nav::NavGrid;
use crate::terrain::Heightmap;

/// A reward this near a walked way is on it: nothing need lead you there.
pub const ON_PATH: f32 = 12.0;
/// A structure this tall reads as a silhouette...
pub const SILHOUETTE_HEIGHT: f32 = 2.5;
/// ...from this far off (m): a roof, a wall, a tower over the grass.
pub const SILHOUETTE_SIGHT: f32 = 90.0;
/// A structure this near a reward stands for it: its building, or the one
/// it's beside.
pub const CONTAINER: f32 = 8.0;
/// A road or path within this of a reward leads to it...
pub const TRAIL_NEAR: f32 = 8.0;
/// ...if it leaves a walked way (passes this near one).
pub const TRAIL_JOIN: f32 = 6.0;
/// A landmark's lamp this near a reward marks it, seen from this far.
pub const LAMP_NEAR: f32 = 30.0;
pub const LAMP_SIGHT: f32 = 250.0;
/// A sign this near a reward points at it, read from this near a way.
pub const SIGN_NEAR: f32 = 60.0;
pub const SIGN_READ: f32 = 12.0;
/// An anomaly shows (its shimmer, its sound) from this far.
pub const ANOMALY_SIGHT: f32 = 50.0;
/// The walked ways are judged every this many metres.
const STEP: f32 = 4.0;

/// A reward, how far it lies off the walked ways, and what cues it.
#[derive(Clone, Debug)]
pub struct Crumb {
    pub kind: &'static str,
    pub name: String,
    pub at: P2,
    /// The nearest walked way, metres, and its nearest point.
    pub off: f32,
    pub from: P2,
    /// The cue and how far from the way it's met (None: none).
    pub cue: Option<(String, f32)>,
}

/// A staged scene's place (`assets/data/scenes.ron`): which map, its name
/// and where.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct SceneAt {
    pub map: String,
    pub name: String,
    pub at: (f32, f32),
}

/// Every reward in `map`, with `scenes` (the staged scenes of this map),
/// against the walked ways.
pub fn crumbs(
    map: &MapDef,
    hm: &Heightmap,
    comfy: &NavGrid,
    routes: &[Route],
    scenes: &[SceneAt],
) -> Vec<Crumb> {
    let walked: Vec<P2> = routed_ways(map, comfy, routes)
        .into_iter()
        .flat_map(|(_, w)| {
            w.into_iter()
                .step_by((STEP / crate::map_lint::STEP).max(1.0) as usize)
        })
        .collect();
    let solids = solids(map);
    let marks = crate::map_landmarks::landmarks(map, hm);
    let mut rewards: Vec<(&'static str, String, P2)> = Vec::new();
    for m in &map.markers {
        let kind = match &m.kind {
            MarkerKind::Loot { .. } => "cache",
            MarkerKind::Note { .. } => "note",
            MarkerKind::Terminal { .. } => "terminal",
            MarkerKind::ValleyKey { .. } => "key",
            MarkerKind::Offering => "offering",
            MarkerKind::ArtifactField => "artifacts",
            MarkerKind::Poi => "place",
            _ => continue,
        };
        rewards.push((kind, m.name.clone(), m.at));
    }
    for s in scenes {
        rewards.push(("scene", s.name.clone(), [s.at.0, s.at.1]));
    }
    rewards
        .into_iter()
        .map(|(kind, name, at)| {
            let (off, from) = walked
                .iter()
                .map(|w| (dist(*w, at), *w))
                .fold((f32::INFINITY, at), |a, b| if b.0 < a.0 { b } else { a });
            let cue = if off <= ON_PATH {
                Some(("on the way".to_string(), off))
            } else {
                cue_for(map, hm, &solids, &marks, &walked, kind, at)
            };
            Crumb {
                kind,
                name,
                at,
                off,
                from,
                cue,
            }
        })
        .collect()
}

/// The nearest cue to `at` met from the walked ways: what it is and how
/// far from the way.
fn cue_for(
    map: &MapDef,
    hm: &Heightmap,
    solids: &[Solid],
    marks: &[crate::map_landmarks::Landmark],
    walked: &[P2],
    kind: &str,
    at: P2,
) -> Option<(String, f32)> {
    let mut best: Option<(String, f32)> = None;
    let mut take = |what: String, d: f32| {
        if best.as_ref().is_none_or(|(_, b)| d < *b) {
            best = Some((what, d));
        }
    };
    // Its building, or one beside it, over the ground.
    for s in solids.iter().filter(|s| s.rect.sdf(at) <= CONTAINER) {
        let h = prefab_footprint(&s.prefab)[1];
        if h < SILHOUETTE_HEIGHT {
            continue;
        }
        for w in walked {
            let d = dist(*w, s.rect.at);
            if d <= SILHOUETTE_SIGHT && sees(hm, *w, s.rect.at, h) {
                take(format!("the {}'s silhouette", s.prefab), d);
            }
        }
    }
    // A landmark's lamp near it.
    for m in marks {
        for (top, y) in &m.tops {
            if dist(*top, at) > LAMP_NEAR {
                continue;
            }
            let h = y - hm.height(*top);
            for w in walked {
                let d = dist(*w, *top);
                if d <= LAMP_SIGHT && sees(hm, *w, *top, h) {
                    take(format!("the {}'s lamp", m.name), d);
                }
            }
        }
    }
    // A trail off the way toward it.
    for r in &map.roads {
        let (d, _, _) = polyline_closest(&r.points, at);
        if d > TRAIL_NEAR + r.width * 0.5 {
            continue;
        }
        if let Some(w) = walked
            .iter()
            .find(|w| polyline_closest(&r.points, **w).0 <= TRAIL_JOIN + r.width * 0.5)
        {
            take(format!("the {}", r.name), dist(*w, at));
        }
    }
    // A sign beside a way pointing at it.
    for m in map
        .markers
        .iter()
        .filter(|m| matches!(m.kind, MarkerKind::Sign { .. }))
    {
        if dist(m.at, at) > SIGN_NEAR {
            continue;
        }
        if walked.iter().any(|w| dist(*w, m.at) <= SIGN_READ) {
            take(format!("the sign \"{}\"", m.name), dist(m.at, at));
        }
    }
    // The anomaly an artifact field lies in.
    if kind == "artifacts" {
        for m in &map.markers {
            let MarkerKind::Anomaly { radius, .. } = &m.kind else {
                continue;
            };
            if dist(m.at, at) > radius + 10.0 {
                continue;
            }
            for w in walked {
                let d = dist(*w, m.at);
                if d <= ANOMALY_SIGHT + radius {
                    take("its anomaly".to_string(), d);
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenes() -> Vec<SceneAt> {
        let text = include_str!("../../../assets/data/scenes.ron");
        ron::from_str(text).expect("scenes.ron parses as a list of (map, name, at)")
    }

    /// Prints every valley's rewards and their cues, the uncued worst
    /// first (`MAP_ID` for one valley).
    #[test]
    #[ignore]
    fn probe_breadcrumbs() {
        let only = std::env::var("MAP_ID").ok();
        let scenes = scenes();
        for (id, text) in crate::map::VALLEYS {
            if only.as_deref().is_some_and(|o| o != id) {
                continue;
            }
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let comfy = crate::map_nav::zone_grid(&map, &hm, crate::map_lint::COMFORT_SLOPE);
            let r = crate::map_lint::lint(&map, &hm);
            let here: Vec<SceneAt> = scenes.iter().filter(|s| s.map == *id).cloned().collect();
            let mut cs = crumbs(&map, &hm, &comfy, &r.routes, &here);
            cs.sort_by(|a, b| {
                (a.cue.is_some(), -a.off)
                    .partial_cmp(&(b.cue.is_some(), -b.off))
                    .unwrap()
            });
            let uncued = cs.iter().filter(|c| c.cue.is_none()).count();
            println!("== {id}: {} rewards, {uncued} uncued", cs.len());
            for c in &cs {
                match &c.cue {
                    None => println!(
                        "  UNCUED {} \"{}\" at ({:.1}, {:.1}), {:.0} m off, nearest way ({:.1}, {:.1})",
                        c.kind, c.name, c.at[0], c.at[1], c.off, c.from[0], c.from[1]
                    ),
                    Some((q, d)) => println!(
                        "  {} \"{}\" {:.0} m off: {q} ({d:.0} m)",
                        c.kind, c.name, c.off
                    ),
                }
            }
        }
    }
}
