//! The robots' minds, played out: hearing a distraction, losing you behind
//! a building and searching for you, and not seeing you in the grass.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use super::search::{Searching, plan_search};
use super::{Awareness, Brain, Job, JobKind};
use crate::rig::{LocalPlayer, RigInput};
use crate::world::Zone;

#[derive(Resource, Default)]
struct Heard(Vec<Searching>);

fn listen(mut s: MessageReader<Searching>, mut heard: ResMut<Heard>) {
    heard.0.extend(s.read().copied());
}

fn app(weather: Option<&str>) -> (App, Entity, Zone) {
    let opts = crate::LaunchOptions {
        robots: false,
        weather: weather.map(Into::into),
        ..Default::default()
    };
    let mut app = crate::headless_app(opts, false);
    app.init_resource::<Heard>().add_systems(Last, listen);
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
    // Nothing drives the rig here: stand the player up (the head defaults
    // to the floor, and the body follows it down).
    app.world_mut()
        .get_mut::<RigInput>(player)
        .unwrap()
        .head
        .translation
        .y = 1.65;
    let zone = app.world().resource::<Zone>().clone();
    (app, player, zone)
}

/// An unarmed walker (it watches; it doesn't shoot the test) standing at
/// its post, facing -Z.
fn walker_at(app: &mut App, zone: &Zone, post: Vec3) -> Entity {
    let mut queue = CommandQueue::default();
    let w = {
        let mut c = Commands::new(&mut queue, app.world());
        super::walker::spawn_walker_with(
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
    w
}

fn put(app: &mut App, player: Entity, at: Vec3) {
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = at + Vec3::Y * 0.05;
}

fn flat(v: Vec3) -> f32 {
    Vec2::new(v.x, v.z).length()
}

/// The classic distraction: a piece of scrap thrown down the street
/// clatters 14 m from a walker at its post, and the walker walks over to
/// where it landed (not to you) and searches round it.
#[test]
fn a_thrown_bolt_draws_a_walker() {
    let (mut app, player, zone) = app(None);
    let post = zone.on_ground(100.0, 200.0);
    // Well out of its sight.
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let walker = walker_at(&mut app, &zone, post);
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let spot = zone.on_ground(post.x + 14.0, post.z + 3.0);
    let mut queue = CommandQueue::default();
    let bolt = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind("scrap_metal").unwrap(),
                transform: Transform::from_translation(spot + Vec3::Y * 2.0),
                condition: 1.0,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    app.world_mut()
        .get_mut::<avian3d::prelude::LinearVelocity>(bolt)
        .unwrap()
        .0 = Vec3::new(0.0, -8.0, 0.0);
    let mut closest = f32::MAX;
    let mut secs = 0.0;
    for i in 0..90 * 25 {
        app.update();
        let at = app.world().get::<Transform>(walker).unwrap().translation;
        closest = closest.min(flat(at - spot));
        if closest < 3.5 {
            secs = i as f32 / 90.0;
            break;
        }
    }
    assert!(
        closest < 3.5,
        "walked to where it landed (closest {closest:.1} m)"
    );
    assert!(secs < 15.0, "in good time ({secs:.1} s)");
    for _ in 0..90 * 2 {
        app.update();
    }
    assert!(
        matches!(
            app.world().get::<Brain>(walker).unwrap(),
            Brain::Search { .. }
        ),
        "and looked round: {:?}",
        app.world().get::<Brain>(walker).unwrap()
    );
}

/// You're seen beside a building, then walk round behind it and slip
/// away. The walker can't see through the building: it goes to where it
/// lost you and searches there.
#[test]
fn a_walker_loses_you_behind_a_building_and_searches_where_you_were() {
    let (mut app, player, zone) = app(None);
    let shells = app
        .world()
        .resource::<super::doors::Entrances>()
        .shells
        .clone();
    let nav = app.world().resource::<super::walker::Nav>().0.clone();
    let walkable = |p: Vec3| nav.walkable([p.x, p.z]);
    // A building with open, walkable ground on its +x side and round its
    // +z face to the back.
    let pick = shells.iter().find_map(|s| {
        if s.half.x < 3.0 || s.half.y < 4.0 {
            return None;
        }
        let w = |x: f32, z: f32| {
            let p = s.xf.transform_point(Vec3::new(x, 0.0, z));
            zone.on_ground(p.x, p.z)
        };
        let (hx, hz) = (s.half.x, s.half.y);
        let post = w(hx + 24.0, 0.0);
        let seen = w(hx + 3.0, hz + 3.0);
        let corner = w(-hx - 3.5, hz + 3.5);
        let hide = w(-hx - 3.0, -hz + 1.0);
        let route = [seen, corner, hide];
        let ok = [post, seen, corner, hide].iter().all(|p| walkable(*p))
            && (post.y - seen.y).abs() < 3.0;
        ok.then_some((post, route))
    });
    let Some((post, route)) = pick else {
        panic!("no building with room round it on this map");
    };
    put(&mut app, player, route[0]);
    let walker = walker_at(&mut app, &zone, post);
    // Face the player.
    {
        let to = route[0] - post;
        let mut w = app
            .world_mut()
            .get_mut::<super::walker::Walker>(walker)
            .unwrap();
        w.yaw = (-to.x).atan2(-to.z);
    }
    let mut engaged = false;
    for _ in 0..90 * 4 {
        app.update();
        engaged |= matches!(app.world().get::<Brain>(walker).unwrap(), Brain::Engage);
        if engaged {
            break;
        }
    }
    assert!(engaged, "it saw you by the building");
    // Walk round the corner and behind, at a walk.
    let mut at = route[0];
    let mut lost_at = None;
    for leg in [route[1], route[2]] {
        while flat(leg - at) > 0.1 {
            let step = (leg - at).normalize() * (1.4 / 90.0);
            at += step.clamp_length_max(flat(leg - at));
            let p = zone.on_ground(at.x, at.z);
            put(&mut app, player, p);
            app.update();
            let aware = app.world().get::<Awareness>(walker).unwrap();
            if aware.can_see_target {
                lost_at = None;
            } else if lost_at.is_none() {
                lost_at = Some(p);
            }
        }
    }
    let lost_at = lost_at.expect("out of its sight behind the building");
    let hide = route[2];
    // A moment behind the building (long enough that it's guessing), then
    // slip away out of everyone's sight.
    for _ in 0..(90.0 * 1.5) as usize {
        app.update();
    }
    assert!(
        !app.world().get::<Awareness>(walker).unwrap().can_see_target,
        "it can't see through the building"
    );
    put(&mut app, player, zone.on_ground(hide.x, hide.z + 200.0));
    let mut searched = None;
    for _ in 0..90 * 45 {
        app.update();
        if let Some(s) = app.world().resource::<Heard>().0.first() {
            if s.robot == walker {
                searched = Some(app.world().get::<Brain>(walker).unwrap().clone());
                break;
            }
        }
    }
    let Some(Brain::Search { at, .. }) = searched else {
        panic!("it searched: {searched:?}");
    };
    assert!(
        flat(at - lost_at) < 6.0,
        "where it lost you ({at:.1}, lost at {lost_at:.1}, hid at {hide:.1})"
    );
    let me = app.world().get::<Transform>(walker).unwrap().translation;
    assert!(flat(me - at) < 4.0, "it went there ({me:.1})");
}

/// Crouched still in the grass, 30 m in front of a walker, in a
/// pea-souper: it doesn't see you. Stand up and it does.
#[test]
fn crouched_in_grass_in_fog_at_30m_you_are_not_seen() {
    let (mut app, player, zone) = app(Some("fog"));
    assert_eq!(
        app.world()
            .resource::<crate::world::weather::Conditions>()
            .kind,
        crate::world::weather::WeatherKind::PeaSouper
    );
    // Open grass, a walker 30 m north (-Z of you, it faces -Z... so it
    // stands south of you and looks north at you).
    let nav = app.world().resource::<super::walker::Nav>().0.clone();
    let spot = (0..400)
        .map(|i| {
            let a = i as f32 * 2.399;
            let r = 20.0 + i as f32 * 0.6;
            zone.on_ground(a.cos() * r, a.sin() * r)
        })
        .find(|p| {
            let post = zone.on_ground(p.x, p.z + 30.0);
            let grass = |q: Vec3| {
                zone.hm
                    .surface([q.x, q.z], zone.forest_cover(q.x, q.z) > 0.5)
                    == extraction_core::terrain::Surface::Grass
            };
            grass(*p)
                && nav.walkable([p.x, p.z])
                && nav.walkable([post.x, post.z])
                && (post.y - p.y).abs() < 1.5
                && nav.line_clear([p.x, p.z], [post.x, post.z])
        })
        .expect("open grass somewhere");
    put(&mut app, player, spot);
    let head = |app: &mut App, y: f32| {
        app.world_mut()
            .get_mut::<RigInput>(player)
            .unwrap()
            .head
            .translation
            .y = y;
    };
    head(&mut app, 0.85);
    let walker = walker_at(&mut app, &zone, zone.on_ground(spot.x, spot.z + 30.0));
    for _ in 0..90 * 8 {
        head(&mut app, 0.85);
        app.update();
        let aware = app.world().get::<Awareness>(walker).unwrap();
        assert!(
            !aware.can_see_target && aware.suspicion < 0.2,
            "crouched in the grass: {aware:?}, body {:?}",
            app.world()
                .get::<crate::player::Body>(player)
                .unwrap()
                .height
        );
    }
    let mut seen = false;
    for _ in 0..90 * 6 {
        head(&mut app, 1.65);
        app.update();
        seen |= app.world().get::<Awareness>(walker).unwrap().can_see_target;
    }
    assert!(seen, "stood up, it sees you");
}

/// A search near a building takes in its doorway and the ground by its
/// walls, not just open field.
#[test]
fn a_search_takes_in_doorways_and_cover() {
    let (app, _, _) = app(None);
    let e = app.world().resource::<super::doors::Entrances>();
    let nav = app.world().resource::<super::walker::Nav>().0.clone();
    let door = e.doors.first().expect("a door");
    let at = door.mid + door.out * 6.0;
    let spots = plan_search(
        at,
        10.0,
        at + Vec3::X * 20.0,
        super::indoor::NavRef::Zone(&nav),
        e,
    );
    assert!(spots.len() >= 2, "{spots:?}");
    assert!(
        spots
            .iter()
            .any(|s| flat(*s - (door.mid + door.out * 1.8)) < 0.5),
        "the doorway: {spots:?}"
    );
}

/// A robot close by calling the others about you puts a line on the
/// wrist, once: a second call straight after says nothing.
#[test]
fn a_call_about_you_close_by_is_said_once() {
    let (mut app, player, _) = app(None);
    app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
    let me = app
        .world()
        .get::<GlobalTransform>(player)
        .unwrap()
        .translation();
    let lines = |app: &App| app.world().resource::<crate::raid::Raid>().log.len();
    let before = lines(&app);
    for robot in [
        Entity::from_raw_u32(9001).unwrap(),
        Entity::from_raw_u32(9002).unwrap(),
    ] {
        app.world_mut().write_message(super::search::CalledOut {
            robot,
            at: me + Vec3::X * 12.0,
            heard: 2,
            about: Some(player),
        });
        app.update();
    }
    assert_eq!(
        lines(&app) - before,
        1,
        "{:?}",
        app.world().resource::<crate::raid::Raid>().log
    );
}

/// A 30 m square site on open ground at `centre`, its own grid blocked
/// wherever `rock` says (site-local x/z).
fn site(centre: Vec3, rock: impl Fn(Vec2) -> bool) -> super::indoor::LocalNav {
    let (n, cell, origin) = (60, 0.5, -15.0);
    let mut cost = vec![1u8; n * n];
    for j in 0..n {
        for i in 0..n {
            let p = Vec2::new(
                origin + (i as f32 + 0.5) * cell,
                origin + (j as f32 + 0.5) * cell,
            );
            if rock(p) {
                cost[j * n + i] = extraction_core::nav::BLOCKED;
            }
        }
    }
    super::indoor::LocalNav {
        grid: std::sync::Arc::new(extraction_core::nav::NavGrid {
            n,
            cell,
            origin,
            cost,
            lanes: Vec::new(),
        }),
        xf: Transform::from_translation(centre),
        floors: None,
    }
}

fn crawler_at(
    app: &mut App,
    zone: &Zone,
    nav: super::indoor::LocalNav,
    route: Vec<Vec3>,
) -> Entity {
    let at = route.first().copied().unwrap_or(nav.xf.translation);
    let site = super::indoor::IndoorSite {
        nav,
        patrol: route.clone(),
        spawn: at,
        crawlers: 1,
    };
    let mut queue = CommandQueue::default();
    let e = {
        let mut c = Commands::new(&mut queue, app.world());
        super::indoor::spawn_crawler(&mut c, zone, &site, at, route)
    };
    queue.apply(app.world_mut());
    app.update();
    e
}

/// A crawler walks its round on its site's own grid: round the end of a
/// wall that only the site's grid knows about, there and back.
#[test]
fn a_crawler_patrols_on_its_sites_grid() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let centre = zone.on_ground(100.0, 200.0);
    // A wall across the middle, x -11..11; the way round is past its ends.
    // The grid keeps a metre off it, as a site's plan keeps a body's width
    // off the rock.
    let wall = |p: Vec2| p.x.abs() < 11.0 && p.y.abs() < 1.0;
    let nav = site(centre, |p| p.x.abs() < 12.0 && p.y.abs() < 2.0);
    let a = zone.on_ground(centre.x - 4.0, centre.z - 6.0);
    let b = zone.on_ground(centre.x - 4.0, centre.z + 6.0);
    let crawler = crawler_at(&mut app, &zone, nav, vec![a, b]);
    let (mut reached_b, mut back_at_a, mut crossed) = (false, false, false);
    for _ in 0..90 * 90 {
        app.update();
        let p = app.world().get::<Transform>(crawler).unwrap().translation - centre;
        crossed |= wall(Vec2::new(p.x, p.z));
        if !reached_b {
            reached_b = Vec2::new(p.x + 4.0, p.z - 6.0).length() < 1.5;
        } else if Vec2::new(p.x + 4.0, p.z + 6.0).length() < 1.5 {
            back_at_a = true;
            break;
        }
    }
    assert!(
        reached_b && back_at_a,
        "round and back ({reached_b}, {back_at_a})"
    );
    assert!(!crossed, "never through the wall");
}

/// In the dark, standing 12 m in front of a crawler, you're nothing; a
/// torch on you, and it has you.
#[test]
fn in_the_dark_a_torch_gives_you_away() {
    let (mut app, player, zone) = app(None);
    let centre = zone.on_ground(100.0, 200.0);
    app.world_mut()
        .resource_mut::<super::senses::Lighting>()
        .dark
        .push((
            Transform::from_translation(centre),
            Vec3::new(15.0, 4.0, 15.0),
        ));
    let me = zone.on_ground(centre.x, centre.z - 12.0);
    put(&mut app, player, me);
    let crawler = crawler_at(&mut app, &zone, site(centre, |_| false), vec![]);
    for _ in 0..90 * 6 {
        app.update();
        let aware = app.world().get::<Awareness>(crawler).unwrap();
        assert!(
            !aware.can_see_target && aware.suspicion < 0.2,
            "in the dark: {aware:?}"
        );
    }
    // A lit torch at the head, pointed at the crawler.
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let mut queue = CommandQueue::default();
    let (torch, cell) = {
        let mut c = Commands::new(&mut queue, app.world());
        let mut spawn = |id: &str| {
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind(id).unwrap(),
                    transform: Transform::from_translation(me + Vec3::Y * 1.5),
                    condition: 1.0,
                },
            )
        };
        (spawn("flashlight"), spawn("power_cell"))
    };
    queue.apply(app.world_mut());
    app.update();
    let socket = app
        .world_mut()
        .query::<(Entity, &crate::interact::SocketOwner)>()
        .iter(app.world())
        .find(|(_, o)| o.0 == torch)
        .map(|(e, _)| e)
        .expect("the torch's cell socket");
    {
        let mut q =
            bevy::ecs::system::SystemState::<(Commands, Query<&mut crate::interact::Socket>)>::new(
                app.world_mut(),
            );
        let (mut c, mut sockets) = q.get_mut(app.world_mut()).unwrap();
        let mut s = sockets.get_mut(socket).unwrap();
        crate::interact::seat_in_socket(&mut c, cell, socket, &mut s);
        q.apply(app.world_mut());
    }
    app.world_mut()
        .entity_mut(torch)
        .insert(avian3d::prelude::RigidBody::Kinematic);
    app.world_mut().write_message(crate::items::usage::UseItem {
        player: Entity::PLACEHOLDER,
        item: torch,
    });
    let aim = Transform::from_translation(me + Vec3::Y * 1.5).looking_at(centre + Vec3::Y, Vec3::Y);
    let mut seen = false;
    for _ in 0..90 * 5 {
        *app.world_mut().get_mut::<Transform>(torch).unwrap() = aim;
        app.update();
        seen |= app
            .world()
            .get::<Awareness>(crawler)
            .unwrap()
            .can_see_target;
    }
    assert!(
        app.world().get::<crate::gadgets::Powered>(torch).is_some(),
        "the torch is on"
    );
    assert!(seen, "the torch gave you away");
}

/// Underground, sound goes round corners: a crawler hears a noise behind
/// a pillar by the walk round it; a walker on the open grid, the same
/// distance off, hears it through the pillar as faint as through a wall.
#[test]
fn a_crawler_hears_round_a_corner() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let centre = zone.on_ground(100.0, 200.0);
    let pillar = |p: Vec2| p.x.abs() < 2.0 && p.y.abs() < 2.0;
    app.world_mut().spawn((
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(4.0, 6.0, 4.0),
        avian3d::prelude::CollisionLayers::new(
            crate::world::Layer::World,
            avian3d::prelude::LayerMask::ALL,
        ),
        Transform::from_translation(centre + Vec3::Y * 2.0),
    ));
    let at = zone.on_ground(centre.x + 3.2, centre.z);
    let crawler = crawler_at(&mut app, &zone, site(centre, pillar), vec![at]);
    let walker = walker_at(&mut app, &zone, zone.on_ground(centre.x, centre.z + 3.2));
    // The walker's twin of the noise: as far off, behind the same pillar.
    for _ in 0..10 {
        app.update();
    }
    let noise = |at: Vec3| super::Noise {
        at,
        radius: 7.0,
        source: None,
    };
    let ear = app.world().get::<Transform>(walker).unwrap().translation;
    app.world_mut()
        .write_message(noise(zone.on_ground(centre.x - 3.2, centre.z)));
    app.world_mut().write_message(noise(
        zone.on_ground(centre.x, centre.z - 3.2 - (ear.z - centre.z - 3.2)),
    ));
    app.update();
    app.update();
    let heard = |e: Entity, app: &App| app.world().get::<Awareness>(e).unwrap().suspicion;
    assert!(
        heard(crawler, &app) > 0.1,
        "round the corner: {}",
        heard(crawler, &app)
    );
    assert!(
        heard(walker, &app) < 0.05,
        "through the pillar: {}",
        heard(walker, &app)
    );
}

/// A walker with a leg shot off doesn't give chase: told where you were,
/// it stays put and looks; a sound one goes there.
#[test]
fn a_lame_walker_does_not_chase() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let sound = walker_at(&mut app, &zone, zone.on_ground(100.0, 200.0));
    let lame = walker_at(&mut app, &zone, zone.on_ground(130.0, 200.0));
    let leg = app
        .world_mut()
        .query::<(Entity, &super::RobotPart)>()
        .iter(app.world())
        .find(|(_, p)| p.robot == lame && p.role == super::PartRole::Leg(0))
        .map(|(e, _)| e)
        .unwrap();
    app.world_mut().get_mut::<super::RobotPart>(leg).unwrap().hp = 0.0;
    app.world_mut().write_message(super::PartDestroyed {
        robot: lame,
        part: leg,
        role: super::PartRole::Leg(0),
        at: Vec3::ZERO,
    });
    for _ in 0..30 {
        app.update();
    }
    assert!(
        app.world().get::<super::anim::Gait>(lame).unwrap().limp > super::walker::LAME,
        "three legs is lame"
    );
    let start = |e: Entity, app: &App| app.world().get::<Transform>(e).unwrap().translation;
    let (s0, l0) = (start(sound, &app), start(lame, &app));
    for _ in 0..90 * 3 {
        for (e, at) in [(sound, s0), (lame, l0)] {
            *app.world_mut().get_mut::<Brain>(e).unwrap() = Brain::Engage;
            let mut a = app.world_mut().get_mut::<Awareness>(e).unwrap();
            a.target = Some(player);
            a.last_known = Some(zone.on_ground(at.x, at.z - 25.0));
            a.suspicion = 1.0;
            a.unseen = 1.0;
        }
        app.update();
    }
    let moved = |e: Entity, from: Vec3, app: &App| flat(start(e, app) - from);
    assert!(
        moved(sound, s0, &app) > 2.0,
        "the sound one chased: {}",
        moved(sound, s0, &app)
    );
    assert!(
        moved(lame, l0, &app) < 0.8,
        "the lame one held: {}",
        moved(lame, l0, &app)
    );
}
