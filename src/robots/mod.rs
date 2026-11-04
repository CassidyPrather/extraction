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
pub mod broken;
pub mod collector;
pub mod damage;
pub mod devices;
pub mod director;
pub mod dirigible;
pub mod disturbed;
pub mod doors;
pub mod drone;
#[cfg(test)]
mod drone_drill;
pub mod flee;
pub mod flight;
pub mod fx;
pub mod grappler;
pub mod grip;
pub mod hazard_look;
pub mod hazards;
pub mod heavy;
pub mod heavy_look;
pub mod heavy_mind;
pub mod hound;
pub mod hound_mind;
pub mod indoor;
#[cfg(test)]
mod intent_roster;
pub mod memory;
pub mod mender;
pub mod mender_look;
#[cfg(test)]
mod mind_tests;
pub mod minelayer;
pub mod minelayer_look;
pub mod network;
pub mod plough;
pub mod plough_bank;
pub mod plough_look;
pub mod pylon;
pub mod recall;
pub mod reel;
pub mod salvage;
pub mod search;
pub mod senses;
pub mod sentry;
pub mod sentry_look;
pub mod signs;
pub mod sorter;
pub mod sorter_look;
pub mod sorter_mind;
pub mod spotter;
pub mod spotter_look;
pub mod squad;
#[cfg(test)]
mod squad_drill;
pub mod subjects;
pub mod tracks;
pub mod visuals;
pub mod walker;
pub mod weak_points;
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

/// No machine's tell before it does harm is shorter than this (s): long
/// enough to see or hear and do something about (a newcomer was hit with
/// nothing to read first: Onboarding's first_jobs_reckless).
/// `tells_tests::every_machines_tell_is_long_enough_to_read` holds each to it.
pub const MIN_TELL: f32 = 0.5;

#[cfg(test)]
mod tells_tests {
    /// Each machine's tell before it hurts: the drone's lock, the walker's
    /// aim and its fire through a wall, the sentry's chirp, the hound's
    /// crouch, the crawler's charge, the heavy's raised arm, the grappler's
    /// wind-up, the sorter's; and the cage's bell before it drops.
    #[test]
    fn every_machines_tell_is_long_enough_to_read() {
        let tells = [
            ("drone lock", super::drone::LOCK_TIME),
            ("walker aim", super::walker::AIM_TIME),
            ("walker through a wall", super::walker::BLIND_TELL),
            ("sentry chirp", extraction_core::sentry::CHIRP),
            ("hound crouch", super::hound_mind::CROUCH_TIME),
            ("crawler charge", super::indoor::ZAP_CHARGE),
            ("heavy's raised arm", super::heavy::RAISE_TIME),
            ("grappler wind-up", super::grappler::WINDUP),
            ("sorter wind-up", super::sorter::WINDUP),
            // Not a machine's, but the same rule: the cage's bell, then the
            // wait before it comes down on whoever's under it (Underground's).
            ("cage bell's wait", crate::world::cage::BELL_WAIT),
            // And an artifact's: the wet aegis fizzes before it shocks.
            ("aegis fizz", extraction_core::anomaly::AEGIS_TELL),
        ];
        let short: Vec<_> = tells
            .iter()
            .filter(|(_, t)| *t < super::MIN_TELL)
            .map(|(w, t)| format!("{w}: {t} s"))
            .collect();
        assert!(short.is_empty(), "under {} s: {short:?}", super::MIN_TELL);
        // And a drone that goes quiet tells again before it fires.
        assert!(super::drone::RETELL > super::drone::BURST_GAP);
    }

    /// The gunners, each to its own rule for what comes after its tell
    /// before a round can hurt you (Robot minds' fair first fights, 1
    /// November): the sentry's and the drone's first burst after every
    /// tell is a harmless warning; the walker's warning is its full aim,
    /// the machine you're meant to respect. The behaviour is held in each
    /// one's own tests (`it_chirps_before_it_fires`, `its_first_burst_warns`,
    /// `a_drones_first_burst_after_a_lock_warns`,
    /// `every_walker_round_comes_a_full_aim_after_its_tell`).
    #[test]
    fn every_gunner_warns_before_it_hurts() {
        use extraction_core::sentry;
        // (who, what hurts first comes this long after its tell at least, s)
        let gunners = [
            (
                "sentry",
                sentry::CHIRP + sentry::ROUND_GAP * (sentry::BURST - 1) as f32 + sentry::BURST_GAP,
            ),
            ("drone", super::drone::LOCK_TIME + super::drone::BURST_GAP),
            ("walker", super::walker::AIM_TIME),
        ];
        let floors = [("sentry", 2.5), ("drone", 3.3), ("walker", 1.5)];
        for ((who, first), (_, floor)) in gunners.iter().zip(floors) {
            assert!(
                *first >= floor - 1e-4,
                "{who}: first harm {first} s after its tell"
            );
        }
        // The sentry's first burst after its chirp is its warning.
        let mut g = sentry::Gun::default();
        assert_eq!(g.step(true, 0.01), sentry::GunAct::Chirp);
        let first = (0..1000)
            .find_map(|_| (g.step(true, 0.01) == sentry::GunAct::Fire).then(|| g.warning()));
        assert_eq!(
            first,
            Some(true),
            "the sentry's first round wasn't a warning"
        );
        // The drone's warning is a whole burst, landing short of you.
        assert!(super::drone::WARNING_SHORT > 1.0 && super::drone::BURST > 0);
    }
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
            .add_message::<damage::RobotStruck>()
            .add_observer(seed_robot)
            .add_message::<Noticed>()
            .add_message::<Noise>()
            .init_resource::<senses::Lighting>()
            .add_plugins((
                hound_mind::HoundMindPlugin,
                flee::FleePlugin,
                sorter_mind::SorterMindPlugin,
                tracks::TracksPlugin,
                signs::SignsPlugin,
                disturbed::DisturbedPlugin,
                memory::MemoryPlugin,
                network::NetworkPlugin,
                devices::DevicesPlugin,
                recall::RecallPlugin,
                squad::SquadPlugin,
                subjects::SubjectsPlugin,
                hazards::HazardsPlugin,
                broken::BrokenPlugin,
            ))
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
                    salvage::SalvagePlugin,
                    grip::GripPlugin,
                    armour::ArmourPlugin,
                    hound::HoundPlugin,
                    sorter::SorterPlugin,
                    heavy::HeavyPlugin,
                    heavy_mind::HeavyMindPlugin,
                    sentry::SentryPlugin,
                    spotter::SpotterPlugin,
                    minelayer::MinelayerPlugin,
                    mender::MenderPlugin,
                    plough::PloughPlugin,
                ),
                search::SearchPlugin,
                indoor::IndoorPlugin,
            ))
            .add_plugins((hazard_look::HazardLookPlugin, weak_points::WeakPointsPlugin))
            .add_systems(
                FixedUpdate,
                (damage_parts, senses::door_noise, perceive, wreck_loot)
                    .chain()
                    .after(crate::weapons::ballistics::BallisticsSystems),
            );
    }
}

/// A blunt blow (a crowbar, a rifle's butt) on an armour plate does this
/// much more to it than its harm: it shakes the plate's bolts loose, so a
/// plate beaten hangs (`armour::Hanging`) and comes off sooner than one
/// shot. Elsewhere a blow is its harm.
pub const BLUNT_LOOSENS: f32 = 1.8;

/// What a hit of `damage` does to a part in this role. A weak point (a
/// heavy's bolt: `weapons::melee::WeakPoint`) has its own multiplier,
/// already in `damage`: no more on top.
pub fn part_damage(
    role: PartRole,
    weak: bool,
    damage: f32,
    blow: Option<extraction_core::melee::Blow>,
) -> f32 {
    match (role, blow) {
        (PartRole::Armour(_), Some(extraction_core::melee::Blow::Blunt)) if !weak => {
            damage * BLUNT_LOOSENS
        }
        _ => damage,
    }
}

/// Bullets (and blows) damage the part they strike.
fn damage_parts(
    mut hits: MessageReader<Hit>,
    mut parts: Query<(
        &mut RobotPart,
        &GlobalTransform,
        Has<crate::weapons::melee::WeakPoint>,
    )>,
    mut robots: Query<&mut Robot>,
    mut destroyed: MessageWriter<PartDestroyed>,
    mut struck: MessageWriter<damage::RobotStruck>,
    mut noise: MessageWriter<Noise>,
    mut minds: MessageWriter<search::MindShift>,
    mut log: ResMut<search::MindLog>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for hit in hits.read() {
        let Ok((mut part, gt, weak)) = parts.get_mut(hit.entity) else {
            continue;
        };
        let dealt = part_damage(part.role, weak, hit.damage, hit.blow);
        // What it did, for the flash, the flinch and the sound: a dead
        // part only shrugs.
        struck.write(damage::RobotStruck {
            robot: part.robot,
            part: hit.entity,
            role: part.role,
            at: hit.point,
            normal: hit.normal,
            dir: hit.velocity.normalize_or_zero(),
            dealt,
            strike: damage::strike(part.role, part.hp, part.max_hp, dealt),
        });
        if !part.alive() {
            continue;
        }
        // Being shot is loud and tells everyone roughly where from.
        noise.write(Noise {
            at: hit.point,
            radius: 40.0,
            source: hit.shooter,
        });
        part.hp -= dealt;
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

/// A robot's own seed, from where and what it was when it came into the
/// world: whatever a robot's body does by chance (which legs tear off,
/// where its pieces fly, how a plate falls) is drawn from this, not from
/// entity ids or query order, so the same raid plays the same however many
/// other things are about.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RobotSeed(pub u64);

impl RobotSeed {
    pub fn for_robot(kind: RobotKind, at: Vec3) -> Self {
        // To the centimetre: a spawn point is a spawn point.
        let q = |v: f32| (v * 100.0).round() as i64 as u64;
        let mut h = 0xCBF2_9CE4_8422_2325u64 ^ kind as u64;
        for v in [q(at.x), q(at.y), q(at.z)] {
            h = (h ^ v).wrapping_mul(0x0100_0000_01B3);
        }
        Self(h)
    }

    /// 32 bits of it, for the small hashes (`wreck::jitter`).
    pub fn low(self) -> u32 {
        (self.0 ^ (self.0 >> 32)) as u32
    }
}

/// A robot's own dice for what it drops (its wreck, a dirigible's cargo
/// pods), seeded from where and what it was when it came into the world:
/// how many other robots there are, and the order a query meets them in,
/// can't change one robot's rolls.
#[derive(Component, Clone, Debug)]
pub struct LootRng(pub extraction_core::noise::Rng);

fn seed_robot(add: On<Add, Robot>, robots: Query<(&Robot, &Transform)>, mut commands: Commands) {
    if let Ok((robot, t)) = robots.get(add.entity) {
        let seed = RobotSeed::for_robot(robot.kind, t.translation);
        commands
            .entity(add.entity)
            .try_insert((seed, LootRng(extraction_core::noise::Rng::new(seed.0))));
    }
}

fn wreck_loot(
    mut commands: Commands,
    mut robots: Query<
        (
            Entity,
            &Robot,
            &GlobalTransform,
            &mut LootRng,
            Has<indoor::Crawler>,
            Has<hound::Hound>,
        ),
        (Without<Salvaged>, Without<heavy::Heavy>),
    >,
    parts: Query<&RobotPart>,
    loot: Option<Res<crate::items::loot::Loot>>,
    catalog: Res<crate::items::Catalog>,
    mut timers: Local<std::collections::HashMap<Entity, f32>>,
    time: Res<Time>,
) {
    let Some(loot) = loot else {
        return;
    };
    // (Salvaged or gone: no timer to keep.)
    timers.retain(|e, _| robots.contains(*e));
    for (e, robot, gt, mut rng, crawler, hound) in &mut robots {
        if !robot.wrecked {
            continue;
        }
        let t = timers.entry(e).or_insert(0.0);
        *t += time.delta_secs();
        if *t < 3.0 {
            continue;
        }
        commands.entity(e).insert(Salvaged);
        timers.remove(&e);
        let base = gt.translation() + Vec3::Y * 0.4;
        let mut drops = loot.roll(wreck_table(robot.kind), &catalog, &mut rng.0);
        // And whatever came through whole is there to unbolt.
        let whole: Vec<PartRole> = parts
            .iter()
            .filter(|p| p.robot == e && p.alive())
            .map(|p| p.role)
            .collect();
        for id in part_salvage(robot.kind, crawler, hound, &whole, &mut rng.0) {
            if let Some(kind) = catalog.kind(id) {
                drops.push(extraction_core::loot::Drop {
                    kind,
                    condition: rng.0.range(0.35, 0.9),
                });
            }
        }
        for (i, drop) in drops.into_iter().enumerate() {
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

/// Parts a wreck gives up besides its table: one chance per part that
/// came through whole (a knee that still bends, a rotor still on its arm),
/// at most two of a kind. A crawler's capacitor bank is in its body, so
/// it's there unless the core went up with it; a hound's legs are too
/// small to be worth the bother.
pub fn part_salvage(
    kind: RobotKind,
    crawler: bool,
    hound: bool,
    whole: &[PartRole],
    rng: &mut extraction_core::noise::Rng,
) -> Vec<&'static str> {
    let count = |f: fn(&PartRole) -> bool| whole.iter().filter(|r| f(r)).count();
    let tries = |n: usize, p: f32, id: &'static str, rng: &mut extraction_core::noise::Rng| {
        (0..n)
            .filter(|_| rng.chance(p))
            .take(2)
            .map(|_| id)
            .collect::<Vec<_>>()
    };
    match kind {
        RobotKind::Walker if crawler => tries(1, 0.5, "crawler_capacitor", rng),
        RobotKind::Walker if hound => Vec::new(),
        // A walker's servo packs are bolted on its wreck, taken off by hand
        // (`salvage`), not dropped.
        RobotKind::Walker => Vec::new(),
        RobotKind::Drone => tries(
            count(|r| matches!(r, PartRole::Rotor(_))),
            0.35,
            "drone_rotor_motor",
            rng,
        ),
        RobotKind::Dirigible => tries(
            count(|r| matches!(r, PartRole::GasCell(_))),
            0.4,
            "dirigible_gas_valve",
            rng,
        ),
        RobotKind::Pylon => tries(
            count(|r| *r == PartRole::Sensor),
            0.6,
            "pylon_relay_coil",
            rng,
        ),
        // Its servo pack is bolted on its wreck (`salvage`).
        RobotKind::Sorter => Vec::new(),
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
            // (Hand: a palm stops on a robot, or shoves it: `interact::fight`.)
            [
                Layer::World,
                Layer::Item,
                Layer::Robot,
                Layer::Player,
                Layer::Hand,
            ],
        ),
        at,
        Visibility::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a walker wreck at `at` drops, with `others` wrecks spawned
    /// (and wrecked) before it elsewhere.
    fn drops_of_a_wreck(others: usize) -> Vec<String> {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let wreck = |app: &mut App, at: Vec3| {
            app.world_mut().spawn((
                Robot {
                    kind: RobotKind::Walker,
                    wrecked: true,
                },
                Transform::from_translation(at),
                GlobalTransform::from_translation(at),
            ));
        };
        for k in 0..others {
            wreck(&mut app, zone.on_ground(60.0 + 6.0 * k as f32, 200.0));
        }
        let at = zone.on_ground(40.0, 230.0);
        wreck(&mut app, at);
        for _ in 0..(90 * 5) {
            app.update();
        }
        let mut kinds: Vec<String> = app
            .world_mut()
            .query::<(&crate::items::Item, &GlobalTransform)>()
            .iter(app.world())
            .filter(|(_, gt)| gt.translation().xz().distance(at.xz()) < 4.0)
            .map(|(item, _)| format!("{:?}", item.kind()))
            .collect();
        kinds.sort();
        kinds
    }

    /// Whole parts give up their own salvage: never more than two of a
    /// kind, nothing from parts that are gone, nothing from a hound (and
    /// nothing loose from a walker: its are on the wreck), and every id
    /// is a real item.
    #[test]
    fn whole_parts_come_off_as_salvage() {
        let mut rng = extraction_core::noise::Rng::new(9);
        let legs: Vec<PartRole> = (0..4).map(PartRole::Leg).collect();
        let rotors: Vec<PartRole> = (0..4).map(PartRole::Rotor).collect();
        let mut seen = 0;
        for _ in 0..200 {
            let got = part_salvage(RobotKind::Drone, false, false, &rotors, &mut rng);
            assert!(got.len() <= 2);
            assert!(got.iter().all(|id| *id == "drone_rotor_motor"));
            seen += got.len();
        }
        // About 0.35 a rotor, four rotors, capped at two.
        assert!((150..320).contains(&seen), "{seen} motors in 200 wrecks");
        assert!(part_salvage(RobotKind::Drone, false, false, &[], &mut rng).is_empty());
        // A walker's servo packs are bolted on (`salvage`), not dropped.
        assert!(part_salvage(RobotKind::Walker, false, false, &legs, &mut rng).is_empty());
        assert!(part_salvage(RobotKind::Walker, false, true, &legs, &mut rng).is_empty());
        let catalog = crate::items::Catalog::load();
        let all = [
            (RobotKind::Walker, false, legs.clone()),
            (RobotKind::Walker, true, vec![]),
            (
                RobotKind::Drone,
                false,
                (0..3).map(PartRole::Rotor).collect(),
            ),
            (
                RobotKind::Dirigible,
                false,
                (0..3).map(PartRole::GasCell).collect(),
            ),
            (RobotKind::Pylon, false, vec![PartRole::Sensor]),
            (RobotKind::Sorter, false, vec![]),
        ];
        for (kind, crawler, whole) in all {
            for _ in 0..50 {
                for id in part_salvage(kind, crawler, false, &whole, &mut rng) {
                    assert!(catalog.kind(id).is_some(), "{id} isn't an item");
                }
            }
        }
    }

    /// A blunt blow beats a plate loose faster than its harm; a blade or a
    /// round does its harm, and a blow elsewhere is its harm.
    #[test]
    fn blunt_blows_shake_plates_loose() {
        use extraction_core::melee::Blow;
        let plate = PartRole::Armour(0);
        assert!(part_damage(plate, false, 10.0, Some(Blow::Blunt)) > 10.0);
        assert_eq!(part_damage(plate, false, 10.0, Some(Blow::Edge)), 10.0);
        assert_eq!(part_damage(plate, false, 10.0, None), 10.0);
        let core = PartRole::Core;
        assert_eq!(part_damage(core, false, 10.0, Some(Blow::Blunt)), 10.0);
        // A heavy's bolt is a weak point: its own multiplier, not this.
        assert_eq!(part_damage(plate, true, 10.0, Some(Blow::Blunt)), 10.0);
    }

    /// A wreck's salvage is its own: other robots wrecked first, somewhere
    /// else, don't change what it drops.
    #[test]
    fn other_wrecks_dont_change_ones_salvage() {
        let alone = drops_of_a_wreck(0);
        assert!(!alone.is_empty(), "it dropped something");
        assert_eq!(drops_of_a_wreck(3), alone);
    }

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
