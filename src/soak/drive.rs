//! What plays a soak run, and `extraction soak` itself.
//!
//! [`Raids`] is the cheap driver: it walks the player out of the truck's
//! pull-in and leaves them standing in the Zone with the Surge's clock
//! wound on, so a raid lasts `raid_secs`. The director sends its robots,
//! wildlife wander, the weather turns, then the Surge comes and the player
//! goes down in the open; they wake in the truck, the Zone resets (fresh
//! loot, the remains swept away) and out they go again. Each cycle is a
//! whole raid's worth of the world's own churn, with nobody tidying up.

use bevy::prelude::*;

use super::census::{self, Census, Record};
use crate::raid::{Phase, Raid, SURGE_AFTER};
use crate::rig::LocalPlayer;
use crate::world::Zone;

/// Raid after raid: out, the Surge, down, home, the reset, out again.
pub struct Raids {
    /// Seconds from stepping out to the Surge.
    pub raid_secs: f32,
    /// Frames spent at base since getting back.
    at_base: u32,
    out: bool,
    /// Raids started.
    pub raids: u32,
}

impl Raids {
    pub fn new(raid_secs: f32) -> Self {
        Self {
            raid_secs,
            at_base: 0,
            out: false,
            raids: 0,
        }
    }

    /// Call after every frame.
    pub fn drive(&mut self, app: &mut App) {
        let world = app.world_mut();
        let phase = world.resource::<Raid>().phase.clone();
        match phase {
            Phase::AtBase => {
                self.out = false;
                self.at_base += 1;
                // A few seconds in the truck (the Zone resets in there),
                // then out past the pull-in.
                if self.at_base == 90 * 4 {
                    let Some(at) = step_out(world, self.raids) else {
                        return;
                    };
                    for mut t in world
                        .query_filtered::<&mut Transform, With<LocalPlayer>>()
                        .iter_mut(world)
                    {
                        t.translation = at;
                    }
                }
            }
            _ => {
                self.at_base = 0;
                if !self.out {
                    self.out = true;
                    self.raids += 1;
                    let mut raid = world.resource_mut::<Raid>();
                    raid.clock = raid.clock.max(SURGE_AFTER - self.raid_secs);
                }
            }
        }
    }
}

/// A spot on open ground 20-30 m from the truck, a different one each raid.
fn step_out(world: &mut World, n: u32) -> Option<Vec3> {
    let zone = world.resource::<Zone>().clone();
    let truck = world
        .query_filtered::<&GlobalTransform, With<crate::raid::truck::Truck>>()
        .iter(world)
        .next()?
        .translation();
    let a = n as f32 * 2.4;
    let r = 22.0 + (n % 3) as f32 * 4.0;
    let (x, z) = (truck.x + a.cos() * r, truck.z + a.sin() * r);
    Some(zone.on_ground(x, z) + Vec3::Y * 0.05)
}

/// A headless game for a soak run.
pub fn app(map: &str, seed: u64, robots: bool) -> App {
    let opts = crate::LaunchOptions {
        map: map.into(),
        map_explicit: true,
        seed,
        robots,
        ..Default::default()
    };
    let mut app = crate::headless_app(opts, false);
    app.finish();
    app.cleanup();
    app
}

/// The whole game, visuals and sound included, with no window and no GPU:
/// what the client half makes (meshes, materials, voices) piles up here
/// as it would in a headset session.
pub fn client_app(map: &str, seed: u64, robots: bool) -> App {
    use bevy::render::RenderPlugin;
    use bevy::render::settings::WgpuSettings;
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(bevy::window::WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: WgpuSettings {
                    backends: None,
                    ..default()
                }
                .into(),
                ..default()
            })
            .set(AssetPlugin {
                file_path: crate::asset_root_path().to_string_lossy().into_owned(),
                ..default()
            })
            .disable::<bevy::winit::WinitPlugin>(),
    );
    app.insert_resource(crate::LaunchOptions {
        map: map.into(),
        map_explicit: true,
        seed,
        robots,
        headless: true,
        flat: true,
        ..Default::default()
    })
    .insert_resource(bevy::time::TimeUpdateStrategy::FixedTimesteps(1))
    .insert_resource(Time::<Fixed>::from_hz(90.0))
    .add_plugins(avian3d::prelude::PhysicsPlugins::default())
    .add_plugins((crate::GamePlugin, crate::ClientPlugin));
    app.finish();
    app.cleanup();
    app
}

/// Plays raid after raid on `map` for `secs` of game time (`client`: with
/// the visuals and sound).
pub fn raids(
    map: &str,
    seed: u64,
    client: bool,
    secs: f32,
    raid_secs: f32,
    every: f32,
    log: impl FnMut(&Census),
) -> Record {
    let mut app = if client {
        client_app(map, seed, true)
    } else {
        app(map, seed, true)
    };
    let mut driver = Raids::new(raid_secs);
    census::soak(&mut app, secs, every, |app, _| driver.drive(app), log)
}

/// `extraction soak [--map M] [--seed S] [--minutes N] [--raid-minutes R]
/// [--every SECS] [--out FILE] [--client]`: plays raid after raid headless for N
/// minutes of game time, writes a census a line (JSON) and prints the
/// verdict. Exits 1 on a leak.
pub fn cli(args: impl Iterator<Item = String>) -> AppExit {
    let mut map = "marrow_creek".to_string();
    let (mut seed, mut minutes, mut raid_minutes, mut every) = (1u64, 60.0f32, 8.0f32, 20.0f32);
    let mut out = None;
    let mut client = false;
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_default();
        match a.as_str() {
            "--map" => map = val(),
            "--seed" => seed = val().parse().unwrap_or(seed),
            "--minutes" => minutes = val().parse().unwrap_or(minutes),
            "--raid-minutes" => raid_minutes = val().parse().unwrap_or(raid_minutes),
            "--every" => every = val().parse().unwrap_or(every),
            "--out" => out = Some(std::path::PathBuf::from(val())),
            "--client" => client = true,
            other => {
                eprintln!("soak: unknown argument {other}");
                return AppExit::error();
            }
        }
    }
    let out =
        out.unwrap_or_else(|| std::path::PathBuf::from(format!("target/soak/{map}-{seed}.jsonl")));
    if let Some(d) = out.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut file = std::io::BufWriter::new(std::fs::File::create(&out).expect("soak output"));
    let started = std::time::Instant::now();
    let rec = raids(
        &map,
        seed,
        client,
        minutes * 60.0,
        raid_minutes * 60.0,
        every,
        |c| {
            use std::io::Write;
            let _ = writeln!(file, "{}", serde_json::to_string(c).unwrap_or_default());
            let _ = file.flush();
            eprintln!(
                "t {:6.0} s  {:9} raids {:3}  entities {:6}  bodies {:5}  awake {:4}  contacts {:5}  items {:5}  robots {:3}  voices {:3}  meshes {:5}  mats {:5}  log {:3}  rss {:6.0} MB  frame {:6.2} ms  save {:7}",
                c.t,
                c.phase,
                c.raids,
                c.entities,
                c.bodies,
                c.awake,
                c.contacts,
                c.items,
                c.robots,
                c.voices,
                c.meshes,
                c.materials,
                c.raid_log,
                c.rss_mb,
                c.tick_ms,
                c.save_bytes
            );
        },
    );
    println!(
        "soak {map} seed {seed}: {:.0} s wall -> {}\n{}",
        started.elapsed().as_secs_f32(),
        out.display(),
        rec.report()
    );
    if rec.leaks().is_empty() {
        AppExit::Success
    } else {
        AppExit::error()
    }
}
