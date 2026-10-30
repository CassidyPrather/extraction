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

pub mod assets;
pub mod checks;
pub mod cinematic;
pub mod fighting;
pub mod glb;
pub mod landmarks;
pub mod path;
pub mod sweep;
pub mod trailer;

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        // Experiment: Bevy's static-transform skip off.
        if std::env::var_os("EXTRACTION_NO_STATIC_TRANSFORMS").is_some() {
            app.insert_resource(bevy::transform::StaticTransformOptimizations::Disabled);
        }
        app.add_plugins((
            cinematic::CinematicPlugin,
            sweep::SweepPlugin,
            trailer::TrailerPlugin,
        ))
        .add_systems(PostUpdate, (keep_alive, hide_hands))
        .add_systems(
            PostUpdate,
            landmarks::check_buildings
                .after(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        )
        .add_systems(Update, clear_air.after(crate::world::sky::WeatherLook))
        .add_systems(
            PostUpdate,
            hold_lens.before(bevy::camera::CameraUpdateSystems),
        );
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

/// `--hide-hands`: the player's gloves and sleeves out of the picture, for
/// reels and shots where the camera is the player's eye but the subject
/// is in front of it (a walker crossing the snow, the gloves filling the
/// bottom of every frame).
fn hide_hands(
    opts: Res<crate::launch::LaunchOptions>,
    mut parts: Query<
        &mut Visibility,
        Or<(
            With<crate::render::props::HandVisual>,
            With<crate::render::arms::Arm>,
        )>,
    >,
) {
    if !opts.capture.hide_hands {
        return;
    }
    for mut v in &mut parts {
        if *v != Visibility::Hidden {
            *v = Visibility::Hidden;
        }
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

/// A capture camera's field of view, vertical, radians, held every frame:
/// in flat mode the optic zoom (`gadgets::visuals`) eases every camera's
/// view back toward the game's 75° unless a scope is at the eye, which
/// quietly undid the sweep's framing and the cinematics' lenses.
#[derive(Component, Clone, Copy)]
pub struct Lens(pub f32);

fn hold_lens(mut cams: Query<(&Lens, &mut Projection)>) {
    for (lens, mut proj) in &mut cams {
        if let Projection::Perspective(p) = &mut *proj {
            if p.fov != lens.0 {
                p.fov = lens.0;
            }
        }
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
    /// `--sweep-turn N`: a turntable instead of the grid, N frames round
    /// each subject from one full-window camera.
    pub sweep_turn: Option<u32>,
    /// `--trailer SECONDS`: film the `--sim-run` raid, up to SECONDS of
    /// footage.
    pub trailer: Option<String>,
    /// `--hide-hands 1`: hide the player's gloves and sleeves.
    pub hide_hands: bool,
    /// `--check-buildings 1`: fail loudly when a building in view draws
    /// nothing (`landmarks`).
    pub check_buildings: bool,
    /// `--cine-dump IN.cine,OUT.stills`: write each shot's first pose as a
    /// still and exit (`cinematic::dump`).
    pub dump: Option<(PathBuf, PathBuf)>,
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
                | "--sweep-turn"
                | "--trailer"
                | "--hide-hands"
                | "--cine-dump"
                | "--check-buildings"
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
            "--sweep-turn" => self.sweep_turn = val.parse().ok().filter(|n| *n > 0),
            "--trailer" => self.trailer = Some(val),
            "--hide-hands" => self.hide_hands = val != "0",
            "--check-buildings" => self.check_buildings = val != "0",
            "--cine-dump" => {
                self.dump = val
                    .split_once(',')
                    .map(|(a, b)| (PathBuf::from(a), PathBuf::from(b)))
            }
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
