//! A small, copy-pasteable cooking system: foods, processes, recipes, and a
//! station that turns one into another.
//!
//! This module is meant to be lifted out whole and dropped into another
//! game as a starting point — for a resource pipeline that wants tuning, a
//! survival game that wants meals, a shop that wants something to sell. It
//! uses nothing but `std` and refers to its own files only through `super`,
//! so copying `cooking.rs` and the `cooking/` directory is the whole
//! install; a test at the bottom keeps it that way. Rename it, prune the
//! foods, retune the recipes: it is data first and code second.
//!
//! Kitchen Garden (`kitchen_garden`) is the demo built on it, and shows one
//! way to put gathering, scarcity and opportunity cost around it.
//!
//! - [`food`] — every food: raw ingredients, what they become, dishes, and
//!   the two kinds of failure.
//! - [`process`] — what can be done to food, in three kinds: *physical*
//!   (driven by work: chopping, kneading), *heat* (driven by the clock, and
//!   it burns if left), and *time* (driven by the clock, and it waits).
//! - [`recipe`] — the table: which inputs, under which process, make what,
//!   and how much work or time it takes.
//! - [`station`] — the state machine: a place food goes in, gets processed,
//!   and comes out. One per chopping board, pot, oven or crock.
//! - [`stock`] — a count of every food, for pantries, bills and shops.
//! - [`bill`] — what a food costs from the ground up: raw ingredients,
//!   strokes of work, ticks of heat and of waiting. The hook for tuning.
//!
//! Time is counted in integer *ticks*, so a game driving this on a fixed
//! step stays deterministic. [`TICKS_PER_SEC`] is only the scale the recipe
//! table is written in; change it and every duration rescales with it.

pub mod bill;
pub mod food;
pub mod process;
pub mod recipe;
pub mod station;
pub mod stock;

pub use bill::{Bill, bill};
pub use food::{Food, Group, Stage};
pub use process::{Drive, Kind, Process};
pub use recipe::{RECIPES, Recipe};
pub use station::{Event, Refusal, State, Station};
pub use stock::Stock;

/// How many ticks the recipe table counts to a second. The demo advances
/// its stations at exactly this rate.
pub const TICKS_PER_SEC: u32 = 20;

/// `n` seconds, in ticks.
#[must_use]
pub const fn secs(n: u32) -> u32 {
    n * TICKS_PER_SEC
}

#[cfg(test)]
mod tests {
    /// Every file of the module, as text.
    const SOURCES: [(&str, &str); 7] = [
        ("cooking.rs", include_str!("cooking.rs")),
        ("bill.rs", include_str!("cooking/bill.rs")),
        ("food.rs", include_str!("cooking/food.rs")),
        ("process.rs", include_str!("cooking/process.rs")),
        ("recipe.rs", include_str!("cooking/recipe.rs")),
        ("station.rs", include_str!("cooking/station.rs")),
        ("stock.rs", include_str!("cooking/stock.rs")),
    ];

    #[test]
    fn the_module_can_be_copied_out_whole() {
        // Assembled, so this test does not trip over its own needles.
        let needles = [
            ["cra", "te::"].concat(),
            ["macro", "quad"].concat(),
            ["fast", "rand"].concat(),
            ["extern ", "crate"].concat(),
        ];
        for (file, text) in SOURCES {
            for needle in &needles {
                assert!(
                    !text.contains(needle.as_str()),
                    "{file} mentions `{needle}`; the module must only need std and itself"
                );
            }
        }
    }
}
