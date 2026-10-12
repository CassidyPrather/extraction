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
    LoadoutSave, PlacedItem, RemainsSave, SAVE_VERSION, SaveGame, SavedGun, SavedItem,
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
            .add_systems(Last, save_on_request_or_exit);
    }
}

/// `$XDG_DATA_HOME/extraction` (or the platform equivalent).
pub fn data_dir() -> PathBuf {
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

fn choose_slot(mut commands: Commands, opts: Res<LaunchOptions>) {
    let slot = match &opts.save {
        Some(p) => Some(p.clone()),
        None if opts.scenario.is_some()
            || opts.screenshot.is_some()
            || opts.camera.is_some()
            || opts.headless
            || opts.gallery.is_some()
            || opts.scene != crate::launch::SceneChoice::Raid =>
        {
            None
        }
        None => Some(data_dir().join(format!("{}.ron", opts.map))),
    };
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
) {
    let Some(path) = &slot.0 else {
        return;
    };
    if opts.new_game {
        info!("new game: ignoring any existing save");
        commands.insert_resource(Restore {
            save: fresh_save(&opts.map),
            zone: false,
        });
        return;
    }
    let save = match std::fs::read_to_string(path) {
        Ok(text) => match SaveGame::from_ron(&text) {
            Ok(s) => s,
            Err(e) => {
                // Keep the broken file for inspection rather than overwrite it.
                let bad = path.with_extension("ron.bad");
                let _ = std::fs::rename(path, &bad);
                error!("{}: {e}; moved to {}", path.display(), bad.display());
                fresh_save(&opts.map)
            }
        },
        Err(_) => {
            info!("no save at {}; starting fresh", path.display());
            fresh_save(&opts.map)
        }
    };
    let zone = save.zone.map == opts.map && save.zone.rolled;
    raid.raids = save.stats.raids;
    raid.extractions = save.stats.extractions;
    raid.deaths = save.stats.deaths;
    raid.clock = save.zone.clock;
    raid.cycle = save.zone.cycle;
    memorial.0 = save.memorial.clone();
    info!(
        "loaded save: {} items, cycle {}, {:.0} s to the Surge",
        save.item_count(),
        save.zone.cycle,
        raid.surge_in()
    );
    commands.insert_resource(Restore { save, zone });
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
    };
    let mut pistol = item("pm_pistol", 0, [0.2, -0.2, -0.4]);
    pistol.inserted.push(item("pm_mag", 8, [0.0; 3]));
    let mut torch = item("flashlight", 0, [0.0, -0.2, 0.2]);
    torch.inserted.push(item("power_cell", 0, [0.0; 3]));
    s.stash = vec![
        pistol,
        torch,
        item("pm_mag", 8, [0.25, -0.2, -0.2]),
        item("ammo_9x18", 16, [-0.2, -0.2, -0.4]),
        item("radio", 0, [-0.2, -0.2, 0.0]),
        item("field_dressing", 0, [0.2, -0.2, 0.3]),
        item("canned_stew", 0, [-0.25, -0.2, 0.4]),
        item("cleaning_kit", 0, [0.0, -0.2, -0.1]),
        item("map_case", 0, [-0.3, -0.25, -0.3]),
        item("compass", 0, [0.3, -0.25, 0.0]),
    ];
    s
}

/// Spawns one saved item (and what's seated in it) with physics on, at
/// `at`. Returns `None` for ids the catalog no longer knows.
fn restore_item(world: &mut World, saved: &SavedItem, at: Transform) -> Option<Entity> {
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
    if let Some(mut it) = world.get_mut::<Item>(e) {
        it.condition = saved.condition;
        it.rounds = saved.rounds;
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
        f.internal_rounds = g.internal_rounds;
        f.mode = g.mode.min(f.def.modes.len().saturating_sub(1));
        f.condition = g.condition;
        f.fouling = g.fouling;
    }
    for inner in &saved.inserted {
        let Some(m) = restore_item(world, inner, Transform::IDENTITY) else {
            continue;
        };
        if world.get::<Mag>(m).is_some() {
            seat_magazine(world, e, m);
        } else {
            seat_in_mount(world, e, m);
        }
    }
    Some(e)
}

/// Puts a magazine entity into a gun's well and the mechanism.
fn seat_magazine(world: &mut World, gun_e: Entity, mag_e: Entity) {
    let Some(well) = world.get::<Gun>(gun_e).and_then(|g| g.magwell) else {
        return;
    };
    let Some(mag) = world.get::<Mag>(mag_e).cloned() else {
        return;
    };
    let rounds = world.get::<Item>(mag_e).map_or(0, |i| i.rounds);
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
fn seat_in_mount(world: &mut World, owner: Entity, item: Entity) {
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

fn put_in_bag(world: &mut World, item: Entity, bag_e: Entity, local: Vec3) {
    let Some(mut bag) = world.get_mut::<Backpack>(bag_e) else {
        return;
    };
    bag.contents.push(item);
    let local = local.clamp(-bag.half, bag.half);
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

fn apply_restore(world: &mut World) {
    let Some(Restore { save, zone }) = world.remove_resource::<Restore>() else {
        return;
    };
    let stash = world
        .query_filtered::<Entity, With<Stash>>()
        .iter(world)
        .next();
    let player = world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(world)
        .next();
    let mut placed = 0;
    // The stash, and anything that was in your hands.
    for saved in save.stash.iter().chain(&save.loadout.hands) {
        let Some(e) = restore_item(world, saved, Transform::IDENTITY) else {
            continue;
        };
        match stash {
            Some(s) => put_in_bag(world, e, s, v3(saved.local)),
            None => {}
        }
        placed += 1;
    }
    // What you were wearing.
    if let Some(p) = player {
        let slots: Vec<(Entity, usize)> = world
            .query::<(Entity, &BodySlot, &SocketOwner)>()
            .iter(world)
            .filter(|(_, _, o)| o.0 == p)
            .map(|(e, s, _)| (e, s.0))
            .collect();
        let pack = world
            .query::<(Entity, &Backpack)>()
            .iter(world)
            .find(|(_, b)| b.owner == p && b.carried)
            .map(|(e, _)| e);
        for (index, saved) in &save.loadout.slots {
            let Some(e) = restore_item(world, saved, Transform::IDENTITY) else {
                continue;
            };
            match slots.iter().find(|(_, i)| i == index) {
                Some((slot_e, _)) => put_in_slot(world, e, *slot_e),
                None => {
                    if let Some(s) = stash {
                        put_in_bag(world, e, s, Vec3::ZERO);
                    }
                }
            }
            placed += 1;
        }
        for saved in &save.loadout.backpack {
            let Some(e) = restore_item(world, saved, Transform::IDENTITY) else {
                continue;
            };
            if let Some(b) = pack.or(stash) {
                put_in_bag(world, e, b, v3(saved.local));
            }
            placed += 1;
        }
    }
    if zone {
        for p in &save.zone.items {
            let t = Transform::from_translation(v3(p.at)).with_rotation(Quat::from_array(p.rot));
            if restore_item(world, &p.item, t).is_some() {
                placed += 1;
            }
        }
        for r in &save.zone.remains {
            let bag = spawn_remains(world, v3(r.at));
            for saved in &r.items {
                if let Some(e) = restore_item(world, saved, Transform::IDENTITY) {
                    put_in_bag(world, e, bag, v3(saved.local));
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
fn snapshot_item(world: &World, e: Entity, local: Vec3) -> Option<SavedItem> {
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
        save.zone.clock = raid.clock;
        save.zone.cycle = raid.cycle;
    }
    save.zone.rolled = true;
    save.memorial = world.resource::<Memorial>().0.clone();
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
    save.zone.remains = remains.into_values().collect();
    save.loadout = loadout;
    save
}

fn write_save(path: &std::path::Path, save: &SaveGame) {
    let text = match save.to_ron() {
        Ok(t) => t,
        Err(e) => {
            error!("save: {e}");
            return;
        }
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Write beside and rename, so a crash mid-write never eats the save.
    let tmp = path.with_extension("ron.tmp");
    if let Err(e) = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, path)) {
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

fn save_on_request_or_exit(world: &mut World) {
    let asked = world.resource_mut::<Messages<SaveNow>>().drain().count() > 0;
    let exiting = world
        .get_resource::<Messages<AppExit>>()
        .is_some_and(|m| !m.is_empty());
    if !asked && !exiting {
        return;
    }
    let Some(path) = world.resource::<SaveSlot>().0.clone() else {
        return;
    };
    let save = snapshot(world);
    write_save(&path, &save);
    debug!("saved {} items", save.item_count());
}

/// Back at base after the Surge has been through: the Zone resets. Loose
/// items and remains are gone, fresh loot is rolled, the clock restarts.
fn surge_reset_at_base(world: &mut World) {
    let due = {
        let raid = world.resource::<Raid>();
        raid.phase == Phase::AtBase && raid.surge_in() <= 0.0
    };
    if !due {
        return;
    }
    reset_zone(world);
}

/// Wipes and re-rolls the Zone. Also used when you sleep through a Surge.
pub fn reset_zone(world: &mut World) {
    let loose: Vec<Entity> = world
        .query_filtered::<Entity, (
            With<Item>,
            Without<InBackpack>,
            Without<InSocket>,
            Without<Held>,
        )>()
        .iter(world)
        .collect();
    let bags: Vec<(Entity, Vec<Entity>)> = world
        .query_filtered::<(Entity, &Backpack), With<Remains>>()
        .iter(world)
        .map(|(e, b)| (e, b.contents.clone()))
        .collect();
    let mut gone = 0;
    for e in loose {
        // Magazines in loose guns go with their gun (despawn is recursive).
        world.despawn(e);
        gone += 1;
    }
    for (bag, contents) in bags {
        gone += contents.len();
        world.despawn(bag);
    }
    let cycle = {
        let mut raid = world.resource_mut::<Raid>();
        raid.cycle += 1;
        raid.clock = 0.0;
        raid.say("The Surge has passed. The Zone is different now.");
        raid.cycle
    };
    crate::anomaly::spawn_cycle_anomalies(world);
    crate::items::loot::roll_zone_loot(world);
    info!("Surge reset: cycle {cycle}, {gone} items swept away");
    world.write_message(SaveNow);
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
        let dir = std::env::temp_dir().join(format!("extraction-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join(name);
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn save_and_restore_round_trip() {
        let path = temp_save("round_trip.ron");
        let mut a = app_with_save(&path);
        let items_a = count::<With<Item>>(a.world_mut());
        let stash_a = stash_len(a.world_mut());
        assert!(stash_a >= 6, "a new game starts with a kit in the stash");
        a.world_mut().resource_mut::<Raid>().clock = 500.0;
        let save = snapshot(a.world_mut());
        assert_eq!(save.item_count(), items_a, "every item is saved once");
        write_save(&path, &save);

        let mut b = app_with_save(&path);
        assert_eq!(count::<With<Item>>(b.world_mut()), items_a);
        assert_eq!(stash_len(b.world_mut()), stash_a);
        assert_eq!(b.world().resource::<Raid>().clock, 500.0);
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
        a.update();
        let raid = a.world().resource::<Raid>();
        assert_eq!(raid.cycle, 1);
        assert_eq!(raid.clock, 0.0);
        let after = loose(a.world_mut());
        assert!(after > 20, "fresh loot was rolled");
        let _ = std::fs::remove_file(&path);
    }
}
