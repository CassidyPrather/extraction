//! A tiny tracker: tunes written in Music Macro Language, the notation old
//! home computers played tunes from. One string per part:
//!
//! ```text
//! t96 o4 l8 v12  c d e f g4 g4 | a a a a g2 | > c4 < b4 a4 r4 & a2
//! ```
//!
//! - `a`-`g` a note, then `+`/`#` sharp or `-` flat, then an optional
//!   length (`4` a crotchet, `8` a quaver, `16`...) and dots (`.`);
//! - `r` a rest, with the same length rules;
//! - `&` ties the next note on (or extends a rest) without a new attack;
//! - `o3` octave, `<` down one, `>` up one;
//! - `l8` the default length, `t120` tempo (crotchets a minute),
//!   `v0`-`v15` loudness;
//! - `|` and whitespace are ignored (bar lines, for reading);
//! - `[ ... ]2` repeats what's inside (nesting allowed);
//! - `~` a little rubato: this note is played late by a hair (a person
//!   playing, not a sequencer); `!` a fumble: a wrong neighbour note
//!   played first and corrected (a learner).
//!
//! Parsing is total: anything it doesn't know is an error with its place,
//! so a typo in a tune fails the tests, not the player's ears.

/// One note of a part: when, how long, what pitch, how hard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    /// Seconds from the tune's start.
    pub start: f32,
    /// Seconds it's held.
    pub len: f32,
    /// MIDI note number (60 = middle C).
    pub key: i32,
    /// 0..1.
    pub vel: f32,
}

impl Note {
    pub fn freq(&self) -> f32 {
        midi_freq(self.key as f32)
    }
}

pub fn midi_freq(key: f32) -> f32 {
    440.0 * ((key - 69.0) / 12.0).exp2()
}

#[derive(Debug, PartialEq)]
pub struct MmlError {
    pub at: usize,
    pub what: String,
}

impl std::fmt::Display for MmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at {}: {}", self.at, self.what)
    }
}

/// Expands `[...]n` repeats.
fn expand(src: &str) -> Result<String, MmlError> {
    fn go(chars: &[char], i: &mut usize, depth: usize) -> Result<String, MmlError> {
        let mut out = String::new();
        while *i < chars.len() {
            let c = chars[*i];
            if c == '[' {
                *i += 1;
                let inner = go(chars, i, depth + 1)?;
                if *i >= chars.len() || chars[*i] != ']' {
                    return Err(MmlError {
                        at: *i,
                        what: "unclosed [".into(),
                    });
                }
                *i += 1;
                let mut n = 0usize;
                let mut any = false;
                while *i < chars.len() && chars[*i].is_ascii_digit() {
                    n = n * 10 + chars[*i].to_digit(10).unwrap() as usize;
                    *i += 1;
                    any = true;
                }
                let n = if any { n } else { 2 };
                for _ in 0..n {
                    out.push_str(&inner);
                    out.push(' ');
                }
            } else if c == ']' {
                if depth == 0 {
                    return Err(MmlError {
                        at: *i,
                        what: "] without [".into(),
                    });
                }
                return Ok(out);
            } else {
                out.push(c);
                *i += 1;
            }
        }
        Ok(out)
    }
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    go(&chars, &mut i, 0)
}

/// Parses one part. `seed` makes the rubato and fumbles repeatable.
pub fn parse(src: &str, seed: u64) -> Result<Vec<Note>, MmlError> {
    let text = expand(src)?;
    let chars: Vec<char> = text.chars().collect();
    let mut rng = crate::noise::Rng::new(seed ^ 0x3311);
    let mut out: Vec<Note> = Vec::new();
    let mut t = 0.0f32;
    let mut octave = 4i32;
    let mut default_len = 4.0f32;
    let mut tempo = 120.0f32;
    let mut vel = 0.75f32;
    let mut tie = false;
    let mut late = false;
    let mut fumble = false;
    let mut i = 0usize;
    let number = |chars: &[char], i: &mut usize| -> Option<f32> {
        let s = *i;
        while *i < chars.len() && (chars[*i].is_ascii_digit()) {
            *i += 1;
        }
        (s != *i).then(|| chars[s..*i].iter().collect::<String>().parse().unwrap())
    };
    let dots = |chars: &[char], i: &mut usize, base: f32| -> f32 {
        let mut len = base;
        let mut add = base;
        while *i < chars.len() && chars[*i] == '.' {
            add *= 0.5;
            len += add;
            *i += 1;
        }
        len
    };
    while i < chars.len() {
        let c = chars[i].to_ascii_lowercase();
        let at = i;
        i += 1;
        match c {
            ' ' | '\n' | '\t' | '\r' | '|' => {}
            'a'..='g' | 'r' => {
                let mut key = None;
                if c != 'r' {
                    let base = match c {
                        'c' => 0,
                        'd' => 2,
                        'e' => 4,
                        'f' => 5,
                        'g' => 7,
                        'a' => 9,
                        _ => 11,
                    };
                    let mut k = 12 * (octave + 1) + base;
                    while i < chars.len() && matches!(chars[i], '+' | '#' | '-') {
                        k += if chars[i] == '-' { -1 } else { 1 };
                        i += 1;
                    }
                    key = Some(k);
                }
                let div = number(&chars, &mut i).unwrap_or(default_len);
                if div <= 0.0 {
                    return Err(MmlError {
                        at,
                        what: "zero length".into(),
                    });
                }
                let beats = dots(&chars, &mut i, 4.0 / div);
                let secs = beats * 60.0 / tempo;
                match key {
                    Some(k) if tie => {
                        if let Some(last) = out.last_mut().filter(|n| n.key == k) {
                            last.len += secs;
                        } else {
                            return Err(MmlError {
                                at,
                                what: "& ties to a different note".into(),
                            });
                        }
                    }
                    Some(k) => {
                        let mut start = t;
                        if late {
                            start += rng.range(0.02, 0.06);
                        }
                        if fumble {
                            // A wrong neighbour, quickly corrected.
                            let wrong = k + if rng.chance(0.5) { 1 } else { -1 };
                            let f = (secs * 0.25).min(0.12);
                            out.push(Note {
                                start,
                                len: f,
                                key: wrong,
                                vel: vel * 0.8,
                            });
                            start += f + 0.05;
                        }
                        out.push(Note {
                            start,
                            len: (secs - (start - t)).max(0.03),
                            key: k,
                            vel,
                        });
                    }
                    None => {}
                }
                t += secs;
                tie = false;
                late = false;
                fumble = false;
            }
            '&' => tie = true,
            '~' => late = true,
            '!' => fumble = true,
            '<' => octave -= 1,
            '>' => octave += 1,
            'o' | 'l' | 't' | 'v' => {
                let Some(n) = number(&chars, &mut i) else {
                    return Err(MmlError {
                        at,
                        what: format!("{c} needs a number"),
                    });
                };
                match c {
                    'o' => octave = n as i32,
                    'l' => default_len = dots(&chars, &mut i, n),
                    't' => tempo = n.max(1.0),
                    _ => vel = (n / 15.0).clamp(0.0, 1.0),
                }
            }
            other => {
                return Err(MmlError {
                    at,
                    what: format!("unknown '{other}'"),
                });
            }
        }
    }
    Ok(out)
}

/// When the last note of a part ends (s).
pub fn length(notes: &[Note]) -> f32 {
    notes.iter().map(|n| n.start + n.len).fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_and_lengths() {
        let n = parse("t60 o4 l4 c d e8 f8 g2.", 0).unwrap();
        assert_eq!(
            n.iter().map(|n| n.key).collect::<Vec<_>>(),
            [60, 62, 64, 65, 67]
        );
        assert_eq!(n[0].start, 0.0);
        assert_eq!(n[2].len, 0.5);
        assert!((n[4].start - 3.0).abs() < 1e-5);
        assert!((n[4].len - 3.0).abs() < 1e-5);
    }

    #[test]
    fn accidentals_octaves_ties_rests() {
        let n = parse("o4 c+ d- > c < b r c&c", 0).unwrap();
        let keys: Vec<i32> = n.iter().map(|n| n.key).collect();
        assert_eq!(keys, [61, 61, 72, 71, 60]);
        // The tied note is one note, twice as long.
        assert!((n[4].len - 1.0).abs() < 1e-5);
        // The rest left a gap.
        assert!((n[4].start - 2.5).abs() < 1e-5);
    }

    #[test]
    fn repeats_nest() {
        let n = parse("[c [d]3]2", 0).unwrap();
        assert_eq!(n.len(), 8);
    }

    #[test]
    fn errors_say_where() {
        assert_eq!(parse("c x", 0).unwrap_err().at, 2);
        assert!(parse("[c", 0).is_err());
        assert!(parse("c ]", 0).is_err());
        assert!(parse("c & d", 0).is_err());
        assert!(parse("o", 0).is_err());
    }

    #[test]
    fn a_fumble_is_a_wrong_note_then_the_right_one() {
        let n = parse("l4 !e", 7).unwrap();
        assert_eq!(n.len(), 2);
        assert_eq!((n[0].key - 64).abs(), 1);
        assert_eq!(n[1].key, 64);
        assert!(n[1].start > n[0].start);
    }
}
