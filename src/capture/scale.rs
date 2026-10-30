//! Props at their real size (M4 in `docs/playtests/2026-10-28/TRANSCRIPT.md`).
//! The playtest found "the world's smallest carrots", and a matchbox,
//! grenade and field dressing too small to read in the hand: "they can be
//! a little bit bigger … it would be fine" (0:25:25, 0:51:54).
//!
//! [`REAL`] lists every item with its real-world size (its largest side, in
//! metres) and how its model may differ from it ([`Rule`]). The test holds
//! every model's largest side (its drawn bounds, `_lod` stand-ins left
//! out) to its rule, and every item in the catalog to having a row.
//!
//! Sources: makers' and army handbook dimensions where there are any (the
//! guns, their magazines and cartridges, grenades, the PSO-1, the IPP-1
//! dressing); otherwise a typical example measured by hand, noted.

/// How far a model may be from its real size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rule {
    /// It fits something exactly (a cartridge in a chamber, a magazine in
    /// a well, a cassette in a deck): 0.9 to 1.12 times.
    Fit,
    /// Held and looked at. In a headset, small things read smaller than
    /// they are: under [`HAND_SMALL`] they're drawn 1.1 to 1.6 times real,
    /// over it 0.9 to 1.4 times.
    Hand,
    /// Made up (the Zone's machines, artifacts): the size is a guess at
    /// what it would be, 0.6 to 1.7 times.
    Made,
}

/// A held thing smaller than this is drawn a little bigger (m).
pub const HAND_SMALL: f32 = 0.15;

impl Rule {
    pub fn range(self, real: f32) -> (f32, f32) {
        match self {
            Rule::Fit => (0.9, 1.12),
            Rule::Hand if real < HAND_SMALL => (1.1, 1.6),
            Rule::Hand => (0.9, 1.4),
            Rule::Made => (0.6, 1.7),
        }
    }
}

use Rule::*;

/// Every item: (id, real largest side in metres, rule, what it is).
pub const REAL: &[(&str, f32, Rule, &str)] = &[
    // Guns: overall length, stock out (the Mosin with its bayonet, as drawn).
    ("pm_pistol", 0.161, Fit, "PM, 161 mm"),
    ("ak74", 0.943, Fit, "AK-74, 943 mm"),
    ("mosin", 1.66, Fit, "M91/30 with bayonet, 1660 mm"),
    ("sks", 1.02, Fit, "SKS, 1020 mm"),
    ("nagant", 0.235, Fit, "M1895, 235 mm"),
    ("ppsh", 0.843, Fit, "PPSh-41, 843 mm"),
    ("toz_double", 1.14, Fit, "TOZ-34, about 1140 mm"),
    ("mp133", 1.13, Fit, "MP-133, 1130 mm"),
    ("svd", 1.225, Fit, "SVD, 1225 mm"),
    ("toz8", 1.11, Fit, "TOZ-8M, 1110 mm"),
    ("rpk74", 1.06, Fit, "RPK-74, 1060 mm (bipod down)"),
    ("flare_pistol", 0.22, Fit, "SPSh-44, 220 mm"),
    ("crossbow", 0.9, Made, "a home-made crossbow"),
    // Magazines: height along the curve or diameter.
    ("pm_mag", 0.1, Fit, "PM magazine"),
    ("ak_mag", 0.2, Fit, "AK-74 magazine"),
    (
        "mosin_clip",
        0.077,
        Fit,
        "a stripper clip of 7.62x54R: its rounds' length",
    ),
    ("svd_mag", 0.13, Fit, "SVD magazine"),
    ("rpk_mag", 0.27, Fit, "RPK-74 45-round magazine"),
    ("ppsh_drum", 0.19, Fit, "PPSh 71-round drum"),
    ("sks_clip", 0.08, Fit, "7.62x39 stripper clip with rounds"),
    // Attachments.
    ("weapon_light", 0.09, Fit, "a weapon light"),
    ("laser_module", 0.07, Fit, "a laser module"),
    ("pu_scope", 0.27, Fit, "PU 3.5x, 270 mm"),
    ("pso_scope", 0.375, Fit, "PSO-1 with shade, 375 mm"),
    ("pbs_suppressor", 0.2, Fit, "PBS-1, 200 mm"),
    ("can_suppressor", 0.13, Made, "a home-made can"),
    ("side_rail", 0.1, Fit, "a dovetail side mount"),
    ("artifact_adapter", 0.09, Made, "an artifact cradle"),
    // Grenades and their bits.
    ("rgd5", 0.114, Hand, "RGD-5 with fuse, 114 mm"),
    ("f1", 0.117, Hand, "F-1 with fuse, 117 mm"),
    ("grenade_pin", 0.045, Hand, "a grenade's ring and pin"),
    ("grenade_spoon", 0.075, Hand, "a UZRGM fuse's lever"),
    ("emp_grenade", 0.11, Made, "the Zone's EMP grenade"),
    ("emp_trap", 0.08, Made, "the Zone's EMP trap"),
    // Kit.
    ("knife", 0.26, Hand, "a field knife"),
    ("vest", 0.46, Hand, "a plate carrier"),
    ("steel_plate", 0.3, Hand, "a 250 x 300 mm plate"),
    ("helmet", 0.29, Hand, "SSh-68"),
    ("flashlight", 0.19, Hand, "a two-cell torch"),
    ("power_cell", 0.06, Fit, "a D-size cell, 61 mm"),
    (
        "compass",
        0.065,
        Fit,
        "Adrianov compass (its face and needle are drawn to it)",
    ),
    ("map_case", 0.33, Hand, "a map case"),
    ("radio", 0.21, Hand, "a handheld radio"),
    ("cassette_player", 0.155, Hand, "a pocket cassette recorder"),
    ("binoculars", 0.17, Hand, "BPTs 7x50"),
    ("detector", 0.16, Made, "an anomaly detector"),
    ("detector_sweep", 0.2, Made, "a sweep detector"),
    ("detector_bear", 0.16, Made, "the Bear detector"),
    ("canary", 0.24, Hand, "a canary cage"),
    ("lead_case", 0.12, Made, "a lead-lined case"),
    ("camera_drone", 0.19, Made, "a camera drone"),
    ("cloak_generator", 0.3, Made, "a cloak generator"),
    ("jammer", 0.24, Made, "a relay jammer"),
    ("deployable_cover", 0.6, Made, "a folding shield"),
    ("climbing_kit", 0.3, Hand, "a coiled rope and grapnel"),
    ("zipline_kit", 0.3, Hand, "a coiled line and trolley"),
    ("matches", 0.05, Hand, "a Soviet matchbox, 50 mm"),
    ("door_wedge", 0.13, Hand, "a wooden door wedge"),
    ("chemlight", 0.15, Hand, "a 6-inch lightstick"),
    ("hand_flare", 0.23, Hand, "a hand flare"),
    ("relay_box", 0.16, Made, "a relay box"),
    ("camera", 0.14, Hand, "Zorki-4, 140 mm"),
    // Medical.
    (
        "field_dressing",
        0.11,
        Hand,
        "IPP-1 individual dressing, 110 mm",
    ),
    ("splint", 0.45, Hand, "a ladder splint"),
    ("regen_injector", 0.13, Made, "an autoinjector"),
    // Food.
    ("water_bottle", 0.24, Hand, "an army canteen"),
    ("condensed_milk", 0.078, Hand, "a 400 g tin, 78 mm across"),
    ("egg_powder", 0.08, Hand, "a packet of egg powder"),
    ("carrot", 0.18, Hand, "a bunch of carrots"),
    ("onion", 0.075, Hand, "an onion"),
    ("canned_tomato", 0.11, Hand, "a 500 g tin"),
    ("canned_beans", 0.11, Hand, "a 500 g tin"),
    ("canned_stew", 0.1, Hand, "tushonka, 100 mm across"),
    ("flour_sack", 0.25, Hand, "a 2 kg bag of flour"),
    ("rice_bag", 0.2, Hand, "a 1 kg bag of rice"),
    ("sugar_cubes", 0.1, Hand, "a box of sugar cubes"),
    ("tea_tin", 0.1, Hand, "a tea tin"),
    ("spice_jar", 0.08, Hand, "a spice jar"),
    ("salt_packet", 0.1, Hand, "a 1 kg salt packet"),
    ("crackers", 0.12, Hand, "a pack of army galety"),
    // Tools.
    ("cleaning_rod", 0.6, Fit, "a cleaning rod"),
    ("cleaning_kit", 0.25, Hand, "a cleaning kit roll"),
    ("gun_oil", 0.08, Hand, "an oil can"),
    ("spare_parts", 0.1, Hand, "a tin of spare parts"),
    ("multitool", 0.1, Hand, "a folded multitool"),
    ("crowbar", 0.76, Hand, "a 750 mm crowbar"),
    ("bolt_cutters", 0.61, Hand, "24-inch bolt cutters"),
    ("hacksaw", 0.45, Hand, "a hacksaw"),
    ("spanner", 0.3, Hand, "a spanner"),
    ("fuel_can", 0.36, Hand, "a 10 L jerrycan"),
    ("scour_key", 0.62, Hand, "a valve key"),
    // Gun parts: the gun's own.
    ("stock_mosin", 0.265, Fit, "the Mosin's butt"),
    ("stock_sks", 0.242, Fit, "the SKS's butt"),
    ("stock_toz_double", 0.306, Fit, "the TOZ's butt"),
    ("stock_mp133", 0.358, Fit, "the MP-133's butt"),
    ("stock_ak74", 0.29, Fit, "the AK's butt"),
    ("stock_rpk74", 0.29, Fit, "the RPK's butt"),
    (
        "bolt_mosin",
        0.14,
        Fit,
        "the Mosin model's own bolt, handle out",
    ),
    // Materials and salvage.
    ("scrap_metal", 0.07, Made, "a handful of scrap"),
    ("electronics", 0.065, Made, "a salvaged board"),
    ("polymer", 0.07, Made, "a block of polymer stock"),
    ("powder", 0.06, Hand, "a powder tin"),
    ("brass", 0.05, Made, "a bag of cases"),
    ("bolt", 0.07, Hand, "an M10 bolt"),
    ("bus_panel", 0.9, Hand, "a bus's skirt panel"),
    ("car_door", 1.0, Hand, "a Lada's front door"),
    ("car_wheel", 0.6, Hand, "a Lada's wheel"),
    ("car_bonnet", 1.3, Hand, "a Lada's bonnet"),
    // The machines' parts and the Zone's artifacts: made up.
    ("rotor_assembly", 0.1, Made, "a drone rotor hub"),
    ("servo", 0.08, Made, "a servo"),
    ("sensor_eye", 0.06, Made, "a sensor eye"),
    ("robot_cell", 0.085, Made, "a robot cell"),
    ("robot_gunpod", 1.08, Made, "a robot's gun pod"),
    ("robot_plate", 0.3, Made, "a robot's plate"),
    ("drone_rotor_motor", 0.25, Made, "a drone's rotor motor"),
    ("walker_leg_actuator", 0.39, Made, "a walker's knee"),
    ("crawler_capacitor", 0.28, Made, "a crawler's capacitors"),
    ("dirigible_gas_valve", 0.37, Made, "an airship's valve"),
    ("pylon_relay_coil", 0.27, Made, "a pylon's coil"),
    ("sorter_servo_pack", 0.86, Made, "a sorter's servo pack"),
    ("heavy_cell", 0.44, Made, "a heavy's cell"),
    ("heavy_plate", 1.08, Made, "a heavy's plate"),
    ("heavy_knee", 0.48, Made, "a heavy's knee"),
    ("heavy_claw", 0.56, Made, "a heavy's claw"),
    ("pressure_mine", 0.31, Made, "a mine"),
    ("robot_ammo_belt", 0.08, Made, "a coil of robot belt"),
    ("art_featherstone", 0.036, Made, "an artifact"),
    ("art_hearth", 0.054, Made, "an artifact"),
    ("art_glasseye", 0.033, Made, "an artifact"),
    ("art_aegis", 0.047, Made, "an artifact"),
    ("art_lodestone", 0.06, Made, "an artifact"),
    ("art_hushstone", 0.05, Made, "an artifact"),
    ("art_stormglass", 0.04, Made, "an artifact"),
    ("art_shard", 0.11, Made, "an artifact"),
    ("art_rimestone", 0.045, Made, "an artifact"),
    // Curios.
    ("tape_practice", 0.1004, Fit, "a compact cassette, 100.4 mm"),
    ("tape_hymns", 0.1004, Fit, "a compact cassette"),
    ("tape_lodge", 0.1004, Fit, "a compact cassette"),
    ("tape_duty", 0.1004, Fit, "a compact cassette"),
    ("tape_birthday", 0.1004, Fit, "a compact cassette"),
    ("tape_bells", 0.1004, Fit, "a compact cassette"),
    ("record_augustin", 0.254, Fit, "a 10-inch record"),
    ("record_entertainer", 0.254, Fit, "a 10-inch record"),
    ("record_susanna", 0.254, Fit, "a 10-inch record"),
    ("work_boots", 0.32, Hand, "a pair of work boots"),
    ("key_marrow", 0.14, Hand, "a key on its tag"),
    ("key_kettle", 0.14, Hand, "a key on its tag"),
    ("key_cinder", 0.14, Hand, "a key on its tag"),
    ("wristwatch", 0.04, Hand, "a wristwatch's case"),
    ("old_photo", 0.13, Hand, "a 9 x 13 cm print"),
    ("chess_piece", 0.06, Hand, "a wooden knight"),
    ("intake_ledger", 0.32, Hand, "a ledger"),
    ("hana_letter", 0.16, Hand, "a folded letter"),
    ("holm_watch", 0.04, Hand, "a pocket watch"),
    ("hajek_tin", 0.09, Hand, "a tobacco tin"),
    ("pump_log", 0.21, Hand, "a logbook"),
    ("deputy_lamp", 0.25, Hand, "a miner's flame safety lamp"),
    ("charging_manifest", 0.3, Hand, "a manifest on a board"),
    ("field_recorder", 0.17, Hand, "a field recorder"),
];

/// Ammunition: a round by its cartridge's overall length, a box or packet
/// by its pack. Rounds fit chambers and magazines; packs are held.
pub fn ammo(id: &str) -> Option<(f32, Rule)> {
    let round = |id: &str| -> Option<f32> {
        Some(match id {
            i if i.contains("9x18") => 0.025,
            i if i.contains("545") => 0.057,
            i if i.contains("762x39") => 0.056,
            i if i.contains("762x38r") => 0.039,
            i if i.contains("762x25") => 0.0345,
            i if i.contains("762r") => 0.077,
            i if i.contains("12ga") => 0.07,
            i if i.contains("22lr") => 0.025,
            _ => return None,
        })
    };
    if let Some(rest) = id
        .strip_prefix("round_")
        .or_else(|| id.strip_prefix("shell_"))
        .or_else(|| id.strip_prefix("slug_"))
    {
        let _ = rest;
        return round(id).map(|r| (r, Fit));
    }
    if id.starts_with("flare_") {
        return Some((0.078, Fit));
    }
    if id == "crossbow_bolt" || id == "bolt_bundle" {
        return Some((0.38, Fit));
    }
    if id.starts_with("ammo_") {
        // Paper packets and card boxes of 8 to 30.
        let pack = match id {
            i if i.contains("9x18") => 0.07,
            i if i.contains("545") => 0.1,
            i if i.contains("762x39") => 0.06,
            i if i.contains("762x38r") => 0.07,
            i if i.contains("762x25") => 0.1,
            i if i.contains("762r") => 0.09,
            i if i.contains("12ga") => 0.12,
            i if i.contains("22lr") => 0.076,
            _ => return None,
        };
        // As issued: a packet's size is its count of rounds. Re-packed
        // ones are whatever box came to hand.
        let rule = if id.contains("repack") { Made } else { Fit };
        return Some((pack, rule));
    }
    None
}

/// An item's row: real size and rule.
pub fn real(id: &str) -> Option<(f32, Rule)> {
    REAL.iter()
        .find(|r| r.0 == id)
        .map(|r| (r.1, r.2))
        .or_else(|| ammo(id))
}

/// Cooking's dishes, set aside with cooking (the designer, 0:13:36): they
/// come only from the cooking table, which the truck and yard no longer
/// carry. Not held to a size until cooking comes back.
pub fn set_aside(id: &str) -> bool {
    id.starts_with("cook_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::glb;

    /// The drawn model's largest side, `_lod` stand-ins left out.
    fn drawn(path: &std::path::Path) -> Option<f32> {
        let info = glb::read(path).ok()?;
        let (lo, hi) = info.bounds_of(|n| !n.name.contains("_lod"))?;
        Some((hi - lo).max_element())
    }

    /// Prints every item's real and drawn size, and holds each to its
    /// rule; every item has a row.
    #[test]
    fn items_are_drawn_near_their_real_size() {
        let catalog = crate::items::Catalog::load();
        let mut bad = Vec::new();
        let mut rows = 0;
        for (_, def) in catalog.iter() {
            if set_aside(&def.id) {
                continue;
            }
            let Some((real, rule)) = real(&def.id) else {
                bad.push(format!("{}: no row in capture::scale::REAL", def.id));
                continue;
            };
            let folder = match def.category {
                extraction_core::items::Category::Magazine => "mags",
                _ if def.firearm.is_some() => "guns",
                _ => "items",
            };
            let path = crate::asset_root_path().join(format!("models/{folder}/{}.glb", def.id));
            // No model: the game draws its box (`dims`), a placeholder
            // held only to being about the right size.
            let model = drawn(&path);
            let size = model.unwrap_or_else(|| def.size_m().into_iter().fold(0.0, f32::max));
            let ratio = size / real;
            let (lo, hi) = if model.is_none() && rule == Hand {
                Rule::Hand.range(1.0)
            } else {
                rule.range(real)
            };
            rows += 1;
            let ok = (lo..=hi).contains(&ratio);
            println!(
                "{:<5} {:<22} real {:>6.3} m  drawn {:>6.3} m  x{ratio:.2}  ({rule:?} {lo}..{hi})",
                if ok { "ok" } else { "FAIL" },
                def.id,
                real,
                size
            );
            if !ok && !KNOWN.iter().any(|k| k.0 == def.id) {
                bad.push(format!(
                    "{}: drawn {size:.3} m, real {real:.3} m: x{ratio:.2}, outside {lo}..{hi} ({rule:?})",
                    def.id
                ));
            }
        }
        assert!(rows > 150, "only {rows} rows");
        assert!(
            bad.is_empty(),
            "items off their real size:\n{}",
            bad.join("\n")
        );
    }

    /// Items off their size waiting on their owners: (id, why).
    const KNOWN: &[(&str, &str)] = &[];

    /// Every row names an item that exists.
    #[test]
    fn every_row_is_an_item() {
        let catalog = crate::items::Catalog::load();
        for (id, ..) in REAL {
            assert!(catalog.kind(id).is_some(), "{id} is in no catalog");
        }
    }
}
