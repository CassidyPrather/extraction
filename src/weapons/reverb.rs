//! How much a valley's sides smear where a sound seems to come from, for
//! robots' ears ([`valley_smear`]): a pure function of the ground, which
//! `hearing` grids over the map for its outdoor points.

use bevy::prelude::*;

/// Most a valley smears.
pub const VALLEY_MAX: f32 = 2.2;
/// Ground this much higher than you, within [`VALLEY_SPAN`], throws a
/// shot back (m).
pub const WALL_RISE: f32 = 6.0;
/// How far out a valley's sides are looked for (m).
pub const VALLEY_SPAN: (f32, f32) = (15.0, 70.0);

/// How much the ground round (`x`, `z`) smears a sound made there: the
/// share of eight bearings on which it rises [`WALL_RISE`] within
/// [`VALLEY_SPAN`], and more when the walls face each other (a valley's
/// two sides, a flutter between them).
pub fn valley_smear(ground: impl Fn(f32, f32) -> f32, x: f32, z: f32) -> f32 {
    let h0 = ground(x, z);
    let mut walls = [false; 8];
    for (k, wall) in walls.iter_mut().enumerate() {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        let d = Vec2::new(a.cos(), a.sin());
        let mut r = VALLEY_SPAN.0;
        while r <= VALLEY_SPAN.1 {
            if ground(x + d.x * r, z + d.y * r) - h0 >= WALL_RISE {
                *wall = true;
                break;
            }
            r += 10.0;
        }
    }
    let closed = walls.iter().filter(|w| **w).count() as f32 / 8.0;
    let facing = (0..4).filter(|&k| walls[k] && walls[k + 4]).count() as f32 / 4.0;
    (1.0 + 0.6 * closed + 0.6 * facing).min(VALLEY_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Open ground doesn't smear; a valley between two ridges does, and a
    /// hollow ringed by hills more.
    #[test]
    fn valleys_smear_and_plains_dont() {
        let flat = |_: f32, _: f32| 10.0;
        let valley = |x: f32, _: f32| 10.0 + 0.4 * x.abs().min(40.0);
        let bowl = |x: f32, z: f32| 10.0 + 0.4 * (x * x + z * z).sqrt().min(40.0);
        let (f, v, b) = (
            valley_smear(flat, 0.0, 0.0),
            valley_smear(valley, 0.0, 0.0),
            valley_smear(bowl, 0.0, 0.0),
        );
        println!("smear: flat {f:.2}, valley {v:.2}, bowl {b:.2}");
        assert_eq!(f, 1.0);
        assert!(v > 1.4, "{v}");
        assert!(b > v, "{b} vs {v}");
        // Up on the ridge, it's open again.
        assert!(valley_smear(valley, 60.0, 0.0) < v);
    }
}
