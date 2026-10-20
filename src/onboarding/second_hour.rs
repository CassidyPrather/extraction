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
    pending: Option<(usize, f32)>,
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
            First::Heard => (
                "get low, keep still",
                vec![Task::Wait(20.0), Task::Wait(20.0)],
            ),
            First::DroneLock => (
                "get behind something / fight",
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
            run.pending = Some((row, now + 45.0));
        } else {
            run.pending = Some((row, now + 3.0));
        }
    }
    // Get low for the machine that heard you.
    let low = run.rows.last().is_some_and(|r| {
        r.first == First::Heard && run.pending.is_some_and(|p| p.0 + 1 == run.rows.len())
    });
    if input.crouch != low {
        input.crouch = low;
    }

    // Did the answer work?
    if let Some((row, deadline)) = run.pending {
        let r = &run.rows[row];
        let ok = match r.first {
            First::Jam(_) => world.gun_clear(player),
            First::Worn => Some(true),
            First::Anomaly if r.response == "go round it" => Some(!world.inside_anomaly(here)),
            First::Anomaly => world.anomaly_answered(),
            First::PackFull => world.hand_empty(Side::Left),
            First::Surge => Some(matches!(
                raid.phase,
                Phase::TruckCalled { .. } | Phase::TruckWaiting { .. }
            )),
            First::Heard => Some(firsts.met(First::LostYou) || world.no_one_engaging()),
            First::DroneLock => Some(world.alive(player) && world.drones_down()),
            First::Death => Some(true),
            _ => Some(true),
        };
        let settled = bot.tasks.len() <= 1;
        if ok == Some(true) && settled || now > deadline {
            run.rows[row].right = Some(ok == Some(true));
            run.pending = None;
            if run.rows[row].first == First::Surge {
                // Answered: the clock goes back, so the walk home isn't a
                // race the run would lose to the Surge.
                raid.clock = 0.0;
                bot.log
                    .push("  (clock wound back for the walk home)".into());
            }
        }
        if run.pending.is_some() {
            return;
        }
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
                let ins = world.insertion();
                tasks.extend(world.legs(here, ins + Vec2::new(0.0, -25.0)));
            }
        }
        Step::Heard => {
            if !firsts.met(First::Heard) && run.tries < 2 {
                let o = here;
                tasks.push(Spawn {
                    kind: crate::robots::RobotKind::Walker,
                    at: Vec3::new(o.x, 0.0, o.y - 35.0),
                });
                tasks.push(Wait(12.0));
                run.tries += 1;
            } else {
                advance(&mut run, Step::Drone);
            }
        }
        Step::Drone => {
            if !firsts.met(First::DroneLock) && run.tries < 2 {
                tasks.push(Gun(MacroKind::Reload));
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
            match (run.remains, world.remains_item(player)) {
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
    robots: Query<'w, 's, (&'static crate::robots::Robot, &'static crate::robots::Brain)>,
    health: Query<'w, 's, &'static crate::health::Health>,
    bags: Query<'w, 's, (&'static Backpack, Option<&'static crate::raid::Remains>)>,
    pressables: Query<'w, 's, (Entity, &'static Pressable)>,
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
            .any(|(r, b)| !r.wrecked && matches!(b, crate::robots::Brain::Engage))
    }

    fn drones_down(&self) -> bool {
        !self
            .robots
            .iter()
            .any(|(r, _)| !r.wrecked && r.kind == crate::robots::RobotKind::Drone)
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
                let Some(truck) = self.trucks.iter().next().map(|g| g.translation().xz()) else {
                    return vec![Task::Wait(1.0)];
                };
                let mut t = self.legs(here, truck);
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

    fn radio_in_hand(&self) -> bool {
        self.holding(Side::Left)
            .is_some_and(|e| self.id(e) == Some("radio"))
    }
}
