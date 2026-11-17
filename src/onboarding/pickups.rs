//! What a thing is for, the first time one's picked up out there: the
//! driver says one short line about its family, once a career (in the
//! firsts' queue, `firsts::First::Picked`). Nothing in the world tells a
//! newcomer what gun oil or a lead case is for, and a thing you can't
//! place is what made the houses muddy.
//!
//! A family that's plain to see (food, guns, their magazines) or taught in
//! the yard has no line; it's on [`OBVIOUS`]. Each line's claim is what
//! the item does in the game (where, beside it).

use extraction_core::items::{Category, ItemDef};

/// A family of things that each get one line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Family {
    /// Oil, a cleaning kit, spare parts: touched to a gun
    /// (`weapons::flat`, "gun_oil" cleans, "spare_parts" mends wear).
    GunCare,
    /// A cleaning rod: a bullet stuck in the bore (`weapons::faults`).
    CleaningRod,
    /// A chemlight or a hand flare: used, it lights, and lights its holder
    /// for the machines (`world::glow`).
    Lights,
    /// A signal flare: pulled, a red star goes up sixty metres
    /// (`items::usage`, #94).
    Signal,
    /// Matches: a safehouse cabin's stove (`progression::safehouse`).
    Matches,
    /// Scrap, boards, brass, powder: the truck's upkeep and the bench
    /// (`progression::Upgrade::upkeep`, `fabrication`).
    Salvage,
    /// What comes off a machine: the driver's tier-1 wants
    /// (`progression::WANTS`).
    MachineParts,
    /// A helmet, a vest, a plate: let go where it's worn.
    Armour,
    /// Along a broken shin (`interact::first_aid`).
    Splint,
    /// In the thigh: heals, and slows a bleed (`items::usage`).
    Injector,
    /// Pin out, thrown: a few seconds (`weapons::grenades`, `gadgets`).
    Grenade,
    /// Set down: the first machine near it drops (`gadgets`).
    EmpTrap,
    /// Thrust at a joint or an eye (`weapons::melee`).
    Knife,
    /// A wreck's servo packs come off with it (`interact::tools::spanner`).
    Spanner,
    /// The hacksaw (the vice) and the multitool (a recipe): bench tools.
    Bench,
    /// Jams a door (`robots::doors`).
    Wedge,
    /// Held to the eyes, they magnify (`gadgets::Optic`).
    Binoculars,
    /// An artifact in it can't be felt, nor help you (`anomaly`).
    LeadCase,
    /// A watch, a photo, a tape: an offering place answers one with where
    /// a stone lies (`anomaly::offering`).
    Trinkets,
    /// Rounds for another gun, and the machines' belts.
    Rounds,
    /// A stock, a bolt: off a gun at the bench vice, and back on one of its
    /// kind (`weapons::gunsmith`, tagged `gun_part`).
    GunParts,
}

pub const FAMILIES: &[Family] = &[
    Family::GunCare,
    Family::CleaningRod,
    Family::Lights,
    Family::Signal,
    Family::Matches,
    Family::Salvage,
    Family::MachineParts,
    Family::Armour,
    Family::Splint,
    Family::Injector,
    Family::Grenade,
    Family::EmpTrap,
    Family::Knife,
    Family::Spanner,
    Family::Bench,
    Family::Wedge,
    Family::Binoculars,
    Family::LeadCase,
    Family::Trinkets,
    Family::Rounds,
    Family::GunParts,
];

/// Kinds that need no word: plain to see, or the yard taught them (its
/// stations put each in your hands). With the food and every gun and
/// magazine, by category (`obvious`).
pub const OBVIOUS: &[&str] = &[
    // The yard's.
    "pm_pistol",
    "pm_mag",
    "ammo_9x18",
    "radio",
    "flashlight",
    "power_cell",
    "field_dressing",
    "detector",
    "bolt",
    "map_case",
    "compass",
    // A tape player: press play.
    "cassette_player",
    // A can of diesel: jobs pay in it, nothing yet uses it.
    "fuel_can",
];

/// Needs no line.
pub fn obvious(id: &str, category: Category) -> bool {
    matches!(
        category,
        Category::Food | Category::Firearm | Category::Magazine | Category::Artifact
    ) || OBVIOUS.contains(&id)
}

/// The family a kind belongs to, if it has a line.
pub fn family(def: &ItemDef) -> Option<Family> {
    let (id, category) = (def.id.as_str(), def.category);
    if obvious(id, category) {
        return None;
    }
    if def.has_tag("gun_part") {
        return Some(Family::GunParts);
    }
    Some(match id {
        "gun_oil" | "cleaning_kit" | "spare_parts" => Family::GunCare,
        "cleaning_rod" => Family::CleaningRod,
        "chemlight" | "hand_flare" => Family::Lights,
        "signal_flare" => Family::Signal,
        "matches" => Family::Matches,
        "scrap_metal" | "electronics" | "polymer" | "brass" | "powder" => Family::Salvage,
        "helmet" | "vest" | "steel_plate" => Family::Armour,
        "splint" => Family::Splint,
        "regen_injector" => Family::Injector,
        "rgd5" | "emp_grenade" => Family::Grenade,
        "emp_trap" => Family::EmpTrap,
        "knife" => Family::Knife,
        "spanner" => Family::Spanner,
        "hacksaw" | "multitool" => Family::Bench,
        "door_wedge" => Family::Wedge,
        "binoculars" => Family::Binoculars,
        "lead_case" => Family::LeadCase,
        _ => match category {
            // (What the driver's jobs ask for: a drone's motor, say, isn't.)
            Category::RobotPart if extraction_core::progression::PARTS.contains(&id) => {
                Family::MachineParts
            }
            Category::Curio => Family::Trinkets,
            Category::Ammo => Family::Rounds,
            _ => return None,
        },
    })
}

impl Family {
    /// What the driver says.
    pub fn lines(self) -> &'static [&'static str] {
        match self {
            Family::GunCare => {
                &["Truck: \"Touch it to your gun. Oil and a kit clean it; parts mend wear.\""]
            }
            Family::CleaningRod => {
                &["Truck: \"Round stuck in the bore? Knock it out with that, from the muzzle.\""]
            }
            Family::Lights => &["Truck: \"Use it and it lights. Machines see you by it too.\""],
            Family::Signal => &[
                "Truck: \"Signal flare. Pull the cord, hold it up: the whole valley sees the star.\"",
            ],
            Family::Matches => {
                &["Truck: \"Matches. Keep a box: a cabin's cold stove wants them.\""]
            }
            Family::Salvage => {
                &["Truck: \"Scrap and boards: bring them home. The truck and bench use them.\""]
            }
            Family::MachineParts => {
                &["Truck: \"Parts off a machine. Bring them home: there's jobs want them.\""]
            }
            Family::Armour => {
                &["Truck: \"Let a helmet go at your head, a vest at your chest. Plates go in it.\""]
            }
            Family::Splint => {
                &["Truck: \"Leg broken? Hold the splint along the shin, other hand strapping.\""]
            }
            Family::Injector => {
                &["Truck: \"Tip to your thigh, pull the trigger. It heals, slows a bleed.\""]
            }
            Family::Grenade => {
                &["Truck: \"Pull the pin, then throw it far. You've seconds, not many.\""]
            }
            Family::EmpTrap => {
                &["Truck: \"Set it down where they walk. The first one near it drops.\""]
            }
            Family::Knife => {
                &["Truck: \"Thrust it hard at a joint or the eye. A poke only scratches.\""]
            }
            Family::Spanner => {
                &["Truck: \"A spanner. A wrecked machine's servos come off with it.\""]
            }
            Family::Bench => {
                &["Truck: \"Bench tools. Bring them home: the bench works with them.\""]
            }
            Family::Wedge => &["Truck: \"Jam a door shut with it. They go round, or through.\""],
            Family::Binoculars => {
                &["Truck: \"Hold them to your eyes. Look a place over before you go in.\""]
            }
            Family::LeadCase => {
                &["Truck: \"A stone in that case: the Zone can't feel it. Nor can you.\""]
            }
            Family::Trinkets => {
                &["Truck: \"Leave a trinket at a shrine or well. It tells where a stone lies.\""]
            }
            Family::Rounds => {
                &["Truck: \"Check the box. Rounds fit only the guns made for them.\""]
            }
            Family::GunParts => {
                &["Truck: \"A gun's part. At the bench vice it goes on a gun of its kind.\""]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use extraction_core::items::{BUILTIN_CATALOG, ItemCatalog};

    fn catalog() -> ItemCatalog {
        ItemCatalog::from_ron(BUILTIN_CATALOG).expect("items")
    }

    /// Each kind the loot tables can roll before raid 6, and the first
    /// raid it can.
    fn rollable_before(catalog: &ItemCatalog, raid: u32) -> Vec<ItemDef> {
        let text = std::fs::read_to_string(crate::asset_root_path().join("data/loot.ron"))
            .expect("loot.ron");
        let loot = extraction_core::loot::LootTables::from_ron(&text, catalog).expect("loot");
        let names: Vec<String> = loot.names().cloned().collect();
        let mut out: Vec<ItemDef> = catalog
            .iter()
            .filter(|(_, d)| {
                names
                    .iter()
                    .any(|n| loot.first_raid(n, &d.id).is_some_and(|r| r < raid))
            })
            .map(|(_, d)| d.clone())
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Everything a career's first six raids can turn up in a house has a
    /// line, or needs none.
    #[test]
    fn every_kind_rollable_before_raid_6_has_a_line_or_is_obvious() {
        let catalog = catalog();
        let kinds = rollable_before(&catalog, 6);
        assert!(kinds.len() > 50, "the tables read: {}", kinds.len());
        let mut families = std::collections::BTreeSet::new();
        let mut bad = Vec::new();
        for d in &kinds {
            match family(d) {
                Some(f) => {
                    families.insert(f);
                }
                None if obvious(&d.id, d.category) => {}
                None => bad.push(format!("{} ({:?}): no line, not obvious", d.id, d.category)),
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        // Each family with a line is one the first six raids can show
        // (the knife and the lead case included), bar none.
        for f in FAMILIES {
            assert!(families.contains(f), "{f:?}: nothing before raid 6 is one");
        }
    }

    /// Each kind named here is a real one, and is what its line says: the
    /// game's code that does it names it.
    #[test]
    fn each_line_is_what_the_game_does() {
        let catalog = catalog();
        let ids: Vec<&str> = catalog.iter().map(|(_, d)| d.id.as_str()).collect();
        for id in OBVIOUS {
            assert!(ids.contains(id), "obvious: no kind {id}");
        }
        // (kind, the code that does what its line says, what's in it).
        let does: &[(&str, &str, &str)] = &[
            // "Oil and a kit clean it; parts mend wear": touched to a gun.
            (
                "gun_oil",
                include_str!("../weapons/flat.rs"),
                "gun.firearm.clean(0.3)",
            ),
            (
                "cleaning_kit",
                include_str!("../weapons/flat.rs"),
                "gun.firearm.clean(1.0)",
            ),
            (
                "spare_parts",
                include_str!("../weapons/flat.rs"),
                "gun.firearm.repair(",
            ),
            // "Round stuck in the bore? Knock it out ... from the muzzle."
            (
                "cleaning_rod",
                include_str!("../weapons/faults.rs"),
                "cleaning_rod",
            ),
            // "Use it and it lights. Machines see you by it too."
            (
                "chemlight",
                include_str!("../world/glow.rs"),
                "\"chemlight\" => Some(Self::Chem)",
            ),
            (
                "hand_flare",
                include_str!("../world/glow.rs"),
                "\"hand_flare\" => Some(Self::Flare)",
            ),
            (
                "chemlight",
                include_str!("../world/glow.rs"),
                "robots::senses::Lighting",
            ),
            // "Pull the cord, hold it up: the whole valley sees the star."
            (
                "signal_flare",
                include_str!("../items/usage.rs"),
                "\"signal_flare\" =>",
            ),
            // "A cabin's cold stove wants them."
            (
                "matches",
                include_str!("../progression/safehouse.rs"),
                "== \"matches\"",
            ),
            // "The truck and bench use them."
            (
                "scrap_metal",
                include_str!("../../crates/extraction_core/src/progression.rs"),
                "&[(\"scrap_metal\", 2)]",
            ),
            (
                "electronics",
                include_str!("../../crates/extraction_core/src/progression.rs"),
                "&[(\"electronics\", 1)]",
            ),
            (
                "polymer",
                include_str!("../../crates/extraction_core/src/fabrication.rs"),
                "\"polymer\"",
            ),
            (
                "brass",
                include_str!("../../crates/extraction_core/src/fabrication.rs"),
                "\"brass\"",
            ),
            (
                "powder",
                include_str!("../../crates/extraction_core/src/fabrication.rs"),
                "\"powder\"",
            ),
            // "A helmet at your head, a vest at your chest. Plates go in it."
            (
                "helmet",
                include_str!("../weapons/armour.rs"),
                "the helmet at your head",
            ),
            (
                "vest",
                include_str!("../weapons/armour.rs"),
                "The vest goes on by letting it go at your chest",
            ),
            (
                "steel_plate",
                include_str!("../weapons/armour.rs"),
                "a plate at either of the",
            ),
            // "Hold the splint along the shin."
            (
                "splint",
                include_str!("../interact/first_aid.rs"),
                "hold it\n//!   along the shin",
            ),
            // "Tip to your thigh, pull the trigger. It heals, slows a bleed."
            (
                "regen_injector",
                include_str!("../interact/first_aid.rs"),
                "Put its tip to your thigh and pull\n//!   the trigger",
            ),
            (
                "regen_injector",
                include_str!("../items/usage.rs"),
                "h.bleeding *= 0.3",
            ),
            // "Pull the pin, then throw it far. You've seconds."
            (
                "emp_grenade",
                include_str!("../gadgets/mod.rs"),
                "Armed::Fuse(3.5)",
            ),
            ("rgd5", include_str!("../weapons/grenades.rs"), "rgd5"),
            // "Set it down where they walk. The first one near it drops."
            (
                "emp_trap",
                include_str!("../gadgets/mod.rs"),
                "\"emp_trap\"",
            ),
            // "Thrust it hard at a joint or the eye."
            ("knife", include_str!("../weapons/melee.rs"), "knife"),
            // "A wrecked machine's servos come off with it."
            (
                "spanner",
                include_str!("../interact/tools/spanner.rs"),
                "servo",
            ),
            // "Bench tools": the vice, a recipe.
            (
                "hacksaw",
                include_str!("../weapons/gunsmith.rs"),
                "\"hacksaw\"",
            ),
            (
                "multitool",
                include_str!("../../crates/extraction_core/src/fabrication.rs"),
                "Need::Item(\"multitool\")",
            ),
            // "Jam a door shut with it."
            (
                "door_wedge",
                include_str!("../robots/doors.rs"),
                "door_wedge",
            ),
            // "Hold them to your eyes."
            (
                "binoculars",
                include_str!("../gadgets/mod.rs"),
                "\"binoculars\" => {\n            commands.entity(e).insert(Optic",
            ),
            // "A stone in that case: the Zone can't feel it."
            ("lead_case", include_str!("../anomaly/mod.rs"), "lead_case"),
        ];
        for (id, code, needle) in does {
            assert!(ids.contains(id), "no kind {id}");
            assert!(
                family(catalog.get(catalog.kind(id).unwrap())).is_some(),
                "{id}: no line"
            );
            assert!(
                code.contains(needle),
                "{id}: the code no longer says {needle:?}"
            );
        }
        // Every kind named in `family` is one of those, so each is checked.
        for (_, d) in catalog.iter() {
            if let Some(f) = family(d)
                && !matches!(
                    f,
                    Family::MachineParts | Family::Trinkets | Family::Rounds | Family::GunParts
                )
            {
                assert!(
                    does.iter().any(|(id, ..)| *id == d.id),
                    "{}: its line unchecked",
                    d.id
                );
            }
        }
        // The rest, by category: "jobs want them", "a shrine or well tells
        // where a stone lies", "rounds fit only their guns".
        let offering = include_str!("../anomaly/offering.rs");
        let gunsmith = include_str!("../weapons/gunsmith.rs");
        assert!(gunsmith.contains("comes off no gun"));
        assert!(offering.contains("has(Category::Curio)"));
        for (_, d) in catalog.iter() {
            match family(d) {
                Some(Family::MachineParts) => assert!(
                    extraction_core::progression::fetch_takes(
                        extraction_core::progression::ANY_PARTS,
                        &d.id
                    ),
                    "{}: no job wants it",
                    d.id
                ),
                Some(Family::Trinkets) => assert_eq!(d.category, Category::Curio),
                Some(Family::Rounds) => assert_eq!(d.category, Category::Ammo, "{}", d.id),
                // Each comes off a gun and goes back on one
                // (`gunsmith::tests`, "comes off no gun").
                Some(Family::GunParts) => assert!(d.has_tag("gun_part")),
                _ => {}
            }
        }
    }
}
