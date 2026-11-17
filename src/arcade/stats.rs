//! A run's numbers, kept by `record` as it happens: per gun and per
//! machine, what it took to put things down, where the hits landed and
//! did what, accuracy by distance, what reloads and stoppages cost. The
//! summary file and the board's gun card are both written from here, so
//! they and the jsonl's lines agree.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A fight's shots by the distance to the nearest machine standing, m:
/// under the first, up to the second, over it.
pub const BANDS: [f32; 2] = [15.0, 30.0];
pub const BAND_NAMES: [&str; 3] = ["<15m", "15-30m", ">30m"];

/// The range's plates, m from the firing line.
pub const PLATES: [f32; 3] = [10.0, 25.0, 50.0];
pub const PLATE_NAMES: [&str; 3] = ["10m", "25m", "50m"];

/// A gun's shot after this long without one starts a new string, s.
pub const STRING_GAP: f32 = 3.0;

/// The first shots of a string that count for "hits in the first three".
pub const OPENING_SHOTS: u32 = 3;

/// Which band a distance falls in.
pub fn band(d: f32) -> usize {
    BANDS.iter().position(|b| d < *b).unwrap_or(BANDS.len())
}

/// The middle value (of an even count, the mean of the middle two).
pub fn median(v: &[f32]) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    let n = s.len();
    Some(if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) * 0.5
    })
}

/// The value `q` of the way up (0 to 1), nearest rank.
pub fn quantile(v: &[f32], q: f32) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    let i = ((s.len() - 1) as f32 * q).round() as usize;
    Some(s[i])
}

fn pct(hit: u32, of: u32) -> String {
    if of == 0 {
        "-".into()
    } else {
        format!("{:.0}%", 100.0 * hit as f32 / of as f32)
    }
}

fn secs(v: Option<f32>) -> String {
    v.map_or("-".into(), |s| format!("{s:.1} s"))
}

/// One gun's run. A "hit" is a shot that struck (a shotgun's pellets that
/// land are one hit; each pellet is a strike in `parts`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GunStats {
    pub shots: u32,
    pub hits: u32,
    pub reloads: u32,
    pub mid_fight_reloads: u32,
    pub stoppages: u32,
    /// Machines this gun finished, and what they took: the run's fight
    /// shots since the wave came in or the last machine went down, and
    /// the shots that struck the machine.
    pub kills: u32,
    pub kill_shots: u32,
    pub kill_hits: u32,
    pub ttk: Vec<f32>,
    /// Strikes and damage done, by the part struck.
    pub parts: BTreeMap<String, (u32, f32)>,
    /// (shots, hits) in a fight, by [`BANDS`].
    pub fight: [(u32, u32); 3],
    /// (shots, hits) at the range, by [`PLATES`].
    pub range: [(u32, u32); 3],
    /// A reload's cost: the last shot before it to the first after, s,
    /// and whether it was mid-fight.
    pub reload_cost: Vec<(f32, bool)>,
    /// A reload's own time: the magazine out (or the first round in) to
    /// the first shot after, s.
    pub reload_time: Vec<f32>,
    /// A stoppage to the next shot, s.
    pub clear_time: Vec<f32>,
    /// A string's first shot to its first hit, s.
    pub first_hit: Vec<f32>,
    /// Shots and hits among each string's first [`OPENING_SHOTS`].
    pub opening: (u32, u32),
    /// Gaps between shots within a string, s.
    pub gaps: Vec<f32>,
}

impl GunStats {
    pub fn accuracy(&self) -> Option<f32> {
        (self.shots > 0).then(|| self.hits as f32 / self.shots as f32)
    }

    pub fn shots_to_kill(&self) -> Option<f32> {
        (self.kills > 0).then(|| self.kill_shots as f32 / self.kills as f32)
    }

    pub fn hits_to_kill(&self) -> Option<f32> {
        (self.kills > 0).then(|| self.kill_hits as f32 / self.kills as f32)
    }

    pub fn reload_median(&self) -> Option<f32> {
        median(&self.reload_cost.iter().map(|(c, _)| *c).collect::<Vec<_>>())
    }

    pub fn mid_fight_reload_median(&self) -> Option<f32> {
        median(
            &self
                .reload_cost
                .iter()
                .filter(|(_, m)| *m)
                .map(|(c, _)| *c)
                .collect::<Vec<_>>(),
        )
    }

    /// Stoppages per hundred rounds.
    pub fn stoppage_rate(&self) -> Option<f32> {
        (self.shots > 0).then(|| 100.0 * self.stoppages as f32 / self.shots as f32)
    }
}

/// What one kind of machine took to put down.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KillStats {
    pub kills: u32,
    pub ttk: Vec<f32>,
    pub shots: u32,
    pub hits: u32,
}

/// The run's totals.
#[derive(Clone, Debug, Default)]
pub struct RunStats {
    pub guns: BTreeMap<String, GunStats>,
    pub items: BTreeMap<String, u32>,
    pub machines: BTreeMap<String, KillStats>,
    pub cause: Option<String>,
    pub waves_cleared: u32,
    /// Guns in the order first picked up.
    pub picked: Vec<String>,
}

impl RunStats {
    /// The guns that fired, in the order picked up (then by name).
    pub fn used(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .picked
            .iter()
            .filter(|g| self.guns.get(*g).is_some_and(|s| s.shots > 0))
            .cloned()
            .collect();
        for (g, s) in &self.guns {
            if s.shots > 0 && !out.contains(g) {
                out.push(g.clone());
            }
        }
        out
    }

    /// The summary's text: the same numbers as the jsonl's lines.
    /// `down`: the run ended with you down (else a RESTART ended it).
    pub fn summary(&self, wave: u32, score: u32, t: f32, down: bool) -> String {
        let end = if down { "down" } else { "restarted" };
        let mut s = format!(
            "Waves cleared {}, {end} on wave {wave}\nScore {score}, time {}:{:02}\n",
            self.waves_cleared,
            t as u32 / 60,
            t as u32 % 60
        );
        if let Some(c) = &self.cause {
            s += &format!("Taken down by: {c}\n");
        }
        let guns: Vec<(&String, &GunStats)> =
            self.guns.iter().filter(|(_, g)| g.shots > 0).collect();
        s += "\nGUN        shots hits  acc reloads mid-fight stoppages\n";
        for (g, st) in &self.guns {
            s += &format!(
                "{g:<10} {:>5} {:>4} {:>4} {:>7} {:>9} {:>9}\n",
                st.shots,
                st.hits,
                pct(st.hits, st.shots),
                st.reloads,
                st.mid_fight_reloads,
                st.stoppages
            );
        }
        if guns.iter().any(|(_, g)| g.kills > 0) {
            s += "\nTO KILL    kills shots/kill hits/kill median ttk\n";
            for (g, st) in guns.iter().filter(|(_, g)| g.kills > 0) {
                s += &format!(
                    "{g:<10} {:>5} {:>10.1} {:>9.1} {:>10}\n",
                    st.kills,
                    st.shots_to_kill().unwrap_or(0.0),
                    st.hits_to_kill().unwrap_or(0.0),
                    secs(median(&st.ttk))
                );
            }
        }
        if !guns.is_empty() {
            s += &format!(
                "\nACCURACY   fight {} {} {} | range {} {} {}\n",
                BAND_NAMES[0],
                BAND_NAMES[1],
                BAND_NAMES[2],
                PLATE_NAMES[0],
                PLATE_NAMES[1],
                PLATE_NAMES[2]
            );
            for (g, st) in &guns {
                let cell = |(n, h): (u32, u32)| {
                    if n == 0 {
                        "-".to_string()
                    } else {
                        format!("{h}/{n} {}", pct(h, n))
                    }
                };
                s += &format!(
                    "{g:<10} {} | {}\n",
                    st.fight.map(cell).join("  "),
                    st.range.map(cell).join("  ")
                );
            }
        }
        if guns.iter().any(|(_, g)| !g.parts.is_empty()) {
            s += "\nSTRUCK     part: strikes x mean damage\n";
            for (g, st) in guns.iter().filter(|(_, g)| !g.parts.is_empty()) {
                let parts: Vec<String> = st
                    .parts
                    .iter()
                    .map(|(p, (n, d))| format!("{p} {n} x {:.0}", d / (*n).max(1) as f32))
                    .collect();
                s += &format!("{g:<10} {}\n", parts.join(", "));
            }
        }
        if guns.iter().any(|(_, g)| g.reloads > 0) {
            s += "\nRELOAD     median cost (last shot to next) | mid-fight | magazine out to next shot\n";
            for (g, st) in guns.iter().filter(|(_, g)| g.reloads > 0) {
                s += &format!(
                    "{g:<10} {} | {} | {}\n",
                    secs(st.reload_median()),
                    secs(st.mid_fight_reload_median()),
                    secs(median(&st.reload_time))
                );
            }
        }
        if !guns.is_empty() {
            s += "\nHANDLING   stoppages/100 | median to clear | first hit of a string | hits in its first 3 | shot gap median (middle half)\n";
            for (g, st) in &guns {
                let gaps = match (quantile(&st.gaps, 0.25), quantile(&st.gaps, 0.75)) {
                    (Some(a), Some(b)) => {
                        format!("{:.2} s ({a:.2}-{b:.2})", median(&st.gaps).unwrap_or(0.0))
                    }
                    _ => "-".into(),
                };
                s += &format!(
                    "{g:<10} {:.1} | {} | {} | {} | {gaps}\n",
                    st.stoppage_rate().unwrap_or(0.0),
                    secs(median(&st.clear_time)),
                    secs(median(&st.first_hit)),
                    if st.opening.0 == 0 {
                        "-".into()
                    } else {
                        format!("{}/{}", st.opening.1, st.opening.0)
                    },
                );
            }
        }
        if !self.machines.is_empty() {
            s += "\nDOWN       kills median ttk shots/kill hits/kill\n";
            for (m, k) in &self.machines {
                let n = k.kills.max(1) as f32;
                s += &format!(
                    "{m:<10} {:>5} {:>10} {:>10.1} {:>9.1}\n",
                    k.kills,
                    secs(median(&k.ttk)),
                    k.shots as f32 / n,
                    k.hits as f32 / n
                );
            }
        }
        if !self.picked.is_empty() {
            s += &format!("\nPICKED UP  {}\n", self.picked.join(", "));
        }
        if !self.items.is_empty() {
            s += "\nUSED\n";
            for (i, n) in &self.items {
                s += &format!("{i:<16} {n}\n");
            }
        }
        s
    }
}

/// A gun's best so far, across runs (`<arcade dir>/guns.json`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GunBest {
    pub accuracy: Option<f32>,
    pub hits_to_kill: Option<f32>,
    pub ttk: Option<f32>,
    pub reload: Option<f32>,
}

pub type Bests = BTreeMap<String, GunBest>;

fn better(old: Option<f32>, new: Option<f32>, higher: bool) -> Option<f32> {
    match (old, new) {
        (Some(o), Some(n)) => Some(if (n > o) == higher { n } else { o }),
        (o, n) => o.or(n),
    }
}

/// The bests with this run's numbers taken in.
pub fn take_in(bests: &mut Bests, run: &RunStats) {
    for (g, st) in run.guns.iter().filter(|(_, s)| s.shots > 0) {
        let b = bests.entry(g.clone()).or_default();
        b.accuracy = better(b.accuracy, st.accuracy(), true);
        b.hits_to_kill = better(b.hits_to_kill, st.hits_to_kill(), false);
        b.ttk = better(b.ttk, median(&st.ttk), false);
        b.reload = better(b.reload, st.reload_median(), false);
    }
}

/// The board's gun card: each gun used this run, its numbers against its
/// best before this run.
pub fn gun_card(title: &str, run: &RunStats, bests: &Bests) -> String {
    let mut s = format!("GUN CARD\n{title}\n");
    let used = run.used();
    if used.is_empty() {
        s += "\nNo shots yet.\nThe range is east of the bench:\nplates at 10, 25 and 50 m.\n";
        return s;
    }
    for g in used {
        let st = &run.guns[&g];
        let b = bests.get(&g).cloned().unwrap_or_default();
        let vs =
            |now: Option<f32>, best: Option<f32>, higher: bool, f: &dyn Fn(f32) -> String| match (
                now, best,
            ) {
                (Some(n), Some(b)) if (n > b) == higher && n != b => format!("{} NEW BEST", f(n)),
                (Some(n), Some(b)) => format!("{} (best {})", f(n), f(b)),
                (Some(n), None) => f(n),
                (None, _) => "-".into(),
            };
        let p = |v: f32| format!("{:.0}%", v * 100.0);
        let n1 = |v: f32| format!("{v:.1}");
        let sec = |v: f32| format!("{v:.1} s");
        s += &format!(
            "\n{}  {} shots, {} hit\n acc {}\n",
            g.to_uppercase(),
            st.shots,
            st.hits,
            vs(st.accuracy(), b.accuracy, true, &p)
        );
        if st.kills > 0 {
            s += &format!(
                " to kill {} shots, {} hits\n ttk {}\n",
                n1(st.shots_to_kill().unwrap_or(0.0)),
                vs(st.hits_to_kill(), b.hits_to_kill, false, &n1),
                vs(median(&st.ttk), b.ttk, false, &sec)
            );
        }
        if !st.reload_cost.is_empty() {
            s += &format!(
                " reload {}\n",
                vs(st.reload_median(), b.reload, false, &sec)
            );
        }
        let cells = |a: [(u32, u32); 3], names: [&str; 3]| {
            a.iter()
                .zip(names)
                .filter(|((n, _), _)| *n > 0)
                .map(|((n, h), name)| format!("{name} {}", pct(*h, *n)))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let fight = cells(st.fight, BAND_NAMES);
        if !fight.is_empty() {
            s += &format!(" fight {fight}\n");
        }
        let range = cells(st.range, PLATE_NAMES);
        if !range.is_empty() {
            s += &format!(" range {range}\n");
        }
        if st.stoppages > 0 {
            s += &format!(" stoppages {}\n", st.stoppages);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ak() -> GunStats {
        GunStats {
            shots: 20,
            hits: 5,
            reloads: 2,
            mid_fight_reloads: 1,
            stoppages: 1,
            kills: 2,
            kill_shots: 14,
            kill_hits: 4,
            ttk: vec![3.0, 4.0],
            parts: [("Core".to_string(), (3, 120.0))].into(),
            fight: [(10, 4), (6, 1), (0, 0)],
            range: [(4, 0), (0, 0), (0, 0)],
            reload_cost: vec![(2.5, true), (4.5, false)],
            reload_time: vec![1.5, 2.5],
            clear_time: vec![1.2],
            first_hit: vec![0.4, 0.8],
            opening: (6, 2),
            gaps: vec![0.1, 0.12, 0.2, 0.15],
        }
    }

    #[test]
    fn bands_and_middles() {
        assert_eq!(band(3.0), 0);
        assert_eq!(band(15.0), 1);
        assert_eq!(band(29.9), 1);
        assert_eq!(band(31.0), 2);
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0]), Some(2.5));
        assert_eq!(quantile(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.25), Some(2.0));
    }

    #[test]
    fn the_summary_carries_the_numbers() {
        let mut s = RunStats::default();
        s.guns.insert("ak74".into(), ak());
        s.machines.insert(
            "walker".into(),
            KillStats {
                kills: 2,
                ttk: vec![3.0, 4.0],
                shots: 14,
                hits: 4,
            },
        );
        s.items.insert("field_dressing".into(), 1);
        s.cause = Some("walker".into());
        s.waves_cleared = 2;
        s.picked = vec!["ak74".into()];
        let text = s.summary(3, 450, 125.0, true);
        for want in [
            "Waves cleared 2, down on wave 3",
            "Score 450, time 2:05",
            "Taken down by: walker",
            " 25%",
            // To kill: 7 shots and 2 hits a kill, the median of 3 and 4 s.
            "7.0",
            "2.0",
            "3.5 s",
            // By distance and at the range.
            "4/10 40%",
            "0/4 0%",
            "Core 3 x 40",
            // Reload cost, mid-fight, and the magazine's own time.
            "3.5 s | 2.5 s | 2.0 s",
            // Five stoppages a hundred, cleared in 1.2 s.
            "5.0 | 1.2 s",
            "2/6",
            "PICKED UP  ak74",
            "field_dressing",
        ] {
            assert!(text.contains(want), "{want:?} not in\n{text}");
        }
    }

    #[test]
    fn the_card_holds_each_gun_to_its_best() {
        let mut run = RunStats::default();
        run.guns.insert("ak74".into(), ak());
        run.guns.insert("ppsh".into(), GunStats::default());
        run.picked = vec!["ppsh".into(), "ak74".into()];
        assert_eq!(run.used(), vec!["ak74".to_string()], "only guns that fired");
        let mut bests = Bests::new();
        let card = gun_card("wave 1 cleared", &run, &bests);
        assert!(card.contains("AK74  20 shots, 5 hit"), "{card}");
        assert!(card.contains("acc 25%\n"), "{card}");
        assert!(!card.contains("PPSH"), "{card}");
        bests.insert(
            "ak74".into(),
            GunBest {
                accuracy: Some(0.5),
                hits_to_kill: Some(3.0),
                ttk: Some(5.0),
                reload: Some(2.0),
            },
        );
        let card = gun_card("wave 1 cleared", &run, &bests);
        assert!(card.contains("acc 25% (best 50%)"), "{card}");
        assert!(card.contains("2.0 NEW BEST"), "fewer hits to kill: {card}");
        assert!(card.contains("ttk 3.5 s NEW BEST"), "{card}");
        assert!(card.contains("reload 3.5 s (best 2.0 s)"), "{card}");
        assert!(card.contains("range 10m 0%"), "{card}");
        take_in(&mut bests, &run);
        let b = &bests["ak74"];
        assert_eq!(b.accuracy, Some(0.5), "a worse run keeps the best");
        assert_eq!(b.hits_to_kill, Some(2.0));
        assert_eq!(b.ttk, Some(3.5));
        assert_eq!(b.reload, Some(2.0));
        assert!(
            gun_card("x", &RunStats::default(), &bests).contains("No shots yet"),
            "an empty run says where to shoot"
        );
    }
}
