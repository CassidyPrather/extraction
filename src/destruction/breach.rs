//! Holes in walls. A building's walls are boxes in one compound collider
//! and a few merged meshes; a hole replaces the wall box it's in with the
//! boxes round it (`split`: over it, under it, either side), in the
//! building's prefab as it's built from now on. The collider and its
//! surfaces are rebuilt at once, the client builds the meshes again
//! (`buildings::redress`).
//!
//! What makes them:
//! - a [`Breach`](super::hooks::Breach): a heavy machine's arm tears a
//!   doorway out (floor to 2 m), a way in for the robots;
//! - sustained fire: rounds and blows wear a patch of a plank, log or
//!   plaster wall ([`WALL_WEAR`] by material) until it gives, a hole the
//!   size of a head to see and shoot through;
//! - a grenade against a wall blows a hole a metre across, down to the
//!   floor if it went off at its foot.
//!
//! Holes are kept by where they are in the world and how big
//! ([`Breaches`]), saved with the Zone, and mended by the Surge.

use bevy::prelude::*;
use extraction_core::fracture;
use extraction_core::noise::Rng;
use std::collections::HashMap;

use super::debris::{self, DebrisPool, Kind, Piece};
use super::hooks::Breach;
use super::{Broke, Stuff, key};
use crate::render::materials::Mat;
use crate::weapons::ballistics::Hit;
use crate::world::buildings::{Building, Geo, Shape};

/// A torn doorway's width, metres, and how high it goes above the
/// wall's foot.
pub const HOLE_WIDTH: f32 = 1.1;
pub const HOLE_HEIGHT: f32 = 2.0;
/// A hole worn by fire: about a head's size.
pub const SHOT_HOLE: Vec2 = Vec2::new(0.45, 0.45);
/// A hole blown by a grenade against a wall.
pub const BLAST_HOLE: Vec2 = Vec2::new(0.9, 0.9);
/// A grenade this close to a wall (metres) blows a hole in it.
pub const BLAST_REACH: f32 = 1.3;
/// What's left over a doorway, at least, metres (a lintel's worth).
const HEAD: f32 = 0.15;
/// A point this far either side of a wall's centre plane is in it.
const REACH: f32 = 0.3;
/// Wear is kept in patches of a wall this big (metres).
const PATCH: f32 = 0.5;

/// Harm a patch of wall takes before it gives way, by what it is.
pub fn wall_wear(m: Mat) -> f32 {
    match m {
        Mat::Plaster | Mat::SootPlaster => 300.0,
        Mat::Planks | Mat::Timber | Mat::Charred => 360.0,
        Mat::Corrugated => 500.0,
        Mat::LogWall => 900.0,
        _ => f32::INFINITY,
    }
}

/// One hole: its middle (world, on the wall's plane), its size across
/// and up, and whether it goes down to the floor (a way in).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hole {
    pub at: Vec3,
    pub size: Vec2,
    pub floor: bool,
}

impl Hole {
    pub fn doorway(at: Vec3) -> Self {
        Self {
            at,
            size: Vec2::new(HOLE_WIDTH, HOLE_HEIGHT),
            floor: true,
        }
    }
}

/// Every hole made since the Surge.
#[derive(Resource, Default, Debug, Clone)]
pub struct Breaches(pub Vec<Hole>);

impl Breaches {
    /// As saved: centimetres, [x, y, z, across, up, floor].
    pub fn to_save(&self) -> Vec<[i32; 6]> {
        self.0
            .iter()
            .map(|h| {
                let k = key(h.at);
                let s = (h.size * 100.0).round();
                [k[0], k[1], k[2], s.x as i32, s.y as i32, h.floor as i32]
            })
            .collect()
    }

    pub fn from_save(holes: &[[i32; 6]], doorways: &[[i32; 3]]) -> Self {
        let cm = |v: i32| v as f32 / 100.0;
        Self(
            holes
                .iter()
                .map(|h| Hole {
                    at: Vec3::new(cm(h[0]), cm(h[1]), cm(h[2])),
                    size: Vec2::new(cm(h[3]), cm(h[4])),
                    floor: h[5] != 0,
                })
                // Saves from before holes had sizes: doorways.
                .chain(
                    doorways
                        .iter()
                        .map(|c| Hole::doorway(Vec3::new(cm(c[0]), cm(c[1]), cm(c[2])))),
                )
                .collect(),
        )
    }
}

/// A hole as a way in, for the robots' doorway index
/// (`robots::doors::Entrances`): it stands where a door would, and having
/// no `Door`, it's always open.
#[derive(Component, Debug)]
pub struct BreachHole;

/// Where a hole is a way in: its middle (world) and the way out of the
/// building (horizontal).
fn way_in(bxf: &Transform, hole: Vec3) -> (Vec3, Vec3) {
    let local = bxf.compute_affine().inverse().transform_point3(hole);
    // Out: away from the footprint's middle, square to the wall.
    let flat = Vec2::new(local.x, local.z);
    let n = if flat.x.abs() > flat.y.abs() {
        Vec3::X * flat.x.signum()
    } else {
        Vec3::Z * flat.y.signum()
    };
    let out = (bxf.rotation * n).with_y(0.0).normalize_or(Vec3::Z);
    (hole, out)
}

fn index_hole(
    commands: &mut Commands,
    building: Entity,
    prefab: &str,
    bxf: &Transform,
    hole: Vec3,
) {
    let size = extraction_core::map::prefab_footprint(prefab);
    let xf = *bxf;
    let (mid, out) = way_in(bxf, hole);
    let marker = commands
        .spawn((
            Name::new("Breach hole"),
            BreachHole,
            Transform::from_translation(mid),
        ))
        .id();
    commands.queue(move |w: &mut World| {
        if let Some(mut e) = w.get_resource_mut::<crate::robots::doors::Entrances>() {
            // A building with no door had no box to be inside: give it one,
            // or nobody is ever indoors there for a robot to come for.
            // (Only once the index is built: an early shell would stop it
            // being built at all.)
            if !e.shells.is_empty() && !e.shells.iter().any(|s| s.building == building) {
                e.shells.push(crate::robots::doors::Shell {
                    building,
                    xf,
                    half: Vec2::new(size[0], size[2]) * 0.5,
                    height: size[1],
                });
            }
            e.doors.push(crate::robots::doors::Entrance {
                door: marker,
                building,
                mid,
                out,
            });
        }
    });
}

/// Whether a point is within `pad` of a building's footprint box: cheap,
/// so only the buildings it could be in get their prefab built.
fn near_building(prefab: &str, bxf: &Transform, p: Vec3, pad: f32) -> bool {
    let f = extraction_core::map::prefab_footprint(prefab);
    let l = bxf.compute_affine().inverse().transform_point3(p);
    l.x.abs() <= f[0] * 0.5 + pad && l.z.abs() <= f[2] * 0.5 + pad && l.y < f[1] + pad + 2.0
}

/// A building whose walls changed: the client builds its meshes again.
#[derive(Component)]
pub struct Breached;

/// Walls that can be holed: boards, logs, plaster and sheet.
pub fn breachable(m: Mat) -> bool {
    matches!(
        m,
        Mat::Planks
            | Mat::LogWall
            | Mat::Timber
            | Mat::Charred
            | Mat::Plaster
            | Mat::SootPlaster
            | Mat::Corrugated
    )
}

/// The wall piece holding a point (building space), if it can be holed:
/// a standing box, thin one way.
pub fn wall_at(geo: &Geo, local: Vec3) -> Option<usize> {
    geo.pieces.iter().position(|p| {
        let Shape::Box(s) = p.shape else {
            return false;
        };
        if !p.collide || !breachable(p.mat) || s.z > 0.45 || s.y < 0.3 || s.x < 0.2 {
            return false;
        }
        let q = p.at.rotation.inverse() * (local - p.at.translation);
        q.x.abs() <= s.x * 0.5 && q.y.abs() <= s.y * 0.5 && q.z.abs() <= s.z * 0.5 + REACH
    })
}

/// The rect a hole of `size` at `local` cuts in wall piece `i`, in the
/// piece's own space: (x0, x1, y0, y1). None if there's no room for it.
pub fn hole_in(geo: &Geo, i: usize, local: Vec3, size: Vec2, floor: bool) -> Option<[f32; 4]> {
    let p = &geo.pieces[i];
    let Shape::Box(s) = p.shape else {
        return None;
    };
    let (hx, hy) = (s.x * 0.5, s.y * 0.5);
    let q = p.at.rotation.inverse() * (local - p.at.translation);
    let mut x0 = (q.x - size.x * 0.5).max(-hx);
    let mut x1 = (q.x + size.x * 0.5).min(hx);
    // No slivers left standing by an edge.
    if x0 + hx < 0.2 {
        x0 = -hx;
    }
    if hx - x1 < 0.2 {
        x1 = hx;
    }
    let (y0, y1) = if floor {
        (-hy, (-hy + size.y).min(hy - HEAD))
    } else {
        let mut y0 = (q.y - size.y * 0.5).max(-hy);
        let mut y1 = (q.y + size.y * 0.5).min(hy);
        if y0 + hy < 0.15 {
            y0 = -hy;
        }
        if hy - y1 < 0.15 {
            y1 = hy;
        }
        (y0, y1)
    };
    let tall = if floor { 1.0 } else { 0.2 };
    (x1 - x0 >= 0.2 && y1 - y0 >= tall).then_some([x0, x1, y0, y1])
}

/// Cut a hole in wall piece `i`: it becomes one of the boxes round the
/// hole, and the rest are added at the end (so the indices the building's
/// props hold stay right). A hole as big as the piece leaves nothing.
fn split(geo: &mut Geo, i: usize, hole: [f32; 4]) {
    let p = geo.pieces[i];
    let Shape::Box(s) = p.shape else {
        return;
    };
    let (hx, hy) = (s.x * 0.5, s.y * 0.5);
    let [x0, x1, y0, y1] = hole;
    let piece = |a: f32, b: f32, lo: f32, hi: f32| {
        let mut q = p;
        q.shape = Shape::Box(Vec3::new(b - a, hi - lo, s.z));
        q.at.translation =
            p.at.translation + p.at.rotation * Vec3::new((a + b) * 0.5, (lo + hi) * 0.5, 0.0);
        q
    };
    let mut parts = Vec::new();
    if y1 < hy - 0.01 {
        parts.push(piece(x0, x1, y1, hy));
    }
    if y0 > -hy + 0.01 {
        parts.push(piece(x0, x1, -hy, y0));
    }
    if x0 > -hx + 0.01 {
        parts.push(piece(-hx, x0, -hy, hy));
    }
    if x1 < hx - 0.01 {
        parts.push(piece(x1, hx, -hy, hy));
    }
    let mut parts = parts.into_iter();
    match parts.next() {
        Some(first) => geo.pieces[i] = first,
        None => {
            geo.pieces[i].collide = false;
            geo.pieces[i].shape = Shape::Box(Vec3::ZERO);
        }
    }
    geo.pieces.extend(parts);
}

/// A building's prefab with every hole in it cut: what `body` and the
/// meshes are built from. Returns how many holes landed.
pub fn apply(geo: &mut Geo, bxf: &Transform, holes: &[Hole]) -> usize {
    let inv = bxf.compute_affine().inverse();
    let mut n = 0;
    for h in holes {
        let local = inv.transform_point3(h.at);
        let Some(i) = wall_at(geo, local) else {
            continue;
        };
        if let Some(rect) = hole_in(geo, i, local, h.size, h.floor) {
            split(geo, i, rect);
            n += 1;
        }
    }
    n
}

pub struct BreachPlugin;

impl Plugin for BreachPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Breaches>()
            .init_resource::<Wear>()
            .add_systems(
                FixedUpdate,
                (restore, tear, wear, blasts)
                    .chain()
                    .in_set(super::DestructionSystems),
            );
    }
}

/// The building's collider and surfaces, rebuilt from its prefab with its
/// holes cut.
fn rebuild(
    commands: &mut Commands,
    e: Entity,
    building: &Building,
    bxf: &Transform,
    holes: &[Hole],
) {
    let Some(mut geo) = crate::world::buildings::prefab(&building.prefab) else {
        return;
    };
    apply(&mut geo, bxf, holes);
    if let Some((collider, surfaces)) = crate::world::buildings::body(&geo) {
        commands.entity(e).insert((collider, surfaces, Breached));
    }
}

/// A restored Zone's holes, as each building appears.
fn restore(
    mut commands: Commands,
    buildings: Query<(Entity, &Building, &Transform), Added<Building>>,
    breaches: Res<Breaches>,
) {
    if breaches.0.is_empty() {
        return;
    }
    for (e, b, t) in &buildings {
        let Some(mut geo) = crate::world::buildings::prefab(&b.prefab) else {
            continue;
        };
        if apply(&mut geo, t, &breaches.0) > 0 {
            rebuild(&mut commands, e, b, t, &breaches.0);
            let inv = t.compute_affine().inverse();
            let base = crate::world::buildings::prefab(&b.prefab);
            for h in breaches.0.iter().filter(|h| h.floor) {
                if base
                    .as_ref()
                    .and_then(|g| wall_at(g, inv.transform_point3(h.at)))
                    .is_some()
                {
                    index_hole(&mut commands, e, &b.prefab, t, h.at);
                }
            }
        }
    }
}

/// Make a hole at `at` (world) in whichever building's wall holds it:
/// the building rebuilt round it, what was there flying along `push`,
/// the crash heard. Returns the hole made, if any.
#[allow(clippy::too_many_arguments)]
fn carve(
    commands: &mut Commands,
    buildings: &Query<(Entity, &Building, &Transform)>,
    breaches: &mut Breaches,
    pool: &mut DebrisPool,
    out: &mut MessageWriter<Broke>,
    at: Vec3,
    size: Vec2,
    floor: bool,
    push: Vec3,
) -> Option<Hole> {
    for (e, b, t) in buildings {
        if !near_building(&b.prefab, t, at, 0.5) {
            continue;
        }
        let Some(mut geo) = crate::world::buildings::prefab(&b.prefab) else {
            continue;
        };
        // As the building stands now, with its holes so far.
        apply(&mut geo, t, &breaches.0);
        let local = t.compute_affine().inverse().transform_point3(at);
        let Some(i) = wall_at(&geo, local) else {
            continue;
        };
        let Some([x0, x1, y0, y1]) = hole_in(&geo, i, local, size, floor) else {
            continue;
        };
        let p = geo.pieces[i];
        let Shape::Box(s) = p.shape else {
            continue;
        };
        let centre = t.transform_point(
            p.at.translation + p.at.rotation * Vec3::new((x0 + x1) * 0.5, (y0 + y1) * 0.5, 0.0),
        );
        let hole = Hole {
            at: centre,
            size: Vec2::new(x1 - x0, y1 - y0),
            floor,
        };
        breaches.0.push(hole);
        rebuild(commands, e, b, t, &breaches.0);
        if floor {
            index_hole(commands, e, &b.prefab, t, centre);
            // A way through: what keeps a map of the way can open it.
            let across = frame_of(t, &p) * Vec3::new(half_of(hole).x, half_of(hole).y, s.z);
            commands.write_message(super::Cleared {
                min: centre - across.abs(),
                max: centre + across.abs(),
            });
        }
        out.write(Broke {
            at: centre,
            stuff: Stuff::Wood,
            size: hole.size.x * hole.size.y * 1.4,
        });
        // What was in the hole flies, the way it was pushed.
        let frame = t.rotation * p.at.rotation;
        let half = hole.size * 0.5;
        let mut rng = Rng::new(super::glass::seed_of(key(centre)) ^ 0xB4EA);
        let pieces = if matches!(p.mat, Mat::Plaster | Mat::SootPlaster) {
            let cells = (6.0 + hole.size.x * hole.size.y * 4.0).min(14.0) as usize;
            let s2 = fracture::shatter(half.to_array(), [0.0, 0.0], rng.next_u64(), cells);
            s2.falling
                .into_iter()
                .chain(s2.kept)
                .map(|sh| {
                    let at = centre + frame * Vec3::new(sh.at[0], sh.at[1], 0.0);
                    let (velocity, spin) = debris::fling(&mut rng, at, centre, push);
                    Piece {
                        kind: Kind::Wood(p.mat),
                        poly: sh.poly,
                        thick: s.z * 0.6,
                        at: Transform::from_translation(at).with_rotation(frame),
                        velocity,
                        spin,
                    }
                })
                .collect()
        } else {
            // Boards, standing in the wall: their length is up the wall.
            let boards = (hole.size.x / 0.16).round().max(2.0) as usize;
            let bw = hole.size.x / boards as f32;
            let up = frame * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
            let mut pieces = Vec::new();
            for k in 0..boards {
                let c = centre + frame * Vec3::new(-half.x + bw * (k as f32 + 0.5), 0.0, 0.0);
                for sh in fracture::splinter([half.y, bw * 0.5], rng.next_u64(), 2) {
                    let at = c + up * Vec3::new(sh.at[0], sh.at[1], 0.0);
                    let (velocity, spin) = debris::fling(&mut rng, at, centre, push);
                    pieces.push(Piece {
                        kind: Kind::Wood(p.mat),
                        poly: sh.poly,
                        thick: 0.025,
                        at: Transform::from_translation(at).with_rotation(up),
                        velocity,
                        spin,
                    });
                }
            }
            pieces
        };
        debris::throw(commands, pool, pieces);
        return Some(hole);
    }
    None
}

fn frame_of(t: &Transform, p: &crate::world::buildings::Piece) -> Quat {
    t.rotation * p.at.rotation
}

fn half_of(h: Hole) -> Vec2 {
    h.size * 0.5
}

/// A heavy's arm tears a doorway out where a breach lands in a wall.
fn tear(
    mut commands: Commands,
    mut breaches_in: MessageReader<Breach>,
    buildings: Query<(Entity, &Building, &Transform)>,
    mut breaches: ResMut<Breaches>,
    mut pool: ResMut<DebrisPool>,
    mut out: MessageWriter<Broke>,
) {
    for br in breaches_in.read() {
        carve(
            &mut commands,
            &buildings,
            &mut breaches,
            &mut pool,
            &mut out,
            br.at,
            Vec2::new(HOLE_WIDTH, HOLE_HEIGHT),
            true,
            br.out.normalize_or_zero() * 4.0,
        );
    }
}

/// Harm taken by patches of wall so far, by the patch's middle (world,
/// centimetres).
#[derive(Resource, Default, Debug)]
pub struct Wear(pub HashMap<[i32; 3], f32>);

/// Rounds and blows wear walls: a patch that's taken its fill gives way.
#[allow(clippy::too_many_arguments)]
fn wear(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    buildings: Query<(Entity, &Building, &Transform)>,
    mut breaches: ResMut<Breaches>,
    mut worn: ResMut<Wear>,
    mut pool: ResMut<DebrisPool>,
    mut out: MessageWriter<Broke>,
    mut geos: Local<HashMap<Entity, (usize, Geo)>>,
) {
    // A building gone (the zone left): its walls with it.
    geos.retain(|e, _| buildings.contains(*e));
    for h in hits.read() {
        let Ok((e, b, t)) = buildings.get(h.entity) else {
            continue;
        };
        // The building as it stands (cached until its holes change).
        let n = breaches.0.len();
        let stale = geos.get(&e).is_none_or(|(k, _)| *k != n);
        if stale {
            let Some(mut geo) = crate::world::buildings::prefab(&b.prefab) else {
                continue;
            };
            apply(&mut geo, t, &breaches.0);
            geos.insert(e, (n, geo));
        }
        let geo = &geos[&e].1;
        let local = t.compute_affine().inverse().transform_point3(h.point);
        let Some(i) = wall_at(geo, local) else {
            continue;
        };
        let p = geo.pieces[i];
        let need = wall_wear(p.mat);
        if !need.is_finite() {
            continue;
        }
        let share = match h.blow {
            None => 1.0,
            Some(_) => super::wood::blow_share(h, false),
        };
        // The patch it struck, on the wall's plane.
        let q = p.at.rotation.inverse() * (local - p.at.translation);
        let cell = Vec2::new(
            ((q.x / PATCH).floor() + 0.5) * PATCH,
            ((q.y / PATCH).floor() + 0.5) * PATCH,
        );
        let centre = t.transform_point(p.at.translation + p.at.rotation * cell.extend(0.0));
        let k = key(centre);
        let total = worn.0.entry(k).or_default();
        *total += h.damage * share;
        if *total < need {
            continue;
        }
        worn.0.remove(&k);
        carve(
            &mut commands,
            &buildings,
            &mut breaches,
            &mut pool,
            &mut out,
            centre,
            SHOT_HOLE,
            false,
            h.velocity.normalize_or_zero() * 2.0,
        );
    }
}

/// A grenade by a wall blows a hole in it: a metre across, down to the
/// floor if it went off at the wall's foot.
fn blasts(
    mut commands: Commands,
    mut events: MessageReader<crate::weapons::grenades::GrenadeEvent>,
    buildings: Query<(Entity, &Building, &Transform)>,
    mut breaches: ResMut<Breaches>,
    mut pool: ResMut<DebrisPool>,
    mut out: MessageWriter<Broke>,
) {
    for ev in events.read() {
        if ev.event != extraction_core::grenade::Event::Exploded {
            continue;
        }
        // The nearest holeable wall within reach.
        let mut best: Option<(f32, Vec3, bool)> = None;
        for (_, b, t) in &buildings {
            if !near_building(&b.prefab, t, ev.at, BLAST_REACH) {
                continue;
            }
            let Some(mut geo) = crate::world::buildings::prefab(&b.prefab) else {
                continue;
            };
            apply(&mut geo, t, &breaches.0);
            let local = t.compute_affine().inverse().transform_point3(ev.at);
            for p in &geo.pieces {
                let Shape::Box(s) = p.shape else { continue };
                if !p.collide || !breachable(p.mat) || s.z > 0.45 || s.y < 0.3 {
                    continue;
                }
                let q = p.at.rotation.inverse() * (local - p.at.translation);
                let h = s * 0.5;
                let near = Vec3::new(q.x.clamp(-h.x, h.x), q.y.clamp(-h.y, h.y), 0.0);
                let d = (q - near).length();
                if d > BLAST_REACH || best.is_some_and(|(bd, ..)| bd <= d) {
                    continue;
                }
                // At the foot of the wall (the floor's within reach): a hole
                // to crawl through.
                let floor = near.y - -h.y < 0.6;
                let at = t.transform_point(p.at.translation + p.at.rotation * near);
                best = Some((d, at, floor));
            }
        }
        let Some((d, at, floor)) = best else {
            continue;
        };
        let size = if floor {
            Vec2::new(BLAST_HOLE.x, 1.2)
        } else {
            BLAST_HOLE
        };
        carve(
            &mut commands,
            &buildings,
            &mut breaches,
            &mut pool,
            &mut out,
            at,
            size,
            floor,
            (at - ev.at).normalize_or_zero() * (6.0 - d * 2.0),
        );
    }
}

/// The Surge: every wall whole again.
pub fn mend_all(world: &mut World) {
    world.resource_mut::<Wear>().0.clear();
    let markers: Vec<Entity> = world
        .query_filtered::<Entity, With<BreachHole>>()
        .iter(world)
        .collect();
    if let Some(mut e) = world.get_resource_mut::<crate::robots::doors::Entrances>() {
        e.doors.retain(|d| !markers.contains(&d.door));
    }
    for m in markers {
        world.despawn(m);
    }
    let holes = std::mem::take(&mut world.resource_mut::<Breaches>().0);
    if holes.is_empty() {
        return;
    }
    let buildings: Vec<(Entity, Building, Transform)> = world
        .query::<(Entity, &Building, &Transform)>()
        .iter(world)
        .map(|(e, b, t)| {
            (
                e,
                Building {
                    prefab: b.prefab.clone(),
                },
                *t,
            )
        })
        .collect();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for (e, b, t) in &buildings {
            let Some(mut geo) = crate::world::buildings::prefab(&b.prefab) else {
                continue;
            };
            if apply(&mut geo, t, &holes) > 0 {
                rebuild(&mut commands, *e, b, t, &[]);
            }
        }
    }
    queue.apply(world);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hole_leaves_the_wall_round_it() {
        let geo = crate::world::buildings::prefab("house_plank").unwrap();
        let mut g = geo.clone();
        // The back wall (building space: +Z is the back), at waist
        // height, clear of the partition meeting it at x 0.
        let at = Vec3::new(0.9, 1.2, 3.5);
        let i = wall_at(&g, at).expect("a plank wall there");
        let before = g.pieces.len();
        let n = apply(&mut g, &Transform::IDENTITY, &[Hole::doorway(at)]);
        assert_eq!(n, 1);
        assert!(g.pieces.len() > before, "the wall either side was kept");
        // Nothing solid where the hole is, from the floor up.
        let solid = |g: &Geo, p: Vec3| {
            g.pieces.iter().any(|q| {
                let Shape::Box(s) = q.shape else { return false };
                q.collide && {
                    let l = q.at.rotation.inverse() * (p - q.at.translation);
                    l.x.abs() < s.x * 0.5 && l.y.abs() < s.y * 0.5 && l.z.abs() < s.z * 0.5
                }
            })
        };
        let p = geo.pieces[i];
        let floor = p.at.translation.y
            - match p.shape {
                Shape::Box(s) => s.y * 0.5,
                _ => 0.0,
            };
        assert!(solid(&geo, Vec3::new(0.9, floor + 1.0, 3.5)));
        assert!(
            !solid(&g, Vec3::new(0.9, floor + 1.0, 3.5)),
            "a hole you walk through"
        );
        assert!(
            !solid(&g, Vec3::new(0.9, floor + 0.1, 3.5)),
            "down to the floor"
        );
        assert!(
            solid(&g, Vec3::new(0.9, floor + 2.6, 3.5)),
            "under what's left over it"
        );
        assert!(
            solid(&g, Vec3::new(-0.8, floor + 1.0, 3.5)),
            "and beside it"
        );
        // The props' indices still name their pieces.
        for d in &g.props {
            for k in d.pieces.clone() {
                assert_eq!(g.pieces[k].mat, geo.pieces[k].mat);
            }
        }
    }

    #[test]
    fn concrete_doesnt_breach() {
        let g = crate::world::buildings::prefab("house_plank").unwrap();
        // The plinth.
        assert!(wall_at(&g, Vec3::new(0.0, 0.2, 0.0)).is_none());
    }
}
