//! Wrecks: when a robot dies it comes apart physically. A walker's legs
//! break off at the hips and the knees, a panel or two blows off the orb,
//! the turret pops off its ring, and the orb drops, rolls and settles; a
//! drone sheds rotors (spinning) and a shell panel as it falls. Every piece
//! is a rigid body with a sensible mass, and once it has lain still a
//! moment (or tumbled long enough) it is made static, so a fight leaves
//! wreckage that costs nothing to keep. The robot's own entity stays as
//! the wreck: its core is still there to loot (`wreck_loot`) and to count.
//!
//! Debris is bounded: past [`MAX_DEBRIS`] pieces in the world, the oldest
//! goes.

use std::collections::VecDeque;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::drone::{Drone, ROTORS};
use super::walker::{LOWER, LegPoses, UPPER, Walker, WalkerLeg, WalkerTurret};
use super::{PartRole, Robot, RobotKind, RobotPart};
use crate::world::Layer;

/// At most this many loose pieces in the world.
pub const MAX_DEBRIS: usize = 72;
/// A piece lying this still for this long is done moving.
const STILL_SPEED: f32 = 0.08;
const STILL_FOR: f32 = 0.8;
/// However it's going, after this long it stops.
const SETTLE_BY: f32 = 14.0;

/// What a loose piece is (for its looks).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    /// A leg segment, upper or lower.
    LegUpper,
    LegLower,
    /// A curved shell panel off the body; the number picks its shape.
    Panel(u8),
    Turret,
    Rotor,
}

/// A loose piece of a wrecked robot.
#[derive(Component, Debug, Clone, Copy)]
pub struct Debris {
    pub kind: RobotKind,
    pub piece: Piece,
}

/// The robot has come apart (its wreck is handled).
#[derive(Component)]
pub struct Broken;

/// Moving until it has lain still a moment, then made static.
#[derive(Component, Debug, Default)]
pub struct Settling {
    still: f32,
    age: f32,
}

#[derive(Resource, Default)]
struct DebrisLedger(VecDeque<Entity>);

pub struct WreckPlugin;

impl Plugin for WreckPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebrisLedger>().add_systems(
            FixedUpdate,
            (break_up, settle)
                .chain()
                .after(super::perceive)
                .before(PhysicsSystems::First),
        );
    }
}

fn debris_body(mass: f32, collider: Collider, inertia: Vec3) -> impl Bundle {
    (
        RigidBody::Dynamic,
        collider,
        Mass(mass),
        NoAutoMass,
        AngularInertia::new(inertia),
        NoAutoAngularInertia,
        Friction::new(0.8),
        Restitution::new(0.15),
        LinearDamping(0.15),
        AngularDamping(0.6),
        CollisionLayers::new(Layer::Robot, [Layer::World, Layer::Robot, Layer::Item]),
        Settling::default(),
        Visibility::default(),
    )
}

/// A small deterministic hash, for scatter.
fn jitter(seed: u32, k: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0 * 2.0 - 1.0
}

#[allow(clippy::type_complexity)]
fn break_up(
    mut commands: Commands,
    robots: Query<
        (
            Entity,
            &Robot,
            &Transform,
            &Children,
            Option<&Walker>,
            Option<&LegPoses>,
            Option<&Drone>,
            Has<super::collector::Collector>,
        ),
        Without<Broken>,
    >,
    parts: Query<(
        &RobotPart,
        &GlobalTransform,
        Has<WalkerLeg>,
        Has<WalkerTurret>,
    )>,
    velocities: Query<(&LinearVelocity, &AngularVelocity)>,
    mut ledger: ResMut<DebrisLedger>,
) {
    for (e, robot, t, children, walker, poses, drone, collector) in &robots {
        // (The collector drops its cargo first, the tick it's wrecked.)
        if !robot.wrecked || collector {
            continue;
        }
        let seed = e.index_u32();
        let mut spawned: Vec<Entity> = Vec::new();
        let mut spawn =
            |commands: &mut Commands, b: (Debris, Transform, Vec3, Vec3, f32, Collider, Vec3)| {
                let (debris, at, v, w, mass, collider, inertia) = b;
                let id = commands
                    .spawn((
                        Name::new("Robot debris"),
                        debris,
                        at,
                        debris_body(mass, collider, inertia),
                        LinearVelocity(v),
                        AngularVelocity(w),
                    ))
                    .id();
                spawned.push(id);
            };
        match (robot.kind, walker, poses, drone) {
            (RobotKind::Walker, Some(w), Some(poses), _) => {
                commands.entity(e).insert(Broken).remove::<LegPoses>();
                let v0 = w.velocity;
                // Legs come off at the hips and knees.
                for (i, [hip, knee, foot]) in poses.0.iter().copied().enumerate() {
                    let out = Vec3::new(hip.x - t.translation.x, 0.0, hip.z - t.translation.z)
                        .normalize_or(Vec3::X);
                    for (k, (a, b, len, piece, mass)) in [
                        (hip, knee, UPPER, Piece::LegUpper, 7.0),
                        (knee, foot, LOWER, Piece::LegLower, 5.0),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let dir = (b - a).normalize_or(Vec3::NEG_Y);
                        let j = jitter(seed, (i * 2 + k) as u32);
                        let v = v0 + out * (0.6 + 0.5 * j) + Vec3::Y * (0.8 + 0.6 * j.abs());
                        let spin = out.cross(Vec3::Y) * (2.0 + 2.0 * j);
                        let r = if piece == Piece::LegUpper {
                            0.06
                        } else {
                            0.045
                        };
                        // Its own frame: Y down the segment.
                        let at = Transform::from_translation((a + b) * 0.5)
                            .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir));
                        let i_ax = mass * len * len / 12.0;
                        spawn(
                            &mut commands,
                            (
                                Debris {
                                    kind: robot.kind,
                                    piece,
                                },
                                at,
                                v,
                                spin,
                                mass,
                                Collider::capsule(r, (len - 2.0 * r).max(0.1)),
                                Vec3::new(i_ax, mass * r * r * 0.5, i_ax),
                            ),
                        );
                    }
                }
                // A panel or two blown off the orb, and the turret.
                for k in 0..2u32 {
                    let a = jitter(seed, 20 + k) * std::f32::consts::PI;
                    let dir = Vec3::new(a.cos(), 0.6, a.sin()).normalize();
                    let at = Transform::from_translation(t.translation + dir * 0.42)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir));
                    spawn(
                        &mut commands,
                        (
                            Debris {
                                kind: robot.kind,
                                piece: Piece::Panel(k as u8),
                            },
                            at,
                            v0 + dir * 3.5,
                            Vec3::new(jitter(seed, 30 + k), 1.0, jitter(seed, 40 + k)) * 6.0,
                            1.5,
                            Collider::cuboid(0.3, 0.025, 0.24),
                            Vec3::new(0.008, 0.018, 0.012),
                        ),
                    );
                }
                for c in children.iter() {
                    let Ok((_, gt, leg, turret)) = parts.get(c) else {
                        continue;
                    };
                    if turret {
                        let at = Transform::from_translation(gt.translation())
                            .with_rotation(gt.rotation());
                        spawn(
                            &mut commands,
                            (
                                Debris {
                                    kind: robot.kind,
                                    piece: Piece::Turret,
                                },
                                at,
                                v0 + Vec3::new(jitter(seed, 50), 2.5, jitter(seed, 51)),
                                Vec3::new(jitter(seed, 52), 0.5, jitter(seed, 53)) * 5.0,
                                9.0,
                                Collider::cuboid(0.16, 0.12, 0.4),
                                Vec3::new(0.13, 0.13, 0.03),
                            ),
                        );
                    }
                    if leg || turret {
                        commands.entity(c).despawn();
                    }
                }
                // The orb drops and rolls.
                commands.entity(e).insert((
                    RigidBody::Dynamic,
                    Mass(70.0),
                    NoAutoMass,
                    AngularInertia::new(Vec3::splat(0.4 * 70.0 * 0.16)),
                    NoAutoAngularInertia,
                    LinearVelocity(v0 + Vec3::Y * 0.5),
                    AngularVelocity(Vec3::new(jitter(seed, 60), 0.0, jitter(seed, 61)) * 1.5),
                    LinearDamping(0.3),
                    AngularDamping(1.2),
                    Friction::new(0.9),
                    Settling::default(),
                ));
            }
            (RobotKind::Drone, _, _, Some(d)) => {
                commands.entity(e).insert((Broken, Settling::default()));
                let (v0, w0) = velocities
                    .get(e)
                    .map(|(v, w)| (v.0, w.0))
                    .unwrap_or_default();
                // Two rotors that were still turning fly off, spinning.
                let mut shed = 0;
                for c in children.iter() {
                    let Ok((part, gt, _, _)) = parts.get(c) else {
                        continue;
                    };
                    let PartRole::Rotor(i) = part.role else {
                        continue;
                    };
                    if !d.rotor_alive[i as usize] || shed >= 2 {
                        continue;
                    }
                    shed += 1;
                    let out =
                        t.rotation * Vec3::new(ROTORS[i as usize].x, 0.0, ROTORS[i as usize].z);
                    let at =
                        Transform::from_translation(gt.translation()).with_rotation(gt.rotation());
                    spawn(
                        &mut commands,
                        (
                            Debris {
                                kind: robot.kind,
                                piece: Piece::Rotor,
                            },
                            at,
                            v0 + out.normalize_or_zero() * 3.0 + Vec3::Y * 1.5,
                            t.rotation * Vec3::Y * 40.0 + w0,
                            0.4,
                            Collider::cylinder(0.15, 0.04),
                            Vec3::new(0.0025, 0.0045, 0.0025),
                        ),
                    );
                    commands.entity(c).despawn();
                }
                let up = t.rotation * Vec3::Y;
                spawn(
                    &mut commands,
                    (
                        Debris {
                            kind: robot.kind,
                            piece: Piece::Panel(2),
                        },
                        Transform::from_translation(t.translation + up * 0.12)
                            .with_rotation(t.rotation),
                        v0 + up * 3.0 + Vec3::new(jitter(seed, 70), 0.0, jitter(seed, 71)),
                        Vec3::new(jitter(seed, 72), 0.3, jitter(seed, 73)) * 8.0,
                        0.5,
                        Collider::cuboid(0.22, 0.02, 0.18),
                        Vec3::new(0.0015, 0.003, 0.002),
                    ),
                );
            }
            _ => {
                // Dirigibles and pylons wreck in their own way.
                commands.entity(e).insert(Broken);
            }
        }
        for id in spawned {
            ledger.0.push_back(id);
        }
        while ledger.0.len() > MAX_DEBRIS {
            if let Some(old) = ledger.0.pop_front() {
                if let Ok(mut ec) = commands.get_entity(old) {
                    ec.despawn();
                }
            }
        }
    }
}

/// Pieces that have lain still a moment, or tumbled long enough, stop.
fn settle(
    mut commands: Commands,
    mut moving: Query<(Entity, &mut Settling, &LinearVelocity, &AngularVelocity)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut s, v, w) in &mut moving {
        s.age += dt;
        let still = v.0.length() < STILL_SPEED && w.0.length() < STILL_SPEED * 3.0;
        s.still = if still { s.still + dt } else { 0.0 };
        if (s.age > 0.5 && s.still > STILL_FOR) || s.age > SETTLE_BY {
            commands
                .entity(e)
                .remove::<Settling>()
                .insert(RigidBody::Static);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A walker shot through the core comes apart: its pieces fall, come to
    /// rest on the ground (not through it) within seconds, and nothing is
    /// left moving or NaN.
    #[test]
    fn a_wrecked_walker_comes_apart_and_settles() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
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
            c.entity(e).remove::<super::super::Senses>();
            q.apply(app.world_mut());
            e
        };
        for _ in 0..30 {
            app.update();
        }
        app.world_mut().get_mut::<Robot>(walker).unwrap().wrecked = true;
        app.update();
        let pieces: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<Debris>>()
            .iter(app.world())
            .collect();
        assert!(pieces.len() >= 8, "it came apart ({} pieces)", pieces.len());
        for _ in 0..(90.0 * SETTLE_BY) as usize {
            app.update();
        }
        let mut all = pieces.clone();
        all.push(walker);
        for e in all {
            let t = app.world().get::<Transform>(e).unwrap();
            assert!(t.translation.is_finite() && t.rotation.is_finite());
            let g = zone.ground(t.translation.x, t.translation.z);
            assert!(
                t.translation.y > g - 0.3 && t.translation.y < g + 1.2,
                "a piece at {:.2} m over the ground",
                t.translation.y - g
            );
            assert!(
                t.translation.distance(at) < 12.0,
                "a piece rolled {:.1} m away",
                t.translation.distance(at)
            );
            assert_eq!(
                app.world().get::<RigidBody>(e),
                Some(&RigidBody::Static),
                "at rest"
            );
        }
        // And the wreck is still a robot, still wrecked, to loot.
        assert!(app.world().get::<Robot>(walker).unwrap().wrecked);
    }
}
