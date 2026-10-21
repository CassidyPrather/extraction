//! Sound for the newer kit and the bare hands (`docs/AUDIO.md`, "What you
//! hear and feel"): a signal flare's pop and hiss, the RPK's bipod
//! swinging and locking, and a palm slapped on whatever it meets. (The
//! pump-action's rack is a gun event, heard with the rest in
//! `audio::gun_sounds`.)
//!
//! Like the rest of Audio this only listens: nothing in the game knows it
//! is here.

use bevy::prelude::*;
use bevy_seedling::prelude::*;
use std::collections::HashMap;

use crate::audio::{AudioRng, SoundBank, play_at_speed, play_filtered_db, reverb_send};
use crate::audio_mix as mix;
use crate::interact::touch::{HandStruck, SLAP, Stuff};
use crate::weapons::armoury::Bipod;
use crate::weapons::flares::Flare;

pub struct KitSoundsPlugin;

impl Plugin for KitSoundsPlugin {
    fn build(&self, app: &mut App) {
        // (Touch registers it too; this keeps the reader valid in apps
        // without the hands.)
        app.add_message::<HandStruck>().add_systems(
            Update,
            (flare_sounds, flare_hiss, bipod_sounds, slap_sounds),
        );
    }
}

/// A flare's burning hiss, on the star.
#[derive(Component)]
struct FlareHiss;

/// A star leaving the muzzle: a soft, dull pop (a pistol's report
/// low-passed to 1.2 kHz and quiet, the star's charge catching, not a
/// shot), and the hiss of it burning that rides along with it: the burner
/// anomaly's roar sped up into a fizz, silent until it lights.
fn flare_sounds(
    mut commands: Commands,
    added: Query<(Entity, &GlobalTransform), Added<Flare>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, gt) in &added {
        let speed = 1.1 + rng.0.f32() as f64 * 0.1;
        play_filtered_db(
            &mut commands,
            &bank,
            &mut rng.0,
            "guns/pistol_shot",
            gt.translation(),
            "flare.pop",
            0.0,
            speed,
            1_200.0,
        );
        let cue = "flare.burn";
        if let Some(sample) = bank.pick("anomaly/burner_roar", &mut rng.0) {
            commands.entity(e).with_child((
                FlareHiss,
                SamplePlayer::new(sample)
                    .looping()
                    .with_volume(Volume::Decibels(mix::db(cue))),
                PlaybackSettings::default().with_speed(1.7 + rng.0.f32() as f64 * 0.2),
                Transform::default(),
                sample_effects![
                    VolumeNode {
                        volume: Volume::SILENT,
                        ..default()
                    },
                    mix::spatial(cue),
                    reverb_send(cue)
                ],
            ));
        }
    }
}

/// The hiss follows the star's glow: nothing before it lights, full while
/// it burns, fading over its last second. It goes with the star.
fn flare_hiss(
    hisses: Query<(&ChildOf, &SampleEffects), With<FlareHiss>>,
    stars: Query<&Flare>,
    mut volumes: Query<&mut VolumeNode>,
) {
    for (parent, effects) in &hisses {
        let Ok(f) = stars.get(parent.parent()) else {
            continue;
        };
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = Volume::Linear(f.glow());
        }
    }
}

/// Seconds between one bipod's sounds, however it's worked.
const BIPOD_EVERY: f32 = 0.15;

/// What a bipod was last frame, and when it may next be heard.
#[derive(Clone, Copy)]
struct BipodWas {
    deployed: bool,
    swing: f32,
    quiet_until: f32,
}

/// The RPK's legs: a short metal creak as they start to swing, and a
/// clank as they lock down (or fold home), at the legs. At most one
/// every [`BIPOD_EVERY`] s, so working them back and forth doesn't
/// chatter.
fn bipod_sounds(
    mut commands: Commands,
    bipods: Query<(Entity, &Bipod, &GlobalTransform)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut was: Local<HashMap<Entity, BipodWas>>,
) {
    let now = time.elapsed_secs();
    was.retain(|e, _| bipods.contains(*e));
    for (e, b, gt) in &bipods {
        let Some(last) = was.insert(
            e,
            BipodWas {
                deployed: b.deployed,
                swing: b.swing,
                quiet_until: 0.0,
            },
        ) else {
            continue;
        };
        let at = gt.transform_point(b.feet_now());
        let swung = b.deployed != last.deployed;
        let locked = b.swing != last.swing && (b.swing <= 0.0 || b.swing >= 1.0);
        let mut quiet_until = last.quiet_until;
        if (swung || locked) && now >= last.quiet_until {
            let (set, cue, speed) = if locked {
                ("machines/clank", "bipod.lock", 1.5)
            } else {
                ("physical/handle_metal", "bipod.swing", 1.2)
            };
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                cue,
                0.0,
                speed,
                0.08,
            );
            quiet_until = now + BIPOD_EVERY;
        }
        if let Some(w) = was.get_mut(&e) {
            w.quiet_until = quiet_until;
        }
    }
}

/// A hand's speed (m/s) that slaps at the cue's full level; slower is
/// quieter, down to [`SLAP_SOFT_DB`] at [`SLAP`].
const SLAP_FULL: f32 = 6.0;
const SLAP_SOFT_DB: f32 = -12.0;

/// How much quieter than its cue a slap at `speed` m/s is, dB: -12 at
/// the slowest that's heard, 0 at a full swing.
pub fn slap_db(speed: f32) -> f32 {
    let t = ((speed - SLAP) / (SLAP_FULL - SLAP)).clamp(0.0, 1.0);
    SLAP_SOFT_DB * (1.0 - t)
}

/// A slap's sound by what the hand met: (set, cue, speed, its
/// brightest cutoff in Hz).
fn slap_of(stuff: Stuff) -> (&'static str, &'static str, f64, f32) {
    match stuff {
        Stuff::Wood => ("impacts/wood", "hand.slap_wood", 1.15, 9_000.0),
        Stuff::Metal => ("impacts/metal", "hand.slap_metal", 1.1, 12_000.0),
        // A knock on a pane, not a break: the break sped up and small.
        Stuff::Glass => ("physical/glass_break", "hand.slap_glass", 1.8, 14_000.0),
        Stuff::Stone => ("impacts/concrete", "hand.slap_stone", 1.2, 6_000.0),
        Stuff::Skin => ("physical/body_hit", "hand.slap_skin", 1.05, 3_500.0),
        // Chain-link shaken: a quick metal rattle.
        Stuff::Wire => ("physical/handle_metal", "hand.slap_wire", 1.6, 10_000.0),
    }
}

/// A bare hand striking something (`interact::touch`): a palm slap by
/// what it's made of, where it met. Harder is louder and brighter: from
/// 12 dB down and dull at a 1.5 m/s tap to the cue's level and its full
/// edge at 6 m/s.
fn slap_sounds(
    mut commands: Commands,
    mut struck: MessageReader<HandStruck>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for s in struck.read() {
        let (set, cue, speed, bright) = slap_of(s.stuff);
        let t = ((s.speed - SLAP) / (SLAP_FULL - SLAP)).clamp(0.0, 1.0);
        let hz = bright * (0.25 + 0.75 * t);
        let speed = speed * (1.0 + (rng.0.f32() as f64 - 0.5) * 0.12);
        play_filtered_db(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            s.at,
            cue,
            slap_db(s.speed),
            speed,
            hz,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slap's level only ever comes down from its cue's, and rises with
    /// the swing.
    #[test]
    fn slaps_are_louder_the_harder() {
        assert_eq!(slap_db(SLAP), SLAP_SOFT_DB);
        assert_eq!(slap_db(SLAP_FULL), 0.0);
        assert_eq!(slap_db(20.0), 0.0);
        assert!(slap_db(3.0) < slap_db(4.5));
    }

    /// Every cue this file plays is in the mix, with the set it plays.
    #[test]
    fn the_kit_cues_are_in_the_mix() {
        let mut cues = vec![
            ("guns/pistol_shot", "flare.pop"),
            ("anomaly/burner_roar", "flare.burn"),
            ("machines/clank", "bipod.lock"),
            ("physical/handle_metal", "bipod.swing"),
        ];
        for s in [
            Stuff::Wood,
            Stuff::Metal,
            Stuff::Glass,
            Stuff::Stone,
            Stuff::Skin,
            Stuff::Wire,
        ] {
            let (set, cue, _, _) = slap_of(s);
            cues.push((set, cue));
        }
        for (set, cue) in cues {
            let c = mix::MIX.iter().find(|c| c.name == cue);
            assert_eq!(c.map(|c| c.set), Some(set), "{cue}");
        }
    }
}
