//! A census of what the world holds, taken every so often over a long run,
//! and the gauge that reads a run's censuses for leaks.
//!
//! Anything that keeps growing over hours of play is a leak: entities,
//! physics bodies, contact pairs, assets, sound voices, memory, the save.
//! [`leaks`] compares the steady part of a run's start with its end, and
//! [`growers`] names the kinds of entity (by their components) that grew,
//! which points at the system that spawns them.
//!
//! `extraction soak --map M --minutes N` plays a long run headless (see
//! [`cli`] and `docs/SOAK.md`); tests in this module hold short runs to the
//! same gauge.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::BTreeMap;

/// One look at the world.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Census {
    /// Seconds of game time since the run started.
    pub t: f32,
    pub entities: usize,
    /// Rigid bodies, and how many dynamic ones are awake.
    pub bodies: usize,
    pub awake: usize,
    pub colliders: usize,
    /// Contact pairs the broad phase is tracking.
    pub contacts: usize,
    pub items: usize,
    pub robots: usize,
    /// Sounds playing (client runs only).
    pub voices: usize,
    /// GPU readbacks waiting (client runs only; with no GPU they never
    /// finish, so they pile up: the harness, not the game).
    pub readbacks: usize,
    pub meshes: usize,
    pub images: usize,
    pub materials: usize,
    /// Lines waiting in the raid's message log.
    pub raid_log: usize,
    /// Resident memory, MB (Linux; 0 elsewhere).
    pub rss_mb: f32,
    /// Mean wall time of a frame since the last census, ms.
    pub tick_ms: f32,
    /// The save this moment would write, bytes.
    pub save_bytes: usize,
    pub phase: String,
    pub raids: u32,
    /// Robots by kind, job and what they're doing ("Walker Collect
    /// Leave"): which ones pile up.
    pub robot_kinds: BTreeMap<String, usize>,
    /// Stowed things gone astray (`stowed::audit`): none, ever.
    pub astray: Vec<String>,
}

/// Entity counts by kind: the sorted short names of an archetype's
/// components.
pub type Kinds = BTreeMap<String, usize>;

/// Entities by kind: the entity's name (or its nearest named ancestor's),
/// digits dropped, and how many components it has. (Component names need
/// Bevy's `debug` feature; names say more anyway.)
pub fn kinds(world: &World) -> Kinds {
    let mut out = Kinds::new();
    for arch in world.archetypes().iter() {
        for e in arch.entities() {
            let mut at = e.id();
            let mut name = None;
            for _ in 0..8 {
                if let Some(n) = world.get::<Name>(at) {
                    name = Some(n.as_str().to_owned());
                    break;
                }
                match world.get::<ChildOf>(at) {
                    Some(p) => at = p.parent(),
                    None => break,
                }
            }
            let name: String = name
                .unwrap_or_else(|| "(unnamed)".into())
                .chars()
                .filter(|c| !c.is_ascii_digit())
                .collect();
            let own = if at == e.id() { "" } else { "under " };
            *out.entry(format!("{own}{name} [{}]", arch.component_count()))
                .or_default() += 1;
        }
    }
    out
}

fn assets<A: Asset>(world: &World) -> usize {
    world.get_resource::<Assets<A>>().map_or(0, |a| a.len())
}

fn rss_mb() -> f32 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1)?.parse::<f32>().ok())
        .map_or(0.0, |pages| pages * 4096.0 / 1e6)
}

/// Takes a census. `tick_ms` is filled in by whoever times the frames.
pub fn take(world: &mut World, t: f32) -> Census {
    let count = |world: &mut World, f: fn(&mut World) -> usize| f(world);
    let bodies = count(world, |w| w.query::<&RigidBody>().iter(w).count());
    // Dynamic bodies that are awake: a settled item should be asleep.
    let awake = count(world, |w| {
        w.query_filtered::<&RigidBody, Without<Sleeping>>()
            .iter(w)
            .filter(|b| b.is_dynamic())
            .count()
    });
    let colliders = count(world, |w| w.query::<&Collider>().iter(w).count());
    let items = count(world, |w| w.query::<&crate::items::Item>().iter(w).count());
    let robots = count(world, |w| {
        w.query::<&crate::robots::Robot>().iter(w).count()
    });
    let voices = count(world, |w| {
        w.query::<&bevy_seedling::prelude::SamplePlayer>()
            .iter(w)
            .count()
    });
    let mut robot_kinds = BTreeMap::new();
    for (r, job, brain) in world
        .query::<(
            &crate::robots::Robot,
            Option<&crate::robots::Job>,
            Option<&crate::robots::Brain>,
        )>()
        .iter(world)
    {
        let brain = brain.map_or("-".to_string(), |b| {
            format!("{b:?}")
                .split([' ', '{', '('])
                .next()
                .unwrap_or("")
                .to_owned()
        });
        let key = format!(
            "{:?} {} {}{}",
            r.kind,
            job.map_or("-".to_string(), |j| format!("{:?}", j.kind)),
            brain,
            if r.wrecked { " wrecked" } else { "" }
        );
        *robot_kinds.entry(key).or_default() += 1;
    }
    let readbacks = count(world, |w| {
        w.query::<&bevy::render::gpu_readback::Readback>()
            .iter(w)
            .count()
    });
    let contacts = world
        .get_resource::<ContactGraph>()
        .map_or(0, |g| g.edges.edge_count());
    let (raid_log, phase, raids) = world
        .get_resource::<crate::raid::Raid>()
        .map_or((0, String::new(), 0), |r| {
            (r.log.len(), format!("{:?}", r.phase), r.raids)
        });
    let phase = phase.split([' ', '{']).next().unwrap_or("").to_owned();
    let save_bytes = if world.get_resource::<crate::world::Zone>().is_some() {
        crate::persist::snapshot(world)
            .to_ron()
            .map_or(0, |s| s.len())
    } else {
        0
    };
    Census {
        t,
        entities: world.archetypes().iter().map(|a| a.len() as usize).sum(),
        bodies,
        awake,
        colliders,
        contacts,
        items,
        robots,
        voices,
        readbacks,
        meshes: assets::<Mesh>(world),
        images: assets::<Image>(world),
        materials: assets::<StandardMaterial>(world),
        raid_log,
        rss_mb: rss_mb(),
        tick_ms: 0.0,
        save_bytes,
        phase,
        raids,
        robot_kinds,
        astray: super::stowed::audit(world),
    }
}

/// A measure that kept growing.
#[derive(Clone, Debug, PartialEq)]
pub struct Leak {
    pub what: &'static str,
    /// The most it reached early on, and the least it fell to late on.
    pub early: f32,
    pub late: f32,
}

impl std::fmt::Display for Leak {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} grew: at most {:.0} early on, never under {:.0} late on",
            self.what, self.early, self.late
        )
    }
}

/// What the gauge reads, and how much growth each may show before it's a
/// leak: (name, value, slack as a fraction, slack absolute). Memory and
/// frame time get more slack (allocators and caches settle slowly; a busy
/// fight costs time).
#[allow(clippy::type_complexity)]
const MEASURES: &[(&str, fn(&Census) -> f32, f32, f32)] = &[
    ("entities", |c| c.entities as f32, 0.05, 200.0),
    ("bodies", |c| c.bodies as f32, 0.05, 20.0),
    ("awake bodies", |c| c.awake as f32, 0.25, 30.0),
    ("colliders", |c| c.colliders as f32, 0.05, 30.0),
    ("contact pairs", |c| c.contacts as f32, 0.25, 100.0),
    ("items", |c| c.items as f32, 0.05, 20.0),
    ("voices", |c| c.voices as f32, 0.25, 16.0),
    ("meshes", |c| c.meshes as f32, 0.05, 50.0),
    ("images", |c| c.images as f32, 0.05, 20.0),
    ("materials", |c| c.materials as f32, 0.05, 50.0),
    ("raid log", |c| c.raid_log as f32, 0.0, 12.0),
    ("memory (MB)", |c| c.rss_mb, 0.25, 300.0),
    ("frame time (ms)", |c| c.tick_ms, 1.0, 4.0),
    ("save (bytes)", |c| c.save_bytes as f32, 0.25, 20_000.0),
];

/// Reads a run for leaks. The first `warm` of the run (a fraction) is the
/// world settling in and is skipped; what's left is cut in thirds, and a
/// measure leaks when the least it reaches in the last third is still above
/// the most it reached in the first, past its slack. Something that comes
/// and goes (a fight, a Surge) overlaps itself and passes; something that
/// only ever piles up doesn't.
pub fn leaks(run: &[Census], warm: f32) -> Vec<Leak> {
    let skip = ((run.len() as f32 * warm) as usize).min(run.len());
    let steady = &run[skip..];
    if steady.len() < 6 {
        return Vec::new();
    }
    let third = steady.len() / 3;
    let (first, last) = (&steady[..third], &steady[steady.len() - third..]);
    let mut out = Vec::new();
    for (what, f, rel, abs) in MEASURES {
        let early = first.iter().map(f).fold(f32::MIN, f32::max);
        let late = last.iter().map(f).fold(f32::MAX, f32::min);
        if late > early * (1.0 + rel) + abs {
            out.push(Leak { what, early, late });
        }
    }
    out
}

/// The kinds of entity that grew most between two censuses' kinds, most
/// first: (kind, before, after).
pub fn growers(before: &Kinds, after: &Kinds, n: usize) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = after
        .iter()
        .map(|(k, a)| (k.clone(), before.get(k).copied().unwrap_or(0), *a))
        .filter(|(_, b, a)| a > b)
        .collect();
    out.sort_by_key(|(_, b, a)| std::cmp::Reverse(a - b));
    out.truncate(n);
    out
}

/// A soak run's record: the censuses, and kinds at the steady start and at
/// the end.
#[derive(Default)]
pub struct Record {
    pub censuses: Vec<Census>,
    pub start_kinds: Kinds,
    pub end_kinds: Kinds,
}

impl Record {
    pub fn leaks(&self) -> Vec<Leak> {
        leaks(&self.censuses, WARM)
    }

    /// A verdict to print: the leaks and what grew.
    pub fn report(&self) -> String {
        let mut s = String::new();
        if let (Some(a), Some(b)) = (self.censuses.first(), self.censuses.last()) {
            s += &format!(
                "{:.0} s of game time, {} raids: entities {} -> {}, bodies {} -> {}, items {} -> {}, \
                 meshes {} -> {}, voices {} -> {}, memory {:.0} -> {:.0} MB, save {} -> {} bytes, \
                 frame {:.2} -> {:.2} ms\n",
                b.t - a.t,
                b.raids,
                a.entities,
                b.entities,
                a.bodies,
                b.bodies,
                a.items,
                b.items,
                a.meshes,
                b.meshes,
                a.voices,
                b.voices,
                a.rss_mb,
                b.rss_mb,
                a.save_bytes,
                b.save_bytes,
                a.tick_ms,
                b.tick_ms,
            );
        }
        let astray: Vec<&String> = self.censuses.iter().flat_map(|c| &c.astray).collect();
        for a in astray.iter().take(8) {
            s += &format!("ASTRAY: {a}\n");
        }
        if astray.len() > 8 {
            s += &format!("ASTRAY: and {} more\n", astray.len() - 8);
        }
        let leaks = self.leaks();
        if leaks.is_empty() {
            s += "no leaks\n";
        }
        for l in &leaks {
            s += &format!("LEAK: {l}\n");
        }
        if let Some(last) = self.censuses.last() {
            let robots: Vec<String> = last
                .robot_kinds
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect();
            s += &format!("robots at the end: {}\n", robots.join(", "));
        }
        for (k, b, a) in growers(&self.start_kinds, &self.end_kinds, 8) {
            s += &format!("  grew {b} -> {a}: {k}\n");
        }
        s
    }
}

/// Fraction of a run skipped as warm-up.
pub const WARM: f32 = 0.2;

/// Plays `app` for `secs` of game time (90 frames a second), taking a
/// census every `every` seconds. `each` runs after every frame (a driver).
pub fn soak(
    app: &mut App,
    secs: f32,
    every: f32,
    mut each: impl FnMut(&mut App, u64),
    mut log: impl FnMut(&Census),
) -> Record {
    let frames = (secs * 90.0) as u64;
    let per = ((every * 90.0) as u64).max(1);
    let mut rec = Record::default();
    let mut busy = 0.0f64;
    let warm_frame = (frames as f32 * WARM) as u64;
    for f in 0..frames {
        let t0 = std::time::Instant::now();
        app.update();
        busy += t0.elapsed().as_secs_f64();
        each(app, f);
        if f % per == per - 1 {
            let mut c = take(app.world_mut(), (f + 1) as f32 / 90.0);
            c.tick_ms = (busy * 1000.0 / per as f64) as f32;
            busy = 0.0;
            log(&c);
            rec.censuses.push(c);
        }
        if f == warm_frame {
            rec.start_kinds = kinds(app.world());
        }
    }
    rec.end_kinds = kinds(app.world());
    rec
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(values: &[usize]) -> Vec<Census> {
        values
            .iter()
            .enumerate()
            .map(|(i, &n)| Census {
                t: i as f32,
                entities: n,
                ..default()
            })
            .collect()
    }

    #[test]
    fn the_gauge_tells_a_leak_from_coming_and_going() {
        // Settling in, then steady with fights coming and going: no leak.
        let mut v = vec![100, 500, 900];
        for i in 0..60 {
            v.push(1000 + if i % 7 < 3 { 300 } else { 0 });
        }
        assert!(
            leaks(&run(&v), 0.05).is_empty(),
            "{:?}",
            leaks(&run(&v), 0.05)
        );
        // A slow, steady pile-up: a leak.
        let v: Vec<usize> = (0..60).map(|i| 1000 + i * 20).collect();
        let found = leaks(&run(&v), 0.05);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].what, "entities");
    }

    /// End to end: a system that leaks an entity every few frames trips the
    /// gauge, which names what grew; the same world without it doesn't.
    #[test]
    fn a_leaking_system_trips_the_gauge() {
        fn leak(mut commands: Commands, mut n: Local<u32>) {
            *n += 1;
            if *n % 3 == 0 {
                commands.spawn(Name::new("Leaked spark"));
            }
        }
        let play = |leaky: bool| {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            if leaky {
                app.add_systems(Update, leak);
            }
            for _ in 0..10 {
                app.world_mut().spawn(Name::new("Rock"));
            }
            soak(&mut app, 60.0, 1.0, |_, _| {}, |_| {})
        };
        let rec = play(true);
        let found = rec.leaks();
        assert!(found.iter().any(|l| l.what == "entities"), "{found:?}");
        assert!(rec.report().contains("Leaked spark"), "{}", rec.report());
        let rec = play(false);
        assert!(
            !rec.leaks().iter().any(|l| l.what == "entities"),
            "{}",
            rec.report()
        );
    }
}
