//! `--scenario contract_<id>` (run by name, not in `all`: each is several
//! raids): a bot works one of the driver's contracts
//! (`extraction_core::contracts`) from the card to its pay. Each step is
//! done with the bot's own hands and presses: the kit off the hatch's
//! ledge, out of the truck, the step done where it lives (a thing picked
//! up, a relay set down, a photograph taken, a place seen), home, what's
//! wanted into the stash crate, and the card pressed. At the end the
//! contract is done, its pay has come through the hatch and what it opens
//! is open.
//!
//! Every contract's steps lie in one valley (a photograph can be taken in
//! any), so each runs whole on its own map:
//!
//! - `contract_post_counter`, `contract_mast_relay`,
//!   `contract_the_overflow`: `--map marrow_creek`
//! - `contract_hajek_tin`: `--map kettle_pass`
//! - `contract_walker_portrait`, `contract_the_heavy`: `--map cinder_works`
//! - `contract_scour_key`: `--map sallow_reservoir`
//!
//! Shortcuts:
//! - The career is set at the start so the contract is the one the driver
//!   offers: its place in its tier (or past the ladder), the contracts
//!   before it done and their trust earned, and for the overflow a raid in
//!   each of the first three valleys. No fittings (their props would stand in the
//!   bed).
//! - Every raid ends with the raid's phase set to extracting (no radio
//!   call, no walk to a rendezvous); the first one too, so the bot starts
//!   in the truck.
//! - The walk across the valley is skipped (`Task::Teleport` to some way
//!   short of the place, or of the door of the house a thing lies in);
//!   the last stretch is walked, the door opened, the thing picked up.
//! - A fetch's things are laid loose a little way out from the truck (a
//!   scenario's stash starts empty), and the bot carries them home.
//! - A photograph's subject is put there, some way in front of the bot,
//!   and taken away again once it's in the picture.

use bevy::prelude::*;
use extraction_core::contracts::{CONTRACTS, Contract, Offered, Subject, Unlock, contract};
use extraction_core::progression::{
    FIRST_VALLEYS, Job, Progress, SCOUT_RADIUS, TIERS, fetch_takes,
};

use super::super::Progression;
use crate::bot::{Bot, Task};
use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{Held, InSocket, Socket, SocketOwner};
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::launch::LaunchOptions;
use crate::raid::truck::{FLOOR, Stash, Truck};
use crate::raid::{Phase, Raid};
use crate::rig::{LocalPlayer, RigHead, RigInput, Side};
use crate::robots::{Breed, Robot, RobotKind};
use crate::world::Zone;

/// (scenario, contract): one run per contract.
pub const SCENARIOS: &[(&str, &str)] = &[
    ("contract_post_counter", "post_counter"),
    ("contract_mast_relay", "mast_relay"),
    ("contract_hajek_tin", "hajek_tin"),
    ("contract_walker_portrait", "walker_portrait"),
    ("contract_the_overflow", "the_overflow"),
    ("contract_scour_key", "scour_key"),
    ("contract_the_heavy", "the_heavy"),
    ("contract_annas_tape", "annas_tape"),
];

/// The last task while the scenario runs: the director adds each step's.
const IDLE: f32 = 4.5;
/// Teleported this far short of a step's place; the rest is walked.
const SHORT: f32 = 25.0;
/// A photograph's subject is put this far in front.
const SUBJECT_AT: f32 = 25.0;
/// Give up on a stage after this many goes.
const TRIES: u32 = 8;

pub fn is(name: &str) -> bool {
    SCENARIOS.iter().any(|(s, _)| *s == name)
}

pub fn opening(name: &str) -> Vec<Task> {
    vec![
        Task::Log(format!("{name}: a contract from the card to its pay")),
        Task::Wait(IDLE),
    ]
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    #[default]
    Setup,
    Offered,
    Out,
    Go,
    Check,
    Home,
    Take,
    Aim,
    Stowed,
    Press,
    Handed,
    Paid,
    Done,
}

#[derive(Default)]
pub struct Run {
    stage: Stage,
    tries: u32,
    /// The contract's step being worked.
    step: u32,
    /// Things to get into the stash crate, one at a time.
    carry: Vec<Entity>,
    /// Keep the head on the photograph's subject.
    subject: Option<Entity>,
    /// The fetch's things are out.
    laid: bool,
}

/// What the director reads of the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Scene<'w, 's> {
    opts: Res<'w, LaunchOptions>,
    zone: Res<'w, Zone>,
    catalog: Res<'w, Catalog>,
    handovers: Res<'w, crate::raid::lev::Handovers>,
    trucks: Query<'w, 's, &'static GlobalTransform, With<Truck>>,
    stash: Query<'w, 's, (&'static GlobalTransform, &'static Backpack), With<Stash>>,
    cards: Query<'w, 's, Entity, With<crate::raid::requests::RequestCard>>,
    items: Query<
        'w,
        's,
        (
            Entity,
            &'static Item,
            &'static GlobalTransform,
            Option<&'static Held>,
            Option<&'static InSocket>,
            Option<&'static InBackpack>,
        ),
    >,
    sockets: Query<'w, 's, (Entity, &'static SocketOwner), With<Socket>>,
    bags: Query<'w, 's, &'static Backpack>,
    heads: Query<'w, 's, (&'static GlobalTransform, &'static ChildOf), With<RigHead>>,
    robots: Query<
        'w,
        's,
        (
            Entity,
            &'static Robot,
            &'static GlobalTransform,
            Option<&'static Breed>,
        ),
    >,
    benches: Query<'w, 's, &'static crate::raid::bench::Bench>,
    entrances: Res<'w, crate::robots::doors::Entrances>,
    spatial: avian3d::prelude::SpatialQuery<'w, 's>,
}

impl Scene<'_, '_> {
    fn id(&self, e: Entity) -> Option<&str> {
        self.items
            .get(e)
            .ok()
            .map(|(_, it, ..)| self.catalog.get(it.kind()).id.as_str())
    }

    /// The bot has it: in a hand, a body slot or its pack.
    fn carried(&self, e: Entity, player: Entity) -> bool {
        let Ok((_, _, _, held, sock, bag)) = self.items.get(e) else {
            return false;
        };
        held.is_some_and(|h| h.player == player)
            || sock.is_some_and(|s| self.sockets.get(s.socket).is_ok_and(|(_, o)| o.0 == player))
            || bag.is_some_and(|b| self.bags.get(b.bag).is_ok_and(|b| b.owner == player))
    }

    /// What the bot carries that `want` takes (a Bring's or Fetch's id).
    fn carrying(&self, want: &str, player: Entity) -> Vec<Entity> {
        self.items
            .iter()
            .filter(|(e, it, ..)| {
                fetch_takes(want, &self.catalog.get(it.kind()).id) && self.carried(*e, player)
            })
            .map(|(e, ..)| e)
            .collect()
    }

    /// The nearest loose `id` to `near`.
    fn loose(&self, id: &str, near: Vec2) -> Option<(Entity, Vec3)> {
        self.items
            .iter()
            .filter(|(_, it, _, held, sock, bag)| {
                held.is_none()
                    && sock.is_none()
                    && bag.is_none()
                    && self.catalog.get(it.kind()).id == id
            })
            .map(|(e, _, gt, ..)| (e, gt.translation()))
            .min_by(|a, b| a.1.xz().distance(near).total_cmp(&b.1.xz().distance(near)))
    }

    /// Nothing of the world in the way, `d` along from `from` to `to`, at
    /// knee and chest height.
    fn clear(&self, from: Vec2, to: Vec2, d: f32) -> bool {
        let Ok(dir) = Dir3::new(Vec3::new(to.x - from.x, 0.0, to.y - from.y)) else {
            return true;
        };
        let filter = avian3d::prelude::SpatialQueryFilter::from_mask(crate::world::Layer::World);
        [0.5, 1.3].into_iter().all(|h| {
            let at = self.zone.on_ground(from.x, from.y) + Vec3::Y * h;
            self.spatial.cast_ray(at, dir, d, true, &filter).is_none()
        })
    }

    fn in_stash(&self, e: Entity) -> bool {
        self.stash.iter().any(|(_, b)| b.contents.contains(&e))
    }

    fn holding(&self, player: Entity, side: Side) -> Option<Entity> {
        self.items
            .iter()
            .find(|(.., held, _, _)| held.is_some_and(|h| h.player == player && h.side == side))
            .map(|(e, ..)| e)
    }

    /// Into the `side` hand, from wherever the bot has it: a body slot
    /// (drawn), the pack (pulled round, taken out, put back) or the
    /// ground.
    fn take(&self, e: Entity, player: Entity, side: Side) -> Vec<Task> {
        let mut t = Vec::new();
        if self.holding(player, side).is_some_and(|h| h != e) {
            t.push(Task::Stow(side));
        }
        let Ok((_, _, _, held, sock, bag)) = self.items.get(e) else {
            return t;
        };
        if held.is_some_and(|h| h.player == player && h.side == side) {
            return t;
        }
        if let Some(s) = sock {
            // `Task::Draw`'s slot: the bot's sockets in entity order.
            let mut mine: Vec<Entity> = self
                .sockets
                .iter()
                .filter(|(_, o)| o.0 == player)
                .map(|(s, _)| s)
                .collect();
            mine.sort();
            if let Some(slot) = mine.iter().position(|m| *m == s.socket) {
                t.push(Task::Draw { slot, side });
                t.push(Task::Wait(0.5));
                return t;
            }
        }
        if bag.is_some() {
            t.extend([
                Task::ToggleBackpack,
                Task::Wait(0.6),
                Task::GrabFromHere { item: e, side },
                Task::ToggleBackpack,
                Task::Wait(0.5),
            ]);
            return t;
        }
        t.push(Task::Grab { item: e, side });
        t
    }

    fn subject(&self, s: Subject) -> Option<(Entity, Vec3)> {
        self.robots
            .iter()
            .filter(|(_, r, _, breed)| {
                !r.wrecked
                    && match s {
                        Subject::Heavy => breed.is_some_and(|b| b.0 == "heavy"),
                        Subject::Walker => {
                            r.kind == RobotKind::Walker && !breed.is_some_and(|b| b.0 == "heavy")
                        }
                    }
            })
            .map(|(e, _, gt, _)| (e, gt.translation()))
            .next()
    }
}

/// Where to stand in the bed, and where to look, so that a thing held at
/// `v` (in the head's frame) is in the stash crate's opening (within
/// `inner` of its middle, crate space). From `from` only, if `fixed`.
#[allow(clippy::too_many_arguments)]
fn aim(
    v: Vec3,
    eye_h: f32,
    floor: f32,
    truck: &GlobalTransform,
    stash: &GlobalTransform,
    inner: Vec3,
    from: Vec2,
    fixed: bool,
) -> Option<(Vec2, Vec3)> {
    let roots: Vec<Vec2> = if fixed {
        vec![from]
    } else {
        // The bed's left half and middle, clear of the bench's top.
        let mut r = Vec::new();
        for i in 0..=18 {
            for k in 0..=60 {
                let (x, z) = (-0.85 + i as f32 * 0.05, -1.5 + k as f32 * 0.05);
                r.push(truck.transform_point(Vec3::new(x, FLOOR, z)).xz());
            }
        }
        r
    };
    let to_local = stash.affine().inverse();
    let reach = v.length();
    let a = v.y.hypot(v.z);
    let mut best: Option<(f32, Vec2, Vec3)> = None;
    for r in roots {
        let eye = Vec3::new(r.x, floor + eye_h, r.y);
        for i in 0..5 {
            for j in 0..5 {
                for k in 0..5 {
                    let s = |n: i32| (n as f32 - 2.0) * 0.5;
                    let p = stash.transform_point(Vec3::new(s(i), s(j), s(k)) * inner);
                    let w = (p - eye).normalize_or_zero() * reach;
                    let t = eye + w;
                    let local = to_local.transform_point3(t);
                    if local.abs().cmpgt(inner).any() || a < 1e-3 || w.y.abs() > a {
                        continue;
                    }
                    // The pitch that brings it to the height wanted, then
                    // the turn that brings it round.
                    let phi = v.z.atan2(v.y);
                    let base = (w.y / a).clamp(-1.0, 1.0).acos();
                    let Some(pitch) = [base - phi, -base - phi]
                        .into_iter()
                        .map(|p| {
                            (p + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                                - std::f32::consts::PI
                        })
                        .filter(|p| p.abs() < 1.3)
                        .filter(|p| ((Quat::from_rotation_x(*p) * v).y - w.y).abs() < 0.01)
                        .min_by(|a, b| a.abs().total_cmp(&b.abs()))
                    else {
                        continue;
                    };
                    let u = Quat::from_rotation_x(pitch) * v;
                    let yaw = (-w.z).atan2(w.x) - (-u.z).atan2(u.x);
                    let turn = Quat::from_rotation_y(yaw);
                    if (turn * u - w).length() > 0.02 {
                        continue;
                    }
                    let score = local.length() + 0.5 * r.distance(from);
                    if best.is_none_or(|(b, ..)| score < b) {
                        let dir = turn * Quat::from_rotation_x(pitch) * Vec3::NEG_Z;
                        best = Some((score, r, eye + dir * 20.0));
                    }
                }
            }
        }
    }
    best.map(|(_, r, look)| (r, look))
}

/// The career as it would be when the driver offers `c`.
fn career_for(c: &Contract) -> Progress {
    let mut p = Progress::default();
    for before in CONTRACTS.iter().take_while(|b| b.id != c.id) {
        p.contracts_done.push(before.id.to_owned());
        p.trust += before.trust;
    }
    let (tier, at) = match c.offered {
        Offered::Tier { tier, at } => (tier, at),
        Offered::Deep { .. } => (TIERS.len() as u32, 0),
    };
    p.tier = tier;
    p.done_in_tier = at;
    p.done = TIERS
        .iter()
        .take(tier as usize)
        .map(|t| t.jobs)
        .sum::<u32>()
        + at;
    if c.raided_all {
        for v in FIRST_VALLEYS {
            p.count_raid(v);
        }
    }
    p
}

fn pass(bot: &mut Bot, msg: String) {
    info!("bot: PASS {msg}");
    bot.passed += 1;
    bot.log.push(format!("PASS {msg}"));
}

fn fail(bot: &mut Bot, msg: String) {
    info!("bot: FAIL {msg}");
    bot.failed += 1;
    bot.log.push(format!("FAIL {msg}"));
}

#[allow(clippy::too_many_arguments)]
pub fn direct(
    mut commands: Commands,
    scene: Scene,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
    mut bots: Query<(Entity, &mut Bot, &Transform, &mut RigInput), With<LocalPlayer>>,
    mut run: Local<Run>,
) {
    let Some(name) = scene.opts.scenario.as_deref() else {
        return;
    };
    let Some(c) = SCENARIOS
        .iter()
        .find(|(s, _)| *s == name)
        .and_then(|(_, id)| contract(id))
    else {
        return;
    };
    let Some((player, mut bot, root, mut input)) = bots.iter_mut().next() else {
        return;
    };
    // The head on the subject, as it moves.
    if let Some(at) = run
        .subject
        .and_then(|e| scene.robots.get(e).ok())
        .map(|(_, _, gt, _)| gt.translation())
    {
        bot.look = at;
    }
    // An empty list is the end of the run: keep the idle task going.
    let idle =
        bot.tasks.len() == 1 && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE);
    if idle && bot.elapsed > IDLE - 1.0 && run.stage != Stage::Done {
        bot.elapsed = 0.7;
    }
    if !idle || bot.elapsed < 0.6 {
        return;
    }
    let (Some(truck), Some((stash_gt, stash))) =
        (scene.trucks.iter().next(), scene.stash.iter().next())
    else {
        return;
    };
    let push = |bot: &mut Bot, tasks: Vec<Task>| {
        let wait = bot.tasks.pop_back();
        bot.tasks.extend(tasks);
        bot.tasks.extend(wait);
    };
    // Truck space to the ground under the bed.
    let bed = |x: f32, z: f32| truck.transform_point(Vec3::new(x, 0.0, z)).xz();
    let here = root.translation.xz();
    let legs = |from: Vec2, to: Vec2, within: f32| -> Vec<Task> {
        let n = (from.distance(to) / 14.0).ceil().max(1.0) as usize;
        (1..=n)
            .map(|k| Task::WalkTo {
                to: from.lerp(to, k as f32 / n as f32),
                within: if k == n { within } else { 1.5 },
            })
            .collect()
    };
    let job = progression.0.job.clone();
    run.tries += 1;
    if run.tries > TRIES * 4 && run.stage != Stage::Done {
        fail(&mut bot, format!("stuck at {:?}", run.stage));
        run.stage = Stage::Done;
    }
    let advance = |run: &mut Run, next: Stage| {
        run.stage = next;
        run.tries = 0;
    };
    match run.stage {
        Stage::Setup => {
            progression.0 = career_for(c);
            // Home, and the driver offers it with its kit.
            raid.phase = Phase::Extracting { left: 0.3 };
            push(&mut bot, vec![Task::Wait(1.5)]);
            advance(&mut run, Stage::Offered);
        }
        Stage::Offered => {
            if raid.phase != Phase::AtBase || scene.handovers.busy() {
                push(&mut bot, vec![Task::Wait(0.5)]);
                return;
            }
            let on = progression.0.contract.as_ref().map(|a| a.id.as_str());
            if on == Some(c.id) {
                pass(&mut bot, format!("the driver offers {}", c.name));
            } else {
                fail(
                    &mut bot,
                    format!("{} not offered (under way: {on:?})", c.id),
                );
                advance(&mut run, Stage::Done);
                return;
            }
            for (id, n) in c.kit {
                let got = scene
                    .handovers
                    .handed
                    .iter()
                    .filter(|(h, _)| h == id)
                    .count();
                if got as u32 >= *n {
                    pass(&mut bot, format!("{id} through the hatch"));
                } else {
                    fail(&mut bot, format!("{id}: {got} of {n} through the hatch"));
                }
            }
            // What it opens isn't open yet.
            for u in c.unlocks {
                let open = match u {
                    Unlock::Recipe(r) => scene.benches.iter().any(|b| b.taught.contains(r)),
                    Unlock::Valley(v) => progression.0.opens(v),
                };
                if open {
                    fail(&mut bot, format!("{u:?} open before the contract"));
                } else {
                    pass(&mut bot, format!("{u:?} not open yet"));
                }
            }
            advance(&mut run, Stage::Out);
        }
        Stage::Out => {
            let Some(job) = job else {
                push(&mut bot, vec![Task::Wait(0.5)]);
                return;
            };
            run.step = progression.0.contract.as_ref().map_or(0, |a| a.step);
            bot.log.push(format!("step {}: {job:?}", run.step + 1));
            let mut t = Vec::new();
            // The kit it needs, off the ledge.
            let kit = match &job {
                Job::Plant { item, .. } => Some(item.as_str()),
                Job::Photograph { .. } => Some("camera"),
                _ => None,
            };
            if let Some(id) = kit {
                let have = scene
                    .items
                    .iter()
                    .any(|(e, ..)| scene.id(e) == Some(id) && scene.carried(e, player));
                if !have {
                    if scene.holding(player, Side::Right).is_some() {
                        t.push(Task::Stow(Side::Right));
                    }
                    t.push(Task::PickUp {
                        id: id.to_owned(),
                        side: Side::Right,
                    });
                }
                // The camera stays in hand; the relay goes on the belt.
                if id != "camera" {
                    t.push(Task::Stow(Side::Right));
                }
            }
            // Out of the back, down its steps, and away up the road.
            t.push(Task::WalkTo {
                to: bed(0.05, 0.3),
                within: 0.4,
            });
            t.push(Task::WalkTo {
                to: bed(0.0, 4.0),
                within: 0.8,
            });
            t.extend(legs(bed(0.0, 4.0), bed(0.0, 20.0), 1.5));
            push(&mut bot, t);
            run.laid = false;
            advance(&mut run, Stage::Go);
        }
        Stage::Go => {
            if raid.phase == Phase::AtBase {
                if run.tries > TRIES {
                    fail(&mut bot, "never got out of the truck".into());
                    advance(&mut run, Stage::Done);
                } else {
                    push(&mut bot, legs(here, bed(0.0, 20.0), 1.5));
                }
                return;
            }
            let Some(job) = job else {
                fail(&mut bot, "no job on the card".into());
                advance(&mut run, Stage::Done);
                return;
            };
            let truck_at = truck.translation().xz();
            // Most of the way there in a blink, and the rest on foot.
            let approach = |to: Vec2, within: f32| -> Vec<Task> {
                // From the truck's side if the way's clear, else round
                // till it is (the chimney's yard has walls).
                let first = (truck_at - to).normalize_or(Vec2::Y);
                let short = (0..16)
                    .map(|k| {
                        Vec2::from_angle(k as f32 * std::f32::consts::TAU / 16.0).rotate(first)
                    })
                    .map(|d| to + d * SHORT)
                    .find(|from| scene.clear(*from, to, SHORT - within))
                    .unwrap_or(to + first * SHORT);
                let mut t = vec![Task::Teleport(Vec3::new(short.x, 0.0, short.y))];
                t.extend(legs(short, to, within));
                t
            };
            let mut t = Vec::new();
            match &job {
                Job::Scout { at, .. } => {
                    // Near enough to see it (a chimney or a mast is no
                    // place to stand in).
                    t.extend(approach(Vec2::from_array(*at), SCOUT_RADIUS * 0.6));
                    t.push(Task::Wait(1.0));
                }
                Job::Plant { item, at, .. } => {
                    t.extend(approach(Vec2::from_array(*at), 1.0));
                    // Out of its slot (or the pack), and set down here.
                    if let Some(e) = scene.carrying(item, player).first() {
                        t.extend(scene.take(*e, player, Side::Right));
                    } else {
                        fail(&mut bot, format!("no {item} to plant"));
                    }
                    t.extend([
                        Task::Wait(0.5),
                        Task::LookAt(Vec3::new(at[0], root.translation.y, at[1])),
                        Task::Release(Side::Right),
                        Task::Wait(1.5),
                    ]);
                }
                Job::Bring { item, at, .. } => {
                    let Some((_, p)) = scene.loose(item, Vec2::from_array(*at)) else {
                        if run.tries > TRIES {
                            fail(&mut bot, format!("no {item} laid by its place"));
                            advance(&mut run, Stage::Done);
                        } else {
                            push(&mut bot, vec![Task::Wait(1.0)]);
                        }
                        return;
                    };
                    bot.log.push(format!("{item} lies at {:.0?}", p));
                    // Indoors: up to its door, open it, and in (and through
                    // an inner door, if the room it's in has one nearer).
                    let building = scene.entrances.building_at(p);
                    let shell = building
                        .and_then(|b| scene.entrances.shells.iter().find(|s| s.building == b));
                    let doors: Vec<_> = scene
                        .entrances
                        .doors
                        .iter()
                        .filter(|d| Some(d.building) == building)
                        .collect();
                    // In the shell's walls, or a wall within it.
                    let outer = |d: &&crate::robots::doors::Entrance| {
                        shell.is_some_and(|s| {
                            let l = s.xf.compute_affine().inverse().transform_point3(d.mid);
                            l.x.abs() > s.half.x - 0.6 || l.z.abs() > s.half.y - 0.6
                        })
                    };
                    let nearest = |want_outer: bool| {
                        doors
                            .iter()
                            .filter(|d| outer(d) == want_outer)
                            .min_by(|a, b| a.mid.distance(p).total_cmp(&b.mid.distance(p)))
                            .copied()
                    };
                    let front_door = nearest(true);
                    let inner = nearest(false).filter(|i| {
                        front_door.is_some_and(|f| i.mid.distance(p) < f.mid.distance(p))
                    });
                    match front_door {
                        Some(d) => {
                            let out = d.out.xz().normalize_or(Vec2::Y);
                            let front = d.mid.xz() + out * 1.3;
                            let short = d.mid.xz() + out * SHORT;
                            let inside = d.mid.xz() - out * 1.5;
                            t.push(Task::Teleport(Vec3::new(short.x, 0.0, short.y)));
                            t.extend(legs(short, front, 0.4));
                            t.extend([
                                Task::OpenDoor(d.door),
                                Task::WalkTo {
                                    to: inside,
                                    within: 0.5,
                                },
                            ]);
                            if let Some(i) = inner {
                                // From this side of it to the other.
                                let n = i.out.xz().normalize_or(Vec2::Y);
                                let k = if (inside - i.mid.xz()).dot(n) > 0.0 {
                                    1.0
                                } else {
                                    -1.0
                                };
                                t.extend([
                                    Task::WalkTo {
                                        to: i.mid.xz() + n * k * 1.3,
                                        within: 0.4,
                                    },
                                    Task::OpenDoor(i.door),
                                    Task::WalkTo {
                                        to: i.mid.xz() - n * k * 1.3,
                                        within: 0.5,
                                    },
                                ]);
                            }
                        }
                        None => t.extend(approach(p.xz(), 1.0)),
                    }
                    t.extend([
                        Task::PickUp {
                            id: item.clone(),
                            side: Side::Right,
                        },
                        Task::Stow(Side::Right),
                    ]);
                }
                Job::Fetch { item, n } => {
                    // Found out here (the stash starts empty): a few
                    // strides on, picked up and packed.
                    if !run.laid {
                        let ahead = here + (here - truck_at).normalize_or(Vec2::NEG_Y) * 6.0;
                        if let Some(kind) = scene.catalog.kind(item) {
                            for i in 0..*n {
                                let p = ahead + Vec2::new(i as f32 * 0.6, 0.0);
                                spawn_item(
                                    &mut commands,
                                    &scene.catalog,
                                    SpawnItem {
                                        kind,
                                        transform: Transform::from_translation(
                                            scene.zone.on_ground(p.x, p.y) + Vec3::Y * 0.1,
                                        ),
                                        condition: 0.8,
                                    },
                                );
                            }
                        }
                        run.laid = true;
                        push(&mut bot, vec![Task::Wait(0.5)]);
                        return;
                    }
                    for _ in 0..*n {
                        t.extend([
                            Task::PickUp {
                                id: item.clone(),
                                side: Side::Right,
                            },
                            Task::Stow(Side::Right),
                        ]);
                    }
                }
                Job::Photograph { subject, .. } => {
                    let ahead = here + (here - truck_at).normalize_or(Vec2::NEG_Y) * SUBJECT_AT;
                    let ground = scene.zone.on_ground(ahead.x, ahead.y);
                    match subject {
                        Subject::Walker => t.push(Task::Spawn {
                            kind: RobotKind::Walker,
                            at: ground,
                        }),
                        Subject::Heavy => {
                            run.subject = Some(crate::robots::heavy::spawn_heavy(
                                &mut commands,
                                &scene.zone,
                                &scene.catalog,
                                ground,
                                crate::robots::Job {
                                    kind: crate::robots::JobKind::Survey,
                                    site: ground,
                                    duration: 120.0,
                                    exit: ground + Vec3::new(0.0, 0.0, -300.0),
                                },
                            ));
                        }
                    }
                    let camera = scene
                        .items
                        .iter()
                        .find(|(e, ..)| scene.id(*e) == Some("camera") && scene.carried(*e, player))
                        .map(|(e, ..)| e);
                    match camera {
                        Some(cam) => t.extend(scene.take(cam, player, Side::Right)),
                        None => fail(&mut bot, "no camera".into()),
                    }
                    t.push(Task::Wait(0.5));
                }
                _ => {
                    fail(&mut bot, format!("not a contract's step: {job:?}"));
                    advance(&mut run, Stage::Done);
                    return;
                }
            }
            push(&mut bot, t);
            advance(&mut run, Stage::Check);
        }
        Stage::Check => {
            // The photograph: the head on its subject (the director keeps
            // it there as it moves), and press.
            if let Some(Job::Photograph { subject, taken }) = &job {
                if !taken && run.tries <= 3 {
                    if run.subject.is_none() {
                        run.subject = scene.subject(*subject).map(|(e, _)| e);
                    }
                    if run.tries > 1 {
                        bot.log.push("(not in the picture: again)".into());
                    }
                    push(
                        &mut bot,
                        vec![Task::Wait(1.5), Task::Use(Side::Right), Task::Wait(0.5)],
                    );
                    return;
                }
            }
            let what = match &job {
                Some(Job::Scout { place, seen, .. }) => (*seen, format!("saw {place}")),
                Some(Job::Plant {
                    place,
                    item,
                    planted,
                    ..
                }) => (*planted, format!("{item} planted by {place}")),
                Some(Job::Photograph { subject, taken }) => {
                    (*taken, format!("photographed {}", subject.name()))
                }
                Some(Job::Bring { item, .. }) => (
                    !scene.carrying(item, player).is_empty(),
                    format!("carrying {item}"),
                ),
                Some(Job::Fetch { item, n }) => {
                    let got = scene.carrying(item, player).len() as u32;
                    (got >= *n, format!("carrying {got} of {n} {item}"))
                }
                other => (false, format!("the step: {other:?}")),
            };
            if what.0 {
                pass(&mut bot, what.1);
            } else {
                fail(&mut bot, what.1);
            }
            if let Some(e) = run.subject.take() {
                commands.entity(e).despawn();
            }
            // Home.
            raid.phase = Phase::Extracting { left: 0.3 };
            push(&mut bot, vec![Task::Wait(1.0)]);
            advance(&mut run, Stage::Home);
        }
        Stage::Home => {
            if raid.phase != Phase::AtBase {
                push(&mut bot, vec![Task::Wait(0.5)]);
                return;
            }
            run.carry = match &job {
                Some(Job::Bring { item, .. }) => scene.carrying(item, player),
                Some(Job::Fetch { item, n }) => scene
                    .carrying(item, player)
                    .into_iter()
                    .take(*n as usize)
                    .collect(),
                _ => Vec::new(),
            };
            let next = if run.carry.is_empty() {
                Stage::Press
            } else {
                Stage::Take
            };
            advance(&mut run, next);
            push(&mut bot, vec![Task::Wait(0.2)]);
        }
        Stage::Take => {
            let Some(&e) = run.carry.first() else {
                advance(&mut run, Stage::Press);
                return;
            };
            // Up to the crate, then it in hand.
            let mut t = vec![Task::WalkTo {
                to: bed(-0.3, 0.2),
                within: 0.4,
            }];
            t.extend(scene.take(e, player, Side::Right));
            t.push(Task::Wait(1.0));
            push(&mut bot, t);
            advance(&mut run, Stage::Aim);
        }
        Stage::Aim => {
            let Some(&e) = run.carry.first() else {
                advance(&mut run, Stage::Press);
                return;
            };
            // Bent down to the crate.
            if !input.crouch {
                input.crouch = true;
                push(&mut bot, vec![Task::Wait(1.0)]);
                return;
            }
            let held = scene.holding(player, Side::Right) == Some(e);
            let head = scene
                .heads
                .iter()
                .find(|(_, c)| c.parent() == player)
                .map(|(g, _)| *g);
            let (Some(head), Ok((_, _, item_gt, ..)), true) = (head, scene.items.get(e), held)
            else {
                fail(
                    &mut bot,
                    format!("{:?} not in hand for the crate", scene.id(e)),
                );
                run.carry.remove(0);
                input.crouch = false;
                advance(&mut run, Stage::Take);
                return;
            };
            let at = item_gt.translation();
            let local = stash_gt.affine().inverse().transform_point3(at);
            let opening = stash.half * 1.3;
            if run.tries > 1 && local.abs().cmplt(opening - Vec3::splat(0.05)).all() {
                push(&mut bot, vec![Task::Release(Side::Right), Task::Wait(0.5)]);
                advance(&mut run, Stage::Stowed);
                return;
            }
            if run.tries > TRIES {
                fail(
                    &mut bot,
                    format!("couldn't get it over the crate (at {local:.2?})"),
                );
                run.carry.remove(0);
                input.crouch = false;
                advance(&mut run, Stage::Take);
                return;
            }
            // Where it is in the head's frame stays put as the head turns.
            let v = head.rotation().inverse() * (at - head.translation());
            let eye_h = head.translation().y - root.translation.y;
            let inner = opening * 0.6;
            let found = aim(
                v,
                eye_h,
                root.translation.y,
                truck,
                stash_gt,
                inner,
                here,
                run.tries > 1,
            )
            .or_else(|| {
                aim(
                    v,
                    eye_h,
                    root.translation.y,
                    truck,
                    stash_gt,
                    inner,
                    here,
                    false,
                )
            });
            let Some((stand, look)) = found else {
                fail(
                    &mut bot,
                    format!("nowhere to stand to reach the crate (v {v:.2?})"),
                );
                run.carry.remove(0);
                input.crouch = false;
                advance(&mut run, Stage::Take);
                return;
            };
            let mut t = Vec::new();
            if stand.distance(here) > 0.25 {
                t.push(Task::WalkTo {
                    to: stand,
                    within: 0.25,
                });
            }
            t.extend([Task::LookAt(look), Task::Wait(1.0)]);
            push(&mut bot, t);
        }
        Stage::Stowed => {
            let e = run.carry.remove(0);
            let id = scene.id(e).unwrap_or("?").to_owned();
            if scene.in_stash(e) {
                pass(&mut bot, format!("{id} into the stash crate"));
            } else {
                fail(&mut bot, format!("{id} isn't in the stash crate"));
            }
            input.crouch = false;
            let next = if run.carry.is_empty() {
                Stage::Press
            } else {
                Stage::Take
            };
            advance(&mut run, next);
            push(&mut bot, vec![Task::Wait(0.5)]);
        }
        Stage::Press => {
            let Some(card) = scene.cards.iter().next() else {
                fail(&mut bot, "no job card".into());
                advance(&mut run, Stage::Done);
                return;
            };
            push(
                &mut bot,
                vec![
                    Task::WalkTo {
                        to: bed(0.0, 0.9),
                        within: 0.4,
                    },
                    Task::Press(card),
                    Task::Wait(1.0),
                ],
            );
            advance(&mut run, Stage::Handed);
        }
        Stage::Handed => {
            let a = progression
                .0
                .contract
                .as_ref()
                .map(|a| (a.id.clone(), a.step));
            let done = progression.0.contracts_done.iter().any(|d| d == c.id);
            match a {
                Some((id, step)) if id == c.id && step == run.step + 1 => {
                    pass(&mut bot, format!("step {} handed in", run.step + 1));
                    advance(&mut run, Stage::Out);
                    push(&mut bot, vec![Task::Wait(0.5)]);
                }
                _ if done => {
                    pass(
                        &mut bot,
                        format!("step {} handed in: {} done", run.step + 1, c.name),
                    );
                    advance(&mut run, Stage::Paid);
                    push(&mut bot, vec![Task::Wait(0.5)]);
                }
                _ => {
                    let said: Vec<&str> = raid.log.iter().map(|l| l.0.as_str()).collect();
                    fail(
                        &mut bot,
                        format!(
                            "step {} not taken: {a:?}; the driver: {said:?}",
                            run.step + 1
                        ),
                    );
                    advance(&mut run, Stage::Done);
                }
            }
        }
        Stage::Paid => {
            if scene.handovers.busy() {
                push(&mut bot, vec![Task::Wait(0.5)]);
                return;
            }
            for (id, n) in c.pay {
                let kit = c
                    .kit
                    .iter()
                    .filter(|(k, _)| k == id)
                    .map(|(_, n)| n)
                    .sum::<u32>();
                let got = scene
                    .handovers
                    .handed
                    .iter()
                    .filter(|(h, _)| h == id)
                    .count() as u32;
                if got >= n + kit {
                    pass(&mut bot, format!("paid {n} {id} through the hatch"));
                } else {
                    fail(
                        &mut bot,
                        format!("paid {} of {n} {id}", got.saturating_sub(kit)),
                    );
                }
            }
            for u in c.unlocks {
                match u {
                    Unlock::Recipe(r) => {
                        if scene.benches.iter().any(|b| b.taught.contains(r)) {
                            pass(&mut bot, format!("the bench knows {r}"));
                        } else {
                            fail(&mut bot, format!("the bench doesn't know {r}"));
                        }
                    }
                    Unlock::Valley(v) => {
                        if progression.0.opens(v) {
                            pass(&mut bot, format!("the route card offers {v}"));
                        } else {
                            fail(&mut bot, format!("{v} still kept back"));
                        }
                    }
                }
            }
            advance(&mut run, Stage::Done);
        }
        Stage::Done => {
            bot.tasks.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_contract_has_its_run() {
        for c in CONTRACTS {
            assert!(
                SCENARIOS.iter().any(|(_, id)| *id == c.id),
                "{}: no scenario",
                c.id
            );
        }
        for (s, _) in SCENARIOS {
            assert!(crate::bot::scenarios::OPT_IN.contains(s), "{s}: not opt-in");
        }
    }
}
