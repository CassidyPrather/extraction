//! `--scenario crossing`: a raid's crossing, start to finish, with nothing
//! set up. The bot steps down from the truck, takes the radio from the
//! ground by the insertion, and walks the map lint's main route to the
//! nearest rendezvous (`extraction_core::map_lint`, the walk a first-timer
//! takes), radios the truck, boards it when it comes and pulls the cord.
//! It passes only if the raid ends in an extraction; it fails, saying
//! where, if the walk stalls (a step it can't climb, a gap it falls into,
//! a wall across the way).
//!
//! Without `--map` it plays Sallow Reservoir (`docs/FOURTH_VALLEY.md`),
//! whose shortest crossing climbs the dam's face stair and walks its
//! crest: `--map <valley>` crosses any other.

use bevy::prelude::*;
use extraction_core::map::MarkerKind;

use super::{Bot, Task};
use crate::launch::LaunchOptions;
use crate::raid::truck::Pressable;
use crate::raid::{Phase, Raid};
use crate::rig::{LocalPlayer, Side};
use crate::world::Zone;

pub const SCENARIO: &str = "crossing";
/// The valley it crosses when no `--map` is given.
pub const DEFAULT_MAP: &str = "sallow_reservoir";

/// The last task while the run goes on: the director adds each stage's
/// tasks ahead of it.
const IDLE: f32 = 3.25;
/// Route points are every 2 m; the bot walks to every other one.
const STRIDE: usize = 2;
/// Seconds without getting further along the route before it's stuck.
const STALL: f32 = 45.0;
/// How far short of the rendezvous marker the walk stops (the map lint's
/// `TRUCK_ROOM` is 8 m: nothing stands nearer).
const TRUCK_ROOM: f32 = 9.0;
/// Seconds of game time the whole crossing may take.
const DEADLINE: f32 = 1800.0;

pub fn opening() -> Vec<Task> {
    vec![
        Task::Log("crossing: insertion to the nearest rendezvous, and out".into()),
        Task::Wait(IDLE),
    ]
}

pub struct CrossingPlugin;

impl Plugin for CrossingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_radio)
            .add_systems(Update, direct);
    }
}

/// The radio (a fresh game keeps it in the stash, which the scenario
/// doesn't have), laid by the insertion.
fn spawn_radio(
    mut commands: Commands,
    zone: Res<Zone>,
    catalog: Res<crate::items::Catalog>,
    opts: Res<LaunchOptions>,
) {
    if opts.scenario.as_deref() != Some(SCENARIO) {
        return;
    }
    let (Some(ins), Some(kind)) = (zone.marker(&MarkerKind::Insertion), catalog.kind("radio"))
    else {
        return;
    };
    let (x, z) = (ins.at[0] + 1.0, ins.at[1] - 3.0);
    crate::items::spawn_item(
        &mut commands,
        &catalog,
        crate::items::SpawnItem {
            kind,
            transform: Transform::from_xyz(x, zone.ground(x, z) + 0.25, z),
            condition: 0.9,
        },
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Start,
    Walk,
    Call,
    Wait,
    Board,
    Done,
}

#[derive(Default)]
struct Run {
    stage: Stage,
    /// The route walked: its rendezvous, its points.
    to: String,
    route: Vec<Vec2>,
    /// The furthest route point reached, and when.
    reached: usize,
    since: f32,
    tries: u32,
}

#[allow(clippy::too_many_arguments)]
fn direct(
    opts: Res<LaunchOptions>,
    zone: Res<Zone>,
    mut raid: ResMut<Raid>,
    mut bots: Query<(&mut Bot, &Transform), With<LocalPlayer>>,
    pressables: Query<(Entity, &Pressable)>,
    time: Res<Time>,
    mut run: Local<Run>,
) {
    if opts.scenario.as_deref() != Some(SCENARIO) || run.stage == Stage::Done {
        return;
    }
    let Some((mut bot, root)) = bots.iter_mut().next() else {
        return;
    };
    let now = time.elapsed_secs();
    let here = root.translation.xz();
    // The run doesn't sit out the truck's whole drive.
    if let Phase::TruckCalled { eta, .. } = &mut raid.phase {
        *eta = eta.min(10.0);
    }
    let fail = |bot: &mut Bot, run: &mut Run, why: String| {
        bot.failed += 1;
        bot.log.push(format!("FAIL {why}"));
        bot.tasks.clear();
        run.stage = Stage::Done;
    };
    if now > DEADLINE {
        let why = format!("still {:?} after {DEADLINE:.0} s, at {here:.0}", run.stage);
        fail(&mut bot, &mut run, why);
        return;
    }
    // Progress along the route: the furthest point within reach of where
    // the bot is.
    if run.stage == Stage::Walk {
        let ahead = run
            .route
            .iter()
            .enumerate()
            .skip(run.reached)
            .take(20)
            .filter(|(_, p)| p.distance(here) < 4.0)
            .map(|(i, _)| i)
            .max();
        if let Some(i) = ahead.filter(|i| *i > run.reached) {
            run.reached = i;
            run.since = now;
        }
        if now - run.since > STALL {
            let why = format!(
                "stuck {STALL:.0} s on the way to {} at {here:.0} ({:.0} m of {:.0})",
                run.to,
                run.reached as f32 * 2.0,
                run.route.len() as f32 * 2.0
            );
            fail(&mut bot, &mut run, why);
            return;
        }
    }
    let idling =
        bot.tasks.len() == 1 && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE);
    if !idling || bot.elapsed < 0.5 {
        return;
    }
    let mut t = Vec::new();
    match run.stage {
        Stage::Start => {
            let report = extraction_core::map_lint::lint(&zone.map, &zone.hm);
            let Some(route) = report
                .routes
                .iter()
                .min_by(|a, b| a.length.total_cmp(&b.length))
            else {
                fail(&mut bot, &mut run, "the lint found no main route".into());
                return;
            };
            run.to = route.to.clone();
            // Up to the truck's room, not onto the marker: the truck
            // appears there.
            let end = Vec2::from_array(*route.points.last().unwrap());
            run.route = route
                .points
                .iter()
                .map(|p| Vec2::from_array(*p))
                .filter(|p| p.distance(end) > TRUCK_ROOM)
                .collect();
            if run.route.is_empty() {
                fail(&mut bot, &mut run, "the route is all truck room".into());
                return;
            }
            run.since = now;
            bot.log.push(format!(
                "to {}: {:.0} m by the lint's main route",
                route.to, route.length
            ));
            t.push(Task::PickUp {
                id: "radio".into(),
                side: Side::Left,
            });
            let last = run.route.len() - 1;
            for (i, p) in run.route.iter().enumerate() {
                if i % STRIDE == 0 || i == last {
                    t.push(Task::WalkTo {
                        to: *p,
                        within: if i == last { 1.0 } else { 1.4 },
                    });
                }
            }
            run.stage = Stage::Walk;
        }
        Stage::Walk => {
            bot.log.push(format!("at {} after {now:.0} s", run.to));
            t.push(Task::Use(Side::Left));
            t.push(Task::Wait(1.0));
            run.stage = Stage::Call;
        }
        Stage::Call => match raid.phase {
            Phase::TruckCalled { .. } | Phase::TruckWaiting { .. } => run.stage = Stage::Wait,
            _ if run.tries < 3 => {
                run.tries += 1;
                t.push(Task::Use(Side::Left));
                t.push(Task::Wait(2.0));
            }
            _ => {
                let why = format!("the radio didn't call the truck ({:?})", raid.phase);
                fail(&mut bot, &mut run, why);
                return;
            }
        },
        Stage::Wait => {
            if let Phase::TruckWaiting { rv, .. } = raid.phase {
                let pose = crate::raid::truck::rv_pose(&zone, rv);
                let p = |z: f32| pose.transform_point(Vec3::new(0.0, 0.0, z)).xz();
                let bell = pressables
                    .iter()
                    .find(|(_, p)| matches!(p, Pressable::BellPull))
                    .map(|(e, _)| e);
                t.push(Task::WalkTo {
                    to: p(8.0),
                    within: 1.5,
                });
                t.extend(crate::sim::raider::board_tasks(p, bell));
                run.tries = 0;
                run.stage = Stage::Board;
            } else {
                t.push(Task::Wait(1.0));
            }
        }
        Stage::Board => {
            if raid.extractions > 0 || matches!(raid.phase, Phase::Extracting { .. }) {
                bot.passed += 1;
                bot.log.push(format!(
                    "PASS extracted at {} after {now:.0} s on foot and in the truck",
                    run.to
                ));
                bot.tasks.clear();
                run.stage = Stage::Done;
                return;
            }
            if run.tries >= 3 {
                let why = format!("couldn't leave from {} ({:?})", run.to, raid.phase);
                fail(&mut bot, &mut run, why);
                return;
            }
            run.tries += 1;
            run.stage = Stage::Wait;
        }
        Stage::Done => return,
    }
    let idle = bot.tasks.pop_back();
    bot.tasks.extend(t);
    bot.tasks.extend(idle);
}
