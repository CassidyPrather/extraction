//! Hands on the machines, heard (`docs/AUDIO.md`, "Hands on the
//! machines"): a palm shoving a robot's part, a robot straining in a grip
//! and kicking free of it, and the crawler (`robots::indoor`), which had
//! no sound but its voice: its little feet, its spark gun charging (the
//! tell) and its zap. (A crowbar's or a knife's blow is an `Impact` like
//! any hit, heard by `audio::hit_sounds` as a blow.)
//!
//! Like the rest of Audio this only listens: nothing in the game knows it
//! is here.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::audio::{AudioRng, SoundBank, play_at_speed};
use crate::interact::fight::{Gripped, Shoved, TornFree};
use crate::robots::grip::KickedFree;
use crate::robots::indoor::{Crawler, CrawlerCharge, CrawlerZap};

pub struct FightSoundsPlugin;

impl Plugin for FightSoundsPlugin {
    fn build(&self, app: &mut App) {
        // (Their own plugins register these too; this keeps the readers
        // valid in apps without them.)
        app.add_message::<Shoved>()
            .add_message::<Gripped>()
            .add_message::<KickedFree>()
            .add_message::<TornFree>()
            .add_message::<CrawlerCharge>()
            .add_message::<CrawlerZap>()
            .add_systems(
                Update,
                (
                    shove_sounds,
                    strain_sounds,
                    kick_sounds,
                    torn_sounds,
                    crawler_feet,
                    crawler_gun,
                ),
            );
    }
}

/// A shove (N·s) heard at its cue's level; a lighter one quieter, to
/// 12 dB under at [`SHOVE_SOFT`].
const SHOVE_FULL: f32 = 40.0;
const SHOVE_SOFT: f32 = 8.0;

/// How far under its cue a shove of `impulse` N·s is, dB.
pub fn shove_db(impulse: f32) -> f32 {
    -12.0 * (1.0 - ((impulse - SHOVE_SOFT) / (SHOVE_FULL - SHOVE_SOFT)).clamp(0.0, 1.0))
}

/// A palm shoving a robot (`interact::fight::Shoved`): the flat of the
/// hand on its plate (`fight.shove`), and the machine thrown off its
/// footing, its servos catching it (`fight.stagger`).
fn shove_sounds(
    mut commands: Commands,
    mut shoves: MessageReader<Shoved>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for s in shoves.read() {
        let db = shove_db(s.impulse.length());
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "impacts/metal",
            Some(s.at),
            "fight.shove",
            db,
            0.7,
            0.08,
        );
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/servo",
            Some(s.at),
            "fight.stagger",
            db,
            1.25,
            0.1,
        );
    }
}

/// Strain (m, the hand from where it holds) a robot's fight is first
/// heard at; and between its whines, s.
const STRAIN_HEARD: f32 = 0.08;
const STRAIN_EVERY: f32 = 0.35;

/// A robot fighting a grip (`interact::fight::Gripped`): its servos
/// whining against the hand, higher the harder it pulls (`fight.strain`),
/// every [`STRAIN_EVERY`] while it does.
fn strain_sounds(
    mut commands: Commands,
    mut gripped: MessageReader<Gripped>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut next: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    // Forget a robot once its next whine is due (and so anything let go).
    next.retain(|_, t| now < *t);
    for g in gripped.read() {
        let strain = g.strain.length();
        if strain < STRAIN_HEARD || next.contains_key(&g.robot) {
            continue;
        }
        next.insert(g.robot, now + STRAIN_EVERY);
        let pull = ((strain - STRAIN_HEARD) / (crate::interact::fight::SLIP - STRAIN_HEARD))
            .clamp(0.0, 1.0);
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/servo",
            Some(g.at),
            "fight.strain",
            -9.0 * (1.0 - pull),
            1.0 + 0.4 * pull as f64,
            0.06,
        );
    }
}

/// A robot kicking free of the hands on it (`robots::grip::KickedFree`):
/// a hard metal shove off (`fight.kick`) and its servos snapping round.
fn kick_sounds(
    mut commands: Commands,
    mut kicks: MessageReader<KickedFree>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for k in kicks.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/clank",
            Some(k.at),
            "fight.kick",
            0.0,
            1.15,
            0.08,
        );
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/servo",
            Some(k.at),
            "fight.stagger",
            0.0,
            1.5,
            0.1,
        );
    }
}

/// A held robot torn out of the hand (`interact::fight::TornFree`: a
/// drone thrusting free): metal wrenched (`fight.torn`, a creak at speed)
/// and the machine lurching away (`fight.lurch`).
fn torn_sounds(
    mut commands: Commands,
    mut torn: MessageReader<TornFree>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for t in torn.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/door_creak",
            Some(t.at),
            "fight.torn",
            0.0,
            1.9,
            0.08,
        );
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/whoosh",
            Some(t.at),
            "fight.lurch",
            0.0,
            1.3,
            0.1,
        );
    }
}

/// A crawler is heard moving faster than this (m/s); its feet patter
/// every [`CRAWLER_PATTER`] s at a run, slower at a creep.
const CRAWLER_MOVING: f32 = 0.3;
const CRAWLER_PATTER: f32 = 0.11;
const CRAWLER_RUN: f32 = 3.0;

/// The crawler's feet: many small metal points pattering on the floor
/// (`crawler.step`, the walker's footfall sped up and small), quicker the
/// quicker it goes. It has no gait message; its speed is read from where
/// it was.
fn crawler_feet(
    mut commands: Commands,
    crawlers: Query<(Entity, &GlobalTransform, &Crawler)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut was: Local<HashMap<Entity, (Vec3, f32)>>,
) {
    let now = time.elapsed_secs();
    let dt = time.delta_secs().max(1e-4);
    was.retain(|e, _| crawlers.contains(*e));
    for (e, gt, c) in &crawlers {
        let at = gt.translation();
        let (last, next) = was.entry(e).or_insert((at, now));
        let speed = at.distance(*last) / dt;
        *last = at;
        if c.cleared || speed < CRAWLER_MOVING || now < *next {
            continue;
        }
        let run = (speed / CRAWLER_RUN).clamp(0.2, 1.0);
        *next = now + CRAWLER_PATTER / run * (0.8 + 0.4 * rng.0.f32());
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "footsteps/metal",
            Some(at),
            "crawler.step",
            -6.0 * (1.0 - run),
            2.2,
            0.12,
        );
    }
}

/// The crawler's spark gun: its charge, the tell, a rising electric
/// crackle (`crawler.charge`) over the half second before it fires; the
/// zap, a hard snap at the emitter and where it lands (`crawler.zap`).
fn crawler_gun(
    mut commands: Commands,
    mut charges: MessageReader<CrawlerCharge>,
    mut zaps: MessageReader<CrawlerZap>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for c in charges.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            crate::audio::CRACKLE_SET,
            Some(c.at + Vec3::Y * 0.3),
            "crawler.charge",
            0.0,
            0.8,
            0.03,
        );
    }
    for z in zaps.read() {
        for (at, db) in [(z.from, 0.0), (z.to, -6.0)] {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                crate::audio::CRACKLE_SET,
                Some(at),
                "crawler.zap",
                db,
                1.6,
                0.05,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every cue this file plays is in the mix, with the set it plays.
    #[test]
    fn the_fight_cues_are_in_the_mix() {
        for (set, cue) in [
            ("impacts/metal", "fight.shove"),
            ("machines/servo", "fight.stagger"),
            ("machines/servo", "fight.strain"),
            ("machines/clank", "fight.kick"),
            ("physical/door_creak", "fight.torn"),
            ("physical/whoosh", "fight.lurch"),
            ("footsteps/metal", "crawler.step"),
            (crate::audio::CRACKLE_SET, "crawler.charge"),
            (crate::audio::CRACKLE_SET, "crawler.zap"),
        ] {
            let c = crate::audio_mix::MIX.iter().find(|c| c.name == cue);
            assert_eq!(c.map(|c| c.set), Some(set), "{cue}");
        }
    }

    /// A shove's level only comes down from its cue's, and rises with the
    /// push.
    #[test]
    fn shoves_are_louder_the_harder() {
        assert_eq!(shove_db(SHOVE_SOFT), -12.0);
        assert_eq!(shove_db(SHOVE_FULL), 0.0);
        assert!(shove_db(15.0) < shove_db(30.0));
    }
}
