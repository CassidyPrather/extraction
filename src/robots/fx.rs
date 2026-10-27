//! Damage you can see (the hit itself is Weapon feel's, `weapons::impacts`,
//! by part): a badly shot-up part
//! that keeps sputtering, a smoking core, and a wreck that smoulders (a
//! low orange glow and a thread of dark smoke) for a couple of minutes.
//!
//! Budget: at most [`MAX_SPARKS`] sparks, [`MAX_PUFFS`] smoke puffs and
//! [`MAX_EMBERS`] wreck glows (point lights without shadows) at once; past
//! that new ones are skipped. Everything shares two meshes and three
//! materials.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::wreck::Broken;
use super::{PartRole, Robot, RobotPart};

pub const MAX_SPARKS: usize = 96;
pub const MAX_PUFFS: usize = 64;
pub const MAX_EMBERS: usize = 4;
/// Bullet marks on robot shells, all robots together.
pub const MAX_MARKS: usize = 160;
/// How long a wreck smoulders, seconds.
const SMOULDER: f32 = 150.0;

pub struct RobotFxPlugin;

impl Plugin for RobotFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                wreck_trails,
                wreck_impacts,
                drag_sparks,
                scuff_dust,
                eye_out,
                cell_arcs,
                hazard_bursts,
                shell_marks,
                sputter,
                light_wrecks,
                smoulder,
                fly_sparks,
                drift_puffs,
                charge_emitters,
                zap_arcs,
                splash_steps,
            )
                .chain(),
        );
    }
}

#[derive(Resource, Clone)]
struct FxAssets {
    spark: Handle<Mesh>,
    spark_mat: Handle<StandardMaterial>,
    puff: Handle<Mesh>,
    smoke: Handle<StandardMaterial>,
    soot: Handle<StandardMaterial>,
    /// Blue-white, for a crawler's arcs and crackle.
    arc: Handle<StandardMaterial>,
    /// Water thrown up by a foot.
    water: Handle<StandardMaterial>,
    /// A wound's coolant: pale green, a little lit so it reads at range.
    coolant: Handle<StandardMaterial>,
    /// Dirt and snow kicked up by a dragged foot.
    dust: Handle<StandardMaterial>,
    snow_dust: Handle<StandardMaterial>,
    /// A hazard's marks (`hazard_look`): orange embers, ice glinting, a
    /// gas cloud, a frost's breath, a shimmer in the air, a hush's haze.
    embers: Handle<StandardMaterial>,
    /// A snare's or an EMP's arcs: electric blue (a crawler's own are
    /// paler: `arc`), to read on a white shell.
    bolt: Handle<StandardMaterial>,
    crystals: Handle<StandardMaterial>,
    gas: Handle<StandardMaterial>,
    frost: Handle<StandardMaterial>,
    shimmer: Handle<StandardMaterial>,
    still: Handle<StandardMaterial>,
}

fn assets(
    commands: &mut Commands,
    existing: &Option<Res<FxAssets>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> FxAssets {
    if let Some(a) = existing {
        return (*a).clone();
    }
    let a = FxAssets {
        spark: meshes.add(Cuboid::new(0.012, 1.0, 0.012)),
        spark_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.5),
            emissive: LinearRgba::rgb(60.0, 30.0, 8.0),
            unlit: true,
            ..default()
        }),
        puff: meshes.add(Sphere::new(0.5).mesh().ico(2).unwrap()),
        smoke: materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.55, 0.55, 0.16),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        soot: materials.add(StandardMaterial {
            base_color: Color::srgba(0.16, 0.15, 0.14, 0.2),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        arc: materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.9, 1.0),
            emissive: LinearRgba::rgb(30.0, 50.0, 120.0),
            unlit: true,
            ..default()
        }),
        coolant: materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.95, 0.6),
            emissive: LinearRgba::rgb(0.6, 2.2, 0.8),
            ..default()
        }),
        dust: materials.add(StandardMaterial {
            base_color: Color::srgba(0.42, 0.36, 0.28, 0.32),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        snow_dust: materials.add(StandardMaterial {
            base_color: Color::srgba(0.92, 0.95, 1.0, 0.4),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        // (Unlit: these show their base colour, bright.)
        embers: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.42, 0.08),
            unlit: true,
            ..default()
        }),
        bolt: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.55, 1.0),
            unlit: true,
            ..default()
        }),
        // (Cyan, not white: it reads on a white shell.)
        crystals: materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.85, 1.0),
            unlit: true,
            ..default()
        }),
        gas: materials.add(StandardMaterial {
            base_color: Color::srgba(0.72, 0.8, 0.25, 0.3),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        frost: materials.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.9, 1.0, 0.45),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        shimmer: materials.add(StandardMaterial {
            base_color: Color::srgba(0.55, 0.75, 0.85, 0.35),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        }),
        still: materials.add(StandardMaterial {
            base_color: Color::srgba(0.12, 0.1, 0.18, 0.35),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        water: materials.add(StandardMaterial {
            base_color: Color::srgba(0.72, 0.78, 0.82, 0.55),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.15,
            reflectance: 0.6,
            ..default()
        }),
    };
    commands.insert_resource(a.clone());
    a
}

#[derive(Component)]
struct Spark {
    vel: Vec3,
    life: f32,
}

/// A spark drawn this many times as wide (and as long, in its square root):
/// a hazard's mark, to read at 10 m (`hazard_bursts`).
#[derive(Component)]
struct Bold(f32);

/// How bold a hazard's streaks and how big its clouds are, against a
/// wound's.
const HAZARD_BOLD: f32 = 4.0;
const HAZARD_CLOUD: f32 = 2.0;

#[derive(Component)]
struct Puff {
    vel: Vec3,
    life: f32,
    max: f32,
    size: f32,
}

/// A wreck smouldering: seconds left, and to the next puff.
#[derive(Component)]
struct Smoulder {
    left: f32,
    next: f32,
}

/// The glow under a smouldering wreck.
#[derive(Component)]
struct Ember;

/// A tiny LCG, for scatter.
#[derive(Default)]
struct Dice(u32);

impl Dice {
    fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    fn signed(&mut self) -> Vec3 {
        Vec3::new(self.f() - 0.5, self.f() - 0.5, self.f() - 0.5) * 2.0
    }
}

fn burst(
    commands: &mut Commands,
    a: &FxAssets,
    dice: &mut Dice,
    at: Vec3,
    normal: Vec3,
    n: usize,
    speed: f32,
) {
    for _ in 0..n {
        let vel = (normal + dice.signed() * 0.8).normalize_or(Vec3::Y) * speed * (0.5 + dice.f());
        commands.spawn((
            Spark {
                vel,
                life: 0.2 + dice.f() * 0.35,
            },
            Mesh3d(a.spark.clone()),
            MeshMaterial3d(a.spark_mat.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(0.0)),
            NotShadowCaster,
        ));
    }
}

fn puff(commands: &mut Commands, a: &FxAssets, dice: &mut Dice, at: Vec3, dark: bool, size: f32) {
    let max = 2.5 + dice.f() * 1.5;
    commands.spawn((
        Puff {
            vel: Vec3::Y * (0.5 + dice.f() * 0.3) + dice.signed() * 0.18,
            life: max,
            max,
            size,
        },
        Mesh3d(a.puff.clone()),
        MeshMaterial3d(if dark {
            a.soot.clone()
        } else {
            a.smoke.clone()
        }),
        Transform::from_translation(at).with_scale(Vec3::splat(size * 0.3)),
        NotShadowCaster,
    ));
}

/// A hurt part shows it (`damage::tell`): sparks, smoke and coolant off
/// the wound at its band's rates, each on its own count so they don't
/// tick together.
fn sputter(
    mut commands: Commands,
    parts: Query<(Entity, &RobotPart, &GlobalTransform)>,
    robots: Query<&Robot>,
    sparks: Query<(), With<Spark>>,
    puffs: Query<(), With<Puff>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut counts: Local<std::collections::HashMap<Entity, Vec3>>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    time: Res<Time>,
    mut dice: Local<Dice>,
) {
    let dt = time.delta_secs();
    let eye = cameras.iter().next().map(|g| g.translation());
    let (mut n_sparks, mut n_puffs) = (sparks.iter().len(), puffs.iter().len());
    let mut a = None;
    counts.retain(|e, _| parts.contains(*e));
    for (e, part, gt) in &parts {
        // A wreck smoulders instead (`smoulder`).
        if robots.get(part.robot).map_or(true, |r| r.wrecked) {
            continue;
        }
        let tell = super::damage::tell(part.role, part.hp / part.max_hp.max(1.0));
        if tell.sparks <= 0.0 && tell.smoke <= 0.0 && tell.coolant <= 0.0 {
            continue;
        }
        // (Started anywhere in the count, so a hurt robot's parts don't
        // all go at once.)
        let c = counts
            .entry(e)
            .or_insert_with(|| Vec3::new(dice.f(), dice.f(), dice.f()));
        *c += Vec3::new(tell.sparks, tell.smoke, tell.coolant) * dt * (0.6 + 0.8 * dice.f());
        if c.max_element() < 1.0 {
            continue;
        }
        let a =
            a.get_or_insert_with(|| assets(&mut commands, &existing, &mut meshes, &mut materials));
        let at = gt.translation();
        if c.x >= 1.0 {
            c.x -= 1.0;
            if n_sparks < MAX_SPARKS {
                burst(&mut commands, a, &mut dice, at, Vec3::Y, 3, 2.5);
                n_sparks += 3;
            }
        }
        if c.y >= 1.0 {
            c.y -= 1.0;
            if n_puffs < MAX_PUFFS {
                // A gas cell's vent is pale; a burning part's is grey.
                let size = 0.3 * at_range(eye, at).sqrt();
                puff(&mut commands, a, &mut dice, at + Vec3::Y * 0.2, false, size);
                n_puffs += 1;
            }
        }
        if c.z >= 1.0 {
            c.z -= 1.0;
            if n_sparks < MAX_SPARKS {
                leak(&mut commands, a, &mut dice, at);
                n_sparks += 2;
            }
        }
    }
}

/// Coolant spattering from a wound: a couple of pale-green drops, out and
/// down.
fn leak(commands: &mut Commands, a: &FxAssets, dice: &mut Dice, at: Vec3) {
    for _ in 0..2 {
        let out = Vec3::new(dice.f() - 0.5, 0.0, dice.f() - 0.5).normalize_or(Vec3::X);
        commands.spawn((
            Spark {
                vel: out * (0.5 + dice.f() * 0.6) - Vec3::Y * 0.4,
                life: 0.5 + dice.f() * 0.3,
            },
            Mesh3d(a.spark.clone()),
            MeshMaterial3d(a.coolant.clone()),
            Transform::from_translation(at).with_scale(Vec3::splat(0.0)),
            NotShadowCaster,
        ));
    }
}

/// A robot that comes apart starts smouldering.
fn light_wrecks(
    mut commands: Commands,
    broken: Query<(Entity, &Robot), Added<Broken>>,
    embers: Query<(), With<Ember>>,
) {
    let mut lit = embers.iter().len();
    for (e, robot) in &broken {
        if !matches!(
            robot.kind,
            super::RobotKind::Walker | super::RobotKind::Drone
        ) {
            continue;
        }
        let Ok(mut ec) = commands.get_entity(e) else {
            continue;
        };
        ec.insert(Smoulder {
            left: SMOULDER,
            next: 0.0,
        });
        if lit < MAX_EMBERS {
            lit += 1;
            ec.with_child((
                Ember,
                PointLight {
                    color: Color::srgb(1.0, 0.45, 0.15),
                    intensity: 0.0,
                    range: 5.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.1, 0.0),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn smoulder(
    mut commands: Commands,
    mut wrecks: Query<(Entity, &mut Smoulder, &GlobalTransform, Option<&Children>)>,
    mut embers: Query<(&mut PointLight, &mut Transform), With<Ember>>,
    puffs: Query<(), With<Puff>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut dice: Local<Dice>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let mut n_puffs = puffs.iter().len();
    let mut a = None;
    for (e, mut s, gt, children) in &mut wrecks {
        s.left -= dt;
        s.next -= dt;
        let k = (s.left / SMOULDER).clamp(0.0, 1.0);
        // The glow: a slow, uneven breathing, dying down over the minutes.
        for c in children.into_iter().flatten() {
            if let Ok((mut light, mut lt)) = embers.get_mut(*c) {
                let breath = 0.6
                    + 0.25 * (now * 1.7 + e.index_u32() as f32).sin()
                    + 0.15 * (now * 5.3).sin();
                light.intensity = 9_000.0 * k * breath;
                // Stay under the wreck as it rolls.
                lt.translation = gt.rotation().inverse() * Vec3::Y * 0.1;
            }
        }
        if s.left <= 0.0 {
            commands.entity(e).remove::<Smoulder>();
            for c in children.into_iter().flatten() {
                if embers.contains(*c) {
                    commands.entity(*c).despawn();
                }
            }
            continue;
        }
        if s.next <= 0.0 && n_puffs < MAX_PUFFS {
            let a = a.get_or_insert_with(|| {
                assets(&mut commands, &existing, &mut meshes, &mut materials)
            });
            let at = gt.translation() + Vec3::Y * 0.25 + dice.signed() * 0.1;
            puff(&mut commands, a, &mut dice, at, true, 0.3 + 0.3 * k);
            n_puffs += 1;
            // Thick at first, a thread later.
            s.next = 0.12 + (1.0 - k) * 0.8 + dice.f() * 0.12;
        }
    }
}

/// How much bigger a wound's sparks, drops and smoke are drawn this far
/// from the eye, so they still read at 20-40 m: 1 within 10 m, up to 4.
fn at_range(eye: Option<Vec3>, at: Vec3) -> f32 {
    eye.map_or(1.0, |e| (e.distance(at) / 10.0).clamp(1.0, 4.0))
}

fn fly_sparks(
    mut commands: Commands,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform, Option<&Bold>)>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let eye = cameras.iter().next().map(|g| g.translation());
    for (e, mut s, mut t, bold) in &mut sparks {
        s.life -= dt;
        if s.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        s.vel += Vec3::new(0.0, -9.81, 0.0) * dt;
        s.vel *= 1.0 - 1.5 * dt;
        t.translation += s.vel * dt;
        let speed = s.vel.length();
        // A streak along its path.
        t.rotation = Quat::from_rotation_arc(Vec3::Y, s.vel / speed.max(1e-3));
        let b = bold.map_or(1.0, |b| b.0);
        let w = at_range(eye, t.translation) * b;
        t.scale = Vec3::new(w, (speed * 0.02).clamp(0.01, 0.12) * w.sqrt() * b.sqrt(), w);
    }
}

fn drift_puffs(
    mut commands: Commands,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut p, mut t) in &mut puffs {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let age = 1.0 - p.life / p.max;
        p.vel *= 1.0 - 0.3 * dt;
        t.translation += p.vel * dt;
        // Swells as it rises; thins away at the end.
        let fade = (p.life / (p.max * 0.3)).min(1.0);
        t.scale = Vec3::splat(p.size * (0.4 + 2.2 * age) * fade.sqrt());
    }
}

/// A crawler's emitter charging: a blue glow that swells over the charge,
/// and crackle.
#[derive(Component)]
struct Charging {
    left: f32,
    next: f32,
}

#[allow(clippy::too_many_arguments)]
fn charge_emitters(
    mut commands: Commands,
    mut charges: MessageReader<super::indoor::CrawlerCharge>,
    crawlers: Query<&Children>,
    emitters: Query<&GlobalTransform, With<super::indoor::CrawlerGun>>,
    mut glows: Query<(Entity, &mut Charging, &mut PointLight, &GlobalTransform)>,
    sparks: Query<(), With<Spark>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut dice: Local<Dice>,
) {
    use super::indoor::ZAP_CHARGE;
    for c in charges.read() {
        let Some(emitter) = crawlers
            .get(c.crawler)
            .ok()
            .and_then(|ch| ch.iter().find(|e| emitters.contains(*e)))
        else {
            continue;
        };
        commands.entity(emitter).with_child((
            Charging {
                left: ZAP_CHARGE,
                next: 0.0,
            },
            PointLight {
                color: Color::srgb(0.55, 0.75, 1.0),
                intensity: 0.0,
                range: 6.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, -0.08),
        ));
    }
    let dt = time.delta_secs();
    let mut n = sparks.iter().len();
    let mut a = None;
    for (e, mut ch, mut light, gt) in &mut glows {
        ch.left -= dt;
        ch.next -= dt;
        if ch.left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let k = 1.0 - ch.left / ZAP_CHARGE;
        // Swelling, and stuttering like a bad contact.
        let flick = if dice.f() < 0.25 { 0.3 } else { 1.0 };
        light.intensity = 30_000.0 * k * k * flick;
        if ch.next <= 0.0 && n < MAX_SPARKS {
            let a = a.get_or_insert_with(|| {
                assets(&mut commands, &existing, &mut meshes, &mut materials)
            });
            for _ in 0..2 {
                let vel = dice.signed().normalize_or(Vec3::Y) * (1.0 + dice.f() * 2.0);
                commands.spawn((
                    Spark {
                        vel,
                        life: 0.08 + dice.f() * 0.1,
                    },
                    Mesh3d(a.spark.clone()),
                    MeshMaterial3d(a.arc.clone()),
                    Transform::from_translation(gt.translation()).with_scale(Vec3::ZERO),
                    NotShadowCaster,
                ));
            }
            n += 2;
            ch.next = 0.04 + dice.f() * 0.06;
        }
    }
}

/// A zap: a jagged blue-white arc from the emitter to where it aimed, a
/// tenth of a second long, and a flash.
#[derive(Component)]
struct Arc {
    left: f32,
}

#[allow(clippy::too_many_arguments)]
fn zap_arcs(
    mut commands: Commands,
    mut zaps: MessageReader<super::indoor::CrawlerZap>,
    mut arcs: Query<(Entity, &mut Arc)>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut dice: Local<Dice>,
) {
    const SEGMENTS: usize = 10;
    for z in zaps.read() {
        let a = assets(&mut commands, &existing, &mut meshes, &mut materials);
        let span = z.to - z.from;
        let len = span.length();
        if len < 0.05 {
            continue;
        }
        let side = span.any_orthonormal_vector();
        let up = span.cross(side).normalize_or(Vec3::Y);
        // Kinks across the line, biggest in the middle.
        let points: Vec<Vec3> = (0..=SEGMENTS)
            .map(|i| {
                let t = i as f32 / SEGMENTS as f32;
                let bulge = (std::f32::consts::PI * t).sin().sqrt() * len * 0.1;
                let j = if i == 0 || i == SEGMENTS {
                    Vec3::ZERO
                } else {
                    (side * (dice.f() - 0.5) + up * (dice.f() - 0.5)) * 2.0 * bulge
                };
                z.from + span * t + j
            })
            .collect();
        let root = commands
            .spawn((
                Arc { left: 0.1 },
                Transform::from_translation(z.from),
                Visibility::default(),
                PointLight {
                    color: Color::srgb(0.6, 0.8, 1.0),
                    intensity: 60_000.0,
                    range: 8.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
            ))
            .id();
        for w in points.windows(2) {
            let d = w[1] - w[0];
            let l = d.length().max(1e-3);
            commands.spawn((
                Mesh3d(a.spark.clone()),
                MeshMaterial3d(a.arc.clone()),
                // Child of the root at `from`, so local = world - from.
                Transform::from_translation((w[0] + w[1]) * 0.5 - z.from)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, d / l))
                    .with_scale(Vec3::new(1.4, l, 1.4)),
                NotShadowCaster,
                ChildOf(root),
            ));
        }
    }
    let dt = time.delta_secs();
    for (e, mut arc) in &mut arcs {
        arc.left -= dt;
        if arc.left <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}

/// A drone going down trails dark smoke.
fn wreck_trails(
    mut commands: Commands,
    falling: Query<(Entity, &GlobalTransform), With<super::wreck::Spiral>>,
    puffs: Query<(), With<Puff>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut next: Local<std::collections::HashMap<Entity, f32>>,
    time: Res<Time>,
    mut dice: Local<Dice>,
) {
    let dt = time.delta_secs();
    next.retain(|e, _| falling.contains(*e));
    let mut n = puffs.iter().len();
    for (e, gt) in &falling {
        let t = next.entry(e).or_insert(0.0);
        *t -= dt;
        if *t > 0.0 || n >= MAX_PUFFS {
            continue;
        }
        *t = 0.05;
        let a = assets(&mut commands, &existing, &mut meshes, &mut materials);
        puff(&mut commands, &a, &mut dice, gt.translation(), true, 0.35);
        n += 1;
    }
}

/// A wreck hitting the ground: a burst of sparks and a puff of dust.
#[allow(clippy::too_many_arguments)]
fn wreck_impacts(
    mut commands: Commands,
    mut impacts: MessageReader<super::wreck::WreckImpact>,
    sparks: Query<(), With<Spark>>,
    puffs: Query<(), With<Puff>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dice: Local<Dice>,
) {
    let (mut ns, mut np) = (sparks.iter().len(), puffs.iter().len());
    for imp in impacts.read() {
        let a = assets(&mut commands, &existing, &mut meshes, &mut materials);
        if ns < MAX_SPARKS {
            burst(&mut commands, &a, &mut dice, imp.at, Vec3::Y, 10, 5.0);
            ns += 10;
        }
        for _ in 0..4 {
            if np >= MAX_PUFFS {
                break;
            }
            let at = imp.at + dice.signed() * Vec3::new(0.6, 0.1, 0.6) * imp.size
                - Vec3::Y * 0.3 * imp.size;
            puff(&mut commands, &a, &mut dice, at, false, 0.6 * imp.size);
            np += 1;
        }
    }
}

/// A dead leg dragging along the ground as its walker limps throws sparks.
#[allow(clippy::too_many_arguments)]
fn drag_sparks(
    mut commands: Commands,
    walkers: Query<(&super::walker::Walker, &super::walker::LegPoses)>,
    sparks: Query<(), With<Spark>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dice: Local<Dice>,
    time: Res<Time>,
) {
    let mut n = sparks.iter().len();
    let dt = time.delta_secs();
    for (w, poses) in &walkers {
        let speed = w.velocity.length();
        if speed < 0.3 {
            continue;
        }
        for (i, f) in w.feet.iter().enumerate() {
            if f.alive || n >= MAX_SPARKS {
                continue;
            }
            // About six a second at a walk, a few at a time.
            if dice.f() > dt * 6.0 * speed {
                continue;
            }
            let foot = poses.0[i][2];
            let a = assets(&mut commands, &existing, &mut meshes, &mut materials);
            let back = -w.velocity.normalize_or_zero();
            burst(
                &mut commands,
                &a,
                &mut dice,
                foot,
                back + Vec3::Y * 0.6,
                3,
                2.5,
            );
            n += 3;
        }
    }
}

/// An eye shot out: its socket spits a fistful of sparks as it goes
/// (`visuals::last_flicker` is the eye's own last stutter).
fn eye_out(
    mut commands: Commands,
    mut deaths: MessageReader<super::PartDestroyed>,
    sparks: Query<(), With<Spark>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dice: Local<Dice>,
) {
    let mut n = sparks.iter().len();
    for d in deaths.read() {
        if d.role != PartRole::Sensor || n >= MAX_SPARKS {
            continue;
        }
        let a = assets(&mut commands, &existing, &mut meshes, &mut materials);
        burst(&mut commands, &a, &mut dice, d.at, Vec3::Y, 10, 3.5);
        n += 10;
    }
}

/// A live cell throwing a hand off (`salvage::Arced`): a crack of
/// blue-white arc off it.
fn cell_arcs(
    mut commands: Commands,
    mut arcs: MessageReader<super::salvage::Arced>,
    sparks: Query<(), With<Spark>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dice: Local<Dice>,
) {
    let mut n = sparks.iter().len();
    for arc in arcs.read() {
        if n >= MAX_SPARKS {
            continue;
        }
        let a = assets(&mut commands, &existing, &mut meshes, &mut materials);
        for _ in 0..12 {
            let vel = (Vec3::Y + dice.signed()).normalize_or(Vec3::Y) * (1.5 + 2.5 * dice.f());
            commands.spawn((
                Spark {
                    vel,
                    life: 0.08 + dice.f() * 0.12,
                },
                Mesh3d(a.spark.clone()),
                MeshMaterial3d(a.arc.clone()),
                Transform::from_translation(arc.at).with_scale(Vec3::splat(0.0)),
                NotShadowCaster,
            ));
        }
        n += 12;
    }
}

/// A hazard striking a machine (`hazards::HazardStruck`) marks it its own
/// way (`hazard_look::mark`): streaks off it and a cloud round it, sized
/// to read at range (`at_range`), again each time it's struck.
#[allow(clippy::too_many_arguments)]
fn hazard_bursts(
    mut commands: Commands,
    mut struck: MessageReader<super::hazards::HazardStruck>,
    bodies: Query<(&GlobalTransform, Option<&super::anim::Gait>)>,
    sparks: Query<(), With<Spark>>,
    puffs: Query<(), With<Puff>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut dice: Local<Dice>,
) {
    use super::hazard_look::{Bits, Cloud, mark};
    let eye = cameras.iter().next().map(|g| g.translation());
    let (mut n_sparks, mut n_puffs) = (sparks.iter().len(), puffs.iter().len());
    let mut a = None;
    for s in struck.read() {
        let m = mark(s.hazard, s.effect);
        if !m.visible() {
            continue;
        }
        // On its body (a hazard's `at` is where it is, roughly).
        let (at, size) = bodies
            .get(s.robot)
            .map(|(g, gait)| (g.translation(), gait.map_or(1.0, |g| g.size)))
            .unwrap_or((s.at, 1.0));
        let a =
            a.get_or_insert_with(|| assets(&mut commands, &existing, &mut meshes, &mut materials));
        let range = at_range(eye, at);
        if let Some((bits, n)) = m.bits {
            let (mat, speed, up, life) = match bits {
                Bits::Sparks => (&a.spark_mat, 3.5, 0.6, 0.35),
                // (Arcs cling: slow, and longer-lived, crawling over it.)
                Bits::Arcs => (&a.bolt, 1.2, 0.3, 0.45),
                Bits::Embers => (&a.embers, 1.4, 1.2, 0.8),
                Bits::Crystals => (&a.crystals, 1.0, 0.4, 0.9),
            };
            for _ in 0..n {
                if n_sparks >= MAX_SPARKS {
                    break;
                }
                let out = dice.signed().normalize_or(Vec3::X);
                let from = at + out * 0.4 * size;
                commands.spawn((
                    Spark {
                        vel: (out + Vec3::Y * up).normalize_or(Vec3::Y) * speed * (0.6 + dice.f()),
                        life: life * (0.6 + 0.8 * dice.f()),
                    },
                    Bold(HAZARD_BOLD),
                    Mesh3d(a.spark.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_translation(from).with_scale(Vec3::splat(0.0)),
                    NotShadowCaster,
                ));
                n_sparks += 1;
            }
        }
        if let Some((cloud, n)) = m.cloud {
            let (mat, rise, out) = match cloud {
                Cloud::Smoke => (&a.smoke, 0.5, 0.2),
                Cloud::Soot => (&a.soot, 0.4, 0.3),
                Cloud::Dust => (&a.dust, 0.3, 1.0),
                Cloud::Gas => (&a.gas, 0.15, 0.4),
                Cloud::Frost => (&a.frost, 0.2, 0.5),
                Cloud::Shimmer => (&a.shimmer, 0.3, 0.3),
                Cloud::Still => (&a.still, 0.02, 0.05),
            };
            for _ in 0..n {
                if n_puffs >= MAX_PUFFS {
                    break;
                }
                let dir = Vec3::new(dice.f() - 0.5, 0.0, dice.f() - 0.5).normalize_or(Vec3::X);
                let max = 1.0 + dice.f() * 0.8;
                commands.spawn((
                    Puff {
                        vel: Vec3::Y * rise + dir * out,
                        life: max,
                        max,
                        size: 0.35 * HAZARD_CLOUD * size * range.sqrt(),
                    },
                    Mesh3d(a.puff.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_translation(at + dir * 0.3 * size).with_scale(Vec3::splat(0.0)),
                    NotShadowCaster,
                ));
                n_puffs += 1;
            }
        }
    }
}

/// A foot dragged along the ground kicks up dirt (snow where it lies):
/// a dead leg trailing, or a bad one scuffing through its swing
/// (`anim::SCUFF_UNTIL`). Low, quick puffs behind the foot, so a limp
/// reads at range even in a still frame.
#[allow(clippy::too_many_arguments)]
fn scuff_dust(
    mut commands: Commands,
    walkers: Query<(
        &super::walker::Walker,
        &super::anim::Gait,
        &super::walker::LegPoses,
        &Robot,
    )>,
    puffs: Query<(), With<Puff>>,
    zone: Option<Res<crate::world::Zone>>,
    snow: Option<Res<crate::world::snow::Snow>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dice: Local<Dice>,
    time: Res<Time>,
) {
    let Some(zone) = zone else {
        return;
    };
    let mut n = puffs.iter().len();
    let dt = time.delta_secs();
    let mut a = None;
    for (w, g, poses, robot) in &walkers {
        let speed = w.velocity.length();
        if speed < 0.3 || robot.wrecked {
            continue;
        }
        for (i, f) in w.feet.iter().enumerate() {
            if n >= MAX_PUFFS {
                return;
            }
            let dragging =
                !f.alive || (g.leg_health[i] <= 0.5 && f.t > 0.0 && f.t < super::anim::SCUFF_UNTIL);
            if !dragging {
                continue;
            }
            let foot = poses.0[i][2];
            if foot.y - zone.ground(foot.x, foot.z) > 0.12 {
                continue;
            }
            // About eight a second at a walk.
            if dice.f() > dt * 8.0 * speed {
                continue;
            }
            let a = a.get_or_insert_with(|| {
                assets(&mut commands, &existing, &mut meshes, &mut materials)
            });
            let snowy = snow.as_ref().is_some_and(|s| s.lies(foot.y) > 0.3);
            let back = -w.velocity.normalize_or_zero();
            let max = 0.7 + dice.f() * 0.5;
            commands.spawn((
                Puff {
                    vel: back * 0.6 + Vec3::Y * (0.35 + dice.f() * 0.3) + dice.signed() * 0.15,
                    life: max,
                    max,
                    size: 0.22 * g.size,
                },
                Mesh3d(a.puff.clone()),
                MeshMaterial3d(if snowy {
                    a.snow_dust.clone()
                } else {
                    a.dust.clone()
                }),
                Transform::from_translation(foot + Vec3::Y * 0.05).with_scale(Vec3::splat(0.0)),
                NotShadowCaster,
            ));
            n += 1;
        }
    }
}

/// Bullet marks on robot shells, oldest first (each mark is a pock and its
/// hole).
#[derive(Default)]
struct Marks(std::collections::VecDeque<[Entity; 2]>);

/// A round that strikes a robot's shell leaves a mark on it: a bright pock
/// with a dark hole, Weapon feel's look (`weapons::impacts`), parented to
/// the part so it moves with it (and goes with it when it's shot off).
fn shell_marks(
    mut commands: Commands,
    mut impacts: MessageReader<crate::weapons::impacts::Impact>,
    parts: Query<(&RobotPart, &GlobalTransform)>,
    look: Option<Res<crate::weapons::impacts::FxAssets>>,
    mut marks: Local<Marks>,
) {
    use crate::weapons::impacts::{DECAL_PROUD, ImpactKind};
    let Some(look) = look else {
        impacts.clear();
        return;
    };
    for imp in impacts.read() {
        if imp.kind != ImpactKind::Robot {
            continue;
        }
        let Some((part_e, (part, gt))) = imp.entity.and_then(|e| parts.get(e).ok().map(|p| (e, p)))
        else {
            continue;
        };
        // Gas cells are fabric, not shell.
        if matches!(part.role, PartRole::GasCell(_)) {
            continue;
        }
        let n = imp.normal.normalize_or(Vec3::Y);
        let inv = gt.affine().inverse();
        let scale = gt.scale().max_element().max(1e-3);
        let local_rot = gt.rotation().inverse() * Quat::from_rotation_arc(Vec3::Z, n);
        let at = |proud: f32| inv.transform_point3(imp.point + n * proud);
        let pock = commands
            .spawn((
                Mesh3d(look.disc.clone()),
                MeshMaterial3d(look.bright_pock.clone()),
                Transform::from_translation(at(DECAL_PROUD))
                    .with_rotation(local_rot)
                    .with_scale(Vec3::splat(0.035 / scale)),
                NotShadowCaster,
                ChildOf(part_e),
            ))
            .id();
        let hole = commands
            .spawn((
                Mesh3d(look.disc.clone()),
                MeshMaterial3d(look.hole.clone()),
                Transform::from_translation(at(DECAL_PROUD + 0.004))
                    .with_rotation(local_rot)
                    .with_scale(Vec3::splat(0.014 / scale)),
                NotShadowCaster,
                ChildOf(part_e),
            ))
            .id();
        marks.0.push_back([pock, hole]);
        while marks.0.len() > MAX_MARKS {
            if let Some(old) = marks.0.pop_front() {
                for e in old {
                    if let Ok(mut ec) = commands.get_entity(e) {
                        ec.despawn();
                    }
                }
            }
        }
    }
}

/// A foot landing this far under standing water (m), or in a puddle at
/// least this full (`world::puddles`), throws water.
const SPLASH_DEPTH: f32 = 0.02;
const SPLASH_PUDDLE: f32 = 0.4;

/// Whether a foot coming down at height `y` splashes: in water whose
/// surface is at `water`, or in a puddle this full.
fn splashes(y: f32, water: Option<f32>, puddle: f32) -> bool {
    water.is_some_and(|w| w > y + SPLASH_DEPTH) || puddle >= SPLASH_PUDDLE
}

/// A walker's foot in a stream or a puddle throws a spray of water.
#[allow(clippy::too_many_arguments)]
fn splash_steps(
    mut commands: Commands,
    mut steps: MessageReader<super::walker::WalkerStep>,
    zone: Option<Res<crate::world::Zone>>,
    wet: Option<Res<crate::world::weather::Wetness>>,
    levels: Option<Res<crate::world::underground::Levels>>,
    sparks: Query<(), With<Spark>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut dice: Local<Dice>,
) {
    let Some(zone) = zone else {
        steps.clear();
        return;
    };
    let wet = wet.map_or(0.0, |w| w.level);
    let mut n = sparks.iter().len();
    let mut a = None;
    for step in steps.read() {
        if n >= MAX_SPARKS {
            break;
        }
        let p = [step.at.x, step.at.z];
        let puddle = if wet > 0.0 {
            crate::world::puddles::puddle_at(&zone.hm, p, wet)
        } else {
            0.0
        };
        let water = crate::world::underground::site_water(&zone, levels.as_deref(), step.at)
            .unwrap_or_else(|| zone.hm.water_level(p));
        if !splashes(step.at.y, water, puddle) {
            continue;
        }
        let a =
            a.get_or_insert_with(|| assets(&mut commands, &existing, &mut meshes, &mut materials));
        let at = Vec3::new(
            step.at.x,
            water.unwrap_or(step.at.y).max(step.at.y),
            step.at.z,
        );
        for _ in 0..8 {
            let out = Vec3::new(dice.f() - 0.5, 0.0, dice.f() - 0.5).normalize_or(Vec3::X);
            let vel = (Vec3::Y * 1.6 + out * 0.9) * (0.7 + 0.6 * dice.f());
            commands.spawn((
                Spark {
                    vel,
                    life: 0.25 + dice.f() * 0.25,
                },
                Mesh3d(a.spark.clone()),
                MeshMaterial3d(a.water.clone()),
                Transform::from_translation(at).with_scale(Vec3::splat(0.0)),
                NotShadowCaster,
            ));
            n += 1;
        }
    }
}

#[cfg(test)]
mod splash_tests {
    use super::*;

    /// A foot splashes in a stream over it or in a full puddle, not on
    /// dry ground or in water below it (a bridge over a stream).
    #[test]
    fn feet_splash_where_the_water_is() {
        assert!(splashes(10.0, Some(10.2), 0.0), "in a stream");
        assert!(splashes(10.0, None, 0.8), "in a puddle");
        assert!(!splashes(10.0, None, 0.1), "damp ground");
        assert!(!splashes(10.0, Some(6.0), 0.0), "over the water");
        assert!(!splashes(10.0, None, 0.0), "dry");
    }
}
