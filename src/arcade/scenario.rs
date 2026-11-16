//! `--headless --scenario arcade_smoke` (or with `--arcade`): a bot at
//! the bench takes the AK off the table, readies it, presses CALL WAVE,
//! fights wave 1's walker when it comes through the north gate, reloads
//! from a spare on the table, and presses RESTART. It passes when the
//! run's record says all of that happened (its jsonl and its summary),
//! the ground is laid out again, and no career was touched.

use bevy::prelude::*;

use super::arena::{ArcadeCard, Laid};
use super::record::Recording;
use super::{Arcade, ArcadeSystems, RunPhase};
use crate::bot::{Bot, Task};
use crate::launch::LaunchOptions;
use crate::rig::{LocalPlayer, Side};
use crate::weapons::flat::MacroKind;

pub const SCENARIO: &str = "arcade_smoke";

/// The bot fights a machine this near, m.
const ENGAGE_AT: f32 = 35.0;

/// Game seconds the whole run may take.
const TIME_LIMIT: f32 = 400.0;

/// The scenario's opening (`bot::scenarios::tasks`): a wait while the
/// bench is set up; [`drive`] gives the rest.
pub fn opening() -> Vec<Task> {
    vec![
        Task::Log("arcade smoke: one wave from the bench, recorded".into()),
        Task::Wait(5.0),
    ]
}

/// Waits that keep the bot busy while the run is watched.
fn hold() -> Vec<Task> {
    vec![Task::Wait(20.0); (TIME_LIMIT / 20.0) as usize]
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
enum Step {
    #[default]
    Waiting,
    Fighting,
    Restarting,
    Done,
}

pub struct ScenarioPlugin;

impl Plugin for ScenarioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive.after(ArcadeSystems::Record));
    }
}

#[allow(clippy::too_many_arguments)]
fn drive(
    opts: Res<LaunchOptions>,
    arcade: Res<Arcade>,
    rec: Res<Recording>,
    laid: Res<Laid>,
    mut bots: Query<&mut Bot, With<LocalPlayer>>,
    cards: Query<(Entity, &ArcadeCard)>,
    time: Res<Time>,
    machines: Query<(&crate::robots::Robot, &GlobalTransform), With<super::waves::WaveMachine>>,
    you: Query<&GlobalTransform, With<LocalPlayer>>,
    mut step: Local<Step>,
    mut engaged: Local<bool>,
) {
    let you = you.iter().next();
    if opts.scenario.as_deref() != Some(SCENARIO) || *step == Step::Done {
        return;
    }
    let Some(mut bot) = bots.iter_mut().next() else {
        return;
    };
    let card = |which: ArcadeCard| cards.iter().find(|(_, c)| **c == which).map(|(e, _)| e);
    let fail = |bot: &mut Bot, why: String| {
        error!("arcade smoke: {why}");
        bot.failed += 1;
        bot.log.push(format!("FAIL {why}"));
    };
    if time.elapsed_secs() > TIME_LIMIT {
        fail(
            &mut bot,
            format!(
                "not done in {TIME_LIMIT} s: at {:?}, {:?}",
                *step, arcade.phase
            ),
        );
        bot.tasks.clear();
        *step = Step::Done;
        return;
    }
    if let (Step::Fighting, RunPhase::Over { wave }) = (*step, arcade.phase) {
        fail(&mut bot, format!("the bot went down on wave {wave}"));
        bot.tasks.clear();
        *step = Step::Done;
        return;
    }
    match *step {
        Step::Waiting => {
            let (Some(call), true) = (card(ArcadeCard::Call), laid.0) else {
                return;
            };
            use Task::*;
            bot.tasks.extend([
                PickUp {
                    id: "ak74".into(),
                    side: Side::Right,
                },
                Ready { off_safe: true },
                Press(call),
                Log("arcade smoke: wave called".into()),
            ]);
            // Held open while the rest is checked (a task gives up after
            // 30 s).
            bot.tasks.extend(hold());
            *step = Step::Fighting;
        }
        Step::Fighting if !*engaged => {
            // Fight it once it's near and in the open, as anyone at the
            // bench would: the house stands between the bench and the
            // north gate.
            let near = machines.iter().any(|(r, g)| {
                !r.wrecked
                    && g.translation()
                        .xz()
                        .distance(you.map_or(Vec2::ZERO, |y| y.translation().xz()))
                        < ENGAGE_AT
            });
            if near {
                use Task::*;
                bot.tasks.clear();
                bot.tasks.extend([
                    Log("arcade smoke: engaging".into()),
                    Engage { shots: 30 },
                    Wait(2.0),
                    PickUp {
                        id: "ak_mag".into(),
                        side: Side::Left,
                    },
                    Gun(MacroKind::Reload),
                    Engage { shots: 30 },
                ]);
                bot.tasks.extend(hold());
                *engaged = true;
            }
        }
        Step::Fighting => {
            let reloaded = rec.stats.guns.get("ak74").is_some_and(|g| g.reloads > 0);
            if matches!(arcade.phase, RunPhase::Cleared { wave: 1 }) && reloaded {
                let Some(restart) = card(ArcadeCard::Restart) else {
                    return;
                };
                bot.tasks.clear();
                bot.tasks.extend([
                    Task::Press(restart),
                    Task::Log("arcade smoke: restart".into()),
                ]);
                bot.tasks.extend(hold());
                *step = Step::Restarting;
            }
        }
        Step::Restarting => {
            if rec.finished.is_empty() || arcade.phase != RunPhase::Ready || !laid.0 {
                return;
            }
            let problems = check(&opts, &rec);
            if problems.is_empty() {
                bot.passed += 1;
                bot.log.push(format!(
                    "ok: the run's record has every beat ({})",
                    rec.finished[0].display()
                ));
            } else {
                for p in problems {
                    fail(&mut bot, p);
                }
            }
            bot.tasks.clear();
            *step = Step::Done;
        }
        Step::Done => {}
    }
}

/// What the finished run's files must hold.
fn check(opts: &LaunchOptions, rec: &Recording) -> Vec<String> {
    let mut out = Vec::new();
    let jsonl = &rec.finished[0];
    let text = std::fs::read_to_string(jsonl).unwrap_or_default();
    let evs: Vec<String> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v["ev"].as_str().map(str::to_owned))
        .collect();
    for want in [
        "arcade_run",
        "wave_called",
        "wave_arrived",
        "fired",
        "shot",
        "hit",
        "machine_down",
        "wave_cleared",
        "reload",
        "run_over",
    ] {
        if !evs.iter().any(|e| e == want) {
            out.push(format!("no `{want}` line in {}", jsonl.display()));
        }
    }
    let summary = jsonl.with_extension("summary.txt");
    match std::fs::read_to_string(&summary) {
        Ok(s) if s.contains("ak74") && s.contains("walker") => {}
        Ok(s) => out.push(format!("the summary misses the AK or the walker:\n{s}")),
        Err(e) => out.push(format!("no summary at {}: {e}", summary.display())),
    }
    if crate::persist::career_path(opts).is_some() {
        out.push("an arcade session has a career path".into());
    }
    if crate::persist::owns_profile(opts)
        && crate::persist::profile(opts).join("career.ron").exists()
    {
        out.push("a career.ron was written".into());
    }
    out
}
