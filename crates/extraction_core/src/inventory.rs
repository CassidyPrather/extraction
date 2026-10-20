//! What a player carries, and what it costs them to carry it.
//!
//! Three kinds of place to put things, after Into the Radius:
//!
//! - The **backpack**: a volume you reach into. It takes anything that is
//!   not [`SizeClass::Bulky`] until the summed bulk hits its capacity. No
//!   tetris, no collision.
//! - **Slots**: holster, pouches, pockets, slings. Each takes one item of a
//!   permitted size class.
//! - **Hands**: whatever you are holding.
//!
//! The constraints that matter are mass ([`Encumbrance`]) and bulk; the two
//! long-gun slings are the hard cap on rifles.

use crate::items::{ItemCatalog, ItemKind, SizeClass};
use serde::{Deserialize, Serialize};

/// Kinds of fixed slot on the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlotKind {
    /// Over-the-shoulder sling for one long gun.
    Sling,
    /// Hip holster: pistols only (medium).
    Holster,
    /// Chest-rig magazine pouch: small items.
    Pouch,
    /// Belt or trouser pocket: tiny items.
    Pocket,
    /// Wrist or chest gadget mount: small items.
    Mount,
    /// Worn on the torso: a plate carrier (the vest), nothing else.
    Vest,
    /// Worn on the head: a helmet, nothing else.
    Head,
}

impl SlotKind {
    /// Whether an item of `size` fits this slot.
    pub fn accepts(self, size: SizeClass) -> bool {
        match self {
            Self::Sling => size == SizeClass::Bulky,
            Self::Holster => size == SizeClass::Medium,
            Self::Pouch | Self::Mount => size <= SizeClass::Small,
            Self::Pocket => size == SizeClass::Tiny,
            // Only what's made to be worn there (the game checks the mount).
            Self::Vest | Self::Head => false,
        }
    }
}

/// The default chest-rig-and-belt layout every thief starts with.
pub const DEFAULT_SLOTS: &[SlotKind] = &[
    SlotKind::Sling,
    SlotKind::Sling,
    SlotKind::Holster,
    SlotKind::Pouch,
    SlotKind::Pouch,
    SlotKind::Pouch,
    SlotKind::Pouch,
    SlotKind::Pocket,
    SlotKind::Pocket,
    SlotKind::Mount,
    SlotKind::Vest,
    SlotKind::Head,
];

/// Why something can't be stowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StowError {
    /// Bulky things never go in the backpack.
    TooBulkyForBackpack,
    /// The backpack would exceed its capacity.
    BackpackFull,
    /// The slot doesn't take things that size.
    WrongSize,
    /// The slot already holds something.
    Occupied,
}

/// Backpack capacity model.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Backpack {
    /// Litres.
    pub capacity: f32,
    /// Mass of the empty bag, kg.
    pub mass: f32,
}

impl Default for Backpack {
    fn default() -> Self {
        Self {
            capacity: 40.0,
            mass: 1.4,
        }
    }
}

impl Backpack {
    /// Checks whether an item of `size`/`bulk` can join items already
    /// totalling `used` litres.
    pub fn check(&self, used: f32, size: SizeClass, bulk: f32) -> Result<(), StowError> {
        if size == SizeClass::Bulky {
            return Err(StowError::TooBulkyForBackpack);
        }
        if used + bulk > self.capacity + 1e-4 {
            return Err(StowError::BackpackFull);
        }
        Ok(())
    }

    /// How awkward it is to rummage, 0..=1: crowding slows finding things.
    /// The game uses this to slow hand motion and add jostle inside the bag.
    pub fn crowding(&self, used: f32) -> f32 {
        (used / self.capacity.max(1e-3)).clamp(0.0, 1.0).powf(1.5)
    }
}

/// A body slot and what's in it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slot<T> {
    pub kind: SlotKind,
    pub item: Option<T>,
}

/// The whole loadout, generic over what an "item" is to the caller (an
/// entity in the game, an [`ItemKind`] in tests and sims).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Loadout<T> {
    pub backpack: Backpack,
    pub backpack_items: Vec<T>,
    pub slots: Vec<Slot<T>>,
}

impl<T> Default for Loadout<T> {
    fn default() -> Self {
        Self {
            backpack: Backpack::default(),
            backpack_items: Vec::new(),
            slots: DEFAULT_SLOTS
                .iter()
                .map(|&kind| Slot { kind, item: None })
                .collect(),
        }
    }
}

impl<T: Clone> Loadout<T> {
    /// Everything carried, backpack and slots.
    pub fn all(&self) -> impl Iterator<Item = &T> {
        self.backpack_items
            .iter()
            .chain(self.slots.iter().filter_map(|s| s.item.as_ref()))
    }

    /// Stows into the backpack, given the item's size and bulk and a way to
    /// get the bulk of things already inside.
    pub fn stow_backpack(
        &mut self,
        item: T,
        size: SizeClass,
        bulk: f32,
        bulk_of: impl Fn(&T) -> f32,
    ) -> Result<(), (T, StowError)> {
        let used: f32 = self.backpack_items.iter().map(&bulk_of).sum();
        match self.backpack.check(used, size, bulk) {
            Ok(()) => {
                self.backpack_items.push(item);
                Ok(())
            }
            Err(e) => Err((item, e)),
        }
    }

    /// Puts `item` in slot `index`.
    pub fn stow_slot(
        &mut self,
        index: usize,
        item: T,
        size: SizeClass,
    ) -> Result<(), (T, StowError)> {
        let Some(slot) = self.slots.get_mut(index) else {
            return Err((item, StowError::WrongSize));
        };
        if slot.item.is_some() {
            return Err((item, StowError::Occupied));
        }
        if !slot.kind.accepts(size) {
            return Err((item, StowError::WrongSize));
        }
        slot.item = Some(item);
        Ok(())
    }

    /// First free slot that would take `size`, preferring the most specific.
    pub fn free_slot_for(&self, size: SizeClass) -> Option<usize> {
        let order = [
            SlotKind::Pocket,
            SlotKind::Pouch,
            SlotKind::Mount,
            SlotKind::Holster,
            SlotKind::Sling,
        ];
        order.iter().find_map(|&kind| {
            self.slots
                .iter()
                .position(|s| s.kind == kind && s.item.is_none() && kind.accepts(size))
        })
    }
}

impl Loadout<ItemKind> {
    /// Carried mass in kg, with the backpack's own weight.
    pub fn mass(&self, catalog: &ItemCatalog) -> f32 {
        self.backpack.mass + self.all().map(|k| catalog.get(*k).mass).sum::<f32>()
    }

    pub fn backpack_used(&self, catalog: &ItemCatalog) -> f32 {
        self.backpack_items
            .iter()
            .map(|k| catalog.get(*k).bulk)
            .sum()
    }

    pub fn attention(&self, catalog: &ItemCatalog) -> f32 {
        self.all().map(|k| catalog.get(*k).attention).sum()
    }
}

/// What carried mass does to a body. Thresholds are in kg on top of the
/// body; tuned so a sensible scavenging load (~15 kg) is free, a greedy one
/// (~30 kg) is felt, and a ridiculous one (40+) is a crawl.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Encumbrance {
    /// Multiplier on walk speed.
    pub speed: f32,
    /// Multiplier on stamina regeneration.
    pub stamina_regen: f32,
    /// Multiplier on stamina drain while sprinting.
    pub stamina_drain: f32,
    /// Multiplier on how far your footsteps carry.
    pub noise: f32,
    /// Whether sprinting is possible at all.
    pub can_sprint: bool,
}

/// Kg before any penalty.
pub const FREE_CARRY: f32 = 18.0;
/// Kg at which sprinting stops.
pub const NO_SPRINT: f32 = 35.0;
/// Kg at which you can barely move.
pub const MAX_CARRY: f32 = 50.0;

impl Encumbrance {
    pub fn from_mass(kg: f32) -> Self {
        let over = ((kg - FREE_CARRY) / (MAX_CARRY - FREE_CARRY)).clamp(0.0, 1.0);
        Self {
            speed: 1.0 - 0.75 * over * over.sqrt(),
            stamina_regen: 1.0 - 0.8 * over,
            stamina_drain: 1.0 + 2.0 * over,
            noise: 1.0 + 1.5 * over,
            can_sprint: kg < NO_SPRINT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::BUILTIN_CATALOG;

    fn catalog() -> ItemCatalog {
        ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap()
    }

    #[test]
    fn rifles_never_go_in_the_backpack() {
        let cat = catalog();
        let ak = cat.expect("ak74");
        let mut lo = Loadout::<ItemKind>::default();
        let d = cat.get(ak);
        let err = lo
            .stow_backpack(ak, d.size, d.bulk, |k| cat.get(*k).bulk)
            .unwrap_err();
        assert_eq!(err.1, StowError::TooBulkyForBackpack);
    }

    #[test]
    fn two_slings_cap_long_guns_at_two() {
        let cat = catalog();
        let mut lo = Loadout::<ItemKind>::default();
        for id in ["ak74", "mosin", "toz_double"] {
            let k = cat.expect(id);
            match lo.free_slot_for(cat.get(k).size) {
                Some(i) => lo.stow_slot(i, k, cat.get(k).size).unwrap(),
                None => {
                    assert_eq!(id, "toz_double", "third long gun has nowhere to go");
                }
            }
        }
    }

    #[test]
    fn backpack_fills_up() {
        let cat = catalog();
        let can = cat.expect("canned_stew");
        let mut lo = Loadout::<ItemKind>::default();
        let d = cat.get(can);
        let mut stored = 0;
        while lo
            .stow_backpack(can, d.size, d.bulk, |k| cat.get(*k).bulk)
            .is_ok()
        {
            stored += 1;
            assert!(stored < 10_000);
        }
        assert!(lo.backpack_used(&cat) <= lo.backpack.capacity + 1e-3);
        assert!(lo.backpack.crowding(lo.backpack_used(&cat)) > 0.9);
    }

    #[test]
    fn pockets_take_tiny_things_only() {
        assert!(SlotKind::Pocket.accepts(SizeClass::Tiny));
        assert!(!SlotKind::Pocket.accepts(SizeClass::Small));
        assert!(SlotKind::Pouch.accepts(SizeClass::Small));
        assert!(!SlotKind::Holster.accepts(SizeClass::Bulky));
    }

    #[test]
    fn encumbrance_curve() {
        let light = Encumbrance::from_mass(10.0);
        assert_eq!(light.speed, 1.0);
        assert!(light.can_sprint);
        let heavy = Encumbrance::from_mass(40.0);
        assert!(heavy.speed < 0.8);
        assert!(!heavy.can_sprint);
        let max = Encumbrance::from_mass(100.0);
        assert!(max.speed > 0.2, "never fully stuck");
    }
}
