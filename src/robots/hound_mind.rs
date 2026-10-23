//! The hound's mind: it hunts by ear.
//!
//! Its eyes are poor (short, narrow); its ears are the best in the Zone.
//! A sound sends it running there, and for the last stretch it drops low
//! and stalks; there it noses round (the walkers' search). It sees you
//! close up, and then it comes round to your side, crouches (the tell:
//! [`super::hound::HoundCrouch`], [`CROUCH_TIME`]), and lunges to bite. So the way to
//! beat a hound is the opposite of the way to beat a drone: don't run, keep
//! still and quiet, and it has nothing to go on. The director sends a pair
//! in answer to gunfire now and then, instead of a drone ([`HOUND_SHARE`]).
//!
//! The behaviour lives in `walker::walker_brain` (a hound is a walker
//! body); this module holds its tunables, state, the bite and its spawn.

use bevy::prelude::*;

use super::hound::Hound;
use super::{Breed, Senses};
use crate::weapons::ballistics::Shot;

/// Sight: a hound sees this far...
pub const SIGHT: f32 = 25.0;
/// ...in this wide a cone (degrees)...
pub const FOV: f32 = 120.0;
/// ...and hears this much better than a walker hears 1.2. It sees
/// movement: keep still and it makes out this share of you (a walker 0.55).
pub const HEARING: f32 = 2.5;
pub const STILL: f32 = 0.2;
/// Its run, as an `Intent::pace` (its gait lopes at 1; `anim::Shape`).
pub const RUN: f32 = 2.25;
/// Running to a sound, it drops to a stalk within this of it (m)...
pub const STALK_WITHIN: f32 = 15.0;
/// ...at this share of its run...
pub const STALK_PACE: f32 = 0.4;
/// ...and noses round at this one.
pub const NOSE_PACE: f32 = 0.25;
/// It comes round to your side from further than this (m)...
pub const FLANK_FROM: f32 = 8.0;
/// ...this far out to the side of you.
pub const FLANK_OFFSET: f32 = 6.0;
/// Within this it crouches to lunge (m)...
pub const LUNGE_RANGE: f32 = 4.5;
/// ...crouching this long first (the tell, s: the body's coil)...
pub const CROUCH_TIME: f32 = super::hound::COIL_TIME;
/// ...then springing at this speed (m/s), past its run...
pub const LUNGE_SPEED: f32 = 7.0;
/// ...for this long...
pub const LUNGE_TIME: f32 = 0.5;
/// ...and biting whatever it reaches within this (m)...
pub const BITE_RANGE: f32 = 1.8;
/// ...this hard...
pub const BITE_DAMAGE: f32 = 16.0;
/// ...then it backs off and comes again after this long (s).
pub const BITE_COOLDOWN: f32 = 2.5;
/// A response to gunfire is a pair of hounds this often, else a drone.
pub const HOUND_SHARE: f32 = 0.35;

/// Where a hound is in its attack.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct HoundMind {
    pub cooldown: f32,
    /// Seconds left crouching before the lunge (the tell).
    pub crouch: f32,
    /// Seconds left of the lunge.
    pub lunge: f32,
    /// It has bitten this lunge.
    pub bit: bool,
    /// The coil's over: spring at this (the body's `hound::lunge`).
    pub spring: Option<Vec3>,
}

/// A hound bit (for its sound and the body's snap).
#[derive(Message, Clone, Copy, Debug)]
pub struct HoundBite {
    pub hound: Entity,
    pub at: Vec3,
}

pub struct HoundMindPlugin;

impl Plugin for HoundMindPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HoundBite>()
            .add_systems(FixedUpdate, wake_hounds);
    }
}

/// A new hound gets its mind: an ear more than an eye.
fn wake_hounds(mut commands: Commands, hounds: Query<(Entity, &Senses), Added<Hound>>) {
    for (e, senses) in &hounds {
        commands.entity(e).insert((
            HoundMind::default(),
            Breed("Hound"),
            super::senses::MotionSight(STILL),
            Senses {
                sight: SIGHT,
                fov_deg: FOV,
                hearing: HEARING,
                ..senses.clone()
            },
        ));
    }
}

/// A hound whose coil is over springs ([`LUNGE_SPEED`], the body's
/// `hound::lunge`): right after the brain, before the gait walks.
pub fn spring_hounds(
    mut hounds: Query<(
        &mut HoundMind,
        &mut super::walker::Walker,
        &mut super::anim::Gait,
        &GlobalTransform,
    )>,
) {
    for (mut h, mut w, mut g, gt) in &mut hounds {
        if let Some(at) = h.spring.take() {
            super::hound::lunge(&mut w, &mut g, gt.translation(), at, LUNGE_SPEED);
        }
    }
}

/// The bite: a short, heavy round from its jaws at whoever it reached.
/// (Shots from a hound make no gunshot noise: `weapons::report`.)
pub fn bite(e: Entity, from: Vec3, target: Vec3, seed: u64, shots: &mut MessageWriter<Shot>) {
    let to = (target - from).normalize_or(Vec3::NEG_Z);
    shots.write(Shot {
        gun: e,
        shooter: Some(e),
        origin: from,
        direction: to,
        speed: 120.0,
        damage: BITE_DAMAGE,
        projectiles: 1,
        spread: 0.0,
        recoil: 0.0,
        seed,
    });
}

/// Where a hound goes to come at you from the side: out to whichever side
/// of your facing it's already on, a little behind you; straight at you
/// once it's close.
pub fn flank_point(me: Vec3, target: Vec3, facing: Vec3) -> Vec3 {
    flank_point_side(me, target, facing, None)
}

/// [`flank_point`] to a given side of you (`Some(1.0)` your right,
/// `Some(-1.0)` your left), for a pack splitting up.
pub fn flank_point_side(me: Vec3, target: Vec3, facing: Vec3, side: Option<f32>) -> Vec3 {
    let d = Vec2::new(target.x - me.x, target.z - me.z).length();
    if d < FLANK_FROM {
        return target;
    }
    let f = Vec3::new(facing.x, 0.0, facing.z).normalize_or(Vec3::NEG_Z);
    let right = Vec3::new(-f.z, 0.0, f.x);
    let side = match side {
        Some(k) => right * k.signum(),
        None if (me - target).dot(right) >= 0.0 => right,
        None => -right,
    };
    target + side * FLANK_OFFSET - f * 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hound_comes_round_to_your_side() {
        let me = Vec3::new(0.0, 0.0, -30.0);
        let target = Vec3::ZERO;
        // You face it, down -Z... it's straight ahead of you.
        let p = flank_point(me, target, Vec3::NEG_Z);
        assert!(p.x.abs() > FLANK_OFFSET * 0.9, "out to a side: {p}");
        assert!(p.z > -0.5, "not in front of you: {p}");
        // A pack splits: one each side.
        let r = flank_point_side(me, target, Vec3::NEG_Z, Some(1.0));
        let l = flank_point_side(me, target, Vec3::NEG_Z, Some(-1.0));
        assert!(r.x * l.x < 0.0, "opposite sides: {r} {l}");
        // Close, it comes straight in.
        assert_eq!(
            flank_point(Vec3::new(0.0, 0.0, -5.0), target, Vec3::NEG_Z),
            target
        );
    }
}
