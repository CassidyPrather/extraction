//! The sentry's rules: how it sweeps, where it can never look, how it
//! warns before it fires, and what draws it off. (The machine itself is
//! `robots::sentry` in the game.)
//!
//! A sentry is a turret head on a mast, guarding a fixed approach. It is
//! built to be outwitted, not fought:
//!
//! - **It sweeps a fixed arc**, back and forth at a steady pace with a
//!   pause at each end ([`sweep_yaw`]): watch it and you learn its rhythm.
//! - **Behind it is blind.** Its head can't turn past its arc
//!   ([`clamp_to_arc`]), so the sector behind it is never seen
//!   ([`can_ever_see`]), even once it has you: get round behind it and it
//!   can only stare at the edge of its arc.
//! - **It warns before it fires**: a chirp, then [`CHIRP`] seconds before
//!   the first round of a burst; lose it a while and it has to chirp
//!   again ([`Gun`]).
//! - **A noise or a hot thing draws it**: it swings to look and holds
//!   there for [`DECOY_HOLD`] — a thrown can is a window.
//! - **It runs on a cable** to a junction box: cut the cable or wreck the
//!   box and it's dead.

/// Degrees a second it sweeps at, and the pause at each end of its arc.
pub const SWEEP_SPEED: f32 = 24.0;
pub const DWELL: f32 = 1.5;
/// Degrees a second it slews to track something.
pub const TRACK_SPEED: f32 = 70.0;
/// Its eye's field of view, degrees (narrow: a turret, not an orb).
pub const FOV: f32 = 50.0;
/// How far it sees by day, m (its searchlight's reach at night).
pub const SIGHT: f32 = 55.0;
pub const BEAM_REACH: f32 = 45.0;
/// The searchlight's cone, half-angle in degrees.
pub const BEAM_HALF: f32 = 9.0;

/// The warning: seconds from the chirp to the first round (about two
/// metres at a walk to get out of its sight).
pub const CHIRP: f32 = 1.6;
/// Rounds in a burst, seconds between them, and between bursts.
pub const BURST: u32 = 3;
pub const ROUND_GAP: f32 = 0.12;
pub const BURST_GAP: f32 = 1.4;
/// Out of its sight this long and it has to chirp again.
pub const LOSE_LOCK: f32 = 2.0;

/// A noise (a thrown can landing) heard within this draws it; it holds on
/// it this long.
pub const DECOY_RANGE: f32 = 35.0;
pub const DECOY_HOLD: f32 = 5.0;

/// Seconds between its reports to the director.
pub const REPORT_EVERY: f32 = 90.0;

/// An angle brought into -180..180 degrees.
pub fn wrap_deg(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

/// Where its head points `t` seconds into its sweep (degrees, absolute):
/// from one end of its arc to the other at [`SWEEP_SPEED`], a pause of
/// [`DWELL`] at each end, and back. `centre` is the middle of the arc,
/// `arc` its full width.
pub fn sweep_yaw(t: f32, centre: f32, arc: f32) -> f32 {
    let half = (arc * 0.5).max(0.0);
    if half < 1e-3 {
        return centre;
    }
    let travel = arc / SWEEP_SPEED;
    let period = 2.0 * (travel + DWELL);
    let p = t.rem_euclid(period);
    let off = if p < travel {
        -half + p * SWEEP_SPEED
    } else if p < travel + DWELL {
        half
    } else if p < 2.0 * travel + DWELL {
        half - (p - travel - DWELL) * SWEEP_SPEED
    } else {
        -half
    };
    wrap_deg(centre + off)
}

/// A wanted heading, held inside its arc: its head doesn't turn further.
pub fn clamp_to_arc(want: f32, centre: f32, arc: f32) -> f32 {
    let off = wrap_deg(want - centre).clamp(-arc * 0.5, arc * 0.5);
    wrap_deg(centre + off)
}

/// One tick of slewing from `yaw` toward `want` at [`TRACK_SPEED`], kept
/// inside its arc.
/// It turns through its arc, never across the blind back.
pub fn slew(yaw: f32, want: f32, centre: f32, arc: f32, dt: f32) -> f32 {
    let half = arc * 0.5;
    let at = wrap_deg(yaw - centre).clamp(-half, half);
    let to = wrap_deg(want - centre).clamp(-half, half);
    let step = TRACK_SPEED * dt;
    wrap_deg(centre + at + (to - at).clamp(-step, step))
}

/// Whether anything in direction `bearing` (degrees) can ever be seen: its
/// head at the end of its arc still has half its field of view past it.
pub fn can_ever_see(bearing: f32, centre: f32, arc: f32) -> bool {
    wrap_deg(bearing - centre).abs() <= arc * 0.5 + FOV * 0.5
}

/// Whether `bearing` is in view with its head at `yaw`.
pub fn in_view(bearing: f32, yaw: f32) -> bool {
    wrap_deg(bearing - yaw).abs() <= FOV * 0.5
}

/// The bearing (degrees, 0 toward -Z, positive toward +X: the game's yaw)
/// from `from` to `to`, on the ground plane.
pub fn bearing(from: [f32; 2], to: [f32; 2]) -> f32 {
    let (dx, dz) = (to[0] - from[0], to[1] - from[1]);
    dx.atan2(-dz).to_degrees()
}

/// The gun: a chirp, then bursts, until it loses you.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gun {
    /// Seconds since the chirp (None: not warned yet).
    pub warned: Option<f32>,
    /// Rounds left in this burst, and seconds to the next round.
    pub left: u32,
    pub next: f32,
    /// Seconds without sight of its target.
    pub unseen: f32,
    /// Bursts fired since the chirp: the first is a warning ([`Gun::warning`]).
    pub bursts: u32,
}

/// What the gun does this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GunAct {
    Quiet,
    /// The warning: it has you.
    Chirp,
    Fire,
}

impl Gun {
    /// One tick: `sees` whether its target is in view.
    pub fn step(&mut self, sees: bool, dt: f32) -> GunAct {
        if sees {
            self.unseen = 0.0;
        } else {
            self.unseen += dt;
            if self.unseen >= LOSE_LOCK {
                *self = Gun {
                    unseen: self.unseen,
                    ..Default::default()
                };
            }
            return GunAct::Quiet;
        }
        let Some(w) = self.warned else {
            self.warned = Some(0.0);
            self.left = BURST;
            self.next = CHIRP;
            return GunAct::Chirp;
        };
        self.warned = Some(w + dt);
        self.next -= dt;
        if self.next > 0.0 {
            return GunAct::Quiet;
        }
        if self.left == 0 {
            self.left = BURST;
            self.bursts += 1;
        }
        self.left -= 1;
        self.next = if self.left == 0 { BURST_GAP } else { ROUND_GAP };
        GunAct::Fire
    }

    /// Whether the round it fires now is in its first burst since the
    /// chirp: a warning, wide and short, walking in on you.
    pub fn warning(&self) -> bool {
        self.bursts == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sweep runs end to end and back at its pace, pausing at each
    /// end, and never leaves its arc: the same every time round.
    #[test]
    fn it_sweeps_its_arc_in_a_rhythm() {
        let (centre, arc) = (30.0, 200.0);
        let travel = arc / SWEEP_SPEED;
        let period = 2.0 * (travel + DWELL);
        let mut t = 0.0;
        while t < period * 2.0 {
            let y = sweep_yaw(t, centre, arc);
            assert!(wrap_deg(y - centre).abs() <= arc * 0.5 + 1e-3, "{y} at {t}");
            assert!(
                (sweep_yaw(t + period, centre, arc) - y).abs() < 1e-2,
                "repeats"
            );
            t += 0.05;
        }
        // At the ends, it pauses.
        let end = sweep_yaw(travel + DWELL * 0.5, centre, arc);
        assert!((wrap_deg(end - centre) - arc * 0.5).abs() < 1e-3);
        assert_eq!(
            sweep_yaw(travel + 0.1, centre, arc),
            sweep_yaw(travel + 1.0, centre, arc)
        );
        // Slow enough to watch: an end to end crossing takes seconds.
        assert!(travel > 5.0);
    }

    /// Behind it is blind: its head stops at the end of its arc, so what's
    /// behind is never in view however it turns.
    #[test]
    fn behind_it_is_blind() {
        let (centre, arc) = (0.0, 200.0);
        assert!(can_ever_see(0.0, centre, arc));
        assert!(
            can_ever_see(120.0, centre, arc),
            "the end of its arc, and its eye past it"
        );
        assert!(!can_ever_see(180.0, centre, arc), "straight behind");
        assert!(!can_ever_see(-140.0, centre, arc));
        // Told to look behind, it goes as far as it can and no further.
        assert_eq!(clamp_to_arc(180.0, centre, arc), 100.0);
        let mut yaw = 0.0;
        for _ in 0..600 {
            yaw = slew(yaw, 170.0, centre, arc, 1.0 / 60.0);
        }
        assert!((yaw - 100.0).abs() < 1e-3);
        assert!(!in_view(170.0, yaw), "still can't see you there");
        // Across the back, from one end to the other, it goes round the
        // front: the long way.
        let mut y = 95.0;
        y = slew(y, -95.0, centre, arc, 0.1);
        assert!(y < 95.0 && y > 0.0, "turned back through the front: {y}");
    }

    /// It warns before it fires: a chirp, then nothing for [`CHIRP`]
    /// seconds, then bursts of [`BURST`]; out of sight a moment it keeps
    /// its lock, out of sight a while and it has to warn again.
    #[test]
    fn it_chirps_before_it_fires() {
        let dt = 0.01;
        let mut g = Gun::default();
        assert_eq!(g.step(true, dt), GunAct::Chirp);
        let mut first = None;
        let mut fired = 0;
        for k in 1..400 {
            if g.step(true, dt) == GunAct::Fire {
                first.get_or_insert(k as f32 * dt);
                fired += 1;
            }
        }
        let first = first.expect("it fired");
        assert!(first >= CHIRP - 0.02, "first round at {first}");
        // About a burst every burst cycle.
        let cycle = ROUND_GAP * (BURST - 1) as f32 + BURST_GAP;
        let bursts = ((4.0 - CHIRP) / cycle).floor() as u32;
        assert!(
            fired >= bursts * BURST && fired <= (bursts + 1) * BURST,
            "{fired}"
        );
        // A moment behind cover: it keeps you.
        for _ in 0..50 {
            assert_ne!(g.step(false, dt), GunAct::Fire);
        }
        let mut chirped = false;
        for _ in 0..50 {
            chirped |= g.step(true, dt) == GunAct::Chirp;
        }
        assert!(!chirped, "a half-second break keeps the lock");
        // A while: it has to warn again.
        for _ in 0..((LOSE_LOCK / dt) as usize + 5) {
            g.step(false, dt);
        }
        assert_eq!(g.step(true, dt), GunAct::Chirp);
    }

    /// Its first burst after a chirp is the warning; the next isn't.
    #[test]
    fn its_first_burst_warns() {
        let dt = 0.01;
        let mut g = Gun::default();
        g.step(true, dt);
        let mut first = Vec::new();
        for _ in 0..600 {
            if g.step(true, dt) == GunAct::Fire {
                first.push(g.warning());
            }
        }
        assert!(first.len() > BURST as usize * 2, "{first:?}");
        assert!(first[..BURST as usize].iter().all(|w| *w), "{first:?}");
        assert!(first[BURST as usize..].iter().all(|w| !*w), "{first:?}");
    }

    #[test]
    fn bearings_match_the_games_yaw() {
        assert!(
            (bearing([0.0, 0.0], [0.0, -10.0]) - 0.0).abs() < 1e-4,
            "-Z is 0"
        );
        assert!(
            (bearing([0.0, 0.0], [10.0, 0.0]) - 90.0).abs() < 1e-4,
            "+X is 90"
        );
        assert!((bearing([0.0, 0.0], [-10.0, 0.0]) + 90.0).abs() < 1e-4);
    }
}
