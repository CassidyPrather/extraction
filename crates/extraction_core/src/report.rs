//! How far a shot is heard: its report (the muzzle blast) and, for a round
//! faster than sound, its crack (the bow wave along its flight).
//!
//! The report is reckoned in decibels against the AK-74's. A heavier
//! powder charge is louder; a barrel too short to burn its charge vents
//! the rest at the muzzle and is louder still; a suppressor takes decibels
//! off, fewer as it wears. Reach doubles every [`DB_PER_DOUBLING`]: a
//! robot listens over the wind and the valley's own noise, not in a
//! quiet room.
//!
//! The crack is heard only near the round's path, and from the path: a
//! listener downrange hears it come from somewhere along the line, not
//! from the gun.

use crate::ammo::Load;

/// The speed of sound, m/s.
pub const SPEED_OF_SOUND: f32 = 343.0;
/// A round slower than this doesn't crack (a little over the speed of
/// sound: the bow wave needs some margin to form).
pub const CRACKS_OVER: f32 = SPEED_OF_SOUND * 1.03;
/// The AK-74's report carries this far (m): the yardstick.
pub const REF_REACH: f32 = 220.0;
/// Reach doubles for this many decibels more.
pub const DB_PER_DOUBLING: f32 = 12.0;
/// Shortest and longest a report is heard (m).
pub const REACH_MIN: f32 = 40.0;
pub const REACH_MAX: f32 = 320.0;
/// A subsonic load's charge, against ball's.
pub const SUBSONIC_CHARGE: f32 = 0.6;

/// Grams of powder in a cartridge.
pub fn charge_g(cartridge: &str) -> f32 {
    match cartridge {
        "9x18" => 0.25,
        "7.62x38R" => 0.24,
        "7.62x25" => 0.52,
        "5.45x39" => 1.45,
        "7.62x39" => 1.6,
        "7.62x54R" => 3.1,
        "12ga" => 2.0,
        "22lr" => 0.1,
        // A signal star's lift charge.
        "26mm" => 0.6,
        _ => 1.45,
    }
}

/// The barrel length (mm) that burns a cartridge's charge before the
/// muzzle: shorter, and the rest goes off in the open.
pub fn full_burn_mm(cartridge: &str) -> f32 {
    match cartridge {
        "9x18" | "7.62x38R" => 110.0,
        "7.62x25" => 200.0,
        "5.45x39" | "7.62x39" => 500.0,
        "7.62x54R" => 650.0,
        "12ga" => 600.0,
        "22lr" => 400.0,
        _ => 500.0,
    }
}

/// A gun's barrel length (mm), by catalog id; `None` for a gun not listed
/// (it's taken to burn its charge fully).
pub fn barrel_mm(gun: &str) -> Option<f32> {
    Some(match gun {
        "pm_pistol" => 93.0,
        "nagant" => 114.0,
        "toz8" => 640.0,
        "ppsh" => 269.0,
        "mp133" => 510.0,
        "toz_double" => 720.0,
        "sks" => 520.0,
        "ak74" => AK74_BARREL_MM,
        "rpk74" => 590.0,
        "svd" => 620.0,
        "mosin" => 730.0,
        "flare_pistol" => 120.0,
        _ => return None,
    })
}

/// Decibels a short barrel adds per halving under its full burn (and a
/// long one takes off per doubling over it).
const DB_PER_HALVED_BARREL: f32 = 4.0;

/// The report's level against the AK-74's (dB), unsuppressed.
pub fn report_db(cartridge: &str, barrel_mm: f32, load: Load) -> f32 {
    raw_db(cartridge, barrel_mm, load) - raw_db("5.45x39", AK74_BARREL_MM, Load::Standard)
}

/// The AK-74's barrel (mm).
pub const AK74_BARREL_MM: f32 = 415.0;

fn raw_db(cartridge: &str, barrel_mm: f32, load: Load) -> f32 {
    let charge = charge_g(cartridge)
        * if load == Load::Subsonic {
            SUBSONIC_CHARGE
        } else {
            1.0
        };
    let short = (full_burn_mm(cartridge) / barrel_mm.max(20.0))
        .log2()
        .clamp(-0.5, 2.0);
    10.0 * charge.max(0.01).log10() + DB_PER_HALVED_BARREL * short
}

/// How far a report `db` over the AK-74's carries (m).
pub fn reach_m(db: f32) -> f32 {
    (REF_REACH * 2f32.powf(db / DB_PER_DOUBLING)).clamp(REACH_MIN, REACH_MAX)
}

/// A suppressor on the muzzle: what it takes off new, and how many shots
/// it lasts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Suppressor {
    pub cut_db: f32,
    pub life_shots: f32,
}

/// The PBS: machined baffles, good for a long time.
pub const PBS: Suppressor = Suppressor {
    cut_db: 18.0,
    life_shots: 1500.0,
};
/// A can packed at the bench: it works, for a while.
pub const CAN: Suppressor = Suppressor {
    cut_db: 14.0,
    life_shots: 40.0,
};

impl Suppressor {
    /// What it takes off at `condition` (1 new, 0 burnt through): a worn
    /// one, its baffles eroded and fouled, a little less, then much less.
    pub fn cut_at(self, condition: f32) -> f32 {
        self.cut_db * condition.clamp(0.0, 1.0).sqrt()
    }

    /// Condition lost per shot.
    pub fn wear_per_shot(self) -> f32 {
        1.0 / self.life_shots.max(1.0)
    }
}

/// How far the shot's report carries (m): `cut_db` off for a suppressor.
pub fn shot_reach(cartridge: &str, barrel_mm: f32, load: Load, cut_db: f32) -> f32 {
    reach_m(report_db(cartridge, barrel_mm, load) - cut_db)
}

/// Whether a round this fast cracks.
pub fn cracks(speed: f32) -> bool {
    speed > CRACKS_OVER
}

/// How far either side of its path a round's crack is heard (m), by its
/// mass and speed: none below the speed of sound.
pub fn crack_reach_m(mass_g: f32, speed: f32) -> f32 {
    if !cracks(speed) {
        return 0.0;
    }
    // A bigger bullet pushes a bigger wave; a faster one a sharper one.
    let over = ((speed - SPEED_OF_SOUND) / SPEED_OF_SOUND).min(2.0);
    (14.0 * (mass_g / 3.4).max(0.2).sqrt() * (0.6 + 0.4 * over)).clamp(6.0, 45.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's guns: (name, cartridge, barrel mm, muzzle m/s), as
    /// `items.ron` authors them.
    pub(crate) const GUNS: [(&str, &str, f32, f32); 11] = [
        ("pm_pistol", "9x18", 1.0, 315.0),
        ("nagant", "7.62x38R", 1.0, 272.0),
        ("toz8", "22lr", 1.0, 330.0),
        // The PPSh's jacket is a compensator: the blast goes out its
        // sides (`report` 1.25 in items.ron).
        ("ppsh", "7.62x25", 1.25, 490.0),
        ("mp133", "12ga", 1.0, 400.0),
        ("toz_double", "12ga", 1.0, 400.0),
        ("sks", "7.62x39", 1.0, 735.0),
        ("ak74", "5.45x39", 1.0, 900.0),
        ("rpk74", "5.45x39", 1.0, 960.0),
        ("svd", "7.62x54R", 1.0, 830.0),
        ("mosin", "7.62x54R", 1.0, 865.0),
    ];

    fn reach(gun: &str, load: Load, cut: f32) -> f32 {
        let g = GUNS.iter().find(|g| g.0 == gun).unwrap();
        shot_reach(g.1, barrel_mm(gun).unwrap(), load, cut) * g.2
    }

    /// Every gun in the catalog has its barrel listed (the crossbow aside:
    /// no report).
    #[test]
    fn every_gun_has_a_barrel() {
        let cat = crate::items::ItemCatalog::from_ron(crate::items::BUILTIN_CATALOG).unwrap();
        for (_, def) in cat.iter() {
            if let Some(f) = &def.firearm {
                if f.cartridge != "bolt" {
                    assert!(barrel_mm(&def.id).is_some(), "{}", def.id);
                }
            }
        }
    }

    /// The hearing gauge: every gun's report, ball and subsonic, bare and
    /// under each suppressor new and worn, printed (`cargo test -p
    /// extraction_core hearing_gauge -- --nocapture`) and held to its
    /// order: the Mosin and the SVD loudest, the PPSh well over a pistol,
    /// the .22 quietest; a suppressor takes a rifle under a pistol, a worn
    /// can gives most of it back.
    #[test]
    fn the_hearing_gauge() {
        let mut t = format!(
            "{:>11}{:>7}{:>7}{:>7}{:>9}{:>8}{:>7}{:>7}",
            "", "dB", "ball", "subs", "PBS", "PBS 10%", "can", "can 10%"
        );
        for (gun, cart, _, _) in GUNS {
            let barrel = barrel_mm(gun).unwrap();
            let sub = if matches!(cart, "5.45x39" | "7.62x39") {
                format!("{:>7.0}", reach(gun, Load::Subsonic, 0.0))
            } else {
                format!("{:>7}", "-")
            };
            t += &format!(
                "\n{gun:>11}{:>7.1}{:>7.0}{sub}{:>9.0}{:>8.0}{:>7.0}{:>7.0}",
                report_db(cart, barrel, Load::Standard),
                reach(gun, Load::Standard, 0.0),
                reach(gun, Load::Standard, PBS.cut_at(1.0)),
                reach(gun, Load::Standard, PBS.cut_at(0.1)),
                reach(gun, Load::Standard, CAN.cut_at(1.0)),
                reach(gun, Load::Standard, CAN.cut_at(0.1)),
            );
        }
        println!("How far each gun's report is heard (m):\n{t}");
        let r = |g| reach(g, Load::Standard, 0.0);
        assert!((r("ak74") - REF_REACH).abs() < 0.5);
        assert!(r("mosin") > r("ak74") && r("svd") > r("ak74"), "{t}");
        assert!(r("ppsh") > 1.2 * r("pm_pistol"), "{t}");
        assert!(r("pm_pistol") > r("toz8") && r("nagant") > r("toz8"), "{t}");
        assert!(r("ak74") > r("ppsh"), "{t}");
        for g in GUNS {
            assert!(r(g.0) <= REACH_MAX && r(g.0) >= REACH_MIN);
        }
        // Suppressed, a rifle is quieter than a bare pistol.
        let ak_pbs = reach("ak74", Load::Standard, PBS.cut_at(1.0));
        assert!(ak_pbs < r("pm_pistol"), "{t}");
        // Subsonic, quieter still.
        assert!(reach("ak74", Load::Subsonic, PBS.cut_at(1.0)) < ak_pbs);
        // A worn can gives most of it back.
        let can_new = reach("ak74", Load::Standard, CAN.cut_at(1.0));
        let can_worn = reach("ak74", Load::Standard, CAN.cut_at(0.1));
        assert!(can_new < 0.5 * r("ak74"));
        assert!(can_worn > 0.7 * r("ak74"), "{can_worn}");
    }

    /// The crack: rifles crack, pistols and subsonic loads don't; a big
    /// round's is heard further from its path.
    #[test]
    fn the_crack_gauge() {
        let mut t = String::new();
        for (gun, cart, _, v) in GUNS {
            let m = crate::firearm_terminal::bullet_mass_g(cart);
            t += &format!("{gun:>11}: {:>4.0} m\n", crack_reach_m(m, v));
        }
        println!("How far from its path a round's crack is heard:\n{t}");
        for (gun, cart, _, v) in GUNS {
            let m = crate::firearm_terminal::bullet_mass_g(cart);
            let rifle = matches!(
                gun,
                "ak74" | "rpk74" | "sks" | "svd" | "mosin" | "ppsh" | "mp133" | "toz_double"
            );
            assert_eq!(crack_reach_m(m, v) > 0.0, rifle, "{gun}\n{t}");
        }
        assert!(crack_reach_m(9.6, 865.0) > crack_reach_m(3.4, 900.0));
        assert_eq!(crack_reach_m(5.2, 300.0), 0.0, "a subsonic round");
    }
}
