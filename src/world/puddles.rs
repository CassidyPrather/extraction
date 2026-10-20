//! Puddles: where rain stands. Hollows on flat ground (where the ground
//! round a point sits higher than it), wet banks and the roads' ruts fill
//! as the ground soaks (`weather::Wetness`) and drain as it dries. The
//! terrain shader draws them (`assets/shaders/terrain.wgsl`, which carries
//! the same mask: keep the two in step); this is the CPU's copy, for what
//! a footstep lands in (`puddle_at`).

use extraction_core::terrain::Heightmap;

/// How far round a point the ground is compared with it (m).
const RING: f32 = 2.0;

/// How much of a hollow `p` is, 0..1: the ground round it higher (by 2-20
/// cm), on flat ground. Baked into the terrain's mud weight, so hollows
/// are muddy as well as where the water stands.
pub fn hollow(hm: &Heightmap, p: [f32; 2]) -> f32 {
    let h = hm.height(p);
    let mut sum = 0.0;
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        sum += hm.height([p[0] + a.cos() * RING, p[1] + a.sin() * RING]);
    }
    let dip = sum / 8.0 - h;
    let flat = 1.0 - smooth(4.0, 9.0, hm.slope_deg(p));
    smooth(0.02, 0.2, dip) * flat
}

/// How full the puddles are for how wet the ground is (0 none, 1 brim).
pub fn fill(wetness: f32) -> f32 {
    smooth(0.35, 0.95, wetness)
}

/// How much of a puddle `p` is in, 0..1, at this fill. `basin` is the
/// terrain's mud weight there (hollows, wet banks), `road` its road
/// weight, `up` the ground normal's y.
pub fn mask(p: [f32; 2], basin: f32, road: f32, up: f32, fill: f32) -> f32 {
    if fill <= 0.0 {
        return 0.0;
    }
    let depth = basin.max(road * 0.5) * 0.9
        + (vnoise([p[0] * 0.45, p[1] * 0.45]) - 0.5) * 0.5
        + (vnoise([p[0] * 1.7 + 13.0, p[1] * 1.7 + 7.0]) - 0.5) * 0.15;
    let level = 1.05 - 0.55 * fill;
    smooth(level, level + 0.07, depth) * smooth(0.985, 0.997, up)
}

/// Whether a footstep at `p` lands in standing water, 0..1.
pub fn puddle_at(hm: &Heightmap, p: [f32; 2], wetness: f32) -> f32 {
    let basin = hollow(hm, p).max(hm.wetness(p));
    let road = hm.road_at(p).weight as f32 / 255.0;
    let up = hm.normal(p)[1];
    mask(p, basin, road, up, fill(wetness))
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The shader's value noise (its `hash2`/`vnoise`), so both agree.
fn hash2(p: [f32; 2]) -> f32 {
    let h = p[0] * 127.1 + p[1] * 311.7;
    (h.sin() * 43_758.547).rem_euclid(1.0)
}

fn vnoise(p: [f32; 2]) -> f32 {
    let i = [p[0].floor(), p[1].floor()];
    let f = [p[0] - i[0], p[1] - i[1]];
    let u = [
        f[0] * f[0] * (3.0 - 2.0 * f[0]),
        f[1] * f[1] * (3.0 - 2.0 * f[1]),
    ];
    let a = hash2(i);
    let b = hash2([i[0] + 1.0, i[1]]);
    let c = hash2([i[0], i[1] + 1.0]);
    let d = hash2([i[0] + 1.0, i[1] + 1.0]);
    let lerp = |x: f32, y: f32, t: f32| x + (y - x) * t;
    lerp(lerp(a, b, u[0]), lerp(c, d, u[0]), u[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puddles_fill_with_the_wet_and_only_on_the_flat() {
        // A spot that's a deep basin on flat ground.
        let p = [10.0, 10.0];
        assert_eq!(mask(p, 1.0, 0.0, 1.0, 0.0), 0.0, "dry ground, no puddle");
        let full = mask(p, 1.0, 0.0, 1.0, 1.0);
        assert!(full > 0.99, "a full basin stands in water: {full}");
        // On a slope, water runs off.
        assert_eq!(mask(p, 1.0, 0.0, 0.95, 1.0), 0.0);
        // More of the ground is under water the fuller they are.
        let share = |fill: f32| {
            let mut n = 0;
            for i in 0..60 {
                for j in 0..60 {
                    let q = [i as f32 * 0.7, j as f32 * 0.7];
                    if mask(q, 0.5, 0.0, 1.0, fill) > 0.5 {
                        n += 1;
                    }
                }
            }
            n
        };
        assert!(share(0.3) < share(0.7) && share(0.7) < share(1.0));
        assert_eq!(fill(0.0), 0.0);
        assert_eq!(fill(1.0), 1.0);
    }
}
