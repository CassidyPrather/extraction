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

/// "the quarry", "the gatekeeper's cottage": a place in a sentence.
fn the(place: &str) -> String {
    crate::raid::lines::the(place)
}

/// "The quarry", at the start of a line.
fn cap_the(place: &str) -> String {
    let p = the(place);
    let mut c = p.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

// --- On the card -----------------------------------------------------------

/// A fetch: `n` x `name`, and why.
pub fn ask_fetch(n: u32, name: &str, why: &str) -> String {
    let them = if n == 1 { "it" } else { "them" };
    format!("{n} x {name}\n\"{why}\"\nStash {them}, press here.")
}

/// A recover, `place` as "in the barn by the Low farm." (or "somewhere in
/// the Hollow." before it's put out).
pub fn ask_recover(place: &str) -> String {
    format!(
        "\"One of the Authority's survey recorders, left {place} \
Somebody in town still pays for what's on them.\"\nStash it, press here."
    )
}

/// A scout of `place`.
pub fn ask_scout(place: &str) -> String {
    format!(
        "\"Go up to {} and look round. Tell me what's changed.\"",
        the(place)
    )
}

/// A scout, seen, not yet home.
pub fn scout_seen(place: &str) -> String {
    format!("{}: seen.\nGet home, press here.", cap_the(place))
}

/// The last job of all: Hana's letter.
pub const ASK_LETTER: &str = "\"Marrow. The house by the well. There's a letter on \
the table, for me. Bring it. Don't open it.\"\nStash it, press here.";

/// The first deep job: the depot's intake ledger.
pub const ASK_LEDGER: &str = "\"Under the Gate. Where they take everything. Their ledger \
for the village, bay K-14. Bring it.\"\nStash it, press here.";

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
pub const IN_THE_YARD: &str = "Driver: \"Not in the yard. Out there, for real.\"";
pub const NO_RECORDER: &str = "Driver: \"No recorder in the stash. Still out there, is it?\"";
pub const NO_LEDGER: &str = "Driver: \"Not in the stash. Bay K-14, under the Gate.\"";
pub const NO_LETTER: &str = "Driver: \"Not in the stash. The house by the well. The table.\"";

pub fn fetch_short(have: usize, n: u32, name: &str) -> String {
    format!("Driver: \"That's {have} of {n} {name}. In the stash, mind.\"")
}

pub fn scout_not_seen(place: &str) -> String {
    format!(
        "Driver: \"You've not been up to {} yet. Go and look.\"",
        the(place)
    )
}

// --- Done ---------------------------------------------------------------------

/// Thanks for a scout of `place`; `None` for the other kinds.
pub fn thanks(scouted: Option<&str>) -> String {
    match scouted {
        Some(place) => format!("{}, still standing. Good.", cap_the(place)),
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
pub const LETTER_HOME: &str =
    "Driver: \"...That's her hand.\" He turns it over. \"Give me a minute.\"";

/// Handing over the ledger (said before the thanks).
pub const LEDGER_HOME: &str = "Driver: \"...Marek, H. Kept.\" He shuts it. \"Kept.\"";

/// A tier done: something fitted. Each one says a little about him.
pub fn fitted(u: Upgrade) -> String {
    match u {
        Upgrade::StashRack => "Driver: \"Rack's up over the stash. Mind your head on it.\"",
        Upgrade::BenchVice => "Driver: \"Put a vice on the bench. It was my father's.\"",
        Upgrade::Aerial => "Driver: \"New aerial on the cab. I'll hear the Surge before you do.\"",
        Upgrade::MedCabinet => "Driver: \"Medicine cabinet. My sister's spare. Keep it full.\"",
        Upgrade::StashRack2 => "Driver: \"Another shelf. You've more in there than I own.\"",
    }
    .to_owned()
}

/// The end of the second tier: his carbine in the crate.
pub const GIFT_CARBINE: &str =
    "Driver: \"My old carbine's in the crate. Never been past the fence. Take it.\"";

/// The end of the third tier: the sub-machine gun.
pub const GIFT_SMG: &str =
    "Driver: \"Man in town owed me. There's a gun in the crate. Don't ask.\"";

/// What he says when a fitting comes with something in the crate.
pub fn gift(u: Upgrade) -> &'static str {
    match u {
        Upgrade::Aerial => GIFT_SMG,
        _ => GIFT_CARBINE,
    }
}

/// A fetch he's given up waiting for.
pub fn fetch_swapped(name: &str) -> String {
    format!("Driver: \"Forget the {name}. Something else on the card.\"")
}

// --- Upkeep and orders (after the ladder) ---------------------------------

/// "2 scrap metal", "1 electronics".
pub fn parts(parts: &[(&str, u32)], name: impl Fn(&str) -> String) -> String {
    parts
        .iter()
        .map(|(id, n)| format!("{n} {}", name(id)))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn upkeep_paid(fitting: &str, parts: &str) -> String {
    format!("Driver: \"Took {parts} for the {fitting}.\"")
}

pub fn laid_up(fitting: &str, parts: &str) -> String {
    format!("Driver: \"The {fitting}'s laid up. Wants {parts}.\"")
}

pub fn back_in_order(fitting: &str) -> String {
    format!("Driver: \"The {fitting}'s working again.\"")
}

/// The order card, past the ladder.
pub fn order_card(order: &str) -> String {
    format!("MY CACHES: {order}\n(press to change)")
}

/// The order card, before the ladder's done.
pub const ORDER_LATER: &str = "Driver: \"Do my jobs first. Then you can ask.\"";

pub fn ordered(order: &str) -> String {
    format!("Driver: \"{order}, then. I'll see what I can find.\"")
}

// --- Out there ----------------------------------------------------------------

pub fn scout_noticed(place: &str) -> String {
    format!("{}: noted. Now get it home.", cap_the(place))
}

pub fn scout_forgotten(place: &str) -> String {
    format!("What you saw at {} goes with you.", the(place))
}

/// Over a carried radio, with the aerial fitted.
pub const AERIAL_SURGE: &str = "Truck: \"Aerial's hissing. Surge in ten minutes, maybe less.\"";

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
            LEDGER_HOME.into(),
            NO_LEDGER.into(),
            GIFT_CARBINE.into(),
            GIFT_SMG.into(),
            fetch_swapped(name),
            scout_noticed(place),
            scout_forgotten(place),
            AERIAL_SURGE.into(),
            upkeep_paid("cabinet", "1 field dressing"),
            laid_up("cabinet", "1 field dressing"),
            back_in_order("cabinet"),
            ORDER_LATER.into(),
            ordered("Medicine"),
        ];
        lines.extend(Upgrade::ALL.iter().map(|u| fitted(*u)));
        assert_eq!(
            scout_noticed("The quarry"),
            "The quarry: noted. Now get it home."
        );
        let long: Vec<&String> = lines.iter().filter(|l| !fits_wrist(l)).collect();
        assert!(long.is_empty(), "over three lines of 26: {long:#?}");
    }
}
