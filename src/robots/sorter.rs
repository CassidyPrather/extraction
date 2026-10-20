//! The Sorter: the Depot's heavy resident, a ceiling gantry in the sorting
//! hall. A bridge runs the hall's length on two rails under the ceiling, a
//! carriage runs across the bridge, and from the carriage hangs a long
//! jointed arm (two-bone IK, like a walker's legs) with a grab. A scanning
//! head under the carriage sweeps its eye. Its idle work is shifting
//! orange totes between spots in the aisle; it can also throw one, hard.
//! Kill its carriage and the whole thing comes down off its rails.
//!
//! Hall space: origin at the hall's centre on its floor, X along the
//! rails (the hall's length), Y up. The body follows a [`SorterOrder`]
//! from a mind (Robot minds'); with none, and [`SorterScript`] on, it runs
//! its own loop: sort totes, and throw one at whoever it sees within
//! [`THROW_RANGE`] every [`THROW_COOLDOWN`] seconds.
//!
//! Cost: one two-bone IK and a handful of transforms a tick; no rays.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::walker::LegPoses;
use super::wreck::{Broken, Debris, Piece, Settling, WreckImpact, WreckJoint, spawn_debris};
use super::{
    Awareness, Brain, Intent, Job, JobKind, PartRole, Robot, RobotKind, Senses, part_bundle,
};
use crate::world::Layer;

/// Upper arm and forearm, metres (as modelled, art/robots/sorter.py).
pub const UPPER: f32 = 2.3;
pub const LOWER: f32 = 2.1;
/// From the wrist down to where the grab closes on a tote's rim.
pub const GRAB: f32 = 0.45;
/// The carriage rides this far under the ceiling (the bridge's top is
/// then 0.35 m under it: clear of the hall's lamp).
pub const CARRIAGE_DROP: f32 = 1.1;
/// The shoulder, under the carriage's middle.
const SHOULDER: f32 = 0.46;
/// The rails, either side, this far from the hall's middle.
pub const RAIL_Z: f32 = 7.2;
const SPEED: f32 = 1.6;
const ACCEL: f32 = 1.1;
/// How fast the grab moves (m/s), and on a throw's swing.
const GRAB_SPEED: f32 = 1.8;
const SWING_SPEED: f32 = 7.0;
pub const THROW_RANGE: f32 = 14.0;
pub const THROW_COOLDOWN: f32 = 6.0;
/// A tote: size and mass.
pub const TOTE: Vec3 = Vec3::new(0.6, 0.4, 0.45);
const TOTE_MASS: f32 = 12.0;

/// What a mind asks of the Sorter. Without one it does its own work
/// (when [`SorterScript`] is on).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq)]
pub enum SorterOrder {
    /// Its own sort loop.
    #[default]
    Idle,
    /// Track a point with the head (the arm carries on).
    Watch { at: Vec3 },
    /// Move over a point and reach the grab toward it.
    Reach { at: Vec3 },
    /// Throw a tote at a point (fetching the nearest first).
    Throw { at: Vec3 },
}

/// The scripted loop runs for Sorters without a mind's orders.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SorterScript(pub bool);

impl Default for SorterScript {
    fn default() -> Self {
        Self(true)
    }
}

/// A Sorter threw a tote (for sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct SorterThrow {
    pub sorter: Entity,
    pub tote: Entity,
    pub from: Vec3,
}

/// The Sorter is about to throw at `at` (the tell: it stops, its head
/// locks on and flares red, its grab draws back, for [`WINDUP`] seconds).
/// A mind may send it before a `Throw`; the body sends its own as it
/// winds up.
#[derive(Message, Clone, Copy, Debug)]
pub struct SorterWindup {
    pub sorter: Entity,
    pub at: Vec3,
}

pub const WINDUP: f32 = 0.7;

/// What the body tells a mind.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct SorterState {
    /// The tote in its grab.
    pub holding: Option<Entity>,
    /// At a job (fetching, carrying, throwing, reaching) rather than resting.
    pub busy: bool,
    /// How far out the grab can get from straight under the carriage at
    /// the floor, metres.
    pub reach: f32,
}

/// A tote the Sorter shifts about. Not loot: it's heavy and orange.
#[derive(Component, Debug)]
pub struct Tote;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Task {
    Rest(f32),
    /// Fetch `tote` and set it down at `to` (hall xz).
    Fetch {
        tote: Entity,
        to: Vec2,
        stage: u8,
    },
    /// Throw a tote at `at` (world).
    Throw {
        tote: Option<Entity>,
        at: Vec3,
        stage: u8,
        t: f32,
    },
    /// Reach toward `at` (world).
    Reach {
        at: Vec3,
    },
}

/// The gantry's state (hall space unless said).
#[derive(Component, Debug)]
pub struct Sorter {
    /// Hall space to world.
    pub hall: Transform,
    /// The hall's half extents (x along the rails, z across).
    pub half: Vec2,
    /// Floor to ceiling.
    pub height: f32,
    /// The carriage over the floor (x on the rails, z on the bridge).
    pub pos: Vec2,
    pub vel: Vec2,
    /// Where the grab is.
    pub grab: Vec3,
    pub holding: Option<Entity>,
    pub task: Task,
    pub cooldown: f32,
    /// The head's sweep, radians from straight ahead.
    pub head_yaw: f32,
    pub arm_alive: bool,
    /// Where totes may be set down.
    pub spots: Vec<Vec2>,
    /// Showing the throw's tell: seconds left, and at what.
    pub windup: f32,
    pub windup_at: Vec3,
    seed: u32,
}

impl Sorter {
    pub fn carriage_y(&self) -> f32 {
        self.height - CARRIAGE_DROP
    }
    pub fn shoulder(&self) -> Vec3 {
        Vec3::new(self.pos.x, self.carriage_y() - SHOULDER, self.pos.y)
    }
    /// Hall space to world, and back.
    pub fn world(&self, p: Vec3) -> Vec3 {
        self.hall.transform_point(p)
    }
    pub fn local(&self, p: Vec3) -> Vec3 {
        self.hall.compute_affine().inverse().transform_point3(p)
    }
    fn rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Shoulder, elbow, wrist and grab, world space (for visuals and the
/// arm's collider).
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct ArmPose {
    pub shoulder: Vec3,
    pub elbow: Vec3,
    pub wrist: Vec3,
    pub grab: Vec3,
    /// 0 open .. 1 shut.
    pub grip: f32,
}

/// A falling Sorter, until it hits.
#[derive(Component, Debug, Default)]
struct Falling {
    t: f32,
}

pub struct SorterPlugin;

impl Plugin for SorterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SorterScript>()
            .add_message::<SorterThrow>()
            .add_message::<SorterWindup>()
            .add_systems(
                FixedUpdate,
                (spawn_in_depot, sort, wreck_sorters, land)
                    .chain()
                    .after(super::perceive)
                    .before(PhysicsSystems::First),
            );
    }
}

/// Two-bone IK: the elbow, for upper and lower lengths, bent toward
/// `pole`.
pub fn elbow(shoulder: Vec3, wrist: Vec3, pole: Vec3, l1: f32, l2: f32) -> Vec3 {
    let d = (wrist - shoulder).length().clamp(0.05, l1 + l2 - 0.001);
    let dir = (wrist - shoulder).normalize_or(Vec3::NEG_Y);
    let a = (l1 * l1 - l2 * l2 + d * d) / (2.0 * d);
    let h = (l1 * l1 - a * a).max(0.0).sqrt();
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::X);
    shoulder + dir * a + bend * h
}

/// The aisle spots in the Depot's sorting hall (hall space), clear of its
/// shelves, crates and pallets by 1.2 m.
pub fn depot_spots() -> Vec<Vec2> {
    let mut v = Vec::new();
    for x in [-10.0, -6.4, -2.6, 2.6, 6.4, 10.0] {
        for z in [-0.9, 0.9] {
            v.push(Vec2::new(x, z));
        }
    }
    v
}

/// A Sorter in a hall (`hall`: hall space to world; `half`: its half
/// extents; `height`: floor to ceiling), with totes on the first few of
/// `spots`.
pub fn spawn_sorter(
    commands: &mut Commands,
    hall: Transform,
    half: Vec2,
    height: f32,
    spots: Vec<Vec2>,
    totes: usize,
) -> Entity {
    let start = Vec2::new(0.0, 0.0);
    let s = Sorter {
        hall,
        half,
        height,
        pos: start,
        vel: Vec2::ZERO,
        grab: Vec3::new(start.x, height - CARRIAGE_DROP - SHOULDER - 2.0, start.y),
        holding: None,
        task: Task::Rest(3.0),
        cooldown: 2.0,
        head_yaw: 0.0,
        arm_alive: true,
        spots: spots.clone(),
        windup: 0.0,
        windup_at: Vec3::ZERO,
        seed: 0x50_27_e2,
    };
    let at = s.world(Vec3::new(start.x, s.carriage_y(), start.y));
    let site = s.world(Vec3::ZERO);
    let robot = commands
        .spawn((
            Name::new("Sorter"),
            Robot {
                kind: RobotKind::Sorter,
                wrecked: false,
            },
            Senses {
                // A ceiling scanner: it sees the whole floor under it, not
                // far down a corridor.
                sight: 30.0,
                fov_deg: 320.0,
                hearing: 1.2,
                eye: Vec3::new(0.0, -0.42, -0.62),
            },
            Awareness::default(),
            Intent::default(),
            Brain::Working {
                at: site,
                left: f32::INFINITY,
            },
            Job {
                kind: JobKind::Watch,
                site,
                duration: f32::INFINITY,
                exit: site,
            },
            Transform::from_translation(at).with_rotation(hall.rotation),
            Visibility::default(),
            RigidBody::Kinematic,
            ArmPose::default(),
            LegPoses::default(),
            SorterState::default(),
        ))
        .id();
    commands.entity(robot).insert(s);
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Sorter carriage"),
            part_bundle(
                robot,
                PartRole::Core,
                260.0,
                Collider::cuboid(1.4, 0.6, 1.2),
                Transform::IDENTITY,
            ),
        ));
        c.spawn((
            Name::new("Sorter head"),
            part_bundle(
                robot,
                PartRole::Sensor,
                40.0,
                Collider::cuboid(0.34, 0.2, 0.3),
                Transform::from_xyz(0.0, -0.42, -0.5),
            ),
        ));
        c.spawn((
            Name::new("Sorter arm"),
            part_bundle(
                robot,
                PartRole::Leg(0),
                120.0,
                Collider::capsule(0.1, LOWER - 0.2),
                Transform::IDENTITY,
            ),
        ));
    });
    for (k, spot) in spots.iter().take(totes).enumerate() {
        let p = hall.transform_point(Vec3::new(spot.x, TOTE.y * 0.5 + 0.01, spot.y));
        commands.spawn((
            Name::new(format!("Tote {k}")),
            Tote,
            Transform::from_translation(p).with_rotation(hall.rotation),
            tote_body(),
        ));
    }
    robot
}

fn tote_body() -> impl Bundle {
    (
        RigidBody::Dynamic,
        Collider::cuboid(TOTE.x, TOTE.y, TOTE.z),
        Mass(TOTE_MASS),
        NoAutoMass,
        AngularInertia::new(Vec3::new(0.36, 0.56, 0.52)),
        NoAutoAngularInertia,
        Friction::new(0.7),
        // (Not the Robot layer: the grab and forearm close round it.)
        CollisionLayers::new(Layer::Item, [Layer::World, Layer::Item, Layer::Player]),
        Visibility::default(),
    )
}

/// Once there's a Depot (and the director's on, and the machines haven't
/// been recalled), its sorting hall gets its Sorter.
fn spawn_in_depot(
    mut commands: Commands,
    buildings: Query<(&crate::world::buildings::Building, &Transform)>,
    director: Option<Res<super::director::Director>>,
    raid: Option<Res<crate::raid::Raid>>,
    mut done: Local<bool>,
) {
    use extraction_core::underground as ug;
    if *done || !director.is_some_and(|d| d.enabled) || raid.is_some_and(|r| r.recalled) {
        return;
    }
    let depot = crate::world::underground::DEPOT;
    let Some((_, xf)) = buildings.iter().find(|(b, _)| b.prefab == depot) else {
        return;
    };
    *done = true;
    let Some(l) = ug::layout(depot) else {
        return;
    };
    let Some(room) = l.rooms.iter().find(|r| r.id == "sorting_hall") else {
        return;
    };
    let floor = l.floor_of(room);
    let hall = *xf * Transform::from_xyz(room.at[0], floor, room.at[1]);
    let half = Vec2::new(room.size[0], room.size[1]) * 0.5;
    info!("director: the Sorter wakes in the Depot's sorting hall");
    spawn_sorter(&mut commands, hall, half, room.height, depot_spots(), 6);
}

/// The body: the carriage to where the task wants it, the grab likewise,
/// the arm solved between, totes carried and set down and thrown.
#[allow(clippy::type_complexity)]
fn sort(
    mut sorters: Query<(
        Entity,
        &mut Sorter,
        &mut Transform,
        &mut ArmPose,
        &mut LegPoses,
        &Robot,
        &Awareness,
        &Children,
        Option<&SorterOrder>,
        Option<&Intent>,
        Has<super::Stunned>,
        &mut SorterState,
    )>,
    mut parts: Query<(&super::RobotPart, &mut Transform), Without<Sorter>>,
    mut totes: Query<
        (Entity, &mut Transform, &RigidBody),
        (With<Tote>, Without<Sorter>, Without<super::RobotPart>),
    >,
    targets: Query<&GlobalTransform>,
    script: Res<SorterScript>,
    time: Res<Time>,
    mut commands: Commands,
    mut throws: MessageWriter<SorterThrow>,
    mut windups: ParamSet<(MessageReader<SorterWindup>, MessageWriter<SorterWindup>)>,
) {
    // A mind's windups: show the tell.
    let told: Vec<SorterWindup> = windups.p0().read().copied().collect();
    let mut windups = windups.p1();
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (
        e,
        mut s,
        mut t,
        mut pose,
        mut legs,
        robot,
        aware,
        children,
        order,
        intent,
        stunned,
        mut state,
    ) in &mut sorters
    {
        if robot.wrecked {
            continue;
        }
        s.cooldown -= dt;
        for w in told.iter().filter(|w| w.sorter == e) {
            s.windup = WINDUP;
            s.windup_at = w.at;
        }
        s.windup = (s.windup - dt).max(0.0);
        // Is the arm still on?
        s.arm_alive = children.iter().any(|c| {
            parts
                .get(c)
                .is_ok_and(|(p, _)| p.role == PartRole::Leg(0) && p.alive())
        });
        // What it can see, and where.
        let seen = aware
            .target
            .filter(|_| aware.can_see_target)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation());
        // Orders: a mind's, else the script's.
        let order = order.copied().unwrap_or_default();
        if !stunned && s.arm_alive {
            match order {
                SorterOrder::Throw { at }
                    if !matches!(s.task, Task::Throw { .. }) && s.cooldown <= 0.0 =>
                {
                    s.task = Task::Throw {
                        tote: s.holding,
                        at,
                        stage: 0,
                        t: 0.0,
                    };
                }
                SorterOrder::Reach { at } => s.task = Task::Reach { at },
                SorterOrder::Idle if script.0 => {
                    if let Some(at) = seen {
                        let flat = s.local(at) - s.shoulder();
                        let near = Vec2::new(flat.x, flat.z).length() < THROW_RANGE;
                        if near && s.cooldown <= 0.0 && !matches!(s.task, Task::Throw { .. }) {
                            s.task = Task::Throw {
                                tote: s.holding,
                                at: at + Vec3::Y * 1.1,
                                stage: 0,
                                t: 0.0,
                            };
                        }
                    }
                }
                _ => {}
            }
            if matches!(s.task, Task::Reach { .. }) && !matches!(order, SorterOrder::Reach { .. }) {
                s.task = Task::Rest(1.0);
            }
        }
        // Tote positions, hall space.
        let find =
            |q: &Query<
                (Entity, &mut Transform, &RigidBody),
                (With<Tote>, Without<Sorter>, Without<super::RobotPart>),
            >,
             s: &Sorter,
             e: Entity| q.get(e).ok().map(|(_, tt, _)| s.local(tt.translation));
        // A tote that's gone through the floor (or out of the hall) comes
        // back to a spot: there are only so many.
        for (te, mut tt, _) in &mut totes {
            let l = s.local(tt.translation);
            if l.y < -2.0 || l.y > s.height + 2.0 {
                let k = te.index_u32() as usize % s.spots.len().max(1);
                if let Some(sp) = s.spots.get(k).copied() {
                    tt.translation = s.world(Vec3::new(sp.x, TOTE.y * 0.5 + 0.05, sp.y));
                    commands.entity(te).insert(LinearVelocity::ZERO);
                }
            }
        }
        let all: Vec<(Entity, Vec3)> = totes
            .iter()
            .map(|(te, tt, _)| (te, s.local(tt.translation)))
            .filter(|(_, p)| p.x.abs() <= s.half.x && p.z.abs() <= s.half.y)
            .collect();
        let floor_tote = TOTE.y * 0.5;
        let carry_y = 2.4;
        let shoulder = s.shoulder();
        let mut car_to = s.pos;
        let mut grab_to = Vec3::new(s.pos.x, shoulder.y - 2.2, s.pos.y);
        let mut grab_speed = GRAB_SPEED;
        let mut release: Option<Vec3> = None;
        let task = s.task;
        let next: Option<Task> = if stunned || !s.arm_alive {
            // Sagging, it lets go.
            if s.holding.is_some() {
                release = Some(Vec3::ZERO);
            }
            grab_to = Vec3::new(s.pos.x, 0.6, s.pos.y);
            None
        } else {
            match task {
                Task::Rest(left) => {
                    let left = left - dt;
                    if left > 0.0 {
                        Some(Task::Rest(left))
                    } else {
                        // The next job: a tote not where it's wanted, to a
                        // free spot.
                        let pick = (s.rand() * all.len() as f32) as usize;
                        let free: Vec<Vec2> = s
                            .spots
                            .iter()
                            .copied()
                            .filter(|sp| {
                                all.iter()
                                    .all(|(_, p)| Vec2::new(p.x, p.z).distance(*sp) > 0.9)
                            })
                            .collect();
                        match (
                            all.get(pick.min(all.len().saturating_sub(1))),
                            free.is_empty(),
                        ) {
                            (Some((te, _)), false) => {
                                let to = free[(s.rand() * free.len() as f32) as usize % free.len()];
                                Some(Task::Fetch {
                                    tote: *te,
                                    to,
                                    stage: 0,
                                })
                            }
                            _ => Some(Task::Rest(3.0)),
                        }
                    }
                }
                Task::Fetch { tote, to, stage } => {
                    let tp = if s.holding == Some(tote) {
                        Some(s.grab - Vec3::Y * TOTE.y * 0.5)
                    } else {
                        find(&totes, &s, tote)
                    };
                    match tp {
                        None => Some(Task::Rest(1.0)),
                        Some(tp) => {
                            let over = Vec2::new(tp.x, tp.z);
                            let top = tp.y + TOTE.y * 0.5 + 0.02;
                            let still = s.vel.length() < 0.15;
                            match stage {
                                0 => {
                                    car_to = over;
                                    grab_to = Vec3::new(over.x, carry_y, over.y);
                                    let there = s.pos.distance(over) < 0.12 && still;
                                    Some(Task::Fetch {
                                        tote,
                                        to,
                                        stage: if there { 1 } else { 0 },
                                    })
                                }
                                1 => {
                                    car_to = over;
                                    grab_to = Vec3::new(over.x, top, over.y);
                                    let down = s.grab.distance(grab_to) < 0.06;
                                    if down {
                                        s.holding = Some(tote);
                                    }
                                    Some(Task::Fetch {
                                        tote,
                                        to,
                                        stage: if down { 2 } else { 1 },
                                    })
                                }
                                2 => {
                                    car_to = over;
                                    grab_to = Vec3::new(over.x, carry_y, over.y);
                                    let up = s.grab.y > carry_y - 0.1;
                                    Some(Task::Fetch {
                                        tote,
                                        to,
                                        stage: if up { 3 } else { 2 },
                                    })
                                }
                                3 => {
                                    car_to = to;
                                    grab_to = Vec3::new(s.pos.x, carry_y, s.pos.y);
                                    let there = s.pos.distance(to) < 0.12 && still;
                                    Some(Task::Fetch {
                                        tote,
                                        to,
                                        stage: if there { 4 } else { 3 },
                                    })
                                }
                                4 => {
                                    car_to = to;
                                    grab_to = Vec3::new(to.x, floor_tote * 2.0 + 0.03, to.y);
                                    let down = s.grab.distance(grab_to) < 0.05;
                                    if down {
                                        release = Some(Vec3::ZERO);
                                    }
                                    Some(Task::Fetch {
                                        tote,
                                        to,
                                        stage: if down { 5 } else { 4 },
                                    })
                                }
                                _ => {
                                    grab_to = Vec3::new(s.pos.x, carry_y + 0.3, s.pos.y);
                                    if s.grab.y > carry_y + 0.2 {
                                        let r = 2.0 + 3.0 * s.rand();
                                        Some(Task::Rest(r))
                                    } else {
                                        Some(task)
                                    }
                                }
                            }
                        }
                    }
                }
                Task::Throw { tote, at, stage, t } => {
                    let at_l = s.local(at);
                    let t = t + dt;
                    match (tote, stage) {
                        // Nothing in hand: the nearest tote.
                        (None, _) => {
                            let near = all
                                .iter()
                                .min_by(|a, b| {
                                    let da = Vec2::new(a.1.x, a.1.z).distance(s.pos);
                                    let db = Vec2::new(b.1.x, b.1.z).distance(s.pos);
                                    da.total_cmp(&db)
                                })
                                .map(|(e, _)| *e);
                            match near {
                                Some(te) => Some(Task::Throw {
                                    tote: Some(te),
                                    at,
                                    stage: 0,
                                    t: 0.0,
                                }),
                                None => Some(Task::Rest(2.0)),
                            }
                        }
                        (Some(te), 0..=2) => {
                            // Fetch it, quick.
                            let Some(tp) = find(&totes, &s, te).or_else(|| {
                                (s.holding == Some(te)).then(|| s.grab - Vec3::Y * 0.2)
                            }) else {
                                return_rest(&mut s);
                                continue;
                            };
                            let over = Vec2::new(tp.x, tp.z);
                            let top = tp.y + TOTE.y * 0.5 + 0.02;
                            car_to = over;
                            grab_speed = GRAB_SPEED * 1.4;
                            let (g, adv) = match stage {
                                0 => (
                                    Vec3::new(over.x, carry_y, over.y),
                                    s.pos.distance(over) < 0.2,
                                ),
                                1 => {
                                    let g = Vec3::new(over.x, top, over.y);
                                    let down = s.grab.distance(g) < 0.08;
                                    if down {
                                        s.holding = Some(te);
                                    }
                                    (g, down)
                                }
                                _ => (Vec3::new(over.x, 3.0, over.y), s.grab.y > 2.9),
                            };
                            grab_to = g;
                            Some(Task::Throw {
                                tote: Some(te),
                                at,
                                stage: if adv { stage + 1 } else { stage },
                                // (The wind-up's clock starts when it does.)
                                t: if adv && stage == 2 { 0.0 } else { t },
                            })
                        }
                        (Some(te), _) if s.holding != Some(te) => Some(Task::Rest(1.0)),
                        (Some(te), 3) => {
                            // Wind up: the grab swings back away from them.
                            let dir =
                                Vec2::new(at_l.x - s.pos.x, at_l.z - s.pos.y).normalize_or(Vec2::X);
                            car_to = s.pos;
                            grab_to =
                                shoulder + Vec3::new(-dir.x, 0.0, -dir.y) * 1.6 - Vec3::Y * 1.2;
                            grab_speed = GRAB_SPEED * 1.6;
                            // The tell, as it starts: it stops, its head
                            // locks on, it draws back, for WINDUP seconds.
                            if t <= dt * 1.5 && s.windup <= 0.0 {
                                s.windup = WINDUP;
                                s.windup_at = at;
                                windups.write(SorterWindup { sorter: e, at });
                            }
                            let ready = s.grab.distance(grab_to) < 0.15 && t >= WINDUP;
                            Some(Task::Throw {
                                tote: Some(te),
                                at,
                                stage: if ready { 4 } else { 3 },
                                t: if ready { 0.0 } else { t },
                            })
                        }
                        (Some(te), _) => {
                            // The swing, and let go on the way through.
                            let dir =
                                Vec2::new(at_l.x - s.pos.x, at_l.z - s.pos.y).normalize_or(Vec2::X);
                            car_to = s.pos;
                            grab_to = shoulder + Vec3::new(dir.x, 0.0, dir.y) * 1.8 - Vec3::Y * 1.6;
                            grab_speed = SWING_SPEED;
                            if t > 0.18 {
                                let from = s.grab - Vec3::Y * 0.2;
                                release = Some(ballistic(from, at_l));
                                s.cooldown = THROW_COOLDOWN;
                                throws.write(SorterThrow {
                                    sorter: e,
                                    tote: te,
                                    from: s.world(from),
                                });
                                Some(Task::Rest(1.5))
                            } else {
                                Some(Task::Throw {
                                    tote: Some(te),
                                    at,
                                    stage: 4,
                                    t,
                                })
                            }
                        }
                    }
                }
                Task::Reach { at } => {
                    let l = s.local(at);
                    car_to = Vec2::new(l.x, l.z);
                    grab_to = l;
                    grab_speed = GRAB_SPEED * 1.5;
                    Some(task)
                }
            }
        };
        if let Some(n) = next {
            s.task = n;
        }

        if s.windup > 0.0 {
            car_to = s.pos;
        }
        // The carriage: heavy, slow to start and stop.
        let lim = Vec2::new(s.half.x - 1.0, (s.half.y - 1.0).min(RAIL_Z - 0.6));
        let car_to = car_to.clamp(-lim, lim);
        let off = car_to - s.pos;
        let want = (off * 1.4).clamp_length_max(SPEED);
        let dv = (want - s.vel).clamp_length_max(ACCEL * dt);
        s.vel += dv;
        let v = s.vel;
        s.pos += v * dt;
        s.pos = s.pos.clamp(-lim, lim);
        // The grab, carried with the carriage, toward its target, within
        // the arm's reach.
        let carry = Vec3::new(v.x, 0.0, v.y) * dt;
        let g = s.grab + carry;
        let to = grab_to;
        let step = (to - g).clamp_length_max(grab_speed * dt);
        let mut g = g + step;
        let shoulder = s.shoulder();
        let reach = UPPER + LOWER - 0.02;
        let wrist = g + Vec3::Y * GRAB;
        let off = wrist - shoulder;
        if off.length() > reach {
            g = shoulder + off.normalize() * reach - Vec3::Y * GRAB;
        }
        g.y = g.y.max(0.05);
        let grab_vel = (g - s.grab) / dt;
        s.grab = g;

        // Let go: set down (or thrown, with a velocity).
        if let Some(throw_v) = release {
            if let Some(te) = s.holding.take() {
                commands.entity(te).insert(RigidBody::Dynamic);
                let v_world = s.hall.rotation
                    * if throw_v == Vec3::ZERO {
                        grab_vel * 0.3
                    } else {
                        throw_v
                    };
                commands.entity(te).insert((
                    LinearVelocity(v_world),
                    AngularVelocity(Vec3::new(2.0, 1.0, 0.5)),
                ));
            }
        }
        // A held tote rides the grab.
        if let Some(te) = s.holding {
            if let Ok((_, mut tt, rb)) = totes.get_mut(te) {
                if *rb != RigidBody::Kinematic {
                    commands.entity(te).insert(RigidBody::Kinematic);
                }
                tt.translation = s.world(s.grab - Vec3::Y * (TOTE.y * 0.5 + 0.02));
                tt.rotation = s.hall.rotation;
            } else {
                s.holding = None;
            }
        }

        // The head sweeps (or tracks what it's told to, or sees).
        let look = match order {
            _ if s.windup > 0.0 => Some(s.windup_at),
            SorterOrder::Watch { at } => Some(at),
            _ => None,
        }
        .or(intent.and_then(|i| i.look))
        .or(seen);
        let want_yaw = match look {
            Some(p) => {
                let l = s.local(p) - Vec3::new(s.pos.x, s.carriage_y(), s.pos.y);
                (-l.x).atan2(-l.z)
            }
            None => (time.elapsed_secs() * 0.45 + e.index_u32() as f32).sin() * 1.1,
        };
        let d = super::anim::wrap_angle(want_yaw - s.head_yaw);
        s.head_yaw += d.clamp(-1.5 * dt, 1.5 * dt);

        // Place the body, and the arm.
        t.translation = s.world(Vec3::new(s.pos.x, s.carriage_y(), s.pos.y));
        t.rotation = s.hall.rotation;
        let wrist = s.grab + Vec3::Y * GRAB;
        let toward = Vec3::new(wrist.x - shoulder.x, 0.0, wrist.z - shoulder.z);
        // The elbow kinks out away from where it's reaching, up, like a
        // crane's: and toward +X when it's straight down.
        let pole = -toward.normalize_or(Vec3::NEG_X) + Vec3::Y * 0.6;
        let el = elbow(shoulder, wrist, pole, UPPER, LOWER);
        *pose = ArmPose {
            shoulder: s.world(shoulder),
            elbow: s.world(el),
            wrist: s.world(wrist),
            grab: s.world(s.grab),
            grip: if s.holding.is_some() { 1.0 } else { 0.0 },
        };
        legs.0[0] = [pose.shoulder, pose.elbow, pose.wrist];
        *state = SorterState {
            holding: s.holding,
            busy: !matches!(s.task, Task::Rest(_)),
            reach: ((UPPER + LOWER + GRAB).powi(2) - shoulder.y.powi(2))
                .max(0.0)
                .sqrt(),
        };
        // Arm collider on the forearm; head turned.
        let inv = t.compute_affine().inverse();
        let inv_rot = t.rotation.inverse();
        for c in children.iter() {
            if let Ok((p, mut pt)) = parts.get_mut(c) {
                match p.role {
                    PartRole::Leg(0) => {
                        let mid = (pose.elbow + pose.wrist) * 0.5;
                        let dir = (pose.elbow - pose.wrist).normalize_or(Vec3::Y);
                        *pt = Transform::from_translation(inv.transform_point3(mid))
                            .with_rotation(inv_rot * Quat::from_rotation_arc(Vec3::Y, dir));
                    }
                    PartRole::Sensor => {
                        pt.rotation = Quat::from_rotation_y(s.head_yaw);
                    }
                    _ => {}
                }
            }
        }
    }
}

fn return_rest(s: &mut Sorter) {
    s.task = Task::Rest(1.0);
}

/// The launch velocity that carries something from `from` to `to` (hall
/// space), on a flight of about a second, capped.
pub fn ballistic(from: Vec3, to: Vec3) -> Vec3 {
    let d = to - from;
    let flat = Vec2::new(d.x, d.z).length();
    let t = (flat / 11.0).clamp(0.35, 1.3);
    let v = d / t + Vec3::Y * 0.5 * 9.81 * t;
    v.clamp_length_max(18.0)
}

/// Its carriage shot through, the Sorter comes off its rails: it drops,
/// one end first, its arm (jointed at shoulder and elbow) flailing, and
/// what it held falls.
#[allow(clippy::type_complexity)]
fn wreck_sorters(
    mut commands: Commands,
    mut sorters: Query<
        (Entity, &mut Sorter, &Robot, &ArmPose, &Children),
        (Without<Broken>, Without<Falling>),
    >,
    parts: Query<&super::RobotPart>,
) {
    for (e, mut s, robot, pose, children) in &mut sorters {
        if !robot.wrecked {
            continue;
        }
        if let Some(te) = s.holding.take() {
            commands.entity(te).insert(RigidBody::Dynamic);
        }
        let v = s.hall.rotation * Vec3::new(s.vel.x, 0.0, s.vel.y);
        commands.entity(e).remove::<LegPoses>().insert((
            Broken,
            Falling::default(),
            RigidBody::Dynamic,
            Mass(500.0),
            NoAutoMass,
            AngularInertia::new(Vec3::new(120.0, 140.0, 110.0)),
            NoAutoAngularInertia,
            LinearVelocity(v),
            // One end goes first.
            AngularVelocity(s.hall.rotation * Vec3::new(0.0, 0.0, 0.9)),
            LinearDamping(0.1),
            AngularDamping(0.8),
            Friction::new(0.8),
        ));
        // The arm, if it's still on, comes down with it on its joints.
        let arm = children.iter().find(|c| {
            parts
                .get(*c)
                .is_ok_and(|p| p.role == PartRole::Leg(0) && p.alive())
        });
        if let Some(c) = arm {
            let mut ids = [Entity::PLACEHOLDER; 2];
            for (k, (a, b, len, mass)) in [
                (pose.shoulder, pose.elbow, UPPER, 60.0),
                (pose.elbow, pose.wrist, LOWER, 45.0),
            ]
            .into_iter()
            .enumerate()
            {
                let dir = (b - a).normalize_or(Vec3::NEG_Y);
                let i_ax = mass * len * len / 12.0;
                ids[k] = spawn_debris(
                    &mut commands,
                    (
                        Debris {
                            kind: RobotKind::Sorter,
                            piece: Piece::Arm(k as u8),
                        },
                        Transform::from_translation((a + b) * 0.5)
                            .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir)),
                        v,
                        Vec3::ZERO,
                        mass,
                        Collider::capsule(0.12, (len - 0.24).max(0.1)),
                        Vec3::new(i_ax, mass * 0.01, i_ax),
                    ),
                    1.0,
                );
            }
            commands.spawn((
                Name::new("Sorter shoulder"),
                SphericalJoint::new(e, ids[0])
                    .with_anchor(pose.shoulder)
                    .with_swing_limits(0.0, 1.6),
                JointCollisionDisabled,
                WreckJoint { a: e, b: ids[0] },
            ));
            commands.spawn((
                Name::new("Sorter elbow"),
                SphericalJoint::new(ids[0], ids[1])
                    .with_anchor(pose.elbow)
                    .with_swing_limits(0.0, 2.2),
                JointCollisionDisabled,
                WreckJoint {
                    a: ids[0],
                    b: ids[1],
                },
            ));
            commands.entity(c).despawn();
        }
    }
}

/// A falling Sorter that has hit the floor: dust and sparks, then it
/// settles like any wreck.
fn land(
    mut commands: Commands,
    mut falling: Query<(Entity, &mut Falling, &Transform, &LinearVelocity)>,
    mut impacts: MessageWriter<WreckImpact>,
    time: Res<Time>,
) {
    for (e, mut f, t, v) in &mut falling {
        f.t += time.delta_secs();
        if (f.t > 0.4 && v.0.y > -0.5) || f.t > 6.0 {
            impacts.write(WreckImpact {
                at: t.translation,
                size: 2.0,
            });
            commands
                .entity(e)
                .remove::<Falling>()
                .insert(Settling::default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arm_keeps_its_lengths_and_the_throw_lands() {
        let s = Vec3::new(0.0, 4.4, 0.0);
        for w in [
            Vec3::new(1.0, 1.0, 0.5),
            Vec3::new(0.0, 0.2, 0.0),
            Vec3::new(-3.0, 3.0, 1.0),
        ] {
            let e = elbow(s, w, Vec3::Y + Vec3::X, UPPER, LOWER);
            assert!((e.distance(s) - UPPER).abs() < 1e-3, "{e}");
            if w.distance(s) < UPPER + LOWER {
                assert!((e.distance(w) - LOWER).abs() < 1e-3);
            }
        }
        // A thrown tote reaches where it was aimed (no drag).
        let from = Vec3::new(0.0, 2.5, 0.0);
        let to = Vec3::new(9.0, 1.2, 4.0);
        let v = ballistic(from, to);
        let flat = Vec2::new(to.x - from.x, to.z - from.z).length();
        let tf = flat / Vec2::new(v.x, v.z).length();
        let p = from + v * tf + Vec3::Y * (-0.5 * 9.81 * tf * tf);
        assert!(p.distance(to) < 0.05, "{p}");
    }

    fn app_with_sorter() -> (App, Entity, Transform) {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        // An open-air hall on the meadow (no ceiling, but the floor's real).
        // (At the highest ground under it, so nothing's set down under
        // the meadow.)
        let c = Vec2::new(40.0, 220.0);
        let mut top = f32::MIN;
        for i in -12..=12 {
            for j in -8..=8 {
                top = top.max(zone.ground(c.x + i as f32, c.y + j as f32));
            }
        }
        let hall = Transform::from_xyz(c.x, top, c.y);
        let e = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            // A concrete floor, as the Depot's hall has.
            c.spawn((
                RigidBody::Static,
                Collider::cuboid(30.0, 0.2, 20.0),
                CollisionLayers::new(Layer::World, [Layer::Item, Layer::Robot, Layer::Player]),
                Transform::from_xyz(hall.translation.x, top - 0.1, hall.translation.z),
            ));
            let e = spawn_sorter(&mut c, hall, Vec2::new(12.0, 8.0), 6.0, depot_spots(), 4);
            c.entity(e).remove::<Senses>();
            q.apply(app.world_mut());
            e
        };
        (app, e, hall)
    }

    /// Left to itself, it sorts: totes end up moved from where they were,
    /// set down upright on the floor, and nothing is NaN.
    #[test]
    fn it_sorts_totes() {
        let (mut app, sorter, _) = app_with_sorter();
        for _ in 0..5 {
            app.update();
        }
        let before: Vec<(Entity, Vec3)> = app
            .world_mut()
            .query_filtered::<(Entity, &Transform), With<Tote>>()
            .iter(app.world())
            .map(|(e, t)| (e, t.translation))
            .collect();
        assert_eq!(before.len(), 4);
        for _ in 0..(90 * 60) {
            app.update();
            let p = app.world().get::<ArmPose>(sorter).unwrap();
            assert!(p.grab.is_finite() && p.elbow.is_finite());
        }
        let moved = before
            .iter()
            .filter(|(e, p)| {
                app.world()
                    .get::<Transform>(*e)
                    .is_some_and(|t| t.translation.distance(*p) > 1.0)
            })
            .count();
        assert!(moved >= 2, "it moved {moved} totes in a minute");
    }

    /// Ordered to throw at a point, it fetches a tote and throws it there.
    #[test]
    fn it_throws_a_tote_where_its_told() {
        let (mut app, sorter, hall) = app_with_sorter();
        let at = hall.transform_point(Vec3::new(6.0, 1.0, 5.0));
        app.world_mut()
            .entity_mut(sorter)
            .insert(SorterOrder::Throw { at });
        let mut thrown = false;
        for _ in 0..(90 * 25) {
            app.update();
            let s = app.world().get::<Sorter>(sorter).unwrap();
            // (It starts at rest with its cooldown running down.)
            if s.cooldown > THROW_COOLDOWN - 0.5 {
                thrown = true;
                break;
            }
        }
        assert!(thrown, "it threw");
        // After its tell (the carriage stopped, the head on the mark).
        let state = app.world().get::<SorterState>(sorter).unwrap();
        assert!(state.reach > 1.5, "reach {:.1}", state.reach);
        app.world_mut()
            .entity_mut(sorter)
            .insert(SorterOrder::Watch { at });
        // The closest a tote comes to the mark, low down (landing there).
        let mut near = f32::MAX;
        for _ in 0..(90 * 3) {
            app.update();
            for t in app
                .world_mut()
                .query_filtered::<&Transform, With<Tote>>()
                .iter(app.world())
            {
                if t.translation.y - at.y < 1.0 {
                    near = near
                        .min(Vec2::new(t.translation.x - at.x, t.translation.z - at.z).length());
                }
            }
        }
        assert!(near < 3.5, "a tote landed {near:.1} m from the mark");
    }

    /// Shot through the carriage, it comes down off its rails and rests on
    /// the floor, its arm still jointed to it or torn away, nothing NaN.
    #[test]
    fn it_comes_down_off_its_rails() {
        let (mut app, sorter, hall) = app_with_sorter();
        for _ in 0..90 {
            app.update();
        }
        app.world_mut().get_mut::<Robot>(sorter).unwrap().wrecked = true;
        for _ in 0..(90 * 16) {
            app.update();
        }
        let t = app.world().get::<Transform>(sorter).unwrap();
        assert!(t.translation.is_finite());
        let over = t.translation.y - hall.translation.y;
        assert!(over < 1.2, "it came down ({over:.2} m up)");
        assert_eq!(
            app.world().get::<RigidBody>(sorter),
            Some(&RigidBody::Static),
            "at rest"
        );
        let arm = app
            .world_mut()
            .query::<&Debris>()
            .iter(app.world())
            .filter(|d| matches!(d.piece, Piece::Arm(_)))
            .count();
        assert_eq!(arm, 2);
    }
}
