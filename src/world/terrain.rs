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

pub struct TerrainVisualsPlugin;

impl Plugin for TerrainVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_terrain_meshes);
    }
}

#[derive(Component)]
pub struct TerrainChunk;

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
            let mesh = chunk_mesh(&zone, i0, j0, i1, j1);
            commands.spawn((
                Name::new("Terrain chunk"),
                TerrainChunk,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
            ));
            i0 = i1;
        }
        j0 += step;
    }
}

/// Splat weights packed into vertex colour: r = bare dirt/mud, g = rock,
/// b = road gravel/asphalt, a = forest floor (needles, moss). Grass is
/// whatever is left over. Road kind goes in UV1.x so the shader can pick
/// asphalt vs gravel vs dirt.
fn chunk_mesh(zone: &Zone, i0: usize, j0: usize, i1: usize, j1: usize) -> Mesh {
    let hm = &zone.hm;
    let w = i1 - i0 + 1;
    let h = j1 - j0 + 1;
    let mut positions = Vec::with_capacity(w * h);
    let mut normals = Vec::with_capacity(w * h);
    let mut uvs = Vec::with_capacity(w * h);
    let mut uv1 = Vec::with_capacity(w * h);
    let mut colors = Vec::with_capacity(w * h);
    for j in j0..=j1 {
        for i in i0..=i1 {
            let p = hm.pos(i, j);
            let y = hm.heights[hm.idx(i, j)];
            positions.push([p[0], y, p[1]]);
            normals.push(hm.normal(p));
            // World-space UVs in metres; the shader scales per layer.
            uvs.push([p[0], p[1]]);
            let road = hm.roads[hm.idx(i, j)];
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
