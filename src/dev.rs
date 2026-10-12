//! Developer tooling: screenshot-and-quit, the camera override, a frame
//! time readout.

use crate::launch::LaunchOptions;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameCount>()
            .add_systems(Update, (count_frames, screenshot_when_ready));
    }
}

#[derive(Resource, Default)]
struct FrameCount {
    frames: u32,
    requested: bool,
}

fn count_frames(mut count: ResMut<FrameCount>) {
    count.frames += 1;
}

fn screenshot_when_ready(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    mut count: ResMut<FrameCount>,
) {
    let Some(path) = &opts.screenshot else {
        return;
    };
    if count.requested || count.frames < opts.frames {
        return;
    }
    count.requested = true;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut save = save_to_disk(path.clone());
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            save(captured);
            exit.write(AppExit::Success);
        },
    );
}
