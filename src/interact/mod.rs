//! Hands and the things they hold.
//!
//! - [`Grabbable`] items have grab points: free (keep the pose you grabbed
//!   at), primary (a pistol grip snaps into the palm), support (a
//!   handguard, for the second hand) and handles (a slide or bolt, which
//!   the weapons module turns into mechanism motion).
//! - Held items stay dynamic rigid bodies whose velocity is driven toward
//!   the hand every tick, so they collide with the world and heavy things
//!   lag a swinging arm. Let go and they keep the hand's velocity: throws.
//! - [`Socket`]s accept items released near them: body slots, mag wells,
//!   rails. The [`body`] module lays out a thief's rig and the backpack.
//!
//! VR drives all of this through grip presses at hand positions; flat mode
//! sends [`HandAction`]s that do the same things without needing to aim a
//! hand.

pub mod body;
pub mod flat;

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::inventory::SlotKind;
use extraction_core::items::SizeClass;

use crate::items::{Catalog, Item};
use crate::player::Player;
use crate::rig::{RigHand, RigInput, Side};
use crate::world::Layer;

/// How a hand holds a grab point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GripRole {
    /// Keep the relative pose the hand grabbed at.
    Free,
    /// The item snaps so this point sits in the palm (pistol grips).
    Primary,
    /// A second hand steadies an item held by the first (handguards).
    Support,
    /// A moving part: slide, charging handle, bolt. The id is the part's.
    Handle(u8),
}

#[derive(Clone, Debug)]
pub struct GrabPoint {
    /// Pose of the hand's grip point in the item's local space.
    pub local: Transform,
    pub role: GripRole,
    /// How close the hand must be, metres.
    pub radius: f32,
}

impl GrabPoint {
    pub fn free() -> Self {
        Self {
            local: Transform::IDENTITY,
            role: GripRole::Free,
            radius: 0.0,
        }
    }

    pub fn new(local: Transform, role: GripRole, radius: f32) -> Self {
        Self {
            local,
            role,
            radius,
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct Grabbable {
    pub points: Vec<GrabPoint>,
}

/// On an item while a hand holds it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Held {
    pub player: Entity,
    pub hand: Entity,
    pub side: Side,
    pub point: usize,
    /// The item's pose in hand space.
    pub offset: Transform,
}

/// On an item while a second hand steadies it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Supported {
    pub hand: Entity,
    pub side: Side,
    pub point: usize,
}

/// Per-hand state, on the rig hand entity.
#[derive(Component, Default, Debug)]
pub struct Hand {
    pub holding: Option<Entity>,
    pub supporting: Option<Entity>,
    /// A moving part being worked: (item, handle id).
    pub handle: Option<(Entity, u8)>,
    /// Hand pose when the handle was grabbed, in the item's space.
    pub handle_origin: Vec3,
    pub grip_down: bool,
    /// World-space velocity, smoothed, for throws.
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    last: Option<Transform>,
    /// World pose this tick.
    pub world: Transform,
}

/// What a socket takes.
#[derive(Clone, Debug, PartialEq)]
pub enum SocketKind {
    Body(SlotKind),
    /// A gun's magazine well; takes magazines of this family.
    MagWell(String),
    /// An attachment point or insert (dovetail, rail, muzzle, battery
    /// compartment, artifact cradle); takes items that fit that mount.
    Mount(extraction_core::items::Mount),
}

#[derive(Component, Clone, Debug)]
pub struct Socket {
    pub kind: SocketKind,
    pub occupant: Option<Entity>,
    /// Snap distance for a released item, metres.
    pub radius: f32,
    /// Pose the occupant takes, relative to the socket.
    pub seat: Transform,
}

/// On an item sitting in a socket.
#[derive(Component, Clone, Copy, Debug)]
pub struct InSocket {
    pub socket: Entity,
}

/// Who a socket belongs to (a player, or a gun).
#[derive(Component, Clone, Copy, Debug)]
pub struct SocketOwner(pub Entity);

/// Flat-mode (and bot) hand commands: do what a VR hand would, without
/// positioning it first.
#[derive(Message, Clone, Debug)]
pub enum HandAction {
    /// Grab this item (or its nearest grab point) with this hand.
    Grab {
        player: Entity,
        side: Side,
        item: Entity,
        point: Option<usize>,
    },
    /// Open the hand. `throw` adds forward speed, m/s.
    Release {
        player: Entity,
        side: Side,
        throw: f32,
    },
    /// Put the held item away: best body slot, else the backpack.
    Stow { player: Entity, side: Side },
    /// Draw from a body slot into an empty hand.
    Draw {
        player: Entity,
        side: Side,
        slot: usize,
    },
}

/// Something was grabbed or let go, for sounds and haptics.
#[derive(Message, Clone, Debug)]
pub enum InteractEvent {
    Grabbed {
        item: Entity,
        side: Side,
    },
    Released {
        item: Entity,
        side: Side,
        speed: f32,
    },
    Socketed {
        item: Entity,
        socket: Entity,
    },
    Unsocketed {
        item: Entity,
        socket: Entity,
    },
    Stowed {
        item: Entity,
    },
    Refused {
        item: Entity,
    },
    /// A pressable (bell pull, button) was grabbed.
    Pressed {
        item: Entity,
        player: Entity,
    },
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub enum InteractSystems {
    /// Hand poses and velocities computed.
    Track,
    /// Grabs and releases resolved.
    Grab,
    /// Held items driven toward hands.
    Follow,
}

pub struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HandAction>()
            .add_message::<InteractEvent>()
            .configure_sets(
                FixedUpdate,
                (
                    InteractSystems::Track,
                    InteractSystems::Grab,
                    InteractSystems::Follow,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                (
                    track_hands.in_set(InteractSystems::Track),
                    (grip_from_input, apply_hand_actions)
                        .chain()
                        .in_set(InteractSystems::Grab),
                    follow_hands.in_set(InteractSystems::Follow),
                ),
            )
            .add_plugins(body::BodyPlugin);
    }
}

/// World pose of every rig hand, from the root and the rig input (not last
/// frame's propagated transforms, which lag a tick).
fn track_hands(
    players: Query<(&Transform, &RigInput, &Children), With<Player>>,
    mut hands: Query<(&RigHand, &mut Hand)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (root, input, children) in &players {
        for child in children.iter() {
            let Ok((rh, mut hand)) = hands.get_mut(child) else {
                continue;
            };
            let world = root.mul_transform(input.hand(rh.side).pose);
            if let Some(last) = hand.last {
                let v = (world.translation - last.translation) / dt;
                hand.velocity = hand.velocity.lerp(v, 0.5);
                let dq = world.rotation * last.rotation.inverse();
                let (axis, angle) = dq.to_axis_angle();
                let w = if angle > std::f32::consts::PI {
                    axis * (angle - std::f32::consts::TAU)
                } else {
                    axis * angle
                } / dt;
                hand.angular_velocity = hand.angular_velocity.lerp(w, 0.5);
            }
            hand.last = Some(world);
            hand.world = world;
        }
    }
}

/// VR path: grip presses become grabs of whatever is in reach, releases
/// become drops or socketings.
fn grip_from_input(
    players: Query<(Entity, &RigInput, &Children), With<Player>>,
    mut hands: Query<(Entity, &RigHand, &mut Hand)>,
    grabbables: Query<(
        Entity,
        &GlobalTransform,
        &Grabbable,
        Option<&Held>,
        Has<ColliderDisabled>,
    )>,
    spatial: SpatialQuery,
    mut actions: MessageWriter<HandAction>,
) {
    for (player, input, children) in &players {
        for child in children.iter() {
            let Ok((_, rh, mut hand)) = hands.get_mut(child) else {
                continue;
            };
            let grip = input.hand(rh.side).grip;
            let pressed = !hand.grip_down && grip > 0.6;
            let released = hand.grip_down && grip < 0.35;
            if pressed {
                hand.grip_down = true;
                let busy =
                    hand.holding.is_some() || hand.supporting.is_some() || hand.handle.is_some();
                if !busy {
                    if let Some((item, point)) =
                        find_grab(&hand.world, player, &grabbables, &spatial)
                    {
                        actions.write(HandAction::Grab {
                            player,
                            side: rh.side,
                            item,
                            point: Some(point),
                        });
                    }
                }
            } else if released {
                hand.grip_down = false;
                actions.write(HandAction::Release {
                    player,
                    side: rh.side,
                    throw: 0.0,
                });
            }
        }
    }
}

/// Nearest grab point in reach of a hand. Points with a radius must be
/// within it; free items are found by touching their collider.
fn find_grab(
    hand: &Transform,
    _player: Entity,
    grabbables: &Query<(
        Entity,
        &GlobalTransform,
        &Grabbable,
        Option<&Held>,
        Has<ColliderDisabled>,
    )>,
    spatial: &SpatialQuery,
) -> Option<(Entity, usize)> {
    let reach = Collider::sphere(0.07);
    let touching = spatial.shape_intersections(
        &reach,
        hand.translation,
        Quat::IDENTITY,
        &SpatialQueryFilter::from_mask([Layer::Item]),
    );
    let mut best: Option<(f32, Entity, usize)> = None;
    for (e, gt, g, _held, disabled) in grabbables.iter() {
        for (i, p) in g.points.iter().enumerate() {
            let d = if p.role == GripRole::Free {
                // Socketed and stowed things have no live collider: reach for their middle.
                if disabled {
                    let d = gt.translation().distance(hand.translation);
                    if d > 0.13 {
                        continue;
                    }
                    d
                } else if touching.contains(&e) {
                    gt.translation().distance(hand.translation) * 0.5
                } else {
                    continue;
                }
            } else {
                let world = gt.mul_transform(p.local).translation();
                let d = world.distance(hand.translation);
                if d > p.radius {
                    continue;
                }
                d
            };
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, e, i));
            }
        }
    }
    best.map(|(_, e, i)| (e, i))
}

/// Resolves grabs, releases, stows and draws, from VR grips and flat-mode
/// commands alike.
fn apply_hand_actions(
    mut commands: Commands,
    mut actions: MessageReader<HandAction>,
    mut events: MessageWriter<InteractEvent>,
    players: Query<(&Transform, &RigInput, &Children), With<Player>>,
    mut hands: Query<(Entity, &RigHand, &mut Hand)>,
    items: Query<(
        &GlobalTransform,
        &Grabbable,
        Option<&Held>,
        Option<&InSocket>,
        Option<&Item>,
    )>,
    mut sockets: Query<(Entity, &GlobalTransform, &mut Socket, Option<&SocketOwner>)>,
    mut backpacks: Query<(Entity, &GlobalTransform, &mut body::Backpack)>,
    catalog: Res<Catalog>,
    pressables: Query<(), With<crate::raid::truck::Pressable>>,
    fixed: Query<(), With<crate::raid::truck::Stash>>,
) {
    for action in actions.read() {
        match *action {
            HandAction::Grab {
                player,
                side,
                item,
                point,
            } if pressables.contains(item) => {
                let _ = (side, point);
                events.write(InteractEvent::Pressed { item, player });
            }
            HandAction::Grab { item, .. } if fixed.contains(item) => {}
            HandAction::Grab {
                player,
                side,
                item,
                point,
            } => {
                let Some((hand_e, hand_world)) = hand_of(&players, &hands, player, side) else {
                    warn!("grab: player {player} has no {side:?} hand");
                    continue;
                };
                let Ok((gt, grabbable, held, in_socket, _)) = items.get(item) else {
                    warn!("grab: {item} is not grabbable");
                    continue;
                };
                let point = point.unwrap_or_else(|| nearest_point(grabbable, gt, &hand_world));
                let Some(gp) = grabbable.points.get(point).cloned() else {
                    continue;
                };
                let Ok((_, _, mut hand)) = hands.get_mut(hand_e) else {
                    continue;
                };
                match gp.role {
                    GripRole::Support | GripRole::Handle(_)
                        if held.is_some_and(|h| h.player == player) =>
                    {
                        if let GripRole::Handle(id) = gp.role {
                            hand.handle = Some((item, id));
                            hand.handle_origin = gt
                                .affine()
                                .inverse()
                                .transform_point3(hand_world.translation);
                        } else {
                            hand.supporting = Some(item);
                            commands.entity(item).insert(Supported {
                                hand: hand_e,
                                side,
                                point,
                            });
                        }
                        events.write(InteractEvent::Grabbed { item, side });
                    }
                    _ => {
                        // Taking it from the other hand, a socket or the bag.
                        if let Some(h) = held {
                            if let Ok((_, _, mut other)) = hands.get_mut(h.hand) {
                                other.holding = None;
                            }
                        }
                        if let Some(s) = in_socket {
                            if let Ok((_, _, mut socket, _)) = sockets.get_mut(s.socket) {
                                socket.occupant = None;
                            }
                            events.write(InteractEvent::Unsocketed {
                                item,
                                socket: s.socket,
                            });
                            commands.entity(item).remove::<InSocket>();
                        }
                        for (_, _, mut bag) in &mut backpacks {
                            bag.contents.retain(|e| *e != item);
                        }
                        let Ok((_, _, mut hand)) = hands.get_mut(hand_e) else {
                            continue;
                        };
                        let offset = match gp.role {
                            GripRole::Free => hand_world.compute_affine().inverse() * gt.affine(),
                            _ => gp.local.compute_affine().inverse(),
                        };
                        let offset = Transform::from_matrix(offset.into());
                        hand.holding = Some(item);
                        let world = hand_world.mul_transform(offset);
                        physics_on(&mut commands, item, world, hand.velocity, Vec3::ZERO, false);
                        commands.entity(item).insert(Held {
                            player,
                            hand: hand_e,
                            side,
                            point,
                            offset,
                        });
                        events.write(InteractEvent::Grabbed { item, side });
                    }
                }
            }
            HandAction::Release {
                player,
                side,
                throw,
            } => {
                let Some((hand_e, hand_world)) = hand_of(&players, &hands, player, side) else {
                    continue;
                };
                let Ok((_, _, mut hand)) = hands.get_mut(hand_e) else {
                    continue;
                };
                hand.handle = None;
                if let Some(item) = hand.supporting.take() {
                    commands.entity(item).remove::<Supported>();
                }
                let Some(item) = hand.holding.take() else {
                    continue;
                };
                let velocity = hand.velocity + hand_world.forward() * throw;
                let angular = hand.angular_velocity;
                commands.entity(item).remove::<(Held, Supported)>();
                // Any other hand supporting it lets go too.
                for (_, _, mut h) in &mut hands {
                    if h.supporting == Some(item) {
                        h.supporting = None;
                    }
                }
                let Ok((gt, _, _, _, it)) = items.get(item) else {
                    continue;
                };
                if it.is_none() {
                    // Not an item (the backpack): just let go; its own
                    // system decides where it ends up.
                    commands
                        .entity(item)
                        .insert((GravityScale(1.0), LinearVelocity(velocity)));
                    events.write(InteractEvent::Released {
                        item,
                        side,
                        speed: velocity.length(),
                    });
                    continue;
                }
                let size = it
                    .map(|i| catalog.get(i.kind()).size)
                    .unwrap_or(SizeClass::Medium);
                let is_gun = it.is_some_and(|i| catalog.get(i.kind()).firearm.is_some());
                let mount = it.and_then(|i| catalog.get(i.kind()).fits_mount());
                let family = it.and_then(|i| {
                    catalog
                        .get(i.kind())
                        .magazine
                        .as_ref()
                        .map(|m| m.fits.clone())
                });
                // Socket in reach?
                let pos = gt.translation();
                let mut target = None;
                for (se, sgt, socket, owner) in &sockets {
                    if socket.occupant.is_some()
                        || !accepts_item(&socket.kind, size, family.as_deref(), is_gun, mount)
                    {
                        continue;
                    }
                    if owner.is_some_and(|o| o.0 == item) {
                        continue;
                    }
                    let d = sgt.translation().distance(pos);
                    if d < socket.radius && target.is_none_or(|(bd, _)| d < bd) {
                        target = Some((d, se));
                    }
                }
                if let Some((_, se)) = target {
                    if let Ok((_, _, mut socket, _)) = sockets.get_mut(se) {
                        seat_in_socket(&mut commands, item, se, &mut socket);
                        events.write(InteractEvent::Socketed { item, socket: se });
                        continue;
                    }
                }
                // Into an open backpack?
                let mut stowed = false;
                for (be, bgt, mut bag) in &mut backpacks {
                    if !bag.open || bag.owner != player {
                        continue;
                    }
                    let local = bgt.affine().inverse().transform_point3(pos);
                    if local.abs().cmple(bag.half * 1.3).all() {
                        if let Some(it) = it {
                            let def = catalog.get(it.kind());
                            let used = bag.used(&catalog, &items_kinds(&items, &bag.contents));
                            if bag.fits(used, def.size, def.bulk) {
                                body::stow_in_backpack(&mut commands, item, be, &mut bag, local);
                                events.write(InteractEvent::Stowed { item });
                                stowed = true;
                            } else {
                                events.write(InteractEvent::Refused { item });
                            }
                        }
                        break;
                    }
                }
                if stowed {
                    continue;
                }
                commands.entity(item).insert((
                    GravityScale(1.0),
                    LinearVelocity(velocity),
                    AngularVelocity(angular),
                ));
                events.write(InteractEvent::Released {
                    item,
                    side,
                    speed: velocity.length(),
                });
            }
            HandAction::Stow { player, side } => {
                let Some((hand_e, _)) = hand_of(&players, &hands, player, side) else {
                    continue;
                };
                let Ok((_, _, mut hand)) = hands.get_mut(hand_e) else {
                    continue;
                };
                let Some(item) = hand.holding else {
                    continue;
                };
                let Ok((_, _, _, _, Some(it))) = items.get(item) else {
                    continue;
                };
                let def = catalog.get(it.kind());
                // A body slot first (most specific), then the backpack.
                let mut slots: Vec<(u8, Entity)> = sockets
                    .iter()
                    .filter(|(_, _, s, o)| {
                        o.is_some_and(|o| o.0 == player)
                            && s.occupant.is_none()
                            && accepts_item(&s.kind, def.size, None, def.firearm.is_some(), None)
                    })
                    .map(|(e, _, s, _)| (slot_rank(&s.kind), e))
                    .collect();
                slots.sort_by_key(|(r, _)| *r);
                if let Some((_, se)) = slots.first().copied() {
                    if let Ok((_, _, mut socket, _)) = sockets.get_mut(se) {
                        hand.holding = None;
                        commands.entity(item).remove::<(Held, Supported)>();
                        seat_in_socket(&mut commands, item, se, &mut socket);
                        events.write(InteractEvent::Socketed { item, socket: se });
                        continue;
                    }
                }
                let mut done = false;
                for (be, _, mut bag) in &mut backpacks {
                    if bag.owner != player || !bag.carried {
                        continue;
                    }
                    let used = bag.used(&catalog, &items_kinds(&items, &bag.contents));
                    if bag.fits(used, def.size, def.bulk) {
                        hand.holding = None;
                        commands.entity(item).remove::<(Held, Supported)>();
                        let n = bag.contents.len() as f32;
                        let spot = Vec3::new(
                            ((n * 0.37) % 1.0 - 0.5) * 0.3,
                            ((n * 0.61) % 1.0 - 0.5) * 0.35,
                            ((n * 0.23) % 1.0 - 0.5) * 0.15,
                        );
                        body::stow_in_backpack(&mut commands, item, be, &mut bag, spot);
                        events.write(InteractEvent::Stowed { item });
                        done = true;
                    } else {
                        events.write(InteractEvent::Refused { item });
                    }
                    break;
                }
                let _ = done;
            }
            HandAction::Draw { player, side, slot } => {
                let mine: Vec<(Entity, &Socket)> = sockets
                    .iter()
                    .filter(|(_, _, _, o)| o.is_some_and(|o| o.0 == player))
                    .map(|(e, _, s, _)| (e, s))
                    .collect();
                let mut ordered = mine;
                ordered.sort_by_key(|(e, _)| *e);
                let Some(item) = ordered.get(slot).and_then(|(_, s)| s.occupant) else {
                    continue;
                };
                // Reuse the grab path: it unsockets.
                let point = items
                    .get(item)
                    .ok()
                    .map(|(_, g, ..)| {
                        g.points
                            .iter()
                            .position(|p| p.role == GripRole::Primary)
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);
                // Defer to next tick through the message queue.
                let _ = point;
                commands.queue(move |world: &mut World| {
                    world.write_message(HandAction::Grab {
                        player,
                        side,
                        item,
                        point: Some(point),
                    });
                });
            }
        }
    }
}

fn items_kinds(
    items: &Query<(
        &GlobalTransform,
        &Grabbable,
        Option<&Held>,
        Option<&InSocket>,
        Option<&Item>,
    )>,
    contents: &[Entity],
) -> Vec<extraction_core::items::ItemKind> {
    contents
        .iter()
        .filter_map(|e| items.get(*e).ok().and_then(|(.., it)| it.map(|i| i.kind())))
        .collect()
}

fn slot_rank(kind: &SocketKind) -> u8 {
    match kind {
        SocketKind::Body(SlotKind::Pocket) => 0,
        SocketKind::Body(SlotKind::Pouch) => 1,
        SocketKind::Body(SlotKind::Mount) => 2,
        SocketKind::Body(SlotKind::Holster) => 3,
        SocketKind::Body(SlotKind::Sling) => 4,
        _ => 9,
    }
}

/// Whether a socket takes an item of this size (and magazine family).
/// Holsters are for pistols only.
pub fn accepts(kind: &SocketKind, size: SizeClass, mag_family: Option<&str>) -> bool {
    accepts_item(kind, size, mag_family, true, None)
}

pub fn accepts_item(
    kind: &SocketKind,
    size: SizeClass,
    mag_family: Option<&str>,
    is_gun: bool,
    mount: Option<extraction_core::items::Mount>,
) -> bool {
    match kind {
        SocketKind::Body(SlotKind::Holster) => size == SizeClass::Medium && is_gun,
        SocketKind::Body(slot) => slot.accepts(size),
        SocketKind::MagWell(family) => mag_family == Some(family.as_str()),
        SocketKind::Mount(m) => mount == Some(*m),
    }
}

/// Parents an item to a socket and switches its physics off. Physics is
/// disabled rather than removed: Avian keeps its bookkeeping straight that
/// way, and switching back on is just removing the markers.
pub fn seat_in_socket(
    commands: &mut Commands,
    item: Entity,
    socket_e: Entity,
    socket: &mut Socket,
) {
    socket.occupant = Some(item);
    commands.entity(item).remove::<(Held, Supported)>().insert((
        InSocket { socket: socket_e },
        RigidBodyDisabled,
        ColliderDisabled,
        ChildOf(socket_e),
        socket.seat,
    ));
}

/// Takes an item out of whatever holds it and gives it its physics back at
/// its current world pose.
pub fn physics_on(
    commands: &mut Commands,
    item: Entity,
    world: Transform,
    velocity: Vec3,
    angular: Vec3,
    gravity: bool,
) {
    commands
        .entity(item)
        .remove::<(
            ChildOf,
            InSocket,
            body::InBackpack,
            RigidBodyDisabled,
            ColliderDisabled,
        )>()
        .insert((
            world,
            Position(world.translation),
            Rotation(world.rotation),
            RigidBody::Dynamic,
            LinearVelocity(velocity),
            AngularVelocity(angular),
            GravityScale(if gravity { 1.0 } else { 0.0 }),
            Visibility::Inherited,
        ));
}

fn nearest_point(g: &Grabbable, gt: &GlobalTransform, hand: &Transform) -> usize {
    g.points
        .iter()
        .enumerate()
        .filter(|(_, p)| !matches!(p.role, GripRole::Support | GripRole::Handle(_)))
        .min_by(|a, b| {
            let da = gt
                .mul_transform(a.1.local)
                .translation()
                .distance(hand.translation);
            let db = gt
                .mul_transform(b.1.local)
                .translation()
                .distance(hand.translation);
            da.total_cmp(&db)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn hand_of(
    players: &Query<(&Transform, &RigInput, &Children), With<Player>>,
    hands: &Query<(Entity, &RigHand, &mut Hand)>,
    player: Entity,
    side: Side,
) -> Option<(Entity, Transform)> {
    let (_, _, children) = players.get(player).ok()?;
    children.iter().find_map(|c| {
        hands
            .get(c)
            .ok()
            .filter(|(_, rh, _)| rh.side == side)
            .map(|(e, _, h)| (e, h.world))
    })
}

/// Drives held items toward their hands with velocities, so they still
/// collide. Heavy things and one-handed long guns are slower to swing.
fn follow_hands(
    hands: Query<&Hand>,
    mut held: Query<(
        &Held,
        Option<&Supported>,
        &Grabbable,
        &Position,
        &Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
        Option<&Mass>,
    )>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (h, support, grabbable, pos, rot, mut lin, mut ang, mass) in &mut held {
        let Ok(hand) = hands.get(h.hand) else {
            continue;
        };
        let mut target = hand.world.mul_transform(h.offset);
        let mass = mass.map_or(1.0, |m| m.0);
        let mut two_handed = false;
        if let Some(s) = support {
            if let (Ok(support_hand), Some(primary), Some(sp)) = (
                hands.get(s.hand),
                grabbable.points.get(h.point),
                grabbable.points.get(s.point),
            ) {
                // Point the item from the primary grip toward the support hand.
                let from = hand.world.translation;
                let to = support_hand.world.translation;
                let local_axis =
                    (sp.local.translation - primary.local.translation).normalize_or_zero();
                let world_axis = (to - from).normalize_or_zero();
                if local_axis != Vec3::ZERO && world_axis != Vec3::ZERO {
                    let up = hand.world.rotation * Vec3::Y;
                    // Rotation that takes local axis -> world axis, keeping roll from the primary hand.
                    let base = Transform::IDENTITY.looking_to(world_axis, up).rotation
                        * Transform::IDENTITY
                            .looking_to(local_axis, Vec3::Y)
                            .rotation
                            .inverse();
                    target.rotation = base;
                    target.translation = from - base * primary.local.translation;
                    two_handed = true;
                }
            }
        }
        // Velocity tracking with weight-dependent limits.
        let heft = (mass / if two_handed { 2.5 } else { 1.0 }).max(0.2);
        let max_lin = 14.0 / heft.sqrt();
        let max_ang = if two_handed { 30.0 } else { 22.0 } / heft.sqrt();
        if !target.translation.is_finite() || !target.rotation.is_finite() {
            continue;
        }
        let dv = (target.translation - pos.0) / dt;
        lin.0 = dv.clamp_length_max(max_lin);
        let dq = target.rotation * rot.0.inverse();
        let (axis, mut angle) = dq.to_axis_angle();
        if angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        let w = axis * angle / dt;
        ang.0 = if w.is_finite() {
            w.clamp_length_max(max_ang)
        } else {
            Vec3::ZERO
        };
    }
}
