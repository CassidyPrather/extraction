//! Facts about the cartridges the guns chamber.

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
        "22lr" => 2.6,
        // A signal star: a burning pellet of compound.
        "26mm" => 20.0,
        _ => 8.0,
    }
}
