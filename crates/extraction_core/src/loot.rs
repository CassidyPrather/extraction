//! Loot tables: what a container, a shelf or a robot drops.
//!
//! Places are designed (map markers, building furniture); *what* turns up
//! there is rolled from a named table. Tables can reference other tables,
//! so "kitchen" can include "food_common" plus the odd "curio".

use crate::items::{ItemCatalog, ItemKind};
use crate::noise::Rng;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
pub enum Entry {
    /// A specific item, with a condition range.
    Item {
        id: String,
        weight: f32,
        #[serde(default = "full")]
        condition: (f32, f32),
        /// Rolls only once a career has set off on this many raids (the
        /// first raid is raid 0): families come a few at a time (the
        /// designer, in a first house: "bombarded with too many item types
        /// at once"). 0, the default: always.
        #[serde(default)]
        from_raid: u32,
    },
    /// Roll once on another table.
    Table {
        name: String,
        weight: f32,
        #[serde(default)]
        from_raid: u32,
    },
    /// Nothing (keeps tables from always paying out).
    Nothing { weight: f32 },
}

const fn full() -> (f32, f32) {
    (0.6, 1.0)
}

#[derive(Clone, Debug, Deserialize)]
pub struct Table {
    /// How many rolls: inclusive range.
    pub rolls: (u32, u32),
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Deserialize)]
struct TablesFile {
    tables: HashMap<String, Table>,
}

#[derive(Clone, Debug, Default)]
pub struct LootTables {
    tables: HashMap<String, Table>,
}

/// One rolled item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drop {
    pub kind: ItemKind,
    pub condition: f32,
}

impl LootTables {
    pub fn from_ron(text: &str, catalog: &ItemCatalog) -> Result<Self, String> {
        let file: TablesFile = ron::from_str(text).map_err(|e| e.to_string())?;
        // Validate references up front so typos fail loudly at load.
        for (name, t) in &file.tables {
            for e in &t.entries {
                match e {
                    Entry::Item { id, .. } if catalog.kind(id).is_none() => {
                        return Err(format!("table `{name}`: unknown item `{id}`"));
                    }
                    Entry::Table { name: sub, .. } if !file.tables.contains_key(sub) => {
                        return Err(format!("table `{name}`: unknown table `{sub}`"));
                    }
                    _ => {}
                }
            }
        }
        Ok(Self {
            tables: file.tables,
        })
    }

    pub fn has(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.tables.keys()
    }

    /// Whether any table can drop item `id` (for checks that everything
    /// is found somewhere).
    pub fn mentions(&self, id: &str) -> bool {
        self.tables.values().any(|t| {
            t.entries
                .iter()
                .any(|e| matches!(e, Entry::Item { id: i, .. } if i == id))
        })
    }

    /// Rolls a table. Unknown tables roll nothing.
    pub fn roll(&self, name: &str, catalog: &ItemCatalog, rng: &mut Rng) -> Vec<Drop> {
        self.roll_in(name, None, catalog, rng)
    }

    /// Rolls a table in a valley (its map's name, as `valley_key` gives
    /// it): wherever a table `<name>@<valley>` exists, it stands in for
    /// `<name>`, nested tables too. A valley's table usually rolls the
    /// common one and adds what's peculiar to the place.
    pub fn roll_in(
        &self,
        name: &str,
        valley: Option<&str>,
        catalog: &ItemCatalog,
        rng: &mut Rng,
    ) -> Vec<Drop> {
        self.roll_at(name, valley, None, catalog, rng)
    }

    /// As [`roll_in`](Self::roll_in), on a career's raid `raid` (`None`:
    /// every entry, as for a sim): entries `from_raid` later stay out.
    pub fn roll_at(
        &self,
        name: &str,
        valley: Option<&str>,
        raid: Option<u32>,
        catalog: &ItemCatalog,
        rng: &mut Rng,
    ) -> Vec<Drop> {
        let mut out = Vec::new();
        self.roll_into(name, valley, raid, catalog, rng, &mut out, 0);
        out
    }

    /// The first raid on which `root` (rolled in no valley) can drop item
    /// `id`, if it ever can: for checks that what a job wants turns up in
    /// time.
    pub fn first_raid(&self, root: &str, id: &str) -> Option<u32> {
        self.first_raid_in(root, id, 0)
    }

    fn first_raid_in(&self, name: &str, id: &str, depth: u32) -> Option<u32> {
        let t = self.tables.get(name)?;
        if depth > 8 {
            return None;
        }
        t.entries
            .iter()
            .filter_map(|e| match e {
                Entry::Item {
                    id: i,
                    weight,
                    from_raid,
                    ..
                } if i == id && *weight > 0.0 => Some(*from_raid),
                Entry::Table {
                    name: inner,
                    weight,
                    from_raid,
                } if *weight > 0.0 => self
                    .first_raid_in(inner, id, depth + 1)
                    .map(|r| r.max(*from_raid)),
                _ => None,
            })
            .min()
    }

    #[allow(clippy::too_many_arguments)]
    fn roll_into(
        &self,
        name: &str,
        valley: Option<&str>,
        raid: Option<u32>,
        catalog: &ItemCatalog,
        rng: &mut Rng,
        out: &mut Vec<Drop>,
        depth: u32,
    ) {
        let local = valley
            .map(|v| format!("{name}@{v}"))
            .and_then(|k| self.tables.get(&k));
        let Some(t) = local.or_else(|| self.tables.get(name)) else {
            return;
        };
        if depth > 8 {
            return;
        }
        let rolls = t.rolls.0 + rng.below((t.rolls.1 - t.rolls.0 + 1) as usize) as u32;
        let weights: Vec<f32> = t
            .entries
            .iter()
            .map(|e| {
                let (weight, from) = match e {
                    Entry::Item {
                        weight, from_raid, ..
                    }
                    | Entry::Table {
                        weight, from_raid, ..
                    } => (*weight, *from_raid),
                    Entry::Nothing { weight } => (*weight, 0),
                };
                // Not yet, on this raid of a career.
                if raid.is_some_and(|r| from > r) {
                    0.0
                } else {
                    weight
                }
            })
            .collect();
        for _ in 0..rolls {
            let Some(i) = rng.weighted(&weights) else {
                continue;
            };
            match &t.entries[i] {
                Entry::Item { id, condition, .. } => {
                    if let Some(kind) = catalog.kind(id) {
                        out.push(Drop {
                            kind,
                            condition: rng.range(condition.0, condition.1),
                        });
                    }
                }
                Entry::Table { name: inner, .. } => {
                    // A valley's table naming the common one rolls it
                    // plain (not itself again).
                    let v = valley.filter(|_| !(local.is_some() && inner == name));
                    self.roll_into(inner, v, raid, catalog, rng, out, depth + 1)
                }
                Entry::Nothing { .. } => {}
            }
        }
    }
}

/// A valley's key in table names: its map's name, lower case, words
/// joined by `_` ("Kettle Pass" is `kettle_pass`).
pub fn valley_key(map_name: &str) -> String {
    map_name
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("_")
}

pub const BUILTIN_LOOT: &str = include_str!("../../../assets/data/loot.ron");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::BUILTIN_CATALOG;

    /// What a house shows on a career's raid `raid`: the kinds a few
    /// hundred rolls turn up.
    fn house_kinds(raid: Option<u32>) -> std::collections::BTreeSet<String> {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        let loot = LootTables::from_ron(BUILTIN_LOOT, &cat).unwrap();
        let mut rng = Rng::new(11);
        (0..600)
            .flat_map(|_| loot.roll_at("house", Some("marrow_creek"), raid, &cat, &mut rng))
            .map(|d| cat.get(d.kind).id.clone())
            .collect()
    }

    /// A first raid's houses draw from a small starter set, and the
    /// families arrive raid by raid (the designer, in Marrow's first
    /// houses: "bombarded with too many item types at once"; 31 kinds in
    /// the Village's six then).
    #[test]
    fn a_first_raids_houses_draw_from_a_starter_set() {
        let first = house_kinds(Some(0));
        assert!(first.len() <= 14, "{} kinds on raid 0: {first:?}", first.len());
        for id in ["canned_stew", "field_dressing", "ammo_9x18", "power_cell", "bolt"] {
            assert!(first.contains(id), "no {id} on raid 0");
        }
        for id in ["ak74", "ammo_545", "pu_scope", "tape_duty", "emp_grenade"] {
            assert!(!first.contains(id), "{id} on raid 0");
        }
        let mut last = first.len();
        for raid in [2, 4, 6] {
            let n = house_kinds(Some(raid)).len();
            assert!(n > last, "raid {raid}: {n} kinds, not more than {last}");
            last = n;
        }
        // A sim, or a career far on, has every family.
        assert_eq!(house_kinds(None).len(), house_kinds(Some(99)).len());
    }

    /// What the driver's first jobs ask for can be found by then
    /// (`progression::WANTS`): the odd jobs' from the first raids, the
    /// errands' radio and electronics by the third.
    #[test]
    fn what_the_first_jobs_want_turns_up_in_time() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        let loot = LootTables::from_ron(BUILTIN_LOOT, &cat).unwrap();
        let soonest = |id: &str| {
            ["house", "workshop", "kitchen"]
                .iter()
                .filter_map(|t| loot.first_raid(t, id))
                .min()
        };
        for (id, ..) in crate::progression::WANTS[0] {
            assert!(soonest(id).is_some_and(|r| r <= 1), "{id}: {:?}", soonest(id));
        }
        for id in ["radio", "electronics"] {
            assert!(soonest(id).is_some_and(|r| r <= 3), "{id}: {:?}", soonest(id));
        }
    }

    #[test]
    fn builtin_tables_load_and_roll() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        let loot = LootTables::from_ron(BUILTIN_LOOT, &cat).unwrap();
        let mut rng = Rng::new(3);
        let mut total = 0;
        for name in loot.names().cloned().collect::<Vec<_>>() {
            for _ in 0..50 {
                total += loot.roll(&name, &cat, &mut rng).len();
            }
        }
        assert!(total > 0);
    }

    #[test]
    fn unknown_items_are_rejected() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        let bad = r#"(tables: {"x": (rolls: (1, 1), entries: [Item(id: "nope", weight: 1.0)])})"#;
        assert!(LootTables::from_ron(bad, &cat).is_err());
    }

    /// A valley's table stands in for the common one there, nested too,
    /// and rolls the common one inside it plainly; elsewhere the common
    /// one rolls.
    #[test]
    fn a_valley_has_its_own_tables() {
        let catalog = ItemCatalog::from_ron(crate::items::BUILTIN_CATALOG).unwrap();
        let text = r#"(tables: {
            "guns": (rolls: (1, 1), entries: [Item(id: "pm_pistol", weight: 1.0)]),
            "guns@kettle_pass": (rolls: (2, 2), entries: [
                Table(name: "guns", weight: 1.0),
                Item(id: "mosin", weight: 1.0),
            ]),
            "house": (rolls: (1, 1), entries: [Table(name: "guns", weight: 1.0)]),
        })"#;
        let t = LootTables::from_ron(text, &catalog).unwrap();
        let id = |d: &Drop| catalog.get(d.kind).id.clone();
        let mut rng = Rng::new(3);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..40 {
            let drops = t.roll_in("house", Some("kettle_pass"), &catalog, &mut rng);
            assert_eq!(drops.len(), 2, "the valley's table rolled");
            seen.extend(drops.iter().map(id));
            let plain = t.roll_in("house", Some("marrow_creek"), &catalog, &mut rng);
            assert_eq!(plain.iter().map(id).collect::<Vec<_>>(), vec!["pm_pistol"]);
        }
        assert!(
            seen.contains("mosin") && seen.contains("pm_pistol"),
            "{seen:?}"
        );
        assert_eq!(valley_key("Kettle Pass"), "kettle_pass");
    }
}
