//! Robot looks: sleek, pale, panelled shells with a few emissive accents,
//! so they read instantly against the murk. Rotors spin with their thrust;
//! walker legs are drawn from the IK joints every frame.

use bevy::prelude::*;

use super::drone::{Drone, DroneGun};
use super::walker::{LegPoses, WalkerLeg, WalkerTurret};
use super::{PartRole, Robot, RobotKind, RobotPart};

pub struct RobotVisualsPlugin;

impl Plugin for RobotVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (dress_parts, spin_rotors, pose_leg_visuals, dim_dead_eyes),
        );
    }
}

#[derive(Resource)]
struct RobotMats {
    shell: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    eye: Handle<StandardMaterial>,
    eye_dead: Handle<StandardMaterial>,
    rotor: Handle<StandardMaterial>,
}

/// A spinning rotor disc (visual only).
#[derive(Component)]
struct RotorDisc {
    index: usize,
}

/// A drawn leg segment: which leg, upper (0) or lower (1).
#[derive(Component)]
struct LegSegment {
    robot: Entity,
    leg: usize,
    lower: bool,
}

#[derive(Component)]
struct Eye;

fn mats(materials: &mut Assets<StandardMaterial>) -> RobotMats {
    RobotMats {
        shell: materials.add(StandardMaterial {
            base_color: Color::srgb(0.86, 0.87, 0.88),
            perceptual_roughness: 0.32,
            clearcoat: 0.6,
            clearcoat_perceptual_roughness: 0.2,
            ..default()
        }),
        dark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.085, 0.09),
            metallic: 0.7,
            perceptual_roughness: 0.4,
            ..default()
        }),
        eye: materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.02, 0.02),
            emissive: LinearRgba::rgb(9.0, 1.2, 0.6),
            ..default()
        }),
        eye_dead: materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.05, 0.05),
            perceptual_roughness: 0.2,
            ..default()
        }),
        rotor: materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 0.1, 0.1, 0.35),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    }
}

fn dress_parts(
    mut commands: Commands,
    parts: Query<
        (
            Entity,
            &RobotPart,
            Has<DroneGun>,
            Has<WalkerTurret>,
            Has<WalkerLeg>,
        ),
        Added<RobotPart>,
    >,
    robots: Query<&Robot>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Option<Res<RobotMats>>,
) {
    if parts.is_empty() {
        return;
    }
    let m = match existing {
        Some(m) => RobotMats {
            shell: m.shell.clone(),
            dark: m.dark.clone(),
            eye: m.eye.clone(),
            eye_dead: m.eye_dead.clone(),
            rotor: m.rotor.clone(),
        },
        None => {
            let m = mats(&mut materials);
            commands.insert_resource(RobotMats {
                shell: m.shell.clone(),
                dark: m.dark.clone(),
                eye: m.eye.clone(),
                eye_dead: m.eye_dead.clone(),
                rotor: m.rotor.clone(),
            });
            m
        }
    };
    for (e, part, gun, turret, leg) in &parts {
        let Ok(robot) = robots.get(part.robot) else {
            continue;
        };
        let mut ec = commands.entity(e);
        match (robot.kind, part.role) {
            (RobotKind::Drone, PartRole::Core) => {
                // A flattened, pebble-like shell with four arms.
                ec.insert((
                    Mesh3d(meshes.add(Sphere::new(0.24).mesh().uv(32, 16))),
                    MeshMaterial3d(m.shell.clone()),
                ));
                ec.insert(Transform::from_scale(Vec3::new(1.0, 0.42, 1.15)));
                for p in super::drone::ROTORS {
                    let arm = Cuboid::new(0.05, 0.035, p.length() * 1.05);
                    let dir = Vec3::new(p.x, 0.0, p.z).normalize();
                    commands.spawn((
                        Mesh3d(meshes.add(arm)),
                        MeshMaterial3d(m.shell.clone()),
                        Transform::from_translation(dir * p.length() * 0.5)
                            .with_rotation(Quat::from_rotation_arc(Vec3::Z, dir))
                            .with_scale(Vec3::new(1.0, 1.0 / 0.42, 1.0 / 1.15)),
                        ChildOf(e),
                    ));
                }
            }
            (RobotKind::Drone, PartRole::Rotor(i)) => {
                ec.insert((
                    Mesh3d(meshes.add(Cylinder::new(0.045, 0.06))),
                    MeshMaterial3d(m.dark.clone()),
                ));
                commands.spawn((
                    RotorDisc { index: i as usize },
                    Mesh3d(meshes.add(Cylinder::new(0.16, 0.006))),
                    MeshMaterial3d(m.rotor.clone()),
                    Transform::from_xyz(0.0, 0.035, 0.0),
                    ChildOf(e),
                ));
            }
            (_, PartRole::Sensor) => {
                let mesh = if robot.kind == RobotKind::Drone {
                    meshes.add(Sphere::new(0.06))
                } else {
                    meshes.add(Cuboid::new(0.28, 0.05, 0.05))
                };
                ec.insert((Mesh3d(mesh), MeshMaterial3d(m.eye.clone()), Eye));
            }
            (_, PartRole::Weapon) if gun => {
                ec.insert((
                    Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 0.3))),
                    MeshMaterial3d(m.dark.clone()),
                ));
                commands.spawn((
                    Mesh3d(
                        meshes.add(
                            Cylinder::new(0.012, 0.18)
                                .mesh()
                                .build()
                                .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                        ),
                    ),
                    MeshMaterial3d(m.dark.clone()),
                    Transform::from_xyz(0.0, 0.0, -0.22),
                    ChildOf(e),
                ));
            }
            (_, PartRole::Weapon) if turret => {
                ec.insert((
                    Mesh3d(meshes.add(Cuboid::new(0.16, 0.1, 0.36))),
                    MeshMaterial3d(m.shell.clone()),
                ));
                commands.spawn((
                    Mesh3d(
                        meshes.add(
                            Cylinder::new(0.018, 0.35)
                                .mesh()
                                .build()
                                .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                        ),
                    ),
                    MeshMaterial3d(m.dark.clone()),
                    Transform::from_xyz(0.0, 0.0, -0.33),
                    ChildOf(e),
                ));
            }
            (RobotKind::Walker, PartRole::Core) => {
                ec.insert((
                    Mesh3d(meshes.add(Sphere::new(0.4).mesh().uv(40, 20))),
                    MeshMaterial3d(m.shell.clone()),
                ));
                // An equator seam ring.
                commands.spawn((
                    Mesh3d(meshes.add(Torus::new(0.38, 0.415))),
                    MeshMaterial3d(m.dark.clone()),
                    Transform::default(),
                    ChildOf(e),
                ));
            }
            (RobotKind::Walker, PartRole::Leg(i)) if leg => {
                // The collider entity stays invisible; segments are drawn separately.
                for lower in [false, true] {
                    commands.spawn((
                        LegSegment {
                            robot: part.robot,
                            leg: i as usize,
                            lower,
                        },
                        Mesh3d(meshes.add(Capsule3d::new(if lower { 0.045 } else { 0.06 }, 1.0))),
                        MeshMaterial3d(if lower {
                            m.dark.clone()
                        } else {
                            m.shell.clone()
                        }),
                        Transform::default(),
                        Visibility::default(),
                    ));
                }
            }
            _ => {}
        }
    }
}

fn spin_rotors(
    drones: Query<(&Drone, &Children)>,
    rotor_parts: Query<&Children, With<RobotPart>>,
    mut discs: Query<(&RotorDisc, &mut Transform)>,
    time: Res<Time>,
) {
    for (d, children) in &drones {
        for c in children.iter() {
            let Ok(grand) = rotor_parts.get(c) else {
                continue;
            };
            for g in grand.iter() {
                if let Ok((disc, mut t)) = discs.get_mut(g) {
                    let rate = if d.rotor_alive[disc.index] {
                        40.0 + d.thrust[disc.index] * 3.0
                    } else {
                        0.0
                    };
                    let dir = if disc.index % 2 == 0 { 1.0 } else { -1.0 };
                    t.rotate_y(rate * dir * time.delta_secs());
                }
            }
        }
    }
}

fn pose_leg_visuals(
    mut commands: Commands,
    mut segments: Query<(Entity, &LegSegment, &mut Transform)>,
    poses: Query<&LegPoses>,
) {
    for (e, seg, mut t) in &mut segments {
        let Ok(p) = poses.get(seg.robot) else {
            commands.entity(e).despawn();
            continue;
        };
        let [hip, knee, foot] = p.0[seg.leg];
        let (a, b) = if seg.lower { (knee, foot) } else { (hip, knee) };
        let len = a.distance(b).max(0.01);
        let dir = (b - a) / len;
        *t = Transform::from_translation((a + b) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir))
            .with_scale(Vec3::new(1.0, len / 1.1, 1.0));
    }
}

fn dim_dead_eyes(
    parts: Query<(&RobotPart, &MeshMaterial3d<StandardMaterial>, Entity), With<Eye>>,
    robots: Query<&Robot>,
    mats: Option<Res<RobotMats>>,
    mut commands: Commands,
) {
    let Some(m) = mats else {
        return;
    };
    for (part, mat, e) in &parts {
        let dead = !part.alive() || robots.get(part.robot).is_ok_and(|r| r.wrecked);
        if dead && mat.0 != m.eye_dead {
            commands
                .entity(e)
                .insert(MeshMaterial3d(m.eye_dead.clone()));
        }
    }
}
