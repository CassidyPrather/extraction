//! `--tour DIR`: a QA walk round the map. After the scene settles, the
//! free camera visits every placement, from outside (a three-quarter view
//! of its front) and, for buildings you go into, from inside (by the back
//! wall, looking to the door), and saves a screenshot of each to DIR with
//! a `tour.json` saying what each one shows. Notes, signs and terminals
//! get a view each, from where you'd stand to read them. Run it under
//! `scripts/tour.sh` and look through the pictures for odd models,
//! z-fighting, things floating or sunk, clipping, unreadable text.
//! `--tour-only TEXT` keeps the views whose file name contains TEXT.

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
            // Inside: in a back corner at eye height above the floor (the
            // building stands where `Heightmap::footing` says; floors sit
            // up to half a metre above that, the ranger tower's cabin on
            // its legs), looking across to the opposite front corner, so
            // the room shows rather than the nearest wall.
            let footing = zone.hm.footing(&p.prefab, p.at, p.rot);
            let floor = match p.prefab.as_str() {
                "ranger_tower" => 9.1,
                _ => 0.4,
            };
            let (cx, cz) = if p.prefab == "ranger_tower" {
                (1.6, 1.6)
            } else {
                (f.x * 0.5 - 0.6, f.z * 0.5 - 0.6)
            };
            let corner = Vec3::new(p.at[0], 0.0, p.at[1]) + rot * Vec3::new(cx, 0.0, cz);
            let eye = Vec3::new(corner.x, footing + floor + 1.6, corner.z);
            let across = Vec3::new(p.at[0], 0.0, p.at[1]) + rot * Vec3::new(-cx, 0.0, -cz);
            out.push(View {
                file: format!("{i:03}_{}_in.png", p.prefab),
                what: format!("{} at ({:.0}, {:.0}), inside", p.prefab, p.at[0], p.at[1]),
                at: eye.to_array(),
                look: Vec3::new(across.x, eye.y - 0.6, across.z).to_array(),
            });
        }
    }
    // Words: from where you'd stand to read them.
    for (i, m) in zone.map.markers.iter().enumerate() {
        use extraction_core::map::MarkerKind;
        let (kind, reach, lift, back) = match &m.kind {
            MarkerKind::Note { .. } => ("note", 2.2 + m.y, 0.0, 0.55),
            MarkerKind::Terminal { .. } => ("terminal", 2.2 + m.y, 0.23, 0.9),
            MarkerKind::Sign { .. } => ("sign", 0.0, 1.45, 1.6),
            MarkerKind::ValleyKey { .. } => ("key", 0.0, 0.1, 1.4),
            // The depot lock beside the Gate (see `capstone`).
            MarkerKind::RobotGate => {
                let yaw = Quat::from_rotation_y(-m.rot.to_radians());
                let off = yaw * Vec3::new(-10.5, 0.0, -2.5);
                let at = Vec3::new(m.at[0] + off.x, 0.0, m.at[1] + off.z);
                let target = Vec3::new(at.x, zone.ground(at.x, at.z) + 1.2, at.z);
                let eye = target + yaw * Vec3::new(0.0, 0.3, 1.3);
                out.push(View {
                    file: format!(
                        "w{i:03}_lock_{}.png",
                        m.name.to_lowercase().replace(' ', "_")
                    ),
                    what: format!("depot lock by \"{}\"", m.name),
                    at: eye.to_array(),
                    look: target.to_array(),
                });
                continue;
            }
            _ => continue,
        };
        let y = if kind == "sign" {
            zone.ground(m.at[0], m.at[1])
        } else {
            crate::world::notes::rest_height(zone, m.at[0], m.at[1], reach)
        };
        // The text faces the marker's local +Z.
        let r = m.rot.to_radians();
        let out_dir = Vec3::new(-r.sin(), 0.0, r.cos());
        let target = Vec3::new(m.at[0], y + lift, m.at[1]);
        let eye = target + out_dir * back + Vec3::Y * if kind == "note" { 0.55 } else { 0.15 };
        let slug = m.name.to_lowercase().replace(' ', "_");
        out.push(View {
            file: format!("w{i:03}_{kind}_{slug}.png"),
            what: format!("{kind} \"{}\" at ({:.0}, {:.0})", m.name, m.at[0], m.at[1]),
            at: eye.to_array(),
            look: target.to_array(),
        });
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
        let mut v = views(&zone);
        if let Some(only) = &opts.tour_only {
            v.retain(|v| v.file.contains(only.as_str()));
        }
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
