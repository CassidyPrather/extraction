//! Comfort settings for headset play, from `<data dir>/settings.ron` with
//! command-line overrides:
//!
//! ```ron
//! (smooth_turn: false, turn_speed: 120.0, snap_angle: 30.0, vignette: 0.6,
//!  narrow_dynamics: false)
//! ```
//!
//! `--smooth-turn`, `--snap-turn`, `--vignette 0..1`, `--narrow-dynamics`.

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
    /// Quiet sounds up and loud ones down (`audio_mix::NARROW`).
    pub narrow_dynamics: bool,
}

impl Default for Comfort {
    fn default() -> Self {
        Self {
            smooth_turn: false,
            turn_speed: 120.0,
            snap_angle: 30.0,
            vignette: 0.6,
            narrow_dynamics: false,
        }
    }
}

impl Comfort {
    /// Keep the settings for next time.
    pub fn save(&self) {
        let path = crate::persist::data_dir().join("settings.ron");
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new()) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    warn!("{}: {e}", path.display());
                }
            }
            Err(e) => warn!("settings: {e}"),
        }
    }

    /// The turning presets the comfort board steps through.
    pub const TURNING: [(bool, f32); 5] = [
        (false, 30.0),
        (false, 45.0),
        (true, 90.0),
        (true, 120.0),
        (true, 180.0),
    ];
    pub const VIGNETTES: [f32; 4] = [0.0, 0.3, 0.6, 0.9];

    pub fn turning_label(&self) -> String {
        if self.smooth_turn {
            format!("TURNING\nsmooth, {:.0}°/s", self.turn_speed)
        } else {
            format!("TURNING\nsnap, {:.0}°", self.snap_angle)
        }
    }

    pub fn vignette_label(&self) -> String {
        if self.vignette <= 0.01 {
            "VIGNETTE\noff".into()
        } else {
            format!("VIGNETTE\n{:.0}%", self.vignette * 100.0)
        }
    }

    pub fn sound_label(&self) -> String {
        if self.narrow_dynamics {
            "SOUND\nnarrow range".into()
        } else {
            "SOUND\nfull range".into()
        }
    }

    /// Step to the next turning preset.
    pub fn next_turning(&mut self) {
        let now = Self::TURNING.iter().position(|(smooth, v)| {
            *smooth == self.smooth_turn
                && (if *smooth {
                    self.turn_speed
                } else {
                    self.snap_angle
                } - v)
                    .abs()
                    < 0.5
        });
        let (smooth, v) = Self::TURNING[now.map_or(0, |i| (i + 1) % Self::TURNING.len())];
        self.smooth_turn = smooth;
        if smooth {
            self.turn_speed = v;
        } else {
            self.snap_angle = v;
        }
    }

    pub fn next_vignette(&mut self) {
        let now = Self::VIGNETTES
            .iter()
            .position(|v| (v - self.vignette).abs() < 0.05);
        self.vignette = Self::VIGNETTES[now.map_or(0, |i| (i + 1) % Self::VIGNETTES.len())];
    }

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
                "--narrow-dynamics" => c.narrow_dynamics = true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_board_steps_through_the_presets_and_round() {
        let mut c = Comfort::default();
        assert_eq!(c.turning_label(), "TURNING\nsnap, 30°");
        for _ in 0..2 {
            c.next_turning();
        }
        assert!(c.smooth_turn && (c.turn_speed - 90.0).abs() < 0.1);
        for _ in 0..3 {
            c.next_turning();
        }
        assert!(
            !c.smooth_turn && (c.snap_angle - 30.0).abs() < 0.1,
            "round again"
        );
        c.next_vignette();
        assert!((c.vignette - 0.9).abs() < 1e-3);
        c.next_vignette();
        assert_eq!(c.vignette_label(), "VIGNETTE\noff");
        assert_eq!(c.sound_label(), "SOUND\nfull range");
    }
}
