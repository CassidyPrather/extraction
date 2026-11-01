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
//! **The values here are still the second pass's.** The game has no
//! environment light yet; one is coming (Visual design), and finishes
//! tuned without it would be wrong once it lands. Tried on the bench
//! without it, blued steel at an even 0.1 reflectance showed the PM's
//! arched slide and the Mosin's round receiver; lacquered stamped steel as
//! a paint (metallic 0, roughness 0.38) showed the AK's receiver. Re-tune
//! against the environment light, in the yard and the mill.

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
    /// The scan's tile, metres.
    pub tile: f32,
}

/// The scan's tile on blued and stamped steel, metres (at the scan's own
/// 0.15 m it read as a casting's grain at 30 cm).
pub const TILE: f32 = 0.075;

/// Machined steel, polished and blued: the scan at half its tile, a little
/// glossier.
pub const BLUED: SteelFinish = SteelFinish {
    gain: Some([1.0, 1.0, 1.0]),
    flat: [0.0; 3],
    metallic: 1.0,
    roughness: 0.8,
    keep_scan_metal: true,
    tile: TILE,
};

/// Stamped sheet under black lacquer over phosphate: near black, little
/// metal showing, the scan's relief only.
pub const STAMPED: SteelFinish = SteelFinish {
    gain: None,
    flat: [0.0054, 0.0054, 0.0057],
    metallic: 0.2,
    roughness: 0.5,
    keep_scan_metal: false,
    tile: TILE,
};

/// Blackened steel, matt: butt plates and small fittings.
pub const BLACK: SteelFinish = SteelFinish {
    gain: Some([0.173, 0.173, 0.173]),
    flat: [0.0; 3],
    metallic: 1.0,
    roughness: 1.0,
    keep_scan_metal: true,
    tile: 0.15,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Stamped steel reads apart from blued: less metal, its own surface.
    #[test]
    fn stamped_is_not_blued() {
        assert!(STAMPED.metallic < BLUED.metallic);
        assert!(!STAMPED.keep_scan_metal && BLUED.keep_scan_metal);
    }
}
