//! How a mender looks (client only): dressed from
//! `assets/models/robots/mender.glb` (`art/robots/mender.py`) in three
//! LODs, its four rotors spinning. Welding, its torch's arc flickers
//! blue-white: a light you see from far off.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;

use super::drone::Drone;
use super::heavy_look::{lod_name, lod_range};
use super::mender::{ARM, Mender, ROTORS, TORCH, WeldArm};
use super::{PartRole, RobotPart};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;

const MODEL: &str = "models/robots/mender.glb";
/// Where the arm part sits against the arm's shoulder (see `mender`).
const ARM_PART: Vec3 = Vec3::new(0.0, -0.13, -0.15);
/// The blades over each rotor hub.
const BLADES_UP: f32 = 0.02;

pub struct MenderLookPlugin;

impl Plugin for MenderLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress, spin, arcs));
    }
}

#[derive(Component)]
struct Dressed;

/// A rotor's blades (which rotor).
#[derive(Component)]
struct Blades(usize);

/// Its torch's arc light.
#[derive(Component)]
struct Arc(Entity);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dress(
    mut commands: Commands,
    menders: Query<(Entity, &Children), (With<Mender>, Without<Dressed>)>,
    parts: Query<(&RobotPart, Has<WeldArm>)>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if menders.is_empty() {
        return;
    }
    let Some(h) = models.request(MODEL, &server) else {
        return;
    };
    let model = match Models::state(&h, &server, &gltfs) {
        ModelState::Ready(g) => model_nodes(g, &nodes, &gltf_meshes),
        ModelState::Loading => return,
        ModelState::Failed => {
            warn!("mender model failed to load");
            for (e, _) in &menders {
                commands.entity(e).insert(Dressed);
            }
            return;
        }
    };
    let mut hang = |commands: &mut Commands,
                    materials: &mut Assets<StandardMaterial>,
                    parent: Entity,
                    name: &str,
                    at: Transform|
     -> Entity {
        let holder = commands
            .spawn((at, Visibility::default(), ChildOf(parent)))
            .id();
        for lod in 0..3 {
            let Some(node) = ModelNode::find(&model, &lod_name(name, lod)) else {
                continue;
            };
            for p in &node.primitives {
                let m = match p.material.as_deref() {
                    Some(n) => {
                        model_mats.get(super::weak_points::paint(n), &mut lib, &server, materials)
                    }
                    None => model_mats.get("Dark", &mut lib, &server, materials),
                };
                let mut ec =
                    commands.spawn((Mesh3d(p.mesh.clone()), MeshMaterial3d(m), ChildOf(holder)));
                if let Some(r) = lod_range(lod) {
                    ec.insert(r);
                }
                if matches!(p.material.as_deref(), Some("Shell")) {
                    ec.insert(super::weathering::Weather { shell: true });
                }
            }
        }
        holder
    };
    for (e, children) in &menders {
        hang(
            &mut commands,
            &mut materials,
            e,
            "core",
            Transform::IDENTITY,
        );
        for (i, at) in ROTORS.iter().enumerate() {
            hang(
                &mut commands,
                &mut materials,
                e,
                "rotor",
                Transform::from_translation(*at),
            );
            let b = hang(
                &mut commands,
                &mut materials,
                e,
                "blades",
                Transform::from_translation(*at + Vec3::Y * BLADES_UP),
            );
            commands.entity(b).insert(Blades(i));
        }
        for c in children.iter() {
            let Ok((part, arm)) = parts.get(c) else {
                continue;
            };
            if arm {
                hang(
                    &mut commands,
                    &mut materials,
                    c,
                    "arm",
                    Transform::from_translation(-ARM_PART),
                );
                hang(
                    &mut commands,
                    &mut materials,
                    c,
                    "torch",
                    Transform::from_translation(TORCH - ARM - ARM_PART),
                );
            } else if part.role == PartRole::Sensor {
                hang(&mut commands, &mut materials, c, "eye", Transform::IDENTITY);
            }
        }
        let light = commands
            .spawn((
                PointLight {
                    color: Color::srgb(0.7, 0.85, 1.0),
                    intensity: 0.0,
                    range: 25.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(TORCH - Vec3::Y * 0.05),
                ChildOf(e),
            ))
            .id();
        commands.entity(e).insert((Arc(light), Dressed));
    }
}

fn spin(
    menders: Query<(&Drone, &Children), With<Mender>>,
    mut blades: Query<(&Blades, &mut Transform)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (d, children) in &menders {
        for c in children.iter() {
            if let Ok((b, mut t)) = blades.get_mut(c) {
                if d.rotor_alive[b.0] {
                    let rate = 40.0 + d.thrust[b.0] * 2.5;
                    let dir = if b.0 % 2 == 0 { 1.0 } else { -1.0 };
                    t.rotate_y(dir * rate * dt);
                }
            }
        }
    }
}

/// The arc: a hard blue-white flicker while it welds.
fn arcs(menders: Query<(&Mender, &Arc)>, mut lights: Query<&mut PointLight>, time: Res<Time>) {
    let t = time.elapsed_secs();
    for (m, arc) in &menders {
        if let Ok(mut l) = lights.get_mut(arc.0) {
            l.intensity = if m.welding.is_some() {
                let f = ((t * 53.0).sin() * (t * 31.0).sin()).abs();
                20_000.0 + 140_000.0 * f
            } else {
                0.0
            };
        }
    }
}
