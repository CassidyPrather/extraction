//! The heavy's rules: what its plates, bolts and weak points take, how it
//! heats and stalls, and when the Custodian sends one. (The body and its
//! mind are `robots::heavy` and `robots::heavy_mind` in the game.)
//!
//! The heavy is not a health bar. Its hull stops everything; what can be
//! hurt is small and particular:
//!
//! - **Plates** are bolted on, four bolts each: a hatch over the power cell
//!   at its back, a plate down each flank over a cooling vent. Rounds that
//!   strike a plate do nothing to it; the *bolts* can be shot out, and a
//!   plate hanging on [`PRY_AT`] bolts or fewer can be torn off by hand.
//!   The last bolt gone, it drops on its own.
//! - **Weak points** behind them take nothing while their plate is on
//!   ([`covered`]): a blast or a lucky round through the gap does nothing.
//! - **Knees**: the actuators at its knees are out in the open. Lose one
//!   and it limps; lose two and it goes down for good.
//! - **Vents** shed its heat. Every vent lost, it runs hotter; past
//!   [`STALL_AT`] it stalls, sinking onto its knees for [`STALL_TIME`], and
//!   the hatch over its cell blows its bolts to dump the heat.
//! - **The cell** sits low on its back. Shoot it through and it's dead (the
//!   cell with it); pull it out by hand and it's dead and the cell is yours.
//!   Standing, it's out of reach: it has to be down on its knees (stalled,
//!   stunned, or held by a well).

/// The hatch over the cell (back), then the flank plates over the vents.
pub const PLATES: usize = 3;
pub const HATCH: usize = 0;
pub const BOLTS_PER_PLATE: usize = 4;
pub const VENTS: usize = 2;

/// Hit points of each piece. The hull and the plates themselves are
/// effectively unbreakable ([`HARDENED`]).
pub const HARDENED: f32 = 1.0e9;
/// One rifle or pistol round each; they're small.
pub const BOLT_HP: f32 = 30.0;
pub const VENT_HP: f32 = 120.0;
/// About five rifle rounds, three from a Mosin.
pub const KNEE_HP: f32 = 320.0;
pub const CELL_HP: f32 = 200.0;
pub const ARM_HP: f32 = 300.0;
pub const EYE_HP: f32 = 80.0;

/// A plate on this many bolts or fewer can be torn off by hand.
pub const PRY_AT: usize = 2;

/// What a piece of the heavy is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The hull: stops everything.
    Hull,
    Plate(u8),
    /// Bolt `bolt` of plate `plate`.
    Bolt {
        plate: u8,
        bolt: u8,
    },
    /// A knee actuator, by leg.
    Knee(u8),
    Vent(u8),
    Cell,
    Arm,
    Eye,
}

impl Slot {
    /// The piece's hit points when new.
    pub fn hp(self) -> f32 {
        match self {
            Slot::Hull | Slot::Plate(_) => HARDENED,
            Slot::Bolt { .. } => BOLT_HP,
            Slot::Knee(_) => KNEE_HP,
            Slot::Vent(_) => VENT_HP,
            Slot::Cell => CELL_HP,
            Slot::Arm => ARM_HP,
            Slot::Eye => EYE_HP,
        }
    }

    /// The plate over this piece, if it sits behind one.
    pub fn behind(self) -> Option<usize> {
        match self {
            Slot::Cell => Some(HATCH),
            Slot::Vent(v) => Some(1 + v as usize),
            _ => None,
        }
    }
}

/// Whether `slot` is shielded from harm: it sits behind a plate that's
/// still on. A covered weak point takes nothing, whatever reaches it.
pub fn covered(slot: Slot, plates_on: [bool; PLATES]) -> bool {
    slot.behind().is_some_and(|p| plates_on[p])
}

/// Harm that gets through to `slot`.
pub fn harm(slot: Slot, plates_on: [bool; PLATES], damage: f32) -> f32 {
    if covered(slot, plates_on) || matches!(slot, Slot::Hull | Slot::Plate(_)) {
        0.0
    } else {
        damage.max(0.0)
    }
}

/// What a plate does with `bolts` bolts left in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlateHold {
    /// Fast: shoot the bolts.
    Fast,
    /// Loose: it hangs askew, and a hand can tear it off.
    Loose,
    /// It falls.
    Off,
}

pub fn plate_hold(bolts: usize) -> PlateHold {
    match bolts {
        0 => PlateHold::Off,
        b if b <= PRY_AT => PlateHold::Loose,
        _ => PlateHold::Fast,
    }
}

/// Legs it can lose and still walk.
pub fn standing(knees_alive: usize) -> bool {
    knees_alive >= 3
}

// --- Heat ------------------------------------------------------------------

/// Heat at which it stalls (0 is cold).
pub const STALL_AT: f32 = 1.0;
/// How long a stall lasts, s, and the heat it comes out of it with.
pub const STALL_TIME: f32 = 10.0;
pub const AFTER_STALL: f32 = 0.35;
/// What it sheds, per second: a little through the hull, the rest
/// through each live vent.
pub const COOL_HULL: f32 = 0.03;
pub const COOL_VENT: f32 = 0.06;
/// What it makes: walking, per second at pace 1; each blow of its arm.
pub const HEAT_WALK: f32 = 0.045;
pub const HEAT_BLOW: f32 = 0.12;
/// Standing in a burner, per second; a snare's discharge, at once.
pub const HEAT_BURNER: f32 = 0.32;
pub const HEAT_SNARE: f32 = 0.35;

/// One tick of heat: `pace` is how hard it's walking (0 still, 1 its
/// usual), `external` the heat put into it this tick (burners, snares,
/// blows), `vents` how many still work.
pub fn heat_step(level: f32, dt: f32, pace: f32, external: f32, vents: usize) -> f32 {
    let cool = COOL_HULL + COOL_VENT * vents.min(VENTS) as f32;
    let made = HEAT_WALK * pace.max(0.0) * dt + external.max(0.0);
    (level + made - cool * dt).max(0.0)
}

// --- When the Custodian sends one ------------------------------------------

/// Felt attention ([`crate::anomaly`]'s feelings, 0-4) at which the Zone
/// is looking hard at you, and how long it has to stay there.
pub const PEAK_FEELING: usize = 4;
pub const PEAK_HOLD: f32 = 40.0;
/// Not in a raid's first minutes for attention: you've barely started.
pub const EARLY: f32 = 360.0;
/// Someone at a Gate with a key, this long, is a threat to the depot.
pub const GATE_HOLD: f32 = 6.0;
/// Between one heavy leaving (or falling) and the next.
pub const COOLDOWN: f32 = 900.0;
/// At most this many a raid.
pub const PER_RAID: u32 = 2;
/// From the call to it walking out of the Gate, s: you hear it coming.
pub const LEAD: f32 = 20.0;

/// Why one is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// The whole Zone has been looking at you for a while.
    Attention,
    /// Someone's at the depot with a key.
    Gate,
}

/// The director's book on heavies, one raid.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CallBook {
    /// Seconds into the raid.
    pub raid_time: f32,
    /// Seconds the peak has lasted.
    pub peak: f32,
    /// Seconds someone has stood at the Gate with a key.
    pub at_gate: f32,
    /// Seconds before another may be sent.
    pub cooldown: f32,
    pub sent: u32,
}

/// What the director sees this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CallInput {
    pub dt: f32,
    /// The strongest feeling of attention on anyone (0-4).
    pub feeling: usize,
    /// Someone with a valley key near a Gate.
    pub gate_threat: bool,
    /// A heavy is out (walking, working or down on its knees; not wrecked).
    pub out: bool,
    /// The director is sending anything at all.
    pub enabled: bool,
}

/// One tick of the book: a call, when it's time to send one. The Gate
/// comes before anything (even early in a raid); attention has to peak and
/// hold, and not in a raid's first minutes. Never two at once, never
/// within [`COOLDOWN`] of the last one going, at most [`PER_RAID`].
pub fn call_step(book: &mut CallBook, input: CallInput) -> Option<Call> {
    book.raid_time += input.dt;
    book.peak = if input.feeling >= PEAK_FEELING {
        book.peak + input.dt
    } else {
        0.0
    };
    book.at_gate = if input.gate_threat {
        book.at_gate + input.dt
    } else {
        0.0
    };
    if input.out {
        // The cooldown runs from when it's gone.
        book.cooldown = COOLDOWN;
        return None;
    }
    book.cooldown = (book.cooldown - input.dt).max(0.0);
    if !input.enabled || book.cooldown > 0.0 || book.sent >= PER_RAID {
        return None;
    }
    let call = if book.at_gate >= GATE_HOLD {
        Some(Call::Gate)
    } else if book.peak >= PEAK_HOLD && book.raid_time >= EARLY {
        Some(Call::Attention)
    } else {
        None
    };
    if call.is_some() {
        book.sent += 1;
        book.cooldown = COOLDOWN;
        book.peak = 0.0;
        book.at_gate = 0.0;
    }
    call
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_ON: [bool; PLATES] = [true; PLATES];

    /// The hull and plates take nothing; covered weak points take nothing
    /// until their plate is off; open ones take it all.
    #[test]
    fn plates_shield_what_is_behind_them() {
        assert_eq!(harm(Slot::Hull, ALL_ON, 500.0), 0.0);
        assert_eq!(harm(Slot::Plate(1), ALL_ON, 500.0), 0.0);
        assert_eq!(harm(Slot::Cell, ALL_ON, 500.0), 0.0);
        assert_eq!(harm(Slot::Vent(0), ALL_ON, 500.0), 0.0);
        assert_eq!(harm(Slot::Vent(1), ALL_ON, 500.0), 0.0);
        // Bolts, knees, the arm and the eye are out in the open.
        for open in [
            Slot::Bolt { plate: 0, bolt: 2 },
            Slot::Knee(3),
            Slot::Arm,
            Slot::Eye,
        ] {
            assert_eq!(harm(open, ALL_ON, 40.0), 40.0, "{open:?}");
        }
        let hatch_off = [false, true, true];
        assert_eq!(harm(Slot::Cell, hatch_off, 50.0), 50.0);
        assert_eq!(harm(Slot::Vent(0), hatch_off, 50.0), 0.0);
        let left_off = [true, false, true];
        assert_eq!(harm(Slot::Vent(0), left_off, 50.0), 50.0);
        assert_eq!(harm(Slot::Vent(1), left_off, 50.0), 0.0);
        assert_eq!(harm(Slot::Cell, left_off, 50.0), 0.0);
    }

    /// A plate holds fast on three bolts or more, hangs loose (for a hand
    /// to tear off) on two or one, and drops on none.
    #[test]
    fn a_plate_comes_off_bolt_by_bolt() {
        assert_eq!(plate_hold(4), PlateHold::Fast);
        assert_eq!(plate_hold(3), PlateHold::Fast);
        assert_eq!(plate_hold(2), PlateHold::Loose);
        assert_eq!(plate_hold(1), PlateHold::Loose);
        assert_eq!(plate_hold(0), PlateHold::Off);
    }

    /// Every bolt is one ordinary round; a knee takes several rifle rounds
    /// but not a magazine; nothing on it but the plates is unbreakable.
    #[test]
    fn hit_points_are_about_rounds() {
        let (pistol, rifle, mosin) = (34.0, 70.0, 110.0);
        assert!(BOLT_HP <= pistol);
        let rounds = |hp: f32, d: f32| (hp / d).ceil() as u32;
        assert!((4..=6).contains(&rounds(KNEE_HP, rifle)));
        assert!(rounds(KNEE_HP, mosin) <= 3);
        assert!(rounds(CELL_HP, rifle) <= 3, "once you're behind it");
        for s in [
            Slot::Knee(0),
            Slot::Vent(0),
            Slot::Cell,
            Slot::Arm,
            Slot::Eye,
        ] {
            assert!(s.hp() < 1000.0, "{s:?}");
        }
        assert!(!standing(2) && standing(3) && standing(4));
    }

    /// With both vents it can walk and swing all day; with one it holds out
    /// a long fight; with none it stalls within half a minute of chasing.
    /// A burner stalls it in seconds, vents or not.
    #[test]
    fn heat_follows_its_vents() {
        let dt = 0.1;
        let run = |vents: usize, external_per_s: f32, secs: f32| {
            let mut h = 0.0;
            let mut t = 0.0;
            while t < secs {
                // Chasing at a hurry, a blow every four seconds.
                let blow = if (t / 4.0).floor() != ((t + dt) / 4.0).floor() {
                    HEAT_BLOW
                } else {
                    0.0
                };
                h = heat_step(h, dt, 1.3, blow + external_per_s * dt, vents);
                if h >= STALL_AT {
                    return Some(t);
                }
                t += dt;
            }
            None
        };
        assert_eq!(run(2, 0.0, 600.0), None, "two vents: never");
        assert_eq!(run(1, 0.0, 120.0), None, "one vent: a long fight");
        let none = run(0, 0.0, 120.0).expect("no vents: it stalls");
        assert!(none < 30.0, "stalled after {none:.0} s");
        let burnt = run(2, HEAT_BURNER, 60.0).expect("a burner stalls it");
        assert!(burnt < 8.0, "stalled after {burnt:.1} s in a burner");
        // Cold and still, it cools.
        assert_eq!(heat_step(0.5, 100.0, 0.0, 0.0, 2), 0.0);
    }

    fn input(feeling: usize, gate: bool, out: bool) -> CallInput {
        CallInput {
            dt: 1.0,
            feeling,
            gate_threat: gate,
            out,
            enabled: true,
        }
    }

    /// Attention has to peak and hold, and not early in a raid.
    #[test]
    fn attention_has_to_peak_and_hold() {
        let mut book = CallBook::default();
        // Peaking from the off: nothing in the first minutes.
        for _ in 0..(EARLY as usize - 1) {
            assert_eq!(call_step(&mut book, input(4, false, false)), None);
        }
        // Then it comes.
        assert_eq!(
            call_step(&mut book, input(4, false, false)),
            Some(Call::Attention)
        );
        // A feeling that dips resets the hold.
        let mut book = CallBook {
            raid_time: EARLY,
            ..Default::default()
        };
        for k in 0..200 {
            let f = if k % 30 == 29 { 3 } else { 4 };
            assert_eq!(call_step(&mut book, input(f, false, false)), None);
        }
        // "The air hums" is not enough.
        for _ in 0..600 {
            assert_eq!(call_step(&mut book, input(3, false, false)), None);
        }
    }

    /// Standing at a Gate with a key brings one at once, early or not; but
    /// never a second while one is out, nor within the cooldown after.
    #[test]
    fn the_gate_brings_one_but_only_one() {
        let mut book = CallBook::default();
        let mut calls = Vec::new();
        for t in 0..(GATE_HOLD as usize + 1) {
            if let Some(c) = call_step(&mut book, input(0, true, false)) {
                calls.push((t, c));
            }
        }
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, Call::Gate);
        // Out for five minutes, with someone still at the Gate.
        for _ in 0..300 {
            assert_eq!(call_step(&mut book, input(4, true, true)), None);
        }
        // Gone: the cooldown runs, then another may come.
        let mut after = None;
        for t in 0..(COOLDOWN as usize + 30) {
            if call_step(&mut book, input(0, true, false)).is_some() {
                after = Some(t);
                break;
            }
        }
        let after = after.expect("a second, after the cooldown");
        assert!(after as f32 >= COOLDOWN - 1.0, "{after}");
        // And no more than that in one raid.
        for _ in 0..5000 {
            assert_eq!(call_step(&mut book, input(4, true, false)), None);
        }
        assert_eq!(book.sent, PER_RAID);
    }

    /// The director off (the recall, the training yard): none, ever.
    #[test]
    fn none_when_the_director_is_off() {
        let mut book = CallBook::default();
        for _ in 0..3000 {
            let mut i = input(4, true, false);
            i.enabled = false;
            assert_eq!(call_step(&mut book, i), None);
        }
    }
}
