//! The sentry: the Custodian's guard post. A turret head on a mast,
//! standing over what matters (a Gate, the Depot's mouth, a key's
//! hiding place), sweeping its arc back and forth, a searchlight at night.
//! It's built to be outwitted, not fought (the rules are
//! `extraction_core::sentry`):
//!
//! - **Learn its sweep.** It turns end to end at a steady pace and pauses
//!   at each end; you can time a crossing.
//! - **Behind it is blind.** Its head can't turn past its arc: get round
//!   behind it and, even with you spotted, it stares at the end of its
//!   travel.
//! - **It warns.** A chirp (and the lamp flaring red) a second before the
//!   first round; duck out of sight a while and it has to warn again.
//! - **Draw it off.** A thrown thing landing within earshot swings it to
//!   look for five seconds; a hearth or a mirror shard flying past, it
//!   tracks.
//! - **Cut its power.** A cable runs along the ground from its foot to a
//!   junction box: trace it, cut it with bolt cutters, or wreck the box,
//!   and the head goes dark and sags.
//! - **An EMP** holds it still; **a heavy** lured across its arc takes the
//!   rounds meant for you.
//!
//! What it sees it reports (a response team follows). Shoot its head
//! through and it's a wreck, a landmark: the head hangs off its mast.
//!
//! Built from robot parts like any other machine: the head (core), its
//! eye and its gun ride the robot root, which sits at the top of the mast
//! and turns (`perceive` looks along it). The mast and the box are its
//! own static pieces beside it.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::sentry::{self as rules, Gun, GunAct};

use super::{Awareness, Brain, Job, JobKind, PartRole, Robot, RobotKind, Senses, part_bundle};
use crate::weapons::ballistics::Shot;
use crate::world::{Layer, Zone};

/// The head's height over the ground at its foot (m).
pub const MAST: f32 = 4.6;
/// The cable's segments: each a stretch of this length a cutters' jaws
/// find within [`CABLE_REACH`] of its middle.
pub const CABLE_STEP: f32 = 0.6;
pub const CABLE_REACH: f32 = 0.35;
/// What its parts take.
pub const HEAD_HP: f32 = 260.0;
pub const EYE_HP: f32 = 60.0;
pub const GUN_HP: f32 = 140.0;
pub const BOX_HP: f32 = 90.0;
/// Its rounds.
pub const DAMAGE: f32 = 16.0;
pub const SPREAD: f32 = 0.02;
/// It looks this far down from level as it sweeps (deg): at the ground
/// short of it, where people walk.
pub const SWEEP_PITCH: f32 = -9.0;
/// Hearth or mirror shard flying faster than this past it, it tracks.
pub const DECOY_SPEED: f32 = 2.5;
/// Thrown things it watches fly.
pub const DECOYS: [&str; 2] = ["art_hearth", "art_shard"];

/// A sentry's own state, beside the shared robot ones.
#[derive(Component, Debug, Clone)]
pub struct Sentry {
    /// The middle of its arc and its width (deg, the game's yaw).
    pub centre: f32,
    pub arc: f32,
    /// Where its head points now (deg).
    pub yaw: f32,
    pub pitch: f32,
    /// Seconds into its sweep (it pauses while it's looking at something).
    pub clock: f32,
    pub gun: Gun,
    /// Something it's turned to look at, and for how long yet.
    pub look: Option<(Vec3, f32)>,
    pub powered: bool,
    pub report: f32,
    /// Its foot (the ground at the mast).
    pub foot: Vec3,
}

/// One of a sentry's pieces that isn't on its head: the mast, the box, a
/// stretch of cable.
#[derive(Component, Debug, Clone, Copy)]
pub enum SentryPiece {
    Mast,
    Box,
    Cable,
}

/// Which sentry a piece belongs to.
#[derive(Component, Debug, Clone, Copy)]
pub struct OfSentry(pub Entity);

/// A sentry's gun (its muzzle is the part's middle, forward).
#[derive(Component, Debug)]
pub struct SentryGun;

/// Something happened at a sentry (for sound, light, the wrist).
#[derive(Message, Clone, Copy, Debug)]
pub struct SentryEvent {
    pub sentry: Entity,
    pub at: Vec3,
    pub what: SentryAct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SentryAct {
    /// The warning: it has you.
    Chirp,
    Fire,
    /// It swung to look at something (a decoy).
    Drawn,
    /// Its power went (the cable cut, the box wrecked).
    PowerLost,
    /// Its head was shot through.
    Wrecked,
}

pub struct SentryPlugin;

impl Plugin for SentryPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SentryEvent>()
            .add_systems(
                FixedUpdate,
                (sentry_power, sentry_mind, sentry_report)
                    .chain()
                    .after(super::perceive)
                    .before(PhysicsSystems::First),
            )
            .add_systems(FixedUpdate, stage_scenario);
    }
}

/// The head's pose for a yaw and pitch, on the mast at `foot`.
pub fn head_transform(foot: Vec3, yaw: f32, pitch: f32) -> Transform {
    Transform::from_translation(foot + Vec3::Y * MAST).with_rotation(Quat::from_euler(
        EulerRot::YXZ,
        -yaw.to_radians(),
        pitch.to_radians(),
        0.0,
    ))
}

/// A sentry standing at `at` (x/z, on the ground), its arc centred on
/// `centre` degrees and `arc` wide, its cable running to a junction box
/// at `power` (x/z).
pub fn spawn_sentry(
    commands: &mut Commands,
    zone: &Zone,
    at: Vec2,
    centre: f32,
    arc: f32,
    power: Vec2,
) -> Entity {
    let foot = zone.on_ground(at.x, at.y);
    let yaw = rules::sweep_yaw(0.0, centre, arc);
    let robot = commands
        .spawn((
            Name::new("Sentry"),
            Robot {
                kind: RobotKind::Pylon,
                wrecked: false,
            },
            super::Breed("sentry"),
            Sentry {
                centre,
                arc,
                yaw,
                pitch: SWEEP_PITCH,
                clock: 0.0,
                gun: Gun::default(),
                look: None,
                powered: true,
                report: 0.0,
                foot,
            },
            Senses {
                sight: rules::SIGHT,
                fov_deg: rules::FOV,
                hearing: 1.0,
                eye: Vec3::new(0.0, 0.05, -0.3),
            },
            Awareness::default(),
            Brain::Working {
                at: foot,
                left: f32::INFINITY,
            },
            Job {
                kind: JobKind::Watch,
                site: foot,
                duration: f32::INFINITY,
                exit: foot,
            },
            head_transform(foot, yaw, SWEEP_PITCH),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Sentry head"),
            part_bundle(
                robot,
                PartRole::Core,
                HEAD_HP,
                Collider::cuboid(0.5, 0.36, 0.6),
                Transform::IDENTITY,
            ),
        ));
        c.spawn((
            Name::new("Sentry eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                EYE_HP,
                Collider::cuboid(0.2, 0.12, 0.06),
                Transform::from_xyz(0.0, 0.05, -0.32),
            ),
        ));
        c.spawn((
            Name::new("Sentry gun"),
            SentryGun,
            part_bundle(
                robot,
                PartRole::Weapon,
                GUN_HP,
                Collider::cuboid(0.12, 0.12, 0.6),
                Transform::from_xyz(0.0, -0.22, -0.35),
            ),
        ));
    });
    // The mast: world, not robot (rounds stop on it like a post).
    commands.spawn((
        Name::new("Sentry mast"),
        SentryPiece::Mast,
        OfSentry(robot),
        Transform::from_translation(foot + Vec3::Y * (MAST * 0.5 - 0.1)),
        Visibility::default(),
        RigidBody::Static,
        Collider::cylinder(0.16, MAST - 0.2),
        CollisionLayers::new(Layer::World, [Layer::Player, Layer::Item, Layer::Robot]),
    ));
    // The junction box, and the cable to it.
    let box_at = zone.on_ground(power.x, power.y);
    commands.spawn((
        Name::new("Sentry junction box"),
        SentryPiece::Box,
        OfSentry(robot),
        part_bundle(
            robot,
            PartRole::Armour(40),
            BOX_HP,
            Collider::cuboid(0.5, 0.8, 0.35),
            Transform::from_translation(box_at + Vec3::Y * 0.4).with_rotation(
                Quat::from_rotation_y((foot - box_at).x.atan2((foot - box_at).z)),
            ),
        ),
    ));
    for (mid, len, rot) in cable_run(zone, foot, box_at) {
        commands.spawn((
            Name::new("Sentry cable"),
            SentryPiece::Cable,
            OfSentry(robot),
            crate::destruction::hooks::Cuttable { needs: 0.0 },
            // What the cutters' jaws find: a thin overlap-only box along
            // the stretch, which nothing collides with or shoots.
            Sensor,
            Collider::cuboid(0.08, 0.06, len),
            CollisionLayers::new(Layer::Hand, LayerMask::NONE),
            Transform::from_translation(mid).with_rotation(rot),
            CableLength(len),
            Visibility::default(),
        ));
    }
    robot
}

/// A stretch of cable's length (for its look).
#[derive(Component, Debug, Clone, Copy)]
pub struct CableLength(pub f32);

/// The cable from the mast's foot to the box: stretches of about
/// [`CABLE_STEP`] lying on the ground, each turned along its run.
pub fn cable_run(zone: &Zone, from: Vec3, to: Vec3) -> Vec<(Vec3, f32, Quat)> {
    let flat = Vec2::new(to.x - from.x, to.z - from.z);
    let n = ((flat.length() / CABLE_STEP).ceil() as usize).max(1);
    (0..n)
        .map(|i| {
            let a = Vec2::new(from.x, from.z) + flat * (i as f32 / n as f32);
            let b = Vec2::new(from.x, from.z) + flat * ((i + 1) as f32 / n as f32);
            let pa = zone.on_ground(a.x, a.y) + Vec3::Y * 0.03;
            let pb = zone.on_ground(b.x, b.y) + Vec3::Y * 0.03;
            let d = pb - pa;
            (
                (pa + pb) * 0.5,
                d.length(),
                Quat::from_rotation_arc(Vec3::Z, d.normalize_or(Vec3::Z)),
            )
        })
        .collect()
}

/// The cable cut, the box wrecked, the recall: no
/// power, and a sentry without power is a dark head sagging on its mast.
#[allow(clippy::too_many_arguments)]
fn sentry_power(
    mut commands: Commands,
    mut cuts: MessageReader<crate::destruction::hooks::Cut>,
    mut destroyed: MessageReader<super::PartDestroyed>,
    pieces: Query<(&SentryPiece, &OfSentry)>,
    mut sentries: Query<(Entity, &mut Sentry, &Robot, &Transform)>,
    raid: Option<Res<crate::raid::Raid>>,
    orphans: Query<(Entity, &OfSentry)>,
    mut out: MessageWriter<SentryEvent>,
) {
    // Pieces whose sentry is gone go with it.
    for (e, of) in &orphans {
        if !sentries.contains(of.0) {
            commands.entity(e).despawn();
        }
    }
    let mut lost: Vec<Entity> = Vec::new();
    for c in cuts.read() {
        if let Ok((SentryPiece::Cable, of)) = pieces.get(c.target) {
            lost.push(of.0);
            // Cut through: it's two loose ends now, not a cable.
            commands
                .entity(c.target)
                .remove::<(crate::destruction::hooks::Cuttable, Collider)>()
                .insert(CableCut);
        }
    }
    for d in destroyed.read() {
        if let Ok((SentryPiece::Box, of)) = pieces.get(d.part) {
            lost.push(of.0);
        }
        if d.role == PartRole::Core && sentries.contains(d.robot) {
            out.write(SentryEvent {
                sentry: d.robot,
                at: d.at,
                what: SentryAct::Wrecked,
            });
        }
    }
    // After the recall the depots cut them off.
    let recalled = raid.is_some_and(|r| r.recalled);
    for (e, mut s, robot, t) in &mut sentries {
        if !s.powered || robot.wrecked {
            continue;
        }
        if lost.contains(&e) || recalled {
            s.powered = false;
            out.write(SentryEvent {
                sentry: e,
                at: t.translation,
                what: SentryAct::PowerLost,
            });
            info!("sentry: power lost at {:.0}", t.translation);
        }
    }
}

/// A stretch of cable that's been cut.
#[derive(Component, Debug)]
pub struct CableCut;

/// Sweep, look, track, warn, fire.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sentry_mind(
    mut sentries: Query<(
        Entity,
        &mut Sentry,
        &mut Transform,
        &Awareness,
        &Robot,
        &Children,
        Has<super::Stunned>,
    )>,
    guns: Query<(&GlobalTransform, &super::RobotPart), With<SentryGun>>,
    targets: Query<&GlobalTransform>,
    mut noises: MessageReader<super::Noise>,
    flying: Query<(&crate::items::Item, &GlobalTransform, &LinearVelocity)>,
    catalog: Res<crate::items::Catalog>,
    mut shots: MessageWriter<Shot>,
    mut out: MessageWriter<SentryEvent>,
    time: Res<Time>,
    mut seed: Local<u64>,
) {
    let dt = time.delta_secs();
    let heard: Vec<(Vec3, f32, Option<Entity>)> =
        noises.read().map(|n| (n.at, n.radius, n.source)).collect();
    let decoys: Vec<Vec3> = flying
        .iter()
        .filter(|(it, _, v)| {
            v.0.length() > DECOY_SPEED && DECOYS.contains(&catalog.get(it.kind()).id.as_str())
        })
        .map(|(_, g, _)| g.translation())
        .collect();
    for (e, mut s, mut t, aware, robot, children, stunned) in &mut sentries {
        if robot.wrecked {
            // Its head hangs off the mast, still.
            let hang = head_transform(s.foot, s.yaw, -55.0);
            t.rotation = t.rotation.slerp(hang.rotation, (2.0 * dt).min(1.0));
            t.translation = t
                .translation
                .lerp(hang.translation - Vec3::Y * 0.25, (2.0 * dt).min(1.0));
            continue;
        }
        if !s.powered || stunned {
            // Dark, sagging; no eyes, no gun.
            s.gun = Gun::default();
            let droop = if s.powered { -30.0 } else { -40.0 };
            s.pitch += (droop - s.pitch) * (1.5 * dt).min(1.0);
            *t = head_transform(s.foot, s.yaw, s.pitch);
            continue;
        }
        let head = t.translation;
        let flat = |p: Vec3| [p.x, p.z];
        let bearing = |p: Vec3| rules::bearing(flat(head), flat(p));
        // What it wants to look at, most pressing first.
        let seen = aware
            .target
            .filter(|_| aware.can_see_target && aware.suspicion >= 0.5)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation());
        let mut want: Option<Vec3> = None;
        if let Some(p) = seen {
            want = Some(p + Vec3::Y * 1.2);
        } else {
            // A hearth or shard flying where it could see.
            if let Some(d) = decoys.iter().find(|d| {
                d.distance(head) < rules::DECOY_RANGE
                    && rules::can_ever_see(bearing(**d), s.centre, s.arc)
            }) {
                if s.look.is_none() {
                    out.write(SentryEvent {
                        sentry: e,
                        at: *d,
                        what: SentryAct::Drawn,
                    });
                }
                s.look = Some((*d, rules::DECOY_HOLD));
            }
            // A noise near enough to hear and somewhere it could look.
            for (at, radius, source) in &heard {
                if *source == Some(e) {
                    continue;
                }
                let d = at.distance(head);
                if d < rules::DECOY_RANGE.min(*radius * 1.5)
                    && rules::can_ever_see(bearing(*at), s.centre, s.arc)
                {
                    if s.look.is_none() {
                        out.write(SentryEvent {
                            sentry: e,
                            at: *at,
                            what: SentryAct::Drawn,
                        });
                    }
                    s.look = Some((*at, rules::DECOY_HOLD));
                }
            }
            if let Some((at, left)) = s.look.as_mut() {
                *left -= dt;
                want = Some(*at);
                if *left <= 0.0 {
                    s.look = None;
                }
            } else if aware.suspicion > 0.3 {
                want = aware.last_known;
            }
        }
        match want {
            Some(p) => {
                s.yaw = rules::slew(s.yaw, bearing(p), s.centre, s.arc, dt);
                let d = p - head;
                let pitch = d.y.atan2(Vec2::new(d.x, d.z).length()).to_degrees();
                s.pitch += (pitch.clamp(-45.0, 20.0) - s.pitch) * (4.0 * dt).min(1.0);
            }
            None => {
                // Back on its sweep (it held its place while it looked).
                s.clock += dt;
                let sweep = rules::sweep_yaw(s.clock, s.centre, s.arc);
                s.yaw = rules::slew(s.yaw, sweep, s.centre, s.arc, dt);
                s.pitch += (SWEEP_PITCH - s.pitch) * (2.0 * dt).min(1.0);
            }
        }
        *t = head_transform(s.foot, s.yaw, s.pitch);
        // The gun: in view and in its arc, a warning, then bursts.
        let in_view = seen.is_some_and(|p| rules::in_view(bearing(p), s.yaw));
        let gun = children.iter().find_map(|c| guns.get(c).ok());
        let armed = gun.is_some_and(|(_, part)| part.alive());
        match s.gun.step(in_view && armed, dt) {
            GunAct::Chirp => {
                out.write(SentryEvent {
                    sentry: e,
                    at: head,
                    what: SentryAct::Chirp,
                });
            }
            GunAct::Fire => {
                let (Some((ggt, _)), Some(p)) = (gun, seen) else {
                    continue;
                };
                let muzzle = ggt.transform_point(Vec3::new(0.0, 0.0, -0.32));
                let aim = p + Vec3::Y * 1.1;
                *seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                shots.write(Shot {
                    gun: e,
                    shooter: Some(e),
                    origin: muzzle,
                    direction: (aim - muzzle).normalize_or(Vec3::NEG_Z),
                    speed: 600.0,
                    damage: DAMAGE,
                    projectiles: 1,
                    spread: SPREAD,
                    recoil: 0.0,
                    seed: *seed,
                });
                out.write(SentryEvent {
                    sentry: e,
                    at: muzzle,
                    what: SentryAct::Fire,
                });
            }
            GunAct::Quiet => {}
        }
    }
}

/// What it sees it reports: a response team follows, now and then.
fn sentry_report(
    mut sentries: Query<(&mut Sentry, &Awareness, &Robot)>,
    director: Option<ResMut<super::director::Director>>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    let Some(mut director) = director else {
        return;
    };
    let dt = time.delta_secs();
    for (mut s, aware, robot) in &mut sentries {
        s.report -= dt;
        if robot.wrecked
            || !s.powered
            || !aware.can_see_target
            || aware.suspicion < 0.6
            || s.report > 0.0
        {
            continue;
        }
        if let Some(at) = aware.target.and_then(|t| targets.get(t).ok()) {
            let delay = director.rng.range(20.0, 40.0);
            director.queue_response(delay, at.translation());
            s.report = rules::REPORT_EVERY;
            info!("sentry: reported someone at {:.0}", at.translation());
        }
    }
}

/// The `sentry_cable` scenario (`bot::scenarios`): a sentry ahead of the
/// bot, looking away from it, its cable running back toward the bot
/// through its blind sector to a junction box; bolt cutters at the bot's
/// feet. Laid out along whichever way from the bot is open ground.
pub const SCENARIO: &str = "sentry_cable";
/// How far ahead the sentry stands, and where it's walked to after.
pub const STAGE_AHEAD: f32 = 18.0;
pub const STAGE_PAST: f32 = 14.0;
/// The scenario's walk past the sentry, before the setup knows where:
/// `stage_scenario` fills it in on the bot's task list.
pub const PAST_PLACEHOLDER: Vec2 = Vec2::new(-1.0e6, -1.0e6);

/// Where the scenario's sentry stands for a bot at `at` facing `dir`, and
/// its junction box.
pub fn stage_layout(at: Vec3, dir: Vec3) -> (Vec2, Vec2) {
    let side = Vec3::new(-dir.z, 0.0, dir.x);
    let foot = at + dir * STAGE_AHEAD;
    let power = at + dir * 7.0 + side * 6.0;
    (Vec2::new(foot.x, foot.z), Vec2::new(power.x, power.z))
}

fn stage_scenario(
    mut commands: Commands,
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    zone: Res<Zone>,
    nav: Res<super::walker::Nav>,
    catalog: Res<crate::items::Catalog>,
    mut bots: Query<&mut crate::bot::Bot>,
    anomalies: Query<(&crate::anomaly::Anomaly, &GlobalTransform)>,
) {
    // Clear of anomalies (a burner by the junction box is no test of a
    // sentry).
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
            let side = Vec3::new(-d.z, 0.0, d.x);
            (2..=((STAGE_AHEAD + STAGE_PAST) as usize + 2))
                .step_by(2)
                .all(|m| {
                    let p = s.at + d * m as f32;
                    nav.0.walkable([p.x, p.z]) && clear(p)
                })
                && (0..=6).all(|m| {
                    let p = s.at + d * 7.0 + side * m as f32;
                    nav.0.walkable([p.x, p.z]) && clear(p)
                })
        };
        let dir = (0..16)
            .map(|k| Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * f)
            .find(|d| open(*d))
            .unwrap_or(f);
        let (foot, power) = stage_layout(s.at, dir);
        // Its arc faces away from the bot: the bot starts in its blind back.
        let centre = rules::bearing([s.at.x, s.at.z], [foot.x, foot.y]);
        info!("scenario: a sentry at {foot:.0}, its box at {power:.0}");
        let past = foot + Vec2::new(dir.x, dir.z) * STAGE_PAST;
        for mut bot in &mut bots {
            for t in bot.tasks.iter_mut() {
                if let crate::bot::Task::WalkTo { to, .. } = t {
                    if *to == PAST_PLACEHOLDER {
                        *to = past;
                    }
                }
            }
        }
        spawn_sentry(&mut commands, &zone, foot, centre, 200.0, power);
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

    pub(crate) fn world_with_sentry(at: [f32; 2], centre: f32, arc: f32) -> (App, Entity) {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let e = {
            let mut c = app.world_mut().commands();
            let e = spawn_sentry(
                &mut c,
                &zone,
                Vec2::from_array(at),
                centre,
                arc,
                Vec2::new(at[0] + 8.0, at[1] + 10.0),
            );
            app.world_mut().flush();
            e
        };
        (app, e)
    }

    /// Unwatched, it sweeps its arc in its rhythm, its head on the mast,
    /// never turning into its blind sector.
    #[test]
    fn it_sweeps_and_never_looks_behind() {
        let (mut app, e) = world_with_sentry([40.0, 230.0], 0.0, 200.0);
        let mut seen = (f32::MAX, f32::MIN);
        for _ in 0..(90 * 30) {
            app.update();
            let s = app.world().get::<Sentry>(e).unwrap();
            seen = (seen.0.min(s.yaw), seen.1.max(s.yaw));
            let fwd = app.world().get::<GlobalTransform>(e).unwrap().forward();
            let bearing = (fwd.x).atan2(-fwd.z).to_degrees();
            assert!(
                (rules::wrap_deg(bearing - s.yaw)).abs() < 1.0,
                "head faces its yaw"
            );
        }
        assert!(seen.0 < -95.0 && seen.1 > 95.0, "covered its arc: {seen:?}");
        let s = app.world().get::<Sentry>(e).unwrap();
        let t = app.world().get::<Transform>(e).unwrap();
        assert!(
            (t.translation.y - s.foot.y - MAST).abs() < 0.01,
            "on its mast"
        );
    }

    /// A player in front of it, in the open, in its arc: when its sweep
    /// comes round it chirps, and only then, a second later, fires.
    /// The same player behind it is never seen, never shot.
    fn stand_and_watch(behind: bool, cut: bool) -> (bool, bool, f32) {
        let mut vr = crate::vr_harness::Vr::new(&crate::LaunchOptions::default().map);
        vr.app
            .init_resource::<Acts>()
            .init_resource::<Hurts>()
            .add_systems(Update, collect);
        let zone = vr.app.world().resource::<Zone>().clone();
        let at = Vec2::new(40.0, 230.0);
        // Its arc faces -Z (toward z 215); behind it is +Z.
        let e = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_sentry(&mut c, &zone, at, 0.0, 160.0, Vec2::new(48.0, 236.0));
            vr.app.world_mut().flush();
            e
        };
        let me = if behind {
            zone.on_ground(40.0, 242.0)
        } else {
            zone.on_ground(40.0, 216.0)
        };
        vr.stand(me, 0.0);
        if cut {
            vr.step(5);
            let cable = vr
                .app
                .world_mut()
                .query::<(Entity, &SentryPiece, &OfSentry)>()
                .iter(vr.app.world())
                .find(|(_, p, o)| matches!(p, SentryPiece::Cable) && o.0 == e)
                .map(|(c, ..)| c)
                .unwrap();
            vr.app
                .world_mut()
                .write_message(crate::destruction::hooks::Cut {
                    target: cable,
                    by: vr.player,
                });
        }
        vr.step(90 * 25);
        let acts = &vr.app.world().resource::<Acts>().0;
        let chirp = acts.iter().position(|a| *a == SentryAct::Chirp);
        let fire = acts.iter().position(|a| *a == SentryAct::Fire);
        if let (Some(c), Some(f)) = (chirp, fire) {
            assert!(c < f, "chirped before it fired");
        }
        let hurt = vr.app.world().resource::<Hurts>().0;
        (chirp.is_some(), fire.is_some(), hurt)
    }

    #[derive(Resource, Default)]
    struct Acts(Vec<SentryAct>);

    fn collect(
        mut ev: MessageReader<SentryEvent>,
        mut acts: ResMut<Acts>,
        mut hurt: MessageReader<crate::health::Hurt>,
        mut hurts: ResMut<Hurts>,
    ) {
        acts.0.extend(ev.read().map(|e| e.what));
        hurts.0 += hurt.read().map(|h| h.amount).sum::<f32>();
    }

    /// Harm done to the player (it can go down and be reset: count it as
    /// it lands).
    #[derive(Resource, Default)]
    struct Hurts(f32);

    #[test]
    fn in_its_arc_it_warns_then_fires() {
        let (chirp, fire, hurt) = stand_and_watch(false, false);
        assert!(chirp && fire, "chirp {chirp} fire {fire}");
        assert!(hurt > 0.0, "and hit");
    }

    #[test]
    fn behind_it_you_are_never_seen() {
        let (chirp, fire, hurt) = stand_and_watch(true, false);
        assert!(!chirp && !fire && hurt == 0.0, "{chirp} {fire} {hurt}");
    }

    #[test]
    fn cut_its_cable_and_its_dark() {
        let (chirp, fire, hurt) = stand_and_watch(false, true);
        assert!(!chirp && !fire && hurt == 0.0, "{chirp} {fire} {hurt}");
    }

    /// A noise where it could look swings it there, and it holds there a
    /// while before going back to its sweep.
    #[test]
    fn a_thrown_noise_draws_it() {
        let (mut app, e) = world_with_sentry([40.0, 230.0], 0.0, 200.0);
        for _ in 0..30 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let thud = zone.on_ground(55.0, 220.0);
        let foot = app.world().get::<Sentry>(e).unwrap().foot;
        let toward = rules::bearing([foot.x, foot.z], [thud.x, thud.z]);
        app.world_mut().write_message(super::super::Noise {
            at: thud,
            radius: 25.0,
            source: None,
        });
        for _ in 0..(90 * 3) {
            app.update();
        }
        let yaw = app.world().get::<Sentry>(e).unwrap().yaw;
        assert!(
            rules::wrap_deg(yaw - toward).abs() < 3.0,
            "looking at it: {yaw} vs {toward}"
        );
        for _ in 0..90 {
            app.update();
        }
        let still = app.world().get::<Sentry>(e).unwrap().yaw;
        assert!(rules::wrap_deg(still - toward).abs() < 3.0, "holds there");
        for _ in 0..(90 * 4) {
            app.update();
        }
        assert!(
            app.world().get::<Sentry>(e).unwrap().look.is_none(),
            "and goes back"
        );
    }
}
