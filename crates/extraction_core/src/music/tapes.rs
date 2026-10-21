//! The tapes in the valleys: who made each, what's on its two sides, and
//! how worn it is. Each is its own item (`tape_*` in `items.ron`), so a
//! loot table or a staged scene can place a particular one.
//!
//! A side is a list of takes: a tune (from [`super::tunes`]) or a stretch
//! of it, with the gaps a person leaves pressing record and pause. What's
//! after the last take is blank tape, hissing, to the side's end.

use super::deck::Side;
use super::{media, synth, tunes};

/// One stretch of a side.
#[derive(Clone, Copy, Debug)]
pub struct Take {
    pub tune: &'static str,
    /// Seconds into the tune it starts (taped from the middle) and where
    /// it's cut (the tape ran out, the button was pressed): `None` to the
    /// end.
    pub from: f32,
    pub to: Option<f32>,
    /// Seconds of blank tape before it.
    pub gap: f32,
}

const fn take(tune: &'static str) -> Take {
    Take {
        tune,
        from: 0.0,
        to: None,
        gap: 2.0,
    }
}

const fn cut(tune: &'static str, to: f32) -> Take {
    Take {
        tune,
        from: 0.0,
        to: Some(to),
        gap: 2.0,
    }
}

#[derive(Debug)]
pub struct TapeDef {
    /// Its item id.
    pub item: &'static str,
    /// What's written on its label, side A and side B.
    pub labels: [&'static str; 2],
    pub sides: [&'static [Take]; 2],
    /// 0 fresh, 1 worn out.
    pub wear: f32,
}

/// Blank tape after the last take, at least (s).
pub const RUN_OUT: f32 = 6.0;

/// Who made each, where it lies, and why (`docs/LORE.md`; owners and
/// labels are Story's).
pub const TAPES: &[TapeDef] = &[
    // Hana Marek, the house by the well. Anna, Lev's sister, as a girl at
    // the upright: the stumble and the restart. Side B, Hana humming to
    // Rusty.
    TapeDef {
        item: "tape_practice",
        labels: ["for Anna - practice", "B"],
        sides: [
            &[take("minuet_g_learner"), take("bratre_kubo"), cut("fur_elise", 30.0)],
            &[take("brahms_lullaby")],
        ],
        wear: 0.5,
    },
    // The sexton, Kettle chapel: the harmonium, one verse each.
    TapeDef {
        item: "tape_hymns",
        labels: ["HARVEST HYMNS", "ADVENT"],
        sides: [&[take("old_hundredth"), take("largo")], &[take("silent_night")]],
        wear: 0.3,
    },
    // Old Fiala, the hunting lodge: dances taped off the wireless.
    TapeDef {
        item: "tape_lodge",
        labels: ["Lodge dance (off the wireless)", "Lodge dance 2"],
        sides: [
            &[take("polka"), take("waltz_folk")],
            &[take("dance_band"), take("greensleeves")],
        ],
        wear: 0.7,
    },
    // A soldier, Marrow's south gate booth: it runs out mid-song.
    TapeDef {
        item: "tape_duty",
        labels: ["DUTY MIX 2", "DUTY MIX 2 B"],
        sides: [
            &[take("dance_band"), take("ragtime")],
            &[take("polka"), cut("ragtime", 35.0)],
        ],
        wear: 0.6,
    },
    // Vlasta Kralova, Cinder: her piano went into the furnace; this is
    // what's left of it. A slow waltz, twice.
    TapeDef {
        item: "tape_birthday",
        labels: ["Our piano. Ruben's birthday.", "B"],
        sides: [&[take("gymnopedie"), take("gymnopedie")], &[take("minuet_g")]],
        wear: 0.4,
    },
    // Alois Pech, Sallow: his copy of his father's wire recording of Old
    // Sallow before the water. Hiss, the bells, a few bars of the organ,
    // and the copy cuts off.
    TapeDef {
        item: "tape_bells",
        labels: ["Old Sallow bells - from Dad's wire", ""],
        sides: [
            &[take("bells_rounds"), cut("hymn_to_joy", 14.0)],
            &[take("carillon")],
        ],
        wear: 0.95,
    },
];

pub fn get(item: &str) -> Option<&'static TapeDef> {
    TAPES.iter().find(|t| t.item == item)
}

fn take_len(t: &Take) -> f32 {
    let full = tunes::get(t.tune).map_or(0.0, |tune| tune.length());
    let end = t.to.map_or(full, |to| to.min(full));
    t.gap + (end - t.from).max(0.0)
}

/// How long a side is (s). Both sides of a cassette are the same length:
/// the longer side's, plus the run-out.
pub fn side_length(def: &TapeDef) -> f32 {
    def.sides
        .iter()
        .map(|s| s.iter().map(take_len).sum::<f32>())
        .fold(0.0, f32::max)
        + RUN_OUT
}

/// A side as you'd hear it from the tape: the takes in order with their
/// gaps, through a cassette of the tape's wear, levelled to an RMS of
/// -20 dBFS (about where music sits after mastering), mono at `sr`.
pub fn render_side(def: &TapeDef, side: Side, sr: f32) -> Vec<f32> {
    let len = side_length(def);
    let mut out = vec![0.0f32; (len * sr) as usize];
    let mut at = 0.0f32;
    let seed = def.item.bytes().fold(0u64, |h, b| h.wrapping_mul(31) ^ b as u64);
    for (k, t) in def.sides[side.index()].iter().enumerate() {
        at += t.gap;
        let Some(tune) = tunes::get(t.tune) else {
            continue;
        };
        let audio = synth::render(&tune.parts(seed ^ k as u64), sr, seed ^ k as u64);
        let from = (t.from * sr) as usize;
        let end = t
            .to
            .map_or(audio.len(), |to| ((to * sr) as usize).min(audio.len()));
        let start = (at * sr) as usize;
        for (i, s) in audio[from.min(end)..end].iter().enumerate() {
            if let Some(o) = out.get_mut(start + i) {
                *o += *s;
            }
        }
        // A cut take ends with the button's clunk of silence, not a fade.
        at += (end.saturating_sub(from)) as f32 / sr;
    }
    let level = synth::rms(&out[..((at * sr) as usize).min(out.len())]).max(1e-6);
    let gain = 0.1 / level;
    for s in out.iter_mut() {
        *s *= gain;
    }
    let mut out = out;
    media::tape(&mut out, sr, seed ^ side.index() as u64, def.wear);
    out.truncate((len * sr) as usize);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_take_is_a_tune() {
        for def in TAPES {
            assert!(def.item.starts_with("tape_"), "{}", def.item);
            for side in def.sides {
                assert!(!side.is_empty(), "{}", def.item);
                for t in side.iter() {
                    assert!(tunes::get(t.tune).is_some(), "{}: no tune {}", def.item, t.tune);
                }
            }
            let len = side_length(def);
            assert!((40.0..=600.0).contains(&len), "{}: {len} s", def.item);
        }
        let mut ids: Vec<_> = TAPES.iter().map(|t| t.item).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), TAPES.len());
    }

    /// The music is where the side says, and the run-out is hiss.
    #[test]
    fn a_side_plays_then_hisses() {
        let sr = 8000.0;
        let def = &TAPES[0];
        let x = render_side(def, Side::A, sr);
        assert_eq!(x.len(), (side_length(def) * sr) as usize);
        let music = synth::rms(&x[(4.0 * sr) as usize..(10.0 * sr) as usize]);
        let tail = synth::rms(&x[x.len() - (2.0 * sr) as usize..]);
        assert!(music > 0.03, "{music}");
        assert!(tail < music * 0.2, "{tail} vs {music}");
        assert!(tail > 0.0);
    }
}
