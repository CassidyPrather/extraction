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
                add_lenses,
                lenses,
                dress_cover,
                cloak_bubbles,
                add_drone_feeds,
                add_wrist_feed,
                drone_feeds,
                dress_ropes,
                mark_caches,
                mark_job,
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
                let steel = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.25, 0.27, 0.3),
                    perceptual_roughness: 0.5,
                    metallic: 0.6,
                    ..default()
                });
                commands.entity(e).with_children(|c| {
                    c.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.024, 0.002))),
                        MeshMaterial3d(face),
                        Transform::from_xyz(0.0, 0.016, 0.0),
                    ));
                    // Red to the north, dark steel to the south: a needle red
                    // end to end didn't say which end was north
                    // (`docs/LEGIBILITY.md`, wayfinding).
                    let half = meshes.add(Cuboid::new(0.004, 0.002, 0.019));
                    c.spawn((
                        Needle,
                        Transform::from_xyz(0.0, 0.019, 0.0),
                        Visibility::default(),
                    ))
                    .with_children(|n| {
                        n.spawn((
                            Mesh3d(half.clone()),
                            MeshMaterial3d(red),
                            Transform::from_xyz(0.0, 0.0, -0.0095),
                        ));
                        n.spawn((
                            Mesh3d(half),
                            MeshMaterial3d(steel),
                            Transform::from_xyz(0.0, 0.0, 0.0095),
                        ));
                    });
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
                    MapSheet,
                ));
            }
            _ => {}
        }
    }
}

/// A hung rope's radius: thick enough to pick out from a few steps (a
/// degree across at 1.5 m; `render::affordance`).
pub const ROPE_R: f32 = 0.016;

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
    mut cams: Query<
        (&GlobalTransform, &mut Projection),
        (With<Camera3d>, Without<crate::render::OffscreenCamera>),
    >,
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

/// A picture-in-picture lens: a narrow camera at the objective renders
/// into a texture shown on the eyepiece, only while an eye is right at it.
/// This is how magnification works in a headset (zooming the whole view
/// would make people sick); flat mode zooms the camera instead.
#[derive(Component)]
struct Lens {
    camera: Entity,
    glass: Handle<StandardMaterial>,
    image: Handle<Image>,
    reticle: Entity,
    /// Scopes have a reticle; binoculars don't.
    scope: bool,
}

/// A PU-style reticle: a post from below with a pointed tip at the centre,
/// and a thin horizontal line either side.
fn reticle_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let n = 256u32;
    let mut data = vec![0u8; (n * n * 4) as usize];
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            // Post: from the bottom edge up to the centre, tapering to a tip.
            let half = if fy > 8.0 {
                3.0
            } else if fy > 0.0 {
                3.0 * fy / 8.0
            } else {
                -1.0
            };
            let post = fx.abs() <= half;
            let line = fy.abs() <= 0.8 && fx.abs() > 18.0;
            if post || line {
                let i = ((y * n + x) * 4) as usize;
                data[i + 3] = 255;
            }
        }
    }
    images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    ))
}

/// Apparent size of the lens at the eye, degrees.
const LENS_ANGLE: f32 = 26.0;

fn add_lenses(
    mut commands: Commands,
    optics: Query<(Entity, &Optic, Has<crate::weapons::zero::Rangefinder>), Added<Optic>>,
    weather: Res<crate::world::sky::Weather>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
    for (e, o, rangefinder) in &optics {
        let mut image = Image::new_fill(
            Extent3d {
                width: 384,
                height: 384,
                ..default()
            },
            TextureDimension::D2,
            // One Rgba16Float pixel: eight bytes.
            &[0u8; 8],
            TextureFormat::Rgba16Float,
            bevy::asset::RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        let handle = images.add(image);
        let objective = Vec3::new(o.eye.x, o.eye.y, -o.eye.z.abs());
        let camera = commands
            .spawn((
                crate::render::OffscreenCamera,
                Camera3d::default(),
                Camera {
                    order: -5,
                    is_active: false,
                    ..default()
                },
                bevy::camera::RenderTarget::Image(handle.clone().into()),
                bevy::camera::Hdr,
                Projection::Perspective(PerspectiveProjection {
                    fov: (LENS_ANGLE / o.magnification).to_radians(),
                    near: 0.05,
                    far: 2000.0,
                    ..default()
                }),
                // Linear HDR into the texture: the player's own camera
                // tonemaps it once, along with everything else.
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                bevy::camera::Exposure { ev100: 10.6 },
                weather.fog(),
                Msaa::Off,
                Transform::from_translation(objective),
                ChildOf(e),
            ))
            .id();
        let glass = materials.add(StandardMaterial {
            base_color: Color::srgb(0.02, 0.025, 0.03),
            perceptual_roughness: 0.05,
            reflectance: 0.6,
            ..default()
        });
        let reticle = commands
            .spawn((
                Mesh3d(meshes.add(Circle::new(0.017))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    base_color_texture: Some(if rangefinder {
                        crate::weapons::zero::pso_reticle(&mut images, LENS_ANGLE / o.magnification)
                    } else {
                        reticle_texture(&mut images)
                    }),
                    alpha_mode: AlphaMode::Mask(0.5),
                    unlit: true,
                    ..default()
                })),
                Transform::from_translation(o.eye + Vec3::Z * 0.0015),
                Visibility::Hidden,
                NotShadowCaster,
                ChildOf(e),
            ))
            .id();
        // The eyepiece faces the eye (+Z).
        commands.entity(e).with_child((
            Lens {
                camera,
                glass: glass.clone(),
                image: handle,
                reticle,
                scope: o.magnification < 5.0,
            },
            Mesh3d(meshes.add(Circle::new(0.017))),
            MeshMaterial3d(glass),
            Transform::from_translation(o.eye + Vec3::Z * 0.001),
            NotShadowCaster,
        ));
    }
}

fn lenses(
    lenses: Query<(&Lens, &GlobalTransform)>,
    mut cams: Query<&mut Camera>,
    mut vis: Query<&mut Visibility>,
    eyes: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (lens, gt) in &lenses {
        let at = gt.translation();
        let looking = eyes.iter().any(|eye| {
            let rel = at - eye.translation();
            rel.length() < 0.16 && rel.normalize_or_zero().dot(*eye.forward()) > 0.9
        });
        if let Ok(mut cam) = cams.get_mut(lens.camera) {
            if cam.is_active != looking {
                cam.is_active = looking;
                if let Ok(mut v) = vis.get_mut(lens.reticle) {
                    *v = if looking && lens.scope {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
                if let Some(mut m) = materials.get_mut(&lens.glass) {
                    if looking {
                        m.base_color = Color::WHITE;
                        m.unlit = true;
                        m.base_color_texture = Some(lens.image.clone());
                    } else {
                        m.base_color = Color::srgb(0.02, 0.025, 0.03);
                        m.unlit = false;
                        m.base_color_texture = None;
                    }
                }
            }
        }
    }
}

fn dress_cover(
    mut commands: Commands,
    walls: Query<Entity, Added<super::CoverWall>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<crate::render::materials::MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for e in &walls {
        let plate = library.get(
            crate::render::materials::Mat::PaintedMetal,
            &server,
            &mut materials,
        );
        let dark = library.get(
            crate::render::materials::Mat::DarkMetal,
            &server,
            &mut materials,
        );
        commands.entity(e).with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(crate::render::materials::metric_box(Vec3::new(
                    1.2, 1.05, 0.05,
                )))),
                MeshMaterial3d(plate),
            ));
            // A vision slit and two legs behind.
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.25, 0.04, 0.06))),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(0.0, 0.3, 0.0),
            ));
            for x in [-0.45, 0.45] {
                c.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.03, 0.03, 0.7))),
                    MeshMaterial3d(dark.clone()),
                    Transform::from_xyz(x, -0.25, 0.3).with_rotation(Quat::from_rotation_x(0.6)),
                ));
            }
        });
    }
}

#[derive(Component)]
struct Bubble;

/// A faint, shimmering shell round a running cloak.
fn cloak_bubbles(
    mut commands: Commands,
    cloaks: Res<super::Cloaks>,
    mut bubbles: Query<(Entity, &mut Transform), With<Bubble>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut assets: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    time: Res<Time>,
) {
    let (mesh, mat) = assets
        .get_or_insert_with(|| {
            (
                meshes.add(Sphere::new(1.0).mesh().ico(4).unwrap()),
                materials.add(StandardMaterial {
                    base_color: Color::srgba(0.6, 0.75, 0.9, 0.06),
                    emissive: LinearRgba::rgb(0.05, 0.08, 0.12),
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    cull_mode: None,
                    perceptual_roughness: 0.1,
                    ..default()
                }),
            )
        })
        .clone();
    let mut existing: Vec<(Entity, Mut<Transform>)> = bubbles.iter_mut().collect();
    let wobble = 1.0 + 0.01 * (time.elapsed_secs() * 7.0).sin();
    for (i, (c, r)) in cloaks.0.iter().enumerate() {
        let t = Transform::from_translation(*c).with_scale(Vec3::splat(r * wobble));
        if let Some((_, et)) = existing.get_mut(i) {
            **et = t;
        } else {
            commands.spawn((
                Bubble,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                t,
                bevy::light::NotShadowCaster,
            ));
        }
    }
    for (e, _) in existing.into_iter().skip(cloaks.0.len()) {
        commands.entity(e).despawn();
    }
}

/// The camera drone's camera, rendering into `image` while it flies.
#[derive(Component)]
struct DroneFeed {
    camera: Entity,
    image: Handle<Image>,
}

/// The screen above the left wrist that shows a flying drone's feed.
#[derive(Component)]
struct WristFeed(Handle<StandardMaterial>);

fn add_drone_feeds(
    mut commands: Commands,
    drones: Query<Entity, Added<super::camera_drone::CameraDrone>>,
    weather: Res<crate::world::sky::Weather>,
    mut images: ResMut<Assets<Image>>,
) {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
    for e in &drones {
        let mut image = Image::new_fill(
            Extent3d {
                width: 320,
                height: 240,
                ..default()
            },
            TextureDimension::D2,
            &[0u8; 8],
            TextureFormat::Rgba16Float,
            bevy::asset::RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        let image = images.add(image);
        let camera = commands
            .spawn((
                Name::new("Camera drone feed"),
                crate::render::OffscreenCamera,
                Camera3d::default(),
                Camera {
                    order: -6,
                    is_active: false,
                    ..default()
                },
                bevy::camera::RenderTarget::Image(image.clone().into()),
                bevy::camera::Hdr,
                Projection::Perspective(PerspectiveProjection {
                    fov: 55f32.to_radians(),
                    near: 0.1,
                    far: 1500.0,
                    ..default()
                }),
                bevy::core_pipeline::tonemapping::Tonemapping::None,
                bevy::camera::Exposure { ev100: 10.6 },
                weather.fog(),
                Msaa::Off,
                Transform::default(),
            ))
            .id();
        commands.entity(e).insert(DroneFeed { camera, image });
    }
}

/// The feed screen rides above the wrist panel, hidden until a drone flies.
fn add_wrist_feed(
    mut commands: Commands,
    hands: Query<(Entity, &crate::rig::RigHand, &ChildOf), Added<crate::rig::RigHand>>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, hand, parent) in &hands {
        if hand.side != crate::rig::Side::Left || !locals.contains(parent.parent()) {
            continue;
        }
        let m = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            ..default()
        });
        commands.entity(e).with_child((
            WristFeed(m.clone()),
            Mesh3d(meshes.add(Rectangle::new(0.096, 0.072))),
            MeshMaterial3d(m),
            Transform::from_xyz(0.0, 0.075, 0.1).with_rotation(Quat::from_rotation_x(-1.2)),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }
}

fn drone_feeds(
    drones: Query<(
        &super::camera_drone::CameraDrone,
        &GlobalTransform,
        &DroneFeed,
    )>,
    mut cams: Query<(&mut Camera, &mut Transform)>,
    mut screens: Query<(&WristFeed, &mut Visibility)>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut showing = None;
    for (d, gt, feed) in &drones {
        let on = d.airborne() && d.owner.is_some_and(|o| locals.contains(o));
        let Ok((mut cam, mut t)) = cams.get_mut(feed.camera) else {
            continue;
        };
        if cam.is_active != on {
            cam.is_active = on;
        }
        if on {
            let at = gt.translation() - Vec3::Y * 0.1;
            // Down at the middle of its circle (straight down: north up).
            let look = d.home - at;
            let up = if look.normalize_or_zero().y < -0.98 {
                Vec3::NEG_Z
            } else {
                Vec3::Y
            };
            *t = Transform::from_translation(at).looking_at(d.home, up);
            showing = Some(feed.image.clone());
        }
    }
    for (screen, mut vis) in &mut screens {
        let want = if showing.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if let Some(img) = &showing {
            if let Some(m) = materials.get(&screen.0) {
                if m.base_color_texture.as_ref() != Some(img) {
                    if let Some(mut m) = materials.get_mut(&screen.0) {
                        m.base_color_texture = Some(img.clone());
                    }
                }
            }
        }
    }
}

/// A rope: down from the edge, and back over it to the hook.
fn dress_ropes(
    mut commands: Commands,
    ropes: Query<(Entity, &super::climbing::Rope), Added<super::climbing::Rope>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mat: Local<Option<Handle<StandardMaterial>>>,
) {
    for (e, rope) in &ropes {
        let m = mat
            .get_or_insert_with(|| {
                // A climbing rope's orange: tan went into the rock and the
                // trees (`docs/LEGIBILITY.md`, affordances).
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.78, 0.33, 0.08),
                    perceptual_roughness: 0.95,
                    ..default()
                })
            })
            .clone();
        let mid = (rope.top + rope.bottom) * 0.5;
        let span = |a: Vec3, b: Vec3| {
            let d = b - a;
            Transform::from_translation((a + b) * 0.5 - mid)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y)))
        };
        let hang = rope.top - rope.bottom;
        let over = rope.anchor - rope.top;
        commands.entity(e).with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Cylinder::new(ROPE_R, hang.length()))),
                MeshMaterial3d(m.clone()),
                span(rope.bottom, rope.top),
            ));
            if over.length() > 0.05 {
                c.spawn((
                    Mesh3d(meshes.add(Cylinder::new(ROPE_R, over.length()))),
                    MeshMaterial3d(m.clone()),
                    span(rope.top, rope.anchor),
                ));
            }
        });
    }
}

/// The paper in a map case (0.28 m square, north up).
#[derive(Component)]
pub struct MapSheet;

/// The driver's cache, pencilled on a map sheet as a red cross.
#[derive(Component)]
struct CacheMark;

const SHEET: f32 = 0.28;

fn mark_caches(
    mut commands: Commands,
    requests: Res<crate::raid::requests::Requests>,
    opts: Res<crate::launch::LaunchOptions>,
    zone: Res<crate::world::Zone>,
    sheets: Query<(Entity, Option<&Children>), With<MapSheet>>,
    mut marks: Query<&mut Transform, With<CacheMark>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    let at = requests
        .cache_at
        .as_ref()
        .filter(|(map, _)| *map == opts.map)
        .map(|(_, p)| *p);
    for (sheet, children) in &sheets {
        let mark = children.and_then(|c| c.iter().find(|c| marks.contains(*c)));
        match (at, mark) {
            (Some(p), None) => {
                let (mesh, mat) = look
                    .get_or_insert_with(|| {
                        (
                            meshes.add(Cuboid::new(0.016, 0.0006, 0.003)),
                            materials.add(StandardMaterial {
                                base_color: Color::srgb(0.7, 0.08, 0.06),
                                perceptual_roughness: 0.95,
                                ..default()
                            }),
                        )
                    })
                    .clone();
                let local = Vec3::new(p[0], 0.0, p[1]) / zone.map.size * SHEET + Vec3::Y * 0.0006;
                commands.entity(sheet).with_child((
                    CacheMark,
                    Transform::from_translation(local),
                    Visibility::default(),
                    children![
                        (
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(mat.clone()),
                            Transform::from_rotation(Quat::from_rotation_y(0.785)),
                        ),
                        (
                            Mesh3d(mesh),
                            MeshMaterial3d(mat),
                            Transform::from_rotation(Quat::from_rotation_y(-0.785)),
                        ),
                    ],
                ));
            }
            (Some(p), Some(m)) => {
                if let Ok(mut t) = marks.get_mut(m) {
                    t.translation =
                        Vec3::new(p[0], 0.0, p[1]) / zone.map.size * SHEET + Vec3::Y * 0.0006;
                }
            }
            (None, Some(m)) => commands.entity(m).despawn(),
            (None, None) => {}
        }
    }
}

/// The driver's job, pencilled on a map sheet as a blue ring.
#[derive(Component)]
struct JobMark;

fn mark_job(
    mut commands: Commands,
    progression: Res<crate::progression::Progression>,
    opts: Res<crate::launch::LaunchOptions>,
    zone: Res<crate::world::Zone>,
    sheets: Query<(Entity, Option<&Children>), With<MapSheet>>,
    mut marks: Query<&mut Transform, With<JobMark>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    let at = progression
        .0
        .job
        .as_ref()
        .and_then(|j| j.mark())
        .filter(|(map, _)| *map == opts.map)
        .map(|(_, p)| p);
    let local = |p: [f32; 2]| Vec3::new(p[0], 0.0, p[1]) / zone.map.size * SHEET + Vec3::Y * 0.0006;
    for (sheet, children) in &sheets {
        let mark = children.and_then(|c| c.iter().find(|c| marks.contains(*c)));
        match (at, mark) {
            (Some(p), None) => {
                let (mesh, mat) = look
                    .get_or_insert_with(|| {
                        (
                            meshes.add(Torus::new(0.0085, 0.0100)),
                            materials.add(StandardMaterial {
                                base_color: Color::srgb(0.12, 0.2, 0.55),
                                perceptual_roughness: 0.95,
                                ..default()
                            }),
                        )
                    })
                    .clone();
                commands.entity(sheet).with_child((
                    JobMark,
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    // Flattened onto the paper.
                    Transform::from_translation(local(p)).with_scale(Vec3::new(1.0, 0.05, 1.0)),
                    Visibility::default(),
                ));
            }
            (Some(p), Some(m)) => {
                if let Ok(mut t) = marks.get_mut(m) {
                    t.translation = local(p);
                }
            }
            (None, Some(m)) => commands.entity(m).despawn(),
            (None, None) => {}
        }
    }
}
