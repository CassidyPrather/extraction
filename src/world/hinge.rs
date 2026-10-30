//! A door in the hand. In VR a hand that grips a door's leaf holds it:
//! the leaf turns about its hinge to follow the hand round, as far as it
//! goes and no faster than a door swings, and stays where it's let go.
//! (A press, flat or a bot's, still toggles it: `buildings::swing_doors`,
//! which leaves a door alone while a hand has it and once it's been left
//! part-way, till something toggles it again.)

use bevy::prelude::*;

use super::buildings::{Door, DoorLeaf};
use crate::interact::{Hand, InteractEvent};

/// `Door::angle` turns the leaf by this, radians per unit
/// (`buildings::swing_doors`).
pub const PER_UNIT: f32 = 1.75;
/// The fastest a hand turns a leaf, radians a second: a door pulled hard
/// swings quick, but never jumps.
pub const MOST: f32 = 4.0;
/// A hand nearer the hinge than this (m) can't lever the leaf round.
const NEAR_HINGE: f32 = 0.12;
/// A squeeze shorter than this (seconds) that hardly moves the leaf is a
/// tap: it toggles the door, which swings over a second
/// (`swing_doors`), as a press does.
pub const TAP: f32 = 0.5;
const TAP_TURN: f32 = 0.05;

/// A door a hand has hold of: the hand, and where round the hinge it and
/// the leaf were when it took hold.
#[derive(Component, Clone, Copy, Debug)]
pub struct Swinging {
    pub hand: Entity,
    hand_from: f32,
    leaf_from: f32,
    since: f32,
}

/// When a hand let go of this door (seconds, `Time::elapsed`): a palm
/// still on its leaf isn't a push for a moment (`physics_hands::push_doors`
/// would shut a door the hand's just swung, or tapped open).
#[derive(Component, Clone, Copy, Debug)]
pub struct LetGo(pub f32);

/// How long after a hand lets go its palm on the leaf isn't a push.
pub const LET_GO: f32 = 0.6;

/// A door left where a hand let it go: `swing_doors` doesn't drive it to
/// either end until it's toggled (`open` no longer what it was).
#[derive(Component, Clone, Copy, Debug)]
pub struct Resting {
    pub open: bool,
}

/// The way round the hinge (radians, the leaf's own sense) a point lies,
/// in the door's parent's space.
fn round_hinge(leaf: &DoorLeaf, p: Vec3) -> Option<f32> {
    let v = p - leaf.0.hinge;
    let flat = Vec2::new(v.x, -v.z);
    (flat.length() > NEAR_HINGE).then(|| flat.y.atan2(flat.x))
}

/// The shortest way from `a` to `b`, radians.
fn turn(a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d - std::f32::consts::TAU
    } else {
        d
    }
}

/// A VR grip on a door takes hold of it rather than toggling it.
#[allow(clippy::type_complexity)]
pub fn take_hold(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    driver: Res<crate::rig::RigDriver>,
    players: Query<&Children, With<crate::player::Player>>,
    hands: Query<(Entity, &Hand)>,
    doors: Query<
        (&Door, &DoorLeaf, &ChildOf),
        (
            Without<Swinging>,
            Without<crate::robots::doors::Wedged>,
            Without<super::underground::PowerLocked>,
        ),
    >,
    parents: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, player } = ev else {
            continue;
        };
        if *driver != crate::rig::RigDriver::Xr {
            continue;
        }
        let Ok((door, leaf, parent)) = doors.get(*item) else {
            continue;
        };
        let Ok(to_parent) = parents.get(parent.parent()).map(|g| g.affine().inverse()) else {
            continue;
        };
        // The gripping hand nearest the leaf's middle.
        let dir = Quat::from_rotation_y(leaf.0.yaw + door.angle * PER_UNIT) * Vec3::X;
        let middle = leaf.0.hinge + dir * leaf.0.width * 0.5 + Vec3::Y * leaf.0.height * 0.5;
        let Some((hand, at)) = players
            .get(*player)
            .into_iter()
            .flat_map(|c| c.iter())
            .filter_map(|c| hands.get(c).ok())
            .filter(|(_, h)| h.grip_down)
            .map(|(e, h)| (e, to_parent.transform_point3(h.world.translation)))
            .min_by(|a, b| a.1.distance(middle).total_cmp(&b.1.distance(middle)))
        else {
            continue;
        };
        let Some(hand_from) = round_hinge(leaf, at) else {
            continue;
        };
        commands.entity(*item).insert(Swinging {
            hand,
            hand_from,
            leaf_from: door.angle,
            since: time.elapsed_secs(),
        });
    }
}

/// The leaf follows the hand that has it, round its hinge; let go, it
/// stays where it was left.
pub fn swing_by_hand(
    mut commands: Commands,
    mut doors: Query<(
        Entity,
        &mut Door,
        &mut Transform,
        &DoorLeaf,
        &Swinging,
        &ChildOf,
    )>,
    hands: Query<&Hand>,
    parents: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    let step = MOST / PER_UNIT * time.delta_secs();
    for (e, mut door, mut t, leaf, held, parent) in &mut doors {
        let hand = hands.get(held.hand).ok().filter(|h| h.grip_down);
        let Some(hand) = hand else {
            let mut e = commands.entity(e);
            e.remove::<Swinging>().insert(LetGo(time.elapsed_secs()));
            let tap = time.elapsed_secs() - held.since < TAP
                && (door.angle - held.leaf_from).abs() * PER_UNIT < TAP_TURN;
            if tap {
                // A squeeze and let go: it swings to the other end.
                door.open = held.leaf_from < 0.5;
            } else {
                // Let go: where it is is where it rests.
                door.open = door.angle > 0.5;
                e.insert(Resting { open: door.open });
            }
            continue;
        };
        let Ok(to_parent) = parents.get(parent.parent()).map(|g| g.affine().inverse()) else {
            continue;
        };
        let at = to_parent.transform_point3(hand.world.translation);
        if let Some(now) = round_hinge(leaf, at) {
            let want = (held.leaf_from + turn(held.hand_from, now) / PER_UNIT).clamp(0.0, 1.0);
            door.angle += (want - door.angle).clamp(-step, step);
        }
        t.rotation = Quat::from_rotation_y(leaf.0.yaw + door.angle * PER_UNIT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shortest_way_round() {
        assert!((turn(3.0, -3.0) - (std::f32::consts::TAU - 6.0)).abs() < 1e-5);
        assert!((turn(0.2, 0.5) - 0.3).abs() < 1e-5);
        assert!((turn(-3.0, 3.0) + (std::f32::consts::TAU - 6.0)).abs() < 1e-5);
    }
}
