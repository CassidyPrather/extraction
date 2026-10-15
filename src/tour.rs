//! `--tour DIR`: a QA walk round the map. After the scene settles, the
//! free camera visits every placement, from outside (a three-quarter view
//! of its front) and, for buildings you go into, from inside (by the back
//! wall, looking to the door), and saves a screenshot of each to DIR with
//! a `tour.json` saying what each one shows. Run it under `scripts/tour.sh`
//! and look through the pictures for odd models, z-fighting, things
//! floating or sunk, clipping.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use crate::launch::LaunchOptions;
use crate::render::FreeCamera;
use crate::world::Zone;

pub struct TourPlugin;

impl Plugin for TourPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tour>().add_systems(Update, tour);
    }
}

/// One stop: where the camera stands and looks, and what it's for.
#[derive(Clone, Debug, serde::Serialize)]
pub struct View {
    pub file: String,
    pub what: String,
    pub at: [f32; 3],
    pub look: [f32; 3],
}

#[derive(Resource, Default)]
struct Tour {
    views: Option<Vec<View>>,
    next: usize,
    wait: u32,
    shooting: bool,
}

/// Frames for a view to settle (streamed tree detail, the fog).
const SETTLE: u32 = 6;

/// Prefabs you walk into.
fn has_inside(prefab: &str) -> bool {
    !matches!(
        prefab,
        "radio_mast"
            | "chimney"
            | "log_pile"
            | "boxcar"
            | "anomaly_core"
            | "robot_gate"
            | "quarantine_gate"
            | "footbridge"
    )
}

/// Every placement's views.
pub fn views(zone: &Zone) -> Vec<View> {
    let mut out = Vec::new();
    for (i, p) in zone.map.placements.iter().enumerate() {
        let f = crate::world::structures::footprint(&p.prefab);
        let yaw = -p.rot.to_radians();
        let rot = Quat::from_rotation_y(yaw);
        let ground = zone.ground(p.at[0], p.at[1]);
        let centre = Vec3::new(p.at[0], ground + f.y * 0.4, p.at[1]);
        // Outside: off the front-left corner, far enough to see it whole.
        let reach = f.x.max(f.z) * 0.9 + 6.0;
        let from =
            Vec3::new(p.at[0], 0.0, p.at[1]) + rot * Vec3::new(-0.6, 0.0, -1.0).normalize() * reach;
        let from = Vec3::new(
            from.x,
            zone.ground(from.x, from.z) + 1.7 + f.y * 0.3,
            from.z,
        );
        out.push(View {
            file: format!("{i:03}_{}_out.png", p.prefab),
            what: format!("{} at ({:.0}, {:.0}), outside", p.prefab, p.at[0], p.at[1]),
            at: from.to_array(),
            look: centre.to_array(),
        });
        if has_inside(&p.prefab) {
            // Inside: by the back wall, eye height above the floor (the
            // building stands on its lowest corner; floors sit up to half a
            // metre above it), looking to the front.
            let back =
                Vec3::new(p.at[0], 0.0, p.at[1]) + rot * Vec3::new(0.15 * f.x, 0.0, f.z * 0.32);
            let eye = Vec3::new(back.x, ground + 2.1, back.z);
            let front = Vec3::new(p.at[0], 0.0, p.at[1]) + rot * Vec3::new(0.0, 0.0, -f.z * 0.5);
            out.push(View {
                file: format!("{i:03}_{}_in.png", p.prefab),
                what: format!("{} at ({:.0}, {:.0}), inside", p.prefab, p.at[0], p.at[1]),
                at: eye.to_array(),
                look: Vec3::new(front.x, eye.y - 0.4, front.z).to_array(),
            });
        }
    }
    out
}

fn tour(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Option<Res<Zone>>,
    mut state: ResMut<Tour>,
    mut cams: Query<&mut Transform, With<FreeCamera>>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let (Some(dir), Some(zone)) = (&opts.tour, zone) else {
        return;
    };
    *frames += 1;
    // The scene's first settling: textures, the impostors' pictures.
    if *frames < opts.frames {
        return;
    }
    let state = &mut *state;
    let views = state.views.get_or_insert_with(|| {
        let v = views(&zone);
        let _ = std::fs::create_dir_all(dir);
        if let Ok(json) = serde_json::to_string_pretty(&v) {
            let _ = std::fs::write(dir.join("tour.json"), json);
        }
        info!("tour: {} views", v.len());
        v
    });
    if state.shooting {
        return;
    }
    let Some(view) = views.get(state.next).cloned() else {
        exit.write(AppExit::Success);
        return;
    };
    let Ok(mut cam) = cams.single_mut() else {
        return;
    };
    let want = Transform::from_translation(Vec3::from_array(view.at))
        .looking_at(Vec3::from_array(view.look), Vec3::Y);
    if *cam != want {
        *cam = want;
        state.wait = 0;
        return;
    }
    state.wait += 1;
    if state.wait < SETTLE {
        return;
    }
    state.shooting = true;
    let mut save = save_to_disk(dir.join(&view.file));
    commands.spawn(Screenshot::primary_window()).observe(
        move |shot: On<ScreenshotCaptured>, mut state: ResMut<Tour>| {
            save(shot);
            state.shooting = false;
            state.next += 1;
        },
    );
}
