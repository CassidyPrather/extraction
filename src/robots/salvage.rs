//! Salvage by hand: a wreck is taken apart, not looted from a list. What
//! came through is still on it, a real item seated where it was (Heavy's
//! seats are the model: `heavy::Fastened`), and each comes off its own
//! way:
//!
//! - a **gun pod** hangs on its cables: cut them (bolt cutters,
//!   Destruction's `Cut`) and it's free;
//! - a **servo pack** at a hip whose leg came through is bolted on:
//!   undo its [`BOLTS`] bolts with a spanner (Destruction's `Turned`,
//!   `interact::tools::spanner`) and it lifts out;
//! - an **armour plate** is pried off with a crowbar (the crowbar's own
//!   plate seam, `interact::tools::crowbar`) and comes away as an item
//!   (`armour::shed_plates`);
//! - a **power cell** lifts out by hand, but not while the wreck is fresh:
//!   for [`HOT_FOR`] seconds it's live, and a hand that closes on it is
//!   thrown off by an arc.
//!
//! What a part is worth comes from what it took: its condition is its
//! health when the robot died ([`condition`]); a part shot through isn't
//! that part any more, it's scrap metal, about one for every
//! [`KG_A_SCRAP`] of it ([`scrap_for`]). Where you shot it matters.

use avian3d::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::heavy::Fastened;
use super::walker::WalkerTurret;
use super::{PartRole, Robot, RobotKind, RobotPart};
use crate::destruction::hooks::{Cuttable, Turned, Unboltable};
use crate::interact::{Hand, Socket, SocketKind, seat_in_socket};
use crate::items::{Catalog, SpawnItem, spawn_item};

/// Seconds after a robot dies that its cell is still live.
pub const HOT_FOR: f32 = 60.0;
/// The worst condition a part that came through comes off in.
pub const SCRAP: f32 = 0.05;
/// A part shot through is this much metal a piece of scrap (kg).
pub const KG_A_SCRAP: f32 = 2.0;
/// How near a closed hand has to be to a live cell to be thrown off (m).
const ARC_REACH: f32 = 0.14;
/// Cables holding a gun pod on.
pub const CABLES: usize = 2;
/// How thick a cable is to the cutters' jaws (m): a hand needn't find a
/// 12 mm wire to the millimetre.
const CABLE_REACH: f32 = 0.035;
/// Bolts holding a servo pack on, and how far each turns to come out
/// (radians: a turn).
pub const BOLTS: usize = 3;
pub const BOLT_TURN: f32 = std::f32::consts::TAU;
/// How far out of the hip the bolts' heads stand (m): proud of the hull,
/// so a spanner gets to them.
const BOLT_PROUD: f32 = 0.1;
/// Bolts holding the Sorter's servo pack on (a bigger mount).
pub const SORTER_BOLTS: usize = 4;
/// Servo packs a walker wreck gives up at most (its soundest hips).
pub const MOST_SERVOS: usize = 2;

pub struct SalvagePlugin;

impl Plugin for SalvagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Arced>().add_systems(
            Update,
            (
                fit_salvage,
                follow_wrecks,
                bolts_turned,
                holds_gone,
                cool_cells,
                live_cells,
            )
                .chain(),
        );
    }
}

/// The condition a part comes off in: its health when the robot died.
pub fn condition(health: f32) -> f32 {
    health.clamp(SCRAP, 1.0)
}

/// Pieces of scrap metal a part this heavy (kg) is, shot through.
pub fn scrap_for(mass: f32) -> usize {
    ((mass / KG_A_SCRAP).round() as usize).max(1)
}

/// Where a wreck's servo pack is (in its space), and the health of the
/// limb it drove when it fell: the Sorter's, at its shoulder
/// (`sorter::wreck_sorters`).
#[derive(Component, Clone, Copy, Debug)]
pub struct ServoAt {
    pub local: Vec3,
    pub health: f32,
}

/// A part's health when its robot died (a plate's, pried off a wreck
/// later: `armour::shed_plates` gives it as an item in this condition).
#[derive(Component, Clone, Copy, Debug)]
pub struct HealthAtDeath(pub f32);

/// A wreck's salvage is fitted (once).
#[derive(Component, Debug)]
pub struct Fitted;

/// What holds a seated part on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    /// Cables, cut one by one.
    Cables,
    /// Bolts, undone one by one.
    Bolts,
    /// Nothing but the charge in it (a cell).
    Live,
}

/// A seat on a wreck with a part in it.
#[derive(Component, Debug)]
pub struct SalvageSeat {
    pub wreck: Entity,
    pub hold: Hold,
    pub item: Entity,
}

/// One of the cables holding a part on: cut it (`Cuttable`).
#[derive(Component, Debug)]
pub struct Cable {
    pub seat: Entity,
}

/// One of the bolts holding a part on: undo it (`Unboltable`).
#[derive(Component, Debug)]
pub struct Bolt {
    pub seat: Entity,
}

/// A cable or a bolt: its own collider, carried with the wreck it's on
/// ([`follow_wrecks`]). A settled wreck's child colliders aren't kept
/// where it is, and the tools' queries would look for them where they
/// were.
#[derive(Component, Debug)]
pub struct OnWreck {
    pub wreck: Entity,
    pub local: Transform,
}

/// A cell still live: it arcs at a hand until `left` runs out.
#[derive(Component, Debug)]
pub struct Live {
    pub left: f32,
    /// To the next arc it may throw.
    pub next: f32,
}

/// A hand thrown off a live cell.
#[derive(Message, Clone, Copy, Debug)]
pub struct Arced {
    pub cell: Entity,
    pub at: Vec3,
}

/// Where a walker's cell sits in its orb (local, before its size).
const CELL_AT: Vec3 = Vec3::new(0.0, 0.3, 0.12);

/// Seats an item on a wreck (held back from hands until it's freed).
fn seat(
    commands: &mut Commands,
    catalog: &Catalog,
    wreck: Entity,
    id: &str,
    at: Transform,
    size: f32,
    health: f32,
    hold: Hold,
) -> Option<(Entity, Entity)> {
    let kind = catalog.kind(id)?;
    // Shot through: scrap, lying where it was.
    if health <= 0.0 {
        let scrap = catalog.kind("scrap_metal")?;
        commands.queue(move |world: &mut World| {
            let Some(base) = world.get::<GlobalTransform>(wreck).map(|g| *g) else {
                return;
            };
            let Some(catalog) = world.get_resource::<Catalog>().cloned() else {
                return;
            };
            let n = scrap_for(catalog.get(kind).mass);
            let at = base.transform_point(at.translation) + Vec3::Y * 0.2;
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(world);
            let mut c = q.get_mut(world).unwrap();
            for k in 0..n {
                let a = k as f32 * 2.1;
                spawn_item(
                    &mut c,
                    &catalog,
                    SpawnItem {
                        kind: scrap,
                        transform: Transform::from_translation(
                            at + Vec3::new(a.cos() * 0.15, 0.08 * k as f32, a.sin() * 0.15),
                        ),
                        condition: 1.0,
                    },
                );
            }
            q.apply(world);
        });
        return None;
    }
    let item = spawn_item(
        commands,
        catalog,
        SpawnItem {
            kind,
            transform: Transform::default(),
            condition: condition(health),
        },
    );
    let seat = commands
        .spawn((
            Name::new(format!("Salvage seat {id}")),
            at,
            Visibility::default(),
            ChildOf(wreck),
        ))
        .id();
    commands
        .entity(seat)
        .insert(SalvageSeat { wreck, hold, item });
    commands.queue(move |world: &mut World| {
        let mut socket = Socket {
            // (A mount nothing else fits: nothing seats back in it.)
            kind: SocketKind::MagWell("salvage".into()),
            occupant: None,
            radius: 0.0,
            // Items are world-sized; the orb may be scaled.
            seat: Transform::from_scale(Vec3::splat(1.0 / size.max(0.01))),
        };
        let mut q = bevy::ecs::system::SystemState::<Commands>::new(world);
        let mut c = q.get_mut(world).unwrap();
        seat_in_socket(&mut c, item, seat, &mut socket);
        q.apply(world);
        if let Ok(mut e) = world.get_entity_mut(item) {
            if let Some(g) = e.take::<crate::interact::Grabbable>() {
                e.insert(Fastened(g));
            }
        }
        if let Ok(mut e) = world.get_entity_mut(seat) {
            e.insert(socket);
        }
    });
    Some((seat, item))
}

/// Gives a seated item back to the hands.
fn free(commands: &mut Commands, item: Entity) {
    commands.queue(move |world: &mut World| {
        if let Ok(mut e) = world.get_entity_mut(item) {
            if let Some(f) = e.take::<Fastened>() {
                e.insert(f.0);
            }
        }
    });
}

/// A wreck, settled a moment, gets what came through seated on it: a
/// walker's gun pod (on its cables) and its cell (live a while).
#[allow(clippy::type_complexity)]
fn fit_salvage(
    mut commands: Commands,
    wrecks: Query<
        (
            Entity,
            &Robot,
            &Children,
            Option<&super::anim::Gait>,
            Option<&super::wreck::WreckLegs>,
            Option<&ServoAt>,
            Has<super::indoor::Crawler>,
            Has<super::hound::Hound>,
        ),
        (
            With<super::wreck::Broken>,
            Without<Fitted>,
            With<super::Salvaged>,
        ),
    >,
    parts: Query<(&RobotPart, &Transform, Has<WalkerTurret>)>,
    all_parts: Query<&RobotPart>,
    catalog: Option<Res<Catalog>>,
    // (Not in a headless app: no cable to see there.)
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(catalog) = catalog else {
        return;
    };
    for (e, robot, children, gait, legs, servo, crawler, hound) in &wrecks {
        commands.entity(e).insert(Fitted);
        // Its plates, for prying off (`armour::shed_plates`).
        for c in children.iter() {
            if let Ok((part, _, _)) = parts.get(c) {
                if matches!(part.role, PartRole::Armour(_)) {
                    commands
                        .entity(c)
                        .insert(HealthAtDeath(part.hp / part.max_hp.max(1.0)));
                }
            }
        }
        let bolt_look = match (meshes.as_mut(), materials.as_mut()) {
            (Some(m), Some(s)) => Some((
                m.add(Cylinder::new(0.014, 0.02)),
                s.add(StandardMaterial {
                    base_color: Color::srgb(0.35, 0.33, 0.3),
                    metallic: 0.8,
                    perceptual_roughness: 0.4,
                    ..default()
                }),
            )),
            _ => None,
        };
        // The Sorter's servo pack, bolted on at its shoulder.
        if let Some(sv) = servo {
            if let Some((seat_e, _)) = seat(
                &mut commands,
                &catalog,
                e,
                "sorter_servo_pack",
                Transform::from_translation(sv.local),
                1.0,
                sv.health,
                Hold::Bolts,
            ) {
                bolt_on(
                    &mut commands,
                    e,
                    seat_e,
                    sv.local + Vec3::Y * 0.2,
                    Quat::IDENTITY,
                    SORTER_BOLTS,
                    0.15,
                    &bolt_look,
                );
            }
        }
        if robot.kind != RobotKind::Walker || crawler || hound {
            continue;
        }
        let size = gait.map_or(1.0, |g| g.size);
        // The cell: the core's health is the cell's (a core shot through
        // cooked it).
        let core = all_parts
            .iter()
            .find(|p| p.robot == e && p.role == PartRole::Core)
            .map_or(0.0, |p| p.hp / p.max_hp.max(1.0));
        if let Some((_, item)) = seat(
            &mut commands,
            &catalog,
            e,
            "robot_cell",
            Transform::from_translation(CELL_AT),
            size,
            core,
            Hold::Live,
        ) {
            commands.entity(item).insert(Live {
                left: HOT_FOR,
                next: 0.0,
            });
        }
        // The servo packs at the soundest hips, bolted on.
        let shape = gait.map_or(super::anim::Shape::Orb, |g| g.shape);
        let mut sound: Vec<(usize, f32)> = legs
            .map(|l| l.0)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .filter(|(_, h)| *h > 0.0)
            .collect();
        sound.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (i, health) in sound.into_iter().take(MOST_SERVOS) {
            let hip = shape.hip(i) * size;
            let out = Vec3::new(hip.x, 0.0, hip.z).normalize_or(Vec3::X);
            // Facing out of the hip, the bolts round it.
            let face = Quat::from_rotation_arc(Vec3::Y, out);
            let Some((seat_e, _)) = seat(
                &mut commands,
                &catalog,
                e,
                "walker_leg_actuator",
                Transform::from_translation(hip).with_rotation(face),
                size,
                health,
                Hold::Bolts,
            ) else {
                continue;
            };
            bolt_on(
                &mut commands,
                e,
                seat_e,
                hip + out * BOLT_PROUD,
                face,
                BOLTS,
                0.06,
                &bolt_look,
            );
        }
        // The gun pod, where the turret was, on its cables.
        for c in children.iter() {
            let Ok((part, t, turret)) = parts.get(c) else {
                continue;
            };
            if !turret || part.role != PartRole::Weapon {
                continue;
            }
            let health = part.hp / part.max_hp.max(1.0);
            let at = *t;
            commands.entity(c).despawn();
            let Some((seat_e, _)) = seat(
                &mut commands,
                &catalog,
                e,
                "robot_gunpod",
                at,
                size,
                health,
                Hold::Cables,
            ) else {
                continue;
            };
            let look = match (meshes.as_mut(), materials.as_mut()) {
                (Some(m), Some(s)) => Some((
                    m.add(Cylinder::new(0.012, 0.22)),
                    s.add(StandardMaterial {
                        base_color: Color::srgb(0.08, 0.08, 0.09),
                        perceptual_roughness: 0.6,
                        ..default()
                    }),
                )),
                _ => None,
            };
            for k in 0..CABLES {
                // From the hull up to the pod's mount, either side.
                let side = if k == 0 { -1.0 } else { 1.0 };
                let mid = at.translation * 0.75 + Vec3::new(0.09 * side, 0.0, 0.05);
                let local =
                    Transform::from_translation(mid).with_rotation(Quat::from_rotation_x(0.5));
                let cable = commands
                    .spawn((
                        Name::new("Salvage cable"),
                        Cable { seat: seat_e },
                        OnWreck { wreck: e, local },
                        Cuttable { needs: 0.0 },
                        // (Thicker than it looks, so jaws put to it find it;
                        // found as a padlock is, a collider that meets only
                        // the world. Its own collider, not one of the
                        // wreck's: a settled body's child colliders aren't
                        // kept where the body is, and the cutters' query
                        // would look for it where it was. `follow_wrecks`
                        // carries it.)
                        Collider::capsule(CABLE_REACH, 0.2),
                        CollisionLayers::new(
                            crate::world::Layer::Item,
                            [crate::world::Layer::World],
                        ),
                        local,
                        Visibility::default(),
                    ))
                    .id();
                if let Some((mesh, mat)) = &look {
                    commands.entity(cable).insert((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(mat.clone()),
                        NotShadowCaster,
                    ));
                }
            }
        }
    }
}

/// `n` bolts in a ring of `radius` round `centre` (the wreck's space),
/// their heads facing `face`'s +Y, holding the part in `seat` on.
#[allow(clippy::too_many_arguments)]
fn bolt_on(
    commands: &mut Commands,
    wreck: Entity,
    seat: Entity,
    centre: Vec3,
    face: Quat,
    n: usize,
    radius: f32,
    look: &Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
) {
    for k in 0..n {
        let a = k as f32 * std::f32::consts::TAU / n as f32;
        let local = Transform::from_translation(
            centre + face * (Vec3::new(a.cos(), 0.0, a.sin()) * radius),
        )
        .with_rotation(face);
        let bolt = commands
            .spawn((
                Name::new("Salvage bolt"),
                Bolt { seat },
                OnWreck { wreck, local },
                Unboltable {
                    needs: BOLT_TURN,
                    axis: Vec3::Y,
                    turned: 0.0,
                },
                Collider::sphere(0.02),
                CollisionLayers::new(crate::world::Layer::Item, [crate::world::Layer::World]),
                local,
                Visibility::default(),
            ))
            .id();
        if let Some((mesh, mat)) = look {
            commands.entity(bolt).insert((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                NotShadowCaster,
            ));
        }
    }
}

/// Cables ride their wreck (dragged off, it takes them with it); a
/// wreck gone takes its cables.
fn follow_wrecks(
    mut commands: Commands,
    mut on: Query<(
        Entity,
        &OnWreck,
        &mut Transform,
        &mut Position,
        &mut Rotation,
    )>,
    wrecks: Query<&GlobalTransform>,
) {
    for (e, w, mut t, mut pos, mut rot) in &mut on {
        let Ok(gt) = wrecks.get(w.wreck) else {
            commands.entity(e).despawn();
            continue;
        };
        let at = gt.mul_transform(w.local).compute_transform();
        let at = Transform::from_translation(at.translation).with_rotation(at.rotation);
        if t.translation.distance_squared(at.translation) > 1e-8
            || t.rotation.angle_between(at.rotation) > 1e-4
        {
            *t = at;
            pos.0 = at.translation;
            rot.0 = at.rotation;
        }
    }
}

/// A bolt turned far enough is out.
fn bolts_turned(
    mut commands: Commands,
    mut turns: MessageReader<Turned>,
    mut bolts: Query<&mut Unboltable, With<Bolt>>,
) {
    for t in turns.read() {
        let Ok(mut u) = bolts.get_mut(t.target) else {
            continue;
        };
        u.turned += t.angle;
        if u.turned >= u.needs {
            commands.entity(t.target).try_despawn();
        }
    }
}

/// A part with nothing left holding it on is free: its cables cut
/// (Destruction's `cut` takes a cut thing away), its bolts out.
fn holds_gone(
    mut commands: Commands,
    seats: Query<(Entity, &SalvageSeat)>,
    cables: Query<&Cable>,
    bolts: Query<&Bolt>,
    fastened: Query<(), With<Fastened>>,
) {
    for (seat, s) in &seats {
        if !fastened.contains(s.item) {
            continue;
        }
        let held = match s.hold {
            Hold::Cables => cables.iter().any(|c| c.seat == seat),
            Hold::Bolts => bolts.iter().any(|b| b.seat == seat),
            Hold::Live => continue,
        };
        if !held {
            free(&mut commands, s.item);
        }
    }
}

/// A live cell cools: once it's dead it lifts out.
fn cool_cells(mut commands: Commands, mut cells: Query<(Entity, &mut Live)>, time: Res<Time>) {
    let dt = time.delta_secs();
    for (e, mut live) in &mut cells {
        live.left -= dt;
        live.next -= dt;
        if live.left <= 0.0 {
            commands.entity(e).remove::<Live>();
            free(&mut commands, e);
        }
    }
}

/// A hand closing on a live cell is thrown off by an arc.
fn live_cells(
    mut cells: Query<(Entity, &mut Live, &GlobalTransform)>,
    hands: Query<(&Hand, &GlobalTransform, &crate::rig::RigHand)>,
    mut arcs: MessageWriter<Arced>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
) {
    for (cell, mut live, gt) in &mut cells {
        if live.next > 0.0 {
            continue;
        }
        let at = gt.translation();
        for (hand, hgt, rig) in &hands {
            if !hand.grip_down || hgt.translation().distance(at) > ARC_REACH {
                continue;
            }
            arcs.write(Arced { cell, at });
            haptics.write(crate::rig::haptics::Haptic {
                side: rig.side,
                amplitude: 0.9,
                seconds: 0.12,
            });
            live.next = 0.5;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part comes off in the condition it was in; shot through, it's
    /// scrap metal by its weight (Progression's rate: a piece per 2 kg, at
    /// least one).
    #[test]
    fn where_you_shot_it_is_what_its_worth() {
        assert_eq!(condition(1.0), 1.0);
        assert_eq!(condition(0.6), 0.6);
        assert!(condition(0.6) > condition(0.2));
        assert_eq!(scrap_for(6.0), 3);
        assert_eq!(scrap_for(7.5), 4);
        assert_eq!(scrap_for(0.9), 1);
    }
}
