//! Bots: players driven by a task list instead of a person.
//!
//! A bot writes its rig's [`RigInput`] (head direction, walking, trigger)
//! and sends the same [`HandAction`]s flat mode does, so everything a bot
//! does exercises the real game. Scenarios ([`scenarios`]) are task lists
//! with expectations; run one with `--scenario NAME` (add `--headless` to
//! run faster than real time with no window). Map simulations build on the
//! same tasks.

pub mod scenarios;

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::interact::body::{Backpack, ToggleBackpack};
use crate::interact::{Hand, HandAction, Held, InSocket};
use crate::items::{Catalog, Item};
use crate::player::{Body, Player};
use crate::rig::{RigDriverSystems, RigHand, RigInput, Side};
use crate::weapons::ballistics::Hit;
use crate::weapons::flat::{Combine, GunMacro, MacroKind, macro_steps};
use crate::weapons::{Gun, Mag};

/// One thing for a bot to do.
#[derive(Clone, Debug)]
pub enum Task {
    Wait(f32),
    /// Walk to a point on the ground.
    WalkTo {
        to: Vec2,
        within: f32,
    },
    /// Turn the head toward a point.
    LookAt(Vec3),
    /// Find the nearest loose item with this catalog id, walk to it, grab it.
    PickUp {
        id: String,
        side: Side,
    },
    /// Put away what's in a hand (body slot, else backpack).
    Stow(Side),
    /// Take from body slot `slot` into a hand.
    Draw {
        slot: usize,
        side: Side,
    },
    Release(Side),
    /// Second hand onto the gun's handguard.
    Support,
    /// A gun-handling macro with the right-hand gun.
    Gun(MacroKind),
    /// Aim at a point and pull the trigger `shots` times (or hold for full auto).
    Fire {
        at: Vec3,
        shots: u32,
    },
    /// Bring the two held items together.
    Combine,
    ToggleBackpack,
    /// Spawn a robot here (scenario setup), already on the bot's trail.
    Spawn {
        kind: crate::robots::RobotKind,
        at: Vec3,
    },
    /// Put an anomaly here holding a stone (`at` on the ground), and loose
    /// kit to work it with (anomaly harvesting scenarios).
    Stage {
        kind: extraction_core::anomaly::AnomalyKind,
        at: Vec3,
        radius: f32,
        artifact: String,
        kit: Vec<(String, Vec3)>,
    },
    /// Fight the nearest robot: aim at its core, shoot until it's wrecked
    /// or `shots` trigger pulls are spent.
    Engage {
        shots: u32,
    },
    Expect(Expect),
    Log(String),
    /// Stand somewhere else at once (scenario setup: skip the walk).
    Teleport(Vec3),
    /// Walk within reach of this item and grab it (for bots that choose
    /// what they want, rather than "the nearest of an id").
    Grab {
        item: Entity,
        side: Side,
    },
    /// Reach for this item from where the bot stands, without walking up
    /// to it (what a far grab does: something up high, or in an anomaly).
    GrabFromHere {
        item: Entity,
        side: Side,
    },
    /// Walk within reach of a pressable (bell pull, button) and press it.
    Press(Entity),
    /// Walk to a door and press it if it's shut; done when it's open.
    OpenDoor(Entity),
    /// Use the item in this hand (radio, dressing, food): what VR's primary
    /// button and flat mode's U do.
    Use(Side),
    /// Throw what's in this hand at a point (a bolt into an anomaly).
    Throw {
        side: Side,
        at: Vec3,
    },
    /// Shoot at one robot's core with the right-hand gun: `shots` trigger
    /// pulls, or until it's wrecked or the gun is empty.
    Shoot {
        target: Entity,
        shots: u32,
    },
    /// Stage a scenario's world by name, where the bot stands: the module
    /// under test answers [`scenarios::Setup`] (a heavy and a well).
    Setup(&'static str),
}

impl Task {
    /// The same task with its hands swapped: scenarios are written for a
    /// right-handed bot, and a left-handed one plays them mirrored.
    pub fn mirrored(self, m: bool) -> Self {
        if !m {
            return self;
        }
        match self {
            Task::PickUp { id, side } => Task::PickUp {
                id,
                side: side.other(),
            },
            Task::Stow(side) => Task::Stow(side.other()),
            Task::Draw { slot, side } => Task::Draw {
                slot,
                side: side.other(),
            },
            Task::Release(side) => Task::Release(side.other()),
            Task::Grab { item, side } => Task::Grab {
                item,
                side: side.other(),
            },
            Task::GrabFromHere { item, side } => Task::GrabFromHere {
                item,
                side: side.other(),
            },
            Task::Use(side) => Task::Use(side.other()),
            Task::Throw { side, at } => Task::Throw {
                side: side.other(),
                at,
            },
            Task::Expect(Expect::Holding(side, id)) => {
                Task::Expect(Expect::Holding(side.other(), id))
            }
            other => other,
        }
    }
}

/// How far a bot reaches for things, metres (horizontal, from the body).
pub const REACH: f32 = 1.2;
/// How quickly a bot's body swings onto its aim point, per second.
const AIM_RATE: f32 = 14.0;

/// Checks a scenario makes along the way.
#[derive(Clone, Debug)]
pub enum Expect {
    /// The hand holds an item with this id.
    Holding(Side, String),
    /// Live rounds in the right-hand gun, inclusive range.
    GunRounds(u32, u32),
    /// At least this many of the bot's shots have hit something.
    Hits(u32),
    /// Carried mass at least this, kg.
    CarryingAtLeast(f32),
    /// Carried mass is at most this, kg.
    CarryingAtMost(f32),
    /// Items in the backpack, inclusive range.
    PackItems(usize, usize),
    /// The bot is within this distance of a point.
    Near(Vec2, f32),
    /// At least this many robots are wrecks.
    Wrecked(usize),
    /// At least this many of the bot's shots have struck a robot.
    RobotHits(u32),
    /// The bot is still standing.
    Alive,
    /// The player would have heard at least this many robots' words (of
    /// this mind, if given): `robot_voice::Heard`, in the listening
    /// scenario.
    Heard(Option<crate::robot_voice::Utterance>, usize),
}

#[derive(Component, Debug)]
pub struct Bot {
    pub tasks: VecDeque<Task>,
    /// Seconds spent on the current task.
    pub elapsed: f32,
    /// Shots fired in the current Fire task.
    pub fired: u32,
    pub look: Vec3,
    pub walk: Option<Vec2>,
    pub aim: bool,
    pub trigger: bool,
    /// Run instead of walking (stamina permitting).
    pub sprint: bool,
    /// The last aim point and when, for leading a moving target.
    track: Option<(Vec3, f32)>,
    pub hits: u32,
    /// Of those, how many struck a robot.
    pub robot_hits: u32,
    pub passed: u32,
    pub failed: u32,
    pub log: Vec<String>,
    /// Waiting for an action issued this task to land.
    pending: bool,
    /// Seconds since the pending action was issued.
    retry: f32,
}

impl Bot {
    pub fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks: tasks.into(),
            elapsed: 0.0,
            fired: 0,
            look: Vec3::NEG_Z,
            walk: None,
            aim: false,
            trigger: false,
            sprint: false,
            track: None,
            hits: 0,
            robot_hits: 0,
            passed: 0,
            failed: 0,
            log: Vec::new(),
            pending: false,
            retry: 0.0,
        }
    }

    pub fn done(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Drops whatever it was doing (hands stay as they are) and takes on
    /// these tasks instead: for brains that react to the world.
    pub fn replace(&mut self, tasks: impl IntoIterator<Item = Task>) {
        self.tasks = tasks.into_iter().collect();
        self.elapsed = 0.0;
        self.fired = 0;
        self.pending = false;
        self.retry = 0.0;
        self.walk = None;
        self.aim = false;
        self.trigger = false;
    }

    /// Adds tasks after the current ones.
    pub fn extend(&mut self, tasks: impl IntoIterator<Item = Task>) {
        self.tasks.extend(tasks);
    }

    fn note(&mut self, msg: String) {
        info!("bot: {msg}");
        self.log.push(msg);
    }

    fn next(&mut self) {
        self.tasks.pop_front();
        self.elapsed = 0.0;
        self.fired = 0;
        self.pending = false;
        self.retry = 0.0;
        self.trigger = false;
    }
}

pub struct BotPlugin;

impl Plugin for BotPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, drive_bots.in_set(RigDriverSystems))
            .add_systems(Update, (run_tasks, count_hits));
    }
}

/// Bots' rig input: head toward the look target, walking toward the walk
/// target, hands in the ready pose (or aimed down the sights).
fn drive_bots(
    mut bots: Query<
        (
            &Bot,
            &mut RigInput,
            &mut Transform,
            &crate::weapons::AimHint,
            &Children,
        ),
        With<Player>,
    >,
    hands: Query<(&RigHand, &Hand)>,
    time: Res<Time>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
) {
    let gun_side = comfort.as_ref().map_or(Side::Right, |c| c.gun_side());
    for (bot, mut input, mut root, hint, children) in &mut bots {
        let supporting = children.iter().any(|c| {
            hands
                .get(c)
                .is_ok_and(|(rh, h)| rh.side == gun_side.other() && h.supporting.is_some())
        });
        // Crouched, the head comes down as flat mode's C does (and the
        // body with it).
        let eye = Vec3::new(0.0, if input.crouch { 1.05 } else { 1.68 }, 0.0);
        let world_eye = root.translation + eye;
        let to = (bot.look - world_eye).normalize_or(Vec3::NEG_Z);
        // Turn the body toward the target's bearing, smoothly.
        let target_yaw = (-to.x).atan2(-to.z);
        let (cur_yaw, _, _) = root.rotation.to_euler(EulerRot::YXZ);
        let mut d = target_yaw - cur_yaw;
        while d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        while d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        // Turning to aim is quicker than turning to walk.
        let rate = if bot.aim { AIM_RATE } else { 6.0 };
        let step = (rate * time.delta_secs()).min(1.0);
        root.rotation = Quat::from_rotation_y(cur_yaw + d * step);
        let pitch = to.y.clamp(-1.0, 1.0).asin();
        let head_rot = Quat::from_rotation_x(pitch);
        input.head = Transform::from_translation(eye).with_rotation(head_rot);
        input.locomotion = match bot.walk {
            Some(target) => {
                let delta = Vec2::new(target.x - root.translation.x, target.y - root.translation.z);
                if delta.length() < 0.2 {
                    Vec2::ZERO
                } else {
                    // Into head-relative axes (forward = -Z of the root).
                    let local = root.rotation.inverse() * Vec3::new(delta.x, 0.0, delta.y);
                    Vec2::new(local.x, -local.z).normalize_or_zero()
                }
            }
            None => Vec2::ZERO,
        };
        input.sprint = bot.sprint && bot.walk.is_some();
        for side in Side::BOTH {
            let offset = Vec3::new(0.2 * side.sign(), -0.32, -0.38);
            let pose = Transform::from_translation(eye + head_rot * offset)
                .with_rotation(head_rot * Quat::from_rotation_x(-0.3));
            let hand = input.hand_mut(side);
            hand.pose = pose;
            hand.tracked = true;
            hand.trigger = 0.0;
        }
        if bot.aim {
            // Sights on the eye line.
            let sight = hint.sight;
            let eye_target =
                Transform::from_translation(eye + head_rot * Vec3::new(0.0, 0.0, -0.32))
                    .with_rotation(head_rot);
            let right = input.hand_mut(gun_side);
            right.pose = Transform::from_matrix(
                (eye_target.compute_affine() * sight.compute_affine().inverse()).into(),
            );
            right.trigger = if bot.trigger { 1.0 } else { 0.0 };
        }
        if let (true, Some(support)) = (supporting, hint.support) {
            let right_pose = input.hand(gun_side).pose;
            input.hand_mut(gun_side.other()).pose =
                Transform::from_translation(right_pose.transform_point(support))
                    .with_rotation(right_pose.rotation);
        }
    }
}

fn count_hits(
    mut hits: MessageReader<Hit>,
    mut bots: Query<(Entity, &mut Bot)>,
    parts: Query<(), With<crate::robots::RobotPart>>,
) {
    for hit in hits.read() {
        for (e, mut bot) in &mut bots {
            if hit.shooter == Some(e) {
                bot.hits += 1;
                if parts.contains(hit.entity) {
                    bot.robot_hits += 1;
                }
            }
        }
    }
}

/// What the newer tasks need, bundled to keep `run_tasks` under Bevy's
/// parameter limit.
#[derive(bevy::ecs::system::SystemParam)]
struct TaskExtras<'w, 's> {
    placed: Query<'w, 's, (&'static GlobalTransform, Option<&'static Held>)>,
    doors: Query<'w, 's, &'static crate::world::buildings::Door>,
    parts: Query<'w, 's, (&'static crate::robots::RobotPart, &'static GlobalTransform)>,
    uses: MessageWriter<'w, crate::items::usage::UseItem>,
    heard: Option<Res<'w, crate::robot_voice::Heard>>,
    setups: MessageWriter<'w, scenarios::Setup>,
}

/// Advances each bot's current task.
fn run_tasks(
    mut bots: Query<(
        Entity,
        &mut Bot,
        &Transform,
        &Body,
        &Children,
        &mut GunMacro,
    )>,
    hands: Query<(&RigHand, &Hand)>,
    loose: Query<
        (Entity, &Item, &GlobalTransform),
        (
            Without<Held>,
            Without<InSocket>,
            Without<crate::interact::body::InBackpack>,
        ),
    >,
    items: Query<&Item>,
    guns: Query<&Gun>,
    mags: Query<&Mag>,
    bags: Query<&Backpack>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut actions: MessageWriter<HandAction>,
    mut combine: MessageWriter<Combine>,
    mut toggle: MessageWriter<ToggleBackpack>,
    robots: Query<(Entity, &crate::robots::Robot, &GlobalTransform)>,
    (zone, healths, mut ext): (
        Res<crate::world::Zone>,
        Query<&crate::health::Health>,
        TaskExtras,
    ),
    mut commands: Commands,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
) {
    let dt = time.delta_secs();
    // The gun hand, and the other: a left-handed bot plays mirrored.
    let mirror = comfort.as_ref().is_some_and(|c| c.left_handed);
    let gun_side = Side::Right.mirrored(mirror);
    let off_side = gun_side.other();
    for (player, mut bot, root, body, children, mut mac) in &mut bots {
        let Some(task) = bot.tasks.front().cloned() else {
            continue;
        };
        let task = task.mirrored(mirror);
        bot.elapsed += dt;
        let hand = |side: Side| {
            children.iter().find_map(|c| {
                hands
                    .get(c)
                    .ok()
                    .filter(|(rh, _)| rh.side == side)
                    .map(|(_, h)| h)
            })
        };
        let holding_id = |side: Side| {
            hand(side)
                .and_then(|h| h.holding)
                .and_then(|e| items.get(e).ok())
                .map(|i| catalog.get(i.kind()).id.clone())
        };
        let pos = Vec2::new(root.translation.x, root.translation.z);
        // Give up on anything that takes absurdly long.
        if bot.elapsed > 30.0 {
            bot.failed += 1;
            bot.note(format!("FAIL timed out on {task:?}"));
            bot.walk = None;
            bot.aim = false;
            bot.next();
            continue;
        }
        match task {
            Task::Wait(s) => {
                if bot.elapsed >= s {
                    bot.next();
                }
            }
            Task::Log(msg) => {
                bot.note(msg);
                bot.next();
            }
            Task::Teleport(to) => {
                let mut t = *root;
                t.translation = zone.on_ground(to.x, to.z);
                commands.entity(player).insert(t);
                bot.walk = None;
                bot.next();
            }
            Task::Setup(name) => {
                ext.setups.write(scenarios::Setup {
                    name,
                    at: root.translation,
                    facing: root.forward().as_vec3(),
                });
                bot.next();
            }
            Task::LookAt(p) => {
                bot.look = p;
                if bot.elapsed > 0.4 {
                    bot.next();
                }
            }
            Task::WalkTo { to, within } => {
                bot.walk = Some(to);
                bot.look = Vec3::new(to.x, root.translation.y + 1.6, to.y);
                if pos.distance(to) <= within {
                    bot.walk = None;
                    bot.next();
                }
            }
            Task::PickUp { id, side } => {
                if holding_id(side).as_deref() == Some(id.as_str()) {
                    bot.walk = None;
                    bot.note(format!("picked up {id}"));
                    bot.next();
                    continue;
                }
                let nearest = loose
                    .iter()
                    .filter(|(_, it, _)| catalog.get(it.kind()).id == id)
                    .min_by(|a, b| {
                        a.2.translation()
                            .distance(root.translation)
                            .total_cmp(&b.2.translation().distance(root.translation))
                    });
                let Some((e, _, gt)) = nearest else {
                    bot.failed += 1;
                    bot.note(format!("FAIL no loose {id} to pick up"));
                    bot.next();
                    continue;
                };
                let at = gt.translation();
                bot.look = at;
                let flat = Vec2::new(at.x, at.z);
                if pos.distance(flat) > 1.2 {
                    bot.walk = Some(flat);
                } else {
                    bot.walk = None;
                    bot.retry += dt;
                    if !bot.pending || bot.retry > 2.0 {
                        actions.write(HandAction::Grab {
                            player,
                            side,
                            item: e,
                            point: None,
                        });
                        bot.pending = true;
                        bot.retry = 0.0;
                    }
                }
            }
            Task::Stow(side) => {
                if !bot.pending {
                    actions.write(HandAction::Stow { player, side });
                    bot.pending = true;
                } else if bot.elapsed > 0.2 {
                    bot.next();
                }
            }
            Task::Draw { slot, side } => {
                if !bot.pending {
                    actions.write(HandAction::Draw { player, side, slot });
                    bot.pending = true;
                } else if bot.elapsed > 0.3 {
                    bot.next();
                }
            }
            Task::Release(side) => {
                actions.write(HandAction::Release {
                    player,
                    side,
                    throw: 0.0,
                });
                bot.next();
            }
            Task::Support => {
                if hand(off_side).is_some_and(|h| h.supporting.is_some()) {
                    bot.next();
                    continue;
                }
                if let Some(item) = hand(gun_side).and_then(|h| h.holding) {
                    if !bot.pending {
                        actions.write(HandAction::Grab {
                            player,
                            side: off_side,
                            item,
                            point: Some(1),
                        });
                        bot.pending = true;
                    }
                }
                if bot.elapsed > 0.5 {
                    bot.next();
                }
            }
            Task::Gun(kind) => {
                if !bot.pending {
                    let Some(gun_e) = hand(gun_side).and_then(|h| h.holding) else {
                        bot.failed += 1;
                        bot.note("FAIL no gun in hand".into());
                        bot.next();
                        continue;
                    };
                    if let Ok(gun) = guns.get(gun_e) {
                        mac.start(gun_e, macro_steps(kind, gun));
                    }
                    bot.pending = true;
                } else if !mac.busy() && bot.elapsed > 0.1 {
                    bot.next();
                }
            }
            Task::Fire { at, shots } => {
                bot.look = at;
                bot.aim = true;
                let Some(gun) = hand(gun_side)
                    .and_then(|h| h.holding)
                    .and_then(|e| guns.get(e).ok())
                else {
                    bot.aim = false;
                    bot.next();
                    continue;
                };
                // Settle the aim, then pulse the trigger.
                if bot.elapsed < 0.6 {
                    continue;
                }
                let auto = gun.firearm.fire_mode() == extraction_core::firearm::FireMode::Auto;
                let period = if auto { 0.15 } else { 0.35 };
                let phase = ((bot.elapsed - 0.6) / period) as u32;
                bot.trigger = (bot.elapsed - 0.6) % period < period * 0.5;
                if phase >= shots || gun.firearm.live_rounds() == 0 && !bot.trigger {
                    bot.trigger = false;
                    bot.aim = false;
                    bot.next();
                }
            }
            Task::Combine => {
                if let (Some(a), Some(b)) = (
                    hand(gun_side).and_then(|h| h.holding),
                    hand(off_side).and_then(|h| h.holding),
                ) {
                    combine.write(Combine { a, b });
                }
                bot.next();
            }
            Task::Spawn { kind, at } => {
                let exit = at + Vec3::new(0.0, 0.0, -200.0);
                let job = crate::robots::Job {
                    kind: crate::robots::JobKind::Response,
                    site: root.translation,
                    duration: 120.0,
                    exit,
                };
                match kind {
                    crate::robots::RobotKind::Drone => {
                        crate::robots::drone::spawn_drone(&mut commands, at, job);
                    }
                    crate::robots::RobotKind::Walker => {
                        crate::robots::walker::spawn_walker(&mut commands, &zone, at, job);
                    }
                    crate::robots::RobotKind::Dirigible => {
                        crate::robots::dirigible::spawn_dirigible(&mut commands, at, job);
                    }
                    crate::robots::RobotKind::Pylon => {
                        crate::robots::pylon::spawn_pylon(&mut commands, at, 300.0, exit);
                    }
                    crate::robots::RobotKind::Sorter => {
                        // An open-air hall round where it was put.
                        let floor = zone.on_ground(at.x, at.z);
                        crate::robots::sorter::spawn_sorter(
                            &mut commands,
                            Transform::from_translation(floor),
                            Vec2::new(12.0, 8.0),
                            6.0,
                            crate::robots::sorter::depot_spots(),
                            6,
                        );
                    }
                }
                bot.note(format!("spawned a {kind:?}"));
                bot.next();
            }
            Task::Stage {
                kind,
                at,
                radius,
                artifact,
                kit,
            } => {
                let (artifact, kit) = (artifact.clone(), kit.clone());
                commands.queue(move |world: &mut World| {
                    crate::anomaly::harvest::stage(world, kind, at, radius, &artifact, &kit);
                });
                bot.note(format!("staged a {kind:?}"));
                bot.next();
            }
            Task::Engage { shots } => {
                let nearest = robots.iter().filter(|(_, r, _)| !r.wrecked).min_by(|a, b| {
                    a.2.translation()
                        .distance(root.translation)
                        .total_cmp(&b.2.translation().distance(root.translation))
                });
                let Some((_, _, rgt)) = nearest else {
                    bot.trigger = false;
                    bot.aim = false;
                    bot.next();
                    continue;
                };
                bot.look = rgt.translation();
                bot.aim = true;
                let gun = hand(gun_side)
                    .and_then(|h| h.holding)
                    .and_then(|e| guns.get(e).ok());
                let empty = gun.is_none_or(|g| g.firearm.live_rounds() == 0);
                if bot.elapsed > 0.5 && !empty && bot.fired < shots {
                    let period = 0.3;
                    let was = bot.trigger;
                    bot.trigger = (bot.elapsed % period) < period * 0.5;
                    if bot.trigger && !was {
                        bot.fired += 1;
                    }
                } else if empty || bot.fired >= shots {
                    bot.trigger = false;
                    bot.aim = false;
                    bot.next();
                }
            }
            Task::Grab { item, side } => {
                if hand(side).and_then(|h| h.holding) == Some(item) {
                    bot.walk = None;
                    bot.next();
                    continue;
                }
                let Ok((gt, held)) = ext.placed.get(item) else {
                    bot.failed += 1;
                    bot.note(format!("FAIL {item} is gone"));
                    bot.walk = None;
                    bot.next();
                    continue;
                };
                if held.is_some_and(|h| h.player != player) {
                    bot.failed += 1;
                    bot.note(format!("FAIL someone else has {item}"));
                    bot.walk = None;
                    bot.next();
                    continue;
                }
                let at = gt.translation();
                bot.look = at;
                let flat = Vec2::new(at.x, at.z);
                if pos.distance(flat) > REACH {
                    bot.walk = Some(flat);
                } else {
                    bot.walk = None;
                    bot.retry += dt;
                    if !bot.pending || bot.retry > 2.0 {
                        actions.write(HandAction::Grab {
                            player,
                            side,
                            item,
                            point: None,
                        });
                        bot.pending = true;
                        bot.retry = 0.0;
                    }
                }
            }
            Task::GrabFromHere { item, side } => {
                bot.walk = None;
                if hand(side).and_then(|h| h.holding) == Some(item) {
                    bot.next();
                    continue;
                }
                if let Ok((gt, _)) = ext.placed.get(item) {
                    bot.look = gt.translation();
                }
                bot.retry += dt;
                if !bot.pending || bot.retry > 1.0 {
                    actions.write(HandAction::Grab {
                        player,
                        side,
                        item,
                        point: None,
                    });
                    bot.pending = true;
                    bot.retry = 0.0;
                }
                if bot.elapsed > 5.0 {
                    bot.next();
                }
            }
            Task::Press(item) => {
                let Ok((gt, _)) = ext.placed.get(item) else {
                    bot.failed += 1;
                    bot.note(format!("FAIL nothing to press at {item}"));
                    bot.next();
                    continue;
                };
                let at = gt.translation();
                bot.look = at;
                let flat = Vec2::new(at.x, at.z);
                if pos.distance(flat) > REACH {
                    bot.walk = Some(flat);
                } else {
                    bot.walk = None;
                    actions.write(HandAction::Grab {
                        player,
                        side: off_side,
                        item,
                        point: None,
                    });
                    bot.next();
                }
            }
            Task::OpenDoor(door) => {
                let (Ok(d), Ok((gt, _))) = (ext.doors.get(door), ext.placed.get(door)) else {
                    bot.next();
                    continue;
                };
                bot.walk = None;
                bot.look = gt.translation() + Vec3::Y * 1.0;
                if d.open {
                    // Wait for it to swing most of the way.
                    if d.angle > 0.85 {
                        bot.next();
                    }
                } else if !bot.pending || bot.retry > 2.0 {
                    let hinge = Vec2::new(gt.translation().x, gt.translation().z);
                    if pos.distance(hinge) > REACH + 1.5 {
                        bot.failed += 1;
                        bot.note("FAIL door out of reach".into());
                        bot.next();
                        continue;
                    }
                    actions.write(HandAction::Grab {
                        player,
                        side: off_side,
                        item: door,
                        point: None,
                    });
                    bot.pending = true;
                    bot.retry = 0.0;
                } else {
                    bot.retry += dt;
                }
            }
            Task::Use(side) => {
                if let Some(item) = hand(side).and_then(|h| h.holding) {
                    ext.uses
                        .write(crate::items::usage::UseItem { player, item });
                } else {
                    bot.failed += 1;
                    bot.note(format!("FAIL nothing in the {side:?} hand to use"));
                }
                bot.next();
            }
            Task::Throw { side, at } => {
                bot.walk = None;
                // Lob it: aim a little above the point, by distance.
                let d = Vec2::new(at.x - pos.x, at.z - pos.y).length();
                bot.look = at + Vec3::Y * (0.15 * d);
                if bot.elapsed > 0.4 {
                    actions.write(HandAction::Release {
                        player,
                        side,
                        throw: (2.5 + d * 0.7).min(12.0),
                    });
                    bot.next();
                }
            }
            Task::Shoot { target, shots } => {
                let alive = robots
                    .get(target)
                    .ok()
                    .filter(|(_, r, _)| !r.wrecked)
                    .map(|(_, _, gt)| gt.translation());
                let Some(root_at) = alive else {
                    bot.trigger = false;
                    bot.aim = false;
                    bot.next();
                    continue;
                };
                // The core if it's still there, else the body.
                let core = ext
                    .parts
                    .iter()
                    .find(|(p, _)| {
                        p.robot == target && p.role == crate::robots::PartRole::Core && p.alive()
                    })
                    .map(|(_, gt)| gt.translation());
                bot.walk = None;
                // Lead a moving target: by the swing lag and the bullet's
                // flight.
                let at = core.unwrap_or(root_at);
                let now = bot.elapsed;
                let vel = match bot.track {
                    Some((p, t)) if now > t + 1e-3 && now - t < 0.5 => (at - p) / (now - t),
                    _ => Vec3::ZERO,
                };
                bot.track = Some((at, now));
                let flight = at.distance(root.translation) / 700.0;
                bot.look = at + vel * (1.0 / AIM_RATE + flight);
                bot.aim = true;
                let gun = hand(gun_side)
                    .and_then(|h| h.holding)
                    .and_then(|e| guns.get(e).ok());
                let empty = gun.is_none_or(|g| g.firearm.live_rounds() == 0);
                // Squeeze only with the body square to the target.
                let to = bot.look - root.translation;
                let bearing = (-to.x).atan2(-to.z);
                let (yaw, _, _) = root.rotation.to_euler(EulerRot::YXZ);
                let off = (bearing - yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                let on_target = off.abs() < 0.05 || bot.elapsed > 6.0;
                if bot.elapsed > 0.5 && !empty && bot.fired < shots && (on_target || bot.trigger) {
                    let period = 0.35;
                    let was = bot.trigger;
                    bot.trigger = (bot.elapsed % period) < period * 0.5;
                    if bot.trigger && !was {
                        bot.fired += 1;
                    }
                } else if empty || bot.fired >= shots {
                    bot.trigger = false;
                    bot.aim = false;
                    bot.next();
                }
            }
            Task::ToggleBackpack => {
                toggle.write(ToggleBackpack { player });
                bot.next();
            }
            Task::Expect(expect) => {
                let (ok, what) = match &expect {
                    Expect::Holding(side, id) => {
                        let got = holding_id(*side);
                        (
                            got.as_deref() == Some(id.as_str()),
                            format!("{side:?} hand holds {id} (holds {got:?})"),
                        )
                    }
                    Expect::GunRounds(lo, hi) => {
                        let n = hand(gun_side)
                            .and_then(|h| h.holding)
                            .and_then(|e| guns.get(e).ok())
                            .map(|g| g.firearm.live_rounds());
                        (
                            n.is_some_and(|n| n >= *lo && n <= *hi),
                            format!("gun live rounds in {lo}..={hi} (got {n:?})"),
                        )
                    }
                    Expect::Hits(n) => (
                        bot.hits >= *n,
                        format!("at least {n} hits (got {})", bot.hits),
                    ),
                    Expect::CarryingAtLeast(kg) => (
                        body.carried_kg >= *kg,
                        format!("carrying >= {kg} kg (got {:.2})", body.carried_kg),
                    ),
                    Expect::CarryingAtMost(kg) => (
                        body.carried_kg <= *kg,
                        format!("carrying <= {kg} kg (got {:.2})", body.carried_kg),
                    ),
                    Expect::PackItems(lo, hi) => {
                        let n = bags
                            .iter()
                            .find(|b| b.owner == player && b.carried)
                            .map_or(0, |b| b.contents.len());
                        (
                            n >= *lo && n <= *hi,
                            format!("pack items in {lo}..={hi} (got {n})"),
                        )
                    }
                    Expect::Near(p, d) => {
                        let dist = pos.distance(*p);
                        (dist <= *d, format!("within {d} m of {p} (got {dist:.1})"))
                    }
                    Expect::Wrecked(n) => {
                        let w = robots.iter().filter(|(_, r, _)| r.wrecked).count();
                        (w >= *n, format!("at least {n} robots wrecked (got {w})"))
                    }
                    Expect::RobotHits(n) => (
                        bot.robot_hits >= *n,
                        format!("at least {n} hits on robots (got {})", bot.robot_hits),
                    ),
                    Expect::Heard(said, n) => {
                        let got = ext.heard.as_ref().map_or(0, |h| h.count(*said));
                        (
                            got >= *n,
                            format!("heard at least {n} {said:?} words (got {got})"),
                        )
                    }
                    Expect::Alive => {
                        let h = healths
                            .get(player)
                            .map(|h| (h.down, h.hp))
                            .unwrap_or((false, 0.0));
                        (!h.0, format!("still standing (hp {:.0})", h.1))
                    }
                };
                if ok {
                    bot.passed += 1;
                    bot.note(format!("PASS {what}"));
                } else {
                    bot.failed += 1;
                    bot.note(format!("FAIL {what}"));
                }
                bot.next();
            }
        }
        let _ = &mags;
    }
}
