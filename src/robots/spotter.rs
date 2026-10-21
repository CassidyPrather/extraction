//! The Custodian's spotter: an observation balloon on a tether, a camera
//! slung under it, turning a slow circle over a valley. It carries no
//! gun. What it does is watch, and when it has you it calls the others.
//! (Its rules are `extraction_core::spotter`.)
//!
//! - **The tell.** Its beacon blinks slow amber while it watches. When it
//!   sees you, its eye swings round to hold you and the beacon quickens.
//!   If it gets a full fix, a klaxon carries across the valley and a
//!   response sets out for where you stood.
//! - **The weakness.** It's on a string. Get to the winch (straight under
//!   it is blind) and cut the tether with bolt cutters, or wreck the winch:
//!   it rises and drifts off downwind, trailing its rope, and is gone.
//! - **The salvage.** Hole the envelope and it sinks to the ground. Its
//!   gas valve, the envelope's fabric and (if you didn't shoot it out)
//!   its camera's eye lie in the wreck.
//! - An EMP blinds it for a while, and it can't call.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::spotter::{self as rules, Watch, WatchAct};

use super::{Awareness, Brain, Job, JobKind, PartRole, Robot, RobotKind, Senses, part_bundle};
use crate::world::{Layer, Zone};

/// What its parts take. The envelope is big and easy to hit, but it
/// takes a fair few rounds to hole it enough to sink.
pub const ENVELOPE_HP: f32 = 160.0;
pub const EYE_HP: f32 = 30.0;
pub const GONDOLA_HP: f32 = 80.0;
pub const WINCH_HP: f32 = 140.0;
/// The envelope's radius (its collider).
pub const ENVELOPE_R: f32 = 1.4;
/// Its eye and gondola, under the envelope.
pub const EYE: Vec3 = Vec3::new(0.0, -2.0, -0.27);
pub const GONDOLA: Vec3 = Vec3::new(0.0, -2.0, 0.0);
/// Where the tether meets the envelope (local), and the winch's top.
pub const TETHER_TOP: Vec3 = Vec3::new(0.45, -1.3, 0.0);
pub const WINCH_TOP: f32 = 0.9;
/// The tether's stretches, and how high off the winch a stretch can be
/// reached by a hand with cutters.
pub const STRETCHES: usize = 24;
pub const REACH: f32 = 2.2;
/// Degrees a second its eye turns to hold you (slow enough to see).
pub const TRACK_SPEED: f32 = 35.0;
/// The counter a scenario reads: calls made.
pub const CALLS: &str = "spotter_calls";

/// A spotter (its root: the envelope's centre, turning with its eye).
#[derive(Component, Debug)]
pub struct Spotter {
    /// The winch on the ground.
    pub winch: Vec3,
    /// Seconds since it went up (its circle and sway).
    pub clock: f32,
    /// Where its circle started, degrees.
    pub phase: f32,
    /// Where its eye points, degrees (the game's yaw).
    pub yaw: f32,
    pub watch: Watch,
    pub float: Float,
}

/// How it's flying.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Float {
    /// On its tether.
    Aloft,
    /// Cut loose: seconds since, and where it was.
    Loose(f32, Vec3),
    /// Holed: coming down.
    Sinking,
    /// On the ground, a wreck.
    Down,
}

/// The rest of a spotter: the winch, and each stretch of its tether (by
/// its place from the winch up).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotterPiece {
    Winch,
    Tether(usize),
}

/// Which spotter a piece belongs to.
#[derive(Component, Debug, Clone, Copy)]
pub struct OfSpotter(pub Entity);

/// A stretch of tether's length now (for its look).
#[derive(Component, Debug, Clone, Copy)]
pub struct TetherLength(pub f32);

/// What a spotter did, for sound and light.
#[derive(Message, Debug, Clone, Copy)]
pub struct SpotterEvent {
    pub spotter: Entity,
    pub at: Vec3,
    pub what: SpotterAct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotterAct {
    /// It's seen you and is building a fix.
    Fixing,
    /// The klaxon: it's called the others.
    Call,
    /// Cut loose.
    Loose,
    /// Holed, it's coming down.
    Holed,
    /// On the ground.
    Down,
    /// Drifted off out of the valley.
    Gone,
}

pub struct SpotterPlugin;

impl Plugin for SpotterPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpotterEvent>()
            .init_resource::<crate::bot::Counters>()
            .init_resource::<Severed>()
            .add_observer(tether_gone)
            .add_systems(
                FixedUpdate,
                (spotter_loose, spotter_mind, spotter_fly, spotter_salvage)
                    .chain()
                    .after(super::perceive)
                    .before(PhysicsSystems::First),
            )
            .add_systems(FixedUpdate, (place_spotters, stage_scenario));
    }
}

/// A spotter over a winch at `at` (x/z, on the ground).
pub fn spawn_spotter(commands: &mut Commands, zone: &Zone, at: Vec2) -> Entity {
    let winch = zone.on_ground(at.x, at.y);
    let seed = super::senses::mix(
        (at.x.to_bits() as u64) | ((at.y.to_bits() as u64) << 32),
        0x5907,
    );
    let phase = (seed % 360) as f32;
    let robot = commands
        .spawn((
            Name::new("Spotter"),
            Robot {
                kind: RobotKind::Pylon,
                wrecked: false,
            },
            super::Breed("spotter"),
            super::heavy::OwnLook,
            Spotter {
                winch,
                clock: 0.0,
                phase,
                yaw: phase,
                watch: Watch::default(),
                float: Float::Aloft,
            },
            Senses {
                sight: rules::SIGHT,
                fov_deg: rules::FOV,
                hearing: 0.0,
                eye: EYE,
            },
            Awareness::default(),
            Brain::Working {
                at: winch,
                left: f32::INFINITY,
            },
            Job {
                kind: JobKind::Watch,
                site: winch,
                duration: f32::INFINITY,
                exit: winch,
            },
            Transform::from_translation(winch + Vec3::Y * rules::TETHER)
                .with_rotation(Quat::from_rotation_y(-phase.to_radians())),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Spotter envelope"),
            part_bundle(
                robot,
                PartRole::Core,
                ENVELOPE_HP,
                Collider::sphere(ENVELOPE_R),
                Transform::IDENTITY,
            ),
        ));
        c.spawn((
            Name::new("Spotter eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                EYE_HP,
                Collider::cuboid(0.2, 0.15, 0.08),
                Transform::from_translation(EYE),
            ),
        ));
        c.spawn((
            Name::new("Spotter gondola"),
            part_bundle(
                robot,
                PartRole::Armour(30),
                GONDOLA_HP,
                Collider::cuboid(0.5, 0.4, 0.5),
                Transform::from_translation(GONDOLA),
            ),
        ));
    });
    commands.spawn((
        Name::new("Spotter winch"),
        SpotterPiece::Winch,
        OfSpotter(robot),
        part_bundle(
            robot,
            PartRole::Armour(40),
            WINCH_HP,
            Collider::cuboid(1.2, 0.8, 0.9),
            Transform::from_translation(winch + Vec3::Y * 0.4),
        ),
    ));
    let top = winch + Vec3::Y * rules::TETHER + TETHER_TOP;
    for (k, (mid, len, rot)) in tether_line(winch + Vec3::Y * WINCH_TOP, top)
        .into_iter()
        .enumerate()
    {
        let mut e = commands.spawn((
            Name::new("Spotter tether"),
            SpotterPiece::Tether(k),
            OfSpotter(robot),
            Transform::from_translation(mid).with_rotation(rot),
            TetherLength(len),
            Visibility::default(),
        ));
        if mid.y - winch.y < WINCH_TOP + REACH {
            // In reach of a hand: what the cutters' jaws find.
            e.insert((
                crate::destruction::hooks::Cuttable { needs: 0.0 },
                Sensor,
                Collider::cuboid(0.06, 0.06, len),
                CollisionLayers::new(Layer::Hand, LayerMask::NONE),
            ));
        }
    }
    robot
}

/// The tether from the winch to the envelope: [`STRETCHES`] stretches
/// (middle, length, turned along it), sagging a little downwind.
pub fn tether_line(from: Vec3, to: Vec3) -> Vec<(Vec3, f32, Quat)> {
    let wind = Vec3::new(rules::WIND[0], 0.0, rules::WIND[1]).normalize_or_zero();
    let sag = (to - from).length() * 0.04;
    let at = |k: usize| {
        let f = k as f32 / STRETCHES as f32;
        from.lerp(to, f) + wind * sag * 4.0 * f * (1.0 - f)
    };
    (0..STRETCHES)
        .map(|k| {
            let (a, b) = (at(k), at(k + 1));
            let d = b - a;
            (
                (a + b) * 0.5,
                d.length(),
                Quat::from_rotation_arc(Vec3::Z, d.normalize_or(Vec3::Y)),
            )
        })
        .collect()
}

/// Spotters whose tether lost a stretch since [`spotter_loose`] looked.
#[derive(Resource, Default)]
struct Severed(Vec<Entity>);

/// A stretch of tether taken away (Destruction despawns what the cutters
/// cut).
fn tether_gone(
    gone: On<Remove, SpotterPiece>,
    pieces: Query<(&SpotterPiece, &OfSpotter)>,
    mut severed: ResMut<Severed>,
) {
    if let Ok((SpotterPiece::Tether(_), of)) = pieces.get(gone.entity) {
        severed.0.push(of.0);
    }
}

/// The tether cut or the winch wrecked: loose. Holed: sinking. Pieces of
/// a spotter that's gone go with it.
#[allow(clippy::too_many_arguments)]
fn spotter_loose(
    mut commands: Commands,
    mut cuts: MessageReader<crate::destruction::hooks::Cut>,
    mut destroyed: MessageReader<super::PartDestroyed>,
    mut severed: ResMut<Severed>,
    pieces: Query<(Entity, &SpotterPiece, &OfSpotter)>,
    mut spotters: Query<(Entity, &mut Spotter, &Robot, &Transform)>,
    mut out: MessageWriter<SpotterEvent>,
) {
    for (e, _, of) in &pieces {
        if !spotters.contains(of.0) {
            commands.entity(e).despawn();
        }
    }
    let mut loose: Vec<Entity> = std::mem::take(&mut severed.0);
    for c in cuts.read() {
        if let Ok((_, SpotterPiece::Tether(_), of)) = pieces.get(c.target) {
            loose.push(of.0);
        }
    }
    for d in destroyed.read() {
        if let Ok((_, SpotterPiece::Winch, of)) = pieces.get(d.part) {
            loose.push(of.0);
        }
    }
    for (e, mut s, robot, t) in &mut spotters {
        if robot.wrecked && matches!(s.float, Float::Aloft | Float::Loose(..)) {
            s.float = Float::Sinking;
            out.write(SpotterEvent {
                spotter: e,
                at: t.translation,
                what: SpotterAct::Holed,
            });
            continue;
        }
        if s.float == Float::Aloft && loose.contains(&e) {
            s.float = Float::Loose(0.0, t.translation);
            info!("spotter: cut loose at {:.0}", t.translation);
            out.write(SpotterEvent {
                spotter: e,
                at: t.translation,
                what: SpotterAct::Loose,
            });
            // What's left of the tether below goes slack on the ground;
            // the rest trails under it (`spotter_fly`). Nothing to cut now.
            for (p, piece, of) in &pieces {
                if of.0 == e && matches!(piece, SpotterPiece::Tether(_)) {
                    commands
                        .entity(p)
                        .try_remove::<(crate::destruction::hooks::Cuttable, Collider)>();
                }
            }
        }
    }
}

/// Turn, look, fix, call.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn spotter_mind(
    mut spotters: Query<(
        Entity,
        &mut Spotter,
        &Transform,
        &Awareness,
        Has<super::Stunned>,
    )>,
    targets: Query<&GlobalTransform>,
    mut director: Option<ResMut<super::director::Director>>,
    mut noise: MessageWriter<super::Noise>,
    mut out: MessageWriter<SpotterEvent>,
    mut counters: ResMut<crate::bot::Counters>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut s, t, aware, stunned) in &mut spotters {
        s.clock += dt;
        if s.float != Float::Aloft || stunned {
            s.watch.fix = 0.0;
            continue;
        }
        let at = t.translation;
        let seen = aware
            .target
            .filter(|_| aware.can_see_target && aware.suspicion >= 0.5)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation());
        // Its eye holds you; otherwise it goes round its circle.
        let want = match seen {
            Some(p) => extraction_core::sentry::bearing([at.x, at.z], [p.x, p.z]),
            None => rules::pan_yaw(s.clock, s.phase),
        };
        let step = TRACK_SPEED * dt;
        let off = extraction_core::sentry::wrap_deg(want - s.yaw);
        s.yaw = extraction_core::sentry::wrap_deg(s.yaw + off.clamp(-step, step));
        let was = s.watch.fix;
        match s.watch.step(seen.is_some(), dt) {
            WatchAct::Fixing if was == 0.0 => {
                out.write(SpotterEvent {
                    spotter: e,
                    at,
                    what: SpotterAct::Fixing,
                });
            }
            WatchAct::Call => {
                let Some(p) = seen else { continue };
                info!("spotter: called the others to {p:.0}");
                if let Some(d) = director.as_mut() {
                    d.queue_response(rules::RESPONSE_DELAY, p);
                }
                noise.write(super::Noise {
                    at,
                    radius: rules::KLAXON,
                    source: Some(e),
                });
                counters.bump(CALLS);
                out.write(SpotterEvent {
                    spotter: e,
                    at,
                    what: SpotterAct::Call,
                });
            }
            _ => {}
        }
    }
}

/// Where it flies: on its tether, swaying; loose, up and away; holed, down
/// to the ground. And its tether with it.
#[allow(clippy::type_complexity)]
fn spotter_fly(
    mut commands: Commands,
    mut spotters: Query<(
        Entity,
        &mut Spotter,
        &mut Transform,
        Option<&super::senses::MindSeed>,
    )>,
    mut pieces: Query<
        (&SpotterPiece, &OfSpotter, &mut Transform, &mut TetherLength),
        Without<Spotter>,
    >,
    zone: Res<Zone>,
    mut out: MessageWriter<SpotterEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut s, mut t, seed) in &mut spotters {
        let seed = seed.map_or(0, |m| m.0);
        let yaw = Quat::from_rotation_y(-s.yaw.to_radians());
        let mut tethered = true;
        match s.float {
            Float::Aloft => {
                let w = rules::sway(s.clock, seed);
                t.translation = s.winch + Vec3::new(w[0], rules::TETHER, w[1]);
            }
            Float::Loose(age, from) => {
                let age = age + dt;
                s.float = Float::Loose(age, from);
                let o = rules::loose_offset(age);
                t.translation = from + Vec3::from(o);
                tethered = false;
                if age > rules::GONE_AFTER {
                    out.write(SpotterEvent {
                        spotter: e,
                        at: t.translation,
                        what: SpotterAct::Gone,
                    });
                    commands.entity(e).despawn();
                    continue;
                }
            }
            Float::Sinking => {
                let ground = zone.ground(t.translation.x, t.translation.z);
                let drift = Vec3::new(rules::WIND[0], 0.0, rules::WIND[1]) * 0.25;
                t.translation += (Vec3::NEG_Y * rules::SINK + drift) * dt;
                if t.translation.y - ground < ENVELOPE_R * 0.5 {
                    t.translation.y = ground + ENVELOPE_R * 0.5;
                    s.float = Float::Down;
                    commands
                        .entity(e)
                        .insert((super::wreck::Broken, super::Salvaged));
                    out.write(SpotterEvent {
                        spotter: e,
                        at: t.translation,
                        what: SpotterAct::Down,
                    });
                }
            }
            Float::Down => {}
        }
        t.rotation = yaw;
        // The tether: from the winch to it, or trailing under it.
        let top = t.translation + yaw * TETHER_TOP;
        let line = if tethered {
            tether_line(s.winch + Vec3::Y * WINCH_TOP, top)
        } else {
            let hang = top - Vec3::Y * 0.6 * STRETCHES as f32;
            tether_line(hang, top)
        };
        for (piece, of, mut pt, mut len) in &mut pieces {
            let SpotterPiece::Tether(k) = *piece else {
                continue;
            };
            if of.0 != e {
                continue;
            }
            if s.float == Float::Down {
                // Slack on the ground from the winch to the wreck.
                let f = (k as f32 + 0.5) / STRETCHES as f32;
                let p = s.winch.lerp(t.translation, f);
                pt.translation = zone.on_ground(p.x, p.z) + Vec3::Y * 0.03;
                continue;
            }
            let (mid, l, rot) = line[k];
            pt.translation = mid;
            pt.rotation = rot;
            len.0 = l;
        }
    }
}

/// Its salvage, once it's down: the gas valve, the envelope's fabric, and
/// its eye if it's whole.
fn spotter_salvage(
    mut commands: Commands,
    mut events: MessageReader<SpotterEvent>,
    parts: Query<(&super::RobotPart, &ChildOf)>,
    catalog: Res<crate::items::Catalog>,
) {
    for ev in events.read() {
        if ev.what != SpotterAct::Down {
            continue;
        }
        let eye_whole = parts
            .iter()
            .any(|(p, c)| c.parent() == ev.spotter && p.role == PartRole::Sensor && p.alive());
        let mut drops = vec!["dirigible_gas_valve", "polymer", "polymer"];
        if eye_whole {
            drops.push("sensor_eye");
        }
        for (k, id) in drops.into_iter().enumerate() {
            let Some(kind) = catalog.kind(id) else {
                continue;
            };
            let a = k as f32 * 1.9;
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        ev.at + Vec3::new(a.cos() * 1.2, 0.3, a.sin() * 1.2),
                    ),
                    condition: 0.7,
                },
            );
        }
    }
}

/// The map's spotters, put up once the Zone is in (with robots on).
fn place_spotters(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    opts: Option<Res<crate::LaunchOptions>>,
    mut done: Local<bool>,
) {
    let Some(zone) = zone else { return };
    if *done {
        return;
    }
    *done = true;
    if !opts.is_some_and(|o| o.robots) {
        return;
    }
    for m in &zone.map.markers {
        if m.kind == extraction_core::map::MarkerKind::Spotter {
            spawn_spotter(&mut commands, &zone, Vec2::from(m.at));
        }
    }
}

/// The scenario: a spotter up ahead; go in under it, cut its tether, and
/// walk on out across the ground it watched.
pub const SCENARIO: &str = "spotter_tether";
/// The same without the cut (run it by name): it calls.
pub const CONTROL: &str = "spotter_control";
/// How far ahead of the bot its winch goes, and how far past it the bot
/// walks after.
pub const STAGE_AHEAD: f32 = 12.0;
pub const STAGE_PAST: f32 = 45.0;
/// Filled in by the setup: where the bot walks to after the cut.
pub const PAST_PLACEHOLDER: Vec2 = Vec2::new(-2e6, -2e6);

fn stage_scenario(
    mut commands: Commands,
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    zone: Res<Zone>,
    nav: Res<super::walker::Nav>,
    catalog: Res<crate::items::Catalog>,
    mut bots: Query<&mut crate::bot::Bot>,
    anomalies: Query<(&crate::anomaly::Anomaly, &GlobalTransform)>,
) {
    let clear = |p: Vec3| {
        anomalies.iter().all(|(a, g)| {
            let c = g.translation();
            Vec2::new(p.x - c.x, p.z - c.z).length() > a.kind.reach(a.radius) + 3.0
        })
    };
    for s in setups.read() {
        if s.name != SCENARIO {
            continue;
        }
        let f = Vec3::new(s.facing.x, 0.0, s.facing.z).normalize_or(Vec3::NEG_Z);
        let open = |d: Vec3| {
            (2..=((STAGE_AHEAD + STAGE_PAST) as usize + 2))
                .step_by(2)
                .all(|m| {
                    let p = s.at + d * m as f32;
                    nav.0.walkable([p.x, p.z]) && clear(p)
                })
        };
        let dir = (0..16)
            .map(|k| Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * f)
            .find(|d| open(*d))
            .unwrap_or(f);
        let winch = s.at + dir * STAGE_AHEAD;
        let past = winch + dir * STAGE_PAST;
        info!("scenario: a spotter's winch at {winch:.0}");
        for mut bot in &mut bots {
            for t in bot.tasks.iter_mut() {
                if let crate::bot::Task::WalkTo { to, .. } = t {
                    if *to == PAST_PLACEHOLDER {
                        *to = Vec2::new(past.x, past.z);
                    }
                }
            }
        }
        // Its eye starts looking the other way: you came in unseen.
        let e = spawn_spotter(&mut commands, &zone, Vec2::new(winch.x, winch.z));
        let away = extraction_core::sentry::bearing([s.at.x, s.at.z], [past.x, past.z]) + 180.0;
        commands.queue(move |w: &mut World| {
            if let Some(mut sp) = w.get_mut::<Spotter>(e) {
                sp.phase = away;
                sp.yaw = away;
            }
        });
        if let Some(kind) = catalog.kind("bolt_cutters") {
            let side = Vec3::new(-dir.z, 0.0, dir.x);
            let p = s.at + side * 0.7 + dir * 0.5;
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        zone.on_ground(p.x, p.z) + Vec3::Y * 0.1,
                    ),
                    condition: 1.0,
                },
            );
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Acts(Vec<SpotterAct>);

    fn collect(mut ev: MessageReader<SpotterEvent>, mut acts: ResMut<Acts>) {
        acts.0.extend(ev.read().map(|e| e.what));
    }

    /// A player in the open under its watch: it turns to them, builds a
    /// fix and calls; the call is a klaxon and a response queued. A
    /// player straight under it is never seen.
    fn stand(at: Vec2, cut_first: bool) -> (Vec<SpotterAct>, Entity, crate::vr_harness::Vr) {
        let mut vr = crate::vr_harness::Vr::new(&crate::LaunchOptions::default().map);
        vr.app.init_resource::<Acts>().add_systems(Update, collect);
        let zone = vr.app.world().resource::<Zone>().clone();
        let e = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_spotter(&mut c, &zone, Vec2::new(40.0, 230.0));
            vr.app.world_mut().flush();
            e
        };
        vr.stand(zone.on_ground(at.x, at.y), 0.0);
        if cut_first {
            vr.step(3);
            let low = tether(&mut vr.app, e, 0);
            vr.app
                .world_mut()
                .write_message(crate::destruction::hooks::Cut {
                    target: low,
                    by: vr.player,
                });
        }
        vr.step(90 * 50);
        let acts = vr.app.world().resource::<Acts>().0.clone();
        (acts, e, vr)
    }

    pub(crate) fn tether(app: &mut App, spotter: Entity, k: usize) -> Entity {
        app.world_mut()
            .query::<(Entity, &SpotterPiece, &OfSpotter)>()
            .iter(app.world())
            .find(|(_, p, o)| **p == SpotterPiece::Tether(k) && o.0 == spotter)
            .map(|(e, ..)| e)
            .expect("a tether")
    }

    #[test]
    fn it_sees_you_in_the_open_and_calls() {
        let (acts, _, vr) = stand(Vec2::new(40.0, 190.0), false);
        let fix = acts.iter().position(|a| *a == SpotterAct::Fixing);
        let call = acts.iter().position(|a| *a == SpotterAct::Call);
        assert!(fix.is_some() && call.is_some(), "{acts:?}");
        assert!(fix < call, "it built a fix before it called");
        assert_eq!(
            acts.iter().filter(|a| **a == SpotterAct::Call).count(),
            1,
            "once, then it waits"
        );
        let n = vr.app.world().resource::<crate::bot::Counters>().0[CALLS];
        assert_eq!(n, 1);
    }

    #[test]
    fn straight_under_it_you_are_unseen() {
        let r = rules::blind_radius(rules::TETHER);
        let (acts, _, _) = stand(Vec2::new(40.0 + r * 0.4, 230.0), false);
        assert!(!acts.contains(&SpotterAct::Call), "{acts:?}");
    }

    /// Cut its tether and it goes up and away, trailing its rope, and is
    /// gone; it calls no one.
    #[test]
    fn cut_loose_it_drifts_off() {
        let (acts, e, vr) = stand(Vec2::new(40.0, 190.0), true);
        assert!(acts.contains(&SpotterAct::Loose), "{acts:?}");
        assert!(!acts.contains(&SpotterAct::Call), "{acts:?}");
        assert!(acts.contains(&SpotterAct::Gone), "{acts:?}");
        assert!(vr.app.world().get_entity(e).is_err(), "gone");
    }

    /// Hole its envelope and it comes down where it was, a wreck with its
    /// salvage beside it.
    #[test]
    fn holed_it_sinks_to_a_wreck() {
        let mut vr = crate::vr_harness::Vr::new(&crate::LaunchOptions::default().map);
        vr.app.init_resource::<Acts>().add_systems(Update, collect);
        let zone = vr.app.world().resource::<Zone>().clone();
        let e = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_spotter(&mut c, &zone, Vec2::new(40.0, 230.0));
            vr.app.world_mut().flush();
            e
        };
        vr.stand(zone.on_ground(40.0, 236.0), 0.0);
        vr.step(5);
        let envelope = vr
            .app
            .world_mut()
            .query::<(Entity, &super::super::RobotPart)>()
            .iter(vr.app.world())
            .find(|(_, p)| p.robot == e && p.role == PartRole::Core)
            .map(|(c, _)| c)
            .unwrap();
        let at = vr
            .app
            .world()
            .get::<GlobalTransform>(envelope)
            .unwrap()
            .translation();
        let _ = at;
        crate::robots::heavy::tests::hit(&mut vr.app, envelope, ENVELOPE_HP + 10.0);
        vr.step(90 * 40);
        let acts = vr.app.world().resource::<Acts>().0.clone();
        assert!(acts.contains(&SpotterAct::Holed), "{acts:?}");
        assert!(acts.contains(&SpotterAct::Down), "{acts:?}");
        let t = *vr.app.world().get::<Transform>(e).expect("the wreck stays");
        let over = t.translation.y - zone.ground(t.translation.x, t.translation.z);
        assert!(over < 1.5, "on the ground: {over}");
        let catalog = vr.app.world().resource::<crate::items::Catalog>().clone();
        let near: Vec<String> = vr
            .app
            .world_mut()
            .query::<(&crate::items::Item, &GlobalTransform)>()
            .iter(vr.app.world())
            .filter(|(_, g)| g.translation().distance(t.translation) < 6.0)
            .map(|(i, _)| catalog.get(i.kind()).id.clone())
            .collect();
        assert!(near.iter().any(|s| s == "dirigible_gas_valve"), "{near:?}");
        assert!(near.iter().any(|s| s == "sensor_eye"), "{near:?}");
    }
}
