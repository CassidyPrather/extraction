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
pub const ENCOUNTERS: [&str; 8] = [
    "walker", "drone", "hush", "sentry", "night", "broken", "hound", "heavy",
];

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
    // Everything else on the command line goes to both sessions, bar the
    // valley (`--map`), which is the raids' (the yard is the yard).
    let (passthrough, map) = raids_map(passthrough());
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
            // a job, each in the valley the career's headed for, as the
            // game opens itself again (a card in another valley: the route
            // card turned, the bell rung, the next raid there).
            (0..chain.raids).map(|i| {
                let first = i == 0;
                (
                    RAID,
                    if first {
                        vec!["--map".to_string(), map.clone()]
                    } else {
                        vec![]
                    },
                )
            }),
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

/// The valley a career's raids are in (`--map`, Marrow if none's given),
/// and the rest of the command line without it.
fn raids_map(args: Vec<String>) -> (Vec<String>, String) {
    let mut rest = Vec::new();
    let mut map = "marrow_creek".to_string();
    let mut args = args.into_iter();
    while let Some(a) = args.next() {
        if a == "--map" {
            if let Some(m) = args.next() {
                map = m;
            }
        } else {
            rest.push(a);
        }
    }
    (rest, map)
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
    // Marrow's, unless another valley's asked for (`--map`): each valley's
    // first fight, on the way to the job its card names.
    let map = pass
        .iter()
        .position(|a| a == "--map")
        .and_then(|i| pass.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "marrow_creek".to_string());
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
                .args(["--data-dir", &dir_s, "--save", &save, "--map", &map]);
            if p.one_handed {
                cmd.arg("--one-handed");
            }
            if *e == "night" {
                cmd.args(["--hour", "night"]);
            }
            println!("first fight: {} meets the {e}", p.name);
            // (Its output passed on as it was, and read for the warning.)
            let out = cmd.output();
            let (ok, said) = match &out {
                Ok(o) => {
                    use std::io::Write;
                    let _ = std::io::stdout().write_all(&o.stdout);
                    let _ = std::io::stderr().write_all(&o.stderr);
                    let text = String::from_utf8_lossy(&o.stdout).to_string()
                        + &String::from_utf8_lossy(&o.stderr);
                    (o.status.success(), text)
                }
                Err(_) => (false, String::new()),
            };
            // The reckless one against a drone: it takes it on in the open,
            // and in the open after a warning is meant to lose. By design
            // only if the warning played (its lock-on chirp) and a roof was
            // in reach (`a_roof_is_in_reach_of_the_first_fights_drone`).
            let reckless_drone =
                p.reckless && *e == "drone" && said.contains("tell: a drone's lock-on");
            // ...and it does take it on: armed, locked on, it shoots back
            // (twelve raids after a lost kit, none did, and all went down).
            if reckless_drone && !said.contains("shot back at it") {
                println!(
                    "first fight: {}/{e}: locked on, armed, and it never shot back",
                    p.name
                );
                failed.push(format!("{}/{e}", p.name));
                continue;
            }
            if !ok {
                if p.hoards || reckless_drone {
                    // By design: it carries on as if nothing were there,
                    // the driver's warnings and all, and out in the open
                    // after a warning is meant to lose (Robot minds'
                    // fairness rules). Its going down is the game being
                    // fair, not a failure of what it was told.
                    println!(
                        "first fight: {}/{e} didn't come through, by design ({})",
                        p.name,
                        if p.hoards {
                            "it heeds no warning"
                        } else {
                            "it took the drone on in the open, warned, with a roof in reach"
                        }
                    );
                    by_design.push(format!("{}/{e}", p.name));
                } else {
                    failed.push(format!("{}/{e}", p.name));
                }
            }
        }
    }
    if !by_design.is_empty() {
        println!("first fight: down by design: {by_design:?}");
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
    /// The card's place is in another valley: the route card turned to
    /// it, and the bell rung.
    Route,
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
        "shelter" => return shelter_job(&map),
        _ => return None,
    })
}

/// The job card's thing in the shelter's room farthest from any way out of
/// it (the site with a lockdown: Marrow's bunker): down there, the
/// newcomer has to come up through the site to get home. A regression run
/// (`--job shelter`), so a career that no longer goes underground can't
/// hide a walk from below that goes straight at the surface through the
/// rock (the shelter on Marrow, seed 3 job 2).
pub fn shelter_job(map: &str) -> Option<Job> {
    use extraction_core::underground as ug;
    let def = crate::world::load_map_def(map);
    let dist = |a: [f32; 2], b: [f32; 2]| Vec2::from(a).distance(Vec2::from(b));
    ug::placed(&def).into_iter().find_map(|p| {
        let l = ug::layout(&p.id)?;
        l.lockdown.as_ref()?;
        let room = l.rooms.iter().filter(|r| !r.pit).max_by(|a, b| {
            let out = |r: &ug::Room| {
                l.portals
                    .iter()
                    .map(|q| dist(q.at, r.at))
                    .fold(f32::MAX, f32::min)
            };
            out(a).total_cmp(&out(b))
        })?;
        // Named on the card by the place nearest its way in (what the
        // newcomer reads to go: "the shelter" is no place it can find).
        let place = def
            .markers
            .iter()
            .filter(|m| m.kind == MarkerKind::Poi && !m.name.is_empty())
            .min_by(|a, b| dist(a.at, p.at).total_cmp(&dist(b.at, p.at)))?
            .name
            .clone();
        Some(Job::Bring {
            map: map.to_owned(),
            place,
            item: "holm_watch".into(),
            at: p.to_world(room.at),
        })
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

/// The errand, in the valley the job is in: a job pencilled in another
/// valley isn't gone after here (a Marrow job's place, walked to inside
/// Cinder, stood the newcomer in a corner of the works).
pub(super) fn errand_here(job: &Job, map: &str) -> Option<Errand> {
    job.map().is_none_or(|m| m == map).then(|| errand(job))
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
    /// Creeping away from the walker hunting it (`creep_away`), till then.
    creep: Option<(Entity, f32)>,
    /// The heavy that raised its arm at it, while it's still after it.
    heavy: Option<Entity>,
    /// The hound that coiled at it, while it's still close and has it.
    hound: Option<Entity>,
    /// Walking off from it till then (each walk away's length).
    walking_off_until: f32,
    /// When it first raised its arm at it.
    heavy_since: f32,
    /// Took what the driver put on the ledge, woken in the truck.
    took_ledge: bool,
    /// The radios it went for off its body (`radio_to_fetch`), once a go.
    radios_tried: Vec<Entity>,
    /// The dressings it went for by its way (`by_the_way`), once each.
    dressings_tried: Vec<Entity>,
    /// No gun of its own at the kit: the driver's spare pistol, magazine
    /// and rounds to take, from the crate or the ledge.
    spare_gun: bool,
    /// The walk last looked at as it came up (`unvetted`): once each.
    vetted: Option<Vec2>,
    /// The walk last put through `routed_leg` as it came up: once each.
    routed: Option<Vec2>,
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
    /// When it last readied its gun with a machine on it (`threatened`).
    readied_at: f32,
    /// Time's short (the Surge close, or the truck's wait running down)
    /// till then: a roof is a breath, not a hold (`TIME_SHORT_FOR`). (Run on
    /// from a drone with time short, far from the truck, it was shot down
    /// in the open inside 40 s.)
    pressed_until: f32,
    /// When it last came out from under cover for the truck.
    hurried_at: f32,
    /// The driver's "bleeding badly ... Get to the truck": how many times
    /// it has been answered (once a raid), and the last answer's time with
    /// what the bleed gave it then (s to bleed out; None: it eases first).
    bleed_heard: usize,
    bled: Option<(f32, Option<f32>)>,
    /// The job dropped for it (still out on it when told): home unpaid,
    /// as it should be.
    bled_off: bool,
    /// The walker that last aimed at it (its eye is what to break line with).
    aimer: Option<Entity>,
    /// When it last looked whether it's still out of that walker's line.
    line_checked: f32,
    /// When a walker last aimed at it.
    aimed_at: f32,
    /// The walkers' aims at it the last [`HUNT_WINDOW`]: which, where its
    /// eye was (it was seen aiming from there), and when.
    aims: Vec<(Entity, Vec3, f32)>,
    /// Where it stood in that walker's line (as it saw it from there),
    /// each a couple of metres on from the last, and the walker's eye
    /// height: where it'll come to look.
    seen_trail: Vec<(Vec2, f32)>,
    /// When a walker last had it in its line.
    seen_at: f32,
    /// When it last put a dressing on.
    dressed_at: f32,
    /// When it last answered a crawler's crackle.
    crawled_at: f32,
    /// On its way to a spot out of a walker's line till then.
    line_run_until: f32,
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

    /// After "bleeding badly": how long from the line to `how` (in the
    /// truck, or down), against what the bleed gave it then. Once.
    fn note_bled(&mut self, now: f32, how: &str) {
        if let Some((at, left)) = self.bled.take() {
            let gave = left.map_or("it was easing before it was out".into(), |t| {
                format!("{t:.0} s to bleed out")
            });
            self.note(
                now,
                Kind::Learned,
                format!("bled: {how} {:.0} s after the line ({gave})", now - at),
            );
        }
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
    /// Where a sentry has chirped at the newcomer: its walks keep out of
    /// their sight after (`round_sentries`).
    sentries: Local<'s, Vec<Vec2>>,
    stash: Query<'w, 's, &'static Backpack, With<Stash>>,
    card: Query<'w, 's, (Entity, &'static ScreenText), With<RequestCard>>,
    /// The truck's route card ("NEXT DRIVE ...").
    route: Query<'w, 's, (Entity, &'static ScreenText), With<crate::raid::route::RouteCard>>,
    progression: Option<Res<'w, crate::progression::Progression>>,
    hatch: Res<'w, crate::raid::lev::Handovers>,
    /// What the driver's firsts said, filled (for the report: a line's
    /// template quoted "{s} things" where "Crate's empty" was said).
    spoken: Option<Res<'w, super::firsts::Spoken>>,
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
    // Whether each leaf is shut, as it's seen to be.
    door_state: Query<'w, 's, &'static crate::world::buildings::Door>,
    bags: Query<'w, 's, &'static Backpack>,
    entrances: Option<Res<'w, crate::robots::doors::Entrances>>,
    robots: Query<'w, 's, crate::robots::subjects::Candidate>,
    trucks: Query<'w, 's, &'static GlobalTransform, With<crate::raid::truck::Truck>>,
    health: Query<'w, 's, &'static crate::health::Health, With<LocalPlayer>>,
    /// Doors held shut on an underground site's power right now (a way out
    /// planned through one went at it for the rest of the raid).
    power_locked: Query<'w, 's, (), With<crate::world::underground::PowerLocked>>,
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
    mut view: RaidView,
    raid: Res<Raid>,
    mut met: MessageReader<FirstMet>,
    mut fight: Option<ResMut<Fight>>,
    mut state: Local<Newcomer>,
    crates: crate::bot::crating::CrateWorld,
    mut tells: Tells,
    minds: Query<&crate::robots::Awareness>,
    mut plans: Local<Vec<crate::sim::indoor::BuildingPlan>>,
    time: Res<Time>,
    (spatial, eyes): (
        avian3d::prelude::SpatialQuery,
        Query<(&GlobalTransform, &crate::robots::Senses)>,
    ),
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
    // Walking off from a heavy, upright: crept, the cautious newcomer made
    // 1.25 m/s to its hunt's 1.7, never got clear, and bled out.
    let off_from_heavy = state.heavy.is_some();
    let creep = (p.reads || state.low_until > now || low)
        && !fleeing
        && !off_from_heavy
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
            // The bell rung for the card's valley: the driver's off there.
            if state.leg == Leg::Done && l.contains("Long drive") {
                state.note(now, Kind::Learned, DRIVEN);
            }
        }
    }
    state.showing = showing;
    // The buildings' plans, once they're up (in from a drone, a find indoors).
    if plans.is_empty() && !view.buildings.is_empty() {
        *plans = crate::sim::indoor::plan_buildings(&view.buildings, &view.leaves);
    }
    // Which of their doors are held shut on power now (as the raider keeps
    // them, `spot_held`): no way out is planned through one.
    for plan in plans.iter_mut() {
        plan.held.clear();
        plan.held.extend(
            plan.doors
                .iter()
                .map(|d| d.entity)
                .filter(|d| view.power_locked.contains(*d)),
        );
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
        // (Not the way out's own step: to just outside one of its doors,
        // on its outside, in line with the doorway. Anywhere else by the
        // building's wall, a walk round it from inside went at the wall:
        // a waypoint stepped out of the shed's box by its door.)
        let exit_step = |d: &crate::robots::doors::Entrance| {
            let out = d.out.xz().normalize_or(Vec2::Y);
            let off = to - d.mid.xz();
            let along = off.dot(out);
            (0.8..3.0).contains(&along) && (off - out * along).length() < 0.8
        };
        // (Not below, in a site: its way out is the director's, by the
        // site's plan; taken for a building's here, the walk out of the
        // shelter was planned again every frame and never walked.)
        // (Nor a walk straight out through an opening its plan has, a bay
        // without a leaf: creeping out of the rail shed's bay, away from a
        // walker, the wanderer was turned round to its one door, on the
        // walker's side, and found there.)
        let straight_out = || {
            let feet = root.translation + Vec3::Y;
            plans
                .iter()
                .find(|pl| pl.contains(feet, 0.3))
                .is_some_and(|pl| {
                    let a = pl.to_local(feet).xz();
                    let b = pl.to_local(world.zone.on_ground(to.x, to.y) + Vec3::Y).xz();
                    pl.path(a, b)
                        .is_some_and(|w| path_len(a, &w) <= a.distance(b) + STRAIGHT_OUT)
                })
        };
        if let Some(b) = inside(here)
            && inside(to) != Some(b)
            && !en.doors.iter().any(|d| d.building == b && exit_step(d))
            && below_in(&world.zone, &plans, root.translation).is_none()
            && !straight_out()
        {
            let (out, _) = way_out_at(&world, &view, &plans, root.translation);
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
    // Under a roof from a drone: the wait held while any drone still has it
    // (a flat 20 s, and it came out to be found again, every 25 s for ten
    // minutes, till the truck had gone), for two minutes at most.
    if state.roof_try.is_some_and(|(_, _, t)| now - t < ROOF_HOLD)
        && state.pressed_until <= now
        && matches!(bot.tasks.front(), Some(Task::Wait(_)))
        && view.entrances.as_deref().is_some_and(|en| {
            en.building_at(world.zone.on_ground(here.x, here.y) + Vec3::Y)
                .is_some()
        })
        && world
            .robots
            .iter()
            .any(|(e, r, _)| r.kind == crate::robots::RobotKind::Drone && !r.wrecked && on_me(e))
    {
        bot.elapsed = bot.elapsed.min(1.0);
    }
    let mut again: Vec<First> = vec![];
    if tells.locks.read().any(|ev| on_me(ev.drone)) {
        again.push(First::DroneLock);
    }
    if let Some(ev) = tells.aims.read().filter(|ev| on_me(ev.walker)).last() {
        state.aimer = Some(ev.walker);
        state.aimed_at = now;
        again.push(First::Aimed);
        // (Where it aimed from: it was in sight, aiming.)
        if let Some(eye) = eye_of(&eyes, ev.walker) {
            state.aims.retain(|(_, _, t)| now - t < HUNT_WINDOW);
            if state
                .aims
                .iter()
                .all(|(w, _, t)| *w != ev.walker || now - t >= HUNT_APART)
            {
                state.aims.push((ev.walker, eye, now));
            }
        }
    }
    // Where it was in the aiming walker's sight (it could see the
    // walker from there): where the walker will come to look.
    // (From the first look, not the aim: the walker that found the
    // wanderer searched from where it first saw it, before it aimed.)
    if let Some(eye) = the_walker_of(&world, &eyes, state.aimer, here, &on_me, &|e| {
        by_eye(&view, e)
    })
    .and_then(|w| eye_of(&eyes, w))
    .filter(|e| e.xz().distance(here) < SEEN_REACH)
    {
        let head = if state.low_until > now {
            LOW_HEAD
        } else {
            STAND_HEAD
        };
        if !hidden_at(&spatial, &world, here, eye, head) {
            let up = eye.y - world.zone.on_ground(eye.x, eye.z).y;
            state.seen_at = now;
            if state
                .seen_trail
                .last()
                .is_none_or(|(p, _)| p.distance(here) > SEEN_STEP)
            {
                if state.seen_trail.len() >= SEEN_KEPT {
                    state.seen_trail.remove(0);
                }
                state.seen_trail.push((here, up));
            }
        }
    }
    if now - state.seen_at >= SEEN_LAST_FOR {
        state.seen_trail.clear();
    }
    let mut chirped = false;
    for ev in tells.sentries.read() {
        if ev.what == crate::robots::sentry::SentryAct::Chirp && ev.at.xz().distance(here) < 60.0 {
            chirped = true;
            let at = ev.at.xz();
            if !view.sentries.iter().any(|s| s.distance(at) < 5.0) {
                view.sentries.push(at);
            }
        }
    }
    if chirped {
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
    let mut coiled = false;
    for ev in tells.hounds.read() {
        if on_me(ev.hound) {
            coiled = true;
            state.hound = Some(ev.hound);
        }
    }
    // A hound after it: low and still, from its coil, and held as long as
    // it's close and has it (a hound loses someone low and still; up and
    // back to the job after 29 s, the newcomer was bitten down with the
    // hound still by it). Up and on once it's moved off or lost it.
    if let Some(h) = state.hound
        && !p.reckless
        && !p.hoards
    {
        match world.robot_at(h).map(|g| g.xz()) {
            Some(from) if on_me(h) && from.distance(here) < 15.0 => {
                state.low_until = state.low_until.max(now + 1.0);
                if !matches!(bot.tasks.front(), Some(Task::Wait(_))) {
                    super::second_hour::cut_in(&mut bot, vec![Task::Wait(2.0)]);
                }
            }
            _ => {
                state.hound = None;
                state.note(
                    now,
                    Kind::Learned,
                    "kept low and still till the hound moved off",
                );
            }
        }
    }
    // A crawler's crackle at it: where from.
    let charge_from = tells
        .crawlers
        .read()
        .filter(|ev| on_me(ev.crawler))
        .last()
        .map(|ev| ev.at.xz());
    let charged = charge_from.is_some();
    let mut raised = false;
    for ev in tells.heavies.read() {
        if ev.what == crate::robots::heavy::HeavyAct::Raise && on_me(ev.heavy) {
            raised = true;
            // (From its latest raise: the same heavy back after the
            // minute and a half found it walking on, and killed it.)
            state.heavy_since = now;
            state.heavy = Some(ev.heavy);
        }
    }
    // A heavy still after it: walking on, as long as it is ("walk away and
    // keep going": 15 m bought twelve seconds, and the heavy, hunting on
    // at 1.3 to a walk's 2.4, caught it back at the job). Done when it's
    // lost it, or fallen far behind.
    if let Some(h) = state.heavy
        && !p.reckless
        && !p.hoards
    {
        let at = world.robot_at(h).map(|g| g.xz());
        match at {
            // (For a minute and a half at most: off from a heavy hunting
            // 300 s, the newcomer walked to and fro and never got home.)
            Some(from)
                if on_me(h) && from.distance(here) < 35.0 && now - state.heavy_since < 90.0 =>
            {
                // (On again before the last leg's done: a steady walk, not
                // legs and a stand, which netted 1.9 m/s to its 1.7.)
                if now > state.walking_off_until {
                    state.walking_off_until = now + 6.0;
                    super::second_hour::cut_in(&mut bot, walk_off_heavy(&world, here, from));
                }
            }
            _ => {
                state.heavy = None;
                state.note(now, Kind::Learned, "walked off from the heavy, then on");
            }
        }
    }
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
    // Heard and shot at in the same breath (a sentry's chirp, its report,
    // and "Something heard you" in one frame): going still is for a
    // machine that's only listening. Stood still in a sentry's sweep for
    // the 15 s, the newcomer took six rounds and went down; the cover the
    // other answer runs for comes first.
    let in_its_sights = state.shots.iter().any(|t| now - t < 5.0)
        // (A crawler crackling at it as much as a sentry's sweep: stood
        // still for "Something heard you", five newcomers took a zap every
        // 2 s in a shelter's dorm, Marrow job 2.)
        || charged
        || firsts.iter().chain(&again).any(|f| {
            matches!(
                f,
                First::Sentry | First::DroneLock | First::Aimed | First::Heavy | First::Hound
            )
        });
    // A gun to hand (in a hand, or on the body), for the reckless to shoot
    // back with.
    let armed_now = world.items.iter().any(|(_, it, _, held, sock, _)| {
        world.catalog.get(it.kind()).category == extraction_core::items::Category::Firearm
            && (held.is_some_and(|h| h.player == player)
                || sock.is_some_and(|s| world.owners.get(s.socket).is_ok_and(|o| o.0 == player)))
    });
    // A machine on it: noticed, aimed at, locked on, its sweep, shot at.
    let threatened = in_its_sights || noticed_now || fired_on;
    // A crawler charging at it: armed, it shoots it (it goes down to a few
    // rounds); with nothing to shoot, it backs out the way it came, out of
    // its line. (No answer at all, the newcomers stood in its line, or
    // pressed on into it, and were zapped down.)
    if let Some(from) = charge_from
        && now - state.crawled_at > CRAWLER_EVERY
        && world.alive(player)
    {
        state.crawled_at = now;
        // (Every armed one: backing out of a shelter's dorm at a walk, the
        // careful newcomers made a metre between zaps and went down.)
        let packed = world
            .packed_guns
            .iter()
            .find(|(_, g, by)| by.0 == player && g.firearm.live_rounds() > 0)
            .map(|(e, ..)| e);
        // (Armed is a round to shoot: with the slide back and no loaded
        // spare, "shot it" was an Engage that ended at once, and the
        // newcomer stood where it was through eight zaps.)
        let rounds = guns()
            .any(|(g, by, packed)| by == player && !packed && g.firearm.live_rounds() > 0)
            || spare_loaded(&world, &view, player);
        // (Stillness is no hiding from a machine zapping it: a wait to keep
        // still, under way, goes.)
        bot.tasks
            .retain(|t| !matches!(t, Task::Wait(w) if *w >= CRAWLER_STILL));
        let how = crawler_answer(armed_now, rounds, packed.is_some());
        let answer = if how == CrawlerAnswer::Shoot {
            state.fought_at = now;
            state.note(now, Kind::Learned, "a crawler crackling at it: shot it");
            take_it_on()
        } else if let (CrawlerAnswer::Unpack, Some(gun)) = (how, packed) {
            // Its only gun in the pack (put there with the hands full after
            // a fight): out, and at it. (Backing out instead, the reckless
            // one was zapped down with a loaded gun on its back.)
            state.fought_at = now;
            state.note(
                now,
                Kind::Learned,
                "a crawler crackling at it: its gun out of the pack, and shot it",
            );
            let mut t = unpack_gun(gun);
            t.extend(take_it_on());
            t
        } else {
            state.note(now, Kind::Learned, "a crawler crackling at it: backed out");
            let (out, _) = way_out_at(&world, &view, &plans, root.translation);
            if out.is_empty() {
                walk_away(&world, here, from)
            } else {
                out
            }
        };
        if let Some(f) = fight.as_mut() {
            f.flee_until = 0.0;
        }
        super::second_hour::cut_in(&mut bot, answer);
    }
    let moments = firsts
        .into_iter()
        .map(|f| (f, true))
        .chain(again.into_iter().map(|f| (f, false)));
    for (f, first) in moments {
        if first {
            // As said, filled ("Crate's as you left it: 4 things."), where
            // the speaker has it yet.
            let said = view
                .spoken
                .as_ref()
                .and_then(|s| s.2.iter().rev().find(|(t, _)| *t == f.lines()[0]))
                .map_or(f.lines()[0].to_string(), |(_, l)| l.clone());
            state.note(now, Kind::Learned, format!("{f:?}: {said}"));
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
            First::Heard if !p.reckless && !in_its_sights => vec![Task::Wait(15.0)],
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
            // (The hoarder too: walking on through its chirp it was shot
            // down. Not yet the reckless one: sent for the nearest roof by
            // the way round, it crossed the sweep and went down where
            // walking on got it home, first_fight's sentry; its own answer
            // is to come.)
            First::Sentry if !p.reckless => {
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
            // Reckless, armed, with a drone locked on or a walker's gun on
            // it: shoots back, there and then (it waited for its next "a
            // machine in range" look, half a minute and more, and a drone
            // has you down in thirty seconds: twelve raids running, not a
            // round fired).
            First::DroneLock | First::Aimed if p.reckless && armed_now => {
                state.fought_at = now;
                state.note(now, Kind::Learned, "shot back at it");
                take_it_on()
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
                // home). Its bed is the roof nearest: the truck waiting
                // within a short run, on into it (at 21 m from the truck
                // back at the gate, the one-handed newcomer went under the
                // gate's shelter instead, waited out in the open beside it,
                // and two drones shot it down 15 m behind the bed).
                let going_home = (matches!(state.leg, Leg::Board)
                    && matches!(raid.phase, Phase::TruckWaiting { .. })
                    && world
                        .trucks
                        .iter()
                        .any(|t| t.translation().xz().distance(here) < BED_RUN))
                    // Pinned, and the truck there for it: no roof keeps
                    // that drone off, and the truck won't wait (back under
                    // its roof at each lock on the way out to it, the
                    // wanderer watched the truck come and go, seed 20).
                    || (state.pinned
                        && matches!(state.leg, Leg::Rendezvous | Leg::Board)
                        && matches!(raid.phase, Phase::TruckWaiting { .. }));
                // Pinned: a drone that keeps finding it however often it
                // goes under a roof (re-locked every 23 s for 20 minutes at
                // the post office, worn down and killed, three raids). A
                // person gives the job up: home from here, after this one
                // in cover.
                if overhead {
                    if found_again(&mut state.relocks, now) >= PINNED_AFTER
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
                        // Not on with the walk it had after: out from under
                        // the roof that way, it was found again at each
                        // try, pinned or not, till shot down (seed 20).
                        bot.tasks.clear();
                        bot.tasks.push_back(Task::Wait(IDLE));
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
                // "Break its line": out of its eye's sight, the nearest way
                // (low where it stands, round the corner, behind the wall):
                // run for a building's far side, the careful newcomer stood
                // in its line on the way and took its rounds, raid after
                // raid.
                let line = (!overhead)
                    .then(|| {
                        let eye = walker_eye(&world, &eyes, state.aimer, here, &on_me)?;
                        break_line(&spatial, &world, here, eye)
                    })
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
                    None => match line {
                        Some((mut tasks, how)) => {
                            // Hunted (the same walker on it again inside
                            // 20 s): out of its line, then on away, low,
                            // out of its hearing and its views. (Breaking
                            // its line on the spot each time it came round,
                            // the wanderer was found again inside 40 s, and
                            // shot.)
                            let end = tasks
                                .iter()
                                .rev()
                                .find_map(|t| match t {
                                    Task::WalkTo { to, .. } => Some(*to),
                                    _ => None,
                                })
                                .unwrap_or(here);
                            let crept = hunted(&state.aims, now)
                                .filter(|_| under_walls(&world, &plans, root.translation + Vec3::Y))
                                .and_then(|eye| {
                                    let mut eyes_on: Vec<Vec3> = state
                                        .seen_trail
                                        .iter()
                                        .map(|(at, up)| {
                                            world.zone.on_ground(at.x, at.y) + Vec3::Y * *up
                                        })
                                        .collect();
                                    eyes_on.push(eye);
                                    creep_away(&spatial, &world, &plans, end, eye.xz(), &eyes_on)
                                });
                            if matches!(crept, Some(_))
                                && matches!(tasks.last(), Some(Task::Wait(_)))
                            {
                                tasks.pop();
                            }
                            let run = run_to_line(&tasks, here);
                            state.line_run_until = now + run;
                            if let Some(f) = fight.as_mut() {
                                f.flee_until = now + run;
                            }
                            match crept {
                                Some(creep) => {
                                    let far = walked_len(&creep, end);
                                    state.note(
                                        now,
                                        Kind::Learned,
                                        format!(
                                            "hunted: broke its line ({how}), then crept away out of its view and hearing, {far:.0} m"
                                        ),
                                    );
                                    state.low_until = now + run + far / CREEP_PACE + 31.0;
                                    state.creep = hunter(&state.aims, now)
                                        .map(|w| (w, now + run + far / CREEP_PACE));
                                    tasks.extend(creep);
                                    tasks.push(Task::Wait(29.0));
                                }
                                None => {
                                    state.note(
                                        now,
                                        Kind::Learned,
                                        format!("broke its line: {how}"),
                                    );
                                    state.low_until = now + run + LINE_HELD;
                                }
                            }
                            tasks
                        }
                        None => run_for_cover(&world.zone, here, from, 0.0),
                    },
                }
            }
            // "Plant your feet and shoot. A hound: don't run": stopped where
            // it is, at it, for those who'd take a machine on. The rest stay
            // low and still in its sight (`state.hound`, the answer that got
            // them home: stood shooting, the lost one held the hound by it
            // and never got up again).
            First::Hound if armed_now && (p.reckless || p.hoards) => {
                if let Some(f) = fight.as_mut() {
                    f.flee_until = 0.0;
                }
                state.fought_at = now;
                state.note(
                    now,
                    Kind::Learned,
                    "a hound: stood and shot, as the driver said",
                );
                take_it_on()
            }
            // Nothing to shoot with: feet planted, still.
            First::Hound if p.reckless || p.hoards => {
                if let Some(f) = fight.as_mut() {
                    f.flee_until = 0.0;
                }
                vec![Task::Wait(8.0)]
            }
            // "Big one, and slow. Walk away from it": off, upright, at a
            // walk (a heavy sees a still shape as well as a moving one;
            // its hunt is 1.3 m/s to a walk's 2.4).
            First::Heavy if !p.reckless && !p.hoards => {
                let from = world.nearest_robot(here).unwrap_or(here + Vec2::Y * 10.0);
                walk_off_heavy(&world, here, from)
            }
            // "Low behind it, and keep still": a still shape behind cover
            // isn't glimpsed; a moving one is, and found.
            // (Not with a crawler charging at it: still, the careful
            // newcomer stood in a shelter's generator room through eight
            // zaps, told "keep still" by a hunter's first.)
            First::ComingToLook
                if !p.reckless && !p.hoards && !crawler_at_it(now, state.crawled_at) =>
            {
                if let Some(f) = fight.as_mut() {
                    f.flee_until = 0.0;
                }
                // Behind it from where it's coming to look, not only from
                // where it was: crouched where it had run to, the wanderer
                // was in plain view from the walker's search 18 m off, and
                // was found, shot and bled out.
                // What it knows: where the walker saw it (it comes to look
                // there: the first sighting as much as the last; held out
                // of the last, the wanderer was found from the first), and
                // the walker's eye where it can see it.
                let walker = the_walker_of(&world, &eyes, state.aimer, here, &on_me, &|e| {
                    by_eye(&view, e)
                });
                let mut looks: Vec<Vec3> = state
                    .seen_trail
                    .iter()
                    .rev()
                    .map(|(at, up)| world.zone.on_ground(at.x, at.y) + Vec3::Y * *up)
                    .collect();
                if let Some(eye) = walker.and_then(|w| eye_of(&eyes, w))
                    && !hidden_at(&spatial, &world, here, eye, STAND_HEAD)
                {
                    looks.push(eye);
                }
                let seen = looks
                    .iter()
                    .any(|e| !line_hidden(&spatial, &world, here, *e));
                // (Under the bot's 30 s give-up, or a frame past it is a
                // failure.)
                let hold = Task::Wait(29.0);
                // Hunted (the same walker on it again inside 20 s): away
                // from it, low, out of earshot and every view it has, and
                // still there. (Held out of view near where it searched,
                // the wanderer was walked back toward its search to the
                // rail shed's far side, found there again and shot.)
                let crept = hunted(&state.aims, now)
                    .filter(|_| under_walls(&world, &plans, root.translation + Vec3::Y))
                    .and_then(|from| {
                        let mut eyes_on = looks.clone();
                        eyes_on.push(from);
                        creep_away(&spatial, &world, &plans, here, from.xz(), &eyes_on)
                    });
                if let Some(mut tasks) = crept {
                    let run = walked_len(&tasks, here);
                    state.note(
                        now,
                        Kind::Learned,
                        format!("hunted: crept away out of its view and hearing, {run:.0} m"),
                    );
                    tasks.push(hold);
                    state.low_until = now + run / CREEP_PACE + 31.0;
                    state.creep = hunter(&state.aims, now).map(|w| (w, now + run / CREEP_PACE));
                    state.line_run_until = 0.0;
                    tasks
                } else {
                    match seen
                        .then(|| break_line_from(&spatial, &world, here, &looks, true))
                        .flatten()
                    {
                        Some((mut tasks, how)) => {
                            state.note(
                                now,
                                Kind::Learned,
                                format!("coming to look, and in its view from there: {how}"),
                            );
                            if let Some(last) = tasks.last_mut() {
                                *last = hold;
                            }
                            let run = run_to_line(&tasks, here);
                            state.low_until = now + run + 30.0;
                            tasks
                        }
                        None => {
                            state.note(
                                now,
                                Kind::Learned,
                                format!(
                                    "coming to look: {} from the {} spots it saw me at, held",
                                    if seen {
                                        "no cover within 30 m out of its view"
                                    } else {
                                        "out of its view"
                                    },
                                    looks.len(),
                                ),
                            );
                            state.low_until = now + 30.0;
                            vec![hold]
                        }
                    }
                }
            }
            // Bleeding with a heavy still in reach: walking on first, the
            // dressing once it's behind (bound beside it, the one-handed
            // newcomer took its second blow).
            First::Bleeding
                if state.heavy.is_some_and(|h| {
                    world
                        .robot_at(h)
                        .is_some_and(|g| g.xz().distance(here) < HEAVY_DRESS_CLEAR)
                }) =>
            {
                let from = state
                    .heavy
                    .and_then(|h| world.robot_at(h))
                    .map_or(here + Vec2::Y * 10.0, |g| g.xz());
                state.note(
                    now,
                    Kind::Learned,
                    "bleeding, with a heavy in reach: on away first, then the dressing",
                );
                walk_off_heavy(&world, here, from)
            }
            // Bleeding with a crawler charging at it: out of its line first
            // (its answer's doing that), the dressing once it's behind.
            First::Bleeding if crawler_at_it(now, state.crawled_at) => {
                state.note(
                    now,
                    Kind::Learned,
                    "bleeding, with a crawler at it: out of its line first, then the dressing",
                );
                vec![]
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
                        state.dressed_at = now;
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
                    Some((e, ..)) => {
                        state.dressed_at = now;
                        dress(e)
                    }
                    None => {
                        state.note(now, Kind::Confused, "bleeding, and no dressing on me");
                        vec![]
                    }
                }
            }
            First::Anomaly => {
                // Back the way it came, then on round.
                let away = state
                    .goal
                    .as_ref()
                    .map_or(Vec2::ZERO, |(_, g)| -(*g - here).normalize_or_zero());
                // Indoors, or down a site, by its plan: a step back it can
                // take (in the shelter's generator room the step back went
                // into the wall, and stood there the task's 30 s).
                let at = root.translation + Vec3::Y * 1.0;
                match plans.iter().find(|pl| pl.contains(at, 0.3)) {
                    Some(plan) => step_back_in(plan, here, away),
                    None => vec![Task::WalkTo {
                        to: here + away * 4.0,
                        within: 1.0,
                    }],
                }
            }
            First::Surge => {
                state.leg = Leg::Call;
                vec![]
            }
            _ => vec![],
        };
        if !answer.is_empty() {
            // Where the answer leaves it: the way on from there, round the
            // sentry's sight, not back across it.
            let n = answer.len();
            let left_at = answer
                .iter()
                .rev()
                .find_map(|t| match t {
                    Task::WalkTo { to, .. } => Some(*to),
                    _ => None,
                })
                .unwrap_or(here);
            let moved = answer.iter().any(|t| matches!(t, Task::WalkTo { .. }));
            super::second_hour::cut_in(&mut bot, answer);
            // Any answer that walked it off its way (out of a sentry's
            // sight, out of a walker's line, off quiet): the rest of the
            // walk from where it's left, not the legs it had from where it
            // was (out of the walker's line, the one-handed newcomer went
            // on with its old legs into the Old farm's log pile, and stood
            // there two walks' 30 s).
            if f == First::Sentry || moved {
                reroute_after(&mut bot, n, left_at, &world, &view);
            }
        }
    }
    // Armed, with a machine on it, and nothing up the spout: it readies the
    // gun, a loaded magazine in from those on it (Ready). (Three of the
    // careful newcomer's losses: dead with an empty magazine in the gun and
    // a loaded one on it, never a round fired.) The hoarder carries on as
    // it does.
    let loadable = guns().any(|(g, by, packed)| {
        by == player && !packed && g.firearm.magazine.as_ref().is_some_and(|m| m.rounds > 0)
    }) || spare_loaded(&world, &view, player);
    if threatened
        && armed_now
        && !p.hoards
        && loadable
        && !one_up(guns(), player)
        && now - state.readied_at > 10.0
    {
        state.readied_at = now;
        state.note(now, Kind::Learned, "a machine on it: readied the gun");
        super::second_hour::cut_in(
            &mut bot,
            vec![Task::DrawGun, Task::Ready { off_safe: false }],
        );
    }
    // Bleeding, a dressing on it, nothing at it this moment: the dressing
    // on, whoever said so and however long ago. (Told once a career, the
    // careful newcomer shot a walker blind at r10 and bled to death beside
    // it over twenty seconds, a dressing in its pouch.)
    if wound_to_dress(
        world.bleeding(player),
        &state.shots,
        now,
        state.dressed_at,
        state.line_run_until > now,
    ) && world.alive(player)
        // (Not twice: the first bleed's own answer, or the last, still on
        // its way; a second grab found the dressing gone.)
        && !bot.tasks.iter().any(|t| matches!(t, Task::Use(_) | Task::Grab { .. }))
        // (Not with a heavy in reach: its blows aren't shots, so "nothing
        // at it" was wrong, and the one-handed newcomer bound its wound
        // beside one and took the second blow.)
        && !state.heavy.is_some_and(|h| {
            world
                .robot_at(h)
                .is_some_and(|g| g.xz().distance(here) < HEAVY_DRESS_CLEAR)
        })
        // (Nor with a crawler charging at it: bound in its line, the careful
        // newcomer took the next zap.)
        && !crawler_at_it(now, state.crawled_at)
    {
        let dressing = world.items.iter().find(|(_, it, _, held, sock, _)| {
            held.is_none()
                && sock.is_some_and(|s| world.owners.get(s.socket).is_ok_and(|o| o.0 == player))
                && world.catalog.get(it.kind()).id == "field_dressing"
        });
        if let Some((e, ..)) = dressing {
            state.dressed_at = now;
            state.note(
                now,
                Kind::Learned,
                "bleeding, nothing at it: the dressing on",
            );
            super::second_hour::cut_in(
                &mut bot,
                vec![
                    Task::Stow(Side::Left),
                    Task::Grab {
                        item: e,
                        side: Side::Left,
                    },
                    Task::Use(Side::Left),
                ],
            );
        }
    }
    // A dressing lying by its way, none on it: picked up, as anyone would
    // (the careful and the one-handed newcomers won the first fight on
    // Marrow and bled out after it, their one dressing on the first wound).
    // Out on the job only, clear of any fight, and each once.
    if raid.phase == Phase::Out
        && world.alive(player)
        && now - state.fought_at > CLEAR_TO_PICK
        && now - state.aimed_at > CLEAR_TO_PICK
        && state.shots.iter().all(|t| now - t > CLEAR_TO_PICK)
        && state.line_run_until <= now
        && state.heavy.is_none()
        && state.hound.is_none()
        && !bot.tasks.iter().any(|t| {
            matches!(
                t,
                Task::Use(_) | Task::Grab { .. } | Task::GrabFromHere { .. }
            )
        })
        && carried(&world, &view, player, "field_dressing").is_empty()
    {
        let loose = world
            .items
            .iter()
            .filter(|(e, it, _, held, sock, bag)| {
                held.is_none()
                    && sock.is_none()
                    && bag.is_none()
                    && world.catalog.get(it.kind()).id == "field_dressing"
                    && !view.stash.iter().any(|b| b.contents.contains(e))
            })
            .map(|(e, _, gt, ..)| (e, gt.translation()));
        // (Not up a tower's stairs: a thing it passes, not a climb.)
        if let Some((e, at)) = by_the_way(here, loose, &state.dressings_tried, |at| {
            in_reach(&world, &plans, at) && way_down(&world, &plans, at).is_empty()
        }) {
            state.dressings_tried.push(e);
            state.note(now, Kind::Learned, "a dressing by the way, picked up");
            let mut t = way_to(&world, &view, &plans, here, at);
            t.extend([
                Task::Grab {
                    item: e,
                    side: Side::Right,
                },
                Task::Stow(Side::Right),
            ]);
            super::second_hour::cut_in(&mut bot, t);
        }
    }
    // In a walker's line with no way out of it, a round up: it shoots back.
    // (Told "Break its line", the careful newcomer stood in it with a
    // loaded gun and took six rounds in eighteen seconds, its trigger never
    // pulled, six raids of eleven.)
    let eye = walker_eye(&world, &eyes, state.aimer, here, &on_me);
    let hidden = eye.is_some_and(|e| line_hidden(&spatial, &world, here, e));
    // On its way to a spot out of its line (not any run for cover: a run
    // that doesn't break its line is seven rounds in six seconds, r10).
    let running = state.line_run_until > now;
    if !p.reckless
        && eye.is_some()
        && now - state.fought_at > 8.0
        && no_way_out(
            &state.shots,
            now,
            state.aimed_at,
            hidden,
            running,
            armed_now && one_up(guns(), player),
        )
    {
        state.fought_at = now;
        state.note(
            now,
            Kind::Learned,
            "in its line, no way out of it: shot back",
        );
        super::second_hour::cut_in(&mut bot, take_it_on());
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
        // A walk that didn't get there: the rest of that walk was planned
        // from where it isn't (on the far side of the camp's fence, it
        // pressed against the wire leg after leg, sixty times). Dropped,
        // so what's next is planned from here.
        if line.contains("timed out on WalkTo") {
            while matches!(bot.tasks.front(), Some(Task::WalkTo { .. })) && bot.tasks.len() > 1 {
                bot.tasks.pop_front();
            }
            bot.restart();
        }
    }
    // A walk about to be made that `walk` didn't plan: a long straight one,
    // or one through the fence, isn't made (once each). (Walked off from
    // a heavy out through the south gate, it went on with the way it had
    // planned from inside, at the wire; and a 90 m stretch along the fence
    // timed out at a walk.)
    // As a timeout does it (above), only before the 30 s at the wire: the
    // rest of that walk dropped, and what's next planned from here. (Put
    // through `walk` to the same old waypoint instead, it went back the
    // way it had been going, past the sentry, and round again.)
    if let Some(&Task::WalkTo { to, .. }) = bot.tasks.front()
        && state.vetted != Some(to)
        && {
            // (Looked at once, as it comes up.)
            state.vetted = Some(to);
            unvetted(&world, here, to)
        }
    {
        while matches!(bot.tasks.front(), Some(Task::WalkTo { .. })) && bot.tasks.len() > 1 {
            bot.tasks.pop_front();
        }
        bot.restart();
    }
    // Every walk, as it comes up, from where the newcomer is (not where it
    // was planned from): straight through something `walk` goes round, it's
    // `walk`'s way to the same place instead. (The wanderer, handed a leg
    // through the rail shed's south wall, slid along it 40 s till the walk
    // timed out: wherever a leg comes from, it's made here.)
    if let Some(&Task::WalkTo { to, within }) = bot.tasks.front()
        && state.routed != Some(to)
    {
        state.routed = Some(to);
        if let Some(way) = routed_leg(&world, root.translation, to, within) {
            bot.tasks.pop_front();
            for t in way.into_iter().rev() {
                bot.tasks.push_front(t);
            }
            bot.restart();
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
        // And stops: it walked on with its errand, fell out there, woke in
        // the truck and walked straight out of it again with nothing (the
        // raid saved out there, no radio: Progression's careers wedged).
        bot.tasks.clear();
        bot.tasks.push_back(Task::Wait(IDLE));
        bot.restart();
    }
    // Woken from the last one's fall (a career resumed while down): not
    // this raid's death; wait for the truck.
    if matches!(raid.phase, Phase::Down { .. }) && state.leg == Leg::Camp {
        return;
    }
    if matches!(raid.phase, Phase::Down { .. }) {
        if state.leg != Leg::Done {
            state.note(now, Kind::Killed, "went down");
            state.note_bled(now, "down");
            state.leg = Leg::Done;
        }
        // That's this first hour over: nothing left of the walk (down
        // after its half hour, already Done, it kept the walk it had, and
        // woke in the truck to walk out of it).
        if !matches!(bot.tasks.front(), Some(Task::Wait(w)) if *w == IDLE) || bot.tasks.len() > 1 {
            bot.tasks.clear();
            bot.tasks.push_back(Task::Wait(IDLE));
            bot.restart();
        }
    }

    // Creeping away from the walker hunting it, and it turns its eyes this
    // way with nothing between: down behind what's near, and still. (On
    // across its view, the lost newcomer was glimpsed by it coming to
    // look, followed, found and shot; a person would freeze.)
    if let Some((w, until)) = state.creep
        && now - state.line_checked > CREEP_CHECK
    {
        state.line_checked = now;
        let creeping = until > now && matches!(bot.tasks.front(), Some(Task::WalkTo { .. }));
        match eyes.get(w) {
            Ok((gt, senses)) if creeping => {
                if faces_me(&spatial, &world, gt, senses, here)
                    && let Some((tasks, how)) =
                        break_line(&spatial, &world, here, gt.transform_point(senses.eye))
                {
                    state.creep = None;
                    state.note(
                        now,
                        Kind::Learned,
                        format!("creeping, it turned this way: {how}"),
                    );
                    let run = run_to_line(&tasks, here);
                    state.line_run_until = now + run;
                    state.low_until = now + run + LINE_HELD;
                    // (The creep's walk on is dropped: held where it went down.)
                    while matches!(bot.tasks.front(), Some(Task::WalkTo { .. })) {
                        bot.tasks.pop_front();
                    }
                    if matches!(bot.tasks.front(), Some(Task::Wait(_))) {
                        bot.tasks.pop_front();
                    }
                    let n = tasks.len();
                    let left_at = tasks
                        .iter()
                        .rev()
                        .find_map(|t| match t {
                            Task::WalkTo { to, .. } => Some(*to),
                            _ => None,
                        })
                        .unwrap_or(here);
                    super::second_hour::cut_in(&mut bot, tasks);
                    reroute_after(&mut bot, n, left_at, &world, &view);
                    return;
                }
            }
            _ => state.creep = None,
        }
    }
    // Held out of a walker's line: still out of it? (It comes round to
    // look: low behind a car, the careful newcomer was found from the side
    // five seconds on.) Seen again, out of it again; once a second.
    if state.low_until > now
        && now - state.line_checked > 1.0
        && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == LINE_HELD)
        && let Some(eye) = walker_eye(&world, &eyes, state.aimer, here, &on_me)
    {
        state.line_checked = now;
        if !line_hidden(&spatial, &world, here, eye)
            && let Some((tasks, how)) = break_line(&spatial, &world, here, eye)
        {
            state.note(now, Kind::Learned, format!("in its line again: {how}"));
            let run = run_to_line(&tasks, here);
            state.line_run_until = now + run;
            state.low_until = now + run + LINE_HELD;
            if let Some(f) = fight.as_mut() {
                f.flee_until = now + run;
            }
            bot.tasks.pop_front();
            // (And the walk on from its new cover, as for any answer that
            // moves it.)
            let n = tasks.len();
            let left_at = tasks
                .iter()
                .rev()
                .find_map(|t| match t {
                    Task::WalkTo { to, .. } => Some(*to),
                    _ => None,
                })
                .unwrap_or(here);
            super::second_hour::cut_in(&mut bot, tasks);
            reroute_after(&mut bot, n, left_at, &world, &view);
            return;
        }
    }
    // "You're bleeding badly, nothing to bind it. Get to the truck." A
    // person told that goes: the job's off, the truck's called now, from
    // where it stands (one-handed v the walker heard it at 97 s, walked on
    // to the farm and back to the gate as the card said, and was in the
    // truck at 292 s).
    let told = state
        .heard
        .iter()
        .filter(|l| *l == super::firsts::BLEEDING_BADLY_LINE)
        .count();
    if told > state.bleed_heard {
        state.bleed_heard = told;
        let left = view
            .health
            .single()
            .ok()
            .and_then(|h| bleeds_out_in(h.hp, h.bleeding));
        state.bled = Some((now, left));
        if matches!(state.leg, Leg::Out | Leg::Place)
            && bleed_answer(state.leg, &raid.phase).is_some()
        {
            state.bled_off = true;
        }
        let gave = left.map_or("it eases before it's out".into(), |t| {
            format!("out in {t:.0} s")
        });
        match bleed_answer(state.leg, &raid.phase) {
            Some(BleedGo::ToTruck) => {
                state.note(
                    now,
                    Kind::Learned,
                    format!("bleeding badly: out to the truck ({gave})"),
                );
                state.roof_try = None;
                bot.tasks.clear();
                bot.walk = None;
                bot.tasks.extend(world.go_home(here, &raid, state.tries));
                bot.tasks.push_back(Task::Wait(IDLE));
                bot.restart();
                state.leg = Leg::Board;
                return;
            }
            Some(BleedGo::Call) => {
                state.note(
                    now,
                    Kind::Learned,
                    format!("bleeding badly: called the truck from here ({gave})"),
                );
                state.leg = Leg::Call;
                // (Not back to the gate first: there's no time for it.)
                state.at_gate = true;
                state.roof_try = None;
                bot.tasks.clear();
                bot.tasks.push_back(Task::Wait(IDLE));
                bot.walk = None;
                bot.restart();
                return;
            }
            None => state.note(
                now,
                Kind::Learned,
                format!("bleeding badly: already on its way to the truck ({gave})"),
            ),
        }
    }
    // A time sense: in cover or on the job, the clock still runs. A person
    // under a roof keeps half an eye on the time, and goes when it's short:
    // the Surge close (the driver's warning is said once a career: told
    // before, the careful newcomer sat at a door 17 minutes into it, and
    // never called), or the truck there and its wait running down (held
    // under a roof by a drone that kept finding it, the truck waited five
    // minutes and went, and the Surge took it 28 minutes on).
    let hiding = state.roof_try.is_some()
        || matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s != IDLE && *s >= 10.0);
    // A drone that has it still: out from under its roof is out under its
    // gun (out to the truck with one on it, the careful newcomer was worn
    // down in the open, at each try). Between its looks, then.
    let hunted = world
        .robots
        .iter()
        .any(|(e, r, _)| r.kind == crate::robots::RobotKind::Drone && !r.wrecked && on_me(e));
    let hurry = time_short(
        &raid.phase,
        raid.surge_in(),
        state.leg,
        hiding,
        hunted,
        state.pressed_until > now,
        now - state.hurried_at,
    );
    if hurry == Some(Hurry::Surge) {
        state.note(now, Kind::Learned, "the Surge is close: home now");
        state.leg = Leg::Call;
        state.pressed_until = now + TIME_SHORT_FOR;
        state.roof_try = None;
        bot.tasks.clear();
        bot.tasks.push_back(Task::Wait(IDLE));
        bot.walk = None;
        bot.restart();
        return;
    }
    if hurry == Some(Hurry::Truck) {
        state.hurried_at = now;
        state.note(now, Kind::Learned, "the truck won't wait: out to it");
        state.pressed_until = now + TIME_SHORT_FOR;
        state.roof_try = None;
        bot.tasks.clear();
        bot.walk = None;
        bot.tasks.extend(world.go_home(here, &raid, state.tries));
        bot.tasks.push_back(Task::Wait(IDLE));
        bot.restart();
        state.leg = Leg::Board;
        return;
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
        // (Bleeding badly, it's home it wants, not a fight.)
        && state.bled.is_none()
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
            // Round to the tailgate and up into the bed: square behind the
            // steps first, then up them on their line, as home is boarded
            // (`go_home`). (From Sallow's camp, a metre off the line, it
            // went at the steps aslant and stood at their side every raid.)
            let mut way = route(&world, &view, here, at_truck(0.0, 4.0));
            if let Some(Task::WalkTo { within, .. }) = way.last_mut() {
                *within = 0.5;
            }
            tasks.extend(way);
            for (x, z) in [(0.05, 2.9), (0.05, 0.3), (-0.2, -0.6)] {
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
                .and_then(|j| match errand_here(j, &opts.map)? {
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
                .and_then(|j| errand_here(j, &opts.map));
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
            // No gun of its own anywhere (lost with the rest at a fall): the
            // driver's spare pistol, its magazine and rounds, once he's
            // looked round (off before then, every raid, it went out
            // unarmed and was shot down two minutes in: Progression's
            // careers, raids 6-20).
            let armed = world.items.iter().any(|(e, it, _, held, sock, bag)| {
                world.catalog.get(it.kind()).firearm.is_some()
                    && (held.is_some_and(|h| h.player == player)
                        || sock.is_some_and(|s| {
                            world.owners.get(s.socket).is_ok_and(|o| o.0 == player)
                        })
                        || bag
                            .is_some_and(|b| view.bags.get(b.bag).is_ok_and(|b| b.owner == player))
                        || view.stash.iter().any(|b| b.contents.contains(&e)))
            });
            // (Kept till it's all taken: once the spare's in the crate, it's
            // "armed" by the crate, and its rounds still to take.)
            state.spare_gun |= !armed;
            if takes && state.spare_gun && opts.map != crate::camp::TRAINING_YARD {
                need.extend(["pm_pistol", "pm_mag", "ammo_9x18"].map(String::from));
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
            // (Taken already, a moment ago.)
            need.retain(|id| carried(&world, &view, player, id).is_empty());
            for id in need {
                // On the ledge, or in the kit crate (the driver's spares go
                // there), and not already being taken from it.
                let taking = |e: Entity| {
                    tasks
                        .iter()
                        .any(|t| matches!(t, Task::Grab { item, .. } if *item == e))
                };
                let on_ledge = world.items.iter().find(|(e, it, gt, held, sock, bag)| {
                    let crated = view.stash.iter().any(|b| b.contents.contains(e));
                    held.is_none()
                        && sock.is_none()
                        && world.catalog.get(it.kind()).id == id
                        && !taking(*e)
                        && (crated
                            || (bag.is_none()
                                && gt.translation().xz().distance(at_truck(0.0, 0.0)) < 4.0))
                });
                match on_ledge {
                    Some((e, ..)) => {
                        if state
                            .heard
                            .iter()
                            .any(|l| l.contains("off the ledge") || l.contains("in the crate"))
                        {
                            state.note(now, Kind::Learned, format!("{id}, as the driver said"));
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
            state.spare_gun = false;
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
                None if !view.route.is_empty() && raid.phase == Phase::AtBase => {
                    // Maybe another valley's: the route card on the wall
                    // says where the truck goes next.
                    state.tries = 0;
                    state.leg = Leg::Route;
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
        Leg::Route => {
            let route = view.route.iter().next().map(|(e, c)| (e, c.text.clone()));
            let bell = world
                .pressables
                .iter()
                .find(|(_, p)| **p == crate::raid::truck::Pressable::BellPull)
                .map(|(e, _)| e);
            match (route, bell) {
                (Some((_, shown)), Some(bell))
                    if let Some(to) = card_drives_to(&state.card, &shown) =>
                {
                    state.note(
                        now,
                        Kind::Learned,
                        format!("the card's job is in {to}: turned the route card to it, and rang the bell"),
                    );
                    tasks.extend([Task::Press(bell), Task::Wait(3.0)]);
                    state.leg = Leg::Done;
                }
                (Some((card, _)), Some(_)) if state.tries < 6 => {
                    tasks.extend([Task::Press(card), Task::Wait(1.0)]);
                    state.tries += 1;
                }
                _ => {
                    let card = state.card.clone();
                    state.note(
                        now,
                        Kind::Confused,
                        format!("no place I can find on the card: {card:?}"),
                    );
                    state.tries = 0;
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
                    tasks.extend(way_out_at(&world, &view, &plans, root.translation).0);
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
                // Short of any building that lands in (walked at, the lost
                // newcomer pinned itself to Kettle's chapel wall ten
                // minutes a raid).
                let wrong = outside(&world, &view, from, from + side * 70.0);
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
                    tasks.extend(way_out_at(&world, &view, &plans, root.translation).0);
                    state.leg = Leg::Call;
                    state.tries = 0;
                }
                Err(why) => {
                    state.note(now, Kind::Stuck, why);
                    tasks.extend(way_out_at(&world, &view, &plans, root.translation).0);
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
            } else if !close && state.tries >= 3 {
                // Not there, three walks on: caught on the way (in a
                // building on the line: the lost newcomer stood in Kettle's
                // chapel ten minutes, 66 m short of the cottage, and called
                // it there). Out of the building it's in by its plan, and on
                // again; at last, said so.
                state.tries += 1;
                let inside = view.entrances.as_deref().is_some_and(|en| {
                    en.building_at(world.zone.on_ground(here.x, here.y) + Vec3::Y)
                        .is_some()
                });
                if state.tries > 6 {
                    state.note(now, Kind::Stuck, "couldn't get to the place");
                    state.leg = Leg::Call;
                    state.tries = 0;
                } else if let Some((_, g)) = state.goal.clone() {
                    if inside {
                        tasks.extend(way_out_at(&world, &view, &plans, root.translation).0);
                    }
                    tasks.extend(route(&world, &view, here, g));
                }
            } else if close {
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
                // From the bed that's down over the tailgate, not through
                // the canvas (Kettle, an out-of-valley card: it walked at
                // the bed's side from 50 s on and never got off).
                let from = if in_truck_bed(truck.as_ref(), here) {
                    for (x, z) in [(0.05, 1.2), (0.0, 4.0)] {
                        tasks.push(Task::WalkTo {
                            to: at_truck(x, z),
                            within: 0.5,
                        });
                    }
                    at_truck(0.0, 4.0)
                } else {
                    here
                };
                let away = from + (from - at_truck(0.0, 0.0)).normalize_or(Vec2::Y) * 20.0;
                tasks.extend(route(&world, &view, from, away));
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
                    // Near it, to wait: not on the spot itself. Drones
                    // circling a place in sight of it, and the truck still
                    // on its way: under the roof or in the lee the
                    // rendezvous has for that. Already there: to it.
                    let coming = matches!(raid.phase, Phase::TruckCalled { .. });
                    let near = coming
                        .then(|| wait_point(&world.zone.map, rv))
                        .flatten()
                        .unwrap_or_else(|| rv + (here - rv).normalize_or(Vec2::Y) * 8.0);
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
                    // ...or at its feet: a raid picked up again sets down
                    // what was in the hands (`persist::apply_restore`).
                    // (Looked for only on the body, the careers wedged: "no
                    // radio", done, every run after it the same.)
                    (None, None) => match radio_at_feet(&world, &plans, root.translation)
                        .filter(|(e, _)| tries_at(&state.radios_tried, *e) < RADIO_TRIES)
                        .filter(|_| state.radios_tried.len() < RADIO_FETCHES)
                    {
                        Some((e, at)) => {
                            // (Counted: one out of reach overhead was picked
                            // up again every 3 s till a machine came.)
                            state.radios_tried.push(e);
                            state.note(now, Kind::Learned, "the radio at its feet, picked up");
                            tasks.extend([
                                Task::Stow(Side::Left),
                                Task::WalkTo {
                                    to: at,
                                    within: 0.5,
                                },
                                Task::Grab {
                                    item: e,
                                    side: Side::Left,
                                },
                            ]);
                        }
                        // ...or where one lies: in remains (its own first),
                        // or loose. The radio is the only way to call the
                        // truck, so a newcomer with none goes for one
                        // (resumed with its radio in remains 130 m off, the
                        // careers wedged: "no radio", done, every run after).
                        None => {
                            match radio_to_fetch(&world, &plans, player, here, &state.radios_tried)
                                .filter(|_| state.radios_tried.len() < RADIO_FETCHES)
                            {
                                Some((e, at)) => {
                                    state.radios_tried.push(e);
                                    state.note(
                                        now,
                                        Kind::Learned,
                                        format!(
                                            "no radio on it: went for the one {:.0} m off",
                                            at.xz().distance(here)
                                        ),
                                    );
                                    // By the way in where it lies indoors (walked
                                    // at straight, it stood 12 m off, at the
                                    // wall between, till the grab timed out).
                                    tasks.push(Task::Stow(Side::Left));
                                    tasks.extend(way_to(&world, &view, &plans, here, at));
                                    tasks.push(Task::Grab {
                                        item: e,
                                        side: Side::Left,
                                    });
                                    tasks.extend(way_down(&world, &plans, at));
                                }
                                None => {
                                    state.note(
                                        now,
                                        Kind::Confused,
                                        "no radio to call the truck with",
                                    );
                                    state.leg = Leg::Done;
                                }
                            }
                        }
                    },
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
                state.note_bled(now, "in the truck");
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
            } else if state.tries >= 3 && state.bled_off {
                // Gone home bleeding, as the driver said: not done either.
                state.note(
                    now,
                    Kind::Learned,
                    "home, the job dropped for the bleed: unpaid, as it should be",
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
            // And with no radio on it, the crate's: the driver's spares go
            // there now, not on the ledge. (Woken in the truck after a fall,
            // it stepped out with nothing, and the raid saved out there
            // with no radio to call the truck with: the careers wedged.)
            let mut ledge = ledge;
            if carried(&world, &view, player, "radio").is_empty()
                && let Some(e) = view
                    .stash
                    .iter()
                    .flat_map(|b| b.contents.iter().copied())
                    .find(|e| world.id(*e) == Some("radio"))
            {
                ledge.push(e);
            }
            if view.hatch.busy() && state.tries < 30 {
                tasks.push(Task::Wait(1.0));
                state.tries += 1;
            } else if let Some(&e) = ledge.first().filter(|_| state.tries < 40) {
                let id = world.id(e).unwrap_or("?").to_owned();
                let from = if view.stash.iter().any(|b| b.contents.contains(&e)) {
                    "out of the crate"
                } else {
                    "off the ledge, from the driver"
                };
                state.note(now, Kind::Learned, format!("{id} {from}"));
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
            // Woken in the truck after a fall: first what the driver put on
            // the ledge (the spare radio, the spare pistol; left there, the
            // newcomer went back out with no radio to call the truck with).
            if raid.phase == Phase::AtBase && !state.took_ledge {
                state.took_ledge = true;
                state.leg = Leg::Handed;
                state.tries = 0;
                return;
            }
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
                // A moment noted as its line's template ("{s} things")
                // quotes what was said instead.
                if let Some(filled) = view.spoken.as_ref() {
                    for m in state.moments.iter_mut() {
                        if let Some((t, said)) = filled
                            .2
                            .iter()
                            .find(|(t, said)| said != t && !said.is_empty() && m.2.ends_with(*t))
                        {
                            m.2 = format!("{}{said}", &m.2[..m.2.len() - t.len()]);
                        }
                    }
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
        // Below ground, in a site, with a walk planned off its floors (to
        // the truck at the ranger tower, over the shelter it was in): up
        // and out by the site's way first.
        let below = below_in(&world.zone, &plans, root.translation)
            .is_some_and(|pl| !on_its_floors(pl, *to));
        if below || inside(here).is_some_and(|b| inside(*to) != Some(b)) {
            let (mut out, end) = way_out_at(&world, &view, &plans, root.translation);
            if below && !out.is_empty() {
                // The walk on from where the way out leaves it, not from
                // below: planned from the shelter, its first leg ran from
                // the kiosk's door straight back through the kiosk to the
                // truck over the shelter, and the newcomer went in again.
                let lead = tasks
                    .iter()
                    .take_while(|t| matches!(t, Task::WalkTo { .. }))
                    .count();
                if let Some(Task::WalkTo { to: dest, .. }) = lead.checked_sub(1).map(|i| &tasks[i])
                {
                    let dest = *dest;
                    tasks.splice(..lead, walk(&world, end, dest));
                }
            }
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
/// What the newcomer notes when the driver takes the truck to the card's
/// valley (the route card turned, the bell rung).
const DRIVEN: &str = "the driver's off to the card's valley: a long drive";

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
    // Or off to the valley on the card, as the driver said: the raid's
    // there (the next stage opens in it).
    let driven = moments.iter().any(|(_, _, w)| w == DRIVEN);
    if home && bad == 0 {
        bot.passed += 1;
        bot.log.push(format!("PASS {} got home", p.name));
    } else if driven && bad == 0 {
        bot.passed += 1;
        bot.log
            .push(format!("PASS {} off to the card's valley", p.name));
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
    /// Its machine's on the route (placed once the newcomer's out, its kit
    /// on): before that, a tell is some other machine's business.
    fn heeds_tells(&self) -> bool {
        self.machine.is_some()
    }
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

/// Nearer a heavy than this, a wound waits: on away first.
const HEAVY_DRESS_CLEAR: f32 = 12.0;

/// Off from a heavy, as the driver says ("walk away, and keep going"): a
/// longer leg than [`walk_away`]'s and no stand at its end, renewed
/// before it's done, so it's a steady walk (2.4 m/s against its hunt's
/// 1.7) until it lets go.
fn walk_off_heavy(world: &World2, here: Vec2, from: Vec2) -> Vec<Task> {
    let mut t = walk_away_by(world, here, from, 25.0);
    if matches!(t.last(), Some(Task::Wait(_))) {
        t.pop();
    }
    t
}

/// Away from a slow machine at `from`, at a walk, by the way round: 15 m
/// off, and a breath to look back.
fn walk_away(world: &World2, here: Vec2, from: Vec2) -> Vec<Task> {
    walk_away_by(world, here, from, 15.0)
}

/// [`walk_away`], `leg` metres off.
fn walk_away_by(world: &World2, here: Vec2, from: Vec2, leg: f32) -> Vec<Task> {
    let away = (here - from).normalize_or(Vec2::Y);
    // Away, or turned along whatever's ahead (the fence, the map's edge):
    // the first that stays inside the fence. (Straight away every 15 m, the
    // lost newcomer went out through the south gate to the map's edge, and
    // the heavy had it in the corner.)
    let inside = world.zone.map.half() - world.zone.map.fence_inset.max(0.0) - 5.0;
    let to = [0.0f32, 45.0, -45.0, 90.0, -90.0]
        .into_iter()
        .map(|deg| here + Vec2::from_angle(deg.to_radians()).rotate(away) * leg)
        .find(|p| p.x.abs() < inside && p.y.abs() < inside)
        .unwrap_or(here + away * leg);
    let mut t = walk(world, here, to);
    t.push(Task::Wait(2.0));
    t
}

/// A straight walk `walk` wouldn't make: longer than its legs go, or
/// through the quarantine fence (out of doors only: a step in a building
/// is its plan's).
fn unvetted(world: &World2, here: Vec2, to: Vec2) -> bool {
    let indoors = |p: Vec2| {
        world
            .entrances
            .building_at(Vec3::new(p.x, world.zone.on_ground(p.x, p.y).y + 0.5, p.y))
            .is_some()
    };
    if indoors(here) || indoors(to) {
        return false;
    }
    here.distance(to) > 20.0
        || fence_boxes(&world.zone).iter().any(|(xf, half)| {
            let inv = xf.compute_affine().inverse();
            let a = inv.transform_point3(Vec3::new(here.x, 0.0, here.y)).xz();
            let b = inv.transform_point3(Vec3::new(to.x, 0.0, to.y)).xz();
            segment_hits_box(a, b, *half).is_some()
        })
}

/// The first of `boxes` a straight walk `a`..`b` goes through (its
/// centre), if any. A box an end is in is no bar: that's a way into it or
/// out of it (a building's plan, a door's step). Nor is a corner it cuts
/// by less than a body's half-width: a body grazes it and goes on (a dash
/// into cover by the lorry wreck on the Marrow farm, 0.2 m into its corner,
/// put round the corner instead was three times as long, out in the
/// walker's line, and the one-handed newcomer was shot).
fn crossed(boxes: &[(Transform, Vec2)], a: Vec2, b: Vec2) -> Option<Vec2> {
    boxes.iter().find_map(|(xf, half)| {
        let inv = xf.compute_affine().inverse();
        let la = inv.transform_point3(Vec3::new(a.x, 0.0, a.y)).xz();
        let lb = inv.transform_point3(Vec3::new(b.x, 0.0, b.y)).xz();
        // (On its edge counts: a way out of a box, or a spot by its side.)
        let edge = *half + Vec2::splat(ON_EDGE);
        let inside = |l: Vec2| l.x.abs() < edge.x && l.y.abs() < edge.y;
        let deep = (*half - Vec2::splat(BODY)).max(Vec2::splat(0.01));
        (!inside(la) && !inside(lb) && segment_hits_box(la, lb, deep).is_some())
            .then(|| xf.translation.xz())
    })
}

/// The one way a walk leg comes to the newcomer's feet: from `here` to
/// `to`, straight through something `walk` goes round (a building, a
/// solid, an open hall's wall, the fence), `walk`'s way there instead, its
/// last leg as near as the leg asked; `None` to walk it as it is (clear,
/// a sidestep round a body, or no clear way round found). What `walk` goes
/// round stands on the ground: in a building or under it, the leg is the
/// indoor plan's, and walked as it is. (The lost newcomer, in the post
/// office's cellar, had a leg under the post office's wall put round it
/// above: out to the cellar's wall, and stuck there.)
fn routed_leg(world: &World2, feet: Vec3, to: Vec2, within: f32) -> Option<Vec<Task>> {
    let here = feet.xz();
    if within == crate::bot::SIDESTEP || within == ON_STAIRS || here.distance(to) < 0.5 {
        return None;
    }
    let below = feet.y < world.zone.on_ground(here.x, here.y).y - 1.5;
    if below || world.entrances.building_at(feet + Vec3::Y * 0.5).is_some() {
        return None;
    }
    let boxes = walk_boxes(world);
    crossed(&boxes, here, to)?;
    let clear_way = |to: Vec2| {
        let mut way = walk(world, here, to);
        match way.last_mut() {
            Some(Task::WalkTo { to: end, within: w }) if end.distance(to) < 0.1 => *w = within,
            _ => return None,
        }
        let mut at = here;
        for t in &way {
            if let Task::WalkTo { to: p, .. } = t {
                if crossed(&boxes, at, *p).is_some() {
                    return None;
                }
                at = *p;
            }
        }
        Some(way)
    };
    // No clear way to the spot itself: it's in something (cover picked 6 m
    // past a building's middle, on the rail shed's far wall). Out of that
    // to its nearest side, and there.
    clear_way(to).or_else(|| {
        let out = clear_of(&boxes, to);
        (out != to).then(|| clear_way(out)).flatten()
    })
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

/// What the clock says to do now, under cover or on the job.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hurry {
    /// Home ahead of the Surge: call the truck, from wherever it is.
    Surge,
    /// The truck's there and its wait running down: out from cover to it.
    Truck,
}

/// A time sense (`direct_raid`): out on the job with the Surge's warning
/// near, or hiding while it is (once), it calls; hiding with the truck
/// there a minute and more, out to it while no drone has it (not more
/// than every 30 s).
fn time_short(
    phase: &Phase,
    surge_in: f32,
    leg: Leg,
    hiding: bool,
    hunted: bool,
    pressed: bool,
    since_hurried: f32,
) -> Option<Hurry> {
    let surge_close =
        matches!(phase, Phase::Out) && surge_in < crate::raid::SURGE_WARNING + SURGE_MARGIN;
    let wait_running = matches!(phase, Phase::TruckWaiting { left, .. }
        if *left < crate::raid::TRUCK_WAIT - TRUCK_PATIENCE);
    if surge_close
        && (matches!(leg, Leg::Out | Leg::Place) || (leg == Leg::Call && hiding && !pressed))
    {
        return Some(Hurry::Surge);
    }
    (wait_running
        && hiding
        && !hunted
        && matches!(leg, Leg::Call | Leg::Rendezvous | Leg::Board)
        && since_hurried > 30.0)
        .then_some(Hurry::Truck)
}

/// How a bleed eases by itself (HP a second, each second: `health`).
const BLEED_EASES: f32 = 0.02;

/// Seconds till `hp` is bled away at `rate` HP a second, the rate easing
/// by `BLEED_EASES` each second; None if it stops first. (Health lost
/// by t is rate·t − ½·BLEED_EASES·t².)
fn bleeds_out_in(hp: f32, rate: f32) -> Option<f32> {
    if hp <= 0.0 {
        return Some(0.0);
    }
    let disc = rate * rate - 2.0 * BLEED_EASES * hp;
    (rate > 0.0 && disc >= 0.0).then(|| (rate - disc.sqrt()) / BLEED_EASES)
}

/// What "bleeding badly ... Get to the truck" means from where it is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BleedGo {
    /// The truck's at the rendezvous: to it, now.
    ToTruck,
    /// Call it from here (off the job; not back to the gate first).
    Call,
}

fn bleed_answer(leg: Leg, phase: &Phase) -> Option<BleedGo> {
    let truck_there = matches!(phase, Phase::TruckWaiting { .. });
    match leg {
        Leg::Out | Leg::Place | Leg::Call | Leg::Rendezvous if truck_there => {
            Some(BleedGo::ToTruck)
        }
        Leg::Out | Leg::Place => Some(BleedGo::Call),
        // Called already, but still out on its way back to the gate or
        // under a roof: from here, now.
        Leg::Call if matches!(phase, Phase::Out) => Some(BleedGo::Call),
        _ => None,
    }
}

/// The eye of the walker after it: the one that last aimed, else the
/// nearest walker with it in mind, else the nearest walker in reach (the
/// first warning can come before any aim it's told of, or a mind that has
/// it).
fn walker_eye(
    world: &World2,
    eyes: &Query<(&GlobalTransform, &crate::robots::Senses)>,
    aimer: Option<Entity>,
    here: Vec2,
    on_me: &impl Fn(Entity) -> bool,
) -> Option<Vec3> {
    the_walker(world, eyes, aimer, here, on_me).and_then(|e| eye_of(eyes, e))
}

fn eye_of(eyes: &Query<(&GlobalTransform, &crate::robots::Senses)>, e: Entity) -> Option<Vec3> {
    eyes.get(e).ok().map(|(gt, s)| gt.transform_point(s.eye))
}

/// The walker that has it in mind: the one that aimed, else the nearest
/// on it (or near).
fn the_walker(
    world: &World2,
    eyes: &Query<(&GlobalTransform, &crate::robots::Senses)>,
    aimer: Option<Entity>,
    here: Vec2,
    on_me: &impl Fn(Entity) -> bool,
) -> Option<Entity> {
    the_walker_of(world, eyes, aimer, here, on_me, &|_| true)
}

/// A walker that hunts by eye: not a hound, a crawler or a grappler, each
/// a walker's legs under another mind (a hound goes for what moves: moved
/// out of its sight when it came to look, the lost one was bitten down).
fn by_eye(view: &RaidView, e: Entity) -> bool {
    view.robots
        .get(e)
        .map_or(true, |c| !c.hound && !c.crawler && !c.grappler)
}

/// `the_walker`, of those `keep` keeps.
fn the_walker_of(
    world: &World2,
    eyes: &Query<(&GlobalTransform, &crate::robots::Senses)>,
    aimer: Option<Entity>,
    here: Vec2,
    on_me: &impl Fn(Entity) -> bool,
    keep: &dyn Fn(Entity) -> bool,
) -> Option<Entity> {
    aimer
        .filter(|e| keep(*e) && eye_of(eyes, *e).is_some())
        .or_else(|| {
            world
                .robots
                .iter()
                .filter(|(e, r, _)| {
                    r.kind == crate::robots::RobotKind::Walker && !r.wrecked && keep(*e)
                })
                .filter_map(|(e, ..)| eye_of(eyes, e).map(|p| (e, p, on_me(e))))
                .filter(|(_, p, mine)| *mine || p.xz().distance(here) < WALKER_NEAR)
                .min_by(|a, b| {
                    (!a.2, a.1.xz().distance(here))
                        .partial_cmp(&(!b.2, b.1.xz().distance(here)))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, ..)| e)
        })
}

/// A standing head (m above the ground).
const STAND_HEAD: f32 = 1.6;
/// How far either side a fighting walker's eye is allowed for (m).
const EYE_SWAY: f32 = 2.5;
/// A walker this near with nobody else in mind is the one aiming (m).
const WALKER_NEAR: f32 = 80.0;
/// Low and still out of a walker's line this long (s): its aim lasts
/// 1.5 s, and it comes round to look.
const LINE_HELD: f32 = 12.0;
/// A crouched head (m above the ground), as a walker's eye looks for it.
const LOW_HEAD: f32 = 0.9;
/// How long after a walker's aim where it last saw it still counts as
/// where it'll come to look.
const SEEN_LAST_FOR: f32 = 90.0;
/// How far on from the last a spot it was seen at is kept (m).
const SEEN_STEP: f32 = 3.0;
/// How near a walker has to be for being in its line to count (m).
const SEEN_REACH: f32 = 80.0;
/// How many it keeps.
const SEEN_KEPT: usize = 8;

/// Whether to shoot back at a walker (`direct_raid`): aimed at lately, out
/// of cover and not on the way to any, a round up, and shot at twice in a
/// few seconds (its line couldn't be broken in time).
fn no_way_out(
    shots: &[f32],
    now: f32,
    aimed_at: f32,
    hidden: bool,
    running: bool,
    ready: bool,
) -> bool {
    ready
        && !hidden
        && !running
        && now - aimed_at < AIMED_FOR
        && shots.iter().filter(|t| now - **t < SHOT_WINDOW).count() >= 2
}

/// Whether to dress a wound now: bleeding enough to matter, no round at
/// it in the last few seconds, not on a run to cover, and not again for a
/// few seconds after the last (its use takes that long).
fn wound_to_dress(bleeding: f32, shots: &[f32], now: f32, dressed_at: f32, running: bool) -> bool {
    bleeding > BLEED_WORTH
        && !running
        && now - dressed_at > DRESS_EVERY
        && shots.iter().all(|t| now - t > CONTACT_FOR)
}

/// Not answering a crawler's crackle again sooner than this (s): its zaps
/// come every 2 s or so, and the answer takes longer.
const CRAWLER_EVERY: f32 = 3.0;
/// A crawler that charged at it this lately (s) is still at it: no keeping
/// still, no dressing in its line.
const CRAWLER_CLEAR: f32 = 5.0;
/// A wait this long (s) is one to keep still in, and goes when a crawler
/// charges.
const CRAWLER_STILL: f32 = 5.0;

/// A crawler charged at it lately enough to be at it still (`CRAWLER_CLEAR`).
fn crawler_at_it(now: f32, crawled_at: f32) -> bool {
    now - crawled_at < CRAWLER_CLEAR
}

/// What a crawler's charge gets from the newcomer.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CrawlerAnswer {
    /// A gun to hand and a round to fire (in it, or a loaded spare).
    Shoot,
    /// Its only loaded gun in the pack: out with it, then at it.
    Unpack,
    /// Nothing to shoot: back out the way it came, out of its line.
    BackOut,
}

fn crawler_answer(armed: bool, rounds: bool, packed_loaded: bool) -> CrawlerAnswer {
    if armed && rounds {
        CrawlerAnswer::Shoot
    } else if packed_loaded {
        CrawlerAnswer::Unpack
    } else {
        CrawlerAnswer::BackOut
    }
}

/// A bleed this fast is worth stopping to dress (hp/s).
const BLEED_WORTH: f32 = 0.3;
/// Not more often than this (s).
const DRESS_EVERY: f32 = 8.0;
/// Shot at within this, it's still in the fight (s).
const CONTACT_FOR: f32 = 3.0;

/// A walker's aim this recent still has it in mind (s).
const AIMED_FOR: f32 = 20.0;
/// Two rounds at it inside this, out of cover, and it shoots back (s).
const SHOT_WINDOW: f32 = 4.0;

/// How long the run to a spot out of a walker's line takes (s): upright and
/// at a run till there, then low (crept there, it was in the walker's line
/// three times as long, and took its rounds on the way: first_jobs raid 5).
/// Nothing to run to (down where it is), none.
fn run_to_line(tasks: &[Task], here: Vec2) -> f32 {
    let mut at = here;
    let mut run = 0.0;
    for t in tasks {
        let Task::WalkTo { to, .. } = t else {
            break;
        };
        run += at.distance(*to);
        at = *to;
    }
    if run > 0.0 { run / RUN_PACE + 0.5 } else { 0.0 }
}

/// A run's pace on open ground (m/s), for timing one.
const RUN_PACE: f32 = 3.0;

/// A crouched head at `p` out of sight of `eye`: something solid between.
fn line_hidden(
    spatial: &avian3d::prelude::SpatialQuery,
    world: &World2,
    p: Vec2,
    eye: Vec3,
) -> bool {
    hidden_at(spatial, world, p, eye, LOW_HEAD)
}

/// A head `head` m up at `p` out of sight of `eye`.
fn hidden_at(
    spatial: &avian3d::prelude::SpatialQuery,
    world: &World2,
    p: Vec2,
    eye: Vec3,
    head: f32,
) -> bool {
    let walls = avian3d::prelude::SpatialQueryFilter::from_mask([crate::world::Layer::World]);
    let head = world.zone.on_ground(p.x, p.y) + Vec3::Y * head;
    let to = head - eye;
    let d = to.length();
    Dir3::new(to)
        .ok()
        .and_then(|dir| spatial.cast_ray(eye, dir, d - 0.3, true, &walls))
        .is_some()
}

/// Knee height over the ground, for what's in the way of a step (m): a
/// body-wide sweep from there clears the ground underfoot.
const KNEE: f32 = 0.6;
/// Chest height over the ground (m): a body-wide sweep from the knee to
/// here, and a body-wide end on it, reaches a standing head's height (1.7)
/// and no higher.
const CHEST: f32 = 1.4;
/// A body's half width, kept clear past the end of a step (m).
const BODY: f32 = 0.3;

/// Out of sight of `eye`, the nearest way: low where it is, if that does it
/// (a wall, a bank, a car between); else the nearest spot within 12 m it
/// can walk to where a crouched head is hidden, and still there. None:
/// nothing near breaks the line.
fn break_line(
    spatial: &avian3d::prelude::SpatialQuery,
    world: &World2,
    here: Vec2,
    eye: Vec3,
) -> Option<(Vec<Task>, &'static str)> {
    break_line_from(spatial, world, here, &[eye], false)
}

/// [`break_line`], out of every one of `eyes`' lines (the first the one
/// it goes away from); `wide`, further off where nothing near will do.
fn break_line_from(
    spatial: &avian3d::prelude::SpatialQuery,
    world: &World2,
    here: Vec2,
    eyes: &[Vec3],
    wide: bool,
) -> Option<(Vec<Task>, &'static str)> {
    let eye = *eyes.first()?;
    let hidden = |p: Vec2| eyes.iter().all(|e| line_hidden(spatial, world, p, *e));
    if hidden(here) {
        return Some((vec![Task::Wait(LINE_HELD)], "down where it was"));
    }
    let nav = world.nav.as_deref().map(|n| n.0.as_ref());
    // Walkable on the grid, and walked to straight: nothing solid from knee
    // to head height between, nor at the spot (the grid's cells are
    // coarse).
    // (Not the ground itself: a body-wide ball at the knee met every bump
    // of it, no spot passed, and the newcomer, with no way out of the
    // walker's line, shot it out and lost.)
    let walls = avian3d::prelude::SpatialQueryFilter::from_mask([crate::world::Layer::World])
        .with_excluded_entities(world.terrain.iter());
    let mid = |q: Vec2| world.zone.on_ground(q.x, q.y) + Vec3::Y * (KNEE + CHEST) * 0.5;
    // (A body's width, not a ray: a ray slipped past a trunk the newcomer
    // then pressed against, 1.4 m short, the same half minute. And a
    // body's height, not a ball at the knee: the ball passed under the
    // quarantine fence where the ground dips beneath it, and the cautious
    // newcomer stood against the wire, 1.1 m short, the half minute.)
    let body = avian3d::prelude::Collider::capsule(BODY, CHEST - KNEE);
    let reach = |a: Vec2, p: Vec2| {
        let to = mid(p) - mid(a);
        Dir3::new(to).ok().is_none_or(|dir| {
            spatial
                .cast_shape(
                    &body,
                    mid(a),
                    Quat::IDENTITY,
                    dir,
                    &avian3d::prelude::ShapeCastConfig::from_max_distance(to.length() + BODY),
                    &walls,
                )
                .is_none()
        })
    };
    // (Nor up a bank: the one-handed newcomer's spot was on the creek's,
    // 3 m up in 4, and it stood at the bank's foot the half minute.)
    let open = |a: Vec2, p: Vec2| {
        nav.is_none_or(|n| n.walkable(p.to_array()) && blocked_run(n, a, p) < 0.5)
            && steep_run(&world.zone, a, p) <= STEEP_RUN
            && reach(a, p)
    };
    let away = (here - eye.xz()).normalize_or(Vec2::Y);
    // The eye a step or two either side of where it is: a walker that's
    // fighting moves, and a spot only just out of its line now was in it
    // again a breath later ("in its line again", twice, on Rime).
    let side = away.perp() * EYE_SWAY;
    let sway: Vec<Vec3> = eyes
        .iter()
        .flat_map(|e| {
            [
                *e,
                *e + Vec3::new(side.x, 0.0, side.y),
                *e - Vec3::new(side.x, 0.0, side.y),
            ]
        })
        .collect();
    // Best first: hidden standing from all three, then low from all three;
    // last, low from where it is now.
    let ranks: [(&dyn Fn(Vec2) -> bool, &'static str); 3] = [
        (
            &|p| {
                sway.iter()
                    .all(|e| hidden_at(spatial, world, p, *e, STAND_HEAD))
            },
            "behind something tall",
        ),
        (
            &|p| {
                sway.iter()
                    .all(|e| hidden_at(spatial, world, p, *e, LOW_HEAD))
            },
            "low behind something",
        ),
        (&hidden, "behind something near"),
    ];
    for (good, how) in ranks {
        if let Some(way) = nearest_where(here, away, &open, good) {
            let n = way.len();
            let mut t: Vec<Task> = way
                .into_iter()
                .enumerate()
                .map(|(k, to)| Task::WalkTo {
                    to,
                    within: if k + 1 == n { 0.6 } else { 0.8 },
                })
                .collect();
            t.push(Task::Wait(LINE_HELD));
            return Some((t, how));
        }
    }
    // Nothing that near: further off, to 30 m, the far sides of buildings
    // too, but only a spot out of every one of its views (run to a
    // building's far side unlooked, the wanderer crouched in the open in
    // the view of where the walker came to look, and was shot down).
    // Not under its aim: run 30 m across its eye, three newcomers who'd
    // have come home were shot down on the way.
    if !wide {
        return None;
    }
    let mut t = wide_cover(world, here, &hidden)?;
    t.push(Task::Wait(LINE_HELD));
    Some((t, "out of its view, further off"))
}

/// How far the wider look for cover goes (m).
const WIDE_COVER: f32 = 30.0;

/// A hunt is the same walker aiming at it again inside this (s).
const HUNT_WINDOW: f32 = 20.0;
/// Aims this close together are one sighting (s).
const HUNT_APART: f32 = 2.0;
/// How far a creep away goes (m): out of a searching walker's hearing
/// (a crouched creep carries a couple of metres) and on past its search.
const CREEP_NEAR: f32 = 20.0;
const CREEP_FAR: f32 = 30.0;
/// At least this much further from the walker than where it starts (m).
const CREEP_GAIN: f32 = 12.0;
/// A crouched walk's pace (m/s), for timing one.
const CREEP_PACE: f32 = crate::player::CROUCH_SPEED;
/// How often (s) a creep looks back at the walker it's creeping from.
const CREEP_CHECK: f32 = 0.5;

/// Hunted: a walker that has aimed at it, then again after losing it,
/// inside [`HUNT_WINDOW`]. Its eye where it last aimed from.
fn hunted(aims: &[(Entity, Vec3, f32)], now: f32) -> Option<Vec3> {
    let recent = |w: Entity| {
        aims.iter()
            .filter(|(e, _, t)| *e == w && now - t < HUNT_WINDOW)
            .count()
    };
    aims.iter()
        .rev()
        .find(|(w, _, t)| now - t < HUNT_WINDOW && recent(*w) >= 2)
        .map(|(_, eye, _)| *eye)
}

/// Whether `feet` are inside walls: a building's (its plan or its shell)
/// or a kiosk's (`kiosk_boxes`). Only from there does a hunted newcomer
/// creep away ([`creep_away`]). Indoors, hunted, you're cornered, and the
/// walls are a way out of its sight: out of the far side, unseen. In the
/// open, moving draws the eye: low behind cover and still is the better
/// answer, and the line break stands. (Crept away across open ground, the
/// lost newcomer was glimpsed on the way, followed and shot; held where it
/// froze, it was beside where the walker came to look, and shot.)
fn under_walls(world: &World2, plans: &[crate::sim::indoor::BuildingPlan], feet: Vec3) -> bool {
    plans.iter().any(|pl| pl.contains(feet, 0.3))
        || world.entrances.building_at(feet - Vec3::Y * 0.5).is_some()
        || kiosk_boxes(world).iter().any(|(xf, half)| {
            let l = xf.compute_affine().inverse().transform_point3(feet);
            l.x.abs() < half.x && l.z.abs() < half.y
        })
}

/// The walker [`hunted`] names.
fn hunter(aims: &[(Entity, Vec3, f32)], now: f32) -> Option<Entity> {
    let recent = |w: Entity| {
        aims.iter()
            .filter(|(e, _, t)| *e == w && now - t < HUNT_WINDOW)
            .count()
    };
    aims.iter()
        .rev()
        .find(|(w, _, t)| now - t < HUNT_WINDOW && recent(*w) >= 2)
        .map(|(w, _, _)| *w)
}

/// Facing it, with it in its eyes' cone, and nothing between: the walker
/// at `eye` (`gt`, `senses`) could see it at `here` now.
fn faces_me(
    spatial: &avian3d::prelude::SpatialQuery,
    world: &World2,
    gt: &GlobalTransform,
    senses: &crate::robots::Senses,
    here: Vec2,
) -> bool {
    let eye = gt.transform_point(senses.eye);
    let to = (here - eye.xz()).normalize_or_zero();
    let ahead = gt.forward().as_vec3().xz().normalize_or_zero();
    to.dot(ahead) >= (senses.fov_deg.to_radians() * 0.5).cos()
        && !line_hidden(spatial, world, here, eye)
}

/// How far a walk's legs go from `here` (m).
fn walked_len(tasks: &[Task], here: Vec2) -> f32 {
    let mut at = here;
    let mut len = 0.0;
    for t in tasks {
        if let Task::WalkTo { to, .. } = t {
            len += at.distance(*to);
            at = *to;
        }
    }
    len
}

/// Away from a walker hunting it (last seen aiming from `from`): a spot
/// [`CREEP_NEAR`]..[`CREEP_FAR`] m off, [`CREEP_GAIN`] m further from it
/// than here, out of view of every one of `eyes` (a crouched head), and a
/// way there (`walk`, round buildings) that never closes on it. In a
/// building (`plans`), out first by a way that doesn't close on it either
/// (in the rail shed, the wanderer's straight legs out were squared
/// through its one door, on the walker's side, and it was found in it).
/// The shortest such walk, or None.
fn creep_away(
    spatial: &avian3d::prelude::SpatialQuery,
    world: &World2,
    plans: &[crate::sim::indoor::BuildingPlan],
    here: Vec2,
    from: Vec2,
    eyes: &[Vec3],
) -> Option<Vec<Task>> {
    let start = here.distance(from);
    let feet = world.zone.on_ground(here.x, here.y) + Vec3::Y;
    let outs: Vec<(Vec<Task>, Vec2, f32)> = match plans.iter().find(|pl| pl.contains(feet, 0.3)) {
        Some(plan) => {
            let mut outs: Vec<_> = open_ways_out(plan, plan.to_local(feet).xz())
                .into_iter()
                .filter(|(_, o, _)| o.distance(from) >= start - 3.0)
                .collect();
            // (The nearest few: each is a search out of doors.)
            outs.sort_by(|a, b| a.2.total_cmp(&b.2));
            outs.truncate(4);
            outs
        }
        None => vec![(vec![], here, 0.0)],
    };
    outs.into_iter()
        .filter_map(|(mut t, at, len)| {
            let rest = creep_from(spatial, world, at, from, start, eyes)?;
            let total = len + walked_len(&rest, at);
            t.extend(rest);
            Some((t, total))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(t, _)| t)
}

/// [`creep_away`]'s walk out of doors from `here`, for one that started
/// `start` m from the walker.
fn creep_from(
    spatial: &avian3d::prelude::SpatialQuery,
    world: &World2,
    here: Vec2,
    from: Vec2,
    start: f32,
    eyes: &[Vec3],
) -> Option<Vec<Task>> {
    let far_enough = start.max(CREEP_NEAR - CREEP_GAIN) + CREEP_GAIN;
    let nav = world.nav.as_deref().map(|n| n.0.as_ref());
    let mut spots: Vec<Vec2> = [CREEP_NEAR, 23.0, 26.0, CREEP_FAR]
        .into_iter()
        .flat_map(|r| {
            (0..32)
                .map(move |k| here + Vec2::from_angle(k as f32 * std::f32::consts::TAU / 32.0) * r)
        })
        .filter(|p| p.distance(from) >= far_enough)
        .filter(|p| nav.is_none_or(|n| n.walkable(p.to_array())))
        .filter(|p| {
            world
                .entrances
                .building_at(world.zone.on_ground(p.x, p.y) + Vec3::Y)
                .is_none()
        })
        .filter(|p| eyes.iter().all(|e| line_hidden(spatial, world, *p, *e)))
        .collect();
    // Furthest from it first among the near ones: the nearest ring
    // first, and on a ring the way most directly away.
    let away = (here - from).normalize_or(Vec2::Y);
    spots.sort_by(|a, b| {
        let key = |p: &Vec2| {
            (p.distance(here) / 3.0).round() * 10.0 - (*p - here).normalize_or_zero().dot(away)
        };
        key(a).total_cmp(&key(b))
    });
    spots
        .into_iter()
        .take(8)
        .map(|p| walk(world, here, p))
        .filter(|t| !t.is_empty())
        .filter(|t| {
            t.iter().all(|task| match task {
                Task::WalkTo { to, .. } => to.distance(from) >= start - 3.0,
                _ => true,
            })
        })
        .min_by(|a, b| walked_len(a, here).total_cmp(&walked_len(b, here)))
}

/// The nearest spot within [`WIDE_COVER`] where `hidden` holds, by the way
/// walked there (round buildings, `walk`): rings out from `here` and round
/// each building near. The walk to it, or None.
fn wide_cover(world: &World2, here: Vec2, hidden: &dyn Fn(Vec2) -> bool) -> Option<Vec<Task>> {
    let rings = [15.0f32, 18.0, 22.0, 26.0, WIDE_COVER]
        .into_iter()
        .flat_map(|r| {
            (0..24)
                .map(move |k| here + Vec2::from_angle(k as f32 * std::f32::consts::TAU / 24.0) * r)
        });
    let sides = world
        .zone
        .map
        .placements
        .iter()
        .filter(|b| Vec2::from(b.at).distance(here) < WIDE_COVER + 10.0)
        .flat_map(|b| {
            let foot = extraction_core::map::prefab_footprint(&b.prefab);
            let r = 0.5 * Vec2::new(foot[0], foot[2]).length() + 1.5;
            let c = Vec2::from(b.at);
            (0..12).map(move |k| c + Vec2::from_angle(k as f32 * std::f32::consts::TAU / 12.0) * r)
        });
    let nav = world.nav.as_deref().map(|n| n.0.as_ref());
    let mut spots: Vec<Vec2> = rings
        .chain(sides)
        .filter(|p| p.distance(here) <= WIDE_COVER)
        .filter(|p| nav.is_none_or(|n| n.walkable(p.to_array())))
        .filter(|p| {
            world
                .entrances
                .building_at(world.zone.on_ground(p.x, p.y) + Vec3::Y)
                .is_none()
        })
        .collect();
    spots.sort_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
    let walked = |t: &[Task]| {
        let mut at = here;
        let mut len = 0.0;
        for task in t {
            if let Task::WalkTo { to, .. } = task {
                len += at.distance(*to);
                at = *to;
            }
        }
        len
    };
    spots
        .into_iter()
        .filter(|p| hidden(*p))
        .take(6)
        .map(|p| walk(world, here, p))
        .filter(|t| !t.is_empty())
        .min_by(|a, b| walked(a).total_cmp(&walked(b)))
}

/// The nearest walkable spot within 12 m where `good` holds, and the way
/// to it (`open` from one point to the next: straight, or round one corner
/// where straight won't do): nearest first, and at a range those off the
/// line from `away`'s far end before those back toward it.
fn nearest_where(
    here: Vec2,
    away: Vec2,
    open: &dyn Fn(Vec2, Vec2) -> bool,
    good: &dyn Fn(Vec2) -> bool,
) -> Option<Vec<Vec2>> {
    for r in [2.0f32, 3.5, 5.0, 7.0, 9.5, 12.0] {
        // Nearest first; at a range, those off its line before those
        // back toward it.
        let mut ring: Vec<Vec2> = (0..16)
            .map(|k| here + Vec2::from_angle(k as f32 * std::f32::consts::TAU / 16.0) * r)
            .collect();
        ring.sort_by(|a, b| {
            let fa = (*a - here).normalize_or_zero().dot(away);
            let fb = (*b - here).normalize_or_zero().dot(away);
            fb.total_cmp(&fa)
        });
        if let Some(way) = ring
            .into_iter()
            .filter(|p| good(*p))
            .find_map(|p| way_round(here, p, open))
        {
            return Some(way);
        }
    }
    None
}

/// From `here` to `p` by `open` legs: straight, or else round one corner a
/// stride or two off either end (round the very thing it's crouched by: the
/// cautious newcomer stood on the walker's side of a log pile with the far
/// side 4.8 m off, and the straight way there went through the pile).
/// The shortest such way; none if neither does it.
fn way_round(here: Vec2, p: Vec2, open: &dyn Fn(Vec2, Vec2) -> bool) -> Option<Vec<Vec2>> {
    if open(here, p) {
        return Some(vec![p]);
    }
    let around = |c: Vec2| {
        [1.5f32, 2.5, 3.5].into_iter().flat_map(move |r| {
            (0..16).map(move |k| c + Vec2::from_angle(k as f32 * std::f32::consts::TAU / 16.0) * r)
        })
    };
    let mut corners: Vec<Vec2> = around(here).chain(around(p)).collect();
    corners.sort_by(|a, b| {
        (here.distance(*a) + a.distance(p)).total_cmp(&(here.distance(*b) + b.distance(p)))
    });
    corners
        .into_iter()
        .find(|q| open(here, *q) && open(*q, p))
        .map(|q| vec![q, p])
}

/// Home ahead of the Surge's warning by this much (s): time to call and
/// get to the truck.
const SURGE_MARGIN: f32 = 180.0;
/// Under cover with the truck there, out to it once it has waited this
/// long (s; it waits `TRUCK_WAIT`).
const TRUCK_PATIENCE: f32 = 60.0;
/// Once time's short, it stays short this long (s).
const TIME_SHORT_FOR: f32 = 600.0;
/// The truck waiting within this, a run for its bed beats any roof (m).
const BED_RUN: f32 = 40.0;

/// Clear of walls round a building's box, m: a body (0.28) and a stride.
const ROUND: f32 = 2.0;
/// How near a box's margin an end of a stretch counts as on it (m).
const ON_EDGE: f32 = 0.05;

/// From `from` to `to` on foot, round any building in the way (the
/// straight line through the village went into a house wall and stayed
/// there): the waypoints after `from`.
fn round_buildings(world: &World2, from: Vec2, to: Vec2) -> Vec<Vec2> {
    round_boxes(&walk_boxes(world), from, to)
}

/// What a walk goes round: the fence, the buildings
/// (an underground site's kiosks among them, its exits' too) and the
/// solids.
fn walk_boxes(world: &World2) -> Vec<(Transform, Vec2)> {
    let mut boxes: Vec<(Transform, Vec2)> = fence_boxes(&world.zone);
    boxes.extend(world.entrances.shells.iter().map(|s| (s.xf, s.half)));
    // A site's shell is its entrance kiosk's only: an exit's kiosk, tens
    // of metres off, is walls up top as well (the pump wells' outfall
    // stair: a leg from cover past it went in by its door and down the
    // stair, and the lost newcomer was stuck under its cover).
    boxes.extend(kiosk_boxes(world));
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
    boxes.extend(
        world
            .buildings
            .iter()
            .flat_map(|(b, xf)| part_boxes(&b.prefab, xf)),
    );
    boxes
}

/// Every underground site's kiosk not already a building's shell (its
/// exits'), as a box to walk round.
fn kiosk_boxes(world: &World2) -> Vec<(Transform, Vec2)> {
    let half = Vec2::from(extraction_core::underground::SHELL_HALF);
    extraction_core::underground::kiosks(&world.zone.map)
        .into_iter()
        .map(|(c, rot)| (Vec2::from(c), rot))
        .filter(|(c, _)| {
            world
                .entrances
                .shells
                .iter()
                .all(|s| s.xf.translation.xz().distance(*c) > 1.0)
        })
        .map(|(c, rot)| {
            let xf = Transform::from_translation(world.zone.on_ground(c.x, c.y))
                .with_rotation(Quat::from_rotation_y(-rot.to_radians()));
            (xf, half)
        })
        .collect()
}

/// What of a prefab with a way through it can't be walked through, part by
/// part (one box over all of it would shut the way). A quarantine gate's
/// concrete block and post, each side of the road: the newcomer walked from
/// the guard booth for the road, into the block, and stood there a minute
/// at the 0.6 m between it and the booth.
fn part_boxes(prefab: &str, xf: &Transform) -> Vec<(Transform, Vec2)> {
    // An open hall walked into and through: what stands on its floor, from
    // its own pieces (the sawmill's log stack lies inside its posts, off
    // any grid, and the newcomer walking home from the logging camp stood
    // against it two minutes).
    if OPEN_HALLS.contains(&prefab) {
        return standing_parts(prefab)
            .into_iter()
            .map(|(at, half)| (*xf * at, half))
            .collect();
    }
    match prefab {
        // (`buildings::quarantine_gate`: a 1.4 m block at 5.2, a post at
        // 4.3, either side.)
        "quarantine_gate" => [-1.0f32, 1.0]
            .map(|side| {
                let at = xf.transform_point(Vec3::new(side * 5.04, 0.0, 0.0));
                (
                    Transform::from_translation(at).with_rotation(xf.rotation),
                    Vec2::new(0.86, 0.7),
                )
            })
            .to_vec(),
        _ => vec![],
    }
}

/// Halls with a way in for a road, walked into and through, open inside
/// (`extraction_core::fit`'s drive-in prefabs).
const OPEN_HALLS: [&str; 5] = ["bus_depot", "rail_shed", "factory_hall", "sawmill", "barn"];

/// A prefab's solid pieces in a body's way, knee to head (`KNEE`..1.7 m
/// over its floor, a ball's half width either side of it), as boxes on the
/// ground in its own space: each piece's extent, square to the prefab.
fn standing_parts(prefab: &str) -> Vec<(Transform, Vec2)> {
    use crate::world::buildings::Shape;
    let Some(geo) = crate::world::buildings::prefab(prefab) else {
        return vec![];
    };
    let (low, high) = (KNEE - BODY, CHEST + BODY);
    geo.pieces
        .iter()
        .filter(|p| p.collide)
        .filter_map(|p| {
            let half = match p.shape {
                Shape::Box(s) | Shape::Prism(s) => s * 0.5,
                Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
            };
            let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
            for k in 0..8 {
                let c = Vec3::new(
                    if k & 1 == 0 { -half.x } else { half.x },
                    if k & 2 == 0 { -half.y } else { half.y },
                    if k & 4 == 0 { -half.z } else { half.z },
                );
                let w = p.at.transform_point(c);
                lo = lo.min(w);
                hi = hi.max(w);
            }
            (lo.y < high && hi.y > low).then(|| {
                let mid = (lo + hi) * 0.5;
                (
                    Transform::from_xyz(mid.x, 0.0, mid.z),
                    Vec2::new(hi.x - lo.x, hi.z - lo.z) * 0.5,
                )
            })
        })
        .collect()
}

/// `p`, or, inside one of `boxes` (with a body's clearance), out of it to
/// its nearest side, a stride clear (a sidestep round a bank landed in the
/// pumping station's kiosk, and the newcomer stood against its wall).
fn clear_of(boxes: &[(Transform, Vec2)], p: Vec2) -> Vec2 {
    let mut p = p;
    for (xf, half) in boxes {
        let a = xf.compute_affine();
        let l = a.inverse().transform_point3(Vec3::new(p.x, 0.0, p.y));
        let (hx, hz) = (half.x + 0.6, half.y + 0.6);
        if l.x.abs() >= hx || l.z.abs() >= hz {
            continue;
        }
        let out = if hx - l.x.abs() < hz - l.z.abs() {
            Vec3::new(l.x.signum() * (half.x + ROUND), 0.0, l.z)
        } else {
            Vec3::new(l.x, 0.0, l.z.signum() * (half.y + ROUND))
        };
        p = a.transform_point3(out).xz();
    }
    p
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
        // An end in the box itself: going into it (a place in a house: the
        // Old farm's is), so it's no bar to the stretch. (Taken as one,
        // every stretch to the place "hit" the house at its end, no way was
        // found, and the walk there went straight, through a log pile.)
        // `walk` steps a start out of any box first.
        if within(la, bx.1) || within(lb, bx.1) {
            return true;
        }
        let margin = bx.1 + Vec2::splat(ROUND * 0.5);
        // (On the margin's edge counts as in it: a way out of a building
        // ends a margin from its wall, and from there every stretch out
        // "entered" the margin at its first step, so no way was found and
        // the walk went straight, through a house and into a garden.)
        let edge = margin + Vec2::splat(ON_EDGE);
        let h = if within(la, edge) || within(lb, edge) {
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

/// Where to wait for the truck at the rendezvous `rv` when the drones
/// surveying a place nearby can see it: under the shelter's roof, or in its
/// lee away from that place (`map_lint::wait_shelter`, as raiders wait).
fn wait_point(map: &extraction_core::map::MapDef, rv: Vec2) -> Option<Vec2> {
    let at = rv.to_array();
    let (poi, _) = extraction_core::map_lint::watched_by(map, at)?;
    let from = map.markers_of(&MarkerKind::Poi).find(|m| m.name == poi)?.at;
    extraction_core::map_lint::wait_shelter(map, at, from).map(|(_, _, p)| Vec2::from_array(p))
}

/// This many drone locks in a row, cover taken each time and found again
/// on coming out (each within `RELOCK_GAP` of the last): pinned, and home.
const PINNED_AFTER: usize = 3;
/// Under a roof from a drone, held at most this long (s) while it still
/// has it.
const ROOF_HOLD: f32 = 120.0;
/// A lock this soon after the last is the same drone finding it again (s):
/// the hold under the roof, and a minute out from it. (Counted within two
/// minutes flat, the wanderer was found every 148 s coming out from the
/// same roof, held the full two minutes each time, for 25 minutes: never
/// three inside the window, never pinned.)
const RELOCK_GAP: f32 = ROOF_HOLD + 60.0;

/// A drone's lock heard now: how many in a row it's been found by.
fn found_again(relocks: &mut Vec<f32>, now: f32) -> usize {
    if relocks.last().is_some_and(|t| now - t > RELOCK_GAP) {
        relocks.clear();
    }
    relocks.push(now);
    relocks.len()
}

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
/// `gun`, out of the pack into the right hand.
fn unpack_gun(gun: Entity) -> Vec<Task> {
    vec![
        Task::Stow(Side::Right),
        Task::ToggleBackpack,
        Task::Wait(0.6),
        Task::GrabFromHere {
            item: gun,
            side: Side::Right,
        },
        Task::ToggleBackpack,
        Task::Wait(0.5),
    ]
}

fn take_it_on() -> Vec<Task> {
    vec![
        Task::DrawGun,
        Task::Ready { off_safe: true },
        Task::Engage { shots: 12 },
    ]
}

/// A radio lying loose within reach of `here` (m), and where.
fn radio_at_feet(
    world: &World2,
    plans: &[crate::sim::indoor::BuildingPlan],
    feet: Vec3,
) -> Option<(Entity, Vec2)> {
    // (By the floor-plane distance alone, the one up in the tower's cab
    // over its head was "at its feet".)
    world
        .items
        .iter()
        .filter(|(_, it, gt, held, sock, bag)| {
            held.is_none()
                && sock.is_none()
                && bag.is_none()
                && world.catalog.get(it.kind()).id == "radio"
                && in_reach(world, plans, gt.translation())
        })
        .map(|(e, _, gt, ..)| (e, gt.translation()))
        .filter(|(_, at)| at.distance(feet) < FEET_REACH)
        .min_by(|a, b| a.1.distance(feet).total_cmp(&b.1.distance(feet)))
        .map(|(e, at)| (e, at.xz()))
}

/// A radio not on `player` to go and get: the nearest in its own remains,
/// else in anyone's, else lying loose; not one gone for `RADIO_TRIES`
/// times already (`tried`) and not got. Where it is, to go to.
/// How long clear of a fight before it stops for a dressing by its way (s).
const CLEAR_TO_PICK: f32 = 15.0;
/// How far off its way a dressing's worth the steps (m).
const BY_THE_WAY: f32 = 6.0;
/// The nearest of `found` within [`BY_THE_WAY`] of `here` that it hasn't
/// tried and a hand gets to (`reach`): a thing it passes, not one it goes
/// looking for.
fn by_the_way(
    here: Vec2,
    found: impl Iterator<Item = (Entity, Vec3)>,
    tried: &[Entity],
    reach: impl Fn(Vec3) -> bool,
) -> Option<(Entity, Vec3)> {
    found
        .filter(|(e, at)| !tried.contains(e) && at.xz().distance(here) < BY_THE_WAY && reach(*at))
        .min_by(|a, b| a.1.xz().distance(here).total_cmp(&b.1.xz().distance(here)))
}

fn radio_to_fetch(
    world: &World2,
    plans: &[crate::sim::indoor::BuildingPlan],
    player: Entity,
    here: Vec2,
    tried: &[Entity],
) -> Option<(Entity, Vec3)> {
    // (One a standing hand gets from the floor under it: the nearest on
    // Marrow from the gate was up in the ranger tower's cab, and the
    // wanderer stood under it, then under it again, then went for one
    // 328 m off and was killed on the way.)
    let is_radio = |e: Entity| {
        tries_at(tried, e) < RADIO_TRIES
            && world.items.get(e).is_ok_and(|(_, it, gt, ..)| {
                world.catalog.get(it.kind()).id == "radio"
                    && in_reach(world, plans, gt.translation())
            })
    };
    let own = |mine: bool| {
        world
            .remains_bags
            .iter()
            .filter(|(_, _, r)| (r.owner == player) == mine)
            .flat_map(|(gt, bag, _)| {
                let at = gt.translation();
                bag.contents
                    .iter()
                    .copied()
                    .filter(|e| is_radio(*e))
                    .map(move |e| ((e, at), at.xz()))
            })
            .collect::<Vec<_>>()
    };
    let loose = || {
        world
            .items
            .iter()
            .filter(|(e, _, _, held, sock, bag)| {
                held.is_none() && sock.is_none() && bag.is_none() && is_radio(*e)
            })
            .map(|(e, _, gt, ..)| ((e, gt.translation()), gt.translation().xz()))
            .collect::<Vec<_>>()
    };
    pick_radio(here, [own(true), own(false), loose()]).map(|(t, _)| t)
}

/// The nearest of the first of `tiers` with any in it (its own remains,
/// anyone's, loose): a radio of its own 300 m off before a stranger's 30 m
/// off, as a person goes back to where they fell for their kit.
fn pick_radio<T: Copy>(here: Vec2, tiers: [Vec<(T, Vec2)>; 3]) -> Option<(T, Vec2)> {
    tiers.into_iter().find_map(|c| {
        c.into_iter()
            .min_by(|a, b| a.1.distance(here).total_cmp(&b.1.distance(here)))
    })
}

/// Goes after a radio off its body in one raid before giving the call up.
const RADIO_FETCHES: usize = 4;
/// Goes after any one radio (a walk that timed out on the way is planned
/// again from where it stood; one out of reach is left after the second).
const RADIO_TRIES: usize = 2;

fn tries_at(tried: &[Entity], e: Entity) -> usize {
    tried.iter().filter(|t| **t == e).count()
}

/// Whether a hand gets to `at` from the floor it can walk under it: a
/// building's (its plan's, upper floors and all where it has them), else
/// the ground.
fn in_reach(world: &World2, plans: &[crate::sim::indoor::BuildingPlan], at: Vec3) -> bool {
    match plans
        .iter()
        .find(|pl| pl.contains(at, 0.3) && (on_foot(world, pl) || stairs_up(pl, at).is_some()))
    {
        Some(plan) => {
            (on_foot(world, plan) && plan.reachable_height(at)) || stairs_up(plan, at).is_some()
        }
        None => over_floor(at.y, world.zone.on_ground(at.x, at.z).y),
    }
}

/// Whether a body walks onto `plan`'s floor from the ground: it has
/// floors of its own, stairs and all, or a way in at the ground's own
/// height (steps up to a door, a sill). (The ranger tower's plan is its
/// cab, its floor at the cab's door 6 m up: by it, the radio in the cab
/// was within reach of the floor, and the wanderer stood under it.)
fn on_foot(world: &World2, plan: &crate::sim::indoor::BuildingPlan) -> bool {
    plan.levels.is_some()
        || plan
            .doors
            .iter()
            .filter_map(|d| d.outside)
            .chain(plan.entries.iter().copied())
            .any(|o| {
                let w = plan.to_world(o);
                (w.y - world.zone.on_ground(w.x, w.z).y).abs() <= DOOR_STEPS
            })
}

/// The most a building's floor stands over the ground at its way in and
/// is still walked onto (m): a flight of steps to a door.
const DOOR_STEPS: f32 = 1.5;

/// The way up by `plan`'s own outside stairs to the floor `at` lies on
/// (a tower's cab or its open platform), world points from the foot on
/// the ground to the head: one whose head is a standing hand's reach under
/// `at`, near it.
fn stairs_up(plan: &crate::sim::indoor::BuildingPlan, at: Vec3) -> Option<Vec<Vec3>> {
    let geo = crate::world::buildings::prefab(&plan.name)?;
    geo.stairs
        .iter()
        .map(|w| {
            w.iter()
                .map(|p| plan.xf.transform_point(*p))
                .collect::<Vec<_>>()
        })
        .find(|w| {
            w.last().is_some_and(|h| {
                (-STAIR_HEAD_Y..=crate::bot::STAND_REACH).contains(&(at.y - h.y))
                    && h.xz().distance(at.xz()) < STAIR_HEAD
            })
        })
}

/// How far across from a stair's head its floor's things lie (m): a
/// tower's cab, 4 m square, beside its top landing.
const STAIR_HEAD: f32 = 6.0;
/// How far under a stair's head its floor may lie (m): the cab's deck a
/// step over the landing, a thing on its boards.
const STAIR_HEAD_Y: f32 = 0.5;

/// The `within` of a leg up or down a building's outside stairs: walked as
/// it is (`routed_leg` leaves it be), since a stair's flights run under
/// the floor they climb to, in the building's footprint.
const ON_STAIRS: f32 = 0.35;

/// Up or down `stairs` (world points, foot first), leg by leg: each
/// landing in turn.
fn stair_legs<'a>(stairs: impl Iterator<Item = &'a Vec3>) -> impl Iterator<Item = Task> {
    stairs.map(|p| Task::WalkTo {
        to: p.xz(),
        within: ON_STAIRS,
    })
}

/// Back down from `p` where it's up a building's outside stairs (a radio
/// got from a tower's cab): out of its floor's rooms to the stairs' head,
/// and down them to the ground. None where it isn't.
fn way_down(world: &World2, plans: &[crate::sim::indoor::BuildingPlan], p: Vec3) -> Vec<Task> {
    let Some((plan, stairs)) = plans
        .iter()
        .filter(|pl| pl.contains(p, 0.3))
        .find_map(|pl| Some((pl, stairs_up(pl, p)?)))
    else {
        return vec![];
    };
    // Out of a cab's rooms to its door first (an open platform: straight
    // to the stairs' head).
    let mut t = match out_of(plan, plan.to_local(p).xz()).filter(|_| !on_foot(world, plan)) {
        // To its door, not the stride out past it (off the landing's edge).
        Some((mut t, o)) => {
            if matches!(t.last(), Some(Task::WalkTo { to, .. }) if to.distance(o) < 0.1) {
                t.pop();
            }
            t
        }
        None => vec![],
    };
    t.extend(stair_legs(stairs.iter().rev()));
    // Off the foot of them, a stride out: on the ground, walked as any.
    if let Some(Task::WalkTo { within, .. }) = t.last_mut() {
        *within = 0.6;
    }
    t
}

/// `y` within a standing hand's reach over a floor at `floor`.
fn over_floor(y: f32, floor: f32) -> bool {
    y - floor <= crate::bot::STAND_REACH
}

/// How far off a thing set down at a resume still counts as at its feet (m).
const FEET_REACH: f32 = 3.0;

/// Something on `player` to load a gun from, besides what's in it: a
/// loaded box magazine, a stripper clip with rounds on it, or loose rounds
/// (box magazines only, and the careful newcomer with an SKS and two full
/// clips read as having nothing to load).
fn spare_loaded(world: &World2, view: &RaidView, player: Entity) -> bool {
    world.items.iter().any(|(e, it, ..)| {
        let def = world.catalog.get(it.kind());
        (def.magazine.is_some() || def.category == extraction_core::items::Category::Ammo)
            && it.rounds > 0
            && carried(world, view, player, &def.id).contains(&e)
    })
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
fn route(world: &World2, view: &RaidView, from: Vec2, to: Vec2) -> Vec<Task> {
    let mut t = vec![];
    let mut at = from;
    // (A way round a sentry by the map's edge stays on the map.)
    let edge = world.zone.map.size * 0.5 - 12.0;
    for p in round_sentries(&view.sentries, from, to, 3) {
        let p = if p == to {
            p
        } else {
            p.clamp(Vec2::splat(-edge), Vec2::splat(edge))
        };
        t.extend(walk(world, at, p));
        at = p;
    }
    t
}

/// How far out of a sentry's way a walk keeps once it's chirped: past its
/// sight, and a margin.
const SENTRY_BERTH: f32 = extraction_core::sentry::SIGHT + 5.0;

/// `a`..`b` kept out of the sight of each sentry in `sentries` (it doesn't
/// come after you; it has whoever crosses its sweep): by a point out on its
/// circle, on the side the way passes, as far as `depth` deep: out of its
/// sight first from in it, and in to `b` from its edge. The points
/// after `a`, to `b`. (Home past one that had chirped, the newcomer
/// walked back into its sweep, back under the roof, and out again, the
/// whole raid, worn down a round at a time.)
fn round_sentries(sentries: &[Vec2], a: Vec2, b: Vec2, depth: u32) -> Vec<Vec2> {
    let ab = b - a;
    let near = |s: Vec2| {
        let t = ((s - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        a + ab * t
    };
    let crossed = sentries
        .iter()
        .copied()
        .find(|s| near(*s).distance(*s) < SENTRY_BERTH - 1.0);
    let (Some(s), true) = (crossed, depth > 0) else {
        return vec![b];
    };
    let on_circle = |p: Vec2| s + (p - s).normalize_or(Vec2::X) * SENTRY_BERTH;
    // In its sight already: straight out of it first ("Down, out of its
    // sweep"), then on.
    if a.distance(s) < SENTRY_BERTH - 1.0 {
        let out = on_circle(a);
        let mut pts = vec![out];
        pts.extend(round_sentries(sentries, out, b, depth - 1));
        return pts;
    }
    // Going somewhere in it: round to the nearest of its edge, then in.
    if b.distance(s) < SENTRY_BERTH - 1.0 {
        let mut pts = round_sentries(sentries, a, on_circle(b), depth - 1);
        pts.push(b);
        return pts;
    }
    let side = (near(s) - s)
        .try_normalize()
        .unwrap_or_else(|| ab.perp().normalize_or(Vec2::X));
    // (A fifth further out: the stretches either side of it are chords,
    // and dip back in at the berth itself.)
    let w = s + side * SENTRY_BERTH * 1.2;
    let mut out = round_sentries(sentries, a, w, depth - 1);
    out.extend(round_sentries(sentries, w, b, depth - 1));
    out
}

/// A `WalkTo` this tight is a stop the walk must make on its way, not a
/// leg's end (m): round to a truck's back and up its ramp, a doorway's
/// front and back.
const PRECISE_STOP: f32 = 0.5;

/// The walk queued after the first `n` tasks (the run of `WalkTo`s next),
/// planned again from `from` by `route`: what it was set on before a
/// sentry chirped walked back across its sweep. Planned to the run's
/// first precise stop ([`PRECISE_STOP`]) and kept as it was from there:
/// to the end of the run, a walk round to a truck's back, up its ramp and
/// on to the cord came out as a walk straight at the cord, into the
/// truck's side (Marrow's rail yard, from the north: stood there till the
/// walk's half minute ran out).
/// The valley the route card shows (`route`: "NEXT DRIVE\nMARROW
/// CREEK\n(ring the bell to go)"), when it's another valley's and the job
/// card (`card`) says its job is in it ("... In Marrow Creek ...").
fn card_drives_to(card: &str, route: &str) -> Option<String> {
    let mut lines = route.lines();
    let name = lines.nth(1)?.trim();
    if name.is_empty() || !lines.any(|l| l.contains("ring the bell")) {
        return None;
    }
    let card = card
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase();
    card.contains(&format!("IN {name}"))
        .then(|| name.to_owned())
}

/// Whether `at` stands over the truck (`truck`'s pose): in its bed, where
/// the way off is the tailgate.
fn in_truck_bed(truck: Option<&Transform>, at: Vec2) -> bool {
    truck.is_some_and(|t| {
        let l =
            t.compute_affine()
                .inverse()
                .transform_point3(Vec3::new(at.x, t.translation.y, at.y));
        l.x.abs() < 1.3 && (-5.0..2.5).contains(&l.z)
    })
}

fn reroute_after(bot: &mut Bot, n: usize, from: Vec2, world: &World2, view: &RaidView) {
    let Some((end, dest, within)) = reroute_span(&bot.tasks, n) else {
        return;
    };
    let mut t = route(world, view, from, dest);
    match t.last_mut() {
        Some(Task::WalkTo { within: w, .. }) => *w = within,
        _ => t.push(Task::WalkTo { to: dest, within }),
    }
    let rest: Vec<Task> = bot.tasks.drain(n..).skip(end - n).collect();
    bot.tasks.extend(t);
    bot.tasks.extend(rest);
}

/// The part of the walk queued after the first `n` tasks that
/// [`reroute_after`] plans again: the end of it (as an index) and where
/// and how near it ends, at the first precise stop or the run's end.
fn reroute_span(tasks: &std::collections::VecDeque<Task>, n: usize) -> Option<(usize, Vec2, f32)> {
    let mut end = n;
    let mut last = None;
    while let Some(Task::WalkTo { to, within }) = tasks.get(end) {
        last = Some((*to, *within));
        end += 1;
        if *within <= PRECISE_STOP {
            break;
        }
    }
    last.map(|(to, within)| (end, to, within))
}

/// The way on foot outdoors from `from` to `to`, as `legs`: round the
/// buildings and solids in the way, through the fence's gaps, round the
/// banks (`route`'s; the second hour's walked straight, into a garden's
/// fence and the wire).
pub(super) fn walk(world: &World2, from: Vec2, to: Vec2) -> Vec<Task> {
    let mut at = from;
    let mut pts: Vec<Vec2> = vec![];
    let boxes = walk_boxes(world);
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
        // The grid's waypoints and a bank's sidestep know nothing of
        // buildings: each out of any it lands in (bar the end), and each
        // stretch to one round them (from against the pump house's wall,
        // a grid waypoint past it was walked to straight through it).
        for q in way {
            for r in round_banks(&world.zone, at, q, 3) {
                pts.push(r);
                at = r;
            }
        }
        at = p;
    }
    // A bridge's deck only by its ends: the grid's flat, and joined the
    // gully under Marrow's Forest path footbridge to the deck over it; the
    // newcomer walked the gully floor to the deck's far end and stood under
    // its abutment, 2 m below it, till the walk gave up (Map design).
    let decks: Vec<(crate::world::bridges::Deck, [Vec2; 2])> =
        crate::world::bridges::decks(&world.zone)
            .into_iter()
            .map(|d| {
                let [a, b] = d.ground_ends(&world.zone);
                (d, [a.xz(), b.xz()])
            })
            .collect();
    let pts = by_deck_ends(&decks, from, pts);
    // The grid's waypoints and a bank's sidestep know nothing of buildings:
    // each out of any it lands in (bar the end), and each stretch round any
    // in its way (from by the pump house, a waypoint past it was walked to
    // straight through it, and two newcomers stood at its wall).
    let mut way = vec![];
    let mut at = from;
    // From inside a box (pressed against what it stands for: a log pile's
    // box is square round round logs), out of it to its nearest side
    // first: from in it, every stretch out "hit" it at once, no way was
    // found, and the walk went straight on through it again (the one-
    // handed newcomer, twice by the Old farm's pile).
    let out = clear_of(&boxes, from);
    if out != from {
        way.push(out);
        at = out;
    }
    for r in pts {
        let r = if r == to { r } else { clear_of(&boxes, r) };
        for q in round_boxes(&boxes, at, r) {
            way.push(q);
            at = q;
        }
    }
    // Then off the decks' sides, the way as it'll be walked: round a house
    // by the District bridge's north end, the way cut the deck's corner
    // (Map design's wreck repro), where the waypoints before it hadn't.
    let way = round_deck_sides(&decks, from, way, &|p: Vec2| world.zone.ground(p.x, p.y));
    let mut t = vec![];
    let mut at = from;
    for q in way {
        t.extend(world.legs(at, q));
        at = q;
    }
    t
}

/// The bridges: decks joined end to end (a bridge over a bend is two or
/// more) as one, each with its outer ends (those joined to no other deck):
/// where it meets the ground, and a point out from that along the deck's
/// line, to line up from (a footbridge is 1.6 m wide: come at its end on
/// the slant and the first step is off its side, into the gully under
/// it). Per deck, its bridge's index; per bridge, its ends as (ground end,
/// lined-up approach).
#[allow(clippy::type_complexity)]
fn bridges_of(
    decks: &[(crate::world::bridges::Deck, [Vec2; 2])],
) -> (Vec<usize>, Vec<Vec<(Vec2, Vec2)>>) {
    let n = decks.len();
    let ends = |i: usize| [decks[i].0.a, decks[i].0.b];
    let joined = |i: usize, k: usize| {
        (0..n).any(|j| j != i && ends(j).iter().any(|e| e.distance(ends(i)[k]) < DECK_JOIN))
    };
    // Bridge of each deck: joined ones share the lowest index's.
    let mut of: Vec<usize> = (0..n).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..n {
            for j in 0..n {
                let touch = ends(i)
                    .iter()
                    .any(|a| ends(j).iter().any(|b| a.distance(*b) < DECK_JOIN));
                if i != j && touch && of[j] < of[i] {
                    of[i] = of[j];
                    changed = true;
                }
            }
        }
    }
    let mut outer: Vec<Vec<(Vec2, Vec2)>> = vec![vec![]; n];
    for i in 0..n {
        for k in 0..2 {
            if !joined(i, k) {
                let out = (ends(i)[k] - ends(i)[1 - k]).normalize_or(Vec2::X);
                let ground = decks[i].1[k];
                outer[of[i]].push((ground, ground + out * DECK_LINE_UP));
            }
        }
    }
    (of, outer)
}

/// How far out from a bridge's end it lines up to step on (m).
const DECK_LINE_UP: f32 = 3.0;

/// Deck ends this near one another are one bridge's (m).
const DECK_JOIN: f32 = 1.5;

/// `pts`, walked from `from`, on and off each bridge only at its ends (its
/// outer decks' ground ends, `bridges_of`): a point on a bridge reached
/// from off it goes by the end nearer where it comes from, and one off it
/// reached from the bridge by the end nearer where it's going. Along a
/// bridge, deck to deck over a bend, straight on; a walk under one, never
/// on it, is left as it is.
fn by_deck_ends(
    decks: &[(crate::world::bridges::Deck, [Vec2; 2])],
    from: Vec2,
    pts: Vec<Vec2>,
) -> Vec<Vec2> {
    let (of, ends) = bridges_of(decks);
    let on = |p: Vec2| {
        decks
            .iter()
            .position(|(d, _)| d.holds(p, 0.0))
            .map(|i| of[i])
    };
    let nearer = |b: usize, p: Vec2| {
        ends[b]
            .iter()
            .copied()
            .min_by(|x, y| x.0.distance(p).total_cmp(&y.0.distance(p)))
    };
    let mut out = Vec::with_capacity(pts.len() + 2);
    let mut at = from;
    for p in pts {
        let (was, is) = (on(at), on(p));
        if was != is {
            // Off one bridge (by the end toward where it's going), then on
            // another (by the end toward where it comes from).
            // (Off: its end, then out along its line; on: lined up
            // first, then its end.)
            if let Some((e, line)) = was.and_then(|b| nearer(b, p))
                && e.distance(at) > DECK_END_NEAR
                && e.distance(p) > DECK_END_NEAR
            {
                out.push(e);
                out.push(line);
                at = line;
            }
            if let Some((e, line)) = is.and_then(|b| nearer(b, at))
                && e.distance(at) > DECK_END_NEAR
                && e.distance(p) > DECK_END_NEAR
            {
                if line.distance(at) > DECK_END_NEAR {
                    out.push(line);
                }
                out.push(e);
            }
        }
        out.push(p);
        at = p;
    }
    out
}

/// A waypoint this near a deck's end is at it already (m).
const DECK_END_NEAR: f32 = 1.0;

/// How far out from a doorway, either side, a walk through it lines up
/// and comes out (m).
const DOOR_SQUARE: f32 = 0.8;
/// A point this near a doorway's axis (its middle, square to it) is on
/// its line already (m): more than a stop's `DOOR_STOP_WITHIN`, so a bot
/// at its stop is on the line and isn't sent back to it.
const DOOR_ON_LINE: f32 = 0.45;
/// How near its stops in front of and behind a doorway the walk comes (m).
const DOOR_STOP_WITHIN: f32 = 0.3;
/// A leg from this far off a doorway's middle isn't one walking through it
/// (m): a way through a door starts by it (the plan's stop before a door,
/// or a step aside), and a straight line from across a site that happens
/// to cross a doorway (a walk aimed at the surface from below it) is no
/// way through it to square.
const DOOR_NEAR: f32 = 6.0;

/// A doorway as a walk meets it: its closed leaf's line, hinge to latch
/// (on the map), the height of its sill, its door, and whether that's
/// shut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Doorway {
    pub a: Vec2,
    pub b: Vec2,
    pub y: f32,
    pub door: Entity,
    pub shut: bool,
}

/// A stop on a walk through doorways: a place, or a shut door to open
/// from where the walk stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum DoorStop {
    At(Vec2),
    Open(Entity),
}

/// Every door's doorway, from its leaf's closed line in its building.
fn doorways(view: &RaidView) -> Vec<Doorway> {
    view.leaves
        .iter()
        .filter_map(|(door, leaf, parent)| {
            let (_, _, bxf) = view.buildings.get(parent.parent()).ok()?;
            let d = &leaf.0;
            let a = bxf.transform_point(d.hinge);
            let axis = bxf.rotation * Quat::from_rotation_y(d.yaw) * Vec3::X;
            let b = a + axis * d.width;
            Some(Doorway {
                a: a.xz(),
                b: b.xz(),
                y: a.y,
                door,
                shut: view.door_state.get(door).is_ok_and(|s| !s.open),
            })
        })
        .collect()
}

/// Where the walk `p`..`q` crosses the segment `a`..`b`, as a fraction of
/// the walk (strictly between its ends).
fn crosses_at(p: Vec2, q: Vec2, a: Vec2, b: Vec2) -> Option<f32> {
    let r = q - p;
    let s = b - a;
    let den = r.perp_dot(s);
    if den.abs() < 1e-6 {
        return None;
    }
    let t = (a - p).perp_dot(s) / den;
    let u = (a - p).perp_dot(r) / den;
    (t > 1e-3 && t < 1.0 - 1e-3 && (-0.05..=1.05).contains(&u)).then_some(t)
}

/// The walk `from`, then each of `pts`, through any doorway square on its
/// line: lined up in front of its middle, through, and out square behind
/// it, as a person goes through a door. A leg cutting a doorway's corner
/// went by its hinge into the pocket behind the open leaf, and stood
/// there the rest of the raid (the shelter on Marrow, cautious, seed 3
/// job 2: door 1465, 0.6 m from its hinge, 3,200 frames). The points
/// after `from`, with the stops put in; a leg already on a doorway's line
/// is left as it is.
#[cfg(test)]
pub(super) fn through_doorways(doors: &[Doorway], from: Vec2, y: f32, pts: &[Vec2]) -> Vec<Vec2> {
    door_stops(doors, from, y, pts)
        .into_iter()
        .filter_map(|s| match s {
            DoorStop::At(p) => Some(p),
            DoorStop::Open(_) => None,
        })
        .collect()
}

/// `through_doorways`, with a shut door opened from in front of it before
/// the walk goes through (the fallback way in to Sallow's pump station
/// opened its front door only; lined up on the office's shut door, the
/// newcomer walked into it for the rest of the raid, the gun oil on the
/// table behind it).
pub(super) fn door_stops(doors: &[Doorway], from: Vec2, y: f32, pts: &[Vec2]) -> Vec<DoorStop> {
    let mut out = Vec::with_capacity(pts.len());
    let mut at = from;
    for &p in pts {
        let mut leg_from = at;
        // (More than one doorway on a leg: each in turn, nearest first.)
        for _ in 0..4 {
            let Some(d) = doors
                .iter()
                .filter(|d| (d.y - y).abs() < 2.0)
                .filter(|d| leg_from.distance((d.a + d.b) * 0.5) <= DOOR_NEAR)
                .filter_map(|d| crosses_at(leg_from, p, d.a, d.b).map(|t| (t, *d)))
                .min_by(|x, y| x.0.total_cmp(&y.0))
                .map(|(_, d)| d)
            else {
                break;
            };
            let w = d.b - d.a;
            let along = w.normalize_or(Vec2::X);
            let n = along.perp();
            let mid = (d.a + d.b) * 0.5;
            let on = |q: Vec2| (q - mid).dot(along).abs() < DOOR_ON_LINE;
            let side = (leg_from - mid).dot(n).signum();
            let (front, back) = (mid + n * side * DOOR_SQUARE, mid - n * side * DOOR_SQUARE);
            if !on(leg_from) {
                out.push(DoorStop::At(front));
            }
            if d.shut {
                out.push(DoorStop::Open(d.door));
            }
            if !on(p) {
                out.push(DoorStop::At(back));
                leg_from = back;
            } else {
                break;
            }
        }
        out.push(DoorStop::At(p));
        at = p;
    }
    out
}

/// After the newcomer's director: every walk in its queue through a
/// doorway square on its line (`through_doorways`), the ones an answer cut
/// in among them too (a step aside, then on, went through a doorway by
/// its hinge).
pub(super) fn square_through_doorways(
    opts: Res<LaunchOptions>,
    mut bots: Query<(&mut Bot, &Transform), With<LocalPlayer>>,
    view: RaidView,
) {
    if opts.scenario.as_deref() != Some(RAID) {
        return;
    }
    let Some((mut bot, root)) = bots.iter_mut().next() else {
        return;
    };
    if !bot.tasks.iter().any(|t| matches!(t, Task::WalkTo { .. })) {
        return;
    }
    let doors = doorways(&view);
    if doors.is_empty() {
        return;
    }
    let y = root.translation.y;
    let mut at = root.translation.xz();
    let mut tasks = std::collections::VecDeque::with_capacity(bot.tasks.len() + 4);
    let mut changed_front = false;
    let mut changed = false;
    for (i, t) in bot.tasks.iter().enumerate() {
        if let Task::WalkTo { to, .. } = t {
            let way = door_stops(&doors, at, y, &[*to]);
            for stop in &way[..way.len() - 1] {
                let task = match *stop {
                    DoorStop::At(p) => Task::WalkTo {
                        to: p,
                        within: DOOR_STOP_WITHIN,
                    },
                    // (Not again where it's opened just before: the
                    // plan's own way through it, or this one's.)
                    DoorStop::Open(d) => {
                        if matches!(tasks.back(), Some(Task::OpenDoor(e)) if *e == d) {
                            continue;
                        }
                        Task::OpenDoor(d)
                    }
                };
                tasks.push_back(task);
                changed = true;
                changed_front |= i == 0;
            }
            at = *to;
        }
        tasks.push_back(t.clone());
    }
    if changed {
        bot.tasks = tasks;
        if changed_front {
            bot.restart();
        }
    }
}

/// A deck this little above the ground under it is one a walk steps up
/// onto (and into its railing), not one it passes under (m).
const DECK_STEP_UP: f32 = 1.4;
/// How far out from a deck's side a walk round its end keeps (m).
const DECK_SIDE_CLEAR: f32 = 1.2;

/// Whether the straight walk `a`..`b` meets a deck's side: over its
/// footprint (a capsule's width either side) where the deck is within a
/// step of the ground, so the walk goes up on it and into its railing
/// (from the road north of the District bridge, a leg to the bank west of
/// it cut the deck's corner, and the newcomer stood against the railing
/// for the rest of the raid). Under a deck over its gully: clear.
fn meets_deck_side(
    decks: &[(crate::world::bridges::Deck, [Vec2; 2])],
    a: Vec2,
    b: Vec2,
    ground: &dyn Fn(Vec2) -> f32,
) -> bool {
    let n = (a.distance(b) / 0.25).ceil().max(1.0) as usize;
    (0..=n).map(|k| a.lerp(b, k as f32 / n as f32)).any(|q| {
        decks
            .iter()
            .any(|(d, _)| d.holds(q, 0.35) && d.y_at(q) - ground(q) < DECK_STEP_UP)
    })
}

/// The walk's legs from off every bridge to off every bridge kept off
/// their decks' sides (`meets_deck_side`): round the outside of the
/// nearer end of the deck in the way, by its line-up point or beside its
/// ground end, whichever clears and is shortest. On and off a bridge by
/// its ends (`by_deck_ends`) is left as it is, and so is a leg no way
/// round clears.
fn round_deck_sides(
    decks: &[(crate::world::bridges::Deck, [Vec2; 2])],
    from: Vec2,
    pts: Vec<Vec2>,
    ground: &dyn Fn(Vec2) -> f32,
) -> Vec<Vec2> {
    if decks.is_empty() {
        return pts;
    }
    let (of, ends) = bridges_of(decks);
    let off = |p: Vec2| !decks.iter().any(|(d, _)| d.holds(p, 0.0));
    // The legs `by_deck_ends` lays to an end from its line-up point (and
    // back): along the deck's line, and on or off by the end.
    let is_end_leg = |a: Vec2, b: Vec2| {
        ends.iter().flatten().any(|(e, l)| {
            (a.distance(*e) < 0.05 && b.distance(*l) < 0.05)
                || (a.distance(*l) < 0.05 && b.distance(*e) < 0.05)
        })
    };
    let meets = |a: Vec2, b: Vec2, i: usize| meets_deck_side(&decks[i..=i], a, b, ground);
    // Ways round bridge `k`: each outer end's line-up point, and beside its
    // ground end and its line-up point, either side, clear of the railing.
    let round_of = |k: usize| -> Vec<Vec2> {
        let mut r = Vec::new();
        for (e, l) in &ends[k] {
            let Some((d, _)) = decks
                .iter()
                .enumerate()
                .filter(|(i, _)| of[*i] == k)
                .map(|(_, d)| d)
                .min_by(|x, y| {
                    x.0.end_near(*e)
                        .distance(*e)
                        .total_cmp(&y.0.end_near(*e).distance(*e))
                })
            else {
                continue;
            };
            let side = (d.b - d.a).perp().normalize_or(Vec2::X) * (d.width * 0.5 + DECK_SIDE_CLEAR);
            let out = (*l - *e).normalize_or(Vec2::X);
            r.push(*l);
            for s in [side, -side] {
                r.push(*e + out + s);
                r.push(*l + s);
            }
        }
        r
    };
    let mut out = Vec::with_capacity(pts.len());
    let mut at = from;
    for p in pts {
        let hit: Vec<usize> = (0..decks.len()).filter(|i| meets(at, p, *i)).collect();
        if off(at) && off(p) && !is_end_leg(at, p) && !hit.is_empty() {
            let clear = |x: Vec2, y: Vec2| !meets_deck_side(decks, x, y, ground);
            let mut round: Vec<Vec2> = Vec::new();
            for i in &hit {
                round.extend(round_of(of[*i]));
            }
            round.dedup_by(|a, b| a.distance(*b) < 0.05);
            // One stop round, else two (round the end, then along the
            // side): the shortest that clears.
            let one = round
                .iter()
                .filter(|c| clear(at, **c) && clear(**c, p))
                .map(|c| (at.distance(*c) + c.distance(p), vec![*c]));
            let two = round.iter().flat_map(|c| {
                round
                    .iter()
                    .filter(move |k| k.distance(*c) > 0.05)
                    .map(move |k| (*c, *k))
            });
            let two = two
                .filter(|(c, k)| clear(at, *c) && clear(*c, *k) && clear(*k, p))
                .map(|(c, k)| (at.distance(c) + c.distance(k) + k.distance(p), vec![c, k]));
            if let Some((_, via)) = one.chain(two).min_by(|x, y| x.0.total_cmp(&y.0)) {
                out.extend(via);
            }
        }
        out.push(p);
        at = p;
    }
    out
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

/// The plan of the underground site the bot stands in at `at` (its feet,
/// at their real height), if it's in one: the ground over a shelter isn't
/// in it, so a test by the ground's height (as indoors on the surface)
/// never finds it.
fn below_in<'a>(
    zone: &crate::world::Zone,
    plans: &'a [crate::sim::indoor::BuildingPlan],
    at: Vec3,
) -> Option<&'a crate::sim::indoor::BuildingPlan> {
    // Below the ground, not on it: the site's plan has its kiosk up top and
    // the step outside its door too, and stood there, out of the shelter
    // with the watch, the newcomer was sent back in through the door by its
    // way out, and out again, for the rest of the raid.
    if at.y > zone.ground(at.x, at.z) - BELOW_GROUND {
        return None;
    }
    let p = at + Vec3::Y * 1.0;
    plans
        .iter()
        .find(|pl| pl.levels.is_some() && pl.contains(p, 0.3))
}

/// Feet this far under the ground over them are below it, in a site (m).
const BELOW_GROUND: f32 = 1.5;

/// Whether `to` (world, on the map) is on one of `plan`'s floors, at any
/// level: a walk there from inside it stays in it.
fn on_its_floors(plan: &crate::sim::indoor::BuildingPlan, to: Vec2) -> bool {
    let l = plan.to_local(Vec3::new(to.x, plan.xf.translation.y, to.y));
    let Some((i, j)) = plan.grid.cell_of([l.x, l.z]) else {
        return false;
    };
    let k = j * plan.grid.n + i;
    !plan.outside[k]
        && plan
            .levels
            .as_ref()
            .is_none_or(|lv| lv.get(k).is_some_and(|y| !y.is_nan()))
}

/// As [`way_out`], from the bot's feet at their real height: below ground
/// in a site, up through its rooms and out by its way to the surface
/// (in the shelter under the ranger tower with the truck waiting overhead,
/// the walk to it went straight at the truck through the rock, and it
/// pressed for the bell from under the ground the rest of the raid).
fn way_out_at(
    world: &World2,
    view: &RaidView,
    plans: &[crate::sim::indoor::BuildingPlan],
    at: Vec3,
) -> (Vec<Task>, Vec2) {
    if let Some(plan) = below_in(&world.zone, plans, at)
        && let Some(out) = out_of(plan, plan.to_local(at + Vec3::Y * 1.0).xz())
    {
        return out;
    }
    way_out(world, view, plans, at.xz())
}

/// Through `plan`'s rooms from `from` (its space) to the nearest of its
/// ways outside: the walk, and where it leaves the bot (world).
fn out_of(plan: &crate::sim::indoor::BuildingPlan, from: Vec2) -> Option<(Vec<Task>, Vec2)> {
    ways_out(plan, from)
        .into_iter()
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(t, o, _)| (t, o))
}

/// Through `plan`'s rooms from `from` (its space) to each of its ways
/// outside that a path reaches: the walk, where it leaves the bot (world),
/// and how far it is.
fn ways_out(plan: &crate::sim::indoor::BuildingPlan, from: Vec2) -> Vec<(Vec<Task>, Vec2, f32)> {
    plan.doors
        .iter()
        .filter_map(|d| d.outside)
        .filter_map(|o| {
            let path = plan.path(from, o)?;
            let len = path_len(from, &path);
            Some((walk_out(plan, from, &path), plan.world2(o), len))
        })
        .collect()
}

/// Through `plan`'s rooms from `from` (its space) to just outside it all
/// round, by any way out a body walks through (a door, or an open hall's
/// bay: its plan's doors are only those with a leaf, and the rail shed's
/// one door was on the walker's side): the walk, where it leaves the bot
/// (world), and how far it is.
fn open_ways_out(
    plan: &crate::sim::indoor::BuildingPlan,
    from: Vec2,
) -> Vec<(Vec<Task>, Vec2, f32)> {
    let edge = plan.half + Vec2::splat(OUT_PAST);
    let along = |a: Vec2, b: Vec2| {
        let n = (a.distance(b) / OUT_EVERY).ceil().max(1.0) as usize;
        (0..n).map(move |k| a.lerp(b, k as f32 / n as f32))
    };
    let corners = [
        Vec2::new(-edge.x, -edge.y),
        Vec2::new(edge.x, -edge.y),
        Vec2::new(edge.x, edge.y),
        Vec2::new(-edge.x, edge.y),
    ];
    (0..4)
        .flat_map(|k| along(corners[k], corners[(k + 1) % 4]))
        .filter(|p| {
            plan.grid
                .cell_of(p.to_array())
                .is_some_and(|(i, j)| plan.outside[j * plan.grid.n + i])
        })
        .filter_map(|p| {
            let path = plan.path(from, p)?;
            let len = path_len(from, &path);
            Some((walk_out(plan, from, &path), plan.world2(p), len))
        })
        .collect()
}

/// A walk out of a building whose plan's way is no more than this longer
/// than straight is a straight one out through an opening (m).
const STRAIGHT_OUT: f32 = 0.5;

/// How far past a building's walls a way out of it comes out (m).
const OUT_PAST: f32 = 1.5;
/// How far apart the spots round it are looked at (m).
const OUT_EVERY: f32 = 2.0;

/// The walk through `plan`'s rooms from `from` along `path` (its space).
fn walk_out(plan: &crate::sim::indoor::BuildingPlan, from: Vec2, path: &[Vec2]) -> Vec<Task> {
    let mut t = Vec::new();
    // Down a site, in legs short enough to walk inside a task's time (up
    // the shelter's kiosk stair, 10 m at a climb, the one leg timed out,
    // and the door at its head was pressed from the foot of it).
    let deep = plan.levels.is_some();
    let mut at = plan.world2(from);
    let mut walk_to = |t: &mut Vec<Task>, to: Vec2| {
        if deep {
            let n = (at.distance(to) / DEEP_LEG).ceil().max(1.0) as usize;
            for k in 1..n {
                t.push(Task::WalkTo {
                    to: at.lerp(to, k as f32 / n as f32),
                    within: 0.6,
                });
            }
        }
        t.push(Task::WalkTo { to, within: 0.4 });
        at = to;
    };
    for stop in indoor_stops(plan, from, path) {
        match stop {
            Stop::Door(door, before) => {
                walk_to(&mut t, plan.world2(before));
                t.push(Task::OpenDoor(door));
            }
            Stop::At(w) => walk_to(&mut t, plan.world2(w)),
        }
    }
    if let Some(Task::WalkTo { within, .. }) = t.last_mut() {
        *within = 0.6;
    }
    t
}

/// A step back from `here` along `away` inside `plan`: 4 m if its floors
/// go that far that way and a path gets there, else 3, else 2, by the
/// plan's rooms; none if no step back is to be had (a step back from an
/// anomaly "checks behind it": in a room's corner, a wall).
fn step_back_in(plan: &crate::sim::indoor::BuildingPlan, here: Vec2, away: Vec2) -> Vec<Task> {
    if away == Vec2::ZERO {
        return vec![];
    }
    let from = plan
        .to_local(Vec3::new(here.x, plan.xf.translation.y, here.y))
        .xz();
    [4.0f32, 3.0, 2.0]
        .into_iter()
        .map(|d| here + away * d)
        .filter(|p| on_its_floors(plan, *p))
        .find_map(|p| {
            let to = plan
                .to_local(Vec3::new(p.x, plan.xf.translation.y, p.y))
                .xz();
            let path = plan.path(from, to)?;
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
                        within: 0.6,
                    }),
                }
            }
            Some(t)
        })
        .unwrap_or_default()
}

/// The longest leg of a way out of a site (m): a stair's climb walks at a
/// quarter of the pace, and a task's time is 30 s.
const DEEP_LEG: f32 = 4.0;

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
    if let Some(plan) = plans.iter().find(|pl| pl.contains(at, 0.3))
        && let Some(out) = out_of(plan, plan.to_local(at).xz())
    {
        return out;
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
            // Up its stairs where it's a floor only they get to (a tower's
            // cab: routed to under its door, the newcomer stood on the
            // ground under the radio in it).
            match stairs_up(plan, p).filter(|_| !on_foot(world, plan)) {
                // From the stairs' head straight to its door (the plan's
                // way in is a stride out from it, past the landing's edge:
                // walked to, the newcomer stepped off the tower).
                Some(up) => {
                    t.extend(route(world, view, here, up[0].xz()));
                    t.extend(stair_legs(up[1..].iter()));
                }
                None => t.extend(route(world, view, here, plan.world2(o))),
            }
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
    // Up an open platform's stairs (a survey tower's: its plan is the
    // ground round its legs), and across it to the thing.
    if let Some(up) = plans
        .iter()
        .filter(|pl| pl.contains(p, 0.3))
        .find_map(|pl| stairs_up(pl, p))
    {
        t.extend(route(world, view, here, up[0].xz()));
        t.extend(stair_legs(up[1..].iter()));
        // (To beside it: it lies on a table.)
        t.push(Task::WalkTo {
            to: p.xz(),
            within: 0.9,
        });
        return t;
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
            t.extend(way_down(world, plans, p));
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
/// In a valley with no Old farm, on the way to the place the job card
/// names (`card`), so the machine is on the newcomer's way.
/// The loose `item` nearest the drop: where a newcomer on a fetch goes first.
fn fetch_target(world: &World2, zone: &crate::world::Zone, item: &str) -> Option<Vec2> {
    let drop = zone
        .map
        .markers
        .iter()
        .find(|m| m.kind == MarkerKind::Insertion)
        .map(|m| Vec2::from(m.at))?;
    world
        .items
        .iter()
        .filter(|(e, _, _, held, sock, bag)| {
            held.is_none() && sock.is_none() && bag.is_none() && world.id(*e) == Some(item)
        })
        .map(|(_, _, gt, ..)| gt.translation().xz())
        .min_by(|a, b| a.distance(drop).total_cmp(&b.distance(drop)))
}

fn on_the_way_to(zone: &crate::world::Zone, card: Option<Vec2>) -> (Vec2, Vec2) {
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
        .map(|m| Vec2::from(m.at))
        .or(card)
        .unwrap_or(drop + Vec2::new(100.0, -100.0));
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
    cards: Query<&ScreenText, With<RequestCard>>,
    progression: Option<Res<crate::progression::Progression>>,
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
        // The place the card names; a fetch names none, so the nearest of
        // what it asks for (off where the newcomer isn't going, Rime's first
        // machine met nobody, and the director's walker had them instead).
        let card = cards
            .iter()
            .find_map(|c| place_on_card(&c.text, &zone))
            .map(|(_, at)| at)
            .or_else(|| {
                let crate::progression::Progression(p) = progression.as_deref()?;
                let Some(extraction_core::progression::Job::Fetch { item, .. }) = p.job.as_ref()
                else {
                    return None;
                };
                fetch_target(&world, &zone, item)
            });
        let (at, ahead) = on_the_way_to(&zone, card);
        info!("first fight: {kind} placed at {at}, on the way to {card:?}");
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
            // The melee machines, on their rounds off the way as the walker
            // is: they close to strike, so how near they are when you first
            // know is the fight.
            // As the director sends one: a response team to where the
            // newcomer was heard, on its short job, so having lost them it
            // searches, gives up and goes home through the Gate (a hound
            // is never on a watch post).
            "hound" => {
                let p = at + side * 20.0;
                let site = zone.on_ground(p.x, p.y);
                let to = zone.on_ground(at.x, at.y);
                let response = crate::robots::Job {
                    kind: crate::robots::JobKind::Response,
                    site: to,
                    duration: 40.0,
                    exit: crate::robots::director::gate(&zone),
                };
                crate::robots::hound::spawn_hound(&mut commands, &zone, site, response);
                p
            }
            "heavy" => {
                let p = at + side * 25.0;
                let site = zone.on_ground(p.x, p.y);
                crate::robots::heavy::spawn_heavy(
                    &mut commands,
                    &zone,
                    &world.catalog,
                    site,
                    job(site),
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
        info!(
            "first fight: a {kind} at {placed:.0} for the {}, on the way to {}",
            p.name,
            card.map_or("the Old farm or a guess".to_string(), |c| format!("{c:.0}"))
        );
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
    // (None before its machine is out: a crawler on its round noticing a
    // deer 0.2 s in, with the kit still in the crate, had every reckless
    // newcomer draw a gun it hadn't got, 13 runs of 40.)
    if !fight.heeds_tells() {
        came.clear();
    }
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
    // A heavy: "Walk away from it. Don't hide."
    let heavy = kind == "heavy" && !p.reckless && !p.hoards && !hush;
    let answer: Vec<Task> = if (p.reckless || broken) && !hush {
        take_it_on()
    } else if heavy {
        fight.flee_until = 0.0;
        walk_off_heavy(&world, here, from)
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
    /// A job in another valley: the route card turned till it shows that
    /// valley, then the bell (Kettle raid 3, the post counter "In Marrow
    /// Creek": no place on Kettle's map, and it froze).
    #[test]
    fn a_card_in_another_valley_drives_there() {
        let card = "CONTRACT: THE POST COUNTER -\n1 OF 1\nFor the Holm boy.\nThe post\noffice.\nIn Marrow\nCreek, by the bridge.";
        let go = "NEXT DRIVE\nMARROW CREEK\n(ring the bell to go)";
        assert_eq!(card_drives_to(card, go).as_deref(), Some("MARROW CREEK"));
        let here = "NEXT DRIVE\nMARROW CREEK\n(here again)";
        assert_eq!(card_drives_to(card, here), None, "already there");
        let other = "NEXT DRIVE\nCINDER WORKS\n(ring the bell to go)";
        assert_eq!(card_drives_to(card, other), None, "not the card's");
        assert_eq!(card_drives_to("", go), None);
    }

    /// A body over the truck is in its bed (its way off is the tailgate).
    #[test]
    fn the_bed_is_told_from_the_ground_round_the_truck() {
        let t = Transform::from_xyz(90.9, 14.3, 232.4).with_rotation(Quat::from_rotation_y(0.7));
        let at = |x: f32, z: f32| t.transform_point(Vec3::new(x, 0.0, z)).xz();
        assert!(in_truck_bed(Some(&t), at(0.3, 0.6)), "where a raid starts");
        assert!(in_truck_bed(Some(&t), at(0.8, 1.5)), "by the crate");
        assert!(!in_truck_bed(Some(&t), at(0.0, 4.0)), "behind the tailgate");
        assert!(!in_truck_bed(Some(&t), at(2.5, 0.0)), "beside it");
        assert!(!in_truck_bed(None, at(0.3, 0.6)), "no truck");
    }

    /// Planned again after a cut-in, a walk to a truck keeps its way
    /// round to the back and up the ramp: only the way to the first
    /// precise stop is planned again (to the run's end, it was a walk
    /// straight at the bell cord, into the truck's side).
    #[test]
    fn a_rerouted_walk_keeps_its_precise_stops() {
        let w = |x: f32, within: f32| Task::WalkTo {
            to: Vec2::new(x, 0.0),
            within,
        };
        let tasks: std::collections::VecDeque<Task> = vec![
            Task::Wait(12.0),
            w(10.0, 2.0),
            w(20.0, 1.0),
            w(24.0, 0.5),
            w(25.0, 0.3),
            w(27.0, 0.3),
            Task::Wait(1.0),
        ]
        .into();
        let (end, to, within) = reroute_span(&tasks, 1).expect("a walk");
        assert_eq!((end, to, within), (4, Vec2::new(24.0, 0.0), 0.5));
        // A walk of legs alone: to its end, as before.
        let legs: std::collections::VecDeque<Task> =
            vec![w(10.0, 2.0), w(20.0, 1.0), Task::Wait(1.0)].into();
        assert_eq!(reroute_span(&legs, 0), Some((2, Vec2::new(20.0, 0.0), 1.0)));
        assert_eq!(reroute_span(&legs, 2), None);
    }

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

    /// Home past a sentry that's chirped, the way keeps out of its sight:
    /// every stretch clear of it by its berth (the first fight's: the lost
    /// one walked back across its sweep the whole raid). One the newcomer is
    /// going to is gone to.
    #[test]
    fn the_way_home_keeps_out_of_a_sentrys_sight() {
        let s = Vec2::new(67.0, 138.0);
        let (a, b) = (Vec2::new(106.0, 78.0), Vec2::new(10.0, 190.0));
        let pts = round_sentries(&[s], a, b, 3);
        assert_eq!(pts.last(), Some(&b));
        let mut at = a;
        for p in &pts {
            let ab = *p - at;
            let t = ((s - at).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            let d = (at + ab * t).distance(s);
            assert!(
                d > SENTRY_BERTH - 1.5,
                "{at} -> {p}: {d:.1} m from it ({pts:?})"
            );
            at = *p;
        }
        // To somewhere in its sight: round to its edge, then in. From in
        // it: straight out first.
        let inside = s + Vec2::new(-30.0, 10.0);
        let to_it = round_sentries(&[s], a, inside, 3);
        assert_eq!(to_it.last(), Some(&inside));
        assert!(to_it.len() >= 2, "{to_it:?}");
        let from_it = round_sentries(&[s], inside, b, 3);
        assert!(
            (from_it[0].distance(s) - SENTRY_BERTH).abs() < 0.1,
            "{from_it:?}"
        );
        assert_eq!(round_sentries(&[], a, b, 3), vec![b]);
    }

    /// The bot's PM against a drone held still, slightly above it, at 10,
    /// 20 and 33 m: hits out of eight. A measurement (the coordinator's
    /// question: seven rounds a raid at a locked-on drone, none hit), not
    /// a gate: `cargo test ... the_bots_pm_at_a_still_drone -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore]
    fn the_bots_pm_at_a_still_drone() {
        use avian3d::prelude::{AngularVelocity, LinearVelocity};
        let drop = |app: &mut App, id: &str, at: Vec3, rounds: Option<u32>| {
            let catalog = app.world().resource::<crate::items::Catalog>().clone();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut c = Commands::new(&mut queue, app.world());
                crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: catalog.kind(id).unwrap(),
                        transform: Transform::from_translation(at),
                        condition: 1.0,
                    },
                )
            };
            queue.apply(app.world_mut());
            if let Some(n) = rounds {
                let mut i = app.world_mut().get_mut::<crate::items::Item>(e).unwrap();
                i.rounds = n;
                i.loads.fit(n);
            }
            e
        };
        for d in [10.0f32, 20.0, 33.0] {
            let opts = crate::launch::LaunchOptions {
                robots: false,
                map: "marrow_creek".into(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            #[derive(Resource, Default)]
            struct Struck(Vec<(String, Vec3)>);
            fn struck(
                mut hits: MessageReader<crate::weapons::ballistics::Hit>,
                names: Query<&Name>,
                parents: Query<&ChildOf>,
                mut out: ResMut<Struck>,
            ) {
                for h in hits.read() {
                    let mut e = h.entity;
                    let mut name = format!("{e:?}");
                    for _ in 0..4 {
                        if let Ok(n) = names.get(e) {
                            name = n.to_string();
                            break;
                        }
                        match parents.get(e) {
                            Ok(c) => e = c.parent(),
                            Err(_) => break,
                        }
                    }
                    out.0.push((name, h.point));
                }
            }
            app.init_resource::<Struck>().add_systems(Update, struck);
            app.finish();
            app.cleanup();
            for _ in 0..10 {
                app.update();
            }
            app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
            let p = app
                .world_mut()
                .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
                .single(app.world())
                .unwrap();
            // Out of the truck (in it, every round hit its wall): in the
            // open inside the south gate, the drone to the north.
            let zone = app.world().resource::<crate::world::Zone>().clone();
            let me = zone.on_ground(22.0, 140.0) + Vec3::Y * 0.05;
            let fwd = Vec3::NEG_Z;
            {
                let mut t = app.world_mut().get_mut::<Transform>(p).unwrap();
                t.translation = me;
                t.rotation = Quat::IDENTITY;
            }
            for _ in 0..10 {
                app.update();
            }
            let gun = drop(&mut app, "pm_pistol", me + Vec3::new(0.3, 0.9, 0.0), None);
            let mag = drop(&mut app, "pm_mag", me + Vec3::new(-0.3, 0.9, 0.0), Some(8));
            let at = me + fwd * d + Vec3::Y * 6.0;
            let drone = {
                let zone = app.world().resource::<crate::world::Zone>().clone();
                let mut queue = bevy::ecs::world::CommandQueue::default();
                let e = {
                    let mut c = Commands::new(&mut queue, app.world());
                    let site = zone.on_ground(at.x, at.z);
                    crate::robots::drone::spawn_drone(
                        &mut c,
                        at,
                        crate::robots::Job {
                            kind: crate::robots::JobKind::Watch,
                            site,
                            duration: f32::INFINITY,
                            exit: site,
                        },
                    )
                };
                queue.apply(app.world_mut());
                e
            };
            app.update();
            app.world_mut()
                .entity_mut(p)
                .insert(crate::bot::Bot::new(vec![
                    crate::bot::Task::Grab {
                        item: mag,
                        side: crate::rig::Side::Left,
                    },
                    crate::bot::Task::Stow(crate::rig::Side::Left),
                    crate::bot::Task::Grab {
                        item: gun,
                        side: crate::rig::Side::Right,
                    },
                    crate::bot::Task::Ready { off_safe: true },
                    crate::bot::Task::Engage { shots: 8 },
                ]));
            for _ in 0..90 * 25 {
                if let Ok(mut e) = app.world_mut().get_entity_mut(drone) {
                    if let Some(mut t) = e.get_mut::<Transform>() {
                        t.translation = at;
                        t.rotation = Quat::IDENTITY;
                    }
                    if let Some(mut v) = e.get_mut::<LinearVelocity>() {
                        v.0 = Vec3::ZERO;
                    }
                    if let Some(mut v) = e.get_mut::<AngularVelocity>() {
                        v.0 = Vec3::ZERO;
                    }
                    if let Some(mut dr) = e.get_mut::<crate::robots::drone::Drone>() {
                        dr.gun_cooldown = 99.0;
                    }
                }
                app.update();
                if app.world().get::<crate::bot::Bot>(p).unwrap().done() {
                    break;
                }
            }
            let eye = me + Vec3::Y * 1.6;
            println!(
                "AIM {d} m: drone at {at:.1}, eye {eye:.1}; struck {:?}",
                app.world()
                    .resource::<Struck>()
                    .0
                    .iter()
                    .map(|(n, p)| format!("{n} @ {p:.1}"))
                    .collect::<Vec<_>>()
            );
            let bot = app.world().get::<crate::bot::Bot>(p).unwrap();
            println!(
                "AIM {d} m (slant {:.1} m): fired {}, hits {}, robot hits {}, log {:?}",
                me.distance(at),
                bot.fired,
                bot.hits,
                bot.robot_hits,
                bot.log
                    .iter()
                    .filter(|l| l.contains("FAIL"))
                    .collect::<Vec<_>>()
            );
        }
    }

    /// A job in another valley isn't gone after here: its place is that
    /// valley's (a Marrow job's place, walked to inside Cinder, stuck the
    /// newcomer in a corner of the works). One with no valley goes anywhere.
    #[test]
    fn a_job_in_another_valley_isnt_gone_after_here() {
        for job in one_of_each() {
            assert_eq!(
                errand_here(&job, "marrow_creek"),
                Some(errand(&job)),
                "{job:?}"
            );
            assert_eq!(
                errand_here(&job, "cinder_works").is_some(),
                job.map().is_none(),
                "{job:?}"
            );
        }
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

    /// A deck point is reached by the deck's own ends: from the gully under
    /// it (by the end nearer), from either end (straight on), and off it to
    /// the ground below (by the end nearer there). A walk under the bridge,
    /// never on it, is left alone.
    #[test]
    fn a_bridge_deck_is_walked_on_and_off_only_at_its_ends() {
        use crate::world::bridges::Deck;
        let deck = Deck {
            a: Vec2::new(0.0, 0.0),
            b: Vec2::new(0.0, 20.0),
            ya: 18.0,
            yb: 18.0,
            width: 2.0,
        };
        let ends = [Vec2::new(0.0, -2.0), Vec2::new(0.0, 22.0)];
        let decks = [(deck, ends)];
        // Lined up out along its line from each end.
        let line = [
            Vec2::new(0.0, -2.0 - DECK_LINE_UP),
            Vec2::new(0.0, 22.0 + DECK_LINE_UP),
        ];
        let mid = Vec2::new(0.0, 12.0);
        // Up from the gully east of it, nearer the far end: lined up, on.
        let gully = Vec2::new(8.0, 16.0);
        assert_eq!(
            by_deck_ends(&decks, gully, vec![mid]),
            vec![line[1], ends[1], mid]
        );
        // Along it from either end: straight on.
        assert_eq!(by_deck_ends(&decks, ends[0], vec![mid]), vec![mid]);
        assert_eq!(by_deck_ends(&decks, ends[1], vec![mid]), vec![mid]);
        // Off it, down to the gully by the near end, out along its line.
        let below = Vec2::new(6.0, 3.0);
        assert_eq!(
            by_deck_ends(&decks, mid, vec![below]),
            vec![ends[0], line[0], below]
        );
        // Under it, east to west, never on it: as it was.
        let (e, w) = (Vec2::new(6.0, 10.0), Vec2::new(-6.0, 10.0));
        assert_eq!(by_deck_ends(&decks, e, vec![w]), vec![w]);
    }

    /// A crawler's charge: shot back only with a round to fire (an empty
    /// gun's "shot it" was an Engage that ended at once, and the careful
    /// newcomer stood through eight zaps in the shelter's generator room);
    /// its gun out of the pack if that's the loaded one; else out of its
    /// line. And while one's at it, no keeping still and no dressing.
    /// Where a newcomer with no radio could find one, valley by valley:
    /// each loot spot whose table can roll one, on the ground or up a
    /// floor (a tower's cab), and whether any is on the ground. Run with
    /// `--ignored --nocapture` for the list.
    #[test]
    #[ignore = "a report: the radio spots in every valley"]
    fn radio_spots_in_every_valley() {
        use extraction_core::loot::{BUILTIN_LOOT, LootTables};
        let catalog =
            extraction_core::items::ItemCatalog::from_ron(extraction_core::items::BUILTIN_CATALOG)
                .unwrap();
        let loot = LootTables::from_ron(BUILTIN_LOOT, &catalog).unwrap();
        let mut none_low = Vec::new();
        for (name, text) in extraction_core::map::VALLEYS {
            let map = extraction_core::map::MapDef::from_ron(text).unwrap();
            let (mut low, mut high) = (Vec::new(), Vec::new());
            for pl in &map.placements {
                let Some(geo) = crate::world::buildings::prefab(&pl.prefab) else {
                    continue;
                };
                for spot in geo.loot.iter().filter(|s| loot.can_drop(s.table, "radio")) {
                    let floor = spot.at.y - spot.lift.unwrap_or(0.0);
                    let at = format!(
                        "{} at {:?} ({}, floor {:.1} m)",
                        pl.prefab, pl.at, spot.table, floor
                    );
                    if floor > super::DOOR_STEPS {
                        high.push(at);
                    } else {
                        low.push(at);
                    }
                }
            }
            println!(
                "== {name}: {} on the ground, {} up a floor",
                low.len(),
                high.len()
            );
            for a in &high {
                println!("   up:  {a}");
            }
            for a in &low {
                println!("   low: {a}");
            }
            if low.is_empty() {
                none_low.push(name);
            }
        }
        println!("valleys with no radio spot on the ground: {none_low:?}");
    }

    /// A radio up in a tower's cab, over the ground floor it stands on, is
    /// none to go for; one on a shelf is.
    #[test]
    fn a_radio_overhead_is_none_to_go_for() {
        use super::over_floor;
        // The ranger tower's cab, 6 m over the ground at its foot.
        assert!(!over_floor(26.0, 20.0));
        // A shelf's top, a table, the floor.
        assert!(over_floor(21.8, 20.0));
        assert!(over_floor(20.9, 20.0));
        assert!(over_floor(20.0, 20.0));
        // Just past a raised hand.
        assert!(!over_floor(22.3, 20.0));
    }

    /// Hunted: the same walker aiming at it again inside 20 s (from where
    /// it last aimed); one aim, aims from two walkers, or a second one
    /// after the window, aren't a hunt.
    #[test]
    fn a_hunt_is_the_same_walker_again_inside_its_window() {
        use super::{HUNT_WINDOW, hunted};
        let e = |i: u32| Entity::from_raw_u32(i).unwrap();
        let at = |x: f32| Vec3::new(x, 2.0, 0.0);
        assert_eq!(hunted(&[], 10.0), None);
        assert_eq!(hunted(&[(e(1), at(1.0), 0.0)], 1.0), None);
        assert_eq!(
            hunted(&[(e(1), at(1.0), 0.0), (e(2), at(2.0), 3.0)], 4.0),
            None
        );
        assert_eq!(
            hunted(&[(e(1), at(1.0), 0.0), (e(1), at(5.0), 3.0)], 4.0),
            Some(at(5.0))
        );
        // The first aim gone stale: one in the window.
        assert_eq!(
            hunted(
                &[(e(1), at(1.0), 0.0), (e(1), at(5.0), 3.0)],
                HUNT_WINDOW + 1.0
            ),
            None
        );
    }

    /// A dressing it passes is picked up: the nearest a hand gets to, a few
    /// steps off its way, each once; none it'd have to go looking for.
    #[test]
    fn a_dressing_by_its_way_is_picked_up_once() {
        use super::{BY_THE_WAY, by_the_way};
        let e = |i: u32| Entity::from_raw_u32(i).unwrap();
        let here = Vec2::ZERO;
        let found = [
            (e(1), Vec3::new(4.0, 0.0, 0.0)),
            (e(2), Vec3::new(0.0, 0.0, 2.5)),
            // Over a raised hand (a shelf up a tower).
            (e(3), Vec3::new(1.0, 6.0, 0.0)),
            (e(4), Vec3::new(BY_THE_WAY + 1.0, 0.0, 0.0)),
        ];
        let reach = |at: Vec3| at.y < 2.0;
        let pick = |tried: &[Entity]| by_the_way(here, found.into_iter(), tried, reach);
        assert_eq!(pick(&[]).map(|p| p.0), Some(e(2)));
        assert_eq!(pick(&[e(2)]).map(|p| p.0), Some(e(1)));
        assert_eq!(pick(&[e(1), e(2)]), None);
    }

    #[test]
    fn with_no_radio_on_it_it_goes_for_the_nearest_it_can_get() {
        use super::pick_radio;
        let here = Vec2::ZERO;
        let at = |x: f32| Vec2::new(x, 0.0);
        // Its own remains first, however far.
        assert_eq!(
            pick_radio(
                here,
                [
                    vec![(1, at(300.0)), (2, at(130.0))],
                    vec![(3, at(20.0))],
                    vec![(4, at(5.0))]
                ]
            ),
            Some((2, at(130.0)))
        );
        // Then anyone's, then loose.
        assert_eq!(
            pick_radio(here, [vec![], vec![(3, at(60.0))], vec![(4, at(5.0))]]),
            Some((3, at(60.0)))
        );
        assert_eq!(
            pick_radio(here, [vec![], vec![], vec![(4, at(50.0)), (5, at(-9.0))]]),
            Some((5, at(-9.0)))
        );
        // None anywhere: the call is given up (the only time).
        assert_eq!(pick_radio::<u8>(here, [vec![], vec![], vec![]]), None);
    }

    #[test]
    fn a_crawler_at_it_is_answered_with_what_it_has() {
        assert_eq!(crawler_answer(true, true, false), CrawlerAnswer::Shoot);
        assert_eq!(crawler_answer(true, false, false), CrawlerAnswer::BackOut);
        assert_eq!(crawler_answer(true, false, true), CrawlerAnswer::Unpack);
        assert_eq!(crawler_answer(false, false, true), CrawlerAnswer::Unpack);
        assert_eq!(crawler_answer(false, true, false), CrawlerAnswer::BackOut);
        assert_eq!(crawler_answer(false, false, false), CrawlerAnswer::BackOut);
        let now = 100.0;
        assert!(crawler_at_it(now, now - 1.0));
        assert!(crawler_at_it(now, now - CRAWLER_CLEAR + 0.1));
        assert!(!crawler_at_it(now, now - CRAWLER_CLEAR - 0.1));
        // (Never charged: the newcomer's clock starts it at zero.)
        assert!(!crawler_at_it(now, 0.0));
        // A wait to keep still in is one the charge clears.
        assert!(CRAWLER_STILL <= 5.0);
    }

    /// The shelter job (`--job shelter`) puts the card's thing down in
    /// Marrow's bunker, in a room the newcomer has to come up out of: a
    /// regression run for the walk home from below.
    #[test]
    fn it_waits_for_the_truck_where_raiders_do() {
        // Every rendezvous a survey drone can see has its wait point (the
        // same one `map_lint::wait_shelter` gives raiders); the rest none.
        let mut watched = 0;
        for name in [
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "rime_fell",
            "sallow_reservoir",
        ] {
            let def = crate::world::load_map_def(name);
            for m in def.markers_of(&MarkerKind::Rendezvous) {
                let rv = Vec2::from(m.at);
                let seen = extraction_core::map_lint::watched_by(&def, m.at).is_some();
                let at = wait_point(&def, rv);
                if !seen {
                    assert_eq!(at, None, "{name} {}: unwatched", m.name);
                    continue;
                }
                watched += 1;
                let at =
                    at.unwrap_or_else(|| panic!("{name} {}: watched, nowhere to wait", m.name));
                assert!(
                    at.distance(rv) <= extraction_core::map_lint::WAIT_COVER + 30.0,
                    "{name} {}: waits {:.0} m off",
                    m.name,
                    at.distance(rv)
                );
            }
        }
        assert!(watched > 0, "no watched rendezvous in any valley");
    }

    #[test]
    fn the_shelter_job_is_down_the_shelter() {
        let Some(Job::Bring { map, item, at, .. }) = shelter_job("marrow_creek") else {
            panic!("no shelter job on Marrow");
        };
        assert_eq!(
            (map.as_str(), item.as_str()),
            ("marrow_creek", "holm_watch")
        );
        let def = crate::world::load_map_def("marrow_creek");
        let (site, room) = extraction_core::contracts::down_site(&def, at)
            .unwrap_or_else(|| panic!("{at:?} isn't down a site"));
        assert!(
            extraction_core::underground::layout(&site).is_some_and(|l| l.lockdown.is_some()),
            "{site}: not the shelter"
        );
        assert_ne!(room, "lock", "the way in's own room");
        assert_eq!(
            seeded_job("shelter", "marrow_creek"),
            shelter_job("marrow_creek")
        );
        // Its place one the newcomer can find on the card (a POI's name).
        let Some(Job::Bring { place, .. }) = shelter_job("marrow_creek") else {
            unreachable!()
        };
        assert!(
            def.markers
                .iter()
                .any(|m| m.kind == MarkerKind::Poi && m.name == place),
            "{place}: no such place"
        );
    }

    /// Through a doorway square on its line, from anywhere round it: every
    /// leg that crosses its line does so on its axis, from a stop in front
    /// to one behind, never by its hinge (the shelter's door 1465: in by
    /// its hinge from a step aside, into the pocket behind the open leaf).
    /// A leg on its line already, and one that never crosses it, as they were.
    #[test]
    fn a_walk_goes_through_a_doorway_square() {
        let d = Doorway {
            a: Vec2::new(0.0, 0.0),
            b: Vec2::new(0.9, 0.0),
            y: 0.0,
            door: Entity::PLACEHOLDER,
            shut: false,
        };
        let doors = [d];
        let mid = Vec2::new(0.45, 0.0);
        let ring: Vec<Vec2> = (0..16)
            .map(|k| {
                let t = k as f32 / 16.0 * std::f32::consts::TAU;
                mid + Vec2::new(t.cos(), t.sin()) * 3.0
            })
            .collect();
        let on_axis = |q: Vec2| (q.x - mid.x).abs() < DOOR_ON_LINE;
        let mut crossings = 0;
        for a in &ring {
            for b in &ring {
                let way = through_doorways(&doors, *a, 0.0, &[*b]);
                assert_eq!(way.last(), Some(b));
                let mut at = *a;
                for p in &way {
                    if crosses_at(at, *p, d.a, d.b).is_some() {
                        crossings += 1;
                        assert!(
                            on_axis(at) && on_axis(*p),
                            "{a}..{b}: leg {at}..{p} crosses off the axis in {way:?}"
                        );
                    }
                    at = *p;
                }
                // Again from where it ends up: nothing more put in.
                let mut at = *a;
                let mut again = Vec::new();
                for p in &way {
                    again.extend(through_doorways(&doors, at, 0.0, &[*p]));
                    at = *p;
                }
                assert_eq!(again, way, "{a}..{b}: not settled");
            }
        }
        assert!(crossings > 0);
        // Square through already: as it was.
        let (front, back) = (Vec2::new(0.45, 1.0), Vec2::new(0.45, -1.0));
        assert_eq!(through_doorways(&doors, front, 0.0, &[back]), vec![back]);
        // Never across it: as it was.
        let (e, w) = (Vec2::new(3.0, 1.0), Vec2::new(-3.0, 1.0));
        assert_eq!(through_doorways(&doors, e, 0.0, &[w]), vec![w]);
        // From across the site, a straight line through it: as it was.
        let (far, beyond) = (Vec2::new(0.6, 15.0), Vec2::new(0.2, -15.0));
        assert!(crosses_at(far, beyond, d.a, d.b).is_some());
        assert_eq!(through_doorways(&doors, far, 0.0, &[beyond]), vec![beyond]);
        // A doorway on another floor: as it was.
        let (n, s) = (Vec2::new(0.1, 2.0), Vec2::new(0.1, -2.0));
        assert_eq!(through_doorways(&doors, n, 5.0, &[s]), vec![s]);
        // By its hinge from a step aside (the shelter's case): lined up
        // in front, through, out behind.
        // (Across its line 0.2 m from the hinge.)
        let (from, to) = (Vec2::new(-0.3, 1.0), Vec2::new(1.2, -2.0));
        assert!(crosses_at(from, to, d.a, d.b).is_some());
        assert_eq!(
            through_doorways(&doors, from, 0.0, &[to]),
            vec![
                Vec2::new(0.45, DOOR_SQUARE),
                Vec2::new(0.45, -DOOR_SQUARE),
                to
            ]
        );
    }

    /// A shut door on the walk's line is opened from in front of it before
    /// the walk goes through (Sallow's pump station: lined up on the
    /// office's shut door, the newcomer walked into it for the rest of the
    /// raid). An open one, and one on a leg that never crosses it, as they
    /// were.
    #[test]
    fn a_drone_finding_it_at_each_roof_pins_it() {
        // Found every 148 s coming out from under the same roof (seed 20's
        // wanderer): the third is pinned.
        let mut r = vec![];
        assert_eq!(found_again(&mut r, 299.0), 1);
        assert_eq!(found_again(&mut r, 448.0), 2);
        assert_eq!(found_again(&mut r, 596.0), PINNED_AFTER);
        // Every 23 s (the post office): pinned as soon.
        let mut r = vec![];
        for t in [0.0, 23.0] {
            found_again(&mut r, t);
        }
        assert_eq!(found_again(&mut r, 46.0), PINNED_AFTER);
        // Long apart: a new drone each time, not one that has it.
        let mut r = vec![];
        for t in [0.0, 300.0] {
            assert_eq!(found_again(&mut r, t), 1);
        }
    }

    #[test]
    fn a_shut_door_on_the_way_is_opened_first() {
        let door = Entity::from_raw_u32(7).unwrap();
        let d = Doorway {
            a: Vec2::new(0.0, 0.0),
            b: Vec2::new(0.9, 0.0),
            y: 0.0,
            door,
            shut: true,
        };
        let (from, to) = (Vec2::new(-0.3, 1.0), Vec2::new(1.2, -2.0));
        assert_eq!(
            door_stops(&[d], from, 0.0, &[to]),
            vec![
                DoorStop::At(Vec2::new(0.45, DOOR_SQUARE)),
                DoorStop::Open(door),
                DoorStop::At(Vec2::new(0.45, -DOOR_SQUARE)),
                DoorStop::At(to),
            ]
        );
        // Lined up already: opened where it stands, then through.
        let (front, back) = (Vec2::new(0.45, 1.0), Vec2::new(0.45, -1.0));
        assert_eq!(
            door_stops(&[d], front, 0.0, &[back]),
            vec![DoorStop::Open(door), DoorStop::At(back)]
        );
        // Open: through without stopping to press it.
        let open = Doorway { shut: false, ..d };
        assert_eq!(
            door_stops(&[open], front, 0.0, &[back]),
            vec![DoorStop::At(back)]
        );
        // Never across it: as it was.
        let (e, w) = (Vec2::new(3.0, 1.0), Vec2::new(-3.0, 1.0));
        assert_eq!(door_stops(&[d], e, 0.0, &[w]), vec![DoorStop::At(w)]);
    }

    /// A walk past a bridge's end, from off it to off it, keeps off its
    /// deck's side where the deck is level with the ground (into its
    /// railing): round the end. Under it, over its gully: as it was.
    #[test]
    fn from_inside_a_box_the_walk_steps_out_before_it_goes_round() {
        use super::{clear_of, round_boxes, segment_hits_box};
        // A log pile, turned as the Old farm's is, and a newcomer pressed
        // against its logs: inside its box. To beyond it.
        let pile = (
            Transform::from_xyz(0.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(-2.705)),
            Vec2::new(1.25, 3.5),
        );
        let boxes = [pile];
        let from = Vec2::new(-1.0, 0.2);
        let to = Vec2::new(60.0, -40.0);
        let through = |a: Vec2, b: Vec2| {
            let inv = pile.0.compute_affine().inverse();
            let la = inv.transform_point3(Vec3::new(a.x, 0.0, a.y)).xz();
            let lb = inv.transform_point3(Vec3::new(b.x, 0.0, b.y)).xz();
            segment_hits_box(la, lb, pile.1).is_some()
        };
        // From in it, there's no way round: straight on, through it.
        assert_eq!(round_boxes(&boxes, from, to), vec![to]);
        // And to a place inside a house beyond it: still round the pile.
        let house = (Transform::from_xyz(60.0, 0.0, -40.0), Vec2::new(4.0, 3.5));
        let both = [pile, house];
        let mut at = clear_of(&both, from);
        for p in round_boxes(&both, at, Vec2::new(60.0, -40.0)) {
            assert!(
                !through(at, p),
                "{at} -> {p} through the pile, to the house"
            );
            at = p;
        }
        // Out of it first (as `walk` now does), and round.
        let out = clear_of(&boxes, from);
        assert_ne!(out, from, "stepped out of the box");
        let mut at = out;
        for p in round_boxes(&boxes, out, to) {
            assert!(!through(at, p), "{at} -> {p} through the pile");
            at = p;
        }
    }

    /// Crouched on a walker's side of a log pile, the cover behind its far
    /// side is reached round the pile's end, not through it (Marrow's
    /// first fight: the pile at (8, 146.5), the newcomer at (10.8, 145.8),
    /// the walker 3.8 m east).
    #[test]
    fn cover_behind_what_it_crouches_by_is_reached_round_it() {
        use super::{nearest_where, segment_hits_box};
        let pile = (Vec2::new(8.0, 146.5), Vec2::new(1.25 + 0.3, 3.5 + 0.3));
        let through = |a: Vec2, b: Vec2| segment_hits_box(a - pile.0, b - pile.0, pile.1).is_some();
        let open = |a: Vec2, b: Vec2| !through(a, b);
        let here = Vec2::new(10.8, 145.8);
        let eye = Vec2::new(14.6, 145.0);
        // Hidden: the pile itself between it and the eye, and the eye a
        // step either side (`EYE_SWAY`), as `break_line` asks.
        let sight = (pile.0, Vec2::new(1.25, 3.5));
        let side = (here - eye).normalize().perp() * super::EYE_SWAY;
        let good = |p: Vec2| {
            [eye, eye + side, eye - side]
                .iter()
                .all(|e| segment_hits_box(*e - sight.0, p - sight.0, sight.1).is_some())
        };
        let away = (here - eye).normalize();
        let way = nearest_where(here, away, &open, &good).expect("cover round the pile");
        let end = *way.last().unwrap();
        assert!(end.x < 8.0 - 1.25, "behind the pile's far side: {way:?}");
        let mut at = here;
        for q in &way {
            assert!(!through(at, *q), "{at} -> {q} through the pile: {way:?}");
            at = *q;
        }
    }

    /// Told a walker's coming to look, in its view from where it saw it
    /// (the first sighting and the last, 20 m apart, as the shelter
    /// wanderer's): cover 8 m off, and it goes behind it from both, not
    /// to a spot hidden only from the last (where the wanderer held, and
    /// was found from the first).
    #[test]
    fn coming_to_look_it_hides_from_every_spot_it_was_seen_at() {
        use super::{nearest_where, segment_hits_box};
        // A shed, 4 x 3 m, just west; it crouches east of it.
        let shed = (Vec2::new(-3.0, 0.0), Vec2::new(2.0, 1.5));
        let through = |a: Vec2, b: Vec2| segment_hits_box(a - shed.0, b - shed.0, shed.1).is_some();
        let open = |a: Vec2, b: Vec2| !through(a, b);
        let here = Vec2::ZERO;
        // Where it was seen: first 15 m south-south-west, last 12 m west.
        let (first, last) = (Vec2::new(-6.0, -14.0), Vec2::new(-12.0, 0.0));
        let hidden_from = |e: Vec2, p: Vec2| through(e, p);
        // Out of the last's view where it is (so, held, as the wanderer
        // did), but in the first's (where the walker came to look from).
        assert!(hidden_from(last, here) && !hidden_from(first, here));
        // From both: it moves, to a spot neither sees.
        let both = |p: Vec2| hidden_from(first, p) && hidden_from(last, p);
        assert!(!both(here));
        let away = (here - last).normalize();
        let way = nearest_where(here, away, &open, &both).expect("cover from both");
        let end = *way.last().unwrap();
        assert!(both(end), "{end} out of both views: {way:?}");
        let mut at = here;
        for q in &way {
            assert!(!through(at, *q), "{at} -> {q} through the shed: {way:?}");
            at = *q;
        }
    }

    #[test]
    fn a_walk_through_an_open_hall_goes_round_what_stands_in_it() {
        use super::{part_boxes, round_boxes, segment_hits_box};
        // The sawmill turned as on Marrow, and a walk across it the long
        // way, over where its log stack lies (`buildings::sawmill`: six
        // logs along x, 5 m, at -9, from -2.25 to -0.45 across).
        let xf = Transform::from_xyz(-90.8, 0.0, 88.5)
            .with_rotation(Quat::from_rotation_y(90f32.to_radians()));
        let logs = (
            xf * Transform::from_xyz(-9.0, 0.0, -1.35),
            Vec2::new(2.5, 0.9),
        );
        let at = |x: f32, z: f32| xf.transform_point(Vec3::new(x, 0.0, z)).xz();
        let (from, to) = (at(-9.0, 3.0), at(-9.0, -4.5));
        let boxes = part_boxes("sawmill", &xf);
        let way = round_boxes(&boxes, from, to);
        let inv = logs.0.compute_affine().inverse();
        let mut p = from;
        for q in &way {
            let a = inv.transform_point3(Vec3::new(p.x, 0.0, p.y)).xz();
            let b = inv.transform_point3(Vec3::new(q.x, 0.0, q.y)).xz();
            assert!(
                segment_hits_box(a, b, logs.1).is_none(),
                "{p} -> {q} over the log stack: {way:?}"
            );
            p = *q;
        }
    }

    #[test]
    fn from_the_edge_of_a_box_margin_the_way_still_goes_round_the_next() {
        use super::{ROUND, round_boxes, segment_hits_box};
        // A tower, and a house far on past it: from just by the tower's
        // wall, on its margin's edge (where a way out of it ends), to beyond
        // the house.
        let tower = (Transform::from_xyz(0.0, 0.0, 0.0), Vec2::new(3.0, 2.5));
        let house = (Transform::from_xyz(-60.0, 0.0, -40.0), Vec2::new(4.0, 3.5));
        let boxes = [tower, house];
        let from = Vec2::new(3.0 + ROUND * 0.5, 1.2);
        let to = Vec2::new(-120.0, -79.0);
        let way = round_boxes(&boxes, from, to);
        assert!(
            way.len() > 1,
            "a way round the house, not straight: {way:?}"
        );
        let mut at = from;
        for p in &way {
            let (xf, half) = &house;
            let inv = xf.compute_affine().inverse();
            let a = inv.transform_point3(Vec3::new(at.x, 0.0, at.y)).xz();
            let b = inv.transform_point3(Vec3::new(p.x, 0.0, p.y)).xz();
            assert!(
                segment_hits_box(a, b, *half).is_none(),
                "{at} -> {p} through the house"
            );
            at = *p;
        }
    }

    #[test]
    fn a_walk_past_a_bridge_end_goes_round_it() {
        use crate::world::bridges::Deck;
        let deck = Deck {
            a: Vec2::new(0.0, 0.0),
            b: Vec2::new(0.0, 20.0),
            ya: 18.0,
            yb: 18.0,
            width: 7.0,
        };
        let decks = [(deck, [Vec2::new(0.0, 0.0), Vec2::new(0.0, 20.0)])];
        // Level with the deck by its ends; a gully 8 m down between.
        let ground = |p: Vec2| if p.y < 4.0 || p.y > 16.0 { 18.0 } else { 10.0 };
        // From the road north-east of its end to the bank west of it,
        // straight across its corner.
        let (from, to) = (Vec2::new(5.0, -4.0), Vec2::new(-6.0, 3.0));
        assert!(meets_deck_side(&decks, from, to, &ground));
        let way = round_deck_sides(&decks, from, vec![to], &ground);
        assert_eq!(way.last(), Some(&to));
        assert!(way.len() > 1, "{way:?}");
        let mut at = from;
        for p in &way {
            assert!(
                !meets_deck_side(&decks, at, *p, &ground),
                "{at}..{p} in {way:?}"
            );
            at = *p;
        }
        // Under it, east to west over the gully: as it was.
        let (e, w) = (Vec2::new(8.0, 10.0), Vec2::new(-8.0, 10.0));
        assert!(!meets_deck_side(&decks, e, w, &ground));
        assert_eq!(round_deck_sides(&decks, e, vec![w], &ground), vec![w]);
        // On and off by its end, along its line: as it was.
        assert_eq!(
            round_deck_sides(
                &decks,
                Vec2::new(0.0, -3.0),
                vec![Vec2::new(0.0, 0.0)],
                &ground
            ),
            vec![Vec2::new(0.0, 0.0)]
        );
    }

    /// Every bridge in every valley: a walk between any two points round
    /// its ends (off it) never goes up on a deck's side; and on Marrow, the
    /// leg past the District road bridge's north end that cut its corner
    /// (Map design's wreck repro) goes round.
    #[test]
    fn walks_past_every_bridge_end_keep_off_its_sides() {
        use crate::world::bridges;
        let mut bad = Vec::new();
        let mut legs = 0;
        for map in crate::test_maps() {
            let zone = crate::world::Zone::load(&map);
            let decks: Vec<_> = bridges::decks(&zone)
                .into_iter()
                .map(|d| {
                    let [a, b] = d.ground_ends(&zone);
                    (d, [a.xz(), b.xz()])
                })
                .collect();
            let ground = |p: Vec2| zone.ground(p.x, p.y);
            if map == "marrow_creek" {
                let (a, b) = (
                    Vec2::new(7.304625, -2.495706),
                    Vec2::new(-0.6695702, -12.451674),
                );
                assert!(
                    meets_deck_side(&decks, a, b, &ground),
                    "the District bridge leg should cut the deck's corner"
                );
                {
                    let way = round_deck_sides(&decks, a, vec![b], &ground);
                    let mut at = a;
                    for p in &way {
                        if meets_deck_side(&decks, at, *p, &ground) {
                            bad.push(format!("{map}: District bridge leg {at}..{p} in {way:?}"));
                        }
                        at = *p;
                    }
                }
            }
            let (_, ends) = bridges_of(&decks);
            for bridge in &ends {
                for (e, _) in bridge {
                    let ring: Vec<Vec2> = (0..12)
                        .map(|k| {
                            let t = k as f32 / 12.0 * std::f32::consts::TAU;
                            *e + Vec2::new(t.cos(), t.sin()) * 9.0
                        })
                        .filter(|p| !decks.iter().any(|(d, _)| d.holds(*p, 0.5)))
                        .collect();
                    for a in &ring {
                        for b in &ring {
                            if a == b {
                                continue;
                            }
                            legs += 1;
                            let way = round_deck_sides(&decks, *a, vec![*b], &ground);
                            let mut at = *a;
                            for p in &way {
                                if meets_deck_side(&decks, at, *p, &ground) {
                                    bad.push(format!("{map}: {a}..{b}: leg {at}..{p}"));
                                    break;
                                }
                                at = *p;
                            }
                        }
                    }
                }
            }
        }
        assert!(legs > 0);
        assert!(
            bad.is_empty(),
            "{} of {legs} walks past bridge ends on a deck's side:\n{}",
            bad.len(),
            bad.iter().take(30).cloned().collect::<Vec<_>>().join("\n")
        );
    }

    /// Every bridge in every valley: a deck's middle reached from the
    /// ground beside it goes by one of its ends, and from each end straight
    /// on along it.
    #[test]
    fn every_bridge_is_walked_onto_by_its_ends() {
        use crate::world::bridges;
        for map in crate::test_maps() {
            let zone = crate::world::Zone::load(&map);
            let decks: Vec<_> = bridges::decks(&zone)
                .into_iter()
                .map(|d| {
                    let [a, b] = d.ground_ends(&zone);
                    (d, [a.xz(), b.xz()])
                })
                .collect();
            let (of, ends) = bridges_of(&decks);
            for (i, (d, _)) in decks.iter().enumerate() {
                let outer = &ends[of[i]];
                assert!(
                    !outer.is_empty(),
                    "{map}: a bridge with no ends at {:?}",
                    d.a
                );
                let mid = (d.a + d.b) * 0.5;
                let side = (d.b - d.a).perp().normalize_or(Vec2::X) * (d.width + 6.0);
                // (Its middle at one of the bridge's ends already: straight on.)
                let at_end = outer.iter().any(|(e, _)| e.distance(mid) <= DECK_END_NEAR);
                for beside in [mid + side, mid - side] {
                    // (Beside it on another deck of the same bridge: along it.)
                    if decks
                        .iter()
                        .position(|(d, _)| d.holds(beside, 0.0))
                        .map(|j| of[j])
                        == Some(of[i])
                    {
                        continue;
                    }
                    let way = by_deck_ends(&decks, beside, vec![mid]);
                    // Lined up out on its line, then its end, then on.
                    let by_an_end = way.len() == 3
                        && outer.iter().any(|(e, l)| way[0] == *l && way[1] == *e)
                        && way[2] == mid;
                    assert!(
                        by_an_end || (at_end && way == vec![mid]),
                        "{map}: deck {:?}..{:?} from {beside:?}: {way:?}",
                        d.a,
                        d.b
                    );
                }
                for (end, _) in outer {
                    let way = by_deck_ends(&decks, *end, vec![mid]);
                    assert_eq!(way, vec![mid], "{map}: from its end {end:?}");
                }
            }
        }
    }

    #[test]
    fn a_careers_raids_go_to_the_valley_asked() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (rest, map) = raids_map(args(&[
            "--headless",
            "--map",
            "rime_fell",
            "--weather",
            "fog",
        ]));
        assert_eq!(map, "rime_fell");
        assert_eq!(rest, args(&["--headless", "--weather", "fog"]));
        let (rest, map) = raids_map(args(&["--headless"]));
        assert_eq!(map, "marrow_creek");
        assert_eq!(rest, args(&["--headless"]));
    }

    #[test]
    fn the_fight_waits_for_its_machine() {
        let mut fight = Fight::default();
        // Loading, the kit in the crate: another machine's tell isn't it.
        assert!(!fight.heeds_tells());
        fight.machine = Some(("walker".into(), Vec2::ZERO));
        assert!(fight.heeds_tells());
    }

    #[test]
    fn a_bleed_is_dressed_once_out_of_contact() {
        let now = 400.0;
        // Bleeding, nothing at it for a while: dress it.
        assert!(wound_to_dress(1.5, &[now - 10.0], now, 0.0, false));
        // Still being shot at, or running to cover: not yet.
        assert!(!wound_to_dress(1.5, &[now - 1.0], now, 0.0, false));
        assert!(!wound_to_dress(1.5, &[now - 10.0], now, 0.0, true));
        // A graze that's stopping: no.
        assert!(!wound_to_dress(0.1, &[], now, 0.0, false));
        // Just put one on: not again at once.
        assert!(!wound_to_dress(1.5, &[], now, now - 2.0, false));
        assert!(wound_to_dress(1.5, &[], now, now - 20.0, false));
    }

    #[test]
    fn shot_at_in_the_open_it_shoots_back() {
        let now = 300.0;
        let two = [now - 3.0, now - 1.0];
        // Aimed at, in the open, a round up, two rounds in: back at it.
        assert!(no_way_out(&two, now, now - 5.0, false, false, true));
        // Behind something, or on its way there: no.
        assert!(!no_way_out(&two, now, now - 5.0, true, false, true));
        assert!(!no_way_out(&two, now, now - 5.0, false, true, true));
        // Nothing up the spout: no (readying comes first).
        assert!(!no_way_out(&two, now, now - 5.0, false, false, false));
        // One round only, or long ago: not yet.
        assert!(!no_way_out(
            &[now - 1.0],
            now,
            now - 5.0,
            false,
            false,
            true
        ));
        assert!(!no_way_out(
            &[now - 9.0, now - 8.0],
            now,
            now - 5.0,
            false,
            false,
            true
        ));
        // No walker's aim on it lately: not this rule's.
        assert!(!no_way_out(&two, now, now - 60.0, false, false, true));
    }

    #[test]
    fn the_clock_gets_it_out_from_under_cover() {
        use crate::raid::{SURGE_AFTER, SURGE_WARNING, TRUCK_WAIT};
        let out = Phase::Out;
        let far = SURGE_AFTER - 600.0;
        let near = SURGE_WARNING + SURGE_MARGIN - 1.0;
        // Hours to the Surge: on with the job, hidden or not.
        for leg in [Leg::Out, Leg::Place, Leg::Call] {
            assert_eq!(time_short(&out, far, leg, true, false, false, 99.0), None);
        }
        // The Surge near, on the job: home, whether it's hiding or not
        // (told once a career, a second warning isn't said).
        for leg in [Leg::Out, Leg::Place] {
            for hiding in [false, true] {
                assert_eq!(
                    time_short(&out, near, leg, hiding, false, false, 99.0),
                    Some(Hurry::Surge),
                    "{leg:?}, hiding {hiding}"
                );
            }
        }
        // Calling already: only out from cover, and once.
        assert_eq!(
            time_short(&out, near, Leg::Call, true, false, false, 99.0),
            Some(Hurry::Surge)
        );
        assert_eq!(
            time_short(&out, near, Leg::Call, true, false, true, 99.0),
            None
        );
        assert_eq!(
            time_short(&out, near, Leg::Call, false, false, false, 99.0),
            None
        );
        // At the camp, in the truck, home: the clock isn't its business.
        for leg in [Leg::Camp, Leg::Kit, Leg::Paid, Leg::Done] {
            assert_eq!(time_short(&out, near, leg, true, false, false, 99.0), None);
        }
        // The truck there: a minute's grace, then out from cover to it.
        let just = Phase::TruckWaiting {
            rv: 0,
            left: TRUCK_WAIT - 10.0,
        };
        let running = Phase::TruckWaiting {
            rv: 0,
            left: TRUCK_WAIT - TRUCK_PATIENCE - 1.0,
        };
        for leg in [Leg::Call, Leg::Rendezvous, Leg::Board] {
            assert_eq!(time_short(&just, far, leg, true, false, false, 99.0), None);
            assert_eq!(
                time_short(&running, far, leg, true, false, false, 99.0),
                Some(Hurry::Truck),
                "{leg:?}"
            );
            // Not hiding: its walk to the truck is already the way.
            assert_eq!(
                time_short(&running, far, leg, false, false, false, 99.0),
                None
            );
            // A drone on it still: under the roof till it looks away.
            assert_eq!(
                time_short(&running, far, leg, true, true, false, 99.0),
                None
            );
            // Not every frame: once in 30 s.
            assert_eq!(time_short(&running, far, leg, true, false, true, 5.0), None);
        }
    }

    /// "Bleeding badly ... Get to the truck": off the job, the truck called
    /// from where it stands; the truck there, to it; already boarding or at
    /// the camp, nothing new.
    #[test]
    fn bleeding_badly_goes_for_the_truck() {
        let there = Phase::TruckWaiting { rv: 0, left: 60.0 };
        let called = Phase::TruckCalled { rv: 0, eta: 60.0 };
        for leg in [Leg::Out, Leg::Place] {
            assert_eq!(
                bleed_answer(leg, &Phase::Out),
                Some(BleedGo::Call),
                "{leg:?}"
            );
        }
        // Called, but still out (on its way back to the gate, or under a
        // roof): from here, now.
        assert_eq!(bleed_answer(Leg::Call, &Phase::Out), Some(BleedGo::Call));
        for leg in [Leg::Out, Leg::Place, Leg::Call, Leg::Rendezvous] {
            assert_eq!(bleed_answer(leg, &there), Some(BleedGo::ToTruck), "{leg:?}");
        }
        assert_eq!(bleed_answer(Leg::Rendezvous, &called), None);
        assert_eq!(bleed_answer(Leg::Board, &there), None);
        for leg in [Leg::Camp, Leg::Kit, Leg::Card, Leg::Paid, Leg::Done] {
            assert_eq!(bleed_answer(leg, &Phase::Out), None, "{leg:?}");
        }
    }

    /// Time to bleed out, against the game's own bleed stepped second by
    /// second (`health`: hp less the rate, the rate easing).
    #[test]
    fn bleeds_out_in_matches_the_bleed() {
        let step = |mut hp: f32, mut rate: f32| {
            let dt = 0.001;
            let mut t = 0.0;
            while rate > 0.0 && t < 1000.0 {
                hp -= rate * dt;
                rate = (rate - BLEED_EASES * dt).max(0.0);
                t += dt;
                if hp <= 0.0 {
                    return Some(t);
                }
            }
            None
        };
        for (hp, rate) in [
            (65.0, 1.5),
            (40.0, 1.1),
            (80.0, 3.0),
            (20.0, 1.0),
            (30.0, 1.0),
        ] {
            match (bleeds_out_in(hp, rate), step(hp, rate)) {
                (Some(a), Some(b)) => assert!((a - b).abs() < 0.1, "{hp} {rate}: {a} v {b}"),
                (a, b) => assert_eq!(a.is_some(), b.is_some(), "{hp} {rate}: {a:?} v {b:?}"),
            }
        }
        // 1 HP a second eases off after 25 HP: 30 is never bled away.
        assert_eq!(bleeds_out_in(30.0, 1.0), None);
        assert!(bleeds_out_in(20.0, 1.0).is_some());
        assert_eq!(bleeds_out_in(50.0, 0.0), None);
    }
}

#[cfg(test)]
mod indoor_tests {
    use super::{
        STEEP_RUN, Stop, below_in, indoor_stops, on_its_floors, out_of, round_banks, round_boxes,
        segment_hits_box, shot_warns, solid, steep_run, step_back_in,
    };
    use crate::bot::Task;
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

    /// From any room of any underground site, on every map, a walk off
    /// its floors (to the truck) goes up and out by the site's way first:
    /// the bot below ground is known to be in it at its feet's height (in
    /// the shelter under the ranger tower, the walk to the truck went
    /// straight at it through the rock, and it pressed for the bell from
    /// under the ground the rest of the raid).
    #[test]
    fn from_every_room_below_ground_the_way_out_goes_up() {
        use extraction_core::underground as ug;
        let mut bad = Vec::new();
        let mut rooms = 0;
        for map in crate::test_maps() {
            let opts = crate::launch::LaunchOptions {
                robots: false,
                map: map.clone(),
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
            let zone = crate::world::Zone::load(&map);
            for (_, building, xf) in b.iter() {
                let Some(l) = ug::layout(&building.prefab) else {
                    continue;
                };
                for room in &l.rooms {
                    let at = xf.transform_point(Vec3::new(
                        room.at[0],
                        l.floor_of(room) + 0.05,
                        room.at[1],
                    ));
                    rooms += 1;
                    let what = format!("{map}: {} `{}` at {at:.1}", building.prefab, room.id);
                    let Some(plan) = below_in(&zone, &plans, at) else {
                        bad.push(format!("{what}: not known to be in the site"));
                        continue;
                    };
                    let from = plan.to_local(at + Vec3::Y * 1.0).xz();
                    // (Shut off as it stands, by heat not vented or a door
                    // held on power: no way in either, so not where it'd be.)
                    let into = plan
                        .doors
                        .iter()
                        .filter_map(|d| d.outside)
                        .any(|o| plan.path(o, from).is_some());
                    if !into {
                        rooms -= 1;
                        continue;
                    }
                    // A step back from an anomaly, any way: only ever onto
                    // the site's floors (the generator room's wall).
                    for k in 0..8 {
                        let t = k as f32 / 8.0 * std::f32::consts::TAU;
                        let away = Vec2::new(t.cos(), t.sin());
                        for task in step_back_in(plan, at.xz(), away) {
                            if let Task::WalkTo { to, .. } = task
                                && !on_its_floors(plan, to)
                            {
                                bad.push(format!(
                                    "{what}: a step back {away:.1} goes off its floors to {to:.1}"
                                ));
                            }
                        }
                    }
                    match out_of(plan, from) {
                        None => bad.push(format!("{what}: no way out")),
                        Some((_, end)) if on_its_floors(plan, end) => {
                            bad.push(format!("{what}: the way out ends on its floors, {end:.1}"))
                        }
                        Some(_) => {}
                    }
                }
            }
        }
        assert!(rooms > 0, "no underground rooms on any map");
        assert!(
            bad.is_empty(),
            "{} of {rooms} rooms below ground:\n{}",
            bad.len(),
            bad.join("\n")
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
        let (at, _) = super::on_the_way_to(zone, None);
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

    /// A waypoint the grid or a bank's sidestep puts inside a building's
    /// box steps out of it, clear: the walk home past the pumping station
    /// was sent to (-82.2, -5.4), in its kiosk, and stood at its wall.
    #[test]
    fn a_waypoint_in_a_building_steps_out_of_it() {
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
        let en = app.world().resource::<crate::robots::doors::Entrances>();
        let boxes: Vec<(Transform, Vec2)> = en.shells.iter().map(|s| (s.xf, s.half)).collect();
        let stall = Vec2::new(-82.16, -5.35);
        let inside = |p: Vec2| {
            boxes.iter().any(|(xf, h)| {
                let l = xf
                    .compute_affine()
                    .inverse()
                    .transform_point3(Vec3::new(p.x, 0.0, p.y));
                l.x.abs() < h.x + 0.3 && l.z.abs() < h.y + 0.3
            })
        };
        assert!(
            inside(stall),
            "the stall's waypoint is in the pumping station's box"
        );
        let out = super::clear_of(&boxes, stall);
        assert!(!inside(out), "{stall} -> {out}, still in a box");
        assert!(out.distance(stall) < 4.0, "{stall} -> {out}: too far");
        // Clear of every box already: left as it is.
        let open = Vec2::new(-60.0, -40.0);
        assert!(!inside(open));
        assert_eq!(super::clear_of(&boxes, open), open);
    }

    /// The walk on from against the pumping station's wall, where both
    /// newcomers stood: no stretch of it goes through a building (its
    /// machines'-grid detour went straight through the pump house).
    #[test]
    fn the_walk_from_the_pump_house_wall_goes_round_it() {
        let opts = crate::launch::LaunchOptions {
            map: "marrow_creek".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..30 {
            app.update();
        }
        let w = app.world_mut();
        let mut st: bevy::ecs::system::SystemState<super::World2> =
            bevy::ecs::system::SystemState::new(w);
        let world = st.get(w).unwrap();
        assert!(world.nav.is_some(), "the machines' grid is up");
        let boxes = super::walk_boxes(&world);
        let rv = Vec2::new(-108.0, -50.0);
        for (from, to) in [
            (Vec2::new(-84.3, -5.4), Vec2::new(-72.15, -8.33)),
            (Vec2::new(-90.2, 0.1), Vec2::new(-72.15, -8.33)),
            // Home to the ranger tower from the post office, the village,
            // the logging camp and the drop.
            (Vec2::new(-16.2, 33.5), rv),
            (Vec2::new(-30.0, 40.0), rv),
            (Vec2::new(-90.9, 80.3), rv),
            (Vec2::new(22.0, 150.0), rv),
            (Vec2::new(-60.0, 10.0), rv),
            (Vec2::new(-74.8, 8.8), rv),
            // Back in from the south gate's guard booth, outside the fence
            // (under its roof from the first fight's sentry, the newcomer
            // walked at the wire east of the gate).
            (Vec2::new(32.2, 161.0), Vec2::new(31.64, 139.93)),
            (Vec2::new(32.2, 161.0), rv),
        ] {
            let mut at = from;
            for t in super::walk(&world, from, to) {
                let crate::bot::Task::WalkTo { to: p, .. } = t else {
                    continue;
                };
                for (xf, half) in &boxes {
                    let inv = xf.compute_affine().inverse();
                    let a = inv.transform_point3(Vec3::new(at.x, 0.0, at.y)).xz();
                    let b = inv.transform_point3(Vec3::new(p.x, 0.0, p.y)).xz();
                    assert!(
                        segment_hits_box(a, b, *half).is_none(),
                        "from {from}: {at} -> {p} goes through a building at {}",
                        xf.translation.xz()
                    );
                }
                at = p;
            }
            assert!(at.distance(to) < 0.1, "from {from}: ends at {at}");
        }
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
        // By the guard booth, for the road through the south gate: round
        // the gate's block, not into it (save18's wanderer stood a minute
        // against it at (30, 165)).
        let mut gate = boxes.clone();
        let mut q = w.query::<(&crate::world::buildings::Building, &Transform)>();
        let parts: Vec<_> = q
            .iter(w)
            .flat_map(|(b, xf)| super::part_boxes(&b.prefab, xf))
            .collect();
        assert!(parts.len() >= 2, "Marrow's south gate has its blocks");
        gate.extend(parts.iter().copied());
        let zone = w.resource::<crate::world::Zone>();
        let (from, to) = (Vec2::new(30.0, 165.0), Vec2::new(28.0, 153.8));
        let mut at = from;
        for p in round_boxes(&gate, from, to) {
            for (xf, half) in &parts {
                let inv = xf.compute_affine().inverse();
                let a = inv.transform_point3(Vec3::new(at.x, 0.0, at.y)).xz();
                let b = inv.transform_point3(Vec3::new(p.x, 0.0, p.y)).xz();
                assert!(
                    segment_hits_box(a, b, *half).is_none(),
                    "{from} -> {to}: {at} -> {p} goes into the gate's block"
                );
            }
            at = p;
        }
        // And the road between the blocks is still a way through.
        assert_eq!(
            round_boxes(&parts, Vec2::new(22.0, 170.0), Vec2::new(22.0, 157.0)),
            vec![Vec2::new(22.0, 157.0)],
            "the road through the gate is open"
        );
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

    /// A thing up in a tower's cab is fetched up its stairs and brought
    /// down them: on Marrow, each tower's cab (two ranger towers, the
    /// survey tower), from the ground beside it, the bot walks up the
    /// three flights, takes what's on the cab's table, and is back on the
    /// ground at the stairs' foot. (Before, `way_to` routed to under the
    /// cab's door and the newcomer stood on the ground under the radio.)
    #[test]
    fn a_tower_cab_is_fetched_from_up_its_stairs() {
        // Room over a stair's tread a standing body walks up (m): the
        // player's 1.75 m capsule, and a head's swing.
        const HEADROOM: f32 = 2.0;
        use super::{in_reach, way_down, way_to};
        use crate::bot::Bot;
        use crate::rig::{LocalPlayer, Side};
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
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        // Each tower: where its table's thing lies, where the bot starts,
        // the walk, and a loose thing to put there.
        let towers = {
            let w = app.world_mut();
            let mut st: SystemState<(
                super::World2,
                super::RaidView,
                avian3d::prelude::SpatialQuery,
            )> = SystemState::new(w);
            let (world, view, spatial) = st.get(w).unwrap();
            let plans = crate::sim::indoor::plan_buildings(&view.buildings, &view.leaves);
            let mut loose = world
                .items
                .iter()
                .filter(|(_, _, _, held, sock, bag)| {
                    held.is_none() && sock.is_none() && bag.is_none()
                })
                .map(|(e, ..)| e);
            let mut towers = Vec::new();
            for plan in plans
                .iter()
                .filter(|p| p.name == "ranger_tower" || p.name == "survey_tower")
            {
                let geo = crate::world::buildings::prefab(&plan.name).unwrap();
                // A standing body clears every flight: along the walked
                // line from the stairs' foot, 2 m over the tread or landing
                // underfoot (in one lane, the first flight's top ran 0.2 m
                // under the second's foot).
                for w in &geo.stairs {
                    let line: Vec<Vec3> = w.iter().map(|q| plan.xf.transform_point(*q)).collect();
                    for leg in line.windows(2).skip(1) {
                        for i in 0..=24 {
                            let p = leg[0].lerp(leg[1], i as f32 / 24.0);
                            let filter = avian3d::prelude::SpatialQueryFilter::default();
                            let Some(down) = spatial.cast_ray(
                                p + Vec3::Y * 0.6,
                                Dir3::NEG_Y,
                                1.4,
                                true,
                                &filter,
                            ) else {
                                continue;
                            };
                            let floor = p + Vec3::Y * (0.6 - down.distance + 0.02);
                            let room = spatial
                                .cast_ray(floor, Dir3::Y, 3.0, true, &filter)
                                .map_or(3.0, |h| h.distance);
                            assert!(
                                room >= HEADROOM,
                                "{} at {:.0?}: {room:.2} m over the stair at {:.2?} \
                                 (a standing body needs {HEADROOM})",
                                plan.name,
                                plan.xf.translation.xz(),
                                plan.to_local(floor)
                            );
                        }
                    }
                }
                let spot = geo.loot.iter().find(|s| s.table == "gadgets").unwrap();
                let p = plan.xf.transform_point(spot.at + Vec3::Y * 0.05);
                assert!(
                    in_reach(&world, &plans, p),
                    "{} at {:.0?}: its cab's table ({:.1?}) is in reach by its stairs \
                     (in its plan: {}; floor {:.2}; ways in {:.1?}; stairs {:.1?})",
                    plan.name,
                    plan.xf.translation.xz(),
                    p,
                    plan.contains(p, 0.3),
                    plan.floor,
                    plan.doors
                        .iter()
                        .filter_map(|d| d.outside)
                        .chain(plan.entries.iter().copied())
                        .map(|o| plan.to_world(o))
                        .collect::<Vec<_>>(),
                    geo.stairs
                        .iter()
                        .map(|w| w
                            .iter()
                            .map(|q| plan.xf.transform_point(*q))
                            .collect::<Vec<_>>())
                        .collect::<Vec<_>>()
                );
                let start = plan.xf.transform_point(Vec3::new(-7.0, 0.0, 0.0)).xz();
                let mut tasks = way_to(&world, &view, &plans, start, p);
                let e = loose.next().unwrap();
                tasks.push(Task::Grab {
                    item: e,
                    side: Side::Right,
                });
                let down = way_down(&world, &plans, p);
                assert!(!down.is_empty(), "{}: a way back down", plan.name);
                let foot = match down.last() {
                    Some(Task::WalkTo { to, .. }) => *to,
                    _ => unreachable!(),
                };
                tasks.extend(down);
                towers.push((plan.name.clone(), p, start, tasks, e, foot));
            }
            towers
        };
        assert_eq!(towers.len(), 3, "Marrow's three towers");
        for (name, p, start, tasks, e, foot) in towers {
            let zone = app.world().resource::<crate::world::Zone>().clone();
            app.world_mut().get_mut::<Transform>(e).unwrap().translation = p;
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = zone.on_ground(start.x, start.y) + Vec3::Y * 0.05;
            app.update();
            let planned: Vec<String> = tasks.iter().map(|t| format!("{t:?}")).collect();
            app.world_mut().entity_mut(player).insert(Bot::new(tasks));
            let (mut top, mut held_up) = (f32::NEG_INFINITY, false);
            for _ in 0..90 * 150 {
                app.update();
                let at = app.world().get::<Transform>(player).unwrap().translation;
                top = top.max(at.y - zone.on_ground(at.x, at.z).y);
                held_up |= app
                    .world()
                    .get::<crate::interact::Held>(e)
                    .is_some_and(|h| h.player == player);
                if app.world().get::<Bot>(player).unwrap().done() {
                    break;
                }
            }
            let at = app.world().get::<Transform>(player).unwrap().translation;
            let bot = app.world().get::<Bot>(player).unwrap();
            assert!(
                bot.done(),
                "{name}: the walk up and down ends (left: {:?})",
                bot.tasks.front()
            );
            assert!(
                top > 8.5,
                "{name}: up at the cab ({top:.1} m over the ground; the walk: {planned:#?}; its log: {:#?})",
                bot.log
            );
            assert!(
                held_up,
                "{name}: what's on the cab's table taken (the walk: {planned:#?}; its log: {:#?}; \
                 the thing at {:.1?})",
                bot.log,
                app.world().get::<Transform>(e).unwrap().translation
            );
            assert!(
                at.xz().distance(foot) < 1.0 && (at.y - zone.on_ground(at.x, at.z).y).abs() < 0.6,
                "{name}: back down at the stairs' foot ({:.1?}, {:.1} m over the ground)",
                at.xz(),
                at.y - zone.on_ground(at.x, at.z).y
            );
            app.world_mut().entity_mut(player).remove::<Bot>();
        }
    }

    /// Every walk leg comes to the newcomer's feet through `routed_leg`,
    /// whatever handed it out: the way there (`route`), a wanderer's road
    /// (`by_road`), off from a machine (`walk_away`, `walk_off_heavy`), into
    /// cover (`run_for_cover`), out of a machine's line (`break_line`), to a
    /// thing to fetch (`way_to`), and a walk planned from somewhere it no
    /// longer is (the wanderer's, through the rail shed's south wall). Past
    /// every building on Marrow, from each side to the far one: as walked,
    /// no leg goes through what `walk` goes round. And without it, some
    /// would (the check bites).
    #[test]
    fn every_walk_leg_comes_to_the_feet_routed() {
        use super::{
            break_line, by_road, crossed, route, routed_leg, run_for_cover, walk, walk_away,
            walk_boxes, walk_off_heavy, way_to,
        };
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
        let w = app.world_mut();
        let mut st: SystemState<(
            super::World2,
            super::RaidView,
            avian3d::prelude::SpatialQuery,
        )> = SystemState::new(w);
        let (world, view, spatial) = st.get(w).unwrap();
        let plans = crate::sim::indoor::plan_buildings(&view.buildings, &view.leaves);
        let boxes = walk_boxes(&world);
        // Each leg as the runner meets it, from where the last one ended:
        // through `routed_leg` (`choke`), or as handed out.
        let as_walked = |here: Vec2, tasks: Vec<Task>, choke: bool| {
            let mut q: std::collections::VecDeque<Task> = tasks.into();
            let (mut at, mut legs, mut n) = (here, vec![], 0);
            while let Some(t) = q.pop_front() {
                n += 1;
                assert!(n < 5000, "from {here}: the legs never end");
                let Task::WalkTo { to, within } = t else {
                    continue;
                };
                let feet = world.zone.on_ground(at.x, at.y);
                if choke && let Some(way) = routed_leg(&world, feet, to, within) {
                    for t in way.into_iter().rev() {
                        q.push_front(t);
                    }
                    continue;
                }
                legs.push((at, to));
                at = to;
            }
            legs
        };
        let shells: Vec<(Transform, Vec2)> = view
            .entrances
            .as_deref()
            .expect("Marrow's doors")
            .shells
            .iter()
            .map(|s| (s.xf, s.half))
            .collect();
        assert!(shells.len() > 10, "{} buildings on Marrow", shells.len());
        let inside = world.zone.map.half() - 10.0;
        let mut raw: std::collections::BTreeMap<&str, usize> = Default::default();
        let mut through = vec![];
        for (k, (xf, half)) in shells.iter().enumerate() {
            let c = xf.translation.xz();
            for side in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                let dir = xf.rotation.mul_vec3(Vec3::new(side.x, 0.0, side.y)).xz();
                let reach = half.length();
                let (here, to) = (c + dir * (reach + 3.0), c - dir * (reach + 12.0));
                let edge = |p: Vec2| p.x.abs() < inside && p.y.abs() < inside;
                if !edge(here) || !edge(to) || crossed(&boxes, here, here).is_some() {
                    continue;
                }
                // The machine behind it, on its far side from the building.
                let machine = here + dir * 15.0;
                let eye = world.zone.on_ground(machine.x, machine.y) + Vec3::Y * 1.6;
                let p3 = world.zone.on_ground(to.x, to.y);
                let elsewhere = c + dir.perp() * (reach + 12.0);
                let mut made: Vec<(&str, Vec<Task>)> = vec![
                    ("route", route(&world, &view, here, to)),
                    (
                        "by_road",
                        by_road(&world, &view, here, to, to + Vec2::splat(500.0)),
                    ),
                    ("walk_away", walk_away(&world, here, machine)),
                    ("walk_off_heavy", walk_off_heavy(&world, here, machine)),
                    (
                        "run_for_cover",
                        run_for_cover(&world.zone, here, machine, 0.0),
                    ),
                    (
                        "run_for_cover, seen",
                        run_for_cover(&world.zone, here, machine, 10.0),
                    ),
                    ("way_to", way_to(&world, &view, &plans, here, p3)),
                    // Planned from round the corner, walked from here.
                    ("planned elsewhere", walk(&world, elsewhere, to)),
                ];
                if k % 3 == 0
                    && let Some((t, _)) = break_line(&spatial, &world, here, eye)
                {
                    made.push(("break_line", t));
                }
                for (by, tasks) in made {
                    let bad = |legs: &[(Vec2, Vec2)]| {
                        legs.iter()
                            .filter_map(|(a, b)| crossed(&boxes, *a, *b).map(|x| (*a, *b, x)))
                            .collect::<Vec<_>>()
                    };
                    *raw.entry(by).or_default() +=
                        bad(&as_walked(here, tasks.clone(), false)).len();
                    for (a, b, x) in bad(&as_walked(here, tasks, true)) {
                        through.push(format!(
                            "{by} from {here}: {a} -> {b} through the box at {x}"
                        ));
                    }
                }
            }
        }
        eprintln!("legs through a box as handed out: {raw:?}");
        assert!(
            through.is_empty(),
            "{} legs walked through a box:\n{}",
            through.len(),
            through.join("\n")
        );
        assert!(
            raw.values().sum::<usize>() > 0,
            "no producer handed out a leg through a box: the check is untested"
        );
    }

    /// A leg under a building, or in one, is the indoor plan's: walked as
    /// it is. The lost newcomer in the post office's cellar on Marrow, a
    /// step under its wall, had it put round the post office above, out
    /// to the cellar's wall (`Vec2(-19.5, 72.9)`), and stood there till the
    /// walk timed out. (That step only grazed the wall's corner, and now
    /// isn't through it: here, one from there right under the post office.)
    #[test]
    fn a_leg_under_the_ground_is_not_routed_round_what_stands_on_it() {
        use super::{crossed, routed_leg, walk_boxes};
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
        let w = app.world_mut();
        let mut st: SystemState<super::World2> = SystemState::new(w);
        let world = st.get(w).unwrap();
        // (Under the post office from side to side, at the cellar's floor.)
        let (at, to) = (Vec2::new(-20.2, 72.0), Vec2::new(-20.2, 55.0));
        let ground = world.zone.on_ground(at.x, at.y);
        let cellar = Vec3::new(at.x, 12.29, at.y);
        assert!(cellar.y < ground.y - 1.5, "the cellar under {ground}");
        assert!(
            crossed(&walk_boxes(&world), at, to).is_some(),
            "drawn on the ground, the leg goes through the post office"
        );
        let way = routed_leg(&world, cellar, to, 0.4);
        assert!(way.is_none(), "in the cellar, put round above: {way:?}");
    }

    /// A dash that cuts a corner by less than a body's half-width isn't
    /// through it: the lorry wreck on the Marrow farm (2.6 m by 7 m), and
    /// the one-handed newcomer's four dashes into cover by it from the
    /// walker, 0.2 to 0.25 m into a corner each. Put round the corner, the
    /// first was 3.1 m made 9, in the walker's line. A step through the
    /// wreck's middle, or a long walk through its side, still is.
    #[test]
    fn a_corner_grazed_is_not_crossed() {
        use super::crossed;
        let wreck = [(Transform::from_xyz(13.5, 0.0, 136.0), Vec2::new(1.3, 3.5))];
        let v = |x: f32, y: f32| Vec2::new(x, y);
        for (a, b) in [
            (v(11.792, 138.691), v(13.601, 141.244)),
            (v(11.734, 138.668), v(13.543, 141.221)),
            (v(16.575, 139.228), v(13.082, 140.044)),
            (v(12.611, 140.098), v(11.407, 135.479)),
        ] {
            assert_eq!(crossed(&wreck, a, b), None, "{a} -> {b}, grazing a corner");
        }
        for (a, b) in [
            (v(10.0, 136.0), v(17.0, 136.0)),
            (v(13.5, 128.0), v(13.5, 144.0)),
            (v(11.0, 138.5), v(16.0, 138.5)),
        ] {
            assert!(crossed(&wreck, a, b).is_some(), "{a} -> {b}, through it");
        }
    }
}
