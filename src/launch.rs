//! Command-line launch options.
//!
//! ```text
//! extraction [--flat] [--map NAME] [--seed N] [--scene NAME]
//!            [--screenshot PATH [--frames N] [--camera X,Y,Z,YAW,PITCH]]
//!            [--gallery SUBJECT[,YAW[,DIST[,SHIFT]]]] [--bot] [--size WxH]
//!            [--save PATH] [--new-game] [--surge-in SECONDS] [--no-record]
//! extraction --sim N [--map NAME] [--seed N] [--jobs J] [--personality P]
//!            [--sim-minutes M] [--sim-out DIR]
//! ```
//!
//! `--sim N` plays N bot raids headless, each in its own process, and
//! writes telemetry (see `sim` and `docs/SIMULATION.md`).
//!
//! `--screenshot` renders `--frames` frames, saves the window to `PATH` and
//! quits: the tool for checking how things look on a machine with no
//! screen (it runs fine under `xvfb-run` with a software Vulkan driver).

use bevy::prelude::*;
use std::path::PathBuf;

/// Which scene to boot into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SceneChoice {
    /// The Zone, at the insertion point.
    #[default]
    Raid,
    /// A flat test range with targets and every gun on a table.
    Range,
    /// The truck: base of operations.
    Truck,
}

#[derive(Resource, Clone, Debug)]
pub struct LaunchOptions {
    /// Skip OpenXR even if it is compiled in.
    pub flat: bool,
    pub map: String,
    pub seed: u64,
    pub scene: SceneChoice,
    pub screenshot: Option<PathBuf>,
    /// Frames to run before taking the screenshot.
    pub frames: u32,
    /// Camera override: position and yaw/pitch in degrees.
    pub camera: Option<(Vec3, f32, f32)>,
    /// The local player is driven by the bot brain.
    pub bot: bool,
    /// Run a scripted bot scenario (see `bot::scenarios`); exits when done.
    pub scenario: Option<String>,
    /// No window, no rendering; time runs as fast as the CPU allows.
    pub headless: bool,
    /// Let the director send robots in (off for scenarios unless they ask).
    pub robots: bool,
    pub window_size: (u32, u32),
    /// Save file override (by default: the data dir, and only for a normal
    /// game; see `persist`).
    pub save: Option<PathBuf>,
    /// Ignore any existing save and start over.
    pub new_game: bool,
    /// Don't write raid recordings (`sim::playtest`).
    pub no_record: bool,
    /// Dev: start with the Surge this many seconds away (negative: raging).
    pub surge_in: Option<f32>,
    /// Dev: put a gun, magazine or robot in front of the camera to look at
    /// (see `dev::gallery`).
    pub gallery: Option<String>,
    /// Dev: screenshot every placement on the map into this folder and
    /// quit (see `tour`).
    pub tour: Option<std::path::PathBuf>,
    /// The map was named on the command line (otherwise a saved career
    /// picks it: wherever the truck was headed).
    pub map_explicit: bool,
    /// Dev: force the weather (`drizzle`, `fog`, `downpour`, `clearing`).
    pub weather: Option<String>,
    /// Map simulation: play this many bot raids (each in a subprocess).
    pub sim: Option<u32>,
    /// Map simulation: this process plays one bot raid under this run name.
    pub sim_run: Option<String>,
    /// Bot personality for a sim raid (`looter`, `hunter`, `cautious`,
    /// `reckless`); by default it rotates with the run index.
    pub personality: Option<String>,
    /// Raid minutes a sim bot plans for before heading out (default: by
    /// personality, 9-14).
    pub sim_minutes: Option<f32>,
    /// Where sim telemetry goes (default `target/sim/<map>`).
    pub sim_out: Option<PathBuf>,
    /// Sim raids run at once.
    pub jobs: usize,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            flat: false,
            map: "marrow_creek".into(),
            seed: 1,
            scene: SceneChoice::Raid,
            screenshot: None,
            frames: 90,
            camera: None,
            tour: None,
            bot: false,
            scenario: None,
            headless: false,
            robots: true,
            window_size: (1600, 900),
            save: None,
            new_game: false,
            no_record: false,
            surge_in: None,
            gallery: None,
            weather: None,
            map_explicit: false,
            sim: None,
            sim_run: None,
            personality: None,
            sim_minutes: None,
            sim_out: None,
            jobs: 2,
        }
    }
}

impl LaunchOptions {
    pub fn from_args() -> Self {
        Self::parse(std::env::args().skip(1))
    }

    pub fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut o = Self::default();
        let mut args = args.into_iter();
        while let Some(a) = args.next() {
            let mut val = || args.next().unwrap_or_default();
            match a.as_str() {
                "--flat" => o.flat = true,
                "--bot" => o.bot = true,
                "--headless" => o.headless = true,
                "--no-robots" => o.robots = false,
                "--new-game" => o.new_game = true,
                "--no-record" => o.no_record = true,
                // Comfort settings; read by `rig::comfort`.
                "--smooth-turn" | "--snap-turn" => {}
                "--vignette" => {
                    let _ = val();
                }
                "--surge-in" => o.surge_in = val().parse().ok(),
                "--weather" => o.weather = Some(val()),
                "--save" => o.save = Some(PathBuf::from(val())),
                "--scenario" => {
                    o.scenario = Some(val());
                    o.robots = false;
                }
                "--sim" => o.sim = val().parse().ok(),
                "--sim-run" => {
                    o.sim_run = Some(val());
                    o.headless = true;
                }
                "--personality" => o.personality = Some(val()),
                "--sim-minutes" => o.sim_minutes = val().parse().ok(),
                "--sim-out" => o.sim_out = Some(PathBuf::from(val())),
                "--jobs" => o.jobs = val().parse().unwrap_or(2).max(1),
                "--map" => {
                    o.map = val();
                    o.map_explicit = true;
                }
                "--seed" => o.seed = val().parse().unwrap_or(1),
                "--scene" => {
                    o.scene = match val().as_str() {
                        "range" => SceneChoice::Range,
                        "truck" => SceneChoice::Truck,
                        _ => SceneChoice::Raid,
                    }
                }
                "--screenshot" => {
                    o.screenshot = Some(PathBuf::from(val()));
                    o.flat = true;
                }
                "--frames" => o.frames = val().parse().unwrap_or(90),
                "--tour" => {
                    o.tour = Some(val().into());
                    o.robots = false;
                    o.flat = true;
                    // The tour drives a free camera.
                    o.camera
                        .get_or_insert((Vec3::new(0.0, 60.0, 0.0), 0.0, -30.0));
                }
                "--gallery" => {
                    o.gallery = Some(val());
                    o.robots = false;
                }
                "--camera" => {
                    let v: Vec<f32> = val().split(',').filter_map(|s| s.parse().ok()).collect();
                    if v.len() >= 3 {
                        o.camera = Some((
                            Vec3::new(v[0], v[1], v[2]),
                            v.get(3).copied().unwrap_or(0.0),
                            v.get(4).copied().unwrap_or(0.0),
                        ));
                    }
                }
                "--size" => {
                    let v: Vec<u32> = val().split('x').filter_map(|s| s.parse().ok()).collect();
                    if v.len() == 2 {
                        o.window_size = (v[0], v[1]);
                    }
                }
                other => eprintln!("ignoring unknown argument {other}"),
            }
        }
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_screenshot_and_camera() {
        let o = LaunchOptions::parse(
            "--screenshot out.png --frames 30 --camera 1,2,3,90,-10 --seed 9"
                .split(' ')
                .map(String::from),
        );
        assert!(o.flat, "screenshots force flat mode");
        assert_eq!(o.frames, 30);
        assert_eq!(o.seed, 9);
        assert_eq!(o.camera, Some((Vec3::new(1.0, 2.0, 3.0), 90.0, -10.0)));
    }
}
