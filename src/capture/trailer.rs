//! `--trailer`: a sim bot's raid (`--sim-run`), rendered, with two
//! cameras on it: a chase camera over its shoulder (the window) and a
//! drone's-eye view circling high above (a texture). Time steps exactly as
//! in a headless sim (one 1/90 s fixed step a frame), so the raid plays as
//! the bots' do; the cameras only render while something is worth seeing
//! (leaving the truck, a robot near, a fight, an anomaly, the truck called
//! and the ride out, going down, and a stretch of walking now and then),
//! and the frames between cost little. Each recorded frame goes to
//! `<capture-out>/chase/f00000.png` and `<capture-out>/drone/f00000.png`;
//! `trailer.json` lists the clips (frames, raid time, why); the run's own
//! telemetry is written as any sim run's. `scripts/capture_trailer.sh`
//! plays a raid and `scripts/capture_trailer.py` cuts the trailer.

use avian3d::prelude::*;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use crate::launch::LaunchOptions;
use crate::raid::{Phase, Raid};
use crate::render::FreeCamera;
use crate::rig::{LocalPlayer, RigHead};

pub struct TrailerPlugin;

impl Plugin for TrailerPlugin {
    fn build(&self, app: &mut App) {
        let on = app
            .world()
            .get_resource::<LaunchOptions>()
            .is_some_and(|o| o.capture.trailer.is_some());
        if !on {
            return;
        }
        app.init_resource::<Director>()
            .insert_resource(bevy::time::TimeUpdateStrategy::FixedTimesteps(1))
            .add_systems(
                Update,
                (direct, aim).chain().after(crate::sim::raider::think),
            )
            .add_systems(Update, (show_body, dress_body, quiet_head));
    }
}

/// Raid seconds per saved frame: 30 frames a second of raid.
const STEP: f32 = 1.0 / 30.0;
/// How long a reason keeps the cameras rolling after it ends, seconds.
const TAIL: f32 = 3.0;
/// Seconds the cameras render before anything is kept: the valley streams
/// its detail in round them (unfilmed frames draw nothing).
const WARMUP: f32 = 6.0;
/// The share of the budget walking may take.
const WALK_SHARE: f32 = 0.35;
/// The drone films only while less than this share of its sight line to
/// the bot is in tree crowns.
const DRONE_BLOCKED: f32 = 0.2;
/// Shortest clip worth keeping, seconds.
const MIN_CLIP: f32 = 2.5;

#[derive(Clone, Debug, serde::Serialize)]
struct Clip {
    reason: String,
    /// Raid seconds.
    start: f32,
    end: f32,
    /// Saved frames [first, last).
    first: u32,
    last: u32,
}

#[derive(Resource, Default)]
struct Director {
    t: f32,
    next_frame: f32,
    frame: u32,
    clips: Vec<Clip>,
    open: Option<Clip>,
    /// Until when each reason holds the cameras on (raid seconds).
    hold: Vec<(String, f32)>,
    budget: f32,
    used: f32,
    /// The drone camera's picture, and its camera.
    drone: Option<(Entity, Handle<Image>)>,
    /// The cameras' eased positions.
    chase_at: Option<Vec3>,
    drone_at: Option<Vec3>,
    drone_angle: f32,
    last_walk: f32,
    in_flight: usize,
    done: bool,
    /// Seconds to the drone's next look round for a clearer sight.
    look_round: f32,
    /// The share of the drone's sight line to the bot in tree crowns.
    drone_blocked: f32,
    /// Seconds to the next position line in the log (`RUST_LOG=...=debug`).
    log_in: f32,
}

impl Director {
    fn want(&mut self, reason: &str, until: f32) {
        match self.hold.iter_mut().find(|(r, _)| r == reason) {
            Some(h) => h.1 = h.1.max(until),
            None => self.hold.push((reason.to_string(), until)),
        }
    }

    /// Opens, extends or closes the current clip for `reason` at raid time
    /// `t`. A clip runs at least `MIN_CLIP`, whatever happens; after that a
    /// new reason cuts to a new clip and none ends it. True when a clip
    /// closed.
    fn advance(&mut self, t: f32, reason: Option<String>) -> bool {
        let young = self.open.as_ref().is_some_and(|c| t - c.start < MIN_CLIP);
        let same = matches!((&self.open, &reason), (Some(c), Some(r)) if c.reason == *r);
        if young || same {
            if let Some(c) = &mut self.open {
                c.end = t;
            }
            return false;
        }
        let closed = self.open.take().map(|c| self.clips.push(c)).is_some();
        let first = self.frame;
        self.open = reason.map(|reason| Clip {
            reason,
            start: t,
            end: t,
            first,
            last: first,
        });
        closed
    }

    /// The reason the cameras roll now, if any: the most telling one held.
    fn reason(&self) -> Option<String> {
        const RANK: [&str; 8] = [
            "down", "fight", "extract", "anomaly", "robot", "leaving", "truck", "walk",
        ];
        RANK.iter()
            .find(|r| {
                self.hold
                    .iter()
                    .any(|(h, until)| h == *r && *until > self.t)
            })
            .map(|r| r.to_string())
    }
}

#[allow(clippy::too_many_arguments)]
fn direct(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    time: Res<Time>,
    raid: Res<Raid>,
    mut dir: ResMut<Director>,
    players: Query<(&Transform, Option<&crate::sim::raider::Raider>), With<LocalPlayer>>,
    robots: Query<(&crate::robots::Robot, &GlobalTransform)>,
    anomalies: Query<(&crate::anomaly::Anomaly, &GlobalTransform)>,
    mut cams: Query<&mut Camera, With<FreeCamera>>,
    mut drone_cams: Query<&mut Camera, Without<FreeCamera>>,
    windows: Query<&Window>,
    mut images: ResMut<Assets<Image>>,
    log: Option<Res<crate::sim::telemetry::SimLog>>,
    mut exit: MessageWriter<AppExit>,
) {
    let d = &mut *dir;
    if d.done {
        return;
    }
    let out = opts.capture.out_dir();
    if d.drone.is_none() {
        let Some(size) = windows.iter().next().map(|w| w.physical_size()) else {
            return;
        };
        let image = images.add(Image::new_target_texture(
            size.x,
            size.y,
            TextureFormat::Bgra8UnormSrgb,
            None,
        ));
        let weather = crate::world::sky::Weather::default();
        let cam = commands
            .spawn((
                Name::new("Drone camera"),
                crate::render::OffscreenCamera,
                DroneCamera,
                super::ClearAir(2.5),
                crate::render::camera_bundle(&weather),
                Camera {
                    order: -1,
                    is_active: false,
                    ..default()
                },
                RenderTarget::Image(image.clone().into()),
                Transform::from_xyz(0.0, 100.0, 0.0),
            ))
            .id();
        d.drone = Some((cam, image));
        // The chase camera, in the window. (No `--camera`: that would place
        // the bot where it stands rather than in the truck.)
        commands.spawn((
            Name::new("Chase camera"),
            FreeCamera,
            crate::render::camera_bundle(&weather),
            Transform::from_xyz(0.0, 100.0, 0.0),
        ));
        d.budget = opts
            .capture
            .trailer
            .as_deref()
            .and_then(|b| b.parse().ok())
            .unwrap_or(75.0);
        for v in ["chase", "drone"] {
            let _ = std::fs::create_dir_all(out.join(v));
        }
        return;
    }
    d.t += time.delta_secs();
    let t = d.t;
    let Some((body, raider)) = players.iter().next() else {
        return;
    };
    let me = body.translation;
    // What's worth seeing.
    if t < WARMUP {
        d.next_frame = t;
        // Rendering, so the detail streams in, but nothing kept.
        if let Ok(mut c) = cams.single_mut() {
            c.is_active = true;
        }
        if let Some(Ok(mut c)) = d.drone.as_ref().map(|(e, _)| drone_cams.get_mut(*e)) {
            c.is_active = true;
        }
        return;
    }
    if t < WARMUP + 12.0 {
        d.want("leaving", t + 0.1);
    }
    if raider.is_some_and(|r| r.combat.is_some()) {
        d.want("fight", t + TAIL);
    }
    if robots
        .iter()
        .any(|(r, g)| !r.wrecked && g.translation().distance(me) < 30.0)
    {
        d.want("robot", t + TAIL);
    }
    if anomalies.iter().any(|(a, g)| {
        Vec2::new(g.translation().x - me.x, g.translation().z - me.z).length() < a.radius + 6.0
    }) {
        d.want("anomaly", t + TAIL);
    }
    match raid.phase {
        Phase::TruckCalled { .. } | Phase::TruckWaiting { .. } => d.want("truck", t + 0.1),
        Phase::Extracting { .. } => d.want("extract", t + 6.0),
        Phase::Down { .. } => d.want("down", t + 6.0),
        _ => {}
    }
    // A stretch of the walk now and then, while the budget lasts for
    // better things.
    if t - d.last_walk > 75.0 && d.used < d.budget * WALK_SHARE {
        d.last_walk = t;
        d.want("walk", t + 6.0);
    }
    let reason = d.reason().filter(|_| d.used < d.budget);
    if d.advance(t, reason) {
        write_manifest(d, &opts, &out);
    }
    let rolling = d.open.is_some();
    // Only the camera the cut will use renders: the one for now, and the
    // one for a moment ahead, so a camera has drawn a frame or two before
    // its first saved one.
    // The drone only when it can see the bot: in a closed wood it films
    // needles.
    let drone_ok = d.drone_blocked < DRONE_BLOCKED;
    let pick = |reason: &str, s: f32| match camera_for(reason, s) {
        View::Drone if !drone_ok => View::Chase,
        v => v,
    };
    let (now, soon) = d.open.as_ref().map_or((None, None), |c| {
        let lead = 2.5 * time.delta_secs();
        (
            Some(pick(&c.reason, t - c.start)),
            Some(pick(&c.reason, t - c.start + lead)),
        )
    });
    let on = |v: View| now == Some(v) || soon == Some(v);
    if let Ok(mut c) = cams.single_mut() {
        c.is_active = on(View::Chase);
    }
    if let Some((e, _)) = &d.drone {
        if let Ok(mut c) = drone_cams.get_mut(*e) {
            c.is_active = on(View::Drone);
        }
    }
    // The raid's over (the sim closes its telemetry and quits) or the
    // budget's spent. The list is written as each clip closes, since the
    // sim's quitting doesn't wait for us.
    let finished = log.as_ref().is_some_and(|l| l.finished);
    if finished || (d.used >= d.budget && d.open.is_none()) {
        if let Some(c) = d.open.take() {
            d.clips.push(c);
        }
        write_manifest(d, &opts, &out);
        if d.in_flight == 0 {
            info!(
                "trailer: {} clips, {} frames, raid {:.0} s",
                d.clips.len(),
                d.frame,
                t
            );
            d.done = true;
            exit.write(AppExit::Success);
        }
        return;
    }
    if !rolling {
        d.next_frame = t;
        return;
    }
    // A frame every STEP of raid time (never catching up on time spent
    // not filming).
    if d.next_frame < t - STEP {
        d.next_frame = t;
    }
    if t + 1e-4 < d.next_frame {
        return;
    }
    d.next_frame += STEP;
    let n = d.frame;
    d.frame += 1;
    d.used += STEP;
    if let Some(c) = &mut d.open {
        c.last = d.frame;
    }
    d.in_flight += 1;
    let name = format!("f{n:05}.png");
    let view = now.unwrap_or(View::Chase);
    let mut save = save_to_disk(out.join(view.dir()).join(&name));
    let done = move |shot: On<ScreenshotCaptured>, mut d: ResMut<Director>| {
        save(shot);
        d.in_flight = d.in_flight.saturating_sub(1);
    };
    match view {
        View::Chase => {
            commands.spawn(Screenshot::primary_window()).observe(done);
        }
        View::Drone => {
            let (_, image) = d.drone.clone().unwrap();
            commands.spawn(Screenshot::image(image)).observe(done);
        }
    }
}

/// Which camera films a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum View {
    Chase,
    Drone,
}

impl View {
    fn dir(self) -> &'static str {
        match self {
            View::Chase => "chase",
            View::Drone => "drone",
        }
    }
}

/// The cut, decided as it's filmed: which camera has a clip `s` seconds
/// in. A fight cuts back and forth every 2.5 s, starting close; company
/// arriving is seen from above first; walking, the truck and the ride out
/// from above; the rest close.
fn camera_for(reason: &str, s: f32) -> View {
    match reason {
        "fight" if (s / 2.5) as u32 % 2 == 1 => View::Drone,
        "fight" => View::Chase,
        "robot" | "down" if s < 3.0 => View::Drone,
        "walk" | "truck" | "extract" => View::Drone,
        _ => View::Chase,
    }
}

fn write_manifest(d: &Director, opts: &LaunchOptions, out: &std::path::Path) {
    let clips: Vec<&Clip> = d
        .clips
        .iter()
        .chain(d.open.iter())
        .filter(|c| c.last > c.first)
        .collect();
    let _ = std::fs::write(
        out.join("trailer.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "fps": (1.0 / STEP).round(),
            "map": opts.map,
            "seed": opts.seed,
            "raid_seconds": d.t,
            "clips": clips,
        }))
        .unwrap_or_default(),
    );
}

#[derive(Component)]
struct DroneCamera;

/// The share of the sight line from `from` to `to` that passes through a
/// tree's crown.
fn blocked(trees: &super::cinematic::Trees, from: Vec3, to: Vec3) -> f32 {
    let n = (from.distance(to) / 1.0).ceil().max(1.0) as usize;
    let hit = (1..n)
        .filter(|i| trees.hits(from.lerp(to, *i as f32 / n as f32), 1.5))
        .count();
    hit as f32 / n as f32
}

/// The player has no body a camera draws (in the headset you see your
/// gloves and sleeves); its shadow comes from a proxy of capsules on a
/// layer only the sun sees (`render::arms::SHADOW_ONLY`). The trailer's
/// cameras see that layer too: the bot as a figure.
fn show_body(
    mut commands: Commands,
    cams: Query<Entity, (With<Camera>, Or<(Added<FreeCamera>, Added<DroneCamera>)>)>,
) {
    use crate::render::arms::SHADOW_ONLY;
    for e in &cams {
        commands
            .entity(e)
            .insert(bevy::camera::visibility::RenderLayers::from_layers(&[
                0,
                SHADOW_ONLY,
            ]));
    }
}

/// The flat driver gives the bot's head a camera; the chase camera has
/// the window.
fn quiet_head(mut heads: Query<&mut Camera, (With<RigHead>, Without<FreeCamera>)>) {
    for mut c in &mut heads {
        if c.is_active {
            c.is_active = false;
        }
    }
}

/// And dressed: the proxy's plain white in a field jacket's drab.
fn dress_body(
    mut commands: Commands,
    parts: Query<
        (Entity, &bevy::camera::visibility::RenderLayers),
        (Added<bevy::camera::visibility::RenderLayers>, With<Mesh3d>),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut drab: Local<Option<Handle<StandardMaterial>>>,
) {
    use crate::render::arms::SHADOW_ONLY;
    for (e, layers) in &parts {
        if !layers.intersects(&bevy::camera::visibility::RenderLayers::layer(SHADOW_ONLY))
            || layers.intersects(&bevy::camera::visibility::RenderLayers::layer(0))
        {
            continue;
        }
        let m = drab
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.24, 0.26, 0.19),
                    perceptual_roughness: 0.9,
                    ..default()
                })
            })
            .clone();
        commands.entity(e).insert(MeshMaterial3d(m));
    }
}

/// Points the cameras: the chase camera eases to a spot behind and above
/// the bot's shoulder (pulled in where a wall or the ground is in the
/// way), looking a little ahead of it; the drone circles 30 m up and
/// 18 m out (higher over a crown), looking down at it.
#[allow(clippy::too_many_arguments)]
fn aim(
    mut dir: ResMut<Director>,
    time: Res<Time>,
    zone: Option<Res<crate::world::Zone>>,
    players: Query<(Entity, &Transform), With<LocalPlayer>>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    spatial: SpatialQuery,
    mut chase: Query<
        &mut Transform,
        (With<FreeCamera>, Without<DroneCamera>, Without<LocalPlayer>),
    >,
    mut drone: Query<
        &mut Transform,
        (With<DroneCamera>, Without<FreeCamera>, Without<LocalPlayer>),
    >,
    mut trees: Local<Option<super::cinematic::Trees>>,
) {
    let Some(zone) = zone else {
        return;
    };
    let Some((me, body)) = players.iter().next() else {
        return;
    };
    let head = heads
        .iter()
        .find(|(_, p)| p.parent() == me)
        .map(|(g, _)| g.compute_transform())
        .unwrap_or(Transform::from_translation(
            body.translation + Vec3::Y * 1.6,
        ));
    let dt = time.delta_secs().max(1e-4);
    let fwd = head.forward().as_vec3();
    let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
    let right = flat.cross(Vec3::Y);
    let eye = head.translation;
    // Chase: behind, above and to the right.
    let want = eye - flat * 3.6 + Vec3::Y * 0.9 + right * 0.7;
    let to = want - eye;
    let clear = spatial
        .cast_ray(
            eye,
            Dir3::new(to).unwrap_or(Dir3::NEG_Z),
            to.length(),
            true,
            // The world only: the bot's own body and hands are colliders too.
            &SpatialQueryFilter::from_mask(crate::world::Layer::World),
        )
        .map_or(to.length(), |hit| (hit.distance - 0.3).max(0.4));
    let want = eye + to.normalize_or(Vec3::NEG_Z) * clear;
    let d = &mut *dir;
    let k = 1.0 - (-dt / 0.35).exp();
    let at = d.chase_at.map_or(want, |p| p.lerp(want, k));
    let at = Vec3::new(at.x, at.y.max(zone.ground(at.x, at.z) + 0.5), at.z);
    d.chase_at = Some(at);
    if let Ok(mut c) = chase.single_mut() {
        *c = Transform::from_translation(at).looking_at(eye + flat * 5.0 - Vec3::Y * 0.4, Vec3::Y);
    }
    d.log_in -= dt;
    if d.log_in <= 0.0 {
        d.log_in = 5.0;
        debug!(
            "trailer: t {:.1} body {:.1} eye {:.1} chase {:.1}",
            d.t, body.translation, eye, at
        );
    }
    // Drone: a slow circle over the canopy, eased behind. Twice a second
    // it looks round the circle for the clearest sight of the bot past
    // the crowns and moves there if it's much better; it climbs over any
    // crown in its way.
    let trees = trees.get_or_insert_with(|| super::cinematic::Trees::new(&zone));
    let spot = |a: f32| eye + Vec3::new(a.sin(), 0.0, a.cos()) * 18.0 + Vec3::Y * 30.0;
    d.drone_angle += dt * 7f32.to_radians();
    d.look_round -= dt;
    if d.look_round <= 0.0 {
        d.look_round = 0.5;
        let here = blocked(trees, spot(d.drone_angle), eye);
        d.drone_blocked = here;
        let best = (1..8)
            .map(|k| d.drone_angle + k as f32 * std::f32::consts::FRAC_PI_4)
            .map(|a| (a, blocked(trees, spot(a), eye)))
            .min_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((a, b)) = best {
            if b + 0.15 < here {
                d.drone_angle = a;
                d.drone_blocked = b;
            }
        }
    }
    let mut want = spot(d.drone_angle);
    while trees.hits(want, super::cinematic::CROWN + 2.0) && want.y < eye.y + 60.0 {
        want.y += 2.0;
    }
    let k = 1.0 - (-dt / 1.2).exp();
    let at = d.drone_at.map_or(want, |p| p.lerp(want, k));
    let at = Vec3::new(at.x, at.y.max(zone.ground(at.x, at.z) + 8.0), at.z);
    d.drone_at = Some(at);
    if let Ok(mut c) = drone.single_mut() {
        *c = Transform::from_translation(at).looking_at(eye, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_run_their_minimum_and_cut_on_a_new_reason() {
        let mut d = Director::default();
        let r = |s: &str| Some(s.to_string());
        assert!(!d.advance(0.0, r("robot")));
        // The robot wanders off at once: the clip still runs its minimum.
        assert!(!d.advance(1.0, None));
        assert!(d.open.is_some());
        // A fight starts: a cut to a new clip.
        assert!(d.advance(MIN_CLIP + 0.1, r("fight")));
        assert_eq!(d.clips[0].reason, "robot");
        assert_eq!(d.open.as_ref().unwrap().reason, "fight");
        // The fight goes on.
        assert!(!d.advance(10.0, r("fight")));
        assert_eq!(d.open.as_ref().unwrap().end, 10.0);
        // And ends: the clip closes, nothing rolls.
        assert!(d.advance(11.0, None));
        assert!(d.open.is_none());
        assert_eq!(d.clips.len(), 2);
    }

    /// A trailer is a sim raid with cameras: no career, so no save slot,
    /// so the dev cache by the insertion the bot kits up from is there. (A
    /// trailer once had a career: its bot found no AK, decided at 103 s it
    /// had no ammunition, and walked about for eight minutes.)
    #[test]
    fn a_trailer_never_saves() {
        let o = LaunchOptions::parse(
            "--map marrow_creek --data-dir /tmp/x --sim-run trailer --trailer 60"
                .split(' ')
                .map(String::from),
        );
        assert!(!o.headless, "a trailer renders");
        assert!(o.camera.is_none(), "a trailer's bot starts in the truck");
        assert_eq!(crate::persist::career_path(&o), None);
    }

    #[test]
    fn the_cut_is_decided_as_it_is_filmed() {
        assert_eq!(camera_for("fight", 0.5), View::Chase);
        assert_eq!(camera_for("fight", 3.0), View::Drone);
        assert_eq!(camera_for("fight", 5.5), View::Chase);
        assert_eq!(camera_for("robot", 1.0), View::Drone);
        assert_eq!(camera_for("robot", 4.0), View::Chase);
        assert_eq!(camera_for("walk", 4.0), View::Drone);
        assert_eq!(camera_for("leaving", 4.0), View::Chase);
    }

    #[test]
    fn the_most_telling_reason_wins() {
        let mut d = Director {
            t: 5.0,
            ..default()
        };
        d.want("walk", 10.0);
        d.want("robot", 10.0);
        assert_eq!(d.reason().as_deref(), Some("robot"));
        d.want("fight", 6.0);
        assert_eq!(d.reason().as_deref(), Some("fight"));
        d.t = 7.0;
        assert_eq!(d.reason().as_deref(), Some("robot"));
        d.t = 11.0;
        assert_eq!(d.reason(), None);
    }
}
