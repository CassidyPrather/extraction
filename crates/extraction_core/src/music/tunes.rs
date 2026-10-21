//! The valleys' repertoire: real tunes, all public domain, written out in
//! [`super::mml`] for the instruments in [`super::synth`]. Folk songs and
//! dances, hymns, a child's piano pieces, bells. The licence manifest (who
//! wrote each one, when, and why it's free to use) is in `docs/MUSIC.md`;
//! anything whose status was in doubt was left out.
//!
//! Each voice of a tune is one MML string; bar lines (`|`) are only for
//! reading, but the tests hold every voice of a tune to the same length so
//! a dropped beat in a transcription shows up. Octaves are written out
//! (`o4`) at the start of most bars rather than walked with `<` and `>`.
//! Harmonies for the hymns and the accompaniments are plain arrangements
//! made for the game.

use super::mml;
use super::synth::{Instrument, Part};

/// One line of a tune, on one instrument.
#[derive(Clone, Copy, Debug)]
pub struct Voice {
    pub instrument: Instrument,
    pub mml: &'static str,
    pub gain: f32,
}

/// A tune: what it is, where it comes from, and its voices.
#[derive(Clone, Copy, Debug)]
pub struct Tune {
    pub id: &'static str,
    pub title: &'static str,
    /// Composer or origin, and date.
    pub source: &'static str,
    pub voices: &'static [Voice],
}

impl Tune {
    /// Parses every voice into a [`Part`]. Each voice gets its own seed
    /// (so two hands don't fumble together). The tests parse every tune,
    /// so this doesn't fail in practice.
    pub fn parts(&self, seed: u64) -> Vec<Part> {
        self.voices
            .iter()
            .enumerate()
            .map(|(i, v)| Part {
                instrument: v.instrument,
                notes: mml::parse(v.mml, seed ^ (i as u64).wrapping_mul(0x9E37_79B9))
                    .unwrap_or_else(|e| panic!("tune {} voice {i}: {e}", self.id)),
                gain: v.gain,
            })
            .collect()
    }

    /// When the last note ends (s), not counting ring-out.
    pub fn length(&self) -> f32 {
        self.parts(0)
            .iter()
            .map(|p| mml::length(&p.notes))
            .fold(0.0, f32::max)
    }
}

/// Finds a tune by id.
pub fn get(id: &str) -> Option<&'static Tune> {
    TUNES.iter().find(|t| t.id == id)
}

const fn v(instrument: Instrument, mml: &'static str, gain: f32) -> Voice {
    Voice {
        instrument,
        mml,
        gain,
    }
}

use Instrument::*;

// ---------------------------------------------------------------- piano

/// Petzold's Minuet in G, first half (16 bars), right hand.
macro_rules! minuet_rh_a {
    () => {
        "o5 d4 o4 g a b o5 c | d4 o4 g4 g4 | o5 e4 c d e f+ | g4 o4 g4 g4 |
         o5 c4 d c o4 b a | b4 o5 c o4 b a g | f+4 g a b g | a2. |
         o5 d4 o4 g a b o5 c | d4 o4 g4 g4 | o5 e4 c d e f+ | g4 o4 g4 g4 |
         o5 c4 d c o4 b a | b4 o5 c o4 b a g | a4 b a g f+ | g2. |"
    };
}
macro_rules! minuet_lh_a {
    () => {
        "o3 g2 a | b2. | o4 c2. | o3 b2. | a2. | g2. | o4 d o3 b g | o4 d o3 d8 o4 c8 o3 b8 a8 |
         o3 b2 a | g b g | o4 c2. | o3 b o4 c8 o3 b8 a8 g8 | a2 f+ | g2 b | o4 c d o3 d | g2 o2 g |"
    };
}
/// Minuet in G, second half (bars 17-32), right hand.
macro_rules! minuet_rh_b {
    () => {
        "o5 b4 g a b g | a4 d e f+ d | g4 e f+ g d | c+4 o4 b o5 c+ o4 a4 |
         o4 a b o5 c+ d e f+ | g4 f+4 e4 | f+4 o4 a4 o5 c+4 | d2. |
         o5 d4 o4 g f+ g4 | o5 e4 o4 g f+ g4 | o5 d4 c4 o4 b4 | a g f+ g a4 |
         o4 d e f+ g a b | o5 c4 o4 b4 a4 | b o5 d o4 g4 f+4 | g2. |"
    };
}
macro_rules! minuet_lh_b {
    () => {
        "o3 g2. | f+2. | e2 g4 | a2 a4 |
         o3 a2. | b4 a4 g4 | a4 f+4 a4 | d2 o2 d4 |
         o3 b2. | o4 c2. | o3 b4 a4 g4 | d2. |
         o3 d2 f+4 | e2 g4 | d4 f+4 g4 | o2 g2. |"
    };
}

const MINUET_G: &[Voice] = &[
    v(
        Piano,
        concat!("t112 l8 v11 ", minuet_rh_a!(), minuet_rh_b!()),
        1.0,
    ),
    v(
        Piano,
        concat!("t112 l4 v9 ", minuet_lh_a!(), minuet_lh_b!()),
        0.75,
    ),
];

/// The same minuet, a child practising: slow, a few wrong notes, and the
/// first half twice.
const MINUET_G_LEARNER: &[Voice] = &[
    v(
        Piano,
        "t84 l8 v10
         o5 d4 o4 g a b o5 c | d4 o4 g4 g4 | o5 e4 c d e !f+ | g4 o4 g4 g4 |
         o5 c4 d c o4 b a | b4 o5 c o4 b a g | !f+4 g a b g | a2. |
         o5 d4 o4 g a b o5 c | d4 o4 g4 g4 | o5 e4 c d e f+ | g4 o4 g4 g4 |
         o5 !c4 d c o4 b a | b4 o5 c o4 b a g | a4 b a g f+ | g2. |
         o5 ~d4 o4 g a b o5 c | d4 o4 g4 g4 | o5 e4 c d e f+ | ~g4 o4 g4 g4 |
         o5 c4 d c o4 b a | b4 o5 c o4 b a !g | f+4 g a b g | a2. |
         o5 d4 o4 g a b o5 c | d4 o4 g4 g4 | o5 e4 c d e f+ | g4 o4 g4 g4 |
         o5 c4 d c o4 b a | b4 o5 c o4 b a g | ~a4 b a g f+ | g2. |",
        1.0,
    ),
    v(
        Piano,
        concat!("t84 l4 v8 ", minuet_lh_a!(), minuet_lh_a!()),
        0.7,
    ),
];

/// Für Elise, A section twice, the episode, A again.
const FUR_ELISE: &[Voice] = &[
    v(
        Piano,
        "t72 l16 v10 o5 e d+ |
         o5 e d+ e o4 b o5 d c | o4 a8 r c e a | b8 r e g+ b | o5 c8 r o4 e o5 e d+ |
         e d+ e o4 b o5 d c | o4 a8 r c e a | b8 r e o5 c o4 b | a8 r8 o5 e d+ |
         o5 e d+ e o4 b o5 d c | o4 a8 r c e a | b8 r e g+ b | o5 c8 r o4 e o5 e d+ |
         e d+ e o4 b o5 d c | o4 a8 r c e a | b8 r e o5 c o4 b | a8 r b o5 c d |
         o5 e8. o4 g o5 f e | d8. o4 f o5 e d | c8. o4 e o5 d c | o4 b8 r e o5 e r |
         r e o6 e r r o5 d+ | e r r d+ e d+ |
         o5 e d+ e o4 b o5 d c | o4 a8 r c e a | b8 r e g+ b | o5 c8 r o4 e o5 e d+ |
         e d+ e o4 b o5 d c | o4 a8 r c e a | b8 r e o5 c o4 b | a4. |",
        1.0,
    ),
    v(
        Piano,
        "t72 l16 v8 r8 |
         r4. | o2 a o3 e a r r8 | o2 e o3 e g+ r r8 | o2 a o3 e a r r8 |
         r4. | o2 a o3 e a r r8 | o2 e o3 e g+ r r8 | o2 a o3 e a r r8 |
         r4. | o2 a o3 e a r r8 | o2 e o3 e g+ r r8 | o2 a o3 e a r r8 |
         r4. | o2 a o3 e a r r8 | o2 e o3 e g+ r r8 | o2 a o3 e a r r8 |
         o3 c g o4 c r r8 | o2 g o3 g b r r8 | o2 a o3 e a r r8 | o2 e o3 e r r r8 |
         r4. | r4. |
         r4. | o2 a o3 e a r r8 | o2 e o3 e g+ r r8 | o2 a o3 e a r r8 |
         r4. | o2 a o3 e a r r8 | o2 e o3 e g+ r r8 | o2 a4. |",
        0.75,
    ),
];

/// A child's first tune, right hand and a held C: breaks down, starts
/// again.
const BRATRE_KUBO: &[Voice] = &[
    v(
        Piano,
        "t80 l4 v10
         o4 c d e c | c d e c | e f !g2 | e f g2 | g8 a8 !g8 f8 r2 | r1 |
         t72 o4 c d e c | c d e c | e f g2 | e f g2 |
         g8 a8 g8 f8 e c | g8 a8 g8 f8 !e c | c o3 g o4 c2 | c o3 g o4 c2 |",
        1.0,
    ),
    v(
        Piano,
        "t80 v7 o3 c1 | c1 | c1 | c1 | c2 r2 | r1 |
         t72 o3 c1 | c1 | c1 | c1 | c1 | c1 | c1 | c1 |",
        0.6,
    ),
];

/// Gymnopédie No. 1: the introduction and first phrase, played twice.
const GYMNOPEDIE: &[Voice] = &[
    v(
        Piano,
        "t72 l4 v10 [r2. r2. r2. r2. | r o5 ~f+ a | g f+ c+ | o4 b o5 c+ d | ~o4 a2. |
         f+2.&f+2.&f+2.&f+2. |]2",
        1.0,
    ),
    v(Piano, "t72 v9 [o2 g2. d2.]12", 0.7),
    v(Piano, "t72 v6 [r4 o3 b2 r4 o3 a2]12", 0.5),
    v(Piano, "t72 v6 [r4 o4 d2 r4 o4 c+2]12", 0.5),
    v(Piano, "t72 v6 [r4 o4 f+2 r4 o4 f+2]12", 0.5),
];

/// The Entertainer: introduction, then the A strain twice. A strain
/// bars 1-11 for each voice; bar 12 leads back, or ends.
macro_rules! ent_mel {
    () => {
        "o5 e o6 c8 o5 e o6 c8 o5 e o6 c& | c4 c d d+ e | c d e8 o5 b o6 d8 c& | c4 r8 o5 d d+ |
         o5 e o6 c8 o5 e o6 c8 o5 e o6 c& | c4 r o5 a g f+ | a o6 c e8 d c o5 a o6 d& | d4 r8 o5 d d+ |
         o5 e o6 c8 o5 e o6 c8 o5 e o6 c& | c4 c d d+ e | c d e8 o5 b o6 d8 c& |"
    };
}
macro_rules! ent_bass {
    () => {
        "o2 c r g r | c r g r | c r g r | c r g r |
         o2 c r g r | c r a r | d r a r | g r o3 d r |
         o2 c r g r | c r g r | c r g r |"
    };
}
macro_rules! ent_ch1 {
    () => {
        "r o3 e r e | r e r e | r e r f | r e r e |
         r o3 e r e | r e r f+ | r f+ r f+ | r f r f |
         r o3 e r e | r e r e | r e r f |"
    };
}
macro_rules! ent_ch2 {
    () => {
        "r o3 g r g | r g r g | r g r g | r g r g |
         r o3 g r g | r g r a | r a r a | r b r b |
         r o3 g r g | r g r g | r g r g |"
    };
}
macro_rules! ent_ch3 {
    () => {
        "r o4 c r c | r c r c | r c r o3 b | r o4 c r c |
         r o4 c r c | r c r c | r c r c | r d r d |
         r o4 c r c | r c r c | r c r o3 b |"
    };
}
const RAGTIME: &[Voice] = &[
    v(
        Piano,
        concat!(
            "t76 l16 v11 o6 d e c o5 a8 b g8 | d e c o4 a8 b g8 | o4 d e c o3 a8 b a a- | g4 r8 o5 d d+ |",
            ent_mel!(),
            "o6 c4 r8 o5 d d+ |",
            ent_mel!(),
            "o6 c2 |"
        ),
        1.0,
    ),
    // Left hand: the introduction in octaves, then a stride bass.
    v(
        Piano,
        concat!(
            "t76 l8 v9 o5 d16 e16 c16 o4 a b16 g | o4 d16 e16 c16 o3 a b16 g |
             o3 d16 e16 c16 o2 a b16 a16 a-16 | o2 g4 r4 |",
            ent_bass!(),
            "o2 c r g r |",
            ent_bass!(),
            "o2 c4 o1 c4 |"
        ),
        0.75,
    ),
    v(
        Piano,
        concat!(
            "t76 l8 v7 r2 | r2 | r2 | r o3 f r4 |",
            ent_ch1!(),
            "r o3 e r e |",
            ent_ch1!(),
            "r o3 e e4 |"
        ),
        0.45,
    ),
    v(
        Piano,
        concat!(
            "t76 l8 v7 r2 | r2 | r2 | r o3 b r4 |",
            ent_ch2!(),
            "r o3 g r g |",
            ent_ch2!(),
            "r o3 g g4 |"
        ),
        0.45,
    ),
    v(
        Piano,
        concat!(
            "t76 l8 v7 r2 | r2 | r2 | r o4 d r4 |",
            ent_ch3!(),
            "r o4 c r c |",
            ent_ch3!(),
            "r o4 c c4 |"
        ),
        0.45,
    ),
];

// ------------------------------------------------------------------ hum

macro_rules! wiegenlied {
    () => {
        "o4 g4. e e4 | g2 e g | o5 c4 o4 b4. a | a4 g4 d e | f4 d4 d e | f2 d f |
         b a g4 b4 | o5 c2 o4 c c | o5 c2 o4 a f | g2 e c | f4 g4 a4 | g2 c c |
         o5 c2 o4 a f | g2 e c |"
    };
}
/// Brahms' Wiegenlied, hummed twice through, slowing at the end.
const BRAHMS_LULLABY: &[Voice] = &[v(
    Hum,
    concat!(
        "t63 l8 v11 o4 e e |",
        wiegenlied!(),
        "f4 e4 d4 | c2 e e |",
        wiegenlied!(),
        "t56 f4 e4 d4 | t50 c2. |"
    ),
    1.0,
)];

/// The Largo's theme from the New World symphony, hummed at work.
macro_rules! largo {
    () => {
        "o4 e4. g8 g2 | e4. d8 c2 | d4. e8 g4. e8 | d2. r4 |
         e4. g8 g2 | e4. d8 c2 | d4. e8 d4. c8 |"
    };
}
const LARGO: &[Voice] = &[v(
    Hum,
    concat!("t54 v11 ", largo!(), "c2. r4 |", largo!(), "c1 |"),
    1.0,
)];

// --------------------------------------------------------------- hymns

// OLD 100TH in G, four parts; each line a minim, six crotchets, a minim.
const OLD_HUNDREDTH: &[Voice] = &[
    v(
        Harmonium,
        "t80 l4 v11 [o4 g2 g f+ e d g a b2 | b2 b b a g o5 c o4 b a2 |
         g2 a b a g e f+ g2 | o5 d2 o4 b g a o5 c o4 b a g2 |]2 o4 g2 g2",
        1.0,
    ),
    v(
        Harmonium,
        "t80 l4 v9 [o4 d2 d d o3 b b o4 e f+ g2 | g2 g g f+ d e e d2 |
         d2 d g f+ e c d d2 | g2 g e f+ e d d o3 b2 |]2 o4 e2 d2",
        0.7,
    ),
    v(
        Harmonium,
        "t80 l4 v9 [o3 b2 g a g g b o4 d d2 | d2 e d d o3 g a g a2 |
         b2 a o4 d d o3 b a a b2 | b2 b o4 c o3 a a g f+ g2 |]2 o4 c2 o3 b2",
        0.7,
    ),
    v(
        Harmonium,
        "t80 l4 v10 [o2 g2 b o3 d e o2 b o3 e d o2 g2 | g2 o3 e o2 b o3 d o2 b a o3 e f+2 |
         g2 f+ g d e o2 a o3 d o2 g2 | g2 o3 e e d o2 a o3 d d o2 g2 |]2 o3 c2 o2 g2",
        0.8,
    ),
];

/// Stille Nacht in C, one verse, four parts.
const SILENT_NIGHT: &[Voice] = &[
    v(
        Harmonium,
        "t80 v11 o4 g4. a8 g4 | e2. | g4. a8 g4 | e2. | o5 d2 d4 | o4 b2. | o5 c2 c4 | o4 g2. |
         a2 a4 | o5 c4. o4 b8 a4 | g4. a8 g4 | e2. | a2 a4 | o5 c4. o4 b8 a4 | g4. a8 g4 | e2. |
         o5 d2 d4 | f4. d8 o4 b4 | o5 c2. | e2. | c4 o4 g4 e4 | g4. f8 d4 | c2.&c2. |",
        1.0,
    ),
    v(
        Harmonium,
        "t80 v9 o4 e4. f8 e4 | c2. | e4. f8 e4 | c2. | f2 f4 | f2. | e2 e4 | e2. |
         f2 f4 | a4. g8 f4 | e4. f8 e4 | c2. | f2 f4 | a4. g8 f4 | e4. f8 e4 | c2. |
         f2 f4 | f2. | e2. | g2. | e4 e4 c4 | d4. d8 o3 b4 | g2.&g2. |",
        0.7,
    ),
    v(
        Harmonium,
        "t80 v9 o4 c2.&c2.&c2.&c2. | o3 b2.&b2. | o4 c2.&c2. | c2.&c2. | c2.&c2. |
         c2.&c2. | c2.&c2. | o3 b2.&b2. | o4 c2.&c2. | o3 g2. | g2. | e2.&e2. |",
        0.65,
    ),
    v(
        Harmonium,
        "t80 v10 o3 c2.&c2.&c2.&c2. | o2 g2.&g2. | o3 c2.&c2. | o2 f2.&f2. | o3 c2.&c2. |
         o2 f2.&f2. | o3 c2.&c2. | o2 g2.&g2. | o3 c2.&c2. | c2. | o2 g2. | o3 c2.&c2. |",
        0.8,
    ),
];

/// HYMN TO JOY in G on the organ, four parts, two verses.
const HYMN_TO_JOY: &[Voice] = &[
    v(
        Organ,
        "t96 l4 v11 [o4 b b o5 c d | d c o4 b a | g g a b | b4. a8 a2 |
         b b o5 c d | d c o4 b a | g g a b | a4. g8 g2 |
         a a b g | a b8 o5 c8 o4 b g | a b8 o5 c8 o4 b a | g a d2 |
         b b o5 c d | d c o4 b a | g g a b | a4. g8 g2 |]2",
        1.0,
    ),
    v(
        Organ,
        "t96 l4 v9 [o4 g g g g | g g g f+ | d e f+ g | g2 f+2 |
         g g g g | g g g f+ | d e f+ g | f+2 d2 |
         f+ f+ g e | f+ f+ g d | f+ f+ g f+ | e e d2 |
         g g g g | g g g f+ | d e f+ g | f+2 d2 |]2",
        0.7,
    ),
    v(
        Organ,
        "t96 l4 v9 [o4 d d e d | d e d d | o3 b b a o4 d | d2 d2 |
         d d e d | d e d d | o3 b b a o4 d | c2 o3 b2 |
         o4 d c o3 b b | o4 d c o3 b b | o4 d c o3 b a | b g f+2 |
         o4 d d e d | d e d d | o3 b b a o4 d | c2 o3 b2 |]2",
        0.7,
    ),
    v(
        Organ,
        "t96 l4 v10 [o2 g g o3 c o2 b | b o3 c o2 g o3 d | g e d o2 b | g2 o3 d2 |
         o2 g g o3 c o2 b | b o3 c o2 g o3 d | g e d o2 b | o3 d2 o2 g2 |
         o3 d f+ g e | d d g o2 g | o3 d d g d | e o2 a o3 d2 |
         o2 g g o3 c o2 b | b o3 c o2 g o3 d | g e d o2 b | o3 d2 o2 g2 |]2",
        0.8,
    ),
];

// ------------------------------------------------------- accordion

/// O du lieber Augustin: a folk waltz, accordion with bass.
macro_rules! augustin {
    ($o:literal, $lo:literal) => {
        concat!(
            "o",
            $o,
            " g4. a8 g8 f8 | e4 c4 c4 | d4 o",
            $lo,
            " g4 g4 | o",
            $o,
            " e4 c4 c4 | g4. a8 g8 f8 | e4 c4 c4 | d4 o",
            $lo,
            " g4 g4 | o",
            $o,
            " c2. |"
        )
    };
}
const WALTZ_FOLK: &[Voice] = &[
    v(
        Accordion,
        concat!(
            "t156 v11 ",
            augustin!("5", "4"),
            augustin!("4", "3"),
            "~",
            augustin!("5", "4"),
            augustin!("5", "4")
        ),
        1.0,
    ),
    v(
        Accordion,
        "t156 l4 v7 [r o4 e e | r e e | r d d | r e e | r e e | r e e | r d d | r e e |]4",
        0.45,
    ),
    v(
        Accordion,
        "t156 l4 v7 [r o4 g g | r g g | r f f | r g g | r g g | r g g | r f f | r g g |]4",
        0.45,
    ),
    v(
        Bass,
        "t156 v11 [o3 c4 r2 | o2 g4 r2 | g4 r2 | o3 c4 r2 | c4 r2 | o2 g4 r2 | g4 r2 | o3 c4 r2 |]3
         o3 c4 r2 | o2 g4 r2 | g4 r2 | o3 c4 r2 | c4 r2 | o2 g4 r2 | g4 r2 | o3 c2. |",
        0.9,
    ),
];

/// Hänschen klein as a village polka, three times round.
macro_rules! hanschen {
    ($hi:literal, $lo:literal) => {
        concat!(
            "o",
            $hi,
            " d8 o",
            $lo,
            " b8 b4 | o",
            $hi,
            " c8 o",
            $lo,
            " a8 a4 | g8 a8 b8 o",
            $hi,
            " c8 | d8 d8 d4 | d8 o",
            $lo,
            " b8 b4 | o",
            $hi,
            " c8 o",
            $lo,
            " a8 a4 | g8 b8 o",
            $hi,
            " d8 d8 | o",
            $lo,
            " g2 |",
            "o",
            $lo,
            " a8 a8 a8 a8 | a8 b8 o",
            $hi,
            " c4 | o",
            $lo,
            " b8 b8 b8 b8 | b8 o",
            $hi,
            " c8 d4 | d8 o",
            $lo,
            " b8 b4 | o",
            $hi,
            " c8 o",
            $lo,
            " a8 a4 | g8 b8 o",
            $hi,
            " d8 d8 |"
        )
    };
}
macro_rules! polka_bass {
    () => {
        "o2 g r o3 d r | o2 d r a r | g r o3 d r | o2 g r o3 d r |
         o2 g r o3 d r | o2 d r a r | g r o3 d r | o2 g r o3 d r |
         o2 d r a r | d r a r | g r o3 d r | o2 g r o3 d r |
         o2 g r o3 d r | o2 d r a r | g r o3 d r |"
    };
}
const POLKA: &[Voice] = &[
    v(
        Accordion,
        concat!(
            "t126 v11 ",
            hanschen!("5", "4"),
            "o4 g2 |",
            hanschen!("4", "3"),
            "o3 g2 |",
            hanschen!("5", "4"),
            "o4 g8 r8 o3 g4 |"
        ),
        1.0,
    ),
    v(
        Accordion,
        "t126 l8 v7 [r o3 b r b | r a r a | r b r b | r b r b | r b r b | r a r a | r b r b | r b r b |
         r a r a | r a r a | r b r b | r b r b | r b r b | r a r a | r b r b | r b r b |]3",
        0.45,
    ),
    v(
        Accordion,
        "t126 l8 v7 [r o4 d r d | r c r c | r d r d | r d r d | r d r d | r c r c | r d r d | r d r d |
         r c r c | r c r c | r d r d | r d r d | r d r d | r c r c | r d r d | r d r d |]3",
        0.45,
    ),
    v(
        Bass,
        concat!(
            "t126 l8 v11 [",
            polka_bass!(),
            "o2 g r o3 d r |]2",
            polka_bass!(),
            "o2 g r o1 g4 |"
        ),
        0.9,
    ),
];

// ---------------------------------------------------------- guitar

/// Greensleeves in D minor: picked guitar, someone humming the tune.
const GREENSLEEVES: &[Voice] = &[
    v(
        Hum,
        "t66 v10 r2. | r2 r8 o4 d8 |
         o4 f4 g8 a8. b-16 a8 | g4 e8 c8. d16 e8 | f4 d8 d8. c+16 d8 | e4 c+8 o3 a4 o4 d8 |
         f4 g8 a8. b-16 a8 | g4 e8 c8. d16 e8 | f8. e16 d8 c+8. o3 b16 o4 c+8 | d4. d4. |
         o5 c4. c8. o4 b16 a8 | g4 e8 c8. d16 e8 | f4 d8 d8. c+16 d8 | e4 c+8 o3 a4. |
         o5 c4. c8. o4 b16 a8 | g4 e8 c8. d16 e8 | f8. e16 d8 c+8. o3 b16 o4 c+8 | d4. d4. |",
        1.0,
    ),
    // Bass string, held through the bar.
    v(
        Guitar,
        "t66 v10 o3 d2. | d2. | d2. | c2. | d2. | o2 a2. | o3 d2. | c2. | d4. o2 a4. | o3 d2. |
         o2 f2. | o3 c2. | d2. | o2 a2. | f2. | o3 c2. | d4. o2 a4. | o3 d2. |",
        0.8,
    ),
    v(
        Guitar,
        "t66 v8 [r8 o3 a4 r8 a4]3 | r8 g4 r8 g4 | r8 a4 r8 a4 | r8 e4 r8 e4 | r8 a4 r8 a4 |
         r8 g4 r8 g4 | r8 a4 r8 e4 | r8 a4 r8 a4 |
         r8 c4 r8 c4 | r8 g4 r8 g4 | r8 a4 r8 a4 | r8 e4 r8 e4 | r8 c4 r8 c4 |
         r8 g4 r8 g4 | r8 a4 r8 e4 | r8 a4 r8 a4 |",
        0.6,
    ),
    v(
        Guitar,
        concat!(
            "t66 v8 [r4 o4 d8 f4 d8]3 | r4 o4 c8 e4 c8 | r4 o4 d8 f4 d8 | r4 o3 a8 o4 c+4 o3 a8 |",
            "r4 o4 d8 f4 d8 | r4 o4 c8 e4 c8 | r4 o4 d8 r4 c+8 | r4 o4 d8 f4 d8 |",
            "r4 o3 f8 a4 f8 | r4 o4 c8 e4 c8 | r4 o4 d8 f4 d8 | r4 o3 a8 o4 c+4 o3 a8 |",
            "r4 o3 f8 a4 f8 | r4 o4 c8 e4 c8 | r4 o4 d8 r4 c+8 | r4 o4 d8 f4 d8 |"
        ),
        0.6,
    ),
];

// ----------------------------------------------------------- bells

/// Rounds, a plain course of Plain Hunt on six, call changes to Queens
/// and back, rounds. Bells 1-6 are A G F E D C (the tenor on C); a beat's
/// gap before every handstroke row.
const BELLS_ROUNDS: &[Voice] = &[v(
    Bell,
    "t180 l4 v11 o4 [r a g f e d c a g f e d c]3
     r g a e f c d g e a c f d | r e g c a d f e c g d a f | r c e d g f a c d e f g a |
     r d c f e a g d f c a e g | r f d a c g e f a d g c e | r a f g d e c a g f e d c |
     [r a g f e d c a g f e d c]2
     r a f g e d c a f g e d c | r a f e g d c a f e g d c |
     r a f e d g c a f e d g c | r a f d e g c a f d e g c |
     [r a f d g e c a f d g e c]2
     r a f g d e c a f g d e c | r a g f d e c a g f d e c |
     [r a g f e d c a g f e d c]2",
    0.9,
)];

/// The Westminster Quarters for the hour, then six strokes.
const WESTMINSTER: &[Voice] = &[v(
    Bell,
    "t76 l4 v11 o4 e g+ f+ o3 b2 r | o4 e f+ g+ e2 r | o4 g+ e f+ o3 b2 r | o3 b o4 f+ g+ e2 r |
     r2 [o3 e1]6",
    0.9,
)];

/// OLD 100TH as a carillon plays it, with a bourdon bell under the lines.
const CARILLON: &[Voice] = &[
    v(
        Bell,
        "t72 l4 v11 o4 g2 g f+ e d g a b2 | b2 b b a g o5 c o4 b a2 |
         g2 a b a g e f+ g2 | o5 d2 o4 b g a o5 c o4 b a g2 |",
        0.9,
    ),
    v(
        Bell,
        "t72 v9 o3 g2 r1 r2 g2 | g2 r1 r2 d2 | g2 r1 r2 g2 | g2 r1 r2 g2 |",
        0.6,
    ),
];

// ------------------------------------------------------- music box

macro_rules! twinkle_mel {
    () => {
        "o5 c c g g | a a g2 | f f e e | d d c2 | g g f f | e e d2 | g g f f | e e d2 |"
    };
}
macro_rules! twinkle_bass {
    () => {
        "o4 c e | f e | d c | g c | e d | c g | e d | c g |"
    };
}
/// Ah ! vous dirai-je, maman: twice, the spring running down.
const MUSIC_BOX: &[Voice] = &[
    v(
        MusicBox,
        concat!(
            "t104 l4 v11 ",
            twinkle_mel!(),
            "c c g g | a a g2 | f f e e | d d c2 |",
            twinkle_mel!(),
            "t92 c c g g | t82 a a g2 | t70 f f e e | t58 d d c2 |"
        ),
        1.0,
    ),
    v(
        MusicBox,
        concat!(
            "t104 l2 v8 ",
            twinkle_bass!(),
            "c e | f e | d c | g c |",
            twinkle_bass!(),
            "t92 c e | t82 f e | t70 d c | t58 g c |"
        ),
        0.6,
    ),
];

// ------------------------------------------------------- dance band

macro_rules! susanna {
    () => {
        "o5 e8 g8 g8. a16 | g8 e8 c8. d16 | e8 e8 d8 c8 | d4 c8 d8 |
         e8 g8 g8. a16 | g8 e8 c8. d16 | e8 e8 d8 d8 | c2 |
         f4 f4 | a4 a4 | g8 g8 e8 c8 | d4 c8 d8 |
         e8 g8 g8. a16 | g8 e8 c8. d16 | e8 e8 d8 d8 |"
    };
}
/// Oh! Susanna for a far-off station's dance band: accordion lead, piano
/// vamping, plucked bass.
const DANCE_BAND: &[Voice] = &[
    v(
        Accordion,
        concat!(
            "t116 v11 r2 | r2 | r2 | r4 o5 c8 d8 |",
            susanna!(),
            "c4 c8 d8 |",
            susanna!(),
            "c2 |"
        ),
        1.0,
    ),
    v(
        Piano,
        "t116 l8 v8 [r o4 e r e | r d r d]2
         [r o4 e r e | r e r e | r e r e | r d r d | r e r e | r e r e | r d r d | r e r e |
          r f r f | r f r f | r e r e | r d r d | r e r e | r e r e | r d r d | r e r e |]2",
        0.5,
    ),
    v(
        Piano,
        "t116 l8 v8 [r o4 g r g | r f r f]2
         [r o4 g r g | r g r g | r g r g | r f r f | r g r g | r g r g | r f r f | r g r g |
          r a r a | r a r a | r g r g | r f r f | r g r g | r g r g | r f r f | r g r g |]2",
        0.5,
    ),
    v(
        Bass,
        "t116 l4 v11 [o3 c o2 g | g o3 d]2
         [o3 c o2 g | o3 c o2 g | o3 c o2 g | g o3 d | c o2 g | o3 c o2 g | g o3 d | c o2 g |
          o2 f o3 c | o2 f o3 c | c o2 g | g o3 d | c o2 g | o3 c o2 g | g o3 d | c o2 g |]2",
        0.9,
    ),
];

/// Every tune in the valleys.
pub const TUNES: &[Tune] = &[
    Tune {
        id: "minuet_g",
        title: "Minuet in G",
        source: "Christian Petzold, Minuet in G major, BWV Anh. 114 (c. 1725)",
        voices: MINUET_G,
    },
    Tune {
        id: "minuet_g_learner",
        title: "Minuet in G (practice)",
        source: "Christian Petzold, Minuet in G major, BWV Anh. 114 (c. 1725)",
        voices: MINUET_G_LEARNER,
    },
    Tune {
        id: "fur_elise",
        title: "Für Elise",
        source: "L. van Beethoven, Bagatelle in A minor, WoO 59 (1810)",
        voices: FUR_ELISE,
    },
    Tune {
        id: "bratre_kubo",
        title: "Bratře Kubo (practice)",
        source: "\"Frère Jacques\", French trad. round (18th c.)",
        voices: BRATRE_KUBO,
    },
    Tune {
        id: "gymnopedie",
        title: "Gymnopédie No. 1",
        source: "Erik Satie, Gymnopédie No. 1 (1888)",
        voices: GYMNOPEDIE,
    },
    Tune {
        id: "ragtime",
        title: "The Entertainer",
        source: "Scott Joplin, The Entertainer (1902)",
        voices: RAGTIME,
    },
    Tune {
        id: "brahms_lullaby",
        title: "Wiegenlied",
        source: "J. Brahms, Wiegenlied Op. 49 No. 4 (1868)",
        voices: BRAHMS_LULLABY,
    },
    Tune {
        id: "largo",
        title: "Largo (Goin' Home)",
        source: "A. Dvořák, Symphony No. 9 \"From the New World\", Largo (1893)",
        voices: LARGO,
    },
    Tune {
        id: "old_hundredth",
        title: "Old Hundredth",
        source: "Louis Bourgeois, OLD 100TH, Genevan Psalter (1551)",
        voices: OLD_HUNDREDTH,
    },
    Tune {
        id: "silent_night",
        title: "Stille Nacht",
        source: "Franz Xaver Gruber, Stille Nacht (1818)",
        voices: SILENT_NIGHT,
    },
    Tune {
        id: "hymn_to_joy",
        title: "Hymn to Joy",
        source: "L. van Beethoven, Symphony No. 9, finale theme (1824)",
        voices: HYMN_TO_JOY,
    },
    Tune {
        id: "waltz_folk",
        title: "O du lieber Augustin",
        source: "Viennese folk song, trad. (c. 1800)",
        voices: WALTZ_FOLK,
    },
    Tune {
        id: "polka",
        title: "Hänschen klein",
        source: "German folk tune, trad. (words F. Wiedemann, 1860)",
        voices: POLKA,
    },
    Tune {
        id: "greensleeves",
        title: "Greensleeves",
        source: "English folk tune, trad. (late 16th c.)",
        voices: GREENSLEEVES,
    },
    Tune {
        id: "bells_rounds",
        title: "Rounds, Plain Hunt and call changes",
        source: "English change ringing, trad. (17th c.)",
        voices: BELLS_ROUNDS,
    },
    Tune {
        id: "westminster",
        title: "Westminster Quarters",
        source: "attrib. William Crotch, Westminster Quarters (1793)",
        voices: WESTMINSTER,
    },
    Tune {
        id: "carillon",
        title: "Old Hundredth (carillon)",
        source: "Louis Bourgeois, OLD 100TH, Genevan Psalter (1551)",
        voices: CARILLON,
    },
    Tune {
        id: "music_box",
        title: "Ah ! vous dirai-je, maman",
        source: "French folk melody, trad. (first printed 1761)",
        voices: MUSIC_BOX,
    },
    Tune {
        id: "dance_band",
        title: "Oh! Susanna",
        source: "Stephen Foster, Oh! Susanna (1848)",
        voices: DANCE_BAND,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::synth::{render, rms};

    #[test]
    fn every_tune_parses() {
        for t in TUNES {
            for (i, voice) in t.voices.iter().enumerate() {
                if let Err(e) = mml::parse(voice.mml, 1) {
                    panic!("{} voice {i}: {e}", t.id);
                }
            }
        }
    }

    #[test]
    fn ids_are_unique_and_found() {
        for (i, t) in TUNES.iter().enumerate() {
            assert!(TUNES[..i].iter().all(|u| u.id != t.id), "{} twice", t.id);
            assert_eq!(get(t.id).unwrap().id, t.id);
        }
        assert!(get("no such tune").is_none());
    }

    #[test]
    fn lengths_in_range() {
        for t in TUNES {
            let l = t.length();
            assert!((25.0..=120.0).contains(&l), "{} is {l:.1} s", t.id);
        }
    }

    /// Every voice of a tune ends together: a bar dropped or doubled in
    /// one hand shows up here.
    #[test]
    fn voices_align() {
        for t in TUNES {
            let ends: Vec<f32> = t.parts(3).iter().map(|p| mml::length(&p.notes)).collect();
            let hi = ends.iter().cloned().fold(0.0, f32::max);
            let lo = ends.iter().cloned().fold(f32::MAX, f32::min);
            assert!(hi - lo < 0.5, "{}: voices end at {ends:?}", t.id);
        }
    }

    #[test]
    fn notes_in_each_instruments_range() {
        for t in TUNES {
            for p in t.parts(5) {
                let (lo, hi) = match p.instrument {
                    Piano => (21, 108),
                    Hum => (48, 72),
                    Bass => (28, 55),
                    Bell => (48, 84),
                    Organ | Harmonium => (36, 84),
                    Accordion => (48, 88),
                    Guitar => (40, 84),
                    MusicBox => (60, 96),
                    Beep => (0, 127),
                };
                for n in &p.notes {
                    assert!(
                        (lo..=hi).contains(&n.key),
                        "{}: {:?} key {} at {:.2} s",
                        t.id,
                        p.instrument,
                        n.key,
                        n.start
                    );
                }
            }
        }
    }

    /// The first few seconds of each tune render to something finite and
    /// audible (the whole of them would make the test slow).
    #[test]
    fn every_tune_sounds() {
        const CUT: f32 = 5.0;
        for t in TUNES {
            let parts: Vec<Part> = t
                .parts(7)
                .into_iter()
                .map(|mut p| {
                    p.notes.retain(|n| n.start < CUT);
                    for n in p.notes.iter_mut() {
                        n.len = n.len.min(CUT - n.start);
                    }
                    p
                })
                .collect();
            let x = render(&parts, 24000.0, 7);
            assert!(x.iter().all(|s| s.is_finite()), "{}", t.id);
            assert!(rms(&x) > 0.003, "{} is silent", t.id);
        }
    }
}
