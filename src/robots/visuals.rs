//! Robot looks: sleek, pale, panelled shells with a few emissive accents,
//! so they read instantly against the murk. Rotors spin with their thrust;
//! walker legs are drawn from the IK joints every frame.
//!
//! Each part is dressed from `assets/models/robots/<kind>.glb` (see
//! `art/robots/`): one object per part kind, its origin at the part's spawn
//! offset. Without the file, or if it fails to load, parts get the old
//! primitive shapes.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;

use super::drone::{Drone, DroneGun};
use super::walker::{LegPoses, WalkerLeg, WalkerTurret};
use super::{PartRole, Robot, RobotKind, RobotPart};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;

pub struct RobotVisualsPlugin;

impl Plugin for RobotVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (request_models, dress_parts).chain(),
                spin_rotors,
                spin_propellers,
                pose_leg_visuals,
                dim_dead_eyes,
            ),
        );
    }
}

#[derive(Resource, Clone)]
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

/// A dirigible's engine propeller, turning about its Z while the ship lives.
#[derive(Component)]
struct Propeller {
    robot: Entity,
}

/// A drawn leg segment: which leg, upper (0) or lower (1). With a model,
/// `length` is the segment's modelled length (origin at the near joint,
/// running along +Y); without, it is drawn as a centred capsule.
#[derive(Component)]
struct LegSegment {
    robot: Entity,
    leg: usize,
    lower: bool,
    model_length: Option<f32>,
}

/// An emissive mesh, dimmed when its part dies or the robot is wrecked.
#[derive(Component)]
struct Eye {
    part: Entity,
}

/// A part waiting to be dressed (its robot's model may still be loading).
#[derive(Component)]
struct Undressed;

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
            base_color: Color::srgba(0.1, 0.1, 0.1, 0.25),
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    }
}

fn model_path(kind: RobotKind) -> &'static str {
    match kind {
        RobotKind::Drone => "models/robots/drone.glb",
        RobotKind::Walker => "models/robots/walker.glb",
        RobotKind::Dirigible => "models/robots/dirigible.glb",
    }
}

/// New parts wait in `Undressed` until their robot's model is settled.
fn request_models(
    mut commands: Commands,
    parts: Query<(Entity, &RobotPart), Added<RobotPart>>,
    robots: Query<&Robot>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
) {
    for (e, part) in &parts {
        if let Ok(robot) = robots.get(part.robot) {
            models.request(model_path(robot.kind), &server);
        }
        commands.entity(e).insert(Undressed);
    }
}

/// The robot model's material for a name; robot materials stay the
/// hand-tuned ones, except the shell, which takes the scanned set if any.
fn robot_material(
    name: Option<&str>,
    m: &RobotMats,
    model_mats: &mut ModelMaterials,
    lib: &mut MaterialLibrary,
    server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    match name {
        Some("Shell") => model_mats.get("Shell", lib, server, materials),
        Some("Eye") => m.eye.clone(),
        _ => m.dark.clone(),
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
        With<Undressed>,
    >,
    robots: Query<&Robot>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Option<Res<RobotMats>>,
) {
    if parts.is_empty() {
        return;
    }
    let m = match existing {
        Some(m) => m.clone(),
        None => {
            let m = mats(&mut materials);
            commands.insert_resource(m.clone());
            m
        }
    };
    let mut cache: Vec<(RobotKind, Option<Vec<ModelNode>>)> = Vec::new();
    for (e, part, gun, turret, leg) in &parts {
        let Ok(robot) = robots.get(part.robot) else {
            continue;
        };
        // The model's nodes, or None for the blockout; skip while loading.
        let model = match cache.iter().position(|(k, _)| *k == robot.kind) {
            Some(i) => cache[i].1.as_ref(),
            None => {
                let nodes_for_kind = match models.request(model_path(robot.kind), &server) {
                    None => Some(None),
                    Some(h) => match Models::state(&h, &server, &gltfs) {
                        ModelState::Loading => None,
                        ModelState::Failed => {
                            warn!("robot model failed to load; using primitives");
                            Some(None)
                        }
                        ModelState::Ready(g) => Some(Some(model_nodes(g, &nodes, &gltf_meshes))),
                    },
                };
                let Some(n) = nodes_for_kind else {
                    continue;
                };
                cache.push((robot.kind, n));
                cache.last().unwrap().1.as_ref()
            }
        };
        commands.entity(e).remove::<Undressed>();
        let Some(model) = model else {
            dress_blockout(
                &mut commands,
                e,
                robot.kind,
                part,
                gun,
                turret,
                leg,
                &mut meshes,
                &m,
            );
            continue;
        };
        let mut mat = |name: Option<&str>| {
            robot_material(name, &m, &mut model_mats, &mut lib, &server, &mut materials)
        };
        let node_name = match (robot.kind, part.role) {
            (_, PartRole::Core) => "core",
            (_, PartRole::Sensor) => "sensor",
            (RobotKind::Drone, PartRole::Weapon) if gun => "gun",
            (RobotKind::Walker, PartRole::Weapon) if turret => "turret",
            (RobotKind::Drone, PartRole::Rotor(_)) => "rotor",
            (RobotKind::Walker, PartRole::Leg(_)) if leg => "leg",
            (RobotKind::Dirigible, PartRole::GasCell(i)) => match i {
                0 => "cell_0",
                1 => "cell_1",
                _ => "cell_2",
            },
            _ => continue,
        };
        match (robot.kind, part.role) {
            (RobotKind::Drone, PartRole::Rotor(i)) => {
                let (Some(rotor), Some(blades)) = (
                    ModelNode::find(model, "rotor"),
                    ModelNode::find(model, "blades"),
                ) else {
                    continue;
                };
                // Rotor 0 is modelled; turn copies to face out from the others.
                let corner = |p: Vec3| Vec2::new(p.x, p.z).normalize_or_zero();
                let a = corner(super::drone::ROTORS[0]);
                let b = corner(super::drone::ROTORS[i as usize]);
                let yaw = Quat::from_rotation_y(a.angle_to(b) * -1.0);
                attach(
                    &mut commands,
                    e,
                    rotor,
                    Transform::from_rotation(yaw),
                    e,
                    &mut mat,
                );
                let disc = commands
                    .spawn((
                        RotorDisc { index: i as usize },
                        Transform::from_translation(
                            blades.transform.translation - rotor.transform.translation,
                        ),
                        Visibility::default(),
                        ChildOf(e),
                    ))
                    .id();
                attach(
                    &mut commands,
                    disc,
                    blades,
                    Transform::IDENTITY,
                    e,
                    &mut mat,
                );
                // A faint blur disc, so a spinning rotor reads as one.
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.142, 0.002))),
                    MeshMaterial3d(m.rotor.clone()),
                    ChildOf(disc),
                ));
            }
            (RobotKind::Walker, PartRole::Leg(i)) => {
                // The collider entity stays invisible; segments are drawn separately.
                for (lower, name, length) in [
                    (false, "leg_upper", super::walker::UPPER),
                    (true, "leg_lower", super::walker::LOWER),
                ] {
                    let Some(node) = ModelNode::find(model, name) else {
                        continue;
                    };
                    let seg = commands
                        .spawn((
                            LegSegment {
                                robot: part.robot,
                                leg: i as usize,
                                lower,
                                model_length: Some(length),
                            },
                            Transform::default(),
                            Visibility::default(),
                        ))
                        .id();
                    attach(&mut commands, seg, node, Transform::IDENTITY, e, &mut mat);
                }
            }
            (RobotKind::Dirigible, PartRole::Core) => {
                let Some(node) = ModelNode::find(model, node_name) else {
                    continue;
                };
                attach(&mut commands, e, node, Transform::IDENTITY, e, &mut mat);
                // The right engine's propeller, and a copy on the left.
                let Some(prop) = ModelNode::find(model, "prop") else {
                    continue;
                };
                let rel = prop.transform.translation - node.transform.translation;
                for x in [rel.x, -rel.x] {
                    let hub = commands
                        .spawn((
                            Propeller { robot: part.robot },
                            Transform::from_xyz(x, rel.y, rel.z),
                            Visibility::default(),
                            ChildOf(e),
                        ))
                        .id();
                    attach(&mut commands, hub, prop, Transform::IDENTITY, e, &mut mat);
                }
            }
            _ => {
                let Some(node) = ModelNode::find(model, node_name) else {
                    continue;
                };
                attach(&mut commands, e, node, Transform::IDENTITY, e, &mut mat);
            }
        }
    }
}

/// Hangs a model node's meshes on `parent`, at `at` (the node's own
/// translation is its part's spawn offset, which the part already has).
/// Emissive meshes are marked as eyes of `part`.
fn attach(
    commands: &mut Commands,
    parent: Entity,
    node: &ModelNode,
    at: Transform,
    part: Entity,
    mat: &mut impl FnMut(Option<&str>) -> Handle<StandardMaterial>,
) {
    let local = Transform {
        translation: Vec3::ZERO,
        ..node.transform
    };
    let holder = commands
        .spawn((at * local, Visibility::default(), ChildOf(parent)))
        .id();
    for p in &node.primitives {
        let mut ec = commands.spawn((
            Mesh3d(p.mesh.clone()),
            MeshMaterial3d(mat(p.material.as_deref())),
            ChildOf(holder),
        ));
        if p.material.as_deref() == Some("Eye") {
            ec.insert(Eye { part });
        }
    }
}

/// The primitive shapes, for when there is no model.
fn dress_blockout(
    commands: &mut Commands,
    e: Entity,
    kind: RobotKind,
    part: &RobotPart,
    gun: bool,
    turret: bool,
    leg: bool,
    meshes: &mut Assets<Mesh>,
    m: &RobotMats,
) {
    let mut ec = commands.entity(e);
    match (kind, part.role) {
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
            let mesh = if kind == RobotKind::Drone {
                meshes.add(Sphere::new(0.06))
            } else {
                meshes.add(Cuboid::new(0.28, 0.05, 0.05))
            };
            ec.insert((Mesh3d(mesh), MeshMaterial3d(m.eye.clone()), Eye { part: e }));
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
                        model_length: None,
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
        (RobotKind::Dirigible, PartRole::GasCell(i)) => {
            // Overlapping ellipsoids read as one long envelope.
            ec.insert((
                Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(32, 16))),
                MeshMaterial3d(m.shell.clone()),
            ));
            let len = if i == 1 { 3.4 } else { 3.0 };
            ec.insert(
                Transform::from_xyz(0.0, 0.0, (i as f32 - 1.0) * 4.8)
                    .with_scale(Vec3::new(2.6, 2.6, len)),
            );
            if i == 2 {
                // Tail fins.
                ec.with_children(|c| {
                    for a in [0.0f32, 1.5708, 3.1416, 4.7124] {
                        c.spawn((
                            Mesh3d(meshes.add(Cuboid::new(0.04, 0.9, 0.7))),
                            MeshMaterial3d(m.dark.clone()),
                            Transform::from_rotation(Quat::from_rotation_z(a)).with_translation(
                                Quat::from_rotation_z(a) * Vec3::Y * 1.05 + Vec3::Z * 0.75,
                            ),
                        ));
                    }
                });
            }
        }
        (RobotKind::Dirigible, PartRole::Core) => {
            // The gondola, its engine pods and a red running light.
            ec.insert((
                Mesh3d(meshes.add(Cuboid::new(1.4, 1.1, 4.0))),
                MeshMaterial3d(m.shell.clone()),
            ));
            ec.with_children(|c| {
                for x in [-1.3f32, 1.3] {
                    c.spawn((
                        Mesh3d(
                            meshes.add(
                                Cylinder::new(0.35, 1.2)
                                    .mesh()
                                    .build()
                                    .rotated_by(Quat::from_rotation_x(1.5708)),
                            ),
                        ),
                        MeshMaterial3d(m.dark.clone()),
                        Transform::from_xyz(x, 0.2, 0.8),
                    ));
                    c.spawn((
                        RotorDisc { index: 0 },
                        Mesh3d(
                            meshes.add(
                                Cylinder::new(0.55, 0.02)
                                    .mesh()
                                    .build()
                                    .rotated_by(Quat::from_rotation_x(1.5708)),
                            ),
                        ),
                        MeshMaterial3d(m.rotor.clone()),
                        Transform::from_xyz(x, 0.2, 1.45),
                    ));
                }
                c.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.08))),
                    MeshMaterial3d(m.eye.clone()),
                    Transform::from_xyz(0.0, -0.6, 1.8),
                ));
            });
        }
        _ => {}
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

fn spin_propellers(
    mut props: Query<(&Propeller, &mut Transform)>,
    robots: Query<&Robot>,
    time: Res<Time>,
) {
    for (p, mut t) in &mut props {
        if robots.get(p.robot).is_ok_and(|r| !r.wrecked) {
            t.rotate_local_z(14.0 * time.delta_secs());
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
        let Some(nominal) = seg.model_length else {
            *t = Transform::from_translation((a + b) * 0.5)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir))
                .with_scale(Vec3::new(1.0, len / 1.1, 1.0));
            continue;
        };
        // X along the knee's hinge axis (the leg plane's normal), Y down the
        // segment, so the modelled joints hinge the right way.
        let mut x = (knee - hip).cross(foot - knee);
        if x.length_squared() < 1e-8 {
            x = dir.any_orthonormal_vector();
        }
        let x = (x - dir * x.dot(dir)).normalize_or(dir.any_orthonormal_vector());
        let z = x.cross(dir);
        *t = Transform::from_translation(a)
            .with_rotation(Quat::from_mat3(&Mat3::from_cols(x, dir, z)))
            .with_scale(Vec3::new(1.0, len / nominal, 1.0));
    }
}

fn dim_dead_eyes(
    eyes: Query<(Entity, &Eye, &MeshMaterial3d<StandardMaterial>)>,
    parts: Query<&RobotPart>,
    robots: Query<&Robot>,
    mats: Option<Res<RobotMats>>,
    mut commands: Commands,
) {
    let Some(m) = mats else {
        return;
    };
    for (e, eye, mat) in &eyes {
        let dead = parts
            .get(eye.part)
            .is_ok_and(|part| !part.alive() || robots.get(part.robot).is_ok_and(|r| r.wrecked));
        if dead && mat.0 != m.eye_dead {
            commands
                .entity(e)
                .insert(MeshMaterial3d(m.eye_dead.clone()));
        }
    }
}
