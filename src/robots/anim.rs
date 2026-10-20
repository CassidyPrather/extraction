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

use super::walker::{LOWER, LegPoses, UPPER, Walker, WalkerLeg, WalkerStep};
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

/// Kept low (a grappler lying in wait, `Intent::low`), the orb sits this
/// much lower on bent legs.
pub const LOW: f32 = 0.35;

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
}

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
        }
    }
}

impl Gait {
    /// A knock to the body (a hit): `impulse` in m/s at the point, roughly.
    pub fn knock(&mut self, at_offset: Vec3, impulse: Vec3) {
        // Torque-ish: a hit high on the orb rocks it more than one low.
        let spin = at_offset.cross(impulse) * 1.6;
        self.wobble_vel += spin.clamp_length_max(2.5);
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
    let d = (foot - hip).length().clamp(0.05, UPPER + LOWER - 0.001);
    let dir = (foot - hip).normalize_or(Vec3::NEG_Y);
    // Distance along hip->foot to the knee's projection, and the knee's offset.
    let a = (UPPER * UPPER - LOWER * LOWER + d * d) / (2.0 * d);
    let h = (UPPER * UPPER - a * a).max(0.0).sqrt();
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Y);
    hip + dir * a + bend * h
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
    let alive_legs = w.feet.iter().filter(|f| f.alive).count();
    let dead: Vec3 = (0..LEGS)
        .filter(|i| !w.feet[*i].alive)
        .map(|i| Vec3::new(hip_offset(i).x, 0.0, hip_offset(i).z))
        .sum();
    // Lameness: the worst living leg, and anything missing.
    let worst = (0..LEGS)
        .filter(|i| w.feet[*i].alive)
        .map(|i| 1.0 - g.leg_health[i])
        .fold(0.0f32, f32::max);
    let missing = LEGS - alive_legs;
    g.limp = ((worst - 0.4).max(0.0) / 0.6 * 0.5 + missing as f32 * 0.55).min(1.0);
    let speed_scale = drive.pace.clamp(0.0, 1.6) * (1.0 - 0.6 * g.limp);
    let desired = if drive.down {
        Vec3::ZERO
    } else {
        Vec3::new(drive.desired.x, 0.0, drive.desired.z) * speed_scale
    };

    // Velocity: built up and bled off at a heavy machine's rate.
    let dv = (desired - w.velocity).clamp_length_max(ACCEL * dt);
    w.velocity += dv;
    let v = w.velocity;
    let accel = (v - g.last_v) / dt;
    g.last_v = v;
    g.accel = g.accel.lerp(accel, (6.0 * dt).min(1.0));
    let speed = v.length();

    // Yaw with inertia.
    let face = drive.face.unwrap_or(v);
    let want_rate = if !drive.down && Vec2::new(face.x, face.z).length() > 0.2 {
        let want = (-face.x).atan2(-face.z);
        (wrap(want - w.yaw) * 2.5).clamp(-TURN_RATE, TURN_RATE)
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
    let fast = (speed / SPEED).clamp(0.0, 1.5);
    let step_time = 0.36 - 0.1 * fast;
    let trigger = 0.3 + 0.16 * fast;
    let rot_ahead = Quat::from_rotation_y(w.yaw + g.yaw_rate * step_time * 0.9);
    let lookahead = v * step_time + v.normalize_or_zero() * trigger * 0.45 * fast.min(1.0);
    let probe_y = next.y;
    let ideal_xz = |i: usize| next + rot_ahead * rest_offset(i) + lookahead;
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
        let lift = (STEP_HEIGHT * (0.8 + 0.4 * fast.min(1.0)) + dy.abs() * 0.4)
            * (std::f32::consts::PI * nt).sin();
        let xz = f.from.lerp(f.to, s);
        f.planted = Vec3::new(xz.x, f.from.y + dy * sv + lift, xz.z);
        if nt >= 1.0 {
            f.planted = f.to;
            landed(f.to);
            // A heavy foot taking weight; a bad leg buckles a little more.
            kick += 0.22 + 0.1 * fast.min(1.0) + (1.0 - h) * 0.5;
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
        for _ in 0..LEGS {
            let air = w.feet.iter().filter(|f| f.alive && f.t < 1.0).count();
            if air >= max_air {
                break;
            }
            // The neediest foot that may go.
            let pick = (0..LEGS)
                .filter(|&i| {
                    let neighbours_down = [(i + 1) % LEGS, (i + LEGS - 1) % LEGS]
                        .iter()
                        .all(|&j| !w.feet[j].alive || w.feet[j].t >= 1.0);
                    let ready = need[i] > 1.0 || (g.idle > 0.6 && need[i] * trigger > 0.12);
                    w.feet[i].alive && w.feet[i].t >= 1.0 && ready && neighbours_down
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
    let target_sag = if drive.down && !drive.stunned {
        BODY_HEIGHT - 0.4
    } else if drive.stunned {
        0.55
    } else {
        missing as f32 * 0.14
            + g.limp * 0.05
            + g.brace.length() * 0.12
            + if drive.low { LOW } else { 0.0 }
    };
    g.sag += (target_sag - g.sag) * (3.0 * dt).min(1.0);
    let rest_y = if drive.down && !drive.stunned {
        under + BODY_HEIGHT
    } else {
        centroid.y.max(under - 0.2) + BODY_HEIGHT
    } - g.sag;
    next.y += (rest_y - next.y) * (6.0 * dt).min(1.0);

    // The bob: a spring kicked down by landings, lifted through swings.
    let swing = w
        .feet
        .iter()
        .filter(|f| f.alive && f.t < 1.0)
        .map(|f| (std::f32::consts::PI * f.t).sin())
        .fold(0.0f32, f32::max);
    let bob_target = 0.025 * swing * fast.min(1.0);
    g.bob_vel -= kick;
    g.bob_vel += ((bob_target - g.sway.y) * 140.0 - g.bob_vel * 15.0) * dt;
    let bob = (g.sway.y + g.bob_vel * dt).clamp(-0.15, 0.08);

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
    .clamp_length_max(0.07);
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
    let haul = Vec3::Y.cross(-g.brace) * 0.18;
    let tilt = if drive.down {
        Vec3::ZERO
    } else {
        slope + lean + list + haul
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
        let hip = t.transform_point(hip_offset(i));
        let hang = hip + rot * (Vec3::new(hip_offset(i).x, 0.0, hip_offset(i).z) * 1.2)
            - Vec3::Y * (UPPER + LOWER) * 0.85;
        if g.dangle[i] == Vec3::ZERO || g.dangle[i].distance(hang) > 2.5 {
            g.dangle[i] = hang;
        }
        let d = g.dangle[i].lerp(hang, (3.0 * dt).min(1.0));
        let floor = ground(Vec3::new(d.x, probe_y, d.z)) + 0.04;
        g.dangle[i] = Vec3::new(d.x, d.y.max(floor), d.z);
    }
}

/// Joints for each leg (hip, knee, foot), after [`step_walker`].
pub fn leg_poses(w: &Walker, g: &Gait, t: &Transform) -> [[Vec3; 3]; LEGS] {
    std::array::from_fn(|i| {
        let hip = t.transform_point(hip_offset(i));
        let foot = if w.feet[i].alive {
            w.feet[i].planted
        } else {
            g.dangle[i]
        };
        let out = (hip - t.translation).normalize_or(Vec3::X);
        let knee = solve_knee(hip, foot, out + Vec3::Y * 0.8);
        [hip, knee, foot]
    })
}

/// Body motion along the path, then feet, then IK, for every walker.
#[allow(clippy::type_complexity)]
pub(crate) fn walk(
    mut walkers: Query<(
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
) {
    let dt = time.delta_secs();
    let filter = SpatialQueryFilter::from_mask([Layer::World]);
    for (mut w, mut g, mut t, mut poses, aware, children, robot, intent, stunned) in &mut walkers {
        let pos = t.translation - g.sway;
        // Steering along the path.
        let mut desired = Vec3::ZERO;
        if let Some(next) = w.path.first().copied() {
            let to = Vec3::new(next.x - pos.x, 0.0, next.z - pos.z);
            if to.length() < 0.8 {
                w.path.remove(0);
            } else {
                desired = to.normalize() * SPEED;
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
        let drive = Drive {
            desired,
            face,
            pace: intent.pace,
            down: w.collapsed || robot.wrecked || stunned,
            stunned: stunned && !w.collapsed && !robot.wrecked,
            low: intent.low,
        };
        let mut ground = |p: Vec3| {
            let h = zone.ground(p.x, p.z);
            // Steps, floors and kerbs: a ray down from about hip height.
            if p.y > h + 0.25 {
                if let Some(hit) = spatial.cast_ray(
                    Vec3::new(p.x, p.y, p.z),
                    Dir3::NEG_Y,
                    p.y - h + 0.3,
                    true,
                    &filter,
                ) {
                    // Started inside something (a wall): not a footing.
                    if hit.distance > 0.02 {
                        return p.y - hit.distance;
                    }
                }
            }
            h
        };
        let mut landed = |at: Vec3| {
            steps.write(WalkerStep { at });
        };
        step_walker(&mut w, &mut g, &mut t, &drive, dt, &mut ground, &mut landed);
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
            ),
        );
    }
}

/// A grappler hauling on a line leans back from it and crouches; firing
/// the harpoon kicks it back.
fn haul_lines(
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
    for h in fired.read() {
        let (Ok((_, mut g, gt)), Ok(to)) = (walkers.get_mut(h.from), targets.get(h.to)) else {
            continue;
        };
        let dir = (to.translation() - gt.translation()).normalize_or_zero();
        g.knock(Vec3::Y * 0.3, -dir * 0.9);
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
