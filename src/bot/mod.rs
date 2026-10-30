//! Bots: players driven by a task list instead of a person.
//!
//! A bot writes its rig's [`RigInput`] (head direction, walking, trigger)
//! and sends the same [`HandAction`]s flat mode does, so everything a bot
//! does exercises the real game. Scenarios ([`scenarios`]) are task lists
//! with expectations; run one with `--scenario NAME` (add `--headless` to
//! run faster than real time with no window). Map simulations build on the
//! same tasks.

pub mod crating;
pub mod crossing;
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
    /// Stand exactly here at once: down a site, under the ground
    /// `Teleport` would put it on.
    TeleportDown(Vec3),
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
    /// Grab `item` as `Grab` does, unless it still lies within `radius` of
    /// `clear_of` (a stone not yet out of a burner's coals): then leave it.
    GrabClear {
        item: Entity,
        side: Side,
        clear_of: Vec3,
        radius: f32,
    },
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
    /// Take a gun off the body (sling, holster) into the gun hand.
    DrawGun,
    /// A press of one of the gun hand's buttons.
    Button(Btn),
    /// Reload with the gun hand alone (`Comfort::one_handed`): the
    /// magazine out, the well brought down onto one in a pouch, the second
    /// button to rack.
    OneHandReload,
    /// One-handed: bring what the gun hand holds to this (set down, or in
    /// a pouch), for them to combine (rounds into a magazine).
    OneHandCombine(Entity),
    /// Stage a scenario's world by name, where the bot stands: the module
    /// under test answers [`scenarios::Setup`] (a heavy and a well).
    Setup(&'static str),
    /// With bolt cutters in hand, walk to the nearest thing they cut
    /// within `within` m and cut it (a sentry's cable, a padlock): the
    /// bot's way, like `Grab`, without working the handles.
    Cut {
        within: f32,
    },
}

/// A controller button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    /// A or X: the magazine release.
    Primary = 0,
    /// B or Y: the slide stop, or (one-handed) a rack.
    Secondary,
    /// The stick's click: the fire selector.
    Stick,
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
            Task::GrabClear {
                item,
                side,
                clear_of,
                radius,
            } => Task::GrabClear {
                item,
                side: side.other(),
                clear_of,
                radius,
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

/// Waypoints on a circle of `r` round `centre` that take a walk from
/// `from` to `to` round it rather than across it (none when the straight
/// line keeps clear).
pub fn round_about(from: Vec2, to: Vec2, centre: Vec2, r: f32) -> Vec<Vec2> {
    let d = to - from;
    let t = ((centre - from).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
    if (from + d * t).distance(centre) >= r - 0.5 {
        return Vec::new();
    }
    let a0 = (from - centre).to_angle();
    let a1 = (to - centre).to_angle();
    let turn =
        (a1 - a0 + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    // A waypoint every radian or less of the way round.
    let steps = turn.abs().ceil().max(1.0) as usize;
    (1..=steps)
        .map(|k| centre + Vec2::from_angle(a0 + turn * k as f32 / steps as f32) * r)
        .filter(|p| p.distance(to) > 0.8)
        .collect()
}

/// How far a bot reaches for things, metres (horizontal, from the body).
pub const REACH: f32 = 1.2;
/// How high over its feet a standing bot's hand gets, arm up: what's higher
/// (a lift's stone riding its column) no hand takes.
pub const STAND_REACH: f32 = 2.2;
/// A seated bot's eye (the rig lifts it to a standing one).
pub const SEATED_EYE: f32 = 1.2;
/// The lowest a seated hand gets, leaning: over the feet, unlifted.
pub const SEATED_HAND_LOW: f32 = 0.35;
/// How far past a hand's middle a grab still takes something.
const GRAB_SLACK: f32 = 0.15;
/// A step to the side round something in the way: how near it needs to
/// get (and how a step is told from a walk).
const SIDESTEP: f32 = 1.01;
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
    /// The bot has at least this much health (it came through unhurt).
    HealthAtLeast(f32),
    /// A named [`Counters`] count is in this inclusive range (what a
    /// machine did: a spotter's calls, a mine going off).
    Counter(&'static str, u32, u32),
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
    /// Clearing a stoppage mid-string (`clear_stoppage`).
    clearing: bool,
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
    /// The gun hand's buttons held down this tick.
    buttons: [bool; 3],
    /// Where the gun hand's grip should be (world), for a move the ready
    /// pose doesn't make (a well onto a pouched magazine).
    gun_to: Option<Vec3>,
    /// Times the current task was put off for something done first (one
    /// hand: put the gun away, draw it back).
    deferred: u32,
    /// The gun's well from the gun hand's grip, world, for a reload.
    well_off: Option<Vec3>,
    /// The task it's on (as written), to know a new one from it.
    on: Option<String>,
    /// Walking: the nearest it's got, and how long since it got nearer.
    progress: (f32, f32),
    /// Steps to the side on the current walk (kept across them).
    sidesteps: u32,
    /// Seated: the crouch button held for something low.
    reach_low: bool,
    /// The gun it last had in hand: the one to draw back.
    last_gun: Option<Entity>,
}

impl Bot {
    pub fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks: tasks.into(),
            elapsed: 0.0,
            fired: 0,
            clearing: false,
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
            buttons: [false; 3],
            gun_to: None,
            deferred: 0,
            well_off: None,
            on: None,
            progress: (f32::INFINITY, 0.0),
            sidesteps: 0,
            reach_low: false,
            last_gun: None,
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
        self.clearing = false;
        self.tasks.pop_front();
        self.restart();
        self.deferred = 0;
    }

    /// Holds one of the gun hand's buttons down (or lets it go), for a
    /// scenario that presses it itself: the bot's driver writes the
    /// buttons each frame, over the rig input.
    pub fn hold(&mut self, button: Btn, down: bool) {
        self.buttons[button as usize] = down;
    }

    /// A fresh start on whatever task is now first: for a director that
    /// puts new tasks in front of (or in place of) the running one, whose
    /// clock and half-done state aren't theirs.
    pub fn restart(&mut self) {
        self.elapsed = 0.0;
        self.fired = 0;
        self.pending = false;
        self.retry = 0.0;
        self.trigger = false;
        self.buttons = [false; 3];
        self.gun_to = None;
        self.well_off = None;
        self.reach_low = false;
        self.progress = (f32::INFINITY, 0.0);
    }

    /// Seated (`Comfort::seated`), something lower than a seated hand
    /// reaches needs the crouch button first: the view and the hands come
    /// down with it. `rel_y`: its height over the feet.
    fn within_seated_reach(&mut self, rel_y: f32, lift: Option<f32>) -> bool {
        let Some(lift) = lift else {
            return true;
        };
        if rel_y >= lift + SEATED_HAND_LOW - GRAB_SLACK {
            return true;
        }
        self.reach_low = true;
        false
    }

    /// Do these first, then the current task again.
    fn first(&mut self, tasks: Vec<Task>) {
        self.deferred += 1;
        for t in tasks.into_iter().rev() {
            self.tasks.push_front(t);
        }
        self.restart();
    }
}

pub struct BotPlugin;

impl Plugin for BotPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crossing::CrossingPlugin)
            .add_systems(PreUpdate, drive_bots.in_set(RigDriverSystems))
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
            Option<&crate::rig::seated::Seat>,
        ),
        With<Player>,
    >,
    hands: Query<(&RigHand, &Hand)>,
    time: Res<Time>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
) {
    let gun_side = comfort.as_ref().map_or(Side::Right, |c| c.gun_side());
    let seated = comfort.as_ref().is_some_and(|c| c.seated);
    for (bot, mut input, mut root, hint, children, seat) in &mut bots {
        let supporting = children.iter().any(|c| {
            hands
                .get(c)
                .is_ok_and(|(rh, h)| rh.side == gun_side.other() && h.supporting.is_some())
        });
        // Crouched, the head comes down as flat mode's C does (and the
        // body with it).
        // Seated, the head is a seated head (the rig lifts it, and lowers
        // it with the crouch button); else crouched, it comes down as flat
        // mode's C does (and the body with it).
        if bot.reach_low {
            input.crouch = true;
        }
        let lift = if seated {
            seat.map_or(0.0, |s| s.lift)
        } else {
            0.0
        };
        let eye = Vec3::new(
            0.0,
            if seated {
                SEATED_EYE
            } else if input.crouch {
                1.05
            } else {
                1.68
            },
            0.0,
        );
        let world_eye = root.translation + eye + Vec3::Y * lift;
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
        {
            let hand = input.hand_mut(gun_side);
            hand.primary = bot.buttons[0];
            hand.secondary = bot.buttons[1];
            hand.stick_click = bot.buttons[2];
        }
        if let Some(to) = bot.gun_to {
            let local = root.compute_affine().inverse().transform_point3(to) - Vec3::Y * lift;
            input.hand_mut(gun_side).pose.translation = local;
        }
        if seated {
            // No lower than a seated hand reaches.
            for side in Side::BOTH {
                let p = &mut input.hand_mut(side).pose.translation;
                p.y = p.y.max(SEATED_HAND_LOW);
            }
        }
        // Raw poses: the seated lift goes on after (`rig::seated`).
        input.lifted = 0.0;
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

/// What a one-handed bot does about a task.
enum OneHand {
    Do(Task),
    /// Do these first (put the gun away, draw it back), then this again.
    First(Vec<Task>),
    /// Two-handed only (a hand on the handguard, two things together).
    Skip,
}

/// One hand does everything: the gun goes away for anything else the hand
/// takes, and comes back out to shoot; a reload and a rack are done with
/// the gun hand alone.
fn one_hand(
    task: Task,
    gun: Side,
    held: Option<Entity>,
    held_id: Option<String>,
    gun_held: bool,
) -> OneHand {
    let free_first = |t: Task| {
        if held.is_some() {
            OneHand::First(vec![Task::Stow(gun)])
        } else {
            OneHand::Do(t)
        }
    };
    let armed_first = |t: Task| {
        if gun_held {
            OneHand::Do(t)
        } else if held.is_some() {
            OneHand::First(vec![Task::Stow(gun), Task::DrawGun])
        } else {
            OneHand::First(vec![Task::DrawGun])
        }
    };
    match task {
        Task::Support | Task::Combine => OneHand::Skip,
        Task::Gun(MacroKind::Reload) => armed_first(Task::OneHandReload),
        Task::Gun(MacroKind::Rack) => armed_first(Task::Button(Btn::Secondary)),
        Task::Gun(MacroKind::Mode) => armed_first(Task::Button(Btn::Stick)),
        t @ (Task::Fire { .. }
        | Task::Engage { .. }
        | Task::Shoot { .. }
        | Task::Button(_)
        | Task::OneHandReload) => armed_first(t),
        // Already in hand: done.
        Task::PickUp { id, .. } if held_id.as_deref() == Some(id.as_str()) => {
            OneHand::Do(Task::PickUp { id, side: gun })
        }
        Task::PickUp { id, .. } => free_first(Task::PickUp { id, side: gun }),
        Task::Grab { item, .. } if held == Some(item) => OneHand::Do(Task::Wait(0.0)),
        Task::Grab { item, .. } => free_first(Task::Grab { item, side: gun }),
        Task::GrabFromHere { item, .. } if held == Some(item) => OneHand::Do(Task::Wait(0.0)),
        Task::GrabFromHere { item, .. } => free_first(Task::GrabFromHere { item, side: gun }),
        t @ (Task::Press(_) | Task::OpenDoor(_)) => free_first(t),
        // Letting go of what the other hand held: a gun is put away, not
        // dropped.
        Task::Release(side) if side != gun && gun_held => OneHand::Do(Task::Stow(gun)),
        Task::Release(_) => OneHand::Do(Task::Release(gun)),
        Task::Stow(_) => OneHand::Do(Task::Stow(gun)),
        Task::Use(_) => OneHand::Do(Task::Use(gun)),
        Task::Throw { at, .. } => OneHand::Do(Task::Throw { side: gun, at }),
        Task::Draw { slot, .. } => OneHand::Do(Task::Draw { slot, side: gun }),
        Task::Expect(Expect::Holding(_, id)) => OneHand::Do(Task::Expect(Expect::Holding(gun, id))),
        other => OneHand::Do(other),
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
    socketed: Query<'w, 's, (Entity, &'static InSocket, &'static GlobalTransform)>,
    seats: Query<'w, 's, &'static crate::rig::seated::Seat>,
    sockets: Query<
        'w,
        's,
        (
            &'static crate::interact::Socket,
            &'static crate::interact::SocketOwner,
            &'static GlobalTransform,
        ),
    >,
    heard: Option<Res<'w, crate::robot_voice::Heard>>,
    setups: MessageWriter<'w, scenarios::Setup>,
    cuttables: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::destruction::hooks::Cuttable,
            &'static GlobalTransform,
        ),
    >,
    cuts: MessageWriter<'w, crate::destruction::hooks::Cut>,
    counters: Option<Res<'w, Counters>>,
}

/// Counts any system can bump for a scenario's [`Expect::Counter`].
#[derive(Resource, Default, Debug)]
pub struct Counters(pub std::collections::HashMap<&'static str, u32>);

impl Counters {
    pub fn bump(&mut self, name: &'static str) {
        *self.0.entry(name).or_default() += 1;
    }
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
    // One-handed, the other hand's work is the gun hand's too.
    let one_handed = comfort.as_ref().is_some_and(|c| c.one_handed);
    let off_side = if one_handed {
        gun_side
    } else {
        gun_side.other()
    };
    for (player, mut bot, root, body, children, mut mac) in &mut bots {
        let Some(task) = bot.tasks.front().cloned() else {
            continue;
        };
        // A new task in front (a director put it there, ahead of or in
        // place of the one running) starts its own clock: a button press
        // timed from another's never comes.
        let key = format!("{task:?}");
        if bot.on.as_deref() != Some(key.as_str()) {
            if bot.on.is_some() {
                bot.restart();
            }
            bot.on = Some(key);
        }
        let task = task.mirrored(mirror);
        // Seated: the lift, for what's in reach.
        let lift = comfort
            .as_ref()
            .filter(|c| c.seated)
            .map(|_| ext.seats.get(player).map_or(0.0, |s| s.lift));
        let task = if one_handed {
            let held = children.iter().find_map(|c| {
                hands
                    .get(c)
                    .ok()
                    .filter(|(rh, _)| rh.side == gun_side)
                    .and_then(|(_, h)| h.holding)
            });
            let gun_held = held.is_some_and(|e| guns.get(e).is_ok());
            if gun_held {
                bot.last_gun = held;
            }

            let held_id = held
                .and_then(|e| items.get(e).ok())
                .map(|i| catalog.get(i.kind()).id.clone());
            match one_hand(task, gun_side, held, held_id, gun_held) {
                OneHand::Do(t) => t,
                OneHand::First(tasks) if bot.deferred < 4 => {
                    bot.first(tasks);
                    continue;
                }
                OneHand::First(_) => {
                    bot.failed += 1;
                    let t = bot.tasks.front().cloned();
                    bot.note(format!("FAIL one hand: can't get to {t:?}"));
                    bot.next();
                    continue;
                }
                OneHand::Skip => {
                    bot.next();
                    continue;
                }
            }
        } else {
            task
        };
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
            Task::GrabClear {
                item,
                side,
                clear_of,
                radius,
            } => {
                let inside = ext.placed.get(item).is_ok_and(|(gt, _)| {
                    let p = gt.translation();
                    Vec2::new(p.x - clear_of.x, p.z - clear_of.z).length() < radius
                });
                if inside {
                    bot.note(format!("left {item}: still in the anomaly"));
                    bot.next();
                    continue;
                }
                if let Some(front) = bot.tasks.front_mut() {
                    *front = Task::Grab { item, side };
                }
                // Round the anomaly to it, never across.
                if let Ok((gt, _)) = ext.placed.get(item) {
                    let to = gt.translation().xz();
                    for p in round_about(pos, to, clear_of.xz(), radius + 1.0)
                        .into_iter()
                        .rev()
                    {
                        bot.tasks.push_front(Task::WalkTo { to: p, within: 0.6 });
                    }
                }
                continue;
            }
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
            Task::TeleportDown(to) => {
                let mut t = *root;
                t.translation = to;
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
            Task::Cut { within } => {
                if holding_id(gun_side).as_deref() != Some("bolt_cutters") {
                    bot.failed += 1;
                    bot.note("FAIL no bolt cutters in hand".to_string());
                    bot.next();
                    continue;
                }
                let nearest = ext
                    .cuttables
                    .iter()
                    .map(|(e, _, g)| (e, g.translation()))
                    .filter(|(_, p)| p.distance(root.translation) < within)
                    .min_by(|a, b| {
                        a.1.distance(root.translation)
                            .total_cmp(&b.1.distance(root.translation))
                    });
                let Some((target, at)) = nearest else {
                    bot.failed += 1;
                    bot.note(format!("FAIL nothing to cut within {within} m"));
                    bot.next();
                    continue;
                };
                bot.look = at;
                let flat = Vec2::new(at.x, at.z);
                if pos.distance(flat) > 0.9 {
                    bot.walk = Some(flat);
                } else {
                    bot.walk = None;
                    ext.cuts
                        .write(crate::destruction::hooks::Cut { target, by: player });
                    bot.note("cut it".to_string());
                    bot.next();
                }
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
                let d = pos.distance(to);
                // A step aside that can't be made isn't worth a fail.
                if within == SIDESTEP && bot.elapsed > 4.0 {
                    bot.walk = None;
                    bot.next();
                    continue;
                }
                if d <= within {
                    bot.walk = None;
                    if within != SIDESTEP {
                        bot.sidesteps = 0;
                    }
                    bot.next();
                    continue;
                }
                // Up against something: a step to the side, as a person
                // would (one way, then the other), and on.
                if d < bot.progress.0 - 0.3 {
                    bot.progress = (d, 0.0);
                } else {
                    bot.progress.1 += dt;
                }
                // (Not when lining up close: indoors, a step aside put it
                // in a corner.)
                if bot.progress.1 > 2.5 && bot.sidesteps < 3 && within != SIDESTEP && d > 2.0 {
                    let ahead = (to - pos).normalize_or(Vec2::X);
                    let side = if bot.sidesteps % 2 == 0 { 1.0 } else { -1.0 };
                    let step = pos + ahead.perp() * side * 3.0 - ahead * 0.5;
                    bot.sidesteps += 1;
                    bot.tasks.push_front(Task::WalkTo {
                        to: step,
                        within: SIDESTEP,
                    });
                    bot.restart();
                    continue;
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
                    if (!bot.pending || bot.retry > 2.0)
                        && bot.within_seated_reach(at.y - root.translation.y, lift)
                    {
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
            Task::DrawGun => {
                if hand(gun_side)
                    .and_then(|h| h.holding)
                    .is_some_and(|e| guns.get(e).is_ok())
                {
                    bot.next();
                    continue;
                }
                if !bot.pending || bot.retry > 1.0 {
                    // A gun in one of this player's body slots (not on a
                    // bench or rack of its own): the one it last had out,
                    // else any.
                    let on_body: Vec<Entity> = ext
                        .socketed
                        .iter()
                        .filter(|(e, s, _)| {
                            guns.get(*e).is_ok()
                                && ext.sockets.get(s.socket).is_ok_and(|(so, o, _)| {
                                    o.0 == player
                                        && matches!(so.kind, crate::interact::SocketKind::Body(_))
                                })
                        })
                        .map(|(e, ..)| e)
                        .collect();
                    let slung = on_body
                        .iter()
                        .find(|e| Some(**e) == bot.last_gun)
                        .or(on_body.first())
                        .map(|e| (*e,));
                    let Some((gun_e, ..)) = slung else {
                        bot.failed += 1;
                        bot.note("FAIL no gun on the body to draw".into());
                        // Nor what wanted the gun.
                        bot.next();
                        bot.next();
                        continue;
                    };
                    actions.write(HandAction::Grab {
                        player,
                        side: gun_side,
                        item: gun_e,
                        point: Some(0),
                    });
                    bot.pending = true;
                    bot.retry = 0.0;
                } else {
                    bot.retry += dt;
                }
            }
            Task::Button(b) => {
                let i = match b {
                    Btn::Primary => 0,
                    Btn::Secondary => 1,
                    Btn::Stick => 2,
                };
                bot.buttons = [false; 3];
                bot.buttons[i] = bot.elapsed < 0.15;
                if bot.elapsed > 0.2 && !mac.busy() {
                    bot.next();
                }
            }
            Task::OneHandCombine(with) => {
                bot.aim = false;
                let Ok((at, _)) = ext.placed.get(with) else {
                    bot.failed += 1;
                    bot.note("FAIL nothing there to combine with".into());
                    bot.next();
                    continue;
                };
                let Some((hand_at, item_at)) = hand(gun_side).and_then(|h| {
                    h.holding
                        .and_then(|e| ext.placed.get(e).ok())
                        .map(|(g, _)| (h.world.translation, g.translation()))
                }) else {
                    bot.failed += 1;
                    bot.note("FAIL nothing in hand to combine".into());
                    bot.next();
                    continue;
                };
                // What's held, not the hand, down onto it from above:
                // steered by where it is now (held where it was taken, it
                // turns with the hand), looking at it to keep still.
                bot.look = at.translation();
                let above = (0.15 - bot.elapsed * 0.1).max(0.03);
                let goal = at.translation() + Vec3::Y * above;
                // A quarter of the way each tick: it lags the hand, and a
                // whole step overshoots.
                let from = bot.gun_to.unwrap_or(hand_at);
                bot.gun_to = Some(from + (goal - item_at) * 0.25);
                if bot.elapsed > 2.5 {
                    bot.next();
                }
            }
            Task::OneHandReload => {
                bot.aim = false;
                let Some((gun_e, gun, hand_at)) = hand(gun_side).and_then(|h| {
                    h.holding
                        .and_then(|e| guns.get(e).ok().map(|g| (e, g, h.world.translation)))
                }) else {
                    bot.failed += 1;
                    bot.note("FAIL no gun in hand to reload".into());
                    bot.next();
                    continue;
                };
                let Some((well_s, _, well_at)) = ext.sockets.iter().find(|(s, o, _)| {
                    o.0 == gun_e && matches!(s.kind, crate::interact::SocketKind::MagWell(_))
                }) else {
                    bot.failed += 1;
                    bot.note("FAIL no magazine well".into());
                    bot.next();
                    continue;
                };
                let crate::interact::SocketKind::MagWell(family) = &well_s.kind else {
                    continue;
                };
                let in_well = well_s.occupant;
                let well_at = well_at.translation();
                if bot.fired == 0 {
                    // The empty one out.
                    if in_well.is_some() {
                        // Pressed again each half second: a press the gun
                        // missed (it was put away mid-press) isn't the end.
                        bot.buttons[0] = bot.elapsed % 0.5 < 0.15;
                        if bot.elapsed > 3.0 {
                            bot.failed += 1;
                            bot.note("FAIL the button didn't drop the magazine".into());
                            bot.next();
                        }
                        continue;
                    }
                    bot.buttons = [false; 3];
                    bot.fired = 1;
                    bot.elapsed = 0.0;
                }
                if in_well.is_none() {
                    // The fullest magazine of its kind in a pouch or pocket.
                    let pouched = ext
                        .socketed
                        .iter()
                        .filter(|(m, s, _)| {
                            ext.sockets.get(s.socket).is_ok_and(|(so, o, _)| {
                                o.0 == player
                                    && matches!(so.kind, crate::interact::SocketKind::Body(_))
                            }) && mags.get(*m).is_ok_and(|mg| &mg.family == family)
                                && items.get(*m).is_ok_and(|it| it.rounds > 0)
                        })
                        .max_by_key(|(m, ..)| items.get(*m).map_or(0, |it| it.rounds));
                    let Some((m, _, mgt)) = pouched else {
                        // Out of magazines: as a reload macro with none, it
                        // stays empty.
                        bot.note("no loaded magazine in a pouch".into());
                        bot.next();
                        continue;
                    };
                    let h = mags.get(m).map_or(0.1, |mg| mg.height);
                    let top = mgt.transform_point(Vec3::Y * h * 0.5);
                    // Down onto it from above.
                    let above = (0.12 - bot.elapsed * 0.25).max(-0.01);
                    // The well's offset from the grip, taken once: the gun
                    // sits fixed in the hand, and the hand's own position
                    // lags its target a tick.
                    let off = *bot.well_off.get_or_insert(well_at - hand_at);
                    bot.gun_to = Some(top + Vec3::Y * above - off);
                    if bot.elapsed > 4.0 {
                        bot.failed += 1;
                        bot.note("FAIL the well didn't take the pouched magazine".into());
                        bot.next();
                    }
                    continue;
                }
                bot.gun_to = None;
                // In: rack a round in, if none is.
                if bot.fired == 1 {
                    bot.fired = 2;
                    bot.elapsed = 0.0;
                }
                if gun.firearm.chambers.first().is_some_and(|c| c.is_none()) || bot.elapsed < 0.2 {
                    bot.buttons[1] = bot.elapsed < 0.15;
                    if bot.elapsed > 1.5 {
                        bot.next();
                    }
                    continue;
                }
                if !mac.busy() {
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
                let Some((gun_e, gun)) = hand(gun_side)
                    .and_then(|h| h.holding)
                    .and_then(|e| guns.get(e).ok().map(|g| (e, g)))
                else {
                    bot.aim = false;
                    bot.next();
                    continue;
                };
                // Settle the aim, then pulse the trigger.
                if bot.elapsed < 0.6 {
                    continue;
                }
                if clear_stoppage(&mut bot, &mut mac, gun_e, gun) {
                    continue;
                }
                let auto = gun.firearm.fire_mode() == extraction_core::firearm::FireMode::Auto;
                let period = if auto { 0.15 } else { 0.35 };
                let was = bot.trigger;
                bot.trigger = bot.fired < shots && (bot.elapsed - 0.6) % period < period * 0.5;
                if bot.trigger && !was {
                    bot.fired += 1;
                }
                if bot.fired >= shots && !bot.trigger
                    || gun.firearm.live_rounds() == 0 && !bot.trigger
                {
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
                    combine.write(Combine::all(a, b));
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
                let held = hand(gun_side).and_then(|h| h.holding);
                let gun = held.and_then(|e| guns.get(e).ok());
                if let (Some(e), Some(g)) = (held, gun)
                    && clear_stoppage(&mut bot, &mut mac, e, g)
                {
                    continue;
                }
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
                } else if at.y - root.translation.y > STAND_REACH {
                    // Over its head, out of any hand's reach: it waits a
                    // little (it may come down), then gives up.
                    bot.walk = None;
                    bot.retry += dt;
                    if bot.retry > 3.0 {
                        bot.failed += 1;
                        bot.note(format!("FAIL {item} is out of reach overhead"));
                        bot.retry = 0.0;
                        bot.next();
                    }
                } else {
                    bot.walk = None;
                    bot.retry += dt;
                    if (!bot.pending || bot.retry > 2.0)
                        && bot.within_seated_reach(at.y - root.translation.y, lift)
                    {
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
                let mut rel_y = 1.0;
                if let Ok((gt, _)) = ext.placed.get(item) {
                    bot.look = gt.translation();
                    rel_y = gt.translation().y - root.translation.y;
                }
                bot.retry += dt;
                if (!bot.pending || bot.retry > 1.0) && bot.within_seated_reach(rel_y, lift) {
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
                    // Wait for it to swing most of the way, then let go of
                    // it (held, it comes along).
                    if d.angle > 0.85 {
                        if hand(off_side).and_then(|h| h.holding) == Some(door) {
                            actions.write(HandAction::Release {
                                player,
                                side: off_side,
                                throw: 0.0,
                            });
                        }
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
                let held = hand(gun_side).and_then(|h| h.holding);
                let gun = held.and_then(|e| guns.get(e).ok());
                if let (Some(e), Some(g)) = (held, gun)
                    && clear_stoppage(&mut bot, &mut mac, e, g)
                {
                    continue;
                }
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
                    Expect::Counter(name, lo, hi) => {
                        let n = ext
                            .counters
                            .as_ref()
                            .and_then(|c| c.0.get(name).copied())
                            .unwrap_or(0);
                        (
                            n >= *lo && n <= *hi,
                            format!("{name} in {lo}..={hi} (got {n})"),
                        )
                    }
                    Expect::HealthAtLeast(n) => {
                        let hp = healths.get(player).map_or(0.0, |h| h.hp);
                        (hp >= *n, format!("health at least {n} (hp {hp:.0})"))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A walk to the far side of an anomaly goes round it; one that keeps
    /// clear goes straight.
    #[test]
    fn a_walk_goes_round_an_anomaly_not_across() {
        let c = Vec2::ZERO;
        let way = round_about(Vec2::new(5.0, 0.0), Vec2::new(-5.0, 0.3), c, 3.0);
        assert!(!way.is_empty());
        let mut at = Vec2::new(5.0, 0.0);
        for p in way.iter().chain([&Vec2::new(-5.0, 0.3)]) {
            // Each leg keeps out of the 2 m that burns.
            for k in 0..=10 {
                let q = at.lerp(*p, k as f32 / 10.0);
                assert!(q.distance(c) > 2.0, "{q} crosses it");
            }
            at = *p;
        }
        assert!(round_about(Vec2::new(5.0, 0.0), Vec2::new(5.0, 6.0), c, 3.0).is_empty());
    }
}

/// A stoppage in the gun in hand, as a shooter meets it: the trigger off,
/// the gun's own clearance worked (tap-rack; for a double feed, drop the
/// magazine, rack, reload: `flat::macro_steps`), then back to the string.
/// True while it's being cleared; the presses then aren't counted, so a
/// jam costs time, not the drill (a worn gun's dice, seeded by where it
/// lay, would otherwise decide whether a scenario passes).
fn clear_stoppage(bot: &mut Bot, mac: &mut GunMacro, gun_e: Entity, gun: &Gun) -> bool {
    if bot.clearing {
        if mac.busy() {
            bot.trigger = false;
            return true;
        }
        bot.clearing = false;
        bot.note("cleared a stoppage".into());
    }
    let Some(m) = gun.firearm.malfunction else {
        return false;
    };
    bot.trigger = false;
    bot.clearing = true;
    bot.note(format!("stoppage: {m:?}, {}", m.remedy()));
    mac.start(gun_e, macro_steps(MacroKind::Rack, gun));
    true
}
