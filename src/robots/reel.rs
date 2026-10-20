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
//! you), `limp` (a leg shot off, then across), `hurt` (a leg at a fifth),

//! `sorter` (the Depot's gantry at its sorting, in the open), `hound` (a
//! hound running across), `crawler` (a crawler across), `eyes` (calm, suspicious and seeing walkers and a drone, for judging eyes
//! and lights at `--hour dusk` or `night`), `shot` (rounds into a walker: the marks they leave), `zap` (a crawler charging and
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
            "wreck" | "shot" => {
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
    if what == "eyes" && frame == 5 {
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
                "wreck" | "zap" | "shot" => vec![],
                "hound" => vec![p(eye + flat * 6.0 + right * 14.0)],
                _ => vec![p(eye + flat * 6.0 + right * 8.0)],
            };
        }
        for (e, mut p, gt) in &mut parts {
            if p.robot != subject || p.role != PartRole::Leg(1) {
                continue;
            }
            match what {
                "limp" => {
                    p.hp = 0.0;
                    destroyed.write(PartDestroyed {
                        robot: subject,
                        part: e,
                        role: PartRole::Leg(1),
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
