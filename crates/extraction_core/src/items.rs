//! What things are: the item catalog.
//!
//! Items are data. The catalog lives in `assets/data/items.ron` and is parsed
//! into an [`ItemCatalog`]; everything else refers to items by [`ItemKind`],
//! a dense index into it, so per-kind lookups are array reads.
//!
//! Every item in the game is a physical object with mass and bulk. There are
//! no stacks and no prices: what an item is *worth* is what it does, what it
//! weighs, how much room it takes, and how much attention it draws.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Dense handle to an [`ItemDef`] in an [`ItemCatalog`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemKind(pub u16);

/// Where an attachment or insert goes: the kind of socket that takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mount {
    /// A receiver's side dovetail: scopes, the side-mount rail.
    Dovetail,
    /// A short accessory rail: lights, lasers.
    Rail,
    /// Over the muzzle: the artifact adapter.
    Muzzle,
    /// A battery compartment: power cells.
    Cell,
    /// The artifact adapter's cradle: artifacts.
    Cradle,
    /// Worn on the torso (a plate carrier).
    Vest,
    /// Worn on the head.
    Helmet,
    /// A plate carrier's pocket: armour plates.
    Plate,
    /// A cassette player's well: cassettes.
    Cassette,
    /// A gramophone's platter: records.
    Record,
}

/// How big a thing is, for pockets and slings. Bulk (a continuous volume) is
/// what the backpack counts; size class is what a fixed slot accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SizeClass {
    /// Rounds, matches, a lens.
    Tiny,
    /// A magazine, a battery, a can of food.
    Small,
    /// A pistol, a flashlight, a cleaning kit.
    Medium,
    /// A helmet, a robot gun-pod. Backpack only.
    Large,
    /// A rifle or a shotgun. Slings only; never the backpack.
    Bulky,
}

/// The broad family an item belongs to. Behaviour hangs off the more
/// specific blocks in [`ItemDef`] (`firearm`, `magazine`, ...); this is for
/// sorting, loot tables and UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Firearm,
    Magazine,
    Ammo,
    Attachment,
    Gadget,
    Medical,
    Food,
    Tool,
    Material,
    RobotPart,
    Artifact,
    Curio,
}

/// One kind of item, as authored in `items.ron`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    /// Stable string id used by data files and saves (`"pm_pistol"`).
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub category: Category,
    pub size: SizeClass,
    /// Kilograms.
    pub mass: f32,
    /// Litres of backpack capacity it takes up.
    pub bulk: f32,
    /// Attention emitted while carried (artifacts, mostly). Per second.
    #[serde(default)]
    pub attention: f32,
    /// Kilograms it takes *off* your carried load (the featherstone).
    #[serde(default)]
    pub lift_kg: f32,
    /// The socket kind it fits, if it's an attachment or insert.
    #[serde(default)]
    pub mount: Option<Mount>,
    /// Cartridge this item *is*, if it is ammunition (a single round, or a
    /// box of them; see `rounds`).
    #[serde(default)]
    pub cartridge: Option<String>,
    /// Rounds in a fresh box of ammunition.
    #[serde(default)]
    pub rounds: u32,
    /// Present if this item is a detachable magazine or stripper clip.
    #[serde(default)]
    pub magazine: Option<MagazineDef>,
    /// Present if this item is a firearm.
    #[serde(default)]
    pub firearm: Option<crate::firearm::FirearmDef>,
    /// Present if this item is a hand grenade.
    #[serde(default)]
    pub grenade: Option<crate::grenade::GrenadeDef>,
    /// Present if this item is a blade.
    #[serde(default)]
    pub melee: Option<crate::melee::MeleeDef>,
    /// Ammunition: what its rounds are loaded with.
    #[serde(default)]
    pub load: crate::ammo::Load,
    /// Ammunition packed with every this many rounds a tracer (0: none).
    #[serde(default)]
    pub tracer_every: u32,
    /// Present if this item is worn armour.
    #[serde(default)]
    pub armour: Option<crate::firearm_terminal::ArmourDef>,
    /// Charge capacity in watt-hours, for batteries and things that hold them.
    #[serde(default)]
    pub battery_wh: f32,
    /// Free-form tags for loot tables and recipes (`"electronics"`, `"metal"`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Physical box size in metres: x width, y height, z length (z runs
    /// along a gun's barrel). Derived from bulk when absent.
    #[serde(default)]
    pub dims: Option<[f32; 3]>,
}

impl ItemDef {
    /// The physical box size, from `dims` or a cube of the item's bulk.
    pub fn size_m(&self) -> [f32; 3] {
        self.dims.unwrap_or_else(|| {
            let side = (self.bulk.max(0.01) / 1000.0).cbrt();
            [side, side, side]
        })
    }

    /// The mount it fits (artifacts all fit a cradle).
    pub fn fits_mount(&self) -> Option<Mount> {
        self.mount
            .or((self.category == Category::Artifact).then_some(Mount::Cradle))
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

/// A detachable box magazine (or a stripper clip, which is a magazine you
/// can only empty into a gun).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MagazineDef {
    /// Which family of guns it fits (`"pm"`, `"ak545"`).
    pub fits: String,
    pub cartridge: String,
    pub capacity: u32,
    /// Stripper clips feed through a gun's top, then fall away.
    #[serde(default)]
    pub stripper: bool,
}

/// The whole catalog. Built once at load; immutable afterwards.
#[derive(Clone, Debug, Default)]
pub struct ItemCatalog {
    defs: Vec<ItemDef>,
    by_id: HashMap<String, ItemKind>,
}

/// The on-disk shape of `items.ron`.
#[derive(Debug, Deserialize)]
struct CatalogFile {
    items: Vec<ItemDef>,
}

/// Why a catalog failed to load.
#[derive(Debug)]
pub enum CatalogError {
    Parse(String),
    DuplicateId(String),
    Invalid { id: String, reason: String },
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "items.ron does not parse: {e}"),
            Self::DuplicateId(id) => write!(f, "item id `{id}` is defined twice"),
            Self::Invalid { id, reason } => write!(f, "item `{id}`: {reason}"),
        }
    }
}

impl std::error::Error for CatalogError {}

impl ItemCatalog {
    /// Parses and validates a catalog from RON text.
    pub fn from_ron(text: &str) -> Result<Self, CatalogError> {
        let file: CatalogFile =
            ron::from_str(text).map_err(|e| CatalogError::Parse(e.to_string()))?;
        Self::from_defs(file.items)
    }

    pub fn from_defs(defs: Vec<ItemDef>) -> Result<Self, CatalogError> {
        let mut by_id = HashMap::with_capacity(defs.len());
        for (i, def) in defs.iter().enumerate() {
            if by_id.insert(def.id.clone(), ItemKind(i as u16)).is_some() {
                return Err(CatalogError::DuplicateId(def.id.clone()));
            }
            validate(def)?;
        }
        Ok(Self { defs, by_id })
    }

    pub fn get(&self, kind: ItemKind) -> &ItemDef {
        &self.defs[kind.0 as usize]
    }

    pub fn kind(&self, id: &str) -> Option<ItemKind> {
        self.by_id.get(id).copied()
    }

    /// Like [`Self::kind`], for ids that are known to exist (tests, built-in
    /// content). Panics with the id if it is missing.
    pub fn expect(&self, id: &str) -> ItemKind {
        self.kind(id)
            .unwrap_or_else(|| panic!("no item with id `{id}` in the catalog"))
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (ItemKind, &ItemDef)> {
        self.defs
            .iter()
            .enumerate()
            .map(|(i, d)| (ItemKind(i as u16), d))
    }
}

fn validate(def: &ItemDef) -> Result<(), CatalogError> {
    let bad = |reason: &str| {
        Err(CatalogError::Invalid {
            id: def.id.clone(),
            reason: reason.to_owned(),
        })
    };
    if !(def.mass > 0.0) {
        return bad("mass must be positive");
    }
    if def.bulk < 0.0 {
        return bad("bulk must not be negative");
    }
    if def.category == Category::Firearm && def.firearm.is_none() {
        return bad("a Firearm needs a `firearm` block");
    }
    if def.category == Category::Magazine && def.magazine.is_none() {
        return bad("a Magazine needs a `magazine` block");
    }
    if def.category == Category::Ammo && def.cartridge.is_none() {
        return bad("Ammo needs a `cartridge`");
    }
    Ok(())
}

/// Per-instance mutable state. Every physical item in the world carries one.
/// What fields mean depends on the kind; unused ones stay at their defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemState {
    pub kind: ItemKind,
    /// 0..=1. Guns and tools wear; at 0 they are broken.
    pub condition: f32,
    /// Rounds in a box of ammo, or loaded in a magazine.
    pub rounds: u32,
    /// Stored energy in watt-hours, for batteries.
    pub charge_wh: f32,
    /// What the rounds are loaded with, top last (see
    /// [`crate::ammo::Loads`]); a box's are all its own load.
    pub loads: crate::ammo::Loads,
}

impl ItemState {
    /// A fresh item as it would be found: full boxes, full charge, condition
    /// as given.
    pub fn fresh(catalog: &ItemCatalog, kind: ItemKind, condition: f32) -> Self {
        let def = catalog.get(kind);
        Self {
            kind,
            condition: condition.clamp(0.0, 1.0),
            rounds: def.rounds,
            charge_wh: def.battery_wh,
            loads: crate::ammo::Loads::with_tracers(def.rounds, def.tracer_every),
        }
    }
}

/// The built-in catalog shipped with the game.
pub const BUILTIN_CATALOG: &str = include_str!("../../../assets/data/items.ron");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_catalog_loads() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).expect("catalog should load");
        assert!(cat.len() > 10);
        let pm = cat.expect("pm_pistol");
        assert_eq!(cat.get(pm).category, Category::Firearm);
    }

    #[test]
    fn every_magazine_fits_some_gun_and_shares_its_cartridge() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        for (_, def) in cat.iter() {
            let Some(mag) = &def.magazine else { continue };
            if mag.stripper {
                // Clips name the gun they feed by id.
                let gun = cat.get(cat.expect(&mag.fits)).firearm.as_ref().unwrap();
                assert_eq!(
                    gun.cartridge, mag.cartridge,
                    "{} cartridge mismatch",
                    def.id
                );
                continue;
            }
            let gun = cat
                .iter()
                .filter_map(|(_, d)| d.firearm.as_ref())
                .find(|f| f.magazine_family.as_deref() == Some(mag.fits.as_str()));
            let gun = gun.unwrap_or_else(|| panic!("{} fits no gun", def.id));
            assert_eq!(
                gun.cartridge, mag.cartridge,
                "{} cartridge mismatch",
                def.id
            );
        }
    }

    #[test]
    fn every_firearm_has_ammo_in_the_world() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        for (_, def) in cat.iter() {
            let Some(gun) = &def.firearm else { continue };
            assert!(
                cat.iter().any(|(_, d)| d.category == Category::Ammo
                    && d.cartridge.as_deref() == Some(gun.cartridge.as_str())),
                "nothing feeds {}",
                def.id
            );
        }
    }

    #[test]
    fn bulky_things_are_long_guns() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        for (_, def) in cat.iter() {
            if def.size == SizeClass::Bulky {
                assert!(matches!(
                    def.category,
                    Category::Firearm | Category::RobotPart
                ));
            }
        }
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let text = r#"(items: [
            (id: "a", name: "A", category: Curio, size: Tiny, mass: 0.1, bulk: 0.1),
            (id: "a", name: "A again", category: Curio, size: Tiny, mass: 0.1, bulk: 0.1),
        ])"#;
        assert!(matches!(
            ItemCatalog::from_ron(text),
            Err(CatalogError::DuplicateId(_))
        ));
    }
}
