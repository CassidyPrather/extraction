//! Everything the driver says about his jobs, in one place, so the words
//! can be written apart from the logic (Story owns the wording; see
//! `docs/LORE.md`, "The driver"). Lines on the wrist (`raid.say`) keep to
//! three lines of 26 columns; card text is fitted to the card. The tests
//! below hold the wrist lines to that with the longest names in the game.
//!
//! The "why" of each fetch is with the fetches themselves, in
//! `extraction_core::progression::WANTS`; tier and fitting names in
//! `Tier::name` and `Upgrade::name` there.

use extraction_core::progression::Upgrade;

// --- On the card -----------------------------------------------------------

/// A fetch: `n` x `name`, and why.
pub fn ask_fetch(n: u32, name: &str, why: &str) -> String {
    let them = if n == 1 { "it" } else { "them" };
    format!("{n} x {name}\n\"{why}\"\nStash {them}, press here.")
}

/// A recover, `place` as "in the barn by the Low farm." (or "somewhere in
/// the Hollow." before it's put out).
pub fn ask_recover(place: &str) -> String {
    format!("\"A surveyor's recorder, left {place} Bring it home.\"\nStash it, press here.")
}

/// A scout of `place`.
pub fn ask_scout(place: &str) -> String {
    format!("\"Walk up to the {place} and have a look round. Come back and tell me.\"")
}

/// A scout, seen, not yet home.
pub fn scout_seen(place: &str) -> String {
    format!("The {place}: seen.\nGet home, press here.")
}

/// The last job of all: Hana's letter.
pub const ASK_LETTER: &str = "\"There's a house by the well in the village. On the table. \
Bring me what's there.\"\nStash it, press here.";

/// Before the job, when it's in another valley.
pub fn elsewhere(valley: &str) -> String {
    format!("In {valley}: ")
}

/// Under the job, when it's the last of a tier.
pub fn promise(fitting: &str) -> String {
    format!("Do this and I'll fit {fitting}.")
}

pub const NOTHING_YET: &str = "Nothing yet.";

// --- Pressing the card, not done ------------------------------------------

pub const NO_JOB: &str = "Driver: \"Nothing else for now. Go on.\"";
pub const NOT_AT_BASE: &str = "Driver: \"Back at base. Tell me there.\"";
pub const ONE_A_RAID: &str = "Driver: \"One thing at a time. After your next run.\"";
pub const IN_THE_YARD: &str = "Driver: \"That's for when we're out there for real.\"";
pub const NO_RECORDER: &str = "Driver: \"No recorder in the stash. Still out there, is it?\"";
pub const NO_LETTER: &str = "Driver: \"Not in the stash. The table, by the well.\"";

pub fn fetch_short(have: usize, n: u32, name: &str) -> String {
    format!("Driver: \"That's {have} of {n} {name}. In the stash, mind.\"")
}

pub fn scout_not_seen(place: &str) -> String {
    format!("Driver: \"You've not been to the {place} yet. Go and look.\"")
}

// --- Done ---------------------------------------------------------------------

/// Thanks for a scout of `place`; `None` for the other kinds.
pub fn thanks(scouted: Option<&str>) -> String {
    match scouted {
        Some(place) => format!("The {place}, eh. Good to know."),
        None => "Ta.".to_owned(),
    }
}

/// Thanks, and where his cache is.
pub fn paid(thanks: &str, cache: &str) -> String {
    format!("Driver: \"{thanks} My cache: {cache} It's on your map.\"")
}

/// Thanks, with no cache to give (no building far enough off).
pub fn paid_nothing(thanks: &str) -> String {
    format!("Driver: \"{thanks} I owe you one.\"")
}

/// Handing over Hana's letter (said instead of the thanks).
pub const LETTER_HOME: &str = "Driver: \"...That's her hand. Give me a minute.\"";

/// A tier done: something fitted.
pub fn fitted(u: Upgrade) -> String {
    format!("Driver: \"And I've put up {}. Have a look.\"", u.name())
}

/// The end of the second tier: his carbine in the crate.
pub const GIFT_CARBINE: &str = "Driver: \"And my old carbine's in the crate. I'm past it.\"";

/// A fetch he's given up waiting for.
pub fn fetch_swapped(name: &str) -> String {
    format!("Driver: \"Forget the {name}. New job on the card.\"")
}

// --- Out there ----------------------------------------------------------------

pub fn scout_noticed(place: &str) -> String {
    format!("The {place}: noted. Now get it home.")
}

pub fn scout_forgotten(place: &str) -> String {
    format!("What you saw at the {place} goes with you.")
}

/// Over a carried radio, with the aerial fitted.
pub const AERIAL_SURGE: &str = "Truck: \"Aerial's picking up the Surge. Ten minutes, maybe less.\"";

#[cfg(test)]
mod tests {
    use super::*;

    /// The wrist shows three lines of 26 (`raid::lines` holds its own to
    /// the same).
    fn fits_wrist(line: &str) -> bool {
        crate::render::screen::wrap_lines(line, 26).lines().count() <= 3
    }

    #[test]
    fn wrist_lines_fit_the_wrist() {
        // The longest names the jobs can carry.
        let place = "Gatekeeper's cottage";
        let name = "Robot ammunition belt";
        let mut lines: Vec<String> = vec![
            NO_JOB.into(),
            NOT_AT_BASE.into(),
            ONE_A_RAID.into(),
            IN_THE_YARD.into(),
            NO_RECORDER.into(),
            NO_LETTER.into(),
            fetch_short(2, 3, name),
            scout_not_seen(place),
            paid_nothing(&thanks(Some(place))),
            LETTER_HOME.into(),
            GIFT_CARBINE.into(),
            fetch_swapped(name),
            scout_noticed(place),
            scout_forgotten(place),
            AERIAL_SURGE.into(),
        ];
        lines.extend(Upgrade::ALL.iter().map(|u| fitted(*u)));
        let long: Vec<&String> = lines.iter().filter(|l| !fits_wrist(l)).collect();
        assert!(long.is_empty(), "over three lines of 26: {long:#?}");
    }
}
