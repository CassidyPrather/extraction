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
/// (out of the sash's face).
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
    let panes: Vec<(Entity, Pane)> = app
        .world_mut()
        .query::<(Entity, &Pane)>()
        .iter(app.world())
        .map(|(e, p)| (e, p.clone()))
        .collect();
    assert!(
        panes.len() > 50,
        "the valley's windows are glazed ({})",
        panes.len()
    );
    for (e, p) in panes {
        for out in [p.world.rotation * Vec3::NEG_Z, p.world.rotation * Vec3::Z] {
            let from = p.centre() + out * 2.0;
            if spatial_ok(app, from, p.centre()) {
                return (e, p.centre(), out);
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
        .filter(|p| p.broken.is_some())
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
            .all(|p| p.broken.is_none()),
        "every pane whole"
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
