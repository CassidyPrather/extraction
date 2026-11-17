//! What the arcade adds to see and hear: each gate's lamp on its post
//! (dark, or burning red from a wave's call until it's cleared),
//! the siren at that gate as the wave is called, and the range's frames,
//! plates and firing line.

use bevy::prelude::*;

use super::range::{BAR, CHAIN, PLATE, RangeFrame, RangeLine, RangePlate, THICK};
use super::waves::{GateLamp, Siren};
use crate::launch::LaunchOptions;
use crate::render::materials::{Mat, MaterialLibrary, metric_box};

/// A lamp's head and light, and its two looks.
#[derive(Component)]
struct LampParts {
    head: Entity,
    light: Entity,
    dark: Handle<StandardMaterial>,
    lit: Handle<StandardMaterial>,
}

/// The lamp's height over the ground, m.
pub const LAMP_HEIGHT: f32 = 3.4;
/// Its light, lit (lumens): seen across the ground by day.
const LAMP_LUMENS: f32 = 120_000.0;

pub struct ArcadeVisualsPlugin;

impl Plugin for ArcadeVisualsPlugin {
    fn build(&self, app: &mut App) {
        let Some(opts) = app.world().get_resource::<LaunchOptions>().cloned() else {
            return;
        };
        if !super::active(&opts) {
            return;
        }
        app.add_systems(Update, (build_lamps, show_lamps).chain())
            .add_systems(Update, build_range)
            .add_systems(
                Update,
                siren.run_if(
                    resource_exists::<crate::audio::SoundBank>
                        .and_then(resource_exists::<crate::audio::AudioRng>),
                ),
            );
    }
}

fn build_lamps(
    mut commands: Commands,
    lamps: Query<Entity, Added<GateLamp>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for e in &lamps {
        let timber = lib.get(Mat::Timber, &server, &mut materials);
        commands.spawn((
            Mesh3d(meshes.add(metric_box(Vec3::new(0.14, LAMP_HEIGHT, 0.14)))),
            MeshMaterial3d(timber),
            Transform::from_xyz(0.0, LAMP_HEIGHT * 0.5, 0.0),
            ChildOf(e),
        ));
        let dark = materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.1, 0.1),
            perceptual_roughness: 0.4,
            ..default()
        });
        let lit = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.2, 0.1),
            emissive: LinearRgba::rgb(40.0, 4.0, 1.5),
            ..default()
        });
        let head = commands
            .spawn((
                Mesh3d(meshes.add(Sphere::new(0.28).mesh().uv(16, 10))),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(0.0, LAMP_HEIGHT + 0.2, 0.0),
                ChildOf(e),
            ))
            .id();
        let light = commands
            .spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.25, 0.12),
                    intensity: 0.0,
                    range: 30.0,
                    ..default()
                },
                Transform::from_xyz(0.0, LAMP_HEIGHT + 0.2, 0.0),
                ChildOf(e),
            ))
            .id();
        commands.entity(e).insert(LampParts {
            head,
            light,
            dark,
            lit,
        });
    }
}

fn show_lamps(
    lamps: Query<(&GateLamp, &LampParts), Or<(Changed<GateLamp>, Added<LampParts>)>>,
    mut heads: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut lights: Query<&mut PointLight>,
) {
    for (l, parts) in &lamps {
        if let Ok(mut m) = heads.get_mut(parts.head) {
            m.0 = if l.lit {
                parts.lit.clone()
            } else {
                parts.dark.clone()
            };
        }
        if let Ok(mut p) = lights.get_mut(parts.light) {
            p.intensity = if l.lit { LAMP_LUMENS } else { 0.0 };
        }
    }
}

/// The range's frames (two posts and a bar), its plates (painted steel,
/// on two chains) and the beam laid at the firing line.
fn build_range(
    mut commands: Commands,
    frames: Query<Entity, Added<RangeFrame>>,
    plates: Query<Entity, Added<RangePlate>>,
    lines: Query<Entity, Added<RangeLine>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    let timber = lib.get(Mat::Timber, &server, &mut materials);
    let iron = lib.get(Mat::DarkMetal, &server, &mut materials);
    // Frames stand across the lane: local X is across, -Z downrange.
    let span = PLATE.x * 0.5 + 0.45;
    for e in &frames {
        for side in [-1.0, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(metric_box(Vec3::new(0.1, BAR + 0.1, 0.1)))),
                MeshMaterial3d(timber.clone()),
                Transform::from_xyz(side * span, (BAR + 0.1) * 0.5, 0.0),
                ChildOf(e),
            ));
        }
        commands.spawn((
            Mesh3d(meshes.add(metric_box(Vec3::new(span * 2.0 + 0.1, 0.1, 0.1)))),
            MeshMaterial3d(timber.clone()),
            Transform::from_xyz(0.0, BAR + 0.05, 0.0),
            ChildOf(e),
        ));
    }
    for e in &plates {
        let paint = materials.add(StandardMaterial {
            base_color: Color::srgb(0.82, 0.8, 0.74),
            metallic: 0.3,
            perceptual_roughness: 0.55,
            ..default()
        });
        commands.entity(e).insert((
            Mesh3d(meshes.add(metric_box(Vec3::new(PLATE.x, PLATE.y, THICK)))),
            MeshMaterial3d(paint),
        ));
        for side in [-1.0, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(metric_box(Vec3::new(0.012, CHAIN, 0.012)))),
                MeshMaterial3d(iron.clone()),
                Transform::from_xyz(side * PLATE.x * 0.33, PLATE.y * 0.5 + CHAIN * 0.5, 0.0),
                ChildOf(e),
            ));
        }
    }
    for e in &lines {
        commands.spawn((
            Mesh3d(meshes.add(metric_box(Vec3::new(2.4, 0.12, 0.16)))),
            MeshMaterial3d(timber.clone()),
            Transform::from_xyz(0.0, 0.06, 0.0),
            ChildOf(e),
        ));
        commands.spawn((
            Mesh3d(meshes.add(metric_box(Vec3::new(0.08, 1.3, 0.08)))),
            MeshMaterial3d(timber.clone()),
            Transform::from_xyz(1.5, 0.65, 0.0),
            ChildOf(e),
        ));
    }
}

/// The machines' own siren (the spotter's call), from the gate.
fn siren(
    mut commands: Commands,
    mut sirens: MessageReader<Siren>,
    bank: Res<crate::audio::SoundBank>,
    mut rng: ResMut<crate::audio::AudioRng>,
) {
    for s in sirens.read() {
        crate::audio::play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/siren",
            Some(s.at),
            "spotter.call",
            0.0,
            0.02,
        );
    }
}
