//! Getting things into the truck's stash crate by hand, as a player
//! does: each into the right hand, stood in the bed and bent down to the
//! crate, held over its opening, let go. For the bots that bring things
//! home (`progression::contracts::scenario`, `onboarding::first_hour`).

use bevy::prelude::*;

use super::Task;
use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{Held, InSocket, Socket, SocketOwner};
use crate::raid::truck::{FLOOR, Stash, Truck};
use crate::rig::{RigHead, RigInput, Side};

/// Where to stand in the bed, and where to look, so that a thing held at
/// `v` (in the head's frame) is in the stash crate's opening (within
/// `inner` of its middle, crate space). From `from` only, if `fixed`.
#[allow(clippy::too_many_arguments)]
pub fn aim(
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

/// What getting things into the crate reads of the world.
#[derive(bevy::ecs::system::SystemParam)]
pub struct CrateWorld<'w, 's> {
    trucks: Query<'w, 's, &'static GlobalTransform, With<Truck>>,
    stash: Query<'w, 's, (&'static GlobalTransform, &'static Backpack), With<Stash>>,
    items: Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            Option<&'static Held>,
            Option<&'static InSocket>,
            Option<&'static InBackpack>,
        ),
    >,
    sockets: Query<'w, 's, (Entity, &'static SocketOwner), With<Socket>>,
    heads: Query<'w, 's, (&'static GlobalTransform, &'static ChildOf), With<RigHead>>,
}

impl CrateWorld<'_, '_> {
    pub fn in_stash(&self, e: Entity) -> bool {
        self.stash.iter().any(|(_, b)| b.contents.contains(&e))
    }

    fn holding(&self, e: Entity, player: Entity, side: Side) -> bool {
        self.items
            .get(e)
            .is_ok_and(|(_, held, ..)| held.is_some_and(|h| h.player == player && h.side == side))
    }

    /// Into the `side` hand, from wherever the bot has it: a body slot
    /// (drawn), the pack (pulled round, taken out, put back) or the
    /// ground.
    fn take(&self, e: Entity, player: Entity, side: Side) -> Vec<Task> {
        let mut t = vec![Task::Stow(side)];
        let Ok((_, held, sock, bag)) = self.items.get(e) else {
            return t;
        };
        if held.is_some_and(|h| h.player == player && h.side == side) {
            return vec![];
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
                t.extend([Task::Draw { slot, side }, Task::Wait(0.5)]);
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
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    #[default]
    Take,
    Aim,
    Stowed,
}

/// Things to get into the crate, one at a time, and how it went.
#[derive(Default)]
pub struct Crating {
    pub carry: Vec<Entity>,
    pub stowed: Vec<Entity>,
    pub missed: Vec<(Entity, String)>,
    stage: Stage,
    tries: u32,
}

impl Crating {
    pub fn new(carry: Vec<Entity>) -> Self {
        Self { carry, ..default() }
    }

    pub fn done(&self) -> bool {
        self.carry.is_empty()
    }

    fn next(&mut self, stage: Stage) {
        self.stage = stage;
        self.tries = 0;
    }

    fn miss(&mut self, why: String, input: &mut RigInput) {
        let e = self.carry.remove(0);
        self.missed.push((e, why));
        input.crouch = false;
        self.next(Stage::Take);
    }

    /// The next few tasks towards it, once the last lot are done; none when
    /// everything is in (or given up on).
    pub fn step(
        &mut self,
        w: &CrateWorld,
        player: Entity,
        root: &Transform,
        input: &mut RigInput,
    ) -> Vec<Task> {
        let (Some(truck), Some((stash_gt, stash))) =
            (w.trucks.iter().next(), w.stash.iter().next())
        else {
            for e in std::mem::take(&mut self.carry) {
                self.missed.push((e, "no truck".into()));
            }
            return vec![];
        };
        let bed = |x: f32, z: f32| truck.transform_point(Vec3::new(x, 0.0, z)).xz();
        let here = root.translation.xz();
        let Some(&e) = self.carry.first() else {
            return vec![];
        };
        self.tries += 1;
        match self.stage {
            Stage::Take => {
                self.next(Stage::Aim);
                let mut t = vec![Task::WalkTo {
                    to: bed(-0.3, 0.2),
                    within: 0.4,
                }];
                t.extend(w.take(e, player, Side::Right));
                t.push(Task::Wait(1.0));
                t
            }
            Stage::Aim => {
                // Bent down to the crate.
                if !input.crouch {
                    input.crouch = true;
                    return vec![Task::Wait(1.0)];
                }
                let head = w
                    .heads
                    .iter()
                    .find(|(_, c)| c.parent() == player)
                    .map(|(g, _)| *g);
                let (Some(head), Ok((item_gt, ..)), true) =
                    (head, w.items.get(e), w.holding(e, player, Side::Right))
                else {
                    self.miss("not in hand for the crate".into(), input);
                    return vec![Task::Wait(0.5)];
                };
                let at = item_gt.translation();
                let local = stash_gt.affine().inverse().transform_point3(at);
                let opening = stash.half * 1.3;
                if self.tries > 2 && local.abs().cmplt(opening - Vec3::splat(0.05)).all() {
                    self.next(Stage::Stowed);
                    return vec![Task::Release(Side::Right), Task::Wait(0.5)];
                }
                if self.tries > 8 {
                    self.miss(
                        format!("couldn't get it over the crate (at {local:.2?})"),
                        input,
                    );
                    return vec![Task::Wait(0.5)];
                }
                // Where it is in the head's frame stays put as the head turns.
                let v = head.rotation().inverse() * (at - head.translation());
                let eye_h = head.translation().y - root.translation.y;
                let inner = opening * 0.6;
                let fixed = self.tries > 2;
                let found = aim(
                    v,
                    eye_h,
                    root.translation.y,
                    truck,
                    stash_gt,
                    inner,
                    here,
                    fixed,
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
                    self.miss("nowhere to stand to reach the crate".into(), input);
                    return vec![Task::Wait(0.5)];
                };
                let mut t = Vec::new();
                if stand.distance(here) > 0.25 {
                    t.push(Task::WalkTo {
                        to: stand,
                        within: 0.25,
                    });
                }
                t.extend([Task::LookAt(look), Task::Wait(1.0)]);
                t
            }
            Stage::Stowed => {
                let e = self.carry.remove(0);
                if w.in_stash(e) {
                    self.stowed.push(e);
                } else {
                    self.missed.push((e, "let go, but not in the crate".into()));
                }
                input.crouch = false;
                self.next(Stage::Take);
                vec![Task::Wait(0.5)]
            }
        }
    }
}
