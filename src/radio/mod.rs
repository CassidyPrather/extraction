//! Radio and music: everything you hear as music is played by something
//! in the valleys. There is no soundtrack.
//!
//! - [`deck`]: a cassette player you carry, and the tapes you find.
//! - [`dial`]: the field radio's tuning knob, and the stations it finds.
//! - [`piano`]: an upright piano played with your fingertips.
//! - [`gramophone`]: a wind-up gramophone and its 78s.
//! - [`places`]: where the tapes lie.
//!
//! The rules (the tracker, the instruments, the mechanism) are engine-free
//! in `extraction_core::music`. [`RadioPlugin`] runs headless; the sound
//! is [`sound::MusicSoundPlugin`], added with the audio.

pub mod deck;
pub mod dial;
pub mod gramophone;
pub mod piano;
pub mod places;
pub mod sound;
pub mod visuals;

use bevy::prelude::*;

/// Music is playing within [`MUSIC_NEAR`] of the local player: a tape, a
/// station on a radio out loud, a gramophone turning, a piano just
/// played. The score (`audio_score`) steps aside for it: one musical
/// layer at a time.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct MusicNear(pub bool);

pub const MUSIC_NEAR: f32 = 15.0;

/// How long after its last note a piano still counts as playing (s).
const PIANO_LINGERS: f32 = 6.0;

fn music_near(
    mut near: ResMut<MusicNear>,
    players: Query<&GlobalTransform, With<crate::rig::LocalPlayer>>,
    decks: Query<(&deck::TapeDeck, &GlobalTransform)>,
    radios: Query<(&dial::Tuner, &GlobalTransform)>,
    grams: Query<(&gramophone::Gramophone, &GlobalTransform)>,
    mut notes: MessageReader<piano::PianoNote>,
    time: Res<Time>,
    mut last_note: Local<Option<(Vec3, f32)>>,
) {
    let now = time.elapsed_secs();
    if let Some(n) = notes.read().filter(|n| n.on).last() {
        *last_note = Some((n.at, now));
    }
    let Some(me) = players.iter().next().map(|g| g.translation()) else {
        return;
    };
    let close = |at: Vec3| at.distance(me) < MUSIC_NEAR;
    let playing = decks.iter().any(|(d, g)| {
        d.deck.mode == extraction_core::music::deck::Mode::Playing && close(g.translation())
    }) || radios.iter().any(|(t, g)| {
        t.on()
            && t.aloud
            && t.reception.stations.iter().any(|s| s.gain > 0.3)
            && close(g.translation())
    }) || grams
        .iter()
        .any(|(gr, g)| gr.speed > 0.1 && close(g.translation()))
        || last_note.is_some_and(|(at, t)| now - t < PIANO_LINGERS && close(at));
    if near.0 != playing {
        near.0 = playing;
    }
}

pub struct RadioPlugin;

impl Plugin for RadioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MusicNear>()
            .add_message::<deck::DeckEvent>()
            .add_message::<dial::DialClick>()
            .add_message::<piano::PianoNote>()
            .add_observer(deck::deck_parts)
            .add_observer(dial::tuner_parts)
            .add_systems(
                Update,
                (
                    deck::mount_buttons,
                    deck::fit_cells,
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
            )
            .add_systems(Update, (places::stand_fixtures, piano::play_pianos))
            .add_systems(
                Update,
                (gramophone::work_gramophones, gramophone::run_gramophones).chain(),
            )
            .add_systems(Update, music_near)
            .add_systems(
                PostUpdate,
                deck::note_turns.after(bevy::transform::TransformSystems::Propagate),
            );
    }
}

#[cfg(test)]
mod lure_tests;
#[cfg(test)]
mod tests;
