//! The extraction game: Bevy plugins on top of [`extraction_core`].
//!
//! The split that matters: [`GamePlugin`] is everything that decides what
//! happens and runs headless; [`ClientPlugin`] is everything you see, hear
//! and touch. See `docs/DESIGN.md`.

pub mod bot;
pub mod dev;
pub mod health;
pub mod hud;
pub mod interact;
pub mod items;
pub mod launch;
pub mod player;
pub mod raid;
pub mod render;
pub mod rig;
pub mod robots;
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
        ));
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
        ))
        .add_systems(Update, world::structures::blockout_visuals);
    }
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
    let defaults = DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(window),
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

    app.insert_resource(opts)
        .insert_resource(Time::<Fixed>::from_hz(90.0))
        .add_plugins(PhysicsPlugins::default())
        .add_plugins((GamePlugin, ClientPlugin));
    app.run()
}

/// The game with no window and no renderer, stepping one fixed tick per
/// update as fast as it can: for scenarios and map simulations.
pub fn run_headless(opts: LaunchOptions) -> AppExit {
    headless_app(opts, true).run()
}

/// The headless app, ready to run or to step by hand in tests.
pub fn headless_app(opts: LaunchOptions, log: bool) -> App {
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::time::TimeUpdateStrategy;
    let mut app = App::new();
    if log {
        app.add_plugins(bevy::log::LogPlugin::default());
    }
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO)),
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
