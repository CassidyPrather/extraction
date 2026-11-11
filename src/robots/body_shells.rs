//! Where a walker-bodied machine is hit: its core's hit volume and its
//! plates' places, by its body (`anim::Shape`). The walker keeps its orb;
//! the grappler's winch box and the crawler's carapace (Models'
//! `grappler_core` and `crawler_core`, walker.glb) are hit where they're
//! drawn, not where the walker's orb would be.
//!
//! In walker units, local to the robot (a crawler is ×0.6 on top).
//! Each core keeps the eye proud of it: the visor sits at (0, 0.12,
//! -0.36), its front face at z -0.39, and a round at the eye from straight
//! ahead must meet the eye, not the core in front of it.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::anim::Shape;

/// The grappler's winch box: across (X), tall, deep, and its middle.
pub const GRAPPLER_CORE: Vec3 = Vec3::new(1.10, 0.55, 0.78);
pub const GRAPPLER_CORE_AT: Vec3 = Vec3::new(0.0, -0.025, 0.03);
/// The crawler's carapace as hit: across, tall, deep (z ±0.385, inside
/// the visor; the shell itself runs to ±0.426), and its middle.
pub const CRAWLER_CORE: Vec3 = Vec3::new(1.40, 0.55, 0.77);
pub const CRAWLER_CORE_AT: Vec3 = Vec3::new(0.0, -0.025, 0.0);

/// A core's hit volume for a body (local to the core part, which sits at
/// the robot's middle).
pub fn core(shape: Shape) -> Collider {
    let boxed = |size: Vec3, at: Vec3| {
        Collider::compound(vec![(
            at,
            Quat::IDENTITY,
            Collider::cuboid(size.x, size.y, size.z),
        )])
    };
    match shape {
        Shape::Grappler => boxed(GRAPPLER_CORE, GRAPPLER_CORE_AT),
        Shape::Crawler => boxed(CRAWLER_CORE, CRAWLER_CORE_AT),
        _ => Collider::sphere(0.4),
    }
}

/// A plate's place on a box body, facing out (+Y out, as `armour` lays
/// every plate): plate 0 low on the front, 1 and 2 on the sides. None
/// for the orb (its plates sit on the sphere: `armour::walker_plate_dir`).
pub fn plate(shape: Shape, i: usize) -> Option<Transform> {
    let (front, side) = match shape {
        Shape::Grappler => (Vec3::new(0.0, -0.14, -0.375), Vec3::new(0.565, -0.03, 0.03)),
        Shape::Crawler => (Vec3::new(0.0, -0.17, -0.40), Vec3::new(0.715, -0.03, 0.0)),
        _ => return None,
    };
    let (at, out) = match i {
        0 => (front, Vec3::NEG_Z),
        1 => (side * Vec3::new(-1.0, 1.0, 1.0), Vec3::NEG_X),
        _ => (side, Vec3::X),
    };
    Some(Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::Y, out)))
}

/// Which way a plate faces out, robot space, by body.
pub fn plate_dir(shape: Shape, i: usize) -> Vec3 {
    plate(shape, i).map_or_else(
        || super::armour::walker_plate_dir(i),
        |t| t.rotation * Vec3::Y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The eye stays proud of every body's core, head-on; the box cores
    /// stop at their shells' flat tops; every core is big enough a weak
    /// point not to be grown, at its machine's size.
    #[test]
    fn cores_fit_their_shells() {
        let eye_front = Vec3::new(0.0, 0.12, -0.39);
        for (shape, top, size) in [
            (Shape::Orb, None, 1.0),
            (Shape::Grappler, Some(0.25), 1.0),
            (
                Shape::Crawler,
                Some(0.25),
                super::super::indoor::CRAWLER_SIZE,
            ),
        ] {
            let c = core(shape);
            assert!(
                !c.contains_point(Vec3::ZERO, Quat::IDENTITY, eye_front),
                "{shape:?}'s core is in front of its eye"
            );
            if let Some(top) = top {
                let aabb = c.aabb(Vec3::ZERO, Quat::IDENTITY);
                assert!(aabb.max.y <= top + 1e-3, "{shape:?} top {:.3}", aabb.max.y);
            }
            let across = super::super::weak_points::across(&c) * size;
            assert!(
                across >= super::super::weak_points::WEAK_ACROSS,
                "{shape:?} across {across:.3}"
            );
        }
    }

    /// A box body's plates lie on its faces, facing out, clear of its
    /// core (on it, not in it).
    #[test]
    fn plates_face_out_from_their_faces() {
        for shape in [Shape::Grappler, Shape::Crawler] {
            let c = core(shape);
            for i in 0..3 {
                let t = plate(shape, i).unwrap();
                let out = t.rotation * Vec3::Y;
                assert!(out.dot(t.translation.normalize()) > 0.5, "{shape:?} {i}");
                assert!(
                    !c.contains_point(Vec3::ZERO, Quat::IDENTITY, t.translation + out * 0.02),
                    "{shape:?} plate {i} is inside its core"
                );
            }
        }
        assert!(plate(Shape::Orb, 0).is_none());
    }
}
