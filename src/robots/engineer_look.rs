//! How the engineer looks (#98, client only; `engineer_body`): a pale,
//! workaday cart (not armour grey: a worker, not a fighter), long and
//! low, its kit bundled on its back with struts standing up out of it,
//! and its scan lamp on a short arm at its front, the orange-red rim the
//! machines' lit tell. No gun.
//!
//! While it surveys a site its lamp throws a beam (`render::fog_beam`,
//! the searchlights' look, pale) over it; while it builds it hammers:
//! sparks at its nose.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use super::engineer_body::{Engineer, Hammering, KIT_SIZE, ScanLamp};
use super::{PartRole, RobotPart};
use crate::render::fog_beam::{self, FogBeamMaterial, FogBeamParams};

pub struct EngineerLookPlugin;

impl Plugin for EngineerLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (dress_engineers, make_beams, scan_beams, hammer_sparks),
        );
    }
}

/// Its paint: a pale, sandy workaday cream.
pub const PAINT: Color = Color::srgb(0.96, 0.92, 0.74);
/// The kit's canvas: olive drab.
pub const KIT: Color = Color::srgb(0.36, 0.4, 0.26);
/// The lamp's rim: the machines' orange-red lit tell (the searchlights').
pub const RIM: Color = Color::srgb(0.91, 0.34, 0.16);

#[derive(Component)]
struct Dressed;

/// No model to dress this engineer part from (walker.glb's `engineer_*`
/// nodes missing, or the model failed): its procedural look.
#[derive(Component)]
pub struct EngineerFallback;

/// Its scan beam is made.
#[derive(Component)]
struct Beamed;

/// Its scan beam (a child of the lamp part) and the beam's material.
#[derive(Component)]
struct ScanBeam {
    robot: Entity,
    material: Handle<FogBeamMaterial>,
}

/// How far its scan lamp throws, and how wide (rad).
const SCAN_REACH: f32 = 9.0;
const SCAN_HALF: f32 = 0.22;

#[allow(clippy::too_many_arguments)]
fn dress_engineers(
    mut commands: Commands,
    parts: Query<(Entity, &RobotPart), (With<EngineerFallback>, Without<Dressed>)>,
    engineers: Query<(), With<Engineer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut looks: Local<Option<Looks>>,
) {
    for (e, part) in &parts {
        if !engineers.contains(part.robot) {
            continue;
        }
        let l = looks.get_or_insert_with(|| Looks::new(&mut meshes, &mut materials));
        let piece = |commands: &mut Commands,
                     mesh: &Handle<Mesh>,
                     mat: &Handle<StandardMaterial>,
                     t: Transform| {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                t,
                ChildOf(e),
            ));
        };
        match part.role {
            PartRole::Core => {
                let at = super::body_shells::ENGINEER_CORE_AT;
                let size = super::body_shells::ENGINEER_CORE;
                // The hull: a long pale box, a dark skirt at its foot.
                piece(
                    &mut commands,
                    &l.cube,
                    &l.paint,
                    Transform::from_translation(at + Vec3::Y * 0.02)
                        .with_scale(size - Vec3::new(0.0, 0.04, 0.0)),
                );
                piece(
                    &mut commands,
                    &l.cube,
                    &l.dark,
                    Transform::from_translation(at - Vec3::Y * (size.y * 0.5 - 0.02))
                        .with_scale(Vec3::new(size.x + 0.04, 0.06, size.z + 0.04)),
                );
                // A sloped nose under the eye: it reads front-first.
                piece(
                    &mut commands,
                    &l.cube,
                    &l.paint,
                    Transform::from_translation(Vec3::new(0.0, -0.08, -0.38))
                        .with_rotation(Quat::from_rotation_x(-0.5))
                        .with_scale(Vec3::new(size.x * 0.9, 0.16, 0.14)),
                );
                // Tool rails down its flanks.
                for x in [-1.0, 1.0] {
                    piece(
                        &mut commands,
                        &l.rod,
                        &l.dark,
                        Transform::from_translation(Vec3::new(
                            x * (size.x * 0.5 + 0.02),
                            0.04,
                            at.z,
                        ))
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(
                            0.025,
                            size.z * 0.85,
                            0.025,
                        )),
                    );
                }
            }
            PartRole::Kit => {
                // The bundle: olive canvas, a dark strap round it, and the
                // struts it builds with standing up out of it (its outline:
                // nothing else in the line-up has them).
                piece(
                    &mut commands,
                    &l.cube,
                    &l.kit,
                    Transform::from_scale(KIT_SIZE),
                );
                piece(
                    &mut commands,
                    &l.cube,
                    &l.dark,
                    Transform::from_scale(Vec3::new(KIT_SIZE.x + 0.02, KIT_SIZE.y + 0.02, 0.06)),
                );
                for (x, z, h, lean) in [
                    (-0.14, -0.16, 0.55, -0.12),
                    (0.0, 0.05, 0.7, 0.04),
                    (0.15, 0.2, 0.48, 0.14),
                ] {
                    piece(
                        &mut commands,
                        &l.rod,
                        &l.steel,
                        Transform::from_translation(Vec3::new(
                            x,
                            KIT_SIZE.y * 0.5 + h * 0.5 - 0.1,
                            z,
                        ))
                        .with_rotation(Quat::from_rotation_z(lean))
                        .with_scale(Vec3::new(0.035, h, 0.035)),
                    );
                }
                // A folded panel slung on its side, and a coil of cable.
                piece(
                    &mut commands,
                    &l.cube,
                    &l.steel,
                    Transform::from_translation(Vec3::new(KIT_SIZE.x * 0.5 + 0.03, 0.02, 0.0))
                        .with_scale(Vec3::new(0.03, KIT_SIZE.y * 1.2, KIT_SIZE.z * 0.8)),
                );
                piece(
                    &mut commands,
                    &l.coil,
                    &l.dark,
                    Transform::from_translation(Vec3::new(-KIT_SIZE.x * 0.5 - 0.04, 0.0, 0.1))
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                );
            }
            PartRole::Lamp(_) => {
                // The part lies along its own Y (turned to face forward):
                // a short arm back to the hull, the drum, its orange-red
                // rim and its lens.
                piece(
                    &mut commands,
                    &l.rod,
                    &l.dark,
                    Transform::from_translation(Vec3::new(0.0, -0.1, 0.0))
                        .with_scale(Vec3::new(0.03, 0.16, 0.03)),
                );
                piece(&mut commands, &l.drum, &l.dark, Transform::IDENTITY);
                piece(
                    &mut commands,
                    &l.ring,
                    &l.rim,
                    Transform::from_translation(Vec3::new(0.0, -0.065, 0.0)),
                );
                piece(
                    &mut commands,
                    &l.lens,
                    &l.lens_mat,
                    Transform::from_translation(Vec3::new(0.0, -0.062, 0.0)),
                );
            }
            _ => {}
        }
        commands.entity(e).insert(Dressed);
    }
}

struct Looks {
    cube: Handle<Mesh>,
    rod: Handle<Mesh>,
    drum: Handle<Mesh>,
    ring: Handle<Mesh>,
    lens: Handle<Mesh>,
    coil: Handle<Mesh>,
    paint: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    kit: Handle<StandardMaterial>,
    steel: Handle<StandardMaterial>,
    rim: Handle<StandardMaterial>,
    lens_mat: Handle<StandardMaterial>,
}

impl Looks {
    fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        Self {
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            rod: meshes.add(Cylinder::new(1.0, 1.0)),
            drum: meshes.add(Cylinder::new(0.075, 0.12)),
            ring: meshes.add(Torus::new(0.06, 0.085)),
            lens: meshes.add(Cylinder::new(0.062, 0.01)),
            coil: meshes.add(Torus::new(0.07, 0.11)),
            // (A touch of its own light, so it stays pale at dusk: a worker
            // in workaday cream, never a fighter's grey.)
            paint: materials.add(StandardMaterial {
                base_color: PAINT,
                emissive: LinearRgba::from(PAINT) * 0.08,
                perceptual_roughness: 0.75,
                ..default()
            }),
            dark: materials.add(StandardMaterial {
                base_color: Color::srgb(0.13, 0.13, 0.14),
                perceptual_roughness: 0.7,
                ..default()
            }),
            kit: materials.add(StandardMaterial {
                base_color: KIT,
                perceptual_roughness: 0.95,
                ..default()
            }),
            steel: materials.add(StandardMaterial {
                base_color: Color::srgb(0.6, 0.62, 0.64),
                metallic: 0.7,
                perceptual_roughness: 0.4,
                ..default()
            }),
            rim: materials.add(StandardMaterial {
                base_color: RIM,
                emissive: LinearRgba::from(RIM) * 2.0,
                ..default()
            }),
            lens_mat: materials.add(StandardMaterial {
                base_color: Color::srgb(0.85, 0.92, 1.0),
                emissive: LinearRgba::rgb(1.2, 1.3, 1.4),
                ..default()
            }),
        }
    }
}

/// Its scan lamp's beam (dark until it surveys), however the lamp is
/// dressed.
fn make_beams(
    mut commands: Commands,
    lamps: Query<(Entity, &RobotPart), Without<Beamed>>,
    engineers: Query<(), With<Engineer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut fog: ResMut<Assets<FogBeamMaterial>>,
    mut cone: Local<Option<Handle<Mesh>>>,
) {
    for (e, part) in &lamps {
        if !matches!(part.role, PartRole::Lamp(_)) || !engineers.contains(part.robot) {
            continue;
        }
        let cone = cone
            .get_or_insert_with(|| meshes.add(fog_beam::cone()))
            .clone();
        let material = fog.add(FogBeamMaterial {
            params: FogBeamParams {
                light: Vec4::new(0.85, 0.92, 1.0, 0.0),
                apex: Vec4::new(0.0, 0.0, 0.0, SCAN_HALF.tan()),
                axis: Vec3::NEG_Z.extend(SCAN_REACH),
                air: Vec4::new(0.5, 0.0, -1000.0, fog_beam::LAMP_SPREAD),
            },
            inside: false,
        });
        commands.spawn((
            Name::new("Engineer scan beam"),
            ScanBeam {
                robot: part.robot,
                material: material.clone(),
            },
            Mesh3d(cone),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            NotShadowReceiver,
        ));
        commands.entity(e).insert(Beamed);
    }
}

/// Its scan beam: lit while it surveys, from its lens toward what it's
/// looking at (or ahead and down), cut where it lands.
#[allow(clippy::type_complexity)]
fn scan_beams(
    engineers: Query<(&ScanLamp, &super::Robot), With<Engineer>>,
    lamps: Query<(&RobotPart, &GlobalTransform), Without<ScanBeam>>,
    mut beams: Query<(&ScanBeam, &mut Transform, &mut Visibility)>,
    mut fog: ResMut<Assets<FogBeamMaterial>>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    weather: Option<Res<crate::world::sky::Weather>>,
    spatial: avian3d::prelude::SpatialQuery,
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
    for (beam, mut t, mut v) in &mut beams {
        let Ok((scan, robot)) = engineers.get(beam.robot) else {
            continue;
        };
        let on = scan.on && !robot.wrecked && !fog_beam::hidden();
        // The lamp: its part, alive.
        let lamp = lamps
            .iter()
            .find(|(p, _)| p.robot == beam.robot && matches!(p.role, PartRole::Lamp(_)));
        let Some((part, gt)) = lamp.filter(|(p, _)| p.alive() && on) else {
            v.set_if_neq(Visibility::Hidden);
            continue;
        };
        let _ = part;
        // (The lens faces down the part's -Y.)
        let from = gt.translation() + gt.rotation() * Vec3::NEG_Y * 0.07;
        let dir = match scan.aim {
            Some(at) => (at - from).normalize_or(Vec3::NEG_Y),
            None => (gt.rotation() * Vec3::NEG_Y + Vec3::NEG_Y * 0.6).normalize(),
        };
        let landed = spatial
            .cast_ray(
                from + dir * 0.1,
                Dir3::new(dir).unwrap_or(Dir3::NEG_Y),
                SCAN_REACH,
                true,
                &avian3d::prelude::SpatialQueryFilter::from_mask([crate::world::Layer::World]),
            )
            .map_or(SCAN_REACH, |h| h.distance + 0.1);
        let tan = SCAN_HALF.tan();
        let want = fog_beam::placed(from, dir, tan, landed);
        if t.translation.distance(want.translation) > 0.01
            || t.rotation.angle_between(want.rotation) > 0.003
            || (t.scale - want.scale).length() > 0.02
        {
            *t = want;
        }
        v.set_if_neq(Visibility::Visible);
        let end = from + dir * landed;
        let p = FogBeamParams {
            // Pale (the searchlights' sweep), a lamp's strength, as the air
            // shows it (`beams::lamp_haze`).
            light: Vec4::new(0.85, 0.92, 1.0, 0.7 * wet),
            apex: from.extend(tan),
            axis: dir.extend(landed),
            air: Vec4::new(density, day, end.y, fog_beam::LAMP_SPREAD),
        };
        let inside = eye.is_some_and(|e| fog_beam::eye_inside(e, from, dir, tan, landed));
        if let Some(m) = fog.get(&beam.material) {
            let q = m.params;
            if (q.apex - p.apex).length() > 0.01
                || (q.axis - p.axis).length() > 0.003
                || (q.light - p.light).length() > 0.01
                || (q.air - p.air).length() > 0.02
                || m.inside != inside
            {
                if let Some(mut m) = fog.get_mut(&beam.material) {
                    m.params = p;
                    m.inside = inside;
                }
            }
        }
    }
}

/// A spark struck while it hammers.
#[derive(Component)]
struct WorkSpark {
    v: Vec3,
    life: f32,
}

/// While it builds: a spark or two from its nose every beat.
fn hammer_sparks(
    mut commands: Commands,
    engineers: Query<(&Hammering, &GlobalTransform, &super::Robot), With<Engineer>>,
    mut sparks: Query<(Entity, &mut WorkSpark, &mut Transform)>,
    time: Res<Time>,
    mut beat: Local<f32>,
    mut kit: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    for (e, mut s, mut t) in &mut sparks {
        s.life -= dt;
        if s.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        s.v += Vec3::NEG_Y * 9.0 * dt;
        t.translation += s.v * dt;
    }
    *beat += dt;
    if *beat < 0.45 {
        return;
    }
    *beat = 0.0;
    let (mesh, mat) = kit
        .get_or_insert_with(|| {
            (
                meshes.add(Cuboid::new(0.012, 0.012, 0.05)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.8, 0.4),
                    emissive: LinearRgba::rgb(12.0, 7.0, 2.0),
                    unlit: true,
                    ..default()
                }),
            )
        })
        .clone();
    let k = time.elapsed_secs();
    for (h, gt, robot) in &engineers {
        if !h.0 || robot.wrecked {
            continue;
        }
        let nose = gt.transform_point(Vec3::new(0.0, -0.3, -0.7));
        for i in 0..3 {
            let a = k * 7.3 + i as f32 * 2.1;
            let v = gt.rotation() * Vec3::new(a.sin() * 1.2, 1.6 + 0.4 * (a * 1.7).cos(), -0.8);
            commands.spawn((
                WorkSpark { v, life: 0.35 },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(nose).looking_to(v, Vec3::Y),
                NotShadowCaster,
            ));
        }
    }
}
