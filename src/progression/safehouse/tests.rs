use super::*;
use crate::raid::requests::tests::{go, one};
use std::path::{Path, PathBuf};

fn app_with_save(path: &Path) -> App {
    let opts = LaunchOptions {
        robots: false,
        save: Some(path.to_owned()),
        map: "marrow_creek".into(),
        ..Default::default()
    };
    let mut app = crate::headless_app(opts, false);
    app.finish();
    app.cleanup();
    for _ in 0..5 {
        app.update();
    }
    app
}

fn temp_save(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("extraction-safehouse-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir.join("career.ron")
}

fn house(app: &App) -> Safehouse {
    app.world().resource::<Safehouse>().clone()
}

fn thing(app: &mut App, which: SafehouseThing) -> Entity {
    app.world_mut()
        .query::<(Entity, &SafehouseThing)>()
        .iter(app.world())
        .find(|(_, t)| **t == which)
        .map(|(e, _)| e)
        .expect("the safehouse's thing")
}

fn press(app: &mut App, which: SafehouseThing) {
    let item = thing(app, which);
    let player = one::<With<LocalPlayer>>(app);
    app.world_mut()
        .write_message(InteractEvent::Pressed { item, player });
    app.update();
    app.update();
}

/// Something in the player's worn pack (so it's carried).
fn carry(app: &mut App, id: &str) -> Entity {
    let catalog = app.world().resource::<Catalog>().clone();
    let player = one::<With<LocalPlayer>>(app);
    let pack = app
        .world_mut()
        .query::<(Entity, &Backpack)>()
        .iter(app.world())
        .find(|(_, b)| b.owner == player && b.carried)
        .unwrap()
        .0;
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let e = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind(id).unwrap(),
                transform: Transform::default(),
                condition: 1.0,
            },
        )
    };
    queue.apply(app.world_mut());
    app.world_mut()
        .get_mut::<Backpack>(pack)
        .unwrap()
        .contents
        .push(e);
    app.world_mut().entity_mut(e).insert((
        crate::interact::body::InBackpack { bag: pack },
        ChildOf(pack),
        RigidBodyDisabled,
        ColliderDisabled,
    ));
    app.update();
    e
}

/// Walk in, wedge the door, light the stove: claimed.
fn claim_it(app: &mut App) {
    let cabin = house(app).cabin.expect("the cabin is found");
    let at = app
        .world()
        .get::<GlobalTransform>(cabin)
        .unwrap()
        .translation();
    go(app, [at.x, at.z]);
    assert!(house(app).state.cleared, "inside, nothing about: cleared");
    let door = house(app).door.expect("its door");
    app.world_mut()
        .entity_mut(door)
        .insert(crate::robots::doors::Wedged(Entity::PLACEHOLDER));
    app.update();
    assert!(house(app).state.barred);
    press(app, SafehouseThing::Stove);
    assert!(!house(app).state.lit, "no matches, no fire");
    carry(app, "matches");
    press(app, SafehouseThing::Stove);
    assert!(house(app).state.lit);
    app.update();
    assert!(house(app).state.claimed());
    assert!(house(app).crate_bag.is_some(), "claimed: its crate");
}

#[test]
fn a_claimed_safehouse_and_its_crate_persist() {
    let path = temp_save("persist");
    let mut a = app_with_save(&path);
    claim_it(&mut a);
    // Something left in its crate.
    let bag = house(&a).crate_bag.unwrap();
    let stew = carry(&mut a, "canned_stew");
    {
        let player = one::<With<LocalPlayer>>(&mut a);
        let pack = a
            .world()
            .get::<crate::interact::body::InBackpack>(stew)
            .unwrap()
            .bag;
        a.world_mut()
            .get_mut::<Backpack>(pack)
            .unwrap()
            .contents
            .retain(|e| *e != stew);
        crate::persist::put_in_bag(a.world_mut(), stew, bag, Vec3::ZERO);
        let _ = player;
    }
    a.world_mut().write_message(crate::persist::SaveNow);
    a.update();
    let mut b = app_with_save(&path);
    for _ in 0..5 {
        b.update();
    }
    let h = house(&b);
    assert!(h.state.claimed(), "still claimed after a reload");
    let bag = h.crate_bag.expect("its crate is back");
    let catalog = b.world().resource::<Catalog>().clone();
    let inside: Vec<String> = b
        .world()
        .get::<Backpack>(bag)
        .unwrap()
        .contents
        .iter()
        .filter_map(|e| b.world().get::<Item>(*e))
        .map(|i| catalog.get(i.kind()).id.clone())
        .collect();
    assert_eq!(inside, vec!["canned_stew".to_owned()]);
    // And the stew isn't in the truck's stash as well.
    let truck: usize = b
        .world_mut()
        .query_filtered::<&Backpack, With<crate::raid::truck::Stash>>()
        .iter(b.world())
        .flat_map(|s| s.contents.iter())
        .filter(|e| {
            b.world()
                .get::<Item>(**e)
                .is_some_and(|i| catalog.get(i.kind()).id == "canned_stew")
        })
        .count();
    assert_eq!(truck, 1, "only the starter tin in the truck");
}

#[test]
fn a_night_in_the_safehouse() {
    let path = temp_save("night");
    let mut app = app_with_save(&path);
    claim_it(&mut app);
    // Out on a raid, the truck waiting at an RV, told to wait the night.
    {
        let mut raid = app.world_mut().resource_mut::<Raid>();
        raid.phase = Phase::TruckWaiting { rv: 0, left: 30.0 };
    }
    press(&mut app, SafehouseThing::Radio);
    let clock = app.world().resource::<Raid>().clock;
    press(&mut app, SafehouseThing::Bed);
    let raid = app.world().resource::<Raid>();
    assert!((raid.clock - clock - NIGHT).abs() < 1.0, "a night passed");
    assert!(
        matches!(raid.phase, Phase::TruckWaiting { left, .. } if left > 60.0),
        "the truck waited: {:?}",
        raid.phase
    );
    assert_eq!(
        app.world().resource::<HourShift>().0,
        1,
        "the hour moved on"
    );
    assert_eq!(house(&app).nights_out, 1);
    // Untold, the truck doesn't wait another night.
    press(&mut app, SafehouseThing::Bed);
    assert_eq!(app.world().resource::<Raid>().phase, Phase::Out);
    // Not with the Surge close.
    app.world_mut().resource_mut::<Raid>().clock = crate::raid::SURGE_AFTER - 60.0;
    let before = app.world().resource::<Raid>().clock;
    press(&mut app, SafehouseThing::Bed);
    assert!(
        app.world().resource::<Raid>().clock < before + 5.0,
        "no sleep with the Surge close"
    );
}

#[test]
fn a_loud_raid_brings_them_to_the_door_and_into_the_cabin() {
    let path = temp_save("visit");
    let mut app = app_with_save(&path);
    claim_it(&mut app);
    app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
    // Loud: something comes to the door in the night.
    let player = one::<With<LocalPlayer>>(&mut app);
    app.world_mut().get_mut::<Attention>(player).unwrap().noise = 2.0;
    app.update();
    let director_had = app
        .world()
        .get_resource::<crate::robots::director::Director>()
        .map(|d| d.responses.len());
    press(&mut app, SafehouseThing::Bed);
    if let Some(had) = director_had {
        let now = app
            .world()
            .resource::<crate::robots::director::Director>()
            .responses
            .len();
        assert!(now > had, "a team sent to the door");
    }
    let log: Vec<String> = app
        .world()
        .resource::<Raid>()
        .log
        .iter()
        .map(|l| l.0.clone())
        .collect();
    assert!(log.iter().any(|l| l == lines::SAFE_NIGHT_VISIT), "{log:?}");
    assert_eq!(
        app.world().get::<Attention>(player).unwrap().noise,
        0.0,
        "a night behind the door cools it"
    );
    // Home after a loud raid: they found the cabin.
    app.world_mut().resource_mut::<Safehouse>().peak = extraction_core::safehouse::FOUND_AT;
    crate::raid::requests::tests::home(&mut app);
    app.update();
    let h = house(&app);
    assert!(h.state.broken && !h.state.claimed(), "broken into");
    assert!(h.crate_bag.is_none(), "its crate tipped out");
}

#[test]
fn the_surge_ends_the_claim() {
    let path = temp_save("surge");
    let mut app = app_with_save(&path);
    claim_it(&mut app);
    app.world_mut().resource_mut::<Raid>().cycle += 1;
    app.update();
    let h = house(&app);
    // Still standing in it with the wedge in: cleared and barred again at
    // once, but the stove's out.
    assert!(!h.state.claimed() && !h.state.lit);
    assert!(h.crate_bag.is_none());
    assert_eq!(app.world().resource::<HourShift>().0, 0);
}

#[test]
fn an_expedition_counts_only_after_a_night_out() {
    use crate::raid::requests::tests::{press_card, stash};
    let mut app = crate::raid::requests::tests::app();
    app.world_mut()
        .resource_mut::<super::super::Progression>()
        .0
        .job = Some(extraction_core::progression::Job::Recover {
        map: "marrow_creek".into(),
        region: "The Hollow".into(),
        at: None,
        place: None,
        overnight: true,
    });
    // Whatever recorder the job put out, one is home in the stash.
    stash(&mut app, extraction_core::progression::RECOVER_ITEM, 1);
    let entry = |nights| crate::raid::logbook::RaidEntry {
        number: 1,
        home: true,
        minutes: 30.0,
        wrecked: Default::default(),
        items: 3,
        artifacts: Vec::new(),
        nights,
    };
    app.world_mut()
        .resource_mut::<crate::raid::logbook::Logbook>()
        .last = Some(entry(0));
    press_card(&mut app);
    let done = |app: &App| app.world().resource::<super::super::Progression>().0.done;
    assert_eq!(done(&app), 0, "a day trip doesn't count");
    app.world_mut()
        .resource_mut::<crate::raid::logbook::Logbook>()
        .last = Some(entry(1));
    press_card(&mut app);
    assert_eq!(done(&app), 1, "a night out does");
}
