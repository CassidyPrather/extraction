//! The hound drill: you can't outrun a hound, so the driver's word for one
//! is to face it and shoot when it coils (coordinator, after Rime's first
//! hound ran three walking newcomers down). Is that a fight a newcomer
//! wins? A hound comes at someone standing in the open with a PM, aiming
//! as a person in a headset does (`drone_drill`'s [`AIM_LAG`],
//! [`AIM_JITTER`], [`SHOT_EVERY`], magazines and reloads): who's out of it
//! first, the hound (wrecked, or a leg or its eye shot out) or them?
//!
//! `cargo test --lib hound_drill -- --ignored --nocapture`.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use extraction_core::noise::Rng;

use super::drone_drill::{
    AIM_JITTER, AIM_LAG, MAG, MAGS, RELOAD, SHOT_EVERY, VALLEYS, normal, open_ground, put,
    valley_app,
};
use super::{Awareness, Brain, Job, JobKind, PartRole};
use crate::world::Zone;

/// The hound starts this far off you (m): on its way to you, seen.
pub const FROM: f32 = 25.0;
/// Fights a valley.
pub const RUNS: usize = 8;
/// A fight ends after this long (s).
pub const FIGHT_TIME: f32 = 40.0;
const HZ: f32 = 90.0;
/// The PM's round (`items.ron`).
const ROUND_SPEED: f32 = 315.0;
const ROUND_DAMAGE: f32 = 34.0;
/// Within this they shoot (m): a pistol at a dog-sized thing coming.
const IN_RANGE: f32 = 25.0;

/// How they stand to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    /// Feet planted, still but for the gun: a hound, seeing movement more
    /// than shape, hardly makes them out.
    Still,
    /// Shuffling a step to and fro as they aim ([`SHUFFLE`]): a nervous
    /// newcomer, seen, so it comes in, coils and springs.
    Shuffling,
}
/// A shuffle's pace (m/s), over a metre each way: past what counts as
/// keeping still (0.3).
pub const SHUFFLE: f32 = 0.6;

#[derive(Clone, Debug)]
pub struct Fight {
    pub valley: &'static str,
    pub stance: Stance,
    /// When the hound was out of it (s): wrecked, or its core or a leg
    /// gone (it can't come at you).
    pub won: Option<f32>,
    /// When its eye went (s): blind, it still hunts by ear.
    pub blinded: Option<f32>,
    /// When they went down (s).
    pub lost: Option<f32>,
    pub shots: u32,
    pub hits: u32,
    /// Its coils, and its bites that hurt (s).
    pub coils: Vec<f32>,
    pub bites: Vec<f32>,
    pub hp: f32,
}

#[derive(Resource, Default)]
struct Coils(Vec<f32>);

fn coils(mut c: MessageReader<super::hound::HoundCrouch>, time: Res<Time>, mut out: ResMut<Coils>) {
    for _ in c.read() {
        out.0.push(time.elapsed_secs());
    }
}

fn out_of_it(app: &mut App, hound: Entity) -> bool {
    if app
        .world()
        .get::<super::Robot>(hound)
        .is_none_or(|r| r.wrecked)
    {
        return true;
    }
    let mut parts = app.world_mut().query::<&super::RobotPart>();
    parts.iter(app.world()).any(|p| {
        p.robot == hound && !p.alive() && matches!(p.role, PartRole::Core | PartRole::Leg(_))
    })
}

fn blind(app: &mut App, hound: Entity) -> bool {
    let mut parts = app.world_mut().query::<&super::RobotPart>();
    parts
        .iter(app.world())
        .any(|p| p.robot == hound && !p.alive() && p.role == PartRole::Sensor)
}

fn dealt(app: &mut App, hound: Entity) -> f32 {
    let mut parts = app.world_mut().query::<&super::RobotPart>();
    parts
        .iter(app.world())
        .filter(|p| p.robot == hound)
        .map(|p| p.max_hp - p.hp.max(0.0))
        .sum()
}

/// One hound on someone standing at `p` with a PM.
pub fn fight(
    app: &mut App,
    player: Entity,
    zone: &Zone,
    valley: &'static str,
    p: Vec2,
    stance: Stance,
    rng: &mut Rng,
) -> Fight {
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
    app.world_mut().resource_mut::<Coils>().0.clear();
    let home = zone.on_ground(p.x, p.y);
    let mut at = home;
    put(app, player, at, Vec2::ZERO);
    let a = rng.range(0.0, std::f32::consts::TAU);
    let from = zone.on_ground(at.x + a.cos() * FROM, at.z + a.sin() * FROM);
    let mut queue = CommandQueue::default();
    let hound = {
        let mut c = Commands::new(&mut queue, app.world());
        super::hound::spawn_hound(
            &mut c,
            zone,
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
        let mut aw = app.world_mut().get_mut::<Awareness>(hound).unwrap();
        aw.target = Some(player);
        aw.last_known = Some(at);
        aw.suspicion = 1.0;
    }
    *app.world_mut().get_mut::<Brain>(hound).unwrap() = Brain::Engage;
    let mut out = Fight {
        valley,
        stance,
        won: None,
        blinded: None,
        lost: None,
        shots: 0,
        hits: 0,
        coils: vec![],
        bites: vec![],
        hp: 100.0,
    };
    let mut seen: std::collections::VecDeque<(f32, Vec3)> = Default::default();
    let t0 = app.world().resource::<Time>().elapsed_secs();
    let mut next_shot = 0.6;
    let mut in_mag = MAG;
    let mut was_dealt = 0.0;
    let mut hp = 100.0;
    for _ in 0..(FIGHT_TIME * HZ) as usize {
        let now = app.world().resource::<Time>().elapsed_secs();
        let t = now - t0;
        if out_of_it(app, hound) {
            out.won = Some(t);
            break;
        }
        if out.blinded.is_none() && blind(app, hound) {
            out.blinded = Some(t);
        }
        // Where it is (its body), as they saw it a moment ago.
        let them = app
            .world()
            .get::<GlobalTransform>(hound)
            .map(|g| g.translation())
            .unwrap_or(from);
        seen.push_back((now, them));
        while seen.len() > 2 && seen[1].0 <= now - AIM_LAG {
            seen.pop_front();
        }
        let aim = seen.front().map_or(them, |s| s.1);
        let eye = at + Vec3::Y * 1.6;
        if let Some(mut r) = app.world_mut().get_mut::<crate::rig::RigInput>(player) {
            let to = (aim - eye).normalize_or(Vec3::NEG_Z);
            r.head.rotation = Transform::IDENTITY.looking_to(to, Vec3::Y).rotation;
        }
        let left = MAG * MAGS - out.shots;
        if t >= next_shot && left > 0 && eye.distance(aim) < IN_RANGE {
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
            next_shot = t + SHOT_EVERY;
            if in_mag == 0 {
                in_mag = MAG;
                next_shot += RELOAD;
            }
        }
        // Standing their ground: still, or shuffling a step to and fro
        // (square to the way it came).
        let v = match stance {
            Stance::Still => Vec2::ZERO,
            Stance::Shuffling => {
                let across = (home.xz() - from.xz()).perp().normalize_or(Vec2::X);
                let leg = (t / (1.2 / SHUFFLE)) as i32;
                across * if leg % 2 == 0 { SHUFFLE } else { -SHUFFLE }
            }
        };
        at = zone.on_ground(at.x + v.x / HZ, at.z + v.y / HZ);
        put(app, player, at, v);
        app.update();
        let d = dealt(app, hound);
        if d > was_dealt + 0.5 {
            out.hits += 1;
            was_dealt = d;
        }
        let h = app.world().get::<crate::health::Health>(player).unwrap();
        if h.hp < hp - 5.0 {
            out.bites.push(t);
        }
        hp = h.hp;
        out.hp = h.hp;
        if h.down || h.hp <= 0.0 {
            out.lost = Some(t);
            break;
        }
    }
    out.coils = app
        .world()
        .resource::<Coils>()
        .0
        .iter()
        .map(|c| c - t0)
        .collect();
    app.world_mut().despawn(hound);
    for _ in 0..3 {
        app.update();
    }
    out
}

/// A valley's fights, from open ground.
pub fn drill(valley: &'static str, seed: u64, runs: usize, stance: Stance) -> Vec<Fight> {
    let (mut app, player, zone) = valley_app(valley);
    app.init_resource::<Coils>().add_systems(Last, coils);
    let ground = open_ground(&app, &zone);
    let mut rng = Rng::new(seed);
    (0..runs)
        .map(|_| {
            let (p, _) = ground[rng.below(ground.len())];
            let mut r = Rng::new(rng.next_u64());
            fight(&mut app, player, &zone, valley, p, stance, &mut r)
        })
        .collect()
}

/// Every valley, every fight, and how it went.
#[test]
#[ignore]
fn hound_drill() {
    for stance in [Stance::Still, Stance::Shuffling] {
        let mut won = 0;
        let mut lost = 0;
        let mut all = 0;
        for v in VALLEYS {
            let f = drill(v, 41, RUNS, stance);
            let w = f.iter().filter(|f| f.won.is_some()).count();
            let l = f.iter().filter(|f| f.lost.is_some()).count();
            println!("HOUND {stance:?} {v}: won {w}, lost {l} of {}", f.len());
            for x in &f {
                println!("  {x:?}");
            }
            won += w;
            lost += l;
            all += f.len();
        }
        println!("HOUND {stance:?} all: won {won}, lost {lost} of {all}");
    }
}

/// The driver's word for a hound, face it and shoot when it coils: a
/// newcomer standing their ground with a PM, aiming as a person in a
/// headset does, comes out of it more often than not (coordinator).
#[test]
fn a_newcomer_standing_with_a_pm_beats_a_hound_more_often_than_not() {
    // Shuffling, so it sees them and comes in (still, it hardly does).
    let f = drill("marrow_creek", 41, RUNS, Stance::Shuffling);
    let won = f.iter().filter(|f| f.won.is_some()).count();
    assert!(won * 2 > f.len(), "won {won} of {}: {f:?}", f.len());
}
