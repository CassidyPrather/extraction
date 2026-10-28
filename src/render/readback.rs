//! Reading a texture back from the GPU, handled exactly once. A
//! `Readback` repeats every frame until it's taken off, and the
//! completions of one frame are triggered without a flush between them:
//! an observer that just despawns its entity runs twice when two land in
//! one frame, and the second despawn is an error (354 of them in one
//! night's logs, burying the real ones; `docs/playtests/2026-10-28`).
//! Every readback in the game goes through [`read_texture`], and its
//! observer starts with [`taken`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};

/// Whether a readback's picture has been taken.
#[derive(Component, Default)]
pub struct ReadOnce(Arc<AtomicBool>);

/// Starts reading `image` back; observe `ReadbackComplete` on what this
/// returns, beginning with [`taken`].
pub fn read_texture<'a>(commands: &'a mut Commands, image: Handle<Image>) -> EntityCommands<'a> {
    commands.spawn((Readback::texture(image), ReadOnce::default()))
}

/// Whether `done` is its readback's first picture: if so, the readback is
/// packed up (despawned, so it reads no more), and the observer goes on;
/// if not (a second in the same frame), it returns.
pub fn taken(done: &ReadbackComplete, once: &Query<&ReadOnce>, commands: &mut Commands) -> bool {
    let Ok(o) = once.get(done.entity) else {
        return false;
    };
    if o.0.swap(true, Ordering::AcqRel) {
        return false;
    }
    commands.entity(done.entity).despawn();
    true
}

/// How many warnings (or worse) `f` logs through `log` on this thread:
/// Bevy's command errors (a despawn of what's gone) warn through it.
#[cfg(test)]
pub fn warnings(f: impl FnOnce()) -> usize {
    use std::cell::Cell;
    thread_local! {
        static COUNTING: Cell<Option<usize>> = const { Cell::new(None) };
    }
    struct Counter;
    impl log::Log for Counter {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.level() <= log::Level::Warn
        }
        fn log(&self, r: &log::Record) {
            if r.level() <= log::Level::Warn {
                COUNTING.with(|c| c.set(c.get().map(|n| n + 1)));
            }
        }
        fn flush(&self) {}
    }
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        if log::set_logger(&Counter).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
    });
    // If another test installed Bevy's logger first (it hands `log`'s
    // records to `tracing`), they come this way instead.
    use bevy::log::tracing;
    use bevy::log::tracing_subscriber::layer::{Context, Layer, SubscriberExt};
    struct Traced;
    impl<S: tracing::Subscriber> Layer<S> for Traced {
        fn on_event(&self, e: &tracing::Event<'_>, _: Context<'_, S>) {
            if *e.metadata().level() <= tracing::Level::WARN {
                COUNTING.with(|c| c.set(c.get().map(|n| n + 1)));
            }
        }
    }
    COUNTING.with(|c| c.set(Some(0)));
    let sub = bevy::log::tracing_subscriber::registry().with(Traced);
    tracing::subscriber::with_default(sub, f);
    COUNTING.with(|c| c.take()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every readback in the game goes through `read_texture`: one that
    /// spawns its own `Readback` is back to being taken twice.
    #[test]
    fn every_readback_is_read_once() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        let mut bad = Vec::new();
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") || p.ends_with("render/readback.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap();
                for (i, l) in text.lines().enumerate() {
                    if l.contains("Readback::") {
                        bad.push(format!("{}:{}: {}", p.display(), i + 1, l.trim()));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "read textures back with render::readback::read_texture:\n{}",
            bad.join("\n")
        );
    }

    /// Two pictures in one frame: the observer goes on for the first only.
    #[test]
    fn two_in_a_frame_are_taken_once() {
        #[derive(Resource, Default)]
        struct Taken(u32);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Image>>()
            .init_resource::<Taken>();
        let world = app.world_mut();
        let mut commands = world.commands();
        let e = read_texture(&mut commands, Handle::default())
            .observe(
                |done: On<ReadbackComplete>,
                 once: Query<&ReadOnce>,
                 mut n: ResMut<Taken>,
                 mut commands: Commands| {
                    if taken(&done, &once, &mut commands) {
                        n.0 += 1;
                    }
                },
            )
            .id();
        world.flush();
        let warned = warnings(|| {
            for _ in 0..2 {
                world.trigger(ReadbackComplete {
                    entity: e,
                    data: vec![0; 4],
                });
            }
            world.flush();
        });
        assert_eq!(warned, 0);
        assert_eq!(world.resource::<Taken>().0, 1);
        assert!(world.get_entity(e).is_err());
    }
}
