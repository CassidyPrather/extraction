//! Soak and fuzz: what breaks over long play and odd input.
//!
//! - [`census`]: counts what the world holds (entities, bodies, assets,
//!   voices, memory, tick time, save size) over a long run, and a gauge
//!   that fails when any of it keeps growing.
//! - `chaos` (tests): a monkey at the headset's controls doing random but
//!   plausible things with its hands.
//! - `save_fuzz` (tests): saving at odd moments, and mangled save files.
//! - `physics` (tests): thrown, stacked and buried items.
//!
//! Findings and fixes are written up in `docs/SOAK.md`.

pub mod census;
#[cfg(test)]
mod chaos;
pub mod drive;
pub mod floors;
pub mod ledges;
#[cfg(test)]
mod physics;
pub mod quiet;
#[cfg(test)]
mod save_fuzz;
#[cfg(test)]
mod stowed;

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    /// Sounds that never finish (no output device) don't pile up: past
    /// the cap the oldest one-shots go, and looping beds are left alone.
    #[test]
    fn voices_that_never_finish_are_retired() {
        use bevy_seedling::prelude::*;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Last, crate::audio::retire_voices);
        let bed = app
            .world_mut()
            .spawn(SamplePlayer::new(Handle::default()).looping())
            .id();
        let n = crate::audio::VOICE_CAP + 300;
        for _ in 0..n {
            app.world_mut().spawn(SamplePlayer::new(Handle::default()));
            app.update();
        }
        let left = app
            .world_mut()
            .query::<&SamplePlayer>()
            .iter(app.world())
            .count();
        assert_eq!(left, crate::audio::VOICE_CAP + 1, "the cap, and the bed");
        assert!(app.world().get_entity(bed).is_ok());
    }

    /// A robot brought down stays as a wreck until the Surge sweeps the
    /// Zone; then it goes with everything else. (Ordinary wrecks used to
    /// stay all session: only the heavies' were swept.)
    #[test]
    fn the_surge_sweeps_wrecks_away() {
        use crate::robots::{Job, JobKind, Robot};
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(10.0, 10.0);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let walker = {
            let mut c = Commands::new(&mut queue, app.world());
            let job = Job {
                kind: JobKind::Survey,
                site: at,
                duration: 999.0,
                exit: at,
            };
            crate::robots::walker::spawn_walker(&mut c, &zone, at, job)
        };
        queue.apply(app.world_mut());
        for _ in 0..10 {
            app.update();
        }
        app.world_mut().get_mut::<Robot>(walker).unwrap().wrecked = true;
        for _ in 0..10 {
            app.update();
        }
        assert!(app.world().get_entity(walker).is_ok(), "a wreck stays");
        crate::persist::reset_zone(app.world_mut());
        app.update();
        assert!(
            app.world().get_entity(walker).is_err(),
            "the Surge took the wreck"
        );
    }

    /// A system's `Local` map or set keyed by `Entity` must forget
    /// entities: `retain`, `remove`, `clear` or rebuild it. Every one the
    /// soak audit found unpruned held each robot, door or item it ever saw
    /// for the whole session. The few that only ever see a fixed handful
    /// (the hands, the player, the buildings) are listed with why.
    #[test]
    fn entity_keyed_locals_forget_what_is_gone() {
        // (file, binding): why it can't grow.
        const BOUNDED: &[(&str, &str, &str)] = &[
            ("src/audio.rs", "bench_was", "the truck's benches"),
            (
                "src/audio.rs",
                "was",
                "players' bodies, packs, buttons: a fixed few",
            ),
            ("src/rig/haptics.rs", "was", "screen buttons, made once"),
            ("src/render/props.rs", "lit", "keyed by hand"),
            ("src/render/props.rs", "made", "keyed by hand"),
            ("src/weapons/faults.rs", "last", "keyed by hand"),
            ("src/weapons/mod.rs", "boxes", "keyed by hand"),
            ("src/weapons/mod.rs", "clips", "keyed by hand"),
            ("src/interact/touch.rs", "last", "keyed by hand"),
            ("src/destruction/breach.rs", "geos", "buildings"),
            ("src/world/snow.rs", "last", "players' footsteps"),
            ("src/world/live.rs", "told", "players, told once"),
            ("src/items/usage.rs", "last", "keyed by player and hand"),
            (
                "src/wildlife/bodies.rs",
                "known",
                "removed as each carcass is taken (a retain would lose a death before it's marked)",
            ),
            ("src/world/underground.rs", "inside", "players"),
            ("src/wildlife/rats.rs", "*", "buildings"),
            ("src/world/furnace.rs", "*", "buildings"),
            ("src/dev.rs", "*", "dev tools"),
        ];
        let mut bad = Vec::new();
        let mut files = Vec::new();
        let mut stack = vec![std::path::PathBuf::from("src")];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    files.push(p);
                }
            }
        }
        for path in files {
            let file = path.to_string_lossy().replace('\\', "/");
            let src = std::fs::read_to_string(&path).unwrap();
            let mut from = 0;
            while let Some(i) = src[from..].find(": Local<") {
                let at = from + i;
                from = at + 1;
                let line_end = src[at..].find('\n').map_or(src.len(), |n| at + n);
                let ty = &src[at..line_end];
                let keyed = [
                    "HashMap<Entity",
                    "HashSet<Entity",
                    "HashMap<(Entity",
                    "HashSet<(Entity",
                    "Vec<Entity>",
                    "Vec<(Entity",
                ]
                .iter()
                .any(|k| ty.contains(k));
                if !keyed {
                    continue;
                }
                let line_start = src[..at].rfind('\n').map_or(0, |n| n + 1);
                let name = src[line_start..at]
                    .trim()
                    .trim_start_matches("mut ")
                    .to_string();
                if BOUNDED
                    .iter()
                    .any(|(f, n, _)| file.ends_with(f) && (*n == "*" || *n == name))
                {
                    continue;
                }
                // The function body: from the parameter list's `{` to its
                // matching `}`.
                let Some(open) = src[at..].find(") {").map(|n| at + n + 2) else {
                    continue;
                };
                let mut depth = 0;
                let mut close = src.len();
                for (k, ch) in src[open..].char_indices() {
                    match ch {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                close = open + k;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let body = &src[open..close];
                // (Not `.remove(`: removing one case, as a hand lets go, says
                // nothing of entities despawned while in there; Destruction's
                // glazing map hid behind one.)
                let forgets = [".retain(", ".clear()", ".drain(", " = "].iter().any(|op| {
                    body.contains(&format!("{name}{op}")) || body.contains(&format!("*{name}{op}"))
                }) || body.contains(&format!("&mut *{name}"));
                if !forgets {
                    let line = src[..at].matches('\n').count() + 1;
                    bad.push(format!("{file}:{line} `{name}`"));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "entity-keyed Locals that never forget an entity (prune them, or list \
             them in BOUNDED with why they can't grow):\n{}",
            bad.join("\n")
        );
    }
}
