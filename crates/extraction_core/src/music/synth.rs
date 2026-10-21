//! The instruments: everything heard as music in the valleys is played
//! here, note by note, from tunes in [`super::mml`]. Nothing is
//! recorded, so nothing needs a licence but the tunes' own (all public
//! domain: folk songs, hymns, old dances).
//!
//! Each instrument is a small physical caricature rather than a sample:
//!
//! - **Piano**: struck strings, two to a note and a hair apart (the slow
//!   beat of an upright that wants tuning), partials stretched sharp the
//!   way stiff strings are, the high ones dying first, a felt thump on
//!   the strike, dampers on release.
//! - **Harmonium** (the chapel's): a free reed, bright and buzzy, slow to
//!   speak, with the bellows' breath in it.
//! - **Organ**: flue pipes, a few ranks (8', 4', 2'), a chiff as each
//!   speaks, steady as a machine.
//! - **Accordion**: two reeds a few cents apart (the musette's shimmer).
//! - **Hum**: a person humming, lips closed, with a little vibrato.
//! - **Bell**: a church bell's inharmonic partials (the hum note under it,
//!   the minor third that makes a bell sound like a bell).
//! - **Guitar**: a plucked string (Karplus-Strong).
//! - **Bass**: a plucked double bass, round and short.
//! - **Music box**: a struck tine, glassy.
//! - **Beep**: a pure tone, for Morse.
//!
//! Rendering is mono (every player in the world is one point) at any rate;
//! the game renders at 24 kHz, plenty for a cassette and far more than a
//! radio carries.

use super::mml::Note;
use crate::noise::Rng;
use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Instrument {
    Piano,
    Harmonium,
    Organ,
    Accordion,
    Hum,
    Bell,
    Guitar,
    Bass,
    MusicBox,
    Beep,
}

/// A part of a tune: an instrument playing a line of notes.
#[derive(Clone, Debug)]
pub struct Part {
    pub instrument: Instrument,
    pub notes: Vec<Note>,
    /// Linear gain.
    pub gain: f32,
}

/// A sine oscillator that steps by rotation, so a partial costs a few
/// multiplies a sample rather than a `sin`.
#[derive(Clone, Copy)]
struct Phasor {
    re: f32,
    im: f32,
    c: f32,
    s: f32,
}

impl Phasor {
    fn new(freq: f32, sr: f32, phase: f32) -> Self {
        let w = TAU * freq / sr;
        Self {
            re: phase.cos(),
            im: phase.sin(),
            c: w.cos(),
            s: w.sin(),
        }
    }

    #[inline]
    fn next(&mut self) -> f32 {
        let re = self.re * self.c - self.im * self.s;
        let im = self.re * self.s + self.im * self.c;
        self.re = re;
        self.im = im;
        im
    }

    /// Keeps the amplitude at 1 (rounding drifts it over many steps).
    fn renorm(&mut self) {
        let m = (self.re * self.re + self.im * self.im).sqrt();
        if m > 0.0 {
            self.re /= m;
            self.im /= m;
        }
    }
}

/// How long an instrument rings after a note's written end (s).
fn tail(inst: Instrument, freq: f32) -> f32 {
    match inst {
        Instrument::Piano => 0.35,
        Instrument::Bell => 2.0 + 600.0 / freq.max(50.0),
        Instrument::Guitar | Instrument::MusicBox => 1.0,
        Instrument::Bass => 0.3,
        Instrument::Organ | Instrument::Harmonium | Instrument::Accordion => 0.12,
        Instrument::Hum => 0.15,
        Instrument::Beep => 0.005,
    }
}

/// One-pole lowpass coefficient for a cutoff.
fn lp_coef(cut: f32, sr: f32) -> f32 {
    1.0 - (-TAU * cut / sr).exp()
}

/// Renders one note of `inst` into `out` (added), starting at sample 0 of
/// `out`. `hold` is how long it's held (s).
fn note(inst: Instrument, freq: f32, vel: f32, hold: f32, sr: f32, rng: &mut Rng, out: &mut [f32]) {
    let n_hold = (hold * sr) as usize;
    let n = out.len();
    match inst {
        Instrument::Piano => {
            // Stiff-string partials, two strings a little apart.
            let b = 0.0004 * (freq / 261.0).powf(0.5);
            let count = ((5000.0 / freq) as usize).clamp(3, 14);
            let detune = 1.0 + rng.range(0.0006, 0.0018);
            let mut osc = Vec::with_capacity(count * 2);
            for k in 1..=count {
                let kf = k as f32;
                let f = freq * kf * (1.0 + b * kf * kf).sqrt();
                if f > sr * 0.45 {
                    break;
                }
                // Brighter when struck harder; the strike point (1/7 of
                // the string) thins the 7th.
                let amp = (1.0 / kf.powf(1.3 - 0.5 * vel)) * if k == 7 { 0.3 } else { 1.0 };
                // The higher, the shorter; low notes ring long.
                let t60 = (9.0 * (110.0 / freq).powf(0.6)).clamp(0.8, 14.0) / (1.0 + 0.35 * kf);
                let decay = (-6.9 / (t60 * sr)).exp();
                for s in [1.0, detune] {
                    osc.push((Phasor::new(f * s, sr, rng.f32() * TAU), amp * 0.5, decay));
                }
            }
            let damp = (-6.9 / (0.18 * sr)).exp();
            let mut env = 1.0f32;
            // The hammer's thump: a short burst of low noise.
            let thump_len = (0.012 * sr) as usize;
            let mut lp = 0.0f32;
            let k_lp = lp_coef(freq * 3.0, sr);
            for (i, o) in out.iter_mut().enumerate() {
                if i % 1024 == 0 {
                    for p in osc.iter_mut() {
                        p.0.renorm();
                    }
                }
                let mut s = 0.0;
                for (p, a, d) in osc.iter_mut() {
                    s += p.next() * *a;
                    *a *= *d;
                }
                if i >= n_hold {
                    env *= damp;
                }
                if i < thump_len {
                    lp += (rng.range(-1.0, 1.0) - lp) * k_lp;
                    s += lp * 0.6 * (1.0 - i as f32 / thump_len as f32);
                }
                // A soft attack (1.5 ms) so it clicks like felt, not a gate.
                let a = (i as f32 / (0.0015 * sr)).min(1.0);
                *o += s * env * a * vel * 0.35;
            }
        }
        Instrument::Harmonium | Instrument::Accordion => {
            // A band-limited saw (additive), reeds a few cents apart.
            let reeds: &[f32] = if inst == Instrument::Accordion {
                &[1.0, 1.0035, 0.9968]
            } else {
                &[1.0, 2.0]
            };
            let mut osc = Vec::new();
            for (r, &m) in reeds.iter().enumerate() {
                let f0 = freq * m;
                let count = ((sr * 0.4 / f0) as usize).clamp(1, 24);
                for k in 1..=count {
                    let kf = k as f32;
                    // Reeds are bright: 1/k, with odd partials a touch
                    // stronger for the harmonium's nasal edge.
                    let odd = if inst == Instrument::Harmonium && k % 2 == 1 {
                        1.2
                    } else {
                        1.0
                    };
                    let rank = if r == 0 { 1.0 } else { 0.45 };
                    osc.push((Phasor::new(f0 * kf, sr, rng.f32() * TAU), odd * rank / kf));
                }
            }
            let attack = if inst == Instrument::Harmonium {
                0.06
            } else {
                0.025
            };
            let release = tail(inst, freq);
            let trem = rng.range(4.5, 5.5);
            let mut tp = Phasor::new(trem, sr, 0.0);
            let mut breath = 0.0f32;
            let kb = lp_coef(1200.0, sr);
            for (i, o) in out.iter_mut().enumerate() {
                if i % 1024 == 0 {
                    for p in osc.iter_mut() {
                        p.0.renorm();
                    }
                    tp.renorm();
                }
                let t = i as f32 / sr;
                let env = if i < n_hold {
                    (t / attack).min(1.0)
                } else {
                    (-(t - hold) / (release / 4.6)).exp() * (hold / attack).min(1.0)
                };
                let mut s = 0.0;
                for (p, a) in osc.iter_mut() {
                    s += p.next() * *a;
                }
                let wobble = if inst == Instrument::Harmonium {
                    1.0 + 0.05 * tp.next()
                } else {
                    1.0 + 0.02 * tp.next()
                };
                breath += (rng.range(-1.0, 1.0) - breath) * kb;
                *o += (s * 0.18 + breath * 0.04) * env * wobble * vel;
            }
        }
        Instrument::Organ => {
            // 8', 4', 2' flue ranks, nearly sine with a 2nd and 3rd.
            let ranks = [(1.0, 1.0), (2.0, 0.5), (4.0, 0.25), (3.0, 0.2)];
            let mut osc: Vec<(Phasor, f32)> = Vec::new();
            for (m, a) in ranks {
                for (k, h) in [(1.0, 1.0), (2.0, 0.25), (3.0, 0.1)] {
                    let f = freq * m * k;
                    if f < sr * 0.45 {
                        osc.push((Phasor::new(f, sr, rng.f32() * TAU), a * h));
                    }
                }
            }
            let attack = 0.05;
            let release = tail(inst, freq);
            let chiff_len = (0.04 * sr) as usize;
            let mut lp = 0.0f32;
            let kc = lp_coef(freq * 4.0, sr);
            for (i, o) in out.iter_mut().enumerate() {
                if i % 1024 == 0 {
                    for p in osc.iter_mut() {
                        p.0.renorm();
                    }
                }
                let t = i as f32 / sr;
                let env = if i < n_hold {
                    (t / attack).min(1.0)
                } else {
                    (-(t - hold) / (release / 4.6)).exp()
                };
                let mut s = 0.0;
                for (p, a) in osc.iter_mut() {
                    s += p.next() * *a;
                }
                if i < chiff_len {
                    lp += (rng.range(-1.0, 1.0) - lp) * kc;
                    s += lp * 0.5 * (1.0 - i as f32 / chiff_len as f32);
                }
                *o += s * 0.2 * env * vel;
            }
        }
        Instrument::Hum => {
            // A glottal buzz (falling partials) through "mm": a strong
            // low formant, the rest damped by closed lips.
            let count = ((3000.0 / freq) as usize).clamp(2, 20);
            let formants = [(250.0, 1.0, 120.0), (2200.0, 0.06, 300.0)];
            let mut osc: Vec<(f32, f32, f32)> = Vec::new(); // (mult, amp, phase)
            for k in 1..=count {
                let f = freq * k as f32;
                let mut a = 0.0;
                for (fc, g, bw) in formants {
                    let x = (f - fc) / bw;
                    a += g / (1.0 + x * x);
                }
                a = a / (k as f32).powf(0.6) + 0.02 / k as f32;
                osc.push((k as f32, a, rng.f32() * TAU));
            }
            let vib_rate = rng.range(5.0, 6.0);
            let attack = 0.08;
            let release = tail(inst, freq);
            let mut phase = 0.0f32;
            let mut vib = 0.0f32;
            for (i, o) in out.iter_mut().enumerate() {
                let t = i as f32 / sr;
                // Vibrato that grows in after the note starts.
                vib = (vib + vib_rate / sr).fract();
                let depth = 0.006 * ((t - 0.25) / 0.4).clamp(0.0, 1.0);
                let f = freq * (1.0 + depth * (vib * TAU).sin());
                phase = (phase + f / sr).fract();
                let env = if i < n_hold {
                    (t / attack).min(1.0)
                } else {
                    (-(t - hold) / (release / 4.6)).exp()
                };
                let mut s = 0.0;
                for (k, a, p0) in &osc {
                    s += a * (phase * TAU * k + p0).sin();
                }
                *o += s * 0.5 * env * vel;
            }
        }
        Instrument::Bell => {
            // Partials of a tuned church bell, relative to the strike note:
            // hum, prime, tierce, quint, nominal, and some upper ones.
            let parts = [
                (0.5, 0.5, 1.0),
                (1.0, 0.7, 0.55),
                (1.19, 0.5, 0.45),
                (1.5, 0.3, 0.35),
                (2.0, 0.6, 0.3),
                (2.51, 0.25, 0.2),
                (3.01, 0.2, 0.15),
                (4.07, 0.12, 0.1),
            ];
            let ring = tail(inst, freq);
            let mut osc: Vec<(Phasor, f32, f32)> = Vec::new();
            for (m, a, life) in parts {
                let f = freq * m * (1.0 + rng.range(-0.002, 0.002));
                if f < sr * 0.45 {
                    osc.push((
                        Phasor::new(f, sr, rng.f32() * TAU),
                        a,
                        (-6.9 / (ring * life * sr)).exp(),
                    ));
                }
            }
            for (i, o) in out.iter_mut().enumerate() {
                if i % 1024 == 0 {
                    for p in osc.iter_mut() {
                        p.0.renorm();
                    }
                }
                let mut s = 0.0;
                for (p, a, d) in osc.iter_mut() {
                    s += p.next() * *a;
                    *a *= *d;
                }
                let a = (i as f32 / (0.002 * sr)).min(1.0);
                *o += s * 0.3 * a * vel;
            }
        }
        Instrument::Guitar | Instrument::Bass => {
            // Karplus-Strong: a noise burst round a damped delay line.
            let period = (sr / freq).max(2.0);
            // Averaging y[n-len] and y[n-len+1] makes the loop len - 0.5 long.
            let len = ((period + 0.5).round() as usize).max(3);
            let mut line: Vec<f32> = (0..len + 2).map(|_| rng.range(-1.0, 1.0)).collect();
            // Soften the burst for the bass (a thumb, not a pick).
            let passes = if inst == Instrument::Bass { 6 } else { 1 };
            for _ in 0..passes {
                for j in 1..line.len() {
                    line[j] = 0.5 * (line[j] + line[j - 1]);
                }
            }
            let loss = if inst == Instrument::Bass {
                0.994
            } else {
                0.997
            };
            let mut idx = 0usize;
            let damp = (-6.9 / (0.08 * sr)).exp();
            let mut env = 1.0f32;
            let ll = line.len();
            for (i, o) in out.iter_mut().enumerate() {
                let a = line[idx % ll];
                let b = line[(idx + 1) % ll];
                let y = a;
                let new = loss * 0.5 * (a + b);
                line[(idx + len) % ll] = new;
                idx += 1;
                if i >= n_hold {
                    env *= damp;
                }
                let g = if inst == Instrument::Bass { 0.5 } else { 0.3 };
                *o += y * g * env * vel;
            }
        }
        Instrument::MusicBox => {
            let parts = [(1.0, 1.0, 1.0), (2.76, 0.4, 0.4), (5.4, 0.2, 0.2)];
            let mut osc: Vec<(Phasor, f32, f32)> = parts
                .iter()
                .filter(|(m, ..)| freq * m < sr * 0.45)
                .map(|&(m, a, life)| {
                    (
                        Phasor::new(freq * m, sr, 0.0),
                        a,
                        (-6.9 / (1.6 * life * sr)).exp(),
                    )
                })
                .collect();
            for (i, o) in out.iter_mut().enumerate() {
                if i % 1024 == 0 {
                    for p in osc.iter_mut() {
                        p.0.renorm();
                    }
                }
                let mut s = 0.0;
                for (p, a, d) in osc.iter_mut() {
                    s += p.next() * *a;
                    *a *= *d;
                }
                *o += s * 0.25 * vel;
            }
        }
        Instrument::Beep => {
            let mut p = Phasor::new(freq, sr, 0.0);
            // 4 ms edges: a keyed tone without key clicks.
            let edge = 0.004 * sr;
            for (i, o) in out.iter_mut().enumerate() {
                if i % 1024 == 0 {
                    p.renorm();
                }
                let fi = i as f32;
                let env = (fi / edge)
                    .min(1.0)
                    .min(((n_hold as f32 - fi) / edge).clamp(0.0, 1.0));
                *o += p.next() * env * vel * 0.5;
            }
        }
    }
    let _ = n;
}

/// Renders parts to mono at `sr`, `seconds` long (0: as long as the
/// music plus its ring-out).
pub fn render(parts: &[Part], sr: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed ^ 0x51A7);
    let end = parts
        .iter()
        .flat_map(|p| {
            p.notes
                .iter()
                .map(|n| n.start + n.len + tail(p.instrument, n.freq()))
        })
        .fold(0.0f32, f32::max);
    let mut out = vec![0.0f32; (end * sr) as usize + 1];
    for p in parts {
        for n in &p.notes {
            let start = (n.start * sr) as usize;
            if start >= out.len() {
                continue;
            }
            let ring = tail(p.instrument, n.freq());
            // Plucked and struck things ring out on their own.
            let span = match p.instrument {
                Instrument::Piano
                | Instrument::Bell
                | Instrument::Guitar
                | Instrument::Bass
                | Instrument::MusicBox => n.len + ring,
                _ => n.len + ring,
            };
            let stop = (start + (span * sr) as usize).min(out.len());
            note(
                p.instrument,
                n.freq(),
                n.vel * p.gain,
                n.len,
                sr,
                &mut rng,
                &mut out[start..stop],
            );
        }
    }
    out
}

/// Scales so the loudest sample is at `peak`.
pub fn normalize(x: &mut [f32], peak: f32) {
    let m = x.iter().fold(0.0f32, |a, s| a.max(s.abs()));
    if m > 0.0 {
        let g = peak / m;
        for s in x.iter_mut() {
            *s *= g;
        }
    }
}

/// RMS over the whole buffer.
pub fn rms(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::mml::parse;

    const SR: f32 = 24000.0;

    /// The strongest frequency in a stretch, by a plain DFT scan.
    fn peak_freq(x: &[f32], lo: f32, hi: f32) -> f32 {
        let mut best = (0.0, lo);
        let mut f = lo;
        while f < hi {
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for (i, s) in x.iter().enumerate() {
                let p = TAU * f * i as f32 / SR;
                re += s * p.cos();
                im += s * p.sin();
            }
            let m = re * re + im * im;
            if m > best.0 {
                best = (m, f);
            }
            f += 1.0;
        }
        best.1
    }

    /// Every instrument sounds at the pitch written (a bell's strike note
    /// is its prime; its hum an octave under is allowed to win).
    #[test]
    fn every_instrument_plays_in_tune() {
        use Instrument::*;
        for inst in [
            Piano, Harmonium, Organ, Accordion, Hum, Guitar, Bass, MusicBox, Beep,
        ] {
            let notes = parse("t60 o4 a2", 1).unwrap();
            let x = render(
                &[Part {
                    instrument: inst,
                    notes,
                    gain: 1.0,
                }],
                SR,
                1,
            );
            assert!(rms(&x) > 0.005, "{inst:?} is silent");
            assert!(x.iter().all(|s| s.is_finite()), "{inst:?}");
            let window = &x[(0.2 * SR) as usize..(0.6 * SR) as usize];
            let f = peak_freq(window, 380.0, 500.0);
            assert!((f - 440.0).abs() < 6.0, "{inst:?} at {f} Hz");
        }
    }

    /// A struck or plucked note dies away; a reed or pipe holds.
    #[test]
    fn struck_things_decay_and_blown_things_hold() {
        let held = |inst| {
            let x = render(
                &[Part {
                    instrument: inst,
                    notes: parse("t60 o4 c1", 2).unwrap(),
                    gain: 1.0,
                }],
                SR,
                2,
            );
            let a = rms(&x[(0.3 * SR) as usize..(2.0 * SR) as usize]);
            let b = rms(&x[(2.0 * SR) as usize..(3.7 * SR) as usize]);
            b / a
        };
        assert!(held(Instrument::Piano) < 0.6);
        assert!(held(Instrument::Guitar) < 0.6);
        assert!(held(Instrument::Organ) > 0.8);
        assert!(held(Instrument::Accordion) > 0.8);
    }

    /// The same tune, the same seed: the same samples.
    #[test]
    fn deterministic() {
        let p = || Part {
            instrument: Instrument::Piano,
            notes: parse("~c ~e ~g", 3).unwrap(),
            gain: 1.0,
        };
        assert_eq!(render(&[p()], SR, 9), render(&[p()], SR, 9));
    }
}
