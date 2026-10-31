//! Model correctness: placed models through each other (M3 in
//! `docs/playtests/2026-10-28/TRANSCRIPT.md`, parts through parts, at the
//! scale of a room). The seated lint holds an item in its mount and the
//! sill lint a thing on its sill; this holds every pair of models a site
//! places, in every valley and the yard: clutter through furniture,
//! furniture through furniture, a scene's things through each other.
//!
//! Each site's models as the game places them (`footing::sites`). Their
//! boxes first; where two overlap, how far each one's points lie inside the
//! other (winding number, `seated::through`). A pair is through each other
//! past [`DEPTH`]. Fittings made to nest (a window's sash in its casing, a
//! film on a sill) and ground cover are left out ([`apart`]).
//!
//! `cargo test --lib capture::overlap -- --nocapture` prints every finding.

use bevy::math::Vec3;

use super::footing::{self, Placed, Shapes};

/// How far a point may lie inside another model (m): a leg bedded in a rug,
/// a book's corner in a shelf's board.
pub const DEPTH: f32 = 0.02;

/// Models that sit in or on others by design: a window's or door's parts
/// in their frames, films and stains lying on what they cover, rubble and
/// ash heaped round what's in them, ground cover.
pub fn apart(name: &str) -> bool {
    [
        "window_", "door_", "ash_film", "rug", "rubble", "ash_heap", "_burnt", "_charred", "silt",
        "puddle", "moss", "snow",
    ]
    .iter()
    .any(|k| name.contains(k))
}

/// Every pair of models through each other at a site: (a, b, depth, where).
pub fn through_pairs(models: &[Placed]) -> Vec<(String, String, f32, Vec3)> {
    let mut out = Vec::new();
    for i in 0..models.len() {
        let a = &models[i];
        if apart(&a.name) {
            continue;
        }
        for b in &models[i + 1..] {
            if apart(&b.name) {
                continue;
            }
            // Boxes first.
            if !(a.lo.cmplt(b.hi - DEPTH).all() && b.lo.cmplt(a.hi - DEPTH).all()) {
                continue;
            }
            let pts = |m: &Placed| -> Vec<Vec3> { m.tris.iter().flatten().copied().collect() };
            let (da, _, at_a) = super::seated::through(&b.tris, &pts(a));
            let (db, _, at_b) = super::seated::through(&a.tris, &pts(b));
            let (d, at) = if da >= db { (da, at_a) } else { (db, at_b) };
            if d > DEPTH {
                out.push((a.name.clone(), b.name.clone(), d, at));
            }
        }
    }
    out
}

/// Every finding in a valley, one line each.
pub fn findings(map: &str, shapes: &mut Shapes) -> Vec<String> {
    let (_, sites) = footing::sites(map, shapes);
    let mut out = Vec::new();
    for site in &sites {
        for (a, b, d, at) in through_pairs(&site.models) {
            out.push(format!(
                "{map} {}: {a} and {b} {:.0} mm through each other at ({:.2}, {:.2}, {:.2})",
                site.name,
                d * 1000.0,
                at.x,
                at.y,
                at.z
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No two models a valley places are through each other.
    #[test]
    fn placed_models_keep_out_of_each_other() {
        let mut shapes = Shapes::default();
        let mut bad = Vec::new();
        for map in footing::maps() {
            for f in findings(&map, &mut shapes) {
                println!("{f}");
                if !KNOWN.iter().any(|k| f.contains(k)) {
                    bad.push(f);
                }
            }
        }
        assert!(
            bad.is_empty(),
            "models through each other:\n{}",
            bad.join("\n")
        );
    }

    /// Pairs made to meet.
    const KNOWN: &[&str] = &[
        // The barrier arm's pivot pin through its post.
        "gate_post and barrier_arm",
        // The drying rack hung round the stove's pipe.
        "stove and stove_pipe_rack",
        // The motor's shaft into its gearbox's coupling.
        "gearbox and drive_motor",
        // The cold-blast valves' pipe stubs into the stoves they feed, the
        // clay gun's nozzle into the taphole.
        "blast_furnace and gate_valve",
        "blast_furnace and mud_gun",
        // The observatory's bunk frame round its lower bed, posts at its
        // corners.
        "bed and obs_bunk",
    ];
}
