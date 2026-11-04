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
    /// Near the job, goes off elsewhere (the playtest's first raid: in to
    /// 137 m, then west); comes back only if told which way it is.
    pub wanders: bool,
    /// Takes nothing from the crate (the playtest's second raid); goes back
    /// for its kit only if told it has none.
    pub empty_handed: bool,
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
        wanders: false,
        empty_handed: false,
    }
}

pub const PERSONAS: [Persona; 7] = [
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
    // By name only (not in `first_hour`'s five): the playtest's two misses
    // (`docs/playtests/2026-10-28`), for what the game says to put them
    // right (`progression::nudge`, `progression::tailgate`).
    Persona {
        wanders: true,
        ..persona("wanderer", "first_hour_wanderer")
    },
    Persona {
        empty_handed: true,
        ..persona("empty-handed", "first_hour_empty_handed")
    },
];

/// The newcomers the full runs take: the five, not the by-name two.
fn everyday(p: &&Persona) -> bool {
    !p.wanders && !p.empty_handed
}

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
/// (`broken`: the broken walker a career's second job meets, `robots::broken`.)
pub const ENCOUNTERS: [&str; 6] = ["walker", "drone", "hush", "sentry", "night", "broken"];

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
    let all = || PERSONAS.iter().filter(everyday).collect::<Vec<_>>();
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
    let (mut failed, mut by_design) = (Vec::new(), Vec::new());
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
                if p.hoards {
                    // By design: it carries on as if nothing were there,
                    // the driver's warnings and all, and out in the open
                    // after a warning is meant to lose (Robot minds'
                    // fairness rules). Its going down is the game being
                    // fair, not a failure of what it was told.
                    println!(
                        "first fight: {}/{e} didn't come through, by design (it heeds no warning)",
                        p.name
                    );
                    by_design.push(format!("{}/{e}", p.name));
                } else {
                    failed.push(format!("{}/{e}", p.name));
                }
            }
        }
    }
    if !by_design.is_empty() {
        println!("first fight: down by design (heeding nothing): {by_design:?}");
    }
    if failed.is_empty() {
        println!("first fight: every newcomer who heeds the driver came through");
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
    /// A machine had it with nothing chambered, the driver's "one up the
    /// spout" not done.
    Unready,
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
/// ladder runs live (the place it's put is the game's own). `scout` is the
/// first job the valley `here` deals a new career: a first hour in a valley
/// the ladder only reaches later.
pub fn seeded_job(kind: &str, here: &str) -> Option<Job> {
    let map = "marrow_creek".to_owned();
    Some(match kind {
        "scout" => {
            let (id, def) = crate::progression::valleys()
                .iter()
                .find(|(id, _)| *id == here)?;
            return extraction_core::progression::first_job(id, def);
        }
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
    match seeded_job(kind, &opts.map) {
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
    /// Roofs it couldn't get under (a way in it stuck at: Cinder's shop
    /// has one on a wall with only a window), and the one it's making for
    /// with the bot's failures then.
    no_roof: Vec<Entity>,
    roof_try: Option<(Entity, u32, f32)>,
    /// When it last did as a machine's tell says (not more than every 15 s).
    tell_at: f32,
    /// When it got home (the truck back at base, it in the bed).
    home_at: Option<f32>,
    /// Hit after a warning: further hits till then are the same fight.
    warned_fight_until: f32,
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
    /// The empty-handed one: stepped off the truck to see what's said, and
    /// went back for its kit.
    stepped_off: bool,
    went_back: bool,
    /// The wanderer: when it went off from near the job, and whether it was
    /// told the way back.
    wandered_at: Option<f32>,
    told_back: bool,
    /// When the reckless one last picked a fight.
    fought_at: f32,
    /// When machines after it fired at it, the last half minute.
    shots: Vec<f32>,
    /// When a drone locked on to it again, lately (`PINNED_AFTER`).
    relocks: Vec<f32>,
    /// Pinned by one: it calls from where it is, not the gate.
    pinned: bool,
    /// Racked at the driver's "one up the spout".
    racked: bool,
    /// The broken walker wrecked (job 2's first fight, won).
    broke_it: bool,
    /// A machine has noticed it this raid (and whether a round was up).
    noticed: bool,
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
    robots: Query<'w, 's, crate::robots::subjects::Candidate>,
    trucks: Query<'w, 's, &'static GlobalTransform, With<crate::raid::truck::Truck>>,
    health: Query<'w, 's, &'static crate::health::Health, With<LocalPlayer>>,
}

/// The wanderer goes this share of the way to the job (the playtest: in
/// to 137 m of 188), then off to a place this many times as far from the
/// job as the drop was, and waits this long (s) to be told the way back
/// before giving it up. Shares, not metres: Marrow's warp brought the job
/// from 188 m to 103 m, and 130 m short of it was behind the gate.
const WANDER_SHARE: f32 = 0.6;
/// The place it wanders to is this much past the nudge's distance (m): on
/// Cinder the nearest past 1.3 times the way was short of it, and nothing
/// was said.
const WANDER_MARGIN: f32 = 25.0;
const WANDER_OFF: f32 = 1.3;
const WANDER_WAIT: f32 = 240.0;

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
    // Stuck on the way under a roof: the rest of that way is no good either
    // (in, wait, out the way it came), and nor is that roof.
    // (There, in, its wait and out: on Cinder the first stuck came 75 s
    // after the plan was made.)
    if state
        .roof_try
        .is_some_and(|(_, _, t)| time.elapsed_secs() - t > 150.0)
    {
        state.roof_try = None;
    }
    if let Some((b, failed, _)) = state.roof_try {
        if bot.failed > failed {
            bot.tasks.clear();
            // (Idle, so the next leg's planned: an empty list is the end.)
            bot.tasks.push_back(Task::Wait(IDLE));
            bot.walk = None;
            state.no_roof.push(b);
            state.roof_try = None;
        }
    }
    let now = time.elapsed_secs();
    let here = root.translation.xz();
    // A cautious newcomer creeps once it's out.
    // Running for it, as told: upright and fast.
    let fleeing = fight.as_ref().is_some_and(|f| f.flee_until > now);
    let low = fight.as_ref().is_some_and(|f| f.low_until > now);
    let creep = (p.reads || state.low_until > now || low)
        && !fleeing
        && !matches!(
            state.leg,
            Leg::Camp | Leg::Kit | Leg::Card | Leg::Board | Leg::Paid | Leg::Handed
        );
    // A photograph: the head kept on the machine as it moves, the camera
    // up (the card: "it in the middle").
    if let (Leg::Place, Some(Errand::Photo(subject))) = (state.leg, &state.errand)
        && let Some(p) = photo_subject(&world.zone, &view, *subject, here)
            .filter(|p| p.xz().distance(here) < 60.0)
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
    // The buildings' plans, once they're up (in from a drone, a find indoors).
    if plans.is_empty() && !view.buildings.is_empty() {
        *plans = crate::sim::indoor::plan_buildings(&view.buildings, &view.leaves);
    }
    // Indoors, a walk about to start that some answer cut in (from a
    // shed, sheltered from a drone, the walk to cover went at its wall
    // for half a minute): not one to or by a door of this building (the
    // way out's own last step).
    if let (Some(en), Some(Task::WalkTo { to, .. })) =
        (view.entrances.as_deref(), bot.tasks.front().cloned())
        && bot.elapsed < 0.2
    {
        let inside = |p: Vec2| en.building_at(world.zone.on_ground(p.x, p.y) + Vec3::Y * 1.0);
        if let Some(b) = inside(here)
            && inside(to) != Some(b)
            && !en
                .doors
                .iter()
                .any(|d| d.building == b && d.mid.xz().distance(to) < 4.0)
        {
            let (out, _) = way_out(&world, &view, &plans, here);
            if !out.is_empty() {
                for t in out.into_iter().rev() {
                    bot.tasks.push_front(t);
                }
                bot.restart();
            }
        }
    }
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
    let noticed_now = tells.noticed.read().any(|ev| on_me(ev.robot));
    // The first fight it can win: the broken walker wrecked.
    if !state.broke_it && world.broken.iter().any(|r| r.wrecked) {
        state.broke_it = true;
        state.note(now, Kind::Learned, "wrecked the broken walker");
    }
    // "One up the spout": out, racked, back in the holster (the safety
    // stays on).
    let guns = || world.carried_guns.iter().map(|(g, by, p)| (g, by.0, p));
    let has_gun = guns().any(|(_, by, p)| by == player && !p);
    // (What it's told is by the gun it holds: a round up, a magazine in
    // first, load up from the crate, or no rounds at all.)
    use super::hints::{IN_PACK, LOAD_UP, MAG_IN, NO_ROUNDS, ONE_UP};
    let told = |l: &str, heard: &[String]| heard.iter().any(|h| h == l);
    let no_rounds = told(NO_ROUNDS, &state.heard);
    let load_up = told(LOAD_UP, &state.heard);
    let in_pack = told(IN_PACK, &state.heard);
    let ready_told = told(ONE_UP, &state.heard) || told(MAG_IN, &state.heard) || load_up || in_pack;
    if !state.racked && has_gun && ready_told {
        state.racked = true;
        // "Load up from the crate": the loaded magazines in it, first (and
        // then readied: two cut-ins, the later went first, and it readied
        // before it loaded).
        let mut first = Vec::new();
        if load_up {
            let loaded: Vec<Entity> = view
                .stash
                .iter()
                .flat_map(|b| b.contents.iter().copied())
                .filter(|e| {
                    world.id(*e).is_some_and(|id| id.ends_with("_mag"))
                        && world.items.get(*e).is_ok_and(|(_, it, ..)| it.rounds > 0)
                })
                .collect();
            state.note(
                now,
                Kind::Learned,
                format!(
                    "empty, as the driver said: {} loaded from the crate",
                    loaded.len()
                ),
            );
            let none = loaded.is_empty();
            for e in loaded {
                first.push(Task::Grab {
                    item: e,
                    side: Side::Right,
                });
                first.push(Task::Stow(Side::Right));
            }
            // None loaded there, only rounds: its own magazine out of the
            // gun, filled from a box of them, into a pouch (the reload
            // after puts it in).
            if none {
                let wells: Vec<Entity> = guns()
                    .filter(|(_, by, packed)| *by == player && !packed)
                    .filter_map(|(g, ..)| g.magwell)
                    .collect();
                let mag = world
                    .items
                    .iter()
                    .find(|(_, _, _, _, sock, _)| sock.is_some_and(|s| wells.contains(&s.socket)));
                let cartridge = mag
                    .and_then(|(_, it, ..)| world.catalog.get(it.kind()).magazine.clone())
                    .map(|m| m.cartridge);
                let rounds = view
                    .stash
                    .iter()
                    .flat_map(|b| b.contents.iter().copied())
                    .find(|e| {
                        world.items.get(*e).is_ok_and(|(_, it, ..)| {
                            it.rounds > 0
                                && world.catalog.get(it.kind()).cartridge.is_some()
                                && world.catalog.get(it.kind()).cartridge == cartridge
                        })
                    });
                if let (Some((m, ..)), Some(r)) = (mag, rounds) {
                    state.note(
                        now,
                        Kind::Learned,
                        "loaded its magazine from the crate's rounds",
                    );
                    first.extend([
                        Task::Stow(Side::Left),
                        Task::Stow(Side::Right),
                        Task::Grab {
                            item: m,
                            side: Side::Left,
                        },
                        Task::Grab {
                            item: r,
                            side: Side::Right,
                        },
                        Task::Combine,
                        Task::Stow(Side::Right),
                        Task::Stow(Side::Left),
                    ]);
                }
            }
        }
        // "Your loaded one's in your pack: swap them": the loaded one out
        // of the pack into the holster, the empty one into the pack.
        let loaded = world
            .packed_guns
            .iter()
            .find(|(_, g, by)| {
                by.0 == player && g.firearm.magazine.as_ref().is_some_and(|m| m.rounds > 0)
            })
            .map(|(e, ..)| e);
        if let Some(e) = loaded.filter(|_| in_pack) {
            state.note(
                now,
                Kind::Learned,
                "swapped in the loaded one from the pack",
            );
            super::second_hour::cut_in(
                &mut bot,
                vec![
                    Task::Stow(Side::Left),
                    Task::DrawGun,
                    Task::ToggleBackpack,
                    Task::Wait(0.6),
                    Task::GrabFromHere {
                        item: e,
                        side: Side::Left,
                    },
                    Task::ToggleBackpack,
                    Task::Wait(0.5),
                    // (A body slot first: the holster, free with the empty
                    // one out; then the empty one finds it full, and goes
                    // in the pack.)
                    Task::Stow(Side::Left),
                    Task::Stow(Side::Right),
                    Task::DrawGun,
                    Task::Ready { off_safe: false },
                    Task::Stow(Side::Right),
                ],
            );
        } else if !one_up(guns(), player) {
            state.note(now, Kind::Learned, "one up the spout, as the driver said");
            // Readied by the one drawn (a magazine in if it has none with
            // rounds, then a round up), the safety left on.
            first.extend([
                Task::DrawGun,
                Task::Ready { off_safe: false },
                Task::Stow(Side::Right),
            ]);
        }
        if !first.is_empty() {
            super::second_hour::cut_in(&mut bot, first);
        }
    }
    // The first machine to have it: a round up by then, or not.
    if noticed_now && !state.noticed && has_gun {
        state.noticed = true;
        if one_up(guns(), player) {
            state.note(now, Kind::Learned, "noticed, with a round up");
        } else if no_rounds {
            // Nothing anywhere to load, and told so: not unready by its own
            // doing.
            state.note(
                now,
                Kind::Learned,
                "noticed, with no rounds to load (the driver said)",
            );
        } else {
            // What it had to load: the gun's magazine, and the full ones on
            // it (out of them after a fight, a reload had nothing in it).
            let in_gun = guns()
                .find(|(_, by, p)| *by == player && !p)
                .map(|(g, ..)| g.firearm.magazine.as_ref().map_or(0, |m| m.rounds));
            let spares = world
                .items
                .iter()
                .filter(|(e, it, ..)| {
                    world.id(*e).is_some_and(|id| id.ends_with("_mag"))
                        && it.rounds > 0
                        && carried(&world, &view, player, world.id(*e).unwrap_or("")).contains(e)
                })
                .count();
            let every: Vec<String> = guns()
                .filter(|(_, by, _)| *by == player)
                .map(|(g, _, packed)| {
                    format!(
                        "{}{}",
                        g.firearm.magazine.as_ref().map_or(0, |m| m.rounds),
                        if packed { " packed" } else { "" }
                    )
                })
                .collect();
            info!("first hour: unready: every gun on it, rounds in its magazine: {every:?}");
            state.note(
                now,
                Kind::Unready,
                format!(
                    "noticed, with nothing chambered (in the gun's magazine {}, loaded spares on it {spares})",
                    in_gun.map_or("none".to_string(), |r| r.to_string())
                ),
            );
        }
    }
    if !again.is_empty() {
        // Warned: by the machine itself.
        state.said_at = now;
    }
    // The other machines' own tells on it warn as well (Legibility's
    // survey: a hound's coil, a crawler's charge, a heavy's raised arm, a
    // grappler's windup), and a machine noticing it.
    let coiled = tells.hounds.read().any(|ev| on_me(ev.hound));
    let charged = tells.crawlers.read().any(|ev| on_me(ev.crawler));
    let raised = tells
        .heavies
        .read()
        .any(|ev| ev.what == crate::robots::heavy::HeavyAct::Raise && on_me(ev.heavy));
    let wound = tells.harpoons.read().any(|ev| ev.to == player);
    // A machine still firing at it, hit or miss: one fight, heard all
    // through (a drone's bursts 1.8 s apart never let it re-lock).
    // (Not the round that hits: only those before it, `shot_warns`.)
    let fired_on = tells.shots.read().any(|s| s.shooter.is_some_and(on_me));
    if coiled || charged || raised || wound || noticed_now {
        state.said_at = now;
    }
    if fired_on {
        state.shots.retain(|t| now - t < 30.0);
        state.shots.push(now);
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
            // (Out first: the jammed gun may be back on the hip by now.)
            First::Jam(_) => vec![
                Task::DrawGun,
                Task::Gun(crate::weapons::flat::MacroKind::Rack),
            ],
            First::Heard if !p.reckless => vec![Task::Wait(15.0)],
            // "Its eye's still good: shoot that": the one to take on (bar
            // the lost one, who goes anywhere but at it).
            First::BrokenNoticed if !p.lost => {
                state.note(
                    now,
                    Kind::Learned,
                    "a broken walker: took it on, as the driver said",
                );
                take_it_on()
            }
            // "Keep your distance": back off it, and at it again.
            First::Shoved if !p.lost => {
                let from = world.nearest_robot(here).unwrap_or(here + Vec2::Y);
                let away = (here - from).normalize_or(Vec2::Y);
                let mut t = vec![Task::WalkTo {
                    to: here + away * 4.0,
                    within: 1.0,
                }];
                t.extend(take_it_on());
                t
            }
            // "Run across it, to cover", "Break its line", "Down, out of
            // its sweep": for it, as told.
            // "Down, out of its sweep": low, and away from it (a sentry
            // doesn't come after you).
            // Those who'd listen go under the nearest roof, by the way
            // round, as the driver teaches (a roof keeps out a drone's
            // burst and a sentry's sweep alike): out in the open after a
            // warning, a burst every 2 s wore them down. No roof near:
            // low across its line.
            First::Sentry if !p.reckless && !p.hoards => {
                let from = world.nearest_robot(here).unwrap_or(here + Vec2::Y * 20.0);
                match under_a_roof(&world, &plans, here, &state.no_roof) {
                    Some((tasks, b)) => {
                        state.roof_try = Some((b, bot.failed, now));
                        tasks
                    }
                    None => {
                        state.low_until = now + 25.0;
                        run_for_cover(&world.zone, here, from, 1.0)
                    }
                }
            }
            First::DroneLock | First::Aimed if !p.reckless && !p.hoards => {
                let from = world.nearest_robot(here).unwrap_or(here + Vec2::Y * 20.0);
                if let Some(f) = fight.as_mut() {
                    f.flee_until = now + 12.0;
                }
                let overhead = f == First::DroneLock;
                // At the truck's tail, boarding: on, as the driver says
                // ("Don't stop. Sprint across it"), not back to a roof with
                // the truck there. Anywhere short of it, a roof still
                // (pinned and running on in the open, three newcomers were
                // shot down in under two minutes; under roofs, two got
                // home).
                let going_home = matches!(state.leg, Leg::Board)
                    && world
                        .trucks
                        .iter()
                        .any(|t| t.translation().xz().distance(here) < 15.0);
                // Pinned: a drone that keeps finding it however often it
                // goes under a roof (re-locked every 23 s for 20 minutes at
                // the post office, worn down and killed, three raids). A
                // person gives the job up: home from here, after this one
                // in cover.
                if overhead {
                    state.relocks.retain(|t| now - t < PINNED_FOR);
                    state.relocks.push(now);
                    if state.relocks.len() >= PINNED_AFTER
                        && !matches!(state.leg, Leg::Rendezvous | Leg::Board | Leg::Done)
                        && !state.pinned
                    {
                        // (Already on its way to the gate to call, as the
                        // card said: from here instead; it never got out of
                        // the post office's door.)
                        state.pinned = true;
                        state.note(now, Kind::Learned, "pinned by a drone: home from here");
                        state.relocks.clear();
                        state.leg = Leg::Call;
                    }
                }
                // On its way home from it (pinned, or the truck waiting):
                // on, at a run, as the driver says ("Don't stop. Sprint
                // across it"). Under a roof at every re-lock, the truck came
                // to the gate and pulled out again while it hid, and the
                // drone wore it down for eight minutes more.
                let roof = (overhead && !going_home)
                    .then(|| under_a_roof(&world, &plans, here, &state.no_roof))
                    .flatten();
                match roof {
                    _ if overhead && going_home => {
                        state.note(now, Kind::Learned, "a drone on it, homeward: ran on");
                        vec![]
                    }
                    Some((tasks, b)) => {
                        state.roof_try = Some((b, bot.failed, now));
                        tasks
                    }
                    None => run_for_cover(&world.zone, here, from, 0.0),
                }
            }
            // "Low behind it, and keep still": a still shape behind cover
            // isn't glimpsed; a moving one is, and found.
            First::ComingToLook if !p.reckless && !p.hoards => {
                if let Some(f) = fight.as_mut() {
                    f.flee_until = 0.0;
                }
                state.low_until = now + 30.0;
                // (Under the bot's 30 s give-up, or a frame past it is a
                // failure.)
                vec![Task::Wait(29.0)]
            }
            First::Bleeding => {
                let dressing = world.items.iter().find(|(_, it, _, held, sock, _)| {
                    held.is_none()
                        && sock.is_some()
                        && world.catalog.get(it.kind()).id == "field_dressing"
                });
                let dress = |e: Entity| {
                    vec![
                        Task::Stow(Side::Left),
                        Task::Grab {
                            item: e,
                            side: Side::Left,
                        },
                        Task::Use(Side::Left),
                    ]
                };
                // On its way under a roof from a drone: in first, then the
                // dressing, at the spot it waits (bound in the open, under
                // the drone's bursts, it lost 60 hp in nine seconds three
                // metres from the door).
                let sheltering = state.roof_try.is_some_and(|(_, _, t)| now - t < 25.0);
                let wait_at = bot.tasks.iter().position(|t| matches!(t, Task::Wait(_)));
                match dressing {
                    Some((e, ..)) if sheltering && wait_at.is_some() => {
                        let at = wait_at.unwrap_or(0);
                        for (k, t) in dress(e).into_iter().enumerate() {
                            bot.tasks.insert(at + k, t);
                        }
                        state.note(
                            now,
                            Kind::Learned,
                            "bleeding: in under the roof first, then the dressing",
                        );
                        vec![]
                    }
                    Some((e, ..)) => dress(e),
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
            // Warned: in a seeded fight, the machine's own tell; else
            // something said or shown lately (a line, a lock, an aim); or
            // the same fight still going (hit again within 15 s of a warned
            // hit: a drone that locked on keeps at it without locking again).
            let warned = match fight.as_ref().filter(|f| f.machine.is_some()) {
                Some(f) => {
                    f.last_tell().is_some_and(|t| now - t < 15.0)
                        || shot_warns(&state.shots, now, 15.0)
                }
                None => now - state.said_at < 30.0 || shot_warns(&state.shots, now, 30.0),
            } || now < state.warned_fight_until;
            if warned {
                state.warned_fight_until = now + 15.0;
            }
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
    // (With a gun to hand: in a hand or on the body, not at the bottom of
    // the pack; the last fall may have left it out there.)
    let armed = world.items.iter().any(|(_, it, _, held, sock, _)| {
        world.catalog.get(it.kind()).category == extraction_core::items::Category::Firearm
            && (held.is_some_and(|h| h.player == player)
                || sock.is_some_and(|s| world.owners.get(s.socket).is_ok_and(|o| o.0 == player)))
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
        super::second_hour::cut_in(&mut bot, take_it_on());
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
        // Resumed out in the valley ("Back where you were": the last stage
        // ended mid-raid): not at the camp, and no kit in the crate. Home
        // from here (it went to the crate for a radio that was out there
        // with it, and told no one).
        Leg::Camp if !matches!(raid.phase, Phase::AtBase | Phase::Down { .. }) => {
            state.note(
                now,
                Kind::Learned,
                "back where it was out there: home from here",
            );
            state.leg = match raid.phase {
                Phase::TruckWaiting { .. } => Leg::Rendezvous,
                Phase::TruckCalled { .. } => Leg::Rendezvous,
                _ => Leg::Call,
            };
        }
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
            // Empty-handed: nothing, the first time (it goes back if told).
            let takes = !p.empty_handed || state.went_back;
            for (e, id) in in_crate.into_iter().filter(|_| takes) {
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
            if takes && carried(&world, &view, player, "radio").is_empty() && !in_crate("radio") {
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
                    // Down over the tailgate, and out through the gate.
                    for (x, z) in [(0.05, 1.2), (0.0, 4.0)] {
                        tasks.push(Task::WalkTo {
                            to: at_truck(x, z),
                            within: 0.5,
                        });
                    }
                    if let Some(drop) = world
                        .zone
                        .map
                        .markers
                        .iter()
                        .find(|m| m.kind == MarkerKind::Insertion)
                        .map(|m| Vec2::from(m.at))
                    {
                        tasks.extend(route(&world, &view, at_truck(0.0, 4.0), drop));
                    }
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
            // Empty-handed: off the steps, and what's said there.
            if p.empty_handed && !state.went_back {
                if !state.stepped_off {
                    state.stepped_off = true;
                    for (x, z) in [(0.05, 1.2), (0.0, 4.0)] {
                        tasks.push(Task::WalkTo {
                            to: at_truck(x, z),
                            within: 0.5,
                        });
                    }
                    tasks.push(Task::Wait(1.5));
                    let idle = bot.tasks.pop_back();
                    bot.tasks.extend(tasks);
                    bot.tasks.extend(idle);
                    bot.restart();
                    return;
                }
                if state.heard.iter().any(|l| l.contains(" on you.")) {
                    state.note(
                        now,
                        Kind::Learned,
                        "off the truck with nothing: the driver said so, went back for the kit",
                    );
                    state.went_back = true;
                    for (x, z) in [(0.0, 4.0), (0.05, 1.2), (0.3, 0.3)] {
                        tasks.push(Task::WalkTo {
                            to: at_truck(x, z),
                            within: 0.4,
                        });
                    }
                    state.leg = Leg::Kit;
                    let idle = bot.tasks.pop_back();
                    bot.tasks.extend(tasks);
                    bot.tasks.extend(idle);
                    bot.restart();
                    return;
                }
                state.note(
                    now,
                    Kind::Confused,
                    "set off with nothing on me, and nothing said",
                );
                state.went_back = true;
                // Back up the steps: out from the bed, through the gate.
                tasks.push(Task::WalkTo {
                    to: at_truck(0.05, 1.2),
                    within: 0.5,
                });
                let idle = bot.tasks.pop_back();
                bot.tasks.extend(tasks);
                bot.tasks.extend(idle);
                bot.restart();
                return;
            }
            // The wanderer, gone off: back only once it's told the way.
            if let Some(off) = state.wandered_at
                && !state.told_back
            {
                let told = state
                    .heard
                    .iter()
                    .any(|l| l.contains("That's the job") || l.starts_with("The job: "));
                if told {
                    state.told_back = true;
                    state.note(
                        now,
                        Kind::Learned,
                        format!("wandered off; told which way {name} was, and went back"),
                    );
                } else if now - off > WANDER_WAIT {
                    state.note(
                        now,
                        Kind::Confused,
                        format!("wandered off from {name}, and nothing said which way it was"),
                    );
                    tasks.extend(way_out(&world, &view, &plans, here).0);
                    state.leg = Leg::Call;
                    let idle = bot.tasks.pop_back();
                    bot.tasks.extend(tasks);
                    bot.tasks.extend(idle);
                    bot.restart();
                    return;
                } else {
                    tasks.push(Task::Wait(3.0));
                    let idle = bot.tasks.pop_back();
                    bot.tasks.extend(tasks);
                    bot.tasks.extend(idle);
                    bot.restart();
                    return;
                }
            }
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
            // The wanderer: most of the way, then off elsewhere, away from
            // it (the playtest: in to 137 m, then west).
            if p.wanders && state.wandered_at.is_none() {
                let way = from.distance(at);
                let short = from.lerp(at, WANDER_SHARE);
                // Off to somewhere with a name, farther from the job (the
                // playtest: the logging camp, the village).
                let Some(off) = world
                    .zone
                    .map
                    .markers_of(&MarkerKind::Poi)
                    .map(|m| Vec2::from(m.at))
                    // Far enough out that the driver would say (past where
                    // it turned by the nudge's distance, and a margin).
                    .filter(|m| {
                        m.distance(at)
                            > (way * WANDER_OFF).max(
                                short.distance(at)
                                    + extraction_core::progression::NUDGE_AWAY
                                    + WANDER_MARGIN,
                            )
                    })
                    .min_by(|a, b| a.distance(short).total_cmp(&b.distance(short)))
                else {
                    state.wandered_at = Some(now);
                    state.told_back = true;
                    return;
                };
                // Up to it, not into it (a named place is often a building:
                // in at its door was a walk the bot got stuck on).
                let off = outside(&world, &view, short, off);
                tasks.extend(route(&world, &view, from, short));
                tasks.extend(by_road(&world, &view, short, off, at));
                state.wandered_at = Some(now);
                state.note(
                    now,
                    Kind::Confused,
                    format!("near {name}, then off elsewhere"),
                );
                let idle = bot.tasks.pop_back();
                bot.tasks.extend(tasks);
                bot.tasks.extend(idle);
                bot.restart();
                return;
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
                    tasks.extend(way_out(&world, &view, &plans, here).0);
                    state.leg = Leg::Call;
                    state.tries = 0;
                }
                Err(why) => {
                    state.note(now, Kind::Stuck, why);
                    tasks.extend(way_out(&world, &view, &plans, here).0);
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
                && !state.pinned
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
            if raid.phase == Phase::AtBase && state.home_at.is_none() {
                state.home_at = Some(now);
            }
            // A moment in the bed first: everything's where it was out
            // there until the drive home's settled.
            let settled = state.home_at.is_some_and(|t| now - t > 2.0);
            if raid.phase == Phase::AtBase && !settled {
                tasks.push(Task::Wait(1.0));
            } else if raid.phase == Phase::AtBase
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
                let told = state
                    .heard
                    .iter()
                    .any(|l| l.to_lowercase().contains("press the card"));
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
            } else if state.tries >= 3 && state.pinned {
                // Given up for the drone: not done, and the card says so.
                state.note(
                    now,
                    Kind::Learned,
                    "home, the job given up: unpaid, as it should be",
                );
                state.leg = Leg::Done;
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
    // Indoors, with a walk out of the building planned: out by its plan
    // first, its doors opened (sheltered from a drone, the newcomer was in
    // a house when the truck came; the walk to it went straight at the
    // wall, and it stood there half an hour).
    if let (Some(en), Some(Task::WalkTo { to, .. })) = (view.entrances.as_deref(), tasks.first()) {
        let inside = |p: Vec2| en.building_at(world.zone.on_ground(p.x, p.y) + Vec3::Y * 1.0);
        if let Some(b) = inside(here)
            && inside(*to) != Some(b)
        {
            let (mut out, _) = way_out(&world, &view, &plans, here);
            out.append(&mut tasks);
            tasks = out;
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
        .filter(|(_, k, _)| {
            matches!(
                k,
                Kind::Stuck | Kind::HurtUnwarned | Kind::Killed | Kind::Unready
            )
        })
        .count();
    // What the hurt count can't see: it compares hp a tick at a time.
    bot.log.push(
        "(hurt counts drops over 5 hp in one tick: smaller hits and damage over time aren't in it)"
            .into(),
    );
    let home = moments.iter().any(|(_, _, w)| w == "in the truck and away");
    if home && bad == 0 {
        bot.passed += 1;
        bot.log.push(format!("PASS {} got home", p.name));
    } else {
        bot.failed += 1;
        bot.log.push(format!(
            "FAIL {}: {}{bad} stuck, hurt unwarned, killed or unready",
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
    /// Low and still until then (a hound loses someone low and still).
    pub low_until: f32,
    pub tells: Vec<(f32, String)>,
    last_any: Option<f32>,
    pub first_hit: Option<f32>,
    reacted: bool,
    hp: f32,
    /// What was put on the route, and where.
    pub machine: Option<(String, Vec2)>,
}

impl Fight {
    /// The last tell, if any.
    /// When any tell last came (a second lock, a later aim): what warns of
    /// a hit. `tells` keeps each kind's first, for the report.
    pub fn last_tell(&self) -> Option<f32> {
        self.last_any
    }

    fn tell(&mut self, now: f32, what: &str) -> bool {
        self.last_any = Some(now);
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
    /// A hound coiling (0.6 s before the bite).
    hounds: MessageReader<'w, 's, crate::robots::hound::HoundCrouch>,
    /// A crawler's spark gun charging (0.5 s before each zap).
    crawlers: MessageReader<'w, 's, crate::robots::indoor::CrawlerCharge>,
    /// A heavy's arm rising (1.2 s before the slam), among its other news.
    heavies: MessageReader<'w, 's, crate::robots::heavy::HeavyEvent>,
    /// A grappler winding up a throw.
    harpoons: MessageReader<'w, 's, crate::robots::grappler::HarpoonWindup>,
    /// Rounds fired (each heard, the crack and the tracer, hit or miss).
    shots: MessageReader<'w, 's, crate::weapons::ballistics::Shot>,
    hushed: Query<'w, 's, (), (With<LocalPlayer>, With<crate::anomaly::hush::InHush>)>,
}

/// Something above you, or a sentry's sweep on you: in under a roof,
/// through the nearest door, if one's within a sprint (trees hide nobody
/// from overhead: only their trunks stop a look; a roof is all a drone
/// can't see through). To the door by the routed way (`walk`): straight at
/// it went into the garden fence in front of it.
fn under_a_roof(
    world: &World2,
    plans: &[crate::sim::indoor::BuildingPlan],
    here: Vec2,
    skip: &[Entity],
) -> Option<(Vec<Task>, Entity)> {
    let entrances = &*world.entrances;
    let d = entrances
        .doors
        .iter()
        .filter(|d| d.outside && !skip.contains(&d.building))
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
    // Just in: under the roof (2.5 m in was through Cinder's shop's
    // partition, from its back door).
    let inside = d.mid.xz() - out * 1.2;
    // Off to one side of the doorway: not in a look through it, and
    // somewhere the body can stand (the rooms' plan says; the shop's shelves
    // are in the way of some).
    let plan = plans.iter().find(|pl| pl.entity == d.building);
    let standable = |p: Vec2| match plan {
        Some(pl) => {
            let from = pl.to_local(Vec3::new(inside.x, d.mid.y, inside.y)).xz();
            let to = pl.to_local(Vec3::new(p.x, d.mid.y, p.y)).xz();
            // ...and walked to straight from just in (it's one walk: a way
            // round the shop's shelf on the plan left the newcomer against
            // the shelf, the drone at the door).
            pl.grid.line_clear(from.to_array(), to.to_array())
                && pl
                    .path(from, to)
                    .and_then(|path| path.last().copied())
                    .is_some_and(|end| end.distance(to) < 0.4)
        }
        None => entrances.building_at(Vec3::new(p.x, d.mid.y, p.y)) == Some(d.building),
    };
    let aside = [2.0, 1.4, 2.6]
        .into_iter()
        .flat_map(|k| {
            [
                out.perp() * k,
                -out.perp() * k,
                out.perp() * k - out,
                -out.perp() * k - out,
            ]
        })
        .map(|s| inside + s)
        .find(|p| standable(*p))
        .unwrap_or(inside);
    let door_out = d.mid.xz() + out * 1.3;
    let mut tasks = walk(world, here, door_out);
    match tasks.last_mut() {
        Some(Task::WalkTo { within, .. }) => *within = 0.6,
        _ => tasks.push(Task::WalkTo {
            to: door_out,
            within: 0.6,
        }),
    }
    tasks.extend([
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
        // And out the way it came in, not through a wall, the door opened
        // again (swung to while it waited, the newcomer stood against it
        // inside for the rest of the raid).
        Task::WalkTo {
            to: inside,
            within: 0.6,
        },
        Task::OpenDoor(d.door),
        Task::WalkTo {
            to: d.mid.xz() + out * 1.5,
            within: 0.8,
        },
    ]);
    Some((tasks, d.building))
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
fn round_buildings(world: &World2, from: Vec2, to: Vec2) -> Vec<Vec2> {
    let mut boxes: Vec<(Transform, Vec2)> = fence_boxes(&world.zone);
    boxes.extend(world.entrances.shells.iter().map(|s| (s.xf, s.half)));
    boxes.extend(
        world
            .buildings
            .iter()
            .filter(|(b, _)| solid(&b.prefab))
            .map(|(b, xf)| {
                let s = crate::robots::doors::Shell::of(Entity::PLACEHOLDER, &b.prefab, *xf);
                (s.xf, s.half)
            }),
    );
    round_boxes(&boxes, from, to)
}

/// The quarantine fence's runs as boxes (`structures::fence_runs`): the
/// way in or out is through a gap, where a road crosses it. (From the
/// camp, outside it, a newcomer resumed there walked at the fence to the
/// south-west and pressed against it for the rest of the raid.)
fn fence_boxes(zone: &crate::world::Zone) -> Vec<(Transform, Vec2)> {
    crate::world::structures::fence_runs(zone)
        .into_iter()
        .filter(|r| r.a.distance(r.b) > 0.5)
        .map(|r| {
            let d = r.b - r.a;
            let mid = (r.a + r.b) * 0.5;
            let xf = Transform::from_xyz(mid.x, 0.0, mid.y)
                .with_rotation(Quat::from_rotation_y((-d.y).atan2(d.x)));
            (xf, Vec2::new(d.length() * 0.5, 0.2))
        })
        .collect()
}

/// Something with no way in that a body can't walk through, over or under,
/// small enough to have no blocked run on the grid worth going round
/// (`SOLID_RUN`): a log pile, a wreck, the Custodian's stock, a staged
/// scene's crates, a chimney or a mast. (A log pile on the way from the
/// drop held the reckless newcomer a minute.) Not gates, fences, dams or
/// gantries: those are walked through or under. A garden's fence is
/// one (its panels stand till they're broken).
fn solid(prefab: &str) -> bool {
    prefab.starts_with(crate::world::scenes::PREFIX)
        || prefab.starts_with("wreck_")
        || prefab.starts_with("stock_")
        || matches!(
            prefab,
            "log_pile"
                | "chimney"
                | "radio_mast"
                | "water_tower"
                | "steeple"
                | "target"
                // A garden's fence panels: 1.3 m, over no step (the second
                // hour walked into one and stayed).
                | "fence_garden"
        )
}

/// The way from `from` to `to` round `boxes` (each a transform and its
/// half extents): the shortest way through their corners (out by `ROUND`)
/// where each straight stretch keeps `ROUND / 2` off every box, then `to`.
/// (A corner at a time, the way went through the next box over when two
/// stood close: a scene's crates against a log pile.) Straight there when
/// nothing's in the way, or no way round is found.
fn round_boxes(boxes: &[(Transform, Vec2)], from: Vec2, to: Vec2) -> Vec<Vec2> {
    let local = |bx: &Transform, q: Vec2| {
        bx.compute_affine()
            .inverse()
            .transform_point3(Vec3::new(q.x, 0.0, q.y))
            .xz()
    };
    let within = |l: Vec2, h: Vec2| l.x.abs() < h.x && l.y.abs() < h.y;
    // Whether a straight walk a..b keeps clear of a box: its margin, or the
    // box itself where an end is in the margin (a door's step, a corner).
    let clear_of = |bx: &(Transform, Vec2), a: Vec2, b: Vec2| {
        let (la, lb) = (local(&bx.0, a), local(&bx.0, b));
        let margin = bx.1 + Vec2::splat(ROUND * 0.5);
        let h = if within(la, margin) || within(lb, margin) {
            bx.1
        } else {
            margin
        };
        segment_hits_box(la, lb, h).is_none()
    };
    // The boxes near enough the way to matter.
    let near: Vec<&(Transform, Vec2)> = boxes
        .iter()
        .filter(|bx| {
            let c = bx.0.translation.xz();
            let d = to - from;
            let t = ((c - from).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
            c.distance(from + d * t) < bx.1.length() + ROUND + 20.0
        })
        .collect();
    let clear = |a: Vec2, b: Vec2| near.iter().all(|bx| clear_of(bx, a, b));
    if clear(from, to) {
        return vec![to];
    }
    // Each box's corners, out by `ROUND`, bar those in another box.
    let mut nodes = vec![from, to];
    for bx in &near {
        let h = bx.1 + Vec2::splat(ROUND);
        for c in [
            Vec2::new(-h.x, -h.y),
            Vec2::new(h.x, -h.y),
            Vec2::new(h.x, h.y),
            Vec2::new(-h.x, h.y),
        ] {
            let q = bx.0.transform_point(Vec3::new(c.x, 0.0, c.y)).xz();
            if !near
                .iter()
                .any(|o| within(local(&o.0, q), o.1 + Vec2::splat(ROUND * 0.5)))
            {
                nodes.push(q);
            }
        }
    }
    // Shortest way from node 0 to node 1 (Dijkstra; a few dozen nodes).
    let n = nodes.len();
    let mut dist = vec![f32::INFINITY; n];
    let mut prev = vec![usize::MAX; n];
    let mut done = vec![false; n];
    dist[0] = 0.0;
    loop {
        let Some(u) = (0..n)
            .filter(|&i| !done[i] && dist[i].is_finite())
            .min_by(|&a, &b| dist[a].total_cmp(&dist[b]))
        else {
            break;
        };
        if u == 1 {
            break;
        }
        done[u] = true;
        for v in 0..n {
            if done[v] || v == u {
                continue;
            }
            let d = dist[u] + nodes[u].distance(nodes[v]);
            if d < dist[v] && clear(nodes[u], nodes[v]) {
                dist[v] = d;
                prev[v] = u;
            }
        }
    }
    if !dist[1].is_finite() {
        return vec![to];
    }
    let mut way = vec![];
    let mut at = 1;
    while at != 0 {
        way.push(nodes[at]);
        at = prev[at];
    }
    way.reverse();
    way
}

/// Where along a→b (0..1) it first enters the box |x| < half, if it does.
pub(super) fn segment_hits_box(a: Vec2, b: Vec2, half: Vec2) -> Option<f32> {
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

/// From `from` to `to` by a road that runs near both (as someone walks off
/// somewhere they've no reason to go: the way that's there), else straight.
fn by_road(world: &World2, view: &RaidView, from: Vec2, to: Vec2, keep_off: Vec2) -> Vec<Task> {
    const NEAR: f32 = 70.0;
    // Not a road past the place it's wandering off from (Cinder's track
    // runs by Ash Row: walked, the place was seen).
    const OFF: f32 = 40.0;
    let nearest = |pts: &[Vec2], p: Vec2| {
        pts.iter()
            .enumerate()
            .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
            .map(|(i, q)| (i, q.distance(p)))
    };
    let best = world
        .zone
        .map
        .roads
        .iter()
        .filter_map(|r| {
            let pts: Vec<Vec2> = r.points.iter().map(|p| Vec2::from(*p)).collect();
            let (i, a) = nearest(&pts, from)?;
            let (j, b) = nearest(&pts, to)?;
            let along = &pts[i.min(j)..=i.max(j)];
            let clear = along.iter().all(|p| p.distance(keep_off) > OFF);
            (a < NEAR && b < NEAR && i != j && clear).then_some((pts, i, j, a + b))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3));
    let Some((pts, i, j, _)) = best else {
        return route(world, view, from, to);
    };
    let along: Vec<Vec2> = if i < j {
        pts[i..=j].to_vec()
    } else {
        pts[j..=i].iter().rev().copied().collect()
    };
    let mut t = vec![];
    let mut at = from;
    for p in along.into_iter().chain(std::iter::once(to)) {
        t.extend(route(world, view, at, p));
        at = p;
    }
    t
}

/// `to`, or if that's inside a building, a couple of metres short of its
/// wall on the way from `from`.
fn outside(world: &World2, view: &RaidView, from: Vec2, to: Vec2) -> Vec2 {
    let Some(en) = view.entrances.as_deref() else {
        return to;
    };
    let inside = |p: Vec2| {
        en.building_at(world.zone.on_ground(p.x, p.y) + Vec3::Y)
            .is_some()
    };
    if !inside(to) {
        return to;
    }
    let back = (from - to).normalize_or_zero();
    (1..60)
        .map(|k| to + back * k as f32)
        .find(|p| !inside(*p))
        .map_or(to, |p| p + back * 2.0)
}

/// A blocked stretch this long on the walking grid is something solid to go
/// round, not a gate a body walks through (m). Measured: the South gates'
/// openings read 6 and 8 m blocked; Cinder's blast furnace, 50.
const SOLID_RUN: f32 = 12.0;

/// Ground steeper than the machines' grid takes (`map_nav::MAX_SLOPE`)
/// for this far along the way is a bank to go round, however short: no
/// gate's that steep (a 54-degree bank by the radio mast held the reckless
/// newcomer half a minute, two metres short of the top) (m).
const STEEP_RUN: f32 = 1.0;

/// The longest run along `a`..`b` of ground steeper than
/// `map_nav::MAX_SLOPE` (m).
fn steep_run(zone: &crate::world::Zone, a: Vec2, b: Vec2) -> f32 {
    let step = 0.5;
    let n = (a.distance(b) / step).ceil().max(1.0) as usize;
    let (mut run, mut best) = (0.0f32, 0.0f32);
    for k in 0..=n {
        let q = a.lerp(b, k as f32 / n as f32);
        if zone.hm.slope_deg(q.to_array()) > extraction_core::map_nav::MAX_SLOPE {
            run += step;
            best = best.max(run);
        } else {
            run = 0.0;
        }
    }
    best
}

/// The longest run of blocked ground on the grid along `a`..`b` (m).
fn blocked_run(nav: &extraction_core::nav::NavGrid, a: Vec2, b: Vec2) -> f32 {
    let step = nav.cell * 0.5;
    let n = (a.distance(b) / step).ceil().max(1.0) as usize;
    let (mut run, mut best) = (0.0f32, 0.0f32);
    for k in 0..=n {
        if nav.walkable(a.lerp(b, k as f32 / n as f32).to_array()) {
            run = 0.0;
        } else {
            run += step;
            best = best.max(run);
        }
    }
    best
}

/// This many drone locks within `PINNED_FOR` (s), cover taken each time:
/// pinned, and home.
const PINNED_AFTER: usize = 3;
const PINNED_FOR: f32 = 120.0;

/// A round in flight is this long at most (s): a shot this recent may be
/// the one that hit, and warns of nothing.
const SHOT_LEAD: f32 = 0.6;

/// Whether rounds fired at it warn of a hit `now`: one fired at least
/// `SHOT_LEAD` before (heard, the crack and the tracer), within `window`.
/// The round that lands first, with no tell before it, isn't warned by
/// itself.
fn shot_warns(shots: &[f32], now: f32, window: f32) -> bool {
    shots
        .iter()
        .any(|t| now - t >= SHOT_LEAD && now - t < window)
}

/// A fight, for the reckless one: the gun out, readied by the one in hand
/// (`Task::Ready`: the PM comes out of the crate on safe, and with two on
/// it, the one drawn may not be the one first found), and fired.
fn take_it_on() -> Vec<Task> {
    vec![
        Task::DrawGun,
        Task::Ready { off_safe: true },
        Task::Engage { shots: 12 },
    ]
}

/// Whether a gun on `player`'s body has a round up (any: a rifle picked
/// up on the job, empty, beside the racked pistol, read as unready).
fn one_up<'a>(
    mut guns: impl Iterator<Item = (&'a crate::weapons::Gun, Entity, bool)>,
    player: Entity,
) -> bool {
    guns.any(|(g, by, packed)| {
        by == player && !packed && g.firearm.chambers.iter().any(|c| c.is_some())
    })
}

/// `legs`, round the buildings in the way.
fn route(world: &World2, _view: &RaidView, from: Vec2, to: Vec2) -> Vec<Task> {
    walk(world, from, to)
}

/// The way on foot outdoors from `from` to `to`, as `legs`: round the
/// buildings and solids in the way, through the fence's gaps, round the
/// banks (`route`'s; the second hour's walked straight, into a garden's
/// fence and the wire).
pub(super) fn walk(world: &World2, from: Vec2, to: Vec2) -> Vec<Task> {
    let mut at = from;
    let mut t = vec![];
    for p in round_buildings(world, from, to) {
        // Something solid in the way that has no doors (the stoves at Cinder
        // Works' blast furnace held a newcomer twenty minutes): the
        // machines' grid's way round it.
        let round = world
            .nav
            .as_ref()
            // (Outdoors to outdoors: indoors is `round_buildings`' and the
            // rooms' plans'.)
            .filter(|n| n.0.walkable(at.to_array()) && n.0.walkable(p.to_array()))
            // A post or a gate's narrow way isn't worth a detour (the
            // grid's clearance shuts what a body walks through, and its way
            // round Marrow's gate went into the fence): only a long solid.
            .filter(|n| {
                blocked_run(&n.0, at, p) > SOLID_RUN || steep_run(&world.zone, at, p) > STEEP_RUN
            })
            .and_then(|n| n.0.path(at.to_array(), p.to_array()));
        let mut way: Vec<Vec2> = round
            .map(|w| w.into_iter().skip(1).map(Vec2::from).collect())
            .unwrap_or_default();
        way.push(p);
        // The grid's cells are too coarse to see a short bank: its way can
        // still go up one (the radio mast's, again). Round each that's left.
        for q in way {
            for r in round_banks(&world.zone, at, q, 3) {
                t.extend(world.legs(at, r));
                at = r;
            }
        }
        at = p;
    }
    t
}

/// `a`..`b` round any bank too steep to walk up on it (`steep_run`): by a
/// point off to one side of its middle, the nearest that clears it both
/// ways, as far as `depth` deep. The points after `a`, to `b`.
fn round_banks(zone: &crate::world::Zone, a: Vec2, b: Vec2, depth: u32) -> Vec<Vec2> {
    if depth == 0 || steep_run(zone, a, b) <= STEEP_RUN {
        return vec![b];
    }
    let mid = (a + b) * 0.5;
    let side = (b - a).perp().normalize_or(Vec2::X);
    let clear = |m: Vec2| steep_run(zone, a, m) <= STEEP_RUN && steep_run(zone, m, b) <= STEEP_RUN;
    let off = [4.0f32, 8.0, 12.0, 18.0, 25.0, 35.0]
        .into_iter()
        .flat_map(|d| [mid + side * d, mid - side * d])
        .find(|m| clear(*m))
        .unwrap_or(mid + side * 12.0);
    let mut out = round_banks(zone, a, off, depth - 1);
    out.extend(round_banks(zone, off, b, depth - 1));
    out
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
/// Through the rooms by the building's plan, where it has one (straight at
/// the door from behind the post office's counter, the newcomer pressed
/// against the counter for the rest of the raid, the watch in its pack).
fn way_out(
    world: &World2,
    view: &RaidView,
    plans: &[crate::sim::indoor::BuildingPlan],
    here: Vec2,
) -> (Vec<Task>, Vec2) {
    let Some(en) = view.entrances.as_deref() else {
        return (vec![], here);
    };
    let at = world.zone.on_ground(here.x, here.y) + Vec3::Y * 1.0;
    if let Some(plan) = plans.iter().find(|pl| pl.contains(at, 0.3)) {
        let from = plan.to_local(at).xz();
        let way = plan
            .doors
            .iter()
            .filter_map(|d| d.outside)
            .filter_map(|o| Some((o, plan.path(from, o)?)))
            .min_by(|a, b| path_len(from, &a.1).total_cmp(&path_len(from, &b.1)));
        if let Some((o, path)) = way {
            let mut t = Vec::new();
            for stop in indoor_stops(plan, from, &path) {
                match stop {
                    Stop::Door(door, before) => {
                        t.push(Task::WalkTo {
                            to: plan.world2(before),
                            within: 0.4,
                        });
                        t.push(Task::OpenDoor(door));
                    }
                    Stop::At(w) => t.push(Task::WalkTo {
                        to: plan.world2(w),
                        within: 0.4,
                    }),
                }
            }
            if let Some(Task::WalkTo { within, .. }) = t.last_mut() {
                *within = 0.6;
            }
            return (t, plan.world2(o));
        }
    }
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

/// How far a walk along `path` from `from` goes (m).
fn path_len(from: Vec2, path: &[Vec2]) -> f32 {
    let mut at = from;
    path.iter()
        .map(|p| {
            let d = at.distance(*p);
            at = *p;
            d
        })
        .sum()
}

/// A place a newcomer walks to on the way through a building
/// (building-local).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stop {
    /// Walk to it.
    At(Vec2),
    /// Up to this side of the door, then open it.
    Door(Entity, Vec2),
}

/// The stops along an indoor `path` from `o`: each door the way crosses,
/// and the waypoints a straight walk can't skip. A waypoint is left out
/// only where the walk from the last stop on to the next stays clear
/// (dropping three in four cut a corner through the post office's table
/// and chair, and the newcomer climbed onto the table and stuck there).
fn indoor_stops(plan: &crate::sim::indoor::BuildingPlan, o: Vec2, path: &[Vec2]) -> Vec<Stop> {
    let mut out = Vec::new();
    let (mut prev, mut kept) = (o, o);
    let last = path.len().saturating_sub(1);
    for (i, &w) in path.iter().enumerate() {
        let doors = plan.doors_on(prev, w);
        if doors.is_empty()
            && i != last
            && plan
                .grid
                .line_clear(kept.to_array(), path[i + 1].to_array())
        {
            prev = w;
            continue;
        }
        for (door, cross) in doors {
            // This side of where the way crosses it; where that's past a
            // jamb (by a bunker's narrow door), the waypoint before it.
            let mut before = cross + (prev - cross).normalize_or_zero() * 0.8;
            let clear = |a: Vec2, b: Vec2| plan.grid.line_clear(a.to_array(), b.to_array());
            if !clear(kept, before) && prev != kept {
                out.push(Stop::At(prev));
                kept = prev;
            }
            if !clear(kept, before) {
                before = kept;
            }
            out.push(Stop::Door(door, before));
        }
        out.push(Stop::At(w));
        prev = w;
        kept = w;
    }
    out
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
    let (mut t, here) = way_out(world, view, plans, here);
    if let Some(plan) = plans.iter().find(|pl| pl.contains(p, 0.3)) {
        let goal = plan.to_local(p).xz();
        let entry = plan
            .doors
            .iter()
            .filter_map(|d| d.outside)
            .min_by(|a, b| a.distance(goal).total_cmp(&b.distance(goal)));
        if let Some((o, path)) = entry.and_then(|o| Some((o, plan.path(o, goal)?))) {
            t.extend(route(world, view, here, plan.world2(o)));
            for stop in indoor_stops(plan, o, &path) {
                match stop {
                    Stop::Door(door, before) => {
                        t.push(Task::WalkTo {
                            to: plan.world2(before),
                            within: 0.4,
                        });
                        t.push(Task::OpenDoor(door));
                    }
                    Stop::At(w) => t.push(Task::WalkTo {
                        to: plan.world2(w),
                        within: 0.4,
                    }),
                }
            }
            return t;
        }
    }
    let door = view.entrances.as_deref().and_then(|en| {
        let b = en.building_at(p)?;
        en.doors
            .iter()
            .filter(|d| d.outside && d.building == b)
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
            let target = photo_subject(&world.zone, view, *subject, here);
            let Some(p) = target.filter(|p| p.xz().distance(here) < 250.0) else {
                // None about: where the card says he's heard them (it's
                // read again: the line comes once one's heard), else round
                // the valley's places to look, a while at each.
                if let Some((_, at)) = view
                    .card
                    .iter()
                    .next()
                    .and_then(|(_, c)| place_on_card(&c.text, &world.zone))
                    .filter(|(_, at)| at.distance(here) > 25.0 && tries < 6)
                {
                    let mut t = route(world, view, here, at);
                    t.push(Task::Wait(10.0));
                    return Ok(Some((t, None)));
                }
                if tries > 8 {
                    return Err(
                        format!("no {subject:?} to be found for the picture").to_lowercase()
                    );
                }
                let mut places: Vec<Vec2> = world
                    .zone
                    .map
                    .markers
                    .iter()
                    .filter(|m| m.kind == MarkerKind::Poi)
                    .map(|m| Vec2::from(m.at))
                    .collect();
                places.sort_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
                let Some(&to) = places.get(tries as usize % places.len().max(1)) else {
                    return Err("nowhere to look for one".into());
                };
                let mut t = route(world, view, here, to);
                t.push(Task::Wait(10.0));
                return Ok(Some((t, None)));
            };
            // As far off as the picture takes (the camera's 60 m), a little
            // inside it, and nearer each time it's not in it (something in
            // the way): not so near it notices.
            let away = (here - p.xz()).normalize_or(Vec2::Y);
            let stand = p.xz() + away * (48.0 - 7.0 * tries as f32).max(25.0);
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
                // And back off from it, the way it came.
                Task::WalkTo {
                    to: stand + away * 25.0,
                    within: 2.0,
                },
            ]);
            Ok(Some((t, None)))
        }
    }
}

/// The nearest machine a photograph job wants, up here where it can be
/// seen (not one at work underground).
fn photo_subject(
    zone: &crate::world::Zone,
    view: &RaidView,
    subject: Subject,
    here: Vec2,
) -> Option<Vec3> {
    view.robots
        .iter()
        .filter(|c| c.findable(subject, zone))
        .map(|c| c.at.translation())
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
    mut bots: Query<(Entity, &mut Bot, &Transform, &mut crate::rig::RigInput), With<LocalPlayer>>,
    health: Query<&crate::health::Health, With<LocalPlayer>>,
    world: World2,
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
            // Its gun dead, limping at you from just off the way.
            "broken" => {
                let p = at + side * crate::robots::broken::ASIDE;
                let site = zone.on_ground(p.x, p.y);
                crate::robots::broken::spawn_broken_walker(
                    &mut commands,
                    &zone,
                    site,
                    site + Vec3::new(0.0, 0.0, -300.0),
                );
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
    for _ in tells.hounds.read() {
        came.push("a hound coiling (haunches down)");
    }
    for _ in tells.crawlers.read() {
        came.push("a crawler's spark gun charging (crackle, glow)");
    }
    for ev in tells.heavies.read() {
        if ev.what == crate::robots::heavy::HeavyAct::Raise {
            came.push("a heavy's arm rising");
        }
    }
    for _ in tells.harpoons.read() {
        came.push("a grappler winding up (the reel)");
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
    let Ok((_, mut bot, root, _)) = bots.single_mut() else {
        return;
    };
    fight.reacted = true;
    let here = root.translation.xz();
    let from = fight.machine.as_ref().map_or(here, |(_, at)| *at);
    let away = (here - from).normalize_or(Vec2::Y);
    let hush = what.contains("hush");
    // The broken walker is the one the driver says to take on (bar the
    // lost one).
    let broken = kind == "broken" && !p.lost;
    // A drone or a sentry: under the nearest roof, by the way round, for
    // all who'd listen (the lost one too: it's told, and does as told).
    let roofed = matches!(kind, "drone" | "sentry") && !p.reckless && !p.hoards && !hush;
    let roof = roofed
        .then(|| under_a_roof(&world, &[], here, &[]))
        .flatten();
    // A hound coiling: "Low behind it, and keep still" (a hound loses
    // someone low and still, even close by), not upright for cover, which
    // it runs down. For all who'd listen.
    let hound = kind == "hound" && !p.reckless && !p.hoards && !hush;
    let answer: Vec<Task> = if (p.reckless || broken) && !hush {
        take_it_on()
    } else if hound {
        fight.low_until = now + 30.0;
        fight.flee_until = 0.0;
        // (Under the bot's 30 s give-up.)
        vec![Task::Wait(29.0)]
    } else if let Some((tasks, _)) = roof {
        fight.flee_until = now + 12.0;
        tasks
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
        // For cover, as the driver says, upright and fast (no roof near).
        fight.flee_until = now + 12.0;
        run_for_cover(&zone, here, from, 0.0)
    };
    if !answer.is_empty() {
        super::second_hour::cut_in(&mut bot, answer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// To the truck's steps from all round it (and from against its side,
    /// where the newcomer stood at the ranger tower), no leg goes through
    /// its body.
    #[test]
    fn the_way_to_the_steps_goes_round_the_truck() {
        let (centre, half) = super::super::second_hour::TRUCK_BOX;
        let mut from: Vec<Vec2> = (0..24)
            .map(|k| Vec2::from_angle(k as f32 / 24.0 * std::f32::consts::TAU) * 9.0)
            .collect();
        for z in [-4.0, -2.0, 0.0, 1.5] {
            from.extend([Vec2::new(2.2, z), Vec2::new(-2.2, z)]);
        }
        for here in from {
            let mut at = here;
            for p in super::super::second_hour::round_the_truck(here) {
                assert!(
                    segment_hits_box(at - centre, p - centre, half).is_none(),
                    "from {here}: {at} -> {p} goes through the truck"
                );
                at = p;
            }
            assert_eq!(at, Vec2::new(0.0, 4.0), "from {here}");
        }
    }

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

#[cfg(test)]
mod indoor_tests {
    use super::{
        STEEP_RUN, Stop, indoor_stops, round_banks, round_boxes, segment_hits_box, shot_warns,
        solid, steep_run,
    };
    use bevy::ecs::system::SystemState;
    use bevy::prelude::*;

    /// Indoors, each straight walk between a newcomer's stops stays on the
    /// building plan's open cells (furniture and its clearance kept out):
    /// from the door nearest each loot spot in every building on Marrow, to
    /// the spot, and back out. (The post office's table and chair were
    /// walked through, and climbed.)
    #[test]
    fn indoor_walks_go_round_the_furniture() {
        let opts = crate::launch::LaunchOptions {
            robots: false,
            map: "marrow_creek".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let w = app.world_mut();
        let mut st: SystemState<(
            Query<(Entity, &crate::world::buildings::Building, &Transform)>,
            Query<(Entity, &crate::world::buildings::DoorLeaf, &ChildOf)>,
        )> = SystemState::new(w);
        let (b, d) = st.get(w).unwrap();
        let plans = crate::sim::indoor::plan_buildings(&b, &d);
        let spots: Vec<Vec3> = w
            .get_resource::<crate::world::buildings::BuildingLoot>()
            .map(|l| l.0.iter().map(|s| s.at).collect())
            .unwrap_or_default();
        let (mut walks, mut cut) = (0, Vec::new());
        for plan in &plans {
            for spot in spots.iter().filter(|s| plan.contains(**s, 0.3)) {
                let goal = plan.to_local(*spot).xz();
                let Some(o) = plan
                    .doors
                    .iter()
                    .filter_map(|d| d.outside)
                    .min_by(|a, b| a.distance(goal).total_cmp(&b.distance(goal)))
                else {
                    continue;
                };
                let Some(path) = plan.path(o, goal) else {
                    continue;
                };
                // In to the spot, and back out from it (straight at the
                // door from behind the post office's counter, the newcomer
                // pressed against the counter).
                // (From where it stood to take it: the spot's on a shelf.)
                let back = plan.path(goal, o).unwrap_or_default();
                let stood = back.first().copied().unwrap_or(goal);
                for (start, path) in [(o, path), (stood, back)] {
                    walks += 1;
                    let mut from = Some(start);
                    for stop in indoor_stops(plan, start, &path) {
                        match stop {
                            Stop::Door(_, before) => {
                                if let Some(f) = from {
                                    if !plan.grid.line_clear(f.to_array(), before.to_array()) {
                                        cut.push(format!("{} {f} -> {before}", plan.name));
                                    }
                                }
                                // Through the door: its leaf is in the way till
                                // it's open.
                                from = None;
                            }
                            Stop::At(p) => {
                                if let Some(f) = from {
                                    if !plan.grid.line_clear(f.to_array(), p.to_array()) {
                                        cut.push(format!("{} {f} -> {p}", plan.name));
                                    }
                                }
                                from = Some(p);
                            }
                        }
                    }
                }
            }
        }
        assert!(walks > 100, "only {walks} indoor walks tried");
        assert!(
            cut.is_empty(),
            "{} of {walks} indoor walks cut through furniture:\n{}",
            cut.len(),
            cut.join("\n")
        );
    }

    /// The first fight's drone is met within a sprint of a roof (a door in
    /// to it, from outside): open ground is no excuse for losing to it,
    /// so a newcomer who does as told can live. (Cautious went down there:
    /// it had sheltered, but bound a wound in the open on its way back in.)
    #[test]
    fn a_roof_is_in_reach_of_the_first_fights_drone() {
        let opts = crate::launch::LaunchOptions {
            robots: false,
            map: "marrow_creek".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..30 {
            app.update();
        }
        let w = app.world();
        let zone = w.resource::<crate::world::Zone>();
        let en = w.resource::<crate::robots::doors::Entrances>();
        let (at, _) = super::on_the_way(zone);
        let mut ds: Vec<(f32, Vec2)> = en
            .doors
            .iter()
            .filter(|d| d.outside)
            .filter(|d| {
                let inside = d.mid - d.out.normalize_or(Vec3::Z) * 1.5;
                en.building_at(inside) == Some(d.building)
            })
            .map(|d| (d.mid.xz().distance(at), d.mid.xz()))
            .collect();
        ds.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(
            ds.first().is_some_and(|(d, _)| *d < 30.0),
            "nearest roof from the fight at {at}: {:?}",
            ds.first()
        );
    }

    /// Outdoors, a walk straight through a solid with no way in (a log
    /// pile, a wreck, the stock, a scene's crates) goes round it instead:
    /// every one on Marrow, crossed through its middle from either side.
    /// And a bank too steep to walk up is one, however short.
    #[test]
    fn outdoor_walks_go_round_what_cant_be_walked_through() {
        let opts = crate::launch::LaunchOptions {
            robots: false,
            map: "marrow_creek".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let w = app.world_mut();
        let mut q = w.query::<(&crate::world::buildings::Building, &Transform)>();
        let boxes: Vec<(Transform, Vec2)> = q
            .iter(w)
            .filter(|(b, _)| solid(&b.prefab))
            .map(|(b, xf)| {
                let s = crate::robots::doors::Shell::of(Entity::PLACEHOLDER, &b.prefab, *xf);
                (s.xf, s.half)
            })
            .collect();
        assert!(boxes.len() > 20, "only {} solids on Marrow", boxes.len());
        let mut through = Vec::new();
        for (xf, half) in &boxes {
            let c = xf.translation.xz();
            let reach = half.length() + 6.0;
            // (An end inside a solid has no way to it: two garden panels
            // end to end, walked along their line.)
            let inside = |q: Vec2| {
                boxes.iter().any(|(bx, h)| {
                    let l = bx
                        .compute_affine()
                        .inverse()
                        .transform_point3(Vec3::new(q.x, 0.0, q.y));
                    l.x.abs() < h.x + 1.0 && l.z.abs() < h.y + 1.0
                })
            };
            for dir in [Vec2::X, Vec2::Y, Vec2::new(1.0, 1.0).normalize()] {
                let (from, to) = (c - dir * reach, c + dir * reach);
                if inside(from) || inside(to) {
                    continue;
                }
                let mut at = from;
                for p in round_boxes(&boxes, from, to) {
                    let inv = xf.compute_affine().inverse();
                    let a = inv.transform_point3(Vec3::new(at.x, 0.0, at.y)).xz();
                    let b = inv.transform_point3(Vec3::new(p.x, 0.0, p.y)).xz();
                    if segment_hits_box(a, b, *half).is_some() {
                        through.push(format!("{c} from {from}: {at} -> {p}"));
                    }
                    at = p;
                }
            }
        }
        assert!(
            through.is_empty(),
            "{} walks went through a solid:\n{}",
            through.len(),
            through.join("\n")
        );
        let zone = w.resource::<crate::world::Zone>();
        // From the camp, outside the fence, into the valley: through a gap
        // (the gate), not into the wire.
        let fence = super::fence_boxes(zone);
        assert!(!fence.is_empty(), "Marrow has a fence");
        for (from, to) in [
            (Vec2::new(12.1, 165.0), Vec2::new(-104.1, -43.0)),
            (Vec2::new(22.0, 177.6), Vec2::new(-108.0, -50.0)),
        ] {
            assert!(
                fence.iter().any(|(xf, half)| {
                    let inv = xf.compute_affine().inverse();
                    let a = inv.transform_point3(Vec3::new(from.x, 0.0, from.y)).xz();
                    let b = inv.transform_point3(Vec3::new(to.x, 0.0, to.y)).xz();
                    segment_hits_box(a, b, *half).is_some()
                }),
                "{from} -> {to} straight crosses the fence"
            );
            let mut at = from;
            for p in round_boxes(&fence, from, to) {
                for (xf, half) in &fence {
                    let inv = xf.compute_affine().inverse();
                    let a = inv.transform_point3(Vec3::new(at.x, 0.0, at.y)).xz();
                    let b = inv.transform_point3(Vec3::new(p.x, 0.0, p.y)).xz();
                    assert!(
                        segment_hits_box(a, b, *half).is_none(),
                        "{from} -> {to}: {at} -> {p} goes through the fence"
                    );
                }
                at = p;
            }
        }
        let bank = steep_run(zone, Vec2::new(-38.1, -41.7), Vec2::new(-39.97, -45.04));
        assert!(bank > STEEP_RUN, "the radio mast's bank: {bank} m steep");
        let road = steep_run(zone, Vec2::new(22.0, 148.5), Vec2::new(9.25, 109.69));
        assert!(road <= STEEP_RUN, "the way from the drop: {road} m steep");
        // Straight over the bank (as the grid's way went), it's rounded:
        // no stretch of the way left steep.
        // (On past the bank, to ground a body stands on: the leg's own end
        // was at its foot.)
        let a = Vec2::new(-36.47, -41.82);
        let b = (1..=12)
            .map(|k| a + (Vec2::new(-45.56, -53.54) - a) * (1.5 + k as f32 * 0.25))
            .find(|q| zone.hm.slope_deg(q.to_array()) < 20.0)
            .expect("flat ground past the bank");
        assert!(steep_run(zone, a, b) > STEEP_RUN, "the bank's on that line");
        let mut at = a;
        for p in round_banks(zone, a, b, 3) {
            let s = steep_run(zone, at, p);
            assert!(s <= STEEP_RUN, "{at} -> {p}: {s} m steep");
            at = p;
        }
    }

    /// A machine's rounds warn of later hits only: the first round that
    /// lands, with nothing before it, is unwarned; once it's firing (a
    /// burst 1.8 s back), the next hit is warned.
    #[test]
    fn a_shot_warns_only_of_hits_after_it() {
        // The first round, fired now (or a moment ago) and hitting now.
        assert!(!shot_warns(&[100.0], 100.0, 30.0));
        assert!(!shot_warns(&[99.7], 100.0, 30.0));
        // Fired 1.8 s before this hit: heard first.
        assert!(shot_warns(&[98.2, 100.0], 100.0, 30.0));
        // Long ago: no longer the same fight.
        assert!(!shot_warns(&[60.0], 100.0, 30.0));
        assert!(!shot_warns(&[], 100.0, 30.0));
    }
}
