//! The quadcopter: four rotors, a sensor eye, a gun pod.
//!
//! Flight is physical. A position controller asks for an acceleration, an
//! attitude controller turns that into a torque, and a mixer splits thrust
//! and torque across the four rotors, each pushing at its own position.
//! Shoot a rotor off and the mixer can't balance any more: the drone
//! tumbles out of the sky on its own, no scripted death needed.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::search::{self, Step};
use super::{
    Awareness, Brain, Intent, Job, PartDestroyed, PartRole, Robot, RobotKind, RobotPart, Senses,
    part_bundle,
};
use crate::weapons::ballistics::Shot;
use crate::world::Zone;

/// Rotor arm length (x and z), metres.
pub const ARM: f32 = 0.32;
pub const MASS: f32 = 6.0;
pub const MAX_ROTOR_N: f32 = MASS * 9.81 / 4.0 * 2.2;

pub const ROTORS: [Vec3; 4] = [
    Vec3::new(ARM, 0.02, ARM),
    Vec3::new(-ARM, 0.02, ARM),
    Vec3::new(-ARM, 0.02, -ARM),
    Vec3::new(ARM, 0.02, -ARM),
];

/// The body (`flight`) flies it toward `goal`; see
/// [`super::Intent`] for where it looks and how hard it pushes.
#[derive(Component, Debug)]
#[require(super::flight::Flight)]
pub struct Drone {
    /// Thrust per rotor this tick, newtons (for visuals and sound).
    pub thrust: [f32; 4],
    pub rotor_alive: [bool; 4],
    pub gun_cooldown: f32,
    pub burst_left: u32,
    /// Seconds spent near the ground without lift: it has crashed.
    pub grounded: f32,
    /// Where the controller is trying to be.
    pub goal: Vec3,
    /// Orbit phase while working or engaging.
    pub phase: f32,
    pub looted: bool,
    /// Seconds it has held a firing solution on its target: it fires only
    /// once this reaches [`LOCK_TIME`].
    pub lock: f32,
    /// Locks on the one it's fighting that no round of it has hit since
    /// ([`RELOCKS`])...
    pub locks_unhit: u8,
    /// ...when this fight began (s, `Time::elapsed`; below 0 before one),
    /// and when it last was in it (a search between is the same fight)...
    pub engaged_at: f32,
    pub last_engaged: f32,
    /// ...and, broken off, till when it leaves fighting alone.
    pub broke_off_until: f32,
    /// Seconds locked on without a shot (its gun swung off you, a pause):
    /// past [`RETELL`] it locks on afresh, chirp and all, before it fires
    /// again (`lose_a_quiet_lock`).
    pub quiet: f32,
    /// Rounds of its warning burst left: set at a lock after a lull
    /// ([`WARN_AFTER`]), the first burst after it kicks up the ground short
    /// of you and hurts no one ([`WARNING_SHORT`])...
    pub warning_left: u32,
    /// ...and when it last fired (s, `Time::elapsed`).
    pub last_shot: f32,
}

/// A lock after this long without firing (or its first) warns before it
/// hurts (s). In a fight it's already in, a lock lost and found again
/// between bursts doesn't start the warning over (or every burst of a drone
/// that dodges about between them would be one).
pub const WARN_AFTER: f32 = 10.0;

/// The first burst after a lock that comes after a lull ([`WARN_AFTER`]) is
/// a warning, as the sentry's is: its
/// rounds land this far short of your feet (m), wider, and do no harm. A
/// newcomer hears the chirp and sees the dirt kick up, and the first round
/// that hurts comes no sooner than [`LOCK_TIME`] + [`BURST_GAP`] after the
/// chirp (two bursts, 54 hp, landed in 2 s on a newcomer who only moved
/// when the first one hit: Onboarding's first_jobs_cautious).
pub const WARNING_SHORT: f32 = 2.5;
/// Rounds in a burst, and each one's harm (hp).
pub const BURST: u32 = 3;
pub const DAMAGE: f32 = 9.0;

/// Whether someone standing at `feet` is under a roof, out of any
/// drone's reach (`drone_guns`; `drone_brain` sends one going home off
/// them by the same test).
pub fn under_a_roof(entrances: &super::doors::Entrances, feet: Vec3) -> bool {
    entrances.building_at(feet + Vec3::Y * 0.5).is_some()
}

/// How long a locked drone may go without firing before its next burst
/// needs a fresh lock (and its chirp): longer than a burst's gap. One that
/// went quiet 24 s and then fired again, with nothing new to see or hear,
/// hit a newcomer four times unwarned (Onboarding's first_jobs_reckless).
pub const RETELL: f32 = 4.0;

/// Locked on and not firing: past [`RETELL`] the lock goes, so the next
/// burst comes after a fresh chirp. Whether it went.
pub fn lose_a_quiet_lock(d: &mut Drone, dt: f32) -> bool {
    d.quiet += dt;
    if d.quiet > RETELL {
        d.lock = 0.0;
        d.quiet = 0.0;
        true
    } else {
        false
    }
}

/// A fight it isn't winning it gives up: after this many locks without a
/// round in, or [`FIGHT_FOR`] seconds, it breaks off back to its job, and
/// leaves fighting alone for [`BROKEN_OFF`] (a drone that kept re-locking a
/// newcomer hiding from it, every 24 s for three minutes, till it had
/// them, read as unfair: Robot minds' fairness runs, 1 November).
pub const RELOCKS: u8 = 3;
pub const FIGHT_FOR: f32 = 60.0;
pub const BROKEN_OFF: f32 = 60.0;
/// Out of a fight this long (s), the next one's a new one.
pub const FIGHT_GAP: f32 = 20.0;

/// A drone settling its gun on someone, before the first burst: the chirp
/// that tells you to get behind something. It holds still while it does
/// ([`holding`]): the window to put a round in it, or to run.
pub const LOCK_TIME: f32 = 1.5;
/// Between bursts (s)...
pub const BURST_GAP: f32 = 1.8;
/// ...the first of which it holds still, after a burst, as it does
/// through the burst (s): someone in a headset with a pistol gets a
/// still thing to aim at every couple of seconds (`drone_drill`).
pub const HOLD_AFTER: f32 = 1.2;
/// In a fight it flies this gently (`Intent::pace`: its push, 1 is full).
pub const FIGHT_PACE: f32 = 0.6;

/// Locking on, firing, or just after a burst: it hovers where it is.
pub fn holding(d: &Drone) -> bool {
    (d.lock > 0.0 && d.lock < LOCK_TIME)
        || d.burst_left > 0
        || d.gun_cooldown > BURST_GAP - HOLD_AFTER
}
/// A still body shows a drone this share of a moving one (a walker 0.55).
pub const STILL_SHOWS: f32 = 0.3;
/// Its rounds scatter this much more (radians) per m/s its target runs
/// across its line: sprint across open ground and it mostly misses (at
/// 25 m, a sprint throws them about 3 m wide; a walk about 1).
/// A drone that sees someone on its way home fights for this long (s),
/// then goes on home.
pub const LEAVING_FIGHT: f32 = super::recall::lasts(super::recall::Memory::Roofed);
pub const CROSSING_SPREAD: f32 = 0.025;

/// A drone began locking on (audio chirps; the eye flares).
#[derive(Message, Clone, Copy, Debug)]
pub struct DroneLock {
    pub drone: Entity,
    pub at: Vec3,
}

/// The gun pod's muzzle, for shots.
#[derive(Component)]
pub struct DroneGun;

pub struct DronePlugin;

impl Plugin for DronePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DroneLock>().add_systems(
            FixedUpdate,
            (
                drone_parts_destroyed,
                drone_brain,
                super::flight::fly_drones,
                drone_guns,
            )
                .chain()
                .before(PhysicsSystems::First),
        );
    }
}

/// Spawns a drone at `at` with a job.
pub fn spawn_drone(commands: &mut Commands, at: Vec3, job: Job) -> Entity {
    let robot = commands
        .spawn((
            Name::new("Drone"),
            Robot {
                kind: RobotKind::Drone,
                wrecked: false,
            },
            Drone {
                thrust: [0.0; 4],
                rotor_alive: [true; 4],
                gun_cooldown: 2.0,
                burst_left: 0,
                grounded: 0.0,
                goal: at,
                phase: 0.0,
                looted: false,
                lock: 0.0,
                locks_unhit: 0,
                engaged_at: -1.0,
                last_engaged: -1.0,
                broke_off_until: 0.0,
                quiet: 0.0,
                warning_left: 0,
                last_shot: f32::NEG_INFINITY,
            },
            Senses {
                // A camera dome: sees almost all round, not so far.
                sight: 90.0,
                fov_deg: 320.0,
                hearing: 1.0,
                eye: Vec3::new(0.0, -0.08, -0.22),
            },
            // From up there it picks out movement more than shape: keep
            // still when one passes over.
            super::senses::MotionSight(STILL_SHOWS),
            Awareness::default(),
            Intent::default(),
            Brain::Transit { to: job.site },
            job,
            Transform::from_translation(at),
            Visibility::default(),
            (
                RigidBody::Dynamic,
                Mass(MASS),
                NoAutoMass,
                AngularInertia::new(Vec3::new(0.16, 0.26, 0.16)),
                NoAutoAngularInertia,
                LinearDamping(0.3),
                AngularDamping(1.5),
            ),
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Drone shell"),
            part_bundle(
                robot,
                PartRole::Core,
                70.0,
                Collider::cuboid(0.42, 0.13, 0.42),
                Transform::IDENTITY,
            ),
        ));
        c.spawn((
            Name::new("Drone eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                15.0,
                Collider::sphere(0.07),
                Transform::from_xyz(0.0, -0.06, -0.22),
            ),
        ));
        c.spawn((
            Name::new("Drone gun pod"),
            DroneGun,
            part_bundle(
                robot,
                PartRole::Weapon,
                30.0,
                Collider::cuboid(0.08, 0.08, 0.32),
                Transform::from_xyz(0.0, -0.14, -0.06),
            ),
        ));
        for (i, p) in ROTORS.iter().enumerate() {
            c.spawn((
                Name::new(format!("Rotor {i}")),
                part_bundle(
                    robot,
                    PartRole::Rotor(i as u8),
                    14.0,
                    Collider::cylinder(0.15, 0.04),
                    Transform::from_translation(*p + Vec3::Y * 0.03),
                ),
            ));
        }
    });
    robot
}

fn drone_parts_destroyed(
    mut events: MessageReader<PartDestroyed>,
    mut drones: Query<(&mut Drone, &mut Robot)>,
    mut commands: Commands,
) {
    for ev in events.read() {
        let Ok((mut d, mut robot)) = drones.get_mut(ev.robot) else {
            continue;
        };
        match ev.role {
            PartRole::Rotor(i) => {
                d.rotor_alive[i as usize] = false;
                // The rotor's collider goes; the visual hub stays.
                commands.entity(ev.part).insert(ColliderDisabled);
            }
            PartRole::Core => robot.wrecked = true,
            _ => {}
        }
    }
}

/// Suspicion at which a drone that sees you opens fire.
pub const ENGAGE_AT: f32 = 0.6;
/// Out of sight this long in a fight, it goes to look where you were.
pub const PATIENCE: f32 = 10.0;
/// It fights from this far out (m)...
pub const STANDOFF: f32 = 20.0;
/// ...this high over the ground (m), higher when it has lost sight of you
/// (to look over what you're behind)...
pub const FIGHT_ALTITUDE: (f32, f32) = (8.0, 13.0);
/// Clear of a roof under a goal, at least this (m): over a building, a
/// drone's height is over its top, not the ground round it.
pub const OVER_ROOF: f32 = 4.0;

/// Where a drone flies to for `p` at `h` over what's under it: the
/// ground there or under the drone (`ground`), whichever is higher, and
/// any building's top there, `OVER_ROOF` clear (Robot bodies' flyer sweep,
/// 31 October: a goal over a house put the drone pressed to its wall,
/// trying to fly into it).
pub fn cruise_at(
    zone: &Zone,
    entrances: &super::doors::Entrances,
    p: Vec3,
    ground: f32,
    h: f32,
) -> Vec3 {
    let under = zone.ground(p.x, p.z).max(ground);
    let y = match entrances.top_at(p.x, p.z, 2.0) {
        Some(top) => (under + h).max(top + OVER_ROOF),
        None => under + h,
    };
    Vec3::new(p.x, y, p.z)
}
/// ...circling (rad/s), faster while you're looking at it: it slides out
/// of your aim, round to your flank. Across your view at its standoff
/// that's about 9 and 14 degrees a second: a moving target a pistol in a
/// headset can follow (it was 20 and 52: "way too fast for somebody in VR
/// to possibly have a shot", playtest, 28 October).
pub const ORBIT: (f32, f32) = (0.15, 0.25);
/// Searching, it sweeps the spots this high (m)...
const SEARCH_ALTITUDE: f32 = 6.0;
/// ...and comes down to a doorway on its round, this high and this far
/// out from it, to look in (m).
pub const DOORWAY_ALTITUDE: f32 = 2.7;
pub const DOORWAY_STANDOFF: f32 = 3.5;

/// Picks where the drone wants to be, from its brain state.
#[allow(clippy::too_many_arguments)]
fn drone_brain(
    mut drones: Query<(
        Entity,
        &mut Drone,
        &mut Brain,
        &mut Job,
        &Awareness,
        &Transform,
        &Robot,
        &mut Intent,
        (
            Option<&super::flee::Fleeing>,
            Option<&super::squad::SquadRole>,
        ),
    )>,
    zone: Res<Zone>,
    targets: Query<&GlobalTransform>,
    heads: Query<(&ChildOf, &GlobalTransform), With<crate::rig::RigHead>>,
    time: Res<Time>,
    mut commands: Commands,
    entrances: Res<super::doors::Entrances>,
    door_states: Query<&crate::world::buildings::Door>,
    nav: Res<super::walker::Nav>,
    mut off_duty: Local<std::collections::HashMap<Entity, f32>>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let sweep = time.elapsed_secs() * 0.9;
    off_duty.retain(|e, _| drones.contains(*e));
    for (e, mut d, mut brain, mut job, aware, t, robot, mut intent, (fleeing, role)) in &mut drones
    {
        if robot.wrecked {
            *brain = Brain::Down;
            continue;
        }
        let pos = t.translation;
        *intent = Intent::default();
        let ground = zone.ground(pos.x, pos.z);
        // A live grenade under it: up and away, out past its reach.
        if let Some(f) = fleeing {
            let to = f.away(pos);
            d.goal = Vec3::new(to.x, zone.ground(to.x, to.z).max(ground) + 12.0, to.z);
            intent.look = Some(f.from);
            continue;
        }
        let cruise = |p: Vec3, h: f32| cruise_at(&zone, &entrances, p, ground, h);
        // Someone indoors: hover outside a doorway, low, and look in
        // (further back from one that's shut, waiting for it to open).
        let peek = |at: Vec3| {
            let b = entrances.building_at(at)?;
            let (ent, open) = entrances.approach(b, pos, |door| {
                super::doors::is_open(door_states.get(door).ok())
            })?;
            let p = ent.mid + ent.out * if open { 3.5 } else { 7.0 };
            Some(Vec3::new(p.x, zone.ground(p.x, p.z) + 2.7, p.z))
        };
        let still = matches!(*brain, Brain::Engage) && holding(&d);
        if !still {
            d.phase += dt * ORBIT.0;
        }
        // Alarm overrides the job.
        let leaving = matches!(*brain, Brain::Leave { .. });
        // Broken off a fight it wasn't winning: its job, not you, a while.
        if now >= d.broke_off_until {
            search::react(&mut brain, aware, ENGAGE_AT);
            search::lost_sight(&mut brain, aware, PATIENCE, pos);
        }
        if matches!(*brain, Brain::Engage) {
            // (Lost you and looking: the same fight, back at it.)
            if d.engaged_at < 0.0 || now - d.last_engaged > FIGHT_GAP {
                d.engaged_at = now;
                d.locks_unhit = 0;
            }
            d.last_engaged = now;
            let worn = d.locks_unhit >= RELOCKS && d.lock == 0.0 && d.burst_left == 0;
            if worn || now - d.engaged_at > FIGHT_FOR {
                d.broke_off_until = now + BROKEN_OFF;
                d.engaged_at = -1.0;
                // Back from its job it starts from nothing: a fresh lock,
                // chirp and all, before its first round (`RETELL`).
                d.lock = 0.0;
                d.quiet = 0.0;
                d.burst_left = 0;
                *brain = Brain::Transit { to: job.site };
            }
        }
        // On its way home it spares a burst or two, no more, and none for
        // someone under a roof (`LEAVING_FIGHT`).
        if matches!(*brain, Brain::Engage) {
            if leaving {
                off_duty.insert(e, now + LEAVING_FIGHT);
            }
            if let Some(until) = off_duty.get(&e).copied() {
                let indoors = aware
                    .target
                    .and_then(|p| targets.get(p).ok())
                    .is_some_and(|g| under_a_roof(&entrances, g.translation()));
                if now > until || indoors {
                    off_duty.remove(&e);
                    *brain = Brain::Leave { to: job.exit };
                }
            }
        } else if !matches!(*brain, Brain::Leave { .. }) {
            off_duty.remove(&e);
        }
        match &mut *brain {
            Brain::Transit { to } => {
                d.goal = cruise(*to, 22.0);
                if Vec2::new(pos.x - to.x, pos.z - to.z).length() < 6.0 {
                    *brain = Brain::Working {
                        at: *to,
                        left: job.duration,
                    };
                }
            }
            Brain::Working { at, left } => {
                // Survey: slow circles over the site, low enough to look in.
                let r = 14.0;
                d.goal = cruise(
                    *at + Vec3::new(d.phase.cos() * r, 0.0, d.phase.sin() * r),
                    11.0,
                );
                *left -= dt;
                if *left <= 0.0 {
                    *brain = Brain::Leave { to: job.exit };
                }
            }
            Brain::Investigate { at, left } => {
                let at = *at;
                let looking_in = peek(at);
                d.goal = looking_in.unwrap_or_else(|| {
                    cruise(
                        at + Vec3::new(d.phase.cos() * 6.0, 0.0, d.phase.sin() * 6.0),
                        9.0,
                    )
                });
                intent.look = Some(at);
                *left -= dt;
                let there = match looking_in {
                    Some(g) => g.distance(pos) < 3.0,
                    None => Vec2::new(pos.x - at.x, pos.z - at.z).length() < 9.0,
                };
                if there || *left <= 0.0 {
                    *brain = search::begin_search(
                        at,
                        aware,
                        pos,
                        super::indoor::NavRef::Zone(&nav.0),
                        &entrances,
                    );
                }
            }
            Brain::Search { .. } => {
                // A doorway on the round: down to it, hanging back, and
                // look in.
                let doorway = |p: Vec3| {
                    entrances.doors.iter().find(|e| {
                        let s = e.mid + e.out * 1.8;
                        Vec2::new(p.x - s.x, p.z - s.z).length() < 2.6
                    })
                };
                // Not there till it's down at the doorway, not just over it.
                let next = match &*brain {
                    Brain::Search { spots, dwell, .. } if *dwell <= 0.0 => spots.first().copied(),
                    _ => None,
                };
                let descending = next.and_then(doorway).map(|e| {
                    let h = e.mid + e.out * DOORWAY_STANDOFF;
                    Vec3::new(h.x, zone.ground(h.x, h.z) + DOORWAY_ALTITUDE, h.z)
                });
                if let Some(h) = descending.filter(|h| h.distance(pos) > 1.5) {
                    if let Brain::Search { left, .. } = &mut *brain {
                        *left -= dt;
                    }
                    // Over it first, then down (not low across the yard).
                    let over = Vec2::new(h.x - pos.x, h.z - pos.z).length() < 4.0;
                    d.goal = if over { h } else { cruise(h, SEARCH_ALTITUDE) };
                    intent.pace = 0.7;
                    continue;
                }
                match search::search_step(&mut brain, pos, 4.0, dt, sweep) {
                    Step::Go(p) => {
                        d.goal = match doorway(p) {
                            Some(e) => {
                                let h = e.mid + e.out * DOORWAY_STANDOFF;
                                Vec3::new(h.x, zone.ground(h.x, h.z) + DOORWAY_ALTITUDE, h.z)
                            }
                            None => cruise(p, SEARCH_ALTITUDE),
                        };
                        intent.pace = 0.7;
                    }
                    Step::Look(l) => {
                        intent.look = Some(doorway(pos).map_or(l, |e| e.mid - e.out * 3.0));
                    }
                    Step::Done => search::give_up(&mut brain, &mut job),
                }
            }
            Brain::Engage => {
                let seen = aware
                    .target
                    .filter(|_| aware.can_see_target)
                    .and_then(|p| targets.get(p).ok())
                    .map(|g| g.translation());
                let Some(target) = seen.or(aware.last_known) else {
                    *brain = Brain::Leave { to: job.exit };
                    continue;
                };
                if let Some(g) = peek(target) {
                    d.goal = g;
                    continue;
                }
                // Spotting for a flanker (`squad`): over its way in,
                // lighting it, still watching you.
                if let Some(r) = role.filter(|r| r.role == super::squad::Role::Spot) {
                    d.goal = r.post;
                    intent.look = Some(target);
                    continue;
                }
                // Looked at down your sights? Slide round to your flank.
                let eyed = aware
                    .target
                    .filter(|_| seen.is_some())
                    .and_then(|p| heads.iter().find(|(c, _)| c.parent() == p))
                    .is_some_and(|(_, h)| {
                        let to_me = (pos - h.translation()).normalize_or_zero();
                        (h.rotation() * Vec3::NEG_Z).dot(to_me) > 0.9
                    });
                if (eyed || seen.is_none()) && !still {
                    d.phase += dt * (ORBIT.1 - ORBIT.0);
                }
                intent.pace = FIGHT_PACE;
                // Stand off and circle: hard to hit, keeps a firing line;
                // over where you were, higher, when it's lost you.
                let h = if seen.is_some() {
                    FIGHT_ALTITUDE.0
                } else {
                    FIGHT_ALTITUDE.1
                };
                let r = if seen.is_some() {
                    STANDOFF
                } else {
                    STANDOFF * 0.6
                };
                let off = Vec3::new(d.phase.cos() * r, 0.0, d.phase.sin() * r);
                d.goal = cruise(target + off, h);
                intent.look = Some(target);
            }
            Brain::Leave { to } => {
                d.goal = cruise(*to, 25.0);
                if Vec2::new(pos.x - to.x, pos.z - to.z).length() < 8.0 {
                    commands.entity(e).despawn();
                }
            }
            Brain::Down => {}
        }
    }
}

/// Bursts at whoever it's engaging, when the gun bears.
fn drone_guns(
    mut drones: Query<(
        Entity,
        &mut Drone,
        &Brain,
        &Awareness,
        &Robot,
        &Children,
        &Transform,
    )>,
    guns: Query<(&GlobalTransform, &RobotPart), With<DroneGun>>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
    mut shots: MessageWriter<Shot>,
    mut locks: MessageWriter<DroneLock>,
    mut seed: Local<u64>,
    minds: Query<&super::senses::MindSeed>,
    mut fighting_since: Local<std::collections::HashMap<Entity, f32>>,
    (bodies, roles, mut hits, parents, entrances): (
        Query<&crate::player::Body>,
        Query<&super::squad::SquadRole>,
        MessageReader<crate::weapons::ballistics::Hit>,
        Query<&ChildOf>,
        Res<super::doors::Entrances>,
    ),
) {
    let dt = time.delta_secs();
    // A round of its own in the one it's fighting: that fight's going on.
    for h in hits.read() {
        // A warning round in them isn't a fight being won.
        let Some(shooter) = h.shooter.filter(|_| h.damage > 0.0) else {
            continue;
        };
        let Ok((_, mut d, _, aware, ..)) = drones.get_mut(shooter) else {
            continue;
        };
        let on = |x: Entity| Some(x) == aware.target;
        if on(h.entity) || parents.get(h.entity).is_ok_and(|p| on(p.parent())) {
            d.locks_unhit = 0;
        }
    }
    fighting_since.retain(|e, _| {
        drones
            .get(*e)
            .is_ok_and(|(_, _, b, ..)| matches!(b, Brain::Engage))
    });
    for (e, mut d, brain, aware, robot, children, t) in &mut drones {
        d.gun_cooldown -= dt;
        // Climbing to its station over a flanker's way in (`squad`): it
        // holds its fire.
        let moving_up = super::squad::repositioning(roles.get(e).ok(), t.translation);
        if robot.wrecked || !matches!(brain, Brain::Engage) || moving_up {
            d.lock = 0.0;
            d.quiet = 0.0;
            continue;
        }

        if !aware.can_see_target {
            // A moment behind cover doesn't shake it; a while does.
            if aware.unseen > 1.5 {
                d.lock = 0.0;
                d.quiet = 0.0;
            }
            continue;
        }
        let Some(target) = aware
            .target
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() + Vec3::Y * 1.2)
        else {
            continue;
        };
        // Only a roof stops it, and a roof always does: someone under one
        // it neither locks on to nor fires at, whatever gap it sees them
        // through (a door, a window). It hangs about over the building
        // till its fight wears out (`FIGHT_FOR`) and it goes back to its
        // job. (Through the post office's door it killed a newcomer who'd
        // taken cover as the driver said: first_jobs_cautious, seed 1.)
        if under_a_roof(&entrances, target - Vec3::Y * 1.2) {
            d.lock = 0.0;
            d.quiet = 0.0;
            d.burst_left = 0;
            continue;
        }
        let Some((gun_gt, part)) = children.iter().find_map(|c| guns.get(c).ok()) else {
            continue;
        };
        if !part.alive() {
            continue;
        }
        // A first look at you before it locks on (`walker::SIGHT_BEAT`).
        let looked = fighting_since.entry(e).or_insert(0.0);
        *looked += dt;
        if *looked < super::walker::SIGHT_BEAT {
            continue;
        }
        if d.lock < LOCK_TIME {
            if d.lock == 0.0 {
                d.locks_unhit = d.locks_unhit.saturating_add(1);
                if time.elapsed_secs() - d.last_shot > WARN_AFTER {
                    d.warning_left = BURST;
                }
                locks.write(DroneLock {
                    drone: e,
                    at: t.translation,
                });
            }
            d.lock += dt;
            continue;
        }
        if lose_a_quiet_lock(&mut d, dt) {
            continue;
        }
        if d.gun_cooldown > 0.0 {
            continue;
        }
        let muzzle = gun_gt.transform_point(Vec3::new(0.0, 0.0, -0.18));
        let to = (target - muzzle).normalize_or(Vec3::NEG_Z);
        // The pod gimbals, but only so far from the nose.
        if (gun_gt.rotation() * Vec3::NEG_Z).angle_between(to) > 0.9 {
            continue;
        }
        if d.burst_left == 0 {
            d.burst_left = BURST;
        }
        // The warning: at the ground short of your feet, on the line from
        // it to you, wide, harmless.
        let warning = d.warning_left > 0;
        let to = if warning {
            d.warning_left -= 1;
            let feet = target - Vec3::Y * 1.2;
            let back = (muzzle - feet).with_y(0.0).normalize_or_zero();
            (feet + back * WARNING_SHORT - muzzle).normalize_or(to)
        } else {
            to
        };
        // Someone running across its line is hard to hit from up here.
        let crossing = aware
            .target
            .and_then(|p| bodies.get(p).ok())
            .map_or(0.0, |b| {
                let v = Vec3::new(b.velocity.x, 0.0, b.velocity.z);
                (v - to * v.dot(to)).length()
            });
        *seed = super::senses::shot_seed(&minds, e, &time);
        shots.write(Shot {
            gun: e,
            shooter: Some(e),
            origin: muzzle,
            direction: to,
            speed: 380.0,
            damage: if warning { 0.0 } else { DAMAGE },
            projectiles: 1,
            spread: if warning {
                0.035 * 2.0
            } else {
                0.035 + CROSSING_SPREAD * crossing
            },
            recoil: 0.0,
            seed: *seed,
        });
        d.burst_left -= 1;
        d.quiet = 0.0;
        d.last_shot = time.elapsed_secs();
        d.gun_cooldown = if d.burst_left == 0 { BURST_GAP } else { 0.11 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drone's goal over a building is over its roof, clear, in every
    /// valley: never inside it, where the drone would press against its
    /// wall trying to get there (Robot bodies' flyer sweep, 31 October).
    #[test]
    fn a_drones_goal_clears_every_roof() {
        let mut inside = vec![];
        let mut tried = 0;
        for (id, _) in extraction_core::map::VALLEYS {
            let opts = crate::LaunchOptions {
                robots: false,
                map: id.to_string(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..3 {
                app.update();
            }
            let zone = app.world().resource::<Zone>().clone();
            let entrances = app.world().resource::<super::super::doors::Entrances>();
            for s in &entrances.shells {
                let c = s.xf.translation;
                let top = c.y + s.height;
                tried += 1;
                let g = cruise_at(&zone, entrances, c, zone.ground(c.x, c.z), FIGHT_ALTITUDE.0);
                if g.y < top + OVER_ROOF - 1e-3 {
                    inside.push(format!(
                        "{id} at ({:.0}, {:.0}): goal {:.1} under its top {:.1}",
                        c.x, c.z, g.y, top
                    ));
                }
            }
        }
        assert!(tried >= 20, "tried {tried}");
        assert!(
            inside.is_empty(),
            "{} goals in a building: {inside:#?}",
            inside.len()
        );
    }

    /// Firing steadily (bursts 1.8 s apart) it keeps its lock; gone quiet
    /// past [`RETELL`], it locks on afresh, so its next burst comes after a
    /// chirp.
    #[test]
    fn a_drone_gone_quiet_chirps_again_before_it_fires() {
        let mut d = Drone {
            thrust: [0.0; 4],
            rotor_alive: [true; 4],
            gun_cooldown: 0.0,
            burst_left: 0,
            grounded: 0.0,
            goal: Vec3::ZERO,
            phase: 0.0,
            looted: false,
            lock: LOCK_TIME,
            quiet: 0.0,
            warning_left: 0,
            last_shot: f32::NEG_INFINITY,
            locks_unhit: 0,
            engaged_at: -1.0,
            last_engaged: -1.0,
            broke_off_until: 0.0,
        };
        let dt = 1.0 / 90.0;
        // Bursts: the gap between them, then a shot (which resets `quiet`).
        for _ in 0..5 {
            for _ in 0..(BURST_GAP / dt) as usize {
                assert!(
                    !lose_a_quiet_lock(&mut d, dt),
                    "lost its lock between bursts"
                );
            }
            d.quiet = 0.0;
        }
        assert_eq!(d.lock, LOCK_TIME);
        // Its gun off you for a while: the lock goes.
        let lost = (0..(RETELL / dt) as usize + 10).any(|_| lose_a_quiet_lock(&mut d, dt));
        assert!(lost && d.lock == 0.0, "still locked after {RETELL} s quiet");
    }

    #[derive(Resource, Default)]
    struct Fight {
        locks: Vec<f32>,
        shots: Vec<f32>,
        /// Harmless rounds it fired (its warnings).
        warnings: usize,
        /// When it hurt you, and how much.
        hurts: Vec<(f32, f32)>,
    }

    fn tally(
        mut locks: MessageReader<DroneLock>,
        mut shots: MessageReader<Shot>,
        mut hurts: MessageReader<crate::health::Hurt>,
        drones: Query<(), With<Drone>>,
        time: Res<Time>,
        mut fight: ResMut<Fight>,
    ) {
        let t = time.elapsed_secs();
        fight.locks.extend(locks.read().map(|_| t));
        for s in shots.read() {
            if s.shooter.is_some_and(|e| drones.contains(e)) {
                fight.shots.push(t);
                fight.warnings += (s.damage <= 0.0) as usize;
            }
        }
        for h in hurts.read() {
            if h.by.is_some_and(|e| drones.contains(e)) {
                fight.hurts.push((t, h.amount));
            }
        }
    }

    /// Only a roof stops it, and a roof always does: someone in the post
    /// office a step in and aside from its door, where three newcomers who'd
    /// taken cover from a drone were shot through the doorway, is never
    /// locked on to or fired at by a drone coming at them from any side
    /// (first_jobs_cautious, seed 1).
    #[test]
    fn no_drone_fires_at_someone_under_a_roof() {
        // Onboarding's (-19.4, 35.3) on Marrow Creek, from its post office at
        // (-22.2, 33.5); the test field has it at (-20, 65), turned the same.
        // On its floor (a step above the ground outside). And the control:
        // in the open 12 m east of its door, where the same drone does fire
        // (else this would pass on a drone that never fires at all).
        let spot = |zone: &crate::world::Zone| zone.on_ground(-17.2, 66.8) + Vec3::Y * 0.65;
        let open = |zone: &crate::world::Zone| zone.on_ground(-5.0, 66.8);
        let fight = |k: usize, indoors: bool| {
            let ang = k as f32 * std::f32::consts::TAU / 8.0;
            let (mut app, player, _drone, me) = drone_over(|zone| {
                let me = if indoors { spot(zone) } else { open(zone) };
                (me, me + Vec3::new(ang.sin() * 15.0, 6.0, ang.cos() * 15.0))
            });
            for _ in 0..30 {
                app.update();
            }
            if indoors {
                assert!(
                    under_a_roof(app.world().resource::<super::super::doors::Entrances>(), me),
                    "the spot isn't in the post office: {me}"
                );
            }
            for i in 0..90 * 40 {
                // On the move (as anyone ducking in from a drone is): what
                // it notices; held to the spot.
                app.world_mut()
                    .get_mut::<Transform>(player)
                    .unwrap()
                    .translation = me;
                let a = i as f32 / 90.0;
                app.world_mut()
                    .get_mut::<crate::player::Body>(player)
                    .unwrap()
                    .velocity = Vec3::new(a.cos(), 0.0, a.sin()) * 1.4;
                let mut h = app
                    .world_mut()
                    .get_mut::<crate::health::Health>(player)
                    .unwrap();
                h.hp = h.max;
                h.down = false;
                app.update();
            }
            let f = app.world().resource::<Fight>();
            (f.locks.len(), f.shots.len())
        };
        let (_, shots) = fight(0, false);
        assert!(shots > 0, "the control: no shots at someone in the open");
        let fired: Vec<String> = (0..8)
            .map(|k| (k, fight(k, true)))
            .filter(|(_, (l, s))| *l > 0 || *s > 0)
            .map(|(k, (l, s))| format!("bearing {k}: {l} locks, {s} shots"))
            .collect();
        assert!(fired.is_empty(), "under the post office roof: {fired:?}");
    }

    /// Its first burst after every lock warns and hurts no one: standing
    /// in plain view, nothing hurts you sooner than `LOCK_TIME` +
    /// `BURST_GAP` after its chirp, and never more than one burst's harm
    /// in any two seconds (two bursts in 2 s killed the cautious newcomer
    /// who moved only once the first hit: first_jobs_cautious, seed 1).
    #[test]
    fn a_drones_first_burst_after_a_lock_warns() {
        let (mut app, player, drone, at) = drone_on_you();
        // Walking about till it's seen you, then on the spot, stepping
        // toward it (moving, so it sees you: standing still hides you from
        // a drone; but not across its line, which is hard to hit, and this
        // is about what hits you).
        for i in 0..90 * 5 {
            step(&mut app, player, at, true, i);
        }
        for _ in 0..90 * 55 {
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = at;
            let to = app.world().get::<Transform>(drone).unwrap().translation - at;
            app.world_mut()
                .get_mut::<crate::player::Body>(player)
                .unwrap()
                .velocity = to.with_y(0.0).normalize_or_zero() * 1.4;
            let mut h = app
                .world_mut()
                .get_mut::<crate::health::Health>(player)
                .unwrap();
            h.hp = h.max;
            h.down = false;
            app.update();
        }
        let f = app.world().resource::<Fight>();
        assert!(
            !f.locks.is_empty() && !f.hurts.is_empty(),
            "no fight: locks {:?}, shots {:?} ({} warnings)",
            f.locks,
            f.shots,
            f.warnings
        );
        assert!(
            f.warnings >= BURST as usize,
            "it never warned: {} shots",
            f.shots.len()
        );
        // The floor itself: 1.5 s chirp, then a harmless burst, then 1.8 s.
        const FIRST_HARM: f32 = 3.3;
        // Every lock after a lull (none of its rounds in the WARN_AFTER
        // before it): nothing hurts you within FIRST_HARM of it.
        let soon: Vec<String> = f
            .locks
            .iter()
            .filter(|l| !f.shots.iter().any(|s| *s < **l && *s > **l - WARN_AFTER))
            .flat_map(|l| {
                f.hurts
                    .iter()
                    .filter(move |(t, _)| *t >= *l && *t < l + FIRST_HARM - 0.05)
                    .map(move |(t, _)| format!("hurt {:.2} s after a lock", t - l))
            })
            .collect();
        assert!(soon.is_empty(), "{soon:?}");
        let worst = f
            .hurts
            .iter()
            .map(|(t0, _)| {
                f.hurts
                    .iter()
                    .filter(|(t, _)| *t >= *t0 && *t < t0 + 2.0)
                    .map(|(_, a)| a)
                    .sum::<f32>()
            })
            .fold(0.0, f32::max);
        assert!(
            worst <= BURST as f32 * DAMAGE + 0.01,
            "{worst} hp in 2 s (a burst is {})",
            BURST as f32 * DAMAGE
        );
    }

    /// A drone over the test field with you walking under it; your
    /// health kept up (a fight, not a death, is what's tested).
    fn drone_on_you() -> (App, Entity, Entity, Vec3) {
        drone_over(|zone| {
            let at = zone.on_ground(40.0, 255.0);
            (at, at + Vec3::new(0.0, 6.0, -15.0))
        })
    }

    /// The same, with you at and its job's site where `place` says.
    fn drone_over(
        place: impl Fn(&crate::world::Zone) -> (Vec3, Vec3),
    ) -> (App, Entity, Entity, Vec3) {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.init_resource::<Fight>().add_systems(Last, tally);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let (at, site) = place(&zone);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let drone = {
            let mut c = Commands::new(&mut queue, app.world());
            spawn_drone(
                &mut c,
                site,
                Job {
                    kind: super::super::JobKind::Survey,
                    site,
                    duration: 9999.0,
                    exit: site,
                },
            )
        };
        queue.apply(app.world_mut());
        (app, player, drone, at)
    }

    /// One tick of you: walking a small circle at `at` (seen), or far off
    /// (out of its sight); hale either way.
    fn step(app: &mut App, player: Entity, at: Vec3, seen: bool, i: usize) {
        let a = i as f32 / 90.0 * 0.5;
        let here = if seen {
            at + Vec3::new(a.cos(), 0.0, a.sin()) * 2.0
        } else {
            at + Vec3::new(400.0, 0.0, 0.0)
        };
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = here;
        app.world_mut()
            .get_mut::<crate::player::Body>(player)
            .unwrap()
            .velocity = Vec3::new(-a.sin(), 0.0, a.cos());
        let mut h = app
            .world_mut()
            .get_mut::<crate::health::Health>(player)
            .unwrap();
        h.hp = h.max;
        h.down = false;
        app.update();
    }

    /// A drone that keeps locking on to someone who keeps getting out of
    /// its sight before it can fire gives up after [`RELOCKS`] locks: it
    /// goes back to its job and doesn't lock on again for a while, though
    /// they stand in plain view (one re-locked a hiding newcomer every
    /// 24 s for three minutes, till it had them).
    #[test]
    fn a_drone_gives_up_after_its_relocks() {
        let (mut app, player, drone, at) = drone_on_you();
        let mut i = 0;
        let mut seen = 0;
        // Out of sight a beat after each lock, before it can fire.
        while app.world().resource::<Fight>().locks.len() < RELOCKS as usize && i < 90 * 60 {
            let locks = app.world().resource::<Fight>().locks.len();
            let t = app.world().resource::<Time>().elapsed_secs();
            let since = app
                .world()
                .resource::<Fight>()
                .locks
                .last()
                .map_or(99.0, |l| t - l);
            let visible = since > 3.5 || (since < 1.0 && locks == seen);
            if since < 1.0 {
                seen = locks;
            }
            step(
                &mut app,
                player,
                at,
                visible && since > 1.0 || since > 3.5,
                i,
            );
            i += 1;
        }
        assert_eq!(
            app.world().resource::<Fight>().locks.len(),
            RELOCKS as usize,
            "never locked {RELOCKS} times"
        );
        // Gone a while; then in plain view, 40 s.
        for k in 0..90 * 3 {
            step(&mut app, player, at, false, i + k);
        }
        i += 90 * 3;
        let before = app.world().resource::<Fight>().locks.len();
        for k in 0..90 * 40 {
            step(&mut app, player, at, true, i + k);
        }
        let f = app.world().resource::<Fight>();
        assert!(f.shots.is_empty(), "it fired: {:?}", f.shots);
        assert_eq!(f.locks.len(), before, "locked on again: {:?}", f.locks);
        assert!(
            !matches!(app.world().get::<Brain>(drone), Some(Brain::Engage)),
            "still fighting"
        );
    }

    /// Nor does it fight anyone longer than [`FIGHT_FOR`]: then it breaks
    /// off, and leaves them be a while.
    #[test]
    fn a_drone_breaks_off_a_long_fight() {
        let (mut app, player, drone, at) = drone_on_you();
        let mut engaged = None;
        let mut off = None;
        for i in 0..90 * 120 {
            step(&mut app, player, at, true, i);
            let t = app.world().resource::<Time>().elapsed_secs();
            let fighting = matches!(app.world().get::<Brain>(drone), Some(Brain::Engage));
            if fighting && engaged.is_none() {
                engaged = Some(t);
            }
            if !fighting && engaged.is_some() {
                off = Some(t);
                break;
            }
        }
        let (on, off) = (
            engaged.expect("it fought"),
            off.expect("it never broke off"),
        );
        assert!(off - on <= FIGHT_FOR + 1.0, "fought {:.0} s", off - on);
        let locks = app.world().resource::<Fight>().locks.len();
        for i in 0..90 * 40 {
            step(&mut app, player, at, true, i);
        }
        assert_eq!(
            app.world().resource::<Fight>().locks.len(),
            locks,
            "locked on again inside {BROKEN_OFF} s"
        );
    }

    #[derive(Resource, Default)]
    struct Heard {
        lock: Option<f32>,
        shot: Option<f32>,
    }

    fn listen(
        mut locks: MessageReader<DroneLock>,
        mut shots: MessageReader<Shot>,
        drones: Query<(), With<Drone>>,
        time: Res<Time>,
        mut heard: ResMut<Heard>,
    ) {
        let t = time.elapsed_secs();
        if locks.read().next().is_some() && heard.lock.is_none() {
            heard.lock = Some(t);
        }
        for s in shots.read() {
            if s.shooter.is_some_and(|e| drones.contains(e)) && heard.shot.is_none() {
                heard.shot = Some(t);
            }
        }
    }

    #[test]
    fn a_drone_chirps_before_it_fires() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.init_resource::<Heard>().add_systems(Last, listen);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let at = zone.on_ground(40.0, 255.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = at;
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let site = at + Vec3::new(0.0, 6.0, -15.0);
        {
            let mut c = Commands::new(&mut queue, app.world());
            spawn_drone(
                &mut c,
                site,
                Job {
                    kind: super::super::JobKind::Survey,
                    site,
                    duration: 999.0,
                    exit: site,
                },
            );
        }
        queue.apply(app.world_mut());
        // Walking about (a drone picks out movement more than shape: one
        // that stood stock still might not be made out at all, which is
        // `mind_tests::keeping_still_beats_a_drone_overhead`).
        for i in 0..90 * 15 {
            let a = i as f32 / 90.0 * 0.5;
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = at + Vec3::new(a.cos(), 0.0, a.sin()) * 2.0;
            app.world_mut()
                .get_mut::<crate::player::Body>(player)
                .unwrap()
                .velocity = Vec3::new(-a.sin(), 0.0, a.cos());
            app.update();
            if app.world().resource::<Heard>().shot.is_some() {
                break;
            }
        }
        let heard = app.world().resource::<Heard>();
        let lock = heard.lock.expect("it locked on");
        let shot = heard.shot.expect("then fired");
        assert!(
            shot - lock >= LOCK_TIME - 0.05,
            "a beat to duck: locked at {lock:.2}, fired at {shot:.2}"
        );
    }
}
