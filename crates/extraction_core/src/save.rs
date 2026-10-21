//! The save file: the Zone's persistent state between raids, the truck's
//! stash, what you were carrying, and the memorial board.
//!
//! Items are saved by catalog id (not by index) so the catalog can grow
//! without breaking saves. A gun saves its mechanism and whatever magazine
//! sits in its well, nested.

use serde::{Deserialize, Serialize};

use crate::ammo::{Load, Loads};
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
    /// What's been broken (panes, doors, crates) and stays broken until
    /// the Surge: each by where it stood, centimetres.
    #[serde(default)]
    pub broken: Vec<[i32; 3]>,
    /// Holes torn in walls, by their centres, centimetres.
    #[serde(default)]
    pub breaches: Vec<[i32; 3]>,
    /// The valley's safehouse (`safehouse`), once you've started on it.
    #[serde(default)]
    pub safehouse: Option<crate::safehouse::SafehouseSave>,
    /// Heavies' wrecks, where they fell: landmarks until the Surge.
    #[serde(default)]
    pub wrecks: Vec<WreckSave>,
}

/// A heavy's wreck: where it lies, how it leans, what's still on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WreckSave {
    pub at: [f32; 3],
    pub yaw: f32,
    /// Its settled lean (scaled axis, world).
    pub lean: [f32; 3],
    /// Which plates are still on it (hatch, left, right).
    pub plates: [bool; 3],
    /// Its cell is still in it.
    pub cell: bool,
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
    /// What its rounds are loaded with, top last (empty: all standard).
    #[serde(default)]
    pub loads: Loads,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedGun {
    pub chambers: Vec<Option<Round>>,
    pub hammers: Vec<bool>,
    pub internal_rounds: u32,
    pub mode: usize,
    pub condition: f32,
    pub fouling: f32,
    /// The internal magazine's loads, and each chamber's.
    #[serde(default)]
    pub internal_loads: Loads,
    #[serde(default)]
    pub chamber_loads: Vec<Load>,
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

/// The most of anything a save may claim: counters past this are a
/// mangled file (and `+= 1` on `u32::MAX` panics in a debug build).
const MAX_COUNT: u32 = 1_000_000;
/// How far from the map's origin a saved position may be, metres.
const MAX_REACH: f32 = 100_000.0;

fn finite_or(v: f32, or: f32) -> f32 {
    if v.is_finite() { v } else { or }
}

fn sane_pos(p: [f32; 3]) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() < MAX_REACH)
}

fn clamp_count(v: &mut u32, what: &str, fixed: &mut Vec<String>) {
    if *v > MAX_COUNT {
        fixed.push(format!("{what} was {v}; set to {MAX_COUNT}"));
        *v = MAX_COUNT;
    }
}

impl SavedItem {
    /// Mends what a mangled file can put in an item (NaN, infinities,
    /// absurd counts), it and what's seated in it. Returns how many
    /// values were mended.
    pub fn sanitize(&mut self) -> usize {
        let mut n = 0;
        let mut fix = |v: &mut f32, lo: f32, hi: f32, or: f32| {
            let good = if v.is_finite() { v.clamp(lo, hi) } else { or };
            if good != *v && !(v.is_nan() && good.is_nan()) {
                *v = good;
                n += 1;
            }
        };
        fix(&mut self.condition, 0.0, 1.0, 0.5);
        // Negative means "the kind's default charge".
        fix(&mut self.charge_wh, -1.0, 1.0e6, -1.0);
        for c in &mut self.local {
            fix(c, -MAX_REACH, MAX_REACH, 0.0);
        }
        if let Some(g) = &mut self.gun {
            fix(&mut g.condition, 0.0, 1.0, 0.5);
            fix(&mut g.fouling, 0.0, 1.0, 0.0);
            if g.internal_rounds > 10_000 {
                g.internal_rounds = 10_000;
                n += 1;
            }
            if g.internal_loads.0.len() > 10_000 {
                g.internal_loads.0.truncate(10_000);
                n += 1;
            }
        }
        if self.rounds > 10_000 {
            self.rounds = 10_000;
            n += 1;
        }
        if self.loads.0.len() > self.rounds as usize {
            self.loads.0.truncate(self.rounds as usize);
            n += 1;
        }
        n + self
            .inserted
            .iter_mut()
            .map(|i| i.sanitize())
            .sum::<usize>()
    }
}

impl ZoneSave {
    /// Mends a valley's state read from a file: drops what lies nowhere
    /// (NaN or a world away), mends the rest. Says what it did.
    pub fn sanitize(&mut self, fixed: &mut Vec<String>) {
        if !self.clock.is_finite() || self.clock < 0.0 || self.clock > 1.0e7 {
            fixed.push(format!("{}: Zone clock {} reset", self.map, self.clock));
            self.clock = finite_or(self.clock, 0.0).clamp(0.0, 1.0e7);
        }
        clamp_count(&mut self.cycle, "Surge cycle", fixed);
        let before = self.items.len();
        self.items.retain(|p| sane_pos(p.at));
        if self.items.len() < before {
            fixed.push(format!(
                "{}: {} item(s) lying nowhere dropped",
                self.map,
                before - self.items.len()
            ));
        }
        let mut mended = 0;
        for p in &mut self.items {
            let q = p.rot;
            let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
            if !len.is_finite() || len < 1.0e-3 {
                p.rot = [0.0, 0.0, 0.0, 1.0];
                mended += 1;
            } else if (len - 1.0).abs() > 1.0e-3 {
                p.rot = q.map(|v| v / len);
                mended += 1;
            }
            mended += p.item.sanitize();
        }
        let before = self.remains.len();
        self.remains.retain(|r| sane_pos(r.at));
        if self.remains.len() < before {
            fixed.push(format!(
                "{}: {} body bag(s) lying nowhere dropped",
                self.map,
                before - self.remains.len()
            ));
        }
        for r in &mut self.remains {
            mended += r.items.iter_mut().map(|i| i.sanitize()).sum::<usize>();
        }
        self.wrecks.retain(|w| {
            sane_pos(w.at) && w.yaw.is_finite() && w.lean.iter().all(|v| v.is_finite())
        });
        if let Some(h) = &mut self.safehouse {
            mended += h.stash.iter_mut().map(|i| i.sanitize()).sum::<usize>();
        }
        if mended > 0 {
            fixed.push(format!("{}: {mended} item value(s) mended", self.map));
        }
    }
}

impl Stats {
    fn sanitize(&mut self, fixed: &mut Vec<String>) {
        clamp_count(&mut self.raids, "raids", fixed);
        clamp_count(&mut self.extractions, "extractions", fixed);
        clamp_count(&mut self.deaths, "deaths", fixed);
    }
}

fn sanitize_shared(
    stash: &mut [SavedItem],
    loadout: &mut LoadoutSave,
    requests: &mut RequestsSave,
    fixed: &mut Vec<String>,
) {
    let mended: usize = stash
        .iter_mut()
        .chain(loadout.slots.iter_mut().map(|s| &mut s.1))
        .chain(loadout.backpack.iter_mut())
        .chain(loadout.hands.iter_mut())
        .map(|i| i.sanitize())
        .sum();
    if mended > 0 {
        fixed.push(format!("{mended} carried item value(s) mended"));
    }
    clamp_count(&mut requests.number, "requests", fixed);
    if requests
        .cache_at
        .as_ref()
        .is_some_and(|(_, p)| !p.iter().all(|v| v.is_finite()))
    {
        requests.cache_at = None;
        fixed.push("the driver's cache had no place; forgotten".into());
    }
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

    /// Mends what a mangled (but parseable) file can hold: NaN and
    /// infinite numbers, absurd counts, things lying nowhere. Returns a
    /// line for each kind of repair (empty: the file was sound). Loading
    /// calls it, so nothing past here sees a NaN from a file.
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut fixed = Vec::new();
        self.stats.sanitize(&mut fixed);
        sanitize_shared(
            &mut self.stash,
            &mut self.loadout,
            &mut self.requests,
            &mut fixed,
        );
        for z in &mut self.zones {
            z.sanitize(&mut fixed);
        }
        fixed
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

    /// See [`Career::sanitize`].
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut fixed = Vec::new();
        self.stats.sanitize(&mut fixed);
        sanitize_shared(
            &mut self.stash,
            &mut self.loadout,
            &mut self.requests,
            &mut fixed,
        );
        self.zone.sanitize(&mut fixed);
        fixed
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
            loads: Loads::default(),
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
            internal_loads: Loads::default(),
            chamber_loads: vec![Load::Ap],
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

    /// A magazine of ball with AP on top and a tracer over that keeps its
    /// order through a save; a save from before loads were kept still
    /// reads (all ball).
    #[test]
    fn a_mixed_magazine_keeps_its_order() {
        let mut s = SaveGame::new("marrow_creek");
        let mut mag = item("ak_mag");
        mag.rounds = 30;
        mag.loads.fill(27, Load::Ap, 2);
        mag.loads.push(29, Load::Tracer);
        s.stash.push(mag.clone());
        let back = SaveGame::from_ron(&s.to_ron().unwrap()).unwrap();
        assert_eq!(
            back.stash[0].loads.0,
            vec![Load::Ap, Load::Ap, Load::Tracer]
        );
        // An older save: no `loads` anywhere.
        s.stash[0].loads = Loads::default();
        let old: String = s
            .to_ron()
            .unwrap()
            .lines()
            .filter(|l| !l.trim_start().starts_with("loads:"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!old.contains("loads"));
        let back = SaveGame::from_ron(&old).unwrap();
        assert!(back.stash[0].loads.0.is_empty());
    }

    /// A career with something in every field a file can carry.
    fn rich_career() -> Career {
        let mut s = SaveGame::new("marrow_creek");
        s.zone.rolled = true;
        s.zone.clock = 812.25;
        s.zone.cycle = 2;
        s.stats.raids = 9;
        let mut ak = item("ak74");
        ak.gun = Some(SavedGun {
            chambers: vec![Some(Round::Live)],
            hammers: vec![true],
            internal_rounds: 0,
            mode: 1,
            condition: 0.7,
            fouling: 0.2,
            internal_loads: Loads::default(),
            chamber_loads: vec![Load::Ap],
        });
        let mut mag = item("ak_mag");
        mag.rounds = 29;
        mag.loads.fill(27, Load::Ap, 2);
        ak.inserted.push(mag);
        s.loadout.slots.push((0, ak));
        s.loadout.backpack.push(item("canned_stew"));
        s.loadout.hands.push(item("radio"));
        s.stash.push(item("cleaning_kit"));
        for i in 0..4 {
            s.zone.items.push(PlacedItem {
                at: [10.0 + i as f32, 2.0, -30.5],
                rot: [0.0, 0.38, 0.0, 0.92],
                item: item("bolt"),
            });
        }
        s.zone.remains.push(RemainsSave {
            at: [5.0, 1.0, 5.0],
            items: vec![item("pm_pistol")],
        });
        s.zone.broken.push([1, 2, 3]);
        s.zone.wrecks.push(WreckSave {
            at: [3.0, 0.0, 4.0],
            yaw: 1.0,
            lean: [0.1, 0.0, 0.0],
            plates: [true, false, true],
            cell: true,
        });
        s.requests.cache_at = Some(("marrow_creek".into(), [1.0, 2.0]));
        s.memorial.lost.push("ak74".into());
        let mut c = Career::from_game(&s);
        c.zones.push(SaveGame::new("kettle_pass").zone);
        c
    }

    /// Every float in a career, for checking none is NaN or infinite.
    fn floats(c: &Career) -> Vec<f32> {
        fn item(i: &SavedItem, out: &mut Vec<f32>) {
            out.push(i.condition);
            out.push(i.charge_wh);
            out.extend(i.local);
            if let Some(g) = &i.gun {
                out.push(g.condition);
                out.push(g.fouling);
            }
            for x in &i.inserted {
                item(x, out);
            }
        }
        let mut out = Vec::new();
        for i in c
            .stash
            .iter()
            .chain(c.loadout.slots.iter().map(|s| &s.1))
            .chain(&c.loadout.backpack)
            .chain(&c.loadout.hands)
        {
            item(i, &mut out);
        }
        for z in &c.zones {
            out.push(z.clock);
            for p in &z.items {
                out.extend(p.at);
                out.extend(p.rot);
                item(&p.item, &mut out);
            }
            for r in &z.remains {
                out.extend(r.at);
                for i in &r.items {
                    item(i, &mut out);
                }
            }
            for w in &z.wrecks {
                out.extend(w.at);
                out.push(w.yaw);
                out.extend(w.lean);
            }
        }
        out
    }

    /// Reading a mangled file never panics; whatever reads is mended to
    /// finite numbers and writes back out as a file that reads again.
    fn check_mangled(text: &str, what: &str) -> bool {
        let read = std::panic::catch_unwind(|| Career::from_ron(text));
        let Ok(read) = read else {
            panic!("{what}: from_ron panicked on:\n{text}");
        };
        let Ok(mut c) = read else {
            return false;
        };
        c.sanitize();
        let bad: Vec<f32> = floats(&c).into_iter().filter(|v| !v.is_finite()).collect();
        assert!(
            bad.is_empty(),
            "{what}: still not finite after mending: {bad:?}"
        );
        assert!(c.sanitize().is_empty(), "{what}: mending is idempotent");
        let again = c.to_ron().expect("writes");
        Career::from_ron(&again).unwrap_or_else(|e| panic!("{what}: rewrite unreadable: {e}"));
        true
    }

    /// The save fuzzer (`docs/SOAK.md`): truncation everywhere, byte
    /// flips, deleted lines, and numbers swapped for poison.
    #[test]
    fn mangled_saves_never_panic_and_mend_to_sound_ones() {
        let text = rich_career().to_ron().unwrap();
        assert!(check_mangled(&text, "untouched"));
        // Cut short anywhere (a crash mid-write, a full disk).
        let mut read = 0;
        for cut in (0..text.len()).step_by(7) {
            if text.is_char_boundary(cut) {
                read += check_mangled(&text[..cut], &format!("cut at {cut}")) as usize;
            }
        }
        assert!(read <= 2, "a truncated save hardly ever reads ({read})");
        // A cheap deterministic generator.
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let bytes = text.as_bytes();
        for round in 0..1500 {
            let mut b = bytes.to_vec();
            for _ in 0..1 + next(4) {
                let i = next(b.len());
                b[i] = match next(4) {
                    0 => b[i] ^ (1 << next(8)),
                    1 => b"0123456789"[next(10)],
                    2 => b"()[]{},:-.\"e"[next(12)],
                    _ => b' ',
                };
            }
            let t = String::from_utf8_lossy(&b);
            check_mangled(&t, &format!("flip round {round}"));
        }
        // Whole lines gone (a hand edit, a merge gone wrong).
        let lines: Vec<&str> = text.lines().collect();
        for i in 0..lines.len() {
            let t: Vec<&str> = lines
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, l)| *l)
                .collect();
            check_mangled(&t.join("\n"), &format!("line {i} gone"));
        }
        // Numbers swapped for poison: every number, one at a time.
        let poison = [
            "NaN",
            "inf",
            "-inf",
            "1e39",
            "-1",
            "4294967295",
            "4294967296",
            "-0.0",
            "99999999999",
        ];
        let mut mended = 0;
        let numbers: Vec<(usize, usize)> = {
            let mut v = Vec::new();
            let b = text.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i].is_ascii_digit()
                    && (i == 0 || !b[i - 1].is_ascii_alphanumeric() && b[i - 1] != b'_')
                {
                    let start = if i > 0 && b[i - 1] == b'-' { i - 1 } else { i };
                    let mut j = i;
                    while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.' || b[j] == b'e') {
                        j += 1;
                    }
                    v.push((start, j));
                    i = j;
                } else {
                    i += 1;
                }
            }
            v
        };
        assert!(numbers.len() > 40, "found the numbers ({})", numbers.len());
        for (k, (a, b)) in numbers.iter().enumerate() {
            let p = poison[k % poison.len()];
            let t = format!("{}{}{}", &text[..*a], p, &text[*b..]);
            if check_mangled(&t, &format!("number {k} -> {p}")) {
                mended += 1;
            }
        }
        assert!(mended > 10, "some poison parses, and is mended ({mended})");
        // A deep nest of brackets must not blow the stack.
        let deep = format!("{}{}", "(".repeat(100_000), ")".repeat(100_000));
        check_mangled(&deep, "deep nest");
    }

    /// The poison a parser lets through: NaN and infinities in positions,
    /// clocks and conditions; counters at `u32::MAX`. Mended, with a line
    /// saying so, and loose things lying nowhere are dropped (counted).
    #[test]
    fn poison_values_are_mended_and_reported() {
        let mut c = rich_career();
        c.stats.raids = u32::MAX;
        let z = &mut c.zones[0];
        z.clock = f32::NAN;
        z.items[0].at[1] = f32::INFINITY;
        z.items[1].rot = [0.0; 4];
        z.items[2].item.condition = f32::NAN;
        z.remains[0].at[0] = f32::NAN;
        c.stash[0].charge_wh = f32::INFINITY;
        c.loadout.slots[0].1.inserted[0].rounds = 4_000_000_000;
        let text = c.to_ron().unwrap();
        let mut back = Career::from_ron(&text).expect("RON writes NaN and inf readably");
        let fixed = back.sanitize();
        assert!(fixed.len() >= 5, "{fixed:#?}");
        let z = &back.zones[0];
        assert_eq!(z.clock, 0.0);
        assert_eq!(z.items.len(), 3, "the one at infinity is gone");
        assert_eq!(z.items[0].rot, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(z.items[1].item.condition, 0.5);
        assert!(z.remains.is_empty());
        assert_eq!(back.stash[0].charge_wh, -1.0, "the kind's default charge");
        assert_eq!(back.stats.raids, MAX_COUNT);
        assert!(floats(&back).iter().all(|v| v.is_finite()));
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
