//! The worst minute of a raid, not the average: in a headset one dropped
//! frame in a firefight is what's remembered. Each valley's stress scene
//! is a dusk downpour by a building, three machines on the player (two
//! walkers and a drone), the player firing in bursts (muzzle flash, a hot
//! barrel's smoke), a grenade's blast and smoke every few seconds, a door
//! breached in the building's wall, and a dirigible overhead.
//!
//! [`StressPlugin`] stages it and runs it, the same in the game (`--stress`,
//! once the world is in: in a headset, its GPU per eye; it logs the
//! minute's CPU and GPU and quits) and in the gauge (`tests`), which
//! measures each frame's CPU (wall time, and against a reference workload
//! timed beside it, so a slower machine isn't a regression) and every few
//! frames both eyes' load at the headset's size (draws, shadow draws,
//! shaded fragments: `vr_budget`'s stereo budget, which is what the GPU's
//! 11.1 ms comes to at the default render scale). What the worst frames
//! spend more on is named with `STRESS_PROFILE=1` and
//! `--features "bevy/trace bevy/debug"`. See docs/PERFORMANCE.md, "The
//! worst minute".

use bevy::prelude::*;
use std::collections::HashMap;

use crate::rig::Side;

/// Frames run: 10 s at 90 Hz (`STRESS_FRAMES`; a minute is 5400).
pub fn frames() -> u32 {
    std::env::var("STRESS_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900)
}

/// Frames of the fight before it's measured: a second (`STRESS_WARM`;
/// fewer for a software GPU's few frames).
pub fn warm_frames() -> u32 {
    std::env::var("STRESS_WARM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90)
}

/// Stages the scene once [`StressRun::go`] (the game: once the world is
/// in) and runs it for [`StressRun::frames`].
pub struct StressPlugin;

impl Plugin for StressPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<StressRun>() {
            let mut run = StressRun::new(frames());
            run.quit = true;
            app.insert_resource(run);
        }
        add_systems(app);
    }
}

/// The run's systems (the gauge's app is built and finished before its
/// scene is added: it adds these itself).
pub(crate) fn add_systems(app: &mut App) {
    {
        app.add_systems(Update, (go, drive).chain())
            .add_systems(Last, record)
            // In the fixed step, before the guns read it: the flat driver
            // writes the trigger every frame too, from the mouse.
            .add_systems(FixedPreUpdate, pull);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    /// For the world to be in.
    Waiting,
    /// The player placed and the guns spawned (this frame).
    Placed,
    /// The guns taken and the machines spawned, this many frames ago.
    Arming(u32),
    /// A second of the fight before it's measured.
    Warming(u32),
    /// Measured.
    Running,
    Done,
}

/// The run: where it's staged and how far it's got.
#[derive(Resource)]
pub struct StressRun {
    /// Start now (the game sets it once the world is in).
    pub go: bool,
    /// The game's own run (`--stress`): measured by the game, logged,
    /// and it quits when done.
    pub quit: bool,
    pub frames: u32,
    /// The measured frame it's on.
    pub frame: u32,
    stage: Stage,
    wall: Vec3,
    stand: Vec3,
    guns: Vec<(Entity, Side)>,
    machines: Vec<Entity>,
    grenade: Option<Entity>,
    trigger: bool,
    /// The game's measure: main schedule ms, and the GPU's passes' ms,
    /// each measured frame.
    cpu: Vec<f64>,
    gpu: Vec<HashMap<String, f64>>,
    gpu_seen: Option<std::time::Instant>,
    /// Views each pass is timed for: a headset's two eyes, or a flat
    /// screen's one.
    views: usize,
}

impl StressRun {
    pub fn new(frames: u32) -> Self {
        Self {
            go: false,
            quit: false,
            frames,
            frame: 0,
            stage: Stage::Waiting,
            wall: Vec3::ZERO,
            stand: Vec3::ZERO,
            guns: Vec::new(),
            machines: Vec::new(),
            grenade: None,
            trigger: false,
            cpu: Vec::new(),
            gpu: Vec::new(),
            gpu_seen: None,
            views: 1,
        }
    }

    pub fn running(&self) -> bool {
        self.stage == Stage::Running
    }

    pub fn done(&self) -> bool {
        self.stage == Stage::Done
    }
}

/// The game's run starts once the world's in (everything drawn ready).
fn go(
    mut run: ResMut<StressRun>,
    world: super::timeline::WorldReady,
    eyes: Query<(), With<bevy_mod_xr::camera::XrCamera>>,
) {
    if run.quit && !run.go && run.stage == Stage::Waiting && world.ready() {
        run.go = true;
        run.views = eyes.iter().count().max(1);
    }
}

/// The building the scene is staged at: the one with an inside nearest
/// the map's middle.
fn placement(world: &World) -> extraction_core::map::Placement {
    let zone = world.resource::<crate::world::Zone>();
    zone.map
        .placements
        .iter()
        .filter(|p| crate::tour::has_inside(&p.prefab))
        .min_by(|a, b| a.at[0].hypot(a.at[1]).total_cmp(&b.at[0].hypot(b.at[1])))
        .expect("a building to go into")
        .clone()
}

/// The middle of the building's `n`th wall (front, right, back, left), a
/// metre up, and the way out through it.
pub fn side(world: &World, n: u32) -> (Vec3, Vec3) {
    let zone = world.resource::<crate::world::Zone>();
    let p = placement(world);
    let f = crate::world::structures::footprint(&p.prefab);
    let rot = Quat::from_rotation_y(-p.rot.to_radians());
    let (dir, half) = match n % 4 {
        0 => (Vec3::NEG_Z, f.z * 0.5),
        1 => (Vec3::X, f.x * 0.5),
        2 => (Vec3::Z, f.z * 0.5),
        _ => (Vec3::NEG_X, f.x * 0.5),
    };
    let out = rot * dir;
    let ground = zone.ground(p.at[0], p.at[1]);
    (Vec3::new(p.at[0], ground + 1.0, p.at[1]) + out * half, out)
}

fn player(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
        .single(world)
        .ok()
}

fn spawn_item(world: &mut World, id: &str, at: Vec3) -> Entity {
    let catalog = world.resource::<crate::items::Catalog>().0.clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let e = {
        let mut c = Commands::new(&mut queue, world);
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog
                    .kind(id)
                    .unwrap_or_else(|| panic!("{id} in the catalogue")),
                transform: Transform::from_translation(at),
                condition: 1.0,
            },
        )
    };
    queue.apply(world);
    e
}

fn drive(world: &mut World) {
    let stage = world.resource::<StressRun>().stage;
    match stage {
        Stage::Waiting if world.resource::<StressRun>().go => place(world),
        Stage::Waiting | Stage::Done => {}
        Stage::Placed => {
            take_guns(world);
            spawn_machines(world);
            world.resource_mut::<StressRun>().stage = Stage::Arming(0);
        }
        Stage::Arming(n) => {
            keep_alive(world);
            // The machines' brains are in once they've had a frame.
            if n == 1 {
                hunt(world);
            }
            world.resource_mut::<StressRun>().stage = if n >= 10 {
                load(world);
                Stage::Warming(0)
            } else {
                Stage::Arming(n + 1)
            };
        }
        Stage::Warming(n) => {
            fight(world, n);
            world.resource_mut::<StressRun>().stage = if n + 1 >= warm_frames() {
                Stage::Running
            } else {
                Stage::Warming(n + 1)
            };
        }
        Stage::Running => {
            let (f, frames, wall) = {
                let run = world.resource::<StressRun>();
                (run.frame, run.frames, run.wall)
            };
            fight(world, f);
            // On the 10 s minute's clock (a shorter run's, squeezed: a
            // software GPU's few frames still see each): a blast every 4 s
            // from 1.3 s; the wall breached at 5 s, at the front wall's
            // middle, then (if no hole came) each other side's.
            let at = |t: u32| f * 900 / frames <= t && t < (f + 1) * 900 / frames;
            if [120, 480, 840].into_iter().any(at) {
                blast(world, wall);
            }
            if let Some(n) = (0..4).find(|n| at(450 + 10 * n)) {
                if holes(world) == 0 {
                    let (at, out) = side(world, n);
                    world.write_message(crate::destruction::hooks::Breach { at, out });
                }
            }
            let quit = {
                let mut run = world.resource_mut::<StressRun>();
                run.frame += 1;
                if run.frame < run.frames {
                    return;
                }
                run.stage = Stage::Done;
                if run.quit {
                    for l in report(&run) {
                        info!("stress: {l}");
                    }
                }
                run.quit
            };
            if quit {
                world.write_message(AppExit::Success);
            }
        }
    }
}

/// Holes breached so far.
pub fn holes(world: &World) -> usize {
    world
        .get_resource::<crate::destruction::breach::Breaches>()
        .map_or(0, |b| b.0.len())
}

/// The player 10 m out from the building's front wall, facing it; the
/// guns spawned in reach.
fn place(world: &mut World) {
    let (wall, out) = side(world, 0);
    let zone = world.resource::<crate::world::Zone>().clone();
    let stand = zone.on_ground(wall.x + out.x * 10.0, wall.z + out.z * 10.0);
    let Some(p) = player(world) else {
        return;
    };
    let face = Transform::from_translation(stand)
        .looking_at(wall.with_y(stand.y), Vec3::Y)
        .rotation;
    if let Some(mut t) = world.get_mut::<Transform>(p) {
        t.translation = stand;
        t.rotation = face;
    }
    let mut q = world.query_filtered::<&mut Transform, With<super::FreeCamera>>();
    for mut t in q.iter_mut(world) {
        *t = Transform::from_translation(stand + Vec3::Y * 1.7).looking_at(wall, Vec3::Y);
    }
    let guns = [("ak74", Side::Right), ("pm_pistol", Side::Left)]
        .into_iter()
        .map(|(id, side)| (spawn_item(world, id, stand + Vec3::Y * 1.2), side))
        .collect();
    let mut run = world.resource_mut::<StressRun>();
    run.wall = wall;
    run.stand = stand;
    run.guns = guns;
    run.stage = Stage::Placed;
}

fn take_guns(world: &mut World) {
    let Some(player) = player(world) else {
        return;
    };
    let guns = world.resource::<StressRun>().guns.clone();
    for (item, side) in guns {
        world.write_message(crate::interact::HandAction::Grab {
            player,
            side,
            item,
            point: None,
        });
    }
}

/// Three machines 30 m off (two walkers and a drone), and a dirigible
/// over the building.
fn spawn_machines(world: &mut World) {
    let zone = world.resource::<crate::world::Zone>().clone();
    let (stand, wall) = {
        let run = world.resource::<StressRun>();
        (run.stand, run.wall)
    };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut machines = Vec::new();
    {
        let mut c = Commands::new(&mut queue, world);
        for (i, a) in [0.6f32, 2.4, 4.2].into_iter().enumerate() {
            let spot = zone.on_ground(stand.x + a.cos() * 30.0, stand.z + a.sin() * 30.0);
            let job = crate::robots::Job {
                kind: crate::robots::JobKind::Response,
                site: stand,
                duration: 999.0,
                exit: spot,
            };
            machines.push(if i == 2 {
                crate::robots::drone::spawn_drone(&mut c, spot + Vec3::Y * 15.0, job)
            } else {
                crate::robots::walker::spawn_walker(&mut c, &zone, spot, job)
            });
        }
        crate::robots::dirigible::spawn_dirigible(
            &mut c,
            wall + Vec3::Y * 40.0,
            crate::robots::Job {
                kind: crate::robots::JobKind::Watch,
                site: wall,
                duration: 999.0,
                exit: wall + Vec3::X * 300.0,
            },
        );
    }
    queue.apply(world);
    world.resource_mut::<StressRun>().machines = machines;
}

/// The scene's walkers and drone after the player.
fn hunt(world: &mut World) {
    let (at, machines) = {
        let run = world.resource::<StressRun>();
        (run.stand, run.machines.clone())
    };
    for m in machines {
        if let Some(mut b) = world.get_mut::<crate::robots::Brain>(m) {
            *b = crate::robots::Brain::Investigate { at, left: 999.0 };
        }
    }
}

/// A frame of the fight: everyone kept standing (the minute is about
/// cost, not who wins), bursts of 0.4 s every second, the guns topped up.
fn fight(world: &mut World, frame: u32) {
    keep_alive(world);
    if frame % 90 == 0 {
        load(world);
    }
    world.resource_mut::<StressRun>().trigger = frame % 90 < 36;
    // The grenade's fuse lit once it's in (it came in last frame).
    if let Some(g) = world.resource_mut::<StressRun>().grenade.take() {
        if let Some(mut live) = world.get_mut::<crate::weapons::grenades::Live>(g) {
            live.grenade.fuse = extraction_core::grenade::Fuse::Burning(1e-4);
        }
    }
}

fn keep_alive(world: &mut World) {
    for mut h in world.query::<&mut crate::health::Health>().iter_mut(world) {
        h.hp = h.max;
        h.down = false;
    }
}

/// The player's guns loaded: a full magazine, a round in, on auto.
fn load(world: &mut World) {
    use extraction_core::firearm::{FireMode, Magazine};
    let mut q = world.query_filtered::<&mut crate::weapons::Gun, With<crate::interact::Held>>();
    for mut g in q.iter_mut(world) {
        let f = &mut g.firearm;
        let Some(family) = f.def.magazine_family.clone() else {
            continue;
        };
        if f.rounds_in_feed() < 5 {
            let _ = f.remove_magazine();
            let _ = f.insert_magazine(Magazine {
                family,
                capacity: 60,
                rounds: 60,
                ..Default::default()
            });
            f.set_bolt_travel(1.0);
            f.release_bolt();
        }
        let auto = f.def.modes.contains(&FireMode::Auto);
        let want = if auto { FireMode::Auto } else { FireMode::Semi };
        for _ in 0..3 {
            if f.fire_mode() == want {
                break;
            }
            f.cycle_mode();
        }
        f.condition = 1.0;
    }
}

/// An RGD-5 6 m toward the building, its fuse lit next frame: its blast,
/// its fragments and its smoke.
fn blast(world: &mut World, wall: Vec3) {
    let at = world.resource::<StressRun>().stand;
    let to = (wall - at).with_y(0.0).normalize_or_zero();
    let zone = world.resource::<crate::world::Zone>().clone();
    let spot = zone.on_ground(at.x + to.x * 6.0, at.z + to.z * 6.0) + Vec3::Y * 0.3;
    let g = spawn_item(world, "rgd5", spot);
    world.resource_mut::<StressRun>().grenade = Some(g);
}

/// The player's trigger this frame (set at the fixed step's start, after
/// the flat driver's mouse, or the headset's controller, has written it).
fn pull(
    run: Res<StressRun>,
    mut input: Query<&mut crate::rig::RigInput, With<crate::rig::LocalPlayer>>,
) {
    if !matches!(run.stage, Stage::Warming(_) | Stage::Running) {
        return;
    }
    if let Ok(mut i) = input.single_mut() {
        i.hand_mut(Side::Right).trigger = if run.trigger { 1.0 } else { 0.0 };
    }
}

/// The game's measure, each frame of the minute: the main schedule's CPU
/// (`cpu::ScheduleTimes`) and the GPU's passes (the render diagnostics'
/// timestamps, as they come in).
fn record(
    mut run: ResMut<StressRun>,
    cpu: Option<Res<super::cpu::ScheduleTimes>>,
    store: Option<Res<bevy::diagnostic::DiagnosticsStore>>,
) {
    if !run.quit {
        return;
    }
    let mut passes = HashMap::new();
    if let Some(store) = &store {
        let mut newest = run.gpu_seen;
        for d in store.iter() {
            let path = d.path().as_str();
            let Some(pass) = path
                .strip_prefix("render/")
                .and_then(|p| p.strip_suffix("/elapsed_gpu"))
            else {
                continue;
            };
            for m in d.measurements() {
                if run.gpu_seen.is_some_and(|s| m.time <= s) {
                    continue;
                }
                *passes.entry(pass.to_string()).or_default() += m.value;
                newest = newest.max(Some(m.time));
            }
        }
        run.gpu_seen = newest;
    }
    if run.stage != Stage::Running {
        return;
    }
    // (The frame before's: this one's `Last` isn't out yet.)
    let ms = cpu
        .and_then(|c| {
            c.frames
                .last()
                .and_then(|f| f.get("main schedule").copied())
        })
        .unwrap_or(0.0);
    run.cpu.push(ms);
    run.gpu.push(passes);
}

fn pct(v: &[f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() as f64 * q) as usize).min(s.len() - 1)]
}

/// The game's minute: the CPU's p99 and worst, the GPU's (all views
/// together, and per view: a headset's two eyes are two, each pass timed
/// for both; a flat screen's one view is an eye's work at its size), and
/// what the GPU's worst frames (its slowest 1%, five at least) spend on,
/// against every frame's mean.
fn report(run: &StressRun) -> Vec<String> {
    let views = run.views as f64;
    let total = |p: &HashMap<String, f64>| -> f64 {
        p.iter()
            .filter(|(k, _)| !k.contains('/'))
            .map(|(_, v)| v)
            .sum()
    };
    // The timestamps come in a few frames late, and now and then two
    // frames' at once: only frames that got one frame's count.
    let gpu: Vec<&HashMap<String, f64>> = run.gpu.iter().filter(|p| !p.is_empty()).collect();
    let totals: Vec<f64> = gpu.iter().map(|p| total(p)).collect();
    let mut out = vec![format!(
        "{} frames; CPU (main schedule) p99 {:.1} ms, worst {:.1}; GPU ({} frames timed, {} views) p99 {:.1} ms, worst {:.1}; per view p99 {:.1}, worst {:.1}, of 11.1",
        run.cpu.len(),
        pct(&run.cpu, 0.99),
        pct(&run.cpu, 1.0),
        totals.len(),
        run.views,
        pct(&totals, 0.99),
        pct(&totals, 1.0),
        pct(&totals, 0.99) / views,
        pct(&totals, 1.0) / views,
    )];
    let mut order: Vec<usize> = (0..gpu.len()).collect();
    order.sort_by(|a, b| totals[*b].total_cmp(&totals[*a]));
    order.truncate((gpu.len() / 100).max(5));
    if order.is_empty() {
        return out;
    }
    let all = gpu.len() as f64;
    let mut typical: HashMap<&str, f64> = HashMap::new();
    for p in &gpu {
        for (k, v) in p.iter() {
            *typical.entry(k).or_default() += v / all;
        }
    }
    let n = order.len() as f64;
    let mut worst: HashMap<&str, f64> = HashMap::new();
    for i in &order {
        for (k, v) in gpu[*i].iter() {
            *worst.entry(k).or_default() += v / n;
        }
    }
    let mut v: Vec<(&str, f64)> = worst.into_iter().collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    out.push(format!(
        "the GPU's worst 1%, ms a frame, every view (typically): {}",
        v.iter()
            .take(8)
            .map(|(k, t)| format!("{k} {t:.2} ({:.2})", typical.get(k).copied().unwrap_or(0.0)))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use std::collections::HashMap;
    use std::time::Instant;

    use super::super::budget::tests::{
        SystemTimes, as_the_game_runs, client_threads, heavy, settle,
    };
    use super::StressRun;
    use crate::perf::fight::reference_ms;

    /// The gauge's frames: 5 s a valley at 90 Hz, so the gauge tier keeps
    /// to its time (`STRESS_FRAMES=900`, or 5400 for the whole minute, for
    /// performance work). The minute's blasts and breach come in the
    /// frames there are.
    fn gauge_frames() -> u32 {
        std::env::var("STRESS_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(450)
    }

    /// The CPU's p99 frame, in reference units (the busy moment's own
    /// budget for its mean, `budget::CPU_FRAME_UNITS`): the worst
    /// minute's slow frames within what an average busy frame may take.
    const P99_UNITS: f64 = super::super::budget::CPU_FRAME_UNITS;

    /// Shots fired in the scene (the player's and the machines').
    #[derive(Resource, Default)]
    struct Shots(usize);

    fn count_shots(
        mut shots: MessageReader<crate::weapons::ballistics::Shot>,
        mut n: ResMut<Shots>,
    ) {
        n.0 += shots.read().count();
    }

    static PROFILE: std::sync::OnceLock<SystemTimes> = std::sync::OnceLock::new();

    fn profile() {
        if std::env::var_os("STRESS_PROFILE").is_some() && PROFILE.get().is_none() {
            use bevy::log::tracing_subscriber::layer::SubscriberExt;
            let times = PROFILE.get_or_init(Default::default).clone();
            let subscriber = bevy::log::tracing_subscriber::registry().with(times);
            let _ = bevy::log::tracing::subscriber::set_global_default(subscriber);
        }
    }

    fn systems() -> HashMap<String, f64> {
        PROFILE
            .get()
            .map(|p| std::mem::take(&mut *p.0.lock().unwrap()))
            .unwrap_or_default()
    }

    /// A valley's stress scene, staged and warmed (the plugin's), at the
    /// start of its measured minute.
    fn stress_scene(map: &str) -> App {
        profile();
        let mut app = client_threads(map, "downpour", None);
        app.world_mut()
            .resource_mut::<crate::launch::LaunchOptions>()
            .hour = Some("dusk".into());
        as_the_game_runs(&mut app);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 90.0),
        ))
        .init_resource::<Shots>()
        .insert_resource(StressRun::new(gauge_frames()))
        .add_systems(Last, count_shots);
        // `STATIC_OFF=1`: Bevy's static transform optimisation off (what
        // it costs: `transforms_static_on_off`).
        if std::env::var_os("STATIC_OFF").is_some() {
            app.insert_resource(bevy::transform::systems::StaticTransformOptimizations::Disabled);
        }
        super::add_systems(&mut app);
        settle(&mut app);
        app.world_mut().resource_mut::<StressRun>().go = true;
        for _ in 0..200 {
            if app.world().resource::<StressRun>().running() {
                break;
            }
            app.update();
        }
        assert!(
            app.world().resource::<StressRun>().running(),
            "{map}: the stress scene never got going"
        );
        let held = {
            let w = app.world_mut();
            w.query_filtered::<(), (With<crate::weapons::Gun>, With<crate::interact::Held>)>()
                .iter(w)
                .count()
        };
        assert_eq!(held, 2, "{map}: the player should hold two guns");
        app
    }

    struct Stress {
        /// CPU ms and reference units, each frame.
        ms: Vec<f64>,
        units: Vec<f64>,
        /// Both eyes' loads, sampled: (draws, shadow draws, fragments M).
        eyes: Vec<(usize, usize, f32)>,
        /// What the worst frames spend more on (with the profile):
        /// `worst_costs`.
        worst: Vec<(f64, Vec<(String, f64)>)>,
        holes: usize,
    }

    /// A valley's worst minute (or `STRESS_FRAMES`).
    fn stress(map: &str) -> Stress {
        let mut app = stress_scene(map);
        let eyes = super::super::vr_budget::both_eyes(&mut app);
        super::super::vr_budget::eye_projections(&mut app);
        let px = super::super::vr_budget::eye_px();
        let (mut ms, mut units, mut loads) = (Vec::new(), Vec::new(), Vec::new());
        let mut each: Vec<(f64, HashMap<String, f64>)> = Vec::new();
        let mut fragments = 0.0f32;
        let mut lights_each = Vec::new();
        let mut f = 0;
        while !app.world().resource::<StressRun>().done() {
            systems();
            let t = Instant::now();
            app.update();
            let frame = t.elapsed().as_secs_f64() * 1000.0;
            let reference = reference_ms();
            ms.push(frame);
            units.push(frame / reference);
            let named = systems();
            if !named.is_empty() {
                let v: HashMap<String, f64> = named
                    .into_iter()
                    .filter(|(k, _)| {
                        !k.contains("run_main")
                            && !k.contains("run_fixed_main")
                            && !k.contains("run_physics_schedule")
                            && !k.contains("run_substep_schedule")
                    })
                    .map(|(k, v)| (k.rsplit("::").next().unwrap_or(&k).to_string(), v))
                    .collect();
                each.push((frame, v));
            }
            // Both eyes' load every third of a second; their fragments
            // (the costlier count) every second, held between.
            if f % 30 == 0 {
                let (l, r, _) = super::super::vr_budget::measure(&mut app, eyes);
                if f % 90 == 0 || fragments == 0.0 {
                    fragments = [eyes.0, eyes.1]
                        .into_iter()
                        .map(|e| {
                            super::super::overdraw::measure(app.world_mut(), e).mean * px * px / 1e6
                        })
                        .sum();
                }
                loads.push((
                    l.draws + r.draws,
                    l.shadow_draws + r.shadow_draws,
                    fragments,
                ));
            }
            if f % 90 == 0 {
                let w = app.world_mut();
                let n = w.query::<&PointLight>().iter(w).count();
                lights_each.push(n);
            }
            f += 1;
        }
        let holes = super::holes(app.world());
        {
            let w = app.world_mut();
            let robots = w
                .query_filtered::<(), With<crate::robots::Robot>>()
                .iter(w)
                .count();
            let lights = w.query::<&PointLight>().iter(w).count();
            let shadowed = w
                .query::<&PointLight>()
                .iter(w)
                .filter(|l| l.shadow_maps_enabled)
                .count();
            let sun = w
                .query::<&DirectionalLight>()
                .iter(w)
                .map(|l| (l.illuminance, l.shadow_maps_enabled))
                .collect::<Vec<_>>();
            let shots = w.resource::<Shots>().0;
            println!("    point lights each second: {lights_each:?}");
            println!(
                "    scene: {robots} machines, {shots} shots, {lights} point lights ({shadowed} shadowed), sun {sun:?}"
            );
        }
        Stress {
            ms,
            units,
            eyes: loads,
            worst: worst_costs(each),
            holes,
        }
    }

    /// What the worst frames (the slowest 1%, five at least) spend more
    /// on: each system's mean over them, against its mean over every
    /// frame, the eight the worst frames spend most on. First: the frames'
    /// own mean, and what the traced systems add up to in them (the rest
    /// is outside any system: the schedule, the render world's own work).
    fn worst_costs(mut each: Vec<(f64, HashMap<String, f64>)>) -> Vec<(f64, Vec<(String, f64)>)> {
        if each.is_empty() {
            return Vec::new();
        }
        let all = each.len() as f64;
        let mut typical: HashMap<String, f64> = HashMap::new();
        for (_, v) in &each {
            for (k, t) in v {
                *typical.entry(k.clone()).or_default() += t / all;
            }
        }
        each.sort_by(|a, b| b.0.total_cmp(&a.0));
        each.truncate((each.len() / 100).max(5));
        let n = each.len() as f64;
        let mean = each.iter().map(|(f, _)| f).sum::<f64>() / n;
        let mut in_worst: HashMap<String, f64> = HashMap::new();
        for (_, v) in &each {
            for (k, t) in v {
                *in_worst.entry(k.clone()).or_default() += t / n;
            }
        }
        let traced = in_worst.values().sum::<f64>();
        let mut v: Vec<(String, f64)> = in_worst.into_iter().collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v.truncate(8);
        let v = v
            .into_iter()
            .map(|(k, t)| {
                let typ = typical.get(&k).copied().unwrap_or(0.0);
                (format!("{k} (typically {typ:.2})"), t)
            })
            .collect();
        vec![
            (mean, vec![("all traced systems".to_string(), traced)]),
            (mean, v),
        ]
    }

    use super::pct;

    /// What transform propagation costs a frame, calm Marrow (after the
    /// load and the warm-up) and in its worst minute, as the game runs it
    /// or with `STATIC_OFF=1` (Bevy's static transform optimisation off: a
    /// stopgap for children added under a static tree going unpropagated).
    /// With `STRESS_PROFILE=1 --features "bevy/trace bevy/debug"`;
    /// `CALM_ONLY=1` skips the fight.
    #[test]
    #[ignore = "diagnosis"]
    fn transforms_static_on_off() {
        let _one_at_a_time = heavy();
        let names = [
            "mark_dirty_trees",
            "propagate_parent_transforms",
            "sync_simple_transforms",
        ];
        let report = |what: &str, rows: &Vec<(f64, HashMap<String, f64>)>| {
            let mut total = Vec::new();
            for n in names {
                let mut v: Vec<f64> = rows
                    .iter()
                    .map(|(_, m)| {
                        m.iter()
                            .filter(|(k, _)| k.contains(n))
                            .map(|(_, t)| t)
                            .sum()
                    })
                    .collect();
                if total.is_empty() {
                    total = vec![0.0; v.len()];
                }
                for (t, x) in total.iter_mut().zip(&v) {
                    *t += x;
                }
                let mean = v.iter().sum::<f64>() / v.len() as f64;
                v.sort_by(f64::total_cmp);
                println!(
                    "  {what} {n}: mean {mean:.3} ms, p99 {:.3}, max {:.3}",
                    pct(&v, 0.99),
                    pct(&v, 1.0)
                );
            }
            let mean = total.iter().sum::<f64>() / total.len() as f64;
            let frames: Vec<f64> = rows.iter().map(|r| r.0).collect();
            println!(
                "  {what} all three: mean {mean:.3} ms, p99 {:.3}, max {:.3}; frame p99 {:.1} ms",
                pct(&total, 0.99),
                pct(&total, 1.0),
                pct(&frames, 0.99)
            );
        };
        // Calm: Marrow settled, nothing happening.
        profile();
        let mut app = client_threads("marrow_creek", "clearing", None);
        as_the_game_runs(&mut app);
        if std::env::var_os("STATIC_OFF").is_some() {
            app.insert_resource(bevy::transform::systems::StaticTransformOptimizations::Disabled);
        }
        // The game's clock, a frame at a time (on real time a slow
        // harness frame runs several fixed ticks, each propagating).
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 90.0),
        ));
        settle(&mut app);
        // Past the warm-up shelf (its 900 frames) and what follows a load.
        for _ in 0..1000 {
            app.update();
        }
        let mut rows = Vec::new();
        for _ in 0..450 {
            systems();
            let t = Instant::now();
            app.update();
            rows.push((t.elapsed().as_secs_f64() * 1000.0, systems()));
        }
        drop(app);
        report("calm", &rows);
        if std::env::var_os("CALM_ONLY").is_some() {
            return;
        }
        // The fight.
        let mut app = stress_scene("marrow_creek");
        let mut rows = Vec::new();
        while !app.world().resource::<StressRun>().done() {
            systems();
            let t = Instant::now();
            app.update();
            rows.push((t.elapsed().as_secs_f64() * 1000.0, systems()));
        }
        report("fight", &rows);
    }

    /// `STRESS_MAP=<map>` for one valley; every valley otherwise.
    fn maps() -> Vec<String> {
        std::env::var("STRESS_MAP").map_or_else(
            |_| {
                extraction_core::map::VALLEYS
                    .iter()
                    .map(|v| v.0.to_string())
                    .collect()
            },
            |m| vec![m],
        )
    }

    /// Every valley's worst minute holds: the CPU's p99 within the busy
    /// frame's budget (reference units), and both eyes' p99 load within
    /// the stereo budget at the default render scale.
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_every_valley_holds_its_worst_minute() {
        let _one_at_a_time = heavy();
        let budget = super::super::vr_budget::budget();
        let mut wrong = Vec::new();
        for map in maps() {
            let t = Instant::now();
            let s = stress(&map);
            let (p99, worst) = (pct(&s.ms, 0.99), pct(&s.ms, 1.0));
            let (u99, uworst) = (pct(&s.units, 0.99), pct(&s.units, 1.0));
            let d99 = pct(&s.eyes.iter().map(|e| e.0 as f64).collect::<Vec<_>>(), 0.99);
            let sd99 = pct(&s.eyes.iter().map(|e| e.1 as f64).collect::<Vec<_>>(), 0.99);
            let f99 = pct(
                &s.eyes.iter().map(|e| f64::from(e.2)).collect::<Vec<_>>(),
                0.99,
            );
            println!(
                "{map}: {} frames; CPU p99 {p99:.1} ms ({u99:.0} units of {P99_UNITS:.0}), worst {worst:.1} ms ({uworst:.0}); \
                 both eyes p99: {d99:.0} draws of {}, {sd99:.0} shadow draws of {}, {f99:.1} M fragments of {:.1}; \
                 {} holes breached ({:.0} s)",
                s.ms.len(),
                budget.draws,
                budget.shadow_draws,
                budget.fragments_m,
                s.holes,
                t.elapsed().as_secs_f64()
            );
            for (ms, top) in &s.worst {
                println!(
                    "    worst 1% (mean {ms:.1} ms): {}",
                    top.iter()
                        .map(|(k, v)| format!("{k} {v:.2}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            if u99 > P99_UNITS {
                wrong.push(format!("{map}: CPU p99 {u99:.0} units > {P99_UNITS:.0}"));
            }
            if d99 > budget.draws as f64 {
                wrong.push(format!(
                    "{map}: {d99:.0} draws a frame (p99) > {}",
                    budget.draws
                ));
            }
            if sd99 > budget.shadow_draws as f64 {
                wrong.push(format!(
                    "{map}: {sd99:.0} shadow draws (p99) > {}",
                    budget.shadow_draws
                ));
            }
            if f99 > f64::from(budget.fragments_m) {
                wrong.push(format!(
                    "{map}: {f99:.1} M fragments (p99) > {:.1}",
                    budget.fragments_m
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "worst minutes over budget:\n{}",
            wrong.join("\n")
        );
    }
}
