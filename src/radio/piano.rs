//! An upright piano you play with your fingers: point (the trigger off,
//! the hand empty) and push a key down. Each forefinger is one finger on
//! the keyboard, so it's two-finger playing, which is what most people do
//! at somebody else's piano. A key sounds as it bottoms out, louder the
//! faster it went down, and is damped when the finger lifts. The robots
//! hear it: a piano is loud.
//!
//! The model is `art/props/music.py`'s `upright_piano`: 88 keys, white
//! tops at 0.72 m, keys pivoting at their back ends, the front -Z.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::interact::Hand;
use crate::interact::poke::FINGERTIP;
use crate::rig::{LocalPlayer, RigDriver, RigHand, RigInput};

/// The lowest and highest keys (MIDI).
pub const LOW: u8 = 21;
pub const HIGH: u8 = 108;
/// White keys' pitch (m), and the keyboard's centre line.
pub const WHITE: f32 = 0.023;
/// White keys' tops, and the black keys' (m, above the floor).
pub const WHITE_TOP: f32 = 0.72;
pub const BLACK_TOP: f32 = 0.732;
/// The keys' back pivot, the black keys' and white keys' front ends (z).
pub const BACK: f32 = -0.065;
pub const BLACK_FRONT: f32 = -0.16;
pub const WHITE_FRONT: f32 = -0.215;
/// A black key's half width (m).
pub const BLACK_HALF: f32 = 0.00575;
/// How far a key goes down (m), and where along it it sounds.
pub const TRAVEL: f32 = 0.01;
pub const SOUNDS_AT: f32 = 0.006;
/// How far a robot hears a key struck hard (m).
pub const HEARD_AT: f32 = 30.0;

/// A piano, and its keys' state.
#[derive(Component, Debug, Default)]
pub struct Piano {
    /// How far each key is down (m), by MIDI number - LOW.
    pub depth: Vec<f32>,
    /// The key each fingertip holds down, with its last height.
    held: HashMap<(Entity, usize), (u8, f32)>,
    pub noise_in: f32,
}

/// A key struck or let go.
#[derive(Message, Clone, Copy, Debug)]
pub struct PianoNote {
    pub piano: Entity,
    pub key: u8,
    /// 0..1, from how fast the key went down.
    pub velocity: f32,
    pub on: bool,
    /// The key, in the world.
    pub at: Vec3,
}

fn is_black(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

/// White keys below `key` from the bottom A.
fn whites_below(key: u8) -> u32 {
    (LOW..key).filter(|k| !is_black(*k)).count() as u32
}

/// The x of a key's middle, in the piano's space.
pub fn key_x(key: u8) -> f32 {
    let whites = (LOW..=HIGH).filter(|k| !is_black(*k)).count() as f32;
    let left = -whites * WHITE * 0.5;
    if is_black(key) {
        // Between the white keys either side.
        left + whites_below(key) as f32 * WHITE
    } else {
        left + (whites_below(key) as f32 + 0.5) * WHITE
    }
}

/// The key's top (y) at rest.
pub fn key_top(key: u8) -> f32 {
    if is_black(key) { BLACK_TOP } else { WHITE_TOP }
}

/// Which key a point over the keyboard is over (piano space), black keys
/// first where they stand.
pub fn key_under(p: Vec3) -> Option<u8> {
    if p.z > BACK || p.z < WHITE_FRONT {
        return None;
    }
    if p.z >= BLACK_FRONT {
        if let Some(k) = (LOW..=HIGH)
            .filter(|k| is_black(*k))
            .find(|k| (p.x - key_x(*k)).abs() <= BLACK_HALF)
        {
            return Some(k);
        }
    }
    (LOW..=HIGH)
        .filter(|k| !is_black(*k))
        .find(|k| (p.x - key_x(*k)).abs() <= WHITE * 0.5)
}

/// Fingertips push keys down; a key sounds as it passes most of its
/// travel, with the speed it went, and is damped as the finger lifts.
pub(super) fn play_pianos(
    players: Query<(Entity, &RigInput, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
    mut pianos: Query<(Entity, &GlobalTransform, &mut Piano)>,
    driver: Res<RigDriver>,
    time: Res<Time>,
    mut notes: MessageWriter<PianoNote>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    let dt = time.delta_secs().max(1e-4);
    let mut tips = Vec::new();
    if *driver == RigDriver::Xr {
        for (player, input, children) in &players {
            for (i, child) in children.iter().enumerate() {
                let Ok((rh, hand)) = hands.get(child) else {
                    continue;
                };
                let h = input.hand(rh.side);
                if h.tracked && h.trigger < 0.5 && hand.holding.is_none() && hand.handle.is_none()
                {
                    tips.push(((player, i), hand.world.transform_point(FINGERTIP)));
                }
            }
        }
    }
    for (pe, gt, mut piano) in &mut pianos {
        if piano.depth.len() != (HIGH - LOW + 1) as usize {
            piano.depth = vec![0.0; (HIGH - LOW + 1) as usize];
        }
        piano.noise_in -= dt;
        let to_local = gt.affine().inverse();
        let mut depth = vec![0.0f32; piano.depth.len()];
        let mut still = Vec::new();
        for (who, tip) in &tips {
            let p = to_local.transform_point3(*tip);
            let held = piano.held.get(who).copied();
            // A finger already on a key stays on it while it's over it.
            let key = held
                .map(|(k, _)| k)
                .filter(|k| key_under(p) == Some(*k))
                .or_else(|| key_under(p));
            let Some(key) = key else {
                continue;
            };
            let d = (key_top(key) - p.y).clamp(0.0, TRAVEL);
            let i = (key - LOW) as usize;
            depth[i] = depth[i].max(d);
            match held {
                Some((k, _)) if k == key => {
                    if d < SOUNDS_AT * 0.4 {
                        // Lifted: damped.
                        notes.write(PianoNote {
                            piano: pe,
                            key,
                            velocity: 0.0,
                            on: false,
                            at: gt.transform_point(Vec3::new(key_x(key), key_top(key), BACK)),
                        });
                    } else {
                        still.push((*who, (key, p.y)));
                    }
                }
                _ => {
                    if let Some((k, _)) = held {
                        notes.write(PianoNote {
                            piano: pe,
                            key: k,
                            velocity: 0.0,
                            on: false,
                            at: gt.transform_point(Vec3::new(key_x(k), key_top(k), BACK)),
                        });
                    }
                    if d >= SOUNDS_AT {
                        // How fast it came down, from where it was last
                        // frame (a finger resting on a key and pressed
                        // slowly plays it softly).
                        let was = piano
                            .held
                            .get(who)
                            .map_or(p.y + 0.01, |(_, y)| *y)
                            .max(p.y);
                        let speed = (was - p.y) / dt;
                        let velocity = (speed / 0.6).clamp(0.15, 1.0);
                        let at = gt.transform_point(Vec3::new(key_x(key), key_top(key), BACK));
                        notes.write(PianoNote {
                            piano: pe,
                            key,
                            velocity,
                            on: true,
                            at,
                        });
                        if piano.noise_in <= 0.0 {
                            piano.noise_in = 2.0;
                            noise.write(crate::robots::Noise {
                                at,
                                radius: HEARD_AT * velocity.sqrt(),
                                source: None,
                            });
                        }
                        still.push((*who, (key, p.y)));
                    }
                }
            }
        }
        // Fingers gone from the keyboard let their keys up.
        let gone: Vec<u8> = piano
            .held
            .iter()
            .filter(|(w, _)| !still.iter().any(|(s, _)| s == *w))
            .map(|(_, (k, _))| *k)
            .collect();
        for k in gone {
            if !still.iter().any(|(_, (sk, _))| *sk == k) {
                notes.write(PianoNote {
                    piano: pe,
                    key: k,
                    velocity: 0.0,
                    on: false,
                    at: gt.transform_point(Vec3::new(key_x(k), key_top(k), BACK)),
                });
            }
        }
        piano.held = still.into_iter().collect();
        piano.depth = depth;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The keyboard's ends and middle C are where the model has them
    /// (`art/props/music.py`: key_21 at -0.5865, key_60 at -0.0575).
    #[test]
    fn keys_where_the_model_has_them() {
        assert!((key_x(21) - -0.5865).abs() < 1e-4, "{}", key_x(21));
        assert!((key_x(60) - -0.0575).abs() < 1e-4, "{}", key_x(60));
        assert!((key_x(108) - 0.5865).abs() < 1e-4, "{}", key_x(108));
        assert_eq!((LOW..=HIGH).filter(|k| !is_black(*k)).count(), 52);
    }

    /// A point over each key's middle finds that key; black keys win where
    /// they stand, and in front of them it's the white key.
    #[test]
    fn every_key_is_found() {
        for k in LOW..=HIGH {
            let z = if is_black(k) { -0.1 } else { -0.19 };
            assert_eq!(key_under(Vec3::new(key_x(k), 0.75, z)), Some(k), "{k}");
        }
        // Behind the keys, or off the end: nothing.
        assert_eq!(key_under(Vec3::new(0.0, 0.75, 0.0)), None);
        assert_eq!(key_under(Vec3::new(0.7, 0.75, -0.1)), None);
    }
}
