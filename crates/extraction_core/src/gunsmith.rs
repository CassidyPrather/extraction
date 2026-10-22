//! Gunsmithing at the vice: what a gun has been made into, and what that
//! does to it.
//!
//! - **A cut barrel** (an obrez, a sawn-off): the powder has less barrel
//!   to push the bullet along, so it leaves slower and the rest burns at
//!   the muzzle as flash and noise (`report`'s 4 dB per halving). The
//!   front sight goes with the muzzle and the sight line shortens, so it
//!   groups wider. It's lighter and shorter: handier in a room. A cut
//!   doesn't grow back.
//! - **The stock off**: lighter and shorter again, but nothing to put to
//!   the shoulder, so the kick climbs and the hold sways. It goes back on.
//! - **Parts swapped** between guns of one model: a bolt or a barrel takes
//!   its condition with it. A gun's `condition` is the sum of its parts;
//!   each part's own is kept as its difference from the gun's
//!   ([`Build::bolt_dev`], [`Build::barrel_dev`]), so swapping back puts
//!   both guns as they were.

use serde::{Deserialize, Serialize};

/// What a gun has been made into. The default is as it left the factory.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Build {
    /// Its barrel cut to this (mm); `None` as made.
    pub barrel_mm: Option<f32>,
    /// Its butt stock on.
    pub stock: bool,
    /// Its bolt out of the receiver: it neither cycles nor fires.
    pub bolt_out: bool,
    /// Its bolt's condition, against the gun's.
    pub bolt_dev: f32,
    /// Its barrel's condition, against the gun's.
    pub barrel_dev: f32,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            barrel_mm: None,
            stock: true,
            bolt_out: false,
            bolt_dev: 0.0,
            barrel_dev: 0.0,
        }
    }
}

/// The bolt's share of a gun's condition.
pub const BOLT_SHARE: f32 = 0.35;
/// The barrel's.
pub const BARREL_SHARE: f32 = 0.35;
/// The shortest a barrel can be cut (mm): the chamber and a hand's width.
pub const SHORTEST: f32 = 160.0;

/// What a gun of `id` can have done to it at the vice: (barrel cut, stock
/// off, bolt out by hand).
pub fn workable(id: &str) -> (bool, bool, bool) {
    match id {
        "mosin" => (true, true, true),
        "toz_double" => (true, true, false),
        "sks" => (true, true, false),
        "mp133" => (true, true, false),
        "ak74" | "rpk74" | "svd" => (false, true, false),
        _ => (false, false, false),
    }
}

/// Its butt stock's mass (kg): what comes off with it.
pub fn stock_kg(id: &str) -> f32 {
    match id {
        "mosin" => 0.75,
        "sks" => 0.6,
        "toz_double" | "mp133" => 0.55,
        "ak74" | "rpk74" => 0.35,
        "svd" => 0.4,
        _ => 0.4,
    }
}

/// Its butt stock's length behind the grip (m).
pub fn stock_m(id: &str) -> f32 {
    match id {
        "mosin" => 0.3,
        "toz_double" | "mp133" => 0.3,
        _ => 0.25,
    }
}

/// Barrel and whatever runs along it (forend, bands, rod), kg per metre.
pub fn barrel_kg_per_m(cartridge: &str) -> f32 {
    match cartridge {
        // Two tubes and a rib.
        "12ga" => 1.6,
        "7.62x54R" => 1.4,
        _ => 1.2,
    }
}

/// How much of the bullet's speed a barrel `mm` long gives, against an
/// endless one: the powder's push runs out along it.
pub fn speed_share(cartridge: &str, mm: f32) -> f32 {
    let tau = crate::report::full_burn_mm(cartridge) / 3.5;
    1.0 - (-mm.max(1.0) / tau).exp()
}

/// The gun's barrel as built (mm): cut, or as made.
pub fn barrel_now(build: &Build, factory_mm: f32) -> f32 {
    build.barrel_mm.unwrap_or(factory_mm).min(factory_mm)
}

/// Muzzle speed from a barrel `mm` long, a gun made with `factory_mm`
/// giving `muzzle` (m/s).
pub fn muzzle_speed(cartridge: &str, muzzle: f32, factory_mm: f32, mm: f32) -> f32 {
    muzzle * speed_share(cartridge, mm) / speed_share(cartridge, factory_mm)
}

/// How much wider it groups, against as made: a shorter sight line (and,
/// cut, no front sight but a filed stub), and with no stock to cheek, a
/// wobblier hold.
pub fn spread_factor(build: &Build, factory_mm: f32) -> f32 {
    let mm = barrel_now(build, factory_mm);
    let cut = if build.barrel_mm.is_some() {
        (factory_mm / mm).sqrt() * 1.4
    } else {
        1.0
    };
    cut * if build.stock { 1.0 } else { 1.3 }
}

/// How big the muzzle flash is, against as made: the powder still burning
/// as it leaves.
pub fn flash_factor(cartridge: &str, factory_mm: f32, mm: f32) -> f32 {
    let unburnt = |l: f32| 1.0 - speed_share(cartridge, l) + 0.05;
    (unburnt(mm) / unburnt(factory_mm)).clamp(1.0, 6.0)
}

/// The gun's mass as built (kg), `mass` as made.
pub fn mass_kg(id: &str, cartridge: &str, mass: f32, build: &Build, factory_mm: f32) -> f32 {
    let cut = (factory_mm - barrel_now(build, factory_mm)) / 1000.0;
    let stock = if build.stock { 0.0 } else { stock_kg(id) };
    (mass - cut * barrel_kg_per_m(cartridge) - stock).max(0.3)
}

/// The gun's length as built (m), `length` as made.
pub fn length_m(id: &str, length: f32, build: &Build, factory_mm: f32) -> f32 {
    let cut = (factory_mm - barrel_now(build, factory_mm)) / 1000.0;
    let stock = if build.stock { 0.0 } else { stock_m(id) };
    (length - cut - stock).max(0.2)
}

/// The kick's climb and push, against as made: lighter kicks harder, and
/// with no stock against the shoulder the wrist takes it, so it climbs.
pub fn kick_factor(mass_now: f32, mass: f32, build: &Build) -> (f32, f32) {
    let light = (mass / mass_now.max(0.3)).sqrt();
    if build.stock {
        (light, light)
    } else {
        (light * 1.7, light * 0.8)
    }
}

/// A part's own condition, the gun at `cond` and the part `dev` off it.
pub fn part_condition(cond: f32, dev: f32) -> f32 {
    (cond + dev).clamp(0.0, 1.0)
}

/// A part of condition `part` fitted to a gun at `cond` whose own part
/// (`old_dev` off it) came out: the gun's new condition, and the new
/// part's difference from it. Fitting back the part that came out leaves
/// the gun as it was.
pub fn fit_part(cond: f32, old_dev: f32, part: f32, share: f32) -> (f32, f32) {
    let old = part_condition(cond, old_dev);
    let now = (cond + share * (part - old)).clamp(0.0, 1.0);
    (now, part - now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{BUILTIN_CATALOG, ItemCatalog};
    use crate::report::{barrel_mm, shot_reach};

    /// The variants on the gauge: (name, gun id, barrel cut to, stock on).
    const VARIANTS: [(&str, &str, Option<f32>, bool); 10] = [
        ("Mosin", "mosin", None, true),
        ("Mosin, no stock", "mosin", None, false),
        ("Mosin obrez", "mosin", Some(250.0), false),
        ("TOZ double", "toz_double", None, true),
        ("sawn-off", "toz_double", Some(300.0), false),
        ("SKS", "sks", None, true),
        ("SKS cut", "sks", Some(300.0), true),
        ("MP-133 cut", "mp133", Some(350.0), true),
        ("AK-74", "ak74", None, true),
        ("AK-74, no stock", "ak74", None, false),
    ];

    struct Row {
        name: &'static str,
        mm: f32,
        speed: f32,
        reach: f32,
        moa: f32,
        flash: f32,
        kg: f32,
        m: f32,
        climb: f32,
    }

    fn rows() -> Vec<Row> {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        VARIANTS
            .iter()
            .map(|&(name, id, cut, stock)| {
                let def = cat.get(cat.kind(id).unwrap());
                let f = def.firearm.as_ref().unwrap();
                let factory = barrel_mm(id).unwrap();
                let build = Build {
                    barrel_mm: cut,
                    stock,
                    ..Default::default()
                };
                let mm = barrel_now(&build, factory);
                let kg = mass_kg(id, &f.cartridge, def.mass, &build, factory);
                Row {
                    name,
                    mm,
                    speed: muzzle_speed(&f.cartridge, f.muzzle_velocity, factory, mm),
                    reach: shot_reach(&f.cartridge, mm, crate::ammo::Load::Standard, 0.0)
                        * f.report,
                    moa: f.spread_moa * spread_factor(&build, factory),
                    flash: flash_factor(&f.cartridge, factory, mm),
                    kg,
                    m: length_m(id, def.size_m()[2], &build, factory),
                    climb: kick_factor(kg, def.mass, &build).0,
                }
            })
            .collect()
    }

    /// The gunsmith's gauge: each variant's barrel, muzzle speed, report,
    /// group, flash, mass, length and kick, printed (`cargo test -p
    /// extraction_core gunsmith_gauge -- --nocapture`) and held to what a
    /// cut and a stock off should do.
    #[test]
    fn the_gunsmith_gauge() {
        let rows = rows();
        let mut t = format!(
            "{:>16}{:>7}{:>7}{:>8}{:>7}{:>7}{:>6}{:>6}{:>7}",
            "", "mm", "m/s", "heard", "MOA", "flash", "kg", "m", "climb"
        );
        for r in &rows {
            t += &format!(
                "\n{:>16}{:>7.0}{:>7.0}{:>8.0}{:>7.1}{:>7.1}{:>6.2}{:>6.2}{:>7.2}",
                r.name, r.mm, r.speed, r.reach, r.moa, r.flash, r.kg, r.m, r.climb
            );
        }
        println!("Gunsmithing, variant by variant:\n{t}");
        let row = |n: &str| rows.iter().find(|r| r.name == n).unwrap();
        let (m, o) = (row("Mosin"), row("Mosin obrez"));
        assert!(o.speed < 0.85 * m.speed && o.speed > 0.6 * m.speed, "{t}");
        assert!(o.reach > 1.2 * m.reach, "louder: {t}");
        assert!(o.moa > 3.0 * m.moa, "groups wider: {t}");
        assert!(o.flash > 2.0, "flashes: {t}");
        assert!(o.kg < m.kg - 1.0 && o.m < 0.7 * m.m, "handier: {t}");
        assert!(o.climb > 1.7 * m.climb, "kicks: {t}");
        let (d, s) = (row("TOZ double"), row("sawn-off"));
        assert!(s.m < 0.75 * d.m && s.kg < d.kg && s.reach > d.reach, "{t}");
        // A stock off alone: lighter and shorter, same barrel, kicks up.
        let n = row("Mosin, no stock");
        assert_eq!(n.speed, m.speed);
        assert!(n.kg < m.kg && n.m < m.m && n.climb > m.climb, "{t}");
        let (a, an) = (row("AK-74"), row("AK-74, no stock"));
        assert!(an.climb > a.climb && an.moa > a.moa, "{t}");
    }

    /// A bolt from a wrecked gun into a worn one: the worn one comes up by
    /// the bolt's share, the wreck goes down, and swapping back puts both
    /// as they were.
    #[test]
    fn parts_take_their_condition_and_swap_back() {
        let (good, wreck) = (0.9f32, 0.2f32);
        let (good_bolt, wreck_bolt) = (part_condition(good, 0.0), part_condition(wreck, 0.0));
        // Out of each, into the other.
        let (wreck2, wreck_dev) = fit_part(wreck, 0.0, good_bolt, BOLT_SHARE);
        let (good2, good_dev) = fit_part(good, 0.0, wreck_bolt, BOLT_SHARE);
        assert!(wreck2 > wreck + 0.2, "{wreck2}");
        assert!(good2 < good - 0.2, "{good2}");
        assert!((part_condition(wreck2, wreck_dev) - good_bolt).abs() < 1e-6);
        // And back again.
        let back_bolt = part_condition(wreck2, wreck_dev);
        let (good3, good_dev3) = fit_part(good2, good_dev, back_bolt, BOLT_SHARE);
        let (wreck3, _) = fit_part(
            wreck2,
            wreck_dev,
            part_condition(good2, good_dev),
            BOLT_SHARE,
        );
        assert!((good3 - good).abs() < 1e-5 && good_dev3.abs() < 1e-5);
        assert!((wreck3 - wreck).abs() < 1e-5);
    }

    /// Every gun that can be worked has its barrel listed.
    #[test]
    fn workable_guns_have_barrels() {
        for id in [
            "mosin",
            "toz_double",
            "sks",
            "mp133",
            "ak74",
            "rpk74",
            "svd",
        ] {
            assert!(workable(id).1);
            assert!(barrel_mm(id).is_some(), "{id}");
        }
        assert_eq!(workable("pm_pistol"), (false, false, false));
    }
}
