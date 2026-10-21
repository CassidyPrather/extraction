//! Mallards on open water: a tell across water.
//!
//! A raft ([`Raft`]) is three to nine ducks, drakes and their hens,
//! paddling slowly about a spot of open water ([`Water`]: deep enough to
//! float, [`MIN_DEPTH`], with room round it), dabbling now and then (tail
//! up), keeping together by a few boids rules on the surface. They're
//! wary: someone on the bank within [`WARY`] metres and they swim off,
//! slowly, keeping their distance; nearer than [`FLUSH`], or anyone
//! running within [`WARY`], or a shot within [`flock::FLUSH_NOISE`] of its
//! reach, and they burst up off the water ([`DuckState::RunUp`]: a few
//! metres of splashing run along the surface), fly low and fast over the
//! water ([`LOW`]; higher over land, [`OVER_LAND`]) to the far end or
//! another water, and come down with a skid ([`DuckState::Skid`]).
//! Machines scare them the same.
//!
//! A raft a player flushes says so the way a flock does: a
//! [`Flushed`] (its `flock` the raft's entity, `birds` its ducks), with
//! `alarm` at most once every [`ALARM_EVERY`] seconds a raft, which
//! `flock::alarm_robots` turns into a [`crate::robots::Noise`]. Sound and
//! the machines hear ducks without knowing they're ducks; to tell, look
//! for a [`Raft`] on the entity.
//!
//! Every valley with water deep enough gets one or two rafts, at places
//! fixed by the map and the seed ([`homes`]). On the water a duck is
//! always at the surface ([`surface`]) of water at least [`MIN_DEPTH`]
//! deep and outside anything built ([`perches::Obstacles`]); in the air
//! it's never under the ground or the water, or inside anything.
//! Headless and deterministic for a seed.

use bevy::prelude::*;
use extraction_core::noise::Rng;

use super::flock::{self, Flush};
use super::perches::Obstacles;
use super::{FlushCause, Flushed, Threat, ThreatKind, Threats};
use crate::world::Zone;

// --- Where they live -------------------------------------------------------

/// Ducks float on water at least this deep (m).
pub const MIN_DEPTH: f32 = 0.25;
/// The water is sampled this often for places a raft can sit (m)...
pub const SAMPLE: f32 = 2.0;
/// ...each with open water at least this far all round it (m)...
pub const ROOM: f32 = 3.0;
/// ...counted out to this.
pub const ROOM_MAX: f32 = 8.0;
/// Nothing built within this of a raft's place (a bridge, the dam).
pub const CLEAR: f32 = 3.0;
/// Ducks in a raft, at least and at most.
pub const RAFT: (usize, usize) = (3, 9);
/// A valley's second raft is at least this far from its first.
pub const RAFTS_APART: f32 = 120.0;

// --- What scares them ------------------------------------------------------

/// Someone this close and they swim away...
pub const WARY: f32 = 25.0;
/// ...this close (or running within [`WARY`]) and they go up.
pub const FLUSH: f32 = 12.0;
/// A run, m/s.
pub const RUN: f32 = 3.5;
/// Crouched, you get this much closer.
pub const CROUCHED: f32 = 0.7;
/// After a fright they're warier for a while: this much further.
pub const SHY: f32 = 1.3;
pub const SHY_FOR: f32 = 120.0;
/// A raft a player flushes raises the alarm at most this often (s).
pub const ALARM_EVERY: f32 = 60.0;

// --- How they move ---------------------------------------------------------

/// Paddling about, and swimming off from someone, m/s.
pub const PADDLE: f32 = 0.3;
pub const AWAY: f32 = 0.8;
/// The run-up: this long, to this speed (m/s) when they lift off.
pub const RUN_UP: f32 = 0.8;
pub const TAKE_OFF: f32 = 5.5;
/// Flying: speed, and how hard they turn and climb.
pub const FLY: f32 = 15.0;
pub const MAX_SPEED: f32 = 18.0;
pub const ACCEL: f32 = 14.0;
/// Over water they fly this low; over land this high.
pub const LOW: f32 = 3.0;
pub const OVER_LAND: f32 = 12.0;
/// Coming in, they start down this far out (m).
pub const APPROACH: f32 = 45.0;
/// The next water is this far off, give or take...
pub const HOP: (f32, f32) = (70.0, 420.0);
/// ...and at least this far from whatever scared them.
pub const SAFE: f32 = 60.0;
/// The skid's slowing, per second.
pub const SKID: f32 = 2.2;

/// How deep the water is at `p` (0 on dry land).
pub fn depth(zone: &Zone, p: Vec2) -> f32 {
    super::fish::depth(zone, p)
}

/// The surface a duck floats on at `p`, if the water there is deep
/// enough.
pub fn surface(zone: &Zone, p: Vec2) -> Option<f32> {
    (depth(zone, p) >= MIN_DEPTH)
        .then(|| zone.hm.water_level([p.x, p.y]))
        .flatten()
}

/// Can a duck float at `p`: deep enough, and not inside anything built
/// (a bridge's pier, the dam's foot).
pub fn swimmable(zone: &Zone, obstacles: &Obstacles, p: Vec2) -> bool {
    surface(zone, p).is_some_and(|y| !obstacles.inside(Vec3::new(p.x, y + 0.12, p.y), 0.25))
}

/// Places on the valley's water a raft can sit: open water, deep enough,
/// clear of anything built, and how much room round each (m).
#[derive(Resource, Default, Clone, Debug)]
pub struct Water {
    pub spots: Vec<(Vec2, f32)>,
}

/// How far deep water runs all round `p`, out to [`ROOM_MAX`].
fn room(zone: &Zone, p: Vec2) -> f32 {
    let mut r = 1.0;
    while r <= ROOM_MAX {
        for k in 0..8 {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            if depth(zone, p + Vec2::from_angle(a) * r) < MIN_DEPTH {
                return r - 1.0;
            }
        }
        r += 1.0;
    }
    ROOM_MAX
}

/// The valley's water, sampled every [`SAMPLE`] metres.
pub fn survey(zone: &Zone, obstacles: &Obstacles) -> Water {
    let half = zone.map.size * 0.5;
    let n = (zone.map.size / SAMPLE) as i32;
    let mut spots = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            let p = Vec2::new(-half + i as f32 * SAMPLE, -half + j as f32 * SAMPLE);
            let Some(y) = surface(zone, p) else {
                continue;
            };
            if depth(zone, p) < MIN_DEPTH + 0.1 {
                continue;
            }
            let r = room(zone, p);
            if r < ROOM {
                continue;
            }
            if obstacles.top_near(Vec3::new(p.x, y, p.y), CLEAR).is_some() {
                continue;
            }
            spots.push((p, r));
        }
    }
    Water { spots }
}

/// Where a valley's rafts start: the roomiest water more often (a
/// reservoir's arm before a creek's pool), one or two, the second well
/// away from the first. Indices into `water.spots`.
pub fn homes(water: &Water, map: &str, seed: u64) -> Vec<usize> {
    let mut rng = Rng::new(seed ^ 0xD0C5_A11A ^ flock::name_hash(map));
    let mut out = Vec::new();
    for _ in 0..2 {
        let weights: Vec<f32> = water
            .spots
            .iter()
            .map(|(p, r)| {
                let apart = out
                    .iter()
                    .all(|&k: &usize| water.spots[k].0.distance(*p) > RAFTS_APART);
                if apart { r * r } else { 0.0 }
            })
            .collect();
        match rng.weighted(&weights) {
            Some(k) => out.push(k),
            None => break,
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DuckState {
    /// On the water: paddling, or dabbling ([`Duck::dabble`]).
    Swim,
    /// Running along the surface, wings going, to get up.
    RunUp {
        left: f32,
    },
    Fly,
    /// Down on the water, sliding to a stop.
    Skid,
}

#[derive(Clone, Debug)]
pub struct Duck {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Heading, radians about Y (atan2(z, x)).
    pub yaw: f32,
    pub state: DuckState,
    /// A drake (green head) or a hen.
    pub drake: bool,
    /// How hard it's working its wings, 0 to 1.
    pub flap: f32,
    /// Seconds left with its tail up.
    pub dabble: f32,
    /// Seconds before it reacts to a fright.
    delay: f32,
    /// Its place in the raft, about the raft's home.
    slot: Vec2,
    /// Where it comes down, flying.
    land: Vec2,
}

impl Duck {
    pub fn on_water(&self) -> bool {
        matches!(
            self.state,
            DuckState::Swim | DuckState::Skid | DuckState::RunUp { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Paddling about its home.
    Afloat,
    /// Flushed: up, away and down onto `water.spots[to]`.
    Up { to: usize, time: f32 },
}

#[derive(Component, Clone, Debug)]
pub struct Raft {
    pub ducks: Vec<Duck>,
    pub phase: Phase,
    /// What it paddles about (it moves, as they swim off from you).
    pub home: Vec2,
    /// The water spot it's on or headed for.
    pub spot: usize,
    rng: Rng,
    /// Seconds it stays warier.
    pub shy: f32,
    alarm_cool: f32,
    /// Where its home has been, swimming off: the way along the water
    /// for the ones left behind. Oldest first.
    trail: Vec<Vec2>,
}

/// What a raft's tick needs to know.
pub struct Ctx<'a> {
    pub zone: &'a Zone,
    pub obstacles: &'a Obstacles,
    pub water: &'a Water,
    pub threats: &'a [Threat],
    /// Loud noises: where, and how near scares them.
    pub loud: &'a [(Vec3, f32)],
    /// The other rafts' homes, not to land on.
    pub others: &'a [Vec2],
}

pub fn new_raft(
    zone: &Zone,
    obstacles: &Obstacles,
    water: &Water,
    spot: usize,
    rng: &mut Rng,
) -> Raft {
    let home = water.spots[spot].0;
    let n = RAFT.0 + rng.below(RAFT.1 - RAFT.0 + 1);
    let spread = 0.5 + 0.45 * (n as f32).sqrt();
    let ducks = (0..n)
        .map(|_| {
            let mut slot = Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * spread;
            if !swimmable(zone, obstacles, home + slot) {
                slot = Vec2::ZERO;
            }
            let p = home + slot;
            let y = surface(zone, p).unwrap_or(0.0);
            Duck {
                pos: Vec3::new(p.x, y, p.y),
                vel: Vec3::ZERO,
                yaw: rng.range(0.0, std::f32::consts::TAU),
                state: DuckState::Swim,
                drake: rng.chance(0.55),
                flap: 0.0,
                dabble: 0.0,
                delay: 0.0,
                slot,
                land: p,
            }
        })
        .collect();
    Raft {
        ducks,
        phase: Phase::Afloat,
        home,
        spot,
        rng: Rng::new(rng.next_u64()),
        shy: 0.0,
        alarm_cool: 0.0,
        trail: Vec::new(),
    }
}

impl Raft {
    /// Where the ducks are, on average.
    pub fn mean(&self) -> Vec3 {
        self.ducks.iter().map(|d| d.pos).sum::<Vec3>() / self.ducks.len().max(1) as f32
    }

    pub fn afloat(&self) -> bool {
        self.phase == Phase::Afloat
    }

    /// The home it's on or headed for.
    pub fn heading_for(&self, water: &Water) -> Vec2 {
        match self.phase {
            Phase::Afloat => self.home,
            Phase::Up { to, .. } => water.spots[to].0,
        }
    }

    /// One tick: frights, then paddling or flight.
    pub fn tick(&mut self, ctx: &Ctx, dt: f32) -> Option<Flush> {
        self.shy = (self.shy - dt).max(0.0);
        self.alarm_cool = (self.alarm_cool - dt).max(0.0);
        let mut flushed = None;
        let mut flee = Vec2::ZERO;
        if self.afloat() {
            let at = self.mean();
            let shy = if self.shy > 0.0 { SHY } else { 1.0 };
            let mut fright: Option<(f32, &Threat)> = None;
            for t in ctx.threats {
                let k = shy
                    * match t.kind {
                        ThreatKind::Player { crouched: true } => CROUCHED,
                        _ => 1.0,
                    };
                let d = t.at.distance(at);
                if d < FLUSH * k || (d < WARY * k && t.speed > RUN) {
                    if fright.is_none_or(|(f, _)| d < f) {
                        fright = Some((d, t));
                    }
                } else if d < WARY * k {
                    flee += (at - t.at).xz().normalize_or_zero() * (1.0 - d / (WARY * k) + 0.2);
                }
            }
            if let Some((_, t)) = fright {
                let cause = match t.kind {
                    ThreatKind::Player { .. } => FlushCause::Player,
                    _ => FlushCause::Robot,
                };
                flushed = Some(self.flush(ctx, cause, t.at, Some(t.entity)));
            } else if let Some(&(p, _)) = ctx.loud.iter().find(|(p, r)| p.distance(at) < *r) {
                flushed = Some(self.flush(ctx, FlushCause::Noise, p, None));
            }
        }
        if self.afloat() {
            self.swim_off(ctx, flee, dt);
        }
        self.move_ducks(ctx, flee != Vec2::ZERO, dt);
        if let Phase::Up { to, time } = self.phase {
            if self.ducks.iter().all(|d| d.state == DuckState::Swim) {
                self.home = ctx.water.spots[to].0;
                self.trail.clear();
                self.spot = to;
                for d in &mut self.ducks {
                    d.slot = d.land - self.home;
                }
                self.phase = Phase::Afloat;
            } else {
                self.phase = Phase::Up {
                    to,
                    time: time + dt,
                };
            }
        }
        flushed
    }

    /// Someone on the bank: the raft's home moves off, slowly, along the
    /// water away from them.
    fn swim_off(&mut self, ctx: &Ctx, flee: Vec2, dt: f32) {
        if flee == Vec2::ZERO {
            return;
        }
        let dir = flee.normalize();
        // Straight away if the water lets it, else round along the bank.
        for turn in [0.0f32, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5] {
            let d = Vec2::from_angle(turn).rotate(dir);
            let next = self.home + d * AWAY * dt;
            // Room ahead for the raft to follow.
            if swimmable(ctx.zone, ctx.obstacles, next + d * 2.0)
                && swimmable(ctx.zone, ctx.obstacles, next)
            {
                self.home = next;
                if self.trail.last().is_none_or(|t| t.distance(next) > 1.0) {
                    self.trail.push(next);
                    if self.trail.len() > 40 {
                        self.trail.remove(0);
                    }
                }
                return;
            }
        }
    }

    /// Up they go, from a fright at `from` (and who it was).
    fn flush(&mut self, ctx: &Ctx, cause: FlushCause, from: Vec3, by: Option<Entity>) -> Flush {
        let at = self.mean();
        let to = self.destination(ctx, from.xz(), at.xz());
        self.phase = Phase::Up { to, time: 0.0 };
        self.shy = SHY_FOR;
        let dest = ctx.water.spots[to].0;
        let away = (at - from).xz().normalize_or(Vec2::X);
        for d in &mut self.ducks {
            // Nearest the fright go first.
            d.delay = (d.pos.distance(from) * 0.015).min(0.35) + self.rng.range(0.0, 0.3);
            d.state = DuckState::RunUp { left: RUN_UP };
            d.dabble = 0.0;
            let dir = Vec2::from_angle(self.rng.range(-0.4, 0.4)).rotate(away);
            d.vel = Vec3::new(dir.x, 0.0, dir.y) * 0.5;
            let land = dest + d.slot;
            d.land = if swimmable(ctx.zone, ctx.obstacles, land) {
                land
            } else {
                dest
            };
        }
        let alarm = cause == FlushCause::Player && self.alarm_cool <= 0.0;
        if alarm {
            self.alarm_cool = ALARM_EVERY;
        }
        Flush {
            at,
            cause,
            by: by.filter(|_| matches!(cause, FlushCause::Player | FlushCause::Robot)),
            alarm,
        }
    }

    /// Other water, well away from what scared them, more often onward
    /// than back past it; failing that, anywhere quiet.
    fn destination(&mut self, ctx: &Ctx, from: Vec2, at: Vec2) -> usize {
        let away = (at - from).normalize_or(Vec2::X);
        let quiet = |p: Vec2, safe: f32| {
            ctx.threats.iter().all(|t| t.at.xz().distance(p) > safe)
                && p.distance(from) > safe
                && ctx.others.iter().all(|o| o.distance(p) > 30.0)
        };
        let weights: Vec<f32> = ctx
            .water
            .spots
            .iter()
            .map(|&(p, r)| {
                let d = p.distance(at);
                if !(HOP.0..=HOP.1).contains(&d) || !quiet(p, SAFE) {
                    return 0.0;
                }
                (1.2 + away.dot((p - at) / d)) * r / (d + 60.0)
            })
            .collect();
        if let Some(k) = self.rng.weighted(&weights) {
            return k;
        }
        // Nowhere quiet in reach: the furthest from the fright.
        (0..ctx.water.spots.len())
            .max_by(|&a, &b| {
                let f = |i: usize| ctx.water.spots[i].0.distance(from);
                f(a).total_cmp(&f(b))
            })
            .unwrap_or(self.spot)
    }

    fn move_ducks(&mut self, ctx: &Ctx, fleeing: bool, dt: f32) {
        let (zone, obstacles) = (ctx.zone, ctx.obstacles);
        let snapshot: Vec<(Vec3, bool)> = self
            .ducks
            .iter()
            .map(|d| (d.pos, d.state == DuckState::Fly))
            .collect();
        let up = match self.phase {
            Phase::Up { to, .. } => Some(ctx.water.spots[to].0),
            Phase::Afloat => None,
        };
        let home = self.home;
        let trail = std::mem::take(&mut self.trail);
        for i in 0..self.ducks.len() {
            let rng = &mut self.rng;
            let d = &mut self.ducks[i];
            if d.delay > 0.0 {
                d.delay -= dt;
                // Not yet off: still on the water where it was.
                d.vel *= (1.0 - dt * 2.0).max(0.0);
                continue;
            }
            match d.state {
                DuckState::Swim => {
                    // Its place in the raft, or the raft's home if the raft
                    // has swum on and its place is up the bank.
                    let target = match up {
                        Some(_) => d.land,
                        None if swimmable(zone, obstacles, home + d.slot) => home + d.slot,
                        None => home,
                    };
                    // Left behind: along the way the raft went, not
                    // straight at it over the bank.
                    let target = match trail
                        .iter()
                        .enumerate()
                        .min_by(|a, b| {
                            let f = |p: &Vec2| p.distance_squared(d.pos.xz());
                            f(a.1).total_cmp(&f(b.1))
                        })
                        .map(|(k, _)| k)
                    {
                        Some(k) if up.is_none() && d.pos.xz().distance(target) > 3.0 => {
                            trail.get(k + 2).copied().unwrap_or(target)
                        }
                        _ => target,
                    };
                    // Separation on the water.
                    let mut sep = Vec2::ZERO;
                    for (j, &(p, flying)) in snapshot.iter().enumerate() {
                        let off = d.pos.xz() - p.xz();
                        let l = off.length();
                        if j != i && !flying && l < 0.7 && l > 1e-4 {
                            sep += off / l * (0.7 - l);
                        }
                    }
                    if d.dabble > 0.0 {
                        d.dabble = if fleeing { 0.0 } else { d.dabble - dt };
                    } else if !fleeing && up.is_none() && rng.chance(dt / 14.0) {
                        d.dabble = rng.range(2.0, 5.0);
                    }
                    // Now and then one paddles off to another place in the raft.
                    if up.is_none() && rng.chance(dt / 25.0) {
                        let s = d.slot.length().max(1.0);
                        let nudge = Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
                        let slot = (d.slot + nudge).clamp_length_max(s);
                        if swimmable(zone, obstacles, home + slot) {
                            d.slot = slot;
                        }
                    }
                    let max = if fleeing { AWAY * 1.4 } else { PADDLE };
                    let mut want = ((target - d.pos.xz()) * 0.5).clamp_length_max(max) + sep * 1.5;
                    if d.dabble > 0.0 {
                        want = Vec2::ZERO;
                    }
                    let v = d.vel.xz().lerp(want, (dt * 1.5).min(1.0));
                    d.vel = Vec3::new(v.x, 0.0, v.y);
                    paddle(d, zone, obstacles, dt);
                    if v.length() > 0.05 {
                        turn_to(d, v.y.atan2(v.x), 2.0 * dt);
                    }
                    d.flap = 0.0;
                }
                DuckState::RunUp { left } => {
                    let dir = d.vel.xz().normalize_or(Vec2::from_angle(d.yaw));
                    let speed = d.vel.xz().length() + TAKE_OFF / RUN_UP * dt;
                    let v = dir * speed;
                    d.vel = Vec3::new(v.x, 0.0, v.y);
                    d.flap = 1.0;
                    turn_to(d, dir.y.atan2(dir.x), 8.0 * dt);
                    let next = d.pos.xz() + v * dt;
                    let left = left - dt;
                    if left <= 0.0 || !swimmable(zone, obstacles, next) {
                        // Off the water.
                        d.state = DuckState::Fly;
                        d.vel.y = 3.0;
                        d.pos.y += 0.35;
                    } else {
                        d.state = DuckState::RunUp { left };
                        let y = surface(zone, next).unwrap_or(d.pos.y);
                        d.pos = Vec3::new(next.x, y, next.y);
                    }
                }
                DuckState::Fly => {
                    let Some(_) = up else {
                        // Shouldn't be: down onto the water where it is,
                        // or home.
                        d.state = DuckState::Skid;
                        continue;
                    };
                    fly(d, &snapshot, i, zone, obstacles, dt);
                }
                DuckState::Skid => {
                    let k = (-SKID * dt).exp();
                    d.vel = Vec3::new(d.vel.x * k, 0.0, d.vel.z * k);
                    paddle(d, zone, obstacles, dt);
                    d.flap = 0.0;
                    if d.vel.length() < 0.4 {
                        d.state = DuckState::Swim;
                    }
                }
            }
        }
        self.trail = trail;
    }
}

/// On the water: move by its velocity if the water there will float it,
/// sliding along the bank if not; always at the surface.
fn paddle(d: &mut Duck, zone: &Zone, obstacles: &Obstacles, dt: f32) {
    let step = d.vel.xz() * dt;
    let p = d.pos.xz();
    // Straight on, or turned along the bank as little as will do.
    let next = [0.0f32, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5]
        .into_iter()
        .map(|a| Vec2::from_angle(a).rotate(step))
        .find(|&s| swimmable(zone, obstacles, p + s));
    match next {
        Some(s) => {
            let q = p + s;
            if dt > 0.0 {
                d.vel = Vec3::new(s.x, 0.0, s.y) / dt;
            }
            d.pos = Vec3::new(q.x, surface(zone, q).unwrap_or(d.pos.y), q.y);
        }
        None => {
            d.vel = Vec3::ZERO;
            if let Some(y) = surface(zone, p) {
                d.pos.y = y;
            }
        }
    }
}

fn turn_to(d: &mut Duck, want: f32, max: f32) {
    let a = crate::robots::anim::wrap_angle(want - d.yaw);
    d.yaw += a.clamp(-max, max);
}

/// What's under a point: the water's surface, or the ground, and which.
fn floor(zone: &Zone, p: Vec2) -> (f32, bool) {
    match zone.hm.water_level([p.x, p.y]) {
        Some(w) => (w, true),
        None => (zone.ground(p.x, p.y), false),
    }
}

/// The flight out to its water: low over water, higher over land and
/// anything built, coming down to the surface and onto it.
fn fly(
    d: &mut Duck,
    snapshot: &[(Vec3, bool)],
    i: usize,
    zone: &Zone,
    obstacles: &Obstacles,
    dt: f32,
) {
    let to = d.land - d.pos.xz();
    let h = to.length();
    let dir = to.normalize_or(Vec2::from_angle(d.yaw));
    // How high to be: over what's under it and just ahead.
    let mut cruise = f32::MIN;
    let mut all_water = true;
    for ahead in [0.0f32, 8.0, 20.0] {
        let q = d.pos.xz() + dir * ahead.min(h);
        let (f, water) = floor(zone, q);
        all_water &= water;
        cruise = cruise.max(f);
        if let Some(top) = obstacles.top_near(Vec3::new(q.x, f, q.y), 5.0) {
            cruise = cruise.max(top + 4.0 - LOW);
        }
    }
    cruise += if all_water { LOW } else { OVER_LAND };
    let land_y = surface(zone, d.land).unwrap_or(cruise);
    let coming_in = h < APPROACH;
    let target_y = if coming_in {
        let glide = land_y + LOW * ((h - 5.0) / (APPROACH - 5.0)).clamp(0.0, 1.0);
        // Over land still: not down into the trees.
        let (f, water) = floor(zone, d.pos.xz());
        if water { glide } else { glide.max(f + 4.0) }
    } else {
        cruise
    };
    let speed = if coming_in {
        6.0 + (FLY - 6.0) * (h / APPROACH)
    } else {
        FLY
    };
    let mut sep = Vec3::ZERO;
    for (j, &(p, flying)) in snapshot.iter().enumerate() {
        let off = d.pos - p;
        let l = off.length();
        if j != i && flying && l < 1.2 && l > 1e-4 {
            sep += off / l * (1.2 - l);
        }
    }
    let want = Vec3::new(
        dir.x * speed,
        ((target_y - d.pos.y) * 1.5).clamp(-4.0, 5.0),
        dir.y * speed,
    ) + sep * 4.0;
    let acc = ((want - d.vel) * 3.0).clamp_length_max(ACCEL);
    d.vel = (d.vel + acc * dt).clamp_length_max(MAX_SPEED);
    // Wings: hard climbing and speeding up, set and gliding coming in.
    let work = (acc.y.max(0.0) / ACCEL + acc.length() / (ACCEL * 2.0)).min(1.0);
    let flap = if coming_in && h < 25.0 {
        0.1
    } else {
        work.max(0.45)
    };
    d.flap += (flap - d.flap) * (dt * 3.0).min(1.0);
    let prev = d.pos;
    d.pos += d.vel * dt;
    let v = d.vel.xz();
    if v.length() > 0.5 {
        turn_to(d, v.y.atan2(v.x), 6.0 * dt);
    }
    // Down: near its spot, low over water that floats it.
    if h < 7.0
        && let Some(y) = surface(zone, d.pos.xz())
        && d.pos.y - y < 0.6
        && swimmable(zone, obstacles, d.pos.xz())
    {
        d.pos.y = y;
        d.vel.y = 0.0;
        d.state = DuckState::Skid;
        return;
    }
    // Never under the water or the ground, or inside anything built.
    let (f, _) = floor(zone, d.pos.xz());
    let g = zone.ground(d.pos.x, d.pos.z);
    let min = f.max(g) + 0.3;
    if d.pos.y < min {
        d.pos.y = min;
        d.vel.y = d.vel.y.max(0.5);
    }
    if obstacles.inside(d.pos, 0.3) {
        d.pos = prev;
        let mut lift = 0;
        while obstacles.inside(d.pos, 0.3) && lift < 80 {
            d.pos.y += 0.25;
            lift += 1;
        }
        d.vel = Vec3::new(d.vel.x * 0.3, d.vel.y.max(2.0), d.vel.z * 0.3);
    }
}

/// Every raft's tick; the rafts themselves once the valley's surveyed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rafts(
    mut commands: Commands,
    mut rafts: Query<(Entity, &mut Raft, &mut Transform)>,
    obstacles: Option<Res<Obstacles>>,
    water: Option<Res<Water>>,
    threats: Res<Threats>,
    zone: Res<Zone>,
    opts: Res<crate::LaunchOptions>,
    mut noises: MessageReader<crate::robots::Noise>,
    mut flushed: MessageWriter<Flushed>,
    time: Res<Time>,
) {
    let Some(obstacles) = obstacles else {
        noises.clear();
        return;
    };
    let Some(water) = water else {
        let started = std::time::Instant::now();
        let water = survey(&zone, &obstacles);
        let off = std::env::var("EXTRACTION_WILDLIFE").is_ok_and(|v| v == "off");
        let homes = if off {
            Vec::new()
        } else {
            homes(&water, &opts.map, opts.seed)
        };
        let mut rng = Rng::new(opts.seed ^ 0xD0C5 ^ flock::name_hash(&opts.map));
        for &k in &homes {
            let raft = new_raft(&zone, &obstacles, &water, k, &mut rng);
            info!(
                "a raft of {} mallards on the water at {:.0}",
                raft.ducks.len(),
                water.spots[k].0
            );
            commands.spawn((
                Name::new("Raft"),
                Transform::from_translation(raft.mean()),
                Visibility::default(),
                raft,
            ));
        }
        debug!(
            "ducks: {} places on open water, in {:.0} ms",
            water.spots.len(),
            started.elapsed().as_secs_f32() * 1000.0
        );
        commands.insert_resource(water);
        noises.clear();
        return;
    };
    let dt = time.delta_secs();
    let loud: Vec<(Vec3, f32)> = noises
        .read()
        .filter(|n| n.radius >= flock::LOUD)
        .map(|n| (n.at, n.radius * flock::FLUSH_NOISE))
        .collect();
    if dt <= 0.0 || water.spots.is_empty() {
        return;
    }
    let homes: Vec<(Entity, Vec2)> = rafts
        .iter()
        .map(|(e, r, _)| (e, r.heading_for(&water)))
        .collect();
    for (e, mut raft, mut tf) in &mut rafts {
        let others: Vec<Vec2> = homes
            .iter()
            .filter(|(o, _)| *o != e)
            .map(|(_, h)| *h)
            .collect();
        let ctx = Ctx {
            zone: &zone,
            obstacles: &obstacles,
            water: &water,
            threats: &threats.0,
            loud: &loud,
            others: &others,
        };
        if let Some(f) = raft.tick(&ctx, dt) {
            debug!("a raft went up at {:.0} ({:?})", f.at, f.cause);
            flushed.write(Flushed {
                flock: e,
                at: f.at,
                birds: raft.ducks.len() as u32,
                cause: f.cause,
                by: f.by,
                alarm: f.alarm,
            });
        }
        tf.translation = raft.mean();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 30.0;

    struct Valley {
        map: &'static str,
        zone: Zone,
        obstacles: Obstacles,
        water: Water,
    }

    fn valley(map: &'static str) -> Valley {
        let zone = Zone::load(map);
        let (obstacles, _) = super::super::perches::survey(&zone);
        let water = survey(&zone, &obstacles);
        Valley {
            map,
            zone,
            obstacles,
            water,
        }
    }

    fn valleys() -> Vec<Valley> {
        extraction_core::map::VALLEYS
            .map(|v| v.0)
            .into_iter()
            .map(valley)
            .collect()
    }

    fn rafts_of(v: &Valley, seed: u64) -> Vec<Raft> {
        let mut rng = Rng::new(seed);
        homes(&v.water, v.map, seed)
            .into_iter()
            .map(|k| new_raft(&v.zone, &v.obstacles, &v.water, k, &mut rng))
            .collect()
    }

    fn player(at: Vec3, speed: f32, crouched: bool) -> Threat {
        Threat {
            entity: Entity::PLACEHOLDER,
            at,
            speed,
            kind: ThreatKind::Player { crouched },
        }
    }

    fn tick(
        v: &Valley,
        raft: &mut Raft,
        threats: &[Threat],
        loud: &[(Vec3, f32)],
    ) -> Option<Flush> {
        let ctx = Ctx {
            zone: &v.zone,
            obstacles: &v.obstacles,
            water: &v.water,
            threats,
            loud,
            others: &[],
        };
        raft.tick(&ctx, DT)
    }

    /// Every duck where it may be: on the water, at the surface of water
    /// deep enough and outside anything built; in the air, over the ground
    /// and the water and outside anything built.
    fn check(v: &Valley, raft: &Raft, when: &str) {
        let map = v.map;
        for d in &raft.ducks {
            let p = d.pos.xz();
            assert!(
                !v.obstacles.inside(d.pos + Vec3::Y * 0.1, 0.0),
                "{map}: a duck inside something at {:.1} ({:?}, {when})",
                d.pos,
                d.state
            );
            let (f, _) = floor(&v.zone, p);
            if d.on_water() {
                let s = surface(&v.zone, p).unwrap_or_else(|| {
                    panic!(
                        "{map}: a duck on {:.2} m of water at {:.1} ({:?}, {when})",
                        depth(&v.zone, p),
                        d.pos,
                        d.state
                    )
                });
                assert!(
                    (d.pos.y - s).abs() < 1e-3,
                    "{map}: a swimming duck {:.2} m off the surface ({when})",
                    d.pos.y - s
                );
            } else {
                assert!(
                    d.pos.y >= f.max(v.zone.ground(p.x, p.y)) + 0.29,
                    "{map}: a flying duck under the water or the ground at {:.1} ({when})",
                    d.pos
                );
            }
            assert!(d.vel.length() <= MAX_SPEED + 0.01);
        }
    }

    /// Every valley has open water deep enough for ducks, and one or two
    /// rafts on it; Sallow Reservoir's on its upper arm, among others.
    #[test]
    fn every_valley_with_water_has_rafts() {
        for v in valleys() {
            assert!(
                v.water.spots.len() >= 20,
                "{}: {} places on open water",
                v.map,
                v.water.spots.len()
            );
            let rafts = rafts_of(&v, 1);
            assert!(!rafts.is_empty(), "{}: no rafts", v.map);
            for r in &rafts {
                assert!((RAFT.0..=RAFT.1).contains(&r.ducks.len()));
                check(&v, r, "placed");
            }
            if v.map == "sallow_reservoir" {
                // Over a few seeds, the upper arm's open water (round
                // (-180, -170), its surface 47.6) is the likeliest place.
                let on_arm = (0..8)
                    .filter(|&seed| {
                        rafts_of(&v, seed).iter().any(|r| {
                            let h = r.home;
                            (-250.0..-70.0).contains(&h.x)
                                && (-215.0..-120.0).contains(&h.y)
                                && (surface(&v.zone, h).unwrap() - 47.6).abs() < 0.3
                        })
                    })
                    .count();
                assert!(
                    on_arm >= 6,
                    "a raft on the upper arm in {on_arm} of 8 seeds"
                );
            }
        }
    }

    /// Left alone, and with someone pacing the bank, the ducks paddle about
    /// on deep enough water at its surface for minutes, and stay together.
    #[test]
    fn swimming_ducks_stay_on_deep_water() {
        for v in valleys() {
            for mut raft in rafts_of(&v, 3) {
                let start = raft.home;
                let mut dabbles = 0;
                for step in 0..(240.0 / DT) as usize {
                    // Two minutes alone, then two with someone walking up
                    // and down 18 m off.
                    let threats = if step > (120.0 / DT) as usize {
                        let t = step as f32 * DT;
                        let side = Vec2::from_angle(t * 0.05) * 18.0;
                        let p = raft.mean() + Vec3::new(side.x, 0.0, side.y);
                        vec![player(p, 1.0, false)]
                    } else {
                        vec![]
                    };
                    let was: Vec<bool> = raft.ducks.iter().map(|d| d.dabble > 0.0).collect();
                    assert!(tick(&v, &mut raft, &threats, &[]).is_none(), "{}", v.map);
                    dabbles += raft
                        .ducks
                        .iter()
                        .zip(was)
                        .filter(|(d, w)| d.dabble > 0.0 && !w)
                        .count();
                    check(&v, &raft, "swimming");
                    let m = raft.mean();
                    for d in &raft.ducks {
                        assert!(
                            d.pos.distance(m) < 8.0,
                            "{}: a duck strayed {:.1} m at {:.1}, step {step}",
                            v.map,
                            d.pos.distance(m),
                            d.pos
                        );
                    }
                }
                assert!(dabbles >= 4, "{}: {dabbles} dabbles", v.map);
                assert!(
                    raft.home.distance(start) > 1.0,
                    "{}: never swam off from someone at 18 m",
                    v.map
                );
            }
        }
    }

    /// Walk at a raft and they swim off from you within 25 m, keeping
    /// their distance, and go up when you're 12 m off; run at them and they
    /// go from 25 m; creep crouched and you get nearer. A machine scares
    /// them the same, a shot from further.
    #[test]
    fn they_keep_away_then_flush_at_the_right_distances() {
        for v in valleys() {
            let fresh = || rafts_of(&v, 5).remove(0);
            for (speed, r) in [(1.2, FLUSH), (5.0, WARY)] {
                let mut raft = fresh();
                let c = raft.mean();
                let mut at = c + Vec3::new(45.0, 0.0, 0.0);
                let mut went = None;
                let mut kept = 0.0f32;
                for _ in 0..(200.0 / DT) as usize {
                    // Straight at where they are now.
                    let m = raft.mean();
                    at += (m - at).with_y(0.0).normalize() * speed * DT;
                    let fl = tick(&v, &mut raft, &[player(at, speed, false)], &[]);
                    let d = at.distance(raft.mean());
                    if let Some(f) = fl {
                        assert_eq!(f.cause, FlushCause::Player);
                        assert!(f.alarm, "the first fright raises the alarm");
                        went = Some(d);
                        break;
                    }
                    if d < WARY {
                        kept = kept.max(m.xz().distance(c.xz()));
                    }
                }
                let d = went.unwrap_or_else(|| panic!("{}: never flushed at {speed} m/s", v.map));
                assert!(
                    d <= r + 0.05 && d >= r - speed * DT * 3.0 - 0.6,
                    "{}: up at {d:.1} m, radius {r:.1} ({speed} m/s)",
                    v.map
                );
                if speed < RUN {
                    assert!(kept > 0.5, "{}: they didn't swim off ({kept:.2} m)", v.map);
                }
            }
            // Crouched, 10 m off: they swim off, but stay down; standing
            // up there puts them up.
            let mut raft = fresh();
            let p = raft.mean() + Vec3::new(10.0, 0.0, 0.0);
            for _ in 0..(5.0 / DT) as usize {
                assert!(
                    tick(&v, &mut raft, &[player(p, 0.0, true)], &[]).is_none(),
                    "{}: up from someone crouched at 10 m",
                    v.map
                );
            }
            let mut raft = fresh();
            let p = raft.mean() + Vec3::new(10.0, 0.0, 0.0);
            assert!(tick(&v, &mut raft, &[player(p, 0.0, false)], &[]).is_some());
            // A walker 11 m off: up, but no alarm.
            let mut raft = fresh();
            let walker = Threat {
                entity: Entity::PLACEHOLDER,
                at: raft.mean() + Vec3::new(11.0, 0.0, 0.0),
                speed: 1.5,
                kind: ThreatKind::Walker,
            };
            let f = tick(&v, &mut raft, &[walker], &[]);
            assert!(
                f.is_some_and(|f| f.cause == FlushCause::Robot && !f.alarm),
                "{}: a walker at 11 m",
                v.map
            );
            // A rifle shot 120 m off (heard at 220 m) puts them up.
            let mut raft = fresh();
            let shot = [(
                raft.mean() + Vec3::new(120.0, 0.0, 0.0),
                220.0 * flock::FLUSH_NOISE,
            )];
            let f = tick(&v, &mut raft, &[], &shot);
            assert!(f.is_some_and(|f| f.cause == FlushCause::Noise), "{}", v.map);
            // Standing still 18 m off: they swim off, and never go up.
            let mut raft = fresh();
            let p = raft.mean() + Vec3::new(18.0, 0.0, 0.0);
            for _ in 0..(30.0 / DT) as usize {
                assert!(tick(&v, &mut raft, &[player(p, 0.0, false)], &[]).is_none());
            }
        }
    }

    /// Flush them again and again: they run up along the water, fly off
    /// over it, and come down onto open water well away, never inside
    /// anything built, under the ground or the water, and raise the alarm
    /// at most once a minute.
    #[test]
    fn flushed_ducks_fly_off_and_land_on_water_again() {
        for v in valleys() {
            for mut raft in rafts_of(&v, 7) {
                let mut alarms = Vec::new();
                let mut ran = 0;
                let mut t = 0.0;
                for round in 0..4 {
                    // Walk up to 8 m off them.
                    let m = raft.mean();
                    let at = m + Vec3::new(8.0, 0.0, 0.0);
                    let f = tick(&v, &mut raft, &[player(at, 1.0, false)], &[])
                        .unwrap_or_else(|| panic!("{}: round {round}, not flushed", v.map));
                    if f.alarm {
                        alarms.push(t);
                    }
                    let from = raft.mean();
                    let mut steps = 0;
                    while !raft.afloat() {
                        tick(&v, &mut raft, &[player(at, 0.0, false)], &[]);
                        ran += raft
                            .ducks
                            .iter()
                            .filter(|d| matches!(d.state, DuckState::RunUp { .. }))
                            .count();
                        check(&v, &raft, "flushed");
                        steps += 1;
                        t += DT;
                        assert!(
                            steps < (120.0 / DT) as usize,
                            "{}: round {round}, still up after two minutes",
                            v.map
                        );
                    }
                    // Settled on the water, away from the fright.
                    for _ in 0..(10.0 / DT) as usize {
                        tick(&v, &mut raft, &[], &[]);
                        check(&v, &raft, "landed");
                        t += DT;
                    }
                    let to = raft.mean();
                    assert!(raft.ducks.iter().all(|d| d.state == DuckState::Swim));
                    assert!(
                        to.xz().distance(at.xz()) > SAFE * 0.8,
                        "{}: came down {:.0} m from the fright",
                        v.map,
                        to.xz().distance(at.xz())
                    );
                    assert!(to.distance(from) > 40.0, "{}: barely moved", v.map);
                }
                assert!(ran > 0, "{}: no run-up", v.map);
                for w in alarms.windows(2) {
                    assert!(
                        w[1] - w[0] >= ALARM_EVERY - 0.1,
                        "{}: alarms {alarms:?}",
                        v.map
                    );
                }
                assert!(!alarms.is_empty());
            }
        }
    }

    /// The same seed, the same ducks, the same flight.
    #[test]
    fn ducks_are_deterministic() {
        let v = valley("sallow_reservoir");
        let run = || {
            let mut rafts = rafts_of(&v, 9);
            for i in 0..(60.0 / DT) as usize {
                for r in &mut rafts {
                    let p = r.mean() + Vec3::X * (30.0 - i as f32 * 0.02);
                    tick(&v, r, &[player(p, 1.2, false)], &[]);
                }
            }
            rafts
                .iter()
                .flat_map(|r| r.ducks.iter().map(|d| (d.pos, d.drake)))
                .collect::<Vec<_>>()
        };
        let a = run();
        assert!(!a.is_empty());
        assert_eq!(a, run());
        // A different seed, different places.
        assert_ne!(homes(&v.water, v.map, 1), homes(&v.water, v.map, 2));
    }
}
