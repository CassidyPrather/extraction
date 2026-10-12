//! Comfort settings for headset play, from `<data dir>/settings.ron` with
//! command-line overrides:
//!
//! ```ron
//! (smooth_turn: false, turn_speed: 120.0, snap_angle: 30.0, vignette: 0.6)
//! ```
//!
//! `--smooth-turn`, `--snap-turn`, `--vignette 0..1`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Comfort {
    /// Turn continuously with the right stick instead of in snaps.
    pub smooth_turn: bool,
    /// Degrees per second, smooth turning.
    pub turn_speed: f32,
    /// Degrees per snap.
    pub snap_angle: f32,
    /// How dark the edges of the view go while the stick moves you (0 off).
    pub vignette: f32,
}

impl Default for Comfort {
    fn default() -> Self {
        Self {
            smooth_turn: false,
            turn_speed: 120.0,
            snap_angle: 30.0,
            vignette: 0.6,
        }
    }
}

impl Comfort {
    pub fn load(args: &[String]) -> Self {
        let path = crate::persist::data_dir().join("settings.ron");
        let mut c: Comfort = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| {
                ron::from_str(&t)
                    .map_err(|e| warn!("{}: {e}", path.display()))
                    .ok()
            })
            .unwrap_or_default();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--smooth-turn" => c.smooth_turn = true,
                "--snap-turn" => c.smooth_turn = false,
                "--vignette" => {
                    if let Some(v) = it.next().and_then(|v| v.parse::<f32>().ok()) {
                        c.vignette = v.clamp(0.0, 1.0);
                    }
                }
                _ => {}
            }
        }
        c
    }
}
