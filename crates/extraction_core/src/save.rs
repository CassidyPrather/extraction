//! The save file: the Zone's persistent state between raids, the truck's
//! stash, what you were carrying, and the memorial board.
//!
//! Items are saved by catalog id (not by index) so the catalog can grow
//! without breaking saves. A gun saves its mechanism and whatever magazine
//! sits in its well, nested.

use serde::{Deserialize, Serialize};

use crate::firearm::Round;

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
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub raids: u32,
    pub extractions: u32,
    pub deaths: u32,
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
}
