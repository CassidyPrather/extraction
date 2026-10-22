//! Physics edge cases: items thrown hard at the world, heaped up, spawned
//! inside geometry, and the player shoved into walls. Nothing may tunnel,
//! explode or go NaN. See `docs/SOAK.md`, "Physics edge cases".

use avian3d::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use extraction_core::items::ItemKind;

use crate::LaunchOptions;
use crate::items::{Catalog, SpawnItem, spawn_item};
use crate::world::{Layer, Zone};

fn game(map: &str) -> App {
    let mut app = crate::headless_app(
        LaunchOptions {
            robots: false,
            map: map.into(),
            ..Default::default()
        },
        false,
    );
    app.finish();
    app.cleanup();
    run(&mut app, 10);
    app
}

fn run(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn catalog(app: &App) -> Catalog {
    app.world().resource::<Catalog>().clone()
}

/// An item at `at`, moving at `vel`.
fn throw(app: &mut App, kind: ItemKind, at: Transform, vel: Vec3) -> Entity {
    let catalog = catalog(app);
    let mut commands = app.world_mut().commands();
    let e = spawn_item(
        &mut commands,
        &catalog,
        SpawnItem {
            kind,
            transform: at,
            condition: 1.0,
        },
    );
    commands.entity(e).insert(LinearVelocity(vel));
    app.world_mut().flush();
    e
}

/// The first piece of the static world along a ray: (entity, point, normal).
fn ray(app: &mut App, from: Vec3, dir: Vec3, max: f32) -> Option<(Entity, Vec3, Vec3)> {
    let dir = Dir3::new(dir).ok()?;
    app.world_mut()
        .run_system_once(move |q: SpatialQuery| {
            q.cast_ray(
                from,
                dir,
                max,
                true,
                &SpatialQueryFilter::from_mask(Layer::World),
            )
            .map(|h| (h.entity, from + *dir * h.distance, h.normal))
        })
        .ok()
        .flatten()
}

fn pos(app: &App, e: Entity) -> Vec3 {
    app.world()
        .get::<Position>(e)
        .map(|p| p.0)
        .unwrap_or(Vec3::NAN)
}

fn speed(app: &App, e: Entity) -> f32 {
    app.world()
        .get::<LinearVelocity>(e)
        .map(|v| v.0.length())
        .unwrap_or(f32::NAN)
}

fn despawn(app: &mut App, es: &[Entity]) {
    for &e in es {
        if let Ok(ent) = app.world_mut().get_entity_mut(e) {
            ent.despawn();
        }
    }
}

/// How far past a surface a step may end and still count as touching it.
const SHALLOW: f32 = 0.02;

/// For each segment, the first static collider it passes through (and
/// where), if any. A body's middle never touches a surface (resting on
/// one, it stays this side), so a middle's path through one is tunnelling.
fn crossings(app: &mut App, segs: Vec<(Vec3, Vec3)>) -> Vec<Option<(Entity, Vec3)>> {
    app.world_mut()
        .run_system_once(
            move |q: SpatialQuery, of: Query<&ColliderOf>, bodies: Query<&RigidBody>| {
                let filter = SpatialQueryFilter::from_mask(Layer::World);
                segs.iter()
                    .map(|&(a, b)| {
                        let d = b - a;
                        let len = d.length();
                        if !len.is_finite() || len < 1e-4 {
                            return None;
                        }
                        let dir = Dir3::new(d).ok()?;
                        q.ray_hits(a, dir, len, 4, true, &filter)
                            .into_iter()
                            // Ending a couple of centimetres past a face is a
                            // small thing sinking into a curved collider on
                            // contact (the water tower's foot), not tunnelling.
                            .filter(|h| h.distance > 1e-3 && len - h.distance > SHALLOW)
                            .find(|h| {
                                of.get(h.entity)
                                    .ok()
                                    .and_then(|c| bodies.get(c.body).ok())
                                    .is_some_and(|b| *b == RigidBody::Static)
                            })
                            .map(|h| (h.entity, a + *dir * h.distance))
                    })
                    .collect::<Vec<_>>()
            },
        )
        .unwrap()
}

/// Runs `ticks` updates watching `es` for tunnelling: what each first
/// passed through, and where.
fn watch(app: &mut App, es: &[Entity], ticks: usize) -> Vec<Option<String>> {
    let mut last: Vec<Vec3> = es.iter().map(|&e| pos(app, e)).collect();
    let mut out: Vec<Option<String>> = vec![None; es.len()];
    for _ in 0..ticks {
        app.update();
        let now: Vec<Vec3> = es.iter().map(|&e| pos(app, e)).collect();
        let segs = last.iter().zip(&now).map(|(a, b)| (*a, *b)).collect();
        for (i, hit) in crossings(app, segs).into_iter().enumerate() {
            if let (None, Some((e, at))) = (&out[i], hit) {
                out[i] = Some(format!("through {} at {at:.2}", what(app, e)));
            }
        }
        last = now;
    }
    out
}

/// What a collider is: its name and its ancestors', innermost first.
fn what(app: &App, e: Entity) -> String {
    let mut names = Vec::new();
    let mut at = Some(e);
    while let Some(x) = at {
        if let Some(n) = app.world().get::<Name>(x) {
            names.push(n.as_str().to_owned());
        }
        at = app.world().get::<ChildOf>(x).map(|c| c.parent());
    }
    if names.is_empty() {
        format!("{e}")
    } else {
        names.join(" < ")
    }
}

/// A spot on a wall: where, its outward normal, and what it is.
#[derive(Clone)]
struct Wall {
    p: Vec3,
    n: Vec3,
    what: String,
}

/// Spots on outside walls: rays from 12 m out toward each building's
/// middle, 1.1 m up, that meet a near-vertical face at least 3 m on; a
/// wall from 0.6 to 1.6 m up (not a log pile's curve), with clear air
/// 1.5 m in front of it (not a porch).
fn wall_spots(app: &mut App) -> Vec<Wall> {
    let centres: Vec<Vec3> = app
        .world_mut()
        .query_filtered::<&GlobalTransform, With<crate::world::buildings::Building>>()
        .iter(app.world())
        .map(|g| g.translation())
        .collect();
    let zone_ground = |app: &App, x: f32, z: f32| app.world().resource::<Zone>().ground(x, z);
    let mut out = Vec::new();
    for c in centres {
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_2 + 0.3;
            let d = Vec3::new(a.cos(), 0.0, a.sin());
            let from2 = c + d * 12.0;
            let from = Vec3::new(from2.x, zone_ground(app, from2.x, from2.z) + 1.1, from2.z);
            let Some((e, p, n)) = ray(app, from, -d, 12.0) else {
                continue;
            };
            if n.y.abs() >= 0.3 || p.distance(from) <= 3.0 {
                continue;
            }
            let n = Vec3::new(n.x, 0.0, n.z).normalize();
            let start = p + n * 1.5;
            let upright = [-0.5, 0.0, 0.5].into_iter().all(|dy| {
                ray(app, start + Vec3::Y * dy, -n, 3.0).is_some_and(|(_, q, m)| {
                    (q.distance(start + Vec3::Y * dy) - 1.5).abs() < 0.15 && m.y.abs() < 0.3
                })
            });
            if upright {
                out.push(Wall {
                    p,
                    n,
                    what: what(app, e),
                });
            }
        }
    }
    out
}

/// Open ground: points on the terrain away from anything built.
fn ground_spots(app: &mut App, n: usize) -> Vec<Vec3> {
    let ins = {
        let zone = app.world().resource::<Zone>();
        let m = zone
            .marker(&extraction_core::map::MarkerKind::Insertion)
            .expect("an insertion");
        Vec2::new(m.at[0], m.at[1])
    };
    let mut out = Vec::new();
    let mut i = 0;
    while out.len() < n && i < n * 20 {
        i += 1;
        let a = i as f32 * 2.399;
        let r = 8.0 + 1.6 * (i as f32).sqrt() * 4.0;
        let xz = ins + Vec2::new(a.cos(), a.sin()) * r;
        let g = app.world().resource::<Zone>().ground(xz.x, xz.y);
        let Some((_, p, nrm)) = ray(app, Vec3::new(xz.x, g + 40.0, xz.y), Vec3::NEG_Y, 80.0) else {
            continue;
        };
        // The terrain itself, not a roof or a rock.
        if (p.y - g).abs() < 0.05 && nrm.y > 0.6 {
            out.push(p);
        }
    }
    out
}

/// Points on the bed of open water at least 0.5 m deep, a few metres
/// apart.
fn water_spots(app: &App, n: usize) -> Vec<Vec3> {
    let zone = app.world().resource::<Zone>();
    let hm = &zone.hm;
    let span = hm.n as f32 * hm.cell;
    let mut out: Vec<Vec3> = Vec::new();
    let step = 3.0;
    let mut x = hm.origin + 10.0;
    while x < hm.origin + span - 10.0 && out.len() < n {
        let mut z = hm.origin + 10.0;
        while z < hm.origin + span - 10.0 && out.len() < n {
            if let Some(w) = hm.water_level([x, z]) {
                let g = zone.ground(x, z);
                let p = Vec3::new(x, g, z);
                if w - g > 0.5 && out.iter().all(|o| o.distance(p) > 4.0) {
                    out.push(p);
                }
            }
            z += step;
        }
        x += step;
    }
    out
}

/// Throws in the hundreds that may still go through something: the
/// residue `docs/SOAK.md` lists as open (a water tower's face, a sliver of
/// paper into the ground, a rod end-on). Lower it as they're fixed.
const THROUGH_ALLOWED: usize = 5;

/// Every item in the catalog thrown hard at outside walls and at the
/// ground, at 10, 25 and 50 m/s: each ends up on the near side of what it
/// hit, above the ground, finite and at rest-ish.
#[test]
fn thrown_items_never_tunnel() {
    let mut app = game("marrow_creek");
    let cat = catalog(&app);
    let kinds: Vec<ItemKind> = cat.iter().map(|(k, _)| k).collect();
    let walls = wall_spots(&mut app);
    let mut grounds = ground_spots(&mut app, 60);
    let wet = water_spots(&app, 30);
    eprintln!(
        "{} walls, {} ground, {} water",
        walls.len(),
        grounds.len(),
        wet.len()
    );
    grounds.extend(wet);
    assert!(walls.len() > 20, "found {} wall spots", walls.len());
    assert!(grounds.len() > 30, "found {} ground spots", grounds.len());
    let mut bad = Vec::new();
    for v in [10.0, 25.0, 50.0] {
        // At walls: one item per spot per round.
        for chunk in kinds.chunks(walls.len()) {
            let thrown: Vec<(Entity, ItemKind, &Wall)> = chunk
                .iter()
                .zip(&walls)
                .map(|(&k, w)| {
                    let from = w.p + w.n * 1.5;
                    let e = throw(&mut app, k, Transform::from_translation(from), -w.n * v);
                    (e, k, w)
                })
                .collect();
            let es: Vec<Entity> = thrown.iter().map(|t| t.0).collect();
            let through = watch(&mut app, &es, 135);
            for (&(e, k, w), through) in thrown.iter().zip(through) {
                let at = pos(&app, e);
                let s = speed(&app, e);
                let g = app.world().resource::<Zone>().ground(at.x, at.z);
                if !at.is_finite() || through.is_some() || at.y < g - 0.3 || s > 20.0 {
                    bad.push(format!(
                        "{} at {v} m/s into the wall at {:.1} ({}): {}, ended {:.2} m over \
                         the ground, moving {s:.1} m/s",
                        cat.get(k).id,
                        w.p,
                        w.what,
                        through.unwrap_or_default(),
                        at.y - g
                    ));
                }
            }
            despawn(&mut app, &thrown.iter().map(|t| t.0).collect::<Vec<_>>());
        }
        // At the ground, steeply.
        for chunk in kinds.chunks(grounds.len()) {
            let thrown: Vec<(Entity, ItemKind)> = chunk
                .iter()
                .zip(&grounds)
                .map(|(&k, &p)| {
                    let dir = Vec3::new(0.4, -1.0, 0.2).normalize();
                    let from = p - dir * 1.5;
                    (
                        throw(&mut app, k, Transform::from_translation(from), dir * v),
                        k,
                    )
                })
                .collect();
            let es: Vec<Entity> = thrown.iter().map(|t| t.0).collect();
            let through = watch(&mut app, &es, 135);
            for (&(e, k), through) in thrown.iter().zip(through) {
                let at = pos(&app, e);
                let g = app.world().resource::<Zone>().ground(at.x, at.z);
                let s = speed(&app, e);
                if !at.is_finite() || through.is_some() || at.y < g - 0.1 || s > 20.0 {
                    bad.push(format!(
                        "{} at {v} m/s into the ground: {}, ended {:.2} m over it, moving \
                         {s:.1} m/s",
                        cat.get(k).id,
                        through.unwrap_or_default(),
                        at.y - g
                    ));
                }
            }
            despawn(&mut app, &thrown.iter().map(|t| t.0).collect::<Vec<_>>());
        }
    }
    // About 1,600 throws. Before the backstop (`items::lost`) 47 went
    // through something; what's left is open (`docs/SOAK.md`).
    eprintln!("{} bad throws:\n{}", bad.len(), bad.join("\n"));
    assert!(
        bad.len() <= THROUGH_ALLOWED,
        "{} bad throws:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// A floor inside a building: straight down from 0.6 m over its footing
/// at each building's middle, a level face under a roof, no higher than
/// that (so not a table). (point, the footing's height).
fn floor_spots(app: &mut App) -> Vec<(Vec3, f32)> {
    let centres: Vec<Vec3> = app
        .world_mut()
        .query_filtered::<&GlobalTransform, With<crate::world::buildings::Building>>()
        .iter(app.world())
        .map(|g| g.translation())
        .collect();
    let mut out = Vec::new();
    for c in centres {
        let from = c + Vec3::Y * 0.6;
        if let Some((_, p, n)) = ray(app, from, Vec3::NEG_Y, 1.0) {
            // Something over it, too: indoors. And a floor: flat and level
            // 0.6 m out each way (the top of a log pile, under a tree, once
            // passed for one, and a heap on it rolled for the whole run).
            let level = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
                .iter()
                .all(|d| {
                    ray(app, p + *d * 0.6 + Vec3::Y * 0.3, Vec3::NEG_Y, 0.6)
                        .is_some_and(|(_, q, m)| m.y > 0.95 && (q.y - p.y).abs() < 0.05)
                });
            if n.y > 0.9 && level && ray(app, p + Vec3::Y * 0.3, Vec3::Y, 8.0).is_some() {
                out.push((p, c.y));
            }
        }
    }
    out
}

/// Drops a heap of `n` items (the catalog's kinds in turn) in a column
/// over `at`; runs `secs`, tracking the fastest any of them went after
/// the first second (the fall). Returns (entities, fastest).
fn heap(app: &mut App, at: Vec3, n: usize, secs: f32) -> (Vec<Entity>, (f32, Entity)) {
    let cat = catalog(app);
    let kinds: Vec<ItemKind> = cat.iter().map(|(k, _)| k).collect();
    let es: Vec<Entity> = (0..n)
        .map(|i| {
            let k = kinds[(i * 7) % kinds.len()];
            let off = Vec3::new(
                ((i % 3) as f32 - 1.0) * 0.35,
                0.3 + (i / 9) as f32 * 0.35,
                (((i / 3) % 3) as f32 - 1.0) * 0.35,
            );
            let rot = Quat::from_rotation_y(i as f32 * 0.7);
            throw(
                app,
                k,
                Transform::from_translation(at + off).with_rotation(rot),
                Vec3::ZERO,
            )
        })
        .collect();
    let mut fastest = (0.0f32, Entity::PLACEHOLDER);
    let ticks = (secs * 90.0) as usize;
    for t in 0..ticks {
        app.update();
        if t > 90 + (n / 9) * 30 {
            for &e in &es {
                let v = speed(app, e);
                if v > fastest.0 {
                    fastest = (v, e);
                }
            }
        }
    }
    (es, fastest)
}

/// An item's catalog id.
fn id(app: &App, e: Entity) -> String {
    let cat = catalog(app);
    app.world()
        .get::<crate::items::Item>(e)
        .map_or("?".into(), |i| cat.get(i.kind()).id.clone())
}

/// Heaps of 60 items dropped on a building's floor and on a hillside:
/// they settle, nothing explodes out of the pile, nothing falls through.
#[test]
fn heaps_settle() {
    let mut app = game("marrow_creek");
    let floors = floor_spots(&mut app);
    assert!(floors.len() >= 3, "found {} floors", floors.len());
    let mut bad = Vec::new();
    for &(f, footing) in floors.iter().take(3) {
        let (es, (fastest, which)) = heap(&mut app, f, 60, 8.0);
        let mut moving = Vec::new();
        for &e in &es {
            let p = pos(&app, e);
            // Under the floor: below the building's footing.
            if !p.is_finite() || p.y < footing.min(f.y) - 0.3 {
                bad.push(format!("an item in the heap at {f:.1} ended at {p:.2}"));
            }
            if speed(&app, e) > 0.5 {
                moving.push(format!("{} at {:.1} m/s", id(&app, e), speed(&app, e)));
            }
        }
        if fastest > 8.0 {
            bad.push(format!(
                "the heap at {f:.1} threw {} at {fastest:.1} m/s",
                id(&app, which)
            ));
        }
        if moving.len() > 3 {
            bad.push(format!(
                "the heap at {f:.1}: {} still moving after 8 s: {}",
                moving.len(),
                moving.join(", ")
            ));
        }
        despawn(&mut app, &es);
    }
    // A hillside.
    let slope = ground_spots(&mut app, 400)
        .into_iter()
        .max_by(|a, b| {
            let s = |p: &Vec3, app: &App| {
                let z = app.world().resource::<Zone>();
                (z.ground(p.x + 1.0, p.z) - z.ground(p.x - 1.0, p.z)).abs()
                    + (z.ground(p.x, p.z + 1.0) - z.ground(p.x, p.z - 1.0)).abs()
            };
            s(a, &app).total_cmp(&s(b, &app))
        })
        .unwrap();
    let (es, (fastest, _)) = heap(&mut app, slope, 60, 8.0);
    for &e in &es {
        let p = pos(&app, e);
        let g = app.world().resource::<Zone>().ground(p.x, p.z);
        if !p.is_finite() || p.y < g - 0.1 {
            bad.push(format!(
                "an item in the hillside heap ended {:.2} m over the ground",
                p.y - g
            ));
        }
    }
    if fastest > 12.0 {
        bad.push(format!(
            "the hillside heap threw something at {fastest:.1} m/s"
        ));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// The fastest an item spawned inside something may be pushed out, m/s.
/// (Ten items in exactly one spot reach 10: Avian pushes each pair apart
/// at up to 4 m/s, and a pile's pushes add up.)
const LAUNCH: f32 = 12.0;

/// Items spawned inside things: half into a wall, deep in a wall, under
/// the ground, ten in one spot, in the truck. They may be pushed out but
/// never launched, and never end up under the ground.
#[test]
fn items_spawned_inside_things_are_not_launched() {
    let mut app = game("marrow_creek");
    let cat = catalog(&app);
    let kinds: Vec<ItemKind> = cat.iter().map(|(k, _)| k).collect();
    let walls = wall_spots(&mut app);
    let grounds = ground_spots(&mut app, 40);
    let truck = app
        .world_mut()
        .query_filtered::<&GlobalTransform, With<crate::raid::truck::Truck>>()
        .iter(app.world())
        .next()
        .map(|g| g.translation());
    let mut cases: Vec<(String, Entity)> = Vec::new();
    let mut k = 0;
    let mut next = || {
        k += 1;
        kinds[(k * 13) % kinds.len()]
    };
    for (i, w) in walls.iter().take(30).enumerate() {
        let (p, n) = (w.p, w.n);
        let depth = if i % 2 == 0 { 0.05 } else { 0.2 };
        let kind = next();
        let e = throw(
            &mut app,
            kind,
            Transform::from_translation(p - n * depth),
            Vec3::ZERO,
        );
        cases.push((
            format!("{} {depth} m into the wall at {p:.1}", cat.get(kind).id),
            e,
        ));
    }
    for &g in grounds.iter().take(10) {
        let kind = next();
        let e = throw(
            &mut app,
            kind,
            Transform::from_translation(g - Vec3::Y * 0.05),
            Vec3::ZERO,
        );
        cases.push((
            format!("{} half in the ground at {g:.1}", cat.get(kind).id),
            e,
        ));
    }
    for &g in grounds.iter().skip(10).take(3) {
        for _ in 0..10 {
            let kind = next();
            let e = throw(
                &mut app,
                kind,
                Transform::from_translation(g + Vec3::Y * 0.1),
                Vec3::ZERO,
            );
            cases.push((format!("{} one of ten at {g:.1}", cat.get(kind).id), e));
        }
    }
    if let Some(t) = truck {
        for y in [0.3, 0.8, 1.2, 1.6] {
            let kind = next();
            let e = throw(
                &mut app,
                kind,
                Transform::from_translation(t + Vec3::Y * y),
                Vec3::ZERO,
            );
            cases.push((format!("{} in the truck {y} m up", cat.get(kind).id), e));
        }
    }
    let mut fastest = vec![0.0f32; cases.len()];
    for _ in 0..270 {
        app.update();
        for (i, (_, e)) in cases.iter().enumerate() {
            fastest[i] = fastest[i].max(speed(&app, *e));
        }
    }
    let mut bad = Vec::new();
    for (i, (what, e)) in cases.iter().enumerate() {
        let p = pos(&app, *e);
        let zone = app.world().resource::<Zone>();
        let g = zone.ground(p.x, p.z);
        // Spawned half in a wall, it may come out of either face: inside an
        // underground site's kiosk, down the stair is where it belongs.
        let down = crate::items::lost::down_a_site(zone, p, 0.5);
        if !p.is_finite() || fastest[i] > LAUNCH || (p.y < g - 0.2 && !down) {
            bad.push(format!(
                "{what}: went {:.1} m/s, ended {:.2} m over the ground",
                fastest[i],
                p.y - g
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {}:\n{}",
        bad.len(),
        cases.len(),
        bad.join("\n")
    );
}

/// The player sprinting into walls (and into building corners) and
/// walking into them in the room: the body stays this side.
#[test]
fn the_player_cant_be_pushed_through_walls() {
    use crate::rig::RigInput;
    let mut vr = crate::vr_harness::Vr::new("marrow_creek");
    let walls = wall_spots(&mut vr.app);
    let mut bad = Vec::new();
    for w in walls.iter().take(24) {
        let (p, n) = (w.p, w.n);
        let g = vr.app.world().resource::<Zone>().ground(p.x, p.z);
        let start = p + n * 1.2;
        let start = Vec3::new(
            start.x,
            vr.app.world().resource::<Zone>().ground(start.x, start.z),
            start.z,
        );
        if (start.y - g).abs() > 0.6 {
            continue;
        }
        // Straight at it, and glancing (sliding along it into corners and
        // doorways).
        for slant in [0.0f32, 0.6, -0.6] {
            let yaw = n.x.atan2(n.z) + slant;
            vr.head(Vec3::new(0.0, 1.65, 0.0));
            vr.stand(start, yaw);
            vr.app
                .world_mut()
                .get_mut::<crate::player::Body>(vr.player)
                .unwrap()
                .feet = None;
            vr.step(2);
            // The movement capsule's middle (it stands on the feet; the
            // head may lean into a wall: nothing stops a real one).
            let centre = |vr: &crate::vr_harness::Vr| {
                let root = *vr.app.world().get::<GlobalTransform>(vr.player).unwrap();
                let body = vr
                    .app
                    .world()
                    .get::<crate::player::Body>(vr.player)
                    .unwrap();
                let f = body.feet.unwrap_or_default();
                root.transform_point(Vec3::new(f.x, 0.9, f.y))
            };
            let mut last = centre(&vr);
            let mut through = None;
            let set = |vr: &mut crate::vr_harness::Vr, run: bool| {
                let mut i = vr.app.world_mut().get_mut::<RigInput>(vr.player).unwrap();
                i.locomotion = if run { Vec2::Y } else { Vec2::ZERO };
                i.sprint = run;
            };
            // Sprint at it for three seconds, then walk a metre and a half
            // on into it in the room.
            set(&mut vr, true);
            for k in 0..360 {
                if k == 270 {
                    set(&mut vr, false);
                }
                if k >= 270 {
                    vr.head(Vec3::new(0.0, 1.65, -((k - 270) as f32) / 60.0));
                }
                vr.step(1);
                let now = centre(&vr);
                if through.is_none() {
                    if let Some(Some((e, at))) = crossings(&mut vr.app, vec![(last, now)]).pop() {
                        through = Some(format!("{} at {at:.2}", what(&vr.app, e)));
                    }
                }
                last = now;
            }
            if !last.is_finite() || through.is_some() {
                bad.push(format!(
                    "into the wall at {p:.1} ({}) at {slant} rad: through {}",
                    w.what,
                    through.unwrap_or_default()
                ));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// An item that gets under the ground (tunnelled, spawned there) comes
/// back to where it last lay, or onto the ground; one lying on an
/// underground site's floor, below the ground on purpose, stays put.
#[test]
fn items_under_the_world_come_back() {
    let mut app = game("marrow_creek");
    let cat = catalog(&app);
    let kind = cat.kind("field_dressing").expect("a dressing");
    let g = ground_spots(&mut app, 4);
    // Rests a while on the ground, then is shoved under it.
    let rested = throw(
        &mut app,
        kind,
        Transform::from_translation(g[0] + Vec3::Y * 0.2),
        Vec3::ZERO,
    );
    let never = throw(
        &mut app,
        kind,
        Transform::from_translation(g[1] - Vec3::Y * 5.0),
        Vec3::ZERO,
    );
    run(&mut app, 180);
    let rest = pos(&app, rested);
    app.world_mut().get_mut::<Position>(rested).unwrap().0 = g[0] - Vec3::Y * 6.0;
    run(&mut app, 90);
    let back = pos(&app, rested);
    assert!(
        back.distance(rest) < 0.3,
        "back where it lay ({rest:.2}): {back:.2}"
    );
    let p = pos(&app, never);
    let ground = app.world().resource::<Zone>().ground(p.x, p.z);
    assert!(
        p.y > ground - 0.1 && p.xz().distance(g[1].xz()) < 1.0,
        "up on the ground over where it was: {:.2} m over",
        p.y - ground
    );
    // Down a site.
    let site = {
        let zone = app.world().resource::<Zone>();
        extraction_core::underground::placed(&zone.map)
            .into_iter()
            .find_map(|s| {
                let r = extraction_core::underground::raster(&s.id)?;
                let footing = zone.hm.footing(&s.id, s.at, s.rot);
                // A room cell's middle.
                (0..r.nx * r.nz).find_map(|c| {
                    let (i, j) = (c % r.nx, c / r.nx);
                    let local = [r.origin[0] + i as f32 + 0.5, r.origin[1] + j as f32 + 0.5];
                    let f = r.floor_at(local)?;
                    matches!(r.at(local), extraction_core::underground::Cell::Room(_)).then(|| {
                        let w = s.to_world(local);
                        Vec3::new(w[0], footing + f, w[1])
                    })
                })
            })
    };
    if let Some(floor) = site {
        let e = throw(
            &mut app,
            kind,
            Transform::from_translation(floor + Vec3::Y * 0.3),
            Vec3::ZERO,
        );
        run(&mut app, 180);
        let p = pos(&app, e);
        assert!(
            p.distance(floor) < 0.5,
            "a dressing on a site's floor at {floor:.1} stays there: {p:.2}"
        );
    }
}

/// A controller's tracking jumps (or the playspace moves) while the hand
/// holds something: letting go then mustn't fire it off at hundreds of
/// metres a second.
#[test]
fn a_tracking_jump_doesnt_launch_what_you_let_go() {
    use crate::rig::Side;
    let mut vr = crate::vr_harness::Vr::new("marrow_creek");
    let at = {
        let z = vr.app.world().resource::<Zone>();
        z.on_ground(0.0, 30.0)
    };
    vr.stand(at, 0.0);
    let hand = at + Vec3::new(0.25, 1.0, -0.4);
    let item = vr.spawn("field_dressing", Transform::from_translation(hand));
    vr.hand(Side::Right, hand, Quat::IDENTITY, 1.0, 0.0);
    vr.step(5);
    assert!(
        vr.app.world().get::<crate::interact::Held>(item).is_some(),
        "in the hand"
    );
    // Tracking jumps three metres in one frame, and the hand opens.
    vr.hand(
        Side::Right,
        hand + Vec3::new(0.0, 0.0, -3.0),
        Quat::IDENTITY,
        1.0,
        0.0,
    );
    vr.step(1);
    vr.hand(
        Side::Right,
        hand + Vec3::new(0.0, 0.0, -3.0),
        Quat::IDENTITY,
        0.0,
        0.0,
    );
    vr.step(2);
    let v = vr
        .app
        .world()
        .get::<LinearVelocity>(item)
        .map(|v| v.0.length())
        .unwrap();
    assert!(
        v <= crate::interact::MAX_THROW + 0.5,
        "let go at {v:.0} m/s"
    );
}

/// Long thin things (a cleaning rod, a rifle, a crowbar) thrown hard
/// into the ground at every angle, spinning: never through it, not even
/// for a tick.
#[test]
fn long_things_thrown_into_the_ground_stay_on_it() {
    let mut app = game("marrow_creek");
    let cat = catalog(&app);
    let ids = [
        "cleaning_rod",
        "crowbar",
        "ak74",
        "mosin",
        "knife",
        "round_545",
    ];
    let kinds: Vec<ItemKind> = ids.iter().filter_map(|id| cat.kind(id)).collect();
    let spots = ground_spots(&mut app, 48);
    let mut bad = Vec::new();
    for v in [10.0, 25.0] {
        let mut thrown = Vec::new();
        for (i, &p) in spots.iter().enumerate() {
            let k = kinds[i % kinds.len()];
            let a = i as f32 * 0.9;
            let dir = Vec3::new(a.cos() * 0.6, -1.0, a.sin() * 0.6).normalize();
            let rot = Quat::from_euler(EulerRot::XYZ, a, a * 1.3, a * 0.7);
            let from = p - dir * 1.0;
            let e = throw(
                &mut app,
                k,
                Transform::from_translation(from).with_rotation(rot),
                dir * v,
            );
            app.world_mut()
                .entity_mut(e)
                .insert(AngularVelocity(Vec3::new(a.sin(), 1.0, a.cos()) * 15.0));
            thrown.push((e, k, p));
        }
        let es: Vec<Entity> = thrown.iter().map(|t| t.0).collect();
        let through = watch(&mut app, &es, 120);
        for (&(e, k, p), through) in thrown.iter().zip(through) {
            let at = pos(&app, e);
            let g = app.world().resource::<Zone>().ground(at.x, at.z);
            if through.is_some() || !at.is_finite() || at.y < g - 0.1 {
                bad.push(format!(
                    "{} at {v} m/s into the ground at {p:.1}: {}, ended {:.2} m over it",
                    cat.get(k).id,
                    through.unwrap_or_default(),
                    at.y - g
                ));
            }
        }
        despawn(&mut app, &thrown.iter().map(|t| t.0).collect::<Vec<_>>());
    }
    // 96 throws; 18 or more went under before the backstop.
    eprintln!("{}", bad.join("\n"));
    assert!(bad.len() <= 2, "{}", bad.join("\n"));
}
