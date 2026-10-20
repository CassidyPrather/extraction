//! The save file: the Zone's persistent state between raids, the truck's
//! stash, what you were carrying, and the memorial board.
//!
//! Items are saved by catalog id (not by index) so the catalog can grow
//! without breaking saves. A gun saves its mechanism and whatever magazine
//! sits in its well, nested.

use serde::{Deserialize, Serialize};

use crate::firearm::Round;
use crate::progression::Progress;

pub const SAVE_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SaveGame {
    pub version: u32,
    pub stats: Stats,
    pub zone: ZoneSave,
    /// The truck's stash crate, positions local to the crate.
    pub stash: Vec<SavedItem>,
    pub loadout: LoadoutSave,
    pub memorial: Memorial,
    /// The logbook's write-up of the last raid.
    #[serde(default)]
    pub logbook: Option<String>,
    #[serde(default)]
    pub requests: RequestsSave,
    /// The driver's jobs and what's been fitted to the truck. Older saves
    /// have none: they're credited from `requests` when loaded.
    #[serde(default)]
    pub progress: Option<Progress>,
    /// The first-time moments the driver has already talked you through
    /// (one line each, once a career).
    #[serde(default)]
    pub firsts: Vec<String>,
}

/// The driver's requests: how many there have been, what's wanted now,
/// and where the last cache was left.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RequestsSave {
    pub number: u32,
    pub want: Option<(String, u32)>,
    pub cache: Option<String>,
    /// The cache's valley and (x, z) there.
    #[serde(default)]
    pub cache_at: Option<(String, [f32; 2])>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub raids: u32,
    pub extractions: u32,
    pub deaths: u32,
    /// The three keys were turned at a Gate: the machines went home.
    #[serde(default)]
    pub recalled: bool,
    /// The notes read, "map/name", in order: the cork board's pins.
    #[serde(default)]
    pub notes_read: Vec<String>,
    /// The ending played: Lev went down to the Depot with you.
    #[serde(default)]
    pub epilogue: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ZoneSave {
    pub map: String,
    /// False for a new game: the Zone hasn't been rolled yet.
    #[serde(default)]
    pub rolled: bool,
    /// Which Surge cycle this is: loot is rolled from the seed and this.
    pub cycle: u32,
    /// Seconds of Zone time since the last Surge.
    pub clock: f32,
    /// Everything lying loose in the Zone (loot not yet taken, caches you
    /// left, things you dropped).
    pub items: Vec<PlacedItem>,
    pub remains: Vec<RemainsSave>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacedItem {
    pub at: [f32; 3],
    pub rot: [f32; 4],
    pub item: SavedItem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemainsSave {
    pub at: [f32; 3],
    pub items: Vec<SavedItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LoadoutSave {
    /// Body slot index and what's in it.
    pub slots: Vec<(usize, SavedItem)>,
    pub backpack: Vec<SavedItem>,
    /// What was in your hands goes back in your hands (or at your feet).
    pub hands: Vec<SavedItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Memorial {
    /// Convoys that got away with someone's kit.
    pub convoys: u32,
    /// Names of everything they took, oldest first.
    pub lost: Vec<String>,
    /// Bodies recovered before the convoy got them out.
    pub recovered: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedItem {
    pub id: String,
    pub condition: f32,
    #[serde(default)]
    pub rounds: u32,
    #[serde(default)]
    pub charge_wh: f32,
    /// Position inside a container, when it's in one.
    #[serde(default)]
    pub local: [f32; 3],
    #[serde(default)]
    pub gun: Option<SavedGun>,
    /// A magazine in the well (or later, attachments).
    #[serde(default)]
    pub inserted: Vec<SavedItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedGun {
    pub chambers: Vec<Option<Round>>,
    pub hammers: Vec<bool>,
    pub internal_rounds: u32,
    pub mode: usize,
    pub condition: f32,
    pub fouling: f32,
}

/// A whole career in one file: what you own and carry, your record and
/// the memorial (all shared), where the truck is headed next, and the
/// state of every valley you've raided (each its own persistent Zone).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Career {
    pub version: u32,
    /// The valley the truck drives to next (and the game opens in).
    pub map: String,
    pub stats: Stats,
    pub stash: Vec<SavedItem>,
    pub loadout: LoadoutSave,
    pub memorial: Memorial,
    pub zones: Vec<ZoneSave>,
    #[serde(default)]
    pub logbook: Option<String>,
    #[serde(default)]
    pub requests: RequestsSave,
    #[serde(default)]
    pub progress: Option<Progress>,
    /// The first-time moments the driver has already talked you through
    /// (one line each, once a career).
    #[serde(default)]
    pub firsts: Vec<String>,
}

impl Career {
    /// A career built around one game's save (a new career, or an old
    /// one-file-per-map save being brought in).
    pub fn from_game(save: &SaveGame) -> Self {
        let mut c = Career {
            version: SAVE_VERSION,
            ..Default::default()
        };
        c.store(save);
        c
    }

    /// The game for one valley: the shared parts, and that valley's Zone
    /// (unrolled, if it's never been visited).
    pub fn game(&self, map: &str) -> SaveGame {
        let mut save = SaveGame::new(map);
        save.stats = self.stats.clone();
        save.stash = self.stash.clone();
        save.loadout = self.loadout.clone();
        save.memorial = self.memorial.clone();
        save.logbook = self.logbook.clone();
        save.requests = self.requests.clone();
        save.progress = self.progress.clone();
        save.firsts = self.firsts.clone();
        if let Some(z) = self.zones.iter().find(|z| z.map == map) {
            save.zone = z.clone();
        }
        save
    }

    /// Take in a game's state: the shared parts, and its valley's Zone.
    pub fn store(&mut self, save: &SaveGame) {
        self.stats = save.stats.clone();
        self.stash = save.stash.clone();
        self.loadout = save.loadout.clone();
        self.memorial = save.memorial.clone();
        self.logbook = save.logbook.clone();
        self.requests = save.requests.clone();
        self.progress = save.progress.clone();
        self.firsts = save.firsts.clone();
        if self.map.is_empty() {
            self.map = save.zone.map.clone();
        }
        match self.zones.iter_mut().find(|z| z.map == save.zone.map) {
            Some(z) => *z = save.zone.clone(),
            None => self.zones.push(save.zone.clone()),
        }
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new().depth_limit(6))
    }

    pub fn from_ron(text: &str) -> Result<Self, String> {
        let c: Career = ron::from_str(text).map_err(|e| e.to_string())?;
        if c.version > SAVE_VERSION {
            return Err(format!(
                "save version {} is newer than this build ({SAVE_VERSION})",
                c.version
            ));
        }
        Ok(c)
    }
}

impl SaveGame {
    pub fn new(map: &str) -> Self {
        Self {
            version: SAVE_VERSION,
            zone: ZoneSave {
                map: map.to_owned(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new().depth_limit(6))
    }

    pub fn from_ron(text: &str) -> Result<Self, String> {
        let save: SaveGame = ron::from_str(text).map_err(|e| e.to_string())?;
        if save.version > SAVE_VERSION {
            return Err(format!(
                "save version {} is newer than this build ({SAVE_VERSION})",
                save.version
            ));
        }
        Ok(save)
    }

    /// Every item in the save, nested ones included.
    pub fn item_count(&self) -> usize {
        fn count(i: &SavedItem) -> usize {
            1 + i.inserted.iter().map(count).sum::<usize>()
        }
        let all = self
            .zone
            .items
            .iter()
            .map(|p| &p.item)
            .chain(self.zone.remains.iter().flat_map(|r| &r.items))
            .chain(&self.stash)
            .chain(self.loadout.slots.iter().map(|s| &s.1))
            .chain(&self.loadout.backpack)
            .chain(&self.loadout.hands);
        all.map(count).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str) -> SavedItem {
        SavedItem {
            id: id.into(),
            condition: 0.8,
            rounds: 0,
            charge_wh: 0.0,
            local: [0.0; 3],
            gun: None,
            inserted: Vec::new(),
        }
    }

    #[test]
    fn round_trips() {
        let mut s = SaveGame::new("marrow_creek");
        s.zone.clock = 123.5;
        let mut ak = item("ak74");
        ak.gun = Some(SavedGun {
            chambers: vec![Some(Round::Live)],
            hammers: vec![true],
            internal_rounds: 0,
            mode: 1,
            condition: 0.7,
            fouling: 0.2,
        });
        let mut mag = item("ak_mag");
        mag.rounds = 29;
        ak.inserted.push(mag);
        s.loadout.slots.push((0, ak));
        s.stash.push(item("cleaning_kit"));
        s.zone.items.push(PlacedItem {
            at: [1.0, 2.0, 3.0],
            rot: [0.0, 0.0, 0.0, 1.0],
            item: item("canned_stew"),
        });
        let text = s.to_ron().unwrap();
        let back = SaveGame::from_ron(&text).unwrap();
        assert_eq!(back.item_count(), 4);
        assert_eq!(back.loadout.slots[0].1, s.loadout.slots[0].1);
        assert_eq!(back.zone.clock, 123.5);
    }

    #[test]
    fn rejects_future_versions() {
        let mut s = SaveGame::new("x");
        s.version = SAVE_VERSION + 1;
        assert!(SaveGame::from_ron(&s.to_ron().unwrap()).is_err());
    }

    #[test]
    fn a_career_keeps_each_valley_and_shares_the_rest() {
        let mut marrow = SaveGame::new("marrow_creek");
        marrow.zone.rolled = true;
        marrow.zone.cycle = 3;
        marrow.stats.raids = 5;
        let mut career = Career::from_game(&marrow);
        assert_eq!(career.map, "marrow_creek");
        // Off to the other valley: a fresh Zone, the same stash and record.
        let mut kettle = career.game("kettle_pass");
        assert!(!kettle.zone.rolled);
        assert_eq!(kettle.stats.raids, 5);
        kettle.zone.rolled = true;
        kettle.stats.raids = 6;
        career.store(&kettle);
        assert_eq!(career.zones.len(), 2);
        // Back again: Marrow Creek as it was left, the record as it is now.
        let back = career.game("marrow_creek");
        assert_eq!(back.zone.cycle, 3);
        assert_eq!(back.stats.raids, 6);
        let text = career.to_ron().unwrap();
        let again = Career::from_ron(&text).unwrap();
        assert_eq!(again.zones.len(), 2);
    }

    #[test]
    fn a_career_from_before_jobs_still_loads() {
        let mut c = Career::from_game(&SaveGame::new("marrow_creek"));
        c.requests.number = 4;
        let text = c.to_ron().unwrap();
        let old: String = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("progress:"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(old, text, "the field was there to take out");
        let back = Career::from_ron(&old).unwrap();
        assert!(back.progress.is_none());
        assert_eq!(back.requests.number, 4);
        // And the jobs, once there are some, come back.
        let mut c = back;
        c.progress = Some(Progress {
            done: 2,
            upgrades: vec!["stash_rack".into()],
            ..Default::default()
        });
        let again = Career::from_ron(&c.to_ron().unwrap()).unwrap();
        assert_eq!(again.progress, c.progress);
        assert_eq!(
            again.game("kettle_pass").progress,
            c.progress,
            "every valley"
        );
    }
}
