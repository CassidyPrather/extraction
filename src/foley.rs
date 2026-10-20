//! What your kit sounds like as you move, and how far it carries.
//!
//! On every step, whatever a player carries rattles a little:
//!
//! - **Plates** (`steel_plate` carried anywhere) knock in their carrier.
//! - **Sling swivels** jingle, one per slung long gun.
//! - **The pack's contents** shift, the more the heavier it's loaded
//!   (nothing under [`PACK_QUIET_KG`]).
//! - **Water** sloshes in each bottle.
//!
//! Each part has a reach: how far a robot with ordinary hearing notices it
//! at a walk (m). Like a footstep, a run carries further and a crouch less
//! (`robots::senses::footstep_reach`'s pace and crouch). The parts sum by
//! power into one [`Noise`] the robots hear (louder kit is heard farther),
//! and a [`GearRattle`] the audio plays, each part at a level by its reach.
//! `scripts/audio_mix.py` checks that each part's sound carries about as
//! far for your ears as its reach says it does for theirs.
//!
//! Logic only: runs headless.

use bevy::prelude::*;

use crate::anomaly::artifacts::CarriedBy;
use crate::interact::body::Backpack;
use crate::interact::{InSocket, Socket, SocketKind};
use crate::items::{Catalog, Item};
use crate::player::{Body, Footstep, Player};
use crate::robots::Noise;
use extraction_core::inventory::SlotKind;

/// A plate knocking in its carrier, heard to (m), per plate.
pub const PLATE_REACH: f32 = 5.0;
/// A slung gun's swivels, per gun.
pub const SWIVEL_REACH: f32 = 3.0;
/// The pack's contents shifting, per kg past [`PACK_QUIET_KG`], up to
/// [`PACK_REACH_MAX`].
pub const PACK_REACH_PER_KG: f32 = 0.6;
pub const PACK_QUIET_KG: f32 = 2.0;
pub const PACK_REACH_MAX: f32 = 8.0;
/// Water sloshing, per bottle.
pub const SLOSH_REACH: f32 = 2.5;

/// What a player carries that makes a noise.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kit {
    pub plates: u32,
    pub slung: u32,
    /// kg in the worn pack.
    pub pack_kg: f32,
    pub bottles: u32,
}

/// How far each part of the kit is heard on one step (m), and all of it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rattle {
    pub plates: f32,
    pub swivels: f32,
    pub pack: f32,
    pub slosh: f32,
}

impl Rattle {
    /// The parts summed by power: two equal rattles carry √2 as far.
    pub fn reach(&self) -> f32 {
        (self.plates.powi(2) + self.swivels.powi(2) + self.pack.powi(2) + self.slosh.powi(2)).sqrt()
    }
}

/// The kit's rattle at `speed` (m/s), crouched or not (`height`, as
/// `footstep_reach` takes it). Equal parts add by power, so two plates
/// knock √2 as far as one.
pub fn rattle(kit: &Kit, speed: f32, height: f32) -> Rattle {
    // A footstep's pace, but linear (a footstep's goes as its 0.8 power):
    // a run shakes kit about more.
    let pace = (speed / 1.4).clamp(0.3, 2.6);
    let crouch = if height > 0.0 && height < crate::robots::senses::CROUCHED {
        0.6
    } else {
        1.0
    };
    let k = pace * crouch;
    let n = |count: u32| (count as f32).sqrt();
    let pack = ((kit.pack_kg - PACK_QUIET_KG).max(0.0) * PACK_REACH_PER_KG).min(PACK_REACH_MAX);
    Rattle {
        plates: PLATE_REACH * n(kit.plates) * k,
        swivels: SWIVEL_REACH * n(kit.slung) * k,
        pack: pack * k,
        slosh: SLOSH_REACH * n(kit.bottles) * k,
    }
}

/// A player's kit rattled on a step.
#[derive(Message, Clone, Copy, Debug)]
pub struct GearRattle {
    pub player: Entity,
    pub at: Vec3,
    pub rattle: Rattle,
}

pub struct FoleyPlugin;

impl Plugin for FoleyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<GearRattle>().add_systems(
            FixedUpdate,
            rattle_on_steps.after(crate::player::move_bodies),
        );
    }
}

/// What `player` carries that rattles.
fn kit_of(
    player: Entity,
    carried: &Query<(Entity, &Item, &CarriedBy, Option<&InSocket>)>,
    sockets: &Query<&Socket>,
    packs: &Query<&Backpack>,
    items: &Query<&Item>,
    catalog: &Catalog,
) -> Kit {
    let mut kit = Kit::default();
    for (_, item, by, socket) in carried {
        if by.0 != player {
            continue;
        }
        let def = catalog.get(item.kind());
        if def.id == "steel_plate" {
            kit.plates += 1;
        }
        if def.has_tag("food:Water") {
            kit.bottles += 1;
        }
        let slung = socket
            .and_then(|s| sockets.get(s.socket).ok())
            .is_some_and(|s| s.kind == SocketKind::Body(SlotKind::Sling));
        if slung && def.firearm.is_some() {
            kit.slung += 1;
        }
    }
    for pack in packs.iter().filter(|p| p.carried && p.owner == player) {
        kit.pack_kg += pack
            .contents
            .iter()
            .filter_map(|e| items.get(*e).ok())
            .map(|i| catalog.get(i.kind()).mass)
            .sum::<f32>();
    }
    kit
}

#[allow(clippy::too_many_arguments)]
fn rattle_on_steps(
    mut steps: MessageReader<Footstep>,
    players: Query<&Body, With<Player>>,
    carried: Query<(Entity, &Item, &CarriedBy, Option<&InSocket>)>,
    sockets: Query<&Socket>,
    packs: Query<&Backpack>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut rattles: MessageWriter<GearRattle>,
    mut noise: MessageWriter<Noise>,
) {
    for s in steps.read() {
        let Ok(body) = players.get(s.entity) else {
            continue;
        };
        if body.swimming {
            continue;
        }
        let kit = kit_of(s.entity, &carried, &sockets, &packs, &items, &catalog);
        let r = rattle(&kit, s.speed, body.height);
        let reach = r.reach();
        if reach <= 0.0 {
            continue;
        }
        let at = s.position + Vec3::Y * 1.0;
        rattles.write(GearRattle {
            player: s.entity,
            at,
            rattle: r,
        });
        noise.write(Noise {
            at,
            radius: reach,
            source: Some(s.entity),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing carried, nothing heard; more kit, heard farther; a run
    /// farther than a walk, a crouch less.
    #[test]
    fn louder_kit_carries_farther() {
        let walk = 1.4;
        assert_eq!(rattle(&Kit::default(), walk, 0.0).reach(), 0.0);
        let light = Kit {
            slung: 1,
            pack_kg: 1.0,
            ..default()
        };
        let heavy = Kit {
            plates: 2,
            slung: 2,
            pack_kg: 12.0,
            bottles: 2,
        };
        let (l, h) = (rattle(&light, walk, 0.0), rattle(&heavy, walk, 0.0));
        assert_eq!(l.pack, 0.0, "a light pack is quiet");
        assert!(h.reach() > 2.0 * l.reach(), "{h:?} vs {l:?}");
        assert!(rattle(&heavy, 4.0, 0.0).reach() > h.reach());
        let crouched = crate::robots::senses::CROUCHED * 0.9;
        assert!(rattle(&heavy, walk, crouched).reach() < h.reach());
        // The heaviest kit, running, still carries less than a run on
        // gravel (the footstep itself stays the loudest thing you do).
        let run = rattle(&heavy, 5.0, 0.0).reach();
        let gravel = crate::robots::senses::footstep_reach(1.45, 5.0, 0.0);
        assert!(run < gravel * 1.5, "{run} vs {gravel}");
    }
}
