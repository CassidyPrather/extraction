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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
}

impl Stuff {
    pub const ALL: [Stuff; 10] = [
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
            Stuff::Concrete | Stuff::Rock | Stuff::Sheet | Stuff::Plate | Stuff::Brick
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
