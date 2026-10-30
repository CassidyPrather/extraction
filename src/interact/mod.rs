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
pub mod carry;
pub mod far;
pub mod fight;
pub mod first_aid;
pub mod flat;
pub mod gestures;
pub mod hand_climb;
#[cfg(test)]
mod physics_audit;
pub mod physics_hands;
pub mod poke;
pub mod read;
pub mod tools;
pub mod touch;
pub mod wounds;

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

/// How a held item handles, set by whoever owns its feel (the weapons
/// module: recoil, sway, a heavy gun's lag). The hand's target is smoothed
/// in the player's space (so walking or riding never trails it), then
/// `offset` (in the item's own space) is applied on top.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct HoldFeel {
    pub offset: Transform,
    /// Seconds: how softly the item follows the hand (0 snaps).
    pub lag: f32,
    /// The smoothed target, player space.
    pub smoothed: Option<Transform>,
}

/// Per-hand state, on the rig hand entity.
#[derive(Component, Default, Debug, Clone)]
pub struct Hand {
    pub holding: Option<Entity>,
    pub supporting: Option<Entity>,
    /// A moving part being worked: (item, handle id).
    pub handle: Option<(Entity, u8)>,
    /// Hand pose when the handle was grabbed, in the item's space.
    pub handle_origin: Vec3,
    pub grip_down: bool,
    /// Tracked fingers were closed last tick (for `Comfort::finger_grab`:
    /// closing them grabs, as the squeeze does).
    pub fingers_closed: bool,
    /// VR: the trigger is down (handles are worked with the trigger).
    pub trigger_down: bool,
    /// The handle in `handle` was taken with the trigger: its release ends
    /// it, not the grip's.
    pub handle_by_trigger: bool,
    /// VR: what the grip would take right now (for the glove's tint and a
    /// tick in the controller).
    pub hover: Option<Entity>,
    /// VR: what the empty hand points at out of reach (see [`far`]).
    pub far: Option<Entity>,
    /// A far grab on its way to this hand, and when it set off.
    pub pulling: Option<(Entity, f32)>,
    /// World-space velocity, smoothed, for throws.
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    last: Option<Transform>,
    /// World pose this tick.
    pub world: Transform,
    /// The hand's physics body (VR, see [`physics_hands`]), if it has one.
    pub body: Option<Entity>,
    /// Where that body got to: the hand you see, stopped at walls (`None`:
    /// no body, or the hand is holding something and rides on it).
    pub felt: Option<Transform>,
}

/// A box (in the item's space) that counts as reaching it: the backpack on
/// your back is taken by reaching over the shoulder anywhere behind the
/// head, not by finding its middle with your hand.
#[derive(Component, Clone, Copy, Debug)]
pub struct ReachZone {
    pub centre: Vec3,
    pub half: Vec3,
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

/// An item lying loose in the world: not in a hand, not in a socket (a
/// body slot, the belt, a vest pocket, the sling, a case, a gun) and not
/// in a pack (anyone's, or a dead raider's). What anything but the hands
/// may move, eat or despawn: machines, anomalies, the world.
pub type Loose = (With<crate::items::Item>, NotCarried);

/// Anything (an item or not) that isn't in a hand, a socket or a pack.
pub type NotCarried = (Without<Held>, Without<InSocket>, Without<body::InBackpack>);

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
    /// A hand came within reach of something it could take.
    Hover {
        item: Entity,
        side: Side,
    },
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
    /// A far grab set off toward a hand.
    Pulled {
        item: Entity,
        side: Side,
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
            .init_resource::<LeashBlocked>()
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
                    (
                        grip_from_input,
                        poke::poke_buttons,
                        far::far_grab,
                        apply_hand_actions,
                        seat_on_contact,
                        seat_from_body,
                    )
                        .chain()
                        .in_set(InteractSystems::Grab),
                    (drop_orphaned_holds, leash_check, follow_hands)
                        .chain()
                        .in_set(InteractSystems::Follow),
                ),
            )
            .add_observer(held_layers)
            .add_observer(poke::mount_button)
            .add_observer(unheld_layers)
            .add_plugins((
                body::BodyPlugin,
                physics_hands::PhysicsHandsPlugin,
                hand_climb::HandClimbPlugin,
                tools::ToolsPlugin,
                first_aid::FirstAidPlugin,
                gestures::GesturesPlugin,
                touch::TouchPlugin,
                carry::CarryPlugin,
                fight::FightPlugin,
                wounds::WoundsPlugin,
                read::ReadPlugin,
            ));
    }
}

/// What a free item collides with, and what one in a hand does: things in
/// your hands pass through each other (a magazine has to go up inside the
/// gun in your other hand; the first headset playtest found the gun's box
/// keeping it out), still meeting the world and robots.
pub(crate) const LOOSE: [Layer; 5] = [
    Layer::World,
    Layer::Item,
    Layer::Robot,
    Layer::Hand,
    Layer::Container,
];
const IN_HAND: [Layer; 2] = [Layer::World, Layer::Robot];

fn held_layers(add: On<Add, Held>, mut commands: Commands) {
    commands
        .entity(add.entity)
        .try_insert(CollisionLayers::new(Layer::Item, IN_HAND));
}

/// Back to colliding with items once let go, unless whatever took it from
/// the hand (the bag, a socket) set its own layers meanwhile.
fn unheld_layers(remove: On<Remove, Held>, mut commands: Commands) {
    let e = remove.entity;
    commands.queue(move |world: &mut World| {
        let held = CollisionLayers::new(Layer::Item, IN_HAND);
        if let Ok(mut ent) = world.get_entity_mut(e)
            && ent.get::<Held>().is_none()
            && ent.get::<CollisionLayers>() == Some(&held)
        {
            ent.insert(CollisionLayers::new(Layer::Item, LOOSE));
        }
    });
}

/// A magazine just pulled from a well (by its foot, or held out by the
/// release): it doesn't seat back in that well until it's been drawn clear
/// of it ([`CLEAR_OF_WELL`]).
#[derive(Component, Debug)]
pub struct Clearing(pub Entity);

/// How far a pulled magazine's top must get from the well it left before
/// it can seat there again, metres.
pub const CLEAR_OF_WELL: f32 = 0.08;

/// A magazine brought up into the well of the gun in your other hand seats
/// as it arrives: in a headset you don't let go of it first.
fn seat_on_contact(
    mut commands: Commands,
    mut hands: Query<(Entity, &RigHand, &mut Hand)>,
    mags: Query<(
        Entity,
        &Held,
        &GlobalTransform,
        &crate::weapons::Mag,
        Option<&Clearing>,
    )>,
    mut sockets: Query<(Entity, &mut Socket, &GlobalTransform, &SocketOwner)>,
    held: Query<&Held>,
    mut events: MessageWriter<InteractEvent>,
) {
    for (mag_e, h, gt, mag, clearing) in &mags {
        let top = gt.transform_point(Vec3::Y * mag.height * 0.5);
        for (socket_e, mut socket, sgt, owner) in &mut sockets {
            let SocketKind::MagWell(family) = &socket.kind else {
                continue;
            };
            // Just pulled from this well: not back in until drawn clear.
            if clearing.is_some_and(|c| c.0 == socket_e) {
                if top.distance(sgt.translation()) > CLEAR_OF_WELL {
                    commands.entity(mag_e).remove::<Clearing>();
                }
                continue;
            }
            if socket.occupant.is_some()
                || *family != mag.family
                || owner.0 == mag_e
                || held.get(owner.0).map_or(true, |g| g.player != h.player)
            {
                continue;
            }
            if top.distance(sgt.translation()) > 0.045 {
                continue;
            }
            if let Ok((_, _, mut hand)) = hands.get_mut(h.hand) {
                hand.holding = None;
            }
            seat_in_socket(&mut commands, mag_e, socket_e, &mut socket);
            events.write(InteractEvent::Socketed {
                item: mag_e,
                socket: socket_e,
            });
            break;
        }
    }
}

/// Played one-handed (`Comfort::one_handed`), the gun's empty well brought
/// down onto a magazine in a pouch or pocket takes it: there's no other
/// hand to bring the magazine up.
fn seat_from_body(
    mut commands: Commands,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mags: Query<(Entity, &InSocket, &GlobalTransform, &crate::weapons::Mag)>,
    mut sockets: Query<(Entity, &mut Socket, &GlobalTransform, &SocketOwner)>,
    held: Query<&Held>,
    mut events: MessageWriter<InteractEvent>,
) {
    if !comfort.is_some_and(|c| c.one_handed) {
        return;
    }
    let mut moves = Vec::new();
    for (mag_e, in_socket, gt, mag) in &mags {
        let Ok((_, from, _, from_owner)) = sockets.get(in_socket.socket) else {
            continue;
        };
        let SocketKind::Body(_) = from.kind else {
            continue;
        };
        let top = gt.transform_point(Vec3::Y * mag.height * 0.5);
        let well = sockets.iter().find(|(_, s, sgt, owner)| {
            matches!(&s.kind, SocketKind::MagWell(f) if *f == mag.family)
                && s.occupant.is_none()
                && held.get(owner.0).is_ok_and(|g| g.player == from_owner.0)
                && top.distance(sgt.translation()) < BODY_SEAT
        });
        if let Some((well_e, ..)) = well {
            moves.push((mag_e, in_socket.socket, well_e));
        }
    }
    for (mag_e, from_e, well_e) in moves {
        if let Ok((_, mut from, ..)) = sockets.get_mut(from_e) {
            from.occupant = None;
        }
        events.write(InteractEvent::Unsocketed {
            item: mag_e,
            socket: from_e,
        });
        if let Ok((_, mut well, ..)) = sockets.get_mut(well_e) {
            if well.occupant.is_some() {
                continue;
            }
            seat_in_socket(&mut commands, mag_e, well_e, &mut well);
        }
        events.write(InteractEvent::Socketed {
            item: mag_e,
            socket: well_e,
        });
    }
}

/// How far a thing taken by a free grip may be from the hand's middle, at
/// its nearest side, and stay where it's taken (m); further, it's brought
/// in to the hand.
const PALM_REACH: f32 = 0.05;

/// The fastest anything leaves a hand, m/s: a hard overarm throw. A
/// controller's tracking jumping (or the playspace moving) while it holds
/// something would otherwise let go of it at hundreds of m/s.
pub const MAX_THROW: f32 = 25.0;
/// And the fastest it spins, rad/s.
pub const MAX_THROW_SPIN: f32 = 40.0;

/// How near a pouched magazine's top the gun's well has to come, one-handed.
pub const BODY_SEAT: f32 = 0.06;

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

/// What the hand's search sees of each grabbable.
type GrabTargets<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GlobalTransform,
        &'static Grabbable,
        Option<&'static Held>,
        Has<ColliderDisabled>,
        Option<&'static Visibility>,
        Option<&'static ReachZone>,
        // Clamped in a vice: its moving parts are worked without holding it.
        Has<crate::weapons::gunsmith::InVice>,
        // Seated in something (a magazine in its well, a scope on its
        // mount, a pistol in its holster): see `find_grab`.
        Option<&'static InSocket>,
    ),
>;

/// How near a seated part's foot (a magazine's floorplate under the well)
/// a hand must close to pull it from a host held in the other hand, metres.
pub const SEATED_REACH: f32 = 0.06;
/// What a seated part gives away to its host in a close call, metres: a
/// hand on the gun's grip takes the gun, one right at the floorplate the
/// magazine.
pub const SEATED_BIAS: f32 = 0.025;

/// VR path. The grip takes hold of things (and presses doors, cards,
/// cords); the trigger works a moving part (bolt, slide, charging handle)
/// with an empty hand, and lets it go when released. On a hand holding a
/// gun the trigger fires instead (`weapons`). Releases become drops or
/// socketings.
fn grip_from_input(
    players: Query<(Entity, &RigInput, &Children), With<Player>>,
    mut hands: Query<(Entity, &RigHand, &mut Hand)>,
    grabbables: GrabTargets,
    parents: Query<&ChildOf>,
    seats: Seats,
    torsos: Query<(&GlobalTransform, &ChildOf), With<body::Torso>>,
    bags: Query<(), With<body::Backpack>>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    driver: Res<crate::rig::RigDriver>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    spatial: SpatialQuery,
    terrain: Query<Entity, With<crate::world::TerrainCollider>>,
    mut actions: MessageWriter<HandAction>,
    mut events: MessageWriter<InteractEvent>,
) {
    let terrain = terrain.iter().next();
    for (player, input, children) in &players {
        let torso = torsos
            .iter()
            .find(|(_, p)| p.parent() == player)
            .map(|(t, _)| *t);
        for child in children.iter() {
            let Ok((_, rh, mut hand)) = hands.get_mut(child) else {
                continue;
            };
            if !input.hand(rh.side).tracked {
                continue;
            }
            let grip = input.hand(rh.side).grip;
            let trigger = input.hand(rh.side).trigger;
            let busy = hand.holding.is_some() || hand.supporting.is_some() || hand.handle.is_some();
            // A held thing goes when the grip opens and, where the fingers
            // are tracked (Index controllers), the fingers open too: a
            // relaxed hand reads no squeeze but still wraps the gun.
            let wrapped = input
                .hand(rh.side)
                .fingers
                .is_some_and(|f| crate::rig::fingers::wrapped(&f));
            // With the comfort setting on, fingers closing take hold too.
            let finger_grab =
                comfort.as_ref().is_some_and(|c| c.finger_grab) && wrapped && !hand.fingers_closed;
            hand.fingers_closed = wrapped;
            let pressed = !hand.grip_down && (grip > 0.6 || finger_grab);
            let holds = hand.holding.is_some() || hand.supporting.is_some();
            let released = hand.grip_down && grip < 0.35 && !(holds && wrapped);
            // What the grip would take: the glove tints and the controller
            // ticks as something comes into reach.
            let hover = if busy || !locals.contains(player) || *driver != crate::rig::RigDriver::Xr
            {
                None
            } else {
                find_grab(
                    &hand.world,
                    player,
                    Reach::Hover,
                    &grabbables,
                    &parents,
                    &seats,
                    &spatial,
                    terrain,
                )
                .map(|(e, _)| e)
            };
            if hover != hand.hover {
                hand.hover = hover;
                if let Some(item) = hover {
                    events.write(InteractEvent::Hover {
                        item,
                        side: rh.side,
                    });
                }
            }
            if pressed {
                hand.grip_down = true;
                if !busy {
                    if let Some((item, point)) = find_grab(
                        &hand.world,
                        player,
                        Reach::Grip,
                        &grabbables,
                        &parents,
                        &seats,
                        &spatial,
                        terrain,
                    ) {
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
                // Let go over the shoulder (above it, behind the neck): into
                // the pack.
                let over_shoulder = torso.is_some_and(|t| {
                    let local = t
                        .affine()
                        .inverse()
                        .transform_point3(hand.world.translation);
                    local.y > 0.05 && local.z > 0.0 && local.x.abs() < 0.45
                });
                if over_shoulder && hand.holding.is_some_and(|h| !bags.contains(h)) {
                    actions.write(HandAction::Stow {
                        player,
                        side: rh.side,
                    });
                    continue;
                }
                // A handle held on the trigger stays held.
                if !(hand.handle_by_trigger && hand.holding.is_none() && hand.supporting.is_none())
                {
                    actions.write(HandAction::Release {
                        player,
                        side: rh.side,
                        throw: 0.0,
                    });
                }
            }
            let t_pressed = !hand.trigger_down && trigger > 0.6;
            let t_released = hand.trigger_down && trigger < 0.35;
            if t_pressed {
                hand.trigger_down = true;
                if !busy {
                    if let Some((item, point)) = find_grab(
                        &hand.world,
                        player,
                        Reach::Handle,
                        &grabbables,
                        &parents,
                        &seats,
                        &spatial,
                        terrain,
                    ) {
                        hand.handle_by_trigger = true;
                        actions.write(HandAction::Grab {
                            player,
                            side: rh.side,
                            item,
                            point: Some(point),
                        });
                    }
                }
            } else if t_released {
                hand.trigger_down = false;
                if hand.handle_by_trigger {
                    hand.handle_by_trigger = false;
                    if hand.holding.is_none() && hand.supporting.is_none() {
                        actions.write(HandAction::Release {
                            player,
                            side: rh.side,
                            throw: 0.0,
                        });
                    }
                }
            }
        }
    }
}

/// What a hand reaches for: things to hold (grip), or moving parts to work
/// (trigger).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reach {
    Grip,
    Handle,
    /// What the grip would take, asked every tick: items only (a door's
    /// leaf is found when you actually squeeze).
    Hover,
}

/// Nearest grab point in reach of a hand. Points with a radius must be
/// within it; free items are found by touching their collider, or a
/// collider of theirs on a child (doors: the leaf is the world's).
/// What a seated part is seated in: its socket's owner, and a magazine's
/// height (its foot is where it's pulled from).
type Seats<'w, 's> = (
    Query<'w, 's, &'static SocketOwner>,
    Query<'w, 's, &'static crate::weapons::Mag>,
    // Held half out by the mag release: taken anywhere along it.
    Query<'w, 's, (), With<crate::weapons::reload::Presented>>,
);

fn find_grab(
    hand: &Transform,
    player: Entity,
    reach_for: Reach,
    grabbables: &GrabTargets,
    parents: &Query<&ChildOf>,
    (owners, mags, presented): &Seats,
    spatial: &SpatialQuery,
    terrain: Option<Entity>,
) -> Option<(Entity, usize)> {
    let reach = Collider::sphere(0.07);
    let mut touching = spatial.shape_intersections(
        &reach,
        hand.translation,
        Quat::IDENTITY,
        &SpatialQueryFilter::from_mask([Layer::Item]),
    );
    if reach_for == Reach::Grip {
        for hit in spatial.shape_intersections(
            &reach,
            hand.translation,
            Quat::IDENTITY,
            // (A wreck's limbs are the robot's parts: `carry::Draggable`.)
            // Not the terrain: nothing on it is grabbed, and a sphere's
            // test against the map's heightfield is 28 ms
            // (`world::sphere_in_world`).
            &SpatialQueryFilter::from_mask([Layer::World, Layer::Robot])
                .with_excluded_entities(terrain),
        ) {
            let mut up = hit;
            for _ in 0..4 {
                if grabbables.contains(up) {
                    touching.push(up);
                    break;
                }
                let Ok(p) = parents.get(up) else {
                    break;
                };
                up = p.parent();
            }
        }
    }
    let mut best: Option<(f32, Entity, usize)> = None;
    for (e, gt, g, held, disabled, vis, zone, vice, seat) in grabbables.iter() {
        // Moving parts are worked on a gun already in your other hand, or
        // in a vice.
        if reach_for == Reach::Handle && !vice && held.is_none_or(|h| h.player != player) {
            continue;
        }
        // Out of sight (in a closed bag) is out of reach.
        if vis == Some(&Visibility::Hidden) {
            continue;
        }
        // A part seated in another thing (a magazine in its well, a scope
        // on its mount) is never what a grab takes: the thing it's in is.
        // It comes out by its own control (the mag release), or by a
        // deliberate pull on a host already in the other hand, taken by
        // its foot. (A pouch or holster is the player's, not a thing's.)
        let host = seat
            .and_then(|s| owners.get(s.socket).ok())
            .map(|o| o.0)
            .filter(|o| grabbables.contains(*o));
        if let Some(host) = host {
            let host_held = grabbables
                .get(host)
                .is_ok_and(|h| h.3.is_some_and(|h| h.player == player));
            if !host_held || reach_for == Reach::Handle {
                continue;
            }
            let foot = mags.get(e).map_or(gt.translation(), |m| {
                gt.translation() - gt.rotation() * Vec3::Y * (m.height * 0.5)
            });
            let d = if presented.contains(e) {
                foot.distance(hand.translation)
                    .min(gt.translation().distance(hand.translation))
            } else {
                foot.distance(hand.translation)
            };
            if d <= SEATED_REACH && best.is_none_or(|b| d + SEATED_BIAS < b.0) {
                let point = g
                    .points
                    .iter()
                    .position(|p| p.role == GripRole::Free)
                    .unwrap_or(0);
                best = Some((d + SEATED_BIAS, e, point));
            }
            continue;
        }
        // Nothing reaches more than an arm's length past its own middle
        // (a rifle's grips, the pack's zone): cull the rest before looking
        // at points. This runs every tick for each hand.
        // (Touching it counts whatever its size: a door's middle is its hinge.)
        if gt.translation().distance_squared(hand.translation) > 1.2 * 1.2 && !touching.contains(&e)
        {
            continue;
        }
        if let (Some(z), Reach::Grip | Reach::Hover) = (zone, reach_for) {
            let local = gt.affine().inverse().transform_point3(hand.translation) - z.centre;
            if local.abs().cmple(z.half).all() {
                // Anything you've actually got your hand on comes first;
                // of two zones, the smaller (a slung gun's over the
                // pack's) is the one meant.
                let d = 0.12 * (z.half.length() / 0.6).min(1.0);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, e, 0));
                }
                continue;
            }
        }
        for (i, p) in g.points.iter().enumerate() {
            let is_handle = matches!(p.role, GripRole::Handle(_));
            if is_handle != (reach_for == Reach::Handle) {
                continue;
            }
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
        Has<carry::Bulky>,
    )>,
    mut sockets: Query<(Entity, &GlobalTransform, &mut Socket, Option<&SocketOwner>)>,
    mut backpacks: Query<(Entity, &GlobalTransform, &mut body::Backpack)>,
    catalog: Res<Catalog>,
    pressables: Query<(), With<crate::raid::truck::Pressable>>,
    // Things to take up and read (`read`): theirs to handle.
    reading: (Query<(), With<read::Readable>>, Res<crate::rig::RigDriver>),
    // The stash crate stays put; a stone just dropped too hot isn't taken.
    fixed: (
        Query<(), With<crate::raid::truck::Stash>>,
        Query<(), With<crate::anomaly::harvest::Scalded>>,
        Query<(), With<crate::weapons::gunsmith::InVice>>,
    ),
    // Things held where the hand is (a rope, a ladder).
    holds: (
        Query<&crate::gadgets::climbing::Rope>,
        Query<(), With<hand_climb::Ladder>>,
    ),
    time: Res<Time>,
    slot_places: Query<&Transform, With<body::BodySlot>>,
    bounds: Query<&ColliderAabb>,
) {
    let (ropes, ladders) = holds;
    for action in actions.read() {
        match *action {
            HandAction::Grab {
                player, side, item, ..
            } if ropes.contains(item) => {
                // A rope stays put: the hand takes hold of it instead.
                let (Some((hand_e, hand_world)), Ok(rope)) =
                    (hand_of(&players, &hands, player, side), ropes.get(item))
                else {
                    continue;
                };
                commands
                    .entity(hand_e)
                    .insert(crate::gadgets::climbing::RopeGrip {
                        rope: item,
                        at: rope.nearest(hand_world.translation),
                        since: time.elapsed_secs(),
                        speed: 0.0,
                    });
                events.write(InteractEvent::Grabbed { item, side });
            }
            HandAction::Grab {
                player, side, item, ..
            } if ladders.contains(item) => {
                // A ladder: the hand holds on where it is.
                let Some((hand_e, hand_world)) = hand_of(&players, &hands, player, side) else {
                    continue;
                };
                commands.entity(hand_e).insert(hand_climb::HandHold {
                    on: hand_climb::Hold::Ladder(item),
                    at: hand_world.translation,
                    since: time.elapsed_secs(),
                });
                events.write(InteractEvent::Grabbed { item, side });
            }
            HandAction::Grab { item, .. }
                if reading.0.contains(item)
                    && read::reads(*reading.1, pressables.contains(item)) => {}
            HandAction::Grab {
                player,
                side,
                item,
                point,
            } if pressables.contains(item) => {
                let _ = (side, point);
                events.write(InteractEvent::Pressed { item, player });
            }
            HandAction::Grab { item, .. } if fixed.0.contains(item) || fixed.1.contains(item) => {}
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
                let Ok((gt, grabbable, held, in_socket, is_item, bulky)) = items.get(item) else {
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
                    // A second hand on something bulky takes its other end:
                    // carried in both hands (`carry`).
                    _ if bulky && held.is_some_and(|h| h.player == player && h.hand != hand_e) => {
                        let other = held.map_or(0, |h| h.point);
                        let point = if point == other {
                            (other + 1) % grabbable.points.len().max(1)
                        } else {
                            point
                        };
                        hand.supporting = Some(item);
                        commands.entity(item).insert(Supported {
                            hand: hand_e,
                            side,
                            point,
                        });
                        events.write(InteractEvent::Grabbed { item, side });
                    }
                    // A gun in the vice: its bolt is worked where it is.
                    GripRole::Handle(id) if fixed.2.contains(item) => {
                        hand.handle = Some((item, id));
                        hand.handle_origin = gt
                            .affine()
                            .inverse()
                            .transform_point3(hand_world.translation);
                        events.write(InteractEvent::Grabbed { item, side });
                    }
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
                                if matches!(socket.kind, SocketKind::MagWell(_)) {
                                    commands.entity(item).insert(Clearing(s.socket));
                                }
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
                        let mut offset = Transform::from_matrix(offset.into());
                        // A thing held touches the hand. Taken from further
                        // off (caught by a far grab, snatched as it flew
                        // past), it comes in until its nearest side is at the
                        // hand, as it's turned: held where it was caught, it
                        // hung a foot ahead of the hand.
                        if gp.role == GripRole::Free
                            && is_item.is_some()
                            && let Ok(aabb) = bounds.get(item)
                        {
                            // Its size from the bounds, its place from where
                            // it is: a socketed or bagged thing's bounds are
                            // stale (its collider was off), and from those it
                            // was pulled anywhere.
                            let half = ((aabb.max - aabb.min) * 0.5).min(Vec3::splat(1.0));
                            let c = gt.translation();
                            let at = hand_world.translation;
                            let gap = at.clamp(c - half, c + half) - at;
                            if gap.length() > PALM_REACH {
                                offset.translation -= hand_world.rotation.inverse() * gap;
                            }
                        }
                        hand.holding = Some(item);
                        let world = hand_world.mul_transform(offset);
                        // Something bulky doesn't take on the hand's speed as
                        // it's taken hold of: it starts from where it lay.
                        let v = if bulky {
                            Vec3::ZERO
                        } else {
                            hand.velocity.clamp_length_max(MAX_THROW)
                        };
                        physics_on(&mut commands, item, world, v, Vec3::ZERO, false);
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
                commands
                    .entity(hand_e)
                    .remove::<(crate::gadgets::climbing::RopeGrip, hand_climb::HandHold)>();
                let Ok((_, _, mut hand)) = hands.get_mut(hand_e) else {
                    continue;
                };
                hand.handle = None;
                hand.handle_by_trigger = false;
                if let Some(item) = hand.supporting.take() {
                    commands.entity(item).remove::<Supported>();
                }
                let Some(item) = hand.holding.take() else {
                    continue;
                };
                let mut velocity =
                    (hand.velocity + hand_world.forward() * throw).clamp_length_max(MAX_THROW);
                let mut angular = hand.angular_velocity.clamp_length_max(MAX_THROW_SPIN);
                // Something bulky is shoved, not thrown: an arm puts so much
                // into it, and it hardly turns (`carry`).
                if let Ok((.., it, true)) = items.get(item) {
                    // (A wreck has no catalog entry: about a walker's hull.)
                    let m = it.map_or(80.0, |it| catalog.get(it.kind()).mass).max(0.5);
                    velocity = hand
                        .velocity
                        .clamp_length_max((2.0 * carry::SHOVE / m).sqrt());
                    angular *= 0.2;
                }
                commands.entity(item).remove::<(Held, Supported)>();
                // Any other hand supporting it lets go too.
                for (_, _, mut h) in &mut hands {
                    if h.supporting == Some(item) {
                        h.supporting = None;
                    }
                }
                let Ok((gt, _, _, _, it, _)) = items.get(item) else {
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
                    let mut d = sgt.translation().distance(pos);
                    let mut radius = socket.radius;
                    // A long gun let go near a shoulder goes onto that side's
                    // sling, front or back.
                    if socket.kind == SocketKind::Body(SlotKind::Sling) {
                        if let Ok(local) = slot_places.get(se) {
                            let shoulder =
                                sgt.transform_point(body::sling_shoulder(local.translation));
                            d = d.min(shoulder.distance(pos));
                            radius = radius.max(body::SLING_REACH);
                        }
                    }
                    if d < radius && target.is_none_or(|(bd, _)| d < bd) {
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
                                // As it was put in, at whatever angle.
                                let posed = Transform::from_matrix(
                                    (bgt.affine().inverse() * gt.affine()).into(),
                                );
                                body::stow_posed(&mut commands, item, be, &mut bag, posed);
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
                let Ok((_, _, _, _, Some(it), _)) = items.get(item) else {
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
                    .map(|(e, _, s, _)| (slot_rank(&s.kind, def.magazine.is_some()), e))
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
        Has<carry::Bulky>,
    )>,
    contents: &[Entity],
) -> Vec<extraction_core::items::ItemKind> {
    contents
        .iter()
        .filter_map(|e| {
            items
                .get(*e)
                .ok()
                .and_then(|(.., it, _)| it.map(|i| i.kind()))
        })
        .collect()
}

/// Which free slot a put-away item goes to first. The pouches are kept for
/// magazines (where a reload reaches, and where a one-handed one takes
/// them from, `seat_from_body`): anything else goes to a pocket first, a
/// pouch only when nothing else on the body is free.
fn slot_rank(kind: &SocketKind, magazine: bool) -> u8 {
    match (kind, magazine) {
        (SocketKind::Body(SlotKind::Pouch), true) => 0,
        (SocketKind::Body(SlotKind::Pocket), _) => 1,
        (SocketKind::Body(SlotKind::Mount), _) => 2,
        (SocketKind::Body(SlotKind::Holster), _) => 3,
        (SocketKind::Body(SlotKind::Sling), _) => 4,
        (SocketKind::Body(SlotKind::Pouch), false) => 5,
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
        // Worn things go on only where they're worn.
        SocketKind::Body(SlotKind::Vest) => mount == Some(extraction_core::items::Mount::Vest),
        SocketKind::Body(SlotKind::Head) => mount == Some(extraction_core::items::Mount::Helmet),
        // A sling's for a long gun: a car door or a heavy's claw is carried
        // in the hands (`carry`).
        SocketKind::Body(SlotKind::Sling) => is_gun && SlotKind::Sling.accepts(size),
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
/// How far a held item can lag its hand before it's put back in it.
const LEASH: f32 = 0.6;
/// Past this the item goes back to the hand even through a wall.
const LEASH_FAR: f32 = 2.0;

/// Held items with something solid between them and their hand this tick
/// (`leash_check`): the leash doesn't snap them back through it.
#[derive(Resource, Default)]
pub struct LeashBlocked(pub std::collections::HashSet<Entity>);

/// Which held items have a wall between them and where their hand wants
/// them (worked out apart from `follow_hands`, which moves bodies the
/// spatial query reads).
fn leash_check(
    hands: Query<&Hand>,
    held: Query<(Entity, &Held, &Position)>,
    spatial: SpatialQuery,
    mut blocked: ResMut<LeashBlocked>,
) {
    blocked.0.clear();
    for (e, h, pos) in &held {
        let Ok(hand) = hands.get(h.hand) else {
            continue;
        };
        let gap = hand.world.mul_transform(h.offset).translation - pos.0;
        if gap.length() <= LEASH {
            continue;
        }
        if let Ok(dir) = Dir3::new(gap)
            && spatial
                .cast_ray(
                    pos.0,
                    dir,
                    gap.length(),
                    true,
                    &SpatialQueryFilter::from_mask(Layer::World),
                )
                .is_some()
        {
            blocked.0.insert(e);
        }
    }
}

/// Something marked held by a hand that's let go of it, or taken hold of
/// something else since, drops. (A hoarder's hand took one thing after
/// another; the stone it had held stayed stuck to it, hot, and nothing
/// could make it let go.)
fn drop_orphaned_holds(mut commands: Commands, held: Query<(Entity, &Held)>, hands: Query<&Hand>) {
    for (e, h) in &held {
        if hands.get(h.hand).is_ok_and(|hand| hand.holding != Some(e)) {
            commands
                .entity(e)
                .remove::<(Held, Supported)>()
                .insert(GravityScale(1.0));
        }
    }
}

/// A support point nearer the primary grip than this steadies the item but
/// doesn't aim it, metres.
pub const CUP_SPAN: f32 = 0.12;

pub(crate) fn follow_hands(
    hands: Query<&Hand>,
    mut held: Query<(
        Entity,
        &Held,
        Option<&Supported>,
        &Grabbable,
        &mut Position,
        &mut Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
        Option<&Mass>,
        Option<&mut HoldFeel>,
        Has<carry::Bulky>,
        Has<carry::Draggable>,
    )>,
    // The pack hangs from the hand by its own rule (`body::hang_held_pack`).
    packs: Query<(), With<body::Backpack>>,
    roots: Query<&GlobalTransform, With<Player>>,
    blocked: Res<LeashBlocked>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (e, h, support, grabbable, mut pos, mut rot, mut lin, mut ang, mass, feel, bulky, wreck) in
        &mut held
    {
        // One hand on something bulky drags it, and a wreck is only ever
        // dragged (`carry`). A pack hangs under the hand: followed here as
        // well, by the offset it was taken off the shoulder at, it was
        // snapped back to that (turned with the wrist, or onto you on a
        // step) whenever the two drew apart.
        if (bulky && support.is_none()) || wreck || packs.contains(e) {
            continue;
        }
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
                // Point the item from the primary grip toward the support
                // hand; a support this close (a palm cupped under a
                // pistol's grip) steadies it without aiming it.
                let from = hand.world.translation;
                let to = support_hand.world.translation;
                let span = sp.local.translation - primary.local.translation;
                let local_axis = if span.length() < CUP_SPAN {
                    Vec3::ZERO
                } else {
                    span.normalize_or_zero()
                };
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
        if let (Some(mut feel), Ok(root)) = (feel, roots.get(h.player)) {
            let root = root.compute_transform();
            let local = Transform::from_matrix(
                (root.compute_affine().inverse() * target.compute_affine()).into(),
            );
            let k = if feel.lag > 0.0 {
                1.0 - (-dt / feel.lag).exp()
            } else {
                1.0
            };
            let s = feel.smoothed.map_or(local, |s| Transform {
                translation: s.translation.lerp(local.translation, k),
                rotation: s.rotation.slerp(local.rotation, k),
                scale: Vec3::ONE,
            });
            feel.smoothed = Some(s);
            target = root.mul_transform(s).mul_transform(feel.offset);
        }
        // Snagged on a door frame and left behind: back into the hand, if
        // nothing solid is between (a gun pushed into a wall stays this
        // side of it), or come what may when the hand is far away.
        let gap = target.translation - pos.0;
        let clear = || !blocked.0.contains(&e);
        if gap.length() > LEASH_FAR || (gap.length() > LEASH && clear()) {
            pos.0 = target.translation;
            rot.0 = target.rotation;
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
