//! Waiting: how long from launch until each thing the player waits on is
//! ready, logged once each as `load: <what> at <s> s`. A headset player
//! waits through all of it, so each step's share is what to cut.
//! `main` marks the start (`start`); without it, the first plugin built
//! does. See docs/PERFORMANCE.md, "Waiting".

use bevy::prelude::*;
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

/// The process's start, as near as `main` can say.
pub fn start() {
    let _ = START.get_or_init(Instant::now);
}

/// Seconds since the start.
pub fn since_start() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// Logs `what` once, with the time since the start.
pub fn mark(what: &'static str) {
    static DONE: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let mut done = DONE.lock().unwrap();
    if done.contains(&what) {
        return;
    }
    done.push(what);
    info!("load: {what} at {:.2} s", since_start());
}

pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        start();
        mark("plugins building");
        app.add_systems(PreStartup, || mark("startup begins (window, GPU)"))
            .add_systems(PostStartup, || {
                mark("startup done (zone, terrain, nav, props)")
            })
            .add_systems(First, first_frame)
            .add_systems(Last, settled);
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.add_systems(bevy::render::Render, first_rendered);
        }
    }
}

fn first_frame(mut n: Local<u32>) {
    *n += 1;
    if *n == 1 {
        mark("first frame");
    }
}

fn first_rendered(mut done: Local<bool>) {
    if !*done {
        *done = true;
        mark("first frame rendered");
    }
}

/// The steps after the first frame, as each comes in.
#[allow(clippy::too_many_arguments)]
fn settled(
    eyes: Query<(), With<Camera3d>>,
    forest: crate::world::trees::ForestSettled,
    props: Query<(), With<crate::world::buildings::PropsPending>>,
    models: Res<crate::models::Models>,
    gltfs: Res<Assets<bevy::gltf::Gltf>>,
    server: Res<AssetServer>,
    mut all: Local<bool>,
) {
    if *all {
        return;
    }
    if !eyes.is_empty() {
        mark("a 3D view drawing");
    }
    let forest_ok = forest.settled();
    if forest_ok {
        mark("forest streamed, impostors captured");
    }
    let props_ok = props.is_empty();
    if props_ok {
        mark("buildings dressed (props in)");
    }
    let preload_ok = models.handles().all(|h| {
        gltfs.contains(h)
            || matches!(
                server.get_load_state(h),
                Some(bevy::asset::LoadState::Failed(_))
            )
    });
    if preload_ok {
        mark("every model loaded (preload landed)");
    }
    if forest_ok && props_ok && preload_ok {
        *all = true;
        mark("everything in");
    }
}
