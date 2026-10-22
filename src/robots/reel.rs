//! `--gallery reel:WHAT[,SHOTS[,EVERY]]`: a robot doing something in front
//! of the `--camera`, shot as a frame sequence, for looking at how bodies
//! move (walk cycles, banking, limps, wrecks). Time steps a fixed 1/30 s a
//! frame however slow the renderer is, so EVERY frames is EVERY/30 s.
//! Frames go next to the `--screenshot` path as `<stem>_NN.png`; give a
//! `--frames` larger than the reel so the ordinary screenshot never fires.
//!
//! ```text
//! scripts/shot.sh docs/shots/robot-bodies/walk.png 40,0,262,0,-12 99999 \
//!     --no-robots --gallery reel:walk,12,4
//! ```
//!
//! WHAT is `walk` (across the view), `turn` (a right angle in front of
//! you), `damaged` (plates on their last bolt, the core smoking, its
//! eye guttering, walking across), `limp` (a leg shot off, then across), `crawl` (two legs shot
//! off: it drags itself across), `hurt` (a leg at a fifth),
//! `sorter` (the Depot's gantry at its sorting, in the open), `hound` (a
//! hound running across), `tells`/`tellsbefore` (a shot leg, a dead
//! eye, a failing core and a hit rotor at 24-36 m, and the same whole),
//! `knockdown` (a heavy round into a walker's
//! side: down, and up again), `rubble` (a walker over a heap of broken
//! blocks), `crawler` (a crawler across), `eyes` (calm, suspicious and seeing walkers and a drone, for judging eyes
//! and lights at `--hour dusk` or `night`), `eyesfar` (calm walkers at
//! 30, 60 and 80 m and an alert one, for eyes at range after dark), `shot` (rounds into a walker: the marks they leave), `zap` (a crawler charging and
//! zapping past you), `drone` (a drone circling in front of you), `wreck`/`dronewreck` (killed
//! in view, then left to settle).

use std::time::Duration;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use super::drone::spawn_drone;
use super::walker::{Walker, spawn_walker};
use super::{Brain, Job, JobKind, PartDestroyed, PartRole, RobotPart, Senses};
use crate::launch::LaunchOptions;
use crate::world::Zone;

pub struct ReelPlugin;

/// For the `eyes` reel: how alert to hold this robot (0 calm, 1
/// suspicious, 2 seeing you).
#[derive(Component)]
struct EyeLevel(u8);

impl Plugin for ReelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, reel);
    }
}

/// Frames to let things settle before the first shot.
const WARMUP: u32 = 45;

/// Which damage a `tells` robot takes.
#[derive(Component)]
struct TellCase(u8);

#[derive(Default)]
struct Reel {
    frame: u32,
    subject: Option<Entity>,
    shots: u32,
    every: u32,
    taken: u32,
    /// Frame on which to kill the subject (wrecks).
    kill_at: Option<u32>,
}

#[allow(clippy::too_many_arguments)]
fn reel(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Option<Res<Zone>>,
    mut state: Local<Reel>,
    mut walkers: Query<&mut Walker>,
    mut drones: Query<&mut super::drone::Drone>,
    mut parts: Query<(Entity, &mut RobotPart, &GlobalTransform)>,
    mut robots: Query<&mut super::Robot>,
    mut destroyed: MessageWriter<PartDestroyed>,
    mut charges: MessageWriter<super::indoor::CrawlerCharge>,
    mut zaps: MessageWriter<super::indoor::CrawlerZap>,
    mut shots: MessageWriter<crate::weapons::ballistics::Shot>,
    mut levels: Query<(&EyeLevel, &mut super::Awareness)>,
    (mut meshes, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    cases: Query<&TellCase>,
) {
    let (Some(spec), Some((cam, yaw, pitch)), Some(zone), Some(out)) =
        (&opts.gallery, opts.camera, zone, &opts.screenshot)
    else {
        return;
    };
    let Some(spec) = spec.strip_prefix("reel:") else {
        return;
    };
    let mut args = spec.split(',');
    let what = args.next().unwrap_or("walk");
    state.frame += 1;
    let frame = state.frame;
    if frame == 1 {
        state.shots = args.next().and_then(|v| v.parse().ok()).unwrap_or(12);
        state.every = args.next().and_then(|v| v.parse().ok()).unwrap_or(4).max(1);
        commands.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f32(1.0 / 30.0),
        ));
        return;
    }
    let ground = zone.ground(cam.x, cam.z);
    let eye = Vec3::new(
        cam.x,
        if cam.y == 0.0 {
            ground + 1.7
        } else {
            cam.y.max(ground + 0.3)
        },
        cam.z,
    );
    let rot = Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0);
    let fwd = rot * Vec3::NEG_Z;
    let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
    let right = Vec3::new(-flat.z, 0.0, flat.x);
    let job = |at: Vec3| Job {
        kind: JobKind::Survey,
        site: at,
        duration: 1.0e6,
        exit: at,
    };
    if frame == 3 {
        let walk_start = eye + flat * 6.0 - right * 4.5;
        let e = match what {
            "drone" | "dronewreck" => {
                let at = eye + flat * 8.0 + Vec3::Y * 0.8;
                spawn_drone(&mut commands, at, job(at))
            }
            "turn" => {
                let start = eye + flat * 9.0 - right * 3.5;
                spawn_walker(&mut commands, &zone, start, job(start))
            }
            "sorter" => {
                // An open-air hall in front of you, its rails running
                // across your view, a slab for its floor.
                let c = eye + flat * 12.0;
                let mut top = f32::MIN;
                for i in -12..=12 {
                    for j in -8..=8 {
                        let p = c + right * i as f32 + flat * j as f32;
                        top = top.max(zone.ground(p.x, p.z));
                    }
                }
                let rot = Quat::from_mat3(&Mat3::from_cols(right, Vec3::Y, -flat));
                let hall = Transform::from_xyz(c.x, top, c.z).with_rotation(rot);
                commands.spawn((
                    avian3d::prelude::RigidBody::Static,
                    avian3d::prelude::Collider::cuboid(30.0, 0.2, 20.0),
                    avian3d::prelude::CollisionLayers::new(
                        crate::world::Layer::World,
                        [crate::world::Layer::Item, crate::world::Layer::Robot],
                    ),
                    hall * Transform::from_xyz(0.0, -0.1, 0.0),
                ));
                super::sorter::spawn_sorter(
                    &mut commands,
                    hall,
                    Vec2::new(12.0, 8.0),
                    6.0,
                    super::sorter::depot_spots(),
                    6,
                )
            }
            "hound" => {
                let start = eye + flat * 6.0 - right * 7.0;
                super::hound::spawn_hound(&mut commands, &zone, start, job(start))
            }
            "eyes" => {
                // Three walkers and a drone, calm, suspicious and seeing
                // you (set below), to judge eyes and lights by.
                let mut first = None;
                for (k, x) in [-3.4f32, 0.0, 3.4].into_iter().enumerate() {
                    let at = eye + flat * 8.0 + right * x;
                    let w = spawn_walker(&mut commands, &zone, at, job(at));
                    commands
                        .entity(w)
                        .remove::<Senses>()
                        .remove::<Brain>()
                        .insert(EyeLevel(k as u8));
                    first.get_or_insert(w);
                }
                let d = eye + flat * 9.0 + Vec3::Y * 2.2 + right * 1.3;
                let drone = spawn_drone(&mut commands, d, job(d));
                commands
                    .entity(drone)
                    .remove::<Senses>()
                    .remove::<Brain>()
                    .insert(EyeLevel(2));
                first.unwrap()
            }
            "eyesfar" => {
                // Calm walkers at 30, 60 and 80 m, and one that sees you
                // beside the far one: do their eyes read after dark?
                let mut first = None;
                // Bearings taken: a far walker straight behind a near one
                // is hidden by it.
                let mut taken: Vec<Vec3> = Vec::new();
                for (k, (d, x, level)) in [
                    (30.0f32, -2.0f32, 0u8),
                    (60.0, 3.0, 0),
                    (80.0, -4.0, 0),
                    (80.0, 4.0, 2),
                ]
                .into_iter()
                .enumerate()
                {
                    // Swung round (up to 40°, inside the frame) until the
                    // ground between doesn't hide it: any map, any camera.
                    let tries: Vec<Vec3> = (0..=32)
                        .map(|i| {
                            (if i % 2 == 0 { 1.0 } else { -1.0 }) * (i / 2 + i % 2) as f32 * 2.5
                        })
                        .map(|deg: f32| {
                            let b = (x / d).atan() + deg.to_radians();
                            let dir = flat * b.cos() + right * b.sin();
                            let p = eye + dir * d;
                            zone.on_ground(p.x, p.z)
                        })
                        .collect();
                    let seen = |at: &Vec3| hidden(&zone, eye, *at + Vec3::Y * 1.1);
                    let free = |at: &Vec3| {
                        let dir = (*at - eye).with_y(0.0).normalize_or_zero();
                        taken
                            .iter()
                            .all(|t| t.angle_between(dir).to_degrees() > 5.0)
                    };
                    // The nearest swing that's clear and not behind another,
                    // else the least hidden.
                    let at = tries
                        .iter()
                        .copied()
                        .find(|at| seen(at) < 0.0 && free(at))
                        .unwrap_or_else(|| {
                            tries
                                .iter()
                                .copied()
                                .min_by(|a, b| seen(a).total_cmp(&seen(b)))
                                .unwrap()
                        });
                    info!(
                        "reel eyesfar: walker {k} at {:.0} m, {at:.1} (hidden by {:.2} m)",
                        at.distance(eye),
                        hidden(&zone, eye, at + Vec3::Y * 1.1).max(0.0)
                    );
                    taken.push((at - eye).with_y(0.0).normalize_or_zero());
                    let w = spawn_walker(&mut commands, &zone, at, job(at));
                    commands
                        .entity(w)
                        .remove::<Senses>()
                        .remove::<Brain>()
                        .insert(EyeLevel(level));
                    if k == 0 {
                        first = Some(w);
                    }
                }
                first.unwrap()
            }
            "wreck" | "shot" | "knockdown" | "damaged" => {
                let at = eye + flat * if what == "shot" { 3.5 } else { 5.0 };
                spawn_walker(&mut commands, &zone, at, job(at))
            }
            "crawler" | "zap" => {
                let at = if what == "zap" {
                    eye + flat * 3.0 + right * 0.6
                } else {
                    walk_start
                };
                let e = super::walker::spawn_walker_sized(
                    &mut commands,
                    &zone,
                    at,
                    job(at),
                    false,
                    60.0,
                    super::indoor::CRAWLER_SIZE,
                    None,
                );
                commands.entity(e).with_children(|c| {
                    c.spawn((
                        super::indoor::CrawlerGun,
                        super::part_bundle(
                            e,
                            PartRole::Weapon,
                            30.0,
                            avian3d::prelude::Collider::sphere(0.08),
                            Transform::from_xyz(0.0, -0.1, -0.44),
                        ),
                    ));
                });
                e
            }
            "tells" | "tellsbefore" => {
                // Damage at range (20-40 m): a walker on a shot leg, one
                // with its eye shot out, one with a failing core, and a
                // drone with a rotor hit (`tellsbefore`: the same, whole).
                let mut first = None;
                for (k, (d, x)) in [(24.0f32, -6.0f32), (30.0, 0.0), (36.0, 6.0)]
                    .into_iter()
                    .enumerate()
                {
                    let p = eye + flat * d + right * x;
                    let at = zone.on_ground(p.x, p.z);
                    let w = spawn_walker(&mut commands, &zone, at, job(at));
                    commands.entity(w).insert(TellCase(k as u8));
                    first.get_or_insert(w);
                }
                let d = eye + flat * 26.0 + right * 2.5 + Vec3::Y * 4.0;
                let drone = spawn_drone(&mut commands, d, job(d));
                commands
                    .entity(drone)
                    .remove::<Senses>()
                    .remove::<Brain>()
                    .insert(TellCase(3));
                first.unwrap()
            }
            "dying" => {
                // At 30 m: a walker on a bad leg limping across, and one
                // facing you whose eye is shot out once the frames run.
                let mut first = None;
                for (k, (d, x)) in [(28.0f32, -6.0f32), (32.0, 3.0)].into_iter().enumerate() {
                    let p = eye + flat * d + right * x;
                    let at = zone.on_ground(p.x, p.z);
                    let w = spawn_walker(&mut commands, &zone, at, job(at));
                    // (Blind to the camera: they keep to the reel's script.)
                    commands
                        .entity(w)
                        .remove::<Senses>()
                        .remove::<Brain>()
                        .insert(TellCase(k as u8));
                    first.get_or_insert(w);
                }
                first.unwrap()
            }
            "rubble" => {
                // A heap of broken blocks across its way (Destruction's
                // debris layer): do its feet stand on them?
                use avian3d::prelude::*;
                let mid = eye + flat * 6.0;
                for (k, (dx, dz, w, h, d)) in [
                    (0.0f32, 0.0f32, 1.4f32, 0.3f32, 1.0f32),
                    (1.2, 0.3, 0.6, 0.18, 0.7),
                    (-1.0, -0.2, 0.5, 0.45, 0.5),
                ]
                .into_iter()
                .enumerate()
                {
                    let c = mid + right * dx + flat * dz;
                    let y = zone.ground(c.x, c.z);
                    commands.spawn((
                        Name::new("Reel rubble"),
                        RigidBody::Static,
                        Collider::cuboid(w, h, d),
                        CollisionLayers::new(
                            crate::world::Layer::Debris,
                            [crate::world::Layer::World],
                        ),
                        Mesh3d(meshes.add(Cuboid::new(w, h, d))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.42, 0.4, 0.37),
                            perceptual_roughness: 0.95,
                            ..default()
                        })),
                        Transform::from_xyz(c.x, y + h * 0.5, c.z)
                            .with_rotation(Quat::from_rotation_y(0.3 * k as f32)),
                    ));
                }
                spawn_walker(&mut commands, &zone, walk_start, job(walk_start))
            }
            _ => spawn_walker(&mut commands, &zone, walk_start, job(walk_start)),
        };
        // Deaf and blind: it's here to be looked at, not to hunt.
        commands.entity(e).remove::<Senses>();
        if what.starts_with("drone") {
            // No brain: the reel flies it (below).
            commands.entity(e).remove::<Brain>();
        } else {
            // At work already: it stands where it is and follows the path
            // given below.
            commands.entity(e).insert(Brain::Working {
                at: walk_start,
                left: 1.0e6,
            });
        }
        state.subject = Some(e);
        state.kill_at = match what {
            "wreck" | "dronewreck" => Some(WARMUP + 6),
            _ => None,
        };
        return;
    }
    let Some(subject) = state.subject else {
        return;
    };
    // A drone flies a figure of eight in front of you, banking through
    // the turns, nose leading.
    if let Ok(mut d) = drones.get_mut(subject) {
        let k = frame as f32 / 30.0 * std::f32::consts::TAU / 7.0;
        let centre = eye + flat * 8.0 + Vec3::Y * 0.8;
        d.goal = centre + right * (k.sin() * 4.5) + flat * ((2.0 * k).sin() * 2.0);
        if what == "dronewreck" {
            d.goal = centre;
        }
    }
    for (lvl, mut aware) in &mut levels {
        let (sus, sees) = [(0.0, false), (0.6, false), (1.0, true)][lvl.0 as usize];
        aware.suspicion = sus;
        aware.can_see_target = sees;
        aware.exposure = if sees { 1.0 } else { 0.0 };
    }
    if (what == "eyes" || what == "eyesfar" || what == "dying") && frame == 5 {
        // Turned to face you.
        for mut w in &mut walkers {
            w.yaw = flat.x.atan2(flat.z);
        }
    }
    if frame == 5 {
        if let Ok(mut w) = walkers.get_mut(subject) {
            if what == "zap" || what == "shot" {
                // Facing you.
                w.yaw = flat.x.atan2(flat.z);
            }
            let p = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
            w.path = match what {
                "turn" => vec![
                    p(eye + flat * 4.5 - right * 3.5),
                    p(eye + flat * 4.5 + right * 5.0),
                ],
                "wreck" | "zap" | "shot" | "eyesfar" | "knockdown" => vec![],
                "hound" => vec![p(eye + flat * 6.0 + right * 14.0)],
                "damaged" => vec![p(eye + flat * 5.0 + right * 3.0)],
                // The lame one walks across, so it limps.
                "tells" | "tellsbefore" => vec![p(eye + flat * 24.0 + right * 6.0)],
                "dying" => vec![p(eye + flat * 28.0 + right * 8.0)],
                _ => vec![p(eye + flat * 6.0 + right * 8.0)],
            };
        }
        for (e, mut p, gt) in &mut parts {
            let shot_off = match what {
                "crawl" => matches!(p.role, PartRole::Leg(0) | PartRole::Leg(1)),
                _ => p.role == PartRole::Leg(1),
            };
            if p.robot != subject || !shot_off {
                continue;
            }
            let role = p.role;
            match what {
                "limp" | "crawl" => {
                    p.hp = 0.0;
                    destroyed.write(PartDestroyed {
                        robot: subject,
                        part: e,
                        role,
                        at: gt.translation(),
                    });
                }
                "hurt" => p.hp = p.max_hp * 0.2,
                _ => {}
            }
        }
    }
    if state.kill_at == Some(frame) {
        if let Some((e, mut p, gt)) = parts
            .iter_mut()
            .find(|(_, p, _)| p.robot == subject && p.role == PartRole::Core)
        {
            p.hp = -1.0;
            if let Ok(mut r) = robots.get_mut(subject) {
                r.wrecked = true;
            }
            destroyed.write(PartDestroyed {
                robot: subject,
                part: e,
                role: PartRole::Core,
                at: gt.translation(),
            });
        }
    }
    // Rounds into a walker from beside the camera, a few a second: the
    // marks they leave, and an eye shot that stutters.
    if what == "shot" && frame > 10 && frame % 4 == 0 {
        let aim = parts
            .iter()
            .filter(|(_, p, _)| p.robot == subject)
            .map(|(_, p, gt)| (p.role, gt.translation()))
            .find(|(r, _)| {
                if (frame / 4) % 5 == 0 {
                    *r == PartRole::Sensor
                } else {
                    *r == PartRole::Core
                }
            })
            .map(|(_, p)| p);
        if let Some(aim) = aim {
            let k = frame as f32 * 0.7;
            let target = aim + Vec3::new(k.sin() * 0.2, k.cos() * 0.15, 0.0);
            // (Clear of the player's own body, which stands at the camera.)
            let origin = eye + flat * 0.9 - Vec3::Y * 0.3;
            shots.write(crate::weapons::ballistics::Shot {
                // (Not the walker: a round ignores its own gun's parts.)
                gun: Entity::PLACEHOLDER,
                shooter: None,
                origin,
                direction: (target - origin).normalize_or(Vec3::NEG_Z),
                speed: 700.0,
                damage: 7.0,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: frame as u64,
            });
        }
    }
    // The damage for `tells`, once they're up: by case, the leg, the eye,
    // the core, the drone's rotor.
    // `dying`: the leg hurt from the start; the eye shot out a few frames
    // into the reel, so it catches the eye's last flicker.
    if what == "dying" && (frame == 5 || frame == WARMUP + 5) {
        for (e, mut p, gt) in &mut parts {
            let Ok(case) = cases.get(p.robot) else {
                continue;
            };
            match (case.0, p.role, frame == 5) {
                (0, PartRole::Leg(1), true) => p.hp = p.max_hp * 0.3,
                (1, PartRole::Sensor, false) => {
                    p.hp = 0.0;
                    destroyed.write(PartDestroyed {
                        robot: p.robot,
                        part: e,
                        role: p.role,
                        at: gt.translation(),
                    });
                }
                _ => {}
            }
        }
    }
    if what == "tells" && frame == 5 {
        for (e, mut p, gt) in &mut parts {
            let Ok(case) = cases.get(p.robot) else {
                continue;
            };
            let hurt = match (case.0, p.role) {
                (0, PartRole::Leg(1)) => Some(0.3),
                (1, PartRole::Sensor) => Some(0.0),
                (2, PartRole::Core) => Some(0.2),
                (3, PartRole::Rotor(1)) => Some(0.15),
                _ => None,
            };
            if let Some(h) = hurt {
                p.hp = p.max_hp * h;
                if h == 0.0 {
                    destroyed.write(PartDestroyed {
                        robot: p.robot,
                        part: e,
                        role: p.role,
                        at: gt.translation(),
                    });
                }
            }
        }
    }
    // Shot about but going: plates on their last bolt, the core at a fifth
    // (smoke, sparks, a guttering eye).
    if what == "damaged" && frame == 5 {
        for (_, mut p, _) in &mut parts {
            if p.robot != subject {
                continue;
            }
            match p.role {
                PartRole::Armour(_) => p.hp = p.max_hp * 0.3,
                PartRole::Core => p.hp = p.max_hp * 0.2,
                _ => {}
            }
        }
    }
    // One heavy round into its side: it goes down, lies a moment, gets up.
    // (Its core is made to take it: this is about the knock.)
    if what == "knockdown" && frame == 12 {
        let core = parts
            .iter_mut()
            .find(|(_, p, _)| p.robot == subject && p.role == PartRole::Core);
        if let Some((_, mut p, gt)) = core {
            p.hp = 1.0e6;
            p.max_hp = 1.0e6;
            let target = gt.translation();
            let origin = target - right * 3.0;
            shots.write(crate::weapons::ballistics::Shot {
                gun: Entity::PLACEHOLDER,
                shooter: None,
                origin,
                direction: (target - origin).normalize_or(Vec3::X),
                speed: 700.0,
                damage: 90.0,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: 1,
            });
        }
    }
    // A crawler zapping at a point beside the camera: charge, then the arc.
    if what == "zap" {
        let emitter = parts
            .iter()
            .find(|(_, p, _)| p.robot == subject && p.role == PartRole::Weapon)
            .map(|(_, _, gt)| gt.translation());
        if let Some(from) = emitter {
            let k = frame.saturating_sub(WARMUP - 6) % 30;
            if k == 0 {
                charges.write(super::indoor::CrawlerCharge {
                    crawler: subject,
                    at: from,
                });
            }
            if k == 15 {
                zaps.write(super::indoor::CrawlerZap {
                    crawler: subject,
                    from,
                    to: eye - Vec3::Y * 0.5 + right * 0.9,
                });
            }
        }
    }
    if frame < WARMUP || (frame - WARMUP) % state.every != 0 || state.taken >= state.shots {
        return;
    }
    let stem = out
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "reel".into());
    let dir = out.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{stem}_{:02}.png", state.taken));
    state.taken += 1;
    let last = state.taken == state.shots;
    let mut save = save_to_disk(path);
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            save(captured);
            if last {
                exit.write(AppExit::Success);
            }
        },
    );
}

/// How far the ground between `eye` and `to` rises into the line of
/// sight, at its worst (m; negative: it's clear by that much).
fn hidden(zone: &Zone, eye: Vec3, to: Vec3) -> f32 {
    (1..40)
        .map(|i| {
            let p = eye.lerp(to, i as f32 / 40.0);
            zone.ground(p.x, p.z) - (p.y - 0.2)
        })
        .fold(f32::MIN, f32::max)
}
