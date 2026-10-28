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
}

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
    };
    let t0 = app.world().resource::<Time>().elapsed_secs();
    let mut lock: Option<f32> = None;
    for _ in 0..((RUN_TIME + 10.0) * HZ) as usize {
        let now = app.world().resource::<Time>().elapsed_secs();
        if lock.is_none() {
            lock = app.world().resource::<Locked>().0;
        }
        let since = lock.map(|l| now - l);
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
            if s > RUN_TIME {
                break;
            }
        } else if now - t0 > 15.0 {
            break;
        }
    }
    app.world_mut().despawn(drone);
    for _ in 0..3 {
        app.update();
    }
    out
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
        for go in [Go::Stand, Go::Roof, Go::Across] {
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
    for v in VALLEYS {
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
                    "first_hit": r.first_hit, "down": r.down, "in_at": r.in_at, "hp": r.hp})
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
