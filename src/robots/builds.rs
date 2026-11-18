//! What the engineer builds (#98, Robot bodies; who builds what, where
//! and why is Robot minds'): one [`Build`] entity per build, its foot on
//! the ground, its progress 0..1 written by the mind. Each stage reads at
//! a glance:
//!
//! - **Barricade** (its local X across the way it blocks): struts first,
//!   one by one; then panels on them (from half way), what blocks; done,
//!   an orange strap across them ([`BarricadeStrap`]: cut it or drag it by
//!   it, and the panels drop, the mind's rule). Each panel a
//!   [`BarricadePanel`], a World collider.
//! - **Camera:** a mast growing up out of the ground; on it from 0.4 its
//!   head, a lens with a lit cone ([`CameraAim`], written by the mind)
//!   sweeping pale, the searchlights' red cast as it builds a fix on you;
//!   done, its cable to the network ([`CameraCable`]: a #101 cable
//!   `Line`). Its head a [`CameraLens`] to shoot.
//!
//! Its health in step with its progress is the mind's; its colliders, per
//! stage, are here.

use avian3d::prelude::*;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::render::fog_beam::{self, FogBeamMaterial, FogBeamParams};
use crate::world::Layer;

pub struct BuildsPlugin;

impl Plugin for BuildsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (frame_builds, (aim_cameras, string_cables)).chain());
    }
}

/// What an engineer builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum BuildKind {
    Barricade,
    Camera,
    /// A relay pylon: its body is `pylon`'s, as now.
    Pylon,
}

/// A build, as far as it's got: 0 just begun .. 1 done (the mind's).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Build {
    pub kind: BuildKind,
    pub progress: f32,
}

/// A barricade's strap (done): where hands grip to drag it, and where
/// cutters bite.
#[derive(Component, Clone, Copy, Debug)]
pub struct BarricadeStrap {
    pub of: Entity,
}

/// One of a barricade's panels: what blocks the way.
#[derive(Component, Clone, Copy, Debug)]
pub struct BarricadePanel {
    pub of: Entity,
}

/// A camera's head: shoot it out.
#[derive(Component, Clone, Copy, Debug)]
pub struct CameraLens {
    pub of: Entity,
}

/// Where a camera looks (the mind's): its yaw and pitch from the build's
/// own frame (rad), how wide and far it sees, and how far it's got with a
/// fix on you (0..1 over its 3.5 s).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CameraAim {
    pub yaw: f32,
    pub pitch: f32,
    pub half_angle: f32,
    pub reach: f32,
    pub fix: f32,
}

impl Default for CameraAim {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.25,
            half_angle: 0.3,
            reach: 22.0,
            fix: 0.0,
        }
    }
}

/// Where a camera's cable runs (world): the network's pylon or junction.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CameraCable {
    pub to: Vec3,
}

/// A barricade's size: across the way, how high.
pub const BARRICADE_WIDTH: f32 = 3.6;
pub const BARRICADE_HEIGHT: f32 = 1.5;
/// Its struts, and when each goes up (progress).
const STRUTS: [(f32, f32); 3] = [(-1.4, 0.0), (0.0, 0.16), (1.4, 0.32)];
/// Its panels: left and right, and when each goes on.
const PANELS: [(f32, f32); 2] = [(-0.9, 0.5), (0.9, 0.75)];
/// A camera's mast: how tall when done, and when its head goes on.
pub const MAST: f32 = 2.4;
const HEAD_AT: f32 = 0.4;

/// The pieces a build's look keeps (children of the build).
#[derive(Component)]
struct Framed {
    pieces: Vec<(Entity, Piece)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Piece {
    Strut(usize),
    Panel(usize),
    Strap,
    Mast,
    Head,
    Cone,
}

#[derive(Resource, Clone)]
struct BuildAssets {
    cube: Handle<Mesh>,
    rod: Handle<Mesh>,
    cone: Handle<Mesh>,
    timber: Handle<StandardMaterial>,
    panel: Handle<StandardMaterial>,
    strap: Handle<StandardMaterial>,
    mast: Handle<StandardMaterial>,
    head: Handle<StandardMaterial>,
    rim: Handle<StandardMaterial>,
}

fn build_assets(
    commands: &mut Commands,
    have: Option<&BuildAssets>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> BuildAssets {
    if let Some(a) = have {
        return a.clone();
    }
    let a = BuildAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        rod: meshes.add(Cylinder::new(1.0, 1.0)),
        cone: meshes.add(fog_beam::cone()),
        // Raw timber struts, grey steel panels: scavenged.
        timber: materials.add(StandardMaterial {
            base_color: Color::srgb(0.54, 0.42, 0.24),
            perceptual_roughness: 0.9,
            ..default()
        }),
        panel: materials.add(StandardMaterial {
            base_color: Color::srgb(0.49, 0.51, 0.53),
            metallic: 0.5,
            perceptual_roughness: 0.6,
            ..default()
        }),
        // The strap: the one shoot-here orange (cut it).
        strap: materials.add(StandardMaterial {
            base_color: super::weak_points::SHOOT_HERE,
            emissive: LinearRgba::from(super::weak_points::SHOOT_HERE) * 0.5,
            perceptual_roughness: 0.7,
            ..default()
        }),
        mast: materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.38, 0.4),
            metallic: 0.6,
            perceptual_roughness: 0.45,
            ..default()
        }),
        head: materials.add(StandardMaterial {
            base_color: Color::srgb(0.13, 0.13, 0.14),
            perceptual_roughness: 0.6,
            ..default()
        }),
        rim: materials.add(StandardMaterial {
            base_color: super::engineer_look::RIM,
            emissive: LinearRgba::from(super::engineer_look::RIM) * 2.0,
            ..default()
        }),
    };
    commands.insert_resource(a.clone());
    a
}

/// A strut's place: leaning back from the barricade's face, its foot
/// behind (+Z), its top over the face.
fn strut(x: f32) -> Transform {
    let len = BARRICADE_HEIGHT * 1.08;
    Transform::from_translation(Vec3::new(x, BARRICADE_HEIGHT * 0.5, 0.18))
        .with_rotation(Quat::from_rotation_x(-0.38))
        .with_scale(Vec3::new(0.06, len, 0.06))
}

/// A panel's place: a slab leaning on the struts, its foot on the ground.
fn panel(x: f32) -> Transform {
    Transform::from_translation(Vec3::new(x, BARRICADE_HEIGHT * 0.47, 0.0))
        .with_rotation(Quat::from_rotation_x(-0.12))
        .with_scale(Vec3::new(
            BARRICADE_WIDTH * 0.5 - 0.02,
            BARRICADE_HEIGHT * 0.94,
            0.04,
        ))
}

#[allow(clippy::type_complexity)]
fn frame_builds(
    mut commands: Commands,
    builds: Query<(Entity, &Build, Option<&Framed>), Or<(Changed<Build>, Without<Framed>)>>,
    have: Option<Res<BuildAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fog: ResMut<Assets<FogBeamMaterial>>,
    mut shown: Query<&mut Visibility>,
    mut xf: Query<&mut Transform, Without<Build>>,
) {
    if builds.is_empty() {
        return;
    }
    let a = build_assets(&mut commands, have.as_deref(), &mut meshes, &mut materials);
    for (e, b, framed) in &builds {
        let Some(framed) = framed else {
            // First seen: every piece it will have, hidden till its stage.
            let mut pieces = Vec::new();
            let mut add = |commands: &mut Commands,
                           piece: Piece,
                           mesh: &Handle<Mesh>,
                           mat: &Handle<StandardMaterial>,
                           t: Transform,
                           extra: &dyn Fn(&mut EntityCommands)| {
                let mut c = commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    t,
                    Visibility::Hidden,
                    ChildOf(e),
                ));
                extra(&mut c);
                pieces.push((c.id(), piece));
            };
            match b.kind {
                BuildKind::Barricade => {
                    for (i, (x, _)) in STRUTS.iter().enumerate() {
                        add(
                            &mut commands,
                            Piece::Strut(i),
                            &a.rod,
                            &a.timber,
                            strut(*x),
                            &|_| {},
                        );
                    }
                    for (i, (x, _)) in PANELS.iter().enumerate() {
                        add(
                            &mut commands,
                            Piece::Panel(i),
                            &a.cube,
                            &a.panel,
                            panel(*x),
                            &|c| {
                                c.insert(BarricadePanel { of: e });
                            },
                        );
                    }
                    add(
                        &mut commands,
                        Piece::Strap,
                        &a.cube,
                        &a.strap,
                        Transform::from_translation(Vec3::new(0.0, BARRICADE_HEIGHT * 0.55, -0.06))
                            .with_rotation(Quat::from_rotation_x(-0.12))
                            .with_scale(Vec3::new(BARRICADE_WIDTH + 0.06, 0.1, 0.03)),
                        &|c| {
                            c.insert(BarricadeStrap { of: e });
                        },
                    );
                }
                BuildKind::Camera => {
                    add(
                        &mut commands,
                        Piece::Mast,
                        &a.rod,
                        &a.mast,
                        Transform::from_scale(Vec3::new(0.05, 0.01, 0.05)),
                        &|_| {},
                    );
                    add(
                        &mut commands,
                        Piece::Head,
                        &a.cube,
                        &a.head,
                        Transform::from_translation(Vec3::Y * (MAST + 0.1))
                            .with_scale(Vec3::new(0.2, 0.16, 0.3)),
                        &|c| {
                            c.insert(CameraLens { of: e });
                            // Its lens, ringed in the lit tell's orange-red.
                            c.with_child((
                                Mesh3d(a.cube.clone()),
                                MeshMaterial3d(a.rim.clone()),
                                Transform::from_translation(Vec3::new(0.0, 0.0, -0.52))
                                    .with_scale(Vec3::new(0.7, 0.7, 0.08)),
                            ));
                        },
                    );
                    let material = fog.add(FogBeamMaterial {
                        params: FogBeamParams {
                            light: Vec4::new(0.85, 0.92, 1.0, 0.0),
                            apex: Vec4::new(0.0, 0.0, 0.0, 0.3f32.tan()),
                            axis: Vec3::NEG_Z.extend(10.0),
                            air: Vec4::new(0.5, 0.0, -1000.0, fog_beam::LAMP_SPREAD),
                        },
                        inside: false,
                    });
                    let cone = commands
                        .spawn((
                            Name::new("Camera cone"),
                            Mesh3d(a.cone.clone()),
                            MeshMaterial3d(material),
                            Transform::default(),
                            Visibility::Hidden,
                            NotShadowCaster,
                            NotShadowReceiver,
                            ConeOf(e),
                        ))
                        .id();
                    pieces.push((cone, Piece::Cone));
                }
                BuildKind::Pylon => {}
            }
            commands.entity(e).insert(Framed { pieces });
            continue;
        };
        // Each piece shown at its stage, its collider with it.
        let p = b.progress.clamp(0.0, 1.0);
        for &(pe, piece) in &framed.pieces {
            let on = shown_at(p, piece);
            if let Ok(mut v) = shown.get_mut(pe) {
                let want = if on {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *v != want {
                    *v = want;
                    let mut c = commands.entity(pe);
                    match (piece, on) {
                        (Piece::Strut(_), true) => {
                            c.insert((
                                RigidBody::Static,
                                Collider::cylinder(1.0, 1.0),
                                CollisionLayers::new(Layer::World, LayerMask::ALL),
                            ));
                        }
                        (Piece::Panel(_), true) => {
                            c.insert((
                                RigidBody::Static,
                                Collider::cuboid(1.0, 1.0, 1.0),
                                CollisionLayers::new(Layer::World, LayerMask::ALL),
                            ));
                        }
                        (Piece::Strap, true) => {
                            // What hands and cutters find.
                            c.insert((
                                crate::destruction::hooks::Cuttable { needs: 0.0 },
                                Sensor,
                                Collider::cuboid(1.0, 1.6, 2.5),
                                CollisionLayers::new(Layer::Hand, LayerMask::NONE),
                            ));
                        }
                        (Piece::Head, true) => {
                            c.insert((
                                RigidBody::Static,
                                Collider::cuboid(1.0, 1.0, 1.0),
                                CollisionLayers::new(Layer::World, LayerMask::ALL),
                            ));
                        }
                        (Piece::Cone, _) => {}
                        (_, false) => {
                            c.remove::<(RigidBody, Collider, Sensor)>();
                        }
                        _ => {}
                    }
                }
            }
            // The mast grows up out of the ground as it's built.
            if piece == Piece::Mast {
                if let Ok(mut t) = xf.get_mut(pe) {
                    let h = MAST * (p / HEAD_AT).min(1.0);
                    let want = Transform::from_translation(Vec3::Y * h * 0.5)
                        .with_scale(Vec3::new(0.05, h.max(0.01), 0.05));
                    if *t != want {
                        *t = want;
                    }
                }
            }
        }
    }
}

/// Whether a piece is up at `progress`.
fn shown_at(p: f32, piece: Piece) -> bool {
    match piece {
        Piece::Strut(i) => p > STRUTS[i].1,
        Piece::Panel(i) => p >= PANELS[i].1,
        Piece::Strap => p >= 1.0,
        Piece::Mast => p > 0.0,
        Piece::Head | Piece::Cone => p >= HEAD_AT,
    }
}

/// A camera's cone (top level: its params are world space), and whose.
#[derive(Component)]
struct ConeOf(Entity);

/// A camera's head turned where it looks, and its lit cone from its lens:
/// pale as it sweeps, the searchlights' red cast as it builds a fix.
#[allow(clippy::type_complexity)]
fn aim_cameras(
    mut commands: Commands,
    cameras: Query<(&Build, &GlobalTransform, Option<&CameraAim>, &Framed)>,
    mut cones: Query<
        (
            Entity,
            &ConeOf,
            &MeshMaterial3d<FogBeamMaterial>,
            &mut Transform,
            &mut Visibility,
        ),
        Without<Framed>,
    >,
    mut heads: Query<&mut Transform, (With<CameraLens>, Without<ConeOf>, Without<Framed>)>,
    mut fog: ResMut<Assets<FogBeamMaterial>>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    weather: Option<Res<crate::world::sky::Weather>>,
    spatial: SpatialQuery,
    eyes: crate::render::puff::Eyes,
) {
    let eye = crate::render::puff::eye(&eyes);
    let density = conditions
        .as_ref()
        .map_or(0.5, |c| fog_beam::air_density(c));
    let wet = conditions
        .as_ref()
        .map_or(0.5, |c| crate::world::beams::lamp_haze(c));
    let day = weather
        .as_ref()
        .map_or(1.0, |w| 1.0 - w.night.clamp(0.0, 1.0));
    for (cone_e, of, mat, mut t, mut v) in &mut cones {
        let Ok((b, gt, aim, framed)) = cameras.get(of.0) else {
            commands.entity(cone_e).despawn();
            continue;
        };
        let aim = aim.copied().unwrap_or_default();
        let on = b.progress >= HEAD_AT && !fog_beam::hidden();
        let turn = Quat::from_euler(EulerRot::YXZ, aim.yaw, aim.pitch, 0.0);
        // The head turns where it looks.
        if let Some(&(head, _)) = framed.pieces.iter().find(|(_, p)| *p == Piece::Head) {
            if let Ok(mut h) = heads.get_mut(head) {
                if h.rotation.angle_between(turn) > 0.002 {
                    h.rotation = turn;
                }
            }
        }
        if !on {
            v.set_if_neq(Visibility::Hidden);
            continue;
        }
        let rot = gt.rotation() * turn;
        let dir = rot * Vec3::NEG_Z;
        let from = gt.transform_point(Vec3::Y * (MAST + 0.1)) + dir * 0.18;
        let landed = Dir3::new(dir)
            .ok()
            .and_then(|d| {
                spatial.cast_ray(
                    from,
                    d,
                    aim.reach,
                    true,
                    &SpatialQueryFilter::from_mask([Layer::World]),
                )
            })
            .map_or(aim.reach, |h| h.distance);
        let tan = aim.half_angle.tan();
        let want = fog_beam::placed(from, dir, tan, landed);
        if t.translation.distance(want.translation) > 0.01
            || t.rotation.angle_between(want.rotation) > 0.003
            || (t.scale - want.scale).length() > 0.02
        {
            *t = want;
        }
        v.set_if_neq(Visibility::Visible);
        // Pale as it sweeps; the searchlights' red cast as it fixes on you.
        let light = super::searchlight_look::PALE
            .lerp(super::searchlight_look::ON_YOU, aim.fix.clamp(0.0, 1.0));
        let end = from + dir * landed;
        let p = FogBeamParams {
            light: light.extend(0.6 * wet + 0.2 * aim.fix),
            apex: from.extend(tan),
            axis: dir.extend(landed),
            air: Vec4::new(density, day, end.y, fog_beam::LAMP_SPREAD),
        };
        let inside = eye.is_some_and(|e| fog_beam::eye_inside(e, from, dir, tan, landed));
        if let Some(m) = fog.get(&mat.0) {
            let q = m.params;
            if (q.apex - p.apex).length() > 0.01
                || (q.axis - p.axis).length() > 0.003
                || (q.light - p.light).length() > 0.01
                || (q.air - p.air).length() > 0.02
                || m.inside != inside
            {
                if let Some(mut m) = fog.get_mut(&mat.0) {
                    m.params = p;
                    m.inside = inside;
                }
            }
        }
    }
}

/// A camera's cable, once it's done: a #101 cable from its mast's foot to
/// the network, a grip sleeve at the camera end.
#[allow(clippy::type_complexity)]
fn string_cables(
    mut commands: Commands,
    cameras: Query<(Entity, &Build, &GlobalTransform, &CameraCable)>,
    mut cables: Query<(Entity, &CableOf, &mut super::lines::LineSpan)>,
) {
    for (cable, of, mut span) in &mut cables {
        let Ok((_, b, gt, to)) = cameras.get(of.0) else {
            commands.entity(cable).despawn();
            continue;
        };
        if b.progress < 1.0 {
            commands.entity(cable).despawn();
            continue;
        }
        let want = cable_span(gt, to.to);
        if *span != want {
            *span = want;
        }
    }
    for (e, b, gt, to) in &cameras {
        if b.progress < 1.0 || cables.iter().any(|(_, of, _)| of.0 == e) {
            continue;
        }
        commands.spawn((
            Name::new("Camera cable"),
            CableOf(e),
            super::lines::Line::new(super::lines::Gauge::Cable).with_grip(0.08),
            cable_span(gt, to.to),
            Transform::default(),
            Visibility::default(),
        ));
    }
}

#[derive(Component)]
struct CableOf(Entity);

fn cable_span(gt: &GlobalTransform, to: Vec3) -> super::lines::LineSpan {
    super::lines::LineSpan {
        from: gt.transform_point(Vec3::new(0.0, 0.25, 0.06)),
        to,
        slack: 0.6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A barricade goes up in its stages: struts, then panels (what blocks
    /// the way), then its strap; a camera's head and cone come at 0.4.
    #[test]
    fn a_build_shows_its_stages_in_order() {
        let on = shown_at;
        assert!(on(0.1, Piece::Strut(0)) && !on(0.1, Piece::Panel(0)));
        assert!(on(0.5, Piece::Panel(0)) && !on(0.5, Piece::Strap));
        assert!(on(1.0, Piece::Strap));
        assert!(on(0.2, Piece::Mast) && !on(0.2, Piece::Head));
        assert!(on(0.4, Piece::Head) && on(0.4, Piece::Cone));
    }
}
