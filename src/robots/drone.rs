//! The quadcopter: four rotors, a sensor eye, a gun pod.
//!
//! Flight is physical. A position controller asks for an acceleration, an
//! attitude controller turns that into a torque, and a mixer splits thrust
//! and torque across the four rotors, each pushing at its own position.
//! Shoot a rotor off and the mixer can't balance any more: the drone
//! tumbles out of the sky on its own, no scripted death needed.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{
    Awareness, Brain, Job, PartDestroyed, PartRole, Robot, RobotKind, RobotPart, Senses,
    part_bundle,
};
use crate::weapons::ballistics::Shot;
use crate::world::Zone;

/// Rotor arm length (x and z), metres.
const ARM: f32 = 0.32;
const MASS: f32 = 6.0;
const MAX_ROTOR_N: f32 = MASS * 9.81 / 4.0 * 2.2;

pub const ROTORS: [Vec3; 4] = [
    Vec3::new(ARM, 0.02, ARM),
    Vec3::new(-ARM, 0.02, ARM),
    Vec3::new(-ARM, 0.02, -ARM),
    Vec3::new(ARM, 0.02, -ARM),
];

#[derive(Component, Debug)]
pub struct Drone {
    /// Thrust per rotor this tick, newtons (for visuals and sound).
    pub thrust: [f32; 4],
    pub rotor_alive: [bool; 4],
    pub gun_cooldown: f32,
    pub burst_left: u32,
    /// Seconds spent near the ground without lift: it has crashed.
    pub grounded: f32,
    /// Where the controller is trying to be.
    pub goal: Vec3,
    /// Orbit phase while working or engaging.
    pub phase: f32,
    pub looted: bool,
}

/// The gun pod's muzzle, for shots.
#[derive(Component)]
pub struct DroneGun;

pub struct DronePlugin;

impl Plugin for DronePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (drone_parts_destroyed, drone_brain, fly_drones, drone_guns)
                .chain()
                .before(PhysicsSystems::First),
        );
    }
}

/// Spawns a drone at `at` with a job.
pub fn spawn_drone(commands: &mut Commands, at: Vec3, job: Job) -> Entity {
    let robot = commands
        .spawn((
            Name::new("Drone"),
            Robot {
                kind: RobotKind::Drone,
                wrecked: false,
            },
            Drone {
                thrust: [0.0; 4],
                rotor_alive: [true; 4],
                gun_cooldown: 2.0,
                burst_left: 0,
                grounded: 0.0,
                goal: at,
                phase: 0.0,
                looted: false,
            },
            Senses {
                // A camera dome: sees almost all round, not so far.
                sight: 90.0,
                fov_deg: 320.0,
                hearing: 1.0,
                eye: Vec3::new(0.0, -0.08, -0.22),
            },
            Awareness::default(),
            Brain::Transit { to: job.site },
            job,
            Transform::from_translation(at),
            Visibility::default(),
            (
                RigidBody::Dynamic,
                Mass(MASS),
                NoAutoMass,
                AngularInertia::new(Vec3::new(0.16, 0.26, 0.16)),
                NoAutoAngularInertia,
                LinearDamping(0.3),
                AngularDamping(1.5),
            ),
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Drone shell"),
            part_bundle(
                robot,
                PartRole::Core,
                70.0,
                Collider::cuboid(0.42, 0.13, 0.42),
                Transform::IDENTITY,
            ),
        ));
        c.spawn((
            Name::new("Drone eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                15.0,
                Collider::sphere(0.07),
                Transform::from_xyz(0.0, -0.06, -0.22),
            ),
        ));
        c.spawn((
            Name::new("Drone gun pod"),
            DroneGun,
            part_bundle(
                robot,
                PartRole::Weapon,
                30.0,
                Collider::cuboid(0.08, 0.08, 0.32),
                Transform::from_xyz(0.0, -0.14, -0.06),
            ),
        ));
        for (i, p) in ROTORS.iter().enumerate() {
            c.spawn((
                Name::new(format!("Rotor {i}")),
                part_bundle(
                    robot,
                    PartRole::Rotor(i as u8),
                    14.0,
                    Collider::cylinder(0.15, 0.04),
                    Transform::from_translation(*p + Vec3::Y * 0.03),
                ),
            ));
        }
    });
    robot
}

fn drone_parts_destroyed(
    mut events: MessageReader<PartDestroyed>,
    mut drones: Query<(&mut Drone, &mut Robot)>,
    mut commands: Commands,
) {
    for ev in events.read() {
        let Ok((mut d, mut robot)) = drones.get_mut(ev.robot) else {
            continue;
        };
        match ev.role {
            PartRole::Rotor(i) => {
                d.rotor_alive[i as usize] = false;
                // The rotor's collider goes; the visual hub stays.
                commands.entity(ev.part).insert(ColliderDisabled);
            }
            PartRole::Core => robot.wrecked = true,
            _ => {}
        }
    }
}

/// Picks where the drone wants to be, from its brain state.
fn drone_brain(
    mut drones: Query<(
        Entity,
        &mut Drone,
        &mut Brain,
        &Job,
        &Awareness,
        &Transform,
        &Robot,
    )>,
    zone: Res<Zone>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
    mut commands: Commands,
    entrances: Res<super::doors::Entrances>,
    door_states: Query<&crate::world::buildings::Door>,
) {
    let dt = time.delta_secs();
    for (e, mut d, mut brain, job, aware, t, robot) in &mut drones {
        if robot.wrecked {
            *brain = Brain::Down;
            continue;
        }
        let pos = t.translation;
        let ground = zone.ground(pos.x, pos.z);
        let cruise = |p: Vec3, h: f32| Vec3::new(p.x, zone.ground(p.x, p.z).max(ground) + h, p.z);
        // Someone indoors: hover outside a doorway, low, and look in
        // (further back from one that's shut, waiting for it to open).
        let peek = |at: Vec3| {
            let b = entrances.building_at(at)?;
            let (ent, open) = entrances.approach(b, pos, |door| {
                super::doors::is_open(door_states.get(door).ok())
            })?;
            let p = ent.mid + ent.out * if open { 3.5 } else { 7.0 };
            Some(Vec3::new(p.x, zone.ground(p.x, p.z) + 2.7, p.z))
        };
        d.phase += dt * 0.35;
        // Alarm overrides the job.
        if aware.can_see_target
            && aware.suspicion > 0.6
            && !matches!(*brain, Brain::Engage | Brain::Down)
        {
            *brain = Brain::Engage;
        } else if !matches!(
            *brain,
            Brain::Engage | Brain::Investigate { .. } | Brain::Down | Brain::Leave { .. }
        ) && aware.suspicion > 0.35
        {
            if let Some(at) = aware.last_known {
                *brain = Brain::Investigate { at, left: 25.0 };
            }
        }
        match &mut *brain {
            Brain::Transit { to } => {
                d.goal = cruise(*to, 22.0);
                if Vec2::new(pos.x - to.x, pos.z - to.z).length() < 6.0 {
                    *brain = Brain::Working {
                        at: *to,
                        left: job.duration,
                    };
                }
            }
            Brain::Working { at, left } => {
                // Survey: slow circles over the site, low enough to look in.
                let r = 14.0;
                d.goal = cruise(
                    *at + Vec3::new(d.phase.cos() * r, 0.0, d.phase.sin() * r),
                    11.0,
                );
                *left -= dt;
                if *left <= 0.0 {
                    *brain = Brain::Leave { to: job.exit };
                }
            }
            Brain::Investigate { at, left } => {
                d.goal = peek(*at).unwrap_or_else(|| {
                    cruise(
                        *at + Vec3::new(d.phase.cos() * 8.0, 0.0, d.phase.sin() * 8.0),
                        9.0,
                    )
                });
                *left -= dt;
                if *left <= 0.0 {
                    *brain = Brain::Leave { to: job.exit };
                }
            }
            Brain::Engage => {
                let Some(target) = aware
                    .target
                    .and_then(|p| targets.get(p).ok())
                    .map(|g| g.translation())
                else {
                    *brain = Brain::Leave { to: job.exit };
                    continue;
                };
                if aware.unseen > 12.0 {
                    *brain = Brain::Investigate {
                        at: aware.last_known.unwrap_or(target),
                        left: 20.0,
                    };
                    continue;
                }
                if let Some(g) = peek(target) {
                    d.goal = g;
                    continue;
                }
                // Stand off and circle: hard to hit, keeps a firing line.
                let r = 18.0;
                let off = Vec3::new(d.phase.cos() * r, 0.0, d.phase.sin() * r);
                d.goal = cruise(target + off, 7.0);
            }
            Brain::Leave { to } => {
                d.goal = cruise(*to, 25.0);
                if Vec2::new(pos.x - to.x, pos.z - to.z).length() < 8.0 {
                    commands.entity(e).despawn();
                }
            }
            Brain::Down => {}
        }
    }
}

/// Position -> acceleration -> attitude -> per-rotor thrust.
fn fly_drones(
    mut drones: Query<(
        &mut Drone,
        &Robot,
        &Transform,
        &Awareness,
        Has<super::Stunned>,
        Forces,
    )>,
    targets: Query<&GlobalTransform>,
    zone: Res<Zone>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let g = Vec3::new(0.0, -9.81, 0.0);
    for (mut d, robot, t, aware, stunned, mut forces) in &mut drones {
        let pos = t.translation;
        let lin = LinearVelocity(forces.linear_velocity());
        let ang = AngularVelocity(forces.angular_velocity());
        let near_ground = pos.y - zone.ground(pos.x, pos.z) < 0.6;
        if robot.wrecked
            || stunned
            || d.rotor_alive.iter().filter(|a| **a).count() < 4 && near_ground
        {
            d.thrust = [0.0; 4];
            if near_ground {
                d.grounded += dt;
            }
            continue;
        }
        // Position controller.
        let err = d.goal - pos;
        let mut acc = err * 1.2 - lin.0 * 1.6;
        let horiz = Vec2::new(acc.x, acc.z).clamp_length_max(5.0);
        acc = Vec3::new(horiz.x, acc.y.clamp(-4.0, 5.0), horiz.y);
        // Don't scrape the ground.
        if pos.y - zone.ground(pos.x, pos.z) < 2.5 {
            acc.y = acc.y.max(3.0);
        }
        let f_world = (acc - g) * MASS;
        // Desired up, tilted at most ~35 degrees.
        let mut up_des = f_world.normalize_or(Vec3::Y);
        let tilt = up_des.angle_between(Vec3::Y);
        if tilt > 0.6 {
            let axis = Vec3::Y.cross(up_des).normalize_or(Vec3::X);
            up_des = Quat::from_axis_angle(axis, 0.6) * Vec3::Y;
        }
        let up = t.rotation * Vec3::Y;
        let thrust_total = f_world.dot(up).max(0.0);
        // Attitude: tilt toward up_des, yaw toward the target or travel.
        let tilt_axis = up.cross(up_des);
        let tilt_err = tilt_axis.normalize_or_zero() * up.angle_between(up_des);
        let face = aware
            .target
            .filter(|_| aware.can_see_target)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() - pos)
            .unwrap_or(lin.0);
        let fwd = t.rotation * Vec3::NEG_Z;
        let face_flat = Vec3::new(face.x, 0.0, face.z);
        let yaw_err = if face_flat.length() > 0.5 {
            let f = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
            f.cross(face_flat.normalize()).y.clamp(-1.0, 1.0).asin()
        } else {
            0.0
        };
        let torque_world = tilt_err * 9.0 - ang.0 * 2.2 + Vec3::Y * yaw_err * 2.0;
        // Mix into rotor thrusts (body frame), X-quad allocation.
        let torque_body = t.rotation.inverse() * torque_world;
        let a2 = 4.0 * ARM * ARM;
        let mut f = [0.0f32; 4];
        for (i, r) in ROTORS.iter().enumerate() {
            let fi = thrust_total / 4.0 - torque_body.x * r.z / a2 + torque_body.z * r.x / a2;
            f[i] = if d.rotor_alive[i] {
                fi.clamp(0.0, MAX_ROTOR_N)
            } else {
                0.0
            };
        }
        d.thrust = f;
        for (i, r) in ROTORS.iter().enumerate() {
            if f[i] > 0.0 {
                let at = t.transform_point(*r);
                forces.apply_force_at_point(up * f[i], at);
            }
        }
        // Yaw comes from rotor drag; apply it directly.
        forces.apply_torque(Vec3::Y * torque_world.y.clamp(-3.0, 3.0));
        d.grounded = 0.0;
    }
}

/// Bursts at whoever it's engaging, when the gun bears.
fn drone_guns(
    mut drones: Query<(
        Entity,
        &mut Drone,
        &Brain,
        &Awareness,
        &Robot,
        &Children,
        &Transform,
    )>,
    guns: Query<(&GlobalTransform, &RobotPart), With<DroneGun>>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
    mut shots: MessageWriter<Shot>,
    mut seed: Local<u64>,
) {
    let dt = time.delta_secs();
    for (e, mut d, brain, aware, robot, children, _t) in &mut drones {
        d.gun_cooldown -= dt;
        if robot.wrecked || !matches!(brain, Brain::Engage) || !aware.can_see_target {
            continue;
        }
        let Some(target) = aware
            .target
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() + Vec3::Y * 1.2)
        else {
            continue;
        };
        let Some((gun_gt, part)) = children.iter().find_map(|c| guns.get(c).ok()) else {
            continue;
        };
        if !part.alive() || d.gun_cooldown > 0.0 {
            continue;
        }
        let muzzle = gun_gt.transform_point(Vec3::new(0.0, 0.0, -0.18));
        let to = (target - muzzle).normalize_or(Vec3::NEG_Z);
        // The pod gimbals, but only so far from the nose.
        if (gun_gt.rotation() * Vec3::NEG_Z).angle_between(to) > 0.9 {
            continue;
        }
        if d.burst_left == 0 {
            d.burst_left = 3;
        }
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        shots.write(Shot {
            gun: e,
            shooter: Some(e),
            origin: muzzle,
            direction: to,
            speed: 380.0,
            damage: 9.0,
            projectiles: 1,
            spread: 0.035,
            recoil: 0.0,
            seed: *seed,
        });
        d.burst_left -= 1;
        d.gun_cooldown = if d.burst_left == 0 { 1.8 } else { 0.11 };
    }
}
