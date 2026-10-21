//! A gun carried on the body: the sling takes its weight, and it knocks
//! against what you carry it past.
//!
//! - **The sling**: a gun once slung by its owner stays on its sling. Let it
//!   go (not thrown) and the sling catches it, back where it hangs, rather
//!   than it dropping to the floor. Thrown, or put in a bag or another's
//!   hands, it's off the sling.
//! - **Bumps**: a held gun that meets a wall, a doorframe or a tree as you
//!   move it feels it: the muzzle is knocked back off it (`Handling`), the
//!   hands feel what it hit (`rig::shapes`, by material), and a hard knock
//!   is heard ([`GunBumped`]).

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

use super::Gun;
use super::feel::Handling;
use crate::interact::touch::{Stuff, stuff_of};
use crate::interact::{
    Held, InSocket, InteractEvent, InteractSystems, Socket, SocketKind, SocketOwner, Supported,
};
use crate::rig::shapes::HapticShape;
use extraction_core::inventory::SlotKind;

/// Let go faster than this (m/s) and it was thrown: off the sling.
pub const THROWN: f32 = 2.5;
/// A held gun moving this fast (m/s) into something knocks; slower, it
/// just rests against it.
pub const BUMP: f32 = 0.35;
/// How far a knock turns the muzzle, rad/s per m/s of the gun's speed
/// across its bore.
pub const KNOCK: f32 = 2.5;
/// Stopped: this share of the hands' drive taken away by what it met.
pub const STOPPED: f32 = 0.6;
/// How far ahead of a stopped gun to look for what stopped it, metres.
pub const KNOCK_LOOK: f32 = 0.12;
/// The same gun feels nothing new for this long after a knock, seconds.
pub const KNOCK_SETTLE: f32 = 0.15;
/// A knock this fast (m/s) or faster is heard.
pub const KNOCK_HEARD: f32 = 0.8;

/// The sling a gun was last carried on: its owner's sling slot.
#[derive(Component, Clone, Copy, Debug)]
pub struct OnSling {
    pub player: Entity,
    pub socket: Entity,
}

/// A held gun knocked into something hard enough to hear.
#[derive(Message, Clone, Copy, Debug)]
pub struct GunBumped {
    pub gun: Entity,
    pub at: Vec3,
    pub stuff: Stuff,
    /// m/s.
    pub speed: f32,
}

pub struct CarryPlugin;

impl Plugin for CarryPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<GunBumped>()
            .init_resource::<Driven>()
            .add_systems(
                FixedUpdate,
                (
                    (note_slings, sling_catches)
                        .chain()
                        .after(InteractSystems::Grab)
                        .before(InteractSystems::Follow),
                    note_drive.after(InteractSystems::Follow),
                ),
            )
            .add_systems(FixedPostUpdate, knocks.after(PhysicsSystems::Writeback));
    }
}

/// A gun seated on its owner's sling is on that sling until thrown or put
/// away; picked up by someone else, it isn't.
fn note_slings(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    sockets: Query<(&Socket, Option<&SocketOwner>)>,
    guns: Query<(), With<Gun>>,
    held: Query<&Held>,
    on: Query<&OnSling>,
) {
    for ev in events.read() {
        match *ev {
            InteractEvent::Socketed { item, socket } if guns.contains(item) => {
                let Ok((s, owner)) = sockets.get(socket) else {
                    continue;
                };
                match (&s.kind, owner) {
                    (SocketKind::Body(SlotKind::Sling), Some(o)) => {
                        commands.entity(item).try_insert(OnSling {
                            player: o.0,
                            socket,
                        });
                    }
                    _ => {
                        commands.entity(item).try_remove::<OnSling>();
                    }
                }
            }
            InteractEvent::Stowed { item } => {
                commands.entity(item).try_remove::<OnSling>();
            }
            InteractEvent::Grabbed { item, .. } => {
                if let (Ok(h), Ok(s)) = (held.get(item), on.get(item))
                    && h.player != s.player
                {
                    commands.entity(item).try_remove::<OnSling>();
                }
            }
            _ => {}
        }
    }
}

/// Let go, a gun on its sling is caught by it.
fn sling_catches(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    on: Query<&OnSling, (Without<Held>, Without<InSocket>)>,
    mut sockets: Query<&mut Socket>,
) {
    for ev in events.read() {
        let InteractEvent::Released { item, speed, .. } = *ev else {
            continue;
        };
        let Ok(s) = on.get(item) else {
            continue;
        };
        if speed > THROWN {
            commands.entity(item).try_remove::<OnSling>();
            continue;
        }
        let Ok(mut socket) = sockets.get_mut(s.socket) else {
            continue;
        };
        if socket.occupant.is_some() {
            continue;
        }
        crate::interact::seat_in_socket(&mut commands, item, s.socket, &mut socket);
    }
}

/// What each held gun's hands drove it at this tick (`follow_hands`'s
/// velocity), before the physics had its say.
#[derive(Resource, Default)]
struct Driven(HashMap<Entity, Vec3>);

fn note_drive(
    mut driven: ResMut<Driven>,
    guns: Query<(Entity, &LinearVelocity), (With<Gun>, With<Held>)>,
) {
    driven.0.clear();
    for (e, v) in &guns {
        driven.0.insert(e, v.0);
    }
}

/// A held gun driven into something and stopped short by it: knocked off
/// it, felt, heard. (The physics keeps a driven body off what it meets
/// rather than letting it touch, so the stop is what tells.)
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn knocks(
    driven: Res<Driven>,
    mut guns: Query<
        (
            Entity,
            &Held,
            Option<&Supported>,
            &LinearVelocity,
            &GlobalTransform,
            &Gun,
            &mut Handling,
        ),
        With<Gun>,
    >,
    spatial: SpatialQuery,
    parents: Query<&ChildOf>,
    things: Query<(
        Option<&crate::robots::RobotPart>,
        Option<&crate::items::Item>,
        Option<&crate::world::surfaces::Surfaces>,
        Has<crate::world::structures::Fence>,
        Has<crate::destruction::wood::Timber>,
        &GlobalTransform,
    )>,
    catalog: Res<crate::items::Catalog>,
    time: Res<Time>,
    mut shapes: MessageWriter<HapticShape>,
    mut bumped: MessageWriter<GunBumped>,
    mut last: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    for (gun, held, support, v, gt, g, mut handling) in &mut guns {
        let Some(&want) = driven.0.get(&gun) else {
            continue;
        };
        let speed = want.length();
        if speed < BUMP || last.get(&gun).is_some_and(|t| now - t < KNOCK_SETTLE) {
            continue;
        }
        // How much of the drive the physics took away.
        let dir = want / speed;
        let lost = speed - v.0.dot(dir);
        if lost < STOPPED * speed {
            continue;
        }
        // What stopped it: whatever's just ahead, the way it was going,
        // from the end of the gun that leads.
        let (lo, hi) = g.layout.bounds();
        // (From a little back, in case the end is already in it.)
        let ends = [
            gt.transform_point(g.layout.muzzle),
            gt.transform_point(Vec3::new(0.0, (lo.y + hi.y) * 0.5, lo.z)),
            gt.transform_point(Vec3::new(0.0, (lo.y + hi.y) * 0.5, hi.z)),
        ];
        let Some((at, other)) = ends.iter().find_map(|end| {
            let d = Dir3::new(dir).ok()?;
            let from = *end - dir * KNOCK_LOOK;
            spatial
                .cast_ray(
                    from,
                    d,
                    KNOCK_LOOK * 2.0,
                    true,
                    &SpatialQueryFilter::from_mask(crate::world::Layer::World),
                )
                .map(|hit| (from + dir * hit.distance, hit.entity))
        }) else {
            continue;
        };
        last.insert(gun, now);
        // Knocked back the way it came: what drove it across its bore
        // turns the muzzle back against that.
        let local = gt.rotation().inverse() * want;
        handling.knock(Vec2::new(-local.y, local.x) * KNOCK);
        let stuff = stuff_of(other, at, &parents, &things, &catalog);
        let scale = (0.4 + 0.6 * (speed / 2.0)).clamp(0.4, 1.0);
        shapes.write(HapticShape {
            side: held.side,
            shape: stuff.shape(),
            scale,
        });
        if let Some(s) = support {
            shapes.write(HapticShape {
                side: s.side,
                shape: stuff.shape(),
                scale: scale * 0.6,
            });
        }
        if speed >= KNOCK_HEARD {
            bumped.write(GunBumped {
                gun,
                at,
                stuff,
                speed,
            });
        }
    }
    last.retain(|e, _| guns.contains(*e));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::{GripRole, ReachZone};
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use bevy::ecs::message::Messages;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    fn slot_of(vr: &Vr, gun: Entity) -> Option<SocketKind> {
        vr.app
            .world()
            .get::<InSocket>(gun)
            .and_then(|s| vr.app.world().get::<Socket>(s.socket))
            .map(|s| s.kind.clone())
    }

    /// An AK in the right hand, slung over the right shoulder and taken
    /// back off it; the gun and where the feet are.
    fn slung_and_taken() -> (Vr, Entity, Vec3) {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        let at = feet + Vec3::new(0.2, 1.2, -0.4);
        let gun = vr.spawn("ak74", Transform::from_translation(at));
        vr.app.world_mut().entity_mut(gun).insert(RigidBody::Static);
        let grip = vr.point(gun, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip, Quat::IDENTITY, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(gun)
            .insert(RigidBody::Dynamic);
        vr.step(10);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(gun));
        let shoulder = {
            let mut q = vr
                .app
                .world_mut()
                .query::<(&GlobalTransform, &crate::interact::body::Torso)>();
            let (t, _) = q.iter(vr.app.world()).next().expect("a torso");
            t.transform_point(crate::interact::body::SHOULDER)
        };
        let from = vr.hand_state(Side::Right).world.translation;
        let to = shoulder + Vec3::new(0.0, 0.05, -0.12);
        for i in 1..=20 {
            vr.move_hand(Side::Right, from.lerp(to, i as f32 / 20.0));
            vr.step(1);
        }
        vr.grip(Side::Right, 0.0);
        vr.step(5);
        assert_eq!(
            slot_of(&vr, gun),
            Some(SocketKind::Body(SlotKind::Sling)),
            "slung"
        );
        vr.move_hand(Side::Right, feet + Vec3::new(0.3, 1.0, -0.3));
        vr.step(10);
        assert!(vr.app.world().get::<ReachZone>(gun).is_some());
        let over = shoulder + Vec3::new(0.0, 0.06, 0.05);
        vr.move_hand(Side::Right, over);
        vr.step(4);
        vr.grip(Side::Right, 1.0);
        vr.step(5);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(gun), "taken back");
        (vr, gun, feet)
    }

    /// A gun off its sling, brought out in front and let go: the sling
    /// takes it, back where it hangs; it doesn't fall. Thrown, it goes.
    #[test]
    fn a_gun_let_go_hangs_on_its_sling() {
        let (mut vr, gun, feet) = slung_and_taken();
        let from = vr.hand_state(Side::Right).world.translation;
        let out = feet + Vec3::new(0.25, 1.25, -0.5);
        for i in 1..=30 {
            vr.move_hand(Side::Right, from.lerp(out, i as f32 / 30.0));
            vr.step(1);
        }
        vr.step(10);
        vr.grip(Side::Right, 0.0);
        vr.step(30);
        assert_eq!(
            slot_of(&vr, gun),
            Some(SocketKind::Body(SlotKind::Sling)),
            "caught by the sling"
        );
        let y = vr
            .app
            .world()
            .get::<GlobalTransform>(gun)
            .unwrap()
            .translation()
            .y;
        assert!(y > feet.y + 0.6, "hanging, not on the ground: {y}");

        // Taken again, held out, and thrown away hard: off the sling.
        let (mut vr, gun, feet) = slung_and_taken();
        let from = vr.hand_state(Side::Right).world.translation;
        for i in 1..=30 {
            vr.move_hand(Side::Right, from.lerp(out, i as f32 / 30.0));
            vr.step(1);
        }
        vr.step(10);
        let from = out;
        let far = feet + Vec3::new(0.4, 1.0, -1.6);
        // Let go as the hand flings forward.
        for i in 1..=6 {
            if i == 5 {
                vr.grip(Side::Right, 0.0);
            }
            vr.move_hand(Side::Right, from.lerp(far, i as f32 / 6.0));
            vr.step(1);
        }
        vr.step(30);
        assert!(
            vr.app.world().get::<InSocket>(gun).is_none(),
            "thrown, it goes"
        );
        assert!(
            vr.app.world().get::<OnSling>(gun).is_none(),
            "and is off the sling"
        );
    }

    /// An AK held in both hands, carried forward into a wall: the hands
    /// feel the knock, the muzzle is knocked back off it, a hard knock is
    /// heard, and the barrel stops at the wall's face (its collider runs
    /// to the muzzle).
    #[test]
    fn a_gun_carried_into_a_wall_knocks() {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        let face = feet.z - 1.4;
        vr.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(3.0, 3.0, 0.2),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Transform::from_translation(Vec3::new(feet.x, feet.y + 1.2, face - 0.1)),
        ));
        let gun = vr.spawn(
            "ak74",
            Transform::from_translation(feet + Vec3::new(0.15, 1.2, -0.3)),
        );
        vr.app.world_mut().entity_mut(gun).insert(RigidBody::Static);
        let grip = vr.point(gun, |r| *r == GripRole::Primary);
        let rake = vr.app.world().get::<Gun>(gun).unwrap().layout.grip.rotation;
        vr.hand(Side::Right, grip, rake, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(gun)
            .insert(RigidBody::Dynamic);
        vr.step(10);
        let support = vr.point(gun, |r| *r == GripRole::Support);
        vr.hand(Side::Left, support, Quat::IDENTITY, 1.0, 0.0);
        vr.step(20);
        assert_eq!(vr.hand_state(Side::Left).supporting, Some(gun));
        let mut shapes = vr
            .app
            .world()
            .resource::<Messages<HapticShape>>()
            .get_cursor_current();
        let mut heard = vr
            .app
            .world()
            .resource::<Messages<GunBumped>>()
            .get_cursor_current();
        let (mut felt, mut knocked, mut bumps) = (0, 0.0f32, 0);
        let push = Vec3::new(0.0, 0.0, -0.8);
        for i in 1..=30 {
            let k = (i as f32 / 12.0).min(1.0);
            vr.move_hand(Side::Right, grip + push * k);
            vr.move_hand(Side::Left, support + push * k);
            vr.step(1);
            let w = vr.app.world();
            felt += shapes
                .read(w.resource::<Messages<HapticShape>>())
                .filter(|s| s.side == Side::Right)
                .count();
            bumps += heard.read(w.resource::<Messages<GunBumped>>()).count();
            knocked = knocked.max(w.get::<Handling>(gun).unwrap().rise.length());
        }
        let w = vr.app.world();
        let g = w.get::<Gun>(gun).unwrap();
        let tip = w
            .get::<GlobalTransform>(gun)
            .unwrap()
            .transform_point(g.layout.muzzle);
        assert!(
            face - tip.z > -0.03,
            "the muzzle went {:.3} m into the wall",
            tip.z - face
        );
        assert!(felt >= 1, "the gun hand felt it");
        assert!(bumps >= 1, "heard");
        assert!(knocked > 0.005, "the muzzle knocked off it: {knocked} rad");
    }
}
