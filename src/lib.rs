//! The extraction game: Bevy plugins on top of [`extraction_core`].
//!
//! The split that matters: [`GamePlugin`] is everything that decides what
//! happens and runs headless; [`ClientPlugin`] is everything you see, hear
//! and touch. See `docs/DESIGN.md`.

pub mod anomaly;
pub mod audio;
pub mod bot;
pub mod dev;
pub mod gadgets;
pub mod health;
pub mod hud;
pub mod interact;
pub mod items;
pub mod launch;
pub mod models;
mod perf;
pub mod persist;
pub mod player;
pub mod raid;
pub mod render;
pub mod rig;
pub mod robots;
pub mod sim;
pub mod weapons;
pub mod world;

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::PresentMode;

pub use launch::LaunchOptions;

/// Present when the OpenXR plugins were added (a headset may still be
/// absent; the plugins fall back to a window).
#[derive(Resource)]
pub struct XrActive;

/// Gameplay logic. Headless-safe.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            world::WorldPlugin,
            rig::RigPlugin,
            player::PlayerPlugin,
            items::ItemsPlugin,
            interact::InteractPlugin,
            weapons::WeaponsPlugin,
            weapons::WeaponEffectsPlugin,
            weapons::flat::CombinePlugin,
            bot::BotPlugin,
            bot::scenarios::ScenarioPlugin,
            robots::RobotsPlugin,
            health::HealthPlugin,
            raid::RaidPlugin,
            (
                persist::PersistPlugin,
                anomaly::AnomalyPlugin,
                gadgets::GadgetsPlugin,
            ),
        ))
        .add_plugins((sim::SimPlugin, sim::playtest::PlaytestPlugin));
    }
}

/// Everything that needs a screen: visuals, input drivers, dev tools.
pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            render::RenderPlugin,
            world::WorldVisualsPlugin,
            rig::flat::FlatDriverPlugin,
            dev::DevPlugin,
            items::ItemVisualsPlugin,
            weapons::visuals::GunVisualsPlugin,
            interact::flat::FlatInteractPlugin,
            weapons::flat::FlatGunPlugin,
            hud::HudPlugin,
            robots::visuals::RobotVisualsPlugin,
            render::props::PropVisualsPlugin,
            anomaly::visuals::AnomalyVisualsPlugin,
            gadgets::visuals::GadgetVisualsPlugin,
            models::ModelsPlugin,
        ))
        .init_resource::<render::materials::MaterialLibrary>()
        .add_systems(
            Update,
            (
                world::structures::blockout_visuals,
                world::structures::fence_visuals,
                world::buildings::building_visuals,
                log_frame_times,
            ),
        );
    }
}

/// Every ten seconds, how the frames went (in the session log): mean, the
/// slowest 1% and the worst, in real milliseconds. A headset wants every
/// frame under its refresh (11.1 ms at 90 Hz).
fn log_frame_times(
    time: Res<Time<Real>>,
    raid: Option<Res<raid::Raid>>,
    mut frames: Local<Vec<f32>>,
    mut since: Local<f32>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    frames.push(dt * 1000.0);
    *since += dt;
    if *since < 10.0 {
        return;
    }
    *since = 0.0;
    let mut f = std::mem::take(&mut *frames);
    f.sort_by(|a, b| a.total_cmp(b));
    let mean = f.iter().sum::<f32>() / f.len() as f32;
    let p99 = f[((f.len() as f32 * 0.99) as usize).min(f.len() - 1)];
    let worst = f[f.len() - 1];
    let phase = raid.map(|r| format!("{:?}", r.phase)).unwrap_or_default();
    info!(
        "frames: {} in 10 s, mean {mean:.1} ms, 1% {p99:.1} ms, worst {worst:.1} ms ({phase})",
        f.len()
    );
}

/// Builds and runs the client (or, with `--headless`, the bare simulation).
pub fn run(opts: LaunchOptions) -> AppExit {
    if opts.headless {
        return run_headless(opts);
    }
    let mut app = App::new();
    let window = Window {
        title: "Extraction".into(),
        resolution: opts.window_size.into(),
        present_mode: PresentMode::AutoVsync,
        ..default()
    };
    capture_panics();
    let defaults = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(window),
            ..default()
        })
        .set(bevy::log::LogPlugin {
            custom_layer: log_file_layer,
            ..default()
        })
        .set(AssetPlugin {
            file_path: asset_root(),
            ..default()
        });
    #[cfg(feature = "xr")]
    if !opts.flat {
        app.add_plugins(bevy_mod_openxr::add_xr_plugins(defaults));
        app.add_plugins(rig::xr::XrDriverPlugin);
        app.insert_resource(XrActive);
    } else {
        app.add_plugins(defaults);
    }
    #[cfg(not(feature = "xr"))]
    app.add_plugins(defaults);

    // A missing audio device or a flaky asset shouldn't take the game down:
    // system errors become warnings.
    app.set_error_handler(bevy::ecs::error::warn);
    if opts.screenshot.is_none() {
        app.add_plugins(audio::AudioPlugin);
    }
    app.insert_resource(opts)
        .insert_resource(Time::<Fixed>::from_hz(90.0))
        .add_plugins(PhysicsPlugins::default())
        .add_plugins((GamePlugin, ClientPlugin));
    app.run()
}

/// Where this session's log goes: `<data dir>/logs/`, one file a session,
/// the last ten kept (send them with a bug report).
fn log_dir() -> std::path::PathBuf {
    persist::data_dir().join("logs")
}

fn session_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Everything logged also goes to a file (no colours, with timestamps).
fn log_file_layer(_app: &mut App) -> Option<bevy::log::BoxedLayer> {
    use bevy::log::tracing_subscriber::Layer;
    let dir = log_dir();
    std::fs::create_dir_all(&dir).ok()?;
    // Keep the newest few.
    let mut old: Vec<_> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "log"))
        .collect();
    old.sort();
    while old.len() >= 10 {
        let _ = std::fs::remove_file(old.remove(0));
    }
    let file = std::fs::File::create(dir.join(format!("session-{}.log", session_stamp()))).ok()?;
    Some(
        bevy::log::tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file))
            .boxed(),
    )
}

/// A crash writes what happened, and where, next to the logs before the
/// usual report.
fn capture_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let dir = log_dir();
        let _ = std::fs::create_dir_all(&dir);
        let text = format!("{info}\n\n{}", std::backtrace::Backtrace::force_capture());
        let _ = std::fs::write(dir.join(format!("crash-{}.txt", session_stamp())), text);
        default(info);
    }));
}

/// The game with no window and no renderer, stepping one fixed tick per
/// update as fast as it can: for scenarios and map simulations.
pub fn run_headless(opts: LaunchOptions) -> AppExit {
    headless_app(opts, true).run()
}

/// A valley to open the game in again once this session ends (the truck
/// drove there). `main` re-launches into it.
static RESTART_INTO: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn request_restart(map: &str) {
    *RESTART_INTO.lock().unwrap() = Some(map.to_owned());
}

pub fn take_restart() -> Option<String> {
    RESTART_INTO.lock().unwrap().take()
}

/// The arguments to open the game again in `map`: this session's, less any
/// `--map` (and `--new-game`, which would throw the career away).
pub fn relaunch_args(args: impl IntoIterator<Item = String>, map: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--map" => {
                it.next();
            }
            "--new-game" => {}
            _ => out.push(a),
        }
    }
    out.extend(["--map".to_owned(), map.to_owned()]);
    out
}

/// The valleys a map test walks: `EXTRACTION_TEST_MAP` if set, else every
/// map in `assets/maps`.
#[cfg(test)]
pub(crate) fn test_maps() -> Vec<String> {
    if let Ok(map) = std::env::var("EXTRACTION_TEST_MAP") {
        return vec![map];
    }
    let mut maps: Vec<String> = std::fs::read_dir(asset_root_path().join("maps"))
        .expect("assets/maps")
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "ron").then(|| p.file_stem()?.to_str().map(str::to_owned))?
        })
        .collect();
    maps.sort();
    maps
}

#[cfg(test)]
mod relaunch_tests {
    #[test]
    fn relaunching_keeps_the_options_and_swaps_the_valley() {
        let args = [
            "--flat",
            "--map",
            "marrow_creek",
            "--new-game",
            "--vignette",
            "0.3",
        ]
        .map(String::from);
        assert_eq!(
            super::relaunch_args(args, "kettle_pass"),
            ["--flat", "--vignette", "0.3", "--map", "kettle_pass"].map(String::from)
        );
    }
}

/// The headless app, ready to run or to step by hand in tests.
pub fn headless_app(mut opts: LaunchOptions, log: bool) -> App {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::time::TimeUpdateStrategy;
    // Headless runs (sims, tests) never touch the player's save unless
    // given one explicitly.
    opts.headless = true;
    let mut app = App::new();
    if log {
        app.add_plugins(bevy::log::LogPlugin::default());
    }
    let threads = std::env::var("EXTRACTION_THREADS")
        .ok()
        .and_then(|n| n.parse().ok());
    let pools = match threads {
        Some(n) => bevy::app::TaskPoolPlugin {
            task_pool_options: bevy::app::TaskPoolOptions::with_num_threads(n),
        },
        None => bevy::app::TaskPoolPlugin::default(),
    };
    app.add_plugins((
        MinimalPlugins
            .set(ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO))
            .set(pools),
        TransformPlugin,
        bevy::asset::AssetPlugin {
            file_path: asset_root(),
            ..default()
        },
        bevy::mesh::MeshPlugin,
        bevy::scene::ScenePlugin,
    ))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    .insert_resource(opts)
    .insert_resource(Time::<Fixed>::from_hz(90.0))
    .add_plugins(PhysicsPlugins::default())
    .add_plugins(GamePlugin);
    app
}

/// The asset root as a path, for checking whether an optional file exists.
pub(crate) fn asset_root_path() -> std::path::PathBuf {
    std::path::PathBuf::from(asset_root())
}

/// `assets/` next to the crate when run from a checkout, or next to the
/// executable in a packaged build.
fn asset_root() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let packaged = dir.join("assets");
            if packaged.join("maps").exists() {
                return packaged.to_string_lossy().into_owned();
            }
        }
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod client_tests {
    use super::*;

    /// Builds the whole client (every visual, input and dev system) with
    /// no window and no GPU, and runs a few frames: system parameter
    /// conflicts and missing resources only show up when systems first
    /// run, and the headless tests never run the client half.
    #[test]
    fn client_systems_initialise() {
        use bevy::render::RenderPlugin;
        use bevy::render::settings::WgpuSettings;
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: None,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root(),
                    ..default()
                })
                .disable::<bevy::winit::WinitPlugin>(),
        );
        app.insert_resource(LaunchOptions {
            flat: true,
            ..default()
        })
        .insert_resource(Time::<Fixed>::from_hz(90.0))
        .add_plugins(PhysicsPlugins::default())
        .add_plugins((GamePlugin, ClientPlugin));
        app.finish();
        app.cleanup();
        for _ in 0..20 {
            app.update();
        }
        // A rough census of what the renderer has to deal with.
        let w = app.world_mut();
        let meshes = w.query::<&Mesh3d>().iter(w).count();
        let points = w.query::<&PointLight>().iter(w).count();
        let spots = w.query::<&SpotLight>().iter(w).count();
        let entities = w.entities().len();
        eprintln!(
            "census: {entities} entities, {meshes} meshes, {points} point lights, {spots} spot lights"
        );
        // Meshes by their top-level ancestor's name.
        let mut by: std::collections::HashMap<String, usize> = default();
        let tops: Vec<Entity> = w.query_filtered::<Entity, With<Mesh3d>>().iter(w).collect();
        for e in tops {
            let mut cur = e;
            while let Some(p) = w.get::<ChildOf>(cur) {
                cur = p.parent();
            }
            let name = w
                .get::<Name>(cur)
                .map(|n| n.as_str().split(' ').next().unwrap_or("").to_string())
                .unwrap_or_else(|| "(unnamed)".into());
            *by.entry(name).or_default() += 1;
        }
        let mut v: Vec<_> = by.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        eprintln!("census by root: {:?}", &v[..v.len().min(12)]);
    }
}
