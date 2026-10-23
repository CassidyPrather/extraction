//! Waiting: how long from launch until each thing the player waits on is
//! ready, logged once each as `load: <what> at <s> s`. A headset player
//! waits through all of it, so each step's share is what to cut.
//! `main` marks the start (`start`); without it, the first plugin built
//! does. See docs/PERFORMANCE.md, "Waiting".

use bevy::prelude::*;
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

/// Frames begun: a step that waits on frames rather than work takes as
/// many on any machine, so its seconds here say little about the headset's.
static FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

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
    info!(
        "load: {what} at {:.2} s (frame {})",
        since_start(),
        FRAMES.load(std::sync::atomic::Ordering::Relaxed)
    );
}

/// What the GPU can't draw with yet: drawn things whose materials aren't
/// prepared (their textures still loading), and pipelines still compiling.
/// Bevy draws nothing for either, so a valley's ground and buildings are
/// missing until it's 0: on the software renderer the ground's pipelines
/// took 18 s, and a shot at frame 30 had trees floating over nothing.
/// Counted in the render world once a frame's draws are queued, and only
/// clear after [`CLEAR_FRAMES`] frames in a row with none (a material that
/// just readied queues its pipelines a frame or two later); read in the
/// main world. Without a render world, always clear.
#[derive(Resource, Clone)]
pub struct Unprepared(std::sync::Arc<std::sync::atomic::AtomicU32>);

/// Render frames in a row with nothing waiting before the GPU counts as
/// ready.
const CLEAR_FRAMES: u32 = 4;

impl Unprepared {
    /// Whether the GPU has had nothing waiting for [`CLEAR_FRAMES`] frames.
    pub fn clear(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed) >= CLEAR_FRAMES
    }
}

/// Whether the world is in, as the player would see it: the buildings
/// dressed, the forest built with its impostors taken, every drawn
/// thing's material ready on the GPU, and the warm-up shelf's pipelines
/// for the eyes it's warming (the headset's, before its session) built. The headset's session, a screenshot
/// and "everything in" wait on it.
#[derive(bevy::ecs::system::SystemParam)]
pub struct WorldReady<'w, 's> {
    forest: crate::world::trees::ForestSettled<'w, 's>,
    props: Query<'w, 's, (), With<crate::world::buildings::PropsPending>>,
    unprepared: Option<Res<'w, Unprepared>>,
    warmed: Option<Res<'w, super::warmup::Warmed>>,
}

impl WorldReady<'_, '_> {
    pub fn forest(&self) -> bool {
        self.forest.settled()
    }

    pub fn props(&self) -> bool {
        self.props.is_empty()
    }

    /// Every drawn thing's material and pipeline ready, the warm-up
    /// shelf's for what's still to be seen included (`warmup::Warmed`).
    pub fn materials(&self) -> bool {
        self.warmed.as_ref().is_none_or(|w| w.0)
            && self.unprepared.as_ref().is_none_or(|u| u.clear())
    }

    pub fn ready(&self) -> bool {
        self.forest() && self.props() && self.materials()
    }
}

/// Present once everything's in ("everything in"): work paced over frames
/// so play doesn't hitch can go faster before it, when no one's playing.
#[derive(Resource)]
pub struct Loaded;

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
            .add_systems(Last, (settled, leaving));
        let unprepared = Unprepared(std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)));
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render
                .insert_resource(unprepared.clone())
                .add_systems(bevy::render::Render, first_rendered)
                .add_systems(
                    bevy::render::Render,
                    count_unprepared.in_set(bevy::render::RenderSystems::Cleanup),
                );
            app.insert_resource(unprepared);
        }
    }
}

fn first_frame() {
    if FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
        mark("first frame");
    }
}

/// The way out: a valley change waits through the close as well.
fn leaving(exit: MessageReader<AppExit>) {
    if !exit.is_empty() {
        mark("leaving (exit asked)");
    }
}

fn count_unprepared(
    out: Res<Unprepared>,
    pipelines: Option<Res<bevy::render::render_resource::PipelineCache>>,
    instances: Option<Res<bevy::pbr::RenderMaterialInstances>>,
    prepared: Option<
        Res<bevy::render::erased_render_asset::ErasedRenderAssets<bevy::pbr::PreparedMaterial>>,
    >,
) {
    use bevy::render::render_resource::CachedPipelineState;
    let compiling = pipelines.map_or(0, |c| {
        c.pipelines()
            .filter(|p| {
                matches!(
                    p.state,
                    CachedPipelineState::Queued | CachedPipelineState::Creating(_)
                )
            })
            .count()
    });
    let waiting = match (instances, prepared) {
        (Some(instances), Some(prepared)) => instances
            .instances
            .values()
            .filter(|i| prepared.get(i.asset_id).is_none())
            .count(),
        _ => 0,
    };
    let streak = if compiling + waiting == 0 {
        out.0
            .load(std::sync::atomic::Ordering::Relaxed)
            .saturating_add(1)
    } else {
        0
    };
    out.0.store(streak, std::sync::atomic::Ordering::Relaxed);
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
    world: WorldReady,
    models: Res<crate::models::Models>,
    gltfs: Res<Assets<bevy::gltf::Gltf>>,
    server: Res<AssetServer>,
    mut all: Local<bool>,
    mut commands: Commands,
) {
    if *all {
        return;
    }
    if !eyes.is_empty() {
        mark("a 3D view drawing");
    }
    let forest_ok = world.forest();
    if forest_ok {
        mark("forest streamed, impostors captured");
    }
    let props_ok = world.props();
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
    let materials_ok = world.materials();
    if materials_ok {
        mark("every material and pipeline ready (ground and buildings drawn)");
    }
    if forest_ok && props_ok && preload_ok && materials_ok {
        *all = true;
        commands.insert_resource(Loaded);
        mark("everything in");
    }
}
