//! The spotter's rules: a balloon on a tether that watches a valley and
//! calls the others in. (The machine itself is `robots::spotter` in the
//! game.)
//!
//! It's an alarm, not a gun:
//!
//! - **It turns a slow circle**, its eye pitched down ([`pan_yaw`]): a
//!   full turn every [`PAN_PERIOD`] seconds, so any one way is watched a
//!   little over a quarter of the time.
//! - **Straight under it is blind.** Its eye looks out level with a
//!   [`FOV`]-wide view: within [`blind_radius`] of the winch, below the
//!   bottom of its view, it can't see you at all.
//! - **It takes its time** ([`Watch`]): seen, its fix builds over [`FIX`]
//!   seconds while its beacon blinks faster and faster; break its sight
//!   and the fix drains. Only a full fix calls the others.
//! - **It's on a string.** Cut the tether at the winch and it rises and
//!   drifts off downwind ([`loose_offset`]); hole the envelope and it sinks
//!   to the ground ([`SINK`]), a wreck to pick over.

/// Height of the envelope's centre over the winch, m.
pub const TETHER: f32 = 24.0;
/// How far it wanders on its tether in the wind, m.
pub const SWAY: f32 = 2.0;
/// Seconds for a full turn of its circle.
pub const PAN_PERIOD: f32 = 40.0;
/// Its field of view, degrees (round its level eye).
pub const FOV: f32 = 100.0;
/// How far it sees by day, m.
pub const SIGHT: f32 = 85.0;

/// Seconds of being seen for a full fix; a fix drains at [`DRAIN`] of
/// that rate while it can't see you.
pub const FIX: f32 = 3.5;
pub const DRAIN: f32 = 0.5;
/// After a call, seconds before it calls again.
pub const COOLDOWN: f32 = 150.0;
/// Seconds before the response it calls sets out.
pub const RESPONSE_DELAY: f32 = 15.0;
/// How far its klaxon carries, m (robots near enough come to look).
pub const KLAXON: f32 = 140.0;

/// Cut loose: how fast it rises, the wind it drifts on (m/s), and how long
/// before it's gone.
pub const RISE: f32 = 1.4;
pub const WIND: [f32; 2] = [1.8, 0.7];
pub const GONE_AFTER: f32 = 50.0;
/// Holed: how fast it sinks, m/s.
pub const SINK: f32 = 1.1;

/// Where its eye points `t` seconds in (degrees, the game's yaw: 0 toward
/// -Z, 90 toward +X), from where it started (`phase`).
pub fn pan_yaw(t: f32, phase: f32) -> f32 {
    let a = (phase + t / PAN_PERIOD * 360.0).rem_euclid(360.0);
    if a > 180.0 { a - 360.0 } else { a }
}

/// Its wander on the tether at `t` (x, z offsets, m): slow, never the same
/// twice round, different for each balloon (`seed`).
pub fn sway(t: f32, seed: u64) -> [f32; 2] {
    let p = (seed % 1000) as f32 * 0.006_283;
    let q = ((seed / 1000) % 1000) as f32 * 0.006_283;
    [
        SWAY * (0.7 * (t * 0.11 + p).sin() + 0.3 * (t * 0.29 + q).sin()),
        SWAY * (0.7 * (t * 0.083 + q).cos() + 0.3 * (t * 0.23 + p).sin()),
    ]
}

/// How far out from under it the ground is out of its sight (m): below the
/// bottom edge of its view, at a height of `height`.
pub fn blind_radius(height: f32) -> f32 {
    let lowest = (FOV * 0.5).min(89.0).to_radians();
    height / lowest.tan()
}

/// Where it is `t` seconds after it was cut loose, from where it was.
pub fn loose_offset(t: f32) -> [f32; 3] {
    [WIND[0] * t, RISE * t, WIND[1] * t]
}

/// The fix it's building on you.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Watch {
    /// 0..1.
    pub fix: f32,
    /// Seconds before it can call again.
    pub cooldown: f32,
}

/// What its watch does this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchAct {
    Quiet,
    /// It's building a fix (its beacon quickens).
    Fixing,
    /// A full fix: the klaxon, and the others are called.
    Call,
}

impl Watch {
    /// One tick: `sees` whether it can see you.
    pub fn step(&mut self, sees: bool, dt: f32) -> WatchAct {
        self.cooldown = (self.cooldown - dt).max(0.0);
        if sees {
            self.fix = (self.fix + dt / FIX).min(1.0);
        } else {
            self.fix = (self.fix - dt * DRAIN / FIX).max(0.0);
        }
        if self.fix >= 1.0 && self.cooldown <= 0.0 {
            self.cooldown = COOLDOWN;
            self.fix = 0.0;
            return WatchAct::Call;
        }
        if self.fix > 0.0 && sees {
            WatchAct::Fixing
        } else {
            WatchAct::Quiet
        }
    }

    /// Seconds between beacon blinks: slow at rest, quick near a fix.
    pub fn blink(&self) -> f32 {
        1.6 - 1.4 * self.fix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// It turns a full circle in its period, steadily.
    #[test]
    fn it_turns_a_slow_circle() {
        assert!((pan_yaw(0.0, 30.0) - 30.0).abs() < 1e-4);
        assert!((pan_yaw(PAN_PERIOD, 30.0) - 30.0).abs() < 1e-3);
        assert!((pan_yaw(PAN_PERIOD * 0.5, 0.0).abs() - 180.0).abs() < 1e-3);
        // Any one way is in view a little over a quarter of the time.
        let share = FOV / 360.0;
        assert!(share > 0.25 && share < 0.33, "{share}");
    }

    /// Straight under it is blind, and the blind ground is big enough to
    /// walk to the winch through.
    #[test]
    fn under_it_is_blind() {
        let r = blind_radius(TETHER);
        assert!(r > 12.0 && r < 25.0, "{r}");
    }

    /// A fix builds over its time and calls once; a glimpse doesn't; out of
    /// sight it drains; after a call it waits.
    #[test]
    fn it_takes_its_time() {
        let dt = 0.05;
        let mut w = Watch::default();
        // A second's glimpse, then gone: no call, and it drains away.
        for _ in 0..20 {
            assert_ne!(w.step(true, dt), WatchAct::Call);
        }
        assert!(w.fix > 0.2);
        for _ in 0..400 {
            w.step(false, dt);
        }
        assert_eq!(w.fix, 0.0);
        // Held in view: a call after about FIX seconds.
        let mut t = 0.0;
        let mut called = None;
        for _ in 0..200 {
            t += dt;
            if w.step(true, dt) == WatchAct::Call {
                called = Some(t);
                break;
            }
        }
        let at = called.expect("it called");
        assert!((at - FIX).abs() < 0.2, "called at {at}");
        // It doesn't call again for a while, however long it sees you.
        for _ in 0..((COOLDOWN - 10.0) / dt) as usize {
            assert_ne!(w.step(true, dt), WatchAct::Call);
        }
        // Its beacon quickens as the fix builds.
        let rest = Watch::default().blink();
        let near = Watch {
            fix: 0.9,
            cooldown: 0.0,
        }
        .blink();
        assert!(near < rest * 0.4);
    }

    /// It sways on its tether, and two balloons sway differently.
    #[test]
    fn it_sways() {
        let a: Vec<[f32; 2]> = (0..100).map(|k| sway(k as f32, 7)).collect();
        let b: Vec<[f32; 2]> = (0..100).map(|k| sway(k as f32, 123_456)).collect();
        assert!(a.iter().all(|p| p[0].abs() <= SWAY && p[1].abs() <= SWAY));
        assert!(a != b);
        // Cut loose, it goes up and away.
        let o = loose_offset(GONE_AFTER);
        assert!(o[1] > 50.0 && (o[0] * o[0] + o[2] * o[2]).sqrt() > 50.0);
    }
}
