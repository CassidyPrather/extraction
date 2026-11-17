//! A village's well sweep (`well_sweep`): a tall post with a fork at the
//! top, a long pole pivoted in it, a stone counterweight lashed to its
//! short end and a rope hanging from its long end to the well. At rest the
//! weight is down and the long end up over the well, so the pole stands
//! out over the roofs: Marrow's village landmark, by The well.
//!
//! The placement stands on the post. Its front (+Z, `rot` 0) is the well:
//! the long end reaches [`REACH`] out that way. Only the post is solid.
//! No lamp and no loot: it is somewhere to name, not to light or to search.

use bevy::prelude::*;

use super::buildings::{Builder, Geo, Shape};
use crate::render::materials::Mat;

pub const PREFAB: &str = "well_sweep";

/// The pivot's height on the post, the pole's tilt above level (radians),
/// and its arms either side of the pivot (m).
const PIVOT: f32 = 6.5;
const TILT: f32 = 0.61;
const LONG: f32 = 7.3;
const SHORT: f32 = 3.7;

/// How far out the long end reaches, level, from the post (m): where the
/// well's ring stands.
pub const REACH: f32 = 5.98;

pub fn geo(id: &str) -> Option<Geo> {
    if id != PREFAB {
        return None;
    }
    let mut b = Builder::default();
    // The post, sunk a little, and the fork's two cheeks at its top.
    b.bx(
        Vec3::new(0.32, PIVOT + 0.3, 0.32),
        Vec3::new(0.0, (PIVOT + 0.3) * 0.5 - 0.1, 0.0),
        Mat::Timber,
    );
    for x in [-0.16f32, 0.16] {
        b.deco(
            Vec3::new(0.08, 0.7, 0.3),
            Vec3::new(x, PIVOT + 0.5, 0.0),
            Mat::Timber,
        );
    }
    // The pole: one length through the fork, tipped so the long end (+Z)
    // rises.
    let dir = Vec3::new(0.0, TILT.sin(), TILT.cos());
    let pivot = Vec3::Y * (PIVOT + 0.4);
    let mid = pivot + dir * (LONG - SHORT) * 0.5;
    let pole = Quat::from_rotation_arc(Vec3::Y, dir);
    b.piece(
        Shape::Cylinder(0.08, LONG + SHORT),
        Transform::from_translation(mid).with_rotation(pole),
        Mat::Timber,
        false,
    );
    // The counterweight: a stone lashed under the short end.
    let short = pivot - dir * SHORT;
    b.piece(
        Shape::Box(Vec3::new(0.6, 0.5, 0.6)),
        Transform::from_translation(short - Vec3::Y * 0.3).with_rotation(pole),
        Mat::Boulder,
        false,
    );
    // The rope, straight down from the long end to a bucket at the well's
    // lip.
    let long = pivot + dir * LONG;
    let bucket = 1.1;
    b.deco(
        Vec3::new(0.03, long.y - bucket, 0.03),
        Vec3::new(0.0, (long.y + bucket) * 0.5, long.z),
        Mat::Canvas,
    );
    b.piece(
        Shape::Cylinder(0.16, 0.32),
        Transform::from_translation(Vec3::new(0.0, bucket - 0.16, long.z)),
        Mat::DarkMetal,
        false,
    );
    Some(b.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The long end hangs over the well, [`REACH`] out; the sweep is as
    /// tall as the core's footprint says, and short of a landmark's height
    /// (no landmark lamp on it); only the post collides.
    #[test]
    fn the_sweep_reaches_the_well_and_only_its_post_is_solid() {
        let geo = geo(PREFAB).unwrap();
        let dir = Vec3::new(0.0, TILT.sin(), TILT.cos());
        let long = Vec3::Y * (PIVOT + 0.4) + dir * LONG;
        assert!((long.z - REACH).abs() < 0.05, "reach {:.2}", long.z);
        let top = long.y + 0.08;
        let foot = extraction_core::map::prefab_footprint(PREFAB);
        assert!((foot[1] - top).abs() < 0.2, "top {top:.2} vs {:?}", foot);
        assert!(top < extraction_core::map_lint::LANDMARK_HEIGHT);
        let solid: Vec<_> = geo.pieces.iter().filter(|p| p.collide).collect();
        assert_eq!(solid.len(), 1, "only the post is solid");
        assert!(geo.loot.is_empty(), "no loot on the sweep");
        assert!(geo.lights.is_empty(), "no lamp on the sweep");
    }
}
