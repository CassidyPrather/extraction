//! The bench's looks: the modelled bench (`art/vehicles/bench.py`: plank
//! top on steel legs, tray, vise, tool rack), or boxes without it; a red
//! selector knob, a lever, and a work lamp.

use bevy::prelude::*;

use bevy::gltf::{Gltf, GltfMesh, GltfNode};

use super::materials::{Mat, MaterialLibrary, metric_box};
use crate::models::{ModelMaterials, ModelState, Models, model_nodes, spawn_primitives};
use crate::raid::bench::{BENCH_SIZE, Bench};
use crate::raid::truck::Pressable;

pub struct BenchVisualsPlugin;

impl Plugin for BenchVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (dress_bench, finish_bench_model).chain(),
                dress_controls,
                animate_lever,
            ),
        );
    }
}

/// The bench model (see `art/vehicles/bench.py`); without it, boxes.
const BENCH_MODEL: &str = "models/vehicles/bench.glb";

/// A bench waiting for its model to load (or fail).
#[derive(Component)]
struct PendingBenchModel(Handle<Gltf>);

fn dress_bench(
    mut commands: Commands,
    benches: Query<(Entity, &Bench), Added<Bench>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    mut models: ResMut<Models>,
    server: Res<AssetServer>,
) {
    for (e, bench) in &benches {
        let top = BENCH_SIZE.y * 0.5;
        commands.entity(e).with_child((
            PointLight {
                color: Color::srgb(1.0, 0.85, 0.6),
                intensity: 12_000.0,
                range: 3.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.1, top + 0.9, 0.0),
        ));
        if let Some(h) = models.request(BENCH_MODEL, &server) {
            commands.entity(e).insert(PendingBenchModel(h));
            continue;
        }
        bench_blockout(
            &mut commands,
            e,
            bench,
            &mut meshes,
            &mut materials,
            &mut library,
            &server,
        );
    }
}

/// Swaps the model in once it has loaded (its tray is part of it), or the
/// boxes if it failed.
#[allow(clippy::too_many_arguments)]
fn finish_bench_model(
    mut commands: Commands,
    pending: Query<(Entity, &Bench, &PendingBenchModel)>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
) {
    for (e, bench, p) in &pending {
        let gltf = match Models::state(&p.0, &server, &gltfs) {
            ModelState::Loading => continue,
            ModelState::Failed => {
                warn!("{BENCH_MODEL} failed to load; using boxes");
                commands.entity(e).remove::<PendingBenchModel>();
                bench_blockout(
                    &mut commands,
                    e,
                    bench,
                    &mut meshes,
                    &mut materials,
                    &mut library,
                    &server,
                );
                continue;
            }
            ModelState::Ready(g) => g,
        };
        commands.entity(e).remove::<PendingBenchModel>();
        for node in model_nodes(gltf, &nodes, &gltf_meshes) {
            let holder = commands
                .spawn((node.transform, Visibility::default(), ChildOf(e)))
                .id();
            spawn_primitives(&mut commands, holder, &node, |name| {
                model_mats.get(
                    name.unwrap_or("Paint"),
                    &mut library,
                    &server,
                    &mut materials,
                )
            });
        }
    }
}

/// The bench as boxes, for when there is no model.
fn bench_blockout(
    commands: &mut Commands,
    e: Entity,
    bench: &Bench,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    library: &mut MaterialLibrary,
    server: &AssetServer,
) {
    let wood = library.get(Mat::WoodFloor, server, materials);
    let steel = library.get(Mat::PaintedMetal, server, materials);
    let dark = library.get(Mat::DarkMetal, server, materials);
    let s = BENCH_SIZE;
    let top = s.y * 0.5;
    let mut part = |size: Vec3, at: Vec3, m: &Handle<StandardMaterial>, parent: Entity| {
        commands.spawn((
            Mesh3d(meshes.add(metric_box(size))),
            MeshMaterial3d(m.clone()),
            Transform::from_translation(at),
            ChildOf(parent),
        ));
    };
    part(Vec3::new(s.x, 0.05, s.z), Vec3::Y * (top - 0.025), &wood, e);
    for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        part(
            Vec3::new(0.04, s.y - 0.05, 0.04),
            Vec3::new(x * (s.x * 0.5 - 0.04), -0.025, z * (s.z * 0.5 - 0.04)),
            &dark,
            e,
        );
    }
    part(
        Vec3::new(s.x - 0.08, 0.03, s.z - 0.08),
        Vec3::Y * (-top + 0.15),
        &wood,
        e,
    );
    // A vise on the front corner.
    part(
        Vec3::new(0.12, 0.09, 0.16),
        Vec3::new(-s.x * 0.5 + 0.08, top + 0.045, -s.z * 0.5 + 0.1),
        &steel,
        e,
    );
    // The tray: a shallow sheet-steel box (floor and lips).
    if let Some(tray) = bench.tray {
        let (hx, hz) = (0.27, 0.43);
        part(
            Vec3::new(hx * 2.0, 0.008, hz * 2.0),
            Vec3::Y * -0.075,
            &steel,
            tray,
        );
        for (size, at) in [
            (Vec3::new(0.008, 0.04, hz * 2.0), Vec3::new(-hx, -0.06, 0.0)),
            (Vec3::new(0.008, 0.04, hz * 2.0), Vec3::new(hx, -0.06, 0.0)),
            (Vec3::new(hx * 2.0, 0.04, 0.008), Vec3::new(0.0, -0.06, -hz)),
            (Vec3::new(hx * 2.0, 0.04, 0.008), Vec3::new(0.0, -0.06, hz)),
        ] {
            part(size, at, &steel, tray);
        }
    }
}

#[derive(Component)]
struct LeverHandle;

fn dress_controls(
    mut commands: Commands,
    controls: Query<(Entity, &Pressable), Added<Pressable>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, p) in &controls {
        match p {
            Pressable::BenchSelect => {
                let red = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.08, 0.06),
                    perceptual_roughness: 0.4,
                    ..default()
                });
                commands.entity(e).insert((
                    Mesh3d(
                        meshes.add(
                            Cylinder::new(0.035, 0.03)
                                .mesh()
                                .resolution(16)
                                .build()
                                .rotated_by(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                        ),
                    ),
                    MeshMaterial3d(red),
                ));
            }
            Pressable::BenchRun => {
                let black = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.05, 0.05, 0.05),
                    perceptual_roughness: 0.5,
                    metallic: 0.6,
                    ..default()
                });
                commands.entity(e).with_child((
                    LeverHandle,
                    Mesh3d(meshes.add(Cuboid::new(0.025, 0.16, 0.025))),
                    MeshMaterial3d(black.clone()),
                    Transform::from_xyz(-0.02, 0.06, 0.0).with_rotation(Quat::from_rotation_z(0.4)),
                ));
                commands.entity(e).insert((
                    Mesh3d(meshes.add(Cuboid::new(0.03, 0.05, 0.06))),
                    MeshMaterial3d(black),
                ));
            }
            _ => {}
        }
    }
}

/// The lever leans back while the bench runs.
fn animate_lever(
    benches: Query<(&Bench, &Children)>,
    controls: Query<(&Pressable, &Children)>,
    mut handles: Query<&mut Transform, With<LeverHandle>>,
) {
    for (bench, children) in &benches {
        for c in children.iter() {
            let Ok((Pressable::BenchRun, kids)) = controls.get(c) else {
                continue;
            };
            for k in kids.iter() {
                if let Ok(mut t) = handles.get_mut(k) {
                    let a = if bench.running.is_some() { -0.5 } else { 0.4 };
                    t.rotation = Quat::from_rotation_z(a);
                }
            }
        }
    }
}
