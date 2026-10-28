//! Sim telemetry: one JSON object per line, written as the raid happens.
//!
//! Every line has `t` (seconds of game time since the run started) and
//! `ev` (what happened); the rest depends on `ev`. The format is
//! documented in `docs/SIMULATION.md`; `scripts/sim_report.py` reads it.

use bevy::prelude::*;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::io::Write;

use super::raider::Raider;
use crate::anomaly::{Anomaly, AnomalyEvent};
use crate::health::{Downed, Health};
use crate::player::{Body, BodyCollider};
use crate::raid::{Phase, Raid};
use crate::rig::LocalPlayer;
use crate::robots::{Robot, RobotPart};
use crate::weapons::ballistics::{Hit, Shot};

/// Telemetry format version, bumped when a field changes meaning.
pub const FORMAT: u32 = 1;

/// The run's telemetry: the open file and running totals for the summary.
#[derive(Resource)]
pub struct SimLog {
    out: Option<std::io::BufWriter<std::fs::File>>,
    pub path: std::path::PathBuf,
    /// Game seconds since the run started.
    pub t: f32,
    pub stats: Stats,
    pub finished: bool,
    /// Recent damage (time, cause, amount), for naming a death.
    pub recent: Vec<(f32, String, f32)>,
    /// What dealt the latest damage.
    pub last_cause: String,
    /// The recorder's bookkeeping between frames. It belongs to this
    /// recording, not to the app: kept in a system's `Local` it outlived the
    /// raid, and a session's second raid logged no positions and no damage
    /// (its next sample was due at the first raid's 1,168 s).
    pub tracker: Tracker,
    /// The death's cause and finisher, once known (the summary names it).
    pub death: Option<(String, String)>,
}

#[derive(Default, Debug, Clone)]
pub struct Stats {
    pub loot: Vec<String>,
    pub shots: u32,
    pub hits: u32,
    /// Rounds that hit something other than a robot.
    pub blocked: u32,
    pub engagements: u32,
    pub kills: BTreeMap<String, u32>,
    pub damage: BTreeMap<String, f32>,
    pub distance: f32,
    pub anomaly_entries: u32,
    pub heals: u32,
    /// Blows from a hand or a tool that struck something other than a
    /// robot (rounds stopped by the world are `blocked`).
    pub blows: u32,
    pub extract_reason: Option<String>,
}

impl SimLog {
    pub fn open(path: std::path::PathBuf) -> Self {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let out = std::fs::File::create(&path)
            .map_err(|e| error!("sim: can't write {}: {e}", path.display()))
            .ok()
            .map(std::io::BufWriter::new);
        Self {
            out,
            path,
            t: 0.0,
            stats: Stats::default(),
            finished: false,
            recent: Vec::new(),
            last_cause: String::new(),
            tracker: Tracker::default(),
            death: None,
        }
    }

    /// The next raid in the same recording file (the soak's career plays
    /// raid after raid in one process): everything kept for one raid
    /// starts again.
    pub fn new_raid(&mut self) {
        self.t = 0.0;
        self.stats = Stats::default();
        self.finished = false;
        self.recent.clear();
        self.last_cause.clear();
        self.tracker = Tracker::default();
        self.death = None;
    }

    /// Writes the damage waiting for the next sample (at the end of a
    /// recording, so the last second's isn't lost).
    pub fn flush_damage(&mut self, at: Vec3, hp: f32) {
        let pending = std::mem::take(&mut self.tracker.pending);
        for (cause, amount) in pending {
            *self.stats.damage.entry(cause.clone()).or_default() += amount;
            self.event(
                "damage",
                json!({"cause": cause, "amount": round(amount, 1), "x": round(at.x, 1),
                       "z": round(at.z, 1), "hp": round(hp.max(0.0), 0)}),
            );
        }
    }

    /// Writes one event line. `fields` must be a JSON object.
    pub fn event(&mut self, ev: &str, fields: Value) {
        if self.finished {
            return;
        }
        let mut m = Map::new();
        m.insert("t".into(), json!(round(self.t, 2)));
        m.insert("ev".into(), json!(ev));
        if let Value::Object(f) = fields {
            m.extend(f);
        }
        if let Some(out) = &mut self.out {
            let _ = writeln!(out, "{}", Value::Object(m));
        }
    }

    pub fn flush(&mut self) {
        if let Some(out) = &mut self.out {
            let _ = out.flush();
        }
    }
}

pub fn round(v: f32, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (v as f64 * k).round() / k
}

/// A fight a player is in (the sim's raider keeps its own): when it
/// started and was last active, the robot's name, shots fired and hits
/// landed when it started.
#[derive(Clone, Debug)]
pub struct Fight {
    pub since: f32,
    pub last: f32,
    pub robot: String,
    pub shots_at_start: u32,
    pub hits_at_start: u32,
}

/// A player's fight ends this long after the last shot or hit either way.
pub const FIGHT_LULL: f32 = 20.0;

/// Per-run bookkeeping the recorder keeps between frames.
#[derive(Default)]
pub struct Tracker {
    /// A player's fights, by robot.
    fights: Vec<(Entity, Fight)>,
    last_pos: Option<Vec3>,
    next_sample: f32,
    last_hp: Option<f32>,
    /// Damage this second, by cause, waiting to be written.
    pending: BTreeMap<String, f32>,
    /// Robot hits seen but not yet taken off the health bar (the bar may
    /// update a tick later): (cause, damage, seconds to live).
    expect: Vec<(String, f32, f32)>,
    inside: Vec<Entity>,
    phase: String,
    /// The longest spell airborne lately, s, fading a second a second: a
    /// landing that hurts follows most of a second of free fall.
    recent_air: f32,
}

pub(crate) fn phase_name(p: &Phase) -> &'static str {
    match p {
        Phase::AtBase => "at_base",
        Phase::Out => "out",
        Phase::TruckCalled { .. } => "truck_called",
        Phase::TruckWaiting { .. } => "truck_waiting",
        Phase::Extracting { .. } => "extracting",
        Phase::Down { .. } => "down",
    }
}

/// Watches the bot and the world and writes what happened.
pub fn record(
    mut log: ResMut<SimLog>,
    time: Res<Time>,
    raid: Res<Raid>,
    bots: Query<
        (
            Entity,
            &Transform,
            &Health,
            &Body,
            Option<&Raider>,
            &Children,
            (
                Has<crate::world::workings::InGas>,
                Has<crate::world::furnace::InHeat>,
            ),
        ),
        With<LocalPlayer>,
    >,
    colliders: Query<(), With<BodyCollider>>,
    robots: Query<(
        &Robot,
        Option<&crate::robots::Breed>,
        Option<&GlobalTransform>,
    )>,
    parts: Query<&RobotPart>,
    anomalies: Query<(Entity, &Anomaly, &GlobalTransform)>,
    mut shots: MessageReader<Shot>,
    mut hits: MessageReader<Hit>,
    mut anomaly_events: MessageReader<AnomalyEvent>,
    (mut bites, mut shocks, mut treated): (
        MessageReader<crate::anomaly::costs::ArtifactBite>,
        MessageReader<crate::world::live::Shocked>,
        MessageReader<crate::items::usage::Treated>,
    ),
    (names, parents): (Query<&Name>, Query<&ChildOf>),
) {
    if log.finished {
        return;
    }
    let dt = time.delta_secs();
    log.t += dt;
    let t = log.t;
    let Some((me, tf, health, body, raider, children, (in_gas, in_heat))) = bots.iter().next()
    else {
        return;
    };
    let mut tr = std::mem::take(&mut log.tracker);
    record_frame(
        &mut log,
        &mut tr,
        t,
        dt,
        &raid,
        (me, tf, health, body, raider, children, in_gas, in_heat),
        &colliders,
        &robots,
        &parts,
        &anomalies,
        (&mut shots, &mut hits, &mut anomaly_events),
        (&mut bites, &mut shocks, &mut treated),
        (&names, &parents),
    );
    log.tracker = tr;
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn record_frame(
    log: &mut SimLog,
    tr: &mut Tracker,
    t: f32,
    dt: f32,
    raid: &Raid,
    (me, tf, health, body, raider, children, in_gas, in_heat): (
        Entity,
        &Transform,
        &Health,
        &Body,
        Option<&Raider>,
        &Children,
        bool,
        bool,
    ),
    colliders: &Query<(), With<BodyCollider>>,
    robots: &Query<(
        &Robot,
        Option<&crate::robots::Breed>,
        Option<&GlobalTransform>,
    )>,
    parts: &Query<&RobotPart>,
    anomalies: &Query<(Entity, &Anomaly, &GlobalTransform)>,
    (shots, hits, anomaly_events): (
        &mut MessageReader<Shot>,
        &mut MessageReader<Hit>,
        &mut MessageReader<AnomalyEvent>,
    ),
    (bites, shocks, treated): (
        &mut MessageReader<crate::anomaly::costs::ArtifactBite>,
        &mut MessageReader<crate::world::live::Shocked>,
        &mut MessageReader<crate::items::usage::Treated>,
    ),
    (names, parents): (&Query<&Name>, &Query<&ChildOf>),
) {
    // (The sim's raider logs its own fights and heals.)
    let player = raider.is_none();
    let pos = tf.translation;
    let xz = |p: Vec3| json!({"x": round(p.x, 1), "z": round(p.z, 1)});

    // Distance, ignoring teleports (truck rides).
    if let Some(last) = tr.last_pos {
        let d = Vec2::new(pos.x - last.x, pos.z - last.z).length();
        if d < 3.0 {
            log.stats.distance += d;
        }
    }
    tr.last_pos = Some(pos);

    // Raid phase changes.
    let phase = phase_name(&raid.phase).to_string();
    if phase != tr.phase {
        log.event(
            "phase",
            json!({"phase": phase, "clock": round(raid.clock, 1)}),
        );
        tr.phase = phase;
    }

    // Shots and hits.
    for s in shots.read() {
        if s.shooter == Some(me) {
            log.stats.shots += 1;
            log.event("shot", xz(s.origin));
        }
    }
    for h in hits.read() {
        if h.shooter == Some(me) {
            if let Ok(part) = parts.get(h.entity) {
                log.stats.hits += 1;
                let kind = robots
                    .get(part.robot)
                    .map(|(r, b, _)| crate::robots::robot_name(r.kind, b))
                    .unwrap_or_default();
                if player {
                    fight_with(log, tr, t, part.robot, &kind, robots, pos);
                }
                log.event(
                    "hit",
                    json!({"robot": kind, "part": format!("{:?}", part.role), "damage": round(h.damage, 1)}),
                );
            } else {
                // A round stopped by something else: a wall, the ground.
                let what = names
                    .get(h.entity)
                    .ok()
                    .or_else(|| {
                        parents
                            .get(h.entity)
                            .ok()
                            .and_then(|p| names.get(p.parent()).ok())
                    })
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_else(|| "world".into());
                // A hand's or a tool's blow isn't a stopped round.
                let (ev, count) = if h.blow.is_some() {
                    ("blow", &mut log.stats.blows)
                } else {
                    ("blocked", &mut log.stats.blocked)
                };
                *count += 1;
                log.event(
                    ev,
                    json!({"what": what, "x": round(h.point.x, 1), "z": round(h.point.z, 1),
                           "y": round(h.point.y - pos.y, 1)}),
                );
            }
        } else if colliders.contains(h.entity) && children.contains(&h.entity) {
            if player {
                if let Some(s) = h.shooter.filter(|s| robots.contains(*s)) {
                    let kind = robots
                        .get(s)
                        .map(|(r, b, _)| crate::robots::robot_name(r.kind, b))
                        .unwrap_or_default();
                    fight_with(log, tr, t, s, &kind, robots, pos);
                }
            }
            let cause = h
                .shooter
                .and_then(|s| robots.get(s).ok())
                .map(|(r, b, _)| crate::robots::robot_name(r.kind, b))
                // Nobody's: a mine, a grenade or an anomaly's blast, a
                // falling beam, a stray round. Named by what it was.
                .unwrap_or_else(|| {
                    if h.blow.is_some() {
                        "blow"
                    } else if h.mass_g == 0.0 {
                        "blast"
                    } else {
                        "stray round"
                    }
                    .into()
                });
            tr.expect.push((cause, h.damage, 0.3));
        }
    }

    // A carried artifact biting back.
    for b in bites.read() {
        // What it took (nothing at its floor), so nothing else's damage is
        // put down to it.
        if b.player == me && b.what == crate::anomaly::costs::Bite::Shock && b.damage > 0.0 {
            tr.expect.push(("aegis".into(), b.damage, 0.3));
        }
    }
    // Live water throwing whoever wades in.
    for s in shocks.read() {
        if s.player == me {
            tr.expect
                .push(("live_water".into(), crate::world::live::SHOCK, 0.3));
        }
    }
    tr.recent_air = body.airborne.max(tr.recent_air - dt);

    // A player's dressings and injectors.
    for u in treated.read() {
        if u.player == me && player {
            log.stats.heals += 1;
            log.event(
                "heal",
                json!({"item": u.item, "hp": round(health.hp, 0), "bleeding": round(health.bleeding, 2)}),
            );
        }
    }
    // A player's fights end with the robot wrecked or gone, a lull, or
    // the player down.
    let fights = std::mem::take(&mut tr.fights);
    for (robot, f) in fights {
        let wrecked = robots.get(robot).map_or(None, |(r, ..)| Some(r.wrecked));
        let outcome = match wrecked {
            Some(true) => Some("wrecked"),
            None => Some("gone"),
            _ if health.down => Some("died"),
            _ if t - f.last > FIGHT_LULL => Some("broke_off"),
            _ => None,
        };
        match outcome {
            Some(o) => {
                if o == "wrecked" {
                    *log.stats.kills.entry(f.robot.clone()).or_default() += 1;
                }
                log.event(
                    "engage_end",
                    json!({"robot": f.robot, "outcome": o,
                           "shots": log.stats.shots - f.shots_at_start,
                           "hits": log.stats.hits - f.hits_at_start,
                           "secs": round(t - f.since, 1), "x": round(pos.x, 1), "z": round(pos.z, 1)}),
                );
            }
            None => tr.fights.push((robot, f)),
        }
    }

    // Anomalies: walking into one, and anything one does nearby.
    let mut inside_now = Vec::new();
    let mut inside_kind = None;
    for (e, a, gt) in anomalies.iter() {
        let rel = pos - gt.translation();
        if Vec2::new(rel.x, rel.z).length() < a.radius + 0.3 && rel.y < 3.0 {
            inside_now.push(e);
            inside_kind = Some(format!("{:?}", a.kind));
            if !tr.inside.contains(&e) {
                log.stats.anomaly_entries += 1;
                log.event(
                    "anomaly_enter",
                    json!({"kind": format!("{:?}", a.kind), "x": round(pos.x, 1), "z": round(pos.z, 1),
                           "bloom": a.bloom.is_some(),
                           "deliberate": raider.is_some_and(|r| r.diving)}),
                );
            }
        }
    }
    tr.inside = inside_now;
    for ev in anomaly_events.read() {
        let d = ev.at.distance(pos);
        if d < 10.0 {
            log.event(
                "anomaly",
                json!({"kind": format!("{:?}", ev.kind), "act": format!("{:?}", ev.what),
                       "x": round(ev.at.x, 1), "z": round(ev.at.z, 1), "dist": round(d, 1)}),
            );
        }
    }

    // Damage taken, by cause: robot hits are known exactly; the rest is
    // whatever is hurting right now.
    let hp = health.hp;
    if let Some(last) = tr.last_hp {
        let lost = last - hp;
        if lost > 1e-4 && hp < last {
            let mut rest = lost;
            let mut claimed: Option<String> = None;
            let mut expect = std::mem::take(&mut tr.expect);
            for (cause, d, _) in expect.iter_mut() {
                let take = d.min(rest);
                if take <= 0.0 {
                    continue;
                }
                rest -= take;
                *d -= take;
                *tr.pending.entry(cause.clone()).or_default() += take;
                log.recent.push((t, cause.clone(), take));
                log.last_cause = cause.clone();
                claimed = Some(cause.clone());
            }
            tr.expect = expect;
            // A sliver left over from a hit claimed this tick (armour and
            // rounding make the bar's drop differ from the hit's damage by
            // hundredths) is that hit's, not something else's.
            if let Some(cause) = claimed.filter(|_| rest < 0.05) {
                *tr.pending.entry(cause.clone()).or_default() += rest;
                rest = 0.0;
            }
            // Every unclaimed loss counts, however small a tick of it is:
            // gas at 2 hp a second is 0.02 a tick, and once that went
            // unrecorded a raider died underground of nothing at all.
            let mut parts: Vec<(String, f32)> = Vec::new();
            if let Some(k) = &inside_kind {
                parts.push((k.clone(), rest));
            } else if raid.surging() {
                parts.push(("surge".to_string(), rest));
            } else {
                let bleed = if health.bleeding > 0.0 {
                    rest.min(health.bleeding * dt * 1.5 + 1e-3)
                } else {
                    0.0
                };
                if bleed > 0.0 {
                    parts.push(("bleeding".to_string(), bleed));
                }
                let left = rest - bleed;
                if left > 1e-4 {
                    let cause = if in_gas {
                        "gas"
                    } else if in_heat {
                        "heat"
                    } else if tr.recent_air > 0.4 {
                        "fall"
                    } else {
                        // Nothing above claimed it: say so rather than guess.
                        "other"
                    };
                    parts.push((cause.to_string(), left));
                }
            }
            for (cause, amount) in parts {
                *tr.pending.entry(cause.clone()).or_default() += amount;
                log.recent.push((t, cause.clone(), amount));
                log.last_cause = cause;
            }
        }
    }
    for e in tr.expect.iter_mut() {
        e.2 -= dt;
    }
    tr.expect.retain(|e| e.1 > 0.01 && e.2 > 0.0);
    tr.last_hp = Some(if health.down { hp.min(0.0) } else { hp });
    log.recent.retain(|r| t - r.0 < 90.0);

    // Once a second: position and pending damage.
    if t >= tr.next_sample {
        tr.next_sample = t + 1.0;
        log.event(
            "pos",
            json!({"x": round(pos.x, 1), "z": round(pos.z, 1), "hp": round(hp.max(0.0), 0),
                   "kg": round(body.carried_kg, 1),
                   "stage": raider.map_or("player", |r| r.stage_name()),
                   "doing": raider.map_or("player", |r| r.doing())}),
        );
        log.tracker.pending = std::mem::take(&mut tr.pending);
        log.flush_damage(pos, hp);
    }
}

/// A player in a fight with `robot`: a new one is an engagement.
fn fight_with(
    log: &mut SimLog,
    tr: &mut Tracker,
    t: f32,
    robot: Entity,
    kind: &str,
    robots: &Query<(
        &Robot,
        Option<&crate::robots::Breed>,
        Option<&GlobalTransform>,
    )>,
    pos: Vec3,
) {
    if let Some((_, f)) = tr.fights.iter_mut().find(|(e, _)| *e == robot) {
        f.last = t;
        return;
    }
    let at = robots
        .get(robot)
        .ok()
        .and_then(|(.., g)| g.map(|g| g.translation()))
        .unwrap_or(pos);
    log.stats.engagements += 1;
    log.event(
        "engage",
        json!({"robot": kind, "dist": round(at.distance(pos), 0), "noticed": true,
               "x": round(pos.x, 1), "z": round(pos.z, 1), "rx": round(at.x, 1), "rz": round(at.z, 1)}),
    );
    tr.fights.push((
        robot,
        Fight {
            since: t,
            last: t,
            robot: kind.to_owned(),
            shots_at_start: log.stats.shots,
            hits_at_start: log.stats.hits,
        },
    ));
}

/// Ends the run: extracted, dead, or out of time.
pub fn finish(
    mut log: ResMut<SimLog>,
    raid: Res<Raid>,
    mut downed: MessageReader<Downed>,
    mut bots: Query<(Entity, &Transform, &mut Raider), With<LocalPlayer>>,
    opts: Res<crate::launch::LaunchOptions>,
    mut exit: MessageWriter<AppExit>,
    progression: Option<Res<crate::progression::Progression>>,
    zone: Res<crate::world::Zone>,
    house: Option<Res<crate::progression::safehouse::Safehouse>>,
) {
    if log.finished {
        return;
    }
    let Some((me, tf, mut raider)) = bots.iter_mut().next() else {
        return;
    };
    let pos = tf.translation;
    let mut outcome = None;
    for d in downed.read() {
        if d.player == me {
            // Name the death after what did the most harm lately (bleeding
            // out is the wound's fault, not the blood's).
            let (cause, finisher) = death_cause(&log);
            if let Some(mut c) = raider.combat.take() {
                c.close(&mut log, "died");
            }
            log.event(
                "death",
                json!({"cause": cause, "finisher": finisher, "x": round(pos.x, 1), "z": round(pos.z, 1),
                       "doing": raider.doing()}),
            );
            log.death = Some((cause, finisher));
            outcome = Some("died");
        }
    }
    if outcome.is_none() && matches!(raid.phase, Phase::Extracting { .. }) {
        log.event(
            "extract",
            json!({"x": round(pos.x, 1), "z": round(pos.z, 1)}),
        );
        outcome = Some("extracted");
    }
    let limit = opts.sim_minutes.unwrap_or(raider.traits.minutes) * 60.0 + 15.0 * 60.0;
    if outcome.is_none() && log.t > limit {
        outcome = Some("timeout");
    }
    let Some(outcome) = outcome else {
        return;
    };
    raider.stage = super::raider::Stage::Done;
    let s = log.stats.clone();
    let death = log.death.clone();
    log.event(
        "end",
        json!({
            "outcome": outcome,
            "raid_time": round(raid.clock, 1),
            "distance": round(s.distance, 0),
            "loot": s.loot,
            "loot_count": s.loot.len(),
            "shots": s.shots,
            "hits": s.hits,
            "blocked": s.blocked,
            "blows": s.blows,
            "engagements": s.engagements,
            "kills": s.kills,
            "damage": s.damage.iter().map(|(k, v)| (k.clone(), round(*v, 1))).collect::<BTreeMap<_, _>>(),
            "anomaly_entries": s.anomaly_entries,
            "heals": s.heals,
            "extract_reason": s.extract_reason,
            "death_cause": death.map(|d| d.0),
            // The driver's scout job (`progression`), when there is one here.
            "job": progression.as_ref().and_then(|p| match &p.0.job {
                Some(extraction_core::progression::Job::Scout { map, place, at, seen }) if *map == opts.map => {
                    Some(json!({"kind": "scout", "place": place, "seen": seen, "tier": p.0.tier,
                                "armour": std::env::var("SIM_ARMOUR").ok(),
                                "danger": zone.map.region_at(*at).map(|r| r.danger)}))
                }
                _ => None,
            }),
            // A contract's step (`SIM_CONTRACT`): whether it was met.
            "contract": progression.as_ref().and_then(|p| {
                let a = p.0.contract.as_ref()?;
                let job = p.0.job.as_ref()?;
                let home = outcome == "extracted";
                use extraction_core::progression::Job;
                // `done`: the step's own part (set down, seen, taken, picked
                // up); `met`: that, and home with it.
                let (kind, done) = match job {
                    Job::Plant { planted, .. } => ("plant", *planted),
                    Job::Bring { item, .. } => ("bring", s.loot.iter().any(|l| l == item)),
                    Job::Scout { seen, .. } => ("scout", *seen),
                    Job::Photograph { taken, .. } => ("photograph", *taken),
                    _ => ("other", false),
                };
                Some(json!({"id": a.id, "step": a.step, "kind": kind, "done": done, "met": done && home}))
            }),
            // Nights slept in the safehouse (`SIM_OVERNIGHT`).
            "nights": house.as_ref().map_or(0, |h| h.nights_out),
            "night_visits": house.as_ref().map_or(0, |h| h.visits),
            "x": round(pos.x, 1),
            "z": round(pos.z, 1),
        }),
    );
    log.flush();
    log.finished = true;
    println!(
        "sim {}: {outcome} after {:.0} s ({} items, {} shots, {} hits) -> {}",
        opts.sim_run.as_deref().unwrap_or(""),
        log.t,
        s.loot.len(),
        s.shots,
        s.hits,
        log.path.display()
    );
    exit.write(AppExit::Success);
}

/// What killed the bot: the cause that did the most harm in the last 90
/// seconds (bleeding out is the wound's fault), and what dealt the last
/// point.
pub(crate) fn death_cause(log: &SimLog) -> (String, String) {
    let mut by: BTreeMap<&str, f32> = BTreeMap::new();
    for (_, c, a) in &log.recent {
        if c != "bleeding" {
            *by.entry(c.as_str()).or_default() += a;
        }
    }
    let worst = by
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(c, _)| c.to_string())
        .unwrap_or_else(|| log.last_cause.clone());
    (worst, log.last_cause.clone())
}
