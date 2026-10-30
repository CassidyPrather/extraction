//! The one larger animal a valley: a few roe deer in Marrow Creek and
//! Kettle Pass, a pack of stray dogs in Cinder Works.
//!
//! **Deer** graze along a forest edge, heads down, looking up now and
//! then. Something coming ([`alert_radius`]: you moving, a machine) and
//! they stop, heads up, turned to it; come on ([`flee_radius`]) or shoot
//! near them and they bolt, bounding away from it into the trees, a white
//! rump flashing, and settle somewhere else. The roe's bark as they go is
//! [`Startled`]. A deer that's alert or running tells you the way the
//! trouble is.
//!
//! **Dogs** (Cinder Works) keep their distance from people: they trot
//! round you at [`DOG_KEEP`] metres and bark at you if you come closer —
//! and barking is a noise the machines hear ([`crate::robots::Noise`], from
//! you, at the dogs; at most once every [`DOG_BARK_EVERY`] s). The robot
//! hounds are what they truly fear: a hound near and the pack runs,
//! barking (and hounds hunt by ear, so it comes after them), and one a
//! hound catches is killed ([`Startle::Killed`]).
//!
//! Movement is on the walkers' nav grid (`robots::walker::Nav`), never
//! into a building, a rock, deep water or a tree trunk: each step is
//! checked against it, and a run picks, every half second, the most open
//! way away from the danger. Deterministic for a seed. What they look like
//! (a procedural gait with two-bone legs) is `herd_look`.

use bevy::prelude::*;
use extraction_core::nav::NavGrid;
use extraction_core::noise::Rng;

use super::{Threat, ThreatKind, Threats};
use crate::world::Zone;

/// Which animal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Deer,
    Dog,
    /// Rime Fell's: a deer's ways on bare high ground, quicker over scree.
    Chamois,
}

impl Kind {
    pub fn for_map(map: &str) -> Option<Self> {
        match map {
            "marrow_creek" | "kettle_pass" | "sallow_reservoir" => Some(Kind::Deer),
            "cinder_works" => Some(Kind::Dog),
            "rime_fell" => Some(Kind::Chamois),
            _ => None,
        }
    }

    /// Top running speed, m/s; a trot; a grazing amble.
    pub fn speeds(self) -> (f32, f32, f32) {
        match self {
            Kind::Deer => (10.0, 3.2, 0.5),
            Kind::Chamois => (11.0, 3.6, 0.5),
            Kind::Dog => (9.0, 3.0, 0.9),
        }
    }
}

// --- What they make of you -----------------------------------------------

/// A deer looks up at you standing in the open this far off (m)...
pub const ALERT_STILL: f32 = 20.0;
/// ...and this much further per m/s you're moving, up to [`ALERT_MAX`].
pub const ALERT_PER_SPEED: f32 = 16.0;
pub const ALERT_MAX: f32 = 80.0;
/// Crouched, this much closer.
pub const ALERT_CROUCHED: f32 = 0.55;
/// Machines: a walker or hound, a drone, the airship.
pub const ALERT_WALKER: f32 = 90.0;
pub const ALERT_DRONE: f32 = 80.0;
pub const ALERT_DIRIGIBLE: f32 = 140.0;
/// Something thrown that lands this near puts them to flight (m).
pub const THROWN_NEAR: f32 = 10.0;
/// A herd bolting is heard by the machines this far (m).
pub const BOLT_HEARD: f32 = 45.0;
/// They run when it comes within this share of the alert radius...
pub const FLEE_SHARE: f32 = 0.6;
/// ...or when it's still coming after this long alert (s).
pub const ALERT_PATIENCE: f32 = 5.0;
/// A loud noise within this share of its reach and they're gone.
pub const FLEE_NOISE: f32 = 0.7;
/// They run until the danger's this far behind (m), or this long (s).
pub const SAFE: f32 = 110.0;
pub const RUN_FOR: f32 = 25.0;

/// Dogs bark at someone who comes nearer than this...
pub const DOG_BARK: f32 = 34.0;
/// ...and keep about this far from them...
pub const DOG_KEEP: f32 = 28.0;
/// ...heard by machines this far off, at most this often a pack.
pub const DOG_BARK_REACH: f32 = 70.0;
pub const DOG_BARK_EVERY: f32 = 45.0;
/// A hound this near and the pack runs; this near and it has one.
pub const DOG_HOUND: f32 = 45.0;
pub const DOG_CAUGHT: f32 = 1.4;

/// Grazing, they stay within this of home.
pub const HOME_RANGE: f32 = 30.0;

/// How close a threat may come before a deer looks up.
pub fn alert_radius(t: &Threat) -> f32 {
    match t.kind {
        ThreatKind::Player { crouched } => {
            let r = (ALERT_STILL + ALERT_PER_SPEED * t.speed).min(ALERT_MAX);
            if crouched { r * ALERT_CROUCHED } else { r }
        }
        ThreatKind::Walker => ALERT_WALKER,
        ThreatKind::Drone => ALERT_DRONE,
        ThreatKind::Dirigible => ALERT_DIRIGIBLE,
    }
}

/// How close before it runs.
pub fn flee_radius(t: &Threat) -> f32 {
    alert_radius(t) * FLEE_SHARE
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    /// Head down, eating; `look` up now and then.
    Graze,
    /// Ambling to somewhere else to eat.
    Wander { to: Vec2 },
    /// Head up, watching `at` (since `for_s` seconds).
    Alert { at: Vec3, for_s: f32 },
    /// Running from `from`.
    Flee { from: Vec3, for_s: f32 },
    /// A dog keeping its distance from someone at `from`.
    Keep { from: Vec3 },
    /// Caught by a hound: lies where it fell.
    Dead,
}

#[derive(Clone, Debug)]
pub struct Beast {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Heading (radians about Y, from +X, as atan2(z, x)).
    pub yaw: f32,
    pub state: State,
    /// Head up (1) or down to the grass (0).
    pub head_up: f32,
    /// Where it's looking, if anywhere in particular.
    pub look: Option<Vec3>,
    timer: f32,
    /// Dead and taken away (its carcass collected): nothing to draw.
    pub gone: bool,
    /// Its place round the leader, running or ambling.
    slot: Vec2,
    /// The way round to where it's going, when it can't go straight: the
    /// waypoints left, and where they lead.
    route: Vec<Vec2>,
    route_to: Option<Vec2>,
    /// Ticks since the route was worked out.
    route_age: u16,
}

/// A beast more than this from its waypoint goes straight there; within
/// it, on to the next (m).
const WAYPOINT: f32 = 1.5;
/// A target moved this far is a new one, routed afresh (m).
const REROUTE: f32 = 4.0;
/// Its next waypoint out of reach from where it's got to (pushed off the
/// line, slid along a corner), it's worked out again from there, at most
/// this often (ticks).
const REROUTE_EVERY: u16 = 45;

impl Beast {
    pub fn alive(&self) -> bool {
        self.state != State::Dead
    }
}

/// Something about a herd for sound and the machines.
#[derive(Message, Clone, Copy, Debug)]
pub struct Startled {
    pub herd: Entity,
    pub at: Vec3,
    pub kind: Kind,
    /// Barked (a roe's alarm, a dog at you), bolted, or was killed.
    pub what: Startle,
    /// The player (or hound) it's about, if any.
    pub by: Option<Entity>,
    /// The machines hear of it (a dog barking at you).
    pub alarm: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Startle {
    Bark,
    Bolt,
    Killed,
}

#[derive(Component, Clone, Debug)]
pub struct Herd {
    pub kind: Kind,
    pub beasts: Vec<Beast>,
    pub home: Vec2,
    /// Food put down that it's going to, or eating (`bait`).
    pub lure: Option<Entity>,
    rng: Rng,
    bark_cool: f32,
}

impl Herd {
    pub fn leader(&self) -> Option<&Beast> {
        self.beasts.iter().find(|b| b.alive())
    }

    /// Something to eat at `to`: home is there now, and those grazing or
    /// ambling far from it walk over (not those running or watching).
    pub fn lure(&mut self, to: Vec2) {
        self.home = to;
        for b in self.beasts.iter_mut().filter(|b| b.alive()) {
            let far = b.pos.xz().distance(to) > super::bait::EAT_NEAR * 2.0;
            if far && matches!(b.state, State::Graze | State::Wander { .. }) {
                b.state = State::Wander {
                    to: to + b.slot * 0.5,
                };
                b.timer = 30.0;
            }
        }
    }
}

/// Where the herds' animals are this tick, for the machines' eyes: you
/// walking among them is one more shape moving (`robots::senses`).
#[derive(Resource, Default)]
pub struct Among(pub Vec<Vec3>);

/// You're among animals here: this many within this (m).
pub const AMONG_NEAR: f32 = 12.0;
pub const AMONG_COUNT: usize = 2;

impl Among {
    pub fn hides(&self, p: Vec3) -> bool {
        self.0
            .iter()
            .filter(|a| a.xz().distance(p.xz()) < AMONG_NEAR)
            .count()
            >= AMONG_COUNT
    }
}

/// The walkers' nav grid as the animals have it: water blocked too (a
/// deer doesn't wade the reservoir), so a way round never goes through it.
pub fn herd_grid(zone: &Zone, nav: &NavGrid) -> NavGrid {
    let mut grid = nav.clone();
    // Any water in the cell (its middle or near a corner) and it's shut:
    // a way the grid calls clear is then clear to `open` too.
    let h = grid.cell * 0.49;
    for j in 0..grid.n {
        for i in 0..grid.n {
            let c = grid.centre(i, j);
            let wet = [[0.0, 0.0], [h, h], [h, -h], [-h, h], [-h, -h]]
                .iter()
                .any(|o| {
                    zone.hm.surface([c[0] + o[0], c[1] + o[1]], false)
                        == extraction_core::terrain::Surface::Water
                });
            if wet {
                grid.cost[j * grid.n + i] = extraction_core::nav::BLOCKED;
            }
        }
    }
    grid
}

/// Is the straight way from `a` to `b` all ground an animal can stand on
/// (every metre)?
fn clear_way(nav: &NavGrid, zone: &Zone, a: Vec2, b: Vec2) -> bool {
    let n = (a.distance(b) as usize).max(1);
    (1..=n).all(|k| open(nav, zone, a.lerp(b, k as f32 / n as f32)))
}

/// Is `p` somewhere an animal can stand?
pub fn open(nav: &NavGrid, zone: &Zone, p: Vec2) -> bool {
    nav.walkable([p.x, p.y])
        && zone.hm.surface([p.x, p.y], false) != extraction_core::terrain::Surface::Water
}

/// A herd's home for a valley: open ground near a forest's edge (for
/// chamois, the highest open ground), away from where the truck drops you,
/// deterministic for the seed. A valley with no forest edge (bare fell)
/// takes open ground anywhere.
pub fn pick_home(zone: &Zone, nav: &NavGrid, seed: u64, kind: Kind) -> Option<Vec2> {
    let edge = kind != Kind::Chamois;
    home_where(zone, nav, seed, kind, edge).or_else(|| home_where(zone, nav, seed, kind, false))
}

fn home_where(zone: &Zone, nav: &NavGrid, seed: u64, kind: Kind, edge_only: bool) -> Option<Vec2> {
    let mut rng = Rng::new(seed ^ 0xDEE2_0BAD);
    let start = zone
        .marker(&extraction_core::map::MarkerKind::Insertion)
        .map(|m| Vec2::new(m.at[0], m.at[1]))
        .unwrap_or(Vec2::ZERO);
    let half = zone.map.size * 0.5 - zone.map.fence_inset - 40.0;
    let mut best: Option<(f32, Vec2)> = None;
    for _ in 0..400 {
        let p = Vec2::new(rng.range(-half, half), rng.range(-half, half));
        if !open(nav, zone, p) || p.distance(start) < 150.0 {
            continue;
        }
        // An edge: some forest, not deep in it; room round it.
        let edge = zone.forest_cover(p.x, p.y);
        if edge_only && !(0.05..0.6).contains(&edge) {
            continue;
        }
        // Where the herd is put down (new_herd's slots) clear, metre by
        // metre: no beast starts inside a rock.
        let put_down =
            (-4..=9).all(|x| (-4..=4).all(|z| open(nav, zone, p + Vec2::new(x as f32, z as f32))));
        if !put_down {
            continue;
        }
        let room = (0..8)
            .filter(|k| {
                let a = *k as f32 / 8.0 * std::f32::consts::TAU;
                open(nav, zone, p + Vec2::new(a.cos(), a.sin()) * 15.0)
            })
            .count();
        // Chamois: up high (a metre is worth a tenth of a point).
        let high = if kind == Kind::Chamois {
            zone.ground(p.x, p.y) * 0.1
        } else {
            0.0
        };
        let score = room as f32 + high + rng.f32();
        if room >= 6 && best.is_none_or(|(s, _)| score > s) {
            best = Some((score, p));
        }
    }
    best.map(|(_, p)| p)
}

pub fn new_herd(kind: Kind, home: Vec2, zone: &Zone, rng: &mut Rng) -> Herd {
    let n = match kind {
        Kind::Deer | Kind::Chamois => 2 + rng.below(3),
        Kind::Dog => 3 + rng.below(2),
    };
    let beasts = (0..n)
        .map(|i| {
            let slot = Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * 4.0
                + Vec2::new(i as f32 * 1.5, 0.0);
            let p = home + slot;
            Beast {
                pos: zone.on_ground(p.x, p.y),
                vel: Vec3::ZERO,
                yaw: rng.range(0.0, std::f32::consts::TAU),
                state: State::Graze,
                head_up: 0.0,
                look: None,
                timer: rng.range(2.0, 10.0),
                gone: false,
                slot,
                route: Vec::new(),
                route_to: None,
                route_age: 0,
            }
        })
        .collect();
    Herd {
        kind,
        beasts,
        home,
        lure: None,
        rng: Rng::new(rng.next_u64()),
        bark_cool: 0.0,
    }
}

/// What a herd's tick needs.
pub struct Ctx<'a> {
    pub zone: &'a Zone,
    pub nav: &'a NavGrid,
    pub threats: &'a [Threat],
    pub loud: &'a [(Vec3, f32)],
    /// Robot hounds: where (only dogs care).
    pub hounds: &'a [(Entity, Vec3)],
}

impl Herd {
    /// One tick; what startled it, if anything did.
    pub fn tick(&mut self, ctx: &Ctx, dt: f32) -> Vec<(Startle, Vec3, Option<Entity>, bool)> {
        let mut out = Vec::new();
        self.bark_cool = (self.bark_cool - dt).max(0.0);
        let kind = self.kind;
        let (run, trot, amble) = kind.speeds();
        // The herd decides together, from where its leader is.
        let Some(lead) = self.leader().map(|b| b.pos) else {
            return out;
        };
        // The worst thing about.
        let mut danger: Option<(Vec3, Option<Entity>, bool)> = None; // at, who, flee now
        let mut alert: Option<(Vec3, Option<Entity>)> = None;
        let mut keep: Option<(Vec3, Entity)> = None;
        for t in ctx.threats {
            let d = t.at.distance(lead);
            let is_player = matches!(t.kind, ThreatKind::Player { .. });
            if kind == Kind::Dog && is_player {
                if d < DOG_KEEP + 12.0 && keep.is_none_or(|(k, _)| k.distance(lead) > d) {
                    keep = Some((t.at, t.entity));
                }
                continue;
            }
            if kind == Kind::Dog && t.kind == ThreatKind::Walker {
                // Walkers (hounds aside, below) only bother dogs up close.
                if d < 15.0 {
                    danger = Some((t.at, Some(t.entity), true));
                }
                continue;
            }
            if d < flee_radius(t) {
                danger = Some((t.at, Some(t.entity), true));
            } else if d < alert_radius(t) && alert.is_none() {
                alert = Some((t.at, Some(t.entity)));
            }
        }
        for &(p, _) in ctx
            .loud
            .iter()
            .filter(|(p, r)| p.distance(lead) < *r * FLEE_NOISE)
        {
            danger = Some((p, None, true));
        }
        let mut hound = false;
        if kind == Kind::Dog {
            for &(e, p) in ctx.hounds {
                if p.distance(lead) < DOG_HOUND {
                    danger = Some((p, Some(e), true));
                    hound = true;
                }
            }
            // Caught.
            for b in self.beasts.iter_mut().filter(|b| b.alive()) {
                if let Some(&(e, _)) = ctx
                    .hounds
                    .iter()
                    .find(|(_, p)| p.distance(b.pos) < DOG_CAUGHT)
                {
                    b.state = State::Dead;
                    b.vel = Vec3::ZERO;
                    out.push((Startle::Killed, b.pos, Some(e), false));
                }
            }
        }
        // The herd's state, from its leader's.
        let lead_state = self.leader().map(|b| b.state).unwrap_or(State::Dead);
        let next = match (danger, lead_state) {
            (Some((from, who, _)), s) => {
                let fresh = !matches!(s, State::Flee { .. });
                if fresh {
                    out.push((Startle::Bolt, lead, who, false));
                    if kind != Kind::Dog {
                        out.push((Startle::Bark, lead, who, false));
                    }
                    // Dogs bark at a hound as they go: and hounds hunt by
                    // ear (a noise from nobody: it comes to the dogs).
                    if hound && self.bark_cool <= 0.0 {
                        self.bark_cool = DOG_BARK_EVERY;
                        out.push((Startle::Bark, lead, None, true));
                    }
                }
                let for_s = match s {
                    State::Flee { for_s, .. } => for_s,
                    _ => 0.0,
                };
                Some(State::Flee { from, for_s })
            }
            (None, State::Flee { from, for_s }) => {
                let for_s = for_s + dt;
                if for_s > RUN_FOR || from.distance(lead) > SAFE {
                    // Safe: this is home now.
                    self.home = lead.xz();
                    Some(State::Graze)
                } else {
                    Some(State::Flee { from, for_s })
                }
            }
            (None, _) if keep.is_some() => {
                let (from, who) = keep.unwrap();
                if from.distance(lead) < DOG_BARK && self.bark_cool <= 0.0 {
                    self.bark_cool = DOG_BARK_EVERY;
                    out.push((Startle::Bark, lead, Some(who), true));
                }
                Some(State::Keep { from })
            }
            (None, State::Keep { .. }) => {
                self.home = lead.xz();
                Some(State::Graze)
            }
            (None, State::Alert { at, for_s }) => match alert {
                Some((now, who)) => {
                    let coming = now.distance(lead) < at.distance(lead) - 0.5;
                    let for_s = for_s + dt;
                    if coming && for_s > ALERT_PATIENCE {
                        out.push((Startle::Bolt, lead, who, false));
                        if kind != Kind::Dog {
                            out.push((Startle::Bark, lead, None, false));
                        }
                        Some(State::Flee {
                            from: now,
                            for_s: 0.0,
                        })
                    } else {
                        // Keep the older mark while it holds still.
                        Some(State::Alert {
                            at: if coming { at } else { now },
                            for_s,
                        })
                    }
                }
                None => Some(State::Graze),
            },
            (None, State::Graze | State::Wander { .. }) if alert.is_some() => {
                let (at, _) = alert.unwrap();
                Some(State::Alert { at, for_s: 0.0 })
            }
            _ => None,
        };
        let home = self.home;
        let rng = &mut self.rng;
        // Each beast: follow the herd's state, or go on grazing.
        let lead_pos = lead;
        for b in self.beasts.iter_mut().filter(|b| b.alive()) {
            if let Some(s) = next {
                let same = std::mem::discriminant(&s) == std::mem::discriminant(&b.state);
                if !same
                    || matches!(
                        s,
                        State::Flee { .. } | State::Alert { .. } | State::Keep { .. }
                    )
                {
                    b.state = s;
                }
            }
            b.timer -= dt;
            let (want, speed, head, look): (Option<Vec2>, f32, f32, Option<Vec3>) = match b.state {
                State::Graze => {
                    if b.timer <= 0.0 {
                        // Somewhere else to eat, near home.
                        let to = home
                            + Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * HOME_RANGE;
                        b.state = State::Wander { to };
                        b.timer = rng.range(4.0, 10.0);
                    }
                    // Look up now and then.
                    let up = (b.timer * 0.7).sin() > 0.75;
                    (None, 0.0, if up { 1.0 } else { 0.0 }, None)
                }
                State::Wander { to } => {
                    if b.timer <= 0.0 || b.pos.xz().distance(to) < 1.0 {
                        b.state = State::Graze;
                        b.timer = rng.range(5.0, 15.0);
                    }
                    (Some(to), amble, 0.4, None)
                }
                State::Alert { at, .. } => (None, 0.0, 1.0, Some(at)),
                State::Flee { from, for_s } => {
                    // Flat out to begin with, then a trot.
                    let speed = if for_s < 8.0 { run } else { trot };
                    let away = escape(ctx.nav, ctx.zone, b.pos.xz(), from.xz(), b.vel.xz(), rng);
                    (Some(b.pos.xz() + away * 10.0), speed, 1.0, None)
                }
                State::Keep { from } => {
                    // Round you at DOG_KEEP: away if too near, idle if not.
                    let off = b.pos.xz() - from.xz();
                    let d = off.length().max(0.1);
                    if d < DOG_KEEP {
                        let to = from.xz() + off / d * (DOG_KEEP + 4.0) + b.slot;
                        (Some(to), trot, 1.0, Some(from))
                    } else {
                        (None, 0.0, 1.0, Some(from))
                    }
                }
                State::Dead => (None, 0.0, 0.0, None),
            };
            // Keep with the leader (not when standing about).
            let want = want.map(|w| {
                if b.pos.distance(lead_pos) > 12.0 {
                    lead_pos.xz() + b.slot
                } else {
                    w
                }
            });
            b.head_up += (head - b.head_up) * (dt * 3.0).min(1.0);
            b.look = look;
            step(b, ctx, want, speed, dt);
        }
        out
    }
}

/// The most open way away from `from`, every half second (otherwise the
/// way it's going, if that's still open). Cornered (a ridge, a fence, the
/// creek: no open run away), it takes a shorter one, and failing that
/// breaks past whatever's at it, wide of it, rather than stand there.
fn escape(nav: &NavGrid, zone: &Zone, at: Vec2, from: Vec2, vel: Vec2, rng: &mut Rng) -> Vec2 {
    let away = (at - from).normalize_or(Vec2::X);
    let going = vel.normalize_or_zero();
    let ahead_open =
        |d: Vec2, run: f32| (1..=4).all(|k| open(nav, zone, at + d * (k as f32 * run / 4.0)));
    if going != Vec2::ZERO && going.dot(away) > 0.3 && ahead_open(going, 16.0) && !rng.chance(0.03)
    {
        return going;
    }
    let dirs: Vec<Vec2> = (0..16)
        .map(|k| {
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            Vec2::new(a.cos(), a.sin())
        })
        .collect();
    let score = |d: &Vec2| {
        d.dot(away)
            + 0.3 * d.dot(going)
            + zone.forest_cover(at.x + d.x * 15.0, at.y + d.y * 15.0) * 0.4
    };
    for run in [16.0, 8.0, 4.0] {
        // Not straight back into it: past it, wide.
        let past = |d: &Vec2| d.dot(away) > -0.7;
        if let Some(d) = dirs
            .iter()
            .filter(|d| ahead_open(**d, run) && past(d))
            .max_by(|a, b| score(a).total_cmp(&score(b)))
        {
            return *d;
        }
    }
    away
}

/// Move toward `want` at up to `speed`, turning as an animal does, never
/// onto ground it can't stand on.
fn step(b: &mut Beast, ctx: &Ctx, want: Option<Vec2>, speed: f32, dt: f32) {
    let here = b.pos.xz();
    let want = want.map(|w| route(b, ctx, here, w));
    let target_vel = match want {
        Some(w) if w.distance(here) > 0.3 => (w - here).normalize() * speed,
        _ => Vec2::ZERO,
    };
    let vel = b.vel.xz();
    let accel = if speed > 5.0 { 14.0 } else { 4.0 };
    let dv = (target_vel - vel).clamp_length_max(accel * dt);
    let mut vel = vel + dv;
    let next = here + vel * dt;
    // Somewhere it can't be (put down on a rock's edge, a fence's line):
    // off it the way it's going, whatever's underfoot, until it's clear.
    if !open(ctx.nav, ctx.zone, here) {
        b.pos = ctx.zone.on_ground(next.x, next.y);
        b.vel = Vec3::new(vel.x, 0.0, vel.y);
        if vel.length() > 0.2 {
            b.yaw = vel.y.atan2(vel.x);
        }
        return;
    }
    if !open(ctx.nav, ctx.zone, next) {
        // Blocked: slide along whichever axis is open, or stop.
        let sx = Vec2::new(next.x, here.y);
        let sz = Vec2::new(here.x, next.y);
        if open(ctx.nav, ctx.zone, sx) {
            vel.y = 0.0;
        } else if open(ctx.nav, ctx.zone, sz) {
            vel.x = 0.0;
        } else {
            vel = Vec2::ZERO;
        }
    }
    let next = here + vel * dt;
    if open(ctx.nav, ctx.zone, next) {
        b.pos = ctx.zone.on_ground(next.x, next.y);
    }
    b.vel = Vec3::new(vel.x, 0.0, vel.y);
    if vel.length() > 0.2 {
        b.yaw = vel.y.atan2(vel.x);
    } else if let Some(at) = b.look {
        // Turn to face what it's watching, slowly.
        let to = at.xz() - here;
        let want = to.y.atan2(to.x);
        let d = crate::robots::anim::wrap_angle(want - b.yaw);
        b.yaw += d.clamp(-2.0 * dt, 2.0 * dt);
    }
}

/// Where to head for to get to `w`: straight at it when the way's open,
/// else the next waypoint of a path round (a fence, a rock, a building in
/// the way: a deer walked into one and stood there for good).
fn route(b: &mut Beast, ctx: &Ctx, here: Vec2, w: Vec2) -> Vec2 {
    if clear_way(ctx.nav, ctx.zone, here, w) {
        b.route.clear();
        b.route_to = None;
        return w;
    }
    b.route_age = b.route_age.saturating_add(1);
    let stale = b.route_age >= REROUTE_EVERY
        && b.route
            .first()
            .is_none_or(|&p| !clear_way(ctx.nav, ctx.zone, here, p));
    if b.route_to.is_none_or(|t| t.distance(w) > REROUTE) || stale {
        b.route = ctx
            .nav
            .path([here.x, here.y], [w.x, w.y])
            .map(|p| p.into_iter().skip(1).map(Vec2::from).collect())
            .unwrap_or_default();
        b.route_to = Some(w);
        b.route_age = 0;
    }
    while b.route.len() > 1 && b.route[0].distance(here) < WAYPOINT {
        b.route.remove(0);
    }
    b.route.first().copied().unwrap_or(w)
}

/// Herds' ticks.
#[allow(clippy::too_many_arguments)]
pub(crate) fn herds(
    mut commands: Commands,
    mut herds: Query<(Entity, &mut Herd, &mut Transform)>,
    nav: Option<Res<crate::robots::walker::Nav>>,
    zone: Res<Zone>,
    threats: Res<Threats>,
    hounds: Query<
        (Entity, &GlobalTransform, &crate::robots::Robot),
        With<crate::robots::hound::Hound>,
    >,
    mut noises: MessageReader<crate::robots::Noise>,
    landed: Res<super::Landed>,
    mut among: ResMut<Among>,
    mut startled: MessageWriter<Startled>,
    opts: Res<crate::LaunchOptions>,
    mut spawned: Local<bool>,
    mut grid: Local<Option<std::sync::Arc<NavGrid>>>,
    time: Res<Time>,
) {
    let Some(nav) = nav else {
        return;
    };
    if grid.is_none() || nav.is_changed() {
        *grid = Some(std::sync::Arc::new(herd_grid(&zone, &nav.0)));
    }
    let walk = grid.as_deref().unwrap();
    if !*spawned {
        *spawned = true;
        let off = std::env::var("EXTRACTION_WILDLIFE").is_ok_and(|v| v == "off");
        if let (Some(kind), false) = (Kind::for_map(&opts.map), off) {
            if let Some(home) = pick_home(&zone, &nav.0, opts.seed, kind) {
                let mut rng = Rng::new(opts.seed ^ 0x4E2D);
                let herd = new_herd(kind, home, &zone, &mut rng);
                info!("{} {kind:?}s about {home:.0}", herd.beasts.len());
                commands.spawn((
                    Name::new("Herd"),
                    Transform::from_translation(zone.on_ground(home.x, home.y)),
                    herd,
                ));
            }
        }
    }
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Loud noises, and things thrown among them (a stone into the
    // thicket they're in): `tick` puts them to flight within
    // FLEE_NOISE of the radius.
    let loud: Vec<(Vec3, f32)> = noises
        .read()
        .filter(|n| n.radius >= super::flock::LOUD)
        .map(|n| (n.at, n.radius))
        .chain(landed.0.iter().map(|&p| (p, THROWN_NEAR / FLEE_NOISE)))
        .collect();
    let hounds: Vec<(Entity, Vec3)> = hounds
        .iter()
        .filter(|(_, _, r)| !r.wrecked)
        .map(|(e, g, _)| (e, g.translation()))
        .collect();
    let ctx = Ctx {
        zone: &zone,
        nav: walk,
        threats: &threats.0,
        loud: &loud,
        hounds: &hounds,
    };
    among.0.clear();
    for (e, mut herd, mut tf) in &mut herds {
        among
            .0
            .extend(herd.beasts.iter().filter(|b| b.alive()).map(|b| b.pos));
        for (what, at, by, alarm) in herd.tick(&ctx, dt) {
            startled.write(Startled {
                herd: e,
                at,
                kind: herd.kind,
                what,
                by,
                alarm,
            });
        }
        if let Some(l) = herd.leader() {
            tf.translation = l.pos;
        }
    }
}

/// A dog barking is heard by the machines: at you, as from you, at the dog;
/// at a hound, as from nobody (the hound comes to the dogs). A herd
/// bolting crashes off through whatever's there, heard [`BOLT_HEARD`] as
/// from nobody (they come to look where it went, as for a thrown can), but
/// not when a machine put it up: they know their own.
pub(crate) fn bark_alarm(
    mut startled: MessageReader<Startled>,
    robots: Query<(), With<crate::robots::Robot>>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    for s in startled.read() {
        if s.alarm {
            noise.write(crate::robots::Noise {
                at: s.at + Vec3::Y * 0.6,
                radius: DOG_BARK_REACH,
                source: s.by,
            });
        } else if s.what == Startle::Bolt && !s.by.is_some_and(|b| robots.contains(b)) {
            noise.write(crate::robots::Noise {
                at: s.at + Vec3::Y * 0.6,
                radius: BOLT_HEARD,
                source: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valley(map: &str) -> (Zone, NavGrid) {
        let zone = Zone::load(map);
        let nav = extraction_core::map_nav::zone_grid(
            &zone.map,
            &zone.hm,
            extraction_core::map_nav::MAX_SLOPE,
        );
        let nav = herd_grid(&zone, &nav);
        (zone, nav)
    }

    #[test]
    fn every_valley_has_a_home_for_its_herd() {
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let (zone, nav) = valley(map);
            let home = pick_home(&zone, &nav, 1, Kind::for_map(map).unwrap())
                .unwrap_or_else(|| panic!("{map}: no home"));
            assert!(open(&nav, &zone, home), "{map}");
        }
    }

    const DT: f32 = 1.0 / 30.0;

    fn player(at: Vec3, speed: f32, crouched: bool) -> Threat {
        Threat {
            entity: Entity::PLACEHOLDER,
            at,
            speed,
            kind: ThreatKind::Player { crouched },
        }
    }

    fn herd_in(map: &str, kind: Kind) -> (Zone, NavGrid, Herd) {
        let (zone, nav) = valley(map);
        let home = pick_home(&zone, &nav, 1, kind).unwrap();
        let mut rng = Rng::new(9);
        let herd = new_herd(kind, home, &zone, &mut rng);
        (zone, nav, herd)
    }

    /// The way in to a herd from 120 m off: the most open of a few.
    fn approach(zone: &Zone, nav: &NavGrid, home: Vec2) -> Vec2 {
        (0..12)
            .map(|k| {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                Vec2::new(a.cos(), a.sin())
            })
            // The longest unbroken open run out from the herd: the walker in
            // stops at the first blocked step.
            .max_by_key(|d| {
                (0..120)
                    .take_while(|i| open(nav, zone, home + *d * *i as f32))
                    .count()
            })
            .unwrap()
    }

    /// Walk at the deer and they look up when you're within the walk's
    /// alert radius, then bolt before you're at their flight distance
    /// (keep coming and they don't wait for that); creep up crouched and
    /// you get within 18 m first.
    #[test]
    fn deer_look_up_then_bolt_as_you_come_on() {
        for map in ["marrow_creek", "kettle_pass"] {
            for (speed, crouched) in [(1.4f32, false), (0.5, true)] {
                let (zone, nav, mut herd) = herd_in(map, Kind::Deer);
                let dir = approach(&zone, &nav, herd.home);
                let mut at = herd.home + dir * 120.0;
                let (mut alerted, mut bolted) = (None, None);
                for _ in 0..(300.0 / DT) as usize {
                    // Coming on at the leader where it is now (grazing,
                    // they wander round home: a fixed line can pass them by).
                    let lead = herd.leader().unwrap().pos;
                    at += (lead.xz() - at).normalize_or(-dir) * speed * DT;
                    let me = zone.on_ground(at.x, at.y);
                    let ctx = Ctx {
                        zone: &zone,
                        nav: &nav,
                        threats: &[player(me, speed, crouched)],
                        loud: &[],
                        hounds: &[],
                    };
                    let out = herd.tick(&ctx, DT);
                    let d = me.distance(lead);
                    if alerted.is_none()
                        && matches!(herd.leader().unwrap().state, State::Alert { .. })
                    {
                        alerted = Some(d);
                    }
                    if out.iter().any(|o| o.0 == Startle::Bolt) {
                        bolted = Some(d);
                        break;
                    }
                }
                let r = alert_radius(&player(Vec3::ZERO, speed, crouched));
                let a = alerted.unwrap_or_else(|| panic!("{map}: never looked up at {speed} m/s"));
                let b = bolted.unwrap_or_else(|| panic!("{map}: never bolted at {speed} m/s"));
                assert!(
                    a <= r + 0.5 && a > r - 3.0,
                    "{map}: looked up at {a:.1} m (radius {r:.1})"
                );
                assert!(
                    b < a && b >= r * FLEE_SHARE - 1.0,
                    "{map}: bolted at {b:.1} m"
                );
                if crouched {
                    assert!(b < 18.0, "{map}: a crouched creep gets close: {b:.1} m");
                } else {
                    assert!(b > 25.0, "{map}: walking, they go from {b:.1} m");
                }
            }
        }
    }

    /// Food put down 120 m off: they amble over (four minutes or so, not
    /// at a run) and graze about it; walking among them, you're one more
    /// of them.
    #[test]
    fn bait_draws_them_and_hides_you_among_them() {
        // A 120 m open way in: the test field's meadow (a tight valley has
        // none by design).
        let (zone, nav, mut herd) = herd_in(extraction_core::map::TEST_FIELD_ID, Kind::Deer);
        let to = approach(&zone, &nav, herd.home) * 120.0 + herd.home;
        assert!(open(&nav, &zone, to));
        let ctx = Ctx {
            zone: &zone,
            nav: &nav,
            threats: &[],
            loud: &[],
            hounds: &[],
        };
        let mut came = None;
        for step in 0..(360.0 / DT) as usize {
            herd.lure(to);
            herd.tick(&ctx, DT);
            if herd.leader().unwrap().pos.xz().distance(to) < super::super::bait::EAT_NEAR {
                came = Some(step as f32 * DT);
                break;
            }
        }
        let came = came.expect("never came to the bait");
        assert!(came > 60.0, "there in {came:.0} s: ran?");
        // A minute grazing there, all together.
        for _ in 0..(60.0 / DT) as usize {
            herd.lure(to);
            herd.tick(&ctx, DT);
        }
        let among = Among(herd.beasts.iter().map(|b| b.pos).collect());
        let me = zone.on_ground(to.x, to.y);
        assert!(among.hides(me));
        assert!(!among.hides(me + Vec3::X * (AMONG_NEAR + 40.0)));
    }

    /// Cornered (the valley's edge behind them, someone 3 m off): they
    /// break out past, not stand there to be caught.
    #[test]
    fn cornered_they_break_out() {
        for map in ["marrow_creek", "kettle_pass", "sallow_reservoir"] {
            let (zone, nav, mut herd) = herd_in(map, Kind::Deer);
            // Walk out from home until the way on is shut: a corner.
            let found = (0..16).find_map(|k| {
                let a = k as f32 / 16.0 * std::f32::consts::TAU;
                let dir = Vec2::from_angle(a);
                let mut p = herd.home;
                for _ in 0..200 {
                    let next = p + dir * 2.0;
                    if !open(&nav, &zone, next) {
                        let shut = (1..=4).any(|k| !open(&nav, &zone, p + dir * (k as f32 * 4.0)));
                        let back = (1..=4).all(|k| open(&nav, &zone, p - dir * (k as f32 * 4.0)));
                        return (shut && back).then_some((p, dir));
                    }
                    p = next;
                }
                None
            });
            let Some((corner, dir)) = found else {
                continue;
            };
            for b in &mut herd.beasts {
                b.pos = zone.on_ground(corner.x, corner.y);
            }
            let me = zone.on_ground(corner.x - dir.x * 3.0, corner.y - dir.y * 3.0);
            let threats = [player(me, 4.0, false)];
            let ctx = Ctx {
                zone: &zone,
                nav: &nav,
                threats: &threats,
                loud: &[],
                hounds: &[],
            };
            let mut furthest = 0.0f32;
            for _ in 0..(20.0 / DT) as usize {
                herd.tick(&ctx, DT);
                furthest = furthest.max(herd.leader().unwrap().pos.xz().distance(me.xz()));
            }
            assert!(
                furthest > 15.0,
                "{map}: cornered at {corner:.0}, got {furthest:.0} m off"
            );
        }
    }

    /// Bait with something in the straight way (a fence, a rock, a
    /// building, water): every animal finds its way round to it, from
    /// eight homes in every valley.
    #[test]
    fn they_go_round_whats_in_the_way() {
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let Some(kind) = Kind::for_map(map) else {
                continue;
            };
            let (zone, nav) = valley(map);
            // Eight homes a valley: one home's way in proves little (a
            // home moved by a map change found a deer stuck 81 m off).
            for seed in 1..=8u64 {
                let Some(home) = pick_home(&zone, &nav, seed, kind) else {
                    continue;
                };
                let mut rng = Rng::new(9 + seed);
                let herd = new_herd(kind, home, &zone, &mut rng);
                if let Err(e) = round_to_bait(&zone, &nav, herd) {
                    panic!("{map}, seed {seed}: {kind:?} {e}");
                }
            }
        }
    }

    /// Bait 60-150 m off with the straight line to it blocked: every
    /// animal there within ten minutes, or which one wasn't, and where.
    fn round_to_bait(zone: &Zone, nav: &NavGrid, mut herd: Herd) -> Result<(), String> {
        let from = herd.home;
        let to = (0..48).find_map(|k| {
            let a = k as f32 / 48.0 * std::f32::consts::TAU;
            (60..=150).step_by(15).find_map(|d| {
                let p = from + Vec2::from_angle(a) * d as f32;
                (open(nav, zone, p)
                    && !nav.line_clear([from.x, from.y], [p.x, p.y])
                    && nav.path([from.x, from.y], [p.x, p.y]).is_some())
                .then_some(p)
            })
        });
        let Some(to) = to else {
            return Ok(());
        };
        let ctx = Ctx {
            zone,
            nav,
            threats: &[],
            loud: &[],
            hounds: &[],
        };
        for _ in 0..(600.0 / DT) as usize {
            herd.lure(to);
            herd.tick(&ctx, DT);
            if herd
                .beasts
                .iter()
                .all(|b| b.pos.xz().distance(to) < AMONG_NEAR)
            {
                return Ok(());
            }
        }
        for (i, b) in herd.beasts.iter().enumerate() {
            let d = b.pos.xz().distance(to);
            if d >= AMONG_NEAR {
                return Err(format!(
                    "{i} stuck {d:.0} m off at {:.0} ({:?}, route {:?}, bait at {to:.0}, leader at {:.0})",
                    b.pos,
                    b.state,
                    b.route,
                    herd.leader().unwrap().pos
                ));
            }
        }
        Ok(())
    }

    /// A stone into the thicket they're grazing in puts them to flight
    /// (who threw it, from where, they don't know); one well off doesn't.
    #[test]
    fn a_stone_among_them_puts_them_to_flight() {
        for (d, runs) in [(5.0, true), (THROWN_NEAR * 2.0, false)] {
            let (zone, nav, mut herd) = herd_in("marrow_creek", Kind::Deer);
            let lead = herd.leader().unwrap().pos;
            let loud = [(lead + Vec3::X * d, THROWN_NEAR / FLEE_NOISE)];
            let ctx = Ctx {
                zone: &zone,
                nav: &nav,
                threats: &[],
                loud: &loud,
                hounds: &[],
            };
            let out = herd.tick(&ctx, DT);
            let bolted = out.iter().any(|o| o.0 == Startle::Bolt && o.2.is_none());
            assert_eq!(bolted, runs, "a stone {d} m off");
        }
    }

    /// Six minutes of being chased and shot at, in every valley: no animal
    /// ever stands anywhere it can't (in a building, a rock, a trunk, deep
    /// water, off the grid), always on the ground, and a run carries them
    /// well away.
    #[test]
    fn herds_never_stand_where_they_cant_and_get_away() {
        for (map, kind) in [
            ("marrow_creek", Kind::Deer),
            ("kettle_pass", Kind::Deer),
            ("cinder_works", Kind::Dog),
        ] {
            let (zone, nav, mut herd) = herd_in(map, kind);
            let mut rng = Rng::new(4);
            // From the herd's most open side ("east of it" is no sure field
            // in a tight valley).
            let mut at = herd.home + approach(&zone, &nav, herd.home) * 60.0;
            let mut runs = 0;
            let mut furthest = 0.0f32;
            for step in 0..(360.0 / DT) as usize {
                // Chase the leader at a run, now and then a shot.
                let lead = herd.leader().unwrap().pos;
                let to = lead.xz() - at;
                if to.length() > 3.0 {
                    let next = at + to.normalize() * 4.0 * DT;
                    if open(&nav, &zone, next) {
                        at = next;
                    }
                }
                let me = zone.on_ground(at.x, at.y);
                let loud = if rng.chance(DT / 30.0) {
                    vec![(me, 220.0)]
                } else {
                    vec![]
                };
                let threats = [player(me, 4.0, false)];
                let hounds: Vec<(Entity, Vec3)> = if kind == Kind::Dog && step % 3000 < 300 {
                    vec![(Entity::PLACEHOLDER, me + Vec3::X * 40.0)]
                } else {
                    vec![]
                };
                let ctx = Ctx {
                    zone: &zone,
                    nav: &nav,
                    threats: &threats,
                    loud: &loud,
                    hounds: &hounds,
                };
                let out = herd.tick(&ctx, DT);
                runs += out.iter().filter(|o| o.0 == Startle::Bolt).count();
                for (i, b) in herd.beasts.iter().enumerate() {
                    assert!(
                        open(&nav, &zone, b.pos.xz()),
                        "{map}: beast {i} somewhere it can't be at {:.1}",
                        b.pos
                    );
                    assert!(
                        (b.pos.y - zone.ground(b.pos.x, b.pos.z)).abs() < 0.01,
                        "{map}: off the ground"
                    );
                    assert!(b.vel.length() <= kind.speeds().0 + 0.01);
                }
                furthest = furthest.max(herd.leader().unwrap().pos.distance(me));
            }
            assert!(runs >= 3, "{map}: ran {runs} times");
            assert!(
                furthest > 60.0,
                "{map}: never got more than {furthest:.0} m away"
            );
        }
    }

    /// Dogs keep away from you, and bark at you as you come (once, heard by
    /// the machines), and a hound has one that it catches.
    #[test]
    fn dogs_keep_their_distance_bark_and_fear_hounds() {
        let (zone, nav, mut herd) = herd_in("cinder_works", Kind::Dog);
        let dir = approach(&zone, &nav, herd.home);
        let mut at = herd.home + dir * 60.0;
        let mut barks = 0;
        let mut nearest = f32::MAX;
        for _ in 0..(40.0 / DT) as usize {
            let next = at - dir * 1.4 * DT;
            if open(&nav, &zone, next) {
                at = next;
            }
            let me = zone.on_ground(at.x, at.y);
            let ctx = Ctx {
                zone: &zone,
                nav: &nav,
                threats: &[player(me, 1.4, false)],
                loud: &[],
                hounds: &[],
            };
            for o in herd.tick(&ctx, DT) {
                if o.0 == Startle::Bark {
                    assert!(o.3, "a dog's bark at you is an alarm");
                    barks += 1;
                }
            }
            for b in &herd.beasts {
                nearest = nearest.min(b.pos.distance(me));
            }
        }
        assert_eq!(barks, 1, "barked {barks} times in 40 s");
        assert!(nearest > 10.0, "a dog let you within {nearest:.1} m");
        // A hound on top of one: it's had.
        let dog = herd.beasts[0].pos;
        let ctx = Ctx {
            zone: &zone,
            nav: &nav,
            threats: &[],
            loud: &[],
            hounds: &[(Entity::PLACEHOLDER, dog)],
        };
        let out = herd.tick(&ctx, DT);
        assert!(out.iter().any(|o| o.0 == Startle::Killed));
        assert_eq!(herd.beasts[0].state, State::Dead);
    }

    #[test]
    fn herds_are_deterministic() {
        let run = || {
            let (zone, nav, mut herd) = herd_in("marrow_creek", Kind::Deer);
            let at = zone.on_ground(herd.home.x + 30.0, herd.home.y);
            for _ in 0..(30.0 / DT) as usize {
                let ctx = Ctx {
                    zone: &zone,
                    nav: &nav,
                    threats: &[player(at, 2.0, false)],
                    loud: &[],
                    hounds: &[],
                };
                herd.tick(&ctx, DT);
            }
            herd.beasts.iter().map(|b| b.pos).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
