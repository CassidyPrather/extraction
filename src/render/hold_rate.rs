//! Holding the headset's frame rate: the eyes render a smaller or larger
//! part of their swapchain images as the GPU's time a frame comes and goes,
//! and the runtime is told the part (each projection view's image rect),
//! so the compositor scales it to the display. No swapchain is made again.
//!
//! The governor (`Governor`) steps the scale every few frames from the
//! GPU's measured time against the display's frame period: down at once
//! when frames' work runs past 85% of the period (two of the last eight:
//! one long frame isn't the pixels' doing), by as much as the overrun says
//! (time goes with pixels, the square of the scale); up only slowly, and
//! only after half a second whose slow end (90th percentile) leaves room,
//! so the eyes don't pump. The floor is the narrowest eye that keeps must-read text at
//! its legibility floor (`render::legibility`); the cap, the runtime's
//! recommended size (the swapchain is made that big when this is on).
//!
//! "HOLD FRAME RATE" on the truck's comfort board (`Comfort::hold_rate`,
//! default on); off, the eyes keep the SHARPNESS size. The scale is in the
//! frame log's fourth column and the playtest report. See
//! docs/PERFORMANCE.md, "Holding the frame rate".

use bevy::prelude::*;

/// Work a frame may take, as a share of the display's period: the rest is
/// the compositor's, and margin.
const TARGET: f64 = 0.85;
/// Room under the target a step up needs, over the frames it looks back.
const ROOM: f64 = 0.80;
/// Frames between steps.
const EVERY: u32 = 4;
/// Frames looked back at going down (the worst of them), and up.
const HOT_FRAMES: usize = 8;
const COOL_FRAMES: usize = 45;
/// A step up, and the deepest step down.
const UP: f32 = 1.03;
const DOWN_MOST: f32 = 0.80;
/// After a step down, frames before the next step up may come.
const SETTLE: u32 = 45;
/// Frames a scale that ran late stays out of reach (a minute at 90 Hz):
/// whatever made those frames late, the GPU's time didn't show it.
const CEILING_FRAMES: u32 = 5400;

/// The narrowest eye, pixels across, that keeps must-read text at its
/// floor (7.5 px of x-height at `legibility::MUST_READ_DEG`).
/// `EXTRACTION_EYE_FLOOR_PX` sets it instead (to see the scale move on a
/// small simulated headset).
pub fn floor_eye_px() -> f32 {
    use super::legibility::{EYE_TAN_SPAN, MUST_READ_DEG};
    if let Some(px) = std::env::var("EXTRACTION_EYE_FLOOR_PX")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
    {
        return px;
    }
    let px_per_deg = 7.5 / MUST_READ_DEG;
    px_per_deg * EYE_TAN_SPAN * 180.0 / std::f32::consts::PI
}

/// Steps the eyes' scale (a share of the swapchain's size, each axis) from
/// the GPU's time a frame.
#[derive(Clone, Debug)]
pub struct Governor {
    pub scale: f32,
    pub min: f32,
    pub max: f32,
    /// GPU milliseconds a frame, newest last.
    times: std::collections::VecDeque<f64>,
    /// Whether the runtime was reprojecting, the last `HOT_FRAMES` frames.
    late: std::collections::VecDeque<bool>,
    frame: u32,
    settle: u32,
    /// A scale the runtime reprojected at, and frames it stays a ceiling.
    ceiling: Option<(f32, u32)>,
}

impl Governor {
    pub fn new(start: f32, min: f32, max: f32) -> Self {
        let min = min.min(max);
        Self {
            scale: start.clamp(min, max),
            min,
            max,
            times: Default::default(),
            late: Default::default(),
            frame: 0,
            settle: 0,
            ceiling: None,
        }
    }

    /// One frame: the GPU's time for it (None: none came back), and the
    /// display's period (ms). Gives the scale to render the next at.
    pub fn step(&mut self, gpu_ms: Option<f64>, period_ms: f64) -> f32 {
        self.step_seen(gpu_ms, period_ms, false)
    }

    /// As `step`, and whether the runtime is reprojecting (frames landing
    /// at a multiple of the display's period, `Display::see`): late frames
    /// are over the budget whatever the GPU's measured time says, which
    /// may not be all the frame's work.
    pub fn step_seen(&mut self, gpu_ms: Option<f64>, period_ms: f64, late: bool) -> f32 {
        if let Some(t) = gpu_ms {
            self.times.push_back(t);
            while self.times.len() > COOL_FRAMES {
                self.times.pop_front();
            }
        }
        self.late.push_back(late);
        while self.late.len() > HOT_FRAMES {
            self.late.pop_front();
        }
        self.frame += 1;
        self.settle = self.settle.saturating_sub(1);
        let lates = self.late.iter().filter(|l| **l).count();
        if self.frame % EVERY != 0 || (self.times.is_empty() && lates == 0) {
            return self.scale;
        }
        let target = period_ms * TARGET;
        // Down on the second-worst of the last few: a single long frame (a
        // pipeline built, a hitch on the CPU) isn't the pixels' doing, and
        // fewer pixels wouldn't have saved it.
        let mut recent: Vec<f64> = self.times.iter().rev().take(HOT_FRAMES).copied().collect();
        recent.sort_by(|a, b| b.total_cmp(a));
        let mut hot = recent.get(1).copied().unwrap_or(0.0);
        if lates >= 2 {
            // Reprojecting: a tenth off each axis a step.
            hot = hot.max(target / 0.81);
            self.ceiling = Some((self.scale, CEILING_FRAMES));
        }
        if let Some((_, left)) = &mut self.ceiling {
            *left = left.saturating_sub(EVERY);
            if *left == 0 {
                self.ceiling = None;
            }
        }
        if hot > target {
            // Time goes with the pixels: the square of the scale.
            let k = ((target / hot).sqrt() as f32).clamp(DOWN_MOST, 0.98);
            self.scale = (self.scale * k).max(self.min);
            self.settle = SETTLE;
            // What was measured was at the old size.
            self.times.clear();
            self.late.clear();
        } else if self.settle == 0 && lates == 0 && self.times.len() >= COOL_FRAMES {
            // Up on the slow end of the last half second (its 90th
            // percentile): room for that at the larger size.
            let mut all: Vec<f64> = self.times.iter().copied().collect();
            all.sort_by(f64::total_cmp);
            let slow = all[all.len() * 9 / 10];
            let after = slow * f64::from(UP * UP);
            let most = self.ceiling.map_or(self.max, |(c, _)| (c / UP).min(self.max));
            if after < target * ROOM && self.scale < most {
                self.scale = (self.scale * UP).min(most);
                // The next step up waits for a half second all at this
                // size. What was measured stays for a step down: a late
                // frame just before this mustn't be forgotten.
                self.settle = COOL_FRAMES as u32;
            }
        }
        self.scale
    }
}

/// The display's own frame period, from the runtime's predicted one. A
/// runtime that's reprojecting (SteamVR, frames late) predicts a multiple
/// of it: the playtest's Beyond 2 at 75 Hz said 26.7 ms, and the eyes,
/// judged against that, sat at 100% with every frame at half rate. So the
/// period is the shortest predicted (no PC headset refreshes under 60 Hz),
/// and a longer one says the frames are late.
#[derive(Clone, Copy, Debug)]
pub struct Display {
    pub period_ms: f64,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            period_ms: 1000.0 / 60.0,
        }
    }
}

impl Display {
    /// This frame's predicted period: the display's, and whether it's late.
    pub fn see(&mut self, predicted_ms: f64) -> (f64, bool) {
        if predicted_ms > 0.0 {
            self.period_ms = self.period_ms.min(predicted_ms);
        }
        (self.period_ms, predicted_ms > self.period_ms * 1.5)
    }
}

/// The eyes' size this frame, pixels, for the cameras' viewports and the
/// projection layer's image rects (extracted, so both are the same
/// frame's). `scale` is the share of the runtime's recommended width.
#[derive(
    Resource, Clone, Copy, Debug, Default, bevy::render::extract_resource::ExtractResource,
)]
pub struct EyeRect {
    pub size: UVec2,
    pub scale: f32,
}

/// The governor and what it governs, once the headset's session is made.
#[derive(Resource)]
pub struct HoldRate {
    pub governor: Governor,
    /// The swapchain's size (the rect's most).
    pub full: UVec2,
    /// The runtime's recommended width (for the logged scale).
    pub recommended: u32,
    pub display: Display,
}

pub struct HoldRatePlugin;

impl Plugin for HoldRatePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::render::extract_resource::ExtractResourcePlugin::<
            EyeRect,
        >::default());
        #[cfg(feature = "xr")]
        app.add_systems(PostUpdate, xr::govern);
    }

    fn finish(&self, _app: &mut App) {
        #[cfg(feature = "xr")]
        if let Some(render) = _app.get_sub_app_mut(bevy::render::RenderApp) {
            if let Some(mut layers) = render
                .world_mut()
                .get_resource_mut::<bevy_mod_openxr::resources::OxrRenderLayers>()
            {
                for l in layers.0.iter_mut() {
                    *l = Box::new(xr::RectLayer);
                }
            }
        }
    }
}

#[cfg(feature = "xr")]
mod xr {
    use super::*;
    use bevy_mod_openxr::layer_builder::{
        CompositionLayer, CompositionLayerProjection, CompositionLayerProjectionView,
        LayerProvider, SwapchainSubImage,
    };
    use bevy_mod_openxr::resources::{
        OxrCurrentSessionConfig, OxrFrameState, OxrSwapchain, OxrViews,
    };
    use bevy_mod_xr::camera::XrCamera;
    use bevy_mod_xr::spaces::XrPrimaryReferenceSpace;

    /// Each frame: the GPU's time, the governor's step, the eyes' viewports.
    #[allow(clippy::too_many_arguments)]
    pub fn govern(
        mut commands: Commands,
        config: Option<Res<OxrCurrentSessionConfig>>,
        frame: Option<Res<OxrFrameState>>,
        sizes: Option<Res<crate::rig::xr::EyeSizes>>,
        comfort: Res<crate::rig::comfort::Comfort>,
        store: Option<Res<bevy::diagnostic::DiagnosticsStore>>,
        hold: Option<ResMut<HoldRate>>,
        time: Res<Time<Real>>,
        mut seen: Local<Option<std::time::Instant>>,
        mut said: Local<f32>,
        mut eyes: Query<&mut Camera, With<XrCamera>>,
    ) {
        let (Some(config), Some(sizes)) = (config, sizes) else {
            return;
        };
        let Some(mut hold) = hold else {
            let full = config.resolution;
            let w = full.x.max(1) as f32;
            commands.insert_resource(HoldRate {
                governor: Governor::new(
                    sizes.start.x as f32 / w,
                    (floor_eye_px() / w).min(1.0),
                    1.0,
                ),
                full,
                recommended: sizes.recommended.x,
                display: Display::default(),
            });
            return;
        };
        let gpu = store
            .as_deref()
            .and_then(|s| super::super::budget::newest_gpu_ms(s, &mut seen));
        // `EXTRACTION_FRAME_BUDGET_MS` stands in for the display's period (a
        // simulated runtime's follows the game's own pace).
        let predicted = frame.map(|f| f.predicted_display_period.as_nanos() as f64 / 1e6);
        let (period, late) = match std::env::var("EXTRACTION_FRAME_BUDGET_MS")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
        {
            Some(p) => (p, false),
            None => predicted.map_or((1000.0 / 90.0, false), |p| hold.display.see(p)),
        };
        let scale = if comfort.hold_rate {
            hold.governor.step_seen(gpu, period.max(1.0), late)
        } else {
            // Off: the SHARPNESS size, as launched.
            hold.governor.scale = sizes.start.x as f32 / hold.full.x.max(1) as f32;
            hold.governor.scale
        };
        // Even sizes (the compositor samples by halves).
        let size = ((hold.full.as_vec2() * scale / 2.0).round() * 2.0)
            .as_uvec2()
            .clamp(UVec2::splat(2), hold.full);
        let rect = EyeRect {
            size,
            scale: size.x as f32 / hold.recommended.max(1) as f32,
        };
        // Every ten seconds in the session log, with what it went on.
        *said += time.delta_secs();
        if *said >= 10.0 {
            *said = 0.0;
            info!(
                "eyes: {}x{} ({:.0}% of the runtime's width){}, gpu {} of a {period:.1} ms frame{}",
                size.x,
                size.y,
                rect.scale * 100.0,
                if comfort.hold_rate {
                    ", holding the frame rate"
                } else {
                    ""
                },
                gpu.map_or("not measured".into(), |g| format!("{g:.1} ms")),
                predicted
                    .filter(|_| late)
                    .map_or(String::new(), |p| format!(
                        " (late: the runtime says {p:.1})"
                    )),
            );
        }
        commands.insert_resource(rect);
        for mut cam in &mut eyes {
            let want = Some(bevy::camera::Viewport {
                physical_position: UVec2::ZERO,
                physical_size: size,
                ..default()
            });
            if cam.viewport.as_ref().map(|v| v.physical_size) != Some(size) {
                cam.viewport = want;
            }
        }
    }

    /// `bevy_mod_openxr`'s projection layer, but each view's image rect is
    /// the part the eyes rendered this frame (`EyeRect`).
    pub struct RectLayer;

    impl LayerProvider for RectLayer {
        fn get<'a>(&self, world: &'a World) -> Option<Box<dyn CompositionLayer<'a> + 'a>> {
            let stage = world.get_resource::<XrPrimaryReferenceSpace>()?;
            let views = world.get_resource::<OxrViews>()?;
            let swapchain = world.get_resource::<OxrSwapchain>()?;
            let config = world.get_resource::<OxrCurrentSessionConfig>()?;
            let size = world
                .get_resource::<EyeRect>()
                .map(|r| r.size)
                .filter(|s| s.x > 0 && s.y > 0)
                .unwrap_or(config.resolution)
                .min(config.resolution);
            let rect = openxr::Rect2Di {
                offset: openxr::Offset2Di { x: 0, y: 0 },
                extent: openxr::Extent2Di {
                    width: size.x as _,
                    height: size.y as _,
                },
            };
            if views.len() < 2 {
                return None;
            }
            let view = |i: usize| {
                CompositionLayerProjectionView::new()
                    .pose(views.0[i].pose)
                    .fov(views.0[i].fov)
                    .sub_image(
                        SwapchainSubImage::new()
                            .swapchain(swapchain)
                            .image_array_index(i as u32)
                            .image_rect(rect),
                    )
            };
            Some(Box::new(
                CompositionLayerProjection::new()
                    .layer_flags(openxr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA)
                    .space(stage)
                    .views(&[view(0), view(1)]),
            ))
        }
    }
}

/// The hitch gauge's forced events (the response team, the Surge and the
/// rest) through the governor: each frame's measured load, against the
/// busy moment's own median, taken as the GPU's (a fifth of it fixed, the
/// rest the pixels'). Wherever frames run late in a run (two within a
/// step's look back) the scale comes down, and it's back within seconds.
/// That's the governor's steps on the real moments' shapes; what a
/// headset's GPU spends is the frame log's fourth column. Run by
/// `spikes::tests::hitches_hold_their_ceiling` on its own trace (what's
/// wrong, by moment).
#[cfg(test)]
pub(crate) fn follows_busy_moments(
    base: &[f64],
    moments: &[(String, Vec<f64>)],
) -> Vec<(String, String)> {
    const PERIOD: f64 = 1000.0 / 90.0;
    let mut sorted = base.to_vec();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[sorted.len() / 2];
    // At 0.7 the quiet moment takes three quarters of the target.
    let s0 = 0.7f32;
    let t0 = 0.75 * TARGET * PERIOD;
    let (rest, pixels) = (0.2 * t0, 0.8 * t0 / f64::from(s0 * s0));
    let gpu = |load: f64, s: f32| load / median * (rest + pixels * f64::from(s * s));
    let mut g = Governor::new(s0, 0.55, 1.0);
    let mut quiet = base.iter().copied().cycle();
    for _ in 0..1200 {
        let s = g.scale;
        g.step(Some(gpu(quiet.next().unwrap(), s)), PERIOD);
    }
    let mut wrong = Vec::new();
    let mut runs = 0;
    for (name, frames) in moments {
        let before = g.scale;
        let mut least = before;
        let mut late = Vec::new();
        for (i, &f) in frames.iter().enumerate() {
            let s = g.scale;
            let t = gpu(f, s);
            if t > PERIOD * TARGET {
                late.push(i);
            }
            g.step(Some(t), PERIOD);
            least = least.min(g.scale);
        }
        // Two late frames a step apart or less always fall in one step's
        // look back (its last eight frames): a run the governor must see.
        let run = late.windows(2).any(|w| w[1] - w[0] <= EVERY as usize);
        let mut back = None;
        for i in 0..900 {
            let s = g.scale;
            g.step(Some(gpu(quiet.next().unwrap(), s)), PERIOD);
            if back.is_none() && g.scale >= before * 0.97 {
                back = Some(i);
            }
        }
        println!(
            "    governor: {name:<22} {:>3} late{}; scale {before:.2} > {least:.2}, back in {}",
            late.len(),
            if run { " in a run" } else { "" },
            back.map_or("-".into(), |f| format!("{:.1} s", f as f64 / 90.0))
        );
        if run {
            runs += 1;
            // (At the floor already, there's nowhere to go.)
            if least >= before && before > g.min + 1e-4 {
                wrong.push((
                    name.clone(),
                    format!("{name}: late in a run, and the eyes' scale didn't drop"),
                ));
            }
            if !back.is_some_and(|f| f < 600) {
                wrong.push((
                    name.clone(),
                    format!("{name}: the eyes' scale not back within 6.7 s"),
                ));
            }
        }
    }
    println!("    governor: {runs} moments ran late in a run");
    wrong
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD: f64 = 1000.0 / 90.0;

    /// The GPU's time at a scale, for a scene whose pixel work at scale 1
    /// is `pixels` ms and whose other work is `rest` ms.
    fn cost(scale: f32, pixels: f64, rest: f64) -> f64 {
        rest + pixels * f64::from(scale * scale)
    }

    #[test]
    fn the_floor_keeps_must_read_text_at_its_floor() {
        let px = floor_eye_px();
        // At the auto size the floors hold with a little to spare.
        assert!((1650.0..1800.0).contains(&px), "{px}");
        let ppd = px / super::super::legibility::EYE_TAN_SPAN * std::f32::consts::PI / 180.0;
        assert!(super::super::legibility::MUST_READ_DEG * ppd >= 7.5 - 1e-3);
    }

    /// A light scene climbs to the cap; a heavy one comes down to where it
    /// fits, never under the floor; neither moves once there.
    #[test]
    fn it_finds_the_size_that_fits_and_stays() {
        let mut g = Governor::new(0.6, 0.55, 1.0);
        for _ in 0..2000 {
            let s = g.scale;
            g.step(Some(cost(s, 3.0, 2.0)), PERIOD);
        }
        assert_eq!(g.scale, 1.0, "room to spare: the cap");

        let mut g = Governor::new(1.0, 0.55, 1.0);
        let mut seen = Vec::new();
        for i in 0..3000 {
            let s = g.scale;
            g.step(Some(cost(s, 14.0, 2.0)), PERIOD);
            if i >= 2000 {
                seen.push(g.scale);
            }
        }
        let worst = cost(g.scale, 14.0, 2.0);
        assert!(worst <= PERIOD * TARGET, "fits: {worst:.1} ms");
        assert!(
            worst > PERIOD * TARGET * 0.7,
            "not far under: {worst:.1} ms"
        );
        let (lo, hi) = seen
            .iter()
            .fold((9.0f32, 0.0f32), |(l, h), s| (l.min(*s), h.max(*s)));
        assert!(hi / lo < 1.035, "no pumping once settled: {lo}..{hi}");

        let mut g = Governor::new(1.0, 0.55, 1.0);
        for _ in 0..500 {
            let s = g.scale;
            g.step(Some(cost(s, 60.0, 2.0)), PERIOD);
        }
        assert_eq!(g.scale, 0.55, "never under the floor");
    }

    /// Noise round a scene that fits doesn't move it.
    #[test]
    fn noise_does_not_pump_it() {
        let mut g = Governor::new(0.8, 0.55, 1.0);
        let mut rng = fastrand::Rng::with_seed(3);
        let mut steps = 0;
        let mut last = g.scale;
        for _ in 0..3000 {
            let s = g.scale;
            // At 0.8: about 7.5 ms, ±8%.
            let t = cost(s, 9.0, 1.7) * (0.92 + 0.16 * rng.f64());
            g.step(Some(t), PERIOD);
            if g.scale != last {
                steps += 1;
                last = g.scale;
            }
        }
        assert!(steps <= 6, "{steps} steps in 33 s of noise");
    }

    /// A busy moment: it comes down within a few frames, holds, and goes
    /// back up within a few seconds once the moment's over.
    #[test]
    fn a_busy_moment_drops_it_and_it_recovers() {
        // Settled where it climbs to by itself.
        let mut g = Governor::new(0.6, 0.55, 1.0);
        let base = |s: f32| cost(s, 8.0, 1.5);
        for _ in 0..1500 {
            let s = g.scale;
            g.step(Some(base(s)), PERIOD);
        }
        let before = g.scale;
        let mut over = 0;
        let mut dropped_at = None;
        for f in 0..180 {
            let s = g.scale;
            let t = base(s) * 1.6;
            over += usize::from(t > PERIOD);
            g.step(Some(t), PERIOD);
            if dropped_at.is_none() && g.scale < before {
                dropped_at = Some(f);
            }
        }
        assert!(
            dropped_at.is_some_and(|f| f < 8),
            "drops at once: {dropped_at:?}"
        );
        assert!(over < 12, "{over} frames missed in the moment");
        let during = g.scale;
        let mut back = None;
        for f in 0..900 {
            let s = g.scale;
            g.step(Some(base(s)), PERIOD);
            if back.is_none() && g.scale >= before * 0.97 {
                back = Some(f);
            }
        }
        assert!(during < before * 0.9, "{before} > {during}");
        assert!(back.is_some_and(|f| f < 450), "back within 5 s: {back:?}");
    }

    /// The playtest's frame: a Beyond 2 at 75 Hz, the eyes' work more than
    /// the period at 100%, and the GPU's measured time only part of it (a
    /// third: one view's). Every late frame the runtime reprojects, and
    /// says so with a period twice the display's (26.7 ms). The eyes come
    /// down until frames land at the display's rate, and stay there.
    #[test]
    fn half_rate_frames_step_the_eyes_down() {
        let display = 1000.0 / 75.0;
        // At 100%: 22 ms of work, the frames at half rate.
        let truth = |s: f32| cost(s, 19.0, 3.0);
        let mut g = Governor::new(0.6, 0.45, 1.0);
        let mut d = Display::default();
        let mut late_at_end = 0;
        // As the session starts, a frame or two on time.
        d.see(display);
        for f in 0..3000 {
            let s = g.scale;
            let work = truth(s);
            let late = work > display;
            let predicted = if late { 2.0 * display } else { display };
            let (period, reprojecting) = d.see(predicted);
            g.step_seen(Some(work / 3.0), period, reprojecting);
            if f >= 2500 && late {
                late_at_end += 1;
            }
        }
        assert!((d.period_ms - display).abs() < 1e-6, "{}", d.period_ms);
        assert_eq!(late_at_end, 0, "frames still late at {:.2}", g.scale);
        assert!(g.scale < 0.85, "{}", g.scale);
        assert!(truth(g.scale) > display * 0.5, "not far under: {}", g.scale);
    }

    /// Reprojecting from the start (no on-time frame seen): no PC headset
    /// refreshes under 60 Hz, so 26.7 ms is a late one all the same.
    #[test]
    fn a_session_late_from_the_start_is_still_late() {
        let mut d = Display::default();
        let (period, late) = d.see(2000.0 / 75.0);
        assert!(late, "{period}");
        assert!(period <= 1000.0 / 60.0 + 1e-9);
        let (period, late) = d.see(1000.0 / 90.0);
        assert!(!late && (period - 1000.0 / 90.0).abs() < 1e-9);
    }

    /// No GPU time back (no timestamp queries): it holds where it is.
    #[test]
    fn without_gpu_times_it_holds() {
        let mut g = Governor::new(0.7, 0.55, 1.0);
        for _ in 0..500 {
            g.step(None, PERIOD);
        }
        assert_eq!(g.scale, 0.7);
    }
}
