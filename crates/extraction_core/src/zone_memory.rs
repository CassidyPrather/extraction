//! What a valley remembers of you across raids: where you walk, which
//! rendezvous you call, where you killed its machines, which buildings you
//! cleared. Compact (a coarse grid and a few short lists), saved with the
//! valley (`save::ZoneSave::memory`), faded each raid ([`DECAY`]) so that
//! changing your habits pays off, and capped, so it only ever leans.
//!
//! A raid is traced as it happens ([`RaidTrace`]) and folded in when it
//! ends ([`ZoneMemory::fold`]). What the director makes of it (patrols
//! toward your corridors, a sentry's eye on your rendezvous, a cleared
//! building found shut, a watcher where you killed) is the game's
//! (`robots::memory`); what's here only answers "how much".

use serde::{Deserialize, Serialize};

/// The one knob: how much the Zone's memory of you counts, 0 (not at all)
/// to 1. Everything it does is scaled by this.
pub const MEMORY: f32 = 1.0;
/// The grid your routes are kept on (m a side).
pub const CELL: f32 = 30.0;
/// Each raid, what it remembers fades to this share...
pub const DECAY: f32 = 0.6;
/// ...and is forgotten under this.
pub const FORGET: f32 = 0.1;
/// A weight tops out here (about three raids running): as habitual as
/// it gets.
pub const HEAT_MAX: f32 = 2.0;
/// At most this many cells, kills, buildings and rendezvous are kept (the
/// weakest go).
pub const MAX_CELLS: usize = 400;
pub const MAX_SPOTS: usize = 12;
/// At most this many places searched are kept (the weakest go).
pub const MAX_SEARCHED: usize = 40;
/// A kill within this of a remembered one (of the same kind), or a loot
/// spot within this of a remembered building's, is the same place (m).
pub const SAME_PLACE: f32 = 15.0;
/// A rendezvous used this much (a raid's worth) is a favourite.
pub const HABIT: f32 = 1.0;

/// A place remembered, and how strongly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    pub at: [f32; 2],
    /// What happened there (a kill: the machine's kind; else empty).
    #[serde(default)]
    pub kind: String,
    pub weight: f32,
}

/// A valley's memory of you.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZoneMemory {
    /// Raids folded in.
    pub raids: u32,
    /// Cells you walked through, and how habitually.
    pub heat: Vec<([i32; 2], f32)>,
    pub rvs: Vec<Spot>,
    pub kills: Vec<Spot>,
    pub looted: Vec<Spot>,
    /// Where its walkers searched for you and found nothing: the paint
    /// they leave is still there next raid, fading.
    #[serde(default)]
    pub searched: Vec<Spot>,
}

/// One raid, as it happened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaidTrace {
    /// Cells walked through (each once, however long you were there).
    pub cells: Vec<[i32; 2]>,
    /// The rendezvous you got out from.
    pub rv: Option<[f32; 2]>,
    /// Machines you wrecked: where, what.
    pub kills: Vec<([f32; 2], String)>,
    /// Loot spots you cleared.
    pub looted: Vec<[f32; 2]>,
    /// Where walkers searched for you.
    pub searched: Vec<[f32; 2]>,
}

/// The grid cell `(x, z)` lies in.
pub fn cell(x: f32, z: f32) -> [i32; 2] {
    [(x / CELL).floor() as i32, (z / CELL).floor() as i32]
}

/// The middle of a cell.
pub fn centre(c: [i32; 2]) -> [f32; 2] {
    [(c[0] as f32 + 0.5) * CELL, (c[1] as f32 + 0.5) * CELL]
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl RaidTrace {
    /// You were at `(x, z)`.
    pub fn walked(&mut self, x: f32, z: f32) {
        let c = cell(x, z);
        if !self.cells.contains(&c) {
            self.cells.push(c);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
            && self.rv.is_none()
            && self.kills.is_empty()
            && self.looted.is_empty()
            && self.searched.is_empty()
    }
}

/// Add `weight` to the spot near `at` (of `kind`), or a new one; keep the
/// strongest `MAX_SPOTS`.
fn remember(spots: &mut Vec<Spot>, at: [f32; 2], kind: &str, weight: f32, cap: f32) {
    match spots
        .iter_mut()
        .find(|s| s.kind == kind && dist(s.at, at) < SAME_PLACE)
    {
        Some(s) => s.weight = (s.weight + weight).min(cap),
        None => spots.push(Spot {
            at,
            kind: kind.to_string(),
            weight: weight.min(cap),
        }),
    }
}

fn keep_strongest<T>(v: &mut Vec<T>, cap: usize, w: impl Fn(&T) -> f32) {
    if v.len() > cap {
        v.sort_by(|a, b| w(b).total_cmp(&w(a)));
        v.truncate(cap);
    }
}

fn fade<T>(v: &mut Vec<T>, w: impl Fn(&mut T) -> &mut f32) {
    for x in v.iter_mut() {
        *w(x) *= DECAY;
    }
    v.retain_mut(|x| *w(x) >= FORGET);
}

impl ZoneMemory {
    /// A raid's end: what it remembered fades, and this raid goes in.
    pub fn fold(&mut self, trace: &RaidTrace) {
        self.raids = self.raids.saturating_add(1);
        fade(&mut self.heat, |h| &mut h.1);
        fade(&mut self.rvs, |s| &mut s.weight);
        fade(&mut self.kills, |s| &mut s.weight);
        fade(&mut self.looted, |s| &mut s.weight);
        fade(&mut self.searched, |s| &mut s.weight);
        for c in &trace.cells {
            match self.heat.iter_mut().find(|h| h.0 == *c) {
                Some(h) => h.1 = (h.1 + 1.0).min(HEAT_MAX),
                None => self.heat.push((*c, 1.0)),
            }
        }
        if let Some(rv) = trace.rv {
            remember(&mut self.rvs, rv, "", 1.0, HEAT_MAX);
        }
        for (at, kind) in &trace.kills {
            remember(&mut self.kills, *at, kind, 1.0, HEAT_MAX);
        }
        for at in &trace.looted {
            remember(&mut self.looted, *at, "", 1.0, HEAT_MAX);
        }
        for at in &trace.searched {
            remember(&mut self.searched, *at, "", 1.0, 1.0);
        }
        keep_strongest(&mut self.heat, MAX_CELLS, |h| h.1);
        keep_strongest(&mut self.rvs, MAX_SPOTS, |s| s.weight);
        keep_strongest(&mut self.kills, MAX_SPOTS, |s| s.weight);
        keep_strongest(&mut self.looted, MAX_SPOTS, |s| s.weight);
        keep_strongest(&mut self.searched, MAX_SEARCHED, |s| s.weight);
    }

    /// How habitually you pass `(x, z)`: 0 (never) to 1, times [`MEMORY`].
    pub fn corridor(&self, x: f32, z: f32) -> f32 {
        let c = cell(x, z);
        self.heat
            .iter()
            .find(|h| h.0 == c)
            .map_or(0.0, |h| (h.1 / HEAT_MAX).min(1.0))
            * MEMORY
    }

    /// The most habitual of the cells round `(x, z)` (it and its eight
    /// neighbours): how near your usual ways a place is.
    pub fn corridor_near(&self, x: f32, z: f32) -> f32 {
        let mut best = 0.0f32;
        for dx in -1..=1 {
            for dz in -1..=1 {
                best = best.max(self.corridor(x + dx as f32 * CELL, z + dz as f32 * CELL));
            }
        }
        best
    }

    /// How habitual the most habitual cell within `r` of `at` is (0..1,
    /// times [`MEMORY`]): read-only, for anything that wants to keep to,
    /// or keep off, your usual ways.
    pub fn heat_near(&self, at: [f32; 2], r: f32) -> f32 {
        self.heat
            .iter()
            .filter(|(c, _)| dist(centre(*c), at) <= r + CELL * 0.71)
            .map(|(_, w)| (w / HEAT_MAX).min(1.0))
            .fold(0.0, f32::max)
            * MEMORY
    }

    /// The rendezvous you favour, if you favour one: where, and how
    /// strongly (0..1, times [`MEMORY`]).
    pub fn favourite_rv(&self) -> Option<([f32; 2], f32)> {
        strongest(&self.rvs, HABIT)
    }

    /// Where you last killed a machine (the strongest such place): where,
    /// what kind, how strongly.
    pub fn kill_spot(&self) -> Option<([f32; 2], String, f32)> {
        let s = self
            .kills
            .iter()
            .filter(|s| s.weight >= HABIT * DECAY)
            .max_by(|a, b| a.weight.total_cmp(&b.weight))?;
        Some((
            s.at,
            s.kind.clone(),
            (s.weight / HEAT_MAX).min(1.0) * MEMORY,
        ))
    }

    /// Loot spots you cleared lately enough to remember, strongest first.
    pub fn looted(&self) -> Vec<([f32; 2], f32)> {
        let mut v: Vec<([f32; 2], f32)> = self
            .looted
            .iter()
            .filter(|s| s.weight >= HABIT * DECAY)
            .map(|s| (s.at, (s.weight / HEAT_MAX).min(1.0) * MEMORY))
            .collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v
    }

    /// Nothing to remember (a fresh career, or long forgotten).
    pub fn is_blank(&self) -> bool {
        self.heat.is_empty()
            && self.rvs.is_empty()
            && self.kills.is_empty()
            && self.looted.is_empty()
            && self.searched.is_empty()
    }

    /// Places its walkers searched for you, and how fresh the paint is
    /// (0..1, times [`MEMORY`]): last raid's strongest.
    pub fn searched(&self) -> Vec<([f32; 2], f32)> {
        self.searched
            .iter()
            .map(|s| (s.at, s.weight.min(1.0) * MEMORY))
            .collect()
    }

    /// Mends one read from a file: drops what's not a number or a world
    /// away, clamps weights.
    pub fn sanitize(&mut self) {
        let ok = |p: [f32; 2]| p.iter().all(|v| v.is_finite() && v.abs() < 100_000.0);
        let w = |x: f32| {
            if x.is_finite() {
                x.clamp(0.0, HEAT_MAX)
            } else {
                0.0
            }
        };
        self.heat.retain(|h| h.0.iter().all(|c| c.abs() < 10_000));
        for h in &mut self.heat {
            h.1 = w(h.1);
        }
        for list in [&mut self.rvs, &mut self.kills, &mut self.looted] {
            list.retain(|s| ok(s.at));
            for s in list.iter_mut() {
                s.weight = w(s.weight);
                s.kind.truncate(32);
            }
            keep_strongest(list, MAX_SPOTS, |s| s.weight);
        }
        self.searched.retain(|s| ok(s.at));
        for s in &mut self.searched {
            s.weight = w(s.weight).min(1.0);
            s.kind.clear();
        }
        keep_strongest(&mut self.searched, MAX_SEARCHED, |s| s.weight);
        keep_strongest(&mut self.heat, MAX_CELLS, |h| h.1);
    }
}

fn strongest(spots: &[Spot], at_least: f32) -> Option<([f32; 2], f32)> {
    let s = spots
        .iter()
        .filter(|s| s.weight >= at_least)
        .max_by(|a, b| a.weight.total_cmp(&b.weight))?;
    Some((s.at, (s.weight / HEAT_MAX).min(1.0) * MEMORY))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raid(cells: &[(f32, f32)], rv: Option<[f32; 2]>) -> RaidTrace {
        let mut t = RaidTrace::default();
        for (x, z) in cells {
            t.walked(*x, *z);
        }
        t.rv = rv;
        t
    }

    /// A fresh valley remembers nothing and leans on nothing.
    #[test]
    fn a_fresh_valley_remembers_nothing() {
        let m = ZoneMemory::default();
        assert!(m.is_blank());
        assert_eq!(m.corridor(100.0, 100.0), 0.0);
        assert!(m.favourite_rv().is_none());
        assert!(m.kill_spot().is_none());
        assert!(m.looted().is_empty());
        // A raid that went nowhere leaves it blank.
        let mut m = m;
        m.fold(&RaidTrace::default());
        assert!(m.is_blank());
    }

    /// The same way three raids running is a habit; then a change of
    /// route fades it out over a few raids.
    #[test]
    fn habits_build_and_fade() {
        let mut m = ZoneMemory::default();
        let mill = [(100.0, 100.0), (130.0, 100.0)];
        for _ in 0..3 {
            m.fold(&raid(&mill, Some([500.0, 20.0])));
        }
        let habit = m.corridor(100.0, 100.0);
        assert!(habit > 0.8, "a habit: {habit}");
        assert!(m.heat_near([110.0, 110.0], 10.0) > 0.8);
        assert_eq!(m.heat_near([600.0, 600.0], 50.0), 0.0);
        assert!(
            m.favourite_rv()
                .is_some_and(|(at, s)| at == [500.0, 20.0] && s > 0.5)
        );
        // Elsewhere from now on, out at another rendezvous.
        let mut faded = vec![];
        for _ in 0..6 {
            m.fold(&raid(&[(900.0, 900.0)], Some([20.0, 500.0])));
            faded.push(m.corridor(100.0, 100.0));
        }
        assert!(faded.windows(2).all(|w| w[1] <= w[0]), "fades: {faded:?}");
        assert_eq!(*faded.last().unwrap(), 0.0, "and is forgotten: {faded:?}");
        assert!(
            m.favourite_rv().is_some_and(|(at, _)| at == [20.0, 500.0]),
            "the new favourite"
        );
    }

    /// Kills and cleared buildings are places: the same place again adds
    /// up, another is another; and only so many are kept.
    #[test]
    fn places_add_up_and_are_capped() {
        let mut m = ZoneMemory::default();
        let mut t = RaidTrace::default();
        t.kills.push(([10.0, 10.0], "Walker".into()));
        t.kills.push(([14.0, 12.0], "Walker".into()));
        t.kills.push(([14.0, 12.0], "Hound".into()));
        t.looted.push([300.0, 300.0]);
        m.fold(&t);
        assert_eq!(m.kills.len(), 2, "two walkers there are one place");
        let (at, kind, _) = m.kill_spot().unwrap();
        assert_eq!(kind, "Walker");
        assert!(dist(at, [10.0, 10.0]) < 1.0);
        assert_eq!(m.looted().len(), 1);
        let mut t = RaidTrace::default();
        for i in 0..40 {
            t.kills.push(([i as f32 * 100.0, 0.0], "Walker".into()));
            t.walked(i as f32 * 1000.0, 0.0);
        }
        m.fold(&t);
        assert!(m.kills.len() <= MAX_SPOTS);
        // A grid of a thousand cells keeps the strongest.
        let mut t = RaidTrace::default();
        for i in 0..1000 {
            t.walked(i as f32 * CELL, 5000.0);
        }
        m.fold(&t);
        assert!(m.heat.len() <= MAX_CELLS);
    }

    /// A mangled memory is mended, not trusted.
    /// A place searched is remembered a raid on, faded, and again searched
    /// it's as fresh as it gets (no more); left alone it's forgotten.
    #[test]
    fn searched_places_fade_raid_by_raid() {
        let mut m = ZoneMemory::default();
        let t = RaidTrace {
            searched: vec![[10.0, 10.0], [12.0, 11.0], [200.0, 0.0]],
            ..Default::default()
        };
        m.fold(&t);
        assert_eq!(
            m.searched.len(),
            2,
            "two spots, one place: {:?}",
            m.searched
        );
        assert!(m.searched.iter().all(|s| s.weight == 1.0));
        m.fold(&t);
        assert!(m.searched.iter().all(|s| s.weight == 1.0), "capped");
        let mut raids = 0;
        while !m.searched.is_empty() {
            m.fold(&RaidTrace::default());
            raids += 1;
        }
        // 1.0 -> 0.6 -> 0.36 -> 0.22 -> 0.13 -> gone.
        assert_eq!(raids, 5);
        assert!(m.is_blank());
    }

    #[test]
    fn a_mangled_memory_is_mended() {
        let mut m = ZoneMemory {
            raids: 3,
            heat: vec![([1, 1], f32::NAN), ([2, 2], 99.0)],
            rvs: vec![Spot {
                at: [f32::INFINITY, 0.0],
                kind: String::new(),
                weight: 1.0,
            }],
            kills: vec![],
            looted: vec![],
            searched: vec![Spot {
                at: [1.0, 2.0],
                kind: String::new(),
                weight: f32::NAN,
            }],
        };
        m.sanitize();
        assert!(m.rvs.is_empty());
        assert_eq!(m.searched[0].weight, 0.0);
        assert!(m.heat.iter().all(|h| h.1.is_finite() && h.1 <= HEAT_MAX));
    }
}
