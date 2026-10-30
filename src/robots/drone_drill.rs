//! The drone drill (playtest, 28 October, raid 2): is a first drone death
//! fair? A drone locks on to someone in the open; they either stand, or do
//! as the driver says ("Run across it, to cover") and sprint for the
//! nearest way in under a roof. Measured over each valley's open ground:
//! how far the nearest roof is, how long from the lock chirp to the first
//! hit and to going down, and whether a runner makes it in.
//!
//! `cargo test --lib drone_drill -- --ignored --nocapture`; a JSON line a
//! run to `SIM_DRILL_OUT` (default `target/drone_drill.jsonl`).

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use extraction_core::noise::Rng;

use super::{Awareness, Brain, Job, JobKind};
use crate::rig::{LocalPlayer, RigInput};
use crate::world::Zone;

/// The valleys.
pub const VALLEYS: [&str; 5] = [
    "marrow_creek",
    "cinder_works",
    "kettle_pass",
    "rime_fell",
    "sallow_reservoir",
];
/// Open ground: sampled this far apart (m)...
pub const GRID: f32 = 20.0;
/// ...at least this far from any building (m).
pub const OPEN: f32 = 8.0;
/// Runs a valley.
pub const RUNS: usize = 8;
/// The drone starts this far off you, this high (m): its standoff.
pub const FROM: (f32, f32) = (20.0, 10.0);
/// A run ends after this long from the lock (s).
pub const RUN_TIME: f32 = 30.0;
const HZ: f32 = 90.0;

/// A way in from outside: (door, a spot under its roof).
fn ways_in(e: &super::doors::Entrances) -> Vec<(Vec2, Vec2)> {
    e.doors
        .iter()
        .filter_map(|d| {
            let n = d.out.normalize_or(Vec3::Z);
            let inside = d.mid - n * 1.5;
            (e.building_at(inside) == Some(d.building))
                .then(|| (d.mid.xz(), (d.mid - n * 2.5).xz()))
        })
        .collect()
}

/// What they do at the lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Go {
    /// Stand where they are.
    Stand,
    /// Sprint for the nearest way in under a roof.
    Roof,
    /// Sprint across its line, and keep going (no roof).
    Across,
    /// Stand and shoot it with a pistol, as a person in a headset does
    /// ([`AIM_LAG`], [`AIM_JITTER`], [`SHOT_EVERY`]).
    Shoot,
}

/// The shooter aims where they saw it this long ago (s): eye to hand to
/// trigger, tracking a moving thing.
pub const AIM_LAG: f32 = 0.25;
/// ...give or take this (radians, one sigma each way): a pistol held out
/// in a headset.
pub const AIM_JITTER: f32 = 0.021;
/// An aimed shot this often (s)...
pub const SHOT_EVERY: f32 = 0.45;
/// ...eight to a magazine, this long to change it (s), three magazines.
pub const MAG: u32 = 8;
pub const RELOAD: f32 = 2.5;
pub const MAGS: u32 = 3;
/// The pistol's round (the PM's: `items.ron`).
const ROUND_SPEED: f32 = 315.0;
const ROUND_DAMAGE: f32 = 34.0;

#[derive(Clone, Debug)]
pub struct Run {
    pub valley: &'static str,
    pub go: Go,
    /// To the nearest way in (m).
    pub roof: f32,
    /// From the lock chirp (s).
    pub first_hit: Option<f32>,
    pub down: Option<f32>,
    /// In under the roof, before it was down (s).
    pub in_at: Option<f32>,
    pub hp: f32,
    /// Shooting it: rounds fired, and when it was brought down (s, from
    /// when it came at you).
    pub shots: u32,
    pub killed: Option<f32>,
    /// What the rounds did to its parts (hp).
    pub dealt: f32,
    /// Out of the fight: wrecked, a rotor gone (it sinks) or its gun gone
    /// (s, from when it came at you).
    pub crippled: Option<f32>,
    /// When it locked on, from when it came at you (s).
    pub locked_at: Option<f32>,
}

/// One normal draw.
fn normal(rng: &mut Rng) -> f32 {
    let u = rng.range(1e-6, 1.0);
    let v = rng.range(0.0, std::f32::consts::TAU);
    (-2.0 * u.ln()).sqrt() * v.cos()
}

/// Every open-ground spot in a valley, and how far its nearest roof is.
pub fn open_ground(app: &App, zone: &Zone) -> Vec<(Vec2, f32)> {
    let e = app.world().resource::<super::doors::Entrances>();
    let nav = &app.world().resource::<super::walker::Nav>().0;
    let ways = ways_in(e);
    let half = zone.map.size * 0.5 - 20.0;
    let n = (2.0 * half / GRID) as i32;
    let mut out = vec![];
    for i in 0..=n {
        for j in 0..=n {
            let p = Vec2::new(-half + i as f32 * GRID, -half + j as f32 * GRID);
            if !nav.walkable(p.to_array()) {
                continue;
            }
            let near_building = e.shells.iter().any(|s| {
                let l = s.xf.compute_affine().inverse().transform_point3(Vec3::new(
                    p.x,
                    s.xf.translation.y,
                    p.y,
                ));
                l.x.abs() < s.half.x + OPEN && l.z.abs() < s.half.y + OPEN
            });
            if near_building {
                continue;
            }
            let roof = ways
                .iter()
                .map(|(door, _)| door.distance(p))
                .fold(f32::MAX, f32::min);
            out.push((p, roof));
        }
    }
    out
}

fn valley_app(valley: &str) -> (App, Entity, Zone) {
    let mut app = crate::headless_app(
        crate::LaunchOptions {
            robots: false,
            map: valley.into(),
            ..Default::default()
        },
        false,
    );
    app.finish();
    app.cleanup();
    for _ in 0..3 {
        app.update();
    }
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(app.world())
        .next()
        .unwrap();
    app.world_mut()
        .get_mut::<RigInput>(player)
        .unwrap()
        .head
        .translation
        .y = 1.65;
    let zone = app.world().resource::<Zone>().clone();
    app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
    (app, player, zone)
}

fn put(app: &mut App, player: Entity, at: Vec3, v: Vec2) {
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = at + Vec3::Y * 0.05;
    if let Some(mut b) = app.world_mut().get_mut::<crate::player::Body>(player) {
        b.velocity.x = v.x;
        b.velocity.z = v.y;
    }
}

#[derive(Resource, Default)]
struct Locked(Option<f32>);

fn locked(mut l: MessageReader<super::drone::DroneLock>, time: Res<Time>, mut at: ResMut<Locked>) {
    if l.read().next().is_some() && at.0.is_none() {
        at.0 = Some(time.elapsed_secs());
    }
}

/// One drone on someone at `p`, its nearest roof `roof` away (under it,
/// `inside`), doing `go` at the lock.
#[allow(clippy::too_many_arguments)]
fn run(
    app: &mut App,
    player: Entity,
    zone: &Zone,
    valley: &'static str,
    p: Vec2,
    roof: f32,
    inside: Vec2,
    go: Go,
    rng: &mut Rng,
) -> Run {
    {
        let mut h = app
            .world_mut()
            .get_mut::<crate::health::Health>(player)
            .unwrap();
        h.hp = h.max;
        h.down = false;
        h.bleeding = 0.0;
    }
    app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
    app.world_mut().resource_mut::<Locked>().0 = None;
    let mut at = zone.on_ground(p.x, p.y);
    put(app, player, at, Vec2::ZERO);
    // From a side away from the roof (it came from somewhere).
    let a = rng.range(0.0, std::f32::consts::TAU);
    let from = at + Vec3::new(a.cos(), 0.0, a.sin()) * FROM.0 + Vec3::Y * FROM.1;
    let mut queue = CommandQueue::default();
    let drone = {
        let mut c = Commands::new(&mut queue, app.world());
        super::drone::spawn_drone(
            &mut c,
            from,
            Job {
                kind: JobKind::Response,
                site: at,
                duration: 999.0,
                exit: from,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    {
        let mut aw = app.world_mut().get_mut::<Awareness>(drone).unwrap();
        aw.target = Some(player);
        aw.last_known = Some(at);
        aw.suspicion = 1.0;
    }
    *app.world_mut().get_mut::<Brain>(drone).unwrap() = Brain::Engage;
    let mut out = Run {
        valley,
        go,
        roof,
        first_hit: None,
        down: None,
        in_at: None,
        hp: 100.0,
        shots: 0,
        killed: None,
        dealt: 0.0,
        crippled: None,
        locked_at: None,
    };
    // Where it was, for the shooter's lag.
    let mut seen: std::collections::VecDeque<(f32, Vec3)> = Default::default();
    let mut next_shot = 1.0;
    let mut in_mag = MAG;
    let t0 = app.world().resource::<Time>().elapsed_secs();
    let mut lock: Option<f32> = None;
    for _ in 0..((RUN_TIME + 10.0) * HZ) as usize {
        let now = app.world().resource::<Time>().elapsed_secs();
        if lock.is_none() {
            lock = app.world().resource::<Locked>().0;
        }
        let since = lock.map(|l| now - l);
        if out.locked_at.is_none() {
            out.locked_at = lock.map(|l| l - t0);
        }
        if out.crippled.is_none() {
            let mut parts = app.world_mut().query::<&super::RobotPart>();
            let out_of_it = app
                .world()
                .get::<super::Robot>(drone)
                .is_none_or(|r| r.wrecked)
                || parts.iter(app.world()).any(|p| {
                    p.robot == drone
                        && !p.alive()
                        && matches!(p.role, super::PartRole::Rotor(_) | super::PartRole::Weapon)
                });
            if out_of_it {
                out.crippled = Some(now - t0);
            }
        }
        // Shooting it.
        if go == Go::Shoot && out.killed.is_none() && out.crippled.is_none() {
            let them = app
                .world()
                .get::<GlobalTransform>(drone)
                .map(|g| g.translation());
            let wrecked = app
                .world()
                .get::<super::Robot>(drone)
                .is_none_or(|r| r.wrecked);
            if wrecked {
                out.killed = Some(now - t0);
            } else if let Some(them) = them {
                seen.push_back((now, them));
                while seen.len() > 2 && seen[1].0 <= now - AIM_LAG {
                    seen.pop_front();
                }
                let aim = seen.front().map_or(them, |s| s.1);
                let eye = at + Vec3::Y * 1.6;
                // Looking at it, down the sights (the drone sees that).
                if let Some(mut r) = app.world_mut().get_mut::<RigInput>(player) {
                    let to = (aim - eye).normalize_or(Vec3::NEG_Z);
                    r.head.rotation = Transform::IDENTITY.looking_to(to, Vec3::Y).rotation;
                }
                let left = MAG * MAGS - out.shots;
                if now - t0 >= next_shot && left > 0 && eye.distance(aim) < 50.0 {
                    let dir = (aim - eye).normalize_or(Vec3::NEG_Z);
                    let right = dir.cross(Vec3::Y).normalize_or(Vec3::X);
                    let up = right.cross(dir);
                    let dir = (dir
                        + right * (normal(rng) * AIM_JITTER).tan()
                        + up * (normal(rng) * AIM_JITTER).tan())
                    .normalize();
                    app.world_mut()
                        .write_message(crate::weapons::ballistics::Shot {
                            gun: player,
                            shooter: Some(player),
                            origin: eye + dir * 0.4,
                            direction: dir,
                            speed: ROUND_SPEED,
                            damage: ROUND_DAMAGE,
                            projectiles: 1,
                            spread: 0.0,
                            recoil: 0.0,
                            seed: rng.next_u64(),
                        });
                    out.shots += 1;
                    in_mag -= 1;
                    next_shot += SHOT_EVERY;
                    if in_mag == 0 {
                        in_mag = MAG;
                        next_shot += RELOAD;
                    }
                }
            }
        }
        // The driver's word comes at the lock; a beat to take it in.
        let going = go != Go::Stand && since.is_some_and(|s| s > 0.5) && out.in_at.is_none();
        let mut v = Vec2::ZERO;
        if going {
            let to = match go {
                Go::Roof => inside - at.xz(),
                // Square to the line from it to you, to and fro on the
                // open ground there (15 m each way).
                _ => {
                    let across = (p - from.xz()).perp().normalize_or(Vec2::X);
                    let leg = (since.unwrap_or(0.0) / (15.0 / crate::player::SPRINT_SPEED)) as i32;
                    let end = p + across * if leg % 2 == 0 { 15.0 } else { -15.0 };
                    let to = end - at.xz();
                    if to.length() < 0.5 { across } else { to }
                }
            };
            if to.length() < 0.5 {
                out.in_at = since;
            } else {
                v = to.normalize() * crate::player::SPRINT_SPEED;
                let step = (v / HZ).clamp_length_max(to.length());
                at = zone.on_ground(at.x + step.x, at.z + step.y);
            }
        }
        put(app, player, at, v);
        app.update();
        let h = app.world().get::<crate::health::Health>(player).unwrap();
        out.hp = h.hp;
        if let Some(s) = since {
            if out.first_hit.is_none() && h.hp < h.max {
                out.first_hit = Some(s);
            }
            if h.down || h.hp <= 0.0 {
                out.down = Some(s);
                break;
            }
            if s > RUN_TIME || out.killed.is_some() || out.crippled.is_some() {
                break;
            }
        } else if now - t0 > 15.0 || out.killed.is_some() || out.crippled.is_some() {
            break;
        }
    }
    let mut parts = app.world_mut().query::<&super::RobotPart>();
    out.dealt = parts
        .iter(app.world())
        .filter(|p| p.robot == drone)
        .map(|p| p.max_hp - p.hp.max(0.0))
        .sum();
    app.world_mut().despawn(drone);
    for _ in 0..3 {
        app.update();
    }
    out
}

/// The arms a batch runs (`SIM_DRONE_ARMS`, e.g. "Stand,Shoot"; all by
/// default).
fn arms() -> Vec<Go> {
    let all = [Go::Stand, Go::Roof, Go::Across, Go::Shoot];
    match std::env::var("SIM_DRONE_ARMS") {
        Ok(v) => all
            .into_iter()
            .filter(|g| v.split(',').any(|a| a.trim() == format!("{g:?}")))
            .collect(),
        Err(_) => all.to_vec(),
    }
}

pub fn drill(valley: &'static str, seed: u64) -> (Vec<(Vec2, f32)>, Vec<Run>) {
    let (mut app, player, zone) = valley_app(valley);
    app.init_resource::<Locked>().add_systems(Last, locked);
    let ground = open_ground(&app, &zone);
    let ways = ways_in(app.world().resource::<super::doors::Entrances>());
    let mut rng = Rng::new(seed ^ 0xD20E);
    let mut runs = vec![];
    for _ in 0..RUNS {
        let (p, roof) = ground[rng.below(ground.len())];
        let inside = ways
            .iter()
            .min_by(|a, b| a.0.distance(p).total_cmp(&b.0.distance(p)))
            .map(|w| w.1)
            .unwrap_or(p);
        let seed_rng = rng.next_u64();
        for go in arms() {
            let mut r = Rng::new(seed_rng);
            runs.push(run(
                &mut app, player, &zone, valley, p, roof, inside, go, &mut r,
            ));
        }
    }
    (ground, runs)
}

#[test]
#[ignore]
fn drone_drill() {
    use std::io::Write;
    let path = std::env::var("SIM_DRILL_OUT").unwrap_or_else(|_| "target/drone_drill.jsonl".into());
    let mut f = std::fs::File::create(&path).unwrap();
    let only = std::env::var("SIM_DRONE_VALLEYS").ok();
    for v in VALLEYS {
        if only.as_ref().is_some_and(|o| !o.split(',').any(|x| x == v)) {
            continue;
        }
        let (ground, runs) = drill(v, 1);
        let mut d: Vec<f32> = ground.iter().map(|g| g.1).collect();
        d.sort_by(f32::total_cmp);
        let q = |x: f32| d[((d.len() - 1) as f32 * x) as usize];
        let within: Vec<f32> = [10.0, 20.0, 30.0, 45.0, 60.0]
            .iter()
            .map(|r| d.iter().filter(|x| **x <= *r).count() as f32 / d.len() as f32)
            .collect();
        let line = serde_json::json!({"valley": v, "open": d.len(), "roof_p50": q(0.5),
            "roof_p90": q(0.9), "roof_max": q(1.0), "within_10_20_30_45_60": within});
        writeln!(f, "{line}").unwrap();
        println!("{line}");
        for r in runs {
            writeln!(
                f,
                "{}",
                serde_json::json!({"valley": r.valley, "go": format!("{:?}", r.go), "roof": r.roof,
                    "first_hit": r.first_hit, "down": r.down, "in_at": r.in_at, "hp": r.hp,
                    "shots": r.shots, "killed": r.killed, "dealt": r.dealt, "crippled": r.crippled, "locked_at": r.locked_at})
            )
            .unwrap();
            println!("{r:?}");
        }
    }
}

/// The driver's advice at a drone's lock works where it's given, the open:
/// stood still you're down inside the run; sprinting across its line,
/// with no roof to make for, you're still up.
#[test]
fn in_the_open_a_runner_outlasts_a_drone() {
    let (mut app, player, zone) = valley_app("marrow_creek");
    app.init_resource::<Locked>().add_systems(Last, locked);
    let ground = open_ground(&app, &zone);
    // Open ground with no roof near.
    // (All of the ground 15 m round it open too: room to run.)
    let open: Vec<Vec2> = ground.iter().map(|g| g.0).collect();
    let roomy = |q: Vec2| {
        [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y]
            .iter()
            .all(|d| open.iter().any(|o| o.distance(q + *d * GRID) < 1.0))
    };
    let (p, roof) = *ground
        .iter()
        .find(|g| g.1 > 60.0 && roomy(g.0))
        .expect("open ground far from a roof");
    let mut rng = Rng::new(7);
    let stood = run(
        &mut app,
        player,
        &zone,
        "marrow_creek",
        p,
        roof,
        p,
        Go::Stand,
        &mut rng,
    );
    let mut rng = Rng::new(7);
    let ran = run(
        &mut app,
        player,
        &zone,
        "marrow_creek",
        p,
        roof,
        p,
        Go::Across,
        &mut rng,
    );
    assert!(stood.first_hit.is_some(), "it fires: {stood:?}");
    assert!(stood.down.is_some(), "stood still, down: {stood:?}");
    assert!(ran.down.is_none(), "running across it, still up: {ran:?}");
}

/// The drill's rounds go where they're aimed: ten at a drone hovering
/// still, 20 m off, land (or the shooter's misses would be the model's,
/// not the drone's).
#[test]
fn the_drills_rounds_land_on_a_still_drone() {
    let (mut app, player, zone) = valley_app("marrow_creek");
    let at = zone.on_ground(40.0, 255.0);
    put(&mut app, player, at, Vec2::ZERO);
    let from = at + Vec3::new(0.0, 8.0, 20.0);
    let mut queue = CommandQueue::default();
    let drone = {
        let mut c = Commands::new(&mut queue, app.world());
        super::drone::spawn_drone(
            &mut c,
            from,
            Job {
                kind: JobKind::Watch,
                site: from,
                duration: 999.0,
                exit: from,
            },
        )
    };
    queue.apply(app.world_mut());
    for _ in 0..90 {
        app.update();
    }
    for i in 0..10 {
        let them = app
            .world()
            .get::<GlobalTransform>(drone)
            .unwrap()
            .translation();
        let eye = at + Vec3::Y * 1.6;
        let dir = (them - eye).normalize();
        app.world_mut()
            .write_message(crate::weapons::ballistics::Shot {
                gun: player,
                shooter: Some(player),
                origin: eye + dir * 0.4,
                direction: dir,
                speed: ROUND_SPEED,
                damage: ROUND_DAMAGE,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: i,
            });
        for _ in 0..20 {
            app.update();
        }
    }
    let mut parts = app.world_mut().query::<&super::RobotPart>();
    let dealt: f32 = parts
        .iter(app.world())
        .filter(|p| p.robot == drone)
        .map(|p| p.max_hp - p.hp.max(0.0))
        .sum();
    assert!(dealt >= ROUND_DAMAGE, "dealt {dealt}");
}

/// Playtest, 28 October, 1:05:24: "way too fast for somebody in VR to
/// possibly have a shot". With a pistol, as a person in a headset aims
/// ([`AIM_LAG`], [`AIM_JITTER`]), a drone on you in the open is brought out
/// of the fight in most fights (it holds still to lock and to fire; it
/// circles slowly enough to follow): in two of eight fights at least.
/// It was in none.
#[test]
fn a_pistol_in_a_headset_can_take_a_drone_out() {
    let (mut app, player, zone) = valley_app("marrow_creek");
    app.init_resource::<Locked>().add_systems(Last, locked);
    let ground = open_ground(&app, &zone);
    let mut rng = Rng::new(31);
    let mut out = 0;
    let mut runs = vec![];
    for _ in 0..8 {
        let (p, roof) = ground[rng.below(ground.len())];
        let mut r = Rng::new(rng.next_u64());
        let run = run(
            &mut app,
            player,
            &zone,
            "marrow_creek",
            p,
            roof,
            p,
            Go::Shoot,
            &mut r,
        );
        out += run.crippled.is_some() as u32;
        runs.push(run);
    }
    // (About half in the drill, in every valley; the old drone, none of
    // eight, and not a round that did it harm.)
    assert!(out >= 2, "out of the fight in {out} of 8: {runs:?}");
}
