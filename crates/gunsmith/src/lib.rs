//! The gunsmith's workshop: each gun judged on its own, away from the game.
//!
//! - [`firearm`]: the mechanisms as plain state machines (what the hands
//!   work, what the trigger does, the faults and their cures).
//! - [`parts`]: how each named part of a model moves with the mechanism
//!   (the bolt carrier back with the handle, the selector through its
//!   detents, the trigger about its pin), as engine-free poses the game and
//!   the bench both apply.
//! - [`ammo`], [`cartridge`], [`build`], [`dice`]: what the mechanism needs
//!   to know about the rounds, the gun's alterations and its luck.
//!
//! The bench (`cargo run -p gunsmith --example bench -- ak74`) puts one gun
//! at hand distance under the game's light; `docs/craft/GUNSMITHING.md` is
//! the procedure for judging it.

pub mod ammo;
pub mod build;
pub mod cartridge;
pub mod dice;
pub mod finish;
pub mod firearm;
pub mod parts;
pub mod spec;
