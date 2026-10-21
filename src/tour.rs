//! `--tour DIR`: a QA walk round the map. After the scene settles, the
//! free camera visits every placement, from outside (a three-quarter view
//! of its front) and, for buildings you go into, from inside (by the back
//! wall, looking to the door), and saves a screenshot of each to DIR with
//! a `tour.json` saying what each one shows. Notes, signs and terminals
//! get a view each, from where you'd stand to read them. Run it under
//! `scripts/tour.sh` and look through the pictures for odd models,
//! z-fighting, things floating or sunk, clipping, unreadable text.
//! Then the open ground (`g###_*`): boulders, rain hollows, forest floor,
//! verges, banks and the land round building pads.
//! `--tour-only TEXT` keeps the views whose file name contains TEXT.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use crate::launch::LaunchOptions;
use crate::render::FreeCamera;
use crate::world::Zone;

pub struct TourPlugin;

impl Plugin for TourPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tour>()
            .init_resource::<TourAt>()
            .add_systems(Update, tour);
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

/// The view the tour is holding the camera at, by file name (`None`
/// between views and once done): the benchmark measures it.
#[derive(Resource, Default)]
pub struct TourAt {
    pub view: Option<String>,
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
            | "funicular_car"
    )
}

/// Every placement's views.
/// Somewhere to stand: not in a tree or a rock.
struct Clearance {
    trees: Vec<Vec2>,
    rocks: Vec<(Vec2, f32)>,
}

impl Clearance {
    fn new(zone: &Zone) -> Self {
        Self {
            trees: extraction_core::forest::scatter_trees(&zone.map, &zone.hm)
                .iter()
                .map(|t| Vec2::new(t.pos[0], t.pos[2]))
                .collect(),
            rocks: extraction_core::rocks::scatter_rocks(&zone.map, &zone.hm)
                .iter()
                .map(|r| (Vec2::new(r.pos[0], r.pos[2]), r.radius))
                .collect(),
        }
    }

    fn blocked(&self, p: Vec2) -> bool {
        self.trees.iter().any(|t| t.distance(p) < 5.0)
            || self
                .rocks
                .iter()
                .any(|(q, r)| q.distance(p) < r * 1.4 + 1.2)
    }

    /// Nothing between `from` and `target`: no trunk within 3.5 m of the
    /// line (a spruce's skirt spreads that far at eye height), no rock.
    fn sight(&self, from: Vec2, target: Vec2) -> bool {
        let d = from.distance(target);
        let steps = (d / 1.0).ceil().max(1.0) as usize;
        (1..steps).all(|i| {
            let q = from.lerp(target, i as f32 / steps as f32);
            self.trees.iter().all(|t| t.distance(q) > 3.5)
                && self.rocks.iter().all(|(c, r)| c.distance(q) > r + 0.3)
        })
    }

    /// `from`, or the nearest clear spot walking round `target` at the
    /// same distance (first with a clear sight of it, then without).
    fn of(&self, from: Vec2, target: Vec2) -> Vec2 {
        let off = from - target;
        let round = |k: usize, scale: f32| {
            let a = (k as f32 * 0.5).ceil() * if k % 2 == 0 { 1.0 } else { -1.0 } * 0.4;
            target + Vec2::from_angle(a).rotate(off * scale)
        };
        // Walk round at the same distance, then further back (in a wood
        // the clear sight may only be from further off).
        [1.0, 1.5, 2.0, 2.5]
            .into_iter()
            .flat_map(|s| (0..16).map(move |k| (k, s)))
            .map(|(k, s)| round(k, s))
            .find(|p| !self.blocked(*p) && self.sight(*p, target))
            .or_else(|| (0..16).map(|k| round(k, 1.0)).find(|p| !self.blocked(*p)))
            .unwrap_or(from)
    }
}

pub fn views(zone: &Zone) -> Vec<View> {
    let mut out = Vec::new();
    let clear = Clearance::new(zone);
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
        let from = clear.of(from.xz(), Vec2::from(p.at));
        let from = Vec3::new(
            from.x,
            zone.ground(from.x, from.y) + 1.7 + f.y * 0.3,
            from.y,
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
                "ranger_tower" | "survey_tower" => 9.1,
                // The ore mill's top bay, at the back.
                "ore_mill" => 12.1,
                _ => 0.4,
            };
            let (cx, cz) = match p.prefab.as_str() {
                "ranger_tower" | "survey_tower" => (1.6, 1.6),
                "ore_mill" => (9.0, 19.0),
                // Inside its thick stone walls.
                "observatory" => (4.3, 3.8),
                _ => (f.x * 0.5 - 0.6, f.z * 0.5 - 0.6),
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
    out.extend(ground_views(zone, &clear));
    out
}

/// The open ground, from standing height: the biggest boulders on steep
/// ground, the most channel-like hollows (where the rain runs), the
/// forest floor, road and water edges, and the ring where the worn land
/// meets a building's pad.
fn ground_views(zone: &Zone, clear: &Clearance) -> Vec<View> {
    use extraction_core::geom;
    let hm = &zone.hm;
    let mut out = Vec::new();
    let stand = |p: Vec2| Vec3::new(p.x, zone.ground(p.x, p.y) + 1.7, p.y);
    let on = |p: Vec2, up: f32| Vec3::new(p.x, zone.ground(p.x, p.y) + up, p.y);
    let mut push = |kind: &str, what: String, from: Vec3, to: Vec3| {
        let n = out.len();
        out.push(View {
            file: format!("g{n:03}_{kind}.png"),
            what,
            at: from.to_array(),
            look: to.to_array(),
        });
    };
    let far_apart = |picked: &[Vec2], p: Vec2, d: f32| picked.iter().all(|q| q.distance(p) > d);
    // Boulders: the biggest, on the steepest ground, from downhill.
    let mut rocks = extraction_core::rocks::scatter_rocks(&zone.map, hm);
    rocks.sort_by(|a, b| {
        let score = |r: &extraction_core::rocks::RockSpot| {
            r.radius * (1.0 + hm.slope_deg([r.pos[0], r.pos[2]]) / 20.0)
        };
        score(b).total_cmp(&score(a))
    });
    let mut picked = Vec::new();
    for r in rocks.iter() {
        let p = Vec2::new(r.pos[0], r.pos[2]);
        if picked.len() == 4 || !far_apart(&picked, p, 60.0) {
            continue;
        }
        picked.push(p);
        let n = hm.normal([p.x, p.y]);
        let down = Vec2::new(n[0], n[2]).normalize_or(Vec2::X);
        push(
            "boulder",
            format!("boulder r {:.1} m at ({:.0}, {:.0})", r.radius, p.x, p.y),
            stand(clear.of(p + down * (r.radius + 7.0), p)),
            Vec3::from_array(r.pos),
        );
    }
    // Gullies: ground that dips across its fall line (a channel, not the
    // foot of a slope), on a moderate slope, away from roads, water and
    // building pads, looking down it.
    let half = zone.map.half() - zone.map.fence_inset.max(10.0) - 20.0;
    let off_road = |p: Vec2, by: f32| {
        hm.road_lines
            .iter()
            .all(|rl| geom::polyline_closest(&rl.points, [p.x, p.y]).0 > rl.width * 0.5 + by)
    };
    let dry = |p: Vec2, by: f32| {
        zone.map
            .water
            .iter()
            .all(|w| geom::polyline_closest(&w.points, [p.x, p.y]).0 > w.width * 0.5 + by)
    };
    let off_pads = |p: Vec2| {
        zone.map.placements.iter().all(|pl| {
            Vec2::from(pl.at).distance(p)
                > extraction_core::map::prefab_clearance(&pl.prefab) + 25.0
        })
    };
    let mut hollows = Vec::new();
    let mut z = -half;
    while z < half {
        let mut x = -half;
        while x < half {
            let p = Vec2::new(x, z);
            let slope = hm.slope_deg([x, z]);
            if (6.0..25.0).contains(&slope)
                && off_road(p, 12.0)
                && dry(p, 25.0)
                && off_pads(p)
                && !clear.blocked(p)
            {
                let n = hm.normal([x, z]);
                let down = Vec2::new(n[0], n[2]).normalize_or(Vec2::X);
                let across = down.perp() * 3.0;
                let h = |q: Vec2| hm.height([q.x, q.y]);
                let channel = h(p + across) + h(p - across) - 2.0 * h(p);
                hollows.push((channel, p));
            }
            x += 7.0;
        }
        z += 7.0;
    }
    hollows.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut picked = Vec::new();
    for (_, p) in hollows {
        if picked.len() == 3 {
            break;
        }
        if !far_apart(&picked, p, 80.0) {
            continue;
        }
        picked.push(p);
        let n = hm.normal([p.x, p.y]);
        let down = Vec2::new(n[0], n[2]).normalize_or(Vec2::X);
        push(
            "hollow",
            format!("rain hollow at ({:.0}, {:.0}), looking down it", p.x, p.y),
            stand(clear.of(p - down * 6.0, p)),
            on(p + down * 10.0, 0.0),
        );
    }
    // Forest floor: in the first three forests with room inside the fence,
    // the open spot off the roads nearest each one's middle.
    let mut forests = 0;
    for f in &zone.map.forests {
        if forests == 3 {
            break;
        }
        let (lo, hi) = geom::bounds(&f.polygon);
        let mid = Vec2::new((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5);
        let mut best: Option<Vec2> = None;
        let mut z = lo[1];
        while z < hi[1] {
            let mut x = lo[0];
            while x < hi[0] {
                let c = Vec2::new(x, z);
                let look = c + Vec2::new(8.0, 3.0);
                let inside = zone.map.half() - zone.map.fence_inset.max(10.0) - 10.0;
                if c.x.abs().max(c.y.abs()) < inside
                    && geom::point_in_polygon(&f.polygon, [c.x, c.y])
                    && off_road(c, 10.0)
                    && off_road(look, 6.0)
                    && dry(c, 8.0)
                    && !clear.blocked(c)
                    && clear.sight(c, look)
                    && best.is_none_or(|b| b.distance(mid) > c.distance(mid))
                {
                    best = Some(c);
                }
                x += 4.0;
            }
            z += 4.0;
        }
        let Some(c) = best else {
            continue;
        };
        forests += 1;
        push(
            "forest",
            format!("forest floor in \"{}\" at ({:.0}, {:.0})", f.name, c.x, c.y),
            stand(c),
            on(c + Vec2::new(8.0, 3.0), 0.3),
        );
    }
    // Road edges: along the first three roads, a third of the way, from
    // the verge.
    for rl in hm.road_lines.iter().take(3) {
        let len = geom::polyline_length(&rl.points);
        let (p, dir) = geom::polyline_at(&rl.points, len * 0.33);
        let (p, dir) = (Vec2::from(p), Vec2::from(dir).normalize_or(Vec2::X));
        let side = Vec2::new(-dir.y, dir.x) * (rl.width * 0.5 + 1.0);
        push(
            "verge",
            format!("road verge at ({:.0}, {:.0})", p.x, p.y),
            stand(clear.of(p + side, p + side + dir * 12.0)),
            on(p + side + dir * 12.0, 0.2),
        );
    }
    // Water's edge.
    for w in zone.map.water.iter().take(2) {
        let line = geom::catmull_rom(&w.points, 2.0);
        let len = geom::polyline_length(&line);
        let (p, dir) = geom::polyline_at(&line, len * 0.6);
        let (p, dir) = (Vec2::from(p), Vec2::from(dir).normalize_or(Vec2::X));
        // Stand on the bank, off any road or bridge, on whichever side
        // has room.
        let side = Vec2::new(-dir.y, dir.x);
        let from = [1.0, -1.0]
            .into_iter()
            .flat_map(|s| [4.0, 7.0, 10.0].map(|d| p + side * s * (w.width * 0.5 + d)))
            .find(|q| off_road(*q, 4.0) && hm.water_level([q.x, q.y]).is_none());
        let Some(from) = from else {
            continue;
        };
        push(
            "bank",
            format!("bank of \"{}\" at ({:.0}, {:.0})", w.name, p.x, p.y),
            stand(clear.of(from, p)),
            on(p, 0.0),
        );
    }
    // Where the worn land meets a building's settled ground.
    for pl in zone.map.placements.iter().step_by(7).take(3) {
        let r = extraction_core::map::prefab_clearance(&pl.prefab);
        let at = Vec2::from(pl.at);
        let from = at + Vec2::new(0.7, 0.7).normalize() * (r + 10.0);
        push(
            "pad",
            format!("ground round {} at ({:.0}, {:.0})", pl.prefab, at.x, at.y),
            stand(clear.of(from, at)),
            on(at, 0.5),
        );
    }
    out
}

/// `--tour-views FILE`: one view a line, `name x,y,z,yaw,pitch`, the
/// same numbers as `--camera` (y 0 stands you there, eyes 1.7 m up; `~h`
/// puts you h metres over the ground; yaw 0 looks toward -Z, 90 toward
/// +X). `#` starts a comment.
pub fn scripted_views(zone: &Zone, file: &std::path::Path) -> Vec<View> {
    let text = std::fs::read_to_string(file).unwrap_or_else(|e| {
        error!("tour: can't read {}: {e}", file.display());
        String::new()
    });
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_whitespace();
        let (Some(name), Some(cam)) = (parts.next(), parts.next()) else {
            continue;
        };
        // `~h` for the height: h metres over the ground; `=y` the world's
        // height as given, under the ground too (an underground site).
        let above = cam.split(',').nth(1).is_some_and(|y| y.starts_with('~'));
        let exact = cam.split(',').nth(1).is_some_and(|y| y.starts_with('='));
        let v: Vec<f32> = cam
            .split(',')
            .filter_map(|s| s.trim_start_matches(['~', '=']).parse().ok())
            .collect();
        if v.len() < 5 {
            warn!("tour: bad view line {line:?}");
            continue;
        }
        let ground = zone.ground(v[0], v[2]);
        let y = if exact {
            v[1]
        } else if above {
            ground + v[1].max(0.3)
        } else if v[1] <= 0.0 {
            ground + 1.7
        } else {
            v[1].max(ground + 0.3)
        };
        let at = Vec3::new(v[0], y, v[2]);
        let rot = Quat::from_euler(EulerRot::YXZ, -v[3].to_radians(), v[4].to_radians(), 0.0);
        out.push(View {
            file: format!("{name}.png"),
            what: line.to_string(),
            at: at.to_array(),
            look: (at + rot * Vec3::NEG_Z * 10.0).to_array(),
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
    mut at: ResMut<TourAt>,
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
        let mut v = match &opts.tour_views {
            Some(file) => scripted_views(&zone, file),
            None => views(&zone),
        };
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
        at.view = None;
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
        at.view = None;
        return;
    }
    if at.view.as_ref() != Some(&view.file) {
        at.view = Some(view.file.clone());
    }
    state.wait += 1;
    // Ground views wait for the grass round them to be built.
    let settle = if view.file.starts_with('g') {
        SETTLE * 4
    } else {
        SETTLE
    };
    // The benchmark holds each view longer, for steadier timings.
    let settle = settle.max(crate::render::budget::hold());
    if state.wait < settle {
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
