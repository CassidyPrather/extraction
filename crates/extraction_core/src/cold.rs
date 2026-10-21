//! Cold: the pressure of a high valley (`docs/FIFTH_VALLEY.md`, "Cold").
//!
//! There are no meters in this game, so cold is in the body: out in the
//! wind up high you chill, and a chilled body shivers. Shivering caps your
//! stamina (shorter sprints, and the guns' sway already grows as stamina
//! runs out) and the hands shake. A roof over your head stops the chill and
//! warms you back; so, slowly, does going back down. A valley says how cold
//! it is (`MapDef::cold`); most aren't at all.

use crate::geom::smoothstep;

/// Exposed at the top of a fully cold valley in ordinary weather: seconds
/// from warm to fully chilled.
pub const CHILL_SECS: f32 = 360.0;
/// Under a roof: seconds from fully chilled back to warm.
pub const WARM_SECS: f32 = 50.0;
/// Out in the open but low or sheltered from the worst: seconds to warm.
pub const THAW_SECS: f32 = 150.0;
/// Below this exposure the open air warms you rather than chills you.
pub const MILD: f32 = 0.15;
/// Height over the insertion where exposure starts, and where it's full,
/// metres: the Bottoms are mild, the bench raw, the col and summit bitter.
pub const EXPOSURE_FROM: f32 = 12.0;
pub const EXPOSURE_FULL: f32 = 85.0;
/// Chill at which the shivering starts, and where it's worst.
pub const SHIVER_FROM: f32 = 0.35;
/// The share of stamina a full shiver takes away.
pub const SHIVER_STAMINA: f32 = 0.55;

/// The weather's part: a clear spell is kinder, rain and fog wet you
/// through, snow (`Snow`) and a storm worst.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Weather {
    Clear,
    Ordinary,
    Fog,
    Rain,
    Snow,
}

impl Weather {
    pub fn factor(self) -> f32 {
        match self {
            Weather::Clear => 0.75,
            Weather::Ordinary => 1.0,
            Weather::Fog => 1.15,
            Weather::Rain => 1.35,
            Weather::Snow => 1.6,
        }
    }
}

/// What the air is doing to one body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air {
    /// The valley's cold, 0..=1 (`MapDef::cold`).
    pub cold: f32,
    /// Height over the valley's insertion, metres.
    pub height: f32,
    pub weather: Weather,
    pub night: bool,
    /// A roof over your head.
    pub sheltered: bool,
}

/// How exposed a height is, 0..=1.
pub fn exposure(height: f32) -> f32 {
    smoothstep(EXPOSURE_FROM, EXPOSURE_FULL, height)
}

/// How fast a body chills, per second (negative: it warms).
pub fn rate(air: &Air) -> f32 {
    if air.cold <= 0.0 {
        return -1.0 / WARM_SECS;
    }
    if air.sheltered {
        return -1.0 / WARM_SECS;
    }
    let bite = air.cold
        * exposure(air.height)
        * air.weather.factor()
        * if air.night { 1.3 } else { 1.0 };
    if bite < MILD {
        -1.0 / THAW_SECS
    } else {
        bite / CHILL_SECS
    }
}

/// One step of the chill, kept in 0..=1.
pub fn step(level: f32, air: &Air, dt: f32) -> f32 {
    (level + rate(air) * dt).clamp(0.0, 1.0)
}

/// How hard a body shivers at a chill, 0..=1.
pub fn shiver(level: f32) -> f32 {
    smoothstep(SHIVER_FROM, 1.0, level)
}

/// The most stamina a body has at a chill.
pub fn stamina_cap(level: f32) -> f32 {
    1.0 - SHIVER_STAMINA * shiver(level)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn air(height: f32) -> Air {
        Air {
            cold: 1.0,
            height,
            weather: Weather::Ordinary,
            night: false,
            sheltered: false,
        }
    }

    fn minutes_to(level: f32, air: &Air) -> f32 {
        let (mut c, mut t) = (0.0, 0.0);
        while c < level && t < 7200.0 {
            c = step(c, air, 1.0);
            t += 1.0;
        }
        t / 60.0
    }

    #[test]
    fn the_top_chills_you_in_minutes_the_bottoms_never() {
        let top = minutes_to(1.0, &air(90.0));
        assert!((5.0..8.0).contains(&top), "{top} min on the col");
        // The bench (40 m up) takes longer; the Bottoms never get there.
        let bench = minutes_to(1.0, &air(42.0));
        assert!(bench > top * 1.5, "bench {bench} vs top {top}");
        assert!(minutes_to(0.2, &air(5.0)) >= 120.0, "the Bottoms are mild");
    }

    #[test]
    fn a_roof_warms_you_back_in_under_a_minute() {
        let mut a = air(90.0);
        a.sheltered = true;
        let mut c = 1.0;
        for _ in 0..55 {
            c = step(c, &a, 1.0);
        }
        assert_eq!(c, 0.0);
    }

    #[test]
    fn snow_and_night_bite_harder() {
        let ordinary = rate(&air(90.0));
        let mut snow = air(90.0);
        snow.weather = Weather::Snow;
        snow.night = true;
        assert!(rate(&snow) > ordinary * 1.9);
        let mut clear = air(90.0);
        clear.weather = Weather::Clear;
        assert!(rate(&clear) < ordinary);
    }

    #[test]
    fn a_warm_valley_never_chills() {
        let mut a = air(200.0);
        a.cold = 0.0;
        assert!(rate(&a) < 0.0);
    }

    #[test]
    fn shivering_starts_part_way_and_takes_stamina() {
        assert_eq!(shiver(0.3), 0.0);
        assert_eq!(stamina_cap(0.0), 1.0);
        assert!(shiver(0.7) > 0.3 && shiver(0.7) < 0.8);
        assert!((stamina_cap(1.0) - (1.0 - SHIVER_STAMINA)).abs() < 1e-6);
    }
}
