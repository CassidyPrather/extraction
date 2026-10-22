//! How robots sense the world: sight and hearing, in one place.
//!
//! **Sight** is an *exposure*: how much of you a robot can make out, from
//! 0 (nothing) to about 1 (stood in the open, close, moving). It is the
//! product of range, fog, light, your posture, the ground cover you're in
//! and whether you're moving ([`exposure`]). Above [`SEEN`] the robot has
//! you: it can aim and fire, and its suspicion climbs fast. Between
//! [`GLIMPSE`] and [`SEEN`] it has caught something — a shape in the fog,
//! grass moving — and its suspicion creeps up, but only to [`GLIMPSE_CAP`]:
//! enough to come and look, never enough to shoot. Below that, nothing.
//! Crouch in grass, in fog, keep still, and a robot thirty metres off
//! looks straight through you.
//!
//! **Hearing**: every sound has a reach in metres for ordinary ears
//! ([`Noise::radius`]): gunshots by calibre (`weapons::report`; robots'
//! own by [`gunshot_reach`]), footsteps
//! by surface, speed and crouch ([`footstep_reach`]), doors, things that
//! land (`items::impacts`), robots at work. What's between the sound and
//! the robot cuts its reach, by the same path the player's ears hear it
//! by (`crate::hearing`): a timber wall to [`OCCLUDED`], each wall or shut
//! door by what it's made of, an open doorway a little. A heard sound tells
//! the robot *where the sound was*, not where you are: it goes there.
//! Quiet sounds only nudge suspicion ([`HEARD_FULL`]), so one footstep
//! behind a walker makes it turn, a few make it look.
//!
//! **Memory**: what a robot believes is in [`Awareness`]: a last-known
//! position and an uncertainty, in metres, that grows while it hasn't
//! seen or heard you ([`UNCERTAINTY_GROWTH`]). For a moment after you
//! break line of sight ([`TRACK_GRACE`]) it still knows which way you
//! went; after that it is guessing, and searches (see `search`).

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::terrain::Surface;

use super::{Awareness, NOTICED, Noise, Noticed, PartRole, Robot, RobotPart, Senses, Stunned};
use crate::player::{Body, Footstep, Player};
use crate::rig::RigHead;
use crate::world::{Layer, Zone};

// --- Sight -------------------------------------------------------------

/// A full-height body (m): posture is measured against it.
pub const STANDING: f32 = 1.75;
/// Below this body height you count as crouched (ground cover hides you).
pub const CROUCHED: f32 = 1.3;
/// The least posture counts for, lying as flat as the body goes.
pub const POSTURE_MIN: f32 = 0.45;
/// Crouched in grass or undergrowth, this much of you shows...
pub const GRASS_HIDE: f32 = 0.35;
/// ...beyond this range (closer, the robot sees into the grass).
pub const GRASS_NEAR: f32 = 4.0;
/// Under trees (forest ground) beyond [`CANOPY_NEAR`], standing or not,
/// this much of you shows through the trunks and the leaves to a machine
/// looking across, not down (one [`CANOPY_ABOVE`] or more above you sees
/// down through them)...
pub const CANOPY_HIDE: f32 = 0.6;
/// ...from this far off (m).
pub const CANOPY_NEAR: f32 = 20.0;
/// A machine this far (m) above you looks down through the trees.
pub const CANOPY_ABOVE: f32 = 8.0;
/// Looked down on from steeper than this (radians), grass hides half as
/// well: a drone overhead sees into it.
pub const GRASS_STEEP: f32 = 0.6;
/// Ground fog: exposure falls as `exp(-density * FOG_SIGHT * distance)`
/// (density per `Conditions::ground_fog`, 0.006 clear to 0.024 thick).
pub const FOG_SIGHT: f32 = 1.6;
/// Keeping still: how much of a moving body's exposure a still one has
/// (robots that see movement more than shape carry their own:
/// [`MotionSight`]).
pub const STILL: f32 = 0.55;

/// This robot sees movement more than shape: a still body shows it only
/// this share of a moving one's exposure (in place of [`STILL`]).
#[derive(Component, Debug, Clone, Copy)]
pub struct MotionSight(pub f32);
/// Running: more than walking.
pub const RUNNING: f32 = 1.35;
/// Light: the sky's (`Conditions::light_at`: a day outdoors 1, dusk 0.36,
/// a room by day 0.3, a clear night 0.02 new moon to 0.08 full, 0.01 under
/// fog) counts as its square root, so dusk leaves 0.6 of the day's
/// exposure, a room 0.55, a full-moon night 0.28, a black one this floor's
/// 0.14: a robot makes out a standing body moving within about 20 m.
pub const NIGHT_LIGHT: f32 = 0.02;
/// Within this, a robot sees you whatever: cone, fog, grass.
pub const POINT_BLANK: f32 = 4.0;
/// Exposure at which a robot has you: aims, fires, suspicion climbs.
pub const SEEN: f32 = 0.12;
/// Exposure at which it has caught something.
pub const GLIMPSE: f32 = 0.045;
/// How far suspicion climbs on glimpses alone.
pub const GLIMPSE_CAP: f32 = 0.5;
/// A glimpse places you this share of the distance out (a guess, as a
/// sound is: [`heard_guess`])...
pub const GLIMPSE_SPREAD: f32 = 0.1;
/// ...and at least this (m). A glimpse of you keeping still doesn't place
/// you at all, once it has a guess: nothing moved.
pub const GLIMPSE_SPREAD_MIN: f32 = 3.0;
/// Suspicion per second while seen, at exposure 1: `1.5 + 30 / distance`.
pub const SIGHT_GAIN: f32 = 1.5;
pub const SIGHT_GAIN_NEAR: f32 = 30.0;
/// Suspicion lost per second with nothing seen.
pub const SUSPICION_DECAY: f32 = 0.05;

// --- Darkness --------------------------------------------------------

/// In the dark (underground: [`Lighting::dark`]) with no light on you,
/// this much of daylight's exposure is left: a few metres at most.
pub const DARK: f32 = 0.08;
/// A lamp lights you fully at its foot, falling to nothing at its range.
pub const LAMP_FALLOFF: f32 = 1.5;
/// A torch you carry lights you up in the dark, fully...
pub const TORCH: f32 = 1.0;
/// ...and its beam pointed at a robot (within this angle, radians) is seen
/// as a light from as far as the beam carries...
pub const TORCH_CONE: f32 = 0.45;
pub const TORCH_REACH: f32 = 45.0;
/// ...with at least this exposure, at the robot's end.
pub const TORCH_GLARE: f32 = 0.6;

/// Where it's dark and what lights it: dark volumes (a centre transform and
/// half extents: the underground sites) and lamps (where, how far they
/// reach). Whoever builds the dark places fills it.
#[derive(Resource, Default, Debug, Clone)]
pub struct Lighting {
    pub dark: Vec<(Transform, Vec3)>,
    pub lamps: Vec<(Vec3, f32)>,
}

impl Lighting {
    pub fn in_dark(&self, p: Vec3) -> bool {
        self.dark.iter().any(|(xf, half)| {
            let l = xf.compute_affine().inverse().transform_point3(p);
            l.x.abs() <= half.x && l.y.abs() <= half.y && l.z.abs() <= half.z
        })
    }

    /// How lit `p` is, 0..1, by the lamps alone (1 outside the dark).
    pub fn lamplight(&self, p: Vec3) -> f32 {
        if !self.in_dark(p) {
            return 1.0;
        }
        self.lamps
            .iter()
            .map(|(at, range)| {
                (1.0 - at.distance(p) / range.max(0.1))
                    .max(0.0)
                    .powf(LAMP_FALLOFF)
            })
            .fold(0.0, f32::max)
    }
}

// --- Hearing -----------------------------------------------------------

/// A wall or a hill in the way: the sound carries this share as far.
pub const OCCLUDED: f32 = 0.45;
/// Sound bends over a low rise: it counts as blocked only by what stands
/// this high over its source (a wall, a hill; not a hump in a field) (m).
pub const HEARD_OVER: f32 = 1.2;
/// A robot that searched somewhere and found nothing pays no heed, this
/// long (s), to sounds nobody made within [`BORED_RADIUS`] m of where it
/// gave up: a lure that keeps going wears out its welcome. (Sounds you
/// make there still count.)
pub const BORED_FOR: f32 = 120.0;
pub const BORED_RADIUS: f32 = 10.0;
/// Sounds with at least this reach (m) raise suspicion fully; quieter ones
/// by their share of it.
pub const HEARD_FULL: f32 = 15.0;
/// Suspicion from a sound: `HEARD_BASE + HEARD_LOUD * loudness` (loudness
/// 1 at the source, 0 at the edge of its reach), scaled as above.
pub const HEARD_BASE: f32 = 0.3;
pub const HEARD_LOUD: f32 = 0.5;
/// Where a sound came from, from afar, is a guess: the uncertainty is
/// this share of the distance (more through a wall)...
pub const HEARD_SPREAD: f32 = 0.2;
/// ...and at least this.
pub const HEARD_SPREAD_MIN: f32 = 1.5;
/// Its guess lands at least this share of the spread off the true spot:
/// it heard roughly where, so a still shooter isn't walked straight onto.
pub const HEARD_MISS: f32 = 0.35;
/// A rifle's report (AK-74: 900 m/s, 48 damage) carries this far.
pub const GUNSHOT: f32 = 220.0;
/// Walking pace footsteps (1.4 m/s) on firm ground carry this far.
pub const FOOTSTEP: f32 = 7.0;
/// A door swinging.
pub const DOOR: f32 = 14.0;
/// Underground, sound follows the corridors: round a corner, it's heard as
/// far as the walk there, this much further than in the open.
pub const CORRIDOR: f32 = 1.3;
/// At most this many corridor walks worked out a tick (each is a path on
/// the site's grid).
const CORRIDOR_BUDGET: usize = 8;

// --- Memory ------------------------------------------------------------

/// Unseen, where you might be spreads at about walking pace (m/s)...
pub const UNCERTAINTY_GROWTH: f32 = 1.2;
/// ...up to this.
pub const UNCERTAINTY_MAX: f32 = 30.0;
/// Just after losing sight it still knows which way you went (s).
pub const TRACK_GRACE: f32 = 1.0;

/// Everything sight depends on, for [`exposure`].
#[derive(Clone, Copy, Debug)]
pub struct SightCase {
    pub distance: f32,
    /// How far the robot's eyes reach today (`Conditions::sight`).
    pub reach: f32,
    pub ground_fog: f32,
    /// The sky's light where you are, against a day outdoors
    /// (`Conditions::light_at`; your torch makes it 1).
    pub light: f32,
    /// The body's height now (`Body::height`).
    pub height: f32,
    /// Horizontal speed, m/s.
    pub speed: f32,
    /// Standing in grass or undergrowth.
    pub cover: bool,
    /// Angle below the horizontal the robot looks down at you, radians.
    pub depression: f32,
    /// In the dark: how lit you are there (0..1: lamps, your torch). None
    /// in daylight.
    pub dark: Option<f32>,
    /// What keeping still leaves of you to this robot ([`STILL`], or its
    /// [`MotionSight`]).
    pub still: f32,
    /// Standing among trees (forest ground): [`CANOPY_HIDE`].
    pub canopy: bool,
}

/// How much of you a robot makes out (see the module docs).
pub fn exposure(c: &SightCase) -> f32 {
    if c.distance > c.reach {
        return 0.0;
    }
    let range = 1.0 - (c.distance / c.reach).powi(2);
    let fog = (-c.ground_fog * FOG_SIGHT * c.distance).exp();
    let light = match c.dark {
        Some(l) => l.clamp(DARK, 1.0),
        None => c.light.clamp(NIGHT_LIGHT, 1.0).sqrt(),
    };
    let height = if c.height > 0.0 { c.height } else { STANDING };
    let posture = (height / STANDING).clamp(POSTURE_MIN, 1.0);
    let hidden = if c.cover && height < CROUCHED && c.distance > GRASS_NEAR {
        if c.depression > GRASS_STEEP {
            (1.0 + GRASS_HIDE) * 0.5
        } else {
            GRASS_HIDE
        }
    } else {
        1.0
    };
    let motion = if c.speed > 3.0 {
        RUNNING
    } else if c.speed > 0.3 {
        1.0
    } else {
        c.still
    };
    // (From above, a drone or a spotter sees down through the trees.)
    let above = c.distance * c.depression.max(0.0).sin();
    let canopy = if c.canopy && c.distance > CANOPY_NEAR && above < CANOPY_ABOVE {
        CANOPY_HIDE
    } else {
        1.0
    };
    let e = range * fog * light * posture * hidden * motion * canopy;
    if c.distance < POINT_BLANK {
        e.max(0.9)
    } else {
        e
    }
}

/// How far a gunshot carries, by its calibre: muzzle energy (speed squared
/// times damage per projectile, times projectiles) against the AK's, to
/// the 0.3 power. A pistol is heard at about 150 m, a Mosin at 250.
pub fn gunshot_reach(speed: f32, damage: f32, projectiles: u32) -> f32 {
    let energy = (speed / 900.0).powi(2) * damage * projectiles.max(1) as f32 / 48.0;
    (GUNSHOT * energy.max(0.01).powf(0.15)).clamp(120.0, 280.0)
}

/// How loud each surface is underfoot, against firm ground.
pub fn surface_loudness(s: Surface) -> f32 {
    match s {
        Surface::Grass => 0.65,
        Surface::Forest => 0.9,
        Surface::Dirt => 0.75,
        Surface::Mud => 0.9,
        Surface::Rock => 1.0,
        Surface::Gravel => 1.45,
        Surface::Asphalt => 1.0,
        Surface::Water => 1.6,
    }
}

/// Built floors (boards, a deck, a stair) are louder than earth.
pub const FLOOR_LOUDNESS: f32 = 1.2;

/// How far a footstep carries: by surface loudness, speed (a run is
/// heard about twice as far as a walk, a creep a third as far) and crouch.
pub fn footstep_reach(loudness: f32, speed: f32, height: f32) -> f32 {
    let pace = (speed / 1.4).clamp(0.3, 2.6).powf(0.8);
    let crouch = if height > 0.0 && height < CROUCHED {
        0.6
    } else {
        1.0
    };
    FOOTSTEP * loudness * pace * crouch
}

/// Something heard this tick.
#[derive(Clone, Copy, Debug)]
struct Sound {
    at: Vec3,
    radius: f32,
    source: Option<Entity>,
}

/// A door swinging open or shut is heard: whoever's next to it did it.
pub(crate) fn door_noise(
    doors: Query<(Entity, &crate::world::buildings::Door, &GlobalTransform)>,
    players: Query<(Entity, &GlobalTransform), With<Player>>,
    mut was_open: Local<std::collections::HashMap<Entity, bool>>,
    mut noise: MessageWriter<Noise>,
) {
    // Broken doors come back as new entities after the Surge.
    was_open.retain(|e, _| doors.contains(*e));
    for (e, door, gt) in &doors {
        let before = was_open.insert(e, door.open);
        if before.is_none_or(|b| b == door.open) {
            continue;
        }
        let at = gt.translation();
        // A robot opening it isn't news; a player is.
        if let Some((p, _)) = players
            .iter()
            .find(|(_, pg)| pg.translation().distance(at) < 3.0)
        {
            noise.write(Noise {
                at,
                radius: DOOR,
                source: Some(p),
            });
        }
    }
}

/// Sight and hearing, into each robot's [`Awareness`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn perceive(
    mut robots: Query<(
        Entity,
        &Robot,
        &Senses,
        &GlobalTransform,
        &mut Awareness,
        Has<Stunned>,
        Option<&super::indoor::LocalNav>,
        Option<&MotionSight>,
        Has<super::indoor::Dormant>,
        Option<&MindSeed>,
    )>,
    players: Query<
        (
            Entity,
            &GlobalTransform,
            &Body,
            Option<&crate::health::Health>,
        ),
        With<Player>,
    >,
    torches: Query<(&crate::gadgets::Device, &GlobalTransform), With<crate::gadgets::Powered>>,
    (lighting, hearing, cloaks, conditions, zone, entrances, hushes, glints, starlight): (
        Res<Lighting>,
        Option<Res<crate::hearing::Hearing>>,
        Res<crate::gadgets::Cloaks>,
        Res<crate::world::weather::Conditions>,
        Res<Zone>,
        Res<super::doors::Entrances>,
        Option<Res<crate::anomaly::hush::Hushes>>,
        Option<Res<crate::weapons::optics::Glints>>,
        Option<Res<crate::weapons::flares::Starlight>>,
    ),
    heads: Query<(&ChildOf, &GlobalTransform), With<RigHead>>,
    loads: Query<(&crate::interact::Held, &LinearVelocity), With<crate::interact::carry::Bulky>>,
    sensors: Query<&RobotPart>,
    parts_of: Query<&Children>,
    mut noises: MessageReader<Noise>,
    mut steps: MessageReader<Footstep>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut noticed: MessageWriter<Noticed>,
    (mut gave_up, mut bored): (
        MessageReader<super::search::LostYou>,
        Local<std::collections::HashMap<Entity, Vec<(Vec3, f32)>>>,
    ),
) {
    let dt = time.delta_secs();
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    // Where each robot looked and found nothing lately: sounds nobody made
    // there (a tape playing, a hum, a flare's crackle) it stops answering.
    let now = time.elapsed_secs();
    for g in gave_up.read() {
        bored
            .entry(g.robot)
            .or_default()
            .push((g.searched, now + BORED_FOR));
    }
    bored.retain(|e, spots| {
        spots.retain(|(_, until)| *until > now);
        !spots.is_empty() && robots.contains(*e)
    });
    let mut sounds: Vec<Sound> = noises
        .read()
        .map(|n| Sound {
            at: n.at,
            radius: n.radius,
            source: n.source,
        })
        .collect();
    // Footsteps: by what's underfoot, how fast, how low.
    for s in steps.read() {
        let Ok((_, _, body, _)) = players.get(s.entity) else {
            continue;
        };
        let p = [s.position.x, s.position.z];
        let loud = if entrances.building_at(s.position + Vec3::Y * 0.5).is_some()
            || lighting.in_dark(s.position + Vec3::Y * 0.5)
        {
            FLOOR_LOUDNESS
        } else {
            surface_loudness(zone.hm.surface(p, zone.forest_cover(p[0], p[1]) > 0.5))
        };
        sounds.push(Sound {
            at: s.position + Vec3::Y * 0.1,
            radius: footstep_reach(loud, s.speed, body.height),
            source: Some(s.entity),
        });
    }
    // A Hush eats every sound made in it (`anomaly::hush`).
    if let Some(h) = &hushes {
        sounds.retain(|s| !h.eats(s.at));
    }
    // Where each player's eyes are, and what they stand in.
    struct Seen {
        e: Entity,
        feet: Vec3,
        head: Vec3,
        height: f32,
        speed: f32,
        cover: bool,
        /// Among trees (`SightCase::canopy`).
        canopy: bool,
        /// The sky's light on you (`SightCase::light`).
        light: f32,
        /// In the dark: how lit (see `SightCase::dark`).
        dark: Option<f32>,
        /// A torch on: where, and which way its beam goes.
        torch: Option<(Vec3, Vec3)>,
        /// A bulky load in your hands: how fast it's moving
        /// (`signs::LOAD_SHOWS`).
        load: Option<f32>,
    }
    let targets: Vec<Seen> = players
        .iter()
        // A player who's down isn't a target (`search::forget_the_downed`).
        .filter(|(_, gt, _, health)| {
            !cloaks.hides(gt.translation()) && !health.is_some_and(|h| h.down)
        })
        .map(|(e, gt, body, _)| {
            let feet = gt.translation();
            let height = if body.height > 0.0 {
                body.height
            } else {
                STANDING
            };
            // The rig's head, unless nothing drives it (it rests at the
            // feet): then the top of the body.
            let head = heads
                .iter()
                .find(|(c, _)| c.parent() == e)
                .map(|(_, h)| h.translation())
                .filter(|h| h.y > feet.y + 0.5)
                .unwrap_or(feet + Vec3::Y * (height - 0.15));
            let p = [feet.x, feet.z];
            let indoors = entrances.building_at(feet + Vec3::Y * 0.5).is_some();
            let surface = zone.hm.surface(p, zone.forest_cover(p[0], p[1]) > 0.5);
            // A powered light on you (in a hand, on your gun, at your head):
            // within reach of your body, from the feet to over your head.
            let torch = torches
                .iter()
                .filter(|(d, _)| d.kind == crate::gadgets::DeviceKind::Light)
                .find(|(_, g)| {
                    let p = g.translation();
                    Vec2::new(p.x - feet.x, p.z - feet.z).length() < 1.0
                        && p.y > feet.y - 0.2
                        && p.y < feet.y + STANDING + 0.5
                })
                .map(|(_, g)| (g.translation(), g.forward().as_vec3()));
            let dark = lighting.in_dark(feet + Vec3::Y * 0.5).then(|| {
                let lamps = lighting.lamplight(feet + Vec3::Y * 0.5);
                if torch.is_some() {
                    TORCH.max(lamps)
                } else {
                    lamps
                }
            });
            Seen {
                e,
                feet,
                head,
                height,
                speed: Vec2::new(body.velocity.x, body.velocity.z).length(),
                cover: !indoors
                    && dark.is_none()
                    && matches!(surface, Surface::Grass | Surface::Forest),
                canopy: !indoors && zone.forest_cover(p[0], p[1]) > 0.5,
                // Your torch lights you when the sky doesn't; so does a
                // white star overhead (`weapons::flares`).
                light: if torch.is_some() {
                    conditions.light_at(indoors).max(TORCH)
                } else {
                    conditions.light_at(indoors)
                }
                .max(starlight.as_ref().map_or(0.0, |s| s.at(head))),
                dark,
                torch,
                load: loads
                    .iter()
                    .filter(|(h, _)| h.player == e)
                    .map(|(_, v)| v.0.length())
                    .reduce(f32::max),
            }
        })
        .collect();
    let mut corridor_budget = CORRIDOR_BUDGET;
    for (robot_e, robot, senses, gt, mut aware, stunned, local, motion, dormant, mind_seed) in
        &mut robots
    {
        let mind_seed = mind_seed.map_or(robot_e.to_bits(), |m| m.0);
        aware.news = false;
        // Asleep: only a loud noise reaches it (`indoor::wake_residents`).
        if robot.wrecked || dormant {
            aware.can_see_target = false;
            continue;
        }
        if stunned {
            // Senseless: it can't see you, so it can't shoot you.
            aware.can_see_target = false;
            aware.exposure = 0.0;
            continue;
        }
        // Blind if every sensor part is gone.
        let has_eyes = parts_of
            .get(robot_e)
            .map(|c| {
                let mut any = false;
                let mut alive = false;
                for p in c.iter() {
                    if let Ok(part) = sensors.get(p) {
                        if part.role == PartRole::Sensor {
                            any = true;
                            alive |= part.alive();
                        }
                    }
                }
                !any || alive
            })
            .unwrap_or(true);
        let eye = gt.transform_point(senses.eye);
        let forward = gt.forward().as_vec3();
        let reach = conditions.sight(senses.sight);
        let was = aware.suspicion;
        aware.can_see_target = false;
        let mut best = 0.0f32;
        if has_eyes {
            for t in &targets {
                let to = t.head - eye;
                let dist = to.length();
                // A scope pointed this way into a low sun flashes in its
                // eyes (`weapons::optics`), further than it sees a body.
                let glint = glints.as_ref().map_or(0.0, |g| g.seen_by(t.e, eye));
                if (dist > reach && glint < GLIMPSE) || dist < 0.01 {
                    continue;
                }
                let angle = forward.angle_between(to / dist).to_degrees();
                if angle > senses.fov_deg * 0.5 && dist > POINT_BLANK {
                    continue;
                }
                let blocked = Dir3::new(to)
                    .ok()
                    .and_then(|dir| spatial.cast_ray(eye, dir, dist - 0.3, true, &world));
                if blocked.is_some() {
                    continue;
                }
                let mut e = exposure(&SightCase {
                    distance: dist,
                    reach,
                    ground_fog: conditions.ground_fog,
                    light: t.light,
                    height: t.height,
                    // A swinging load isn't keeping still.
                    speed: match t.load {
                        Some(v) if v > super::signs::LOAD_SWINGING => t.speed.max(v),
                        _ => t.speed,
                    },
                    cover: t.cover,
                    depression: (-to.y / dist).clamp(-1.0, 1.0).asin(),
                    dark: t.dark,
                    still: motion.map_or(STILL, |m| m.0),
                    canopy: t.canopy,
                });
                // A load in your hands is more of you to see.
                if t.load.is_some() && dist < reach {
                    e *= super::signs::LOAD_SHOWS;
                }
                // In the dark (underground, or a dim sky), a torch pointed
                // this way is a light in its eyes.
                let dim = t.dark.is_some() || conditions.light_at(false) < 0.5;
                if let (true, Some((at, beam))) = (dim, t.torch) {
                    let back = (eye - at).normalize_or_zero();
                    if beam.angle_between(back) < TORCH_CONE && dist < TORCH_REACH {
                        e = e.max(TORCH_GLARE * (1.0 - dist / TORCH_REACH).max(0.25));
                    }
                }
                e = e.max(glint);
                if e < GLIMPSE {
                    continue;
                }
                best = best.max(e);
                let gain = dt * (SIGHT_GAIN + SIGHT_GAIN_NEAR / dist.max(1.0)) * e;
                aware.news = true;
                if e >= SEEN {
                    aware.can_see_target = true;
                    aware.target = Some(t.e);
                    aware.last_known = Some(t.feet);
                    aware.uncertainty = 0.5;
                    aware.unseen = 0.0;
                    aware.suspicion = (aware.suspicion + gain).min(1.0);
                } else {
                    // Something over there.
                    if aware.target.is_none() {
                        aware.target = Some(t.e);
                    }
                    // If it moved (or flashed), roughly there; a still
                    // shape it can't place, beyond what it already had.
                    let moved = t.speed > 0.3 || glint >= GLIMPSE;
                    if !aware.can_see_target && (moved || aware.last_known.is_none()) {
                        let spread = (dist * GLIMPSE_SPREAD).max(GLIMPSE_SPREAD_MIN);
                        aware.last_known = Some(heard_guess(t.feet, spread, mind_seed));
                        aware.uncertainty = spread;
                    }
                    if aware.suspicion < GLIMPSE_CAP {
                        aware.suspicion = (aware.suspicion + gain).min(GLIMPSE_CAP);
                    }
                }
            }
        }
        aware.exposure = best;
        if !aware.can_see_target {
            aware.unseen += dt;
            aware.suspicion = (aware.suspicion - dt * SUSPICION_DECAY).max(0.0);
            // It saw which way you went; then it's guessing.
            let tracked = aware
                .target
                .and_then(|p| targets.iter().find(|t| t.e == p))
                .filter(|_| aware.unseen < TRACK_GRACE && aware.uncertainty < 1.0);
            if let Some(t) = tracked {
                aware.last_known = Some(t.feet);
            } else {
                aware.uncertainty =
                    (aware.uncertainty + dt * UNCERTAINTY_GROWTH).min(UNCERTAINTY_MAX);
            }
        }
        let ear = gt.translation();
        for s in &sounds {
            if s.source == Some(robot_e) {
                continue;
            }
            let dull = s.source.is_none()
                && bored.get(&robot_e).is_some_and(|spots| {
                    spots.iter().any(|(at, _)| at.distance(s.at) < BORED_RADIUS)
                });
            if dull {
                continue;
            }
            let d = s.at.distance(ear);
            let mut reach = s.radius * senses.hearing * conditions.hearing();
            // On a site of its own, round corners it hears as far as the walk.
            let site = local.filter(|l| l.contains(s.at) && l.contains(ear));
            if d >= reach * if site.is_some() { CORRIDOR } else { 1.0 } {
                continue;
            }
            // Through walls and shut doors, up through floors and rock,
            // over a hill it carries less far; round an open doorway a
            // little less far. The same path the player's ears hear it by
            // (`hearing`, a table read): a sound that's quieter by `db`
            // carries `10^(db/20)` as far.
            let blocked = |a: Vec3, b: Vec3| crate::acoustics::blocked(&spatial, a, b);
            let (occ, through_wall) = hearing
                .as_deref()
                .map_or((crate::acoustics::CLEAR, false), |h| {
                    h.heard(Some(&zone), s.at, ear, Some(&blocked))
                });
            let occluded = occ.db < 0.0;
            let mut heard_over = d;
            if occluded {
                let walk = site
                    .filter(|_| through_wall && corridor_budget > 0)
                    .and_then(|l| {
                        corridor_budget -= 1;
                        super::indoor::NavRef::Local(l).walk_length(s.at, ear)
                    });
                match walk {
                    Some(len) => {
                        reach *= CORRIDOR;
                        heard_over = len;
                    }
                    None => reach *= 10f32.powf(occ.db / 20.0),
                }
                if heard_over >= reach {
                    continue;
                }
            }
            let loud = 1.0 - heard_over / reach;
            let share = (s.radius / HEARD_FULL).min(1.0);
            aware.suspicion = (aware.suspicion + (HEARD_BASE + loud * HEARD_LOUD) * share).min(1.0);
            // Heard, not seen: they know roughly where, not exactly.
            if !aware.can_see_target {
                let spread =
                    (d * HEARD_SPREAD * if occluded { 1.5 } else { 1.0 }).max(HEARD_SPREAD_MIN);
                aware.last_known = Some(heard_guess(s.at, spread, mind_seed));
                aware.uncertainty = spread;
                aware.news = true;
                if let Some(p) = s.source.filter(|s| players.contains(*s)) {
                    aware.target = Some(p);
                }
            }
        }
        if was < NOTICED && aware.suspicion >= NOTICED {
            noticed.write(Noticed {
                robot: robot_e,
                at: eye,
            });
        }
    }
}

/// A robot's own seed for its guesses and its aim's scatter: the one
/// seed it has (`RobotSeed`, from its kind and spawn point), so it doesn't
/// change with how many other things exist (entity ids do).
pub type MindSeed = super::RobotSeed;

/// splitmix64 of `a` and `b`.
pub fn mix(a: u64, b: u64) -> u64 {
    let mut h = a ^ b.rotate_left(29);
    h = h.wrapping_add(0x9e37_79b9_7f4a_7c15);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

/// A seed for a shot `robot` fires now: its [`MindSeed`] and the (fixed)
/// clock, so it doesn't hang on which robot a system happened to visit
/// first.
pub fn shot_seed(minds: &Query<&MindSeed>, robot: Entity, time: &Time) -> u64 {
    let own = minds.get(robot).map_or(robot.to_bits(), |m| m.0);
    mix(own, (time.elapsed_secs_f64() * 1000.0) as u64)
}

/// Where a robot guesses a sound at `at` came from, `spread` out: a point
/// [`HEARD_MISS`]..1 of the spread off it, the same for the same robot and
/// spot (so a second shot from there doesn't walk its guess about) but
/// different between robots (`seed`: its [`MindSeed`]).
pub fn heard_guess(at: Vec3, spread: f32, seed: u64) -> Vec3 {
    let h = mix(
        seed,
        (at.x.to_bits() as u64) | ((at.z.to_bits() as u64) << 32),
    );
    let angle = (h & 0xffff) as f32 / 65536.0 * std::f32::consts::TAU;
    let r = spread * (HEARD_MISS + (1.0 - HEARD_MISS) * ((h >> 16) & 0xffff) as f32 / 65536.0);
    at + Vec3::new(angle.cos(), 0.0, angle.sin()) * r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Among trees, far off, you show less, standing or not; close by,
    /// trees don't hide you.
    #[test]
    fn trees_hide_you_from_afar() {
        let open = exposure(&case());
        let trees = exposure(&SightCase {
            canopy: true,
            ..case()
        });
        assert!(
            (trees - open * CANOPY_HIDE).abs() < 1e-4,
            "{open} -> {trees}"
        );
        let near = SightCase {
            distance: CANOPY_NEAR - 2.0,
            ..case()
        };
        assert_eq!(
            exposure(&near),
            exposure(&SightCase {
                canopy: true,
                ..near
            })
        );
        // From above, through the leaves.
        let above = SightCase {
            depression: (CANOPY_ABOVE / 30.0 + 0.05).asin(),
            ..case()
        };
        assert_eq!(
            exposure(&above),
            exposure(&SightCase {
                canopy: true,
                ..above
            })
        );
    }

    fn case() -> SightCase {
        SightCase {
            distance: 30.0,
            reach: 75.0,
            ground_fog: 0.011,
            light: 1.0,
            height: STANDING,
            speed: 1.4,
            cover: true,
            depression: 0.05,
            dark: None,
            still: STILL,
            canopy: false,
        }
    }

    /// Stood walking in the open at 30 m on a drizzly day, you're seen;
    /// crouched still in the grass, in a pea-souper, you're not even
    /// glimpsed. Crouching alone, or fog alone, isn't enough.
    #[test]
    fn crouch_grass_and_fog_hide_you() {
        let open = exposure(&case());
        assert!(open > SEEN * 2.0, "walking in the open: {open}");
        let hidden = exposure(&SightCase {
            height: 0.95,
            speed: 0.0,
            ground_fog: 0.024,
            ..case()
        });
        assert!(hidden < GLIMPSE, "crouched in grass in fog: {hidden}");
        let crouched = exposure(&SightCase {
            height: 0.95,
            cover: false,
            ..case()
        });
        assert!(crouched > GLIMPSE, "crouched on a road: {crouched}");
        // A drone straight overhead sees into the grass.
        let above = exposure(&SightCase {
            height: 0.95,
            speed: 0.0,
            depression: 1.2,
            ..case()
        });
        let level = exposure(&SightCase {
            height: 0.95,
            speed: 0.0,
            ..case()
        });
        assert!(above > level * 1.5, "{above} vs {level}");
        // Point blank, nothing hides you.
        let near = exposure(&SightCase {
            distance: 3.0,
            height: 0.95,
            speed: 0.0,
            ground_fog: 0.024,
            ..case()
        });
        assert!(near >= SEEN);
    }

    #[test]
    fn calibre_and_footing_set_how_far_you_are_heard() {
        let pistol = gunshot_reach(315.0, 34.0, 1);
        let ak = gunshot_reach(900.0, 48.0, 1);
        let mosin = gunshot_reach(865.0, 110.0, 1);
        let shotgun = gunshot_reach(400.0, 14.0, 9);
        assert!((ak - GUNSHOT).abs() < 1.0, "{ak}");
        assert!(pistol < shotgun && shotgun < ak && ak < mosin);
        assert!(pistol > 150.0, "a pistol still gets reported: {pistol}");
        let walk = footstep_reach(surface_loudness(Surface::Asphalt), 1.4, STANDING);
        let run = footstep_reach(surface_loudness(Surface::Gravel), 4.5, STANDING);
        let creep = footstep_reach(surface_loudness(Surface::Grass), 0.7, 1.0);
        assert!((walk - FOOTSTEP).abs() < 0.01);
        assert!(run > walk * 2.5, "{run}");
        assert!(creep < 2.5, "{creep}");
    }
}
