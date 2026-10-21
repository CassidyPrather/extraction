//! The driver's contracts: the job board as a career arc
//! (`docs/PROGRESSION.md`, "Contracts").
//!
//! A contract is a short chain of steps, each one a job on the card in
//! turn ([`step_job`]): fetch from the stash, go and see a place, bring a
//! named thing home, leave something somewhere, photograph a machine. It
//! starts at the head of a tier (its steps are that tier's jobs, so the
//! ladder keeps its length) or past the ladder, once the career has the
//! trust it asks. Done, it pays kit through the cab's hatch and trust, and
//! may open a valley or teach the bench a recipe.

use serde::{Deserialize, Serialize};

use crate::map::{MapDef, MarkerKind};
use crate::progression::{Job, Progress, TIERS, Valley};

/// One step of a contract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Hand over `n` of `item` from the stash.
    Fetch { item: &'static str, n: u32 },
    /// Go and look at the point of interest `place` in `map`, then get home.
    Scout {
        map: &'static str,
        place: &'static str,
    },
    /// Bring home `item`, left by the point of interest `place` in `map`
    /// (or at `near`, when it's somewhere the place's name only points to).
    Bring {
        map: &'static str,
        place: &'static str,
        item: &'static str,
        near: Option<[f32; 2]>,
    },
    /// Leave `item` (the contract's kit) by the point of interest `place`
    /// in `map`, then get home.
    Plant {
        map: &'static str,
        place: &'static str,
        item: &'static str,
    },
    /// Photograph a machine (`subject`) with the contract's camera, then
    /// get home with the picture.
    Photograph { subject: Subject },
}

/// What a photograph is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Subject {
    Walker,
    Heavy,
}

impl Subject {
    pub fn name(self) -> &'static str {
        match self {
            Subject::Walker => "a walker",
            Subject::Heavy => "a heavy",
        }
    }
}

/// What a contract opens up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unlock {
    /// A valley the route card keeps back (`progression::LATE_VALLEYS`).
    Valley(&'static str),
    /// A bench recipe (`fabrication::CONTRACT_RECIPES`).
    Recipe(&'static str),
}

/// When the driver offers it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Offered {
    /// In this ladder tier (an index into [`TIERS`]), as its job `at` (0
    /// is the tier's first).
    Tier { tier: u32, at: u32 },
    /// Past the ladder, with this much trust.
    Deep { trust: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contract {
    pub id: &'static str,
    pub name: &'static str,
    /// Who it's for, in the driver's words.
    pub giver: &'static str,
    /// What he says about each step, on the card with it ("" for nothing
    /// more than the step's own ask).
    pub says: &'static [&'static str],
    pub offered: Offered,
    /// Raids needed in every one of the first valleys before it's offered.
    pub raided_all: bool,
    /// Contracts done before it's offered.
    pub after: &'static [&'static str],
    pub steps: &'static [Step],
    /// Handed over with the first step (a relay to leave, a camera).
    pub kit: &'static [(&'static str, u32)],
    /// Handed over when it's done.
    pub pay: &'static [(&'static str, u32)],
    pub trust: u32,
    pub unlocks: &'static [Unlock],
    /// What he says when it's done ("" for the usual thanks).
    pub done: &'static str,
}

/// Within this of a planted thing's place (m).
pub const PLANT_RADIUS: f32 = 4.0;
/// A photograph's subject, no farther than this (m) and this close to the
/// middle of the frame (radians off the camera's line).
pub const PHOTO_RANGE: f32 = 60.0;
pub const PHOTO_CONE: f32 = 0.2;

pub const CONTRACTS: &[Contract] = &[
    Contract {
        id: "post_counter",
        name: "The post counter",
        giver: "the Holm boy",
        says: &["The Holm boy's grown. Wants his grandad's watch. The post office."],
        offered: Offered::Tier { tier: 0, at: 1 },
        raided_all: false,
        after: &[],
        steps: &[Step::Bring {
            map: "marrow_creek",
            place: "Village",
            item: "holm_watch",
            // The post office's counter, in the village.
            near: Some([-20.0, 65.0]),
        }],
        kit: &[],
        pay: &[("field_dressing", 1), ("matches", 2)],
        trust: 1,
        unlocks: &[],
        done: "",
    },
    Contract {
        id: "mast_relay",
        name: "The mast relay",
        giver: "Lev",
        says: &[
            "I want to hear them talk. Put this up the radio mast for me.",
            "",
        ],
        offered: Offered::Tier { tier: 1, at: 0 },
        raided_all: false,
        after: &[],
        steps: &[
            Step::Plant {
                map: "marrow_creek",
                place: "Radio mast",
                item: "relay_box",
            },
            Step::Fetch {
                item: "radio",
                n: 1,
            },
        ],
        kit: &[("relay_box", 1)],
        pay: &[("power_cell", 2), ("ammo_545", 2)],
        trust: 1,
        unlocks: &[Unlock::Recipe("pack_545_ap")],
        done: "",
    },
    Contract {
        id: "annas_tape",
        name: "Anna's tape",
        giver: "Anna",
        says: &["Anna asked about a tape. Her playing, years ago. Mum's house."],
        offered: Offered::Tier { tier: 1, at: 2 },
        raided_all: false,
        after: &[],
        steps: &[Step::Bring {
            map: "marrow_creek",
            place: "Village",
            item: "tape_practice",
            // Hana's house by the well (Radio lays the tape there).
            near: Some([-17.3, 93.7]),
        }],
        kit: &[],
        pay: &[("canned_stew", 2), ("tea_tin", 1)],
        trust: 1,
        unlocks: &[],
        done: "Driver: \"She'll cry. Good crying.\"",
    },
    Contract {
        id: "hajek_tin",
        name: "Hajek's tin",
        giver: "Vera Hajkova",
        says: &[
            "His sister wants what he kept. Mirek, at the charcoal hut. He stayed.",
            "If he walked down, he'd have passed the weir. Look in there.",
        ],
        offered: Offered::Tier { tier: 2, at: 0 },
        raided_all: false,
        after: &[],
        steps: &[
            Step::Bring {
                map: "kettle_pass",
                place: "Charcoal hut",
                item: "hajek_tin",
                near: None,
            },
            Step::Scout {
                map: "kettle_pass",
                place: "Weir cottage",
            },
        ],
        kit: &[],
        pay: &[("regen_injector", 1), ("field_dressing", 2)],
        trust: 2,
        unlocks: &[Unlock::Recipe("pack_762x39_ap")],
        done: "",
    },
    Contract {
        id: "walker_portrait",
        name: "A walker's portrait",
        giver: "the town paper",
        says: &[
            "A man from the town paper. Nobody believes it out there. He wants one, side on. Take the camera.",
            "",
        ],
        offered: Offered::Tier { tier: 3, at: 0 },
        raided_all: false,
        after: &[],
        steps: &[
            Step::Photograph {
                subject: Subject::Walker,
            },
            Step::Scout {
                map: "cinder_works",
                place: "The chimney",
            },
        ],
        kit: &[("camera", 1)],
        pay: &[("pu_scope", 1), ("ammo_762r", 2)],
        trust: 2,
        unlocks: &[Unlock::Recipe("pack_762r_ap")],
        done: "",
    },
    Contract {
        id: "the_overflow",
        name: "The overflow",
        giver: "Lev",
        says: &[
            "The depot's intake goes somewhere past the gate. Put a relay on it.",
            "",
        ],
        offered: Offered::Deep { trust: 3 },
        raided_all: true,
        after: &[],
        steps: &[
            Step::Plant {
                map: "marrow_creek",
                place: "The Depot",
                item: "relay_box",
            },
            Step::Fetch {
                item: "electronics",
                n: 2,
            },
        ],
        kit: &[("relay_box", 1)],
        pay: &[("fuel_can", 1), ("radio", 1)],
        trust: 2,
        unlocks: &[Unlock::Valley("sallow_reservoir")],
        done: "",
    },
    Contract {
        id: "scour_key",
        name: "The scour key",
        giver: "Lev",
        says: &[
            "The scours opened by themselves. Somebody left the key at the valve house. Bring it, so nobody opens them again.",
            "Now the draw-off tower. Put a relay up there; that's where it all talks from.",
        ],
        offered: Offered::Deep { trust: 4 },
        raided_all: false,
        after: &["the_overflow"],
        steps: &[
            Step::Bring {
                map: "sallow_reservoir",
                place: "Valve house",
                item: "scour_key",
                near: None,
            },
            Step::Plant {
                map: "sallow_reservoir",
                place: "Draw-off tower",
                item: "relay_box",
            },
        ],
        kit: &[("relay_box", 1)],
        pay: &[("emp_trap", 1), ("ammo_545_ap", 2)],
        trust: 2,
        unlocks: &[],
        done: "",
    },
    Contract {
        id: "the_heavy",
        name: "The heavy",
        giver: "the town paper",
        says: &["Now he wants the big one. Get close. Get home.", ""],
        offered: Offered::Deep { trust: 5 },
        raided_all: false,
        after: &[],
        steps: &[
            Step::Photograph {
                subject: Subject::Heavy,
            },
            Step::Plant {
                map: "cinder_works",
                place: "Fire lookout",
                item: "relay_box",
            },
        ],
        kit: &[("camera", 1), ("relay_box", 1)],
        pay: &[("emp_grenade", 2), ("steel_plate", 1)],
        trust: 3,
        unlocks: &[],
        done: "",
    },
];

pub fn contract(id: &str) -> Option<&'static Contract> {
    CONTRACTS.iter().find(|c| c.id == id)
}

/// The contract under way, and how far along it is.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Active {
    pub id: String,
    pub step: u32,
}

/// The contract the driver would start now, if any: none under way, and
/// this one not yet done, its tier's head (with room for its steps before
/// the tier's last job) or the trust it asks past the ladder.
pub fn next_contract(p: &Progress) -> Option<&'static Contract> {
    if p.contract.is_some() {
        return None;
    }
    CONTRACTS.iter().find(|c| {
        if p.contracts_done.iter().any(|d| d == c.id) {
            return false;
        }
        if c.raided_all
            && !crate::progression::FIRST_VALLEYS
                .iter()
                .all(|v| p.raids_in(v) > 0)
        {
            return false;
        }
        if !c
            .after
            .iter()
            .all(|a| p.contracts_done.iter().any(|d| d == a))
        {
            return false;
        }
        match c.offered {
            Offered::Tier { tier: t, at } => {
                let Some(tier) = TIERS.get(t as usize) else {
                    return false;
                };
                // The last tier's last job is the letter's.
                let last = (t as usize == TIERS.len() - 1) as u32;
                p.tier == t && p.done_in_tier == at && at + c.steps.len() as u32 + last <= tier.jobs
            }
            Offered::Deep { trust } => p.past_ladder() && p.trust >= trust,
        }
    })
}

/// The point of interest `place` in `map`.
pub fn place_at(map: &MapDef, place: &str) -> Option<[f32; 2]> {
    map.markers_of(&MarkerKind::Poi)
        .find(|m| m.name == place)
        .map(|m| m.at)
}

/// The contract step as a job on the card.
pub fn step_job(step: &Step, valleys: &[Valley]) -> Option<Job> {
    let at = |map: &str, place: &str| {
        valleys
            .iter()
            .find(|v| v.id == map)
            .and_then(|v| place_at(v.map, place))
    };
    Some(match *step {
        Step::Fetch { item, n } => Job::Fetch {
            item: item.to_owned(),
            n,
        },
        Step::Scout { map, place } => Job::Scout {
            map: map.to_owned(),
            place: place.to_owned(),
            at: at(map, place)?,
            seen: false,
        },
        Step::Bring {
            map,
            place,
            item,
            near,
        } => Job::Bring {
            map: map.to_owned(),
            place: place.to_owned(),
            item: item.to_owned(),
            at: match near {
                Some(n) => n,
                None => at(map, place)?,
            },
        },
        Step::Plant { map, place, item } => Job::Plant {
            map: map.to_owned(),
            place: place.to_owned(),
            item: item.to_owned(),
            at: at(map, place)?,
            planted: false,
        },
        Step::Photograph { subject } => Job::Photograph {
            subject,
            taken: false,
        },
    })
}

/// The job the contract under way puts on the card now.
pub fn contract_job(p: &Progress, valleys: &[Valley]) -> Option<Job> {
    let a = p.contract.as_ref()?;
    let c = contract(&a.id)?;
    step_job(c.steps.get(a.step as usize)?, valleys)
}

/// The bench recipes the career's contracts have taught it.
pub fn recipes(p: &Progress) -> Vec<&'static str> {
    p.contracts_done
        .iter()
        .filter_map(|id| contract(id))
        .flat_map(|c| c.unlocks.iter())
        .filter_map(|u| match u {
            Unlock::Recipe(r) => Some(*r),
            Unlock::Valley(_) => None,
        })
        .collect()
}

/// A valley a contract has opened.
pub fn opened(p: &Progress, map: &str) -> bool {
    p.contracts_done
        .iter()
        .filter_map(|id| contract(id))
        .flat_map(|c| c.unlocks.iter())
        .any(|u| matches!(u, Unlock::Valley(v) if *v == map))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every valley, the late ones too (a contract may be in one).
    fn valleys() -> Vec<(&'static str, MapDef)> {
        crate::map::VALLEYS
            .into_iter()
            .map(|(id, text)| (id, MapDef::from_ron(text).unwrap()))
            .collect()
    }

    #[test]
    fn every_step_of_every_contract_has_its_place() {
        let v = valleys();
        let vs: Vec<Valley> = v.iter().map(|(id, map)| Valley { id, map }).collect();
        for c in CONTRACTS {
            for s in c.steps {
                assert!(step_job(s, &vs).is_some(), "{}: no place for {s:?}", c.id);
            }
            assert_eq!(c.says.len(), c.steps.len(), "{}: a line a step", c.id);
            if let Offered::Tier { tier: t, at } = c.offered {
                let last = (t as usize == TIERS.len() - 1) as u32;
                assert!(
                    at + c.steps.len() as u32 + last <= TIERS[t as usize].jobs,
                    "{}: too long for its tier",
                    c.id
                );
            }
        }
    }

    #[test]
    fn a_career_walks_the_arc_and_the_letter_still_comes() {
        let v = valleys();
        let vs: Vec<Valley> = v.iter().map(|(id, map)| Valley { id, map }).collect();
        let mut p = Progress::default();
        let mut letter = None;
        let mut started = Vec::new();
        for n in 0..60 {
            if p.contract.is_none() && p.start_contract().is_some() {
                started.push((p.contract.clone().unwrap().id, p.tier));
            }
            let job = contract_job(&p, &vs)
                .unwrap_or_else(|| crate::progression::roll_job(&p, 7, "marrow_creek", &vs));
            if job == Job::Letter {
                letter = Some(n);
            }
            for valley in crate::progression::FIRST_VALLEYS {
                p.count_raid(valley);
            }
            p.complete_job();
        }
        assert!(letter.is_some(), "Hana's letter: {started:?}");
        assert_eq!(p.tier as usize, TIERS.len() + 0, "the ladder's done");
        let ids: Vec<&str> = started.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "post_counter",
                "mast_relay",
                "annas_tape",
                "hajek_tin",
                "walker_portrait",
                "the_overflow",
                "scour_key",
                "the_heavy"
            ]
        );
        assert_eq!(p.contracts_done.len(), CONTRACTS.len());
        assert_eq!(p.trust, CONTRACTS.iter().map(|c| c.trust).sum::<u32>());
        assert!(p.opens("sallow_reservoir"));
    }

    #[test]
    fn every_contract_names_real_things() {
        let cat = crate::items::ItemCatalog::from_ron(crate::items::BUILTIN_CATALOG).unwrap();
        for c in CONTRACTS {
            for (id, _) in c.kit.iter().chain(c.pay) {
                assert!(cat.kind(id).is_some(), "{}: {id}", c.id);
            }
            for s in c.steps {
                if let Step::Fetch { item, .. }
                | Step::Bring { item, .. }
                | Step::Plant { item, .. } = s
                {
                    assert!(cat.kind(item).is_some(), "{}: {item}", c.id);
                }
                if let Step::Plant { item, .. } = s {
                    assert!(
                        c.kit.iter().any(|(k, _)| k == item),
                        "{}: {item} isn't handed over",
                        c.id
                    );
                }
                if let Step::Photograph { .. } = s {
                    assert!(
                        c.kit.iter().any(|(k, _)| *k == "camera"),
                        "{}: no camera",
                        c.id
                    );
                }
            }
            for u in c.unlocks {
                if let Unlock::Recipe(r) = u {
                    assert!(crate::fabrication::recipe(r).is_some(), "{}: {r}", c.id);
                }
            }
        }
    }

    #[test]
    fn the_arc_offers_each_contract_once_in_its_place() {
        let mut p = Progress::default();
        assert!(next_contract(&p).is_none(), "not the very first job");
        p.done = 1;
        p.done_in_tier = 1;
        assert_eq!(next_contract(&p).map(|c| c.id), Some("post_counter"));
        p.contracts_done.push("post_counter".into());
        p.done = 3;
        p.done_in_tier = 0;
        p.tier = 1;
        assert_eq!(next_contract(&p).map(|c| c.id), Some("mast_relay"));
        p.done_in_tier = 1;
        assert!(next_contract(&p).is_none(), "only at the tier's head");
        p.done_in_tier = 0;
        p.contracts_done.push("mast_relay".into());
        assert!(next_contract(&p).is_none(), "once");
        // Past the ladder: the overflow wants trust and all three valleys.
        p.tier = TIERS.len() as u32;
        p.trust = 3;
        assert!(next_contract(&p).is_none());
        for v in crate::progression::FIRST_VALLEYS {
            p.count_raid(v);
        }
        assert_eq!(next_contract(&p).map(|c| c.id), Some("the_overflow"));
        p.contracts_done.push("the_overflow".into());
        assert!(opened(&p, "sallow_reservoir"));
        assert!(p.opens("sallow_reservoir"));
        assert!(
            next_contract(&p).is_none(),
            "the scour key wants more trust"
        );
        p.trust = 4;
        assert_eq!(next_contract(&p).map(|c| c.id), Some("scour_key"));
        p.contracts_done.push("scour_key".into());
        assert!(next_contract(&p).is_none(), "the heavy wants more trust");
        p.trust = 5;
        assert_eq!(next_contract(&p).map(|c| c.id), Some("the_heavy"));
        assert_eq!(recipes(&p), vec!["pack_545_ap"]);
    }
}
