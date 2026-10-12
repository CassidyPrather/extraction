//! Engine-free rules for the extraction game.
//!
//! Nothing in here knows about Bevy. Anything that is a *rule* — how much a
//! thing weighs, whether a magazine fits a gun, what a recipe makes — lives
//! here so it can be unit-tested in milliseconds and reused by the headless
//! simulations, the client, and (eventually) a server or a VRChat port.

pub mod cooking;
pub mod geom;
pub mod map;
pub mod noise;
pub mod terrain;
pub mod firearm;
pub mod inventory;
pub mod items;
