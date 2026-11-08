//! The Zone's pressure on you: how busy you've made it, from what you do.
//!
//! Danger follows what the player does, reads in the world before it
//! arrives, and comes slow and strategic before fast and tactical (the
//! user's direction for early difficulty, 6 November). There's no career
//! counter in it: one account a raid, fed by your own doings.
//!
//! - **Stir**, from your actions, and it dies down: a shot that's heard
//!   ([`SHOT`]), a fight with a machine on you ([`FIGHTING`] a second),
//!   time on hot ground (a region of danger 2 or more, [`HOT`] a second,
//!   and [`INTO_HOT`] each time you go in), and what you carry draws
//!   ([`ATTENTION`] a second, `anomaly::Attention`). It drains at
//!   [`DRAIN`] a second. Shoot one weak machine and leave: it stays low,
//!   and goes.
//! - **Linger**, the slow clock: [`LINGER`] a second out in the Zone, and it
//!   doesn't drain. A cautious half hour isn't safe forever. The Surge is
//!   still the hard clock.
//!
//! The account sets a [`Level`]. Each step up is said in the world before
//! anything it brings ([`stepped`], [`Stepped`]): a fast machine (a drone,
//! hounds) is only sent once a step has been heard [`CUE_LEAD`] before
//! (`director`). Below that, an answer to a noise is one walker coming to
//! look.

use bevy::prelude::*;

/// A shot that's reported (`director::LOUD`, at most one a
/// `director::HEAT_GAP`) stirs this much...
pub const SHOT: f32 = 0.25;
/// ...a second with a machine on you (it sees you and has you)...
pub const FIGHTING: f32 = 1.0 / 60.0;
/// ...a second on hot ground (danger 2 and up: about two and a half
/// minutes there is a step, over the drain), and each time you go in...
pub const HOT: f32 = 1.0 / 120.0;
pub const INTO_HOT: f32 = 0.3;
/// ...and a second of the Zone's attention, per unit
/// (`anomaly::Attention::total`: about 1 for a hard minute's noise, more
/// for an artifact in the pack).
pub const ATTENTION: f32 = 1.0 / 120.0;
/// Stir drains this much a second.
pub const DRAIN: f32 = 1.0 / 300.0;
/// Linger: a second out in the Zone adds this (a step in 15 minutes).
pub const LINGER: f32 = 1.0 / 900.0;
/// A fast machine is sent only this long (s) after the step that allows
/// it was said.
pub const CUE_LEAD: f32 = 10.0;
/// No step is said within this long (s) of the last, and each is one level
/// (slow and strategic before fast and tactical: two steps 28 s apart was
/// the deep end). A step's answer is sent [`CUE_LEAD`] after it's said and
/// a drone from the Gate takes about 40 s to come; 50 s lets each step's
/// answer arrive, and be met, before the next can be said.
pub const STEP_GAP: f32 = 50.0;
/// Danger at or above this is hot ground (`Region::danger`; Map design's
/// read: 0–1 quiet edges, 2–3 hot).
pub const HOT_DANGER: u8 = 2;

/// How busy you've made the Zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Level {
    /// It's at its own work: walkers at their jobs, patrols, the dirigible.
    #[default]
    Quiet,
    /// Up and about: it looks round where you've been, a drone over hot
    /// ground, and a noise may bring one.
    Stirred,
    /// Busy: drones about, responses in force.
    Busy,
    /// Hunting you.
    Hunting,
}

impl Level {
    /// The level above (Hunting stays).
    pub fn next(self) -> Self {
        match self {
            Level::Quiet => Level::Stirred,
            Level::Stirred => Level::Busy,
            Level::Busy | Level::Hunting => Level::Hunting,
        }
    }

    pub fn of(account: f32) -> Self {
        match account {
            a if a < 1.0 => Level::Quiet,
            a if a < 2.0 => Level::Stirred,
            a if a < 3.0 => Level::Busy,
            _ => Level::Hunting,
        }
    }
}

/// What the account was fed this tick (s and counts).
#[derive(Clone, Copy, Debug, Default)]
pub struct Doings {
    /// Out in the Zone (the raid's clock running).
    pub out: bool,
    /// Reported shots this tick.
    pub shots: u32,
    /// Distress flares burst this tick (`distress::STIR` each).
    pub flares: u32,
    /// A machine has you this tick.
    pub fought: bool,
    /// On hot ground this tick, and whether you just went in.
    pub hot: bool,
    pub into_hot: bool,
    /// The Zone's attention on you (`anomaly::Attention::total`).
    pub attention: f32,
}

/// The account, a raid's.
#[derive(Resource, Clone, Debug, Default)]
pub struct Pressure {
    pub stir: f32,
    pub linger: f32,
    pub level: Level,
    /// When the level last stepped up (s, `Time::elapsed`), if it has.
    pub stepped_at: Option<f32>,
    /// What most stirred the last step (for its line).
    pub why: Why,
    /// A machine has had you on the job's own hot ground this raid
    /// ([`job_hot_place`]): till then that ground counts as quiet, and
    /// nothing fast is sent there.
    pub met_at_job: bool,
}

/// The job's place, when the job sends you onto hot ground (Map design's
/// five first jobs, on purpose: the Radio mast, the Charcoal hut, The works,
/// Cinder wood, The summit): its region's name. The first machine met there
/// is slow and weak, so the first contact doesn't bring a big answer
/// (coordinator's ruling): keyed to the place and the job, not a count.
///
/// `map` is the valley's key (`LaunchOptions::map`, "marrow_creek"), what a
/// job's mark names; not `MapDef::name`, its display name ("Marrow Creek").
pub fn job_hot_place(
    zone: &crate::world::Zone,
    map: &str,
    progression: Option<&crate::progression::Progression>,
) -> Option<String> {
    let (job_map, at) = progression?.0.job.as_ref()?.mark()?;
    if job_map != map {
        return None;
    }
    zone.map
        .region_at(at)
        .filter(|r| r.danger >= HOT_DANGER)
        .map(|r| r.name.clone())
}

/// How near the pad the truck's called to the wait there is: no hot
/// ground within it (coordinator's ruling, #86).
pub const RV_WAIT: f32 = 25.0;

/// Whether `at` is hot ground for the account: a region of [`HOT_DANGER`]
/// or more, but not within [`RV_WAIT`] of `called`, the pad the truck's
/// been called to (`called_pad`): the run in can be tense, the wait at the
/// pad can't. Only that pad, only while it's called (it was every pad's
/// `director::SURVEY_CLEAR`, called or not, and blanked 57-65% of the hot
/// ground on the ways to the jobs). Survey drones keep off every pad's
/// 80 m on their own (`director::survey_places`).
pub fn hot_at(zone: &crate::world::Zone, at: [f32; 2], called: Option<[f32; 2]>) -> bool {
    zone.map
        .region_at(at)
        .is_some_and(|r| r.danger >= HOT_DANGER)
        && called.is_none_or(|c| Vec2::from_array(c).distance(Vec2::from_array(at)) > RV_WAIT)
}

/// The pad the truck's been called to, or is waiting at: none before the
/// call, nor for the short exfil (back to the base).
pub fn called_pad(zone: &crate::world::Zone, phase: &crate::raid::Phase) -> Option<[f32; 2]> {
    let rv = match *phase {
        crate::raid::Phase::TruckCalled { rv, .. }
        | crate::raid::Phase::TruckWaiting { rv, .. } => rv,
        _ => return None,
    };
    zone.map
        .markers_of(&extraction_core::map::MarkerKind::Rendezvous)
        .nth(rv)
        .map(|m| m.at)
}

/// Whether `p` is on the job's hot place ([`job_hot_place`]).
pub fn on_job_place(zone: &crate::world::Zone, place: Option<&str>, p: Vec3) -> bool {
    place.is_some_and(|name| {
        zone.map
            .region_at([p.x, p.z])
            .is_some_and(|r| r.name == name)
    })
}

/// What moved it up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Why {
    #[default]
    Time,
    Noise,
    Fight,
    Ground,
    Carried,
}

impl Pressure {
    pub fn account(&self) -> f32 {
        self.stir + self.linger
    }

    /// One tick: fed `d`, `dt` seconds, at `now`. The new level if it
    /// stepped up.
    pub fn feed(&mut self, d: Doings, dt: f32, now: f32) -> Option<Level> {
        if !d.out {
            *self = Pressure::default();
            return None;
        }
        let parts = [
            (
                Why::Noise,
                SHOT * d.shots as f32 + super::distress::STIR * d.flares as f32,
            ),
            (Why::Fight, if d.fought { FIGHTING * dt } else { 0.0 }),
            (
                Why::Ground,
                if d.hot { HOT * dt } else { 0.0 } + if d.into_hot { INTO_HOT } else { 0.0 },
            ),
            (Why::Carried, ATTENTION * d.attention.max(0.0) * dt),
        ];
        let fed: f32 = parts.iter().map(|p| p.1).sum();
        self.stir = (self.stir + fed - DRAIN * dt).max(0.0);
        self.linger += LINGER * dt;
        let mut level = Level::of(self.account());
        let up = level > self.level;
        if up {
            // Not within the gap of the last step, and one level at a time.
            if self.stepped_at.is_some_and(|t| now - t < STEP_GAP) {
                return None;
            }
            level = level.min(self.level.next());
        }
        // (Down it goes quietly; it was said going up.)
        self.level = level;
        if up {
            self.stepped_at = Some(now);
            self.why = parts
                .iter()
                .filter(|p| p.1 > 0.0)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map_or(Why::Time, |p| p.0);
            return Some(level);
        }
        None
    }

    /// Whether a fast machine may be sent now: a step up was said at least
    /// [`CUE_LEAD`] ago, and it's still stirred.
    pub fn fast_allowed(&self, now: f32) -> bool {
        self.level >= Level::Stirred && self.stepped_at.is_some_and(|t| now - t >= CUE_LEAD)
    }
}

/// A step up, said in the world (`raid::Raid::say`) before what it brings:
/// Audio gives it a sound (`at`: where you are).
#[derive(Message, Clone, Copy, Debug)]
pub struct Stepped {
    pub level: Level,
    pub why: Why,
    pub at: Vec3,
}

/// What's said at a step up, by what moved it (Onboarding's words to
/// own; these say what each step means).
pub fn stepped(level: Level, why: Why) -> &'static str {
    match (level, why) {
        (Level::Quiet, _) => "",
        (Level::Stirred, Why::Time) => {
            "Someone's been about too long. A whine from the Gate's side, rising."
        }
        (Level::Stirred, Why::Ground) => {
            "You're on their ground now. A whine from the Gate's side, rising."
        }
        (Level::Stirred, _) => "They heard that. A whine from the Gate's side, rising.",
        (Level::Busy, _) => "Machine traffic on the wind, all of it this way.",
        (Level::Hunting, _) => "The whole Zone is looking at you.",
    }
}

pub struct PressurePlugin;

impl Plugin for PressurePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pressure>()
            .init_resource::<Doing>()
            .add_message::<Stepped>()
            .add_systems(
                FixedUpdate,
                tally_pressure
                    .after(super::director::hear_reports)
                    .before(super::director::run_director),
            );
    }
}

/// This tick's reported shots (`director::hear_reports` counts them),
/// and whether you were on hot ground last tick.
#[derive(Resource, Debug, Default)]
pub struct Doing {
    pub shots: u32,
    /// Distress flares that burst this tick (`distress`).
    pub flares: u32,
    pub was_hot: bool,
}

#[allow(clippy::too_many_arguments)]
fn tally_pressure(
    mut pressure: ResMut<Pressure>,
    mut doing: ResMut<Doing>,
    raid: Option<ResMut<crate::raid::Raid>>,
    zone: Res<crate::world::Zone>,
    players: Query<
        (Entity, &GlobalTransform, Option<&crate::anomaly::Attention>),
        With<crate::rig::LocalPlayer>,
    >,
    robots: Query<(&super::Awareness, Has<super::broken::Broken>)>,
    time: Res<Time>,
    mut out: MessageWriter<Stepped>,
    progression: Option<Res<crate::progression::Progression>>,
    opts: Option<Res<crate::launch::LaunchOptions>>,
) {
    let Some(mut raid) = raid else {
        return;
    };
    let Some((me, gt, attention)) = players.iter().next() else {
        return;
    };
    let at = gt.translation();
    let in_zone = matches!(
        raid.phase,
        crate::raid::Phase::Out
            | crate::raid::Phase::TruckCalled { .. }
            | crate::raid::Phase::TruckWaiting { .. }
    );
    let place = opts
        .as_deref()
        .and_then(|o| job_hot_place(&zone, &o.map, progression.as_deref()));
    let at_job = on_job_place(&zone, place.as_deref(), at);
    // The job's own hot ground is quiet till a machine's had you there.
    let called = called_pad(&zone, &raid.phase);
    let hot = hot_at(&zone, [at.x, at.z], called) && !(at_job && !pressure.met_at_job);
    // The broken walker (a dead gun: the fight a newcomer is meant to win)
    // feeds nothing: not as a fight, nor the shots at it while it's the
    // only one on you (coordinator's ruling).
    let on_me = |a: &super::Awareness| a.target == Some(me) && a.can_see_target;
    let fought = robots.iter().any(|(a, broken)| !broken && on_me(a));
    let only_broken = !fought && robots.iter().any(|(a, broken)| broken && on_me(a));
    let shots = std::mem::take(&mut doing.shots);
    let d = Doings {
        out: in_zone,
        shots: if only_broken { 0 } else { shots },
        flares: std::mem::take(&mut doing.flares),
        fought,
        hot,
        into_hot: hot && !doing.was_hot,
        attention: attention.map_or(0.0, |a| a.total()),
    };
    doing.was_hot = hot && in_zone;
    let stepped_up = pressure.feed(d, time.delta_secs(), time.elapsed_secs());
    if fought && at_job && in_zone {
        pressure.met_at_job = true;
    }
    if let Some(level) = stepped_up {
        let why = pressure.why;
        info!("pressure: {level:?} ({why:?}), {:.2}", pressure.account());
        raid.say(stepped(level, why));
        out.write(Stepped { level, why, at });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALLEYS: [&str; 5] = [
        "marrow_creek",
        "kettle_pass",
        "cinder_works",
        "rime_fell",
        "sallow_reservoir",
    ];

    /// Points round `c` at `r` m, sixteen of them.
    fn ring(c: [f32; 2], r: f32) -> impl Iterator<Item = [f32; 2]> {
        (0..16).map(move |k| {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
    }

    fn hot_region(zone: &crate::world::Zone, p: [f32; 2]) -> bool {
        zone.map
            .region_at(p)
            .is_some_and(|r| r.danger >= HOT_DANGER)
    }

    /// The wait at the pad the truck's called to isn't hot ground: within
    /// [`RV_WAIT`] of it, never, in any valley, even where its region is
    /// (Marrow's Bus depot). Only that pad, only while it's called: past
    /// 25 m, at any other pad, or with no truck called, hot ground is hot
    /// (coordinator's ruling, #86: it was every pad's 80 m, called or not,
    /// and blanked 57-65% of the hot ground on the way to the jobs).
    #[test]
    fn only_the_called_pads_wait_is_quiet() {
        let mut hot_pads = vec![];
        for map in VALLEYS {
            let zone = crate::world::Zone::load(map);
            let pads: Vec<[f32; 2]> = zone
                .map
                .markers_of(&extraction_core::map::MarkerKind::Rendezvous)
                .map(|m| m.at)
                .collect();
            for (i, rv) in pads.iter().enumerate() {
                let name = format!("{map} pad {i}");
                for r in [0.0, 5.0, 12.0, 20.0, 24.0] {
                    for p in ring(*rv, r) {
                        assert!(!hot_at(&zone, p, Some(*rv)), "{name} at {r} m is hot");
                    }
                }
                // Hot ground round it stays hot: uncalled, past the wait,
                // or with the truck called to another pad.
                let other = pads.iter().find(|o| *o != rv).copied();
                for r in [0.0, 10.0, 30.0, 50.0, 79.0] {
                    for p in ring(*rv, r).filter(|p| hot_region(&zone, *p)) {
                        assert!(hot_at(&zone, p, None), "{name} at {r} m, uncalled");
                        if let Some(o) = other.filter(|o| {
                            Vec2::from_array(*o).distance(Vec2::from_array(p)) > RV_WAIT
                        }) {
                            assert!(hot_at(&zone, p, Some(o)), "{name} at {r} m, other called");
                        }
                        if r > RV_WAIT {
                            assert!(hot_at(&zone, p, Some(*rv)), "{name} at {r} m, called");
                        }
                    }
                }
                if hot_region(&zone, *rv) {
                    hot_pads.push(format!("{map} {i}"));
                }
            }
        }
        // The rule has work to do: some pads are in hot regions.
        assert!(!hot_pads.is_empty(), "no pad in hot ground");
    }

    /// How much of each valley's hot ground `hot_at` blanks, on its own
    /// (before the job's place waits on `met_at_job`): none with no truck
    /// called, and with one called, only its pad's wait, a sliver of it
    /// (10% at most: Marrow's Bus depot, a pad in a small hot region, is
    /// the most).
    #[test]
    fn hot_at_blanks_only_a_sliver_of_hot_ground() {
        for map in VALLEYS {
            let zone = crate::world::Zone::load(map);
            let pads: Vec<[f32; 2]> = zone
                .map
                .markers_of(&extraction_core::map::MarkerKind::Rendezvous)
                .map(|m| m.at)
                .collect();
            let half = zone.map.size / 2.0;
            let mut hot = vec![];
            let mut x = -half;
            while x <= half {
                let mut z = -half;
                while z <= half {
                    if hot_region(&zone, [x, z]) {
                        hot.push([x, z]);
                    }
                    z += 4.0;
                }
                x += 4.0;
            }
            assert!(!hot.is_empty(), "{map} has hot ground");
            let blank = |called: Option<[f32; 2]>| {
                hot.iter().filter(|p| !hot_at(&zone, **p, called)).count() as f32 / hot.len() as f32
            };
            assert_eq!(blank(None), 0.0, "{map} uncalled");
            // The old rule: every pad's 80 m, called or not.
            let old = hot
                .iter()
                .filter(|p| {
                    pads.iter().any(|rv| {
                        Vec2::from_array(*rv).distance(Vec2::from_array(**p))
                            <= super::super::director::SURVEY_CLEAR
                    })
                })
                .count() as f32
                / hot.len() as f32;
            for (i, rv) in pads.iter().enumerate() {
                let b = blank(Some(*rv));
                eprintln!(
                    "{map} pad {i}: called blanks {:.1}% (every pad's 80 m: {:.1}%)",
                    b * 100.0,
                    old * 100.0
                );
                assert!(b <= 0.10, "{map} pad {i} called blanks {:.1}%", b * 100.0);
            }
        }
    }

    /// Every way on every valley that walks from quiet ground into hot
    /// (`extraction_core::map_gradient::crossings`) tells of it from the
    /// place itself on the approach (a sign, a wreck, a gate, a sentry, an
    /// anomaly: not only a red lamp far off), so a newcomer stepping onto
    /// the ground that fills the account and voices its step was told
    /// before it. A machine seen on the way is a bonus, not the promise:
    /// it moves, dies and is drawn away (coordinator, #82). Known gaps are
    /// named here till they're closed, and this fails once one is (take
    /// it off).
    #[test]
    fn every_quiet_to_hot_crossing_is_told_by_its_place() {
        use extraction_core::map::MapDef;
        use extraction_core::terrain::Heightmap;
        // (valley, into, near, the way, part of its name): waiting, and on
        // whom.
        const GAPS: &[(&str, &str, [f32; 2], &str, &str)] = &[
            (
                "marrow_creek",
                "Radio mast",
                [-69.0, -66.0],
                "mast_relay",
                "the jeep on the north way, waiting on Verifier's loot fix",
            ),
            // Found by this gauge, not shown by `probe_gradient` (it prints
            // the first crossing of a 20 m cell; these are another way in
            // there): Map design's to place or rule out.
            (
                "cinder_works",
                "The works",
                [32.0, 23.0],
                "route to Works yard",
                "Map design's",
            ),
        ];
        let mut untold = vec![];
        let mut closed = vec![];
        for (id, text) in extraction_core::map::VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let comfy = extraction_core::map_nav::zone_grid(
                &map,
                &hm,
                extraction_core::map_lint::COMFORT_SLOPE,
            );
            let r = extraction_core::map_lint::lint(&map, &hm);
            let cs = extraction_core::map_gradient::crossings(&map, &hm, &comfy, &r.routes);
            let gap = |c: &extraction_core::map_gradient::Crossing| {
                GAPS.iter().find(|g| {
                    g.0 == id
                        && g.1 == c.into.0
                        && c.way.contains(g.3)
                        && Vec2::from_array(g.2).distance(Vec2::from_array(c.at)) < 25.0
                })
            };
            for c in &cs {
                let told = c.signals.iter().any(|s| s.0 != "red lamp over it");
                match (told, gap(c)) {
                    (false, None) => untold.push(format!(
                        "{id}: {} ({}) -> {} ({}) at ({:.0}, {:.0}) on {}",
                        c.from.0, c.from.1, c.into.0, c.into.1, c.at[0], c.at[1], c.way
                    )),
                    (true, Some(g)) => closed.push(format!("{id} {} on {} ({})", g.1, g.3, g.4)),
                    _ => {}
                }
            }
        }
        assert!(untold.is_empty(), "crossings nothing tells of: {untold:#?}");
        assert!(closed.is_empty(), "told now, take it off GAPS: {closed:?}");
    }

    fn out() -> Doings {
        Doings {
            out: true,
            ..Default::default()
        }
    }

    fn run(p: &mut Pressure, secs: f32, mut d: impl FnMut(f32) -> Doings) -> Vec<(f32, Level)> {
        let dt = 1.0 / 90.0;
        let mut steps = vec![];
        let t0 = p.stepped_at.unwrap_or(0.0);
        for i in 0..(secs * 90.0) as usize {
            let t = t0 + i as f32 * dt;
            if let Some(l) = p.feed(d(t), dt, t) {
                steps.push((t, l));
            }
        }
        steps
    }

    /// Quiet, it stays quiet a good while; lingering still counts, slowly,
    /// and says so when it steps: a cautious half hour isn't risk-free.
    #[test]
    fn lingering_steps_slowly_and_says_so() {
        let mut p = Pressure::default();
        let steps = run(&mut p, 30.0 * 60.0, |_| out());
        let first = steps.first().expect("it stepped in half an hour");
        assert!(
            (14.0 * 60.0..16.0 * 60.0).contains(&first.0),
            "first step at {:.0} s",
            first.0
        );
        assert_eq!(p.why, Why::Time);
        assert!(stepped(Level::Stirred, Why::Time).contains("too long"));
        assert!(p.level >= Level::Stirred, "half an hour: {p:?}");
    }

    /// Shoot one weak machine and leave: it stirs, short of a step, and
    /// dies down.
    #[test]
    fn a_shot_or_two_is_a_small_answer_that_dies_down() {
        let mut p = Pressure::default();
        let mut first = true;
        let steps = run(&mut p, 60.0, |_| Doings {
            shots: if std::mem::take(&mut first) { 2 } else { 0 },
            ..out()
        });
        assert!(steps.is_empty(), "{steps:?}");
        assert!(p.stir > 0.25, "it stirred: {p:?}");
        run(&mut p, 150.0, |_| out());
        assert!(p.stir < 0.05, "it died down: {p:?}");
        assert!(!p.fast_allowed(1e4));
    }

    /// Keep shooting, and fight, and it builds: a step, said, and only
    /// some seconds after it may anything fast come.
    #[test]
    fn keeping_it_up_builds_and_fast_waits_on_the_cue() {
        let mut p = Pressure::default();
        // A shot every 8 s (one a heat gap), and a machine on you.
        let steps = run(&mut p, 60.0, |t| Doings {
            shots: ((t * 90.0) as usize % 720 == 0) as u32,
            fought: true,
            ..out()
        });
        let (at, l) = *steps.first().expect("it stepped");
        assert_eq!(l, Level::Stirred);
        assert!(!p.fast_allowed(at + CUE_LEAD - 0.5), "fast before its cue");
        assert!(p.fast_allowed(at + CUE_LEAD + 0.5) || p.level > Level::Stirred);
        assert!(matches!(p.why, Why::Noise | Why::Fight), "{p:?}");
    }

    /// However hard it's pushed (a shot every 2 s, a fight, hot ground),
    /// the steps come one level at a time and [`STEP_GAP`] apart (they
    /// came 28 s apart, Stirred to Busy to Hunting in under a minute:
    /// lost's job 2).
    #[test]
    fn steps_are_one_level_and_a_gap_apart() {
        let mut p = Pressure::default();
        let steps = run(&mut p, 240.0, |t| Doings {
            shots: ((t * 90.0) as usize % 180 == 0) as u32,
            fought: true,
            hot: true,
            ..out()
        });
        assert_eq!(
            steps.first().map(|s| s.1),
            Some(Level::Stirred),
            "{steps:?}"
        );
        assert!(steps.len() >= 3, "{steps:?}");
        for w in steps.windows(2) {
            assert!(w[1].0 - w[0].0 >= STEP_GAP - 0.05, "{steps:?}");
            assert_eq!(w[1].1, w[0].1.next(), "{steps:?}");
        }
    }

    /// Going onto hot ground is a step sooner than anywhere else.
    #[test]
    fn hot_ground_stirs_it() {
        let mut quiet = Pressure::default();
        let mut hot = Pressure::default();
        let mut entered = false;
        run(&mut quiet, 300.0, |_| out());
        let s = run(&mut hot, 300.0, |_| Doings {
            hot: true,
            into_hot: !std::mem::replace(&mut entered, true),
            ..out()
        });
        assert!(hot.account() > quiet.account() + 1.0, "{hot:?} {quiet:?}");
        assert_eq!(s.first().map(|s| s.1), Some(Level::Stirred));
        assert_eq!(hot.why, Why::Ground);
    }

    /// Back at base it's all forgotten: each raid starts quiet.
    #[test]
    fn each_raid_starts_quiet() {
        let mut p = Pressure {
            stir: 2.5,
            linger: 0.5,
            level: Level::Hunting,
            ..Default::default()
        };
        p.feed(Doings::default(), 0.1, 1.0);
        assert_eq!(p.level, Level::Quiet);
        assert_eq!(p.account(), 0.0);
    }
}
