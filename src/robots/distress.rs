//! Distress flares (#79, the coordinator's design): a walker or a sentry
//! that has you and is visibly failing (a part `wear::failing`: the state
//! that sends up Heavy's soot wisp and gutters its eye) may call
//! for help, once. It's a flare you can stop:
//!
//! - **The tell** ([`TELL`]): the tube rises, its lamp ticking. Put a
//!   round into it, the tube ([`FlareTube`]) or any part of it ("shoot the
//!   one that's smoking"), kill it, or EMP it, and nothing goes up.
//! - **The climb** ([`CLIMB`] to [`HEIGHT`]): a canister on a hissing
//!   trail. Shoot it down and it fizzles where it falls, calling nothing.
//! - **The burst**: a red signal star (`weapons::flares`), seen from
//!   `flares::SEEN_SIGNAL`. Only now does anything know: the director owes
//!   a response to the ground under it (`Director::owed`: the radio says
//!   they'll come, so they come), the account takes [`STIR`] (a step's
//!   worth, still a step at a time), and the star draws what sees it.
//!
//! Never from the broken walker (the fight a newcomer is meant to win),
//! never at Quiet (a flare is something you made happen), never from a
//! resident (`indoor::LocalNav`: a flare in a hall goes nowhere).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Awareness, Robot, RobotKind, RobotPart, Stunned};
use crate::weapons::ballistics::Hit;
use crate::world::{Layer, Zone};

/// The tell before it fires (s): longer than every attack's readable lead
/// (`visuals::MIN_LEAD`), slow before fast: time to see the tube go up and
/// put a round in the machine (coordinator, #79: at 1.5 s, a pistol in a
/// headset stopped half; at 2.0 s with any round spoiling it, see the
/// drill in `mind_tests`).
pub const TELL: f32 = 2.0;
/// The canister's climb (s) and how high it bursts (m above the tube).
pub const CLIMB: f32 = 2.5;
pub const HEIGHT: f32 = 60.0;
/// The tube's hit points: one round from anything, a pistol's included.
pub const TUBE_HP: f32 = 30.0;
/// The tube's housing (m, whole lengths), what a round must strike: big
/// enough that someone aiming at it in its tell, as a person in a
/// headset aims, hits it more often than not (a 0.12 m tube took no round
/// in eight tries at 15 m: `mind_tests`' drill).
pub const TUBE_SIZE: Vec3 = Vec3::new(0.3, 0.6, 0.3);
/// What a round must strike of the canister: generous against its look
/// (0.12 m), so it's a fair pistol shot at 20-40 m (Weapon feel's call).
pub const CANISTER_RADIUS: f32 = 0.2;
/// What a burst stirs the account by: a level's worth (still a level at
/// a time, `pressure::STEP_GAP` apart).
pub const STIR: f32 = 1.0;

/// What a flare did, for Audio (a ratchet, a hiss, a pop, a fizzle, a
/// clank) and the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlareAct {
    /// The tube's up and ticking: [`TELL`] to stop it.
    Tell,
    /// The canister's away (`at`: the tube).
    Launch,
    /// It burst (`at`: up there): a response is coming.
    Burst,
    /// Shot down in its climb (`at`: where it was hit).
    Downed,
    /// Stopped in its tell: a round in it (the tube or any part), the
    /// machine wrecked or stunned.
    Stopped,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct FlareEvent {
    pub robot: Entity,
    pub at: Vec3,
    pub what: FlareAct,
}

/// A machine's flare tube: its own piece (not a `PartRole`), a child of
/// the robot. `raised` goes 0 to 1 over the tell (Robot bodies' pose).
#[derive(Component, Debug)]
pub struct FlareTube {
    pub robot: Entity,
    pub raised: f32,
    pub hp: f32,
}

/// Where a machine's flare is.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub enum Distress {
    #[default]
    Ready,
    /// In its tell, this long (s).
    Telling(f32),
    /// A round struck it in its tell: the launch is spoiled.
    Spoiled,
    /// Fired, or stopped: never again.
    Spent,
}

/// A flare canister climbing.
#[derive(Component, Debug)]
pub struct Canister {
    pub robot: Entity,
    pub from: Vec3,
    pub age: f32,
}

/// What the radio has said of flares this raid (it teaches once).
#[derive(Resource, Default, Debug)]
pub struct FlaresSaid {
    pub up: bool,
    pub downed: bool,
    /// Bursts said this raid (which of Story's words is next).
    pub bursts: u32,
}

pub struct DistressPlugin;

impl Plugin for DistressPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FlareEvent>()
            .init_resource::<FlaresSaid>()
            .add_systems(
                Update,
                (fit_tubes, hit_tubes, tell, climb, hit_canisters).chain(),
            );
    }
}

/// The canister's climbing speed `t` s into the climb (m/s): fast off the
/// tube, slowing to hang at the top.
pub fn climb_speed(t: f32) -> f32 {
    let v0 = 2.0 * HEIGHT / CLIMB;
    (v0 * (1.0 - t / CLIMB)).max(0.0)
}

/// The canister's height above its tube `t` s into the climb.
pub fn climb_height(t: f32) -> f32 {
    let t = t.clamp(0.0, CLIMB);
    let v0 = 2.0 * HEIGHT / CLIMB;
    v0 * t - 0.5 * (v0 / CLIMB) * t * t
}

/// Where a kind's tube stands while it's up, in the robot's frame (a
/// stand-in: Robot bodies' mounts and raised pose to come): a walker's up
/// behind its gun pod (clear of it: the pod runs the top, z -0.4 to 0.4,
/// up to y 0.58), a sentry's on its free left flank; both where you can
/// see it, and hit it, from in front (`mind_tests`' drill).
fn mount(kind: Mount) -> Vec3 {
    match kind {
        Mount::Walker => Vec3::new(0.0, 0.9, 0.3),
        Mount::Sentry => Vec3::new(-0.72, 0.05, 0.1),
    }
}

#[derive(Clone, Copy)]
enum Mount {
    Walker,
    Sentry,
}

/// Every walker and sentry that may call gets a tube (and a flare).
#[allow(clippy::type_complexity)]
fn fit_tubes(
    mut commands: Commands,
    robots: Query<
        (Entity, &Robot, Has<super::sentry::Sentry>),
        (
            Without<Distress>,
            Without<super::broken::Broken>,
            Without<super::indoor::LocalNav>,
            Without<super::grappler::Grappler>,
            Without<super::collector::Collector>,
        ),
    >,
) {
    for (e, robot, sentry) in &robots {
        let m = if sentry {
            Mount::Sentry
        } else if robot.kind == RobotKind::Walker {
            Mount::Walker
        } else {
            continue;
        };
        commands
            .entity(e)
            .insert(Distress::Ready)
            .with_children(|c| {
                c.spawn((
                    Name::new("Flare tube"),
                    FlareTube {
                        robot: e,
                        raised: 0.0,
                        hp: TUBE_HP,
                    },
                    crate::weapons::impacts::StruckMetal,
                    Collider::cuboid(TUBE_SIZE.x, TUBE_SIZE.y, TUBE_SIZE.z),
                    CollisionLayers::new(Layer::Robot, [Layer::World, Layer::Robot]),
                    Transform::from_translation(mount(m)),
                    Visibility::default(),
                ));
            });
    }
}

/// Rounds on a machine in its tell: the tube takes its damage, and any
/// round that bites, on the tube or any part of it, spoils the launch.
fn hit_tubes(
    mut hits: MessageReader<Hit>,
    mut tubes: Query<&mut FlareTube>,
    parts: Query<&RobotPart>,
    mut machines: Query<&mut Distress>,
) {
    for h in hits.read() {
        if h.damage <= 0.0 {
            continue;
        }
        let robot = if let Ok(mut t) = tubes.get_mut(h.entity) {
            t.hp -= h.damage;
            t.robot
        } else if let Ok(p) = parts.get(h.entity) {
            p.robot
        } else {
            continue;
        };
        if let Ok(mut d) = machines.get_mut(robot)
            && matches!(*d, Distress::Telling(_))
        {
            *d = Distress::Spoiled;
        }
    }
}

/// Whether a machine shows it's failing: any part of it `wear::failing`
/// (Heavy's one test: what sends up its soot and gutters its lights).
pub fn failing<'a>(parts: impl IntoIterator<Item = &'a RobotPart>) -> bool {
    parts.into_iter().any(super::wear::failing)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tell(
    mut commands: Commands,
    mut robots: Query<(
        Entity,
        &Robot,
        &Awareness,
        &mut Distress,
        Has<Stunned>,
        &Children,
    )>,
    parts: Query<&RobotPart>,
    mut tubes: Query<(&mut FlareTube, &GlobalTransform)>,
    me: Query<Entity, With<crate::rig::LocalPlayer>>,
    pressure: Option<Res<super::pressure::Pressure>>,
    mut out: MessageWriter<FlareEvent>,
    mut raid: Option<ResMut<crate::raid::Raid>>,
    mut said: ResMut<FlaresSaid>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    // The radio teaches once a raid.
    let out_there = raid.as_deref().is_some_and(|r| {
        matches!(
            r.phase,
            crate::raid::Phase::Out
                | crate::raid::Phase::TruckCalled { .. }
                | crate::raid::Phase::TruckWaiting { .. }
        )
    });
    if !out_there {
        *said = FlaresSaid::default();
    }
    let me = me.single().ok();
    let stirred = pressure.is_some_and(|p| p.level >= super::pressure::Level::Stirred);
    for (e, robot, aware, mut d, stunned, children) in &mut robots {
        let Some(tube) = children.iter().find(|c| tubes.contains(*c)) else {
            continue;
        };
        let Ok((mut t, g)) = tubes.get_mut(tube) else {
            continue;
        };
        let tgt = g.translation();
        let has_you = me.is_some() && aware.target == me && aware.can_see_target;
        match *d {
            Distress::Ready => {
                let mine = parts.iter().filter(|p| p.robot == e);
                if !robot.wrecked && !stunned && t.hp > 0.0 && has_you && stirred && failing(mine) {
                    *d = Distress::Telling(0.0);
                    out.write(FlareEvent {
                        robot: e,
                        at: tgt,
                        what: FlareAct::Tell,
                    });
                }
            }
            Distress::Spoiled => {
                *d = Distress::Spent;
                t.raised = 0.0;
                out.write(FlareEvent {
                    robot: e,
                    at: tgt,
                    what: FlareAct::Stopped,
                });
            }
            Distress::Telling(age) => {
                if robot.wrecked || stunned || t.hp <= 0.0 {
                    *d = Distress::Spent;
                    t.raised = 0.0;
                    out.write(FlareEvent {
                        robot: e,
                        at: tgt,
                        what: FlareAct::Stopped,
                    });
                    continue;
                }
                let age = age + dt;
                t.raised = (age / TELL).min(1.0);
                if age < TELL {
                    *d = Distress::Telling(age);
                    continue;
                }
                *d = Distress::Spent;
                commands.spawn((
                    Name::new("Flare canister"),
                    Canister {
                        robot: e,
                        from: tgt,
                        age: 0.0,
                    },
                    crate::weapons::impacts::StruckMetal,
                    // Moved by its velocity, so the physics index (what a
                    // round's ray tests) keeps up with it (Weapon feel).
                    RigidBody::Kinematic,
                    LinearVelocity(Vec3::Y * climb_speed(0.0)),
                    Collider::sphere(CANISTER_RADIUS),
                    CollisionLayers::new(Layer::Robot, [Layer::World]),
                    Transform::from_translation(tgt),
                    Visibility::default(),
                ));
                out.write(FlareEvent {
                    robot: e,
                    at: tgt,
                    what: FlareAct::Launch,
                });
                if !said.up {
                    said.up = true;
                    if let Some(r) = raid.as_deref_mut() {
                        r.say(crate::raid::lines::FLARE_UP);
                    }
                }
            }
            Distress::Spent => {
                t.raised = (t.raised - dt).max(0.0);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn climb(
    mut commands: Commands,
    mut cans: Query<(Entity, &mut Canister, &Transform, &mut LinearVelocity)>,
    mut out: MessageWriter<FlareEvent>,
    mut director: Option<ResMut<super::director::Director>>,
    mut doing: Option<ResMut<super::pressure::Doing>>,
    mut raid: Option<ResMut<crate::raid::Raid>>,
    mut said: ResMut<FlaresSaid>,
    zone: Option<Res<Zone>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut c, t, mut v) in &mut cans {
        c.age += dt;
        v.0 = Vec3::Y * climb_speed(c.age);
        if c.age < CLIMB {
            continue;
        }
        let at = t.translation;
        commands.entity(e).despawn();
        commands.spawn((
            Name::new("Flare"),
            crate::weapons::flares::Flare::lit(
                crate::weapons::flares::Star::Red,
                Vec3::ZERO,
                Some(c.robot),
            ),
            Transform::from_translation(at),
            Visibility::default(),
        ));
        out.write(FlareEvent {
            robot: c.robot,
            at,
            what: FlareAct::Burst,
        });
        // They come to where it went up from: the director owes it.
        let ground = zone
            .as_deref()
            .map_or(c.from, |z| z.on_ground(c.from.x, c.from.z));
        if let Some(d) = director.as_deref_mut() {
            d.queue_response(super::pressure::CUE_LEAD, ground);
            d.owed.push(ground);
        }
        if let Some(d) = doing.as_deref_mut() {
            d.flares += 1;
        }
        // Every burst says something's coming (it is), each the next of
        // Story's words: a second inside the radio's 30 s repeat throttle
        // is still heard.
        if let Some(r) = raid.as_deref_mut() {
            r.say(crate::raid::lines::pick(
                crate::raid::lines::FLARE_BURST,
                said.bursts,
            ));
        }
        said.bursts += 1;
    }
}

fn hit_canisters(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    cans: Query<&Canister>,
    mut out: MessageWriter<FlareEvent>,
    mut raid: Option<ResMut<crate::raid::Raid>>,
    mut said: ResMut<FlaresSaid>,
) {
    for h in hits.read() {
        let Ok(c) = cans.get(h.entity) else {
            continue;
        };
        // (It burst this tick: too late.)
        if h.damage <= 0.0 || c.age >= CLIMB {
            continue;
        }
        commands.entity(h.entity).try_despawn();
        out.write(FlareEvent {
            robot: c.robot,
            at: h.point,
            what: FlareAct::Downed,
        });
        if !said.downed {
            said.downed = true;
            if let Some(r) = raid.as_deref_mut() {
                r.say(crate::raid::lines::FLARE_DOWNED);
            }
        }
    }
}
