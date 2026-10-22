//! Comfort settings for headset play, from `<data dir>/settings.ron` with
//! command-line overrides:
//!
//! ```ron
//! (smooth_turn: false, turn_speed: 120.0, snap_angle: 30.0, vignette: 0.6,
//!  narrow_dynamics: false, render_scale: 0.0, msaa: 4)
//! ```
//!
//! `--smooth-turn`, `--snap-turn`, `--vignette 0..1`, `--narrow-dynamics`,
//! `--finger-grab`, `--climb-teleport`, `--no-binaural`, `--no-score`, `--no-machine-tick`,
//! `--no-post-blur`, `--no-body`, `--left-handed`, `--seated`, `--one-handed`, `--no-captions`,
//! `--render-scale 0.3..1` (0 or absent: auto), `--msaa 4|2|0`,
//! `--a2c 1|0` (soft or hard leaves), `--hold-rate 1|0`.

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
    /// The eyes' resolution as a share of what the runtime recommends;
    /// 0 is auto (about 1800 px across an eye). Next launch.
    pub render_scale: f32,
    /// With tracked fingers (Index controllers), closing them round a thing
    /// takes it, as the squeeze does. Off by default: an unmeant grab is
    /// worse than a missed one.
    pub finger_grab: bool,
    /// Climbing (ladders, ledges): a grip on the ladder or both palms on
    /// the ledge puts you at the top, for those that pulling yourself up
    /// makes sick.
    pub climb_teleport: bool,
    /// In the headset, placed sounds through a head-related filter, so
    /// above and behind can be told apart (`audio`); off, they're panned.
    pub binaural: bool,
    /// The ambient score that rises with danger (`audio`).
    pub score: bool,
    /// The gun hand is the left: the holster and the chest mount move to
    /// the other side, and the wrist display to the right wrist.
    pub left_handed: bool,
    /// Played sitting down: the view is lifted to a standing eye height,
    /// and the crouch button brings it down to the floor.
    pub seated: bool,
    /// Played with one hand: a magazine seats from a pouch when the gun's
    /// well is brought to it, and the second button racks.
    pub one_handed: bool,
    /// What's said (the driver, the radio, the machines) shown on the
    /// wrist with who's speaking and from where.
    pub captions: bool,
    /// A faint tick on the wrist when a machine nearby turns certain of
    /// you (`rig::haptics`).
    pub machine_tick: bool,
    /// Iron sights: the front post goes soft as the eye comes down behind
    /// them, focused on the target (`weapons::optics`).
    pub post_blur: bool,
    /// Your body drawn under you, looking down (`render::body`); off, only
    /// the arms and hands.
    pub body: bool,
    /// Samples a pixel on the edges of things (MSAA): 4, 2, or 0 (off).
    /// Each eye carries a target this many times its size; fewer is
    /// cheaper, with harder edges on wires and blades
    /// (`docs/PERFORMANCE.md`, "The headset's two eyes").
    pub msaa: u32,
    /// Leaves' cut edges smoothed by the samples (alpha to coverage);
    /// off, cut hard (a test), which crawls as the head turns. Next
    /// launch.
    pub soft_leaves: bool,
    /// The eyes render less when the GPU runs late and more when it has
    /// room, between the legibility floor and the runtime's full size
    /// (`render::hold_rate`); off, they keep the SHARPNESS size.
    pub hold_rate: bool,
}

impl Default for Comfort {
    fn default() -> Self {
        Self {
            smooth_turn: false,
            turn_speed: 120.0,
            snap_angle: 30.0,
            vignette: 0.6,
            narrow_dynamics: false,
            render_scale: 0.0,
            finger_grab: false,
            climb_teleport: false,
            binaural: true,
            score: true,
            left_handed: false,
            seated: false,
            one_handed: false,
            captions: true,
            machine_tick: true,
            post_blur: true,
            body: true,
            msaa: 4,
            soft_leaves: true,
            hold_rate: true,
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

    /// Pixels across an eye that auto aims for: enough for a sharp view on
    /// a laptop GPU (the playtest's Bigscreen Beyond 2 asked for 2980 and
    /// ran at 20-30 fps).
    pub const AUTO_EYE_PX: f32 = 1800.0;
    /// The SHARPNESS card's steps (0 is auto). None below 60%: at 50% a
    /// Beyond 2's text loses a quarter of its pixels and drops under the
    /// legibility floor (`docs/LEGIBILITY.md`).
    pub const RENDER_SCALES: [f32; 5] = [0.0, 0.6, 0.7, 0.85, 1.0];

    /// The share of the runtime's `recommended` width to render.
    pub fn render_scale_for(&self, recommended: u32) -> f32 {
        if self.render_scale > 0.0 {
            self.render_scale.clamp(0.3, 1.0)
        } else {
            (Self::AUTO_EYE_PX / recommended.max(1) as f32).min(1.0)
        }
    }

    pub fn next_render_scale(&mut self) {
        let now = Self::RENDER_SCALES
            .iter()
            .position(|v| (v - self.render_scale).abs() < 0.01);
        self.render_scale =
            Self::RENDER_SCALES[now.map_or(0, |i| (i + 1) % Self::RENDER_SCALES.len())];
    }

    /// The SMOOTHING card's steps: samples, and soft leaves (alpha to
    /// coverage on foliage, `trees::foliage_alpha`; with no samples it's
    /// a plain test whatever this says).
    pub const SMOOTHINGS: [(u32, bool); 4] = [(4, true), (4, false), (2, true), (0, true)];

    pub fn next_msaa(&mut self) {
        let now = Self::SMOOTHINGS
            .iter()
            .position(|s| *s == (self.msaa, self.soft_leaves));
        (self.msaa, self.soft_leaves) =
            Self::SMOOTHINGS[now.map_or(0, |i| (i + 1) % Self::SMOOTHINGS.len())];
    }

    pub fn hold_rate_label(&self) -> String {
        if self.hold_rate {
            "HOLD FRAME RATE\non".into()
        } else {
            "HOLD FRAME RATE\noff".into()
        }
    }

    /// The camera setting for `msaa`.
    pub fn msaa_setting(&self) -> Msaa {
        match self.msaa {
            0 | 1 => Msaa::Off,
            2 => Msaa::Sample2,
            8 => Msaa::Sample8,
            _ => Msaa::Sample4,
        }
    }

    pub fn msaa_label(&self) -> String {
        let samples = match self.msaa_setting() {
            Msaa::Off => return "SMOOTHING\noff (fastest)".into(),
            Msaa::Sample2 => "2x",
            Msaa::Sample8 => "8x",
            _ => "4x",
        };
        // (A material's alpha is set as it's made: a change to the leaves
        // shows next launch. Two short lines, as the other cards.)
        let title = if self.soft_leaves != soft_leaves_at_launch() {
            "SMOOTHING (relaunch)"
        } else {
            "SMOOTHING"
        };
        if self.soft_leaves {
            format!("{title}\n{samples}+leaves")
        } else {
            format!("{title}\n{samples} hard leaves")
        }
    }

    pub fn render_scale_label(&self) -> String {
        if self.render_scale <= 0.0 {
            "SHARPNESS\nauto (next launch)".into()
        } else {
            format!("SHARPNESS\n{:.0}% (next launch)", self.render_scale * 100.0)
        }
    }

    pub fn binaural_label(&self) -> String {
        if self.binaural {
            "HEARING\nall around you".into()
        } else {
            "HEARING\nflat STEREO".into()
        }
    }

    pub fn score_label(&self) -> String {
        if self.score {
            "MUSIC\nrises with danger".into()
        } else {
            "MUSIC\nOFF".into()
        }
    }

    pub fn handed_label(&self) -> String {
        if self.left_handed {
            "GUN HAND\nleft".into()
        } else {
            "GUN HAND\nright".into()
        }
    }

    pub fn seated_label(&self) -> String {
        if self.seated {
            "PLAYING\nseated".into()
        } else {
            "PLAYING\nstanding".into()
        }
    }

    pub fn one_handed_label(&self) -> String {
        if self.one_handed {
            "HANDS\none".into()
        } else {
            "HANDS\ntwo".into()
        }
    }

    pub fn captions_label(&self) -> String {
        if self.captions {
            "CAPTIONS\non the wrist".into()
        } else {
            "CAPTIONS\noff".into()
        }
    }

    /// The gun hand.
    pub fn gun_side(&self) -> super::Side {
        if self.left_handed {
            super::Side::Left
        } else {
            super::Side::Right
        }
    }

    pub fn body_label(&self) -> String {
        if self.body {
            "BODY\nshown".into()
        } else {
            "BODY\nhidden".into()
        }
    }

    pub fn machine_tick_label(&self) -> String {
        if self.machine_tick {
            "WARNING TICK\non the wrist".into()
        } else {
            "WARNING TICK\nOFF".into()
        }
    }

    pub fn finger_grab_label(&self) -> String {
        if self.finger_grab {
            "GRAB\nsqueeze or fingers".into()
        } else {
            "GRAB\nsqueeze".into()
        }
    }

    pub fn climb_label(&self) -> String {
        if self.climb_teleport {
            "CLIMBING\nstraight to the top".into()
        } else {
            "CLIMBING\nhand over hand".into()
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
        c.apply_args(args);
        let _ = SOFT_LEAVES.set(c.soft_leaves);
        c
    }

    /// The command line's settings over these.
    pub fn apply_args(&mut self, args: &[String]) {
        let c = self;
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--smooth-turn" => c.smooth_turn = true,
                "--snap-turn" => c.smooth_turn = false,
                "--narrow-dynamics" => c.narrow_dynamics = true,
                "--finger-grab" => c.finger_grab = true,
                "--climb-teleport" => c.climb_teleport = true,
                "--no-binaural" => c.binaural = false,
                "--no-score" => c.score = false,
                "--left-handed" => c.left_handed = true,
                "--seated" => c.seated = true,
                "--one-handed" => c.one_handed = true,
                "--no-captions" => c.captions = false,
                "--no-machine-tick" => c.machine_tick = false,
                "--no-post-blur" => c.post_blur = false,
                "--no-body" => c.body = false,
                "--render-scale" => {
                    if let Some(v) = it.next().and_then(|v| v.parse::<f32>().ok()) {
                        c.render_scale = if v <= 0.0 { 0.0 } else { v.clamp(0.3, 1.0) };
                    }
                }
                "--a2c" => {
                    // `--a2c` alone or `--a2c 1`: soft leaves; `--a2c 0`: hard.
                    c.soft_leaves = match it.clone().next().map(String::as_str) {
                        Some("0") => {
                            it.next();
                            false
                        }
                        Some("1") => {
                            it.next();
                            true
                        }
                        _ => true,
                    };
                }
                "--msaa" => {
                    if let Some(v) = it.next().and_then(|v| v.parse::<u32>().ok()) {
                        c.msaa = v;
                    }
                }
                "--hold-rate" => {
                    // `--hold-rate 0`: keep the SHARPNESS size.
                    c.hold_rate = match it.clone().next().map(String::as_str) {
                        Some("0") => {
                            it.next();
                            false
                        }
                        Some("1") => {
                            it.next();
                            true
                        }
                        _ => true,
                    };
                }
                "--vignette" => {
                    if let Some(v) = it.next().and_then(|v| v.parse::<f32>().ok()) {
                        c.vignette = v.clamp(0.0, 1.0);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Soft leaves as the game started (materials are made once).
static SOFT_LEAVES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Whether foliage is alpha to coverage this run (`Comfort::soft_leaves`,
/// `--a2c`); the default if the settings weren't loaded (tests, sims).
pub fn soft_leaves_at_launch() -> bool {
    SOFT_LEAVES
        .get()
        .copied()
        .unwrap_or_else(|| Comfort::default().soft_leaves)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SMOOTHING steps 4x, 4x with soft leaves, 2x, off and round;
    /// `--msaa` sets it.
    #[test]
    fn smoothing_steps_and_takes_a_flag() {
        let mut c = Comfort::default();
        assert_eq!(c.msaa_setting(), Msaa::Sample4);
        assert!(c.soft_leaves);
        assert_eq!(c.msaa_label(), "SMOOTHING\n4x+leaves");
        c.next_msaa();
        assert_eq!((c.msaa_setting(), c.soft_leaves), (Msaa::Sample4, false));
        assert_eq!(c.msaa_label(), "SMOOTHING (relaunch)\n4x hard leaves");
        c.next_msaa();
        assert_eq!((c.msaa_setting(), c.soft_leaves), (Msaa::Sample2, true));
        assert_eq!(c.msaa_label(), "SMOOTHING\n2x+leaves");
        c.next_msaa();
        assert_eq!(c.msaa_setting(), Msaa::Off);
        c.next_msaa();
        assert_eq!((c.msaa_setting(), c.soft_leaves), (Msaa::Sample4, true));
        // (A settings file from before the setting reads as the default.)
        let old: Comfort = ron::from_str("(smooth_turn: true)").unwrap();
        assert_eq!((old.msaa, old.soft_leaves), (4, true));
        let args = |a: &[&str]| -> Vec<String> { a.iter().map(|s| s.to_string()).collect() };
        let mut c = Comfort::default();
        c.apply_args(&args(&["--msaa", "2", "--a2c", "0", "--seated"]));
        assert_eq!((c.msaa, c.soft_leaves, c.seated), (2, false, true));
        c.apply_args(&args(&["--a2c", "--one-handed"]));
        assert!(c.soft_leaves && c.one_handed);
    }

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
        // Auto: a 2980-px eye renders at about 60%, a 1800-px one in full.
        assert!((c.render_scale_for(2980) - 0.604).abs() < 0.01);
        assert_eq!(c.render_scale_for(1600), 1.0);
        c.next_render_scale();
        assert_eq!(c.render_scale_for(2980), 0.6);
    }
}
