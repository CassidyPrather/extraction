//! What gadgets look like working: torch beams, the laser's line and dot,
//! the compass needle, the paper map, EMP flashes, and optics magnifying
//! the view (flat mode: the camera zooms while the optic's eyepiece is on
//! your eye line; in a headset that needs a picture-in-picture lens,
//! which is still to do).

use avian3d::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::{Device, DeviceKind, EmpBlast, Optic, Powered};
use crate::items::{Catalog, Item};
use crate::world::Layer;

pub struct GadgetVisualsPlugin;

impl Plugin for GadgetVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                beams,
                lasers,
                dress_gadgets,
                compass_needles,
                emp_flashes,
                optic_zoom,
            ),
        );
    }
}

#[derive(Component)]
struct Beam;

/// Powered lights get a spotlight, pointing down the item's -Z.
fn beams(
    mut commands: Commands,
    devices: Query<(Entity, &Device, Has<Powered>, Option<&Children>)>,
    beams: Query<Entity, With<Beam>>,
) {
    for (e, dev, powered, children) in &devices {
        if dev.kind != DeviceKind::Light {
            continue;
        }
        let existing = children.and_then(|c| c.iter().find(|c| beams.contains(*c)));
        match (powered, existing) {
            (true, None) => {
                commands.entity(e).with_child((
                    Beam,
                    SpotLight {
                        color: Color::srgb(1.0, 0.95, 0.85),
                        intensity: 250_000.0,
                        range: 45.0,
                        radius: 0.01,
                        inner_angle: 0.12,
                        outer_angle: 0.38,
                        shadow_maps_enabled: true,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, -0.06),
                ));
            }
            (false, Some(b)) => {
                commands.entity(b).despawn();
            }
            _ => {}
        }
    }
}

#[derive(Component)]
struct LaserBeam;

#[derive(Component)]
struct LaserDot;

/// A thin red line to the first thing in the way, and a dot on it.
fn lasers(
    mut commands: Commands,
    devices: Query<(Entity, &Device, Has<Powered>, &GlobalTransform)>,
    mut parts: Query<(&mut Transform, &mut Visibility), Or<(With<LaserBeam>, With<LaserDot>)>>,
    spatial: SpatialQuery,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<std::collections::HashMap<Entity, (Entity, Entity)>>,
    mut assets: Local<Option<(Handle<Mesh>, Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    let (line, dot, red) = assets
        .get_or_insert_with(|| {
            (
                meshes.add(Cylinder::new(0.0012, 1.0)),
                meshes.add(Sphere::new(0.012)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.1, 0.05),
                    emissive: LinearRgba::rgb(60.0, 2.0, 1.0),
                    unlit: true,
                    ..default()
                }),
            )
        })
        .clone();
    made.retain(|dev, (b, d)| {
        if devices.contains(*dev) {
            true
        } else {
            commands.entity(*b).despawn();
            commands.entity(*d).despawn();
            false
        }
    });
    for (e, dev, powered, gt) in &devices {
        if dev.kind != DeviceKind::Laser {
            continue;
        }
        let (b, d) = *made.entry(e).or_insert_with(|| {
            let b = commands
                .spawn((
                    LaserBeam,
                    Mesh3d(line.clone()),
                    MeshMaterial3d(red.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    NotShadowCaster,
                ))
                .id();
            let d = commands
                .spawn((
                    LaserDot,
                    Mesh3d(dot.clone()),
                    MeshMaterial3d(red.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    NotShadowCaster,
                ))
                .id();
            (b, d)
        });
        let origin = gt.transform_point(Vec3::new(0.0, 0.0, -0.035));
        let dir = Dir3::new(gt.rotation() * Vec3::NEG_Z).unwrap_or(Dir3::NEG_Z);
        let hit = spatial.cast_ray(
            origin,
            dir,
            150.0,
            true,
            &SpatialQueryFilter::from_mask([Layer::World, Layer::Robot, Layer::Player]),
        );
        let len = hit.map_or(150.0, |h| h.distance);
        if let Ok((mut t, mut v)) = parts.get_mut(b) {
            *v = if powered {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            *t = Transform::from_translation(origin + *dir * len * 0.5)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, *dir))
                .with_scale(Vec3::new(1.0, len, 1.0));
        }
        if let Ok((mut t, mut v)) = parts.get_mut(d) {
            *v = if powered && hit.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            *t = Transform::from_translation(origin + *dir * (len - 0.005));
        }
    }
}

#[derive(Component)]
struct Needle;

/// The compass gets a face and a needle; the map case shows the map.
fn dress_gadgets(
    mut commands: Commands,
    items: Query<(Entity, &Item), Added<Item>>,
    catalog: Res<Catalog>,
    opts: Res<crate::launch::LaunchOptions>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, item) in &items {
        match catalog.get(item.kind()).id.as_str() {
            "compass" => {
                let face = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.85, 0.82, 0.72),
                    perceptual_roughness: 0.6,
                    ..default()
                });
                let red = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.75, 0.08, 0.05),
                    emissive: LinearRgba::rgb(0.6, 0.05, 0.02),
                    ..default()
                });
                commands.entity(e).with_children(|c| {
                    c.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.024, 0.002))),
                        MeshMaterial3d(face),
                        Transform::from_xyz(0.0, 0.016, 0.0),
                    ));
                    c.spawn((
                        Needle,
                        Mesh3d(meshes.add(Cuboid::new(0.004, 0.002, 0.038))),
                        MeshMaterial3d(red),
                        Transform::from_xyz(0.0, 0.019, 0.0),
                    ));
                });
            }
            "map_case" => {
                let tex: Handle<Image> =
                    server.load(format!("textures/ui/{}_field_map.jpg", opts.map));
                let paper = materials.add(StandardMaterial {
                    base_color_texture: Some(tex),
                    perceptual_roughness: 0.9,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                });
                // A sheet in a sleeve, about A4, face up.
                commands.entity(e).with_child((
                    Mesh3d(
                        meshes.add(
                            Rectangle::new(0.28, 0.28)
                                .mesh()
                                .build()
                                .rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                        ),
                    ),
                    MeshMaterial3d(paper),
                    Transform::from_xyz(0.0, 0.006, 0.0),
                ));
            }
            _ => {}
        }
    }
}

/// North is -Z.
fn compass_needles(
    mut needles: Query<(&mut Transform, &ChildOf), With<Needle>>,
    parents: Query<&GlobalTransform>,
) {
    for (mut t, parent) in &mut needles {
        let Ok(gt) = parents.get(parent.parent()) else {
            continue;
        };
        let inv = gt.rotation().inverse();
        let north = inv * Vec3::NEG_Z;
        let flat = Vec3::new(north.x, 0.0, north.z);
        if flat.length() > 0.05 {
            t.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, flat.normalize());
        }
    }
}

#[derive(Component)]
struct EmpFlash(f32);

fn emp_flashes(
    mut commands: Commands,
    mut blasts: MessageReader<EmpBlast>,
    mut flashes: Query<(
        Entity,
        &mut EmpFlash,
        &mut PointLight,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for b in blasts.read() {
        commands.spawn((
            EmpFlash(1.0),
            PointLight {
                color: Color::srgb(0.5, 0.75, 1.0),
                intensity: 3_000_000.0,
                range: b.radius * 3.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.5, 0.75, 1.0, 0.25),
                emissive: LinearRgba::rgb(2.0, 4.0, 8.0),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(b.at).with_scale(Vec3::splat(0.5)),
            NotShadowCaster,
        ));
    }
    let dt = time.delta_secs();
    for (e, mut f, mut light, mut t, mat) in &mut flashes {
        f.0 -= dt * 2.0;
        if f.0 <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        light.intensity = 3_000_000.0 * f.0 * f.0;
        t.scale = Vec3::splat(super::EMP_RADIUS * (1.0 - f.0 * f.0) + 0.5);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.emissive = LinearRgba::rgb(2.0, 4.0, 8.0) * f.0;
        }
    }
}

/// Flat mode: while an optic's eyepiece sits on the eye line just in front
/// of the camera, narrow the field of view by its magnification.
fn optic_zoom(
    optics: Query<(&Optic, &GlobalTransform)>,
    mut cams: Query<(&GlobalTransform, &mut Projection), With<Camera3d>>,
    driver: Res<crate::rig::RigDriver>,
    time: Res<Time>,
) {
    if *driver == crate::rig::RigDriver::Xr {
        return;
    }
    for (cam, mut proj) in &mut cams {
        let Projection::Perspective(p) = &mut *proj else {
            continue;
        };
        let eye = cam.translation();
        let fwd = cam.forward();
        let mut mag: f32 = 1.0;
        for (o, gt) in &optics {
            let piece = gt.transform_point(o.eye);
            let rel = piece - eye;
            let along = rel.dot(*fwd);
            let off = (rel - *fwd * along).length();
            // On the eye line, in front, within arm's length, pointing the
            // same way as the view.
            let aligned = (gt.rotation() * Vec3::NEG_Z).dot(*fwd) > 0.995;
            if along > -0.05 && along < 0.45 && off < 0.025 && aligned {
                mag = mag.max(o.magnification);
            }
        }
        let want = 75f32.to_radians() / mag;
        p.fov += (want - p.fov) * (time.delta_secs() * 12.0).min(1.0);
    }
}
