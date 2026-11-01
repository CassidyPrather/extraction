//! The finishes gun steel wears, as numbers the game's materials
//! (`src/models.rs`) and the bench both build from, so a finish judged on
//! the bench is the finish in the game.
//!
//! Each is built on the CC0 gun-metal scan (`materials/gun_metal`), whose
//! albedo averages 0.045 linear at 0.85 metallic. For a metal the albedo is
//! its reflectance, and 0.045 is darker than charcoal: polished steel
//! reflects about 0.55, bluing (a thin black oxide) takes it to roughly
//! 0.15-0.2. With no environment light in the game, too low a reflectance
//! leaves a metal black but for the sun's glint, and a curve has nothing
//! to show itself by. `docs/craft/GUNSMITHING.md`, "Finishes".
//!
//! (A metal finish without `gain` keeps the scan's metal, roughness and
//! normal maps and takes a flat reflectance: `keep_scan_metal`.)

/// How a finish is made from the gun-metal scan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteelFinish {
    /// Multiplies the scan's albedo (linear rgb): its reflectance as a
    /// metal, its colour as paint. `None`: a flat colour of `flat` instead.
    pub gain: Option<[f32; 3]>,
    /// The flat colour (linear rgb) when there's no `gain`.
    pub flat: [f32; 3],
    /// Metallic: multiplies the scan's where it keeps it (`keep_scan_metal`).
    pub metallic: f32,
    /// Roughness: multiplies the scan's, or is it, as `keep_scan_metal`.
    pub roughness: f32,
    /// Keep the scan's metal and roughness map (a metal); else flat values
    /// (a paint).
    pub keep_scan_metal: bool,
}

/// The scan's tile on blued and stamped steel, metres (at the scan's own
/// 0.15 m it read as a casting's grain at 30 cm).
pub const TILE: f32 = 0.075;

/// Machined steel, polished and blued: a dark blue-grey metal reflecting
/// about 0.1, even (the scan's albedo left off: scaled up, its mottle read
/// as a casting's or galvanising, not bluing), its sheen varying with the
/// scan's roughness.
pub const BLUED: SteelFinish = SteelFinish {
    gain: None,
    flat: [0.09, 0.095, 0.11],
    metallic: 1.0,
    roughness: 1.15,
    keep_scan_metal: true,
};

/// Stamped sheet under black lacquer over phosphate: a paint, not a metal
/// (its gloss is the lacquer's), near black, the scan's relief only.
pub const STAMPED: SteelFinish = SteelFinish {
    gain: None,
    flat: [0.028, 0.028, 0.03],
    metallic: 0.0,
    roughness: 0.38,
    keep_scan_metal: false,
};

/// Blackened steel, matt: butt plates and small fittings, reflecting about
/// 0.08.
pub const BLACK: SteelFinish = SteelFinish {
    gain: Some([1.8, 1.8, 1.85]),
    flat: [0.0; 3],
    metallic: 1.0,
    roughness: 1.9,
    keep_scan_metal: true,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Blued steel reflects far more than the scan's 0.045 and far less
    /// than bare steel's 0.55; blackened steel less again.
    #[test]
    fn blued_reflects_as_blued_steel_does() {
        let r = BLUED.flat[1];
        assert!((0.08..0.2).contains(&r), "{r}");
        let k = BLACK.gain.unwrap()[1] * 0.045;
        assert!(k < r, "blackened under blued");
        assert!(STAMPED.metallic == 0.0, "lacquer is a paint");
    }
}
