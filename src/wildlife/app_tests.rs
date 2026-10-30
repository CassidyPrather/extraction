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
    app_on(None)
}

/// [`app`] on a valley of its own (`None`: the default, Marrow Creek).
fn app_on(map: Option<&str>) -> (App, Entity, Zone) {
    let mut opts = crate::LaunchOptions {
        robots: false,
        ..Default::default()
    };
    if let Some(m) = map {
        opts.map = m.to_string();
    }
    let mut app = crate::headless_app(opts, false);
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
    one_flock_heard(app, zone, away, false)
}

/// [`one_flock`], and if `heard`, with nothing between the birds going up
/// and a walker's ear at that ground for their alarm to carry over (a
/// trunk in the way muffles it like a hill: `acoustics::blocked`).
fn one_flock_heard(app: &mut App, zone: &Zone, away: f32, heard: bool) -> (Entity, Vec3, Vec3) {
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
    let mut q = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
    let spatial = q.get(app.world()).unwrap();
    let open = |centre: Vec3, p: Vec3| {
        let up = centre + Vec3::Y * super::flock::ALARM_HEIGHT;
        !heard || !crate::acoustics::blocked(&spatial, up, p + Vec3::Y * 1.2)
    };
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
                (nav.walkable([p.x, p.z]) && (p.y - s.centre.y).abs() < 6.0 && open(s.centre, p))
                    .then_some((i, p))
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

/// Flush a flock with a walker 75 m off (past where it'd flush them
/// itself), its back to you and the birds:
/// it hears the birds go up and walks over to look where they were. Stand
/// just out of the flock's reach instead and it never stirs.
#[test]
fn a_walker_goes_to_look_where_you_flushed_the_birds() {
    for flush in [true, false] {
        let (mut app, player, zone) = app();
        let (_, centre, post) = one_flock_heard(&mut app, &zone, 75.0, true);
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
            // Heard from afar, where is a guess: within HEARD_SPREAD of
            // the distance.
            let spread = crate::robots::senses::HEARD_SPREAD * post.distance(centre);
            assert!(
                looked < spread.max(8.0),
                "went to look {looked:.1} m from the flock ({spread:.1} m allowed)"
            );
        } else {
            assert!(ours.is_empty(), "flushed from 14 m standing still");
            assert_eq!(looked, f32::MAX, "investigated with no birds going up");
        }
    }
}

/// A stone onto the birds' roof: they go up calling, and a walker 75 m
/// off goes to look where they were. It's a diversion, not a give-away:
/// the alarm is from nobody, so the walker isn't onto you.
#[test]
fn a_stone_onto_their_roof_sends_a_walker_to_look_there() {
    let (mut app, player, zone) = app();
    let (_, centre, post) = one_flock_heard(&mut app, &zone, 75.0, true);
    // You, well away: the far side of the building, standing still.
    let side = (centre - post).with_y(0.0).normalize();
    let off = centre + side * 30.0;
    put(&mut app, player, zone.on_ground(off.x, off.z));
    let walker = walker_at(&mut app, &zone, post, centre);
    // A fresh flock on that roof, sitting (getting the walker to its post
    // may have put the first one up).
    let roof = app.world().resource::<Perches>().clone();
    let i = roof
        .sites
        .iter()
        .position(|s| s.centre.distance(centre) < 0.1)
        .unwrap();
    let old: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<Flock>>()
        .iter(app.world())
        .collect();
    for e in old {
        app.world_mut().despawn(e);
    }
    let mut rng = extraction_core::noise::Rng::new(2);
    let flock = new_flock(Species::Rook, i, &roof.sites[i], &mut rng);
    app.world_mut()
        .spawn((Transform::from_translation(centre), flock));
    {
        let mut log = app.world_mut().resource_mut::<Log>();
        log.investigating.clear();
        log.flushed.clear();
    }
    let site = &roof.sites[i];
    // Thrown: just out of a hand.
    let stone = app.world_mut().spawn_empty().id();
    let now = app.world().resource::<Time>().elapsed_secs();
    app.world_mut()
        .resource_mut::<super::LetGo>()
        .0
        .insert(stone, now);
    for _ in 0..3 {
        app.world_mut()
            .write_message(crate::items::impacts::ItemImpact {
                item: stone,
                at: centre.with_y(site.top()),
                speed: 6.0,
                mass: 0.3,
                material: crate::items::impacts::Material::Wood,
                ground: None,
            });
        app.update();
    }
    for _ in 0..90 * 6 {
        app.update();
    }
    let log = app.world().resource::<Log>();
    let up = log.flushed.first().expect("the flock went up");
    assert_eq!(up.cause, FlushCause::Thrown);
    assert!(up.alarm && up.by.is_none());
    // Gone to look (or looking round) near where the birds were.
    let looked = match app.world().get::<Brain>(walker) {
        Some(Brain::Investigate { at, .. } | Brain::Search { at, .. }) => {
            Vec2::new(at.x - centre.x, at.z - centre.z).length()
        }
        _ => f32::MAX,
    };
    let spread = crate::robots::senses::HEARD_SPREAD * post.distance(centre);
    assert!(
        looked < spread.max(8.0),
        "went to look {looked:.1} m from the flock ({spread:.1} m allowed)"
    );
    let aware = app.world().get::<crate::robots::Awareness>(walker).unwrap();
    assert_ne!(aware.target, Some(player), "it's onto you");
    // And down again somewhere, said for the sound of it.
    #[derive(Resource, Default)]
    struct Down(Vec<super::Alighted>);
    app.init_resource::<Down>().add_systems(
        Last,
        |mut a: MessageReader<super::Alighted>, mut d: ResMut<Down>| d.0.extend(a.read().copied()),
    );
    for _ in 0..90 * 120 {
        app.update();
        if !app.world().resource::<Down>().0.is_empty() {
            break;
        }
    }
    let down = app
        .world()
        .resource::<Down>()
        .0
        .first()
        .copied()
        .expect("never came down");
    assert!(down.birds > 0);
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
    let herd_e = app
        .world_mut()
        .query_filtered::<Entity, With<Herd>>()
        .iter(app.world())
        .next()
        .expect("Marrow Creek has its deer");
    let deer: Vec<Vec3> = app
        .world()
        .get::<Herd>(herd_e)
        .unwrap()
        .beasts
        .iter()
        .map(|b| b.pos)
        .collect();
    let bodies: Vec<(Entity, AnimalBody)> = app
        .world_mut()
        .query::<(Entity, &AnimalBody)>()
        .iter(app.world())
        .map(|(e, b)| (e, *b))
        .collect();
    // A clear line to a deer, as a shooter would find one: 6 to 12 m off,
    // round it, at chest height, the first ray down the ballistics' own
    // filter that finds a deer's body (a rise, a rock or a trunk in the way
    // is the map's business, not this test's).
    let (body, target) = {
        let mut q = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
        let spatial = q.get(app.world()).unwrap();
        let filter = SpatialQueryFilter::from_mask([
            crate::world::Layer::World,
            crate::world::Layer::Item,
            crate::world::Layer::Robot,
            crate::world::Layer::Player,
        ]);
        let mut found = None;
        'look: for &at in &deer {
            let target = at + Vec3::Y * 0.6;
            for d in [6.0, 9.0, 12.0] {
                for k in 0..16 {
                    let a = k as f32 / 16.0 * std::f32::consts::TAU;
                    let from = target + Vec3::new(a.cos() * d, 0.5, a.sin() * d);
                    let Some(h) = spatial.cast_ray(
                        from,
                        Dir3::new(target - from).unwrap(),
                        d + 2.0,
                        true,
                        &filter,
                    ) else {
                        continue;
                    };
                    if bodies
                        .iter()
                        .any(|(e, b)| *e == h.entity && b.owner == herd_e)
                    {
                        found = Some((h.entity, target));
                        break 'look;
                    }
                }
            }
        }
        found.expect("no clear line to any deer: a bullet can't find one")
    };
    let index = app.world().get::<AnimalBody>(body).unwrap().index;
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
        app.world().get::<Herd>(herd_e).unwrap().beasts[index].state,
        State::Dead
    );
    let carcass = app
        .world_mut()
        .query_filtered::<Entity, With<Carcass>>()
        .iter(app.world())
        .next()
        .expect("a carcass");
    assert_eq!(app.world().resource::<Deaths>().0.len(), 1);
    // A body to drag: it comes to rest on the ground, lying, and the deer
    // is drawn where it's dragged to.
    {
        let w = app.world();
        assert!(
            w.get::<crate::interact::carry::Draggable>(carcass)
                .is_some()
        );
        assert!(w.get::<crate::interact::Grabbable>(carcass).is_some());
        assert_eq!(w.get::<Mass>(carcass).map(|m| m.0), Some(50.0));
    }
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let lying = |app: &App| {
        let t = app.world().get::<Transform>(carcass).unwrap().translation;
        let b = &app.world().get::<Herd>(herd_e).unwrap().beasts[index];
        (t, b.pos)
    };
    // Drawn on the ground under it (the box, kept level, may rest on its
    // downhill edge).
    let under = |t: Vec3| t.with_y(zone.ground(t.x, t.z));
    for _ in 0..90 * 3 {
        app.update();
    }
    let (t, drawn) = lying(&app);
    let ground = zone.ground(t.x, t.z);
    assert!(
        (0.1..0.5).contains(&(t.y - ground)),
        "the carcass at {t}, the ground {ground:.2}"
    );
    assert!(drawn.distance(under(t)) < 0.01);
    let to = t + Vec3::new(3.0, 0.0, 0.0);
    let to = to.with_y(zone.ground(to.x, to.z) + 0.3);
    app.world_mut()
        .entity_mut(carcass)
        .insert((RigidBody::Dynamic, Position(to)));
    for _ in 0..90 {
        app.update();
    }
    let (t, drawn) = lying(&app);
    assert!(
        t.xz().distance(to.xz()) < 0.3,
        "dragged to {to}, lies at {t}"
    );
    assert!(drawn.distance(under(t)) < 0.01, "drawn at {drawn}");
    // The Custodian takes it.
    app.world_mut().despawn(carcass);
    for _ in 0..5 {
        app.update();
    }
    let h = app.world().get::<Herd>(herd_e).unwrap();
    assert!(h.beasts[index].gone, "its body is still there");
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

/// The diversion gauge (`docs/WILDLIFE.md`, "Using them"): in each valley,
/// at up to four of its low roofs, a walker on watch 65 m off with its back
/// to the birds, and a 40 m crossing 35 m in front of it, in its sight.
/// Walk it with a stone onto the birds' roof first (4 s before), and
/// without: how often the walker gets onto you. Prints the table.
///
///     cargo test --profile fast --lib wildlife_diversion_gauge -- --ignored --nocapture
#[test]
#[ignore = "a gauge: prints the table"]
fn wildlife_diversion_gauge() {
    println!("| Valley | Crossings | Seen, plain | Seen, a stone onto the birds first |");
    println!("| - | - | - | - |");
    for map in extraction_core::map::VALLEYS.map(|v| v.0) {
        let (mut app, _, zone) = app_on(Some(map));
        let perches = app.world().resource::<Perches>().clone();
        let nav = app
            .world()
            .resource::<crate::robots::walker::Nav>()
            .0
            .clone();
        let ways = crossings(&mut app, &zone, &perches, &nav);
        drop(app);
        let mut seen = [0, 0];
        for &(i, post, from, to) in &ways {
            for (arm, stone) in [(0, false), (1, true)] {
                if crossing_seen(map, i, post, from, to, stone) {
                    seen[arm] += 1;
                }
            }
        }
        println!("| {map} | {} | {} | {} |", ways.len(), seen[0], seen[1]);
    }
}

/// Up to four set-ups in a valley: (site, walker's post, crossing from, to).
fn crossings(
    app: &mut App,
    zone: &Zone,
    perches: &Perches,
    nav: &extraction_core::nav::NavGrid,
) -> Vec<(usize, Vec3, Vec3, Vec3)> {
    let mut q = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
    let spatial = q.get(app.world()).unwrap();
    let world = SpatialQueryFilter::from_mask(crate::world::Layer::World);
    let sees = |a: Vec3, b: Vec3| {
        Dir3::new(b - a).is_ok_and(|d| {
            spatial
                .cast_ray(a, d, a.distance(b) - 0.3, true, &world)
                .is_none()
        })
    };
    let walk = |p: Vec3| nav.walkable([p.x, p.z]);
    let mut out = Vec::new();
    for (i, s) in perches.sites.iter().enumerate() {
        if out.len() >= 4 {
            break;
        }
        if s.spots.len() < 8 || s.top() > zone.ground(s.centre.x, s.centre.z) + 8.0 {
            continue;
        }
        let up = s.centre.with_y(s.top()) + Vec3::Y * super::flock::ALARM_HEIGHT;
        let found = (0..16).find_map(|k| {
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            let dir = Vec3::new(a.cos(), 0.0, a.sin());
            let side = Vec3::new(-dir.z, 0.0, dir.x);
            let post = zone.on_ground(s.centre.x + dir.x * 65.0, s.centre.z + dir.z * 65.0);
            let mid = post + dir * 35.0;
            let mid = zone.on_ground(mid.x, mid.z);
            let ends = [mid - side * 20.0, mid + side * 20.0].map(|p| zone.on_ground(p.x, p.z));
            let eye = post + Vec3::Y * 1.5;
            let ok = walk(post)
                && walk(mid)
                && ends.iter().all(|&p| walk(p))
                && !crate::acoustics::blocked(&spatial, up, eye)
                && ends
                    .iter()
                    .chain([&mid])
                    .all(|&p| sees(eye, p + Vec3::Y * 1.6));
            ok.then_some((i, post, ends[0], ends[1]))
        });
        out.extend(found);
    }
    out
}

/// One crossing, from its own fresh valley: did the walker get onto you?
fn crossing_seen(map: &str, site: usize, post: Vec3, from: Vec3, to: Vec3, stone: bool) -> bool {
    let (mut app, player, zone) = app_on(Some(map));
    // Out of the way while it takes its post.
    put(&mut app, player, zone.on_ground(post.x + 250.0, post.z));
    let perches = app.world().resource::<Perches>().clone();
    let centre = perches.sites[site].centre;
    let walker = walker_at(&mut app, &zone, post, centre);
    let old: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, Or<(With<Flock>, With<super::ducks::Raft>)>>()
        .iter(app.world())
        .collect();
    for e in old {
        app.world_mut().despawn(e);
    }
    let mut rng = extraction_core::noise::Rng::new(5);
    let flock = new_flock(Species::Rook, site, &perches.sites[site], &mut rng);
    app.world_mut()
        .spawn((Transform::from_translation(centre), flock));
    if stone {
        let s = app.world_mut().spawn_empty().id();
        let now = app.world().resource::<Time>().elapsed_secs();
        app.world_mut()
            .resource_mut::<super::LetGo>()
            .0
            .insert(s, now);
        let at = centre.with_y(perches.sites[site].top());
        for _ in 0..3 {
            app.world_mut()
                .write_message(crate::items::impacts::ItemImpact {
                    item: s,
                    at,
                    speed: 6.0,
                    mass: 0.3,
                    material: crate::items::impacts::Material::Wood,
                    ground: None,
                });
            app.update();
        }
    } else {
        for _ in 0..3 {
            app.update();
        }
    }
    for _ in 0..90 * 4 {
        app.update();
    }
    // Across, at a walk.
    put(&mut app, player, from);
    let along = (to - from).with_y(0.0).normalize();
    let mut onto = false;
    for _ in 0..90 * 32 {
        {
            let mut input = app.world_mut().get_mut::<RigInput>(player).unwrap();
            input.head.rotation = Quat::from_rotation_arc(Vec3::NEG_Z, along);
            input.locomotion = Vec2::Y;
        }
        app.update();
        let aware = app.world().get::<crate::robots::Awareness>(walker).unwrap();
        let brain = app.world().get::<Brain>(walker).unwrap();
        onto |= aware.target == Some(player)
            && (aware.suspicion > 0.5 || matches!(brain, Brain::Engage));
        let at = app.world().get::<Transform>(player).unwrap().translation;
        if at.xz().distance(to.xz()) < 1.5 {
            break;
        }
    }
    onto
}
