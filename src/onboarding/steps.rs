//! What's said when the Zone steps up (`robots::pressure`): Onboarding's
//! words for Robot minds' steps. Each says first what to do, then why, in
//! the world's terms (a sound, a direction), and fits the wrist whole.

use crate::robots::pressure::{Level, Why};

/// The line for a step up to `level`, by what most moved it ("" for
/// Quiet: it never steps down with a word).
pub fn said(level: Level, why: Why) -> &'static str {
    match (level, why) {
        (Level::Quiet, _) => "",
        (Level::Stirred, Why::Time) => {
            "Time to move. You've been about too long: a whine's rising at the Gate."
        }
        (Level::Stirred, Why::Ground) => {
            "Go careful: you're on their ground. A whine's rising at the Gate."
        }
        (Level::Stirred, Why::Carried) => {
            "Get it home. What you carry draws them: a whine at the Gate."
        }
        (Level::Stirred, Why::Noise | Why::Fight) => {
            "Move off, quiet. They heard that: a whine's rising at the Gate."
        }
        (Level::Busy, _) => "Keep moving. Machine traffic on the wind, all of it this way.",
        (Level::Hunting, _) => "Get out now. The whole Zone is looking for you.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVELS: [Level; 3] = [Level::Stirred, Level::Busy, Level::Hunting];
    const WHYS: [Why; 5] = [Why::Time, Why::Noise, Why::Fight, Why::Ground, Why::Carried];

    /// Every step up says something, fits the wrist whole (three lines of
    /// 26, as the firsts), with no game words, and says what to do before
    /// why: its first sentence is short.
    #[test]
    fn every_step_says_what_to_do_and_fits_the_wrist() {
        const BANNED: [&str; 7] = ["loot", "quest", "spawn", "respawn", "hp", "xp", "pressure"];
        for level in LEVELS {
            for why in WHYS {
                let l = said(level, why);
                assert!(!l.is_empty(), "{level:?} {why:?} says nothing");
                let n = crate::render::screen::wrap(l, 26, usize::MAX).len();
                assert!(n <= 3, "{level:?} {why:?}: {n} lines: {l}");
                for w in l
                    .to_lowercase()
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| !w.is_empty())
                {
                    assert!(!BANNED.contains(&w), "{level:?} {why:?} says '{w}': {l}");
                }
                let first = l.split(['.', ':']).next().unwrap();
                assert!(
                    first.split_whitespace().count() <= 4,
                    "{level:?} {why:?}: what to do first, short: {l}"
                );
            }
        }
        assert_eq!(said(Level::Quiet, Why::Time), "");
    }

    /// Each step has its own words: a step up never reads as the one
    /// before it.
    #[test]
    fn each_step_reads_differently() {
        for why in WHYS {
            let lines: Vec<&str> = LEVELS.iter().map(|l| said(*l, why)).collect();
            for (i, a) in lines.iter().enumerate() {
                for b in &lines[i + 1..] {
                    assert_ne!(a, b, "{why:?}");
                }
            }
        }
    }

    /// What moved it is told true: lingering says so, their ground says
    /// so, what you carry says so, a noise or a fight says they heard.
    #[test]
    fn a_stirring_says_what_stirred_it() {
        assert!(said(Level::Stirred, Why::Time).contains("too long"));
        assert!(said(Level::Stirred, Why::Ground).contains("their ground"));
        assert!(said(Level::Stirred, Why::Carried).contains("carry"));
        for why in [Why::Noise, Why::Fight] {
            assert!(said(Level::Stirred, why).contains("heard"), "{why:?}");
        }
    }
}
