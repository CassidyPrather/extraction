//! Robot looks: sleek, pale, panelled shells with a few emissive accents,
//! so they read instantly against the murk. Rotors spin with their thrust;
//! walker legs are drawn from the IK joints every frame.
//!
//! Each part is dressed from `assets/models/robots/<kind>.glb` (see
//! `art/robots/`): one object per part kind, its origin at the part's spawn
//! offset. Without the file, or if it fails to load, parts get the old
//! primitive shapes.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::drone::{Drone, DroneGun};
use super::walker::{LegPoses, WalkerLeg, WalkerTurret};
use super::{PartRole, Robot, RobotKind, RobotPart};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;

pub struct RobotVisualsPlugin;

impl Plugin for RobotVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(super::reel::ReelPlugin).add_systems(
            Update,
            (
                (request_models, dress_parts).chain(),
                spin_rotors,
                spin_propellers,
                pose_leg_visuals,
                dim_dead_eyes,
                blink_beacons,
                show_halos,
                dress_pylon_masts,
                pylon_work_lights,
                lock_flares,
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
    /// Cyan running lights on weak points (rotors, knees, vents): brighter
    /// than the kit's `Glow`, to carry through fog at 30-80 m.
    glow: Handle<StandardMaterial>,
    /// Drone beacons: amber, one material for them all, blinked by
    /// `blink_beacons`.
    beacon: Handle<StandardMaterial>,
    /// The soft additive glow round a drone's beacon, flashing with it.
    halo: Handle<StandardMaterial>,
}

/// A beacon's halo shell, hidden when its part dies.
#[derive(Component)]
struct Halo {
    part: Entity,
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
        glow: materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.4, 0.5),
            emissive: LinearRgba::rgb(1.2, 7.0, 9.0),
            ..default()
        }),
        beacon: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.75, 0.4),
            emissive: BEACON_ON * BEACON_REST,
            ..default()
        }),
        halo: materials.add(StandardMaterial {
            base_color: HALO.with_alpha(0.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            // A lamp in fog: the glow is what's left when the drone is gone.
            fog_enabled: false,
            ..default()
        }),
    }
}

fn model_path(kind: RobotKind) -> &'static str {
    match kind {
        RobotKind::Drone => "models/robots/drone.glb",
        RobotKind::Walker => "models/robots/walker.glb",
        RobotKind::Dirigible => "models/robots/dirigible.glb",
        RobotKind::Pylon => "models/robots/pylon.glb",
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
/// hand-tuned ones (the eye, so it can be dimmed), the rest by name from
/// [`ModelMaterials`] (the shell takes the scanned set, the pylon's glow
/// and hazard paint keep their colours).
fn robot_material(
    name: Option<&str>,
    m: &RobotMats,
    model_mats: &mut ModelMaterials,
    lib: &mut MaterialLibrary,
    server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    match name {
        Some("Eye") => m.eye.clone(),
        Some("Glow") => m.glow.clone(),
        Some("Beacon") => m.beacon.clone(),
        Some("Shell") => {
            // The scanned shell with a faint glow of its own: under the
            // overcast it lit to about the fog's own value, and a robot
            // has to stand out pale against the forest at 30-60 m
            // (docs/ART_DIRECTION.md: shell 150-200).
            let base = model_mats.get("Shell", lib, server, materials);
            model_mats.derived("RobotShell", materials, &base, |mat| {
                mat.emissive = LinearRgba::rgb(0.22, 0.22, 0.23);
            })
        }
        Some(name) => model_mats.get(name, lib, server, materials),
        None => m.dark.clone(),
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
            (RobotKind::Drone, PartRole::Core) => {
                let Some(node) = ModelNode::find(model, node_name) else {
                    continue;
                };
                attach(&mut commands, e, node, Transform::IDENTITY, e, &mut mat);
                // The beacon's halo: three nested additive shells, so it
                // reads as a soft glow 40 cm across, not a 3 cm dot.
                if let Some(halo) = ModelNode::find(model, "halo") {
                    for r in [0.06, 0.14, 0.26] {
                        commands.spawn((
                            Halo { part: e },
                            Mesh3d(meshes.add(Sphere::new(r).mesh().ico(2).unwrap())),
                            MeshMaterial3d(m.halo.clone()),
                            Transform::from_translation(halo.transform.translation),
                            NotShadowCaster,
                            ChildOf(e),
                        ));
                    }
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
/// Emissive meshes (`Eye`, `Glow`) are marked as eyes of `part`.
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
        // Eyes and glowing seams go dark with their part.
        if matches!(p.material.as_deref(), Some("Eye" | "Glow" | "Beacon")) {
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
        (RobotKind::Pylon, PartRole::Core) => {
            // A squat white cabinet, a glowing seam round its lid.
            ec.insert((
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.2, 1.0))),
                MeshMaterial3d(m.shell.clone()),
            ));
            ec.with_child((
                Mesh3d(meshes.add(Cuboid::new(1.02, 0.04, 1.02))),
                MeshMaterial3d(m.eye.clone()),
                Transform::from_xyz(0.0, 0.45, 0.0),
            ));
        }
        (RobotKind::Pylon, PartRole::Sensor) => {
            // The dish: a shallow disc on a yoke, its eye in the middle.
            ec.insert((
                Mesh3d(
                    meshes.add(
                        Cylinder::new(0.45, 0.08)
                            .mesh()
                            .build()
                            .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ),
                ),
                MeshMaterial3d(m.shell.clone()),
            ));
            ec.with_child((
                Mesh3d(meshes.add(Sphere::new(0.07))),
                MeshMaterial3d(m.eye.clone()),
                Transform::from_xyz(0.0, 0.0, -0.06),
            ));
        }
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
                    for a in [
                        0.0f32,
                        std::f32::consts::FRAC_PI_2,
                        std::f32::consts::PI,
                        (3.0 * std::f32::consts::FRAC_PI_2),
                    ] {
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
                                    .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
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
                                    .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
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

/// A beacon at full flash, and how much of it glows between flashes.
const BEACON_ON: LinearRgba = LinearRgba::rgb(40.0, 24.0, 8.0);
const BEACON_REST: f32 = 0.06;
/// Seconds between flashes, and how long each lasts.
const BEACON_PERIOD: f32 = 1.3;
const BEACON_FLASH: f32 = 0.18;
/// The halo's colour (amber, like the beacon).
const HALO: Color = Color::LinearRgba(LinearRgba::new(2.4, 1.3, 0.45, 1.0));

/// How bright a beacon is `t` seconds in: a short flash each period, a
/// faint glow between (a blink in the corner of the eye finds a drone in
/// fog that a steady light doesn't).
fn beacon_level(t: f32) -> f32 {
    let phase = t.rem_euclid(BEACON_PERIOD);
    if phase < BEACON_FLASH {
        // Up fast, down a little slower: a lamp, not a strobe.
        let k = phase / BEACON_FLASH;
        BEACON_REST + (1.0 - BEACON_REST) * (1.0 - (2.0 * k - 1.0).powi(2)).max(0.0).sqrt()
    } else {
        BEACON_REST
    }
}

/// Blinks every drone's beacon (one shared material; a dead drone's beacon
/// has been swapped for the dead-eye material).
fn blink_beacons(
    time: Res<Time>,
    mats: Option<Res<RobotMats>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(m) = mats else {
        return;
    };
    let level = beacon_level(time.elapsed_secs());
    if let Some(mut mat) = materials.get_mut(&m.beacon) {
        mat.emissive = BEACON_ON * level;
    }
    if let Some(mut mat) = materials.get_mut(&m.halo) {
        // Only the flash has a halo.
        let k = ((level - BEACON_REST) / (1.0 - BEACON_REST)).max(0.0);
        mat.base_color = HALO.with_alpha(0.3 * k);
    }
}

/// Halos are for seeing a drone from far off: shown beyond `HALO_NEAR` of
/// the camera (closer, the nested shells read as discs, and the drone
/// itself is plain to see), and gone with a dead drone's beacon.
const HALO_NEAR: f32 = 12.0;

fn show_halos(
    mut halos: Query<(&Halo, &GlobalTransform, &mut Visibility)>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    parts: Query<&RobotPart>,
    robots: Query<&Robot>,
) {
    let eye = cameras.iter().next().map(|t| t.translation());
    for (h, gt, mut vis) in &mut halos {
        let dead = parts
            .get(h.part)
            .is_ok_and(|part| !part.alive() || robots.get(part.robot).is_ok_and(|r| r.wrecked));
        let near = eye.is_some_and(|e| e.distance(gt.translation()) < HALO_NEAR);
        let want = if dead || near {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
    }
}

/// A pylon's mast: a dark lattice box, scaled with the pylon's progress.
fn dress_pylon_masts(
    mut commands: Commands,
    // Not `Added`: the robot materials may not exist yet when it's spawned.
    masts: Query<Entity, (With<super::pylon::PylonMast>, Without<Mesh3d>)>,
    mats: Option<Res<RobotMats>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(m) = mats else {
        return;
    };
    for e in &masts {
        commands.entity(e).insert((
            Mesh3d(meshes.add(lattice_mast())),
            MeshMaterial3d(m.dark.clone()),
        ));
    }
}

/// A unit-tall lattice mast (the pylon stretches it as it goes up): three
/// legs, a ring and a pair of diagonals for every metre at full height.
fn lattice_mast() -> Mesh {
    const R: f32 = 0.13;
    // At full height (`pylon::MAST`), one bay a metre.
    let bays = super::pylon::MAST.round().max(1.0) as usize;
    let legs: Vec<Vec3> = (0..3)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 3.0;
            Vec3::new(a.cos() * R, 0.0, a.sin() * R)
        })
        .collect();
    let mut out: Option<Mesh> = None;
    let mut add = |size: Vec3, t: Transform| {
        let m = Mesh::from(Cuboid::from_size(size)).transformed_by(t);
        match &mut out {
            Some(o) => {
                let _ = o.merge(&m);
            }
            None => out = Some(m),
        }
    };
    for l in &legs {
        add(
            Vec3::new(0.035, 1.0, 0.035),
            Transform::from_translation(*l),
        );
    }
    let step = 1.0 / bays as f32;
    for b in 0..=bays {
        let y = -0.5 + b as f32 * step;
        for i in 0..3 {
            let (p, q) = (legs[i], legs[(i + 1) % 3]);
            let mid = (p + q) * 0.5 + Vec3::Y * y;
            let d = q - p;
            let yaw = Quat::from_rotation_y(-d.z.atan2(d.x));
            add(
                Vec3::new(d.length(), 0.004, 0.02),
                Transform::from_translation(mid).with_rotation(yaw),
            );
            if b < bays {
                // A diagonal across the face, up to the next ring.
                let a3 = p + Vec3::Y * y;
                let b3 = q + Vec3::Y * (y + step);
                let along = b3 - a3;
                add(
                    Vec3::new(0.015, along.length(), 0.015),
                    Transform::from_translation((a3 + b3) * 0.5)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, along.normalize())),
                );
            }
        }
    }
    out.unwrap_or_else(|| Mesh::from(Cuboid::new(0.25, 1.0, 0.25)))
}

/// A pylon going up (or coming down) throws a flickering work light from
/// the top of its mast, like welding.
#[derive(Component)]
struct WorkLight;

fn pylon_work_lights(
    mut commands: Commands,
    pylons: Query<(
        Entity,
        &super::pylon::Pylon,
        &super::Robot,
        Option<&Children>,
    )>,
    mut lights: Query<(&mut PointLight, &mut Transform), With<WorkLight>>,
    time: Res<Time>,
) {
    use super::pylon::PylonState;
    let t = time.elapsed_secs();
    for (e, p, robot, children) in &pylons {
        let working =
            !robot.wrecked && matches!(p.state, PylonState::Building | PylonState::Packing);
        let existing = children.and_then(|c| c.iter().find(|c| lights.contains(*c)));
        match (working, existing) {
            (true, None) => {
                commands.spawn((
                    WorkLight,
                    PointLight {
                        color: Color::srgb(0.75, 0.85, 1.0),
                        intensity: 0.0,
                        range: 10.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::default(),
                    ChildOf(e),
                ));
            }
            (true, Some(l)) => {
                if let Ok((mut light, mut lt)) = lights.get_mut(l) {
                    // Bursts of arc light, never steady.
                    let n = (t * 23.0 + e.index_u32() as f32).sin() * (t * 7.3).sin();
                    light.intensity = if n > 0.2 { 60_000.0 * n } else { 0.0 };
                    lt.translation =
                        Vec3::Y * (1.25 + super::pylon::MAST * p.progress.max(0.01) + 0.1);
                }
            }
            (false, Some(l)) => commands.entity(l).despawn(),
            (false, None) => {}
        }
    }
}

/// A red flare under a drone settling its gun on you.
#[derive(Component)]
struct LockFlare {
    left: f32,
}

fn lock_flares(
    mut commands: Commands,
    mut locks: MessageReader<super::drone::DroneLock>,
    mut flares: Query<(Entity, &mut LockFlare, &mut PointLight)>,
    time: Res<Time>,
) {
    for l in locks.read() {
        if let Ok(mut c) = commands.get_entity(l.drone) {
            c.with_child((
                LockFlare {
                    left: super::drone::LOCK_TIME,
                },
                PointLight {
                    color: Color::srgb(1.0, 0.12, 0.08),
                    intensity: 0.0,
                    range: 8.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, -0.15, -0.25),
            ));
        }
    }
    let dt = time.delta_secs();
    for (e, mut f, mut light) in &mut flares {
        f.left -= dt;
        if f.left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        // Pulses in step with the chirps, the last held.
        let k = 1.0 - f.left / super::drone::LOCK_TIME;
        let on = k > 0.38 || (k % 0.19) < 0.09;
        light.intensity = if on { 25_000.0 } else { 0.0 };
    }
}

#[cfg(test)]
mod beacon_tests {
    use super::*;

    /// A short flash at the start of each period, a faint glow between.
    #[test]
    fn beacons_flash_then_rest() {
        let peak = beacon_level(BEACON_FLASH * 0.5);
        assert!((peak - 1.0).abs() < 1e-4, "peak {peak}");
        assert_eq!(beacon_level(BEACON_FLASH + 0.1), BEACON_REST);
        assert_eq!(beacon_level(BEACON_PERIOD - 0.01), BEACON_REST);
        let again = beacon_level(BEACON_PERIOD * 3.0 + BEACON_FLASH * 0.5);
        assert!((again - 1.0).abs() < 1e-3, "periodic: {again}");
    }
}
