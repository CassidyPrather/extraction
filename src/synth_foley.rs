//! Stand-ins: sounds of real things, synthesised because no recording of
//! them could be fetched yet (Freesound out of reach from where the bank
//! was last built). Each is a small model of the real sound (a bark is a
//! pitched burst through a dog's formants; a drip a droplet's rising
//! chirp; a breath shaped noise through the mouth), varied per variant,
//! deterministic, levelled like the robots' words. When a recording can
//! be fetched it replaces the set by name and this entry goes.
//!
//! Rendered with the robots' words at start-up (`audio::render_voices`),
//! measured by the audit like any set (`scripts/audio_voices.py`).
//!
//! Logic only: no audio types.

use extraction_core::noise::Rng;
use std::f32::consts::TAU;

/// A stand-in set: its name, how many variants, and how to make one.
pub struct StandIn {
    pub set: &'static str,
    pub variants: usize,
    pub render: fn(&mut Rng, f32) -> Vec<f32>,
}

/// Every stand-in in the bank.
pub const STAND_INS: &[StandIn] = &[
    StandIn {
        set: "wildlife/deer_bark",
        variants: 4,
        render: deer_bark,
    },
    StandIn {
        set: "wildlife/dog_bark",
        variants: 5,
        render: dog_bark,
    },
    StandIn {
        set: "wildlife/dog_yelp",
        variants: 3,
        render: dog_yelp,
    },
    StandIn {
        set: "wildlife/rats",
        variants: 4,
        render: rats,
    },
    StandIn {
        set: "wildlife/chough",
        variants: 4,
        render: chough,
    },
    StandIn {
        set: "wildlife/heron",
        variants: 3,
        render: heron,
    },
    StandIn {
        set: "wildlife/chamois",
        variants: 3,
        render: chamois,
    },
    StandIn {
        set: "wildlife/duck",
        variants: 5,
        render: duck,
    },
    StandIn {
        set: "wildlife/bats",
        variants: 3,
        render: bats,
    },
    StandIn {
        set: "body/breath",
        variants: 4,
        render: breath,
    },
    StandIn {
        set: "ambience/drip",
        variants: 6,
        render: drip,
    },
    StandIn {
        set: "ambience/lap",
        variants: 4,
        render: lap,
    },
];

pub fn is_stand_in(set: &str) -> bool {
    STAND_INS.iter().any(|s| s.set == set)
}

/// Render variant `v` of `set` at `sample_rate`, seeded by both.
pub fn render(set: &str, v: usize, sample_rate: f32) -> Option<Vec<f32>> {
    let s = STAND_INS.iter().find(|s| s.set == set)?;
    let seed = set.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x100_0000_01b3)
    });
    let mut rng = Rng::new(seed ^ (v as u64).wrapping_mul(0x9e37_79b9));
    let mut x = (s.render)(&mut rng, sample_rate);
    level(&mut x);
    Some(x)
}

/// The level every stand-in is rendered at: its sounding part's RMS
/// (dBFS), peaks no higher than -1 dBFS. The mix sets the rest.
pub const LEVEL_DBFS: f32 = -18.0;

fn level(x: &mut [f32]) {
    let sounding: Vec<f32> = x.iter().copied().filter(|v| v.abs() > 1e-4).collect();
    if sounding.is_empty() {
        return;
    }
    let rms = (sounding.iter().map(|v| v * v).sum::<f32>() / sounding.len() as f32).sqrt();
    let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let g = (10f32.powf(LEVEL_DBFS / 20.0) / rms.max(1e-9)).min(0.89 / peak.max(1e-9));
    for v in x.iter_mut() {
        *v *= g;
    }
}

/// A resonant band-pass (RBJ, 0 dB peak).
struct Bp {
    b0: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Bp {
    fn new(f: f32, q: f32, sr: f32) -> Self {
        let w = TAU * f.min(sr * 0.45) / sr;
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b2: -alpha / a0,
            a1: -2.0 * w.cos() / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

fn noise(rng: &mut Rng) -> f32 {
    rng.f32() * 2.0 - 1.0
}

fn jitter(rng: &mut Rng, amount: f32) -> f32 {
    1.0 + (rng.f32() - 0.5) * 2.0 * amount
}

/// A pitched, buzzy burst through formants: the shape of most calls.
/// `pitch(t)` in Hz over the call's `secs`, `env(t)` its loudness,
/// `formants` (Hz, Q, weight), `rough` how much noise rides on it.
#[allow(clippy::too_many_arguments)]
fn call(
    rng: &mut Rng,
    sr: f32,
    secs: f32,
    pitch: impl Fn(f32) -> f32,
    env: impl Fn(f32) -> f32,
    formants: &[(f32, f32, f32)],
    harmonics: usize,
    rough: f32,
) -> Vec<f32> {
    let n = (secs * sr) as usize;
    let mut filters: Vec<(Bp, f32)> = formants
        .iter()
        .map(|&(f, q, w)| (Bp::new(f, q, sr), w))
        .collect();
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let x = t / secs;
            phase = (phase + pitch(x) / sr).fract();
            let mut src = 0.0;
            for h in 1..=harmonics {
                src += (TAU * phase * h as f32).sin() / h as f32;
            }
            src += noise(rng) * rough;
            let shaped: f32 = filters.iter_mut().map(|(f, w)| f.run(src) * *w).sum();
            shaped * env(x)
        })
        .collect()
}

fn swell(x: f32, attack: f32) -> f32 {
    (x / attack).min(1.0) * (1.0 - x).max(0.0).powf(0.7)
}

/// A roe deer's alarm: one hoarse, barking "bö", 0.3 s, a falling pitch
/// under a lot of breath.
fn deer_bark(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let p = 420.0 * jitter(rng, 0.12);
    let secs = 0.3 * jitter(rng, 0.15);
    call(
        rng,
        sr,
        secs,
        |x| p * (1.0 - 0.35 * x),
        |x| swell(x, 0.08),
        &[(700.0, 3.0, 1.0), (1300.0, 4.0, 0.6), (2600.0, 5.0, 0.3)],
        8,
        0.9,
    )
}

/// A dog's bark: short and punchy, pitch rising then falling, 0.16 s.
fn dog_bark(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let p = 380.0 * jitter(rng, 0.2);
    let secs = 0.16 * jitter(rng, 0.2);
    call(
        rng,
        sr,
        secs,
        |x| p * (1.0 + 0.3 * (x * 3.0).min(1.0) - 0.5 * x),
        |x| swell(x, 0.05),
        &[(600.0, 2.5, 1.0), (1500.0, 3.0, 0.7), (2800.0, 4.0, 0.3)],
        10,
        0.5,
    )
}

/// A dog's yelp: a high, thin glide down, 0.35 s.
fn dog_yelp(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let p = 900.0 * jitter(rng, 0.15);
    let secs = 0.35 * jitter(rng, 0.15);
    call(
        rng,
        sr,
        secs,
        |x| p * (1.3 - 0.6 * x),
        |x| swell(x, 0.03),
        &[(1200.0, 3.0, 1.0), (2600.0, 4.0, 0.5)],
        6,
        0.15,
    )
}

/// Rats in a wall: a few high squeaks and the scrabble of claws.
fn rats(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let secs = 1.2;
    let n = (secs * sr) as usize;
    let mut out = vec![0.0f32; n];
    let squeaks = 2 + (rng.f32() * 3.0) as usize;
    for _ in 0..squeaks {
        let at = (rng.f32() * 0.9 * sr) as usize;
        let f = 4200.0 * jitter(rng, 0.25);
        let len = (0.04 + rng.f32() * 0.05) * sr;
        let mut ph = 0.0f32;
        for i in 0..len as usize {
            if at + i >= n {
                break;
            }
            let x = i as f32 / len;
            ph = (ph + f * (1.0 + 0.2 * (x * 9.0).sin()) / sr).fract();
            out[at + i] += (TAU * ph).sin() * (x * (1.0 - x) * 4.0) * 0.6;
        }
    }
    // Claws: tiny clicks in a run.
    let mut bp = Bp::new(3000.0, 1.5, sr);
    let mut t = rng.f32() * 0.3;
    while t < secs - 0.05 {
        let at = (t * sr) as usize;
        for i in 0..((0.004 * sr) as usize) {
            if at + i < n {
                out[at + i] += bp.run(noise(rng)) * (1.0 - i as f32 / (0.004 * sr)) * 0.8;
            }
        }
        t += 0.02 + rng.f32() * 0.05;
    }
    out
}

/// An alpine chough: a high, ringing, rising-then-falling "chee-oo".
fn chough(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let p = 1700.0 * jitter(rng, 0.1);
    let secs = 0.32 * jitter(rng, 0.15);
    call(
        rng,
        sr,
        secs,
        |x| p * (1.0 + 0.25 * (x * 2.5).min(1.0) - 0.5 * x * x),
        |x| swell(x, 0.15),
        &[(2400.0, 2.5, 1.0), (4200.0, 4.0, 0.3)],
        3,
        0.05,
    )
}

/// A grey heron's croak: a harsh, rasping "frarnk", low and long.
fn heron(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let p = 210.0 * jitter(rng, 0.1);
    let secs = 0.5 * jitter(rng, 0.15);
    call(
        rng,
        sr,
        secs,
        |x| p * (1.0 + 0.1 * (x * 40.0).sin() - 0.2 * x),
        |x| swell(x, 0.1),
        &[(700.0, 2.0, 1.0), (1600.0, 3.0, 0.7), (3000.0, 4.0, 0.35)],
        14,
        0.8,
    )
}

/// A chamois' alarm: a sharp, breathy whistle.
fn chamois(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let p = 2300.0 * jitter(rng, 0.1);
    let secs = 0.55 * jitter(rng, 0.15);
    call(
        rng,
        sr,
        secs,
        |x| p * (1.0 - 0.08 * x),
        |x| swell(x, 0.06),
        &[(p, 8.0, 1.0)],
        1,
        1.2,
    )
}

/// A mallard's quack: nasal, a pitched burst through two bright
/// formants, 0.18 s.
fn duck(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let p = 320.0 * jitter(rng, 0.15);
    let secs = 0.18 * jitter(rng, 0.2);
    call(
        rng,
        sr,
        secs,
        |x| p * (1.1 - 0.25 * x),
        |x| swell(x, 0.1),
        &[(1100.0, 5.0, 1.0), (2400.0, 6.0, 0.7)],
        12,
        0.3,
    )
}

/// Bats: a quiet flicker of high ticks, the little of their calls a
/// person hears.
fn bats(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let secs = 0.6;
    let n = (secs * sr) as usize;
    let mut out = vec![0.0f32; n];
    let mut bp = Bp::new(9000.0, 3.0, sr);
    let mut t = rng.f32() * 0.05;
    while t < secs - 0.01 {
        let at = (t * sr) as usize;
        let len = (0.002 * sr) as usize;
        for i in 0..len {
            if at + i < n {
                out[at + i] += bp.run(noise(rng)) * (1.0 - i as f32 / len as f32);
            }
        }
        t += 0.03 + rng.f32() * 0.08;
    }
    out
}

/// A heavy breath, out and in: shaped noise through the mouth's
/// formants, 1.1 s.
fn breath(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let secs = 1.1 * jitter(rng, 0.1);
    let n = (secs * sr) as usize;
    let mut a = Bp::new(900.0 * jitter(rng, 0.1), 1.2, sr);
    let mut b = Bp::new(2200.0, 1.5, sr);
    (0..n)
        .map(|i| {
            let x = i as f32 / n as f32;
            // Out (louder), a catch, in.
            let env = if x < 0.45 {
                (x / 0.08).min(1.0) * (1.0 - x / 0.45).powf(0.5)
            } else if x < 0.5 {
                0.0
            } else {
                0.55 * ((x - 0.5) / 0.1).min(1.0) * (1.0 - (x - 0.5) / 0.5)
            };
            let s = noise(rng);
            (a.run(s) + 0.4 * b.run(s)) * env
        })
        .collect()
}

/// A drop of water falling into water: the bubble's quick rising chirp
/// and its ring, 0.12 s.
fn drip(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let f0 = 700.0 * jitter(rng, 0.35);
    let n = (0.12 * sr) as usize;
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let f = f0 * (1.0 + 1.6 * (t / 0.02).min(1.0));
            ph = (ph + f / sr).fract();
            (TAU * ph).sin() * (-t * 45.0).exp() * (t / 0.001).min(1.0)
        })
        .collect()
}

/// Water lapping at a shore or a wall: a low swash in and its suck back,
/// 1 s.
fn lap(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let secs = 1.0 * jitter(rng, 0.2);
    let n = (secs * sr) as usize;
    let mut a = Bp::new(500.0 * jitter(rng, 0.2), 0.9, sr);
    let mut b = Bp::new(1600.0, 1.2, sr);
    (0..n)
        .map(|i| {
            let x = i as f32 / n as f32;
            let env = (x / 0.2).min(1.0).powi(2) * (1.0 - x).powf(1.5);
            let s = noise(rng);
            (a.run(s) + 0.35 * b.run(s) * (1.0 - x)) * env
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Every stand-in renders, sounds, never clips, is the same each time
    /// for a variant and different across them.
    #[test]
    fn every_stand_in_renders() {
        for s in STAND_INS {
            let a = render(s.set, 0, SR).unwrap();
            assert_eq!(a, render(s.set, 0, SR).unwrap(), "{}", s.set);
            if s.variants > 1 {
                assert_ne!(a, render(s.set, 1, SR).unwrap(), "{}", s.set);
            }
            let peak = a.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.05 && peak <= 0.9, "{}: {peak}", s.set);
            assert!(a.iter().all(|v| v.is_finite()), "{}", s.set);
            let secs = a.len() as f32 / SR;
            assert!((0.05..=2.0).contains(&secs), "{}: {secs}", s.set);
        }
    }

    /// The renders for the audit: target/audio_audit/standins/<set>_<v>.wav.
    #[test]
    fn renders_for_the_audit() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/audio_audit/standins");
        std::fs::create_dir_all(&dir).unwrap();
        for s in STAND_INS {
            for v in 0..s.variants {
                let x = render(s.set, v, SR).unwrap();
                let name = format!("{}_{v}.wav", s.set.replace('/', "__"));
                let mut b = Vec::with_capacity(44 + x.len() * 2);
                let data = (x.len() * 2) as u32;
                b.extend_from_slice(b"RIFF");
                b.extend_from_slice(&(36 + data).to_le_bytes());
                b.extend_from_slice(b"WAVEfmt ");
                b.extend_from_slice(&16u32.to_le_bytes());
                b.extend_from_slice(&1u16.to_le_bytes());
                b.extend_from_slice(&1u16.to_le_bytes());
                b.extend_from_slice(&48_000u32.to_le_bytes());
                b.extend_from_slice(&96_000u32.to_le_bytes());
                b.extend_from_slice(&2u16.to_le_bytes());
                b.extend_from_slice(&16u16.to_le_bytes());
                b.extend_from_slice(b"data");
                b.extend_from_slice(&data.to_le_bytes());
                for v in &x {
                    b.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
                }
                std::fs::write(dir.join(name), b).unwrap();
            }
        }
    }
}
