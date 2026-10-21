//! What the music sounds like: every tape side, station and instrument is
//! rendered (`extraction_core::music`) on the compute pool the first time
//! it's wanted, mono at the stream's rate (the sampler doesn't resample
//! what it's handed), kept while it's in use, and played as an ordinary placed
//! sound at the thing playing it: heard through the head-related filter,
//! muffled by walls, sent to the room's reverb, like any other sound.
//!
//! Client only (it needs the audio graph); the mechanism it follows runs
//! headless in [`super::deck`].

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy_seedling::prelude::*;
use bevy_seedling::sample::AudioSample;
use extraction_core::music::deck::{Deck, Mode, Side};
use extraction_core::music::stations::{self, Kind};
use extraction_core::music::{media, tapes, tunes};
use std::collections::HashMap;
use std::num::NonZeroU32;

use super::deck::{DeckCell, TapeDeck};
use crate::audio::Placed;
use crate::audio_mix as mix;
use crate::interact::{Socket, SocketOwner};
use crate::items::{Catalog, Item};

/// The rate renders are made at before the stream's is known (tests).
pub const RATE: u32 = 48_000;

/// Renders unused this long (s) are let go: a tape side is tens of MB.
const KEEP: f32 = 90.0;

/// The synthesised sets the mix's cues name (`audio_mix`), not files.
pub fn is_synth_set(set: &str) -> bool {
    matches!(
        set,
        "music/tape"
            | "music/motor"
            | "music/station"
            | "music/static"
            | "music/gramophone"
            | "music/piano"
            | "music/organ"
    )
}

/// Something to render.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Render {
    /// A tape's side, by the tape's item id.
    TapeSide(&'static str, usize),
    /// The motor winding a tape, a short loop.
    Motor,
    /// A synthesised station's loop, by id, for a day (`Raid::raids`):
    /// the numbers change daily.
    Station(&'static str, u64),
    /// The hiss between stations.
    Static,
    /// A record on the gramophone, piano and organ pieces: for the audit
    /// until the instruments in the world play their own.
    Gramophone(usize),
    PianoTune(usize),
    OrganTune(usize),
    /// One piano note, struck firmly and held (the dampers are the
    /// player's volume): every third key, the rest shifted to it.
    PianoKey(u8),
}

impl Render {
    /// Rendered, at `rate`, levelled: every music render is an asset at
    /// one level (an RMS of -20 dBFS, about where mastered music sits),
    /// so the mix's cue gains are all that set how loud it plays.
    fn run(&self, rate: u32) -> Vec<f32> {
        let mut x = self.render(rate);
        match self {
            // A note is levelled over its first second (its strike), so
            // every key is as loud struck as another.
            Render::PianoKey(_) => {
                let head = ((rate as usize).min(x.len())).max(1);
                let r = extraction_core::music::synth::rms(&x[..head]);
                if r > 1e-6 {
                    let g = LEVEL / r;
                    x.iter_mut().for_each(|s| *s = (*s * g).clamp(-1.0, 1.0));
                }
            }
            _ => level(&mut x),
        }
        x
    }

    fn render(&self, rate: u32) -> Vec<f32> {
        let sr = rate as f32;
        match self {
            Render::TapeSide(id, side) => tapes::get(id)
                .map(|def| {
                    let side = if *side == 0 { Side::A } else { Side::B };
                    tapes::render_side(def, side, sr)
                })
                .unwrap_or_default(),
            Render::Motor => motor_loop(sr),
            Render::Static => media::static_bed((24.0 * sr) as usize, sr, 0x57),
            Render::Gramophone(i) => {
                let mut x = tune_on(RECORDS[i % RECORDS.len()], sr);
                media::gramophone(&mut x, sr, *i as u64);
                x
            }
            Render::PianoTune(i) => tune_on(PIANO[i % PIANO.len()], sr),
            Render::OrganTune(i) => tune_on(ORGAN[i % ORGAN.len()], sr),
            Render::PianoKey(k) => {
                use extraction_core::music::{mml::Note, synth};
                let part = synth::Part {
                    instrument: synth::Instrument::Piano,
                    notes: vec![Note {
                        start: 0.0,
                        len: 5.0,
                        key: *k as i32,
                        vel: 0.85,
                    }],
                    gain: 1.0,
                };
                synth::render(&[part], sr, *k as u64)
            }
            Render::Station(id, day) => {
                let seed = id.bytes().fold(*day, |h, b| h.wrapping_mul(131) ^ b as u64);
                match stations::get(id).map(|s| s.kind) {
                    Some(Kind::Numbers) => stations::numbers_program(seed, sr),
                    Some(Kind::Ghosts) => stations::ghosts_program(seed, sr),
                    Some(Kind::FarMusic) => {
                        let tune = FAR_MUSIC[(*day as usize) % FAR_MUSIC.len()];
                        let parts = tunes::get(tune).map(|t| t.parts(seed)).unwrap_or_default();
                        stations::far_music_program(&parts, seed, sr)
                    }
                    _ => Vec::new(),
                }
            }
        }
    }
}

/// The level every render is an asset at (RMS, linear).
pub const LEVEL: f32 = 0.1;

pub fn level(x: &mut [f32]) {
    let r = extraction_core::music::synth::rms(x);
    if r > 1e-6 {
        let g = LEVEL / r;
        for s in x.iter_mut() {
            *s = (*s * g).clamp(-1.0, 1.0);
        }
    }
}

/// A tune as written, unrecorded.
fn tune_on(id: &str, sr: f32) -> Vec<f32> {
    tunes::get(id)
        .map(|t| extraction_core::music::synth::render(&t.parts(7), sr, 7))
        .unwrap_or_default()
}

/// What's on the 78s, the piano's and the organ's pieces.
const RECORDS: &[&str] = &["waltz_folk", "ragtime"];
const PIANO: &[&str] = &["gymnopedie", "minuet_g"];
const ORGAN: &[&str] = &["old_hundredth", "eventide"];

/// What the station over the hills plays, a tune a day.
const FAR_MUSIC: &[&str] = &["dance_band", "waltz_folk", "polka"];

/// Rendered sounds, and those on their way.
#[derive(Resource, Default)]
pub struct Renders {
    done: HashMap<Render, (Handle<AudioSample>, f32)>,
    pending: HashMap<Render, Task<Vec<f32>>>,
    /// The stream's rate, and the clock (s), for letting go.
    rate: Option<u32>,
    now: f32,
}

impl Renders {
    /// The sound if it's ready; otherwise starts it (once) and says no.
    pub fn want(&mut self, what: Render) -> Option<Handle<AudioSample>> {
        let now = self.now;
        if let Some((h, used)) = self.done.get_mut(&what) {
            *used = now;
            return Some(h.clone());
        }
        let rate = self.rate?;
        if !self.pending.contains_key(&what) {
            let job = what.clone();
            let task = AsyncComputeTaskPool::get().spawn(async move { job.run(rate) });
            self.pending.insert(what, task);
        }
        None
    }
}

fn collect_renders(
    mut renders: ResMut<Renders>,
    mut samples: ResMut<Assets<AudioSample>>,
    rate: Option<Res<bevy_seedling::context::SampleRate>>,
    time: Res<Time>,
) {
    let r = &mut *renders;
    if r.rate.is_none() {
        r.rate = rate.map(|r| r.get().get());
    }
    r.now = time.elapsed_secs();
    let now = r.now;
    r.done.retain(|_, (_, used)| now - *used < KEEP);
    let Some(rate) = r.rate else {
        return;
    };
    let mut finished = Vec::new();
    for (what, task) in r.pending.iter_mut() {
        if let Some(buf) = bevy::tasks::futures::check_ready(task) {
            finished.push((what.clone(), buf));
        }
    }
    for (what, buf) in finished {
        r.pending.remove(&what);
        let buf = if buf.is_empty() { vec![0.0; 16] } else { buf };
        let h = samples.add(AudioSample::new(vec![buf], NonZeroU32::new(rate).unwrap()));
        r.done.insert(what, (h, now));
    }
}

/// The motor and tape rushing past the heads while it winds: a small DC
/// motor's whine over the hiss of tape. Two seconds, loopable.
fn motor_loop(sr: f32) -> Vec<f32> {
    use extraction_core::noise::Rng;
    use std::f32::consts::TAU;
    let n = (2.0 * sr) as usize;
    let mut rng = Rng::new(0x707);
    let mut out = Vec::with_capacity(n);
    // Whole cycles in two seconds, so it loops clean.
    let (f1, f2) = (190.0f32, 760.0f32);
    let mut lp = 0.0f32;
    for i in 0..n {
        let t = i as f32 / sr;
        let whine = (TAU * f1 * t).sin() * 0.3 + (TAU * f2 * t).sin() * 0.12;
        lp += (rng.range(-1.0, 1.0) - lp) * 0.25;
        let rush = lp * 0.5 * (1.0 + 0.3 * (TAU * 6.5 * t).sin());
        out.push((whine + rush) * 0.5);
    }
    out
}

/// A tape playing in a player.
#[derive(Component)]
struct DeckVoice {
    deck: Entity,
    render: Render,
}

/// A player winding.
#[derive(Component)]
struct MotorVoice {
    deck: Entity,
}

fn cell_charge(
    deck: Entity,
    cells: &Query<(&Socket, &SocketOwner), With<DeckCell>>,
    items: &Query<&Item>,
    catalog: &Catalog,
) -> f32 {
    cells
        .iter()
        .find(|(_, o)| o.0 == deck)
        .and_then(|(s, _)| s.occupant)
        .and_then(|c| items.get(c).ok())
        .map_or(0.0, |i| {
            let full = catalog.get(i.kind()).battery_wh;
            if full > 0.0 {
                i.0.charge_wh / full
            } else {
                0.0
            }
        })
}

/// Tapes play where their player is, at its speed; a dying cell drags
/// them flat, and with a wobble.
fn play_decks(
    mut commands: Commands,
    mut renders: ResMut<Renders>,
    decks: Query<(Entity, &TapeDeck)>,
    tapes_q: Query<&Item>,
    cells: Query<(&Socket, &SocketOwner), With<DeckCell>>,
    catalog: Res<Catalog>,
    mut voices: Query<(Entity, &DeckVoice, &mut PlaybackSettings, Option<&Sampler>)>,
    motors: Query<(Entity, &MotorVoice)>,
    time: Res<Time>,
) {
    let mut playing: HashMap<Entity, Render> = HashMap::new();
    for (e, d) in &decks {
        if d.deck.mode != Mode::Playing {
            continue;
        }
        let Some(id) = d
            .tape
            .and_then(|t| tapes_q.get(t).ok())
            .map(|i| catalog.get(i.kind()).id.as_str())
            .and_then(|id| tapes::get(id).map(|def| def.item))
        else {
            continue;
        };
        let side = d.deck.tape.map_or(0, |(_, s)| s.index());
        playing.insert(e, Render::TapeSide(id, side));
    }
    let t = time.elapsed_secs();
    // Keep or stop what's playing.
    let mut have: HashMap<Entity, Entity> = HashMap::new();
    for (v, voice, mut settings, sampler) in &mut voices {
        let Some(want) = playing.get(&voice.deck) else {
            commands.entity(v).despawn();
            continue;
        };
        if *want != voice.render {
            commands.entity(v).despawn();
            continue;
        }
        let Ok((_, d)) = decks.get(voice.deck) else {
            commands.entity(v).despawn();
            continue;
        };
        // Still in use: keep its render.
        renders.want(voice.render.clone());
        let charge = cell_charge(voice.deck, &cells, &tapes_q, &catalog);
        let mut speed = Deck::motor_speed(charge);
        if speed < 1.0 {
            // A tired motor doesn't sag evenly.
            speed *= 1.0 + 0.012 * (t * 3.1).sin() + 0.006 * (t * 7.3).sin();
        }
        if (settings.speed - speed as f64).abs() > 0.002 {
            settings.speed = speed as f64;
        }
        // Drifted from the mechanism (a stall, a hitch): back in step.
        if let Some(at) = sampler.and_then(|s| s.try_playhead_seconds()) {
            if (at as f32 - d.deck.position()).abs() > 1.0 {
                commands.entity(v).despawn();
                continue;
            }
        }
        have.insert(voice.deck, v);
    }
    for (deck, render) in playing {
        if have.contains_key(&deck) {
            continue;
        }
        let Some(sample) = renders.want(render.clone()) else {
            continue;
        };
        let Ok((_, d)) = decks.get(deck) else {
            continue;
        };
        let charge = cell_charge(deck, &cells, &tapes_q, &catalog);
        let volume = mix::db("tape.music") + 20.0 * d.volume.max(0.01).log10();
        commands.spawn((
            DeckVoice { deck, render },
            SamplePlayer::new(sample).with_volume(Volume::Decibels(volume)),
            PlaybackSettings::default()
                .with_play_from(PlayFrom::Seconds(d.deck.position() as f64))
                .with_speed(Deck::motor_speed(charge).max(0.01) as f64),
            Transform::default(),
            Placed::new("tape.music"),
            ChildOf(deck),
        ));
    }
    // The motor, while it winds.
    let mut winding: Vec<Entity> = decks
        .iter()
        .filter(|(_, d)| matches!(d.deck.mode, Mode::Rewinding | Mode::Winding))
        .map(|(e, _)| e)
        .collect();
    for (v, m) in &motors {
        if let Some(i) = winding.iter().position(|d| *d == m.deck) {
            winding.swap_remove(i);
        } else {
            commands.entity(v).despawn();
        }
    }
    for deck in winding {
        let Some(sample) = renders.want(Render::Motor) else {
            continue;
        };
        commands.spawn((
            MotorVoice { deck },
            SamplePlayer::new(sample)
                .looping()
                .with_volume(Volume::Decibels(mix::db("tape.motor"))),
            PlaybackSettings::default(),
            Transform::default(),
            Placed::new("tape.motor"),
            ChildOf(deck),
        ));
    }
}

/// A station (or, `None`, the static) coming out of a radio.
#[derive(Component)]
struct RadioVoice {
    radio: Entity,
    station: Option<&'static str>,
}

/// Station loops that are files rather than renders (the voiced ones).
#[derive(Resource, Default)]
struct StationFiles(HashMap<&'static str, Handle<AudioSample>>);

/// The volume a station at `gain` (0..1 amplitude) plays at.
fn station_db(cue: &str, gain: f32) -> f32 {
    mix::db(cue) + 20.0 * gain.max(1e-4).log10()
}

/// Radios out loud play what their dial picks up: each station heard,
/// and the static, at their share, crossfading as the dial turns. A
/// station runs on the air whether you're listening or not, so tuning in
/// lands mid-broadcast.
fn play_radios(
    mut commands: Commands,
    mut renders: ResMut<Renders>,
    mut files: ResMut<StationFiles>,
    server: Res<AssetServer>,
    radios: Query<(Entity, &super::dial::Tuner)>,
    voices: Query<(Entity, &RadioVoice, &SampleEffects)>,
    mut volumes: Query<&mut VolumeNode>,
    samples: Res<Assets<AudioSample>>,
    raid: Option<Res<crate::raid::Raid>>,
    time: Res<Time>,
) {
    let day = raid.as_ref().map_or(0, |r| r.raids as u64);
    // What each radio wants: (station or static, amplitude).
    let mut want: HashMap<(Entity, Option<&'static str>), f32> = HashMap::new();
    for (e, t) in &radios {
        if !t.on() || !t.aloud {
            continue;
        }
        for h in &t.reception.stations {
            if h.gain > 0.01 {
                want.insert((e, Some(h.id)), h.gain);
            }
        }
        if t.reception.static_gain > 0.01 {
            want.insert((e, None), t.reception.static_gain);
        }
    }
    let mut have = std::collections::HashSet::new();
    for (v, voice, effects) in &voices {
        let key = (voice.radio, voice.station);
        let Some(gain) = want.get(&key) else {
            commands.entity(v).despawn();
            continue;
        };
        have.insert(key);
        match voice.station {
            None => {
                renders.want(Render::Static);
            }
            Some(id) => {
                if stations::get(id).is_some_and(|d| d.asset.is_none()) {
                    renders.want(Render::Station(id, day));
                }
            }
        }
        let cue = if voice.station.is_some() {
            "radio.station"
        } else {
            "radio.hiss"
        };
        if let Ok(mut vol) = volumes.get_effect_mut(effects) {
            vol.volume = Volume::Decibels(station_db(cue, *gain) - mix::db(cue));
        }
    }
    for ((radio, station), gain) in want {
        if have.contains(&(radio, station)) {
            continue;
        }
        let (sample, cue) = match station {
            None => (renders.want(Render::Static), "radio.hiss"),
            Some(id) => {
                let def = stations::get(id);
                let sample = match def.and_then(|d| d.asset) {
                    Some(path) => Some(
                        files
                            .0
                            .entry(id)
                            .or_insert_with(|| server.load(path))
                            .clone(),
                    ),
                    None if def.is_some_and(|d| d.kind == Kind::Chatter) => None,
                    None => renders.want(Render::Station(id, day)),
                };
                (sample, "radio.station")
            }
        };
        let Some(sample) = sample else {
            continue;
        };
        let rate = renders.rate.unwrap_or(RATE) as f32;
        let len = samples
            .get(&sample)
            .map_or(0.0, |s| s.get().len_frames() as f32 / rate);
        let at = stations::on_air_at(time.elapsed_secs_f64() + day as f64 * 86_400.0, len);
        commands.spawn((
            RadioVoice { radio, station },
            SamplePlayer::new(sample)
                .looping()
                .with_volume(Volume::Decibels(mix::db(cue))),
            PlaybackSettings::default().with_play_from(PlayFrom::Seconds(at as f64)),
            Transform::default(),
            sample_effects![
                VolumeNode {
                    volume: Volume::Decibels(station_db(cue, gain) - mix::db(cue)),
                    ..default()
                },
                mix::spatial(cue),
                crate::audio::reverb_send(cue)
            ],
            ChildOf(radio),
        ));
    }
}

/// The rendered key nearest `key`, and the speed that shifts it there.
pub fn piano_source(key: u8) -> (u8, f64) {
    use super::piano::{HIGH, LOW};
    let base = (LOW + 1 + ((key.saturating_sub(LOW)) / 3) * 3).min(HIGH);
    (base, 2f64.powf((key as f64 - base as f64) / 12.0))
}

/// A piano note sounding, and once damped how long it has left.
#[derive(Component)]
struct PianoVoice {
    piano: Entity,
    key: u8,
    damping: Option<f32>,
    level: f32,
}

/// The dampers' fall (s).
const DAMP: f32 = 0.18;

/// Notes struck sound from their key; notes let go are damped. Near a
/// piano its notes are rendered ahead, so the first one isn't late.
fn sound_pianos(
    mut commands: Commands,
    mut renders: ResMut<Renders>,
    mut notes: MessageReader<super::piano::PianoNote>,
    pianos: Query<&GlobalTransform, With<super::piano::Piano>>,
    ears: Query<&GlobalTransform, With<SpatialListener3D>>,
    mut voices: Query<(Entity, &mut PianoVoice, &SampleEffects)>,
    mut volumes: Query<&mut VolumeNode>,
    time: Res<Time>,
) {
    use super::piano::{HIGH, LOW};
    let near = ears.iter().any(|e| {
        pianos
            .iter()
            .any(|p| p.translation().distance(e.translation()) < 8.0)
    });
    if near {
        for k in (LOW + 1..=HIGH).step_by(3) {
            renders.want(Render::PianoKey(k));
        }
    }
    for n in notes.read() {
        // A key struck again, or let go: what was sounding is damped.
        for (_, mut v, _) in &mut voices {
            if v.piano == n.piano && v.key == n.key && v.damping.is_none() {
                v.damping = Some(DAMP);
            }
        }
        if !n.on {
            continue;
        }
        let (base, speed) = piano_source(n.key);
        let Some(sample) = renders.want(Render::PianoKey(base)) else {
            continue;
        };
        let level = 20.0 * n.velocity.max(0.05).log10();
        commands.spawn((
            PianoVoice {
                piano: n.piano,
                key: n.key,
                damping: None,
                level,
            },
            SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db("piano.note"))),
            PlaybackSettings::default().with_speed(speed),
            Transform::from_translation(n.at),
            sample_effects![
                VolumeNode {
                    volume: Volume::Decibels(level),
                    ..default()
                },
                mix::spatial("piano.note"),
                crate::audio::reverb_send("piano.note")
            ],
        ));
    }
    let dt = time.delta_secs();
    for (e, mut v, effects) in &mut voices {
        let Some(left) = v.damping.as_mut() else {
            continue;
        };
        *left -= dt;
        if *left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let k = (*left / DAMP).clamp(0.0, 1.0);
        let db = v.level + 40.0 * k.log10().max(-2.0);
        if let Ok(mut vol) = volumes.get_effect_mut(effects) {
            vol.volume = Volume::Decibels(db);
        }
    }
}

pub struct MusicSoundPlugin;

impl Plugin for MusicSoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Renders>()
            .init_resource::<StationFiles>()
            .add_systems(
                Update,
                (collect_renders, play_decks, play_radios, sound_pianos).chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(path: &std::path::Path, x: &[f32], sr: u32) {
        let mut data = Vec::with_capacity(x.len() * 4);
        for v in x {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&3u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&sr.to_le_bytes());
        w.extend_from_slice(&(sr * 4).to_le_bytes());
        w.extend_from_slice(&4u16.to_le_bytes());
        w.extend_from_slice(&32u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(data.len() as u32).to_le_bytes());
        w.extend_from_slice(&data);
        std::fs::write(path, w).unwrap();
    }

    /// One piece for each synthesised set the mix names, levelled as the
    /// game plays it (before the cue's gain), for the audio audit
    /// (`scripts/audio_music.py`): target/audio_audit/music/<set>.wav,
    /// float, mono, 48 kHz.
    #[test]
    fn renders_for_the_audit() {
        let dir = crate::asset_root_path().join("../target/audio_audit/music");
        let _ = std::fs::create_dir_all(&dir);
        let sr = RATE;
        let cut = |mut x: Vec<f32>, secs: f32| {
            x.truncate((secs * sr as f32) as usize);
            x
        };
        let first = tapes::TAPES[0].item;
        let pieces: Vec<(&str, Vec<f32>)> = vec![
            ("music/tape", cut(Render::TapeSide(first, 0).run(sr), 60.0)),
            ("music/motor", Render::Motor.run(sr)),
            ("music/station", Render::Station("far_music", 0).run(sr)),
            ("music/static", Render::Static.run(sr)),
            ("music/gramophone", cut(Render::Gramophone(0).run(sr), 60.0)),
            ("music/piano", cut(Render::PianoTune(0).run(sr), 60.0)),
            ("music/organ", cut(Render::OrganTune(0).run(sr), 60.0)),
        ];
        for (set, x) in pieces {
            assert!(is_synth_set(set), "{set}");
            assert!(!x.is_empty() && x.iter().all(|v| v.is_finite()), "{set}");
            wav(&dir.join(format!("{}.wav", set.replace('/', "_"))), &x, sr);
        }
    }

    /// Every cue in the music's part of the mix names a set made here.
    #[test]
    fn every_music_cue_is_rendered() {
        for c in crate::audio_mix::MIX {
            if c.set.starts_with("music/") {
                assert!(is_synth_set(c.set), "{}: {}", c.name, c.set);
            }
        }
    }
}
