//! Physics hands (VR): each tracked hand of the local player is also a
//! body chased toward the controller's pose, so it stops at walls, tables
//! and doors and shoves loose things, as in Into the Radius. A palm pushed
//! through a door's leaf swings it open (or shut from the other side).
//!
//! What the game *does* with the hand (grabbing, buttons, bots, flat mode)
//! still goes by the tracked pose in [`Hand::world`]: the body only stops
//! the hand you see ([`Hand::felt`]; the glove, with a faint ghost at the
//! real pose when they part) and pushes things. A hand holding something
//! leaves the work to the held item, which is a body already
//! (`follow_hands`): its own collider meets the world, and the glove rides
//! on its grip.
//!
//! Loose items get pushed only while nothing is in the hand's reach: a hand
//! closing on a magazine mustn't knock it off the table first.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Hand, InteractSystems};
use crate::rig::{LocalPlayer, RigDriver, RigHand};
use crate::world::Layer;

/// The hand's box in its own frame (-Z where it points, +Y up, the palm
/// facing in): a fist round the controller, fingers out ahead.
pub const HAND_HALF: Vec3 = Vec3::new(0.03, 0.045, 0.08);
/// Where that box sits from the hand's grip point (the middle of the fist),
/// toward the fingers.
pub const HAND_CENTRE: Vec3 = Vec3::new(0.0, 0.0, -0.01);
/// How fast the body may chase the controller, m/s and rad/s: quick enough
/// to feel like your hand, slow enough not to punch through a wall.
const MAX_SPEED: f32 = 8.0;
const MAX_SPIN: f32 = 40.0;
/// How long after letting go a hand leaves loose things alone, seconds.
const QUIET: f32 = 0.4;
/// How far past a door's leaf (its middle) a palm has to reach to push it.
const DOOR_PUSH: f32 = 0.04;

pub struct PhysicsHandsPlugin;

impl Plugin for PhysicsHandsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (spawn_hand_bodies, drive_hand_bodies, push_doors)
                .chain()
                .after(InteractSystems::Grab)
                .before(InteractSystems::Follow),
        )
        .add_systems(FixedPostUpdate, feel_hands.after(PhysicsSystems::Writeback));
    }
}

/// A hand's body: chases [`Hand::world`] (the entity is the rig hand's).
#[derive(Component, Debug)]
pub struct HandBody {
    pub hand: Entity,
    /// Seconds left before it shoves loose things again: a hand that has
    /// just let go mustn't knock what it threw (it's where that was).
    pub quiet: f32,
}

/// One body per headset hand of the local player, made when the headset
/// driver is in charge (never for bots or flat mode).
fn spawn_hand_bodies(
    mut commands: Commands,
    driver: Res<RigDriver>,
    mut hands: Query<(Entity, &RigHand, &mut Hand, &ChildOf)>,
    locals: Query<(), With<LocalPlayer>>,
) {
    if *driver != RigDriver::Xr {
        return;
    }
    for (e, rh, mut hand, parent) in &mut hands {
        if hand.body.is_some() || !locals.contains(parent.parent()) {
            continue;
        }
        let at = hand.world;
        let body = commands
            .spawn((
                Name::new(format!("{:?} hand body", rh.side)),
                HandBody {
                    hand: e,
                    quiet: 0.0,
                },
                RigidBody::Dynamic,
                GravityScale(0.0),
                // A hand's weight behind it, so it shoves a tin rather than
                // bouncing off it.
                Mass(2.0),
                NoAutoMass,
                SweptCcd::default(),
                // What it meets, felt (`touch`).
                CollisionEventsEnabled,
                Collider::compound(vec![(
                    HAND_CENTRE,
                    Quat::IDENTITY,
                    Collider::cuboid(HAND_HALF.x * 2.0, HAND_HALF.y * 2.0, HAND_HALF.z * 2.0),
                )]),
                CollisionLayers::new(Layer::Hand, [Layer::World]),
                Position(at.translation),
                Rotation(at.rotation),
                Transform::from_isometry(at.to_isometry()),
            ))
            .id();
        hand.body = Some(body);
        hand.felt = Some(at);
    }
}

/// Velocity toward the tracked pose (a stiff, damped spring, as fast as the
/// physics tick allows), capped. While the hand holds something, the body
/// steps aside (no collider) and waits at the hand.
#[allow(clippy::type_complexity)]
fn drive_hand_bodies(
    mut commands: Commands,
    hands: Query<&Hand>,
    mut bodies: Query<(
        Entity,
        &mut HandBody,
        &mut Position,
        &mut Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
    )>,
    layers: Query<&CollisionLayers>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (e, mut body, mut pos, mut rot, mut lin, mut ang) in &mut bodies {
        let Ok(hand) = hands.get(body.hand) else {
            commands.entity(e).despawn();
            continue;
        };
        let target = hand.world;
        let busy = hand.holding.is_some() || hand.handle.is_some();
        // Loose things only when nothing's in reach to take; never what it
        // holds (the held item's own body does that work).
        body.quiet = if busy {
            QUIET
        } else {
            (body.quiet - dt).max(0.0)
        };
        let want = if busy {
            CollisionLayers::new(Layer::Hand, LayerMask::NONE)
        } else if hand.hover.is_none() && body.quiet <= 0.0 {
            CollisionLayers::new(Layer::Hand, [Layer::World, Layer::Item])
        } else {
            CollisionLayers::new(Layer::Hand, [Layer::World])
        };
        if layers.get(e).is_ok_and(|l| *l != want) {
            commands.entity(e).insert(want);
        }
        if busy {
            pos.0 = target.translation;
            rot.0 = target.rotation;
            lin.0 = Vec3::ZERO;
            ang.0 = Vec3::ZERO;
            continue;
        }
        // Far behind (a teleport, the first tick): put it back.
        if pos.0.distance(target.translation) > 1.0 {
            pos.0 = target.translation;
            rot.0 = target.rotation;
        }
        lin.0 = ((target.translation - pos.0) / dt).clamp_length_max(MAX_SPEED);
        let dq = target.rotation * rot.0.inverse();
        let (axis, mut angle) = dq.to_axis_angle();
        if angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        ang.0 = (axis * angle / dt).clamp_length_max(MAX_SPIN);
    }
}

/// Where each hand's body ended up this tick: the hand you see.
fn feel_hands(bodies: Query<(&HandBody, &Position, &Rotation)>, mut hands: Query<&mut Hand>) {
    for (body, pos, rot) in &bodies {
        if let Ok(mut hand) = hands.get_mut(body.hand) {
            let felt = if hand.holding.is_some() || hand.handle.is_some() {
                None
            } else {
                Some(Transform::from_translation(pos.0).with_rotation(rot.0))
            };
            if hand.felt != felt {
                hand.felt = felt;
            }
        }
    }
}

/// A palm pushed through a door's leaf, from the side it swings away
/// from, opens it; from the other side, shuts it. (Gripping its edge still
/// toggles it, `world::buildings::swing_doors`.) Wedged and power-locked
/// doors don't move.
#[allow(clippy::type_complexity)]
fn push_doors(
    hands: Query<&Hand>,
    bodies: Query<(&HandBody, &Position)>,
    mut doors: Query<(
        &mut crate::world::buildings::Door,
        &GlobalTransform,
        &crate::world::buildings::DoorLeaf,
        Has<crate::robots::doors::Wedged>,
        Has<crate::world::underground::PowerLocked>,
    )>,
) {
    for (body, pos) in &bodies {
        let Ok(hand) = hands.get(body.hand) else {
            continue;
        };
        if hand.holding.is_some() || hand.handle.is_some() {
            continue;
        }
        let palm = hand.world.transform_point(HAND_CENTRE);
        for (mut door, gt, leaf, wedged, locked) in &mut doors {
            if wedged || locked {
                continue;
            }
            let to_local = gt.affine().inverse();
            let real = to_local.transform_point3(palm);
            let felt = to_local.transform_point3(pos.0);
            let (w, h) = (leaf.0.width, leaf.0.height);
            if real.x < 0.05 || real.x > w || real.y < 0.0 || real.y > h {
                continue;
            }
            // The leaf swings toward its -Z: a hand held back on +Z while
            // the real one reaches through pushes it open, and the reverse.
            if !door.open && felt.z > 0.0 && real.z < -DOOR_PUSH {
                door.open = true;
            } else if door.open && felt.z < 0.0 && real.z > DOOR_PUSH {
                door.open = false;
            }
        }
    }
}
