//! A siren mast (`siren_mast`): a steel pole the Authority stood by a works
//! it ran from afar, its horn near the top and an amber lamp over that, to
//! call the alarm across a valley when the telemetry said so. Sallow's
//! stands by the gauge house (tagged [`GAUGE_HORN`]): tall enough that its
//! horn and lamp show over the catchwater's cutting from every step of the
//! catchwater path, and over the bed below.
//!
//! The game builds it (`world::siren_mast`); here is where its horn and
//! lamp are, for whatever sounds it and lights it.

use crate::map::{MapDef, Placement};
use crate::terrain::Heightmap;

/// The prefab's id.
pub const PREFAB: &str = "siren_mast";
/// The tag on the gauge house's mast.
pub const GAUGE_HORN: &str = "gauge_horn";
/// The mast's height (m), its foot to the lamp's cap.
pub const HEIGHT: f32 = 16.4;
/// The horn's mouth, in the prefab's frame (x, y up from its foot, z): it
/// faces the prefab's -Z.
pub const HORN: [f32; 3] = [0.0, 15.0, -0.75];
/// The lamp's middle, in the prefab's frame.
pub const LAMP: [f32; 3] = [0.0, 16.0, 0.0];

/// A point in a placement's frame, in the world: map x, height, map z.
fn world(hm: &Heightmap, p: &Placement, at: [f32; 3]) -> [f32; 3] {
    let base = if p.absolute {
        p.y
    } else {
        hm.footing(&p.prefab, p.at, p.rot) + p.y
    };
    let (s, c) = p.rot.to_radians().sin_cos();
    [
        p.at[0] + at[0] * c - at[2] * s,
        base + at[1],
        p.at[1] + at[0] * s + at[2] * c,
    ]
}

/// Where a siren mast's horn is: map x, height, map z.
pub fn horn_at(hm: &Heightmap, p: &Placement) -> [f32; 3] {
    world(hm, p, HORN)
}

/// Where a siren mast's lamp is: map x, height, map z.
pub fn lamp_at(hm: &Heightmap, p: &Placement) -> [f32; 3] {
    world(hm, p, LAMP)
}

/// The gauge house's siren mast, where a map has one.
pub fn gauge_horn(map: &MapDef) -> Option<&Placement> {
    map.placements
        .iter()
        .find(|p| p.prefab == PREFAB && p.tag == GAUGE_HORN)
}
