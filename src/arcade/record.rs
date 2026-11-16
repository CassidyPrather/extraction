//! Each run written down for playtests, in two files under
//! `<data dir>/arcade/` (`target/arcade/` for a headless run with no data
//! dir of its own):
//!
//! - `run-<stamp>-<n>.jsonl`: the sims' telemetry (`sim::telemetry`:
//!   shots, hits by machine and part, damage by cause, heals, kills,
//!   positions), so `scripts/sim-report.sh` reads it as it reads a raid;
//!   and the arcade's own lines: `wave_called`, `wave_arrived`,
//!   `wave_cleared`, `fired` (gun), `reload` (gun, `mid_fight`),
//!   `stoppage` (gun, kind), `machine_down` (machine, gun, ttk, points),
//!   `item_used`, `grenade`, `run_over` (wave, score, cause).
//! - `run-<stamp>-<n>.summary.txt`: per gun shots, hits, accuracy,
//!   reloads, mid-fight reloads and stoppages; items used; mean time to
//!   kill by machine; the death and its cause; waves, score and time.
//!
//! "Mid-fight" is one rule, here and in both files ([`mid_fight`]): a
//! machine of the run's still standing within [`FIGHT_NEAR`] of you, or
//! aware of you.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde_json::json;

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

/// Hits within this of a shot are that gun's, s.
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

/// One gun's run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GunStats {
    pub shots: u32,
    pub hits: u32,
    pub reloads: u32,
    pub mid_fight_reloads: u32,
    pub stoppages: u32,
}

/// The run's totals, for the summary.
#[derive(Debug, Default)]
pub struct RunStats {
    pub guns: BTreeMap<String, GunStats>,
    pub items: BTreeMap<String, u32>,
    /// Time to kill by machine: (sum, count), s.
    pub ttk: BTreeMap<String, (f32, u32)>,
    pub kills: BTreeMap<String, u32>,
    pub cause: Option<String>,
    pub waves_cleared: u32,
}

impl RunStats {
    /// The summary's text: the same numbers as the jsonl's lines.
    pub fn summary(&self, wave: u32, score: u32, t: f32) -> String {
        let mut s = format!(
            "Waves cleared {}, down on wave {wave}\nScore {score}, time {}:{:02}\n",
            self.waves_cleared,
            t as u32 / 60,
            t as u32 % 60
        );
        if let Some(c) = &self.cause {
            s += &format!("Taken down by: {c}\n");
        }
        s += "\nGUN        shots hits  acc reloads mid-fight stoppages\n";
        for (g, st) in &self.guns {
            let acc = if st.shots > 0 {
                format!("{:3.0}%", 100.0 * st.hits as f32 / st.shots as f32)
            } else {
                "   -".into()
            };
            s += &format!(
                "{g:<10} {:>5} {:>4} {acc} {:>7} {:>9} {:>9}\n",
                st.shots, st.hits, st.reloads, st.mid_fight_reloads, st.stoppages
            );
        }
        if !self.kills.is_empty() {
            s += "\nDOWN       kills mean ttk\n";
            for (m, n) in &self.kills {
                let ttk = self
                    .ttk
                    .get(m)
                    .filter(|(_, c)| *c > 0)
                    .map(|(sum, c)| format!("{:.1} s", sum / *c as f32))
                    .unwrap_or_else(|| "-".into());
                s += &format!("{m:<10} {n:>5} {ttk:>8}\n");
            }
        }
        if !self.items.is_empty() {
            s += "\nUSED\n";
            for (i, n) in &self.items {
                s += &format!("{i:<16} {n}\n");
            }
        }
        s
    }
}

/// The run being recorded.
#[derive(Resource, Default)]
pub struct Recording {
    pub stats: RunStats,
    /// Where the summary goes (the jsonl's name, `.summary.txt`).
    pub summary_path: Option<std::path::PathBuf>,
    /// The gun last fired, and when.
    last_gun: Option<(String, f32)>,
    /// Per machine: first hit, last hit's gun.
    struck: BTreeMap<Entity, (f32, String)>,
    /// Machines already counted down.
    down: Vec<Entity>,
    /// Per gun, when it last took a round by hand (one reload's run of
    /// them).
    thumbed: BTreeMap<Entity, f32>,
    /// The run's beat last frame: (which, wave).
    last_beat: (u8, u32),
    /// The finished run's files, for the smoke scenario.
    pub finished: Vec<std::path::PathBuf>,
}

pub struct RecordPlugin;

impl Plugin for RecordPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Recording>().add_systems(
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

/// A run's first call opens its files.
fn open_run(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    arcade: Res<Arcade>,
    log: Option<Res<SimLog>>,
    mut rec: ResMut<Recording>,
) {
    if log.is_some() || !matches!(arcade.phase, RunPhase::Warning { .. }) {
        return;
    }
    let name = format!("run-{}-{}", stamp(), arcade.runs);
    let d = dir(&opts);
    let path = d.join(format!("{name}.jsonl"));
    let mut log = SimLog::open(path.clone());
    log.event(
        "arcade_run",
        json!({"format": telemetry::FORMAT, "map": opts.map, "start_at": arcade.start_at,
               "run": arcade.runs, "build": crate::report::build()}),
    );
    info!("arcade: recording to {}", path.display());
    *rec = Recording {
        summary_path: Some(d.join(format!("{name}.summary.txt"))),
        finished: std::mem::take(&mut rec.finished),
        ..default()
    };
    commands.insert_resource(log);
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn note(
    mut log: ResMut<SimLog>,
    mut rec: ResMut<Recording>,
    mut arcade: ResMut<Arcade>,
    catalog: Res<crate::items::Catalog>,
    me: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    machines: Query<(
        Entity,
        &WaveMachine,
        &Robot,
        &GlobalTransform,
        Option<&Awareness>,
    )>,
    parts: Query<&RobotPart>,
    items: Query<&crate::items::Item>,
    (mut guns, mut hits, mut used, mut treated, mut grenades): (
        MessageReader<crate::weapons::GunEvent>,
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
    let fighting = mid_fight(
        me,
        you,
        machines
            .iter()
            .map(|(_, _, r, g, a)| (g.translation(), r.wrecked, a)),
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
                let (gate, ..) = super::waves::GATES[super::waves::gate(wave)];
                log.event(
                    "wave_called",
                    json!({"wave": wave, "machines": super::waves::describe(&super::waves::wave(wave)),
                           "gate": gate.to_lowercase()}),
                );
            }
            RunPhase::Fighting { wave } => log.event("wave_arrived", json!({"wave": wave})),
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
    // What the guns did.
    for ev in guns.read() {
        let gun = ev.item.clone();
        match &ev.event {
            FireEvent::Fired { .. } => {
                rec.stats.guns.entry(gun.clone()).or_default().shots += 1;
                rec.last_gun = Some((gun.clone(), t));
                log.event("fired", json!({"gun": gun}));
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
                log.event(
                    "reload",
                    json!({"gun": gun, "mid_fight": fighting,
                           "how": if by_hand { "rounds" } else { "magazine" }}),
                );
            }
            FireEvent::Malfunctioned(m) => {
                rec.stats.guns.entry(gun.clone()).or_default().stoppages += 1;
                log.event(
                    "stoppage",
                    json!({"gun": gun, "kind": format!("{m:?}"), "mid_fight": fighting}),
                );
            }
            _ => {}
        }
    }
    // Hits on the run's machines, put down to the gun that fired last.
    for h in hits.read() {
        if h.shooter != Some(me) {
            continue;
        }
        let Ok(part) = parts.get(h.entity) else {
            continue;
        };
        let Ok((_, w, ..)) = machines.get(part.robot) else {
            continue;
        };
        let _ = w;
        let gun = if h.blow.is_some() {
            "blow".to_string()
        } else if h.mass_g == 0.0 {
            "grenade".to_string()
        } else {
            rec.last_gun
                .as_ref()
                .filter(|(_, at)| t - at <= HIT_AFTER_SHOT)
                .map(|(g, _)| g.clone())
                .unwrap_or_else(|| "unknown".into())
        };
        if let Some(st) = rec.stats.guns.get_mut(&gun) {
            st.hits += 1;
        }
        let first = rec.struck.get(&part.robot).map_or(t, |(f, _)| *f);
        rec.struck.insert(part.robot, (first, gun));
    }
    // Machines down: whose, how long it took, the points.
    for (e, w, r, g, _) in &machines {
        if !r.wrecked || rec.down.contains(&e) {
            continue;
        }
        rec.down.push(e);
        let name = w.kind.name().to_string();
        let (first, gun) = rec.struck.get(&e).cloned().unwrap_or((t, "unknown".into()));
        let ttk = t - first;
        let points = w.kind.points();
        arcade.score += points;
        *rec.stats.kills.entry(name.clone()).or_default() += 1;
        let e2 = rec.stats.ttk.entry(name.clone()).or_default();
        e2.0 += ttk;
        e2.1 += 1;
        let p = g.translation();
        log.event(
            "machine_down",
            json!({"machine": name, "wave": w.wave, "gun": gun, "ttk": round(ttk, 2),
                   "points": points, "x": round(p.x, 1), "z": round(p.z, 1)}),
        );
    }
    // Items used.
    for u in used.read() {
        if u.player != me {
            continue;
        }
        if let Ok(it) = items.get(u.item) {
            let id = catalog.get(it.0.kind).id.clone();
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
/// summary, the files closed.
fn close_run(
    mut commands: Commands,
    log: Option<ResMut<SimLog>>,
    mut rec: ResMut<Recording>,
    mut arcade: ResMut<Arcade>,
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
               "cause": rec.stats.cause}),
    );
    log.flush();
    let summary = rec.stats.summary(wave, arcade.score, arcade.run_t);
    if let Some(p) = rec.summary_path.clone() {
        if let Err(e) = std::fs::write(&p, &summary) {
            warn!("arcade: can't write {}: {e}", p.display());
        }
        let jsonl = log.path.clone();
        rec.finished.push(jsonl);
        info!("arcade: run written: {}", p.display());
    }
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
    fn the_summary_carries_the_numbers() {
        let mut s = RunStats::default();
        s.guns.insert(
            "ak74".into(),
            GunStats {
                shots: 20,
                hits: 5,
                reloads: 2,
                mid_fight_reloads: 1,
                stoppages: 1,
            },
        );
        s.kills.insert("walker".into(), 2);
        s.ttk.insert("walker".into(), (7.0, 2));
        s.items.insert("field_dressing".into(), 1);
        s.cause = Some("walker".into());
        s.waves_cleared = 2;
        let text = s.summary(3, 450, 125.0);
        assert!(text.contains("Waves cleared 2, down on wave 3"), "{text}");
        assert!(text.contains("Score 450, time 2:05"), "{text}");
        assert!(text.contains("ak74"), "{text}");
        assert!(text.contains(" 25%"), "{text}");
        assert!(text.contains("3.5 s"), "{text}");
        assert!(text.contains("Taken down by: walker"), "{text}");
        assert!(text.contains("field_dressing"), "{text}");
    }
}
