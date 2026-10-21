//! What's on the radio's dial. A small AM set picks up a handful of
//! stations in the valleys, and nothing in between but static:
//!
//! - **The Authority's evacuation loop** (702 kHz): the valley office's
//!   automated announcement, still going. Nobody turned it off, so the
//!   recall doesn't either. The voice is an asset
//!   (`audio/radio/evacuation.ogg`, made by `scripts/radio_voices.py`).
//! - **The Custodian's numbers** (1413 kHz): an interval signal on a music
//!   box, a call sign, then groups of five figures in Morse, new every
//!   raid day ([`numbers_program`]). It is the machines' own, and stops
//!   with them at the recall.
//! - **Nobody's numbers** (1215 kHz): a woman reading figures, slowly.
//!   Nobody knows whose it is; it is heard only once you've been coming
//!   in a while, best from high ground, and the recall doesn't touch it.
//! - **A station over the hills** (909 kHz): music from somewhere still
//!   living, faint and fading ([`far_music_program`]). Only from high
//!   ground, best on the Kettle saddle and over the reservoir.
//! - **Radio Marrow** (1080 kHz), off the air for twelve years: static
//!   with the old station surfacing in it, a jingle, the time pips, a bar
//!   of a tune ([`ghosts_program`]). Strongest under its mast in Marrow.
//! - **The machines' traffic** (1560 kHz): the robot chatter the game
//!   already plays; here it only gets a place on the dial.
//!
//! Engine-free: [`signal`] says how well a station comes in where you
//! stand, [`reception`] what a dial setting hears, and the `*_program`
//! functions render the loops the synthesised stations play.

use super::media;
use super::mml::{self, Note};
use super::synth::{self, Instrument, Part};
use crate::noise::Rng;
use std::f32::consts::TAU;

/// The AM band an ordinary set tunes (kHz).
pub const BAND: (f32, f32) = (530.0, 1600.0);

/// How far off a station's spot it still comes in: the bell's standard
/// deviation (kHz): wide enough that a gloved wrist finds it (the knob is
/// geared about 40 kHz a radian). Two of these off it is a seventh of its strength; five
/// off, gone.
pub const SPREAD: f32 = 6.0;

/// Height above the valley floor (m) at which high ground counts in full.
pub const HIGH_GROUND: f32 = 60.0;

/// Static under even the clearest station (share of amplitude).
pub const STATIC_FLOOR: f32 = 0.1;

/// Static heard underground, where nothing else gets through.
pub const UNDERGROUND_STATIC: f32 = 0.3;

/// Morse speed of the numbers station (words a minute, PARIS timing).
pub const MORSE_WPM: f32 = 15.0;

/// The numbers station's tone (Hz).
pub const MORSE_HZ: f32 = 800.0;

/// The numbers station's call sign.
pub const CALL_SIGN: &str = "QAT";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Figures in groups of five: in Morse after an interval signal, or
    /// read aloud (when the station has an `asset`).
    Numbers,
    /// The Authority's automated loop: attention tone, then a voice.
    Evacuation,
    /// A far-off station playing music, faint and fading.
    FarMusic,
    /// Static with the old valley station half-heard in it.
    Ghosts,
    /// The machines' radio traffic (played by the game).
    Chatter,
}

/// How well a station reaches one valley.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    /// Map id.
    pub valley: &'static str,
    /// Strength on high ground, 0..1.
    pub strength: f32,
    /// Share of that heard down on the valley floor, 0..1 (1: height
    /// doesn't matter; 0: only from high ground).
    pub floor: f32,
}

const fn r(valley: &'static str, strength: f32, floor: f32) -> Reach {
    Reach {
        valley,
        strength,
        floor,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Station {
    pub id: &'static str,
    /// What the station is, as a person would put it.
    pub name: &'static str,
    /// Its spot on the dial (kHz, inside [`BAND`]).
    pub dial: f32,
    pub kind: Kind,
    /// Per-valley reach; maps not listed get `elsewhere` (its `valley`
    /// is unused).
    pub reach: &'static [Reach],
    pub elsewhere: Reach,
    /// Multiplies its signal once the machines are recalled.
    pub after_recall: f32,
    /// Career rank from which it's heard at all.
    pub from_rank: u32,
    /// The recording it plays, when it isn't synthesised (asset path).
    pub asset: Option<&'static str>,
}

const MARROW: &str = "marrow_creek";
const KETTLE: &str = "kettle_pass";
const CINDER: &str = "cinder_works";
const SALLOW: &str = "sallow_reservoir";

pub const STATIONS: &[Station] = &[
    // Broadcast from the valley office's transmitter on the Marrow mast;
    // over the hills it thins.
    Station {
        id: "evacuation",
        name: "Quarantine Authority, valley office Marrow",
        dial: 702.0,
        kind: Kind::Evacuation,
        reach: &[
            r(MARROW, 1.0, 0.8),
            r(KETTLE, 0.55, 0.5),
            r(CINDER, 0.6, 0.6),
            r(SALLOW, 0.45, 0.4),
        ],
        elsewhere: r("", 0.2, 0.3),
        // Nobody turned it off.
        after_recall: 1.0,
        from_rank: 0,
        asset: Some("audio/radio/evacuation.ogg"),
    },
    // The Custodian's own: every depot sends, so it's strong everywhere.
    Station {
        id: "custodian_numbers",
        name: "Figures in Morse",
        dial: 1413.0,
        kind: Kind::Numbers,
        reach: &[
            r(MARROW, 0.9, 0.7),
            r(KETTLE, 0.85, 0.6),
            r(CINDER, 0.9, 0.7),
            r(SALLOW, 0.8, 0.6),
        ],
        elsewhere: r("", 0.3, 0.5),
        // It goes quiet with the machines.
        after_recall: 0.0,
        from_rank: 0,
        asset: None,
    },
    // Nobody's: from somewhere past the hills, only noticed by someone who
    // has been coming in a while.
    Station {
        id: "nobody_numbers",
        name: "A woman reading figures",
        dial: 1215.0,
        kind: Kind::Numbers,
        reach: &[
            r(KETTLE, 0.7, 0.2),
            r(SALLOW, 0.65, 0.3),
            r(MARROW, 0.5, 0.15),
            r(CINDER, 0.45, 0.15),
        ],
        elsewhere: r("", 0.4, 0.2),
        after_recall: 1.0,
        from_rank: 2,
        asset: Some("audio/radio/numbers_voice.ogg"),
    },
    // Somewhere still living. The valleys sit low; it needs the tops.
    Station {
        id: "far_music",
        name: "A station over the hills",
        dial: 909.0,
        kind: Kind::FarMusic,
        reach: &[
            r(KETTLE, 0.8, 0.0),
            r(SALLOW, 0.7, 0.35),
            r(MARROW, 0.4, 0.0),
            r(CINDER, 0.25, 0.0),
        ],
        elsewhere: r("", 0.5, 0.2),
        after_recall: 1.0,
        from_rank: 0,
        asset: None,
    },
    // The old valley station, off the air since the Lift, under its mast.
    Station {
        id: "radio_marrow",
        name: "Radio Marrow",
        dial: 1080.0,
        kind: Kind::Ghosts,
        reach: &[
            r(MARROW, 0.9, 0.6),
            r(KETTLE, 0.4, 0.3),
            r(CINDER, 0.45, 0.4),
            r(SALLOW, 0.5, 0.3),
        ],
        elsewhere: r("", 0.0, 0.0),
        after_recall: 1.0,
        from_rank: 0,
        asset: None,
    },
    // The machines talking: as close as the nearest pylon, so height
    // doesn't matter.
    Station {
        id: "chatter",
        name: "The machines' traffic",
        dial: 1560.0,
        kind: Kind::Chatter,
        reach: &[
            r(MARROW, 1.0, 1.0),
            r(KETTLE, 1.0, 1.0),
            r(CINDER, 1.0, 1.0),
            r(SALLOW, 1.0, 1.0),
        ],
        elsewhere: r("", 0.0, 0.0),
        after_recall: 0.0,
        from_rank: 0,
        asset: None,
    },
];

pub fn get(id: &str) -> Option<&'static Station> {
    STATIONS.iter().find(|s| s.id == id)
}

/// Where the radio is.
#[derive(Clone, Copy, Debug)]
pub struct Listen<'a> {
    /// Map id.
    pub valley: &'a str,
    /// Metres above the valley floor.
    pub height: f32,
    pub underground: bool,
    /// The machines have been recalled (the story's end).
    pub recalled: bool,
    /// Career rank.
    pub rank: u32,
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How well `station` comes in here, 0..1.
pub fn signal(station: &Station, at: &Listen) -> f32 {
    if at.underground || at.rank < station.from_rank {
        return 0.0;
    }
    let reach = station
        .reach
        .iter()
        .find(|r| r.valley == at.valley)
        .copied()
        .unwrap_or(station.elsewhere);
    let high = smoothstep(0.0, HIGH_GROUND, at.height);
    let s = reach.strength * (reach.floor + (1.0 - reach.floor) * high);
    let s = if at.recalled {
        s * station.after_recall
    } else {
        s
    };
    s.clamp(0.0, 1.0)
}

/// A station heard at a dial setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard {
    pub id: &'static str,
    /// Amplitude, 0..1.
    pub gain: f32,
    /// How far off its spot the dial is (kHz, signed). Slightly off, a
    /// set whistles at the beat ([`heterodyne_hz`]).
    pub detune: f32,
}

/// What a dial setting hears: the stations in reach and the static.
/// Above ground the powers sum to one, so turning the dial never makes
/// the set louder or quieter, only clearer.
#[derive(Clone, Debug, PartialEq)]
pub struct Reception {
    /// Strongest first; only those above a whisper.
    pub stations: Vec<Heard>,
    /// Amplitude, 0..1.
    pub static_gain: f32,
}

/// The whistle a set makes `detune` kHz off a carrier (Hz): the beat
/// between the carrier and where the set is listening, scaled down to the
/// dial's gearing so a station's edge whistles low and its far skirt high.
pub fn heterodyne_hz(detune: f32) -> f32 {
    detune.abs() * 250.0
}

/// What the set hears tuned to `dial` kHz.
pub fn reception(dial: f32, at: &Listen) -> Reception {
    if at.underground {
        return Reception {
            stations: Vec::new(),
            static_gain: UNDERGROUND_STATIC,
        };
    }
    let mut stations: Vec<Heard> = STATIONS
        .iter()
        .filter_map(|s| {
            let d = dial - s.dial;
            let bell = (-0.5 * (d / SPREAD).powi(2)).exp();
            let gain = signal(s, at) * bell;
            (gain > 0.01).then_some(Heard {
                id: s.id,
                gain,
                detune: d,
            })
        })
        .collect();
    let power: f32 = stations.iter().map(|h| h.gain * h.gain).sum();
    // Two strong stations on top of each other share the power.
    if power > 1.0 {
        let k = power.sqrt().recip();
        for h in stations.iter_mut() {
            h.gain *= k;
        }
    }
    let power = power.min(1.0);
    // Leave room for a little static under everything.
    let keep = (1.0 - STATIC_FLOOR * STATIC_FLOOR).sqrt();
    for h in stations.iter_mut() {
        h.gain *= keep;
    }
    let static_gain = (1.0 - power * keep * keep).max(0.0).sqrt();
    stations.sort_by(|a, b| b.gain.total_cmp(&a.gain));
    Reception {
        stations,
        static_gain,
    }
}

/// Where in a loop of `len` seconds a station is at world time `clock`
/// (s): every set in the valleys hears the same moment of the broadcast.
pub fn on_air_at(clock: f64, len: f32) -> f32 {
    if len <= 0.0 {
        return 0.0;
    }
    clock.rem_euclid(len as f64) as f32
}

// ---------------------------------------------------------------- Morse

fn morse_code(c: char) -> Option<&'static str> {
    Some(match c.to_ascii_uppercase() {
        'A' => ".-",
        'B' => "-...",
        'C' => "-.-.",
        'D' => "-..",
        'E' => ".",
        'F' => "..-.",
        'G' => "--.",
        'H' => "....",
        'I' => "..",
        'J' => ".---",
        'K' => "-.-",
        'L' => ".-..",
        'M' => "--",
        'N' => "-.",
        'O' => "---",
        'P' => ".--.",
        'Q' => "--.-",
        'R' => ".-.",
        'S' => "...",
        'T' => "-",
        'U' => "..-",
        'V' => "...-",
        'W' => ".--",
        'X' => "-..-",
        'Y' => "-.--",
        'Z' => "--..",
        '0' => "-----",
        '1' => ".----",
        '2' => "..---",
        '3' => "...--",
        '4' => "....-",
        '5' => ".....",
        '6' => "-....",
        '7' => "--...",
        '8' => "---..",
        '9' => "----.",
        '.' => ".-.-.-",
        ',' => "--..--",
        '?' => "..--..",
        '/' => "-..-.",
        // BT (a break) and AR (end of message).
        '=' => "-...-",
        '+' => ".-.-.",
        _ => return None,
    })
}

/// `text` in Morse, ITU timing: a dot is one unit on, a dash three, one
/// unit between the elements of a letter, three between letters, seven
/// between words. Spans of key down (`true`) and key up, in units; no
/// gap before the first element or after the last. Characters Morse has
/// no sign for are skipped.
pub fn morse(text: &str) -> Vec<(bool, f32)> {
    let mut out: Vec<(bool, f32)> = Vec::new();
    let mut gap = 0.0f32;
    for c in text.chars() {
        if c.is_whitespace() {
            if !out.is_empty() {
                gap = 7.0;
            }
            continue;
        }
        let Some(code) = morse_code(c) else { continue };
        if !out.is_empty() {
            out.push((false, gap.max(3.0)));
        }
        for (i, e) in code.chars().enumerate() {
            if i > 0 {
                out.push((false, 1.0));
            }
            out.push((true, if e == '-' { 3.0 } else { 1.0 }));
        }
        gap = 0.0;
    }
    out
}

/// Length of a Morse sequence in units.
pub fn morse_units(seq: &[(bool, f32)]) -> f32 {
    seq.iter().map(|(_, u)| u).sum()
}

/// Adds a keyed tone at `freq` from sample `start` for `len` samples,
/// with 5 ms edges (no key clicks).
fn key_tone(out: &mut [f32], start: usize, len: usize, freq: f32, sr: f32, amp: f32) {
    let edge = (0.005 * sr).max(1.0);
    let w = TAU * freq / sr;
    for i in 0..len {
        let Some(o) = out.get_mut(start + i) else {
            break;
        };
        let fi = i as f32;
        let env = (fi / edge).min(1.0).min((len as f32 - fi) / edge).max(0.0);
        *o += amp * env * (w * fi).sin();
    }
}

/// The day's figures: `n` groups of five, from `seed`.
pub fn number_groups(seed: u64, n: usize) -> Vec<String> {
    let mut rng = Rng::new(seed ^ 0x4E55_4D42);
    (0..n)
        .map(|_| {
            (0..5)
                .map(|_| char::from(b'0' + rng.below(10) as u8))
                .collect()
        })
        .collect()
}

/// The numbers station's interval signal: a short phrase on a music box.
const INTERVAL: &str = "t104 o5 l8 g e c e g4 > c4 < b8 a8 g4";

fn interval_signal(sr: f32) -> Vec<f32> {
    let notes = mml::parse(INTERVAL, 0).expect("interval signal");
    synth::render(
        &[Part {
            instrument: Instrument::MusicBox,
            notes,
            gain: 1.0,
        }],
        sr,
        0,
    )
}

/// Silence kept at each end of a program, so the loop seam falls in the
/// static and nowhere else.
const MARGIN: f32 = 1.0;

/// Mixes a loopable static bed of `n` samples under `x` at `level` of
/// its RMS, then levels the whole to a peak of 0.9.
fn under_static(x: &mut [f32], sr: f32, seed: u64, level: f32) {
    let n = x.len();
    let bed = media::static_bed(n + (0.5 * sr) as usize, sr, seed);
    let g = level * synth::rms(x).max(1e-4) / synth::rms(&bed).max(1e-6);
    for (s, b) in x.iter_mut().zip(bed) {
        *s += b * g;
    }
    synth::normalize(x, 0.9);
}

/// A numbers-station transmission, 60-120 s, loopable: the interval
/// signal three times, the call sign, then the day's groups of five
/// figures in Morse at [`MORSE_WPM`] on [`MORSE_HZ`], and AR at the end;
/// through a radio, over static. The figures come from `seed` (the game
/// seeds it with the raid day).
pub fn numbers_program(seed: u64, sr: f32) -> Vec<f32> {
    let unit = 1.2 / MORSE_WPM;
    let interval = interval_signal(sr);
    let ilen = interval.len() as f32 / sr;
    let call = format!("{CALL_SIGN} {CALL_SIGN} {CALL_SIGN} =");
    let call_s = (morse_units(&morse(&call)) + 7.0) * unit;
    let end = morse("+");
    let end_s = (morse_units(&end) + 7.0) * unit;
    let mut t = MARGIN + 3.0 * (ilen + 0.4) + 0.8 + call_s;
    // As many groups as fit in about a minute and a half.
    let mut groups = Vec::new();
    let mut day = Rng::new(seed);
    let mut k = 0u64;
    loop {
        let g = number_groups(day.next_u64() ^ k, 1).remove(0);
        let g_s = (morse_units(&morse(&g)) + 7.0) * unit;
        if t + g_s + end_s + MARGIN > 105.0 {
            break;
        }
        t += g_s;
        groups.push(g);
        k += 1;
    }
    let total = (t + end_s + MARGIN).max(60.0);
    let n = (total * sr) as usize;
    let mut x = vec![0.0f32; n];
    // Interval signal, three times.
    let mut at = MARGIN;
    for _ in 0..3 {
        let s = (at * sr) as usize;
        for (o, v) in x[s..].iter_mut().zip(&interval) {
            *o += v;
        }
        at += ilen + 0.4;
    }
    at += 0.8;
    // Then the call, the groups, the end.
    let text = format!("{call} {} +", groups.join(" "));
    let level = synth::rms(&interval).max(0.05) * 2.0;
    for (on, u) in morse(&text) {
        if on {
            key_tone(
                &mut x,
                (at * sr) as usize,
                (u * unit * sr) as usize,
                MORSE_HZ,
                sr,
                level,
            );
        }
        at += u * unit;
    }
    media::radio(&mut x, sr, seed, 0.4);
    under_static(&mut x, sr, seed, 0.25);
    x
}

// --------------------------------------------------------------- Ghosts

/// Bits of Radio Marrow that surface: tunes in the old station's style
/// (the project's own).
const GHOST_BARS: &[(Instrument, &str)] = &[
    (Instrument::Accordion, "t132 o4 l8 g a b > d4 < b g4 a8 b8"),
    (Instrument::Piano, "t96 o4 l8 e g > c4 < b a g4. e8"),
    (Instrument::Organ, "t80 o4 l4 c e g e d2"),
    (Instrument::Guitar, "t120 o3 l8 a > c e a e c < a4"),
];

/// The station ident: four rising bells.
const IDENT: &str = "t120 o5 l8 c e g > c4.";

fn render_mml(inst: Instrument, src: &str, seed: u64, sr: f32) -> Vec<f32> {
    let notes: Vec<Note> = mml::parse(src, seed).expect("ghost tune");
    synth::render(
        &[Part {
            instrument: inst,
            notes,
            gain: 1.0,
        }],
        sr,
        seed,
    )
}

/// The time signal: six pips on the second, the last one long.
fn pips(sr: f32) -> Vec<f32> {
    let mut x = vec![0.0; (5.8 * sr) as usize];
    for i in 0..6 {
        let len = if i == 5 { 0.5 } else { 0.1 };
        key_tone(
            &mut x,
            (i as f32 * sr) as usize,
            (len * sr) as usize,
            1000.0,
            sr,
            0.5,
        );
    }
    x
}

/// Static, with the old valley station surfacing in it now and then: a
/// bar of a tune, the six pips, the station's chime. Each fades up out
/// of the hiss, breaks up, and sinks back. 80-100 s, loopable.
pub fn ghosts_program(seed: u64, sr: f32) -> Vec<f32> {
    let mut rng = Rng::new(seed ^ 0x6405);
    let total = rng.range(80.0, 100.0);
    let n = (total * sr) as usize;
    let mut x = media::static_bed(n + (0.5 * sr) as usize, sr, seed);
    x.truncate(n);
    let bed = synth::rms(&x).max(1e-4);
    let mut t = rng.range(3.0, 6.0);
    let mut i = 0u64;
    loop {
        let mut frag = match rng.weighted(&[3.0, 1.0, 1.5]) {
            Some(0) => {
                let (inst, src) = GHOST_BARS[rng.below(GHOST_BARS.len())];
                render_mml(inst, src, seed ^ i, sr)
            }
            Some(1) => pips(sr),
            _ => render_mml(Instrument::Bell, IDENT, seed ^ i, sr),
        };
        let len = frag.len() as f32 / sr;
        if t + len + MARGIN > total {
            break;
        }
        media::radio(&mut frag, sr, seed ^ (i << 8), 0.8);
        // Up out of the hiss, breaking up, and back down.
        let rise = rng.range(0.8, 2.0).min(len * 0.4);
        let fall = rng.range(1.0, 2.5).min(len * 0.5);
        let peak = rng.range(0.6, 1.4) * bed / synth::rms(&frag).max(1e-4);
        let mut chop = 1.0f32;
        let mut target = 1.0f32;
        let start = (t * sr) as usize;
        for (j, v) in frag.iter().enumerate() {
            let s = j as f32 / sr;
            let env = smoothstep(0.0, rise, s) * smoothstep(0.0, fall, len - s);
            if j % (0.05 * sr) as usize == 0 {
                target = if rng.chance(0.25) {
                    rng.range(0.1, 0.5)
                } else {
                    1.0
                };
            }
            chop += (target - chop) * (200.0 / sr);
            x[start + j] += v * env * chop * peak;
        }
        t += len + rng.range(5.0, 14.0);
        i += 1;
    }
    synth::normalize(&mut x, 0.9);
    x
}

// ------------------------------------------------------------ Far music

/// A far-off station playing `parts`: the tune through a radio, fading
/// deep and slow the way a skywave does, a carrier whistle where another
/// station beats against it, and static that comes up as it fades.
/// At least 30 s (the tune repeats to fill), loopable.
pub fn far_music_program(parts: &[Part], seed: u64, sr: f32) -> Vec<f32> {
    let mut rng = Rng::new(seed ^ 0xFA4);
    let tune = synth::render(parts, sr, seed);
    let tune_s = if synth::rms(&tune) > 0.0 {
        tune.len() as f32 / sr
    } else {
        0.0
    };
    // Whole plays of the tune, 1.5 s apart, at least 30 s with margins.
    let plays = if tune_s > 0.0 {
        ((30.0 - 2.0 * MARGIN) / (tune_s + 1.5)).ceil().max(1.0) as usize
    } else {
        0
    };
    let total = (2.0 * MARGIN + plays as f32 * (tune_s + 1.5)).max(30.0);
    let n = (total * sr) as usize;
    let mut music = vec![0.0f32; n];
    for p in 0..plays {
        let s = ((MARGIN + p as f32 * (tune_s + 1.5)) * sr) as usize;
        for (o, v) in music[s..].iter_mut().zip(&tune) {
            *o += v;
        }
    }
    if plays > 0 {
        media::radio(&mut music, sr, seed, 1.0);
    }
    let level = synth::rms(&music).max(0.05);
    let mut bed = media::static_bed(n + (0.5 * sr) as usize, sr, seed);
    bed.truncate(n);
    let bed_g = level / synth::rms(&bed).max(1e-6);
    // Fading and whistle, both periodic in the loop so the seam is clean.
    let cycles = 2.0 + rng.below(2) as f32;
    let phase = rng.f32() * TAU;
    let wf = (rng.range(700.0, 1300.0) * total).round() / total;
    let wdev = rng.range(30.0, 90.0);
    let mut wphase = 0.0f64;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr;
        let u = t / total;
        let swell = 0.5 + 0.5 * (TAU * cycles * u + phase).cos();
        let fade = 0.15 + 0.85 * swell * swell;
        let f = wf + wdev * (TAU * u).sin();
        wphase += f as f64 / sr as f64;
        let whistle = (TAU * wphase.fract() as f32).sin() * level * 0.08;
        let hiss = bed[i] * bed_g * (0.25 + 0.6 * (1.0 - fade));
        out.push(music[i] * fade + whistle * (0.5 + 0.5 * fade) + hiss);
    }
    synth::normalize(&mut out, 0.9);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 24000.0;

    fn at(valley: &str, height: f32) -> Listen<'_> {
        Listen {
            valley,
            height,
            underground: false,
            recalled: false,
            rank: 5,
        }
    }

    fn finite_and_heard(x: &[f32]) {
        assert!(x.iter().all(|s| s.is_finite()));
        assert!(synth::rms(x) > 0.02, "rms {}", synth::rms(x));
    }

    /// The last 10 ms runs into the first 10 ms without a jump.
    fn loops_cleanly(x: &[f32]) {
        let w = (0.01 * SR) as usize;
        let (head, tail) = (&x[..w], &x[x.len() - w..]);
        let step = |s: &[f32]| s.windows(2).map(|p| (p[1] - p[0]).abs()).sum::<f32>() / w as f32;
        let typical = 0.5 * (step(head) + step(tail));
        let jump = (x[x.len() - 1] - x[0]).abs();
        assert!(jump < 4.0 * typical + 1e-3, "jump {jump} typical {typical}");
        let (a, b) = (synth::rms(head), synth::rms(tail));
        assert!(a < b * 3.0 + 1e-3 && b < a * 3.0 + 1e-3, "{a} {b}");
    }

    #[test]
    fn the_table_is_sane() {
        let mut ids = std::collections::HashSet::new();
        for s in STATIONS {
            assert!(ids.insert(s.id), "{} twice", s.id);
            assert!(s.dial > BAND.0 && s.dial < BAND.1, "{}", s.id);
            for r in s.reach.iter().chain([&s.elsewhere]) {
                assert!((0.0..=1.0).contains(&r.strength) && (0.0..=1.0).contains(&r.floor));
            }
            assert_eq!(get(s.id), Some(s));
        }
        // Far enough apart that two never share a spot.
        for a in STATIONS {
            for b in STATIONS {
                if a.id != b.id {
                    assert!((a.dial - b.dial).abs() > 6.0 * SPREAD);
                }
            }
        }
        for k in [
            Kind::Numbers,
            Kind::Evacuation,
            Kind::FarMusic,
            Kind::Ghosts,
            Kind::Chatter,
        ] {
            assert!(STATIONS.iter().any(|s| s.kind == k), "{k:?}");
        }
    }

    #[test]
    fn high_ground_helps_and_underground_nothing() {
        let far = get("far_music").unwrap();
        assert_eq!(signal(far, &at("kettle_pass", 0.0)), 0.0);
        assert!(signal(far, &at("kettle_pass", 80.0)) > 0.7);
        assert!(signal(far, &at("sallow_reservoir", 0.0)) > 0.2);
        let evac = get("evacuation").unwrap();
        let low = signal(evac, &at("marrow_creek", 0.0));
        let high = signal(evac, &at("marrow_creek", 60.0));
        assert!(low > 0.7 && high > low);
        assert!(signal(evac, &at("marrow_creek", 60.0)) > signal(evac, &at("kettle_pass", 60.0)));
        let under = Listen {
            underground: true,
            ..at("marrow_creek", 100.0)
        };
        for s in STATIONS {
            assert_eq!(signal(s, &under), 0.0, "{}", s.id);
        }
    }

    #[test]
    fn the_recall_silences_the_machines_not_the_authority() {
        let after = Listen {
            recalled: true,
            ..at("marrow_creek", 30.0)
        };
        assert_eq!(signal(get("custodian_numbers").unwrap(), &after), 0.0);
        assert_eq!(signal(get("chatter").unwrap(), &after), 0.0);
        assert!(signal(get("evacuation").unwrap(), &after) > 0.5);
        assert!(signal(get("radio_marrow").unwrap(), &after) > 0.5);
        assert!(signal(get("nobody_numbers").unwrap(), &after) > 0.2);
    }

    #[test]
    fn nobodys_numbers_wait_for_a_regular() {
        let s = get("nobody_numbers").unwrap();
        let new = Listen {
            rank: 0,
            ..at("kettle_pass", 80.0)
        };
        assert_eq!(signal(s, &new), 0.0);
        assert!(signal(s, &at("kettle_pass", 80.0)) > 0.5);
    }

    #[test]
    fn on_a_strong_station_it_is_clear() {
        let r = reception(702.0, &at("marrow_creek", 60.0));
        assert_eq!(r.stations[0].id, "evacuation");
        assert!(r.stations[0].gain > 0.95);
        assert!(r.stations[0].detune.abs() < 1e-4);
        assert!(r.static_gain < 0.2);
    }

    #[test]
    fn between_stations_it_is_static() {
        let r = reception(800.0, &at("marrow_creek", 60.0));
        let loudest = r.stations.first().map_or(0.0, |h| h.gain);
        assert!(r.static_gain > 0.95 && loudest < 0.1);
        // Slightly off, the station is there, with a whistle.
        let r = reception(706.0, &at("marrow_creek", 60.0));
        let h = r.stations[0];
        assert!(h.gain > 0.5 && h.gain < 0.95);
        assert!((heterodyne_hz(h.detune) - 1000.0).abs() < 1.0);
    }

    #[test]
    fn the_power_is_constant_across_the_dial() {
        let here = at("kettle_pass", 70.0);
        let mut d = BAND.0;
        while d < BAND.1 {
            let r = reception(d, &here);
            let p: f32 =
                r.stations.iter().map(|h| h.gain * h.gain).sum::<f32>() + r.static_gain.powi(2);
            assert!((p - 1.0).abs() < 0.02, "{d}: {p}");
            d += 0.5;
        }
    }

    #[test]
    fn underground_only_static() {
        let r = reception(
            702.0,
            &Listen {
                underground: true,
                ..at("marrow_creek", 0.0)
            },
        );
        assert!(r.stations.is_empty());
        assert!(r.static_gain > 0.0);
    }

    #[test]
    fn the_broadcast_is_the_same_moment_everywhere() {
        assert_eq!(on_air_at(130.0, 60.0), 10.0);
        assert_eq!(on_air_at(-5.0, 60.0), 55.0);
    }

    #[test]
    fn sos() {
        let (on, off) = (true, false);
        assert_eq!(
            morse("SOS"),
            vec![
                (on, 1.0),
                (off, 1.0),
                (on, 1.0),
                (off, 1.0),
                (on, 1.0),
                (off, 3.0),
                (on, 3.0),
                (off, 1.0),
                (on, 3.0),
                (off, 1.0),
                (on, 3.0),
                (off, 3.0),
                (on, 1.0),
                (off, 1.0),
                (on, 1.0),
                (off, 1.0),
                (on, 1.0),
            ]
        );
        // PARIS plus its word gap is 50 units: the standard word.
        assert_eq!(morse_units(&morse("PARIS")) + 7.0, 50.0);
        // Words are seven apart; unknown signs are skipped.
        assert_eq!(morse("E E"), vec![(on, 1.0), (off, 7.0), (on, 1.0)]);
        assert_eq!(morse("E~E"), morse("EE"));
    }

    #[test]
    fn the_figures_are_the_days() {
        let a = number_groups(1, 4);
        assert_eq!(a, number_groups(1, 4));
        assert_ne!(a, number_groups(2, 4));
        assert!(
            a.iter()
                .all(|g| g.len() == 5 && g.bytes().all(|b| b.is_ascii_digit()))
        );
    }

    #[test]
    fn the_numbers_program() {
        let a = numbers_program(11, SR);
        let len = a.len() as f32 / SR;
        assert!((60.0..=120.0).contains(&len), "{len} s");
        finite_and_heard(&a);
        loops_cleanly(&a);
        let b = numbers_program(12, SR);
        assert_ne!(a, b);
    }

    #[test]
    fn the_ghosts_program() {
        let x = ghosts_program(3, SR);
        let len = x.len() as f32 / SR;
        assert!((80.0..=100.0).contains(&len), "{len} s");
        finite_and_heard(&x);
        loops_cleanly(&x);
        assert_eq!(x, ghosts_program(3, SR));
    }

    #[test]
    fn the_far_music_program() {
        let parts = [Part {
            instrument: Instrument::Accordion,
            notes: mml::parse("t120 o4 l8 c e g e c e g4 | f a > c < a g2", 1).unwrap(),
            gain: 1.0,
        }];
        let x = far_music_program(&parts, 5, SR);
        let len = x.len() as f32 / SR;
        assert!((30.0..60.0).contains(&len), "{len} s");
        finite_and_heard(&x);
        loops_cleanly(&x);
        // With nothing to play, still a carrier in the static.
        let y = far_music_program(&[], 5, SR);
        finite_and_heard(&y);
        loops_cleanly(&y);
    }
}
