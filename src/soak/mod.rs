//! Soak and fuzz: what breaks over long play and odd input.
//!
//! - [`census`]: counts what the world holds (entities, bodies, assets,
//!   voices, memory, tick time, save size) over a long run, and a gauge
//!   that fails when any of it keeps growing.
//! - `chaos` (tests): a monkey at the headset's controls doing random but
//!   plausible things with its hands.
//! - `save_fuzz` (tests): saving at odd moments, and mangled save files.
//! - `physics` (tests): thrown, stacked and buried items.
//!
//! Findings and fixes are written up in `docs/SOAK.md`.

pub mod census;
#[cfg(test)]
mod chaos;
#[cfg(test)]
mod physics;
#[cfg(test)]
mod save_fuzz;
