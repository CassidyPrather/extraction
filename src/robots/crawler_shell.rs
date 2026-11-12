//! A crawler's core is hit where its shell is: Models' `crawler_core`
//! carapace (walker.glb), low, wide and flat-topped, not the walker's
//! orb. The walker's sphere (r 0.4) stood 13 cm above the carapace's flat
//! top, and rounds over the shell hit air that counted.
//!
//! The carapace, walker units (a crawler is ×0.6): x ±0.511, z ±0.426,
//! y -0.310 to +0.254. The hit volume is a flat cylinder top to bottom,
//! kept inside the eye's front (the visor sits flush with the shell at
//! (0, 0.12, -0.36): a round at the eye from in front meets the eye, not
//! the core in front of it), and a slab across for the shell's width.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::indoor::Crawler;
use super::{PartRole, RobotPart};

pub struct CrawlerShellPlugin;

impl Plugin for CrawlerShellPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, shell_cores);
    }
}

/// The carapace's bottom and flat top, walker units.
pub const SHELL_BOTTOM: f32 = -0.310;
pub const SHELL_TOP: f32 = 0.254;
/// Its half-width across (X) and half-depth (Z).
pub const SHELL_X: f32 = 0.511;
pub const SHELL_Z: f32 = 0.426;
/// The cylinder's radius: inside the eye's front face (z -0.39).
const BODY_R: f32 = 0.385;
/// The slab across: its half-depth, and its height (the carapace's
/// middle, where it's widest).
const SIDE_Z: f32 = 0.22;
const SIDE_BOTTOM: f32 = -0.20;
const SIDE_TOP: f32 = 0.15;

/// A crawler core's hit volume (its own units, local to the core).
pub fn core_collider() -> Collider {
    let mid = (SHELL_BOTTOM + SHELL_TOP) * 0.5;
    let side_mid = (SIDE_BOTTOM + SIDE_TOP) * 0.5;
    Collider::compound(vec![
        (
            Vec3::Y * mid,
            Quat::IDENTITY,
            Collider::cylinder(BODY_R, SHELL_TOP - SHELL_BOTTOM),
        ),
        (
            Vec3::Y * side_mid,
            Quat::IDENTITY,
            Collider::cuboid(2.0 * (SHELL_X - 0.01), SIDE_TOP - SIDE_BOTTOM, 2.0 * SIDE_Z),
        ),
    ])
}

fn shell_cores(
    mut commands: Commands,
    crawlers: Query<&Children, Added<Crawler>>,
    parts: Query<&RobotPart>,
) {
    for children in &crawlers {
        for c in children.iter() {
            if parts.get(c).is_ok_and(|p| p.role == PartRole::Core) {
                commands.entity(c).try_insert(core_collider());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inside the carapace top and bottom, and nothing above its flat top
    /// (the sphere stood 13 cm over it); in front, behind the eye.
    #[test]
    fn the_core_fits_the_carapace() {
        let c = core_collider();
        let aabb = c.aabb(Vec3::ZERO, Quat::IDENTITY);
        assert!(
            (aabb.max.y - SHELL_TOP).abs() < 1e-3,
            "top {:.3}",
            aabb.max.y
        );
        assert!(
            (aabb.min.y - SHELL_BOTTOM).abs() < 1e-3,
            "bottom {:.3}",
            aabb.min.y
        );
        assert!(aabb.max.x <= SHELL_X && aabb.min.x >= -SHELL_X, "{aabb:?}");
        assert!(aabb.max.z <= SHELL_Z && aabb.min.z >= -SHELL_Z, "{aabb:?}");
        // The eye's front face, at the eye's height: the core stays behind it.
        let eye_front = Vec3::new(0.0, 0.12, -0.39);
        assert!(!c.contains_point(Vec3::ZERO, Quat::IDENTITY, eye_front));
        // Big enough a weak point not to be grown at a crawler's size.
        let scale = super::super::indoor::CRAWLER_SIZE;
        assert!(
            super::super::weak_points::across(&c) * scale >= super::super::weak_points::WEAK_ACROSS,
            "across {:.3}",
            super::super::weak_points::across(&c) * scale
        );
    }
}
