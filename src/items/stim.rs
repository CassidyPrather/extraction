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
        app.init_resource::<WorldPace>()
            .add_systems(First, stim_clock);
    }
}

/// Counts the stim down in real seconds and eases the world's pace to
/// [`SLOW`] and back; the stim's done once the pace is home.
fn stim_clock(
    mut commands: Commands,
    mut players: Query<(Entity, &mut Stimmed), With<LocalPlayer>>,
    mut virt: ResMut<Time<Virtual>>,
    mut pace: ResMut<WorldPace>,
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
        s.left = (s.left - real).max(0.0);
        if s.left <= 0.0 && pace.0 >= 1.0 {
            commands.entity(e).remove::<Stimmed>();
        }
    }
    if (virt.relative_speed() - pace.0).abs() > 1e-4 {
        virt.set_relative_speed(pace.0);
    }
}
