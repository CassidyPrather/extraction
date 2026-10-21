//! Animals in the whole game, headless: what the machines make of a flock
//! you flush, and birds against every collider in the valley.

use avian3d::prelude::*;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use super::flock::{Flock, Species, new_flock};
use super::perches::Perches;
use super::{FlushCause, Flushed};
use crate::rig::{LocalPlayer, RigInput};
use crate::robots::search::Investigating;
use crate::robots::{Brain, Job, JobKind};
use crate::world::Zone;

#[derive(Resource, Default)]
struct Log {
    flushed: Vec<Flushed>,
    investigating: Vec<Investigating>,
}

fn listen(
    mut f: MessageReader<Flushed>,
    mut i: MessageReader<Investigating>,
    mut log: ResMut<Log>,
) {
    log.flushed.extend(f.read().copied());
    log.investigating.extend(i.read().copied());
}

fn app() -> (App, Entity, Zone) {
    let mut app = crate::headless_app(
        crate::LaunchOptions {
            robots: false,
            ..Default::default()
        },
        false,
    );
    app.init_resource::<Log>().add_systems(Last, listen);
    app.finish();
    app.cleanup();
    for _ in 0..3 {
        app.update();
    }
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(app.world())
        .next()
        .unwrap();
    app.world_mut()
        .get_mut::<RigInput>(player)
        .unwrap()
        .head
        .translation
        .y = 1.65;
    let zone = app.world().resource::<Zone>().clone();
    (app, player, zone)
}

fn put(app: &mut App, player: Entity, at: Vec3) {
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = at + Vec3::Y * 0.05;
}

/// The valley's flocks cleared away and one put on a low roof with
/// walkable ground `away` metres off to one side: the flock, where it sits,
/// and that ground.
fn one_flock(app: &mut App, zone: &Zone, away: f32) -> (Entity, Vec3, Vec3) {
    // The ducks too: their flushes count as birds'.
    let old: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, Or<(With<Flock>, With<super::ducks::Raft>)>>()
        .iter(app.world())
        .collect();
    for e in old {
        app.world_mut().despawn(e);
    }
    let perches = app.world().resource::<Perches>().clone();
    let nav = app
        .world()
        .resource::<crate::robots::walker::Nav>()
        .0
        .clone();
    let (i, post) = perches
        .sites
        .iter()
        .enumerate()
        .filter(|(_, s)| s.spots.len() >= 10 && s.top() < zone.ground(s.centre.x, s.centre.z) + 5.0)
        .find_map(|(i, s)| {
            (0..16).find_map(|k| {
                let a = k as f32 / 16.0 * std::f32::consts::TAU;
                let p = s.centre + Vec3::new(a.cos(), 0.0, a.sin()) * away;
                let p = zone.on_ground(p.x, p.z);
                (nav.walkable([p.x, p.z]) && (p.y - s.centre.y).abs() < 6.0).then_some((i, p))
            })
        })
        .expect("a low roof with open ground beside it");
    let mut rng = extraction_core::noise::Rng::new(1);
    let flock = new_flock(Species::Rook, i, &perches.sites[i], &mut rng);
    let centre = perches.sites[i].centre;
    let e = app
        .world_mut()
        .spawn((Transform::from_translation(centre), flock))
        .id();
    (e, centre, post)
}

/// An unarmed walker on watch at `post`, its back to `behind`.
fn walker_at(app: &mut App, zone: &Zone, post: Vec3, behind: Vec3) -> Entity {
    let mut queue = CommandQueue::default();
    let w = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::robots::walker::spawn_walker_with(
            &mut c,
            zone,
            post,
            Job {
                kind: JobKind::Watch,
                site: post,
                duration: 999.0,
                exit: post,
            },
            false,
            130.0,
        )
    };
    queue.apply(app.world_mut());
    for _ in 0..90 {
        app.update();
    }
    *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Working {
        at: post,
        left: 999.0,
    };
    let away = post - behind;
    app.world_mut()
        .get_mut::<crate::robots::walker::Walker>(w)
        .unwrap()
        .yaw = (-away.x).atan2(-away.z);
    w
}

/// Flush a flock with a walker 35 m off, its back to you and the birds:
/// it hears the birds go up and walks over to look where they were. Stand
/// just out of the flock's reach instead and it never stirs.
#[test]
fn a_walker_goes_to_look_where_you_flushed_the_birds() {
    for flush in [true, false] {
        let (mut app, player, zone) = app();
        let (_, centre, post) = one_flock(&mut app, &zone, 35.0);
        // Standing still 14 m off the far side of the building to start with
        // (the walker's back to you), then (or not) right beside it, under
        // the birds.
        let side = (centre - post).with_y(0.0).normalize();
        let off = centre + side * 14.0;
        put(&mut app, player, zone.on_ground(off.x, off.z));
        let walker = walker_at(&mut app, &zone, post, centre);
        {
            let mut log = app.world_mut().resource_mut::<Log>();
            log.investigating.clear();
            log.flushed.clear();
        }
        if flush {
            // The first open ground out from the middle of the roof (toward
            // the walker's side, where the ground's open): right beside the
            // building, whichever way it's turned.
            let nav = app
                .world()
                .resource::<crate::robots::walker::Nav>()
                .0
                .clone();
            let spot = (4..16)
                .map(|k| centre - side * (k as f32 * 0.5))
                .find(|p| nav.walkable([p.x, p.z]))
                .expect("open ground beside the building");
            put(&mut app, player, zone.on_ground(spot.x, spot.z));
        }
        for _ in 0..90 * 6 {
            app.update();
        }
        let log = app.world().resource::<Log>();
        let ours: Vec<&Flushed> = log
            .flushed
            .iter()
            .filter(|f| f.cause == FlushCause::Player)
            .collect();
        let looked = log
            .investigating
            .iter()
            .filter(|i| i.robot == walker)
            .map(|i| Vec2::new(i.toward.x - centre.x, i.toward.z - centre.z).length())
            .fold(f32::MAX, f32::min);
        if flush {
            assert!(
                ours.first()
                    .is_some_and(|f| f.alarm && f.by == Some(player))
            );
            assert!(looked < 8.0, "went to look {looked:.1} m from the flock");
        } else {
            assert!(ours.is_empty(), "flushed from 14 m standing still");
            assert_eq!(looked, f32::MAX, "investigated with no birds going up");
        }
    }
}

/// Sitting and flying, no bird is ever inside any collider in the valley
/// (buildings, bridges, trees, rocks, fences, the ground), by the physics'
/// own point queries, while you chase a flock round for a minute and a half.
#[test]
fn no_bird_is_ever_inside_a_collider() {
    let (mut app, player, zone) = app();
    let (_, centre, _) = one_flock(&mut app, &zone, 10.0);
    let mut inside = Vec::new();
    for step in 0..90 * 90 {
        // Once it's down, after it: to wherever the flock went.
        if step % 90 == 0 {
            let f = app
                .world_mut()
                .query::<&Flock>()
                .iter(app.world())
                .next()
                .unwrap()
                .clone();
            if f.phase == super::flock::Phase::Settled {
                let at = app.world().resource::<Perches>().sites[f.site].centre;
                let at = if step == 0 { centre } else { at };
                put(&mut app, player, zone.on_ground(at.x + 1.0, at.z + 1.0));
            }
        }
        app.update();
        if step % 9 != 0 {
            continue;
        }
        let birds: Vec<Vec3> = app
            .world_mut()
            .query::<&Flock>()
            .iter(app.world())
            .flat_map(|f| {
                f.birds.iter().map(|b| match b.state {
                    super::flock::BirdState::Sitting(_) => b.pos + Vec3::Y * 0.12,
                    _ => b.pos,
                })
            })
            .collect();
        let mut q = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
        let spatial = q.get(app.world()).unwrap();
        let filter = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
        for p in birds {
            if !spatial.point_intersections(p, &filter).is_empty() {
                inside.push(p);
            }
        }
    }
    let flushes = app.world().resource::<Log>().flushed.len();
    assert!(flushes >= 2, "chased, the flock went up {flushes} times");
    assert!(
        inside.is_empty(),
        "{} bird positions inside colliders: {:?}",
        inside.len(),
        &inside[..inside.len().min(5)]
    );
}

/// A deer is something a bullet finds: a ray down the ballistics' own
/// filter at it hits its body. A hit kills it and leaves a carcass; when
/// the carcass is taken away (a robot collects it), the body goes too. A
/// bird shot off its perch falls and lies on the roof or the ground.
#[test]
fn animals_can_be_shot_and_their_carcasses_collected() {
    use super::bodies::{AnimalBody, Carcass, Killed};
    use super::herd::{Herd, State};
    #[derive(Resource, Default)]
    struct Deaths(Vec<Killed>);
    let mut app = crate::headless_app(
        crate::LaunchOptions {
            robots: false,
            ..Default::default()
        },
        false,
    );
    app.init_resource::<Deaths>().add_systems(
        Last,
        |mut k: MessageReader<Killed>, mut d: ResMut<Deaths>| d.0.extend(k.read().copied()),
    );
    app.finish();
    app.cleanup();
    for _ in 0..30 {
        app.update();
    }
    let (herd_e, deer) = app
        .world_mut()
        .query::<(Entity, &Herd)>()
        .iter(app.world())
        .next()
        .map(|(e, h)| (e, h.beasts[0].pos))
        .expect("Marrow Creek has its deer");
    // From 10 m off, at chest height, at the deer's middle.
    let target = deer + Vec3::Y * 0.6;
    let from = target + Vec3::new(10.0, 0.5, 0.0);
    let hit = {
        let mut q = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
        let spatial = q.get(app.world()).unwrap();
        let filter = SpatialQueryFilter::from_mask([
            crate::world::Layer::World,
            crate::world::Layer::Item,
            crate::world::Layer::Robot,
            crate::world::Layer::Player,
        ]);
        spatial
            .cast_ray(from, Dir3::new(target - from).unwrap(), 20.0, true, &filter)
            .map(|h| h.entity)
    };
    let body = hit.expect("the ray found nothing");
    assert!(
        app.world().get::<AnimalBody>(body).is_some(),
        "the ray hit something else"
    );
    app.world_mut()
        .write_message(crate::weapons::ballistics::Hit {
            entity: body,
            point: target,
            normal: Vec3::X,
            velocity: Vec3::NEG_X * 700.0,
            damage: 48.0,
            shooter: None,
            mass_g: 3.4,
            load: Default::default(),
            blow: None,
        });
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world().get::<Herd>(herd_e).unwrap().beasts[0].state,
        State::Dead
    );
    let carcass = app
        .world_mut()
        .query_filtered::<Entity, With<Carcass>>()
        .iter(app.world())
        .next()
        .expect("a carcass");
    assert_eq!(app.world().resource::<Deaths>().0.len(), 1);
    // The Custodian takes it.
    app.world_mut().despawn(carcass);
    for _ in 0..5 {
        app.update();
    }
    let h = app.world().get::<Herd>(herd_e).unwrap();
    assert!(h.beasts[0].gone, "its body is still there");
    assert!(
        app.world().get_entity(body).is_err(),
        "its collider is still there"
    );
    // A sitting bird, shot: it falls onto whatever's under it.
    let (flock_e, i, perch) = app
        .world_mut()
        .query::<(Entity, &Flock)>()
        .iter(app.world())
        .next()
        .map(|(e, f)| (e, 0, f.birds[0].pos))
        .unwrap();
    let bird_body = app
        .world_mut()
        .query::<(Entity, &AnimalBody)>()
        .iter(app.world())
        .find(|(_, b)| b.owner == flock_e && b.index == i)
        .map(|(e, _)| e)
        .unwrap();
    app.world_mut()
        .write_message(crate::weapons::ballistics::Hit {
            entity: bird_body,
            point: perch,
            normal: Vec3::X,
            velocity: Vec3::NEG_X * 300.0,
            damage: 34.0,
            shooter: None,
            mass_g: 6.0,
            load: Default::default(),
            blow: None,
        });
    for _ in 0..90 * 4 {
        app.update();
    }
    let f = app.world().get::<Flock>(flock_e).unwrap();
    let b = &f.birds[i];
    assert!(!b.state.alive());
    let zone = app.world().resource::<Zone>().clone();
    let obstacles = app.world().resource::<super::perches::Obstacles>().clone();
    let under = obstacles
        .below(b.pos + Vec3::Y * 0.05)
        .unwrap_or(f32::MIN)
        .max(zone.ground(b.pos.x, b.pos.z));
    assert!(
        (b.pos.y - under).abs() < 0.05,
        "it's not lying on anything: {:.2} over {under:.2}",
        b.pos.y
    );
    assert!(app.world().resource::<Deaths>().0.len() >= 2);
}
