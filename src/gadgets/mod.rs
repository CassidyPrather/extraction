//! Gadgets and attachments.
//!
//! - **Mounts**: guns have a dovetail (scopes, the side-mount rail) and a
//!   muzzle (the artifact adapter); the side-mount rail has a rail (lights,
//!   lasers); lights and lasers have a cell compartment; the adapter has a
//!   cradle for an artifact. Bring a thing close to where it goes and let
//!   go: it seats like a magazine. Mounted things follow the gun around
//!   ([`MountedOn`]).
//! - **Power**: lights and lasers run on power cells, which drain while
//!   they're switched on — unless whoever carries them also carries a
//!   hearth. Use (U / the use button) switches a held light, or every
//!   light and laser on a held gun.
//! - **EMP**: arm a grenade and throw it, or arm a trap and leave it.
//!   Robots in the blast lose their senses, rotors and legs for a while.
//! - **Optics** ([`Optic`]): the scope and binoculars magnify (client side).
//! - **Camera drone** ([`camera_drone`]): flies a circle and spots robots.
//! - **Lead-lined case** ([`LeadCase`]): hides an artifact from the Zone.
//! - **Climbing kit** ([`climbing`]): a grapnel and a rope to climb.

pub mod camera_drone;
pub mod climbing;
pub mod visuals;

use bevy::prelude::*;
use extraction_core::items::Mount;

use crate::anomaly::artifacts::{ArtifactEffects, CarriedBy};
use crate::interact::{Held, InSocket, Socket, SocketKind, SocketOwner};
use crate::items::usage::{LightOn, UseItem};
use crate::items::{Catalog, Item};
use crate::robots::{Robot, RobotPart, Stunned};
use crate::weapons::Gun;

/// The item at the root of whatever this is mounted on (a scope's gun, a
/// cell's flashlight).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MountedOn(pub Entity);

/// A device that draws power while switched on ([`LightOn`]), in watts.
#[derive(Component, Clone, Copy, Debug)]
pub struct Device {
    pub watts: f32,
    pub kind: DeviceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Light,
    Laser,
    /// Set down and switched on, it hides everyone inside its bubble.
    Cloak,
    /// Switched on, the machines' relays near it can't report
    /// (`robots::network::Jammer`).
    Jammer,
}

/// Switched on *and* has power this tick.
#[derive(Component, Clone, Copy, Debug)]
pub struct Powered;

/// A gun with a suppressor on its muzzle: robots hear its shots from
/// less far ([`SUPPRESSED_REACH`] of the usual), and it sounds quieter.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Suppressed;

/// How far a suppressed shot carries, against the same shot bare.
pub const SUPPRESSED_REACH: f32 = 0.35;

/// Magnifying glass: where the eye goes (item space) and how much.
#[derive(Component, Clone, Copy, Debug)]
pub struct Optic {
    pub magnification: f32,
    pub eye: Vec3,
}

/// An armed EMP: a grenade counting down, or a trap waiting.
#[derive(Component, Clone, Copy, Debug)]
pub enum Armed {
    Fuse(f32),
    Trap { radius: f32 },
}

/// An EMP went off (for visuals and sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct EmpBlast {
    pub at: Vec3,
    pub radius: f32,
}

pub const EMP_RADIUS: f32 = 9.0;
pub const EMP_STUN: f32 = 12.0;

/// Where running cloaks are this tick: (centre, radius).
#[derive(Resource, Default, Debug)]
pub struct Cloaks(pub Vec<(Vec3, f32)>);

impl Cloaks {
    pub fn hides(&self, p: Vec3) -> bool {
        self.0.iter().any(|(c, r)| c.distance(p) < *r)
    }
}

/// A deployed cover panel (a wall; fold it back up with a grab).
#[derive(Component)]
pub struct CoverWall;

pub const CLOAK_RADIUS: f32 = 3.0;

pub struct GadgetsPlugin;

impl Plugin for GadgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((camera_drone::CameraDronePlugin, climbing::ClimbingPlugin))
            .add_message::<EmpBlast>()
            .init_resource::<Cloaks>()
            .add_observer(gun_mounts)
            .add_observer(item_mounts)
            .add_systems(
                FixedUpdate,
                (
                    track_mounts,
                    seat_on_muzzle,
                    mark_suppressed,
                    power_devices,
                    arm_and_switch,
                    fuses,
                    recover,
                    cloaks,
                    covers,
                )
                    .chain()
                    .after(crate::interact::InteractSystems::Follow),
            );
    }
}

/// Where each gun takes a scope (or rail) and an adapter, gun space.
fn gun_mount_points(id: &str, muzzle: Vec3) -> Vec<(Mount, Transform)> {
    let mut v = Vec::new();
    match id {
        "ak74" => v.push((Mount::Dovetail, Transform::from_xyz(0.0, 0.125, -0.12))),
        "mosin" => v.push((Mount::Dovetail, Transform::from_xyz(0.0, 0.112, -0.1))),
        // Over the receiver cover, clear of the rear sight.
        "sks" => v.push((Mount::Dovetail, Transform::from_xyz(0.0, 0.1, -0.08))),
        // The RPK's is the AK's; the SVD's scope sits over the bore off
        // the rail on its left; the TOZ-8's receiver has a dovetail.
        "rpk74" => v.push((Mount::Dovetail, Transform::from_xyz(0.0, 0.125, -0.12))),
        "svd" => v.push((Mount::Dovetail, Transform::from_xyz(0.0, 0.125, -0.1))),
        "toz8" => v.push((Mount::Dovetail, Transform::from_xyz(0.0, 0.085, -0.11))),
        _ => {}
    }
    // Shotguns and the flare gun have no threaded muzzle.
    if !matches!(id, "toz_double" | "mp133" | "flare_pistol" | "crossbow") {
        v.push((Mount::Muzzle, Transform::from_translation(muzzle)));
    }
    v
}

/// A lead-lined case: an artifact seated in it is [`Shielded`](crate::anomaly::artifacts::Shielded).
#[derive(Component, Debug)]
pub struct LeadCase;

fn spawn_mount(
    commands: &mut Commands,
    owner: Entity,
    mount: Mount,
    at: Transform,
    seat: Transform,
) {
    commands.spawn((
        Name::new(format!("{mount:?} mount")),
        Socket {
            kind: SocketKind::Mount(mount),
            occupant: None,
            radius: 0.09,
            seat,
        },
        SocketOwner(owner),
        at,
        Visibility::default(),
        ChildOf(owner),
    ));
}

fn gun_mounts(
    add: On<Add, Gun>,
    mut commands: Commands,
    guns: Query<(&Gun, &Item)>,
    catalog: Res<Catalog>,
) {
    let Ok((gun, item)) = guns.get(add.entity) else {
        return;
    };
    let id = &catalog.get(item.kind()).id;
    for (m, at) in gun_mount_points(id, gun.layout.muzzle) {
        // The adapter sits in front of the muzzle.
        let seat = if m == Mount::Muzzle {
            Transform::from_xyz(0.0, 0.0, -0.045)
        } else {
            Transform::IDENTITY
        };
        spawn_mount(&mut commands, add.entity, m, at, seat);
    }
}

/// Sockets that come with gadgets themselves, and their behaviour markers.
fn item_mounts(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    let e = add.entity;
    match catalog.get(item.kind()).id.as_str() {
        "side_rail" => spawn_mount(
            &mut commands,
            e,
            Mount::Rail,
            Transform::from_xyz(0.0, 0.006, 0.0),
            Transform::from_xyz(0.0, 0.016, 0.0),
        ),
        "flashlight" | "weapon_light" | "laser_module" | "cloak_generator" | "jammer" => {
            let id = catalog.get(item.kind()).id.as_str();
            spawn_mount(
                &mut commands,
                e,
                Mount::Cell,
                Transform::from_xyz(0.0, 0.0, 0.02),
                Transform::IDENTITY,
            );
            let (watts, kind) = match id {
                "laser_module" => (0.6, DeviceKind::Laser),
                // A cell runs it for about forty minutes.
                "cloak_generator" => (15.0, DeviceKind::Cloak),
                // Hungrier still: a cell lasts it about half an hour.
                "jammer" => (20.0, DeviceKind::Jammer),
                _ => (6.0, DeviceKind::Light),
            };
            commands.entity(e).insert(Device { watts, kind });
        }
        "zipline_kit" => {
            commands.entity(e).insert(climbing::Zipline::Packed);
        }
        "climbing_kit" => {
            commands.entity(e).insert(climbing::Grapnel::Coiled);
        }
        "camera_drone" => {
            commands
                .entity(e)
                .insert(camera_drone::CameraDrone::default());
        }
        "lead_case" => {
            spawn_mount(
                &mut commands,
                e,
                Mount::Cradle,
                Transform::from_xyz(0.0, 0.0, 0.0),
                Transform::IDENTITY,
            );
            commands.entity(e).insert(LeadCase);
        }
        "artifact_adapter" => spawn_mount(
            &mut commands,
            e,
            Mount::Cradle,
            Transform::from_xyz(0.0, 0.03, 0.0),
            Transform::IDENTITY,
        ),
        "pu_scope" => {
            commands.entity(e).insert(Optic {
                magnification: 3.5,
                eye: Vec3::new(0.0, 0.0, 0.2),
            });
        }
        "pso_scope" => {
            commands.entity(e).insert(Optic {
                magnification: 4.0,
                eye: Vec3::new(0.0, 0.0, 0.2),
            });
        }
        "binoculars" => {
            commands.entity(e).insert(Optic {
                magnification: 6.0,
                eye: Vec3::new(0.0, 0.0, 0.1),
            });
        }
        _ => {}
    }
}

/// Keeps [`MountedOn`] pointing at the root item of every mounted thing.
fn track_mounts(
    mut commands: Commands,
    items: Query<(Entity, Option<&InSocket>, Option<&MountedOn>), With<Item>>,
    sockets: Query<(&Socket, &SocketOwner)>,
) {
    for (e, sock, current) in &items {
        let mut root = None;
        let mut s = sock.map(|s| s.socket);
        for _ in 0..4 {
            let Some(se) = s else {
                break;
            };
            let Ok((socket, owner)) = sockets.get(se) else {
                break;
            };
            if !matches!(socket.kind, SocketKind::Mount(_)) {
                break;
            }
            root = Some(owner.0);
            s = items
                .get(owner.0)
                .ok()
                .and_then(|(_, is, _)| is.map(|i| i.socket));
        }
        let want = root.map(MountedOn);
        if want.as_ref() != current {
            match want {
                Some(m) => {
                    commands.entity(e).insert(m);
                }
                None => {
                    commands.entity(e).remove::<MountedOn>();
                }
            }
        }
    }
}

/// A thing on a muzzle sits with its back face on the muzzle, however
/// long it is (the mount's seat is right for the short adapter only).
fn seat_on_muzzle(
    mut seated: Query<(&InSocket, &Item, &mut Transform), Added<InSocket>>,
    sockets: Query<&Socket>,
    catalog: Res<Catalog>,
) {
    for (in_socket, item, mut t) in &mut seated {
        let Ok(socket) = sockets.get(in_socket.socket) else {
            continue;
        };
        if socket.kind == SocketKind::Mount(Mount::Muzzle) {
            t.translation.z = -catalog.get(item.kind()).size_m()[2] * 0.5;
        }
    }
}

/// Keeps [`Suppressed`] on every gun that has a suppressor on its muzzle.
fn mark_suppressed(
    mut commands: Commands,
    guns: Query<(Entity, Has<Suppressed>), With<Gun>>,
    sockets: Query<(&Socket, &SocketOwner)>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    for (gun, marked) in &guns {
        let fitted = sockets.iter().any(|(s, o)| {
            o.0 == gun
                && s.kind == SocketKind::Mount(Mount::Muzzle)
                && s.occupant
                    .and_then(|i| items.get(i).ok())
                    .is_some_and(|i| catalog.get(i.kind()).has_tag("suppressor"))
        });
        if fitted && !marked {
            commands.entity(gun).insert(Suppressed);
        } else if !fitted && marked {
            commands.entity(gun).remove::<Suppressed>();
        }
    }
}

/// The cell seated in an item's own cell compartment.
fn cell_of(item: Entity, sockets: &Query<(&Socket, &SocketOwner)>) -> Option<Entity> {
    sockets
        .iter()
        .find(|(s, o)| o.0 == item && s.kind == SocketKind::Mount(Mount::Cell))
        .and_then(|(s, _)| s.occupant)
}

/// Switched-on devices draw from their cells; a carried hearth feeds them
/// for nothing.
fn power_devices(
    mut commands: Commands,
    devices: Query<(
        Entity,
        &Device,
        Has<LightOn>,
        Has<Powered>,
        Option<&CarriedBy>,
    )>,
    sockets: Query<(&Socket, &SocketOwner)>,
    mut cells: Query<&mut Item>,
    effects: Query<&ArtifactEffects>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, dev, on, was, carrier) in &devices {
        let hearth = carrier.is_some_and(|c| effects.get(c.0).is_ok_and(|fx| fx.hearth));
        let mut powered = false;
        if on {
            if let Some(cell) = cell_of(e, &sockets) {
                if let Ok(mut it) = cells.get_mut(cell) {
                    if it.charge_wh > 0.0 || hearth {
                        powered = true;
                        if !hearth {
                            it.charge_wh = (it.charge_wh - dev.watts * dt / 3600.0).max(0.0);
                        }
                    }
                }
            }
        }
        if powered != was {
            if powered {
                commands.entity(e).insert(Powered);
            } else {
                commands.entity(e).remove::<Powered>();
            }
        }
    }
}

/// Use on a gun switches what's mounted on it; use on an EMP arms it.
fn arm_and_switch(
    mut commands: Commands,
    mut uses: MessageReader<UseItem>,
    items: Query<&Item>,
    guns: Query<(), With<Gun>>,
    mounted: Query<(Entity, &MountedOn, Has<LightOn>), With<Device>>,
    armed: Query<(), With<Armed>>,
    on: Query<(), With<LightOn>>,
    catalog: Res<Catalog>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for u in uses.read() {
        if guns.contains(u.item) {
            let mine: Vec<(Entity, bool)> = mounted
                .iter()
                .filter(|(_, m, _)| m.0 == u.item)
                .map(|(e, _, on)| (e, on))
                .collect();
            // All on if any are off, else all off.
            let switch_on = mine.iter().any(|(_, on)| !on);
            for (e, _) in mine {
                if switch_on {
                    commands.entity(e).insert(LightOn);
                } else {
                    commands.entity(e).remove::<LightOn>();
                }
            }
            continue;
        }
        let Ok(item) = items.get(u.item) else {
            continue;
        };
        if armed.contains(u.item) {
            continue;
        }
        match catalog.get(item.kind()).id.as_str() {
            "emp_grenade" => {
                commands.entity(u.item).insert(Armed::Fuse(3.5));
                raid.say("You pull the pin. Three. Two...");
            }
            "cloak_generator" => {
                if on.contains(u.item) {
                    commands.entity(u.item).remove::<LightOn>();
                } else {
                    commands.entity(u.item).insert(LightOn);
                    raid.say("The generator whines up. Set it down and stay close.");
                }
            }
            "jammer" => {
                if on.contains(u.item) {
                    commands.entity(u.item).remove::<LightOn>();
                } else {
                    commands.entity(u.item).insert(LightOn);
                    raid.say("The jammer whines up. Every relay near you goes deaf.");
                }
            }
            "emp_trap" => {
                commands.entity(u.item).insert(Armed::Trap { radius: 4.0 });
                raid.say("The trap clicks and starts to hum. Put it down somewhere.");
            }
            _ => {}
        }
    }
}

/// Running cloaks: switched on, powered, and set down loose.
fn cloaks(
    devices: Query<(
        &Device,
        &GlobalTransform,
        Has<Powered>,
        Has<Held>,
        Has<InSocket>,
        Has<crate::interact::body::InBackpack>,
    )>,
    mut cloaks: ResMut<Cloaks>,
) {
    cloaks.0.clear();
    for (d, gt, powered, held, socketed, stowed) in &devices {
        if d.kind == DeviceKind::Cloak && powered && !held && !socketed && !stowed {
            cloaks.0.push((gt.translation(), CLOAK_RADIUS));
        }
    }
}

/// Deploying cover: the folded panel becomes a wall where you stand,
/// facing the way you face; grabbing the wall folds it back up.
fn covers(
    mut commands: Commands,
    mut uses: MessageReader<UseItem>,
    mut presses: MessageReader<crate::interact::InteractEvent>,
    items: Query<&Item>,
    walls: Query<&GlobalTransform, With<CoverWall>>,
    players: Query<&GlobalTransform>,
    mut hands: Query<&mut crate::interact::Hand>,
    catalog: Res<Catalog>,
    zone: Res<crate::world::Zone>,
) {
    use avian3d::prelude::*;
    for u in uses.read() {
        let Ok(item) = items.get(u.item) else {
            continue;
        };
        if catalog.get(item.kind()).id != "deployable_cover" {
            continue;
        }
        let Ok(p) = players.get(u.player) else {
            continue;
        };
        let fwd = p.forward().as_vec3();
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        let at = p.translation() + flat * 1.0;
        let ground = zone.ground(at.x, at.z);
        for mut h in &mut hands {
            if h.holding == Some(u.item) {
                h.holding = None;
            }
        }
        commands.entity(u.item).despawn();
        commands.spawn((
            Name::new("Cover"),
            CoverWall,
            crate::raid::truck::Pressable::FoldCover,
            crate::interact::Grabbable {
                points: vec![crate::interact::GrabPoint::free()],
            },
            Collider::cuboid(1.2, 1.05, 0.06),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Transform::from_xyz(at.x, ground + 0.55, at.z).looking_to(-flat, Vec3::Y),
            Visibility::default(),
        ));
    }
    for ev in presses.read() {
        let crate::interact::InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok(gt) = walls.get(*item) else {
            continue;
        };
        let at = gt.translation();
        commands.entity(*item).despawn();
        if let Some(kind) = catalog.kind("deployable_cover") {
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            );
        }
    }
}

fn fuses(
    mut commands: Commands,
    mut armed: Query<(
        Entity,
        &mut Armed,
        &GlobalTransform,
        Has<Held>,
        Has<InSocket>,
    )>,
    robots: Query<(Entity, &GlobalTransform), (With<Robot>, Without<Stunned>)>,
    all_robots: Query<(Entity, &GlobalTransform), With<Robot>>,
    parts: Query<(&RobotPart, &GlobalTransform)>,
    time: Res<Time>,
    mut blasts: MessageWriter<EmpBlast>,
    mut hands: Query<&mut crate::interact::Hand>,
) {
    let dt = time.delta_secs();
    for (e, mut a, gt, held, socketed) in &mut armed {
        let at = gt.translation();
        let go = match &mut *a {
            Armed::Fuse(t) => {
                *t -= dt;
                *t <= 0.0
            }
            // Traps only bite once they're set down loose.
            Armed::Trap { radius } => {
                !held
                    && !socketed
                    && robots
                        .iter()
                        .any(|(_, r)| r.translation().distance(at) < *radius)
            }
        };
        if !go {
            continue;
        }
        let mut hit = Vec::new();
        for (part, pgt) in &parts {
            if pgt.translation().distance(at) < EMP_RADIUS && !hit.contains(&part.robot) {
                hit.push(part.robot);
            }
        }
        for (r, rgt) in &all_robots {
            if rgt.translation().distance(at) < EMP_RADIUS && !hit.contains(&r) {
                hit.push(r);
            }
        }
        for r in hit {
            commands.entity(r).insert(Stunned(EMP_STUN));
        }
        blasts.write(EmpBlast {
            at,
            radius: EMP_RADIUS,
        });
        for mut h in &mut hands {
            if h.holding == Some(e) {
                h.holding = None;
            }
        }
        commands.entity(e).despawn();
    }
}

fn recover(mut commands: Commands, mut stunned: Query<(Entity, &mut Stunned)>, time: Res<Time>) {
    for (e, mut s) in &mut stunned {
        s.0 -= time.delta_secs();
        if s.0 <= 0.0 {
            commands.entity(e).remove::<Stunned>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{SpawnItem, spawn_item};

    fn spawn(app: &mut App, id: &str, at: Vec3) -> Entity {
        let catalog = app.world().resource::<Catalog>().clone();
        let kind = catalog.kind(id).unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, app.world());
            spawn_item(
                &mut c,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.world_mut().flush();
        e
    }

    fn seat(app: &mut App, item: Entity, owner: Entity, mount: Mount) {
        let se = app
            .world_mut()
            .query::<(Entity, &Socket, &SocketOwner)>()
            .iter(app.world())
            .find(|(_, s, o)| o.0 == owner && s.kind == SocketKind::Mount(mount))
            .map(|(e, _, _)| e)
            .expect("socket");
        let mut q =
            bevy::ecs::system::SystemState::<(Commands, Query<&mut Socket>)>::new(app.world_mut());
        let (mut c, mut sockets) = q.get_mut(app.world_mut()).unwrap();
        let mut s = sockets.get_mut(se).unwrap();
        crate::interact::seat_in_socket(&mut c, item, se, &mut s);
        q.apply(app.world_mut());
    }

    fn app() -> App {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    /// A PBS on an AK's muzzle: the gun is suppressed while it's on, and
    /// the can sits with its back face on the muzzle.
    #[test]
    fn a_suppressor_on_the_muzzle_quiets_the_gun() {
        let mut app = app();
        let p = Vec3::new(40.0, 40.0, 262.0);
        let ak = spawn(&mut app, "ak74", p);
        let can = spawn(&mut app, "pbs_suppressor", p);
        app.update();
        assert!(app.world().get::<Suppressed>(ak).is_none());
        seat(&mut app, can, ak, Mount::Muzzle);
        app.update();
        app.update();
        assert!(app.world().get::<Suppressed>(ak).is_some());
        let z = app.world().get::<Transform>(can).unwrap().translation.z;
        assert!((z + 0.1).abs() < 1e-4, "seated at z {z}");
        // Off again: no longer suppressed.
        let se = app.world().get::<InSocket>(can).unwrap().socket;
        app.world_mut().get_mut::<Socket>(se).unwrap().occupant = None;
        app.update();
        assert!(app.world().get::<Suppressed>(ak).is_none());
    }

    #[test]
    fn a_rail_light_on_an_ak_runs_on_its_cell() {
        let mut app = app();
        let p = Vec3::new(40.0, 40.0, 262.0);
        let ak = spawn(&mut app, "ak74", p);
        let rail = spawn(&mut app, "side_rail", p);
        let light = spawn(&mut app, "weapon_light", p);
        let cell = spawn(&mut app, "power_cell", p);
        app.update();
        seat(&mut app, rail, ak, Mount::Dovetail);
        seat(&mut app, light, rail, Mount::Rail);
        seat(&mut app, cell, light, Mount::Cell);
        app.update();
        assert_eq!(app.world().get::<MountedOn>(light), Some(&MountedOn(ak)));
        assert_eq!(app.world().get::<MountedOn>(cell), Some(&MountedOn(ak)));
        app.world_mut().write_message(UseItem {
            player: Entity::PLACEHOLDER,
            item: ak,
        });
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world().get::<Powered>(light).is_some(),
            "the light came on"
        );
        let before = app.world().get::<Item>(cell).unwrap().charge_wh;
        for _ in 0..90 {
            app.update();
        }
        let after = app.world().get::<Item>(cell).unwrap().charge_wh;
        assert!(after < before, "the cell drained ({before} -> {after})");
    }

    #[test]
    fn an_emp_grenade_stuns_a_drone() {
        let mut app = app();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(40.0, 240.0) + Vec3::Y * 3.0;
        let drone = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let d = crate::robots::drone::spawn_drone(
                &mut c,
                at,
                crate::robots::Job {
                    kind: crate::robots::JobKind::Survey,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            );
            q.apply(app.world_mut());
            d
        };
        let nade = spawn(&mut app, "emp_grenade", at - Vec3::Y * 2.0);
        app.update();
        app.world_mut().write_message(UseItem {
            player: Entity::PLACEHOLDER,
            item: nade,
        });
        app.update();
        assert!(app.world().get::<Armed>(nade).is_some(), "use armed it");
        // Short fuse, so the drone hasn't climbed away yet.
        *app.world_mut().get_mut::<Armed>(nade).unwrap() = Armed::Fuse(0.1);
        for _ in 0..30 {
            app.update();
        }
        assert!(
            app.world().get_entity(nade).is_err(),
            "the grenade went off"
        );
        assert!(
            app.world().get::<Stunned>(drone).is_some(),
            "the drone is stunned"
        );
        let y0 = app
            .world()
            .get::<GlobalTransform>(drone)
            .unwrap()
            .translation()
            .y;
        for _ in 0..90 {
            app.update();
        }
        let y1 = app
            .world()
            .get::<GlobalTransform>(drone)
            .unwrap()
            .translation()
            .y;
        assert!(y1 < y0 - 0.5, "and falls ({y0:.1} -> {y1:.1})");
    }
    #[test]
    fn a_running_cloak_hides_you_from_a_drone() {
        let mut app = app();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let at = zone.on_ground(40.0, 255.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = at;
        let drone = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let site = at + Vec3::new(0.0, 6.0, -15.0);
            let d = crate::robots::drone::spawn_drone(
                &mut c,
                site,
                crate::robots::Job {
                    kind: crate::robots::JobKind::Survey,
                    site,
                    duration: 999.0,
                    exit: site,
                },
            );
            q.apply(app.world_mut());
            d
        };
        let sees = |app: &mut App| {
            app.world()
                .get::<crate::robots::Awareness>(drone)
                .is_some_and(|a| a.can_see_target)
        };
        let mut seen = false;
        for _ in 0..90 * 3 {
            app.update();
            seen |= sees(&mut app);
        }
        assert!(seen, "in the open, the drone sees you");
        let cloak = spawn(&mut app, "cloak_generator", at + Vec3::new(0.5, 0.3, 0.0));
        let cell = spawn(&mut app, "power_cell", at);
        app.update();
        seat(&mut app, cell, cloak, Mount::Cell);
        app.world_mut().entity_mut(cloak).insert(LightOn);
        for _ in 0..30 {
            app.update();
        }
        assert!(
            !app.world().resource::<Cloaks>().0.is_empty(),
            "the cloak is running"
        );
        let mut seen_after = false;
        for _ in 0..90 * 2 {
            app.update();
            seen_after |= sees(&mut app);
        }
        assert!(!seen_after, "inside the bubble, it doesn't");
    }
}
