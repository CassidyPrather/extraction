//! Boulders and outcrops (`extraction_core::rocks`): colliders for the
//! logic (the big ones stop you, bullets and robots' eyes), and faceted
//! meshes for the client, merged per chunk with a coarser LOD further out.

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use extraction_core::rocks::{ROCK_VARIANTS, RockSpot, SOLID_RADIUS, scatter_rocks};

use super::{Layer, Zone};
use crate::render::materials::{Mat, MaterialLibrary};

/// Chunk size for merged rock meshes and compound colliders (small enough
/// that a chunk's distance is near enough each rock's for the LOD bands).
const CHUNK: f32 = 32.0;
/// Boulders: fine (ico 2) to here, then medium (ico 1)...
const NEAR_LOD: f32 = 60.0;
/// ...then coarse (ico 0), which casts no shadow...
const FAR_LOD: f32 = 130.0;
/// ...and past this they're gone (the fog has them).
const CULL: f32 = 420.0;
/// Field stones: medium to here, coarse to `STONES`, never a shadow.
const STONES_NEAR: f32 = 40.0;
const STONES: f32 = 90.0;

pub fn rocks(zone: &Zone) -> Vec<RockSpot> {
    scatter_rocks(&zone.map, &zone.hm)
}

fn scale_of(r: &RockSpot) -> Vec3 {
    Vec3::new(
        r.radius * r.stretch,
        r.radius * r.squash,
        r.radius / r.stretch,
    )
}

fn chunk_of(p: [f32; 3]) -> (i32, i32) {
    ((p[0] / CHUNK).floor() as i32, (p[2] / CHUNK).floor() as i32)
}

fn hash(v: u64) -> f32 {
    let mut r = extraction_core::noise::Rng::new(v);
    r.f32()
}

/// A rock's shape: an icosphere pushed in and out by a few lumps and
/// planed flat on a couple of sides, unit size. `detail` is the
/// icosphere's subdivision (2 near, 1 far, 1 for colliders).
pub fn rock_points(variant: usize, detail: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let sphere = Sphere::new(1.0).mesh().ico(detail).unwrap();
    let pos: Vec<Vec3> = sphere
        .attribute(Mesh::ATTRIBUTE_POSITION)
        .and_then(|a| a.as_float3())
        .unwrap()
        .iter()
        .map(|p| Vec3::from_array(*p))
        .collect();
    let idx: Vec<u32> = match sphere.indices() {
        Some(Indices::U32(v)) => v.clone(),
        Some(Indices::U16(v)) => v.iter().map(|i| *i as u32).collect(),
        None => (0..pos.len() as u32).collect(),
    };
    let seed = 0x0B01_DE25 + variant as u64 * 977;
    // Lumps: a handful of random directions that push the surface out or in.
    let lumps: Vec<(Vec3, f32)> = (0..9)
        .map(|k| {
            let s = seed + k * 31;
            let d = Vec3::new(hash(s) - 0.5, hash(s + 1) - 0.5, hash(s + 2) - 0.5)
                .normalize_or(Vec3::Y);
            (d, (hash(s + 3) - 0.45) * 0.8)
        })
        .collect();
    // Planes that shear off a side (fractured faces), and the flat base.
    let cuts: Vec<(Vec3, f32)> = (0..4)
        .map(|k| {
            let s = seed + 500 + k * 17;
            let d = Vec3::new(hash(s) - 0.5, (hash(s + 1) - 0.2) * 0.9, hash(s + 2) - 0.5)
                .normalize_or(Vec3::X);
            (d, 0.6 + 0.25 * hash(s + 3))
        })
        .chain(std::iter::once((Vec3::NEG_Y, 0.55)))
        .collect();
    let pos = pos
        .into_iter()
        .map(|d| {
            let mut r = 1.0;
            for (l, a) in &lumps {
                r += a * d.dot(*l).max(0.0).powi(3);
            }
            let mut p = d * r.max(0.4);
            for (n, at) in &cuts {
                let over = p.dot(*n) - at;
                if over > 0.0 {
                    p -= *n * over;
                }
            }
            p
        })
        .collect();
    let tris = idx.chunks(3).map(|t| [t[0], t[1], t[2]]).collect();
    (pos, tris)
}

/// A rock's faces: positions, normals, box-projected UVs (metres), and a
/// colour (multiplying the texture).
type RockFaces = (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<[f32; 4]>);

/// A faceted mesh for one rock placed in the world: flat-shaded faces,
/// each textured by the axis it faces most. No two rocks are quite the
/// same tone, and the faces that look at the sky carry moss (or, in a
/// burnt valley, soot).
fn rock_mesh(r: &RockSpot, detail: u32, burnt: bool) -> RockFaces {
    let (pts, tris) = rock_points(r.variant, detail);
    let xf = Transform::from_xyz(r.pos[0], r.pos[1], r.pos[2])
        .with_rotation(Quat::from_rotation_y(r.yaw))
        .with_scale(scale_of(r));
    let mut pos = Vec::with_capacity(tris.len() * 3);
    let mut nrm = Vec::with_capacity(tris.len() * 3);
    let mut uv = Vec::with_capacity(tris.len() * 3);
    let mut col = Vec::with_capacity(tris.len() * 3);
    let key = (r.pos[0] * 13.0 + r.pos[2] * 7.0) as i64 as u64;
    // (The texture is a cool grey; the tint warms it, and a burnt valley's
    // rocks are smoke-dark.)
    let tone = (0.7 + 0.35 * hash(key)) * if burnt { 0.72 } else { 1.0 };
    let warm = (hash(key.wrapping_add(1)) - 0.5) * 0.12;
    let moss = hash(key.wrapping_add(2)) * 0.9;
    let base = [tone * (1.0 + warm), tone, tone * (1.0 - warm)];
    let top = if burnt {
        [0.32, 0.3, 0.28]
    } else {
        [0.55, 0.66, 0.38]
    };
    for t in tris {
        let w = t.map(|i| xf.transform_point(pts[i as usize]));
        let n = (w[1] - w[0]).cross(w[2] - w[0]).normalize_or(Vec3::Y);
        let a = n.abs();
        let up = ((n.y - 0.45) / 0.45).clamp(0.0, 1.0) * moss;
        let c = [0, 1, 2].map(|i| base[i] * (1.0 - up) + base[i] * top[i] * up);
        for p in w {
            col.push([c[0], c[1], c[2], 1.0]);
            pos.push(p.to_array());
            nrm.push(n.to_array());
            uv.push(if a.y >= a.x && a.y >= a.z {
                [p.x, p.z]
            } else if a.x >= a.z {
                [p.z, p.y]
            } else {
                [p.x, p.y]
            });
        }
    }
    (pos, nrm, uv, col)
}

/// Logic: a compound collider of convex hulls per chunk, for the rocks big
/// enough to matter.
pub fn spawn_rock_colliders(mut commands: Commands, zone: Res<Zone>) {
    let started = std::time::Instant::now();
    let all = rocks(&zone);
    let hulls: Vec<Vec<Vec3>> = (0..ROCK_VARIANTS).map(|v| rock_points(v, 1).0).collect();
    let mut chunks: std::collections::HashMap<(i32, i32), Vec<(Position, Rotation, Collider)>> =
        Default::default();
    for r in all.iter().filter(|r| r.radius >= SOLID_RADIUS) {
        let scale = scale_of(r);
        let pts: Vec<Vec3> = hulls[r.variant].iter().map(|p| *p * scale).collect();
        let Some(hull) = Collider::convex_hull(pts) else {
            continue;
        };
        let (cx, cz) = chunk_of(r.pos);
        let origin = Vec3::new(cx as f32 * CHUNK, 0.0, cz as f32 * CHUNK);
        chunks.entry((cx, cz)).or_default().push((
            Position(Vec3::from_array(r.pos) - origin),
            Rotation(Quat::from_rotation_y(r.yaw)),
            hull,
        ));
    }
    for ((cx, cz), shapes) in chunks {
        commands.spawn((
            Name::new("Rocks"),
            Transform::from_xyz(cx as f32 * CHUNK, 0.0, cz as f32 * CHUNK),
            RigidBody::Static,
            Collider::compound(shapes),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
    info!(
        "{} rocks scattered in {:.0} ms",
        all.len(),
        started.elapsed().as_secs_f32() * 1000.0
    );
}

pub struct RockVisualsPlugin;

impl Plugin for RockVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_rock_meshes);
    }
}

/// Client: one merged mesh per chunk, near and far.
fn spawn_rock_meshes(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    mut done: Local<bool>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    opts: Res<crate::launch::LaunchOptions>,
) {
    let Some(zone) = zone else {
        return;
    };
    if *done {
        return;
    }
    *done = true;
    let mat = library.get(Mat::Boulder, &server, &mut materials);
    let mut chunks: std::collections::HashMap<(i32, i32), Vec<RockSpot>> = Default::default();
    for r in rocks(&zone) {
        // Lower detail thins the field stones (they're only looks).
        if r.radius < SOLID_RADIUS
            && hash((r.pos[0] * 7.0 + r.pos[2] * 131.0) as i64 as u64) > opts.detail
        {
            continue;
        }
        chunks.entry(chunk_of(r.pos)).or_default().push(r);
    }
    use bevy::camera::visibility::VisibilityRange;
    let range = |from: f32, to: f32| VisibilityRange {
        start_margin: if from > 0.0 {
            (from - 10.0)..(from + 10.0)
        } else {
            0.0..0.0
        },
        end_margin: (to - 10.0)..(to + 10.0),
        use_aabb: true,
    };
    for (_, list) in chunks {
        let (small, big): (Vec<&RockSpot>, Vec<&RockSpot>) =
            list.iter().partition(|r| r.radius < SOLID_RADIUS);
        // Boulders: three LODs. Field stones: two, and only near (they're
        // grain in the ground, not landmarks). Shadows only from what's
        // near enough to show one.
        for (rocks, detail, range, shadow) in [
            (&big, 2, range(0.0, NEAR_LOD), true),
            (&big, 1, range(NEAR_LOD, FAR_LOD), true),
            (&big, 0, range(FAR_LOD, CULL), false),
            (&small, 1, range(0.0, STONES_NEAR), false),
            (&small, 0, range(STONES_NEAR, STONES), false),
        ] {
            if rocks.is_empty() {
                continue;
            }
            let (mut pos, mut nrm, mut uv, mut col) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for r in rocks {
                let (p, n, u, c) = rock_mesh(r, detail, zone.map.ash > 0.0);
                pos.extend(p);
                nrm.extend(n);
                uv.extend(u);
                col.extend(c);
            }
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col);
            let _ = mesh.generate_tangents();
            let mut e = commands.spawn((
                Name::new("Rock chunk"),
                crate::render::centred(&mut meshes, mesh),
                MeshMaterial3d(mat.clone()),
                range,
            ));
            if !shadow {
                e.insert(bevy::light::NotShadowCaster);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rocks_are_closed_lumpy_and_sit_on_a_flat_base() {
        for v in 0..ROCK_VARIANTS {
            let (pts, tris) = rock_points(v, 2);
            assert!(tris.len() >= 150, "variant {v}: {} faces", tris.len());
            let low = pts.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
            assert!((low + 0.55).abs() < 0.02, "variant {v}: base at {low}");
            let radii: Vec<f32> = pts.iter().map(|p| p.length()).collect();
            let spread = radii.iter().cloned().fold(0.0, f32::max)
                - radii.iter().cloned().fold(f32::INFINITY, f32::min);
            assert!(spread > 0.15, "variant {v} isn't a ball: spread {spread}");
            assert!(Collider::convex_hull(rock_points(v, 1).0).is_some());
        }
    }
}
