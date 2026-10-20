//! What the machines say: a small synthesised vocabulary for each kind of
//! robot, one utterance for each turn of its mind, so a machine's intent
//! can be read by ear.
//!
//! **One grammar.** Every kind says the same things the same way; only
//! the voice differs. Learn one kind's and the next kind's makes sense:
//!
//! | Mind | Shape |
//! | --- | --- |
//! | idle, on its rounds | two soft low notes, level, a little apart |
//! | noticed something | one note gliding up: a question |
//! | investigating | three short notes stepping up |
//! | searching | four short notes see-sawing: scanning |
//! | certain, engaging | two hard notes, high and falling: a statement |
//! | lost you, giving up | one long note sinking away |
//! | calling others | three pulses rising to a held note |
//! | answering a call | the call's top note, then a step down: "heard" |
//! | hurt | a stutter of rough notes, pitch thrown about |
//! | dying | one long fall, slowing and breaking up into noise |
//!
//! **A voice per kind** ([`Timbre`]): its pitch, its harmonics (thin,
//! hollow, bright), a formant that colours it, how much noise and buzz is
//! in it, and its pace. A drone is a quick thin whistle; a walker a hollow
//! mid voice; a grappler the walker's with a metal ring; the dirigible a
//! slow deep horn; a crawler fast insect chirps; a hound a rough bark; a
//! warden a low heavy walker.
//!
//! Each utterance is rendered once, deterministically from its kind,
//! mind and variant ([`render`]), into a mono buffer at the stream's
//! rate; `audio` plays them as ordinary sets (`voice/<kind>/<mind>`), so
//! they are placed, heard through the head-related filter, muffled by
//! walls and doors and sent to the room's reverb like any other sound.
//! Renders are levelled to one loudness; the mix table (`voice.<mind>`)
//! sets how loud and how far each is heard. `scripts/audio_voices.py`
//! measures the renders (written by this module's test) against the mix.
//!
//! **Who says what** ([`VoicePlugin`], headless): robots' minds turning
//! become [`Turned`]; [`speak`] lets each through as exactly one
//! [`Spoke`] (the audio plays it, the wrist feels a nearby machine turn
//! certain, the listening scenario logs it), except a robot doesn't say
//! the same thing twice within [`REPEAT`] s, isn't hurt more than once a
//! second, and says nothing once it's said it's dying. Answers to a call
//! come from the nearest robots that heard it, a beat later; a response
//! team answers as it sets out, from wherever it starts (so a call
//! answered far off is the warning that they're coming, heard as the
//! radio picks up their chatter). While a machine goes about its rounds
//! within [`IDLE_NEAR`] of someone it mutters now and then.
//!
//! The synth is logic only: no audio types, runs anywhere.

use bevy::prelude::*;
use extraction_core::noise::Rng;
use std::collections::HashMap;
use std::f32::consts::TAU;

use crate::player::Player;
use crate::robots::search::{CalledOut, Investigating, LostYou, Searching};
use crate::robots::{
    Brain, Job, JobKind, Noticed, PartDestroyed, PartRole, Robot, RobotKind, RobotPart,
};

/// The kinds that speak.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VoiceKind {
    Drone,
    Walker,
    Grappler,
    Dirigible,
    Crawler,
    Hound,
    Warden,
}

impl VoiceKind {
    pub const ALL: [Self; 7] = [
        Self::Drone,
        Self::Walker,
        Self::Grappler,
        Self::Dirigible,
        Self::Crawler,
        Self::Hound,
        Self::Warden,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Drone => "drone",
            Self::Walker => "walker",
            Self::Grappler => "grappler",
            Self::Dirigible => "dirigible",
            Self::Crawler => "crawler",
            Self::Hound => "hound",
            Self::Warden => "warden",
        }
    }
}

/// What a mind says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Utterance {
    Idle,
    Noticed,
    Investigating,
    Searching,
    Engaging,
    Lost,
    Calling,
    Answering,
    Hurt,
    Dying,
}

impl Utterance {
    pub const ALL: [Self; 10] = [
        Self::Idle,
        Self::Noticed,
        Self::Investigating,
        Self::Searching,
        Self::Engaging,
        Self::Lost,
        Self::Calling,
        Self::Answering,
        Self::Hurt,
        Self::Dying,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Noticed => "noticed",
            Self::Investigating => "investigating",
            Self::Searching => "searching",
            Self::Engaging => "engaging",
            Self::Lost => "lost",
            Self::Calling => "calling",
            Self::Answering => "answering",
            Self::Hurt => "hurt",
            Self::Dying => "dying",
        }
    }

    /// Its mix cue (`audio_mix`).
    pub fn cue(self) -> &'static str {
        match self {
            Self::Idle => "voice.idle",
            Self::Noticed => "voice.noticed",
            Self::Investigating => "voice.investigating",
            Self::Searching => "voice.searching",
            Self::Engaging => "voice.engaging",
            Self::Lost => "voice.lost",
            Self::Calling => "voice.calling",
            Self::Answering => "voice.answering",
            Self::Hurt => "voice.hurt",
            Self::Dying => "voice.dying",
        }
    }
}

/// The bank set an utterance plays from.
pub fn set_name(kind: VoiceKind, u: Utterance) -> String {
    format!("voice/{}/{}", kind.name(), u.name())
}

/// Variants rendered of each utterance.
pub const VARIANTS: usize = 3;

/// One note of an utterance: when (s), how long, the pitch it glides
/// between (semitones over the voice's base), how loud, and how much extra
/// noise is in it. `curve` > 1 holds the start pitch longer before the
/// glide (a question's lift at the end); < 1 moves early.
#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub at: f32,
    pub len: f32,
    pub from: f32,
    pub to: f32,
    pub gain: f32,
    pub noise: f32,
    pub curve: f32,
}

const fn n(at: f32, len: f32, from: f32, to: f32, gain: f32) -> Note {
    Note {
        at,
        len,
        from,
        to,
        gain,
        noise: 0.0,
        curve: 1.0,
    }
}

/// The grammar: each mind's notes, in the voice's own time (scaled by its
/// pace). Shared by every kind.
pub fn phrase(u: Utterance) -> Vec<Note> {
    match u {
        Utterance::Idle => vec![n(0.0, 0.16, 0.0, -1.0, 0.55), n(0.3, 0.2, -2.0, -3.0, 0.45)],
        Utterance::Noticed => vec![Note {
            curve: 2.2,
            ..n(0.0, 0.42, -2.0, 7.0, 1.0)
        }],
        Utterance::Investigating => vec![
            n(0.0, 0.09, 0.0, 2.0, 0.8),
            n(0.15, 0.09, 2.0, 4.0, 0.85),
            n(0.3, 0.12, 4.0, 5.0, 0.9),
        ],
        Utterance::Searching => vec![
            n(0.0, 0.09, 0.0, 3.0, 0.75),
            n(0.17, 0.09, 3.0, 0.0, 0.75),
            n(0.34, 0.09, 0.0, 3.0, 0.75),
            n(0.51, 0.11, 3.0, -1.0, 0.7),
        ],
        Utterance::Engaging => vec![
            Note {
                noise: 0.15,
                curve: 0.5,
                ..n(0.0, 0.08, 10.0, 7.0, 1.0)
            },
            Note {
                noise: 0.15,
                curve: 0.5,
                ..n(0.11, 0.15, 9.0, 3.0, 1.0)
            },
        ],
        Utterance::Lost => vec![Note {
            curve: 0.6,
            ..n(0.0, 0.95, 3.0, -9.0, 0.8)
        }],
        Utterance::Calling => vec![
            n(0.0, 0.24, 0.0, 5.0, 0.85),
            n(0.36, 0.24, 5.0, 7.0, 0.9),
            n(0.72, 0.55, 7.0, 7.0, 1.0),
        ],
        Utterance::Answering => vec![n(0.0, 0.22, 7.0, 7.0, 0.9), n(0.32, 0.34, 7.0, 5.0, 0.85)],
        Utterance::Hurt => vec![
            Note {
                noise: 0.6,
                ..n(0.0, 0.05, 2.0, -1.0, 1.0)
            },
            Note {
                noise: 0.6,
                ..n(0.07, 0.05, 5.0, 1.0, 0.9)
            },
            Note {
                noise: 0.7,
                ..n(0.14, 0.08, 0.0, -4.0, 0.95)
            },
        ],
        Utterance::Dying => vec![Note {
            noise: 0.5,
            curve: 0.7,
            ..n(0.0, 1.5, 5.0, -24.0, 1.0)
        }],
    }
}

/// A kind's voice.
#[derive(Clone, Copy, Debug)]
pub struct Timbre {
    /// Pitch at 0 semitones, Hz.
    pub base: f32,
    /// Weights of harmonics 1-6.
    pub harmonics: [f32; 6],
    /// Centre of its formant, Hz, and the formant's Q.
    pub formant: f32,
    pub q: f32,
    /// How much of it is the formant (0: none, 1: all).
    pub colour: f32,
    /// Noise under every note, 0..1.
    pub noise: f32,
    /// Amplitude buzz (a rotor's chop, a growl), Hz and depth.
    pub buzz: f32,
    pub buzz_depth: f32,
    /// Time scale: 1 is the grammar's own pace, 0.6 quicker, 1.6 slower.
    pub pace: f32,
}

pub fn timbre(kind: VoiceKind) -> Timbre {
    const SAW: [f32; 6] = [1.0, 0.5, 0.33, 0.25, 0.2, 0.16];
    const HOLLOW: [f32; 6] = [1.0, 0.0, 0.33, 0.0, 0.2, 0.0];
    const THIN: [f32; 6] = [1.0, 0.15, 0.05, 0.0, 0.0, 0.0];
    match kind {
        VoiceKind::Drone => Timbre {
            base: 880.0,
            harmonics: THIN,
            formant: 2400.0,
            q: 2.0,
            colour: 0.3,
            noise: 0.04,
            buzz: 38.0,
            buzz_depth: 0.35,
            pace: 0.7,
        },
        VoiceKind::Walker => Timbre {
            base: 220.0,
            harmonics: HOLLOW,
            formant: 900.0,
            q: 3.0,
            colour: 0.6,
            noise: 0.03,
            buzz: 0.0,
            buzz_depth: 0.0,
            pace: 1.0,
        },
        VoiceKind::Grappler => Timbre {
            base: 185.0,
            harmonics: [1.0, 0.2, 0.5, 0.1, 0.4, 0.3],
            formant: 1500.0,
            q: 8.0,
            colour: 0.55,
            noise: 0.04,
            buzz: 0.0,
            buzz_depth: 0.0,
            pace: 1.1,
        },
        VoiceKind::Dirigible => Timbre {
            base: 98.0,
            harmonics: HOLLOW,
            formant: 420.0,
            q: 2.5,
            colour: 0.5,
            noise: 0.02,
            buzz: 0.0,
            buzz_depth: 0.0,
            pace: 1.7,
        },
        VoiceKind::Crawler => Timbre {
            base: 1500.0,
            harmonics: [1.0, 0.0, 0.3, 0.0, 0.0, 0.0],
            formant: 3200.0,
            q: 4.0,
            colour: 0.4,
            noise: 0.12,
            buzz: 90.0,
            buzz_depth: 0.6,
            pace: 0.55,
        },
        VoiceKind::Hound => Timbre {
            base: 300.0,
            harmonics: SAW,
            formant: 700.0,
            q: 2.0,
            colour: 0.6,
            noise: 0.25,
            buzz: 28.0,
            buzz_depth: 0.4,
            pace: 0.75,
        },
        VoiceKind::Warden => Timbre {
            base: 130.0,
            harmonics: SAW,
            formant: 600.0,
            q: 3.0,
            colour: 0.6,
            noise: 0.04,
            buzz: 0.0,
            buzz_depth: 0.0,
            pace: 1.3,
        },
    }
}

/// The loudness every render is levelled to: the RMS of its sounding part
/// (dBFS). The mix sets each cue's level from there.
pub const LEVEL_DBFS: f32 = -18.0;

/// One utterance, mono, at `sample_rate`. Deterministic: the same
/// arguments always give the same samples; variants differ a little in
/// pitch, timing and grain.
pub fn render(kind: VoiceKind, u: Utterance, variant: usize, sample_rate: f32) -> Vec<f32> {
    let seed = 0x5_0ce_u64 ^ (kind as u64) << 8 ^ (u as u64) << 16 ^ (variant as u64) << 24;
    let mut rng = Rng::new(seed);
    let t = timbre(kind);
    let shift = (rng.f32() - 0.5) * 3.0;
    let stretch = 1.0 + (rng.f32() - 0.5) * 0.2;
    let pace = t.pace * stretch;
    let notes = phrase(u);
    let end = notes.iter().map(|n| n.at + n.len).fold(0.0, f32::max) * pace;
    let tail = 0.06;
    let len = ((end + tail) * sample_rate).ceil() as usize;
    let mut out = vec![0.0f32; len];
    let mut bp = Bandpass::new(t.formant, t.q, sample_rate);
    let mut phase = 0.0f32;
    let mut buzz_phase = rng.f32();
    // Hurt throws the pitch about: one offset per note.
    let jitter: Vec<f32> = notes
        .iter()
        .map(|_| {
            if u == Utterance::Hurt {
                (rng.f32() - 0.5) * 6.0
            } else {
                0.0
            }
        })
        .collect();
    for (i, s) in out.iter_mut().enumerate() {
        let time = i as f32 / sample_rate;
        // The note sounding now (notes don't overlap in the grammar).
        let Some((k, note)) = notes.iter().enumerate().find(|(_, n)| {
            let a = n.at * pace;
            time >= a && time < a + n.len * pace + tail
        }) else {
            // Between notes the formant rings down on silence.
            *s = bp.run(0.0) * t.colour;
            continue;
        };
        let a = note.at * pace;
        let l = note.len * pace;
        let x = ((time - a) / l).clamp(0.0, 1.0);
        let glide = x.powf(note.curve);
        let semis = note.from + (note.to - note.from) * glide + shift + jitter[k];
        let f = t.base * 2f32.powf(semis / 12.0);
        // Dying slows its own buzz as it goes.
        let buzz_rate = if u == Utterance::Dying {
            (t.buzz.max(12.0)) * (1.0 - 0.8 * x)
        } else {
            t.buzz
        };
        phase = (phase + f / sample_rate).fract();
        buzz_phase = (buzz_phase + buzz_rate / sample_rate).fract();
        let mut tone = 0.0;
        for (h, w) in t.harmonics.iter().enumerate() {
            let hf = f * (h + 1) as f32;
            if *w > 0.0 && hf < sample_rate * 0.45 {
                tone += w * (TAU * phase * (h + 1) as f32).sin();
            }
        }
        let depth = if u == Utterance::Dying {
            t.buzz_depth.max(0.5) * x
        } else {
            t.buzz_depth
        };
        let buzz = 1.0 - depth * (0.5 + 0.5 * (TAU * buzz_phase).sin());
        // Noise only ever through the formant: a rasp in the voice, not
        // hiss that carries for half a kilometre.
        let noise = (rng.f32() * 2.0 - 1.0) * (t.noise + note.noise);
        let dry = tone * 0.35 * buzz;
        let coloured = bp.run(dry + noise * buzz);
        let body = dry * (1.0 - t.colour) + coloured * (t.colour * 2.0).max(1.0);
        *s = body * envelope(time - a, l, tail) * note.gain;
    }
    level(&mut out);
    out
}

/// A note's envelope: 6 ms in, held, out over its last fifth and the
/// tail.
fn envelope(t: f32, len: f32, tail: f32) -> f32 {
    let attack = 0.006;
    if t < 0.0 {
        return 0.0;
    }
    if t < attack {
        return t / attack;
    }
    let release = (len * 0.2).max(0.015) + tail;
    let start = len + tail - release;
    if t < start {
        1.0
    } else {
        (1.0 - (t - start) / release).clamp(0.0, 1.0).powi(2)
    }
}

/// Level a render to [`LEVEL_DBFS`] over its sounding part, peaks no
/// higher than -1 dBFS.
fn level(x: &mut [f32]) {
    let sounding: Vec<f32> = x.iter().copied().filter(|v| v.abs() > 1e-4).collect();
    if sounding.is_empty() {
        return;
    }
    let rms = (sounding.iter().map(|v| v * v).sum::<f32>() / sounding.len() as f32).sqrt();
    let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let want = 10f32.powf(LEVEL_DBFS / 20.0);
    let g = (want / rms.max(1e-9)).min(0.89 / peak.max(1e-9));
    for v in x.iter_mut() {
        *v *= g;
    }
}

/// A resonant band-pass (RBJ, constant 0 dB peak).
struct Bandpass {
    b0: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Bandpass {
    fn new(f: f32, q: f32, sr: f32) -> Self {
        let w = TAU * f.min(sr * 0.45) / sr;
        let alpha = w.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b2: -alpha / a0,
            a1: -2.0 * w.cos() / a0,
            a2: (1.0 - alpha) / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// How long an utterance lasts, s (for spacing them).
pub fn duration(kind: VoiceKind, u: Utterance) -> f32 {
    let end = phrase(u).iter().map(|n| n.at + n.len).fold(0.0, f32::max);
    end * timbre(kind).pace * 1.1 + 0.06
}

/// A robot's voice, from what it is: `None` for machines that don't
/// speak (pylons, sorters).
pub fn voice_of(
    kind: RobotKind,
    hound: bool,
    crawler: bool,
    grappler: bool,
    heavy: bool,
) -> Option<VoiceKind> {
    Some(match kind {
        RobotKind::Drone => VoiceKind::Drone,
        RobotKind::Dirigible => VoiceKind::Dirigible,
        RobotKind::Walker if hound => VoiceKind::Hound,
        RobotKind::Walker if crawler => VoiceKind::Crawler,
        RobotKind::Walker if grappler => VoiceKind::Grappler,
        RobotKind::Walker if heavy => VoiceKind::Warden,
        RobotKind::Walker => VoiceKind::Walker,
        RobotKind::Pylon | RobotKind::Sorter => return None,
    })
}

/// A robot's mind turned to something it would say, after `delay` s.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Turned {
    pub robot: Entity,
    pub said: Utterance,
    pub at: Vec3,
    pub delay: f32,
}

/// A robot said something: one cue.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Spoke {
    pub robot: Entity,
    pub kind: VoiceKind,
    pub said: Utterance,
    pub at: Vec3,
}

/// A robot doesn't say the same thing again within this, s...
pub const REPEAT: f32 = 4.0;
/// ...nor cry out hurt more than once in this.
pub const HURT_GAP: f32 = 1.0;
/// It mutters on its rounds only within this of a player, m, once every
/// [`IDLE_EVERY`] s or so.
pub const IDLE_NEAR: f32 = 70.0;
pub const IDLE_EVERY: (f32, f32) = (18.0, 40.0);
/// How many of those near a call answer it.
pub const ANSWERS: usize = 2;

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Turned>()
            .add_message::<Spoke>()
            .init_resource::<Voices>()
            .add_systems(
                FixedPostUpdate,
                (
                    (
                        from_messages,
                        from_brains,
                        from_hits,
                        answers,
                        responders,
                        mutter,
                    ),
                    speak,
                )
                    .chain(),
            );
    }
}

/// Per robot: what it last said and when, whether it's done speaking, and
/// when it next mutters; and what's waiting to be said.
#[derive(Resource, Default)]
pub struct Voices {
    last: HashMap<(Entity, Utterance), f32>,
    dead: std::collections::HashSet<Entity>,
    mutter_at: HashMap<Entity, f32>,
    pending: Vec<(f32, Turned)>,
    brains: HashMap<Entity, u8>,
    rng: Option<Rng>,
}

impl Voices {
    fn rng(&mut self) -> &mut Rng {
        self.rng.get_or_insert_with(|| Rng::new(0x701ce))
    }
}

type Kinds<'w, 's> = Query<
    'w,
    's,
    (
        &'static Robot,
        Has<crate::robots::hound::Hound>,
        Has<crate::robots::indoor::Crawler>,
        Has<crate::robots::grappler::Grappler>,
        Has<crate::robots::indoor::Heavy>,
    ),
>;

fn kind_of(robots: &Kinds, e: Entity) -> Option<VoiceKind> {
    let (r, h, c, g, w) = robots.get(e).ok()?;
    voice_of(r.kind, h, c, g, w)
}

/// The minds' own messages.
#[allow(clippy::too_many_arguments)]
fn from_messages(
    mut noticed: MessageReader<Noticed>,
    mut investigating: MessageReader<Investigating>,
    mut searching: MessageReader<Searching>,
    mut lost: MessageReader<LostYou>,
    mut called: MessageReader<CalledOut>,
    mut destroyed: MessageReader<PartDestroyed>,
    mut out: MessageWriter<Turned>,
) {
    let now = |robot, said, at| Turned {
        robot,
        said,
        at,
        delay: 0.0,
    };
    for n in noticed.read() {
        out.write(now(n.robot, Utterance::Noticed, n.at));
    }
    for i in investigating.read() {
        out.write(now(i.robot, Utterance::Investigating, i.at));
    }
    for s in searching.read() {
        out.write(now(s.robot, Utterance::Searching, s.at));
    }
    for l in lost.read() {
        out.write(now(l.robot, Utterance::Lost, l.at));
    }
    for c in called.read() {
        out.write(now(c.robot, Utterance::Calling, c.at));
    }
    for d in destroyed.read().filter(|d| d.role == PartRole::Core) {
        out.write(now(d.robot, Utterance::Dying, d.at));
    }
}

/// What the minds don't announce: turning to fight, and going down.
fn from_brains(
    brains: Query<(Entity, &Brain, &Robot, &GlobalTransform)>,
    mut voices: ResMut<Voices>,
    mut out: MessageWriter<Turned>,
) {
    for (e, brain, robot, gt) in &brains {
        let state = match brain {
            Brain::Engage => 1,
            Brain::Down => 2,
            _ => 0,
        };
        let before = voices.brains.insert(e, state);
        if before.is_none_or(|b| b == state) {
            continue;
        }
        let said = match state {
            1 => Utterance::Engaging,
            2 if robot.wrecked => Utterance::Dying,
            _ => continue,
        };
        out.write(Turned {
            robot: e,
            said,
            at: gt.translation(),
            delay: 0.0,
        });
    }
}

/// A part hit that it survives.
fn from_hits(
    mut hits: MessageReader<crate::weapons::ballistics::Hit>,
    parts: Query<&RobotPart>,
    mut out: MessageWriter<Turned>,
) {
    for h in hits.read() {
        let Ok(part) = parts.get(h.entity) else {
            continue;
        };
        if part.role == PartRole::Core && part.hp <= 0.0 {
            continue;
        }
        out.write(Turned {
            robot: part.robot,
            said: Utterance::Hurt,
            at: h.point,
            delay: 0.0,
        });
    }
}

/// The nearest that heard a call answer it, a beat apart.
fn answers(
    mut called: MessageReader<CalledOut>,
    robots: Query<(Entity, &Robot, &GlobalTransform)>,
    kinds: Kinds,
    mut out: MessageWriter<Turned>,
) {
    for c in called.read().filter(|c| c.heard > 0) {
        let mut near: Vec<(f32, Entity, Vec3)> = robots
            .iter()
            .filter(|(e, r, _)| *e != c.robot && !r.wrecked && kind_of(&kinds, *e).is_some())
            .map(|(e, _, gt)| (gt.translation().distance(c.at), e, gt.translation()))
            .filter(|(d, ..)| *d < crate::robots::search::CALL_RADIUS)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (i, (_, e, at)) in near.into_iter().take(ANSWERS).enumerate() {
            out.write(Turned {
                robot: e,
                said: Utterance::Answering,
                at,
                delay: 0.8 + 0.6 * i as f32,
            });
        }
    }
}

/// A response team answers as it sets out.
fn responders(
    added: Query<(Entity, &Job, &GlobalTransform), Added<Job>>,
    mut out: MessageWriter<Turned>,
) {
    for (e, job, gt) in &added {
        if job.kind == JobKind::Response {
            out.write(Turned {
                robot: e,
                said: Utterance::Answering,
                at: gt.translation(),
                delay: 0.5,
            });
        }
    }
}

/// On its rounds near someone, now and then.
fn mutter(
    robots: Query<(Entity, &Robot, &Brain, &GlobalTransform)>,
    players: Query<&GlobalTransform, With<Player>>,
    mut voices: ResMut<Voices>,
    time: Res<Time>,
    mut out: MessageWriter<Turned>,
) {
    let now = time.elapsed_secs();
    for (e, robot, brain, gt) in &robots {
        let idle = matches!(brain, Brain::Transit { .. } | Brain::Working { .. });
        let at = gt.translation();
        let near = players
            .iter()
            .any(|p| p.translation().distance(at) < IDLE_NEAR);
        if !idle || !near || robot.wrecked {
            voices.mutter_at.remove(&e);
            continue;
        }
        let (lo, hi) = IDLE_EVERY;
        let gap = lo + voices.rng().f32() * (hi - lo);
        let due = *voices.mutter_at.entry(e).or_insert(now + gap);
        if now >= due {
            voices.mutter_at.insert(e, now + gap);
            out.write(Turned {
                robot: e,
                said: Utterance::Idle,
                at,
                delay: 0.0,
            });
        }
    }
}

/// Each turn of a mind, said once.
pub fn speak(
    mut turned: MessageReader<Turned>,
    kinds: Kinds,
    places: Query<&GlobalTransform>,
    mut voices: ResMut<Voices>,
    time: Res<Time>,
    mut out: MessageWriter<Spoke>,
) {
    let now = time.elapsed_secs();
    for t in turned.read() {
        voices.pending.push((now + t.delay, *t));
    }
    let mut due: Vec<Turned> = Vec::new();
    voices.pending.retain(|(at, t)| {
        if *at <= now {
            due.push(*t);
            false
        } else {
            true
        }
    });
    // Going down outranks anything else it was about to say this tick.
    due.sort_by_key(|t| t.said != Utterance::Dying);
    for t in due {
        let Some(kind) = kind_of(&kinds, t.robot) else {
            continue;
        };
        if voices.dead.contains(&t.robot) {
            continue;
        }
        let gap = if t.said == Utterance::Hurt {
            HURT_GAP
        } else {
            REPEAT
        };
        if voices
            .last
            .get(&(t.robot, t.said))
            .is_some_and(|l| now - l < gap)
        {
            continue;
        }
        voices.last.insert((t.robot, t.said), now);
        if t.said == Utterance::Dying {
            voices.dead.insert(t.robot);
        }
        // A delayed answer comes from where it is by then.
        let at = if t.delay > 0.0 {
            places.get(t.robot).map_or(t.at, |g| g.translation())
        } else {
            t.at
        };
        out.write(Spoke {
            robot: t.robot,
            kind,
            said: t.said,
            at,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn rms(x: &[f32]) -> f32 {
        let s: Vec<f32> = x.iter().copied().filter(|v| v.abs() > 1e-4).collect();
        (s.iter().map(|v| v * v).sum::<f32>() / s.len().max(1) as f32).sqrt()
    }

    /// The pitch of a stretch, Hz: the shortest period whose
    /// autocorrelation is within 15% of the best (so not an octave
    /// below), 25-3000 Hz.
    fn pitch(x: &[f32]) -> f32 {
        // A glide smears a long window: the median of 80 ms ones.
        let w = (SR * 0.08) as usize;
        let mut ps: Vec<f32> = x.chunks_exact(w).map(pitch_of).collect();
        if ps.is_empty() {
            return pitch_of(x);
        }
        ps.sort_by(f32::total_cmp);
        ps[ps.len() / 2]
    }

    fn pitch_of(x: &[f32]) -> f32 {
        let r = |lag: usize| -> f32 {
            let (mut num, mut a, mut b) = (0.0f32, 0.0f32, 0.0f32);
            for i in 0..x.len() - lag {
                num += x[i] * x[i + lag];
                a += x[i] * x[i];
                b += x[i + lag] * x[i + lag];
            }
            num / (a * b).sqrt().max(1e-12)
        };
        let lags: Vec<usize> = ((SR / 3000.0) as usize..(SR / 25.0) as usize).collect();
        let rs: Vec<f32> = lags.iter().map(|&l| r(l)).collect();
        let best = rs.iter().copied().fold(f32::MIN, f32::max);
        // The first peak that comes near the best.
        let i = (1..rs.len() - 1)
            .find(|&i| rs[i] >= 0.85 * best && rs[i] >= rs[i - 1] && rs[i] >= rs[i + 1])
            .unwrap_or(0);
        SR / lags[i] as f32
    }

    /// A minimal world: the robots' messages, the voices, a fixed tick of
    /// 1/90 s.
    fn world() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f64(1.0 / 90.0),
            ))
            .add_message::<Noticed>()
            .add_message::<Investigating>()
            .add_message::<Searching>()
            .add_message::<LostYou>()
            .add_message::<CalledOut>()
            .add_message::<PartDestroyed>()
            .add_message::<crate::weapons::ballistics::Hit>()
            .add_plugins(VoicePlugin);
        app.update();
        app
    }

    fn robot(app: &mut App, kind: RobotKind, at: Vec3) -> Entity {
        app.world_mut()
            .spawn((
                Robot {
                    kind,
                    wrecked: false,
                },
                Brain::Transit { to: Vec3::ZERO },
                GlobalTransform::from_translation(at),
            ))
            .id()
    }

    /// Run ~`secs` and gather what was said.
    fn said(app: &mut App, secs: f32) -> Vec<Spoke> {
        let mut all = Vec::new();
        for _ in 0..(secs * 90.0) as usize {
            app.update();
            all.extend(app.world_mut().resource_mut::<Messages<Spoke>>().drain());
        }
        all
    }

    /// Each turn of a mind is said exactly once, by the right voice; the
    /// same turn again at once isn't repeated; nothing after dying.
    #[test]
    fn each_turn_is_one_cue() {
        let mut app = world();
        let w = robot(&mut app, RobotKind::Walker, Vec3::ZERO);
        let d = robot(&mut app, RobotKind::Drone, Vec3::X * 200.0);
        let at = Vec3::ZERO;
        let turns: Vec<(Entity, Utterance, Box<dyn Fn(&mut World)>)> = vec![
            (
                w,
                Utterance::Noticed,
                Box::new(move |wd: &mut World| {
                    wd.write_message(Noticed { robot: w, at });
                }),
            ),
            (
                w,
                Utterance::Investigating,
                Box::new(move |wd: &mut World| {
                    wd.write_message(Investigating {
                        robot: w,
                        at,
                        toward: at,
                    });
                }),
            ),
            (
                w,
                Utterance::Searching,
                Box::new(move |wd: &mut World| {
                    wd.write_message(Searching { robot: w, at });
                }),
            ),
            (
                w,
                Utterance::Lost,
                Box::new(move |wd: &mut World| {
                    wd.write_message(LostYou { robot: w, at });
                }),
            ),
            (
                d,
                Utterance::Calling,
                Box::new(move |wd: &mut World| {
                    wd.write_message(CalledOut {
                        robot: d,
                        at,
                        heard: 0,
                        about: None,
                    });
                }),
            ),
            (
                w,
                Utterance::Engaging,
                Box::new(move |wd: &mut World| {
                    *wd.get_mut::<Brain>(w).unwrap() = Brain::Engage;
                }),
            ),
        ];
        for (robot, u, act) in &turns {
            act(app.world_mut());
            let s = said(&mut app, 0.2);
            assert_eq!(s.len(), 1, "{u:?}: {s:?}");
            assert_eq!((s[0].robot, s[0].said), (*robot, *u));
            let want = if *robot == w {
                VoiceKind::Walker
            } else {
                VoiceKind::Drone
            };
            assert_eq!(s[0].kind, want);
            // The same again at once: nothing.
            if *u != Utterance::Engaging {
                act(app.world_mut());
                assert!(said(&mut app, 0.2).is_empty(), "{u:?} repeated");
            }
        }
        // Dying, once, and then silence.
        app.world_mut().get_mut::<Robot>(w).unwrap().wrecked = true;
        app.world_mut().write_message(PartDestroyed {
            robot: w,
            part: w,
            role: PartRole::Core,
            at,
        });
        *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Down;
        let s = said(&mut app, 0.2);
        assert_eq!(
            s.iter().map(|s| s.said).collect::<Vec<_>>(),
            [Utterance::Dying]
        );
        app.world_mut().write_message(Noticed { robot: w, at });
        assert!(said(&mut app, 0.2).is_empty());
    }

    /// A call heard is answered by the nearest two, a beat later; a
    /// response team answers as it sets out.
    #[test]
    fn calls_are_answered() {
        let mut app = world();
        let caller = robot(&mut app, RobotKind::Walker, Vec3::ZERO);
        let near = robot(&mut app, RobotKind::Drone, Vec3::X * 10.0);
        let nearer = robot(&mut app, RobotKind::Walker, Vec3::X * 5.0);
        let _far = robot(&mut app, RobotKind::Walker, Vec3::X * 300.0);
        let _third = robot(&mut app, RobotKind::Walker, Vec3::X * 30.0);
        app.world_mut().write_message(CalledOut {
            robot: caller,
            at: Vec3::ZERO,
            heard: 3,
            about: None,
        });
        let s = said(&mut app, 3.0);
        let order: Vec<_> = s.iter().map(|s| (s.robot, s.said)).collect();
        assert_eq!(
            order,
            [
                (caller, Utterance::Calling),
                (nearer, Utterance::Answering),
                (near, Utterance::Answering)
            ]
        );
        let team = app
            .world_mut()
            .spawn((
                Robot {
                    kind: RobotKind::Drone,
                    wrecked: false,
                },
                Job {
                    kind: JobKind::Response,
                    site: Vec3::ZERO,
                    duration: 30.0,
                    exit: Vec3::ZERO,
                },
                GlobalTransform::from_translation(Vec3::Z * 220.0),
            ))
            .id();
        let s = said(&mut app, 1.0);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].robot, s[0].said), (team, Utterance::Answering));
    }

    /// Pylons and sorters don't speak.
    #[test]
    fn some_machines_are_mute() {
        assert_eq!(voice_of(RobotKind::Pylon, false, false, false, false), None);
        assert_eq!(
            voice_of(RobotKind::Sorter, false, false, false, false),
            None
        );
        assert_eq!(
            voice_of(RobotKind::Walker, true, false, false, false),
            Some(VoiceKind::Hound)
        );
    }

    /// Same arguments, same samples; another variant, others.
    #[test]
    fn deterministic_and_varied() {
        let a = render(VoiceKind::Walker, Utterance::Noticed, 0, SR);
        let b = render(VoiceKind::Walker, Utterance::Noticed, 0, SR);
        let c = render(VoiceKind::Walker, Utterance::Noticed, 1, SR);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    /// Every utterance of every kind is sound, short, levelled and never
    /// clips.
    #[test]
    fn every_word_is_said() {
        for k in VoiceKind::ALL {
            for u in Utterance::ALL {
                let x = render(k, u, 0, SR);
                let secs = x.len() as f32 / SR;
                assert!((0.1..=3.0).contains(&secs), "{k:?} {u:?}: {secs} s");
                let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                assert!(peak <= 0.9 && peak > 0.05, "{k:?} {u:?}: peak {peak}");
                let db = 20.0 * rms(&x).log10();
                assert!(
                    (LEVEL_DBFS - 6.0..=LEVEL_DBFS + 0.5).contains(&db),
                    "{k:?} {u:?}: {db}"
                );
                assert!(x.iter().all(|v| v.is_finite()));
            }
        }
    }

    /// A render with its noise and formant taken off: four one-poles at
    /// 2.5 x the voice's base, so the fundamental is what's left.
    fn tone(k: VoiceKind, u: Utterance) -> Vec<f32> {
        let mut x = render(k, u, 0, SR);
        let a = 1.0 - (-TAU * 2.5 * timbre(k).base / SR).exp();
        for _ in 0..4 {
            let mut y = 0.0;
            for v in x.iter_mut() {
                y += a * (*v - y);
                *v = y;
            }
        }
        x
    }

    /// The grammar holds in every voice: a question rises, giving up and
    /// dying fall.
    #[test]
    fn one_grammar() {
        for k in VoiceKind::ALL {
            let q = tone(k, Utterance::Noticed);
            let (a, b) = (q.len() / 6, q.len() / 3);
            let (c, d) = (q.len() * 3 / 5, q.len() * 4 / 5);
            assert!(
                pitch(&q[c..d]) > pitch(&q[a..b]),
                "{k:?}: the question doesn't rise"
            );
            for u in [Utterance::Lost, Utterance::Dying] {
                let x = tone(k, u);
                let (a, b) = (x.len() / 10, x.len() / 4);
                let (c, d) = (x.len() / 2, x.len() * 7 / 10);
                assert!(
                    pitch(&x[c..d]) < pitch(&x[a..b]),
                    "{k:?} {u:?} doesn't fall"
                );
            }
        }
    }

    /// Kinds are told apart by pitch: the drone high, the dirigible low.
    #[test]
    fn kinds_differ() {
        let p = |k| pitch(&tone(k, Utterance::Calling));
        assert!(p(VoiceKind::Drone) > 2.0 * p(VoiceKind::Walker));
        assert!(p(VoiceKind::Walker) > 1.5 * p(VoiceKind::Dirigible));
        assert!(p(VoiceKind::Crawler) > p(VoiceKind::Drone));
    }

    /// The renders for `scripts/audio_voices.py`: every utterance of every
    /// kind, at 48 kHz.
    #[test]
    fn renders_for_the_audit() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/audio_audit/voices");
        std::fs::create_dir_all(&dir).unwrap();
        for k in VoiceKind::ALL {
            for u in Utterance::ALL {
                for v in 0..VARIANTS {
                    let x = render(k, u, v, SR);
                    let path = dir.join(format!("{}_{}_{v}.wav", k.name(), u.name()));
                    write_wav(&path, &x, SR as u32);
                }
            }
        }
    }

    fn write_wav(path: &std::path::Path, x: &[f32], sr: u32) {
        let mut b = Vec::with_capacity(44 + x.len() * 2);
        let data = (x.len() * 2) as u32;
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&sr.to_le_bytes());
        b.extend_from_slice(&(sr * 2).to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data.to_le_bytes());
        for v in x {
            b.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        std::fs::write(path, b).unwrap();
    }
}
