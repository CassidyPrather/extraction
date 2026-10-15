//! Terrain meshes: the heightmap cut into chunks, with per-vertex surface
//! weights the terrain shader blends between.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use extraction_core::map::RoadKind;

use super::Zone;
use crate::render::terrain_material::{TerrainMaterial, terrain_material};

/// Metres per chunk side.
const CHUNK: usize = 64;

/// Detail levels: vertex spacing in cells, and how far out (from the
/// chunk's centre) each takes over. Coarser chunks hang skirts round their
/// edges so the cracks between neighbours at different levels don't show.
const LODS: [(usize, f32); 3] = [(1, 0.0), (2, 140.0), (4, 260.0)];
/// Until the weather says otherwise, chunks vanish this far out.
const FOG_CULL_DEFAULT: f32 = 760.0;

pub struct TerrainVisualsPlugin;

impl Plugin for TerrainVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_terrain_meshes)
            .add_systems(PostUpdate, fog_cull);
    }
}

#[derive(Component)]
pub struct TerrainChunk {
    lod: usize,
}

/// The distance band level `lod` is drawn in; the last level ends at the
/// fog.
fn lod_range(lod: usize, fog: f32) -> bevy::camera::visibility::VisibilityRange {
    let start = LODS[lod].1;
    let end = LODS.get(lod + 1).map_or(fog, |l| l.1);
    bevy::camera::visibility::VisibilityRange {
        start_margin: start..start,
        end_margin: end..end,
        use_aabb: true,
    }
}

/// Past what the weather lets you see (well past: the fog's last few
/// percent still show a hill's line), terrain isn't drawn.
fn fog_cull(
    weather: Res<super::sky::Weather>,
    mut chunks: Query<(
        &TerrainChunk,
        &mut bevy::camera::visibility::VisibilityRange,
    )>,
    added: Query<(), Added<TerrainChunk>>,
) {
    if !weather.is_changed() && added.is_empty() {
        return;
    }
    let fog = weather.visibility * 1.3 + CHUNK as f32 * 0.7;
    for (c, mut r) in &mut chunks {
        let want = lod_range(c.lod, fog);
        if r.end_margin != want.end_margin {
            *r = want;
        }
    }
}

fn spawn_terrain_meshes(
    mut commands: Commands,
    zone: Res<Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let material = materials.add(terrain_material(&asset_server));
    let hm = &zone.hm;
    let cells = hm.n - 1;
    let step = (CHUNK as f32 / hm.cell).round() as usize;
    let mut j0 = 0;
    while j0 < cells {
        let mut i0 = 0;
        while i0 < cells {
            let i1 = (i0 + step).min(cells);
            let j1 = (j0 + step).min(cells);
            for (lod, (spacing, _)) in LODS.iter().enumerate() {
                let mesh = chunk_mesh(&zone, i0, j0, i1, j1, *spacing);
                commands.spawn((
                    Name::new("Terrain chunk"),
                    TerrainChunk { lod },
                    crate::render::centred(&mut meshes, mesh),
                    MeshMaterial3d(material.clone()),
                    lod_range(lod, FOG_CULL_DEFAULT),
                    // Under this sky the land's own shadows are faint, and
                    // the cascades are drawn per eye: it takes shadows
                    // (trees, buildings) but casts none.
                    bevy::light::NotShadowCaster,
                ));
            }
            i0 = i1;
        }
        j0 += step;
    }
}

/// Splat weights packed into vertex colour: r = bare dirt/mud, g = rock,
/// b = road gravel/asphalt, a = forest floor (needles, moss). Grass is
/// whatever is left over. Road kind goes in UV1.x so the shader can pick
/// asphalt vs gravel vs dirt.
fn chunk_mesh(zone: &Zone, i0: usize, j0: usize, i1: usize, j1: usize, spacing: usize) -> Mesh {
    let hm = &zone.hm;
    // Sample rows and columns every `spacing` cells, always ending on the
    // chunk's edge.
    let ticks = |a: usize, b: usize| {
        let mut v: Vec<usize> = (a..b).step_by(spacing).collect();
        v.push(b);
        v
    };
    let (is, js) = (ticks(i0, i1), ticks(j0, j1));
    let (w, h) = (is.len(), js.len());
    let mut positions = Vec::with_capacity(w * h);
    let mut normals = Vec::with_capacity(w * h);
    let mut uvs = Vec::with_capacity(w * h);
    let mut uv1 = Vec::with_capacity(w * h);
    let mut colors = Vec::with_capacity(w * h);
    for &j in &js {
        for &i in &is {
            let p = hm.pos(i, j);
            let y = hm.heights[hm.idx(i, j)];
            positions.push([p[0], y, p[1]]);
            normals.push(hm.normal(p));
            // World-space UVs in metres; the shader scales per layer.
            uvs.push([p[0], p[1]]);
            // Coarse vertices take the strongest road paint around them,
            // so a road doesn't break up into patches far off.
            let mut road = hm.roads[hm.idx(i, j)];
            if spacing > 1 {
                let r = spacing / 2;
                for jj in j.saturating_sub(r)..=(j + r).min(hm.n - 1) {
                    for ii in i.saturating_sub(r)..=(i + r).min(hm.n - 1) {
                        let q = hm.roads[hm.idx(ii, jj)];
                        if q.weight > road.weight {
                            road = q;
                        }
                    }
                }
            }
            let road_w = road.weight as f32 / 255.0;
            let kind = match road.kind {
                Some(RoadKind::Asphalt) => 1.0,
                Some(RoadKind::Gravel) => 2.0,
                Some(RoadKind::Rail) => 3.0,
                Some(RoadKind::Dirt) => 4.0,
                Some(RoadKind::Path) => 5.0,
                None => 0.0,
            };
            uv1.push([kind, hm.wet[hm.idx(i, j)]]);
            let slope = hm.slope_deg(p);
            let rock = smooth(28.0, 42.0, slope);
            let wet = hm.wet[hm.idx(i, j)];
            let forest = if zone.in_forest(p[0], p[1]) { 1.0 } else { 0.0 };
            colors.push([wet.max(0.15 * rock), rock, road_w, forest]);
        }
    }
    let mut indices = Vec::with_capacity((w - 1) * (h - 1) * 6);
    for j in 0..h - 1 {
        for i in 0..w - 1 {
            let a = (j * w + i) as u32;
            let b = a + 1;
            let c = a + w as u32;
            let d = c + 1;
            // Alternate the diagonal to avoid a visible grain.
            if (i + j) % 2 == 0 {
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            } else {
                indices.extend_from_slice(&[a, c, d, a, d, b]);
            }
        }
    }
    // Skirts: the edge ring again, hung below, faced both ways. A coarse
    // chunk's edge can sit a little off its finer neighbour's; the skirt
    // fills the gap with ground.
    if spacing > 1 {
        let drop = 1.5 * spacing as f32;
        let ring: Vec<usize> = (0..w)
            .chain((1..h).map(|j| j * w + w - 1))
            .chain((0..w - 1).rev().map(|i| (h - 1) * w + i))
            .chain((1..h - 1).rev().map(|j| j * w))
            .collect();
        let first = positions.len() as u32;
        for &k in &ring {
            let [x, y, z] = positions[k];
            positions.push([x, y - drop, z]);
            normals.push(normals[k]);
            uvs.push(uvs[k]);
            uv1.push(uv1[k]);
            colors.push(colors[k]);
        }
        let n = ring.len();
        for s in 0..n {
            let (a, b) = (ring[s] as u32, ring[(s + 1) % n] as u32);
            let (c, d) = (first + s as u32, first + ((s + 1) % n) as u32);
            indices.extend_from_slice(&[a, c, b, b, c, d, a, b, c, b, d, c]);
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uv1)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices));
    if let Err(e) = mesh.generate_tangents() {
        warn!("terrain tangents: {e}");
    }
    mesh
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    extraction_core::geom::smoothstep(e0, e1, x)
}
