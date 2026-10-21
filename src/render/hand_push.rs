//! An open hand parts what it pushes through: ground cover and ferns when
//! it's down among them, a tree's near foliage when it's up in the
//! branches (`shaders/sway.wgsl`, `shaders/tree_sway.wgsl`, their
//! `hand_a`/`hand_b`).
//!
//! Writing a sway material re-extracts every mesh that uses it (hundreds,
//! for the cover), so the hands are only written when one is actually in
//! the plants and has moved a few centimetres, or when one leaves them.

use bevy::prelude::*;

use crate::interact::Hand;
use crate::rig::{LocalPlayer, RigHand};
use crate::world::cover::SwayMaterial;
use crate::world::trees::TreeSwayMaterial;

/// How far round a hand plants part, metres.
const PART: f32 = 0.35;
/// A hand this low over the feet is among the ground cover; this high, in
/// the branches.
const COVER_TOP: f32 = 0.9;
const BRANCHES: f32 = 1.5;
/// A hand moved less than this since it was last written isn't worth
/// another write.
const MOVED: f32 = 0.05;

pub struct HandPushPlugin;

impl Plugin for HandPushPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, push_plants);
    }
}

/// Where a hand parts plants of a layer (`low`: ground cover; else
/// branches): its position and reach, or nothing (w 0) when it isn't an
/// open hand in that layer.
pub fn pusher(hand: Vec3, feet_y: f32, empty: bool, low: bool) -> Vec4 {
    let up = hand.y - feet_y;
    let inside = if low { up < COVER_TOP } else { up > BRANCHES };
    if empty && inside {
        hand.extend(PART)
    } else {
        Vec4::ZERO
    }
}

/// What was last written: (cover hands, branch hands).
#[derive(Default)]
struct Written([Vec4; 2], [Vec4; 2]);

fn changed(was: [Vec4; 2], now: [Vec4; 2]) -> bool {
    was.iter().zip(now).any(|(a, b)| {
        (a.w > 0.0) != (b.w > 0.0) || (b.w > 0.0 && a.truncate().distance(b.truncate()) > MOVED)
    })
}

fn push_plants(
    players: Query<(&GlobalTransform, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
    mut cover: ResMut<Assets<SwayMaterial>>,
    mut trees: ResMut<Assets<TreeSwayMaterial>>,
    mut written: Local<Written>,
) {
    let Some((root, children)) = players.iter().next() else {
        return;
    };
    let feet = root.translation().y;
    let mut low = [Vec4::ZERO; 2];
    let mut high = [Vec4::ZERO; 2];
    for c in children.iter() {
        let Ok((rh, hand)) = hands.get(c) else {
            continue;
        };
        // The hand you see: stopped where it met something solid.
        let at = hand.felt.unwrap_or(hand.world).translation;
        let empty = hand.holding.is_none();
        let i = rh.side.index();
        low[i] = pusher(at, feet, empty, true);
        high[i] = pusher(at, feet, empty, false);
    }
    if changed(written.0, low) {
        written.0 = low;
        for (_, m) in cover.iter_mut() {
            m.extension.hand_a = low[0];
            m.extension.hand_b = low[1];
        }
    }
    if changed(written.1, high) {
        written.1 = high;
        for (_, m) in trees.iter_mut() {
            m.extension.hand_a = high[0];
            m.extension.hand_b = high[1];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An open hand parts the cover only while it's down in it, and the
    /// branches only while it's up in them; a full hand parts nothing.
    #[test]
    fn a_hand_parts_only_the_plants_it_is_among() {
        let feet = 10.0;
        let low = Vec3::new(0.0, 10.5, 0.0);
        let high = Vec3::new(0.0, 11.8, 0.0);
        assert_eq!(pusher(low, feet, true, true).w, PART);
        assert_eq!(pusher(low, feet, true, false).w, 0.0);
        assert_eq!(pusher(high, feet, true, false).w, PART);
        assert_eq!(pusher(high, feet, true, true).w, 0.0);
        assert_eq!(pusher(low, feet, false, true).w, 0.0, "a full hand");
    }

    /// Writes only when a hand comes in or goes, or moves a few cm.
    #[test]
    fn plants_are_rewritten_only_when_worth_it() {
        let a = [Vec4::new(0.0, 0.0, 0.0, PART), Vec4::ZERO];
        let nudged = [Vec4::new(0.02, 0.0, 0.0, PART), Vec4::ZERO];
        let moved = [Vec4::new(0.08, 0.0, 0.0, PART), Vec4::ZERO];
        assert!(!changed(a, a));
        assert!(!changed(a, nudged));
        assert!(changed(a, moved));
        assert!(changed(a, [Vec4::ZERO; 2]), "it left");
        assert!(!changed([Vec4::ZERO; 2], [Vec4::ZERO; 2]));
    }
}
