//! What artifacts do, and who is carrying what.
//!
//! - **Featherstone**: falls up. Loose, it hovers a little above the
//!   ground; carried, it takes kilograms off your load.
//! - **Aegis knot**: while you carry it, the first hit glances off; then it
//!   needs a minute and a half to knot itself back up.
//! - **Hearth**: power gadgets plugged into it never run down (with the
//!   gadget work).
//! - **Glass eye**: hold it to your eye and the walls go thin (client side,
//!   `anomaly::visuals`).
//! - **Lodestone** (from wells): rounds fired at whoever carries it curve
//!   aside (`wells::bend_rounds`).
//! - **Mirror shard** (from mirrors): held and used, it steps you a few
//!   metres ahead, once a minute (`mirrors::shards`).
//!
//! [`CarriedBy`] is kept up to date for every item a player has on them —
//! in a hand, in a body slot, in a worn backpack, or seated in something
//! that is — for weight, attention and these effects.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{Held, InSocket, SocketOwner};
use crate::items::{Catalog, Item};
use crate::player::Player;

/// The player this item is on, wherever on them it is.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarriedBy(pub Entity);

/// An artifact sealed in a lead-lined case: it draws no attention, and
/// does nothing for whoever carries it (no aegis, no hearth, no lift).
#[derive(Component, Clone, Copy, Debug)]
pub struct Shielded;

/// Artifact effects active on a player.
#[derive(Component, Debug, Default, Clone)]
pub struct ArtifactEffects {
    pub aegis: bool,
    /// Seconds until the aegis can turn another hit.
    pub aegis_recharge: f32,
    pub hearth: bool,
    pub glass_eye: Option<Entity>,
}

/// Recharge time for the aegis knot.
pub const AEGIS_RECHARGE: f32 = 90.0;

pub struct ArtifactPlugin;

impl Plugin for ArtifactPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(|add: On<Add, Player>, mut commands: Commands| {
            commands
                .entity(add.entity)
                .insert(ArtifactEffects::default());
        })
        .add_systems(
            FixedUpdate,
            (track_carriers, shielding, artifact_effects, featherstones)
                .chain()
                .before(crate::interact::body::CarriedMassSystems),
        );
    }
}

/// Works out who carries each item.
pub fn track_carriers(
    mut commands: Commands,
    items: Query<
        (
            Entity,
            Option<&Held>,
            Option<&InSocket>,
            Option<&InBackpack>,
            Option<&CarriedBy>,
        ),
        With<Item>,
    >,
    socket_owners: Query<&SocketOwner>,
    bags: Query<&Backpack>,
    players: Query<(), With<Player>>,
) {
    // One step: who or what directly holds each item.
    enum Up {
        Player(Entity),
        Item(Entity),
        Nobody,
    }
    let direct = |held: Option<&Held>, sock: Option<&InSocket>, bag: Option<&InBackpack>| {
        if let Some(h) = held {
            return Up::Player(h.player);
        }
        if let Some(s) = sock {
            return match socket_owners.get(s.socket) {
                Ok(o) if players.contains(o.0) => Up::Player(o.0),
                Ok(o) => Up::Item(o.0),
                Err(_) => Up::Nobody,
            };
        }
        if let Some(b) = bag {
            return match bags.get(b.bag) {
                Ok(bp) if bp.carried => Up::Player(bp.owner),
                _ => Up::Nobody,
            };
        }
        Up::Nobody
    };
    for (e, held, sock, bag, current) in &items {
        let mut up = direct(held, sock, bag);
        // Follow item-in-item (a magazine in a gun in a sling) a few levels.
        for _ in 0..3 {
            let Up::Item(parent) = up else {
                break;
            };
            up = match items.get(parent) {
                Ok((_, h, s, b, _)) => direct(h, s, b),
                Err(_) => Up::Nobody,
            };
        }
        let want = match up {
            Up::Player(p) => Some(CarriedBy(p)),
            _ => None,
        };
        if want.as_ref() != current {
            match want {
                Some(c) => {
                    commands.entity(e).insert(c);
                }
                None => {
                    commands.entity(e).remove::<CarriedBy>();
                }
            }
        }
    }
}

/// Marks what sits in a lead-lined case.
fn shielding(
    mut commands: Commands,
    items: Query<(Entity, Option<&InSocket>, Has<Shielded>), With<Item>>,
    socket_owners: Query<&SocketOwner>,
    cases: Query<(), With<crate::gadgets::LeadCase>>,
) {
    for (e, sock, shielded) in &items {
        let sealed = sock
            .and_then(|s| socket_owners.get(s.socket).ok())
            .is_some_and(|o| cases.contains(o.0));
        if sealed && !shielded {
            commands.entity(e).insert(Shielded);
        } else if !sealed && shielded {
            commands.entity(e).remove::<Shielded>();
        }
    }
}

fn artifact_effects(
    mut players: Query<(Entity, &mut ArtifactEffects)>,
    carried: Query<(Entity, &Item, &CarriedBy, Option<&Held>), Without<Shielded>>,
    catalog: Res<Catalog>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (p, mut fx) in &mut players {
        let mut aegis = false;
        let mut hearth = false;
        let mut eye = None;
        for (e, item, by, held) in &carried {
            if by.0 != p {
                continue;
            }
            match catalog.get(item.kind()).id.as_str() {
                "art_aegis" => aegis = true,
                "art_hearth" => hearth = true,
                "art_glasseye" if held.is_some() => eye = Some(e),
                _ => {}
            }
        }
        fx.aegis = aegis;
        fx.hearth = hearth;
        fx.glass_eye = eye;
        fx.aegis_recharge = (fx.aegis_recharge - dt).max(0.0);
    }
}

/// Loose featherstones drift up to hover at about chest height.
fn featherstones(
    mut stones: Query<
        (&Item, &GlobalTransform, &ComputedMass, Forces),
        (Without<RigidBodyDisabled>, Without<Held>),
    >,
    catalog: Res<Catalog>,
    zone: Res<crate::world::Zone>,
) {
    for (item, gt, mass, mut forces) in &mut stones {
        if catalog.get(item.kind()).lift_kg <= 0.0 {
            continue;
        }
        let p = gt.translation();
        let target = zone.ground(p.x, p.z) + 1.4;
        let vy = forces.linear_velocity().y;
        let m = mass.value().max(0.05);
        let up = 9.81 + (target - p.y).clamp(-2.0, 2.0) * 4.0 - vy * 2.5;
        forces.apply_force(Vec3::Y * up * m);
    }
}
