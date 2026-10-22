//! Hand to hand, besides the blades and the rifle butt (`weapons::melee`):
//!
//! - **The crowbar strikes.** Swung, its claw end is swept like a butt: a
//!   blow that lands fast enough on a robot's part (or anything) does
//!   `melee::blunt` of the bar's mass and the speed into it, through the
//!   same `Hit` the guns' blows use; a lazy tap does nothing.
//! - **An open hand shoves.** An empty, open hand that meets a robot fast
//!   enough pushes it, by what an arm puts behind a palm ([`PALM`] kg at the
//!   hand's speed) against what it weighs: a drone or a mender (bodies that
//!   fly) is knocked back; a walker or a crawler rocks, and enough of it
//!   (a small one sooner) puts it on the ground (`robots::anim::Gait`).
//!   [`Shoved`] says so, for the machines' minds and for Audio.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::physics_hands::HandBody;
use super::{Hand, Held};
use crate::items::{Catalog, Item};
use crate::rig::RigHand;
use crate::rig::haptics::Haptic;
use crate::robots::RobotPart;
use crate::weapons::ballistics::Hit;
use crate::world::Layer;

/// What an arm puts behind an open palm, kg (the hand and the forearm,
/// and some of the shoulder).
pub const PALM: f32 = 5.0;
/// Slower than this (m/s, into the robot) is a touch, not a shove.
pub const SHOVE_MIN: f32 = 1.0;
/// How much a shove dazes a walking machine, per N·s: a crawler goes down
/// to one good shove, a walker to three or four.
pub const SHOVE_DAZE: f32 = 2.5;
/// The same robot is struck by the same bar no more often than this.
const RESTRIKE: f32 = 0.35;

/// A robot shoved by an open hand.
#[derive(Message, Clone, Copy, Debug)]
pub struct Shoved {
    pub robot: Entity,
    pub at: Vec3,
    /// The push, N·s, the way it went.
    pub impulse: Vec3,
    pub by: Entity,
}

/// A crowbar's last claw position and what it struck lately.
#[derive(Component, Default)]
pub struct BarStroke {
    prev: Option<Vec3>,
    lately: Vec<(Entity, f32)>,
}

pub struct FightPlugin;

impl Plugin for FightPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Shoved>()
            .add_observer(bar_strokes)
            .add_systems(
                FixedUpdate,
                (
                    strike_with_bars.after(super::InteractSystems::Follow),
                    shove.after(PhysicsSystems::Writeback),
                ),
            );
    }
}

fn bar_strokes(add: On<Add, super::tools::crowbar::Crowbar>, mut commands: Commands) {
    commands.entity(add.entity).try_insert(BarStroke::default());
}

/// How hard a shove at `speed` (m/s, into it) pushes, N·s.
pub fn shove_impulse(speed: f32) -> f32 {
    if speed < SHOVE_MIN { 0.0 } else { PALM * speed }
}

/// The crowbar's claw swept from last tick to this, as a rifle's butt is
/// (`weapons::melee::strike_butts`): what it meets, fast, is struck.
#[allow(clippy::type_complexity)]
fn strike_with_bars(
    mut bars: Query<(Entity, &mut BarStroke, &Item, Option<&Held>)>,
    parts: Query<&RobotPart>,
    weak: Query<&crate::weapons::melee::WeakPoint>,
    parents: Query<&ChildOf>,
    hands: Query<&Hand>,
    catalog: Res<Catalog>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    mut haptics: MessageWriter<Haptic>,
) {
    let dt = time.delta_secs().max(1e-4);
    let now = time.elapsed_secs();
    for (e, mut stroke, item, held) in &mut bars {
        stroke.lately.retain(|(_, t)| now - *t < RESTRIKE);
        let Some(held) = held else {
            stroke.prev = None;
            continue;
        };
        let Ok(hand) = hands.get(held.hand) else {
            continue;
        };
        let at = hand
            .world
            .mul_transform(held.offset)
            .transform_point(super::tools::crowbar::CLAW);
        let Some(prev) = stroke.prev.replace(at) else {
            continue;
        };
        let step = at - prev;
        let Ok(dir) = Dir3::new(step) else {
            continue;
        };
        let own = |h: Entity| {
            let mut cur = h;
            for _ in 0..6 {
                if cur == e || cur == held.player {
                    return true;
                }
                match parents.get(cur) {
                    Ok(c) => cur = c.parent(),
                    Err(_) => return false,
                }
            }
            false
        };
        let filter = SpatialQueryFilter::from_mask([Layer::Robot, Layer::World, Layer::Item]);
        let Some(contact) =
            spatial.cast_ray_predicate(prev, dir, step.length(), true, &filter, &|h| !own(h))
        else {
            continue;
        };
        if contact.distance <= 1e-4 || stroke.lately.iter().any(|(t, _)| *t == contact.entity) {
            continue;
        }
        let v = step / dt;
        let into = v.dot(-contact.normal).max(0.0);
        let mass = catalog.get(item.kind()).mass;
        let Some(harm) = extraction_core::melee::blunt(mass, into) else {
            continue;
        };
        let soft = weak
            .get(contact.entity)
            .map(|w| w.0)
            .or_else(|_| {
                parts
                    .get(contact.entity)
                    .map(|p| crate::weapons::melee::blunt_softness(p.role))
            })
            .unwrap_or(1.0);
        stroke.lately.push((contact.entity, now));
        hits.write(Hit {
            entity: contact.entity,
            point: prev + *dir * contact.distance,
            normal: contact.normal,
            velocity: v,
            damage: harm * soft,
            shooter: Some(held.player),
            mass_g: 0.0,
            load: Default::default(),
            blow: Some(extraction_core::melee::Blow::Blunt),
        });
        haptics.write(Haptic {
            side: held.side,
            amplitude: (0.6 + harm / 120.0).min(1.0),
            seconds: 0.07,
        });
    }
}

/// An open hand meets a robot's part: a shove, if it was going.
#[allow(clippy::type_complexity)]
fn shove(
    mut started: MessageReader<CollisionStart>,
    bodies: Query<&HandBody>,
    hands: Query<(&Hand, &RigHand, &ChildOf)>,
    parts: Query<(&RobotPart, &GlobalTransform)>,
    mut flyers: Query<(&RigidBody, &mut LinearVelocity, Option<&Mass>)>,
    mut gaits: Query<(&mut crate::robots::anim::Gait, &GlobalTransform)>,
    mut shoved: MessageWriter<Shoved>,
    mut haptics: MessageWriter<Haptic>,
) {
    for c in started.read() {
        let (hand_body, other) = match (bodies.get(c.collider1), bodies.get(c.collider2)) {
            (Ok(b), _) => (b, c.collider2),
            (_, Ok(b)) => (b, c.collider1),
            _ => continue,
        };
        let Ok((part, part_at)) = parts.get(other) else {
            continue;
        };
        let Ok((hand, rh, player)) = hands.get(hand_body.hand) else {
            continue;
        };
        if hand.grip_down || hand.holding.is_some() {
            continue;
        }
        let at = part_at.translation();
        let toward = (at - hand.world.translation).normalize_or(hand.velocity.normalize_or_zero());
        let speed = hand.velocity.dot(toward).max(0.0);
        let push = shove_impulse(speed);
        if push <= 0.0 {
            continue;
        }
        let impulse = toward * push;
        if let Ok((kind, mut v, mass)) = flyers.get_mut(part.robot)
            && *kind == RigidBody::Dynamic
        {
            let m = mass.map_or(6.0, |m| m.0).max(0.5);
            v.0 += impulse / m;
        }
        if let Ok((mut gait, body)) = gaits.get_mut(part.robot) {
            gait.knock(at - body.translation(), impulse / 10.0);
            gait.daze(push * SHOVE_DAZE, toward);
        }
        shoved.write(Shoved {
            robot: part.robot,
            at,
            impulse,
            by: player.parent(),
        });
        haptics.write(Haptic {
            side: rh.side,
            amplitude: (0.3 + push / 60.0).min(0.6),
            seconds: 0.05,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A palm laid on doesn't push; a shove does, harder the faster.
    #[test]
    fn a_touch_doesnt_shove_a_push_does() {
        assert_eq!(shove_impulse(0.5), 0.0);
        assert!((shove_impulse(2.0) - 10.0).abs() < 1e-4);
        assert!(shove_impulse(4.0) > shove_impulse(2.0));
    }
}
