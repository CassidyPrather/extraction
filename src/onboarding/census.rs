//! `EXTRACTION_ITEM_CENSUS=1`: which kinds of item a newcomer comes near,
//! and when (the designer: "bombarded with too many item types at once").
//! A kind is counted the first time one lies within [`NEAR`] of the player
//! anywhere but on them: loose, on a shelf, in a crate, in a case. Each is
//! logged once, `census: <t> s <kind> (<n> kinds)`, for a bot run to read.

use bevy::prelude::*;

use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{Held, InSocket, SocketOwner};
use crate::items::{Catalog, Item};
use crate::rig::LocalPlayer;

/// Near enough to see what it is, or reach it (m).
pub const NEAR: f32 = 6.0;

pub fn on() -> bool {
    std::env::var_os("EXTRACTION_ITEM_CENSUS").is_some()
}

#[allow(clippy::too_many_arguments)]
pub fn census(
    players: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    items: Query<(
        &Item,
        &GlobalTransform,
        Option<&Held>,
        Option<&InSocket>,
        Option<&InBackpack>,
    )>,
    owners: Query<&SocketOwner>,
    bags: Query<&Backpack>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut seen: Local<Vec<String>>,
    mut every: Local<f32>,
) {
    *every += time.delta_secs();
    if *every < 0.5 {
        return;
    }
    *every = 0.0;
    let Some((me, at)) = players.iter().next() else {
        return;
    };
    let at = at.translation();
    for (item, gt, held, sock, bag) in &items {
        // On you: in a hand, a slot of yours, your own pack.
        let mine = held.is_some()
            || sock.is_some_and(|s| owners.get(s.socket).is_ok_and(|o| o.0 == me))
            || bag.is_some_and(|b| bags.get(b.bag).is_ok_and(|b| b.owner == me && b.carried));
        if mine || gt.translation().distance(at) > NEAR {
            continue;
        }
        let id = &catalog.get(item.kind()).id;
        if !seen.contains(id) {
            seen.push(id.clone());
            info!(
                "census: {:.0} s {id} ({} kinds)",
                time.elapsed_secs(),
                seen.len()
            );
        }
    }
}
