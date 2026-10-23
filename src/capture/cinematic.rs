//! `--cinematic FILE`: flies the free camera along each shot's spline
//! (`capture::path`) and saves every frame to
//! `<capture-out>/<shot>/f00000.png`, with a `cine.json` saying what was
//! shot. Time steps a fixed 1/fps a frame however slow the renderer is,
//! and pauses while frames are being read back, so rain, trees and robots
//! move at their real speed in the footage. Under `xvfb-run` with the
//! software Vulkan driver it runs at a few seconds a frame;
//! `scripts/capture_cinematic.sh` runs it and encodes the frames.
//!
//! A shot's settings (`shot NAME key=value ...`):
//!
//! | key | meaning |
//! | - | - |
//! | `dur` | seconds (default 8) |
//! | `ease` | share of the time spent speeding up and slowing down at each end (default 0.25; 0 is linear) |
//! | `fov` | vertical field of view, degrees (default 55) |
//! | `pitch` | down-tilt when the shot has no `look` points (default 6) |
//! | `clear` | fog visibility times this, on the shot's camera only (aerials: 2.5) |
//! | `hour`, `weather` | as `--hour` and `--weather`, for this shot (others keep the launch's) |
//! | `settle` | frames to let the scene settle at the first pose before recording (default 40) |
//! | `gen` | make the key points instead of listing them: `orbit`, `road`, `river`, `rise` (see [`generate`]) |
//!
//! Keys that would put the camera under the ground are lifted to
//! `CLEARANCE` above it, all along the path.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use std::time::Duration;

use super::path::{self, Flight, Height, Point, ShotSpec};
use crate::launch::LaunchOptions;
use crate::render::FreeCamera;
use crate::world::Zone;

pub struct CinematicPlugin;

impl Plugin for CinematicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cine>()
            .add_systems(Update, fly.run_if(cinematic_run))
            .add_systems(
                Update,
                dump.run_if(|o: Res<LaunchOptions>| o.capture.dump.is_some()),
            );
    }
}

/// `--cine-dump IN.cine,OUT.stills`: each shot's first pose, resolved
/// against this valley (targets, ground, trees), written as a still
/// (`name x,~h,z,yaw,pitch`), then exit. The visual diff's cameras are
/// made this way once and committed, so every build after (and before)
/// shoots from the same places: a build that predates a target kind still
/// reads a `.stills` file.
fn dump(
    opts: Res<LaunchOptions>,
    zone: Option<Res<Zone>>,
    mut exit: MessageWriter<AppExit>,
    mut done: Local<bool>,
) {
    let (Some(zone), Some((src, dst)), false) = (zone, opts.capture.dump.as_ref(), *done) else {
        return;
    };
    *done = true;
    let lines = match load(&zone, src, 24) {
        Ok(shots) => shots
            .iter()
            .map(|s| {
                let (eye, target) = s.flight.at(0.0);
                let d = target - eye;
                let yaw = d.x.atan2(-d.z).to_degrees().rem_euclid(360.0);
                let pitch = d.y.atan2(Vec2::new(d.x, d.z).length()).to_degrees();
                let h = eye.y - zone.ground(eye.x, eye.z);
                format!(
                    "{:<16} {:.1},~{:.2},{:.1},{:.1},{:.1}",
                    s.name, eye.x, h, eye.z, yaw, pitch
                )
            })
            .collect::<Vec<_>>(),
        Err(e) => {
            error!("cine-dump: {e}");
            exit.write(AppExit::error());
            return;
        }
    };
    let text = format!(
        "# {}: {} resolved by --cine-dump; name x,~h,z,yaw,pitch.\n{}\n",
        opts.map,
        src.display(),
        lines.join("\n")
    );
    match std::fs::write(dst, text) {
        Ok(()) => info!("cine-dump: {} stills to {}", lines.len(), dst.display()),
        Err(e) => error!("cine-dump: {}: {e}", dst.display()),
    }
    exit.write(AppExit::Success);
}

fn cinematic_run(opts: Res<LaunchOptions>) -> bool {
    opts.capture.cinematic.is_some()
}

/// A still's field of view (the game's own camera's), degrees.
pub const STILL_FOV: f32 = 75.0;
/// A still's last settle frames are held with the clock stopped. Whatever
/// keeps time (clouds, a lamp's character) holds for the picture, so one
/// build's still is the same every run, and a lamp with a character is
/// shot lit (a paused clock lights it: `render::landmarks`). Without the
/// hold, a still caught whatever phase its frame landed on, and a
/// blinking mast lamp showed as a change between builds.
pub const STILL_HOLD: u32 = 2;
/// Frames a still settles for at its pose (streamed detail, shadows).
pub const STILL_SETTLE: u32 = 14;

/// The lowest the camera goes over the ground, metres.
pub const CLEARANCE: f32 = 0.6;
/// Frames being read back at once, at most.
const IN_FLIGHT: usize = 2;

/// A shot ready to fly.
#[derive(Clone, Debug)]
pub struct Shot {
    pub name: String,
    pub flight: Flight,
    pub frames: u32,
    pub fov: f32,
    /// Fog visibility times this for the shot (`clear=`, default 1).
    pub clear: f32,
    pub hour: Option<String>,
    pub weather: Option<String>,
    pub settle: u32,
    /// Share of the path inside trees, and of its views a trunk blocks
    /// (`Trees::score`).
    pub trees: (f32, f32),
    /// A still (from a `.stills` file): one frame, saved as
    /// `<capture-out>/<name>.png`.
    pub still: bool,
}

#[derive(Debug, serde::Serialize)]
struct ShotRecord {
    name: String,
    frames: u32,
    fps: u32,
    dir: String,
    hour: Option<String>,
    weather: Option<String>,
    start: [f32; 3],
    end: [f32; 3],
    length_m: f32,
}

#[derive(Resource, Default)]
struct Cine {
    shots: Option<Vec<Shot>>,
    cur: usize,
    /// Frames of the current shot asked for.
    frame: u32,
    /// Frames left to settle before the current shot records.
    settle: Option<u32>,
    /// A still's last frames, held with the clock stopped (`STILL_HOLD`).
    held: bool,
    in_flight: usize,
    records: Vec<ShotRecord>,
    started: Option<std::time::Instant>,
    /// The launch's `--hour` and `--weather`, for shots that set neither.
    launch: Option<(Option<String>, Option<String>)>,
}

/// Reads the shots of a `.cine` file and makes them flyable in `zone`.
/// A `.stills` file (the `--tour-views` format, `name x,y,z,yaw,pitch`)
/// gives one still a line instead: time still steps a frame at a time, so
/// a look that a real-time tour would outlast (the Surge, which passes in
/// a few minutes) holds for every still.
pub fn load(zone: &Zone, file: &std::path::Path, fps: u32) -> Result<Vec<Shot>, String> {
    if file.extension().is_some_and(|e| e == "stills") {
        let trees = Trees::new(zone);
        return Ok(crate::tour::scripted_views(zone, file)
            .into_iter()
            .map(|v| {
                let (at, look) = (Vec3::from_array(v.at), Vec3::from_array(v.look));
                let flight = Flight::new(vec![(at, Some(look)), (at, Some(look))], 0.0, 0.0);
                Shot {
                    name: v.file.trim_end_matches(".png").to_string(),
                    trees: trees.score(&flight),
                    flight,
                    frames: 1,
                    fov: STILL_FOV,
                    clear: 1.0,
                    hour: None,
                    weather: None,
                    settle: STILL_SETTLE,
                    still: true,
                }
            })
            .collect());
    }
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let specs = path::parse(&text)?;
    let trees = Trees::new(zone);
    specs.iter().map(|s| build(zone, &trees, s, fps)).collect()
}

/// A shot from its spec: key points (given or generated) resolved against
/// the ground.
///
/// A generated shot whose path would pass through trees (or whose view a
/// trunk would block) is tried wider, tighter and higher ([`DODGES`]) and
/// the clearest kept; key points written out are flown as written (the
/// path files' test holds them clear).
pub fn build(zone: &Zone, trees: &Trees, spec: &ShotSpec, fps: u32) -> Result<Shot, String> {
    let ground = |x: f32, z: f32| zone.ground(x, z);
    let fly = |keys: Vec<(Point, Option<Point>)>| -> Result<Flight, String> {
        if keys.len() < 2 {
            return Err(format!("shot {}: needs two key points", spec.name));
        }
        let keys: Vec<(Vec3, Option<Vec3>)> = keys
            .into_iter()
            .map(|(at, look)| {
                let mut p = at.resolve(&ground);
                p.y = p.y.max(ground(p.x, p.z) + CLEARANCE);
                (p, look.map(|l| l.resolve(&ground)))
            })
            .collect();
        Ok(Flight::new(
            keys,
            spec.num("ease", 0.25),
            spec.num("pitch", 6.0),
        ))
    };
    let flight = match spec.get("gen") {
        None => fly(spec.keys.clone())?,
        Some(kind) => {
            let mut best: Option<(f32, Flight)> = None;
            for (scale, lift) in DODGES {
                let mut s = spec.clone();
                s.opts.push(("dodge_scale".into(), scale.to_string()));
                s.opts.push(("dodge_lift".into(), lift.to_string()));
                let f = fly(generate(zone, kind, &s)?)?;
                let (inside, blocked) = trees.score(&f);
                // A little height or width costs a little: prefer the
                // shot as asked when it's clear enough.
                let cost = inside * 4.0 + blocked + lift * 0.004 + (scale - 1.0).abs() * 0.02;
                if best.as_ref().is_none_or(|b| cost < b.0) {
                    best = Some((cost, f));
                }
                if inside == 0.0 && blocked < 0.05 {
                    break;
                }
            }
            best.unwrap().1
        }
    };
    let dur = spec.num("dur", 8.0).max(0.5);
    Ok(Shot {
        name: spec.name.clone(),
        trees: trees.score(&flight),
        flight,
        frames: (dur * fps as f32).round() as u32,
        fov: spec.num("fov", 55.0),
        clear: spec.num("clear", 1.0),
        hour: spec.get("hour").map(str::to_string),
        weather: spec.get("weather").map(str::to_string),
        settle: spec.num("settle", 40.0) as u32,
        still: false,
    })
}

/// Where a generated shot is about: a prefab's first placement, a marker's
/// name (any case, `_` for spaces), `x,z`, or, the same words for every
/// valley: `kind:K` (the first marker of kind K, `kind:Anomaly`), `tallest`
/// (the tallest placed prefab) or `interior` (the first placed building
/// with furniture inside).
pub fn find_target(zone: &Zone, name: &str) -> Option<(Vec2, f32, Vec3)> {
    if let Some((x, z)) = name.split_once(',') {
        return Some((
            Vec2::new(x.parse().ok()?, z.parse().ok()?),
            0.0,
            Vec3::splat(4.0),
        ));
    }
    let footprint =
        |p: &extraction_core::map::Placement| crate::world::structures::footprint(&p.prefab);
    if let Some(kind) = name.strip_prefix("kind:") {
        return zone
            .map
            .markers
            .iter()
            .find(|m| {
                // The variant's name, without its fields (`Anomaly { .. }`).
                let k = format!("{:?}", m.kind);
                let name = k.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("");
                name.eq_ignore_ascii_case(kind)
            })
            .map(|m| (Vec2::from(m.at), m.rot, Vec3::splat(4.0)));
    }
    if name == "tallest" {
        return zone
            .map
            .placements
            .iter()
            .max_by(|a, b| footprint(a).y.total_cmp(&footprint(b).y))
            .map(|p| (Vec2::from(p.at), p.rot, footprint(p)));
    }
    if name == "interior" {
        return zone
            .map
            .placements
            .iter()
            .find(|p| {
                crate::world::buildings::prefab(&p.prefab)
                    .is_some_and(|g| g.props.iter().any(|d| d.inside))
            })
            .map(|p| (Vec2::from(p.at), p.rot, footprint(p)));
    }
    if let Some(p) = zone.map.placements.iter().find(|p| p.prefab == name) {
        let f = crate::world::structures::footprint(&p.prefab);
        return Some((Vec2::from(p.at), p.rot, f));
    }
    let norm = |s: &str| s.to_lowercase().replace(' ', "_");
    zone.map
        .markers
        .iter()
        .find(|m| norm(&m.name) == norm(name))
        .map(|m| (Vec2::from(m.at), m.rot, Vec3::splat(4.0)))
}

/// Key points made from the map:
///
/// - `orbit target=T [radius=R] [height=H] [turns=F] [start=DEG] [look=L]`:
///   an arc round T (a prefab, marker or `x,z`), H above the ground,
///   looking at T's middle (L metres up).
/// - `road [index=I] [from=A] [to=B] [height=H] [side=S]`: along road I
///   from fraction A to B of its length, H up and S to the side, looking
///   along the way.
/// - `river [index=I] ...`: the same along a stream.
/// - `rise target=T [from=DEG] [dist=D] [low=H0] [high=H1]`: a crane shot:
///   D from T, rising from H0 to H1 while looking at it.
pub fn generate(
    zone: &Zone,
    kind: &str,
    s: &ShotSpec,
) -> Result<Vec<(Point, Option<Point>)>, String> {
    use extraction_core::geom;
    let above = |x: f32, h: f32, z: f32| Point {
        x,
        y: Height::Above(h),
        z,
    };
    // Set by `build` when it tries the shot again out of the trees.
    let (scale, lift) = (s.num("dodge_scale", 1.0), s.num("dodge_lift", 0.0));
    let target = || {
        let name = s.get("target").unwrap_or("");
        find_target(zone, name).ok_or_else(|| format!("shot {}: no target {name:?}", s.name))
    };
    match kind {
        "orbit" => {
            let (c, rot, f) = target()?;
            let radius = s.num("radius", f.x.max(f.z) * 1.2 + 14.0) * scale;
            let height = s.num("height", f.y + 6.0) + lift;
            let turns = s.num("turns", 0.3);
            let start = s.num("start", 180.0 + rot);
            let lift = s.num("look", f.y * 0.4);
            let n = ((turns.abs() * 16.0).ceil() as usize).max(2);
            Ok((0..=n)
                .map(|i| {
                    let a = (start + 360.0 * turns * i as f32 / n as f32).to_radians();
                    // Compass angle: 0 is north (-Z), clockwise.
                    let p = c + Vec2::new(a.sin(), -a.cos()) * radius;
                    (above(p.x, height, p.y), Some(above(c.x, lift, c.y)))
                })
                .collect())
        }
        "road" | "river" => {
            let i = s.num("index", 0.0) as usize;
            let line: Vec<[f32; 2]> = if kind == "road" {
                zone.hm
                    .road_lines
                    .get(i)
                    .map(|r| r.points.clone())
                    .ok_or_else(|| format!("shot {}: no road {i}", s.name))?
            } else {
                zone.map
                    .water
                    .get(i)
                    .map(|w| geom::catmull_rom(&w.points, 2.0))
                    .ok_or_else(|| format!("shot {}: no stream {i}", s.name))?
            };
            let len = geom::polyline_length(&line);
            let (a, b) = (s.num("from", 0.0), s.num("to", 1.0));
            let h = s.num("height", 6.0) + lift;
            // Out of the trees: to one side of the line, then the other.
            let side = s.num("side", 0.0) + (scale - 1.0) * 12.0;
            let n = (((b - a).abs() * len / 25.0).ceil() as usize).max(2);
            Ok((0..=n)
                .map(|k| {
                    let (p, dir) =
                        geom::polyline_at(&line, len * (a + (b - a) * k as f32 / n as f32));
                    let dir = Vec2::from(dir) * (b - a).signum();
                    let p = Vec2::from(p) + dir.perp() * side;
                    (above(p.x, h, p.y), None)
                })
                .collect())
        }
        "rise" => {
            let (c, rot, f) = target()?;
            let a = s.num("from", 180.0 + rot).to_radians();
            let dist = s.num("dist", f.x.max(f.z) + 18.0) * scale;
            let p = c + Vec2::new(a.sin(), -a.cos()) * dist;
            let (lo, hi) = (s.num("low", 1.6) + lift, s.num("high", f.y + 25.0) + lift);
            let lift = s.num("look", f.y * 0.4);
            let n = 4;
            Ok((0..=n)
                .map(|k| {
                    let t = k as f32 / n as f32;
                    // Pull back a little as it rises, to keep it in frame.
                    let q = p + (p - c).normalize_or(Vec2::X) * dist * 0.3 * t;
                    (
                        above(q.x, lo + (hi - lo) * t, q.y),
                        Some(above(c.x, lift, c.y)),
                    )
                })
                .collect())
        }
        other => Err(format!("shot {}: unknown generator {other:?}", s.name)),
    }
}

/// The valley's trees as the camera must miss them: a trunk and crown
/// within `CROWN` metres sideways, up to `CANOPY` (times the tree's scale)
/// over its foot. Bucketed by 16 m cells.
pub struct Trees {
    cells: std::collections::HashMap<(i32, i32), Vec<(Vec2, f32, f32)>>,
}

/// How far from a trunk the camera keeps, metres.
pub const CROWN: f32 = 3.2;
/// How tall a tree of scale 1 is taken to be, metres.
pub const CANOPY: f32 = 24.0;
const CELL: f32 = 16.0;

impl Trees {
    pub fn new(zone: &Zone) -> Self {
        let mut cells: std::collections::HashMap<(i32, i32), Vec<(Vec2, f32, f32)>> =
            Default::default();
        for t in extraction_core::forest::scatter_trees(&zone.map, &zone.hm) {
            let p = Vec2::new(t.pos[0], t.pos[2]);
            cells
                .entry(((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32))
                .or_default()
                .push((p, t.pos[1], t.pos[1] + CANOPY * t.scale));
        }
        Self { cells }
    }

    /// Whether a point is in a tree, within `margin` metres sideways.
    pub fn hits(&self, p: Vec3, margin: f32) -> bool {
        let (cx, cz) = ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32);
        (cx - 1..=cx + 1).any(|x| {
            (cz - 1..=cz + 1).any(|z| {
                self.cells.get(&(x, z)).is_some_and(|ts| {
                    ts.iter().any(|(q, foot, top)| {
                        p.y > *foot && p.y < *top && q.distance(Vec2::new(p.x, p.z)) < margin
                    })
                })
            })
        })
    }

    /// How much of a shot is spent in trees (camera within [`CROWN`] of a
    /// trunk), and how often its view is blocked by one near the camera
    /// (a trunk within 1.2 m of the line to what it looks at, in its first
    /// 10 m, where it would fill the frame; further off it's foreground),
    /// each as a share of samples.
    pub fn score(&self, flight: &Flight) -> (f32, f32) {
        let n = (flight.path.length() / 0.5).ceil().max(8.0) as usize;
        let mut inside = 0;
        let mut blocked = 0;
        for i in 0..=n {
            let (eye, at) = flight.at(i as f32 / n as f32);
            if self.hits(eye, CROWN) {
                inside += 1;
            }
            let dir = (at - eye).normalize_or(Vec3::NEG_Z);
            let reach = eye.distance(at).min(10.0);
            if (1..=(reach / 1.0) as usize).any(|k| self.hits(eye + dir * k as f32, 1.2)) {
                blocked += 1;
            }
        }
        (
            inside as f32 / (n + 1) as f32,
            blocked as f32 / (n + 1) as f32,
        )
    }
}

/// A generated shot's settings tried in turn until its path keeps out of
/// the trees: (radius or side offset factor, extra height).
const DODGES: [(f32, f32); 10] = [
    (1.0, 0.0),
    (0.85, 0.0),
    (1.2, 0.0),
    (0.7, 0.0),
    (1.4, 0.0),
    (1.0, 6.0),
    (1.2, 6.0),
    (0.85, 10.0),
    (1.4, 14.0),
    (1.0, 26.0),
];

#[allow(clippy::too_many_arguments)]
fn fly(
    mut commands: Commands,
    mut opts: ResMut<LaunchOptions>,
    zone: Option<Res<Zone>>,
    mut cine: ResMut<Cine>,
    mut cams: Query<(Entity, &mut Transform, &mut Projection), With<FreeCamera>>,
    mut time: ResMut<Time<Virtual>>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(zone) = zone else {
        return;
    };
    let fps = opts.capture.fps();
    *frames += 1;
    if *frames == 1 {
        commands.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f64(1.0 / fps as f64),
        ));
    }
    let out = opts.capture.out_dir();
    let cine = &mut *cine;
    if cine.shots.is_none() {
        let file = opts.capture.cinematic.clone().unwrap_or_default();
        let mut shots = match load(&zone, &file, fps) {
            Ok(s) => s,
            Err(e) => {
                error!("cinematic: {e}");
                exit.write(AppExit::error());
                return;
            }
        };
        if let Some(only) = &opts.capture.only {
            shots.retain(|s| s.name.contains(only.as_str()));
        }
        for s in &shots {
            if s.trees.0 > 0.0 || s.trees.1 > 0.1 {
                warn!(
                    "cinematic: {}: {:.0}% of it in trees, {:.0}% of its views blocked by a trunk",
                    s.name,
                    s.trees.0 * 100.0,
                    s.trees.1 * 100.0
                );
            }
        }
        info!(
            "cinematic: {} shots, {} frames at {fps} fps",
            shots.len(),
            shots.iter().map(|s| s.frames).sum::<u32>()
        );
        cine.shots = Some(shots);
        cine.started = Some(std::time::Instant::now());
    }
    let Ok((cam_e, mut cam, mut proj)) = cams.single_mut() else {
        return;
    };
    // Frames in flight are waited out with time stopped (see below), so
    // each saved frame is exactly one step on from the last. Unpausing
    // takes effect next frame: this one is a repeat.
    if time.is_paused() && !cine.held {
        if cine.in_flight < IN_FLIGHT {
            time.unpause();
        }
        return;
    }
    let shots = cine.shots.as_ref().unwrap();
    let Some(shot) = shots.get(cine.cur).cloned() else {
        if cine.in_flight == 0 {
            let _ = std::fs::write(
                out.join("cine.json"),
                serde_json::to_string_pretty(&cine.records).unwrap_or_default(),
            );
            info!(
                "cinematic: done in {:.0} s",
                cine.started.map_or(0.0, |t| t.elapsed().as_secs_f32())
            );
            exit.write(AppExit::Success);
        }
        return;
    };
    let preview = opts.capture.preview;
    let total = preview.unwrap_or(shot.frames).max(1);
    let x = |frame: u32| {
        if preview.is_some() {
            frame as f32 / (total - 1).max(1) as f32
        } else {
            frame as f32 / shot.frames.max(1) as f32
        }
    };
    let place = |cam: &mut Transform, frame: u32| {
        let (eye, at) = shot.flight.at(x(frame));
        let eye = Vec3::new(
            eye.x,
            eye.y.max(zone.ground(eye.x, eye.z) + CLEARANCE),
            eye.z,
        );
        *cam = Transform::from_translation(eye).looking_at(at, Vec3::Y);
    };
    // A new shot: its light and weather, its lens, and time to settle at
    // its first pose (the first shot's is the launch's `--frames`).
    let settle = match cine.settle {
        Some(n) => n,
        None => {
            // The shot's hour and weather, or the launch's.
            let (hour, weather) = cine
                .launch
                .get_or_insert_with(|| (opts.hour.clone(), opts.weather.clone()))
                .clone();
            opts.hour = shot.hour.clone().or(hour);
            opts.weather = shot.weather.clone().or(weather);
            if let Projection::Perspective(p) = &mut *proj {
                p.fov = shot.fov.to_radians();
            }
            commands.entity(cam_e).insert((
                super::ClearAir(shot.clear),
                super::Lens(shot.fov.to_radians()),
            ));
            let n = if cine.cur == 0 {
                opts.frames.max(shot.settle)
            } else {
                shot.settle
            };
            cine.settle = Some(n);
            let _ = std::fs::create_dir_all(if shot.still {
                out.clone()
            } else {
                out.join(&shot.name)
            });
            n
        }
    };
    if settle > 0 {
        place(&mut cam, 0);
        if shot.still && settle <= STILL_HOLD && !time.is_paused() {
            time.pause();
            cine.held = true;
        }
        cine.settle = Some(settle - 1);
        return;
    }
    if cine.frame >= total {
        cine.records.push(ShotRecord {
            name: shot.name.clone(),
            frames: total,
            fps,
            dir: shot.name.clone(),
            hour: opts.hour.clone(),
            weather: opts.weather.clone(),
            start: shot.flight.at(0.0).0.to_array(),
            end: shot.flight.at(1.0).0.to_array(),
            length_m: shot.flight.path.length(),
        });
        info!("cinematic: {} done ({total} frames)", shot.name);
        cine.cur += 1;
        cine.frame = 0;
        cine.settle = None;
        cine.held = false;
        return;
    }
    let frame = cine.frame;
    place(&mut cam, frame);
    // In a preview each still gets a few frames to settle at its pose.
    if preview.is_some() && *frames % 6 != 0 {
        return;
    }
    cine.frame += 1;
    cine.in_flight += 1;
    if cine.in_flight >= IN_FLIGHT {
        time.pause();
    }
    let file = if shot.still {
        out.join(format!("{}.png", shot.name))
    } else {
        out.join(&shot.name).join(format!("f{frame:05}.png"))
    };
    let mut save = save_to_disk(file);
    commands.spawn(Screenshot::primary_window()).observe(
        move |shot: On<ScreenshotCaptured>, mut cine: ResMut<Cine>| {
            save(shot);
            cine.in_flight = cine.in_flight.saturating_sub(1);
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shot in `scripts/capture_paths/` flies clear of the trees and
    /// mostly sees past them: a camera through a spruce's crown is a shot
    /// of needles (as the first sawmill orbit was).
    #[test]
    fn path_files_fly_clear_of_trees() {
        let dir = crate::asset_root_path().join("../scripts/capture_paths");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return; // a packaged build has no scripts/
        };
        let mut bad = Vec::new();
        let mut checked = 0;
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_none_or(|x| x != "cine") {
                continue;
            }
            let map = p.file_stem().unwrap().to_string_lossy().into_owned();
            let zone = Zone::load(&map);
            let shots = load(&zone, &p, 24).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            for s in shots {
                checked += 1;
                println!(
                    "{map} {}: {:.0}% in trees, {:.0}% blocked, {:.0} m",
                    s.name,
                    s.trees.0 * 100.0,
                    s.trees.1 * 100.0,
                    s.flight.path.length()
                );
                if s.trees.0 > 0.0 || s.trees.1 > 0.15 {
                    bad.push(format!(
                        "{map} {}: {:.0}% in trees, {:.0}% of views blocked",
                        s.name,
                        s.trees.0 * 100.0,
                        s.trees.1 * 100.0
                    ));
                }
            }
        }
        assert!(checked >= 12, "only {checked} shots found in {dir:?}");
        assert!(bad.is_empty(), "shots into the trees:\n{}", bad.join("\n"));
    }

    /// Generated shots round a prefab come out where they should.
    #[test]
    fn orbits_circle_their_target() {
        let zone = Zone::load("marrow_creek");
        let spec = path::parse("shot o gen=orbit target=sawmill radius=40 height=10 turns=0.5")
            .unwrap()
            .remove(0);
        let keys = generate(&zone, "orbit", &spec).unwrap();
        let (c, _, _) = find_target(&zone, "sawmill").unwrap();
        for (at, look) in keys {
            let d = Vec2::new(at.x, at.z).distance(c);
            assert!((d - 40.0).abs() < 0.01, "{d}");
            let look = look.unwrap();
            assert!(Vec2::new(look.x, look.z).distance(c) < 0.01);
        }
    }

    /// The visual diff's views (`scripts/visual_diff/views.cine`) name
    /// their targets in words every valley answers to.
    #[test]
    fn generic_targets_resolve_in_every_valley() {
        for map in [
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "sallow_reservoir",
            "rime_fell",
        ] {
            let zone = Zone::load(map);
            for t in [
                "kind:Insertion",
                "kind:Rendezvous",
                "kind:Anomaly",
                "tallest",
                "interior",
            ] {
                assert!(find_target(&zone, t).is_some(), "{map}: no {t}");
            }
        }
        let path = crate::asset_root_path().join("../scripts/visual_diff/views.cine");
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(path::parse(&text).unwrap().len(), 5);
    }
}
