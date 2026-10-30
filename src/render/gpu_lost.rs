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
    stop(&format!("{:?} {}", error.ty, error.description));
}

/// The GPU's gone: the line in the log and the crash file, and out.
pub fn stop(error: &str) -> ! {
    let text = line(
        SESSION.get().map(|s| s.elapsed().as_secs_f64()),
        super::timeline::since_start(),
        error,
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

/// Whether a panic is the GPU going (or what its going leaves behind),
/// from where it was raised and what it said. On the playtest's machine
/// these came before Bevy's device-lost error ever reached `handle`: the
/// window's swapchain torn down mid-acquire (wgpu-hal), a screenshot's
/// readback that couldn't map ("Failed to map buffer"), and the clustering
/// mutex the first panic poisoned (`PoisonError` in a render crate).
pub fn is_gpu_loss(message: &str, file: &str) -> bool {
    let file = file.replace('\\', "/");
    let render = [
        "/bevy_render-",
        "/bevy_pbr-",
        "/bevy_core_pipeline-",
        "/bevy_mod_openxr-",
    ]
    .iter()
    .any(|c| file.contains(c));
    file.contains("/wgpu-hal-")
        || file.contains("/wgpu-core-")
        || message.contains("Failed to map buffer")
        || message.contains("DeviceLost")
        || message.contains("Device lost")
        || message.contains("Parent device is lost")
        || message.contains("swap chain texture")
        || (render && message.contains("PoisonError"))
}

#[cfg(test)]
mod tests {
    /// The playtest's four crashes (docs/playtests/2026-10-28/logs), and
    /// a panic of the game's own that isn't the GPU's.
    #[test]
    fn the_playtests_gpu_panics_are_the_gpus() {
        let reg = r"C:\Users\<user>\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f";
        for (message, file) in [
            (
                "Trying to destroy a SwapchainAcquireSemaphore that is still in use by a SurfaceTexture",
                format!(r"{reg}\wgpu-hal-29.0.4\src\vulkan\swapchain\native.rs"),
            ),
            (
                "Failed to map buffer: BufferAsyncError",
                format!(r"{reg}\bevy_render-0.19.1\src\gpu_readback.rs"),
            ),
            (
                "called `Result::unwrap()` on an `Err` value: PoisonError { .. }",
                format!(r"{reg}\bevy_pbr-0.19.1\src\cluster\gpu.rs"),
            ),
        ] {
            assert!(super::is_gpu_loss(message, &file), "{message}");
        }
        assert!(!super::is_gpu_loss(
            "called `Option::unwrap()` on a `None` value",
            "src/raid/mod.rs"
        ));
        assert!(!super::is_gpu_loss(
            "called `Result::unwrap()` on an `Err` value: PoisonError { .. }",
            "src/lib.rs"
        ));
    }

    #[test]
    fn the_line_says_when() {
        assert_eq!(
            super::line(Some(12.04), 40.0, "DeviceLost Unknown"),
            "the GPU stopped responding 12.0 s into the headset session: DeviceLost Unknown"
        );
        assert!(super::line(None, 7.5, "x").contains("7.5 s after the game started"));
    }
}
