//! `--wayfind DIR`: the render-based wayfinding gauge. From where you get
//! your bearings (the truck's stop, every rendezvous: standing, eyes
//! 1.7 m up), the free camera turns to each landmark in range (a
//! placement at least `map_lint::LANDMARK_HEIGHT` tall, within
//! `map_lint::CLEAR_SIGHT`) with the headset's pixels a degree, and
//! shoots it twice: as it is, and with the landmark hidden. What differs
//! is the landmark as the eye gets it, through fog, trees and the dark:
//! `scripts/wayfind.py` measures its size and contrast and says whether
//! you could steer by it. Writes `wayfind.json` beside the pictures.

use bevy::camera::Camera;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use extraction_core::map::{MarkerKind, prefab_footprint};
use extraction_core::map_lint::{CLEAR_SIGHT, LANDMARK_HEIGHT};

use super::FreeCamera;
use super::legibility::eye_px_per_deg;
use crate::launch::LaunchOptions;
use crate::world::Zone;
use crate::world::buildings::Building;

pub struct WayfindShotsPlugin;

impl Plugin for WayfindShotsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shots>().add_systems(Update, shoot);
    }
}

#[derive(serde::Serialize, Clone)]
struct View {
    /// Where from: "truck" or the rendezvous' name.
    from: String,
    landmark: String,
    distance: f32,
    eye: [f32; 3],
    look: [f32; 3],
    /// With the landmark, and without it.
    with: String,
    without: String,
    /// The landmark's box in the picture: x, y, w, h (pixels).
    rect: [f32; 4],
    #[serde(skip)]
    entity: Option<Entity>,
    /// Its footprint (w, h, d) and pose, for `rect`.
    #[serde(skip)]
    size: Vec3,
    #[serde(skip)]
    pose: Transform,
}

#[derive(Resource, Default)]
struct Shots {
    views: Option<Vec<View>>,
    next: usize,
    /// 0: with the landmark; 1: without it.
    phase: u8,
    wait: u32,
    shooting: bool,
    /// The landmark hidden for the last "without" shot.
    hidden: Option<Entity>,
}

const SETTLE: u32 = 8;
const EYE: f32 = 1.7;

#[allow(clippy::too_many_arguments)]
fn shoot(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Option<Res<Zone>>,
    mut state: ResMut<Shots>,
    buildings: Query<(Entity, &Building, &GlobalTransform)>,
    mut vis: Query<&mut Visibility>,
    mut cams: Query<(&mut Transform, &mut Projection, &Camera), With<FreeCamera>>,
    mut time: ResMut<Time<Virtual>>,
    windows: Query<&Window>,
    mut adapt: ResMut<super::grade::Adaptation>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(dir), Some(zone)) = (&opts.wayfind, zone) else {
        return;
    };
    *frames += 1;
    if *frames < opts.frames || state.shooting {
        return;
    }
    let state = &mut *state;
    let views = state.views.get_or_insert_with(|| {
        let mut from: Vec<(String, Vec3)> = Vec::new();
        for m in &zone.map.markers {
            // Where the truck sets you down (its stop is inside the truck).
            let name = match m.kind {
                MarkerKind::Insertion => "truck".to_string(),
                MarkerKind::Rendezvous => format!("rv {}", m.name),
                _ => continue,
            };
            from.push((name, zone.on_ground(m.at[0], m.at[1]) + Vec3::Y * EYE));
        }
        let marks: Vec<(Entity, String, Transform, Vec3)> = buildings
            .iter()
            .filter_map(|(e, b, g)| {
                let f = prefab_footprint(&b.prefab);
                (f[1] >= LANDMARK_HEIGHT)
                    .then(|| (e, b.prefab.clone(), g.compute_transform(), Vec3::from(f)))
            })
            .collect();
        let mut v = Vec::new();
        for (name, eye) in &from {
            for (e, prefab, pose, size) in &marks {
                let look = pose.translation + Vec3::Y * size.y * 0.6;
                let d = look.xz().distance(eye.xz());
                if d > CLEAR_SIGHT || d < 15.0 {
                    continue;
                }
                let i = v.len();
                v.push(View {
                    from: name.clone(),
                    landmark: prefab.clone(),
                    distance: d,
                    eye: eye.to_array(),
                    look: look.to_array(),
                    with: format!("{i:03}_a.png"),
                    without: format!("{i:03}_b.png"),
                    rect: [0.0; 4],
                    entity: Some(*e),
                    size: *size,
                    pose: *pose,
                });
            }
        }
        let _ = std::fs::create_dir_all(dir);
        // Nothing moves between a view's two shots (rain, grass, a
        // burner's flicker): only the landmark differs.
        time.pause();
        info!("wayfind: {} views from {} places", v.len(), from.len());
        v
    });
    let Some(view) = views.get(state.next).cloned() else {
        if let Ok(json) = serde_json::to_string_pretty(&views) {
            let _ = std::fs::write(dir.join("wayfind.json"), json);
        }
        exit.write(AppExit::Success);
        return;
    };
    let Ok((mut cam, mut proj, camera)) = cams.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let want = Transform::from_translation(Vec3::from_array(view.eye))
        .looking_at(Vec3::from_array(view.look), Vec3::Y);
    let fov = (window.height() / eye_px_per_deg()).to_radians();
    if let Projection::Perspective(p) = &mut *proj {
        if (p.fov - fov).abs() > 1e-4 {
            p.fov = fov;
        }
    }
    if *cam != want {
        *cam = want;
        state.wait = 0;
    }
    // The last view's landmark back; this one's shown for the first shot,
    // hidden for the second.
    if state.phase == 0 {
        if let Some(mut v) = state.hidden.take().and_then(|e| vis.get_mut(e).ok()) {
            *v = Visibility::Inherited;
        }
    } else {
        state.hidden = view.entity;
    }
    if let Some(mut v) = view.entity.and_then(|e| vis.get_mut(e).ok()) {
        let want = if state.phase == 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
            state.wait = 0;
        }
    }
    state.wait += 1;
    if state.wait < SETTLE - 2 {
        adapt.resettle();
    }
    if state.wait < SETTLE {
        return;
    }
    // Where the landmark's box lands in the picture.
    if state.phase == 0 {
        let cam_gt = GlobalTransform::from(want);
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        let s = view.size;
        for c in 0..8 {
            let local = Vec3::new(
                if c & 1 == 0 { -0.5 } else { 0.5 } * s.x,
                // Up past its top, for the lamp on it (`render::landmarks`).
                if c & 2 == 0 { 0.0 } else { s.y + 1.5 },
                if c & 4 == 0 { -0.5 } else { 0.5 } * s.z,
            );
            if let Ok(p) = camera.world_to_viewport(&cam_gt, view.pose.transform_point(local)) {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        let (w, h) = (window.width(), window.height());
        let lo = lo.max(Vec2::ZERO);
        let hi = hi.min(Vec2::new(w, h));
        views[state.next].rect = [lo.x, lo.y, (hi.x - lo.x).max(0.0), (hi.y - lo.y).max(0.0)];
    }
    state.shooting = true;
    let file = if state.phase == 0 {
        view.with.clone()
    } else {
        view.without.clone()
    };
    let mut save = save_to_disk(dir.join(file));
    commands.spawn(Screenshot::primary_window()).observe(
        move |shot: On<ScreenshotCaptured>, mut state: ResMut<Shots>| {
            save(shot);
            state.shooting = false;
            state.wait = 0;
            if state.phase == 0 {
                state.phase = 1;
            } else {
                state.phase = 0;
                state.next += 1;
            }
        },
    );
}
