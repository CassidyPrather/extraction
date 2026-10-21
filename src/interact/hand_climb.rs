//! Climbing with your hands, as in Into the Radius. Grip a ladder anywhere
//! and the hand holds there: move it down and the body goes up, hand over
//! hand; at the top you're over onto what it leads to. Press both palms
//! down on a ledge, a low wall or a window's sill (waist to head high) and
//! push, and you rise; once its top is at your waist you're up on it (or
//! over it and down the far side, if it's thin).
//! Let go with both hands, or lift your palms, and you fall.
//!
//! Or grip the edge: close a hand on a ledge, a wall top (up to [`EDGE_HIGH`],
//! over your head: a pull-up), a fence's top rail or the truck's tailgate,
//! and it holds there as a palm does; two holds (palms or grips) pull you
//! up, and over a fence or a thin wall you go down the far side. How fast
//! you can pull yourself up goes with what you carry ([`pull_rate`]: the
//! load, and whatever you're hauling).
//!
//! The comfort setting `climb_teleport` skips the pulling for those it
//! makes sick: a grip on a ladder, or both palms on a ledge, puts you at
//! the top. On a ladder the stick climbs too (flat mode, or tired arms).
//!
//! Holding works like the rope (`gadgets::climbing`): the hand's hold is a
//! point in the world, and each tick the body moves so the hand is back
//! on it. The body is `Climbing` while anything holds it: no gravity, no
//! walking.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{GrabPoint, Grabbable, Hand, InteractSystems};
use crate::gadgets::climbing::{Climbing, RopeGrip};
use crate::player::{Body, Player};
use crate::rig::comfort::Comfort;
use crate::rig::{LocalPlayer, RigDriver, RigHand, RigInput};
use crate::world::Layer;

/// Climbing a ladder with the stick, m/s.
pub const LADDER_SPEED: f32 = 1.4;
/// Fastest the hands can haul the body, m/s.
const MAX_PULL: f32 = 4.0;
/// A palm counts as on a top when it's within this over it (the hand's
/// origin is the middle of the fist, a few centimetres over the palm).
const ON_TOP: f32 = 0.07;
/// Tops you can mantle: this far over your feet.
pub const LEDGE_LOW: f32 = 0.5;
pub const LEDGE_HIGH: f32 = 1.9;
/// A top gripped from below can be this high over your feet: a wall's top
/// above your head, reached up for.
pub const EDGE_HIGH: f32 = 2.3;
/// Pulling yourself up gets slower with load past this (kg), and with
/// [`LOAD_LIMP`] kg on you, barely at all.
pub const LOAD_EASY: f32 = 12.0;
pub const LOAD_LIMP: f32 = 55.0;

/// How fast you can pull yourself up (a share of the unladen rate) with
/// this much on you, kg.
pub fn pull_rate(kg: f32) -> f32 {
    (1.0 - (kg - LOAD_EASY).max(0.0) / (LOAD_LIMP - LOAD_EASY)).clamp(0.12, 1.0)
}
/// Pushed up until the top is at your waist (this far over your feet),
/// you're on it: one pull from a ledge at head height gets you there.
const OVER: f32 = 1.0;
/// ...but only once you've actually pushed yourself up this far since both
/// palms went down: leaning on a table doesn't put you on it.
const RISE: f32 = 0.12;
/// How far past the hands you land on a top (or beyond a thin wall).
const MANTLE_IN: f32 = 0.35;
/// A planted palm lifts off once it's this far above where it was put, or
/// slides this far.
const LIFT: f32 = 0.05;
const SLIDE: f32 = 0.15;
/// A gripped top's edge is within this of the hand.
const EDGE_REACH: f32 = 0.12;

pub struct HandClimbPlugin;

impl Plugin for HandClimbPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ClimbedOver>().add_systems(
            FixedUpdate,
            (plant_palms, climb_with_hands)
                .chain()
                .after(InteractSystems::Grab)
                .before(crate::player::move_bodies),
        );
    }
}

/// A ladder you can climb with your hands: its rails from `bottom` to
/// `top` (the middle between them), `width` apart, standing out from what
/// it's on toward `out`. `over` is where you're put when you reach the
/// top (the gallery or roof it leads to); `None`: it leads nowhere, you
/// just hang at the top.
#[derive(Component, Clone, Copy, Debug)]
pub struct Ladder {
    pub bottom: Vec3,
    pub top: Vec3,
    pub width: f32,
    pub out: Vec3,
    pub over: Option<Vec3>,
}

impl Ladder {
    /// The ladder as something hands find: a grabbable box over its rails
    /// and rungs, reaching a little out from them (no body of its own: the
    /// rungs you see are the world's, or nothing to collide with).
    pub fn bundle(self) -> impl Bundle {
        let up = (self.top - self.bottom).normalize_or(Vec3::Y);
        let len = self.bottom.distance(self.top);
        let centre = (self.bottom + self.top) * 0.5 + self.out * 0.08;
        (
            Name::new("Ladder"),
            self,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            Collider::cuboid(self.width + 0.12, len + 0.1, 0.26),
            CollisionLayers::new(Layer::Item, [Layer::Hand]),
            Sensor,
            Transform::from_translation(centre).looking_to(-self.out, up),
            Visibility::default(),
        )
    }
}

/// Not to be climbed: the palms and grips find nothing on it. The
/// quarantine perimeter's `world::structures::Fence` and anything of a
/// robot's (a sentry's mast) count as this too.
#[derive(Component, Clone, Copy, Debug)]
pub struct NoClimb;

/// What a hand is holding the body by, at `at` (world).
#[derive(Component, Clone, Copy, Debug)]
pub struct HandHold {
    pub on: Hold,
    pub at: Vec3,
    pub since: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hold {
    /// Gripping a ladder.
    Ladder(Entity),
    /// A palm pressed on a top this high.
    Ledge { top: f32 },
    /// A hand gripping the edge of a top this high.
    Edge { top: f32 },
}

/// Climbing on hand holds (as against a rope), so it's ours to end; and
/// how high the feet were when it began.
#[derive(Component)]
struct OnHands {
    from: f32,
}

/// Over the top: onto what a ladder leads to, or up onto a ledge.
/// `teleport` when the comfort setting put you there at once (the view
/// blinks: `render::blink`).
#[derive(Message, Clone, Copy, Debug)]
pub struct ClimbedOver {
    pub player: Entity,
    pub teleport: bool,
}

/// An empty, open hand pressed on a level top within mantling height
/// plants there; it lifts when raised or slid off, or when it grips.
/// Headset hands only: flat mode's and the bots' hands are held still in
/// front, and over a counter they'd pin the body.
#[allow(clippy::type_complexity)]
fn plant_palms(
    mut commands: Commands,
    barred: Query<
        (),
        Or<(
            With<NoClimb>,
            With<crate::world::structures::Fence>,
            With<crate::robots::Robot>,
        )>,
    >,
    parents: Query<&ChildOf>,
    players: Query<(&Transform, &Children), (With<Player>, With<LocalPlayer>)>,
    hands: Query<(Entity, &Hand, Option<&HandHold>), With<RigHand>>,
    driver: Res<RigDriver>,
    spatial: SpatialQuery,
    time: Res<Time>,
) {
    if *driver != RigDriver::Xr {
        return;
    }
    for (root, children) in &players {
        for c in children.iter() {
            let Ok((e, hand, hold)) = hands.get(c) else {
                continue;
            };
            let p = hand.world.translation;
            let busy = hand.grip_down || hand.holding.is_some() || hand.handle.is_some();
            match hold.map(|h| h.on) {
                Some(Hold::Ladder(_)) => continue,
                // A grip on an edge holds until the hand opens.
                Some(Hold::Edge { .. }) => {
                    if !hand.grip_down || hand.holding.is_some() {
                        commands.entity(e).remove::<HandHold>();
                    }
                    continue;
                }
                Some(Hold::Ledge { .. }) => {
                    let at = hold.unwrap().at;
                    if busy || p.y > at.y + LIFT || p.xz().distance(at.xz()) > SLIDE {
                        commands.entity(e).remove::<HandHold>();
                    }
                    continue;
                }
                None if busy
                    && !(hand.grip_down && hand.holding.is_none() && hand.handle.is_none()) =>
                {
                    continue;
                }
                None => {}
            }
            let gripping = hand.grip_down;
            // The top under the palm, found from a little above it (the
            // tracked hand may be pressed into it).
            let from = p + Vec3::Y * 0.25;
            let Some(hit) = spatial.cast_ray(
                from,
                Dir3::NEG_Y,
                0.25 + ON_TOP,
                true,
                &SpatialQueryFilter::from_mask([Layer::World]),
            ) else {
                continue;
            };
            if hit.normal.y < 0.7 || hit.distance < 0.01 {
                continue;
            }
            // Not the map's perimeter, nor a robot's kit.
            let mut up = Some(hit.entity);
            let mut no = false;
            for _ in 0..4 {
                let Some(e) = up else { break };
                no |= barred.contains(e);
                up = parents.get(e).ok().map(|p| p.parent());
            }
            if no {
                continue;
            }
            let top = from.y - hit.distance;
            let over_feet = top - root.translation.y;
            let high = if gripping { EDGE_HIGH } else { LEDGE_HIGH };
            if !(LEDGE_LOW..=high).contains(&over_feet) || p.y > top + ON_TOP {
                continue;
            }
            // A grip needs an edge to close on: the top ends within a hand's
            // width of the hand, toward you (the near edge of a wall, a
            // rail, a tailgate) and not in the middle of a table.
            if gripping {
                let toward = (root.translation - p).with_y(0.0).normalize_or_zero();
                let beside = p + toward * EDGE_REACH + Vec3::Y * 0.25;
                let filter = SpatialQueryFilter::from_mask([Layer::World]);
                let edge = spatial
                    .cast_ray(beside, Dir3::NEG_Y, 0.25 + 0.15, true, &filter)
                    .is_none();
                if !edge {
                    continue;
                }
            }
            commands.entity(e).insert(HandHold {
                on: if gripping {
                    Hold::Edge { top }
                } else {
                    Hold::Ledge { top }
                },
                at: p,
                since: time.elapsed_secs(),
            });
        }
    }
}

/// The body follows its holds: the newest grip on a ladder, or both palms
/// on a ledge (one palm doesn't hold you). Over the top when you get there.
#[allow(clippy::type_complexity)]
fn climb_with_hands(
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &RigInput,
            &mut Body,
            &mut Transform,
            &Children,
            Option<&OnHands>,
        ),
        With<Player>,
    >,
    hands: Query<(Entity, &Hand, Option<&HandHold>, Has<RopeGrip>), With<RigHand>>,
    ladders: Query<&Ladder>,
    comfort: Option<Res<Comfort>>,
    mut over: MessageWriter<ClimbedOver>,
    spatial: SpatialQuery,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    let teleport = comfort.is_some_and(|c| c.climb_teleport);
    for (player, input, mut body, mut root, children, held) in &mut players {
        let on_hands = held.is_some();
        let risen = held.map_or(0.0, |h| root.translation.y - h.from);
        let mut ladder: Option<(Entity, &Hand, HandHold)> = None;
        let mut ledge: Vec<(&Hand, HandHold)> = Vec::new();
        let mut roped = false;
        for c in children.iter() {
            let Ok((e, hand, hold, rope)) = hands.get(c) else {
                continue;
            };
            roped |= rope;
            match hold.map(|h| (h.on, *h)) {
                Some((Hold::Ladder(_), h)) if ladder.is_none_or(|l| h.since > l.2.since) => {
                    ladder = Some((e, hand, h));
                }
                Some((Hold::Ledge { .. } | Hold::Edge { .. }, h)) => ledge.push((hand, h)),
                _ => {}
            }
        }
        let let_go = |commands: &mut Commands| {
            for c in children.iter() {
                if hands.contains(c) {
                    commands.entity(c).remove::<HandHold>();
                }
            }
        };
        // A hold on a ladder; else both palms down.
        let pull = if let Some((hand_e, hand, hold)) = ladder {
            let Hold::Ladder(l) = hold.on else {
                continue;
            };
            let Ok(l) = ladders.get(l) else {
                commands.entity(hand_e).remove::<HandHold>();
                continue;
            };
            // The stick climbs: the hold slides up (or down) the ladder.
            let stick = input.locomotion.y;
            let mut at = hold.at;
            if stick.abs() > 0.2 {
                let up = (l.top - l.bottom).normalize_or(Vec3::Y);
                at += up * stick * LADDER_SPEED * dt;
                at.y = at.y.clamp(l.bottom.y, l.top.y);
                commands.entity(hand_e).insert(HandHold { at, ..hold });
            }
            let at_top = at.y > l.top.y - 0.3 && root.translation.y > l.top.y - 1.7;
            if let (true, Some(to)) = (teleport || at_top, l.over) {
                stand_at(&mut root, input, to);
                over.write(ClimbedOver { player, teleport });
                body.velocity = Vec3::ZERO;
                let_go(&mut commands);
                end(&mut commands, player, on_hands, roped);
                continue;
            }
            (at - hand.world.translation) / dt
        } else if ledge.len() >= 2 {
            let top = ledge
                .iter()
                .map(|(_, h)| match h.on {
                    Hold::Ledge { top } | Hold::Edge { top } => top,
                    _ => f32::MIN,
                })
                .fold(f32::MIN, f32::max);
            if teleport || (risen >= RISE && root.translation.y > top - OVER) {
                let mid = ledge.iter().map(|(_, h)| h.at).sum::<Vec3>() / ledge.len() as f32;
                let head = root.transform_point(input.head.translation);
                let inward = (mid - head).with_y(0.0).normalize_or_zero();
                let land = mid + inward * MANTLE_IN;
                // Onto the top if it goes on that far; else down the far
                // side of a thin wall.
                let from = land.with_y(top + 0.5);
                let ground = spatial
                    .cast_ray(
                        from,
                        Dir3::NEG_Y,
                        top - root.translation.y + 3.0,
                        true,
                        &SpatialQueryFilter::from_mask([Layer::World]),
                    )
                    .map_or(land.with_y(top), |h| from - Vec3::Y * h.distance);
                stand_at(&mut root, input, ground + Vec3::Y * 0.02);
                over.write(ClimbedOver { player, teleport });
                body.velocity = Vec3::ZERO;
                let_go(&mut commands);
                end(&mut commands, player, on_hands, roped);
                continue;
            }
            ledge
                .iter()
                .map(|(hand, h)| (h.at - hand.world.translation) / dt)
                .sum::<Vec3>()
                / ledge.len() as f32
        } else {
            if on_hands {
                end(&mut commands, player, true, roped);
            }
            continue;
        };
        if !on_hands {
            commands.entity(player).insert((
                OnHands {
                    from: root.translation.y,
                },
                Climbing,
            ));
        }
        // What you carry (and haul) slows you: the arms lift less of it.
        let rate = pull_rate(body.carried_kg) * (1.0 - body.hauling);
        body.velocity = pull.clamp_length_max(MAX_PULL * rate);
        body.airborne = 0.0;
    }
}

/// Put the player's feet at `at` (the playspace moves so the head, which
/// the feet stand under, comes down there).
fn stand_at(root: &mut Transform, input: &RigInput, at: Vec3) {
    let off = root.rotation * input.head.translation.with_y(0.0);
    root.translation = at - off;
}

/// Hands no longer hold the body (the rope may still).
fn end(commands: &mut Commands, player: Entity, on_hands: bool, roped: bool) {
    if !on_hands {
        return;
    }
    commands.entity(player).remove::<OnHands>();
    if !roped {
        commands.entity(player).remove::<Climbing>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::buildings::{Piece, Shape};

    /// Whether a point is inside a building piece (its shapes are full
    /// sizes, centred on its transform).
    fn inside(p: &Piece, at: Vec3) -> bool {
        let l = p.at.compute_affine().inverse().transform_point3(at);
        match p.shape {
            Shape::Box(s) | Shape::Prism(s) => (l.abs() * 2.0).cmple(s + 0.01).all(),
            Shape::Cylinder(r, h) => l.y.abs() * 2.0 <= h + 0.01 && l.xz().length() <= r + 0.01,
        }
    }

    /// The world's ladders (World models' prefabs): each runs up, and its
    /// top puts you on something of the prefab's, not in the air.
    #[test]
    fn the_prefab_ladders_lead_somewhere() {
        for id in ["water_tower", "boxcar", "ropeway_pylon"] {
            let geo = crate::world::buildings::prefab(id).expect(id);
            assert!(!geo.ladders.is_empty(), "{id} has no ladder");
            for l in &geo.ladders {
                assert!(l.top.y - l.bottom.y > 1.5, "{id}: {l:?} doesn't climb");
                let over = l
                    .over
                    .unwrap_or_else(|| panic!("{id}: {l:?} leads nowhere"));
                assert!(
                    over.y > l.top.y - 1.0 && over.distance(l.top) < 2.5,
                    "{id}: {l:?} tops out far from its top"
                );
                // Something of the prefab's just under the top-out point.
                let under = geo
                    .pieces
                    .iter()
                    .any(|p| p.collide && inside(p, over - Vec3::Y * 0.1));
                assert!(under, "{id}: nothing to stand on at {over}");
            }
        }
    }
}
