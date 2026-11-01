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
//! Tuned against Visual design's sky light (`render::sky_light`), in the
//! game in the yard at noon and in the Rime Fell mill: blued steel at
//! about 0.1 reflectance and 0.28 roughness grades from the horizon's
//! band to a darker top; at 0.13 the scan's variation went blotchy. The
//! bench has no sky light, so it shows these flatter than the game does.

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

/// Machined steel, polished and blued: the scan at half its tile, lifted
/// to about 0.1 reflectance, a touch blue, a little glossier.
pub const BLUED: SteelFinish = SteelFinish {
    gain: Some([2.0, 2.05, 2.3]),
    flat: [0.0; 3],
    metallic: 1.0,
    roughness: 0.9,
    keep_scan_metal: true,
    tile: TILE,
};

/// Stamped sheet under black lacquer over phosphate: a semi-gloss black
/// paint, its sheen from the sky, no metal showing.
pub const STAMPED: SteelFinish = SteelFinish {
    gain: None,
    flat: [0.02, 0.02, 0.021],
    metallic: 0.0,
    roughness: 0.35,
    keep_scan_metal: false,
    tile: TILE,
};

/// Blackened steel, matt: butt plates and small fittings.
pub const BLACK: SteelFinish = SteelFinish {
    gain: Some([1.5, 1.5, 1.5]),
    flat: [0.0; 3],
    metallic: 1.0,
    roughness: 1.6,
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
