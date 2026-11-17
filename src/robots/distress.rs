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
            .add_message::<SignalLit>()
            .init_resource::<FlaresSaid>()
            .add_systems(
                Update,
                (
                    fit_tubes,
                    hit_tubes,
                    drop_flares,
                    tell,
                    climb,
                    hit_canisters,
                    signals,
                )
                    .chain(),
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
            // (Keyed on what it is too, not only how it finds its way: a
            // crawler spawned bare, as the gallery's, has no LocalNav.)
            Without<super::indoor::Crawler>,
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

/// A red star someone fired has lit (#94): the alternate extract opens on
/// it (Progression). `by` is who fired it. Machines' own bursts are not
/// sent.
#[derive(Message, Clone, Copy, Debug)]
pub struct SignalLit {
    pub at: Vec3,
    pub by: Option<Entity>,
}

/// The item a wreck's unfired canister drops as (Progression's).
pub const SIGNAL_FLARE: &str = "signal_flare";

/// A wrecked machine whose flare never went up drops it (#94): with its
/// tube whole, the canister falls at the tube as a `signal_flare` you can
/// take and fire yourself. A tube shot through drops nothing. Either way
/// the flare's spent; a wreck stopped in its tell still says so.
fn drop_flares(
    mut commands: Commands,
    catalog: Option<Res<crate::items::Catalog>>,
    mut robots: Query<(Entity, &Robot, &mut Distress, &Children)>,
    tubes: Query<(&FlareTube, &GlobalTransform)>,
    mut out: MessageWriter<FlareEvent>,
) {
    for (e, robot, mut d, children) in &mut robots {
        if !robot.wrecked || !matches!(*d, Distress::Ready | Distress::Telling(_)) {
            continue;
        }
        let Some((t, g)) = children.iter().find_map(|c| tubes.get(c).ok()) else {
            continue;
        };
        let at = g.translation();
        if matches!(*d, Distress::Telling(_)) {
            out.write(FlareEvent {
                robot: e,
                at,
                what: FlareAct::Stopped,
            });
        }
        *d = Distress::Spent;
        if t.hp <= 0.0 {
            continue;
        }
        let Some(catalog) = catalog.as_deref() else {
            continue;
        };
        let Some(kind) = catalog.0.kind(SIGNAL_FLARE) else {
            continue;
        };
        crate::items::spawn_item(
            &mut commands,
            &catalog.0,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_translation(at),
                condition: 1.0,
            },
        );
    }
}

/// A red star you fired, once it's lit, has been noticed by the Zone (it
/// already brings what sees it: `weapons::flares`).
#[derive(Component)]
pub struct Signalled;

/// A red star you fire is a distress signal like a machine's (#94): seen
/// across the valley, it stirs the account by a burst's worth ([`STIR`],
/// as `Why::Noise`) and owes an answer to the ground under it,
/// `pressure::CUE_LEAD` after it lights. The same rule wherever it's
/// fired: an alternate extract opens on it, and this is its price.
/// (A machine's own burst is its shooter's, and counted in `climb`.)
pub fn signals(
    mut commands: Commands,
    stars: Query<(Entity, &crate::weapons::flares::Flare, &GlobalTransform), Without<Signalled>>,
    machines: Query<(), With<Robot>>,
    zone: Option<Res<Zone>>,
    mut director: Option<ResMut<super::director::Director>>,
    mut doing: Option<ResMut<super::pressure::Doing>>,
    mut lit: MessageWriter<SignalLit>,
) {
    for (e, f, gt) in &stars {
        if f.star != crate::weapons::flares::Star::Red || !f.burning() {
            continue;
        }
        commands.entity(e).insert(Signalled);
        if f.shooter.is_some_and(|s| machines.contains(s)) {
            continue;
        }
        let at = gt.translation();
        lit.write(SignalLit { at, by: f.shooter });
        let ground = zone.as_deref().map_or(at, |z| z.on_ground(at.x, at.z));
        if let Some(d) = director.as_deref_mut() {
            d.queue_response(super::pressure::CUE_LEAD, ground);
            d.owed.push(ground);
        }
        if let Some(d) = doing.as_deref_mut() {
            d.flares += 1;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::flares::{Flare, Star};
    use bevy::ecs::system::RunSystemOnce;

    fn director() -> super::super::director::Director {
        super::super::director::Director {
            enabled: true,
            rng: super::super::director::raid_rng(1, 0),
            next_job: 25.0,
            responses: Vec::new(),
            response_cooldown: 0.0,
            max_active: 6,
            next_logistics: 150.0,
            chatter: Vec::new(),
            visits: Vec::new(),
            heat: 0.0,
            heat_gap: 0.0,
            broken_placed: false,
            quiet: 0.0,
            net: Default::default(),
            owed: Vec::new(),
        }
    }

    /// A red star you fire stirs it by a burst's worth and owes an answer
    /// under it, once, as it lights; a white one doesn't, nor a machine's
    /// own burst (counted where it bursts).
    #[test]
    fn your_red_star_stirs_like_a_distress_burst() {
        let mut app = App::new();
        app.init_resource::<super::super::pressure::Doing>()
            .add_message::<SignalLit>()
            .insert_resource(director());
        let w = app.world_mut();
        let machine = w
            .spawn(Robot {
                kind: RobotKind::Walker,
                wrecked: false,
            })
            .id();
        let at = |x: f32| GlobalTransform::from_translation(Vec3::new(x, 60.0, 0.0));
        w.spawn((Flare::lit(Star::White, Vec3::ZERO, None), at(0.0)));
        w.spawn((Flare::lit(Star::Red, Vec3::ZERO, Some(machine)), at(10.0)));
        let fired = w
            .spawn((Flare::lit(Star::Red, Vec3::ZERO, None), at(20.0)))
            .id();
        for _ in 0..3 {
            app.world_mut().run_system_once(signals).unwrap();
        }
        let w = app.world();
        assert_eq!(w.resource::<super::super::pressure::Doing>().flares, 1);
        let d = w.resource::<super::super::director::Director>();
        assert_eq!(d.owed.len(), 1, "one answer owed: {:?}", d.owed);
        assert!(
            (d.owed[0].x - 20.0).abs() < 0.01,
            "under your star: {:?}",
            d.owed
        );
        assert_eq!(d.responses.len(), 1);
        assert!((d.responses[0].0 - super::super::pressure::CUE_LEAD).abs() < 1e-6);
        assert!(w.entity(fired).contains::<Signalled>());
        // Told once, as yours: not the white, not the machine's.
        let lit: Vec<SignalLit> = w
            .resource::<Messages<SignalLit>>()
            .iter_current_update_messages()
            .copied()
            .collect();
        assert_eq!(lit.len(), 1, "{lit:?}");
        assert!((lit[0].at.x - 20.0).abs() < 0.01, "{lit:?}");
        assert!(lit.iter().all(|l| l.by.is_none()));
    }

    /// A wreck whose flare never went up drops it at the tube as a
    /// `signal_flare`; one whose tube was shot through drops nothing; a
    /// standing machine keeps its flare. Each wreck's flare is spent.
    #[test]
    fn a_wrecks_unfired_flare_drops_as_a_signal_flare() {
        let catalog = extraction_core::items::ItemCatalog::from_ron(
            r#"(items: [(id: "signal_flare", name: "Signal flare (red)", description: "",
                category: Gadget, size: Small, mass: 0.2, bulk: 0.1)])"#,
        )
        .unwrap();
        let mut app = App::new();
        app.add_message::<FlareEvent>()
            .insert_resource(crate::items::Catalog(std::sync::Arc::new(catalog)));
        let w = app.world_mut();
        let mut machine = |wrecked: bool, d: Distress, hp: f32, x: f32| {
            let tube = w
                .spawn((
                    FlareTube {
                        robot: Entity::PLACEHOLDER,
                        raised: 0.0,
                        hp,
                    },
                    GlobalTransform::from_translation(Vec3::new(x, 2.0, 0.0)),
                ))
                .id();
            let r = w
                .spawn((
                    Robot {
                        kind: RobotKind::Walker,
                        wrecked,
                    },
                    d,
                ))
                .id();
            w.entity_mut(r).add_child(tube);
            r
        };
        let whole = machine(true, Distress::Ready, TUBE_HP, 5.0);
        let telling = machine(true, Distress::Telling(0.5), TUBE_HP, 9.0);
        let shot = machine(true, Distress::Ready, 0.0, 30.0);
        let fired = machine(true, Distress::Spent, TUBE_HP, 40.0);
        let standing = machine(false, Distress::Ready, TUBE_HP, 50.0);
        for _ in 0..3 {
            app.world_mut().run_system_once(drop_flares).unwrap();
        }
        let w = app.world_mut();
        let mut dropped: Vec<f32> = w
            .query::<(&crate::items::Item, &Transform)>()
            .iter(w)
            .map(|(_, t)| t.translation.x)
            .collect();
        dropped.sort_by(f32::total_cmp);
        assert_eq!(dropped, vec![5.0, 9.0], "at the whole tubes, once each");
        for r in [whole, telling, shot, fired] {
            assert_eq!(*w.get::<Distress>(r).unwrap(), Distress::Spent);
        }
        assert_eq!(*w.get::<Distress>(standing).unwrap(), Distress::Ready);
    }
}
