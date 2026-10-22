//! Sound for the newer kit and the bare hands (`docs/AUDIO.md`, "What you
//! hear and feel"): a signal flare's pop and hiss, the RPK's bipod
//! swinging and locking, a palm slapped on whatever it meets, a held gun
//! knocked into a wall, something heavy dragged over the ground, and the
//! lights for the dark: a chemlight snapped, a hand flare struck; and
//! treatment: a dressing wound on, a splint strapped, a bottle's cap
//! twisted off, a hand at a wound. (The
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
use crate::interact::carry::{DRAG_SPEED, Dragged};
use crate::interact::touch::{HandStruck, SLAP, Stuff};
use crate::weapons::ActionGrind;
use crate::weapons::armoury::Bipod;
use crate::weapons::carry::GunBumped;
use crate::weapons::flares::Flare;
use crate::world::glow::{Burning, GlowKind, Lit};

pub struct KitSoundsPlugin;

impl Plugin for KitSoundsPlugin {
    fn build(&self, app: &mut App) {
        // (Touch registers it too; this keeps the reader valid in apps
        // without the hands.)
        app.add_message::<HandStruck>()
            .add_message::<GunBumped>()
            .add_message::<Dragged>()
            .add_message::<ActionGrind>()
            .add_message::<Lit>()
            .add_systems(
                Update,
                (
                    flare_sounds,
                    flare_hiss,
                    bipod_sounds,
                    slap_sounds,
                    bump_sounds,
                    drag_sounds,
                    grind_sounds,
                    lit_sounds,
                    flare_burns,
                    treatment_sounds,
                ),
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

/// A held gun's bump speed (m/s) heard at its quietest (Weapon feel's
/// threshold) and at the cue's full level.
const BUMP: f32 = 0.8;
const BUMP_FULL: f32 = 3.0;

/// How much quieter than its cue a gun bump at `speed` m/s is, dB.
fn bump_db(speed: f32) -> f32 {
    let t = ((speed - BUMP) / (BUMP_FULL - BUMP)).clamp(0.0, 1.0);
    SLAP_SOFT_DB * (1.0 - t)
}

/// A held gun carried into a wall, a doorframe or a tree
/// (`weapons::carry`): what it met, as a hand would slap it, under the
/// gun's own knock of steel. (The hands feel it already.)
fn bump_sounds(
    mut commands: Commands,
    mut bumps: MessageReader<GunBumped>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for b in bumps.read() {
        let db = bump_db(b.speed);
        let (set, cue, speed, bright) = slap_of(b.stuff);
        play_filtered_db(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            b.at,
            cue,
            db - 3.0,
            speed,
            bright * 0.6,
        );
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/drop_metal",
            Some(b.at),
            "gun.bump",
            db,
            1.15,
            0.08,
        );
    }
}

/// A hand flare's spitting burn, on the flare.
#[derive(Component)]
struct HandFlareBurn;

/// A light for the dark lit (`world::glow`): a chemlight's tube snapped
/// inside (`glow.snap`, a plastic crack, quick); a hand flare's cap rasped
/// across its end (`glow.strike`) and the flare taking, spitting as it
/// burns (`glow.burn`, the burner's roar sped into a sputter) for as long
/// as it does, wherever it's thrown.
fn lit_sounds(
    mut commands: Commands,
    mut lit: MessageReader<Lit>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for l in lit.read() {
        match l.kind {
            GlowKind::Chem => play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                "physical/handle_plastic",
                Some(l.at),
                "glow.snap",
                0.0,
                1.5,
                0.08,
            ),
            GlowKind::Flare => {
                play_at_speed(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "physical/zip",
                    Some(l.at),
                    "glow.strike",
                    0.0,
                    0.7,
                    0.08,
                );
                let cue = "glow.burn";
                if let Some(sample) = bank.pick("anomaly/burner_roar", &mut rng.0) {
                    commands.entity(l.item).with_child((
                        HandFlareBurn,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db(cue))),
                        PlaybackSettings::default().with_speed(2.1 + rng.0.f32() as f64 * 0.2),
                        Transform::default(),
                        sample_effects![mix::spatial(cue), reverb_send(cue)],
                    ));
                }
            }
        }
    }
}

/// A hand flare burnt out (no longer `Burning`): its sputter stops.
fn flare_burns(
    mut commands: Commands,
    burns: Query<(Entity, &ChildOf), With<HandFlareBurn>>,
    burning: Query<(), With<Burning>>,
) {
    for (e, parent) in &burns {
        if !burning.contains(parent.parent()) {
            commands.entity(e).despawn();
        }
    }
}

/// What treatment was where last frame: a dressing's quarter turns round
/// the limb, a splint's half seconds strapping, a cap's quarter turns.
#[derive(Clone, Copy)]
struct Treating {
    steps: f32,
    at: Vec3,
    splint: bool,
}

/// Treatment with the hands (`interact::first_aid`, `wounds`), which
/// sends no messages: its state is read.
///
/// - A dressing wound round a limb (`Winding`): a short rasp of cloth at
///   each quarter turn (`aid.wrap`); the tear when it's done is the use's
///   (`use.bandage`).
/// - A splint strapped along the shin (`Strapping`): a strap pulled
///   tight each half second, as the hand feels it (`aid.strap`, velcro),
///   and cinched hard when it's on (`aid.cinch`).
/// - A bottle's cap twisted (`Capped`): a plastic tick at each quarter
///   turn (`aid.cap`) and the pop as it comes off (`aid.cap_off`); the
///   swallow is the drink's (`use.drink`).
/// - A hand at an open wound (`Bloodied` coming on): a wet squelch
///   (`aid.wet`).
#[allow(clippy::type_complexity)]
fn treatment_sounds(
    mut commands: Commands,
    windings: Query<(
        Entity,
        &crate::interact::first_aid::Winding,
        &GlobalTransform,
    )>,
    straps: Query<(
        Entity,
        &crate::interact::first_aid::Strapping,
        &GlobalTransform,
    )>,
    caps: Query<(
        Entity,
        &crate::interact::first_aid::Capped,
        &GlobalTransform,
    )>,
    places: Query<&GlobalTransform>,
    bloodied: Query<&GlobalTransform, Added<crate::interact::wounds::Bloodied>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    mut was: Local<HashMap<Entity, Treating>>,
) {
    let sound = |commands: &mut Commands, rng: &mut AudioRng, set, cue, at, speed| {
        play_at_speed(
            commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            cue,
            0.0,
            speed,
            0.08,
        );
    };
    let mut seen = Vec::new();
    let now = windings
        .iter()
        .map(|(e, w, g)| {
            (
                e,
                (w.turns * 4.0).floor(),
                g.translation(),
                false,
                "aid.wrap",
            )
        })
        .chain(
            straps
                .iter()
                .map(|(e, s, g)| (e, (s.0 * 2.0).floor(), g.translation(), true, "aid.strap")),
        )
        .chain(caps.iter().map(|(e, c, g)| {
            (
                e,
                (c.twisted * 4.0).floor(),
                g.translation(),
                false,
                "aid.cap",
            )
        }));
    for (e, steps, at, splint, cue) in now {
        seen.push(e);
        let before = was.insert(e, Treating { steps, at, splint });
        if before.is_some_and(|b| steps > b.steps) {
            let (set, speed) = match cue {
                "aid.wrap" => ("physical/bandage", 1.25),
                "aid.strap" => ("physical/velcro", 1.3),
                _ => ("physical/handle_plastic", 1.8),
            };
            sound(&mut commands, &mut rng, set, cue, at, speed);
        }
    }
    // Gone from its state: a splint strapped on (it leaves the hand), a cap
    // off (the bottle stays, capless).
    let mut done = Vec::new();
    was.retain(|e, t| {
        let keep = seen.contains(e);
        if !keep {
            done.push((*e, *t));
        }
        keep
    });
    for (e, t) in done {
        if t.splint {
            if t.steps >= 2.0 {
                sound(
                    &mut commands,
                    &mut rng,
                    "physical/velcro",
                    "aid.cinch",
                    t.at,
                    0.9,
                );
            }
        } else if let Ok(g) = places.get(e)
            && caps.get(e).is_err()
            && windings.get(e).is_err()
        {
            sound(
                &mut commands,
                &mut rng,
                "physical/handle_plastic",
                "aid.cap_off",
                g.translation(),
                1.3,
            );
        }
    }
    for g in &bloodied {
        sound(
            &mut commands,
            &mut rng,
            "footsteps/mud",
            "aid.wet",
            g.translation(),
            1.4,
        );
    }
}

/// The shortest gap between two grinds heard (s): Weapon feel ticks every
/// 60 ms, about twice as often as a grind sounds.
const GRIND_EVERY: f32 = 0.11;

/// A stiff action worked (`weapons::ActionGrind`: a cold bolt, pump or
/// charging handle): a dry metal grate under the hand, as loud as it's
/// stiff, 20 dB down at the faintest.
fn grind_sounds(
    mut commands: Commands,
    mut grinds: MessageReader<ActionGrind>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut quiet_until: Local<f32>,
) {
    let now = time.elapsed_secs();
    for g in grinds.read() {
        if now < *quiet_until {
            continue;
        }
        *quiet_until = now + GRIND_EVERY;
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/handle_metal",
            Some(g.at),
            "gun.grind",
            grind_db(g.stiff),
            1.6,
            0.12,
        );
    }
}

/// How far under its cue a grind of a `stiff` (0..1) action is, dB.
fn grind_db(stiff: f32) -> f32 {
    20.0 * stiff.clamp(0.1, 1.0).log10()
}

/// A drag's loop, following the thing dragged.
#[derive(Component)]
struct DragLoop {
    item: Entity,
    /// When its last `Dragged` came.
    last: f32,
}

/// A drag falls silent this long (s) after its last `Dragged`.
const DRAG_LETGO: f32 = 0.15;
/// How far under its cue a drag is at a crawl, dB.
const DRAG_SOFT_DB: f32 = -15.0;

/// A drag's loop and cue by what's dragged.
fn drag_of(stuff: Stuff, rolling: bool) -> (&'static str, &'static str) {
    if rolling {
        return ("drag/roll", "drag.roll");
    }
    match stuff {
        Stuff::Metal | Stuff::Glass | Stuff::Wire => ("drag/metal", "drag.metal"),
        Stuff::Wood | Stuff::Stone => ("drag/wood", "drag.wood"),
        Stuff::Skin => ("drag/body", "drag.body"),
    }
}

/// How far under its cue a drag at `speed` m/s is, dB, and its playback
/// speed: a crawl is soft and low, a hard haul at [`DRAG_SPEED`] the
/// cue's level and a little higher.
fn drag_level(speed: f32) -> (f32, f64) {
    let t = (speed / DRAG_SPEED).clamp(0.0, 1.0);
    (DRAG_SOFT_DB * (1.0 - t), 0.8 + 0.3 * t as f64)
}

/// Something bulky dragged over the ground (`interact::carry`): a loop
/// per thing, by what it is (a sheet of metal scraping, a wheel rolling,
/// a body), louder and higher the faster it's hauled, following it, and
/// gone a moment after it stops sliding (or is gone itself).
fn drag_sounds(
    mut commands: Commands,
    mut drags: MessageReader<Dragged>,
    mut loops: Query<(
        Entity,
        &mut DragLoop,
        &mut Transform,
        &mut PlaybackSettings,
        &SampleEffects,
    )>,
    mut volumes: Query<&mut VolumeNode>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for d in drags.read() {
        let (set, cue) = drag_of(d.stuff, d.rolling);
        let (db, speed) = drag_level(d.speed);
        let volume = Volume::Decibels(mix::db(cue) + db);
        match loops.iter_mut().find(|(_, l, ..)| l.item == d.item) {
            Some((_, mut l, mut t, mut settings, effects)) => {
                l.last = now;
                t.translation = d.at;
                if (settings.speed - speed).abs() > 0.02 {
                    settings.speed = speed;
                }
                if let Ok(mut v) = volumes.get_effect_mut(effects) {
                    v.volume = volume;
                }
            }
            None => {
                let Some(sample) = bank.pick(set, &mut rng.0) else {
                    continue;
                };
                commands.spawn((
                    DragLoop {
                        item: d.item,
                        last: now,
                    },
                    SamplePlayer::new(sample).looping(),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(d.at),
                    sample_effects![
                        VolumeNode {
                            volume,
                            ..default()
                        },
                        mix::spatial(cue),
                        reverb_send(cue)
                    ],
                ));
            }
        }
    }
    for (e, l, ..) in &loops {
        if now - l.last > DRAG_LETGO {
            commands.entity(e).despawn();
        }
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
        assert_eq!(bump_db(BUMP), SLAP_SOFT_DB);
        assert_eq!(bump_db(BUMP_FULL), 0.0);
        assert_eq!(grind_db(1.0), 0.0);
        assert!((grind_db(0.0) + 20.0).abs() < 1e-4);
    }

    /// A drag is the cue's level at a full haul, softer and lower slower.
    #[test]
    fn drags_are_louder_the_faster() {
        let (full, s) = drag_level(DRAG_SPEED);
        assert!(full == 0.0 && (s - 1.1).abs() < 1e-6);
        let (slow, s) = drag_level(0.3);
        assert!(slow < -10.0 && s < 0.9);
        assert_eq!(drag_of(Stuff::Metal, true).0, "drag/roll");
        assert_eq!(drag_of(Stuff::Skin, false).0, "drag/body");
    }

    /// Every cue this file plays is in the mix, with the set it plays.
    #[test]
    fn the_kit_cues_are_in_the_mix() {
        let mut cues = vec![
            ("guns/pistol_shot", "flare.pop"),
            ("anomaly/burner_roar", "flare.burn"),
            ("machines/clank", "bipod.lock"),
            ("physical/handle_metal", "bipod.swing"),
            ("physical/drop_metal", "gun.bump"),
            ("physical/handle_metal", "gun.grind"),
            ("physical/handle_plastic", "glow.snap"),
            ("physical/zip", "glow.strike"),
            ("anomaly/burner_roar", "glow.burn"),
            ("physical/bandage", "aid.wrap"),
            ("physical/velcro", "aid.strap"),
            ("physical/velcro", "aid.cinch"),
            ("physical/handle_plastic", "aid.cap"),
            ("physical/handle_plastic", "aid.cap_off"),
            ("footsteps/mud", "aid.wet"),
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
            for rolling in [false, true] {
                cues.push(drag_of(s, rolling));
            }
        }
        for (set, cue) in cues {
            let c = mix::MIX.iter().find(|c| c.name == cue);
            assert_eq!(c.map(|c| c.set), Some(set), "{cue}");
        }
    }
}
