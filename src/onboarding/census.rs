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

/// `EXTRACTION_ITEM_CENSUS=map`: once, a few seconds in, every building's
/// kinds (what a newcomer meets in each house), `census house: ...`.
pub fn houses(
    items: Query<(&Item, &GlobalTransform), (Without<Held>, Without<InSocket>)>,
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    buildings: Query<(&crate::world::buildings::Building, &GlobalTransform)>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut done: Local<bool>,
) {
    if *done
        || time.elapsed_secs() < 5.0
        || std::env::var("EXTRACTION_ITEM_CENSUS").ok().as_deref() != Some("map")
    {
        return;
    }
    let Some(en) = entrances else {
        return;
    };
    *done = true;
    let mut by: std::collections::BTreeMap<Entity, Vec<String>> = Default::default();
    for (item, gt) in &items {
        if let Some(b) = en.building_at(gt.translation() + Vec3::Y * 0.1) {
            let id = catalog.get(item.kind()).id.clone();
            let v = by.entry(b).or_default();
            if !v.contains(&id) {
                v.push(id);
            }
        }
    }
    for (b, kinds) in by {
        let Ok((bl, gt)) = buildings.get(b) else {
            continue;
        };
        let p = gt.translation();
        info!(
            "census house: {} at ({:.0}, {:.0}): {} kinds: {}",
            bl.prefab,
            p.x,
            p.z,
            kinds.len(),
            kinds.join(" ")
        );
    }
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
