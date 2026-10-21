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

/// This session's newcomer (`--personality NAME`), in a first-hour run.
pub fn current(opts: &LaunchOptions) -> Option<&'static Persona> {
    if !matches!(opts.scenario.as_deref(), Some(YARD | RAID)) {
        return None;
    }
    let name = opts.personality.as_deref()?;
    PERSONAS.iter().find(|p| p.name == name)
}

/// The newcomers a `--scenario` name asks for, if it's a first-hour one.
pub fn chain(scenario: &str) -> Option<Vec<&'static Persona>> {
    if scenario == SCENARIO {
        return Some(PERSONAS.iter().collect());
    }
    PERSONAS
        .iter()
        .find(|p| p.scenario == scenario)
        .map(|p| vec![p])
}

/// Where a newcomer's two sessions keep the career.
fn folder(p: &Persona) -> PathBuf {
    PathBuf::from("target/first_hour").join(p.name)
}

/// Runs each newcomer's two sessions, a fresh career each, and says how
/// each went.
pub fn run_chain(personas: &[&'static Persona]) -> AppExit {
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
        for (stage, extra) in [
            (YARD, vec![]),
            // As the cord would open it: on to the next valley.
            (RAID, vec!["--map".to_string(), "marrow_creek".to_string()]),
        ] {
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(&passthrough)
                .args(["--scenario", stage, "--personality", p.name])
                .args(["--data-dir", &dir_s, "--save", &save])
                .args(&extra);
            if p.one_handed {
                cmd.arg("--one-handed");
            }
            println!("first hour: {} in {stage}", p.name);
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
use extraction_core::map::MarkerKind;

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
    Done,
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
    zone.map
        .markers
        .iter()
        .filter(|m| m.kind == MarkerKind::Poi && !m.name.is_empty())
        .filter(|m| text.contains(&flat(&m.name)))
        .max_by_key(|m| m.name.len())
        .map(|m| (m.name.clone(), Vec2::from(m.at)))
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
    card: Query<'w, 's, &'static ScreenText, With<RequestCard>>,
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
    mut state: Local<Newcomer>,
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
    let creep = p.reads && !matches!(state.leg, Leg::Camp | Leg::Kit | Leg::Card | Leg::Board);
    if input.crouch != creep {
        input.crouch = creep;
    }
    bot.sprint = p.sprint;

    // Heard: what the driver says is what it knows.
    let showing: Vec<String> = raid.log.iter().map(|(l, _)| l.clone()).collect();
    for l in &showing {
        if !state.showing.contains(l) {
            state.heard.push(l.clone());
            state.said_at = now;
        }
    }
    state.showing = showing;
    for FirstMet(f) in met.read() {
        state.note(now, Kind::Learned, format!("{f:?}: {}", f.lines()[0]));
        let answer: Vec<Task> = match f {
            First::Jam(_) => vec![Task::Gun(crate::weapons::flat::MacroKind::Rack)],
            First::Heard | First::DroneLock if !p.reckless => {
                vec![Task::Wait(15.0)]
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
            let warned = now - state.said_at < 30.0;
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
    if matches!(raid.phase, Phase::Down { .. }) && state.leg != Leg::Done {
        state.note(now, Kind::Killed, "went down");
        state.leg = Leg::Done;
    }

    let idle_only =
        bot.tasks.len() == 1 && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE);
    if !idle_only || bot.elapsed < 0.5 {
        return;
    }
    // Reckless: a machine in reach is a fight (once a minute at most).
    if p.reckless
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
            tasks.extend(world.legs(here, at_truck(0.0, 4.0)));
            for (x, z) in [(0.05, 0.3), (-0.2, -0.6)] {
                tasks.push(Task::WalkTo {
                    to: at_truck(x, z),
                    within: 0.3,
                });
            }
            state.leg = Leg::Kit;
        }
        Leg::Kit => {
            let in_crate: Vec<(Entity, String)> = view
                .stash
                .iter()
                .flat_map(|b| b.contents.iter().copied())
                .filter_map(|e| world.id(e).map(|id| (e, id.to_string())))
                .collect();
            for (e, id) in in_crate {
                if p.hoards || KIT.contains(&id.as_str()) {
                    tasks.push(Task::Grab {
                        item: e,
                        side: Side::Right,
                    });
                    tasks.push(Task::Stow(Side::Right));
                }
            }
            state.leg = Leg::Card;
        }
        Leg::Card => {
            let text = view
                .card
                .iter()
                .next()
                .map(|c| c.text.clone())
                .unwrap_or_default();
            if p.reads {
                tasks.push(Task::Wait(4.0));
            }
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
            }
            // Lost: off the wrong way first, until the map puts it right.
            if p.lost && !state.wrong_way_done {
                state.wrong_way_done = true;
                let wrong = from - (at - from).normalize_or_zero() * 70.0;
                state.note(now, Kind::Confused, "set off the wrong way");
                tasks.extend(world.legs(from, wrong));
                from = wrong;
            }
            tasks.extend(world.legs(from, at));
            state.note(now, Kind::Learned, format!("on to {name}"));
            state.leg = Leg::Place;
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
                    tasks.extend(world.legs(here, g));
                    tasks.push(Task::Wait(3.0));
                }
                state.tries += 1;
                if state.tries > 4 {
                    state.note(now, Kind::Stuck, "at the place, and nothing says it's seen");
                    state.leg = Leg::Call;
                    state.tries = 0;
                }
            } else if let Some((_, g)) = state.goal.clone() {
                tasks.extend(world.legs(here, g));
                state.tries += 1;
            }
        }
        Leg::Call => {
            if raid.phase == Phase::AtBase {
                // Still by the truck: out first (the radio's for the Zone).
                let away = here + (here - at_truck(0.0, 0.0)).normalize_or(Vec2::Y) * 20.0;
                tasks.extend(world.legs(here, away));
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
                    tasks.extend(world.legs(here, rv));
                }
                state.leg = Leg::Rendezvous;
            } else if world.radio_in_hand() {
                tasks.extend([Task::Use(Side::Left), Task::Wait(1.0)]);
            } else {
                let radio = world.items.iter().find(|(_, it, _, held, _, _)| {
                    held.is_none() && world.catalog.get(it.kind()).id == "radio"
                });
                match radio {
                    Some((e, ..)) => tasks.extend([
                        Task::Stow(Side::Left),
                        Task::Grab {
                            item: e,
                            side: Side::Left,
                        },
                    ]),
                    None => {
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
                state.leg = Leg::Done;
            }
            Phase::TruckWaiting { .. } => {
                tasks.extend(world.go_home(here, &raid, state.tries));
                state.tries += 1;
            }
            _ => {
                state.leg = Leg::Call;
            }
        },
        Leg::Done => {
            if !state.reported {
                state.reported = true;
                report(&mut bot, p, &state.moments);
            }
            return;
        }
    }
    let _ = player;
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
