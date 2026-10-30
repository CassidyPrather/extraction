//! How the Sorter looks beyond its parts: the rails and the bridge it
//! runs on, the grab and its jaws on the end of the arm, the totes, and a
//! festoon cable from the bridge to the carriage that sags and swings as
//! the carriage starts and stops.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::sorter::{ArmPose, Sorter, Tote};
use crate::models::{ModelNode, ModelState, Models, model_nodes, spawn_primitives};

const MODEL: &str = "models/robots/sorter.glb";
/// Links in the festoon cable.
const LINKS: usize = 10;

pub struct SorterLookPlugin;

impl Plugin for SorterLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress, follow, swing_cable).chain());
    }
}

/// The bridge of `sorter` (follows its carriage along the rails).
#[derive(Component)]
struct Bridge(Entity);

/// The grab on the end of `sorter`'s arm, and its jaws (+1 the one on +X).
#[derive(Component)]
struct Grab(Entity);
#[derive(Component)]
struct Jaw(f32);

/// One link of `sorter`'s festoon cable.
#[derive(Component)]
struct Link {
    sorter: Entity,
    k: usize,
}

/// The cable's swing (a damped pendulum driven by the carriage).
#[derive(Component, Default)]
struct Swing {
    angle: f32,
    rate: f32,
    last_v: f32,
}

#[derive(Component)]
struct Dressed;

#[allow(clippy::too_many_arguments)]
fn dress(
    mut commands: Commands,
    sorters: Query<(Entity, &Sorter), Without<Dressed>>,
    totes: Query<Entity, (With<Tote>, Without<Dressed>)>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<crate::models::ModelMaterials>,
    mut lib: ResMut<crate::render::materials::MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if sorters.is_empty() && totes.is_empty() {
        return;
    }
    let Some(h) = models.request(MODEL, &server) else {
        return;
    };
    let ModelState::Ready(g) = Models::state(&h, &server, &gltfs) else {
        return;
    };
    let model = model_nodes(g, &nodes, &gltf_meshes);
    let cable_mesh = meshes.add(Cylinder::new(0.025, 1.0));
    let cable_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.05, 0.05),
        perceptual_roughness: 0.8,
        ..default()
    });
    let mut hang = |commands: &mut Commands, parent: Entity, name: &str, at: Transform| {
        let Some(node) = ModelNode::find(&model, name) else {
            return;
        };
        let holder = commands
            .spawn((
                at * Transform {
                    translation: Vec3::ZERO,
                    ..node.transform
                },
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        spawn_primitives(commands, holder, node, |name| {
            model_mats.get(
                super::weak_points::paint(name.unwrap_or("Dark")),
                &mut lib,
                &server,
                &mut materials,
            )
        });
    };
    for e in &totes {
        commands.entity(e).insert(Dressed);
        hang(&mut commands, e, "tote", Transform::IDENTITY);
    }
    for (e, s) in &sorters {
        commands.entity(e).insert((Dressed, Swing::default()));
        // Two rails, under the ceiling either side.
        for z in [-super::sorter::RAIL_Z, super::sorter::RAIL_Z] {
            let rail = commands
                .spawn((
                    s.hall * Transform::from_xyz(0.0, s.carriage_y() + 0.35, z),
                    Visibility::default(),
                ))
                .id();
            hang(&mut commands, rail, "rail", Transform::IDENTITY);
        }
        let bridge = commands
            .spawn((Bridge(e), Transform::default(), Visibility::default()))
            .id();
        hang(&mut commands, bridge, "bridge", Transform::IDENTITY);
        let grab = commands
            .spawn((Grab(e), Transform::default(), Visibility::default()))
            .id();
        hang(&mut commands, grab, "grab", Transform::IDENTITY);
        for side in [1.0f32, -1.0] {
            let jaw = commands
                .spawn((
                    Jaw(side),
                    Transform::from_xyz(0.12 * side, -0.18, 0.0),
                    Visibility::default(),
                    ChildOf(grab),
                ))
                .id();
            // The +X jaw as modelled; the -X one turned round.
            let turn = if side > 0.0 {
                Transform::IDENTITY
            } else {
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
            };
            hang(&mut commands, jaw, "jaw", turn);
        }
        for k in 0..LINKS {
            commands.spawn((
                Link { sorter: e, k },
                Mesh3d(cable_mesh.clone()),
                MeshMaterial3d(cable_mat.clone()),
                Transform::default(),
                NotShadowCaster,
            ));
        }
    }
}

fn follow(
    sorters: Query<(&Sorter, &ArmPose)>,
    mut bridges: Query<(Entity, &Bridge, &mut Transform), Without<Grab>>,
    mut grabs: Query<(Entity, &Grab, &mut Transform, &Children), Without<Bridge>>,
    mut jaws: Query<(&Jaw, &mut Transform), (Without<Bridge>, Without<Grab>)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (b, bridge, mut t) in &mut bridges {
        let Ok((s, _)) = sorters.get(bridge.0) else {
            commands.entity(b).despawn();
            continue;
        };
        *t = s.hall * Transform::from_xyz(s.pos.x, s.carriage_y(), 0.0);
    }
    let dt = time.delta_secs();
    for (g, grab, mut t, children) in &mut grabs {
        let Ok((s, pose)) = sorters.get(grab.0) else {
            commands.entity(g).despawn();
            continue;
        };
        // Hung from the wrist, along the forearm, turned to the hall.
        let dir = (pose.elbow - pose.wrist).normalize_or(Vec3::Y);
        let hang = Quat::from_rotation_arc(Vec3::Y, dir);
        *t = Transform::from_translation(pose.wrist).with_rotation(hang * s.hall.rotation);
        for c in children.iter() {
            if let Ok((jaw, mut jt)) = jaws.get_mut(c) {
                // Shut on a tote, open otherwise.
                let open = 0.45 * (1.0 - pose.grip);
                let want = Quat::from_rotation_z(-open * jaw.0);
                jt.rotation = jt.rotation.slerp(want, (8.0 * dt).min(1.0));
            }
        }
    }
}

/// A festoon from the bridge's end truck to the carriage: a sag that
/// deepens as the carriage runs out, swinging back when it stops.
fn swing_cable(
    mut sorters: Query<(Entity, &Sorter, &mut Swing)>,
    mut links: Query<(Entity, &Link, &mut Transform)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (_, s, mut sw) in &mut sorters {
        // Driven by the carriage's acceleration along the bridge.
        let acc = (s.vel.y - sw.last_v) / dt;
        sw.last_v = s.vel.y;
        sw.rate += (-sw.angle * 9.0 - sw.rate * 1.6 - acc * 0.25) * dt;
        sw.angle = (sw.angle + sw.rate * dt).clamp(-0.6, 0.6);
    }
    for (e, link, mut t) in &mut links {
        let Ok((_, s, sw)) = sorters.get(link.sorter) else {
            commands.entity(e).despawn();
            continue;
        };
        let a = Vec3::new(s.pos.x, s.carriage_y() + 0.55, super::sorter::RAIL_Z);
        let b = Vec3::new(s.pos.x + 0.3, s.carriage_y() + 0.3, s.pos.y);
        let span = a.distance(b);
        let point = |u: f32| {
            let p = a.lerp(b, u);
            // A loop that hangs lower the longer it is, swung sideways.
            let sag = (0.15 + 0.06 * span) * (std::f32::consts::PI * u).sin();
            let swing = Vec3::new(sw.angle.sin(), -sw.angle.cos(), 0.0) * sag;
            p + swing
        };
        let u0 = link.k as f32 / LINKS as f32;
        let u1 = (link.k + 1) as f32 / LINKS as f32;
        let (p0, p1) = (s.world(point(u0)), s.world(point(u1)));
        let len = p0.distance(p1).max(0.01);
        *t = Transform::from_translation((p0 + p1) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, (p1 - p0) / len))
            .with_scale(Vec3::new(1.0, len, 1.0));
    }
}
