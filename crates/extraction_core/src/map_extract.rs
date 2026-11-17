//! The ways out of a valley the map lays down for extraction (#94): the
//! radio set at each pickup ([`MarkerKind::FieldSet`]) and the flare stops
//! ([`MarkerKind::FlareStop`]), held here to what the raid needs of them.
//!
//! - Every rendezvous, and the short exfil, has its set, by name, hung on
//!   a wall's face near its pad and facing out from it.
//! - Every valley has its flare stops: each named, on a level pad the
//!   truck stands on (`map_lint`'s truck stands), its vantage within
//!   [`FLARE_REACH`] and in sight of the truck there.

use crate::geom::{P2, dist};
use crate::map::{MapDef, MarkerKind, prefab_footprint};
use crate::terrain::Heightmap;

/// The farthest a flare's vantage is from its stop (m).
pub const FLARE_REACH: f32 = 150.0;
/// The farthest a set hangs from its pad (m).
pub const SET_REACH: f32 = 30.0;
/// The pad name a short exfil's set carries.
pub const SHORT: &str = "short";

/// Each set's pad, and where it hangs.
pub fn field_sets(map: &MapDef) -> Vec<(&str, P2, f32)> {
    map.markers
        .iter()
        .filter_map(|m| match &m.kind {
            MarkerKind::FieldSet { pad } => Some((pad.as_str(), m.at, m.rot)),
            _ => None,
        })
        .collect()
}

/// Each flare stop: its name, where the truck stops, the vantage.
pub fn flare_stops(map: &MapDef) -> Vec<(&str, P2, P2)> {
    map.markers
        .iter()
        .filter_map(|m| match &m.kind {
            MarkerKind::FlareStop { vantage } => Some((m.name.as_str(), m.at, *vantage)),
            _ => None,
        })
        .collect()
}

/// Whether the ground leaves a line from `a` (`ha` m up) to `b` (`hb` m
/// up) open.
pub fn ground_clear(hm: &Heightmap, a: P2, ha: f32, b: P2, hb: f32) -> bool {
    let ya = hm.height(a) + ha;
    let yb = hm.height(b) + hb;
    let n = (dist(a, b) / 0.5).ceil().max(2.0) as usize;
    (1..n).all(|i| {
        let t = i as f32 / n as f32;
        let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        hm.height(p) < ya + (yb - ya) * t - 0.1
    })
}

/// Where a lookout tower's four legs stand in its frame, and their half
/// width (`world::buildings::tower_legs`): a set can hang on one.
const TOWER_LEGS: ([f32; 2], f32) = ([1.8, 1.8], 0.125);

/// The solid boxes of a placement a set can hang on, in its frame: its
/// footprint, or a tower's legs (its footprint is the legs' and stair's
/// reach, open between).
fn faces(prefab: &str) -> Vec<(P2, P2)> {
    if matches!(prefab, "ranger_tower" | "survey_tower") {
        let ([x, z], h) = TOWER_LEGS;
        return [[-x, -z], [x, -z], [x, z], [-x, z]]
            .map(|at| (at, [h, h]))
            .to_vec();
    }
    let f = prefab_footprint(prefab);
    vec![([0.0, 0.0], [f[0] * 0.5, f[2] * 0.5])]
}

/// How far `p` is off the nearest wall face of a placement (m), and
/// whether `facing` (deg, the marker's rot: its front is (-sin, cos))
/// points out of it there. `None` with no placement near.
pub fn on_a_wall(map: &MapDef, p: P2, facing: f32) -> Option<(f32, bool)> {
    let (fs, fc) = facing.to_radians().sin_cos();
    let front = [-fs, fc];
    map.placements
        .iter()
        .flat_map(|pl| faces(&pl.prefab).into_iter().map(move |b| (pl, b)))
        .filter_map(|(pl, (mid, [hx, hz]))| {
            let (s, c) = pl.rot.to_radians().sin_cos();
            let d = [p[0] - pl.at[0], p[1] - pl.at[1]];
            // Into the placement's frame (local +x is (c, s)), then off the box's middle.
            let l = [d[0] * c + d[1] * s - mid[0], -d[0] * s + d[1] * c - mid[1]];
            let off = (l[0].abs() - hx).max(l[1].abs() - hz);
            if off.abs() > 0.5 {
                return None;
            }
            // The face's outward normal, in the world.
            let n = if l[0].abs() - hx > l[1].abs() - hz {
                [c * l[0].signum(), s * l[0].signum()]
            } else {
                [-s * l[1].signum(), c * l[1].signum()]
            };
            Some((off.abs(), n[0] * front[0] + n[1] * front[1] > 0.7))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::VALLEYS;
    use crate::map_lint::{TRUCK_LEVEL, truck_fall};

    /// Every pickup has its set, by name; every set names a pickup; each
    /// hangs on a wall's face near its pad, facing out.
    #[test]
    fn every_pickup_has_its_set_on_a_wall() {
        let mut bad = Vec::new();
        for (id, text) in VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let sets = field_sets(&map);
            let mut pads: Vec<(String, P2)> = map
                .markers_of(&MarkerKind::Rendezvous)
                .map(|m| (m.name.clone(), m.at))
                .collect();
            pads.extend(
                map.markers_of(&MarkerKind::ShortExfil)
                    .map(|m| (SHORT.to_string(), m.at)),
            );
            for (name, at) in &pads {
                let mine: Vec<_> = sets.iter().filter(|s| s.0 == name).collect();
                if mine.len() != 1 {
                    bad.push(format!("{id}: {} sets for {name}", mine.len()));
                    continue;
                }
                let (_, set, rot) = *mine[0];
                if dist(set, *at) > SET_REACH {
                    bad.push(format!(
                        "{id}: {name}'s set {:.0} m off its pad",
                        dist(set, *at)
                    ));
                }
                match on_a_wall(&map, set, rot) {
                    Some((off, true)) if off < 0.15 => {}
                    w => bad.push(format!("{id}: {name}'s set at {set:?} rot {rot}: {w:?}")),
                }
            }
            for (pad, at, _) in &sets {
                if !pads.iter().any(|(n, _)| n == pad) {
                    bad.push(format!("{id}: the set at {at:?} names no pickup \"{pad}\""));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Every valley has two flare stops, each named, the truck level on
    /// it, its vantage within reach and in sight of the truck there.
    #[test]
    fn every_valley_has_its_flare_stops() {
        let mut bad = Vec::new();
        for (id, text) in VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let stops = flare_stops(&map);
            if stops.len() < 2 {
                bad.push(format!("{id}: {} flare stops", stops.len()));
            }
            for (name, at, v) in stops {
                if name.is_empty() {
                    bad.push(format!("{id}: a flare stop at {at:?} has no name"));
                }
                let d = dist(at, v);
                if d > FLARE_REACH {
                    bad.push(format!("{id}: {name}'s vantage {d:.0} m off"));
                }
                if !ground_clear(&hm, v, 1.6, at, 3.0) {
                    bad.push(format!(
                        "{id}: {name} is out of sight from its vantage {v:?}"
                    ));
                }
                // As the truck stands at a rendezvous: its bed to the valley's middle.
                let yaw = (-at[0]).atan2(-at[1]);
                let fall = truck_fall(&hm, at, yaw);
                if fall.abs() > TRUCK_LEVEL {
                    bad.push(format!(
                        "{id}: {name}'s truck falls {:.1}°",
                        fall.to_degrees()
                    ));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
