//! The heavy: the Custodian's recovery and demolition unit. A hull the size
//! of a van on four long legs, nearly four metres to its back, with one
//! manipulator arm that tears doors off and brings its claw down where you
//! stand. It comes when the Zone has been looking at you too long, or when
//! someone stands at a Gate with a key (`heavy_mind`). You should not be
//! fighting it.
//!
//! It is dismantled, not worn down (the rules are
//! `extraction_core::heavy`). The hull stops everything. What can be hurt:
//!
//! - **Bolts**: four on each of three plates (a hatch over the cell at its
//!   back, a plate down each flank over a vent). Shoot them out; on two
//!   bolts a plate hangs loose and a hand can tear it off (it's an item,
//!   seated on the hull: grab it); on none it drops.
//! - **Knees**: the actuators at its knees, in the open. One gone and it
//!   limps; two and it goes down for good.
//! - **Vents** behind the flank plates: each one lost, it runs hotter
//!   (`Heavy::heat`); past the mark it **stalls**, sinking onto its knees
//!   for ten seconds, and the cell's hatch blows its bolts to vent.
//! - **The cell** behind the hatch. Shoot it and it's dead, the cell with
//!   it. Reach in and pull it (an item, like the plates) and it's dead and
//!   the cell is yours. Standing, the cell is out of reach: it has to be
//!   down (stalled, an EMP, a snare, held in a well).
//! - The **arm** and the **eye** can be shot out like any robot's.
//!
//! Anomalies are the prepared player's tools: a **burner** heats it (and
//! cooks its bolts), a **snare** knocks it to its knees for a moment and
//! heats it, a **well** takes its feet and holds it, dragging it in, and
//! its heart crushes the knees. An **EMP** puts it down for
//! [`EMP_STUN`]: shorter than a walker, long enough to get behind it.
//!
//! The body is a walker's (`Walker` and `anim::Gait` with
//! `anim::Shape::Heavy`, scaled by [`SIZE`]): the same IK legs, footing and
//! weight. Its mind is `heavy_mind`. Its wreck is its own: it sinks onto
//! its folded legs and stays where it fell, a landmark, its loose plates
//! and its cell still there to be had.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::anomaly::{AnomalyKind, WELL_HEIGHT};
use extraction_core::heavy::{self as rules, PlateHold, Slot};

use super::anim::{Gait, Shape};
use super::walker::{Foot, LOWER, LegPoses, UPPER, Walker};
use super::{
    Awareness, Brain, Intent, Job, PartDestroyed, PartRole, Robot, RobotKind, RobotPart, Senses,
    part_bundle,
};
use crate::interact::{InteractEvent, Socket, SocketKind, physics_on, seat_in_socket};
use crate::items::{Catalog, SpawnItem, spawn_item};
use crate::world::{Layer, Zone};

/// How big it is against a walker (its legs, its reach, its hull).
pub const SIZE: f32 = 2.0;
/// An EMP holds it this long (a walker, twelve seconds).
pub const EMP_STUN: f32 = 7.0;
/// A snare's discharge drops it this long.
pub const SNARE_STUN: f32 = 2.5;
/// Held in a well: dragged toward the heart this fast, m/s, and its knees
/// crushed this hard, per second, once its hull is within the heart's
/// pull.
pub const WELL_DRAG: f32 = 0.45;
pub const WELL_CRUSH: f32 = 45.0;

/// Robot space at size 1 (the root scales it). -Z is forward.
pub const HULL_HALF: Vec3 = Vec3::new(0.40, 0.24, 0.52);
pub const HULL_AT: Vec3 = Vec3::new(0.0, 0.04, 0.0);
pub const EYE_AT: Vec3 = Vec3::new(0.0, 0.2, -0.56);
pub const SHOULDER: Vec3 = Vec3::new(0.3, 0.12, -0.5);
/// Where the claw hangs at rest.
pub const ARM_REST: Vec3 = Vec3::new(0.48, -0.9, -1.1);
/// Half extents of a plate (thin along local Z, its outward normal).
pub const PLATE_HALF: Vec3 = Vec3::new(0.27, 0.165, 0.0125);
/// The cell, behind the hatch.
pub const CELL_AT: Vec3 = Vec3::new(0.0, -0.02, 0.475);
/// (World size is twice this: [`SIZE`]; the `heavy_cell` item matches.)
pub const CELL_HALF: Vec3 = Vec3::new(0.11, 0.07, 0.045);
/// A vent, behind each flank plate.
pub const VENT_HALF: Vec3 = Vec3::new(0.03, 0.12, 0.22);
pub const BOLT_RADIUS: f32 = 0.04;
pub const KNEE_RADIUS: f32 = 0.11;

/// Plate `i`'s place: the hatch on its back, the flanks either side.
/// Local +Z faces out.
pub fn plate_transform(i: usize) -> Transform {
    match i {
        0 => Transform::from_xyz(0.0, -0.02, HULL_HALF.z + 0.02),
        1 => Transform::from_xyz(-HULL_HALF.x - 0.02, 0.0, 0.05)
            .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
        _ => Transform::from_xyz(HULL_HALF.x + 0.02, 0.0, 0.05)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
    }
}

/// Bolt `k` of plate `i`: at its corners, standing proud of it.
pub fn bolt_at(i: usize, k: usize) -> Vec3 {
    let x = if k % 2 == 0 { -1.0 } else { 1.0 } * (PLATE_HALF.x - 0.045);
    let y = if k < 2 { -1.0 } else { 1.0 } * (PLATE_HALF.y - 0.045);
    plate_transform(i).transform_point(Vec3::new(x, y, PLATE_HALF.z + BOLT_RADIUS * 0.6))
}

/// Vent `v`'s place, behind flank plate `v + 1`.
pub fn vent_at(v: usize) -> Vec3 {
    let side = if v == 0 { -1.0 } else { 1.0 };
    Vec3::new(side * (HULL_HALF.x - 0.01), 0.0, 0.05)
}

/// The heavy's own state, beside the walker body it walks on.
#[derive(Component, Debug)]
pub struct Heavy {
    /// 0 cold; it stalls at [`rules::STALL_AT`].
    pub heat: f32,
    /// Seconds left of a stall.
    pub stall: f32,
    /// Heat put into it this tick (a blow, a burner, a snare).
    pub heat_in: f32,
    /// Bolts left in each plate.
    pub bolts: [u8; rules::PLATES],
    /// Plates still on.
    pub plates_on: [bool; rules::PLATES],
    pub vents: usize,
    /// Its cell was pulled out (rather than shot).
    pub cell_pulled: bool,
    /// A well has hold of it.
    pub held_by: Option<Entity>,
    /// Seconds since it went down for good.
    pub down_for: f32,
    /// Its wreck's lean, settled once (scaled axis).
    pub lean: Vec3,
}

impl Default for Heavy {
    fn default() -> Self {
        Self {
            heat: 0.0,
            stall: 0.0,
            heat_in: 0.0,
            bolts: [rules::BOLTS_PER_PLATE as u8; rules::PLATES],
            plates_on: [true; rules::PLATES],
            vents: rules::VENTS,
            cell_pulled: false,
            held_by: None,
            down_for: 0.0,
            lean: Vec3::ZERO,
        }
    }
}

/// What a part of a heavy is.
#[derive(Component, Debug, Clone, Copy)]
pub struct HeavyPart(pub Slot);

/// A part posed from the legs each tick: along leg `leg`'s upper or lower
/// segment, or at its knee.
#[derive(Component, Debug, Clone, Copy)]
pub enum LegPiece {
    Thigh(usize),
    Shin(usize),
    Knee(usize),
}

/// A seat on the hull holding an item (a plate, the cell), and which.
#[derive(Component, Debug, Clone, Copy)]
pub struct HeavySeat {
    pub robot: Entity,
    pub slot: Slot,
}

/// What the arm is doing. The mind orders it ([`ArmOrder`]); this module
/// moves it and lands its blows.
#[derive(Component, Debug, Clone)]
pub struct Arm {
    pub state: ArmState,
    /// Seconds into the state.
    pub t: f32,
    /// The claw, world space.
    pub claw: Vec3,
    /// Where it was at the start of the state (world).
    pub from: Vec3,
    /// Where it's going (world): a blow's mark, a door's middle.
    pub mark: Vec3,
    /// A door it's tearing.
    pub door: Option<Entity>,
    pub alive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmState {
    /// Folded at rest.
    Rest,
    /// The tell: up and back over its hull, hydraulics hissing.
    Raise,
    /// Down onto the mark.
    Slam,
    /// Back to rest.
    Recover,
    /// Out to a door, gripping it.
    Reach,
    Grip,
    /// Wrenching it off and away.
    Wrench,
}

/// How long each arm state takes.
pub const RAISE_TIME: f32 = 1.2;
pub const SLAM_TIME: f32 = 0.22;
pub const RECOVER_TIME: f32 = 0.9;
pub const REACH_TIME: f32 = 0.9;
pub const GRIP_TIME: f32 = 1.4;
pub const WRENCH_TIME: f32 = 0.5;
/// How far its claw reaches from the shoulder (world, m).
pub const ARM_REACH: f32 = (UPPER + LOWER) * SIZE * 0.97;
/// A blow hurts whoever is within this of its mark, this much at the
/// middle and less at the edge.
pub const SLAM_RADIUS: f32 = 1.7;
pub const SLAM_DAMAGE: f32 = 70.0;

/// The mind's order to the arm.
#[derive(Message, Clone, Copy, Debug)]
pub enum ArmOrder {
    /// Bring the claw down on `at`.
    Strike { heavy: Entity, at: Vec3 },
    /// Tear `door` (centred at `at`) off its hinges.
    Tear {
        heavy: Entity,
        door: Entity,
        at: Vec3,
    },
}

/// Something happened to a heavy, for sound, haptics and the wrist.
#[derive(Message, Clone, Copy, Debug)]
pub struct HeavyEvent {
    pub heavy: Entity,
    pub at: Vec3,
    pub what: HeavyAct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeavyAct {
    /// A foot came down (heard far, felt near).
    Step,
    /// The arm rose: the tell before a blow.
    Raise,
    /// The claw came down (`hit`: on someone).
    Slam {
        hit: bool,
    },
    /// A door wrenched off.
    Tore,
    /// A bolt shot out; a plate went loose; a plate came off.
    Bolt,
    Loose,
    PlateOff,
    VentOut,
    /// Overheated: down on its knees, venting.
    Stall,
    /// A well has it.
    Held,
    /// The cell came out in someone's hands.
    CellPulled,
    /// It's down for good.
    Fell,
}

/// A demolition unit tears at a building (for the Destruction work: walls
/// and planks that come away). `at` is the point on the wall, `out` the
/// way it pulls.
#[derive(Message, Clone, Copy, Debug)]
pub struct HeavyTear {
    pub heavy: Entity,
    pub building: Entity,
    pub at: Vec3,
    pub out: Vec3,
}

/// A door it threw: a loose slab of the right size.
#[derive(Component, Debug, Clone, Copy)]
pub struct ThrownDoor {
    pub half: Vec3,
}

pub struct HeavyPlugin;

impl Plugin for HeavyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ArmOrder>()
            .add_message::<HeavyEvent>()
            .add_message::<HeavyTear>()
            .add_systems(
                FixedUpdate,
                (
                    heavy_parts_destroyed,
                    seats_taken,
                    heavy_anomalies,
                    heavy_heat,
                    hold_emp,
                )
                    .chain()
                    .after(super::perceive)
                    .before(super::anim::walk),
            )
            .add_systems(
                FixedUpdate,
                (pose_heavy, move_arm, fell_heavy, footfalls)
                    .chain()
                    .after(super::anim::walk)
                    .before(PhysicsSystems::First),
            );
    }
}

/// The parts a heavy is built from, in robot space at size 1:
/// (slot, role, collider, where, starts disabled).
fn parts() -> Vec<(Slot, PartRole, Collider, Transform, bool)> {
    let mut v = vec![
        (
            Slot::Hull,
            PartRole::Armour(100),
            Collider::cuboid(HULL_HALF.x * 2.0, HULL_HALF.y * 2.0, HULL_HALF.z * 2.0),
            Transform::from_translation(HULL_AT),
            false,
        ),
        (
            Slot::Eye,
            PartRole::Sensor,
            Collider::cuboid(0.24, 0.1, 0.08),
            Transform::from_translation(EYE_AT),
            false,
        ),
        (
            Slot::Cell,
            PartRole::Core,
            Collider::cuboid(CELL_HALF.x * 2.0, CELL_HALF.y * 2.0, CELL_HALF.z * 2.0),
            Transform::from_translation(CELL_AT),
            true,
        ),
        (
            Slot::Arm,
            PartRole::Weapon,
            Collider::capsule(0.07, LOWER - 0.14),
            Transform::from_translation(ARM_REST),
            false,
        ),
    ];
    for i in 0..rules::PLATES {
        v.push((
            Slot::Plate(i as u8),
            PartRole::Armour(i as u8),
            Collider::cuboid(PLATE_HALF.x * 2.0, PLATE_HALF.y * 2.0, PLATE_HALF.z * 2.0),
            plate_transform(i),
            false,
        ));
        for k in 0..rules::BOLTS_PER_PLATE {
            v.push((
                Slot::Bolt {
                    plate: i as u8,
                    bolt: k as u8,
                },
                PartRole::Armour(10 + (i * rules::BOLTS_PER_PLATE + k) as u8),
                Collider::sphere(BOLT_RADIUS),
                Transform::from_translation(bolt_at(i, k)),
                false,
            ));
        }
    }
    for vent in 0..rules::VENTS {
        v.push((
            Slot::Vent(vent as u8),
            PartRole::Armour(30 + vent as u8),
            Collider::cuboid(VENT_HALF.x * 2.0, VENT_HALF.y * 2.0, VENT_HALF.z * 2.0),
            Transform::from_translation(vent_at(vent)),
            true,
        ));
    }
    v
}

/// A heavy at `at` (standing on the terrain), with `job`. `catalog` makes
/// the plates and cell it carries (items, seated on its hull).
pub fn spawn_heavy(
    commands: &mut Commands,
    zone: &Zone,
    catalog: &Catalog,
    at: Vec3,
    job: Job,
) -> Entity {
    let shape = Shape::Heavy;
    let pos = Vec3::new(at.x, zone.ground(at.x, at.z) + shape.height() * SIZE, at.z);
    let feet = std::array::from_fn(|i| {
        let p = pos + shape.rest(i) * SIZE;
        let f = Vec3::new(p.x, zone.ground(p.x, p.z), p.z);
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
            Name::new("Heavy"),
            Robot {
                kind: RobotKind::Walker,
                wrecked: false,
            },
            super::Breed("heavy"),
            Heavy::default(),
            Walker {
                feet,
                path: Vec::new(),
                repath: 0.0,
                velocity: Vec3::ZERO,
                yaw: 0.0,
                gun_cooldown: 0.0,
                turret_yaw: 0.0,
                phase: 0,
                collapsed: false,
                goal: None,
                aim: 0.0,
                cover: None,
                rethink: 0.0,
                waited: 0.0,
            },
            {
                let mut g = Gait::sized(SIZE);
                g.shape = shape;
                g
            },
            Intent::default(),
            LegPoses::default(),
            // A big slow eye up front and good ears: it sees you across a
            // field, but a wall is a wall.
            Senses {
                sight: 60.0,
                fov_deg: 150.0,
                hearing: 1.5,
                eye: EYE_AT,
            },
            Awareness::default(),
            Brain::Transit { to: job.site },
            job,
            Transform::from_translation(pos).with_scale(Vec3::splat(SIZE)),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    commands.entity(robot).insert(Arm {
        state: ArmState::Rest,
        t: 0.0,
        claw: pos + ARM_REST * SIZE,
        from: pos + ARM_REST * SIZE,
        mark: pos,
        door: None,
        alive: true,
    });
    commands.entity(robot).with_children(|c| {
        for (slot, role, collider, at, disabled) in parts() {
            let mut e = c.spawn((
                Name::new(format!("Heavy {slot:?}")),
                HeavyPart(slot),
                part_bundle(robot, role, slot.hp(), collider, at),
            ));
            if disabled {
                e.insert(ColliderDisabled);
            }
        }
        for i in 0..4 {
            for piece in [LegPiece::Thigh(i), LegPiece::Shin(i)] {
                let (r, len) = match piece {
                    LegPiece::Thigh(_) => (0.075, UPPER),
                    _ => (0.065, LOWER),
                };
                c.spawn((
                    Name::new(format!("Heavy {piece:?}")),
                    piece,
                    HeavyPart(Slot::Hull),
                    part_bundle(
                        robot,
                        PartRole::Armour(100),
                        rules::HARDENED,
                        Collider::capsule(r, len - 2.0 * r),
                        Transform::IDENTITY,
                    ),
                ));
            }
            c.spawn((
                Name::new(format!("Heavy knee {i}")),
                LegPiece::Knee(i),
                HeavyPart(Slot::Knee(i as u8)),
                part_bundle(
                    robot,
                    PartRole::Leg(i as u8),
                    Slot::Knee(i as u8).hp(),
                    Collider::sphere(KNEE_RADIUS),
                    Transform::IDENTITY,
                ),
            ));
        }
    });
    // The plates and the cell are items, seated on the hull: a hand can
    // take them once they're loose.
    for (slot, id, at) in [
        (Slot::Plate(0), "heavy_plate", plate_transform(0)),
        (Slot::Plate(1), "heavy_plate", plate_transform(1)),
        (Slot::Plate(2), "heavy_plate", plate_transform(2)),
        (
            Slot::Cell,
            "heavy_cell",
            Transform::from_translation(CELL_AT),
        ),
    ] {
        let Some(kind) = catalog.kind(id) else {
            continue;
        };
        let seat = commands
            .spawn((
                Name::new(format!("Heavy seat {slot:?}")),
                HeavySeat { robot, slot },
                at,
                Visibility::default(),
                ChildOf(robot),
            ))
            .id();
        let item = spawn_item(
            commands,
            catalog,
            SpawnItem {
                kind,
                transform: Transform::default(),
                condition: 1.0,
            },
        );
        commands.queue(move |world: &mut World| {
            let mut socket = Socket {
                // (A mount nothing else fits: nothing seats back in it.)
                kind: SocketKind::MagWell("heavy".into()),
                occupant: None,
                radius: 0.0,
                // Items are world-sized; the hull is scaled.
                seat: Transform::from_scale(Vec3::splat(1.0 / SIZE)),
            };
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(world);
            let mut c = q.get_mut(world).unwrap();
            seat_in_socket(&mut c, item, seat, &mut socket);
            q.apply(world);
            // Not to be taken until it's loose.
            if let Ok(mut e) = world.get_entity_mut(item) {
                if let Some(g) = e.take::<crate::interact::Grabbable>() {
                    e.insert(Fastened(g));
                }
            }
            if let Ok(mut e) = world.get_entity_mut(seat) {
                e.insert(socket);
            }
        });
    }
    robot
}

/// An item seated on a heavy that a hand can't take yet: it keeps its
/// grab points here until it comes loose.
#[derive(Component, Debug)]
pub struct Fastened(pub crate::interact::Grabbable);

/// The seat holding `slot` on `robot`, and its item.
fn seat_of(
    seats: &Query<(Entity, &HeavySeat, &Socket)>,
    robot: Entity,
    slot: Slot,
) -> Option<(Entity, Option<Entity>)> {
    seats
        .iter()
        .find(|(_, s, _)| s.robot == robot && s.slot == slot)
        .map(|(e, _, s)| (e, s.occupant))
}

/// Lets a seated item be taken by hand.
fn loosen(commands: &mut Commands, fastened: &Query<&Fastened>, item: Entity) {
    let Ok(f) = fastened.get(item) else {
        return;
    };
    commands
        .entity(item)
        .remove::<Fastened>()
        .insert(f.0.clone());
}

/// Bolts shot out, plates loosened and dropped, vents lost, the cell shot
/// through, the arm shot off.
#[allow(clippy::too_many_arguments)]
fn heavy_parts_destroyed(
    mut commands: Commands,
    mut events: MessageReader<PartDestroyed>,
    mut heavies: Query<(&mut Heavy, &mut Arm, &Children, &Transform)>,
    parts: Query<(Entity, &HeavyPart, &GlobalTransform)>,
    seats: Query<(Entity, &HeavySeat, &Socket)>,
    fastened: Query<&Fastened>,
    mut out: MessageWriter<HeavyEvent>,
) {
    for ev in events.read() {
        let Ok((mut h, mut arm, children, t)) = heavies.get_mut(ev.robot) else {
            continue;
        };
        let Ok((_, part, _)) = parts.get(ev.part) else {
            continue;
        };
        match part.0 {
            Slot::Bolt { plate, .. } => {
                let p = plate as usize;
                commands.entity(ev.part).despawn();
                if !h.plates_on[p] {
                    continue;
                }
                h.bolts[p] = h.bolts[p].saturating_sub(1);
                out.write(HeavyEvent {
                    heavy: ev.robot,
                    at: ev.at,
                    what: HeavyAct::Bolt,
                });
                match rules::plate_hold(h.bolts[p] as usize) {
                    PlateHold::Fast => {}
                    PlateHold::Loose => {
                        // (Once, as it goes from fast to loose.)
                        if h.bolts[p] as usize != rules::PRY_AT {
                            continue;
                        }
                        if let Some((_, Some(item))) = seat_of(&seats, ev.robot, Slot::Plate(plate))
                        {
                            loosen(&mut commands, &fastened, item);
                            out.write(HeavyEvent {
                                heavy: ev.robot,
                                at: ev.at,
                                what: HeavyAct::Loose,
                            });
                        }
                    }
                    PlateHold::Off => {
                        drop_plate(
                            &mut commands,
                            &mut h,
                            p,
                            ev.robot,
                            t,
                            children,
                            &parts,
                            &seats,
                            &fastened,
                            true,
                        );
                        out.write(HeavyEvent {
                            heavy: ev.robot,
                            at: ev.at,
                            what: HeavyAct::PlateOff,
                        });
                    }
                }
            }
            Slot::Vent(_) => {
                h.vents = h.vents.saturating_sub(1);
                out.write(HeavyEvent {
                    heavy: ev.robot,
                    at: ev.at,
                    what: HeavyAct::VentOut,
                });
            }
            Slot::Cell => {
                // Shot through: the cell's gone with it.
                if let Some((_, Some(item))) = seat_of(&seats, ev.robot, Slot::Cell) {
                    commands.entity(item).despawn();
                }
            }
            Slot::Arm => {
                arm.alive = false;
                arm.state = ArmState::Rest;
                arm.door = None;
            }
            _ => {}
        }
    }
}

/// Takes plate `p` off: its seated item drops loose (`thrown`: flung off
/// as by its last bolt; else it's in a hand already), its armour part goes,
/// its bolts with it, and what was behind it is open.
#[allow(clippy::too_many_arguments)]
fn drop_plate(
    commands: &mut Commands,
    h: &mut Heavy,
    p: usize,
    robot: Entity,
    t: &Transform,
    children: &Children,
    parts: &Query<(Entity, &HeavyPart, &GlobalTransform)>,
    seats: &Query<(Entity, &HeavySeat, &Socket)>,
    fastened: &Query<&Fastened>,
    thrown: bool,
) {
    if !h.plates_on[p] {
        return;
    }
    h.plates_on[p] = false;
    h.bolts[p] = 0;
    for c in children.iter() {
        let Ok((e, part, _)) = parts.get(c) else {
            continue;
        };
        match part.0 {
            Slot::Plate(i) | Slot::Bolt { plate: i, .. } if i as usize == p => {
                commands.entity(e).despawn();
            }
            s if s.behind() == Some(p) => {
                commands.entity(e).remove::<ColliderDisabled>();
            }
            _ => {}
        }
    }
    // The hatch off, the cell's there for a hand.
    if p == rules::HATCH {
        if let Some((_, Some(cell))) = seat_of(seats, robot, Slot::Cell) {
            loosen(commands, fastened, cell);
        }
    }
    let Some((seat, Some(item))) = seat_of(seats, robot, Slot::Plate(p as u8)) else {
        return;
    };
    // (Off the hull, it's not in its seat any more, however it went.)
    commands.queue(move |w: &mut World| {
        if let Some(mut s) = w.get_mut::<Socket>(seat) {
            if s.occupant == Some(item) {
                s.occupant = None;
            }
        }
    });
    if !thrown {
        return;
    }
    let local = plate_transform(p);
    let world = Transform::from_translation(t.transform_point(local.translation))
        .with_rotation(t.rotation * local.rotation);
    let outward = world.rotation * Vec3::Z;
    commands.entity(seat).insert(Emptied);
    physics_on(
        commands,
        item,
        world,
        outward * 1.4 + Vec3::Y * 0.5,
        outward.cross(Vec3::Y) * 1.5,
        true,
    );
    loosen(commands, fastened, item);
}

/// A seat whose item has been taken or has dropped.
#[derive(Component, Debug)]
pub struct Emptied;

/// Things taken off a heavy by hand: a loose plate torn away, the cell
/// pulled. Also hands back grab points to whatever came off it.
#[allow(clippy::too_many_arguments)]
fn seats_taken(
    mut commands: Commands,
    mut interact: MessageReader<InteractEvent>,
    seats: Query<(Entity, &HeavySeat, &Socket)>,
    mut heavies: Query<(&mut Heavy, &mut Robot, &Children, &Transform)>,
    parts: Query<(Entity, &HeavyPart, &GlobalTransform)>,
    fastened: Query<&Fastened>,
    mut out: MessageWriter<HeavyEvent>,
) {
    for ev in interact.read() {
        let InteractEvent::Unsocketed { item: _, socket } = ev else {
            continue;
        };
        let Ok((seat, hs, _)) = seats.get(*socket) else {
            continue;
        };
        let Ok((mut h, mut robot, children, t)) = heavies.get_mut(hs.robot) else {
            continue;
        };
        commands.entity(seat).insert(Emptied);
        let at = t.transform_point(match hs.slot {
            Slot::Plate(p) => plate_transform(p as usize).translation,
            _ => CELL_AT,
        });
        match hs.slot {
            Slot::Plate(p) => {
                drop_plate(
                    &mut commands,
                    &mut h,
                    p as usize,
                    hs.robot,
                    t,
                    children,
                    &parts,
                    &seats,
                    &fastened,
                    false,
                );
                out.write(HeavyEvent {
                    heavy: hs.robot,
                    at,
                    what: HeavyAct::PlateOff,
                });
            }
            Slot::Cell => {
                h.cell_pulled = true;
                if !robot.wrecked {
                    robot.wrecked = true;
                }
                // The cell part goes with it: nothing left to shoot.
                for c in children.iter() {
                    if let Ok((pe, part, _)) = parts.get(c) {
                        if matches!(part.0, Slot::Cell) {
                            commands.entity(pe).despawn();
                        }
                    }
                }
                out.write(HeavyEvent {
                    heavy: hs.robot,
                    at,
                    what: HeavyAct::CellPulled,
                });
            }
            _ => {}
        }
    }
}

/// Anomalies on a heavy: a burner heats it, a snare's discharge drops and
/// heats it, a well takes its feet and drags it in, and the heart crushes
/// its knees.
#[allow(clippy::too_many_arguments)]
fn heavy_anomalies(
    mut commands: Commands,
    mut heavies: Query<(
        Entity,
        &mut Heavy,
        &mut Walker,
        &Transform,
        &Robot,
        &Children,
        Option<&super::Stunned>,
    )>,
    anomalies: Query<(Entity, &crate::anomaly::Anomaly, &GlobalTransform)>,
    mut acts: MessageReader<crate::anomaly::AnomalyEvent>,
    knees: Query<(Entity, &RobotPart, &GlobalTransform), With<HeavyPart>>,
    mut hits: MessageWriter<crate::weapons::ballistics::Hit>,
    mut out: MessageWriter<HeavyEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let discharges: Vec<(Vec3, f32)> = acts
        .read()
        .filter(|a| a.what == crate::anomaly::AnomalyAct::Discharge)
        .filter_map(|a| {
            anomalies
                .get(a.anomaly)
                .ok()
                .map(|(_, an, _)| (a.at, an.radius))
        })
        .collect();
    for (e, mut h, mut w, t, robot, children, stunned) in &mut heavies {
        if robot.wrecked {
            h.held_by = None;
            continue;
        }
        let pos = t.translation;
        let feet: Vec<Vec3> = w
            .feet
            .iter()
            .filter(|f| f.alive)
            .map(|f| f.planted)
            .collect();
        let near = |c: Vec3, r: f32| {
            feet.iter()
                .chain(std::iter::once(&pos))
                .any(|p| Vec2::new(p.x - c.x, p.z - c.z).length() < r)
        };
        let mut held = None;
        for (ae, a, gt) in &anomalies {
            let c = gt.translation();
            match a.kind {
                AnomalyKind::Burner if near(c, a.radius) => {
                    h.heat_in += rules::HEAT_BURNER * dt;
                }
                AnomalyKind::Well if near(c, a.kind.reach(a.radius)) => {
                    held = Some((ae, c, a.radius));
                }
                _ => {}
            }
        }
        for (at, r) in &discharges {
            if near(*at, r + 0.6) {
                h.heat_in += rules::HEAT_SNARE;
                let left = stunned.map_or(0.0, |s| s.0);
                commands
                    .entity(e)
                    .insert(super::Stunned(left.max(SNARE_STUN)));
            }
        }
        match held {
            Some((well, c, radius)) => {
                if h.held_by != Some(well) {
                    out.write(HeavyEvent {
                        heavy: e,
                        at: pos,
                        what: HeavyAct::Held,
                    });
                }
                h.held_by = Some(well);
                // Down on its knees, dragged in.
                let left = stunned.map_or(0.0, |s| s.0);
                commands.entity(e).insert(super::Stunned(left.max(0.5)));
                let to = Vec3::new(c.x - pos.x, 0.0, c.z - pos.z);
                w.velocity = to.normalize_or_zero() * WELL_DRAG * (to.length() / 0.5).min(1.0);
                // Its knees in the heart's grip.
                let heart = c + Vec3::Y * WELL_HEIGHT;
                for k in children.iter() {
                    let Ok((ke, part, kgt)) = knees.get(k) else {
                        continue;
                    };
                    if !matches!(part.role, PartRole::Leg(_)) || !part.alive() {
                        continue;
                    }
                    let d = Vec2::new(kgt.translation().x - heart.x, kgt.translation().z - heart.z)
                        .length();
                    if d < radius * 1.5 + KNEE_RADIUS * SIZE {
                        hits.write(crate::weapons::ballistics::Hit {
                            entity: ke,
                            point: kgt.translation(),
                            normal: Vec3::Y,
                            velocity: Vec3::ZERO,
                            damage: WELL_CRUSH * dt,
                            shooter: None,
                            mass_g: 0.0,
                            load: Default::default(),
                        });
                    }
                }
            }
            None => h.held_by = None,
        }
    }
}

/// Heat: made by walking, blows and anomalies, shed through the vents.
/// Past the mark it stalls on its knees, and the hatch over its cell blows
/// its bolts to vent.
#[allow(clippy::too_many_arguments)]
fn heavy_heat(
    mut commands: Commands,
    mut heavies: Query<(
        Entity,
        &mut Heavy,
        &Walker,
        &Gait,
        &Robot,
        &Transform,
        &Children,
        Option<&super::Stunned>,
    )>,
    parts: Query<(Entity, &HeavyPart, &GlobalTransform)>,
    seats: Query<(Entity, &HeavySeat, &Socket)>,
    fastened: Query<&Fastened>,
    mut out: MessageWriter<HeavyEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut h, w, g, robot, t, children, stunned) in &mut heavies {
        if robot.wrecked {
            continue;
        }
        let pace = w.velocity.length() / g.top_speed().max(0.1);
        let external = std::mem::take(&mut h.heat_in);
        h.heat = rules::heat_step(h.heat, dt, pace, external, h.vents);
        h.stall = (h.stall - dt).max(0.0);
        if h.heat >= rules::STALL_AT && h.stall <= 0.0 {
            h.stall = rules::STALL_TIME;
            h.heat = rules::AFTER_STALL;
            let left = stunned.map_or(0.0, |s| s.0);
            commands
                .entity(e)
                .insert(super::Stunned(left.max(rules::STALL_TIME)));
            out.write(HeavyEvent {
                heavy: e,
                at: t.translation,
                what: HeavyAct::Stall,
            });
            // The hatch blows its bolts to vent: the cell's there to be had.
            if h.plates_on[rules::HATCH] {
                for c in children.iter() {
                    if let Ok((pe, part, _)) = parts.get(c) {
                        if matches!(part.0, Slot::Bolt { plate: 0, .. }) {
                            commands.entity(pe).despawn();
                        }
                    }
                }
                drop_plate(
                    &mut commands,
                    &mut h,
                    rules::HATCH,
                    e,
                    t,
                    children,
                    &parts,
                    &seats,
                    &fastened,
                    true,
                );
                out.write(HeavyEvent {
                    heavy: e,
                    at: t.transform_point(plate_transform(0).translation),
                    what: HeavyAct::PlateOff,
                });
            }
        }
    }
}

/// An EMP holds a heavy for [`EMP_STUN`], not a walker's twelve seconds
/// (unless it's down for its own reasons: a stall, a well).
fn hold_emp(mut heavies: Query<(&Heavy, &mut super::Stunned), Added<super::Stunned>>) {
    for (h, mut s) in &mut heavies {
        if h.stall <= 0.0 && h.held_by.is_none() && s.0 > EMP_STUN {
            s.0 = EMP_STUN;
        }
    }
}

/// Its leg armour and knees follow the legs; the gait learns how its knees
/// are; the arm's collider follows the arm.
#[allow(clippy::type_complexity)]
fn pose_heavy(
    mut heavies: Query<(&Transform, &LegPoses, &mut Gait, &Arm, &Children), With<Heavy>>,
    mut pieces: Query<(&mut Transform, Option<&LegPiece>, &RobotPart, &HeavyPart), Without<Heavy>>,
) {
    for (t, poses, mut g, arm, children) in &mut heavies {
        let inv = t.compute_affine().inverse();
        let inv_rot = t.rotation.inverse();
        let shoulder = t.transform_point(SHOULDER);
        let elbow = arm_elbow(t, shoulder, arm.claw);
        for c in children.iter() {
            let Ok((mut pt, piece, part, hp)) = pieces.get_mut(c) else {
                continue;
            };
            let seg = |a: Vec3, b: Vec3| {
                let dir = (b - a).normalize_or(Vec3::Y);
                Transform::from_translation(inv.transform_point3((a + b) * 0.5))
                    .with_rotation(inv_rot * Quat::from_rotation_arc(Vec3::Y, dir))
            };
            match piece {
                Some(LegPiece::Thigh(i)) => {
                    let [hip, knee, _] = poses.0[*i];
                    *pt = seg(hip, knee);
                }
                Some(LegPiece::Shin(i)) => {
                    let [_, knee, foot] = poses.0[*i];
                    *pt = seg(knee, foot);
                }
                Some(LegPiece::Knee(i)) => {
                    let [hip, knee, foot] = poses.0[*i];
                    // Its drum across the knee's hinge.
                    let hinge = (knee - hip).cross(foot - knee).normalize_or(Vec3::X);
                    *pt = Transform::from_translation(inv.transform_point3(knee))
                        .with_rotation(inv_rot * Quat::from_rotation_arc(Vec3::X, hinge));
                    g.leg_health[*i] = (part.hp / part.max_hp).clamp(0.0, 1.0);
                }
                None if matches!(hp.0, Slot::Arm) => {
                    *pt = seg(elbow, arm.claw);
                }
                None => {}
            }
        }
    }
}

/// The elbow of an arm from `shoulder` to `claw`: out, forward and a
/// little up, so at rest it folds in front of the hull like a mantis's.
pub fn arm_elbow(t: &Transform, shoulder: Vec3, claw: Vec3) -> Vec3 {
    let out = t.rotation * Vec3::new(0.8, 0.3, -1.0);
    super::anim::solve_knee_sized(shoulder, claw, out, SIZE)
}

/// Where the claw rests, in the world.
pub fn rest_claw(t: &Transform) -> Vec3 {
    t.transform_point(ARM_REST)
}

fn smooth(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// The arm: folded at rest; ordered to strike, it rises over the hull (the
/// tell) and comes down on its mark; ordered to a door, it reaches out,
/// grips, and wrenches it off and away.
#[allow(clippy::too_many_arguments)]
fn move_arm(
    mut commands: Commands,
    mut heavies: Query<(
        Entity,
        &mut Arm,
        &mut Heavy,
        &Transform,
        &Robot,
        Has<super::Stunned>,
    )>,
    mut orders: MessageReader<ArmOrder>,
    players: Query<(Entity, &GlobalTransform), With<crate::player::Player>>,
    colliders: Query<(Entity, &ChildOf), With<crate::player::BodyCollider>>,
    doors: Query<&GlobalTransform, With<crate::world::buildings::Door>>,
    mut hits: MessageWriter<crate::weapons::ballistics::Hit>,
    mut out: MessageWriter<HeavyEvent>,
    mut noise: MessageWriter<super::Noise>,
    mut blows: MessageWriter<super::doors::DoorBlow>,
    mut impacts: MessageWriter<super::wreck::WreckImpact>,
    mut raid: ResMut<crate::raid::Raid>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for order in orders.read() {
        let (e, at, door) = match *order {
            ArmOrder::Strike { heavy, at } => (heavy, at, None),
            ArmOrder::Tear { heavy, door, at } => (heavy, at, Some(door)),
        };
        let Ok((_, mut arm, _, t, robot, stunned)) = heavies.get_mut(e) else {
            continue;
        };
        if !arm.alive || robot.wrecked || stunned || arm.state != ArmState::Rest {
            continue;
        }
        // Within reach of its shoulder, or as near as it gets.
        let shoulder = t.transform_point(SHOULDER);
        let d = at - shoulder;
        let at = shoulder + d.clamp_length_max(ARM_REACH);
        arm.from = arm.claw;
        arm.mark = at;
        arm.t = 0.0;
        arm.door = door;
        arm.state = if door.is_some() {
            ArmState::Reach
        } else {
            out.write(HeavyEvent {
                heavy: e,
                at: shoulder,
                what: HeavyAct::Raise,
            });
            ArmState::Raise
        };
    }
    for (e, mut arm, mut h, t, robot, stunned) in &mut heavies {
        let rest = rest_claw(t);
        if robot.wrecked || !arm.alive {
            // Hanging: it drops to the ground under the shoulder.
            let shoulder = t.transform_point(SHOULDER);
            let hang = shoulder + Vec3::Y * -(ARM_REACH * 0.9);
            arm.claw = arm.claw.lerp(hang, (2.0 * dt).min(1.0));
            arm.state = ArmState::Rest;
            continue;
        }
        if stunned && arm.state != ArmState::Rest {
            // Knocked out mid-blow: it sags back.
            arm.state = ArmState::Recover;
            arm.from = arm.claw;
            arm.t = 0.0;
            arm.door = None;
        }
        arm.t += dt;
        let k = arm.t;
        let state = arm.state;
        match state {
            ArmState::Rest => {
                arm.claw = arm.claw.lerp(rest, (6.0 * dt).min(1.0));
            }
            ArmState::Raise => {
                // Up and back over its hull.
                let top = t.transform_point(Vec3::new(0.35, 0.95, -0.25));
                arm.claw = arm.from.lerp(top, smooth(k / RAISE_TIME));
                if k >= RAISE_TIME {
                    arm.state = ArmState::Slam;
                    arm.from = arm.claw;
                    arm.t = 0.0;
                }
            }
            ArmState::Slam => {
                // Fast, accelerating down.
                let s = (k / SLAM_TIME).min(1.0);
                arm.claw = arm.from.lerp(arm.mark, s * s);
                if k >= SLAM_TIME {
                    let mark = arm.mark;
                    let mut hit = false;
                    for (pe, pgt) in &players {
                        let p = pgt.translation();
                        let flat = Vec2::new(p.x - mark.x, p.z - mark.z).length();
                        if flat > SLAM_RADIUS || (p.y - mark.y).abs() > 2.2 {
                            continue;
                        }
                        let Some((body, _)) = colliders.iter().find(|(_, c)| c.parent() == pe)
                        else {
                            continue;
                        };
                        hit = true;
                        let damage = SLAM_DAMAGE * (1.0 - 0.5 * flat / SLAM_RADIUS);
                        hits.write(crate::weapons::ballistics::Hit {
                            entity: body,
                            point: p + Vec3::Y * 1.0,
                            normal: Vec3::Y,
                            velocity: Vec3::NEG_Y * 12.0,
                            damage,
                            shooter: Some(e),
                            mass_g: 0.0,
                            load: Default::default(),
                        });
                    }
                    h.heat_in += rules::HEAT_BLOW;
                    noise.write(super::Noise {
                        at: mark,
                        radius: 70.0,
                        source: Some(e),
                    });
                    impacts.write(super::wreck::WreckImpact {
                        at: mark,
                        size: 1.2,
                    });
                    out.write(HeavyEvent {
                        heavy: e,
                        at: mark,
                        what: HeavyAct::Slam { hit },
                    });
                    arm.state = ArmState::Recover;
                    arm.from = arm.claw;
                    arm.t = 0.0;
                }
            }
            ArmState::Recover => {
                arm.claw = arm.from.lerp(rest, smooth(k / RECOVER_TIME));
                if k >= RECOVER_TIME {
                    arm.state = ArmState::Rest;
                    arm.t = 0.0;
                }
            }
            ArmState::Reach => {
                arm.claw = arm.from.lerp(arm.mark, smooth(k / REACH_TIME));
                if k >= REACH_TIME {
                    arm.state = ArmState::Grip;
                    arm.t = 0.0;
                }
            }
            ArmState::Grip => {
                // Straining: the claw shakes.
                let j = (k * 31.0).sin() * 0.03 + (k * 17.0).cos() * 0.02;
                arm.claw = arm.mark + Vec3::new(j, j * 0.5, -j);
                if (k / 0.5).floor() != ((k - dt) / 0.5).floor() {
                    blows.write(super::doors::DoorBlow {
                        at: arm.mark,
                        broke: false,
                    });
                }
                if k >= GRIP_TIME {
                    arm.state = ArmState::Wrench;
                    arm.from = arm.claw;
                    arm.t = 0.0;
                    if let Some(door) = arm.door.take() {
                        let half = Vec3::new(0.45, 1.0, 0.03);
                        let at = doors.get(door).map(|g| g.translation()).unwrap_or(arm.mark);
                        if let Ok(mut ec) = commands.get_entity(door) {
                            ec.despawn();
                        }
                        let away = (rest - arm.mark).with_y(0.0).normalize_or(Vec3::X);
                        let side = away.cross(Vec3::Y);
                        commands.spawn((
                            Name::new("Thrown door"),
                            ThrownDoor { half },
                            Transform::from_translation(at),
                            RigidBody::Dynamic,
                            Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
                            Mass(35.0),
                            CollisionLayers::new(Layer::Item, [Layer::World, Layer::Item]),
                            LinearVelocity(side * 5.0 + away * 2.0 + Vec3::Y * 2.5),
                            AngularVelocity(Vec3::new(1.0, 4.0, 0.5)),
                            Visibility::default(),
                        ));
                        blows.write(super::doors::DoorBlow { at, broke: true });
                        noise.write(super::Noise {
                            at,
                            radius: 60.0,
                            source: Some(e),
                        });
                        out.write(HeavyEvent {
                            heavy: e,
                            at,
                            what: HeavyAct::Tore,
                        });
                        raid.say("A door tears off its hinges with a scream of metal.");
                    }
                }
            }
            ArmState::Wrench => {
                arm.claw = arm.from.lerp(rest, smooth(k / WRENCH_TIME));
                if k >= WRENCH_TIME {
                    arm.state = ArmState::Rest;
                    arm.t = 0.0;
                }
            }
        }
    }
}

/// Down for good: it sinks onto its folded legs (the walker body does
/// that: down, it sags on them), leans as it settles, and stays, a
/// landmark. Its plates come loose in the fall, so a hand can have them
/// and the cell behind the hatch; knees and claw still whole drop as
/// salvage. (Not the walker's wreck: nothing comes apart, nothing is
/// despawned to keep the debris count down.)
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fell_heavy(
    mut commands: Commands,
    mut heavies: Query<(
        Entity,
        &mut Heavy,
        &mut Transform,
        &Robot,
        &Arm,
        &Children,
        Has<super::wreck::Broken>,
    )>,
    parts: Query<(&RobotPart, &HeavyPart, &GlobalTransform)>,
    seats: Query<(Entity, &HeavySeat, &Socket)>,
    fastened: Query<&Fastened>,
    catalog: Res<Catalog>,
    mut impacts: MessageWriter<super::wreck::WreckImpact>,
    mut out: MessageWriter<HeavyEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut h, mut t, robot, arm, children, broken) in &mut heavies {
        if !robot.wrecked {
            continue;
        }
        if !broken {
            commands
                .entity(e)
                .insert((super::wreck::Broken, super::Salvaged));
            let seed = e.index_u32() as f32;
            // Forward onto its face or over on a side: seeded, so no two
            // lie the same.
            // (Well over: a wreck, not something parked.)
            h.lean = Vec3::new(
                0.3 + 0.08 * seed.sin(),
                0.0,
                0.22 * (seed * 1.7).cos().signum(),
            ) + Vec3::new(0.0, 0.0, 0.06 * (seed * 2.3).sin());
            out.write(HeavyEvent {
                heavy: e,
                at: t.translation,
                what: HeavyAct::Fell,
            });
            // What's still on it comes loose in the fall.
            for (_, s, sock) in &seats {
                // (The cell stays behind its hatch, if that's still on.)
                if s.robot != e || (s.slot == Slot::Cell && h.plates_on[rules::HATCH]) {
                    continue;
                }
                if let Some(item) = sock.occupant {
                    loosen(&mut commands, &fastened, item);
                }
            }
            // Whole knees and a whole claw, for the taking.
            let mut drops: Vec<(&str, Vec3)> = Vec::new();
            for c in children.iter() {
                let Ok((part, hp, gt)) = parts.get(c) else {
                    continue;
                };
                match hp.0 {
                    Slot::Knee(_)
                        if part.alive()
                            && drops.iter().filter(|d| d.0 == "heavy_knee").count() < 2 =>
                    {
                        drops.push(("heavy_knee", gt.translation()));
                    }
                    Slot::Arm if part.alive() && arm.alive => {
                        drops.push(("heavy_claw", arm.claw));
                    }
                    _ => {}
                }
            }
            for (id, at) in drops {
                if let Some(kind) = catalog.kind(id) {
                    let ground = at.y.min(t.translation.y);
                    spawn_item(
                        &mut commands,
                        &catalog,
                        SpawnItem {
                            kind,
                            transform: Transform::from_translation(Vec3::new(
                                at.x,
                                ground.max(at.y - 1.0),
                                at.z,
                            )),
                            condition: 0.6,
                        },
                    );
                }
            }
        }
        let before = h.down_for;
        h.down_for += dt;
        if before < 1.1 && h.down_for >= 1.1 {
            impacts.write(super::wreck::WreckImpact {
                at: t.translation,
                size: SIZE * 1.5,
            });
        }
        // The lean comes on as it sinks (the gait rebuilds its rotation
        // each tick; this goes on top).
        let k = smooth(h.down_for / 1.6);
        t.rotation = Quat::from_scaled_axis(t.rotation * (h.lean * k)) * t.rotation;
    }
}

/// Which walker footfalls are a heavy's: [`HeavyAct::Step`] for its
/// sound and the shake in your hands.
fn footfalls(
    mut steps: MessageReader<super::walker::WalkerStep>,
    heavies: Query<(Entity, &Walker), With<Heavy>>,
    mut out: MessageWriter<HeavyEvent>,
) {
    for s in steps.read() {
        if let Some((e, _)) = heavies.iter().find(|(_, w)| {
            w.feet
                .iter()
                .any(|f| f.planted.distance_squared(s.at) < 0.01)
        }) {
            out.write(HeavyEvent {
                heavy: e,
                at: s.at,
                what: HeavyAct::Step,
            });
        }
    }
}

/// Heavies' wrecks, for the save: they stay where they fell until the
/// Surge.
pub fn save_wrecks(world: &mut World) -> Vec<extraction_core::save::WreckSave> {
    let seats: Vec<(Entity, Slot, bool)> = world
        .query::<(&HeavySeat, &Socket)>()
        .iter(world)
        .map(|(s, sock)| (s.robot, s.slot, sock.occupant.is_some()))
        .collect();
    let out = world
        .query::<(Entity, &Robot, &Heavy, &Walker, &Transform)>()
        .iter(world)
        .filter(|(_, r, ..)| r.wrecked)
        .map(|(e, _, h, w, t)| extraction_core::save::WreckSave {
            at: t.translation.to_array(),
            yaw: w.yaw,
            lean: h.lean.to_array(),
            plates: h.plates_on,
            cell: !h.cell_pulled
                && seats
                    .iter()
                    .any(|(r, s, on)| *r == e && *s == Slot::Cell && *on),
        })
        .collect();
    out
}

/// A saved wreck back where it lay: sunk on its legs, its plates and cell
/// as they were, loose for a hand.
pub fn restore_wreck(world: &mut World, saved: &extraction_core::save::WreckSave) {
    let zone = world.resource::<Zone>().clone();
    let catalog = world.resource::<Catalog>().clone();
    let at = Vec3::from_array(saved.at);
    let e = {
        let mut c = world.commands();
        spawn_heavy(
            &mut c,
            &zone,
            &catalog,
            at,
            Job {
                kind: super::JobKind::Survey,
                site: at,
                duration: 0.0,
                exit: at,
            },
        )
    };
    world.flush();
    let mut gone: Vec<Entity> = Vec::new();
    let mut opened: Vec<Entity> = Vec::new();
    let mut loose: Vec<Entity> = Vec::new();
    let mut emptied: Vec<Entity> = Vec::new();
    let children: Vec<Entity> = world
        .get::<Children>(e)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    for c in children {
        if let Some(part) = world.get::<HeavyPart>(c).map(|p| p.0) {
            match part {
                Slot::Plate(i) | Slot::Bolt { plate: i, .. } if !saved.plates[i as usize] => {
                    gone.push(c)
                }
                Slot::Cell if !saved.cell => gone.push(c),
                s if s.behind().is_some_and(|p| !saved.plates[p]) => opened.push(c),
                _ => {}
            }
        }
        if let (Some(seat), Some(sock)) = (world.get::<HeavySeat>(c), world.get::<Socket>(c)) {
            let keep = match seat.slot {
                Slot::Plate(i) => saved.plates[i as usize],
                _ => saved.cell,
            };
            if let Some(item) = sock.occupant {
                if keep {
                    // (The cell only once its hatch is off.)
                    if seat.slot != Slot::Cell || !saved.plates[rules::HATCH] {
                        loose.push(item);
                    }
                } else {
                    gone.push(item);
                    emptied.push(c);
                }
            }
        }
    }
    for g in gone {
        world.despawn(g);
    }
    for s in emptied {
        if let Some(mut sock) = world.get_mut::<Socket>(s) {
            sock.occupant = None;
        }
    }
    for o in opened {
        world.entity_mut(o).remove::<ColliderDisabled>();
    }
    for item in loose {
        let mut em = world.entity_mut(item);
        if let Some(f) = em.take::<Fastened>() {
            em.insert(f.0);
        }
    }
    let mut em = world.entity_mut(e);
    em.insert((super::wreck::Broken, super::Salvaged));
    em.get_mut::<Robot>().unwrap().wrecked = true;
    em.get_mut::<Walker>().unwrap().yaw = saved.yaw;
    let mut h = em.get_mut::<Heavy>().unwrap();
    h.plates_on = saved.plates;
    h.bolts = saved.plates.map(|on| if on { 2 } else { 0 });
    h.cell_pulled = !saved.cell;
    h.lean = Vec3::from_array(saved.lean);
    // Long since settled: no second fall, no second thump.
    h.down_for = 60.0;
}

/// The Surge sweeps heavies' wrecks away with everything else.
pub fn clear_wrecks(world: &mut World) {
    let wrecks: Vec<Entity> = world
        .query::<(Entity, &Robot, &Heavy)>()
        .iter(world)
        .filter(|(_, r, _)| r.wrecked)
        .map(|(e, ..)| e)
        .collect();
    for e in wrecks {
        world.despawn(e);
    }
}

/// The salvage table a heavy's wreck answers to (its parts are its loot:
/// see `fell_heavy`).
pub const SALVAGE: [&str; 4] = ["heavy_cell", "heavy_plate", "heavy_knee", "heavy_claw"];

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::interact::InSocket;
    use crate::items::Item;
    use crate::weapons::ballistics::Hit;

    /// A headless world with no director, and a heavy sent `to` from `at`
    /// (ground x/z), its senses off unless `senses`.
    pub(crate) fn world_with_heavy(at: [f32; 2], to: [f32; 2], senses: bool) -> (App, Entity) {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let catalog = app.world().resource::<Catalog>().clone();
        let from = zone.on_ground(at[0], at[1]);
        let site = zone.on_ground(to[0], to[1]);
        let heavy = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = spawn_heavy(
                &mut c,
                &zone,
                &catalog,
                from,
                Job {
                    kind: super::super::JobKind::Response,
                    site,
                    duration: 999.0,
                    exit: from,
                },
            );
            if !senses {
                c.entity(e).remove::<Senses>();
            }
            q.apply(app.world_mut());
            e
        };
        (app, heavy)
    }

    pub(crate) fn part(app: &mut App, heavy: Entity, slot: Slot) -> Option<(Entity, f32)> {
        app.world_mut()
            .query::<(Entity, &RobotPart, &HeavyPart)>()
            .iter(app.world())
            .find(|(_, p, h)| p.robot == heavy && h.0 == slot)
            .map(|(e, p, _)| (e, p.hp))
    }

    pub(crate) fn hit(app: &mut App, part: Entity, damage: f32) {
        let at = app
            .world()
            .get::<GlobalTransform>(part)
            .map(|g| g.translation())
            .unwrap_or_default();
        app.world_mut().write_message(Hit {
            entity: part,
            point: at,
            normal: Vec3::Y,
            velocity: Vec3::NEG_Z * 500.0,
            damage,
            shooter: None,
            mass_g: 8.0,
            load: Default::default(),
        });
        for _ in 0..3 {
            app.update();
        }
    }

    /// The seated item for `slot`, and whether a hand could take it.
    pub(crate) fn seated(app: &mut App, heavy: Entity, slot: Slot) -> Option<(Entity, bool)> {
        let item = app
            .world_mut()
            .query::<(&HeavySeat, &Socket)>()
            .iter(app.world())
            .find(|(s, _)| s.robot == heavy && s.slot == slot)
            .and_then(|(_, s)| s.occupant)?;
        let free = app
            .world()
            .get::<crate::interact::Grabbable>(item)
            .is_some();
        Some((item, free))
    }

    /// Sent somewhere, it walks there on its four long legs, on the ground,
    /// its hull riding about two and a half metres up, every foot within
    /// reach of its hip, nothing NaN; its plates and cell sit on its hull,
    /// not to be taken.
    #[test]
    fn it_walks_where_its_sent() {
        let (mut app, heavy) = world_with_heavy([40.0, 230.0], [40.0, 205.0], false);
        let zone = app.world().resource::<Zone>().clone();
        let mut fastest = 0.0f32;
        let mut worst = 0.0f32;
        let mut steps = 0;
        for _ in 0..(90 * 30) {
            app.update();
            let w = app.world().get::<Walker>(heavy).unwrap();
            fastest = fastest.max(w.velocity.length());
            for [hip, _, foot] in app.world().get::<LegPoses>(heavy).unwrap().0 {
                worst = worst.max(hip.distance(foot));
            }
            steps += app
                .world()
                .resource::<Messages<super::super::walker::WalkerStep>>()
                .iter_current_update_messages()
                .count();
        }
        let reach = (UPPER + LOWER) * SIZE;
        assert!(worst < reach + 0.05, "a foot {worst:.2} m from its hip");
        let t = *app.world().get::<Transform>(heavy).unwrap();
        assert!(t.translation.is_finite());
        let over = t.translation.y - zone.ground(t.translation.x, t.translation.z);
        assert!(over > 2.2 && over < 3.0, "rides {over:.2} m up");
        let to = zone.on_ground(40.0, 205.0);
        assert!(
            Vec2::new(t.translation.x - to.x, t.translation.z - to.z).length() < 6.0,
            "got there: {:.1?}",
            t.translation
        );
        // Slow: you can outwalk it, but not stroll.
        assert!(fastest > 0.9 && fastest < 1.8, "{fastest:.2} m/s");
        assert!(steps > 8, "it stepped ({steps})");
        for slot in [Slot::Plate(0), Slot::Plate(1), Slot::Plate(2), Slot::Cell] {
            let (_, free) = seated(&mut app, heavy, slot).expect("seated");
            assert!(!free, "{slot:?} can't be taken yet");
        }
        // No walker plates on it.
        let walker_plates = app
            .world_mut()
            .query::<(&RobotPart, Option<&HeavyPart>)>()
            .iter(app.world())
            .filter(|(p, h)| p.robot == heavy && h.is_none())
            .count();
        assert_eq!(walker_plates, 0);
    }

    /// Rounds on the hull and plates do nothing; a covered vent can't be
    /// reached; bolts shot out loosen a plate (a hand may take it), then
    /// drop it, and what was behind it can be hurt.
    #[test]
    fn plates_come_off_at_their_bolts() {
        let (mut app, heavy) = world_with_heavy([40.0, 230.0], [40.0, 230.0], false);
        for _ in 0..60 {
            app.update();
        }
        // The hull and the plate shrug it off.
        let (hull, _) = part(&mut app, heavy, Slot::Hull).unwrap();
        hit(&mut app, hull, 500.0);
        let (plate, _) = part(&mut app, heavy, Slot::Plate(1)).unwrap();
        hit(&mut app, plate, 500.0);
        assert!(!app.world().get::<Robot>(heavy).unwrap().wrecked);
        assert!(part(&mut app, heavy, Slot::Plate(1)).is_some());
        // The vent behind it is out of reach: no collider to strike.
        let (vent, hp0) = part(&mut app, heavy, Slot::Vent(0)).unwrap();
        assert!(app.world().get::<ColliderDisabled>(vent).is_some());
        // A round fired square at the left flank meets the plate.
        let shoot_flank = |app: &mut App| {
            let t = *app.world().get::<GlobalTransform>(heavy).unwrap();
            let aim = t.transform_point(vent_at(0));
            let origin = t.transform_point(vent_at(0) + Vec3::NEG_X * 3.0);
            app.world_mut()
                .write_message(crate::weapons::ballistics::Shot {
                    gun: Entity::PLACEHOLDER,
                    shooter: None,
                    origin,
                    direction: (aim - origin).normalize(),
                    speed: 700.0,
                    damage: 70.0,
                    projectiles: 1,
                    spread: 0.0,
                    recoil: 0.0,
                    seed: 1,
                });
            for _ in 0..6 {
                app.update();
            }
        };
        shoot_flank(&mut app);
        assert_eq!(part(&mut app, heavy, Slot::Vent(0)).unwrap().1, hp0);
        // Two bolts out: loose, a hand may take it.
        for k in 0..2u8 {
            let (b, _) = part(&mut app, heavy, Slot::Bolt { plate: 1, bolt: k }).unwrap();
            hit(&mut app, b, 40.0);
            assert!(part(&mut app, heavy, Slot::Bolt { plate: 1, bolt: k }).is_none());
        }
        let (item, free) = seated(&mut app, heavy, Slot::Plate(1)).unwrap();
        assert!(free, "two bolts out: loose");
        assert!(seated(&mut app, heavy, Slot::Plate(2)).is_some_and(|(_, f)| !f));
        // The rest: it drops, a loose item on its way to the ground.
        for k in 2..4u8 {
            let (b, _) = part(&mut app, heavy, Slot::Bolt { plate: 1, bolt: k }).unwrap();
            hit(&mut app, b, 40.0);
        }
        assert!(seated(&mut app, heavy, Slot::Plate(1)).is_none());
        assert!(app.world().get::<InSocket>(item).is_none());
        assert!(
            app.world()
                .get::<crate::interact::Grabbable>(item)
                .is_some()
        );
        assert!(part(&mut app, heavy, Slot::Plate(1)).is_none());
        assert_eq!(
            app.world().get::<Heavy>(heavy).unwrap().plates_on,
            [true, false, true]
        );
        for _ in 0..60 {
            app.update();
        }
        // Now the same round finds the vent.
        assert!(app.world().get::<ColliderDisabled>(vent).is_none());
        shoot_flank(&mut app);
        assert!(
            part(&mut app, heavy, Slot::Vent(0)).unwrap().1 < hp0,
            "the vent's open"
        );
        // The cell, still behind its hatch, took nothing through all that.
        let (_, cell) = part(&mut app, heavy, Slot::Cell).unwrap();
        assert_eq!(cell, rules::CELL_HP);
    }

    /// Two knees shot out and it goes down for good: it sinks onto its
    /// legs and stays where it fell (still there a minute on), its plates
    /// loose for the taking and its whole knees and claw dropped as
    /// salvage. It isn't the walker's wreck: nothing is despawned.
    #[test]
    fn it_falls_and_stays() {
        let (mut app, heavy) = world_with_heavy([40.0, 230.0], [40.0, 230.0], false);
        let zone = app.world().resource::<Zone>().clone();
        for _ in 0..60 {
            app.update();
        }
        let (k0, _) = part(&mut app, heavy, Slot::Knee(0)).unwrap();
        hit(&mut app, k0, 400.0);
        assert!(
            !app.world().get::<Robot>(heavy).unwrap().wrecked,
            "one knee: it limps"
        );
        let (k2, _) = part(&mut app, heavy, Slot::Knee(2)).unwrap();
        hit(&mut app, k2, 400.0);
        assert!(
            app.world().get::<Robot>(heavy).unwrap().wrecked,
            "two: it's down"
        );
        for _ in 0..(90 * 60) {
            app.update();
        }
        let t = *app.world().get::<Transform>(heavy).expect("still there");
        assert!(t.translation.is_finite());
        let over = t.translation.y - zone.ground(t.translation.x, t.translation.z);
        assert!(over < 1.4 && over > 0.3, "sunk onto its legs: {over:.2} m");
        assert!(
            app.world()
                .get::<super::super::wreck::Broken>(heavy)
                .is_some()
        );
        for p in 0..3u8 {
            let (_, free) = seated(&mut app, heavy, Slot::Plate(p)).unwrap();
            assert!(free, "plate {p} loose on the wreck");
        }
        let (_, free) = seated(&mut app, heavy, Slot::Cell).unwrap();
        assert!(!free, "the cell's behind its hatch still");
        let catalog = app.world().resource::<Catalog>().clone();
        let salvage: Vec<String> = app
            .world_mut()
            .query_filtered::<&Item, Without<InSocket>>()
            .iter(app.world())
            .map(|i| catalog.get(i.kind()).id.clone())
            .filter(|id| id.starts_with("heavy_"))
            .collect();
        assert_eq!(
            salvage.iter().filter(|s| *s == "heavy_knee").count(),
            2,
            "{salvage:?}"
        );
        assert!(salvage.iter().any(|s| s == "heavy_claw"), "{salvage:?}");
        // The generic wreck loot isn't rolled on top.
        assert!(app.world().get::<super::super::Salvaged>(heavy).is_some());
    }

    /// An EMP puts it on its knees, but for less than a walker's twelve
    /// seconds; down, its hatch comes within reach of a hand; then it
    /// gets up.
    #[test]
    fn an_emp_puts_it_down_for_a_while() {
        let (mut app, heavy) = world_with_heavy([40.0, 230.0], [40.0, 230.0], false);
        let zone = app.world().resource::<Zone>().clone();
        for _ in 0..90 {
            app.update();
        }
        let hatch = |app: &mut App| {
            let t = *app.world().get::<GlobalTransform>(heavy).unwrap();
            let p = t.transform_point(plate_transform(0).translation);
            p.y - zone.ground(p.x, p.z)
        };
        let standing = hatch(&mut app);
        assert!(standing > 2.2, "out of reach standing: {standing:.2}");
        app.world_mut()
            .entity_mut(heavy)
            .insert(super::super::Stunned(crate::gadgets::EMP_STUN));
        for _ in 0..(90 * 3) {
            app.update();
        }
        let down = hatch(&mut app);
        assert!(down < 1.7, "in reach down: {down:.2}");
        for _ in 0..(90 * 5) {
            app.update();
        }
        assert!(
            app.world().get::<super::super::Stunned>(heavy).is_none(),
            "up again"
        );
        assert!(crate::gadgets::EMP_STUN > EMP_STUN);
    }

    /// A burner stalls it in seconds (both vents or not): down on its knees
    /// and its hatch blown off, the cell there for a hand.
    #[test]
    fn a_burner_stalls_it_and_opens_the_hatch() {
        let (mut app, heavy) = world_with_heavy([40.0, 230.0], [40.0, 230.0], false);
        let zone = app.world().resource::<Zone>().clone();
        let at = zone.on_ground(40.0, 230.0);
        crate::anomaly::spawn_anomaly(app.world_mut(), AnomalyKind::Burner, at, 2.5, Some(600.0));
        let mut stalled = None;
        for k in 0..(90 * 15) {
            app.update();
            if app.world().get::<Heavy>(heavy).unwrap().stall > 0.0 {
                stalled = Some(k as f32 / 90.0);
                break;
            }
        }
        let after = stalled.expect("it stalled");
        assert!(after < 8.0, "after {after:.1} s");
        app.update();
        app.update();
        assert!(app.world().get::<super::super::Stunned>(heavy).is_some());
        assert!(!app.world().get::<Heavy>(heavy).unwrap().plates_on[rules::HATCH]);
        let (_, free) = seated(&mut app, heavy, Slot::Cell).unwrap();
        assert!(free, "the cell's for the taking");
    }

    /// Walked into a well's reach, it's held: down, dragged toward the
    /// heart, and the heart crushes its knees until it falls.
    #[test]
    fn a_well_takes_it() {
        let (mut app, heavy) = world_with_heavy([40.0, 245.0], [40.0, 215.0], false);
        let zone = app.world().resource::<Zone>().clone();
        let well = zone.on_ground(40.0, 228.0);
        crate::anomaly::spawn_anomaly(app.world_mut(), AnomalyKind::Well, well, 2.0, Some(600.0));
        let mut held_at = None;
        for k in 0..(90 * 60) {
            app.update();
            if held_at.is_none() && app.world().get::<Heavy>(heavy).unwrap().held_by.is_some() {
                held_at = Some(k);
            }
            if app.world().get::<Robot>(heavy).unwrap().wrecked {
                break;
            }
        }
        assert!(held_at.is_some(), "the well took hold");
        assert!(
            app.world().get::<Robot>(heavy).unwrap().wrecked,
            "and it fell"
        );
        let t = app.world().get::<Transform>(heavy).unwrap().translation;
        let d = Vec2::new(t.x - well.x, t.z - well.z).length();
        assert!(d < 6.0, "it fell in the well ({d:.1} m out)");
    }

    /// Through the headset's hands: down on its knees with its hatch on, a
    /// hand at the bay closes on nothing; the hatch shot off, the same
    /// hand pulls the cell out, and that's the end of it.
    #[test]
    fn a_hand_pulls_the_cell() {
        use crate::rig::Side;
        let mut vr = crate::vr_harness::Vr::new(&crate::LaunchOptions::default().map);
        let zone = vr.app.world().resource::<Zone>().clone();
        let catalog = vr.app.world().resource::<Catalog>().clone();
        let at = zone.on_ground(40.0, 230.0);
        let heavy = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_heavy(
                &mut c,
                &zone,
                &catalog,
                at,
                Job {
                    kind: super::super::JobKind::Response,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            );
            c.entity(e).remove::<Senses>();
            vr.app.world_mut().flush();
            e
        };
        vr.step(60);
        // Down (an EMP), and the player at its back.
        vr.app
            .world_mut()
            .entity_mut(heavy)
            .insert(super::super::Stunned(EMP_STUN));
        vr.step(200);
        let (cell, _) = seated(&mut vr.app, heavy, Slot::Cell).unwrap();
        let body = *vr.app.world().get::<GlobalTransform>(heavy).unwrap();
        let back = (body.rotation() * Vec3::Z).with_y(0.0).normalize();
        let bay = vr
            .app
            .world()
            .get::<GlobalTransform>(cell)
            .unwrap()
            .translation();
        let feet = zone.on_ground(bay.x + back.x * 0.8, bay.z + back.z * 0.8);
        assert!(
            bay.y - feet.y < 1.7,
            "the bay's in reach: {:.2} m up",
            bay.y - feet.y
        );
        let yaw = (-back.x).atan2(-back.z) + std::f32::consts::PI;
        vr.stand(feet, yaw);
        let reach = |vr: &mut crate::vr_harness::Vr| {
            vr.hand(Side::Right, bay + back * 0.06, Quat::IDENTITY, 0.0, 0.0);
            vr.step(2);
            vr.grip(Side::Right, 1.0);
            vr.step(4);
            let got = vr.hand_state(Side::Right).holding;
            vr.grip(Side::Right, 0.0);
            vr.step(3);
            got
        };
        assert_eq!(reach(&mut vr), None, "the hatch is on: nothing to take");
        assert!(!vr.app.world().get::<Robot>(heavy).unwrap().wrecked);
        for k in 0..4u8 {
            let (b, _) = part(&mut vr.app, heavy, Slot::Bolt { plate: 0, bolt: k }).unwrap();
            hit(&mut vr.app, b, 40.0);
        }
        vr.step(30);
        vr.app
            .world_mut()
            .entity_mut(heavy)
            .insert(super::super::Stunned(EMP_STUN));
        vr.step(10);
        let bay = vr
            .app
            .world()
            .get::<GlobalTransform>(cell)
            .unwrap()
            .translation();
        vr.hand(Side::Right, bay + back * 0.06, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(4);
        assert_eq!(
            vr.hand_state(Side::Right).holding,
            Some(cell),
            "the cell's in hand"
        );
        vr.step(4);
        let h = vr.app.world().get::<Heavy>(heavy).unwrap();
        assert!(h.cell_pulled);
        assert!(
            vr.app.world().get::<Robot>(heavy).unwrap().wrecked,
            "and it's dead"
        );
    }

    /// Its blow through a real player: stand in reach and it raises its
    /// arm (the tell), then brings it down on you; step back out of reach
    /// while the arm is up and it comes down on the empty road.
    #[test]
    fn its_blow_lands_unless_you_move() {
        let run = |dodge: bool| -> (f32, bool) {
            let mut vr = crate::vr_harness::Vr::new(&crate::LaunchOptions::default().map);
            let zone = vr.app.world().resource::<Zone>().clone();
            let catalog = vr.app.world().resource::<Catalog>().clone();
            let at = zone.on_ground(40.0, 230.0);
            let me = zone.on_ground(40.0, 226.5);
            vr.stand(me, 0.0);
            let heavy = {
                let mut c = vr.app.world_mut().commands();
                let e = spawn_heavy(
                    &mut c,
                    &zone,
                    &catalog,
                    at,
                    Job {
                        kind: super::super::JobKind::Response,
                        site: at,
                        duration: 999.0,
                        exit: at,
                    },
                );
                vr.app.world_mut().flush();
                e
            };
            let hp0 = vr
                .app
                .world()
                .get::<crate::health::Health>(vr.player)
                .unwrap()
                .hp;
            let mut raised = false;
            for _ in 0..(90 * 15) {
                vr.step(1);
                let state = vr.app.world().get::<Arm>(heavy).unwrap().state;
                if state == ArmState::Raise && !raised {
                    raised = true;
                    if dodge {
                        // Back off, well out of its reach.
                        vr.stand(zone.on_ground(40.0, 219.0), 0.0);
                    }
                }
                if raised && state == ArmState::Rest {
                    break;
                }
            }
            let hp = vr
                .app
                .world()
                .get::<crate::health::Health>(vr.player)
                .unwrap()
                .hp;
            (hp0 - hp, raised)
        };
        let (hurt, raised) = run(false);
        assert!(raised, "its arm went up first");
        assert!(hurt > 30.0, "standing there, it hit ({hurt:.0})");
        let (hurt, raised) = run(true);
        assert!(raised);
        assert!(hurt < 1.0, "stepped back, it missed ({hurt:.0})");
    }
}
