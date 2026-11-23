//! The way on in the yard: a trail of dots on the ground from the station
//! you've finished to the next, through the gate between them (playtest
//! 2026-11-22, 6:20: "nothing says to move on to the next lamp … maybe a
//! glow trail along the floor"). Since the yard's gate lines became
//! hoardings (Map design, eac4116), the next station is out of sight until
//! you're through its gate; the trail is what leads you there.
//!
//! Only the leg you're on is laid, and it's gone once you're at the
//! station. Where it runs is worked out here (headless, and tested); how
//! it looks, in `visuals` (the way-lamps' warmth, Legibility's floors).

use bevy::prelude::*;

use super::{Onboarding, STATIONS};
use crate::rig::LocalPlayer;
use crate::world::Zone;

/// The dots of the leg you're on, on the ground, in the order you walk
/// them; empty when there's none to show.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct TrailLeg {
    /// The station it leads to.
    pub to: Option<usize>,
    pub dots: Vec<Vec3>,
}

/// Between two dots (m).
pub const DOT_EVERY: f32 = 1.5;
/// No dot this near either lamp's post (m): they start and end at it.
pub const CLEAR_OF_POST: f32 = 1.5;
/// This near the station it leads to, the leg's done (m).
pub const ARRIVED: f32 = 6.0;
/// How far a dot sits over the ground (m), clear of it.
pub const DOT_LIFT: f32 = 0.02;

/// The line the trail follows from station `from` to station `to`, on
/// the ground's plan (x, z). Through the gate between them if they're in
/// different bays (gate N leads out of bay N): up to it on the track, out
/// through its gap, and past its post on the far side, the one nearer the
/// station.
pub fn route(zone: &Zone, from: usize, to: usize) -> Vec<Vec2> {
    let a = Vec2::from(STATIONS[from].at);
    let b = Vec2::from(STATIONS[to].at);
    let mut line = vec![a];
    let bay = STATIONS[from].bay;
    if STATIONS[to].bay != bay {
        let gate = zone.map.placements.iter().find(|p| {
            p.prefab == "yard_gate" && crate::world::yard::gate_index(&p.tag) == Some(bay as u8)
        });
        if let Some(g) = gate {
            let at = Vec2::from(g.at);
            // Which way along the track the next bay lies.
            let on = Vec2::new(0.0, (b.y - at.y).signum());
            let posts = crate::world::yard::gate_post_anchors(at, g.rot);
            let post = if posts[0].distance(b) < posts[1].distance(b) {
                posts[0]
            } else {
                posts[1]
            };
            line.push(at - on * 2.0);
            line.push(at + on * 1.0);
            line.push(post.lerp(at, 0.3) + on * 1.0);
        }
    }
    line.push(b);
    line
}

/// Dots every [`DOT_EVERY`] along `line`, none within [`CLEAR_OF_POST`] of
/// either end, on the ground.
pub fn dots_along(zone: &Zone, line: &[Vec2]) -> Vec<Vec3> {
    let total: f32 = line.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut out = Vec::new();
    let mut s = CLEAR_OF_POST;
    while s <= total - CLEAR_OF_POST {
        let mut left = s;
        for w in line.windows(2) {
            let len = w[0].distance(w[1]);
            if left <= len {
                let p = w[0].lerp(w[1], if len > 0.0 { left / len } else { 0.0 });
                out.push(Vec3::new(p.x, zone.ground(p.x, p.y) + DOT_LIFT, p.y));
                break;
            }
            left -= len;
        }
        s += DOT_EVERY;
    }
    out
}

/// Lays the leg you're on: from the last station to the one you're on,
/// until you're there.
pub(super) fn lay_trail(
    ob: Res<Onboarding>,
    zone: Res<Zone>,
    me: Query<&GlobalTransform, With<LocalPlayer>>,
    mut leg: ResMut<TrailLeg>,
) {
    let want = match (ob.current(), me.single()) {
        (Some(to), Ok(me)) if to > 0 => {
            let near = me.translation().xz().distance(Vec2::from(STATIONS[to].at)) < ARRIVED;
            (!near).then_some(to)
        }
        _ => None,
    };
    if want == leg.to {
        return;
    }
    leg.to = want;
    leg.dots = want.map_or(Vec::new(), |to| {
        dots_along(&zone, &route(&zone, to - 1, to))
    });
}
