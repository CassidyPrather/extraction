//! Nothing of your own body comes between your eyes and the world: any
//! piece of it drawn (the kit on the torso, the pack on your back, the
//! body and the sleeves when they're shown) that comes within [`NEAR`] of
//! the eye, as it does bending over a table or looking down at your feet,
//! is hidden until it's clear again. Bent over in the 28 October playtest,
//! the chest rig filled the view and the floor couldn't be seen.
//!
//! Only a look: the slots and the pack stay where they are to reach into.

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::VisibilitySystems;
use bevy::ecs::entity::EntityHashSet;
use bevy::prelude::*;

use crate::interact::body::Torso;
use crate::render::arms::ArmPart;
use crate::render::body::BodyPart;
use crate::rig::{LocalPlayer, RigHead};

/// How near the eye a piece of the body may come before it's hidden (m).
pub const NEAR: f32 = 0.2;

pub struct ClearEyesPlugin;

impl Plugin for ClearEyesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            clear_the_eyes
                .after(TransformSystems::Propagate)
                .before(VisibilitySystems::VisibilityPropagate),
        );
    }
}

/// How far `eye` is from the nearest point of a mesh's bounds (`aabb`, in
/// its own space, placed by `gt`), in metres.
pub fn gap(eye: Vec3, gt: &GlobalTransform, aabb: &Aabb) -> f32 {
    let local = gt.affine().inverse().transform_point3(eye);
    let (c, h) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
    let nearest = local.clamp(c - h, c + h);
    gt.transform_point(nearest).distance(eye)
}

#[allow(clippy::type_complexity)]
fn clear_the_eyes(
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: Query<&Children, With<LocalPlayer>>,
    torsos: Query<(), With<Torso>>,
    children: Query<&Children>,
    parts: Query<Entity, Or<(With<BodyPart>, With<ArmPart>)>>,
    mut meshes: Query<(&GlobalTransform, &Aabb, &mut Visibility), With<Mesh3d>>,
    mut hidden: Local<EntityHashSet>,
) {
    let Some((eye, player)) = heads
        .iter()
        .find(|(_, p)| locals.contains(p.parent()))
        .map(|(gt, p)| (gt.translation(), p.parent()))
    else {
        return;
    };
    let mut near = EntityHashSet::default();
    let on_torso = locals
        .get(player)
        .into_iter()
        .flat_map(|c| c.iter())
        .filter(|c| torsos.contains(*c))
        .flat_map(|t| children.iter_descendants(t));
    for e in on_torso.chain(parts.iter()) {
        let Ok((gt, aabb, mut vis)) = meshes.get_mut(e) else {
            continue;
        };
        if gap(eye, gt, aabb) < NEAR {
            vis.set_if_neq(Visibility::Hidden);
            near.insert(e);
        }
    }
    // What was hidden and is clear again shows (the body's and sleeves'
    // own systems set theirs every frame anyway).
    for e in hidden.difference(&near) {
        if let Ok((_, _, mut vis)) = meshes.get_mut(*e) {
            vis.set_if_neq(Visibility::Inherited);
        }
    }
    *hidden = near;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box's gap is to its nearest face, not its middle: the jacket's
    /// middle is at the belly, its top at the chin.
    #[test]
    fn the_gap_is_to_the_nearest_face() {
        let aabb = Aabb::from_min_max(Vec3::new(-0.2, 0.0, -0.1), Vec3::new(0.2, 0.6, 0.1));
        let at = GlobalTransform::from_translation(Vec3::new(0.0, 1.0, 0.0));
        // An eye 10 cm above its top.
        assert!((gap(Vec3::new(0.0, 1.7, 0.0), &at, &aabb) - 0.1).abs() < 1e-4);
        // Inside, nothing.
        assert_eq!(gap(Vec3::new(0.0, 1.3, 0.0), &at, &aabb), 0.0);
        // Scaled, the gap is in the world's metres.
        let big =
            GlobalTransform::from(Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(2.0)));
        assert!((gap(Vec3::new(0.0, 2.4, 0.0), &big, &aabb) - 0.2).abs() < 1e-4);
    }
}
