//! Tracked fingers: OpenXR hand-tracking joints (Index controllers'
//! finger sensing through SteamVR, or bare hands) boiled down to what the
//! glove can show: how far each finger curls and how far each spreads.
//!
//! Engine-free maths on the 26 joints in OpenXR's order
//! (`XR_EXT_hand_tracking`: palm, wrist, then thumb metacarpal..tip and
//! each finger metacarpal..tip). Each joint's -Z runs along its bone toward
//! the fingertip and +Y out of the back of the hand. Only relative poses
//! are used, so the joints can be in any one space.

use bevy::prelude::*;

/// What the glove takes from tracked fingers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Reflect)]
pub struct Fingers {
    /// Index, middle, ring, little, thumb: 0 straight, 1 a closed fist (as
    /// the glove's own curl).
    pub curl: [f32; 5],
    /// Index to little: radians each finger turns out from its knuckle,
    /// positive toward the thumb.
    pub splay: [f32; 4],
}

pub const JOINTS: usize = 26;
const PALM: usize = 0;
const THUMB: usize = 2;
/// A finger's metacarpal; its proximal, intermediate, distal and tip follow.
const fn finger_base(f: usize) -> usize {
    6 + 5 * f
}

/// How much a closed fist bends a finger in all, radians (the glove's
/// joints: 1.4 + 1.65 + 1.1), and a thumb across the palm.
const FINGER_FULL: f32 = 4.15;
const THUMB_FULL: f32 = 1.6;

/// Curls and spreads from the joints, or `None` if they're degenerate
/// (all at one point, as an untracked hand reports).
pub fn fingers_from_joints(joints: &[Transform; JOINTS]) -> Option<Fingers> {
    let back = (joints[PALM].rotation * Vec3::Y).normalize_or_zero();
    let p = |i: usize| joints[i].translation;
    // Across the palm toward the thumb: index knuckle minus little knuckle.
    let radial = p(finger_base(0)) - p(finger_base(3));
    let radial = (radial - back * radial.dot(back)).normalize_or_zero();
    if back == Vec3::ZERO || radial == Vec3::ZERO {
        return None;
    }
    // A bend from one bone to the next: positive toward the palm, signed
    // about the finger's own hinge (out of the back of the hand, across
    // the finger), so it holds past a right angle and on either hand.
    let bend = |a: Vec3, b: Vec3, along: Vec3| {
        let (a, b) = (a.normalize_or_zero(), b.normalize_or_zero());
        let angle = a.angle_between(b);
        if a.cross(b).dot(back.cross(along)) >= 0.0 {
            angle
        } else {
            -angle
        }
    };
    let mut out = Fingers::default();
    for f in 0..4 {
        let b = finger_base(f);
        let bones: Vec<Vec3> = (0..4).map(|k| p(b + k + 1) - p(b + k)).collect();
        if bones.iter().any(|v| v.length_squared() < 1e-10) {
            return None;
        }
        let total: f32 = (0..3).map(|k| bend(bones[k], bones[k + 1], bones[0])).sum();
        out.curl[f] = (total / FINGER_FULL).clamp(0.0, 1.0);
        // The spread: the first bone's turn off its metacarpal, seen from
        // the back of the hand.
        let flat = |v: Vec3| (v - back * v.dot(back)).normalize_or_zero();
        let (m, q) = (flat(bones[0]), flat(bones[1]));
        let angle = m.angle_between(q);
        out.splay[f] = if (q - m).dot(radial) >= 0.0 {
            angle
        } else {
            -angle
        };
    }
    let thumb: Vec<Vec3> = (0..3).map(|k| p(THUMB + k + 1) - p(THUMB + k)).collect();
    if thumb.iter().any(|v| v.length_squared() < 1e-10) {
        return None;
    }
    let total = bend(thumb[0], thumb[1], thumb[0]).abs() + bend(thumb[1], thumb[2], thumb[0]).abs();
    out.curl[4] = (total / THUMB_FULL).clamp(0.0, 1.0);
    Some(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A hand laid out from curls and spreads, the way a runtime reports
    /// one: palm at `at` turned by `turn`, back of the hand +Y, fingers -Z.
    pub(crate) fn synthetic_hand(
        curl: [f32; 5],
        splay: [f32; 4],
        at: Transform,
    ) -> [Transform; JOINTS] {
        let mut j = [Transform::IDENTITY; JOINTS];
        let bend = [1.4, 1.65, 1.1];
        let lens = [0.045, 0.03, 0.022, 0.018];
        for f in 0..4 {
            let b = finger_base(f);
            // Right hand: the index (thumb side) on -X.
            let knuckle = Vec3::new(-0.03 + 0.02 * f as f32, 0.0, 0.02);
            // The metacarpal runs forward to the knuckle; the finger turns
            // out by its spread (toward the thumb, -X: about +Y) and then
            // curls toward the palm (-Y: about +X... -Z turned down).
            let mut rot = Quat::IDENTITY;
            let mut pos = knuckle + Vec3::new(0.0, 0.0, 0.07);
            j[b] = Transform::from_translation(pos).with_rotation(rot);
            pos = knuckle;
            rot = Quat::from_rotation_y(splay[f]);
            for k in 0..4 {
                j[b + 1 + k] = Transform::from_translation(pos).with_rotation(rot);
                if k < 3 {
                    rot *= Quat::from_rotation_x(-bend[k] * curl[f]);
                }
                pos += rot * Vec3::new(0.0, 0.0, -lens[k]);
            }
        }
        let mut pos = Vec3::new(-0.035, -0.01, 0.06);
        let mut rot = Quat::from_rotation_y(0.6);
        for k in 0..4 {
            j[THUMB + k] = Transform::from_translation(pos).with_rotation(rot);
            if k > 0 && k < 3 {
                rot *= Quat::from_rotation_x(-0.8 * curl[4]);
            }
            pos += rot * Vec3::new(0.0, 0.0, -0.035);
        }
        for t in &mut j {
            *t = at * *t;
        }
        j[PALM] = at;
        j[1] = at * Transform::from_xyz(0.0, 0.0, 0.08);
        j
    }

    /// The curls and spreads a hand was posed with come back out of its
    /// joints, wherever the hand is and however it's turned.
    #[test]
    fn curls_and_spreads_come_back_out_of_the_joints() {
        let at = Transform::from_xyz(0.3, 1.2, -0.4).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            0.7,
            -0.4,
            1.1,
        ));
        for (curl, splay) in [
            ([0.0; 5], [0.0; 4]),
            ([0.0, 0.2, 0.5, 0.9, 0.3], [0.1, 0.0, -0.1, -0.2]),
            ([0.95, 0.95, 0.95, 0.95, 0.9], [0.0; 4]),
        ] {
            let got = fingers_from_joints(&synthetic_hand(curl, splay, at)).unwrap();
            for f in 0..4 {
                assert!(
                    (got.curl[f] - curl[f]).abs() < 0.03,
                    "curl {f}: {got:?} vs {curl:?}"
                );
                assert!(
                    (got.splay[f] - splay[f]).abs() < 0.03,
                    "splay {f}: {got:?} vs {splay:?}"
                );
            }
            assert!(
                (got.curl[4] - curl[4]).abs() < 0.05,
                "thumb: {got:?} vs {curl:?}"
            );
        }
    }

    /// A left hand (the right one mirrored) reads the same: curls toward
    /// the palm, spreads toward its thumb.
    #[test]
    fn a_left_hand_reads_the_same() {
        let curl = [0.1, 0.4, 0.7, 0.3, 0.5];
        let splay = [0.12, 0.0, -0.08, -0.15];
        let mirror = |t: Transform| {
            let q = t.rotation;
            Transform::from_translation(t.translation * Vec3::new(-1.0, 1.0, 1.0))
                .with_rotation(Quat::from_xyzw(q.x, -q.y, -q.z, q.w))
        };
        let left = synthetic_hand(curl, splay, Transform::IDENTITY).map(mirror);
        let got = fingers_from_joints(&left).unwrap();
        for f in 0..4 {
            assert!((got.curl[f] - curl[f]).abs() < 0.03, "curl {f}: {got:?}");
            assert!((got.splay[f] - splay[f]).abs() < 0.03, "splay {f}: {got:?}");
        }
    }

    /// An untracked hand (every joint at one place) gives nothing.
    #[test]
    fn an_untracked_hand_gives_nothing() {
        assert!(fingers_from_joints(&[Transform::IDENTITY; JOINTS]).is_none());
    }
}
