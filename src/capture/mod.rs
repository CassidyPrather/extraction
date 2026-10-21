//! Capture and showcase tooling: footage and stills that show the game
//! off, and sweeps that look for what looks wrong. None of it runs unless
//! asked for on the command line; see `docs/CAPTURE.md`.
//!
//! - [`cinematic`]: `--cinematic FILE` flies the camera along spline paths
//!   through a valley and saves every frame (`scripts/capture_cinematic.sh`
//!   encodes them).
//! - [`sweep`]: `--sweep FILTER` shoots every model and prefab alone from
//!   eight angles under three lights, for `scripts/capture_sweep.py` to
//!   flag what looks wrong; [`checks`] is the half of it that runs as tests.
//! - [`trailer`]: `--trailer` films a sim bot's raid from a chase camera and
//!   a drone, for `scripts/capture_trailer.py` to cut.

use bevy::prelude::*;
use std::path::PathBuf;

pub mod checks;
pub mod cinematic;
pub mod fighting;
pub mod glb;
pub mod path;
pub mod sweep;
pub mod trailer;

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            cinematic::CinematicPlugin,
            sweep::SweepPlugin,
            trailer::TrailerPlugin,
        ))
        .add_systems(PostUpdate, keep_alive)
        .add_systems(Update, clear_air.after(crate::world::sky::WeatherLook));
    }
}

/// In a still capture the player stands idle at the insertion while the
/// camera looks round; the Surge kills anyone in the open in seconds,
/// which would end the raid (and the Surge's look) partway through a
/// press kit. A trailer's bot takes its chances.
fn keep_alive(
    opts: Res<crate::launch::LaunchOptions>,
    mut players: Query<&mut crate::health::Health, With<crate::rig::LocalPlayer>>,
) {
    if !opts.capture.still() {
        return;
    }
    for mut h in &mut players {
        h.hp = h.max;
        h.bleeding = 0.0;
    }
}

/// A capture camera that sees further than the weather lets a player:
/// its fog's visibility times this. The world's fog is gameplay (robots
/// see by it), so it stays; a drone or an aerial shot 40 m up just looks
/// through less of it.
#[derive(Component, Clone, Copy)]
pub struct ClearAir(pub f32);

fn clear_air(
    weather: Res<crate::world::sky::Weather>,
    mut cams: Query<(&ClearAir, &mut DistanceFog)>,
) {
    for (k, mut fog) in &mut cams {
        fog.falloff = bevy::pbr::FogFalloff::from_visibility_squared(weather.visibility * k.0);
    }
}

/// The capture flags of the command line (part of `LaunchOptions`).
#[derive(Clone, Debug, Default)]
pub struct CaptureOptions {
    /// `--cinematic FILE`: a `.cine` file of shots to fly and record.
    pub cinematic: Option<PathBuf>,
    /// `--cine-only TEXT`: only the shots whose name contains TEXT.
    pub only: Option<String>,
    /// `--cine-preview N`: N stills spread over each shot instead of every
    /// frame (for framing a shot quickly).
    pub preview: Option<u32>,
    /// `--capture-out DIR`: where frames go (default `target/capture`).
    pub out: Option<PathBuf>,
    /// `--fps N` (default 24).
    pub fps: Option<u32>,
    /// `--sweep FILTER`: the looks-wrong sweep over the subjects FILTER keeps.
    pub sweep: Option<String>,
    /// `--sweep-lights day,dusk,night`: which lights to shoot under.
    pub sweep_lights: Option<String>,
    /// `--trailer SECONDS`: film the `--sim-run` raid, up to SECONDS of
    /// footage.
    pub trailer: Option<String>,
}

impl CaptureOptions {
    /// Whether `flag` is one of ours (each takes a value).
    pub fn takes(flag: &str) -> bool {
        matches!(
            flag,
            "--cinematic"
                | "--cine-only"
                | "--cine-preview"
                | "--capture-out"
                | "--fps"
                | "--sweep"
                | "--sweep-lights"
                | "--trailer"
        )
    }

    pub fn set(&mut self, flag: &str, val: String) {
        match flag {
            "--cinematic" => self.cinematic = Some(val.into()),
            "--cine-only" => self.only = Some(val),
            "--cine-preview" => self.preview = val.parse().ok().filter(|n| *n > 0),
            "--capture-out" => self.out = Some(val.into()),
            "--fps" => self.fps = val.parse().ok().filter(|n| *n > 0),
            "--sweep" => self.sweep = Some(val),
            "--sweep-lights" => self.sweep_lights = Some(val),
            "--trailer" => self.trailer = Some(val),
            _ => {}
        }
    }

    /// A capture run: rendered, flat, driven by tooling.
    pub fn active(&self) -> bool {
        self.cinematic.is_some() || self.sweep.is_some() || self.trailer.is_some()
    }

    /// A capture of a still valley (no robots, nobody playing).
    pub fn still(&self) -> bool {
        self.cinematic.is_some() || self.sweep.is_some()
    }

    pub fn out_dir(&self) -> PathBuf {
        self.out
            .clone()
            .unwrap_or_else(|| PathBuf::from("target/capture"))
    }

    pub fn fps(&self) -> u32 {
        self.fps.unwrap_or(24)
    }
}
