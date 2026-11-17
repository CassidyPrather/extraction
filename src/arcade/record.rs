//! Each run written down for playtests, in two files under
//! `<data dir>/arcade/` (`target/arcade/` for a headless run with no data
//! dir of its own). A run's record opens when the bench is laid (so the
//! range before the first call is in it) and closes when you go down or
//! press RESTART.
//!
//! - `run-<stamp>-<n>.jsonl`: the sims' telemetry (`sim::telemetry`:
//!   shots, hits by machine and part, damage by cause, heals, kills,
//!   positions), so `scripts/sim-report.sh` reads it as it reads a raid;
//!   and the arcade's own lines (`docs/ARCADE.md` lists them).
//! - `run-<stamp>-<n>.summary.txt`: [`RunStats::summary`].
//!
//! "Mid-fight" is one rule, here and in both files ([`mid_fight`]): a
//! machine of the run's still standing within [`FIGHT_NEAR`] of you, or
//! aware of you.
//!
//! After a run, the board asks how each gun used felt, 1 to 5 ([`Feel`]);
//! each answer is added to that run's files.

use std::collections::BTreeMap;
use std::io::Write;

use bevy::prelude::*;
use serde_json::json;

use super::range::{AIMED_NEAR, RangePlate};
use super::stats::{Bests, OPENING_SHOTS, STRING_GAP, band, gun_card, take_in};
pub use super::stats::{GunStats, KillStats, RunStats};
use super::waves::{RunOver, WaveMachine};
use super::{Arcade, ArcadeSystems, RunPhase};
use crate::launch::LaunchOptions;
use crate::rig::LocalPlayer;
use crate::robots::{Awareness, Robot, RobotPart};
use crate::sim::telemetry::{self, SimLog, round};
use extraction_core::firearm::Event as FireEvent;

/// A machine this near is in the fight with you, m.
pub const FIGHT_NEAR: f32 = 40.0;

/// Suspicion at which a machine tracking you counts as aware of you.
pub const AWARE: f32 = 0.5;

/// Hits within this of a shot are that shot's, s.
const HIT_AFTER_SHOT: f32 = 3.0;

/// Rounds pushed in one at a time within this of each other are one
/// reload, s.
const ROUNDS_ONE_RELOAD: f32 = 2.5;

/// The mid-fight rule: a machine of the run's still standing, within
/// [`FIGHT_NEAR`] of `you` or aware of `me`.
pub fn mid_fight<'a>(
    me: Entity,
    you: Vec3,
    machines: impl IntoIterator<Item = (Vec3, bool, Option<&'a Awareness>)>,
) -> bool {
    machines.into_iter().any(|(at, wrecked, aware)| {
        !wrecked
            && (at.distance(you) <= FIGHT_NEAR
                || aware.is_some_and(|a| {
                    a.can_see_target || (a.target == Some(me) && a.suspicion >= AWARE)
                }))
    })
}

/// What a shot was at: a fight's (by the distance band to the nearest
/// machine standing, if any), or a range plate's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    Fight(Option<usize>),
    Range(usize),
}

/// Which plate a shot from `origin` along `dir` was aimed at: of those its
/// line passes within [`AIMED_NEAR`] of, the one nearest by angle.
pub fn aimed_plate(
    origin: Vec3,
    dir: Vec3,
    plates: impl IntoIterator<Item = (usize, Vec3)>,
) -> Option<usize> {
    let dir = dir.normalize_or_zero();
    plates
        .into_iter()
        .filter_map(|(i, c)| {
            let v = c - origin;
            let along = v.dot(dir);
            let miss = (v - dir * along).length();
            (along > 0.0 && miss <= AIMED_NEAR).then_some((i, miss / along))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The last shot, for hits that follow it.
#[derive(Clone, Debug)]
struct LastShot {
    id: u64,
    gun: String,
    t: f32,
    aim: Aim,
    /// Its place in its string (1 for the first).
    ord: u32,
    struck: bool,
}

/// A gun's current string of shots.
#[derive(Clone, Debug)]
struct ShotString {
    start: f32,
    shots: u32,
    hit: bool,
}

/// A reload waiting for the next shot to be costed.
#[derive(Clone, Debug)]
struct Pending {
    last_shot: Option<f32>,
    from: f32,
    mid_fight: bool,
}

/// The run being recorded.
#[derive(Resource, Default)]
pub struct Recording {
    pub stats: RunStats,
    /// Where the summary goes (the jsonl's name, `.summary.txt`).
    pub summary_path: Option<std::path::PathBuf>,
    shots: u64,
    last: Option<LastShot>,
    /// Per gun: its last shot, its string, a reload or a stoppage waiting
    /// for its next shot, a magazine out.
    last_at: BTreeMap<String, f32>,
    strings: BTreeMap<String, ShotString>,
    reloading: BTreeMap<String, Pending>,
    jammed: BTreeMap<String, f32>,
    mag_out: BTreeMap<String, f32>,
    /// Per machine: first hit, last hit's gun, the shots that struck it.
    struck: BTreeMap<Entity, (f32, String, Vec<u64>)>,
    /// Fight shots since the wave came in or the last machine went down.
    shots_since: u32,
    /// Machines already counted down.
    down: Vec<Entity>,
    /// Per gun, when it last took a round by hand (one reload's run of
    /// them).
    thumbed: BTreeMap<Entity, f32>,
    /// The run's beat last frame: (which, wave).
    last_beat: (u8, u32),
    /// The finished runs' files, for the smoke scenario.
    pub finished: Vec<std::path::PathBuf>,
    /// The last finished run, for the gun card until the next fires: its
    /// title, numbers, and the bests it was held to.
    pub last_run: Option<(String, RunStats, Bests)>,
}

/// Each gun's best so far, across runs (`<arcade dir>/guns.json`).
#[derive(Resource, Default)]
pub struct GunBests {
    pub bests: Bests,
    /// The bests before this run: the card holds this run to them.
    pub before: Bests,
    pub path: Option<std::path::PathBuf>,
}

/// The board's question after a run: how did each gun used feel, 1 to 5.
#[derive(Resource, Default, Debug)]
pub struct Feel {
    /// The guns still to answer for, in the order used.
    pub asking: Vec<String>,
    /// The finished run's files and its clock at the end.
    pub jsonl: Option<std::path::PathBuf>,
    pub summary: Option<std::path::PathBuf>,
    pub t: f32,
    /// Answers given, for the scenario: (gun, 1 to 5).
    pub answered: Vec<(String, u8)>,
}

impl Feel {
    /// The gun being asked about.
    pub fn gun(&self) -> Option<&str> {
        self.asking.first().map(String::as_str)
    }

    /// An answer for the gun asked about: 1 to 5, or 0 to skip it. Added
    /// to the run's jsonl and summary.
    pub fn answer(&mut self, score: u8) {
        if self.asking.is_empty() {
            return;
        }
        let gun = self.asking.remove(0);
        if score == 0 {
            return;
        }
        let score = score.min(5);
        let line = json!({"t": round(self.t, 2), "ev": "feel", "gun": gun, "score": score});
        append(self.jsonl.as_deref(), &format!("{line}\n"));
        let head = if self.answered.is_empty() {
            "\nFEEL       (1 poor to 5 great)\n"
        } else {
            ""
        };
        append(
            self.summary.as_deref(),
            &format!("{head}{gun:<10} {score}/5\n"),
        );
        info!("arcade: {gun} felt {score}/5");
        self.answered.push((gun, score));
    }
}

fn append(path: Option<&std::path::Path>, text: &str) {
    let Some(p) = path else {
        return;
    };
    let ok = std::fs::OpenOptions::new()
        .append(true)
        .open(p)
        .and_then(|mut f| f.write_all(text.as_bytes()));
    if let Err(e) = ok {
        warn!("arcade: can't add to {}: {e}", p.display());
    }
}

pub struct RecordPlugin;

impl Plugin for RecordPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Recording>()
            .init_resource::<GunBests>()
            .init_resource::<Feel>()
            .add_systems(
                Update,
                (
                    open_run,
                    telemetry::record.run_if(resource_exists::<SimLog>),
                    note.run_if(resource_exists::<SimLog>),
                    close_run,
                )
                    .chain()
                    .in_set(ArcadeSystems::Record),
            );
    }
}

/// Where runs are written.
pub fn dir(opts: &LaunchOptions) -> std::path::PathBuf {
    if crate::persist::owns_profile(opts) {
        crate::persist::profile(opts).join("arcade")
    } else {
        std::path::PathBuf::from("target/arcade")
    }
}

fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The card's title for where the run is.
fn card_title(arcade: &Arcade) -> String {
    match arcade.phase {
        RunPhase::Ready => "this run (range)".into(),
        RunPhase::Warning { wave, .. } | RunPhase::Fighting { wave } => format!("wave {wave} on"),
        RunPhase::Cleared { wave } => format!("wave {wave} cleared"),
        RunPhase::Over { wave } => format!("down on wave {wave}"),
    }
}

/// The gun card's text: this run's guns (or the last run's, until a gun
/// fires in this one) against their bests before.
pub fn card(arcade: &Arcade, rec: &Recording, bests: &GunBests, feel: &Feel) -> String {
    let mut s = String::new();
    if let Some(g) = feel.gun() {
        s += &format!(
            "HOW DID THE {} FEEL?\n1 poor ... 5 great. SKIP to pass.\n\n",
            g.to_uppercase()
        );
    }
    let fired = rec.stats.guns.values().any(|g| g.shots > 0);
    s += &match (&rec.last_run, fired) {
        (Some((title, run, before)), false) => gun_card(&format!("last run: {title}"), run, before),
        _ => gun_card(&card_title(arcade), &rec.stats, &bests.before),
    };
    s
}

/// The bench is laid and nothing's called yet: a run's record opens.
fn open_run(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    arcade: Res<Arcade>,
    laid: Res<super::arena::Laid>,
    log: Option<Res<SimLog>>,
    mut rec: ResMut<Recording>,
    mut bests: ResMut<GunBests>,
) {
    let opening = matches!(arcade.phase, RunPhase::Ready | RunPhase::Warning { .. });
    if log.is_some() || !opening || !laid.0 || arcade.reset != super::Reset::None {
        return;
    }
    let d = dir(&opts);
    if bests.path.is_none() {
        let p = d.join("guns.json");
        bests.bests = std::fs::read_to_string(&p)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        bests.path = Some(p);
    }
    bests.before = bests.bests.clone();
    let run = arcade.runs + u32::from(matches!(arcade.phase, RunPhase::Ready));
    let name = format!("run-{}-{run}", stamp());
    let path = d.join(format!("{name}.jsonl"));
    let mut log = SimLog::open(path.clone());
    log.event(
        "arcade_run",
        json!({"format": telemetry::FORMAT, "map": opts.map, "start_at": arcade.start_at,
               "run": run, "build": crate::report::build()}),
    );
    info!("arcade: recording to {}", path.display());
    *rec = Recording {
        summary_path: Some(d.join(format!("{name}.summary.txt"))),
        finished: std::mem::take(&mut rec.finished),
        last_run: rec.last_run.take(),
        ..default()
    };
    commands.insert_resource(log);
}

/// The name of a robot's part, without its side.
fn part_name(p: &RobotPart) -> String {
    let full = format!("{:?}", p.role);
    full.split('(').next().unwrap_or(&full).to_string()
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn note(
    mut log: ResMut<SimLog>,
    mut rec: ResMut<Recording>,
    mut arcade: ResMut<Arcade>,
    mut feel: ResMut<Feel>,
    catalog: Res<crate::items::Catalog>,
    me: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    machines: Query<(
        Entity,
        &WaveMachine,
        &Robot,
        &GlobalTransform,
        Option<&Awareness>,
    )>,
    (parts, plates, items, picked): (
        Query<&RobotPart>,
        Query<(&RangePlate, &GlobalTransform)>,
        Query<&crate::items::Item>,
        Query<&crate::items::Item, (Added<crate::interact::Held>, With<crate::weapons::Gun>)>,
    ),
    (mut guns, mut shots, mut hits, mut used, mut treated, mut grenades): (
        MessageReader<crate::weapons::GunEvent>,
        MessageReader<crate::weapons::ballistics::Shot>,
        MessageReader<crate::weapons::ballistics::Hit>,
        MessageReader<crate::items::usage::UseItem>,
        MessageReader<crate::items::usage::Treated>,
        MessageReader<crate::weapons::grenades::GrenadeEvent>,
    ),
) {
    let t = log.t;
    let Some((me, you)) = me.iter().next().map(|(e, g)| (e, g.translation())) else {
        return;
    };
    let id_of = |e: Entity| {
        items
            .get(e)
            .ok()
            .map(|it| catalog.get(it.0.kind).id.clone())
    };
    let fighting = mid_fight(
        me,
        you,
        machines
            .iter()
            .map(|(_, _, r, g, a)| (g.translation(), r.wrecked, a)),
    );
    let wave_on = matches!(
        arcade.phase,
        RunPhase::Warning { .. } | RunPhase::Fighting { .. }
    );
    // The run's own beats.
    let phase = arcade.phase;
    let beat = (
        match phase {
            RunPhase::Ready => 0,
            RunPhase::Warning { .. } => 1,
            RunPhase::Fighting { .. } => 2,
            RunPhase::Cleared { .. } => 3,
            RunPhase::Over { .. } => 4,
        },
        arcade.wave(),
    );
    if beat != rec.last_beat {
        match phase {
            RunPhase::Warning { wave, .. } => {
                // The question's over once a wave is called.
                feel.asking.clear();
                let (gate, ..) = super::waves::GATES[super::waves::gate(wave)];
                log.event(
                    "wave_called",
                    json!({"wave": wave, "machines": super::waves::describe(&super::waves::wave(wave)),
                           "gate": gate.to_lowercase()}),
                );
            }
            RunPhase::Fighting { wave } => {
                rec.shots_since = 0;
                log.event("wave_arrived", json!({"wave": wave}));
            }
            RunPhase::Cleared { wave } => {
                rec.stats.waves_cleared += 1;
                log.event(
                    "wave_cleared",
                    json!({"wave": wave, "score": arcade.score, "time": round(arcade.run_t, 1)}),
                );
            }
            _ => {}
        }
        rec.last_beat = beat;
    }
    // Guns picked up, in order.
    for it in &picked {
        let id = catalog.get(it.0.kind).id.clone();
        if !rec.stats.picked.contains(&id) {
            rec.stats.picked.push(id.clone());
            log.event(
                "picked_up",
                json!({"gun": id, "order": rec.stats.picked.len()}),
            );
        }
    }
    // What the guns did besides fire.
    for ev in guns.read() {
        // The gun's own id (a seated magazine's event names none).
        let gun = id_of(ev.gun).unwrap_or_else(|| ev.item.clone());
        debug!("arcade: {gun} {:?}", ev.event);
        match &ev.event {
            FireEvent::MagazineRemoved => {
                rec.mag_out.insert(gun, t);
            }
            FireEvent::MagazineInserted | FireEvent::RoundLoaded | FireEvent::RoundThumbed => {
                // A clip's or a hand's rounds, one after another, are one
                // reload.
                let by_hand = !matches!(ev.event, FireEvent::MagazineInserted);
                if by_hand {
                    let last = rec.thumbed.insert(ev.gun, t);
                    if last.is_some_and(|l| t - l <= ROUNDS_ONE_RELOAD) {
                        continue;
                    }
                }
                let st = rec.stats.guns.entry(gun.clone()).or_default();
                st.reloads += 1;
                if fighting {
                    st.mid_fight_reloads += 1;
                }
                let from = rec.mag_out.remove(&gun).unwrap_or(t);
                let last_shot = rec.last_at.get(&gun).copied();
                rec.reloading.insert(
                    gun.clone(),
                    Pending {
                        last_shot,
                        from,
                        mid_fight: fighting,
                    },
                );
                log.event(
                    "reload",
                    json!({"gun": gun, "mid_fight": fighting,
                           "how": if by_hand { "rounds" } else { "magazine" }}),
                );
            }
            FireEvent::Malfunctioned(m) => {
                rec.stats.guns.entry(gun.clone()).or_default().stoppages += 1;
                rec.jammed.entry(gun.clone()).or_insert(t);
                log.event(
                    "stoppage",
                    json!({"gun": gun, "kind": format!("{m:?}"), "mid_fight": fighting}),
                );
            }
            _ => {}
        }
    }
    // Shots: what each was at, its string, what it ended.
    let plate_at: Vec<(usize, Vec3)> = plates
        .iter()
        .map(|(p, g)| (p.index, g.translation()))
        .collect();
    for s in shots.read() {
        if s.shooter != Some(me) {
            continue;
        }
        let Some(gun) = id_of(s.gun) else {
            continue;
        };
        let range = (!wave_on)
            .then(|| aimed_plate(s.origin, s.direction, plate_at.iter().copied()))
            .flatten();
        let nearest = machines
            .iter()
            .filter(|(_, _, r, ..)| !r.wrecked)
            .map(|(_, _, _, g, _)| g.translation().distance(s.origin))
            .min_by(f32::total_cmp);
        let aim = match range {
            Some(i) => Aim::Range(i),
            None => Aim::Fight(nearest.map(band)),
        };
        rec.shots += 1;
        let id = rec.shots;
        let prev = rec.last_at.insert(gun.clone(), t);
        let pending = rec.reloading.remove(&gun);
        let jam = rec.jammed.remove(&gun);
        let new_string = pending.is_some() || prev.is_none_or(|p| t - p > STRING_GAP);
        if new_string {
            rec.strings.insert(
                gun.clone(),
                ShotString {
                    start: t,
                    shots: 0,
                    hit: false,
                },
            );
        }
        let ord = {
            let s = rec.strings.get_mut(&gun).expect("a string");
            s.shots += 1;
            s.shots
        };
        if matches!(aim, Aim::Fight(_)) {
            rec.shots_since += 1;
        }
        let st = rec.stats.guns.entry(gun.clone()).or_default();
        st.shots += 1;
        if ord <= OPENING_SHOTS {
            st.opening.0 += 1;
        }
        if let (false, Some(p)) = (new_string, prev) {
            st.gaps.push(t - p);
        }
        match aim {
            Aim::Range(i) => st.range[i].0 += 1,
            Aim::Fight(Some(b)) => st.fight[b].0 += 1,
            Aim::Fight(None) => {}
        }
        if let Some(p) = pending {
            let cost = p.last_shot.map(|l| t - l);
            if let Some(c) = cost {
                st.reload_cost.push((c, p.mid_fight));
            }
            st.reload_time.push(t - p.from);
            log.event(
                "reload_done",
                json!({"gun": gun, "cost": cost.map(|c| round(c, 2)),
                       "time": round(t - p.from, 2), "mid_fight": p.mid_fight}),
            );
        }
        if let Some(j) = jam {
            st.clear_time.push(t - j);
            log.event(
                "stoppage_cleared",
                json!({"gun": gun, "secs": round(t - j, 2)}),
            );
        }
        let (target, band_name) = match aim {
            Aim::Range(i) => (Some(super::stats::PLATES[i]), super::stats::PLATE_NAMES[i]),
            Aim::Fight(b) => (nearest, b.map_or("-", |b| super::stats::BAND_NAMES[b])),
        };
        log.event(
            "fired",
            json!({"gun": gun, "range": matches!(aim, Aim::Range(_)), "band": band_name,
                   "target_m": target.map(|d| round(d, 1)), "mid_fight": fighting}),
        );
        rec.last = Some(LastShot {
            id,
            gun,
            t,
            aim,
            ord,
            struck: false,
        });
    }
    // Hits on the run's machines and the range's plates, put down to the
    // shot fired last.
    for h in hits.read() {
        if h.shooter != Some(me) {
            continue;
        }
        let plate = plates.get(h.entity).ok().map(|(p, _)| p.index);
        let machine = parts
            .get(h.entity)
            .ok()
            .filter(|p| machines.contains(p.robot));
        if plate.is_none() && machine.is_none() {
            continue;
        }
        let shot = if h.blow.is_some() {
            None
        } else {
            rec.last
                .clone()
                .filter(|s| h.mass_g > 0.0 && t - s.t <= HIT_AFTER_SHOT)
        };
        let gun = match (&shot, h.blow.is_some(), h.mass_g == 0.0) {
            (_, true, _) => "blow".to_string(),
            (_, _, true) => "grenade".to_string(),
            (Some(s), ..) => s.gun.clone(),
            (None, ..) => "unknown".into(),
        };
        let dist = h.point.distance(you);
        let part = match (machine, plate) {
            (Some(p), _) => part_name(p),
            _ => "plate".to_string(),
        };
        let fresh = shot.as_ref().is_some_and(|s| !s.struck);
        if let Some(st) = rec.stats.guns.get_mut(&gun) {
            let e = st.parts.entry(part.clone()).or_default();
            e.0 += 1;
            e.1 += h.damage;
            if let (true, Some(s)) = (fresh, &shot) {
                st.hits += 1;
                match s.aim {
                    Aim::Range(i) => st.range[i].1 += 1,
                    Aim::Fight(Some(b)) => st.fight[b].1 += 1,
                    Aim::Fight(None) => {}
                }
                if s.ord <= OPENING_SHOTS {
                    st.opening.1 += 1;
                }
            }
        }
        if let Some(s) = &shot {
            if let Some(string) = rec.strings.get_mut(&s.gun) {
                if !string.hit {
                    string.hit = true;
                    let first = t - string.start;
                    if let Some(st) = rec.stats.guns.get_mut(&s.gun) {
                        st.first_hit.push(first);
                    }
                }
            }
            if let Some(l) = rec.last.as_mut() {
                l.struck = true;
            }
        }
        let what = match (machine, plate) {
            (Some(p), _) => machines
                .get(p.robot)
                .map(|(_, w, ..)| w.kind.name().to_string())
                .unwrap_or_default(),
            (_, Some(i)) => format!("plate_{}", super::stats::PLATE_NAMES[i]),
            _ => String::new(),
        };
        log.event(
            "gun_hit",
            json!({"gun": gun, "target": what, "part": part, "damage": round(h.damage, 1),
                   "dist": round(dist, 1), "range": plate.is_some()}),
        );
        if let Some(p) = machine {
            let shot_id = shot.as_ref().map(|s| s.id);
            let e = rec
                .struck
                .entry(p.robot)
                .or_insert_with(|| (t, gun.clone(), Vec::new()));
            e.1 = gun;
            if let Some(id) = shot_id {
                if !e.2.contains(&id) {
                    e.2.push(id);
                }
            }
        }
    }
    // Machines down: whose, what it took, how far, the points.
    for (e, w, r, g, _) in &machines {
        if !r.wrecked || rec.down.contains(&e) {
            continue;
        }
        rec.down.push(e);
        let name = w.kind.name().to_string();
        let (first, gun, struck) =
            rec.struck
                .get(&e)
                .cloned()
                .unwrap_or((t, "unknown".into(), Vec::new()));
        let ttk = t - first;
        let shots = std::mem::take(&mut rec.shots_since);
        let hits = struck.len() as u32;
        let points = w.kind.points();
        arcade.score += points;
        let k = rec.stats.machines.entry(name.clone()).or_default();
        k.kills += 1;
        k.ttk.push(ttk);
        k.shots += shots;
        k.hits += hits;
        if let Some(st) = rec.stats.guns.get_mut(&gun) {
            st.kills += 1;
            st.kill_shots += shots;
            st.kill_hits += hits;
            st.ttk.push(ttk);
        }
        let p = g.translation();
        log.event(
            "machine_down",
            json!({"machine": name, "wave": w.wave, "gun": gun, "ttk": round(ttk, 2),
                   "shots": shots, "hits": hits, "dist": round(p.distance(you), 1),
                   "points": points, "x": round(p.x, 1), "z": round(p.z, 1)}),
        );
    }
    // Items used.
    for u in used.read() {
        if u.player != me {
            continue;
        }
        if let Some(id) = id_of(u.item) {
            *rec.stats.items.entry(id.clone()).or_default() += 1;
            log.event("item_used", json!({"item": id, "mid_fight": fighting}));
        }
    }
    for tr in treated.read() {
        if tr.player == me {
            *rec.stats.items.entry(tr.item.to_string()).or_default() += 1;
            log.event("item_used", json!({"item": tr.item, "mid_fight": fighting}));
        }
    }
    for g in grenades.read() {
        if g.event == extraction_core::grenade::Event::Exploded {
            *rec.stats.items.entry("grenade".into()).or_default() += 1;
            log.event(
                "grenade",
                json!({"x": round(g.at.x, 1), "z": round(g.at.z, 1), "mid_fight": fighting}),
            );
        }
    }
}

/// The run's end (down, or RESTART part-way): the last lines, the
/// summary, the bests, the files closed, and the board's question.
#[allow(clippy::too_many_arguments)]
fn close_run(
    mut commands: Commands,
    log: Option<ResMut<SimLog>>,
    mut rec: ResMut<Recording>,
    mut arcade: ResMut<Arcade>,
    mut bests: ResMut<GunBests>,
    mut feel: ResMut<Feel>,
    mut over: MessageReader<RunOver>,
    me: Query<(&GlobalTransform, &crate::health::Health), With<LocalPlayer>>,
) {
    let ended = over.read().last().cloned();
    let Some(mut log) = log else {
        return;
    };
    let restarted = arcade.reset != super::Reset::None && ended.is_none();
    if ended.is_none() && !restarted {
        return;
    }
    if let Some((g, h)) = me.iter().next() {
        log.flush_damage(g.translation(), h.hp);
    }
    let wave = arcade.wave();
    let cause = ended.as_ref().map(|_| log.last_cause.clone());
    rec.stats.cause = cause.clone().filter(|c| !c.is_empty());
    log.event(
        "run_over",
        json!({"wave": wave, "score": arcade.score, "time": round(arcade.run_t, 1),
               "waves_cleared": rec.stats.waves_cleared,
               "how": if restarted { "restart" } else { "down" },
               "cause": rec.stats.cause, "guns_used": rec.stats.used()}),
    );
    log.flush();
    let summary = rec
        .stats
        .summary(wave, arcade.score, arcade.run_t, !restarted);
    let jsonl = log.path.clone();
    if let Some(p) = rec.summary_path.clone() {
        if let Err(e) = std::fs::write(&p, &summary) {
            warn!("arcade: can't write {}: {e}", p.display());
        }
        rec.finished.push(jsonl.clone());
        info!("arcade: run written: {}", p.display());
    }
    // The bests take this run in; the card keeps holding it to the ones
    // before.
    let stats = rec.stats.clone();
    let before = bests.before.clone();
    take_in(&mut bests.bests, &stats);
    if let Some(p) = bests.path.clone() {
        let text = serde_json::to_string_pretty(&bests.bests).unwrap_or_default();
        if let Err(e) = std::fs::write(&p, text) {
            warn!("arcade: can't write {}: {e}", p.display());
        }
    }
    *feel = Feel {
        asking: stats.used(),
        jsonl: Some(jsonl),
        summary: rec.summary_path.clone(),
        t: log.t,
        answered: Vec::new(),
    };
    let title = if restarted {
        format!("restarted on wave {wave}")
    } else {
        format!("down on wave {wave}")
    };
    rec.last_run = Some((title, stats, before));
    arcade.summary = Some(summary);
    log.finished = true;
    commands.remove_resource::<SimLog>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aware(target: Option<Entity>, suspicion: f32, sees: bool) -> Awareness {
        Awareness {
            target,
            last_known: None,
            uncertainty: 0.0,
            unseen: 0.0,
            suspicion,
            can_see_target: sees,
            exposure: 0.0,
            news: false,
        }
    }

    #[test]
    fn mid_fight_is_one_rule() {
        let me = Entity::from_raw_u32(7).unwrap();
        let you = Vec3::ZERO;
        let far = Vec3::new(0.0, 0.0, 60.0);
        let near = Vec3::new(0.0, 0.0, 39.0);
        assert!(!mid_fight(me, you, []), "nothing out");
        assert!(mid_fight(me, you, [(near, false, None)]), "near");
        assert!(
            !mid_fight(me, you, [(near, true, None)]),
            "a wreck is out of it"
        );
        assert!(!mid_fight(me, you, [(far, false, None)]), "far, unaware");
        let a = aware(Some(me), 0.6, false);
        assert!(
            mid_fight(me, you, [(far, false, Some(&a))]),
            "far, after you"
        );
        let a = aware(None, 0.0, true);
        assert!(
            mid_fight(me, you, [(far, false, Some(&a))]),
            "far, sees you"
        );
        let a = aware(Some(me), 0.2, false);
        assert!(!mid_fight(me, you, [(far, false, Some(&a))]), "far, unsure");
    }

    #[test]
    fn a_shot_is_put_to_the_plate_it_was_aimed_at() {
        // As the range stands them: staggered across the lane.
        let plates = [0, 1, 2].map(|i| {
            (
                i,
                Vec3::new(
                    super::super::stats::PLATES[i],
                    1.4,
                    super::super::range::ACROSS[i],
                ),
            )
        });
        let from = Vec3::new(0.0, 1.5, 0.0);
        let at = |p: Vec3| p - from;
        assert_eq!(aimed_plate(from, at(plates[1].1), plates), Some(1));
        assert_eq!(
            aimed_plate(from, at(plates[2].1 + Vec3::Z * 1.0), plates),
            Some(2),
            "a near miss is still at it"
        );
        assert_eq!(
            aimed_plate(from, Vec3::Z, plates),
            None,
            "off the lane is no plate's"
        );
        assert_eq!(aimed_plate(from, -Vec3::X, plates), None, "behind the line");
    }

    #[test]
    fn a_feel_answer_goes_into_both_files() {
        let dir = std::env::temp_dir().join(format!("arcade-feel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let jsonl = dir.join("run.jsonl");
        let summary = dir.join("run.summary.txt");
        std::fs::write(&jsonl, "{\"ev\":\"run_over\"}\n").unwrap();
        std::fs::write(&summary, "Score 0\n").unwrap();
        let mut f = Feel {
            asking: vec!["ak74".into(), "sks".into(), "ppsh".into()],
            jsonl: Some(jsonl.clone()),
            summary: Some(summary.clone()),
            t: 12.0,
            answered: Vec::new(),
        };
        assert_eq!(f.gun(), Some("ak74"));
        f.answer(4);
        f.answer(0);
        assert_eq!(f.gun(), Some("ppsh"), "SKIP passes a gun by");
        f.answer(9);
        assert_eq!(f.gun(), None);
        let j = std::fs::read_to_string(&jsonl).unwrap();
        assert!(j.contains("\"ev\":\"feel\""), "{j}");
        assert!(
            j.contains("\"gun\":\"ak74\"") && j.contains("\"score\":4"),
            "{j}"
        );
        assert!(!j.contains("sks"), "{j}");
        assert!(j.contains("\"gun\":\"ppsh\""), "{j}");
        let s = std::fs::read_to_string(&summary).unwrap();
        assert!(s.contains("FEEL") && s.contains("ak74       4/5"), "{s}");
        assert!(s.contains("ppsh       5/5"), "{s}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
