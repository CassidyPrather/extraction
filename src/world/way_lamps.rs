//! A yard lamp on a post (`way_lamp`): the kind a farm, a works or a
//! keeper hangs by a track or a gate and leaves burning, here where the
//! way home can't be told after dark without one (`scripts/night_route.sh`,
//! no torch). The post and its arm are the prefab; its light, a warm bulb
//! under a hood with a glow that carries, is the client's
//! (`render::way_lamps`).

use bevy::prelude::*;

use super::buildings::{Builder, Geo};
use crate::render::materials::Mat;

/// Every prefab built here.
pub const PREFABS: [&str; 1] = ["way_lamp"];

/// The post's height (m), and where its bulb hangs off the arm, in the
/// prefab's frame.
const POST: f32 = 2.7;
pub const BULB_AT: Vec3 = Vec3::new(0.55, 2.38, 0.0);

pub fn geo(id: &str) -> Option<Geo> {
    let mut b = Builder::default();
    match id {
        "way_lamp" => way_lamp(&mut b),
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
