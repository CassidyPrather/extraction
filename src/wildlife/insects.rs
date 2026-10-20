//! The small life you hear rather than see: crickets in the grass, flies,
//! the odd small bird. It goes quiet near danger, the way a wood does, and
//! that's information: when the insects stop, something is close.
//!
//! [`Chorus`] is how loud it is round the local player, 0 (dead quiet) to
//! 1, for sound to play (Audio's night and day beds). It's the product of:
//! - **machines near** ([`HUSH_NEAR`] to [`HUSH_FAR`]): quiet within the
//!   near distance, full beyond the far one;
//! - **a fright** (a shot or a blast heard, a flock going up or a herd
//!   bolting near you): it drops to [`STARTLED`] at once and comes back
//!   over [`RECOVER`] seconds;
//! - **the weather**: rain and wind thin it, a downpour stops it.
//!
//! [`Chorus::kind`] says what's singing: crickets at dusk and night, flies
//! and small birds by day.

use bevy::prelude::*;

use super::{Flushed, ThreatKind, Threats};

/// Machines this near and the chorus is silent...
pub const HUSH_NEAR: f32 = 25.0;
/// ...beyond this they don't hush it at all.
pub const HUSH_FAR: f32 = 70.0;
/// A fright drops it to this...
pub const STARTLED: f32 = 0.05;
/// ...and it takes this long to come back (s).
pub const RECOVER: f32 = 60.0;
/// A fright counts this near you (m).
pub const FRIGHT_NEAR: f32 = 120.0;

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

/// How much machines at these distances leave of the chorus.
pub fn hush(nearest_machine: f32) -> f32 {
    ((nearest_machine - HUSH_NEAR) / (HUSH_FAR - HUSH_NEAR)).clamp(0.0, 1.0)
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
    let machine = threats
        .0
        .iter()
        .filter(|t| !matches!(t.kind, ThreatKind::Player { .. }))
        .map(|t| t.at.distance(at))
        .fold(f32::MAX, f32::min);
    let (weather, kind) = conditions.as_ref().map_or((1.0, Singers::Day), |c| {
        use crate::world::weather::Hour;
        let kind = match c.hour {
            Hour::Dusk | Hour::Night => Singers::Crickets,
            _ => Singers::Day,
        };
        ((1.0 - c.rain).max(0.0) * (1.0 - 0.6 * c.wind()), kind)
    });
    let target = hush(machine) * chorus.fright * weather;
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
        assert_eq!(hush(10.0), 0.0);
        assert_eq!(hush(f32::MAX), 1.0);
        assert!(hush(50.0) > 0.0 && hush(50.0) < 1.0);
    }
}
