//! The stim (#102): a panic button. While the local player is
//! [`Stimmed`], the world runs at [`SLOW`]: the virtual clock everything
//! in the world keeps (physics, machines, rounds in flight, anomalies,
//! timers). The head is tracked, so it never slows; the player's own walk
//! and hands are given back what the clock takes ([`WorldPace`]: their
//! speeds over the pace), so they keep real time.
//!
//! The stim's length is in real seconds: what's left is counted down by
//! the virtual clock's step over the pace (the same thing, and exact in a
//! headless run, whose real clock is its steps). The pace eases in and
//! out over [`EASE`] real seconds rather than jumping.
//!
//! A later timestopper is the same clock at 0.

use bevy::prelude::*;

use super::pen::Stimmed;
use crate::rig::LocalPlayer;

/// The world's pace in a stim.
pub const SLOW: f32 = 0.4;
/// How long (real s) the pace takes to go down, and to come back.
pub const EASE: f32 = 0.3;

/// The stim's last seconds (real) are its tell: the heart quickens.
pub const TELL: f32 = 2.0;
/// The heart's beat in a stim (real s apart), slow and heavy; and in its
/// tell, quick.
pub const BEAT_SLOW: f32 = 1.2;
pub const BEAT_TELL: f32 = 0.5;

/// A stim starting, nearly over, and over (for its sounds: Audio's).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StimCue {
    In,
    Tell,
    Out,
}

/// The world's pace now (1: as normal; [`SLOW`] in a stim): what the
/// player's own motion divides by to keep real time.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct WorldPace(pub f32);

impl Default for WorldPace {
    fn default() -> Self {
        Self(1.0)
    }
}

impl WorldPace {
    /// What a speed of the player's own is multiplied by to keep real
    /// time while the world is slowed.
    pub fn own(&self) -> f32 {
        1.0 / self.0.max(0.05)
    }
}

pub struct StimPlugin;

impl Plugin for StimPlugin {
    fn build(&self, app: &mut App) {
        // The heart's one message, shared with its sound and the hands'
        // feel (`audio_kit::HeartbeatPlugin`), added once.
        if !app.is_plugin_added::<crate::audio_kit::HeartbeatPlugin>() {
            app.add_plugins(crate::audio_kit::HeartbeatPlugin);
        }
        app.init_resource::<WorldPace>()
            .add_message::<StimCue>()
            .add_systems(First, stim_clock)
            .add_systems(
                PostUpdate,
                held_on_real_clock.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

/// Where a stim is, for its cues: on, told it's ending, and the real
/// seconds since its heart last beat.
#[derive(Default)]
struct Felt {
    on: bool,
    told: bool,
    since_beat: f32,
}

/// Counts the stim down in real seconds and eases the world's pace to
/// [`SLOW`] and back; the stim's done once the pace is home. Felt as it
/// goes: [`StimCue`]s, and the heart (`audio_kit::Heartbeat`, heard and
/// felt in both hands) slow and heavy, quick in the [`TELL`].
#[allow(clippy::too_many_arguments)]
fn stim_clock(
    mut commands: Commands,
    mut players: Query<(Entity, &mut Stimmed), With<LocalPlayer>>,
    mut virt: ResMut<Time<Virtual>>,
    mut pace: ResMut<WorldPace>,
    mut cues: MessageWriter<StimCue>,
    mut beats: MessageWriter<crate::audio_kit::Heartbeat>,
    mut felt: Local<Felt>,
) {
    // Real seconds since the last frame: the virtual step over the pace
    // it was taken at.
    let real = virt.delta_secs() / virt.relative_speed().max(0.05);
    let stim = players.iter_mut().next();
    // Nothing to do outside a stim (and the clock left alone: capture and
    // screenshots pause it).
    if stim.is_none() && pace.0 >= 1.0 {
        return;
    }
    let want = match &stim {
        Some((_, s)) if s.left > 0.0 => SLOW,
        _ => 1.0,
    };
    let step = (1.0 - SLOW) * real / EASE;
    pace.0 = pace.0 + (want - pace.0).clamp(-step, step);
    if let Some((e, mut s)) = stim {
        if !felt.on && s.left > 0.0 {
            *felt = Felt {
                on: true,
                told: false,
                // A beat at once.
                since_beat: BEAT_SLOW,
            };
            cues.write(StimCue::In);
        }
        s.left = (s.left - real).max(0.0);
        if s.left > 0.0 {
            let tell = s.left <= TELL;
            if tell && !felt.told {
                felt.told = true;
                cues.write(StimCue::Tell);
            }
            felt.since_beat += real;
            let (period, level) = if tell {
                (BEAT_TELL, 0.7)
            } else {
                (BEAT_SLOW, 1.0)
            };
            if felt.since_beat >= period {
                felt.since_beat = 0.0;
                beats.write(crate::audio_kit::Heartbeat { level });
            }
        }
        if s.left <= 0.0 && pace.0 >= 1.0 {
            commands.entity(e).remove::<Stimmed>();
            if felt.on {
                cues.write(StimCue::Out);
            }
            *felt = Felt::default();
        }
    }
    if (virt.relative_speed() - pace.0).abs() > 1e-4 {
        virt.set_relative_speed(pace.0);
        crate::audio_mix::set_world_pace(pace.0);
    }
}

/// In a stim the world's ticks come less often than frames, so a held
/// thing chased on them would judder in the hand: it's put where the hand
/// holds it every frame instead, as the glove is (`physics_hands`). Held in
/// one hand only; one braced by the other hand, a bulky thing and a drag
/// keep their own rules.
#[allow(clippy::type_complexity)]
fn held_on_real_clock(
    pace: Res<WorldPace>,
    hands: Query<&crate::interact::Hand>,
    mut held: Query<
        (&crate::interact::Held, &mut Transform),
        (
            Without<crate::interact::Supported>,
            Without<crate::interact::carry::Bulky>,
            Without<crate::interact::carry::Draggable>,
            Without<crate::interact::body::Backpack>,
        ),
    >,
) {
    if pace.0 >= 1.0 {
        return;
    }
    for (h, mut t) in &mut held {
        let Ok(hand) = hands.get(h.hand) else {
            continue;
        };
        let want = hand.world.mul_transform(h.offset);
        if t.translation != want.translation || t.rotation != want.rotation {
            t.translation = want.translation;
            t.rotation = want.rotation;
        }
    }
}
