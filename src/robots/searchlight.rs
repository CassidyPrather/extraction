//! Searchlights: what a machine that watches from the air can see. The
//! dirigible and the spotter used to see a long way all round (170 m,
//! nearly all the way round), and nothing showed it: the user's "the
//! blimp 'seeing a long way' reads awkward". Now each carries lamps that
//! sweep the ground slowly, and what a beam lights is what it sees
//! (Robot minds' senses read [`lights`]).
//!
//! - **The sweep.** A lazy figure-eight over its watch arc (`sweep`):
//!   side to side, and nodding near and far, slow enough to step out of.
//! - **Steering.** A mind can turn a lamp on something ([`Beam::steer`]):
//!   a noise, a suspect, you. It turns at [`STEER_RATE`], no faster: you
//!   see it coming. Let go, it eases back into its sweep.
//! - **Where it lands.** One ray down each beam a tick ([`Beam::landed`]):
//!   the beam stops on the ground, a roof, a tree; a wall between hides
//!   what's behind it.
//! - **Off.** Holed and sinking, cut loose, stunned: dark.
//!
//! The look (lamp head, the beam's haze, the pool where it lands, all
//! with a warning rim) is `searchlight_look`.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::dirigible::{Dirigible, RADIUS};
use super::spotter::{Float, Spotter};
use crate::world::Layer;

pub struct SearchlightPlugin;

impl Plugin for SearchlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (fit_searchlights, sweep_searchlights)
                .chain()
                .after(super::perceive),
        );
    }
}

/// What a lamp takes: a few rounds (it's armoured glass in a steel drum).
pub const LAMP_HP: f32 = 45.0;

/// How fast a steered lamp turns by default (rad/s): slow enough to see
/// it coming, and the fastest any mind may turn it.
pub const STEER_RATE: f32 = 0.45;
pub const STEER_RATE_MAX: f32 = 0.7;
/// How fast a lamp let go eases back into its sweep (rad/s).
const RETURN_RATE: f32 = 0.3;

/// A searchlight on a machine (a child of its root). Angles are the
/// lamp's own, against the machine: yaw round its up (0 straight ahead,
/// +left), pitch down from level.
#[derive(Component, Debug, Clone)]
pub struct Beam {
    pub robot: Entity,
    /// Where the lamp hangs, robot space.
    pub mount: Vec3,
    /// The middle of its watch: yaw and pitch (rad).
    pub base_yaw: f32,
    pub base_pitch: f32,
    /// How far it sweeps either side, and nods near and far (rad).
    pub arc: f32,
    pub nod: f32,
    /// Seconds a sweep takes (one figure-eight), and where in it this lamp
    /// starts (0..1), so two lamps don't sweep in step.
    pub period: f32,
    pub phase: f32,
    /// The beam's half-angle (rad) and the furthest it throws (m).
    pub half_angle: f32,
    pub reach: f32,
    /// Where it points now (rad, robot frame).
    pub yaw: f32,
    pub pitch: f32,
    /// Something a mind has turned it on (world), if anything, and how
    /// fast it turns there (rad/s).
    pub target: Option<Vec3>,
    pub steer_rate: f32,
    /// How far down the beam it lands (m; `reach` when it finds nothing).
    pub landed: f32,
    /// Lit (its machine is up and working).
    pub on: bool,
    clock: f32,
}

impl Beam {
    pub fn new(robot: Entity, mount: Vec3, base_yaw: f32, base_pitch: f32) -> Self {
        Self {
            robot,
            mount,
            base_yaw,
            base_pitch,
            arc: 0.8,
            nod: 0.18,
            period: 26.0,
            phase: 0.0,
            half_angle: 0.13,
            reach: 110.0,
            yaw: base_yaw,
            pitch: base_pitch,
            target: None,
            steer_rate: STEER_RATE,
            landed: 110.0,
            on: true,
            clock: 0.0,
        }
    }

    /// Turn it on `at` (world) at `rate` (rad/s; never faster than
    /// [`STEER_RATE_MAX`]: its swing toward you is the tell), until let go
    /// with `None`. Its sweep yields while it's steered, and picks up
    /// again from where it is.
    pub fn steer(&mut self, at: Option<Vec3>, rate: f32) {
        self.target = at;
        self.steer_rate = rate.clamp(0.05, STEER_RATE_MAX);
    }

    /// Where the lamp is and which way it throws (world), given its own
    /// transform (a `Beam` is a child of its machine). Line of sight is
    /// cast from `.0`.
    pub fn aim(lamp: &GlobalTransform) -> (Vec3, Vec3) {
        aim(lamp)
    }

    /// The lamp's rotation against its machine.
    pub fn local_rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(-self.pitch)
    }
}

/// Where in its sweep a lamp is at `t` seconds: (side, nod), each -1..1. A
/// figure-eight: once across and back while it nods twice, slowing at
/// each end (it lingers there, as a hand-turned lamp does).
pub fn sweep(t: f32, period: f32, phase: f32) -> (f32, f32) {
    let a = (t / period.max(0.1) + phase) * std::f32::consts::TAU;
    (a.sin(), (2.0 * a).sin())
}

/// Whether a lamp at `origin` throwing along `dir` (unit), `half_angle`
/// wide, landing `landed` m off, lights `p`: inside the cone and short of
/// where the beam stops. (Line of sight from the lamp is the senses' to
/// check: a wall across the beam short of the ground is `landed`.)
pub fn lights(origin: Vec3, dir: Vec3, half_angle: f32, landed: f32, p: Vec3) -> bool {
    let to = p - origin;
    let along = to.dot(dir);
    if along <= 0.0 || along > landed + 1.5 {
        return false;
    }
    // A little slack at the edge: a pool's rim is lit too.
    to.angle_between(dir) <= half_angle * 1.15
}

/// Where a lamp is and which way it throws, world.
pub fn aim(lamp: &GlobalTransform) -> (Vec3, Vec3) {
    (lamp.translation(), lamp.forward().as_vec3())
}

/// The dirigible's lamps: two under the gondola, one sweeping ahead and
/// down, one astern, out of step. It flies 26-42 m up: at 55° down they
/// land 30-50 m off, a pool 8-12 m across.
pub fn dirigible_lamps(robot: Entity) -> [Beam; 2] {
    let y = -RADIUS - 1.15;
    let mut fore = Beam::new(robot, Vec3::new(0.0, y, -1.6), 0.0, 0.95);
    fore.phase = 0.0;
    let mut aft = Beam::new(robot, Vec3::new(0.0, y, 1.6), std::f32::consts::PI, 0.95);
    aft.phase = 0.5;
    [fore, aft]
}

/// The spotter's lamp: on its eye, under the envelope, looking where the
/// eye does (its root turns with it), out and down across the valley.
pub fn spotter_lamp(robot: Entity) -> Beam {
    let mut l = Beam::new(
        robot,
        super::spotter::EYE + Vec3::new(0.0, -0.15, -0.1),
        0.0,
        0.6,
    );
    l.arc = 0.45;
    l.nod = 0.15;
    l.period = 18.0;
    l
}

#[allow(clippy::type_complexity)]
fn fit_searchlights(
    mut commands: Commands,
    dirigibles: Query<Entity, Added<Dirigible>>,
    spotters: Query<Entity, Added<Spotter>>,
) {
    for e in &dirigibles {
        for (i, l) in dirigible_lamps(e).into_iter().enumerate() {
            spawn_lamp(&mut commands, l, i as u8);
        }
    }
    for e in &spotters {
        spawn_lamp(&mut commands, spotter_lamp(e), 0);
    }
}

/// A lamp is a part of its machine (`PartRole::Lamp`): you can shoot it
/// out. Its drum is what's hit.
fn spawn_lamp(commands: &mut Commands, l: Beam, i: u8) {
    let t = Transform::from_translation(l.mount).with_rotation(l.local_rotation());
    let robot = l.robot;
    commands.spawn((
        Name::new("Searchlight"),
        super::part_bundle(
            robot,
            super::PartRole::Lamp(i),
            LAMP_HP,
            // (Its drum, along the throw: the cylinder turned onto Z.)
            Collider::compound(vec![(
                Vec3::Z * 0.2,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                Collider::cylinder(0.34, 0.5),
            )]),
            t,
        ),
        // (part_bundle brings its Visibility.)
        ChildOf(robot),
        l,
    ));
}

/// Each lamp sweeps (or turns on what it's been set on), and finds where
/// it lands.
#[allow(clippy::type_complexity)]
fn sweep_searchlights(
    mut lamps: Query<(&mut Beam, &mut Transform, Option<&super::RobotPart>)>,
    robots: Query<(
        &GlobalTransform,
        &super::Robot,
        Option<&Dirigible>,
        Option<&Spotter>,
        Has<super::Stunned>,
    )>,
    spatial: SpatialQuery,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    for (mut l, mut t, part) in &mut lamps {
        let Ok((root, robot, dirigible, spotter, stunned)) = robots.get(l.robot) else {
            continue;
        };
        let up = !robot.wrecked
            && part.is_none_or(|p| p.alive())
            && !stunned
            && !dirigible.is_some_and(|d| d.sinking)
            && !spotter.is_some_and(|s| s.float != Float::Aloft);
        if l.on != up {
            l.on = up;
        }
        if !up {
            continue;
        }
        l.clock += dt;
        // Where it wants to point: on what it's been set on, else its sweep.
        let (want_yaw, want_pitch, rate) = match l.target {
            Some(at) => {
                let local = root.affine().inverse().transform_point3(at) - l.mount;
                let flat = Vec2::new(local.x, local.z).length();
                // (Yaw round +Y from straight ahead, -Z.)
                let yaw = (-local.x).atan2(-local.z);
                let pitch = (-local.y).atan2(flat.max(0.01));
                (yaw, pitch, l.steer_rate)
            }
            None => {
                let (side, nod) = sweep(l.clock, l.period, l.phase);
                (
                    l.base_yaw + side * l.arc,
                    l.base_pitch + nod * l.nod,
                    // Its own sweep never outruns this; back from a steer it eases.
                    RETURN_RATE.max(l.arc * std::f32::consts::TAU / l.period * 1.6),
                )
            }
        };
        let step = rate * dt;
        let dy = wrap(want_yaw - l.yaw).clamp(-step, step);
        let dp = (want_pitch - l.pitch).clamp(-step, step);
        l.yaw = wrap(l.yaw + dy);
        l.pitch = (l.pitch + dp).clamp(0.05, 1.5);
        let r = l.local_rotation();
        if t.rotation != r || t.translation != l.mount {
            *t = Transform::from_translation(l.mount).with_rotation(r);
        }
        // Where the beam lands: from the lamp, down its throw (world).
        let rot = root.rotation() * r;
        let dir = rot * Vec3::NEG_Z;
        let from = root.transform_point(l.mount);
        let landed = Dir3::new(dir)
            .ok()
            .and_then(|d| spatial.cast_ray(from + dir * 0.5, d, l.reach, true, &world))
            .map_or(l.reach, |h| h.distance + 0.5);
        if (l.landed - landed).abs() > 0.05 {
            l.landed = landed;
        }
    }
}

fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sweep goes side to side and nods, slowly, and two lamps half a
    /// sweep apart are never in step.
    #[test]
    fn the_sweep_is_a_slow_figure_eight() {
        let period = 26.0;
        let (mut lo, mut hi) = (1.0f32, -1.0f32);
        let mut fastest = 0.0f32;
        let mut prev = sweep(0.0, period, 0.0).0;
        for i in 1..=2600 {
            let t = i as f32 * 0.01;
            let (s, n) = sweep(t, period, 0.0);
            lo = lo.min(s);
            hi = hi.max(s);
            assert!((-1.0..=1.0).contains(&n));
            fastest = fastest.max((s - prev).abs() / 0.01);
            prev = s;
        }
        assert!(lo < -0.99 && hi > 0.99, "{lo} {hi}");
        // Across its arc (0.8 rad a side) at most ~0.4 rad/s: you can
        // step out of it.
        assert!(fastest * 0.8 < 0.45, "{:.2} rad/s", fastest * 0.8);
        let a = sweep(3.0, period, 0.0);
        let b = sweep(3.0, period, 0.5);
        assert!((a.0 - b.0).abs() > 0.1 || (a.1 - b.1).abs() > 0.1);
    }

    /// What's in the cone, short of where the beam lands, is lit; beside
    /// it, behind the lamp, or past a wall the beam stops on isn't.
    #[test]
    fn a_beam_lights_its_cone_and_stops_where_it_lands() {
        let o = Vec3::new(0.0, 40.0, 0.0);
        let dir = Vec3::new(0.0, -1.0, -1.0).normalize();
        let half = 0.13;
        let ground = o + dir * 56.0;
        assert!(lights(o, dir, half, 56.6, ground));
        // A few metres off the middle of an ~15 m pool: lit.
        assert!(lights(o, dir, half, 56.6, ground + Vec3::X * 4.0));
        // Well outside it: not.
        assert!(!lights(o, dir, half, 56.6, ground + Vec3::X * 15.0));
        // Behind the lamp: not.
        assert!(!lights(o, dir, half, 56.6, o - dir * 10.0));
        // The beam stops on a roof 30 m down: the ground under it is dark.
        assert!(!lights(o, dir, half, 30.0, ground));
    }

    /// A lamp's rotation throws where its yaw and pitch say: ahead and
    /// down at yaw 0; turned round at yaw pi.
    #[test]
    fn yaw_and_pitch_point_the_beam() {
        let mut l = Beam::new(Entity::PLACEHOLDER, Vec3::ZERO, 0.0, 0.95);
        let d = l.local_rotation() * Vec3::NEG_Z;
        assert!(d.z < -0.5 && d.y < -0.7, "{d}");
        l.yaw = std::f32::consts::PI;
        let d = l.local_rotation() * Vec3::NEG_Z;
        assert!(d.z > 0.5 && d.y < -0.7, "{d}");
        l.yaw = 0.5;
        let d = l.local_rotation() * Vec3::NEG_Z;
        assert!(d.x < -0.2, "+yaw turns left: {d}");
    }
}
