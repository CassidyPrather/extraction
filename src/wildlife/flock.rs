//! Rooks and crows: flocks that sit along the tops of buildings and masts
//! and go up all at once when something comes near.
//!
//! A flock is one entity holding its birds ([`Flock::birds`]): it's
//! [`Phase::Settled`] on a [`PerchSite`], and something too close — you
//! (how close depends on how fast you move and whether you're crouched:
//! [`flush_radius`]), a machine, a shot within [`FLUSH_NOISE`] of its reach
//! — flushes it ([`Flushed`]). The birds burst up off the roof, the ones
//! nearest the threat first, wheel round overhead for a while
//! ([`Phase::Circling`]), then fly off to another building well away from
//! whatever scared them ([`Phase::Moving`]) and come down onto it one by
//! one ([`Phase::Landing`]). Left alone they move on now and then of their
//! own accord ([`FlushCause::Restless`]), unhurried, and at night they
//! roost.
//!
//! A flock you flush tells the machines: the noise of it carries
//! [`ALARM_REACH`] metres, and a robot that hears it comes to look where
//! the birds were (`robots::senses`: a [`crate::robots::Noise`] whose
//! source is you). At most once every [`ALARM_EVERY`] seconds a flock.
//!
//! Flight is a few boids rules a bird: head for the flock's goal (a point
//! wheeling round the circle, or the next site), keep apart, keep with the
//! others, keep off the ground and out of anything built ([`Obstacles`]).
//! Cost: a few hundred distance checks a flock a tick, no allocation.

use bevy::prelude::*;
use extraction_core::noise::Rng;

use super::perches::{Obstacles, PerchSite, Perches};
use super::{FlushCause, Flushed, Threat, ThreatKind, Threats};
use crate::world::Zone;

// --- What scares them ----------------------------------------------------

/// A player standing still this close flushes a flock (m)...
pub const FLUSH_STILL: f32 = 6.0;
/// ...and this much further for every m/s they're moving...
pub const FLUSH_PER_SPEED: f32 = 6.5;
/// ...up to this (a flat-out sprint).
pub const FLUSH_MAX: f32 = 38.0;
/// Crouched, you get this much closer.
pub const FLUSH_CROUCHED: f32 = 0.6;
/// Machines: a walker or hound, a drone (loud rotors), the airship.
pub const FLUSH_WALKER: f32 = 22.0;
pub const FLUSH_DRONE: f32 = 30.0;
pub const FLUSH_DIRIGIBLE: f32 = 70.0;
/// A loud noise (one heard at least [`LOUD`] m off: a shot, a blast)
/// flushes every flock within this share of its reach.
pub const FLUSH_NOISE: f32 = 0.6;
pub const LOUD: f32 = 100.0;
/// After a fright, a flock is warier for a while: this much further.
pub const WARY: f32 = 1.3;
pub const WARY_FOR: f32 = 90.0;
/// At night they roost: this much closer before they go.
pub const ROOSTING: f32 = 0.6;

/// A flock a player flushes is heard this far by robots...
pub const ALARM_REACH: f32 = 60.0;
/// ...at most once a flock in this many seconds, from this far above
/// where it sat (they're up in a second or two).
pub const ALARM_EVERY: f32 = 60.0;
pub const ALARM_HEIGHT: f32 = 6.0;

// --- Where they go -------------------------------------------------------

/// They come down no nearer any threat than this.
pub const SAFE: f32 = 45.0;
/// The next building is this far from the last, give or take.
pub const HOP_MIN: f32 = 60.0;
pub const HOP_MAX: f32 = 280.0;
/// A staged scene's perches are picked this many times as often.
pub const SCENE_PULL: f32 = 6.0;
/// Wheeling overhead after a fright, seconds.
pub const CIRCLE_FOR: (f32, f32) = (10.0, 22.0);
/// Left alone, they move on after this long, seconds.
pub const RESTLESS: (f32, f32) = (180.0, 540.0);

// --- How they fly --------------------------------------------------------

pub const CRUISE: f32 = 9.0;
pub const MAX_SPEED: f32 = 13.0;
pub const MIN_SPEED: f32 = 3.0;
pub const ACCEL: f32 = 16.0;
/// Flying, they keep this far above the ground.
pub const CLEARANCE: f32 = 8.0;
/// Flying, they keep this far over the top of anything built near them.
pub const OVER: f32 = 3.0;
/// Close enough to its perch to sit.
pub const SIT: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BirdState {
    /// On perch `n` of the flock's site.
    Sitting(u16),
    Flying,
    /// Coming down onto perch `n` of the site it's landing on.
    Landing(u16),
    /// Shot: it falls, and lies where it fell (`bodies`).
    Dead,
    /// Its carcass was taken away: nothing left to draw.
    Gone,
}

impl BirdState {
    pub fn alive(self) -> bool {
        !matches!(self, BirdState::Dead | BirdState::Gone)
    }
}

#[derive(Clone, Debug)]
pub struct Bird {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Which way it faces sitting (radians about Y, from +X).
    pub yaw: f32,
    pub state: BirdState,
    /// How hard it's working its wings, 0 (gliding, sitting) to 1
    /// (bursting up): for the wingbeat and its sound.
    pub flap: f32,
    /// Seconds before it reacts (a flock doesn't lift as one).
    delay: f32,
    /// Its own place in the flock, around the flock's goal.
    slot: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Settled,
    Circling { left: f32 },
    Moving { to: usize },
    Landing { to: usize },
}

/// What kind of bird: each valley has its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Species {
    /// Rooks: big, loud flocks (Marrow Creek, the yard).
    Rook,
    /// Ravens: few, big, high (Kettle Pass).
    Raven,
    /// Jackdaws: small, quick, chattering (Cinder Works).
    Jackdaw,
    /// Alpine choughs: black, quick, in tumbling flocks over the fell (Rime
    /// Fell; the rook's model, smaller and blacker, a yellow bill).
    Chough,
    /// A grey heron, alone, standing in the shallows (Sallow Reservoir):
    /// it perches only on wading spots, and it's wary.
    Heron,
}

impl Species {
    pub fn for_map(map: &str) -> Self {
        match map {
            "kettle_pass" => Species::Raven,
            "cinder_works" => Species::Jackdaw,
            "rime_fell" => Species::Chough,
            _ => Species::Rook,
        }
    }

    /// Birds in a flock, at least and at most.
    fn flock_size(self) -> (usize, usize) {
        match self {
            Species::Rook => (10, 20),
            Species::Raven => (4, 8),
            Species::Jackdaw => (10, 18),
            Species::Chough => (8, 16),
            Species::Heron => (1, 1),
        }
    }

    /// Stands in water (`PerchSite::wading`), not on roofs.
    pub fn wading(self) -> bool {
        self == Species::Heron
    }

    /// How much further off it goes up than a rook.
    pub fn wariness(self) -> f32 {
        if self == Species::Heron { 1.6 } else { 1.0 }
    }

    /// Body length against a rook's (0.45 m).
    pub fn size(self) -> f32 {
        match self {
            Species::Rook => 1.0,
            Species::Raven => 1.35,
            Species::Jackdaw => 0.78,
            Species::Chough => 0.85,
            // Its own model, at its own size.
            Species::Heron => 1.0,
        }
    }
}

#[derive(Component, Clone, Debug)]
pub struct Flock {
    pub species: Species,
    /// The perch site it's on (or was last on).
    pub site: usize,
    pub phase: Phase,
    pub birds: Vec<Bird>,
    /// The circle it wheels round: centre, radius, height, and where on it
    /// the flock's goal is (radians).
    orbit: (Vec3, f32, f32),
    angle: f32,
    rng: Rng,
    /// Seconds until it moves on of its own accord.
    restless: f32,
    /// Seconds it stays wary.
    pub wary: f32,
    /// Seconds before it can raise the alarm again.
    alarm_cool: f32,
    /// Tries at finding somewhere to go, this circling.
    tries: u8,
}

impl Flock {
    pub fn sitting(&self) -> usize {
        self.birds
            .iter()
            .filter(|b| matches!(b.state, BirdState::Sitting(_)))
            .count()
    }

    /// Birds still alive.
    pub fn alive(&self) -> usize {
        self.birds.iter().filter(|b| b.state.alive()).count()
    }

    /// Where the living birds are, on average (where the flock was, if
    /// none are).
    pub fn mean(&self) -> Vec3 {
        let alive: Vec<Vec3> = self
            .birds
            .iter()
            .filter(|b| b.state.alive())
            .map(|b| b.pos)
            .collect();
        if alive.is_empty() {
            return self.orbit.0;
        }
        alive.iter().sum::<Vec3>() / alive.len() as f32
    }
}

/// How close a threat may come before this flock goes up.
pub fn flush_radius(t: &Threat) -> f32 {
    match t.kind {
        ThreatKind::Player { crouched } => {
            let r = (FLUSH_STILL + FLUSH_PER_SPEED * t.speed).min(FLUSH_MAX);
            if crouched { r * FLUSH_CROUCHED } else { r }
        }
        ThreatKind::Walker => FLUSH_WALKER,
        ThreatKind::Drone => FLUSH_DRONE,
        ThreatKind::Dirigible => FLUSH_DIRIGIBLE,
    }
}

/// Flocks for a valley: a few, spread over its buildings, each on a site
/// of its own. Returns how many.
/// `map` is the map's id (`marrow_creek`), as `LaunchOptions::map`.
pub fn spawn_flocks(commands: &mut Commands, map: &str, perches: &Perches, seed: u64) -> usize {
    let species = Species::for_map(map);
    let count = match map {
        "training_yard" => 1,
        _ => (perches.sites.len() / 5).clamp(2, 5),
    };
    let mut rng = Rng::new(seed ^ 0xB12D_5EED ^ name_hash(map));
    let mut taken: Vec<usize> = Vec::new();
    // Nowhere to perch (a map with nothing built): no flocks.
    let count = if perches.sites.iter().any(|s| !s.wading) {
        count
    } else {
        0
    };
    for _ in 0..count {
        // Far from the flocks already placed: the best of a few tries.
        let pick = (0..8)
            .map(|_| rng.below(perches.sites.len()))
            .filter(|&i| {
                !taken.contains(&i)
                    && !perches.sites[i].wading
                    && perches.sites[i].spots.len() >= species.flock_size().0
            })
            .max_by(|&a, &b| {
                let far = |i: usize| {
                    taken
                        .iter()
                        .map(|&t| perches.sites[t].centre.distance(perches.sites[i].centre))
                        .fold(f32::MAX, f32::min)
                };
                far(a).total_cmp(&far(b))
            });
        let Some(site) = pick else {
            continue;
        };
        taken.push(site);
        let flock = new_flock(species, site, &perches.sites[site], &mut rng);
        info!(
            "a flock of {} {species:?}s on the {} at {:.0}",
            flock.birds.len(),
            perches.sites[site].prefab,
            perches.sites[site].centre
        );
        commands.spawn((
            Name::new("Flock"),
            Transform::from_translation(perches.sites[site].centre),
            flock,
        ));
    }
    // Sallow Reservoir's heron, on a wading spot of its own.
    let mut herons = 0;
    if map == "sallow_reservoir" {
        let wading: Vec<usize> = (0..perches.sites.len())
            .filter(|&i| perches.sites[i].wading)
            .collect();
        if !wading.is_empty() {
            let site = wading[rng.below(wading.len())];
            let flock = new_flock(Species::Heron, site, &perches.sites[site], &mut rng);
            info!(
                "a heron in the shallows at {:.0}",
                perches.sites[site].centre
            );
            commands.spawn((
                Name::new("Heron"),
                Transform::from_translation(perches.sites[site].centre),
                flock,
            ));
            herons = 1;
        }
    }
    taken.len() + herons
}

pub(crate) fn name_hash(s: &str) -> u64 {
    s.bytes().fold(0xCBF2_9CE4_8422_2325, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01B3)
    })
}

pub(crate) fn new_flock(species: Species, site: usize, at: &PerchSite, rng: &mut Rng) -> Flock {
    let (lo, hi) = species.flock_size();
    let n = (lo + rng.below(hi - lo + 1)).min(at.spots.len());
    let spots = shuffled(at.spots.len(), rng);
    let birds = spots[..n]
        .iter()
        .map(|&s| {
            let (p, yaw) = at.spots[s];
            Bird {
                pos: p,
                vel: Vec3::ZERO,
                yaw: yaw
                    + if rng.chance(0.5) {
                        std::f32::consts::PI
                    } else {
                        0.0
                    },
                state: BirdState::Sitting(s as u16),
                flap: 0.0,
                delay: 0.0,
                slot: Vec3::new(
                    rng.range(-1.0, 1.0),
                    rng.range(-0.5, 0.5),
                    rng.range(-1.0, 1.0),
                ) * 5.0,
            }
        })
        .collect();
    let mut seed = Rng::new(rng.next_u64());
    Flock {
        species,
        site,
        phase: Phase::Settled,
        birds,
        orbit: (at.centre, 20.0, at.top() + 12.0),
        angle: 0.0,
        // `EXTRACTION_WILDLIFE=restless`: they move on a second in (to see
        // them fly in a screenshot).
        restless: if std::env::var("EXTRACTION_WILDLIFE").is_ok_and(|v| v == "restless") {
            1.0
        } else {
            seed.range(RESTLESS.0, RESTLESS.1)
        },
        rng: seed,
        wary: 0.0,
        alarm_cool: 0.0,
        tries: 0,
    }
}

fn shuffled(n: usize, rng: &mut Rng) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        v.swap(i, rng.below(i + 1));
    }
    v
}

/// What a flock's tick needs to know about the valley.
pub struct Ctx<'a> {
    pub perches: &'a Perches,
    pub obstacles: &'a Obstacles,
    pub zone: &'a Zone,
    pub threats: &'a [Threat],
    /// Loud noises this tick: where, and how near scares a flock.
    pub loud: &'a [(Vec3, f32)],
    /// Sites flocks are on or headed for.
    pub occupied: &'a [usize],
    pub night: bool,
}

/// A flock went up this tick: why, and who did it.
#[derive(Clone, Copy, Debug)]
pub struct Flush {
    pub at: Vec3,
    pub cause: FlushCause,
    pub by: Option<Entity>,
    pub alarm: bool,
}

impl Flock {
    /// One tick: frights, decisions and flight.
    pub fn tick(&mut self, ctx: &Ctx, dt: f32) -> Option<Flush> {
        // Every bird shot: only the falling to see to.
        if self.alive() == 0 {
            fly(self, ctx.perches, ctx.obstacles, ctx.zone, dt);
            return None;
        }
        self.wary = (self.wary - dt).max(0.0);
        self.alarm_cool = (self.alarm_cool - dt).max(0.0);
        let mut flushed = None;
        // What's near the birds, if they're on (or coming down to) a roof.
        let near = match self.phase {
            Phase::Settled => Some(ctx.perches.sites[self.site].centre),
            Phase::Landing { to } => Some(ctx.perches.sites[to].centre),
            _ => None,
        };
        if let Some(at) = near {
            let scale = if self.wary > 0.0 { WARY } else { 1.0 }
                * if ctx.night { ROOSTING } else { 1.0 }
                * self.species.wariness();
            let threat = ctx
                .threats
                .iter()
                .filter(|t| t.at.distance(at) < flush_radius(t) * scale)
                .min_by(|a, b| a.at.distance(at).total_cmp(&b.at.distance(at)));
            if let Some(t) = threat {
                let cause = match t.kind {
                    ThreatKind::Player { .. } => FlushCause::Player,
                    _ => FlushCause::Robot,
                };
                flushed = Some(self.flush(ctx.perches, cause, Some((t.at, t.entity))));
            } else if let Some(&(p, _)) = ctx.loud.iter().find(|(p, r)| p.distance(at) < *r) {
                flushed = Some(self.flush(
                    ctx.perches,
                    FlushCause::Noise,
                    Some((p, Entity::PLACEHOLDER)),
                ));
            }
        }
        if self.phase == Phase::Settled && !ctx.night {
            self.restless -= dt;
            if self.restless <= 0.0 {
                flushed = Some(self.flush(ctx.perches, FlushCause::Restless, None));
            }
        }
        decide(self, ctx.perches, ctx.threats, ctx.occupied, dt);
        fly(self, ctx.perches, ctx.obstacles, ctx.zone, dt);
        flushed
    }
}

/// The flocks' tick, for every flock.
#[allow(clippy::too_many_arguments)]
pub(crate) fn flocks(
    mut flocks: Query<(Entity, &mut Flock, &mut Transform)>,
    perches: Option<Res<Perches>>,
    obstacles: Option<Res<Obstacles>>,
    threats: Res<Threats>,
    zone: Res<Zone>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    mut noises: MessageReader<crate::robots::Noise>,
    mut flushed: MessageWriter<Flushed>,
    time: Res<Time>,
) {
    let (Some(perches), Some(obstacles)) = (perches, obstacles) else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 || perches.sites.is_empty() {
        return;
    }
    let loud: Vec<(Vec3, f32)> = noises
        .read()
        .filter(|n| n.radius >= LOUD)
        .map(|n| (n.at, n.radius * FLUSH_NOISE))
        .collect();
    let occupied: Vec<usize> = flocks.iter().map(|(_, f, _)| f.heading_for()).collect();
    let ctx = Ctx {
        perches: &perches,
        obstacles: &obstacles,
        zone: &zone,
        threats: &threats.0,
        loud: &loud,
        occupied: &occupied,
        night: conditions
            .as_ref()
            .is_some_and(|c| c.hour == crate::world::weather::Hour::Night),
    };
    for (entity, mut flock, mut tf) in &mut flocks {
        if let Some(f) = flock.tick(&ctx, dt) {
            debug!("a flock went up at {:.0} ({:?})", f.at, f.cause);
            flushed.write(Flushed {
                flock: entity,
                at: f.at,
                birds: flock.birds.len() as u32,
                cause: f.cause,
                by: f.by,
                alarm: f.alarm,
            });
        }
        tf.translation = flock.mean();
    }
}

impl Flock {
    /// The site it's on, or headed for.
    pub fn heading_for(&self) -> usize {
        match self.phase {
            Phase::Moving { to } | Phase::Landing { to } => to,
            _ => self.site,
        }
    }

    /// Up they go: from something at `from` (and who it was), or of their
    /// own accord.
    fn flush(
        &mut self,
        perches: &Perches,
        cause: FlushCause,
        from: Option<(Vec3, Entity)>,
    ) -> Flush {
        let at = match self.phase {
            Phase::Landing { to } => perches.sites[to].centre,
            _ => perches.sites[self.site].centre,
        };
        if let Phase::Landing { to } = self.phase {
            self.site = to;
        }
        let calm = cause == FlushCause::Restless;
        let top = perches.sites[self.site].top();
        let rng = &mut self.rng;
        self.phase = Phase::Circling {
            left: if calm {
                rng.range(3.0, 7.0)
            } else {
                rng.range(CIRCLE_FOR.0, CIRCLE_FOR.1)
            },
        };
        self.orbit = (at, rng.range(14.0, 26.0), top + rng.range(8.0, 16.0));
        self.angle = rng.range(0.0, std::f32::consts::TAU);
        self.tries = 0;
        if !calm {
            self.wary = WARY_FOR;
        }
        for b in &mut self.birds {
            // Coming in, it goes round with the rest (not on down onto the
            // roof they're leaving).
            if matches!(b.state, BirdState::Landing(_)) {
                b.state = BirdState::Flying;
            }
            if matches!(b.state, BirdState::Sitting(_)) {
                // Nearest the threat go first; calm, they drift off over a
                // few seconds.
                b.delay = match from {
                    Some((p, _)) if !calm => {
                        (b.pos.distance(p) * 0.02).min(0.4) + rng.range(0.0, 0.35)
                    }
                    _ => rng.range(0.0, 4.0),
                };
            }
        }
        let alarm = cause == FlushCause::Player && self.alarm_cool <= 0.0;
        if alarm {
            self.alarm_cool = ALARM_EVERY;
        }
        self.restless = rng.range(RESTLESS.0, RESTLESS.1);
        Flush {
            at,
            cause,
            by: from
                .map(|(_, e)| e)
                .filter(|_| matches!(cause, FlushCause::Player | FlushCause::Robot)),
            alarm,
        }
    }
}

/// Where next: the flock's own decisions, between frights.
fn decide(flock: &mut Flock, perches: &Perches, threats: &[Threat], occupied: &[usize], dt: f32) {
    match flock.phase {
        Phase::Settled => {}
        Phase::Circling { left } => {
            let left = left - dt;
            if left > 0.0 {
                flock.phase = Phase::Circling { left };
                return;
            }
            match next_site(flock, perches, threats, occupied) {
                Some(to) => flock.phase = Phase::Moving { to },
                None => {
                    flock.tries = flock.tries.saturating_add(1);
                    flock.phase = Phase::Circling { left: 6.0 };
                }
            }
        }
        Phase::Moving { to } => {
            let site = &perches.sites[to];
            // Something there now: go round again and pick another.
            if threats
                .iter()
                .any(|t| t.at.distance(site.centre) < SAFE * 0.8)
            {
                let at = flock.mean();
                flock.orbit = (at.with_y(0.0), 18.0, flock.orbit.2.max(at.y));
                flock.phase = Phase::Circling { left: 4.0 };
                return;
            }
            let m = flock.mean();
            if Vec2::new(m.x - site.centre.x, m.z - site.centre.z).length() < 35.0 {
                // Each bird to a perch of its own.
                let spots = shuffled(site.spots.len(), &mut flock.rng);
                for (b, &s) in flock
                    .birds
                    .iter_mut()
                    .filter(|b| b.state.alive())
                    .zip(spots.iter())
                {
                    b.state = BirdState::Landing(s as u16);
                    b.delay = flock.rng.range(0.0, 2.5);
                }
                flock.phase = Phase::Landing { to };
            }
        }
        Phase::Landing { to } => {
            if flock.sitting() == flock.alive() {
                flock.site = to;
                flock.phase = Phase::Settled;
            }
        }
    }
}

/// Another building, well away from what scared them and from the other
/// flocks, not too far; after a few failed looks, anywhere quiet.
fn next_site(
    flock: &mut Flock,
    perches: &Perches,
    threats: &[Threat],
    occupied: &[usize],
) -> Option<usize> {
    let from = flock.orbit.0;
    let fits = |i: usize, loose: bool| {
        let s = &perches.sites[i];
        let d = s.centre.distance(from);
        i != flock.site
            && s.wading == flock.species.wading()
            && !occupied.contains(&i)
            && s.spots.len() >= flock.birds.len()
            && (loose || (HOP_MIN..=HOP_MAX).contains(&d))
            && threats.iter().all(|t| t.at.distance(s.centre) > SAFE)
    };
    let loose = flock.tries >= 3;
    let choices: Vec<usize> = (0..perches.sites.len())
        .filter(|&i| fits(i, loose))
        .collect();
    if choices.is_empty() {
        return None;
    }
    // Nearer ones more often.
    let weights: Vec<f32> = choices
        .iter()
        .map(|&i| {
            let s = &perches.sites[i];
            // Story's set pieces (a row of buses over an empty bay) more
            // often than not.
            let pull = if s.scene { SCENE_PULL } else { 1.0 };
            pull / (s.centre.distance(from) + 40.0)
        })
        .collect();
    flock.rng.weighted(&weights).map(|k| choices[k])
}

/// The birds' flight: boids round the flock's goal, landing and sitting.
fn fly(flock: &mut Flock, perches: &Perches, obstacles: &Obstacles, zone: &Zone, dt: f32) {
    // The flock's goal.
    let (centre, radius, height) = flock.orbit;
    let goal = match flock.phase {
        Phase::Circling { .. } | Phase::Settled => {
            flock.angle += CRUISE / radius * dt * 0.8;
            let g = centre + Vec3::new(flock.angle.cos(), 0.0, flock.angle.sin()) * radius;
            g.with_y(height.max(zone.ground(g.x, g.z) + CLEARANCE * 2.0))
        }
        Phase::Moving { to } | Phase::Landing { to } => {
            let s = &perches.sites[to];
            s.centre
                .with_y((s.top() + 10.0).max(zone.ground(s.centre.x, s.centre.z) + 18.0))
        }
    };
    let landing_site = match flock.phase {
        Phase::Landing { to } => Some(to),
        _ => None,
    };
    let site = landing_site.unwrap_or(flock.site);
    let n = flock.birds.len();
    // Neighbours' pull and push, from the last tick's positions.
    let snapshot: Vec<(Vec3, Vec3, bool)> = flock
        .birds
        .iter()
        .map(|b| {
            (
                b.pos,
                b.vel,
                matches!(b.state, BirdState::Flying | BirdState::Landing(_)),
            )
        })
        .collect();
    for i in 0..n {
        let b = &mut flock.birds[i];
        match b.state {
            // Falling to whatever's under it, and lying there.
            BirdState::Dead => {
                let floor = obstacles
                    .below(b.pos + Vec3::Y * 0.05)
                    .unwrap_or(f32::MIN)
                    .max(zone.ground(b.pos.x, b.pos.z));
                if b.pos.y > floor + 0.01 {
                    b.vel = (b.vel + Vec3::NEG_Y * 9.8 * dt) * (1.0 - dt * 0.5);
                    b.pos += b.vel * dt;
                    if b.pos.y <= floor {
                        b.pos.y = floor;
                        b.vel = Vec3::ZERO;
                    }
                } else {
                    b.pos.y = floor;
                    b.vel = Vec3::ZERO;
                }
                b.flap = 0.0;
                continue;
            }
            BirdState::Gone => continue,
            // A perch to come down on only while the flock's landing.
            BirdState::Landing(_) if landing_site.is_none() => b.state = BirdState::Flying,
            _ => {}
        }
        if b.delay > 0.0 {
            b.delay -= dt;
            if matches!(b.state, BirdState::Sitting(_)) {
                continue;
            }
        }
        match b.state {
            BirdState::Sitting(s) => {
                // Flushed (or moving on): off the perch, up first.
                if flock.phase != Phase::Settled && landing_site.is_none() {
                    let out = Vec3::new(b.yaw.cos(), 0.0, b.yaw.sin());
                    b.vel = Vec3::Y * 4.0 + out * 1.5 + b.slot * 0.15;
                    // A hop off its feet.
                    b.pos += Vec3::Y * 0.15;
                    b.state = BirdState::Flying;
                    b.flap = 1.0;
                } else {
                    let (p, _) = perches.sites[site].spots[s as usize];
                    b.pos = p;
                    b.vel = Vec3::ZERO;
                    b.flap = 0.0;
                    // Now and then one turns round.
                    if flock.rng.chance(dt / 9.0) {
                        b.yaw += std::f32::consts::PI;
                    }
                }
                continue;
            }
            BirdState::Landing(s) if b.delay <= 0.0 => {
                let (spot, yaw) = perches.sites[site].spots[s as usize];
                let d = spot - b.pos;
                let dh = Vec2::new(d.x, d.z).length();
                if d.length() < SIT {
                    b.pos = spot;
                    b.vel = Vec3::ZERO;
                    b.yaw = yaw
                        + if flock.rng.chance(0.5) {
                            std::f32::consts::PI
                        } else {
                            0.0
                        };
                    b.state = BirdState::Sitting(s);
                    b.flap = 0.0;
                    continue;
                }
                // In from above, slowing as it comes; over anything built
                // on the way (a chimney between it and its roof).
                let mut aim = spot + Vec3::Y * (dh * 0.45).min(10.0);
                if dh > 4.0 {
                    if let Some(top) = obstacles.top_near(b.pos, 4.0) {
                        aim.y = aim.y.max(top + OVER);
                    }
                }
                let to = aim - b.pos;
                let speed = (to.length() * 1.2).clamp(1.0, CRUISE);
                let want = to.normalize_or_zero() * speed;
                let mut acc = ((want - b.vel) * 4.0).clamp_length_max(ACCEL);
                // Over anything in the way (a chimney between it and its
                // roof), not into it: until it's nearly there.
                if d.length() > 2.0 && obstacles.inside(b.pos + b.vel * 0.6, 0.8) {
                    acc.y += ACCEL * 1.5;
                }
                b.vel = (b.vel + acc * dt).clamp_length_max(MAX_SPEED);
                b.flap = if b.vel.y > -0.5 || dh < 3.0 {
                    0.8
                } else {
                    0.15
                };
                let prev = b.pos;
                b.pos += b.vel * dt;
                keep_clear(b, prev, zone, obstacles, true);
                continue;
            }
            _ => {}
        }
        // Flying (or waiting its turn to come in): boids round the goal.
        let mut sep = Vec3::ZERO;
        let mut align = Vec3::ZERO;
        let mut k = 0.0;
        for (j, &(p, v, flying)) in snapshot.iter().enumerate() {
            if j == i || !flying {
                continue;
            }
            let off = b.pos - p;
            let d = off.length();
            if d < 1.6 && d > 1e-4 {
                sep += off / d * (1.6 - d);
            }
            if d < 6.0 {
                align += v;
                k += 1.0;
            }
        }
        let target = goal + b.slot;
        let to = target - b.pos;
        let want = to.normalize_or_zero() * CRUISE * (to.length() / 6.0).clamp(0.6, 1.25);
        let mut acc = (want - b.vel) * 2.2 + sep * 9.0;
        if k > 0.0 {
            acc += (align / k - b.vel) * 0.6;
        }
        // Keep up off the ground and over anything built near, and out of
        // buildings ahead.
        let floor = (zone.ground(b.pos.x, b.pos.z) + CLEARANCE).max(
            obstacles
                .top_near(b.pos, 4.0)
                .map_or(f32::MIN, |t| t + OVER),
        );
        if b.pos.y < floor {
            acc.y += (floor - b.pos.y) * 5.0;
        }
        if obstacles.inside(b.pos + b.vel * 0.6, 0.8) {
            acc.y += ACCEL * 1.5;
        }
        let acc = acc.clamp_length_max(ACCEL * 1.6);
        b.vel += acc * dt;
        let speed = b.vel.length();
        b.vel = b.vel.normalize_or_zero() * speed.clamp(MIN_SPEED, MAX_SPEED);
        // Wings: hard climbing and speeding up, easy gliding round.
        let work = (acc.y.max(0.0) / ACCEL + acc.length() / (ACCEL * 2.0)).min(1.0);
        b.flap += (work.max(0.25) - b.flap) * (dt * 2.0).min(1.0);
        let prev = b.pos;
        b.pos += b.vel * dt;
        keep_clear(b, prev, zone, obstacles, false);
    }
}

/// Never under the ground or inside anything built: back out and up.
fn keep_clear(b: &mut Bird, prev: Vec3, zone: &Zone, obstacles: &Obstacles, landing: bool) {
    // Coming down, it may go as low as its perch: a heron's is the bed of
    // the shallows.
    let off = if landing { 0.0 } else { 0.4 };
    let g = zone.ground(b.pos.x, b.pos.z) + off;
    if b.pos.y < g {
        b.pos.y = g;
        b.vel.y = b.vel.y.max(if landing { 0.0 } else { 2.0 });
    }
    if !obstacles.inside(b.pos, 0.02) {
        return;
    }
    let clear = |p: Vec3| !obstacles.inside(p, 0.02) && p.y >= zone.ground(p.x, p.z) + off - 1e-3;
    // Along the face, not stopped dead at it: what of the step is clear
    // (up a dam's face with the roost beyond it, not pinned to it).
    let step = b.pos - prev;
    for keep in [Vec3::new(0.0, 1.0, 1.0), Vec3::new(1.0, 1.0, 0.0), Vec3::Y] {
        let p = prev + step * keep;
        if step.dot(keep) != 0.0 && clear(p) {
            b.pos = p;
            b.vel *= keep;
            b.vel.y = b.vel.y.max(1.0);
            return;
        }
    }
    // Hemmed in (under a lip on a wall's face): out sideways first (out
    // from under it, not down under it again), else the nearest way; no
    // longer going into it.
    for flat in [true, false] {
        let out = obstacles.push_out(b.pos, 0.02, flat);
        if clear(out) {
            let n = (out - b.pos).normalize_or_zero();
            b.vel -= n * b.vel.dot(n).min(0.0);
            b.pos = out;
            return;
        }
    }
    b.pos = prev;
    let mut lift = 0;
    while obstacles.inside(b.pos, 0.02) && lift < 80 {
        b.pos.y += 0.25;
        lift += 1;
    }
    b.vel = Vec3::new(b.vel.x * 0.3, b.vel.y.max(1.5), b.vel.z * 0.3);
}

/// A flock a player flushed is heard by the machines: a noise at the
/// flock, from that player, as far as [`ALARM_REACH`] (it comes to look
/// where the birds were, not where you are).
pub(crate) fn alarm_robots(
    mut flushed: MessageReader<Flushed>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    for f in flushed.read().filter(|f| f.alarm) {
        // From over the roof, where the birds are going up (not from its
        // surface, which would muffle it): heard over walls and rises.
        noise.write(crate::robots::Noise {
            at: f.at + Vec3::Y * ALARM_HEIGHT,
            radius: ALARM_REACH,
            source: f.by,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valley(map: &str) -> (Zone, Obstacles, Perches) {
        let zone = Zone::load(map);
        let (o, p) = super::super::perches::survey(&zone);
        (zone, o, p)
    }

    fn player(at: Vec3, speed: f32, crouched: bool) -> Threat {
        Threat {
            entity: Entity::PLACEHOLDER,
            at,
            speed,
            kind: ThreatKind::Player { crouched },
        }
    }

    fn flocks_on(perches: &Perches, n: usize, seed: u64) -> Vec<Flock> {
        let mut rng = Rng::new(seed);
        let species = Species::Rook;
        let mut out: Vec<Flock> = Vec::new();
        let mut i = 0;
        while out.len() < n && i < perches.sites.len() {
            if perches.sites[i].spots.len() >= 10 {
                out.push(new_flock(species, i, &perches.sites[i], &mut rng));
            }
            i += 3;
        }
        out
    }

    const DT: f32 = 1.0 / 30.0;

    fn tick_all(
        flocks: &mut [Flock],
        (zone, obstacles, perches): (&Zone, &Obstacles, &Perches),
        threats: &[Threat],
        loud: &[(Vec3, f32)],
    ) -> Vec<Flush> {
        let occupied: Vec<usize> = flocks.iter().map(|f| f.heading_for()).collect();
        let ctx = Ctx {
            perches,
            obstacles,
            zone,
            threats,
            loud,
            occupied: &occupied,
            night: false,
        };
        flocks.iter_mut().filter_map(|f| f.tick(&ctx, DT)).collect()
    }

    #[test]
    fn moving_faster_flushes_them_from_further() {
        let at = Vec3::ZERO;
        let still = flush_radius(&player(at, 0.0, false));
        let creep = flush_radius(&player(at, 0.6, true));
        let walk = flush_radius(&player(at, 1.4, false));
        let sprint = flush_radius(&player(at, 5.0, false));
        assert!(
            creep < still && still < walk && walk < sprint,
            "{creep} {still} {walk} {sprint}"
        );
        assert!(sprint >= 30.0 && sprint <= FLUSH_MAX);
        assert!(creep < 8.0, "a crouched creep gets close: {creep}");
    }

    /// Walk straight at a flock and it goes up when you're within the walk's
    /// flush radius (not before, and not later than a tick's walk after);
    /// sprint and it goes up from 30 m; creep crouched and you can get to
    /// within 8 m. In every valley.
    #[test]
    fn a_flock_flushes_when_you_come_near_and_not_before() {
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let (zone, obstacles, perches) = valley(map);
            for (speed, crouched) in [(1.4, false), (5.0, false), (0.6, true)] {
                let mut flocks = flocks_on(&perches, 1, 7);
                let centre = perches.sites[flocks[0].site].centre;
                let r = flush_radius(&player(centre, speed, crouched));
                // From 80 m out, at the perches' height, straight in.
                let mut at = centre + Vec3::new(80.0, 0.0, 0.0);
                let mut went = None;
                for _ in 0..(200.0 / DT) as usize {
                    at.x -= speed * DT;
                    let fl = tick_all(
                        &mut flocks,
                        (&zone, &obstacles, &perches),
                        &[player(at, speed, crouched)],
                        &[],
                    );
                    if let Some(f) = fl.first() {
                        assert_eq!(f.cause, FlushCause::Player);
                        assert!(f.alarm, "the first fright raises the alarm");
                        went = Some(at.distance(centre));
                        break;
                    }
                }
                let d = went.unwrap_or_else(|| panic!("{map}: never flushed at {speed} m/s"));
                assert!(
                    d <= r && d >= r - speed * DT * 2.0,
                    "{map}: flushed at {d:.1} m, radius {r:.1} ({speed} m/s)"
                );
            }
            // A walker 20 m off flushes them too, a machine's fright.
            let mut flocks = flocks_on(&perches, 1, 7);
            let centre = perches.sites[flocks[0].site].centre;
            let walker = Threat {
                entity: Entity::PLACEHOLDER,
                at: centre + Vec3::new(20.0, 0.0, 0.0),
                speed: 1.5,
                kind: ThreatKind::Walker,
            };
            let fl = tick_all(&mut flocks, (&zone, &obstacles, &perches), &[walker], &[]);
            assert!(
                fl.first()
                    .is_some_and(|f| f.cause == FlushCause::Robot && !f.alarm),
                "{map}: a walker at 20 m"
            );
            // A rifle shot 120 m away flushes them; a footstep doesn't count.
            let mut flocks = flocks_on(&perches, 1, 7);
            let shot = [(centre + Vec3::new(120.0, 0.0, 0.0), 220.0 * FLUSH_NOISE)];
            let fl = tick_all(&mut flocks, (&zone, &obstacles, &perches), &[], &shot);
            assert!(fl.first().is_some_and(|f| f.cause == FlushCause::Noise));
        }
    }

    /// Long raids with someone tramping from flock to flock, shooting now
    /// and then: no bird is ever inside anything built or under the ground,
    /// none flies faster than it can, and once left alone every flock comes
    /// down onto a roof again.
    #[test]
    fn birds_never_go_inside_walls_or_ground_and_settle_again() {
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let (zone, obstacles, perches) = valley(map);
            let mut flocks = flocks_on(&perches, 4, 11);
            assert!(flocks.len() >= 3, "{map}");
            let mut rng = Rng::new(3);
            let mut at = perches.sites[0].centre.with_y(0.0);
            let mut flushes = 0;
            let minutes = 8.0;
            let steps = (minutes * 60.0 / DT) as usize;
            for step in 0..steps + (150.0 / DT) as usize {
                let busy = step < steps;
                if step == steps {
                    // Left alone now: the flocks up must come down (and none
                    // moves on of its own accord in the meantime).
                    for f in &mut flocks {
                        f.restless = f32::INFINITY;
                    }
                }
                // Off to a random flock's site, walking or running.
                let goal = perches.sites[flocks[(step / 900) % flocks.len()].heading_for()].centre;
                let speed = if (step / 300) % 3 == 0 { 5.0 } else { 1.4 };
                let to = (goal - at).with_y(0.0);
                if to.length() > 1.0 {
                    at += to.normalize() * speed * DT;
                }
                at.y = zone.ground(at.x, at.z);
                let threats = if busy {
                    vec![player(at, speed, false)]
                } else {
                    Vec::new()
                };
                let loud = if busy && rng.chance(DT / 40.0) {
                    vec![(at, 220.0 * FLUSH_NOISE)]
                } else {
                    Vec::new()
                };
                flushes +=
                    tick_all(&mut flocks, (&zone, &obstacles, &perches), &threats, &loud).len();
                for (k, f) in flocks.iter().enumerate() {
                    for b in &f.birds {
                        let g = zone.ground(b.pos.x, b.pos.z);
                        let probe = match b.state {
                            BirdState::Sitting(_) => b.pos + Vec3::Y * 0.12,
                            _ => b.pos,
                        };
                        assert!(
                            !obstacles.inside(probe, 0.0),
                            "{map}: flock {k}'s bird inside something at {:.2} ({:?}, {:?})",
                            b.pos,
                            b.state,
                            f.phase
                        );
                        assert!(
                            b.pos.y >= g + 0.3,
                            "{map}: a bird underground at {:.2}",
                            b.pos
                        );
                        assert!(b.vel.length() <= MAX_SPEED + 0.01);
                    }
                }
            }
            assert!(
                flushes >= 6,
                "{map}: only {flushes} flushes in {minutes} minutes"
            );
            for (k, f) in flocks.iter().enumerate() {
                assert_eq!(f.phase, Phase::Settled, "{map}: flock {k} still up");
                assert_eq!(f.sitting(), f.birds.len());
            }
        }
    }

    /// The same seed, the same flight, bird for bird.
    #[test]
    fn flight_is_deterministic() {
        let (zone, obstacles, perches) = valley("marrow_creek");
        let run = || {
            let mut flocks = flocks_on(&perches, 2, 5);
            let centre = perches.sites[flocks[0].site].centre;
            for i in 0..(40.0 / DT) as usize {
                let t = [player(
                    centre + Vec3::X * (30.0 - i as f32 * 0.05),
                    1.4,
                    false,
                )];
                tick_all(&mut flocks, (&zone, &obstacles, &perches), &t, &[]);
            }
            flocks
                .iter()
                .flat_map(|f| f.birds.iter().map(|b| b.pos))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}

#[cfg(test)]
mod empty_tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;

    /// A map with nothing built (a scratch map) has no flocks, rather than
    /// a crash.
    #[test]
    fn no_perches_no_flocks() {
        let world = World::new();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        for map in ["marrow_creek", "sallow_reservoir", "nowhere"] {
            assert_eq!(spawn_flocks(&mut commands, map, &Perches::default(), 1), 0);
        }
    }
}

#[cfg(test)]
mod heron_tests {
    use super::*;

    /// Sallow Reservoir has shallows for its heron. Walk at it and it goes
    /// up from further than a rook would; it only ever comes down in the
    /// shallows, never inside anything built or under the bed.
    #[test]
    fn the_heron_keeps_to_the_shallows_and_is_wary() {
        let zone = Zone::load("sallow_reservoir");
        let (obstacles, mut perches) = crate::wildlife::perches::survey(&zone);
        let wading = crate::wildlife::perches::wading(&zone);
        assert!(wading.len() >= 4, "{} shallows", wading.len());
        let first = perches.sites.len();
        perches.sites.extend(wading);
        let mut rng = Rng::new(2);
        let mut heron = new_flock(Species::Heron, first, &perches.sites[first], &mut rng);
        let centre = perches.sites[first].centre;
        let walk = |at: Vec3| Threat {
            entity: Entity::PLACEHOLDER,
            at,
            speed: 1.4,
            kind: ThreatKind::Player { crouched: false },
        };
        let mut at = centre + Vec3::new(60.0, 0.0, 0.0);
        let mut went = None;
        let mut landings = 0;
        for step in 0..(240.0 * 30.0) as usize {
            if went.is_none() {
                at.x -= 1.4 / 30.0;
            }
            let threats = if step < 2400 { vec![walk(at)] } else { vec![] };
            let ctx = Ctx {
                perches: &perches,
                obstacles: &obstacles,
                zone: &zone,
                threats: &threats,
                loud: &[],
                occupied: &[],
                night: false,
            };
            let was = heron.phase;
            if heron.tick(&ctx, 1.0 / 30.0).is_some() && went.is_none() {
                went = Some(at.distance(centre));
            }
            if heron.phase == Phase::Settled && was != Phase::Settled {
                landings += 1;
                assert!(
                    perches.sites[heron.site].wading,
                    "came down on a {}",
                    perches.sites[heron.site].prefab
                );
            }
            let b = &heron.birds[0];
            assert!(
                !obstacles.inside(b.pos + Vec3::Y * 0.3, 0.0),
                "inside something at {:.1}",
                b.pos
            );
            assert!(
                b.pos.y >= zone.ground(b.pos.x, b.pos.z) - 0.01,
                "under the bed"
            );
        }
        let d = went.expect("never went up");
        let rook = flush_radius(&walk(Vec3::ZERO));
        assert!(d > rook * 1.4, "went up at {d:.1} m; a rook at {rook:.1}");
        assert!(landings >= 1, "never came down again");
    }
}
