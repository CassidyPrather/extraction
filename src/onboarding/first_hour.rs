//! The first hour, measured: five newcomers play from the training yard
//! into a first raid in Marrow Creek, knowing only what the game shows
//! them, and say where they were confused, stuck, or hurt without warning.
//!
//! `--scenario first_hour` runs all five; `--scenario first_hour_<name>`
//! one. Each newcomer is two sessions, as a player's first hour is: the
//! yard (`first_yard`, the yard's own bot with the persona's habits) and,
//! after the cord, the game opened again at the camp in Marrow Creek
//! (`first_raid`), with the career the yard left. Both sessions keep it in
//! `target/first_hour/<name>/`.
//!
//! What each newcomer acts on is what's on show: the yard's boards and
//! lamps, the driver's words (`Raid::log`), the job card, the map sheet's
//! pencilled ring, and whatever's in sight. See `docs/FIRST_HOUR.md`.

use std::path::PathBuf;

use bevy::prelude::*;

use crate::bot::Task;
use crate::launch::LaunchOptions;

/// All five, one after the other.
pub const SCENARIO: &str = "first_hour";
/// A newcomer's yard.
pub const YARD: &str = "first_yard";
/// A newcomer's first raid.
pub const RAID: &str = "first_raid";

/// How a newcomer plays.
#[derive(Debug)]
pub struct Persona {
    pub name: &'static str,
    /// The scenario that runs this one alone.
    pub scenario: &'static str,
    /// Runs everywhere (the others walk).
    pub sprint: bool,
    /// Stops at each board and reads it before doing anything.
    pub reads: bool,
    /// Goes the wrong way first, and has to be put right by what's shown.
    pub lost: bool,
    /// Picks up everything loose it passes.
    pub hoards: bool,
    /// Walks into things it's told to keep out of.
    pub reckless: bool,
    /// Plays with one hand (`Comfort::one_handed`).
    pub one_handed: bool,
}

const fn persona(name: &'static str, scenario: &'static str) -> Persona {
    Persona {
        name,
        scenario,
        sprint: false,
        reads: false,
        lost: false,
        hoards: false,
        reckless: false,
        one_handed: false,
    }
}

pub const PERSONAS: [Persona; 5] = [
    Persona {
        reads: true,
        ..persona("cautious", "first_hour_cautious")
    },
    Persona {
        sprint: true,
        reckless: true,
        ..persona("reckless", "first_hour_reckless")
    },
    Persona {
        lost: true,
        ..persona("lost", "first_hour_lost")
    },
    Persona {
        hoards: true,
        ..persona("hoarder", "first_hour_hoarder")
    },
    Persona {
        one_handed: true,
        ..persona("one-handed", "first_hour_one_handed")
    },
];

/// A newcomer by name ("cautious", "reckless", "lost", "hoarder",
/// "one-handed"), for other bots that want one's ways.
pub fn named(name: &str) -> Option<&'static Persona> {
    PERSONAS.iter().find(|p| p.name == name)
}

/// How `p` goes on foot: (sprinting, crouched). Out in the Zone the
/// cautious one creeps; anyone runs when told to run for it (`fleeing`).
pub fn gait(p: &Persona, fleeing: bool) -> (bool, bool) {
    (p.sprint || fleeing, p.reads && !fleeing)
}

/// This session's newcomer (`--personality NAME`), in a first-hour run.
pub fn current(opts: &LaunchOptions) -> Option<&'static Persona> {
    if !matches!(opts.scenario.as_deref(), Some(YARD | RAID)) {
        return None;
    }
    let name = opts.personality.as_deref()?;
    PERSONAS.iter().find(|p| p.name == name)
}

/// The first fights: each newcomer's first raid with one machine on the
/// route (`first_fight`, or `first_fight_<encounter>` for one kind).
pub const FIGHT: &str = "first_fight";
/// The machines a first fight can be with.
pub const ENCOUNTERS: [&str; 5] = ["walker", "drone", "hush", "sentry", "night"];

/// What a first-hour or first-fight `--scenario` asks for: the newcomers,
/// and the encounters (none: the plain first hour, yard and raid).
pub struct Chain {
    pub personas: Vec<&'static Persona>,
    pub encounters: Vec<&'static str>,
    /// Raids after the yard, one a job (the first hour is one).
    pub raids: u32,
}

/// The first five jobs, by every newcomer: the yard, then a raid a job on
/// the one career (`first_jobs`, or `first_jobs_<name>` for one).
pub const JOBS: &str = "first_jobs";
/// How many raids `first_jobs` runs.
pub const JOBS_RAIDS: u32 = 5;

/// The chain a `--scenario` name asks for, if it's one of these.
pub fn chain(scenario: &str) -> Option<Chain> {
    let all = || PERSONAS.iter().collect::<Vec<_>>();
    if scenario == SCENARIO {
        return Some(Chain {
            personas: all(),
            encounters: vec![],
            raids: 1,
        });
    }
    if scenario == JOBS {
        return Some(Chain {
            personas: all(),
            encounters: vec![],
            raids: JOBS_RAIDS,
        });
    }
    if let Some(name) = scenario.strip_prefix("first_jobs_") {
        let p = PERSONAS.iter().find(|p| p.name.replace('-', "_") == name)?;
        return Some(Chain {
            personas: vec![p],
            encounters: vec![],
            raids: JOBS_RAIDS,
        });
    }
    if scenario == FIGHT {
        return Some(Chain {
            personas: all(),
            encounters: ENCOUNTERS.to_vec(),
            raids: 1,
        });
    }
    if let Some(kind) = scenario.strip_prefix("first_fight_") {
        let e = ENCOUNTERS.iter().find(|e| **e == kind)?;
        return Some(Chain {
            personas: all(),
            encounters: vec![e],
            raids: 1,
        });
    }
    PERSONAS
        .iter()
        .find(|p| p.scenario == scenario)
        .map(|p| Chain {
            personas: vec![p],
            encounters: vec![],
            raids: 1,
        })
}

/// Where a newcomer's two sessions keep the career.
fn folder(p: &Persona) -> PathBuf {
    PathBuf::from("target/first_hour").join(p.name)
}

/// Runs each newcomer's two sessions, a fresh career each, and says how
/// each went; or, for first fights, each newcomer's raid against each
/// encounter, from a fresh career that's done the yard.
pub fn run_chain(chain: &Chain) -> AppExit {
    if !chain.encounters.is_empty() {
        return run_fights(chain);
    }
    let personas = &chain.personas;
    let exe = std::env::current_exe().expect("own executable");
    // Everything else on the command line goes to both sessions.
    let passthrough: Vec<String> = std::env::args()
        .skip(1)
        .scan(false, |skip, a| {
            let keep = !*skip && a != "--scenario";
            *skip = a == "--scenario";
            Some(keep.then_some(a))
        })
        .flatten()
        .collect();
    let mut failed = Vec::new();
    for p in personas {
        let dir = folder(p);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let dir_s = dir.to_string_lossy().to_string();
        let save = dir.join("career.ron").to_string_lossy().to_string();
        let mut ok = true;
        let stages = std::iter::once((YARD, vec![])).chain(
            // As the cord would open it: on to the next valley; then a raid
            // a job.
            (0..chain.raids).map(|_| (RAID, vec!["--map".to_string(), "marrow_creek".to_string()])),
        );
        for (n, (stage, extra)) in stages.enumerate() {
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(&passthrough)
                .args(["--scenario", stage, "--personality", p.name])
                .args(["--data-dir", &dir_s, "--save", &save])
                .args(&extra);
            if p.one_handed {
                cmd.arg("--one-handed");
            }
            println!("first hour: {} in {stage} ({n})", p.name);
            let status = cmd.status();
            if !status.is_ok_and(|s| s.success()) {
                ok = false;
            }
        }
        if !ok {
            failed.push(p.name);
        }
    }
    if failed.is_empty() {
        println!("first hour: all {} newcomers through", personas.len());
        AppExit::Success
    } else {
        println!("first hour: FAILED for {failed:?}");
        AppExit::error()
    }
}

fn passthrough() -> Vec<String> {
    std::env::args()
        .skip(1)
        .scan(false, |skip, a| {
            let keep = !*skip && a != "--scenario";
            *skip = a == "--scenario";
            Some(keep.then_some(a))
        })
        .flatten()
        .collect()
}

fn run_fights(chain: &Chain) -> AppExit {
    let exe = std::env::current_exe().expect("own executable");
    let pass = passthrough();
    let mut failed = Vec::new();
    for e in &chain.encounters {
        for p in &chain.personas {
            let dir = PathBuf::from("target/first_fight").join(e).join(p.name);
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::create_dir_all(&dir);
            // Through the yard already.
            let _ = std::fs::write(dir.join("trained"), "yes\n");
            let dir_s = dir.to_string_lossy().to_string();
            let save = dir.join("career.ron").to_string_lossy().to_string();
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(&pass)
                .args([
                    "--scenario",
                    RAID,
                    "--personality",
                    p.name,
                    "--encounter",
                    e,
                ])
                .args([
                    "--data-dir",
                    &dir_s,
                    "--save",
                    &save,
                    "--map",
                    "marrow_creek",
                ]);
            if p.one_handed {
                cmd.arg("--one-handed");
            }
            if *e == "night" {
                cmd.args(["--hour", "night"]);
            }
            println!("first fight: {} meets the {e}", p.name);
            if !cmd.status().is_ok_and(|s| s.success()) {
                failed.push(format!("{}/{e}", p.name));
            }
        }
    }
    if failed.is_empty() {
        println!("first fight: every newcomer came through");
        AppExit::Success
    } else {
        println!("first fight: FAILED for {failed:?}");
        AppExit::error()
    }
}

/// The raid's first task: the director fills in the rest.
pub fn opening() -> Vec<Task> {
    vec![
        Task::Log("first raid: a newcomer's, from the camp".into()),
        Task::Wait(IDLE),
    ]
}

/// The last task while the raid runs.
pub(super) const IDLE: f32 = 4.25;

// ---------------------------------------------------------------------------
// The first raid, as a newcomer plays it.

use crate::bot::Bot;
use crate::interact::body::Backpack;
use crate::raid::bench::ScreenText;
use crate::raid::requests::RequestCard;
use crate::raid::truck::Stash;
use crate::raid::{Phase, Raid};
use crate::rig::{LocalPlayer, Side};
use extraction_core::contracts::Subject;
use extraction_core::map::MarkerKind;
use extraction_core::progression::{Job, LEDGER_ITEM, LETTER_ITEM, RECOVER_ITEM, fetch_takes};

use super::firsts::{First, FirstMet};
use super::second_hour::World2;

/// What happened to a newcomer worth writing down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Learned something, from what was shown or said.
    Learned,
    /// Had to guess: what it needed wasn't shown.
    Confused,
    /// Couldn't get on (a walk that never arrived, a step that failed).
    Stuck,
    /// Hurt, with a warning in the last half minute.
    Hurt,
    /// Hurt with nothing said or shown first.
    HurtUnwarned,
    /// Went down.
    Killed,
}

/// Where the raid has got to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Leg {
    #[default]
    Camp,
    Kit,
    Card,
    Out,
    Place,
    Call,
    Rendezvous,
    Board,
    /// Home: the card pressed, for the job's pay.
    Paid,
    /// Paid: what the driver passes through the hatch after (the pay, the
    /// next contract's kit), off the ledge.
    Handed,
    Done,
}

/// `--job KIND`: the career's job, so a kind that seldom comes up on the
/// ladder runs live (the place it's put is the game's own).
pub fn seeded_job(kind: &str) -> Option<Job> {
    let map = "marrow_creek".to_owned();
    Some(match kind {
        "recover" => Job::Recover {
            map,
            region: "Village".into(),
            at: None,
            place: None,
            overnight: false,
        },
        "letter" => Job::Letter,
        "ledger" => Job::Ledger,
        "photograph" => Job::Photograph {
            subject: Subject::Walker,
            taken: false,
        },
        "fetch" => Job::Fetch {
            item: "canned_stew".into(),
            n: 2,
        },
        _ => return None,
    })
}

pub(super) fn seed_job(
    opts: Res<LaunchOptions>,
    progression: Option<ResMut<crate::progression::Progression>>,
    mut done: Local<bool>,
) {
    let (Some(kind), Some(mut progression)) = (opts.job.as_deref(), progression) else {
        return;
    };
    if *done || opts.scenario.as_deref() != Some(RAID) {
        return;
    }
    *done = true;
    match seeded_job(kind) {
        Some(job) => {
            info!("first raid: seeded job {job:?}");
            progression.0.job = Some(job);
            // Its card alone, not a contract's head over it.
            progression.0.contract = None;
        }
        None => warn!("--job {kind}: not a kind I can seed"),
    }
}

/// What a job asks of a newcomer, the way one goes about it. Every kind of
/// job has one (`every_job_kind_has_a_way`): the first-hour record never
/// stops at a job the newcomer can't do.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Errand {
    /// Go and look (a scout): the driver says when it's seen.
    Look,
    /// Find `n` of what `want` stands for (round `near`, where the card
    /// says), bring them home, into the stash crate.
    Bring {
        want: String,
        n: u32,
        near: Option<Vec2>,
    },
    /// Take `item` there and leave it, then home.
    Leave { item: String, at: Vec2 },
    /// Take a photograph of a machine, then home.
    Photo(Subject),
}

pub(super) fn errand(job: &Job) -> Errand {
    let bring = |want: &str, near: Option<[f32; 2]>| Errand::Bring {
        want: want.to_owned(),
        n: 1,
        near: near.map(Vec2::from),
    };
    match job {
        Job::Scout { .. } => Errand::Look,
        Job::Fetch { item, n } => Errand::Bring {
            want: item.clone(),
            n: *n,
            near: None,
        },
        Job::Recover { at, .. } => bring(RECOVER_ITEM, *at),
        Job::Letter => bring(LETTER_ITEM, None),
        Job::Ledger => bring(LEDGER_ITEM, None),
        Job::Bring { item, at, .. } => bring(item, Some(*at)),
        Job::Plant { item, at, .. } => Errand::Leave {
            item: item.clone(),
            at: Vec2::from(*at),
        },
        Job::Photograph { subject, .. } => Errand::Photo(*subject),
    }
}

#[derive(Default)]
pub(super) struct Newcomer {
    leg: Leg,
    tries: u32,
    /// What was written down, and when.
    moments: Vec<(f32, Kind, String)>,
    /// Where it's headed and what that's called.
    goal: Option<(String, Vec2)>,
    /// The rendezvous it's walking to.
    rv: Option<Vec2>,
    /// The last thing said, and when (for "warned").
    said_at: f32,
    /// Everything said so far, in order (the log forgets after a few
    /// seconds; a person remembers).
    heard: Vec<String>,
    /// What the log held last frame.
    showing: Vec<String>,
    /// The job card, as read.
    card: String,
    /// Back at the gate to call (the card said so).
    at_gate: bool,
    /// Jobs done when it got home (pressing the card adds one).
    done_home: Option<u32>,
    /// Keeping low till then (backing off behind cover).
    low_until: f32,
    /// When it last did as a machine's tell says (not more than every 15 s).
    tell_at: f32,
    /// What the job asks.
    errand: Option<Errand>,
    /// Getting what it brought into the crate.
    crating: Option<crate::bot::crating::Crating>,
    /// The find it last went for; and finds it went for and couldn't get
    /// (out of reach, behind something): it looks for another.
    reaching: Option<Entity>,
    passed_over: Vec<Entity>,
    /// Finds the rooms' plan didn't get it to (a gap the plan allows and
    /// the body doesn't pass): the plain way in, at the door, next.
    plan_failed: Vec<Entity>,
    hp: f32,
    failed_seen: usize,
    reported: bool,
    wrong_way_done: bool,
    /// When the reckless one last picked a fight.
    fought_at: f32,
}

impl Newcomer {
    fn note(&mut self, now: f32, kind: Kind, what: impl Into<String>) {
        let what = what.into();
        info!("first hour: {now:.0}s {kind:?}: {what}");
        self.moments.push((now, kind, what));
    }
}

/// The card's place: "Go up to the Old farm and look round..." gives the
/// name, and the map sheet has the name written where it is.
fn place_on_card(text: &str, zone: &crate::world::Zone) -> Option<(String, Vec2)> {
    // The card wraps and lower-cases: read it as one line.
    let flat = |t: &str| {
        t.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let text = flat(text);
    // The job, not the driver's cache under it; and the place it names
    // first (the longest name where two start there: "the old farm road").
    let job = text.split("my cache").next().unwrap_or(&text);
    zone.map
        .markers
        .iter()
        .filter(|m| m.kind == MarkerKind::Poi && !m.name.is_empty())
        .filter_map(|m| job.find(&flat(&m.name)).map(|i| (i, m)))
        .min_by_key(|(i, m)| (*i, std::cmp::Reverse(m.name.len())))
        .map(|(_, m)| (m.name.clone(), Vec2::from(m.at)))
}

/// What a line gives of a place's way and distance ("north-east, 200 m"),
/// if it gives any.
fn heading_in(line: &str) -> Option<(Vec2, f32)> {
    const DIRS: [(&str, Vec2); 8] = [
        ("north-east", Vec2::new(1.0, -1.0)),
        ("north-west", Vec2::new(-1.0, -1.0)),
        ("south-east", Vec2::new(1.0, 1.0)),
        ("south-west", Vec2::new(-1.0, 1.0)),
        ("north", Vec2::new(0.0, -1.0)),
        ("south", Vec2::new(0.0, 1.0)),
        ("east", Vec2::new(1.0, 0.0)),
        ("west", Vec2::new(-1.0, 0.0)),
    ];
    let l = line.to_lowercase();
    let (_, dir) = DIRS.iter().find(|(w, _)| l.contains(w))?;
    let metres = l
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .find_map(|s| {
            let i = l.find(s)?;
            l[i + s.len()..]
                .trim_start()
                .starts_with('m')
                .then(|| s.parse::<f32>().ok())?
        })?;
    Some((dir.normalize(), metres))
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct RaidView<'w, 's> {
    stash: Query<'w, 's, &'static Backpack, With<Stash>>,
    card: Query<'w, 's, (Entity, &'static ScreenText), With<RequestCard>>,
    progression: Option<Res<'w, crate::progression::Progression>>,
    hatch: Res<'w, crate::raid::lev::Handovers>,
    buildings: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::world::buildings::Building,
            &'static Transform,
        ),
    >,
    leaves: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::world::buildings::DoorLeaf,
            &'static ChildOf,
        ),
    >,
    bags: Query<'w, 's, &'static Backpack>,
    entrances: Option<Res<'w, crate::robots::doors::Entrances>>,
    robots: Query<
        'w,
        's,
        (
            &'static crate::robots::Robot,
            &'static GlobalTransform,
            Option<&'static crate::robots::Breed>,
        ),
    >,
    trucks: Query<'w, 's, &'static GlobalTransform, With<crate::raid::truck::Truck>>,
    health: Query<'w, 's, &'static crate::health::Health, With<LocalPlayer>>,
}

/// Kit a newcomer takes from the crate: what the yard used, and the map
/// and compass (a hoarder takes the lot).
const KIT: [&str; 9] = [
    "pm_pistol",
    "pm_mag",
    "radio",
    "field_dressing",
    "flashlight",
    "map_case",
    "compass",
    "bolt",
    "detector",
];

#[allow(clippy::too_many_arguments)]
pub(super) fn direct_raid(
    opts: Res<LaunchOptions>,
    mut bots: Query<(Entity, &mut Bot, &Transform, &mut crate::rig::RigInput), With<LocalPlayer>>,
    world: World2,
    view: RaidView,
    raid: Res<Raid>,
    mut met: MessageReader<FirstMet>,
    mut fight: Option<ResMut<Fight>>,
    mut state: Local<Newcomer>,
    crates: crate::bot::crating::CrateWorld,
    mut tells: Tells,
    minds: Query<&crate::robots::Awareness>,
    mut plans: Local<Vec<crate::sim::indoor::BuildingPlan>>,
    time: Res<Time>,
) {
    if opts.scenario.as_deref() != Some(RAID) {
        return;
    }
    let Some(p) = current(&opts) else {
        return;
    };
    let Some((player, mut bot, root, mut input)) = bots.iter_mut().next() else {
        return;
    };
    let now = time.elapsed_secs();
    let here = root.translation.xz();
    // A cautious newcomer creeps once it's out.
    // Running for it, as told: upright and fast.
    let fleeing = fight.as_ref().is_some_and(|f| f.flee_until > now);
    let creep = (p.reads || state.low_until > now)
        && !fleeing
        && !matches!(state.leg, Leg::Camp | Leg::Kit | Leg::Card | Leg::Board);
    // A photograph: the head kept on the machine as it moves, the camera
    // up (the card: "it in the middle").
    if let (Leg::Place, Some(Errand::Photo(subject))) = (state.leg, &state.errand)
        && let Some(p) =
            photo_subject(&view, *subject, here).filter(|p| p.xz().distance(here) < 60.0)
    {
        bot.look = p + Vec3::Y * 1.2;
    }
    // (Bent down to the crate: that's the crating's to say.)
    let crating = state.crating.as_ref().is_some_and(|c| !c.done());
    if input.crouch != creep && !crating {
        input.crouch = creep;
    }
    bot.sprint = p.sprint || fleeing;

    // Heard: what the driver says is what it knows.
    let showing: Vec<String> = raid.log.iter().map(|(l, _)| l.clone()).collect();
    for l in &showing {
        if !state.showing.contains(l) {
            state.heard.push(l.clone());
            state.said_at = now;
        }
    }
    state.showing = showing;
    // The first time, what the driver says; after, the machine's own tell
    // (the chirp, the gun settling): a newcomer who's learned what one
    // means keeps doing as told.
    let firsts: Vec<First> = met.read().map(|m| m.0).collect();
    let on_me = |r: Entity| minds.get(r).is_ok_and(|a| a.target == Some(player));
    let mut again: Vec<First> = vec![];
    if tells.locks.read().any(|ev| on_me(ev.drone)) {
        again.push(First::DroneLock);
    }
    if tells.aims.read().any(|ev| on_me(ev.walker)) {
        again.push(First::Aimed);
    }
    if tells.sentries.read().any(|ev| {
        ev.what == crate::robots::sentry::SentryAct::Chirp && ev.at.xz().distance(here) < 60.0
    }) {
        again.push(First::Sentry);
    }
    if !again.is_empty() {
        // Warned: by the machine itself.
        state.said_at = now;
    }
    again.retain(|f| !firsts.contains(f));
    again.truncate(1);
    if now - state.tell_at < 15.0 {
        again.clear();
    }
    if !again.is_empty() {
        state.tell_at = now;
    }
    let moments = firsts
        .into_iter()
        .map(|f| (f, true))
        .chain(again.into_iter().map(|f| (f, false)));
    for (f, first) in moments {
        if first {
            state.note(now, Kind::Learned, format!("{f:?}: {}", f.lines()[0]));
        } else {
            state.note(
                now,
                Kind::Learned,
                format!("{f:?} again: as the driver said before"),
            );
        }
        let answer: Vec<Task> = match f {
            First::Jam(_) => vec![Task::Gun(crate::weapons::flat::MacroKind::Rack)],
            First::Heard if !p.reckless => vec![Task::Wait(15.0)],
            // "Run across it, to cover", "Break its line", "Down, out of
            // its sweep": for it, as told.
            // "Down, out of its sweep": low, and away from it (a sentry
            // doesn't come after you).
            First::Sentry if !p.reckless && !p.hoards => {
                let from = world.nearest_robot(here).unwrap_or(here + Vec2::Y * 20.0);
                state.low_until = now + 25.0;
                run_for_cover(&world.zone, here, from, 1.0)
            }
            First::DroneLock | First::Aimed if !p.reckless && !p.hoards => {
                let from = world.nearest_robot(here).unwrap_or(here + Vec2::Y * 20.0);
                if let Some(f) = fight.as_mut() {
                    f.flee_until = now + 12.0;
                }
                let overhead = f == First::DroneLock;
                overhead
                    .then(|| under_a_roof(view.entrances.as_deref(), here))
                    .flatten()
                    .unwrap_or_else(|| run_for_cover(&world.zone, here, from, 0.0))
            }
            // "Low behind it, and keep still": a still shape behind cover
            // isn't glimpsed; a moving one is, and found.
            First::ComingToLook if !p.reckless && !p.hoards => {
                if let Some(f) = fight.as_mut() {
                    f.flee_until = 0.0;
                }
                state.low_until = now + 30.0;
                vec![Task::Wait(30.0)]
            }
            First::Bleeding => {
                let dressing = world.items.iter().find(|(_, it, _, held, sock, _)| {
                    held.is_none()
                        && sock.is_some()
                        && world.catalog.get(it.kind()).id == "field_dressing"
                });
                match dressing {
                    Some((e, ..)) => vec![
                        Task::Stow(Side::Left),
                        Task::Grab {
                            item: e,
                            side: Side::Left,
                        },
                        Task::Use(Side::Left),
                    ],
                    None => {
                        state.note(now, Kind::Confused, "bleeding, and no dressing on me");
                        vec![]
                    }
                }
            }
            First::Anomaly => {
                // Back the way it came, then on round.
                let back = state
                    .goal
                    .as_ref()
                    .map_or(here, |(_, g)| here - (*g - here).normalize_or_zero() * 4.0);
                vec![Task::WalkTo {
                    to: back,
                    within: 1.0,
                }]
            }
            First::Surge => {
                state.leg = Leg::Call;
                vec![]
            }
            _ => vec![],
        };
        if !answer.is_empty() {
            super::second_hour::cut_in(&mut bot, answer);
        }
    }

    // Hurt: warned, or not.
    if let Ok(h) = view.health.single() {
        if h.hp < state.hp - 5.0 {
            // Warned: the driver said something lately, or a machine
            // showed its tell (`first_fight`).
            // In a fight, warned is the machine's own tell; else something
            // said lately.
            let warned = match fight.as_ref().filter(|f| f.machine.is_some()) {
                Some(f) => f.last_tell().is_some_and(|t| now - t < 15.0),
                None => now - state.said_at < 30.0,
            };
            let kind = if warned {
                Kind::Hurt
            } else {
                Kind::HurtUnwarned
            };
            let last = state.heard.last().cloned().unwrap_or_default();
            let was = state.hp;
            state.note(
                now,
                kind,
                format!("hp {was:.0} -> {:.0} (last said: {last})", h.hp),
            );
        }
        state.hp = h.hp;
    }
    // A step the bot couldn't do.
    while state.failed_seen < bot.log.len() {
        let line = bot.log[state.failed_seen].clone();
        state.failed_seen += 1;
        if line.trim_start().starts_with("FAIL") && !state.reported {
            state.note(now, Kind::Stuck, line.trim().to_string());
        }
    }
    // A first raid that runs on past half an hour has lost its way.
    if now > 1800.0 && state.leg != Leg::Done {
        let leg = state.leg;
        state.note(
            now,
            Kind::Stuck,
            format!("half an hour gone, still at {leg:?}"),
        );
        state.leg = Leg::Done;
    }
    // Woken from the last one's fall (a career resumed while down): not
    // this raid's death; wait for the truck.
    if matches!(raid.phase, Phase::Down { .. }) && state.leg == Leg::Camp {
        return;
    }
    if matches!(raid.phase, Phase::Down { .. }) && state.leg != Leg::Done {
        state.note(now, Kind::Killed, "went down");
        state.leg = Leg::Done;
        // That's this first hour over: nothing left of the walk.
        bot.tasks.clear();
        bot.tasks.push_back(Task::Wait(IDLE));
        bot.restart();
    }

    let idle_only =
        bot.tasks.len() == 1 && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE);
    if !idle_only || bot.elapsed < 0.5 {
        return;
    }
    // Reckless: a machine in reach is a fight (once a minute at most).
    // (With a gun: the last fall may have left it out there.)
    let armed = world.items.iter().any(|(e, it, _, held, sock, bag)| {
        world.catalog.get(it.kind()).category == extraction_core::items::Category::Firearm
            && (held.is_some_and(|h| h.player == player)
                || sock.is_some_and(|s| world.owners.get(s.socket).is_ok_and(|o| o.0 == player))
                || bag.is_some_and(|b| view.bags.get(b.bag).is_ok_and(|b| b.owner == player)))
            && !view.stash.iter().any(|b| b.contents.contains(&e))
    });
    if p.reckless
        && armed
        && now - state.fought_at > 60.0
        && matches!(state.leg, Leg::Place | Leg::Rendezvous | Leg::Call)
        && world
            .nearest_robot(here)
            .is_some_and(|r| r.distance(here) < 40.0)
    {
        state.fought_at = now;
        state.note(now, Kind::Learned, "a machine in range: took it on");
        super::second_hour::cut_in(&mut bot, vec![Task::DrawGun, Task::Engage { shots: 12 }]);
        return;
    }
    // The truck's there: whatever the walk, to it now.
    if state.leg == Leg::Rendezvous
        && matches!(raid.phase, Phase::TruckWaiting { .. })
        && !matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE)
    {
        state.note(now, Kind::Learned, "the truck's there");
        bot.tasks.retain(|t| !matches!(t, Task::WalkTo { .. }));
        let t = world.go_home(here, &raid, 0);
        super::second_hour::cut_in(&mut bot, t);
        state.leg = Leg::Board;
        return;
    }
    let truck = view.trucks.iter().next().map(|t| t.compute_transform());
    let at_truck =
        |x: f32, z: f32| truck.map_or(here, |t| t.transform_point(Vec3::new(x, 0.0, z)).xz());
    let mut tasks: Vec<Task> = Vec::new();
    match state.leg {
        Leg::Camp => {
            if p.reads {
                tasks.push(Task::Wait(4.0));
            }
            state.note(
                now,
                Kind::Learned,
                "camp: \"Your kit is in the crate in the truck\"",
            );
            // Round to the tailgate and up into the bed.
            tasks.extend(route(&world, &view, here, at_truck(0.0, 4.0)));
            for (x, z) in [(0.05, 0.3), (-0.2, -0.6)] {
                tasks.push(Task::WalkTo {
                    to: at_truck(x, z),
                    within: 0.3,
                });
            }
            state.leg = Leg::Kit;
        }
        Leg::Kit => {
            let mut in_crate: Vec<(Entity, String)> = view
                .stash
                .iter()
                .flat_map(|b| b.contents.iter().copied())
                .filter_map(|e| world.id(e).map(|id| (e, id.to_string())))
                .collect();
            // What the yard used first, to the pockets; anything more after
            // (a hoarder's radio went to the bottom of the pack, out of
            // reach of a hand).
            in_crate.sort_by_key(|(_, id)| !KIT.contains(&id.as_str()));
            // And what the job takes out (the card says: the relay box, the
            // camera), if it's in there.
            let out: Option<String> = view
                .progression
                .as_ref()
                .and_then(|p| p.0.job.as_ref())
                .and_then(|j| match errand(j) {
                    Errand::Leave { item, .. } => Some(item),
                    Errand::Photo(_) => Some("camera".to_owned()),
                    _ => None,
                });
            for (e, id) in in_crate {
                if p.hoards || KIT.contains(&id.as_str()) || out.as_deref() == Some(id.as_str()) {
                    tasks.push(Task::Grab {
                        item: e,
                        side: Side::Right,
                    });
                    tasks.push(Task::Stow(Side::Right));
                }
            }
            state.errand = view
                .progression
                .as_ref()
                .and_then(|p| p.0.job.as_ref())
                .map(errand);
            // Off the ledge under the hatch, what the driver passes through:
            // his spare radio (the last one went with the rest at a fall),
            // and what the job takes out (a thing to leave, the camera).
            let mut need = vec![];
            let in_crate = |id: &str| {
                view.stash
                    .iter()
                    .flat_map(|b| b.contents.iter())
                    .any(|e| world.id(*e) == Some(id))
            };
            if carried(&world, &view, player, "radio").is_empty() && !in_crate("radio") {
                need.push("radio".to_owned());
            }
            match &state.errand {
                Some(Errand::Leave { item, .. })
                    if carried(&world, &view, player, item).is_empty() =>
                {
                    need.push(item.clone())
                }
                Some(Errand::Photo(_)) if carried(&world, &view, player, "camera").is_empty() => {
                    need.push("camera".to_owned())
                }
                _ => {}
            }
            let mut missing = vec![];
            for id in need {
                let on_ledge = world.items.iter().find(|(e, it, gt, held, sock, bag)| {
                    held.is_none()
                        && sock.is_none()
                        && bag.is_none()
                        && world.catalog.get(it.kind()).id == id
                        && gt.translation().xz().distance(at_truck(0.0, 0.0)) < 4.0
                        && !view.stash.iter().any(|b| b.contents.contains(e))
                });
                match on_ledge {
                    Some((e, ..)) => {
                        if state.heard.iter().any(|l| l.contains("off the ledge")) {
                            state.note(
                                now,
                                Kind::Learned,
                                format!("{id} off the ledge, as the driver said"),
                            );
                        }
                        tasks.push(Task::Grab {
                            item: e,
                            side: Side::Right,
                        });
                        tasks.push(Task::Stow(Side::Right));
                    }
                    None => missing.push(id),
                }
            }
            if !missing.is_empty() && state.tries < 15 {
                // Still coming through the hatch, perhaps: a moment.
                state.tries += 1;
                tasks.push(Task::Wait(2.0));
                let idle = bot.tasks.pop_back();
                bot.tasks.extend(tasks);
                bot.tasks.extend(idle);
                bot.restart();
                return;
            }
            for id in missing {
                state.note(
                    now,
                    Kind::Confused,
                    format!("no {id}, and none handed over"),
                );
            }
            state.tries = 0;
            state.leg = Leg::Card;
        }
        Leg::Card => {
            let text = view
                .card
                .iter()
                .next()
                .map(|(_, c)| c.text.clone())
                .unwrap_or_default();
            if p.reads {
                tasks.push(Task::Wait(4.0));
            }
            state.card = text.split_whitespace().collect::<Vec<_>>().join(" ");
            match place_on_card(&text, &world.zone) {
                Some((name, at)) => {
                    state.note(
                        now,
                        Kind::Learned,
                        format!("the job card: {}", text.replace('\n', " ")),
                    );
                    if heading_in(&text).is_none() {
                        state.note(
                            now,
                            Kind::Confused,
                            format!("the card names {name} but not which way or how far: found it on the map sheet"),
                        );
                    }
                    state.goal = Some((name, at));
                    state.leg = Leg::Out;
                }
                None if matches!(
                    state.errand,
                    Some(Errand::Bring { near: None, .. } | Errand::Photo(_))
                ) =>
                {
                    // A fetch, a photograph: no place, things to find
                    // anywhere.
                    state.note(
                        now,
                        Kind::Learned,
                        format!("the job card: {}", text.replace('\n', " ")),
                    );
                    state.goal = None;
                    tasks.extend(route(&world, &view, here, at_truck(0.0, 4.0)));
                    state.leg = Leg::Place;
                }
                None => {
                    state.note(
                        now,
                        Kind::Confused,
                        format!("no place I can find on the card: {text:?}"),
                    );
                    state.leg = Leg::Call;
                }
            }
        }
        Leg::Out => {
            let Some((name, at)) = state.goal.clone() else {
                state.leg = Leg::Call;
                return;
            };
            // Down over the tailgate, not into the side of the bed.
            let mut from = here;
            if truck.is_some_and(|t| t.translation.xz().distance(here) < 3.5) {
                for (x, z) in [(0.05, 1.2), (0.0, 4.0)] {
                    tasks.push(Task::WalkTo {
                        to: at_truck(x, z),
                        within: 0.5,
                    });
                }
                from = at_truck(0.0, 4.0);
                // "Walk out through the gate": the drop point is on the
                // far side of it.
                if let Some(drop) = world
                    .zone
                    .map
                    .markers
                    .iter()
                    .find(|m| m.kind == MarkerKind::Insertion)
                    .map(|m| Vec2::from(m.at))
                {
                    tasks.extend(route(&world, &view, from, drop));
                    from = drop;
                }
            }
            // Lost: off the wrong way first, until the map puts it right.
            if p.lost && !state.wrong_way_done {
                state.wrong_way_done = true;
                // Off at right angles (the wrong road), into the valley
                // rather than back into the fence.
                let side = (at - from).normalize_or_zero().perp();
                let side = if side.dot(-from) >= 0.0 { side } else { -side };
                let wrong = from + side * 70.0;
                state.note(now, Kind::Confused, "set off the wrong way");
                tasks.extend(route(&world, &view, from, wrong));
                from = wrong;
            }
            tasks.extend(route(&world, &view, from, at));
            state.note(now, Kind::Learned, format!("on to {name}"));
            state.leg = Leg::Place;
        }
        Leg::Place if state.errand.as_ref().is_some_and(|e| *e != Errand::Look) => {
            let errand = state.errand.clone().unwrap_or(Errand::Look);
            // Went for it, and it's still lying there: past it, to another.
            if let Some(e) = state.reaching.take()
                && world
                    .items
                    .get(e)
                    .is_ok_and(|(_, _, _, h, s, b)| h.is_none() && s.is_none() && b.is_none())
            {
                if state.plan_failed.contains(&e) {
                    state.passed_over.push(e);
                } else {
                    state.plan_failed.push(e);
                }
            }
            let skip = state.passed_over.clone();
            let no_plan = state.plan_failed.clone();
            if plans.is_empty() && !view.buildings.is_empty() {
                *plans = crate::sim::indoor::plan_buildings(&view.buildings, &view.leaves);
            }
            match errand_tasks(
                &errand,
                &world,
                &view,
                &plans,
                player,
                here,
                state.tries,
                &skip,
                &no_plan,
            ) {
                Ok(Some((t, after))) => {
                    tasks.extend(t);
                    state.reaching = after;
                    state.tries += 1;
                }
                Ok(None) => {
                    state.note(now, Kind::Learned, errand_done(&errand, &world));
                    tasks.extend(way_out(&world, &view, here).0);
                    state.leg = Leg::Call;
                    state.tries = 0;
                }
                Err(why) => {
                    state.note(now, Kind::Stuck, why);
                    tasks.extend(way_out(&world, &view, here).0);
                    state.leg = Leg::Call;
                    state.tries = 0;
                }
            }
        }
        Leg::Place => {
            let noted = state.heard.iter().any(|l| l.contains("noted"));
            let close = state
                .goal
                .as_ref()
                .is_some_and(|(_, g)| g.distance(here) < 12.0);
            if noted {
                state.note(now, Kind::Learned, "the place, noted: \"Now get it home\"");
                state.leg = Leg::Call;
            } else if close || state.tries >= 3 {
                // There, but not noted: about the place's middle.
                if let Some((_, g)) = state.goal.clone() {
                    tasks.extend(route(&world, &view, here, g));
                    tasks.push(Task::Wait(3.0));
                }
                state.tries += 1;
                if state.tries > 4 {
                    state.note(now, Kind::Stuck, "at the place, and nothing says it's seen");
                    state.leg = Leg::Call;
                    state.tries = 0;
                }
            } else if let Some((_, g)) = state.goal.clone() {
                tasks.extend(route(&world, &view, here, g));
                state.tries += 1;
            }
        }
        Leg::Call => {
            // "Call me from the gate after": back the way it came first.
            let drop = world
                .zone
                .map
                .markers
                .iter()
                .find(|m| m.kind == MarkerKind::Insertion)
                .map(|m| Vec2::from(m.at));
            if state.card.contains("from the gate")
                && !state.at_gate
                && raid.phase == Phase::Out
                && let Some(drop) = drop
            {
                state.at_gate = true;
                if drop.distance(here) > 20.0 {
                    state.note(now, Kind::Learned, "back to the gate, as the card said");
                    tasks.extend(route(&world, &view, here, drop));
                    let idle = bot.tasks.pop_back();
                    bot.tasks.extend(tasks);
                    bot.tasks.extend(idle);
                    bot.restart();
                    return;
                }
            }
            if let Phase::TruckWaiting { rv, .. } = raid.phase
                && rv == crate::raid::SHORT_EXFIL
            {
                // It came back for us at the gate: in.
                state.note(now, Kind::Learned, "the truck came back to the gate");
                tasks.extend(world.go_home(here, &raid, 0));
                state.leg = Leg::Board;
            } else if raid.phase == Phase::AtBase {
                // Still by the truck: out first (the radio's for the Zone).
                let away = here + (here - at_truck(0.0, 0.0)).normalize_or(Vec2::Y) * 20.0;
                tasks.extend(route(&world, &view, here, away));
            } else if let Phase::TruckCalled { .. } | Phase::TruckWaiting { .. } = raid.phase {
                // What it said: a name, maybe a way.
                // The truck's last word on where.
                let reply = state
                    .heard
                    .iter()
                    .rev()
                    .find(|l| {
                        l.starts_with("Truck:")
                            && (l.contains("minute") || l.contains("I'm at") || l.contains("At "))
                    })
                    .cloned()
                    .unwrap_or_default();
                let rvs: Vec<Vec2> = world
                    .zone
                    .map
                    .markers
                    .iter()
                    .filter(|m| m.kind == MarkerKind::Rendezvous)
                    .map(|m| Vec2::from(m.at))
                    .collect();
                let rv = match heading_in(&reply) {
                    Some((dir, m)) => {
                        let guess = here + dir * m;
                        rvs.iter()
                            .copied()
                            .min_by(|a, b| a.distance(guess).total_cmp(&b.distance(guess)))
                    }
                    None => {
                        state.note(
                            now,
                            Kind::Confused,
                            format!("{reply}: a name, and the map's rings only say RV; tried the nearest"),
                        );
                        rvs.iter()
                            .copied()
                            .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)))
                    }
                };
                state.rv = rv;
                if let Some(rv) = rv {
                    // Near it, to wait: not on the spot itself.
                    let near = rv + (here - rv).normalize_or(Vec2::Y) * 8.0;
                    tasks.extend(route(&world, &view, here, near));
                }
                state.leg = Leg::Rendezvous;
            } else if world.radio_in_hand() {
                tasks.extend([Task::Use(Side::Left), Task::Wait(1.0)]);
            } else {
                // Its own: on its body, not one lying about the valley.
                let radio = world.items.iter().find(|(_, it, _, held, sock, _)| {
                    held.is_none()
                        && world.catalog.get(it.kind()).id == "radio"
                        && sock.is_some_and(|s| {
                            world.owners.get(s.socket).is_ok_and(|o| o.0 == player)
                        })
                });
                // ...or in its pack: pulled round, out, put back.
                let in_pack = carried(&world, &view, player, "radio")
                    .into_iter()
                    .find(|e| world.items.get(*e).is_ok_and(|(.., bag)| bag.is_some()));
                match (radio, in_pack) {
                    (Some((e, ..)), _) => tasks.extend([
                        Task::Stow(Side::Left),
                        Task::Grab {
                            item: e,
                            side: Side::Left,
                        },
                    ]),
                    (None, Some(e)) => tasks.extend([
                        Task::Stow(Side::Left),
                        Task::ToggleBackpack,
                        Task::Wait(0.6),
                        Task::GrabFromHere {
                            item: e,
                            side: Side::Left,
                        },
                        Task::ToggleBackpack,
                        Task::Wait(0.5),
                    ]),
                    (None, None) => {
                        state.note(now, Kind::Confused, "no radio to call the truck with");
                        state.leg = Leg::Done;
                    }
                }
            }
        }
        Leg::Rendezvous => match raid.phase {
            Phase::TruckWaiting { .. } => {
                tasks.extend(world.go_home(here, &raid, state.tries));
                state.leg = Leg::Board;
            }
            Phase::TruckCalled { .. } => tasks.push(Task::Wait(5.0)),
            Phase::Out => {
                state.note(now, Kind::Confused, "the truck went without me");
                state.leg = Leg::Call;
            }
            _ => tasks.push(Task::Wait(2.0)),
        },
        Leg::Board => match raid.phase {
            Phase::Extracting { .. } | Phase::AtBase => {
                state.note(now, Kind::Learned, "in the truck and away");
                state.leg = Leg::Paid;
                state.tries = 0;
            }
            Phase::TruckWaiting { .. } => {
                tasks.extend(world.go_home(here, &raid, state.tries));
                state.tries += 1;
            }
            _ => {
                state.leg = Leg::Call;
            }
        },
        Leg::Paid => {
            let done = view.progression.as_ref().map_or(0, |p| p.0.done);
            let bring = match &state.errand {
                Some(Errand::Bring { want, .. }) => Some(want.clone()),
                _ => None,
            };
            if raid.phase == Phase::AtBase
                && let Some(want) = bring
                && state.crating.as_ref().is_none_or(|c| !c.done())
            {
                // "Stash it, press here": what it brought, into the crate.
                let crating = state.crating.get_or_insert_with(|| {
                    crate::bot::crating::Crating::new(carried(&world, &view, player, &want))
                });
                let t = crating.step(&crates, player, root, &mut input);
                if crating.done() {
                    let (n, missed) = (crating.stowed.len(), crating.missed.clone());
                    for (e, why) in missed {
                        let id = world.id(e).unwrap_or("?").to_owned();
                        state.note(now, Kind::Stuck, format!("{id}: {why}"));
                    }
                    if n > 0 {
                        state.note(now, Kind::Learned, format!("{n} into the stash crate"));
                    } else {
                        state.note(
                            now,
                            Kind::Confused,
                            "nothing of the job's to put in the crate",
                        );
                    }
                }
                tasks.extend(t);
                if tasks.is_empty() {
                    tasks.push(Task::Wait(0.5));
                }
            } else if raid.phase != Phase::AtBase {
                tasks.push(Task::Wait(1.0));
            } else if state.done_home.is_none() {
                state.done_home = Some(done);
                // Told to, by the driver, or read on the card.
                let told = state.heard.iter().any(|l| l.contains("press the card"));
                let read = view
                    .card
                    .iter()
                    .any(|(_, c)| c.text.to_lowercase().contains("press here"));
                if !told && !read {
                    state.note(
                        now,
                        Kind::Confused,
                        "home, and nothing says how the job is paid",
                    );
                }
                tasks.push(Task::Wait(if p.reads { 4.0 } else { 1.0 }));
            } else if state.done_home.is_some_and(|d| done > d) {
                state.note(now, Kind::Learned, "pressed the card: the job paid");
                state.leg = Leg::Handed;
                state.tries = 0;
            } else if state.tries >= 3 {
                state.note(now, Kind::Stuck, "home, and the card won't take the job");
                state.leg = Leg::Done;
            } else if let Some((card, _)) = view.card.iter().next() {
                tasks.push(Task::Press(card));
                tasks.push(Task::Wait(2.0));
                state.tries += 1;
            } else {
                state.leg = Leg::Done;
            }
        }
        Leg::Handed => {
            let ledge: Vec<Entity> = world
                .items
                .iter()
                .filter(|(e, _, gt, held, sock, bag)| {
                    held.is_none()
                        && sock.is_none()
                        && bag.is_none()
                        && gt.translation().xz().distance(at_truck(0.0, 0.0)) < 4.0
                        && view.hatch.handed.iter().any(|(_, h)| h == e)
                })
                .map(|(e, ..)| e)
                .collect();
            if view.hatch.busy() && state.tries < 30 {
                tasks.push(Task::Wait(1.0));
                state.tries += 1;
            } else if let Some(&e) = ledge.first().filter(|_| state.tries < 40) {
                let id = world.id(e).unwrap_or("?").to_owned();
                state.note(
                    now,
                    Kind::Learned,
                    format!("{id} off the ledge, from the driver"),
                );
                tasks.extend([
                    Task::Grab {
                        item: e,
                        side: Side::Right,
                    },
                    Task::Stow(Side::Right),
                ]);
                state.tries += 1;
            } else {
                state.leg = Leg::Done;
            }
        }
        Leg::Done => {
            if !state.reported {
                state.reported = true;
                // The fight, if there was one: each tell, and the first hit
                // against them.
                if let Some(f) = fight.as_ref().filter(|f| f.machine.is_some()) {
                    for (t, what) in &f.tells {
                        state
                            .moments
                            .push((*t, Kind::Learned, format!("tell: {what}")));
                    }
                    match (f.first_hit, f.tells.first()) {
                        (Some(hit), Some((t, what))) if *t < hit => state.moments.push((
                            hit,
                            Kind::Hurt,
                            format!("first hit, {:.1} s after {what}", hit - t),
                        )),
                        (Some(hit), _) => state.moments.push((
                            hit,
                            Kind::HurtUnwarned,
                            "first hit, before any tell".into(),
                        )),
                        (None, _) => {}
                    }
                    state.moments.sort_by(|a, b| a.0.total_cmp(&b.0));
                }
                report(&mut bot, p, &state.moments);
            }
            return;
        }
    }
    if !tasks.is_empty() {
        let idle = bot.tasks.pop_back();
        bot.tasks.extend(tasks);
        bot.tasks.extend(idle);
        bot.restart();
    }
}

/// The newcomer's table, in the bot's log: it passes when it got home and
/// nothing hurt it unwarned, stalled it or killed it.
fn report(bot: &mut Bot, p: &Persona, moments: &[(f32, Kind, String)]) {
    bot.log.push(format!("newcomer: {}", p.name));
    bot.log.push("| t | What | Moment |".into());
    bot.log.push("| - | - | - |".into());
    for (t, k, what) in moments {
        bot.log.push(format!(
            "| {:.0} s | {k:?} | {} |",
            t,
            what.replace('|', "/")
        ));
    }
    let bad = moments
        .iter()
        .filter(|(_, k, _)| matches!(k, Kind::Stuck | Kind::HurtUnwarned | Kind::Killed))
        .count();
    let home = moments.iter().any(|(_, _, w)| w == "in the truck and away");
    if home && bad == 0 {
        bot.passed += 1;
        bot.log.push(format!("PASS {} got home", p.name));
    } else {
        bot.failed += 1;
        bot.log.push(format!(
            "FAIL {}: {}{bad} stuck, hurt unwarned or killed",
            p.name,
            if home { "" } else { "never got home; " }
        ));
    }
}

// ---------------------------------------------------------------------------
// The first fight: one machine on the route, its tells, the first hit.

/// What the fight's watcher has seen: each kind of tell the first time it
/// came, and the first hit.
#[derive(Resource, Default, Debug)]
pub struct Fight {
    seeded: bool,
    /// Running for cover until then.
    pub flee_until: f32,
    pub tells: Vec<(f32, String)>,
    pub first_hit: Option<f32>,
    reacted: bool,
    hp: f32,
    /// What was put on the route, and where.
    pub machine: Option<(String, Vec2)>,
}

impl Fight {
    /// The last tell, if any.
    pub fn last_tell(&self) -> Option<f32> {
        self.tells.last().map(|(t, _)| *t)
    }

    fn tell(&mut self, now: f32, what: &str) -> bool {
        if self.tells.iter().any(|(_, w)| w == what) {
            return false;
        }
        info!("first fight: {now:.1}s tell: {what}");
        self.tells.push((now, what.to_string()));
        true
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct Tells<'w, 's> {
    noticed: MessageReader<'w, 's, crate::robots::Noticed>,
    aims: MessageReader<'w, 's, crate::robots::walker::WalkerAim>,
    locks: MessageReader<'w, 's, crate::robots::drone::DroneLock>,
    sentries: MessageReader<'w, 's, crate::robots::sentry::SentryEvent>,
    hushed: Query<'w, 's, (), (With<LocalPlayer>, With<crate::anomaly::hush::InHush>)>,
}

/// Something above you: in under a roof, through the nearest door, if one's
/// within a sprint (trees hide nobody from overhead: only their trunks stop
/// a look).
fn under_a_roof(
    entrances: Option<&crate::robots::doors::Entrances>,
    here: Vec2,
) -> Option<Vec<Task>> {
    let entrances = entrances?;
    let d = entrances
        .doors
        .iter()
        .filter(|d| {
            // A way in from outside: its far side is under that roof.
            let inside = d.mid - d.out.normalize_or(Vec3::Z) * 1.5;
            entrances.building_at(inside) == Some(d.building)
        })
        .min_by(|a, b| {
            a.mid
                .xz()
                .distance(here)
                .total_cmp(&b.mid.xz().distance(here))
        })
        .filter(|d| d.mid.xz().distance(here) < 60.0)?;
    let out = d.out.xz().normalize_or(Vec2::Y);
    let inside = d.mid.xz() - out * 2.5;
    // Off to one side of the doorway: not in a look through it.
    let aside = [out.perp(), -out.perp()]
        .into_iter()
        .map(|s| inside + s * 2.0)
        .find(|p| entrances.building_at(Vec3::new(p.x, d.mid.y, p.y)) == Some(d.building))
        .unwrap_or(inside);
    Some(vec![
        Task::WalkTo {
            to: d.mid.xz() + out * 1.3,
            within: 0.6,
        },
        Task::OpenDoor(d.door),
        Task::WalkTo {
            to: inside,
            within: 0.6,
        },
        Task::WalkTo {
            to: aside,
            within: 0.6,
        },
        Task::Wait(20.0),
        // And out the way it came in, not through a wall.
        Task::WalkTo {
            to: inside,
            within: 0.6,
        },
        Task::WalkTo {
            to: d.mid.xz() + out * 1.5,
            within: 0.8,
        },
    ])
}

/// Into cover from a machine at `from`: round the far side of the nearest
/// building within a sprint, or, in the open, hard across its line.
/// `leave` > 0: it saw you here, and is coming to look. Back off from it,
/// keeping this cover between you: to the next cover that way, or straight
/// back.
fn run_for_cover(zone: &crate::world::Zone, here: Vec2, from: Vec2, leave: f32) -> Vec<Task> {
    let away = (here - from).normalize_or(Vec2::Y);
    // Round the far side of something solid (not into trees: a look goes
    // between the trunks).
    let cover = zone
        .map
        .placements
        .iter()
        .map(|b| Vec2::from(b.at))
        .filter(|b| b.distance(here) < 60.0)
        .filter(|b| {
            leave <= 0.0
                || (b.distance(here) > leave && (*b - here).normalize_or_zero().dot(away) > 0.8)
        })
        .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
    let to = match cover {
        Some(b) => b + (b - from).normalize_or(Vec2::Y) * 6.0,
        None if leave > 0.0 => here + away * 30.0,
        None => here + away.perp() * 25.0,
    };
    vec![Task::WalkTo { to, within: 2.0 }, Task::Wait(10.0)]
}

/// Clear of walls round a building's box, m: a body (0.28) and a stride.
const ROUND: f32 = 2.0;

/// From `from` to `to` on foot, round any building in the way (the
/// straight line through the village went into a house wall and stayed
/// there): the waypoints after `from`.
fn round_buildings(view: &RaidView, from: Vec2, to: Vec2) -> Vec<Vec2> {
    let Some(en) = view.entrances.as_deref() else {
        return vec![to];
    };
    let mut points = vec![to];
    let mut at = from;
    let mut out = vec![];
    // A few detours at most; each from where the last left off.
    for _ in 0..6 {
        let Some(&next) = points.last() else {
            break;
        };
        // The first box this stretch goes through, nearest along it.
        let hit = en
            .shells
            .iter()
            .filter_map(|s| {
                let inv = s.xf.compute_affine().inverse();
                let a = inv.transform_point3(Vec3::new(at.x, 0.0, at.y)).xz();
                let b = inv.transform_point3(Vec3::new(next.x, 0.0, next.y)).xz();
                let half = s.half + Vec2::splat(ROUND * 0.5);
                segment_hits_box(a, b, half).map(|t| (t, s))
            })
            .filter(|(t, _)| *t > 0.0)
            .min_by(|x, y| x.0.total_cmp(&y.0));
        let Some((_, s)) = hit else {
            out.push(points.pop().unwrap_or(next));
            at = *out.last().unwrap_or(&next);
            if points.is_empty() {
                break;
            }
            continue;
        };
        // Round its nearer side: the two corners on that side, out by
        // `ROUND`.
        let xf = s.xf.compute_affine();
        let inv = xf.inverse();
        let a = inv.transform_point3(Vec3::new(at.x, 0.0, at.y)).xz();
        let b = inv.transform_point3(Vec3::new(next.x, 0.0, next.y)).xz();
        let h = s.half + Vec2::splat(ROUND);
        let dir = (b - a).normalize_or(Vec2::X);
        let side = if dir.perp().dot(-a) > 0.0 { -1.0 } else { 1.0 };
        // Corners in order along the way, on the chosen side.
        let mut corners: Vec<Vec2> = [
            Vec2::new(-h.x, -h.y),
            Vec2::new(h.x, -h.y),
            Vec2::new(h.x, h.y),
            Vec2::new(-h.x, h.y),
        ]
        .into_iter()
        .filter(|c| dir.perp().dot(*c) * side >= 0.0)
        .collect();
        corners.sort_by(|p, q| dir.dot(*p).total_cmp(&dir.dot(*q)));
        let world_pts: Vec<Vec2> = corners
            .into_iter()
            .map(|c| xf.transform_point3(Vec3::new(c.x, 0.0, c.y)).xz())
            .collect();
        // Next go to them in order, then on to where it was going.
        for p in world_pts.into_iter().rev() {
            points.push(p);
        }
    }
    out.extend(points.into_iter().rev());
    out
}

/// Where along a→b (0..1) it first enters the box |x| < half, if it does.
fn segment_hits_box(a: Vec2, b: Vec2, half: Vec2) -> Option<f32> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for i in 0..2 {
        let (p, v, h) = (a[i], d[i], half[i]);
        if v.abs() < 1e-6 {
            if p.abs() > h {
                return None;
            }
        } else {
            let (mut lo, mut hi) = ((-h - p) / v, (h - p) / v);
            if lo > hi {
                std::mem::swap(&mut lo, &mut hi);
            }
            t0 = t0.max(lo);
            t1 = t1.min(hi);
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(t0)
}

/// `legs`, round the buildings in the way.
fn route(world: &World2, view: &RaidView, from: Vec2, to: Vec2) -> Vec<Task> {
    let mut at = from;
    let mut t = vec![];
    for p in round_buildings(view, from, to) {
        t.extend(world.legs(at, p));
        at = p;
    }
    t
}

/// What the newcomer carries that `want` stands for: in a hand, a pocket
/// or the pack (not the stash crate).
fn carried(world: &World2, view: &RaidView, player: Entity, want: &str) -> Vec<Entity> {
    world
        .items
        .iter()
        .filter(|(e, it, _, held, sock, bag)| {
            // On this newcomer: in its hand, its slots or its own pack (not
            // the pack it left where it fell, nor the stash crate).
            fetch_takes(want, &world.catalog.get(it.kind()).id)
                && (held.is_some_and(|h| h.player == player)
                    || sock
                        .is_some_and(|s| world.owners.get(s.socket).is_ok_and(|o| o.0 == player))
                    || bag.is_some_and(|b| view.bags.get(b.bag).is_ok_and(|b| b.owner == player)))
                && !view.stash.iter().any(|b| b.contents.contains(e))
        })
        .map(|(e, ..)| e)
        .collect()
}

/// A thing's name as the card has it ("field recorder").
fn name_of(world: &World2, id: &str) -> String {
    world
        .catalog
        .kind(id)
        .map(|k| world.catalog.get(k).name.to_lowercase())
        .unwrap_or_else(|| id.replace('_', " "))
}

/// Indoors at `here`: out by the nearest door, and where that leaves it.
fn way_out(world: &World2, view: &RaidView, here: Vec2) -> (Vec<Task>, Vec2) {
    let Some(en) = view.entrances.as_deref() else {
        return (vec![], here);
    };
    let at = world.zone.on_ground(here.x, here.y) + Vec3::Y * 1.0;
    let Some(b) = en.building_at(at) else {
        return (vec![], here);
    };
    let Some(d) = en
        .doors
        .iter()
        .filter(|d| d.building == b)
        .filter(|d| en.building_at(d.mid - d.out.normalize_or(Vec3::Z) * 1.5) == Some(b))
        .min_by(|a, c| {
            a.mid
                .xz()
                .distance(here)
                .total_cmp(&c.mid.xz().distance(here))
        })
    else {
        return (vec![], here);
    };
    let out = d.out.xz().normalize_or(Vec2::Y);
    let outside = d.mid.xz() + out * 2.0;
    (
        vec![
            Task::WalkTo {
                to: d.mid.xz() - out * 1.2,
                within: 0.5,
            },
            Task::OpenDoor(d.door),
            Task::WalkTo {
                to: outside,
                within: 0.6,
            },
        ],
        outside,
    )
}

/// To `p` from `here`: indoors, round to its building's door, open it, in,
/// and through the rooms to it (the building's plan, doors opened on the
/// way: a find in a back room isn't reached through the wall).
fn way_to(
    world: &World2,
    view: &RaidView,
    plans: &[crate::sim::indoor::BuildingPlan],
    here: Vec2,
    p: Vec3,
) -> Vec<Task> {
    let (mut t, here) = way_out(world, view, here);
    if let Some(plan) = plans.iter().find(|pl| pl.contains(p, 0.3)) {
        let goal = plan.to_local(p).xz();
        let entry = plan
            .doors
            .iter()
            .filter_map(|d| d.outside)
            .min_by(|a, b| a.distance(goal).total_cmp(&b.distance(goal)));
        if let Some((o, path)) = entry.and_then(|o| Some((o, plan.path(o, goal)?))) {
            t.extend(route(world, view, here, plan.world2(o)));
            let mut prev = o;
            let last = path.len().saturating_sub(1);
            for (i, w) in path.into_iter().enumerate() {
                let doors = plan.doors_on(prev, w);
                // Every few cells, and at each door and the end.
                if doors.is_empty() && i % 4 != 0 && i != last {
                    continue;
                }
                for (door, _) in doors {
                    t.push(Task::OpenDoor(door));
                }
                t.push(Task::WalkTo {
                    to: plan.world2(w),
                    within: 0.4,
                });
                prev = w;
            }
            return t;
        }
    }
    let door = view.entrances.as_deref().and_then(|en| {
        let b = en.building_at(p)?;
        en.doors
            .iter()
            .filter(|d| d.building == b)
            .filter(|d| en.building_at(d.mid - d.out.normalize_or(Vec3::Z) * 1.5) == Some(b))
            .min_by(|a, c| a.mid.distance(p).total_cmp(&c.mid.distance(p)))
            .copied()
    });
    let Some(d) = door else {
        t.extend(route(&world, &view, here, p.xz()));
        return t;
    };
    let out = d.out.xz().normalize_or(Vec2::Y);
    let front = d.mid.xz() + out * 1.3;
    t.extend(route(&world, &view, here, front));
    t.extend([
        Task::OpenDoor(d.door),
        Task::WalkTo {
            to: d.mid.xz() - out * 1.5,
            within: 0.5,
        },
    ]);
    t
}

/// The next stretch of an errand; `Ok(None)` once it's done out here,
/// `Err` where the newcomer can't see how.
fn errand_tasks(
    errand: &Errand,
    world: &World2,
    view: &RaidView,
    plans: &[crate::sim::indoor::BuildingPlan],
    player: Entity,
    here: Vec2,
    tries: u32,
    skip: &[Entity],
    no_plan: &[Entity],
) -> Result<Option<(Vec<Task>, Option<Entity>)>, String> {
    let job = view.progression.as_ref().and_then(|p| p.0.job.clone());
    match errand {
        Errand::Look => Ok(None),
        Errand::Bring { want, n, near } => {
            let name = name_of(world, want);
            if carried(world, view, player, want).len() as u32 >= *n {
                return Ok(None);
            }
            if tries > 4 + 3 * n {
                return Err(format!("couldn't find the {name}"));
            }
            let from = near.unwrap_or(here);
            let found = world
                .items
                .iter()
                .filter(|(e, it, _, held, sock, bag)| {
                    held.is_none()
                        && sock.is_none()
                        && bag.is_none()
                        && fetch_takes(want, &world.catalog.get(it.kind()).id)
                        && !view.stash.iter().any(|b| b.contents.contains(e))
                        && !skip.contains(e)
                })
                .map(|(e, _, gt, ..)| (e, gt.translation()))
                .min_by(|a, b| a.1.xz().distance(from).total_cmp(&b.1.xz().distance(from)));
            let Some((e, p)) =
                found.filter(|(_, p)| near.is_none_or(|n| p.xz().distance(n) < 80.0))
            else {
                return Err(format!("no {name} anywhere I looked"));
            };
            let plans = if no_plan.contains(&e) { &[][..] } else { plans };
            let mut t = way_to(world, view, plans, here, p);
            t.extend([
                Task::Grab {
                    item: e,
                    side: Side::Right,
                },
                Task::Stow(Side::Right),
                Task::Wait(0.5),
            ]);
            Ok(Some((t, Some(e))))
        }
        Errand::Leave { item, at } => {
            if matches!(job, Some(Job::Plant { planted: true, .. })) {
                return Ok(None);
            }
            let name = name_of(world, item);
            if tries > 4 {
                return Err(format!("left the {name}, and nothing says it's right"));
            }
            let Some(e) = carried(world, view, player, item).first().copied() else {
                return Err(format!("no {name} on me to leave"));
            };
            let mut t = route(&world, &view, here, *at);
            t.extend([
                Task::Grab {
                    item: e,
                    side: Side::Right,
                },
                Task::Wait(0.5),
                Task::LookAt(world.zone.on_ground(at.x, at.y)),
                Task::Release(Side::Right),
                Task::Wait(2.0),
            ]);
            Ok(Some((t, None)))
        }
        Errand::Photo(subject) => {
            if matches!(job, Some(Job::Photograph { taken: true, .. })) {
                return Ok(None);
            }
            if tries > 6 {
                return Err("took the picture, and nothing says it's in it".into());
            }
            let Some(camera) = carried(world, view, player, "camera").first().copied() else {
                return Err("no camera on me".into());
            };
            let target = photo_subject(view, *subject, here);
            let Some(p) = target else {
                return Err(format!("no {subject:?} to be found for the picture").to_lowercase());
            };
            // Near enough to fill the picture, not so near it shoots.
            let stand = p.xz() + (here - p.xz()).normalize_or(Vec2::Y) * 30.0;
            let mut t = route(&world, &view, here, stand);
            t.extend([
                Task::Grab {
                    item: camera,
                    side: Side::Right,
                },
                Task::LookAt(p + Vec3::Y * 1.5),
                Task::Wait(1.0),
                Task::Use(Side::Right),
                Task::Wait(0.5),
            ]);
            Ok(Some((t, None)))
        }
    }
}

/// The nearest machine a photograph job wants.
fn photo_subject(view: &RaidView, subject: Subject, here: Vec2) -> Option<Vec3> {
    view.robots
        .iter()
        .filter(|(r, _, breed)| {
            !r.wrecked
                && r.kind == crate::robots::RobotKind::Walker
                && (breed.is_some_and(|b| b.0 == "heavy") == (subject == Subject::Heavy))
        })
        .map(|(_, gt, _)| gt.translation())
        .min_by(|a, b| a.xz().distance(here).total_cmp(&b.xz().distance(here)))
}

/// What it says once the errand's done out here.
fn errand_done(errand: &Errand, world: &World2) -> String {
    match errand {
        Errand::Look => "the place, seen".into(),
        Errand::Bring { want, n, .. } => format!("found {n} {}, and packed", name_of(world, want)),
        Errand::Leave { item, .. } => format!("left the {} there", name_of(world, item)),
        Errand::Photo(s) => format!("the {s:?}'s picture taken").to_lowercase(),
    }
}

/// Where along the way to the first job the machine is met: the drop to
/// the Old farm, a little short of halfway.
fn on_the_way(zone: &crate::world::Zone) -> (Vec2, Vec2) {
    let drop = zone
        .map
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Insertion)
        .map_or(Vec2::ZERO, |m| Vec2::from(m.at));
    let farm = zone
        .map
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Poi && m.name == "Old farm")
        .map_or(drop + Vec2::new(100.0, -100.0), |m| Vec2::from(m.at));
    let at = drop.lerp(farm, 0.45);
    (at, (farm - drop).normalize_or(Vec2::NEG_Y))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn first_fight(
    opts: Res<LaunchOptions>,
    mut commands: Commands,
    zone: Res<crate::world::Zone>,
    raid: Res<Raid>,
    mut fight: ResMut<Fight>,
    mut tells: Tells,
    mut bots: Query<(&mut Bot, &Transform, &mut crate::rig::RigInput), With<LocalPlayer>>,
    health: Query<&crate::health::Health, With<LocalPlayer>>,
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    time: Res<Time>,
) {
    let Some(kind) = opts.encounter.as_deref() else {
        return;
    };
    let Some(p) = current(&opts) else {
        return;
    };
    let now = time.elapsed_secs();
    // Put it there once the newcomer's out.
    if !fight.seeded && raid.phase == Phase::Out {
        fight.seeded = true;
        let (at, ahead) = on_the_way(&zone);
        let side = ahead.perp();
        let job = |site: Vec3| crate::robots::Job {
            kind: crate::robots::JobKind::Watch,
            site,
            duration: 1800.0,
            exit: site + Vec3::new(0.0, 0.0, -300.0),
        };
        let placed = match kind {
            // On its rounds, a little off the way.
            "walker" | "night" => {
                let p = at + side * 25.0;
                let site = zone.on_ground(p.x, p.y);
                crate::robots::walker::spawn_walker(&mut commands, &zone, site, job(site));
                p
            }
            "drone" => {
                let p = at + side * 30.0;
                let site = zone.on_ground(p.x, p.y);
                crate::robots::drone::spawn_drone(&mut commands, site + Vec3::Y * 22.0, job(site));
                p
            }
            // Right across the way: nothing to see but still dust.
            "hush" => {
                let site = zone.on_ground(at.x, at.y);
                commands.queue(move |w: &mut World| {
                    crate::anomaly::spawn_anomaly(
                        w,
                        extraction_core::anomaly::AnomalyKind::Hush,
                        site,
                        7.0,
                        None,
                    );
                });
                at
            }
            // Off to the side, sweeping across the way.
            "sentry" => {
                let p = at + side * 22.0;
                let to_way = at - p;
                let centre = to_way.x.atan2(-to_way.y).to_degrees();
                crate::robots::sentry::spawn_sentry(
                    &mut commands,
                    &zone,
                    p,
                    centre,
                    140.0,
                    p + side * 6.0,
                );
                p
            }
            _ => at,
        };
        info!("first fight: a {kind} at {placed:.0} for the {}", p.name);
        fight.machine = Some((kind.to_string(), placed));
    }
    // The tells, as they come.
    let mut came = Vec::new();
    for _ in tells.noticed.read() {
        came.push("a machine noticed you (its eye brightens)");
    }
    for _ in tells.aims.read() {
        came.push("a walker's gun settling (clank, servo)");
    }
    for _ in tells.locks.read() {
        came.push("a drone's lock-on (the chirp)");
    }
    for s in tells.sentries.read() {
        if s.what == crate::robots::sentry::SentryAct::Chirp {
            came.push("a sentry's chirp (its lens flares)");
        }
    }
    if !tells.hushed.is_empty() {
        came.push("inside a hush (the sound goes dull)");
    }
    let mut first_tell = None;
    for what in came {
        if fight.tell(now, what) && first_tell.is_none() {
            first_tell = Some(what);
        }
    }
    // The first hit.
    if let Ok(h) = health.single() {
        if fight.hp > 0.0 && h.hp < fight.hp - 0.5 && fight.first_hit.is_none() {
            fight.first_hit = Some(now);
            info!("first fight: {now:.1}s first hit");
        }
        fight.hp = h.hp;
    }
    // What the newcomer does about it.
    let Some(what) = first_tell.filter(|_| !fight.reacted) else {
        return;
    };
    let Ok((mut bot, root, _)) = bots.single_mut() else {
        return;
    };
    fight.reacted = true;
    let here = root.translation.xz();
    let from = fight.machine.as_ref().map_or(here, |(_, at)| *at);
    let away = (here - from).normalize_or(Vec2::Y);
    let hush = what.contains("hush");
    let answer: Vec<Task> = if p.reckless && !hush {
        vec![Task::DrawGun, Task::Engage { shots: 12 }]
    } else if p.lost {
        // Off, anywhere.
        vec![Task::WalkTo {
            to: here + away * 25.0,
            within: 2.0,
        }]
    } else if p.hoards && !hush {
        // Carries on.
        vec![]
    } else if hush {
        // Back out the way it came.
        vec![Task::WalkTo {
            to: here + away * 10.0,
            within: 1.5,
        }]
    } else {
        // For cover, as the driver says, upright and fast: under a roof
        // from a drone.
        fight.flee_until = now + 12.0;
        (kind == "drone")
            .then(|| under_a_roof(entrances.as_deref(), here))
            .flatten()
            .unwrap_or_else(|| run_for_cover(&zone, here, from, 0.0))
    };
    if !answer.is_empty() {
        super::second_hour::cut_in(&mut bot, answer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every kind of job there is (the match in `errand` makes a new
    /// kind fail to build until it has a way).
    fn one_of_each() -> Vec<Job> {
        let map = "marrow_creek".to_owned();
        vec![
            Job::Fetch {
                item: "canned_stew".into(),
                n: 2,
            },
            Job::Recover {
                map: map.clone(),
                region: "Village".into(),
                at: Some([10.0, 20.0]),
                place: Some("in the pump house by the Village.".into()),
                overnight: false,
            },
            Job::Letter,
            Job::Ledger,
            Job::Scout {
                map: map.clone(),
                place: "Old farm".into(),
                at: [100.0, 200.0],
                seen: false,
            },
            Job::Bring {
                map: map.clone(),
                place: "the post office".into(),
                item: "holm_watch".into(),
                at: [-16.0, 35.0],
            },
            Job::Plant {
                map,
                place: "the mast".into(),
                item: "relay_box".into(),
                at: [5.0, 6.0],
                planted: false,
            },
            Job::Photograph {
                subject: Subject::Walker,
                taken: false,
            },
        ]
    }

    /// Every kind of job has a way for the newcomer to do it, and the card
    /// names what it goes for: what the bot is after, a player reads.
    #[test]
    fn every_job_kind_has_a_way() {
        let catalog = crate::items::Catalog::load();
        for job in one_of_each() {
            let card =
                crate::progression::job_text(&job, &catalog, "marrow_creek", false).to_lowercase();
            let named = |id: &str| {
                let name = catalog
                    .kind(id)
                    .map(|k| catalog.get(k).name.to_lowercase())
                    .unwrap_or_else(|| panic!("{id} isn't in the catalog"));
                name.split_whitespace()
                    .filter(|w| w.len() > 3)
                    .any(|w| card.contains(w.trim_end_matches('s')))
            };
            match errand(&job) {
                Errand::Look => assert!(matches!(job, Job::Scout { .. }), "{job:?}"),
                Errand::Bring { want, n, near } => {
                    assert!(n > 0, "{job:?}");
                    assert!(named(&want), "the card for {job:?} doesn't name it: {card}");
                    // A fetch says nothing of where; everything else does.
                    assert_eq!(
                        near.is_none(),
                        matches!(job, Job::Fetch { .. } | Job::Letter | Job::Ledger),
                        "{job:?}"
                    );
                }
                Errand::Leave { item, .. } => {
                    assert!(named(&item), "the card for {job:?} doesn't name it: {card}")
                }
                Errand::Photo(s) => assert!(card.contains(&s.name().to_lowercase()), "{card}"),
            }
        }
    }

    /// A stretch through a box is caught where it goes in; one beside it,
    /// or that stops short, isn't.
    #[test]
    fn a_walk_through_a_house_is_caught() {
        let half = Vec2::new(4.0, 3.0);
        let t = segment_hits_box(Vec2::new(-10.0, 0.0), Vec2::new(10.0, 0.0), half).unwrap();
        assert!((t - 0.3).abs() < 1e-4, "{t}");
        assert!(segment_hits_box(Vec2::new(-10.0, 5.0), Vec2::new(10.0, 5.0), half).is_none());
        assert!(segment_hits_box(Vec2::new(-10.0, 0.0), Vec2::new(-6.0, 0.0), half).is_none());
    }

    /// The job's place, not the driver's cache under it.
    #[test]
    fn the_card_is_read_for_the_job_not_the_cache() {
        let zone = crate::world::Zone::load("marrow_creek");
        let card = "CONTRACT: THE POST COUNTER - 1 OF 1\n\"Post office watch. By the village, \
or in it. Bring it home.\"\nStash it, press here.\n\nMY CACHE: in the survey tower out past \
the Hollow.";
        assert_eq!(
            place_on_card(card, &zone).map(|(n, _)| n).as_deref(),
            Some("Village")
        );
    }
}
