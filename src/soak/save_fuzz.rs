//! Save and load at awkward moments, and from mangled files (see
//! `docs/SOAK.md`, "Save and load").
//!
//! Two kinds of check. Every frame of a scripted run of awkward moments
//! (a reload, a stow, a throw, going down, the Surge) the world is taken
//! apart: every item is in exactly one place, and the save holds every
//! item exactly once. At the moments that matter the save is written and
//! loaded into a fresh game, which must hold the same things. Then
//! mangled careers: damaged files, a newer build's, and parseable poison
//! (NaN, unknown items, magazines that no longer fit): the game never
//! panics, never leaves anything at the world's origin or out of the
//! world, and tells the player when it couldn't keep what was there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use extraction_core::save::{Career, SAVE_VERSION, SavedItem};

use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{HandAction, Held, InSocket, Socket, SocketKind, SocketOwner};
use crate::items::{Catalog, Item};
use crate::launch::LaunchOptions;
use crate::persist::{CareerState, SaveSlot, snapshot, write_career};
use crate::raid::truck::Stash;
use crate::raid::{Phase, Raid};
use crate::rig::{LocalPlayer, Side};
use crate::weapons::{Gun, Mag};

/// A profile folder of its own for each test.
fn profile(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("extraction-savefuzz-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("career.ron")
}

fn app_on(path: &Path, map: &str) -> App {
    let opts = LaunchOptions {
        robots: false,
        map: map.into(),
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

fn app(path: &Path) -> App {
    app_on(path, "marrow_creek")
}

/// Writes the world's state as the game does.
fn save(app: &mut App, path: &Path) {
    let s = snapshot(app.world_mut());
    let mut career = app.world().resource::<CareerState>().0.clone();
    career.store(&s);
    write_career(path, &career);
}

fn player(world: &mut World) -> Entity {
    world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .single(world)
        .unwrap()
}

fn said(app: &App, text: &str) -> bool {
    app.world()
        .resource::<Raid>()
        .log
        .iter()
        .any(|(l, _)| l.contains(text))
}

/// Items that are deliberately not in the save: a cargo pod's load goes
/// with its dirigible, a heavy's plates and cell with its wreck, and what
/// fell out of the world is gone. The safehouse crate's are saved with
/// the valley (counted from there).
fn unsaved(world: &World, e: Entity) -> bool {
    if let Some(b) = world.get::<InBackpack>(e) {
        return world
            .get::<crate::robots::dirigible::CargoPod>(b.bag)
            .is_some()
            || world
                .get::<crate::progression::safehouse::SafehouseCrate>(b.bag)
                .is_some();
    }
    if let Some(s) = world.get::<InSocket>(e) {
        // Seated in something that isn't an item or a body: a robot's.
        let owner = world.get::<SocketOwner>(s.socket).map(|o| o.0);
        let body = world
            .get::<crate::interact::body::BodySlot>(s.socket)
            .is_some();
        return !body && owner.is_none_or(|o| world.get::<Item>(o).is_none());
    }
    let p = world
        .get::<GlobalTransform>(e)
        .map_or(Vec3::NAN, |g| g.translation());
    !p.is_finite() || p.y < -50.0
}

/// Takes the world apart: every item in exactly one place, and the
/// snapshot holding each saved item exactly once. Returns the snapshot's
/// item count. `when` names the moment for the failure message.
fn check_world(world: &mut World, when: &str) -> usize {
    let items: Vec<Entity> = world
        .query_filtered::<Entity, With<Item>>()
        .iter(world)
        .collect();
    // Where each item is claimed to be, from the containers' side.
    let mut claimed: HashMap<Entity, Vec<String>> = HashMap::new();
    for (bag_e, bag) in world.query::<(Entity, &Backpack)>().iter(world) {
        for c in &bag.contents {
            claimed.entry(*c).or_default().push(format!("bag {bag_e}"));
        }
    }
    for (s_e, s) in world.query::<(Entity, &Socket)>().iter(world) {
        if let Some(o) = s.occupant {
            claimed.entry(o).or_default().push(format!("socket {s_e}"));
        }
    }
    for h in world.query::<&crate::interact::Hand>().iter(world) {
        if let Some(o) = h.holding {
            claimed.entry(o).or_default().push("hand".into());
        }
    }
    let mut problems = Vec::new();
    for &e in &items {
        let c = claimed.get(&e).cloned().unwrap_or_default();
        if c.len() > 1 {
            problems.push(format!("{e} is in {} places: {c:?}", c.len()));
        }
        let (bag, sock, held) = (
            world.get::<InBackpack>(e).map(|b| b.bag),
            world.get::<InSocket>(e).map(|s| s.socket),
            world.get::<Held>(e).is_some(),
        );
        let marks = bag.is_some() as u8 + sock.is_some() as u8 + held as u8;
        if marks > 1 {
            problems.push(format!(
                "{e} is marked in {marks} places (bag {bag:?}, socket {sock:?}, held {held})"
            ));
        }
        if let Some(b) = bag {
            if !c.iter().any(|w| w == &format!("bag {b}")) {
                problems.push(format!("{e} says it's in bag {b}, which doesn't list it"));
            }
        }
        if let Some(s) = sock {
            if !c.iter().any(|w| w == &format!("socket {s}")) {
                problems.push(format!(
                    "{e} says it's in socket {s}, which holds something else"
                ));
            }
        }
        let p = world
            .get::<GlobalTransform>(e)
            .map_or(Vec3::NAN, |g| g.translation());
        if !p.is_finite() {
            problems.push(format!("{e} is at {p}"));
        }
    }
    // A container listing something that isn't an item any more.
    for (e, c) in &claimed {
        if world.get_entity(*e).is_err() {
            problems.push(format!("{c:?} lists {e}, which is gone"));
        }
    }
    // A loaded gun's well and its mechanism agree.
    for (e, gun) in world.query::<(Entity, &Gun)>().iter(world) {
        let seated = gun
            .magwell
            .and_then(|w| world.get::<Socket>(w))
            .and_then(|s| s.occupant)
            .is_some();
        if seated != gun.firearm.magazine.is_some() {
            problems.push(format!(
                "gun {e}: magazine in the well {seated}, in the mechanism {}",
                gun.firearm.magazine.is_some()
            ));
        }
    }
    let expected = items.iter().filter(|e| !unsaved(world, **e)).count();
    let s = snapshot(world);
    let house = s.zone.safehouse.as_ref().map_or(0, |h| {
        fn n(i: &SavedItem) -> usize {
            1 + i.inserted.iter().map(n).sum::<usize>()
        }
        h.stash.iter().map(n).sum::<usize>()
    });
    let crated = items
        .iter()
        .filter(|e| {
            world.get::<InBackpack>(**e).is_some_and(|b| {
                world
                    .get::<crate::progression::safehouse::SafehouseCrate>(b.bag)
                    .is_some()
            })
        })
        .count();
    if s.item_count() != expected || (crated > 0 && house < crated) {
        problems.push(format!(
            "the save holds {} items (+{house} in the safehouse crate), the world {expected} (+{crated})",
            s.item_count()
        ));
    }
    assert!(problems.is_empty(), "{when}:\n  {}", problems.join("\n  "));
    s.item_count()
}

/// Every item by catalog id, and every live round, wherever it is.
fn census(world: &mut World) -> (HashMap<String, usize>, u32) {
    let catalog = world.resource::<Catalog>().clone();
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut rounds = 0;
    let list: Vec<Entity> = world
        .query_filtered::<Entity, With<Item>>()
        .iter(world)
        .filter(|e| !unsaved(world, *e))
        .collect();
    for e in list {
        let item = world.get::<Item>(e).unwrap();
        *ids.entry(catalog.get(item.kind()).id.clone()).or_default() += 1;
        let in_well = world.get::<InSocket>(e).is_some_and(|s| {
            world
                .get::<Socket>(s.socket)
                .is_some_and(|s| matches!(s.kind, SocketKind::MagWell(_)))
        });
        if !in_well {
            rounds += item.rounds;
        }
        if let Some(g) = world.get::<Gun>(e) {
            let f = &g.firearm;
            rounds += f.magazine.as_ref().map_or(0, |m| m.rounds) + f.internal_rounds;
            rounds += f
                .chambers
                .iter()
                .filter(|c| matches!(c, Some(extraction_core::firearm::Round::Live)))
                .count() as u32;
        }
    }
    (ids, rounds)
}

/// What you own, by catalog id: the stash and everything on you.
fn owned(world: &mut World) -> HashMap<String, usize> {
    fn add(i: &SavedItem, out: &mut HashMap<String, usize>) {
        *out.entry(i.id.clone()).or_default() += 1;
        for x in &i.inserted {
            add(x, out);
        }
    }
    let s = snapshot(world);
    let mut out = HashMap::new();
    for i in s
        .stash
        .iter()
        .chain(s.loadout.slots.iter().map(|s| &s.1))
        .chain(&s.loadout.backpack)
        .chain(&s.loadout.hands)
    {
        add(i, &mut out);
    }
    out
}

/// Saved past the Surge's coming: the game opens at base, where the Surge
/// has been and gone, so the Zone is re-rolled (by design). What you own
/// comes through whole.
fn round_trip_past_the_surge(a: &mut App, path: &Path) -> App {
    let before = owned(a.world_mut());
    save(a, path);
    let mut b = app(path);
    check_world(b.world_mut(), "past the Surge, loaded");
    assert_eq!(
        b.world().resource::<Raid>().cycle,
        1,
        "the Zone was re-rolled"
    );
    assert_eq!(owned(b.world_mut()), before, "what you own came through");
    b
}

/// Saves, loads into a fresh game, and checks it holds the same.
fn round_trip(a: &mut App, path: &Path, when: &str) -> App {
    let before = census(a.world_mut());
    save(a, path);
    let map = a.world().resource::<LaunchOptions>().map.clone();
    let mut b = app_on(path, &map);
    check_world(b.world_mut(), &format!("{when}, loaded"));
    let after = census(b.world_mut());
    let mut diff: Vec<String> = before
        .0
        .keys()
        .chain(after.0.keys())
        .filter(|k| before.0.get(*k) != after.0.get(*k))
        .map(|k| format!("{k}: {:?} -> {:?}", before.0.get(k), after.0.get(k)))
        .collect();
    diff.sort();
    diff.dedup();
    assert!(
        diff.is_empty(),
        "{when}: items changed over a save:\n  {}",
        diff.join("\n  ")
    );
    assert_eq!(before.1, after.1, "{when}: live rounds before and after");
    b
}

fn stash_items(world: &mut World) -> Vec<Entity> {
    world
        .query_filtered::<&Backpack, With<Stash>>()
        .iter(world)
        .flat_map(|b| b.contents.clone())
        .collect()
}

fn find(world: &mut World, id: &str) -> Option<Entity> {
    let catalog = world.resource::<Catalog>().clone();
    let all = stash_items(world);
    all.into_iter().find(|e| {
        world
            .get::<Item>(*e)
            .is_some_and(|i| catalog.get(i.kind()).id == id)
    })
}

/// Runs `n` frames, taking the world apart after each.
fn frames(app: &mut App, n: usize, when: &str) {
    for i in 0..n {
        app.update();
        check_world(app.world_mut(), &format!("{when}, frame {i}"));
    }
}

/// A raid's awkward moments, checked frame by frame, and saved and loaded
/// at each that matters.
#[test]
fn every_item_is_saved_once_through_a_raids_awkward_moments() {
    let path = profile("moments");
    let mut a = app(&path);
    check_world(a.world_mut(), "at base");
    let me = player(a.world_mut());
    // Out in the Zone.
    a.world_mut().resource_mut::<Raid>().phase = Phase::Out;
    frames(&mut a, 3, "out");
    // The pistol in the right hand, its spare magazine in the left.
    let pistol = find(a.world_mut(), "pm_pistol").expect("the starter pistol");
    let spare = find(a.world_mut(), "pm_mag").expect("its spare magazine");
    for (side, item) in [(Side::Right, pistol), (Side::Left, spare)] {
        a.world_mut().write_message(HandAction::Grab {
            player: me,
            side,
            item,
            point: None,
        });
    }
    frames(&mut a, 4, "pistol and spare in hand");
    let mut a = round_trip(&mut a, &path, "holding a pistol and a magazine");
    let me = player(a.world_mut());
    // The loaded game puts what was in your hands in the stash: take them
    // up again for the reload.
    let pistol = find(a.world_mut(), "pm_pistol").expect("the pistol came back");
    let well_mag = a
        .world()
        .get::<Gun>(pistol)
        .and_then(|g| g.magwell)
        .and_then(|w| a.world().get::<Socket>(w))
        .and_then(|s| s.occupant)
        .expect("the pistol's magazine came back in it");
    a.world_mut().resource_mut::<Raid>().phase = Phase::Out;
    a.world_mut().write_message(HandAction::Grab {
        player: me,
        side: Side::Right,
        item: pistol,
        point: None,
    });
    frames(&mut a, 3, "pistol taken up again");
    // Mid-reload: the left hand pulls the magazine out of the well, and
    // the save is taken with it half out.
    a.world_mut().write_message(HandAction::Grab {
        player: me,
        side: Side::Left,
        item: well_mag,
        point: None,
    });
    frames(&mut a, 3, "magazine pulled");
    let mut a = round_trip(&mut a, &path, "mid-reload, magazine out");
    let me = player(a.world_mut());
    a.world_mut().resource_mut::<Raid>().phase = Phase::Out;
    // Stow everything the stash has into the pack, one hand at a time.
    let kit: Vec<Entity> = stash_items(a.world_mut()).into_iter().take(6).collect();
    for item in kit {
        a.world_mut().write_message(HandAction::Grab {
            player: me,
            side: Side::Right,
            item,
            point: None,
        });
        frames(&mut a, 2, "taken from the stash");
        a.world_mut().write_message(HandAction::Stow {
            player: me,
            side: Side::Right,
        });
        frames(&mut a, 2, "stowed");
    }
    // Dropped, and thrown hard at the ground, mid-flight.
    let loose: Vec<Entity> = stash_items(a.world_mut()).into_iter().take(2).collect();
    for (i, item) in loose.into_iter().enumerate() {
        a.world_mut().write_message(HandAction::Grab {
            player: me,
            side: Side::Left,
            item,
            point: None,
        });
        frames(&mut a, 2, "taken to drop");
        a.world_mut().write_message(HandAction::Release {
            player: me,
            side: Side::Left,
            throw: if i == 0 { 0.0 } else { 40.0 },
        });
        frames(&mut a, 3, "dropped or thrown");
    }
    let mut a = round_trip(&mut a, &path, "kit in the pack, some on the ground");
    // The Surge, raging.
    a.world_mut().resource_mut::<Raid>().phase = Phase::Out;
    a.world_mut().resource_mut::<Raid>().clock = crate::raid::SURGE_AFTER + 10.0;
    frames(&mut a, 5, "the Surge raging");
    let mut a = round_trip_past_the_surge(&mut a, &path);
    // Going down, every frame of it.
    let me = player(a.world_mut());
    a.world_mut().resource_mut::<Raid>().phase = Phase::Out;
    a.world_mut().resource_mut::<Raid>().clock = 100.0;
    a.world_mut()
        .get_mut::<crate::health::Health>(me)
        .unwrap()
        .hp = 0.0;
    frames(&mut a, 30, "going down");
    let _ = round_trip(&mut a, &path, "down");
}

/// Packing up and driving to another valley saves on the way: the kit
/// arrives, and the valley left keeps its state.
#[test]
fn relocating_keeps_every_item() {
    let path = profile("relocate");
    let mut a = app(&path);
    let before = census(a.world_mut());
    let carried: usize = {
        let s = snapshot(a.world_mut());
        s.stash.len() + s.loadout.backpack.len() + s.loadout.slots.len() + s.loadout.hands.len()
    };
    a.world_mut().write_message(crate::raid::route::Relocate {
        to: "kettle_pass".into(),
    });
    a.update();
    let _ = crate::take_restart();
    let text = std::fs::read_to_string(&path).expect("saved on the way");
    let career = Career::from_ron(&text).unwrap();
    assert_eq!(career.map, "kettle_pass");
    let mut b = app_on(&path, "kettle_pass");
    check_world(b.world_mut(), "arrived");
    let s = snapshot(b.world_mut());
    assert_eq!(
        s.stash.len() + s.loadout.backpack.len() + s.loadout.slots.len() + s.loadout.hands.len(),
        carried,
        "the kit came along"
    );
    assert!(before.0.contains_key("pm_pistol"));
}

/// The career, written twice, so the last good save is kept beside it.
fn two_saves(path: &Path) -> (u32, u32) {
    let mut a = app(path);
    a.world_mut().resource_mut::<Raid>().raids = 3;
    save(&mut a, path);
    a.world_mut().resource_mut::<Raid>().raids = 5;
    save(&mut a, path);
    assert!(
        crate::persist::backup_path(path).exists(),
        "the save before is kept"
    );
    (3, 5)
}

/// A career cut short (a crash mid-write on a filesystem without atomic
/// renames, a full disk) goes back to the save before it, says so, and
/// keeps the damaged file.
#[test]
fn a_damaged_career_goes_back_to_the_last_good_save() {
    let path = profile("damaged");
    let (older, _) = two_saves(&path);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, &text[..text.len() / 2]).unwrap();
    let mut b = app(&path);
    assert_eq!(b.world().resource::<Raid>().raids, older, "the save before");
    assert!(said(&b, "damaged"), "and the player is told");
    assert!(
        path.with_extension("ron.bad").exists(),
        "the damaged file is kept"
    );
    assert!(
        stash_items(b.world_mut()).len() > 5,
        "the stash came back with it"
    );
    // The next save is a sound one again.
    save(&mut b, &path);
    assert!(Career::from_ron(&std::fs::read_to_string(&path).unwrap()).is_ok());
}

/// With nothing to go back to, a new career starts, the player is told,
/// and the damaged file is kept (a second never buries the first).
#[test]
fn a_damaged_career_with_no_backup_is_kept_and_reported() {
    let path = profile("damaged_alone");
    std::fs::write(&path, "Career(version: 1, map: \"marrow_cr").unwrap();
    let b = app(&path);
    assert!(
        said(&b, "couldn't be read"),
        "{:?}",
        b.world().resource::<Raid>().log
    );
    assert!(path.with_extension("ron.bad").exists());
    drop(b);
    std::fs::write(&path, [0xff, 0xfe, 0x00, 0x13]).unwrap();
    let _b = app(&path);
    assert!(path.with_extension("ron.bad").exists());
    assert!(
        path.with_extension("ron.bad2").exists(),
        "the first is kept"
    );
}

/// A career a newer build wrote is never touched: this session saves
/// nothing, and says so.
#[test]
fn a_newer_builds_career_is_never_overwritten() {
    let path = profile("newer");
    let mut a = app(&path);
    save(&mut a, &path);
    drop(a);
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace(
            &format!("version: {SAVE_VERSION},"),
            &format!("version: {},", SAVE_VERSION + 1),
        )
        .replace("stash:", "rucksacks: [Shiny(2)], stash:");
    std::fs::write(&path, &text).unwrap();
    let mut b = app(&path);
    assert!(said(&b, "newer version"));
    assert!(
        b.world().resource::<SaveSlot>().0.is_none(),
        "saves nothing"
    );
    // The autosave and quitting both leave it alone.
    b.world_mut().write_message(crate::persist::SaveNow);
    b.world_mut().write_message(AppExit::Success);
    b.update();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

/// Poison a parser lets through, in a real career: NaN and far-off
/// positions, an item the catalog doesn't know, a magazine of the wrong
/// family in a gun, more rounds than a magazine holds, two things for one
/// body slot and a slot that doesn't exist. Nothing panics, nothing ends
/// up at the world's origin or out of the world, and only what had to go
/// is gone.
#[test]
fn a_poisoned_career_loads_whole_and_in_the_world() {
    let path = profile("poison");
    let mut a = app(&path);
    let before = census(a.world_mut());
    save(&mut a, &path);
    drop(a);
    let mut c = Career::from_ron(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let item = |id: &str, rounds: u32| SavedItem {
        id: id.into(),
        condition: f32::NAN,
        rounds,
        charge_wh: f32::INFINITY,
        local: [f32::NAN, 0.0, 1.0e30],
        gun: None,
        inserted: Vec::new(),
        loads: Default::default(),
    };
    // An AK whose well holds a pistol magazine: it no longer fits.
    let mut ak = item("ak74", 0);
    ak.inserted.push(item("pm_mag", 8));
    c.stash.push(ak);
    c.stash.push(item("a_thing_from_the_future", 0));
    c.stash.push(item("pm_mag", 4_000));
    c.loadout.slots.push((0, item("bolt", 0)));
    c.loadout.slots.push((0, item("bolt", 0)));
    c.loadout.slots.push((999, item("compass", 0)));
    let z = &mut c.zones[0];
    let n = z.items.len();
    assert!(n > 10, "a rolled Zone");
    z.items[0].at = [f32::NAN, 0.0, 0.0];
    z.items[1].at = [1.0e9, 0.0, 0.0];
    z.items[2].rot = [0.0; 4];
    z.clock = f32::NAN;
    std::fs::write(&path, c.to_ron().unwrap()).unwrap();

    let mut b = app(&path);
    check_world(b.world_mut(), "poisoned career loaded");
    let w = b.world_mut();
    let after = census(w);
    // What had to go: two zone items lying nowhere, the unknown item.
    let mut want = before.0.clone();
    for id in ["ak74", "pm_mag", "pm_mag", "bolt", "bolt", "compass"] {
        *want.entry(id.into()).or_default() += 1;
    }
    let lost: i64 = want.values().sum::<usize>() as i64 - after.0.values().sum::<usize>() as i64;
    assert_eq!(lost, 2, "only the two lying nowhere are gone");
    assert_eq!(after.0.get("ak74"), want.get("ak74"));
    assert_eq!(
        after.0.get("pm_mag"),
        want.get("pm_mag"),
        "the misfit magazine is kept"
    );
    // Nothing at the origin or out of the world, nothing NaN.
    let catalog = w.resource::<Catalog>().clone();
    let mut q = w.query::<(
        Entity,
        &Item,
        &GlobalTransform,
        Option<&InBackpack>,
        Option<&InSocket>,
    )>();
    let bad: Vec<String> = q
        .iter(w)
        .filter(|(_, it, g, bag, sock)| {
            let p = g.translation();
            let loose = bag.is_none() && sock.is_none();
            !p.is_finite()
                || !it.condition.is_finite()
                || !it.charge_wh.is_finite()
                || (loose && (p.length() < 1.0 || p.y < -50.0))
        })
        .map(|(e, it, g, ..)| format!("{e} {} at {}", catalog.get(it.kind()).id, g.translation()))
        .collect();
    assert!(bad.is_empty(), "{bad:#?}");
    let full = w
        .query::<(&Item, &Mag)>()
        .iter(w)
        .filter(|(i, m)| i.rounds > m.capacity)
        .count();
    assert_eq!(full, 0, "no magazine holds more than it holds");
    assert!(w.resource::<Raid>().clock.is_finite());
}
