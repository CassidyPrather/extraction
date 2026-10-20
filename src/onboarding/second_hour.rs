//! `--scenario second_hour`: a bot plays a short career past the yard (three
//! raids in Marrow Creek, and the start of a fourth) and meets the
//! first-time moments the driver talks you through ([`super::firsts`]).
//! Each moment is set up the way the drills set things up (a worn rifle, a
//! walker up the road, the Surge's clock wound on, health run out); then
//! the bot does what the driver's line says, and the run checks it worked.
//! The table it prints is `docs/SECOND_HOUR.md`'s.
//!
//! Nothing here marks a moment met: the game's own watchers do, from what
//! the bot does.

use bevy::prelude::*;
use extraction_core::map::MarkerKind;

use super::firsts::{First, FirstMet, Firsts};
use crate::bot::{Bot, Task};
use crate::interact::Held;
use crate::interact::body::Backpack;
use crate::items::{Catalog, Item};
use crate::raid::truck::{Pressable, Truck};
use crate::raid::{Phase, Raid};
use crate::rig::{LocalPlayer, Side};
use crate::weapons::flat::MacroKind;
use crate::world::Zone;

pub const SCENARIO: &str = "second_hour";

/// The last task while the run goes on: the director adds each step's
/// tasks ahead of it.
const IDLE: f32 = 4.5;

pub fn opening() -> Vec<Task> {
    vec![
        Task::Log("second hour: three raids past the yard".into()),
        Task::Wait(IDLE),
    ]
}

/// The run, step by step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Step {
    Kit,
    Shoot,
    Anomaly,
    Pack,
    Surge,
    Home1,
    Out2,
    Heard,
    Drone,
    Home2,
    Out3,
    Down,
    Recover,
    Done,
}

/// One moment: when it was met, and whether doing what the driver said
/// worked.
#[derive(Clone, Debug)]
pub struct Row {
    pub first: First,
    pub raid: u32,
    pub response: &'static str,
    pub right: Option<bool>,
}

#[derive(Default)]
pub struct Run {
    step: Option<Step>,
    tries: u32,
    rows: Vec<Row>,
    /// A check waiting on a response: (row, deadline in seconds).
    pending: Vec<(usize, f32)>,
    reported: bool,
    /// What the bot carried when it went down, to fetch back.
    remains: Option<Vec3>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn direct(
    opts: Res<crate::launch::LaunchOptions>,
    mut bots: Query<(Entity, &mut Bot, &Transform, &mut crate::rig::RigInput), With<LocalPlayer>>,
    mut met: MessageReader<FirstMet>,
    firsts: Res<Firsts>,
    mut raid: ResMut<Raid>,
    world: World2,
    time: Res<Time>,
    mut run: Local<Run>,
    mut commands: Commands,
) {
    if opts.scenario.as_deref() != Some(SCENARIO) {
        return;
    }
    let Some((player, mut bot, root, mut input)) = bots.iter_mut().next() else {
        return;
    };
    let now = time.elapsed_secs();
    let here = root.translation.xz();
    // The run doesn't sit out the truck's whole drive.
    if let Phase::TruckCalled { eta, .. } = &mut raid.phase {
        if *eta > 10.0 {
            *eta = 10.0;
        }
    }
    // Keep the idle task from running out while a check waits: an empty
    // list is the end of the run.
    let idle_only =
        bot.tasks.len() == 1 && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE);
    if idle_only && bot.elapsed > IDLE - 1.0 && run.step != Some(Step::Done) {
        bot.elapsed = 0.5;
    }
    if bot.tasks.is_empty() && run.step != Some(Step::Done) {
        bot.tasks.push_back(Task::Wait(IDLE));
    }
    let raid_no = raid.raids;

    // The radio: each first time met gets the bot's answer, now.
    for FirstMet(f) in met.read() {
        let (response, tasks): (&'static str, Vec<Task>) = match f {
            First::Jam(_) => ("rack it", vec![Task::Gun(MacroKind::Rack)]),
            First::Worn => ("carry on; mend it at the bench later", vec![]),
            First::Anomaly if world.nearest_is_burner(here) => ("go round it", vec![]),
            First::Anomaly => match world.throwable(here) {
                Some((item, at)) => (
                    "throw something in first",
                    vec![
                        Task::Release(Side::Left),
                        Task::Grab {
                            item,
                            side: Side::Left,
                        },
                        Task::Throw {
                            side: Side::Left,
                            at,
                        },
                        Task::Wait(1.5),
                    ],
                ),
                None => ("(nothing to throw)", vec![]),
            },
            First::PackFull => ("leave the worst of it", vec![Task::Release(Side::Left)]),
            First::Surge => match world.radio() {
                Some(radio) => (
                    "call the truck, go home",
                    vec![
                        Task::Release(Side::Left),
                        Task::Grab {
                            item: radio,
                            side: Side::Left,
                        },
                        Task::Use(Side::Left),
                    ],
                ),
                None => ("(no radio)", vec![]),
            },
            // Off from where it heard you, low, then still.
            First::Heard => (
                "move off low, then keep still",
                vec![
                    Task::WalkTo {
                        to: here + Vec2::new(-18.0, 6.0),
                        within: 1.5,
                    },
                    Task::Wait(20.0),
                    Task::Wait(20.0),
                ],
            ),
            First::DroneLock => (
                "fight it (two hands, sound rifle)",
                vec![Task::Engage { shots: 30 }],
            ),
            First::Death => ("go back for it", vec![]),
            _ => ("", vec![]),
        };
        let row = run.rows.len();
        run.rows.push(Row {
            first: *f,
            raid: raid_no,
            response,
            right: None,
        });
        bot.log.push(format!("  radio: {f:?} -> {response}"));
        if !tasks.is_empty() {
            // Ahead of whatever it was doing.
            for t in tasks.into_iter().rev() {
                bot.tasks.push_front(t);
            }
            bot.elapsed = 0.0;
            run.pending.push((row, now + 45.0));
        } else {
            run.pending.push((row, now + 3.0));
        }
    }
    // Get low for the machine that heard you.
    let low = run
        .pending
        .iter()
        .any(|(row, _)| run.rows[*row].first == First::Heard);
    if input.crouch != low {
        input.crouch = low;
    }

    // Did the answers work? (Being heard and the drone are judged at the
    // end of their time; the rest as soon as they've come right.)
    let mut still = Vec::new();
    for (row, deadline) in std::mem::take(&mut run.pending) {
        let r = &run.rows[row];
        let ok = match r.first {
            First::Jam(_) => world.gun_clear(player).unwrap_or(false),
            First::Worn => true,
            First::Anomaly if r.response == "go round it" => !world.inside_anomaly(here),
            First::Anomaly => world.anomaly_answered().unwrap_or(false),
            First::PackFull => world.hand_empty(Side::Left).unwrap_or(false),
            First::Surge => matches!(
                raid.phase,
                Phase::TruckCalled { .. } | Phase::TruckWaiting { .. }
            ),
            First::Heard => world.alive(player) && world.no_one_engaging(),
            First::DroneLock => world.alive(player) && world.drones_down(),
            _ => true,
        };
        let early = match r.first {
            First::Heard => false,
            First::DroneLock => ok,
            _ => true,
        };
        if (ok && early) || now > deadline || !world.alive(player) {
            run.rows[row].right = Some(ok);
            match run.rows[row].first {
                First::Surge => {
                    // Answered: the clock goes back, so the walk home isn't
                    // a race the run would lose to the Surge.
                    raid.clock = 0.0;
                    bot.log
                        .push("  (clock wound back for the walk home)".into());
                }
                First::Heard | First::DroneLock => {
                    // The field cleared for what comes next.
                    for e in world.robot_entities() {
                        commands.entity(e).despawn();
                    }
                }
                _ => {}
            }
        } else {
            still.push((row, deadline));
        }
    }
    run.pending = still;
    if !run.pending.is_empty() {
        return;
    }

    // Steps, when the bot has nothing left but to idle.
    let idling =
        bot.tasks.len() == 1 && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE);
    if !idling || bot.elapsed < 0.5 {
        return;
    }
    let step = run.step.unwrap_or(Step::Kit);
    let advance = |run: &mut Run, next: Step| {
        run.step = Some(next);
        run.tries = 0;
    };
    let mut tasks: Vec<Task> = Vec::new();
    use Task::*;
    match step {
        Step::Kit if world.holding(Side::Right).and_then(|e| world.id(e)) == Some("ak74") => {
            // A rifle that's seen better days.
            if let Some(gun) = world.holding(Side::Right) {
                commands.entity(gun).queue(|mut e: EntityWorldMut| {
                    if let Some(mut g) = e.get_mut::<crate::weapons::Gun>() {
                        g.firearm.condition = 0.2;
                        g.firearm.fouling = 0.5;
                    }
                });
            }
            advance(&mut run, Step::Shoot);
        }
        Step::Kit => {
            tasks.extend([
                PickUp {
                    id: "ak74".into(),
                    side: Side::Right,
                },
                PickUp {
                    id: "ak_mag".into(),
                    side: Side::Left,
                },
                Stow(Side::Left),
                PickUp {
                    id: "ak_mag".into(),
                    side: Side::Left,
                },
                Stow(Side::Left),
                Gun(MacroKind::Reload),
                Gun(MacroKind::Mode),
            ]);
        }
        Step::Shoot => {
            // Up the road at nothing, until it jams (or the mags run out).
            let at = Vec3::new(here.x, root.translation.y + 1.5, here.y - 40.0);
            if !firsts.met(First::Jam(super::firsts::JamKind::Eject))
                && !firsts.met(First::Jam(super::firsts::JamKind::Feed))
                && !firsts.met(First::Jam(super::firsts::JamKind::Double))
                && run.tries < 3
            {
                tasks.push(Fire { at, shots: 20 });
                tasks.push(Gun(MacroKind::Reload));
                run.tries += 1;
            } else {
                advance(&mut run, Step::Anomaly);
            }
        }
        Step::Anomaly => match world.anomaly_to_probe(here) {
            Some(target) if !firsts.met(First::Anomaly) && run.tries < 2 => {
                let from = here;
                let d = from.distance(target.xz());
                let legs = (d / 12.0).ceil().max(1.0) as usize;
                for k in 1..=legs {
                    tasks.push(WalkTo {
                        to: from.lerp(target.xz(), k as f32 / legs as f32),
                        within: 1.0,
                    });
                }
                run.tries += 1;
            }
            _ => advance(&mut run, Step::Pack),
        },
        Step::Pack => {
            if !firsts.met(First::PackFull) && run.tries < 12 {
                // Everything loose nearby goes in the pack, till it won't.
                if let Some(item) = world.bulky_near(here) {
                    tasks.push(Grab {
                        item,
                        side: Side::Left,
                    });
                    tasks.push(Stow(Side::Left));
                }
                run.tries += 1;
            } else {
                advance(&mut run, Step::Surge);
            }
        }
        Step::Surge => {
            if !firsts.met(First::Surge) {
                world.drop_radio(&mut commands, root.translation);
                // The clock wound on to the warning.
                raid.clock = crate::raid::SURGE_AFTER - crate::raid::SURGE_WARNING + 2.0;
                tasks.push(Wait(1.0));
            } else {
                advance(&mut run, Step::Home1);
            }
        }
        Step::Home1 | Step::Home2 => {
            if raid.phase == Phase::AtBase {
                let next = if step == Step::Home1 {
                    Step::Out2
                } else {
                    Step::Out3
                };
                bot.log.push(format!("home from raid {}", raid.raids));
                advance(&mut run, next);
            } else {
                tasks.extend(world.go_home(here, &raid, run.tries));
                run.tries += 1;
            }
        }
        Step::Out2 | Step::Out3 => {
            // Out of the truck and up the road.
            if raid.phase != Phase::AtBase {
                let next = if step == Step::Out2 {
                    Step::Heard
                } else {
                    Step::Down
                };
                advance(&mut run, next);
            } else {
                // Armed again, if what it had went down with it.
                let armed = world
                    .holding(Side::Right)
                    .is_some_and(|e| world.guns.get(e).is_ok());
                if !armed {
                    if let Some(gun) = world.any_gun(here) {
                        tasks.push(Grab {
                            item: gun,
                            side: Side::Right,
                        });
                    }
                }
                let ins = world.insertion();
                tasks.extend(world.legs(here, ins + Vec2::new(0.0, -25.0)));
            }
        }
        Step::Heard => {
            if !firsts.met(First::Heard) && run.tries < 2 {
                // A walker about its own business off to one side, and a
                // shot it can't miss hearing.
                let o = here;
                let at = Vec3::new(o.x + 30.0, 0.0, o.y - 30.0);
                let site = world.zone.on_ground(o.x + 60.0, o.y - 50.0);
                crate::robots::walker::spawn_walker(
                    &mut commands,
                    &world.zone,
                    at,
                    crate::robots::Job {
                        kind: crate::robots::JobKind::Survey,
                        site,
                        duration: 180.0,
                        exit: site + Vec3::new(0.0, 0.0, -300.0),
                    },
                );
                tasks.push(Wait(3.0));
                tasks.push(Fire {
                    at: Vec3::new(o.x, root.translation.y + 8.0, o.y - 20.0),
                    shots: 1,
                });
                tasks.push(Wait(15.0));
                run.tries += 1;
            } else {
                advance(&mut run, Step::Drone);
            }
        }
        Step::Drone => {
            let armed = world
                .holding(Side::Right)
                .is_some_and(|e| world.guns.get(e).is_ok());
            if !armed && run.tries < 2 {
                // A rifle and a magazine, if what it had is gone.
                world.drop_kit(&mut commands, root.translation);
                tasks.push(Wait(0.5));
                tasks.push(PickUp {
                    id: "ak74".into(),
                    side: Side::Right,
                });
                tasks.push(PickUp {
                    id: "ak_mag".into(),
                    side: Side::Left,
                });
                tasks.push(Stow(Side::Left));
                tasks.push(Gun(MacroKind::Reload));
                tasks.push(Gun(MacroKind::Mode));
                run.tries += 1;
            } else if world.held_condition(player) < 0.5 && run.tries < 2 {
                // Not a drone with a worn rifle: the worn one on the sling,
                // a sound one in hand.
                world.drop_kit(&mut commands, root.translation);
                tasks.push(Stow(Side::Right));
                tasks.push(Wait(0.5));
                tasks.push(PickUp {
                    id: "ak74".into(),
                    side: Side::Right,
                });
                tasks.push(PickUp {
                    id: "ak_mag".into(),
                    side: Side::Left,
                });
                tasks.push(Stow(Side::Left));
                tasks.push(Gun(MacroKind::Reload));
                tasks.push(Gun(MacroKind::Mode));
                run.tries += 1;
            } else if !firsts.met(First::DroneLock) && run.tries < 4 {
                tasks.push(Gun(MacroKind::Reload));
                tasks.push(Support);
                tasks.push(Spawn {
                    kind: crate::robots::RobotKind::Drone,
                    at: Vec3::new(
                        here.x,
                        world.zone.ground(here.x, here.y - 40.0) + 12.0,
                        here.y - 40.0,
                    ),
                });
                tasks.push(Wait(6.0));
                run.tries += 1;
            } else {
                advance(&mut run, Step::Home2);
            }
        }
        Step::Down => {
            if !firsts.met(First::Death) {
                if !matches!(raid.phase, Phase::Down { .. }) && raid.phase != Phase::AtBase {
                    run.remains = Some(root.translation);
                    world.fall(&mut commands, player);
                }
                tasks.push(Wait(3.0));
            } else {
                advance(&mut run, Step::Recover);
            }
        }
        Step::Recover => {
            // Back out to where it fell, for what it carried.
            match (
                world.remains_at(player).or(run.remains),
                world.remains_item(player),
            ) {
                (Some(at), Some(item)) if run.tries < 3 => {
                    tasks.extend(world.legs(here, at.xz()));
                    tasks.push(Grab {
                        item,
                        side: Side::Right,
                    });
                    run.tries += 1;
                }
                _ => {
                    let back = world
                        .holding(Side::Right)
                        .is_some_and(|e| world.came_from_remains(e));
                    run.rows.push(Row {
                        first: First::KitRecovered,
                        raid: raid_no,
                        response: "went back for it before the convoy",
                        right: Some(back),
                    });
                    advance(&mut run, Step::Done);
                }
            }
        }
        Step::Done => {
            if !run.reported {
                run.reported = true;
                report(&mut bot, &run.rows);
            }
        }
    }
    if !tasks.is_empty() {
        let idle = bot.tasks.pop_back();
        bot.tasks.extend(tasks);
        bot.tasks.extend(idle);
    }
}

/// The table, in the bot's log; a moment counts as passed when it was met
/// and the driver's advice worked.
fn report(bot: &mut Bot, rows: &[Row]) {
    bot.log.push("| Moment | Raid | Bot did | Worked |".into());
    bot.log.push("| - | - | - | - |".into());
    let mut good = 0;
    for r in rows {
        let worked = match r.right {
            Some(true) => {
                good += 1;
                "yes"
            }
            Some(false) => "no",
            None => "?",
        };
        bot.log.push(format!(
            "| {:?} | {} | {} | {} |",
            r.first, r.raid, r.response, worked
        ));
    }
    let wanted = [
        First::Worn,
        First::Anomaly,
        First::PackFull,
        First::Surge,
        First::Heard,
        First::Death,
    ];
    let missing: Vec<_> = wanted
        .iter()
        .filter(|w| !rows.iter().any(|r| r.first == **w))
        .collect();
    let jams = rows.iter().any(|r| matches!(r.first, First::Jam(_)));
    if missing.is_empty() && jams && good + 1 >= rows.len() {
        bot.passed += 1;
        bot.log.push(format!(
            "PASS {} moments met, {good} answered right",
            rows.len()
        ));
    } else {
        bot.failed += 1;
        bot.log.push(format!(
            "FAIL moments missing {missing:?}{}; {good} of {} answered right",
            if jams { "" } else { ", no jam" },
            rows.len()
        ));
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct World2<'w, 's> {
    zone: Res<'w, Zone>,
    catalog: Res<'w, Catalog>,
    items: Query<
        'w,
        's,
        (
            Entity,
            &'static Item,
            &'static GlobalTransform,
            Option<&'static Held>,
            Option<&'static crate::interact::InSocket>,
            Option<&'static crate::interact::body::InBackpack>,
        ),
    >,
    guns: Query<'w, 's, (&'static crate::weapons::Gun, &'static Held)>,
    hands: Query<'w, 's, (&'static crate::rig::RigHand, &'static crate::interact::Hand)>,
    anomalies: Query<'w, 's, (&'static crate::anomaly::Anomaly, &'static GlobalTransform)>,
    anomaly_events: MessageReader<'w, 's, crate::anomaly::AnomalyEvent>,
    robots: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::robots::Robot,
            &'static crate::robots::Brain,
        ),
    >,
    health: Query<'w, 's, &'static crate::health::Health>,
    bags: Query<'w, 's, (&'static Backpack, Option<&'static crate::raid::Remains>)>,
    pressables: Query<'w, 's, (Entity, &'static Pressable)>,
    remains: Query<'w, 's, (&'static GlobalTransform, &'static crate::raid::Remains)>,
    trucks: Query<'w, 's, &'static GlobalTransform, With<Truck>>,
}

impl World2<'_, '_> {
    fn id(&self, e: Entity) -> Option<&str> {
        self.items
            .get(e)
            .ok()
            .map(|(_, it, ..)| self.catalog.get(it.kind()).id.as_str())
    }

    fn holding(&self, side: Side) -> Option<Entity> {
        self.hands
            .iter()
            .find(|(rh, _)| rh.side == side)
            .and_then(|(_, h)| h.holding)
    }

    fn hand_empty(&self, side: Side) -> Option<bool> {
        Some(self.holding(side).is_none())
    }

    fn held_condition(&self, player: Entity) -> f32 {
        self.guns
            .iter()
            .find(|(_, h)| h.player == player)
            .map_or(1.0, |(g, _)| g.firearm.condition)
    }

    fn gun_clear(&self, player: Entity) -> Option<bool> {
        self.guns
            .iter()
            .find(|(_, h)| h.player == player)
            .map(|(g, _)| g.firearm.malfunction.is_none())
    }

    /// Something loose within a few metres to throw, and where to throw it:
    /// at the nearest anomaly that answers a thrown thing (not a burner).
    fn throwable(&self, near: Vec2) -> Option<(Entity, Vec3)> {
        let target = self.anomaly_to_probe(near)?;
        let item = self
            .items
            .iter()
            .filter(|(_, it, gt, held, sock, bag)| {
                held.is_none()
                    && sock.is_none()
                    && bag.is_none()
                    && gt.translation().xz().distance(near) < 6.0
                    && self.catalog.get(it.kind()).category
                        != extraction_core::items::Category::Artifact
            })
            .map(|(e, ..)| e)
            .next()
            .or_else(|| {
                // Or whatever's in the pack.
                self.bags
                    .iter()
                    .find(|(b, r)| b.carried && r.is_none())
                    .and_then(|(b, _)| b.contents.first().copied())
            })?;
        Some((item, target + Vec3::Y * 0.5))
    }

    fn nearest_anomaly(&self, near: Vec2) -> Option<(&crate::anomaly::Anomaly, Vec3)> {
        self.anomalies
            .iter()
            .map(|(a, gt)| (a, gt.translation()))
            .min_by(|a, b| {
                (a.1.xz().distance(near) - a.0.radius)
                    .total_cmp(&(b.1.xz().distance(near) - b.0.radius))
            })
    }

    fn nearest_is_burner(&self, near: Vec2) -> bool {
        self.nearest_anomaly(near)
            .is_some_and(|(a, _)| a.kind == extraction_core::anomaly::AnomalyKind::Burner)
    }

    fn inside_anomaly(&self, near: Vec2) -> bool {
        self.anomalies
            .iter()
            .any(|(a, gt)| gt.translation().xz().distance(near) < a.radius)
    }

    fn anomaly_to_probe(&self, near: Vec2) -> Option<Vec3> {
        self.anomalies
            .iter()
            .filter(|(a, _)| a.kind != extraction_core::anomaly::AnomalyKind::Burner)
            .map(|(_, gt)| gt.translation())
            .min_by(|a, b| a.xz().distance(near).total_cmp(&b.xz().distance(near)))
    }

    fn anomaly_answered(&self) -> Option<bool> {
        // (Read by the director each frame: any lift popping or snare
        // discharging since the throw.)
        Some(!self.anomaly_events.is_empty())
    }

    fn bulky_near(&self, near: Vec2) -> Option<Entity> {
        self.items
            .iter()
            .filter(|(_, it, gt, held, sock, bag)| {
                held.is_none()
                    && sock.is_none()
                    && bag.is_none()
                    && gt.translation().xz().distance(near) < 40.0
                    && self.catalog.get(it.kind()).category
                        != extraction_core::items::Category::Artifact
            })
            .max_by(|a, b| {
                let bulk = |e: &crate::items::Item| self.catalog.get(e.kind()).bulk;
                bulk(a.1).total_cmp(&bulk(b.1))
            })
            .map(|(e, ..)| e)
    }

    /// A rifle and a full magazine at the bot's feet.
    fn drop_kit(&self, commands: &mut Commands, at: Vec3) {
        for (k, id) in ["ak74", "ak_mag"].into_iter().enumerate() {
            let Some(kind) = self.catalog.kind(id) else {
                continue;
            };
            let e = crate::items::spawn_item(
                commands,
                &self.catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        at + Vec3::new(0.5 + 0.3 * k as f32, 1.0, -0.5),
                    ),
                    condition: 1.0,
                },
            );
            if id == "ak_mag" {
                commands.entity(e).queue(|mut w: EntityWorldMut| {
                    if let Some(mut it) = w.get_mut::<Item>() {
                        it.rounds = 30;
                    }
                });
            }
        }
    }

    fn radio(&self) -> Option<Entity> {
        self.items
            .iter()
            .filter(|(e, _, _, held, sock, bag)| {
                held.is_none() && sock.is_none() && bag.is_none() && self.id(*e) == Some("radio")
            })
            .map(|(e, ..)| e)
            .next()
    }

    fn drop_radio(&self, commands: &mut Commands, at: Vec3) {
        let Some(kind) = self.catalog.kind("radio") else {
            return;
        };
        crate::items::spawn_item(
            commands,
            &self.catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_translation(at + Vec3::new(0.4, 1.0, -0.4)),
                condition: 1.0,
            },
        );
    }

    fn no_one_engaging(&self) -> bool {
        !self
            .robots
            .iter()
            .any(|(_, r, b)| !r.wrecked && matches!(b, crate::robots::Brain::Engage))
    }

    fn drones_down(&self) -> bool {
        !self
            .robots
            .iter()
            .any(|(_, r, _)| !r.wrecked && r.kind == crate::robots::RobotKind::Drone)
    }

    fn remains_at(&self, player: Entity) -> Option<Vec3> {
        self.remains
            .iter()
            .find(|(_, r)| r.owner == player)
            .map(|(gt, _)| gt.translation())
    }

    fn robot_entities(&self) -> Vec<Entity> {
        self.robots.iter().map(|(e, ..)| e).collect()
    }

    fn alive(&self, player: Entity) -> bool {
        self.health.get(player).is_ok_and(|h| !h.down)
    }

    fn fall(&self, commands: &mut Commands, player: Entity) {
        commands.entity(player).queue(|mut e: EntityWorldMut| {
            if let Some(mut h) = e.get_mut::<crate::health::Health>() {
                h.hp = 0.0;
            }
        });
    }

    fn remains_item(&self, player: Entity) -> Option<Entity> {
        self.bags
            .iter()
            .find(|(_, r)| r.is_some_and(|r| r.owner == player))
            .and_then(|(b, _)| b.contents.first().copied())
    }

    fn came_from_remains(&self, item: Entity) -> bool {
        // It's in hand now; the remains bag no longer has it.
        !self
            .bags
            .iter()
            .any(|(b, r)| r.is_some() && b.contents.contains(&item))
    }

    fn insertion(&self) -> Vec2 {
        self.zone
            .marker(&MarkerKind::Insertion)
            .map(|m| Vec2::from(m.at))
            .unwrap_or(Vec2::ZERO)
    }

    /// Straight legs a bot can do inside its per-task time.
    fn legs(&self, from: Vec2, to: Vec2) -> Vec<Task> {
        let d = from.distance(to);
        let n = (d / 14.0).ceil().max(1.0) as usize;
        (1..=n)
            .map(|k| Task::WalkTo {
                to: from.lerp(to, k as f32 / n as f32),
                within: if k == n { 1.0 } else { 2.0 },
            })
            .collect()
    }

    /// Home: radio called (by the Surge step); wait for the truck, walk to
    /// it, ring.
    fn go_home(&self, here: Vec2, raid: &Raid, tries: u32) -> Vec<Task> {
        match raid.phase {
            Phase::Out => match self.radio_in_hand() {
                true => vec![Task::Use(Side::Left), Task::Wait(1.0)],
                false => match self.radio() {
                    Some(r) => vec![
                        Task::Release(Side::Left),
                        Task::Grab {
                            item: r,
                            side: Side::Left,
                        },
                    ],
                    None => vec![Task::Wait(1.0)],
                },
            },
            Phase::TruckCalled { .. } => vec![Task::Wait(20.0)],
            Phase::TruckWaiting { .. } => {
                // Round to the back and up its steps, as a person would
                // (straight at it, the bot met the truck's side).
                // Then up the middle of the bed, left of the bench, to just
                // past its front: the cord is in reach there.
                let Some(truck) = self.trucks.iter().next() else {
                    return vec![Task::Wait(1.0)];
                };
                let at = |x: f32, z: f32| truck.transform_point(Vec3::new(x, 0.0, z)).xz();
                let mut t = self.legs(here, at(0.0, 4.0));
                for (x, z) in [(0.05, 0.3), (0.05, -1.2)] {
                    t.push(Task::WalkTo {
                        to: at(x, z),
                        within: 0.3,
                    });
                }
                if let Some((bell, _)) = self
                    .pressables
                    .iter()
                    .find(|(_, p)| **p == Pressable::BellPull)
                {
                    t.push(Task::Press(bell));
                }
                t.push(Task::Wait(if tries > 0 { 3.0 } else { 1.0 }));
                t
            }
            _ => vec![Task::Wait(3.0)],
        }
    }

    /// A gun lying loose, or in the crate.
    /// A gun lying loose nearby (not one out in its remains).
    fn any_gun(&self, near: Vec2) -> Option<Entity> {
        self.items
            .iter()
            .filter(|(e, _, gt, held, sock, bag)| {
                held.is_none()
                    && sock.is_none()
                    && bag.is_none()
                    && gt.translation().xz().distance(near) < 30.0
                    && matches!(self.id(*e), Some("ak74" | "pm_pistol" | "sks" | "mosin"))
            })
            .map(|(e, ..)| e)
            .next()
    }

    fn radio_in_hand(&self) -> bool {
        self.holding(Side::Left)
            .is_some_and(|e| self.id(e) == Some("radio"))
    }
}
