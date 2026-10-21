//! `--legibility DIR`: the render-based legibility gauge. After the scene
//! settles, the free camera visits every text panel from where it's read
//! (`legibility::TextSource::reading`), with a field of view that gives
//! the headset's pixels a degree at the auto render scale, and saves a
//! screenshot of each to DIR with `legibility.json`: what the panel says,
//! where it is in the picture, and its floor. `scripts/legibility.sh` runs
//! it and reads each crop back with OCR (`scripts/legibility.py`).

use bevy::camera::Camera;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use super::FreeCamera;
use super::legibility::{TextSource, eye_px_per_deg, panel_deg};
use crate::launch::LaunchOptions;
use crate::raid::bench::ScreenText;

pub struct LegibilityShotsPlugin;

impl Plugin for LegibilityShotsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shots>().add_systems(Update, shoot);
    }
}

#[derive(serde::Serialize, Clone)]
struct Shot {
    file: String,
    source: String,
    name: String,
    need: String,
    floor: f32,
    /// The gauge's own measure: smallest letters (degrees).
    deg: f32,
    at: f32,
    text: String,
    /// The panel in the picture: x, y, w, h (pixels).
    rect: [f32; 4],
    /// Light (dark letters) or dark (light letters) ground.
    dark: bool,
    /// Where the panel is, and the eye.
    centre: [f32; 3],
    eye: [f32; 3],
}

#[derive(Resource, Default)]
struct Shots {
    targets: Option<Vec<Entity>>,
    next: usize,
    wait: u32,
    shooting: bool,
    done: Vec<Shot>,
}

/// Frames for a view to settle.
const SETTLE: u32 = 6;

#[allow(clippy::too_many_arguments)]
fn shoot(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    mut state: ResMut<Shots>,
    panels: Query<(
        Entity,
        &ScreenText,
        &TextSource,
        &GlobalTransform,
        &InheritedVisibility,
        Option<&Name>,
    )>,
    mut cams: Query<(&mut Transform, &mut Projection, &Camera), With<FreeCamera>>,
    windows: Query<&Window>,
    mut adapt: ResMut<super::grade::Adaptation>,
    ambient: Res<GlobalAmbientLight>,
    grades: Query<&bevy::render::view::ColorGrading, With<FreeCamera>>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(dir) = &opts.legibility else {
        return;
    };
    *frames += 1;
    if *frames < opts.frames {
        return;
    }
    let state = &mut *state;
    if state.shooting {
        return;
    }
    let targets = state.targets.get_or_insert_with(|| {
        let mut t: Vec<(String, Entity)> = panels
            .iter()
            .filter(|(_, s, src, _, vis, _)| {
                vis.get() && !s.text.trim().is_empty() && **src != TextSource::Dev
            })
            // `--tour-only TEXT`: the panels whose source contains TEXT.
            .filter(|(_, _, src, _, _, _)| {
                opts.tour_only
                    .as_ref()
                    .is_none_or(|o| format!("{src:?}").contains(o.as_str()))
            })
            .map(|(e, _, src, gt, _, _)| {
                let p = gt.translation();
                (format!("{src:?} {:08.2} {:08.2}", p.x, p.z), e)
            })
            .collect();
        t.sort_by(|a, b| a.0.cmp(&b.0));
        let _ = std::fs::create_dir_all(dir);
        info!("legibility: {} panels", t.len());
        t.into_iter().map(|(_, e)| e).collect()
    });
    let Some(&target) = targets.get(state.next) else {
        if let Ok(json) = serde_json::to_string_pretty(&state.done) {
            let _ = std::fs::write(dir.join("legibility.json"), json);
        }
        exit.write(AppExit::Success);
        return;
    };
    let Ok((_, screen, source, gt, _, name)) = panels.get(target) else {
        state.next += 1;
        return;
    };
    let Ok((mut cam, mut proj, camera)) = cams.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let r = source.reading();
    // From in front of the panel, moved toward its bottom edge by the
    // reading pose's tilt (a note on a table is read from its near edge).
    let rot = gt.rotation();
    let normal = rot * Quat::from_rotation_x(r.tilt.to_radians()) * Vec3::Z;
    let centre = gt.translation();
    let want =
        Transform::from_translation(centre + normal * r.at).looking_at(centre, rot * Vec3::Y);
    let fov = (window.height() / eye_px_per_deg()).to_radians();
    if let Projection::Perspective(p) = &mut *proj {
        if (p.fov - fov).abs() > 1e-4 {
            p.fov = fov;
        }
    }
    if cam.translation.distance(want.translation) > 0.002 {
        state.wait = 0;
    }
    // Read with eyes used to the place (a room's dark takes a while).
    if state.wait < SETTLE - 1 {
        adapt.resettle();
    }
    *cam = want;
    state.wait += 1;
    if state.wait < SETTLE {
        return;
    }
    // Where the panel's corners land in the picture.
    let cam_gt = GlobalTransform::from(want);
    let half = screen.size * 0.5;
    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let p = gt.transform_point(Vec3::new(x * half.x, y * half.y, 0.0));
        if let Ok(v) = camera.world_to_viewport(&cam_gt, p) {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    let file = format!("{:03}_{:?}.png", state.next, source);
    info!(
        "legibility: {file} panel at {:.2} seen from {:.2} (ambient {:.1}, exposure {:.2}, indoors {:.2})",
        centre,
        want.translation,
        ambient.brightness,
        grades.iter().next().map_or(0.0, |g| g.global.exposure),
        adapt.indoors
    );
    state.done.push(Shot {
        file: file.clone(),
        source: format!("{source:?}"),
        name: name.map_or(String::new(), |n| n.to_string()),
        need: format!("{:?}", r.need),
        floor: r.need.floor(),
        deg: panel_deg(screen, *source),
        at: r.at,
        text: screen.text.clone(),
        rect: [lo.x, lo.y, hi.x - lo.x, hi.y - lo.y],
        dark: matches!(
            screen.style,
            crate::raid::bench::ScreenStyle::Lcd | crate::raid::bench::ScreenStyle::Crt
        ),
        centre: centre.to_array(),
        eye: want.translation.to_array(),
    });
    state.shooting = true;
    let mut save = save_to_disk(dir.join(&file));
    commands.spawn(Screenshot::primary_window()).observe(
        move |shot: On<ScreenshotCaptured>, mut state: ResMut<Shots>| {
            save(shot);
            state.shooting = false;
            state.next += 1;
            state.wait = 0;
        },
    );
}
