use super::*;
use crate::raid::requests::tests::{go, home, one, press_card, stash};
use extraction_core::progression::{STASH_BASE, STASH_RACK, TIERS};
use extraction_core::save::Career;
use std::path::{Path, PathBuf};

fn app_with_save(path: &Path, map: &str) -> App {
    let opts = LaunchOptions {
        robots: false,
        save: Some(path.to_owned()),
        map: map.into(),
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

fn temp_save(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("extraction-progression-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir.join("career.ron")
}

fn save_now(app: &mut App) {
    app.world_mut().write_message(crate::persist::SaveNow);
    app.update();
}

fn progress(app: &App) -> Progress {
    app.world().resource::<Progression>().0.clone()
}

fn stash_capacity(app: &mut App) -> f32 {
    app.world_mut()
        .query_filtered::<&Backpack, With<Stash>>()
        .iter(app.world())
        .next()
        .unwrap()
        .model
        .capacity
}

#[test]
fn jobs_persist_across_save_and_load() {
    let path = temp_save("persist");
    let mut a = app_with_save(&path, "marrow_creek");
    let first = progress(&a).job.clone().expect("a first job");
    {
        let mut p = a.world_mut().resource_mut::<Progression>();
        p.0.done = 2;
        p.0.done_in_tier = 2;
        p.0.count_raid("kettle_pass");
        p.0.job = Some(Job::Recover {
            map: "kettle_pass".into(),
            region: "The quarry".into(),
            at: None,
            place: None,
            overnight: false,
        });
    }
    save_now(&mut a);
    let mut b = app_with_save(&path, "marrow_creek");
    assert_eq!(
        progress(&b),
        progress(&a),
        "the jobs came back as they were"
    );
    assert_ne!(progress(&b).job, Some(first));
    assert_eq!(progress(&b).raids_in("kettle_pass"), 1);
    // And a job handed in is still handed in after a reload.
    b.world_mut().resource_mut::<Progression>().0.job = Some(Job::Fetch {
        item: "matches".into(),
        n: 1,
    });
    stash(&mut b, "matches", 1);
    press_card(&mut b);
    assert_eq!(progress(&b).done, 3);
    save_now(&mut b);
    let c = app_with_save(&path, "marrow_creek");
    assert_eq!(progress(&c).done, 3);
    assert_eq!(progress(&c).tier, 1, "the first tier is done");
}

#[test]
fn an_upgrade_survives_a_valley_change() {
    let path = temp_save("valley");
    let mut a = app_with_save(&path, "marrow_creek");
    assert_eq!(stash_capacity(&mut a), STASH_BASE, "a new career's crate");
    // Finish the first two tiers.
    let jobs = TIERS[0].jobs + TIERS[1].jobs;
    for _ in 0..jobs {
        a.world_mut().write_message(JobDone);
        a.update();
    }
    a.update();
    assert!(progress(&a).has(Upgrade::StashRack));
    assert!(progress(&a).has(Upgrade::BenchVice));
    assert_eq!(stash_capacity(&mut a), STASH_BASE + STASH_RACK);
    // Drive to Kettle Pass: the career is saved pointed there.
    a.world_mut().write_message(crate::raid::route::Relocate {
        to: "kettle_pass".into(),
    });
    a.update();
    let text = std::fs::read_to_string(&path).unwrap();
    let career = Career::from_ron(&text).unwrap();
    assert_eq!(career.map, "kettle_pass");
    let mut b = app_with_save(&path, "kettle_pass");
    assert!(progress(&b).has(Upgrade::StashRack), "the rack came along");
    assert_eq!(stash_capacity(&mut b), STASH_BASE + STASH_RACK);
    let tier = b
        .world_mut()
        .query::<&crate::raid::bench::Bench>()
        .iter(b.world())
        .next()
        .unwrap()
        .tier;
    assert_eq!(tier, 1, "and the vice");
    let props: Vec<Upgrade> = b
        .world_mut()
        .query::<&TruckUpgradeProp>()
        .iter(b.world())
        .map(|p| p.0)
        .collect();
    assert!(props.contains(&Upgrade::StashRack), "{props:?}");
    assert!(props.contains(&Upgrade::BenchVice), "{props:?}");
}

#[test]
fn a_career_from_before_jobs_loads_and_is_credited() {
    let path = temp_save("old");
    // An old career: four requests met, a fifth on the card, no jobs
    // (an old build wrote none: `None` reads as the field missing does,
    // `save::tests` holds that), and more in the stash than a new crate
    // holds.
    let a = app_with_save(&path, "marrow_creek");
    drop(a);
    let mut career = Career::from_game(&extraction_core::save::SaveGame::new("marrow_creek"));
    career.requests.number = 5;
    career.requests.want = Some(("servo".into(), 2));
    for i in 0..60 {
        career.stash.push(extraction_core::save::SavedItem {
            id: "ak74".into(),
            condition: 0.5,
            rounds: 0,
            charge_wh: 0.0,
            local: [0.0, 0.05, -0.5 + i as f32 * 0.016],
            gun: None,
            inserted: Vec::new(),
            loads: Default::default(),
        });
    }
    assert!(career.progress.is_none());
    std::fs::write(&path, career.to_ron().unwrap()).unwrap();
    let mut b = app_with_save(&path, "marrow_creek");
    let p = progress(&b);
    assert_eq!(p.done, 4, "the four requests met count");
    assert_eq!(p.tier, 1);
    assert!(p.has(Upgrade::StashRack));
    assert_eq!(
        p.job,
        Some(Job::Fetch {
            item: "servo".into(),
            n: 2
        }),
        "the one on the card stays"
    );
    assert!(
        stash_capacity(&mut b) > STASH_BASE + STASH_RACK,
        "a crate fuller than its room keeps what's in it"
    );
}

#[test]
fn a_recover_job_puts_a_recorder_out_and_takes_it_home() {
    let mut app = crate::raid::requests::tests::app();
    let region = "Pumping station";
    app.world_mut().resource_mut::<Progression>().0.job = Some(Job::Recover {
        map: "marrow_creek".into(),
        region: region.into(),
        at: None,
        place: None,
        overnight: false,
    });
    for _ in 0..3 {
        app.update();
    }
    let Some(Job::Recover { at, place, .. }) = progress(&app).job else {
        panic!();
    };
    let at = at.expect("put somewhere");
    let zone = app.world().resource::<Zone>().clone();
    assert_eq!(zone.map.region_at(at).unwrap().name, region);
    let place = place.expect("and said where");
    assert!(place.starts_with("in the "), "{place}");
    // Not in the stash yet: the driver says so.
    press_card(&mut app);
    assert_eq!(progress(&app).done, 0);
    // Fetch it (it's in the stash), and hand it in.
    let catalog = app.world().resource::<Catalog>().clone();
    let kind = catalog.kind(RECOVER_ITEM).unwrap();
    let recorders = |app: &mut App| {
        app.world_mut()
            .query::<&Item>()
            .iter(app.world())
            .filter(|i| i.kind() == kind)
            .count()
    };
    assert_eq!(recorders(&mut app), 1, "one recorder out there");
    let loose = app
        .world_mut()
        .query::<(Entity, &Item)>()
        .iter(app.world())
        .find(|(_, i)| i.kind() == kind)
        .unwrap()
        .0;
    app.world_mut().despawn(loose);
    stash(&mut app, RECOVER_ITEM, 1);
    press_card(&mut app);
    assert_eq!(progress(&app).done, 1, "recovered");
    assert_eq!(recorders(&mut app), 0, "the driver has it");
}

#[test]
fn going_down_forgets_what_you_scouted() {
    let mut app = crate::raid::requests::tests::app();
    let Some(Job::Scout { at, .. }) = progress(&app).job else {
        panic!();
    };
    go(&mut app, at);
    assert!(matches!(
        progress(&app).job,
        Some(Job::Scout { seen: true, .. })
    ));
    app.world_mut().resource_mut::<Raid>().phase = Phase::Down { left: 30.0 };
    app.update();
    assert!(matches!(
        progress(&app).job,
        Some(Job::Scout { seen: false, .. })
    ));
    home(&mut app);
    press_card(&mut app);
    assert_eq!(progress(&app).done, 0, "not until you get it home");
}

/// The designer's first raid (playtest 28 October): close to the first
/// job, then away. Told once which way it is; going down, told again at
/// the truck that it's still waiting.
#[test]
fn wandering_off_the_job_is_told_once_and_a_fall_restates_it() {
    let mut app = crate::raid::requests::tests::app();
    let Some(Job::Scout { at, .. }) = progress(&app).job else {
        panic!("the first job's a scout");
    };
    let said = |app: &App, what: &str| {
        app.world()
            .resource::<Raid>()
            .log
            .iter()
            .filter(|(l, _)| l.contains(what))
            .count()
    };
    let nudged = |app: &App| said(app, "That's the job") + said(app, "The job: ");
    // Toward it, then not far off it: nothing.
    go(&mut app, [at[0] - 120.0, at[1]]);
    go(&mut app, [at[0] - 180.0, at[1]]);
    assert_eq!(nudged(&app), 0, "not yet");
    // Well off: once.
    go(&mut app, [at[0] - 260.0, at[1]]);
    assert_eq!(
        nudged(&app),
        1,
        "told: {:?}",
        app.world().resource::<Raid>().log
    );
    go(&mut app, [at[0] - 300.0, at[1] + 20.0]);
    assert_eq!(nudged(&app), 1, "not nagged");
    // Down: woken in the truck, it's still waiting.
    let player = one::<With<LocalPlayer>>(&mut app);
    app.world_mut().write_message(crate::health::Downed {
        player,
        at: Vec3::ZERO,
    });
    app.update();
    if let Phase::Down { left } = &mut app.world_mut().resource_mut::<Raid>().phase {
        *left = 0.0;
    }
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Raid>().phase, Phase::AtBase);
    assert_eq!(
        said(&app, "still waiting"),
        1,
        "{:?}",
        app.world().resource::<Raid>().log
    );
}

/// The designer's second raid (playtest 28 October) left with nothing; the
/// first, without the map. Off the truck's steps without them, the driver
/// says what's missing and where, once; you can still go.
#[test]
fn stepping_off_the_truck_without_kit_is_told_once() {
    let mut app = crate::raid::requests::tests::app();
    let said = |app: &App| {
        app.world()
            .resource::<Raid>()
            .log
            .iter()
            .filter(|(l, _)| l.contains("on you."))
            .map(|(l, _)| l.clone())
            .collect::<Vec<_>>()
    };
    let truck = app
        .world_mut()
        .query_filtered::<&GlobalTransform, With<Truck>>()
        .iter(app.world())
        .next()
        .unwrap()
        .translation();
    // In the bed, then off the steps with the kit all in the crate. (Off
    // the truck before ever being on it, a session's first frames, is the
    // first-hour bots' to show: no line at camp.)
    for id in ["pm_pistol", "radio", "map_case"] {
        stash(&mut app, id, 1);
    }
    // Put straight at camp (a session opening): nothing.
    go(&mut app, [truck.x, truck.z]);
    go(&mut app, [truck.x, truck.z + 6.0]);
    assert!(
        said(&app).is_empty(),
        "not put off the truck, only stepping off"
    );
    // Stepped off, a stride at a time.
    go(&mut app, [truck.x, truck.z]);
    for z in [1.5, 2.3, 3.1, 3.9, 4.7] {
        go(&mut app, [truck.x, truck.z + z]);
    }
    assert_eq!(app.world().resource::<Raid>().phase, Phase::AtBase);
    let lines = said(&app);
    assert_eq!(lines.len(), 1, "{:?}", app.world().resource::<Raid>().log);
    assert!(
        lines[0].contains("No gun, radio or map on you. In the crate."),
        "{lines:?}"
    );
    // Once: about on the gravel, nothing more; and you can still go.
    go(&mut app, [truck.x + 2.0, truck.z + 8.0]);
    assert_eq!(said(&app).len(), 1);
    go(&mut app, [truck.x, truck.z + 20.0]);
    assert_eq!(app.world().resource::<Raid>().phase, Phase::Out);
}

#[test]
fn the_cabinet_has_a_dressing_each_raid_and_the_aerial_warns_early() {
    let mut app = crate::raid::requests::tests::app();
    {
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.0.upgrades = Upgrade::ALL.iter().map(|u| u.id().to_owned()).collect();
    }
    let catalog = app.world().resource::<Catalog>().clone();
    let dressing = catalog.kind("field_dressing").unwrap();
    let dressings = |app: &mut App| {
        app.world_mut()
            .query::<&Item>()
            .iter(app.world())
            .filter(|i| i.kind() == dressing)
            .count()
    };
    let before = dressings(&mut app);
    app.world_mut().resource_mut::<Raid>().raids = 1;
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(dressings(&mut app), before + 1, "one on the shelf");
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(dressings(&mut app), before + 1, "once a raid");
    // And it stays on the shelf.
    for _ in 0..60 {
        app.update();
    }
    let truck = app
        .world_mut()
        .query::<(&crate::raid::truck::Truck, &GlobalTransform)>()
        .iter(app.world())
        .next()
        .expect("the truck")
        .1
        .compute_transform();
    let shelf = truck.transform_point(CABINET_AT);
    let on = app
        .world_mut()
        .query::<(&Item, &GlobalTransform)>()
        .iter(app.world())
        .filter(|(i, _)| i.kind() == dressing)
        .map(|(_, g)| g.translation().distance(shelf))
        .fold(f32::MAX, f32::min);
    assert!(on < 0.25, "the dressing is {on:.2} m from the shelf");
    // Out, with the radio, ten minutes before the Surge.
    let player = one::<With<LocalPlayer>>(&mut app);
    // (Its own radio: the world's loot and the truck's kit move about.)
    let kind = catalog.kind("radio").expect("a radio in the catalog");
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let radio = crate::items::spawn_item(
        &mut Commands::new(&mut queue, app.world()),
        &catalog,
        crate::items::SpawnItem {
            kind,
            transform: Transform::default(),
            condition: 1.0,
        },
    );
    queue.apply(app.world_mut());
    app.update();
    // In the worn pack: carried.
    let pack = app
        .world_mut()
        .query::<(Entity, &Backpack)>()
        .iter(app.world())
        .find(|(_, b)| b.owner == player && b.carried)
        .expect("the player wears a pack")
        .0;
    app.world_mut()
        .get_mut::<Backpack>(pack)
        .unwrap()
        .contents
        .push(radio);
    app.world_mut().entity_mut(radio).insert((
        crate::interact::body::InBackpack { bag: pack },
        ChildOf(pack),
    ));
    {
        let mut raid = app.world_mut().resource_mut::<Raid>();
        raid.phase = Phase::Out;
        raid.clock = crate::raid::SURGE_AFTER - AERIAL_WARNING - 1.0;
        raid.log.clear();
    }
    app.update();
    app.world_mut().resource_mut::<Raid>().clock += 2.0;
    app.update();
    let log: Vec<String> = app
        .world()
        .resource::<Raid>()
        .log
        .iter()
        .map(|l| l.0.clone())
        .collect();
    assert!(log.iter().any(|l| l.contains("Aerial")), "{log:?}");
}

/// The longest things the request card and the career page say still read
/// from where you stand in the bed (`vr_harness`'s panel test sees only
/// what's on them in a new career).
#[test]
fn the_longest_job_and_career_page_still_read() {
    use crate::raid::bench::ScreenText;
    use crate::render::legibility::{TextSource, check_panel};
    let mut app = crate::raid::requests::tests::app();
    let long = "in the hut under the mast out past the Woodward's cabin.";
    {
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.0.tier = 0;
        p.0.done_in_tier = 2;
        p.0.upgrades = Upgrade::ALL.iter().map(|u| u.id().to_owned()).collect();
        p.0.laid_up = p.0.upgrades.clone();
        for m in ["marrow_creek", "kettle_pass", "cinder_works"] {
            p.0.count_raid(m);
        }
        p.0.job = Some(Job::Recover {
            map: "cinder_works".into(),
            region: "The slag heaps".into(),
            at: None,
            place: Some(long.into()),
            overnight: false,
        });
    }
    {
        let mut r = app
            .world_mut()
            .resource_mut::<crate::raid::requests::Requests>();
        r.cache = Some(long.into());
    }
    {
        let mut m = app.world_mut().resource_mut::<crate::persist::Memorial>();
        m.0.convoys = 12;
        m.0.lost = vec!["x".into(); 140];
    }
    app.world_mut()
        .resource_mut::<crate::raid::logbook::Logbook>()
        .career = true;
    {
        let mut raid = app.world_mut().resource_mut::<Raid>();
        raid.raids = 188;
        raid.extractions = 120;
        raid.deaths = 68;
    }
    for _ in 0..3 {
        app.update();
    }
    let mut seen = 0;
    let mut q = app.world_mut().query::<(&Name, &ScreenText)>();
    for (name, screen) in q.iter(app.world()) {
        if !["Request card", "Logbook"].contains(&name.as_str()) {
            continue;
        }
        seen += 1;
        let source = if name.as_str() == "Logbook" {
            TextSource::Logbook
        } else {
            TextSource::Request
        };
        if let Err(e) = check_panel(screen, source) {
            panic!("{name}: {e}\n{}", screen.text);
        }
    }
    assert_eq!(seen, 2);
}

#[test]
fn a_fetch_nobody_can_find_gives_way_to_another_job() {
    let mut app = crate::raid::requests::tests::app();
    {
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.0.done = 1;
        p.0.job = Some(Job::Fetch {
            item: "rotor_assembly".into(),
            n: 2,
        });
        p.0.job_raid = 0;
    }
    for raid in 1..=extraction_core::progression::FETCH_PATIENCE {
        let at = progress(&app).job.clone();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.world_mut().resource_mut::<Raid>().raids = raid;
        app.update();
        home(&mut app);
        app.update();
        if raid < extraction_core::progression::FETCH_PATIENCE {
            assert_eq!(progress(&app).job, at, "still waiting after {raid}");
        }
    }
    let job = progress(&app).job.expect("a new job");
    assert!(
        !matches!(&job, Job::Fetch { item, .. } if item == "rotor_assembly"),
        "{job:?}"
    );
    assert_eq!(progress(&app).done, 1, "a swap isn't a job done");
}

#[test]
fn the_second_and_third_tiers_hand_over_guns() {
    let mut app = crate::raid::requests::tests::app();
    let catalog = app.world().resource::<Catalog>().clone();
    // What Lev has passed back through the cab's hatch (`raid::lev`).
    let ids = |app: &mut App| -> Vec<String> {
        app.world()
            .resource::<crate::raid::lev::Handovers>()
            .handed
            .iter()
            .map(|(id, _)| id.clone())
            .collect()
    };
    let before = ids(&mut app).len();
    for _ in 0..TIERS[0].jobs + TIERS[1].jobs + TIERS[2].jobs {
        app.world_mut().write_message(JobDone);
        app.update();
    }
    // (Contracts' kit and pay come through the hatch too.)
    for _ in 0..6000 {
        app.update();
        if !app.world().resource::<crate::raid::lev::Handovers>().busy() {
            break;
        }
    }
    let after = ids(&mut app);
    let known: Vec<&str> = gifts(Upgrade::BenchVice)
        .iter()
        .chain(gifts(Upgrade::Aerial))
        .filter(|g| catalog.kind(g.0).is_some())
        .map(|g| g.0)
        .collect();
    // Every gift's item is in the catalog (the SKS, the PPSh and their
    // ammunition), and every one came through the hatch.
    let all = gifts(Upgrade::BenchVice).len() + gifts(Upgrade::Aerial).len();
    assert_eq!(known.len(), all, "a gift names an item the catalog lacks");
    for id in &known {
        assert!(after.iter().any(|a| a == id), "{id} in {after:?}");
    }
    assert!(after.len() > before);
}

#[test]
fn the_last_job_is_hanas_letter_from_the_table() {
    use extraction_core::progression::LETTER_ITEM;
    let mut app = crate::raid::requests::tests::app();
    {
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.0.tier = (TIERS.len() - 1) as u32;
        p.0.done_in_tier = TIERS[TIERS.len() - 1].jobs - 1;
        p.0.done = 13;
        p.0.job = None;
    }
    for _ in 0..40 {
        app.update();
    }
    assert_eq!(progress(&app).job, Some(Job::Letter));
    let catalog = app.world().resource::<Catalog>().clone();
    let kind = catalog.kind(LETTER_ITEM).unwrap();
    let at = app
        .world_mut()
        .query::<(&Item, &GlobalTransform)>()
        .iter(app.world())
        .find(|(i, _)| i.kind() == kind)
        .map(|(_, g)| g.translation())
        .expect("the letter is out there");
    let zone = app.world().resource::<Zone>().clone();
    let ground = zone.ground(at.x, at.z);
    assert!(
        at.y > ground + 0.4,
        "on the table, not the floor: {at:?}, ground {ground}"
    );
    let loose = app
        .world_mut()
        .query::<(Entity, &Item)>()
        .iter(app.world())
        .find(|(_, i)| i.kind() == kind)
        .unwrap()
        .0;
    app.world_mut().despawn(loose);
    stash(&mut app, LETTER_ITEM, 1);
    press_card(&mut app);
    let p = progress(&app);
    assert_eq!(p.done, 14);
    assert!(p.has(Upgrade::MedCabinet), "the ladder's done");
    let log: Vec<String> = app
        .world()
        .resource::<Raid>()
        .log
        .iter()
        .map(|l| l.0.clone())
        .collect();
    assert!(log.iter().any(|l| l == lines::LETTER_HOME), "{log:?}");
}

#[test]
fn a_recover_job_can_send_you_underground() {
    let mut app = crate::raid::requests::tests::app();
    let zone = app.world().resource::<Zone>().clone();
    let (name, entrance, _) = extraction_core::progression::underground_sites(&zone.map)
        .into_iter()
        .next()
        .expect("Marrow Creek has its shelter");
    app.world_mut().resource_mut::<Progression>().0.job = Some(Job::Recover {
        map: "marrow_creek".into(),
        region: name.clone(),
        at: None,
        place: None,
        overnight: false,
    });
    for _ in 0..40 {
        app.update();
    }
    let Some(Job::Recover { place, .. }) = progress(&app).job else {
        panic!();
    };
    let catalog = app.world().resource::<Catalog>().clone();
    let kind = catalog.kind(RECOVER_ITEM).unwrap();
    let at = app
        .world_mut()
        .query::<(&Item, &GlobalTransform)>()
        .iter(app.world())
        .find(|(i, _)| i.kind() == kind)
        .map(|(_, g)| g.translation())
        .expect("the recorder is down there");
    assert!(at.y < zone.ground(at.x, at.z) - 1.5, "below ground: {at:?}");
    assert!(at.xz().distance(Vec2::from_array(entrance)) < 60.0);
    let place = place.expect("said where");
    assert!(
        place.starts_with(&format!("down in the {}", name.to_lowercase())),
        "{place}"
    );
}

/// Past the ladder: a fitting due its upkeep takes it from the stash at
/// the homecoming, or lies up (and the vice's recipes go with it).
#[test]
fn upkeep_comes_out_of_the_stash_or_the_fitting_lies_up() {
    let mut app = crate::raid::requests::tests::app();
    let catalog = app.world().resource::<Catalog>().clone();
    {
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.0.credit(TIERS.iter().map(|t| t.jobs).sum());
    }
    let count = |app: &mut App, id: &str| {
        let kind = catalog.kind(id).unwrap();
        app.world_mut()
            .query::<&Item>()
            .iter(app.world())
            .filter(|i| i.kind() == kind)
            .count()
    };
    let homecoming = |app: &mut App, raids: u32| {
        {
            let mut raid = app.world_mut().resource_mut::<Raid>();
            raid.phase = Phase::Out;
            raid.raids = raids;
        }
        app.update();
        home(app);
        app.update();
    };
    // The first homecoming starts every fitting's clock.
    homecoming(&mut app, 30);
    assert!(progress(&app).upkeep_due.len() == Upgrade::ALL.len());
    // Due at 36: scrap for both racks, no oil.
    stash(&mut app, "scrap_metal", 4);
    stash(&mut app, "electronics", 1);
    stash(&mut app, "field_dressing", 1);
    let scrap = count(&mut app, "scrap_metal");
    homecoming(&mut app, 36);
    assert_eq!(
        count(&mut app, "scrap_metal"),
        scrap - 4,
        "two for each rack"
    );
    let p = progress(&app);
    assert!(!p.works(Upgrade::BenchVice), "no oil: the vice is laid up");
    assert!(p.works(Upgrade::Aerial) && p.works(Upgrade::StashRack));
    let tier = app
        .world_mut()
        .query::<&crate::raid::bench::Bench>()
        .iter(app.world())
        .next()
        .unwrap()
        .tier;
    assert_eq!(tier, 0, "and its recipes went with it");
    let log: Vec<String> = app
        .world()
        .resource::<Raid>()
        .log
        .iter()
        .map(|l| l.0.clone())
        .collect();
    assert!(log.iter().any(|l| l.contains("vice's laid up")), "{log:?}");
    // Oil in the stash: back at the next homecoming.
    stash(&mut app, "gun_oil", 1);
    homecoming(&mut app, 37);
    assert!(progress(&app).works(Upgrade::BenchVice));
}

/// Past the ladder the order card shows, cycles, and the caches follow it.
#[test]
fn the_order_card_turns_the_caches() {
    let mut app = crate::raid::requests::tests::app();
    let card = one::<With<OrderCard>>(&mut app);
    let player = one::<With<LocalPlayer>>(&mut app);
    let press = |app: &mut App| {
        app.world_mut()
            .write_message(crate::interact::InteractEvent::Pressed { item: card, player });
        app.update();
        app.update();
    };
    // Before the ladder's done: hidden, and the driver says not yet.
    assert_eq!(
        *app.world().get::<Visibility>(card).unwrap(),
        Visibility::Hidden
    );
    press(&mut app);
    assert_eq!(progress(&app).order, None);
    app.world_mut()
        .resource_mut::<Progression>()
        .0
        .credit(TIERS.iter().map(|t| t.jobs).sum());
    app.update();
    assert_eq!(
        *app.world().get::<Visibility>(card).unwrap(),
        Visibility::Inherited
    );
    press(&mut app);
    assert_eq!(progress(&app).order_def().id, "rounds");
    // A job done now leaves a cache of rounds: three piles of them.
    let catalog = app.world().resource::<Catalog>().clone();
    let ammo = |app: &mut App| {
        app.world_mut()
            .query::<&Item>()
            .iter(app.world())
            .filter(|i| {
                catalog.get(i.kind()).category == extraction_core::items::Category::Ammo
                    || catalog.get(i.kind()).id.ends_with("_mag")
                    || catalog.get(i.kind()).id.ends_with("_clip")
            })
            .count()
    };
    let before = ammo(&mut app);
    app.world_mut().resource_mut::<Progression>().0.job = Some(Job::Fetch {
        item: "matches".into(),
        n: 1,
    });
    stash(&mut app, "matches", 1);
    press_card(&mut app);
    assert!(progress(&app).done > TIERS.iter().map(|t| t.jobs).sum::<u32>());
    let after = ammo(&mut app);
    assert!(
        after >= before + 12,
        "a cache of rounds: {before} -> {after}"
    );
}

/// The first deep job, once Underground's depot is on the map: the intake
/// ledger, left in its bay, handed in from the stash.
#[test]
fn the_ledger_job_puts_the_ledger_in_its_bay_and_takes_it_home() {
    use extraction_core::progression::{LEDGER_AT, LEDGER_ITEM};
    let mut app = crate::raid::requests::tests::app();
    app.world_mut()
        .resource_mut::<Progression>()
        .0
        .credit(TIERS.iter().map(|t| t.jobs).sum());
    app.world_mut().resource_mut::<Progression>().0.job = Some(Job::Ledger);
    for _ in 0..3 {
        app.update();
    }
    let catalog = app.world().resource::<Catalog>().clone();
    let kind = catalog.kind(LEDGER_ITEM).unwrap();
    let ledger = app
        .world_mut()
        .query::<(Entity, &Item, &Transform)>()
        .iter(app.world())
        .find(|(_, i, _)| i.kind() == kind)
        .map(|(e, _, t)| (e, t.translation))
        .expect("the ledger is in its bay");
    assert!(
        ledger
            .1
            .xz()
            .distance(Vec2::new(LEDGER_AT[0], LEDGER_AT[2]))
            < 0.5
    );
    press_card(&mut app);
    assert_eq!(progress(&app).job, Some(Job::Ledger), "not without it");
    app.world_mut().despawn(ledger.0);
    stash(&mut app, LEDGER_ITEM, 1);
    press_card(&mut app);
    assert_ne!(progress(&app).job, Some(Job::Ledger), "handed in");
    let log: Vec<String> = app
        .world()
        .resource::<Raid>()
        .log
        .iter()
        .map(|l| l.0.clone())
        .collect();
    assert!(log.iter().any(|l| l == lines::LEDGER_HOME), "{log:?}");
}
