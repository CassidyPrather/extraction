//! The Zone's robots: sleek, working, in your way.
//!
//! Every robot is built from **parts** (child entities with colliders on
//! [`Layer::Robot`]): shooting a part damages it, and parts matter — a drone
//! that loses a rotor falls out of the sky, a walker that loses legs limps
//! and then goes down. Robots **perceive** (sight cones through the fog,
//! hearing [`Noise`]), think with a small **brain** state machine around
//! their current **job**, and arrive and leave diegetically through the
//! Gate (see [`director`]).

pub mod collector;
pub mod director;
pub mod dirigible;
pub mod drone;
pub mod grappler;
pub mod visuals;
pub mod walker;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::player::Player;
use crate::weapons::ballistics::Hit;
use crate::world::Layer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RobotKind {
    /// Quadcopter with a gun pod.
    Drone,
    /// A sensor orb carried on ball-jointed legs (also the grappler and
    /// the convoy's collector, by their components).
    Walker,
    /// The airship that carries cargo pods.
    Dirigible,
}

#[derive(Component, Debug)]
pub struct Robot {
    pub kind: RobotKind,
    /// Set once the core is destroyed (or it falls apart).
    pub wrecked: bool,
}

/// Knocked out by an EMP for this many seconds: no senses, no rotors, no
/// legs. (`gadgets` adds and times it out.)
#[derive(Component, Debug, Clone, Copy)]
pub struct Stunned(pub f32);

/// What a part does for its robot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartRole {
    /// Destroy it and the robot is done.
    Core,
    /// Optics: blinded robots lose sight.
    Sensor,
    Weapon,
    Rotor(u8),
    Leg(u8),
    /// One of a dirigible's gas cells.
    GasCell(u8),
}

#[derive(Component, Debug)]
pub struct RobotPart {
    pub robot: Entity,
    pub role: PartRole,
    pub hp: f32,
    pub max_hp: f32,
}

impl RobotPart {
    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }
}

/// A part was destroyed (for effects, loot and the brain).
#[derive(Message, Clone, Debug)]
pub struct PartDestroyed {
    pub robot: Entity,
    pub part: Entity,
    pub role: PartRole,
    pub at: Vec3,
}

/// Something made a sound robots might hear. `radius` is how far, in
/// metres, a robot with ordinary hearing notices it.
#[derive(Message, Clone, Debug)]
pub struct Noise {
    pub at: Vec3,
    pub radius: f32,
    pub source: Option<Entity>,
}

/// What a robot knows about the world.
#[derive(Component, Debug, Default)]
pub struct Awareness {
    /// The player it is tracking, if any.
    pub target: Option<Entity>,
    /// Where it last saw or heard something.
    pub last_known: Option<Vec3>,
    /// Seconds since the target was last seen.
    pub unseen: f32,
    /// 0..1 how sure it is something's there.
    pub suspicion: f32,
    pub can_see_target: bool,
}

#[derive(Component, Debug, Clone)]
pub struct Senses {
    pub sight: f32,
    pub fov_deg: f32,
    pub hearing: f32,
    /// The eye, relative to the robot root.
    pub eye: Vec3,
}

/// What a robot is doing.
#[derive(Component, Debug, Clone)]
pub enum Brain {
    /// Heading to the job site.
    Transit { to: Vec3 },
    /// Doing the job until the timer runs out.
    Working { at: Vec3, left: f32 },
    /// Checking out a disturbance.
    Investigate { at: Vec3, left: f32 },
    /// Fighting.
    Engage,
    /// Done; heading out through the Gate.
    Leave { to: Vec3 },
    /// Wrecked or stunned.
    Down,
}

/// The robot's posted job, from the director.
#[derive(Component, Debug, Clone)]
pub struct Job {
    pub kind: JobKind,
    pub site: Vec3,
    pub duration: f32,
    pub exit: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    /// Fly or walk to a point of interest, scan it, leave.
    Survey,
    /// Go to the last known position of a disturbance.
    Response,
    /// Loiter on a route between points.
    Watch,
    /// The memorial convoy: fetch a body (remains) and take it out.
    Collect,
}

pub struct RobotsPlugin;

impl Plugin for RobotsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PartDestroyed>()
            .add_message::<Noise>()
            .add_plugins((
                drone::DronePlugin,
                walker::WalkerPlugin,
                director::DirectorPlugin,
                collector::CollectorPlugin,
                dirigible::DirigiblePlugin,
                grappler::GrapplerPlugin,
            ))
            .add_systems(
                FixedUpdate,
                (damage_parts, perceive, gunshot_noise, wreck_loot)
                    .chain()
                    .after(crate::weapons::ballistics::BallisticsSystems),
            );
    }
}

/// Bullets damage the part they strike.
fn damage_parts(
    mut hits: MessageReader<Hit>,
    mut parts: Query<(&mut RobotPart, &GlobalTransform)>,
    mut robots: Query<&mut Robot>,
    mut destroyed: MessageWriter<PartDestroyed>,
    mut noise: MessageWriter<Noise>,
) {
    for hit in hits.read() {
        let Ok((mut part, gt)) = parts.get_mut(hit.entity) else {
            continue;
        };
        if !part.alive() {
            continue;
        }
        // Being shot is loud and tells everyone roughly where from.
        noise.write(Noise {
            at: hit.point,
            radius: 40.0,
            source: hit.shooter,
        });
        part.hp -= hit.damage;
        if part.hp <= 0.0 {
            destroyed.write(PartDestroyed {
                robot: part.robot,
                part: hit.entity,
                role: part.role,
                at: gt.translation(),
            });
            if part.role == PartRole::Core {
                if let Ok(mut r) = robots.get_mut(part.robot) {
                    r.wrecked = true;
                }
            }
        }
    }
}

/// Shots are heard a long way off.
fn gunshot_noise(
    mut shots: MessageReader<crate::weapons::ballistics::Shot>,
    mut noise: MessageWriter<Noise>,
) {
    for s in shots.read() {
        noise.write(Noise {
            at: s.origin,
            radius: 220.0,
            source: s.shooter,
        });
    }
}

/// Sight and hearing. Sight needs a clear line through the world (and
/// trees) to the player's head, inside the cone and within range; fog
/// shortens everyone's range equally.
pub(crate) fn perceive(
    mut robots: Query<(
        Entity,
        &Robot,
        &Senses,
        &GlobalTransform,
        &mut Awareness,
        Has<Stunned>,
    )>,
    players: Query<(Entity, &GlobalTransform, &crate::player::Body), With<Player>>,
    sensors: Query<&RobotPart>,
    parts_of: Query<&Children>,
    mut noises: MessageReader<Noise>,
    spatial: SpatialQuery,
    time: Res<Time>,
    cloaks: Res<crate::gadgets::Cloaks>,
) {
    let dt = time.delta_secs();
    let noises: Vec<Noise> = noises.read().cloned().collect();
    for (robot_e, robot, senses, gt, mut aware, stunned) in &mut robots {
        if robot.wrecked {
            continue;
        }
        if stunned {
            // Senseless: it can't see you, so it can't shoot you.
            aware.can_see_target = false;
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
        aware.can_see_target = false;
        if has_eyes {
            for (pe, pgt, _) in &players {
                // Inside a running cloak, nothing sees you.
                if cloaks.hides(pgt.translation()) {
                    continue;
                }
                let head = pgt.translation() + Vec3::Y * 1.55;
                let to = head - eye;
                let dist = to.length();
                if dist > senses.sight || dist < 0.01 {
                    continue;
                }
                let flat_fwd = forward;
                let angle = flat_fwd.angle_between(to / dist).to_degrees();
                if angle > senses.fov_deg * 0.5 && dist > 4.0 {
                    continue;
                }
                let blocked = Dir3::new(to).ok().and_then(|dir| {
                    spatial.cast_ray(
                        eye,
                        dir,
                        dist - 0.3,
                        true,
                        &SpatialQueryFilter::from_mask([Layer::World]),
                    )
                });
                if blocked.is_none() {
                    aware.can_see_target = true;
                    aware.target = Some(pe);
                    aware.last_known = Some(pgt.translation());
                    aware.unseen = 0.0;
                    // Closer is quicker to notice.
                    aware.suspicion =
                        (aware.suspicion + dt * (1.5 + 30.0 / dist.max(1.0))).min(1.0);
                }
            }
        }
        if !aware.can_see_target {
            aware.unseen += dt;
            aware.suspicion = (aware.suspicion - dt * 0.05).max(0.0);
        }
        for n in &noises {
            let d = n.at.distance(gt.translation());
            if d < n.radius * senses.hearing {
                let loud = 1.0 - d / (n.radius * senses.hearing);
                aware.suspicion = (aware.suspicion + 0.3 + loud * 0.5).min(1.0);
                // Heard, not seen: they know roughly where, not exactly.
                if !aware.can_see_target {
                    aware.last_known = Some(n.at);
                    if aware.target.is_none() {
                        aware.target = n.source.filter(|s| players.contains(*s));
                    }
                }
            }
        }
    }
}

/// Once a wreck has come to rest, its salvage is there for the taking.
#[derive(Component)]
pub struct Salvaged;

fn wreck_loot(
    mut commands: Commands,
    robots: Query<(Entity, &Robot, &GlobalTransform), Without<Salvaged>>,
    loot: Option<Res<crate::items::loot::Loot>>,
    catalog: Res<crate::items::Catalog>,
    mut timers: Local<std::collections::HashMap<Entity, f32>>,
    time: Res<Time>,
    mut rng: Local<Option<extraction_core::noise::Rng>>,
) {
    let Some(loot) = loot else {
        return;
    };
    let rng = rng.get_or_insert_with(|| extraction_core::noise::Rng::new(0x5A1));
    for (e, robot, gt) in &robots {
        if !robot.wrecked {
            continue;
        }
        let t = timers.entry(e).or_insert(0.0);
        *t += time.delta_secs();
        if *t < 3.0 {
            continue;
        }
        commands.entity(e).insert(Salvaged);
        let base = gt.translation() + Vec3::Y * 0.4;
        for (i, drop) in loot
            .0
            .roll(wreck_table(robot.kind), &catalog, rng)
            .into_iter()
            .enumerate()
        {
            let a = i as f32 * 2.1;
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind: drop.kind,
                    transform: Transform::from_translation(
                        base + Vec3::new(a.cos() * 0.4, 0.1 * i as f32, a.sin() * 0.4),
                    ),
                    condition: drop.condition,
                },
            );
        }
    }
}

/// The loot table a wreck of this kind rolls.
pub fn wreck_table(kind: RobotKind) -> &'static str {
    match kind {
        RobotKind::Drone => "drone_wreck",
        RobotKind::Walker => "walker_wreck",
        RobotKind::Dirigible => "dirigible_wreck",
    }
}

/// Builds a part child with a collider.
pub fn part_bundle(
    robot: Entity,
    role: PartRole,
    hp: f32,
    collider: Collider,
    at: Transform,
) -> impl Bundle {
    (
        RobotPart {
            robot,
            role,
            hp,
            max_hp: hp,
        },
        collider,
        CollisionLayers::new(
            Layer::Robot,
            [Layer::World, Layer::Item, Layer::Robot, Layer::Player],
        ),
        at,
        Visibility::default(),
    )
}
