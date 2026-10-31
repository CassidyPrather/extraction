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
}

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
        search::react(&mut brain, aware, ENGAGE_AT);
        search::lost_sight(&mut brain, aware, PATIENCE, pos);
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
                    .is_some_and(|g| {
                        entrances
                            .building_at(g.translation() + Vec3::Y * 0.5)
                            .is_some()
                    });
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
    (bodies, roles): (Query<&crate::player::Body>, Query<&super::squad::SquadRole>),
) {
    let dt = time.delta_secs();
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
            continue;
        }

        if !aware.can_see_target {
            // A moment behind cover doesn't shake it; a while does.
            if aware.unseen > 1.5 {
                d.lock = 0.0;
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
                locks.write(DroneLock {
                    drone: e,
                    at: t.translation,
                });
            }
            d.lock += dt;
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
            d.burst_left = 3;
        }
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
            damage: 9.0,
            projectiles: 1,
            spread: 0.035 + CROSSING_SPREAD * crossing,
            recoil: 0.0,
            seed: *seed,
        });
        d.burst_left -= 1;
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
