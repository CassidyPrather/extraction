//! Sound: recorded, mastered CC0 sets (see `docs/AUDIO.md`) played through
//! `bevy_seedling` with basic spatialisation.
//!
//! Everything here listens to gameplay messages — footsteps, gun events,
//! hits, grabs — and turns them into sound. Nothing in the game logic
//! knows audio exists, so the headless sims don't either.

use bevy::prelude::*;
use bevy_seedling::prelude::*;
use extraction_core::firearm::{Event as FireEvent, Round};
use extraction_core::noise::Rng;
use extraction_core::terrain::Surface;
use std::collections::HashMap;

use crate::audio_mix as mix;
use crate::audio_score::{self, ScoreNode};
use crate::interact::InteractEvent;
use crate::player::Footstep;
use crate::rig::RigHead;
use crate::robots::Robot;
use crate::robots::drone::Drone;
use crate::weapons::GunEvent;
use crate::weapons::ballistics::Shot;
use crate::world::Zone;
use crate::world::surfaces::{Surfaces, footstep_set, underfoot};
use avian3d::prelude::*;

/// Every sound set by name (`"guns/rifle_shot"`), as loaded handles.
#[derive(Resource, Default)]
pub struct SoundBank {
    sets: HashMap<String, Vec<Handle<AudioSample>>>,
}

impl SoundBank {
    pub fn has(&self, set: &str) -> bool {
        self.sets.contains_key(set)
    }

    pub(crate) fn pick(&self, set: &str, rng: &mut Rng) -> Option<Handle<AudioSample>> {
        let v = self.sets.get(set)?;
        (!v.is_empty()).then(|| v[rng.below(v.len())].clone())
    }
}

#[derive(Resource)]
pub(crate) struct AudioRng(pub(crate) Rng);

/// A positioned, looping sound that follows its parent (drone rotors).
#[derive(Component)]
struct RotorLoop;

/// The rain bed, as loud as the weather says.
#[derive(Component)]
struct RainLoop;

/// A roof's own sound in the rain (`acoustics::rain_under`): tin
/// drumming, a tent's patter.
#[derive(Component)]
struct RoofRain {
    set: &'static str,
    /// The heavy-rain loop (more and bigger drops) or the light one.
    heavy: bool,
}

/// Water that runs where a map says it does, beyond its rivers: Sallow's
/// scour valves, still open, roaring out of the valve house at the dam's
/// toe into the river (loud at the toe and the face stair's foot, fading
/// up the valley). (map, xz, height above ground.) The creek's recording,
/// slowed and louder, until an outflow can be recorded.
pub const OUTFLOWS: &[(&str, [f32; 2], f32)] = &[("Sallow Reservoir", [12.0, 176.0], 1.0)];

#[derive(Component)]
struct OutflowLoop;

fn outflows(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    existing: Query<(), With<OutflowLoop>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    mut done: Local<bool>,
) {
    let Some(zone) = zone else {
        return;
    };
    if *done || !existing.is_empty() {
        return;
    }
    *done = true;
    for &(map, [x, z], up) in OUTFLOWS {
        if zone.map.name != map {
            continue;
        }
        if let Some(sample) = bank.pick("ambience/creek", &mut rng.0) {
            commands.spawn((
                OutflowLoop,
                SamplePlayer::new(sample)
                    .looping()
                    .with_volume(Volume::Decibels(mix::db("water.scour"))),
                PlaybackSettings::default().with_speed(0.75),
                Transform::from_xyz(x, zone.ground(x, z) + up, z),
                Placed::new("water.scour"),
            ));
        }
    }
}

/// A downpour's swell: (dB, seconds).
pub const RAIN_SWELL: (f32, f32) = (4.0, 4.0);

/// Thunder after each flash (`world::weather::Lightning`), as far behind
/// it as sound takes (`storm::thunder_delay`, 343 m/s), from the strike's
/// direction:
///
/// - near (under [`THUNDER_NEAR`]): a sharp crack, echoing the flash's
///   strokes, then the long tearing roll;
/// - far: the low rumble only, darker the further (a 1.5 kHz roll at
///   600 m to 300 Hz at 2.6 km).
///
/// Louder the nearer and the brighter the strike. (In a storm the far
/// thunder of `ambient_oneshots` keeps quiet: every roll is a flash's.)
pub const THUNDER_NEAR: f32 = 600.0;

fn storm_sounds(
    mut commands: Commands,
    mut flashes: MessageReader<crate::world::weather::Lightning>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut pending: Local<Vec<(f32, Vec3, f32, f32)>>,
) {
    let now = time.elapsed_secs();
    for f in flashes.read() {
        let delay = crate::world::storm::thunder_delay(f.distance);
        pending.push((now + delay, f.at, f.distance, f.strength));
    }
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let due: Vec<_> = pending.iter().filter(|p| p.0 <= now).copied().collect();
    pending.retain(|p| p.0 > now);
    for (_, at, d, strength) in due {
        // From the strike's direction, a fixed way off and up: its level is
        // set here, by distance, not by the falloff.
        let dir = (at - ear).with_y(0.0).normalize_or(Vec3::X);
        let from = ear + dir * 120.0 + Vec3::Y * 60.0;
        let db = thunder_db(d, strength);
        if d < THUNDER_NEAR {
            let fc = 8000.0 - 5000.0 * ((d - 250.0) / 350.0).clamp(0.0, 1.0);
            play_filtered_db(
                &mut commands,
                &bank,
                &mut rng.0,
                "explosions/grenade_sharp",
                from,
                "thunder.crack",
                db,
                0.7,
                fc,
            );
        }
        let fc = thunder_cutoff(d);
        play_filtered_db(
            &mut commands,
            &bank,
            &mut rng.0,
            "ambience/thunder_distant",
            from,
            "thunder.roll",
            db,
            0.85,
            fc,
        );
    }
}

/// Thunder's level against its cue, dB: 0 at 250 m from a full strike,
/// 6 dB down each doubling of distance, and down to -6 dB for a faint one.
pub fn thunder_db(distance: f32, strength: f32) -> f32 {
    -6.0 * (distance.max(250.0) / 250.0).log2() - 6.0 * (1.0 - strength.clamp(0.0, 1.0))
}

/// The roll's low-pass by distance, Hz: 1.5 kHz at 600 m to 300 Hz at
/// 2.6 km and beyond, and open up close.
pub fn thunder_cutoff(distance: f32) -> f32 {
    if distance < THUNDER_NEAR {
        return 4000.0;
    }
    let t = ((distance - THUNDER_NEAR) / 2000.0).clamp(0.0, 1.0);
    (1500f32.ln() + (300f32.ln() - 1500f32.ln()) * t).exp()
}

/// The rain bed follows the weather, and where you are: duller and quieter
/// under a roof (a tin roof drumming over you if there's a recording for
/// it), gone underground. Eased, so a doorway is a crossfade.
fn rain_volume(
    mut commands: Commands,
    conditions: Res<crate::world::weather::Conditions>,
    listening: Res<Listening>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    loops: Query<&SampleEffects, With<RainLoop>>,
    roofs: Query<(&RoofRain, &SampleEffects)>,
    mut volumes: Query<&mut VolumeNode>,
    mut lowpass: Query<&mut FastLowpassNode>,
    mut now: Local<Option<(f32, f32)>>,
    mut drums: Local<HashMap<&'static str, f32>>,
    mut swell: Local<(f32, f32)>,
) {
    // In a downpour, a wall of rain sweeps over every 10-20 s (the rain
    // sheets crossing the valley): up 4 dB and back over 4 s.
    let t = time.elapsed_secs();
    if t >= swell.0 {
        swell.1 = t;
        swell.0 = t + rng.0.range(10.0, 20.0);
    }
    let into = t - swell.1;
    let gust = if conditions.rain > 0.7 && into < RAIN_SWELL.1 {
        RAIN_SWELL.0 * (std::f32::consts::PI * into / RAIN_SWELL.1).sin()
    } else {
        0.0
    };
    let rain_db = if conditions.rain < 0.01 {
        None
    } else {
        Some(mix::db("rain") - 18.0 + 18.0 * conditions.rain.sqrt() + gust)
    };
    let (want_db, want_cut, drum) = crate::acoustics::rain_under(&listening.space);
    let k = (time.delta_secs() * 2.0).min(1.0);
    let (db, cut) = now.get_or_insert((want_db, want_cut));
    *db += (want_db - *db) * k;
    // Ease the cutoff in octaves.
    *cut = (cut.log2() + (want_cut.log2() - cut.log2()) * k).exp2();
    for effects in &loops {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = rain_db.map_or(Volume::SILENT, |r| Volume::Decibels(r + *db));
        }
        if let Ok(mut lp) = lowpass.get_effect_mut(effects) {
            if (lp.cutoff_hz - *cut).abs() > 1.0 {
                lp.cutoff_hz = *cut;
            }
        }
    }
    // The roof's drumming: started the first time you're under that roof
    // in the rain, faded in and out after.
    if let (Some(set), Some(_)) = (drum, rain_db) {
        if !drums.contains_key(set) && bank.has(set) {
            drums.insert(set, 0.0);
            for heavy in [true, false] {
                let name = if heavy {
                    set.to_string()
                } else {
                    format!("{set}_light")
                };
                if let Some(sample) = bank.pick(&name, &mut rng.0) {
                    commands.spawn((
                        RoofRain { set, heavy },
                        SamplePlayer::new(sample).looping(),
                        PlaybackSettings::default(),
                        sample_effects![VolumeNode {
                            volume: Volume::SILENT,
                            ..default()
                        }],
                    ));
                }
            }
        }
    }
    // Walled in, the roof's drumming; under an open roof, well under the
    // open rain (`acoustics::drum_level`).
    let drum_at = crate::acoustics::drum_level(&listening.space);
    for (set, level) in drums.iter_mut() {
        let target = if drum == Some(*set) && rain_db.is_some() {
            drum_at
        } else {
            0.0
        };
        *level += (target - *level) * k;
    }
    // The loops are levelled against the outdoor rain bed (light 6 LU
    // under heavy): they play at the rain cue's level, the two blended
    // by the rain's strength (equal power).
    let strength = conditions.rain.clamp(0.0, 1.0);
    for (roof, effects) in &roofs {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            let level = drums.get(roof.set).copied().unwrap_or(0.0);
            let share = if roof.heavy {
                strength.sqrt()
            } else {
                (1.0 - strength).sqrt()
            };
            let g = level * share;
            v.volume = match rain_db {
                Some(_) if g > 0.01 => Volume::Decibels(mix::db("rain") + 20.0 * g.log10()),
                _ => Volume::SILENT,
            };
        }
    }
}

/// The Surge's roar, faded with how much Surge is in the air.
#[derive(Component)]
struct SurgeDrone;

/// A ringing in the ears that grows with attention (`anomaly::Attention`).
#[derive(Component)]
struct AttentionLoop;

/// The ambient creek bed, moved along the bank to stay nearest the listener.
#[derive(Component)]
struct CreekLoop;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SeedlingPlugins)
            .register_node::<ScoreNode>()
            .insert_resource(AudioRng(Rng::new(0xA0D10)))
            .init_resource::<Listening>()
            .init_resource::<Hush>()
            .add_plugins(crate::audio_underground::UndergroundSoundsPlugin)
            .init_resource::<HushNow>()
            .add_plugins(crate::audio_kit::KitSoundsPlugin)
            .add_plugins(crate::audio_fight::FightSoundsPlugin)
            .add_plugins(crate::audio_machines::MachineSoundsPlugin)
            .init_resource::<crate::hearing::Hearing>()
            .init_resource::<Binaural>()
            .add_observer(place)
            .add_systems(Startup, (load_bank, load_hrir))
            .add_systems(PostStartup, (attach_listener, start_ambience))
            .add_systems(
                Update,
                (
                    footsteps,
                    gun_sounds,
                    shot_sounds,
                    hit_sounds,
                    hand_sounds,
                    rotor_loops,
                    gadget_loops,
                    door_sounds,
                    walker_steps,
                    pylon_sounds,
                    radio_sounds,
                    chatter_sounds,
                    splash_sounds,
                    lock_sounds,
                    drop_sounds,
                    break_sounds,
                ),
            )
            .add_systems(
                Update,
                (
                    button_sounds,
                    press_sounds,
                    far_sounds,
                    step_sounds,
                    truck_sounds,
                    use_sounds,
                    part_sounds,
                    pack_sounds,
                    landing_sounds,
                    gadget_sounds,
                    convoy_sounds,
                    dirigible_loops,
                    drone_and_zip_sounds,
                    kitchen_sounds,
                    mind_sounds,
                    (render_voices, voice_sounds).chain(),
                    casing_sounds,
                    detector_sounds,
                    power_sounds,
                    (
                        grenade_sounds,
                        gear_sounds,
                        armour_sounds,
                        epilogue_sounds,
                        night_ambience,
                        turret_sounds,
                        heavy_sounds,
                        heavy_loops,
                        flock_sounds,
                        flock_landing,
                        anomaly_voices,
                        tool_sounds,
                        hush_world,
                        slapbacks,
                        storm_sounds,
                        outflows,
                    ),
                ),
            )
            .add_systems(
                Update,
                (
                    indoor_ambience,
                    attention_hum,
                    ambient_oneshots,
                    move_creek,
                    surge_sounds,
                    anomaly_loops,
                    anomaly_sounds,
                    rain_volume,
                    room_acoustics,
                    binaural_switch,
                    score_drive,
                    (
                        animal_sounds,
                        breath_sounds,
                        water_sounds,
                        sentry_sounds,
                        bat_sounds,
                        fell_sounds,
                    ),
                ),
            )
            // After everything that plays, so a sound is placed the frame
            // it starts.
            .add_systems(
                PostUpdate,
                occlude.after(bevy::transform::TransformSystems::Propagate),
            )
            .add_systems(Last, retire_voices);
    }
}

/// At most this many one-shot sounds at once; past it the oldest go.
pub const VOICE_CAP: usize = 256;
/// No one-shot in the bank runs longer than a minute (the sirens), even
/// slowed: one still playing after this never will.
pub const ONE_SHOT_MAX: f32 = 90.0;

/// Retires one-shot voices that outlive any sound, and the oldest past
/// [`VOICE_CAP`]. A one-shot despawns itself when it finishes playing, but
/// with no output device (none plugged in, or the headset's audio gone
/// mid-session) nothing ever finishes: every footstep, shot and creak
/// stayed as an entity, a few dozen a second (soak finding, `docs/SOAK.md`).
pub(crate) fn retire_voices(
    mut commands: Commands,
    voices: Query<(Entity, &SamplePlayer)>,
    time: Res<Time<Real>>,
    mut born: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    born.retain(|e, _| voices.contains(*e));
    let mut once: Vec<(f32, Entity)> = voices
        .iter()
        .filter(|(_, p)| matches!(p.repeat_mode, RepeatMode::PlayOnce))
        .map(|(e, _)| (*born.entry(e).or_insert(now), e))
        .collect();
    once.sort_by(|a, b| a.0.total_cmp(&b.0));
    let over = once.len().saturating_sub(VOICE_CAP);
    for (i, (t, e)) in once.into_iter().enumerate() {
        if i < over || now - t > ONE_SHOT_MAX {
            born.remove(&e);
            commands.entity(e).try_despawn();
        }
    }
}

fn load_bank(mut commands: Commands, server: Res<AssetServer>) {
    let path = crate::asset_root_path().join("audio/sounds.json");
    let mut bank = SoundBank::default();
    let Ok(text) = std::fs::read_to_string(&path) else {
        warn!("no assets/audio/sounds.json: running silent (run scripts/fetch_audio.py)");
        commands.insert_resource(bank);
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        warn!("sounds.json does not parse");
        commands.insert_resource(bank);
        return;
    };
    if let Some(obj) = json.as_object() {
        for (name, set) in obj {
            let files: Vec<Handle<AudioSample>> = set["files"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|f| f["file"].as_str())
                        .map(|f| server.load(f.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            bank.sets.insert(name.clone(), files);
        }
    }
    info!("sound bank: {} sets", bank.sets.len());
    commands.insert_resource(bank);
}

/// The local player's head hears (it follows the headset in VR, the
/// mouse-look camera when flat).
fn attach_listener(
    mut commands: Commands,
    heads: Query<(Entity, &ChildOf), With<RigHead>>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
) {
    for (e, parent) in &heads {
        if locals.contains(parent.parent()) {
            commands.entity(e).insert(SpatialListener3D);
        }
    }
}

/// The reverb bus's input: every placed sound sends here
/// (`acoustics`), and the room's reverb comes back from it.
#[derive(NodeLabel, PartialEq, Eq, Debug, Hash, Clone)]
pub struct ReverbIn;

/// A sound placed in the world. As it spawns (`place`) it gets its
/// effects: in the headset with binaural on, the head-related filter
/// (`HRTF_VOICES` at a time, the rest panned); otherwise panned. Either
/// way its distance falloff and muffle (and occlusion, set by `occlude`),
/// then a send to the reverb.
#[derive(Component, Clone, Copy, Debug)]
pub struct Placed(&'static str);

impl Placed {
    pub fn new(cue: &str) -> Self {
        Self(mix::static_name(cue))
    }
}

/// Placed sounds heard through the head-related filter at once (each
/// costs a convolution); past this, new ones are panned.
pub const HRTF_VOICES: usize = 24;

/// Past this distance (m) a placed sound is panned: far off, its muffle
/// and reverb tell where it is more than the ears' filter does, and the
/// voices are kept for what's near...
pub const BINAURAL_REACH: f32 = 40.0;
/// ...except a Signal (a threat: a drone, a walker's gait, a sentry's
/// chirp), which keeps the ears' filter as far as it matters: panned, a
/// drone at 60 m is no more ahead than behind, nor above
/// (`scripts/audio_localise.py`).
pub const SIGNAL_BINAURAL_REACH: f32 = 150.0;

/// How far a placed sound playing `cue` keeps the head-related filter.
pub fn binaural_reach(cue: &str) -> f32 {
    if mix::MIX
        .iter()
        .any(|c| c.name == cue && c.role == mix::Role::Signal)
    {
        SIGNAL_BINAURAL_REACH
    } else {
        BINAURAL_REACH
    }
}

/// Panned (flat mode, or past its binaural reach), a sound straight
/// behind you is muffled to this (Hz), easing in from your side: the
/// pinna's shade, so front and back still differ without the ears'
/// filter.
pub const BEHIND_CUTOFF: f32 = 5_000.0;

/// The panner's muffle (Hz) for a sound in direction `to` (from you) with
/// you facing `facing`: none ahead or beside, [`BEHIND_CUTOFF`] straight
/// behind, eased between in octaves.
pub fn behind_cutoff(facing: Vec3, to: Vec3) -> f32 {
    let (f, t) = (facing.xz().normalize_or_zero(), to.xz().normalize_or_zero());
    let behind = (-f.dot(t)).clamp(0.0, 1.0);
    (20_480f32.log2() + (BEHIND_CUTOFF.log2() - 20_480f32.log2()) * behind).exp2()
}

/// The head-related filter's FFT slices: 128-sample slices of a 385-tap
/// sphere make 512-point FFTs (`scripts/audio_hrir.py`, TAPS).
pub const HRTF_SLICE: usize = 128;

/// Binaural playback: on in the headset when the setting is (and the
/// head-related responses loaded); `sphere` is SADIE II's KEMAR
/// (`scripts/audio_hrir.py`).
#[derive(Resource, Default)]
pub struct Binaural {
    pub on: bool,
    sphere: Option<bevy_seedling::prelude::hrtf::SubjectBytes>,
}

/// A placed sound heard through the head-related filter.
#[derive(Component)]
struct BinauralVoice;

fn load_hrir(mut binaural: ResMut<Binaural>) {
    let path = crate::asset_root_path().join("audio/hrir/sadie_d2_kemar.bin");
    match std::fs::read(&path) {
        Ok(bytes) => {
            binaural.sphere = Some(bevy_seedling::prelude::hrtf::SubjectBytes::new(&bytes))
        }
        Err(e) => warn!("{}: {e}; placed sounds stay panned", path.display()),
    }
}

fn binaural_switch(
    mut binaural: ResMut<Binaural>,
    driver: Res<crate::rig::RigDriver>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
) {
    let on = *driver == crate::rig::RigDriver::Xr
        && comfort.is_none_or(|c| c.binaural)
        && binaural.sphere.is_some();
    if binaural.on != on {
        binaural.on = on;
    }
}

/// Whether a placed sound `distance` m off, with `voices` already
/// binaural, is heard through the head-related filter, out to `reach`
/// ([`binaural_reach`]).
pub fn binaural_voice(on: bool, voices: usize, distance: f32, reach: f32) -> bool {
    on && voices < HRTF_VOICES && distance < reach
}

fn place(
    add: On<Add, Placed>,
    placed: Query<(&Placed, Option<&Transform>)>,
    voices: Query<(), With<BinauralVoice>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    binaural: Res<Binaural>,
    mut commands: Commands,
) {
    use bevy_seedling::prelude::{
        HrtfConfig,
        hrtf::{FftSize, HrirSource},
    };
    let Ok((&Placed(cue), at)) = placed.get(add.entity) else {
        return;
    };
    let distance = match (at, listener.iter().next()) {
        (Some(at), Some(ear)) => at.translation.distance(ear.translation()),
        _ => 0.0,
    };
    let mut e = commands.entity(add.entity);
    match binaural.sphere.as_ref() {
        Some(sphere)
            if binaural_voice(
                binaural.on,
                voices.iter().count(),
                distance,
                binaural_reach(cue),
            ) =>
        {
            e.insert((
                BinauralVoice,
                sample_effects![
                    VolumeNode::default(),
                    (
                        mix::hrtf(cue),
                        HrtfConfig {
                            hrir_sphere: HrirSource::InMemory(sphere.clone()),
                            fft_size: FftSize {
                                slice_count: 4,
                                slice_len: HRTF_SLICE,
                            },
                            ..default()
                        }
                    ),
                    reverb_send(cue)
                ],
            ));
        }
        _ => {
            e.insert(sample_effects![mix::spatial(cue), reverb_send(cue)]);
        }
    }
}

/// The reverb bus: a level for the room, then the reverb, to the main
/// bus. `room_acoustics` sets both from where you stand.
fn spawn_reverb_bus(commands: &mut Commands) {
    commands
        .spawn((
            ReverbIn,
            VolumeNode {
                volume: Volume::SILENT,
                ..default()
            },
        ))
        .chain_node(FreeverbNode {
            room_size: 0.5,
            damping: 0.5,
            width: 0.8,
            ..default()
        });
}

/// How much of a cue goes to the reverb: all of it, but for recordings
/// with their own space baked in (the far-off shot's echo, the siren's
/// valley) and the beds.
pub(crate) fn reverb_send(cue: &str) -> SendNode {
    let share = match mix::role(cue) {
        Some(mix::Role::Bed) => 0.0,
        _ if matches!(cue, "gun.distant" | "surge.siren" | "surge.crack") => 0.3,
        _ => 1.0,
    };
    SendNode::new(Volume::Linear(share), ReverbIn)
}

/// Plays one variant of a set at a point (or unspatialised with `None`),
/// at the cue's gain (`audio_mix`) plus `offset_db`, falling off from the
/// cue's reach.
#[allow(clippy::too_many_arguments)]
pub(crate) fn play(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    set: &str,
    at: Option<Vec3>,
    cue: &str,
    offset_db: f32,
    pitch: f64,
) {
    play_at_speed(commands, bank, rng, set, at, cue, offset_db, 1.0, pitch);
}

/// [`play`], centred on `speed` (below 1 is lower and slower: heavier).
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_at_speed(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    set: &str,
    at: Option<Vec3>,
    cue: &str,
    offset_db: f32,
    speed: f64,
    pitch: f64,
) {
    let Some(sample) = bank.pick(set, rng) else {
        return;
    };
    let speed = speed * (1.0 + (rng.f32() as f64 - 0.5) * 2.0 * pitch);
    let player = SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue) + offset_db));
    let settings = PlaybackSettings::default().with_speed(speed);
    match at {
        Some(p) => {
            commands.spawn((
                player,
                settings,
                Transform::from_translation(p),
                Placed::new(cue),
            ));
        }
        None => {
            commands.spawn((player, settings));
        }
    }
}

fn surface_set(s: Surface) -> &'static str {
    match s {
        Surface::Grass => "footsteps/grass",
        Surface::Forest => "footsteps/leaves",
        Surface::Dirt => "footsteps/mud",
        Surface::Mud => "footsteps/mud",
        Surface::Rock | Surface::Asphalt => "footsteps/concrete",
        Surface::Gravel => "footsteps/gravel",
        Surface::Water => "footsteps/water",
    }
}

#[allow(clippy::too_many_arguments)]
fn footsteps(
    mut commands: Commands,
    mut steps: MessageReader<Footstep>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    zone: Res<Zone>,
    trucks: Query<(&GlobalTransform, &crate::raid::truck::Truck)>,
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &Surfaces)>,
    wetness: Option<Res<crate::world::weather::Wetness>>,
    snow: Option<Res<crate::world::snow::Snow>>,
) {
    let wet = wetness.map_or(0.0, |w| w.level);
    for s in steps.read() {
        let set = ground_set(
            &zone,
            &trucks,
            &spatial,
            &built,
            s.position,
            wet,
            snow.as_deref(),
        );
        let offset = -6.0 + (s.speed / 4.0).min(1.0) * 6.0;
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(s.position),
            "step",
            offset,
            0.06,
        );
    }
}

/// How deep snow must lie (`world::snow::Snow::lies`) to crunch underfoot.
const SNOW_UNDERFOOT: f32 = 0.25;

/// How much standing water (`world::puddles::puddle_at`) a step must land
/// in to splash.
const PUDDLE_SPLASH: f32 = 0.5;

/// Your kit on each step (`foley`): plates knock, swivels jingle, the
/// pack's load shifts, water sloshes; each at a level by how far it
/// carries (its reach over [`GEAR_REACH_REF`]), so what robots hear
/// farther you hear louder.
fn gear_sounds(
    mut commands: Commands,
    mut rattles: MessageReader<crate::foley::GearRattle>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for g in rattles.read() {
        let r = g.rattle;
        for (reach, set, cue, speed) in [
            (r.plates, "physical/drop_metal", "gear.plate", 0.7),
            (r.swivels, "physical/handle_metal", "gear.swivel", 1.6),
            (r.pack, "physical/cloth", "gear.pack", 0.85),
            (r.slosh, "footsteps/water", "gear.slosh", 1.2),
        ] {
            if reach <= 0.0 || !rng.0.chance(gear_chance(reach)) {
                continue;
            }
            // Water in a bottle in a pouch: muffled.
            if cue == "gear.slosh" {
                let Some(sample) = bank.pick(set, &mut rng.0) else {
                    continue;
                };
                let db = mix::db(cue) + gear_db(reach);
                commands.spawn((
                    SamplePlayer::new(sample).with_volume(Volume::Decibels(db)),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(g.at),
                    sample_effects![
                        mix::spatial(cue),
                        FastLowpassNode::<2> {
                            cutoff_hz: SLOSH_LOWPASS,
                            ..default()
                        },
                        reverb_send(cue)
                    ],
                ));
                continue;
            }
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(g.at),
                cue,
                gear_db(reach),
                speed,
                0.08,
            );
        }
    }
}

/// The slosh's low-pass, Hz (`scripts/audio_mix.py` CUE_LOWPASS).
const SLOSH_LOWPASS: f32 = 900.0;

/// A part of the kit heard to this far (m) plays at its cue's gain.
pub const GEAR_REACH_REF: f32 = 5.0;

/// A part's level against its cue, by its reach: twice as far, 6 dB up.
pub fn gear_db(reach: f32) -> f32 {
    20.0 * (reach / GEAR_REACH_REF).max(0.05).log10()
}

/// How often a part rattles on a step: a quiet one now and then, a loud
/// one nearly every step.
fn gear_chance(reach: f32) -> f32 {
    (0.25 + reach / 10.0).min(0.95)
}

/// A round meeting worn armour (`weapons::armour::ArmourStruck`): stopped,
/// a steel clang and the thump through it; turned, a ping; through, the
/// clang and the wet hit after it.
fn armour_sounds(
    mut commands: Commands,
    mut struck: MessageReader<crate::weapons::armour::ArmourStruck>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use extraction_core::firearm_terminal::Outcome;
    for s in struck.read() {
        let at = Some(s.at);
        let parts: &[(&str, &str, f32, f64)] = match s.outcome {
            Outcome::Stop => &[
                ("impacts/metal", "armour.clang", 0.0, 0.9),
                ("physical/body_hit", "armour.thump", -3.0, 0.85),
            ],
            Outcome::Ricochet { .. } => &[("impacts/ricochet", "armour.ping", 0.0, 1.0)],
            Outcome::Through { .. } => &[
                ("impacts/metal", "armour.clang", -3.0, 0.95),
                ("physical/body_hit", "armour.thump", 0.0, 1.0),
            ],
        };
        for &(set, cue, db, speed) in parts {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                at,
                cue,
                db,
                speed,
                0.06,
            );
        }
    }
}

/// A scope's elevation drum turned one detent (`weapons::zero`): a small,
/// quick click.
fn turret_sounds(
    mut commands: Commands,
    mut clicks: MessageReader<crate::weapons::zero::TurretClicked>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for c in clicks.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "guns/safety",
            Some(c.at),
            "scope.click",
            0.0,
            1.7,
            0.04,
        );
    }
}

/// A grenade (`weapons::grenades`): the pin's click and the ring's scrape
/// coming away, the spoon's ping as it flies, and the bang, the RGD-5's
/// sharp and the F-1's heavier.
fn grenade_sounds(
    mut commands: Commands,
    mut events: MessageReader<crate::weapons::grenades::GrenadeEvent>,
    items: Query<&crate::items::Item>,
    catalog: Res<crate::items::Catalog>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use extraction_core::grenade::Event as E;
    for ev in events.read() {
        let at = Some(ev.at);
        let mut one = |set: &str, cue: &str, speed: f64| {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                at,
                cue,
                0.0,
                speed,
                0.05,
            );
        };
        match ev.event {
            E::PinPulled => {
                one("guns/safety", "grenade.pin", 1.1);
                one("physical/handle_metal", "grenade.scrape", 1.2);
            }
            E::SpoonOff => one("guns/casing", "grenade.spoon", 1.25),
            E::Exploded => {
                let heavy = items
                    .get(ev.grenade)
                    .is_ok_and(|i| catalog.get(i.kind()).id == "f1");
                if heavy {
                    one("explosions/grenade_heavy", "grenade.bang_heavy", 1.0);
                } else {
                    one("explosions/grenade_sharp", "grenade.bang", 1.0);
                }
            }
        }
    }
}

/// The footstep set for whatever is under `at`: the truck's bed, a built
/// floor, a puddle (on ground as wet as `wet`, `world::puddles`), a
/// drained reservoir's silt, or the ground.
fn ground_set(
    zone: &Zone,
    trucks: &Query<(&GlobalTransform, &crate::raid::truck::Truck)>,
    spatial: &SpatialQuery,
    built: &Query<(&GlobalTransform, &Surfaces)>,
    at: Vec3,
    wet: f32,
    snow: Option<&crate::world::snow::Snow>,
) -> &'static str {
    let p = [at.x, at.z];
    let in_truck = trucks
        .iter()
        .any(|(gt, t)| t.in_bed(gt, at + Vec3::Y * 0.2));
    // A floor, a deck, a stair: whatever's underfoot, if it's built.
    if in_truck {
        "footsteps/wood"
    } else if let Some(m) = underfoot(spatial, built, at) {
        footstep_set(m)
    } else if wet > 0.0 && crate::world::puddles::puddle_at(&zone.hm, p, wet) > PUDDLE_SPLASH {
        "footsteps/water"
    } else if snow.is_some_and(|s| s.lies(at.y) > SNOW_UNDERFOOT) {
        // Snow lying (`world::snow`): a crunch; gravel's until a recording
        // of snow can be fetched.
        "footsteps/gravel"
    } else if crate::world::terrain::drowned_line(&zone.map.name)
        .is_some_and(|d| d.at(p, at.y).0 > 0.5)
    {
        // A drained reservoir's bed (`world::terrain::Drowned`): silt,
        // as it looks.
        "footsteps/mud"
    } else {
        surface_set(zone.hm.surface(p, zone.forest_cover(p[0], p[1]) > 0.5))
    }
}

/// Coming down from a jump or a drop: both feet at once, harder the
/// faster you fell.
#[allow(clippy::too_many_arguments)]
fn landing_sounds(
    mut commands: Commands,
    bodies: Query<(Entity, &crate::player::Body, &GlobalTransform)>,
    zone: Res<Zone>,
    trucks: Query<(&GlobalTransform, &crate::raid::truck::Truck)>,
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &Surfaces)>,
    mut was: Local<HashMap<Entity, (bool, f32, f32)>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    wetness: Option<Res<crate::world::weather::Wetness>>,
    snow: Option<Res<crate::world::snow::Snow>>,
) {
    let wet = wetness.map_or(0.0, |w| w.level);
    for (e, body, gt) in &bodies {
        let before = was.insert(e, (body.grounded, body.velocity.y, body.airborne));
        let Some((was_grounded, vy, airborne)) = before else {
            continue;
        };
        if was_grounded || !body.grounded || body.swimming || airborne < 0.3 || vy > -2.5 {
            continue;
        }
        let at = gt.translation();
        let set = ground_set(&zone, &trucks, &spatial, &built, at, wet, snow.as_deref());
        let hard = ((-vy - 2.5) / 6.0).clamp(0.0, 1.0);
        for _ in 0..2 {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                "land",
                -6.0 + 6.0 * hard,
                0.92,
                0.05,
            );
        }
    }
}

fn shot_set(item: &str) -> &'static str {
    match item {
        "pm_pistol" => "guns/pistol_shot",
        "ak74" => "guns/rifle_shot",
        "mosin" => "guns/bolt_rifle_shot",
        "toz_double" => "guns/shotgun_shot",
        "mp133" => "guns/shotgun_shot",
        "sks" | "rpk74" => "guns/rifle_shot",
        "nagant" | "ppsh" => "guns/pistol_shot",
        "svd" => "guns/bolt_rifle_shot",
        "toz8" | "flare_pistol" | "crossbow" => "guns/pistol_shot",
        _ => "guns/rifle_shot",
    }
}

/// A gun's faults, each its own (`weapons::faults`): true if it was one.
///
/// - A squib: a weak, dull pop. The rifle's shot low-passed hard and far
///   down, no crack, nothing cycling: it must not sound like a shot.
/// - A bulge: a round fired into a squib's bullet. A muffled, wrong bang
///   with the barrel ringing after it.
/// - Rodded: the cleaning rod's scrape down the bore, and the bullet's
///   small tink as it drops out.
/// - A stuck bolt: a dull clack as the handle won't come up. Freed: the
///   slap's sharp metal smack, then the bolt opening.
/// - A slam-fire: the bolt's close is already there (it comes a tick
///   before the shot); a faint snap of the floating pin marks it wrong.
fn fault_sounds(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    event: &FireEvent,
    at: Vec3,
) -> bool {
    let layers = fault_layers(event);
    if layers.is_empty() {
        return false;
    }
    for &(set, cue, speed, cutoff) in layers {
        match cutoff {
            Some(hz) => play_filtered(commands, bank, rng, set, at, cue, speed, hz),
            None => play_at_speed(commands, bank, rng, set, Some(at), cue, 0.0, speed, 0.04),
        }
    }
    true
}

/// A fault's layers (set, cue, speed, low-pass): each stoppage its own, so
/// an ear tells which it is as it happens (`weapons::stoppages`).
///
/// - A stovepipe: the bolt stopping short on the case, and the case's
///   pinched clink in the port. A failure to feed: a light, hollow clack,
///   the bolt home on nothing (no brass). A double feed: a dull jam and a
///   grind. A short stroke: the bolt's draw, smothered.
/// - A dud: the hammer's fall dulled on a primer that doesn't go. A
///   hang-fire: the same fall, and the primer's faint fizz.
/// - A blocked trigger (the safety, a Nagant's open gate): a small creak,
///   the finger against the stop. A dead one (the hammer already down): a
///   faint tick, nothing falling.
pub fn fault_layers(
    event: &FireEvent,
) -> &'static [(&'static str, &'static str, f64, Option<f32>)] {
    use extraction_core::firearm::Malfunction::*;
    match event {
        FireEvent::Malfunctioned(FailureToEject) => &[
            ("guns/bolt_close", "gun.malfunction", 0.72, None),
            ("guns/casing", "gun.stovepipe", 0.8, None),
        ],
        FireEvent::Malfunctioned(FailureToFeed) => {
            &[("guns/bolt_close", "gun.ftf", 1.15, Some(2500.0))]
        }
        FireEvent::Malfunctioned(DoubleFeed) => &[
            ("guns/bolt_close", "gun.double_feed", 0.6, Some(1200.0)),
            ("physical/handle_metal", "gun.grind", 0.7, None),
        ],
        FireEvent::Malfunctioned(ShortStroke) => {
            &[("guns/bolt_open", "gun.short_stroke", 0.8, Some(2500.0))]
        }
        FireEvent::Dud => &[("guns/dry_fire", "gun.dud", 0.8, Some(12000.0))],
        FireEvent::TriggerBlocked => &[(
            "physical/handle_metal",
            "gun.trigger_block",
            1.5,
            Some(2500.0),
        )],
        // The trigger back slack on a hammer already down: the faint tick
        // of the disconnector, nothing falling.
        FireEvent::TriggerDead => &[("guns/dry_fire", "gun.trigger_dead", 1.6, Some(10000.0))],
        // The trigger back on an open action (a slide locked back): the
        // slide's hollow rattle on its catch, nothing falling.
        FireEvent::ActionOpen => &[("guns/bolt_open", "gun.action_open", 1.4, Some(3000.0))],
        FireEvent::Squib { .. } => &[("guns/rifle_shot", "gun.squib", 1.25, Some(450.0))],
        FireEvent::Bulged { .. } => &[
            ("guns/rifle_shot", "gun.bulged", 0.85, Some(1400.0)),
            ("impacts/metal", "gun.bulged_ring", 0.8, None),
        ],
        FireEvent::Rodded => &[
            ("physical/handle_metal", "gun.rod_scrape", 0.8, None),
            ("guns/casing", "gun.rod_tink", 1.35, None),
        ],
        FireEvent::BoltStuck => &[("guns/bolt_open", "gun.bolt_stuck", 0.6, Some(1800.0))],
        FireEvent::BoltFreed => &[
            ("impacts/metal", "gun.bolt_freed", 1.25, None),
            ("guns/bolt_open", "gun.bolt_open", 1.0, None),
        ],
        FireEvent::SlamFire => &[("physical/switch", "gun.slam_snap", 1.4, None)],
        // A hang-fire: the hammer's fall, and the primer fizzing; the shot
        // it owes comes as a `Fired` of its own.
        FireEvent::HangFire => &[
            ("guns/dry_fire", "gun.dry_fire", 1.0, None),
            ("physical/zip", "gun.hang_fizz", 1.8, Some(5000.0)),
        ],
        // Out of battery: the case bursts outside the chamber. A flat,
        // harsh crack (between a squib and a shot: no barrel behind it)
        // and the brass and gas clattering out of the open action.
        FireEvent::OutOfBattery => &[
            ("guns/rifle_shot", "gun.out_of_battery", 1.3, Some(3000.0)),
            ("physical/drop_metal", "gun.burst_case", 1.4, None),
        ],
        _ => &[],
    }
}

/// [`play_filtered`] with a level offset (dB) against its cue.
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_filtered_db(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    set: &str,
    at: Vec3,
    cue: &str,
    db: f32,
    speed: f64,
    hz: f32,
) {
    let Some(sample) = bank.pick(set, rng) else {
        return;
    };
    commands.spawn((
        SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue) + db)),
        PlaybackSettings::default().with_speed(speed),
        Transform::from_translation(at),
        sample_effects![
            mix::spatial(cue),
            FastLowpassNode::<2> {
                cutoff_hz: hz,
                ..default()
            },
            reverb_send(cue)
        ],
    ));
}

/// A placed one-shot through its own low-pass (`hz`), for sounds that
/// are some other sound with its edge taken off.
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_filtered(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    set: &str,
    at: Vec3,
    cue: &str,
    speed: f64,
    hz: f32,
) {
    let Some(sample) = bank.pick(set, rng) else {
        return;
    };
    commands.spawn((
        SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue))),
        PlaybackSettings::default().with_speed(speed),
        Transform::from_translation(at),
        sample_effects![
            mix::spatial(cue),
            FastLowpassNode::<2> {
                cutoff_hz: hz,
                ..default()
            },
            reverb_send(cue)
        ],
    ));
}

/// What a gun's handling sounds like, by event (the shot aside): the set
/// played and its mix cue. Each step of a reload has its own
/// (`weapons::reload`), so a hand working the gun unseen can tell them apart.
pub fn handling_sound(
    event: &FireEvent,
    action: Option<extraction_core::firearm::ActionKind>,
    item: &str,
) -> Option<(&'static str, &'static str)> {
    use extraction_core::firearm::ActionKind::*;
    Some(match (event, action) {
        // A crossbow loosed with no bolt on it: the string slams home on
        // nothing, hard enough to hurt the bow. Not a hammer's click, and
        // heavier than a string let go short.
        (FireEvent::DryFire, Some(Crossbow)) => ("physical/drop_wood", "gun.dry_loose"),
        (FireEvent::DryFire, _) => ("guns/dry_fire", "gun.dry_fire"),
        (FireEvent::MagazineInserted, _) => ("guns/mag_insert", "gun.mag_insert"),
        (FireEvent::MagazineRemoved, _) => ("guns/mag_release", "gun.mag_release"),
        // A detent snapping over: a switch, not a tick, so it's heard in a
        // fight (`scripts/audio_gunstate.py`).
        (FireEvent::ModeChanged(_), _) => ("physical/switch", "gun.safety"),
        (FireEvent::RoundLoaded, Some(BreakAction | Pump)) => {
            ("guns/shell_insert", "gun.shell_insert")
        }
        (FireEvent::RoundLoaded, _) => ("guns/stripper_clip", "gun.stripper_clip"),
        (FireEvent::BoltClosed, Some(BoltAction)) => ("guns/bolt_close", "gun.bolt_close"),
        (FireEvent::BoltClosed, Some(BreakAction)) => ("guns/shotgun_close", "gun.shotgun_close"),
        // A pump run home: the fore-end slapping forward and the bolt
        // locking up behind it.
        (FireEvent::BoltClosed, Some(Pump)) => ("guns/shotgun_close", "gun.pump_fwd"),
        (FireEvent::BoltClosed, Some(SemiAuto)) if item == "ak74" => {
            ("guns/charging_handle", "gun.charging")
        }
        (FireEvent::BoltClosed, Some(SemiAuto)) => ("guns/slide_rack", "gun.slide_rack"),
        (FireEvent::Cocked, Some(BoltAction)) => ("guns/bolt_open", "gun.bolt_open"),
        (FireEvent::Cocked, Some(BreakAction)) => ("guns/shotgun_open", "gun.shotgun_open"),
        // A pump racked back: the fore-end's slide and the bolt unlocking.
        (FireEvent::Cocked, Some(Pump)) => ("guns/shotgun_open", "gun.pump_back"),
        // A spent case is heard where it lands (`casing_sounds`), a beat
        // after the shot; a live round worked out by hand, here.
        (FireEvent::Ejected(Round::Live), _) => ("guns/casing", "gun.casing"),
        // The carrier running home after the shot, a beat behind it,
        // and an open bolt slamming forward just before it.
        (FireEvent::CarrierHome, _) => ("guns/bolt_close", "gun.carrier_home"),
        (FireEvent::OpenBoltSlam, _) => ("guns/bolt_close", "gun.open_bolt"),
        // A Nagant's loading gate flicked open and shut, its cylinder
        // clicking round a chamber under the thumb.
        (FireEvent::BoltLocked, Some(Revolver)) => ("physical/switch", "gun.gate_open"),
        (FireEvent::BoltClosed, Some(Revolver)) => ("physical/switch", "gun.gate_shut"),
        (FireEvent::Cocked, Some(Revolver)) => ("guns/dry_fire", "gun.cylinder"),
        // A crossbow's string let go short of the latch: it slaps home.
        (FireEvent::BoltClosed, Some(Crossbow)) => ("physical/drop_wood", "gun.string_slap"),
        // A crossbow's string caught on the latch.
        (FireEvent::Cocked, Some(Crossbow)) => ("guns/dry_fire", "gun.latch"),
        // Brass sliding off the magazine's lips into the chamber, under
        // the bolt going home: an empty close has only the bolt.
        (FireEvent::Chambered, _) => ("guns/stripper_clip", "gun.chamber"),
        // The slide or bolt meeting its catch: a hard, short click.
        // Locked back on an empty magazine: the slide's half of a rack, short
        // and hard, heard over a fight (`scripts/audio_gunstate.py`). The
        // player who died on 28 October never heard it.
        (FireEvent::BoltLocked, _) => ("guns/slide_rack", "gun.bolt_catch"),
        // Drawn back by hand: the slide or carrier riding over the hammer.
        (FireEvent::Cocked, Some(SemiAuto)) => ("guns/bolt_open", "gun.slide_back"),
        // Pushed in short of its catch: a dull scrape, no snap.
        (FireEvent::MagazineHalfSeated, _) => ("guns/mag_insert", "gun.mag_half"),
        // Shaken out by the rack: it slithers out of the well.
        (FireEvent::MagazineDropped, _) => ("guns/mag_release", "gun.mag_drop"),
        _ => return None,
    })
}

/// How fast [`handling_sound`]'s sample plays for `event`.
pub fn handling_speed(
    event: &FireEvent,
    action: Option<extraction_core::firearm::ActionKind>,
) -> f64 {
    use extraction_core::firearm::ActionKind::Revolver;
    match (event, action) {
        // A Nagant's: the cylinder's quick small ratchet (not a dry fire),
        // the gate flicked open higher than it's pushed shut.
        (FireEvent::Cocked, Some(Revolver)) => 1.6,
        (FireEvent::BoltLocked, Some(Revolver)) => 1.25,
        (FireEvent::BoltClosed, Some(Revolver)) => 0.85,
        (FireEvent::DryFire, Some(extraction_core::firearm::ActionKind::Crossbow)) => 0.75,
        _ => handling_speed_of(event),
    }
}

fn handling_speed_of(event: &FireEvent) -> f64 {
    match event {
        FireEvent::Chambered => 1.3,
        FireEvent::BoltLocked => 1.5,
        FireEvent::MagazineHalfSeated => 0.7,
        FireEvent::MagazineDropped => 0.85,
        // The safety thumbed on low and firm, off high and light.
        FireEvent::ModeChanged(extraction_core::firearm::FireMode::Safe) => 0.85,
        FireEvent::ModeChanged(_) => 1.15,
        _ => 1.0,
    }
}

fn gun_sounds(
    mut commands: Commands,
    mut events: MessageReader<GunEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    guns: Query<&crate::weapons::Gun>,
    suppressed: Query<(), With<crate::gadgets::Suppressed>>,
    reports: Query<&crate::weapons::armoury::Report>,
) {
    let ear = listener
        .iter()
        .next()
        .map(|g| g.translation())
        .unwrap_or_default();
    let mut fired = std::collections::HashSet::new();
    for ev in events.read() {
        if fault_sounds(&mut commands, &bank, &mut rng.0, &ev.event, ev.at) {
            continue;
        }
        // A round stripped into the chamber by the shot's own cycle is under
        // the shot: only one worked by hand is heard.
        if matches!(ev.event, FireEvent::Fired { .. }) {
            fired.insert(ev.gun);
        } else if ev.event == FireEvent::Chambered && fired.contains(&ev.gun) {
            continue;
        }
        let action = guns.get(ev.gun).map(|g| g.firearm.def.action).ok();
        let (set, cue, offset) = match (&ev.event, action) {
            (FireEvent::Fired { .. }, _) => {
                let far = ev.at.distance(ear) > 80.0;
                // (A .22's crack is as small as a suppressed rifle's.)
                let quiet =
                    suppressed.contains(ev.gun) || reports.get(ev.gun).is_ok_and(|r| r.0 < 0.6);
                if quiet && !far {
                    // A suppressor: a cough, the crack's highs and boom gone.
                    if let Some(sample) = bank.pick(shot_set(&ev.item), &mut rng.0) {
                        let cue = "gun.shot_suppressed";
                        commands.spawn((
                            SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue))),
                            PlaybackSettings::default().with_speed(0.94),
                            Transform::from_translation(ev.at),
                            sample_effects![
                                mix::spatial(cue),
                                FastLowpassNode::<2> {
                                    cutoff_hz: 2_200.0,
                                    ..default()
                                },
                                reverb_send(cue)
                            ],
                        ));
                    }
                    continue;
                }
                match (far, quiet) {
                    (true, true) => ("guns/distant_shot", "gun.distant", -14.0),
                    (true, false) => ("guns/distant_shot", "gun.distant", 0.0),
                    _ => (shot_set(&ev.item), "gun.shot", 0.0),
                }
            }
            _ => match handling_sound(&ev.event, action, &ev.item) {
                Some((set, cue)) => (set, cue, 0.0),
                None => continue,
            },
        };
        // Shots aren't pitch-shifted much: a gun has one voice.
        let pitch = if matches!(ev.event, FireEvent::Fired { .. }) {
            0.03
        } else {
            0.07
        };
        let speed = match ev.event {
            // Small and quick inside the receiver.
            FireEvent::CarrierHome => 1.35,
            FireEvent::OpenBoltSlam => 0.9,
            _ => handling_speed(&ev.event, action),
        };
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(ev.at),
            cue,
            offset,
            speed,
            pitch,
        );
    }
}

/// Robot guns have no GunEvent; their shots sound here.
fn shot_sounds(
    mut commands: Commands,
    mut shots: MessageReader<Shot>,
    robots: Query<(), With<Robot>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for s in shots.read() {
        if s.shooter.is_some_and(|e| robots.contains(e)) {
            // A tighter, higher crack than the player's guns.
            let speed = 1.25;
            if let Some(sample) = bank.pick("guns/pistol_shot", &mut rng.0) {
                commands.spawn((
                    SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db("robot.shot"))),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(s.origin),
                    Placed::new("robot.shot"),
                ));
            }
        }
    }
}

/// Speed (m/s) a round hits at full level; slower ones hit softer.
const IMPACT_FULL_SPEED: f32 = 300.0;

/// Slower than this (m/s) it's no round but a blow (a crowbar, a knife,
/// a stock: `Hit::blow`, which `Impact` doesn't carry): the fastest swing
/// is a few tens of m/s, the slowest spent fragment hundreds.
pub const BLOW_SPEED: f32 = 40.0;
/// A blow at this speed (m/s) lands at its cue's level; at
/// [`extraction_core::melee::BLUNT_MIN`], 12 dB under.
const BLOW_FULL: f32 = 10.0;

/// How far under its cue a blow at `speed` m/s lands, dB.
pub fn blow_db(speed: f32) -> f32 {
    let min = extraction_core::melee::BLUNT_MIN;
    -12.0 * (1.0 - ((speed - min) / (BLOW_FULL - min)).clamp(0.0, 1.0))
}

/// Rounds landing, as `weapons::impacts` classes them (so the sound and
/// the spray can't disagree): concrete, wood, steel, glass, earth, water,
/// a robot's part (armour rings, an eye shatters), a body. A round
/// leaving the far side of a thin wall is a second, quieter crack; one
/// glancing off whines.
fn hit_sounds(
    mut commands: Commands,
    mut impacts: MessageReader<crate::weapons::impacts::Impact>,
    mut ricochets: MessageReader<crate::weapons::ballistics::Ricochet>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::weapons::impacts::ImpactKind as K;
    for i in impacts.read() {
        let v = i.velocity.length();
        // A robot's part struck, round or blow, is heard by what the hit
        // did to it (`audio_machines::struck_sounds`, `RobotStruck`); only
        // a round's spall out of its far side is heard here.
        let robot = i.kind == K::Robot;
        if v < BLOW_SPEED && !robot {
            // A blow: what it struck, struck with weight (slower, duller,
            // `hit.blow`), as loud as the swing; steel struck hard rings
            // with a clank under it (a crowbar on a robot's panel).
            let (set, speed) = match i.kind {
                K::Masonry => ("impacts/concrete", 0.8),
                K::Wood => ("impacts/wood", 0.8),
                K::Metal | K::Robot => ("impacts/metal", 0.75),
                K::Glass => ("physical/glass_break", 1.2),
                K::Soil => ("impacts/dirt", 0.8),
                K::Water => ("footsteps/water", 1.2),
                K::Body => ("physical/body_hit", 0.9),
            };
            let db = blow_db(v);
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(i.point),
                "hit.blow",
                db,
                speed,
                0.08,
            );
            if matches!(i.kind, K::Metal | K::Robot) && v > 6.0 {
                play_at_speed(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "machines/clank",
                    Some(i.point),
                    "hit.blow_ring",
                    db,
                    1.1,
                    0.08,
                );
            }
            continue;
        }
        // Out of the far side (a door, a drone's shell, a plank).
        let exit = crate::weapons::impacts::is_exit(i);
        let trim = if exit { -6.0 } else { 0.0 };
        let (set, cue, speed) = match i.kind {
            K::Masonry => ("impacts/concrete", "hit.impact", 1.0),
            K::Wood => ("impacts/wood", "hit.impact", 1.0),
            K::Metal => ("impacts/metal", "hit.impact", 1.0),
            K::Glass => ("physical/glass_break", "hit.glass", 1.15),
            K::Soil => ("impacts/dirt", "hit.impact", 1.0),
            K::Water => ("footsteps/water", "hit.water", 1.6),
            K::Body => ("physical/body_hit", "hit.body", 1.0),
            K::Robot => match (i.part, i.hurt) {
                (Some(crate::robots::PartRole::Sensor), true) => {
                    ("physical/glass_break", "hit.glass", 1.3)
                }
                // A dead part just rings.
                (_, false) => ("impacts/metal", "hit.impact", 1.12),
                _ => ("impacts/metal", "hit.impact", 1.0),
            },
        };
        // Water takes a big round with more of a slap.
        let trim = if i.kind == K::Water {
            trim - 6.0 + 6.0 * (i.velocity.length() / 800.0).min(1.0)
        } else {
            // A slow round (a grenade's fragments, spent) hits softer: full
            // from a pistol's speed, down to -12 dB at a quarter of it.
            trim + 20.0
                * (i.velocity.length() / IMPACT_FULL_SPEED)
                    .clamp(0.25, 1.0)
                    .log10()
        };
        if !robot {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(i.point),
                cue,
                trim,
                if exit { speed * 0.9 } else { speed },
                0.08,
            );
        }
        // Out through steel, a plank or a wall, it throws spall out of
        // the far side: a spray of bits clattering just past the hole
        // (`hit.spall`).
        let spall = match i.kind {
            K::Metal | K::Robot => Some(("physical/drop_metal", 1.7)),
            K::Wood => Some(("impacts/wood", 1.4)),
            K::Masonry => Some(("impacts/concrete", 1.5)),
            _ => None,
        };
        if let Some((set, speed)) = spall.filter(|_| exit) {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(i.point + i.velocity.normalize_or_zero() * 0.4),
                "hit.spall",
                trim + 6.0,
                speed,
                0.12,
            );
        }
    }
    // A round glancing off at a graze (`ballistics::Ricochet`): the whine,
    // a little along the way it left.
    for r in ricochets.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "impacts/ricochet",
            Some(r.point + r.velocity.normalize_or_zero() * 1.5),
            "hit.ricochet",
            0.0,
            0.1,
        );
    }
}

/// What taking something up sounds like: the thing in the glove. A rifle's
/// receiver knocks, a tin clinks, a crate's slat thuds; cloth and food
/// rustle.
pub fn handling_set(material: crate::items::impacts::Material) -> &'static str {
    use crate::items::impacts::Material;
    match material {
        Material::Metal => "physical/handle_metal",
        Material::Wood => "physical/handle_wood",
        Material::Plastic => "physical/handle_plastic",
        Material::Soft => "physical/cloth",
    }
}

/// Weight in the hand, as gain and pitch: +3 dB and about 4% lower per
/// doubling over a kilo (a 50 g tin is a tick, a 4 kg rifle a clack).
pub fn weight_tone(mass: f32) -> (f32, f64) {
    let doublings = mass.max(0.03).log2();
    (
        (3.0 * doublings).clamp(-9.0, 5.0),
        (1.0 - 0.04 * doublings as f64).clamp(0.85, 1.2),
    )
}

#[allow(clippy::too_many_arguments)]
fn hand_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    poses: Query<&GlobalTransform>,
    sockets: Query<&crate::interact::Socket>,
    items: Query<&crate::items::Item>,
    bags: Query<&crate::interact::body::InBackpack>,
    packs: Query<&crate::interact::body::Backpack>,
    catalog: Res<crate::items::Catalog>,
) {
    use crate::interact::SocketKind;
    // An item's handling sound, gain and pitch; cloth for what isn't one
    // (the pack, a rope).
    let feel = |item: Entity| {
        items
            .get(item)
            .ok()
            .map_or(("physical/cloth", 0.0, 1.0), |i| {
                let def = catalog.get(i.kind());
                let (db, speed) = weight_tone(def.mass);
                (
                    handling_set(crate::items::impacts::material_of(def)),
                    db,
                    speed,
                )
            })
    };
    for ev in events.read() {
        let (item, set, cue, db, speed) = match ev {
            InteractEvent::Grabbed { item, .. } => {
                let (set, db, speed) = feel(*item);
                if items.contains(*item) {
                    (*item, set, "hand.take", db, speed)
                } else {
                    (*item, set, "hand.grab", 0.0, 1.0)
                }
            }
            // Letting go: the same thing, lighter, as the fingers open.
            InteractEvent::Released { item, .. } if items.contains(*item) => {
                let (set, db, speed) = feel(*item);
                (*item, set, "hand.release", db, speed * 1.05)
            }
            // Into the pack: its zip. Into a crate, a tray or the stash:
            // the thing set down in it.
            InteractEvent::Stowed { item } => {
                let into_crate = bags
                    .get(*item)
                    .ok()
                    .and_then(|b| packs.get(b.bag).ok())
                    .is_some_and(|p| !p.carried);
                if into_crate {
                    let (set, db, speed) = feel(*item);
                    (*item, set, "hand.place", db, speed)
                } else {
                    (*item, "physical/zip", "hand.stow", 0.0, 1.0)
                }
            }
            // A pouch or holster takes it with a rustle (the long velcro rips
            // sounded like a motor in the first headset session); a gun's
            // magazine well has its own click (`gun.mag_insert`), and a rail
            // or cradle a small click.
            InteractEvent::Socketed { item, socket } => match sockets.get(*socket).map(|s| &s.kind)
            {
                Ok(SocketKind::Body(_)) => (*item, "physical/cloth", "hand.socket", 0.0, 1.0),
                Ok(SocketKind::MagWell(_)) => continue,
                _ => (*item, "guns/safety", "hand.attach", 0.0, 1.0),
            },
            InteractEvent::Refused { item } => (*item, "physical/cloth", "hand.refused", 0.0, 1.0),
            _ => continue,
        };
        let at = poses.get(item).map(|g| g.translation()).ok();
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            at,
            cue,
            db,
            speed,
            0.08,
        );
    }
}

/// Push buttons click going in and again, lighter, coming back out.
fn button_sounds(
    mut commands: Commands,
    buttons: Query<(Entity, &crate::interact::poke::PokeButton, &GlobalTransform)>,
    mut was: Local<HashMap<Entity, bool>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, b, gt) in &buttons {
        let before = was.insert(e, b.down).unwrap_or(false);
        if before == b.down {
            continue;
        }
        let (cue, speed) = if b.down {
            ("button.in", 1.0)
        } else {
            ("button.out", 1.12)
        };
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/button",
            Some(gt.translation()),
            cue,
            0.0,
            speed,
            0.04,
        );
    }
}

/// Things pressed with the grip (or the flat key): the bell cord rings
/// the truck's bell; a card, knob or lever clicks, unless its push button
/// already did. Doors sound in `door_sounds`.
fn press_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    pressables: Query<(&crate::raid::truck::Pressable, &GlobalTransform)>,
    buttons: Query<&crate::interact::poke::PokeButton>,
    locked: Query<(), With<crate::world::underground::PowerLocked>>,
    wedged: Query<(), With<crate::robots::doors::Wedged>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::raid::truck::Pressable;
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok((kind, gt)) = pressables.get(*item) else {
            continue;
        };
        let (set, cue) = match kind {
            Pressable::BellPull => ("physical/bell", "truck.bell"),
            // A door that won't open: a locked bolt rattling, a wedge
            // thudding. (One that opens sounds in `door_sounds`.)
            Pressable::Door if locked.contains(*item) => ("machines/clank", "door.locked"),
            Pressable::Door if wedged.contains(*item) => ("impacts/wood", "door.wedged"),
            Pressable::Door => continue,
            Pressable::FoldCover => ("physical/cloth", "hand.grab"),
            Pressable::BenchRun => ("physical/switch", "press.lever"),
            _ if buttons.iter().any(|b| b.target == *item && b.down) => continue,
            _ => ("physical/button", "button.in"),
        };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(gt.translation()),
            cue,
            0.0,
            0.05,
        );
    }
}

/// The far grab: a whoosh that flies with the thing, and a slap into the
/// palm when it's caught.
fn far_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    poses: Query<&GlobalTransform>,
    time: Res<Time>,
    mut flying: Local<HashMap<Entity, f32>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    let now = time.elapsed_secs();
    flying.retain(|_, t| now - *t < 2.0);
    for ev in events.read() {
        match ev {
            InteractEvent::Pulled { item, .. } => {
                flying.insert(*item, now);
                if let Some(sample) = bank.pick("physical/whoosh", &mut rng.0) {
                    let speed = 1.0 + (rng.0.f32() as f64 - 0.5) * 0.12;
                    commands.spawn((
                        SamplePlayer::new(sample)
                            .with_volume(Volume::Decibels(mix::db("far.whoosh"))),
                        PlaybackSettings::default().with_speed(speed),
                        Transform::default(),
                        Placed::new("far.whoosh"),
                        ChildOf(*item),
                    ));
                }
            }
            InteractEvent::Grabbed { item, .. } if flying.remove(item).is_some() => {
                let at = poses.get(*item).map(|g| g.translation()).ok();
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "physical/catch",
                    at,
                    "far.catch",
                    0.0,
                    0.06,
                );
            }
            _ => {}
        }
    }
}

/// The onboarding's "done, next": a light ding, in your head.
fn step_sounds(
    mut commands: Commands,
    mut steps: MessageReader<crate::rig::haptics::StepDone>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for s in steps.read() {
        let (set, cue) = if s.station {
            ("ui/chime_station", "step.station")
        } else {
            ("physical/chime", "step.done")
        };
        play(&mut commands, &bank, &mut rng.0, set, s.at, cue, 0.0, 0.02);
    }
}

/// Each drone carries a rotor drone; pitch follows thrust.
fn rotor_loops(
    mut commands: Commands,
    drones: Query<(Entity, &Drone, Option<&Children>), With<Robot>>,
    loops: Query<Entity, With<RotorLoop>>,
    mut settings: Query<&mut PlaybackSettings, With<RotorLoop>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, d, children) in &drones {
        let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
        let thrust: f32 = d.thrust.iter().sum::<f32>() / (4.0 * 15.0);
        match existing {
            None => {
                if let Some(sample) = bank.pick("machines/drone_rotor", &mut rng.0) {
                    commands.spawn((
                        RotorLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db("drone.rotor"))),
                        PlaybackSettings::default(),
                        Transform::default(),
                        Placed::new("drone.rotor"),
                        ChildOf(e),
                    ));
                }
            }
            Some(l) => {
                if let Ok(mut s) = settings.get_mut(l) {
                    s.speed = (0.85 + thrust.clamp(0.0, 1.5) as f64 * 0.3).max(0.2);
                }
            }
        }
    }
}

fn start_ambience(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    zone: Option<Res<Zone>>,
) {
    // Each valley has its own wind; fall back to the forest's.
    let wind = zone
        .as_ref()
        .map(|z| z.map.sounds.wind.as_str())
        .filter(|w| bank.has(w))
        .unwrap_or("ambience/wind_conifers");
    spawn_reverb_bus(&mut commands);
    if let Some(sample) = bank.pick("ambience/rain_light", &mut rng.0) {
        commands.spawn((
            RainLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![
                VolumeNode {
                    volume: Volume::SILENT,
                    ..default()
                },
                FastLowpassNode::<2> {
                    cutoff_hz: 20_480.0,
                    ..default()
                }
            ],
        ));
    }
    if let Some(sample) = bank.pick(wind, &mut rng.0) {
        commands.spawn((
            WindLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::Decibels(mix::db("wind")),
                ..default()
            }],
        ));
    }
    // The night's beds (`night_ambience`): insects and leaves, silent
    // until dark.
    for (set, marker) in [
        ("ambience/insects_night", NightLoop::Insects),
        ("ambience/rustle_leaves", NightLoop::Rustle),
    ] {
        if let Some(sample) = bank.pick(set, &mut rng.0) {
            commands.spawn((
                marker,
                SamplePlayer::new(sample).looping(),
                PlaybackSettings::default(),
                sample_effects![VolumeNode {
                    volume: Volume::SILENT,
                    ..default()
                }],
            ));
        }
    }
    if let Some(sample) = bank.pick("ambience/room_tone", &mut rng.0) {
        commands.spawn((
            RoomLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::SILENT,
                ..default()
            }],
        ));
    }
    if let Some(sample) = bank.pick("anomaly/attention", &mut rng.0) {
        commands.spawn((
            AttentionLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::SILENT,
                ..default()
            }],
        ));
    }
    if let Some(sample) = bank.pick("anomaly/surge_drone", &mut rng.0) {
        commands.spawn((
            SurgeDrone,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::SILENT,
                ..default()
            }],
        ));
    }
    if let Some(sample) = bank.pick("ambience/creek", &mut rng.0) {
        commands.spawn((
            CreekLoop,
            SamplePlayer::new(sample)
                .looping()
                .with_volume(Volume::Decibels(mix::db("creek"))),
            PlaybackSettings::default(),
            Transform::default(),
            Placed::new("creek"),
        ));
    }
}

/// Birds, distant thunder, the valley's own far-off sounds: now and then,
/// somewhere out in the fog.
fn ambient_oneshots(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    zone: Res<Zone>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    mut next: Local<(f32, f32, f32, f32)>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let night = conditions
        .as_deref()
        .is_some_and(|c| c.hour == crate::world::weather::Hour::Night);
    let storm = conditions.as_deref().is_some_and(|c| c.storm);
    let dt = time.delta_secs();
    next.0 -= dt;
    next.1 -= dt;
    next.2 -= dt;
    next.3 -= dt;
    // Old works settling: now and then a clank of metal from the nearest
    // big iron building, if one's within earshot.
    if next.2 <= 0.0 {
        next.2 = rng.0.range(20.0, 60.0);
        const IRON: [&str; 6] = [
            "factory_hall",
            "chimney",
            "rail_shed",
            "bus_depot",
            "pump_station",
            "boxcar",
        ];
        let here = [ear.x, ear.z];
        let near = zone
            .map
            .placements
            .iter()
            .filter(|p| IRON.contains(&p.prefab.as_str()))
            .map(|p| (extraction_core::geom::dist(p.at, here), p.at))
            .filter(|(d, _)| *d < 150.0)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, at)) = near {
            let at = Vec3::new(at[0], zone.ground(at[0], at[1]) + 3.0, at[1]);
            // Mostly a clank; sometimes a long groan of a beam taking the
            // weight.
            let (set, cue) = if rng.0.f32() < 0.35 {
                ("machines/groan", "works.groan")
            } else {
                ("machines/clank", "works.clank")
            };
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                cue,
                0.0,
                0.25,
            );
        }
    }
    if next.0 <= 0.0 {
        next.0 = rng.0.range(12.0, 45.0);
        let a = rng.0.range(0.0, std::f32::consts::TAU);
        let d = rng.0.range(25.0, 70.0);
        let at = ear + Vec3::new(a.cos() * d, rng.0.range(4.0, 15.0), a.sin() * d);
        // Birds keep to the day.
        let birds = &zone.map.sounds.birds;
        if !birds.is_empty() && !night {
            let set = &birds[rng.0.below(birds.len())];
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                "birds",
                0.0,
                0.08,
            );
        }
    }
    // The valley's own far-off sounds come from high ground: scree letting
    // go on the slopes above you.
    let distant = &zone.map.sounds.distant;
    if next.3 <= 0.0 && !distant.is_empty() {
        next.3 = rng.0.range(45.0, 140.0);
        let at = (0..6)
            .map(|_| {
                let a = rng.0.range(0.0, std::f32::consts::TAU);
                let d = rng.0.range(70.0, 160.0);
                let (x, z) = (ear.x + a.cos() * d, ear.z + a.sin() * d);
                Vec3::new(x, zone.ground(x, z) + 2.0, z)
            })
            .max_by(|a, b| a.y.total_cmp(&b.y));
        let set = &distant[rng.0.below(distant.len())];
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            at,
            "valley.distant",
            0.0,
            0.1,
        );
    }
    // Far thunder; at night, the Surge's, deeper and more often, a long
    // way off across the Zone.
    if next.1 <= 0.0 && storm {
        // In a storm, all thunder follows a flash (`storm_sounds`).
        next.1 = rng.0.range(60.0, 180.0);
    } else if next.1 <= 0.0 {
        if night {
            next.1 = rng.0.range(35.0, 90.0);
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                "ambience/thunder_distant",
                None,
                "thunder.surge",
                0.0,
                0.78,
                0.06,
            );
        } else {
            next.1 = rng.0.range(60.0, 180.0);
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "ambience/thunder_distant",
                None,
                "thunder",
                0.0,
                0.05,
            );
        }
    }
}

/// The creek sound sits at the nearest point of the water to the listener.
fn move_creek(
    mut creek: Query<&mut Transform, With<CreekLoop>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    zone: Res<Zone>,
    mut lines: Local<Option<Vec<Vec<[f32; 2]>>>>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let lines = lines.get_or_insert_with(|| {
        zone.map
            .water
            .iter()
            .map(|w| extraction_core::geom::catmull_rom(&w.points, 4.0))
            .collect()
    });
    let p = [ear.x, ear.z];
    let mut best: Option<(f32, [f32; 2])> = None;
    for line in lines.iter() {
        for q in line {
            let d = extraction_core::geom::dist(*q, p);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, *q));
            }
        }
    }
    if let (Some((_, q)), Ok(mut t)) = (best, creek.single_mut()) {
        t.translation = zone.on_ground(q[0], q[1]);
    }
}

/// The drone swells with the Surge; discharges crack overhead; the sirens
/// at the Gate wail when the warning starts.
fn surge_sounds(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    raid: Res<crate::raid::Raid>,
    mut flashes: MessageReader<crate::world::sky::SurgeFlash>,
    drones: Query<&SampleEffects, With<SurgeDrone>>,
    mut volumes: Query<&mut VolumeNode>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    zone: Res<Zone>,
    mut warned: Local<bool>,
) {
    let level = raid.surge_level();
    for effects in &drones {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            let db = if level < 0.01 {
                -80.0
            } else {
                mix::db("surge.drone") - 30.0 + 30.0 * level.sqrt()
            };
            v.volume = Volume::Decibels(db);
        }
    }
    let ear = listener.iter().next().map(|g| g.translation());
    for f in flashes.read() {
        let at = ear.map(|e| e + f.dir * 120.0);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "anomaly/surge_crack",
            at,
            "surge.crack",
            -6.0 + 6.0 * f.strength,
            0.12,
        );
    }
    let warning = raid.surge_in() <= crate::raid::SURGE_WARNING && raid.surge_in() > 0.0;
    if warning && !*warned && raid.phase != crate::raid::Phase::AtBase {
        let gate = zone
            .marker(&extraction_core::map::MarkerKind::RobotGate)
            .map(|m| zone.on_ground(m.at[0], m.at[1]) + Vec3::Y * 12.0);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/siren",
            gate,
            "surge.siren",
            0.0,
            0.0,
        );
    }
    *warned = warning;
}

/// A spatial loop on a nearby anomaly.
#[derive(Component)]
struct AnomalyLoop;

/// Where an anomaly's loop is kept, and where it fades: full inside
/// `FADE_NEAR`, down to -40 dB at `KEEP` where it starts and stops, so
/// walking up to one never switches its hum on.
const ANOMALY_FADE_NEAR: f32 = 25.0;
const ANOMALY_KEEP: f32 = 40.0;

fn anomaly_fade(d: f32) -> f32 {
    let t = ((d - ANOMALY_FADE_NEAR) / (ANOMALY_KEEP - ANOMALY_FADE_NEAR)).clamp(0.0, 1.0);
    -40.0 * t * t
}

/// Anomalies hum, roar and buzz when you're near enough to hear them (loops
/// are only kept on the close ones).
fn anomaly_loops(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    anomalies: Query<(
        Entity,
        &crate::anomaly::Anomaly,
        &GlobalTransform,
        Option<&Children>,
    )>,
    loops: Query<(Entity, &SampleEffects), With<AnomalyLoop>>,
    mut volumes: Query<&mut VolumeNode>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    for (e, a, gt, children) in &anomalies {
        let d = gt.translation().distance(ear);
        let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
        match existing {
            None if d < ANOMALY_KEEP => {
                let (set, cue) = match a.kind {
                    extraction_core::anomaly::AnomalyKind::Lift => {
                        ("anomaly/lift_hum", "anomaly.lift")
                    }
                    extraction_core::anomaly::AnomalyKind::Burner => {
                        ("anomaly/burner_roar", "anomaly.burner")
                    }
                    extraction_core::anomaly::AnomalyKind::Snare => {
                        ("anomaly/snare_buzz", "anomaly.snare")
                    }
                    extraction_core::anomaly::AnomalyKind::Well => {
                        ("anomaly/well_drone", "anomaly.well")
                    }
                    extraction_core::anomaly::AnomalyKind::Mirror => {
                        ("anomaly/mirror_shimmer", "anomaly.mirror")
                    }
                    // Dead air: nothing to hear (Anomalies).
                    extraction_core::anomaly::AnomalyKind::Hush => continue,
                    // A frost's thin crystal tinkle over the ice's creak
                    // (a stand-in, `synth_foley`): a tell at a few metres.
                    extraction_core::anomaly::AnomalyKind::Frost => {
                        ("anomaly/frost", "anomaly.frost")
                    }
                };
                if let Some(sample) = bank.pick(set, &mut rng.0) {
                    commands.entity(e).with_child((
                        AnomalyLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db(cue))),
                        PlaybackSettings::default().with_speed(0.9 + rng.0.f32() as f64 * 0.2),
                        Transform::from_xyz(0.0, 1.0, 0.0),
                        sample_effects![
                            VolumeNode {
                                volume: Volume::Decibels(anomaly_fade(d)),
                                ..default()
                            },
                            mix::spatial(cue),
                            reverb_send(cue)
                        ],
                    ));
                }
            }
            Some(l) if d > ANOMALY_KEEP + 5.0 => {
                commands.entity(l).despawn();
            }
            Some(l) => {
                if let Ok((_, effects)) = loops.get(l)
                    && let Ok(mut v) = volumes.get_effect_mut(effects)
                {
                    v.volume = Volume::Decibels(anomaly_fade(d));
                }
            }
            None => {}
        }
    }
}

fn anomaly_sounds(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    mut events: MessageReader<crate::anomaly::AnomalyEvent>,
    mut glances: MessageReader<crate::health::Glanced>,
    mut emps: MessageReader<crate::gadgets::EmpBlast>,
    mut offerings: MessageReader<crate::anomaly::offering::OfferingTaken>,
    mut harpoons: MessageReader<crate::robots::grappler::Harpoon>,
    mut blinked: MessageReader<crate::anomaly::mirrors::Blinked>,
    mut arcs: MessageReader<crate::anomaly::ArcStrike>,
    positions: Query<&GlobalTransform>,
) {
    // Blinked: the air rushing in where it was, the pop where it is now.
    for b in blinked.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/whoosh",
            Some(b.from),
            "anomaly.blink_air",
            0.0,
            0.1,
        );
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "anomaly/mirror_blink",
            Some(b.to),
            "anomaly.blink",
            0.0,
            0.05,
        );
    }
    // A snare's arc to what it struck: a short, sharp crack there.
    for a in arcs.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "anomaly/surge_crack",
            Some(a.to),
            "anomaly.arc",
            0.0,
            2.2,
            0.1,
        );
    }
    for h in harpoons.read() {
        let at = positions.get(h.from).ok().map(|g| g.translation());
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/clank",
            at,
            "harpoon.fire",
            0.0,
            0.1,
        );
        if h.hit {
            let at = positions.get(h.to).ok().map(|g| g.translation());
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "impacts/metal",
                at,
                "harpoon.hit",
                0.0,
                0.1,
            );
        }
    }
    for o in offerings.read() {
        let speed = if o.boon == crate::anomaly::offering::Boon::Nothing {
            1.3
        } else {
            1.0
        };
        if let Some(sample) = bank.pick("anomaly/offering", &mut rng.0) {
            commands.spawn((
                SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db("offering"))),
                PlaybackSettings::default().with_speed(speed),
                Transform::from_translation(o.at),
                Placed::new("offering"),
            ));
        }
    }
    use crate::anomaly::AnomalyAct;
    for ev in events.read() {
        let (set, cue, speed) = match ev.what {
            AnomalyAct::Discharge => ("anomaly/surge_crack", "anomaly.discharge", 1.7),
            AnomalyAct::Pop => ("anomaly/lift_pop", "anomaly.pop", 1.0),
            AnomalyAct::Bloom => ("anomaly/surge_crack", "anomaly.bloom", 0.6),
            AnomalyAct::Flare => continue,
            // Each thing blinked sounds at both ends (`Blinked`, below).
            AnomalyAct::Blink => continue,
            AnomalyAct::Reveal => ("anomaly/reveal", "anomaly.reveal", 1.0),
            // A well crushing something: the thing's metal giving (recorded,
            // slowed), under the knot's deep whump.
            AnomalyAct::Crush => {
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "anomaly/lift_pop",
                    Some(ev.at),
                    "anomaly.pop",
                    -4.0,
                    0.05,
                );
                ("machines/groan", "anomaly.crush", 0.6)
            }
        };
        let Some(sample) = bank.pick(set, &mut rng.0) else {
            continue;
        };
        commands.spawn((
            SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue))),
            PlaybackSettings::default().with_speed(speed),
            Transform::from_translation(ev.at),
            Placed::new(cue),
        ));
    }
    for b in emps.read() {
        for (set, cue, speed) in [
            ("anomaly/surge_crack", "emp.crack", 0.7),
            ("anomaly/lift_pop", "emp.pop", 1.4),
        ] {
            if let Some(sample) = bank.pick(set, &mut rng.0) {
                commands.spawn((
                    SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue))),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(b.at),
                    Placed::new(cue),
                ));
            }
        }
    }
    for g in glances.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "impacts/ricochet",
            Some(g.at),
            "hit.glance",
            0.0,
            0.1,
        );
    }
}

/// A gadget's running sound: the camera drone's rotors, a zipline trolley.
#[derive(Component)]
struct GadgetLoop;

/// The camera drone whines while its rotors turn (a robot drone's rotor,
/// pitched up and quieter); a zipline hums under your hand, rising with
/// speed.
fn gadget_loops(
    mut commands: Commands,
    drones: Query<(
        Entity,
        &crate::gadgets::camera_drone::CameraDrone,
        Option<&Children>,
    )>,
    grips: Query<(
        Entity,
        &crate::gadgets::climbing::RopeGrip,
        Option<&Children>,
    )>,
    ropes: Query<&crate::gadgets::climbing::Rope>,
    loops: Query<Entity, With<GadgetLoop>>,
    mut settings: Query<&mut PlaybackSettings, With<GadgetLoop>>,
    all_children: Query<&Children>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::gadgets::camera_drone::Flight;
    let mut want: Vec<(Entity, &str, &str, f64)> = Vec::new();
    for (e, d, _) in &drones {
        if d.flight != Flight::Idle {
            want.push((e, "machines/drone_rotor", "camera_drone", 1.9));
        }
    }
    for (e, g, _) in &grips {
        if ropes.get(g.rope).is_ok_and(|r| r.zip) && g.speed > 0.3 {
            let pitch = 0.6 + (g.speed / crate::gadgets::climbing::ZIP_SPEED) as f64 * 0.9;
            want.push((e, "machines/hum", "zipline", pitch));
        }
    }
    // Stop the ones that aren't wanted any more.
    for l in &loops {
        let parent_wanted = want
            .iter()
            .any(|(e, ..)| all_children.get(*e).is_ok_and(|c| c.iter().any(|c| c == l)));
        if !parent_wanted {
            commands.entity(l).despawn();
        }
    }
    for (e, set, cue, speed) in want {
        let existing = all_children
            .get(e)
            .ok()
            .and_then(|c| c.iter().find(|c| loops.contains(*c)));
        match existing {
            Some(l) => {
                if let Ok(mut s) = settings.get_mut(l) {
                    if (s.speed - speed).abs() > 0.02 {
                        s.speed = speed;
                    }
                }
            }
            None => {
                if let Some(sample) = bank.pick(set, &mut rng.0) {
                    commands.spawn((
                        GadgetLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db(cue))),
                        PlaybackSettings { speed, ..default() },
                        Transform::default(),
                        Placed::new(cue),
                        ChildOf(e),
                    ));
                }
            }
        }
    }
}

/// Doors creak open and bang shut, whoever moves them; a walker battering
/// a wedged one is heard a long way off.
fn door_sounds(
    mut commands: Commands,
    doors: Query<(Entity, &crate::world::buildings::Door, &GlobalTransform)>,
    mut was_open: Local<std::collections::HashMap<Entity, bool>>,
    mut blows: MessageReader<crate::robots::doors::DoorBlow>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    // Broken doors come back as new entities after the Surge.
    was_open.retain(|e, _| doors.contains(*e));
    for (e, door, gt) in &doors {
        let before = was_open.insert(e, door.open);
        if before.is_some_and(|b| b != door.open) {
            let (set, cue) = if door.open {
                ("physical/door_open", "door.open")
            } else {
                ("physical/door_close", "door.close")
            };
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(gt.translation()),
                cue,
                0.0,
                0.08,
            );
        }
    }
    for b in blows.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "impacts/wood",
            Some(b.at),
            "door.blow",
            0.0,
            0.15,
        );
        if b.broke {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "physical/door_close",
                Some(b.at),
                "door.broken",
                0.0,
                0.2,
            );
        }
    }
}

/// The beds held down for a moment (the ending's `Found`): `left`
/// seconds of [`HUSH_DB`], easing in and out over a second and a half.
#[derive(Resource, Default)]
pub struct Hush {
    pub left: f32,
}

pub const HUSH_DB: f32 = 9.0;
pub const HUSH_SECONDS: f32 = 10.0;

impl Hush {
    pub fn db(&self) -> f32 {
        let edge = 1.5;
        let into = ((HUSH_SECONDS - self.left) / edge).clamp(0.0, 1.0);
        let out = (self.left / edge).clamp(0.0, 1.0);
        HUSH_DB * into.min(out)
    }
}

/// The ending's moments (`raid::epilogue::EpilogueBeat`). The crates
/// opening: a long, low lid creak at hers, then the others round the
/// stacks a beat apart, fainter. Him at her crate: nothing added, the
/// wind and the room held down a while so it sits in quiet.
fn epilogue_sounds(
    mut commands: Commands,
    mut beats: MessageReader<crate::raid::epilogue::EpilogueBeat>,
    mut hush: ResMut<Hush>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut pending: Local<Vec<(f32, Vec3, f32)>>,
) {
    use crate::raid::epilogue::BeatKind;
    let now = time.elapsed_secs();
    for b in beats.read() {
        match b.kind {
            BeatKind::CratesOpen => {
                pending.push((now, b.at, 0.0));
                for i in 0..5 {
                    let a = rng.0.f32() * std::f32::consts::TAU;
                    let r = 6.0 + rng.0.f32() * 16.0;
                    let at = b.at + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                    pending.push((now + 0.6 + i as f32 * 0.45 + rng.0.f32() * 0.3, at, -4.0));
                }
            }
            BeatKind::Found => hush.left = HUSH_SECONDS,
            BeatKind::Begins => {}
        }
    }
    let due: Vec<_> = pending.iter().filter(|p| p.0 <= now).copied().collect();
    pending.retain(|p| p.0 > now);
    for (_, at, db) in due {
        let speed = 0.5 + rng.0.f32() as f64 * 0.1;
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/door_creak",
            Some(at + Vec3::Y),
            "epilogue.creak",
            db,
            speed,
            0.0,
        );
    }
}

/// The night's own beds.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum NightLoop {
    Insects,
    Rustle,
}

/// How dark it is for the night's sounds: 1 at night, half at dusk.
pub fn nightness(hour: crate::world::weather::Hour) -> f32 {
    use crate::world::weather::Hour;
    match hour {
        Hour::Night => 1.0,
        Hour::Dusk => 0.5,
        Hour::Dawn | Hour::Day => 0.0,
    }
}

/// The night: crickets everywhere outside, at the level Wildlife's chorus
/// says (`wildlife::insects::Chorus`: silent near a machine or after a
/// shot, a flush or a bolt, back over a minute, thinned by rain and wind;
/// so they stopping tells you something's there), and wind in the leaves
/// where it's overgrown. Quieter through walls, gone underground.
fn night_ambience(
    conditions: Option<Res<crate::world::weather::Conditions>>,
    chorus: Option<Res<crate::wildlife::insects::Chorus>>,
    listening: Res<Listening>,
    zone: Option<Res<Zone>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    loops: Query<(&NightLoop, &SampleEffects)>,
    mut volumes: Query<&mut VolumeNode>,
) {
    use crate::acoustics::SpaceKind;
    use crate::wildlife::insects::Singers;
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let night = conditions.as_deref().map_or(0.0, |c| nightness(c.hour));
    let crickets = chorus.as_deref().map_or(0.0, |c| {
        if c.kind == Singers::Crickets {
            c.level
        } else {
            0.0
        }
    });
    let under = listening.space.kind == SpaceKind::Underground;
    let walls = listening.space.enclosed;
    let cover = overgrown(zone.as_deref(), ear);
    for (which, effects) in &loops {
        let (level, cue) = match which {
            NightLoop::Insects => (crickets, "insects"),
            NightLoop::Rustle => (night * (0.35 + 0.65 * cover), "rustle"),
        };
        let level = if under { 0.0 } else { level };
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = if level < 0.01 {
                Volume::SILENT
            } else {
                Volume::Decibels(mix::db(cue) + 20.0 * level.log10() - 12.0 * walls)
            };
        }
    }
}

/// How overgrown it is round `at`, 0..1: the ivy and weeds on the
/// buildings within [`OVERGROWN_NEAR`] (`world::overgrowth::amount`,
/// nearer counting more; underground sites grow nothing), or the forest's
/// cover where there are none.
pub const OVERGROWN_NEAR: f32 = 40.0;

fn overgrown(zone: Option<&Zone>, at: Vec3) -> f32 {
    let Some(zone) = zone else {
        return 0.5;
    };
    let here = Vec2::new(at.x, at.z);
    let (mut sum, mut weight) = (0.0, 0.0);
    for p in &zone.map.placements {
        let d = Vec2::from(p.at).distance(here);
        if d > OVERGROWN_NEAR || extraction_core::underground::is_site(&p.prefab) {
            continue;
        }
        let w = 1.0 - d / OVERGROWN_NEAR;
        sum += w * crate::world::overgrowth::amount(Vec2::from(p.at), &p.prefab);
        weight += w;
    }
    let cover = zone.forest_cover(at.x, at.z);
    if weight > 0.0 {
        (sum / weight).max(cover)
    } else {
        cover
    }
}

/// Glass going and wood giving way (`destruction`), each cue levelled at
/// its biggest so the audit sees the worst (the code only takes it down):
///
/// - a step on broken glass: a short bright crunch (a pane's break, quick);
/// - a pane: a shop window crashes lower and longer than a vent pane
///   tinkles, quieter the smaller;
/// - wood: a thump and splinters; a door coming down thuds as well;
/// - a wall breached (the only break of 3 m² and over): masonry going,
///   rubble pouring and the boom of it.
///
/// Robots hear each to `destruction::noise_radius`; the audit checks your
/// ears hear about as far.
fn break_sounds(
    mut commands: Commands,
    mut broke: MessageReader<crate::destruction::Broke>,
    mut crunches: MessageReader<crate::destruction::glass::Crunch>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for c in crunches.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/glass_break",
            Some(c.at),
            "break.crunch",
            0.0,
            1.6,
            0.15,
        );
    }
    for b in broke.read() {
        let at = Some(b.at);
        match b.stuff {
            crate::destruction::Stuff::Glass => {
                let speed = (1.25 - b.size * 0.6).clamp(0.8, 1.25) as f64;
                let db = (b.size * 6.0 - 8.0).clamp(-9.0, 0.0);
                play_at_speed(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "physical/glass_break",
                    at,
                    "break.glass",
                    db,
                    speed,
                    0.1,
                );
            }
            crate::destruction::Stuff::Wood if b.size >= BREACH_SIZE => {
                for (set, cue, speed) in [
                    ("impacts/concrete", "break.breach", 0.7),
                    ("ambience/rockfall", "break.rubble", 1.0),
                    ("physical/door_close", "break.wood_big", 0.6),
                ] {
                    play_at_speed(
                        &mut commands,
                        &bank,
                        &mut rng.0,
                        set,
                        at,
                        cue,
                        0.0,
                        speed,
                        0.08,
                    );
                }
            }
            crate::destruction::Stuff::Wood => {
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "impacts/wood",
                    at,
                    "break.wood",
                    0.0,
                    0.15,
                );
                play_at_speed(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "impacts/wood",
                    at,
                    "break.splinter",
                    0.0,
                    1.6,
                    0.15,
                );
                if b.size > 1.0 {
                    play(
                        &mut commands,
                        &bank,
                        &mut rng.0,
                        "physical/door_close",
                        at,
                        "break.wood_big",
                        0.0,
                        0.2,
                    );
                }
            }
        }
    }
}

/// A flock settling back on a roof (`wildlife::Alighted`, as the last
/// bird sits): the wings braking over the perch, a few birds' worth,
/// softer and slower than going up (`flock.land`), and one call. Every
/// flight is heard both ways (the playtest of 28 October wanted wings on
/// takeoff and landing).
fn flock_landing(
    mut commands: Commands,
    mut alighted: MessageReader<crate::wildlife::Alighted>,
    flocks: Query<&crate::wildlife::flock::Flock>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::wildlife::flock::Species;
    for a in alighted.read() {
        let n = (a.birds as f32 / 6.0).clamp(1.0, 3.0) as usize;
        for _ in 0..n {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                "wildlife/wings",
                Some(a.at + Vec3::Y * 1.0),
                "flock.land",
                0.0,
                0.85,
                0.12,
            );
        }
        let (calls, cue) = match flocks.get(a.flock).map(|f| f.species) {
            Ok(Species::Raven) => ("ambience/raven", "flock.call"),
            Ok(Species::Jackdaw) => ("ambience/jackdaw", "flock.call"),
            Ok(Species::Heron) => ("wildlife/heron", "flock.heron"),
            Ok(Species::Chough) => ("wildlife/chough", "flock.chough"),
            _ => ("ambience/crow", "flock.call"),
        };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            calls,
            Some(a.at + Vec3::Y * 1.0),
            cue,
            -6.0,
            0.1,
        );
    }
}

/// A flock going up (`wildlife::Flushed`): the clatter of wings and the
/// birds' own calls, rooks (as crows), ravens or jackdaws by the flock's
/// species. Alarmed: a burst of calls; restless: one, softer. (The wings
/// are a synthesised wingbeat, `synth_foley::wings`, until a recording of
/// wings can be fetched.)
fn flock_sounds(
    mut commands: Commands,
    mut flushed: MessageReader<crate::wildlife::Flushed>,
    flocks: Query<&crate::wildlife::flock::Flock>,
    rafts: Query<(), With<crate::wildlife::ducks::Raft>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::wildlife::FlushCause;
    use crate::wildlife::flock::Species;
    for f in flushed.read() {
        let (calls, cue) = match flocks.get(f.flock).map(|fl| fl.species) {
            Ok(Species::Raven) => ("ambience/raven", "flock.call"),
            Ok(Species::Jackdaw) => ("ambience/jackdaw", "flock.call"),
            Ok(Species::Heron) => ("wildlife/heron", "flock.heron"),
            Ok(Species::Chough) => ("wildlife/chough", "flock.chough"),
            _ => ("ambience/crow", "flock.call"),
        };
        let restless = f.cause == FlushCause::Restless;
        // A raft of mallards going up off the water: quacks, and the run-up
        // splashing across the surface, with the wings.
        if rafts.contains(f.flock) {
            for _ in 0..(2 + rng.0.below(2)) {
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "wildlife/duck",
                    Some(f.at + Vec3::Y * 0.5),
                    "flock.duck",
                    0.0,
                    0.1,
                );
            }
            for _ in 0..3 {
                play_at_speed(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "footsteps/water",
                    Some(f.at),
                    "flock.splash",
                    0.0,
                    1.5,
                    0.15,
                );
            }
        }
        let up = Some(f.at + Vec3::Y * 2.0);
        let wings = (f.birds as f32 / 5.0).clamp(1.0, 4.0) as usize;
        for _ in 0..wings {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                "wildlife/wings",
                up,
                "flock.wings",
                if restless { -6.0 } else { 0.0 },
                1.0,
                0.12,
            );
        }
        let n = if f.alarm {
            3
        } else if restless {
            1
        } else {
            2
        };
        for _ in 0..n {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                calls,
                Some(f.at + Vec3::Y * 4.0),
                cue,
                if restless { -6.0 } else { 0.0 },
                0.1,
            );
        }
    }
}

/// Loud sounds slapping back off Sallow's dam (`acoustics::slapback`):
/// a shot, a grenade, the heavy's footfall or blow, a wall breached. Each
/// face both the sound and you stand in front of sends back its image, a
/// darkened copy (through 2.5 kHz, 4 dB down for what the stone takes)
/// from the sound's mirror image behind the face, after the extra path:
/// the dam's own voice, half a second after a shot from 90 m out.
fn slapbacks(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    mut walls: Local<Option<Vec<crate::acoustics::Reflector>>>,
    mut guns: MessageReader<GunEvent>,
    mut grenades: MessageReader<crate::weapons::grenades::GrenadeEvent>,
    mut heavies: MessageReader<crate::robots::heavy::HeavyEvent>,
    mut broke: MessageReader<crate::destruction::Broke>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut pending: Local<Vec<(f32, &'static str, Vec3, f64)>>,
) {
    use crate::robots::heavy::HeavyAct;
    let walls = walls.get_or_insert_with(|| {
        let Some(zone) = zone.as_deref() else {
            return Vec::new();
        };
        let placed: Vec<_> = zone
            .map
            .placements
            .iter()
            .map(|p| {
                (
                    p.prefab.clone(),
                    crate::world::buildings::placement_transform(zone, p),
                )
            })
            .collect();
        crate::acoustics::reflectors(&placed)
    });
    let mut loud: Vec<(&'static str, Vec3, f64)> = Vec::new();
    for g in guns.read() {
        if matches!(g.event, FireEvent::Fired { .. }) {
            loud.push((shot_set(&g.item), g.at, 1.0));
        }
    }
    for g in grenades.read() {
        if g.event == extraction_core::grenade::Event::Exploded {
            loud.push(("explosions/grenade_heavy", g.at, 1.0));
        }
    }
    for h in heavies.read() {
        if matches!(h.what, HeavyAct::Step | HeavyAct::Slam { .. }) {
            loud.push(("explosions/grenade_heavy", h.at, 0.6));
        }
    }
    for b in broke.read() {
        if b.size >= BREACH_SIZE {
            loud.push(("impacts/concrete", b.at, 0.7));
        }
    }
    let now = time.elapsed_secs();
    if let Some(ear) = listener.iter().next().map(|g| g.translation()) {
        for (set, at, speed) in loud {
            for w in walls.iter() {
                if let Some((image, delay)) = crate::acoustics::slapback(w, at, ear) {
                    pending.push((now + delay, set, image, speed));
                }
            }
        }
    }
    let due: Vec<_> = pending.iter().filter(|p| p.0 <= now).copied().collect();
    pending.retain(|p| p.0 > now);
    for (_, set, image, speed) in due {
        let Some(sample) = bank.pick(set, &mut rng.0) else {
            continue;
        };
        let cue = "echo.slap";
        commands.spawn((
            SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue))),
            PlaybackSettings::default().with_speed(speed),
            Transform::from_translation(image),
            // It comes from behind the dam on purpose: never occluded.
            Occluded {
                next: f32::INFINITY,
            },
            sample_effects![
                mix::spatial(cue),
                FastLowpassNode::<2> {
                    cutoff_hz: 2_500.0,
                    ..default()
                },
                reverb_send(cue)
            ],
        ));
    }
}

/// The detector's stone ping and an artifact's crackle, synthesised
/// (`robot_voice::render_ping`, `render_crackle`).
const PING_SET: &str = "device/stone_ping";
pub(crate) const CRACKLE_SET: &str = "device/crackle";

/// The Zone's smaller voices (`anomaly`):
///
/// - the sweep detector's ping for a loose stone, faster and higher the
///   nearer it is (it fires 1.25 s apart at range, 0.15 s close);
/// - a wet aegis biting its carrier: a crackle at the chest; a shard's
///   late glint, a faint glass tick;
/// - a hot stone held: a sizzle at the hand for as long as it burns.
fn anomaly_voices(
    mut commands: Commands,
    mut pings: MessageReader<crate::anomaly::detector::StonePing>,
    mut bites: MessageReader<crate::anomaly::costs::ArtifactBite>,
    mut burns: MessageReader<crate::anomaly::harvest::Burned>,
    mut sizzle: Query<(Entity, &mut Transform, &SampleEffects), With<BurnLoop>>,
    mut volumes: Query<&mut VolumeNode>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut last_burn: Local<f32>,
) {
    use crate::anomaly::costs::Bite;
    for p in pings.read() {
        let speed = 1.0 + 0.3 * p.near.clamp(0.0, 1.0) as f64;
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            PING_SET,
            Some(p.at),
            "detector.ping",
            0.0,
            speed,
            0.0,
        );
    }
    for b in bites.read() {
        match b.what {
            Bite::Shock => play(
                &mut commands,
                &bank,
                &mut rng.0,
                CRACKLE_SET,
                Some(b.at),
                "bite.shock",
                0.0,
                0.1,
            ),
            Bite::Glint => play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                "physical/chime",
                Some(b.at),
                "bite.glint",
                0.0,
                2.2,
                0.1,
            ),
        }
    }
    let now = time.elapsed_secs();
    let mut at = None;
    for b in burns.read() {
        at = Some(b.at);
    }
    if let Some(a) = at {
        *last_burn = now;
        match sizzle.iter_mut().next() {
            Some((_, mut t, effects)) => {
                t.translation = a;
                if let Ok(mut v) = volumes.get_effect_mut(effects) {
                    v.volume = Volume::Decibels(mix::db("burn.sizzle"));
                }
            }
            None => {
                if let Some(sample) = bank.pick("anomaly/burner_roar", &mut rng.0) {
                    commands.spawn((
                        BurnLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db("burn.sizzle"))),
                        PlaybackSettings::default().with_speed(1.9),
                        Transform::from_translation(a),
                        Placed::new("burn.sizzle"),
                    ));
                }
            }
        }
    } else if now - *last_burn > 0.2 {
        // Let go: it stops.
        for (e, ..) in &sizzle {
            commands.entity(e).despawn();
        }
    }
}

/// The hot stone's sizzle while it's held.
#[derive(Component)]
struct BurnLoop;

/// Hands' tools (`interact::tools`): the crowbar's strained creak as it
/// levers, higher the harder (once every 0.6 s while it's working), and
/// the crack as it gives; the cutters' snip and the ping of the cut.
fn tool_sounds(
    mut commands: Commands,
    mut pried: MessageReader<crate::interact::tools::crowbar::Pried>,
    mut cuts: MessageReader<crate::interact::tools::cutters::Cut>,
    places: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut next: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    // Forget a target once its creak is due again (and so anything gone).
    next.retain(|_, t| now < *t);
    let at = |e: Entity| places.get(e).ok().map(|g| g.translation());
    for p in pried.read() {
        if next.get(&p.target).is_some_and(|t| now < *t) {
            continue;
        }
        next.insert(p.target, now + 0.6);
        let strain = (p.torque / PRY_FULL).clamp(0.0, 1.0);
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/door_creak",
            at(p.target),
            "tool.pry",
            -6.0 + 6.0 * strain,
            0.5 + 0.4 * strain as f64,
            0.1,
        );
    }
    // A pried thing giving way is Destruction's `Broke`, heard with
    // the rest of the breaks.
    for c in cuts.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "guns/safety",
            at(c.target),
            "tool.cut",
            0.0,
            0.75,
            0.06,
        );
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "impacts/ricochet",
            at(c.target),
            "tool.cut_ping",
            0.0,
            1.8,
            0.08,
        );
    }
}

/// A crowbar's torque that sounds full strain, N·m.
const PRY_FULL: f32 = 400.0;

/// How the Hush (`anomaly::hush`) takes the world away: inside one, 18 dB
/// down and muffled to 350 Hz, your own steps and shots too; carrying a
/// hushstone (within its 1.2 m), a light 4 dB and 3 kHz, all the time.
pub const HUSH_DEEP: (f32, f32) = (18.0, 350.0);
pub const HUSH_LIGHT: (f32, f32) = (4.0, 3000.0);
/// After this long inside, a pressure tone rises (s), to full over 10:
/// a warning well before the Hush starts to hurt (its grace, 8 s).
pub const HUSH_TONE_AFTER: f32 = 4.0;

/// The Hush right now, eased: dB down and the cutoff every placed sound
/// is held under (`occlusion` applies it).
#[derive(Resource, Clone, Copy, Debug)]
pub struct HushNow {
    pub db: f32,
    pub cutoff: f32,
}

impl Default for HushNow {
    fn default() -> Self {
        Self {
            db: 0.0,
            cutoff: 20_480.0,
        }
    }
}

#[derive(Component)]
struct HushTone;

#[allow(clippy::too_many_arguments)]
fn hush_world(
    mut commands: Commands,
    hushes: Option<Res<crate::anomaly::hush::Hushes>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    inside: Query<&crate::anomaly::hush::InHush, With<crate::rig::LocalPlayer>>,
    mut main: Query<&mut VolumeNode, (With<MainBus>, Without<HushTone>)>,
    tone: Query<&SampleEffects, With<HushTone>>,
    mut volumes: Query<&mut VolumeNode, Without<MainBus>>,
    mut now: ResMut<HushNow>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let (db, cutoff) = match hushes.as_deref() {
        Some(h) if h.in_standing(ear) => HUSH_DEEP,
        Some(h) if h.eats(ear) => HUSH_LIGHT,
        _ => (0.0, 20_480.0),
    };
    // A second in or out: the air going dead, not a switch.
    let k = (time.delta_secs() * 3.0).min(1.0);
    now.db += (db - now.db) * k;
    now.cutoff = (now.cutoff.ln() + (cutoff.ln() - now.cutoff.ln()) * k).exp();
    if let Ok(mut v) = main.single_mut() {
        let want = Volume::Decibels(-now.db);
        if v.volume != want {
            v.volume = want;
        }
    }
    let secs = inside.iter().next().map_or(0.0, |h| h.0);
    let rise = ((secs - HUSH_TONE_AFTER) / 10.0).clamp(0.0, 1.0);
    match tone.iter().next() {
        None => {
            if let Some(sample) = bank.pick("anomaly/well_drone", &mut rng.0) {
                commands.spawn((
                    HushTone,
                    SamplePlayer::new(sample).looping(),
                    PlaybackSettings::default().with_speed(0.5),
                    sample_effects![VolumeNode {
                        volume: Volume::SILENT,
                        ..default()
                    }],
                ));
            }
        }
        Some(effects) => {
            if let Ok(mut v) = volumes.get_effect_mut(effects) {
                // Under the duck like everything, so lifted back by it.
                v.volume = if rise < 0.01 {
                    Volume::SILENT
                } else {
                    Volume::Decibels(mix::db("hush.tone") + now.db + 20.0 * rise.log10())
                };
            }
        }
    }
}

/// The animals (`wildlife`), some of them stand-ins (`synth_foley`):
///
/// - a herd startled: a roe deer's hoarse bark, a chamois' whistle, a dog
///   pack barking (two or three of them); bolting, hooves away through the
///   leaves; a dog killed, its yelp;
/// - rats in the walls: squeaks and claws;
/// - an animal killed (`bodies::Killed`): the thump of it going down, and
///   a heron's croak.
fn animal_sounds(
    mut commands: Commands,
    mut startled: MessageReader<crate::wildlife::herd::Startled>,
    mut scurried: MessageReader<crate::wildlife::rats::Scurried>,
    mut killed: MessageReader<crate::wildlife::bodies::Killed>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::wildlife::bodies::CarcassKind;
    use crate::wildlife::herd::{Kind, Startle};
    for s in startled.read() {
        let at = Some(s.at + Vec3::Y * 0.8);
        match (s.kind, s.what) {
            (Kind::Deer, Startle::Bark) => play(
                &mut commands,
                &bank,
                &mut rng.0,
                "wildlife/deer_bark",
                at,
                "animal.deer",
                0.0,
                0.05,
            ),
            (Kind::Chamois, Startle::Bark) => play(
                &mut commands,
                &bank,
                &mut rng.0,
                "wildlife/chamois",
                at,
                "animal.chamois",
                0.0,
                0.05,
            ),
            (Kind::Dog, Startle::Bark) => {
                for i in 0..(2 + rng.0.below(2)) {
                    let off = Vec3::new(i as f32 * 1.3, 0.0, (i as f32 - 1.0) * 1.1);
                    play(
                        &mut commands,
                        &bank,
                        &mut rng.0,
                        "wildlife/dog_bark",
                        Some(s.at + off + Vec3::Y * 0.5),
                        "animal.dog",
                        0.0,
                        0.08,
                    );
                }
            }
            (Kind::Dog, Startle::Killed) => play(
                &mut commands,
                &bank,
                &mut rng.0,
                "wildlife/dog_yelp",
                at,
                "animal.yelp",
                0.0,
                0.05,
            ),
            (_, Startle::Bolt) => {
                for _ in 0..3 {
                    play_at_speed(
                        &mut commands,
                        &bank,
                        &mut rng.0,
                        "footsteps/leaves",
                        at,
                        "animal.hooves",
                        0.0,
                        1.7,
                        0.1,
                    );
                }
            }
            _ => {}
        }
    }
    for r in scurried.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "wildlife/rats",
            Some(r.at),
            "animal.rats",
            0.0,
            0.08,
        );
    }
    for k in killed.read() {
        let speed = match k.kind {
            CarcassKind::Bird => 1.5,
            CarcassKind::Heron => 1.2,
            CarcassKind::Dog => 1.0,
            CarcassKind::Deer => 0.85,
        };
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/body_hit",
            Some(k.at),
            "animal.thump",
            0.0,
            speed,
            0.08,
        );
        if k.kind == CarcassKind::Heron {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "wildlife/heron",
                Some(k.at),
                "flock.heron",
                -4.0,
                0.05,
            );
        }
    }
}

/// Bats at dusk and night (`wildlife::bats`): the little of their calls a
/// person hears, a quiet high flicker from the nearest hunting bat within
/// [`BATS_NEAR`], now and then. A stand-in (`synth_foley`).
pub const BATS_NEAR: f32 = 15.0;

fn bat_sounds(
    mut commands: Commands,
    colonies: Query<&crate::wildlife::bats::Colony>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut next: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + rng.0.range(0.4, 1.2);
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let nearest = colonies
        .iter()
        .filter(|c| matches!(c.state, crate::wildlife::bats::ColonyState::Hunting))
        .flat_map(|c| c.bats.iter().map(|b| b.pos))
        .min_by(|a, b| a.distance(ear).total_cmp(&b.distance(ear)))
        .filter(|p| p.distance(ear) < BATS_NEAR);
    if let Some(at) = nearest {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "wildlife/bats",
            Some(at),
            "animal.bats",
            0.0,
            0.1,
        );
    }
}

/// Winded, you breathe hard (`interact::gestures`' breaths, which robots
/// hear to `gestures::BREATH` m, a third with a hand over your mouth):
/// a breath every [`BREATH_EVERY`] s, louder the more winded, quieter and
/// muffled through the hand. The ears hear it about as far as the robots
/// (the audit checks). A stand-in breath (`synth_foley`) for now.
pub const BREATH_EVERY: f32 = 1.0;

fn breath_sounds(
    mut commands: Commands,
    players: Query<
        (
            &crate::player::Body,
            &crate::interact::gestures::Gestures,
            &GlobalTransform,
        ),
        With<crate::rig::LocalPlayer>,
    >,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut next: Local<f32>,
) {
    let Ok((body, g, gt)) = players.single() else {
        return;
    };
    *next -= time.delta_secs();
    if *next > 0.0 {
        return;
    }
    *next = BREATH_EVERY;
    let winded = (1.0 - body.stamina / 0.6).clamp(0.0, 1.0);
    if winded <= 0.0 {
        return;
    }
    let at = gt.translation() + Vec3::Y * 1.6;
    let db = 20.0 * winded.max(0.05).log10();
    if g.hush {
        let hushed = 20.0 * crate::interact::gestures::HUSHED.log10();
        play_filtered_db(
            &mut commands,
            &bank,
            &mut rng.0,
            "body/breath",
            at,
            "body.breath",
            db + hushed,
            1.0,
            1000.0,
        );
    } else {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "body/breath",
            Some(at),
            "body.breath",
            db,
            0.05,
        );
    }
}

/// Water that isn't a river: any wide water (an arm, a lake, a flooded
/// street; [`LAP_WIDTH`] m and more) laps at the shore nearest you when
/// you're within [`LAP_NEAR`] of it; and where a map's water drips
/// ([`DRIPS`]: Sallow's scours, leaking into the foot of the draw-off
/// tower), drops fall when you're near. Stand-ins (`synth_foley`) for now.
pub const LAP_WIDTH: f32 = 20.0;
pub const LAP_NEAR: f32 = 40.0;
pub const DRIPS: &[(&str, [f32; 2], f32)] = &[("Sallow Reservoir", [-20.0, 102.5], 0.5)];
const DRIP_NEAR: f32 = 50.0;

fn water_sounds(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut next: Local<(f32, f32)>,
    mut shores: Local<Option<Vec<(f32, Vec<[f32; 2]>)>>>,
) {
    let (Some(zone), Some(ear)) = (zone, listener.iter().next().map(|g| g.translation())) else {
        return;
    };
    let now = time.elapsed_secs();
    let shores = shores.get_or_insert_with(|| {
        zone.map
            .water
            .iter()
            .filter(|w| w.width >= LAP_WIDTH)
            .map(|w| (w.width, extraction_core::geom::catmull_rom(&w.points, 4.0)))
            .collect()
    });
    let p = [ear.x, ear.z];
    if now >= next.0 {
        next.0 = now + rng.0.range(1.2, 2.8);
        // The nearest point of the nearest wide water's edge.
        let mut best: Option<(f32, [f32; 2])> = None;
        for (width, line) in shores.iter() {
            for q in line {
                let d = extraction_core::geom::dist(*q, p) - width * 0.5;
                if best.is_none_or(|b| d < b.0) {
                    let to =
                        Vec2::new(p[0] - q[0], p[1] - q[1]).normalize_or_zero() * (width * 0.5);
                    best = Some((d, [q[0] + to.x, q[1] + to.y]));
                }
            }
        }
        if let Some((d, q)) = best.filter(|b| b.0 < LAP_NEAR) {
            let _ = d;
            let at = zone.on_ground(q[0], q[1]);
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "ambience/lap",
                Some(at),
                "water.lap",
                0.0,
                0.1,
            );
        }
    }
    if now >= next.1 {
        next.1 = now + rng.0.range(0.15, 0.9);
        for &(map, [x, z], up) in DRIPS {
            if zone.map.name != map {
                continue;
            }
            let at = Vec3::new(x, zone.ground(x, z) + up, z);
            if at.distance(ear) < DRIP_NEAR {
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "ambience/drip",
                    Some(at),
                    "water.drip",
                    0.0,
                    0.1,
                );
            }
        }
    }
}

/// Rime Fell's derelict works in the wind (`world::fell_sounds`, which
/// says where; nothing there runs):
///
/// - the funicular's slack rope ticking and creaking on its rollers, at
///   the roller nearest you within [`ROPE_NEAR`] (`fell.rope`);
/// - the top station's bull wheel, the cut rope's end groaning on it
///   (`fell.wheel`);
/// - a loose sheet of the mill's corrugated roof banging once to three
///   times now and then, heard across the settlement (`fell.sheet`);
/// - the stopped car's doors creaking and banging (`fell.car`);
/// - wind moaning through each mill bay's broken windows and along the
///   avalanche galleries, a loop at each (`fell.wind`), and the
///   galleries' meltwater dripping (`water.drip`).
///
/// All of it is the wind's (`Conditions::wind`): in a storm or a
/// blizzard the sheet bangs and the doors swing nearly three times as
/// often as in a still fog, and the wind loops are at their cue's level,
/// 10 dB under it in the fog.
const ROPE_NEAR: f32 = 60.0;
const WHEEL_NEAR: f32 = 80.0;
const SHEET_NEAR: f32 = 160.0;
const CAR_NEAR: f32 = 60.0;
const GALLERY_DRIP_NEAR: f32 = 50.0;

/// A wind loop at one of Rime Fell's works.
#[derive(Component)]
struct FellWind;

#[allow(clippy::too_many_arguments)]
fn fell_sounds(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    winds: Query<(Entity, &SampleEffects), With<FellWind>>,
    mut volumes: Query<&mut VolumeNode>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut works: Local<Option<(String, Vec<crate::world::fell_sounds::Emitter>)>>,
    mut next: Local<[f32; 5]>,
    mut bangs: Local<Vec<(f32, &'static str, Vec3, f64)>>,
) {
    use crate::world::fell_sounds::{Source, emitters};
    let (Some(zone), Some(ear)) = (zone, listener.iter().next().map(|g| g.translation())) else {
        return;
    };
    // Worked out once a map, its wind loops with it.
    if works
        .as_ref()
        .is_none_or(|(name, _)| *name != zone.map.name)
    {
        for (e, _) in &winds {
            commands.entity(e).despawn();
        }
        let found = emitters(&zone.map, &zone.hm);
        for w in &found {
            if !matches!(w.source, Source::MillBay | Source::Gallery) {
                continue;
            }
            if let Some(sample) = bank.pick("ambience/wind_works", &mut rng.0) {
                let speed = rng.0.range(0.85, 1.1) as f64;
                let cue = "fell.wind";
                commands.spawn((
                    FellWind,
                    SamplePlayer::new(sample).looping(),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(w.at),
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
        *works = Some((zone.map.name.clone(), found));
    }
    let Some((_, works)) = works.as_ref() else {
        return;
    };
    if works.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let wind = conditions.map_or(0.5, |c| c.wind());
    let wind_db = mix::db("fell.wind") + 20.0 * (0.25 + 0.75 * wind).log10();
    for (_, effects) in &winds {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = Volume::Decibels(wind_db);
        }
    }
    // Gusty weather works the loose things more often.
    let calm = 1.5 - wind;
    let nearest = |s: Source| {
        works
            .iter()
            .filter(|w| w.source == s)
            .map(|w| w.at)
            .min_by(|a, b| a.distance(ear).total_cmp(&b.distance(ear)))
    };
    let mut due = |i: usize, lo: f32, hi: f32, rng: &mut AudioRng| {
        if now < next[i] {
            return false;
        }
        next[i] = now + rng.0.range(lo, hi) * calm;
        true
    };
    if due(0, 2.0, 6.0, &mut rng)
        && let Some(at) = nearest(Source::Rope).filter(|a| a.distance(ear) < ROPE_NEAR)
    {
        let speed = rng.0.range(0.5, 0.75) as f64;
        bangs.push((now, "fell.rope", at, speed));
    }
    if due(1, 6.0, 14.0, &mut rng)
        && let Some(at) = nearest(Source::BullWheel).filter(|a| a.distance(ear) < WHEEL_NEAR)
    {
        bangs.push((now, "fell.wheel", at, 0.8));
    }
    if due(2, 4.0, 15.0, &mut rng)
        && let Some(at) = nearest(Source::MillSheet).filter(|a| a.distance(ear) < SHEET_NEAR)
    {
        let n = 1 + (rng.0.f32() * 2.99) as usize;
        for i in 0..n {
            let t = now + i as f32 * rng.0.range(0.25, 0.5);
            bangs.push((t, "fell.sheet", at, 0.65));
        }
    }
    if due(3, 5.0, 12.0, &mut rng)
        && let Some(at) = nearest(Source::Car).filter(|a| a.distance(ear) < CAR_NEAR)
    {
        // A door swung on its hinge (the rope's creak, quicker), and as
        // often as not banging to.
        let speed = rng.0.range(0.6, 0.8) as f64;
        bangs.push((now, "fell.rope", at, speed));
        if rng.0.f32() < 0.6 {
            bangs.push((now + rng.0.range(0.6, 1.2), "fell.car", at, 0.9));
        }
    }
    if due(4, 0.3, 1.4, &mut rng)
        && let Some(at) = nearest(Source::Gallery).filter(|a| a.distance(ear) < GALLERY_DRIP_NEAR)
    {
        let off = Vec3::new(rng.0.range(-3.0, 3.0), 0.0, rng.0.range(-3.0, 3.0));
        bangs.push((now, "water.drip", at + off, 1.0));
    }
    bangs.retain(|&(t, cue, at, speed)| {
        if now < t {
            return true;
        }
        let set = mix::MIX
            .iter()
            .find(|c| c.name == cue)
            .map_or("", |c| c.set);
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
        false
    });
}

/// A sentry (`robots::sentry`): its warning chirp, the second before it
/// fires (the lock tone, quicker and higher: it must carry, so a
/// newcomer looking elsewhere hears it coming); its shots; the servo as a
/// decoy draws it; its power going (a whine winding down and a clank);
/// its head shot through.
fn sentry_sounds(
    mut commands: Commands,
    mut events: MessageReader<crate::robots::sentry::SentryEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::robots::sentry::SentryAct as A;
    for e in events.read() {
        let at = Some(e.at);
        let layers: &[(&str, &str, f64)] = match e.what {
            A::Chirp => &[
                ("machines/drone_lock", "sentry.chirp", 1.3),
                ("physical/switch", "lock.tick", 1.8),
            ],
            A::Fire => &[("guns/pistol_shot", "robot.shot", 0.95)],
            A::Drawn => &[("machines/servo", "sentry.turn", 1.2)],
            A::PowerLost => &[
                ("machines/servo", "sentry.turn", 0.45),
                ("machines/clank", "robot.part", 1.1),
            ],
            A::Wrecked => &[
                ("physical/glass_break", "robot.glass", 1.2),
                ("machines/groan", "robot.wreck", 1.3),
            ],
        };
        for &(set, cue, speed) in layers {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                at,
                cue,
                0.0,
                speed,
                0.05,
            );
        }
    }
}

/// A break this big (m²) is a wall breached (`destruction::breach`).
const BREACH_SIZE: f32 = 2.5;

/// Whether a space sounds like indoors (the room's hush, the wind cut):
/// walled round under a roof, not just roofed.
pub fn indoor_space(space: &crate::acoustics::Space) -> bool {
    use crate::acoustics::SpaceKind;
    matches!(space.kind, SpaceKind::Room | SpaceKind::Hall)
}

/// The wind in the trees, and the hush of a room: which you hear depends
/// on whether you're walled in under a roof.
#[derive(Component)]
struct WindLoop;
#[derive(Component)]
struct RoomLoop;

fn indoor_ambience(
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    wind: Query<&SampleEffects, With<WindLoop>>,
    room: Query<&SampleEffects, With<RoomLoop>>,
    mut volumes: Query<&mut VolumeNode>,
    listening: Res<Listening>,
    mut hush: ResMut<Hush>,
    time: Res<Time>,
    mut inside: Local<f32>,
    mut below: Local<f32>,
) {
    hush.left = (hush.left - time.delta_secs()).max(0.0);
    let hushed = hush.db();
    // In a building, and walled round (`acoustics::probe`): under a porch
    // roof or an open shed you're out of doors with a roof over you.
    let indoors = entrances.is_some_and(|e| {
        cams.iter()
            .next()
            .is_some_and(|c| e.building_at(c.translation()).is_some())
    }) && indoor_space(&listening.space);
    // Ease over about a second as you step through a door.
    let target = if indoors { 1.0 } else { 0.0 };
    *inside += (target - *inside).clamp(-time.delta_secs(), time.delta_secs());
    let k = *inside;
    // Down the stairs the wind dies away altogether, over a few seconds.
    let under = listening.space.kind == crate::acoustics::SpaceKind::Underground;
    let target = if under { 1.0 } else { 0.0 };
    *below += (target - *below).clamp(-time.delta_secs() * 0.4, time.delta_secs() * 0.4);
    for effects in &wind {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = if *below > 0.99 {
                Volume::SILENT
            } else {
                Volume::Decibels(mix::db("wind") - 14.0 * k - 40.0 * *below - hushed)
            };
        }
    }
    for effects in &room {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = if k < 0.01 {
                Volume::SILENT
            } else {
                Volume::Decibels(mix::db("room") - 12.0 + 12.0 * k - hushed)
            };
        }
    }
}

/// The score (`audio_score`): spawned silent, and steered by what the
/// Zone is doing. Off with the comfort setting `score`.
#[allow(clippy::too_many_arguments)]
fn score_drive(
    mut commands: Commands,
    players: Query<&crate::anomaly::Attention, With<crate::rig::LocalPlayer>>,
    agitation: Option<Res<crate::anomaly::surge::Agitation>>,
    director: Option<Res<crate::robots::director::Director>>,
    raid: Option<Res<crate::raid::Raid>>,
    trucks: Query<&GlobalTransform, With<crate::raid::truck::Truck>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut score: Query<&mut ScoreNode>,
    epilogue: Option<Res<crate::raid::epilogue::Epilogue>>,
    music: Option<Res<crate::radio::MusicNear>>,
    mut spoke: MessageReader<crate::robot_voice::Spoke>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut level: Local<f32>,
    mut duck: Local<(f32, f32)>,
) {
    use crate::raid::Phase;
    let Ok(mut node) = score.single_mut() else {
        commands.spawn(ScoreNode {
            seed: rng.0.next_u64(),
            ..default()
        });
        return;
    };
    let ear = listener.iter().next().map(|g| g.translation());
    // A robot speaking nearby: the score steps aside while it does.
    let now = time.elapsed_secs();
    for s in spoke.read() {
        if ear.is_some_and(|e| e.distance(s.at) < mix::SCORE_DUCK_NEAR) {
            let until = now + crate::robot_voice::duration(s.kind, s.said) + 0.3;
            duck.0 = duck.0.max(until);
        }
    }
    let want_duck = if now < duck.0 {
        mix::SCORE_DUCK_DB
    } else {
        0.0
    };
    let step = if want_duck > duck.1 { 40.0 } else { 10.0 } * time.delta_secs();
    duck.1 += (want_duck - duck.1).clamp(-step, step);
    let volume = Volume::Decibels(mix::SCORE_DB - duck.1);
    if node.volume != volume {
        node.volume = volume;
    }
    // The ending (`raid::epilogue`) is played without it.
    let ending = epilogue.is_some_and(|e| e.active());
    // A tape, a station, a piano or a gramophone near you (`radio`): one
    // musical layer at a time, so the score steps aside.
    let music = music.is_some_and(|m| m.0);
    let at_truck = ending
        || music
        || match raid.as_deref().map(|r| &r.phase) {
            None | Some(Phase::AtBase | Phase::Extracting { .. } | Phase::Down { .. }) => true,
            Some(Phase::TruckWaiting { .. }) => {
                ear.is_some_and(|e| trucks.iter().any(|t| t.translation().distance(e) < 12.0))
            }
            _ => false,
        };
    let response_near = match (director, ear) {
        (Some(d), Some(e)) => d
            .responses
            .iter()
            .any(|(_, at)| at.distance(e) < audio_score::RESPONSE_NEAR),
        _ => false,
    };
    let want = if comfort.is_some_and(|c| !c.score) {
        0.0
    } else {
        audio_score::target(
            players.iter().next().map_or(0.0, |a| a.total()),
            response_near,
            agitation.map_or(0.0, |a| a.0),
            at_truck,
        )
    };
    let rate = if want > *level {
        audio_score::RISE
    } else if at_truck {
        audio_score::FALL_AT_TRUCK
    } else {
        audio_score::FALL
    };
    let dt = time.delta_secs();
    *level += (want - *level).clamp(-rate * dt, rate * dt);
    if (node.intensity - *level).abs() > 1e-3 || (*level == 0.0 && node.intensity != 0.0) {
        node.intensity = *level;
    }
}

/// Attention is felt, not shown: past the first prickle a ringing rises in
/// your ears, loudest when the whole Zone is looking at you.
fn attention_hum(
    players: Query<&crate::anomaly::Attention, With<crate::rig::LocalPlayer>>,
    hum: Query<&SampleEffects, With<AttentionLoop>>,
    mut volumes: Query<&mut VolumeNode>,
    time: Res<Time>,
    mut level: Local<f32>,
) {
    let target = players
        .iter()
        .next()
        .map_or(0.0, |a| ((a.total() - 0.3) / 2.7).clamp(0.0, 1.0));
    // Swells over a few seconds, fades slower.
    let rate = if target > *level { 0.25 } else { 0.1 };
    *level += (target - *level).clamp(-rate * time.delta_secs(), rate * time.delta_secs());
    let k = *level;
    for effects in &hum {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = if k < 0.01 {
                Volume::SILENT
            } else {
                Volume::Decibels(mix::db("attention") - 20.0 + 20.0 * k)
            };
        }
    }
}

/// The Gate's horn, synthesised with the voices (`robot_voice::render_horn`).
const HORN_SET: &str = "voice/heavy/horn";
/// How far from its middle a heavy's feet land, m (it's a walker at twice
/// the size).
const HEAVY_FEET: f32 = 4.0;

/// The heavy machine (`robots::heavy`), heard before it's seen:
///
/// - each footfall a deep thud (a heavy explosion low-passed at 220 Hz and
///   slowed: mostly below 100 Hz, carrying ~150 m) with the clank of its
///   foot over it up close; Hands' footfall pulse is felt within 25 m
///   (`robots::heavy_look`);
/// - the arm rising: a hydraulic hiss and a slow servo, the 1.2 s tell
///   before a blow; the blow a low boom and a clank (and stone if it hit);
/// - a door torn off, a bolt shot out (a ping), something loose rattling,
///   a plate clanging down, a vent of steam, the stall's roar as it sinks,
///   the strain when a well holds it, a cell pulled, the fall;
/// - the Gate's horn when the director calls it out, heard across the
///   valley 20 s before it walks.
fn heavy_sounds(
    mut commands: Commands,
    mut events: MessageReader<crate::robots::heavy::HeavyEvent>,
    mut called: MessageReader<crate::robots::heavy_mind::HeavyCalled>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::robots::heavy::HeavyAct as A;
    for e in events.read() {
        let at = e.at;
        // (set, cue, speed, low-pass)
        let layers: &[(&str, &str, f64, Option<f32>)] = match e.what {
            A::Step => &[
                ("explosions/grenade_heavy", "heavy.step", 0.5, Some(220.0)),
                ("footsteps/metal", "heavy.foot", 0.55, None),
            ],
            A::Raise => &[
                ("machines/servo", "heavy.raise", 0.5, None),
                ("physical/whoosh", "heavy.hiss", 0.4, None),
            ],
            A::Slam { hit: true } => &[
                ("explosions/grenade_heavy", "heavy.slam", 0.7, Some(600.0)),
                ("machines/clank", "heavy.clank", 0.7, None),
                ("impacts/concrete", "heavy.clank", 0.6, None),
            ],
            A::Slam { hit: false } => &[
                ("explosions/grenade_heavy", "heavy.slam", 0.7, Some(600.0)),
                ("machines/clank", "heavy.clank", 0.7, None),
            ],
            A::Tore => &[
                ("physical/door_creak", "heavy.tear", 0.6, None),
                ("machines/groan", "heavy.strain", 0.9, None),
                ("physical/drop_wood", "heavy.clank", 0.6, None),
            ],
            A::Bolt => &[("impacts/ricochet", "heavy.bolt", 1.3, None)],
            A::Loose => &[("machines/clank", "heavy.loose", 1.2, None)],
            A::PlateOff => &[
                ("physical/drop_metal", "heavy.plate", 0.6, None),
                ("machines/clank", "heavy.clank", 0.6, None),
            ],
            A::VentOut => &[("physical/whoosh", "heavy.vent", 0.45, None)],
            A::Stall => &[
                ("anomaly/burner_roar", "heavy.stall", 0.8, Some(2500.0)),
                ("machines/groan", "heavy.strain", 0.6, None),
            ],
            A::Held => &[("machines/groan", "heavy.strain", 0.7, None)],
            A::CellPulled => &[
                ("machines/clank", "heavy.clank", 1.1, None),
                ("physical/switch", "heavy.cell", 0.8, None),
            ],
            A::Fell => &[
                ("explosions/grenade_heavy", "heavy.slam", 0.55, Some(400.0)),
                ("machines/groan", "heavy.strain", 0.5, None),
            ],
        };
        for &(set, cue, speed, cutoff) in layers {
            match cutoff {
                Some(hz) => {
                    play_filtered(&mut commands, &bank, &mut rng.0, set, at, cue, speed, hz)
                }
                None => play_at_speed(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    set,
                    Some(at),
                    cue,
                    0.0,
                    speed,
                    0.05,
                ),
            }
        }
    }
    for c in called.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            HORN_SET,
            Some(c.gate + Vec3::Y * 8.0),
            "heavy.horn",
            0.0,
            1.0,
            0.0,
        );
    }
}

/// A heavy's idle: a low hum and the tick of its systems, on it.
#[derive(Component)]
struct HeavyLoop;

fn heavy_loops(
    mut commands: Commands,
    heavies: Query<
        (Entity, &crate::robots::Robot, Option<&Children>),
        With<crate::robots::heavy::Heavy>,
    >,
    loops: Query<Entity, With<HeavyLoop>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, robot, children) in &heavies {
        let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
        match (existing, robot.wrecked) {
            (None, false) => {
                if let Some(sample) = bank.pick("machines/hum", &mut rng.0) {
                    commands.spawn((
                        HeavyLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db("heavy.idle"))),
                        PlaybackSettings::default().with_speed(0.55),
                        Transform::from_xyz(0.0, 2.0, 0.0),
                        Placed::new("heavy.idle"),
                        ChildOf(e),
                    ));
                }
            }
            (Some(l), true) => commands.entity(l).despawn(),
            _ => {}
        }
    }
}

/// A walker's foot coming down: a heavy metal footfall, and now and then
/// the whine of its servos.
fn walker_steps(
    mut commands: Commands,
    mut steps: MessageReader<crate::robots::walker::WalkerStep>,
    heavies: Query<&GlobalTransform, With<crate::robots::heavy::Heavy>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for s in steps.read() {
        // A heavy's feet are its own (`heavy_sounds`).
        if heavies
            .iter()
            .any(|h| h.translation().xz().distance(s.at.xz()) < HEAVY_FEET)
        {
            continue;
        }
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "footsteps/metal",
            Some(s.at),
            "walker.step",
            0.0,
            0.12,
        );
        if rng.0.f32() < 0.35 {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "machines/servo",
                Some(s.at),
                "walker.servo",
                0.0,
                0.15,
            );
        }
    }
}

/// Keying the radio: a squelch, a breath of static.
fn radio_sounds(
    mut commands: Commands,
    mut calls: MessageReader<crate::raid::RadioCall>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for c in calls.read() {
        let at = positions
            .get(c.player)
            .ok()
            .map(|g| g.translation() + Vec3::Y * 1.3);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_squelch",
            at,
            "radio.squelch",
            0.0,
            0.05,
        );
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_static",
            at,
            "radio.static",
            0.0,
            0.05,
        );
    }
}

/// Robot chatter on a carried radio: a squelch, a hiss of static, and
/// their talk (`machines/robot_voice`: synthesised machine syllables,
/// band-limited and crushed like a cheap radio) — louder when it's about
/// somewhere close.
fn chatter_sounds(
    mut commands: Commands,
    mut heard: MessageReader<crate::raid::chatter::RobotChatter>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for h in heard.read() {
        let at = positions.get(h.radio).ok().map(|g| g.translation());
        let gain = if h.close { 0.0 } else { -5.0 };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_squelch",
            at,
            "chatter.squelch",
            gain,
            0.05,
        );
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_static",
            at,
            "chatter.static",
            gain,
            0.1,
        );
        // Their talk: machine syllables through the radio's chain.
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/robot_voice",
            at,
            "chatter.voice",
            gain,
            0.04,
        );
    }
}

/// A drone settling its gun on you: the chirp that means move, with a
/// hard tick on its onset (`lock.tick`): a tone alone is too narrow for
/// the ears to place above from level (`scripts/audio_localise.py`).
fn lock_sounds(
    mut commands: Commands,
    mut locks: MessageReader<crate::robots::drone::DroneLock>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for l in locks.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/drone_lock",
            Some(l.at),
            "drone.lock",
            0.0,
            0.03,
        );
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/switch",
            Some(l.at),
            "lock.tick",
            0.0,
            1.8,
            0.03,
        );
    }
}

/// Going into deeper water: a splash, louder the harder you went in.
fn splash_sounds(
    mut commands: Commands,
    bodies: Query<(Entity, &crate::player::Body, &GlobalTransform)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    mut was: Local<std::collections::HashMap<Entity, f32>>,
) {
    for (e, body, gt) in &bodies {
        let before = was.insert(e, body.wading).unwrap_or(body.wading);
        if before < 0.25 && body.wading >= 0.25 {
            let hard = (body.velocity.length() / 4.0).clamp(0.2, 1.0);
            for _ in 0..2 {
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "footsteps/water",
                    Some(gt.translation()),
                    "splash",
                    -8.0 + 8.0 * hard,
                    0.15,
                );
            }
        }
    }
}

/// Above this (kg) a landing adds a low thud of the ground taking the weight.
const HEAVY_DROP: f32 = 8.0;

/// Things you drop land with a sound that depends on what they're made of
/// and what they land on, louder the harder they hit (`items::impacts`).
/// On a floor, a crate or a rock you hear the thing; in grass, leaves or
/// mud you hear the ground take it (a magazine in the grass clanged like a
/// pan in the first headset session), with a faint clink for metal dropped
/// hard.
fn drop_sounds(
    mut commands: Commands,
    mut impacts: MessageReader<crate::items::impacts::ItemImpact>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &Surfaces)>,
) {
    use crate::items::impacts::{Material, soft};
    for i in impacts.read() {
        let offset = (-18.0 + 4.0 * i.speed + 3.0 * i.mass.max(0.05).log2()).min(0.0);
        if i.mass > HEAVY_DROP {
            // A door or a wheel: the ground takes its weight, under whatever it is.
            let weight = (3.0 * (i.mass / HEAVY_DROP).log2()).min(6.0);
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                "impacts/dirt",
                Some(i.at),
                "drop.heavy",
                weight - 6.0 + (2.0 * i.speed).min(6.0),
                0.55,
                0.1,
            );
        }
        let on_built = underfoot(&spatial, &built, i.at).is_some();
        if !on_built && soft(i.ground) {
            let ground = surface_set(i.ground.unwrap_or(Surface::Grass));
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                ground,
                Some(i.at),
                "drop",
                offset - 4.0,
                0.1,
            );
            if i.material == Material::Metal && i.speed > 4.0 {
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "physical/drop_metal",
                    Some(i.at),
                    "drop",
                    offset - 14.0,
                    0.1,
                );
            }
            continue;
        }
        let set = match i.material {
            Material::Metal => "physical/drop_metal",
            Material::Wood => "physical/drop_wood",
            Material::Plastic => "physical/drop_plastic",
            Material::Soft => "physical/drop_cardboard",
        };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(i.at),
            "drop",
            offset,
            0.1,
        );
    }
}

/// The truck's diesel, heard where it is: coming up the road to the
/// rendezvous as it's due, idling while it waits for you, pulling away.
/// (The truck itself moves between stops off-stage.)
#[derive(Component)]
struct TruckIdle;

/// Seconds before the truck is due that its approach starts (about the
/// length of `machines/truck_arrive`).
const TRUCK_APPROACH: f32 = 6.0;

#[allow(clippy::too_many_arguments)]
fn truck_sounds(
    mut commands: Commands,
    raid: Res<crate::raid::Raid>,
    zone: Res<Zone>,
    trucks: Query<(Entity, &GlobalTransform), With<crate::raid::truck::Truck>>,
    idles: Query<Entity, With<TruckIdle>>,
    mut last: Local<Option<crate::raid::Phase>>,
    mut heard_coming: Local<bool>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::raid::Phase;
    let engine = |t: &Transform| t.transform_point(Vec3::new(0.0, 1.0, -2.0));
    let rv_engine = |rv: usize| engine(&crate::raid::truck::rv_pose(&zone, rv));
    let Some((truck, truck_at)) = trucks.iter().next() else {
        return;
    };
    let here = engine(&truck_at.compute_transform());
    let phase = raid.phase.clone();
    match &phase {
        Phase::TruckCalled { rv, eta } => {
            if *eta <= TRUCK_APPROACH && !*heard_coming && *rv != crate::raid::SHORT_EXFIL {
                *heard_coming = true;
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "machines/truck_arrive",
                    Some(rv_engine(*rv)),
                    "truck.arrive",
                    0.0,
                    0.02,
                );
            }
        }
        _ => *heard_coming = false,
    }
    let idling = matches!(phase, Phase::TruckWaiting { .. });
    match (idling, idles.iter().next()) {
        (true, None) => {
            if let Some(sample) = bank.pick("machines/truck_idle", &mut rng.0) {
                commands.spawn((
                    TruckIdle,
                    SamplePlayer::new(sample)
                        .looping()
                        .with_volume(Volume::Decibels(mix::db("truck.idle"))),
                    PlaybackSettings::default(),
                    Transform::from_xyz(0.0, 1.0, -2.0),
                    Placed::new("truck.idle"),
                    ChildOf(truck),
                ));
            }
        }
        (false, Some(e)) => commands.entity(e).despawn(),
        _ => {}
    }
    // Leaving: with you in the back, or without you.
    let left_from = match (last.as_ref(), &phase) {
        (Some(Phase::TruckWaiting { .. }), Phase::Extracting { .. }) => Some(here),
        (Some(Phase::TruckWaiting { rv, .. }), Phase::Out) if *rv != crate::raid::SHORT_EXFIL => {
            Some(rv_engine(*rv))
        }
        _ => None,
    };
    if let Some(at) = left_from {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/truck_depart",
            Some(at),
            "truck.depart",
            0.0,
            0.02,
        );
    }
    *last = Some(phase);
}

/// What using a thing sounds like, by what it is: eating, drinking,
/// tearing a dressing open, a switch.
pub fn use_sound(def: &extraction_core::items::ItemDef) -> Option<(&'static str, &'static str)> {
    use extraction_core::items::Category;
    const DRINKS: &[&str] = &[
        "food:Water",
        "food:Milk",
        "food:Tea",
        "food:MilkTea",
        "food:HotWater",
        "food:Soup",
    ];
    match def.id.as_str() {
        "field_dressing" => Some(("physical/bandage", "use.bandage")),
        "radio" | "deployable_cover" | "climbing_kit" | "zipline_kit" | "camera_drone" => None,
        _ if def.category == Category::Food => Some(if DRINKS.iter().any(|t| def.has_tag(t)) {
            ("physical/drink", "use.drink")
        } else {
            ("physical/eat", "use.eat")
        }),
        // Lights, an injector's plunger, a cloak, arming a charge.
        _ if matches!(
            def.category,
            Category::Gadget | Category::Medical | Category::Firearm | Category::Attachment
        ) || def.id.ends_with("light") =>
        {
            Some(("physical/switch", "use.switch"))
        }
        _ => None,
    }
}

/// Using what's in your hand. The kinds of what hands held last frame are
/// kept, because a thing eaten is gone before its message is read.
fn use_sounds(
    mut commands: Commands,
    mut uses: MessageReader<crate::items::usage::UseItem>,
    held: Query<(Entity, &crate::items::Item, &GlobalTransform), With<crate::interact::Held>>,
    catalog: Res<crate::items::Catalog>,
    mut kinds: Local<HashMap<Entity, (extraction_core::items::ItemKind, Vec3)>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, item, gt) in &held {
        kinds.insert(e, (item.kind(), gt.translation()));
    }
    for u in uses.read() {
        let Some((kind, at)) = kinds.get(&u.item).copied() else {
            continue;
        };
        if let Some((set, cue)) = use_sound(catalog.get(kind)) {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                cue,
                0.0,
                0.08,
            );
        }
    }
    kinds.retain(|e, _| held.contains(*e));
}

/// A robot losing a part: the crash of it, glass for an eye, and a groan
/// of the frame giving up when it was the core.
fn part_sounds(
    mut commands: Commands,
    mut parts: MessageReader<crate::robots::PartDestroyed>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::robots::PartRole;
    for p in parts.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/clank",
            Some(p.at),
            "robot.part",
            0.0,
            0.15,
        );
        let extra = match p.role {
            PartRole::Sensor => Some(("physical/glass_break", "robot.glass")),
            PartRole::Core => Some(("machines/groan", "robot.wreck")),
            _ => None,
        };
        if let Some((set, cue)) = extra {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(p.at),
                cue,
                0.0,
                0.1,
            );
        }
    }
}

/// Your pack opening and closing: its zip.
fn pack_sounds(
    mut commands: Commands,
    packs: Query<(Entity, &crate::interact::body::Backpack, &GlobalTransform)>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    mut was: Local<HashMap<Entity, bool>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, p, gt) in &packs {
        if !p.carried || !locals.contains(p.owner) {
            continue;
        }
        let before = was.insert(e, p.open);
        if before.is_none_or(|b| b == p.open) {
            continue;
        }
        let (cue, speed) = if p.open {
            ("pack.open", 1.0)
        } else {
            ("pack.close", 1.1)
        };
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/zip",
            Some(gt.translation()),
            cue,
            0.0,
            speed,
            0.06,
        );
    }
}

/// The grapnel thrown, biting, coiled back up; a cover deployed and
/// folded; the bench finishing a job.
#[allow(clippy::too_many_arguments)]
fn gadget_sounds(
    mut commands: Commands,
    grapnels: Query<(Entity, &crate::gadgets::climbing::Grapnel, &GlobalTransform)>,
    covers: Query<(Entity, &GlobalTransform), With<crate::gadgets::CoverWall>>,
    benches: Query<(Entity, &crate::raid::bench::Bench, &GlobalTransform)>,
    mut grapnel_was: Local<HashMap<Entity, (u8, bool)>>,
    mut cover_at: Local<HashMap<Entity, Vec3>>,
    mut bench_was: Local<HashMap<Entity, bool>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::gadgets::climbing::Grapnel;
    // (Only what still exists: every grapnel the Surge ever rolled stayed.)
    grapnel_was.retain(|e, _| grapnels.contains(*e));
    let mut out: Vec<(&str, &str, Vec3, f64)> = Vec::new();
    for (e, g, gt) in &grapnels {
        let now = match g {
            Grapnel::Coiled => (0, false),
            Grapnel::Ready { thrown, .. } => (1, *thrown),
            Grapnel::Hooked { .. } => (2, false),
        };
        let at = gt.translation();
        match (grapnel_was.insert(e, now), now) {
            (Some((1, false)), (1, true)) => {
                out.push(("physical/whoosh", "grapnel.throw", at, 0.8))
            }
            (Some((1, _)), (2, _)) => out.push(("impacts/metal", "grapnel.hook", at, 1.2)),
            (Some((a, _)), (0, _)) if a != 0 => {
                out.push(("physical/cloth", "grapnel.coil", at, 1.0))
            }
            _ => {}
        }
    }
    let mut seen = Vec::new();
    for (e, gt) in &covers {
        seen.push(e);
        if cover_at.insert(e, gt.translation()).is_none() {
            out.push(("physical/drop_metal", "cover.deploy", gt.translation(), 0.8));
        }
    }
    cover_at.retain(|e, at| {
        let kept = seen.contains(e);
        if !kept {
            out.push(("physical/drop_metal", "cover.fold", *at, 1.1));
        }
        kept
    });
    for (e, b, gt) in &benches {
        let running = b.running.is_some();
        if bench_was.insert(e, running) == Some(true) && !running {
            out.push((
                "physical/bell",
                "bench.done",
                gt.translation() + Vec3::Y,
                1.6,
            ));
        }
    }
    for (set, cue, at, speed) in out {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            cue,
            0.0,
            speed,
            0.06,
        );
    }
}

/// The collector's convoy: the clank of a load taken on, the groan of the
/// walker setting off through the Gate.
fn convoy_sounds(
    mut commands: Commands,
    mut events: MessageReader<crate::robots::collector::ConvoyEvent>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::robots::collector::ConvoyEvent;
    for ev in events.read() {
        let (robot, set, cue) = match ev {
            ConvoyEvent::Loaded { robot } => (*robot, "machines/clank", "convoy.loaded"),
            ConvoyEvent::Delivered { robot } => (*robot, "machines/groan", "convoy.delivered"),
        };
        let at = positions
            .get(robot)
            .ok()
            .map(|g| g.translation() + Vec3::Y * 2.0);
        play(&mut commands, &bank, &mut rng.0, set, at, cue, 0.0, 0.15);
    }
}

/// A dirigible's engines: a slow, low throb overhead, quickening as it
/// moves, gone when it's wrecked.
#[derive(Component)]
struct DirigibleLoop;

fn dirigible_loops(
    mut commands: Commands,
    ships: Query<(
        Entity,
        &crate::robots::dirigible::Dirigible,
        &Robot,
        Option<&Children>,
    )>,
    loops: Query<Entity, With<DirigibleLoop>>,
    mut settings: Query<&mut PlaybackSettings, With<DirigibleLoop>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, d, robot, children) in &ships {
        let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
        match (existing, robot.wrecked) {
            (Some(l), true) => commands.entity(l).despawn(),
            (Some(l), false) => {
                if let Ok(mut s) = settings.get_mut(l) {
                    let speed = 0.4 + (d.velocity.length() as f64 / 8.0).min(1.0) * 0.15;
                    if (s.speed - speed).abs() > 0.01 {
                        s.speed = speed;
                    }
                }
            }
            (None, false) => {
                if let Some(sample) = bank.pick("machines/drone_rotor", &mut rng.0) {
                    commands.spawn((
                        DirigibleLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db("dirigible.engine"))),
                        PlaybackSettings::default().with_speed(0.4),
                        Transform::default(),
                        Placed::new("dirigible.engine"),
                        ChildOf(e),
                    ));
                }
            }
            (None, true) => {}
        }
    }
}

/// The camera drone: the switch as its rotors spin up, a click in your
/// ear as it marks a robot, a knock as it settles back by you. A zipline: the clank
/// of its anchor biting, and its line coiled away.
#[allow(clippy::too_many_arguments)]
fn drone_and_zip_sounds(
    mut commands: Commands,
    drones: Query<(
        Entity,
        &crate::gadgets::camera_drone::CameraDrone,
        &GlobalTransform,
    )>,
    spotted: Query<(), Added<crate::gadgets::camera_drone::Spotted>>,
    ropes: Query<(Entity, &crate::gadgets::climbing::Rope)>,
    mut flight_was: Local<HashMap<Entity, crate::gadgets::camera_drone::Flight>>,
    mut zips: Local<HashMap<Entity, Vec3>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::gadgets::camera_drone::Flight;
    flight_was.retain(|e, _| drones.contains(*e));
    let mut out: Vec<(&str, &str, Vec3)> = Vec::new();
    for (e, d, gt) in &drones {
        let before = flight_was.insert(e, d.flight);
        match (before, d.flight) {
            (Some(Flight::Idle), Flight::SpunUp) => {
                out.push(("physical/switch", "camera_drone.on", gt.translation()))
            }
            (Some(Flight::Up | Flight::Returning), Flight::Idle) => out.push((
                "physical/handle_plastic",
                "camera_drone.land",
                gt.translation(),
            )),
            _ => {}
        }
    }
    // A robot marked: a click in your ear, as from the drone's handset.
    for _ in &spotted {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/switch",
            None,
            "camera_drone.spot",
            0.0,
            1.3,
            0.03,
        );
    }
    let mut live = Vec::new();
    for (e, r) in &ropes {
        if !r.zip {
            continue;
        }
        live.push(e);
        if zips.insert(e, r.top).is_none() {
            out.push(("impacts/metal", "zip.rig", r.top));
        }
    }
    zips.retain(|e, at| {
        let kept = live.contains(e);
        if !kept {
            out.push(("physical/cloth", "zip.pack", *at));
        }
        kept
    });
    for (set, cue, at) in out {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            cue,
            0.0,
            0.06,
        );
    }
}

/// The kitchen: each stroke of work sounds like the work (a knife on the
/// board, stones grinding, a spoon or hands in the bowl), a dish done is a
/// tap on the pot, an abandoned one a rustle.
fn kitchen_sounds(
    mut commands: Commands,
    mut events: MessageReader<crate::raid::kitchen::Cooking>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use extraction_core::cooking::{Event, Process};
    for c in events.read() {
        let (set, cue, speed) = match (c.what, c.process) {
            (Event::Stroke { .. }, Process::Chop) => ("impacts/wood", "kitchen.chop", 1.6),
            (Event::Stroke { .. }, Process::Grind) => ("footsteps/gravel", "kitchen.grind", 1.2),
            (Event::Stroke { .. }, _) => ("physical/cloth", "kitchen.work", 1.0),
            (Event::Done(_), _) => ("physical/handle_metal", "kitchen.done", 0.9),
            (Event::Aborted, _) => ("physical/cloth", "kitchen.work", 0.8),
            _ => continue,
        };
        let at = positions.get(c.station).ok().map(|g| g.translation());
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            at,
            cue,
            0.0,
            speed,
            0.08,
        );
    }
}

/// The robots' words being rendered, off the main thread: every kind's
/// every word, started once the stream's rate is known.
#[derive(Default)]
struct VoiceRenders {
    started: bool,
    tasks: Vec<(String, bevy::tasks::Task<Vec<Vec<f32>>>)>,
}

/// Render every word in the background at start-up (a few hundred ms of
/// synthesis in all, never on a frame) and add each to the bank as its
/// `voice/<kind>/<mind>` set when it's done.
fn render_voices(
    mut renders: Local<VoiceRenders>,
    mut bank: ResMut<SoundBank>,
    mut samples: ResMut<Assets<AudioSample>>,
    rate: Option<Res<bevy_seedling::context::SampleRate>>,
) {
    use crate::robot_voice as rv;
    let Some(rate) = rate.as_deref().map(|r| r.get()) else {
        return;
    };
    if !renders.started {
        renders.started = true;
        let pool = bevy::tasks::AsyncComputeTaskPool::get();
        for k in rv::VoiceKind::ALL {
            for u in rv::Utterance::ALL {
                let sr = rate.get() as f32;
                let task = pool.spawn(async move {
                    (0..rv::VARIANTS)
                        .map(|v| rv::render(k, u, v, sr))
                        .collect::<Vec<_>>()
                });
                renders.tasks.push((rv::set_name(k, u), task));
            }
        }
        let sr = rate.get() as f32;
        let horn = pool.spawn(async move { vec![rv::render_horn(sr)] });
        renders.tasks.push((HORN_SET.to_string(), horn));
        let ping = pool.spawn(async move { vec![rv::render_ping(sr)] });
        renders.tasks.push((PING_SET.to_string(), ping));
        let crackle = pool.spawn(async move { vec![rv::render_crackle(sr)] });
        renders.tasks.push((CRACKLE_SET.to_string(), crackle));
        // Stand-ins for recordings not yet fetched (`synth_foley`), unless
        // the bank has the real thing.
        for st in crate::synth_foley::STAND_INS {
            if bank.has(st.set) {
                continue;
            }
            let (set, variants) = (st.set, st.variants);
            let task = pool.spawn(async move {
                (0..variants)
                    .filter_map(|v| crate::synth_foley::render(set, v, sr))
                    .collect::<Vec<_>>()
            });
            renders.tasks.push((set.to_string(), task));
        }
    }
    renders.tasks.retain_mut(|(set, task)| {
        let Some(buffers) = bevy::tasks::futures::check_ready(task) else {
            return true;
        };
        let handles = buffers
            .into_iter()
            .map(|x| samples.add(AudioSample::new(vec![x], rate)))
            .collect();
        bank.sets.insert(set.clone(), handles);
        false
    });
}

/// What robots say (`robot_voice::Spoke`): each utterance one cue, from
/// the robot, through the head-related filter, walls and the room like
/// any other sound, from the sets [`render_voices`] made. (A word asked
/// for in the first moments, before its render is done, isn't heard.)
fn voice_sounds(
    mut commands: Commands,
    mut spoke: MessageReader<crate::robot_voice::Spoke>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::robot_voice as rv;
    for s in spoke.read() {
        let set = rv::set_name(s.kind, s.said);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            &set,
            Some(s.at + Vec3::Y * 0.5),
            s.said.cue(),
            voice_trim(s.kind),
            0.0,
        );
    }
}

/// A kind's level against its words' cue, dB, by measurement: the big
/// ones' low voices carry more loudness per sample, the crawler's less
/// (`scripts/audio_voices.py` keeps them within 6 LU).
pub fn voice_trim(kind: crate::robot_voice::VoiceKind) -> f32 {
    use crate::robot_voice::VoiceKind as K;
    match kind {
        K::Dirigible => -1.5,
        K::Warden => -1.0,
        K::Crawler => -1.0,
        _ => 0.0,
    }
}

/// The attack tells: a walker's turret settling before its first shot
/// and a grappler winding up its throw. (What a robot thinks it says in
/// its voice: [`voice_sounds`].)
fn mind_sounds(
    mut commands: Commands,
    mut aims: MessageReader<crate::robots::walker::WalkerAim>,
    mut windups: MessageReader<crate::robots::grappler::HarpoonWindup>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    let mut out: Vec<(&str, &str, Vec3, f64)> = Vec::new();
    for m in aims.read() {
        out.push(("machines/clank", "walker.aim", m.at, 1.3));
        out.push(("machines/servo", "walker.aim_lock", m.at, 1.5));
    }
    for m in windups.read() {
        if let Ok(gt) = positions.get(m.from) {
            out.push(("machines/servo", "harpoon.windup", gt.translation(), 0.8));
        }
    }
    for (set, cue, at, speed) in out {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            cue,
            0.0,
            speed,
            0.04,
        );
    }
}

/// The space you're in, probed from your head a few times a second, sets
/// the reverb: dry in the open, a short warm tail in a cabin, a ring in a
/// tin shed, a long echo underground. Eased over half a second, so a
/// doorway is a crossfade, not a switch.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Listening {
    pub space: crate::acoustics::Space,
}

fn room_acoustics(
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &Surfaces)>,
    lighting: Option<Res<crate::robots::senses::Lighting>>,
    time: Res<Time>,
    mut listening: ResMut<Listening>,
    mut send: Query<&mut VolumeNode, With<ReverbIn>>,
    mut reverb: Query<&mut FreeverbNode>,
    mut next: Local<f32>,
    mut now: Local<Option<crate::acoustics::Reverb>>,
) {
    let Some(head) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    *next -= time.delta_secs();
    if *next <= 0.0 {
        *next = 0.2;
        listening.space = crate::acoustics::probe(&spatial, &built, lighting.as_deref(), head);
    }
    let want = listening.space.reverb();
    let k = (time.delta_secs() * 2.0).min(1.0);
    let cur = now.get_or_insert(want);
    cur.wet_db += (want.wet_db - cur.wet_db) * k;
    cur.rt60 += (want.rt60 - cur.rt60) * k;
    cur.damping += (want.damping - cur.damping) * k;
    for mut v in &mut send {
        v.volume = if cur.wet_db <= crate::acoustics::DRY + 1.0 {
            Volume::SILENT
        } else {
            Volume::Decibels(cur.wet_db)
        };
    }
    for mut r in &mut reverb {
        let room = crate::acoustics::freeverb_room(cur.rt60);
        if (r.room_size - room).abs() > 0.005 || (r.damping - cur.damping).abs() > 0.005 {
            r.room_size = room;
            r.damping = cur.damping;
        }
    }
}

/// A placed sound heard round or through what's between it and you
/// (`hearing::Hearing::path`, the robots' rule): checked as it starts,
/// and every quarter second for loops.
#[derive(Component)]
struct Occluded {
    next: f32,
}

#[allow(clippy::too_many_arguments)]
fn occlude(
    mut commands: Commands,
    players: Query<
        (
            Entity,
            &GlobalTransform,
            &SampleEffects,
            &SamplePlayer,
            Option<&Occluded>,
        ),
        Without<SpatialListener3D>,
    >,
    mut nodes: Query<&mut SpatialBasicNode>,
    mut heads: Query<&mut HrtfNode>,
    mut volumes: Query<&mut VolumeNode>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    hush: Res<HushNow>,
    hearing: Res<crate::hearing::Hearing>,
    zone: Option<Res<Zone>>,
    spatial: SpatialQuery,
    time: Res<Time>,
) {
    let blocked = |a: Vec3, b: Vec3| crate::acoustics::blocked(&spatial, a, b);
    let Some(head) = listener.iter().next() else {
        return;
    };
    let (ear, facing) = (head.translation(), head.forward().as_vec3());
    let now = time.elapsed_secs();
    // Each a table read (`hearing`), but a frame needn't redo them all.
    let mut budget = 128;
    for (e, gt, effects, player, occluded) in &players {
        let due = match occluded {
            None => true,
            Some(o) => {
                o.next.is_finite() && player.repeat_mode != RepeatMode::PlayOnce && now >= o.next
            }
        };
        if !due || budget == 0 {
            continue;
        }
        // The way the robots hear it too (`hearing`): round doors, through
        // walls by what they're built of, up through floors and rock.
        let o = || {
            let mut o = hearing.path(zone.as_deref(), gt.translation(), ear, Some(&blocked));
            o.cutoff = o.cutoff.min(hush.cutoff);
            o
        };
        if let Ok(mut node) = nodes.get_effect_mut(effects) {
            budget -= 1;
            let mut o = o();
            // Panned: what's behind you, shaded as the pinna would.
            o.cutoff = o.cutoff.min(behind_cutoff(facing, gt.translation() - ear));
            let v = Volume::Decibels(o.db);
            if node.volume != v {
                node.volume = v;
            }
            if (node.muffle_cutoff_hz - o.cutoff).abs() > 1.0 {
                node.muffle_cutoff_hz = o.cutoff;
            }
        } else if let Ok(mut node) = heads.get_effect_mut(effects) {
            // Binaural: the level on the volume ahead of it, the muffle on it.
            budget -= 1;
            let o = o();
            if (node.muffle_cutoff_hz - o.cutoff).abs() > 1.0 {
                node.muffle_cutoff_hz = o.cutoff;
            }
            if let Ok(mut v) = volumes.get_effect_mut(effects) {
                let want = Volume::Decibels(o.db);
                if v.volume != want {
                    v.volume = want;
                }
            }
        } else {
            // Not placed (in your head): nothing to occlude, ever.
            commands.entity(e).insert(Occluded {
                next: f32::INFINITY,
            });
            continue;
        }
        commands.entity(e).insert(Occluded { next: now + 0.25 });
    }
}

/// Spent cases landing, a beat after the shot: brass rings on steel and
/// concrete, knocks on boards, ticks in the grass, plips in water; a
/// shotgun hull thunks. The first landing is the clink, the bounces
/// quieter.
fn casing_sounds(
    mut commands: Commands,
    mut landed: MessageReader<crate::weapons::CasingLanded>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::weapons::CasingFloor as F;
    for c in landed.read() {
        let (set, trim, speed) = match (c.floor, c.hull) {
            (F::Water, _) => ("footsteps/water", -10.0, 2.2),
            (F::Soft, false) => ("guns/casing", -14.0, 0.7),
            (F::Soft, true) => continue,
            (F::Metal, false) => ("guns/casing", 0.0, 1.1),
            (F::Hard, false) => ("guns/casing", 0.0, 1.0),
            (F::Wood, false) => ("guns/casing", -4.0, 0.8),
            (_, true) => ("physical/drop_plastic", -6.0, 1.2),
        };
        let bounce = if c.first { 0.0 } else { -8.0 };
        let hard = ((c.speed - 3.0) * 1.0).clamp(-4.0, 2.0);
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(c.at),
            "gun.casing",
            trim + bounce + hard,
            speed,
            0.08,
        );
    }
}

/// The anomaly detector ticking: dry clicks, at the hand when it's held
/// (close and louder), from the hip otherwise; brighter near an edge.
fn detector_sounds(
    mut commands: Commands,
    mut clicks: MessageReader<crate::anomaly::detector::DetectorClick>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for c in clicks.read() {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/detector_click",
            Some(c.at),
            "detector.click",
            if c.held { 0.0 } else { -8.0 },
            1.0 + 0.15 * c.near as f64,
            0.03,
        );
    }
}

/// A site's power coming on: a breaker's lever clanks home; a generator
/// is pulled (a whirr of cord) and runs by the switch for as long as it's
/// on (`machines/generator`, synthesised until a recording can be had).
#[derive(Component)]
struct GeneratorLoop;

fn power_sounds(
    mut commands: Commands,
    sites: Query<(Entity, &crate::world::underground::SitePower)>,
    switches: Query<(&GlobalTransform, Option<&Children>)>,
    loops: Query<Entity, With<GeneratorLoop>>,
    mut was: Local<HashMap<(Entity, usize), bool>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use extraction_core::underground::PowerKind;
    // Each stage of a site's power has its own switch, and its own sound.
    for (site, power) in &sites {
        for (i, stage) in power.stages.iter().enumerate() {
            let before = was.insert((site, i), stage.on).unwrap_or(false);
            let Ok((gt, children)) = switches.get(stage.switch) else {
                continue;
            };
            let at = gt.translation();
            if stage.on && !before {
                match stage.kind {
                    PowerKind::Breaker => play_at_speed(
                        &mut commands,
                        &bank,
                        &mut rng.0,
                        "machines/clank",
                        Some(at),
                        "power.breaker",
                        0.0,
                        0.8,
                        0.05,
                    ),
                    PowerKind::Generator => play_at_speed(
                        &mut commands,
                        &bank,
                        &mut rng.0,
                        "physical/whoosh",
                        Some(at),
                        "power.pull",
                        0.0,
                        0.7,
                        0.05,
                    ),
                }
            }
            let running = stage.on && stage.kind == PowerKind::Generator;
            let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
            match (running, existing) {
                (true, None) => {
                    if let Some(sample) = bank.pick("machines/generator", &mut rng.0) {
                        commands.spawn((
                            GeneratorLoop,
                            SamplePlayer::new(sample)
                                .looping()
                                .with_volume(Volume::Decibels(mix::db("power.generator"))),
                            PlaybackSettings::default(),
                            Transform::default(),
                            Placed::new("power.generator"),
                            ChildOf(stage.switch),
                        ));
                    }
                }
                (false, Some(l)) => commands.entity(l).despawn(),
                _ => {}
            }
        }
    }
}

/// A pylon going up or coming down: servos and clanks from its mast.
fn pylon_sounds(
    mut commands: Commands,
    pylons: Query<(&crate::robots::pylon::Pylon, &GlobalTransform)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    use crate::robots::pylon::PylonState;
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.9;
    for (p, gt) in &pylons {
        if !matches!(p.state, PylonState::Building | PylonState::Packing) {
            continue;
        }
        let at = gt.translation() + Vec3::Y * (1.2 + crate::robots::pylon::MAST * p.progress);
        let set = if rng.0.f32() < 0.6 {
            "machines/servo"
        } else {
            "machines/clank"
        };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            "pylon.work",
            0.0,
            0.2,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thunder is quieter and darker the further the strike: 6 dB a
    /// doubling past 250 m, the roll from 1.5 kHz at 600 m to 300 Hz at
    /// 2.6 km.
    #[test]
    fn far_thunder_is_low_and_quiet() {
        assert_eq!(thunder_db(250.0, 1.0), 0.0);
        assert!((thunder_db(1000.0, 1.0) + 12.0).abs() < 1e-4);
        assert!(thunder_db(1000.0, 0.2) < thunder_db(1000.0, 1.0));
        assert!((thunder_cutoff(600.0) - 1500.0).abs() < 1.0);
        assert!((thunder_cutoff(2600.0) - 300.0).abs() < 1.0);
        assert!(thunder_cutoff(1200.0) < thunder_cutoff(800.0));
        assert!(thunder_cutoff(300.0) > thunder_cutoff(600.0));
    }

    /// The night's sounds come with the dark.
    #[test]
    fn night_comes_with_the_dark() {
        use crate::world::weather::Hour;
        assert_eq!(nightness(Hour::Night), 1.0);
        assert_eq!(nightness(Hour::Dusk), 0.5);
        assert_eq!(nightness(Hour::Day), 0.0);
        assert_eq!(nightness(Hour::Dawn), 0.0);
    }

    /// The ending's hush eases in, holds, and lets go.
    #[test]
    fn the_hush_eases_in_and_out() {
        let at = |left| Hush { left }.db();
        assert_eq!(at(0.0), 0.0);
        assert_eq!(at(HUSH_SECONDS), 0.0);
        assert_eq!(at(HUSH_SECONDS / 2.0), HUSH_DB);
        assert!(at(HUSH_SECONDS - 0.5) > 0.0 && at(HUSH_SECONDS - 0.5) < HUSH_DB);
    }

    /// Using a thing sounds like what it is, and anything you'd eat or use
    /// up makes a sound (silence read as "nothing happened" in VR).
    #[test]
    fn using_things_sounds_like_what_they_are() {
        let catalog = crate::items::Catalog::load();
        let sound = |id: &str| {
            let (_, def) = catalog.0.iter().find(|(_, d)| d.id == id).expect(id);
            use_sound(def).map(|s| s.0)
        };
        assert_eq!(sound("crackers"), Some("physical/eat"));
        assert_eq!(sound("water_bottle"), Some("physical/drink"));
        assert_eq!(sound("field_dressing"), Some("physical/bandage"));
        assert_eq!(sound("flashlight"), Some("physical/switch"));
        assert_eq!(sound("radio"), None, "the radio has its own");
        let bank: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(crate::asset_root_path().join("audio/sounds.json")).unwrap(),
        )
        .unwrap();
        for (_, def) in catalog.0.iter() {
            if def.category == extraction_core::items::Category::Food {
                assert!(use_sound(def).is_some(), "{} is eaten silently", def.id);
            }
            if let Some((set, _)) = use_sound(def) {
                assert!(bank.get(set).is_some(), "{set}");
            }
            let handle = handling_set(crate::items::impacts::material_of(def));
            assert!(bank.get(handle).is_some(), "{handle}");
        }
    }

    /// A real audio graph (seedling's mock backend) with the reverb bus,
    /// a listener, and placed loops: `near` of them 3 m off, one 60 m off.
    fn mock_graph(binaural: Binaural, near: usize) -> App {
        use bevy_seedling::platform::mock::MockBackendPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy_seedling::SeedlingCorePlugin,
            MockBackendPlugin,
            TransformPlugin,
        ))
        .insert_resource(binaural)
        .add_observer(place)
        .add_systems(Startup, move |mut commands: Commands| {
            spawn_reverb_bus(&mut commands);
            commands.spawn((Transform::default(), SpatialListener3D));
            for i in 0..=near {
                let at = if i == near { 60.0 } else { 3.0 };
                commands.spawn((
                    SamplePlayer::new(Handle::<AudioSample>::default()).looping(),
                    Transform::from_xyz(at, 0.0, i as f32 * 0.1),
                    Placed::new("step"),
                ));
            }
        });
        app.finish();
        app.cleanup();
        for _ in 0..10 {
            app.update();
        }
        app
    }

    fn graph_edges(
        world: &mut World,
    ) -> Vec<(
        bevy_seedling::firewheel::node::NodeID,
        bevy_seedling::firewheel::node::NodeID,
    )> {
        world
            .resource_mut::<bevy_seedling::context::AudioContext>()
            .with(|cx| cx.edges().map(|e| (e.src_node, e.dst_node)).collect())
    }

    /// On a real audio graph: the reverb bus feeds the reverb, and a placed
    /// sound's send reaches the bus.
    #[test]
    fn placed_sounds_reach_the_reverb() {
        use bevy_seedling::node::FirewheelNode;
        let mut app = mock_graph(Binaural::default(), 1);
        let world = app.world_mut();
        let bus = world
            .query_filtered::<&FirewheelNode, With<ReverbIn>>()
            .iter(world)
            .next()
            .map(|n| n.0)
            .expect("the bus is a node");
        let verb = world
            .query_filtered::<&FirewheelNode, With<FreeverbNode>>()
            .iter(world)
            .next()
            .map(|n| n.0)
            .expect("the reverb is a node");
        let sends: Vec<_> = world
            .query_filtered::<&FirewheelNode, With<SendNode>>()
            .iter(world)
            .map(|n| n.0)
            .collect();
        assert!(!sends.is_empty(), "a send was made");
        let edges = graph_edges(world);
        assert!(edges.contains(&(bus, verb)), "bus -> reverb");
        assert!(
            sends.iter().any(|s| edges.contains(&(*s, bus))),
            "send -> bus: {edges:?}"
        );
        assert_eq!(
            world.query::<&HrtfNode>().iter(world).count(),
            0,
            "panned when binaural is off"
        );
    }

    /// Binaural on: near sounds go through the head-related filter, up to
    /// the cap; past it, and far off, they're panned; the filter feeds
    /// its send to the reverb.
    #[test]
    fn binaural_voices_are_capped_and_near() {
        use bevy_seedling::node::FirewheelNode;
        let bytes = std::fs::read(crate::asset_root_path().join("audio/hrir/sadie_d2_kemar.bin"))
            .expect("the sphere is built");
        let binaural = Binaural {
            on: true,
            sphere: Some(bevy_seedling::prelude::hrtf::SubjectBytes::new(&bytes)),
        };
        let near = HRTF_VOICES + 4;
        let mut app = mock_graph(binaural, near);
        let world = app.world_mut();
        // Sounds, not nodes: seedling plays them through pools of voices
        // (4 to 32), one node chain each.
        let voices: Vec<f32> = world
            .query_filtered::<&Transform, With<BinauralVoice>>()
            .iter(world)
            .map(|t| t.translation.x)
            .collect();
        assert_eq!(voices.len(), HRTF_VOICES, "binaural up to the cap");
        assert!(
            voices.iter().all(|x| *x < BINAURAL_REACH),
            "far ones panned"
        );
        let heads: Vec<_> = world
            .query_filtered::<&FirewheelNode, With<HrtfNode>>()
            .iter(world)
            .map(|n| n.0)
            .collect();
        assert!(!heads.is_empty(), "the filter is a node");
        let bus = world
            .query_filtered::<&FirewheelNode, With<ReverbIn>>()
            .iter(world)
            .next()
            .map(|n| n.0)
            .unwrap();
        let sends: Vec<_> = world
            .query_filtered::<&FirewheelNode, With<SendNode>>()
            .iter(world)
            .map(|n| n.0)
            .collect();
        let edges = graph_edges(world);
        for h in &heads {
            let to: Vec<_> = edges.iter().filter(|e| e.0 == *h).map(|e| e.1).collect();
            assert!(
                to.iter().any(|t| sends.contains(t)),
                "the filter feeds its send: {to:?}"
            );
        }
        assert!(sends.iter().all(|s| edges.contains(&(*s, bus))));
    }

    /// The score is a node in the real graph, out to the main bus.
    #[test]
    fn the_score_joins_the_graph() {
        use bevy_seedling::node::FirewheelNode;
        use bevy_seedling::platform::mock::MockBackendPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy_seedling::SeedlingCorePlugin,
            MockBackendPlugin,
        ))
        .register_node::<ScoreNode>()
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(ScoreNode::default());
        });
        app.finish();
        app.cleanup();
        for _ in 0..10 {
            app.update();
        }
        let world = app.world_mut();
        let score = world
            .query_filtered::<&FirewheelNode, With<ScoreNode>>()
            .single(world)
            .expect("the score is a node")
            .0;
        let main = world
            .query_filtered::<&FirewheelNode, With<MainBus>>()
            .single(world)
            .unwrap()
            .0;
        assert!(
            graph_edges(world).contains(&(score, main)),
            "score -> main bus"
        );
    }

    /// The directions the binaural audit renders, in the listener's frame
    /// (+X right, +Y up, -Z ahead).
    const EARS_FROM: [(&str, [f32; 3]); 8] = [
        ("front", [0.0, 0.0, -1.0]),
        ("back", [0.0, 0.0, 1.0]),
        ("left", [-1.0, 0.0, 0.0]),
        ("right", [1.0, 0.0, 0.0]),
        ("up", [0.0, 1.0, 0.0]),
        ("down", [0.0, -1.0, 0.0]),
        ("front_up", [0.0, 0.707, -0.707]),
        ("back_up", [0.0, 0.707, 0.707]),
    ];

    /// A click rendered by the `hrtf` crate (the node's own convolution)
    /// through a sphere from a direction: what each ear hears.
    fn render_from(sphere: &[u8], dir: [f32; 3]) -> (Vec<f32>, Vec<f32>) {
        let s = ::hrtf::HrirSphere::new(std::io::Cursor::new(sphere.to_vec()), 48000).unwrap();
        let block = HRTF_SLICE * 4;
        let mut p = ::hrtf::HrtfProcessor::new(s, 4, HRTF_SLICE);
        let (mut prev_l, mut prev_r) = (Vec::new(), Vec::new());
        let v = ::hrtf::Vec3::new(dir[0], dir[1], dir[2]);
        let (mut l, mut r) = (Vec::new(), Vec::new());
        for k in 0..3 {
            let mut src = vec![0.0f32; block];
            if k == 0 {
                src[0] = 1.0;
            }
            let mut out = vec![(0.0f32, 0.0f32); block];
            p.process_samples(::hrtf::HrtfContext {
                source: &src,
                output: &mut out,
                new_sample_vector: v,
                prev_sample_vector: v,
                prev_left_samples: &mut prev_l,
                prev_right_samples: &mut prev_r,
                new_distance_gain: 1.0,
                prev_distance_gain: 1.0,
            });
            l.extend(out.iter().map(|x| x.0));
            r.extend(out.iter().map(|x| x.1));
        }
        (l, r)
    }

    /// Mean power (dB) of a response between `lo` and `hi` Hz.
    fn band_db(h: &[f32], lo: f32, hi: f32) -> f32 {
        let n = h.len();
        let (a, b) = (
            (lo * n as f32 / 48000.0) as usize,
            (hi * n as f32 / 48000.0) as usize,
        );
        let power: f32 = (a..b)
            .map(|k| {
                let w = -std::f32::consts::TAU * k as f32 / n as f32;
                let (re, im) = h.iter().enumerate().fold((0.0, 0.0), |(re, im), (i, x)| {
                    (re + x * (w * i as f32).cos(), im + x * (w * i as f32).sin())
                });
                re * re + im * im
            })
            .sum();
        10.0 * (power / (b - a) as f32).log10()
    }

    /// The headset's head-related responses (SADIE II's KEMAR, written in
    /// the listener's frame by scripts/audio_hrir.py) carry the cues,
    /// rendered through the node's convolution: the near ear louder, front
    /// brighter than behind in the pinna's band (4-10 kHz against
    /// 0.5-3 kHz), above brighter still. Writes target/audio_audit/
    /// binaural.json, ours and the node's own IRCAM subject, for the audio
    /// audit (`scripts/audio_binaural.py`), which checks the same cues.
    #[test]
    fn binaural_tells_front_from_back() {
        let ours = std::fs::read(crate::asset_root_path().join("audio/hrir/sadie_d2_kemar.bin"))
            .expect("the sphere is built");
        // The node's scratch buffer only fits an FFT that factors small.
        let taps = u32::from_le_bytes(ours[8..12].try_into().unwrap()) as usize;
        assert!(
            (HRTF_SLICE + taps - 1).is_power_of_two(),
            "{taps} taps make a {}-point FFT",
            HRTF_SLICE + taps - 1
        );
        let ircam: &[u8] = bevy_seedling::prelude::hrtf::Subject::Irc1040.as_ref();
        let mut json = serde_json::Map::new();
        for (name, sphere) in [("sadie_d2_kemar", &ours[..]), ("ircam_1040", ircam)] {
            let mut dirs = serde_json::Map::new();
            for (dir, v) in EARS_FROM {
                let (l, r) = render_from(sphere, v);
                dirs.insert(dir.into(), serde_json::json!({ "left": l, "right": r }));
            }
            json.insert(name.into(), dirs.into());
        }
        let dir = crate::asset_root_path().join("../target/audio_audit");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("binaural.json"),
            serde_json::to_string(&serde_json::json!({ "rate": 48000, "subjects": json })).unwrap(),
        )
        .unwrap();
        // Tilt: the pinna band over the low, both ears.
        let tilt = |v: [f32; 3]| {
            let (l, r) = render_from(&ours, v);
            let t = |h: &[f32]| band_db(h, 4000.0, 10000.0) - band_db(h, 500.0, 3000.0);
            (t(&l) + t(&r)) / 2.0
        };
        let (l, r) = render_from(&ours, [-1.0, 0.0, 0.0]);
        let ild = band_db(&l, 500.0, 10000.0) - band_db(&r, 500.0, 10000.0);
        assert!(ild > 6.0, "left louder in the left ear by {ild:.1} dB");
        let (front, back, up) = (
            tilt([0.0, 0.0, -1.0]),
            tilt([0.0, 0.0, 1.0]),
            tilt([0.0, 1.0, 0.0]),
        );
        assert!(
            front - back > 2.5,
            "front brighter than back by {:.1} dB",
            front - back
        );
        assert!(
            up - front > 2.0,
            "above brighter than ahead by {:.1} dB",
            up - front
        );
    }

    /// The head-related responses on a grid of bearings (clockwise from
    /// ahead, level) and elevations (ahead and behind), for the audit's
    /// localisation pass (`scripts/audio_localise.py`), which plays each
    /// threat's own sound through them at the ranges it matters: written
    /// to target/audio_audit/hrir_grid.json. Our sphere only (the one the
    /// game loads).
    #[test]
    fn localisation_grid() {
        let ours = std::fs::read(crate::asset_root_path().join("audio/hrir/sadie_d2_kemar.bin"))
            .expect("the sphere is built");
        let dir = |az: f32, el: f32| {
            let (a, e) = (az.to_radians(), el.to_radians());
            [a.sin() * e.cos(), e.sin(), -a.cos() * e.cos()]
        };
        let mut grid = serde_json::Map::new();
        let level = [0.0, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0].map(|a| (a, 0.0));
        let raised = [30.0, 60.0, 85.0].map(|e| (0.0, e));
        let behind_up = [(180.0, 30.0), (180.0, 60.0)];
        for (az, el) in level.into_iter().chain(raised).chain(behind_up) {
            let (l, r) = render_from(&ours, dir(az, el));
            grid.insert(
                format!("az{az:.0}_el{el:.0}"),
                serde_json::json!({ "left": l, "right": r }),
            );
        }
        let out = crate::asset_root_path().join("../target/audio_audit");
        let _ = std::fs::create_dir_all(&out);
        std::fs::write(
            out.join("hrir_grid.json"),
            serde_json::to_string(&serde_json::json!({ "rate": 48000, "grid": grid })).unwrap(),
        )
        .unwrap();
        // Sanity: a source at the right is louder in the right ear.
        let (l, r) = render_from(&ours, dir(90.0, 0.0));
        assert!(band_db(&r, 500.0, 10000.0) > band_db(&l, 500.0, 10000.0) + 6.0);
    }

    /// Near sounds take the head-related filter while there are voices;
    /// far ones, or past the cap, or in flat mode, pan.
    #[test]
    fn binaural_is_for_what_is_near() {
        let near = BINAURAL_REACH;
        assert!(binaural_voice(true, 0, 3.0, near));
        assert!(!binaural_voice(false, 0, 3.0, near));
        assert!(!binaural_voice(true, HRTF_VOICES, 3.0, near));
        assert!(!binaural_voice(true, 0, BINAURAL_REACH + 1.0, near));
        // A threat keeps the ears' filter further; a detail doesn't.
        assert_eq!(binaural_reach("drone.rotor"), SIGNAL_BINAURAL_REACH);
        assert_eq!(binaural_reach("gear.plate"), BINAURAL_REACH);
        assert!(binaural_voice(
            true,
            0,
            100.0,
            binaural_reach("drone.rotor")
        ));
    }

    /// Panned, a sound behind you is muffled, one ahead or beside isn't.
    #[test]
    fn the_panner_darkens_what_is_behind() {
        let ahead = Vec3::NEG_Z;
        assert!(behind_cutoff(ahead, Vec3::NEG_Z) > 20_000.0);
        assert!(behind_cutoff(ahead, Vec3::X) > 20_000.0);
        assert!((behind_cutoff(ahead, Vec3::Z) - BEHIND_CUTOFF).abs() < 1.0);
        let half = behind_cutoff(ahead, Vec3::new(1.0, 0.0, 1.0));
        assert!(BEHIND_CUTOFF < half && half < 20_000.0);
    }

    /// Heavier sounds louder and lower, within bounds.
    #[test]
    fn weight_is_heard() {
        let (tin, rifle) = (weight_tone(0.05), weight_tone(3.5));
        assert!(tin.0 < rifle.0 && tin.1 > rifle.1);
        for m in [0.0, 0.01, 1.0, 50.0] {
            let (db, speed) = weight_tone(m);
            assert!((-9.0..=5.0).contains(&db) && (0.85..=1.2).contains(&speed));
        }
    }

    /// Every sound a map asks for is in the index (a missing set plays
    /// nothing, silently).
    #[test]
    fn every_valley_sound_exists() {
        let path = crate::asset_root_path().join("audio/sounds.json");
        let index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut missing = Vec::new();
        for map in crate::test_maps() {
            let sounds = crate::world::Zone::load(&map).map.sounds.clone();
            for set in std::iter::once(&sounds.wind)
                .chain(&sounds.birds)
                .chain(&sounds.distant)
            {
                if index.get(set).is_none() {
                    missing.push(format!("{map}: {set}"));
                }
            }
        }
        assert!(missing.is_empty(), "{missing:?}");
    }
}
