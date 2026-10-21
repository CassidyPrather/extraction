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
    /// A walker's visor or a drone's camera dome.
    Sensor,
    /// A drone's gun pod.
    Gun,
    /// An armour plate shot loose (`armour`).
    Plate,
    /// A Sorter's upper arm (0) or forearm (1).
    Arm(u8),
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

/// A wreck hit the ground hard (for dust and sparks).
#[derive(Message, Clone, Copy, Debug)]
pub struct WreckImpact {
    pub at: Vec3,
    /// How big a thump: about the body's size.
    pub size: f32,
}

/// A walker going down: its legs still hang off it on their joints, the
/// weak ones come away as it lands.
#[derive(Component, Debug)]
pub struct Collapse {
    t: f32,
    /// Each leg's hip joint, its thigh, and whether it's to come off.
    hips: [Option<(Entity, Entity, bool)>; 4],
    seed: u32,
}

/// A drone going down: what lift its rotors still give spins it round as
/// it falls, trailing smoke, until it hits the ground.
#[derive(Component, Debug)]
pub struct Spiral {
    pub t: f32,
    spin: f32,
}

pub struct WreckPlugin;

impl Plugin for WreckPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebrisLedger>()
            .add_message::<WreckImpact>()
            .add_observer(unjoin)
            .add_systems(
                FixedUpdate,
                (
                    ledger_strays,
                    break_up,
                    collapse,
                    spiral,
                    settle,
                    tidy_joints,
                )
                    .chain()
                    .after(super::perceive)
                    .before(PhysicsSystems::First),
            );
    }
}

/// The Surge sweeps every robot's wreck away with everything else loose in
/// the Zone. Without it a wreck (its root, its parts, their colliders and
/// meshes) stayed for the whole session: one more for every robot you
/// ever brought down (soak finding, `docs/SOAK.md`).
pub fn clear_wrecks(world: &mut World) {
    let wrecks: Vec<Entity> = world
        .query::<(Entity, &Robot)>()
        .iter(world)
        .filter(|(_, r)| r.wrecked)
        .map(|(e, _)| e)
        .collect();
    for e in wrecks {
        world.despawn(e);
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
        // A piece never needs more (a wreck throws them at a few m/s); a
        // leg caught between its joints and a slope was flung at 600 m/s,
        // and its speculative contacts touched 15,000 ground triangles in
        // a tick (`render::spikes`).
        MaxLinearSpeed(25.0),
        MaxAngularSpeed(40.0),
        // Item, not Robot: bullets and hands still meet it, but the
        // player's move-and-slide (World and Robot) walks through it.
        CollisionLayers::new(Layer::Item, [Layer::World, Layer::Robot, Layer::Item]),
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
            Option<&super::anim::Gait>,
            Has<super::collector::Collector>,
            Option<&super::RobotSeed>,
        ),
        // (The heavy falls in its own way and stays: `heavy`.)
        (Without<Broken>, Without<super::heavy::Heavy>),
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
    for (e, robot, t, children, walker, poses, drone, gait, collector, seed) in &robots {
        // (The collector drops its cargo first, the tick it's wrecked.)
        if !robot.wrecked || collector {
            continue;
        }
        let seed = seed.map_or(0, |s| s.low());
        // A smaller body (a crawler) breaks into smaller, lighter pieces.
        let size = gait.map_or(1.0, |g| g.size);
        let (m3, m5) = (size.powi(3), size.powi(5));
        let mut spawned: Vec<Entity> = Vec::new();
        let mut spawn =
            |commands: &mut Commands, b: (Debris, Transform, Vec3, Vec3, f32, Collider, Vec3)| {
                let id = spawn_piece(commands, b, size);
                spawned.push(id);
                id
            };
        match (robot.kind, walker, poses, drone) {
            (RobotKind::Walker, Some(w), Some(poses), _) => {
                commands.entity(e).insert(Broken).remove::<LegPoses>();
                let v0 = w.velocity;
                // The legs stay on at their joints (a ball at the hip, a
                // hinge at the knee) and fold under it as it drops; the
                // weak ones come away as it lands (`collapse`).
                let mut hips = [None; 4];
                for (i, [hip, knee, foot]) in poses.0.iter().copied().enumerate() {
                    let out = Vec3::new(hip.x - t.translation.x, 0.0, hip.z - t.translation.z)
                        .normalize_or(Vec3::X);
                    let mut ids = [Entity::PLACEHOLDER; 2];
                    for (k, (a, b, len, piece, mass)) in [
                        (hip, knee, UPPER, Piece::LegUpper, 7.0),
                        (knee, foot, LOWER, Piece::LegLower, 5.0),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let dir = (b - a).normalize_or(Vec3::NEG_Y);
                        let j = jitter(seed, (i * 2 + k) as u32);
                        // Buckling: the feet skid out from under it.
                        let v = v0 + out * if k == 1 { 1.6 + 0.4 * j } else { 0.5 };
                        let spin = out.cross(Vec3::Y) * (1.5 + j);
                        let r = if piece == Piece::LegUpper {
                            0.06
                        } else {
                            0.045
                        };
                        // Its own frame: Y down the segment.
                        let at = Transform::from_translation((a + b) * 0.5)
                            .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir));
                        // (Lengths and masses at full size; scaled on spawn.)
                        let i_ax = mass * len * len / 12.0;
                        ids[k] = spawn(
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
                    let [upper, lower] = ids;
                    // Feet that skid rather than prop it up.
                    commands.entity(lower).insert(Friction::new(0.25));
                    let hip_joint = commands
                        .spawn((
                            Name::new("Wreck hip"),
                            SphericalJoint::new(e, upper)
                                .with_anchor(hip)
                                .with_swing_limits(0.0, 1.5),
                            JointCollisionDisabled,
                            WreckJoint { a: e, b: upper },
                        ))
                        .id();
                    // The knee hinges across the leg's plane.
                    let hinge = (knee - hip)
                        .cross(foot - knee)
                        .normalize_or(out.cross(Vec3::Y));
                    let upper_rot =
                        Quat::from_rotation_arc(Vec3::Y, (knee - hip).normalize_or(Vec3::NEG_Y));
                    commands.spawn((
                        Name::new("Wreck knee"),
                        RevoluteJoint::new(upper, lower)
                            .with_anchor(knee)
                            .with_hinge_axis(upper_rot.inverse() * hinge),
                        JointCollisionDisabled,
                        WreckJoint { a: upper, b: lower },
                    ));
                    let health = children
                        .iter()
                        .filter_map(|c| parts.get(c).ok())
                        .find(|(p, ..)| p.role == PartRole::Leg(i as u8))
                        .map_or(0.0, |(p, ..)| p.hp / p.max_hp.max(1.0));
                    let weak = !w.feet[i].alive || health < 0.4;
                    hips[i] = Some((hip_joint, upper, weak));
                }
                commands.entity(e).insert(Collapse { t: 0.0, hips, seed });
                // A panel or two blown off the orb, and the turret.
                for k in 0..2u32 {
                    let a = jitter(seed, 20 + k) * std::f32::consts::PI;
                    let dir = Vec3::new(a.cos(), 0.6, a.sin()).normalize();
                    let at = Transform::from_translation(t.translation + dir * 0.42 * size)
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
                    let Ok((part, gt, leg, turret)) = parts.get(c) else {
                        continue;
                    };
                    let sensor = part.role == PartRole::Sensor;
                    if sensor && !part.alive() {
                        // (Already popped off when it was shot out.)
                        commands.entity(c).despawn();
                        continue;
                    }
                    if sensor {
                        // The visor knocked off its mount.
                        let fwd = t.rotation * Vec3::NEG_Z;
                        spawn(
                            &mut commands,
                            (
                                Debris {
                                    kind: robot.kind,
                                    piece: Piece::Sensor,
                                },
                                Transform::from_translation(gt.translation())
                                    .with_rotation(gt.rotation()),
                                v0 + fwd * 1.8 + Vec3::Y * 1.2,
                                Vec3::new(jitter(seed, 80), jitter(seed, 81), 1.0) * 7.0,
                                1.2,
                                Collider::cuboid(0.3, 0.08, 0.06),
                                Vec3::new(0.001, 0.009, 0.009),
                            ),
                        );
                        commands.entity(c).despawn();
                        continue;
                    }
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
                    Mass(70.0 * m3),
                    NoAutoMass,
                    AngularInertia::new(Vec3::splat(0.4 * 70.0 * 0.16 * m5)),
                    NoAutoAngularInertia,
                    // Its legs give: it drops.
                    LinearVelocity(v0 - Vec3::Y * 1.5),
                    AngularVelocity(Vec3::new(jitter(seed, 60), 0.0, jitter(seed, 61)) * 1.5),
                    LinearDamping(0.3),
                    AngularDamping(1.2),
                    Friction::new(0.9),
                    Settling::default(),
                ));
            }
            (RobotKind::Drone, _, _, Some(d)) => {
                // It goes down spinning (`spiral`); the panel and pod come
                // off when it hits.
                let spin = if seed % 2 == 0 { 1.0 } else { -1.0 };
                commands.entity(e).insert((Broken, Spiral { t: 0.0, spin }));
                let (v0, w0) = velocities
                    .get(e)
                    .map(|(v, w)| (v.0, w.0))
                    .unwrap_or_default();
                // A rotor that was still turning flies off, spinning.
                let mut shed = 0;
                for c in children.iter() {
                    let Ok((part, gt, _, _)) = parts.get(c) else {
                        continue;
                    };
                    let PartRole::Rotor(i) = part.role else {
                        continue;
                    };
                    if !d.rotor_alive[i as usize] || shed >= 1 {
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
            }
            // The Sorter comes off its rails in its own way (`sorter`).
            (RobotKind::Sorter, ..) => {}
            _ => {
                // Dirigibles and pylons wreck in their own way.
                commands.entity(e).insert(Broken);
            }
        }
        keep(&mut ledger, &mut commands, &spawned);
    }
}

/// Spawns one loose piece (masses and lengths at full size, scaled to
/// `size`).
fn spawn_piece(
    commands: &mut Commands,
    b: (Debris, Transform, Vec3, Vec3, f32, Collider, Vec3),
    size: f32,
) -> Entity {
    let (debris, at, v, w, mass, collider, inertia) = b;
    commands
        .spawn((
            Name::new("Robot debris"),
            debris,
            at.with_scale(Vec3::splat(size)),
            debris_body(mass * size.powi(3), collider, inertia * size.powi(5)),
            LinearVelocity(v),
            AngularVelocity(w),
        ))
        .id()
}

/// A loose piece from outside a wreck (a plate shot off): queued for
/// `wreck` to count against [`MAX_DEBRIS`] like the rest.
pub fn spawn_debris(
    commands: &mut Commands,
    b: (Debris, Transform, Vec3, Vec3, f32, Collider, Vec3),
    size: f32,
) -> Entity {
    let id = spawn_piece(commands, b, size);
    commands.entity(id).insert(Unledgered);
    id
}

/// Debris spawned outside `wreck`, not yet in the ledger.
#[derive(Component)]
struct Unledgered;

fn ledger_strays(
    mut commands: Commands,
    strays: Query<Entity, With<Unledgered>>,
    mut ledger: ResMut<DebrisLedger>,
) {
    let ids: Vec<Entity> = strays.iter().collect();
    for e in &ids {
        commands.entity(*e).remove::<Unledgered>();
    }
    keep(&mut ledger, &mut commands, &ids);
}

fn keep(ledger: &mut DebrisLedger, commands: &mut Commands, ids: &[Entity]) {
    ledger.0.extend(ids.iter().copied());
    while ledger.0.len() > MAX_DEBRIS {
        if let Some(old) = ledger.0.pop_front() {
            // (Its joints go with it: see `unjoin`.)
            if let Ok(mut ec) = commands.get_entity(old) {
                ec.despawn();
            }
        }
    }
}

/// A joint between two pieces of a wreck (avian joints are entities of
/// their own; this says which bodies, so the joint can go with them).
#[derive(Component, Debug, Clone, Copy)]
pub struct WreckJoint {
    pub a: Entity,
    pub b: Entity,
}

/// When a jointed body goes (despawned with a wreck, capped out of the
/// debris, a robot leaving), its joints go first, in the same flush: avian
/// must never hold a joint whose body is gone.
fn unjoin(
    gone: On<Remove, RigidBody>,
    joints: Query<(Entity, &WreckJoint)>,
    mut commands: Commands,
) {
    for (j, wj) in &joints {
        if wj.a == gone.entity || wj.b == gone.entity {
            commands.entity(j).try_despawn();
        }
    }
}

/// Joints whose bodies have both come to rest (or gone) are done with.
fn tidy_joints(
    mut commands: Commands,
    joints: Query<(Entity, &WreckJoint)>,
    bodies: Query<&RigidBody>,
) {
    for (j, wj) in &joints {
        let resting = |e: Entity| bodies.get(e).map_or(true, |b| *b == RigidBody::Static);
        if resting(wj.a) && resting(wj.b) {
            commands.entity(j).try_despawn();
        }
    }
}

/// As a walker lands, its weak legs (shot up, or already dead) tear off
/// at the hip, and one more with them; the rest stay on, folded under it.
fn collapse(
    mut commands: Commands,
    mut falling: Query<(
        Entity,
        &mut Collapse,
        &Transform,
        Option<&super::anim::Gait>,
    )>,
    mut impacts: MessageWriter<WreckImpact>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut c, t, gait) in &mut falling {
        let before = c.t;
        c.t += dt;
        let crossed = |at: f32| before < at && c.t >= at;
        if crossed(0.45) {
            impacts.write(WreckImpact {
                at: t.translation,
                size: gait.map_or(1.0, |g| g.size),
            });
            for (joint, _, weak) in c.hips.iter().flatten() {
                if *weak {
                    // Torn off: the joint goes (never `JointDisabled`: a
                    // disabled joint whose bodies are despawned later is
                    // re-added to avian's graph on the way out, and panics).
                    commands.entity(*joint).try_despawn();
                }
            }
        }
        if crossed(0.8) {
            // Two more tear away (the seed picks which), so no two wrecks
            // lie the same.
            for k in [c.seed % 4, (c.seed / 4 + 1 + c.seed % 4) % 4] {
                if let Some((joint, _, _)) = c.hips[k as usize] {
                    commands.entity(joint).try_despawn();
                }
            }
        }
        if c.t > 1.0 {
            commands.entity(e).remove::<Collapse>();
        }
    }
}

/// A wrecked drone spins down on what lift it has left, tipped over, so it
/// falls in a corkscrew; when it hits it sheds its panel and gun pod.
#[allow(clippy::type_complexity)]
fn spiral(
    mut commands: Commands,
    mut drones: Query<
        (
            Entity,
            &mut Spiral,
            &Transform,
            &Children,
            Forces,
            Option<&super::RobotSeed>,
        ),
        With<Drone>,
    >,
    parts: Query<(&RobotPart, &GlobalTransform)>,
    zone: Res<crate::world::Zone>,
    time: Res<Time>,
    mut impacts: MessageWriter<WreckImpact>,
    mut ledger: ResMut<DebrisLedger>,
) {
    let dt = time.delta_secs();
    for (e, mut s, t, children, mut forces, seed) in &mut drones {
        s.t += dt;
        let pos = t.translation;
        let above = pos.y - zone.ground(pos.x, pos.z);
        let v = forces.linear_velocity();
        if above < 0.5 || s.t > 10.0 {
            commands
                .entity(e)
                .remove::<Spiral>()
                .insert(Settling::default());
            impacts.write(WreckImpact { at: pos, size: 0.5 });
            let seed = seed.map_or(0, |s| s.low());
            let up = t.rotation * Vec3::Y;
            let mut ids = vec![spawn_piece(
                &mut commands,
                (
                    Debris {
                        kind: RobotKind::Drone,
                        piece: Piece::Panel(2),
                    },
                    Transform::from_translation(pos + up * 0.12).with_rotation(t.rotation),
                    v * 0.3 + Vec3::Y * 2.5 + Vec3::new(jitter(seed, 70), 0.0, jitter(seed, 71)),
                    Vec3::new(jitter(seed, 72), 0.3, jitter(seed, 73)) * 8.0,
                    0.5,
                    Collider::cuboid(0.22, 0.02, 0.18),
                    Vec3::new(0.0015, 0.003, 0.002),
                ),
                1.0,
            )];
            for c in children.iter() {
                let Ok((part, gt)) = parts.get(c) else {
                    continue;
                };
                if part.role != PartRole::Weapon {
                    continue;
                }
                ids.push(spawn_piece(
                    &mut commands,
                    (
                        Debris {
                            kind: RobotKind::Drone,
                            piece: Piece::Gun,
                        },
                        Transform::from_translation(gt.translation()).with_rotation(gt.rotation()),
                        v * 0.4 + Vec3::new(jitter(seed, 74), 1.5, jitter(seed, 75)),
                        Vec3::new(jitter(seed, 76), 0.5, 1.0) * 5.0,
                        1.4,
                        Collider::cuboid(0.08, 0.08, 0.32),
                        Vec3::new(0.012, 0.012, 0.0015),
                    ),
                    1.0,
                ));
                commands.entity(c).despawn();
            }
            keep(&mut ledger, &mut commands, &ids);
            continue;
        }
        // What lift is left fades over a couple of seconds.
        let up = t.rotation * Vec3::Y;
        let lift = super::drone::MASS * 9.81 * 0.8 * (1.0 - s.t / 3.5).max(0.0);
        forces.apply_force(up * lift);
        // Spin about its own up, and hold it tipped about 25 degrees: the
        // tipped lift, turning, swings it round in a corkscrew.
        forces.apply_torque(up * 0.9 * s.spin);
        let tilt = up.angle_between(Vec3::Y);
        let axis = Vec3::Y.cross(up).normalize_or(t.rotation * Vec3::X);
        forces.apply_torque(axis * (0.45 - tilt) * 2.5);
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
        if (s.age > 2.0 && s.still > STILL_FOR) || s.age > SETTLE_BY {
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
        // The pieces don't stop the player: their move-and-slide casts
        // against World and Robot, and debris is neither.
        let walks = LayerMask::from([Layer::World, Layer::Robot]);
        for e in &pieces {
            let layers = app.world().get::<CollisionLayers>(*e).unwrap();
            assert_eq!(
                layers.memberships & walks,
                LayerMask::NONE,
                "debris in the way"
            );
            assert!(
                layers.memberships.has_all(Layer::Item),
                "bullets still meet it"
            );
        }
        // And the wreck is still a robot, still wrecked, to loot.
        assert!(app.world().get::<Robot>(walker).unwrap().wrecked);
    }

    /// A drone killed in the air doesn't drop like a stone: it corkscrews
    /// down over a couple of seconds, drifting round, hits, sheds its
    /// panel and pod, and its pieces settle.
    #[test]
    fn a_wrecked_drone_spirals_down() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let site = zone.on_ground(40.0, 200.0) + Vec3::Y * 14.0;
        let drone = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::drone::spawn_drone(
                &mut c,
                site,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site,
                    duration: 999.0,
                    exit: site,
                },
            );
            // No brain: it holds where it was put, 14 m up.
            c.entity(e)
                .remove::<super::super::Senses>()
                .remove::<super::super::Brain>();
            q.apply(app.world_mut());
            e
        };
        for _ in 0..90 {
            app.update();
        }
        let start = app.world().get::<Transform>(drone).unwrap().translation;
        let drop = start.y - zone.ground(start.x, start.z);
        let free_fall = (2.0 * drop / 9.81).sqrt();
        app.world_mut().get_mut::<Robot>(drone).unwrap().wrecked = true;
        let mut track = Vec::new();
        let mut landed = None;
        for tick in 0..90 * 12 {
            app.update();
            let p = app.world().get::<Transform>(drone).unwrap().translation;
            assert!(p.is_finite());
            track.push(p);
            if landed.is_none() && app.world().get::<Spiral>(drone).is_none() && tick > 2 {
                landed = Some(tick as f32 / 90.0);
            }
        }
        let secs = landed.expect("it hit the ground");
        assert!(
            secs > free_fall * 1.4,
            "came down {drop:.1} m in {secs:.2} s: it fell like a stone ({free_fall:.2} s)"
        );
        // It swung round as it fell: the path is no straight drop.
        let reach = track
            .iter()
            .map(|p| Vec2::new(p.x - start.x, p.z - start.z).length())
            .fold(0.0f32, f32::max);
        assert!(reach > 1.0, "drifted only {reach:.2} m");
        let pieces = app
            .world_mut()
            .query_filtered::<&Debris, With<Debris>>()
            .iter(app.world())
            .filter(|d| matches!(d.piece, Piece::Panel(_) | Piece::Gun))
            .count();
        assert!(pieces >= 2, "shed its panel and pod on impact ({pieces})");
        // At rest by SETTLE_BY after it came down, at the latest.
        for _ in 0..(90.0 * (secs + SETTLE_BY + 0.5 - 12.0).max(0.0)) as usize {
            app.update();
        }
        assert_eq!(
            app.world().get::<RigidBody>(drone),
            Some(&RigidBody::Static),
            "at rest"
        );
    }

    /// How a walker at a fixed spot comes apart: its collapse seed and its
    /// pieces' launch velocities, with `filler` entities spawned first (so
    /// its entity id differs).
    fn how_it_breaks(filler: usize) -> (u32, Vec<[i32; 3]>) {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        for _ in 0..filler {
            app.world_mut().spawn_empty();
        }
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
        app.update();
        app.world_mut().get_mut::<Robot>(walker).unwrap().wrecked = true;
        app.update();
        let seed = app.world().get::<Collapse>(walker).unwrap().seed;
        let mut kicks: Vec<[i32; 3]> = app
            .world_mut()
            .query_filtered::<&LinearVelocity, With<Debris>>()
            .iter(app.world())
            .map(|v| (v.0 * 100.0).round().as_ivec3().to_array())
            .collect();
        kicks.sort();
        (seed, kicks)
    }

    /// A wreck comes apart the same way whatever its entity id: which legs
    /// tear and how its pieces fly come from its own seed.
    #[test]
    fn a_wreck_breaks_the_same_whatever_its_id() {
        let a = how_it_breaks(0);
        assert!(!a.1.is_empty());
        assert_eq!(how_it_breaks(37), a);
    }

    /// Wrecks come apart on joints; despawning them (a robot leaving, the
    /// debris cap, a whole tree at once) must never leave avian holding a
    /// joint whose body is gone. (A torn-off leg's `JointDisabled` joint,
    /// despawned with its leg, was re-added to the graph and panicked.)
    #[test]
    fn jointed_wrecks_despawn_cleanly() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let job = |at: Vec3| super::super::Job {
            kind: super::super::JobKind::Survey,
            site: at,
            duration: 999.0,
            exit: at,
        };
        let mut robots = Vec::new();
        for k in 0..6 {
            let at = zone.on_ground(30.0 + k as f32 * 4.0, 230.0);
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = if k % 2 == 0 {
                super::super::walker::spawn_walker(&mut c, &zone, at, job(at))
            } else {
                super::super::hound::spawn_hound(&mut c, &zone, at, job(at))
            };
            c.entity(e).remove::<super::super::Senses>();
            q.apply(app.world_mut());
            robots.push(e);
        }
        for _ in 0..20 {
            app.update();
        }
        for e in &robots {
            app.world_mut().get_mut::<Robot>(*e).unwrap().wrecked = true;
        }
        // Mid-collapse (legs tearing off), then gone: half the robots
        // despawned outright, and enough more wrecks to cap the debris.
        for _ in 0..70 {
            app.update();
        }
        for e in robots.iter().step_by(2) {
            app.world_mut().entity_mut(*e).despawn();
        }
        for k in 0..8 {
            let at = zone.on_ground(30.0 + k as f32 * 4.0, 240.0);
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::walker::spawn_walker(&mut c, &zone, at, job(at));
            q.apply(app.world_mut());
            app.update();
            app.world_mut().get_mut::<Robot>(e).unwrap().wrecked = true;
            for _ in 0..30 {
                app.update();
            }
        }
        for _ in 0..(90 * 4) {
            app.update();
        }
        // And every joint left points at bodies that are there.
        let joints: Vec<WreckJoint> = app
            .world_mut()
            .query::<&WreckJoint>()
            .iter(app.world())
            .copied()
            .collect();
        for j in joints {
            assert!(app.world().get_entity(j.a).is_ok() && app.world().get_entity(j.b).is_ok());
        }
        let debris = app.world_mut().query::<&Debris>().iter(app.world()).count();
        assert!(debris <= MAX_DEBRIS, "{debris} pieces");
    }
}
