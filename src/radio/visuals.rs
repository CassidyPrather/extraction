//! How the music's things move: a cassette player's door swinging, its
//! keys going down (play stays latched while it plays), its reels turning
//! behind the window. Client only; the model's named nodes
//! (`art/items/music.py`) are what moves.

use bevy::prelude::*;
use extraction_core::music::deck::{Key, Mode};

use super::deck::{DOOR_OPEN, DeckKey, TapeDeck};
use crate::interact::poke::{PokeButton, TRAVEL};

/// A moving node's pose as modelled.
#[derive(Component, Clone, Copy)]
struct Rest(Transform);

/// How far a latched key sits down (m).
const LATCHED: f32 = 0.006;

/// Turns per second of a reel playing (a cassette's hub at 4.76 cm/s
/// turns between about a half and two turns a second).
const REEL_PLAYING: f32 = 0.8;

fn key_node(key: Key) -> &'static str {
    match key {
        Key::Play => "btn_play",
        Key::Stop => "btn_stop",
        Key::Rewind => "btn_rew",
        Key::FastForward => "btn_ff",
    }
}

fn animate_decks(
    mut commands: Commands,
    decks: Query<(Entity, &TapeDeck)>,
    keys: Query<(&DeckKey, &PokeButton)>,
    children: Query<&Children>,
    mut nodes: Query<(&Name, &mut Transform, Option<&Rest>)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, d) in &decks {
        // How far each key is down: pushed by a finger, or latched.
        let mut down = [(Key::Play, 0.0f32), (Key::Stop, 0.0), (Key::Rewind, 0.0), (Key::FastForward, 0.0)];
        for (k, b) in &keys {
            if k.deck != e {
                continue;
            }
            let latched = matches!(
                (k.key, d.deck.mode),
                (Key::Play, Mode::Playing)
                    | (Key::Rewind, Mode::Rewinding)
                    | (Key::FastForward, Mode::Winding)
            );
            let depth = b.depth.max(if latched { LATCHED } else { 0.0 });
            if let Some(slot) = down.iter_mut().find(|(key, _)| *key == k.key) {
                slot.1 = depth.min(TRAVEL);
            }
        }
        let reel_rate = match d.deck.mode {
            Mode::Playing => REEL_PLAYING,
            Mode::Rewinding => -REEL_PLAYING * 8.0,
            Mode::Winding => REEL_PLAYING * 8.0,
            Mode::Stopped => 0.0,
        } * std::f32::consts::TAU;
        for n in children.iter_descendants(e) {
            let Ok((name, mut t, rest)) = nodes.get_mut(n) else {
                continue;
            };
            let name = name.as_str();
            let moving = name == "door"
                || name.starts_with("btn_")
                || name == "reel_l"
                || name == "reel_r";
            if !moving {
                continue;
            }
            let rest = match rest {
                Some(r) => r.0,
                None => {
                    commands.entity(n).insert(Rest(*t));
                    *t
                }
            };
            if name == "door" {
                // Hinged on its bottom edge, the top swinging out (+Z).
                t.rotation = rest.rotation * Quat::from_rotation_x(DOOR_OPEN * d.door);
            } else if let Some((_, depth)) = down.iter().find(|(k, _)| key_node(*k) == name) {
                t.translation = rest.translation - Vec3::Y * *depth;
            } else if reel_rate != 0.0 {
                t.rotate_local_z(reel_rate * dt);
            }
        }
    }
}

/// The radio's knob shows where its dial is: as far round as the wrists
/// that turned it (`dial::KHZ_PER_RAD`), clockwise up the band.
fn turn_knobs(
    mut commands: Commands,
    radios: Query<(Entity, &super::dial::Tuner)>,
    children: Query<&Children>,
    mut nodes: Query<(&Name, &mut Transform, Option<&Rest>)>,
) {
    for (e, t) in &radios {
        let turn = -(t.khz - super::dial::OFF) / super::dial::KHZ_PER_RAD;
        for n in children.iter_descendants(e) {
            let Ok((name, mut tf, rest)) = nodes.get_mut(n) else {
                continue;
            };
            if name.as_str() != "dial" {
                continue;
            }
            let rest = match rest {
                Some(r) => r.0,
                None => {
                    commands.entity(n).insert(Rest(*tf));
                    *tf
                }
            };
            let want = rest.rotation * Quat::from_rotation_y(turn);
            if tf.rotation.angle_between(want) > 1e-4 {
                tf.rotation = want;
            }
        }
    }
}

pub struct MusicVisualsPlugin;

impl Plugin for MusicVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (animate_decks, turn_knobs));
    }
}
