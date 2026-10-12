//! The offering places: a stone well with black water, and a ring of
//! standing stones around a flat altar. A cold glow when they take
//! something.

use bevy::prelude::*;

use super::materials::{Mat, MaterialLibrary, metric_box};
use crate::anomaly::offering::{Boon, Offering, OfferingTaken};

pub struct OfferingVisualsPlugin;

impl Plugin for OfferingVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress_offerings, glow));
    }
}

#[derive(Component)]
struct Glow(f32);

fn dress_offerings(
    mut commands: Commands,
    sites: Query<(Entity, &Offering, &Transform), Added<Offering>>,
    zone: Res<crate::world::Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for (e, site, st) in &sites {
        let origin = st.translation;
        let stone = library.get(Mat::Concrete, &server, &mut materials);
        let wood = library.get(Mat::Planks, &server, &mut materials);
        let black = materials.add(StandardMaterial {
            base_color: Color::srgb(0.01, 0.012, 0.015),
            perceptual_roughness: 0.02,
            reflectance: 0.9,
            ..default()
        });
        let mut part = |mesh: Mesh, m: &Handle<StandardMaterial>, t: Transform| {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(m.clone()),
                t,
                ChildOf(e),
            ));
        };
        if site.name.contains("well") {
            // Twelve blocks in a ring, a bucket frame over the top.
            for i in 0..12 {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                part(
                    metric_box(Vec3::new(0.42, 0.65, 0.22)),
                    &stone,
                    Transform::from_xyz(a.cos() * 0.68, -0.38, a.sin() * 0.68)
                        .with_rotation(Quat::from_rotation_y(-a + std::f32::consts::FRAC_PI_2)),
                );
            }
            part(
                Circle::new(0.6)
                    .mesh()
                    .build()
                    .rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                &black,
                Transform::from_xyz(0.0, -0.45, 0.0),
            );
            for x in [-0.8, 0.8] {
                part(
                    metric_box(Vec3::new(0.08, 1.5, 0.08)),
                    &wood,
                    Transform::from_xyz(x, 0.05, 0.0),
                );
            }
            part(
                metric_box(Vec3::new(1.7, 0.08, 0.08)),
                &wood,
                Transform::from_xyz(0.0, 0.8, 0.0),
            );
        } else {
            // Standing stones round a flat altar (the altar takes the gifts).
            part(
                metric_box(Vec3::new(1.1, 0.5, 0.8)),
                &stone,
                Transform::from_xyz(0.0, -0.45, 0.0),
            );
            for i in 0..7 {
                let a = i as f32 / 7.0 * std::f32::consts::TAU + 0.3;
                let h = 1.6 + (i as f32 * 1.7).sin() * 0.5;
                let (x, z) = (a.cos() * 3.2, a.sin() * 3.2);
                // Each stone stands on its own patch of hillside, sunk a little.
                let ground = zone.ground(origin.x + x, origin.z + z) - origin.y;
                part(
                    metric_box(Vec3::new(0.55, h, 0.35)),
                    &stone,
                    Transform::from_xyz(x, ground + h * 0.5 - 0.3, z).with_rotation(
                        Quat::from_rotation_y(-a) * Quat::from_rotation_z(0.05 * (i as f32).sin()),
                    ),
                );
            }
        }
        commands.spawn((
            Glow(0.0),
            PointLight {
                color: Color::srgb(0.6, 0.8, 1.0),
                intensity: 0.0,
                range: 8.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, 0.3, 0.0),
            ChildOf(e),
        ));
    }
}

fn glow(
    mut taken: MessageReader<OfferingTaken>,
    sites: Query<(&GlobalTransform, &Children), With<Offering>>,
    mut lights: Query<(&mut Glow, &mut PointLight)>,
    time: Res<Time>,
) {
    for t in taken.read() {
        for (gt, children) in &sites {
            if gt.translation().distance(t.at) > 0.5 {
                continue;
            }
            for c in children.iter() {
                if let Ok((mut g, _)) = lights.get_mut(c) {
                    g.0 = if t.boon == Boon::Nothing { 0.3 } else { 1.0 };
                }
            }
        }
    }
    for (mut g, mut l) in &mut lights {
        g.0 = (g.0 - time.delta_secs() * 0.25).max(0.0);
        l.intensity = 80_000.0 * g.0 * g.0;
    }
}
