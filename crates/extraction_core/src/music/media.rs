//! What a medium does to a sound on its way to you. Everything is mono and
//! in place, at the buffer's rate.
//!
//! - [`tape`]: a cheap cassette. Wow (the capstan's slow wander) and
//!   flutter, the treble rolled off, a little saturation, hiss, and with
//!   wear, dropouts where the oxide has gone.
//! - [`radio`]: a broadcast on a small set. The band an AM receiver
//!   passes (300 Hz to 3 kHz), squashed loud, fading slowly in and out
//!   the way a far signal does.
//! - [`gramophone`]: a shellac record on a horn. Even narrower and
//!   honkier, surface hiss, a crackle of dust, the thump of a warped disc
//!   once a revolution.
//! - [`static_bed`]: what a radio hears between stations.

use crate::noise::Rng;
use std::f32::consts::TAU;

/// One-pole lowpass, in place.
pub fn lowpass(x: &mut [f32], sr: f32, cut: f32) {
    let k = 1.0 - (-TAU * cut / sr).exp();
    let mut y = 0.0;
    for s in x.iter_mut() {
        y += (*s - y) * k;
        *s = y;
    }
}

/// One-pole highpass, in place.
pub fn highpass(x: &mut [f32], sr: f32, cut: f32) {
    let k = 1.0 - (-TAU * cut / sr).exp();
    let mut lp = 0.0;
    for s in x.iter_mut() {
        lp += (*s - lp) * k;
        *s -= lp;
    }
}

/// A band, two poles each side.
pub fn band(x: &mut [f32], sr: f32, lo: f32, hi: f32) {
    highpass(x, sr, lo);
    highpass(x, sr, lo);
    lowpass(x, sr, hi);
    lowpass(x, sr, hi);
}

/// Soft saturation: unity for small signals, rounding the peaks.
fn saturate(x: &mut [f32], drive: f32) {
    let norm = drive.tanh();
    for s in x.iter_mut() {
        *s = (*s * drive).tanh() / norm;
    }
}

/// Pink-ish noise (a few one-pole bands summed), unit-ish RMS.
fn pink(rng: &mut Rng, state: &mut [f32; 3]) -> f32 {
    let w = rng.range(-1.0, 1.0);
    state[0] = 0.997 * state[0] + 0.029 * w;
    state[1] = 0.985 * state[1] + 0.032 * w;
    state[2] = 0.950 * state[2] + 0.048 * w;
    (state[0] + state[1] + state[2] + 0.05 * w) * 2.5
}

/// Reads `x` at `pos` (samples), linearly.
fn at(x: &[f32], pos: f32) -> f32 {
    let i = pos.floor();
    let f = pos - i;
    let i = i as isize;
    let get = |j: isize| {
        if j < 0 || j as usize >= x.len() {
            0.0
        } else {
            x[j as usize]
        }
    };
    get(i) * (1.0 - f) + get(i + 1) * f
}

/// Wow and flutter: the playback speed wandering. `depth` 1 is a tired
/// portable (about 0.4 % wow, 0.08 % flutter).
pub fn wow(x: &mut Vec<f32>, sr: f32, seed: u64, depth: f32) {
    let mut rng = Rng::new(seed ^ 0x3003);
    let wow_rate = rng.range(0.4, 0.7);
    let wow_phase = rng.f32() * TAU;
    let flutter_rate = rng.range(6.0, 9.0);
    let src = x.clone();
    // Delay modulation: d(t) = A sin(wt) gives a speed change of A w, so
    // A = speed deviation / w (in seconds).
    let a_wow = 0.004 * depth / (TAU * wow_rate) * sr;
    let a_flutter = 0.0008 * depth / (TAU * flutter_rate) * sr;
    let base = a_wow + a_flutter + 2.0;
    for (i, s) in x.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let d = base
            + a_wow * (TAU * wow_rate * t + wow_phase).sin()
            + a_flutter * (TAU * flutter_rate * t).sin();
        *s = at(&src, i as f32 - d);
    }
}

/// A cassette. `wear` 0 is a fresh tape, 1 one played to death and left
/// in a hot car.
pub fn tape(x: &mut Vec<f32>, sr: f32, seed: u64, wear: f32) {
    let wear = wear.clamp(0.0, 1.0);
    wow(x, sr, seed, 0.6 + 0.8 * wear);
    // The treble goes first.
    lowpass(x, sr, 9000.0 - 4500.0 * wear);
    lowpass(x, sr, 11000.0 - 4000.0 * wear);
    highpass(x, sr, 50.0);
    saturate(x, 1.4 + wear);
    let level = rms(x).max(1e-4);
    let mut rng = Rng::new(seed ^ 0x7A9E);
    let mut st = [0.0; 3];
    // Hiss, treble-heavy: white minus a little of the pink.
    let hiss = level * (0.02 + 0.04 * wear);
    let mut dropout = 1.0f32;
    let mut target = 1.0f32;
    for (i, s) in x.iter_mut().enumerate() {
        // Dropouts: a few a minute on a worn tape, a few tens of ms each.
        if i % 256 == 0 {
            target = if rng.chance(wear * wear * 256.0 / (sr * 12.0)) {
                rng.range(0.2, 0.6)
            } else if target < 1.0 && rng.chance(0.15) {
                1.0
            } else {
                target
            };
        }
        dropout += (target - dropout) * 0.003;
        let w = rng.range(-1.0, 1.0);
        let p = pink(&mut rng, &mut st);
        *s = *s * dropout + hiss * (w - 0.3 * p);
    }
}

/// An AM broadcast through a small set. `fade` 0 holds steady; 1 swings
/// the way a skywave does (the level wandering by 10 dB over tens of
/// seconds).
pub fn radio(x: &mut [f32], sr: f32, seed: u64, fade: f32) {
    band(x, sr, 300.0, 3000.0);
    // Broadcast compression, crudely: lift, then round off.
    let level = rms(x).max(1e-4);
    for s in x.iter_mut() {
        *s /= level * 4.0;
    }
    saturate(x, 2.5);
    let mut rng = Rng::new(seed ^ 0xFADE);
    let rates = [rng.range(0.02, 0.05), rng.range(0.07, 0.13)];
    let phases = [rng.f32() * TAU, rng.f32() * TAU];
    for (i, s) in x.iter_mut().enumerate() {
        let t = i as f32 / sr;
        let f = 0.5
            * ((TAU * rates[0] * t + phases[0]).sin()
                + (TAU * rates[1] * t + phases[1]).sin() * 0.5);
        // In dB, mapped down: -10 dB at the trough.
        let db = -5.0 * fade * (1.0 - f);
        *s *= 10f32.powf(db / 20.0);
    }
}

/// A 78 on a wind-up gramophone.
pub fn gramophone(x: &mut Vec<f32>, sr: f32, seed: u64) {
    wow(x, sr, seed, 1.2);
    band(x, sr, 250.0, 4000.0);
    // The horn's honk: a resonance about 1 kHz, from a short comb.
    let d = (sr / 1000.0) as usize;
    for i in (d..x.len()).rev() {
        x[i] += 0.35 * x[i - d];
    }
    saturate(x, 1.8);
    let level = rms(x).max(1e-4);
    let mut rng = Rng::new(seed ^ 0x78);
    let mut st = [0.0; 3];
    let rev = sr * 60.0 / 78.0;
    for (i, s) in x.iter_mut().enumerate() {
        let p = pink(&mut rng, &mut st);
        let mut noise = 0.05 * p;
        // Dust: a click every so often, a bigger one now and then.
        if rng.chance(18.0 / sr) {
            noise += rng.range(-1.0, 1.0) * if rng.chance(0.1) { 2.0 } else { 0.7 };
        }
        // The warp: a soft thump once a turn.
        let r = (i as f32 % rev) / rev;
        if r < 0.01 {
            noise += 0.3 * (r / 0.01 * std::f32::consts::PI).sin();
        }
        *s += level * noise;
    }
}

/// What a radio hears between stations: a hiss with a slow swell, now and
/// then a crackle, and a whistle where two carriers beat. `len` samples.
pub fn static_bed(len: usize, sr: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed ^ 0x57A7);
    let mut st = [0.0; 3];
    let mut out = Vec::with_capacity(len);
    let swell = rng.range(0.1, 0.2);
    let mut whistle_phase = 0.0f32;
    for i in 0..len {
        let t = i as f32 / sr;
        let w = rng.range(-1.0, 1.0);
        let p = pink(&mut rng, &mut st);
        let mut s = 0.35 * w + 0.4 * p;
        s *= 0.8 + 0.2 * (TAU * swell * t).sin();
        if rng.chance(6.0 / sr) {
            s += rng.range(-2.0, 2.0);
        }
        // A heterodyne whistle drifting 600-1400 Hz, faint.
        let wf = 1000.0 + 400.0 * (TAU * 0.031 * t).sin();
        whistle_phase = (whistle_phase + wf / sr).fract();
        s += 0.04 * (whistle_phase * TAU).sin();
        out.push(s);
    }
    band(&mut out, sr, 200.0, 4000.0);
    // Loop it: fold the last half second over the start.
    let x = ((0.5 * sr) as usize).min(len / 4);
    for i in 0..x {
        let k = i as f32 / x as f32;
        let tail = out[len - x + i];
        out[i] = out[i] * k.sqrt() + tail * (1.0 - k).sqrt();
    }
    out.truncate(len - x);
    out
}

pub fn rms(x: &[f32]) -> f32 {
    super::synth::rms(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::mml::parse;
    use crate::music::synth::{Instrument, Part, render};

    const SR: f32 = 24000.0;

    fn tune() -> Vec<f32> {
        render(
            &[Part {
                instrument: Instrument::Piano,
                notes: parse("t120 o4 l8 c e g > c < g e c4", 1).unwrap(),
                gain: 1.0,
            }],
            SR,
            1,
        )
    }

    /// Energy above `f` as a share of the whole.
    fn treble_share(x: &[f32], f: f32) -> f32 {
        let mut hp = x.to_vec();
        highpass(&mut hp, SR, f);
        highpass(&mut hp, SR, f);
        rms(&hp) / rms(x).max(1e-9)
    }

    #[test]
    fn a_worn_tape_is_duller_and_hissier() {
        let mut fresh = tune();
        let mut worn = tune();
        tape(&mut fresh, SR, 1, 0.0);
        tape(&mut worn, SR, 1, 1.0);
        assert!(treble_share(&worn, 4000.0) < treble_share(&fresh, 4000.0) * 1.5);
        assert!(fresh.iter().chain(&worn).all(|s| s.is_finite()));
        assert!(rms(&fresh) > 0.01);
    }

    #[test]
    fn the_radio_cuts_the_bass_and_treble() {
        let mut x = tune();
        let before_low = {
            let mut l = x.clone();
            lowpass(&mut l, SR, 150.0);
            rms(&l) / rms(&x)
        };
        radio(&mut x, SR, 1, 1.0);
        let mut l = x.clone();
        lowpass(&mut l, SR, 150.0);
        assert!(rms(&l) / rms(&x) < before_low);
        assert!(treble_share(&x, 6000.0) < 0.1);
    }

    #[test]
    fn gramophone_and_static_are_finite_and_heard() {
        let mut x = tune();
        gramophone(&mut x, SR, 2);
        assert!(x.iter().all(|s| s.is_finite()) && rms(&x) > 0.01);
        let s = static_bed((3.0 * SR) as usize, SR, 3);
        assert!(s.iter().all(|s| s.is_finite()) && rms(&s) > 0.01);
    }
}
