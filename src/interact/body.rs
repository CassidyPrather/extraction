//! A thief's body: the torso that follows the head, the chest rig and belt
//! sockets on it, and the backpack.
//!
//! The backpack works like Into the Radius: it lives on your back; reach
//! over a shoulder and pull it round, let go in front of you and it hangs
//! there open; drop things into it (no collision inside, no tetris); grab
//! it and let go behind you to put it back on.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::inventory::{self, DEFAULT_SLOTS, SlotKind};
use extraction_core::items::{ItemCatalog, ItemKind, SizeClass};

use super::{
    GrabPoint, Grabbable, Held, InteractEvent, Socket, SocketKind, SocketOwner, Supported,
};
use crate::items::{Catalog, Item};
use crate::player::{Body, Player};
use crate::rig::RigInput;
use crate::world::Layer;

/// Inner half-size of the backpack's stowing volume, metres.
pub const BACKPACK_HALF_EXTENTS: Vec3 = Vec3::new(0.17, 0.24, 0.1);

/// Chest-height anchor for the belt, pouches, holster and pack. It hangs
/// at the height you stand at (learned from the headset), under your neck,
/// and turns lazily with your head: bending down to reach your belt
/// leaves the belt where it is. (It used to hang a fixed distance under
/// the eyes, so the holster and pockets dropped away from a reaching hand:
/// the first headset playtest couldn't reach them standing.)
#[derive(Component)]
pub struct Torso {
    pub yaw: f32,
    /// Standing eye height, the tallest the head has lately been.
    pub standing: f32,
}

/// How far below the standing eyes the torso anchor rides.
pub const CHEST_BELOW_EYES: f32 = 0.38;
/// How far the head can drop (bending, reaching down) before the body is
/// taken to be crouching and the anchor follows.
const BEND: f32 = 0.45;
/// Bending over, the hips stay where they are while the head goes forward
/// and down: the anchor (and the belt on it) may trail the neck by this
/// much per metre the head has dropped, up to [`HINGE_MOST`].
const HINGE: f32 = 1.3;
const HINGE_MOST: f32 = 0.45;

/// Index of a body socket in the default slot layout (1-9 keys in flat mode).
#[derive(Component, Clone, Copy, Debug)]
pub struct BodySlot(pub usize);

/// A container you drop things into: the backpack, or the truck's stash.
#[derive(Component, Debug)]
pub struct Backpack {
    pub owner: Entity,
    pub open: bool,
    pub model: inventory::Backpack,
    pub contents: Vec<Entity>,
    /// Worn (counts toward carried weight, opens with Tab) or not (a crate).
    pub carried: bool,
    /// Inner half-size of the stowing volume.
    pub half: Vec3,
}

impl Backpack {
    pub fn used(&self, catalog: &ItemCatalog, kinds: &[ItemKind]) -> f32 {
        kinds.iter().map(|k| catalog.get(*k).bulk).sum()
    }

    pub fn fits(&self, used: f32, size: SizeClass, bulk: f32) -> bool {
        self.model.check(used, size, bulk).is_ok()
    }
}

/// On an item inside a backpack.
#[derive(Component, Clone, Copy, Debug)]
pub struct InBackpack {
    pub bag: Entity,
}

/// Request to open or close the local backpack (flat mode's Tab).
#[derive(Message, Clone, Copy, Debug)]
pub struct ToggleBackpack {
    pub player: Entity,
}

/// Where the closed backpack rides, relative to the torso.
const BACK: Vec3 = Vec3::new(0.0, -0.12, 0.2);
/// Where flat mode hangs the open backpack, relative to the torso.
const FRONT: Vec3 = Vec3::new(0.0, -0.22, -0.48);

pub struct BodyPlugin;

impl Plugin for BodyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ToggleBackpack>()
            .add_observer(build_body)
            .add_observer(forget_despawned_in_bag)
            .add_observer(forget_despawned_in_socket)
            .add_systems(
                Update,
                (
                    follow_head,
                    toggle_backpacks,
                    backpack_released,
                    sync_backpack_visibility,
                    slung_reach,
                    handed_slots,
                    lean_with_chest
                        .after(handed_slots)
                        .after(toggle_backpacks)
                        .after(backpack_released),
                ),
            )
            .add_systems(FixedUpdate, carried_mass.in_set(CarriedMassSystems));
    }
}

/// The shoulders in the torso's frame (as the arms have them; x is the
/// right one, mirror it for the left).
pub const SHOULDER: Vec3 = Vec3::new(0.17, 0.12, 0.03);
/// A long gun let go this near a shoulder (front or back) goes onto that
/// side's sling, metres.
pub const SLING_REACH: f32 = 0.3;

/// A sling slot's shoulder, from the slot's place on the torso, in the
/// slot's frame.
pub fn sling_shoulder(slot_at: Vec3) -> Vec3 {
    let side = if slot_at.x < 0.0 { -1.0 } else { 1.0 };
    SHOULDER * Vec3::new(side, 1.0, 1.0) - slot_at
}

/// A slung gun's reach zone, put there by [`slung_reach`].
#[derive(Component)]
pub struct SlingReach;

/// A slung gun can be taken by reaching over its shoulder and gripping,
/// as the backpack is.
#[allow(clippy::type_complexity)]
fn slung_reach(
    mut commands: Commands,
    items: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&super::InSocket>,
            Has<SlingReach>,
        ),
        With<crate::items::Item>,
    >,
    sockets: Query<(&super::Socket, &Transform, &GlobalTransform)>,
) {
    for (e, gt, in_socket, has) in &items {
        let slot = in_socket
            .and_then(|s| sockets.get(s.socket).ok())
            .filter(|(s, ..)| s.kind == SocketKind::Body(SlotKind::Sling));
        match (slot, has) {
            (Some((_, local, sgt)), _) => {
                let shoulder = sgt.transform_point(sling_shoulder(local.translation));
                let centre = gt.affine().inverse().transform_point3(shoulder);
                commands.entity(e).insert((
                    SlingReach,
                    super::ReachZone {
                        centre,
                        half: Vec3::splat(0.13),
                    },
                ));
            }
            (None, true) => {
                commands
                    .entity(e)
                    .remove::<(SlingReach, super::ReachZone)>();
            }
            _ => {}
        }
    }
}

/// Where carried weight is summed (artifact tracking runs before it).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CarriedMassSystems;

/// When a player appears, give them a torso, a rig, and a backpack.
/// Something despawned while in a pack (a grenade going off in it) leaves
/// the pack's list too, wherever it was despawned from. The chaos monkey
/// found packs listing items long gone (`docs/SOAK.md`).
fn forget_despawned_in_bag(
    gone: On<bevy::ecs::lifecycle::Despawn, InBackpack>,
    stowed: Query<&InBackpack>,
    mut bags: Query<&mut Backpack>,
) {
    let e = gone.entity;
    if let Ok(i) = stowed.get(e)
        && let Ok(mut bag) = bags.get_mut(i.bag)
    {
        bag.contents.retain(|c| *c != e);
    }
}

/// And one despawned in a socket (a pouch, the holster, a gun's well)
/// leaves it empty.
fn forget_despawned_in_socket(
    gone: On<bevy::ecs::lifecycle::Despawn, super::InSocket>,
    seated: Query<&super::InSocket>,
    mut sockets: Query<&mut Socket>,
) {
    let e = gone.entity;
    if let Ok(i) = seated.get(e)
        && let Ok(mut s) = sockets.get_mut(i.socket)
        && s.occupant == Some(e)
    {
        s.occupant = None;
    }
}

fn build_body(add: On<Add, Player>, mut commands: Commands) {
    let player = add.entity;
    let torso = commands
        .spawn((
            Name::new("Torso"),
            Torso {
                yaw: 0.0,
                standing: 1.65,
            },
            Transform::from_xyz(0.0, 1.3, 0.0),
            Visibility::default(),
            ChildOf(player),
        ))
        .id();
    for (i, slot) in DEFAULT_SLOTS.iter().enumerate() {
        let (pos, seat, radius) = slot_layout(*slot, i);
        commands.spawn((
            Name::new(format!("{slot:?} slot")),
            BodySlot(i),
            Socket {
                kind: SocketKind::Body(*slot),
                occupant: None,
                radius,
                seat,
            },
            SocketOwner(player),
            Transform::from_translation(pos),
            Visibility::default(),
            ChildOf(torso),
        ));
    }
    commands.spawn((
        Name::new("Backpack"),
        Backpack {
            owner: player,
            open: false,
            model: inventory::Backpack::default(),
            contents: Vec::new(),
            carried: true,
            half: BACKPACK_HALF_EXTENTS,
        },
        Grabbable {
            points: vec![GrabPoint::free()],
        },
        // Reach over either shoulder, anywhere behind the head, and it's
        // the bag you take (bag space: it rides 0.5 m below the eyes).
        super::ReachZone {
            centre: Vec3::new(0.0, 0.32, 0.02),
            half: Vec3::new(0.42, 0.34, 0.3),
        },
        Transform::from_translation(BACK),
        Visibility::default(),
        RigidBody::Dynamic,
        RigidBodyDisabled,
        ColliderDisabled,
        Collider::cuboid(0.36, 0.5, 0.22),
        CollisionLayers::new(Layer::Item, [Layer::World]),
        ChildOf(torso),
    ));
}

/// The holster and the chest mount on the gun hand's side and the other:
/// for a left-handed player (`Comfort::left_handed`) they swap sides.
fn handed_slots(
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut slots: Query<(&BodySlot, &Socket, &SocketOwner, &mut Transform)>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    added: Query<(), Added<BodySlot>>,
) {
    let Some(comfort) = comfort else {
        return;
    };
    if !comfort.is_changed() && added.is_empty() {
        return;
    }
    for (slot, socket, owner, mut t) in &mut slots {
        let SocketKind::Body(kind @ (SlotKind::Holster | SlotKind::Mount)) = socket.kind else {
            continue;
        };
        if !locals.contains(owner.0) {
            continue;
        }
        let (mut pos, ..) = slot_layout(kind, slot.0);
        if comfort.left_handed {
            pos.x = -pos.x;
        }
        if t.translation != pos {
            t.translation = pos;
        }
    }
}

/// Is a slot on the chest (leaning with it as you bend over) rather than on
/// the belt (staying where it is, so a hand reaching down finds it)?
pub fn on_chest(slot: SlotKind) -> bool {
    !matches!(slot, SlotKind::Holster | SlotKind::Pocket | SlotKind::Head)
}

/// The chest's lean in the torso anchor's own frame, about the pelvis:
/// where a point on the chest (`at`, torso-local, standing) goes.
pub fn leaned(lean: Quat, at: Vec3) -> Vec3 {
    let pelvis = Vec3::NEG_Y * crate::rig::stance::PELVIS_BELOW_CHEST;
    pelvis + lean * (at - pelvis)
}

/// The chest's slots and the worn pack lean with the chest (the stance's
/// spine, `rig::stance`) as you bend over; the belt's stay. Without a
/// stance (a bot, flat mode before it's placed) nothing leans.
#[allow(clippy::type_complexity)]
fn lean_with_chest(
    players: Query<(&crate::rig::stance::Stance, &Children), With<crate::rig::LocalPlayer>>,
    torsos: Query<(&GlobalTransform, &Children), With<Torso>>,
    mut slots: Query<(&BodySlot, &Socket, &mut Transform), Without<Backpack>>,
    mut bags: Query<(&Backpack, &mut Transform), (Without<BodySlot>, Without<Held>)>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
) {
    let left = comfort.is_some_and(|c| c.left_handed);
    for (st, children) in &players {
        if !st.ready {
            continue;
        }
        let Some((torso_gt, kit)) = children.iter().find_map(|c| torsos.get(c).ok()) else {
            continue;
        };
        let facing = torso_gt.compute_transform().rotation;
        let lean = facing.inverse() * st.chest.rotation;
        for k in kit.iter() {
            if let Ok((slot, socket, mut t)) = slots.get_mut(k) {
                let SocketKind::Body(kind) = socket.kind else {
                    continue;
                };
                if !on_chest(kind) {
                    continue;
                }
                let (mut pos, ..) = slot_layout(kind, slot.0);
                if left && matches!(kind, SlotKind::Mount) {
                    pos.x = -pos.x;
                }
                let want = Transform::from_translation(leaned(lean, pos)).with_rotation(lean);
                if *t != want {
                    *t = want;
                }
            } else if let Ok((bag, mut t)) = bags.get_mut(k) {
                if bag.open || !bag.carried {
                    continue;
                }
                let want = Transform::from_translation(leaned(lean, BACK)).with_rotation(lean);
                if *t != want {
                    *t = want;
                }
            }
        }
    }
}

/// Where each slot sits on the torso (origin mid-chest), how its occupant
/// is seated, and its snap radius.
fn slot_layout(slot: SlotKind, index: usize) -> (Vec3, Transform, f32) {
    use std::f32::consts::FRAC_PI_2;
    match slot {
        SlotKind::Sling => {
            // Diagonally across the back, muzzle up over a shoulder.
            let side = if index == 0 { -1.0 } else { 1.0 };
            (
                Vec3::new(0.12 * side, -0.05, 0.16),
                Transform::from_rotation(
                    Quat::from_rotation_z(0.5 * side) * Quat::from_rotation_x(FRAC_PI_2),
                ),
                0.25,
            )
        }
        SlotKind::Holster => (
            Vec3::new(0.22, -0.42, 0.02),
            Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            0.16,
        ),
        SlotKind::Pouch => {
            let k = index.saturating_sub(3) as f32;
            (
                Vec3::new(-0.14 + k * 0.09, -0.16, -0.13),
                Transform::IDENTITY,
                0.08,
            )
        }
        SlotKind::Pocket => {
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            (
                Vec3::new(0.17 * side, -0.45, -0.08),
                Transform::IDENTITY,
                0.09,
            )
        }
        SlotKind::Mount => (Vec3::new(-0.13, 0.06, -0.12), Transform::IDENTITY, 0.08),
        // Worn: the vest on the chest (let go mid-chest, it sits in the
        // torso's own frame, as its worn model is drawn), the helmet on the
        // head (from the torso's middle).
        SlotKind::Vest => (
            Vec3::new(0.0, -0.08, -0.02),
            Transform::from_xyz(0.0, 0.08, 0.02),
            0.25,
        ),
        SlotKind::Head => (Vec3::new(0.0, 0.43, -0.02), Transform::IDENTITY, 0.2),
    }
}

/// The torso trails the head: always under it, turning when the head has
/// looked far enough away or the player is walking.
fn follow_head(
    players: Query<(&RigInput, &Children), With<Player>>,
    mut torsos: Query<(&mut Torso, &mut Transform)>,
    time: Res<Time>,
) {
    for (input, children) in &players {
        for c in children.iter() {
            let Ok((mut torso, mut t)) = torsos.get_mut(c) else {
                continue;
            };
            let head = input.head;
            let fwd = head.rotation * Vec3::NEG_Z;
            let head_yaw = (-fwd.x).atan2(-fwd.z);
            let mut diff = head_yaw - torso.yaw;
            while diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            }
            while diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }
            let walking = input.locomotion.length() > 0.1;
            let rate = if walking || diff.abs() > 0.8 {
                6.0
            } else {
                0.8
            };
            let dt = time.delta_secs();
            torso.yaw += diff * (rate * dt).min(1.0);
            // Standing height: the tallest lately (a headset recentred lower
            // brings it down a centimetre a minute).
            torso.standing = torso.standing.max(head.translation.y) - dt * 0.01 / 60.0;
            let drop = (torso.standing - head.translation.y - BEND).max(0.0);
            let y = torso.standing - CHEST_BELOW_EYES - drop;
            // Under the neck (behind and below the eyes), so looking down
            // doesn't swing the belt forward; eased, so it doesn't jitter.
            let neck = head.translation + head.rotation * Vec3::new(0.0, -0.1, 0.09);
            // Upright, under the neck; bending over, the hips stay put (up
            // to a point) and the chest leans out over them (`rig::stance`).
            let bent = ((torso.standing - head.translation.y) * HINGE).clamp(0.0, HINGE_MOST);
            let under = Vec2::new(neck.x, neck.z);
            let was = Vec2::new(t.translation.x, t.translation.z);
            let at = under + (was - under).clamp_length_max(bent);
            let want = Vec3::new(at.x, y, at.y);
            let ease = (12.0 * dt).min(1.0);
            t.translation = if t.translation.distance(want) > 0.5 {
                want
            } else {
                t.translation.lerp(want, ease)
            };
            t.rotation = Quat::from_rotation_y(torso.yaw);
        }
    }
}

/// Puts an item inside a bag at a local position.
pub fn stow_in_backpack(
    commands: &mut Commands,
    item: Entity,
    bag_e: Entity,
    bag: &mut Backpack,
    local: Vec3,
) {
    bag.contents.push(item);
    let local = local.clamp(-bag.half, bag.half);
    commands.entity(item).remove::<(Held, Supported)>().insert((
        InBackpack { bag: bag_e },
        ChildOf(bag_e),
        Transform::from_translation(local),
        RigidBodyDisabled,
        ColliderDisabled,
        if bag.open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
    ));
}

fn toggle_backpacks(
    mut requests: MessageReader<ToggleBackpack>,
    mut bags: Query<(&mut Backpack, &mut Transform)>,
) {
    for req in requests.read() {
        for (mut bag, mut t) in &mut bags {
            if bag.owner != req.player || !bag.carried {
                continue;
            }
            bag.open = !bag.open;
            *t = if bag.open {
                Transform::from_translation(FRONT)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
            } else {
                Transform::from_translation(BACK)
            };
        }
    }
}

/// A backpack let go of in VR: behind the shoulders puts it on (closed),
/// anywhere else leaves it hanging open in front of you.
fn backpack_released(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    mut bags: Query<(&mut Backpack, &GlobalTransform)>,
    torsos: Query<(Entity, &GlobalTransform, &ChildOf), With<Torso>>,
) {
    for ev in events.read() {
        let InteractEvent::Released { item, .. } = ev else {
            continue;
        };
        let Ok((mut bag, gt)) = bags.get_mut(*item) else {
            continue;
        };
        let Some((torso_e, torso_gt, _)) = torsos.iter().find(|(_, _, p)| p.parent() == bag.owner)
        else {
            continue;
        };
        let local = torso_gt.affine().inverse() * gt.affine();
        let local = Transform::from_matrix(local.into());
        let behind = local.translation.distance(BACK) < 0.35 || local.translation.z > 0.05;
        bag.open = !behind;
        let t = if behind {
            Transform::from_translation(BACK)
        } else {
            local
        };
        commands
            .entity(*item)
            .insert((ChildOf(torso_e), t, RigidBodyDisabled, ColliderDisabled));
    }
}

fn sync_backpack_visibility(
    bags: Query<&Backpack, Changed<Backpack>>,
    mut vis: Query<&mut Visibility, With<InBackpack>>,
) {
    for bag in &bags {
        for item in &bag.contents {
            if let Ok(mut v) = vis.get_mut(*item) {
                *v = if bag.open {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

/// Sums what each player carries into their body's encumbrance.
/// Sums what each player carries into `Body::carried_kg` (the
/// featherstone takes some off).
fn carried_mass(
    mut players: Query<(Entity, &mut Body)>,
    carried: Query<(
        &Item,
        &crate::anomaly::artifacts::CarriedBy,
        Has<crate::anomaly::artifacts::Shielded>,
    )>,
    bags: Query<&Backpack>,
    catalog: Res<Catalog>,
) {
    for (player, mut body) in &mut players {
        let mut kg = 0.0;
        for (item, by, shielded) in &carried {
            if by.0 == player {
                let def = catalog.get(item.kind());
                kg += def.mass - if shielded { 0.0 } else { def.lift_kg };
            }
        }
        for bag in &bags {
            if bag.owner == player && bag.carried {
                kg += bag.model.mass;
            }
        }
        body.carried_kg = kg.max(0.0);
    }
}
