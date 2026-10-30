//! Doors, casements and drawers in the hand. In VR a hand that grips a
//! door's leaf (or a window's, a cabinet's, or a drawer's front) holds it:
//! a leaf turns about its hinge to follow the hand round, a drawer slides
//! out and in with it, as far as it goes and never faster than it should,
//! and stays where it's let go. A quick squeeze toggles it; a press, flat
//! or a bot's, toggles it too.
//!
//! A building's door is `buildings::Door` (driven here while held, by
//! `swing_doors` otherwise). Anything else that moves is a [`Moving`]
//! part, spawned by [`spawn_moving`].

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

/// How a [`Moving`] part moves: round its rest frame's +Y (a leaf, its
/// +X from the hinge to its free edge), up to `most` radians; or out along
/// its rest frame's -Z (a drawer, its front), up to `travel` metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Hinge { most: f32 },
    Slide { travel: f32 },
}

impl Motion {
    fn end(self) -> f32 {
        match self {
            Motion::Hinge { most } => most,
            Motion::Slide { travel } => travel,
        }
    }
    /// The fastest a hand moves it, and the pace it swings or runs on its
    /// own (toggled), a second.
    fn rates(self) -> (f32, f32) {
        match self {
            Motion::Hinge { .. } => (MOST, 2.8),
            Motion::Slide { .. } => (SLIDE_MOST, 0.6),
        }
    }
}

/// The fastest a hand slides a drawer, m/s.
pub const SLIDE_MOST: f32 = 1.2;

/// Something other than a door that a hand moves: a casement, a cabinet
/// door, a drawer. `rest` is its pose shut (world); `at` how far it's open
/// (radians or metres); `open` the end it's headed for when nothing holds
/// it; `left` it was let go part-way and stays.
#[derive(Component, Clone, Copy, Debug)]
pub struct Moving {
    pub motion: Motion,
    pub rest: Transform,
    pub at: f32,
    pub open: bool,
    pub left: bool,
}

impl Moving {
    /// Its pose at `at`.
    pub fn pose(&self) -> Transform {
        match self.motion {
            Motion::Hinge { .. } => {
                self.rest * Transform::from_rotation(Quat::from_rotation_y(self.at))
            }
            Motion::Slide { .. } => self.rest * Transform::from_translation(Vec3::NEG_Z * self.at),
        }
    }
    /// Where a hand at `p` (world) is, in its own measure: round the hinge
    /// (radians), or along the slide (metres).
    fn measure(&self, p: Vec3) -> Option<f32> {
        let local = self.rest.compute_affine().inverse().transform_point3(p);
        match self.motion {
            Motion::Hinge { .. } => {
                let flat = Vec2::new(local.x, -local.z);
                (flat.length() > NEAR_HINGE).then(|| flat.y.atan2(flat.x))
            }
            Motion::Slide { .. } => Some(-local.z),
        }
    }
}

/// A part a hand has hold of.
#[derive(Component, Clone, Copy, Debug)]
pub struct Gripped {
    pub hand: Entity,
    hand_from: f32,
    part_from: f32,
    since: f32,
}

/// Put up a moving part: shut at `rest`, a box `size` (in its own space at
/// `centre`: a leaf's half its width out along +X) to take hold of and to
/// bump into.
pub fn spawn_moving(
    commands: &mut Commands,
    name: &str,
    rest: Transform,
    motion: Motion,
    size: Vec3,
    centre: Vec3,
) -> Entity {
    use avian3d::prelude::*;
    commands
        .spawn((
            Name::new(name.to_string()),
            Moving {
                motion,
                rest,
                at: 0.0,
                open: false,
                left: false,
            },
            crate::raid::truck::Pressable::Door,
            crate::interact::Grabbable {
                points: vec![crate::interact::GrabPoint::free()],
            },
            rest,
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .with_child((
            Collider::cuboid(size.x, size.y, size.z),
            CollisionLayers::new(super::Layer::World, LayerMask::ALL),
            Transform::from_translation(centre),
        ))
        .id()
}

/// A grip on a moving part: in VR it takes hold; else it toggles.
pub fn take_parts(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    driver: Res<crate::rig::RigDriver>,
    players: Query<&Children, With<crate::player::Player>>,
    hands: Query<(Entity, &Hand)>,
    mut parts: Query<(&mut Moving, &GlobalTransform), Without<Gripped>>,
    time: Res<Time>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, player } = ev else {
            continue;
        };
        let Ok((mut part, gt)) = parts.get_mut(*item) else {
            continue;
        };
        if *driver != crate::rig::RigDriver::Xr {
            part.open = !part.open;
            part.left = false;
            continue;
        }
        let near = gt.translation();
        let Some((hand, at)) = players
            .get(*player)
            .into_iter()
            .flat_map(|c| c.iter())
            .filter_map(|c| hands.get(c).ok())
            .filter(|(_, h)| h.grip_down)
            .map(|(e, h)| (e, h.world.translation))
            .min_by(|a, b| a.1.distance(near).total_cmp(&b.1.distance(near)))
        else {
            continue;
        };
        let Some(hand_from) = part.measure(at) else {
            continue;
        };
        commands.entity(*item).insert(Gripped {
            hand,
            hand_from,
            part_from: part.at,
            since: time.elapsed_secs(),
        });
    }
}

/// Moving parts: one in a hand follows it; let go part-way it stays; a
/// toggled one swings or runs to its end.
pub fn move_parts(
    mut commands: Commands,
    mut parts: Query<(Entity, &mut Moving, &mut Transform, Option<&Gripped>)>,
    hands: Query<&Hand>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut part, mut t, held) in &mut parts {
        let (most, pace) = part.motion.rates();
        let end = part.motion.end();
        match held {
            Some(held) => match hands.get(held.hand).ok().filter(|h| h.grip_down) {
                Some(hand) => {
                    if let Some(now) = part.measure(hand.world.translation) {
                        let moved = match part.motion {
                            Motion::Hinge { .. } => turn(held.hand_from, now),
                            Motion::Slide { .. } => now - held.hand_from,
                        };
                        let want = (held.part_from + moved).clamp(0.0, end);
                        let step = most * dt;
                        part.at += (want - part.at).clamp(-step, step);
                    }
                }
                None => {
                    commands.entity(e).remove::<Gripped>();
                    let tap = time.elapsed_secs() - held.since < TAP
                        && (part.at - held.part_from).abs() < end * 0.03;
                    if tap {
                        part.open = held.part_from < end * 0.5;
                        part.left = false;
                    } else {
                        part.open = part.at > end * 0.5;
                        part.left = true;
                    }
                }
            },
            None if !part.left => {
                let target = if part.open { end } else { 0.0 };
                let step = pace * dt;
                part.at += (target - part.at).clamp(-step, step);
            }
            None => {}
        }
        *t = part.pose();
    }
}

/// A counter's moving nodes (`art/props/furniture.py`, split by Model
/// correctness): `model#node`, the node's origin in the model's space
/// (on a door's hinge, a drawer's middle shut), its box's centre from
/// that origin and its size (read from the model), whether it opens the
/// other way round (a right-hand door: its free edge toward -X), and how
/// it moves.
struct CounterNode(&'static str, [f32; 3], [f32; 3], [f32; 3], bool, Motion);

const COUNTER_NODES: &[CounterNode] = &[
    CounterNode(
        "counter_drawers#drawer_0",
        [0.0, 0.68, -0.04],
        [0.0, 0.0, -0.013],
        [0.566, 0.244, 0.546],
        false,
        Motion::Slide {
            travel: DRAWER_TRAVEL,
        },
    ),
    CounterNode(
        "counter_drawers#drawer_1",
        [0.0, 0.43, -0.04],
        [0.0, 0.0, -0.013],
        [0.566, 0.244, 0.546],
        false,
        Motion::Slide {
            travel: DRAWER_TRAVEL,
        },
    ),
    CounterNode(
        "counter_drawers#drawer_2",
        [0.0, 0.205, -0.04],
        [0.0, 0.0, -0.013],
        [0.566, 0.194, 0.546],
        false,
        Motion::Slide {
            travel: DRAWER_TRAVEL,
        },
    ),
    CounterNode(
        "counter_cabinet#door_l",
        [-0.284, 0.455, -0.298],
        [0.14, 0.0, -0.004],
        [0.281, 0.7, 0.047],
        false,
        Motion::Hinge { most: CABINET_DOOR },
    ),
    CounterNode(
        "counter_cabinet#door_r",
        [0.284, 0.455, -0.298],
        [-0.141, 0.0, -0.004],
        [0.281, 0.7, 0.046],
        true,
        Motion::Hinge { most: CABINET_DOOR },
    ),
];

/// A counter node's origin in its model's space (`model#node`), if it's
/// one we move.
pub fn counter_node_origin(drawn: &str) -> Option<Vec3> {
    COUNTER_NODES
        .iter()
        .find(|n| n.0 == drawn)
        .map(|n| Vec3::from(n.1))
}

/// How far a drawer pulls out (m: `DRAWER_TRAVEL`, the model's), and a
/// cabinet's door swings (radians).
pub const DRAWER_TRAVEL: f32 = 0.4;
pub const CABINET_DOOR: f32 = 1.9;

/// A building's counters' drawers and cabinet doors, put up as moving
/// parts when it's placed (the building draws only their carcasses:
/// `Builder::counter`).
pub fn fit_counters(
    mut commands: Commands,
    buildings: Query<(&super::buildings::Building, &Transform), Added<super::buildings::Building>>,
) {
    for (b, bxf) in &buildings {
        let Some(geo) = super::buildings::prefab(&b.prefab) else {
            continue;
        };
        for d in &geo.props {
            for (name, xf) in &d.models {
                let Some(model) = name.strip_suffix("#carcass") else {
                    continue;
                };
                let prop = bxf.compute_affine() * xf.compute_affine();
                for n in COUNTER_NODES
                    .iter()
                    .filter(|n| n.0.starts_with(model) && n.0[model.len()..].starts_with('#'))
                {
                    spawn_counter_node(&mut commands, prop, xf.scale, n);
                }
            }
        }
    }
}

fn spawn_counter_node(
    commands: &mut Commands,
    prop: bevy::math::Affine3A,
    scale: Vec3,
    CounterNode(drawn, origin, centre, size, flipped, motion): &CounterNode,
) {
    // Shut, in the world: on its hinge or at its middle, unscaled, a
    // right-hand door turned over so it opens with the same sense.
    let flip = if *flipped {
        Quat::from_rotation_z(std::f32::consts::PI)
    } else {
        Quat::IDENTITY
    };
    let node = bevy::math::Affine3A::from_rotation_translation(flip, Vec3::from(*origin));
    let (_, rot, at) = (prop * node).to_scale_rotation_translation();
    let rest = Transform::from_translation(at).with_rotation(rot);
    let box_centre = flip.inverse() * Vec3::from(*centre);
    let box_size = Vec3::from(*size) * Vec3::new(scale.x.abs(), 1.0, 1.0);
    let e = spawn_moving(commands, drawn, rest, *motion, box_size, box_centre);
    // Its node drawn where it hangs in the model, from the part's own space.
    let model_at = Transform::from_matrix((rest.compute_affine().inverse() * prop).into());
    commands.entity(e).insert(crate::destruction::wood::Drawn {
        model: Some(drawn),
        mat: crate::render::materials::Mat::Timber,
        at: model_at,
        size: box_size,
        range: crate::render::lod::near_range().end_margin,
        shadow: false,
    });
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
