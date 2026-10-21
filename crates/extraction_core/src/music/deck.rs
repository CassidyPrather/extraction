//! A cassette player's mechanism, as rules: the door, the tape and which
//! way round it went in, the four keys (play, stop, rewind, fast
//! forward), and the batteries running down.
//!
//! A cassette is one length of tape between two hubs. How far it's wound
//! is one number, `wound`: 0 with all of it on the left hub as you look
//! at side A (the start of side A, the end of side B). Playing side A
//! winds it on; playing side B (the tape turned over) winds it back.
//!
//! The mechanism is a cheap portable's:
//! - **Play** runs only with the door shut and a tape in; at the end of a
//!   side the motor stalls against the leader and the key pops up.
//! - **Rewind** and **fast forward** wind about twelve times as fast, and
//!   stop themselves at the end.
//! - **Stop** stops; pressed again while stopped, it's the eject and the
//!   door springs open. Opening the door stops everything.
//! - Batteries: the motor draws more winding than playing. Below a fifth
//!   of their charge the motor slows and the music sags flat and wobbles;
//!   flat batteries, it stops.

/// Which face of the cassette was outward (toward the window) when it
/// went in: that's the side that plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

impl Side {
    pub fn flip(self) -> Self {
        match self {
            Side::A => Side::B,
            Side::B => Side::A,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Side::A => 0,
            Side::B => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Stopped,
    Playing,
    Rewinding,
    Winding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Play,
    Stop,
    Rewind,
    FastForward,
}

/// What pressing a key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressed {
    /// The mechanism changed mode.
    Engaged(Mode),
    /// Stop while stopped: the door sprang open.
    Ejected,
    /// The key went down and nothing happened (no tape, door open, already
    /// at the end, flat batteries).
    Nothing,
}

/// Seconds of tape wound per second, fast.
pub const FAST: f32 = 12.0;
/// Watts the motor draws: playing, winding.
pub const PLAY_WATTS: f32 = 3.0;
pub const WIND_WATTS: f32 = 5.0;
/// Below this share of a cell's charge the motor starts to sag.
pub const SAG_BELOW: f32 = 0.2;
/// How slow it plays on its last breath.
pub const SAG_SPEED: f32 = 0.82;

#[derive(Clone, Debug, PartialEq)]
pub struct Deck {
    pub door_open: bool,
    /// The tape in it: its length per side (s), which side faces out.
    pub tape: Option<(f32, Side)>,
    /// How far the tape in it is wound (see the module docs).
    pub wound: f32,
    pub mode: Mode,
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            door_open: false,
            tape: None,
            wound: 0.0,
            mode: Mode::Stopped,
        }
    }
}

impl Deck {
    /// Seconds into the side that faces out.
    pub fn position(&self) -> f32 {
        match self.tape {
            Some((_, Side::A)) => self.wound,
            Some((len, Side::B)) => len - self.wound,
            None => 0.0,
        }
    }

    /// Wound as far as it goes in the direction the side plays.
    pub fn at_end(&self) -> bool {
        self.tape
            .is_some_and(|(len, _)| self.position() >= len - 1e-3)
    }

    pub fn at_start(&self) -> bool {
        self.tape.is_some() && self.position() <= 1e-3
    }

    /// A tape goes in (the door must be open). `wound` is how the tape was
    /// left (it stays with the cassette).
    pub fn insert(&mut self, len: f32, side: Side, wound: f32) -> bool {
        if !self.door_open || self.tape.is_some() {
            return false;
        }
        self.tape = Some((len, side));
        self.wound = wound.clamp(0.0, len);
        true
    }

    /// The tape comes out (door open); returns how wound it is.
    pub fn remove(&mut self) -> Option<f32> {
        if !self.door_open {
            return None;
        }
        self.tape.take().map(|_| {
            let w = self.wound;
            self.wound = 0.0;
            w
        })
    }

    pub fn open_door(&mut self) {
        self.door_open = true;
        self.mode = Mode::Stopped;
    }

    pub fn close_door(&mut self) {
        self.door_open = false;
    }

    pub fn press(&mut self, key: Key, charge: f32) -> Pressed {
        let ready = !self.door_open && self.tape.is_some() && charge > 0.0;
        let to = match key {
            Key::Stop => {
                if self.mode == Mode::Stopped {
                    self.open_door();
                    return Pressed::Ejected;
                }
                Mode::Stopped
            }
            Key::Play if ready && !self.at_end() => Mode::Playing,
            Key::Rewind if ready && !self.at_start() => Mode::Rewinding,
            Key::FastForward if ready && !self.at_end() => Mode::Winding,
            _ => return Pressed::Nothing,
        };
        if to == self.mode {
            return Pressed::Nothing;
        }
        self.mode = to;
        Pressed::Engaged(to)
    }

    /// How fast the capstan turns for a cell at `charge` (0..1 of full):
    /// 1 until the cell is low, then sagging to [`SAG_SPEED`], then 0.
    pub fn motor_speed(charge: f32) -> f32 {
        if charge <= 0.0 {
            0.0
        } else if charge >= SAG_BELOW {
            1.0
        } else {
            let k = charge / SAG_BELOW;
            SAG_SPEED + (1.0 - SAG_SPEED) * k.sqrt()
        }
    }

    /// Watts drawn now.
    pub fn watts(&self) -> f32 {
        match self.mode {
            Mode::Stopped => 0.0,
            Mode::Playing => PLAY_WATTS,
            Mode::Rewinding | Mode::Winding => WIND_WATTS,
        }
    }

    /// Runs the mechanism for `dt`; `charge` is the cell's share left.
    /// Returns true if it stopped itself (end of tape, flat cell).
    pub fn step(&mut self, dt: f32, charge: f32) -> bool {
        let Some((len, side)) = self.tape else {
            self.mode = Mode::Stopped;
            return false;
        };
        let speed = Self::motor_speed(charge);
        if speed <= 0.0 && self.mode != Mode::Stopped {
            self.mode = Mode::Stopped;
            return true;
        }
        // Positive: on through the side that faces out.
        let along = match self.mode {
            Mode::Stopped => return false,
            Mode::Playing => speed * dt,
            Mode::Winding => FAST * speed * dt,
            Mode::Rewinding => -FAST * speed * dt,
        };
        let dir = if side == Side::A { 1.0 } else { -1.0 };
        self.wound = (self.wound + along * dir).clamp(0.0, len);
        let stop = match self.mode {
            Mode::Playing | Mode::Winding => self.at_end(),
            Mode::Rewinding => self.at_start(),
            Mode::Stopped => false,
        };
        if stop {
            self.mode = Mode::Stopped;
        }
        stop
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded(side: Side) -> Deck {
        let mut d = Deck::default();
        d.open_door();
        // At the start of the side that faces out.
        let wound = if side == Side::A { 0.0 } else { 60.0 };
        assert!(d.insert(60.0, side, wound));
        d.close_door();
        d
    }

    #[test]
    fn nothing_plays_without_a_tape_or_with_the_door_open() {
        let mut d = Deck::default();
        assert_eq!(d.press(Key::Play, 1.0), Pressed::Nothing);
        d.open_door();
        assert!(d.insert(60.0, Side::A, 0.0));
        assert_eq!(d.press(Key::Play, 1.0), Pressed::Nothing);
        d.close_door();
        assert_eq!(d.press(Key::Play, 1.0), Pressed::Engaged(Mode::Playing));
        // A tape can't go in or come out with the door shut.
        assert!(d.remove().is_none());
    }

    #[test]
    fn side_b_is_the_same_tape_backwards() {
        let mut d = loaded(Side::A);
        d.press(Key::Play, 1.0);
        for _ in 0..100 {
            d.step(0.1, 1.0);
        }
        assert!((d.position() - 10.0).abs() < 1e-3);
        // Turn it over: ten seconds into A is fifty into B.
        d.open_door();
        let w = d.remove().unwrap();
        assert!(d.insert(60.0, Side::B, w));
        assert!((d.position() - 50.0).abs() < 1e-3);
    }

    #[test]
    fn it_stops_at_the_end_and_rewinds_fast() {
        let mut d = loaded(Side::B);
        d.press(Key::FastForward, 1.0);
        let mut t = 0.0;
        while d.mode != Mode::Stopped {
            d.step(0.1, 1.0);
            t += 0.1;
        }
        assert!(d.at_end());
        assert!((t - 60.0 / FAST).abs() < 0.2, "{t}");
        assert_eq!(d.press(Key::Play, 1.0), Pressed::Nothing);
        assert_eq!(d.press(Key::Rewind, 1.0), Pressed::Engaged(Mode::Rewinding));
        while d.mode != Mode::Stopped {
            d.step(0.1, 1.0);
        }
        assert!(d.at_start());
    }

    #[test]
    fn stop_twice_opens_the_door_and_opening_stops_it() {
        let mut d = loaded(Side::A);
        d.press(Key::Play, 1.0);
        assert_eq!(d.press(Key::Stop, 1.0), Pressed::Engaged(Mode::Stopped));
        assert_eq!(d.press(Key::Stop, 1.0), Pressed::Ejected);
        assert!(d.door_open);
        d.close_door();
        d.press(Key::Play, 1.0);
        d.open_door();
        assert_eq!(d.mode, Mode::Stopped);
    }

    #[test]
    fn low_batteries_sag_then_stop() {
        assert_eq!(Deck::motor_speed(1.0), 1.0);
        assert_eq!(Deck::motor_speed(SAG_BELOW), 1.0);
        let low = Deck::motor_speed(0.02);
        assert!(low < 0.95 && low >= SAG_SPEED);
        let mut d = loaded(Side::A);
        d.press(Key::Play, 1.0);
        assert!(d.step(0.1, 0.0));
        assert_eq!(d.mode, Mode::Stopped);
        assert_eq!(d.press(Key::Play, 0.0), Pressed::Nothing);
    }
}
