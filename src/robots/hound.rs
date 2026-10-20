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

pub struct HoundPlugin;

impl Plugin for HoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, shape_hounds.after(super::perceive));
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
        Senses {
            sight: 60.0,
            fov_deg: 220.0,
            hearing: 1.6,
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
        assert!(fastest > 2.5, "ran ({fastest:.2} m/s)");
        // No orb plates on a hound.
        let plates = app
            .world_mut()
            .query::<&RobotPart>()
            .iter(app.world())
            .filter(|p| p.robot == hound && matches!(p.role, PartRole::Armour(_)))
            .count();
        assert_eq!(plates, 0);
    }
}
