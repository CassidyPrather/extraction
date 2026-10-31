//! What plays a soak run, and `extraction soak` itself.
//!
//! [`Raids`] is the cheap driver: it walks the player out of the truck's
//! pull-in and leaves them standing in the Zone with the Surge's clock
//! wound on, so a raid lasts `raid_secs`. The director sends its robots,
//! wildlife wander, the weather turns, then the Surge comes and the player
//! goes down in the open; they wake in the truck, the Zone resets (fresh
//! loot, the remains swept away) and out they go again. Each cycle is a
//! whole raid's worth of the world's own churn, with nobody tidying up.
//!
//! [`Career`] is the sim's raiding bot (`src/sim/raider.rs`) played raid
//! after raid in one process, as a player's evening is: kit up, loot,
//! fight, shoot, get home or die, and go again. Between raids what it
//! brought home goes in the stash (here: away) and it takes a fresh kit.

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

/// The sim's raider, raid after raid.
#[derive(Default)]
pub struct Career {
    at_base: u32,
    /// Raids the bot has finished.
    pub done: u32,
}

impl Career {
    pub fn drive(&mut self, app: &mut App) {
        use crate::sim::{raider::Raider, telemetry::SimLog};
        let world = app.world_mut();
        let finished = world.resource::<SimLog>().finished;
        let at_base = world.resource::<Raid>().phase == Phase::AtBase;
        if !(finished && at_base) {
            self.at_base = 0;
            return;
        }
        self.at_base += 1;
        if self.at_base != 90 * 4 {
            return;
        }
        self.done += 1;
        let Some(me) = world
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(world)
            .next()
        else {
            return;
        };
        // Home: what it carries goes in the stash (away, here), and a
        // fresh kit is laid out by the insertion point.
        let at = world.get::<GlobalTransform>(me).unwrap().translation();
        world.write_message(crate::raid::truck::DropEverything { player: me, at });
        crate::raid::drop_everything(world);
        let bags: Vec<Entity> = world
            .query_filtered::<(Entity, &crate::raid::Remains), ()>()
            .iter(world)
            .filter(|(_, r)| r.owner == me)
            .map(|(e, _)| e)
            .collect();
        for b in bags {
            world.despawn(b);
        }
        lay_out_kit(world);
        world.entity_mut(me).remove::<Raider>();
        world.resource_mut::<SimLog>().new_raid();
        let _ = world.run_system_cached(crate::sim::raider::attach_raider);
    }
}

/// What the raider kits up from: an AK, two full magazines, a box of
/// rounds, the radio and two dressings, by the insertion point.
fn lay_out_kit(world: &mut World) {
    use extraction_core::map::MarkerKind;
    let zone = world.resource::<Zone>().clone();
    let catalog = world.resource::<crate::items::Catalog>().clone();
    let Some(ins) = zone.marker(&MarkerKind::Insertion) else {
        return;
    };
    // What the last raid didn't take (it was down before it got to it, or
    // had no room) goes back in the stash: no pile growing raid on raid.
    let spot = Vec2::new(ins.at[0] + 3.5, ins.at[1] - 3.5);
    let left: Vec<Entity> = world
        .query_filtered::<(Entity, &GlobalTransform), (
            With<crate::items::Item>,
            Without<ChildOf>,
            Without<crate::interact::Held>,
        )>()
        .iter(world)
        .filter(|(_, g)| g.translation().xz().distance(spot) < 4.0)
        .map(|(e, _)| e)
        .collect();
    for e in left {
        world.despawn(e);
    }
    let kit: &[(&str, u32)] = &[
        ("ak74", 0),
        ("ak_mag", 30),
        ("ak_mag", 30),
        ("ammo_545", 30),
        ("radio", 0),
        ("field_dressing", 0),
        ("field_dressing", 0),
    ];
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut made = Vec::new();
    {
        let mut commands = Commands::new(&mut queue, world);
        for (i, (id, rounds)) in kit.iter().enumerate() {
            let Some(kind) = catalog.kind(id) else {
                continue;
            };
            let x = ins.at[0] + 2.5 + (i % 4) as f32 * 0.7;
            let z = ins.at[1] - 3.0 - (i / 4) as f32 * 0.9;
            let e = crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_xyz(x, zone.ground(x, z) + 0.25, z),
                    condition: 0.85,
                },
            );
            made.push((e, *rounds));
        }
    }
    queue.apply(world);
    for (e, rounds) in made {
        if rounds > 0 {
            if let Some(mut it) = world.get_mut::<crate::items::Item>(e) {
                it.rounds = rounds;
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

/// A soak run's game: `map`, robots on; with `career`, the sim's raider
/// at the controls, its telemetry in `dir`.
pub fn options(map: &str, seed: u64, career: Option<std::path::PathBuf>) -> crate::LaunchOptions {
    crate::LaunchOptions {
        map: map.into(),
        map_explicit: true,
        seed,
        robots: true,
        sim_run: career.as_ref().map(|_| "career".into()),
        sim_out: career,
        ..Default::default()
    }
}

/// A headless game for a soak run.
pub fn app(opts: crate::LaunchOptions) -> App {
    let mut app = crate::headless_app(opts, false);
    app.finish();
    app.cleanup();
    app
}

/// The whole game, visuals and sound included, with no window and no GPU:
/// what the client half makes (meshes, materials, voices) piles up here
/// as it would in a headset session.
pub fn client_app(opts: crate::LaunchOptions) -> App {
    use bevy::render::RenderPlugin;
    use bevy::render::settings::WgpuSettings;
    let mut app = App::new();
    app.set_error_handler(bevy::ecs::error::warn);
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
        headless: true,
        flat: true,
        ..opts
    })
    .insert_resource(bevy::time::TimeUpdateStrategy::FixedTimesteps(1))
    .insert_resource(Time::<Fixed>::from_hz(90.0))
    // Despawning a camera (the impostors' captures) needs this; without a
    // renderer its queue is never drained, a few bytes an entity.
    .add_plugins(bevy::render::sync_world::SyncWorldPlugin);
    // As `run` has it: sound, and system errors as warnings.
    // (`SOAK_NO_AUDIO=1`: without sound, to tell what grows because of it.)
    if std::env::var_os("SOAK_NO_AUDIO").is_none() {
        app.add_plugins(crate::audio::AudioPlugin);
    }
    app.add_plugins(avian3d::prelude::PhysicsPlugins::default())
        .add_plugins((crate::GamePlugin, crate::ClientPlugin));
    app.finish();
    app.cleanup();
    app
}

/// Plays raid after raid on `map` for `secs` of game time (`client`: with
/// the visuals and sound; `career`: the raiding bot plays, its telemetry
/// in that directory, rather than standing about until the Surge).
pub fn raids(
    map: &str,
    seed: u64,
    client: bool,
    career: Option<std::path::PathBuf>,
    secs: f32,
    raid_secs: f32,
    every: f32,
    log: impl FnMut(&Census),
) -> Record {
    let bot = career.is_some();
    let opts = options(map, seed, career);
    let mut app = if client { client_app(opts) } else { app(opts) };
    let mut raids = Raids::new(raid_secs);
    let mut career = Career::default();
    census::soak(
        &mut app,
        secs,
        every,
        |app, _| {
            if bot {
                career.drive(app)
            } else {
                raids.drive(app)
            }
        },
        log,
    )
}

/// `extraction soak [--map M] [--seed S] [--minutes N] [--raid-minutes R]
/// [--every SECS] [--out FILE] [--client] [--career]`: plays raid after
/// raid headless for N
/// minutes of game time, writes a census a line (JSON) and prints the
/// verdict. Exits 1 on a leak.
pub fn cli(args: impl Iterator<Item = String>) -> AppExit {
    let mut map = "marrow_creek".to_string();
    let (mut seed, mut minutes, mut raid_minutes, mut every) = (1u64, 60.0f32, 8.0f32, 20.0f32);
    let mut out = None;
    let mut client = false;
    let mut career = false;
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
            "--career" => career = true,
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
    let career = career.then(|| out.with_extension("sim"));
    let rec = raids(
        &map,
        seed,
        client,
        career,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Raid, Surge, down, home, the Zone reset, three times over: each time
    /// it's back at base after a reset the world holds no more than it did
    /// the time before (entities, bodies, items, contact pairs), and the
    /// gauge finds nothing piling up. The hours-long version is
    /// `extraction soak` (`docs/SOAK.md`).
    #[test]
    fn each_zone_reset_puts_the_world_back() {
        // No director: what robots happen to be out differs from one
        // reset to the next. (The CLI runs have them.)
        let mut app = app(crate::LaunchOptions {
            robots: false,
            ..options("marrow_creek", 3, None)
        });
        let mut raids = Raids::new(25.0);
        let rec = census::soak(&mut app, 140.0, 1.0, |app, _| raids.drive(app), |_| {});
        assert!(raids.raids >= 3, "{} raids", raids.raids);
        // The last census at base after each reset.
        let mut at_base: Vec<&Census> = Vec::new();
        for c in &rec.censuses {
            if c.phase == "AtBase" {
                match at_base.last() {
                    Some(l) if l.raids == c.raids => *at_base.last_mut().unwrap() = c,
                    _ => at_base.push(c),
                }
            }
        }
        assert!(at_base.len() >= 2, "{at_base:?}");
        let (a, b) = (at_base[1], at_base[at_base.len() - 1]);
        let grew = |what: &str, x: usize, y: usize, slack: usize| {
            assert!(
                y <= x + slack,
                "{what} grew from one reset to the next: {x} -> {y}\n{}",
                rec.report()
            );
        };
        // Each Surge rolls different loot, so the item count wanders:
        // hold it to a band, and what isn't loot to the reset before.
        let more_items = b.items.saturating_sub(a.items);
        grew("items", a.items, b.items, a.items / 5);
        grew(
            "bodies besides items",
            a.bodies - a.items,
            b.bodies - b.items,
            5,
        );
        grew("entities", a.entities, b.entities, 60 + 8 * more_items);
        grew("contact pairs", a.contacts, b.contacts, 80 + 4 * more_items);
        assert!(rec.leaks().is_empty(), "{}", rec.report());
        assert!(
            rec.censuses.iter().all(|c| c.astray.is_empty()),
            "{}",
            rec.report()
        );
    }
}
