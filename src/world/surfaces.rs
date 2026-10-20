//! What the built things are made of, piece by piece: for the sound of
//! walking on them and of rounds hitting them. Buildings and bridges are
//! one compound collider each, so a hit says which building, not which
//! plank; this finds the piece at the point.

use bevy::prelude::*;

use crate::render::materials::Mat;

/// Solid pieces as boxes in the entity's own frame: (half extents, where,
/// material).
#[derive(Component, Clone, Debug, Default)]
pub struct Surfaces(pub Vec<(Vec3, Transform, Mat)>);

impl Surfaces {
    /// The material of the piece at (or within a few centimetres of) a
    /// world point. Of pieces equally close, the one on top: a floor's
    /// boards lie on its concrete plinth.
    pub fn at(&self, gt: &GlobalTransform, p: Vec3) -> Option<Mat> {
        self.piece_at(gt, p).map(|(.., m)| m)
    }

    /// The piece at (or within a few centimetres of) a world point, as
    /// [`Self::at`] picks it: its half extents, pose and material.
    pub fn piece_at(&self, gt: &GlobalTransform, p: Vec3) -> Option<(Vec3, Transform, Mat)> {
        const PAD: f32 = 0.08;
        let local = gt.affine().inverse().transform_point3(p);
        self.0
            .iter()
            .filter_map(|(half, t, m)| {
                let q = (t.rotation.inverse() * (local - t.translation)).abs();
                let out = (q - *half).max(Vec3::ZERO).length();
                let top = t.translation.y + half.y;
                // Centimetres apart counts as the same distance.
                (out <= PAD).then_some(((out * 50.0).round() as i32, -top, (*half, *t, *m)))
            })
            .min_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)))
            .map(|(.., piece)| piece)
    }
}

/// What's underfoot at a point on the ground, if it's built (a floor, a
/// deck, a stair).
pub fn underfoot(
    spatial: &avian3d::prelude::SpatialQuery,
    built: &Query<(&GlobalTransform, &Surfaces)>,
    feet: Vec3,
) -> Option<Mat> {
    use avian3d::prelude::*;
    let hit = spatial.cast_ray(
        feet + Vec3::Y * 0.3,
        Dir3::NEG_Y,
        0.6,
        true,
        &SpatialQueryFilter::from_mask(super::Layer::World),
    )?;
    let (gt, surf) = built.get(hit.entity).ok()?;
    surf.at(gt, feet + Vec3::Y * (0.3 - hit.distance))
}

/// What a material sounds like underfoot, as a `footsteps/` set.
pub fn footstep_set(m: Mat) -> &'static str {
    match m {
        Mat::Planks | Mat::LogWall | Mat::WoodFloor | Mat::Timber | Mat::Shingles => {
            "footsteps/wood"
        }
        Mat::RustyMetal
        | Mat::PaintedMetal
        | Mat::Corrugated
        | Mat::DarkMetal
        | Mat::RobotShell
        | Mat::GunMetal => "footsteps/metal",
        _ => "footsteps/concrete",
    }
}

/// What a material sounds like when a round hits it, as an `impacts/` set.
pub fn impact_set(m: Mat) -> &'static str {
    match m {
        Mat::Planks
        | Mat::LogWall
        | Mat::WoodFloor
        | Mat::Timber
        | Mat::Shingles
        | Mat::GunWood
        | Mat::BarkSpruce
        | Mat::BarkBirch
        | Mat::BarkPine => "impacts/wood",
        Mat::RustyMetal
        | Mat::PaintedMetal
        | Mat::Corrugated
        | Mat::DarkMetal
        | Mat::RobotShell
        | Mat::GunMetal => "impacts/metal",
        _ => "impacts/concrete",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_piece_at_a_point() {
        let s = Surfaces(vec![
            // A floor slab, its top at y = 0.1.
            (
                Vec3::new(4.0, 0.05, 3.0),
                Transform::from_xyz(0.0, 0.05, 0.0),
                Mat::WoodFloor,
            ),
            // A metal wall turned 90 degrees, along z at x = 4.
            (
                Vec3::new(3.0, 1.5, 0.1),
                Transform::from_xyz(4.0, 1.6, 0.0)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                Mat::Corrugated,
            ),
        ]);
        let gt = GlobalTransform::from(Transform::from_xyz(10.0, 2.0, -5.0));
        let world = |x: f32, y: f32, z: f32| Vec3::new(10.0 + x, 2.0 + y, -5.0 + z);
        assert_eq!(s.at(&gt, world(1.0, 0.1, 1.0)), Some(Mat::WoodFloor));
        assert_eq!(s.at(&gt, world(3.9, 1.5, 2.0)), Some(Mat::Corrugated));
        assert_eq!(s.at(&gt, world(1.0, 3.0, 1.0)), None);
        assert_eq!(s.at(&gt, world(20.0, 0.1, 1.0)), None);
    }

    #[test]
    fn boards_on_a_plinth_sound_like_boards() {
        let s = Surfaces(vec![
            (
                Vec3::new(4.0, 1.0, 3.0),
                Transform::from_xyz(0.0, -0.5, 0.0),
                Mat::Concrete,
            ),
            (
                Vec3::new(3.9, 0.015, 2.9),
                Transform::from_xyz(0.0, 0.515, 0.0),
                Mat::WoodFloor,
            ),
        ]);
        let gt = GlobalTransform::IDENTITY;
        // Standing on the plinth's top, under the boards' visual.
        assert_eq!(s.at(&gt, Vec3::new(1.0, 0.5, 1.0)), Some(Mat::WoodFloor));
        // The plinth's side, below the floor.
        assert_eq!(s.at(&gt, Vec3::new(4.0, -0.5, 1.0)), Some(Mat::Concrete));
    }
}
