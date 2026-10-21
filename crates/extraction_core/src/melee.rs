//! A blade in a tracked hand: what a strike does, from how the blade was
//! moving when it met something. The game measures the blade's tip
//! velocity; these are the rules.
//!
//! A thrust is speed along the blade toward the point; a slash, speed
//! across it on the edge's side. Below [`MeleeDef::min_speed`] a blade
//! only touches (a slow poke never hurts); above it, harm grows with the
//! square of the speed over that floor, a slash a little less than a
//! thrust. Where it lands multiplies it (the game knows the soft spots).

use serde::{Deserialize, Serialize};

/// A blade, as authored in `items.ron`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeleeDef {
    /// Speed, m/s, below which a strike does nothing.
    pub min_speed: f32,
    /// Damage of a thrust at `min_speed + 4` m/s.
    pub damage: f32,
    /// Blade length from the guard, metres.
    pub blade: f32,
}

/// What kind of blow it was: a point driven in (a thrust), an edge drawn
/// across (a slash), or something blunt swung (a rifle butt).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Blow {
    Point,
    Edge,
    Blunt,
}

/// Speed, m/s, below which a blunt blow is only a bump.
pub const BLUNT_MIN: f32 = 2.5;
/// Harm per kilogram per (m/s over the floor) squared.
pub const BLUNT_K: f32 = 1.4;

/// Harm from something of `mass_kg` meeting a thing at `speed` m/s (into
/// its face): nothing below [`BLUNT_MIN`], then with the mass and the
/// square of the speed over it.
pub fn blunt(mass_kg: f32, speed: f32) -> Option<f32> {
    let over = speed - BLUNT_MIN;
    (over > 0.0).then(|| BLUNT_K * mass_kg * over * over)
}

/// Harm from a blade moving at `along` m/s toward its point and `across`
/// m/s toward its edge, and which kind of blow that was (None: it only
/// touched).
pub fn strike(def: &MeleeDef, along: f32, across: f32) -> Option<(f32, Blow)> {
    const SLASH: f32 = 0.7;
    let thrust = along.max(0.0);
    let slash = across.max(0.0);
    let (speed, blow, k) = if thrust >= slash {
        (thrust, Blow::Point, 1.0)
    } else {
        (slash, Blow::Edge, SLASH)
    };
    let over = speed - def.min_speed;
    if over <= 0.0 {
        return None;
    }
    let s = over / 4.0;
    Some((def.damage * k * s * s, blow))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rifle's butt swung hard hurts; nudged, it doesn't; a heavier gun
    /// hits harder at the same speed.
    #[test]
    fn a_butt_stroke_is_mass_and_speed() {
        assert_eq!(blunt(3.6, 1.5), None);
        let ak = blunt(3.6, 6.0).unwrap();
        assert!(ak > 40.0 && ak < 120.0, "{ak}");
        assert!(blunt(4.0, 6.0).unwrap() > ak);
    }

    fn knife() -> MeleeDef {
        MeleeDef {
            min_speed: 1.5,
            damage: 40.0,
            blade: 0.14,
        }
    }

    #[test]
    fn a_slow_poke_doesnt_hurt() {
        assert_eq!(strike(&knife(), 1.0, 0.0), None);
        assert_eq!(strike(&knife(), 0.0, 1.4), None);
        // Backwards or the flat: nothing either.
        assert_eq!(strike(&knife(), -6.0, -6.0), None);
    }

    #[test]
    fn harder_is_more_and_a_thrust_beats_a_slash() {
        let (soft, _) = strike(&knife(), 3.0, 0.0).unwrap();
        let (hard, blow) = strike(&knife(), 5.5, 0.0).unwrap();
        assert_eq!(blow, Blow::Point);
        assert!(hard > soft * 4.0, "{soft} {hard}");
        assert!((hard - 40.0).abs() < 1e-3);
        let (slash, blow) = strike(&knife(), 0.0, 5.5).unwrap();
        assert_eq!(blow, Blow::Edge);
        assert!(slash < hard);
    }
}
