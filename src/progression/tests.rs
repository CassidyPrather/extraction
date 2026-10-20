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
    // Out, with the radio, ten minutes before the Surge.
    let player = one::<With<LocalPlayer>>(&mut app);
    let kind = catalog.kind("radio").unwrap();
    let radio = app
        .world_mut()
        .query::<(Entity, &Item)>()
        .iter(app.world())
        .find(|(_, i)| i.kind() == kind)
        .unwrap()
        .0;
    // In the worn pack: carried.
    let pack = app
        .world_mut()
        .query::<(Entity, &Backpack)>()
        .iter(app.world())
        .find(|(_, b)| b.owner == player && b.carried)
        .unwrap()
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
    use crate::render::screen::{MIN_EM_DEG, em_metres, read_from};
    let mut app = crate::raid::requests::tests::app();
    let long = "in the hut under the mast out past the Woodward's cabin.";
    {
        let mut p = app.world_mut().resource_mut::<Progression>();
        p.0.tier = 0;
        p.0.done_in_tier = 2;
        p.0.upgrades = Upgrade::ALL.iter().map(|u| u.id().to_owned()).collect();
        for m in ["marrow_creek", "kettle_pass", "cinder_works"] {
            p.0.count_raid(m);
        }
        p.0.job = Some(Job::Recover {
            map: "cinder_works".into(),
            region: "The slag heaps".into(),
            at: None,
            place: Some(long.into()),
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
        let deg = (em_metres(screen) / read_from(screen)).atan().to_degrees();
        assert!(
            deg >= MIN_EM_DEG,
            "{name}: {deg:.2} deg letters:\n{}",
            screen.text
        );
    }
    assert_eq!(seen, 2);
}
