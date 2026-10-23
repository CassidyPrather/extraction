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

use avian3d::prelude::*;
use bevy::ecs::system::SystemState;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::noise::Rng;

use super::squad::{CHIRP_REACH, Coordination, Role, SquadCall};
use super::{Awareness, Brain, Job, JobKind};
use crate::rig::{LocalPlayer, RigInput};
use crate::world::Zone;

/// Seeds in a batch, unless `SIM_DRILL_SEEDS` says.
pub const SEEDS: u64 = 24;
/// The newcomer's pace under fire (m/s): a jog.
pub const PACE: f32 = 3.0;
/// It's a moment after a call before it acts on it (s).
pub const REACT: f32 = 1.5;
/// Its pace between covers (m/s): a sprint.
pub const SPRINT: f32 = 4.5;
/// Its bounds, cover to cover, are at most this long (m)...
pub const BOUND: f32 = 18.0;
/// ...with this long in each, crouched (s).
pub const COVER_WAIT: f32 = 2.0;
/// It sees machines this far (m), where nothing's in the way.
pub const SIGHT: f32 = 80.0;
/// It keeps this far off lit ground (m).
pub const LIT_RADIUS: f32 = 10.0;
/// What it's told it keeps in mind this long (s).
pub const TOLD_FOR: f32 = 15.0;
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
    /// Where it's running to next.
    bound: Option<Vec2>,
    /// In cover until then.
    cover_until: f32,
    /// What it's been told, and when.
    told: Vec<(SquadCall, f32)>,
    /// Lit ground to keep off.
    lit: Option<Vec2>,
}

/// The next cover toward the way out: a walkable point a bound ahead,
/// out of sight of as many of the machines it knows of as it can be, and
/// off lit ground.
fn next_cover(
    nav: &extraction_core::nav::NavGrid,
    here: Vec2,
    path: &[Vec2],
    goal: Vec2,
    exposed: &dyn Fn(Vec2) -> usize,
    lit: Option<Vec2>,
) -> Vec2 {
    let ahead = path.first().copied().unwrap_or(goal);
    let way = (ahead - here).normalize_or(Vec2::X);
    let mut best: Option<(f32, Vec2)> = None;
    for r in [6.0, 10.0, 14.0, BOUND] {
        for k in -4..=4 {
            let p = here + Vec2::from_angle(k as f32 * 0.35).rotate(way) * r;
            if !nav.walkable(p.to_array()) || !nav.line_clear(here.to_array(), p.to_array()) {
                continue;
            }
            if lit.is_some_and(|l| l.distance(p) < LIT_RADIUS) {
                continue;
            }
            // Out of sight first, then the most way made.
            let score = exposed(p) as f32 * 100.0 + p.distance(goal) - here.distance(goal);
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, p));
            }
        }
    }
    best.map_or(ahead, |(_, p)| p)
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
        bound: None,
        cover_until: 0.0,
        told: vec![],
        lit: None,
    };
    let mut sight: SystemState<SpatialQuery> = SystemState::new(app.world_mut());
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
        // What it heard: a chirp near, a moment ago; the radio, once it's
        // said it.
        let now = app.world().resource::<Time>().elapsed_secs();
        let mut heard: Vec<SquadCall> = vec![];
        {
            let h = &app.world().resource::<Heard>().0;
            for (c, when) in h.iter().skip(acted) {
                if now - when < REACT {
                    break;
                }
                acted += 1;
                if c.at.xz().distance(here) < CHIRP_REACH {
                    heard.push(*c);
                }
            }
            let a = &app.world().resource::<super::squad::Aired>().0;
            let fresh: Vec<SquadCall> = a
                .iter()
                .filter(|(_, when)| now - when >= REACT && now - when < REACT + 1.0)
                .map(|(c, _)| *c)
                .collect();
            for c in fresh {
                if !heard.contains(&c) && !me.told.iter().any(|(x, _)| *x == c) {
                    heard.push(c);
                }
            }
        }
        for call in heard.iter().filter(|c| c.target == player) {
            if !arm.listen || me.told.iter().any(|(x, _)| x == call) {
                continue;
            }
            me.told.push((*call, t));
            let post = call.post.xz();
            match call.role {
                Role::CutOff => {
                    // Another way out: one the cut-off isn't on the way
                    // to, and not back through the rest of them (where
                    // it's heard them call from). None such: on as it was.
                    let shut = (post - here).normalize_or(Vec2::X);
                    let them = me
                        .told
                        .iter()
                        .map(|(c, _)| c.at.xz())
                        .fold(Vec2::ZERO, |a, p| a + p);
                    let them = (them / me.told.len().max(1) as f32 - here).normalize_or_zero();
                    let other = (0..rvs.len())
                        .filter(|&i| i != me.goal)
                        .filter(|&i| {
                            let to = (rvs[i].1 - here).normalize_or(Vec2::X);
                            to.dot(shut) < 0.5 && to.dot(them) < 0.3
                        })
                        .min_by(|&a, &b| {
                            rvs[a].1.distance(here).total_cmp(&rvs[b].1.distance(here))
                        });
                    if let Some(i) = other {
                        me.goal = i;
                        me.path = route(&app, here, rvs[i].1);
                        me.bound = None;
                        heeded += 1;
                    }
                }
                Role::Flank | Role::Pin => heeded += 1,
                Role::Spot => {
                    me.lit = Some(post);
                    heeded += 1;
                }
                Role::Call => {}
            }
        }
        me.told.retain(|(_, when)| t - when < TOLD_FOR);
        while me.path.len() > 1 && me.path[0].distance(here) < 1.5 {
            me.path.remove(0);
        }
        // In cover: crouched, a breath, then the next bound.
        let crouch = me.bound.is_none() && t < me.cover_until;
        if let Some(mut rig) = app.world_mut().get_mut::<RigInput>(player) {
            rig.crouch = crouch;
        }
        if crouch {
            continue;
        }
        // The machines it knows of: those it can see, and (listening)
        // where it's been told they're going. Seen is seen: a ray, as
        // their eyes and guns have it.
        let nav = app.world().resource::<super::walker::Nav>().0.clone();
        let (calm, chosen) = {
            let sq = sight.get(app.world()).expect("spatial queries");
            let world = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
            let los = |a: Vec2, b: Vec2| {
                let from = ground(a) + Vec3::Y * 1.3;
                let to = ground(b) + Vec3::Y * 1.5;
                let d = to - from;
                Dir3::new(d).map_or(true, |dir| {
                    sq.cast_ray(from, dir, d.length(), true, &world).is_none()
                })
            };
            let seen: Vec<Vec2> = squad
                .iter()
                .filter_map(|e| {
                    let r = app.world().get::<super::Robot>(*e)?;
                    if r.wrecked || r.kind == super::RobotKind::Drone {
                        return None;
                    }
                    let p = app.world().get::<Transform>(*e)?.translation.xz();
                    (p.distance(here) < SIGHT && los(here, p)).then_some(p)
                })
                .collect();
            let mut threats = seen;
            threats.extend(
                me.told
                    .iter()
                    .filter(|(c, _)| matches!(c.role, Role::Flank | Role::Pin | Role::CutOff))
                    .map(|(c, _)| c.post.xz()),
            );
            let exposed = |p: Vec2| {
                threats
                    .iter()
                    .filter(|th| th.distance(p) < SIGHT && los(p, **th))
                    .count()
            };
            let chosen = me.bound.is_none().then(|| {
                if threats.is_empty() {
                    // Nobody in sight: straight on.
                    me.path.first().copied().unwrap_or(rvs[me.goal].1)
                } else {
                    next_cover(&nav, here, &me.path, rvs[me.goal].1, &exposed, me.lit)
                }
            });
            (threats.is_empty(), chosen)
        };
        if me.bound.is_none() {
            me.bound = chosen;
        }
        let threats_empty = calm;
        let aim = me.bound.unwrap();
        let pace = if threats_empty { PACE } else { SPRINT };
        let step = (aim - here).clamp_length_max(pace * dt);
        let next = here + step;
        if nav.walkable(next.to_array()) {
            here = next;
        } else {
            me.path = route(&app, here, rvs[me.goal].1);
            me.bound = None;
        }
        if here.distance(aim) < 0.8 {
            me.bound = None;
            if !threats_empty {
                me.cover_until = t + COVER_WAIT;
            }
        }
        put(&mut app, here);
        if std::env::var("SIM_DRILL_TRACE").is_ok() && (t * HZ) as u32 % 45 == 0 {
            let hp = app
                .world()
                .get::<crate::health::Health>(player)
                .map_or(0.0, |h| h.hp);
            let robots: Vec<String> = squad
                .iter()
                .filter_map(|e| {
                    let tr = app.world().get::<Transform>(*e)?.translation;
                    let b = app.world().get::<Brain>(*e)?;
                    let r = app
                        .world()
                        .get::<super::squad::SquadRole>(*e)
                        .map(|r| r.role);
                    Some(format!(
                        "{:.0}m {:?} {:?}",
                        tr.xz().distance(here),
                        r,
                        std::mem::discriminant(b)
                    ))
                })
                .collect();
            eprintln!(
                "TRACE t {t:.1} hp {hp:.0} at {here:.0} calm {threats_empty} cover {} goal {} {robots:?}",
                me.bound.is_none(),
                rvs[me.goal].0
            );
        }
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
