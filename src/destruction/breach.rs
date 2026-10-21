//! Holes torn in walls: the heavy machine's arm (or anything writing a
//! [`Breach`](super::hooks::Breach)) opens a building up.
//!
//! A building's walls are boxes in one compound collider and a few merged
//! meshes. A breach replaces the wall box it lands in with the boxes round
//! a doorway-sized hole (`split`: either side of it, and over it), in the
//! building's prefab as it's built from now on: the collider and its
//! surfaces are rebuilt at once, and the client builds the meshes again
//! (`buildings::redress`). Holes are kept by where they are in the world
//! ([`Breaches`]), saved with the Zone, and mended by the Surge.

use bevy::prelude::*;
use extraction_core::fracture;
use extraction_core::noise::Rng;

use super::debris::{self, DebrisPool, Kind, Piece};
use super::hooks::Breach;
use super::{Broke, Stuff, key};
use crate::render::materials::Mat;
use crate::world::buildings::{Building, Geo, Shape};

/// A hole's width, metres, and how high it goes above the wall's foot.
pub const HOLE_WIDTH: f32 = 1.1;
pub const HOLE_HEIGHT: f32 = 2.0;
/// What's left over a hole, at least, metres (a lintel's worth).
const HEAD: f32 = 0.15;
/// A point this far either side of a wall's centre plane is in it.
const REACH: f32 = 0.3;

/// Every hole torn since the Surge, by its centre in the world.
#[derive(Resource, Default, Debug, Clone)]
pub struct Breaches(pub Vec<Vec3>);

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

/// A building whose walls changed: the client builds its meshes again.
#[derive(Component)]
pub struct Breached;

/// Walls a breach can tear through: boards, logs, plaster and sheet.
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

/// The wall piece holding a point (building space), if it's one a breach
/// can tear: a standing box, thin one way, tall enough to walk through.
pub fn wall_at(geo: &Geo, local: Vec3) -> Option<usize> {
    geo.pieces.iter().position(|p| {
        let Shape::Box(s) = p.shape else {
            return false;
        };
        if !p.collide || !breachable(p.mat) || s.z > 0.45 || s.y < 1.6 {
            return false;
        }
        let q = p.at.rotation.inverse() * (local - p.at.translation);
        q.x.abs() <= s.x * 0.5 && q.y.abs() <= s.y * 0.5 && q.z.abs() <= s.z * 0.5 + REACH
    })
}

/// The hole a breach at `local` cuts in wall piece `i`, in the piece's own
/// space: (x0, x1, y0, y1). None if the piece is too narrow for one.
pub fn hole_in(geo: &Geo, i: usize, local: Vec3) -> Option<(f32, f32, f32, f32)> {
    let p = &geo.pieces[i];
    let Shape::Box(s) = p.shape else {
        return None;
    };
    let (hx, hy) = (s.x * 0.5, s.y * 0.5);
    let q = p.at.rotation.inverse() * (local - p.at.translation);
    let mut x0 = (q.x - HOLE_WIDTH * 0.5).max(-hx);
    let mut x1 = (q.x + HOLE_WIDTH * 0.5).min(hx);
    // No slivers left standing by an edge.
    if x0 + hx < 0.25 {
        x0 = -hx;
    }
    if hx - x1 < 0.25 {
        x1 = hx;
    }
    let y0 = -hy;
    let y1 = (-hy + HOLE_HEIGHT).min(hy - HEAD);
    (x1 - x0 >= 0.4 && y1 - y0 >= 1.0).then_some((x0, x1, y0, y1))
}

/// Cut a hole in wall piece `i`: it becomes what's over the hole, and
/// what's either side of it is added at the end (so the indices the
/// building's props hold stay right).
fn split(geo: &mut Geo, i: usize, hole: (f32, f32, f32, f32)) {
    let p = geo.pieces[i];
    let Shape::Box(s) = p.shape else {
        return;
    };
    let (hx, hy) = (s.x * 0.5, s.y * 0.5);
    let (x0, x1, y0, y1) = hole;
    let piece = |a: f32, b: f32, lo: f32, hi: f32| {
        let mut q = p;
        q.shape = Shape::Box(Vec3::new(b - a, hi - lo, s.z));
        q.at.translation =
            p.at.translation + p.at.rotation * Vec3::new((a + b) * 0.5, (lo + hi) * 0.5, 0.0);
        q
    };
    // Over the hole (always there: holes stop short of the top).
    geo.pieces[i] = piece(x0, x1, y1, hy);
    if x0 > -hx + 0.01 {
        geo.pieces.push(piece(-hx, x0, -hy, hy));
    }
    if x1 < hx - 0.01 {
        geo.pieces.push(piece(x1, hx, -hy, hy));
    }
    let _ = y0;
}

/// A building's prefab with every hole in it cut: what `body` and the
/// meshes are built from. Returns how many holes landed.
pub fn apply(geo: &mut Geo, bxf: &Transform, holes: &[Vec3]) -> usize {
    let inv = bxf.compute_affine().inverse();
    let mut n = 0;
    for h in holes {
        let local = inv.transform_point3(*h);
        let Some(i) = wall_at(geo, local) else {
            continue;
        };
        if let Some(hole) = hole_in(geo, i, local) {
            split(geo, i, hole);
            n += 1;
        }
    }
    n
}

pub struct BreachPlugin;

impl Plugin for BreachPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Breaches>().add_systems(
            FixedUpdate,
            (restore, tear).chain().in_set(super::DestructionSystems),
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
    holes: &[Vec3],
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
            for h in &breaches.0 {
                if base
                    .as_ref()
                    .and_then(|g| wall_at(g, inv.transform_point3(*h)))
                    .is_some()
                {
                    index_hole(&mut commands, e, &b.prefab, t, *h);
                }
            }
        }
    }
}

/// Tear a hole where a breach lands in a wall: the building rebuilt
/// round it, its boards or plaster flying, the crash heard.
#[allow(clippy::too_many_arguments)]
fn tear(
    mut commands: Commands,
    mut breaches_in: MessageReader<Breach>,
    buildings: Query<(Entity, &Building, &Transform)>,
    mut breaches: ResMut<Breaches>,
    mut pool: ResMut<DebrisPool>,
    mut out: MessageWriter<Broke>,
) {
    for br in breaches_in.read() {
        for (e, b, t) in &buildings {
            let Some(mut geo) = crate::world::buildings::prefab(&b.prefab) else {
                continue;
            };
            // As the building stands now, with its holes so far.
            apply(&mut geo, t, &breaches.0);
            let local = t.compute_affine().inverse().transform_point3(br.at);
            let Some(i) = wall_at(&geo, local) else {
                continue;
            };
            let Some(hole) = hole_in(&geo, i, local) else {
                continue;
            };
            let p = geo.pieces[i];
            let Shape::Box(s) = p.shape else {
                continue;
            };
            // The hole's centre in the world, on the wall's plane.
            let (x0, x1, y0, y1) = hole;
            let centre_local =
                p.at.translation + p.at.rotation * Vec3::new((x0 + x1) * 0.5, (y0 + y1) * 0.5, 0.0);
            let centre = t.transform_point(centre_local);
            breaches.0.push(centre);
            rebuild(&mut commands, e, b, t, &breaches.0);
            index_hole(&mut commands, e, &b.prefab, t, centre);
            out.write(Broke {
                at: centre,
                stuff: Stuff::Wood,
                size: 3.0,
            });
            // What was in the hole flies, the way it was pulled.
            let frame = t.rotation * p.at.rotation;
            let half = Vec2::new((x1 - x0) * 0.5, (y1 - y0) * 0.5);
            let mut rng = Rng::new(super::glass::seed_of(key(centre)) ^ 0xB4EA);
            let push = br.out.normalize_or_zero() * 4.0;
            let pieces = if matches!(p.mat, Mat::Plaster | Mat::SootPlaster) {
                let s2 = fracture::shatter(half.to_array(), [0.0, 0.0], rng.next_u64(), 14);
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
                let boards = ((x1 - x0) / 0.16).round().max(2.0) as usize;
                let bw = (x1 - x0) / boards as f32;
                let up = frame * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
                let mut out = Vec::new();
                for k in 0..boards {
                    let c = centre + frame * Vec3::new(-half.x + bw * (k as f32 + 0.5), 0.0, 0.0);
                    for sh in fracture::splinter([half.y, bw * 0.5], rng.next_u64(), 2) {
                        let at = c + up * Vec3::new(sh.at[0], sh.at[1], 0.0);
                        let (velocity, spin) = debris::fling(&mut rng, at, centre, push);
                        out.push(Piece {
                            kind: Kind::Wood(p.mat),
                            poly: sh.poly,
                            thick: 0.025,
                            at: Transform::from_translation(at).with_rotation(up),
                            velocity,
                            spin,
                        });
                    }
                }
                out
            };
            debris::throw(&mut commands, &mut pool, pieces);
            break;
        }
    }
}

/// The Surge: every wall whole again.
pub fn mend_all(world: &mut World) {
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
        let n = apply(&mut g, &Transform::IDENTITY, &[at]);
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
