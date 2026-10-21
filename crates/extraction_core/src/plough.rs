//! The plough's rules: a tracked rotary snow-blower that keeps a mountain
//! road open for the Custodian's convoys. (The machine is
//! `robots::plough` in the game.) It only makes sense in snow, and it
//! changes the snow:
//!
//! - **It runs the road** ([`Run`]): up from its shed to the road's end,
//!   a pause, and back down, then a rest, then again. It never leaves the
//!   road.
//! - **Behind it is a lane** of packed snow [`BLADE`] wide that takes no
//!   prints until fresh snow fills it ([`lane_fresh`]): the one way to
//!   move that the print-reading machines can't follow, and in full view.
//! - **What lies in its way goes in the bank**: snow, loot and bodies are
//!   thrown out of its chute onto the downhill side ([`bank_side`]) and
//!   buried there. A hand held in the bank digs out what's under it
//!   ([`DIG`]).
//! - **Its tell**: a horn when something's in its lane ahead
//!   ([`HORN_AHEAD`]), the beacon, the plume. Its auger kills.
//! - **Its weakness**: anything heavy ([`JAM_KG`]) into the auger jams it
//!   for [`JAM_TIME`]; jammed, its engine hatch can be pulled.

/// How fast it ploughs (m/s), and how fast it turns (degrees a second).
pub const SPEED: f32 = 2.6;
pub const TURN: f32 = 30.0;
/// Its blade's width (m): the lane it clears.
pub const BLADE: f32 = 3.0;
/// How far ahead of its middle the auger's mouth is (m), and how deep
/// the mouth reaches.
pub const MOUTH: f32 = 2.4;
pub const MOUTH_DEPTH: f32 = 0.9;
/// Harm a second to anything caught in the auger.
pub const AUGER_HARM: f32 = 90.0;
/// Something in its lane this far ahead (m): the horn.
pub const HORN_AHEAD: f32 = 22.0;
/// Seconds of horn before it reaches you at its pace (it doesn't stop).
pub const HORN_LEAD: f32 = HORN_AHEAD / SPEED;
/// How far to the side of the road's middle the bank lies (m).
pub const BANK_OUT: f32 = 4.5;
/// An item at least this heavy (kg) jams the auger; lighter goes in the
/// bank.
pub const JAM_KG: f32 = 1.5;
/// Seconds a jam holds it (the engine labouring, then cutting out).
pub const JAM_TIME: f32 = 30.0;
/// Seconds of a hand held in the bank to dig out what's under it, and
/// how near (m).
pub const DIG: f32 = 1.2;
pub const DIG_REACH: f32 = 0.9;
/// Seconds it pauses at the road's end, and rests in its shed.
pub const PAUSE_TOP: f32 = 20.0;
pub const REST: f32 = 150.0;
/// Seconds a lane stays clean with no snow falling, and in a blizzard.
pub const LANE_LIFE: f32 = 25.0 * 60.0;
pub const LANE_LIFE_FALLING: f32 = 6.0 * 60.0;
/// The lightest lying snow (0..1 cover) it turns out for.
pub const NEEDS_COVER: f32 = 0.2;

/// A road's length (m).
pub fn length(points: &[[f32; 2]]) -> f32 {
    points
        .windows(2)
        .map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt())
        .sum()
}

/// The point `s` metres along a road, and the way along it there (unit
/// x/z). Clamped to its ends.
pub fn along(points: &[[f32; 2]], s: f32) -> ([f32; 2], [f32; 2]) {
    let mut left = s.max(0.0);
    for w in points.windows(2) {
        let (dx, dz) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let l = (dx * dx + dz * dz).sqrt();
        if l < 1e-6 {
            continue;
        }
        let dir = [dx / l, dz / l];
        if left <= l {
            return ([w[0][0] + dir[0] * left, w[0][1] + dir[1] * left], dir);
        }
        left -= l;
    }
    match points {
        [.., a, b] => {
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let l = (dx * dx + dz * dz).sqrt().max(1e-6);
            (*b, [dx / l, dz / l])
        }
        [a] => (*a, [0.0, -1.0]),
        [] => ([0.0, 0.0], [0.0, -1.0]),
    }
}

/// Which side the bank goes (+1: the right of the way it's going, -1:
/// the left): the downhill side, from the ground's height on each.
pub fn bank_side(left_height: f32, right_height: f32) -> f32 {
    if right_height <= left_height {
        1.0
    } else {
        -1.0
    }
}

/// Whether a lane cleared `age` seconds ago is still clean with snow
/// falling this hard (0..1).
pub fn lane_fresh(age: f32, falling: f32) -> bool {
    let life = 1.0 / (1.0 / LANE_LIFE + falling / LANE_LIFE_FALLING);
    age < life
}

/// Whether a point at `ahead` metres in front of its middle and `side`
/// metres off its line (signed) is in the auger's mouth.
pub fn in_mouth(ahead: f32, side: f32) -> bool {
    ahead > MOUTH - 0.4 && ahead < MOUTH + MOUTH_DEPTH && side.abs() < BLADE * 0.5
}

/// Whether a point is in its lane ahead within the horn's reach.
pub fn in_lane_ahead(ahead: f32, side: f32) -> bool {
    ahead > MOUTH - 0.4 && ahead < HORN_AHEAD && side.abs() < BLADE * 0.5 + 0.5
}

/// Where it is on its beat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leg {
    /// Ploughing up the road; `s` metres along.
    Up(f32),
    /// At the road's end; seconds left.
    Top(f32),
    /// Ploughing back down.
    Down(f32),
    /// In its shed; seconds left.
    Rest(f32),
}

/// Its beat along a road `len` metres long.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub leg: Leg,
    pub len: f32,
}

impl Run {
    pub fn new(len: f32) -> Self {
        Run {
            leg: Leg::Up(0.0),
            len,
        }
    }

    /// One tick of `dt` at `speed` (0 when it's jammed or stalled): where
    /// it is now along the road, and whether it's ploughing (moving).
    pub fn step(&mut self, speed: f32, dt: f32) -> (f32, bool) {
        let moved = speed * dt;
        self.leg = match self.leg {
            Leg::Up(s) if s + moved >= self.len => Leg::Top(PAUSE_TOP),
            Leg::Up(s) => Leg::Up(s + moved),
            Leg::Top(t) if t - dt <= 0.0 => Leg::Down(self.len),
            Leg::Top(t) => Leg::Top(t - dt),
            Leg::Down(s) if s - moved <= 0.0 => Leg::Rest(REST),
            Leg::Down(s) => Leg::Down(s - moved),
            Leg::Rest(t) if t - dt <= 0.0 => Leg::Up(0.0),
            Leg::Rest(t) => Leg::Rest(t - dt),
        };
        match self.leg {
            Leg::Up(s) | Leg::Down(s) => (s, speed > 0.0),
            Leg::Top(_) => (self.len, false),
            Leg::Rest(_) => (0.0, false),
        }
    }

    /// Which way along the road it faces (+1 up, -1 down).
    pub fn facing(&self) -> f32 {
        match self.leg {
            Leg::Up(_) | Leg::Rest(_) => 1.0,
            Leg::Down(_) | Leg::Top(_) => -1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROAD: [[f32; 2]; 3] = [[0.0, 0.0], [0.0, -30.0], [40.0, -30.0]];

    #[test]
    fn it_follows_the_road() {
        assert!((length(&ROAD) - 70.0).abs() < 1e-4);
        let (p, d) = along(&ROAD, 10.0);
        assert!((p[0] - 0.0).abs() < 1e-4 && (p[1] + 10.0).abs() < 1e-4);
        assert!((d[1] + 1.0).abs() < 1e-4, "heading -Z");
        let (p, d) = along(&ROAD, 50.0);
        assert!((p[0] - 20.0).abs() < 1e-4 && (p[1] + 30.0).abs() < 1e-4);
        assert!((d[0] - 1.0).abs() < 1e-4, "round the bend, heading +X");
        let (p, _) = along(&ROAD, 500.0);
        assert_eq!(p, [40.0, -30.0], "clamped to the end");
    }

    /// Up, a pause, down, a rest, and up again: a beat you can learn.
    #[test]
    fn its_beat_goes_round() {
        let mut r = Run::new(70.0);
        let dt = 0.1;
        let mut t = 0.0;
        let mut saw = (false, false, false);
        while t < 400.0 {
            r.step(SPEED, dt);
            t += dt;
            match r.leg {
                Leg::Top(_) => saw.0 = true,
                Leg::Down(_) if saw.0 => saw.1 = true,
                Leg::Rest(_) if saw.1 => saw.2 = true,
                Leg::Up(_) if saw.2 => break,
                _ => {}
            }
        }
        assert!(saw.0 && saw.1 && saw.2, "{saw:?}");
        let lap = 2.0 * 70.0 / SPEED + PAUSE_TOP + REST;
        assert!((t - lap).abs() < 2.0, "a lap in {t:.0} s, about {lap:.0}");
        // Jammed, it doesn't move.
        let mut r = Run::new(70.0);
        r.step(SPEED, 1.0);
        let at = r.leg;
        r.step(0.0, 1.0);
        assert_eq!(at, r.leg);
    }

    #[test]
    fn the_horn_comes_well_before_the_auger() {
        assert!(
            HORN_LEAD > 6.0,
            "seconds to get out of its lane: {HORN_LEAD}"
        );
        assert!(in_lane_ahead(15.0, 0.5));
        assert!(!in_lane_ahead(15.0, 3.0), "off the road's middle: no horn");
        assert!(in_mouth(MOUTH + 0.2, 1.0));
        assert!(!in_mouth(MOUTH + 3.0, 0.0));
        assert!(!in_mouth(0.0, 0.0), "beside it, not in it");
    }

    #[test]
    fn the_lane_fills_back_with_snow() {
        assert!(lane_fresh(60.0, 0.0));
        assert!(lane_fresh(LANE_LIFE - 1.0, 0.0));
        assert!(!lane_fresh(LANE_LIFE + 1.0, 0.0));
        assert!(
            !lane_fresh(LANE_LIFE_FALLING + 1.0, 1.0),
            "faster in a blizzard"
        );
        assert_eq!(bank_side(10.0, 9.0), 1.0, "the right is lower");
        assert_eq!(bank_side(9.0, 10.0), -1.0);
    }
}
