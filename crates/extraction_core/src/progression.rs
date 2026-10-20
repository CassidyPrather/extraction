//! The long loop: the driver's jobs, in tiers, and what the driver fits
//! to the truck when a tier is done (see `docs/PROGRESSION.md`).
//!
//! Nothing here is a currency. A job is a card with one thing on it to do;
//! the count of jobs done is what moves you up a tier, and each tier ends
//! with something bolted into the truck bed. Jobs are rolled from the
//! valleys' own regions and points of interest, deterministically from the
//! career seed and the job's number.

use serde::{Deserialize, Serialize};

use crate::map::{MapDef, MarkerKind};
use crate::noise::Rng;

/// Something the driver fits to the truck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    /// A rack over the stash crate: more room.
    StashRack,
    /// A vice on the bench: more recipes.
    BenchVice,
    /// A whip aerial on the cab: earlier warnings.
    Aerial,
    /// A medicine cabinet: a dressing in it every raid.
    MedCabinet,
    /// A shelf on the wall: more room again.
    StashRack2,
}

impl Upgrade {
    pub const ALL: [Upgrade; 5] = [
        Upgrade::StashRack,
        Upgrade::BenchVice,
        Upgrade::Aerial,
        Upgrade::MedCabinet,
        Upgrade::StashRack2,
    ];

    /// Stable id, as saved.
    pub fn id(self) -> &'static str {
        match self {
            Upgrade::StashRack => "stash_rack",
            Upgrade::BenchVice => "bench_vice",
            Upgrade::Aerial => "aerial",
            Upgrade::MedCabinet => "med_cabinet",
            Upgrade::StashRack2 => "stash_rack_2",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|u| u.id() == id)
    }

    /// As the driver says it: "I've put up ...".
    pub fn name(self) -> &'static str {
        match self {
            Upgrade::StashRack => "a rack over the stash",
            Upgrade::BenchVice => "a vice on the bench",
            Upgrade::Aerial => "a whip aerial on the cab",
            Upgrade::MedCabinet => "a medicine cabinet",
            Upgrade::StashRack2 => "a shelf by the stash",
        }
    }

    /// What it does, in a few words, for the career page.
    pub fn does(self) -> &'static str {
        match self {
            Upgrade::StashRack => "stash +100 L",
            Upgrade::BenchVice => "six more recipes",
            Upgrade::Aerial => "Surge warning at 10 min",
            Upgrade::MedCabinet => "a dressing each raid",
            Upgrade::StashRack2 => "stash +100 L",
        }
    }
}

/// What kinds of job a tier hands out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Fetch,
    Recover,
    Scout,
}

/// One rung of the ladder.
#[derive(Clone, Debug)]
pub struct Tier {
    pub name: &'static str,
    /// Jobs to finish it.
    pub jobs: u32,
    /// Danger of the regions its jobs send you to (inclusive).
    pub danger: (u8, u8),
    /// Its jobs are in another valley.
    pub far: bool,
    /// Its kinds of job, drawn evenly.
    pub kinds: &'static [JobKind],
    /// Fitted when it's done.
    pub fits: &'static [Upgrade],
}

pub const TIERS: [Tier; 4] = [
    Tier {
        name: "Odd jobs",
        jobs: 3,
        danger: (1, 1),
        far: false,
        kinds: &[JobKind::Fetch, JobKind::Scout],
        fits: &[Upgrade::StashRack],
    },
    Tier {
        name: "Errands",
        jobs: 3,
        danger: (1, 2),
        far: false,
        kinds: &[JobKind::Fetch, JobKind::Recover, JobKind::Scout],
        fits: &[Upgrade::BenchVice],
    },
    Tier {
        name: "Runs",
        jobs: 4,
        danger: (2, 3),
        far: false,
        kinds: &[JobKind::Recover, JobKind::Scout, JobKind::Fetch],
        fits: &[Upgrade::Aerial],
    },
    Tier {
        name: "Far jobs",
        jobs: 4,
        danger: (2, 3),
        far: true,
        kinds: &[JobKind::Recover, JobKind::Scout],
        fits: &[Upgrade::MedCabinet, Upgrade::StashRack2],
    },
];

/// After the last tier: the driver keeps asking.
pub const STANDING: Tier = Tier {
    name: "Standing jobs",
    jobs: 0,
    danger: (2, 3),
    far: false,
    kinds: &[JobKind::Fetch, JobKind::Recover, JobKind::Scout],
    fits: &[],
};

/// The stash crate's room before any rack, litres of bulk.
pub const STASH_BASE: f32 = 200.0;
/// Each rack adds this.
pub const STASH_RACK: f32 = 100.0;

/// The last job of the ladder: what's on the table in the house by the
/// well in Marrow Creek (Hana's letter, `docs/LORE.md`).
pub const LETTER_ITEM: &str = "hana_letter";
pub const LETTER_MAP: &str = "marrow_creek";
/// The "Village note" on that table.
pub const LETTER_AT: [f32; 2] = [-17.3, 93.7];

/// Raids the driver waits on a fetch before asking for something else.
pub const FETCH_PATIENCE: u32 = 3;

/// What a Recover job sends you for.
pub const RECOVER_ITEM: &str = "field_recorder";
/// A Scout job's place counts once you're this close to it.
pub const SCOUT_RADIUS: f32 = 15.0;

/// What the driver asks you to fetch, by tier (from 0): item id, how many
/// (min, max), and why.
pub const WANTS: [&[(&str, u32, u32, &str)]; 3] = [
    &[
        ("canned_stew", 2, 3, "Something hot that isn't my cooking."),
        ("scrap_metal", 3, 5, "Scrap. The tailgate hinge is going."),
        ("field_dressing", 2, 3, "Dressings. Not for me. Don't ask."),
        ("gun_oil", 1, 1, "Gun oil. The winch is seizing."),
        ("matches", 2, 3, "Matches. Out again."),
        ("tea_tin", 1, 1, "Tea. Real tea."),
    ],
    &[
        (
            "sensor_eye",
            1,
            2,
            "Robot eyes. Someone in town pays in diesel.",
        ),
        ("electronics", 2, 3, "Boards, any. The radio's dying."),
        (
            "robot_cell",
            1,
            1,
            "One of their cells. I want to see inside.",
        ),
        ("servo", 1, 2, "A servo or two. I'm making a thing."),
        (
            ANY_PARTS,
            2,
            3,
            "Robot parts, any. Eyes, servos, cells. I'm not fussy.",
        ),
    ],
    &[
        (
            "robot_gunpod",
            1,
            1,
            "One of their gun pods. Whole, if you can.",
        ),
        ("robot_cell", 2, 2, "Two of their cells. The cab heater."),
        (
            ANY_PARTS,
            4,
            4,
            "Four of their parts, any. A man in town is building one.",
        ),
        ("servo", 2, 2, "Two servos. The winch, properly this time."),
        (
            "regen_injector",
            1,
            1,
            "One of those injectors. For my sister.",
        ),
    ],
];

/// A fetch of this "item" takes any of [`PARTS`]: robots don't drop the
/// same thing in every valley.
pub const ANY_PARTS: &str = "robot_parts";
/// What counts as a robot part.
pub const PARTS: &[&str] = &[
    "sensor_eye",
    "servo",
    "robot_cell",
    "rotor_assembly",
    "robot_gunpod",
    "robot_ammo_belt",
];

/// Whether an item of `id` counts toward a fetch of `want`.
pub fn fetch_takes(want: &str, id: &str) -> bool {
    want == id || (want == ANY_PARTS && PARTS.contains(&id))
}

/// Why the driver wants it (any tier's reason for that item).
pub fn why(id: &str) -> &'static str {
    WANTS
        .iter()
        .flat_map(|t| t.iter())
        .find(|w| w.0 == id)
        .map_or("You know what for.", |w| w.3)
}

/// A job on the card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Job {
    /// Hand over `n` of `item` from the stash.
    Fetch { item: String, n: u32 },
    /// Bring home the recorder left in `region` of `map`. Where exactly
    /// (`at`, and `place` as words) is settled when it's put there.
    Recover {
        map: String,
        region: String,
        #[serde(default)]
        at: Option<[f32; 2]>,
        #[serde(default)]
        place: Option<String>,
    },
    /// Bring home what's on the table in the house by the well
    /// ([`LETTER_ITEM`]): the last job of the last tier.
    Letter,
    /// Go and look at `place` in `map`, then get home.
    Scout {
        map: String,
        place: String,
        at: [f32; 2],
        #[serde(default)]
        seen: bool,
    },
}

impl Job {
    /// The valley it's in, if it's in one.
    pub fn map(&self) -> Option<&str> {
        match self {
            Job::Fetch { .. } => None,
            Job::Letter => Some(LETTER_MAP),
            Job::Recover { map, .. } | Job::Scout { map, .. } => Some(map),
        }
    }

    /// Where it's pencilled on the field map, if anywhere yet.
    pub fn mark(&self) -> Option<(&str, [f32; 2])> {
        match self {
            Job::Fetch { .. } => None,
            Job::Letter => Some((LETTER_MAP, LETTER_AT)),
            Job::Recover { map, at, .. } => at.map(|a| (map.as_str(), a)),
            Job::Scout { map, at, .. } => Some((map.as_str(), *at)),
        }
    }
}

/// The career's standing with the driver.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// Index into [`TIERS`]; past the end, standing jobs.
    pub tier: u32,
    /// Jobs done in this tier.
    pub done_in_tier: u32,
    /// Jobs done, all told.
    pub done: u32,
    /// What's been fitted (upgrade ids).
    pub upgrades: Vec<String>,
    /// The job on the card.
    pub job: Option<Job>,
    /// Raids set off on, by valley.
    #[serde(default)]
    pub raids_by_map: Vec<(String, u32)>,
    /// The raid the medicine cabinet was last stocked for.
    #[serde(default)]
    pub cabinet_raid: u32,
    /// The raid the last job was handed in after: one job a raid.
    #[serde(default)]
    pub last_raid: Option<u32>,
    /// The raid count when the job went on the card.
    #[serde(default)]
    pub job_raid: u32,
    /// Fetches the driver gave up waiting for (varies the next roll).
    #[serde(default)]
    pub swaps: u32,
}

impl Progress {
    pub fn has(&self, u: Upgrade) -> bool {
        self.upgrades.iter().any(|id| id == u.id())
    }

    /// The tier being worked on (standing jobs once they're all done).
    pub fn tier_def(&self) -> &'static Tier {
        TIERS.get(self.tier as usize).unwrap_or(&STANDING)
    }

    /// The tier's jobs are all that's left before the next fitting.
    pub fn next_fitting(&self) -> &'static [Upgrade] {
        self.tier_def().fits
    }

    /// The stash crate's room, litres of bulk.
    pub fn stash_capacity(&self) -> f32 {
        let racks = [Upgrade::StashRack, Upgrade::StashRack2]
            .into_iter()
            .filter(|u| self.has(*u))
            .count();
        STASH_BASE + STASH_RACK * racks as f32
    }

    /// The bench's recipe tier (0: as it came; 1: with the vice).
    pub fn bench_tier(&self) -> u8 {
        self.has(Upgrade::BenchVice) as u8
    }

    /// A job done. Returns what's fitted for it, if it finished a tier.
    pub fn complete(&mut self) -> &'static [Upgrade] {
        self.done += 1;
        self.job = None;
        let Some(tier) = TIERS.get(self.tier as usize) else {
            return &[];
        };
        self.done_in_tier += 1;
        if self.done_in_tier < tier.jobs {
            return &[];
        }
        self.tier += 1;
        self.done_in_tier = 0;
        for u in tier.fits {
            if !self.has(*u) {
                self.upgrades.push(u.id().to_owned());
            }
        }
        tier.fits
    }

    /// An older career's requests met before there were tiers, counted as
    /// jobs done (so a veteran starts where they'd got to).
    pub fn credit(&mut self, jobs: u32) {
        for _ in 0..jobs {
            self.complete();
        }
    }

    /// A fetch still unmet after [`FETCH_PATIENCE`] raids: the driver
    /// wants something else (some parts may just not be turning up).
    pub fn fetch_gone_stale(&self, raids: u32) -> bool {
        matches!(self.job, Some(Job::Fetch { .. })) && raids >= self.job_raid + FETCH_PATIENCE
    }

    pub fn count_raid(&mut self, map: &str) {
        match self.raids_by_map.iter_mut().find(|(m, _)| m == map) {
            Some((_, n)) => *n += 1,
            None => self.raids_by_map.push((map.to_owned(), 1)),
        }
    }

    pub fn raids_in(&self, map: &str) -> u32 {
        self.raids_by_map
            .iter()
            .find(|(m, _)| m == map)
            .map_or(0, |(_, n)| *n)
    }
}

/// A valley the jobs can send you to.
pub struct Valley<'a> {
    pub id: &'a str,
    pub map: &'a MapDef,
}

/// Points of interest in regions of danger `lo..=hi`: (name, at).
fn places(map: &MapDef, (lo, hi): (u8, u8)) -> Vec<(String, [f32; 2])> {
    map.markers_of(&MarkerKind::Poi)
        .filter(|m| !m.name.is_empty())
        .filter(|m| {
            map.region_at(m.at)
                .is_some_and(|r| (lo..=hi).contains(&r.danger))
        })
        .map(|m| (m.name.clone(), m.at))
        .collect()
}

/// Regions of danger `lo..=hi` with a building in them (somewhere to leave
/// a recorder).
fn regions(map: &MapDef, (lo, hi): (u8, u8)) -> Vec<String> {
    map.regions
        .iter()
        .filter(|r| (lo..=hi).contains(&r.danger))
        .filter(|r| {
            map.placements
                .iter()
                .any(|p| crate::geom::point_in_polygon(&r.polygon, p.at) && is_building(&p.prefab))
        })
        .map(|r| r.name.clone())
        .collect()
}

/// Prefabs with a roof and furniture (loot spots) in them.
pub fn is_building(prefab: &str) -> bool {
    matches!(
        prefab,
        "house_plank"
            | "house_brick"
            | "cabin_log"
            | "farmhouse"
            | "barn"
            | "shed"
            | "shop"
            | "pump_station"
            | "bus_depot"
            | "mast_hut"
            | "rail_shed"
            | "guard_booth"
            | "ranger_tower"
            | "factory_hall"
            | "sawmill"
            | "signal_box"
    )
}

/// The first job of a career: look at the danger-1 place nearest the
/// insertion. It teaches the loop in one raid.
pub fn first_job(id: &str, map: &MapDef) -> Option<Job> {
    let ins = map.markers_of(&MarkerKind::Insertion).next()?.at;
    places(map, (1, 1))
        .into_iter()
        .min_by(|a, b| crate::geom::dist(a.1, ins).total_cmp(&crate::geom::dist(b.1, ins)))
        .map(|(place, at)| Job::Scout {
            map: id.to_owned(),
            place,
            at,
            seen: false,
        })
}

/// The next job for `progress`, rolled from the career seed and the job's
/// number. `here` is the valley the truck is in; `valleys` every valley
/// jobs may send you to (it should include `here`).
pub fn roll_job(progress: &Progress, seed: u64, here: &str, valleys: &[Valley]) -> Job {
    let number = progress.done + 1;
    if progress.done == 0 {
        if let Some(job) = valleys
            .iter()
            .find(|v| v.id == here)
            .and_then(|v| first_job(v.id, v.map))
        {
            return job;
        }
    }
    let tier = progress.tier_def();
    if progress.tier as usize == TIERS.len() - 1 && progress.done_in_tier + 1 == tier.jobs {
        return Job::Letter;
    }
    let roll = number as u64 + 1000 * progress.swaps as u64;
    let mut rng = Rng::new(seed ^ 0x10B5_0F7E ^ roll.wrapping_mul(0x9E37_79B9));
    // Which valley: here, or for far jobs the one you've raided least.
    let valley = if tier.far && valleys.len() > 1 {
        valleys
            .iter()
            .filter(|v| v.id != here)
            .min_by_key(|v| (progress.raids_in(v.id), rng.below(100)))
    } else {
        valleys.iter().find(|v| v.id == here)
    };
    let kind = tier.kinds[rng.below(tier.kinds.len())];
    if let Some(v) = valley {
        match kind {
            JobKind::Scout => {
                let p = places(v.map, tier.danger);
                if !p.is_empty() {
                    let (place, at) = p[rng.below(p.len())].clone();
                    return Job::Scout {
                        map: v.id.to_owned(),
                        place,
                        at,
                        seen: false,
                    };
                }
            }
            JobKind::Recover => {
                let r = regions(v.map, tier.danger);
                if !r.is_empty() {
                    return Job::Recover {
                        map: v.id.to_owned(),
                        region: r[rng.below(r.len())].clone(),
                        at: None,
                        place: None,
                    };
                }
            }
            JobKind::Fetch => {}
        }
    }
    let wants = WANTS[(progress.tier as usize).min(WANTS.len() - 1)];
    let (id, lo, hi, _) = wants[rng.below(wants.len())];
    Job::Fetch {
        item: id.to_owned(),
        n: lo + rng.below((hi - lo + 1) as usize) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{CINDER_WORKS, KETTLE_PASS, MARROW_CREEK};

    fn valleys() -> Vec<(&'static str, MapDef)> {
        [
            ("marrow_creek", MARROW_CREEK),
            ("kettle_pass", KETTLE_PASS),
            ("cinder_works", CINDER_WORKS),
        ]
        .into_iter()
        .map(|(id, text)| (id, MapDef::from_ron(text).unwrap()))
        .collect()
    }

    fn as_valleys<'a>(v: &'a [(&'static str, MapDef)]) -> Vec<Valley<'a>> {
        v.iter().map(|(id, map)| Valley { id, map }).collect()
    }

    #[test]
    fn the_first_job_is_a_look_at_somewhere_near() {
        let v = valleys();
        let names: Vec<String> = v
            .iter()
            .map(|(id, map)| match first_job(id, map) {
                Some(Job::Scout { place, .. }) => place,
                other => panic!("{id}: {other:?}"),
            })
            .collect();
        println!("first jobs: {names:?}");
        let job = roll_job(&Progress::default(), 7, "marrow_creek", &as_valleys(&v));
        assert!(matches!(job, Job::Scout { .. }), "{job:?}");
    }

    #[test]
    fn every_tier_has_jobs_in_every_valley() {
        let v = valleys();
        for (id, map) in &v {
            for tier in TIERS.iter().chain([&STANDING]) {
                if tier.kinds.contains(&JobKind::Scout) {
                    assert!(!places(map, tier.danger).is_empty(), "{id}: {}", tier.name);
                }
                if tier.kinds.contains(&JobKind::Recover) {
                    assert!(!regions(map, tier.danger).is_empty(), "{id}: {}", tier.name);
                }
            }
        }
    }

    #[test]
    fn a_tier_done_fits_its_upgrade_and_climbs() {
        let mut p = Progress::default();
        assert_eq!(p.stash_capacity(), STASH_BASE);
        assert!(p.complete().is_empty());
        assert!(p.complete().is_empty());
        assert_eq!(p.complete(), &[Upgrade::StashRack]);
        assert_eq!(p.tier, 1);
        assert_eq!(p.stash_capacity(), STASH_BASE + STASH_RACK);
        let all: u32 = TIERS.iter().map(|t| t.jobs).sum();
        p.credit(all);
        assert_eq!(p.tier as usize, TIERS.len());
        for u in Upgrade::ALL {
            assert!(p.has(u), "{u:?}");
        }
        assert_eq!(p.stash_capacity(), STASH_BASE + 2.0 * STASH_RACK);
        assert_eq!(p.tier_def().name, STANDING.name);
        // Standing jobs go on, fitting nothing more.
        assert!(p.complete().is_empty());
    }

    #[test]
    fn jobs_follow_the_danger_ladder_and_far_jobs_leave_the_valley() {
        let v = valleys();
        let valleys = as_valleys(&v);
        let marrow = &v[0].1;
        for tier in 0..TIERS.len() as u32 {
            let t = &TIERS[tier as usize];
            for done in 1..40 {
                let p = Progress {
                    tier,
                    done,
                    ..Default::default()
                };
                let job = roll_job(&p, 99, "marrow_creek", &valleys);
                if t.far {
                    assert!(
                        job.map().is_none_or(|m| m != "marrow_creek"),
                        "{}: {job:?}",
                        t.name
                    );
                }
                if let (Job::Scout { at, .. }, false) = (&job, t.far) {
                    let d = marrow.region_at(*at).unwrap().danger;
                    assert!((t.danger.0..=t.danger.1).contains(&d), "{job:?}");
                }
            }
        }
    }

    #[test]
    fn the_ladder_ends_with_the_letter() {
        let v = valleys();
        let last = TIERS.len() - 1;
        let p = Progress {
            tier: last as u32,
            done_in_tier: TIERS[last].jobs - 1,
            done: 13,
            ..Default::default()
        };
        assert_eq!(roll_job(&p, 3, "kettle_pass", &as_valleys(&v)), Job::Letter);
        let marrow = &v[0].1;
        let note = marrow
            .markers
            .iter()
            .find(|m| m.name == "Village note")
            .expect("the note on the table");
        assert!(crate::geom::dist(note.at, LETTER_AT) < 0.5);
    }

    #[test]
    fn any_parts_takes_any_robot_part() {
        assert!(fetch_takes(ANY_PARTS, "servo"));
        assert!(fetch_takes(ANY_PARTS, "sensor_eye"));
        assert!(!fetch_takes(ANY_PARTS, "canned_stew"));
        assert!(fetch_takes("servo", "servo"));
        assert!(!fetch_takes("servo", "sensor_eye"));
        let cat = crate::items::ItemCatalog::from_ron(crate::items::BUILTIN_CATALOG).unwrap();
        for id in PARTS {
            assert!(cat.kind(id).is_some(), "{id}");
        }
        for tier in WANTS {
            for w in tier.iter() {
                assert!(w.0 == ANY_PARTS || cat.kind(w.0).is_some(), "{}", w.0);
            }
        }
    }

    #[test]
    fn jobs_round_trip_through_ron() {
        let p = Progress {
            job: Some(Job::Recover {
                map: "kettle_pass".into(),
                region: "The quarry".into(),
                at: Some([1.0, 2.0]),
                place: None,
            }),
            upgrades: vec!["stash_rack".into()],
            ..Default::default()
        };
        let text = ron::to_string(&p).unwrap();
        assert_eq!(ron::from_str::<Progress>(&text).unwrap(), p);
    }
}
