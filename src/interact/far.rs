//! Far grab (VR): point an empty hand at something out of reach, squeeze
//! the grip, and flick the wrist back: it hops up and arcs into your hand.
//! Keep the grip closed and it's caught; open it and it lands at your feet.
//!
//! The first headset playtest spent a lot of time sitting on the floor to
//! pick things up. Most loot now starts at hand height, but things get
//! dropped, fall off shelves, grow out of the ground (artifacts) or sit at
//! the back of a shelf; this is for those.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Hand, HandAction, Held, InteractEvent};
use crate::items::Item;
use crate::rig::{LocalPlayer, RigDriver, RigHand};
use crate::world::Layer;

/// How far a hand can call things from (m).
pub const FAR: f32 = 5.0;
/// Closer than this is the grip's business, not the far grab's.
const NEAR: f32 = 0.45;
/// Hand speed back toward you (m/s), or wrist flick up (rad/s), that calls
/// the thing pointed at.
const PULL_SPEED: f32 = 1.2;
const FLICK_SPIN: f32 = 6.0;
/// Within this of the hand, a closed grip catches it.
const CATCH: f32 = 0.3;

/// What can be called: loose things lying about.
type Loose<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GlobalTransform,
        Option<&'static Visibility>,
    ),
    (
        With<Item>,
        Without<Held>,
        Without<ColliderDisabled>,
        Without<super::InSocket>,
    ),
>;

/// Runs after the grip has had its look (what's in reach always wins) and
/// before grabs are resolved, so a catch lands the same tick.
pub(super) fn far_grab(
    mut commands: Commands,
    players: Query<&Children, (With<crate::player::Player>, With<LocalPlayer>)>,
    mut hands: Query<(&RigHand, &mut Hand, &ChildOf)>,
    loose: Loose,
    mut bodies: Query<(&RigidBody, &mut LinearVelocity, &mut AngularVelocity)>,
    driver: Res<RigDriver>,
    gravity: Res<Gravity>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut actions: MessageWriter<HandAction>,
    mut events: MessageWriter<InteractEvent>,
) {
    if *driver != RigDriver::Xr {
        return;
    }
    let now = time.elapsed_secs();
    for children in &players {
        for child in children.iter() {
            let Ok((rh, mut hand, parent)) = hands.get_mut(child) else {
                continue;
            };
            let player = parent.parent();
            let busy = hand.holding.is_some() || hand.supporting.is_some() || hand.handle.is_some();
            if busy || !hand.grip_down {
                hand.pulling = None;
            }
            // On its way: a closed hand catches it as it arrives.
            if let Some((item, since)) = hand.pulling {
                let Ok((_, gt, _)) = loose.get(item) else {
                    hand.pulling = None;
                    continue;
                };
                if gt.translation().distance(hand.world.translation) < CATCH {
                    actions.write(HandAction::Grab {
                        player,
                        side: rh.side,
                        item,
                        point: None,
                    });
                    hand.pulling = None;
                } else if now - since > 1.5 {
                    hand.pulling = None;
                }
                continue;
            }
            // Something in reach is the grip's.
            if busy || hand.hover.is_some() {
                hand.far = None;
                continue;
            }
            // Squeezed on a target: watch for the flick.
            if hand.grip_down {
                let Some(item) = hand.far else {
                    continue;
                };
                let Ok((_, gt, _)) = loose.get(item) else {
                    hand.far = None;
                    continue;
                };
                let from = gt.translation();
                let back = (hand.world.translation - from).normalize_or_zero();
                let wrist = hand.world.rotation * Vec3::X;
                let flicked = hand.velocity.dot(back) > PULL_SPEED
                    || hand.angular_velocity.dot(wrist) > FLICK_SPIN;
                if !flicked {
                    continue;
                }
                let Ok((body, mut v, mut w)) = bodies.get_mut(item) else {
                    continue;
                };
                if *body != RigidBody::Dynamic {
                    continue;
                }
                v.0 = arc(from, hand.world.translation + Vec3::Y * 0.05, gravity.0);
                w.0 = Vec3::ZERO;
                commands.queue(WakeBody(item));
                hand.far = None;
                hand.pulling = Some((item, now));
                events.write(InteractEvent::Pulled {
                    item,
                    side: rh.side,
                });
                continue;
            }
            // Open hand: what it's pointing at.
            let target = pointed_at(&hand.world, &loose, &spatial);
            if target != hand.far {
                hand.far = target;
                if let Some(item) = target {
                    events.write(InteractEvent::Hover {
                        item,
                        side: rh.side,
                    });
                }
            }
        }
    }
}

/// Launch velocity that carries a thing from `from` to `to` under
/// `gravity`, quicker for nearer things.
pub fn arc(from: Vec3, to: Vec3, gravity: Vec3) -> Vec3 {
    let d = to - from;
    let t = (0.35 + 0.07 * d.length()).clamp(0.4, 0.75);
    d / t - gravity * 0.5 * t
}

/// The loose thing nearest the line a hand points along (its `-Z`), in
/// sight, between arm's length and [`FAR`].
fn pointed_at(hand: &Transform, loose: &Loose, spatial: &SpatialQuery) -> Option<Entity> {
    let origin = hand.translation;
    let ahead = hand.rotation * Vec3::NEG_Z;
    let walls = SpatialQueryFilter::from_mask([Layer::World]);
    let mut best: Option<(f32, Entity)> = None;
    for (e, gt, vis) in loose.iter() {
        if vis == Some(&Visibility::Hidden) {
            continue;
        }
        let to = gt.translation() - origin;
        let along = to.dot(ahead);
        if !(NEAR..=FAR).contains(&along) {
            continue;
        }
        // A cone with a rounded tip: small things at a distance are still
        // easy to point at.
        let off = (to - ahead * along).length();
        let score = off / (0.08 + along * 0.1);
        if score > 1.0 || best.is_some_and(|b| score >= b.0) {
            continue;
        }
        let dist = to.length();
        let Ok(dir) = Dir3::new(to) else {
            continue;
        };
        let blocked = spatial
            .cast_ray(origin, dir, dist, true, &walls)
            .is_some_and(|h| h.distance < dist - 0.25);
        if !blocked {
            best = Some((score, e));
        }
    }
    best.map(|(_, e)| e)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The launch lands where it's aimed, under gravity.
    #[test]
    fn the_arc_arrives_at_the_hand() {
        let g = Vec3::new(0.0, -9.81, 0.0);
        for (from, to) in [
            (Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.2, 1.2, 0.0)),
            (Vec3::new(4.0, 0.9, 1.0), Vec3::new(0.0, 1.0, 0.0)),
            (Vec3::new(0.0, 2.0, -1.0), Vec3::new(0.0, 1.0, 0.0)),
        ] {
            let v = arc(from, to, g);
            let t = (0.35 + 0.07 * (to - from).length()).clamp(0.4, 0.75);
            let at = from + v * t + g * 0.5 * t * t;
            assert!(at.distance(to) < 1e-3, "{from} -> {to}: {at}");
        }
    }
}
