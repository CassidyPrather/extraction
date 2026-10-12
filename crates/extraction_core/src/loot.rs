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
    },
    /// Roll once on another table.
    Table { name: String, weight: f32 },
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

    /// Rolls a table. Unknown tables roll nothing.
    pub fn roll(&self, name: &str, catalog: &ItemCatalog, rng: &mut Rng) -> Vec<Drop> {
        let mut out = Vec::new();
        self.roll_into(name, catalog, rng, &mut out, 0);
        out
    }

    fn roll_into(
        &self,
        name: &str,
        catalog: &ItemCatalog,
        rng: &mut Rng,
        out: &mut Vec<Drop>,
        depth: u32,
    ) {
        let Some(t) = self.tables.get(name) else {
            return;
        };
        if depth > 8 {
            return;
        }
        let rolls = t.rolls.0 + rng.below((t.rolls.1 - t.rolls.0 + 1) as usize) as u32;
        let weights: Vec<f32> = t
            .entries
            .iter()
            .map(|e| match e {
                Entry::Item { weight, .. }
                | Entry::Table { weight, .. }
                | Entry::Nothing { weight } => *weight,
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
                Entry::Table { name, .. } => self.roll_into(name, catalog, rng, out, depth + 1),
                Entry::Nothing { .. } => {}
            }
        }
    }
}

pub const BUILTIN_LOOT: &str = include_str!("../../../assets/data/loot.ron");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::BUILTIN_CATALOG;

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
}
