//! Each valley's look: the colour grade every player-view camera gets
//! (see `docs/ART_DIRECTION.md`). The grade runs inside the tonemapping
//! pass the cameras already have, so it costs nothing per pixel beyond
//! what AgX does anyway.
//!
//! `EXTRACTION_LOOK="sat=1.1,contrast=1.05,..."` overrides fields at
//! launch, for trying a look in screenshots without a rebuild.

use bevy::prelude::*;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};

/// A valley's grade. Neutral is all zeros and ones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Stops on top of the cameras' exposure.
    pub exposure: f32,
    /// Warmer (+) or cooler (-), CIE x.
    pub temperature: f32,
    /// Magenta (+) or green (-), CIE y.
    pub tint: f32,
    /// Saturation after tonemapping (AgX desaturates on its way out).
    pub saturation: f32,
    /// Contrast round the midtones.
    pub contrast: f32,
    /// Shadows' saturation: lower keeps the murk from going muddy.
    pub shadow_saturation: f32,
    /// Lifts the blacks a little (VR: nothing too dark to read).
    pub shadow_lift: f32,
    /// Highlights' gain: under 1 holds the sky back.
    pub highlight_gain: f32,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            temperature: 0.0,
            tint: 0.0,
            saturation: 1.0,
            contrast: 1.0,
            shadow_saturation: 1.0,
            shadow_lift: 0.0,
            highlight_gain: 1.0,
        }
    }
}

impl Look {
    /// The look for a map, by its name.
    pub fn for_map(name: &str) -> Self {
        // AgX greys the overcast out on its way to the screen; every valley
        // gets some saturation and a little contrast back. Temperature only
        // nudges: the air's own colour is the fog's and the sky's job (a
        // grade strong enough to tint the valley read as a photo filter).
        let base = Self {
            saturation: 1.25,
            contrast: 1.12,
            ..default()
        };
        let mut look = match name {
            // High, rocky and cold: cooler, a touch harder, less green.
            "Kettle Pass" => Self {
                temperature: -0.008,
                saturation: 1.15,
                contrast: 1.16,
                ..base
            },
            // Burnt: warm and drained, the ash greying everything out.
            "Cinder Works" => Self {
                temperature: 0.012,
                saturation: 1.0,
                contrast: 1.1,
                ..base
            },
            // The practice ground: a shade brighter and kinder, to read.
            "Training Yard" => Self {
                exposure: 0.15,
                ..base
            },
            _ => base,
        };
        if let Ok(s) = std::env::var("EXTRACTION_LOOK") {
            look.apply_overrides(&s);
        }
        look
    }

    fn apply_overrides(&mut self, s: &str) {
        for kv in s.split(',') {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            let Ok(v) = v.trim().parse::<f32>() else {
                continue;
            };
            match k.trim() {
                "exposure" => self.exposure = v,
                "temp" => self.temperature = v,
                "tint" => self.tint = v,
                "sat" => self.saturation = v,
                "contrast" => self.contrast = v,
                "shadow_sat" => self.shadow_saturation = v,
                "lift" => self.shadow_lift = v,
                "gain" => self.highlight_gain = v,
                other => warn!("EXTRACTION_LOOK: unknown key {other}"),
            }
        }
    }

    pub fn grading(&self) -> ColorGrading {
        // Bevy's own contrast pivots at 0.5 in linear light, before the
        // tonemapper: this scene lives well under that, so it crushed
        // everything to black. Contrast here is a power curve round
        // mid-grey instead, built from the CDL's gain and gamma:
        // 0.18 * (c / 0.18)^k = (c * gain)^k with gain = 0.18^((1 - k) / k).
        let k = self.contrast.max(0.1);
        let section = |saturation: f32, lift: f32| ColorGradingSection {
            saturation,
            gamma: 1.0 / k,
            gain: 0.18f32.powf((1.0 - k) / k),
            lift,
            ..default()
        };
        let mut highlights = section(1.0, 0.0);
        highlights.gain *= self.highlight_gain;
        ColorGrading {
            global: ColorGradingGlobal {
                exposure: self.exposure,
                temperature: self.temperature,
                tint: self.tint,
                post_saturation: self.saturation,
                ..default()
            },
            shadows: section(self.shadow_saturation, self.shadow_lift),
            midtones: section(1.0, 0.0),
            highlights,
        }
    }
}
