//! Saving and loading: the Zone stays as you left it between sessions —
//! looted shelves stay bare, the cache you hid is still under the bridge,
//! your remains are where you fell — until the Surge resets it.
//!
//! The save is a RON file (see [`extraction_core::save`]) in the user's
//! data directory, written when a raid ends, every minute, and on quit.
//! Scenarios, screenshots and headless runs don't touch it unless given
//! `--save PATH`.

use avian3d::prelude::*;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use extraction_core::firearm::Magazine;
use extraction_core::save::{
    Career, LoadoutSave, PlacedItem, RemainsSave, SAVE_VERSION, SaveGame, SavedGun, SavedItem,
};
use std::path::PathBuf;

use crate::interact::body::{Backpack, BodySlot, InBackpack};
use crate::interact::{Held, InSocket, Socket, SocketKind, SocketOwner, Supported};
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::launch::LaunchOptions;
use crate::raid::truck::Stash;
use crate::raid::{Phase, Raid, Remains};
use crate::rig::LocalPlayer;
use crate::weapons::{Gun, Mag};

/// Where this session saves, if anywhere.
#[derive(Resource, Clone, Debug)]
pub struct SaveSlot(pub Option<PathBuf>);

/// A loaded save waiting to be put into the world (consumed at
/// PostStartup). `zone` is false when the save is for another map.
#[derive(Resource)]
pub struct Restore {
    pub save: SaveGame,
    pub zone: bool,
}

/// The memorial board and anything else carried across sessions verbatim.
#[derive(Resource, Default, Clone, Debug)]
pub struct Memorial(pub extraction_core::save::Memorial);

/// Ask for a save at the end of this frame.
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct SaveNow;

pub struct PersistPlugin;

impl Plugin for PersistPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Memorial>()
            .add_message::<SaveNow>()
            .add_systems(PreStartup, choose_slot)
            .add_systems(
                Startup,
                load_save
                    .after(crate::world::buildings::spawn_buildings)
                    .before(crate::items::loot::LootSpawnSystems),
            )
            .add_systems(
                PostStartup,
                apply_restore.after(crate::raid::truck::start_in_truck),
            )
            .add_systems(Update, (autosave, surge_reset_at_base))
            .add_systems(Last, (relocate, save_on_request_or_exit).chain());
    }
}

/// Set by `--data-dir` for the whole process (logs, settings, career).
static DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn set_data_dir(dir: PathBuf) {
    let _ = DATA_DIR.set(dir);
}

/// `$XDG_DATA_HOME/extraction` (or the platform equivalent), unless
/// `--data-dir` says otherwise.
pub fn data_dir() -> PathBuf {
    if let Some(d) = DATA_DIR.get() {
        return d.clone();
    }
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    #[cfg(windows)]
    let base = env("APPDATA").unwrap_or_else(|| PathBuf::from("."));
    #[cfg(target_os = "macos")]
    let base = env("HOME")
        .map(|h| h.join("Library/Application Support"))
        .unwrap_or_else(|| PathBuf::from("."));
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let base = env("XDG_DATA_HOME")
        .or_else(|| env("HOME").map(|h| h.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("extraction")
}

/// Where this session's career is saved: `--save PATH`, or `career.ron` in
/// the data folder for a normal game (tests, scenarios and screenshots
/// don't save).
pub fn career_path(opts: &LaunchOptions) -> Option<PathBuf> {
    match &opts.save {
        Some(p) => Some(p.clone()),
        None if opts.scenario.is_some()
            // Sims never touch the save, rendered (a trailer) or not.
            || opts.sim_run.is_some()
            || opts.screenshot.is_some()
            || opts.camera.is_some()
            || !owns_profile(opts)
            || opts.gallery.is_some()
            || opts.scene != crate::launch::SceneChoice::Raid =>
        {
            None
        }
        // The training yard is practice: nothing there is kept.
        None if opts.map == crate::camp::TRAINING_YARD => None,
        None => Some(profile(opts).join("career.ron")),
    }
}

/// The folder this session's career, `trained` flag and so on live in:
/// `--data-dir`, else the data folder.
pub fn profile(opts: &LaunchOptions) -> PathBuf {
    opts.data_dir.clone().unwrap_or_else(data_dir)
}

/// Whether this session may read and write its profile: a real game, or a
/// headless run given one of its own (a test). Sims and other headless
/// runs leave the player's alone.
pub fn owns_profile(opts: &LaunchOptions) -> bool {
    !opts.headless || opts.data_dir.is_some()
}

/// Where a new session should open, if the command line didn't say: the
/// yard for someone who has never played (no career, yard not done), else
/// the valley the career is headed for.
pub fn choose_start(opts: &mut LaunchOptions) {
    if opts.map_explicit {
        return;
    }
    if career_path(opts).is_some_and(|p| !p.exists()) && !trained(opts) {
        opts.map = crate::camp::TRAINING_YARD.into();
        return;
    }
    if opts.new_game {
        return;
    }
    if let Some(map) = career_destination(opts) {
        if crate::asset_root_path()
            .join("maps")
            .join(format!("{map}.ron"))
            .exists()
        {
            opts.map = map;
        }
    }
}

/// The valley the truck goes on to from the yard: the career's, or the
/// first.
pub fn next_valley(opts: &LaunchOptions) -> String {
    let mut o = opts.clone();
    o.map = "marrow_creek".into();
    career_destination(&o).unwrap_or(o.map)
}

/// Whether this player has been through (or skipped) the training yard.
pub fn trained(opts: &LaunchOptions) -> bool {
    profile(opts).join("trained").exists()
}

pub fn mark_trained(opts: &LaunchOptions) {
    let _ = std::fs::create_dir_all(profile(opts));
    let _ = std::fs::write(profile(opts).join("trained"), "yes\n");
}

/// A new career: the old one is put aside (`career.ron.old`, one deep) and
/// the training yard offered again.
pub fn burn_career(opts: &LaunchOptions) {
    if let Some(path) = career_path(opts) {
        let _ = std::fs::rename(&path, path.with_extension("ron.old"));
    }
    let _ = std::fs::remove_file(profile(opts).join("trained"));
}

/// The valley a saved career is headed for, if there is one.
pub fn career_destination(opts: &LaunchOptions) -> Option<String> {
    let path = career_path(opts)?;
    read_career(&path)
        .career
        .map(|c| c.map)
        .filter(|m| !m.is_empty())
}

/// What reading the career found. Reading changes nothing on disk:
/// [`load_save`] puts a damaged file aside, once.
#[derive(Default)]
pub(crate) struct CareerRead {
    pub career: Option<Career>,
    /// The career file is damaged (moved aside when the session loads).
    pub damaged: bool,
    /// Written by a newer build: leave it alone, save nothing.
    pub newer: Option<u32>,
    /// What to tell the player, if anything.
    pub notice: Option<String>,
}

/// The last good save, kept beside the career: each write moves the one
/// before it here.
pub(crate) fn backup_path(path: &std::path::Path) -> PathBuf {
    path.with_extension("ron.bak")
}

/// A career file read and mended, or why not.
fn read_one(path: &std::path::Path) -> Result<Career, (Option<u32>, String)> {
    let text = std::fs::read_to_string(path).map_err(|e| (None, e.to_string()))?;
    if let Some(v) = extraction_core::save::career_version(&text).filter(|v| *v > SAVE_VERSION) {
        return Err((
            Some(v),
            format!("save version {v} is newer than this build"),
        ));
    }
    let mut career = Career::from_ron(&text).map_err(|e| (None, e))?;
    let fixed = career.sanitize();
    for f in &fixed {
        warn!("{}: {f}", path.display());
    }
    Ok(career)
}

/// Reads a career, or brings in the saves of an older build (one file per
/// map, next to where the career goes) as one. A damaged career falls back
/// to the last good save beside it; a newer build's is left alone.
pub(crate) fn read_career(path: &std::path::Path) -> CareerRead {
    if path.exists() {
        return match read_one(path) {
            Ok(c) => CareerRead {
                career: Some(c),
                ..default()
            },
            Err((Some(v), e)) => {
                error!("{}: {e}; leaving it untouched", path.display());
                CareerRead {
                    newer: Some(v),
                    notice: Some(
                        "This career was saved by a newer version of the game. \
                         It's left as it is, and nothing is saved this session."
                            .into(),
                    ),
                    ..default()
                }
            }
            Err((None, e)) => {
                error!("{}: {e}", path.display());
                let bak = backup_path(path);
                match read_one(&bak) {
                    Ok(c) => {
                        warn!("going back to the last good save, {}", bak.display());
                        CareerRead {
                            career: Some(c),
                            damaged: true,
                            notice: Some(
                                "Your save was damaged. You're back where the save \
                                 before it left you."
                                    .into(),
                            ),
                            ..default()
                        }
                    }
                    Err(_) => CareerRead {
                        damaged: true,
                        notice: Some(format!(
                            "Your save was damaged and couldn't be read. A new career \
                             starts; the old file is kept as {}.",
                            bad_path(path)
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        )),
                        ..default()
                    },
                }
            }
        };
    }
    CareerRead {
        career: read_older_saves(path),
        ..default()
    }
}

/// Where a damaged career is put aside: `career.ron.bad`, or the first
/// free `career.ron.bad<N>` (a second damaged file never buries the first).
fn bad_path(path: &std::path::Path) -> PathBuf {
    let first = path.with_extension("ron.bad");
    if !first.exists() {
        return first;
    }
    (2..1000)
        .map(|n| path.with_extension(format!("ron.bad{n}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// An older build's saves, one file per map, as one career.
fn read_older_saves(path: &std::path::Path) -> Option<Career> {
    let dir = path.parent()?;
    let mut old: Vec<(std::time::SystemTime, SaveGame)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "ron"))
        .filter_map(|e| {
            let mut save = SaveGame::from_ron(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            save.sanitize();
            let when = e.metadata().ok()?.modified().ok()?;
            Some((when, save))
        })
        .collect();
    // Oldest first, so the newest save's stash and kit win.
    old.sort_by_key(|(when, _)| *when);
    let mut career: Option<Career> = None;
    for (_, save) in &old {
        match &mut career {
            Some(c) => c.store(save),
            None => career = Some(Career::from_game(save)),
        }
    }
    if let (Some(c), Some((_, newest))) = (&mut career, old.last()) {
        c.map = newest.zone.map.clone();
        info!(
            "brought {} older save(s) into one career ({} valleys)",
            old.len(),
            c.zones.len()
        );
    }
    career
}

/// The whole career in memory; this session's valley is written into it
/// on every save.
#[derive(Resource, Debug, Default)]
pub struct CareerState(pub Career);

/// Seconds a notice about the save stays on the radio and wrist.
const NOTICE_SECS: f32 = 60.0;

fn choose_slot(mut commands: Commands, opts: Res<LaunchOptions>) {
    let slot = career_path(&opts);
    if let Some(p) = &slot {
        info!("saving to {}", p.display());
    }
    commands.insert_resource(SaveSlot(slot));
}

fn load_save(
    mut commands: Commands,
    slot: Res<SaveSlot>,
    opts: Res<LaunchOptions>,
    mut raid: ResMut<Raid>,
    mut memorial: ResMut<Memorial>,
    mut logbook: ResMut<crate::raid::logbook::Logbook>,
    mut requests: ResMut<crate::raid::requests::Requests>,
    mut progression: ResMut<crate::progression::Progression>,
    mut firsts: ResMut<crate::onboarding::firsts::Firsts>,
    mut notes: ResMut<crate::raid::board::NotesRead>,
    mut epilogue: ResMut<crate::raid::epilogue::Epilogue>,
    mut safehouse: ResMut<crate::progression::safehouse::Safehouse>,
) {
    let Some(path) = &slot.0 else {
        // The yard keeps nothing, but the sign says your kit is in the
        // crate, so it is (the first playtest found it empty, and a pile of
        // every gun in the game on the ground instead).
        // (The onboarding bot's run too: it starts at the crate.)
        let first_run = opts.scenario.as_deref() == Some(crate::onboarding::bot::SCENARIO);
        if opts.map == crate::camp::TRAINING_YARD && (owns_profile(&opts) || first_run) {
            commands.insert_resource(Restore {
                save: fresh_save(&opts.map),
                zone: false,
            });
        }
        return;
    };
    let career = if opts.new_game {
        info!("new game: ignoring any existing save");
        None
    } else {
        let read = read_career(path);
        if read.damaged {
            // Kept for inspection; the next save writes a sound one.
            let bad = bad_path(path);
            match std::fs::rename(path, &bad) {
                Ok(()) => warn!("damaged career moved to {}", bad.display()),
                Err(e) => error!("couldn't move the damaged career aside: {e}"),
            }
        }
        if read.newer.is_some() {
            // Never overwrite what a newer build wrote.
            commands.insert_resource(SaveSlot(None));
        }
        if let Some(n) = &read.notice {
            // Long enough to be read on the wrist after the load.
            raid.log.push((n.clone(), NOTICE_SECS));
        }
        read.career
    };
    let mut career = career.unwrap_or_else(|| {
        info!("no career at {}; starting fresh", path.display());
        Career::from_game(&fresh_save(&opts.map))
    });
    career.map = opts.map.clone();
    let save = career.game(&opts.map);
    commands.insert_resource(CareerState(career));
    let zone = save.zone.map == opts.map && save.zone.rolled;
    if zone {
        commands.insert_resource(crate::destruction::Broken(
            save.zone.broken.iter().copied().collect(),
        ));
        commands.insert_resource(crate::destruction::breach::Breaches::from_save(
            &save.zone.holes,
            &save.zone.breaches,
        ));
    }
    raid.raids = save.stats.raids;
    raid.extractions = save.stats.extractions;
    raid.deaths = save.stats.deaths;
    raid.recalled = save.stats.recalled;
    notes.0 = save.stats.notes_read.clone();
    epilogue.done = save.stats.epilogue;
    raid.clock = save.zone.clock;
    raid.cycle = save.zone.cycle;
    memorial.0 = save.memorial.clone();
    logbook.written = save.logbook.clone();
    *requests = crate::raid::requests::Requests {
        number: save.requests.number,
        cache: save.requests.cache.clone(),
        cache_at: save.requests.cache_at.clone(),
    };
    if zone {
        safehouse.state = save.zone.safehouse.clone().unwrap_or_default();
    }
    progression.0 = save
        .progress
        .clone()
        .unwrap_or_else(|| progress_from_requests(&save.requests));
    firsts.0 = save.firsts.iter().cloned().collect();
    info!(
        "loaded save: {} items, cycle {}, {:.0} s to the Surge",
        save.item_count(),
        save.zone.cycle,
        raid.surge_in()
    );
    commands.insert_resource(Restore { save, zone });
}

/// A career from before the driver's jobs had tiers: every request it met
/// counts as a job done, and the one on the card stays on it.
fn progress_from_requests(
    r: &extraction_core::save::RequestsSave,
) -> extraction_core::progression::Progress {
    let mut p = extraction_core::progression::Progress::default();
    p.credit(r.number.saturating_sub(r.want.is_some() as u32));
    p.job = r
        .want
        .clone()
        .map(|(item, n)| extraction_core::progression::Job::Fetch { item, n });
    p
}

/// A new game: an empty Zone (rolled fresh) and a starter kit in the stash.
fn fresh_save(map: &str) -> SaveGame {
    let mut s = SaveGame::new(map);
    let item = |id: &str, rounds: u32, at: [f32; 3]| SavedItem {
        id: id.into(),
        condition: 0.75,
        rounds,
        charge_wh: -1.0,
        local: at,
        gun: None,
        inserted: Vec::new(),
        loads: Default::default(),
    };
    let mut pistol = item("pm_pistol", 0, [0.2, 0.05, -0.4]);
    pistol.inserted.push(item("pm_mag", 8, [0.0; 3]));
    let mut torch = item("flashlight", 0, [0.0, 0.05, 0.2]);
    torch.inserted.push(item("power_cell", 0, [0.0; 3]));
    s.stash = vec![
        pistol,
        torch,
        item("pm_mag", 8, [0.25, 0.05, -0.2]),
        item("ammo_9x18", 16, [-0.2, 0.05, -0.4]),
        item("radio", 0, [-0.2, 0.05, 0.0]),
        item("field_dressing", 0, [0.2, 0.05, 0.3]),
        item("canned_stew", 0, [-0.25, 0.05, 0.4]),
        item("cleaning_kit", 0, [0.0, 0.05, -0.1]),
        item("map_case", 0, [-0.3, 0.05, -0.3]),
        item("compass", 0, [0.3, 0.05, 0.0]),
        // To find anomalies before they find you: the clicker, and bolts
        // to throw where the air looks wrong.
        item("detector", 0, [0.12, 0.05, 0.42]),
        item("bolt", 0, [-0.1, 0.05, 0.1]),
        item("bolt", 0, [-0.05, 0.05, 0.1]),
        item("bolt", 0, [0.0, 0.05, 0.1]),
    ];
    s
}

/// Spawns one saved item (and what's seated in it) with physics on, at
/// `at`. Returns `None` for ids the catalog no longer knows.
pub(crate) fn restore_item(world: &mut World, saved: &SavedItem, at: Transform) -> Option<Entity> {
    let catalog = world.resource::<Catalog>().clone();
    let Some(kind) = catalog.kind(&saved.id) else {
        warn!("save: unknown item {:?}, skipped", saved.id);
        return None;
    };
    let mut queue = CommandQueue::default();
    let e = {
        let mut commands = Commands::new(&mut queue, world);
        spawn_item(
            &mut commands,
            &catalog,
            SpawnItem {
                kind,
                transform: at,
                condition: saved.condition,
            },
        )
    };
    queue.apply(world);
    world.flush();
    // A magazine holds no more than it holds, whatever the file says.
    let capacity = world.get::<Mag>(e).map(|m| m.capacity);
    if let Some(mut it) = world.get_mut::<Item>(e) {
        it.condition = saved.condition;
        it.rounds = capacity.map_or(saved.rounds, |c| saved.rounds.min(c));
        it.loads = saved.loads.clone();
        let rounds = it.rounds;
        it.loads.fit(rounds);
        if saved.charge_wh >= 0.0 {
            it.charge_wh = saved.charge_wh;
        }
    }
    if let (Some(g), Some(mut gun)) = (&saved.gun, world.get_mut::<Gun>(e)) {
        let f = &mut gun.firearm;
        if g.chambers.len() == f.chambers.len() {
            f.chambers = g.chambers.clone();
        }
        if g.hammers.len() == f.hammers_cocked.len() {
            f.hammers_cocked = g.hammers.clone();
        }
        f.internal_rounds = g.internal_rounds.min(f.def.internal_capacity);
        f.internal_loads = g.internal_loads.clone();
        let n = f.internal_rounds;
        f.internal_loads.fit(n);
        if g.chamber_loads.len() == f.chambers.len() {
            f.chamber_loads = g.chamber_loads.clone();
        }
        f.mode = g.mode.min(f.def.modes.len().saturating_sub(1));
        f.condition = g.condition;
        f.fouling = g.fouling;
        f.grime = g.grime;
        f.build = g.build;
    }
    for inner in &saved.inserted {
        let Some(m) = restore_item(world, inner, at) else {
            continue;
        };
        if world.get::<Mag>(m).is_some() {
            seat_magazine(world, e, m);
        } else {
            seat_in_mount(world, e, m);
        }
        if world.get::<InSocket>(m).is_none() {
            // It no longer fits (the gun's well or mount changed since):
            // it goes where its owner goes, not to the world's origin.
            warn!("save: {} no longer fits its {}", inner.id, saved.id);
            if let Some(mut o) = world.get_mut::<Unseated>(e) {
                o.0.push(m);
            } else {
                world.entity_mut(e).insert(Unseated(vec![m]));
            }
        }
    }
    Some(e)
}

/// Seated in an item's save but no longer fitting it: placed with the
/// item by whoever restores it ([`place_unseated`]).
#[derive(Component)]
pub(crate) struct Unseated(pub Vec<Entity>);

/// Whatever didn't fit back into `item` goes into `bag` with it, or, with
/// no bag, lies beside it.
pub(crate) fn place_unseated(world: &mut World, item: Entity, bag: Option<(Entity, Vec3)>) {
    let Some(Unseated(rest)) = world.entity_mut(item).take::<Unseated>() else {
        return;
    };
    for (i, m) in rest.into_iter().enumerate() {
        place_unseated(world, m, bag);
        match bag {
            Some((b, local)) => put_in_bag(world, m, b, local),
            None => {
                let at = world.get::<Transform>(item).copied().unwrap_or_default();
                if let Some(mut t) = world.get_mut::<Transform>(m) {
                    t.translation = at.translation + Vec3::new(0.1 * (i + 1) as f32, 0.05, 0.0);
                }
            }
        }
    }
}

/// Puts a magazine entity into a gun's well and the mechanism.
fn seat_magazine(world: &mut World, gun_e: Entity, mag_e: Entity) {
    let Some(well) = world.get::<Gun>(gun_e).and_then(|g| g.magwell) else {
        return;
    };
    let Some(mag) = world.get::<Mag>(mag_e).cloned() else {
        return;
    };
    let (rounds, loads) = world
        .get::<Item>(mag_e)
        .map_or((0, Default::default()), |i| (i.rounds, i.loads.clone()));
    let fits = world.get::<Socket>(well).is_some_and(|s| {
        s.occupant.is_none() && matches!(&s.kind, SocketKind::MagWell(f) if *f == mag.family)
    });
    if !fits {
        return;
    }
    let inserted = world.get_mut::<Gun>(gun_e).is_some_and(|mut g| {
        g.firearm
            .insert_magazine(Magazine {
                family: mag.family.clone(),
                capacity: mag.capacity,
                rounds,
                loads,
            })
            .is_ok()
    });
    if !inserted {
        return;
    }
    if let Some(mut s) = world.get_mut::<Socket>(well) {
        s.occupant = Some(mag_e);
    }
    world
        .entity_mut(mag_e)
        .remove::<(Held, Supported)>()
        .insert((
            InSocket { socket: well },
            RigidBodyDisabled,
            ColliderDisabled,
            ChildOf(well),
            Transform::from_xyz(0.0, -mag.height * 0.5, 0.0),
        ));
}

/// Puts an item into the first free mount of `owner` that takes it.
pub(crate) fn seat_in_mount(world: &mut World, owner: Entity, item: Entity) {
    let catalog = world.resource::<Catalog>().clone();
    let Some(mount) = world
        .get::<Item>(item)
        .and_then(|i| catalog.get(i.kind()).fits_mount())
    else {
        return;
    };
    let slot = world.get::<Children>(owner).and_then(|c| {
        c.iter().find(|s| {
            world
                .get::<Socket>(*s)
                .is_some_and(|s| s.occupant.is_none() && s.kind == SocketKind::Mount(mount))
        })
    });
    if let Some(slot) = slot {
        put_in_slot(world, item, slot);
    }
}

pub(crate) fn put_in_bag(world: &mut World, item: Entity, bag_e: Entity, local: Vec3) {
    let stash = world.get::<Stash>(bag_e).is_some();
    let Some(mut bag) = world.get_mut::<Backpack>(bag_e) else {
        return;
    };
    bag.contents.push(item);
    let mut local = local.clamp(-bag.half, bag.half);
    if stash {
        // On the crate's shelf, not down in its bottom (older saves).
        local.y = local.y.max(0.05);
    }
    let vis = if bag.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    world.entity_mut(item).insert((
        InBackpack { bag: bag_e },
        ChildOf(bag_e),
        Transform::from_translation(local),
        RigidBodyDisabled,
        ColliderDisabled,
        vis,
    ));
}

fn put_in_slot(world: &mut World, item: Entity, slot_e: Entity) {
    let Some(mut socket) = world.get_mut::<Socket>(slot_e) else {
        return;
    };
    if socket.occupant.is_some() {
        return;
    }
    socket.occupant = Some(item);
    let seat = socket.seat;
    world.entity_mut(item).insert((
        InSocket { socket: slot_e },
        RigidBodyDisabled,
        ColliderDisabled,
        ChildOf(slot_e),
        seat,
    ));
}

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

/// Puts a restored item into `bag` (at `local`), or, with no bag to take
/// it, lays it on the ground at `fallback`, never leaving it at the
/// world's origin. What no longer fits inside it goes along.
fn settle(world: &mut World, e: Entity, bag: Option<Entity>, local: Vec3, fallback: Vec3) {
    match bag {
        Some(b) => {
            put_in_bag(world, e, b, local);
            place_unseated(world, e, Some((b, local)));
        }
        None => {
            if let Some(mut t) = world.get_mut::<Transform>(e) {
                t.translation = fallback;
            }
            place_unseated(world, e, None);
        }
    }
}

/// Out in the Zone: the raid as it stands, to resume where you stood if
/// the game is closed (or crashes, or the headset's battery dies) mid-raid.
/// At base, or pulling out with the truck: nothing to resume.
fn raid_save(world: &mut World) -> Option<extraction_core::save::RaidSave> {
    use extraction_core::save::{RaidPhaseSave as P, RaidSave};
    let phase = match world.resource::<Raid>().phase {
        Phase::Out => P::Out,
        Phase::TruckCalled { rv, eta } => P::TruckCalled { rv, eta },
        Phase::TruckWaiting { rv, left } => P::TruckWaiting { rv, left },
        Phase::Down { .. } => P::Down,
        Phase::AtBase | Phase::Extracting { .. } => return None,
    };
    let t = *world
        .query_filtered::<&Transform, With<LocalPlayer>>()
        .iter(world)
        .next()?;
    Some(RaidSave {
        at: t.translation.to_array(),
        yaw: t.rotation.to_euler(EulerRot::YXZ).0,
        phase,
    })
}

/// Back out where you quit: the player stood where they were (nudged up
/// out of the ground, inside the map), the raid's phase and the truck's
/// timer as they were. Whatever robots were about, the director sends its
/// own (`docs/SOAK.md`, "Quitting mid-raid").
fn resume_raid(world: &mut World, r: &extraction_core::save::RaidSave) -> Option<Vec3> {
    use extraction_core::save::RaidPhaseSave as P;
    let zone = world.resource::<crate::world::Zone>().clone();
    let half = zone.map.size * 0.5 - 2.0;
    let mut at = v3(r.at);
    at.x = at.x.clamp(-half, half);
    at.z = at.z.clamp(-half, half);
    let ground = zone.ground(at.x, at.z);
    // Underground sites lie a few metres to some tens down; further than
    // that, or past the map's edge, is out of the world: back on the ground.
    if at.y < ground - 40.0 || at.x.abs() >= half || at.z.abs() >= half {
        at.y = ground;
    }
    at.y += 0.1;
    let player = world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(world)
        .next()?;
    if let Some(mut t) = world.get_mut::<Transform>(player) {
        t.translation = at;
        t.rotation = Quat::from_rotation_y(r.yaw);
    }
    let mut raid = world.resource_mut::<Raid>();
    raid.phase = match r.phase {
        P::Out => Phase::Out,
        P::TruckCalled { rv, eta } => Phase::TruckCalled { rv, eta },
        P::TruckWaiting { rv, left } => Phase::TruckWaiting { rv, left },
        // Down is down: the convoy's order goes out as it would have.
        P::Down => Phase::Down { left: 1.0 },
    };
    raid.say("Back where you were. The raid goes on.");
    info!("resuming the raid at {at:.1}");
    Some(at)
}

fn apply_restore(world: &mut World) {
    let Some(Restore { save, zone }) = world.remove_resource::<Restore>() else {
        return;
    };
    // Mid-raid: back out there first, so what was in your hands lands at
    // your feet out there (the stash is back at base).
    let resumed = match (&save.zone.raid, zone) {
        (Some(r), true) => resume_raid(world, r).is_some(),
        _ => false,
    };
    let stash = world
        .query_filtered::<Entity, With<Stash>>()
        .iter(world)
        .next();
    let player = world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(world)
        .next();
    // Where a thing with nowhere to go is laid down: at your feet.
    let feet = player
        .and_then(|p| world.get::<Transform>(p))
        .map(|t| t.translation + Vec3::new(0.3, 0.3, 0.3))
        .unwrap_or(Vec3::Y);
    let mut placed = 0;
    // The stash, and anything that was in your hands (at your feet if
    // the raid resumes).
    for (saved, held) in save
        .stash
        .iter()
        .map(|s| (s, false))
        .chain(save.loadout.hands.iter().map(|s| (s, true)))
    {
        let Some(e) = restore_item(world, saved, Transform::IDENTITY) else {
            continue;
        };
        let bag = if held && resumed { None } else { stash };
        settle(world, e, bag, v3(saved.local), feet);
        placed += 1;
    }
    // What you were wearing.
    let slots: Vec<(Entity, usize)> = match player {
        Some(p) => world
            .query::<(Entity, &BodySlot, &SocketOwner)>()
            .iter(world)
            .filter(|(_, _, o)| o.0 == p)
            .map(|(e, s, _)| (e, s.0))
            .collect(),
        None => Vec::new(),
    };
    let pack = player.and_then(|p| {
        world
            .query::<(Entity, &Backpack)>()
            .iter(world)
            .find(|(_, b)| b.owner == p && b.carried)
            .map(|(e, _)| e)
    });
    for (index, saved) in &save.loadout.slots {
        let Some(e) = restore_item(world, saved, Transform::IDENTITY) else {
            continue;
        };
        let free = slots.iter().find(|(s, i)| {
            i == index
                && world
                    .get::<Socket>(*s)
                    .is_some_and(|s| s.occupant.is_none())
        });
        match free {
            Some((slot_e, _)) => {
                put_in_slot(world, e, *slot_e);
                place_unseated(world, e, stash.map(|s| (s, Vec3::ZERO)));
            }
            // The slot's gone or taken: the stash has room.
            None => settle(world, e, stash, Vec3::ZERO, feet),
        }
        placed += 1;
    }
    for saved in &save.loadout.backpack {
        let Some(e) = restore_item(world, saved, Transform::IDENTITY) else {
            continue;
        };
        settle(world, e, pack.or(stash), v3(saved.local), feet);
        placed += 1;
    }
    if zone {
        for p in &save.zone.items {
            let t = Transform::from_translation(v3(p.at)).with_rotation(Quat::from_array(p.rot));
            if let Some(e) = restore_item(world, &p.item, t) {
                place_unseated(world, e, None);
                placed += 1;
            }
        }
        for w in &save.zone.wrecks {
            crate::robots::heavy::restore_wreck(world, w);
        }
        world.insert_resource(crate::robots::memory::ZoneMind {
            memory: save.zone.memory.clone().unwrap_or_default(),
            ..default()
        });
        for r in &save.zone.remains {
            let bag = spawn_remains(world, v3(r.at));
            for saved in &r.items {
                if let Some(e) = restore_item(world, saved, Transform::IDENTITY) {
                    settle(world, e, Some(bag), v3(saved.local), v3(r.at));
                    placed += 1;
                }
            }
        }
    }
    info!("restored {placed} items");
}

/// An empty remains bag at a spot (the death drop fills its own).
pub fn spawn_remains(world: &mut World, at: Vec3) -> Entity {
    let owner = world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(world)
        .next()
        .unwrap_or(Entity::PLACEHOLDER);
    world
        .spawn((
            Name::new("Remains"),
            Remains { owner },
            Backpack {
                owner,
                open: true,
                model: extraction_core::inventory::Backpack {
                    capacity: 10_000.0,
                    mass: 0.0,
                },
                contents: Vec::new(),
                carried: false,
                half: Vec3::new(0.4, 0.2, 0.9),
            },
            Transform::from_translation(at),
            Visibility::default(),
        ))
        .id()
}

/// One item, as saved, with whatever is seated in it.
pub(crate) fn snapshot_item(world: &World, e: Entity, local: Vec3) -> Option<SavedItem> {
    let catalog = world.resource::<Catalog>();
    let item = world.get::<Item>(e)?;
    let def = catalog.get(item.kind());
    let mut saved = SavedItem {
        id: def.id.clone(),
        condition: item.condition,
        rounds: item.rounds,
        charge_wh: item.charge_wh,
        local: local.to_array(),
        gun: None,
        inserted: Vec::new(),
        loads: item.loads.clone(),
    };
    if let Some(gun) = world.get::<Gun>(e) {
        let f = &gun.firearm;
        saved.gun = Some(SavedGun {
            chambers: f.chambers.clone(),
            hammers: f.hammers_cocked.clone(),
            internal_rounds: f.internal_rounds,
            mode: f.mode,
            condition: f.condition,
            fouling: f.fouling,
            grime: f.grime,
            internal_loads: f.internal_loads.clone(),
            chamber_loads: f.chamber_loads.clone(),
            build: f.build,
        });
        saved.condition = f.condition;
        if let Some(m) = gun
            .magwell
            .and_then(|w| world.get::<Socket>(w))
            .and_then(|s| s.occupant)
        {
            if let Some(mut inner) = snapshot_item(world, m, Vec3::ZERO) {
                // The mechanism's count is the live one while seated.
                if let Some(mag) = &f.magazine {
                    inner.rounds = mag.rounds;
                    inner.loads = mag.loads.clone();
                }
                saved.inserted.push(inner);
            }
        }
    }
    // Whatever sits in its mounts (scopes, rails, lights, cells, artifacts).
    if let Some(children) = world.get::<Children>(e) {
        for c in children.iter() {
            let Some(s) = world.get::<Socket>(c) else {
                continue;
            };
            if !matches!(s.kind, SocketKind::Mount(_)) {
                continue;
            }
            if let Some(inner) = s.occupant.and_then(|o| snapshot_item(world, o, Vec3::ZERO)) {
                saved.inserted.push(inner);
            }
        }
    }
    Some(saved)
}

/// The whole persistent state as it stands.
pub fn snapshot(world: &mut World) -> SaveGame {
    let opts = world.resource::<LaunchOptions>().clone();
    let mut save = SaveGame::new(&opts.map);
    {
        let raid = world.resource::<Raid>();
        save.stats.raids = raid.raids;
        save.stats.extractions = raid.extractions;
        save.stats.deaths = raid.deaths;
        save.stats.recalled = raid.recalled;
        save.zone.clock = raid.clock;
        save.zone.cycle = raid.cycle;
    }
    save.zone.raid = raid_save(world);
    save.zone.rolled = true;
    if let Some(b) = world.get_resource::<crate::destruction::Broken>() {
        let mut keys: Vec<[i32; 3]> = b.0.iter().copied().collect();
        keys.sort();
        save.zone.broken = keys;
    }
    if let Some(b) = world.get_resource::<crate::destruction::breach::Breaches>() {
        save.zone.holes = b.to_save();
    }
    save.memorial = world.resource::<Memorial>().0.clone();
    save.logbook = world
        .get_resource::<crate::raid::logbook::Logbook>()
        .and_then(|l| l.written.clone());
    if let Some(r) = world.get_resource::<crate::raid::requests::Requests>() {
        save.requests = extraction_core::save::RequestsSave {
            number: r.number,
            want: None,
            cache: r.cache.clone(),
            cache_at: r.cache_at.clone(),
        };
    }
    save.progress = world
        .get_resource::<crate::progression::Progression>()
        .map(|p| p.0.clone());
    save.firsts = world
        .get_resource::<crate::onboarding::firsts::Firsts>()
        .map(|f| f.0.iter().cloned().collect())
        .unwrap_or_default();
    save.stats.notes_read = world
        .get_resource::<crate::raid::board::NotesRead>()
        .map(|n| n.0.clone())
        .unwrap_or_default();
    save.stats.epilogue = world
        .get_resource::<crate::raid::epilogue::Epilogue>()
        .is_some_and(|e| e.done);
    save.version = SAVE_VERSION;
    let player = world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(world)
        .next();
    let items: Vec<(
        Entity,
        Option<Entity>,
        Option<Entity>,
        bool,
        Transform,
        GlobalTransform,
    )> = world
        .query::<(
            Entity,
            &Item,
            Option<&InBackpack>,
            Option<&InSocket>,
            Option<&Held>,
            &Transform,
            &GlobalTransform,
        )>()
        .iter(world)
        .map(|(e, _, b, s, h, t, gt)| {
            (
                e,
                b.map(|b| b.bag),
                s.map(|s| s.socket),
                h.is_some(),
                *t,
                *gt,
            )
        })
        .collect();
    let mut remains: std::collections::HashMap<Entity, RemainsSave> = default();
    let mut loadout = LoadoutSave::default();
    for (e, bag, socket, held, t, gt) in items {
        if let Some(s) = socket {
            if world.get::<BodySlot>(s).is_some() {
                let index = world.get::<BodySlot>(s).map_or(0, |b| b.0);
                if let Some(saved) = snapshot_item(world, e, Vec3::ZERO) {
                    loadout.slots.push((index, saved));
                }
            }
            // Anything else in a socket (a magazine in a well) is saved
            // with its owner.
            continue;
        }
        if let Some(b) = bag {
            // The safehouse's crate is saved with its valley (below).
            if world
                .get::<crate::progression::safehouse::SafehouseCrate>(b)
                .is_some()
            {
                continue;
            }
            // A cargo pod's contents go with the dirigible, saved or not.
            if world.get::<crate::robots::dirigible::CargoPod>(b).is_some() {
                continue;
            }
            let Some(saved) = snapshot_item(world, e, t.translation) else {
                continue;
            };
            let worn = world.get::<Backpack>(b).is_some_and(|bp| bp.carried);
            if world.get::<Remains>(b).is_some() {
                let at = world
                    .get::<GlobalTransform>(b)
                    .map_or(Vec3::ZERO, |g| g.translation());
                remains
                    .entry(b)
                    .or_insert_with(|| RemainsSave {
                        at: at.to_array(),
                        items: Vec::new(),
                    })
                    .items
                    .push(saved);
            } else if worn {
                loadout.backpack.push(saved);
            } else {
                // The stash, the bench tray: the truck's, all of it.
                save.stash.push(saved);
            }
            continue;
        }
        if held {
            if let Some(saved) = snapshot_item(world, e, Vec3::ZERO) {
                loadout.hands.push(saved);
            }
            continue;
        }
        // Loose in the Zone. Things that fell out of the world are gone.
        let p = gt.translation();
        if p.y < -50.0 || !p.is_finite() {
            continue;
        }
        if let Some(saved) = snapshot_item(world, e, Vec3::ZERO) {
            save.zone.items.push(PlacedItem {
                at: p.to_array(),
                rot: gt.compute_transform().rotation.to_array(),
                item: saved,
            });
        }
    }
    let _ = player;
    // Deployed cover is saved folded, where it stood.
    let walls: Vec<Vec3> = world
        .query_filtered::<&GlobalTransform, With<crate::gadgets::CoverWall>>()
        .iter(world)
        .map(|g| g.translation())
        .collect();
    for at in walls {
        save.zone.items.push(PlacedItem {
            at: at.to_array(),
            rot: [0.0, 0.0, 0.0, 1.0],
            item: SavedItem {
                id: "deployable_cover".into(),
                condition: 1.0,
                rounds: 0,
                charge_wh: 0.0,
                local: [0.0; 3],
                gun: None,
                inserted: Vec::new(),
                loads: Default::default(),
            },
        });
    }
    save.zone.remains = remains.into_values().collect();
    // The valley's safehouse, and what's in its crate.
    if let Some(house) = world
        .get_resource::<crate::progression::safehouse::Safehouse>()
        .cloned()
    {
        let mut state = house.state.clone();
        if let Some(bag) = house.crate_bag {
            let contents = world
                .get::<Backpack>(bag)
                .map(|b| b.contents.clone())
                .unwrap_or_default();
            let saved: Vec<SavedItem> = contents
                .iter()
                .filter_map(|e| {
                    let local = world
                        .get::<Transform>(*e)
                        .map_or(Vec3::ZERO, |t| t.translation);
                    snapshot_item(world, *e, local)
                })
                .collect();
            // A crate not yet furnished keeps what was saved in it.
            if !saved.is_empty() || state.stash.is_empty() {
                state.stash = saved;
            }
        }
        if state != extraction_core::safehouse::SafehouseSave::default() {
            save.zone.safehouse = Some(state);
        }
    }
    save.zone.wrecks = crate::robots::heavy::save_wrecks(world);
    save.zone.memory = world
        .get_resource::<crate::robots::memory::ZoneMind>()
        .map(|m| m.memory.clone())
        .filter(|m| !m.is_blank());
    save.loadout = loadout;
    save
}

pub(crate) fn write_career(path: &std::path::Path, career: &Career) {
    let text = match career.to_ron() {
        Ok(t) => t,
        Err(e) => {
            error!("save: {e}");
            return;
        }
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Write beside and rename, so a crash mid-write never eats the save;
    // the save before it is kept as the last good one, should this one
    // come back damaged (a full disk, a bad sector, a hand edit).
    let tmp = path.with_extension("ron.tmp");
    if let Err(e) = std::fs::write(&tmp, text) {
        error!("save to {}: {e}", tmp.display());
        return;
    }
    if path.exists() {
        let _ = std::fs::copy(path, backup_path(path));
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        error!("save to {}: {e}", path.display());
    }
}

fn autosave(time: Res<Time<Real>>, mut last: Local<f32>, mut out: MessageWriter<SaveNow>) {
    let now = time.elapsed_secs();
    if now - *last > 60.0 {
        *last = now;
        out.write(SaveNow);
    }
}

/// The truck is driving to another valley: save the career pointed there,
/// and end this session (`main` opens the game again in the new valley).
fn relocate(world: &mut World) {
    let Some(to) = world
        .resource_mut::<Messages<crate::raid::route::Relocate>>()
        .drain()
        .last()
        .map(|r| r.to)
    else {
        return;
    };
    if let Some(path) = world.resource::<SaveSlot>().0.clone() {
        finish_zone_reset(world);
        let save = snapshot(world);
        if let Some(mut career) = world.get_resource_mut::<CareerState>() {
            career.0.store(&save);
            career.0.map = to.clone();
            write_career(&path, &career.0);
        }
    }
    info!("driving to {to}");
    crate::request_restart(&to);
    if !world.resource::<LaunchOptions>().headless {
        world.write_message(AppExit::Success);
    }
}

fn save_on_request_or_exit(world: &mut World) {
    let asked = world.resource_mut::<Messages<SaveNow>>().drain().count() > 0;
    let exiting = world
        .get_resource::<Messages<AppExit>>()
        .is_some_and(|m| !m.is_empty());
    if !asked && !exiting {
        return;
    }
    // A reset under way at base: never save the Zone half-swept. Leaving,
    // finish it first (a hitch on the way out costs nothing); otherwise
    // its last step asks for the save.
    if world.contains_resource::<ZoneReset>() {
        if !exiting {
            return;
        }
        finish_zone_reset(world);
        world
            .resource_mut::<Messages<SaveNow>>()
            .drain()
            .for_each(drop);
    }
    let Some(path) = world.resource::<SaveSlot>().0.clone() else {
        return;
    };
    let save = snapshot(world);
    let n = save.item_count();
    let Some(mut career) = world.get_resource_mut::<CareerState>() else {
        return;
    };
    career.0.store(&save);
    write_career(&path, &career.0);
    debug!("saved {n} items");
}

/// Back at base after the Surge has been through: the Zone resets. Loose
/// items and remains are gone, fresh loot is rolled, the clock restarts.
fn surge_reset_at_base(world: &mut World) {
    // Under way: a step a frame (`ZoneReset`).
    if world.contains_resource::<ZoneReset>() {
        step_zone_reset(world);
        return;
    }
    let due = {
        let raid = world.resource::<Raid>();
        raid.phase == Phase::AtBase && raid.surge_in() <= 0.0
    };
    if !due {
        return;
    }
    let loose = loose_items(world);
    world.insert_resource(ZoneReset {
        step: 0,
        part: 0,
        loose,
        gone: 0,
    });
    step_zone_reset(world);
}

/// A Surge reset at base, a step a frame: done at once it was one frame
/// of 37 ms and more (the homecoming's hitch: the sweep 18, the mending 9,
/// the loot 7, then the next frame setting up what came), a judder in the
/// headset as the truck pulls in.
#[derive(Resource)]
struct ZoneReset {
    step: u32,
    /// The step's part (the mending's, the loot's).
    part: usize,
    loose: Vec<Entity>,
    gone: usize,
}

/// Runs a reset under way to its end now.
fn finish_zone_reset(world: &mut World) {
    for _ in 0..100_000 {
        if !world.contains_resource::<ZoneReset>() {
            return;
        }
        step_zone_reset(world);
    }
}

/// Loose items swept a frame (and fresh loot let fall).
const SWEEP_PER_FRAME: usize = 40;

fn step_zone_reset(world: &mut World) {
    let Some(mut r) = world.remove_resource::<ZoneReset>() else {
        return;
    };
    match r.step {
        0 => {
            let take = r.loose.len().min(SWEEP_PER_FRAME);
            for e in r.loose.drain(..take) {
                if let Ok(e) = world.get_entity_mut(e) {
                    e.despawn();
                    r.gone += 1;
                }
            }
            if r.loose.is_empty() {
                r.step = 1;
            }
        }
        1 => {
            r.gone += sweep_remains_and_wrecks(world);
            new_cycle(world);
            r.step = 2;
        }
        2 => {
            // The mending a part a frame (all at once, 9 ms).
            crate::destruction::surge_step(world, r.part);
            r.part += 1;
            if r.part >= crate::destruction::SURGE_STEPS {
                r.part = 0;
                r.step = 3;
            }
        }
        3 => {
            crate::anomaly::spawn_cycle_anomalies(world);
            r.step = 4;
        }
        4 => {
            // The loot a part a frame, each part's fresh items held still
            // and let fall a few at a time after: hundreds of new bodies
            // settling at once was the next judder (the swept collision
            // pass 19 ms).
            let before: std::collections::HashSet<Entity> = world
                .query_filtered::<Entity, With<Item>>()
                .iter(world)
                .collect();
            crate::items::loot::roll_zone_loot_step(world, r.part);
            let fresh: Vec<Entity> = world
                .query_filtered::<(Entity, &RigidBody), With<Item>>()
                .iter(world)
                .filter(|(e, b)| !before.contains(e) && **b == RigidBody::Dynamic)
                .map(|(e, _)| e)
                .collect();
            for e in &fresh {
                world.entity_mut(*e).insert(RigidBody::Static);
            }
            r.loose.extend(fresh);
            r.part += 1;
            if r.part >= crate::items::loot::ROLL_STEPS {
                r.part = 0;
                r.step = 5;
            }
        }
        5 => {
            let take = r.loose.len().min(SWEEP_PER_FRAME);
            for e in r.loose.drain(..take) {
                if let Ok(mut e) = world.get_entity_mut(e) {
                    e.insert(RigidBody::Dynamic);
                }
            }
            if r.loose.is_empty() {
                r.step = 6;
            }
        }
        _ => {
            finish_reset(world, r.gone);
            return;
        }
    }
    world.insert_resource(r);
}

/// Loose items: lying in the Zone, not in a bag, a socket or a hand.
fn loose_items(world: &mut World) -> Vec<Entity> {
    world
        .query_filtered::<Entity, (
            With<Item>,
            Without<InBackpack>,
            Without<InSocket>,
            Without<Held>,
        )>()
        .iter(world)
        .collect()
}

/// The dead's bags and the robots' wrecks; how many items went with them.
fn sweep_remains_and_wrecks(world: &mut World) -> usize {
    let bags: Vec<(Entity, Vec<Entity>)> = world
        .query_filtered::<(Entity, &Backpack), With<Remains>>()
        .iter(world)
        .map(|(e, b)| (e, b.contents.clone()))
        .collect();
    let mut gone = 0;
    for (bag, contents) in bags {
        gone += contents.len();
        world.despawn(bag);
    }
    crate::robots::heavy::clear_wrecks(world);
    crate::robots::wreck::clear_wrecks(world);
    gone
}

fn new_cycle(world: &mut World) {
    let mut raid = world.resource_mut::<Raid>();
    raid.cycle += 1;
    raid.clock = 0.0;
    raid.say("The Surge has passed. The Zone is different now.");
}

fn finish_reset(world: &mut World, gone: usize) {
    crate::progression::safehouse::relay_leftovers(world);
    let cycle = world.resource::<Raid>().cycle;
    info!("Surge reset: cycle {cycle}, {gone} items swept away");
    world.write_message(SaveNow);
}

/// Wipes and re-rolls the Zone, all at once. Also used when you sleep
/// through a Surge (at base the reset goes a step a frame instead,
/// `surge_reset_at_base`).
pub fn reset_zone(world: &mut World) {
    let mut gone = 0;
    for e in loose_items(world) {
        // Magazines in loose guns go with their gun (despawn is recursive).
        if let Ok(e) = world.get_entity_mut(e) {
            e.despawn();
            gone += 1;
        }
    }
    gone += sweep_remains_and_wrecks(world);
    new_cycle(world);
    crate::destruction::surge(world);
    crate::anomaly::spawn_cycle_anomalies(world);
    crate::items::loot::roll_zone_loot(world);
    finish_reset(world, gone);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raid::SURGE_AFTER;

    fn app_with_save(path: &std::path::Path) -> App {
        let opts = LaunchOptions {
            robots: false,
            save: Some(path.to_owned()),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        app
    }

    /// Runs a Surge reset at base through (a step a frame, `ZoneReset`).
    fn through_the_reset(app: &mut App) {
        app.update();
        for _ in 0..200 {
            if !app.world().contains_resource::<ZoneReset>() {
                break;
            }
            app.update();
        }
        assert!(
            !app.world().contains_resource::<ZoneReset>(),
            "the reset finished"
        );
        app.update();
    }

    fn count<F: bevy::ecs::query::QueryFilter>(world: &mut World) -> usize {
        world.query_filtered::<(), F>().iter(world).count()
    }

    fn stash_len(world: &mut World) -> usize {
        world
            .query_filtered::<&Backpack, With<Stash>>()
            .iter(world)
            .map(|b| b.contents.len())
            .sum()
    }

    fn temp_save(name: &str) -> PathBuf {
        // A folder each: a missing career makes the loader look around it
        // for older saves to bring in.
        let dir = std::env::temp_dir()
            .join(format!("extraction-test-{}", std::process::id()))
            .join(name);
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join(name);
        let _ = std::fs::remove_file(&p);
        p
    }

    /// Quitting mid-raid (or a crash, or a flat battery) resumes the raid
    /// where you stood, with the truck's timer as it was, instead of
    /// waking at base with everything you carried (an escape from any
    /// danger). At base, a save opens at base.
    #[test]
    fn quitting_mid_raid_resumes_where_you_stood() {
        let path = temp_save("mid_raid.ron");
        let mut a = app_with_save(&path);
        let zone = a.world().resource::<crate::world::Zone>().clone();
        let out_there = zone.on_ground(40.0, -60.0);
        let player = a
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(a.world())
            .next()
            .unwrap();
        a.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = out_there;
        {
            let mut raid = a.world_mut().resource_mut::<Raid>();
            raid.raids = 3;
            raid.clock = 700.0;
            raid.phase = Phase::TruckWaiting { rv: 1, left: 90.0 };
        }
        let raids = a.world().resource::<Raid>().raids;
        let save = snapshot(a.world_mut());
        let r = save.zone.raid.clone().expect("mid-raid is saved");
        assert_eq!(
            r.phase,
            extraction_core::save::RaidPhaseSave::TruckWaiting { rv: 1, left: 90.0 }
        );
        let mut career = a.world().resource::<CareerState>().0.clone();
        career.store(&save);
        write_career(&path, &career);

        let mut b = app_with_save(&path);
        let raid = b.world().resource::<Raid>();
        assert!(
            matches!(raid.phase, Phase::TruckWaiting { rv: 1, left } if left > 85.0),
            "the truck still waiting: {:?}",
            raid.phase
        );
        assert_eq!(raid.raids, raids, "the same raid, not a new one");
        let at = b
            .world_mut()
            .query_filtered::<&Transform, With<LocalPlayer>>()
            .iter(b.world())
            .next()
            .unwrap()
            .translation;
        assert!(
            at.xz().distance(out_there.xz()) < 1.0 && (at.y - out_there.y).abs() < 1.5,
            "back where they stood: {at} (was {out_there})"
        );
        // Home again, and saved at base: it opens at base.
        b.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
        let save = snapshot(b.world_mut());
        assert!(save.zone.raid.is_none());
        let _ = std::fs::remove_file(&path);
    }

    /// What the valley remembers of you is saved with it and back next
    /// time; a valley that remembers nothing saves nothing.
    #[test]
    fn the_zone_remembers_you_across_saves() {
        use extraction_core::zone_memory::RaidTrace;
        let path = temp_save("memory.ron");
        let mut a = app_with_save(&path);
        assert!(
            snapshot(a.world_mut()).zone.memory.is_none(),
            "a fresh valley remembers nothing"
        );
        let mut t = RaidTrace::default();
        t.walked(120.0, -40.0);
        t.kills.push(([150.0, -60.0], "Walker".into()));
        t.rv = Some([300.0, 10.0]);
        a.world_mut()
            .resource_mut::<crate::robots::memory::ZoneMind>()
            .memory
            .fold(&t);
        let remembered = a
            .world()
            .resource::<crate::robots::memory::ZoneMind>()
            .memory
            .clone();
        let save = snapshot(a.world_mut());
        assert_eq!(save.zone.memory.as_ref(), Some(&remembered));
        let mut career = a.world().resource::<CareerState>().0.clone();
        career.store(&save);
        write_career(&path, &career);
        let b = app_with_save(&path);
        assert_eq!(
            b.world()
                .resource::<crate::robots::memory::ZoneMind>()
                .memory,
            remembered,
            "back as it was"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_and_restore_round_trip() {
        let path = temp_save("round_trip.ron");
        let mut a = app_with_save(&path);
        let items_a = count::<With<Item>>(a.world_mut());
        let stash_a = stash_len(a.world_mut());
        assert!(stash_a >= 6, "a new game starts with a kit in the stash");
        a.world_mut().resource_mut::<Raid>().clock = 500.0;
        a.world_mut()
            .resource_mut::<crate::raid::logbook::Logbook>()
            .written = Some("RAID 3: HOME, 12 MIN".into());
        let save = snapshot(a.world_mut());
        assert_eq!(save.item_count(), items_a, "every item is saved once");
        let mut career = a.world().resource::<CareerState>().0.clone();
        career.store(&save);
        write_career(&path, &career);

        let mut b = app_with_save(&path);
        assert_eq!(count::<With<Item>>(b.world_mut()), items_a);
        assert_eq!(stash_len(b.world_mut()), stash_a);
        assert_eq!(b.world().resource::<Raid>().clock, 500.0);
        assert_eq!(
            b.world()
                .resource::<crate::raid::logbook::Logbook>()
                .written
                .as_deref(),
            Some("RAID 3: HOME, 12 MIN"),
            "the logbook keeps the last raid"
        );
        assert_eq!(
            *b.world().resource::<crate::raid::requests::Requests>(),
            a.world()
                .resource::<crate::raid::requests::Requests>()
                .clone(),
            "and the driver's request"
        );
        let loaded_guns = b
            .world_mut()
            .query::<&Gun>()
            .iter(b.world())
            .filter(|g| g.firearm.magazine.is_some())
            .count();
        assert!(loaded_guns >= 1, "the starter pistol keeps its magazine");
        let again = snapshot(b.world_mut());
        assert_eq!(again.item_count(), save.item_count());
        let _ = std::fs::remove_file(&path);
    }

    /// The class of the doubled tapes: anything that lays items when a
    /// valley loads, and that the save also keeps, doubles on every load.
    /// In each valley, a fresh load saved and loaded again (twice) has the
    /// same items by kind, a few seconds in.
    #[test]
    fn every_valley_loads_back_to_the_same_items() {
        fn by_kind(world: &mut World) -> std::collections::BTreeMap<String, usize> {
            let catalog = world.resource::<crate::items::Catalog>().clone();
            let mut m = std::collections::BTreeMap::new();
            for it in world.query::<&Item>().iter(world) {
                *m.entry(catalog.get(it.kind()).id.clone()).or_insert(0) += 1;
            }
            m
        }
        fn load(path: &std::path::Path, map: &str) -> App {
            let opts = LaunchOptions {
                robots: false,
                map: map.into(),
                save: Some(path.to_owned()),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..30 {
                app.update();
            }
            app
        }
        for (map, ..) in extraction_core::map::VALLEYS {
            let path = temp_save(&format!("twice_{map}.ron"));
            let mut a = load(&path, map);
            let first = by_kind(a.world_mut());
            for round in 1..=2 {
                let save = snapshot(a.world_mut());
                let mut career = a.world().resource::<CareerState>().0.clone();
                career.store(&save);
                write_career(&path, &career);
                a = load(&path, map);
                let now = by_kind(a.world_mut());
                let diff: Vec<_> = first
                    .keys()
                    .chain(now.keys())
                    .filter(|k| first.get(*k) != now.get(*k))
                    .map(|k| {
                        (
                            k,
                            first.get(k).copied().unwrap_or(0),
                            now.get(k).copied().unwrap_or(0),
                        )
                    })
                    .collect();
                assert!(
                    diff.is_empty(),
                    "{map}, load {round}: (kind, before, after) {diff:?}"
                );
            }
            let _ = std::fs::remove_file(&path);
        }
    }

    /// A pistol whose magazine has AP and then a tracer on top of ball,
    /// and a round in the chamber that's AP: saved and loaded, the rounds
    /// keep their loads and their order.
    #[test]
    fn a_mixed_magazine_survives_a_save() {
        use extraction_core::ammo::Load;
        let path = temp_save("mixed_mag.ron");
        let mut a = app_with_save(&path);
        let pistol = {
            let w = a.world_mut();
            let mut q = w.query::<(Entity, &Gun)>();
            q.iter(w)
                .find(|(_, g)| g.firearm.magazine.is_some())
                .map(|(e, _)| e)
                .expect("the starter pistol with its magazine")
        };
        let want = vec![Load::Ap, Load::Tracer];
        {
            let mut g = a.world_mut().get_mut::<Gun>(pistol).unwrap();
            let m = g.firearm.magazine.as_mut().unwrap();
            m.loads.0 = want.clone();
            g.firearm.chamber_loads[0] = Load::Ap;
        }
        a.update();
        let save = snapshot(a.world_mut());
        let mut career = a.world().resource::<CareerState>().0.clone();
        career.store(&save);
        write_career(&path, &career);

        let mut b = app_with_save(&path);
        let w = b.world_mut();
        let mut q = w.query::<&Gun>();
        let g = q
            .iter(w)
            .find(|g| g.firearm.magazine.is_some())
            .expect("the pistol came back");
        assert_eq!(g.firearm.magazine.as_ref().unwrap().loads.0, want);
        assert_eq!(g.firearm.chamber_load(0), Load::Ap);
    }

    #[test]
    fn an_older_per_map_save_becomes_the_career() {
        let path = temp_save("career.ron");
        let mut old = fresh_save("marrow_creek");
        old.stats.raids = 7;
        old.zone.cycle = 4;
        let text = old.to_ron().unwrap();
        std::fs::write(path.with_file_name("marrow_creek.ron"), text).unwrap();
        let mut app = app_with_save(&path);
        let raid = app.world().resource::<Raid>();
        assert_eq!(raid.raids, 7, "the record came across");
        assert_eq!(raid.cycle, 4, "and the Zone's cycle");
        assert!(stash_len(app.world_mut()) > 0, "and the stash");
    }

    /// A heavy's wreck stays where it fell between sessions (its plates
    /// and cell as they were) until the Surge sweeps it away.
    #[test]
    fn a_heavys_wreck_keeps_until_the_surge() {
        use crate::robots::heavy::{Heavy, HeavyPart, HeavySeat};
        use extraction_core::heavy::Slot;
        let path = temp_save("wreck.ron");
        let mut a = app_with_save(&path);
        let zone = a.world().resource::<crate::world::Zone>().clone();
        let catalog = a.world().resource::<crate::items::Catalog>().clone();
        let at = zone.on_ground(40.0, 230.0);
        let heavy = {
            let mut c = a.world_mut().commands();
            let e = crate::robots::heavy::spawn_heavy(
                &mut c,
                &zone,
                &catalog,
                at,
                crate::robots::Job {
                    kind: crate::robots::JobKind::Survey,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            );
            c.entity(e).remove::<crate::robots::Senses>();
            a.world_mut().flush();
            e
        };
        for _ in 0..30 {
            a.update();
        }
        // The left plate shot off, then two knees: down for good.
        let parts: Vec<(Entity, Slot)> = a
            .world_mut()
            .query::<(Entity, &HeavyPart, &crate::robots::RobotPart)>()
            .iter(a.world())
            .filter(|(_, _, p)| p.robot == heavy)
            .map(|(e, h, _)| (e, h.0))
            .collect();
        for (e, s) in &parts {
            if matches!(
                s,
                Slot::Bolt { plate: 1, .. } | Slot::Knee(0) | Slot::Knee(2)
            ) {
                a.world_mut()
                    .write_message(crate::weapons::ballistics::Hit {
                        blow: None,
                        entity: *e,
                        point: Vec3::ZERO,
                        normal: Vec3::Y,
                        velocity: Vec3::ZERO,
                        damage: 1000.0,
                        shooter: None,
                        mass_g: 0.0,
                        load: Default::default(),
                    });
                a.update();
            }
        }
        for _ in 0..180 {
            a.update();
        }
        let save = snapshot(a.world_mut());
        assert_eq!(save.zone.wrecks.len(), 1);
        let w = &save.zone.wrecks[0];
        assert_eq!(w.plates, [true, false, true]);
        assert!(w.cell);
        let mut career = a.world().resource::<CareerState>().0.clone();
        career.store(&save);
        write_career(&path, &career);

        let mut b = app_with_save(&path);
        let found: Vec<(Entity, Vec3, bool)> = b
            .world_mut()
            .query::<(Entity, &crate::robots::Robot, &Transform, &Heavy)>()
            .iter(b.world())
            .map(|(e, r, t, _)| (e, t.translation, r.wrecked))
            .collect();
        assert_eq!(found.len(), 1, "the wreck is back");
        let (e, pos, wrecked) = found[0];
        assert!(wrecked);
        assert!(
            Vec2::new(pos.x - w.at[0], pos.z - w.at[2]).length() < 0.5,
            "where it fell"
        );
        let seated: Vec<(Slot, bool)> = b
            .world_mut()
            .query::<(&HeavySeat, &crate::interact::Socket)>()
            .iter(b.world())
            .filter(|(s, sock)| s.robot == e && sock.occupant.is_some())
            .map(|(s, sock)| {
                let free = b
                    .world()
                    .get::<crate::interact::Grabbable>(sock.occupant.unwrap())
                    .is_some();
                (s.slot, free)
            })
            .collect();
        assert!(
            !seated.iter().any(|(s, _)| *s == Slot::Plate(1)),
            "{seated:?}"
        );
        assert!(seated.contains(&(Slot::Plate(0), true)), "{seated:?}");
        assert!(
            seated.contains(&(Slot::Cell, false)),
            "behind its hatch: {seated:?}"
        );
        // Saved again, it's the same wreck.
        let again = snapshot(b.world_mut()).zone.wrecks;
        assert_eq!(again.len(), 1);
        assert_eq!((again[0].plates, again[0].cell), (w.plates, w.cell));
        // The Surge takes it.
        b.world_mut().resource_mut::<Raid>().clock = SURGE_AFTER + 1.0;
        through_the_reset(&mut b);
        let left = b.world_mut().query::<&Heavy>().iter(b.world()).count();
        assert_eq!(left, 0, "swept away");
        let _ = std::fs::remove_file(&path);
    }

    /// Quitting while the reset at base is under way finishes it before
    /// saving: the save has the new cycle and the fresh loot, never a
    /// Zone half-swept. A save asked for mid-reset waits for its end.
    #[test]
    fn quitting_mid_reset_saves_it_finished() {
        let path = temp_save("surge_quit.ron");
        let mut a = app_with_save(&path);
        a.world_mut().resource_mut::<Raid>().clock = SURGE_AFTER + 1.0;
        // Started, not finished (a step a frame).
        a.update();
        assert!(a.world().contains_resource::<ZoneReset>(), "under way");
        a.world_mut().write_message(SaveNow);
        a.update();
        assert!(
            a.world().contains_resource::<ZoneReset>(),
            "a save asked for mid-reset doesn't hurry it"
        );
        a.world_mut().write_message(AppExit::Success);
        a.update();
        assert!(
            !a.world().contains_resource::<ZoneReset>(),
            "finished to leave"
        );
        drop(a);
        let mut b = app_with_save(&path);
        let raid = b.world().resource::<Raid>();
        assert_eq!(raid.cycle, 1, "the save has the new cycle");
        let loose = count::<(
            With<Item>,
            Without<InBackpack>,
            Without<InSocket>,
            Without<Held>,
        )>(b.world_mut());
        assert!(loose > 20, "and the fresh loot ({loose} loose)");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn surge_rerolls_the_zone_at_base() {
        let path = temp_save("surge.ron");
        let mut a = app_with_save(&path);
        let loose = |w: &mut World| {
            count::<(
                With<Item>,
                Without<InBackpack>,
                Without<InSocket>,
                Without<Held>,
            )>(w)
        };
        let before = loose(a.world_mut());
        assert!(before > 20, "the Zone has loot");
        a.world_mut().resource_mut::<Raid>().clock = SURGE_AFTER + 1.0;
        through_the_reset(&mut a);
        let raid = a.world().resource::<Raid>();
        assert_eq!(raid.cycle, 1);
        assert_eq!(raid.clock, 0.0);
        let after = loose(a.world_mut());
        assert!(after > 20, "fresh loot was rolled");
        let _ = std::fs::remove_file(&path);
    }
}
