//! Logistics visuals: the cargo pod and the cable it hangs on, the
//! grappler's harpoon launcher, and the line from a grappler to whoever it
//! has speared.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::robots::dirigible::{CargoPod, Dirigible, RADIUS};
use crate::robots::grappler::{Grappler, Tethered};

pub struct LogisticsVisualsPlugin;

impl Plugin for LogisticsVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress_pods, dress_grapplers, cables, tether_lines));
    }
}

fn mat(
    materials: &mut Assets<StandardMaterial>,
    c: Color,
    metal: f32,
    rough: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        metallic: metal,
        perceptual_roughness: rough,
        ..default()
    })
}

fn dress_pods(
    mut commands: Commands,
    pods: Query<Entity, Added<CargoPod>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in &pods {
        let shell = mat(&mut materials, Color::srgb(0.84, 0.85, 0.86), 0.0, 0.35);
        let dark = mat(&mut materials, Color::srgb(0.08, 0.08, 0.09), 0.7, 0.4);
        let glow = materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.4, 0.5),
            emissive: LinearRgba::rgb(0.5, 3.0, 4.0),
            ..default()
        });
        commands.entity(e).with_children(|c| {
            // An open-topped tub so you can see (and reach) what's in it.
            for (size, at, m) in [
                (
                    Vec3::new(1.3, 0.06, 1.8),
                    Vec3::new(0.0, -0.45, 0.0),
                    &shell,
                ),
                (
                    Vec3::new(0.06, 0.9, 1.8),
                    Vec3::new(-0.65, 0.0, 0.0),
                    &shell,
                ),
                (Vec3::new(0.06, 0.9, 1.8), Vec3::new(0.65, 0.0, 0.0), &shell),
                (Vec3::new(1.3, 0.9, 0.06), Vec3::new(0.0, 0.0, -0.9), &shell),
                (Vec3::new(1.3, 0.9, 0.06), Vec3::new(0.0, 0.0, 0.9), &shell),
                (
                    Vec3::new(1.36, 0.08, 0.08),
                    Vec3::new(0.0, 0.44, -0.9),
                    &dark,
                ),
                (
                    Vec3::new(1.36, 0.08, 0.08),
                    Vec3::new(0.0, 0.44, 0.9),
                    &dark,
                ),
                (
                    Vec3::new(1.32, 0.03, 0.06),
                    Vec3::new(0.0, 0.25, 0.92),
                    &glow,
                ),
            ] {
                c.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(size))),
                    MeshMaterial3d(m.clone()),
                    Transform::from_translation(at),
                ));
            }
        });
    }
}

#[derive(Component)]
struct Cable;

fn cables(
    mut commands: Commands,
    ships: Query<(Entity, &Dirigible, &GlobalTransform)>,
    mut cables: Query<(&Cable, &mut Transform, &mut Visibility, &ChildOf)>,
    mut made: Local<Vec<(Entity, Entity)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    made.retain(|(ship, cable)| {
        if ships.contains(*ship) {
            true
        } else {
            commands.entity(*cable).try_despawn();
            false
        }
    });
    for (e, d, _) in &ships {
        if !made.iter().any(|(s, _)| *s == e) {
            let m = mat(&mut materials, Color::srgb(0.05, 0.05, 0.05), 0.8, 0.5);
            let c = commands
                .spawn((
                    Cable,
                    Mesh3d(meshes.add(Cylinder::new(0.025, 1.0))),
                    MeshMaterial3d(m),
                    Transform::default(),
                    Visibility::Hidden,
                    NotShadowCaster,
                    ChildOf(e),
                ))
                .id();
            made.push((e, c));
        }
        let _ = d;
    }
    for (_, mut t, mut v, parent) in &mut cables {
        let Ok((_, d, _)) = ships.get(parent.parent()) else {
            continue;
        };
        let len = d.cable.max(0.0);
        *v = if len > 0.05 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let top = -RADIUS - 1.1;
        *t = Transform::from_xyz(0.0, top - len * 0.5, 0.0).with_scale(Vec3::new(
            1.0,
            len.max(0.01),
            1.0,
        ));
    }
}

fn dress_grapplers(
    mut commands: Commands,
    grapplers: Query<Entity, Added<Grappler>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in &grapplers {
        let dark = mat(&mut materials, Color::srgb(0.08, 0.08, 0.09), 0.7, 0.4);
        let warn = materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.6, 0.1),
            emissive: LinearRgba::rgb(3.0, 1.6, 0.2),
            ..default()
        });
        // A stubby launcher on top with a reel, and a yellow band.
        commands.entity(e).with_children(|c| {
            c.spawn((
                Mesh3d(
                    meshes.add(
                        Cylinder::new(0.06, 0.5)
                            .mesh()
                            .build()
                            .rotated_by(Quat::from_rotation_x(1.5708)),
                    ),
                ),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(0.0, 0.48, -0.1),
            ));
            c.spawn((
                Mesh3d(
                    meshes.add(
                        Cylinder::new(0.13, 0.1)
                            .mesh()
                            .build()
                            .rotated_by(Quat::from_rotation_z(1.5708)),
                    ),
                ),
                MeshMaterial3d(dark),
                Transform::from_xyz(0.0, 0.44, 0.18),
            ));
            c.spawn((
                Mesh3d(meshes.add(Torus::new(0.38, 0.41))),
                MeshMaterial3d(warn),
                Transform::from_xyz(0.0, 0.05, 0.0),
            ));
        });
    }
}

#[derive(Component)]
struct TetherLine(Entity);

fn tether_lines(
    mut commands: Commands,
    tethered: Query<(Entity, &Tethered, &GlobalTransform)>,
    robots: Query<&GlobalTransform>,
    mut lines: Query<(Entity, &TetherLine, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, line, mut t) in &mut lines {
        let Ok((_, teth, pgt)) = tethered.get(line.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let Ok(rgt) = robots.get(teth.by) else {
            continue;
        };
        let a = rgt.translation() + Vec3::Y * 0.5;
        let b = pgt.translation() + Vec3::Y * 1.1;
        let len = a.distance(b).max(0.01);
        *t = Transform::from_translation((a + b) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a) / len))
            .with_scale(Vec3::new(1.0, len, 1.0));
    }
    for (p, _, _) in &tethered {
        if lines.iter().any(|(_, l, _)| l.0 == p) {
            continue;
        }
        let m = mat(&mut materials, Color::srgb(0.9, 0.6, 0.1), 0.3, 0.6);
        commands.spawn((
            TetherLine(p),
            Mesh3d(meshes.add(Cylinder::new(0.008, 1.0))),
            MeshMaterial3d(m),
            Transform::default(),
            NotShadowCaster,
        ));
    }
}
