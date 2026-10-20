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
/// that tells you to get behind something.
pub const LOCK_TIME: f32 = 0.9;

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
/// ...circling (rad/s), faster while you're looking at it: it slides out
/// of your aim, round to your flank.
pub const ORBIT: (f32, f32) = (0.35, 0.9);
/// Searching, it sweeps the spots this high (m).
const SEARCH_ALTITUDE: f32 = 6.0;

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
    )>,
    zone: Res<Zone>,
    targets: Query<&GlobalTransform>,
    heads: Query<(&ChildOf, &GlobalTransform), With<crate::rig::RigHead>>,
    time: Res<Time>,
    mut commands: Commands,
    entrances: Res<super::doors::Entrances>,
    door_states: Query<&crate::world::buildings::Door>,
    nav: Res<super::walker::Nav>,
) {
    let dt = time.delta_secs();
    let sweep = time.elapsed_secs() * 0.9;
    for (e, mut d, mut brain, mut job, aware, t, robot, mut intent) in &mut drones {
        if robot.wrecked {
            *brain = Brain::Down;
            continue;
        }
        let pos = t.translation;
        *intent = Intent::default();
        let ground = zone.ground(pos.x, pos.z);
        let cruise = |p: Vec3, h: f32| Vec3::new(p.x, zone.ground(p.x, p.z).max(ground) + h, p.z);
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
        d.phase += dt * ORBIT.0;
        // Alarm overrides the job.
        search::react(&mut brain, aware, ENGAGE_AT);
        search::lost_sight(&mut brain, aware, PATIENCE, pos);
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
                    *brain = search::begin_search(at, aware, pos, &nav.0, &entrances);
                }
            }
            Brain::Search { .. } => match search::search_step(&mut brain, pos, 4.0, dt, sweep) {
                Step::Go(p) => {
                    d.goal = cruise(p, SEARCH_ALTITUDE);
                    intent.pace = 0.7;
                }
                Step::Look(l) => intent.look = Some(l),
                Step::Done => search::give_up(&mut brain, &mut job),
            },
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
                // Looked at down your sights? Slide round to your flank.
                let eyed = aware
                    .target
                    .filter(|_| seen.is_some())
                    .and_then(|p| heads.iter().find(|(c, _)| c.parent() == p))
                    .is_some_and(|(_, h)| {
                        let to_me = (pos - h.translation()).normalize_or_zero();
                        (h.rotation() * Vec3::NEG_Z).dot(to_me) > 0.9
                    });
                if eyed || seen.is_none() {
                    d.phase += dt * (ORBIT.1 - ORBIT.0);
                }
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
) {
    let dt = time.delta_secs();
    for (e, mut d, brain, aware, robot, children, t) in &mut drones {
        d.gun_cooldown -= dt;
        if robot.wrecked || !matches!(brain, Brain::Engage) {
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
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        shots.write(Shot {
            gun: e,
            shooter: Some(e),
            origin: muzzle,
            direction: to,
            speed: 380.0,
            damage: 9.0,
            projectiles: 1,
            spread: 0.035,
            recoil: 0.0,
            seed: *seed,
        });
        d.burst_left -= 1;
        d.gun_cooldown = if d.burst_left == 0 { 1.8 } else { 0.11 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for _ in 0..90 * 15 {
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
