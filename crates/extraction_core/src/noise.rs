//! Deterministic gradient noise and a tiny seeded RNG, so terrain, scatter
//! and loot rolls come out identical on every machine and in every sim.

pub use ::gunsmith::dice::Rng;

/// Hashes integer lattice coordinates and a seed to a pseudo-random u32.
fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

fn gradient(x: i32, y: i32, seed: u32) -> (f32, f32) {
    let a = (hash2(x, y, seed) as f32 / u32::MAX as f32) * std::f32::consts::TAU;
    (a.cos(), a.sin())
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// 2D Perlin gradient noise, roughly in `-1..1`.
pub fn perlin(x: f32, y: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let dot = |ix: i32, iy: i32, dx: f32, dy: f32| {
        let g = gradient(ix, iy, seed);
        g.0 * dx + g.1 * dy
    };
    let n00 = dot(x0, y0, fx, fy);
    let n10 = dot(x0 + 1, y0, fx - 1.0, fy);
    let n01 = dot(x0, y0 + 1, fx, fy - 1.0);
    let n11 = dot(x0 + 1, y0 + 1, fx - 1.0, fy - 1.0);
    let u = fade(fx);
    let v = fade(fy);
    let a = n00 + u * (n10 - n00);
    let b = n01 + u * (n11 - n01);
    (a + v * (b - a)) * std::f32::consts::SQRT_2
}

/// Fractal Brownian motion: `octaves` of Perlin noise, each at double the
/// frequency and `gain` times the amplitude of the last.
pub fn fbm(x: f32, y: f32, seed: u32, octaves: u32, gain: f32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += amp * perlin(x * freq, y * freq, seed.wrapping_add(o * 101));
        norm += amp;
        amp *= gain;
        freq *= 2.0;
    }
    sum / norm.max(1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_uniformish() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let xs: Vec<f32> = (0..1000).map(|_| a.f32()).collect();
        let ys: Vec<f32> = (0..1000).map(|_| b.f32()).collect();
        assert_eq!(xs, ys);
        let mean = xs.iter().sum::<f32>() / 1000.0;
        assert!((mean - 0.5).abs() < 0.05);
        assert!(xs.iter().all(|x| (0.0..1.0).contains(x)));
    }

    #[test]
    fn weighted_pick_respects_zero_weights() {
        let mut r = Rng::new(1);
        for _ in 0..100 {
            assert_eq!(r.weighted(&[0.0, 1.0, 0.0]), Some(1));
        }
        assert_eq!(r.weighted(&[0.0, 0.0]), None);
    }

    #[test]
    fn perlin_is_bounded_and_continuous() {
        let mut prev = perlin(0.0, 0.3, 9);
        for i in 1..1000 {
            let v = perlin(i as f32 * 0.01, 0.3, 9);
            assert!(v.abs() <= 1.01, "{v}");
            assert!((v - prev).abs() < 0.1);
            prev = v;
        }
    }
}
