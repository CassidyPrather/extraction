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
            // Standing stones round a flat altar (the altar takes the gifts):
            // rough stones, the field's (`world::rocks`). Boxes of the
            // concrete scan read as a sawn slab at arm's length: the walk's
            // crest block (docs/craft/PROP_READING.md).
            use crate::world::rocks::stone_mesh;
            use extraction_core::rocks::{ROCK_VARIANTS, RockSpot};
            let burnt = zone.map.ash > 0.0;
            let rough = library.get(Mat::Boulder, &server, &mut materials);
            // A stone at (x, z) of half-height `hh`, its flat base
            // (`rock_points` planes it at 0.55 under its middle) 15 cm into
            // the ground at `ground` (in the site's space).
            let stone = |x: f32,
                         z: f32,
                         ground: f32,
                         hh: f32,
                         radius: f32,
                         stretch: f32,
                         yaw: f32,
                         v: usize| {
                stone_mesh(
                    &RockSpot {
                        pos: [x, ground + 0.55 * hh - 0.15, z],
                        radius,
                        squash: hh / radius,
                        stretch,
                        yaw,
                        variant: v % ROCK_VARIANTS,
                    },
                    burnt,
                )
            };
            // The altar: a low slab, its top where the box's was.
            part(
                stone(0.0, 0.0, -0.7, 0.4, 0.5, 1.2, 0.4, 5),
                &rough,
                Transform::IDENTITY,
            );
            for i in 0..7 {
                let a = i as f32 / 7.0 * std::f32::consts::TAU + 0.3;
                let h = 1.6 + (i as f32 * 1.7).sin() * 0.5;
                let (x, z) = (a.cos() * 3.2, a.sin() * 3.2);
                // Each stone stands on its own patch of hillside, broad
                // face to the altar.
                let ground = zone.ground(origin.x + x, origin.z + z) - origin.y;
                part(
                    stone(
                        x,
                        z,
                        ground,
                        h / 1.55,
                        0.26,
                        1.35,
                        -a + std::f32::consts::FRAC_PI_2,
                        i,
                    ),
                    &rough,
                    Transform::IDENTITY,
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
