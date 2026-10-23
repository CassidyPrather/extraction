//! The walker: a sensor orb carried on four ball-jointed legs.
//!
//! Locomotion is procedural. The orb glides along an A* path; each foot
//! stays planted until the place it *should* be has drifted too far, then
//! it lifts and arcs to a new footing on the real ground. Legs step in
//! diagonal pairs. Knees come from two-bone IK. Shoot a leg off and it
//! hangs limp; lose two and the orb comes down.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::nav::NavGrid;
use std::sync::Arc;

use super::indoor::NavRef;
use super::search::{self, Step};
use super::{
    Awareness, Brain, Intent, Job, PartDestroyed, PartRole, Robot, RobotKind, RobotPart, Senses,
    part_bundle,
};
use crate::weapons::ballistics::Shot;
use crate::world::Zone;

pub use super::anim::solve_knee;
use super::anim::{BODY_HEIGHT, LEGS, rest_offset};

pub const UPPER: f32 = 0.8;
pub const LOWER: f32 = 0.95;
/// Suspicion at which a walker that sees you opens fire.
pub const ENGAGE_AT: f32 = 0.55;
/// Out of sight this long in a fight, it goes to where it last saw you.
pub const PATIENCE: f32 = 8.0;
/// The turret settles on you this long before its first shot (and again
/// after you've been out of sight a while): the whine that says move.
pub const AIM_TIME: f32 = 0.7;
/// Its aim is thrown off by this (rad) a m/s its target runs across its
/// line of fire: sprinting across it, most rounds miss.
pub const CROSSING_SPREAD: f32 = 0.02;
/// Out of its sight this long (s), it has to aim afresh once you're back.
pub const REACQUIRE: f32 = 0.5;
/// Seeing you for the first time in a fight, it stops and looks this long
/// (its eye and the Heard line have their moment) before it settles its
/// turret on you: no first shot inside a newcomer's reaction time.
pub const SIGHT_BEAT: f32 = 1.5;
/// Fire through a wall: a walker that lost sight of you under a roof
/// moments ago (at least [`BLIND_AFTER`], at most [`BLIND_WITHIN`]
/// seconds), and knows within [`BLIND_SURE`] m where you are, within
/// [`BLIND_RANGE`] m, settles its turret [`BLIND_TELL`] s (the same tell
/// as a shot) and fires [`BLIND_ROUNDS`] through it, if a round would get
/// through (`destruction::through`): once each time it loses you.
pub const BLIND_AFTER: f32 = 0.3;
pub const BLIND_WITHIN: f32 = 4.0;
pub const BLIND_SURE: f32 = 3.0;
pub const BLIND_RANGE: f32 = 12.0;
pub const BLIND_TELL: f32 = 0.4;
pub const BLIND_ROUNDS: u8 = 3;
/// Seconds between them, and their spread (wider than aimed fire).
const BLIND_EVERY: f32 = 0.25;
/// Seconds between its rounds in a fight...
pub const FIRE_EVERY: f32 = 1.1;
/// ...and while it pins you for a squadmate moving up (`squad`): faster,
/// steadier, a cadence you hear change...
pub const SUPPRESS_EVERY: f32 = 0.45;
pub const SUPPRESS_SPREAD: f32 = 0.015;
/// ...kept on where it last saw you this long after you drop from sight (s).
pub const SUPPRESS_HOLD: f32 = 4.0;
const BLIND_SPREAD: f32 = 0.06;
/// A walker round, for the check: 8 g at the turret's 450 m/s.
const ROUND_G: f32 = 8.0;
const ROUND_SPEED: f32 = 450.0;

/// Where a walker is in a burst through a wall.
#[derive(Debug, Default, Clone, Copy)]
struct Blind {
    /// Settling (the tell), then firing.
    tell: f32,
    left: u8,
    /// Done (or not worth it) for this loss of sight.
    spent: bool,
    /// Where it fires.
    at: Vec3,
}
/// Lamer than this (`anim::Gait::limp`), a walker fights from where it
/// stands and doesn't chase.
pub const LAME: f32 = 0.5;
/// It fights from cover at about this range from you (m)...
pub const COVER_RANGE: (f32, f32) = (14.0, 34.0);
/// ...and looks for better cover this often (s).
pub const COVER_RETHINK: f32 = 4.0;

/// The shared navigation grid for ground robots (and bots).
#[derive(Resource, Clone)]
pub struct Nav(pub Arc<NavGrid>);

#[derive(Clone, Copy, Debug)]
pub struct Foot {
    pub planted: Vec3,
    pub from: Vec3,
    pub to: Vec3,
    /// 0..1 through a step; 1 when planted.
    pub t: f32,
    pub alive: bool,
}

/// The body (`anim`) moves it; see [`super::Intent`] for what
/// the brain can ask of it besides a path.
#[derive(Component, Debug)]
#[require(super::anim::Gait)]
pub struct Walker {
    pub feet: [Foot; LEGS],
    pub path: Vec<Vec3>,
    pub repath: f32,
    pub velocity: Vec3,
    pub yaw: f32,
    pub gun_cooldown: f32,
    /// Pinning you for a squadmate moving up (`Suppressing`).
    pub suppressing: bool,
    pub turret_yaw: f32,
    /// Which diagonal pair may step next.
    pub phase: usize,
    pub collapsed: bool,
    pub goal: Option<Vec3>,
    /// Seconds its turret has been settling on its target.
    pub aim: f32,
    /// The cover it's fighting from, if any, and when to think again.
    pub cover: Option<Vec3>,
    pub rethink: f32,
    /// Seconds a grappler has lain in wait for you to come closer.
    pub waited: f32,
}

/// A walker's turret began settling on someone (the tell: audio whines,
/// the eye flares).
#[derive(Message, Clone, Copy, Debug)]
pub struct WalkerAim {
    pub walker: Entity,
    pub at: Vec3,
}

/// A leg's collider and visual root, posed every tick from the IK.
#[derive(Component)]
pub struct WalkerLeg {
    pub index: usize,
}

#[derive(Component)]
pub struct WalkerTurret;

/// Joint positions of each leg, for visuals: hip, knee, foot (world).
#[derive(Component, Default, Debug)]
pub struct LegPoses(pub [[Vec3; 3]; LEGS]);

/// A walker's foot came down (for sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct WalkerStep {
    pub at: Vec3,
    /// Whose foot, and which way it was facing (radians about +Y, 0 = -Z).
    pub by: Entity,
    pub yaw: f32,
}

pub struct WalkerPlugin;

impl Plugin for WalkerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<WalkerStep>()
            .add_message::<WalkerAim>()
            .init_resource::<NavGone>()
            .add_systems(
                Startup,
                build_nav.after(crate::world::trees::spawn_tree_colliders),
            )
            .add_systems(FixedUpdate, reopen_nav.before(walker_brain))
            .add_systems(
                FixedUpdate,
                (
                    walker_parts_destroyed,
                    walker_brain,
                    super::hound_mind::spring_hounds,
                    super::anim::walk,
                    super::anim::aim_turrets,
                    walker_guns,
                )
                    .chain()
                    .before(PhysicsSystems::First),
            );
    }
}

/// Navigation for ground robots: terrain, structures and tree trunks
/// (`extraction_core::map_nav`, which the map lint checks routes on too).
pub fn build_nav(
    mut commands: Commands,
    zone: Res<Zone>,
    opts: Option<Res<crate::launch::LaunchOptions>>,
) {
    let started = std::time::Instant::now();
    let mut g = extraction_core::map_nav::zone_grid(
        &zone.map,
        &zone.hm,
        extraction_core::map_nav::MAX_SLOPE,
    );
    // Story's scenes stand where the map doesn't say (a row of buses).
    let map = opts.map_or("", |o| o.into_inner().map.as_str());
    for ob in crate::world::scenes::nav_obstacles(&zone, map) {
        g.block(ob, 0.6);
    }
    info!(
        "navigation grid built in {:.0} ms",
        started.elapsed().as_secs_f32() * 1000.0
    );
    commands.insert_resource(Nav(Arc::new(g)));
}

/// A placement no bigger than this across (m) that's cleared away (a crate
/// burst: `destruction::Cleared`) comes off the grid...
pub const GONE_SIZE: f32 = 3.0;

/// ...these ones, by index in the map's placements; the grid is built again
/// without them, off the main thread, and swapped in when it's ready.
#[derive(Resource, Default)]
pub struct NavGone {
    pub placements: std::collections::HashSet<usize>,
    dirty: bool,
    building: Option<bevy::tasks::Task<extraction_core::nav::NavGrid>>,
}

/// Something solid is gone: what stood there that was a small placement
/// comes off the grid, so walkers path through where it was and stop
/// counting it as cover.
fn reopen_nav(
    mut cleared: MessageReader<crate::destruction::Cleared>,
    zone: Res<Zone>,
    mut gone: ResMut<NavGone>,
    nav: Option<ResMut<Nav>>,
) {
    let Some(mut nav) = nav else {
        cleared.clear();
        return;
    };
    for c in cleared.read() {
        for (i, p) in zone.map.placements.iter().enumerate() {
            let f = extraction_core::map::prefab_footprint(&p.prefab);
            let small = f[0].max(f[2]) <= GONE_SIZE;
            let inside = p.at[0] >= c.min.x - 0.3
                && p.at[0] <= c.max.x + 0.3
                && p.at[1] >= c.min.z - 0.3
                && p.at[1] <= c.max.z + 0.3;
            if small && inside && gone.placements.insert(i) {
                gone.dirty = true;
            }
        }
    }
    if let Some(task) = gone.building.as_mut() {
        if task.is_finished() {
            let task = gone.building.take().unwrap();
            nav.0 = Arc::new(bevy::tasks::block_on(task));
        }
    }
    if gone.dirty && gone.building.is_none() {
        gone.dirty = false;
        let mut map = (*zone.map).clone();
        let mut k = 0;
        map.placements.retain(|_| {
            k += 1;
            !gone.placements.contains(&(k - 1))
        });
        let hm = zone.hm.clone();
        gone.building = Some(bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
            extraction_core::map_nav::zone_grid(&map, &hm, extraction_core::map_nav::MAX_SLOPE)
        }));
    }
}

pub fn spawn_walker(commands: &mut Commands, zone: &Zone, at: Vec3, job: Job) -> Entity {
    spawn_walker_with(commands, zone, at, job, true, 130.0)
}

/// A walker body: `armed` gives it the turret; `core_hp` is how much it
/// takes to bring down (the convoy collector is sturdier and unarmed).
pub fn spawn_walker_with(
    commands: &mut Commands,
    zone: &Zone,
    at: Vec3,
    job: Job,
    armed: bool,
    core_hp: f32,
) -> Entity {
    spawn_walker_sized(commands, zone, at, job, armed, core_hp, 1.0, None)
}

/// [`spawn_walker_with`] at `size` times a walker's size (a crawler for
/// low ceilings is about 0.6): the body, its legs, gait and wreck all
/// scale with it. `floor` is the height it stands on underground (on an
/// underground site's floor, below the terrain); None stands it on the
/// terrain.
#[allow(clippy::too_many_arguments)]
pub fn spawn_walker_sized(
    commands: &mut Commands,
    zone: &Zone,
    at: Vec3,
    job: Job,
    armed: bool,
    core_hp: f32,
    size: f32,
    floor: Option<f32>,
) -> Entity {
    let floor = |x: f32, z: f32| floor.unwrap_or_else(|| zone.ground(x, z));
    let pos = Vec3::new(at.x, floor(at.x, at.z) + BODY_HEIGHT * size, at.z);
    let feet = std::array::from_fn(|i| {
        let p = pos + rest_offset(i) * size;
        let f = Vec3::new(p.x, floor(p.x, p.z), p.z);
        Foot {
            planted: f,
            from: f,
            to: f,
            t: 1.0,
            alive: true,
        }
    });
    let robot = commands
        .spawn((
            Name::new("Walker"),
            Robot {
                kind: RobotKind::Walker,
                wrecked: false,
            },
            Walker {
                feet,
                path: Vec::new(),
                repath: 0.0,
                velocity: Vec3::ZERO,
                yaw: 0.0,
                gun_cooldown: 2.0,
                suppressing: false,
                turret_yaw: 0.0,
                phase: 0,
                collapsed: false,
                goal: None,
                aim: 0.0,
                cover: None,
                rethink: 0.0,
                waited: 0.0,
            },
            Intent::default(),
            LegPoses::default(),
            Senses {
                sight: 75.0,
                fov_deg: 200.0,
                hearing: 1.2,
                eye: Vec3::new(0.0, 0.15, -0.3),
            },
            Awareness::default(),
            Brain::Transit { to: job.site },
            job,
            Transform::from_translation(pos).with_scale(Vec3::splat(size)),
            super::anim::Gait::sized(size),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Walker orb"),
            part_bundle(
                robot,
                PartRole::Core,
                core_hp,
                Collider::sphere(0.4),
                Transform::IDENTITY,
            ),
        ));
        c.spawn((
            Name::new("Walker eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                25.0,
                Collider::cuboid(0.3, 0.08, 0.06),
                Transform::from_xyz(0.0, 0.12, -0.36),
            ),
        ));
        if armed {
            c.spawn((
                Name::new("Walker turret"),
                WalkerTurret,
                part_bundle(
                    robot,
                    PartRole::Weapon,
                    45.0,
                    Collider::cuboid(0.16, 0.12, 0.4),
                    Transform::from_xyz(0.0, 0.46, 0.0),
                ),
            ));
        }
        for i in 0..LEGS {
            c.spawn((
                Name::new(format!("Walker leg {i}")),
                WalkerLeg { index: i },
                part_bundle(
                    robot,
                    PartRole::Leg(i as u8),
                    35.0,
                    Collider::capsule(0.06, LOWER),
                    Transform::IDENTITY,
                ),
            ));
        }
    });
    robot
}

fn walker_parts_destroyed(
    mut events: MessageReader<PartDestroyed>,
    mut walkers: Query<(&mut Walker, &mut Robot, Has<super::heavy::Heavy>)>,
) {
    for ev in events.read() {
        let Ok((mut w, mut robot, heavy)) = walkers.get_mut(ev.robot) else {
            continue;
        };
        match ev.role {
            PartRole::Leg(i) => {
                w.feet[i as usize].alive = false;
                // On two it crawls (`anim`); on one it's done. (The heavy
                // can't crawl: two knees out and it's down for good.)
                let lost = w.feet.iter().filter(|f| !f.alive).count();
                if lost >= if heavy { 2 } else { 3 } {
                    w.collapsed = true;
                    robot.wrecked = true;
                }
            }
            PartRole::Core => {
                w.collapsed = true;
                robot.wrecked = true;
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walker_brain(
    mut commands: Commands,
    mut walkers: Query<(
        Entity,
        &mut Walker,
        &mut Brain,
        &mut Job,
        &Awareness,
        &Transform,
        &Robot,
        &mut Intent,
        Option<&mut super::doors::Battering>,
        Has<super::grappler::Grappler>,
        (
            Option<&super::indoor::LocalNav>,
            Option<&mut super::indoor::Patrol>,
            Has<super::indoor::Crawler>,
            Option<&super::anim::Gait>,
            Option<&mut super::hound_mind::HoundMind>,
            Option<&super::flee::Fleeing>,
            (Has<super::indoor::Dormant>, Has<super::indoor::Warden>),
        ),
    )>,
    targets: Query<&GlobalTransform>,
    (nav, wary): (Res<Nav>, Option<Res<super::hazards::Wary>>),
    time: Res<Time>,
    mut convoy: MessageWriter<super::collector::ConvoyEvent>,
    entrances: Res<super::doors::Entrances>,
    mut doors: Query<(
        &mut crate::world::buildings::Door,
        Has<super::doors::Wedged>,
    )>,
    mut noise: MessageWriter<super::Noise>,
    mut raid: ResMut<crate::raid::Raid>,
    mut blows: MessageWriter<super::doors::DoorBlow>,
    spatial: SpatialQuery,
    (
        mut shots,
        mut crouches,
        mut bites,
        mut seed,
        minds,
        (flanking, roles),
        (parts, retreating, mut floorings),
    ): (
        MessageWriter<Shot>,
        MessageWriter<super::hound::HoundCrouch>,
        MessageWriter<super::hound_mind::HoundBite>,
        Local<u64>,
        Query<&super::senses::MindSeed>,
        (Query<(), With<Flanker>>, Query<&super::squad::SquadRole>),
        (
            Query<&RobotPart>,
            Query<(), With<Retreating>>,
            Local<std::collections::HashMap<Entity, (bool, f32)>>,
        ),
    ),
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    // Each walker: on the ground now, and when it last went down.
    floorings.retain(|e, _| walkers.contains(*e));
    let sweep = time.elapsed_secs() * 1.1;
    let mut paths_left = PATH_BUDGET;
    // How much core each robot has left (0..1).
    let cores: std::collections::HashMap<Entity, f32> = parts
        .iter()
        .filter(|p| p.role == PartRole::Core)
        .map(|p| (p.robot, (p.hp / p.max_hp.max(1.0)).max(0.0)))
        .collect();
    // Hounds hunting someone, for splitting up: (who, whom, seed).
    let pack: Vec<(Entity, Entity, u64)> = walkers
        .iter()
        .filter_map(
            |(e, _, brain, _, aware, _, robot, _, _, _, (_, _, _, _, hound, ..))| {
                (hound.is_some() && !robot.wrecked && matches!(brain, Brain::Engage))
                    .then_some(())
                    .and(aware.target)
                    .map(|p| (e, p, minds.get(e).map_or(e.to_bits(), |m| m.0)))
            },
        )
        .collect();
    for (
        e,
        mut w,
        mut brain,
        mut job,
        aware,
        t,
        robot,
        mut intent,
        mut battering,
        grappler,
        (local, mut patrol, crawler, gait, mut hound, fleeing, (dormant, warden)),
    ) in &mut walkers
    {
        // (The heavy thinks for itself: `heavy_mind`.)
        if gait.is_some_and(|g| g.shape == super::anim::Shape::Heavy) {
            continue;
        }
        let is_hound = hound.is_some();
        if let Some(h) = hound.as_deref_mut() {
            h.cooldown -= dt;
        }
        if !matches!(*brain, Brain::Engage) && flanking.contains(e) {
            commands.entity(e).remove::<Flanker>();
        }
        // A site's own grid, or the Zone's: the one wary of anomalies
        // (`hazards::Wary`), unless it's chasing you hard, when it comes
        // straight and risks them.
        let hard = super::hazards::chasing_hard(
            &brain,
            aware,
            aware
                .target
                .and_then(|p| targets.get(p).ok())
                .map(|g| g.translation()),
            t.translation,
        );
        let zone_grid = match (&wary, hard) {
            (Some(w), false) => &w.0,
            _ => &nav.0,
        };
        let navr = local.map_or(NavRef::Zone(zone_grid), NavRef::Local);
        if robot.wrecked {
            *brain = Brain::Down;
            w.goal = None;
            continue;
        }
        let pos = t.translation;
        *intent = Intent::default();
        // Asleep in its bay: still, low.
        if dormant {
            intent.low = true;
            w.goal = None;
            w.path.clear();
            continue;
        }
        // A live grenade it knows about: run, out past its reach.
        if let Some(f) = fleeing {
            intent.pace = 1.4;
            intent.look = Some(f.from);
            let reach = f.radius + super::flee::FLEE_MARGIN;
            if w.goal.is_none_or(|g| g.distance(f.from) < reach) || w.path.is_empty() {
                let to = f.away(pos);
                w.goal = Some(to);
                w.repath = 3.0;
                w.path = navr.path(pos, to).unwrap_or_default();
            }
            continue;
        }
        // Knocked off its feet (`anim::Gait::daze`): it holds. Up again,
        // it fights from fresh cover (where it was may not be cover from
        // where you are now). Down twice inside KNOCKED_AGAIN, it's had
        // enough and breaks off.
        if let Some(g) = gait {
            let down = g.down();
            let (was, last) = floorings
                .get(&e)
                .copied()
                .unwrap_or((false, f32::NEG_INFINITY));
            if down && !was {
                if now - last < KNOCKED_AGAIN && !retreating.contains(e) {
                    commands.entity(e).insert(Retreating);
                }
                floorings.insert(e, (true, now));
            } else if !down && was {
                floorings.insert(e, (false, last));
            }
            if down || g.rising > 0.0 {
                w.cover = None;
                w.rethink = 0.0;
                w.goal = None;
                w.path.clear();
                continue;
            }
        }
        // Lame, or down to two legs (it crawls: `anim`), it fights from
        // where it is: no chasing, no going round your side.
        let lame =
            gait.is_some_and(|g| g.limp > LAME) || w.feet.iter().filter(|f| f.alive).count() <= 2;
        if lame && flanking.contains(e) {
            commands.entity(e).remove::<Flanker>();
        }
        // The collector has one job and is not to be distracted from it.
        let single_minded = job.kind == super::JobKind::Collect;
        if !single_minded {
            search::react(&mut brain, aware, ENGAGE_AT);
            // A lame walker doesn't give chase, and gives up sooner.
            let patience = if lame { PATIENCE * 0.5 } else { PATIENCE };
            search::lost_sight(&mut brain, aware, patience, pos);
        }
        if !matches!(*brain, Brain::Engage) {
            w.cover = None;
        }
        let ambushing = crawler && matches!(*brain, Brain::Investigate { .. });
        if !matches!(*brain, Brain::Engage) && !ambushing {
            w.waited = 0.0;
        }
        // Where the one it's after is hiding, if that's indoors.
        let mut indoors: Option<Vec3> = None;
        // Arrived at a doorway it was looking for someone through.
        let mut at_door = false;
        // Lying in wait where it is (a crawler's ambush).
        let mut continue_goal = false;
        let goal = match &mut *brain {
            Brain::Transit { to } => {
                let to = *to;
                if Vec2::new(pos.x - to.x, pos.z - to.z).length() < 3.0 {
                    // Hounds sent to a noise nose round where it was.
                    *brain = if is_hound && job.kind == super::JobKind::Response {
                        search::begin_search(to, aware, pos, navr, &entrances)
                    } else {
                        Brain::Working {
                            at: to,
                            left: job.duration,
                        }
                    };
                }
                Some(to)
            }
            Brain::Working { left, .. } => {
                *left -= dt;
                if *left <= 0.0 {
                    if single_minded {
                        convoy.write(super::collector::ConvoyEvent::Loaded { robot: e });
                    }
                    *brain = Brain::Leave { to: job.exit };
                }
                // On its rounds.
                patrol.as_deref_mut().and_then(|p| {
                    let (go, look) = p.step(pos, dt, sweep);
                    intent.pace = if warden {
                        super::indoor::WARDEN_PACE
                    } else {
                        super::indoor::PATROL_PACE
                    };
                    intent.look = look;
                    go
                })
            }
            Brain::Investigate { at, left } => {
                *left -= dt;
                let at = *at;
                // Careful, not hurried; a grappler creeps, low.
                intent.pace = if grappler { 0.6 } else { 0.8 };
                intent.low = grappler;
                intent.look = Some(at);
                // A hound runs to a sound, then stalks the last of it, low.
                if is_hound {
                    let far = Vec2::new(pos.x - at.x, pos.z - at.z).length()
                        > super::hound_mind::STALK_WITHIN;
                    intent.pace = super::hound_mind::RUN
                        * if far {
                            1.0
                        } else {
                            super::hound_mind::STALK_PACE
                        };
                    intent.low = !far;
                }
                indoors = Some(at);
                // A crawler that hears you close by in the dark keeps still
                // and low, facing it, and lets you come to it (a while).
                let near =
                    Vec2::new(pos.x - at.x, pos.z - at.z).length() < super::indoor::ZAP_RANGE + 3.0;
                if crawler && near && w.waited < super::indoor::AMBUSH {
                    w.waited += dt;
                    intent.low = true;
                    continue_goal = true;
                }
                // (Indoors, "there" is through the door: see below.)
                let there = Vec2::new(pos.x - at.x, pos.z - at.z).length() < 3.0
                    && entrances.building_at(at).is_none();
                if continue_goal {
                    None
                } else if there || *left <= 0.0 {
                    *brain = search::begin_search(at, aware, pos, navr, &entrances);
                    None
                } else {
                    Some(at)
                }
            }
            Brain::Search { .. } => {
                intent.pace = if is_hound {
                    super::hound_mind::NOSE_PACE * super::hound_mind::RUN
                } else {
                    0.7
                };
                intent.low = grappler || is_hound;
                match search::search_step(&mut brain, pos, 2.0, dt, sweep) {
                    Step::Go(p) => Some(p),
                    Step::Look(l) => {
                        intent.look = Some(l);
                        // At a doorway: open it and look in.
                        if let Some(ent) = entrances.doors.iter().find(|d| {
                            let s = d.mid + d.out * 1.8;
                            Vec2::new(pos.x - s.x, pos.z - s.z).length() < 2.6
                        }) {
                            intent.look = Some(ent.mid - ent.out * 3.0);
                            if let Ok((mut d, wedged)) = doors.get_mut(ent.door) {
                                if !wedged && !d.open {
                                    d.open = true;
                                }
                            }
                        }
                        None
                    }
                    Step::Done => {
                        search::give_up(&mut brain, &mut job);
                        None
                    }
                }
            }
            Brain::Engage => {
                let seen = aware
                    .target
                    .filter(|_| aware.can_see_target)
                    .and_then(|p| targets.get(p).ok())
                    .map(|g| g.translation());
                match (seen, aware.last_known) {
                    (Some(tp), _) if grappler && !lame => {
                        // Lie in wait for you to come within reach; spring
                        // when you do, or once it's waited long enough.
                        let d = Vec2::new(tp.x - pos.x, tp.z - pos.z).length();
                        let reach = super::grappler::TETHER_RANGE;
                        if d < reach * 0.75 {
                            None
                        } else if d < reach * 1.3 || w.waited > 10.0 {
                            intent.pace = 1.4;
                            Some(tp)
                        } else {
                            w.waited += dt;
                            intent.low = true;
                            None
                        }
                    }
                    (Some(_), _) if lame => None,
                    (Some(tp), _) if is_hound => {
                        let h = hound.as_deref_mut().unwrap();
                        let d = Vec2::new(tp.x - pos.x, tp.z - pos.z).length();
                        intent.look = Some(tp);
                        if h.crouch > 0.0 {
                            // The tell: down on its haunches, then it springs.
                            h.crouch -= dt;
                            intent.low = true;
                            if h.crouch <= 0.0 {
                                h.lunge = super::hound_mind::LUNGE_TIME;
                                h.bit = false;
                                h.spring = Some(tp);
                            }
                            None
                        } else if h.lunge > 0.0 {
                            h.lunge -= dt;
                            // Flat out (its gait holds it to what it can).
                            intent.pace = super::hound_mind::RUN * 1.6;
                            if d < super::hound_mind::BITE_RANGE && !h.bit {
                                h.bit = true;
                                h.cooldown = super::hound_mind::BITE_COOLDOWN;
                                *seed = super::senses::shot_seed(&minds, e, &time);
                                let head = pos + Vec3::Y * 0.2;
                                super::hound_mind::bite(
                                    e,
                                    head,
                                    tp + Vec3::Y * 0.9,
                                    *seed,
                                    &mut shots,
                                );
                                bites.write(super::hound_mind::HoundBite { hound: e, at: head });
                            }
                            Some(tp)
                        } else if d < super::hound_mind::LUNGE_RANGE && h.cooldown <= 0.0 {
                            h.crouch = super::hound_mind::CROUCH_TIME;
                            crouches.write(super::hound::HoundCrouch { hound: e, at: tp });
                            None
                        } else if h.cooldown > 0.0 && d < super::hound_mind::LUNGE_RANGE {
                            // Backs off between bites.
                            let away = (pos - tp).with_y(0.0).normalize_or(Vec3::X);
                            Some(pos + away * 3.0)
                        } else {
                            intent.pace = super::hound_mind::RUN;
                            let facing = aware
                                .target
                                .and_then(|p| targets.get(p).ok())
                                .map(|g| g.forward().as_vec3())
                                .unwrap_or(Vec3::NEG_Z);
                            // Two on one: one comes round each side.
                            let side = aware.target.and_then(|p| {
                                let mut mates: Vec<&(Entity, Entity, u64)> =
                                    pack.iter().filter(|m| m.1 == p).collect();
                                mates.sort_by_key(|m| (m.2, m.0));
                                let rank = mates.iter().position(|m| m.0 == e)?;
                                (mates.len() > 1).then_some(if rank % 2 == 0 { 1.0 } else { -1.0 })
                            });
                            Some(super::hound_mind::flank_point_side(pos, tp, facing, side))
                        }
                    }
                    (None, Some(lk)) if lame => {
                        intent.look = Some(lk);
                        None
                    }
                    (Some(tp), _) if crawler => {
                        // Close in to spark range, then hold.
                        let d = Vec2::new(tp.x - pos.x, tp.z - pos.z).length();
                        (d > super::indoor::CRAWLER_HOLD).then_some(tp)
                    }
                    (Some(tp), _) => {
                        // Badly hurt, it breaks off: back to cover away from
                        // you (and says so), still shooting.
                        let hurt = cores.get(&e).is_some_and(|c| *c < RETREAT_AT)
                            || retreating.contains(e);
                        if hurt && !retreating.contains(e) {
                            commands.entity(e).insert(Retreating);
                            w.rethink = 0.0;
                        }
                        // Its part in the squad's plan (`squad`): the
                        // pinner holds, a flanker works round to your
                        // side, a cut-off makes for your way out.
                        let role = roles
                            .get(e)
                            .ok()
                            .filter(|r| !hurt && Some(r.target) == aware.target);
                        let flank = role
                            .filter(|r| {
                                matches!(
                                    r.role,
                                    super::squad::Role::Flank | super::squad::Role::CutOff
                                )
                            })
                            .map(|r| r.post);
                        let flanker = role.is_some_and(|r| r.role == super::squad::Role::Flank);
                        if flanker != flanking.contains(e) {
                            if flanker {
                                commands.entity(e).insert(Flanker);
                                w.rethink = 0.0;
                            } else {
                                commands.entity(e).remove::<Flanker>();
                            }
                        }
                        // Fight from cover: move between pieces of it, in range.
                        w.rethink -= dt;
                        if w.rethink <= 0.0 {
                            w.rethink = COVER_RETHINK;
                            w.cover = match (hurt, flank) {
                                (true, _) => {
                                    let away = (pos - tp).with_y(0.0).normalize_or(Vec3::X);
                                    let back = pos + away * RETREAT_DIST;
                                    pick_cover(back, tp, navr, &spatial).or(Some(back))
                                }
                                (false, Some(f)) => pick_cover(f, tp, navr, &spatial).or(Some(f)),
                                (false, None) => pick_cover(pos, tp, navr, &spatial),
                            };
                        }
                        let d = Vec2::new(tp.x - pos.x, tp.z - pos.z).length();
                        match w.cover {
                            Some(c) if Vec2::new(c.x - pos.x, c.z - pos.z).length() > 1.2 => {
                                intent.pace = 1.25;
                                Some(c)
                            }
                            Some(_) => None,
                            None if d > COVER_RANGE.1 => Some(tp),
                            None => None,
                        }
                    }
                    (None, Some(lk)) => {
                        // Out of sight: push to where it last saw you
                        // (through the door, if that's indoors), and
                        // search from there.
                        indoors = Some(lk);
                        intent.low = grappler || is_hound;
                        intent.look = Some(lk);
                        if Vec2::new(pos.x - lk.x, pos.z - lk.z).length() < 3.0 {
                            *brain = search::begin_search(lk, aware, pos, navr, &entrances);
                            None
                        } else {
                            Some(lk)
                        }
                    }
                    (None, None) => {
                        *brain = Brain::Investigate {
                            at: pos,
                            left: search::INVESTIGATE_TIME,
                        };
                        None
                    }
                }
            }
            Brain::Leave { to } => {
                let to = *to;
                // The Gate itself is blocked on the nav grid, so "there" is
                // close, or at the end of the path to the nearest open cell.
                let d = Vec2::new(pos.x - to.x, pos.z - to.z).length();
                let arrived = d < 5.0 || (d < 14.0 && w.goal == Some(to) && w.path.is_empty());
                if arrived {
                    if single_minded {
                        // The convoy system records what it carried out, then
                        // despawns it.
                        convoy.write(super::collector::ConvoyEvent::Delivered { robot: e });
                    } else {
                        commands.entity(e).despawn();
                    }
                    continue;
                }
                Some(to)
            }
            Brain::Down => None,
        };
        // Hiding indoors? Then the door: open it, or tear it down.
        let door = indoors
            .and_then(|at| entrances.building_at(at))
            .and_then(|b| {
                entrances.approach(b, pos, |d| {
                    super::doors::is_open(doors.get(d).ok().map(|(d, _)| d))
                })
            });
        let goal = match door {
            Some((ent, open)) => {
                let stand = ent.mid + ent.out * 1.8;
                let close = Vec2::new(pos.x - stand.x, pos.z - stand.z).length() < 2.2;
                at_door = close && open;
                if close && !open {
                    if let Ok((mut d, wedged)) = doors.get_mut(ent.door) {
                        if !wedged {
                            d.open = true;
                        } else {
                            let b = match battering.as_deref_mut() {
                                Some(b) => b,
                                None => {
                                    commands.entity(e).insert(super::doors::Battering(0.0));
                                    continue;
                                }
                            };
                            let before = b.0;
                            b.0 += dt;
                            // Every blow is loud.
                            if (before / 1.5).floor() != (b.0 / 1.5).floor() {
                                noise.write(super::Noise {
                                    at: ent.mid,
                                    radius: 30.0,
                                    source: Some(e),
                                });
                                blows.write(super::doors::DoorBlow {
                                    at: ent.mid,
                                    broke: false,
                                });
                            }
                            if b.0 >= super::doors::BATTER_TIME {
                                blows.write(super::doors::DoorBlow {
                                    at: ent.mid,
                                    broke: true,
                                });
                                commands.entity(ent.door).despawn();
                                commands.entity(e).remove::<super::doors::Battering>();
                                raid.say("Somewhere close, a door gives way with a crash.");
                            }
                        }
                    }
                }
                if close {
                    intent.look = Some(ent.mid - ent.out * 3.0);
                    None
                } else {
                    Some(stand)
                }
            }
            None => goal,
        };
        // Through the open door and nobody in view: search from here.
        if at_door {
            if let Brain::Investigate { at, .. } = *brain {
                *brain = search::begin_search(at, aware, pos, navr, &entrances);
            }
        }
        w.repath -= dt;
        if goal != w.goal || (goal.is_some() && w.repath <= 0.0 && w.path.is_empty()) {
            // A long search costs: only so many a tick across all walkers
            // (when everyone's target jumps at once, say). Over budget, it
            // keeps its old path and asks again next tick.
            let long =
                goal.is_some_and(|g| Vec2::new(g.x - pos.x, g.z - pos.z).length() > SHORT_PATH);
            if !long || paths_left > 0 {
                if long {
                    paths_left -= 1;
                }
                w.goal = goal;
                w.repath = 3.0;
                w.path = goal.and_then(|g| navr.path(pos, g)).unwrap_or_default();
            }
        }
    }
}

/// Long path searches (to a goal further than [`SHORT_PATH`] m) all the
/// walkers may start in one tick; the rest wait a tick or two.
pub const PATH_BUDGET: u32 = 3;
pub const SHORT_PATH: f32 = 12.0;

/// A walker with less than this share of its core left breaks off...
pub const RETREAT_AT: f32 = 0.35;
/// ...to cover about this much further from you (m).
pub const RETREAT_DIST: f32 = 14.0;
/// Knocked down again within this long (s) of the last time, a walker
/// breaks off as a badly hurt one does.
pub const KNOCKED_AGAIN: f32 = super::recall::lasts(super::recall::Memory::Knocked);

/// A walker that's broken off, hurt (it keeps shooting from further back).
#[derive(Component, Debug)]
pub struct Retreating;

/// A flanker (`squad`) works round to this far off the pinner's line
/// (radians, either side by turns)...
pub const FLANK_ANGLE: f32 = 1.3;
/// ...at the pinner's range, held to this band (m).
pub const FLANK_RANGE: (f32, f32) = (14.0, 24.0);

/// A walker pinning you while a squadmate moves up: faster, steadier fire
/// (`SUPPRESS_EVERY`), on where it last saw you. For the look and sound
/// (its muzzle, its cadence).
#[derive(Component, Debug)]
pub struct Suppressing;

/// A walker working round to its target's side (while another pins it).
#[derive(Component, Debug)]
pub struct Flanker;

/// Somewhere to fight `target` from: walkable ground hard by an obstacle
/// (a wall, a wreck, a trunk), within [`COVER_RANGE`] of it, with a clear
/// line to it, not far from `pos`. Where it stands, if that will do.
fn pick_cover(pos: Vec3, target: Vec3, nav: NavRef, spatial: &SpatialQuery) -> Option<Vec3> {
    let blocked = |p: Vec3| !nav.walkable(p);
    let world = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
    let head = target + Vec3::Y * 1.4;
    let ground_y = pos.y - BODY_HEIGHT;
    let score = |p: Vec3| -> Option<f32> {
        let d = Vec2::new(p.x - target.x, p.z - target.z).length();
        if blocked(p) || d < COVER_RANGE.0 || d > COVER_RANGE.1 {
            return None;
        }
        let cover = (0..8).any(|j| {
            let b = j as f32 * std::f32::consts::FRAC_PI_4;
            blocked(p + Vec3::new(b.cos(), 0.0, b.sin()) * 1.4)
        });
        if !cover {
            return None;
        }
        let eye = Vec3::new(p.x, ground_y + BODY_HEIGHT, p.z);
        let to = head - eye;
        let clear = Dir3::new(to).ok().is_some_and(|dir| {
            spatial
                .cast_ray(eye, dir, to.length() - 0.5, true, &world)
                .is_none()
        });
        clear.then(|| Vec2::new(p.x - pos.x, p.z - pos.z).length() + (d - 24.0).abs() * 0.5)
    };
    let here = score(pos).map(|s| (pos, s - 3.0));
    let mut best = here;
    for ring in [4.0, 8.0] {
        for k in 0..12 {
            let a = k as f32 * std::f32::consts::TAU / 12.0;
            let p = pos + Vec3::new(a.cos(), 0.0, a.sin()) * ring;
            if let Some(s) = score(p) {
                if best.is_none_or(|(_, b)| s < b) {
                    best = Some((p, s));
                }
            }
        }
    }
    best.map(|(p, _)| p)
}

#[allow(clippy::too_many_arguments)]
fn walker_guns(
    mut walkers: Query<(
        Entity,
        &mut Walker,
        &Brain,
        &Awareness,
        &Robot,
        &Children,
        Option<&super::anim::Gait>,
    )>,
    turrets: Query<(&GlobalTransform, &RobotPart), With<WalkerTurret>>,
    parts: Query<&RobotPart>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
    mut commands: Commands,
    plan: Res<super::squad::Coordination>,
    mut shots: MessageWriter<Shot>,
    mut aims: MessageWriter<WalkerAim>,
    mut seed: Local<u64>,
    minds: Query<&super::senses::MindSeed>,
    bodies: Query<&crate::player::Body>,
    (entrances, path, mut blind, mut fighting_since, roles): (
        Res<super::doors::Entrances>,
        crate::destruction::through::RoundPath,
        Local<std::collections::HashMap<Entity, Blind>>,
        Local<std::collections::HashMap<Entity, f32>>,
        Query<(&super::squad::SquadRole, &GlobalTransform)>,
    ),
) {
    let dt = time.delta_secs();
    blind.retain(|e, _| walkers.contains(*e));
    fighting_since.retain(|e, _| {
        walkers
            .get(*e)
            .is_ok_and(|(_, _, b, ..)| matches!(b, Brain::Engage))
    });
    // Whom a squad has someone moving up on (`squad`): their pinner
    // suppresses.
    let covering: Vec<Entity> = roles
        .iter()
        .filter(|(r, g)| super::squad::repositioning(Some(r), g.translation()))
        .map(|(r, _)| r.target)
        .collect();
    for (e, mut w, brain, aware, robot, children, gait) in &mut walkers {
        w.gun_cooldown -= dt;
        // Knocked down, or getting up, it can't shoot, and must aim afresh.
        let floored = gait.is_some_and(|g| g.down() || g.rising > 0.0);
        // On its way to a new post (`squad`): it holds its fire.
        let moving_up = roles
            .get(e)
            .is_ok_and(|(r, g)| super::squad::repositioning(Some(r), g.translation()));
        if robot.wrecked || !matches!(brain, Brain::Engage) || floored || moving_up {
            w.aim = 0.0;
            blind.remove(&e);
            continue;
        }
        if aware.can_see_target {
            // A new loss of sight may earn a new burst.
            blind.remove(&e);
        }
        let suppressing = plan.suppress
            && roles.get(e).is_ok_and(|(r, _)| {
                r.role == super::squad::Role::Pin && covering.contains(&r.target)
            });
        if suppressing != w.suppressing {
            w.suppressing = suppressing;
            if suppressing {
                commands.entity(e).try_insert(Suppressing);
            } else {
                commands.entity(e).try_remove::<Suppressing>();
            }
        }
        // Suppressing and you've dropped from sight: it keeps its rounds
        // on where you went down, a while. Gone from there, they miss.
        if suppressing && !aware.can_see_target && aware.unseen < SUPPRESS_HOLD {
            let (Some(lk), Some((gt, part))) = (
                aware.last_known,
                children.iter().find_map(|c| turrets.get(c).ok()),
            ) else {
                continue;
            };
            if !part.alive() || w.gun_cooldown > 0.0 {
                continue;
            }
            let muzzle = gt.translation() + Vec3::Y * 0.05;
            let at = lk + Vec3::Y * 1.0;
            let dir = (at - muzzle).normalize_or(Vec3::NEG_Z);
            *seed = super::senses::shot_seed(&minds, e, &time);
            shots.write(Shot {
                gun: e,
                shooter: Some(e),
                origin: muzzle + dir * 0.3,
                direction: dir,
                speed: ROUND_SPEED,
                damage: 14.0,
                projectiles: 1,
                spread: SUPPRESS_SPREAD,
                recoil: 0.0,
                seed: *seed,
            });
            w.gun_cooldown = SUPPRESS_EVERY;
            continue;
        }
        if !aware.can_see_target {
            // A moment behind cover doesn't shake it; half a second does:
            // back in sight, it has to aim afresh (`REACQUIRE`).
            if aware.unseen > REACQUIRE {
                w.aim = 0.0;
            }
            // Through the wall, if it's thin and you're close behind it.
            let Some((gt, part)) = children.iter().find_map(|c| turrets.get(c).ok()) else {
                continue;
            };
            if !part.alive() || aware.unseen < BLIND_AFTER {
                continue;
            }
            let muzzle = gt.translation() + Vec3::Y * 0.05;
            let b = blind.entry(e).or_default();
            if b.spent {
                continue;
            }
            if b.left == 0 {
                // Worth it? Close, sure, under a roof, and a round gets there.
                let Some(lk) = aware.last_known else {
                    b.spent = true;
                    continue;
                };
                let at = lk + Vec3::Y * 1.0;
                let near = muzzle.distance(at) < BLIND_RANGE;
                let sure = aware.uncertainty < BLIND_SURE;
                let soon = aware.unseen < BLIND_WITHIN;
                let roofed = entrances.building_at(lk).is_some();
                // Aimed a little short: the check can't tell you from a wall.
                let short = at - (at - muzzle).normalize_or_zero() * 0.6;
                let own = |x: Entity| x == e || parts.get(x).is_ok_and(|p| p.robot == e);
                if !(near && sure && soon && roofed) {
                    if !soon {
                        b.spent = true;
                    }
                    continue;
                }
                if !path.would_reach(muzzle, short, ROUND_G, ROUND_SPEED, &own) {
                    b.spent = true;
                    continue;
                }
                *b = Blind {
                    tell: 0.0,
                    left: BLIND_ROUNDS,
                    spent: false,
                    at,
                };
                aims.write(WalkerAim {
                    walker: e,
                    at: muzzle,
                });
            }
            b.tell += dt;
            if b.tell < BLIND_TELL || w.gun_cooldown > 0.0 {
                continue;
            }
            *seed = super::senses::shot_seed(&minds, e, &time);
            let dir = (b.at - muzzle).normalize_or(Vec3::NEG_Z);
            shots.write(Shot {
                gun: e,
                shooter: Some(e),
                origin: muzzle + dir * 0.3,
                direction: dir,
                speed: ROUND_SPEED,
                damage: 14.0,
                projectiles: 1,
                spread: BLIND_SPREAD,
                recoil: 0.0,
                seed: *seed,
            });
            w.gun_cooldown = BLIND_EVERY;
            b.left -= 1;
            if b.left == 0 {
                b.spent = true;
            }
            continue;
        }
        let Some(target) = aware
            .target
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() + Vec3::Y * 1.1)
        else {
            continue;
        };
        let Some((gt, part)) = children.iter().find_map(|c| turrets.get(c).ok()) else {
            continue;
        };
        if !part.alive() {
            continue;
        }
        let muzzle = gt.translation() + Vec3::Y * 0.05;
        // How fast it's going across the line of fire (m/s).
        let crossing = aware
            .target
            .and_then(|p| bodies.get(p).ok())
            .map_or(0.0, |b| {
                let line = (target - muzzle).with_y(0.0).normalize_or_zero();
                let v = b.velocity.with_y(0.0);
                (v - line * v.dot(line)).length()
            });
        if muzzle.distance(target) > 70.0 {
            continue;
        }
        // A first look at you before anything else (seconds of it).
        let looked = fighting_since.entry(e).or_insert(0.0);
        *looked += dt;
        if *looked < SIGHT_BEAT {
            continue;
        }
        // Tell first: the turret settles, then it fires.
        if w.aim < AIM_TIME {
            if w.aim == 0.0 {
                aims.write(WalkerAim {
                    walker: e,
                    at: muzzle,
                });
            }
            w.aim += dt;
            continue;
        }
        if w.gun_cooldown > 0.0 {
            continue;
        }
        *seed = super::senses::shot_seed(&minds, e, &time);
        shots.write(Shot {
            gun: e,
            shooter: Some(e),
            origin: muzzle + (target - muzzle).normalize_or_zero() * 0.3,
            direction: (target - muzzle).normalize_or(Vec3::NEG_Z),
            speed: 450.0,
            damage: 14.0,
            projectiles: 1,
            // Running across its line throws it off (`CROSSING_SPREAD`).
            // Suppressing it's steadier on you if you hold still, but
            // running across its line throws it off just the same: leaving
            // the pinner's line is what beats it.
            spread: if suppressing { SUPPRESS_SPREAD } else { 0.025 } + CROSSING_SPREAD * crossing,
            recoil: 0.0,
            seed: *seed,
        });
        w.gun_cooldown = if suppressing {
            SUPPRESS_EVERY
        } else {
            FIRE_EVERY
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On every map, every rendezvous can be walked to from the insertion
    /// (the bots path on this grid too: a missing bridge is a valley
    /// nobody crosses).
    #[test]
    fn every_rendezvous_is_reachable_on_every_map() {
        use extraction_core::map::MarkerKind;
        let mut failed = Vec::new();
        for map in crate::test_maps() {
            let opts = crate::LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            app.update();
            let zone = app.world().resource::<crate::world::Zone>().clone();
            let nav = app.world().resource::<Nav>().0.clone();
            let ins = zone.marker(&MarkerKind::Insertion).unwrap().at;
            for rv in zone.map.markers_of(&MarkerKind::Rendezvous) {
                if nav.path(ins, rv.at).is_none() {
                    failed.push(format!("{map}: {}", rv.name));
                }
            }
        }
        assert!(failed.is_empty(), "no way to: {failed:?}");
    }
}
