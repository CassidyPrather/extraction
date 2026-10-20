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
                shell_marks,
                sputter,
                light_wrecks,
                smoulder,
                fly_sparks,
                drift_puffs,
                charge_emitters,
                zap_arcs,
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
    };
    commands.insert_resource(a.clone());
    a
}

#[derive(Component)]
struct Spark {
    vel: Vec3,
    life: f32,
}

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

/// A part shot below a third keeps sparking now and then; a core shot
/// below half smokes.
#[allow(clippy::too_many_arguments)]
fn sputter(
    mut commands: Commands,
    parts: Query<(Entity, &RobotPart, &GlobalTransform)>,
    robots: Query<&Robot>,
    sparks: Query<(), With<Spark>>,
    puffs: Query<(), With<Puff>>,
    existing: Option<Res<FxAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut timers: Local<std::collections::HashMap<Entity, f32>>,
    time: Res<Time>,
    mut dice: Local<Dice>,
) {
    let dt = time.delta_secs();
    let (mut n_sparks, mut n_puffs) = (sparks.iter().len(), puffs.iter().len());
    let mut a = None;
    timers.retain(|e, _| parts.contains(*e));
    for (e, part, gt) in &parts {
        let health = part.hp / part.max_hp.max(1.0);
        // Gas cells don't spark; a wreck smoulders instead.
        if matches!(part.role, PartRole::GasCell(_))
            || robots.get(part.robot).is_ok_and(|r| r.wrecked)
        {
            continue;
        }
        let core = part.role == PartRole::Core;
        let sparky = health < 0.34;
        let smoky = core && health < 0.5;
        if !sparky && !smoky {
            continue;
        }
        let t = timers.entry(e).or_insert_with(|| dice.f());
        *t -= dt;
        if *t > 0.0 {
            continue;
        }
        let a =
            a.get_or_insert_with(|| assets(&mut commands, &existing, &mut meshes, &mut materials));
        let at = gt.translation();
        if smoky && n_puffs < MAX_PUFFS {
            puff(&mut commands, a, &mut dice, at + Vec3::Y * 0.3, false, 0.35);
            n_puffs += 1;
            *t = 0.4 + dice.f() * 0.4;
        }
        if sparky && n_sparks < MAX_SPARKS && dice.f() < 0.5 {
            burst(&mut commands, a, &mut dice, at, Vec3::Y, 3, 2.5);
            n_sparks += 3;
            *t = 0.6 + dice.f() * 1.8;
        }
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

fn fly_sparks(
    mut commands: Commands,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut s, mut t) in &mut sparks {
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
        t.scale = Vec3::new(1.0, (speed * 0.02).clamp(0.01, 0.12), 1.0);
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
