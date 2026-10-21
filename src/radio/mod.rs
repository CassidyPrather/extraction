//! Radio and music: everything you hear as music is played by something
//! in the valleys. There is no soundtrack.
//!
//! - [`deck`]: a cassette player you carry, and the tapes you find.
//! - [`dial`]: the field radio's tuning knob, and the stations it finds.
//!
//! The rules (the tracker, the instruments, the mechanism) are engine-free
//! in `extraction_core::music`. [`RadioPlugin`] runs headless; the sound
//! is [`sound::MusicSoundPlugin`], added with the audio.

pub mod deck;
pub mod dial;
pub mod sound;
pub mod visuals;

use bevy::prelude::*;

pub struct RadioPlugin;

impl Plugin for RadioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<deck::DeckEvent>()
            .add_message::<dial::DialClick>()
            .add_observer(deck::deck_parts)
            .add_observer(dial::tuner_parts)
            .add_systems(
                Update,
                (
                    deck::mount_buttons,
                    deck::press_keys,
                    deck::load_tapes,
                    deck::run_decks,
                    deck::deck_noise,
                )
                    .chain()
                    .after(crate::items::usage::UsageSystems),
            )
            .add_systems(
                Update,
                (dial::turn_dials, dial::receive, dial::radio_noise).chain(),
            );
    }
}

#[cfg(test)]
mod tests;
