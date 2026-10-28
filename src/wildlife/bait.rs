//! Bait: food you put down draws a herd to it (`docs/WILDLIFE.md`, "Using
//! them"). Something to eat (any `Food` item: carrots, bread, a cracker)
//! that a hand puts down or throws is [`Bait`]; loot already lying about
//! isn't. The nearest herd within [`DRAW`] that isn't running makes it
//! home and walks over; at it, they eat it in [`EATS_FOR`] seconds and it's
//! gone. Scared off, they come back to it once they've settled. A herd
//! about you hides your walking among theirs (`herd::Among`), and anything
//! coming crashes them off (`herd::bark_alarm`).

use bevy::prelude::*;

use super::herd::{Herd, State};
use crate::interact::body::InBackpack;
use crate::interact::{Held, InSocket};
use crate::items::{Catalog, Item};

/// A herd this near smells it out (m)...
pub const DRAW: f32 = 250.0;
/// ...eats it within this of their leader (m)...
pub const EAT_NEAR: f32 = 6.0;
/// ...and it lasts them this long (s).
pub const EATS_FOR: f32 = 120.0;

/// Food a hand put down: how long it's been eaten for (s).
#[derive(Component, Debug, Default)]
pub struct Bait {
    pub eaten: f32,
}

pub(crate) fn put_down(
    remove: On<Remove, Held>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut commands: Commands,
) {
    let food = items
        .get(remove.entity)
        .is_ok_and(|i| catalog.get(i.kind()).category == extraction_core::items::Category::Food);
    if food {
        commands.entity(remove.entity).try_insert(Bait::default());
    }
}

pub(crate) fn picked_up(add: On<Add, Held>, mut commands: Commands) {
    commands.entity(add.entity).try_remove::<Bait>();
}

#[allow(clippy::type_complexity)]
pub(crate) fn lure(
    mut commands: Commands,
    mut baits: Query<
        (Entity, &GlobalTransform, &mut Bait),
        (Without<Held>, Without<InBackpack>, Without<InSocket>),
    >,
    mut herds: Query<&mut Herd>,
    nav: Option<Res<crate::robots::walker::Nav>>,
    zone: Res<crate::world::Zone>,
    time: Res<Time>,
) {
    let Some(nav) = nav else {
        return;
    };
    let dt = time.delta_secs();
    let taken: Vec<Entity> = herds.iter().filter_map(|h| h.lure).collect();
    for mut herd in &mut herds {
        let Some(lead) = herd.leader().map(|b| (b.pos, b.state)) else {
            continue;
        };
        // Gone (eaten, picked up again, in a pocket): free.
        if herd.lure.is_some_and(|e| !baits.contains(e)) {
            herd.lure = None;
        }
        if herd.lure.is_none() {
            herd.lure = baits
                .iter()
                .filter(|(e, _, _)| !taken.contains(e))
                .map(|(e, g, _)| (e, g.translation()))
                .filter(|(_, p)| {
                    p.xz().distance(lead.0.xz()) < DRAW && super::herd::open(&nav.0, &zone, p.xz())
                })
                .min_by(|a, b| {
                    a.1.xz()
                        .distance(lead.0.xz())
                        .total_cmp(&b.1.xz().distance(lead.0.xz()))
                })
                .map(|(e, _)| e);
        }
        let Some(e) = herd.lure else {
            continue;
        };
        let Ok((_, g, mut bait)) = baits.get_mut(e) else {
            continue;
        };
        let at = g.translation();
        // Running or watching something: the food can wait.
        if matches!(
            lead.1,
            State::Flee { .. } | State::Alert { .. } | State::Keep { .. }
        ) {
            continue;
        }
        herd.lure(at.xz());
        if lead.0.xz().distance(at.xz()) < EAT_NEAR {
            bait.eaten += dt;
            if bait.eaten >= EATS_FOR {
                commands.entity(e).try_despawn();
                herd.lure = None;
            }
        }
    }
}
