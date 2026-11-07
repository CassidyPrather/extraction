//! What's lit ahead on the ways home at night, without rendering: at each
//! stop of `night_walks` (a first job's way home, a stop every 50 m), is a
//! light in sight within [`CONE`] of the way ahead, the night gauge's dark
//! case (`scripts/night_route.sh`) read off the map? Lights are the yard
//! lamps' bulbs (`world::way_lamps`), the landmarks' lamps, the rendezvous
//! beacons and the lit windows the map asks for (`Cue::LitWindow`), each
//! as far as its glow carries; in sight over the land, past the buildings
//! between and under no canopy, and in the frame's height.

use crate::geom::{P2, dist};
use crate::map::{Cue, MapDef, MarkerKind};
use crate::night_walks::{Stop, walks};
use crate::terrain::Heightmap;

/// How far either side of the way ahead a light counts (degrees): the
/// gauge's `night_route.py` `CONE`.
pub const CONE: f32 = 30.0;
/// Half the stills' vertical field of view (degrees: `STILL_FOV` 75).
pub const HALF_FOV: f32 = 37.5;
/// A yard lamp's bulb off its post's foot, prefab-local (m:
/// `world::way_lamps::BULB_AT`), and how far its glow carries
/// (`render::way_lamps` `HALO_REACH`).
pub const WAY_BULB: (f32, f32) = (0.55, 2.38);
pub const WAY_REACH: f32 = 240.0;

/// A light to steer by.
#[derive(Clone, Debug)]
pub struct Light {
    pub kind: &'static str,
    pub name: String,
    pub at: P2,
    /// Its height over the ground under it (m).
    pub height: f32,
    pub reach: f32,
}

/// Every light in the valley after dark.
pub fn lights(map: &MapDef, hm: &Heightmap) -> Vec<Light> {
    let mut out = Vec::new();
    for p in map.placements.iter().filter(|p| p.prefab == "way_lamp") {
        let (s, c) = p.rot.to_radians().sin_cos();
        let at = [p.at[0] + WAY_BULB.0 * c, p.at[1] + WAY_BULB.0 * s];
        let foot = hm.footing(&p.prefab, p.at, p.rot) + p.y;
        out.push(Light {
            kind: "yard lamp",
            name: format!("({:.0}, {:.0})", p.at[0], p.at[1]),
            at,
            height: foot + WAY_BULB.1 - hm.height(at),
            reach: WAY_REACH,
        });
    }
    for m in crate::map_landmarks::landmarks(map, hm) {
        for (top, y) in &m.tops {
            out.push(Light {
                kind: "landmark lamp",
                name: m.name.clone(),
                at: *top,
                height: y - hm.height(*top),
                reach: crate::map_lint::LAMP_REACH,
            });
        }
    }
    for rv in map.markers_of(&MarkerKind::Rendezvous) {
        out.push(Light {
            kind: "beacon",
            name: rv.name.clone(),
            at: crate::map_landmarks::beacon_at(rv.at),
            height: crate::map_landmarks::BEACON_HEIGHT,
            reach: crate::map_lint::BEACON_REACH,
        });
    }
    for c in &map.cues {
        if let Cue::LitWindow { at, .. } = c {
            out.push(Light {
                kind: "lit window",
                name: format!("({:.0}, {:.0})", at[0], at[1]),
                at: *at,
                height: crate::map_breadcrumbs::WINDOW_HEIGHT,
                reach: crate::map_breadcrumbs::WINDOW_SIGHT,
            });
        }
    }
    out
}

/// Whether a light at `to`, its top at `top`, is in sight from an eye at
/// `eye` over `from`: over the land, and past the buildings between (not
/// the one it's on). As `map_gradient`'s, from the walker's own eye: on a
/// bridge's deck, not the gully under it.
fn in_sight(
    hm: &Heightmap,
    solids: &[crate::map_lint::Solid],
    from: P2,
    eye: f32,
    to: P2,
    top: f32,
) -> bool {
    let d = dist(from, to);
    let n = (d / 2.0).ceil() as usize;
    let land = (1..n).all(|i| {
        let t = i as f32 / n as f32;
        d * (1.0 - t) < 10.0
            || hm.height(crate::geom::lerp(from, to, t)) < eye + (top - eye) * t - 0.3
    });
    land && solids
        .iter()
        .filter(|s| s.rect.sdf(to) > 3.0 && s.rect.sdf(from) > 0.5)
        .all(|s| {
            let h = crate::map::prefab_footprint(&s.prefab)[1];
            (1..n).all(|i| {
                let t = i as f32 / n as f32;
                let p = crate::geom::lerp(from, to, t);
                s.rect.sdf(p) > 0.0 || hm.height(p) + h < eye + (top - eye) * t
            })
        })
}

/// The lights a stop sees ahead: kind, name, how far.
pub(crate) fn seen_ahead(
    hm: &Heightmap,
    solids: &[crate::map_lint::Solid],
    canopy: &crate::map_landmarks::Canopy,
    lights: &[Light],
    s: &Stop,
) -> Vec<(&'static str, String, f32)> {
    let mut out = Vec::new();
    for l in lights {
        let d = dist(s.at, l.at);
        if d > l.reach || d < 1.0 {
            continue;
        }
        let dir = [(l.at[0] - s.at[0]) / d, (l.at[1] - s.at[1]) / d];
        let off = (dir[0] * s.look[0] + dir[1] * s.look[1])
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees();
        if off > CONE {
            continue;
        }
        let top = hm.height(l.at) + l.height;
        let up = ((top - s.eye[1]) / d).atan().to_degrees();
        if (up - s.pitch).abs() > HALF_FOV {
            continue;
        }
        if !in_sight(hm, solids, s.at, s.eye[1], l.at, top) {
            continue;
        }
        if canopy.hides(hm, s.at, l.at, top) {
            continue;
        }
        out.push((l.kind, l.name.clone(), d));
    }
    out.sort_by(|a, b| a.2.total_cmp(&b.2));
    out
}

/// Each stop of the valley's ways home, and what it sees lit ahead (none:
/// the way can't be told there without a torch).
pub fn read(
    map: &MapDef,
    hm: &Heightmap,
    map_id: &str,
) -> Vec<(Stop, Vec<(&'static str, String, f32)>)> {
    let lights = lights(map, hm);
    let solids = crate::map_lint::solids(map);
    let canopy = crate::map_landmarks::Canopy::new(map, hm);
    walks(map, hm, map_id)
        .into_iter()
        .map(|s| {
            let seen = seen_ahead(hm, &solids, &canopy, &lights, &s);
            (s, seen)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each stop, READ (and on what) or LOST, for setting beside the night
    /// gauge's dark case: `MAP_ID=<valley> cargo test -p extraction_core
    /// probe_night -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn probe_night_why() {
        let id = std::env::var("MAP_ID").unwrap();
        let stop = std::env::var("STOP").unwrap();
        let text = crate::map::VALLEYS.iter().find(|v| v.0 == id).unwrap().1;
        let map = MapDef::from_ron(text).unwrap();
        let hm = Heightmap::build(&map);
        let solids = crate::map_lint::solids(&map);
        let canopy = crate::map_landmarks::Canopy::new(&map, &hm);
        let s = walks(&map, &hm, &id)
            .into_iter()
            .find(|s| s.name == stop)
            .unwrap();
        println!("WHY eye {:?} look {:?} pitch {}", s.eye, s.look, s.pitch);
        for l in lights(&map, &hm) {
            let d = dist(s.at, l.at);
            if d > l.reach {
                continue;
            }
            let dir = [(l.at[0] - s.at[0]) / d, (l.at[1] - s.at[1]) / d];
            let off = (dir[0] * s.look[0] + dir[1] * s.look[1])
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees();
            if off > 45.0 {
                continue;
            }
            let top = hm.height(l.at) + l.height;
            let up = ((top - s.eye[1]) / d).atan().to_degrees();
            println!(
                "WHY {} {} d {:.0} off {:.0} up {:.1} sight {} canopy {}",
                l.kind,
                l.name,
                d,
                off,
                up,
                in_sight(&hm, &solids, s.at, s.eye[1], l.at, top),
                canopy.hides(&hm, s.at, l.at, top)
            );
        }
    }

    #[test]
    #[ignore]
    fn probe_night() {
        let only = std::env::var("MAP_ID").ok();
        for (id, text) in crate::map::VALLEYS {
            if only.as_deref().is_some_and(|o| o != id) {
                continue;
            }
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            for (s, seen) in read(&map, &hm, id) {
                let first = seen
                    .first()
                    .map_or(String::new(), |(k, n, d)| format!("{k} {n} ({d:.0} m)"));
                println!(
                    "NIGHT {id} {} {} {}",
                    s.name,
                    if seen.is_empty() && s.to_rv > 25.0 {
                        "LOST"
                    } else {
                        "READ"
                    },
                    first
                );
            }
        }
    }
}
