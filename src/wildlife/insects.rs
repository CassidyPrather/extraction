//! The small life you hear rather than see: crickets in the grass, flies,
//! the odd small bird. It goes quiet near danger, the way a wood does, and
//! that's information: when the insects stop, something is close.
//!
//! [`Chorus`] is how loud it is round the local player, 0 (dead quiet) to
//! 1, for sound to play (Audio's night and day beds). It's the product of:
//! - **machines near**: silent within [`HUSH_NEAR`] of a walker or hound,
//!   [`HUSH_NEAR_DRONE`] of a drone and [`HUSH_NEAR_DIRIGIBLE`] of the
//!   airship, back to full by [`HUSH_RAMP`] times that;
//! - **a fright** (a shot or a blast heard, a flock going up or a herd
//!   bolting near you): it drops to [`STARTLED`] at once and comes back
//!   over [`RECOVER`] seconds;
//! - **the weather**: rain and wind thin it, a downpour stops it.
//!
//! [`Chorus::kind`] says what's singing: crickets at dusk and night, flies
//! and small birds by day.

use bevy::prelude::*;

use super::{Flushed, ThreatKind, Threats};

/// A walker or hound this near and the chorus is silent...
/// (Audio's masking audit, `scripts/audio_voices.py`: over the cricket
/// bed a walker's step is heard to ~46 m against a 60 m warning target,
/// so the hush must reach that far; it reads these.)
pub const HUSH_NEAR: f32 = 60.0;
/// ...a drone this near (its rotor heard to ~71 m, the target 120 m)...
pub const HUSH_NEAR_DRONE: f32 = 120.0;
/// ...the airship's engines this near (it sees you from 170 m)...
pub const HUSH_NEAR_DIRIGIBLE: f32 = 150.0;
/// ...and each hushes it less out to this many times as far (far enough
/// that it's thinning well before the machine can see you: `warning`).
pub const HUSH_RAMP: f32 = 2.0;
/// A fright drops it to this...
pub const STARTLED: f32 = 0.05;
/// ...and it takes this long to come back (s).
pub const RECOVER: f32 = 60.0;
/// A fright (a shot, a flush, a bolt) counts this near you (m).
pub const FRIGHT_NEAR: f32 = 150.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Singers {
    #[default]
    Day,
    /// Crickets: dusk and night.
    Crickets,
}

/// The chorus round the local player.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Chorus {
    /// 0..1, smoothed: what sound should play.
    pub level: f32,
    pub kind: Singers,
    /// The fright factor, 0..1, recovering.
    pub fright: f32,
}

impl Default for Chorus {
    fn default() -> Self {
        Self {
            level: 1.0,
            kind: Singers::Day,
            fright: 1.0,
        }
    }
}

/// How much of the chorus a machine this far off leaves (`near`: its
/// silent radius, [`HUSH_NEAR`] or [`HUSH_NEAR_DRONE`]).
pub fn hush(distance: f32, near: f32) -> f32 {
    ((distance - near) / (near * (HUSH_RAMP - 1.0))).clamp(0.0, 1.0)
}

/// How much of the chorus the weather leaves: wind thins it, a drizzle
/// barely, a downpour stops it.
pub fn weather(c: &crate::world::weather::Conditions) -> f32 {
    (1.0 - c.rain * c.rain).max(0.0) * (1.0 - 0.6 * c.wind())
}

/// A machine of this kind hushes the chorus within this.
pub fn hush_near(kind: ThreatKind) -> f32 {
    match kind {
        ThreatKind::Drone => HUSH_NEAR_DRONE,
        ThreatKind::Dirigible => HUSH_NEAR_DIRIGIBLE,
        _ => HUSH_NEAR,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn chorus(
    mut chorus: ResMut<Chorus>,
    threats: Res<Threats>,
    me: Query<&GlobalTransform, With<crate::rig::LocalPlayer>>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    mut noises: MessageReader<crate::robots::Noise>,
    mut flushed: MessageReader<Flushed>,
    mut startled: MessageReader<super::herd::Startled>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let Ok(me) = me.single() else {
        noises.clear();
        flushed.clear();
        startled.clear();
        return;
    };
    let at = me.translation();
    let near = |p: Vec3| p.distance(at) < FRIGHT_NEAR;
    let fright = noises
        .read()
        .any(|n| n.radius >= super::flock::LOUD && near(n.at))
        | flushed.read().any(|f| near(f.at))
        | startled.read().any(|s| near(s.at));
    if fright {
        chorus.fright = STARTLED;
    } else {
        chorus.fright = (chorus.fright + dt / RECOVER).min(1.0);
    }
    let machines = threats
        .0
        .iter()
        .filter_map(|t| match t.kind {
            ThreatKind::Player { .. } => None,
            kind => Some(hush(t.at.distance(at), hush_near(kind))),
        })
        .fold(1.0f32, f32::min);
    let (weather, kind) = conditions.as_ref().map_or((1.0, Singers::Day), |c| {
        use crate::world::weather::Hour;
        let kind = match c.hour {
            Hour::Dusk | Hour::Night => Singers::Crickets,
            _ => Singers::Day,
        };
        (weather(c), kind)
    });
    let target = machines * chorus.fright * weather;
    // Falls fast, comes back slowly.
    let k = if target < chorus.level { 3.0 } else { 0.4 };
    chorus.level += (target - chorus.level) * (dt * k).min(1.0);
    chorus.kind = kind;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machines_hush_it() {
        // Silent out to Audio's masking radii, a ramp beyond.
        assert_eq!(hush(59.0, HUSH_NEAR), 0.0);
        assert_eq!(hush(119.0, HUSH_NEAR_DRONE), 0.0);
        assert!(hush(80.0, HUSH_NEAR) > 0.0 && hush(80.0, HUSH_NEAR) < 1.0);
        assert_eq!(hush(HUSH_NEAR * HUSH_RAMP, HUSH_NEAR), 1.0);
        assert!(HUSH_NEAR >= 60.0 && HUSH_NEAR_DRONE >= 120.0 && FRIGHT_NEAR >= 150.0);
    }
}
