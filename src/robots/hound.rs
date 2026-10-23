//! The hound: a fast, low quadruped that runs you down and comes at you
//! from the side. The body is here (a long lozenge on the walker's legs,
//! fore and aft, trotting at nearly 4 m/s: `anim::Shape::Hound`), dressed
//! from `hound_body`/`hound_head` in the walker model, wrecking like a
//! walker. Its mind is not written yet (Robot minds): nothing spawns it in
//! a raid. `spawn_hound` is there for that, and for the reels
//! (`--gallery reel:hound`).
//!
//! As a `Walker` it already follows a path, faces what it sees and reads
//! `Intent` (pace 1 is its run; ~0.4 a stalk); `walker_brain` gives it a
//! walker's behaviour meanwhile, unarmed.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::anim::{Gait, Shape};
use super::walker::{Foot, Walker, spawn_walker_sized};
use super::{Job, PartRole, RobotPart, Senses};
use crate::world::Zone;

/// A hound is this big against a walker (its legs, and so its reach).
pub const HOUND_SIZE: f32 = 0.7;

/// Marks a walker body as a hound's.
#[derive(Component, Debug, Default)]
pub struct Hound;

/// A hound coils to spring at `at` (the tell, 0.6 s, before a lunge).
/// A mind sends it; the body crouches, haunches down, nose up toward it.
#[derive(Message, Clone, Copy, Debug)]
pub struct HoundCrouch {
    pub hound: Entity,
    pub at: Vec3,
}

/// How long a hound stays coiled.
pub const COIL_TIME: f32 = 0.6;
/// How long the gait lets a lunge carry it past its top speed.
pub const LEAP_TIME: f32 = 0.4;

/// Launch a lunge: `speed` m/s toward `toward` (horizontal), carried
/// past the gait's top speed for [`LEAP_TIME`].
pub fn lunge(w: &mut Walker, g: &mut Gait, from: Vec3, toward: Vec3, speed: f32) {
    let d = toward - from;
    w.velocity = Vec3::new(d.x, 0.0, d.z).normalize_or_zero() * speed;
    g.leap = LEAP_TIME;
    g.coil = 0.0;
}

pub struct HoundPlugin;

impl Plugin for HoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HoundCrouch>().add_systems(
            FixedUpdate,
            (shape_hounds, coil_hounds).after(super::perceive),
        );
    }
}

fn coil_hounds(mut crouches: MessageReader<HoundCrouch>, mut gaits: Query<&mut Gait, With<Hound>>) {
    for c in crouches.read() {
        if let Ok(mut g) = gaits.get_mut(c.hound) {
            g.coil = COIL_TIME;
            g.coil_at = c.at;
        }
    }
}

/// A hound at `at` (standing on the terrain), with `job`. Unarmed; its
/// bite is its mind's to give it.
pub fn spawn_hound(commands: &mut Commands, zone: &Zone, at: Vec3, job: Job) -> Entity {
    let e = spawn_walker_sized(commands, zone, at, job, false, 90.0, HOUND_SIZE, None);
    let s = HOUND_SIZE;
    let shape = Shape::Hound;
    let pos = Vec3::new(at.x, zone.ground(at.x, at.z) + shape.height() * s, at.z);
    let feet = std::array::from_fn(|i| {
        let p = pos + shape.rest(i) * s;
        let f = Vec3::new(p.x, zone.ground(p.x, p.z), p.z);
        Foot {
            planted: f,
            from: f,
            to: f,
            t: 1.0,
            alive: true,
        }
    });
    commands.entity(e).insert((
        Name::new("Hound"),
        Hound,
        Gait::hound(s),
        Transform::from_translation(pos).with_scale(Vec3::splat(s)),
        // An ear more than an eye: it sees short and narrow and hears far.
        Senses {
            sight: 25.0,
            fov_deg: 120.0,
            hearing: 2.5,
            eye: Vec3::new(0.0, 0.1, -0.7),
        },
    ));
    commands.queue(move |world: &mut World| {
        if let Some(mut w) = world.get_mut::<Walker>(e) {
            w.feet = feet;
        }
    });
    e
}

/// A new hound's parts take its shape: the core a long capsule, the eye
/// on the head out front.
fn shape_hounds(
    mut commands: Commands,
    hounds: Query<&Children, Added<Hound>>,
    parts: Query<&RobotPart>,
) {
    for children in &hounds {
        for c in children.iter() {
            let Ok(part) = parts.get(c) else {
                continue;
            };
            match part.role {
                PartRole::Core => {
                    commands.entity(c).insert(Collider::capsule_endpoints(
                        0.22,
                        Vec3::new(0.0, 0.0, -0.35),
                        Vec3::new(0.0, 0.0, 0.35),
                    ));
                }
                PartRole::Sensor => {
                    commands.entity(c).insert((
                        Collider::cuboid(0.2, 0.12, 0.2),
                        Transform::from_xyz(0.0, 0.1, -0.62),
                    ));
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hound spawned and sent somewhere runs there on its four legs, on
    /// the ground, nothing NaN: the body is ready for a mind.
    #[test]
    fn a_hound_runs_to_where_its_sent() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let at = zone.on_ground(40.0, 230.0);
        let to = zone.on_ground(40.0, 200.0);
        let hound = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = spawn_hound(
                &mut c,
                &zone,
                at,
                Job {
                    kind: super::super::JobKind::Survey,
                    site: to,
                    duration: 999.0,
                    exit: at,
                },
            );
            c.entity(e).remove::<Senses>();
            q.apply(app.world_mut());
            e
        };
        let mut fastest = 0.0f32;
        let mut worst = 0.0f32;
        for _ in 0..(90 * 20) {
            app.update();
            let w = app.world().get::<Walker>(hound).unwrap();
            fastest = fastest.max(w.velocity.length());
            let poses = app
                .world()
                .get::<super::super::walker::LegPoses>(hound)
                .unwrap();
            for [hip, _, foot] in poses.0 {
                worst = worst.max(hip.distance(foot));
            }
        }
        let reach = (super::super::walker::UPPER + super::super::walker::LOWER) * HOUND_SIZE;
        assert!(
            worst < reach + 0.05,
            "a foot got {worst:.2} m from its hip (reach {reach:.2})"
        );
        let t = app.world().get::<Transform>(hound).unwrap();
        // Every foot near its hip: none left behind.
        let poses = app
            .world()
            .get::<super::super::walker::LegPoses>(hound)
            .unwrap();
        for [hip, _, foot] in poses.0 {
            let reach = (super::super::walker::UPPER + super::super::walker::LOWER) * HOUND_SIZE;
            assert!(
                hip.distance(foot) < reach + 0.05,
                "foot {:.2} m from its hip",
                hip.distance(foot)
            );
        }
        assert!(t.translation.is_finite());
        let g = zone.ground(t.translation.x, t.translation.z);
        let over = t.translation.y - g;
        assert!(over > 0.3 && over < 1.0, "rides {over:.2} m up");
        assert!(
            Vec2::new(t.translation.x - to.x, t.translation.z - to.z).length() < 5.0,
            "got there: {:.1?}",
            t.translation
        );
        // (At a walker's brain's pace, its lope.)
        assert!(fastest > 1.7, "ran ({fastest:.2} m/s)");
        // No orb plates on a hound.
        let plates = app
            .world_mut()
            .query::<&RobotPart>()
            .iter(app.world())
            .filter(|p| p.robot == hound && matches!(p.role, PartRole::Armour(_)))
            .count();
        assert_eq!(plates, 0);
    }

    /// Coiled, it all but stops, haunches down and nose up toward the
    /// mark; a lunge then carries it past its run, and it comes back to
    /// its legs after.
    #[test]
    fn it_coils_then_lunges() {
        use super::super::anim::{Drive, leg_poses, step_walker};
        let size = HOUND_SIZE;
        let shape = Shape::Hound;
        let start = Vec3::new(0.0, shape.height() * size, 0.0);
        let feet = std::array::from_fn(|i| {
            let p = start + shape.rest(i) * size;
            let f = Vec3::new(p.x, 0.0, p.z);
            Foot {
                planted: f,
                from: f,
                to: f,
                t: 1.0,
                alive: true,
            }
        });
        let mut w = Walker {
            feet,
            path: Vec::new(),
            repath: 0.0,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            gun_cooldown: 0.0,
            suppressing: false,
            gap: None,
            closing: 0.0,
            turret_yaw: 0.0,
            phase: 0,
            collapsed: false,
            goal: None,
            aim: 0.0,
            cover: None,
            rethink: 0.0,
            waited: 0.0,
        };
        let mut g = Gait::hound(size);
        let mut t = Transform::from_translation(start).with_scale(Vec3::splat(size));
        let mark = Vec3::new(0.0, 0.0, -5.0);
        let drive = |pace: f32| Drive {
            desired: Vec3::NEG_Z * g_top(),
            face: None,
            pace,
            down: false,
            stunned: false,
            low: true,
        };
        fn g_top() -> f32 {
            Gait::hound(HOUND_SIZE).top_speed()
        }
        let dt = 1.0 / 90.0;
        for _ in 0..90 {
            step_walker(
                &mut w,
                &mut g,
                &mut t,
                &drive(0.5),
                dt,
                &mut |_| 0.0,
                &mut |_| {},
            );
        }
        g.coil = COIL_TIME;
        g.coil_at = mark;
        for _ in 0..40 {
            step_walker(
                &mut w,
                &mut g,
                &mut t,
                &drive(0.5),
                dt,
                &mut |_| 0.0,
                &mut |_| {},
            );
        }
        assert!(
            w.velocity.length() < 0.4,
            "coiled, near still ({:.2})",
            w.velocity.length()
        );
        // Nose up: the front's higher than the back.
        let nose = t.transform_point(Vec3::new(0.0, 0.0, -0.5));
        let tail = t.transform_point(Vec3::new(0.0, 0.0, 0.5));
        assert!(
            nose.y > tail.y + 0.05,
            "nose {:.2} tail {:.2}",
            nose.y,
            tail.y
        );
        let from = t.translation;
        lunge(&mut w, &mut g, from, mark, 7.0);
        let mut fastest = 0.0f32;
        for _ in 0..150 {
            step_walker(
                &mut w,
                &mut g,
                &mut t,
                &drive(0.5),
                dt,
                &mut |_| 0.0,
                &mut |_| {},
            );
            fastest = fastest.max(w.velocity.length());
            for p in leg_poses(&w, &g, &t).iter().flatten() {
                assert!(p.is_finite());
            }
        }
        assert!(fastest > 6.5, "lunged at {fastest:.2}");
        assert!(t.translation.distance(from) > 2.5, "covered ground");
        assert!(w.velocity.length() < 2.0, "and came back to its feet");
    }
}
