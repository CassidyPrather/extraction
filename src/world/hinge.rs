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
#[derive(Component, Clone, Copy, Debug, PartialEq)]
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
    let e = moving(
        commands,
        name,
        rest,
        motion,
        crate::interact::GrabPoint::free(),
    );
    commands.entity(e).insert(RigidBody::Kinematic).with_child((
        Collider::cuboid(size.x, size.y, size.z),
        CollisionLayers::new(super::Layer::World, LayerMask::ALL),
        Transform::from_translation(centre),
    ));
    e
}

/// A moving part with nothing to bump into, taken hold of within `reach`
/// of `grip` (its own space): a window's leaf, whose glass is its pane's
/// (`destruction::glass`), which a round or a thrown thing must reach.
pub fn spawn_moving_open(
    commands: &mut Commands,
    name: &str,
    rest: Transform,
    motion: Motion,
    grip: Vec3,
    reach: f32,
) -> Entity {
    let point = crate::interact::GrabPoint::new(
        Transform::from_translation(grip),
        crate::interact::GripRole::Primary,
        reach,
    );
    moving(commands, name, rest, motion, point)
}

/// `EXTRACTION_OPEN_PARTS=1`: every cabinet's door, drawer and locker's
/// door starts swinging open, to look into them (a capture's aid).
fn open_for_capture() -> bool {
    static OPEN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OPEN.get_or_init(|| std::env::var_os("EXTRACTION_OPEN_PARTS").is_some())
}

fn moving(
    commands: &mut Commands,
    name: &str,
    rest: Transform,
    motion: Motion,
    point: crate::interact::GrabPoint,
) -> Entity {
    commands
        .spawn((
            Name::new(name.to_string()),
            Moving {
                motion,
                rest,
                at: 0.0,
                open: open_for_capture(),
                left: false,
            },
            crate::raid::truck::Pressable::Door,
            crate::interact::Grabbable {
                points: vec![point],
            },
            rest,
            Visibility::default(),
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
    for (e, mut stored, mut t, held) in &mut parts {
        // Changed only when it moves: a window's panes follow theirs
        // (`glass::carry_panes`).
        let mut part = *stored;
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
        if part != *stored {
            *stored = part;
        }
        t.set_if_neq(part.pose());
    }
}

/// A counter's moving nodes (`art/props/furniture.py`, split by Model
/// correctness): `model#node`, the node's origin in the model's space
/// (on a door's hinge, a drawer's middle shut), its box's centre from
/// that origin and its size (read from the model, in the model's axes),
/// how its own space is turned from the model's ([`Frame`]), and how it
/// moves.
struct CounterNode(&'static str, [f32; 3], [f32; 3], [f32; 3], Frame, Motion);

/// How a moving part's own space is turned from its model's: a hinge
/// turns about its own Y, a slide runs along its own -Z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Frame {
    /// As the model's: a left-hand door, a drawer.
    Upright,
    /// Turned over (about Z): a right-hand door, its free edge toward -X,
    /// opening with the same sense.
    Over,
    /// A lid hinged along its back (the model's X), lifting its front.
    Lid,
}

impl Frame {
    pub fn turn(self) -> Quat {
        match self {
            Frame::Upright => Quat::IDENTITY,
            Frame::Over => Quat::from_rotation_z(std::f32::consts::PI),
            Frame::Lid => Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
        }
    }
}

impl CounterNode {
    /// Its rest from its prop (`prop`: the prop's placing), unscaled.
    fn rest(&self, prop: bevy::math::Affine3A) -> Transform {
        let node =
            bevy::math::Affine3A::from_rotation_translation(self.4.turn(), Vec3::from(self.1));
        let (_, rot, at) = (prop * node).to_scale_rotation_translation();
        Transform::from_translation(at).with_rotation(rot)
    }
    /// Its box's centre and size in its own space, its prop scaled by
    /// `scale` (across only: a counter's run).
    fn part_box(&self, scale: Vec3) -> (Vec3, Vec3) {
        let q = self.4.turn().inverse();
        let size = Vec3::from(self.3) * Vec3::new(scale.x.abs(), 1.0, 1.0);
        (q * Vec3::from(self.2), (q * size).abs())
    }
}

const COUNTER_NODES: &[CounterNode] = &[
    CounterNode(
        "counter_drawers#drawer_0",
        [0.0, 0.68, -0.04],
        [0.0, 0.0, -0.013],
        [0.566, 0.244, 0.546],
        Frame::Upright,
        Motion::Slide {
            travel: DRAWER_TRAVEL,
        },
    ),
    CounterNode(
        "counter_drawers#drawer_1",
        [0.0, 0.43, -0.04],
        [0.0, 0.0, -0.013],
        [0.566, 0.244, 0.546],
        Frame::Upright,
        Motion::Slide {
            travel: DRAWER_TRAVEL,
        },
    ),
    CounterNode(
        "counter_drawers#drawer_2",
        [0.0, 0.205, -0.04],
        [0.0, 0.0, -0.013],
        [0.566, 0.194, 0.546],
        Frame::Upright,
        Motion::Slide {
            travel: DRAWER_TRAVEL,
        },
    ),
    // A stove's firebox, ash pit and oven doors, each on its left edge
    // (`art/props/furniture.py` `STOVE_DOORS`).
    CounterNode(
        "stove#door_fire",
        [-0.240, 0.62, -0.289],
        [0.110, 0.0, -0.024],
        [0.22, 0.15, 0.048],
        Frame::Upright,
        Motion::Hinge { most: STOVE_DOOR },
    ),
    CounterNode(
        "stove#door_ash",
        [-0.240, 0.2, -0.289],
        [0.110, 0.0, -0.024],
        [0.22, 0.1, 0.048],
        Frame::Upright,
        Motion::Hinge { most: STOVE_DOOR },
    ),
    CounterNode(
        "stove#door_oven",
        [0.030, 0.4, -0.289],
        [0.130, 0.0, -0.024],
        [0.26, 0.3, 0.048],
        Frame::Upright,
        Motion::Hinge { most: STOVE_DOOR },
    ),
    // A locker's door, hung on its left (`art/props/furniture.py`
    // `LOCKER_HINGE`).
    CounterNode(
        "locker#door",
        [-0.238, 0.93, -0.205],
        [0.238, 0.0, -0.02],
        [0.47, 1.7, 0.05],
        Frame::Upright,
        Motion::Hinge { most: LOCKER_DOOR },
    ),
    // A tool chest's lid, hinged along its back (`art/props/interiors.py`
    // `CHEST_HINGE`), lifting past upright.
    CounterNode(
        "tool_chest#lid",
        [0.0, 0.5, 0.25],
        [0.0, 0.025, -0.25],
        [0.94, 0.06, 0.54],
        Frame::Lid,
        Motion::Hinge { most: CHEST_LID },
    ),
    CounterNode(
        "counter_cabinet#door_l",
        [-0.284, 0.455, -0.298],
        [0.14, 0.0, -0.004],
        [0.281, 0.7, 0.047],
        Frame::Upright,
        Motion::Hinge { most: CABINET_DOOR },
    ),
    CounterNode(
        "counter_cabinet#door_r",
        [0.284, 0.455, -0.298],
        [-0.141, 0.0, -0.004],
        [0.281, 0.7, 0.046],
        Frame::Over,
        Motion::Hinge { most: CABINET_DOOR },
    ),
];

/// Every moving node a hand takes hold of (`model#node`): its origin in
/// its model's space, its grab box's centre from there and its size (in
/// the part's own space), and how that space is turned from the model's
/// (`capture::checks::handles`).
pub fn moving_nodes() -> impl Iterator<Item = (&'static str, Vec3, Vec3, Vec3, Quat)> {
    COUNTER_NODES.iter().map(|n| {
        let (centre, size) = n.part_box(Vec3::ONE);
        (n.0, Vec3::from(n.1), centre, size, n.4.turn())
    })
}

/// A moving node's origin in its model's space (`model#node`: a
/// counter's, a window's leaf), if it's one we move.
pub fn node_origin(drawn: &str) -> Option<Vec3> {
    COUNTER_NODES
        .iter()
        .find(|n| n.0 == drawn)
        .map(|n| Vec3::from(n.1))
        .or_else(|| {
            SASH_LEAVES
                .iter()
                .find(|n| n.0 == drawn)
                .map(|n| Vec3::from(n.1))
        })
}

/// How far a drawer pulls out (m: `DRAWER_TRAVEL`, the model's), and a
/// cabinet's door swings (radians).
pub const DRAWER_TRAVEL: f32 = 0.4;
pub const CABINET_DOOR: f32 = 1.9;
/// How far a locker's door swings open, radians: back against its hinge
/// side.
pub const LOCKER_DOOR: f32 = 2.0;
/// How far a stove's doors swing open, radians.
pub const STOVE_DOOR: f32 = 1.7;
/// How far a tool chest's lid lifts, radians: a little past upright, so
/// it stays.
pub const CHEST_LID: f32 = 1.6;

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
                    // As far as it opens clear of the building round it.
                    let clear = clear_end(&geo.pieces, &d.pieces, xf, n);
                    let motion = match n.5 {
                        Motion::Hinge { .. } => Motion::Hinge { most: clear },
                        Motion::Slide { .. } => Motion::Slide { travel: clear },
                    };
                    let part = spawn_counter_node(&mut commands, prop, xf.scale, n, motion);
                    // What's left in it, riding out with it.
                    let local = xf.compute_affine().inverse();
                    for h in geo.holds.iter().filter(|h| h.part == Some(n.0)) {
                        let in_model = local.transform_point3(h.at);
                        if in_model.x.abs() > 0.32 || in_model.z.abs() > 0.32 {
                            continue;
                        }
                        let rest = Transform::from_matrix(
                            (bxf.compute_affine() * bevy::math::Affine3A::from_translation(h.at))
                                .into(),
                        );
                        let part_rest = n.rest(prop);
                        let seat = part_rest.compute_affine().inverse() * rest.compute_affine();
                        commands.spawn((
                            super::buildings::hold_socket(
                                h.table,
                                rest.translation,
                                bxf.rotation * h.out,
                                Transform::from_matrix(seat.into()),
                            ),
                            ChildOf(part),
                        ));
                    }
                }
            }
        }
    }
}

fn spawn_counter_node(
    commands: &mut Commands,
    prop: bevy::math::Affine3A,
    scale: Vec3,
    n: &CounterNode,
    motion: Motion,
) -> Entity {
    let drawn = n.0;
    // Shut, in the world: on its hinge or at its middle, unscaled, turned
    // as its frame is (a right-hand door over, so it opens with the same
    // sense; a lid on its side, so it lifts).
    let rest = n.rest(prop);
    let (box_centre, box_size) = n.part_box(scale);
    let e = spawn_moving(commands, drawn, rest, motion, box_size, box_centre);
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
    e
}

/// A window sash's leaves (`window_sash`, `art/props/openings.py`):
/// `model#node`, the node's origin (its hinge, mid-height) in the model's
/// space, its box's centre from there and its size (read from the model),
/// whether it hangs from its right (its free edge toward -X), and which of
/// [`crate::destruction::glass::sash_panes`] is its glass. The frame
/// keeps the fixed light. All open into the room (+Z).
pub const SASH_LEAVES: [(&str, [f32; 3], [f32; 3], [f32; 3], bool, usize); 3] = [
    (
        "window_sash#leaf_l",
        [-0.44, -0.1325, 0.01],
        [0.207, 0.0, -0.024],
        [0.415, 0.715, 0.048],
        false,
        0,
    ),
    (
        "window_sash#leaf_r",
        [0.44, -0.1325, 0.01],
        [-0.207, 0.0, -0.024],
        [0.415, 0.715, 0.048],
        true,
        1,
    ),
    (
        "window_sash#fortochka",
        [-0.44, 0.3825, 0.01],
        [0.26, 0.0, -0.02],
        [0.52, 0.215, 0.04],
        false,
        2,
    ),
];

/// What the building draws of an intact sash: its frame, the leaves
/// being their own (`fit_sash`).
pub const SASH_FRAME: &str = "window_sash#frame";

/// How far a window's leaf swings in, radians.
pub const WINDOW_LEAF: f32 = 1.75;

/// Hang an intact sash's leaves, `prop` its pose in the world (scaled to
/// its opening): each a moving part on its hinge, drawn from its node of
/// the model. Returns them in [`SASH_LEAVES`]'s order.
pub fn fit_sash(commands: &mut Commands, prop: bevy::math::Affine3A) -> [Entity; 3] {
    let (scale, _, _) = prop.to_scale_rotation_translation();
    SASH_LEAVES.map(|(drawn, origin, centre, size, right, _)| {
        // Turned over so a positive turn swings the free edge into the
        // room: about -Y for a leaf hung on the left, +Y on the right
        // with its +X its free edge.
        let flip = if right {
            Quat::from_rotation_y(std::f32::consts::PI)
        } else {
            Quat::from_rotation_x(std::f32::consts::PI)
        };
        let node = bevy::math::Affine3A::from_rotation_translation(flip, Vec3::from(origin));
        let (_, rot, at) = (prop * node).to_scale_rotation_translation();
        let rest = Transform::from_translation(at).with_rotation(rot);
        let s = scale.abs();
        let box_centre = flip.inverse() * (Vec3::from(centre) * s);
        let box_size = Vec3::from(size) * s;
        let reach = 0.5 * Vec2::new(box_size.x, box_size.y).length();
        let e = spawn_moving_open(
            commands,
            drawn,
            rest,
            Motion::Hinge { most: WINDOW_LEAF },
            box_centre,
            reach.max(0.2),
        );
        let model_at = Transform::from_matrix((rest.compute_affine().inverse() * prop).into());
        commands.entity(e).insert(crate::destruction::wood::Drawn {
            model: Some(drawn),
            mat: crate::render::materials::Mat::Timber,
            at: model_at,
            size: box_size,
            range: crate::render::lod::near_range().end_margin,
            shadow: false,
        });
        e
    })
}

/// How far a container's moving part (`n`, its carcass posed by `xf` in
/// a building) opens before it meets another of the building's solid
/// pieces (not its container's own, `own`): its box swept from shut to
/// open in 48 steps, no point of it more than 2 cm into a piece. The part
/// stops there, as a door stops at a wall (`fit_counters`).
fn clear_end(
    pieces: &[super::buildings::Piece],
    own: &std::ops::Range<usize>,
    xf: &Transform,
    n: &CounterNode,
) -> f32 {
    clear_end_by(pieces, own, xf, n).0
}

/// `clear_end`, and what it meets (its material), if anything.
fn clear_end_by(
    pieces: &[super::buildings::Piece],
    own: &std::ops::Range<usize>,
    xf: &Transform,
    n: &CounterNode,
) -> (f32, Option<crate::render::materials::Mat>) {
    use super::buildings::Shape;
    let rest = n.rest(xf.compute_affine());
    let (centre, size) = n.part_box(xf.scale);
    let half = size * 0.5;
    let mut m = Moving {
        motion: n.5,
        rest,
        at: 0.0,
        open: false,
        left: false,
    };
    let end = n.5.end();
    for step in 1..=48 {
        m.at = end * step as f32 / 48.0;
        let pose = m.pose();
        for i in 0..5 {
            for j in 0..5 {
                for k in 0..3 {
                    let local = centre
                        + half
                            * Vec3::new(i as f32 / 2.0 - 1.0, j as f32 / 2.0 - 1.0, k as f32 - 1.0);
                    let p = pose.transform_point(local);
                    let hit = pieces.iter().enumerate().find(|(pi, piece)| {
                        if !piece.collide || own.contains(pi) {
                            return false;
                        }
                        let Shape::Box(size) = piece.shape else {
                            return false;
                        };
                        let q = piece.at.rotation.inverse() * (p - piece.at.translation);
                        (q.abs() - (size * 0.5 - Vec3::splat(0.02))).max_element() < 0.0
                    });
                    if let Some((_, piece)) = hit {
                        return (end * (step - 1) as f32 / 48.0, Some(piece.mat));
                    }
                }
            }
        }
    }
    (end, None)
}

/// The least of its way a container's part must open clear: a door most
/// of its swing (it may stop against a wall short of its end), a drawer
/// all of its run.
#[cfg(test)]
fn least_clear(motion: Motion) -> f32 {
    match motion {
        Motion::Hinge { most } => most * 0.75,
        Motion::Slide { travel } => travel,
    }
}

#[cfg(test)]
mod tests {
    /// Every container's door swings most of its way and every drawer
    /// runs all of its way clear of the building around it (its walls,
    /// other furniture, not its own carcass): a locker shut against a wall
    /// is a lie too (`clear_end`, `least_clear`). One buried in a drowned
    /// house's silt is plainly stuck, and stays so.
    #[test]
    fn every_container_opens_clear() {
        use super::super::buildings;
        let ids = buildings::PREFABS
            .iter()
            .chain(super::super::dam::PREFABS.iter())
            .chain(super::super::fell::PREFABS.iter())
            .copied()
            .chain(super::super::underground::ids());
        let mut bad = Vec::new();
        let mut parts = 0;
        for id in ids {
            let Some(geo) = buildings::prefab(id) else {
                continue;
            };
            for d in &geo.props {
                for (name, xf) in &d.models {
                    let Some(model) = name.strip_suffix("#carcass") else {
                        continue;
                    };
                    for n in COUNTER_NODES
                        .iter()
                        .filter(|n| n.0.starts_with(model) && n.0[model.len()..].starts_with('#'))
                    {
                        parts += 1;
                        let (clear, by) = clear_end_by(&geo.pieces, &d.pieces, xf, n);
                        // A door down in a drowned house's silt stays
                        // shut, plainly buried: no lie.
                        let buried = by == Some(crate::render::materials::Mat::Silt);
                        if clear < least_clear(n.5) - 1e-4 && !buried {
                            bad.push(format!(
                                "{id}: {} at {:.2?} opens {:.0}% clear",
                                n.0,
                                xf.translation,
                                clear / n.5.end() * 100.0
                            ));
                        }
                    }
                }
            }
        }
        println!("{parts} moving parts swept");
        assert!(parts > 20, "only {parts} parts");
        assert!(
            bad.is_empty(),
            "containers that can't open:\n{}",
            bad.join("\n")
        );
    }

    use super::*;

    #[test]
    fn the_shortest_way_round() {
        assert!((turn(3.0, -3.0) - (std::f32::consts::TAU - 6.0)).abs() < 1e-5);
        assert!((turn(0.2, 0.5) - 0.3).abs() < 1e-5);
        assert!((turn(-3.0, 3.0) + (std::f32::consts::TAU - 6.0)).abs() < 1e-5);
    }
}
