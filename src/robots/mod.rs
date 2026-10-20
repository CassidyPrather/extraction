//! The Zone's robots: sleek, working, in your way.
//!
//! Every robot is built from **parts** (child entities with colliders on
//! [`Layer::Robot`]): shooting a part damages it, and parts matter — a drone
//! that loses a rotor falls out of the sky, a walker that loses legs limps
//! and then goes down. Robots **perceive** (sight through the fog that
//! crouching and grass defeat, hearing [`Noise`] and footsteps through
//! walls: [`senses`]), remember where you were and search for you
//! ([`search`]), think with a small **brain** state machine around their
//! current **job**, and arrive and leave diegetically through the Gate
//! (see [`director`]).

pub mod anim;
pub mod armour;
pub mod collector;
pub mod director;
pub mod dirigible;
pub mod doors;
pub mod drone;
pub mod flee;
pub mod flight;
pub mod fx;
pub mod grappler;
pub mod heavy;
pub mod heavy_look;
pub mod heavy_mind;
pub mod hound;
pub mod hound_mind;
pub mod indoor;
#[cfg(test)]
mod mind_tests;
pub mod pylon;
pub mod reel;
pub mod search;
pub mod senses;
pub mod sorter;
pub mod sorter_look;
pub mod visuals;
pub mod walker;
pub mod weathering;
pub mod wreck;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::weapons::ballistics::Hit;

use crate::world::Layer;
pub(crate) use senses::perceive;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RobotKind {
    /// Quadcopter with a gun pod.
    Drone,
    /// A sensor orb carried on ball-jointed legs (also the grappler and
    /// the convoy's collector, by their components).
    Walker,
    /// The airship that carries cargo pods.
    Dirigible,
    /// A relay pylon walkers put up at a work site (see `pylon`).
    Pylon,
    /// The Depot's ceiling gantry (see `sorter`).
    Sorter,
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
    /// An armour plate over the body: it takes the rounds that strike it
    /// until it comes loose and drops, exposing what's behind (`armour`).
    Armour(u8),
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
    /// How far from `last_known` you might be by now, metres: small when
    /// just seen, a guess when heard from afar, growing while unseen.
    pub uncertainty: f32,
    /// Seconds since the target was last seen.
    pub unseen: f32,
    /// 0..1 how sure it is something's there.
    pub suspicion: f32,
    pub can_see_target: bool,
    /// How much of its target it makes out this tick (`senses::exposure`).
    pub exposure: f32,
    /// It saw, glimpsed, heard or was told something this tick.
    pub news: bool,
}

/// What a robot's brain wants of its body this tick, for locomotion and
/// for how it looks: where to look when it has nobody in sight, how fast
/// to go (1 = its usual pace), and whether to keep low.
#[derive(Component, Debug, Clone, Copy)]
pub struct Intent {
    pub look: Option<Vec3>,
    pub pace: f32,
    pub low: bool,
}

impl Default for Intent {
    fn default() -> Self {
        Self {
            look: None,
            pace: 1.0,
            low: false,
        }
    }
}

/// A robot's suspicion just rose past [`NOTICED`]: it has noticed someone
/// (for its tell, a sound; the robot itself acts on [`Awareness`]).
#[derive(Message, Clone, Copy, Debug)]
pub struct Noticed {
    pub robot: Entity,
    pub at: Vec3,
}

pub const NOTICED: f32 = 0.6;

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
    /// Going to look at a disturbance (`left`: how long it gives it).
    Investigate { at: Vec3, left: f32 },
    /// Looking round where it was: stands and looks for `dwell`, then the
    /// next of `spots`, for `left` seconds at most (see [`search`]).
    Search {
        at: Vec3,
        left: f32,
        spots: Vec<Vec3>,
        dwell: f32,
    },
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
    /// Put up a relay pylon at a work site, run it, take it down.
    Construct,
}

pub struct RobotsPlugin;

impl Plugin for RobotsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PartDestroyed>()
            .add_message::<Noticed>()
            .add_message::<Noise>()
            .init_resource::<senses::Lighting>()
            .add_plugins((hound_mind::HoundMindPlugin, flee::FleePlugin))
            .add_plugins((
                drone::DronePlugin,
                walker::WalkerPlugin,
                director::DirectorPlugin,
                collector::CollectorPlugin,
                dirigible::DirigiblePlugin,
                grappler::GrapplerPlugin,
                doors::DoorsPlugin,
                pylon::PylonPlugin,
                (
                    anim::AnimPlugin,
                    flight::FlightPlugin,
                    wreck::WreckPlugin,
                    armour::ArmourPlugin,
                    hound::HoundPlugin,
                    sorter::SorterPlugin,
                    heavy::HeavyPlugin,
                    heavy_mind::HeavyMindPlugin,
                ),
                search::SearchPlugin,
                indoor::IndoorPlugin,
            ))
            .add_systems(
                FixedUpdate,
                (
                    damage_parts,
                    senses::door_noise,
                    perceive,
                    senses::gunshot_noise,
                    wreck_loot,
                )
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
    mut minds: MessageWriter<search::MindShift>,
    mut log: ResMut<search::MindLog>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
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
                log.shift(
                    &mut minds,
                    part.robot,
                    search::Mind::Dying,
                    gt.translation(),
                    now,
                );
            }
        } else {
            log.shift(
                &mut minds,
                part.robot,
                search::Mind::Hurt,
                gt.translation(),
                now,
            );
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
        RobotKind::Pylon => "pylon_wreck",
        // (Its own table when Models: kit's sorter parts are in.)
        RobotKind::Sorter => "pylon_wreck",
    }
}

/// A robot that is its own breed on a shared body (a crawler, a hound are
/// walkers underneath): what telemetry and logs call it.
#[derive(Component, Debug, Clone, Copy)]
pub struct Breed(pub &'static str);

/// What telemetry and logs call a robot: its breed, else its kind.
pub fn robot_name(kind: RobotKind, breed: Option<&Breed>) -> String {
    match breed {
        Some(b) => b.0.into(),
        None => format!("{kind:?}"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Heard(Vec<Noticed>);

    fn hear(mut n: MessageReader<Noticed>, mut heard: ResMut<Heard>) {
        heard.0.extend(n.read().copied());
    }

    /// A noise close by makes a robot notice, once: the tell plays as its
    /// suspicion passes the mark, not every tick it stays there.
    #[test]
    fn a_robot_notices_once() {
        let mut app = crate::headless_app(Default::default(), false);
        app.init_resource::<Heard>().add_systems(Last, hear);
        app.finish();
        app.cleanup();
        app.update();
        let at = Vec3::new(0.0, 500.0, 0.0);
        let robot = app
            .world_mut()
            .spawn((
                Robot {
                    kind: RobotKind::Drone,
                    wrecked: false,
                },
                Senses {
                    sight: 0.0,
                    fov_deg: 90.0,
                    hearing: 1.0,
                    eye: Vec3::ZERO,
                },
                Awareness::default(),
                Transform::from_translation(at),
                GlobalTransform::from_translation(at),
            ))
            .id();
        for _ in 0..5 {
            app.world_mut().write_message(Noise {
                at: at + Vec3::X,
                radius: 50.0,
                source: None,
            });
            app.update();
        }
        let heard = &app.world().resource::<Heard>().0;
        assert_eq!(heard.len(), 1, "{heard:?}");
        assert_eq!(heard[0].robot, robot);
    }
}
