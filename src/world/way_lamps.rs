//! A yard lamp on a post (`way_lamp`): the kind a farm, a works or a
//! keeper hangs by a track or a gate and leaves burning, here where the
//! way home can't be told after dark without one (`scripts/night_route.sh`,
//! no torch). The post and its arm are the prefab; its light, a warm bulb
//! under a hood with a glow that carries, is the client's
//! (`render::way_lamps`).
//!
//! And a marker post (`marker_post`): a plain timber post a road-mender
//! drives in beside a way over open ground, two bands of reflective tape
//! round its top, as on the fell's snow poles. Where a torch's pool shows
//! nothing of the way (`scripts/night_route.sh`, the torch case), the
//! bands shine back from the beam 15 m on. The tape is the
//! client's (`render::way_lamps`).

use bevy::prelude::*;

use super::buildings::{Builder, Geo};
use crate::render::materials::Mat;

/// Every prefab built here.
pub const PREFABS: [&str; 2] = ["way_lamp", "marker_post"];

/// The post's height (m), and where its bulb hangs off the arm, in the
/// prefab's frame.
const POST: f32 = 2.7;
pub const BULB_AT: Vec3 = Vec3::new(0.55, 2.38, 0.0);
/// A marker post's height (m) and thickness, and its bands' middles and
/// depth (m over the ground), in the prefab's frame: chest high, in a
/// hand torch's beam on the way ahead.
const MARKER: f32 = 1.3;
pub const MARKER_W: f32 = 0.1;
pub const BANDS: [f32; 2] = [1.0, 1.2];
pub const BAND_H: f32 = 0.08;

pub fn geo(id: &str) -> Option<Geo> {
    let mut b = Builder::default();
    match id {
        "way_lamp" => way_lamp(&mut b),
        "marker_post" => {
            b.bx(
                Vec3::new(MARKER_W, MARKER, MARKER_W),
                Vec3::new(0.0, MARKER * 0.5, 0.0),
                Mat::Timber,
            );
        }
        _ => return None,
    }
    Some(b.finish())
}

/// A timber post with an iron arm off its top and a hood over the bulb.
/// Only the post is solid.
fn way_lamp(b: &mut Builder) {
    b.bx(
        Vec3::new(0.12, POST, 0.12),
        Vec3::new(0.0, POST * 0.5, 0.0),
        Mat::Timber,
    );
    b.deco(
        Vec3::new(0.62, 0.05, 0.05),
        Vec3::new(0.3, POST - 0.15, 0.0),
        Mat::DarkMetal,
    );
    b.deco(
        Vec3::new(0.32, 0.1, 0.32),
        Vec3::new(BULB_AT.x, BULB_AT.y + 0.14, 0.0),
        Mat::DarkMetal,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No ivy climbs a marker post: it smothered the bands, so the torch
    /// had nothing to shine back from.
    #[test]
    fn a_marker_post_grows_no_ivy() {
        let geo = geo("marker_post").unwrap();
        let occ = crate::world::buildings::occluders(&geo);
        for k in 1..24u64 {
            let g =
                crate::world::overgrowth::grow_on("marker_post", &geo, &occ, k * 7919, Vec3::NEG_Z);
            assert!(
                g.vines.is_empty() && g.leaves.is_empty(),
                "seed {k}: ivy on a marker post"
            );
        }
    }
}
