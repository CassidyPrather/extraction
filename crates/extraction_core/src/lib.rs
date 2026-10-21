//! Engine-free rules for the extraction game.
//!
//! Nothing in here knows about Bevy. Anything that is a *rule* — how much a
//! thing weighs, whether a magazine fits a gun, what a recipe makes — lives
//! here so it can be unit-tested in milliseconds and reused by the headless
//! simulations, the client, and (eventually) a server or a VRChat port.

pub mod ammo;
pub mod anomaly;
pub mod contracts;
pub mod cooking;
pub mod cover;
pub mod fabrication;
pub mod firearm;
pub mod firearm_terminal;
pub mod fit;
pub mod forest;
pub mod fracture;
pub mod geom;
pub mod grenade;
pub mod heavy;
pub mod inventory;
pub mod items;
pub mod loot;
pub mod map;
pub mod map_doors;
pub mod map_landmarks;
pub mod map_lint;
pub mod map_nav;
pub mod map_sentry;
pub mod map_sight;
pub mod map_surge;
pub mod melee;
pub mod mender;
pub mod minelayer;
pub mod music;
pub mod nav;
pub mod noise;
pub mod progression;
pub mod rocks;
pub mod safehouse;
pub mod save;
pub mod sentry;
pub mod spotter;
pub mod terrain;
pub mod underground;
pub mod unity;
