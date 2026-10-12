//! The game client. See `extraction::launch` for the command line.

use bevy::prelude::*;

fn main() -> AppExit {
    extraction::run(extraction::LaunchOptions::from_args())
}
