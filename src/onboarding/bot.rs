//! `--scenario onboarding`: a bot does the training yard end to end, as
//! someone new would, from the truck bed to pulling the cord at the north
//! post. Each step is done with the bot's ordinary tasks (the same hand
//! actions flat mode sends), and the scenario passes only if the yard
//! noticed every step itself: nothing here marks a step done.

use bevy::prelude::*;

use super::{Goal, Onboarding, Placed, STATIONS, station_of};
use crate::bot::{Bot, Task};
use crate::interact::Held;
use crate::interact::body::Backpack;
use crate::items::{Catalog, Item};
use crate::raid::truck::{Pressable, Stash};
use crate::rig::{LocalPlayer, Side};
use crate::world::Zone;
use crate::world::buildings::{Building, DoorLeaf};

pub const SCENARIO: &str = "onboarding";

/// The last task while the course runs: the director fills in each goal's
/// tasks ahead of it.
const IDLE: f32 = 4.75;

pub fn opening() -> Vec<Task> {
    vec![
        Task::Log("onboarding: a first run of the training yard, every station".into()),
        Task::Wait(IDLE),
    ]
}

/// Goals the director gave up on, and how many tries the current one has
/// had.
#[derive(Default)]
pub struct Director {
    goal: Option<Goal>,
    tries: u32,
    reported: bool,
    /// Metres walked so far, and where the bot was last frame.
    walked: f32,
    last: Option<Vec2>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn direct(
    opts: Res<crate::launch::LaunchOptions>,
    mut ob: ResMut<Onboarding>,
    mut bots: Query<(Entity, &mut Bot, &Transform, &mut crate::rig::RigInput), With<LocalPlayer>>,
    world: DirectorWorld,
    mut keys: Option<ResMut<ButtonInput<KeyCode>>>,
    mut commands: Commands,
    mut state: Local<Director>,
    time: Res<Time>,
) {
    if opts.scenario.as_deref() != Some(SCENARIO) {
        return;
    }
    let Some((player, mut bot, root, mut input)) = bots.iter_mut().next() else {
        return;
    };
    // How far it's walked: the yard's length, and where it doubles back.
    let at = root.translation.xz();
    if let Some(last) = state.last {
        let step = at.distance(last);
        if step < 2.0 {
            state.walked += step;
        }
    }
    state.last = Some(at);
    // The crouch toggle, as the stick or C would: down for its step, up
    // after.
    let crouch = ob.next_goal() == Some(Goal::Crouch);
    if input.crouch != crouch {
        input.crouch = crouch;
    }
    // H on the keyboard to check yourself (the bot plays flat).
    if ob.next_goal() == Some(Goal::Wrist) {
        match keys.as_mut() {
            Some(k) => k.press(KeyCode::KeyH),
            None => commands.insert_resource(ButtonInput::<KeyCode>::default()),
        }
    } else if let Some(k) = keys.as_mut() {
        if k.pressed(KeyCode::KeyH) {
            k.release(KeyCode::KeyH);
        }
    }
    // The gun hand's A (flat: B) for the magazine release, likewise.
    let release = ob.next_goal() == Some(Goal::DropMag) && bot.elapsed > 0.3;
    if input.hand(Side::Right).primary != release {
        input.hand_mut(Side::Right).primary = release;
    }
    // Only when the bot has nothing left but to idle, and a moment after
    // its last task, so what it did has had time to register.
    let idling =
        bot.tasks.len() == 1 && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE);
    if !idling || bot.elapsed < 0.6 {
        return;
    }
    let Some(goal) = ob.next_goal() else {
        if !state.reported {
            state.reported = true;
            let n = super::all_goals().count();
            let skipped = ob.skipped.len();
            if skipped == 0 {
                bot.passed += 1;
                bot.log.push(format!("PASS all {n} steps noticed"));
                bot.log.push(format!(
                    "the yard took {:.0} s and {:.0} m on foot",
                    time.elapsed_secs(),
                    state.walked
                ));
            } else {
                bot.failed += 1;
                bot.log.push(format!(
                    "FAIL {skipped} of {n} steps never registered: {:?}",
                    ob.skipped
                ));
            }
        }
        return;
    };
    if state.goal == Some(goal) {
        state.tries += 1;
    } else {
        state.goal = Some(goal);
        state.tries = 0;
        bot.log.push(format!(
            "{goal:?} ({:.0} s, {:.0} m)",
            time.elapsed_secs(),
            state.walked
        ));
    }
    if state.tries >= 3 {
        // Stuck: skip it, as a person would with the SKIP card, and fail.
        bot.failed += 1;
        bot.log.push(format!("FAIL {goal:?} never registered"));
        ob.skipped.push(goal);
        ob.done.push(goal);
        return;
    }
    if state.tries > 0 {
        bot.log.push(format!("  again ({})", state.tries + 1));
    }
    bot.sprint = true;
    let here = Vec2::new(root.translation.x, root.translation.z);
    let tasks = world.tasks_for(goal, player, here);
    let idle = bot.tasks.pop_back();
    bot.tasks.extend(tasks);
    bot.tasks.extend(idle);
}

#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct DirectorWorld<'w, 's> {
    zone: Res<'w, Zone>,
    catalog: Res<'w, Catalog>,
    items: Query<
        'w,
        's,
        (
            Entity,
            &'static Item,
            &'static GlobalTransform,
            Option<&'static Placed>,
            Option<&'static Held>,
            Option<&'static crate::interact::InSocket>,
        ),
    >,
    stash: Query<'w, 's, &'static Backpack, With<Stash>>,
    pressables: Query<'w, 's, (Entity, &'static Pressable)>,
    cards: Query<'w, 's, (Entity, &'static super::OnboardCard)>,
    doors: Query<'w, 's, (Entity, &'static DoorLeaf, &'static GlobalTransform)>,
    buildings: Query<'w, 's, (&'static Building, &'static GlobalTransform)>,
    hands: Query<'w, 's, (&'static crate::rig::RigHand, &'static crate::interact::Hand)>,
    doorway: Option<Res<'w, super::Doorway>>,
}

impl DirectorWorld<'_, '_> {
    fn id(&self, e: Entity) -> Option<&str> {
        self.items
            .get(e)
            .ok()
            .map(|(_, it, ..)| self.catalog.get(it.kind()).id.as_str())
    }

    fn in_stash(&self, id: &str) -> Option<Entity> {
        self.stash
            .iter()
            .flat_map(|b| b.contents.iter().copied())
            .find(|e| self.id(*e) == Some(id))
    }

    fn placed(&self, which: Placed) -> Option<(Entity, Vec3)> {
        self.items
            .iter()
            .find(|(.., p, _, _)| *p == Some(&which))
            .map(|(e, _, gt, ..)| (e, gt.translation()))
    }

    /// The nearest loose item of `id` to `near`.
    fn loose(&self, id: &str, near: Vec2) -> Option<Entity> {
        self.items
            .iter()
            .filter(|(e, it, _, _, held, sock)| {
                held.is_none()
                    && sock.is_none()
                    && self.catalog.get(it.kind()).id == id
                    && !self.stash.iter().any(|b| b.contents.contains(e))
            })
            .min_by(|a, b| {
                a.2.translation()
                    .xz()
                    .distance(near)
                    .total_cmp(&b.2.translation().xz().distance(near))
            })
            .map(|(e, ..)| e)
    }

    fn holding(&self, side: Side) -> Option<Entity> {
        self.hands
            .iter()
            .find(|(rh, _)| rh.side == side)
            .and_then(|(_, h)| h.holding)
    }

    /// Hands empty: whatever's in them goes away first.
    fn free_hands(&self) -> Vec<Task> {
        let mut t = Vec::new();
        for side in Side::BOTH {
            if self.holding(side).is_some() {
                t.push(Task::Stow(side));
            }
        }
        t
    }

    /// Walk there in legs a bot can do inside its per-task time.
    fn walk(&self, from: Vec2, to: Vec2, within: f32) -> Vec<Task> {
        let d = from.distance(to);
        let legs = (d / 14.0).ceil().max(1.0) as usize;
        (1..=legs)
            .map(|k| Task::WalkTo {
                to: from.lerp(to, k as f32 / legs as f32),
                within: if k == legs { within } else { 1.5 },
            })
            .collect()
    }

    /// Up the track to a point: along the gravel (x = 0), then across.
    fn go(&self, from: Vec2, to: Vec2, within: f32) -> Vec<Task> {
        let mut t = Vec::new();
        let on_track = |p: Vec2| Vec2::new(0.0, p.y);
        let mut at = from;
        if from.distance(to) > 6.0 {
            t.extend(self.walk(at, on_track(from), 1.5));
            at = on_track(from);
            t.extend(self.walk(at, on_track(to), 1.5));
            at = on_track(to);
        }
        t.extend(self.walk(at, to, within));
        t
    }

    fn station(&self, goal: Goal) -> Vec2 {
        Vec2::from(STATIONS[station_of(goal)].at)
    }

    fn tasks_for(&self, goal: Goal, _player: Entity, here: Vec2) -> Vec<Task> {
        use Task::*;
        let mut t = Vec::new();
        match goal {
            Goal::TakePistol | Goal::TakeRadio => {
                let id = if goal == Goal::TakePistol {
                    "pm_pistol"
                } else {
                    "radio"
                };
                let side = if goal == Goal::TakePistol {
                    Side::Right
                } else {
                    Side::Left
                };
                if let Some(e) = self.in_stash(id) {
                    t.push(Grab { item: e, side });
                }
            }
            Goal::Turn => {
                // A look over the shoulder and back.
                t.push(LookAt(Vec3::new(here.x, 13.0, here.y + 10.0)));
                t.push(LookAt(Vec3::new(here.x + 10.0, 13.0, here.y)));
                t.push(LookAt(Vec3::new(here.x, 13.0, here.y - 10.0)));
            }
            Goal::Crouch => t.push(Wait(0.3)),
            Goal::Run => t.extend(self.walk(here, here + Vec2::new(0.0, -6.0), 1.0)),
            Goal::PutAway => {
                if self.holding(Side::Right).is_none() {
                    if let Some(e) = self.in_stash("pm_pistol") {
                        t.push(Grab {
                            item: e,
                            side: Side::Right,
                        });
                    }
                }
                t.push(Stow(Side::Right));
            }
            Goal::Pack => {
                // The cleaning kit is too big for any slot: into the pack.
                t.extend(self.free_hands());
                if let Some(e) = self
                    .in_stash("cleaning_kit")
                    .or_else(|| self.loose("cleaning_kit", here))
                {
                    t.push(Grab {
                        item: e,
                        side: Side::Right,
                    });
                    t.push(Stow(Side::Right));
                }
            }
            Goal::FarGrab => {
                if let Some((e, at)) = self.placed(Placed::FarBox) {
                    t.extend(self.free_hands());
                    // Down off the truck and round to the pen's fence.
                    let pen = Vec2::from(super::FAR_STAND_AT);
                    let front = pen + Vec2::new(0.0, super::PEN_HALF + 0.4);
                    t.extend(self.go(here, front, 0.4));
                    t.push(LookAt(at));
                    t.push(GrabFromHere {
                        item: e,
                        side: Side::Right,
                    });
                }
            }
            Goal::FillMag
            | Goal::SeatMag
            | Goal::Rack
            | Goal::Safety
            | Goal::HitPlate
            | Goal::DropMag => {
                let table = Vec2::from(super::RANGE_TABLE_AT);
                let stand = table + Vec2::new(0.0, 0.9);
                if here.distance(stand) > 2.0 {
                    t.extend(self.go(here, stand, 0.5));
                }
                match goal {
                    Goal::FillMag => {
                        t.extend(self.free_hands());
                        if let (Some(mag), Some(ammo)) =
                            (self.loose("pm_mag", table), self.loose("ammo_9x18", table))
                        {
                            t.push(Grab {
                                item: mag,
                                side: Side::Left,
                            });
                            t.push(Grab {
                                item: ammo,
                                side: Side::Right,
                            });
                            t.push(Combine);
                            t.push(Release(Side::Right));
                            t.push(Stow(Side::Left));
                        }
                    }
                    Goal::SeatMag => {
                        if self.holding(Side::Right).is_none() {
                            if let Some(gun) = self.loose("pm_pistol", table) {
                                t.push(Grab {
                                    item: gun,
                                    side: Side::Right,
                                });
                            }
                        }
                        t.push(Gun(crate::weapons::flat::MacroKind::Reload));
                    }
                    Goal::Rack => t.push(Gun(crate::weapons::flat::MacroKind::Rack)),
                    Goal::Safety => t.push(Gun(crate::weapons::flat::MacroKind::Mode)),
                    Goal::DropMag => t.push(Wait(0.6)),
                    _ => {
                        let plate = self
                            .buildings
                            .iter()
                            .filter(|(b, _)| b.prefab == "target")
                            .map(|(_, gt)| gt.transform_point(Vec3::new(0.0, 1.2, 0.07)))
                            .min_by(|a, b| a.xz().distance(here).total_cmp(&b.xz().distance(here)));
                        if let Some(p) = plate {
                            t.push(Expect(crate::bot::Expect::GunRounds(1, 9)));
                            t.push(Fire { at: p, shots: 3 });
                        }
                    }
                }
            }
            Goal::OpenDoor => {
                let house = self
                    .buildings
                    .iter()
                    .find(|(b, _)| b.prefab.starts_with("house"))
                    .map(|(_, gt)| gt.translation());
                let door = house.and_then(|h| {
                    self.doors.iter().min_by(|a, b| {
                        a.2.translation()
                            .distance(h)
                            .total_cmp(&b.2.translation().distance(h))
                    })
                });
                if let (Some((e, leaf, gt)), Some(h)) = (door, house) {
                    let mid = gt.transform_point(Vec3::new(leaf.0.width * 0.5, 0.0, 0.0));
                    let out = (mid - h).xz().normalize_or(Vec2::X);
                    let front = mid.xz() + out * 1.2;
                    t.extend(self.go(here, front, 1.2));
                    t.push(OpenDoor(e));
                }
            }
            Goal::Search => {
                t.extend(self.free_hands());
                if let Some((e, at)) = self.placed(Placed::HouseFind) {
                    // In at the door, then to the crate beside it.
                    if let Some(d) = &self.doorway {
                        let mid = d.0.xz();
                        let inward = (at.xz() - mid).normalize_or_zero();
                        t.push(WalkTo {
                            to: mid + inward * 0.5,
                            within: 0.4,
                        });
                    }
                    t.push(Grab {
                        item: e,
                        side: Side::Right,
                    });
                }
            }
            Goal::Wrist => {
                t.extend(self.go(here, self.station(goal) + Vec2::new(-1.5, 0.0), 1.0));
                t.push(Wait(0.8));
            }
            Goal::Dressing => {
                let dressing = if self.holding(Side::Right).and_then(|e| self.id(e))
                    == Some("field_dressing")
                {
                    None
                } else {
                    self.loose("field_dressing", here)
                };
                if let Some(e) = dressing {
                    t.extend(self.free_hands());
                    t.push(Grab {
                        item: e,
                        side: Side::Right,
                    });
                }
                t.extend(self.go(here, self.station(goal) + Vec2::new(-1.5, 0.0), 1.0));
                t.push(Use(Side::Right));
            }
            Goal::Probe => {
                t.extend(self.free_hands());
                let crate_at = Vec2::from(super::PARTS_CRATE_AT);
                if let Some(e) = self.loose("spare_parts", crate_at) {
                    t.extend(self.go(here, crate_at + Vec2::new(-1.0, 0.0), 0.6));
                    t.push(Grab {
                        item: e,
                        side: Side::Right,
                    });
                    let snare = Vec2::from(super::SNARE_AT);
                    let from = snare + Vec2::new(-(super::SNARE_R + 1.5), 0.5);
                    t.push(WalkTo {
                        to: from,
                        within: 0.5,
                    });
                    t.push(Throw {
                        side: Side::Right,
                        at: Vec3::new(snare.x, self.zone.ground(snare.x, snare.y) + 0.8, snare.y),
                    });
                    t.push(Wait(1.0));
                }
            }
            Goal::Artifact => {
                if let Some((e, at)) = self.placed(Placed::Stone) {
                    t.extend(self.free_hands());
                    // To the shimmer's edge on the track's side, not in it.
                    let heart = Vec2::from(super::SHIMMER_AT);
                    let edge = heart + Vec2::new(-(super::SHIMMER_R + 1.0), 0.0);
                    t.extend(self.go(here, edge, 0.3));
                    t.push(LookAt(at));
                    t.push(GrabFromHere {
                        item: e,
                        side: Side::Right,
                    });
                }
            }
            Goal::Button => {
                if let Some((e, _)) = self
                    .cards
                    .iter()
                    .find(|(_, c)| **c == super::OnboardCard::Push)
                {
                    t.extend(self.go(here, self.station(goal) + Vec2::new(1.2, 0.0), 0.5));
                    t.push(Press(e));
                }
            }
            Goal::CallTruck => {
                t.extend(self.go(here, self.station(goal) + Vec2::new(0.0, 1.5), 0.8));
                t.extend(self.free_hands());
                let radio = self.loose("radio", Vec2::from(super::HOME_CRATE_AT));
                if let Some(e) = radio {
                    t.push(Grab {
                        item: e,
                        side: Side::Right,
                    });
                    t.push(Use(Side::Right));
                    t.push(Wait(super::HURRY + 1.0));
                }
            }
            Goal::PullCord => {
                if let Some((e, _)) = self
                    .pressables
                    .iter()
                    .find(|(_, p)| **p == Pressable::BellPull)
                {
                    t.push(Press(e));
                }
            }
        }
        let _ = &self.zone;
        t
    }
}
