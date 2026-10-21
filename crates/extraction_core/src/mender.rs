//! The mender's rules: a small drone that flies to wounded machines and
//! welds them whole again. (The machine is `robots::mender` in the game.)
//!
//! - **It picks the worst hurt** ([`need`]): the machine with the most
//!   missing, among those it can reach, nearer ones first.
//! - **It welds slowly** ([`WELD_RATE`]) and only what's still there: a
//!   part shot off stays off.
//! - **Welding, it's a sitting duck.** It hangs still beside the part with
//!   its torch's arc flaring blue-white (you'll see it a valley away), for
//!   as long as the weld takes.

/// Hit points a second it welds back.
pub const WELD_RATE: f32 = 9.0;
/// How near the part it welds from (m), and the part's own reach (its
/// torch is on a short arm).
pub const WELD_REACH: f32 = 1.3;
/// A part this far below whole is worth a weld (fraction of its max).
pub const WORTH: f32 = 0.9;
/// How far it'll fly to a patient (m).
pub const RANGE: f32 = 220.0;
/// Its cruising height over the ground, and how fast it flies (m/s).
pub const CRUISE: f32 = 9.0;

/// How badly a machine wants welding: its parts' (hp, max) that are still
/// there, and how far it is (m). Zero: nothing to do. Higher wins.
pub fn need(parts: &[(f32, f32)], distance: f32) -> f32 {
    if distance > RANGE {
        return 0.0;
    }
    let missing: f32 = parts
        .iter()
        .filter(|(hp, max)| *hp > 0.0 && *hp < max * WORTH)
        .map(|(hp, max)| max - hp)
        .sum();
    if missing <= 0.0 {
        return 0.0;
    }
    missing / (1.0 + distance / 40.0)
}

/// One tick of welding a part at `hp` of `max`: its new hp. A part shot
/// off (0 hp) stays off.
pub fn weld(hp: f32, max: f32, dt: f32) -> f32 {
    if hp <= 0.0 {
        return hp;
    }
    (hp + WELD_RATE * dt).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_picks_the_worst_hurt_near_at_hand() {
        let whole = [(100.0, 100.0), (50.0, 50.0)];
        assert_eq!(need(&whole, 10.0), 0.0, "nothing to do");
        let scratched = [(95.0, 100.0)];
        assert_eq!(need(&scratched, 10.0), 0.0, "not worth it");
        let hurt = [(40.0, 100.0), (50.0, 50.0)];
        let worse = [(10.0, 100.0), (20.0, 50.0)];
        assert!(need(&worse, 50.0) > need(&hurt, 50.0));
        assert!(need(&hurt, 10.0) > need(&hurt, 120.0), "nearer first");
        assert_eq!(need(&hurt, RANGE + 1.0), 0.0, "out of range");
        // A part shot off isn't counted: it can't weld it back.
        assert_eq!(need(&[(0.0, 100.0)], 10.0), 0.0);
    }

    #[test]
    fn it_welds_slowly_and_only_whats_there() {
        let mut hp = 40.0;
        let mut t = 0.0;
        while hp < 100.0 {
            hp = weld(hp, 100.0, 0.1);
            t += 0.1;
        }
        assert_eq!(hp, 100.0);
        assert!(t > 5.0, "slow enough to catch at it: {t}");
        assert_eq!(weld(0.0, 100.0, 1.0), 0.0);
    }
}
