//! What a round does when it meets something: stops in it, goes through
//! it, or glances off it. Pure rules, shared by the game's projectiles and
//! anything that reasons about cover (the sims), so they agree.
//!
//! A round carries its kinetic energy (½mv², the bullet's mass by
//! cartridge and its speed now). Each material costs so many joules per
//! millimetre of path; the path is the material's thickness, longer when
//! the round meets it at an angle. Thin things are thin whatever their
//! collider says: a plank wall is boards, a tin roof is a millimetre of
//! steel. Hard faces met at a grazing angle may turn the round instead.
//!
//! The numbers are tuned for the cases that matter in a raid, which the
//! tests hold: a pistol round stops in a plank wall and a Mosin's goes
//! through it; rifle rounds go through plaster and tin but nothing stops
//! in a pane of glass; brick, concrete and rock stop everything.

/// What a round can meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Stuff {
    /// Boards, shingles, furniture.
    Planks,
    /// A log wall, a tree trunk.
    Logs,
    Plaster,
    Brick,
    Concrete,
    /// Rock, a boulder, asphalt.
    Rock,
    /// Corrugated or painted sheet: a millimetre or so of steel.
    Sheet,
    /// Heavy plate: a robot's shell, a machine's casing.
    Plate,
    Glass,
    /// The ground.
    Soil,
    /// Worn armour: a hard steel plate.
    ArmourPlate,
    /// A steel helmet: softer, thinner.
    Helmet,
}

impl Stuff {
    pub const ALL: [Stuff; 12] = [
        Stuff::Planks,
        Stuff::Logs,
        Stuff::Plaster,
        Stuff::Brick,
        Stuff::Concrete,
        Stuff::Rock,
        Stuff::Sheet,
        Stuff::Plate,
        Stuff::Glass,
        Stuff::Soil,
        Stuff::ArmourPlate,
        Stuff::Helmet,
    ];

    /// Joules a millimetre of path costs a round.
    pub fn cost_per_mm(self) -> f32 {
        match self {
            Stuff::Planks => 15.0,
            Stuff::Logs => 14.0,
            Stuff::Plaster => 6.0,
            Stuff::Brick => 60.0,
            Stuff::Concrete => 90.0,
            Stuff::Rock => 120.0,
            Stuff::Sheet => 250.0,
            Stuff::Plate => 400.0,
            Stuff::Glass => 2.0,
            Stuff::Soil => 40.0,
            Stuff::ArmourPlate => 600.0,
            Stuff::Helmet => 200.0,
        }
    }

    /// How thick it really is, millimetres, given its collider's thickness:
    /// thin things are capped at what they're made of.
    pub fn real_thickness_mm(self, collider_mm: f32) -> f32 {
        let cap = match self {
            Stuff::Planks => 22.0,
            Stuff::Sheet => 0.8,
            Stuff::Glass => 4.0,
            Stuff::Plaster => 120.0,
            Stuff::Logs => 220.0,
            Stuff::Plate => 12.0,
            _ => f32::INFINITY,
        };
        collider_mm.max(0.0).min(cap)
    }

    /// Steep enough a face that a grazing round may glance off it.
    pub fn ricochets(self) -> bool {
        matches!(
            self,
            Stuff::Concrete
                | Stuff::Rock
                | Stuff::Sheet
                | Stuff::Plate
                | Stuff::Brick
                | Stuff::ArmourPlate
                | Stuff::Helmet
        )
    }
}

/// A bullet's mass, grams, by cartridge (one pellet for buckshot).
pub fn bullet_mass_g(cartridge: &str) -> f32 {
    match cartridge {
        "9x18" => 6.1,
        "5.45x39" => 3.4,
        "7.62x54R" => 9.6,
        "7.62x39" => 7.9,
        "7.62x38R" => 7.0,
        "7.62x25" => 5.5,
        "12ga" => 3.5,
        _ => 8.0,
    }
}

/// Kinetic energy, joules.
pub fn energy_j(mass_g: f32, speed: f32) -> f32 {
    0.5 * mass_g / 1000.0 * speed * speed
}

/// The speed that leaves a round of `mass_g` with `energy` joules.
pub fn speed_for(mass_g: f32, energy: f32) -> f32 {
    (2.0 * energy.max(0.0) / (mass_g / 1000.0)).sqrt()
}

/// Below this cosine of the angle off the face's normal (about 70
/// degrees), a round meeting a hard face may glance off.
pub const GRAZE_COS: f32 = 0.34;
/// Share of its speed a round keeps when it glances off.
pub const RICOCHET_KEEP: f32 = 0.6;

/// What became of the round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// Spent in it.
    Stop,
    /// Out the far side at this speed.
    Through { speed: f32 },
    /// Turned off the face at this speed.
    Ricochet { speed: f32 },
}

/// A round of `mass_g` at `speed` meets `stuff` `thickness_mm` thick (its
/// collider's; thin things are capped), `cos_in` the cosine of its angle
/// off the face's normal (1 head on, 0 grazing). `roll` is uniform 0..1.
pub fn strike(
    stuff: Stuff,
    mass_g: f32,
    speed: f32,
    cos_in: f32,
    thickness_mm: f32,
    roll: f32,
) -> Outcome {
    let cos_in = cos_in.clamp(0.0, 1.0);
    if stuff.ricochets() && cos_in < GRAZE_COS {
        // The flatter it comes in, the likelier it glances.
        if roll < 1.0 - cos_in / GRAZE_COS {
            return Outcome::Ricochet {
                speed: speed * RICOCHET_KEEP,
            };
        }
    }
    let path = stuff.real_thickness_mm(thickness_mm) / cos_in.max(0.2);
    let left = energy_j(mass_g, speed) - stuff.cost_per_mm() * path;
    if left > 0.0 && path.is_finite() {
        Outcome::Through {
            speed: speed_for(mass_g, left),
        }
    } else {
        Outcome::Stop
    }
}

/// Where a piece of armour is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ArmourArea {
    Front,
    Back,
    Head,
}

/// A piece of armour, as authored in `items.ron`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArmourDef {
    /// `ArmourPlate` or `Helmet`.
    pub stuff: Stuff,
    pub thickness_mm: f32,
    pub area: ArmourArea,
}

/// Energy a piece of armour can soak, joules per millimetre of it, before
/// it's worn through (condition 0).
pub const ARMOUR_LIFE_PER_MM: f32 = 4000.0;

/// A round meets worn armour of `condition` (0..=1): its effective
/// thickness thins as it wears (half at condition 0). Returns what became
/// of the round and the condition the armour lost.
pub fn armour_strike(
    def: &ArmourDef,
    condition: f32,
    mass_g: f32,
    speed: f32,
    cos_in: f32,
    roll: f32,
) -> (Outcome, f32) {
    let c = condition.clamp(0.0, 1.0);
    let t = def.thickness_mm * (0.5 + 0.5 * c);
    let out = strike(def.stuff, mass_g, speed, cos_in, t, roll);
    let e = energy_j(mass_g, speed);
    let absorbed = match out {
        Outcome::Stop => e,
        Outcome::Through { speed: s } => e - energy_j(mass_g, s),
        Outcome::Ricochet { speed: s } => (e - energy_j(mass_g, s)) * 0.3,
    };
    let wear = absorbed / (ARMOUR_LIFE_PER_MM * def.thickness_mm.max(0.1));
    (out, wear)
}

/// Whether a round of `cartridge` at `speed` gets through `stuff` this
/// thick head on: for cover reasoning (is that wall cover from a rifle?).
pub fn stops(stuff: Stuff, cartridge: &str, speed: f32, thickness_mm: f32) -> bool {
    strike(
        stuff,
        bullet_mass_g(cartridge),
        speed,
        1.0,
        thickness_mm,
        1.0,
    ) == Outcome::Stop
}

#[cfg(test)]
mod tests {
    use super::*;

    // Muzzle velocities as authored in items.ron.
    const PM: (&str, f32) = ("9x18", 315.0);
    const AK: (&str, f32) = ("5.45x39", 900.0);
    const MOSIN: (&str, f32) = ("7.62x54R", 865.0);
    const SKS: (&str, f32) = ("7.62x39", 735.0);
    const BUCK: (&str, f32) = ("12ga", 400.0);

    fn through(gun: (&str, f32), stuff: Stuff, mm: f32) -> bool {
        !stops(stuff, gun.0, gun.1, mm)
    }

    /// A shed's plank wall (a 100 mm collider, 22 mm of boards) is cover
    /// from a pistol and buckshot, not from a rifle.
    #[test]
    fn a_plank_wall_stops_a_pistol_not_a_rifle() {
        assert!(!through(PM, Stuff::Planks, 100.0));
        assert!(!through(BUCK, Stuff::Planks, 100.0));
        for rifle in [AK, MOSIN, SKS] {
            assert!(through(rifle, Stuff::Planks, 100.0), "{}", rifle.0);
        }
    }

    #[test]
    fn masonry_and_rock_stop_everything() {
        for gun in [PM, AK, MOSIN, SKS, BUCK] {
            for s in [Stuff::Brick, Stuff::Concrete, Stuff::Rock, Stuff::Soil] {
                assert!(!through(gun, s, 250.0), "{} through {s:?}", gun.0);
            }
        }
    }

    #[test]
    fn rifles_go_through_plaster_tin_and_glass() {
        for rifle in [AK, MOSIN, SKS] {
            for s in [Stuff::Plaster, Stuff::Sheet, Stuff::Glass] {
                assert!(through(rifle, s, 120.0), "{} {s:?}", rifle.0);
            }
        }
        // A pistol round goes through tin and glass but not a plaster wall.
        assert!(through(PM, Stuff::Sheet, 20.0));
        assert!(through(PM, Stuff::Glass, 10.0));
        assert!(!through(PM, Stuff::Plaster, 120.0));
        // A log wall stops all but the Mosin.
        for gun in [PM, AK, SKS, BUCK] {
            assert!(!through(gun, Stuff::Logs, 250.0), "{}", gun.0);
        }
        assert!(through(MOSIN, Stuff::Logs, 250.0));
    }

    /// Head on, nothing glances; grazing a hard face mostly does, and
    /// keeps some of its speed; soft things never turn a round.
    #[test]
    fn grazing_hard_faces_ricochet() {
        let m = bullet_mass_g(AK.0);
        assert!(!matches!(
            strike(Stuff::Concrete, m, AK.1, 1.0, 200.0, 0.0),
            Outcome::Ricochet { .. }
        ));
        assert_eq!(
            strike(Stuff::Sheet, m, AK.1, 0.05, 1.0, 0.1),
            Outcome::Ricochet {
                speed: AK.1 * RICOCHET_KEEP
            }
        );
        assert!(!matches!(
            strike(Stuff::Planks, m, AK.1, 0.05, 100.0, 0.0),
            Outcome::Ricochet { .. }
        ));
        // A glancing round still has the energy to hurt.
        assert!(energy_j(m, AK.1 * RICOCHET_KEEP) > energy_j(bullet_mass_g(PM.0), PM.1));
    }

    fn plate() -> ArmourDef {
        ArmourDef {
            stuff: Stuff::ArmourPlate,
            thickness_mm: 6.0,
            area: ArmourArea::Front,
        }
    }

    /// A speed after `metres` of flight (the game's linear drag).
    fn at_range(speed: f32, metres: f32) -> f32 {
        // v' = -k v gives distance d = v0 (1 - e^{-kt}) / k: v = v0 - k d.
        (speed - 0.12 * metres).max(0.0)
    }

    /// A new plate stops a pistol at 25 m and a 5.45 at 100 m; a Mosin
    /// round at 100 m goes through a worn one (condition 0.3) though not
    /// a new one; a steep glance turns off it; every stop wears it.
    #[test]
    fn plates_stop_what_they_should() {
        let p = plate();
        let hit = |gun: (&str, f32), m: f32, cond: f32| {
            armour_strike(&p, cond, bullet_mass_g(gun.0), at_range(gun.1, m), 1.0, 1.0)
        };
        assert_eq!(hit(PM, 25.0, 1.0).0, Outcome::Stop);
        assert_eq!(hit(AK, 100.0, 1.0).0, Outcome::Stop);
        assert_eq!(hit(MOSIN, 100.0, 1.0).0, Outcome::Stop, "new: stops it");
        assert!(
            matches!(hit(MOSIN, 100.0, 0.3).0, Outcome::Through { .. }),
            "worn: through"
        );
        let (glance, _) = armour_strike(&p, 1.0, bullet_mass_g(AK.0), AK.1, 0.1, 0.0);
        assert!(matches!(glance, Outcome::Ricochet { .. }));
        let (_, wear_pm) = hit(PM, 25.0, 1.0);
        let (_, wear_ak) = hit(AK, 100.0, 1.0);
        assert!(wear_pm > 0.0 && wear_ak > wear_pm * 2.0, "{wear_pm} {wear_ak}");
        assert!(wear_ak < 0.1, "a plate takes a good few: {wear_ak}");
    }

    /// A helmet turns fragments and glances, not a pistol round head on.
    #[test]
    fn a_helmet_is_for_fragments() {
        let h = ArmourDef {
            stuff: Stuff::Helmet,
            thickness_mm: 1.2,
            area: ArmourArea::Head,
        };
        // An RGD-5 fragment a few metres out.
        let (frag, _) = armour_strike(&h, 1.0, 0.45, 800.0, 1.0, 1.0);
        assert_eq!(frag, Outcome::Stop);
        let (pm, _) = armour_strike(&h, 1.0, bullet_mass_g(PM.0), PM.1, 1.0, 1.0);
        assert!(matches!(pm, Outcome::Through { .. }));
        let (glance, _) = armour_strike(&h, 1.0, bullet_mass_g(PM.0), PM.1, 0.15, 0.0);
        assert!(matches!(glance, Outcome::Ricochet { .. }));
    }

    /// Through at an angle is a longer path: a rifle that goes through a
    /// board head on can stop in it at a slant.
    #[test]
    fn oblique_is_thicker() {
        let m = bullet_mass_g(PM.0);
        let head_on = strike(Stuff::Sheet, m, PM.1, 1.0, 0.8, 1.0);
        let slant = strike(Stuff::Sheet, m, PM.1, 0.4, 0.8, 1.0);
        match (head_on, slant) {
            (Outcome::Through { speed: a }, Outcome::Through { speed: b }) => assert!(b < a),
            (Outcome::Through { .. }, Outcome::Stop) => {}
            other => panic!("{other:?}"),
        }
    }
}
