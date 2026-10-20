//! The score: a generative ambient layer that isn't there until it is.
//! Silent while nothing is wrong. As the Zone's attention turns to you
//! (`anomaly::Attention`, a response team on its way) or the Surge's
//! warning runs, long low drones fade in, and now and then a few slow
//! notes swell and die away. It falls away at the truck.
//!
//! Synthesised here, sample by sample, from one number: `intensity`,
//! 0 (silent) to 1 (the Surge breaking).
//!
//! - **Drones** on D: the root and a slow-beating partner, then the
//!   octave, the fifth, and last (past 0.6) the minor second over the root
//!   for unease. Each has a few harmonics, so a headset's small drivers
//!   still carry it, and each breathes on its own slow swell (17-43 s).
//! - **Fragments**: one to three notes from D minor pentatonic (E flat
//!   joins past 0.7), each swelling in over a second or more and ringing
//!   out over several. Never a hit: nothing in the score starts sharply.
//!   None below 0.2; at 1, one about every twelve seconds.
//!
//! Deterministic for a seed. `audio` plays it through [`ScoreNode`];
//! `scripts/audio_score.py` measures renders of it (written by this
//! module's test) against the mix: it must never hide a robot's sound,
//! and stays under the wind.

use bevy::prelude::*;
use bevy_seedling::firewheel::{
    channel_config::{ChannelConfig, ChannelCount},
    diff::{Diff, Patch},
    event::ProcEvents,
    node::{
        AudioNode, AudioNodeInfo, AudioNodeProcessor, ConstructProcessorContext, EmptyConfig,
        NodeError, ProcBuffers, ProcExtra, ProcInfo, ProcessStatus,
    },
};
use bevy_seedling::prelude::Volume;
use extraction_core::noise::Rng;
use std::f32::consts::TAU;

/// D2, the drones' root (Hz).
const ROOT: f32 = 73.42;

/// Where the score wants to be, from what the Zone is doing: the
/// attention on you (`anomaly::Attention::total`), whether a response
/// team is on its way to near you, the Surge's agitation (0..1), and
/// whether you're at the truck (or out of the raid). Attention alone takes
/// it to 0.75, a response to 0.5; only the Surge takes it all the way.
pub fn target(attention: f32, response_near: bool, agitation: f32, at_truck: bool) -> f32 {
    if at_truck {
        return 0.0;
    }
    let attention = ((attention - 0.3) / 2.7).clamp(0.0, 1.0) * 0.75;
    let response = if response_near { 0.5 } else { 0.0 };
    attention.max(response).max(agitation.clamp(0.0, 1.0))
}

/// Per second: how fast the score rises, falls, and falls at the truck.
pub const RISE: f32 = 1.0 / 10.0;
pub const FALL: f32 = 1.0 / 25.0;
pub const FALL_AT_TRUCK: f32 = 1.0 / 6.0;

/// How near (m) a response team's destination counts as coming for you.
pub const RESPONSE_NEAR: f32 = 150.0;

/// A drone voice: frequency (Hz), level, and the intensity over which it
/// fades in.
const DRONES: [(f32, f32, f32, f32); 6] = [
    (ROOT, 1.0, 0.02, 0.35),
    (ROOT * 1.0035, 0.7, 0.02, 0.35),
    (ROOT * 2.0, 0.45, 0.15, 0.5),
    (ROOT * 1.5, 0.4, 0.3, 0.7),
    (ROOT * 2.0 * 1.4983, 0.2, 0.45, 0.85),
    // The minor second: unease.
    (ROOT * 1.0595, 0.45, 0.6, 1.0),
];

/// A drone's harmonics (relative amplitudes).
const DRONE_PARTIALS: [f32; 3] = [1.0, 0.4, 0.15];

/// The fragments' notes: D minor pentatonic, D4 to D5.
const NOTES: [f32; 6] = [293.66, 349.23, 392.0, 440.0, 523.25, 587.33];
/// E flat over D: joins the fragments past 0.7.
const TENSE_NOTE: f32 = 311.13;

/// A fragment note's level against the drones.
const NOTE_LEVEL: f32 = 0.22;

/// How many fragment notes ring at once, at most.
const MAX_NOTES: usize = 8;

/// The intensity below which no fragment starts.
pub const FRAGMENTS_FROM: f32 = 0.2;

/// Mean seconds between fragments at [`FRAGMENTS_FROM`] and at full.
const FRAGMENT_EVERY: (f32, f32) = (45.0, 12.0);

/// Seconds the synth takes to follow a new intensity (it's smoothed in
/// the ECS too; this just keeps steps out of the audio).
const FOLLOW: f32 = 0.5;

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy, Default)]
struct Note {
    freq: f32,
    phase: f32,
    /// Seconds since it was due (negative: not yet).
    age: f32,
    attack: f32,
    release: f32,
    level: f32,
    /// Left and right gains (constant power).
    pan: (f32, f32),
    on: bool,
}

impl Note {
    fn envelope(&self) -> f32 {
        if self.age < 0.0 {
            0.0
        } else if self.age < self.attack {
            // A slow, rounded swell in.
            let t = self.age / self.attack;
            t * t * (3.0 - 2.0 * t)
        } else {
            (-(self.age - self.attack) / (self.release / 4.6)).exp()
        }
    }
}

/// The synthesiser: one per playing score.
pub struct ScoreSynth {
    sr: f32,
    rng: Rng,
    /// The intensity it's following, and where it is.
    target: f32,
    intensity: f32,
    drone_phase: [[f32; 3]; 6],
    breath_phase: [f32; 6],
    breath_rate: [f32; 6],
    notes: [Note; MAX_NOTES],
    /// Seconds until the next fragment may start.
    next_fragment: f32,
}

impl ScoreSynth {
    pub fn new(seed: u64, sample_rate: f32) -> Self {
        let mut rng = Rng::new(seed ^ 0x5C0E);
        let mut breath_phase = [0.0; 6];
        let mut breath_rate = [0.0; 6];
        for i in 0..6 {
            breath_phase[i] = rng.f32() * TAU;
            breath_rate[i] = TAU / rng.range(17.0, 43.0);
        }
        let next_fragment = rng.range(4.0, 10.0);
        Self {
            sr: sample_rate,
            rng,
            target: 0.0,
            intensity: 0.0,
            drone_phase: [[0.0; 3]; 6],
            breath_phase,
            breath_rate,
            notes: [Note::default(); MAX_NOTES],
            next_fragment,
        }
    }

    pub fn set_intensity(&mut self, i: f32) {
        self.target = i.clamp(0.0, 1.0);
    }

    /// Nothing sounding, and nothing about to.
    pub fn silent(&self) -> bool {
        self.target <= 0.0 && self.intensity < 1e-4 && !self.notes.iter().any(|n| n.on)
    }

    fn start_fragment(&mut self) {
        let count = 1 + self.rng.below(3);
        let tense = self.intensity > 0.7;
        let mut at = 0.0;
        let pan = self.rng.range(-0.6, 0.6);
        for _ in 0..count {
            let Some(slot) = self.notes.iter_mut().position(|n| !n.on) else {
                return;
            };
            let freq = if tense && self.rng.chance(0.3) {
                TENSE_NOTE
            } else {
                NOTES[self.rng.below(NOTES.len())]
            };
            let a = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
            self.notes[slot] = Note {
                freq,
                phase: 0.0,
                age: -at,
                attack: self.rng.range(1.0, 2.5),
                release: self.rng.range(4.0, 7.0),
                level: NOTE_LEVEL * self.rng.range(0.6, 1.0),
                pan: (a.cos(), a.sin()),
                on: true,
            };
            at += self.rng.range(1.5, 4.0);
        }
    }

    /// Renders into `left` and `right` (same length), overwriting them.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let dt = 1.0 / self.sr;
        let follow = 1.0 - (-dt / FOLLOW).exp();
        // Fragments are scheduled per block (blocks are a few ms).
        let block = left.len() as f32 * dt;
        self.next_fragment -= block;
        if self.next_fragment <= 0.0 {
            let i = self.intensity;
            if i >= FRAGMENTS_FROM {
                self.start_fragment();
            }
            let k = ((i - FRAGMENTS_FROM) / (1.0 - FRAGMENTS_FROM)).clamp(0.0, 1.0);
            let mean = FRAGMENT_EVERY.0 + (FRAGMENT_EVERY.1 - FRAGMENT_EVERY.0) * k;
            self.next_fragment = (-(1.0 - self.rng.f32()).max(1e-6).ln() * mean).max(6.0);
        }
        // Each voice's breath, per block (it moves over tens of seconds),
        // and its lean: alternate voices a little left and right.
        let mut gain = [(0.0f32, 0.0f32); 6];
        for (v, g) in gain.iter_mut().enumerate() {
            self.breath_phase[v] = (self.breath_phase[v] + self.breath_rate[v] * block) % TAU;
            let b = 0.7 + 0.3 * self.breath_phase[v].sin();
            *g = if v % 2 == 0 {
                (b, b * 0.85)
            } else {
                (b * 0.85, b)
            };
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.intensity += (self.target - self.intensity) * follow;
            let x = self.intensity;
            let (mut out_l, mut out_r) = (0.0, 0.0);
            for (v, &(f, level, lo, hi)) in DRONES.iter().enumerate() {
                let amp = level * smoothstep(lo, hi, x);
                let phases = &mut self.drone_phase[v];
                let mut s = 0.0;
                for (h, p) in DRONE_PARTIALS.iter().enumerate() {
                    phases[h] = (phases[h] + f * (h + 1) as f32 * dt).fract();
                    s += p * (phases[h] * TAU).sin();
                }
                out_l += amp * gain[v].0 * s;
                out_r += amp * gain[v].1 * s;
            }
            for n in self.notes.iter_mut().filter(|n| n.on) {
                n.age += dt;
                if n.age >= 0.0 {
                    n.phase = (n.phase + n.freq * dt).fract();
                    let p = n.phase * TAU;
                    let s = p.sin() + 0.2 * (2.0 * p).sin() + 0.05 * (3.0 * p).sin();
                    let e = n.envelope() * n.level * s;
                    out_l += e * n.pan.0;
                    out_r += e * n.pan.1;
                }
                if n.age > n.attack + n.release * 1.5 {
                    n.on = false;
                }
            }
            *l = out_l;
            *r = out_r;
        }
    }
}

/// The score as an audio node: stereo out, one parameter.
#[derive(Diff, Patch, Debug, Clone, Component)]
pub struct ScoreNode {
    /// 0 (silent) to 1.
    pub intensity: f32,
    pub volume: Volume,
    /// Seeds the fragments.
    pub seed: u64,
}

impl Default for ScoreNode {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            volume: Volume::Decibels(crate::audio_mix::SCORE_DB),
            seed: 0,
        }
    }
}

impl AudioNode for ScoreNode {
    type Configuration = EmptyConfig;

    fn info(&self, _config: &Self::Configuration) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("score")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::ZERO,
                num_outputs: ChannelCount::STEREO,
            }))
    }

    fn construct_processor(
        &self,
        _config: &Self::Configuration,
        cx: ConstructProcessorContext,
    ) -> Result<impl AudioNodeProcessor, NodeError> {
        let mut synth = ScoreSynth::new(self.seed, cx.stream_info.sample_rate.get() as f32);
        synth.set_intensity(self.intensity);
        Ok(ScoreProcessor {
            synth,
            gain: self.volume.amp(),
        })
    }
}

struct ScoreProcessor {
    synth: ScoreSynth,
    gain: f32,
}

impl AudioNodeProcessor for ScoreProcessor {
    fn events(&mut self, _info: &ProcInfo, events: &mut ProcEvents, _extra: &mut ProcExtra) {
        for patch in events.drain_patches::<ScoreNode>() {
            match patch {
                ScoreNodePatch::Intensity(i) => self.synth.set_intensity(i),
                ScoreNodePatch::Volume(v) => self.gain = v.amp(),
                ScoreNodePatch::Seed(_) => {}
            }
        }
    }

    fn process(
        &mut self,
        _info: &ProcInfo,
        buffers: ProcBuffers,
        _extra: &mut ProcExtra,
    ) -> ProcessStatus {
        if self.synth.silent() {
            return ProcessStatus::ClearAllOutputs;
        }
        let [l, r] = buffers.outputs else {
            return ProcessStatus::ClearAllOutputs;
        };
        self.synth.render(l, r);
        for s in l.iter_mut().chain(r.iter_mut()) {
            *s *= self.gain;
        }
        ProcessStatus::OutputsModified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;

    fn run(intensity: f32, seconds: f32, seed: u64) -> (Vec<f32>, Vec<f32>) {
        let mut synth = ScoreSynth::new(seed, SR);
        synth.set_intensity(intensity);
        let n = (seconds * SR) as usize;
        let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
        for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            synth.render(a, b);
        }
        (l, r)
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// Quiet when nothing's wrong and at the truck; the Surge outranks
    /// everything.
    #[test]
    fn it_follows_the_zone() {
        assert_eq!(target(0.0, false, 0.0, false), 0.0);
        assert_eq!(target(0.2, false, 0.0, false), 0.0);
        assert!(target(3.0, false, 0.0, false) <= 0.75);
        assert_eq!(target(0.0, true, 0.0, false), 0.5);
        assert_eq!(target(0.0, false, 1.0, false), 1.0);
        assert_eq!(target(3.0, true, 1.0, true), 0.0);
    }

    /// Nothing at all until something is wrong.
    #[test]
    fn silent_until_it_is() {
        let (l, r) = run(0.0, 30.0, 1);
        assert!(l.iter().chain(&r).all(|s| *s == 0.0));
        assert!(ScoreSynth::new(1, SR).silent());
    }

    /// Louder as it rises, and nothing in it starts sharply: no 50 ms
    /// more than 6 dB over the half second before (a startle is a jump of
    /// tens; the minor second's beating against the root is a few).
    #[test]
    fn it_rises_and_never_jumps() {
        let mut last = 0.0;
        for i in [0.1, 0.3, 0.6, 1.0] {
            let (l, r) = run(i, 60.0, 2);
            let level = rms(&l) + rms(&r);
            assert!(level > last, "{i}: {level} over {last}");
            last = level;
            let w = (0.05 * SR) as usize;
            let power: Vec<f32> = l.chunks(w).map(|c| rms(c).powi(2)).collect();
            for k in 50..power.len() {
                let before = power[k - 10..k].iter().sum::<f32>() / 10.0;
                let jump = 10.0 * (power[k] / before.max(1e-12)).log10();
                assert!(jump < 6.0, "{i}: +{jump:.1} dB at {:.2} s", k as f32 * 0.05);
            }
        }
    }

    /// The score at full and on the way there, at its in-game gain, for
    /// the audio audit (`scripts/audio_score.py`): target/audio_audit/
    /// score_<intensity>.wav, 90 s each, float, 48 kHz stereo.
    #[test]
    fn renders_for_the_audit() {
        let dir = crate::asset_root_path().join("../target/audio_audit");
        let _ = std::fs::create_dir_all(&dir);
        let gain = Volume::Decibels(crate::audio_mix::SCORE_DB).amp();
        for i in [0.25, 0.5, 0.75, 1.0] {
            let (l, r) = run(i, 90.0, 3);
            let mut data = Vec::with_capacity(l.len() * 8);
            for (a, b) in l.iter().zip(&r) {
                data.extend_from_slice(&(a * gain).to_le_bytes());
                data.extend_from_slice(&(b * gain).to_le_bytes());
            }
            let mut wav = Vec::new();
            let u32le = |v: u32| v.to_le_bytes();
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&u32le(36 + data.len() as u32));
            wav.extend_from_slice(b"WAVEfmt ");
            wav.extend_from_slice(&u32le(16));
            wav.extend_from_slice(&3u16.to_le_bytes()); // float
            wav.extend_from_slice(&2u16.to_le_bytes());
            wav.extend_from_slice(&u32le(SR as u32));
            wav.extend_from_slice(&u32le(SR as u32 * 8));
            wav.extend_from_slice(&8u16.to_le_bytes());
            wav.extend_from_slice(&32u16.to_le_bytes());
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&u32le(data.len() as u32));
            wav.extend_from_slice(&data);
            std::fs::write(dir.join(format!("score_{i:.2}.wav")), wav).unwrap();
        }
    }
}
