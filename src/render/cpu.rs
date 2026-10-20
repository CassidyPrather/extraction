//! Where a frame's CPU goes, by schedule. Tiny marker schedules sit
//! between the main schedules (`First`, `PreUpdate`, the fixed loop,
//! `Update`, `PostUpdate`, `Last`) and between the fixed ones, and stamp
//! the time: each schedule's wall time a frame, exactly, with no tracing
//! build. `budget::tests` holds a raid moment to [`SHARE`]; the session
//! log gets the split every ten seconds when `EXTRACTION_CPU=1`.

use bevy::app::{FixedMainScheduleOrder, MainScheduleOrder};
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use std::collections::HashMap;
use std::time::Instant;

/// The spans, in order: a main schedule, or a fixed one (summed over the
/// frame's fixed ticks; the fixed loop's own span is what's left of it).
pub const MAIN: [&str; 6] = [
    "First",
    "PreUpdate",
    "fixed loop",
    "Update",
    "PostUpdate",
    "Last",
];
pub const FIXED: [&str; 5] = [
    "FixedFirst",
    "FixedPreUpdate",
    "FixedUpdate",
    "FixedPostUpdate",
    "FixedLast",
];

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Mark(u8);
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct FixedMark(u8);

/// Per-frame milliseconds by span, as recorded.
#[derive(Resource, Default)]
pub struct ScheduleTimes {
    at: Option<Instant>,
    fixed_at: Option<Instant>,
    frame: HashMap<&'static str, f64>,
    /// One entry a frame: span name to ms.
    pub frames: Vec<HashMap<&'static str, f64>>,
}

impl ScheduleTimes {
    /// Mean and 99th percentile ms a frame per span, biggest mean first,
    /// over the frames after `skip`.
    pub fn stats(&self, skip: usize) -> Vec<(&'static str, f64, f64)> {
        let frames = &self.frames[skip.min(self.frames.len())..];
        let mut out: Vec<_> = MAIN
            .iter()
            .chain(FIXED.iter())
            .chain(["main schedule"].iter())
            .map(|name| {
                let mut v: Vec<f64> = frames
                    .iter()
                    .map(|f| f.get(name).copied().unwrap_or(0.0))
                    .collect();
                v.sort_by(f64::total_cmp);
                let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
                let p99 = v
                    .get(((v.len() as f64 * 0.99) as usize).min(v.len().saturating_sub(1)))
                    .copied()
                    .unwrap_or(0.0);
                (*name, mean, p99)
            })
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        out
    }
}

pub struct CpuSplitPlugin;

impl Plugin for CpuSplitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScheduleTimes>();
        let main = [
            First.intern(),
            PreUpdate.intern(),
            RunFixedMainLoop.intern(),
            Update.intern(),
            PostUpdate.intern(),
            Last.intern(),
        ];
        for i in 0..=main.len() {
            app.init_schedule(Mark(i as u8));
            app.add_systems(Mark(i as u8), move |mut t: ResMut<ScheduleTimes>| {
                t.main_mark(i)
            });
        }
        {
            let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
            for (i, label) in main.iter().enumerate() {
                if let Some(at) = order.labels.iter().position(|l| l == label) {
                    order.labels.insert(at, Mark(i as u8).intern());
                }
            }
            order.labels.push(Mark(main.len() as u8).intern());
        }
        let fixed = [
            FixedFirst.intern(),
            FixedPreUpdate.intern(),
            FixedUpdate.intern(),
            FixedPostUpdate.intern(),
            FixedLast.intern(),
        ];
        for i in 0..=fixed.len() {
            app.init_schedule(FixedMark(i as u8));
            app.add_systems(FixedMark(i as u8), move |mut t: ResMut<ScheduleTimes>| {
                t.fixed_mark(i)
            });
        }
        {
            let mut order = app.world_mut().resource_mut::<FixedMainScheduleOrder>();
            for (i, label) in fixed.iter().enumerate() {
                if let Some(at) = order.labels.iter().position(|l| l == label) {
                    order.labels.insert(at, FixedMark(i as u8).intern());
                }
            }
            order.labels.push(FixedMark(fixed.len() as u8).intern());
        }
        if std::env::var_os("EXTRACTION_CPU").is_some() {
            app.add_systems(Update, log_split);
        }
    }
}

impl ScheduleTimes {
    fn main_mark(&mut self, i: usize) {
        let now = Instant::now();
        if i == 0 {
            self.frame.clear();
        } else if let Some(at) = self.at {
            let ms = (now - at).as_secs_f64() * 1000.0;
            *self.frame.entry(MAIN[i - 1]).or_default() += ms;
        }
        self.at = Some(now);
        if i == 6 {
            // The fixed loop's own span holds the fixed schedules: keep
            // only what's left, and total the frame.
            let fixed: f64 = FIXED
                .iter()
                .map(|f| self.frame.get(f).copied().unwrap_or(0.0))
                .sum();
            if let Some(l) = self.frame.get_mut("fixed loop") {
                *l = (*l - fixed).max(0.0);
            }
            let total: f64 = self.frame.values().sum();
            self.frame.insert("main schedule", total);
            let frame = std::mem::take(&mut self.frame);
            self.frames.push(frame);
            if self.frames.len() > 20_000 {
                self.frames.drain(..10_000);
            }
        }
    }

    fn fixed_mark(&mut self, i: usize) {
        let now = Instant::now();
        if i > 0 {
            if let Some(at) = self.fixed_at {
                let ms = (now - at).as_secs_f64() * 1000.0;
                *self.frame.entry(FIXED[i - 1]).or_default() += ms;
            }
        }
        self.fixed_at = Some(now);
    }
}

/// Every ten seconds, the split (`EXTRACTION_CPU=1`).
fn log_split(time: Res<Time<Real>>, mut t: ResMut<ScheduleTimes>, mut since: Local<f32>) {
    *since += time.delta_secs();
    if *since < 10.0 {
        return;
    }
    *since = 0.0;
    let line: Vec<String> = t
        .stats(0)
        .iter()
        .map(|(n, mean, p99)| format!("{n} {mean:.1} (1% {p99:.1})"))
        .collect();
    info!("cpu split, ms a frame: {}", line.join(", "));
    t.frames.clear();
}
