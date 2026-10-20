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

    /// One word for it, on the wrist and the career page.
    pub fn short(self) -> &'static str {
        match self {
            Upgrade::StashRack => "rack",
            Upgrade::BenchVice => "vice",
            Upgrade::Aerial => "aerial",
            Upgrade::MedCabinet => "cabinet",
            Upgrade::StashRack2 => "shelf",
        }
    }

    /// What it wants every [`UPKEEP_EVERY`] raids, from the stash.
    pub fn upkeep(self) -> &'static [(&'static str, u32)] {
        match self {
            Upgrade::StashRack | Upgrade::StashRack2 => &[("scrap_metal", 2)],
            Upgrade::BenchVice => &[("gun_oil", 1)],
            Upgrade::Aerial => &[("electronics", 1)],
            Upgrade::MedCabinet => &[("field_dressing", 1)],
        }
    }

    /// What it does, in a few words, for the career page.
    pub fn does(self) -> &'static str {
        match self {
            Upgrade::StashRack => "stash +100 L",
            Upgrade::BenchVice => "seven more recipes",
            Upgrade::Aerial => "Surge warning at 10 min",
            Upgrade::MedCabinet => "a dressing each raid",
            Upgrade::StashRack2 => "stash +100 L",
        }
    }
}

/// A fitting's upkeep, at a homecoming.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upkept {
    /// Its parts were taken.
    Paid,
    /// The stash hadn't them: laid up.
    LaidUp,
    /// Laid up until now; its parts were found.
    Back,
}

/// Raids between a fitting's upkeep.
pub const UPKEEP_EVERY: u32 = 6;

/// What the driver's caches can lean to, past the ladder.
#[derive(Clone, Debug)]
pub struct Order {
    pub id: &'static str,
    /// On the card.
    pub name: &'static str,
    /// The loot table each pile rolls from, and how many times.
    pub table: &'static str,
    pub rolls: u32,
}

pub const ORDERS: [Order; 4] = [
    Order {
        id: "anything",
        name: "ANYTHING",
        table: "driver_cache",
        rolls: 1,
    },
    Order {
        id: "rounds",
        name: "ROUNDS",
        table: "ammo_common",
        rolls: 4,
    },
    Order {
        id: "medicine",
        name: "MEDICINE",
        table: "medical",
        rolls: 4,
    },
    Order {
        id: "gadgets",
        name: "GADGETS",
        table: "gadgets",
        rolls: 3,
    },
];

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
/// Past the ladder, deep jobs: the worst places (and underground), and
/// bigger caches (`docs/PROGRESSION.md`, "After the ladder").
pub const STANDING: Tier = Tier {
    name: "Deep jobs",
    jobs: 0,
    danger: (3, 3),
    far: false,
    kinds: &[JobKind::Fetch, JobKind::Recover, JobKind::Scout],
    fits: &[],
};

/// Past the ladder, after the epilogue: the valleys are quiet, and the
/// jobs ease to danger 2 (underground still counts).
pub const STANDING_QUIET: Tier = Tier {
    name: "Standing jobs",
    jobs: 0,
    danger: (2, 2),
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

/// The tier index from which nights come (after "Runs").
pub const NIGHTS_FROM: u32 = 3;

/// The first deep job after the ladder: the intake ledger from the
/// Custodian's depot under the Marrow Gate (Underground's `depot_marrow`),
/// in the Mareks' bay off the kept stacks, behind two power stages.
pub const LEDGER_ITEM: &str = "intake_ledger";
pub const DEPOT_PREFAB: &str = "depot_marrow";
pub const LEDGER_MAP: &str = "marrow_creek";
/// World (x, y, z): the bay's floor, clear of its crates.
pub const LEDGER_AT: [f32; 3] = [-262.0, 11.31, -179.4];

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
    /// Bring home the intake ledger from the depot ([`LEDGER_ITEM`]): the
    /// first deep job.
    Ledger,
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
            Job::Ledger => Some(LEDGER_MAP),
            Job::Recover { map, .. } | Job::Scout { map, .. } => Some(map),
        }
    }

    /// Where it's pencilled on the field map, if anywhere yet.
    pub fn mark(&self) -> Option<(&str, [f32; 2])> {
        match self {
            Job::Fetch { .. } => None,
            Job::Letter => Some((LETTER_MAP, LETTER_AT)),
            Job::Ledger => Some((LEDGER_MAP, [LEDGER_AT[0], LEDGER_AT[2]])),
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
    /// What the driver's caches lean to, once the ladder's done (an
    /// [`ORDERS`] id; none: anything).
    #[serde(default)]
    pub order: Option<String>,
    /// When each fitting's upkeep is next due: (upgrade id, raid count).
    #[serde(default)]
    pub upkeep_due: Vec<(String, u32)>,
    /// Fittings laid up for want of their upkeep (upgrade ids).
    #[serde(default)]
    pub laid_up: Vec<String>,
    /// After the epilogue the valleys are quiet: deep jobs ease off
    /// ([`STANDING_QUIET`]). Kept in step with the game's epilogue.
    #[serde(default)]
    pub quiet: bool,
}

impl Progress {
    pub fn has(&self, u: Upgrade) -> bool {
        self.upgrades.iter().any(|id| id == u.id())
    }

    /// Fitted and not laid up.
    pub fn works(&self, u: Upgrade) -> bool {
        self.has(u) && !self.laid_up.iter().any(|id| id == u.id())
    }

    /// The ladder is done: deep jobs, and orders.
    pub fn past_ladder(&self) -> bool {
        self.tier as usize >= TIERS.len()
    }

    /// The order on the card, as [`ORDERS`] has it.
    pub fn order_def(&self) -> &'static Order {
        self.order
            .as_deref()
            .and_then(|id| ORDERS.iter().find(|o| o.id == id))
            .unwrap_or(&ORDERS[0])
    }

    /// The next order on the card (pressing it).
    pub fn next_order(&mut self) {
        let i = ORDERS
            .iter()
            .position(|o| o.id == self.order_def().id)
            .unwrap_or(0);
        self.order = Some(ORDERS[(i + 1) % ORDERS.len()].id.to_owned());
    }

    /// Upkeep at a homecoming after raid `raids`: for each fitting due,
    /// `take` is asked for its parts (and takes them if it can). Returns
    /// what happened, for the driver to say.
    pub fn upkeep(
        &mut self,
        raids: u32,
        mut take: impl FnMut(&[(&'static str, u32)]) -> bool,
    ) -> Vec<(Upgrade, Upkept)> {
        let mut said = Vec::new();
        for u in Upgrade::ALL {
            if !self.has(u) {
                continue;
            }
            let Some(i) = self.upkeep_due.iter().position(|(id, _)| id == u.id()) else {
                // Fitted just now (or before there was upkeep): due in a
                // while.
                self.upkeep_due
                    .push((u.id().to_owned(), raids + UPKEEP_EVERY));
                continue;
            };
            if raids < self.upkeep_due[i].1 {
                continue;
            }
            let was_laid_up = !self.works(u);
            if take(u.upkeep()) {
                self.upkeep_due[i].1 = raids + UPKEEP_EVERY;
                self.laid_up.retain(|id| id != u.id());
                said.push((
                    u,
                    if was_laid_up {
                        Upkept::Back
                    } else {
                        Upkept::Paid
                    },
                ));
            } else if !was_laid_up {
                self.laid_up.push(u.id().to_owned());
                said.push((u, Upkept::LaidUp));
            }
        }
        said
    }

    /// The tier being worked on (standing jobs once they're all done).
    pub fn tier_def(&self) -> &'static Tier {
        TIERS.get(self.tier as usize).unwrap_or(if self.quiet {
            &STANDING_QUIET
        } else {
            &STANDING
        })
    }

    /// The tier's jobs are all that's left before the next fitting.
    pub fn next_fitting(&self) -> &'static [Upgrade] {
        self.tier_def().fits
    }

    /// The stash crate's room, litres of bulk.
    pub fn stash_capacity(&self) -> f32 {
        let racks = [Upgrade::StashRack, Upgrade::StashRack2]
            .into_iter()
            .filter(|u| self.works(*u))
            .count();
        STASH_BASE + STASH_RACK * racks as f32
    }

    /// The bench's recipe tier (0: as it came; 1: with the vice).
    pub fn bench_tier(&self) -> u8 {
        self.works(Upgrade::BenchVice) as u8
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

    /// Whether the Surge may bring night raids: once the third tier
    /// ("Runs") is done and the aerial is up, the jobs are the Zone's worst
    /// and night joins the weather's table (`world::weather`).
    pub fn nights(&self) -> bool {
        self.tier >= NIGHTS_FROM
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

/// Underground sites in a valley: (its name, where its entrance is, its
/// danger). Underground is never calm: danger 2 at least, or its region's.
pub fn underground_sites(map: &MapDef) -> Vec<(String, [f32; 2], u8)> {
    map.placements
        .iter()
        .filter_map(|p| {
            let l = crate::underground::layout(&p.prefab)?;
            let danger = map.region_at(p.at).map_or(2, |r| r.danger.max(2));
            Some((l.name.clone(), p.at, danger))
        })
        .collect()
}

/// The underground site a Recover job's `region` names, if it names one.
pub fn underground_site(map: &MapDef, region: &str) -> Option<(String, [f32; 2], u8)> {
    underground_sites(map).into_iter().find(|s| s.0 == region)
}

/// Prefabs with a roof and furniture (loot spots) in them.
pub fn is_building(prefab: &str) -> bool {
    matches!(
        crate::map::prefab_base(prefab),
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
    // The first deep job: the ledger, once there's a depot to fetch it from.
    let ladder: u32 = TIERS.iter().map(|t| t.jobs).sum();
    let depot = valleys
        .iter()
        .any(|v| v.id == LEDGER_MAP && v.map.placements.iter().any(|p| p.prefab == DEPOT_PREFAB));
    if progress.past_ladder() && progress.done == ladder && depot {
        return Job::Ledger;
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
                // Half the time, down into an underground site of the
                // tier's danger, when the valley has one.
                let (lo, hi) = tier.danger;
                let below: Vec<String> = underground_sites(v.map)
                    .into_iter()
                    .filter(|s| (lo..=hi).contains(&s.2) || tier.jobs == 0)
                    .map(|s| s.0)
                    .collect();
                let above = regions(v.map, tier.danger);
                if !below.is_empty() && (rng.chance(0.5) || above.is_empty()) {
                    return Job::Recover {
                        map: v.id.to_owned(),
                        region: below[rng.below(below.len())].clone(),
                        at: None,
                        place: None,
                    };
                }
                let r = above;
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
                    // Deep jobs may go underground instead.
                    let below = tier.jobs == 0 && !underground_sites(map).is_empty();
                    assert!(
                        below || !regions(map, tier.danger).is_empty(),
                        "{id}: {}",
                        tier.name
                    );
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
        assert!(!p.nights(), "no nights early");
        let all: u32 = TIERS.iter().map(|t| t.jobs).sum();
        p.credit(all);
        assert_eq!(p.tier as usize, TIERS.len());
        for u in Upgrade::ALL {
            assert!(p.has(u), "{u:?}");
        }
        assert_eq!(p.stash_capacity(), STASH_BASE + 2.0 * STASH_RACK);
        assert!(p.nights(), "nights at the top");
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
    fn some_recover_jobs_go_underground_in_every_valley() {
        let v = valleys();
        let valleys = as_valleys(&v);
        for (id, map) in &v {
            let sites = underground_sites(map);
            assert!(!sites.is_empty(), "{id} has an underground site");
            assert!(sites.iter().all(|s| s.2 >= 2), "{sites:?}");
            let below = (1..200).any(|done| {
                let p = Progress {
                    tier: 2,
                    done,
                    ..Default::default()
                };
                matches!(roll_job(&p, 5, id, &valleys),
                    Job::Recover { region, .. } if underground_site(map, &region).is_some())
            });
            assert!(below, "{id}: no recover job ever went underground");
        }
    }

    #[test]
    fn fittings_want_upkeep_and_lie_up_without_it() {
        let mut p = Progress::default();
        p.credit(TIERS.iter().map(|t| t.jobs).sum());
        assert!(p.past_ladder());
        // The first homecoming sets every fitting's clock.
        assert!(p.upkeep(30, |_| panic!("nothing due yet")).is_empty());
        assert!(p.upkeep(35, |_| panic!("not yet")).is_empty());
        // Due at 36: the stash has scrap but no electronics.
        let said = p.upkeep(36, |parts| parts.iter().all(|(id, _)| *id == "scrap_metal"));
        assert!(
            said.contains(&(Upgrade::StashRack, Upkept::Paid)),
            "{said:?}"
        );
        assert!(
            said.contains(&(Upgrade::Aerial, Upkept::LaidUp)),
            "{said:?}"
        );
        assert!(!p.works(Upgrade::Aerial));
        assert!(p.works(Upgrade::StashRack));
        assert!(!p.works(Upgrade::BenchVice), "no oil either");
        assert_eq!(p.bench_tier(), 0, "a laid-up vice teaches nothing");
        // Laid up isn't said twice; found parts bring it back.
        let said = p.upkeep(37, |parts| parts.iter().all(|(id, _)| *id != "electronics"));
        assert!(!said.iter().any(|s| s.0 == Upgrade::Aerial), "{said:?}");
        let said = p.upkeep(38, |_| true);
        assert!(said.contains(&(Upgrade::Aerial, Upkept::Back)), "{said:?}");
        assert!(p.works(Upgrade::Aerial));
        assert_eq!(p.bench_tier(), 1);
        // Past the ladder, orders cycle through all and back.
        assert_eq!(p.order_def().id, "anything");
        for _ in 0..ORDERS.len() {
            p.next_order();
        }
        assert_eq!(p.order_def().id, "anything");
    }

    #[test]
    fn deep_jobs_go_to_the_worst_places() {
        let v = valleys();
        let valleys = as_valleys(&v);
        for (id, map) in &v {
            for done in 20..60 {
                let p = Progress {
                    tier: TIERS.len() as u32,
                    done,
                    ..Default::default()
                };
                match roll_job(&p, 9, id, &valleys) {
                    Job::Scout { at, .. } => {
                        assert_eq!(map.region_at(at).unwrap().danger, 3, "{id}")
                    }
                    Job::Recover { region, .. } => {
                        let deep = underground_site(map, &region).is_some()
                            || map
                                .regions
                                .iter()
                                .any(|r| r.name == region && r.danger == 3);
                        assert!(deep, "{id}: {region}");
                    }
                    Job::Fetch { .. } | Job::Letter | Job::Ledger => {}
                }
            }
        }
    }

    #[test]
    fn the_first_deep_job_is_the_ledger_once_there_is_a_depot() {
        let mut v = valleys();
        let p = Progress {
            tier: TIERS.len() as u32,
            done: TIERS.iter().map(|t| t.jobs).sum(),
            ..Default::default()
        };
        // Marrow Creek has the depot; without it, an ordinary deep job.
        assert!(
            v[0].1.placements.iter().any(|p| p.prefab == DEPOT_PREFAB),
            "the depot is on Marrow Creek's map"
        );
        assert_eq!(roll_job(&p, 3, "kettle_pass", &as_valleys(&v)), Job::Ledger);
        v[0].1.placements.retain(|p| p.prefab != DEPOT_PREFAB);
        assert_ne!(roll_job(&p, 3, "kettle_pass", &as_valleys(&v)), Job::Ledger);
        let v = valleys();
        assert_eq!(roll_job(&p, 3, "kettle_pass", &as_valleys(&v)), Job::Ledger);
        // Only the first.
        let later = Progress {
            done: p.done + 1,
            ..p.clone()
        };
        assert_ne!(
            roll_job(&later, 3, "kettle_pass", &as_valleys(&v)),
            Job::Ledger
        );
    }

    #[test]
    fn after_the_epilogue_the_jobs_ease_off() {
        let v = valleys();
        let valleys = as_valleys(&v);
        for (id, map) in &v {
            for done in 20..50 {
                let p = Progress {
                    tier: TIERS.len() as u32,
                    done,
                    quiet: true,
                    ..Default::default()
                };
                if let Job::Scout { at, .. } = roll_job(&p, 4, id, &valleys) {
                    assert_eq!(map.region_at(at).unwrap().danger, 2, "{id}");
                }
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
