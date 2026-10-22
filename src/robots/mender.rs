//! The Custodian's mender: a small quadcopter with a welding arm, sent out
//! to patch up wounded machines. It carries no gun. (Its rules are
//! `extraction_core::mender`; it flies as a drone does, `flight`.)
//!
//! - **The tell.** Welding, its torch's arc flares blue-white and
//!   crackles: you'll see the flicker a valley off, and know that machine
//!   you hurt is being made whole.
//! - **The weakness.** It hangs still beside the part it welds, low, its
//!   back to you: an easy shot. Its welding arm is a part of its own:
//!   shoot that off and it can't mend anything.
//! - **The salvage.** It comes down as a drone does (its rotors and cell);
//!   and a mender's kit of spare parts.

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::mender as rules;

use super::drone::Drone;
use super::{Awareness, PartRole, Robot, RobotKind, RobotPart, part_bundle};
use crate::world::Zone;

pub const CORE_HP: f32 = 45.0;
pub const EYE_HP: f32 = 12.0;
pub const ARM_HP: f32 = 20.0;
pub const ROTOR_HP: f32 = 12.0;
/// Its rotors (the model's), its eye, and its arm's shoulder and torch tip
/// (local).
pub const ROTORS: [Vec3; 4] = [
    Vec3::new(0.42, 0.06, 0.42),
    Vec3::new(-0.42, 0.06, 0.42),
    Vec3::new(-0.42, 0.06, -0.42),
    Vec3::new(0.42, 0.06, -0.42),
];
pub const EYE: Vec3 = Vec3::new(0.0, 0.0, -0.27);
pub const ARM: Vec3 = Vec3::new(0.0, -0.12, -0.15);
pub const TORCH: Vec3 = Vec3::new(0.0, -0.38, -0.45);
/// The lowest it can hang over the ground (`flight` holds drones off it),
/// and how far its arc reaches from there, m.
pub const HOVER_MIN: f32 = 2.7;
pub const ARC_REACH: f32 = 3.6;
/// How far out from the part it hangs (clear of the machine's body).
pub const HANG_OUT: f32 = 2.2;
/// Seconds with nothing to mend before it goes home.
pub const IDLE_HOME: f32 = 45.0;
/// Counters a scenario reads: hit points welded back (whole ones).
pub const WELDED: &str = "mender_welded_hp";

/// A mender.
#[derive(Component, Debug)]
pub struct Mender {
    /// What it's mending: the machine and the part.
    pub patient: Option<(Entity, Entity)>,
    /// Where its torch is on a weld now (for its look and sound).
    pub welding: Option<Vec3>,
    /// Seconds before it looks again for who needs it most.
    pub rethink: f32,
    /// Seconds with nothing to do.
    pub idle: f32,
    /// Where it goes home to.
    pub home: Vec3,
    /// The side of its patient it welds from (x/z, set when it picks
    /// one: the way it came).
    pub side: Vec2,
    /// Weld not yet counted whole.
    pub owed: f32,
    pub looted: bool,
}

/// Its welding arm (a part).
#[derive(Component, Debug)]
pub struct WeldArm;

/// What a mender did, for sound and light.
#[derive(Message, Debug, Clone, Copy)]
pub struct MenderEvent {
    pub mender: Entity,
    pub at: Vec3,
    pub what: MenderAct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenderAct {
    /// It's picked a patient and set off.
    Called,
    /// The arc struck: welding.
    Strike,
    /// A part's whole again.
    Mended,
}

pub struct MenderPlugin;

impl Plugin for MenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MenderEvent>()
            .init_resource::<crate::bot::Counters>()
            .init_resource::<MendersSent>()
            .add_systems(
                FixedUpdate,
                (mender_mind, mender_salvage)
                    .chain()
                    .before(super::flight::fly_drones)
                    .before(PhysicsSystems::First),
            )
            .add_systems(FixedUpdate, (send_mender, stage_scenario));
    }
}

/// A mender at `at` (in the air), home at `home`.
pub fn spawn_mender(commands: &mut Commands, at: Vec3, home: Vec3) -> Entity {
    let robot = commands
        .spawn((
            Name::new("Mender"),
            Robot {
                kind: RobotKind::Drone,
                wrecked: false,
            },
            super::Breed("mender"),
            super::heavy::OwnLook,
            super::heavy::NotOnPatrol,
            Drone {
                thrust: [0.0; 4],
                rotor_alive: [true; 4],
                gun_cooldown: f32::INFINITY,
                burst_left: 0,
                grounded: 0.0,
                goal: at,
                phase: (at.x * 0.37 + at.z * 0.11).fract() * 6.28,
                looted: false,
                lock: 0.0,
            },
            Mender {
                patient: None,
                welding: None,
                side: Vec2::X,
                rethink: 0.0,
                idle: 0.0,
                home,
                owed: 0.0,
                looted: false,
            },
            Awareness::default(),
            Transform::from_translation(at),
            Visibility::default(),
            (
                RigidBody::Dynamic,
                Mass(super::drone::MASS),
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
            Name::new("Mender body"),
            part_bundle(
                robot,
                PartRole::Core,
                CORE_HP,
                Collider::cuboid(0.55, 0.22, 0.55),
                Transform::IDENTITY,
            ),
        ));
        c.spawn((
            Name::new("Mender eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                EYE_HP,
                Collider::sphere(0.06),
                Transform::from_translation(EYE),
            ),
        ));
        c.spawn((
            Name::new("Mender arm"),
            WeldArm,
            part_bundle(
                robot,
                PartRole::Weapon,
                ARM_HP,
                Collider::cuboid(0.08, 0.3, 0.3),
                Transform::from_translation(ARM + Vec3::new(0.0, -0.13, -0.15)),
            ),
        ));
        for (i, p) in ROTORS.iter().enumerate() {
            c.spawn((
                Name::new(format!("Mender rotor {i}")),
                part_bundle(
                    robot,
                    PartRole::Rotor(i as u8),
                    ROTOR_HP,
                    Collider::cylinder(0.16, 0.04),
                    Transform::from_translation(*p),
                ),
            ));
        }
    });
    robot
}

/// Find the worst hurt, fly to it, weld.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn mender_mind(
    mut commands: Commands,
    mut menders: Query<(
        Entity,
        &mut Mender,
        &mut Drone,
        &Transform,
        &Robot,
        &Children,
        Has<super::Stunned>,
    )>,
    robots: Query<(Entity, &Robot, &GlobalTransform), Without<Mender>>,
    mut parts: Query<(Entity, &mut RobotPart, &GlobalTransform)>,
    arms: Query<(), With<WeldArm>>,
    zone: Res<Zone>,
    mut out: MessageWriter<MenderEvent>,
    mut counters: ResMut<crate::bot::Counters>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if menders.is_empty() {
        return;
    }
    // Every machine's parts that are still there.
    let mut by_robot: HashMap<Entity, Vec<(Entity, f32, f32, Vec3)>> = HashMap::new();
    for (e, p, g) in &parts {
        by_robot
            .entry(p.robot)
            .or_default()
            .push((e, p.hp, p.max_hp, g.translation()));
    }
    for (e, mut m, mut d, t, robot, children, stunned) in &mut menders {
        let was_welding = m.welding.take().is_some();
        if robot.wrecked || stunned {
            continue;
        }
        let here = t.translation;
        // Its arm shot off: nothing to weld with. Home.
        let armed = children
            .iter()
            .any(|c| arms.contains(c) && parts.get(c).is_ok_and(|(_, p, _)| p.alive()));
        m.rethink -= dt;
        if armed && (m.rethink <= 0.0 || m.patient.is_none()) {
            m.rethink = 1.0;
            let best = robots
                .iter()
                .filter(|(r, robot, _)| *r != e && !robot.wrecked)
                .filter_map(|(r, _, g)| {
                    let ps = by_robot.get(&r)?;
                    let hp: Vec<(f32, f32)> = ps.iter().map(|p| (p.1, p.2)).collect();
                    let n = rules::need(&hp, g.translation().distance(here));
                    (n > 0.0).then_some((r, n))
                })
                .max_by(|a, b| a.1.total_cmp(&b.1));
            let was = m.patient.map(|p| p.0);
            m.patient = best.and_then(|(r, _)| {
                // Its worst part that's still there.
                let ps = by_robot.get(&r)?;
                ps.iter()
                    .filter(|p| p.1 > 0.0 && p.1 < p.2 * rules::WORTH)
                    .min_by(|a, b| (a.1 / a.2).total_cmp(&(b.1 / b.2)))
                    .map(|p| (r, p.0))
            });
            if let Some((r, _)) = m.patient.filter(|p| Some(p.0) != was) {
                if let Ok((_, _, g)) = robots.get(r) {
                    let to = g.translation();
                    m.side = Vec2::new(here.x - to.x, here.z - to.z).normalize_or(Vec2::X);
                }
            }
            if m.patient.is_some() && m.patient.map(|p| p.0) != was {
                out.write(MenderEvent {
                    mender: e,
                    at: here,
                    what: MenderAct::Called,
                });
            }
        }
        if !armed {
            m.patient = None;
        }
        let Some((_, part)) = m.patient else {
            // Nothing to do: hold here a while, then home.
            m.idle += dt;
            let ground = zone.ground(here.x, here.z);
            d.goal = if m.idle > IDLE_HOME {
                m.home
            } else {
                Vec3::new(here.x, ground + rules::CRUISE, here.z)
            };
            if m.idle > IDLE_HOME && here.distance(m.home) < 3.0 {
                commands.entity(e).despawn();
            }
            continue;
        };
        m.idle = 0.0;
        let Ok((_, mut p, pg)) = parts.get_mut(part) else {
            m.patient = None;
            continue;
        };
        let at = pg.translation();
        if !p.alive() || p.hp >= p.max_hp {
            m.patient = None;
            m.rethink = 0.0;
            continue;
        }
        // Over it at its height, then down beside the part, on the side it
        // came from.
        let flat = Vec2::new(here.x - at.x, here.z - at.z);
        let side = m.side;
        let ground = zone.ground(here.x, here.z);
        // (It can't hover lower than its flight lets it: `flight` keeps
        // it off the ground. Its arc reaches down.)
        let mut beside = at + Vec3::new(side.x, 0.0, side.y) * HANG_OUT + Vec3::Y * 0.5;
        beside.y = beside.y.max(zone.ground(beside.x, beside.z) + HOVER_MIN);
        let over = at + Vec3::new(side.x, 0.0, side.y) * 6.0;
        d.goal = if flat.length() > 10.0 {
            Vec3::new(over.x, (ground + rules::CRUISE).max(at.y + 3.0), over.z)
        } else {
            beside
        };
        if here.distance(beside) < 1.2 && here.distance(at) < ARC_REACH {
            let before = p.hp;
            p.hp = rules::weld(p.hp, p.max_hp, dt);
            m.owed += p.hp - before;
            while m.owed >= 1.0 {
                m.owed -= 1.0;
                counters.bump(WELDED);
            }
            m.welding = Some(at);
            if !was_welding {
                out.write(MenderEvent {
                    mender: e,
                    at,
                    what: MenderAct::Strike,
                });
            }
            if p.hp >= p.max_hp {
                out.write(MenderEvent {
                    mender: e,
                    at,
                    what: MenderAct::Mended,
                });
                m.rethink = 0.0;
            }
        }
    }
}

/// Down: a mender's kit of spare parts by the wreck (the drone's own
/// salvage comes as a drone's does).
fn mender_salvage(
    mut commands: Commands,
    mut menders: Query<(&mut Mender, &Robot, &GlobalTransform)>,
    catalog: Res<crate::items::Catalog>,
) {
    for (mut m, robot, g) in &mut menders {
        if !robot.wrecked || m.looted {
            continue;
        }
        m.looted = true;
        let Some(kind) = catalog.kind("spare_parts") else {
            continue;
        };
        crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_translation(g.translation() + Vec3::Y * 0.3),
                condition: 0.8,
            },
        );
    }
}

/// How badly the valley's machines must be hurt (hit points missing, all
/// told) before a mender sets out; and how many a raid.
pub const SEND_AT: f32 = 120.0;
pub const PER_RAID: u32 = 2;

/// With robots on: when the valley's machines are hurt enough and no
/// mender is out, one sets out from the gate.
#[allow(clippy::too_many_arguments)]
fn send_mender(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    director: Option<Res<super::director::Director>>,
    raid: Option<Res<crate::raid::Raid>>,
    menders: Query<(), With<Mender>>,
    robots: Query<&Robot>,
    parts: Query<&RobotPart>,
    mut sent: ResMut<MendersSent>,
    mut wait: Local<f32>,
    time: Res<Time>,
) {
    let (Some(zone), Some(director)) = (zone, director) else {
        return;
    };
    if !director.enabled || raid.as_ref().is_some_and(|r| r.recalled) {
        return;
    }
    *wait -= time.delta_secs();
    if *wait > 0.0 || sent.0 >= PER_RAID || !menders.is_empty() {
        return;
    }
    *wait = 5.0;
    let missing: f32 = parts
        .iter()
        .filter(|p| p.alive() && robots.get(p.robot).is_ok_and(|r| !r.wrecked))
        .map(|p| p.max_hp - p.hp)
        .sum();
    if missing < SEND_AT {
        return;
    }
    request_mender(&mut commands, &zone, &mut sent, false, "the valley's hurt");
}

/// Menders sent this raid (at most [`PER_RAID`], whoever sends them).
#[derive(Resource, Debug, Default)]
pub struct MendersSent(pub u32);

/// A mender out from the gate, if the raid's budget allows and none is
/// out (`out`): for anything that wants one sent (the hurt valley, a cut
/// relay). Its patient it picks itself, the worst hurt near at hand.
pub fn request_mender(
    commands: &mut Commands,
    zone: &Zone,
    sent: &mut MendersSent,
    out: bool,
    why: &str,
) -> Option<Entity> {
    if out || sent.0 >= PER_RAID {
        return None;
    }
    sent.0 += 1;
    let gate = super::director::gate(zone);
    info!("mender: out from the gate ({why})");
    Some(spawn_mender(
        commands,
        gate + Vec3::Y * rules::CRUISE,
        gate + Vec3::Y * 6.0,
    ))
}

/// The scenario: a damaged pylon ahead and a mender coming to weld it.
/// The bot shoots the mender off it while it's welding.
pub const SCENARIO: &str = "mender_weld";
/// The same, left to it (run by name): the pylon comes back whole.
pub const CONTROL: &str = "mender_control";

#[allow(clippy::too_many_arguments)]
fn stage_scenario(
    mut commands: Commands,
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    zone: Res<Zone>,
    nav: Res<super::walker::Nav>,
    catalog: Res<crate::items::Catalog>,
    mut bots: Query<&mut crate::bot::Bot>,
) {
    for s in setups.read() {
        if s.name != SCENARIO {
            continue;
        }
        let f = Vec3::new(s.facing.x, 0.0, s.facing.z).normalize_or(Vec3::NEG_Z);
        let open = |d: Vec3| {
            (2..=20).step_by(2).all(|m| {
                let p = s.at + d * m as f32;
                nav.0.walkable([p.x, p.z])
            })
        };
        let dir = (0..16)
            .map(|k| Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * f)
            .find(|d| open(*d))
            .unwrap_or(f);
        let site = zone.on_ground(s.at.x + dir.x * 14.0, s.at.z + dir.z * 14.0);
        // A relay pylon, shot about.
        let pylon = super::pylon::spawn_pylon(&mut commands, site, 9999.0, site);
        commands.queue(move |w: &mut World| {
            let mut q = w.query::<&mut RobotPart>();
            for mut p in q.iter_mut(w) {
                if p.robot == pylon {
                    p.hp = p.max_hp * 0.3;
                }
            }
        });
        // From the side: it welds on the side it came from, in the bot's
        // clear view (not behind the pylon).
        let side = Vec3::new(-dir.z, 0.0, dir.x);
        let from = site + side * 60.0 - dir * 6.0 + Vec3::Y * rules::CRUISE;
        let mender = spawn_mender(&mut commands, from, from);
        info!("scenario: a hurt pylon at {site:.0}, a mender on its way");
        for mut bot in &mut bots {
            for t in bot.tasks.iter_mut() {
                if let crate::bot::Task::Shoot { target, .. } = t {
                    if *target == Entity::PLACEHOLDER {
                        *target = mender;
                    }
                }
            }
        }
        for (k, id) in ["ak74", "ak_mag"].into_iter().enumerate() {
            if let Some(kind) = catalog.kind(id) {
                let side = Vec3::new(-dir.z, 0.0, dir.x);
                let p = s.at + side * (0.7 + k as f32 * 0.4) - dir * 0.5;
                let it = crate::items::spawn_item(
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
                if id == "ak_mag" {
                    // (A magazine comes empty; this one's full.)
                    commands.queue(move |w: &mut World| {
                        if let Some(mut i) = w.get_mut::<crate::items::Item>(it) {
                            i.rounds = 30;
                        }
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> (crate::vr_harness::Vr, Zone) {
        let vr = crate::vr_harness::Vr::new(&crate::LaunchOptions::default().map);
        let zone = vr.app.world().resource::<Zone>().clone();
        (vr, zone)
    }

    fn hurt_pylon(vr: &mut crate::vr_harness::Vr, site: Vec3) -> Entity {
        let p = {
            let mut c = vr.app.world_mut().commands();
            let p = super::super::pylon::spawn_pylon(&mut c, site, 9999.0, site);
            vr.app.world_mut().flush();
            p
        };
        vr.step(2);
        let mut q = vr.app.world_mut().query::<&mut RobotPart>();
        for mut part in q.iter_mut(vr.app.world_mut()) {
            if part.robot == p {
                part.hp = part.max_hp * 0.3;
            }
        }
        p
    }

    fn hp_of(vr: &mut crate::vr_harness::Vr, robot: Entity) -> (f32, f32) {
        vr.app
            .world_mut()
            .query::<&RobotPart>()
            .iter(vr.app.world())
            .filter(|p| p.robot == robot)
            .fold((0.0, 0.0), |a, p| (a.0 + p.hp, a.1 + p.max_hp))
    }

    /// A hurt machine: the mender flies to it, welds it whole part by
    /// part, and goes on its way.
    #[test]
    fn it_flies_to_the_hurt_and_welds_them() {
        let (mut vr, zone) = world();
        vr.stand(zone.on_ground(140.0, 330.0), 0.0);
        let site = zone.on_ground(40.0, 230.0);
        let pylon = hurt_pylon(&mut vr, site);
        let (before, max) = hp_of(&mut vr, pylon);
        assert!(before < max * 0.5);
        let from = site + Vec3::new(50.0, 9.0, 0.0);
        let e = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_mender(&mut c, from, from);
            vr.app.world_mut().flush();
            e
        };
        let mut welded = false;
        for _ in 0..(90 * 4 * 30) {
            vr.step(1);
            welded |= vr
                .app
                .world()
                .get::<Mender>(e)
                .is_some_and(|m| m.welding.is_some());
            if hp_of(&mut vr, pylon).0 >= max * 0.9 {
                break;
            }
        }
        assert!(welded, "it struck an arc");
        let (after, _) = hp_of(&mut vr, pylon);
        assert!(after >= max * 0.9, "{before} -> {after} of {max}");
        let n = vr.app.world().resource::<crate::bot::Counters>().0[WELDED];
        assert!(n as f32 >= (after - before) * 0.9);
    }

    /// Shoot its arm off and it can't mend anything.
    #[test]
    fn without_its_arm_it_mends_nothing() {
        let (mut vr, zone) = world();
        vr.stand(zone.on_ground(140.0, 330.0), 0.0);
        let site = zone.on_ground(40.0, 230.0);
        let pylon = hurt_pylon(&mut vr, site);
        let (before, _) = hp_of(&mut vr, pylon);
        let from = site + Vec3::new(15.0, 9.0, 0.0);
        let e = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_mender(&mut c, from, from);
            vr.app.world_mut().flush();
            e
        };
        vr.step(3);
        let arm = vr
            .app
            .world_mut()
            .query_filtered::<(Entity, &RobotPart), With<WeldArm>>()
            .iter(vr.app.world())
            .find(|(_, p)| p.robot == e)
            .map(|(a, _)| a)
            .unwrap();
        crate::robots::heavy::tests::hit(&mut vr.app, arm, ARM_HP * 2.0);
        vr.step(90 * 30);
        let (after, _) = hp_of(&mut vr, pylon);
        assert!((after - before).abs() < 1.0, "{before} -> {after}");
    }
}
