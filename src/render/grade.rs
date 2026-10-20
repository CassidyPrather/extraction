//! Each valley's look: the colour grade every player-view camera gets
//! (see `docs/ART_DIRECTION.md`). The grade runs inside the tonemapping
//! pass the cameras already have, so it costs nothing per pixel beyond
//! what AgX does anyway.
//!
//! `EXTRACTION_LOOK="sat=1.1,contrast=1.05,..."` overrides fields at
//! launch, for trying a look in screenshots without a rebuild.
//!
//! The eyes adapt (`adapt_eyes`): as the light fails (dusk, a room) the
//! exposure opens up, slowly, like an eye and not a camera: never all
//! the way (dusk and a dark room still read darker than day), fast back
//! the other way.

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
        // The blacks lift a little: under the spruce the frame went near
        // black (a hole, in a headset), and the art direction wants the
        // forest floor's texture readable.
        let base = Self {
            saturation: 1.25,
            contrast: 1.12,
            shadow_lift: 0.003,
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

/// How far a room's walls cut the sky's light: inside a building there's
/// this share of the outside's ambient (`dim_indoors`).
pub const INTERIOR_LIGHT: f32 = 0.3;

/// How much of a light deficit the eyes make up (in stops, as a share).
const ADAPT_SHARE: f32 = 0.6;
/// The most they open up, in stops.
const ADAPT_MAX: f32 = 3.0;
/// Opening up to the dark is slow; closing down to the light is quick (s).
const ADAPT_TO_DARK: f32 = 18.0;
const ADAPT_TO_LIGHT: f32 = 2.5;

/// The eyes' current adaptation, in stops over the valley's grade.
#[derive(Resource, Default, Debug)]
pub struct Adaptation {
    pub stops: f32,
    /// How far indoors the eye is, eased (0 outside, 1 in a room).
    pub indoors: f32,
    /// The grade it's added to (set with the weather).
    pub base: f32,
    settled: bool,
}

impl Adaptation {
    /// What the eyes would settle to, at this light (1 = a day outside).
    pub fn target(light: f32) -> f32 {
        (-light.max(1e-3).log2() * ADAPT_SHARE).clamp(0.0, ADAPT_MAX)
    }

    /// One step toward `target` over `dt` seconds.
    pub fn step(&mut self, target: f32, dt: f32) {
        let tau = if target > self.stops {
            ADAPT_TO_DARK
        } else {
            ADAPT_TO_LIGHT
        };
        self.stops += (target - self.stops) * (1.0 - (-dt / tau).exp());
    }
}

pub fn adapt_eyes(
    conditions: Res<crate::world::weather::Conditions>,
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    eyes: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<super::OffscreenCamera>)>,
    mut grades: Query<&mut ColorGrading>,
    mut adapt: ResMut<Adaptation>,
    time: Res<Time>,
) {
    let eye = eyes
        .iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .next();
    let indoors = match (eye, &entrances) {
        (Some(p), Some(e)) => e.building_at(p).is_some(),
        _ => false,
    };
    // Through a doorway the room's dark comes on over a step or two.
    let dt = time.delta_secs();
    let want = if indoors { 1.0 } else { 0.0 };
    adapt.indoors += (want - adapt.indoors) * (1.0 - (-dt / 0.4).exp());
    let light = conditions.light * (1.0 - adapt.indoors * (1.0 - INTERIOR_LIGHT));
    let target = Adaptation::target(light);
    // A new raid starts with the eyes already used to its light.
    if !adapt.settled || conditions.is_changed() {
        adapt.stops = target;
        adapt.settled = true;
    } else {
        adapt.step(target, dt);
    }
    let want = adapt.base + adapt.stops;
    for mut g in &mut grades {
        if (g.global.exposure - want).abs() > 1e-4 {
            g.global.exposure = want;
        }
    }
}

/// The ambient before `dim_indoors` this frame, and what it wrote, so
/// `undim` gives back exactly what was there (if nothing changed it since).
#[derive(Resource, Default)]
pub struct Dimmed(Option<(f32, f32)>);

/// Indoors, the room's walls cut the sky's light: the ambient (the sky's
/// share of every surface's light; lamps and torches aren't it) drops to
/// `INTERIOR_LIGHT` while the eye is inside a building, so a room at dusk
/// is dark until a light is on. Applied last thing before the frame is
/// drawn and taken off again first thing in the next (`undim`), so the
/// sky and the underground, which set the ambient from what they last
/// wrote, never see it. The outdoors through a doorway dims with it; the
/// sky and the fog, unlit, don't.
pub fn dim_indoors(
    adapt: Res<Adaptation>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut dimmed: ResMut<Dimmed>,
) {
    let k = 1.0 - adapt.indoors * (1.0 - INTERIOR_LIGHT);
    dimmed.0 = (k < 0.999).then(|| {
        let before = ambient.brightness;
        ambient.brightness *= k;
        (before, ambient.brightness)
    });
}

pub fn undim(mut ambient: ResMut<GlobalAmbientLight>, mut dimmed: ResMut<Dimmed>) {
    if let Some((before, wrote)) = dimmed.0.take() {
        if ambient.brightness == wrote {
            ambient.brightness = before;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eyes_adapt_part_way_slowly_into_the_dark_and_fast_out() {
        // A day outside needs nothing; dusk and a dark room open up, but
        // never all the way.
        assert_eq!(Adaptation::target(1.0), 0.0);
        let dusk = Adaptation::target(0.36);
        assert!(dusk > 0.5 && dusk < -(0.36f32.log2()));
        assert!(Adaptation::target(0.0) <= ADAPT_MAX);
        // Into a dark room: after two seconds, well short of adapted...
        let mut a = Adaptation::default();
        let room = Adaptation::target(INTERIOR_LIGHT);
        for _ in 0..20 {
            a.step(room, 0.1);
        }
        assert!(a.stops < room * 0.2, "{} of {room}", a.stops);
        // ...and back out into the day, most of the way in two.
        for _ in 0..20 {
            a.step(0.0, 0.1);
        }
        let before = a.stops;
        let mut b = Adaptation {
            stops: room,
            ..default()
        };
        for _ in 0..20 {
            b.step(0.0, 0.1);
        }
        assert!(b.stops < room * 0.5 && before < room * 0.2);
    }
}
