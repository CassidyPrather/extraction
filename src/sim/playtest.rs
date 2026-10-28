//! Recording your own raids. A normal game (one that saves) writes each
//! raid to `<data dir>/raids/<map>/<when>.jsonl` in the sims' telemetry
//! format, so `scripts/sim-report.sh <map> <that folder>` draws the same
//! report and heatmaps for a person as for the bots: where you went, what
//! hurt you, how you died. `--no-record` turns it off.
//!
//! Marks: F9 (or both sticks clicked in VR) flags the moment — a `mark`
//! line in the recording and the log, and a screenshot in
//! `<data dir>/marks/` — so "something was off here" can be found later.

use bevy::prelude::*;
use serde_json::json;

use super::telemetry::{self, SimLog, round};
use crate::health::Downed;
use crate::items::{Catalog, Item};
use crate::launch::LaunchOptions;
use crate::raid::{Phase, Raid};
use crate::rig::LocalPlayer;

pub struct PlaytestPlugin;

impl Plugin for PlaytestPlugin {
    fn build(&self, app: &mut App) {
        // Presses, reads, goals and settings are noted in every game.
        app.add_plugins(super::observe::ObservePlugin);
        let Some(opts) = app.world().get_resource::<LaunchOptions>().cloned() else {
            return;
        };
        // The yard keeps nothing, but it's recorded like a raid: the first
        // night's 32 minutes there left no trace but the log.
        let yard = opts.map == crate::camp::TRAINING_YARD
            && crate::persist::owns_profile(&opts)
            && !opts.tooling();
        if opts.sim_run.is_some()
            || opts.no_record
            || (crate::persist::career_path(&opts).is_none() && !yard)
        {
            return;
        }
        if yard {
            app.insert_resource(Yard)
                .add_systems(Last, close_yard.run_if(resource_exists::<SimLog>));
        }
        let dir = opts
            .save
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| crate::persist::profile(&opts))
            .join("raids")
            .join(&opts.map);
        app.insert_resource(RaidsDir(dir)).add_systems(
            Update,
            (
                marks,
                open_recording,
                telemetry::record.run_if(resource_exists::<SimLog>),
                close_recording
                    .run_if(resource_exists::<SimLog>.and_then(not(resource_exists::<Yard>))),
            )
                .chain()
                .after(crate::raid::tick_raid),
        );
    }
}

#[derive(Resource)]
struct RaidsDir(std::path::PathBuf);

/// This session is the training yard: one recording, from the start to
/// the cord (or quitting).
#[derive(Resource)]
struct Yard;

/// F9 / both sticks: note the moment.
fn marks(
    mut commands: Commands,
    players: Query<(&crate::rig::RigInput, &GlobalTransform), With<LocalPlayer>>,
    log: Option<ResMut<SimLog>>,
    mut raid: ResMut<Raid>,
    dirs: Res<RaidsDir>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
    mut held: Local<bool>,
    mut count: Local<u32>,
) {
    let Some((input, gt)) = players.iter().next() else {
        return;
    };
    // Once per press (VR reports the sticks held).
    let pressed = input.mark && !*held;
    *held = input.mark;
    if !pressed {
        return;
    }
    *count += 1;
    let p = gt.translation();
    warn!(
        "PLAYTEST MARK {} at ({:.1}, {:.1}, {:.1}), raid clock {:.0} s",
        *count, p.x, p.y, p.z, raid.clock
    );
    raid.say(format!("Mark {} noted.", *count));
    if let Some(mut log) = log {
        log.event(
            "mark",
            json!({"n": *count, "x": round(p.x, 1), "z": round(p.z, 1), "y": round(p.y, 1)}),
        );
    }
    if !windows.is_empty() {
        let dir = dirs
            .0
            .parent()
            .and_then(|d| d.parent())
            .map(|d| d.join("marks"));
        if let Some(dir) = dir {
            let _ = std::fs::create_dir_all(&dir);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let path = dir.join(format!("mark-{stamp}-{}.png", *count));
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(path));
        }
    }
}

/// Stepping out of the truck starts a recording.
fn open_recording(
    mut commands: Commands,
    raid: Res<Raid>,
    dir: Res<RaidsDir>,
    opts: Res<LaunchOptions>,
    log: Option<Res<SimLog>>,
    yard: Option<Res<Yard>>,
    mut was_at_base: Local<Option<bool>>,
) {
    let at_base = raid.phase == Phase::AtBase;
    let set_off = was_at_base.replace(at_base) == Some(true) && raid.phase == Phase::Out;
    // The yard records from the start; a raid from stepping out.
    if log.is_some() || !(set_off || yard.is_some()) {
        return;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let name = if yard.is_some() {
        format!("yard-{stamp}")
    } else {
        format!("raid{:03}-{stamp}", raid.raids)
    };
    let mut log = SimLog::open(dir.0.join(format!("{name}.jsonl")));
    log.event(
        "start",
        json!({"run": name, "map": opts.map, "seed": opts.seed, "personality": "player",
               "minutes": 0, "format": telemetry::FORMAT, "cycle": raid.cycle}),
    );
    info!("recording this raid to {}", log.path.display());
    commands.insert_resource(log);
}

/// The yard's recording ends with the session (the cord quits to the
/// valley; or the game is closed), and is flushed now and then meanwhile.
fn close_yard(
    mut commands: Commands,
    mut log: ResMut<SimLog>,
    exits: Res<Messages<AppExit>>,
    onboarding: Option<Res<crate::onboarding::Onboarding>>,
    time: Res<Time>,
    mut flush_in: Local<f32>,
) {
    if exits.is_empty() {
        *flush_in -= time.delta_secs();
        if *flush_in <= 0.0 {
            *flush_in = 5.0;
            log.flush();
        }
        return;
    }
    let goals = onboarding.map_or(0, |o| o.done.len());
    let s = log.stats.clone();
    let t = log.t;
    log.event(
        "end",
        json!({"outcome": "left_yard", "goals": goals, "raid_time": round(t, 1),
               "distance": round(s.distance, 0), "shots": s.shots, "hits": s.hits,
               "blocked": s.blocked, "blows": s.blows, "heals": s.heals}),
    );
    log.flush();
    log.finished = true;
    commands.remove_resource::<SimLog>();
}

/// Getting out, going down, or being back at base ends the recording.
#[allow(clippy::too_many_arguments)]
fn close_recording(
    mut commands: Commands,
    mut log: ResMut<SimLog>,
    raid: Res<Raid>,
    mut downed: MessageReader<Downed>,
    players: Query<(Entity, &Transform), With<LocalPlayer>>,
    carried: Query<(&Item, &crate::anomaly::artifacts::CarriedBy)>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut flush_in: Local<f32>,
    health: Query<&crate::health::Health>,
) {
    let Some((me, tf)) = players.iter().next() else {
        return;
    };
    let pos = tf.translation;
    let mut outcome = None;
    let mut death = None;
    for d in downed.read() {
        if d.player == me {
            let (cause, finisher) = telemetry::death_cause(&log);
            log.event(
                "death",
                json!({"cause": cause, "finisher": finisher, "x": round(pos.x, 1),
                       "z": round(pos.z, 1), "doing": "player"}),
            );
            death = Some(cause);
            outcome = Some("died");
        }
    }
    if outcome.is_none() {
        outcome = match raid.phase {
            Phase::Extracting { .. } => Some("extracted"),
            // Slept out the Surge, or never left: nothing to report.
            Phase::AtBase => Some("abandoned"),
            _ => None,
        };
    }
    let Some(outcome) = outcome else {
        // A crash or a quit mid-raid still leaves most of it on disk.
        *flush_in -= time.delta_secs();
        if *flush_in <= 0.0 {
            *flush_in = 5.0;
            log.flush();
        }
        return;
    };
    // The last second's damage, before the summary counts it.
    let hp = health.get(me).map_or(0.0, |h| h.hp);
    log.flush_damage(pos, hp);
    if outcome == "extracted" {
        log.event(
            "extract",
            json!({"x": round(pos.x, 1), "z": round(pos.z, 1)}),
        );
    }
    // What came home (or went down) with you, as the loot list.
    let loot: Vec<String> = carried
        .iter()
        .filter(|(_, by)| by.0 == me)
        .map(|(i, _)| catalog.get(i.kind()).id.clone())
        .collect();
    let s = log.stats.clone();
    log.event(
        "end",
        json!({
            "outcome": outcome,
            "raid_time": round(raid.clock, 1),
            "distance": round(s.distance, 0),
            "loot": loot,
            "loot_count": loot.len(),
            "shots": s.shots,
            "hits": s.hits,
            "blocked": s.blocked,
            "blows": s.blows,
            "engagements": s.engagements,
            "kills": s.kills,
            "damage": s.damage.iter().map(|(k, v)| (k.clone(), round(*v, 1))).collect::<std::collections::BTreeMap<_, _>>(),
            "anomaly_entries": s.anomaly_entries,
            "heals": s.heals,
            "extract_reason": null,
            "death_cause": death,
            "x": round(pos.x, 1),
            "z": round(pos.z, 1),
        }),
    );
    log.flush();
    log.finished = true;
    info!("raid recorded: {outcome} -> {}", log.path.display());
    commands.remove_resource::<SimLog>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raid_you_play_is_recorded() {
        let dir = std::env::temp_dir()
            .join(format!("extraction-test-{}", std::process::id()))
            .join("playtest");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let opts = LaunchOptions {
            robots: false,
            save: Some(dir.join("career.ron")),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        assert!(
            !app.world().contains_resource::<SimLog>(),
            "nothing at base"
        );
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        for _ in 0..90 * 2 {
            app.update();
        }
        assert!(app.world().contains_resource::<SimLog>(), "recording");
        app.world_mut().resource_mut::<Raid>().phase = Phase::Extracting { left: 4.0 };
        app.update();
        app.update();
        assert!(!app.world().contains_resource::<SimLog>(), "closed");
        let raids = dir.join("raids").join("marrow_creek");
        let file = std::fs::read_dir(&raids)
            .unwrap()
            .flatten()
            .next()
            .expect("a recording")
            .path();
        let text = std::fs::read_to_string(file).unwrap();
        let evs: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let has = |e: &str| evs.iter().any(|v| v["ev"] == e);
        assert!(has("start") && has("pos") && has("end"), "{text}");
        let end = evs.iter().find(|v| v["ev"] == "end").unwrap();
        assert_eq!(end["outcome"], "extracted");
        assert_eq!(evs[0]["personality"], "player");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A session's second raid is recorded as fully as its first: its
    /// positions, its damage and a dressing used. (The recorder's state
    /// lived in a system's `Local` for the app's life; the playtest's
    /// second raid logged no positions and no damage at all.)
    #[test]
    fn every_raid_of_a_session_is_recorded_whole() {
        let dir = std::env::temp_dir()
            .join(format!("extraction-test-{}", std::process::id()))
            .join("two_raids");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let opts = LaunchOptions {
            robots: false,
            save: Some(dir.join("career.ron")),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let me = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let catalog = app.world().resource::<Catalog>().clone();
        let raid = |app: &mut App, secs: usize| {
            app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
            for _ in 0..90 * secs / 2 {
                app.update();
            }
            // Hurt, then a dressing.
            app.world_mut()
                .get_mut::<crate::health::Health>(me)
                .unwrap()
                .hp -= 10.0;
            let dressing = {
                let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
                let mut c = q.get_mut(app.world_mut()).unwrap();
                let e = crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: catalog.kind("field_dressing").unwrap(),
                        transform: Transform::from_xyz(0.0, -100.0, 0.0),
                        condition: 1.0,
                    },
                );
                q.apply(app.world_mut());
                e
            };
            app.world_mut().write_message(crate::items::usage::UseItem {
                player: me,
                item: dressing,
            });
            for _ in 0..90 * secs / 2 {
                app.update();
            }
            app.world_mut().resource_mut::<Raid>().phase = Phase::Extracting { left: 4.0 };
            // Home, and the truck's ride back to base.
            for _ in 0..90 * 6 {
                app.update();
            }
            assert_eq!(app.world().resource::<Raid>().phase, Phase::AtBase);
        };
        // A long first raid and a short second: its first sample must
        // come a second in, not at the first raid's end.
        raid(&mut app, 12);
        raid(&mut app, 4);
        let raids = dir.join("raids").join("marrow_creek");
        let mut files: Vec<_> = std::fs::read_dir(&raids)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        files.sort();
        assert_eq!(files.len(), 2, "{files:?}");
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            let evs: Vec<serde_json::Value> = text
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
            let count = |e: &str| evs.iter().filter(|v| v["ev"] == e).count();
            assert!(count("pos") >= 3, "{}: positions\n{text}", f.display());
            assert!(count("damage") >= 1, "{}: damage\n{text}", f.display());
            assert_eq!(count("heal"), 1, "{}: the dressing\n{text}", f.display());
            let end = evs.iter().find(|v| v["ev"] == "end").unwrap();
            assert_eq!(end["heals"], 1);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A player's kill counts, credited to the hit that wrecked it: the
    /// first night's "no kill" rested on kills being counted for bots only.
    #[test]
    fn a_players_kill_is_counted() {
        use crate::robots::{Job, JobKind, PartRole, RobotPart};
        let dir = std::env::temp_dir()
            .join(format!("extraction-test-{}", std::process::id()))
            .join("kill");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let opts = LaunchOptions {
            robots: false,
            save: Some(dir.join("career.ron")),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let me = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        for _ in 0..30 {
            app.update();
        }
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let feet = app.world().get::<Transform>(me).unwrap().translation;
        let at = zone.on_ground(feet.x + 12.0, feet.z);
        let walker = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let job = Job {
                kind: JobKind::Survey,
                site: at,
                duration: 999.0,
                exit: at,
            };
            let w = crate::robots::walker::spawn_walker(&mut c, &zone, at, job);
            q.apply(app.world_mut());
            w
        };
        for _ in 0..10 {
            app.update();
        }
        let core = app
            .world_mut()
            .query::<(Entity, &RobotPart)>()
            .iter(app.world())
            .find(|(_, p)| p.robot == walker && p.role == PartRole::Core)
            .map(|(e, _)| e)
            .expect("the walker's core");
        let point = app
            .world()
            .get::<GlobalTransform>(core)
            .unwrap()
            .translation();
        app.world_mut()
            .write_message(crate::weapons::ballistics::Hit {
                entity: core,
                point,
                normal: Vec3::Y,
                velocity: Vec3::X * 300.0,
                damage: 10_000.0,
                shooter: Some(me),
                mass_g: 6.0,
                load: Default::default(),
                blow: None,
            });
        for _ in 0..60 {
            app.update();
        }
        assert!(
            app.world()
                .get::<crate::robots::Robot>(walker)
                .unwrap()
                .wrecked,
            "the hit wrecked it"
        );
        app.world_mut().resource_mut::<Raid>().phase = Phase::Extracting { left: 4.0 };
        app.update();
        app.update();
        let raids = dir.join("raids").join("marrow_creek");
        let file = std::fs::read_dir(&raids)
            .unwrap()
            .flatten()
            .next()
            .unwrap()
            .path();
        let text = std::fs::read_to_string(file).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let evs: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert!(evs.iter().any(|v| v["ev"] == "kill"), "a kill: {text}");
        let end = evs.iter().find(|v| v["ev"] == "end").unwrap();
        assert_eq!(end["kills"]["Walker"], 1, "{text}");
        assert!(
            evs.iter().any(|v| v["ev"] == "engage"),
            "and the fight: {text}"
        );
    }
}
