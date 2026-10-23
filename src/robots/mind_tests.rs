//! The robots' minds, played out: hearing a distraction, losing you behind
//! a building and searching for you, and not seeing you in the grass.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use super::search::{Searching, plan_search};
use super::{Awareness, Brain, Job, JobKind, Robot};
use crate::rig::{LocalPlayer, RigInput};
use crate::world::Zone;

#[derive(Resource, Default)]
struct Heard(Vec<Searching>);

fn listen(mut s: MessageReader<Searching>, mut heard: ResMut<Heard>) {
    heard.0.extend(s.read().copied());
}

fn app(weather: Option<&str>) -> (App, Entity, Zone) {
    app_with(crate::LaunchOptions {
        robots: false,
        weather: weather.map(Into::into),
        ..Default::default()
    })
}

fn app_with(opts: crate::LaunchOptions) -> (App, Entity, Zone) {
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

/// The classic distraction: a piece of scrap lobbed down hard 7 m from a
/// walker at its post (on grass, which muffles a landing to half: dropped
/// softly, or from 14 m, it wouldn't be heard), and the walker walks over
/// to about where it landed (not to you) and searches round it.
#[test]
fn a_thrown_bolt_draws_a_walker() {
    let (mut app, player, zone) = app(None);
    let post = zone.on_ground(100.0, 200.0);
    // Well out of its sight.
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let walker = walker_at(&mut app, &zone, post);
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let spot = zone.on_ground(post.x + 7.0, post.z + 3.0);
    let mut queue = CommandQueue::default();
    let bolt = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind("scrap_metal").unwrap(),
                transform: Transform::from_translation(spot + Vec3::Y * 6.0),
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
    // It heard roughly where: it comes to look within its guess's spread.
    let near = 3.5
        + (flat(spot - post) * super::senses::HEARD_SPREAD).max(super::senses::HEARD_SPREAD_MIN);
    let mut closest = f32::MAX;
    let mut secs = 0.0;
    for i in 0..90 * 25 {
        app.update();
        let at = app.world().get::<Transform>(walker).unwrap().translation;
        closest = closest.min(flat(at - spot));
        if closest < near {
            secs = i as f32 / 90.0;
            break;
        }
    }
    assert!(
        closest < near,
        "walked to where it landed (closest {closest:.1} m)"
    );
    assert!(secs < 15.0, "in good time ({secs:.1} s)");
    // Once at its guess, it looks round.
    for _ in 0..90 * 10 {
        app.update();
        if matches!(
            app.world().get::<Brain>(walker).unwrap(),
            Brain::Search { .. }
        ) {
            break;
        }
    }
    assert!(
        matches!(
            app.world().get::<Brain>(walker).unwrap(),
            Brain::Search { .. }
        ),
        "and looked round: {:?}",
        app.world().get::<Brain>(walker).unwrap()
    );
    // And sprayed its mark where it stood to look.
    app.update();
    app.update();
    let at = app.world().get::<Transform>(walker).unwrap().translation;
    assert!(
        marks(&mut app).iter().any(|m| flat(m.at - at) < 4.0),
        "a mark where it searched: {:?}",
        marks(&mut app)
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
    // A door with open ground before it (which comes first in the list
    // depends on the valley's buildings).
    let walkable = |p: Vec3| nav.walkable([p.x, p.z]);
    let door = e
        .doors
        .iter()
        .find(|d| {
            (1..=8).all(|k| walkable(d.mid + d.out * (k as f32 + 1.0)))
                && walkable(d.mid + d.out * 6.0 + Vec3::X * 10.0)
        })
        .expect("a door with open ground before it");
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
        wardens: 0,
        dormant: false,
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
    let torch = lit_torch(&mut app, me);
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

/// A drone searching comes down to a doorway on its round and hangs
/// outside it, low, looking in.
#[test]
fn a_searching_drone_looks_in_at_a_doorway() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let door = *app
        .world()
        .resource::<super::doors::Entrances>()
        .doors
        .first()
        .expect("a door");
    let start = door.mid + door.out * 25.0 + Vec3::Y * 10.0;
    let mut queue = CommandQueue::default();
    let drone = {
        let mut c = Commands::new(&mut queue, app.world());
        super::drone::spawn_drone(
            &mut c,
            start,
            Job {
                kind: JobKind::Survey,
                site: start,
                duration: 999.0,
                exit: start,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    let stand = door.mid + door.out * 1.8;
    let mut best = f32::MAX;
    for _ in 0..90 * 20 {
        if let Brain::Search { .. } = app.world().get::<Brain>(drone).unwrap() {
        } else {
            *app.world_mut().get_mut::<Brain>(drone).unwrap() = Brain::Search {
                at: door.mid + door.out * 6.0,
                left: 60.0,
                spots: vec![stand],
                dwell: 0.0,
            };
        }
        app.update();
        let p = app.world().get::<Transform>(drone).unwrap().translation;
        let hover = door.mid + door.out * super::drone::DOORWAY_STANDOFF;
        let low = p.y - zone.ground(p.x, p.z) < 4.0;
        if low {
            best = best.min(flat(p - hover));
        }
    }
    assert!(best < 2.5, "hung low outside the doorway ({best:.1} m off)");
}

/// A crawler's wreck doesn't wall off a corridor: once it's wrecked,
/// players walk past it (the sims found bots stuck behind one).
#[test]
fn a_wrecked_crawler_does_not_block_the_way() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let centre = zone.on_ground(100.0, 200.0);
    let crawler = crawler_at(&mut app, &zone, site(centre, |_| false), vec![]);
    let blocks = |app: &mut App| {
        let parts: Vec<Entity> = app
            .world()
            .get::<Children>(crawler)
            .unwrap()
            .iter()
            .collect();
        parts.iter().any(|p| {
            app.world()
                .get::<avian3d::prelude::CollisionLayers>(*p)
                .is_some_and(|l| l.filters.has_all(crate::world::Layer::Player))
        })
    };
    assert!(blocks(&mut app), "a live crawler is solid");
    app.world_mut()
        .get_mut::<super::Robot>(crawler)
        .unwrap()
        .wrecked = true;
    for _ in 0..5 {
        app.update();
    }
    assert!(!blocks(&mut app), "its wreck isn't");
}

fn hound_at(app: &mut App, zone: &Zone, at: Vec3, site: Vec3, kind: JobKind) -> Entity {
    let mut queue = CommandQueue::default();
    let h = {
        let mut c = Commands::new(&mut queue, app.world());
        super::hound::spawn_hound(
            &mut c,
            zone,
            at,
            Job {
                kind,
                site,
                duration: 999.0,
                exit: at,
            },
        )
    };
    queue.apply(app.world_mut());
    for _ in 0..30 {
        app.update();
    }
    h
}

#[derive(Resource, Default)]
struct HoundLog {
    crouch: Option<f32>,
    bite: Option<f32>,
}

fn hound_log(
    mut c: MessageReader<super::hound::HoundCrouch>,
    mut b: MessageReader<super::hound_mind::HoundBite>,
    time: Res<Time>,
    mut log: ResMut<HoundLog>,
) {
    let t = time.elapsed_secs();
    if c.read().next().is_some() && log.crouch.is_none() {
        log.crouch = Some(t);
    }
    if b.read().next().is_some() && log.bite.is_none() {
        log.bite = Some(t);
    }
}

/// A hound that sees you comes at you and bites, but crouches first: the
/// tell comes a beat before the bite.
#[test]
fn a_hound_crouches_before_it_bites() {
    let (mut app, player, zone) = app(None);
    app.init_resource::<HoundLog>().add_systems(Last, hound_log);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let at = zone.on_ground(100.0, 215.0);
    let hound = hound_at(&mut app, &zone, at, at, JobKind::Watch);
    *app.world_mut().get_mut::<Brain>(hound).unwrap() = Brain::Working { at, left: 999.0 };
    for _ in 0..90 * 20 {
        app.update();
        if app.world().resource::<HoundLog>().bite.is_some() {
            break;
        }
    }
    let log = app.world().resource::<HoundLog>();
    let crouch = log.crouch.expect("it crouched to lunge");
    let bite = log.bite.expect("then bit");
    assert!(
        bite - crouch >= super::hound_mind::CROUCH_TIME - 0.05,
        "a beat to react: crouched {crouch:.2}, bit {bite:.2}"
    );
}

/// A hound hunts by ear: a shot 60 m off and it's there in a few seconds,
/// at a run (a walker takes half a minute).
#[test]
fn a_hound_runs_to_a_shot() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let at = zone.on_ground(100.0, 200.0);
    let hound = hound_at(&mut app, &zone, at, at, JobKind::Watch);
    *app.world_mut().get_mut::<Brain>(hound).unwrap() = Brain::Working { at, left: 999.0 };
    let shot = zone.on_ground(140.0, 245.0);
    app.world_mut().write_message(super::Noise {
        at: shot,
        radius: 220.0,
        source: None,
    });
    let mut secs = None;
    for i in 0..90 * 30 {
        app.update();
        let p = app.world().get::<Transform>(hound).unwrap().translation;
        if flat(p - shot) < super::hound_mind::STALK_WITHIN + 2.0 {
            secs = Some(i as f32 / 90.0);
            break;
        }
    }
    let secs = secs.expect("it went to the shot");
    assert!(secs < 25.0, "at a run: {secs:.1} s for ~45 m");
}

/// Keep still and a hound nosing round 8 m off doesn't make you out;
/// move, and it does.
#[test]
fn keeping_still_beats_a_hound() {
    let (mut app, player, zone) = app(None);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let at = zone.on_ground(100.0, 208.0);
    let hound = hound_at(&mut app, &zone, at, at, JobKind::Watch);
    // Facing you, standing still.
    *app.world_mut().get_mut::<Brain>(hound).unwrap() = Brain::Working { at, left: 999.0 };
    {
        let mut w = app
            .world_mut()
            .get_mut::<super::walker::Walker>(hound)
            .unwrap();
        w.yaw = 0.0;
    }
    app.world_mut()
        .get_mut::<RigInput>(player)
        .unwrap()
        .head
        .translation
        .y = 0.85;
    for _ in 0..90 * 6 {
        app.update();
        let a = app.world().get::<Awareness>(hound).unwrap();
        assert!(
            !a.can_see_target && a.exposure < super::senses::GLIMPSE,
            "still: {a:?}"
        );
    }
    // Now walk: (move the body each tick, as feet would).
    let mut seen = false;
    for i in 0..90 * 4 {
        let step = Vec3::X * (i as f32 * 1.4 / 90.0).sin() * 0.02;
        let mut t = app.world_mut().get_mut::<Transform>(player).unwrap();
        t.translation += step;
        app.world_mut()
            .get_mut::<crate::player::Body>(player)
            .unwrap()
            .velocity = Vec3::X * 1.4;
        app.update();
        seen |= app.world().get::<Awareness>(hound).unwrap().exposure >= super::senses::GLIMPSE;
    }
    assert!(seen, "moving, it makes you out");
}

/// A suppressed AK is heard a third as far: a robot 150 m off hears a bare
/// one and not a suppressed one.
#[test]
fn a_suppressor_keeps_a_shot_from_carrying() {
    #[derive(Resource, Default)]
    struct Heard(Vec<f32>);
    let (mut app, player, _) = app(None);
    app.init_resource::<Heard>().add_systems(
        Last,
        |mut n: MessageReader<super::Noise>, mut h: ResMut<Heard>| {
            h.0.extend(n.read().filter(|n| n.radius > 50.0).map(|n| n.radius))
        },
    );
    let bare = app.world_mut().spawn_empty().id();
    let quiet = app.world_mut().spawn(crate::gadgets::Suppressed).id();
    for gun in [bare, quiet] {
        app.world_mut()
            .write_message(crate::weapons::ballistics::Shot {
                gun,
                shooter: Some(player),
                origin: Vec3::new(0.0, 500.0, 0.0),
                direction: Vec3::Y,
                speed: 900.0,
                damage: 48.0,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: 1,
            });
        app.update();
        app.update();
    }
    let heard = &app.world().resource::<Heard>().0;
    assert_eq!(heard.len(), 2, "{heard:?}");
    let reach = |r: f32| r * 1.2; // a walker's ears
    assert!(reach(heard[0]) > 150.0, "bare: {}", heard[0]);
    assert!(reach(heard[1]) < 150.0, "suppressed: {}", heard[1]);
}

/// A flashlight with a cell, switched on, at `me`'s head height (kinematic,
/// for the test to aim).
fn lit_torch(app: &mut App, me: Vec3) -> Entity {
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
    torch
}

/// At night, crouched 20 m in front of a walker, you're nothing even out
/// on a road; a torch on, and it has you.
#[test]
fn at_night_you_are_unseen_until_you_light_a_torch() {
    let (mut app, player, zone) = app_with(crate::LaunchOptions {
        robots: false,
        hour: Some("night".into()),
        ..Default::default()
    });
    assert_eq!(
        app.world()
            .resource::<crate::world::weather::Conditions>()
            .hour,
        crate::world::weather::Hour::Night
    );
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    app.world_mut()
        .get_mut::<RigInput>(player)
        .unwrap()
        .head
        .translation
        .y = 0.85;
    let walker = walker_at(&mut app, &zone, zone.on_ground(100.0, 220.0));
    for _ in 0..90 * 6 {
        app.update();
        let a = app.world().get::<Awareness>(walker).unwrap();
        assert!(!a.can_see_target && a.suspicion < 0.2, "in the dark: {a:?}");
    }
    let torch = lit_torch(&mut app, me);
    let aim = Transform::from_translation(me + Vec3::Y * 1.0)
        .looking_at(zone.on_ground(100.0, 220.0) + Vec3::Y, Vec3::Y);
    let mut seen = false;
    for _ in 0..90 * 5 {
        *app.world_mut().get_mut::<Transform>(torch).unwrap() = aim;
        app.update();
        seen |= app.world().get::<Awareness>(walker).unwrap().can_see_target;
    }
    assert!(seen, "the torch gave you away");
}

/// A sound inside a house carries out of an open door a little dulled, and
/// through a shut one as through a wall (the same rule the player hears by,
/// `acoustics::occlusion`).
#[test]
fn an_open_door_lets_a_sound_out() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    // A village house's front door.
    let ent = *app
        .world()
        .resource::<super::doors::Entrances>()
        .doors
        .iter()
        .find(|e| {
            app.world()
                .get::<crate::world::buildings::Building>(e.building)
                .is_some_and(|b| b.prefab == "house_plank")
        })
        .expect("a house door");
    let post = ent.mid + ent.out * 8.0;
    let walker = walker_at(&mut app, &zone, zone.on_ground(post.x, post.z));
    let inside = ent.mid - ent.out * 2.0;
    let heard = |app: &mut App, open: bool| {
        app.world_mut()
            .get_mut::<crate::world::buildings::Door>(ent.door)
            .unwrap()
            .open = open;
        for _ in 0..90 * 2 {
            app.update();
        }
        app.world_mut()
            .get_mut::<Awareness>(walker)
            .unwrap()
            .suspicion = 0.0;
        app.world_mut().write_message(super::Noise {
            at: inside,
            radius: 15.0,
            source: None,
        });
        app.update();
        app.update();
        app.world().get::<Awareness>(walker).unwrap().suspicion
    };
    let shut = heard(&mut app, false);
    let open = heard(&mut app, true);
    assert!(shut < 0.01, "through the shut door: {shut}");
    assert!(open > 0.1, "out of the open one: {open}");
}

/// A live grenade lands 3 m from a walker at its post: it runs, and it's
/// out past the blast when it goes off.
#[test]
fn a_walker_runs_from_a_live_grenade() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let post = zone.on_ground(100.0, 200.0);
    let walker = walker_at(&mut app, &zone, post);
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let at = zone.on_ground(post.x + 3.0, post.z) + Vec3::Y * 0.1;
    let mut queue = CommandQueue::default();
    let g = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind("rgd5").unwrap(),
                transform: Transform::from_translation(at),
                condition: 1.0,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    let radius = {
        let mut live = app
            .world_mut()
            .get_mut::<crate::weapons::grenades::Live>(g)
            .expect("a grenade");
        live.grenade.fuse = extraction_core::grenade::Fuse::Burning(3.5);
        live.grenade.def.blast_radius
    };
    let mut fled = false;
    let mut last = post;
    while app.world().get_entity(g).is_ok() {
        app.update();
        fled |= app.world().get::<super::flee::Fleeing>(walker).is_some();
        last = app.world().get::<Transform>(walker).unwrap().translation;
    }
    assert!(fled, "it saw the grenade");
    assert!(
        flat(last - at) > radius,
        "out past the blast ({:.1} m, blast {radius})",
        flat(last - at)
    );
}

/// A dormant resident sleeps through a player standing in front of it;
/// a loud noise (a breaker's clank) wakes it.
#[test]
fn a_sleeping_crawler_wakes_to_a_loud_noise() {
    let (mut app, player, zone) = app(None);
    let centre = zone.on_ground(100.0, 200.0);
    put(&mut app, player, zone.on_ground(centre.x, centre.z - 6.0));
    let crawler = crawler_at(&mut app, &zone, site(centre, |_| false), vec![centre]);
    app.world_mut()
        .entity_mut(crawler)
        .insert(super::indoor::Dormant);
    for _ in 0..90 * 4 {
        app.update();
        let a = app.world().get::<Awareness>(crawler).unwrap();
        assert!(!a.can_see_target, "asleep: {a:?}");
    }
    app.world_mut().write_message(super::Noise {
        at: zone.on_ground(centre.x + 8.0, centre.z),
        radius: 30.0,
        source: None,
    });
    let mut awake = false;
    for _ in 0..90 * 4 {
        app.update();
        if app.world().get::<super::indoor::Dormant>(crawler).is_none() {
            awake = true;
            break;
        }
    }
    assert!(awake, "the clank woke it");
}

/// The machines recalled: a resident walks back to its bay and sleeps
/// there; the capstone doesn't take it away.
#[test]
fn recalled_residents_sleep_in_their_bays() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let centre = zone.on_ground(100.0, 200.0);
    let bay = zone.on_ground(centre.x - 8.0, centre.z);
    let far = zone.on_ground(centre.x + 8.0, centre.z);
    let crawler = crawler_at(&mut app, &zone, site(centre, |_| false), vec![bay, far]);
    // Out on its round, away from the bay.
    for _ in 0..90 * 15 {
        app.update();
        let p = app.world().get::<Transform>(crawler).unwrap().translation;
        if flat(p - bay) > 10.0 {
            break;
        }
    }
    app.world_mut().resource_mut::<crate::raid::Raid>().recalled = true;
    let mut asleep = false;
    for _ in 0..90 * 30 {
        app.update();
        assert!(
            app.world().get_entity(crawler).is_ok(),
            "the capstone left it be"
        );
        if app.world().get::<super::indoor::Dormant>(crawler).is_some() {
            asleep = true;
            break;
        }
    }
    assert!(asleep, "back in its bay, asleep");
    let p = app.world().get::<Transform>(crawler).unwrap().translation;
    assert!(flat(p - bay) < 3.5, "in its bay ({p:.1} vs {bay:.1})");
}

/// What a robot would voice: one shift per change, none repeated back to
/// back; a robot that hears a call answers it once.
#[test]
fn minds_shift_once_per_change() {
    use super::search::{Mind, MindShift};
    #[derive(Resource, Default)]
    struct Shifts(Vec<MindShift>);
    let (mut app, player, zone) = app(None);
    app.init_resource::<Shifts>().add_systems(
        Last,
        |mut m: MessageReader<MindShift>, mut s: ResMut<Shifts>| s.0.extend(m.read().copied()),
    );
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    // b first, out of your sight (far, facing away); then a, facing you.
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let b = walker_at(&mut app, &zone, zone.on_ground(130.0, 240.0));
    app.world_mut()
        .get_mut::<super::walker::Walker>(b)
        .unwrap()
        .yaw = std::f32::consts::PI;
    put(&mut app, player, me);
    let a = walker_at(&mut app, &zone, zone.on_ground(100.0, 220.0));
    for _ in 0..90 * 8 {
        app.update();
    }
    let shifts = &app.world().resource::<Shifts>().0;
    let of = |r: Entity| {
        shifts
            .iter()
            .filter(|s| s.robot == r)
            .map(|s| s.state)
            .collect::<Vec<_>>()
    };
    let sa = of(a);
    assert!(sa.contains(&Mind::Noticed), "{sa:?}");
    assert!(sa.contains(&Mind::Engaging), "{sa:?}");
    assert!(sa.contains(&Mind::Calling), "{sa:?}");
    let first = |m: Mind| sa.iter().position(|s| *s == m).unwrap();
    assert!(
        first(Mind::Noticed) < first(Mind::Engaging),
        "noticed first: {sa:?}"
    );
    assert_eq!(
        sa.iter().filter(|s| **s == Mind::Noticed).count(),
        1,
        "once: {sa:?}"
    );
    for r in [a, b] {
        let s = of(r);
        for w in s.windows(2) {
            assert!(
                w[0] != w[1] || matches!(w[0], Mind::Answering { .. } | Mind::Hurt),
                "repeated: {s:?}"
            );
        }
    }
    let answers = of(b)
        .iter()
        .filter(|m| **m == Mind::Answering { caller: a })
        .count();
    assert_eq!(answers, 1, "b answered a's call once: {:?}", of(b));
}

/// A shot heard from 40 m off is a guess: two walkers each put it a few
/// metres off the true spot, inside what they know they don't know, and
/// not in the same place.
#[test]
fn a_heard_shot_is_a_guess_not_a_fix() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 400.0));
    let a = walker_at(&mut app, &zone, zone.on_ground(100.0, 200.0));
    let b = walker_at(&mut app, &zone, zone.on_ground(103.0, 200.0));
    app.update();
    let shot = zone.on_ground(101.0, 240.0);
    app.world_mut().write_message(super::Noise {
        at: shot,
        radius: super::senses::GUNSHOT,
        source: Some(player),
    });
    app.update();
    app.update();
    let guess = |e: Entity| {
        let aware = app.world().get::<Awareness>(e).unwrap();
        (aware.last_known.expect("heard it"), aware.uncertainty)
    };
    let (ga, ua) = guess(a);
    let (gb, _) = guess(b);
    let off = flat(ga - shot);
    assert!(ua > 3.0, "40 m off it's rough: {ua:.1} m");
    assert!(
        off >= ua * super::senses::HEARD_MISS * 0.99 && off <= ua * 1.01,
        "off the spot but within its spread: {off:.1} of {ua:.1} m"
    );
    assert!(flat(ga - gb) > 0.5, "each its own guess: {ga} {gb}");
}

/// A measurement, not a test (run it with `--ignored --nocapture`): how
/// often a walker that comes to look for one shot from 40 m finds a
/// shooter crouched still in grass. About half: 3 of 6 when written.
#[test]
#[ignore]
fn experiment_crouched_shooter_at_40m() {
    let mut found = 0;
    let mut tried = 0;
    for k in 0..6 {
        let (mut app, player, zone) = app(None);
        let nav = app.world().resource::<super::walker::Nav>().0.clone();
        let spot = (0..600)
            .map(|i| {
                let a = i as f32 * 2.399 + k as f32 * 0.7;
                let r = 20.0 + i as f32 * 0.6 + k as f32 * 37.0;
                zone.on_ground(a.cos() * r, a.sin() * r)
            })
            .find(|p| {
                let post = zone.on_ground(p.x, p.z + 40.0);
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
            .expect("open grass");
        put(&mut app, player, spot);
        let head = |app: &mut App| {
            app.world_mut()
                .get_mut::<RigInput>(player)
                .unwrap()
                .head
                .translation
                .y = 0.85;
        };
        let walker = walker_at(&mut app, &zone, zone.on_ground(spot.x, spot.z + 40.0));
        let mut early = false;
        for _ in 0..90 * 4 {
            head(&mut app);
            app.update();
            early |= app.world().get::<Awareness>(walker).unwrap().can_see_target;
        }
        if early {
            eprintln!("spot {k}: seen before the shot");
            continue;
        }
        tried += 1;
        app.world_mut().write_message(super::Noise {
            at: spot + Vec3::Y,
            radius: super::senses::GUNSHOT,
            source: Some(player),
        });
        let mut seen = false;
        let mut glimpsed = false;
        let mut closest = f32::MAX;
        for _ in 0..90 * 45 {
            head(&mut app);
            app.update();
            let at = app.world().get::<Transform>(walker).unwrap().translation;
            closest = closest.min(flat(at - spot));
            let aware = app.world().get::<Awareness>(walker).unwrap();
            if aware.exposure >= super::senses::GLIMPSE && !glimpsed {
                glimpsed = true;
                eprintln!(
                    "spot {k}: first glimpse at {:.1} m, e {:.3}",
                    flat(at - spot),
                    aware.exposure
                );
            }
            if aware.can_see_target && !seen {
                eprintln!(
                    "spot {k}: first seen at {:.1} m, guess off {:.1} unc {:.1} brain {:?}",
                    flat(at - spot),
                    aware.last_known.map(|l| flat(l - spot)).unwrap_or(-1.0),
                    aware.uncertainty,
                    app.world().get::<Brain>(walker).unwrap()
                );
            }
            seen |= aware.can_see_target;
        }
        eprintln!("spot {k} at {spot}: seen {seen}, closest {closest:.1} m");
        found += seen as u32;
    }
    eprintln!("FOUND {found} of {tried}");
}

/// A night visit: the response queued near the cabin is dropped, and when
/// it's due two walkers set off from the Gate to look inside, behind the
/// door they were sent to.
#[test]
fn a_night_visit_sends_two_walkers_to_the_door() {
    use super::director::{Director, NightVisit};
    let (mut app, _, _) = app(None);
    app.update();
    let ent = *app
        .world()
        .resource::<super::doors::Entrances>()
        .doors
        .iter()
        .find(|e| {
            app.world()
                .get::<crate::world::buildings::Building>(e.building)
                .is_some_and(|b| b.prefab == "house_plank")
        })
        .expect("a house door");
    {
        let mut d = app.world_mut().resource_mut::<Director>();
        d.enabled = true;
        d.next_job = 1e9;
        d.next_logistics = 1e9;
        d.response_cooldown = 1e9;
        d.queue_response(30.0, ent.mid + ent.out * 20.0);
        d.queue_response(30.0, ent.mid + Vec3::X * 400.0);
    }
    app.world_mut().write_message(NightVisit {
        door: ent.door,
        at: ent.mid - ent.out * 3.0,
        delay: 1.0,
    });
    for _ in 0..90 * 2 {
        app.update();
    }
    let d = app.world().resource::<Director>();
    assert_eq!(
        d.responses.len(),
        1,
        "the near one dropped: {:?}",
        d.responses
    );
    let inside = ent.mid - ent.out * 2.0;
    let visitors: Vec<_> = app
        .world_mut()
        .query::<(&Robot, &Brain, &Job, &Transform)>()
        .iter(app.world())
        .filter(|(r, _, j, _)| {
            r.kind == super::RobotKind::Walker
                && j.kind == JobKind::Response
                && j.site.distance(inside) < 0.1
        })
        .map(|(_, b, _, t)| {
            // In from the woods, a couple of minutes off.
            assert!(
                flat(t.translation - inside) < super::director::VISIT_FROM + 8.0,
                "close enough to come soon: {:.0} m",
                flat(t.translation - inside)
            );
            b.clone()
        })
        .collect();
    assert_eq!(visitors.len(), 2, "{visitors:?}");
    for b in visitors {
        assert!(
            matches!(b, Brain::Investigate { at, .. } if at.distance(inside) < 0.1),
            "looking inside: {b:?}"
        );
    }
}

/// A site sends a new grid (a gallery filled with gas, say) and the
/// crawler already on its round paths round what's now closed.
#[test]
fn a_new_site_grid_reroutes_its_crawlers() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let centre = zone.on_ground(100.0, 200.0);
    let open = site(centre, |_| false);
    let a = zone.on_ground(centre.x - 4.0, centre.z - 6.0);
    let b = zone.on_ground(centre.x - 4.0, centre.z + 6.0);
    let index = {
        let mut sites = app.world_mut().resource_mut::<super::indoor::IndoorSites>();
        sites.spawned = true;
        sites.sites.push(super::indoor::IndoorSite {
            nav: open.clone(),
            patrol: vec![a, b],
            spawn: a,
            crawlers: 1,
            wardens: 0,
            dormant: false,
        });
        sites.sites.len() - 1
    };
    let crawler = crawler_at(&mut app, &zone, open, vec![a, b]);
    app.update();
    let gassed = site(centre, |p| p.x.abs() < 12.0 && p.y.abs() < 2.0).grid;
    app.world_mut().write_message(super::indoor::SiteGrid {
        site: index,
        grid: gassed.clone(),
    });
    app.update();
    app.update();
    let nav = app.world().get::<super::indoor::LocalNav>(crawler).unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&nav.grid, &gassed),
        "it took the new grid"
    );
    let wall = |p: Vec2| p.x.abs() < 11.0 && p.y.abs() < 1.0;
    let (mut reached_b, mut crossed) = (false, false);
    for _ in 0..90 * 60 {
        app.update();
        let p = app.world().get::<Transform>(crawler).unwrap().translation - centre;
        crossed |= wall(Vec2::new(p.x, p.z));
        if Vec2::new(p.x + 4.0, p.z - 6.0).length() < 1.5 {
            reached_b = true;
            break;
        }
    }
    assert!(reached_b, "got there");
    assert!(!crossed, "round the gas, not through it");
}

/// The Sorter: seeing you, it rings the hall once, winds up (the tell)
/// and only then throws. Lose it and it watches where you were, gropes
/// round there with its arm, then goes back to work.
#[test]
fn the_sorter_rings_winds_up_throws_then_gropes_and_calms() {
    use super::sorter::{SorterOrder, SorterThrow, SorterWindup};
    use super::sorter_mind::{ALARM, GROPE_TIME, SorterMind, WATCH_TIME};
    #[derive(Resource, Default)]
    struct Seen {
        alarms: Vec<f32>,
        windups: Vec<f32>,
        throws: Vec<f32>,
    }
    let (mut app, player, zone) = app(None);
    app.init_resource::<Seen>().add_systems(
        Last,
        |mut n: MessageReader<super::Noise>,
         mut w: MessageReader<SorterWindup>,
         mut t: MessageReader<SorterThrow>,
         time: Res<Time>,
         mut s: ResMut<Seen>| {
            let now = time.elapsed_secs();
            s.alarms
                .extend(n.read().filter(|n| n.radius == ALARM).map(|_| now));
            s.windups.extend(w.read().map(|_| now));
            s.throws.extend(t.read().map(|_| now));
        },
    );
    // An open-air hall on a meadow, as the body's own tests use.
    let c = Vec2::new(40.0, 220.0);
    let mut top = f32::MIN;
    for i in -12..=12 {
        for j in -8..=8 {
            top = top.max(zone.ground(c.x + i as f32, c.y + j as f32));
        }
    }
    let hall = Transform::from_xyz(c.x, top, c.y);
    let sorter = {
        let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
        let mut cmd = q.get_mut(app.world_mut()).unwrap();
        cmd.spawn((
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(30.0, 0.2, 20.0),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::World,
                [
                    crate::world::Layer::Item,
                    crate::world::Layer::Robot,
                    crate::world::Layer::Player,
                ],
            ),
            Transform::from_xyz(c.x, top - 0.1, c.y),
        ));
        let e = super::sorter::spawn_sorter(
            &mut cmd,
            hall,
            Vec2::new(12.0, 8.0),
            6.0,
            super::sorter::depot_spots(),
            4,
        );
        q.apply(app.world_mut());
        e
    };
    // Standing in the hall, 7 m off its middle.
    put(&mut app, player, Vec3::new(c.x + 7.0, top, c.y + 2.0));
    for _ in 0..90 * 12 {
        app.update();
    }
    {
        let s = app.world().resource::<Seen>();
        assert_eq!(s.alarms.len(), 1, "rang the hall once: {:?}", s.alarms);
        let thrown = *s.throws.first().expect("it threw");
        let wound = s
            .windups
            .iter()
            .copied()
            .filter(|w| *w <= thrown)
            .next_back()
            .expect("wound up first");
        assert!(
            thrown - wound >= super::sorter::WINDUP * 0.9,
            "the tell ({:.2} s) before the throw",
            thrown - wound
        );
    }
    // Gone, out of its sight: it lost you by the hall's far end.
    put(&mut app, player, zone.on_ground(c.x + 200.0, c.y + 200.0));
    for _ in 0..3 {
        app.update();
    }
    {
        let mut a = app.world_mut().get_mut::<Awareness>(sorter).unwrap();
        a.target = None;
        a.can_see_target = false;
        a.last_known = Some(Vec3::new(c.x - 6.0, top, c.y - 3.0));
        a.uncertainty = 2.0;
    }
    *app.world_mut().get_mut::<SorterMind>(sorter).unwrap() = SorterMind::Watch {
        at: Vec3::new(c.x - 6.0, top, c.y - 3.0),
        left: WATCH_TIME,
    };
    let mut watched = false;
    let mut groped = false;
    for _ in 0..90 * (WATCH_TIME + GROPE_TIME + 6.0) as usize {
        app.update();
        let mind = *app.world().get::<SorterMind>(sorter).unwrap();
        let order = app.world().get::<SorterOrder>(sorter).copied();
        watched |= matches!(mind, SorterMind::Watch { .. })
            && matches!(order, Some(SorterOrder::Watch { .. }));
        groped |= matches!(mind, SorterMind::Grope { .. })
            && matches!(order, Some(SorterOrder::Reach { .. }));
    }
    assert!(watched, "it watched where you were");
    assert!(groped, "and groped round there");
    assert_eq!(
        *app.world().get::<SorterMind>(sorter).unwrap(),
        SorterMind::Calm
    );
    assert_eq!(
        app.world().get::<SorterOrder>(sorter).copied(),
        Some(SorterOrder::Idle),
        "back to its work"
    );
}

/// A player behind a plank house's door; an armed walker 6 m out that has
/// just lost them, believing them `inside` (else out by the door). The
/// times (s after it lost them) of its shots before it sees them again.
fn blind_fire(inside: bool) -> Vec<f32> {
    use crate::weapons::ballistics::Shot;
    #[derive(Resource, Default)]
    struct Fired(Vec<(Entity, f32)>);
    let (mut app, player, zone) = app(None);
    app.init_resource::<Fired>().add_systems(
        Last,
        |mut s: MessageReader<Shot>, time: Res<Time>, mut f: ResMut<Fired>| {
            let now = time.elapsed_secs();
            f.0.extend(s.read().filter_map(|s| s.shooter).map(|e| (e, now)));
        },
    );
    app.update();
    let ent = *app
        .world()
        .resource::<super::doors::Entrances>()
        .doors
        .iter()
        .find(|e| {
            app.world()
                .get::<crate::world::buildings::Building>(e.building)
                .is_some_and(|b| b.prefab == "house_plank")
        })
        .expect("a house door");
    let hide = ent.mid - ent.out * 2.0;
    put(&mut app, player, hide.with_y(ent.mid.y - 1.15));
    let post = ent.mid + ent.out * 6.0;
    // Armed (the helper's walkers aren't).
    let walker = {
        let mut queue = CommandQueue::default();
        let post = zone.on_ground(post.x, post.z);
        let w = {
            let mut c = Commands::new(&mut queue, app.world());
            super::walker::spawn_walker(
                &mut c,
                &zone,
                post,
                Job {
                    kind: JobKind::Watch,
                    site: post,
                    duration: 999.0,
                    exit: post,
                },
            )
        };
        queue.apply(app.world_mut());
        for _ in 0..90 {
            app.update();
        }
        w
    };
    app.update();
    let lost = app.world().resource::<Time>().elapsed_secs();
    {
        let mut a = app.world_mut().get_mut::<Awareness>(walker).unwrap();
        // (Out in the open, nobody to follow: its eyes would put you back.)
        a.target = inside.then_some(player);
        a.last_known = Some(if inside {
            hide.with_y(ent.mid.y - 1.15)
        } else {
            zone.on_ground(post.x + ent.out.z * 4.0, post.z - ent.out.x * 4.0)
        });
        a.uncertainty = 0.5;
        a.suspicion = 1.0;
        a.unseen = 0.0;
        a.can_see_target = false;
    }
    *app.world_mut().get_mut::<Brain>(walker).unwrap() = Brain::Engage;
    // (Until it sees you again: it opens the door, then it's aimed fire.)
    let mut sighted = f32::INFINITY;
    for _ in 0..90 * 6 {
        app.update();
        if app.world().get::<Awareness>(walker).unwrap().can_see_target {
            sighted = sighted.min(app.world().resource::<Time>().elapsed_secs() - lost);
        }
    }
    let fired: Vec<f32> = app
        .world()
        .resource::<Fired>()
        .0
        .iter()
        .filter(|(e, _)| *e == walker)
        .map(|(_, t)| *t - lost)
        .filter(|t| *t < sighted)
        .collect();
    fired
}

/// Behind a plank door, close: the walker that lost you there settles its
/// turret (the tell), then fires a short burst through it, once.
#[test]
fn a_walker_fires_through_a_plank_door_once() {
    let fired = blind_fire(true);
    assert!(!fired.is_empty(), "it fired through the door");
    assert!(
        fired.len() <= super::walker::BLIND_ROUNDS as usize,
        "a short burst, once: {fired:?}"
    );
    assert!(
        fired[0] >= super::walker::BLIND_AFTER + super::walker::BLIND_TELL - 0.05,
        "after the tell: {fired:?}"
    );
}

/// Lost out in the open (no roof to be under), it doesn't fire blind.
#[test]
fn a_walker_does_not_fire_blind_in_the_open() {
    let fired = blind_fire(false);
    assert!(fired.is_empty(), "no blind fire: {fired:?}");
}

/// A walker's guess at a sound hangs on the walker (where it was posted)
/// and the sound, not on how many other things exist: 50 unrelated
/// entities spawned first don't move it.
#[test]
fn a_guess_does_not_hang_on_entity_ids() {
    let guess = |extra: usize| {
        let (mut app, player, zone) = app(None);
        for _ in 0..extra {
            app.world_mut().spawn_empty();
        }
        put(&mut app, player, zone.on_ground(100.0, 400.0));
        let w = walker_at(&mut app, &zone, zone.on_ground(100.0, 200.0));
        app.world_mut().write_message(super::Noise {
            at: zone.on_ground(101.0, 240.0),
            radius: super::senses::GUNSHOT,
            source: Some(player),
        });
        app.update();
        app.update();
        app.world()
            .get::<Awareness>(w)
            .unwrap()
            .last_known
            .expect("heard it")
    };
    let (a, b) = (guess(0), guess(50));
    assert!(a.distance(b) < 1e-3, "{a} vs {b}");
}

/// Two walkers on one player: one pins, the other works round to the
/// side (and says so, once), so cover that faces one doesn't face both.
#[test]
fn two_walkers_pin_and_flank() {
    use super::search::{Mind, MindShift};
    #[derive(Resource, Default)]
    struct Shifts(Vec<MindShift>);
    let (mut app, player, zone) = app(None);
    app.init_resource::<Shifts>().add_systems(
        Last,
        |mut m: MessageReader<MindShift>, mut s: ResMut<Shifts>| s.0.extend(m.read().copied()),
    );
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let a = walker_at(&mut app, &zone, zone.on_ground(98.0, 225.0));
    let b = walker_at(&mut app, &zone, zone.on_ground(102.0, 225.0));
    for w in [a, b] {
        let mut aw = app.world_mut().get_mut::<Awareness>(w).unwrap();
        aw.target = Some(player);
        aw.last_known = Some(me);
        aw.suspicion = 1.0;
        *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
    }
    let angle = |app: &App| {
        let p = |w: Entity| {
            let t = app.world().get::<Transform>(w).unwrap().translation;
            Vec2::new(t.x - me.x, t.z - me.z)
        };
        p(a).angle_to(p(b)).abs().to_degrees()
    };
    let start = angle(&app);
    let mut widest = start;
    for _ in 0..90 * 12 {
        put(&mut app, player, me);
        app.update();
        widest = widest.max(angle(&app));
    }
    assert!(
        widest > 45.0,
        "spread round you: {start:.0}° -> {widest:.0}°"
    );
    let flanks: Vec<Entity> = app
        .world()
        .resource::<Shifts>()
        .0
        .iter()
        .filter(|s| s.state == Mind::Flanking)
        .map(|s| s.robot)
        .collect();
    assert_eq!(
        flanks.len(),
        1,
        "one of them flanks, and says so once: {flanks:?}"
    );
}

/// A walker with its core shot most of the way through breaks off: it
/// says so, once, and falls back further from you than a sound one.
#[test]
fn a_badly_hurt_walker_breaks_off() {
    use super::search::{Mind, MindShift};
    #[derive(Resource, Default)]
    struct Shifts(Vec<MindShift>);
    let run = |hurt: bool| {
        let (mut app, player, zone) = app(None);
        app.init_resource::<Shifts>().add_systems(
            Last,
            |mut m: MessageReader<MindShift>, mut s: ResMut<Shifts>| s.0.extend(m.read().copied()),
        );
        let me = zone.on_ground(100.0, 200.0);
        put(&mut app, player, me);
        let w = walker_at(&mut app, &zone, zone.on_ground(100.0, 216.0));
        if hurt {
            let mut q = app.world_mut().query::<&mut super::RobotPart>();
            for mut p in q.iter_mut(app.world_mut()) {
                if p.robot == w && p.role == super::PartRole::Core {
                    p.hp = p.max_hp * 0.3;
                }
            }
        }
        {
            let mut aw = app.world_mut().get_mut::<Awareness>(w).unwrap();
            aw.target = Some(player);
            aw.last_known = Some(me);
            aw.suspicion = 1.0;
        }
        *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
        for _ in 0..90 * 12 {
            put(&mut app, player, me);
            app.update();
        }
        let d = flat(app.world().get::<Transform>(w).unwrap().translation - me);
        let said = app
            .world()
            .resource::<Shifts>()
            .0
            .iter()
            .filter(|s| s.robot == w && s.state == Mind::Retreating)
            .count();
        (d, said)
    };
    let (sound, sound_said) = run(false);
    let (hurt, hurt_said) = run(true);
    assert_eq!(sound_said, 0);
    assert_eq!(hurt_said, 1, "it says it's breaking off, once");
    assert!(
        hurt > sound + 6.0,
        "it fell back: {hurt:.1} m, a sound one {sound:.1} m"
    );
}

/// Keep shooting and they send more: one reported shot gets the usual
/// response; three in half a minute, and two walkers come to look as well.
#[test]
fn keep_shooting_and_walkers_come_too() {
    use super::director::{Director, ESCALATE, HEAT_GAP};
    let walkers_sent = |shots: usize| {
        let (mut app, player, zone) = app(None);
        app.update();
        {
            let mut d = app.world_mut().resource_mut::<Director>();
            d.enabled = true;
            d.next_job = 1e9;
            d.next_logistics = 1e9;
            d.max_active = 99;
        }
        let at = zone.on_ground(100.0, 200.0);
        for _ in 0..shots {
            app.world_mut().write_message(super::Noise {
                at,
                radius: super::senses::GUNSHOT,
                source: Some(player),
            });
            for _ in 0..(90.0 * (HEAT_GAP + 1.0)) as usize {
                app.update();
            }
        }
        // The response goes out (it was queued 35-70 s after the first).
        for _ in 0..90 * 70 {
            app.update();
        }
        app.world_mut()
            .query::<(&Robot, &Job)>()
            .iter(app.world())
            .filter(|(r, j)| {
                r.kind == super::RobotKind::Walker
                    && j.kind == JobKind::Response
                    && j.site.distance(at) < 0.5
            })
            .count()
    };
    assert!(ESCALATE > 1.0);
    assert_eq!(walkers_sent(1), 0, "one shot: the usual response");
    assert_eq!(walkers_sent(3), 2, "three: two walkers as well");
}

/// A drone that notices you is the walkers' spotter: its call reaches a
/// walker 100 m off; a walker's own call doesn't carry that far.
#[test]
fn a_drone_spots_for_walkers_far_off() {
    let heard = |drone_calls: bool| {
        let (mut app, player, zone) = app(None);
        put(&mut app, player, zone.on_ground(100.0, 330.0));
        let here = zone.on_ground(100.0, 200.0);
        let listener = walker_at(&mut app, &zone, zone.on_ground(100.0, 100.0));
        let caller = if drone_calls {
            let mut queue = CommandQueue::default();
            let d = {
                let mut c = Commands::new(&mut queue, app.world());
                super::drone::spawn_drone(
                    &mut c,
                    here + Vec3::Y * 15.0,
                    Job {
                        kind: JobKind::Survey,
                        site: here,
                        duration: 999.0,
                        exit: here,
                    },
                )
            };
            queue.apply(app.world_mut());
            app.update();
            d
        } else {
            walker_at(&mut app, &zone, here)
        };
        let you = zone.on_ground(100.0, 215.0);
        {
            let mut a = app.world_mut().get_mut::<Awareness>(caller).unwrap();
            a.target = Some(player);
            a.last_known = Some(you);
            a.uncertainty = 1.0;
            a.suspicion = 1.0;
        }
        app.world_mut().write_message(super::Noticed {
            robot: caller,
            at: here,
        });
        app.update();
        app.update();
        let a = app.world().get::<Awareness>(listener).unwrap();
        a.suspicion >= super::search::CALL_SUSPICION * 0.9
            && a.last_known.is_some_and(|l| flat(l - you) < 10.0)
    };
    assert!(heard(true), "the drone's call reached it");
    assert!(!heard(false), "a walker's call doesn't carry 100 m");
}

/// A pair of hounds on one player split up: one comes round each side.
#[test]
fn a_pair_of_hounds_splits_up() {
    let (mut app, player, zone) = app(None);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    // Both start off the same side.
    let a = hound_at(
        &mut app,
        &zone,
        zone.on_ground(103.0, 225.0),
        me,
        JobKind::Response,
    );
    let b = hound_at(
        &mut app,
        &zone,
        zone.on_ground(105.0, 225.0),
        me,
        JobKind::Response,
    );
    for h in [a, b] {
        // (Plain eyes: this is about splitting up, not seeing you.)
        app.world_mut()
            .entity_mut(h)
            .remove::<super::senses::MotionSight>();
        app.world_mut().get_mut::<super::Senses>(h).unwrap().fov_deg = 360.0;
        let mut aw = app.world_mut().get_mut::<Awareness>(h).unwrap();
        aw.target = Some(player);
        aw.last_known = Some(me);
        aw.suspicion = 1.0;
        *app.world_mut().get_mut::<Brain>(h).unwrap() = Brain::Engage;
    }
    let bearing = |app: &App, h: Entity| {
        let t = app.world().get::<Transform>(h).unwrap().translation;
        Vec2::new(t.x - me.x, t.z - me.z)
    };
    // Walking a small circle (a still body barely shows to a hound).
    let mut widest = 0.0f32;
    for i in 0..90 * 6 {
        let ang = i as f32 / 90.0 * 0.6;
        put(
            &mut app,
            player,
            me + Vec3::new(ang.cos(), 0.0, ang.sin()) * 1.5,
        );
        app.update();
        let (pa, pb) = (bearing(&app, a), bearing(&app, b));
        if pa.length() > 5.0 && pb.length() > 5.0 {
            widest = widest.max(pa.angle_to(pb).abs().to_degrees());
        }
    }
    assert!(widest > 60.0, "they split round you: {widest:.0}°");
}

/// Fresh prints in the snow, leading off from in front of a walker: it
/// reads the trail and goes to look along it, not where it stands.
#[test]
fn a_walker_follows_fresh_prints_in_the_snow() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 400.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    // A walker faces -Z: prints from 6 m in front, leading on north.
    {
        let mut tracks = app.world_mut().resource_mut::<crate::world::snow::Tracks>();
        for k in 0..30 {
            let z = post.z - 6.0 - k as f32 * 0.8;
            tracks.add(crate::world::snow::Print {
                at: zone.on_ground(post.x + (k % 2) as f32 * 0.3, z),
                yaw: 0.0,
                left: k % 2 == 0,
                by: player,
                fill: 0.2,
            });
        }
    }
    let mut lead = None;
    for _ in 0..90 * 2 {
        app.update();
        if let Brain::Investigate { at, .. } = *app.world().get::<Brain>(w).unwrap() {
            lead = Some(at);
            break;
        }
    }
    let at = lead.expect("it went to look");
    // Along the trail, well past where it first saw it.
    assert!(
        post.z - at.z > 6.0 + super::tracks::LEAD * 0.6,
        "read on along the trail: {at}"
    );
}

/// A scope's glint gives a still sniper away, further off than a walker
/// sees a body: it places them (within the glimpse spread), where it had
/// a wrong guess before. No glint, and it's none the wiser.
#[test]
fn a_glint_gives_a_still_sniper_away() {
    #[derive(Resource)]
    struct Flash(Option<(Entity, Vec3, Vec3)>);
    let placed = |flash: bool| {
        let (mut app, player, zone) = app(None);
        let post = zone.on_ground(100.0, 200.0);
        let w = walker_at(&mut app, &zone, post);
        // 150 m in front of it (it faces -Z), past its eyes' reach.
        let you = zone.on_ground(100.0, 50.0);
        put(&mut app, player, you);
        let eye = post + Vec3::Y * 1.6;
        let head = you + Vec3::Y * 1.6;
        app.insert_resource(Flash(flash.then_some((player, head, eye - head))))
            .add_systems(
                Last,
                |f: Res<Flash>, mut g: ResMut<crate::weapons::optics::Glints>| {
                    if let Some((p, at, toward)) = f.0 {
                        g.0.push((
                            p,
                            crate::weapons::optics::Glint {
                                strength: 1.0,
                                at,
                                toward,
                            },
                        ));
                    }
                },
            );
        {
            let mut a = app.world_mut().get_mut::<Awareness>(w).unwrap();
            a.target = Some(player);
            a.last_known = Some(zone.on_ground(160.0, 50.0));
            a.uncertainty = 10.0;
        }
        for _ in 0..90 {
            app.update();
        }
        let a = app.world().get::<Awareness>(w).unwrap();
        a.last_known.map(|l| flat(l - you)).unwrap_or(f32::MAX)
    };
    let off = placed(true);
    assert!(
        off < 150.0 * super::senses::GLIMPSE_SPREAD + 1.0,
        "the flash placed you: {off:.1} m off"
    );
    assert!(placed(false) > 50.0, "no flash, no news");
}

/// A lure that keeps going (a tape playing on a table) draws a walker
/// once: it looks, searches, finds nothing, gives up, and then the same
/// sound there doesn't draw it again for a while.
#[test]
fn a_lure_that_keeps_going_wears_out() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 400.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    let spot = zone.on_ground(post.x + 14.0, post.z + 3.0);
    // Once it has given up (2 s calm), it never goes back to the tune.
    let (mut looked, mut calm, mut drawn_again) = (false, 0.0f32, None);
    for i in 0..90 * 150 {
        if i % (90 * 3) == 0 {
            app.world_mut().write_message(super::Noise {
                at: spot,
                radius: 25.0,
                source: None,
            });
        }
        app.update();
        let brain = app.world().get::<Brain>(w).unwrap();
        let looking = matches!(brain, Brain::Investigate { .. } | Brain::Search { .. });
        looked |= looking;
        if looking {
            if looked && calm > 2.0 && drawn_again.is_none() {
                drawn_again = Some(i as f32 / 90.0);
            }
            calm = 0.0;
        } else if looked {
            calm += 1.0 / 90.0;
        }
    }
    assert!(looked && calm > 2.0, "it looked and gave up");
    assert_eq!(drawn_again, None, "and the same tune didn't draw it again");
}

/// A small placement that's cleared away (a crate burst) comes off the
/// walkers' grid: where it stood is walkable again, once the grid is
/// rebuilt (off the main thread).
#[test]
fn a_cleared_crate_comes_off_the_grid() {
    let (mut app, _, zone) = app(None);
    app.update();
    let nav = app.world().resource::<super::walker::Nav>().0.clone();
    let (i, p) = zone
        .map
        .placements
        .iter()
        .enumerate()
        .find(|(_, p)| {
            let f = extraction_core::map::prefab_footprint(&p.prefab);
            f[0].max(f[2]) <= super::walker::GONE_SIZE && !nav.walkable(p.at)
        })
        .expect("a small placement on the grid");
    let at = zone.on_ground(p.at[0], p.at[1]);
    app.world_mut().write_message(crate::destruction::Cleared {
        min: at - Vec3::splat(0.4),
        max: at + Vec3::splat(0.4),
    });
    let mut open = false;
    for _ in 0..90 * 10 {
        app.update();
        let nav = app.world().resource::<super::walker::Nav>().0.clone();
        if nav.walkable(p.at) {
            open = true;
            break;
        }
    }
    assert!(open, "placement {i} ({}) came off the grid", p.prefab);
    assert!(
        app.world()
            .resource::<super::walker::NavGone>()
            .placements
            .contains(&i)
    );
}

/// When everyone's goal jumps at once (the player teleported home), the
/// long path searches are spread over a few ticks, PATH_BUDGET a tick,
/// not all in one frame; and they all get there.
#[test]
fn a_repath_storm_is_spread_over_ticks() {
    use super::walker::{PATH_BUDGET, Walker};
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 400.0));
    let walkers: Vec<Entity> = (0..8)
        .map(|k| {
            walker_at(
                &mut app,
                &zone,
                zone.on_ground(90.0 + k as f32 * 3.0, 200.0),
            )
        })
        .collect();
    let far = zone.on_ground(100.0, 140.0);
    for &w in &walkers {
        *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Investigate {
            at: far,
            left: 60.0,
        };
    }
    let mut ticks = 0;
    let mut done = std::collections::HashSet::new();
    while done.len() < walkers.len() && ticks < 30 {
        app.update();
        ticks += 1;
        let mut this_tick = 0;
        for &w in &walkers {
            let to_far = app
                .world()
                .get::<Walker>(w)
                .unwrap()
                .goal
                .is_some_and(|g| flat(g - far) < 0.5);
            if to_far && done.insert(w) {
                this_tick += 1;
            }
        }
        assert!(
            this_tick <= PATH_BUDGET as usize * 2,
            "{this_tick} long searches in one update"
        );
    }
    assert_eq!(
        done.len(),
        walkers.len(),
        "all of them repathed ({ticks} updates)"
    );
}

/// An armed walker at `post`, at its post.
fn armed_walker_at(app: &mut App, zone: &Zone, post: Vec3) -> Entity {
    let mut queue = CommandQueue::default();
    let w = {
        let mut c = Commands::new(&mut queue, app.world());
        super::walker::spawn_walker(
            &mut c,
            zone,
            post,
            Job {
                kind: JobKind::Watch,
                site: post,
                duration: 999.0,
                exit: post,
            },
        )
    };
    queue.apply(app.world_mut());
    for _ in 0..90 {
        app.update();
    }
    w
}

/// A walker that's just picked a fight looks before it shoots: its first
/// round comes no sooner than SIGHT_BEAT + AIM_TIME after it starts.
#[test]
fn a_walker_looks_before_it_shoots() {
    use crate::weapons::ballistics::Shot;
    #[derive(Resource, Default)]
    struct Fired(Vec<(Entity, f32)>);
    let (mut app, player, zone) = app(None);
    // Timed on the robots' own (fixed) clock.
    app.init_resource::<Fired>().add_systems(
        FixedPostUpdate,
        |mut s: MessageReader<Shot>, time: Res<Time>, mut f: ResMut<Fired>| {
            let now = time.elapsed_secs();
            f.0.extend(s.read().filter_map(|s| s.shooter).map(|e| (e, now)));
        },
    );
    // Out of its sight while it's set up (it would pick the fight early).
    put(&mut app, player, zone.on_ground(100.0, 400.0));
    let me = zone.on_ground(100.0, 200.0);
    let w = armed_walker_at(&mut app, &zone, zone.on_ground(100.0, 218.0));
    put(&mut app, player, me);
    {
        let mut a = app.world_mut().get_mut::<Awareness>(w).unwrap();
        a.target = Some(player);
        a.last_known = Some(me);
        a.suspicion = 1.0;
    }
    *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
    let start = app.world().resource::<Time<Fixed>>().elapsed_secs();
    for _ in 0..90 * 6 {
        put(&mut app, player, me);
        app.update();
    }
    let first = app
        .world()
        .resource::<Fired>()
        .0
        .iter()
        .find(|(e, _)| *e == w)
        .map(|(_, t)| *t - start)
        .expect("it fired in the end");
    assert!(
        first >= super::walker::SIGHT_BEAT + super::walker::AIM_TIME - 0.05,
        "it looked first: first round {first:.2} s in"
    );
}

/// Go down and they let you go: a walker fighting you forgets you and
/// gives up, so it isn't waiting when you come to.
#[test]
fn going_down_lets_you_go() {
    let (mut app, player, zone) = app(None);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let w = walker_at(&mut app, &zone, zone.on_ground(100.0, 218.0));
    {
        let mut a = app.world_mut().get_mut::<Awareness>(w).unwrap();
        a.target = Some(player);
        a.last_known = Some(me);
        a.suspicion = 1.0;
    }
    *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
    app.update();
    // Down, as health does it: the flag, and the message.
    app.world_mut()
        .get_mut::<crate::health::Health>(player)
        .unwrap()
        .down = true;
    app.world_mut()
        .write_message(crate::health::Downed { player, at: me });
    app.update();
    app.update();
    let a = app.world().get::<Awareness>(w).unwrap();
    assert_eq!(a.target, None, "it forgot you");
    assert!(
        !matches!(
            app.world().get::<Brain>(w).unwrap(),
            Brain::Engage | Brain::Investigate { .. } | Brain::Search { .. }
        ),
        "and gave up: {:?}",
        app.world().get::<Brain>(w).unwrap()
    );
}

/// Sprinting across a drone's line spoils its aim: over the same fight,
/// a running player takes well under half the hits a still one does.
#[test]
fn running_across_a_drone_spoils_its_aim() {
    #[derive(Resource, Default)]
    struct Hits(usize);
    let hits = |running: bool| {
        let (mut app, player, zone) = app(None);
        app.init_resource::<Hits>().add_systems(
            Last,
            |mut h: MessageReader<crate::health::Hurt>, mut n: ResMut<Hits>| {
                n.0 += h.read().count();
            },
        );
        put(&mut app, player, zone.on_ground(100.0, 400.0));
        let me = zone.on_ground(100.0, 200.0);
        let over = zone.on_ground(100.0, 220.0) + Vec3::Y * 18.0;
        let mut queue = CommandQueue::default();
        let d = {
            let mut c = Commands::new(&mut queue, app.world());
            super::drone::spawn_drone(
                &mut c,
                over,
                Job {
                    kind: JobKind::Response,
                    site: me,
                    duration: 999.0,
                    exit: over,
                },
            )
        };
        queue.apply(app.world_mut());
        for _ in 0..30 {
            app.update();
        }
        put(&mut app, player, me);
        {
            let mut a = app.world_mut().get_mut::<Awareness>(d).unwrap();
            a.target = Some(player);
            a.last_known = Some(me);
            a.suspicion = 1.0;
        }
        *app.world_mut().get_mut::<Brain>(d).unwrap() = Brain::Engage;
        for _ in 0..90 * 15 {
            put(&mut app, player, me);
            // Running hard across (east), as far as its aim can tell.
            app.world_mut()
                .get_mut::<crate::player::Body>(player)
                .unwrap()
                .velocity = if running { Vec3::X * 5.0 } else { Vec3::ZERO };
            if let Some(mut h) = app.world_mut().get_mut::<crate::health::Health>(player) {
                h.hp = 100.0;
                h.down = false;
            }
            app.update();
        }
        app.world().resource::<Hits>().0
    };
    let still = hits(false);
    let running = hits(true);
    assert!(still >= 4, "it hits a still target ({still})");
    assert!(
        (running as f32) < still as f32 * 0.5,
        "running spoils its aim: {running} hits against {still}"
    );
}

/// A drone picks out movement more than shape: crouched and still, 40 m
/// off in the open, you're not made out; crouched and moving, you are.
#[test]
fn keeping_still_beats_a_drone_overhead() {
    let seen = |moving: bool| {
        let (mut app, player, zone) = app(None);
        put(&mut app, player, zone.on_ground(100.0, 400.0));
        let me = zone.on_ground(100.0, 200.0);
        let over = zone.on_ground(100.0, 238.0) + Vec3::Y * 15.0;
        let mut queue = CommandQueue::default();
        let d = {
            let mut c = Commands::new(&mut queue, app.world());
            super::drone::spawn_drone(
                &mut c,
                over,
                Job {
                    kind: JobKind::Survey,
                    site: over,
                    duration: 999.0,
                    exit: over,
                },
            )
        };
        queue.apply(app.world_mut());
        for _ in 0..30 {
            app.update();
        }
        let mut best = 0.0f32;
        for i in 0..90 * 4 {
            let walk = if moving {
                Vec3::X * (i as f32 / 90.0)
            } else {
                Vec3::ZERO
            };
            put(&mut app, player, me + walk);
            app.world_mut()
                .get_mut::<crate::player::Body>(player)
                .unwrap()
                .velocity = if moving { Vec3::X * 1.0 } else { Vec3::ZERO };
            app.world_mut()
                .get_mut::<RigInput>(player)
                .unwrap()
                .head
                .translation
                .y = 0.85;
            app.update();
            best = best.max(app.world().get::<Awareness>(d).unwrap().exposure);
        }
        best
    };
    let still = seen(false);
    let moving = seen(true);
    assert!(
        still < super::senses::SEEN,
        "still, it can't make you out ({still:.3})"
    );
    assert!(
        moving > still * 1.5,
        "moving shows ({moving:.3} vs {still:.3})"
    );
}

/// Survey drones aren't sent to loiter over a rendezvous.
#[test]
fn survey_drones_keep_off_the_rendezvous() {
    use super::director::{Director, SURVEY_CLEAR};
    use extraction_core::map::MarkerKind;
    let (mut app, _, zone) = app(None);
    app.update();
    {
        let mut d = app.world_mut().resource_mut::<Director>();
        d.enabled = true;
        d.next_logistics = 1e9;
        d.max_active = 999;
    }
    for _ in 0..60 {
        app.world_mut().resource_mut::<Director>().next_job = 0.0;
        app.update();
    }
    let rvs: Vec<Vec2> = zone
        .map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| Vec2::from_array(m.at))
        .collect();
    let surveys: Vec<Vec3> = app
        .world_mut()
        .query::<&Job>()
        .iter(app.world())
        .filter(|j| j.kind == JobKind::Survey)
        .map(|j| j.site)
        .collect();
    assert!(!surveys.is_empty(), "some went");
    for s in surveys {
        for r in &rvs {
            assert!(
                r.distance(Vec2::new(s.x, s.z)) > SURVEY_CLEAR,
                "a survey at {s} by the rendezvous at {r}"
            );
        }
    }
}

/// A dragged car door scrapes (`interact::carry`: a noise of yours, twice
/// a second, `DRAG_HEARD` at a drag's pace): a walker 10 m off comes to
/// look where it's scraping, and doesn't tire of it as of a lure.
#[test]
fn a_dragged_door_draws_a_walker() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    let mut looked = None;
    for i in 0..90 * 6 {
        // Dragged along 10 m off, at a drag's pace.
        if i % 45 == 0 {
            let x = post.x + 10.0 + i as f32 / 90.0 * crate::interact::carry::DRAG_SPEED;
            app.world_mut().write_message(super::Noise {
                at: zone.on_ground(x, post.z - 2.0),
                radius: crate::interact::carry::DRAG_HEARD,
                source: Some(player),
            });
        }
        app.update();
        if let Brain::Investigate { at, .. } = *app.world().get::<Brain>(w).unwrap() {
            looked = Some(at);
            break;
        }
    }
    let at = looked.expect("it went to look at the scrape");
    assert!(
        flat(at - zone.on_ground(post.x + 11.0, post.z - 2.0)) < 5.0,
        "where it scraped: {at}"
    );
}

/// A held gun knocked hard on a doorframe (`weapons::carry::GunBumped`)
/// 5 m from a walker: it comes to look there.
#[test]
fn a_gun_knocked_on_a_doorframe_draws_a_walker() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    // What's knocked: held by you (only the hold matters here).
    let gun = app
        .world_mut()
        .spawn(crate::interact::Held {
            player,
            hand: Entity::PLACEHOLDER,
            side: crate::rig::Side::Right,
            point: 0,
            offset: Transform::IDENTITY,
        })
        .id();
    let at = zone.on_ground(post.x + 4.0, post.z - 3.0) + Vec3::Y;
    app.world_mut()
        .write_message(crate::weapons::carry::GunBumped {
            gun,
            at,
            stuff: crate::interact::touch::Stuff::Wood,
            speed: 1.5,
        });
    let mut looked = None;
    for _ in 0..90 * 2 {
        app.update();
        if let Brain::Investigate { at, .. } = *app.world().get::<Brain>(w).unwrap() {
            looked = Some(at);
            break;
        }
    }
    let to = looked.expect("it went to look at the knock");
    assert!(flat(to - at) < 3.0, "where it knocked: {to}");
    assert_eq!(
        app.world().get::<Awareness>(w).unwrap().target,
        Some(player)
    );
}

/// Standing stock still far enough off, a walker doesn't make you out;
/// with a car door swinging in your hands (`signs::LOAD_SHOWS`), it does.
#[test]
fn a_swinging_car_door_gives_you_away() {
    let seen = |load: bool| {
        let (mut app, player, zone) = app(None);
        let post = zone.on_ground(100.0, 200.0);
        let w = walker_at(&mut app, &zone, post);
        // Far enough off that keeping still hides you, and not so far a
        // swinging load can't show: by its own eyes and today's light.
        let reach = app
            .world()
            .resource::<crate::world::weather::Conditions>()
            .sight(app.world().get::<super::Senses>(w).unwrap().sight);
        let c = app.world().resource::<crate::world::weather::Conditions>();
        let (light, ground_fog) = (c.light_at(false), c.ground_fog);
        let height = app
            .world()
            .get::<crate::player::Body>(player)
            .unwrap()
            .height;
        let case = |d: f32, speed: f32, still: f32| super::senses::SightCase {
            distance: d,
            reach,
            ground_fog,
            light,
            height,
            speed,
            cover: false,
            depression: 0.0,
            dark: None,
            still,
            canopy: false,
        };
        // Where it tells most: still well under seen, swinging well over.
        let margin = |d: f32| {
            let still = super::senses::exposure(&case(d, 0.0, super::senses::STILL));
            let swinging = super::senses::exposure(&case(d, 1.0, 1.0)) * super::signs::LOAD_SHOWS;
            (super::senses::SEEN / still.max(1e-6)).min(swinging / super::senses::SEEN)
        };
        let d = (40..800)
            .map(|k| k as f32 * 0.25)
            .max_by(|a, b| margin(*a).total_cmp(&margin(*b)))
            .unwrap();
        assert!(margin(d) > 1.3, "a range where it tells ({d} m)");
        // In front of it (it faces -Z).
        put(&mut app, player, zone.on_ground(post.x, post.z - d));
        if load {
            app.world_mut().spawn((
                crate::interact::Held {
                    player,
                    hand: Entity::PLACEHOLDER,
                    side: crate::rig::Side::Right,
                    point: 0,
                    offset: Transform::IDENTITY,
                },
                crate::interact::carry::Bulky,
                avian3d::prelude::LinearVelocity(Vec3::new(0.8, 0.0, 0.4)),
            ));
        }
        for _ in 0..90 * 2 {
            app.update();
            if app.world().get::<Awareness>(w).unwrap().can_see_target {
                return true;
            }
        }
        false
    };
    assert!(!seen(false), "standing still, it looks through you");
    assert!(seen(true), "a swinging door, it has you");
}

/// Prints in the snow stop at a plough's fresh lane: a walker reading the
/// trail guesses you went on along the lane, the way you were heading,
/// and goes to look there, not at the last print.
#[test]
fn a_trail_that_stops_at_a_ploughed_lane_is_followed_along_it() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 400.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    // A lane across in front of it, 14 m out, along X, cleared just now.
    let lane_z = post.z - 14.0;
    {
        let now = app.world().resource::<Time<Fixed>>().elapsed_secs();
        let mut lanes = app.world_mut().resource_mut::<super::plough::Lanes>();
        for i in -20..20 {
            let x = post.x + i as f32 * 2.0;
            lanes
                .0
                .push((Vec2::new(x, lane_z), Vec2::new(x + 2.0, lane_z), now));
        }
    }
    // Prints from 6 m in front, heading north and a little east, up to
    // the lane's edge, and none past it.
    {
        let mut tracks = app.world_mut().resource_mut::<crate::world::snow::Tracks>();
        for k in 0..8 {
            let z = post.z - 6.0 - k as f32 * 0.8;
            tracks.add(crate::world::snow::Print {
                at: zone.on_ground(post.x + k as f32 * 0.25, z),
                yaw: 0.0,
                left: k % 2 == 0,
                by: player,
                fill: 0.2,
            });
        }
    }
    let mut lead = None;
    for _ in 0..90 * 2 {
        app.update();
        if let Brain::Investigate { at, .. } = *app.world().get::<Brain>(w).unwrap() {
            lead = Some(at);
            break;
        }
    }
    let at = lead.expect("it went to look");
    assert!((at.z - lane_z).abs() < 2.0, "on the lane: {at}");
    assert!(
        at.x - post.x > super::signs::LANE_LEAD * 0.7,
        "on along it, east: {at}"
    );
}

/// Knocked off its feet mid-fight, a walker holds its fire while it's
/// down and getting up, drops the cover it had, and once up must aim
/// afresh before it fires again.
#[test]
fn a_knocked_down_walker_holds_fire_and_aims_afresh() {
    use crate::weapons::ballistics::Shot;
    #[derive(Resource, Default)]
    struct Fired(Vec<(Entity, f32)>);
    let (mut app, player, zone) = app(None);
    app.init_resource::<Fired>().add_systems(
        FixedPostUpdate,
        |mut s: MessageReader<Shot>, time: Res<Time>, mut f: ResMut<Fired>| {
            let now = time.elapsed_secs();
            f.0.extend(s.read().filter_map(|s| s.shooter).map(|e| (e, now)));
        },
    );
    put(&mut app, player, zone.on_ground(100.0, 400.0));
    let me = zone.on_ground(100.0, 200.0);
    let w = armed_walker_at(&mut app, &zone, zone.on_ground(100.0, 218.0));
    put(&mut app, player, me);
    {
        let mut a = app.world_mut().get_mut::<Awareness>(w).unwrap();
        a.target = Some(player);
        a.last_known = Some(me);
        a.suspicion = 1.0;
    }
    *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
    // Fighting: it has fired by now.
    for _ in 0..90 * 5 {
        put(&mut app, player, me);
        app.update();
    }
    let clock = |app: &App| app.world().resource::<Time<Fixed>>().elapsed_secs();
    assert!(
        app.world()
            .resource::<Fired>()
            .0
            .iter()
            .any(|(e, _)| *e == w),
        "it was firing"
    );
    // A heavy round: down it goes.
    app.world_mut()
        .get_mut::<super::anim::Gait>(w)
        .unwrap()
        .daze(super::anim::KNOCKDOWN_DAMAGE * 1.2, Vec3::X);
    let floored = clock(&app);
    let up = floored + super::anim::FLOORED_FOR + super::anim::RISE_FOR;
    let mut cover_while_down = false;
    for _ in 0..90 * 8 {
        put(&mut app, player, me);
        app.update();
        let g = app.world().get::<super::anim::Gait>(w).unwrap();
        if g.down() || g.rising > 0.0 {
            cover_while_down |= app
                .world()
                .get::<super::walker::Walker>(w)
                .unwrap()
                .cover
                .is_some();
        }
    }
    let after: Vec<f32> = app
        .world()
        .resource::<Fired>()
        .0
        .iter()
        .filter(|(e, t)| *e == w && *t > floored + 0.05)
        .map(|(_, t)| *t)
        .collect();
    let first = *after.first().expect("it fired again once up");
    assert!(
        first >= up + super::walker::AIM_TIME - 0.1,
        "held fire down, aimed afresh up: first round {:.2} s after the knock (up at {:.2})",
        first - floored,
        up - floored
    );
    assert!(!cover_while_down, "it let go of its old cover");
}

/// Knocked down once, a walker gets up and fights on; knocked down again
/// inside KNOCKED_AGAIN, it breaks off.
#[test]
fn knocked_down_twice_a_walker_breaks_off() {
    let (mut app, player, zone) = app(None);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let w = walker_at(&mut app, &zone, zone.on_ground(100.0, 218.0));
    {
        let mut a = app.world_mut().get_mut::<Awareness>(w).unwrap();
        a.target = Some(player);
        a.last_known = Some(me);
        a.suspicion = 1.0;
    }
    *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
    let knock = |app: &mut App| {
        app.world_mut()
            .get_mut::<super::anim::Gait>(w)
            .unwrap()
            .daze(super::anim::KNOCKDOWN_DAMAGE * 1.2, Vec3::X);
        let back_up = super::anim::FLOORED_FOR + super::anim::RISE_FOR + 0.5;
        for _ in 0..(90.0 * back_up) as usize {
            put(app, player, me);
            app.update();
        }
    };
    knock(&mut app);
    assert!(
        !app.world()
            .entity(w)
            .contains::<super::walker::Retreating>(),
        "once, it fights on"
    );
    knock(&mut app);
    assert!(
        app.world()
            .entity(w)
            .contains::<super::walker::Retreating>(),
        "twice in a few seconds, it breaks off"
    );
}

/// Down to two legs, a walker hauls itself along on its belly: it fights
/// from where it is, and never goes round your side.
#[test]
fn a_crawling_walker_doesnt_flank() {
    let (mut app, player, zone) = app(None);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let a = walker_at(&mut app, &zone, zone.on_ground(98.0, 225.0));
    let b = walker_at(&mut app, &zone, zone.on_ground(102.0, 225.0));
    for w in [a, b] {
        {
            let mut walker = app.world_mut().get_mut::<super::walker::Walker>(w).unwrap();
            walker.feet[0].alive = false;
            walker.feet[1].alive = false;
        }
        let mut aw = app.world_mut().get_mut::<Awareness>(w).unwrap();
        aw.target = Some(player);
        aw.last_known = Some(me);
        aw.suspicion = 1.0;
        *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
    }
    let start = [a, b].map(|w| app.world().get::<Transform>(w).unwrap().translation);
    for _ in 0..90 * 8 {
        put(&mut app, player, me);
        app.update();
        for w in [a, b] {
            assert!(
                !app.world().entity(w).contains::<super::walker::Flanker>(),
                "a crawler doesn't flank"
            );
        }
    }
    for (w, s) in [a, b].into_iter().zip(start) {
        let now = app.world().get::<Transform>(w).unwrap().translation;
        assert!(
            flat(now - s) < 4.0,
            "it fought from where it was: {s} -> {now}"
        );
    }
}

/// A door you left open 9 m in front of a walker on its rounds: it comes
/// to look.
#[test]
fn a_door_left_open_draws_a_walker() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    let door = zone.on_ground(post.x + 2.0, post.z - 9.0);
    app.world_mut().write_message(super::disturbed::Disturb {
        at: door,
        kind: super::disturbed::SignKind::DoorOpened,
    });
    let mut looked = None;
    for _ in 0..90 * 2 {
        app.update();
        if let Brain::Investigate { at, .. } = *app.world().get::<Brain>(w).unwrap() {
            looked = Some(at);
            break;
        }
    }
    let at = looked.expect("it went to look at the door");
    assert!(
        flat(at - door) < super::disturbed::SIGN_UNCERTAINTY + 1.0,
        "at the door: {at}"
    );
}

/// A sign left long ago only makes a walker wary: it doesn't go to look.
#[test]
fn an_old_sign_only_makes_a_walker_wary() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    let now = app.world().resource::<Time<Fixed>>().elapsed_secs();
    app.world_mut()
        .resource_mut::<super::disturbed::Signs>()
        .leave(
            zone.on_ground(post.x + 2.0, post.z - 9.0),
            super::disturbed::SignKind::Cut,
            now - super::disturbed::SIGN_FRESH * 2.0,
        );
    let mut wary = 0.0f32;
    for _ in 0..90 * 3 {
        app.update();
        wary = wary.max(app.world().get::<Awareness>(w).unwrap().suspicion);
        assert!(
            !matches!(
                app.world().get::<Brain>(w).unwrap(),
                Brain::Investigate { .. }
            ),
            "an old sign doesn't send it to look"
        );
    }
    assert!(wary > 0.15, "but it's wary: {wary:.2}");
}

/// Two fresh signs in a line (a window broken, then a door opened further
/// on): the walker reads the route and goes to look on past the newer.
#[test]
fn signs_in_a_line_give_a_walker_your_route() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    let now = app.world().resource::<Time<Fixed>>().elapsed_secs();
    {
        let mut signs = app.world_mut().resource_mut::<super::disturbed::Signs>();
        signs.leave(
            zone.on_ground(post.x - 5.0, post.z - 10.0),
            super::disturbed::SignKind::Glass,
            now - 60.0,
        );
        signs.leave(
            zone.on_ground(post.x + 3.0, post.z - 10.0),
            super::disturbed::SignKind::DoorOpened,
            now - 10.0,
        );
    }
    let mut looked = None;
    for _ in 0..90 * 2 {
        app.update();
        if let Brain::Investigate { at, .. } = *app.world().get::<Brain>(w).unwrap() {
            looked = Some(at);
            break;
        }
    }
    let at = looked.expect("it went to look");
    assert!(
        at.x - post.x > 3.0 + super::disturbed::ROUTE_LEAD * 0.6,
        "on past the newer, east: {at}"
    );
}

/// What you change is recorded; what a robot does isn't: a fence cut by
/// you is a sign, one cut by anyone else isn't; a door opened with you at
/// it is, one opened with nobody near isn't.
#[test]
fn what_you_disturb_is_recorded() {
    use super::disturbed::{SignKind, Signs};
    let (mut app, player, zone) = app(None);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    app.update();
    let wire = |app: &mut App, at: Vec3| {
        app.world_mut()
            .spawn((
                Transform::from_translation(at),
                GlobalTransform::from_translation(at),
            ))
            .id()
    };
    let mine = wire(&mut app, me + Vec3::X);
    let theirs = wire(&mut app, me + Vec3::new(30.0, 0.0, 0.0));
    let someone = app.world_mut().spawn_empty().id();
    app.world_mut()
        .write_message(crate::destruction::hooks::Cut {
            target: mine,
            by: player,
        });
    app.world_mut()
        .write_message(crate::destruction::hooks::Cut {
            target: theirs,
            by: someone,
        });
    app.update();
    let cuts: Vec<Vec3> = app
        .world()
        .resource::<Signs>()
        .0
        .iter()
        .filter(|s| s.kind == SignKind::Cut)
        .map(|s| s.at)
        .collect();
    assert_eq!(cuts.len(), 1, "yours, not theirs: {cuts:?}");
    assert!(cuts[0].distance(me + Vec3::X) < 0.1);
    // Doors: one at your hand, one far off, both swung open.
    let door = |app: &mut App, at: Vec3| {
        app.world_mut()
            .spawn((
                crate::world::buildings::Door {
                    open: false,
                    angle: 0.0,
                },
                Transform::from_translation(at),
                GlobalTransform::from_translation(at),
            ))
            .id()
    };
    let near = door(&mut app, me + Vec3::Z);
    let far = door(&mut app, me + Vec3::new(0.0, 0.0, 40.0));
    app.update();
    for d in [near, far] {
        app.world_mut()
            .get_mut::<crate::world::buildings::Door>(d)
            .unwrap()
            .open = true;
    }
    app.update();
    let doors: Vec<Vec3> = app
        .world()
        .resource::<Signs>()
        .0
        .iter()
        .filter(|s| s.kind == SignKind::DoorOpened)
        .map(|s| s.at)
        .collect();
    assert_eq!(doors.len(), 1, "the one you opened: {doors:?}");
    assert!(doors[0].distance(me + Vec3::Z) < 0.5);
}

/// A carcass dragged from where it fell (Wildlife's: `Draggable`) leaves
/// a sign where it's got to; a car door dragged along (not a body)
/// doesn't.
#[test]
fn a_carcass_dragged_off_leaves_a_sign() {
    use super::disturbed::{SignKind, Signs};
    let (mut app, player, zone) = app(None);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    app.update();
    let deer = app
        .world_mut()
        .spawn((
            crate::wildlife::bodies::Carcass {
                kind: crate::wildlife::bodies::CarcassKind::Deer,
                at: me,
                owner: Entity::PLACEHOLDER,
                index: 0,
            },
            crate::interact::carry::Bulky,
            crate::interact::carry::Draggable,
        ))
        .id();
    let door = app.world_mut().spawn(crate::interact::carry::Bulky).id();
    let drag = |item: Entity, at: Vec3| crate::interact::carry::Dragged {
        item,
        at,
        speed: 1.0,
        stuff: crate::interact::touch::Stuff::Metal,
        rolling: false,
    };
    let deer_at = me + Vec3::new(3.0, 0.0, 0.0);
    app.world_mut().write_message(drag(deer, deer_at));
    app.world_mut()
        .write_message(drag(door, me + Vec3::new(-6.0, 0.0, 0.0)));
    app.update();
    let dragged: Vec<Vec3> = app
        .world()
        .resource::<Signs>()
        .0
        .iter()
        .filter(|s| s.kind == SignKind::Dragged)
        .map(|s| s.at)
        .collect();
    assert_eq!(dragged.len(), 1, "the body, not the door: {dragged:?}");
    assert!(dragged[0].distance(deer_at) < 0.1);
}

/// Back at the truck, then out into the Zone at `to`: a raid starts.
fn step_out(app: &mut App, player: Entity, to: Vec3) {
    let truck = app
        .world_mut()
        .query_filtered::<&Transform, With<crate::raid::truck::Truck>>()
        .iter(app.world())
        .next()
        .map(|t| t.translation)
        .expect("a truck");
    app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::AtBase;
    put(app, player, truck);
    app.update();
    app.update();
    put(app, player, to);
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<crate::raid::Raid>().phase,
        crate::raid::Phase::Out
    );
}

fn driver_said(app: &App) -> Vec<String> {
    app.world()
        .resource::<crate::raid::Raid>()
        .log
        .iter()
        .map(|(t, _)| t.clone())
        .filter(|t| t.starts_with("Driver:"))
        .collect()
}

fn watchers_near(app: &mut App, at: Vec3) -> usize {
    app.world_mut()
        .query::<(&super::Job, &Robot)>()
        .iter(app.world())
        .filter(|(j, r)| !r.wrecked && j.kind == JobKind::Watch && flat(j.site - at) < 3.0)
        .count()
}

/// A fresh career: stepping out, nothing the Zone does is any different
/// (no watcher, no remark, no door shut, no sentry turned).
#[test]
fn a_fresh_career_is_unaffected_by_memory() {
    let (mut app, player, zone) = app_with(crate::LaunchOptions {
        robots: true,
        ..Default::default()
    });
    assert!(
        app.world()
            .resource::<super::memory::ZoneMind>()
            .memory
            .is_blank()
    );
    let centres = |app: &mut App| {
        app.world_mut()
            .query::<&super::sentry::Sentry>()
            .iter(app.world())
            .map(|s| s.centre)
            .collect::<Vec<f32>>()
    };
    let before = centres(&mut app);
    let robots = |app: &mut App| app.world_mut().query::<&Robot>().iter(app.world()).count();
    let count = robots(&mut app);
    step_out(&mut app, player, zone.on_ground(100.0, 200.0));
    assert!(driver_said(&app).is_empty(), "{:?}", driver_said(&app));
    assert_eq!(centres(&mut app), before, "no sentry turned");
    assert_eq!(robots(&mut app), count, "nobody left watching");
    assert_eq!(
        app.world_mut()
            .query::<&super::memory::ShutByMemory>()
            .iter(app.world())
            .count(),
        0
    );
}

/// A raid is remembered: where you walked, what you wrecked, where you
/// got out.
#[test]
fn a_raid_is_remembered() {
    let (mut app, player, zone) = app(None);
    step_out(&mut app, player, zone.on_ground(100.0, 200.0));
    for k in 0..5 {
        put(
            &mut app,
            player,
            zone.on_ground(100.0 + k as f32 * 20.0, 200.0),
        );
        for _ in 0..100 {
            app.update();
        }
    }
    let w = walker_at(&mut app, &zone, zone.on_ground(150.0, 230.0));
    app.world_mut().write_message(super::PartDestroyed {
        robot: w,
        part: w,
        role: super::PartRole::Core,
        at: zone.on_ground(150.0, 230.0),
    });
    app.update();
    // Out at the first rendezvous.
    let rv = zone
        .map
        .markers_of(&extraction_core::map::MarkerKind::Rendezvous)
        .next()
        .expect("a rendezvous")
        .at;
    app.world_mut().resource_mut::<crate::raid::Raid>().phase =
        crate::raid::Phase::TruckWaiting { rv: 0, left: 100.0 };
    app.update();
    app.world_mut().resource_mut::<crate::raid::Raid>().phase =
        crate::raid::Phase::Extracting { left: 100.0 };
    app.update();
    let m = &app.world().resource::<super::memory::ZoneMind>().memory;
    assert_eq!(m.raids, 1);
    assert!(
        m.corridor(100.0, 200.0) > 0.0 && m.corridor(180.0, 200.0) > 0.0,
        "{:?}",
        m.heat
    );
    assert_eq!(m.corridor(900.0, 900.0), 0.0);
    assert!(
        m.kills
            .iter()
            .any(|k| (k.at[0] - 150.0).abs() < 1.0 && k.kind.eq_ignore_ascii_case("walker")),
        "{:?}",
        m.kills
    );
    assert_eq!(m.rvs.first().map(|r| r.at), Some(rv));
}

/// A valley that remembers you: stepping out, one of its kind is left
/// watching where you wrecked a walker, and the driver says so.
#[test]
fn a_walker_is_left_watching_where_you_killed_one() {
    let (mut app, player, zone) = app_with(crate::LaunchOptions {
        robots: true,
        ..Default::default()
    });
    let spot = zone.on_ground(300.0, 120.0);
    {
        let mut mind = app.world_mut().resource_mut::<super::memory::ZoneMind>();
        for _ in 0..2 {
            let mut t = extraction_core::zone_memory::RaidTrace::default();
            t.kills.push(([spot.x, spot.z], "Walker".into()));
            mind.memory.fold(&t);
        }
    }
    let before = watchers_near(&mut app, spot);
    step_out(&mut app, player, zone.on_ground(100.0, 200.0));
    assert_eq!(
        watchers_near(&mut app, spot),
        before + 1,
        "one left watching"
    );
    let said = driver_said(&app);
    // One of the watcher's lines, whatever place it names.
    let watcher = |s: &String| {
        crate::raid::lines::MEMORY_WATCHER
            .iter()
            .chain(crate::raid::lines::MEMORY_WATCHER_AWAY)
            .any(|l| {
                let (a, b) = l.split_once("{rv}").unwrap_or((l, ""));
                s.starts_with(a) && s.ends_with(b)
            })
    };
    assert!(said.iter().any(watcher), "{said:?}");
    // And only once: stepping out again with the watcher still about
    // leaves one more (the memory is the same), never a crowd at once.
    assert_eq!(said.len(), 1, "{said:?}");
}

/// Your favourite rendezvous: the sentry nearest it turns its sweep
/// toward it, by no more than a quarter of its arc.
#[test]
fn a_sentry_minds_your_favourite_rendezvous() {
    let (mut app, player, zone) = app_with(crate::LaunchOptions {
        robots: true,
        ..Default::default()
    });
    for _ in 0..3 {
        app.update();
    }
    let sentries: Vec<(Entity, Vec3, f32, f32)> = app
        .world_mut()
        .query::<(Entity, &super::sentry::Sentry)>()
        .iter(app.world())
        .filter(|(_, s)| s.powered)
        .map(|(e, s)| (e, s.foot, s.centre, s.arc))
        .collect();
    // A rendezvous with a sentry in reach, if this valley has one.
    let Some((rv, (e, _, home, arc))) = zone
        .map
        .markers_of(&extraction_core::map::MarkerKind::Rendezvous)
        .find_map(|m| {
            sentries
                .iter()
                .filter(|s| {
                    Vec2::new(s.1.x, s.1.z).distance(Vec2::from(m.at)) < super::memory::SENTRY_REACH
                })
                .min_by(|a, b| {
                    let d = |s: &(Entity, Vec3, f32, f32)| {
                        Vec2::new(s.1.x, s.1.z).distance(Vec2::from(m.at))
                    };
                    d(a).total_cmp(&d(b))
                })
                .map(|s| (m.at, *s))
        })
    else {
        eprintln!("no sentry near a rendezvous in this valley: nothing to test");
        return;
    };
    {
        let mut mind = app.world_mut().resource_mut::<super::memory::ZoneMind>();
        for _ in 0..3 {
            let t = extraction_core::zone_memory::RaidTrace {
                rv: Some(rv),
                ..default()
            };
            mind.memory.fold(&t);
        }
    }
    step_out(&mut app, player, zone.on_ground(100.0, 200.0));
    let now = app.world().get::<super::sentry::Sentry>(e).unwrap().centre;
    let foot = app.world().get::<super::sentry::Sentry>(e).unwrap().foot;
    let toward = extraction_core::sentry::bearing([foot.x, foot.z], rv);
    let off = |a: f32| extraction_core::sentry::wrap_deg(toward - a).abs();
    assert!(
        off(now) <= off(home) + 0.01,
        "turned toward it: {home} -> {now} (rv at {toward})"
    );
    assert!(
        extraction_core::sentry::wrap_deg(now - home).abs()
            <= arc * super::memory::SENTRY_LEAN + 0.01,
        "by no more than a quarter of its arc"
    );
}

/// A building you cleared, remembered: stepping out, its doors are shut,
/// and one wedged only if there's another way in.
#[test]
fn a_building_you_cleared_is_found_shut() {
    let (mut app, player, zone) = app(None);
    for _ in 0..3 {
        app.update();
    }
    // A building with two or more doors, and a loot spot in it.
    let found = {
        let entrances = app.world().resource::<super::doors::Entrances>();
        let loot = &app
            .world()
            .resource::<crate::world::buildings::BuildingLoot>()
            .0;
        loot.iter().find_map(|s| {
            let b = entrances.building_at(s.at + Vec3::Y * 0.3)?;
            let ways: Vec<Entity> = entrances
                .doors
                .iter()
                .filter(|d| d.building == b)
                .map(|d| d.door)
                .collect();
            (ways.len() >= 2).then_some((s.at, ways))
        })
    };
    let Some((spot, ways)) = found else {
        eprintln!("no two-door building with loot in this valley: nothing to test");
        return;
    };
    for d in &ways {
        app.world_mut()
            .get_mut::<crate::world::buildings::Door>(*d)
            .unwrap()
            .open = true;
    }
    {
        let mut mind = app.world_mut().resource_mut::<super::memory::ZoneMind>();
        for _ in 0..3 {
            let t = extraction_core::zone_memory::RaidTrace {
                looted: vec![[spot.x, spot.z]],
                ..default()
            };
            mind.memory.fold(&t);
        }
    }
    step_out(&mut app, player, zone.on_ground(100.0, 200.0));
    let shut = ways
        .iter()
        .filter(|d| {
            !app.world()
                .get::<crate::world::buildings::Door>(**d)
                .unwrap()
                .open
        })
        .count();
    let wedged = ways
        .iter()
        .filter(|d| app.world().entity(**d).contains::<super::doors::Wedged>())
        .count();
    assert_eq!(shut, ways.len(), "all its doors shut");
    assert_eq!(wedged, 1, "one wedged, another still a way in");
}

/// A light that wasn't there (a chemlight thrown, a room's lamps on),
/// 18 m off in front of a walker: further than it makes out other signs,
/// it comes to look; one 30 m off it doesn't see.
#[test]
fn a_new_light_draws_a_walker() {
    let looked = |d: f32| {
        let (mut app, player, zone) = app(None);
        put(&mut app, player, zone.on_ground(100.0, 330.0));
        let post = zone.on_ground(100.0, 200.0);
        let w = walker_at(&mut app, &zone, post);
        app.world_mut().write_message(super::disturbed::Disturb {
            at: zone.on_ground(post.x, post.z - d),
            kind: super::disturbed::SignKind::Light,
        });
        for _ in 0..90 * 2 {
            app.update();
            if matches!(
                app.world().get::<Brain>(w).unwrap(),
                Brain::Investigate { .. }
            ) {
                return true;
            }
        }
        false
    };
    assert!(super::disturbed::LIGHT_SIGHT > super::disturbed::SIGN_SIGHT);
    assert!(looked(18.0), "a new light 18 m off draws it");
    assert!(!looked(30.0), "one 30 m off it doesn't see");
}

/// A running pylon at `site`, the director on with nothing else to do,
/// and you standing in plain view of it.
fn network_app() -> (App, Entity, Zone, Entity, Vec3) {
    let (mut app, player, zone) = app(None);
    let site = zone.on_ground(100.0, 210.0);
    let exit = super::director::gate(&zone);
    let mut queue = CommandQueue::default();
    let pylon = {
        let mut c = Commands::new(&mut queue, app.world());
        super::pylon::spawn_pylon(&mut c, site, 999.0, exit)
    };
    queue.apply(app.world_mut());
    {
        let mut p = app
            .world_mut()
            .get_mut::<super::pylon::Pylon>(pylon)
            .unwrap();
        p.progress = 1.0;
        p.state = super::pylon::PylonState::Running { left: 9999.0 };
    }
    {
        let mut d = app.world_mut().resource_mut::<super::director::Director>();
        d.enabled = true;
        d.next_job = 1e9;
        d.next_logistics = 1e9;
    }
    put(
        &mut app,
        player,
        zone.on_ground(site.x + 25.0, site.z + 10.0),
    );
    app.update();
    (app, player, zone, pylon, site)
}

fn responses(app: &App) -> Vec<Vec3> {
    app.world()
        .resource::<super::director::Director>()
        .responses
        .iter()
        .map(|r| r.1)
        .collect()
}

fn cable_of(app: &mut App, pylon: Entity) -> Option<Entity> {
    app.world_mut()
        .query::<(Entity, &super::network::PylonCable)>()
        .iter(app.world())
        .find(|(_, c)| c.pylon == pylon)
        .map(|(e, _)| e)
}

/// Cut a pylon's cable: it sees you, and nobody comes.
#[test]
fn a_cut_cable_silences_a_pylon() {
    let (mut app, player, _zone, pylon, _) = network_app();
    let cable = cable_of(&mut app, pylon).expect("a running pylon has a cable");
    app.world_mut()
        .write_message(crate::destruction::hooks::Cut {
            target: cable,
            by: player,
        });
    for _ in 0..90 * 10 {
        app.update();
    }
    assert!(
        app.world().get::<Awareness>(pylon).unwrap().can_see_target,
        "it sees you"
    );
    assert!(
        responses(&app).is_empty(),
        "but it's silent: {:?}",
        responses(&app)
    );
}

/// A silent pylon is noticed: a mender comes, and once its cabinet's
/// welded whole it's wired up and reporting again.
#[test]
fn a_silent_pylon_gets_a_mender_and_comes_back() {
    let (mut app, player, _zone, pylon, _) = network_app();
    let cable = cable_of(&mut app, pylon).unwrap();
    app.world_mut()
        .write_message(crate::destruction::hooks::Cut {
            target: cable,
            by: player,
        });
    for _ in 0..(90.0 * (super::network::SILENCE_NOTICED + 2.0)) as usize {
        app.update();
    }
    let menders = app
        .world_mut()
        .query::<&super::mender::Mender>()
        .iter(app.world())
        .count();
    assert!(menders >= 1, "a mender's been sent");
    // Welded whole (as the mender would).
    {
        let mut q = app.world_mut().query::<&mut super::RobotPart>();
        for mut p in q.iter_mut(app.world_mut()) {
            if p.robot == pylon && p.role == super::PartRole::Core {
                p.hp = p.max_hp;
            }
        }
    }
    app.update();
    app.update();
    assert!(
        !app.world()
            .entity(pylon)
            .contains::<super::network::Silenced>()
    );
    assert!(cable_of(&mut app, pylon).is_some(), "wired up again");
    let mut reported = false;
    for _ in 0..90 * 10 {
        app.update();
        if !responses(&app).is_empty() {
            reported = true;
            break;
        }
    }
    assert!(reported, "and it reports you again");
}

/// A salvaged radio let go against a pylon's cabinet is spliced in: it
/// reports contact somewhere far from you, and the radio's gone into it.
#[test]
fn a_spliced_radio_sends_them_elsewhere() {
    let (mut app, player, _zone, pylon, site) = network_app();
    // Out of its sight while you work on it from behind (so its own true
    // report doesn't muddle this one).
    app.world_mut()
        .get_mut::<super::Senses>(pylon)
        .unwrap()
        .sight = 0.1;
    let me = site + Vec3::new(1.0, 0.0, 0.0);
    put(&mut app, player, me);
    let radio = spawn_radio(&mut app, site + Vec3::new(0.7, 0.8, 0.0));
    app.world_mut()
        .write_message(crate::interact::InteractEvent::Released {
            item: radio,
            side: crate::rig::Side::Left,
            speed: 0.0,
        });
    app.update();
    app.update();
    assert!(
        app.world().get_entity(radio).is_err(),
        "the radio's spliced in"
    );
    let r = responses(&app);
    assert_eq!(r.len(), 1, "one false report: {r:?}");
    assert!(
        flat(r[0] - me) >= super::network::DECOY_FROM - 1.0,
        "somewhere you aren't: {} m off",
        flat(r[0] - me)
    );
}

fn spawn_radio(app: &mut App, at: Vec3) -> Entity {
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let mut queue = CommandQueue::default();
    let e = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind("radio").unwrap(),
                transform: Transform::from_translation(at),
                condition: 1.0,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    e
}

/// The same pylon feeding a second false report isn't believed: no team
/// goes, it's distrusted (even a true report of it is ignored), and a
/// walker is sent to look it over.
#[test]
fn a_second_false_report_gets_a_pylon_distrusted() {
    let (mut app, player, _zone, pylon, site) = network_app();
    app.world_mut()
        .get_mut::<super::Senses>(pylon)
        .unwrap()
        .sight = 0.1;
    put(&mut app, player, site + Vec3::new(1.0, 0.0, 0.0));
    for _ in 0..2 {
        let radio = spawn_radio(&mut app, site + Vec3::new(0.7, 0.8, 0.0));
        app.world_mut()
            .write_message(crate::interact::InteractEvent::Released {
                item: radio,
                side: crate::rig::Side::Left,
                speed: 0.0,
            });
        app.update();
        app.update();
    }
    assert_eq!(responses(&app).len(), 1, "only the first was believed");
    assert!(
        app.world()
            .resource::<super::director::Director>()
            .net
            .distrusted
            .contains(&pylon)
    );
    assert_eq!(
        watchers_near(&mut app, zone_post(site)),
        1,
        "someone's coming to look it over"
    );
    // Now it sees you for real, and still nobody comes for that.
    app.world_mut()
        .get_mut::<super::Senses>(pylon)
        .unwrap()
        .sight = 85.0;
    put(&mut app, player, site + Vec3::new(25.0, 0.0, 10.0));
    for _ in 0..90 * 10 {
        app.update();
    }
    assert_eq!(responses(&app).len(), 1, "a distrusted pylon's ignored");
}

/// Where the inspector stands: by the relay.
fn zone_post(site: Vec3) -> Vec3 {
    site + Vec3::new(3.0, 0.0, 3.0)
}

/// A powered jammer by a pylon: what it sees doesn't get out. But the
/// quiet's a sign: before long a team comes to find the jammer.
#[test]
fn a_jammer_blocks_reports_but_is_noticed() {
    let (mut app, _player, _zone, _pylon, site) = network_app();
    let jam = site + Vec3::new(-20.0, 0.5, 5.0);
    app.world_mut().spawn((
        super::network::Jammer { radius: 40.0 },
        crate::gadgets::Powered,
        Transform::from_translation(jam),
        GlobalTransform::from_translation(jam),
    ));
    for _ in 0..90 * 10 {
        app.update();
    }
    assert!(responses(&app).is_empty(), "jammed: {:?}", responses(&app));
    let mut found = None;
    for _ in 0..(90.0 * super::network::JAM_NOTICED) as usize {
        app.update();
        if let Some(r) = responses(&app).first() {
            found = Some(*r);
            break;
        }
    }
    let r = found.expect("they come to find the jam");
    assert!(
        flat(r - jam) <= super::network::JAM_GUESS,
        "roughly where it is: {} m off",
        flat(r - jam)
    );
}

/// The bench's jammer is a jammer: a device that draws hard from its cell
/// and, switched on with power, blocks the relays round it.
#[test]
fn a_jammer_item_is_a_jammer() {
    let (mut app, _player, zone) = app(None);
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let mut queue = CommandQueue::default();
    let e = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind("jammer").expect("a jammer in the catalog"),
                transform: Transform::from_translation(zone.on_ground(100.0, 200.0) + Vec3::Y),
                condition: 1.0,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    let j = app
        .world()
        .get::<super::network::Jammer>(e)
        .expect("it jams");
    assert_eq!(j.radius, super::network::JAM_RADIUS);
    let d = app
        .world()
        .get::<crate::gadgets::Device>(e)
        .expect("a device");
    assert_eq!(d.kind, crate::gadgets::DeviceKind::Jammer);
    assert!(d.watts >= 15.0, "battery-hungry: {} W", d.watts);
    assert!(
        extraction_core::fabrication::CONTRACT_RECIPES
            .iter()
            .any(|r| r.makes == extraction_core::fabrication::Make::Items(&[("jammer", 1)])),
        "a taught bench recipe makes it"
    );
}

/// A device switched on with a charged cell in it, set down at `at`.
fn powered_device(app: &mut App, id: &str, at: Vec3) -> Entity {
    let dev = spawn_item_at(app, id, at);
    let cell = spawn_item_at(app, "power_cell", at + Vec3::Y * 0.3);
    app.update();
    let socket = app
        .world_mut()
        .query::<(
            Entity,
            &crate::interact::Socket,
            &crate::interact::SocketOwner,
        )>()
        .iter(app.world())
        .find(|(_, s, o)| {
            o.0 == dev
                && s.kind == crate::interact::SocketKind::Mount(extraction_core::items::Mount::Cell)
        })
        .map(|(e, ..)| e)
        .expect("a cell bay");
    let mut q =
        bevy::ecs::system::SystemState::<(Commands, Query<&mut crate::interact::Socket>)>::new(
            app.world_mut(),
        );
    let (mut c, mut sockets) = q.get_mut(app.world_mut()).unwrap();
    let mut s = sockets.get_mut(socket).unwrap();
    crate::interact::seat_in_socket(&mut c, cell, socket, &mut s);
    q.apply(app.world_mut());
    app.world_mut()
        .entity_mut(dev)
        .insert(crate::items::usage::LightOn);
    for _ in 0..5 {
        app.update();
    }
    assert!(
        app.world()
            .entity(dev)
            .contains::<crate::gadgets::Powered>(),
        "{id} is running"
    );
    dev
}

fn spawn_item_at(app: &mut App, id: &str, at: Vec3) -> Entity {
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let mut queue = CommandQueue::default();
    let e = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind(id).unwrap(),
                transform: Transform::from_translation(at),
                condition: 1.0,
            },
        )
    };
    queue.apply(app.world_mut());
    e
}

fn noticed(app: &App) -> u32 {
    app.world()
        .get_resource::<crate::bot::Counters>()
        .and_then(|c| c.0.get(super::devices::NOTICED).copied())
        .unwrap_or(0)
}

/// A light switched on and set down in front of a walker: it notices it
/// (the tell) and comes to look.
#[test]
fn a_set_down_light_draws_a_walker_to_look() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let post = zone.on_ground(100.0, 200.0);
    let w = walker_at(&mut app, &zone, post);
    let at = zone.on_ground(post.x + 2.0, post.z - 18.0) + Vec3::Y * 0.2;
    powered_device(&mut app, "flashlight", at);
    let mut looked = None;
    for _ in 0..90 * 3 {
        app.update();
        if let Brain::Investigate { at: a, .. } = *app.world().get::<Brain>(w).unwrap() {
            looked = Some(a);
            break;
        }
    }
    let a = looked.expect("it went to look at the light");
    assert!(flat(a - at) < 4.0, "at the light: {a}");
    assert!(noticed(&app) >= 1, "and the tell went out");
}

/// A running jammer: once it's run a while, a drone's sensors pick up its
/// draw from well off; it hunts it, and shoots it to pieces.
#[test]
fn a_running_jammer_is_hunted_and_shot() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let at = zone.on_ground(100.0, 200.0) + Vec3::Y * 0.2;
    let jam = powered_device(&mut app, "jammer", at);
    let site = at + Vec3::new(0.0, 12.0, 40.0);
    let mut queue = CommandQueue::default();
    {
        let mut c = Commands::new(&mut queue, app.world());
        super::drone::spawn_drone(
            &mut c,
            site,
            Job {
                kind: JobKind::Survey,
                site,
                duration: 999.0,
                exit: site,
            },
        );
    }
    queue.apply(app.world_mut());
    let mut gone = false;
    for _ in 0..(90.0 * (super::devices::EM_SETTLE + 40.0)) as usize {
        app.update();
        if app.world().get_entity(jam).is_err() {
            gone = true;
            break;
        }
    }
    assert!(noticed(&app) >= 1, "the drone picked it up");
    assert!(gone, "and shot it to pieces");
}

/// An armed EMP trap: a walker spots its lamp and shoots it from outside
/// its reach; it goes off, and the walker isn't caught in it.
#[test]
fn an_armed_trap_is_shot_from_outside_its_reach() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let post = zone.on_ground(100.0, 200.0);
    let mut queue = CommandQueue::default();
    let w = {
        let mut c = Commands::new(&mut queue, app.world());
        super::walker::spawn_walker(
            &mut c,
            &zone,
            post,
            Job {
                kind: JobKind::Watch,
                site: post,
                duration: 999.0,
                exit: post,
            },
        )
    };
    queue.apply(app.world_mut());
    for _ in 0..60 {
        app.update();
    }
    let at = zone.on_ground(post.x, post.z - 13.0) + Vec3::Y * 0.1;
    let trap = spawn_item_at(&mut app, "emp_trap", at);
    app.world_mut()
        .entity_mut(trap)
        .insert(crate::gadgets::Armed::Trap { radius: 4.0 });
    #[derive(Resource, Default)]
    struct Blasts(u32);
    app.init_resource::<Blasts>().add_systems(
        Last,
        |mut b: MessageReader<crate::gadgets::EmpBlast>, mut n: ResMut<Blasts>| {
            n.0 += b.read().count() as u32
        },
    );
    let mut blown = false;
    for _ in 0..90 * 20 {
        app.update();
        if app.world().resource::<Blasts>().0 > 0 {
            blown = true;
            break;
        }
    }
    assert!(blown, "it shot the trap and it went off");
    app.update();
    assert!(
        !app.world().entity(w).contains::<super::Stunned>(),
        "from outside its reach"
    );
}

/// An unarmed relay picks a running cloak's draw up, and reports it: a
/// team is sent.
#[test]
fn a_relay_reports_a_hungry_device() {
    let (mut app, player, zone, pylon, site) = network_app();
    put(
        &mut app,
        player,
        zone.on_ground(site.x + 300.0, site.z + 300.0),
    );
    app.world_mut()
        .get_mut::<super::Senses>(pylon)
        .unwrap()
        .sight = 0.1;
    let at = zone.on_ground(site.x + 30.0, site.z) + Vec3::Y * 0.2;
    powered_device(&mut app, "cloak_generator", at);
    for _ in 0..(90.0 * (super::devices::EM_SETTLE + 3.0)) as usize {
        app.update();
    }
    let r = responses(&app);
    assert!(!r.is_empty(), "it reported the cloak");
    assert!(flat(r[0] - at) < 2.0, "where it is: {:?}", r);
}

/// Sprinting across a walker's line of fire throws its aim off: its
/// rounds go wide (CROSSING_SPREAD), not one a second into you.
#[test]
fn a_sprint_across_its_line_throws_a_walker_off() {
    use crate::weapons::ballistics::Shot;
    #[derive(Resource, Default)]
    struct Spreads(Vec<f32>);
    let (mut app, player, zone) = app(None);
    app.init_resource::<Spreads>().add_systems(
        FixedPostUpdate,
        |mut s: MessageReader<Shot>, mut out: ResMut<Spreads>| {
            out.0
                .extend(s.read().filter(|s| s.shooter.is_some()).map(|s| s.spread));
        },
    );
    put(&mut app, player, zone.on_ground(100.0, 400.0));
    let me = zone.on_ground(100.0, 200.0);
    let w = armed_walker_at(&mut app, &zone, zone.on_ground(100.0, 225.0));
    put(&mut app, player, me);
    {
        let mut a = app.world_mut().get_mut::<Awareness>(w).unwrap();
        a.target = Some(player);
        a.last_known = Some(me);
        a.suspicion = 1.0;
    }
    *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
    for _ in 0..90 * 6 {
        // Running across its line (it's to the north, you go east).
        put(&mut app, player, me);
        app.world_mut()
            .get_mut::<crate::player::Body>(player)
            .unwrap()
            .velocity = Vec3::new(5.0, 0.0, 0.0);
        app.update();
    }
    let s = &app.world().resource::<Spreads>().0;
    assert!(!s.is_empty(), "it fired");
    assert!(
        s.iter()
            .all(|x| *x >= 0.025 + super::walker::CROSSING_SPREAD * 4.5),
        "wide of a sprint: {s:?}"
    );
}

/// A drone on its way home that sees you spares a burst or two, then goes
/// on home: it doesn't hang over you.
#[test]
fn a_leaving_drone_spares_only_a_burst() {
    let (mut app, player, zone) = app(None);
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let site = me + Vec3::new(0.0, 14.0, 20.0);
    let exit = zone.on_ground(400.0, 400.0);
    let mut queue = CommandQueue::default();
    let d = {
        let mut c = Commands::new(&mut queue, app.world());
        super::drone::spawn_drone(
            &mut c,
            site,
            Job {
                kind: JobKind::Survey,
                site,
                duration: 999.0,
                exit,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    *app.world_mut().get_mut::<Brain>(d).unwrap() = Brain::Leave { to: exit };
    {
        let mut a = app.world_mut().get_mut::<Awareness>(d).unwrap();
        a.target = Some(player);
        a.last_known = Some(me);
        a.suspicion = 1.0;
        a.can_see_target = true;
    }
    let mut engaged = false;
    let mut home_again = false;
    for _ in 0..(90.0 * (super::drone::LEAVING_FIGHT + 4.0)) as usize {
        put(&mut app, player, me);
        app.update();
        let Ok(b) = app.world().get_entity(d).map(|e| e.get::<Brain>().cloned()) else {
            break;
        };
        match b {
            Some(Brain::Engage) => engaged = true,
            Some(Brain::Leave { .. }) if engaged => home_again = true,
            _ => {}
        }
        if home_again {
            break;
        }
    }
    assert!(engaged, "it turned on you");
    assert!(home_again, "then went on home");
}

fn marks(app: &mut App) -> Vec<super::recall::SearchMark> {
    app.world_mut()
        .query::<&super::recall::SearchMark>()
        .iter(app.world())
        .copied()
        .collect()
}

fn searched_by(app: &mut App, robot: Entity, at: Vec3) {
    app.world_mut()
        .write_message(super::search::Searching { robot, at });
    app.update();
    app.update();
}

/// A walker that searches leaves its paint: fresh while it won't answer a
/// lure there, dull after; searched again close by, the same mark is
/// renewed. A drone searching leaves none.
#[test]
fn a_walker_that_searched_leaves_its_mark() {
    use super::recall::{Memory, lasts};
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let spot = zone.on_ground(100.0, 200.0);
    let walker = walker_at(&mut app, &zone, spot);
    out_raid(&mut app);
    searched_by(&mut app, walker, spot);
    let m = marks(&mut app);
    assert_eq!(m.len(), 1);
    assert!(
        (m[0].at.y - spot.y).abs() < 0.3,
        "on the ground: {:?}",
        m[0]
    );
    let now = app.world().resource::<Time>().elapsed_secs();
    assert!(m[0].fresh(now));
    assert!(
        !m[0].fresh(now + lasts(Memory::Lure)),
        "dull once a lure works again"
    );
    assert!(m[0].paint(now + lasts(Memory::Lure)) < m[0].paint(now));
    searched_by(&mut app, walker, spot + Vec3::new(1.5, 0.0, 0.0));
    assert_eq!(marks(&mut app).len(), 1, "renewed, not another");
    let mut queue = CommandQueue::default();
    let drone = {
        let mut c = Commands::new(&mut queue, app.world());
        super::drone::spawn_drone(
            &mut c,
            spot + Vec3::new(30.0, 15.0, 0.0),
            Job {
                kind: JobKind::Survey,
                site: spot,
                duration: 999.0,
                exit: spot,
            },
        )
    };
    queue.apply(app.world_mut());
    searched_by(&mut app, drone, spot + Vec3::new(30.0, 0.0, 0.0));
    assert_eq!(marks(&mut app).len(), 1, "a drone has no paint");
    assert!(
        !app.world()
            .resource::<super::memory::ZoneMind>()
            .trace
            .searched
            .is_empty(),
        "and the valley will remember"
    );
}

/// The raid as if out (nothing else changed).
fn out_raid(app: &mut App) {
    app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
    app.update();
}

/// Last raid's marks are there when you next step out, faded; a few
/// raids on, they're gone.
#[test]
fn last_raids_marks_are_there_next_raid_and_fade() {
    let (mut app, player, zone) = app(None);
    let spot = zone.on_ground(100.0, 200.0);
    {
        let mut mind = app.world_mut().resource_mut::<super::memory::ZoneMind>();
        let t = extraction_core::zone_memory::RaidTrace {
            searched: vec![[spot.x, spot.z]],
            ..default()
        };
        mind.memory.fold(&t);
    }
    step_out(&mut app, player, zone.on_ground(100.0, 330.0));
    let m = marks(&mut app);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(flat(m[0].at - spot) < 0.5 && (m[0].at.y - spot.y).abs() < 0.5);
    let now = app.world().resource::<Time>().elapsed_secs();
    assert!(!m[0].fresh(now), "last raid's: not fresh");
    let first = m[0].paint(now);
    {
        let mut mind = app.world_mut().resource_mut::<super::memory::ZoneMind>();
        mind.memory.fold(&default());
        mind.trace = default();
    }
    step_out(&mut app, player, zone.on_ground(100.0, 330.0));
    let m = marks(&mut app);
    assert_eq!(m.len(), 1, "one, not two: {m:?}");
    assert!(m[0].paint(now) < first, "fainter a raid on");
    {
        let mut mind = app.world_mut().resource_mut::<super::memory::ZoneMind>();
        for _ in 0..4 {
            mind.memory.fold(&default());
        }
        mind.trace = default();
    }
    step_out(&mut app, player, zone.on_ground(100.0, 330.0));
    assert!(marks(&mut app).is_empty(), "forgotten");
}

/// A pylon that reports you keeps a tally, one per report; it's let go
/// once it's been quiet long enough. A false report it's fed isn't one.
#[test]
fn a_relay_that_reported_you_keeps_a_tally() {
    use super::recall::{Filed, Memory, lasts};
    let (mut app, player, _zone, pylon, site) = network_app();
    put(&mut app, player, site + Vec3::new(25.0, 0.0, 10.0));
    let mut filed = None;
    for _ in 0..90 * 10 {
        app.update();
        filed = app.world().get::<Filed>(pylon).copied();
        if filed.is_some() {
            break;
        }
    }
    let f = filed.expect("it reported you, and keeps the tally");
    assert_eq!(f.count, 1);
    // Another report (its cooldown spent).
    app.world_mut()
        .get_mut::<super::pylon::Pylon>(pylon)
        .unwrap()
        .report_cooldown = 0.0;
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(app.world().get::<Filed>(pylon).unwrap().count, 2);
    // Out of its sight, and quiet long enough: let go.
    put(&mut app, player, site + Vec3::new(400.0, 0.0, 400.0));
    app.world_mut().get_mut::<Filed>(pylon).unwrap().last -= lasts(Memory::Reported);
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().get::<Filed>(pylon).is_none());
    // A spliced radio's false report: no tally.
    let radio = spawn_radio(&mut app, site + Vec3::new(0.7, 0.8, 0.0));
    app.world_mut()
        .write_message(crate::interact::InteractEvent::Released {
            item: radio,
            side: crate::rig::Side::Left,
            speed: 0.0,
        });
    for _ in 0..3 {
        app.update();
    }
    assert!(
        app.world().get::<Filed>(pylon).is_none(),
        "a decoy's no tally"
    );
}

/// A relay distrusted for a second false report is believed again once
/// the last is old enough (`recall::Memory::Distrust`).
#[test]
fn distrust_wears_off() {
    use super::recall::{Memory, lasts};
    let (mut app, player, _zone, pylon, site) = network_app();
    put(&mut app, player, site + Vec3::new(400.0, 0.0, 400.0));
    for _ in 0..2 {
        let radio = spawn_radio(&mut app, site + Vec3::new(0.7, 0.8, 0.0));
        app.world_mut()
            .write_message(crate::interact::InteractEvent::Released {
                item: radio,
                side: crate::rig::Side::Left,
                speed: 0.0,
            });
        app.update();
        app.update();
    }
    let distrusted = |app: &App| {
        app.world()
            .resource::<super::director::Director>()
            .net
            .distrusted
            .contains(&pylon)
    };
    for _ in 0..5 {
        app.update();
    }
    assert!(distrusted(&app), "distrusted, and still a few ticks on");
    app.world_mut()
        .get_mut::<super::network::Relay>(pylon)
        .unwrap()
        .last_false -= lasts(Memory::Distrust);
    app.update();
    app.update();
    assert!(!distrusted(&app), "believed again");
}

#[derive(Resource, Default)]
struct Calls(Vec<super::squad::SquadCall>);

fn squad_app(n: usize, drone: bool) -> (App, Entity, Zone, Vec3, Vec<Entity>) {
    squad_app_with(n, drone, false)
}

/// Out on a raid, carrying a radio if `radio`, with `n` walkers (and a
/// drone) fighting you.
fn squad_app_with(n: usize, drone: bool, radio: bool) -> (App, Entity, Zone, Vec3, Vec<Entity>) {
    let (mut app, player, zone) = app(None);
    app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
    // No plan while they're set down: it starts when the test does.
    app.world_mut()
        .resource_mut::<super::squad::Coordination>()
        .on = false;
    if radio {
        // In the pack on your back.
        let r = spawn_radio(&mut app, zone.on_ground(100.0, 200.0) + Vec3::Y);
        let bag = app
            .world_mut()
            .query::<(Entity, &crate::interact::body::Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == player && b.carried)
            .map(|(e, _)| e)
            .expect("a pack on your back");
        app.world_mut()
            .entity_mut(r)
            .insert(crate::interact::body::InBackpack { bag });
        app.update();
        assert!(
            app.world()
                .get::<crate::anomaly::artifacts::CarriedBy>(r)
                .is_some()
        );
    }
    app.init_resource::<Calls>().add_systems(
        Last,
        |mut m: MessageReader<super::squad::SquadCall>, mut s: ResMut<Calls>| {
            s.0.extend(m.read().copied())
        },
    );
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let mut fighters: Vec<Entity> = (0..n)
        .map(|k| {
            let x = 96.0 + k as f32 * 4.0;
            walker_at(&mut app, &zone, zone.on_ground(x, 225.0))
        })
        .collect();
    if drone {
        let mut queue = CommandQueue::default();
        let d = {
            let mut c = Commands::new(&mut queue, app.world());
            super::drone::spawn_drone(
                &mut c,
                me + Vec3::new(0.0, 18.0, 20.0),
                Job {
                    kind: JobKind::Survey,
                    site: me,
                    duration: 999.0,
                    exit: me,
                },
            )
        };
        queue.apply(app.world_mut());
        app.update();
        fighters.push(d);
    }
    for w in &fighters {
        let mut aw = app.world_mut().get_mut::<Awareness>(*w).unwrap();
        aw.target = Some(player);
        aw.last_known = Some(me);
        aw.suspicion = 1.0;
        *app.world_mut().get_mut::<Brain>(*w).unwrap() = Brain::Engage;
    }
    app.world_mut()
        .resource_mut::<super::squad::Coordination>()
        .on = true;
    app.world_mut().resource_mut::<Calls>().0.clear();
    (app, player, zone, me, fighters)
}

fn role_of(app: &App, e: Entity) -> Option<super::squad::Role> {
    app.world()
        .get::<super::squad::SquadRole>(e)
        .map(|r| r.role)
}

/// Three walkers on you: one pins, one flanks, and one makes for the way
/// to your rendezvous and gets between you and it. Each says its part
/// once.
#[test]
fn three_walkers_hold_flank_and_cut_you_off() {
    use super::squad::Role;
    let (mut app, player, zone, me, w) = squad_app(3, false);
    let rv = super::squad::your_rendezvous(&zone, None, me).expect("a rendezvous");
    let toward = Vec2::new(rv.x - me.x, rv.z - me.z).normalize();
    let mut ahead: f32 = f32::MIN;
    let mut cutter = None;
    for _ in 0..90 * 30 {
        put(&mut app, player, me);
        app.update();
        if cutter.is_none() {
            cutter = w
                .iter()
                .copied()
                .find(|e| role_of(&app, *e) == Some(Role::CutOff));
        }
        if let Some(c) = cutter {
            let t = app.world().get::<Transform>(c).unwrap().translation;
            ahead = ahead.max(Vec2::new(t.x - me.x, t.z - me.z).dot(toward));
        }
    }
    let roles: Vec<Option<Role>> = w.iter().map(|e| role_of(&app, *e)).collect();
    assert!(roles.contains(&Some(Role::Pin)), "{roles:?}");
    assert!(roles.contains(&Some(Role::Flank)), "{roles:?}");
    assert!(cutter.is_some(), "{roles:?}");
    assert!(
        ahead > 10.0,
        "the cut-off got out between you and the rendezvous: {ahead:.1} m toward it"
    );
    let calls = &app.world().resource::<Calls>().0;
    for role in [Role::Pin, Role::Flank, Role::CutOff] {
        assert_eq!(
            calls.iter().filter(|c| c.role == role).count(),
            1,
            "{role:?} said once: {calls:?}"
        );
    }
}

/// With a drone among them it spots for the flanker: over its way in,
/// with its light down on it.
#[test]
fn a_drone_lights_the_flankers_way() {
    use super::squad::Role;
    let (mut app, player, _zone, me, w) = squad_app(2, true);
    let drone = *w.last().unwrap();
    let mut closest = f32::MAX;
    for _ in 0..90 * 12 {
        put(&mut app, player, me);
        app.update();
        if let Some(r) = app.world().get::<super::squad::SquadRole>(drone) {
            let at = app.world().get::<Transform>(drone).unwrap().translation;
            closest = closest.min(Vec2::new(at.x - r.post.x, at.z - r.post.z).length());
        }
    }
    assert_eq!(role_of(&app, drone), Some(Role::Spot));
    assert!(
        closest < 6.0,
        "over the flanker's way in: {closest:.1} m off"
    );
    let lights = app
        .world_mut()
        .query::<&super::squad::SpotterLight>()
        .iter(app.world())
        .filter(|l| l.drone == drone)
        .count();
    assert_eq!(lights, 1, "its light on");
}

/// A radio you carry says it: which way the flanker's coming round.
/// Without one, nothing's said.
#[test]
fn your_radio_says_where_the_flanker_goes() {
    let run = |radio: bool| {
        let (mut app, player, _zone, me, _w) = squad_app_with(2, false, radio);
        for _ in 0..90 * 3 {
            put(&mut app, player, me);
            app.update();
        }
        let flank = app
            .world()
            .resource::<Calls>()
            .0
            .iter()
            .find(|c| c.role == super::squad::Role::Flank)
            .copied()
            .expect("a flank call");
        let way =
            crate::raid::lines::bearing_word(Vec2::new(flank.post.x - me.x, flank.post.z - me.z));
        let said: Vec<String> = app
            .world()
            .resource::<crate::raid::Raid>()
            .log
            .iter()
            .map(|(t, _)| t.clone())
            .collect();
        (said, way)
    };
    let (said, way) = run(true);
    assert!(
        said.iter().any(|l| l.contains(way) && l.contains("round")),
        "the flanker's way, {way}: {said:?}"
    );
    let (said, _) = run(false);
    assert!(
        !said.iter().any(|l| l.contains("round")),
        "no radio, no word: {said:?}"
    );
}

/// With coordination off (the sims' other arm), they fight alone: no
/// roles, no calls, nobody flanks.
#[test]
fn uncoordinated_they_fight_alone() {
    let (mut app, player, _zone, me, w) = squad_app(3, false);
    app.world_mut()
        .resource_mut::<super::squad::Coordination>()
        .on = false;
    for _ in 0..90 * 5 {
        put(&mut app, player, me);
        app.update();
    }
    assert!(w.iter().all(|e| role_of(&app, *e).is_none()));
    assert!(app.world().resource::<Calls>().0.is_empty());
    assert!(
        w.iter()
            .all(|e| !app.world().entity(*e).contains::<super::walker::Flanker>())
    );
}

/// A pane a machine breaks going through is its own doing: no sign, and
/// it doesn't turn to its own crash. One broken with no machine at it is
/// a sign.
#[test]
fn what_a_machine_breaks_is_no_sign_of_you() {
    let (mut app, player, zone) = app(None);
    put(&mut app, player, zone.on_ground(100.0, 330.0));
    let at = zone.on_ground(100.0, 200.0) + Vec3::Y;
    let w = walker_at(&mut app, &zone, at - Vec3::Y);
    let broke = |app: &mut App, at: Vec3| {
        app.world_mut().write_message(crate::destruction::Broke {
            at,
            stuff: crate::destruction::Stuff::Glass,
            size: 1.0,
        });
        app.update();
        app.update();
        app.world().resource::<super::disturbed::Signs>().0.len()
    };
    assert_eq!(broke(&mut app, at), 0, "the walker's own");
    assert!(
        app.world()
            .get::<Awareness>(w)
            .unwrap()
            .last_known
            .is_none(),
        "and it didn't hear it as anyone's"
    );
    assert_eq!(
        broke(&mut app, at + Vec3::new(30.0, 0.0, 0.0)),
        1,
        "someone's"
    );
}

/// A walker on its way to a flank holds its fire, however well it sees
/// you; at its post, it opens up.
#[test]
fn a_flanker_on_its_way_holds_its_fire() {
    use crate::weapons::ballistics::Shot;
    #[derive(Resource, Default)]
    struct Fired(Vec<Entity>);
    let (mut app, player, zone) = app(None);
    app.init_resource::<Fired>().add_systems(
        Last,
        |mut s: MessageReader<Shot>, mut f: ResMut<Fired>| {
            f.0.extend(s.read().filter_map(|s| s.shooter))
        },
    );
    app.world_mut()
        .resource_mut::<super::squad::Coordination>()
        .on = false;
    let me = zone.on_ground(100.0, 200.0);
    put(&mut app, player, me);
    let at = zone.on_ground(100.0, 222.0);
    let mut queue = CommandQueue::default();
    let w = {
        let mut c = Commands::new(&mut queue, app.world());
        super::walker::spawn_walker(
            &mut c,
            &zone,
            at,
            Job {
                kind: JobKind::Watch,
                site: at,
                duration: 999.0,
                exit: at,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    let engage = |app: &mut App| {
        let mut aw = app.world_mut().get_mut::<Awareness>(w).unwrap();
        aw.target = Some(player);
        aw.last_known = Some(me);
        aw.suspicion = 1.0;
        *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
    };
    engage(&mut app);
    let run = |app: &mut App, post: Option<Vec3>, secs: f32| {
        app.world_mut().resource_mut::<Fired>().0.clear();
        for _ in 0..(90.0 * secs) as usize {
            put(app, player, me);
            let here = app.world().get::<Transform>(w).unwrap().translation;
            app.world_mut()
                .entity_mut(w)
                .insert(super::squad::SquadRole {
                    role: super::squad::Role::Flank,
                    target: player,
                    post: post.unwrap_or(here),
                });
            app.update();
        }
        app.world()
            .resource::<Fired>()
            .0
            .iter()
            .filter(|e| **e == w)
            .count()
    };
    // Its post far off, round your side: it goes, and holds its fire.
    let away = zone.on_ground(140.0, 200.0);
    assert_eq!(
        run(&mut app, Some(away), 4.0),
        0,
        "held its fire on the way"
    );
    // There (its post where it stands): it fires.
    engage(&mut app);
    assert!(run(&mut app, None, 6.0) > 0, "and fires from its post");
}

/// While a squadmate moves up, the pinner suppresses: more rounds, on
/// where it last saw you even once you're out of sight. Its rounds count
/// the cadence; nothing moving up, it fires at its usual rate.
#[test]
fn the_pinner_suppresses_while_one_moves_up() {
    use crate::weapons::ballistics::Shot;
    #[derive(Resource, Default)]
    struct Fired(Vec<(Entity, Vec3)>);
    let run = |covering: bool| {
        let (mut app, player, zone) = app(None);
        app.init_resource::<Fired>().add_systems(
            Last,
            |mut s: MessageReader<Shot>, mut f: ResMut<Fired>| {
                f.0.extend(s.read().filter_map(|s| Some((s.shooter?, s.direction))))
            },
        );
        app.world_mut()
            .resource_mut::<super::squad::Coordination>()
            .on = false;
        let me = zone.on_ground(100.0, 200.0);
        put(&mut app, player, me);
        let at = zone.on_ground(100.0, 222.0);
        let mut queue = CommandQueue::default();
        let (w, mate) = {
            let mut c = Commands::new(&mut queue, app.world());
            let job = Job {
                kind: JobKind::Watch,
                site: at,
                duration: 999.0,
                exit: at,
            };
            (
                super::walker::spawn_walker(&mut c, &zone, at, job.clone()),
                super::walker::spawn_walker_with(
                    &mut c,
                    &zone,
                    zone.on_ground(130.0, 222.0),
                    job,
                    false,
                    130.0,
                ),
            )
        };
        queue.apply(app.world_mut());
        app.update();
        {
            let mut aw = app.world_mut().get_mut::<Awareness>(w).unwrap();
            aw.target = Some(player);
            aw.last_known = Some(me);
            aw.suspicion = 1.0;
        }
        *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Engage;
        for _ in 0..(90 * 8) {
            put(&mut app, player, me);
            app.world_mut()
                .entity_mut(w)
                .insert(super::squad::SquadRole {
                    role: super::squad::Role::Pin,
                    target: player,
                    post: at,
                });
            // The mate's post far off (moving up) or where it stands.
            let here = app.world().get::<Transform>(mate).unwrap().translation;
            app.world_mut()
                .entity_mut(mate)
                .insert(super::squad::SquadRole {
                    role: super::squad::Role::Flank,
                    target: player,
                    post: if covering {
                        zone.on_ground(140.0, 190.0)
                    } else {
                        here
                    },
                });
            app.update();
        }
        let n = app
            .world()
            .resource::<Fired>()
            .0
            .iter()
            .filter(|(e, _)| *e == w)
            .count();
        let marked = app.world().get::<super::walker::Suppressing>(w).is_some();
        (n, marked)
    };
    let (alone, marked_alone) = run(false);
    let (covering, marked) = run(true);
    assert!(marked && !marked_alone, "it shows it's suppressing");
    assert!(
        covering as f32 >= alone as f32 * 1.8,
        "a faster cadence: {covering} rounds against {alone}"
    );
}
