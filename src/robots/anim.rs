//! How walker bodies move: procedural legs under a heavy orb.
//!
//! The brain (`walker.rs`) says where to go (`Walker::path`) and where to
//! look, how fast and whether to keep low ([`super::Intent`]). This module turns
//! that into a body: velocity that takes time to build and bleed off, a
//! yaw that swings round with some inertia, and feet that stay planted
//! until the place they *should* be has drifted too far, then lift and arc
//! to a new footing on the real ground (the heightfield, and steps, floors
//! and kerbs by a ray down). At most two feet are up at once, never two
//! neighbours, so it steps in diagonal pairs at a walk and one at a time on
//! three legs. Stride and cadence follow speed. The orb rides on a spring:
//! it dips as each foot takes weight, rises a little through a swing, leans
//! into acceleration, tilts with the slope under its feet, and shifts its
//! weight toward the feet it's standing on.
//!
//! Damage shows: a leg below half health hurries its steps (less time
//! bearing weight) and the body dips when it lands, and the walker slows;
//! a dead leg dangles and drags, the body sags and lists toward the gap,
//! and it limps on three.
//!
//! Cost: per walker per tick, two-bone IK for four legs and at most one ray
//! per foot that starts or retargets a step (two per step), and one per
//! dead leg. No allocation.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::walker::{LOWER, LegPoses, UPPER, Walker, WalkerLeg, WalkerStep, WalkerTurret};
use super::{Awareness, Robot, RobotPart};
use crate::world::{Layer, Zone};

pub const LEGS: usize = 4;
pub const BODY_HEIGHT: f32 = 1.15;
pub const HIP_RADIUS: f32 = 0.32;
pub const FOOT_RADIUS: f32 = 1.05;
/// Walking speed at `pace` 1, m/s.
pub const SPEED: f32 = 1.7;
/// How fast it can change speed, m/s². Heavy things take a moment.
const ACCEL: f32 = 2.6;
/// The fastest it turns, rad/s, and how quickly it gets there.
const TURN_RATE: f32 = 1.7;
const TURN_RESPONSE: f32 = 5.0;
const STEP_HEIGHT: f32 = 0.26;
/// A badly hurt leg (half health or less) drags its foot this far through
/// its swing before lifting it.
pub const SCUFF_UNTIL: f32 = 0.4;
/// The hitch as weight comes on the leg it favours: a drop (bob speed,
/// m/s, at a dead leg's worth of hurt) and a roll onto that side (rad/s).
const HITCH_DIP: f32 = 1.4;
const HITCH_ROLL: f32 = 1.3;

/// Kept low (a grappler lying in wait, `Intent::low`), the orb sits this
/// much lower on bent legs.
pub const LOW: f32 = 0.35;

/// The body a gait carries: the walker's orb on four splayed legs, or the
/// hound's long low body, legs fore and aft, built to run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Orb,
    Hound,
    /// The heavy's hull on four long legs at its corners (`heavy`): slow,
    /// long-stepping, every footfall a weight.
    Heavy,
}

impl Shape {
    /// Where leg `i`'s hip is on the body (robot space, at size 1).
    pub fn hip(self, i: usize) -> Vec3 {
        match self {
            Shape::Orb => hip_offset(i),
            Shape::Hound => quadrant(i) * Vec3::new(0.2, 1.0, 0.42) - Vec3::Y * 0.06,
            Shape::Heavy => quadrant(i) * Vec3::new(0.36, 1.0, 0.45) - Vec3::Y * 0.2,
        }
    }
    /// Where foot `i` rests, standing (robot space, at size 1).
    pub fn rest(self, i: usize) -> Vec3 {
        match self {
            Shape::Orb => rest_offset(i),
            Shape::Hound => quadrant(i) * Vec3::new(0.42, 0.0, 0.62),
            Shape::Heavy => quadrant(i) * Vec3::new(0.9, 0.0, 0.9),
        }
    }
    /// How high the body rides, at size 1.
    pub fn height(self) -> f32 {
        match self {
            Shape::Orb => BODY_HEIGHT,
            Shape::Hound => 0.95,
            Shape::Heavy => 1.35,
        }
    }
    /// Speed at `Intent::pace` 1 against a walker's, acceleration and
    /// turning likewise.
    fn pace(self) -> (f32, f32, f32) {
        match self {
            Shape::Orb => (1.0, 1.0, 1.0),
            // ~2 m/s at a lope; pace 2.25 runs it at 4.5.
            Shape::Hound => (1.41, 3.2, 2.2),
            // ~1.3 m/s at size 2: you can outwalk it, just.
            Shape::Heavy => (0.55, 0.45, 0.45),
        }
    }
    /// The most `Intent::pace` it can take (a hound runs flat out at 2.25).
    pub fn max_pace(self) -> f32 {
        match self {
            Shape::Orb => 1.6,
            Shape::Hound => 2.3,
            Shape::Heavy => 1.4,
        }
    }
}

/// Leg `i`'s quadrant: (±1, 0, ±1) in the walker's leg order.
fn quadrant(i: usize) -> Vec3 {
    let r = rest_offset(i);
    Vec3::new(r.x.signum(), 0.0, r.z.signum())
}

/// The body's own state, beside the brain's [`Walker`].
#[derive(Component, Debug, Clone)]
pub struct Gait {
    /// Weight shift and bob, added on top of where the walker "is".
    pub sway: Vec3,
    bob_vel: f32,
    pub yaw_rate: f32,
    /// World-space tilt (scaled axis), smoothed.
    tilt: Vec3,
    /// Hit wobble: a damped angular spring (scaled axis) and its rate.
    pub wobble: Vec3,
    wobble_vel: Vec3,
    /// Each leg's health, 0..1 (0 when it's gone).
    pub leg_health: [f32; LEGS],
    /// How lame it is, 0..1: what slows it down.
    pub limp: f32,
    /// How low it hangs for missing legs, metres (smoothed).
    sag: f32,
    /// Each swing's length in seconds, set at lift-off.
    dur: [f32; LEGS],
    /// Whether a swing has had its mid-flight retarget.
    retargeted: [bool; LEGS],
    /// Where a dead leg's foot hangs (it lags the body).
    dangle: [Vec3; LEGS],
    /// Seconds standing with a foot out of place, before it tidies up.
    idle: f32,
    /// Last tick's velocity, for the lean into acceleration.
    last_v: Vec3,
    pub accel: Vec3,
    /// Toward whatever it's hauling on (a grappler's line), horizontal;
    /// zero when it isn't. It leans back from it and crouches.
    pub brace: Vec3,
    /// Winding up a throw (a grappler's tell): seconds left, and toward
    /// what. It rears back from it, then the throw snaps it forward.
    pub rear: f32,
    pub rear_dir: Vec3,
    /// Settling its turret (a walker's tell): seconds left. It plants and
    /// sinks a little, steadying.
    pub steady: f32,
    /// The body's scale (1 for a walker; a crawler is smaller). The root's
    /// `Transform::scale` must match: hips come from it.
    pub size: f32,
    /// How far into a crouch it is, 0..1 (smoothed `Intent::low`).
    pub crouch: f32,
    /// Coiled to spring (a hound's lunge tell): seconds left, and toward.
    pub coil: f32,
    pub coil_at: Vec3,
    /// Mid-lunge: seconds the gait lets its speed run past its top.
    pub leap: f32,
    /// The body it carries.
    pub shape: Shape,
    /// Seconds alive, for idle motion.
    age: f32,
    /// How rattled it is by recent hits; at 1 it goes down (`daze`).
    pub dazed: f32,
    /// Knocked off its feet: seconds left on the ground, and the tilt it
    /// lies at (scaled axis: over the way the hit pushed it).
    pub floored: f32,
    fall: Vec3,
    /// Getting up: seconds left, feet first, then the body rises on them.
    pub rising: f32,
    /// How far each planted foot has slid on ice since it came down.
    slid: [f32; LEGS],
}

/// Ice: steeper than this (rise over run) a planted foot creeps downhill
/// on it, faster the steeper; once one has gone this far it stumbles.
pub const ICE_GRIP: f32 = 0.25;
pub const SLIP_SPEED: f32 = 1.6;
pub const STUMBLE_AFTER: f32 = 0.12;

/// Feet on ice: `downhill(p)` is the slope under a point as a horizontal
/// vector (rise over run, pointing down it), or none where there's no ice.
/// A planted foot on a slope past `ICE_GRIP` slides down it; one that
/// has slid `STUMBLE_AFTER` jolts the body and steps afresh.
pub fn slip_feet(
    w: &mut Walker,
    g: &mut Gait,
    dt: f32,
    downhill: impl Fn(Vec3) -> Option<Vec3>,
    ground: &mut impl FnMut(Vec3) -> f32,
) {
    for i in 0..LEGS {
        let f = &mut w.feet[i];
        if !f.alive || f.t < 1.0 {
            g.slid[i] = 0.0;
            continue;
        }
        let Some(slope) = downhill(f.planted) else {
            g.slid[i] = 0.0;
            continue;
        };
        let steep = slope.length() - ICE_GRIP;
        if steep <= 0.0 {
            continue;
        }
        let step = slope.normalize_or_zero() * SLIP_SPEED * steep * dt;
        let p = f.planted + step;
        f.planted = Vec3::new(p.x, ground(p), p.z);
        g.slid[i] += step.length();
        if g.slid[i] > STUMBLE_AFTER {
            g.slid[i] = 0.0;
            // A lurch toward the slipping foot; it'll step from where it is.
            g.wobble_vel += Vec3::Y.cross(slope.normalize_or_zero()) * 0.9;
            g.dazed += 0.15;
        }
    }
}

/// Damage that knocks a walker off its feet in one hit (or two quick
/// ones that add up); a smaller body goes down sooner.
pub const KNOCKDOWN_DAMAGE: f32 = 70.0;
/// Seconds on the ground before it gets up.
pub const FLOORED_FOR: f32 = 1.6;
/// Seconds to get up.
pub const RISE_FOR: f32 = 1.2;

impl Default for Gait {
    fn default() -> Self {
        Self {
            sway: Vec3::ZERO,
            bob_vel: 0.0,
            yaw_rate: 0.0,
            tilt: Vec3::ZERO,
            wobble: Vec3::ZERO,
            wobble_vel: Vec3::ZERO,
            leg_health: [1.0; LEGS],
            limp: 0.0,
            sag: 0.0,
            dur: [0.3; LEGS],
            retargeted: [false; LEGS],
            dangle: [Vec3::ZERO; LEGS],
            idle: 0.0,
            last_v: Vec3::ZERO,
            accel: Vec3::ZERO,
            brace: Vec3::ZERO,
            rear: 0.0,
            rear_dir: Vec3::ZERO,
            steady: 0.0,
            size: 1.0,
            crouch: 0.0,
            coil: 0.0,
            coil_at: Vec3::ZERO,
            leap: 0.0,
            shape: Shape::Orb,
            age: 0.0,
            dazed: 0.0,
            floored: 0.0,
            fall: Vec3::ZERO,
            rising: 0.0,
            slid: [0.0; LEGS],
        }
    }
}

impl Gait {
    /// A body `size` times a walker's.
    pub fn sized(size: f32) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }

    /// A hound's body, `size` times a walker's.
    pub fn hound(size: f32) -> Self {
        Self {
            size,
            shape: Shape::Hound,
            ..Default::default()
        }
    }

    /// The fastest it walks (or runs), m/s, before `Intent::pace`.
    pub fn top_speed(&self) -> f32 {
        SPEED * self.size.sqrt() * self.shape.pace().0
    }

    /// A knock to the body (a hit): `impulse` in m/s at the point, roughly.
    pub fn knock(&mut self, at_offset: Vec3, impulse: Vec3) {
        // Torque-ish: a hit high on the orb rocks it more than one low.
        let spin = at_offset.cross(impulse) * 1.6;
        self.wobble_vel += spin.clamp_length_max(2.5);
    }

    /// A hit's weight rattles it: enough at once (`KNOCKDOWN_DAMAGE`,
    /// less for a smaller body) and it goes down, over the way it was
    /// pushed. The heavy doesn't.
    pub fn daze(&mut self, damage: f32, dir: Vec3) {
        if self.shape == Shape::Heavy || self.floored > 0.0 || self.rising > 0.0 {
            return;
        }
        self.dazed += damage / (KNOCKDOWN_DAMAGE * self.size.powf(1.5));
        if self.dazed >= 1.0 {
            self.dazed = 0.0;
            self.floored = FLOORED_FOR;
            let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or(Vec3::X);
            self.fall = Vec3::Y.cross(flat) * 0.35;
        }
    }

    /// Time on the ground and getting up, and the daze wearing off.
    pub fn recover(&mut self, dt: f32) {
        self.dazed = (self.dazed - 0.7 * dt).max(0.0);
        if self.floored > 0.0 {
            self.floored -= dt;
            if self.floored <= 0.0 {
                self.floored = 0.0;
                self.rising = RISE_FOR;
            }
        }
        self.rising = (self.rising - dt).max(0.0);
    }

    /// On the ground (knocked down), not just hurt.
    pub fn down(&self) -> bool {
        self.floored > 0.0
    }
}

pub fn hip_offset(i: usize) -> Vec3 {
    let a = std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2;
    Vec3::new(a.cos() * HIP_RADIUS, -0.12, a.sin() * HIP_RADIUS)
}

pub fn rest_offset(i: usize) -> Vec3 {
    let a = std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2;
    Vec3::new(a.cos() * FOOT_RADIUS, 0.0, a.sin() * FOOT_RADIUS)
}

/// Two-bone IK: the knee, bent toward `pole`.
pub fn solve_knee(hip: Vec3, foot: Vec3, pole: Vec3) -> Vec3 {
    solve_knee_sized(hip, foot, pole, 1.0)
}

/// [`solve_knee`] for legs `size` times the walker's.
pub fn solve_knee_sized(hip: Vec3, foot: Vec3, pole: Vec3, size: f32) -> Vec3 {
    let (upper, lower) = (UPPER * size, LOWER * size);
    let d = (foot - hip)
        .length()
        .clamp(0.05 * size, upper + lower - 0.001);
    let dir = (foot - hip).normalize_or(Vec3::NEG_Y);
    // Distance along hip->foot to the knee's projection, and the knee's offset.
    let a = (upper * upper - lower * lower + d * d) / (2.0 * d);
    let h = (upper * upper - a * a).max(0.0).sqrt();
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Y);
    hip + dir * a + bend * h
}

/// An angle brought into -pi..pi.
pub fn wrap_angle(a: f32) -> f32 {
    wrap(a)
}

fn wrap(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// One tick of what the body is told.
pub struct Drive {
    /// Horizontal velocity the path asks for, at full health and pace 1.
    pub desired: Vec3,
    /// Direction to face (horizontal), if any.
    pub face: Option<Vec3>,
    pub pace: f32,
    /// Stunned, collapsed or wrecked: no walking.
    pub down: bool,
    /// Stunned only: it sags on its legs rather than falling.
    pub stunned: bool,
    /// Keeping low: crouched on bent legs.
    pub low: bool,
}

/// Moves the body and its feet one tick. `ground(p)` is the height of the
/// walkable surface under `p` (probing down from `p.y`); `landed(at)` is
/// called for each foot that comes down.
pub fn step_walker(
    w: &mut Walker,
    g: &mut Gait,
    t: &mut Transform,
    drive: &Drive,
    dt: f32,
    ground: &mut impl FnMut(Vec3) -> f32,
    landed: &mut impl FnMut(Vec3),
) {
    if dt <= 0.0 {
        return;
    }
    let s = g.size;
    let want_crouch = if drive.low && !drive.down { 1.0 } else { 0.0 };
    g.crouch += (want_crouch - g.crouch) * (2.5 * dt).min(1.0);
    g.rear = (g.rear - dt).max(0.0);
    g.steady = (g.steady - dt).max(0.0);
    // Rearing back: up over the wind-up, held at the top.
    let rear_k = if g.rear > 0.0 {
        (1.0 - g.rear / 0.6).clamp(0.0, 1.0).sqrt()
    } else {
        0.0
    };
    let alive_legs = w.feet.iter().filter(|f| f.alive).count();
    let dead: Vec3 = (0..LEGS)
        .filter(|i| !w.feet[*i].alive)
        .map(|i| Vec3::new(g.shape.hip(i).x, 0.0, g.shape.hip(i).z))
        .sum();
    // Lameness: the worst living leg, and anything missing.
    let worst = (0..LEGS)
        .filter(|i| w.feet[*i].alive)
        .map(|i| 1.0 - g.leg_health[i])
        .fold(0.0f32, f32::max);
    let missing = LEGS - alive_legs;
    g.limp = ((worst - 0.4).max(0.0) / 0.6 * 0.5 + missing as f32 * 0.55).min(1.0);
    // The leg it favours: its worst living one, once it's been hit.
    let favoured = (0..LEGS)
        .filter(|i| w.feet[*i].alive && g.leg_health[*i] <= 0.7)
        .min_by(|a, b| g.leg_health[*a].total_cmp(&g.leg_health[*b]));
    // Down to two legs it can't stand: belly on the ground, it hauls
    // itself along on what it has left.
    let crawling = alive_legs == 2 && !drive.down;
    g.coil = (g.coil - dt).max(0.0);
    g.leap = (g.leap - dt).max(0.0);
    let speed_scale = drive.pace.clamp(0.0, g.shape.max_pace())
        * if g.coil > 0.0 { 0.15 } else { 1.0 }
        * (1.0 - 0.6 * g.limp)
        * if g.steady > 0.0 || g.rear > 0.0 {
            0.3
        } else {
            1.0
        }
        * if crawling { 0.5 } else { 1.0 };
    let desired = if drive.down {
        Vec3::ZERO
    } else {
        Vec3::new(drive.desired.x, 0.0, drive.desired.z) * speed_scale
    };

    // Velocity: built up and bled off at a heavy machine's rate.
    let (_, accel_k, turn_k) = g.shape.pace();
    // Mid-lunge it flies on what it launched with.
    let accel_k = if g.leap > 0.0 { 0.0 } else { accel_k };
    let dv = (desired - w.velocity).clamp_length_max(ACCEL * accel_k * dt);
    w.velocity += dv;
    let v = w.velocity;
    let accel = (v - g.last_v) / dt;
    g.last_v = v;
    g.accel = g.accel.lerp(accel, (6.0 * dt).min(1.0));
    let speed = v.length();

    // Yaw with inertia.
    let coil_dir = g.coil_at - (t.translation - g.sway);
    let face = if g.coil > 0.0 {
        coil_dir
    } else {
        drive.face.unwrap_or(v)
    };
    let want_rate = if !drive.down && Vec2::new(face.x, face.z).length() > 0.2 {
        let want = (-face.x).atan2(-face.z);
        (wrap(want - w.yaw) * 2.5).clamp(-TURN_RATE * turn_k, TURN_RATE * turn_k)
    } else {
        0.0
    };
    g.yaw_rate += (want_rate - g.yaw_rate) * (TURN_RESPONSE * dt).min(1.0);
    w.yaw = wrap(w.yaw + g.yaw_rate * dt);

    // Where the walker "is", without last tick's sway.
    let base = t.translation - g.sway;
    let mut next = base + v * dt;

    // Feet.
    let rot = Quat::from_rotation_y(w.yaw);
    // (Cadence by the pace it can reach: a hound's stride lengthens to its run.)
    let run = if g.shape == Shape::Hound { 2.25 } else { 1.0 };
    let top = SPEED * s * g.shape.pace().0 * run;
    let fast = (speed / top).clamp(0.0, 1.5);
    // (A runner's feet are quicker.)
    let step_time = (0.36 - 0.1 * fast)
        * match g.shape {
            Shape::Hound => 0.7,
            // (Long, slow swings: each leg a crane's arm.)
            Shape::Heavy => 1.9,
            Shape::Orb => 1.0,
        };
    let trigger = (0.3 + 0.16 * fast) * s;
    let rot_ahead = Quat::from_rotation_y(w.yaw + g.yaw_rate * step_time * 0.9);
    let lookahead = v * step_time + v.normalize_or_zero() * trigger * 0.45 * fast.min(1.0);
    let probe_y = next.y;
    // Crouched, it spreads its feet: lower and wider, like a spider.
    let spread = s * (1.0 + 0.18 * g.crouch);
    let shape = g.shape;
    let ideal_xz = |i: usize| next + rot_ahead * shape.rest(i) * spread + lookahead;
    let mut kick = 0.0f32;
    for i in 0..LEGS {
        if !w.feet[i].alive || w.feet[i].t >= 1.0 {
            continue;
        }
        let h = g.leg_health[i];
        let mut f = w.feet[i];
        let nt = (f.t + dt / g.dur[i]).min(1.0);
        // Halfway, aim again at where the foot should go now (turns and
        // speed changes since lift-off).
        if nt >= 0.5 && !g.retargeted[i] {
            g.retargeted[i] = true;
            if !drive.down {
                let p = ideal_xz(i);
                f.to = Vec3::new(p.x, ground(Vec3::new(p.x, probe_y, p.z)), p.z);
            }
        }
        f.t = nt;
        let s = nt * nt * (3.0 - 2.0 * nt);
        // Climbing, the foot rises first; stepping down, it travels first.
        let dy = f.to.y - f.from.y;
        let sv = if dy > 0.0 {
            let k = (nt * 1.6).min(1.0);
            k * k * (3.0 - 2.0 * k)
        } else {
            let k = ((nt - 0.3) / 0.7).clamp(0.0, 1.0);
            k * k * (3.0 - 2.0 * k)
        };
        // The bad leg barely clears the ground: it scuffs through the
        // first of its swing, then swings low (fx kicks dirt up off it).
        let scuff = if favoured == Some(i) {
            let late = ((nt - SCUFF_UNTIL) / (1.0 - SCUFF_UNTIL)).clamp(0.0, 1.0);
            (0.3 + 0.7 * h) * if h <= 0.5 { late } else { 1.0 }
        } else {
            1.0
        };
        let lift = (STEP_HEIGHT * s * (0.8 + 0.4 * fast.min(1.0)) * (1.0 - 0.4 * g.crouch)
            + dy.abs() * 0.4)
            * (std::f32::consts::PI * nt).sin()
            * scuff;
        let xz = f.from.lerp(f.to, s);
        f.planted = Vec3::new(xz.x, f.from.y + dy * sv + lift, xz.z);
        if nt >= 1.0 {
            f.planted = f.to;
            landed(f.to);
            // A heavy foot taking weight; a bad leg buckles a little more.
            kick += (0.22 + 0.1 * fast.min(1.0) + (1.0 - h) * 0.5)
                * if g.shape == Shape::Heavy { 1.6 } else { 1.0 };
            // The hitch: weight on the bad leg, it gives; the body drops
            // and rolls onto that side, and comes up off it quick.
            if favoured == Some(i) {
                let hurt = 1.0 - h;
                kick += HITCH_DIP * hurt;
                let hip = g.shape.hip(i);
                let side = Vec3::Y
                    .cross(rot * Vec3::new(hip.x, 0.0, hip.z))
                    .normalize_or_zero();
                g.wobble_vel += side * HITCH_ROLL * hurt;
            }
        }
        w.feet[i] = f;
    }
    // Lift-off: the feet furthest out of place first, never two
    // neighbours at once, at most two up (one on three legs).
    let max_air = if alive_legs == LEGS { 2 } else { 1 };
    let moving = speed > 0.08 || g.yaw_rate.abs() > 0.08;
    let mut need = [0.0f32; LEGS];
    let mut worst_off = 0.0f32;
    for (i, n) in need.iter_mut().enumerate() {
        let f = &w.feet[i];
        if !f.alive || f.t < 1.0 {
            continue;
        }
        let p = ideal_xz(i);
        let off = Vec2::new(f.planted.x - p.x, f.planted.z - p.z).length();
        worst_off = worst_off.max(off);
        // A weak leg hurries off the ground.
        let trig = trigger * (0.6 + 0.4 * g.leg_health[i]);
        *n = off / trig;
    }
    g.idle = if !moving && worst_off > 0.12 {
        g.idle + dt
    } else {
        0.0
    };
    if !drive.down {
        // A foot dragged out near the end of its reach goes now, whatever
        // its neighbours are doing (turning hard at a run can leave one
        // behind otherwise).
        let reach = (UPPER + LOWER) * s;
        let stretched = |i: usize, w: &Walker| {
            let hip = t.transform_point(g.shape.hip(i));
            w.feet[i].alive && w.feet[i].t >= 1.0 && hip.distance(w.feet[i].planted) > reach * 0.85
        };
        for _ in 0..LEGS {
            let air = w.feet.iter().filter(|f| f.alive && f.t < 1.0).count();
            let urgent = (0..LEGS).any(|i| stretched(i, w));
            if air >= max_air + usize::from(urgent) {
                break;
            }
            // The neediest foot that may go.
            let pick = (0..LEGS)
                .filter(|&i| {
                    let neighbours_down = [(i + 1) % LEGS, (i + LEGS - 1) % LEGS]
                        .iter()
                        .all(|&j| !w.feet[j].alive || w.feet[j].t >= 1.0);
                    let ready = need[i] > 1.0 || (g.idle > 0.6 && need[i] * trigger > 0.12 * s);
                    w.feet[i].alive
                        && w.feet[i].t >= 1.0
                        && (ready && neighbours_down || stretched(i, w))
                })
                .max_by(|a, b| need[*a].total_cmp(&need[*b]));
            let Some(i) = pick else {
                break;
            };
            need[i] = 0.0;
            let p = ideal_xz(i);
            let f = &mut w.feet[i];
            f.from = f.planted;
            f.to = Vec3::new(p.x, ground(Vec3::new(p.x, probe_y, p.z)), p.z);
            f.t = 0.0;
            g.retargeted[i] = false;
            g.dur[i] = step_time * (0.7 + 0.3 * g.leg_health[i]);
        }
    }

    // Height: over the feet it stands on, sagging for what's missing.
    let (mut sum, mut n) = (Vec3::ZERO, 0.0f32);
    for f in w.feet.iter().filter(|f| f.alive) {
        let p = if f.t < 1.0 { f.from } else { f.planted };
        sum += p;
        n += 1.0;
    }
    let centroid = if n > 0.0 { sum / n } else { next };
    let under = ground(Vec3::new(next.x, probe_y, next.z));
    let target_sag = s * if drive.down && !drive.stunned {
        g.shape.height() - 0.4
    } else if drive.stunned {
        0.55
    } else if crawling {
        g.shape.height() - 0.45
    } else {
        missing as f32 * 0.14
            + g.limp * 0.05
            + g.brace.length() * 0.12
            + LOW * g.crouch
            + if g.steady > 0.0 { 0.07 } else { 0.0 }
            - rear_k * 0.1
    };
    g.sag += (target_sag - g.sag) * (3.0 * dt).min(1.0);
    let rest_y = if drive.down && !drive.stunned {
        under + g.shape.height() * s
    } else {
        centroid.y.max(under - 0.2 * s) + g.shape.height() * s
    } - g.sag;
    next.y += (rest_y - next.y) * (6.0 * dt).min(1.0);

    // The bob: a spring kicked down by landings, lifted through swings.
    let swing = w
        .feet
        .iter()
        .filter(|f| f.alive && f.t < 1.0)
        .map(|f| (std::f32::consts::PI * f.t).sin())
        .fold(0.0f32, f32::max);
    // Mid-lunge it's up off its legs.
    let bob_target = 0.025 * swing * fast.min(1.0) + if g.leap > 0.0 { 0.12 * s } else { 0.0 };
    g.bob_vel -= kick;
    g.bob_vel += ((bob_target - g.sway.y) * 140.0 - g.bob_vel * 15.0) * dt;
    let bob = (g.sway.y + g.bob_vel * dt).clamp(-0.15 - 0.1 * s, 0.08);

    // Weight over the planted feet.
    let (mut psum, mut pn) = (Vec3::ZERO, 0.0f32);
    for f in w.feet.iter().filter(|f| f.alive && f.t >= 1.0) {
        psum += f.planted;
        pn += 1.0;
    }
    let shift = if pn > 0.0 {
        let c = psum / pn - next;
        Vec3::new(c.x, 0.0, c.z) * 0.3
    } else {
        Vec3::ZERO
    }
    .clamp_length_max(0.07 * s);
    let flat = Vec3::new(g.sway.x, 0.0, g.sway.z).lerp(shift, (5.0 * dt).min(1.0));
    g.sway = Vec3::new(flat.x, bob, flat.z);
    t.translation = next + g.sway;

    // Tilt: half the slope under its feet, into acceleration and speed,
    // and toward a missing leg.
    let mut slope = Vec3::ZERO;
    {
        let (mut sxx, mut sxz, mut szz, mut sxh, mut szh) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut cnt = 0.0;
        for f in w.feet.iter().filter(|f| f.alive) {
            let p = if f.t < 1.0 { f.to } else { f.planted };
            let d = p - centroid;
            sxx += d.x * d.x;
            sxz += d.x * d.z;
            szz += d.z * d.z;
            sxh += d.x * d.y;
            szh += d.z * d.y;
            cnt += 1.0;
        }
        let det = sxx * szz - sxz * sxz;
        if cnt >= 3.0 && det.abs() > 1e-4 {
            let gx = (sxh * szz - szh * sxz) / det;
            let gz = (szh * sxx - sxh * sxz) / det;
            // The surface's normal leans (-gx, 1, -gz).
            slope = Vec3::Y
                .cross(Vec3::new(-gx, 0.0, -gz))
                .clamp_length_max(0.6)
                * 0.5;
        }
    }
    let lean = Vec3::Y.cross(v * 0.035 + g.accel * 0.05);
    let list = Vec3::Y.cross(rot * dead).normalize_or_zero() * 0.14;
    let haul = Vec3::Y.cross(-g.brace) * 0.18 + Vec3::Y.cross(-g.rear_dir) * 0.3 * rear_k;
    // Lame and standing, it can't settle: it rocks onto its bad side and
    // back, and now and then a shudder runs through it.
    let prev_age = g.age;
    g.age += dt;
    let lame = if g.limp > 0.5 && !drive.down {
        ((g.limp - 0.5) * 2.0).min(1.0) * (1.0 - (speed / (0.5 * top)).min(1.0))
    } else {
        0.0
    };
    let bad = if dead != Vec3::ZERO {
        dead
    } else {
        (0..LEGS)
            .filter(|i| w.feet[*i].alive)
            .min_by(|a, b| g.leg_health[*a].total_cmp(&g.leg_health[*b]))
            .map(|i| Vec3::new(g.shape.hip(i).x, 0.0, g.shape.hip(i).z))
            .unwrap_or(Vec3::X)
    };
    let rock =
        Vec3::Y.cross(rot * bad).normalize_or_zero() * (0.05 * (g.age * 2.3).sin() + 0.02) * lame;
    if lame > 0.2 && (prev_age * 0.31).floor() != (g.age * 0.31).floor() {
        let k = (g.age * 12.9898).sin() * 43758.547;
        let j = k - k.floor() - 0.5;
        g.wobble_vel += Vec3::new(j, 0.0, 0.5 - j.abs()) * 1.6 * lame;
    }
    // Coiled: haunches down, nose up toward what it'll spring at.
    let coil = if g.coil > 0.0 {
        Vec3::Y.cross(-Vec3::new(coil_dir.x, 0.0, coil_dir.z).normalize_or_zero()) * 0.22
    } else {
        Vec3::ZERO
    };
    let tilt = if drive.down {
        // Knocked down: it lies over the way it fell.
        if g.floored > 0.0 { g.fall } else { Vec3::ZERO }
    } else {
        slope + lean + list + haul + rock + coil
    };
    g.tilt = g.tilt.lerp(tilt.clamp_length_max(0.4), (4.0 * dt).min(1.0));
    // Wobble: a stiff, underdamped spring.
    g.wobble_vel += (-g.wobble * 90.0 - g.wobble_vel * 7.0) * dt;
    g.wobble += g.wobble_vel * dt;
    g.wobble = g.wobble.clamp_length_max(0.5);
    t.rotation = Quat::from_scaled_axis(g.tilt + g.wobble) * rot;

    // Dead legs hang under their hips and drag along the ground.
    for i in 0..LEGS {
        if w.feet[i].alive {
            continue;
        }
        let h = g.shape.hip(i);
        let hip = t.transform_point(h);
        let hang = hip + rot * (Vec3::new(h.x, 0.0, h.z).normalize_or_zero() * 0.4 * s)
            - Vec3::Y * (UPPER + LOWER) * 0.85 * s;
        if g.dangle[i] == Vec3::ZERO || g.dangle[i].distance(hang) > 2.5 {
            g.dangle[i] = hang;
        }
        let d = g.dangle[i].lerp(hang, (3.0 * dt).min(1.0));
        // (The ground under the body will do: it hangs within a metre.)
        let floor = under + 0.04;
        g.dangle[i] = Vec3::new(d.x, d.y.max(floor), d.z);
    }
}

/// The walkable surface under `p` (probed from `p`, about hip height):
/// steps, floors and kerbs by a ray down (`ray(from, reach)` gives the
/// distance to what it hits), else the terrain. Below the terrain (an
/// underground site) only the ray counts: the floor is down there, not
/// up on the heightfield; with nothing under it, it stays level.
pub fn footing(
    p: Vec3,
    terrain: f32,
    size: f32,
    ray: impl FnOnce(Vec3, f32) -> Option<f32>,
) -> f32 {
    let underground = p.y < terrain - 0.1;
    if p.y > terrain + 0.25 || underground {
        let reach = if underground {
            (UPPER + LOWER) * size * 1.6
        } else {
            p.y - terrain + 0.3
        };
        // Started inside something (a wall): not a footing.
        if let Some(d) = ray(p, reach).filter(|d| *d > 0.02) {
            return p.y - d;
        }
    }
    if underground {
        p.y - BODY_HEIGHT * size
    } else {
        terrain
    }
}

/// Joints for each leg (hip, knee, foot), after [`step_walker`].
pub fn leg_poses(w: &Walker, g: &Gait, t: &Transform) -> [[Vec3; 3]; LEGS] {
    std::array::from_fn(|i| {
        let hip = t.transform_point(g.shape.hip(i));
        let foot = if w.feet[i].alive {
            w.feet[i].planted
        } else {
            g.dangle[i]
        };
        let out = (hip - t.translation).normalize_or(Vec3::X);
        let knee = solve_knee_sized(hip, foot, out + Vec3::Y * 0.8, g.size);
        [hip, knee, foot]
    })
}

/// Body motion along the path, then feet, then IK, for every walker.
#[allow(clippy::type_complexity)]
pub(crate) fn walk(
    mut walkers: Query<(
        Entity,
        &mut Walker,
        &mut Gait,
        &mut Transform,
        &mut LegPoses,
        &Awareness,
        &Children,
        &Robot,
        Option<&super::Intent>,
        Has<super::Stunned>,
    )>,
    mut legs: Query<(&WalkerLeg, &RobotPart, &mut Transform), Without<Walker>>,
    targets: Query<&GlobalTransform>,
    zone: Res<Zone>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut steps: MessageWriter<WalkerStep>,
    snow: Option<Res<crate::world::snow::Snow>>,
) {
    let dt = time.delta_secs();
    // Feet land on what's lying about too: rubble and shards
    // (`destruction`), wreckage, crates and loose things.
    let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Debris, Layer::Item]);
    for (entity, mut w, mut g, mut t, mut poses, aware, children, robot, intent, stunned) in
        &mut walkers
    {
        let pos = t.translation - g.sway;
        // Steering along the path.
        let mut desired = Vec3::ZERO;
        if let Some(next) = w.path.first().copied() {
            let to = Vec3::new(next.x - pos.x, 0.0, next.z - pos.z);
            if to.length() < 0.8 {
                w.path.remove(0);
            } else {
                // Smaller legs, shorter strides, slower.
                desired = to.normalize() * g.top_speed();
            }
        }
        for c in children.iter() {
            if let Ok((leg, part, _)) = legs.get(c) {
                g.leg_health[leg.index] = (part.hp / part.max_hp).clamp(0.0, 1.0);
            }
        }
        let intent = intent.cloned().unwrap_or_default();
        let seen = aware
            .target
            .filter(|_| aware.can_see_target)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() - pos);
        // The one it can see, else where its brain says to look, else
        // where it's going.
        let face = seen.or(intent.look.map(|l| l - pos));
        g.recover(dt);
        let drive = Drive {
            desired,
            face,
            pace: intent.pace,
            down: w.collapsed || robot.wrecked || stunned || g.down(),
            stunned: stunned && !w.collapsed && !robot.wrecked,
            low: intent.low,
        };
        let size = g.size;
        let mut ground = |p: Vec3| {
            footing(p, zone.ground(p.x, p.z), size, |from, reach| {
                spatial
                    .cast_ray(from, Dir3::NEG_Y, reach, true, &filter)
                    .map(|hit| hit.distance)
            })
        };
        let (by, yaw) = (entity, w.yaw);
        let mut landed = |at: Vec3| {
            steps.write(WalkerStep { at, by, yaw });
        };
        step_walker(&mut w, &mut g, &mut t, &drive, dt, &mut ground, &mut landed);
        if let Some(snow) = snow.as_ref().filter(|s| s.cover > 0.05) {
            // Lying snow packs to ice under it: steep ground is treacherous.
            let ice = |p: Vec3| {
                if snow.lies(p.y) < 0.3 {
                    return None;
                }
                let e = 0.4;
                let dx = zone.ground(p.x + e, p.z) - zone.ground(p.x - e, p.z);
                let dz = zone.ground(p.x, p.z + e) - zone.ground(p.x, p.z - e);
                Some(-Vec3::new(dx, 0.0, dz) / (2.0 * e))
            };
            slip_feet(&mut w, &mut g, dt, ice, &mut ground);
        }
        poses.0 = leg_poses(&w, &g, &t);
        // Leg colliders follow the lower segments (local to the root).
        // Built from parts rather than a matrix, so the scale stays exactly
        // one: a capsule scaled by 0.9999999 is rebuilt as a convex hull,
        // every tick, for every leg.
        let inv = t.compute_affine().inverse();
        let inv_rot = t.rotation.inverse();
        for c in children.iter() {
            if let Ok((leg, _, mut lt)) = legs.get_mut(c) {
                let [_, knee, foot] = poses.0[leg.index];
                let mid = (knee + foot) * 0.5;
                let dir = (knee - foot).normalize_or(Vec3::Y);
                *lt = Transform::from_translation(inv.transform_point3(mid))
                    .with_rotation(inv_rot * Quat::from_rotation_arc(Vec3::Y, dir));
            }
        }
    }
}

pub struct AnimPlugin;

impl Plugin for AnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                knock_walkers.after(crate::weapons::ballistics::BallisticsSystems),
                haul_lines.after(super::perceive),
                kick_loose,
                print_snow,
            ),
        );
    }
}

/// A foot coming down in lying snow leaves a print (`world::snow`): the
/// machines' own ring, so walkers can't push the player's trail out.
fn print_snow(
    mut steps: MessageReader<super::walker::WalkerStep>,
    snow: Option<Res<crate::world::snow::Snow>>,
    tracks: Option<ResMut<crate::world::snow::Tracks>>,
) {
    let (Some(snow), Some(mut tracks)) = (snow, tracks) else {
        steps.clear();
        return;
    };
    for step in steps.read() {
        if snow.lies(step.at.y) > 0.25 {
            tracks.add_other(crate::world::snow::Print {
                at: step.at,
                yaw: step.yaw,
                left: false,
                by: step.by,
                fill: 0.0,
            });
        }
    }
}

/// How far round a footfall reaches for loose things, metres.
const KICK_REACH: f32 = 0.32;
/// What a footfall gives them: an impulse, N·s (a light thing skitters, a
/// crate barely shifts), and the most it may add to a speed.
const KICK: f32 = 2.4;
const KICK_MAX: f32 = 2.5;

/// A foot coming down among loose things kicks them aside: away from
/// where it landed and a little up, harder the lighter they are. (Held
/// and seated things are out of the world, on the Stowed layer or
/// switched off.)
fn kick_loose(
    mut steps: MessageReader<super::walker::WalkerStep>,
    spatial: SpatialQuery,
    mut bodies: Query<
        (
            &RigidBody,
            &GlobalTransform,
            &ComputedMass,
            &mut LinearVelocity,
        ),
        (
            Without<RigidBodyDisabled>,
            crate::interact::NotCarried,
            Without<super::RobotPart>,
        ),
    >,
) {
    let filter = SpatialQueryFilter::from_mask([Layer::Item]);
    for step in steps.read() {
        let foot = step.at + Vec3::Y * 0.08;
        let near = spatial.shape_intersections(
            &Collider::sphere(KICK_REACH),
            foot,
            Quat::IDENTITY,
            &filter,
        );
        for e in near {
            let Ok((rb, gt, mass, mut v)) = bodies.get_mut(e) else {
                continue;
            };
            if !rb.is_dynamic() {
                continue;
            }
            let off = gt.translation() - foot;
            let away = Vec3::new(off.x, 0.0, off.z).normalize_or(Vec3::X);
            let dv = (KICK * mass.inverse()).min(KICK_MAX);
            v.0 += (away + Vec3::Y * 0.4).normalize() * dv;
        }
    }
}

/// A grappler hauling on a line leans back from it and crouches; firing
/// the harpoon kicks it back.
#[allow(clippy::too_many_arguments)]
fn haul_lines(
    mut windups: MessageReader<super::grappler::HarpoonWindup>,
    mut aims: MessageReader<super::walker::WalkerAim>,
    mut fired: MessageReader<super::grappler::Harpoon>,
    tethered: Query<(&super::grappler::Tethered, &GlobalTransform)>,
    mut walkers: Query<(Entity, &mut Gait, &GlobalTransform)>,
    targets: Query<&GlobalTransform>,
) {
    for (e, mut g, gt) in &mut walkers {
        let pull = tethered
            .iter()
            .find(|(t, _)| t.by == e)
            .map(|(_, pgt)| pgt.translation() - gt.translation())
            .map(|d| Vec3::new(d.x, 0.0, d.z).normalize_or_zero())
            .unwrap_or(Vec3::ZERO);
        g.brace = g.brace.lerp(pull, 0.08);
    }
    for w in windups.read() {
        let (Ok((_, mut g, gt)), Ok(to)) = (walkers.get_mut(w.from), targets.get(w.to)) else {
            continue;
        };
        let d = to.translation() - gt.translation();
        g.rear = super::grappler::WINDUP;
        g.rear_dir = Vec3::new(d.x, 0.0, d.z).normalize_or_zero();
    }
    for a in aims.read() {
        if let Ok((_, mut g, _)) = walkers.get_mut(a.walker) {
            g.steady = super::walker::AIM_TIME;
        }
    }
    for h in fired.read() {
        let (Ok((_, mut g, gt)), Ok(to)) = (walkers.get_mut(h.from), targets.get(h.to)) else {
            continue;
        };
        let dir = (to.translation() - gt.translation()).normalize_or_zero();
        g.rear = 0.0;
        // The throw: snapped forward, then rocked back by the line.
        g.knock(Vec3::Y * 0.3, dir * 1.2);
    }
}

/// A walker's turret turns to whoever it's after (or where its brain is
/// looking), at a turret's pace, and dips its barrel to them.
#[allow(clippy::type_complexity)]
pub(crate) fn aim_turrets(
    mut walkers: Query<(
        &mut Walker,
        &Transform,
        &Awareness,
        Option<&super::Intent>,
        &Children,
        &Robot,
    )>,
    mut turrets: Query<&mut Transform, (With<WalkerTurret>, Without<Walker>)>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut w, t, aware, intent, children, robot) in &mut walkers {
        if robot.wrecked {
            continue;
        }
        let at = aware
            .target
            .filter(|_| aware.can_see_target)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() + Vec3::Y * 1.1)
            .or(intent.and_then(|i| i.look));
        let local = at.map(|p| t.rotation.inverse() * (p - t.translation));
        let want = local
            .filter(|l| Vec2::new(l.x, l.z).length() > 0.3)
            .map_or(0.0, |l| (-l.x).atan2(-l.z));
        let d = wrap(want - w.turret_yaw);
        w.turret_yaw = wrap(w.turret_yaw + d.clamp(-2.2 * dt, 2.2 * dt));
        let pitch = local
            .map(|l| (l.y - 0.46).atan2(Vec2::new(l.x, l.z).length().max(0.5)))
            .unwrap_or(0.0)
            .clamp(-0.5, 0.35);
        for c in children.iter() {
            if let Ok(mut tt) = turrets.get_mut(c) {
                tt.rotation = Quat::from_rotation_y(w.turret_yaw) * Quat::from_rotation_x(pitch);
            }
        }
    }
}

/// Hits rock a walker's body.
pub(crate) fn knock_walkers(
    mut hits: MessageReader<crate::weapons::ballistics::Hit>,
    parts: Query<&RobotPart>,
    mut walkers: Query<(&mut Gait, &GlobalTransform)>,
) {
    for hit in hits.read() {
        let Ok(part) = parts.get(hit.entity) else {
            continue;
        };
        let Ok((mut g, gt)) = walkers.get_mut(part.robot) else {
            continue;
        };
        let dir = hit.velocity.normalize_or_zero();
        g.knock(
            hit.point - gt.translation(),
            dir * (hit.damage * 0.02).min(0.6),
        );
        g.daze(hit.damage, dir);
    }
}

#[cfg(test)]
mod tests {
    use super::super::walker::Foot;
    use super::*;

    fn walker_at(pos: Vec3, ground: &impl Fn(Vec3) -> f32) -> (Walker, Gait, Transform) {
        let feet = std::array::from_fn(|i| {
            let p = pos + rest_offset(i);
            let f = Vec3::new(p.x, ground(p), p.z);
            Foot {
                planted: f,
                from: f,
                to: f,
                t: 1.0,
                alive: true,
            }
        });
        let w = Walker {
            feet,
            path: Vec::new(),
            repath: 0.0,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            gun_cooldown: 0.0,
            suppressing: false,
            turret_yaw: 0.0,
            phase: 0,
            collapsed: false,
            goal: None,
            aim: 0.0,
            cover: None,
            rethink: 0.0,
            waited: 0.0,
        };
        (w, Gait::default(), Transform::from_translation(pos))
    }

    /// Runs a walker toward `dir` for `secs`; returns distance covered and
    /// the worst foot-placement error and planted-foot slip seen.
    fn run(
        w: &mut Walker,
        g: &mut Gait,
        t: &mut Transform,
        dir: Vec3,
        secs: f32,
        ground: &impl Fn(Vec3) -> f32,
    ) -> (f32, f32, f32) {
        let dt = 1.0 / 90.0;
        let start = t.translation;
        let (mut err, mut slip) = (0.0f32, 0.0f32);
        let mut landings = 0;
        for _ in 0..(secs / dt) as usize {
            let before = w.feet;
            let drive = Drive {
                desired: dir * SPEED,
                face: None,
                pace: 1.0,
                down: false,
                stunned: false,
                low: false,
            };
            let mut gr = |p: Vec3| ground(p);
            let mut landed = |at: Vec3| {
                landings += 1;
                let _ = at;
            };
            step_walker(w, g, t, &drive, dt, &mut gr, &mut landed);
            for i in 0..LEGS {
                let f = w.feet[i];
                assert!(f.planted.is_finite() && t.translation.is_finite());
                if f.alive && f.t >= 1.0 {
                    err = err.max((f.planted.y - ground(f.planted)).abs());
                    if before[i].t >= 1.0 {
                        slip = slip.max(f.planted.distance(before[i].planted));
                    }
                }
            }
            let poses = leg_poses(w, g, t);
            for p in poses.iter().flatten() {
                assert!(p.is_finite(), "NaN in the legs");
            }
        }
        assert!(landings > 4, "it stepped ({landings})");
        let d = t.translation - start;
        (Vec2::new(d.x, d.z).length(), err, slip)
    }

    /// Up and across a 25% slope, every planted foot sits on the ground
    /// (not under it, not hovering) and never slides.
    #[test]
    fn feet_land_on_a_slope_and_stay_put() {
        let slope = |p: Vec3| 0.25 * p.x + 0.1 * p.z;
        for dir in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::new(0.7, 0.0, -0.7)] {
            let start = Vec3::new(0.0, BODY_HEIGHT, 0.0);
            let (mut w, mut g, mut t) = walker_at(start, &slope);
            let (dist, err, slip) = run(&mut w, &mut g, &mut t, dir, 6.0, &slope);
            assert!(dist > 6.0, "walked {dist:.2} m toward {dir}");
            assert!(err < 0.01, "foot off the ground by {err:.3} m toward {dir}");
            assert!(slip < 1e-5, "a planted foot slid {slip:.4} m toward {dir}");
            // The orb rides at its height over its feet.
            let over = t.translation.y - slope(t.translation);
            assert!((over - BODY_HEIGHT).abs() < 0.3, "body at {over:.2} m");
        }
    }

    /// A crawler (a 0.6 walker) walks the same slope: feet on the ground,
    /// never sliding, legs never stretched past their reach, the orb at
    /// its own height.
    #[test]
    fn a_small_walker_walks_too() {
        let slope = |p: Vec3| 0.2 * p.x - 0.1 * p.z;
        let size = 0.6;
        let start = Vec3::new(0.0, BODY_HEIGHT * size, 0.0);
        let (mut w, _, mut t) = walker_at(start, &slope);
        for i in 0..LEGS {
            let p = start + rest_offset(i) * size;
            let f = Vec3::new(p.x, slope(p), p.z);
            w.feet[i].planted = f;
            w.feet[i].from = f;
            w.feet[i].to = f;
        }
        let mut g = Gait::sized(size);
        t.scale = Vec3::splat(size);
        let (dist, err, slip) = run(&mut w, &mut g, &mut t, Vec3::X, 6.0, &slope);
        assert!(dist > 3.0, "walked {dist:.2} m");
        assert!(err < 0.01 && slip < 1e-5, "err {err:.3} slip {slip:.4}");
        let over = t.translation.y - slope(t.translation);
        assert!(
            (over - BODY_HEIGHT * size).abs() < 0.2,
            "body at {over:.2} m"
        );
        for [hip, knee, foot] in leg_poses(&w, &g, &t) {
            assert!(hip.distance(foot) <= (UPPER + LOWER) * size + 0.01);
            assert!((hip.distance(knee) - UPPER * size).abs() < 1e-3);
        }
    }

    /// Feet meet what's lying about: walking over a block of rubble
    /// (Destruction's debris layer), every foot that comes down on it
    /// stands on its top, neither floating nor sunk.
    #[test]
    fn feet_stand_on_rubble() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(40.0, 230.0);
        let block = Vec3::new(at.x + 2.6, 0.0, at.z);
        let base = zone.ground(block.x, block.z);
        let half = Vec3::new(0.7, 0.15, 0.9);
        let top = base + 2.0 * half.y;
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(2.0 * half.x, 2.0 * half.y, 2.0 * half.z),
            CollisionLayers::new(Layer::Debris, [Layer::World]),
            Transform::from_xyz(block.x, base + half.y, block.z),
        ));
        let walker = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::walker::spawn_walker(
                &mut c,
                &zone,
                at,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            );
            c.entity(e)
                .remove::<super::super::Senses>()
                .remove::<super::super::Brain>();
            q.apply(app.world_mut());
            e
        };
        for _ in 0..20 {
            app.update();
        }
        app.world_mut().get_mut::<Walker>(walker).unwrap().path =
            vec![Vec3::new(at.x + 6.0, 0.0, at.z)];
        let mut on_top = 0;
        for _ in 0..(90 * 8) {
            app.update();
            let w = app.world().get::<Walker>(walker).unwrap();
            for f in &w.feet {
                let inside = (f.planted.x - block.x).abs() < half.x - 0.08
                    && (f.planted.z - block.z).abs() < half.z - 0.08;
                if f.alive && f.t >= 1.0 && inside {
                    assert!(
                        (f.planted.y - top).abs() < 0.06,
                        "a foot on the block at {:.2} m from its top",
                        f.planted.y - top
                    );
                    on_top += 1;
                }
            }
        }
        assert!(
            on_top > 30,
            "it walked over the block ({on_top} foot-ticks on it)"
        );
    }

    /// A heavy hit knocks a walker off its feet, over the way it was
    /// pushed; it lies a moment and gets up on its legs (not snapped back
    /// up). A light hit only rocks it.
    #[test]
    fn a_heavy_hit_floors_a_walker_and_it_gets_up() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(40.0, 230.0);
        let walker = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::walker::spawn_walker(
                &mut c,
                &zone,
                at,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            );
            c.entity(e)
                .remove::<super::super::Senses>()
                .remove::<super::super::Brain>();
            q.apply(app.world_mut());
            e
        };
        for _ in 0..90 {
            app.update();
        }
        // A core that can take it (this is about the knock, not the kill).
        let core = app
            .world_mut()
            .query::<(Entity, &mut RobotPart)>()
            .iter_mut(app.world_mut())
            .find(|(_, p)| p.robot == walker && p.role == super::super::PartRole::Core)
            .map(|(e, mut p)| {
                p.hp = 1.0e6;
                p.max_hp = 1.0e6;
                e
            })
            .unwrap();
        let y = |app: &App| app.world().get::<Transform>(walker).unwrap().translation.y;
        let standing = y(&app);
        let hit = |app: &mut App, damage: f32| {
            let p = app.world().get::<Transform>(walker).unwrap().translation;
            app.world_mut()
                .write_message(crate::weapons::ballistics::Hit {
                    entity: core,
                    point: p,
                    normal: Vec3::NEG_X,
                    velocity: Vec3::X * 600.0,
                    damage,
                    shooter: None,
                    mass_g: 9.0,
                    load: Default::default(),
                    blow: None,
                });
        };
        // A light round: it rocks, it stays up.
        hit(&mut app, 20.0);
        for _ in 0..45 {
            app.update();
            assert!(!app.world().get::<Gait>(walker).unwrap().down());
        }
        for _ in 0..180 {
            app.update();
        }
        // A heavy one: down.
        hit(&mut app, 90.0);
        let mut lowest = standing;
        for _ in 0..90 {
            app.update();
            lowest = lowest.min(y(&app));
        }
        let g = app.world().get::<Gait>(walker).unwrap();
        assert!(g.down(), "it went down");
        assert!(
            standing - lowest > 0.4,
            "it dropped {:.2} m",
            standing - lowest
        );
        // It lies over away from the shot (pushed along +x: its top goes +x).
        let up = app.world().get::<Transform>(walker).unwrap().rotation * Vec3::Y;
        assert!(up.x > 0.1, "it fell the way it was hit ({up:?})");
        // Then up again, rising (not snapping) to where it stood.
        let mut prev = y(&app);
        let mut biggest_rise = 0.0f32;
        for _ in 0..(90 * 4) {
            app.update();
            let now = y(&app);
            assert!(now.is_finite());
            biggest_rise = biggest_rise.max(now - prev);
            prev = now;
        }
        let g = app.world().get::<Gait>(walker).unwrap();
        assert!(!g.down() && g.rising == 0.0);
        assert!(
            (y(&app) - standing).abs() < 0.08,
            "back up ({:.2} m off)",
            y(&app) - standing
        );
        assert!(
            biggest_rise < 0.05,
            "it rose, not jumped ({biggest_rise:.3} m in a tick)"
        );
        let w = app.world().get::<Walker>(walker).unwrap();
        assert!(
            w.feet.iter().all(|f| f.alive && f.t >= 1.0),
            "standing on its feet"
        );
    }

    /// Loose things in a walker's way get kicked aside as its feet come
    /// down among them: some move, none goes flying.
    #[test]
    fn feet_kick_loose_things_aside() {
        let moved = kicked(|_| None).iter().filter(|d| **d > 0.1).count();
        assert!(moved >= 2, "{moved} of 12 moved");
    }

    /// The same walk, with things carried among them (in a hand, a
    /// socket, a pack: `interact::NotCarried`): those aren't kicked.
    #[test]
    fn feet_leave_carried_things_alone() {
        let d = kicked(|k| match k % 4 {
            1 => Some(Carry::Held),
            2 => Some(Carry::Socket),
            3 => Some(Carry::Pack),
            _ => None,
        });
        for (k, d) in d.iter().enumerate() {
            if k % 4 != 0 {
                assert!(*d < 0.05, "carried thing {k} kicked {d:.2} m");
            }
        }
        assert!(
            d.iter().step_by(4).any(|d| *d > 0.1),
            "the loose ones still are: {d:?}"
        );
    }

    #[derive(Clone, Copy)]
    enum Carry {
        Held,
        Socket,
        Pack,
    }

    /// A walker walks through twelve kits (the `k`th carried as `carry`
    /// says): how far each moved.
    fn kicked(carry: impl Fn(usize) -> Option<Carry>) -> Vec<f32> {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let kind = catalog.kind("cleaning_kit").unwrap();
        let at = zone.on_ground(40.0, 230.0);
        let mut items = Vec::new();
        for k in 0..12 {
            let p = Vec3::new(
                at.x + 1.0 + 0.35 * k as f32,
                0.0,
                at.z + if k % 2 == 0 { 0.45 } else { -0.45 },
            );
            let p = zone.on_ground(p.x, p.z) + Vec3::Y * 0.1;
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            items.push(crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(p),
                    condition: 1.0,
                },
            ));
            q.apply(app.world_mut());
            let e = *items.last().unwrap();
            match carry(k) {
                Some(Carry::Held) => {
                    app.world_mut().entity_mut(e).insert(crate::interact::Held {
                        player: e,
                        hand: e,
                        side: crate::rig::Side::Left,
                        point: 0,
                        offset: Transform::IDENTITY,
                    });
                }
                Some(Carry::Socket) => {
                    app.world_mut()
                        .entity_mut(e)
                        .insert(crate::interact::InSocket {
                            socket: Entity::PLACEHOLDER,
                        });
                }
                Some(Carry::Pack) => {
                    app.world_mut()
                        .entity_mut(e)
                        .insert(crate::interact::body::InBackpack {
                            bag: Entity::PLACEHOLDER,
                        });
                }
                None => {}
            }
        }
        let walker = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::walker::spawn_walker(
                &mut c,
                &zone,
                at,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            );
            c.entity(e)
                .remove::<super::super::Senses>()
                .remove::<super::super::Brain>();
            q.apply(app.world_mut());
            e
        };
        // Let the things come to rest first.
        for _ in 0..90 {
            app.update();
        }
        let start: Vec<Vec3> = items
            .iter()
            .map(|e| {
                app.world()
                    .get::<GlobalTransform>(*e)
                    .unwrap()
                    .translation()
            })
            .collect();
        app.world_mut().get_mut::<Walker>(walker).unwrap().path =
            vec![Vec3::new(at.x + 7.0, 0.0, at.z)];
        for _ in 0..(90 * 8) {
            app.update();
        }
        items
            .iter()
            .zip(&start)
            .map(|(e, s0)| {
                let p = app
                    .world()
                    .get::<GlobalTransform>(*e)
                    .unwrap()
                    .translation();
                assert!(p.is_finite());
                let d = p.xz().distance(s0.xz());
                assert!(d < 6.0, "a kit flew {d:.1} m");
                d
            })
            .collect()
    }

    /// A walker in lying snow leaves prints where its feet come down, in
    /// the machines' own ring (the player's trail is left alone); with no
    /// snow, none.
    #[test]
    fn walkers_print_the_snow() {
        let walk = |snowy: bool| {
            // Rime Fell's col lies above its snow line; the training yard
            // has none.
            let mut app = crate::headless_app(
                crate::LaunchOptions {
                    robots: false,
                    map: if snowy { "rime_fell" } else { "training_yard" }.into(),
                    map_explicit: true,
                    ..Default::default()
                },
                false,
            );
            app.finish();
            app.cleanup();
            app.update();
            let zone = app.world().resource::<crate::world::Zone>().clone();
            let at = if snowy {
                zone.on_ground(70.0, -262.0)
            } else {
                zone.on_ground(40.0, 230.0)
            };
            let walker = {
                let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
                let mut c = q.get_mut(app.world_mut()).unwrap();
                let e = super::super::walker::spawn_walker(
                    &mut c,
                    &zone,
                    at,
                    super::super::Job {
                        kind: super::super::JobKind::Survey,
                        site: at,
                        duration: 999.0,
                        exit: at,
                    },
                );
                c.entity(e)
                    .remove::<super::super::Senses>()
                    .remove::<super::super::Brain>();
                q.apply(app.world_mut());
                e
            };
            for _ in 0..20 {
                app.update();
            }
            app.world_mut().get_mut::<Walker>(walker).unwrap().path =
                vec![Vec3::new(at.x + 5.0, 0.0, at.z)];
            for _ in 0..(90 * 4) {
                app.update();
            }
            let tracks = app.world().resource::<crate::world::snow::Tracks>();
            let mine: Vec<Vec3> = tracks
                .others()
                .filter(|p| p.by == walker)
                .map(|p| p.at)
                .collect();
            (mine, tracks.prints().count(), at)
        };
        let (prints, theirs, at) = walk(true);
        assert!(prints.len() >= 6, "{} prints in the snow", prints.len());
        assert_eq!(theirs, 0, "none in the player's trail");
        assert!(prints.iter().all(|p| p.distance(at) < 8.0 && p.is_finite()));
        assert!(walk(false).0.is_empty(), "no snow, no prints");
    }

    /// Underground, below the terrain, footing is the floor the ray finds,
    /// never the heightfield overhead.
    #[test]
    fn footing_underground_is_the_floor() {
        let terrain = 20.0;
        // A floor 6 m under the terrain, the probe 1.1 m above it.
        let p = Vec3::new(0.0, 15.1, 0.0);
        assert!((footing(p, terrain, 1.0, |_, _| Some(1.1)) - 14.0).abs() < 1e-4);
        // Nothing under it: stay level rather than jump to the surface.
        let h = footing(p, terrain, 1.0, |_, _| None);
        assert!(h < terrain - 1.0, "{h}");
        // On the surface, a kerb is found; nothing found is the terrain.
        let q = Vec3::new(0.0, 21.2, 0.0);
        assert!((footing(q, terrain, 1.0, |_, _| Some(1.0)) - 20.2).abs() < 1e-4);
        assert_eq!(footing(q, terrain, 1.0, |_, _| None), terrain);
        // A ray that starts inside a wall isn't a footing.
        assert_eq!(footing(q, terrain, 1.0, |_, _| Some(0.0)), terrain);
    }

    /// Kept low, it crouches: the orb sinks toward `LOW` lower and its feet
    /// spread, and it still walks on the ground.
    #[test]
    fn keeping_low_crouches_and_spreads() {
        let flat = |_: Vec3| 0.0;
        let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &flat);
        for _ in 0..(90 * 4) {
            let drive = Drive {
                desired: Vec3::X * SPEED,
                face: None,
                pace: 0.6,
                down: false,
                stunned: false,
                low: true,
            };
            step_walker(
                &mut w,
                &mut g,
                &mut t,
                &drive,
                1.0 / 90.0,
                &mut |_| 0.0,
                &mut |_| {},
            );
        }
        assert!(g.crouch > 0.95);
        assert!(
            t.translation.y < BODY_HEIGHT - LOW * 0.8,
            "orb at {:.2}",
            t.translation.y
        );
        let spread: f32 = w
            .feet
            .iter()
            .map(|f| {
                Vec2::new(f.planted.x - t.translation.x, f.planted.z - t.translation.z).length()
            })
            .sum::<f32>()
            / LEGS as f32;
        assert!(spread > FOOT_RADIUS * 1.05, "feet {spread:.2} m out");
        assert!(
            w.feet
                .iter()
                .filter(|f| f.t >= 1.0)
                .all(|f| f.planted.y.abs() < 0.01)
        );
    }

    /// Lame and standing, it never quite settles: its tilt keeps moving.
    #[test]
    fn a_lame_walker_sways_where_it_stands() {
        let flat = |_: Vec3| 0.0;
        let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &flat);
        w.feet[3].alive = false;
        g.leg_health[3] = 0.0;
        g.leg_health[1] = 0.3;
        let mut tilts = Vec::new();
        for k in 0..(90 * 6) {
            let drive = Drive {
                desired: Vec3::ZERO,
                face: None,
                pace: 1.0,
                down: false,
                stunned: false,
                low: false,
            };
            step_walker(
                &mut w,
                &mut g,
                &mut t,
                &drive,
                1.0 / 90.0,
                &mut |_| 0.0,
                &mut |_| {},
            );
            if k > 180 {
                tilts.push((t.rotation * Vec3::Y).angle_between(Vec3::Y));
            }
        }
        let (lo, hi) = tilts
            .iter()
            .fold((f32::MAX, 0.0f32), |(a, b), v| (a.min(*v), b.max(*v)));
        assert!(hi - lo > 0.04, "it rocks: tilt {lo:.3}..{hi:.3}");
        assert!(t.translation.is_finite());
    }

    /// A hound runs: up to speed (over 3 m/s), on a slope, its feet on the
    /// ground and never sliding, never past its legs' reach, trotting
    /// (diagonal pairs) rather than walking one foot at a time.
    #[test]
    fn a_hound_runs() {
        let slope = |p: Vec3| 0.12 * p.x + 0.05 * p.z;
        let size = 0.7;
        let shape = Shape::Hound;
        let start = Vec3::new(0.0, shape.height() * size, 0.0);
        let (mut w, _, mut t) = walker_at(start, &slope);
        for i in 0..LEGS {
            let p = start + shape.rest(i) * size;
            let f = Vec3::new(p.x, slope(p), p.z);
            w.feet[i].planted = f;
            w.feet[i].from = f;
            w.feet[i].to = f;
        }
        // Facing away from where it's sent: it has to turn as it goes.
        w.yaw = 0.0;
        let mut g = Gait::hound(size);
        t.scale = Vec3::splat(size);
        let dt = 1.0 / 90.0;
        let (mut err, mut slip, mut pairs, mut fastest) = (0.0f32, 0.0f32, 0, 0.0f32);
        let mut landings = 0;
        for _ in 0..(90 * 6) {
            let before = w.feet;
            let drive = Drive {
                desired: Vec3::X * g.top_speed(),
                face: None,
                // Flat out.
                pace: 2.25,
                down: false,
                stunned: false,
                low: false,
            };
            step_walker(
                &mut w,
                &mut g,
                &mut t,
                &drive,
                dt,
                &mut |p| slope(p),
                &mut |_| landings += 1,
            );
            fastest = fastest.max(w.velocity.length());
            let up: Vec<usize> = (0..LEGS).filter(|i| w.feet[*i].t < 1.0).collect();
            if up.len() == 2 && (up[1] - up[0]) == 2 {
                pairs += 1;
            }
            for i in 0..LEGS {
                let f = w.feet[i];
                if f.t >= 1.0 {
                    err = err.max((f.planted.y - slope(f.planted)).abs());
                    if before[i].t >= 1.0 {
                        slip = slip.max(f.planted.distance(before[i].planted));
                    }
                }
            }
            for [hip, _, foot] in leg_poses(&w, &g, &t) {
                assert!(hip.is_finite() && foot.is_finite());
                assert!(
                    hip.distance(foot) <= (UPPER + LOWER) * size + 0.05,
                    "a foot left behind: {:.2} m from its hip",
                    hip.distance(foot)
                );
            }
        }
        assert!(fastest > 4.2, "ran at {fastest:.2} m/s");
        assert!(err < 0.01 && slip < 1e-5, "err {err:.3} slip {slip:.5}");
        assert!(pairs > 90, "trotted in diagonal pairs ({pairs} ticks)");
        assert!(landings > 20);
    }

    /// A step up: feet find the top of it (the ground function stands in
    /// for the ray onto a kerb or a porch).
    #[test]
    fn feet_find_a_step() {
        let step = |p: Vec3| if p.x > 2.0 { 0.3 } else { 0.0 };
        let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &step);
        let (_, err, _) = run(&mut w, &mut g, &mut t, Vec3::X, 5.0, &step);
        assert!(err < 0.01, "{err}");
        assert!(w.feet.iter().all(|f| f.planted.y > 0.29), "all up the step");
        assert!(t.translation.y > 0.3 + BODY_HEIGHT - 0.15);
    }

    /// A shot-up leg slows it, a lost one more.
    #[test]
    fn a_damaged_leg_slows_the_gait() {
        let flat = |_: Vec3| 0.0;
        let go = |setup: &dyn Fn(&mut Walker, &mut Gait)| {
            let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &flat);
            setup(&mut w, &mut g);
            run(&mut w, &mut g, &mut t, Vec3::Z, 8.0, &flat).0
        };
        let whole = go(&|_, _| {});
        let hurt = go(&|_, g| g.leg_health[1] = 0.2);
        let lame = go(&|w, g| {
            w.feet[1].alive = false;
            g.leg_health[1] = 0.0;
        });
        assert!(hurt < whole * 0.9, "hurt {hurt:.2} vs whole {whole:.2}");
        assert!(lame < hurt * 0.85, "lame {lame:.2} vs hurt {hurt:.2}");
    }

    /// On three legs, only one foot is ever off the ground.
    #[test]
    fn three_legs_step_one_at_a_time() {
        let flat = |_: Vec3| 0.0;
        let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &flat);
        w.feet[2].alive = false;
        for _ in 0..600 {
            let drive = Drive {
                desired: Vec3::X * SPEED,
                face: None,
                pace: 1.0,
                down: false,
                stunned: false,
                low: false,
            };
            step_walker(
                &mut w,
                &mut g,
                &mut t,
                &drive,
                1.0 / 90.0,
                &mut |_| 0.0,
                &mut |_| {},
            );
            let up = w.feet.iter().filter(|f| f.alive && f.t < 1.0).count();
            assert!(up <= 1, "{up} feet up on three legs");
        }
    }

    /// On two legs it can't stand: it drags itself along on its belly,
    /// slowly, one foot reaching at a time, its feet on the ground.
    #[test]
    fn two_legs_crawl() {
        let flat = |_: Vec3| 0.0;
        for dead in [[0, 1], [0, 2]] {
            let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &flat);
            for i in dead {
                w.feet[i].alive = false;
            }
            for _ in 0..(10 * 90) {
                let drive = Drive {
                    desired: Vec3::X * SPEED,
                    face: None,
                    pace: 1.0,
                    down: false,
                    stunned: false,
                    low: false,
                };
                step_walker(
                    &mut w,
                    &mut g,
                    &mut t,
                    &drive,
                    1.0 / 90.0,
                    &mut |_| 0.0,
                    &mut |_| {},
                );
                assert!(t.translation.is_finite());
                for f in w.feet.iter().filter(|f| f.alive && f.t >= 1.0) {
                    assert!(f.planted.y.abs() < 0.01, "a planted foot off the ground");
                }
            }
            assert!(
                t.translation.y < 0.75,
                "{dead:?}: belly down ({:.2} m)",
                t.translation.y
            );
            let went = t.translation.x;
            assert!(
                (1.0..6.0).contains(&went),
                "{dead:?}: it crawled {went:.2} m in 10 s"
            );
        }
    }

    /// A walker on a bad leg hitches: each time weight comes on that leg
    /// the body drops and rolls more than a whole one's does, and the bad
    /// foot scuffs along the ground through the first of its swing.
    #[test]
    fn a_bad_leg_hitches_and_scuffs() {
        let walk = |health: f32| {
            let flat = |_: Vec3| 0.0;
            let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &flat);
            g.leg_health[1] = health;
            let (mut low, mut high, mut roll) = (f32::MAX, f32::MIN, 0.0f32);
            let mut scuffed = (0, 0);
            for k in 0..(8 * 90) {
                let drive = Drive {
                    desired: Vec3::X * SPEED,
                    face: None,
                    pace: 1.0,
                    down: false,
                    stunned: false,
                    low: false,
                };
                step_walker(
                    &mut w,
                    &mut g,
                    &mut t,
                    &drive,
                    1.0 / 90.0,
                    &mut |_| 0.0,
                    &mut |_| {},
                );
                // Past the start-up, measure the bob and the roll.
                if k > 2 * 90 {
                    low = low.min(t.translation.y);
                    high = high.max(t.translation.y);
                    roll = roll.max(g.wobble.length());
                    let f = &w.feet[1];
                    if f.t > 0.05 && f.t < SCUFF_UNTIL {
                        scuffed.1 += 1;
                        if f.planted.y < 0.02 {
                            scuffed.0 += 1;
                        }
                    }
                }
            }
            (high - low, roll, scuffed)
        };
        let (bob, roll, _) = walk(1.0);
        let (hbob, hroll, (low, swung)) = walk(0.3);
        assert!(
            hbob > bob * 2.0 && hbob > 0.06,
            "it dips: {hbob:.3} m vs {bob:.3} m"
        );
        assert!(hroll > roll * 2.0, "and rolls: {hroll:.3} vs {roll:.3}");
        assert!(
            swung > 0 && low * 10 >= swung * 9,
            "it scuffs: {low}/{swung}"
        );
    }

    /// On an icy slope its planted feet creep downhill and it stumbles;
    /// on a gentle one, or without ice, they hold.
    #[test]
    fn feet_slip_on_ice_slopes() {
        let run = |grade: f32, icy: bool| {
            let hill = move |p: Vec3| -grade * p.x;
            let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &hill);
            let start: Vec<Vec3> = w.feet.iter().map(|f| f.planted).collect();
            let mut wobbled = 0.0f32;
            for _ in 0..(3 * 90) {
                let drive = Drive {
                    desired: Vec3::ZERO,
                    face: None,
                    pace: 1.0,
                    down: false,
                    stunned: false,
                    low: false,
                };
                let dt = 1.0 / 90.0;
                step_walker(
                    &mut w,
                    &mut g,
                    &mut t,
                    &drive,
                    dt,
                    &mut |p| hill(p),
                    &mut |_| {},
                );
                let ice = |_: Vec3| icy.then_some(Vec3::new(grade, 0.0, 0.0));
                slip_feet(&mut w, &mut g, dt, ice, &mut |p| hill(p));
                wobbled = wobbled.max(g.wobble.length());
                for f in w.feet.iter().filter(|f| f.t >= 1.0) {
                    assert!(
                        (f.planted.y - hill(f.planted)).abs() < 1e-3,
                        "a foot off the slope"
                    );
                }
            }
            let slid = w
                .feet
                .iter()
                .zip(&start)
                .map(|(f, s)| f.planted.x - s.x)
                .fold(f32::MIN, f32::max);
            (slid, wobbled)
        };
        let (slid, wobbled) = run(0.45, true);
        assert!(slid > 0.1, "it slid {slid:.2} m down the ice");
        assert!(wobbled > 0.02, "it stumbled ({wobbled:.3})");
        assert!(run(0.45, false).0 < 0.05, "dry, it holds");
        assert!(run(0.15, true).0 < 0.05, "a gentle icy slope holds");
    }

    /// Turning on the spot, it steps round rather than skating its feet.
    #[test]
    fn it_steps_round_to_turn() {
        let flat = |_: Vec3| 0.0;
        let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &flat);
        let mut landings = 0;
        for _ in 0..(4.0 * 90.0) as usize {
            let drive = Drive {
                desired: Vec3::ZERO,
                face: Some(Vec3::Z),
                pace: 1.0,
                down: false,
                stunned: false,
                low: false,
            };
            step_walker(
                &mut w,
                &mut g,
                &mut t,
                &drive,
                1.0 / 90.0,
                &mut |_| 0.0,
                &mut |_| landings += 1,
            );
        }
        assert!(wrap(w.yaw - std::f32::consts::PI).abs() < 0.05, "{}", w.yaw);
        assert!(landings >= 4, "it stepped round ({landings})");
        // And ended with its feet roughly under it.
        let rot = Quat::from_rotation_y(w.yaw);
        for i in 0..LEGS {
            let ideal = t.translation + rot * rest_offset(i);
            let off = Vec2::new(w.feet[i].planted.x - ideal.x, w.feet[i].planted.z - ideal.z);
            assert!(off.length() < 0.4, "foot {i} {:.2} off", off.length());
        }
    }

    /// The cost budget: ground probes (rays, in the game) per walker per
    /// tick, walking and turning on three legs and four.
    #[test]
    fn ground_probes_stay_in_budget() {
        let flat = |_: Vec3| 0.0;
        for dead in [None, Some(2)] {
            let (mut w, mut g, mut t) = walker_at(Vec3::new(0.0, BODY_HEIGHT, 0.0), &flat);
            if let Some(i) = dead {
                w.feet[i].alive = false;
            }
            let (mut total, mut worst) = (0usize, 0usize);
            let ticks = 90 * 8;
            for k in 0..ticks {
                let drive = Drive {
                    desired: Quat::from_rotation_y(k as f32 * 0.004) * Vec3::X * SPEED,
                    face: None,
                    pace: 1.3,
                    down: false,
                    stunned: false,
                    low: false,
                };
                let mut n = 0;
                step_walker(
                    &mut w,
                    &mut g,
                    &mut t,
                    &drive,
                    1.0 / 90.0,
                    &mut |_| {
                        n += 1;
                        0.0
                    },
                    &mut |_| {},
                );
                total += n;
                worst = worst.max(n);
            }
            let mean = total as f32 / ticks as f32;
            assert!(mean < 1.6, "{mean:.2} probes a tick on average ({dead:?})");
            assert!(worst <= 4, "{worst} probes in one tick ({dead:?})");
        }
    }

    #[test]
    fn knee_keeps_segment_lengths() {
        let hip = Vec3::new(0.0, 1.0, 0.0);
        let foot = Vec3::new(1.0, 0.0, 0.2);
        let knee = solve_knee(hip, foot, Vec3::Y + Vec3::X);
        assert!((knee.distance(hip) - UPPER).abs() < 1e-3);
        assert!((knee.distance(foot) - LOWER).abs() < 1e-3);
        assert!(knee.y > hip.y - 0.2, "knee bends upward, like a spider's");
    }
}
