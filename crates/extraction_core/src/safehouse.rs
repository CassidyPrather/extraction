//! Safehouses: one log cabin per valley you can claim and sleep in, so a
//! raid can last more than a day (`docs/PROGRESSION.md`, "Safehouses").
//!
//! Claiming is physical, in three steps you can do in any order: clear it
//! (be inside with no machine close), bar it (a door wedge at its door),
//! light its stove (with matches on you). Claimed, it has its bed, a crate
//! of its own and a radio. A claim lasts until the Surge; a loud raid
//! breaks its door and scatters the crate.

use serde::{Deserialize, Serialize};

use crate::save::SavedItem;

/// Each valley's safehouse: the map, and the `cabin_log` placement's
/// (x, z) (the game finds the building there).
pub const SAFEHOUSES: [(&str, [f32; 2]); 3] = [
    // The Logging camp's south cabin: the west side's halfway house.
    ("marrow_creek", [-70.8, 113.5]),
    // The Charcoal hut, the climb's last stop before the rendezvous.
    ("kettle_pass", [119.0, -138.0]),
    // Woodward's cabin, in the Cinder wood.
    ("cinder_works", [-75.0, -159.0]),
];

/// A valley's safehouse, if it has one.
pub fn safehouse(map: &str) -> Option<[f32; 2]> {
    SAFEHOUSES
        .iter()
        .find(|(m, _)| *m == map)
        .map(|(_, at)| *at)
}

/// A machine this close to the cabin means it isn't cleared.
pub const CLEAR_RADIUS: f32 = 30.0;
/// Zone-clock seconds a night's sleep takes (the Surge comes every hour of
/// Zone time, so a night is a real bite out of a cycle).
pub const NIGHT: f32 = 15.0 * 60.0;
/// How loud a raid has to have got (the attention feeling, 0-4) for the
/// machines to find the safehouse.
pub const FOUND_AT: u8 = 3;
/// A safehouse on your usual ways (`zone_memory::ZoneMemory::heat_near`,
/// this habitual or more) is found one feeling sooner: they walk your
/// ways now.
pub const HABIT_FINDS: f32 = 0.6;

/// How loud a raid has to have got for them to find the safehouse, with
/// your ways this habitual round it.
pub fn found_at(habit: f32) -> u8 {
    if habit >= HABIT_FINDS {
        FOUND_AT - 1
    } else {
        FOUND_AT
    }
}

/// One valley's safehouse, as saved with that valley's Zone.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SafehouseSave {
    /// Cleared, barred, lit: all three is claimed.
    pub cleared: bool,
    pub barred: bool,
    pub lit: bool,
    /// The Surge cycle it was claimed in (the next Surge ends the claim).
    pub cycle: u32,
    /// Broken into while you were away: the door's off, the crate tipped.
    pub broken: bool,
    /// What's in its crate, positions local to the crate.
    pub stash: Vec<SavedItem>,
    /// Nights slept there, all told.
    pub nights: u32,
    /// The Surge cycle the last people's leftovers were laid in: once a
    /// cycle (they're saved with the Zone after that, like anything else).
    #[serde(default)]
    pub laid: Option<u32>,
}

impl SafehouseSave {
    pub fn claimed(&self) -> bool {
        self.cleared && self.barred && self.lit && !self.broken
    }

    /// The Surge has been through since it was claimed: the claim's gone,
    /// and what was in the crate with it.
    pub fn surge(&mut self, cycle: u32) {
        if cycle != self.cycle {
            *self = SafehouseSave {
                cycle,
                nights: self.nights,
                laid: self.laid,
                ..Default::default()
            };
        }
    }

    /// Home from a raid as loud as `feeling` (0-4), the safehouse's
    /// surroundings this habitual (`found_at`): if the machines came
    /// looking, they found it. Returns what was in its crate
    /// (to scatter about the floor).
    pub fn found(&mut self, feeling: u8, habit: f32) -> Option<Vec<SavedItem>> {
        if !self.claimed() || feeling < found_at(habit) {
            return None;
        }
        self.broken = true;
        self.barred = false;
        Some(std::mem::take(&mut self.stash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{CINDER_WORKS, KETTLE_PASS, MARROW_CREEK, MapDef};

    #[test]
    fn every_valley_has_a_cabin_at_its_safehouse() {
        for (id, text) in [
            ("marrow_creek", MARROW_CREEK),
            ("kettle_pass", KETTLE_PASS),
            ("cinder_works", CINDER_WORKS),
        ] {
            let map = MapDef::from_ron(text).unwrap();
            let at = safehouse(id).unwrap();
            assert!(
                map.placements
                    .iter()
                    .any(|p| p.prefab == "cabin_log" && crate::geom::dist(p.at, at) < 0.5),
                "{id}: no cabin at {at:?}"
            );
        }
    }

    #[test]
    fn a_claim_lasts_until_the_surge_or_a_loud_raid() {
        let mut s = SafehouseSave {
            cycle: 2,
            ..Default::default()
        };
        assert!(!s.claimed());
        s.cleared = true;
        s.barred = true;
        s.lit = true;
        assert!(s.claimed());
        s.stash.push(SavedItem {
            id: "canned_stew".into(),
            condition: 1.0,
            rounds: 0,
            charge_wh: 0.0,
            local: [0.0; 3],
            gun: None,
            inserted: Vec::new(),
            loads: Default::default(),
        });
        // A quiet raid: nothing comes.
        assert!(s.found(FOUND_AT - 1, 0.0).is_none());
        // A loud one: the door's broken, the crate scattered.
        let scattered = s.found(FOUND_AT, 0.0).unwrap();
        assert_eq!(scattered.len(), 1);
        assert!(!s.claimed() && s.broken && s.stash.is_empty());
        // The same cycle keeps the state; the next Surge wipes it.
        s.nights = 2;
        s.surge(2);
        assert!(s.broken);
        s.surge(3);
        assert!(!s.broken && !s.cleared && s.cycle == 3);
        assert_eq!(s.nights, 2, "the nights slept are the career's");
        s.laid = Some(2);
        s.surge(4);
        assert_eq!(s.laid, Some(2), "leftovers lay once a cycle, by the cycle");
    }

    /// On your usual ways, a raid a feeling quieter leads them to it.
    #[test]
    fn a_safehouse_on_your_ways_is_found_sooner() {
        let claimed = || SafehouseSave {
            cleared: true,
            barred: true,
            lit: true,
            ..Default::default()
        };
        assert!(claimed().found(FOUND_AT - 1, 0.5).is_none());
        assert!(claimed().found(FOUND_AT - 1, HABIT_FINDS).is_some());
        // Never a quiet raid, however habitual.
        assert!(claimed().found(FOUND_AT - 2, 1.0).is_none());
    }
}
