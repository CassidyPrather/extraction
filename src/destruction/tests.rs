//! Breaking each material by each path (a round in flight, a blow, a
//! throw, a walker's shoulder), what's broken surviving a save and
//! going with the Surge, and the debris budget holding under a storm.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::debris::{DEBRIS_CAP, Debris, DebrisPool};
use super::glass::Pane;
use super::wood::Timber;
use super::{Broken, key};
use crate::items::loot::SupplyCrate;
use crate::weapons::ballistics::{Hit, Projectile, ShotMods};
use crate::world::buildings::{Door, DoorLeaf};

fn app(save: Option<std::path::PathBuf>) -> App {
    let opts = crate::LaunchOptions {
        robots: false,
        save,
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

fn run(app: &mut App, ticks: usize) {
    for _ in 0..ticks {
        app.update();
    }
}

/// A pane with clear air a couple of metres out from it, and that way
/// (out of the sash's face): upright, on the ground floor (a walker can
/// shoulder it, a lobbed bolt meets its face), with clear air in front of
/// it down to a body's height. Taken in order of where it stands, so a
/// valley's layout changing elsewhere doesn't change which.
fn open_pane(app: &mut App) -> (Entity, Vec3, Vec3) {
    let spatial_ok = |app: &mut App, from: Vec3, to: Vec3| {
        let world = app.world_mut();
        let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(world);
        let spatial = state.get(world).unwrap();
        let v = to - from;
        spatial
            .cast_ray(
                from,
                Dir3::new(v).unwrap(),
                v.length() - 0.02,
                true,
                &SpatialQueryFilter::from_mask(crate::world::Layer::World),
            )
            .is_none()
    };
    let mut panes: Vec<(Entity, Pane)> = app
        .world_mut()
        .query::<(Entity, &Pane)>()
        .iter(app.world())
        .map(|(e, p)| (e, p.clone()))
        .collect();
    panes.sort_by_key(|(_, p)| key(p.centre()));
    let zone = app.world().resource::<crate::world::Zone>().clone();
    assert!(
        panes.len() > 50,
        "the valley's windows are glazed ({})",
        panes.len()
    );
    for (e, p) in panes {
        let c = p.centre();
        let ground = zone.ground(c.x, c.z);
        if p.broken.is_some() || c.y - ground > 2.2 {
            continue;
        }
        for out in [p.world.rotation * Vec3::NEG_Z, p.world.rotation * Vec3::Z] {
            if out.y.abs() > 0.3 {
                continue;
            }
            let from = c + out * 2.0;
            // Clear to the pane, and at a walker's chest beside it.
            let low = Vec3::new(from.x, ground + 1.0, from.z);
            if spatial_ok(app, from, c) && spatial_ok(app, low, c.with_y(ground + 1.0) + out * 0.3)
            {
                println!("open_pane: {c:.2} facing {out:.2}");
                return (e, c, out);
            }
        }
    }
    panic!("no pane with clear air in front of it");
}

fn shoot(app: &mut App, from: Vec3, to: Vec3, speed: f32, mass_g: f32, damage: f32) {
    let gun = app.world_mut().spawn_empty().id();
    app.world_mut().spawn((
        Projectile {
            velocity: (to - from).normalize() * speed,
            damage,
            shooter: None,
            gun,
            life: 2.0,
            mods: ShotMods::default(),
            mass_g,
            seed: 7,
            passes: 0,
            load: Default::default(),
        },
        Transform::from_translation(from),
    ));
}

fn live_debris(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), With<Debris>>()
        .iter(app.world())
        .count()
}

#[test]
fn a_round_through_a_pane_shatters_it() {
    let mut app = app(None);
    let (pane, at, out) = open_pane(&mut app);
    shoot(&mut app, at + out * 2.0, at, 700.0, 7.9, 40.0);
    run(&mut app, 3);
    let p = app.world().get::<Pane>(pane).unwrap();
    assert!(p.broken.is_some(), "the pane broke");
    assert!(
        app.world().resource::<Broken>().0.contains(&key(at)),
        "and it's noted for the save"
    );
    assert!(live_debris(&mut app) > 3, "shards fall");
    // They fall: a second on, every shard is below the pane it came from.
    run(&mut app, 60);
    let ys: Vec<f32> = app
        .world_mut()
        .query_filtered::<&Transform, With<Debris>>()
        .iter(app.world())
        .map(|t| t.translation.y)
        .collect();
    assert!(ys.iter().all(|y| *y < at.y), "shards dropped: {ys:?}");
}

#[test]
fn a_round_going_away_from_a_pane_leaves_it() {
    let mut app = app(None);
    let (pane, at, out) = open_pane(&mut app);
    shoot(&mut app, at + out * 0.5, at + out * 5.0, 700.0, 7.9, 40.0);
    run(&mut app, 3);
    assert!(
        app.world().get::<Pane>(pane).unwrap().broken.is_none(),
        "a round going away from the glass doesn't break it"
    );
}

#[test]
fn a_thrown_thing_breaks_a_pane_and_a_slow_one_doesnt() {
    let mut app = app(None);
    let (pane, at, out) = open_pane(&mut app);
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let spawn = |app: &mut App, from: Vec3, v: Vec3| {
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind("bolt").unwrap(),
                    transform: Transform::from_translation(from),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.world_mut().entity_mut(e).insert(LinearVelocity(v));
        e
    };
    // Drifting into it at a walk: nothing.
    spawn(&mut app, at + out * 0.3, -out * 0.8 + Vec3::Y * 0.3);
    run(&mut app, 60);
    assert!(app.world().get::<Pane>(pane).unwrap().broken.is_none());
    // Thrown at it.
    spawn(&mut app, at + out * 1.0, -out * 9.0 + Vec3::Y * 1.0);
    run(&mut app, 30);
    assert!(
        app.world().get::<Pane>(pane).unwrap().broken.is_some(),
        "a thrown bolt breaks the glass"
    );
}

#[test]
fn a_walker_walking_into_a_pane_breaks_it() {
    let mut app = app(None);
    let (pane, at, out) = open_pane(&mut app);
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let start = zone.on_ground(at.x + out.x * 3.0, at.z + out.z * 3.0);
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let walker = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::robots::walker::spawn_walker_with(
            &mut c,
            &zone,
            start,
            crate::robots::Job {
                kind: crate::robots::JobKind::Response,
                site: start,
                duration: 999.0,
                exit: start,
            },
            false,
            100.0,
        )
    };
    queue.apply(app.world_mut());
    app.update();
    // Shouldered up against the window.
    let y = app.world().get::<Transform>(walker).unwrap().translation.y;
    let against = Vec3::new(at.x + out.x * 0.2, y.max(at.y - 1.0), at.z + out.z * 0.2);
    app.world_mut()
        .get_mut::<Transform>(walker)
        .unwrap()
        .translation = against;
    run(&mut app, 3);
    assert!(
        app.world().get::<Pane>(pane).unwrap().broken.is_some(),
        "the walker's body went through the glass"
    );
}

/// A plank door, its leaf's collider and where its middle is.
fn plank_door(app: &mut App) -> (Entity, Entity, Vec3, Vec3) {
    let doors: Vec<(Entity, DoorLeaf, Vec<Entity>, GlobalTransform)> = app
        .world_mut()
        .query::<(Entity, &DoorLeaf, &Children, &GlobalTransform)>()
        .iter(app.world())
        .map(|(e, l, c, g)| (e, *l, c.iter().collect(), *g))
        .collect();
    let (door, leaf, kids, gt) = doors
        .into_iter()
        .find(|(_, l, ..)| l.0.mat == crate::render::materials::Mat::Planks)
        .expect("a plank door");
    let mid = gt.transform_point(Vec3::new(leaf.0.width * 0.5, 1.1, 0.0));
    let normal = gt.rotation() * Vec3::Z;
    (door, kids[0], mid, normal)
}

#[test]
fn rounds_go_through_a_plank_door_and_wear_it_down() {
    let mut app = app(None);
    let (door, _, mid, n) = plank_door(&mut app);
    assert!(app.world().get::<Timber>(door).is_some());
    let mut through = 0;
    for i in 0..40 {
        if app.world().get::<Door>(door).is_none() {
            break;
        }
        let from = mid + n * 3.0 + Vec3::Y * (i as f32 * 0.01);
        // A rifle round (7.62x39): through planks with plenty to spare.
        shoot(&mut app, from, mid, 715.0, 7.9, 40.0);
        run(&mut app, 1);
        // Out the far side: a round still flying past the door.
        through += app
            .world_mut()
            .query::<(&Projectile, &Transform)>()
            .iter(app.world())
            .filter(|(_, t)| (t.translation - mid).dot(n) < -0.1)
            .count();
        run(&mut app, 2);
    }
    assert!(through > 0, "rounds went on through the planks");
    assert!(
        app.world().get::<Door>(door).is_none(),
        "the door gave way under the rounds"
    );
    assert!(app.world().resource::<Broken>().0.iter().count() >= 1);
    assert!(live_debris(&mut app) > 5, "and splinters fly");
}

#[test]
fn a_pistol_round_stops_in_a_plank_door() {
    let mut app = app(None);
    let (_, _, mid, n) = plank_door(&mut app);
    // 9x18: 6.1 g at 300 m/s is ~270 J; 22 mm of planks cost 330.
    shoot(&mut app, mid + n * 3.0, mid, 300.0, 6.1, 32.0);
    run(&mut app, 2);
    let beyond = app
        .world_mut()
        .query::<(&Projectile, &Transform)>()
        .iter(app.world())
        .filter(|(_, t)| (t.translation - mid).dot(n) < -0.1)
        .count();
    assert_eq!(beyond, 0, "a pistol round doesn't get through");
}

#[test]
fn blows_break_a_door_down() {
    let mut app = app(None);
    let (door, leaf_collider, mid, n) = plank_door(&mut app);
    // A blade's or a blow's hit (melee writes these with no round mass).
    let mut blows = 0;
    while app.world().get::<Door>(door).is_some() && blows < 40 {
        app.world_mut().write_message(Hit {
            blow: Some(extraction_core::melee::Blow::Blunt),
            entity: leaf_collider,
            point: mid,
            normal: n,
            velocity: -n * 6.0,
            damage: 40.0,
            shooter: None,
            mass_g: 0.0,
            load: Default::default(),
        });
        run(&mut app, 2);
        blows += 1;
    }
    assert!(
        app.world().get::<Door>(door).is_none(),
        "the door came down"
    );
    assert!((6..=8).contains(&blows), "after a fair few blows ({blows})");
}

#[test]
fn a_crate_broken_open_spills_its_loot() {
    let mut app = app(None);
    let crates: Vec<(Entity, Transform)> = app
        .world_mut()
        .query_filtered::<(Entity, &Transform), With<SupplyCrate>>()
        .iter(app.world())
        .map(|(e, t)| (e, *t))
        .collect();
    assert!(!crates.is_empty(), "supply crates stand in the open");
    let (c, t) = crates[0];
    let items = |app: &mut App| {
        app.world_mut()
            .query_filtered::<(), With<crate::items::Item>>()
            .iter(app.world())
            .count()
    };
    let before = items(&mut app);
    let mut shots = 0;
    while app.world().get_entity(c).is_ok() && shots < 20 {
        let from = t.translation + Vec3::new(3.0, 0.1, 0.4);
        shoot(&mut app, from, t.translation, 300.0, 6.1, 32.0);
        run(&mut app, 3);
        shots += 1;
    }
    assert!(app.world().get_entity(c).is_err(), "the crate broke");
    assert!(
        (3..=5).contains(&shots),
        "after a few pistol rounds ({shots})"
    );
    assert!(items(&mut app) > before, "and what was in it spilled out");
    assert!(
        app.world()
            .resource::<Broken>()
            .0
            .contains(&key(t.translation))
    );
}

#[test]
fn whats_broken_stays_broken_until_the_surge() {
    let dir = std::env::temp_dir().join(format!("extraction-destruction-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("career.ron");
    let _ = std::fs::remove_file(&path);
    let mut a = app(Some(path.clone()));
    let (pane, at, out) = open_pane(&mut a);
    shoot(&mut a, at + out * 2.0, at, 700.0, 7.9, 40.0);
    run(&mut a, 3);
    // (A round through one window may go on through the one opposite.)
    let panes_broken = a.world().resource::<Broken>().0.len();
    assert!(panes_broken >= 1);
    let (door, collider, mid, n) = plank_door(&mut a);
    for _ in 0..10 {
        a.world_mut().write_message(Hit {
            blow: Some(extraction_core::melee::Blow::Blunt),
            entity: collider,
            point: mid,
            normal: n,
            velocity: -n * 6.0,
            damage: 60.0,
            shooter: None,
            mass_g: 0.0,
            load: Default::default(),
        });
        run(&mut a, 1);
    }
    run(&mut a, 3);
    assert!(a.world().get::<Pane>(pane).unwrap().broken.is_some());
    assert!(a.world().get::<Door>(door).is_none());
    let crates_a = a
        .world_mut()
        .query_filtered::<(), With<SupplyCrate>>()
        .iter(a.world())
        .count();
    let save = crate::persist::snapshot(a.world_mut());
    assert_eq!(
        save.zone.broken.len(),
        panes_broken + 1,
        "the panes and a door: {:?}",
        save.zone.broken
    );
    let mut career = a
        .world()
        .resource::<crate::persist::CareerState>()
        .0
        .clone();
    career.store(&save);
    crate::persist::write_career(&path, &career);

    let mut b = app(Some(path.clone()));
    run(&mut b, 3);
    let broken_panes: Vec<Vec3> = b
        .world_mut()
        .query::<&Pane>()
        .iter(b.world())
        .filter(|p| p.broken.is_some_and(|b| !b.weathered))
        .map(|p| p.centre())
        .collect();
    assert_eq!(
        broken_panes.len(),
        panes_broken,
        "the panes are still broken"
    );
    assert!(broken_panes.iter().any(|p| p.distance(at) < 0.02));
    let doors_at = |app: &mut App| {
        app.world_mut()
            .query::<(&DoorLeaf, &GlobalTransform)>()
            .iter(app.world())
            .filter(|(l, g)| {
                g.transform_point(Vec3::new(l.0.width * 0.5, 1.1, 0.0))
                    .distance(mid)
                    < 0.05
            })
            .count()
    };
    assert_eq!(doors_at(&mut b), 0, "the door is still down");
    assert_eq!(live_debris(&mut b), 0, "restored quietly: no splinters");
    let crates_b = b
        .world_mut()
        .query_filtered::<(), With<SupplyCrate>>()
        .iter(b.world())
        .count();
    assert_eq!(crates_b, crates_a, "the crates are where they were");

    // The Surge mends it all, and doubles nothing.
    crate::persist::reset_zone(b.world_mut());
    run(&mut b, 3);
    assert!(
        b.world_mut()
            .query::<&Pane>()
            .iter(b.world())
            .all(|p| p.broken.is_none_or(|b| b.weathered)),
        "every pane whole (but the years' breakage)"
    );
    assert_eq!(doors_at(&mut b), 1, "the door hangs again");
    let crates_c = b
        .world_mut()
        .query_filtered::<(), With<SupplyCrate>>()
        .iter(b.world())
        .count();
    assert_eq!(crates_c, crates_a, "no crate doubled");
    assert!(b.world().resource::<Broken>().0.is_empty());
    // The robots know the new door.
    let entrances = b.world().resource::<crate::robots::doors::Entrances>();
    assert!(
        entrances
            .doors
            .iter()
            .all(|e| b.world().get::<Door>(e.door).is_some()),
        "every doorway's door exists"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_debris_storm_stays_in_budget_and_despawns_cleanly() {
    let mut app = app(None);
    let (_, at, _) = open_pane(&mut app);
    let mut rng = extraction_core::noise::Rng::new(3);
    for tick in 0..240 {
        // Breaks every tick for two seconds, far past the cap.
        if tick < 180 {
            let world = app.world_mut();
            let mut p = std::mem::take(&mut *world.resource_mut::<DebrisPool>());
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let mut c = Commands::new(&mut queue, world);
                let pieces = (0..12)
                    .map(|i| super::debris::Piece {
                        kind: super::debris::Kind::Glass,
                        poly: vec![[-0.05, -0.04], [0.06, -0.03], [0.0, 0.07]],
                        thick: 0.004,
                        at: Transform::from_translation(
                            at + Vec3::new(i as f32 * 0.07, 1.0, rng.range(-0.3, 0.3)),
                        ),
                        velocity: Vec3::new(rng.range(-3.0, 3.0), 2.0, rng.range(-3.0, 3.0)),
                        spin: Vec3::ONE,
                    })
                    .collect();
                super::debris::throw(&mut c, &mut p, pieces);
            }
            queue.apply(world);
            *world.resource_mut::<DebrisPool>() = p;
        }
        // Half of what's flying swept away mid-flight, now and then (the
        // Zone reset, a test harness, anything).
        if tick % 20 == 10 {
            let all: Vec<Entity> = app
                .world_mut()
                .query_filtered::<Entity, With<Debris>>()
                .iter(app.world())
                .collect();
            for e in all.iter().step_by(2) {
                app.world_mut().despawn(*e);
            }
        }
        app.update();
        let live = live_debris(&mut app);
        assert!(live <= DEBRIS_CAP, "tick {tick}: {live} pieces alive");
    }
    let pool = app.world().resource::<DebrisPool>();
    assert!(pool.peak <= DEBRIS_CAP);
    assert!(pool.culled > 0, "the oldest made room");
    // And then they all go.
    run(&mut app, (super::debris::DEBRIS_LIFE * 100.0) as usize);
    assert_eq!(live_debris(&mut app), 0, "every piece cleared away");
}

#[test]
fn a_shop_front_shot_out_stays_in_budget() {
    // Every pane of the valley broken in one tick: a frame's worth of
    // pieces never past the cap, and fixed-step physics keeps up.
    let mut app = app(None);
    let panes: Vec<(Entity, Vec3)> = app
        .world_mut()
        .query::<(Entity, &Pane)>()
        .iter(app.world())
        .map(|(e, p)| (e, p.centre()))
        .collect();
    for (_, c) in panes.iter().take(60) {
        shoot(
            &mut app,
            *c + Vec3::Y * 0.05 + Vec3::X * 0.5,
            *c - Vec3::X * 0.5,
            700.0,
            7.9,
            40.0,
        );
    }
    let t0 = std::time::Instant::now();
    run(&mut app, 90);
    let per_tick = t0.elapsed().as_secs_f32() / 90.0;
    let pool = app.world().resource::<DebrisPool>();
    assert!(pool.peak <= DEBRIS_CAP, "peak {}", pool.peak);
    println!(
        "debris budget: peak {} of {DEBRIS_CAP}, {} thrown, {} culled, {:.2} ms a tick",
        pool.peak,
        pool.spawned,
        pool.culled,
        per_tick * 1000.0
    );
}

/// Not a check: stages a save for the screenshots in
/// `docs/shots/destruction/` (the door_shot house with its front windows
/// and door broken). `SHOT_SAVE=/path cargo test --lib stage_broken_house
/// -- --ignored`, then `--save /path` on the screenshot.
#[test]
#[ignore]
fn stage_broken_house() {
    let Ok(path) = std::env::var("SHOT_SAVE") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let _ = std::fs::remove_file(&path);
    let mut a = app(Some(path.clone()));
    let zone = a.world().resource::<crate::world::Zone>().clone();
    let (_, mid, out) = super::scenario::front_door(&zone).unwrap();
    let panes: Vec<(Vec3, Vec3)> = a
        .world_mut()
        .query::<&Pane>()
        .iter(a.world())
        .filter(|p| p.centre().distance(mid) < 4.0 && (p.centre() - mid).dot(out) > -0.3)
        .map(|p| (p.centre(), p.world.rotation * Vec3::Y))
        .collect();
    // Each struck off-centre, so the teeth differ.
    for (i, (c, up)) in panes.iter().enumerate() {
        let off = *up * if i % 2 == 0 { 0.08 } else { -0.1 };
        shoot(&mut a, *c + out * 3.0 + off, *c + off, 700.0, 7.9, 40.0);
        run(&mut a, 2);
    }
    let save = crate::persist::snapshot(a.world_mut());
    let mut career = a
        .world()
        .resource::<crate::persist::CareerState>()
        .0
        .clone();
    career.store(&save);
    crate::persist::write_career(&path, &career);
    println!("staged {} panes broken at {mid}", panes.len());
}

#[test]
fn a_crate_in_a_house_breaks_open_and_comes_back_with_the_surge() {
    use super::wood::BuildingCrate;
    let mut app = app(None);
    let crates: Vec<(Entity, Vec3, f32)> = app
        .world_mut()
        .query::<(Entity, &BuildingCrate, &Transform)>()
        .iter(app.world())
        .map(|(e, c, t)| (e, t.translation, c.size))
        .collect();
    assert!(
        crates.len() > 5,
        "the houses' crates stand on their own ({})",
        crates.len()
    );
    let before = crates.len();
    let (c, at, size) = crates[0];
    // Blows, as a butt or a crowbar swung.
    let mut blows = 0;
    while app.world().get_entity(c).is_ok() && blows < 20 {
        app.world_mut().write_message(Hit {
            blow: Some(extraction_core::melee::Blow::Blunt),
            entity: c,
            point: at + Vec3::Y * size * 0.5,
            normal: Vec3::Y,
            velocity: Vec3::NEG_Y * 5.0,
            damage: 40.0,
            shooter: None,
            mass_g: 0.0,
            load: Default::default(),
        });
        run(&mut app, 2);
        blows += 1;
    }
    assert!(app.world().get_entity(c).is_err(), "the crate broke");
    assert!(live_debris(&mut app) > 4, "into slats");
    assert!(app.world().resource::<Broken>().0.contains(&key(at)));
    crate::persist::reset_zone(app.world_mut());
    run(&mut app, 3);
    let after = app
        .world_mut()
        .query_filtered::<(), With<BuildingCrate>>()
        .iter(app.world())
        .count();
    assert_eq!(after, before, "every crate back, none doubled");
}

#[test]
fn a_smash_breaks_glass_and_wood_about_it() {
    let mut app = app(None);
    let (pane, at, _) = open_pane(&mut app);
    app.world_mut().write_message(super::hooks::Smash {
        at,
        radius: 0.4,
        damage: 50.0,
        push: Vec3::X * 3.0,
    });
    run(&mut app, 2);
    assert!(app.world().get::<Pane>(pane).unwrap().broken.is_some());
    let (door, _, mid, _) = plank_door(&mut app);
    for _ in 0..6 {
        app.world_mut().write_message(super::hooks::Smash {
            at: mid,
            radius: 0.5,
            damage: 50.0,
            push: Vec3::Z * 3.0,
        });
        run(&mut app, 2);
    }
    assert!(
        app.world().get::<Door>(door).is_none(),
        "the door was torn down"
    );
}

#[test]
fn a_crowbar_wrenches_a_door_once_it_has_the_torque() {
    use super::hooks::{DOOR_PRY, PRY_HOLD, Pried};
    let mut app = app(None);
    let (door, _, _, _) = plank_door(&mut app);
    let bar = app.world_mut().spawn_empty().id();
    // Not hard enough: it holds, however long.
    for _ in 0..90 {
        app.world_mut().write_message(Pried {
            target: door,
            torque: DOOR_PRY * 0.8,
            by: bar,
        });
        run(&mut app, 1);
    }
    assert!(app.world().get::<Door>(door).is_some(), "too weak a pull");
    // Hard enough, but let go too soon: it holds.
    for _ in 0..((PRY_HOLD * 90.0) as usize / 2) {
        app.world_mut().write_message(Pried {
            target: door,
            torque: DOOR_PRY * 1.2,
            by: bar,
        });
        run(&mut app, 1);
    }
    run(&mut app, 2);
    assert!(app.world().get::<Door>(door).is_some(), "too short a pull");
    for _ in 0..((PRY_HOLD * 90.0) as usize + 4) {
        app.world_mut().write_message(Pried {
            target: door,
            torque: DOOR_PRY * 1.2,
            by: bar,
        });
        run(&mut app, 1);
    }
    assert!(app.world().get::<Door>(door).is_none(), "wrenched off");
}

#[test]
fn a_rifle_round_would_reach_through_a_plank_door_and_not_a_steel_one() {
    let mut app = app(None);
    let doors: Vec<(DoorLeaf, GlobalTransform)> = app
        .world_mut()
        .query::<(&DoorLeaf, &GlobalTransform)>()
        .iter(app.world())
        .map(|(l, g)| (*l, *g))
        .collect();
    let world = app.world_mut();
    let mut state = bevy::ecs::system::SystemState::<(
        SpatialQuery,
        crate::weapons::ballistics::Struck,
    )>::new(world);
    let (spatial, struck) = state.get(world).unwrap();
    let line = |l: &DoorLeaf, g: &GlobalTransform| {
        let mid = g.transform_point(Vec3::new(l.0.width * 0.5, 1.4, 0.0));
        let n = g.rotation() * Vec3::Z;
        (mid + n * 1.0, mid - n * 1.0)
    };
    let reach = |from: Vec3, to: Vec3, mass: f32, speed: f32| {
        super::through::would_reach(
            &spatial,
            &struck,
            from,
            to,
            mass,
            speed,
            &|_| false,
            &|_| false,
        )
    };
    let planks = doors
        .iter()
        .filter(|(l, _)| l.0.mat == crate::render::materials::Mat::Planks)
        .map(|(l, g)| line(l, g))
        .collect::<Vec<_>>();
    let steel = doors
        .iter()
        .filter(|(l, _)| l.0.mat != crate::render::materials::Mat::Planks)
        .map(|(l, g)| line(l, g))
        .collect::<Vec<_>>();
    assert!(!planks.is_empty() && !steel.is_empty());
    // Most doors have a clear metre either side; count the lines that do.
    let rifle = planks
        .iter()
        .filter(|(a, b)| reach(*a, *b, 7.9, 715.0))
        .count();
    let pistol = planks
        .iter()
        .filter(|(a, b)| reach(*a, *b, 6.1, 300.0))
        .count();
    let through_steel = steel
        .iter()
        .filter(|(a, b)| reach(*a, *b, 7.9, 715.0))
        .count();
    assert!(
        rifle * 2 > planks.len(),
        "a rifle round gets through plank doors ({rifle} of {})",
        planks.len()
    );
    assert_eq!(pistol, 0, "a pistol round never does");
    assert_eq!(through_steel, 0, "nor any round a steel door");
}

#[test]
fn a_step_in_broken_glass_crunches_and_is_heard() {
    use super::glass::Crunch;
    let mut app = app(None);
    let (pane, at, out) = open_pane(&mut app);
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
        .iter(app.world())
        .next()
        .unwrap();
    let step = |app: &mut App, feet: Vec3| {
        app.world_mut().write_message(crate::player::Footstep {
            entity: player,
            position: feet,
            speed: 1.4,
        });
        app.update();
        app.world().resource::<Messages<Crunch>>().len()
    };
    let p = app.world().get::<Pane>(pane).unwrap().clone();
    // Under the pane, where its glass would fall going away from `out`.
    let feet = at - out * 0.4 - Vec3::Y * (p.half.y + 0.9);
    assert_eq!(step(&mut app, feet), 0, "a whole pane: nothing underfoot");
    shoot(&mut app, at + out * 2.0, at, 700.0, 7.9, 40.0);
    run(&mut app, 3);
    assert!(step(&mut app, feet) > 0, "glass crunches underfoot");
    let heard = app
        .world()
        .resource::<Messages<crate::robots::Noise>>()
        .iter_current_update_messages()
        .any(|n| n.radius == super::glass::CRUNCH_HEARD);
    assert!(heard, "and the robots hear it");
    // A step on the other side of the wall, where none fell: quiet.
    run(&mut app, 3);
    assert_eq!(
        step(&mut app, at + out * 0.4 - Vec3::Y * (p.half.y + 0.9)),
        0
    );
}

#[test]
fn a_heavy_tears_a_hole_in_a_wall_and_it_stays_until_the_surge() {
    use super::breach::Breaches;
    use super::hooks::Breach;
    let dir = std::env::temp_dir().join(format!("extraction-breach-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("career.ron");
    let _ = std::fs::remove_file(&path);
    let mut a = app(Some(path.clone()));
    // The door_shot house: its back wall's middle, at waist height.
    let zone = a.world().resource::<crate::world::Zone>().clone();
    let (_, mid, out) = super::scenario::front_door(&zone).unwrap();
    let houses: Vec<(crate::world::buildings::Building, Transform)> = a
        .world_mut()
        .query::<(&crate::world::buildings::Building, &Transform)>()
        .iter(a.world())
        .filter(|(b, t)| b.prefab == "house_plank" && t.translation.distance(mid) < 6.0)
        .map(|(b, t)| {
            (
                crate::world::buildings::Building {
                    prefab: b.prefab.clone(),
                },
                *t,
            )
        })
        .collect();
    let (_, bxf) = &houses[0];
    let wall = bxf.transform_point(Vec3::new(0.9, 1.2, 3.5));
    let back = bxf.rotation * Vec3::Z;
    let _ = out;
    // A ray along the wall's normal through where the hole will be.
    let blocked = |app: &mut App| {
        let world = app.world_mut();
        let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(world);
        let spatial = state.get(world).unwrap();
        spatial
            .cast_ray(
                wall + back * 1.0,
                Dir3::new(-back).unwrap(),
                2.0,
                true,
                &SpatialQueryFilter::from_mask(crate::world::Layer::World),
            )
            .is_some_and(|h| h.distance < 1.1)
    };
    assert!(blocked(&mut a), "the wall stands");
    a.world_mut().write_message(Breach {
        at: wall,
        out: back,
    });
    run(&mut a, 4);
    assert!(!blocked(&mut a), "a hole through it");
    assert_eq!(a.world().resource::<Breaches>().0.len(), 1);
    // The robots know it for a way in: an open doorway facing out.
    let holes = |app: &mut App| {
        let marks: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<super::breach::BreachHole>>()
            .iter(app.world())
            .collect();
        app.world()
            .resource::<crate::robots::doors::Entrances>()
            .doors
            .iter()
            .filter(|e| marks.contains(&e.door))
            .map(|e| (e.mid, e.out))
            .collect::<Vec<_>>()
    };
    let h = holes(&mut a);
    assert_eq!(h.len(), 1, "the hole is in the doorway index");
    assert!(h[0].1.dot(back) > 0.9, "facing out of the house");
    assert!(live_debris(&mut a) > 6, "its boards fly");
    // Saved, and back as it was.
    let save = crate::persist::snapshot(a.world_mut());
    assert_eq!(save.zone.holes.len(), 1);
    let mut career = a
        .world()
        .resource::<crate::persist::CareerState>()
        .0
        .clone();
    career.store(&save);
    crate::persist::write_career(&path, &career);
    let mut b = app(Some(path.clone()));
    run(&mut b, 3);
    assert!(!blocked(&mut b), "the hole is still there after a reload");
    assert_eq!(holes(&mut b).len(), 1, "and still a way in");
    crate::persist::reset_zone(b.world_mut());
    run(&mut b, 3);
    assert!(blocked(&mut b), "the Surge mends the wall");
    assert!(holes(&mut b).is_empty(), "and the way in is gone");
    assert!(b.world().resource::<Breaches>().0.is_empty());
    let _ = std::fs::remove_file(&path);
}

/// The headset path: a knife in a tracked right hand, the player standing
/// `back` metres out from `target` along `out`, facing it.
fn knife_facing(target: Vec3, out: Vec3, back: f32) -> (crate::vr_harness::Vr, Vec3, Quat) {
    use crate::interact::GripRole;
    use crate::rig::Side;
    let mut vr = crate::vr_harness::Vr::new("marrow_creek");
    let zone = vr.app.world().resource::<crate::world::Zone>().clone();
    let flat = out.with_y(0.0).normalize();
    let stand = target + flat * back;
    let feet = zone.on_ground(stand.x, stand.z);
    // Facing back along `out`: -Z of the turn points at the target.
    let yaw = f32::atan2(flat.x, flat.z);
    vr.stand(feet, yaw);
    let rot = Quat::from_rotation_y(yaw);
    let at = target + flat * 0.6;
    let knife = vr.spawn("knife", Transform::from_translation(at).with_rotation(rot));
    vr.app
        .world_mut()
        .entity_mut(knife)
        .insert(RigidBody::Static);
    let grip = vr.point(knife, |r| *r == GripRole::Primary);
    vr.hand(Side::Right, grip, rot, 1.0, 0.0);
    vr.step(3);
    vr.app
        .world_mut()
        .entity_mut(knife)
        .insert(RigidBody::Dynamic);
    vr.step(10);
    assert_eq!(
        vr.hand_state(Side::Right).holding,
        Some(knife),
        "the knife in hand"
    );
    (vr, grip, rot)
}

/// Drive the right hand from `from` along `dir` at `speed` m/s for
/// `metres`, then back.
fn thrust(vr: &mut crate::vr_harness::Vr, from: Vec3, dir: Vec3, speed: f32, metres: f32) {
    use crate::rig::Side;
    vr.move_hand(Side::Right, from);
    vr.step(10);
    let steps = (metres / (speed / 90.0)).ceil() as usize;
    for i in 1..=steps {
        vr.move_hand(Side::Right, from + dir * (speed / 90.0) * i as f32);
        vr.step(1);
    }
    vr.move_hand(Side::Right, from);
    vr.step(20);
}

#[test]
fn a_knife_driven_through_a_pane_breaks_it_and_a_slow_push_doesnt() {
    let mut probe = app(None);
    let (_, at, out) = open_pane(&mut probe);
    drop(probe);
    let (mut vr, grip, _) = knife_facing(at, out, 0.9);
    let pane = vr
        .app
        .world_mut()
        .query::<(Entity, &Pane)>()
        .iter(vr.app.world())
        .find(|(_, p)| p.centre().distance(at) < 0.01)
        .map(|(e, _)| e)
        .unwrap();
    let dir = -out.with_y(0.0).normalize();
    // Eased up to the glass and a little past, at a walk: it holds.
    thrust(&mut vr, grip, dir, 0.8, 0.75);
    assert!(
        vr.app.world().get::<Pane>(pane).unwrap().broken.is_none(),
        "a slow push doesn't break glass"
    );
    // Driven through: it breaks, and the hand feels it give.
    #[derive(Resource, Default)]
    struct Felt(Vec<crate::rig::haptics::Haptic>);
    vr.app.init_resource::<Felt>().add_systems(
        Update,
        |mut h: MessageReader<crate::rig::haptics::Haptic>, mut f: ResMut<Felt>| {
            f.0.extend(h.read().copied())
        },
    );
    thrust(&mut vr, grip, dir, 4.5, 0.75);
    assert!(
        vr.app.world().get::<Pane>(pane).unwrap().broken.is_some(),
        "a knife driven through breaks it"
    );
    assert!(
        vr.app
            .world()
            .resource::<Felt>()
            .0
            .iter()
            .any(|h| h.side == crate::rig::Side::Right && h.amplitude >= 0.6),
        "a kick in the right hand"
    );
}

#[test]
fn a_knife_stabbed_into_a_crate_breaks_it_open() {
    let mut probe = app(None);
    let (c, t) = probe
        .world_mut()
        .query_filtered::<(Entity, &Transform), With<SupplyCrate>>()
        .iter(probe.world())
        .map(|(e, t)| (e, *t))
        .next()
        .unwrap();
    let _ = c;
    drop(probe);
    // Stand off its side, and stab at its middle.
    let side = t.rotation * Vec3::Z;
    let face = t.translation + side * (crate::items::loot::SUPPLY_CRATE.z * 0.5);
    let (mut vr, grip, _) = knife_facing(face, side, 0.9);
    let crate_e = vr
        .app
        .world_mut()
        .query_filtered::<(Entity, &Transform), With<SupplyCrate>>()
        .iter(vr.app.world())
        .find(|(_, x)| x.translation.distance(t.translation) < 0.01)
        .map(|(e, _)| e)
        .unwrap();
    let start = Vec3::new(grip.x, face.y + 0.1, grip.z);
    let mut stabs = 0;
    while vr.app.world().get_entity(crate_e).is_ok() && stabs < 12 {
        thrust(&mut vr, start, -side, 5.5, 0.55);
        stabs += 1;
    }
    assert!(
        vr.app.world().get_entity(crate_e).is_err(),
        "the crate gave way"
    );
    assert!(stabs >= 2, "after a few stabs ({stabs})");
}

#[test]
fn shards_come_to_rest_and_sleep() {
    // Bodies asleep cost the solver nothing: a window's worth on the
    // floor must settle, not jitter for their whole life.
    let mut app = app(None);
    let (_, at, out) = open_pane(&mut app);
    shoot(&mut app, at + out * 2.0, at, 700.0, 7.9, 40.0);
    run(&mut app, 90 * 5);
    let (asleep, all) = app
        .world_mut()
        .query::<(&Debris, Has<Sleeping>)>()
        .iter(app.world())
        .fold((0, 0), |(s, n), (_, z)| (s + z as usize, n + 1));
    assert!(all > 3, "shards still lying there ({all})");
    assert!(
        asleep * 10 >= all * 8,
        "{asleep} of {all} asleep after five seconds"
    );
}

#[test]
fn a_grenade_at_a_doors_foot_blows_it_in_and_one_further_off_doesnt() {
    use crate::weapons::grenades::GrenadeEvent;
    let blast = |app: &mut App, at: Vec3| {
        let g = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(GrenadeEvent {
            grenade: g,
            event: extraction_core::grenade::Event::Exploded,
            at,
        });
        run(app, 3);
    };
    let mut app = app(None);
    let (door, _, mid, n) = plank_door(&mut app);
    // Four metres out: it rattles.
    blast(&mut app, mid + n * 4.0 - Vec3::Y * 0.6);
    assert!(
        app.world().get::<Door>(door).is_some(),
        "a far blast leaves it"
    );
    // At its foot: in it goes.
    blast(&mut app, mid + n * 0.5 - Vec3::Y * 0.6);
    assert!(
        app.world().get::<Door>(door).is_none(),
        "a near blast blows it in"
    );
}

#[test]
fn a_blade_bites_wood_less_than_a_butt() {
    use extraction_core::melee::Blow;
    let hit = |blow, mass_g| Hit {
        entity: Entity::PLACEHOLDER,
        point: Vec3::ZERO,
        normal: Vec3::Y,
        velocity: Vec3::X,
        damage: 40.0,
        shooter: None,
        mass_g,
        load: Default::default(),
        blow,
    };
    let share = super::wood::blow_share;
    assert_eq!(share(&hit(Some(Blow::Blunt), 0.0), true), 1.0);
    assert_eq!(share(&hit(Some(Blow::Edge), 0.0), true), 0.5);
    assert_eq!(share(&hit(Some(Blow::Point), 0.0), true), 0.25);
    assert_eq!(share(&hit(None, 7.9), true), super::wood::ROUND_ON_DOOR);
    assert_eq!(
        share(&hit(None, 7.9), false),
        1.0,
        "a crate's slats take a round whole"
    );
    assert_eq!(share(&hit(None, 0.0), true), 1.0, "a blast");
}

#[test]
fn boards_over_a_window_take_rounds_and_a_crowbar() {
    use super::boards::{BOARD_PRY, WindowBoard, spawn_boards};
    use super::hooks::{PRY_HOLD, Pried};
    let mut app = app(None);
    let (b, t) = app
        .world_mut()
        .query::<(&crate::world::buildings::Building, &Transform)>()
        .iter(app.world())
        .find(|(b, _)| b.prefab == "house_plank")
        .map(|(b, t)| {
            (
                crate::world::buildings::Building {
                    prefab: b.prefab.clone(),
                },
                *t,
            )
        })
        .unwrap();
    // Its first window boarded (World models decides which, in the game;
    // these are ours on top of those).
    let before: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<WindowBoard>>()
        .iter(app.world())
        .collect();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut c = Commands::new(&mut queue, app.world());
        spawn_boards(&mut c, &b, &t, &[true], &Broken::default());
    }
    queue.apply(app.world_mut());
    run(&mut app, 2);
    let boards: Vec<(Entity, GlobalTransform)> = app
        .world_mut()
        .query_filtered::<(Entity, &GlobalTransform), With<WindowBoard>>()
        .iter(app.world())
        .filter(|(e, _)| !before.contains(e))
        .map(|(e, g)| (e, *g))
        .collect();
    assert_eq!(boards.len(), 3, "three boards across the window");
    // Outside the glass, on the street side: a round from out front hits
    // a board first, and two rifle rounds split it.
    let (b0, g0) = boards[0];
    let out = g0.rotation() * Vec3::NEG_Z;
    let c = g0.translation();
    let mut shots = 0;
    while app.world().get_entity(b0).is_ok() && shots < 6 {
        shoot(&mut app, c + out * 3.0, c, 715.0, 7.9, 40.0);
        run(&mut app, 3);
        shots += 1;
    }
    assert!(app.world().get_entity(b0).is_err(), "the board split");
    assert!(shots <= 3, "after a couple of rounds ({shots})");
    assert!(app.world().resource::<Broken>().0.contains(&key(c)));
    // The next pried off at its end.
    let (b1, _) = boards[1];
    let bar = app.world_mut().spawn_empty().id();
    for _ in 0..((PRY_HOLD * 90.0) as usize + 4) {
        app.world_mut().write_message(Pried {
            target: b1,
            torque: BOARD_PRY * 1.2,
            by: bar,
        });
        run(&mut app, 1);
    }
    assert!(app.world().get_entity(b1).is_err(), "pried off");
    assert!(live_debris(&mut app) > 2, "in splinters");
}

#[test]
fn a_hole_in_a_doorless_building_puts_it_in_the_robots_index() {
    use super::hooks::Breach;
    let mut app = app(None);
    run(&mut app, 5);
    let entrances = app.world().resource::<crate::robots::doors::Entrances>();
    assert!(!entrances.shells.is_empty(), "the index is built");
    let indexed: Vec<Entity> = entrances.shells.iter().map(|s| s.building).collect();
    // A building with a breachable wall and no door.
    let found = app
        .world_mut()
        .query::<(Entity, &crate::world::buildings::Building, &Transform)>()
        .iter(app.world())
        .filter(|(e, ..)| !indexed.contains(e))
        .find_map(|(e, b, t)| {
            let geo = crate::world::buildings::prefab(&b.prefab)?;
            let i = geo.pieces.iter().position(|p| {
                matches!(p.shape, crate::world::buildings::Shape::Box(s) if s.z < 0.45 && s.y > 1.6 && s.x > 1.6)
                    && p.collide
                    && super::breach::breachable(p.mat)
            })?;
            Some((e, t.transform_point(geo.pieces[i].at.translation), *t))
        });
    let Some((e, at, t)) = found else {
        panic!("Marrow Creek has a doorless building with a plank wall");
    };
    app.world_mut().write_message(Breach {
        at,
        out: t.rotation * Vec3::Z,
    });
    run(&mut app, 3);
    let entrances = app.world().resource::<crate::robots::doors::Entrances>();
    assert!(
        entrances.shells.iter().any(|s| s.building == e),
        "the building has a box to be inside"
    );
}

/// Not a check: where Marrow Creek's boarded windows are, for the shots.
#[test]
#[ignore]
fn list_boards() {
    let mut app = app(None);
    run(&mut app, 3);
    for g in app
        .world_mut()
        .query_filtered::<&GlobalTransform, With<super::boards::WindowBoard>>()
        .iter(app.world())
    {
        let out = g.rotation() * Vec3::NEG_Z;
        println!("board at {:.2} out {:.2}", g.translation(), out);
    }
}

#[test]
fn a_wrecks_door_comes_off_to_a_crowbar_and_not_to_rounds() {
    use super::hooks::{PRY_HOLD, Pried};
    use super::strip::{Part, Parts, Strippable, spawn_part};
    let mut app = app(None);
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let at = zone.on_ground(40.0, 280.0) + Vec3::Y * 1.0;
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut parts = Parts::default();
        let mut c = Commands::new(&mut queue, app.world());
        spawn_part(
            &mut c,
            &mut parts,
            &Broken::default(),
            Part {
                at: Transform::from_translation(at),
                size: Vec3::new(0.95, 1.85, 0.04),
                mat: crate::render::materials::Mat::DarkMetal,
                yields: vec![("scrap_metal".into(), 3)],
                pry: 200.0,
                model: None,
                model_at: Transform::IDENTITY,
                hp: None,
                shadow: true,
                nuts: false,
            },
        );
    }
    queue.apply(app.world_mut());
    run(&mut app, 2);
    let door = app
        .world_mut()
        .query_filtered::<(Entity, &GlobalTransform), With<Strippable>>()
        .iter(app.world())
        .find(|(_, g)| g.translation().distance(at) < 0.01)
        .unwrap()
        .0;
    // Rounds hole it; they don't tear it off.
    for _ in 0..5 {
        shoot(&mut app, at + Vec3::Z * 3.0, at, 715.0, 7.9, 40.0);
        run(&mut app, 2);
    }
    assert!(
        app.world().get_entity(door).is_ok(),
        "rounds don't strip a wreck"
    );
    let scrap = |app: &mut App| {
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let id = catalog.kind("scrap_metal").unwrap();
        app.world_mut()
            .query::<&crate::items::Item>()
            .iter(app.world())
            .filter(|i| i.kind() == id)
            .count()
    };
    let before = scrap(&mut app);
    let bar = app.world_mut().spawn_empty().id();
    for _ in 0..((PRY_HOLD * 90.0) as usize + 4) {
        app.world_mut().write_message(Pried {
            target: door,
            torque: 240.0,
            by: bar,
        });
        run(&mut app, 1);
    }
    assert!(app.world().get_entity(door).is_err(), "prised off");
    assert_eq!(scrap(&mut app), before + 3, "for its scrap");
    assert!(app.world().resource::<Broken>().0.contains(&key(at)));
}

#[test]
fn a_door_gone_leaves_nothing_in_its_frame() {
    // Nothing of the leaf (its collider, its knobs, its hinges) stays
    // standing in the doorway once it's broken: its children go with it,
    // and what flies is debris on its own layer.
    let mut app = app(None);
    let (door, collider, mid, n) = plank_door(&mut app);
    let kids: Vec<Entity> = app
        .world()
        .get::<Children>(door)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    for _ in 0..12 {
        app.world_mut().write_message(Hit {
            entity: collider,
            point: mid,
            normal: n,
            velocity: -n * 6.0,
            damage: 60.0,
            shooter: None,
            mass_g: 0.0,
            load: Default::default(),
            blow: Some(extraction_core::melee::Blow::Blunt),
        });
        run(&mut app, 1);
    }
    run(&mut app, 3);
    assert!(app.world().get_entity(door).is_err(), "the door went");
    for k in kids {
        assert!(app.world().get_entity(k).is_err(), "and all of its parts");
    }
    // The doorway, from knee to head height, holds no world collider.
    let world = app.world_mut();
    let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(world);
    let spatial = state.get(world).unwrap();
    for dy in [-0.6, 0.0, 0.5] {
        let hits = spatial.shape_intersections(
            &Collider::sphere(0.15),
            mid + Vec3::Y * dy,
            Quat::IDENTITY,
            &SpatialQueryFilter::from_mask(crate::world::Layer::World),
        );
        assert!(
            hits.is_empty(),
            "something stands in the doorway at {dy}: {hits:?}"
        );
    }
}

/// The door_shot house's back wall at a point a little off its middle
/// (building space (0.9, y, 3.5)), the way out through it, and a ray that
/// says whether the wall stands there.
fn back_wall(app: &mut App, y: f32) -> (Vec3, Vec3) {
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let (_, mid, _) = super::scenario::front_door(&zone).unwrap();
    let bxf = app
        .world_mut()
        .query::<(&crate::world::buildings::Building, &Transform)>()
        .iter(app.world())
        .find(|(b, t)| b.prefab == "house_plank" && t.translation.distance(mid) < 6.0)
        .map(|(_, t)| *t)
        .unwrap();
    (
        bxf.transform_point(Vec3::new(0.9, y, 3.5)),
        bxf.rotation * Vec3::Z,
    )
}

fn wall_stands(app: &mut App, at: Vec3, out: Vec3) -> bool {
    let world = app.world_mut();
    let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(world);
    let spatial = state.get(world).unwrap();
    spatial
        .cast_ray(
            at + out * 1.0,
            Dir3::new(-out).unwrap(),
            2.0,
            true,
            &SpatialQueryFilter::from_mask(crate::world::Layer::World),
        )
        .is_some_and(|h| h.distance < 1.1)
}

#[test]
fn sustained_fire_wears_a_hole_through_a_plank_wall() {
    let mut app = app(None);
    let (at, out) = back_wall(&mut app, 1.4);
    assert!(wall_stands(&mut app, at, out));
    let mut rounds = 0;
    while wall_stands(&mut app, at, out) && rounds < 30 {
        // A burst into one patch, a few centimetres of scatter.
        let j = Vec3::new(
            (rounds as f32 * 1.3).sin(),
            (rounds as f32 * 2.1).cos(),
            0.0,
        ) * 0.06;
        shoot(&mut app, at + out * 4.0 + j, at + j, 715.0, 7.9, 40.0);
        run(&mut app, 2);
        rounds += 1;
    }
    assert!(!wall_stands(&mut app, at, out), "a hole worn through");
    assert!(
        (8..=12).contains(&rounds),
        "after a burst's worth ({rounds} rounds)"
    );
    // (The rounds went on through the planks and wore what's behind too.)
    let holes = app.world().resource::<super::breach::Breaches>().0.clone();
    let ours = holes
        .iter()
        .find(|h| h.at.distance(at) < 0.5)
        .expect("a hole where the burst went in");
    assert!(
        !ours.floor && ours.size.x < 0.6,
        "a head-sized hole: {ours:?}"
    );
    // A metre along, the wall still stands.
    let right = at + (Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) * out) * 1.0;
    assert!(wall_stands(&mut app, right, out));
}

#[test]
fn a_grenade_at_a_walls_foot_blows_a_way_in() {
    use crate::weapons::grenades::GrenadeEvent;
    let mut app = app(None);
    let (at, out) = back_wall(&mut app, 0.9);
    let foot = at - Vec3::Y * 0.8 + out * 0.4;
    let g = app.world_mut().spawn_empty().id();
    app.world_mut().write_message(GrenadeEvent {
        grenade: g,
        event: extraction_core::grenade::Event::Exploded,
        at: foot,
    });
    run(&mut app, 4);
    let holes = app.world().resource::<super::breach::Breaches>().0.clone();
    assert_eq!(holes.len(), 1, "a hole blown");
    assert!(holes[0].floor, "down to the floor: {:?}", holes[0]);
    assert!(!wall_stands(&mut app, at, out), "through the wall");
    let marks = app
        .world_mut()
        .query_filtered::<(), With<super::breach::BreachHole>>()
        .iter(app.world())
        .count();
    assert_eq!(marks, 1, "and the robots know it for a way in");
    // Saved and back, at its size.
    let saved = super::breach::Breaches(holes.clone()).to_save();
    let back = super::breach::Breaches::from_save(&saved, &[]);
    assert!((back.0[0].size - holes[0].size).length() < 0.02);
    assert!(back.0[0].at.distance(holes[0].at) < 0.02);
}

#[test]
fn a_grenade_in_the_open_holes_nothing() {
    use crate::weapons::grenades::GrenadeEvent;
    let mut app = app(None);
    let (at, out) = back_wall(&mut app, 0.9);
    let g = app.world_mut().spawn_empty().id();
    app.world_mut().write_message(GrenadeEvent {
        grenade: g,
        event: extraction_core::grenade::Event::Exploded,
        at: at + out * 3.0,
    });
    run(&mut app, 4);
    assert!(
        app.world()
            .resource::<super::breach::Breaches>()
            .0
            .is_empty()
    );
}

#[test]
fn a_crate_gone_clears_its_space() {
    #[derive(Resource, Default)]
    struct Seen(Vec<super::Cleared>);
    let mut app = app(None);
    app.init_resource::<Seen>().add_systems(
        FixedUpdate,
        |mut c: MessageReader<super::Cleared>, mut s: ResMut<Seen>| s.0.extend(c.read().copied()),
    );
    let (c, t) = app
        .world_mut()
        .query_filtered::<(Entity, &Transform), With<SupplyCrate>>()
        .iter(app.world())
        .map(|(e, t)| (e, *t))
        .next()
        .unwrap();
    for _ in 0..6 {
        app.world_mut().write_message(Hit {
            entity: c,
            point: t.translation,
            normal: Vec3::Y,
            velocity: Vec3::NEG_Y,
            damage: 40.0,
            shooter: None,
            mass_g: 0.0,
            load: Default::default(),
            blow: Some(extraction_core::melee::Blow::Blunt),
        });
        run(&mut app, 2);
    }
    let seen = &app.world().resource::<Seen>().0;
    assert!(
        seen.iter().any(|c| {
            let p = t.translation;
            p.cmpge(c.min).all() && p.cmple(c.max).all()
        }),
        "its space is clear: {seen:?}"
    );
}

#[test]
fn shutters_splinter_and_the_bus_strips_and_the_surge_puts_them_back() {
    use super::hooks::{PRY_HOLD, Pried};
    use super::strip::{BUS_PRY, SHUTTER_HP, Splinters, Strippable};
    let mut app = app(None);
    run(&mut app, 3);
    let parts = |app: &mut App| -> Vec<(Entity, Vec3, f32, bool, Vec<(String, u32)>)> {
        app.world_mut()
            .query::<(
                Entity,
                &GlobalTransform,
                &Timber,
                Has<Splinters>,
                &Strippable,
            )>()
            .iter(app.world())
            .map(|(e, g, t, s, p)| (e, g.translation(), t.max, s, p.yields.clone()))
            .collect()
    };
    let all = parts(&mut app);
    let shutters: Vec<_> = all.iter().filter(|p| p.2 == SHUTTER_HP && p.3).collect();
    assert!(
        shutters.len() >= 8,
        "Marrow's houses have shutters ({})",
        shutters.len()
    );
    let fence = all.iter().filter(|p| p.2 == super::strip::FENCE_HP).count();
    // The bus's (the car wrecks' parts yield car_*).
    let bus: Vec<_> = all
        .iter()
        .filter(|p| !p.3 && p.4.iter().all(|(id, _)| !id.starts_with("car_")))
        .collect();
    assert_eq!(bus.len(), 5, "the depot's bus: a door and four panels");
    println!("{} shutters, {fence} fence panels", shutters.len());

    // A shutter beaten: it splinters, and stays gone.
    let (s, at, ..) = shutters[0].clone();
    let debris = live_debris(&mut app);
    for _ in 0..4 {
        if app.world().get_entity(s).is_err() {
            break;
        }
        app.world_mut().write_message(Hit {
            entity: s,
            point: at,
            normal: Vec3::Y,
            velocity: Vec3::NEG_Y * 6.0,
            damage: 60.0,
            shooter: None,
            mass_g: 0.0,
            load: Default::default(),
            blow: Some(extraction_core::melee::Blow::Blunt),
        });
        run(&mut app, 1);
    }
    run(&mut app, 2);
    assert!(app.world().get_entity(s).is_err(), "the shutter broke");
    assert!(live_debris(&mut app) > debris, "in splinters");
    assert!(app.world().resource::<Broken>().0.contains(&key(at)));

    // The bus's door to a crowbar: scrap.
    let (door, door_at, ..) = bus
        .iter()
        .find(|p| p.4.iter().any(|(id, _)| id == "scrap_metal"))
        .cloned()
        .cloned()
        .expect("the bus door yields scrap");
    let scrap = |app: &mut App| {
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let id = catalog.kind("scrap_metal").unwrap();
        app.world_mut()
            .query::<&crate::items::Item>()
            .iter(app.world())
            .filter(|i| i.kind() == id)
            .count()
    };
    let before = scrap(&mut app);
    let out = app.world().get::<GlobalTransform>(door).unwrap().rotation() * Vec3::NEG_Z;
    let bar = app.world_mut().spawn_empty().id();
    for _ in 0..((PRY_HOLD * 90.0) as usize + 4) {
        app.world_mut().write_message(Pried {
            target: door,
            torque: BUS_PRY * 1.2,
            by: bar,
        });
        run(&mut app, 1);
    }
    assert!(app.world().get_entity(door).is_err(), "prised off");
    assert_eq!(scrap(&mut app), before + 1, "for its scrap");
    // Dropped outside it, not into what it was fixed to.
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let id = catalog.kind("scrap_metal").unwrap();
    let dropped = app
        .world_mut()
        .query::<(&crate::items::Item, &GlobalTransform)>()
        .iter(app.world())
        .filter(|(i, _)| i.kind() == id)
        .map(|(_, g)| g.translation())
        .min_by(|a, b| a.distance(door_at).total_cmp(&b.distance(door_at)))
        .unwrap();
    assert!(
        (dropped - door_at).dot(out) > 0.1,
        "the scrap lands outside the bus ({dropped} from {door_at})"
    );

    // The Surge: both back, and nothing doubled.
    super::surge(app.world_mut());
    run(&mut app, 2);
    let after = parts(&mut app);
    assert_eq!(after.len(), all.len(), "every part back, once");
    for p in [at, door_at] {
        assert!(after.iter().any(|q| q.1.distance(p) < 0.01), "back at {p}");
    }
}

/// Not a check: where Marrow Creek's shutters, fence panels and the bus's
/// parts are, for the shots.
#[test]
#[ignore]
fn list_parts() {
    let mut app = app(None);
    run(&mut app, 3);
    for (g, t, s) in app
        .world_mut()
        .query::<(&GlobalTransform, &Timber, Has<super::strip::Splinters>)>()
        .iter(app.world())
    {
        if t.max == super::wood::DOOR_HP || t.max == super::wood::CRATE_HP {
            continue;
        }
        let out = g.rotation() * Vec3::NEG_Z;
        println!(
            "part hp {} wood {s} at {:.2} out {:.2}",
            t.max,
            g.translation(),
            out
        );
    }
}

#[test]
fn nothing_we_spawn_is_drawn_without_its_model_or_its_texture() {
    // What a breakable is drawn with (`wood::Drawn`): a model that's on
    // disk, or a box in a material whose texture is (a plain dark metal
    // is plain on purpose). Never a box in a material that never loads,
    // which draws pale and flat.
    let root = crate::asset_root_path();
    let model = |m: &str| {
        let (m, _) = super::wood::model_node(m);
        root.join(crate::render::world_props::path(m)).exists()
    };
    // Textures are fetched (`scripts/fetch_assets.py`), not in git: a
    // material is textured if the fetch manifest has its set, whether or
    // not this checkout has fetched it.
    let manifest = root.parent().unwrap().join("scripts/assets_manifest.json");
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        eprintln!("no {} (a packaged build): skipped", manifest.display());
        return;
    };
    let sets: serde_json::Value = serde_json::from_str(&text).unwrap();
    let sets = &sets["materials"]["sets"];
    let textured = |mat: crate::render::materials::Mat| {
        let (dir, ..) = mat.info();
        dir.is_none_or(|d| sets.get(d).is_some())
    };
    let mut bad = Vec::new();
    for m in super::wood::CRATE_MODELS
        .into_iter()
        .chain([super::boards::MODEL])
    {
        if !model(m) {
            bad.push(format!("model {m} isn't on disk"));
        }
    }
    let ids = crate::world::buildings::PREFABS
        .into_iter()
        .chain(crate::world::underground::ids());
    for id in ids {
        let Some(geo) = crate::world::buildings::prefab(id) else {
            continue;
        };
        for b in &geo.breakables {
            match b.model {
                Some(m) if !model(m) => bad.push(format!("{id}: {:?}'s {m} isn't on disk", b.kind)),
                None if !textured(b.mat) => bad.push(format!(
                    "{id}: {:?} is a box of {:?}, untextured",
                    b.kind, b.mat
                )),
                _ => {}
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn a_garden_fence_panel_breaks_between_its_posts_and_comes_back() {
    use super::strip::{FENCE_HP, Strippable};
    use bevy::ecs::system::RunSystemOnce;
    let mut app = app(None);
    run(&mut app, 3);
    // No valley has one yet (Map design's): stand one in a clearing.
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let at = zone.on_ground(40.0, 280.0);
    app.world_mut().spawn((
        crate::world::buildings::Building {
            prefab: "fence_garden".into(),
        },
        Transform::from_translation(at),
    ));
    run(&mut app, 3);
    let panels = |app: &mut App| -> Vec<(Entity, Vec3)> {
        app.world_mut()
            .query_filtered::<(Entity, &GlobalTransform, &Timber), With<Strippable>>()
            .iter(app.world())
            .filter(|(_, g, t)| t.max == FENCE_HP && g.translation().distance(at) < 6.0)
            .map(|(e, g, _)| (e, g.translation()))
            .collect()
    };
    let up = panels(&mut app);
    assert_eq!(up.len(), 4, "four panels between five posts");
    // Rifle rounds through the boards: a panel splinters, and its posts
    // (the building's) still stand.
    let (p, c) = up[1];
    let debris = live_debris(&mut app);
    let out = app.world().get::<GlobalTransform>(p).unwrap().rotation() * Vec3::NEG_Z;
    let mut shots = 0;
    while app.world().get_entity(p).is_ok() && shots < 6 {
        shoot(&mut app, c + out * 3.0, c, 715.0, 7.9, 40.0);
        run(&mut app, 3);
        shots += 1;
    }
    assert!(app.world().get_entity(p).is_err(), "the panel broke");
    assert!(
        shots >= 2,
        "a fence panel takes more than one round ({shots})"
    );
    assert!(live_debris(&mut app) > debris, "in splinters");
    assert_eq!(panels(&mut app).len(), 3, "the others stand");
    // The gap is clear to walk through: nothing solid where it stood.
    let spatial = app
        .world_mut()
        .run_system_once(move |s: avian3d::prelude::SpatialQuery| {
            s.point_intersections(
                c,
                &SpatialQueryFilter::from_mask(crate::world::Layer::World),
            )
        })
        .unwrap();
    assert!(spatial.is_empty(), "nothing solid in the gap");
    super::surge(app.world_mut());
    run(&mut app, 2);
    assert_eq!(panels(&mut app).len(), 4, "the Surge puts it back");
}

/// A car wreck's separable parts as its model has them: its top-level
/// nodes, but its `body` and its empties (`pt_*`, for loot), read from
/// the glb's JSON chunk.
fn separable_nodes(model: &str) -> Vec<String> {
    let file = crate::asset_root_path().join(crate::render::world_props::path(model));
    let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("{model}: {e}"));
    let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    let scene = json["scene"].as_u64().unwrap_or(0) as usize;
    let mut out: Vec<String> = json["scenes"][scene]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| nodes[i.as_u64()? as usize]["name"].as_str())
        .filter(|n| *n != "body" && !n.starts_with("body_") && !n.starts_with("pt_"))
        .map(str::to_string)
        .collect();
    out.sort();
    out
}

#[test]
fn the_wrecks_come_apart_where_kit_modelled_them() {
    use crate::world::buildings::BreakKind;
    let car = Transform::from_xyz(3.0, 0.0, -2.0).with_rotation(Quat::from_rotation_y(0.4));
    let building = Transform::from_xyz(100.0, 20.0, 50.0).with_rotation(Quat::from_rotation_y(2.1));
    let back = car.compute_affine().inverse();
    for model in ["wreck_lada", "wreck_uaz", "wreck_lorry"] {
        let parts = super::strip::car_parts(model, car);
        // Every separable part Kit modelled, and nothing else: read off
        // the model, so a part added or renamed there shows here.
        let mut ours: Vec<String> = parts
            .iter()
            .map(|b| {
                super::wood::model_node(b.model.unwrap())
                    .1
                    .unwrap()
                    .to_string()
            })
            .collect();
        ours.sort();
        assert_eq!(
            ours,
            separable_nodes(model),
            "{model}'s parts are its model's"
        );
        let count = |k: BreakKind| parts.iter().filter(|p| p.kind == k).count();
        assert_eq!(count(BreakKind::CarWheel), 4, "{model}'s wheels");
        assert_eq!(count(BreakKind::CarBonnet), 1, "{model}'s bonnet");
        let boxes = super::strip::car_body(model);
        assert!(!boxes.is_empty(), "{model} has a body");
        let body = Vec3::new(0.0, 0.6, 0.0);
        for b in &parts {
            let name = b.model.unwrap();
            // Out of the car: its -Z face points away from the body's middle.
            let mid = back.transform_point3(b.at.translation);
            let out = back.transform_vector3(b.at.rotation * Vec3::NEG_Z);
            assert!(
                out.dot(mid - body) > 0.0,
                "{name} at {mid} faces out ({out})"
            );
            // No body box stands over it: what reaches for the part
            // meets the part.
            let half = b.size * 0.5;
            for (c, sz) in &boxes {
                let (lo, hi) = (*c - *sz * 0.5, *c + *sz * 0.5);
                let inside = |p: Vec3| p.cmpgt(lo + 0.01).all() && p.cmplt(hi - 0.01).all();
                let face = mid + out * half.z;
                assert!(
                    !inside(face),
                    "{name}'s outer face is inside the body box {c} {sz}"
                );
            }
            // Thinnest the way it faces.
            assert!(
                b.size.z <= b.size.x.min(b.size.y) + 1e-4,
                "{name}'s box {} is thinnest out",
                b.size
            );
            // Its model lands back in the car's space, wherever the
            // building stands and wherever the car stands in it.
            let p = super::strip::Part::of(b, &building);
            assert_eq!(p.hp, None, "{name}: metal, off to a crowbar only");
            assert!(
                (p.at * p.model_at)
                    .compute_affine()
                    .abs_diff_eq((building * car).compute_affine(), 1e-4),
                "{name} draws where the car is"
            );
            let want = match b.kind {
                BreakKind::CarDoor => "car_door",
                BreakKind::CarWheel => "car_wheel",
                _ => "car_bonnet",
            };
            assert_eq!(b.yields, Some(want), "{name} yields");
        }
    }
}

#[test]
fn a_ladas_door_comes_off_to_a_crowbar_and_the_surge_hangs_it_back() {
    use super::hooks::{PRY_HOLD, Pried};
    use super::strip::{CAR_DOOR_PRY, Strippable};
    let mut app = app(None);
    run(&mut app, 3);
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let at = zone.on_ground(40.0, 280.0);
    app.world_mut().spawn((
        crate::world::buildings::Building {
            prefab: "wreck_lada".into(),
        },
        Transform::from_translation(at),
    ));
    run(&mut app, 3);
    let parts = |app: &mut App| -> Vec<(Entity, Vec3, Vec<(String, u32)>)> {
        app.world_mut()
            .query::<(Entity, &GlobalTransform, &Strippable)>()
            .iter(app.world())
            .filter(|(_, g, _)| g.translation().distance(at) < 4.0)
            .map(|(e, g, s)| (e, g.translation(), s.yields.clone()))
            .collect()
    };
    let on = parts(&mut app);
    assert_eq!(on.len(), 10, "the Lada's ten parts");
    let (door, door_at, _) = on
        .iter()
        .find(|(_, p, y)| y.iter().any(|(i, _)| i == "car_door") && p.x < at.x)
        .cloned()
        .expect("a left door");
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let id = catalog.kind("car_door").expect("Kit's car_door");
    let doors = |app: &mut App| {
        app.world_mut()
            .query::<&crate::items::Item>()
            .iter(app.world())
            .filter(|i| i.kind() == id)
            .count()
    };
    let before = doors(&mut app);
    let bar = app.world_mut().spawn_empty().id();
    for _ in 0..((PRY_HOLD * 90.0) as usize + 4) {
        app.world_mut().write_message(Pried {
            target: door,
            torque: CAR_DOOR_PRY * 1.2,
            by: bar,
        });
        run(&mut app, 1);
    }
    assert!(app.world().get_entity(door).is_err(), "prised off");
    assert_eq!(doors(&mut app), before + 1, "a car door to carry");
    // Lying on its largest face, not standing on an edge.
    let up = app
        .world_mut()
        .query::<(&crate::items::Item, &GlobalTransform)>()
        .iter(app.world())
        .filter(|(i, g)| i.kind() == id && g.translation().distance(door_at) < 2.0)
        .map(|(_, g)| g.rotation())
        .next()
        .expect("the door, by the car");
    let dims = catalog.get(id).dims.expect("car_door's dims");
    let thin = Vec3::from(dims).min_position();
    let axis = up * [Vec3::X, Vec3::Y, Vec3::Z][thin];
    assert!(axis.y.abs() > 0.9, "the door lies flat ({axis})");
    assert!(
        app.world().resource::<Broken>().0.contains(&key(door_at)),
        "and it stays off till the Surge"
    );
    super::surge(app.world_mut());
    run(&mut app, 2);
    assert_eq!(parts(&mut app).len(), 10, "the Surge hangs it back");
}

#[test]
fn a_wheels_nuts_undone_take_it_off_and_the_surge_bolts_it_back() {
    use super::hooks::{PRY_HOLD, Pried, Turned, Unboltable};
    use super::strip::{CAR_WHEEL_PRY, NUT_TURNS, Nuts, WHEEL_NUTS, WheelNut};
    let mut app = app(None);
    run(&mut app, 3);
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let at = zone.on_ground(40.0, 280.0);
    app.world_mut().spawn((
        crate::world::buildings::Building {
            prefab: "wreck_lada".into(),
        },
        Transform::from_translation(at),
    ));
    run(&mut app, 3);
    let wheels = |app: &mut App| -> Vec<(Entity, Vec3, Vec<Entity>)> {
        app.world_mut()
            .query::<(Entity, &GlobalTransform, &Nuts)>()
            .iter(app.world())
            .filter(|(_, g, _)| g.translation().distance(at) < 4.0)
            .map(|(e, g, n)| (e, g.translation(), n.0.clone()))
            .collect()
    };
    let nuts_near = |app: &mut App| {
        app.world_mut()
            .query_filtered::<&GlobalTransform, With<WheelNut>>()
            .iter(app.world())
            .filter(|g| g.translation().distance(at) < 4.0)
            .count()
    };
    let on = wheels(&mut app);
    assert_eq!(on.len(), 4, "four wheels, each on its nuts");
    assert_eq!(
        nuts_near(&mut app),
        4 * WHEEL_NUTS,
        "and the nuts are there"
    );
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let id = catalog.kind("car_wheel").expect("Kit's car_wheel");
    let count = |app: &mut App| {
        app.world_mut()
            .query::<&crate::items::Item>()
            .iter(app.world())
            .filter(|i| i.kind() == id)
            .count()
    };
    let before = count(&mut app);
    let (wheel, wheel_at, nuts) = on[0].clone();
    // A nut sits on the wheel's outer face, undone along it.
    for n in &nuts {
        let g = *app.world().get::<GlobalTransform>(*n).unwrap();
        let out = g.rotation() * app.world().get::<Unboltable>(*n).unwrap().axis;
        assert!(
            (g.translation() - wheel_at).dot(out) > 0.0,
            "a nut on the outside of its wheel"
        );
    }
    let bar = app.world_mut().spawn_empty().id();
    let turn = |app: &mut App, nut: Entity, angle: f32| {
        // A spanner swings about a quarter turn a tick at most.
        let mut left = angle;
        while left > 0.0 {
            let a = left.min(0.4);
            app.world_mut().write_message(Turned {
                target: nut,
                angle: a,
                by: bar,
            });
            run(app, 1);
            left -= a;
        }
    };
    // Three out, the last only half undone: it's still on.
    for n in &nuts[..WHEEL_NUTS - 1] {
        turn(&mut app, *n, NUT_TURNS * 1.05);
        assert!(app.world().get_entity(*n).is_err(), "the nut's out");
    }
    turn(&mut app, nuts[WHEEL_NUTS - 1], NUT_TURNS * 0.5);
    run(&mut app, 2);
    assert!(
        app.world().get_entity(wheel).is_ok(),
        "one nut still holds it"
    );
    assert_eq!(count(&mut app), before, "nothing dropped yet");
    // The last one out: off it comes.
    turn(&mut app, nuts[WHEEL_NUTS - 1], NUT_TURNS * 0.55);
    run(&mut app, 2);
    assert!(app.world().get_entity(wheel).is_err(), "the wheel's off");
    assert_eq!(count(&mut app), before + 1, "a wheel to carry");
    assert!(app.world().resource::<Broken>().0.contains(&key(wheel_at)));
    // Another prised off with the crowbar: its nuts go with it.
    let (other, _, other_nuts) = on[1].clone();
    for _ in 0..((PRY_HOLD * 90.0) as usize + 4) {
        app.world_mut().write_message(Pried {
            target: other,
            torque: CAR_WHEEL_PRY * 1.2,
            by: bar,
        });
        run(&mut app, 1);
    }
    assert!(app.world().get_entity(other).is_err(), "prised off");
    assert!(
        other_nuts
            .iter()
            .all(|n| app.world().get_entity(*n).is_err()),
        "no nut left in the air where it was"
    );
    assert_eq!(
        nuts_near(&mut app),
        2 * WHEEL_NUTS,
        "the others' nuts still on"
    );
    // The Surge: both back, bolted on.
    super::surge(app.world_mut());
    run(&mut app, 2);
    assert_eq!(wheels(&mut app).len(), 4, "four wheels again");
    assert_eq!(nuts_near(&mut app), 4 * WHEEL_NUTS, "every nut back");
}
