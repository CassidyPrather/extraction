//! The camp kitchen's looks: a folding table, a board and knife, an enamel
//! bowl and whisk, a pot and a pan on a two-ring stove, a tin oven, a crock,
//! and a blue flame under whatever's cooking.

use bevy::prelude::*;
use extraction_core::cooking::Drive;

use super::materials::{Mat, MaterialLibrary, metric_box};
use crate::raid::kitchen::{Cook, CookControl, KitchenTable};

pub struct KitchenVisualsPlugin;

impl Plugin for KitchenVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress_table, dress_stations, flames));
    }
}

fn dress_table(
    mut commands: Commands,
    tables: Query<Entity, Added<KitchenTable>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for e in &tables {
        let wood = library.get(Mat::Planks, &server, &mut materials);
        let steel = library.get(Mat::DarkMetal, &server, &mut materials);
        commands.entity(e).insert((
            Mesh3d(meshes.add(metric_box(Vec3::new(1.9, 0.03, 0.6)))),
            MeshMaterial3d(wood),
        ));
        for (x, z) in [(-0.9, -0.25), (0.9, -0.25), (-0.9, 0.25), (0.9, 0.25)] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.025, 0.76, 0.025))),
                MeshMaterial3d(steel.clone()),
                Transform::from_xyz(x, -0.39, z),
                ChildOf(e),
            ));
        }
    }
}

#[derive(Component)]
struct Flame;

fn dress_stations(
    mut commands: Commands,
    cooks: Query<(Entity, &Cook), Added<Cook>>,
    controls: Query<(Entity, &CookControl), Added<CookControl>>,
    names: Query<&Cook>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mat = |c: Color, metal: f32, rough: f32| {
        materials.add(StandardMaterial {
            base_color: c,
            metallic: metal,
            perceptual_roughness: rough,
            ..default()
        })
    };
    let wood = mat(Color::srgb(0.45, 0.33, 0.2), 0.0, 0.8);
    let enamel = mat(Color::srgb(0.85, 0.85, 0.8), 0.0, 0.3);
    let iron = mat(Color::srgb(0.22, 0.22, 0.23), 0.8, 0.45);
    let blue = mat(Color::srgb(0.16, 0.24, 0.38), 0.0, 0.3);
    let tin = mat(Color::srgb(0.6, 0.6, 0.58), 0.9, 0.35);
    let clay = mat(Color::srgb(0.5, 0.3, 0.2), 0.0, 0.9);
    let flame_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.5, 1.0),
        emissive: LinearRgba::rgb(4.0, 8.0, 30.0),
        unlit: true,
        ..default()
    });
    for (e, cook) in &cooks {
        let (mesh, m, at): (Mesh, Handle<StandardMaterial>, Vec3) = match cook.name {
            "Board" => (
                Cuboid::new(0.26, 0.02, 0.32).into(),
                wood.clone(),
                Vec3::Y * 0.01,
            ),
            "Bowl" => (
                Cylinder::new(0.11, 0.08).into(),
                enamel.clone(),
                Vec3::Y * 0.04,
            ),
            "Pot" => (
                Cylinder::new(0.1, 0.14).into(),
                blue.clone(),
                Vec3::Y * 0.11,
            ),
            "Pan" => (
                Cylinder::new(0.12, 0.03).into(),
                iron.clone(),
                Vec3::Y * 0.055,
            ),
            "Tin oven" => (
                Cuboid::new(0.24, 0.2, 0.22).into(),
                tin.clone(),
                Vec3::Y * 0.1,
            ),
            _ => (
                Cylinder::new(0.09, 0.18).into(),
                clay.clone(),
                Vec3::Y * 0.09,
            ),
        };
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(m),
            Transform::from_translation(at),
            ChildOf(e),
        ));
        if matches!(cook.name, "Pot" | "Pan") {
            // The stove ring under it, and its flame.
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.09, 0.04))),
                MeshMaterial3d(iron.clone()),
                Transform::from_xyz(0.0, 0.02, 0.0),
                ChildOf(e),
            ));
            commands.spawn((
                Flame,
                Mesh3d(meshes.add(Torus::new(0.05, 0.07))),
                MeshMaterial3d(flame_mat.clone()),
                PointLight {
                    color: Color::srgb(0.5, 0.65, 1.0),
                    intensity: 0.0,
                    range: 2.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.045, 0.0).with_scale(Vec3::new(1.0, 0.3, 1.0)),
                Visibility::Hidden,
                ChildOf(e),
            ));
        }
    }
    for (e, control) in &controls {
        let Ok(cook) = names.get(control.0) else {
            continue;
        };
        let (mesh, m): (Mesh, _) = match cook.name {
            "Board" => (Cuboid::new(0.02, 0.015, 0.2).into(), tin.clone()),
            "Bowl" => (
                Cylinder::new(0.012, 0.22)
                    .mesh()
                    .build()
                    .rotated_by(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                tin.clone(),
            ),
            _ => (Cylinder::new(0.022, 0.025).into(), iron.clone()),
        };
        commands
            .entity(e)
            .insert((Mesh3d(meshes.add(mesh)), MeshMaterial3d(m)));
    }
}

/// A flame shows under a pot or pan while it's on.
fn flames(
    cooks: Query<(&Cook, &Children)>,
    mut flames: Query<(&mut Visibility, &mut PointLight), With<Flame>>,
    time: Res<Time>,
) {
    for (cook, children) in &cooks {
        let on = cook.station.drive() == Drive::Clock && !cook.station.is_idle();
        for c in children.iter() {
            if let Ok((mut v, mut l)) = flames.get_mut(c) {
                *v = if on {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                l.intensity = if on {
                    4_000.0 * (1.0 + 0.1 * (time.elapsed_secs() * 17.0).sin())
                } else {
                    0.0
                };
            }
        }
    }
}
