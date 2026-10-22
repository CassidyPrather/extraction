//! The squad drill: does a newcomer who listens get away from machines
//! that work together, where one who doesn't listen is caught?
//!
//! A newcomer with a radio in its pack is out in the south of Marrow
//! Creek, making for the nearest rendezvous at a jog, when a squad finds
//! it: three armed walkers, or two and a drone, all on it at once. It
//! escapes if it gets to a rendezvous before it goes down.
//!
//! A **listening** newcomer acts only on what it would hear: the squad's
//! calls about it (a chirp within `squad::CHIRP_REACH`, its radio within
//! `squad::RADIO_REACH`), a moment after each.
//!
//! - **Cut off**: it makes for another rendezvous, one the cut-off isn't
//!   on the way to.
//! - **Flank**: it breaks the other way for a few seconds, then goes on.
//! - **Spot**: it keeps off the lit ground.
//!
//! A **deaf** one is the same newcomer, hearing nothing. Each runs with
//! the machines coordinating, and not (`squad::Coordination`).
//!
//! `cargo test --lib squad_drill -- --ignored` runs the batch
//! ([`SEEDS`] seeds, more with `SIM_DRILL_SEEDS`), writing one JSON line
//! a drill to `SIM_DRILL_OUT` (default `target/squad_drill.jsonl`); the
//! plain test runs one of each.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::noise::Rng;

use super::squad::{CHIRP_REACH, Coordination, RADIO_REACH, Role, SquadCall};
use super::{Awareness, Brain, Job, JobKind};
use crate::rig::{LocalPlayer, RigInput};
use crate::world::Zone;

/// Seeds in a batch, unless `SIM_DRILL_SEEDS` says.
pub const SEEDS: u64 = 24;
/// The newcomer's pace under fire (m/s): a jog.
pub const PACE: f32 = 3.0;
/// It's a moment after a call before it acts on it (s).
pub const REACT: f32 = 1.5;
/// Breaking away from a flanker, or off lit ground, lasts this long (s)...
pub const BREAK_FOR: f32 = 6.0;
/// ...this far off its way (radians).
pub const BREAK_ANGLE: f32 = 0.9;
/// Within this of a rendezvous it's out (m).
pub const OUT_AT: f32 = 12.0;
/// A drill ends after this long, whatever (s).
pub const DRILL_TIME: f32 = 200.0;
/// The squad finds it from this far (m).
pub const SPAWN: (f32, f32) = (45.0, 55.0);
/// The updates in a second.
const HZ: f32 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arm {
    pub listen: bool,
    pub coordinate: bool,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub seed: u64,
    pub arm: Arm,
    pub escaped: bool,
    pub down: bool,
    pub secs: f32,
    pub hp: f32,
    /// The squad's calls it acted on.
    pub heeded: u32,
    /// The calls made about it.
    pub calls: u32,
    pub squad: &'static str,
    pub rv: String,
}

#[derive(Resource, Default)]
struct Heard(Vec<(SquadCall, f32)>);

fn hear(mut calls: MessageReader<SquadCall>, mut h: ResMut<Heard>, time: Res<Time>) {
    let now = time.elapsed_secs();
    h.0.extend(calls.read().map(|c| (*c, now)));
}

struct Newcomer {
    goal: usize,
    path: Vec<Vec2>,
    /// Breaking away: toward here, until then.
    away: Option<(Vec2, f32)>,
    /// Lit ground to keep off.
    lit: Option<Vec2>,
}

fn rendezvous(zone: &Zone) -> Vec<(String, Vec2)> {
    zone.map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| (m.name.clone(), Vec2::from(m.at)))
        .collect()
}

fn route(app: &App, from: Vec2, to: Vec2) -> Vec<Vec2> {
    let nav = app.world().resource::<super::walker::Nav>();
    nav.0
        .path(from.to_array(), to.to_array())
        .map(|p| p.into_iter().map(Vec2::from_array).collect())
        .unwrap_or_else(|| vec![to])
}

/// One drill.
pub fn drill(seed: u64, arm: Arm) -> Outcome {
    let mut app = crate::headless_app(
        crate::LaunchOptions {
            robots: false,
            seed,
            ..Default::default()
        },
        false,
    );
    app.init_resource::<Heard>().add_systems(Last, hear);
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
    app.world_mut().resource_mut::<Coordination>().on = arm.coordinate;
    let mut rng = Rng::new(seed.wrapping_mul(0x9E37_79B9) ^ 0x5D);
    let rvs = rendezvous(&zone);
    let nav_near = |app: &App, p: Vec2| {
        let nav = app.world().resource::<super::walker::Nav>();
        nav.0
            .nearest_walkable(p.to_array())
            .map_or(p, Vec2::from_array)
    };
    let start = nav_near(
        &app,
        Vec2::new(
            120.0 + rng.range(-15.0, 15.0),
            -185.0 + rng.range(-15.0, 15.0),
        ),
    );
    let ground = |p: Vec2| zone.on_ground(p.x, p.y);
    let put = |app: &mut App, p: Vec2| {
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = ground(p) + Vec3::Y * 0.05;
    };
    put(&mut app, start);
    // A radio in the pack on its back.
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let mut queue = CommandQueue::default();
    let radio = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind("radio").unwrap(),
                transform: Transform::from_translation(ground(start) + Vec3::Y),
                condition: 1.0,
            },
        )
    };
    queue.apply(app.world_mut());
    let bag = app
        .world_mut()
        .query::<(Entity, &crate::interact::body::Backpack)>()
        .iter(app.world())
        .find(|(_, b)| b.owner == player && b.carried)
        .map(|(e, _)| e)
        .expect("a pack");
    app.world_mut()
        .entity_mut(radio)
        .insert(crate::interact::body::InBackpack { bag });
    // The squad, from one side.
    let nearest = (0..rvs.len())
        .min_by(|&a, &b| {
            rvs[a]
                .1
                .distance(start)
                .total_cmp(&rvs[b].1.distance(start))
        })
        .unwrap();
    let side = rng.range(0.0, std::f32::consts::TAU);
    let drone = rng.below(2) == 0;
    let walkers = if drone { 2 } else { 3 };
    let mut squad: Vec<Entity> = vec![];
    let mut queue = CommandQueue::default();
    {
        let mut c = Commands::new(&mut queue, app.world());
        for k in 0..walkers {
            let a = side + (k as f32 - 1.0) * 0.25;
            let p = start + Vec2::from_angle(a) * rng.range(SPAWN.0, SPAWN.1);
            let at = ground(p);
            squad.push(super::walker::spawn_walker(
                &mut c,
                &zone,
                at,
                Job {
                    kind: JobKind::Watch,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            ));
        }
        if drone {
            let p = start + Vec2::from_angle(side) * SPAWN.0;
            squad.push(super::drone::spawn_drone(
                &mut c,
                ground(p) + Vec3::Y * 18.0,
                Job {
                    kind: JobKind::Survey,
                    site: ground(p),
                    duration: 999.0,
                    exit: ground(p),
                },
            ));
        }
    }
    queue.apply(app.world_mut());
    app.update();
    for e in &squad {
        let mut aw = app.world_mut().get_mut::<Awareness>(*e).unwrap();
        aw.target = Some(player);
        aw.last_known = Some(ground(start));
        aw.suspicion = 1.0;
        *app.world_mut().get_mut::<Brain>(*e).unwrap() = Brain::Engage;
    }
    let mut me = Newcomer {
        goal: nearest,
        path: route(&app, start, rvs[nearest].1),
        away: None,
        lit: None,
    };
    let mut here = start;
    let mut heeded = 0;
    let mut acted = 0usize;
    let dt = 1.0 / HZ;
    let mut t = 0.0;
    let (escaped, down) = loop {
        app.update();
        t += dt;
        if app
            .world()
            .get::<crate::health::Health>(player)
            .is_some_and(|h| h.down)
        {
            break (false, true);
        }
        if rvs.iter().any(|(_, at)| at.distance(here) < OUT_AT) {
            break (true, false);
        }
        if t > DRILL_TIME {
            break (false, false);
        }
        // What it heard, a moment ago.
        let now = app.world().resource::<Time>().elapsed_secs();
        let heard: Vec<SquadCall> = {
            let h = &app.world().resource::<Heard>().0;
            let ready: Vec<SquadCall> = h
                .iter()
                .skip(acted)
                .take_while(|(_, when)| now - when >= REACT)
                .map(|(c, _)| *c)
                .collect();
            acted += ready.len();
            ready
        };
        for call in heard.iter().filter(|c| c.target == player) {
            let d = call.at.xz().distance(here);
            if !arm.listen || d > RADIO_REACH.max(CHIRP_REACH) {
                continue;
            }
            let way =
                (me.path.first().copied().unwrap_or(rvs[me.goal].1) - here).normalize_or(Vec2::X);
            let post = call.post.xz();
            match call.role {
                Role::CutOff => {
                    // Another way out: one the cut-off isn't on the way to.
                    let shut = (post - here).normalize_or(Vec2::X);
                    let other = (0..rvs.len())
                        .filter(|&i| (rvs[i].1 - here).normalize_or(Vec2::X).dot(shut) < 0.5)
                        .min_by(|&a, &b| {
                            rvs[a].1.distance(here).total_cmp(&rvs[b].1.distance(here))
                        });
                    if let Some(i) = other {
                        me.goal = i;
                        me.path = route(&app, here, rvs[i].1);
                        heeded += 1;
                    }
                }
                Role::Flank => {
                    let to = post - here;
                    let left = way.perp_dot(to) > 0.0;
                    let turn = if left { -BREAK_ANGLE } else { BREAK_ANGLE };
                    let dir = Vec2::from_angle(turn).rotate(way);
                    me.away = Some((here + dir * PACE * BREAK_FOR, t + BREAK_FOR));
                    heeded += 1;
                }
                Role::Spot => {
                    me.lit = Some(post);
                    heeded += 1;
                }
                Role::Pin | Role::Call => {}
            }
        }
        // Where to now.
        if me.away.is_some_and(|(_, until)| t > until) {
            me.away = None;
            me.path = route(&app, here, rvs[me.goal].1);
        }
        while me.path.len() > 1 && me.path[0].distance(here) < 1.5 {
            me.path.remove(0);
        }
        let mut aim = match me.away {
            Some((p, _)) => p,
            None => me.path.first().copied().unwrap_or(rvs[me.goal].1),
        };
        // Keep off lit ground: round it, not through it.
        if let Some(lit) = me.lit {
            let to = aim - here;
            let off = lit - here;
            if off.length() < 25.0 && to.normalize_or_zero().dot(off.normalize_or_zero()) > 0.7 {
                aim = here + Vec2::from_angle(BREAK_ANGLE).rotate(to);
            }
        }
        let step = (aim - here).clamp_length_max(PACE * dt);
        let next = here + step;
        let walkable = app
            .world()
            .resource::<super::walker::Nav>()
            .0
            .walkable(next.to_array());
        if walkable {
            here = next;
        } else {
            // Into a wall: back on the way.
            me.away = None;
            me.path = route(&app, here, rvs[me.goal].1);
        }
        put(&mut app, here);
    };
    let hp = app
        .world()
        .get::<crate::health::Health>(player)
        .map_or(0.0, |h| h.hp);
    let calls = app
        .world()
        .resource::<Heard>()
        .0
        .iter()
        .filter(|(c, _)| c.target == player)
        .count() as u32;
    Outcome {
        seed,
        arm,
        escaped,
        down,
        secs: t,
        hp,
        heeded,
        calls,
        squad: if drone {
            "2 walkers + drone"
        } else {
            "3 walkers"
        },
        rv: rvs[me.goal].0.clone(),
    }
}

fn line(o: &Outcome) -> String {
    serde_json::json!({
        "seed": o.seed,
        "listen": o.arm.listen,
        "coordinate": o.arm.coordinate,
        "escaped": o.escaped,
        "down": o.down,
        "secs": (o.secs * 10.0).round() / 10.0,
        "hp": o.hp.round(),
        "heeded": o.heeded,
        "calls": o.calls,
        "squad": o.squad,
        "rv": o.rv,
    })
    .to_string()
}

fn batch(arm: Arm) {
    use std::io::Write;
    let seeds: u64 = std::env::var("SIM_DRILL_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(SEEDS);
    let out = std::env::var("SIM_DRILL_OUT").unwrap_or_else(|_| "target/squad_drill.jsonl".into());
    for seed in 1..=seeds {
        let o = drill(seed, arm);
        let l = line(&o);
        eprintln!("{l}");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&out)
            .unwrap();
        writeln!(f, "{l}").unwrap();
    }
}

#[test]
#[ignore]
fn squad_drill_listening_coordinated() {
    batch(Arm {
        listen: true,
        coordinate: true,
    });
}

#[test]
#[ignore]
fn squad_drill_deaf_coordinated() {
    batch(Arm {
        listen: false,
        coordinate: true,
    });
}

#[test]
#[ignore]
fn squad_drill_listening_alone() {
    batch(Arm {
        listen: true,
        coordinate: false,
    });
}

#[test]
#[ignore]
fn squad_drill_deaf_alone() {
    batch(Arm {
        listen: false,
        coordinate: false,
    });
}

/// One drill runs to an end, and a listener hears the squad's calls and
/// acts on them (the batch measures how much that's worth).
#[test]
fn a_listening_newcomer_hears_the_squad() {
    let o = drill(
        3,
        Arm {
            listen: true,
            coordinate: true,
        },
    );
    eprintln!("{}", line(&o));
    assert!(o.calls >= 2, "{o:?}");
    assert!(o.heeded >= 1, "{o:?}");
    assert!(o.escaped || o.down || o.secs > DRILL_TIME, "{o:?}");
}
