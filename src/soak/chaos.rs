//! The chaos monkey: a seeded bot at the headset's controls (poses, grip,
//! trigger and buttons into [`RigInput`], through [`Vr`]) doing random but
//! plausible things with its hands and feet: grabbing whatever is in
//! reach, throwing it hard into walls and water, stuffing pouches and the
//! pack, pulling magazines mid-fire, holstering mid-reload, working doors
//! and ladders, crouching, flipping comfort settings. After every action
//! it checks what must always hold (see [`Monkey::check`]); anything that
//! breaks is a finding, with the actions that led to it.
//!
//! `chaos_monkey_short` runs in CI. The long run is
//! `CHAOS_SEEDS=1-20 CHAOS_SECS=600 CHAOS_MAP=marrow_creek cargo test
//! --profile fast --lib chaos_monkey_long -- --ignored --nocapture`
//! (`CHAOS_MAP=all` for every valley; `CHAOS_ROBOTS=1` with robots on).

use std::collections::{HashMap, HashSet, VecDeque};

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::items::Category;

use crate::interact::body::{Backpack, InBackpack};
use crate::interact::hand_climb::Ladder;
use crate::interact::{Grabbable, Hand, Held, InSocket, Socket, SocketKind, SocketOwner};
use crate::items::{Catalog, Item};
use crate::rig::comfort::Comfort;
use crate::rig::{RigHand, RigInput, Side};
use crate::vr_harness::Vr;
use crate::world::Zone;
use crate::world::buildings::DoorLeaf;

/// How far below the ground (or an underground floor) a loose item may be
/// before it's lost through it.
const UNDER: f32 = 0.6;
/// Faster than anything loose should go, m/s: a hard throw is 25, a fall
/// from a rooftop under 20.
const FAST: f32 = 45.0;
/// Frames a hand may keep hold after its grip and trigger have opened.
const LET_GO: u32 = 20;

/// splitmix64: small, seeded, the same everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f()
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
    fn unit(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
                self.range(-1.0, 1.0),
            );
            let l = v.length();
            if l > 0.05 && l <= 1.0 {
                return v / l;
            }
        }
    }
}

/// Something that broke.
#[derive(Clone, Debug)]
pub(crate) struct Finding {
    pub what: String,
    pub map: String,
    pub seed: u64,
    pub frame: u64,
    /// The last actions before it.
    pub trail: Vec<String>,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "[{} seed {} frame {}] {}",
            self.map, self.seed, self.frame, self.what
        )?;
        for t in &self.trail {
            writeln!(f, "    {t}")?;
        }
        Ok(())
    }
}

pub(crate) struct Monkey {
    pub vr: Vr,
    rng: Rng,
    map: String,
    seed: u64,
    frame: u64,
    trail: VecDeque<String>,
    pub findings: Vec<Finding>,
    /// Kinds of the items seen, to tell a consumed one from a lost one.
    items: HashMap<Entity, (String, Category)>,
    /// Items already reported, so one fault is one finding.
    reported: HashSet<(Entity, &'static str)>,
    /// Frames each hand has had its grip and trigger open while it still
    /// held something.
    open_for: [u32; 2],
    /// Last good pose of every loose item, for the write-up of a lost one.
    last_good: HashMap<Entity, (Vec3, Vec3)>,
    /// Walks that went nowhere, in a row.
    stuck_walks: u32,
    /// `CHAOS_TRACE=<entity index>`: print that entity every frame.
    trace: Option<u32>,
}

const SIDES: [Side; 2] = [Side::Left, Side::Right];

impl Monkey {
    pub fn new(map: &str, seed: u64, robots: bool) -> Self {
        let opts = crate::LaunchOptions {
            robots,
            map: map.into(),
            seed,
            ..Default::default()
        };
        let mut vr = Vr::with(opts, None);
        vr.step(10);
        let mut m = Self {
            vr,
            rng: Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0xC4A0),
            map: map.into(),
            seed,
            frame: 0,
            trail: VecDeque::new(),
            findings: Vec::new(),
            items: HashMap::new(),
            reported: HashSet::new(),
            open_for: [0; 2],
            last_good: HashMap::new(),
            stuck_walks: 0,
            trace: std::env::var("CHAOS_TRACE")
                .ok()
                .and_then(|v| v.parse().ok()),
        };
        m.rest(Side::Left, 0.0);
        m.rest(Side::Right, 0.0);
        m.census();
        m
    }

    fn note(&mut self, s: String) {
        if self.trail.len() >= 24 {
            self.trail.pop_front();
        }
        self.trail.push_back(format!("f{} {s}", self.frame));
    }

    fn find(&mut self, what: String) {
        self.findings.push(Finding {
            what,
            map: self.map.clone(),
            seed: self.seed,
            frame: self.frame,
            trail: self.trail.iter().cloned().collect(),
        });
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.vr.step(1);
            self.frame += 1;
            self.watch_hands();
            if let Some(i) = self.trace {
                self.print_trace(i);
            }
        }
    }

    fn feet(&self) -> Vec3 {
        self.vr.feet()
    }

    fn yaw(&self) -> Quat {
        self.vr
            .app
            .world()
            .get::<Transform>(self.vr.player)
            .unwrap()
            .rotation
    }

    fn input(&mut self) -> Mut<'_, RigInput> {
        let p = self.vr.player;
        self.vr.app.world_mut().get_mut::<RigInput>(p).unwrap()
    }

    fn head_world(&self) -> Vec3 {
        let h = self.vr.app.world().get::<RigInput>(self.vr.player).unwrap();
        let t = self
            .vr
            .app
            .world()
            .get::<Transform>(self.vr.player)
            .unwrap();
        t.transform_point(h.head.translation)
    }

    /// A hand back at its side, in the playspace.
    fn rest(&mut self, side: Side, grip: f32) {
        let x = if side == Side::Left { -0.25 } else { 0.25 };
        let y = self.input().head.translation.y - 0.55;
        self.vr.hand_local(side, Vec3::new(x, y, -0.3), grip);
    }

    fn hand(&mut self, side: Side) -> Hand {
        self.vr.hand_state(side)
    }

    fn hand_world(&mut self, side: Side) -> Vec3 {
        self.vr.hand_state(side).world.translation
    }

    fn controls(&self, side: Side) -> (f32, f32) {
        let h = self.vr.app.world().get::<RigInput>(self.vr.player).unwrap();
        let h = h.hand(side);
        (h.grip, h.trigger)
    }

    /// A hand that has opened must let go.
    fn watch_hands(&mut self) {
        for (i, side) in SIDES.into_iter().enumerate() {
            let (grip, trigger) = self.controls(side);
            let h = self.hand(side);
            let busy = h.holding.is_some() || h.handle.is_some() || h.supporting.is_some();
            if grip < 0.1 && trigger < 0.1 && busy {
                self.open_for[i] += 1;
                if self.open_for[i] == LET_GO {
                    let what = format!(
                        "{side:?} hand opened {LET_GO} frames ago and still has holding {:?} handle {:?} supporting {:?}",
                        h.holding.map(|e| self.name(e)),
                        h.handle.map(|(e, k)| (self.name(e), k)),
                        h.supporting.map(|e| self.name(e)),
                    );
                    self.find(what);
                }
            } else {
                self.open_for[i] = 0;
            }
        }
    }

    fn print_trace(&mut self, index: u32) {
        let w = self.vr.app.world_mut();
        let mut q = w.query::<(Entity, &GlobalTransform, Option<&LinearVelocity>)>();
        for (e, g, v) in q.iter(w) {
            if let Some(v) = v
                && v.0.length() > 50.0
                && e.index_u32() != index
            {
                eprintln!(
                    "fast f{} {e} {:?}: at {:.2} v {:.1}",
                    self.frame,
                    w.get::<Name>(e),
                    g.translation(),
                    v.0
                );
            }
            if e.index_u32() == index {
                let r = w.entity(e);
                let names: Vec<String> = r
                    .archetype()
                    .components()
                    .iter()
                    .filter_map(|c| w.components().get_name(*c))
                    .map(|n| n.shortname().to_string())
                    .filter(|n| {
                        [
                            "Held",
                            "InSocket",
                            "InBackpack",
                            "ChildOf",
                            "RigidBodyDisabled",
                            "ColliderDisabled",
                            "Sleeping",
                            "Supported",
                        ]
                        .contains(&n.as_str())
                    })
                    .collect();
                eprintln!(
                    "trace f{} {e}: at {:.2} v {:?} {names:?}",
                    self.frame,
                    g.translation(),
                    v.map(|v| v.0)
                );
            }
        }
    }

    fn name(&self, e: Entity) -> String {
        let w = self.vr.app.world();
        if let Some(i) = w.get::<Item>(e) {
            let c = w.resource::<Catalog>();
            return format!("{}#{}", c.0.get(i.kind()).id, e.index());
        }
        if let Some(n) = w.get::<Name>(e) {
            return format!("{n}#{}", e.index());
        }
        format!("{e}")
    }

    fn census(&mut self) {
        let w = self.vr.app.world_mut();
        let catalog = w.resource::<Catalog>().clone();
        let mut q = w.query::<(Entity, &Item)>();
        for (e, i) in q.iter(w) {
            let def = catalog.0.get(i.kind());
            self.items
                .entry(e)
                .or_insert_with(|| (def.id.clone(), def.category));
        }
    }

    // ---- actions ----

    /// One random action, then the checks.
    pub fn act(&mut self) {
        let r = self.rng.below(100);
        match r {
            0..=11 => self.walk(),
            12..=15 => self.teleport(),
            16..=37 => self.grab(),
            38..=45 => self.wave(),
            46..=53 => self.throw(),
            54..=58 => self.drop_it(),
            59..=65 => self.stow(),
            66..=69 => self.unstow(),
            70..=73 => self.backpack(),
            74..=81 => self.gun_play(),
            82..=85 => self.mag_to_well(),
            86..=87 => self.posture(),
            88..=89 => self.comfort(),
            90..=93 => self.spawn_things(),
            94..=95 => self.poke_button(),
            96 => self.glitch(),
            _ => self.climb(),
        }
        self.census();
        self.check();
    }

    fn walk(&mut self) {
        let dir = Vec2::new(self.rng.range(-1.0, 1.0), self.rng.range(-0.3, 1.0));
        let turn = if self.rng.chance(0.4) {
            self.rng.range(-2.0, 2.0)
        } else {
            0.0
        };
        let sprint = self.rng.chance(0.25);
        let jump = self.rng.chance(0.1);
        let frames = 20 + self.rng.below(100);
        self.note(format!(
            "walk {dir:.2} turn {turn:.1} sprint {sprint} jump {jump} for {frames}"
        ));
        let from = self.feet();
        {
            let mut i = self.input();
            i.locomotion = dir.clamp_length_max(1.0);
            i.turn = turn;
            i.sprint = sprint;
            i.jump = jump;
        }
        self.step(2);
        self.input().jump = false;
        self.step(frames);
        {
            let mut i = self.input();
            i.locomotion = Vec2::ZERO;
            i.turn = 0.0;
            i.sprint = false;
        }
        self.step(3);
        let moved = (self.feet() - from).xz().length();
        if dir.length() > 0.5 && frames > 60 && moved < 0.05 {
            self.stuck_walks += 1;
            if self.stuck_walks == 6 {
                let at = self.feet();
                self.find(format!(
                    "player went nowhere on six walks in a row at {at:.2}"
                ));
            }
        } else if moved > 0.3 {
            self.stuck_walks = 0;
        }
    }

    /// Somewhere interesting: a door, a ladder, the water's edge, an item,
    /// anywhere on the map.
    fn teleport(&mut self) {
        let zone = self.vr.app.world().resource::<Zone>().clone();
        let w = self.vr.app.world_mut();
        let doors: Vec<Vec3> = w
            .query_filtered::<&GlobalTransform, With<DoorLeaf>>()
            .iter(w)
            .map(|g| g.translation())
            .collect();
        let ladders: Vec<Vec3> = w
            .query::<&Ladder>()
            .iter(w)
            .map(|l| l.bottom + l.out * 0.6)
            .collect();
        let items: Vec<Vec3> = w
            .query_filtered::<&GlobalTransform, (With<Item>, Without<Held>, Without<InSocket>)>()
            .iter(w)
            .map(|g| g.translation())
            .collect();
        let water: Vec<Vec3> = zone
            .map
            .water
            .iter()
            .flat_map(|wd| wd.points.iter())
            .map(|p| Vec3::new(p[0], 0.0, p[1]))
            .collect();
        let (kind, at) = match self.rng.below(5) {
            0 if !doors.is_empty() => ("door", doors[self.rng.below(doors.len())]),
            1 if !ladders.is_empty() => ("ladder", ladders[self.rng.below(ladders.len())]),
            2 if !water.is_empty() => ("water", water[self.rng.below(water.len())]),
            3 if !items.is_empty() => ("item", items[self.rng.below(items.len())]),
            _ => {
                let h = zone.map.size * 0.4;
                (
                    "anywhere",
                    Vec3::new(self.rng.range(-h, h), 0.0, self.rng.range(-h, h)),
                )
            }
        };
        let off = Vec3::new(self.rng.range(-1.5, 1.5), 0.0, self.rng.range(-1.5, 1.5));
        // Inside the fence: the map's own edge is walled off.
        let inside = zone.map.size * 0.5 - 20.0;
        let p = (at + off).clamp(
            Vec3::new(-inside, f32::MIN, -inside),
            Vec3::new(inside, f32::MAX, inside),
        );
        // Stand on whatever is there: an underground floor, a building's
        // floor (dropped from just above the spot), or the ground.
        let ground = zone.ground(p.x, p.z);
        let y = crate::world::underground::floor_under(&zone, p)
            .unwrap_or_else(|| ground.max(at.y.min(ground + 6.0)));
        let yaw = self.rng.range(0.0, std::f32::consts::TAU);
        self.note(format!(
            "teleport to {kind} at {:.1}",
            Vec3::new(p.x, y, p.z)
        ));
        self.vr.stand(Vec3::new(p.x, y + 0.05, p.z), yaw);
        self.frame += 2;
        self.stuck_walks = 0;
        self.rest(Side::Left, 0.0);
        self.rest(Side::Right, 0.0);
        self.step(20);
    }

    /// Every grab point within reach, on anything: items, doors, ladders,
    /// handles on a held gun.
    fn reachable(&mut self, from: Vec3, reach: f32) -> Vec<(Entity, Vec3, bool)> {
        let w = self.vr.app.world_mut();
        let mut out = Vec::new();
        let mut q = w.query::<(Entity, &Grabbable, &GlobalTransform)>();
        for (e, g, gt) in q.iter(w) {
            for p in &g.points {
                let at = gt.mul_transform(p.local).translation();
                if at.distance(from) < reach {
                    let handle = matches!(p.role, crate::interact::GripRole::Handle(_));
                    out.push((e, at, handle));
                }
            }
        }
        out.sort_by_key(|(e, _, _)| *e);
        out
    }

    fn free_side(&mut self) -> Side {
        let mut sides = SIDES;
        if self.rng.chance(0.5) {
            sides.reverse();
        }
        for s in sides {
            let h = self.hand(s);
            if h.holding.is_none() && h.handle.is_none() {
                return s;
            }
        }
        sides[0]
    }

    fn reach_to(&mut self, side: Side, to: Vec3, frames: usize) {
        let from = self.hand_world(side);
        let (grip, trigger) = self.controls(side);
        let rot = self.yaw()
            * Quat::from_euler(
                EulerRot::YXZ,
                self.rng.range(-0.6, 0.6),
                self.rng.range(-0.6, 0.6),
                self.rng.range(-0.6, 0.6),
            );
        for k in 1..=frames {
            let p = from.lerp(to, k as f32 / frames as f32);
            self.vr.hand(side, p, rot, grip, trigger);
            self.step(1);
        }
    }

    fn grab(&mut self) {
        let head = self.head_world();
        let mut targets = self.reachable(head - Vec3::Y * 0.4, 1.1);
        if targets.is_empty() {
            // Nothing in reach: bring something.
            self.spawn_things();
            targets = self.reachable(head - Vec3::Y * 0.4, 1.1);
            if targets.is_empty() {
                return;
            }
        }
        let (e, at, handle) = targets[self.rng.below(targets.len())];
        let side = self.free_side();
        if self.hand(side).holding.is_some() {
            // Both full: let one go first, wherever it is.
            self.vr.grip(side, 0.0);
            self.step(3);
        }
        let by_trigger = handle && self.rng.chance(0.8);
        let name = self.name(e);
        self.note(format!(
            "grab {name} with {side:?} at {at:.2} by {}",
            if by_trigger { "trigger" } else { "grip" }
        ));
        self.vr.grip(side, 0.0);
        self.vr.trigger(side, 0.0);
        let frames = 2 + self.rng.below(6);
        self.reach_to(side, at, frames);
        if by_trigger {
            self.vr.trigger(side, 1.0);
        } else {
            self.vr.grip(side, 1.0);
        }
        let n = 3 + self.rng.below(5);
        self.step(n);
    }

    /// Hands anywhere within reach (into walls, the floor, each other).
    fn wave(&mut self) {
        let frames = 5 + self.rng.below(40);
        self.note(format!("wave for {frames}"));
        let head = self.head_world();
        let mut target = [Vec3::ZERO; 2];
        for (i, _) in SIDES.iter().enumerate() {
            target[i] = head + self.rng.unit() * self.rng.range(0.1, 0.9) - Vec3::Y * 0.3;
        }
        let from = [self.hand_world(Side::Left), self.hand_world(Side::Right)];
        for k in 1..=frames {
            let t = k as f32 / frames as f32;
            for (i, s) in SIDES.into_iter().enumerate() {
                self.vr.move_hand(s, from[i].lerp(target[i], t));
            }
            self.step(1);
        }
    }

    fn holding_side(&mut self) -> Option<Side> {
        let held: Vec<Side> = SIDES
            .into_iter()
            .filter(|s| self.hand(*s).holding.is_some())
            .collect();
        (!held.is_empty()).then(|| held[self.rng.below(held.len())])
    }

    fn throw(&mut self) {
        let Some(side) = self.holding_side() else {
            return self.grab();
        };
        let speed = if self.rng.chance(0.2) {
            self.rng.range(15.0, 40.0)
        } else {
            self.rng.range(1.0, 12.0)
        };
        let mut dir = self.rng.unit();
        if self.rng.chance(0.3) {
            dir.y = -dir.y.abs(); // at the floor
        }
        let what = self.hand(side).holding.map(|e| self.name(e));
        self.note(format!(
            "throw {what:?} from {side:?} at {speed:.1} m/s toward {dir:.2}"
        ));
        let frames = 3 + self.rng.below(4);
        let start = self.hand_world(side);
        let per = dir * speed / 90.0;
        for k in 1..=frames {
            self.vr.move_hand(side, start + per * k as f32);
            self.step(1);
        }
        self.vr.grip(side, 0.0);
        self.vr.trigger(side, 0.0);
        self.step(2);
        self.rest(side, 0.0);
        let n = 20 + self.rng.below(60);
        self.step(n);
    }

    fn drop_it(&mut self) {
        let side = SIDES[self.rng.below(2)];
        self.note(format!("open {side:?}"));
        self.vr.grip(side, 0.0);
        self.vr.trigger(side, 0.0);
        let n = 5 + self.rng.below(30);
        self.step(n);
    }

    fn body_sockets(&mut self, occupied: bool) -> Vec<(Entity, Vec3, SocketKind)> {
        let p = self.vr.player;
        let w = self.vr.app.world_mut();
        let mut q = w.query::<(Entity, &Socket, &SocketOwner, &GlobalTransform)>();
        let mut v: Vec<_> = q
            .iter(w)
            .filter(|(_, s, o, _)| {
                o.0 == p
                    && matches!(s.kind, SocketKind::Body(_))
                    && s.occupant.is_some() == occupied
            })
            .map(|(e, s, _, g)| (e, g.translation(), s.kind.clone()))
            .collect();
        v.sort_by_key(|x| x.0);
        v
    }

    /// Whatever's in hand into a pouch, the holster or the open pack;
    /// sometimes with the trigger still held, or the gun mid-reload.
    fn stow(&mut self) {
        let Some(side) = self.holding_side() else {
            return self.grab();
        };
        let socks = self.body_sockets(false);
        let into_pack = self.rng.chance(0.3) || socks.is_empty();
        let to = if into_pack {
            let w = self.vr.app.world_mut();
            let p = self.vr.player;
            let bag = w
                .query::<(&Backpack, &GlobalTransform)>()
                .iter(w)
                .find(|(b, _)| b.owner == p && b.carried)
                .map(|(_, g)| g.translation());
            match bag {
                Some(b) => b + Vec3::Y * 0.1,
                None => return,
            }
        } else {
            socks[self.rng.below(socks.len())].1
        };
        if self.rng.chance(0.3) {
            // Mid-reload: drop the magazine first.
            self.vr.primary(side, true);
            self.step(2);
            self.vr.primary(side, false);
        }
        let what = self.hand(side).holding.map(|e| self.name(e));
        self.note(format!(
            "stow {what:?} from {side:?} into {} at {to:.2}",
            if into_pack { "pack" } else { "body socket" }
        ));
        let n = 3 + self.rng.below(8);
        self.reach_to(side, to, n);
        self.vr.grip(side, 0.0);
        self.vr.trigger(side, 0.0);
        self.step(4);
        self.rest(side, 0.0);
        self.step(4);
    }

    fn unstow(&mut self) {
        let socks = self.body_sockets(true);
        if socks.is_empty() {
            return self.stow();
        }
        let (_, at, kind) = socks[self.rng.below(socks.len())].clone();
        let side = self.free_side();
        self.note(format!("unstow {kind:?} with {side:?}"));
        self.vr.grip(side, 0.0);
        let n = 3 + self.rng.below(6);
        self.reach_to(side, at, n);
        self.vr.grip(side, 1.0);
        self.step(4);
        self.rest(side, 1.0);
        self.step(6);
    }

    /// Over the shoulder for the pack, round to the front, let go; or a
    /// grab at the open pack's insides.
    fn backpack(&mut self) {
        let side = self.free_side();
        let shoulder = if side == Side::Left { -0.15 } else { 0.15 };
        let y = self.input().head.translation.y - 0.1;
        self.note(format!("pack over the {side:?} shoulder"));
        self.vr.grip(side, 0.0);
        self.vr.hand_local(side, Vec3::new(shoulder, y, 0.2), 0.0);
        self.step(4);
        self.vr.grip(side, 1.0);
        self.step(4);
        for k in 1..=8 {
            let t = k as f32 / 8.0;
            self.vr
                .hand_local(side, Vec3::new(shoulder, y - 0.3 * t, 0.2 - 0.6 * t), 1.0);
            self.step(1);
        }
        let behind = self.rng.chance(0.3);
        if behind {
            self.vr.hand_local(side, Vec3::new(shoulder, y, 0.25), 1.0);
            self.step(6);
        }
        self.vr.grip(side, 0.0);
        self.step(6);
        self.rest(side, 0.0);
        self.step(4);
    }

    /// Trigger pulls, the magazine release, the slide stop, in any order.
    fn gun_play(&mut self) {
        let side = self.holding_side().unwrap_or(SIDES[self.rng.below(2)]);
        let what = self.hand(side).holding.map(|e| self.name(e));
        let n = 1 + self.rng.below(6);
        self.note(format!("gun play x{n} on {side:?} with {what:?}"));
        for _ in 0..n {
            match self.rng.below(4) {
                0 | 1 => {
                    self.vr.trigger(side, 1.0);
                    let n = 1 + self.rng.below(30);
                    self.step(n);
                    self.vr.trigger(side, 0.0);
                }
                2 => {
                    self.vr.primary(side, true);
                    self.step(2);
                    self.vr.primary(side, false);
                }
                _ => {
                    self.vr.secondary(side, true);
                    self.step(2);
                    self.vr.secondary(side, false);
                }
            }
            let n = 1 + self.rng.below(8);
            self.step(n);
        }
        if self.rng.chance(0.3) {
            // Pull the magazine with the other hand while still firing.
            self.vr.trigger(side, 1.0);
            self.pull_mag(side.other());
            self.vr.trigger(side, 0.0);
        }
    }

    fn well_of(&mut self, gun: Entity) -> Option<(Entity, Vec3, Option<Entity>)> {
        let w = self.vr.app.world_mut();
        w.query::<(Entity, &Socket, &SocketOwner, &GlobalTransform)>()
            .iter(w)
            .find(|(_, s, o, _)| o.0 == gun && matches!(s.kind, SocketKind::MagWell(_)))
            .map(|(e, s, _, g)| (e, g.translation(), s.occupant))
    }

    fn pull_mag(&mut self, side: Side) {
        let gun = self.hand(side.other()).holding;
        let Some((_, at, Some(mag))) = gun.and_then(|g| self.well_of(g)) else {
            return;
        };
        let name = self.name(mag);
        self.note(format!("pull {name} with {side:?}"));
        self.vr.grip(side, 0.0);
        self.reach_to(side, at, 3);
        self.vr.grip(side, 1.0);
        self.step(3);
        let down = self.hand_world(side) - Vec3::Y * 0.2;
        self.reach_to(side, down, 4);
    }

    /// A held magazine up into the other hand's gun.
    fn mag_to_well(&mut self) {
        for side in SIDES {
            let gun = self.hand(side).holding;
            let mag = self.hand(side.other()).holding;
            let (Some(gun), Some(mag)) = (gun, mag) else {
                continue;
            };
            if self
                .vr
                .app
                .world()
                .get::<crate::weapons::Mag>(mag)
                .is_none()
            {
                continue;
            }
            let Some((_, well, _)) = self.well_of(gun) else {
                continue;
            };
            let (g, m) = (self.name(gun), self.name(mag));
            self.note(format!("{m} into {g}"));
            let jitter = self.rng.unit() * 0.03;
            self.reach_to(side.other(), well - Vec3::Y * 0.12 + jitter, 3);
            self.reach_to(side.other(), well + jitter, 4);
            if self.rng.chance(0.7) {
                self.vr.grip(side.other(), 0.0);
            }
            self.step(5);
            return;
        }
        // Nothing to load: put a gun and a magazine in hand next time.
        self.grab();
    }

    fn posture(&mut self) {
        let h = match self.rng.below(4) {
            0 => 0.9,  // sitting
            1 => 1.15, // kneeling
            _ => self.rng.range(1.4, 1.85),
        };
        let crouch = self.rng.chance(0.3);
        self.note(format!("head at {h:.2}, crouch {crouch}"));
        {
            let mut i = self.input();
            i.head.translation.y = h;
            i.crouch = crouch;
        }
        self.step(20);
        self.input().crouch = false;
    }

    fn comfort(&mut self) {
        let k = self.rng.below(8);
        let mut c = self.vr.app.world_mut().resource_mut::<Comfort>();
        let what = match k {
            0 => {
                c.smooth_turn = !c.smooth_turn;
                "smooth_turn"
            }
            1 => {
                c.seated = !c.seated;
                "seated"
            }
            2 => {
                c.left_handed = !c.left_handed;
                "left_handed"
            }
            3 => {
                c.one_handed = !c.one_handed;
                "one_handed"
            }
            4 => {
                c.climb_teleport = !c.climb_teleport;
                "climb_teleport"
            }
            5 => {
                c.finger_grab = !c.finger_grab;
                "finger_grab"
            }
            6 => {
                c.vignette = 1.0 - c.vignette;
                "vignette"
            }
            _ => {
                c.captions = !c.captions;
                "captions"
            }
        };
        self.note(format!("comfort: flip {what}"));
        self.step(3);
    }

    /// A few things from the catalog, in the air in front.
    fn spawn_things(&mut self) {
        let catalog = self.vr.app.world().resource::<Catalog>().clone();
        let ids: Vec<String> = catalog
            .0
            .iter()
            .filter(|(_, d)| d.category != Category::Artifact)
            .map(|(_, d)| d.id.clone())
            .collect();
        let n = 1 + self.rng.below(4);
        let head = self.head_world();
        let fwd = self.yaw() * Vec3::NEG_Z;
        let mut names = Vec::new();
        for i in 0..n {
            let id = ids[self.rng.below(ids.len())].clone();
            let at = head + fwd * 0.45 - Vec3::Y * 0.35
                + Vec3::new(
                    self.rng.range(-0.2, 0.2),
                    0.15 * i as f32,
                    self.rng.range(-0.1, 0.1),
                );
            self.vr.spawn(&id, Transform::from_translation(at));
            self.frame += 2;
            names.push(id);
        }
        self.note(format!("spawn {names:?}"));
    }

    fn poke_button(&mut self) {
        use crate::interact::poke::{FINGERTIP, PokeButton};
        let head = self.head_world();
        let w = self.vr.app.world_mut();
        let buttons: Vec<Vec3> = w
            .query_filtered::<&GlobalTransform, With<PokeButton>>()
            .iter(w)
            .map(|g| g.translation())
            .collect();
        let near: Vec<Vec3> = buttons
            .iter()
            .copied()
            .filter(|b| b.distance(head) < 1.2)
            .collect();
        let Some(&b) = near.get(self.rng.below(near.len().max(1))) else {
            if !buttons.is_empty() && self.rng.chance(0.5) {
                // Go to one.
                let b = buttons[self.rng.below(buttons.len())];
                let back = self.yaw() * Vec3::Z * 0.5;
                self.note(format!("walk up to a button at {b:.1}"));
                let zone = self.vr.app.world().resource::<Zone>().clone();
                let p = b + back;
                let y = zone.ground(p.x, p.z).max(b.y - 1.4);
                self.vr.stand(Vec3::new(p.x, y, p.z), 0.0);
                self.frame += 2;
            }
            return;
        };
        let side = SIDES[self.rng.below(2)];
        self.note(format!("poke button at {b:.2} with {side:?}"));
        let rot = self.yaw();
        let tip = rot * FINGERTIP;
        let dir = self.rng.unit() * 0.05;
        self.vr.hand(side, b - tip + dir, rot, 0.0, 0.0);
        self.step(3);
        self.vr.hand(side, b - tip - dir * 0.5, rot, 0.0, 0.0);
        self.step(6);
        self.rest(side, 0.0);
        self.step(3);
    }

    /// The controller's tracking jumps (lost, found again metres off) for
    /// a frame or two, with or without something in the hand.
    fn glitch(&mut self) {
        let side = SIDES[self.rng.below(2)];
        let at = self.hand_world(side);
        let jump = self.rng.unit() * self.rng.range(1.0, 60.0);
        let frames = 1 + self.rng.below(3);
        let what = self.hand(side).holding.map(|e| self.name(e));
        self.note(format!(
            "tracking glitch: {side:?} hand ({what:?}) jumps {jump:.1} for {frames}"
        ));
        self.vr.move_hand(side, at + jump);
        self.step(frames);
        self.vr.move_hand(side, at);
        if self.rng.chance(0.5) {
            self.vr.grip(side, 0.0);
            self.vr.trigger(side, 0.0);
        }
        self.step(30);
    }

    /// Up a ladder, hand over hand (or a grab at a ledge, wherever).
    fn climb(&mut self) {
        let feet = self.vr.feet();
        let w = self.vr.app.world_mut();
        let ladder = w
            .query::<&Ladder>()
            .iter(w)
            .filter(|l| l.bottom.distance(feet) < 3.0)
            .copied()
            .next();
        let Some(l) = ladder else {
            return self.wave();
        };
        self.note(format!("climb ladder at {:.1}", l.bottom));
        let up = (l.top - l.bottom).normalize_or(Vec3::Y);
        let mut on = feet.y + 1.1;
        for k in 0..(4 + self.rng.below(10)) {
            let side = SIDES[k % 2];
            let p = Vec3::new(l.bottom.x, on, l.bottom.z) + l.out * 0.05;
            self.vr.grip(side, 0.0);
            self.reach_to(side, p + up * 0.3, 3);
            self.vr.grip(side, 1.0);
            self.step(2);
            self.vr.grip(side.other(), 0.0);
            // Pull: the hand comes down toward the body.
            let local = self.input().hand(side).pose.translation;
            for j in 1..=6 {
                self.vr
                    .hand_local(side, local - Vec3::Y * 0.05 * j as f32, 1.0);
                self.step(1);
            }
            on += 0.3;
        }
        if self.rng.chance(0.5) {
            // Let go of everything, halfway up.
            self.vr.grip(Side::Left, 0.0);
            self.vr.grip(Side::Right, 0.0);
        }
        self.step(30);
    }

    // ---- invariants ----

    /// What must always hold.
    fn check(&mut self) {
        let zone = self.vr.app.world().resource::<Zone>().clone();
        let half = zone.map.size * 0.5 + 10.0;
        let w = self.vr.app.world_mut();
        let mut bad: Vec<(Option<Entity>, &'static str, String)> = Vec::new();

        // Nothing is NaN or infinite.
        let mut q = w.query::<(Entity, &Transform)>();
        for (e, t) in q.iter(w) {
            if !(t.translation.is_finite() && t.rotation.is_finite() && t.scale.is_finite()) {
                bad.push((Some(e), "nan", format!("non-finite Transform {t:?}")));
            }
        }
        let mut q = w.query::<(Entity, &LinearVelocity, &AngularVelocity)>();
        for (e, v, a) in q.iter(w) {
            if !(v.0.is_finite() && a.0.is_finite()) {
                bad.push((Some(e), "nan", format!("non-finite velocity {v:?} {a:?}")));
            }
        }

        // Loose items stay in the world, on top of it.
        let mut q = w.query_filtered::<(Entity, &GlobalTransform, Option<&LinearVelocity>), (
            With<Item>,
            Without<Held>,
            Without<InSocket>,
            Without<InBackpack>,
            Without<ChildOf>,
        )>();
        let mut good = Vec::new();
        for (e, g, v) in q.iter(w) {
            let p = g.translation();
            let v = v.map(|v| v.0).unwrap_or_default();
            if !p.is_finite() {
                continue;
            }
            if p.x.abs() > half || p.z.abs() > half {
                bad.push((Some(e), "out", format!("off the map at {p:.1}, v {v:.1}")));
                continue;
            }
            if v.length() > FAST {
                bad.push((
                    Some(e),
                    "fast",
                    format!("flying at {:.0} m/s at {p:.1}", v.length()),
                ));
            }
            let floor = crate::world::underground::floor_under(&zone, p)
                .unwrap_or_else(|| zone.ground(p.x, p.z));
            if p.y < floor - UNDER {
                bad.push((
                    Some(e),
                    "under",
                    format!("under the ground: at {p:.2}, ground {floor:.2}, v {v:.1}"),
                ));
            } else {
                good.push((e, p, v));
            }
        }
        for (e, p, v) in good {
            self.last_good.insert(e, (p, v));
        }

        // An item is in one place at a time.
        let w = self.vr.app.world_mut();
        let mut hands = w.query::<(Entity, &RigHand, &Hand)>();
        let hands: Vec<(Entity, Side, Option<Entity>)> = hands
            .iter(w)
            .map(|(e, r, h)| (e, r.side, h.holding))
            .collect();
        for (he, side, holding) in &hands {
            if let Some(i) = holding {
                match w.get::<Held>(*i) {
                    Some(h) if h.hand == *he => {}
                    other => bad.push((
                        Some(*i),
                        "hold",
                        format!("{side:?} hand holds it but its Held is {other:?}"),
                    )),
                }
                if w.get::<InSocket>(*i).is_some() || w.get::<InBackpack>(*i).is_some() {
                    bad.push((Some(*i), "two", "held and socketed/stowed".into()));
                }
            }
        }
        let mut q = w.query::<(Entity, &Held)>();
        for (e, h) in q.iter(w) {
            if !hands
                .iter()
                .any(|(he, _, hold)| *he == h.hand && *hold == Some(e))
            {
                bad.push((
                    Some(e),
                    "held",
                    format!("Held by {:?} but no hand holds it", h.side),
                ));
            }
        }
        let mut q = w.query::<(Entity, &Socket)>();
        let socks: Vec<(Entity, Option<Entity>)> =
            q.iter(w).map(|(e, s)| (e, s.occupant)).collect();
        for (s, occ) in &socks {
            if let Some(o) = occ {
                match w.get_entity(*o) {
                    Err(_) => {
                        bad.push((None, "ghost", format!("socket {s} holds a despawned {o}")))
                    }
                    Ok(r) => match r.get::<InSocket>() {
                        Some(i) if i.socket == *s => {}
                        other => bad.push((
                            Some(*o),
                            "sock",
                            format!("socket {s} has it but its InSocket is {other:?}"),
                        )),
                    },
                }
            }
        }
        let mut q = w.query::<(Entity, &InSocket)>();
        for (e, i) in q.iter(w) {
            if !socks.iter().any(|(s, o)| *s == i.socket && *o == Some(e)) {
                bad.push((
                    Some(e),
                    "sock2",
                    format!("InSocket {} but the socket doesn't hold it", i.socket),
                ));
            }
        }
        let mut q = w.query::<(Entity, &Backpack)>();
        let bags: Vec<(Entity, Vec<Entity>)> =
            q.iter(w).map(|(e, b)| (e, b.contents.clone())).collect();
        for (b, contents) in &bags {
            for c in contents {
                match w.get_entity(*c) {
                    Err(_) => bad.push((None, "ghost", format!("pack {b} lists a despawned {c}"))),
                    Ok(r) => match r.get::<InBackpack>() {
                        Some(i) if i.bag == *b => {}
                        other => bad.push((
                            Some(*c),
                            "bag",
                            format!("pack {b} lists it but its InBackpack is {other:?}"),
                        )),
                    },
                }
            }
        }
        let mut q = w.query::<(Entity, &InBackpack)>();
        for (e, i) in q.iter(w) {
            if !bags.iter().any(|(b, c)| *b == i.bag && c.contains(&e)) {
                bad.push((
                    Some(e),
                    "bag2",
                    "InBackpack but the pack doesn't list it".into(),
                ));
            }
        }

        // Guns, magazines, tools and artifacts are never used up.
        let gone: Vec<(Entity, String, Category)> = self
            .items
            .iter()
            .filter(|(e, _)| w.get_entity(**e).is_err())
            .map(|(e, (id, c))| (*e, id.clone(), *c))
            .collect();
        for (e, id, c) in gone {
            self.items.remove(&e);
            if matches!(
                c,
                Category::Firearm | Category::Magazine | Category::Tool | Category::Artifact
            ) {
                let last = self.last_good.get(&e).copied();
                bad.push((
                    None,
                    "vanished",
                    format!(
                        "{id}#{} ({c:?}) despawned; last loose at {last:?}",
                        e.index()
                    ),
                ));
            }
        }

        // The player stays in the world.
        let feet = self.vr.feet();
        let floor = crate::world::underground::floor_under(&zone, feet)
            .unwrap_or_else(|| zone.ground(feet.x, feet.z));
        if !feet.is_finite() || feet.y < floor - 1.0 {
            bad.push((
                None,
                "player",
                format!("player under the ground at {feet:.2} (ground {floor:.2})"),
            ));
        }

        for (e, kind, what) in bad {
            let key = (e.unwrap_or(Entity::PLACEHOLDER), kind);
            if kind == "player" || kind == "ghost" {
                // Report once per run.
                if !self.reported.insert((Entity::PLACEHOLDER, kind)) {
                    continue;
                }
            } else if !self.reported.insert(key) {
                continue;
            }
            let name = e.map(|e| self.name(e)).unwrap_or_default();
            let extra = e
                .and_then(|e| self.last_good.get(&e))
                .map(|(p, v)| format!(" (last good at {p:.2}, v {v:.1})"))
                .unwrap_or_default();
            self.find(format!("{kind}: {name} {what}{extra}"));
        }
    }
}

/// Runs a monkey for `secs` of game time; a panic is a finding too.
pub(crate) fn run(map: &str, seed: u64, robots: bool, secs: f32) -> Vec<Finding> {
    let frames = (secs * 90.0) as u64;
    let trail = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let t2 = trail.clone();
    let m2 = map.to_string();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let mut m = Monkey::new(&m2, seed, robots);
        while m.frame < frames {
            m.act();
            *t2.lock().unwrap() = m.trail.iter().cloned().collect();
        }
        m.findings
    }));
    match result {
        Ok(f) => f,
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            vec![Finding {
                what: format!("panic: {msg}"),
                map: map.into(),
                seed,
                frame: 0,
                trail: trail.lock().unwrap().clone(),
            }]
        }
    }
}

/// A few seeds, half a minute each, in the yard and a valley.
#[test]
fn chaos_monkey_short() {
    let mut all = Vec::new();
    for (map, seed, robots) in [
        ("training_yard", 1, false),
        ("training_yard", 2, false),
        ("marrow_creek", 3, true),
    ] {
        all.extend(run(map, seed, robots, 30.0));
    }
    let text: String = all.iter().map(|f| f.to_string()).collect();
    assert!(all.is_empty(), "{} findings:\n{text}", all.len());
}

/// The long run: `CHAOS_SEEDS` (`1-20` or `3,7`), `CHAOS_SECS` each,
/// `CHAOS_MAP` (a map or `all`), `CHAOS_ROBOTS=1`.
#[test]
#[ignore]
fn chaos_monkey_long() {
    let env = |k: &str| std::env::var(k).ok();
    let seeds: Vec<u64> = match env("CHAOS_SEEDS") {
        Some(s) if s.contains('-') => {
            let (a, b) = s.split_once('-').unwrap();
            (a.parse().unwrap()..=b.parse().unwrap()).collect()
        }
        Some(s) => s.split(',').map(|x| x.trim().parse().unwrap()).collect(),
        None => (1..=4).collect(),
    };
    let secs: f32 = env("CHAOS_SECS")
        .and_then(|s| s.parse().ok())
        .unwrap_or(120.0);
    let maps: Vec<String> = match env("CHAOS_MAP").as_deref() {
        None | Some("all") => [
            "training_yard",
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "sallow_reservoir",
        ]
        .map(String::from)
        .to_vec(),
        Some(m) => m.split(',').map(String::from).collect(),
    };
    let robots = env("CHAOS_ROBOTS").is_some_and(|v| v == "1");
    let mut all = Vec::new();
    for map in &maps {
        for &seed in &seeds {
            let t = std::time::Instant::now();
            let f = run(map, seed, robots, secs);
            eprintln!(
                "chaos {map} seed {seed}: {} findings in {:.0} s",
                f.len(),
                t.elapsed().as_secs_f32()
            );
            for x in &f {
                eprint!("{x}");
            }
            all.extend(f);
        }
    }
    assert!(all.is_empty(), "{} findings", all.len());
}

/// Scratch: where the player ends up standing at a spot.
#[test]
#[ignore]
fn chaos_probe_stand() {
    let env = |k: &str| std::env::var(k).ok();
    let map = env("PROBE_MAP").unwrap_or("marrow_creek".into());
    let xz: Vec<f32> = env("PROBE_XZ")
        .unwrap_or("-330.5,-61.5".into())
        .split(',')
        .map(|v| v.parse().unwrap())
        .collect();
    let mut vr = Vr::with(
        crate::LaunchOptions {
            robots: false,
            map,
            ..Default::default()
        },
        None,
    );
    let zone = vr.app.world().resource::<Zone>().clone();
    let g = zone.ground(xz[0], xz[1]);
    eprintln!(
        "ground {g:.2} water {:?}",
        zone.hm.water_level([xz[0], xz[1]])
    );
    vr.stand(Vec3::new(xz[0], g + 0.05, xz[1]), 0.0);
    for i in 0..60 {
        vr.step(2);
        let f = vr.feet();
        eprintln!("{i}: feet {f:.2}");
    }
}

/// Every loot spot on every map sits on or over the ground (or an
/// underground site's floor): the monkey found curios at Cinder Works'
/// slag heaps spawned under the hill and falling through it.
#[test]
fn every_loot_spot_is_over_the_ground() {
    use crate::world::buildings::{loot_spot_at, placement_transform, prefab};
    let mut bad = Vec::new();
    let mut n = 0;
    for map in [
        "training_yard",
        "marrow_creek",
        "kettle_pass",
        "cinder_works",
        "sallow_reservoir",
    ] {
        let zone = Zone::load(map);
        for p in &zone.map.placements {
            let Some(geo) = prefab(&p.prefab) else {
                continue;
            };
            let t = placement_transform(&zone, p);
            for s in &geo.loot {
                n += 1;
                let at = loot_spot_at(&zone, &t, s.at);
                if crate::world::underground::floor_under(&zone, at).is_some() {
                    continue;
                }
                let ground = zone.ground(at.x, at.z);
                if at.y < ground - 0.05 {
                    bad.push(format!(
                        "{map} {} at {:?}: spot {at:.2} is {:.2} m under the ground",
                        p.prefab,
                        p.at,
                        ground - at.y
                    ));
                }
            }
        }
    }
    assert!(n > 100, "only {n} loot spots");
    assert!(
        bad.is_empty(),
        "{} spots under the ground:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Scratch: what the underground rasters say about a point.
#[test]
#[ignore]
fn chaos_probe_floor() {
    let env = |k: &str| std::env::var(k).ok();
    let zone = Zone::load(&env("PROBE_MAP").unwrap_or("marrow_creek".into()));
    let p: Vec<f32> = env("PROBE_P")
        .unwrap_or("-245.0,8.5,-152.5".into())
        .split(',')
        .map(|v| v.parse().unwrap())
        .collect();
    let p = Vec3::new(p[0], p[1], p[2]);
    for site in extraction_core::underground::placed(&zone.map) {
        let r = extraction_core::underground::raster(&site.id).unwrap();
        let footing = zone.hm.footing(&site.id, site.at, site.rot);
        let local = site.to_site([p.x, p.z]);
        eprintln!(
            "{} at {:?}: footing {footing:.2} local {local:?} floor {:?}",
            site.id,
            site.at,
            r.floor_at(local)
        );
    }
    eprintln!(
        "ground {:.2} floor_under {:?}",
        zone.ground(p.x, p.z),
        crate::world::underground::floor_under(&zone, p)
    );
}

/// An item despawned while stowed (a grenade going off in the pack or a
/// pouch) leaves the pack's list and the pouch empty: the monkey found a
/// pack still listing an EMP grenade that had gone off in it.
#[test]
fn a_despawned_item_leaves_its_pack_and_pouch() {
    let mut vr = Vr::new("training_yard");
    let feet = vr.feet();
    let a = vr.spawn("tea_tin", Transform::from_translation(feet + Vec3::Y));
    let b = vr.spawn("tea_tin", Transform::from_translation(feet + Vec3::Y * 1.3));
    let player = vr.player;
    let w = vr.app.world_mut();
    let bag = w
        .query::<(Entity, &Backpack)>()
        .iter(w)
        .find(|(_, b)| b.owner == player && b.carried)
        .map(|(e, _)| e)
        .expect("a pack");
    let pouch = w
        .query::<(Entity, &Socket, &SocketOwner)>()
        .iter(w)
        .find(|(_, s, o)| {
            o.0 == player && s.occupant.is_none() && matches!(s.kind, SocketKind::Body(_))
        })
        .map(|(e, _, _)| e)
        .expect("a pouch");
    {
        let mut state: bevy::ecs::system::SystemState<(
            Commands,
            Query<&mut Backpack>,
            Query<&mut Socket>,
        )> = bevy::ecs::system::SystemState::new(w);
        let (mut commands, mut bags, mut sockets) = state.get_mut(w).unwrap();
        let mut bg = bags.get_mut(bag).unwrap();
        crate::interact::body::stow_in_backpack(&mut commands, a, bag, &mut bg, Vec3::ZERO);
        let mut s = sockets.get_mut(pouch).unwrap();
        crate::interact::seat_in_socket(&mut commands, b, pouch, &mut s);
        state.apply(w);
    }
    vr.step(3);
    assert!(
        vr.app
            .world()
            .get::<Backpack>(bag)
            .unwrap()
            .contents
            .contains(&a)
    );
    assert_eq!(
        vr.app.world().get::<Socket>(pouch).unwrap().occupant,
        Some(b)
    );
    vr.app.world_mut().despawn(a);
    vr.app.world_mut().despawn(b);
    vr.step(2);
    assert!(
        !vr.app
            .world()
            .get::<Backpack>(bag)
            .unwrap()
            .contents
            .contains(&a),
        "the pack still lists it"
    );
    assert_eq!(
        vr.app.world().get::<Socket>(pouch).unwrap().occupant,
        None,
        "the pouch still holds it"
    );
}
