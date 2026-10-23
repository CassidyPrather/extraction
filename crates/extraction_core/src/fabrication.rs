//! The truck's bench: the economy's main sink.
//!
//! Recipes break robot parts and spare ammunition down into materials,
//! re-pack materials into rounds for real guns, and spend spare parts on
//! repairs. Every recipe loses something on the way — nothing comes back
//! out at the weight it went in — so the bench is where loot leaves the
//! world. Recipes are matched against what's lying in the bench tray.

use crate::items::{Category, ItemCatalog, ItemKind};

/// What a recipe needs, per slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Need {
    /// This exact item.
    Item(&'static str),
    /// Any ammunition box (not loose rounds, not robot belts).
    AmmoBox,
    /// Any firearm below full condition.
    WornGun,
    /// Any item with this tag.
    Tagged(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Make {
    /// Spawn these items (id, how many); ammo boxes come full.
    Items(&'static [(&'static str, u32)]),
    /// Restore a gun's condition by this much (up to 1).
    Repair(f32),
    /// Re-pack the old (surplus) and damp rounds in an ammunition box with
    /// fresh primers and powder: they come out good ball
    /// (`Load::Standard`); the box stays.
    Restore,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: &'static str,
    pub name: &'static str,
    pub needs: &'static [(Need, u32)],
    pub makes: Make,
    /// Seconds the bench runs.
    pub seconds: f32,
}

pub const RECIPES: &[Recipe] = &[
    // --- Breaking robots down -------------------------------------------
    Recipe {
        id: "strip_rotor",
        name: "Strip rotor assembly",
        needs: &[(Need::Item("rotor_assembly"), 1)],
        makes: Make::Items(&[("electronics", 1)]),
        seconds: 4.0,
    },
    Recipe {
        id: "strip_servo",
        name: "Strip servo",
        needs: &[(Need::Item("servo"), 1)],
        makes: Make::Items(&[("scrap_metal", 1)]),
        seconds: 4.0,
    },
    Recipe {
        id: "strip_eye",
        name: "Strip sensor eye",
        needs: &[(Need::Item("sensor_eye"), 1)],
        makes: Make::Items(&[("electronics", 1)]),
        seconds: 4.0,
    },
    Recipe {
        id: "strip_gunpod",
        name: "Break down robot gun pod",
        needs: &[(Need::Item("robot_gunpod"), 1)],
        makes: Make::Items(&[("scrap_metal", 3), ("electronics", 1), ("powder", 1)]),
        seconds: 10.0,
    },
    Recipe {
        id: "pull_belt",
        name: "Pull robot ammunition belt",
        needs: &[(Need::Item("robot_ammo_belt"), 1)],
        makes: Make::Items(&[("powder", 1), ("brass", 1)]),
        seconds: 6.0,
    },
    Recipe {
        id: "pull_box",
        name: "Pull two boxes of rounds",
        needs: &[(Need::AmmoBox, 2)],
        makes: Make::Items(&[("powder", 1)]),
        seconds: 5.0,
    },
    Recipe {
        id: "drain_cell",
        name: "Decant robot cell into power cells",
        needs: &[(Need::Item("robot_cell"), 1)],
        makes: Make::Items(&[("power_cell", 2)]),
        seconds: 6.0,
    },
    // --- Breaking spares down ------------------------------------------
    // (A second multitool or torch is clutter; its insides aren't.)
    Recipe {
        id: "strip_multitool",
        name: "Break two multitools for parts",
        needs: &[(Need::Item("multitool"), 2)],
        makes: Make::Items(&[("spare_parts", 1)]),
        seconds: 6.0,
    },
    Recipe {
        id: "strip_flashlight",
        name: "Gut two flashlights",
        needs: &[(Need::Item("flashlight"), 2)],
        makes: Make::Items(&[("electronics", 1)]),
        seconds: 6.0,
    },
    // --- Re-packing rounds -----------------------------------------------
    Recipe {
        id: "pack_9x18",
        name: "Re-pack 9x18 (box of 16)",
        needs: &[
            (Need::Item("powder"), 1),
            (Need::Item("brass"), 1),
            (Need::Item("scrap_metal"), 1),
        ],
        makes: Make::Items(&[("ammo_9x18_repack", 1)]),
        seconds: 8.0,
    },
    Recipe {
        id: "pack_545",
        name: "Re-pack 5.45x39 (box of 30)",
        needs: &[
            (Need::Item("powder"), 2),
            (Need::Item("brass"), 2),
            (Need::Item("scrap_metal"), 1),
        ],
        makes: Make::Items(&[("ammo_545_repack", 1)]),
        seconds: 10.0,
    },
    Recipe {
        id: "pack_762r",
        name: "Re-pack 7.62x54R (box of 10)",
        needs: &[
            (Need::Item("powder"), 2),
            (Need::Item("brass"), 1),
            (Need::Item("scrap_metal"), 1),
        ],
        makes: Make::Items(&[("ammo_762r_repack", 1)]),
        seconds: 10.0,
    },
    Recipe {
        id: "pack_12ga",
        name: "Re-pack 12 gauge (box of 10)",
        needs: &[
            (Need::Item("powder"), 1),
            (Need::Item("brass"), 1),
            (Need::Item("scrap_metal"), 1),
            (Need::Item("polymer"), 1),
        ],
        makes: Make::Items(&[("ammo_12ga_repack", 1)]),
        seconds: 10.0,
    },
    Recipe {
        id: "restore_rounds",
        name: "Re-pack a box's old or damp rounds",
        needs: &[(Need::AmmoBox, 1), (Need::Item("powder"), 1)],
        makes: Make::Restore,
        seconds: 8.0,
    },
    // --- Making things -----------------------------------------------------
    Recipe {
        id: "make_emp",
        name: "Wire an EMP grenade",
        needs: &[
            (Need::Item("electronics"), 2),
            (Need::Item("robot_cell"), 1),
        ],
        makes: Make::Items(&[("emp_grenade", 1)]),
        seconds: 12.0,
    },
    Recipe {
        id: "make_parts",
        name: "File firearm spare parts",
        needs: &[(Need::Item("scrap_metal"), 2)],
        makes: Make::Items(&[("spare_parts", 1)]),
        seconds: 12.0,
    },
    Recipe {
        id: "make_bolts",
        name: "File crossbow bolts (bundle of five)",
        needs: &[(Need::Item("scrap_metal"), 1), (Need::Item("polymer"), 1)],
        makes: Make::Items(&[("bolt_bundle", 1)]),
        seconds: 12.0,
    },
    // --- Repair ---------------------------------------------------------------
    Recipe {
        id: "repair_gun",
        name: "Repair a worn gun",
        needs: &[(Need::WornGun, 1), (Need::Item("spare_parts"), 1)],
        makes: Make::Repair(0.35),
        seconds: 15.0,
    },
];

/// What the bench can do once the driver has fitted a vice to it
/// (`progression::Upgrade::BenchVice`): bench tier 1.
pub const VICE_RECIPES: &[Recipe] = &[
    Recipe {
        id: "make_side_rail",
        name: "Cut and file a side rail",
        needs: &[(Need::Item("scrap_metal"), 1)],
        makes: Make::Items(&[("side_rail", 1)]),
        seconds: 14.0,
    },
    Recipe {
        id: "make_door_wedge",
        name: "Bend a door wedge",
        needs: &[(Need::Item("scrap_metal"), 1)],
        makes: Make::Items(&[("door_wedge", 1)]),
        seconds: 8.0,
    },
    Recipe {
        id: "make_weapon_light",
        name: "Build a weapon light",
        needs: &[
            (Need::Item("electronics"), 1),
            (Need::Item("power_cell"), 1),
            (Need::Item("polymer"), 1),
        ],
        makes: Make::Items(&[("weapon_light", 1)]),
        seconds: 14.0,
    },
    Recipe {
        id: "make_emp_trap",
        name: "Rig an EMP trap",
        needs: &[
            (Need::Item("electronics"), 2),
            (Need::Item("robot_cell"), 1),
            (Need::Item("servo"), 1),
        ],
        makes: Make::Items(&[("emp_trap", 1)]),
        seconds: 16.0,
    },
    Recipe {
        id: "pack_762x39",
        name: "Re-pack 7.62x39 (box of 20)",
        needs: &[
            (Need::Item("powder"), 2),
            (Need::Item("brass"), 2),
            (Need::Item("scrap_metal"), 1),
        ],
        makes: Make::Items(&[("ammo_762x39_repack", 1)]),
        seconds: 10.0,
    },
    Recipe {
        id: "pack_762x25",
        name: "Re-pack 7.62x25 (box of 70)",
        needs: &[
            (Need::Item("powder"), 2),
            (Need::Item("brass"), 2),
            (Need::Item("scrap_metal"), 1),
        ],
        makes: Make::Items(&[("ammo_762x25_repack", 1)]),
        seconds: 12.0,
    },
    Recipe {
        id: "make_crossbow",
        name: "Build a crossbow from a leaf spring",
        needs: &[
            // The leaf spring and its stirrup are most of its weight.
            (Need::Item("scrap_metal"), 7),
            (Need::Item("spare_parts"), 1),
            (Need::Item("polymer"), 1),
        ],
        makes: Make::Items(&[("crossbow", 1)]),
        seconds: 30.0,
    },
    Recipe {
        id: "pack_flare",
        name: "Pack two signal cartridges (white)",
        needs: &[
            (Need::Item("powder"), 2),
            (Need::Item("brass"), 1),
            (Need::Item("polymer"), 1),
        ],
        makes: Make::Items(&[("flare_white", 2)]),
        seconds: 12.0,
    },
    Recipe {
        id: "can_suppressor",
        name: "Pack a tin into a suppressor",
        needs: &[(Need::Tagged("can"), 1), (Need::Item("scrap_metal"), 1)],
        makes: Make::Items(&[("can_suppressor", 1)]),
        seconds: 20.0,
    },
    Recipe {
        id: "repair_gun_fine",
        name: "Strip and rebuild a worn gun",
        needs: &[(Need::WornGun, 1), (Need::Item("spare_parts"), 2)],
        makes: Make::Repair(0.7),
        seconds: 25.0,
    },
];

/// What the driver's contracts teach the bench (`contracts::Unlock`):
/// armour-piercing re-packs, each for its contract, and the relay jammer.
pub const CONTRACT_RECIPES: &[Recipe] = &[
    Recipe {
        id: "make_jammer",
        name: "Build a relay jammer",
        // Taught by "The mast relay" (`progression::contracts`). A
        // salvaged radio's guts, a cell to drive it, scrap for the
        // aerials and the box (`robots::network::Jammer`).
        needs: &[
            (Need::Item("radio"), 1),
            (Need::Item("power_cell"), 1),
            (Need::Item("scrap_metal"), 2),
        ],
        makes: Make::Items(&[("jammer", 1)]),
        seconds: 20.0,
    },
    Recipe {
        id: "pack_545_ap",
        name: "Re-pack 5.45x39 AP (box of 30)",
        needs: &[
            (Need::Item("powder"), 2),
            (Need::Item("brass"), 2),
            (Need::Item("scrap_metal"), 2),
        ],
        makes: Make::Items(&[("ammo_545_ap", 1)]),
        seconds: 14.0,
    },
    Recipe {
        id: "pack_762x39_ap",
        name: "Re-pack 7.62x39 AP (box of 20)",
        needs: &[
            (Need::Item("powder"), 2),
            (Need::Item("brass"), 2),
            (Need::Item("scrap_metal"), 2),
        ],
        makes: Make::Items(&[("ammo_762x39_ap", 1)]),
        seconds: 14.0,
    },
    Recipe {
        id: "pack_762r_ap",
        name: "Re-pack 7.62x54R AP (box of 10)",
        needs: &[
            (Need::Item("powder"), 2),
            (Need::Item("brass"), 2),
            (Need::Item("scrap_metal"), 2),
        ],
        makes: Make::Items(&[("ammo_762r_ap", 1)]),
        seconds: 14.0,
    },
];

/// The recipes a bench of `tier` knows, with those the contracts `taught`.
pub fn recipes_taught<'a>(
    tier: u8,
    taught: &'a [&'a str],
) -> impl Iterator<Item = &'static Recipe> + 'a {
    recipes(tier).chain(
        CONTRACT_RECIPES
            .iter()
            .filter(move |r| taught.contains(&r.id)),
    )
}

/// The recipes a bench of `tier` knows, in recipe-book order.
pub fn recipes(tier: u8) -> impl Iterator<Item = &'static Recipe> {
    RECIPES
        .iter()
        .chain(VICE_RECIPES.iter().filter(move |_| tier >= 1))
}

/// One thing in the tray, as the bench sees it.
#[derive(Clone, Copy, Debug)]
pub struct TrayItem {
    pub kind: ItemKind,
    pub condition: f32,
}

fn fits(need: Need, item: &TrayItem, catalog: &ItemCatalog) -> bool {
    let def = catalog.get(item.kind);
    match need {
        Need::Item(id) => def.id == id,
        Need::AmmoBox => {
            def.category == Category::Ammo && def.rounds > 1 && def.id.starts_with("ammo_")
        }
        Need::WornGun => def.firearm.is_some() && item.condition < 0.99,
        Need::Tagged(tag) => def.has_tag(tag),
    }
}

/// Which tray items (by index) a recipe would use, or `None` if the tray
/// can't make it. Specific needs are filled before general ones.
pub fn plan(recipe: &Recipe, tray: &[TrayItem], catalog: &ItemCatalog) -> Option<Vec<usize>> {
    let mut used = vec![false; tray.len()];
    let mut picks = Vec::new();
    let mut needs: Vec<(Need, u32)> = recipe.needs.to_vec();
    needs.sort_by_key(|(n, _)| match n {
        Need::Item(_) => 0,
        _ => 1,
    });
    for (need, count) in needs {
        for _ in 0..count {
            let i = (0..tray.len()).find(|&i| !used[i] && fits(need, &tray[i], catalog))?;
            used[i] = true;
            picks.push(i);
        }
    }
    Some(picks)
}

/// Every recipe the tray can make right now on a bench as it came.
pub fn available<'a>(tray: &[TrayItem], catalog: &ItemCatalog) -> Vec<&'a Recipe> {
    available_at(tray, catalog, 0)
}

/// Every recipe the tray can make right now on a bench of `tier`.
pub fn available_at<'a>(tray: &[TrayItem], catalog: &ItemCatalog, tier: u8) -> Vec<&'a Recipe> {
    recipes(tier)
        .filter(|r| plan(r, tray, catalog).is_some())
        .collect()
}

/// [`available_at`], with the recipes the contracts `taught` the bench.
pub fn available_taught<'a>(
    tray: &[TrayItem],
    catalog: &ItemCatalog,
    tier: u8,
    taught: &[&str],
) -> Vec<&'a Recipe> {
    recipes_taught(tier, taught)
        .filter(|r| plan(r, tray, catalog).is_some())
        .collect()
}

pub fn recipe(id: &str) -> Option<&'static Recipe> {
    every_recipe().find(|r| r.id == id)
}

/// Every recipe there is: every bench tier's and the contracts'.
pub fn every_recipe() -> impl Iterator<Item = &'static Recipe> {
    recipes(u8::MAX).chain(CONTRACT_RECIPES.iter())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::BUILTIN_CATALOG;

    fn cat() -> ItemCatalog {
        ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap()
    }

    fn tray(cat: &ItemCatalog, ids: &[&str]) -> Vec<TrayItem> {
        ids.iter()
            .map(|id| TrayItem {
                kind: cat.kind(id).unwrap_or_else(|| panic!("{id}")),
                condition: 0.5,
            })
            .collect()
    }

    /// Any tin of food and a handful of scrap make a can suppressor, in the
    /// vice.
    #[test]
    fn a_tin_and_scrap_make_a_suppressor() {
        let cat = cat();
        for tin in ["canned_beans", "canned_stew", "canned_tomato"] {
            let t = tray(&cat, &[tin, "scrap_metal"]);
            assert!(
                available_at(&t, &cat, 1)
                    .iter()
                    .any(|r| r.id == "can_suppressor"),
                "{tin}"
            );
        }
        let t = tray(&cat, &["tea_tin", "scrap_metal"]);
        assert!(
            !available_at(&t, &cat, 1)
                .iter()
                .any(|r| r.id == "can_suppressor")
        );
    }

    #[test]
    fn every_recipe_names_real_items() {
        let cat = cat();
        for r in every_recipe() {
            for (n, _) in r.needs {
                if let Need::Item(id) = n {
                    assert!(cat.kind(id).is_some(), "{}: {id}", r.id);
                }
            }
            if let Make::Items(out) = r.makes {
                for (id, _) in out {
                    assert!(cat.kind(id).is_some(), "{}: {id}", r.id);
                }
            }
        }
    }

    #[test]
    fn nothing_comes_out_heavier_than_it_went_in() {
        let cat = cat();
        for r in every_recipe() {
            let Make::Items(out) = r.makes else {
                continue;
            };
            let mass_in: f32 = r
                .needs
                .iter()
                .map(|(n, c)| {
                    *c as f32
                        * match n {
                            Need::Item(id) => cat.get(cat.kind(id).unwrap()).mass,
                            // The lightest ammo box.
                            Need::AmmoBox => 0.17,
                            Need::WornGun => 0.0,
                            Need::Tagged(tag) => cat
                                .iter()
                                .filter(|(_, d)| d.has_tag(tag))
                                .map(|(_, d)| d.mass)
                                .fold(f32::INFINITY, f32::min),
                        }
                })
                .sum();
            let mass_out: f32 = out
                .iter()
                .map(|(id, c)| *c as f32 * cat.get(cat.kind(id).unwrap()).mass)
                .sum();
            assert!(
                mass_out < mass_in,
                "{} makes {mass_out:.2} kg from {mass_in:.2} kg",
                r.id
            );
        }
    }

    #[test]
    fn plans_use_distinct_items() {
        let cat = cat();
        let t = tray(&cat, &["powder", "brass", "scrap_metal", "powder", "brass"]);
        let r = recipe("pack_545").unwrap();
        let p = plan(r, &t, &cat).unwrap();
        assert_eq!(p.len(), 5);
        let mut s = p.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), 5);
        assert!(plan(r, &t[..4], &cat).is_none());
    }

    #[test]
    fn repair_wants_a_worn_gun_and_parts() {
        let cat = cat();
        let mut t = tray(&cat, &["ak74", "spare_parts"]);
        assert!(available(&t, &cat).iter().any(|r| r.id == "repair_gun"));
        t[0].condition = 1.0;
        assert!(!available(&t, &cat).iter().any(|r| r.id == "repair_gun"));
    }

    #[test]
    fn the_vice_teaches_the_bench_more() {
        let cat = cat();
        let t = tray(&cat, &["scrap_metal"]);
        let ids =
            |tier| -> Vec<&str> { available_at(&t, &cat, tier).iter().map(|r| r.id).collect() };
        assert!(!ids(0).contains(&"make_side_rail"));
        let carbine = tray(&cat, &["powder", "powder", "brass", "brass", "scrap_metal"]);
        assert!(
            available_at(&carbine, &cat, 1)
                .iter()
                .any(|r| r.id == "pack_762x39"),
            "the vice packs the carbine's rounds"
        );
        assert!(
            available_at(&carbine, &cat, 1)
                .iter()
                .any(|r| r.id == "pack_762x25"),
            "and the sub-machine gun's"
        );
        assert!(ids(1).contains(&"make_side_rail"));
        assert!(recipe("make_emp_trap").is_some());
        let mut seen: Vec<&str> = recipes(1).map(|r| r.id).collect();
        let n = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), n, "recipe ids are unique");
    }
}
