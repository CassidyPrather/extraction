//! The body under the head: where the local player's pelvis, chest,
//! shoulders, knees and feet are, worked out each frame from the head, the
//! hands and the torso anchor (`interact::body::Torso`, where the belt and
//! the chest rig hang), so the body that's drawn (`render::body`), the
//! arms (`render::arms`) and the body's shadow all agree, and the tests
//! can ask where a knee is without a window.
//!
//! - The chest is the torso anchor: the slots ride it, so the holster sits
//!   on the hip and the pouches on the belly whatever the body does.
//! - The spine runs from the pelvis (under the anchor) up to the neck
//!   (under the head), so bending over or leaning out leans it.
//! - The feet stay planted where they were put down. Turning your head
//!   moves nothing; when the body has turned or moved far enough from
//!   them, a foot steps (one at a time) to where it would stand.
//! - The knees bend by two-bone IK from the hips to the planted ankles:
//!   crouch and they come forward.
//! - Seated (`Comfort::seated`): the lap and the knees where a seated
//!   body's are, below the eyes as they really are (not as lifted).
//! - Reaching out further than the arm, the shoulder comes forward (up to
//!   [`PROTRACT`]); the arm's length is learned from the longest reaches
//!   (within limits), so a long-armed player's sleeves don't pull apart.

use bevy::prelude::*;

use super::comfort::Comfort;
use super::{LocalPlayer, RigHand, RigInput, Side};
use crate::interact::Hand;
use crate::interact::body::Torso;

/// The pelvis (hip joints' middle) under the torso anchor, metres.
pub const PELVIS_BELOW_CHEST: f32 = 0.45;
/// The neck's base under the eyes, and behind them.
const NECK_BELOW_EYES: f32 = 0.2;
const NECK_BEHIND_EYES: f32 = 0.06;
/// The shoulders in the chest's frame, from the neck's base (x right; the
/// left mirrored). With the neck 0.2 m under the eyes, they sit 0.26 m
/// under them, as the arms always had them.
const SHOULDER: Vec3 = Vec3::new(0.17, -0.06, 0.03);
/// Half the hips' width.
pub const HIP: f32 = 0.1;
/// Thigh and shin at a standing eye of [`STAND_EYE`]; scaled with the
/// player's height.
pub const THIGH: f32 = 0.42;
pub const SHIN: f32 = 0.42;
/// The ankle joint over the sole.
pub const ANKLE: f32 = 0.08;
const STAND_EYE: f32 = 1.65;
/// The feet stand this far apart (from the body's middle line).
const STANCE: f32 = 0.12;
/// A foot further than this from where it would stand steps there.
pub const STEP: f32 = 0.3;
/// A foot turned this far (radians) from the way the body faces steps.
pub const TWIST: f32 = 1.0;
/// A foot turned this far from the body for [`SETTLE`] seconds squares up
/// (the body has turned and stayed turned).
pub const SQUARE: f32 = 0.4;
pub const SETTLE: f32 = 1.2;
/// How long a step takes, and how high the foot lifts.
const STEP_TIME: f32 = 0.28;
const STEP_LIFT: f32 = 0.07;
/// Further than this in one frame (a teleport, a climb's top-out) and the
/// feet are put down under the body at once.
const JUMP: f32 = 1.0;
/// How far a shoulder comes forward toward a hand out of reach.
pub const PROTRACT: f32 = 0.1;
/// The arm's reach, shoulder to the grip, at scale 1 (`render::arms`:
/// upper arm, forearm, and the hand to the grip).
pub const ARM: f32 = 0.29 + 0.25 + 0.1;
/// What the arm's length may be scaled to, learned from the reach.
pub const ARM_SCALE: (f32, f32) = (0.92, 1.15);
/// Seated: the lap this far under the real eyes and the knees this far
/// ahead of it.
const SEATED_LAP: f32 = 0.62;
const SEATED_KNEE: f32 = 0.42;
/// The pelvis never goes lower than this over the feet (a deep squat).
const LOWEST: f32 = 0.3;

/// One foot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Foot {
    /// The sole's middle, on the ground (world).
    pub at: Vec3,
    /// Which way the toes point (yaw, radians, as `Transform` turns).
    pub yaw: f32,
    /// Mid-step: from where, to where, how far through (1: planted).
    pub from: Vec3,
    pub to: Vec3,
    pub to_yaw: f32,
    pub t: f32,
    /// How long it's been turned more than [`SQUARE`] from the body.
    pub uneasy: f32,
}

impl Foot {
    pub fn planted(&self) -> bool {
        self.t >= 1.0
    }
}

/// Where the local player's body is, all in world space.
#[derive(Component, Clone, Debug, Default)]
pub struct Stance {
    /// Hips' middle; -Z the way the body faces.
    pub pelvis: Transform,
    /// The chest: the torso anchor's place, turned to lean along the spine.
    pub chest: Transform,
    pub neck: Vec3,
    pub shoulders: [Vec3; 2],
    pub hips: [Vec3; 2],
    pub knees: [Vec3; 2],
    pub ankles: [Vec3; 2],
    pub feet: [Foot; 2],
    /// The learned length of the arms (1: [`ARM`]).
    pub arm: f32,
    /// Legs' length as a share of the standing model's.
    pub legs: f32,
    pub seated: bool,
    /// Set once the body has been placed.
    pub ready: bool,
    last_root: Vec3,
}

impl Stance {
    pub fn shoulder(&self, side: Side) -> Vec3 {
        self.shoulders[side.index()]
    }
}

/// The stance is worked out in `PostUpdate`, after physics has moved the
/// body and before transforms are propagated; the arms and the body are
/// drawn after it.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StanceSystems;

pub struct StancePlugin;

impl Plugin for StancePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, give_stance).add_systems(
            PostUpdate,
            stand
                .in_set(StanceSystems)
                .before(TransformSystems::Propagate),
        );
    }
}

fn give_stance(
    mut commands: Commands,
    players: Query<Entity, (With<LocalPlayer>, Without<Stance>)>,
) {
    for p in &players {
        commands.entity(p).insert(Stance {
            arm: 1.0,
            legs: 1.0,
            ..default()
        });
    }
}

/// The middle joint of a two-bone chain from `a` to `c` (bones `l1`, `l2`),
/// bent toward `hint`. Out of reach, the chain lies straight toward `c`.
pub fn two_bone(a: Vec3, c: Vec3, l1: f32, l2: f32, hint: Vec3) -> Vec3 {
    let to = c - a;
    let d = to.length().clamp(1e-4, l1 + l2 - 1e-4);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let along = (l1 * l1 - l2 * l2 + d * d) / (2.0 * d);
    let up = (l1 * l1 - along * along).max(0.0).sqrt();
    let mut bend = hint - dir * hint.dot(dir);
    if bend.length_squared() < 1e-8 {
        bend = dir.any_orthonormal_vector();
    }
    a + dir * along + bend.normalize() * up
}

/// The yaw (as `Transform` turns: 0 looks down -Z) of a direction.
fn yaw_of(v: Vec3) -> f32 {
    (-v.x).atan2(-v.z)
}

fn wrap(a: f32) -> f32 {
    let mut a = a % std::f32::consts::TAU;
    if a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    if a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// Where a foot stands, under the pelvis, facing with the body.
fn rest(pelvis: &Transform, side: Side, ground: f32) -> Vec3 {
    let p = pelvis.transform_point(Vec3::new(STANCE * side.sign(), 0.0, 0.02));
    Vec3::new(p.x, ground, p.z)
}

/// Feet: planted, or stepping; one steps at a time, the one furthest out.
pub fn step_feet(
    feet: &mut [Foot; 2],
    pelvis: &Transform,
    ground: f32,
    lead: Vec3,
    dt: f32,
    jumped: bool,
) {
    let yaw = yaw_of(pelvis.rotation * Vec3::NEG_Z);
    for side in [Side::Left, Side::Right] {
        let f = &mut feet[side.index()];
        if jumped || f.at == Vec3::ZERO {
            let r = rest(pelvis, side, ground);
            *f = Foot {
                at: r,
                yaw,
                from: r,
                to: r,
                to_yaw: yaw,
                t: 1.0,
                uneasy: 0.0,
            };
            continue;
        }
        if !f.planted() {
            f.t = (f.t + dt / STEP_TIME).min(1.0);
            let s = f.t * f.t * (3.0 - 2.0 * f.t);
            f.at = f.from.lerp(f.to, s) + Vec3::Y * STEP_LIFT * (std::f32::consts::PI * f.t).sin();
            f.yaw += wrap(f.to_yaw - f.yaw) * s;
            if f.planted() {
                f.at = f.to;
                f.yaw = f.to_yaw;
            }
        }
        // The ground may have moved under a planted foot (a slope, a step).
        if f.planted() {
            f.at.y = ground;
        }
    }
    for f in feet.iter_mut() {
        if f.planted() && wrap(f.yaw - yaw).abs() > SQUARE {
            f.uneasy += dt;
        } else {
            f.uneasy = 0.0;
        }
    }
    if feet.iter().any(|f| !f.planted()) {
        return;
    }
    // Which foot has strayed furthest from where it would stand.
    let mut worst: Option<(Side, f32)> = None;
    for side in [Side::Left, Side::Right] {
        let f = &feet[side.index()];
        let r = rest(pelvis, side, ground);
        let off = Vec2::new(f.at.x - r.x, f.at.z - r.z).length() / STEP;
        let twist = wrap(f.yaw - yaw).abs();
        let mut turned = twist / TWIST;
        // Turned a while: square up.
        if f.uneasy > SETTLE {
            turned = turned.max(1.0 + twist);
        }
        let need = off.max(turned);
        if need > 1.0 && worst.is_none_or(|(_, w)| need > w) {
            worst = Some((side, need));
        }
    }
    if let Some((side, _)) = worst {
        let f = &mut feet[side.index()];
        f.from = f.at;
        f.to = rest(pelvis, side, ground) + Vec3::new(lead.x, 0.0, lead.z);
        f.to_yaw = yaw;
        f.t = 0.0;
    }
}

#[allow(clippy::type_complexity)]
fn stand(
    mut players: Query<(&Transform, &RigInput, &Children, &mut Stance), With<LocalPlayer>>,
    torsos: Query<(&Transform, &Torso)>,
    hands: Query<(&RigHand, &Hand)>,
    comfort: Option<Res<Comfort>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let seated = comfort.as_deref().is_some_and(|c| c.seated);
    for (root, input, children, mut st) in &mut players {
        let Some((torso_local, torso)) = children.iter().find_map(|c| torsos.get(c).ok()) else {
            continue;
        };
        let anchor = root.mul_transform(*torso_local);
        let head = root.mul_transform(input.head);
        let ground = root.translation.y;
        let facing = anchor.rotation;
        let fwd = facing * Vec3::NEG_Z;
        let up = Vec3::Y;
        st.legs = (torso.standing / STAND_EYE).clamp(0.8, 1.25);
        let (thigh, shin) = (THIGH * st.legs, SHIN * st.legs);

        // The neck: under and a little behind the eyes, whatever they look at.
        let look = head.rotation * Vec3::NEG_Z;
        let flat = Vec3::new(look.x, 0.0, look.z).normalize_or(fwd);
        let neck = head.translation - up * NECK_BELOW_EYES - flat * NECK_BEHIND_EYES;

        let pelvis_at = if seated {
            // The lap where a seated body's is, under the eyes (the seated
            // view lifts the eyes and the world alike, so this is lifted too).
            Vec3::new(neck.x, head.translation.y - SEATED_LAP, neck.z) - fwd * 0.02
        } else {
            let p = anchor.translation - up * PELVIS_BELOW_CHEST;
            Vec3::new(p.x, p.y.max(ground + LOWEST), p.z)
        };
        let pelvis = Transform::from_translation(pelvis_at).with_rotation(facing);
        let spine = (neck - pelvis_at).normalize_or(up);
        let lean = Quat::from_rotation_arc(up, spine);
        let chest = Transform::from_translation(anchor.translation).with_rotation(lean * facing);
        let chest_side = chest.rotation * Vec3::X;

        let jumped_now = !st.ready || st.last_root.distance(root.translation) > JUMP;
        // Shoulders, then forward toward a hand that's out of reach.
        let mut shoulders = [Vec3::ZERO; 2];
        for side in [Side::Left, Side::Right] {
            shoulders[side.index()] =
                neck + chest.rotation * (SHOULDER * Vec3::new(side.sign(), 1.0, 1.0));
        }
        let mut longest = 0.0f32;
        for c in children.iter() {
            let Ok((rh, hand)) = hands.get(c) else {
                continue;
            };
            let grip = hand.felt.unwrap_or(hand.world).translation;
            let s = &mut shoulders[rh.side.index()];
            let d = s.distance(grip);
            // Learnt from where the hand really is, and only from what an
            // arm could reach (not a hand left behind by a teleport).
            let tracked = s.distance(hand.world.translation) - PROTRACT;
            if tracked < ARM * ARM_SCALE.1 {
                longest = longest.max(tracked);
            }
            let over = d - ARM * st.arm;
            if over > 0.0 {
                *s += (grip - *s).normalize_or_zero() * over.min(PROTRACT);
            }
        }
        // Learn the arm from the longest reaches (a long arm reaches further
        // than the model's could, shoulder forward and all).
        if st.ready && !jumped_now && longest > ARM * st.arm {
            st.arm = (longest / ARM).clamp(ARM_SCALE.0, ARM_SCALE.1);
        }

        let hips = [Side::Left, Side::Right].map(|s| {
            pelvis_at + chest_side.with_y(0.0).normalize_or(facing * Vec3::X) * HIP * s.sign()
        });
        let moved = st.last_root.distance(root.translation);
        let jumped = !st.ready || moved > JUMP;
        let vel = if dt > 0.0 && !jumped {
            (root.translation - st.last_root) / dt
        } else {
            Vec3::ZERO
        };
        st.last_root = root.translation;

        let mut knees = [Vec3::ZERO; 2];
        let mut ankles = [Vec3::ZERO; 2];
        if seated {
            // Thighs ahead, shins down to the feet, the feet on the floor
            // the seated body's on: as far under the lap as a shin is.
            for side in [Side::Left, Side::Right] {
                let i = side.index();
                let knee = hips[i] + fwd * SEATED_KNEE * st.legs + chest_side * 0.03 * side.sign();
                let ankle = knee - up * shin;
                knees[i] = knee;
                ankles[i] = ankle;
                let sole = ankle - up * ANKLE + fwd * 0.02;
                st.feet[i] = Foot {
                    at: sole,
                    yaw: yaw_of(fwd),
                    from: sole,
                    to: sole,
                    to_yaw: yaw_of(fwd),
                    t: 1.0,
                    uneasy: 0.0,
                };
            }
        } else {
            step_feet(&mut st.feet, &pelvis, ground, vel * STEP_TIME, dt, jumped);
            for side in [Side::Left, Side::Right] {
                let i = side.index();
                let foot = st.feet[i];
                let toes = Quat::from_rotation_y(foot.yaw) * Vec3::NEG_Z;
                let ankle = foot.at + up * ANKLE + toes * 0.03;
                // Knees ahead of the toes, a little out.
                let hint = toes + chest_side * 0.15 * side.sign();
                knees[i] = two_bone(hips[i], ankle, thigh, shin, hint);
                ankles[i] = ankle;
            }
        }

        st.pelvis = pelvis;
        st.chest = chest;
        st.neck = neck;
        st.shoulders = shoulders;
        st.hips = hips;
        st.knees = knees;
        st.ankles = ankles;
        st.seated = seated;
        st.ready = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The middle joint keeps both bones' lengths and bends the way it's
    /// told; out of reach, the chain lies straight.
    #[test]
    fn two_bones_keep_their_length_and_bend_where_told() {
        let a = Vec3::new(0.0, 1.0, 0.0);
        let c = Vec3::new(0.0, 0.3, 0.0);
        let k = two_bone(a, c, 0.42, 0.42, Vec3::NEG_Z);
        assert!((k.distance(a) - 0.42).abs() < 1e-3 && (k.distance(c) - 0.42).abs() < 1e-3);
        assert!(k.z < -0.1, "the knee forward: {k}");
        let far = two_bone(a, Vec3::new(0.0, -2.0, 0.0), 0.42, 0.42, Vec3::NEG_Z);
        assert!(far.z.abs() < 0.01 && (far.y - 0.58).abs() < 0.01, "{far}");
    }

    /// A foot stays put until the body's moved or turned far enough from
    /// it, then steps under it; one foot at a time.
    #[test]
    fn feet_stay_planted_until_the_body_leaves_them() {
        let mut feet = [Foot::default(); 2];
        let mut pelvis = Transform::from_xyz(0.0, 0.9, 0.0);
        step_feet(&mut feet, &pelvis, 0.0, Vec3::ZERO, 0.016, true);
        let start = feet;
        // A small shift: nothing moves.
        pelvis.translation.x = 0.15;
        for _ in 0..30 {
            step_feet(&mut feet, &pelvis, 0.0, Vec3::ZERO, 0.016, false);
        }
        assert_eq!(feet, start);
        // Turned past the twist: one foot steps, then the other.
        pelvis.rotation = Quat::from_rotation_y(1.3);
        step_feet(&mut feet, &pelvis, 0.0, Vec3::ZERO, 0.016, false);
        assert_eq!(feet.iter().filter(|f| !f.planted()).count(), 1);
        for _ in 0..120 {
            step_feet(&mut feet, &pelvis, 0.0, Vec3::ZERO, 0.016, false);
        }
        for f in &feet {
            assert!(f.planted() && wrap(f.yaw - 1.3).abs() < 0.05, "{f:?}");
        }
        // A smaller turn: nothing at once, but held, the feet square up.
        pelvis.rotation = Quat::from_rotation_y(1.3 + 0.6);
        for _ in 0..30 {
            step_feet(&mut feet, &pelvis, 0.0, Vec3::ZERO, 0.016, false);
        }
        assert!(
            feet.iter().all(|f| wrap(f.yaw - 1.3).abs() < 0.05),
            "not at once"
        );
        for _ in 0..240 {
            step_feet(&mut feet, &pelvis, 0.0, Vec3::ZERO, 0.016, false);
        }
        for f in &feet {
            assert!(
                f.planted() && wrap(f.yaw - 1.9).abs() < 0.05,
                "squared: {f:?}"
            );
        }
    }
}
