//! The GPU stopping (a driver's timeout: Windows resets a GPU that takes
//! over 2 s on a frame): one line in the log saying so and when, the same
//! in a crash report next to it (the playtest report takes those), and a
//! clean exit. Bevy's own handling asked the app to quit and let it run
//! on a frame, and every system that touched the lost device panicked on
//! the way out (a screenshot's buffer, the headset's swapchain).

use bevy::prelude::*;
use bevy::render::error_handler::{ErrorType, RenderError, RenderErrorHandler, RenderErrorPolicy};
use std::sync::OnceLock;
use std::time::Instant;

pub struct GpuLostPlugin;

impl Plugin for GpuLostPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderErrorHandler(handle))
            .add_systems(Last, headset_started);
    }
}

/// When the headset's eyes first drew.
static SESSION: OnceLock<Instant> = OnceLock::new();

fn headset_started(eyes: Query<(), With<bevy_mod_xr::camera::XrCamera>>) {
    if SESSION.get().is_none() && !eyes.is_empty() {
        let _ = SESSION.set(Instant::now());
    }
}

/// What the log says: when, and what the driver said.
pub fn line(into_session: Option<f64>, since_start: f64, error: &str) -> String {
    let when = match into_session {
        Some(s) => format!("{s:.1} s into the headset session"),
        None => format!("{since_start:.1} s after the game started (no headset session)"),
    };
    format!("the GPU stopped responding {when}: {error}")
}

fn handle(error: &RenderError, main_world: &mut World, _: &mut World) -> RenderErrorPolicy {
    if error.ty != ErrorType::DeviceLost {
        // As Bevy's own: quit, rendering nothing more.
        error!("Quitting the application due to {:?} RenderError", error.ty);
        main_world.write_message(AppExit::error());
        return RenderErrorPolicy::StopRendering;
    }
    let text = line(
        SESSION.get().map(|s| s.elapsed().as_secs_f64()),
        super::timeline::since_start(),
        &format!("{:?} {}", error.ty, error.description),
    );
    error!("{text}");
    let dir = crate::log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(format!("crash-{}.txt", crate::session_stamp())),
        format!(
            "{text}\n\nThe GPU driver reset the device (on Windows, a frame over 2 s).\n\
             The session log beside this one has the frames before it.\n"
        ),
    );
    // Now: whatever still holds the lost device (a screenshot's buffer,
    // the headset's swapchain) would only panic on its way down.
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_line_says_when() {
        assert_eq!(
            super::line(Some(12.04), 40.0, "DeviceLost Unknown"),
            "the GPU stopped responding 12.0 s into the headset session: DeviceLost Unknown"
        );
        assert!(super::line(None, 7.5, "x").contains("7.5 s after the game started"));
    }
}
