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
        app.add_plugins((
            super::reel::ReelPlugin,
            super::fx::RobotFxPlugin,
            super::sorter_look::SorterLookPlugin,
            super::heavy_look::HeavyLookPlugin,
            super::sentry_look::SentryLookPlugin,
            super::spotter_look::SpotterLookPlugin,
            super::minelayer_look::MinelayerLookPlugin,
            super::mender_look::MenderLookPlugin,
            super::plough_look::PloughLookPlugin,
        ))
        .add_systems(
            Update,
            (
                (request_models, dress_parts).chain(),
                spin_rotors,
                spin_propellers,
                pose_leg_visuals,
                dim_dead_eyes,
                last_flicker,
                blink_beacons,
                show_halos,
                dress_pylon_masts,
                pylon_work_lights,
                lock_flares,
                scorch,
                dress_debris,
                alert_eyes,
                shell_by_light,
                super::weathering::weather_meshes,
                part_plants,
                drone_lamps,
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
    /// Shell that has been shot up, and shell that has burnt.
    scorched: Handle<StandardMaterial>,
    burnt: Handle<StandardMaterial>,
}

/// A shell mesh of `part` (a `RobotPart`, or a piece of `Debris`), which
/// scorches as the part is shot up and burns when the robot is wrecked.
#[derive(Component)]
struct Skin {
    part: Entity,
    level: u8,
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
    /// How far round it has spun.
    angle: f32,
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
        scorched: materials.add(StandardMaterial {
            base_color: Color::srgb(0.58, 0.56, 0.53),
            perceptual_roughness: 0.75,
            metallic: 0.2,
            ..default()
        }),
        burnt: materials.add(StandardMaterial {
            base_color: Color::srgb(0.07, 0.065, 0.06),
            perceptual_roughness: 0.9,
            metallic: 0.35,
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
        RobotKind::Sorter => "models/robots/sorter.glb",
    }
}

/// New parts wait in `Undressed` until their robot's model is settled.
fn request_models(
    mut commands: Commands,
    parts: Query<(Entity, &RobotPart), Added<RobotPart>>,
    robots: Query<&Robot>,
    heavies: Query<
        (),
        Or<(
            With<super::heavy::Heavy>,
            With<super::sentry::Sentry>,
            With<super::heavy::OwnLook>,
        )>,
    >,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
) {
    for (e, part) in &parts {
        // (The heavy, the sentry and the others marked `OwnLook` dress
        // themselves: `heavy_look`, `sentry_look`, ...)
        if heavies.contains(part.robot) {
            continue;
        }
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
            Has<super::indoor::CrawlerGun>,
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
    hounds: Query<(), With<super::hound::Hound>>,
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
    for (e, part, gun, turret, leg, emitter) in &parts {
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
        // A dirigible's gondola plates: plain slabs, with a dark rim.
        if robot.kind == RobotKind::Dirigible && matches!(part.role, PartRole::Armour(_)) {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.86, 0.05, 3.3))),
                MeshMaterial3d(m.shell.clone()),
                Skin { part: e, level: 0 },
                ChildOf(e),
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.92, 0.04, 3.4))),
                MeshMaterial3d(m.dark.clone()),
                ChildOf(e),
            ));
            continue;
        }
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
        let hound = hounds.contains(part.robot);
        let node_name = match (robot.kind, part.role) {
            (RobotKind::Sorter, PartRole::Core) => "carriage",
            (RobotKind::Sorter, PartRole::Sensor) => "head",
            (_, PartRole::Core) if hound => "hound_body",
            (_, PartRole::Sensor) if hound => "hound_head",
            (_, PartRole::Core) => "core",
            (_, PartRole::Sensor) => "sensor",
            (RobotKind::Drone, PartRole::Weapon) if gun => "gun",
            (RobotKind::Walker, PartRole::Weapon) if turret => "turret",
            (RobotKind::Walker, PartRole::Weapon) if emitter => "emitter",
            (RobotKind::Walker, PartRole::Armour(_)) => "plate",
            (RobotKind::Sorter, PartRole::Armour(_)) => "plate",
            (RobotKind::Sorter, PartRole::Leg(_)) => "arm_upper",
            (RobotKind::Drone, PartRole::Armour(_)) => "panel",
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
                        RotorDisc {
                            index: i as usize,
                            angle: 0.0,
                        },
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
            (_, PartRole::Armour(_)) => {
                let Some(node) = ModelNode::find(model, node_name) else {
                    continue;
                };
                attach(
                    &mut commands,
                    e,
                    node,
                    plate_turn(robot.kind, node),
                    e,
                    &mut mat,
                );
            }
            (RobotKind::Walker | RobotKind::Sorter, PartRole::Leg(i)) => {
                // The collider entity stays invisible; segments are drawn separately.
                let names = if robot.kind == RobotKind::Sorter {
                    [
                        (false, "arm_upper", super::sorter::UPPER),
                        (true, "arm_lower", super::sorter::LOWER),
                    ]
                } else {
                    [
                        (false, "leg_upper", super::walker::UPPER),
                        (true, "leg_lower", super::walker::LOWER),
                    ]
                };
                for (lower, name, length) in names {
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
        // Shell scorches with its part.
        if p.material.as_deref() == Some("Shell") {
            ec.insert(Skin { part, level: 0 });
        }
        // And wears its valley (soot, rust, moss, dust).
        match p.material.as_deref() {
            Some("Shell") => {
                ec.insert(super::weathering::Weather { shell: true });
            }
            Some("Dark") | None => {
                ec.insert(super::weathering::Weather { shell: false });
            }
            _ => {}
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
                RotorDisc {
                    index: i as usize,
                    angle: 0.0,
                },
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
                        RotorDisc {
                            index: 0,
                            angle: 0.0,
                        },
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

/// Rotors spin with their thrust and tip toward where the drone is
/// trying to go, so you can read its intent off its rotors.
fn spin_rotors(
    drones: Query<(&Drone, &super::flight::Flight, &Transform, &Children)>,
    rotor_parts: Query<&Children, With<RobotPart>>,
    mut discs: Query<(&mut RotorDisc, &mut Transform), Without<Drone>>,
    time: Res<Time>,
) {
    for (d, flight, dt, children) in &drones {
        let a = dt.rotation.inverse() * flight.accel;
        let tip = Vec3::Y
            .cross(Vec3::new(a.x, 0.0, a.z) * 0.06)
            .clamp_length_max(0.3);
        for c in children.iter() {
            let Ok(grand) = rotor_parts.get(c) else {
                continue;
            };
            for g in grand.iter() {
                if let Ok((mut disc, mut t)) = discs.get_mut(g) {
                    let alive = d.rotor_alive[disc.index];
                    let rate = if alive {
                        40.0 + d.thrust[disc.index] * 3.0
                    } else {
                        0.0
                    };
                    let dir = if disc.index % 2 == 0 { 1.0 } else { -1.0 };
                    disc.angle = (disc.angle + rate * dir * time.delta_secs())
                        .rem_euclid(std::f32::consts::TAU);
                    // A dead rotor hangs bent on its mount.
                    let tilt = if alive {
                        Quat::from_scaled_axis(tip)
                    } else {
                        Quat::from_rotation_x(0.35) * Quat::from_rotation_z(0.2)
                    };
                    t.rotation = tilt * Quat::from_rotation_y(disc.angle);
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
    poses: Query<(&LegPoses, Option<&super::anim::Gait>)>,
) {
    for (e, seg, mut t) in &mut segments {
        let Ok((p, gait)) = poses.get(seg.robot) else {
            commands.entity(e).despawn();
            continue;
        };
        // Thickness with the body's size; length from the joints.
        let w = gait.map_or(1.0, |g| g.size);
        let [hip, knee, foot] = p.0[seg.leg];
        let (a, b) = if seg.lower { (knee, foot) } else { (hip, knee) };
        let len = a.distance(b).max(0.01);
        let dir = (b - a) / len;
        let Some(nominal) = seg.model_length else {
            *t = Transform::from_translation((a + b) * 0.5)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir))
                .with_scale(Vec3::new(w, len / 1.1, w));
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
            .with_scale(Vec3::new(w, len / nominal, w));
    }
}

fn dim_dead_eyes(
    eyes: Query<(Entity, &Eye, &MeshMaterial3d<StandardMaterial>)>,
    parts: Query<&RobotPart>,
    robots: Query<&Robot>,
    debris: Query<(), With<super::wreck::Debris>>,
    mats: Option<Res<RobotMats>>,
    mut commands: Commands,
) {
    let Some(m) = mats else {
        return;
    };
    for (e, eye, mat) in &eyes {
        let dead = debris.contains(eye.part)
            || parts
                .get(eye.part)
                .is_ok_and(|part| !part.alive() || robots.get(part.robot).is_ok_and(|r| r.wrecked));
        if dead && mat.0 != m.eye_dead {
            commands
                .entity(e)
                .insert(MeshMaterial3d(m.eye_dead.clone()));
        }
    }
}

/// An eye dying: it flares and stutters at its socket for [`LAST_FLICKER`]
/// seconds, then it's out for good (`dim_dead_eyes`; `fx` throws the
/// socket's sparks). A still frame at 30 m catches it going.
#[derive(Component)]
struct LastFlicker {
    left: f32,
    radius: f32,
    /// The flare itself, which reads by day when the halo doesn't.
    core: Handle<StandardMaterial>,
}

/// How long a dying eye flickers (s).
pub const LAST_FLICKER: f32 = 0.7;

/// Whether a dying eye is lit `left` seconds before it's out: three
/// stutters, the last the brightest, then dark.
pub fn flicker_lit(left: f32) -> f32 {
    let t = LAST_FLICKER - left;
    match t {
        t if t < 0.08 => 1.0,
        t if t < 0.2 => 0.0,
        t if t < 0.28 => 0.6,
        t if t < 0.45 => 0.0,
        t if t < 0.6 => 1.4,
        _ => 0.0,
    }
}

fn last_flicker(
    mut commands: Commands,
    mut deaths: MessageReader<super::PartDestroyed>,
    robots: Query<(&Robot, &GlobalTransform, Option<&super::anim::Gait>)>,
    mut flickers: Query<(Entity, &mut LastFlicker, &mut crate::world::halos::Halo)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flare: Local<Option<Handle<Mesh>>>,
    time: Res<Time>,
) {
    for d in deaths.read() {
        if d.role != PartRole::Sensor {
            continue;
        }
        let Ok((robot, gt, gait)) = robots.get(d.robot) else {
            continue;
        };
        let radius = if robot.kind == RobotKind::Drone {
            0.6
        } else {
            1.2 * gait.map_or(1.0, |g| g.size)
        };
        // Riding with the body, at the socket.
        let local = gt.affine().inverse().transform_point3(d.at);
        let core = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            alpha_mode: AlphaMode::Add,
            fog_enabled: false,
            ..default()
        });
        let mesh = flare
            .get_or_insert_with(|| meshes.add(Sphere::new(0.12).mesh().ico(2).unwrap()))
            .clone();
        commands.spawn((
            LastFlicker {
                left: LAST_FLICKER,
                radius,
                core: core.clone(),
            },
            Mesh3d(mesh),
            MeshMaterial3d(core),
            NotShadowCaster,
            crate::world::halos::Halo {
                color: LinearRgba::BLACK,
                radius: 0.0,
            },
            Transform::from_translation(local),
            Visibility::default(),
            ChildOf(d.robot),
        ));
    }
    let dt = time.delta_secs();
    for (e, mut f, mut halo) in &mut flickers {
        f.left -= dt;
        if f.left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let k = flicker_lit(f.left);
        if let Some(mut m) = materials.get_mut(&f.core) {
            m.emissive = LinearRgba::rgb(12.0, 2.4, 0.7) * k;
        }
        let want = crate::world::halos::Halo {
            color: LinearRgba::rgb(1.0, 0.25, 0.08) * 3.0 * k,
            radius: if k > 0.0 { f.radius } else { 0.0 },
        };
        halo.set_if_neq(want);
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
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
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
    dur: f32,
}

fn lock_flares(
    mut commands: Commands,
    mut locks: MessageReader<super::drone::DroneLock>,
    mut aims: MessageReader<super::walker::WalkerAim>,
    mut windups: MessageReader<super::sorter::SorterWindup>,
    mut flares: Query<(Entity, &mut LockFlare, &mut PointLight)>,
    time: Res<Time>,
) {
    for l in locks.read() {
        if let Ok(mut c) = commands.get_entity(l.drone) {
            c.with_child((
                LockFlare {
                    left: super::drone::LOCK_TIME,
                    dur: super::drone::LOCK_TIME,
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
    // The Sorter about to throw: a red flare off its head.
    for w in windups.read() {
        if let Ok(mut c) = commands.get_entity(w.sorter) {
            c.with_child((
                LockFlare {
                    left: super::sorter::WINDUP,
                    dur: super::sorter::WINDUP,
                },
                PointLight {
                    color: Color::srgb(1.0, 0.12, 0.08),
                    intensity: 0.0,
                    range: 12.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, -0.5, -0.6),
            ));
        }
    }
    // A walker's turret settling: the same red flare, off its gun.
    for a in aims.read() {
        if let Ok(mut c) = commands.get_entity(a.walker) {
            c.with_child((
                LockFlare {
                    left: super::walker::AIM_TIME,
                    dur: super::walker::AIM_TIME,
                },
                PointLight {
                    color: Color::srgb(1.0, 0.12, 0.08),
                    intensity: 0.0,
                    range: 9.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.5, -0.45),
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
        let k = 1.0 - f.left / f.dur;
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

/// Shell goes grey and matt as its part is shot up, and black when it's
/// destroyed or the robot is wrecked (the core burns; the rest scorches).
fn scorch(
    mut commands: Commands,
    mut skins: Query<(Entity, &mut Skin)>,
    parts: Query<&RobotPart>,
    debris: Query<(), With<super::wreck::Debris>>,
    robots: Query<&Robot>,
    mats: Option<Res<RobotMats>>,
) {
    let Some(m) = mats else {
        return;
    };
    for (e, mut skin) in &mut skins {
        let level = if let Ok(part) = parts.get(skin.part) {
            let wrecked = robots.get(part.robot).is_ok_and(|r| r.wrecked);
            let hurt = part.hp / part.max_hp.max(1.0);
            // A wrecked core is scorched all over and burnt black in
            // patches (one panel in four, by the mesh), so it still reads
            // as a robot's shell.
            let patch = e.index_u32().wrapping_mul(2_654_435_761) >> 30 == 0;
            if hurt <= 0.0 && part.role != PartRole::Core
                || (wrecked && part.role == PartRole::Core && patch)
            {
                2
            } else if hurt < 0.5 || wrecked {
                1
            } else {
                0
            }
        } else if debris.contains(skin.part) {
            1
        } else {
            continue;
        };
        if level <= skin.level {
            continue;
        }
        skin.level = level;
        let mat = if level == 2 {
            m.burnt.clone()
        } else {
            m.scorched.clone()
        };
        commands.entity(e).insert(MeshMaterial3d(mat));
    }
}

/// Dresses a loose piece of wreck from its robot's model (or a primitive).
#[allow(clippy::too_many_arguments)]
fn dress_debris(
    mut commands: Commands,
    pieces: Query<(Entity, &super::wreck::Debris), Added<super::wreck::Debris>>,
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
    use super::wreck::Piece;
    if pieces.is_empty() {
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
    for (e, d) in &pieces {
        let model = models.request(model_path(d.kind), &server).and_then(|h| {
            match Models::state(&h, &server, &gltfs) {
                ModelState::Ready(g) => Some(model_nodes(g, &nodes, &gltf_meshes)),
                _ => None,
            }
        });
        let mut mat = |name: Option<&str>| {
            robot_material(name, &m, &mut model_mats, &mut lib, &server, &mut materials)
        };
        let node = |name: &str| model.as_deref().and_then(|n| ModelNode::find(n, name));
        let dressed = match d.piece {
            Piece::Arm(k) => {
                let (name, len) = if k == 0 {
                    ("arm_upper", super::sorter::UPPER)
                } else {
                    ("arm_lower", super::sorter::LOWER)
                };
                node(name).map(|n| {
                    attach(
                        &mut commands,
                        e,
                        n,
                        Transform::from_translation(Vec3::NEG_Y * len * 0.5),
                        e,
                        &mut mat,
                    );
                })
            }
            Piece::LegUpper | Piece::LegLower => {
                let (name, len) = if d.piece == Piece::LegUpper {
                    ("leg_upper", super::walker::UPPER)
                } else {
                    ("leg_lower", super::walker::LOWER)
                };
                node(name).map(|n| {
                    // The model runs +Y from its near joint; the piece is
                    // centred on the segment.
                    attach(
                        &mut commands,
                        e,
                        n,
                        Transform::from_translation(Vec3::NEG_Y * len * 0.5),
                        e,
                        &mut mat,
                    );
                })
            }
            Piece::Plate => {
                let name = if d.kind != RobotKind::Drone {
                    "plate"
                } else {
                    "panel"
                };
                node(name).map(|n| {
                    attach(&mut commands, e, n, plate_turn(d.kind, n), e, &mut mat);
                })
            }
            Piece::Sensor => node("sensor").map(|n| {
                attach(&mut commands, e, n, Transform::IDENTITY, e, &mut mat);
            }),
            Piece::Gun => node("gun").map(|n| {
                attach(&mut commands, e, n, Transform::IDENTITY, e, &mut mat);
            }),
            Piece::Turret => node("turret").map(|n| {
                attach(&mut commands, e, n, Transform::IDENTITY, e, &mut mat);
            }),
            Piece::Rotor => node("rotor").map(|n| {
                attach(&mut commands, e, n, Transform::IDENTITY, e, &mut mat);
                if let (Some(b), Some(r)) = (node("blades"), node("rotor")) {
                    attach(
                        &mut commands,
                        e,
                        b,
                        Transform::from_translation(
                            b.transform.translation - r.transform.translation,
                        ),
                        e,
                        &mut mat,
                    );
                }
            }),
            // The model's loose panel, turned so its outside faces up (the
            // piece's +Y, as it was thrown).
            Piece::Panel(_) => node("panel").map(|n| {
                // A walker's panel faces out from the orb's centre; a
                // drone's (off the top of the lens) already faces up.
                let out = if d.kind == RobotKind::Walker {
                    n.transform.translation.normalize_or(Vec3::Y)
                } else {
                    Vec3::Y
                };
                attach(
                    &mut commands,
                    e,
                    n,
                    Transform::from_rotation(Quat::from_rotation_arc(out, Vec3::Y)),
                    e,
                    &mut mat,
                );
            }),
        };
        if dressed.is_some() {
            continue;
        }
        // Primitives, when there's no model (a drone's panel always).
        match d.piece {
            Piece::Panel(k) => {
                let (w, l) = if k == 2 { (0.22, 0.18) } else { (0.3, 0.24) };
                commands.spawn((
                    Mesh3d(meshes.add(panel_mesh(w, l, 0.42))),
                    MeshMaterial3d(m.scorched.clone()),
                    ChildOf(e),
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(w * 0.92, 0.012, l * 0.92))),
                    MeshMaterial3d(m.dark.clone()),
                    Transform::from_xyz(0.0, -0.012, 0.0),
                    ChildOf(e),
                ));
            }
            Piece::LegUpper | Piece::LegLower => {
                let (r, len) = if d.piece == Piece::LegUpper {
                    (0.06, super::walker::UPPER)
                } else {
                    (0.045, super::walker::LOWER)
                };
                commands.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(r, len - 2.0 * r))),
                    MeshMaterial3d(m.scorched.clone()),
                    ChildOf(e),
                ));
            }
            Piece::Sensor | Piece::Gun | Piece::Plate | Piece::Arm(_) => {
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.08, 0.06, 0.2))),
                    MeshMaterial3d(m.dark.clone()),
                    ChildOf(e),
                ));
            }
            Piece::Turret => {
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.16, 0.1, 0.36))),
                    MeshMaterial3d(m.scorched.clone()),
                    ChildOf(e),
                ));
            }
            Piece::Rotor => {
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.15, 0.01))),
                    MeshMaterial3d(m.dark.clone()),
                    ChildOf(e),
                ));
            }
        }
    }
}

/// How a plate's model sits on its part, whose +Y faces out: a walker's
/// plate is modelled facing out from the orb at its origin, a drone's
/// (its top panel, worn under the belly) already faces +Y.
fn plate_turn(kind: RobotKind, node: &ModelNode) -> Transform {
    if kind == RobotKind::Walker {
        let out = node.transform.translation.normalize_or(Vec3::Y);
        Transform::from_rotation(Quat::from_rotation_arc(out, Vec3::Y))
    } else {
        Transform::IDENTITY
    }
}

/// A curved shell plate `w` x `l`, bulging up (+Y) as a piece of a sphere
/// of radius `r`, its middle at the origin.
fn panel_mesh(w: f32, l: f32, r: f32) -> Mesh {
    const N: usize = 6;
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    for j in 0..=N {
        for i in 0..=N {
            let x = (i as f32 / N as f32 - 0.5) * w;
            let z = (j as f32 / N as f32 - 0.5) * l;
            let p = Vec3::new(x, r, z).normalize() * r - Vec3::Y * r;
            let n = (p + Vec3::Y * r).normalize();
            pos.push(p.to_array());
            nrm.push(n.to_array());
            uv.push([i as f32 / N as f32, j as f32 / N as f32]);
        }
    }
    let mut idx = Vec::new();
    for j in 0..N {
        for i in 0..N {
            let a = (j * (N + 1) + i) as u32;
            let b = a + 1;
            let c = a + (N + 1) as u32;
            let d = c + 1;
            idx.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(bevy::mesh::Indices::U32(idx))
}

/// An eye guttering this badly (0 steady, 1 all but out: a hurt sensor, a
/// failing core, `damage`): it drops out now and then, more the worse it
/// is, like a failing supply.
fn guttering(bad: f32, now: f32, k: u32) -> f32 {
    if bad <= 0.0 {
        return 1.0;
    }
    // Two slow beats against each other, so the drop-outs don't tick.
    let n = (now * 5.3 + k as f32).sin() * (now * 2.1 + k as f32 * 0.7).sin();
    if n > 0.85 - 0.6 * bad { 0.2 } else { 1.0 }
}

/// A robot's own eye material, so its eye can say how alert it is.
#[derive(Component)]
struct AlertEye(Handle<StandardMaterial>, Handle<StandardMaterial>);

/// Eyes say what the robot knows: a dull red at work, brighter as its
/// suspicion rises, and hot (toward orange-white) while it can make you
/// out. In the dark or fog that change is the first thing you see.
fn alert_eyes(
    mut commands: Commands,
    eyes: Query<(Entity, &Eye, &MeshMaterial3d<StandardMaterial>)>,
    mut halos: Query<(&Eye, &mut crate::world::halos::Halo)>,
    gaits: Query<&super::anim::Gait>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    places: Query<&GlobalTransform, With<Robot>>,
    parts: Query<&RobotPart>,
    robots: Query<(Entity, &super::Awareness, Option<&AlertEye>, &Robot)>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Option<Res<RobotMats>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<std::collections::HashMap<Entity, Handle<StandardMaterial>>>,
    mut impacts: MessageReader<crate::weapons::impacts::Impact>,
    mut flicker: Local<std::collections::HashMap<Entity, f32>>,
    time: Res<Time>,
) {
    // A shot into the eye makes it stutter for a moment.
    for imp in impacts.read() {
        if imp.part == Some(PartRole::Sensor) && imp.hurt {
            if let Some(part) = imp.entity.and_then(|e| parts.get(e).ok()) {
                flicker.insert(part.robot, 0.6);
            }
        }
    }
    let dt = time.delta_secs();
    flicker.retain(|_, t| {
        *t -= dt;
        *t > 0.0
    });
    let now = time.elapsed_secs();
    let Some(m) = mats else {
        return;
    };
    made.retain(|e, _| robots.contains(*e));
    // Sensor eyes still on the shared material get their robot's own.
    for (e, eye, mat) in &eyes {
        if mat.0 != m.eye {
            continue;
        }
        let Ok(part) = parts.get(eye.part) else {
            continue;
        };
        if part.role != PartRole::Sensor || !robots.contains(part.robot) {
            continue;
        }
        let kind = robots.get(part.robot).map(|r| r.3.kind).ok();
        let own = made
            .entry(part.robot)
            .or_insert_with(|| {
                let base = materials.get(&m.eye).cloned().unwrap_or_default();
                let h = materials.add(base);
                // A soft glow round the eye, so in the dusk and the dark
                // you see its mood before you see it.
                // (Lit by its emissive alone, so it can outshine the
                // night: an unlit colour stops at white.)
                let glow = materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    alpha_mode: AlphaMode::Add,
                    fog_enabled: false,
                    ..default()
                });
                let r = if kind == Some(RobotKind::Drone) {
                    0.08
                } else {
                    0.15
                };
                commands.spawn((
                    Mesh3d(meshes.add(Sphere::new(r).mesh().ico(2).unwrap())),
                    MeshMaterial3d(glow.clone()),
                    Transform::from_xyz(0.0, 0.0, -0.03),
                    NotShadowCaster,
                    ChildOf(eye.part),
                ));
                commands
                    .entity(part.robot)
                    .insert(AlertEye(h.clone(), glow));
                h
            })
            .clone();
        commands.entity(e).insert((
            MeshMaterial3d(own),
            // After dark, a glow that carries (`world::halos`): set below.
            crate::world::halos::Halo {
                color: LinearRgba::BLACK,
                radius: 0.0,
            },
        ));
    }
    let eye_at = cameras.iter().next().map(|g| g.translation());
    let mut moods: std::collections::HashMap<Entity, (LinearRgba, f32)> = Default::default();
    let light = conditions.map_or(1.0, |c| c.light);
    // The glow tells more the darker it is.
    let dark = (1.2 - light).clamp(0.3, 1.0);
    // After dark the eye's core dims to what it reads as by day: the
    // night's open exposure would clip it to white otherwise and its red
    // would be lost; the halo carries it at range.
    let night = (1.0 - light / 0.25).clamp(0.0, 1.0);
    let gain = 1.0 - 0.75 * night;
    // How badly each robot's eye gutters (`damage`: a hurt sensor, a
    // failing core): 0 steady, 1 all but out.
    let mut cores: std::collections::HashMap<Entity, f32> = Default::default();
    for p in &parts {
        let h = p.hp / p.max_hp.max(1.0);
        let bad = match p.role {
            PartRole::Core => (1.0 - h / 0.4).clamp(0.0, 1.0),
            PartRole::Sensor if h > 0.0 => (1.0 - h / 0.7).clamp(0.0, 1.0),
            _ => continue,
        };
        if super::damage::tell(p.role, h).motion == super::damage::Motion::Gutter {
            let b = cores.entry(p.robot).or_insert(0.0);
            *b = b.max(bad);
        }
    }
    for (robot, aware, own, _) in &robots {
        let Some(own) = own else {
            continue;
        };
        let stutter = if flicker.contains_key(&robot) {
            if (now * 37.0 + robot.index_u32() as f32).sin() > 0.1 {
                0.15
            } else {
                1.6
            }
        } else {
            1.0
        } * guttering(
            cores.get(&robot).copied().unwrap_or(0.0),
            now,
            robot.index_u32(),
        );
        let a = aware.suspicion.clamp(0.0, 1.0);
        let x = if aware.can_see_target {
            aware.exposure.clamp(0.0, 1.0)
        } else {
            0.0
        };
        // Its colour and, for eyes that can't tell red from orange (or a
        // look from far off), its rhythm (`render::tells`); brighter after
        // dark.
        let [r, g, b] = crate::render::tells::eye_colour(a, x);
        let pulse = crate::render::tells::eye_pulse(
            a,
            aware.can_see_target,
            now + robot.index_u32() as f32 * 0.37,
        );
        let want = LinearRgba::rgb(r, g, b) * stutter * pulse * gain;
        if let Some(mut mat) = materials.get_mut(&own.0) {
            if mat.emissive != want {
                mat.emissive = want;
            }
        }
        // Near, the sphere is the eye's glow; from 15 m the halo takes over.
        let near = match (eye_at, places.get(robot)) {
            (Some(c), Ok(gt)) => {
                (1.0 - (c.distance(gt.translation()) - 15.0) / 10.0).clamp(0.0, 1.0)
            }
            _ => 1.0,
        };
        let k = (1.2 + 1.2 * a + 1.6 * x) * stutter.min(1.0);
        moods.insert(
            robot,
            (
                LinearRgba::rgb(1.0, 0.12 + 0.3 * x, 0.05 + 0.1 * x) * k,
                if stutter < 1.0 { 0.0 } else { 1.0 },
            ),
        );
        let alpha = (0.03 + 0.12 * night + 0.12 * a + 0.2 * x) * dark * stutter.min(1.0) * near;
        let tint = LinearRgba::rgb(1.0, 0.08 + 0.3 * x, 0.03 + 0.08 * x) * alpha * 14.0;
        if let Some(mut g) = materials.get_mut(&own.1) {
            if g.emissive != tint {
                g.emissive = tint;
            }
        }
    }
    for (eye, mut halo) in &mut halos {
        let Ok(part) = parts.get(eye.part) else {
            continue;
        };
        let alive = part.alive();
        let want = match (alive, moods.get(&part.robot), robots.get(part.robot)) {
            (true, Some(&(color, on)), Ok((_, _, _, r))) if !r.wrecked && on > 0.0 => {
                // Wide enough to read as a point at 60-80 m on a clear
                // night, when there's no haze to spread it.
                let radius = if r.kind == RobotKind::Drone {
                    0.6
                } else {
                    1.2 * gaits.get(part.robot).map_or(1.0, |g| g.size)
                };
                crate::world::halos::Halo { color, radius }
            }
            // Dead (or stuttering off): dark.
            _ => crate::world::halos::Halo {
                color: LinearRgba::BLACK,
                radius: 0.0,
            },
        };
        halo.set_if_neq(want);
    }
}

/// The shell's own faint glow (what keeps a robot pale against the forest
/// by day, `robot_material`) dims with the light: at night a robot is a
/// dark shape with its eyes and running lights, not a lit ghost.
fn shell_by_light(
    conditions: Option<Res<crate::world::weather::Conditions>>,
    mats: Option<Res<RobotMats>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut done: Local<Option<f32>>,
    mut day_rough: Local<Option<f32>>,
    zone: Option<Res<crate::world::Zone>>,
) {
    let (Some(c), Some(m)) = (conditions, mats) else {
        return;
    };
    if *done == Some(c.light) {
        return;
    }
    *done = Some(c.light);
    let shell = robot_material(
        Some("Shell"),
        &m,
        &mut model_mats,
        &mut lib,
        &server,
        &mut materials,
    );
    let valley = zone.map_or(super::weathering::Valley::None, |z| {
        super::weathering::Valley::of_map(&z.map.name)
    });
    // After dark the shell keeps none of it: the eyes are the tell then.
    let dusk = (c.light / 0.25).clamp(0.0, 1.0);
    let k = 0.22 * c.light.sqrt() * dusk * valley.shell_glow();
    if let Some(mut mat) = materials.get_mut(&shell) {
        mat.emissive = LinearRgba::rgb(k, k, k * 1.05);
        // A clear moon glints off the shell: right, but it mustn't out-read
        // the eyes, so after dark the glint spreads into a soft sheen.
        let day = *day_rough.get_or_insert(mat.perceptual_roughness);
        mat.perceptual_roughness = day.max(0.55) + (day - day.max(0.55)) * dusk;
    }
    // The seams too: by day a cool line, after dark out. Any glow left in
    // the night's open exposure reads as a cyan point beside the eye's red
    // and muddles which robot is looking at you.
    if let Some(mut mat) = materials.get_mut(&m.glow) {
        mat.emissive = LinearRgba::rgb(1.2, 7.0, 9.0) * (dusk * dusk);
    }
}

#[cfg(test)]
mod gutter_tests {
    use super::guttering;

    /// A core's eye: steady until 40%, then worse the lower.
    fn gutter(core: f32, now: f32, k: u32) -> f32 {
        guttering(if core >= 0.4 { 0.0 } else { 1.0 - core / 0.4 }, now, k)
    }

    /// A whole core's eye never gutters; a hurt one drops out now and
    /// then, more the worse it's hurt.
    #[test]
    fn a_hurt_core_gutters_its_eye() {
        let out = |core: f32| {
            (0..900)
                .filter(|i| gutter(core, *i as f32 / 90.0, 7) < 1.0)
                .count()
        };
        assert_eq!(out(1.0), 0);
        assert_eq!(out(0.45), 0);
        let (hurt, dying) = (out(0.3), out(0.05));
        assert!(hurt > 0 && dying > hurt, "{hurt} then {dying} of 900");
        assert!(dying < 600, "it still mostly shines ({dying} of 900 out)");
    }
}

/// How far a walker parts plants, metres (times its size), round a spot
/// low under its orb: the sway shaders part by 3D distance, and the
/// ground cover is down round its feet, not up at the orb.
const PARTS: f32 = 0.9;
const PARTS_LOW: f32 = 0.85;
/// Robots farther from the eye than this part nothing (nobody sees it).
const PARTS_WITHIN: f32 = 20.0;

/// Walkers and hounds push through ground cover and low branches: the
/// four nearest the eye go to the sway shaders (`world::cover`). Each
/// write re-extracts every plant, so a spot only changes once its robot
/// has moved 5 cm, and an empty slot keeps nothing.
fn part_plants(
    pushers: Option<ResMut<crate::world::cover::RobotPushers>>,
    walkers: Query<(&GlobalTransform, &super::anim::Gait, &Robot), With<super::walker::Walker>>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
) {
    let (Some(mut pushers), Some(eye)) = (pushers, cameras.iter().next()) else {
        return;
    };
    let spots = pushing(
        eye.translation(),
        walkers
            .iter()
            .filter(|(_, _, r)| !r.wrecked)
            .map(|(gt, g, _)| {
                (gt.translation() - Vec3::Y * PARTS_LOW * g.size).extend(PARTS * g.size)
            }),
        pushers.spots,
    );
    pushers.set_if_neq(crate::world::cover::RobotPushers { spots });
}

/// The four pushes nearest `eye` (within `PARTS_WITHIN`), each left as
/// it `was` while its robot has moved under 5 cm.
fn pushing(eye: Vec3, robots: impl Iterator<Item = Vec4>, was: [Vec4; 4]) -> [Vec4; 4] {
    let mut near: Vec<(f32, Vec4)> = robots
        .map(|p| (p.truncate().distance_squared(eye), p))
        .filter(|(d2, _)| *d2 < PARTS_WITHIN * PARTS_WITHIN)
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut spots = [Vec4::ZERO; 4];
    for (i, (_, p)) in near.into_iter().take(4).enumerate() {
        spots[i] = if was[i].w > 0.0 && was[i].truncate().distance(p.truncate()) < 0.05 {
            was[i]
        } else {
            p
        };
    }
    spots
}

#[cfg(test)]
mod pusher_tests {
    use super::*;

    /// The nearest four robots part the plants, none past 20 m; a robot
    /// that's barely moved keeps its last spot (no rewrite).
    #[test]
    fn the_nearest_robots_part_the_plants() {
        let eye = Vec3::ZERO;
        let robots: Vec<Vec4> = [3.0, 25.0, 8.0, 1.0, 15.0, 12.0]
            .iter()
            .map(|x| Vec4::new(*x, 1.1, 0.0, 0.6))
            .collect();
        let spots = pushing(eye, robots.iter().copied(), [Vec4::ZERO; 4]);
        let xs: Vec<f32> = spots.iter().map(|s| s.x).collect();
        assert_eq!(xs, vec![1.0, 3.0, 8.0, 12.0]);
        // Nudged 2 cm: the same spots, so nothing is written.
        let nudged = robots.iter().map(|r| *r + Vec4::new(0.02, 0.0, 0.0, 0.0));
        assert_eq!(pushing(eye, nudged, spots), spots);
        // Walked off 30 cm: that one moves.
        let walked = robots.iter().map(|r| *r + Vec4::new(0.3, 0.0, 0.0, 0.0));
        assert_ne!(pushing(eye, walked, spots)[0], spots[0]);
        // None near: nothing parts.
        let far = [Vec4::new(40.0, 1.0, 0.0, 0.6)];
        assert_eq!(pushing(eye, far.into_iter(), spots), [Vec4::ZERO; 4]);
    }
}

/// A drone's searchlight after dark: a spotlight down and ahead, which
/// `world::beams` gives a cone of lit air (dust on a dry night, a shaft
/// through rain or fog).
#[derive(Component)]
struct DroneLamp;

/// Darker than this, a drone hunts with its lamp lit.
const LAMP_BELOW: f32 = 0.3;
/// Farther from the eye than this it's left off (a light each: they cost).
const LAMP_WITHIN: f32 = 60.0;

/// Whether a drone's lamp is lit, at this light and this far from the eye.
fn lamp_lit(light: f32, distance: f32, wrecked: bool) -> bool {
    !wrecked && light < LAMP_BELOW && distance < LAMP_WITHIN
}

fn drone_lamps(
    mut commands: Commands,
    drones: Query<(Entity, &GlobalTransform, &Robot, Option<&Children>), With<super::drone::Drone>>,
    lamps: Query<(), With<DroneLamp>>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
) {
    let light = conditions.map_or(1.0, |c| c.light);
    let eye = cameras.iter().next().map(|g| g.translation());
    for (e, gt, robot, children) in &drones {
        let d = eye.map_or(f32::MAX, |c| c.distance(gt.translation()));
        let lit = lamp_lit(light, d, robot.wrecked);
        let lamp = children.and_then(|c| c.iter().find(|c| lamps.contains(*c)));
        match (lit, lamp) {
            (true, None) => {
                commands.spawn((
                    DroneLamp,
                    SpotLight {
                        color: Color::srgb(0.85, 0.92, 1.0),
                        intensity: 400_000.0,
                        range: 40.0,
                        inner_angle: 0.1,
                        outer_angle: 0.3,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    // Under the belly, down and ahead (-Z is its front).
                    Transform::from_xyz(0.0, -0.12, -0.1)
                        .looking_to(Vec3::new(0.0, -1.0, -0.7), Vec3::NEG_Z),
                    ChildOf(e),
                ));
            }
            (false, Some(l)) => commands.entity(l).despawn(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod lamp_tests {
    use super::lamp_lit;

    /// A drone lights its lamp after dark, near enough to be seen, and
    /// never once it's wrecked.
    #[test]
    fn drones_light_their_lamps_after_dark() {
        assert!(lamp_lit(0.02, 20.0, false));
        assert!(!lamp_lit(0.8, 20.0, false), "not by day");
        assert!(!lamp_lit(0.02, 90.0, false), "not out of sight");
        assert!(!lamp_lit(0.02, 20.0, true), "not wrecked");
    }
}
