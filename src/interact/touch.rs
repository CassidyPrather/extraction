//! What an empty hand feels when it meets something (VR, the physics
//! hands' bodies: [`super::physics_hands`]): a haptic shape for what it's
//! made of (`rig::shapes`), stronger the faster the hand was going. A fast
//! slap is also heard: [`HandStruck`], for Audio to voice by material.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

use super::Hand;
use super::physics_hands::HandBody;
use crate::render::materials::Mat;
use crate::rig::RigHand;
use crate::rig::Side;
use crate::rig::shapes::{self, HapticShape, Shape};

/// A hand this fast (m/s) or faster strikes rather than touches: heard.
pub const SLAP: f32 = 1.5;
/// A hand this slow puts a light touch on; this fast, a full one.
const LIGHT: f32 = 0.2;
const FULL: f32 = 2.0;
/// The same hand feels nothing new for this long after a touch.
const SETTLE: f32 = 0.12;

pub struct TouchPlugin;

impl Plugin for TouchPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HandStruck>()
            .add_systems(FixedPostUpdate, touches.after(PhysicsSystems::Writeback));
    }
}

/// What a hand met is made of, as the hand tells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stuff {
    Wood,
    Metal,
    Glass,
    Stone,
    /// A robot's soft parts.
    Skin,
    /// Chain-link, fence wire.
    Wire,
}

impl Stuff {
    pub fn shape(self) -> Shape {
        match self {
            Stuff::Wood => shapes::WOOD,
            Stuff::Metal => shapes::METAL,
            Stuff::Glass => shapes::GLASS,
            Stuff::Stone => shapes::STONE,
            Stuff::Skin => shapes::SKIN,
            Stuff::Wire => shapes::WIRE,
        }
    }

    /// What a built material is, to the hand.
    pub fn of_mat(m: Mat) -> Stuff {
        match m {
            Mat::Planks
            | Mat::LogWall
            | Mat::WoodFloor
            | Mat::Timber
            | Mat::Shingles
            | Mat::GunWood
            | Mat::BarkSpruce
            | Mat::BarkBirch
            | Mat::BarkPine => Stuff::Wood,
            Mat::RustyMetal
            | Mat::PaintedMetal
            | Mat::Corrugated
            | Mat::DarkMetal
            | Mat::RobotShell
            | Mat::GunMetal => Stuff::Metal,
            Mat::Glass => Stuff::Glass,
            _ => Stuff::Stone,
        }
    }
}

/// A hand struck something, hard enough to hear.
#[derive(Message, Clone, Copy, Debug)]
pub struct HandStruck {
    pub at: Vec3,
    pub stuff: Stuff,
    /// m/s.
    pub speed: f32,
    pub side: Side,
}

/// What the collider `e` (or the thing it's part of) is made of, met at
/// `at`.
#[allow(clippy::type_complexity)]
fn stuff_of(
    e: Entity,
    at: Vec3,
    parents: &Query<&ChildOf>,
    things: &Query<(
        Option<&crate::robots::RobotPart>,
        Option<&crate::items::Item>,
        Option<&crate::world::surfaces::Surfaces>,
        Has<crate::world::structures::Fence>,
        Has<crate::destruction::wood::Timber>,
        &GlobalTransform,
    )>,
    catalog: &crate::items::Catalog,
) -> Stuff {
    let mut up = e;
    for _ in 0..4 {
        if let Ok((part, item, surfaces, fence, timber, gt)) = things.get(up) {
            if let Some(part) = part {
                return if matches!(part.role, crate::robots::PartRole::Armour(_)) {
                    Stuff::Metal
                } else {
                    Stuff::Skin
                };
            }
            if fence {
                return Stuff::Wire;
            }
            if timber {
                return Stuff::Wood;
            }
            if let Some(item) = item {
                let tags = &catalog.get(item.kind()).tags;
                let has = |t: &str| tags.iter().any(|x| x == t);
                return if has("glass") {
                    Stuff::Glass
                } else if has("metal") || has("steel") {
                    Stuff::Metal
                } else {
                    Stuff::Wood
                };
            }
            if let Some(m) = surfaces.and_then(|s| s.at(gt, at)) {
                return Stuff::of_mat(m);
            }
        }
        match parents.get(up) {
            Ok(p) => up = p.parent(),
            Err(_) => break,
        }
    }
    // The ground, rock, anything else of the world's.
    Stuff::Stone
}

/// A hand's body meeting something: its shape in that hand, by speed; a
/// strike heard.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn touches(
    mut started: MessageReader<CollisionStart>,
    bodies: Query<(&HandBody, &Position)>,
    hands: Query<(&Hand, &RigHand)>,
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
    mut shapes: MessageWriter<HapticShape>,
    mut struck: MessageWriter<HandStruck>,
    time: Res<Time>,
    mut last: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    for c in started.read() {
        let (body, other) = match (bodies.get(c.collider1), bodies.get(c.collider2)) {
            (Ok(b), _) => (b, c.collider2),
            (_, Ok(b)) => (b, c.collider1),
            _ => continue,
        };
        let (hb, pos) = body;
        let Ok((hand, rh)) = hands.get(hb.hand) else {
            continue;
        };
        if last.get(&hb.hand).is_some_and(|t| now - t < SETTLE) {
            continue;
        }
        last.insert(hb.hand, now);
        let speed = hand.velocity.length();
        // Where it met: a little ahead of the hand, the way it was going.
        let at = pos.0 + hand.velocity.normalize_or_zero() * 0.06;
        let stuff = stuff_of(other, at, &parents, &things, &catalog);
        let scale = 0.4 + 0.6 * ((speed - LIGHT) / (FULL - LIGHT)).clamp(0.0, 1.0);
        shapes.write(HapticShape {
            side: rh.side,
            shape: stuff.shape(),
            scale,
        });
        if speed >= SLAP {
            struck.write(HandStruck {
                at,
                stuff,
                speed,
                side: rh.side,
            });
        }
    }
}
