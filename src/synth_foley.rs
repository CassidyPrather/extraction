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
    StandIn {
        set: "body/cough",
        variants: 4,
        render: cough,
    },
    StandIn {
        set: "wildlife/canary",
        variants: 5,
        render: canary,
    },
    StandIn {
        set: "anomaly/frost",
        variants: 2,
        render: frost,
    },
    StandIn {
        set: "physical/ice_crack",
        variants: 4,
        render: ice_crack,
    },
    StandIn {
        set: "device/notice",
        variants: 2,
        render: notice,
    },
    StandIn {
        set: "device/notice_hard",
        variants: 2,
        render: notice_hard,
    },
    StandIn {
        set: "signal/squad_pin",
        variants: 2,
        render: squad_pin,
    },
    StandIn {
        set: "signal/squad_flank",
        variants: 2,
        render: squad_flank,
    },
    StandIn {
        set: "signal/squad_cutoff",
        variants: 2,
        render: squad_cutoff,
    },
    StandIn {
        set: "signal/squad_spot",
        variants: 2,
        render: squad_spot,
    },
    StandIn {
        set: "signal/squad_call",
        variants: 2,
        render: squad_call,
    },
    StandIn {
        set: "signal/filed",
        variants: 2,
        render: filed,
    },
    StandIn {
        set: "wildlife/wings",
        variants: 4,
        render: wings,
    },
    StandIn {
        set: "machines/chute_hiss",
        variants: 3,
        render: chute_hiss,
    },
    StandIn {
        set: "alarm/klaxon",
        variants: 2,
        render: klaxon,
    },
    StandIn {
        set: "drag/metal",
        variants: 3,
        render: drag_metal,
    },
    StandIn {
        set: "drag/wood",
        variants: 3,
        render: drag_wood,
    },
    StandIn {
        set: "drag/roll",
        variants: 3,
        render: drag_roll,
    },
    StandIn {
        set: "drag/body",
        variants: 3,
        render: drag_body,
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

/// A cough in bad air: one to three hard barks of breath, each a rush of
/// noise through the throat's formants with a rough voiced edge, falling
/// off as the lungs empty, 0.3-1.1 s.
fn cough(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let fits = 1 + (rng.f32() * 2.99) as usize;
    let mut out = Vec::new();
    for k in 0..fits {
        let p = 160.0 * jitter(rng, 0.2);
        let secs = 0.22 * jitter(rng, 0.2);
        let open = 450.0 * jitter(rng, 0.15);
        let mut bark = call(
            rng,
            sr,
            secs,
            |x| p * (1.0 - 0.3 * x),
            |x| (x / 0.04).min(1.0) * (1.0 - x).max(0.0).powf(1.6),
            &[(open, 2.0, 1.0), (1400.0, 2.5, 0.7), (2700.0, 3.0, 0.35)],
            8,
            2.5,
        );
        // Each one weaker: less breath left.
        let g = 1.0 - 0.25 * k as f32;
        bark.iter_mut().for_each(|v| *v *= g);
        out.extend(bark);
        out.extend(std::iter::repeat_n(
            0.0,
            (0.09 * jitter(rng, 0.3) * sr) as usize,
        ));
    }
    out
}

/// A canary's alarm: a run of thin, quick "tsip" notes falling from
/// 3.5-4.5 kHz, three to seven of them, each 40 ms.
fn canary(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let notes = 3 + (rng.f32() * 4.99) as usize;
    let f0 = 4000.0 * jitter(rng, 0.12);
    let mut out = Vec::new();
    for _ in 0..notes {
        let len = (0.04 * jitter(rng, 0.2) * sr) as usize;
        let f = f0 * jitter(rng, 0.05);
        let mut ph = 0.0f32;
        for i in 0..len {
            let x = i as f32 / len as f32;
            ph = (ph + f * (1.0 - 0.3 * x) / sr).fract();
            let env = (x / 0.15).min(1.0) * (1.0 - x).powf(1.2);
            out.push(((TAU * ph).sin() + 0.2 * (2.0 * TAU * ph).sin()) * env);
        }
        out.extend(std::iter::repeat_n(
            0.0,
            (0.03 * jitter(rng, 0.3) * sr) as usize,
        ));
    }
    out
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

/// How long a drag loop is, s, and how much of its tail is faded into
/// its head so it loops without a seam.
const LOOP_SECS: f32 = 2.0;
const LOOP_FADE: f32 = 0.3;

/// `LOOP_SECS` of `sample(rng, t)` that loops without a click: rendered
/// long, its tail crossfaded (equal power) into its head.
fn seamless(rng: &mut Rng, sr: f32, mut sample: impl FnMut(&mut Rng, f32) -> f32) -> Vec<f32> {
    let n = (LOOP_SECS * sr) as usize;
    let fade = (LOOP_FADE * sr) as usize;
    let long: Vec<f32> = (0..n + fade).map(|i| sample(rng, i as f32 / sr)).collect();
    let mut out = long[..n].to_vec();
    for i in 0..fade {
        let a = (i as f32 + 0.5) / fade as f32 * std::f32::consts::FRAC_PI_2;
        out[i] = out[i] * a.sin() + long[n + i] * a.cos();
    }
    out
}

/// Stick and slip: a rough, uneven grit, `rate` catches a second, each a
/// quick swell that lets go, in 0.3..1.
fn grit(rate: f32, sr: f32) -> impl FnMut(&mut Rng) -> f32 {
    let mut level = 0.6f32;
    let mut next = 0usize;
    let mut i = 0usize;
    move |rng: &mut Rng| {
        if i >= next {
            level = 0.3 + 0.7 * rng.f32();
            next = i + ((sr / rate) * jitter(rng, 0.6)) as usize;
        }
        i += 1;
        let k = (next - i) as f32 / (sr / rate);
        level * (0.5 + 0.5 * k.min(1.0))
    }
}

/// A sheet of metal (a car door, a panel, a robot's hull) dragged over
/// ground: a hiss through the sheet's own ringing modes, catching and
/// slipping, with the grind of what's under it, 2 s, looped.
fn drag_metal(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let mut modes: Vec<(Bp, f32)> = [1100.0, 2600.0, 4200.0, 6300.0]
        .iter()
        .zip([1.0, 0.45, 0.2, 0.08])
        .map(|(&f, w)| (Bp::new(f * jitter(rng, 0.15), 18.0, sr), w))
        .collect();
    let mut grind = Bp::new(320.0 * jitter(rng, 0.2), 0.8, sr);
    let mut catch = grit(35.0, sr);
    seamless(rng, sr, |rng, _| {
        let s = noise(rng);
        let ring: f32 = modes.iter_mut().map(|(m, w)| m.run(s) * *w).sum();
        (2.5 * ring + 0.6 * grind.run(s)) * catch(rng)
    })
}

/// Wood (a crate, a plank, a door) dragged: a dry, hollow chatter, 2 s,
/// looped.
fn drag_wood(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let mut body = Bp::new(650.0 * jitter(rng, 0.2), 1.6, sr);
    let mut edge = Bp::new(2100.0 * jitter(rng, 0.2), 2.0, sr);
    let mut catch = grit(22.0, sr);
    seamless(rng, sr, |rng, _| {
        let s = noise(rng);
        let c = catch(rng);
        body.run(s) * c * c + 0.4 * edge.run(s) * c
    })
}

/// A wheel rolled over ground: a low rumble with the tread's beat and
/// the odd stone crackling under it, 2 s, looped.
fn drag_roll(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let mut low = Bp::new(110.0 * jitter(rng, 0.15), 0.7, sr);
    let mut mid = Bp::new(380.0, 1.0, sr);
    let tread = 9.0 * jitter(rng, 0.2);
    let mut crackle = Bp::new(3000.0, 1.5, sr);
    seamless(rng, sr, |rng, t| {
        let s = noise(rng);
        // A whole number of beats per loop so the tread doesn't jump.
        let beats = (tread * LOOP_SECS).round() / LOOP_SECS;
        let beat = 0.7 + 0.3 * (TAU * beats * t).sin().abs();
        let stone = if rng.f32() < 25.0 / sr { 1.0 } else { 0.0 };
        (2.0 * low.run(s) + 0.5 * mid.run(s)) * beat + 3.0 * crackle.run(stone)
    })
}

/// A body (a person's, a robot's soft parts) dragged: cloth and limbs
/// scuffing the ground in uneven tugs, 2 s, looped.
fn drag_body(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let mut cloth = Bp::new(1400.0 * jitter(rng, 0.2), 0.9, sr);
    let mut thud = Bp::new(180.0, 0.8, sr);
    let tugs = (1.5 * jitter(rng, 0.2) * LOOP_SECS).round() / LOOP_SECS;
    seamless(rng, sr, |rng, t| {
        let s = noise(rng);
        let tug = (TAU * tugs * t).sin().max(0.0).powf(1.5);
        (cloth.run(s) * 0.8 + 0.6 * thud.run(s)) * (0.25 + 0.75 * tug)
    })
}

/// An electric klaxon's blast: a steel diaphragm hammered by a motor's
/// cam, a raw buzz near 420 Hz (the cam's rate riding on it) through the
/// horn's flare, rising a little as the motor comes up to speed, 1.1 s.
fn klaxon(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let secs = 1.1;
    let f0 = 420.0 * jitter(rng, 0.04);
    let cam = 75.0 * jitter(rng, 0.1);
    let mut horn = [
        (Bp::new(800.0, 3.0, sr), 1.0),
        (Bp::new(1600.0, 4.0, sr), 0.7),
        (Bp::new(2500.0, 5.0, sr), 0.4),
    ];
    let n = (secs * sr) as usize;
    let mut ph = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let f = f0 * (0.85 + 0.15 * (t / 0.15).min(1.0));
            ph = (ph + f / sr).fract();
            // A clipped saw: the diaphragm slapping its stop.
            let saw = (2.0 * ph - 1.0) * 3.0;
            let buzz = saw.clamp(-1.0, 1.0) * (0.7 + 0.3 * (TAU * cam * t).sin());
            let src = buzz + 0.05 * noise(rng);
            let env = (t / 0.03).min(1.0) * ((secs - t) / 0.12).clamp(0.0, 1.0);
            let shaped: f32 = horn.iter_mut().map(|(b, w)| b.run(src) * *w).sum();
            (shaped + 0.15 * buzz) * env
        })
        .collect()
}

/// A frost's still cold, heard: a thin, high crystal tinkle (sparse pings
/// at 3.5-8 kHz, each ringing off) with the ice creaking under it now and
/// then (a slow, low groan of filtered noise), 2 s, looped.
fn frost(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let n = (LOOP_SECS * sr) as usize;
    let mut x = vec![0.0f32; n];
    // The tinkle: a ping every 60-220 ms, wrapping round the loop.
    let mut t = 0.0f32;
    while t < LOOP_SECS {
        let f = 3_500.0 + rng.f32() * 4_500.0;
        let a = 0.3 + 0.7 * rng.f32();
        let len = (0.25 * sr) as usize;
        let start = (t * sr) as usize;
        for i in 0..len {
            let s = i as f32 / sr;
            x[(start + i) % n] += a * (TAU * f * s).sin() * (-s * 18.0).exp();
        }
        t += 0.06 + rng.f32() * 0.16;
    }
    // The creak: once a loop, a second of a slow groan.
    let mut groan = Bp::new(180.0 * jitter(rng, 0.2), 6.0, sr);
    let at = (rng.f32() * LOOP_SECS * sr) as usize;
    let len = sr as usize;
    for i in 0..len {
        let e = (std::f32::consts::PI * i as f32 / len as f32).sin();
        x[(at + i) % n] += 1.5 * groan.run(noise(rng)) * e;
    }
    x
}

/// Ice cracking: a hard, bright snap and its short ring, 0.4 s.
fn ice_crack(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let n = (0.4 * sr) as usize;
    let mut ring = Bp::new(2_400.0 * jitter(rng, 0.2), 12.0, sr);
    let mut body = Bp::new(600.0 * jitter(rng, 0.2), 1.5, sr);
    (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            let snap = noise(rng) * (-t * 90.0).exp();
            snap * 0.6 + ring.run(snap) * 3.0 + body.run(snap) * (-t * 25.0).exp()
        })
        .collect()
}

/// A big bird's wings beating: each downstroke a soft whump of air
/// (noise through the body of the wing, a few hundred Hz) with a feathery
/// hiss over it, five to eight strokes at five or six a second, the first
/// the hardest, as a rook or a raven labours up (`wildlife::Flushed`) or
/// brakes to land.
fn wings(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let strokes = 5 + (rng.f32() * 4.0) as usize;
    let rate = 5.0 + rng.f32() * 1.5;
    let len = (strokes as f32 / rate + 0.25) * sr;
    let mut air = Bp::new(420.0 * jitter(rng, 0.15), 1.2, sr);
    let mut feather = Bp::new(3_200.0 * jitter(rng, 0.2), 0.8, sr);
    (0..len as usize)
        .map(|i| {
            let t = i as f32 / sr;
            let k = (t * rate) as usize;
            let into = t * rate - k as f32;
            // A stroke: fast in, slower out; the later ones lighter.
            let stroke = if k < strokes {
                (into / 0.15).min(1.0)
                    * (-(into - 0.15).max(0.0) * 9.0).exp()
                    * (0.55f32).powf(k as f32 * 0.35)
            } else {
                0.0
            };
            let n = noise(rng);
            air.run(n) * stroke * 4.0 + feather.run(n) * stroke * 0.8
        })
        .collect()
}

/// A pneumatic chute charging: air bleeding through a valve, a breathy
/// hiss quick to open, held, then bled off, a thin whistle coming up in
/// it as the pressure builds. Low for a hiss, so a shower doesn't wash it
/// out entirely.
fn chute_hiss(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let len = (0.75 + rng.f32() * 0.25) * sr;
    let hold = len / sr - 0.3;
    let f = 1_800.0 * jitter(rng, 0.15);
    let mut body = Bp::new(f, 0.8, sr);
    let mut whistle = Bp::new(f * 1.9, 12.0, sr);
    let mut low = Bp::new(500.0, 0.7, sr);
    (0..len as usize)
        .map(|i| {
            let t = i as f32 / sr;
            let env = (t / 0.03).min(1.0) * (1.0 - ((t - hold) / 0.3).clamp(0.0, 1.0)).powi(2);
            let n = noise(rng);
            (body.run(n) * 1.4 + whistle.run(n) * 0.9 * (t / hold).min(1.0) + low.run(n) * 1.0)
                * env
        })
        .collect()
}

/// Bright, clipped notes (a square-ish tone, a click of onset, quick
/// decay), one after another: (pitch Hz, length s, gap s).
fn chirps(rng: &mut Rng, sr: f32, notes: &[(f32, f32, f32)]) -> Vec<f32> {
    let mut out = Vec::new();
    for &(f, len, gap) in notes {
        let f = f * jitter(rng, 0.02);
        let n = (len * sr) as usize;
        let mut ph = 0.0f32;
        for i in 0..n {
            let t = i as f32 / sr;
            ph = (ph + f / sr).fract();
            let tone =
                (TAU * ph).sin() + 0.3 * (3.0 * TAU * ph).sin() + 0.15 * (5.0 * TAU * ph).sin();
            let env = (t / 0.004).min(1.0) * (1.0 - t / len).max(0.0).powf(0.6);
            out.push(tone * env);
        }
        out.extend(std::iter::repeat_n(0.0, (gap * sr) as usize));
    }
    out
}

/// A machine noticing something you set down: two short notes stepping
/// up, a question (`robots::devices::DeviceNoticed`, investigating).
fn notice(rng: &mut Rng, sr: f32) -> Vec<f32> {
    chirps(rng, sr, &[(1_400.0, 0.07, 0.05), (2_100.0, 0.09, 0.0)])
}

/// Machine to machine, a plan on you (`robots::squad::SquadCall`), one
/// pattern a part, none of them rising like a question
/// (`device/notice`). Holding you: two level, low, long notes.
fn squad_pin(rng: &mut Rng, sr: f32) -> Vec<f32> {
    chirps(rng, sr, &[(1_000.0, 0.12, 0.06), (1_000.0, 0.16, 0.0)])
}

/// Going round to your side: up and back down, quick.
fn squad_flank(rng: &mut Rng, sr: f32) -> Vec<f32> {
    chirps(
        rng,
        sr,
        &[
            (1_300.0, 0.06, 0.03),
            (1_900.0, 0.06, 0.03),
            (1_300.0, 0.09, 0.0),
        ],
    )
}

/// Making for your way out: a sharp fall.
fn squad_cutoff(rng: &mut Rng, sr: f32) -> Vec<f32> {
    chirps(rng, sr, &[(2_400.0, 0.06, 0.02), (1_200.0, 0.13, 0.0)])
}

/// A drone lighting the way in: a high, quick trill.
fn squad_spot(rng: &mut Rng, sr: f32) -> Vec<f32> {
    let notes: Vec<_> = (0..6)
        .map(|k| (if k % 2 == 0 { 2_800.0 } else { 3_300.0 }, 0.035, 0.015))
        .collect();
    chirps(rng, sr, &notes)
}

/// A relay calling it in: long, short, long, like a ring.
fn squad_call(rng: &mut Rng, sr: f32) -> Vec<f32> {
    chirps(
        rng,
        sr,
        &[
            (1_600.0, 0.14, 0.05),
            (1_600.0, 0.05, 0.05),
            (1_600.0, 0.14, 0.0),
        ],
    )
}

/// A relay filing you (`robots::recall::Filed`): three falling notes,
/// "noted".
fn filed(rng: &mut Rng, sr: f32) -> Vec<f32> {
    chirps(
        rng,
        sr,
        &[
            (2_000.0, 0.05, 0.04),
            (1_600.0, 0.05, 0.04),
            (1_250.0, 0.09, 0.0),
        ],
    )
}

/// The same, hunting it: three, quicker, harder and higher.
fn notice_hard(rng: &mut Rng, sr: f32) -> Vec<f32> {
    chirps(
        rng,
        sr,
        &[
            (1_700.0, 0.05, 0.03),
            (2_300.0, 0.05, 0.03),
            (3_100.0, 0.08, 0.0),
        ],
    )
}
