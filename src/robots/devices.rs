//! The machines read what you set down: every device you place gives
//! something off, and they do something about it.
//!
//! One table, [`TABLE`]: for each kind of placed thing ([`Placed`]), what
//! it gives off while it runs ([`Gives`]): light (`glow`, m it's seen at),
//! sound (`hum`, m it's heard at), power draw (`draw`, watts: the
//! machines' own sensors pick a hungry device up, [`EM_PER_WATT`] m a
//! watt, if they're a drone or a relay, once it's run [`EM_SETTLE`]),
//! and how far it bites (`danger`).
//! What they do follows from that alone ([`reaction`]), so a new device
//! gets its behaviour from its row:
//!
//! - **Dangerous** (an armed trap): shot from outside its reach
//!   ([`Reaction::ShootFromAfar`]).
//! - **Hungry** (drawing [`HUNT_DRAW`] W or more: a jammer, a cloak, a
//!   camera drone): hunted and shot ([`Reaction::Hunt`]).
//! - **Otherwise** (a light, a laser): come and looked at
//!   ([`Reaction::Investigate`]).
//!
//! A machine that can't shoot (a relay, an unarmed walker) reports what
//! it would have hunted (`Director::queue_report`) and keeps away from
//! what bites. Only what's set down counts (not what you carry: that's
//! you, and they see you). A noticed device is shot only from within
//! [`SHOOT_RANGE`], after [`AIM_TIME`]; shot, it's wrecked: scrap where it
//! stood (a trap goes off where it lies).
//!
//! **The tell**: a [`DeviceNoticed`] for sound and light (a chirp, a scan
//! beam) the moment one's noticed, and the machine turns and goes to it.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use super::{Awareness, PartRole, Robot, RobotKind, RobotPart, Senses};
use crate::gadgets::DeviceKind;
use crate::world::Layer;

/// A kind of thing a player sets down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placed {
    Device(DeviceKind),
    /// An EMP trap, armed (`gadgets::Armed::Trap`).
    EmpTrap,
    /// A camera drone, spun up (`gadgets::camera_drone`).
    CameraDrone,
}

/// What a placed thing gives off while it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gives {
    /// Seen this far (m), in view and in line of sight.
    pub glow: f32,
    /// Heard this far (m), by ordinary ears.
    pub hum: f32,
    /// Power drawn (W).
    pub draw: f32,
    /// How far it bites (m), if it does.
    pub danger: f32,
}

/// Every placed thing: what it gives off.
pub const TABLE: &[(Placed, Gives)] = &[
    (
        Placed::Device(DeviceKind::Light),
        Gives {
            glow: 30.0,
            hum: 0.0,
            draw: 6.0,
            danger: 0.0,
        },
    ),
    // (The dot and the beam in the air.)
    (
        Placed::Device(DeviceKind::Laser),
        Gives {
            glow: 15.0,
            hum: 0.0,
            draw: 0.6,
            danger: 0.0,
        },
    ),
    // (Nothing to see, by design: its whine gives it away.)
    (
        Placed::Device(DeviceKind::Cloak),
        Gives {
            glow: 0.0,
            hum: 10.0,
            draw: 15.0,
            danger: 0.0,
        },
    ),
    (
        Placed::Device(DeviceKind::Jammer),
        Gives {
            glow: 3.0,
            hum: 6.0,
            draw: 20.0,
            danger: 0.0,
        },
    ),
    // (Its armed lamp blinking, seen from outside its reach, and its hum.)
    (
        Placed::EmpTrap,
        Gives {
            glow: 14.0,
            hum: 4.0,
            draw: 0.0,
            danger: crate::gadgets::EMP_RADIUS,
        },
    ),
    (
        Placed::CameraDrone,
        Gives {
            glow: 0.0,
            hum: 15.0,
            draw: 10.0,
            danger: 0.0,
        },
    ),
];

/// A device's row.
pub fn gives(p: Placed) -> Option<Gives> {
    TABLE.iter().find(|(k, _)| *k == p).map(|(_, g)| *g)
}

/// What the machines do about a placed thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    /// Come and look at it.
    Investigate,
    /// Hunt it down and shoot it.
    Hunt,
    /// Shoot it from outside its reach.
    ShootFromAfar,
}

/// A device this hungry (W) or more is hunted.
pub const HUNT_DRAW: f32 = 10.0;
/// A drone's or a relay's sensors pick up a device this far a watt (m)...
pub const EM_PER_WATT: f32 = 3.0;
/// ...once it's been running this long (s): its draw takes a while to
/// stand out, so a device buys you that much before it's a beacon.
pub const EM_SETTLE: f32 = 30.0;
/// They shoot one from within this (m)...
pub const SHOOT_RANGE: f32 = 25.0;
/// ...having held it in sight this long (s)...
pub const AIM_TIME: f32 = 1.5;
/// ...and stand off a dangerous one by this much past its reach (m).
pub const STAND_OFF: f32 = 3.0;
/// Each machine looks for devices this often (s).
pub const LOOK_EVERY: f32 = 0.5;
/// Counters a scenario reads.
pub const NOTICED: &str = "device_noticed";
pub const WRECKED: &str = "device_wrecked";

/// What follows from what a thing gives off.
pub fn reaction(g: &Gives) -> Reaction {
    if g.danger > 0.0 {
        Reaction::ShootFromAfar
    } else if g.draw >= HUNT_DRAW {
        Reaction::Hunt
    } else {
        Reaction::Investigate
    }
}

/// A machine noticed a device (for sound and light: its chirp, its scan
/// beam; the voice of `search::MindShift` follows as it turns to it).
#[derive(Message, Clone, Copy, Debug)]
pub struct DeviceNoticed {
    pub robot: Entity,
    pub device: Entity,
    pub at: Vec3,
    pub placed: Placed,
    pub reaction: Reaction,
}

/// A machine going for a device: since when it's had it in its sights.
#[derive(Component, Debug, Clone, Copy)]
pub struct Hunting {
    pub device: Entity,
    pub aimed: f32,
    pub keep_off: f32,
}

pub struct DevicesPlugin;

impl Plugin for DevicesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DeviceNoticed>()
            .add_systems(
                FixedUpdate,
                (notice_devices, hunt_devices)
                    .chain()
                    .after(super::perceive),
            )
            .add_systems(FixedUpdate, (stage_scenarios, seat_cells_later));
    }
}

/// Which placed thing an item is, if it's running.
fn placed_of(
    device: Option<&crate::gadgets::Device>,
    powered: bool,
    armed: Option<&crate::gadgets::Armed>,
    drone: bool,
) -> Option<Placed> {
    if let Some(crate::gadgets::Armed::Trap { .. }) = armed {
        return Some(Placed::EmpTrap);
    }
    if drone {
        return Some(Placed::CameraDrone);
    }
    device.filter(|_| powered).map(|d| Placed::Device(d.kind))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn notice_devices(
    mut commands: Commands,
    items: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&crate::gadgets::Device>,
            Has<crate::gadgets::Powered>,
            Option<&crate::gadgets::Armed>,
            Has<crate::gadgets::camera_drone::CameraDrone>,
        ),
        (
            With<crate::items::Item>,
            Without<crate::anomaly::artifacts::CarriedBy>,
        ),
    >,
    mut robots: Query<(
        Entity,
        &Robot,
        &Senses,
        &GlobalTransform,
        &mut Awareness,
        Has<Hunting>,
        Has<super::mender::Mender>,
    )>,
    parts: Query<&RobotPart>,
    director: Option<ResMut<super::director::Director>>,
    spatial: SpatialQuery,
    mut out: MessageWriter<DeviceNoticed>,
    mut counters: Option<ResMut<crate::bot::Counters>>,
    time: Res<Time>,
    mut seen: Local<HashMap<Entity, (f32, HashSet<Entity>)>>,
    mut since: Local<HashMap<Entity, f32>>,
) {
    let mut director = director;
    let now = time.elapsed_secs();
    let running: Vec<(Entity, Vec3, Placed, Gives)> = items
        .iter()
        .filter_map(|(e, gt, d, powered, armed, drone)| {
            let p = placed_of(d, powered, armed, drone)?;
            Some((e, gt.translation(), p, gives(p)?))
        })
        .collect();
    seen.retain(|e, _| robots.contains(*e));
    // How long each has been running.
    since.retain(|e, _| running.iter().any(|r| r.0 == *e));
    for r in &running {
        since.entry(r.0).or_insert(now);
    }
    if running.is_empty() {
        return;
    }
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    let armed: HashSet<Entity> = parts
        .iter()
        .filter(|p| p.role == PartRole::Weapon && p.alive())
        .map(|p| p.robot)
        .collect();
    for (r, robot, senses, gt, mut aware, hunting, mender) in &mut robots {
        if robot.wrecked {
            continue;
        }
        let (last, known) = seen.entry(r).or_insert((f32::NEG_INFINITY, HashSet::new()));
        if now - *last < LOOK_EVERY {
            continue;
        }
        *last = now;
        known.retain(|d| running.iter().any(|x| x.0 == *d));
        let eye = gt.transform_point(senses.eye);
        let forward = gt.forward().as_vec3();
        let sensor = matches!(robot.kind, RobotKind::Drone | RobotKind::Pylon);
        for &(dev, at, placed, g) in &running {
            if known.contains(&dev) {
                continue;
            }
            let to = at - eye;
            let d = to.length().max(0.01);
            let sees = g.glow > 0.0
                && d < g.glow
                && forward.angle_between(to / d).to_degrees() <= senses.fov_deg * 0.5
                && Dir3::new(to).ok().is_some_and(|dir| {
                    spatial
                        .cast_ray(eye, dir, (d - 0.3).max(0.0), true, &world)
                        .is_none()
                });
            let hears = d < g.hum * senses.hearing;
            let settled = since.get(&dev).is_some_and(|t| now - t >= EM_SETTLE);
            let senses_draw = sensor && settled && d < g.draw * EM_PER_WATT;
            if !(sees || hears || senses_draw) {
                continue;
            }
            known.insert(dev);
            let reaction = reaction(&g);
            out.write(DeviceNoticed {
                robot: r,
                device: dev,
                at,
                placed,
                reaction,
            });
            if let Some(c) = counters.as_mut() {
                c.bump(NOTICED);
            }
            let shooter = armed.contains(&r) && !mender;
            let keep_off = if g.danger > 0.0 {
                g.danger + STAND_OFF
            } else {
                0.0
            };
            // Where it goes: to it, or (one that bites) to just outside it.
            let pos = gt.translation();
            let go = if keep_off > 0.0 {
                at + (pos - at).with_y(0.0).normalize_or(Vec3::X) * keep_off
            } else {
                at
            };
            match reaction {
                Reaction::Hunt | Reaction::ShootFromAfar if shooter => {
                    if !hunting {
                        commands.entity(r).try_insert(Hunting {
                            device: dev,
                            aimed: 0.0,
                            keep_off,
                        });
                    }
                }
                Reaction::Hunt | Reaction::ShootFromAfar => {
                    // Can't shoot it: report it (and stand clear).
                    if let Some(dir) = director.as_mut() {
                        let delay = dir.rng.range(10.0, 20.0);
                        dir.queue_report(r, pos, delay, go);
                    }
                    if keep_off > 0.0 {
                        continue;
                    }
                }
                Reaction::Investigate => {}
            }
            if !aware.can_see_target {
                aware.last_known = Some(go);
                aware.uncertainty = 2.0;
                aware.suspicion = aware.suspicion.max(super::search::INVESTIGATE_AT + 0.1);
                aware.news = true;
            }
        }
    }
}

/// A machine with a device in its sights, near enough: it aims, then
/// shoots it to pieces.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn hunt_devices(
    mut commands: Commands,
    mut hunters: Query<(
        Entity,
        &Robot,
        &Senses,
        &GlobalTransform,
        &mut Awareness,
        &mut Hunting,
    )>,
    devices: Query<(&GlobalTransform, Option<&crate::gadgets::Armed>), With<crate::items::Item>>,
    catalog: Res<crate::items::Catalog>,
    spatial: SpatialQuery,
    mut shots: MessageWriter<crate::weapons::ballistics::Shot>,
    mut counters: Option<ResMut<crate::bot::Counters>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    for (r, robot, senses, gt, mut aware, mut h) in &mut hunters {
        let Ok((dg, armed)) = devices.get(h.device) else {
            commands.entity(r).remove::<Hunting>();
            continue;
        };
        if robot.wrecked {
            commands.entity(r).remove::<Hunting>();
            continue;
        }
        let at = dg.translation();
        let eye = gt.transform_point(senses.eye);
        let to = at - eye;
        let d = to.length().max(0.01);
        // Going for it (unless it's after you now).
        if !aware.can_see_target {
            let pos = gt.translation();
            let go = if h.keep_off > 0.0 {
                at + (pos - at).with_y(0.0).normalize_or(Vec3::X) * h.keep_off
            } else {
                at
            };
            aware.last_known = Some(go);
            aware.uncertainty = 1.0;
            aware.suspicion = aware.suspicion.max(super::search::INVESTIGATE_AT + 0.1);
        }
        let clear = Dir3::new(to).ok().is_some_and(|dir| {
            spatial
                .cast_ray(eye, dir, (d - 0.3).max(0.0), true, &world)
                .is_none()
        });
        let in_range = d <= SHOOT_RANGE && d >= h.keep_off - 0.5 && clear;
        h.aimed = if in_range { h.aimed + dt } else { 0.0 };
        if h.aimed < AIM_TIME {
            continue;
        }
        // Shot: a round its way (heard, seen), and it's in pieces.
        shots.write(crate::weapons::ballistics::Shot {
            gun: r,
            shooter: Some(r),
            origin: eye,
            direction: to / d,
            speed: 700.0,
            damage: 20.0,
            projectiles: 1,
            spread: 0.0,
            recoil: 0.0,
            seed: r.to_bits() ^ h.device.to_bits(),
        });
        if let Some(c) = counters.as_mut() {
            c.bump(WRECKED);
        }
        if matches!(armed, Some(crate::gadgets::Armed::Trap { .. })) {
            // A trap shot goes off where it lies.
            commands
                .entity(h.device)
                .try_insert(crate::gadgets::Armed::Fuse(0.0));
        } else {
            commands.entity(h.device).despawn();
            if let Some(kind) = catalog.kind("scrap_metal") {
                crate::items::spawn_item(
                    &mut commands,
                    &catalog,
                    crate::items::SpawnItem {
                        kind,
                        transform: Transform::from_translation(at + Vec3::Y * 0.1),
                        condition: 0.3,
                    },
                );
            }
        }
        info!("devices: a machine shot a device to pieces");
        commands.entity(r).remove::<Hunting>();
    }
}

/// Every device kind there is (a new one fails to compile here until it's
/// listed, and the table test then wants its row).
pub fn every_device_kind() -> Vec<DeviceKind> {
    let all = [
        DeviceKind::Light,
        DeviceKind::Laser,
        DeviceKind::Cloak,
        DeviceKind::Jammer,
    ];
    // (Exhaustive: add the new kind above too.)
    for k in all {
        match k {
            DeviceKind::Light | DeviceKind::Laser | DeviceKind::Cloak | DeviceKind::Jammer => {}
        }
    }
    all.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No device is invisible to them: every kind has a row, and every
    /// row gives something off.
    #[test]
    fn every_device_kind_has_a_row() {
        for k in every_device_kind() {
            let g = gives(Placed::Device(k)).unwrap_or_else(|| panic!("{k:?} has no row in TABLE"));
            assert!(g.glow + g.hum + g.draw > 0.0, "{k:?} gives nothing off");
        }
        for p in [Placed::EmpTrap, Placed::CameraDrone] {
            assert!(gives(p).is_some(), "{p:?} has no row");
        }
    }

    /// What they do follows from what it gives off: what bites is shot
    /// from afar, what's hungry hunted, the rest looked at.
    #[test]
    fn the_reaction_follows_from_the_row() {
        let r = |p| reaction(&gives(p).unwrap());
        assert_eq!(r(Placed::EmpTrap), Reaction::ShootFromAfar);
        assert_eq!(r(Placed::Device(DeviceKind::Jammer)), Reaction::Hunt);
        assert_eq!(r(Placed::Device(DeviceKind::Cloak)), Reaction::Hunt);
        assert_eq!(r(Placed::CameraDrone), Reaction::Hunt);
        assert_eq!(r(Placed::Device(DeviceKind::Light)), Reaction::Investigate);
        assert_eq!(r(Placed::Device(DeviceKind::Laser)), Reaction::Investigate);
    }
}

/// One scenario a device: it's set down running near a machine, which
/// notices it (`NOTICED`) and does what its row says (a hunted or a
/// dangerous one is shot: `WRECKED`). (name, item, who watches, shot?)
pub const SCENARIOS: &[(&str, &str, Watcher, bool)] = &[
    ("device_light", "flashlight", Watcher::Walker, false),
    ("device_laser", "laser_module", Watcher::Walker, false),
    ("device_cloak", "cloak_generator", Watcher::Drone, true),
    ("device_jammer", "jammer", Watcher::Drone, true),
    ("device_emp_trap", "emp_trap", Watcher::ArmedWalker, true),
];

/// Who's set by a scenario's device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watcher {
    /// Unarmed, 8 m off, facing it.
    Walker,
    /// Armed, 13 m off, facing it.
    ArmedWalker,
    /// A survey drone, 30 m off and up.
    Drone,
}

/// A scenario's device waits for its cell bay to be there to be seated.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct SeatCellLater(Entity);

pub(crate) fn stage_scenarios(
    mut commands: Commands,
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    zone: Res<crate::world::Zone>,
    nav: Res<super::walker::Nav>,
    catalog: Res<crate::items::Catalog>,
) {
    for s in setups.read() {
        let Some(&(_, id, watcher, _)) = SCENARIOS.iter().find(|x| x.0 == s.name) else {
            continue;
        };
        // Off to the bot's side, on open ground.
        let f = Vec3::new(s.facing.x, 0.0, s.facing.z).normalize_or(Vec3::NEG_Z);
        let side = Vec3::new(-f.z, 0.0, f.x);
        let open = |d: Vec3| {
            (2..=40).step_by(2).all(|m| {
                let p = s.at + d * m as f32;
                nav.0.walkable([p.x, p.z])
            })
        };
        let dir = (0..16)
            .map(|k| Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * side)
            .find(|d| open(*d))
            .unwrap_or(side);
        let at = zone.on_ground(s.at.x + dir.x * 16.0, s.at.z + dir.z * 16.0) + Vec3::Y * 0.2;
        let Some(kind) = catalog.kind(id) else {
            continue;
        };
        let dev = crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_translation(at),
                condition: 1.0,
            },
        );
        if id == "emp_trap" {
            commands
                .entity(dev)
                .insert(crate::gadgets::Armed::Trap { radius: 4.0 });
        } else if let Some(cell) = catalog.kind("power_cell") {
            let c = crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind: cell,
                    transform: Transform::from_translation(at + Vec3::Y * 0.3),
                    condition: 1.0,
                },
            );
            commands.entity(dev).insert(SeatCellLater(c));
        }
        // The watcher, beyond it from the bot, facing it (walkers face -Z:
        // set them on its +Z side).
        let job = |site: Vec3| super::Job {
            kind: super::JobKind::Watch,
            site,
            duration: 999.0,
            exit: site,
        };
        match watcher {
            Watcher::Walker | Watcher::ArmedWalker => {
                let off = if watcher == Watcher::Walker {
                    8.0
                } else {
                    13.0
                };
                let post = zone.on_ground(at.x, at.z + off);
                super::walker::spawn_walker_with(
                    &mut commands,
                    &zone,
                    post,
                    job(post),
                    watcher == Watcher::ArmedWalker,
                    130.0,
                );
            }
            Watcher::Drone => {
                let site = at + dir * 30.0 + Vec3::Y * 12.0;
                let mut j = job(site);
                j.kind = super::JobKind::Survey;
                super::drone::spawn_drone(&mut commands, site, j);
            }
        }
        info!("scenario: a running {id} at {at:.0}");
    }
}

/// Seat a scenario device's cell once its bay is there, and switch it on.
pub(crate) fn seat_cells_later(
    mut commands: Commands,
    waiting: Query<(Entity, &SeatCellLater)>,
    mut sockets: Query<(
        Entity,
        &mut crate::interact::Socket,
        &crate::interact::SocketOwner,
    )>,
) {
    for (dev, w) in &waiting {
        let Some((se, mut socket, _)) = sockets.iter_mut().find(|(_, s, o)| {
            o.0 == dev
                && s.kind == crate::interact::SocketKind::Mount(extraction_core::items::Mount::Cell)
        }) else {
            continue;
        };
        crate::interact::seat_in_socket(&mut commands, w.0, se, &mut socket);
        commands
            .entity(dev)
            .remove::<SeatCellLater>()
            .insert(crate::items::usage::LightOn);
    }
}
