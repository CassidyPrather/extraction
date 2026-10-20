//! What the driver has fitted to the truck (`progression`), as blockouts
//! until they're modelled: the rack over the stash, the shelf beside it,
//! the vice on the bench, the whip aerial on the cab, the medicine cabinet.

use bevy::prelude::*;
use extraction_core::progression::Upgrade;

use crate::progression::TruckUpgradeProp;
use crate::render::materials::{Mat, MaterialLibrary, metric_box};
use crate::render::world_props::{Blockout, SwapIn};

pub struct UpgradeVisualsPlugin;

impl Plugin for UpgradeVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, dress_upgrades);
    }
}

fn paint(
    materials: &mut Assets<StandardMaterial>,
    c: Color,
    rough: f32,
    metal: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        perceptual_roughness: rough,
        metallic: metal,
        ..default()
    })
}

fn dress_upgrades(
    mut commands: Commands,
    props: Query<(Entity, &TruckUpgradeProp), Added<TruckUpgradeProp>>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<MaterialLibrary>,
) {
    for (e, prop) in &props {
        let mut parts: Vec<(Vec3, Vec3, Handle<StandardMaterial>)> = Vec::new();
        match prop.0 {
            Upgrade::StashRack => {
                // Along the wall side of the crate, so its top stays open:
                // four posts, a slatted shelf, a brace at the back.
                let timber = lib.get(Mat::Timber, &server, &mut materials);
                for x in [-0.43, -0.07] {
                    for z in [-0.52, 0.52] {
                        parts.push((
                            Vec3::new(0.04, 0.5, 0.04),
                            Vec3::new(x, 0.55, z),
                            timber.clone(),
                        ));
                    }
                }
                for i in 0..4 {
                    parts.push((
                        Vec3::new(0.08, 0.025, 1.12),
                        Vec3::new(-0.41 + 0.105 * i as f32, 0.79, 0.0),
                        timber.clone(),
                    ));
                }
                parts.push((
                    Vec3::new(0.02, 0.08, 1.08),
                    Vec3::new(-0.45, 0.65, 0.0),
                    timber,
                ));
            }
            Upgrade::StashRack2 => {
                let timber = lib.get(Mat::Timber, &server, &mut materials);
                let iron = lib.get(Mat::DarkMetal, &server, &mut materials);
                parts.push((Vec3::new(0.3, 0.025, 1.0), Vec3::ZERO, timber));
                for z in [-0.4, 0.4] {
                    parts.push((
                        Vec3::new(0.02, 0.2, 0.03),
                        Vec3::new(-0.14, -0.11, z),
                        iron.clone(),
                    ));
                    parts.push((
                        Vec3::new(0.24, 0.02, 0.03),
                        Vec3::new(-0.02, -0.03, z),
                        iron.clone(),
                    ));
                }
            }
            Upgrade::BenchVice => {
                let green = paint(&mut materials, Color::srgb(0.16, 0.24, 0.18), 0.6, 0.6);
                let steel = lib.get(Mat::GunMetal, &server, &mut materials);
                // Body, fixed jaw, sliding jaw, screw, handle bar.
                parts.push((
                    Vec3::new(0.12, 0.07, 0.2),
                    Vec3::new(0.0, 0.0, 0.0),
                    green.clone(),
                ));
                parts.push((
                    Vec3::new(0.14, 0.08, 0.04),
                    Vec3::new(0.0, 0.07, 0.08),
                    green.clone(),
                ));
                parts.push((
                    Vec3::new(0.14, 0.08, 0.04),
                    Vec3::new(0.0, 0.07, -0.02),
                    green,
                ));
                parts.push((
                    Vec3::new(0.025, 0.025, 0.16),
                    Vec3::new(0.0, 0.05, -0.12),
                    steel.clone(),
                ));
                parts.push((
                    Vec3::new(0.2, 0.015, 0.015),
                    Vec3::new(0.0, 0.05, -0.2),
                    steel,
                ));
            }
            Upgrade::Aerial => {
                let iron = lib.get(Mat::DarkMetal, &server, &mut materials);
                let whip = paint(&mut materials, Color::srgb(0.55, 0.56, 0.55), 0.35, 0.9);
                parts.push((Vec3::new(0.08, 0.08, 0.08), Vec3::new(0.0, 0.04, 0.0), iron));
                parts.push((
                    Vec3::new(0.008, 2.4, 0.008),
                    Vec3::new(0.0, 1.28, 0.0),
                    whip,
                ));
            }
            Upgrade::MedCabinet => {
                let enamel = paint(&mut materials, Color::srgb(0.86, 0.86, 0.82), 0.45, 0.0);
                let red = paint(&mut materials, Color::srgb(0.7, 0.08, 0.06), 0.6, 0.0);
                // Back, sides, top, the shelf (its collider is logic's), and
                // the open door swung to the right with its cross.
                parts.push((
                    Vec3::new(0.4, 0.48, 0.02),
                    Vec3::new(0.0, 0.0, -0.06),
                    enamel.clone(),
                ));
                for x in [-0.19, 0.19] {
                    parts.push((
                        Vec3::new(0.02, 0.48, 0.14),
                        Vec3::new(x, 0.0, 0.0),
                        enamel.clone(),
                    ));
                }
                parts.push((
                    Vec3::new(0.4, 0.02, 0.14),
                    Vec3::new(0.0, 0.23, 0.0),
                    enamel.clone(),
                ));
                parts.push((
                    Vec3::new(0.4, 0.02, 0.14),
                    Vec3::new(0.0, -0.22, 0.0),
                    enamel.clone(),
                ));
                parts.push((
                    Vec3::new(0.02, 0.46, 0.38),
                    Vec3::new(0.21, 0.0, 0.26),
                    enamel,
                ));
                parts.push((
                    Vec3::new(0.006, 0.18, 0.05),
                    Vec3::new(0.223, 0.04, 0.26),
                    red.clone(),
                ));
                parts.push((
                    Vec3::new(0.006, 0.05, 0.18),
                    Vec3::new(0.223, 0.04, 0.26),
                    red,
                ));
            }
        }
        // The modelled fitting replaces the boxes once it's loaded
        // (`art/props/truck_kit.py`); the boxes stay if it's missing.
        commands.entity(e).insert(SwapIn::one(prop.0.id()));
        commands.entity(e).with_children(|c| {
            for (size, at, mat) in parts {
                c.spawn((
                    Mesh3d(meshes.add(metric_box(size))),
                    MeshMaterial3d(mat),
                    Transform::from_translation(at),
                    Blockout,
                ));
            }
        });
    }
}
