//! The plough's bank, drawn (client only): its sections
//! ([`BankPiece`], one to each [`BANK_STEP`] of road) swept into one
//! windrow of snow along the road, lumpy, higher and lower, wider and
//! narrower as it goes, its toes tucked under the ground it lies on, and
//! tapered off where it stops (a junction, a hut's door, a gallery). It's
//! built in chunks of road, so a new section rebuilds only the chunks
//! around it.

use std::collections::{HashMap, HashSet};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::plough::{BankPiece, OfPlough, Plough};
use crate::render::materials::{Mat, MaterialLibrary};
use crate::world::Zone;

/// Slots of road to a chunk of bank.
const CHUNK: i32 = 8;
/// Rings across the bank to each section (every half metre).
const STEPS: usize = 8;
/// Quads across the bank.
const ACROSS: usize = 12;
/// How far past its last section (m) a bank tapers off.
const TAPER: f32 = 2.5;
/// How far under the ground (m) its toes are tucked.
pub const SINK: f32 = 0.12;

pub(super) fn add_to(app: &mut App) {
    app.init_resource::<Banks>()
        .add_systems(Update, sweep_banks);
}

/// A section of bank as the sweep sees it: where it lies (x/z), along
/// the road, and its own height and half-width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub at: Vec2,
    pub fwd: Vec2,
    pub high: f32,
    pub half: f32,
}

impl Piece {
    /// From a section's transform: its size from where it lies, so the
    /// same road always banks the same way.
    pub fn new(t: &Transform) -> Piece {
        let at = Vec2::new(t.translation.x, t.translation.z);
        let f = t.rotation * Vec3::NEG_Z;
        let mut h = (at.x * 100.0).round() as i64 as u64
            ^ ((at.y * 100.0).round() as i64 as u64).rotate_left(21);
        let mut next = || {
            h = h
                .wrapping_mul(0x5851_f42d_4c95_7f2d)
                .wrapping_add(0x1405_7b7e_f767_814f);
            ((h >> 40) as f32) / (1u64 << 24) as f32
        };
        let (high, wide) = (next(), next());
        Piece {
            at,
            fwd: Vec2::new(f.x, f.z).normalize_or(Vec2::NEG_Y),
            high: 0.7 + 0.55 * high,
            half: 0.85 + 0.4 * wide,
        }
    }

    fn ctl(&self) -> Vec4 {
        Vec4::new(self.at.x, self.at.y, self.high, self.half)
    }

    /// Where a bank ends: past this section along `way` (+1 on, -1 back),
    /// down to nothing.
    fn end(&self, way: f32) -> Vec4 {
        let p = self.at + self.fwd * TAPER * way;
        Vec4::new(p.x, p.y, 0.0, self.half * 0.7)
    }
}

/// Each plough's bank (and the gallery's), by its sections' slots.
#[derive(Resource, Default)]
struct Banks {
    owners: HashMap<Option<Entity>, Bank>,
    /// Which bank and slot each section is (to drop it when it goes).
    of: HashMap<Entity, (Option<Entity>, i32)>,
}

#[derive(Default)]
struct Bank {
    pieces: HashMap<i32, Piece>,
    chunks: HashMap<i32, Entity>,
}

/// A plough whose bank has its seed: a scrap of bank, the same mesh
/// layout and snow, hung out of sight under it from the start, so the
/// warm-up shelf (`render::warmup`) builds the bank's pipelines while
/// the player is still in the truck, not on the frame the first bank is
/// thrown up mid-raid.
#[derive(Component)]
struct Seeded;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sweep_banks(
    mut commands: Commands,
    unseeded: Query<Entity, (With<Plough>, Without<Seeded>)>,
    added: Query<(Entity, &BankPiece, &Transform, Option<&OfPlough>), Added<BankPiece>>,
    mut gone: RemovedComponents<BankPiece>,
    mut banks: ResMut<Banks>,
    zone: Option<Res<Zone>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(zone) = zone else {
        return;
    };
    for e in &unseeded {
        let seed = Piece::new(&Transform::IDENTITY);
        let rings = chunk_rings(|k| (k == 0).then_some(seed), 0, |_, _| -6.0);
        if let Some(mesh) = ring_mesh(&rings) {
            let snow = lib.get(Mat::Snow, &server, &mut materials);
            commands.spawn((
                Name::new("Plough bank seed"),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(snow),
                Transform::IDENTITY,
                Visibility::default(),
                ChildOf(e),
            ));
        }
        commands.entity(e).insert(Seeded);
    }
    let Banks { owners, of } = &mut *banks;
    let mut dirty = HashSet::new();
    // A change at a slot moves the segments that reach it.
    let mut mark = |o: Option<Entity>, slot: i32| {
        for k in slot - 2..=slot + 1 {
            dirty.insert((o, k.div_euclid(CHUNK)));
        }
    };
    for e in gone.read() {
        if let Some((o, slot)) = of.remove(&e) {
            if let Some(b) = owners.get_mut(&o) {
                b.pieces.remove(&slot);
            }
            mark(o, slot);
        }
    }
    for (e, piece, t, owner) in &added {
        let o = owner.map(|p| p.0);
        owners
            .entry(o)
            .or_default()
            .pieces
            .insert(piece.slot, Piece::new(t));
        of.insert(e, (o, piece.slot));
        mark(o, piece.slot);
    }
    if dirty.is_empty() {
        return;
    }
    let snow = lib.get(Mat::Snow, &server, &mut materials);
    for (o, chunk) in dirty {
        let Some(bank) = owners.get_mut(&o) else {
            continue;
        };
        if let Some(old) = bank.chunks.remove(&chunk) {
            commands.entity(old).try_despawn();
        }
        let pieces = &bank.pieces;
        let rings = chunk_rings(|k| pieces.get(&k).copied(), chunk, |x, z| zone.ground(x, z));
        if let Some(mesh) = ring_mesh(&rings) {
            let id = commands
                .spawn((
                    Name::new("Plough bank"),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(snow.clone()),
                    Transform::IDENTITY,
                    Visibility::default(),
                ))
                .id();
            bank.chunks.insert(chunk, id);
        }
        if bank.pieces.is_empty() && bank.chunks.is_empty() {
            owners.remove(&o);
        }
    }
}

/// A point on the bank's surface: its rings' vertex and normal.
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub at: Vec3,
    pub normal: Vec3,
    /// Across the bank, -1 to 1 (its toes at ±1).
    pub u: f32,
}

/// The segments of bank chunk `chunk` draws: for each, its rings from
/// one end to the other, each ring across the bank. `get` is the bank's
/// section at a slot, `ground` the ground's height at x/z.
pub fn chunk_rings(
    get: impl Fn(i32) -> Option<Piece>,
    chunk: i32,
    ground: impl Fn(f32, f32) -> f32,
) -> Vec<Vec<Vec<Vertex>>> {
    let mut out = Vec::new();
    let p = |k: i32| get(k).map(|p| p.ctl());
    for k in chunk * CHUNK..(chunk + 1) * CHUNK {
        let Some(here) = get(k) else {
            continue;
        };
        let before = |k: i32| get(k - 1).map_or_else(|| get(k).unwrap().end(-1.0), |p| p.ctl());
        let after = |k: i32| get(k + 1).map_or_else(|| get(k).unwrap().end(1.0), |p| p.ctl());
        // Where the bank starts, a taper in to this section.
        if get(k - 1).is_none() {
            let (a, b) = (here.end(-1.0), here.ctl());
            out.push(segment([2.0 * a - b, a, b, after(k)], &ground));
        }
        match p(k + 1) {
            Some(next) => {
                let last = if get(k + 2).is_some() {
                    p(k + 2).unwrap()
                } else {
                    get(k + 1).unwrap().end(1.0)
                };
                out.push(segment([before(k), here.ctl(), next, last], &ground));
            }
            // Where it stops, a taper out.
            None => {
                let (a, b) = (here.ctl(), here.end(1.0));
                out.push(segment([before(k), a, b, 2.0 * b - a], &ground));
            }
        }
    }
    out
}

/// Catmull-Rom through four control points (x, z, height, half-width).
fn spline(c: &[Vec4; 4], t: f32) -> Vec4 {
    let (t2, t3) = (t * t, t * t * t);
    0.5 * (2.0 * c[1]
        + (c[2] - c[0]) * t
        + (2.0 * c[0] - 5.0 * c[1] + 4.0 * c[2] - c[3]) * t2
        + (3.0 * c[1] - c[0] - 3.0 * c[2] + c[3]) * t3)
}

/// The bank's ring at `t` along a segment: its points across it, toe to
/// toe.
fn ring(c: &[Vec4; 4], t: f32, ground: &impl Fn(f32, f32) -> f32) -> [Vec3; ACROSS + 1] {
    let s = spline(c, t);
    let ahead = spline(c, t + 0.01) - spline(c, t - 0.01);
    let along = Vec2::new(ahead.x, ahead.y).normalize_or(Vec2::X);
    let right = Vec2::new(-along.y, along.x);
    let (high, half) = (s.z.max(0.0), s.w.max(0.05));
    let centre = Vec2::new(s.x, s.y);
    // Its lumps and the wander of its crest, from where it is.
    let w = centre.dot(Vec2::new(0.83, 0.56));
    let v = centre.dot(Vec2::new(-0.41, 0.91));
    let wander = 0.12 * (0.7 * w + 0.4).sin() * half;
    // (Where it tapers to nothing it's down under the ground with its toes.)
    let lift = high + SINK * (high / 0.3).min(1.0);
    std::array::from_fn(|j| {
        let u = -1.0 + 2.0 * j as f32 / ACROSS as f32;
        let prof = if u.abs() < 0.999 {
            (std::f32::consts::FRAC_PI_2 * u).cos().powf(0.75)
        } else {
            0.0
        };
        let lump = 0.1 * (1.1 * w + 1.3 * u + 0.5).sin()
            + 0.07 * (2.3 * v - 2.0 * u + 1.9).sin()
            + 0.04 * (4.1 * w + 4.0 * u + 0.3).sin()
            + 0.03 * (6.7 * v - 6.0 * u + 2.6).sin();
        let xz = centre + right * (u * half + wander * prof);
        let y = ground(xz.x, xz.y) - SINK + lift * prof * (1.0 + lump);
        Vec3::new(xz.x, y, xz.y)
    })
}

/// A segment's rings, its normals from the rings either side (one past
/// each end, so they agree with the next segment's where they meet).
fn segment(c: [Vec4; 4], ground: &impl Fn(f32, f32) -> f32) -> Vec<Vec<Vertex>> {
    let rings: Vec<[Vec3; ACROSS + 1]> = (0..STEPS + 3)
        .map(|i| ring(&c, (i as f32 - 1.0) / STEPS as f32, ground))
        .collect();
    (1..=STEPS + 1)
        .map(|i| {
            (0..=ACROSS)
                .map(|j| {
                    let at = rings[i][j];
                    let dt = rings[i + 1][j] - rings[i - 1][j];
                    let du = rings[i][(j + 1).min(ACROSS)] - rings[i][j.saturating_sub(1)];
                    let mut normal = dt.cross(du).normalize_or(Vec3::Y);
                    if normal.y < 0.0 {
                        normal = -normal;
                    }
                    let u = -1.0 + 2.0 * j as f32 / ACROSS as f32;
                    Vertex { at, normal, u }
                })
                .collect()
        })
        .collect()
}

/// The segments' rings as one mesh (none if there are none).
pub fn ring_mesh(segments: &[Vec<Vec<Vertex>>]) -> Option<Mesh> {
    if segments.is_empty() {
        return None;
    }
    let (mut pos, mut nor, mut uv, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for rings in segments {
        for (i, ring) in rings.iter().enumerate() {
            let base = pos.len() as u32;
            for v in ring {
                pos.push(v.at.to_array());
                nor.push(v.normal.to_array());
                uv.push([v.u * 0.5 + 0.5, i as f32 / STEPS as f32]);
            }
            if i > 0 {
                let w = ring.len() as u32;
                let prev = base - w;
                for j in 0..w - 1 {
                    let (a, b, c, d) = (prev + j, prev + j + 1, base + j, base + j + 1);
                    idx.extend([a, c, b, b, c, d]);
                }
            }
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh.insert_indices(Indices::U32(idx));
    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slope rising to the east and gently north.
    fn slope(x: f32, z: f32) -> f32 {
        0.12 * x + 0.05 * z + 0.3 * (0.2 * z).sin()
    }

    /// Sections every 4 m north along x = 10, at `slots`.
    fn road(slots: &[i32]) -> impl Fn(i32) -> Option<Piece> + '_ {
        move |k| {
            slots.contains(&k).then(|| {
                let z = -4.0 * k as f32;
                Piece::new(&Transform::from_xyz(10.0, slope(10.0, z), z))
            })
        }
    }

    /// From one chunk to the next the bank runs on unbroken: the ring a
    /// chunk ends on is the ring the next starts from.
    #[test]
    fn the_bank_runs_on_unbroken_between_its_chunks() {
        let slots: Vec<i32> = (0..20).collect();
        let a = chunk_rings(road(&slots), 0, slope);
        let b = chunk_rings(road(&slots), 1, slope);
        // Chunk 0 starts with a taper, then sections 0→1 … 7→8.
        assert_eq!(a.len(), 9);
        assert_eq!(b.len(), 8);
        let end = a.last().unwrap().last().unwrap();
        let start = b.first().unwrap().first().unwrap();
        for (x, y) in end.iter().zip(start) {
            assert!(x.at.distance(y.at) < 1e-3, "{:?} vs {:?}", x.at, y.at);
            assert!(x.normal.dot(y.normal) > 0.999, "a crease between chunks");
        }
    }

    /// It lies on the ground: its toes just under it all along, its crest
    /// a man's waist to chest above it, and it tapers into the ground at
    /// its ends and either side of a gap.
    #[test]
    fn the_bank_lies_on_the_ground_and_tapers_at_its_ends() {
        let slots = [0, 1, 2, 3, 5, 6, 7];
        let segs = chunk_rings(road(&slots), 0, slope);
        // Two runs: in, 0→1, 1→2, 2→3, out; in, 5→6, 6→7, out.
        assert_eq!(segs.len(), 9);
        for ring in segs.iter().flatten() {
            for v in ring {
                let g = slope(v.at.x, v.at.z);
                assert!(v.at.y >= g - SINK - 1e-3, "under the ground at {:?}", v.at);
                if v.u.abs() > 0.999 {
                    assert!((v.at.y - (g - SINK)).abs() < 1e-3, "a toe off the ground");
                }
            }
        }
        let crest = |ring: &Vec<Vertex>| {
            ring.iter()
                .map(|v| v.at.y - slope(v.at.x, v.at.z))
                .fold(f32::MIN, f32::max)
        };
        for seg in &segs[1..4] {
            let c = crest(&seg[STEPS / 2]);
            assert!((0.55..1.6).contains(&c), "a crest {c:.2} m high");
        }
        // Each run's very ends are down in the ground.
        for (seg, ring) in [(0, 0), (4, STEPS), (5, 0), (8, STEPS)] {
            let c = crest(&segs[seg][ring]);
            assert!(c < 0.01, "an end {c:.2} m proud");
        }
    }

    /// No sections, no bank; one section is still a mound.
    #[test]
    fn a_lone_section_is_a_mound() {
        assert!(ring_mesh(&chunk_rings(road(&[]), 0, slope)).is_none());
        let segs = chunk_rings(road(&[3]), 0, slope);
        assert_eq!(segs.len(), 2);
        let mesh = ring_mesh(&segs).unwrap();
        assert_eq!(
            mesh.count_vertices(),
            2 * (STEPS + 1) * (ACROSS + 1),
            "two segments' rings"
        );
    }

    /// What a new section costs: the chunks round it rebuilt (up to two
    /// whole chunks of road, 16 segments). Measured, not held: the
    /// numbers go in the report.
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_a_new_section_rebuilds() {
        let slots: Vec<i32> = (0..40).collect();
        let n = 200;
        let t = std::time::Instant::now();
        for _ in 0..n {
            for chunk in [1, 2] {
                std::hint::black_box(ring_mesh(&chunk_rings(road(&slots), chunk, slope)));
            }
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("a new section: two chunks rebuilt in {ms:.3} ms");
    }
}
