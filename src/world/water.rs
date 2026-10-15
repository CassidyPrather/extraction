//! Standing and running water surfaces, built from the heightmap's water
//! mask.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;

use super::Zone;
use crate::render::water_material::{WaterMaterial, water_material};

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_water);
    }
}

fn spawn_water(
    mut commands: Commands,
    zone: Res<Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let hm = &zone.hm;
    let n = hm.n;
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut vertex_of = std::collections::HashMap::new();
    // A cell gets a quad if any corner is wet; dry corners take the
    // neighbouring level so the surface runs under the banks.
    let level = |i: usize, j: usize| -> Option<f32> {
        let w = hm.water[hm.idx(i, j)];
        (!w.is_nan()).then_some(w)
    };
    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let corners = [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)];
            let levels: Vec<f32> = corners.iter().filter_map(|&(a, b)| level(a, b)).collect();
            if levels.is_empty() {
                continue;
            }
            let fallback = levels.iter().sum::<f32>() / levels.len() as f32;
            let mut ids = [0u32; 4];
            for (k, &(a, b)) in corners.iter().enumerate() {
                let y = level(a, b).unwrap_or(fallback);
                ids[k] = *vertex_of.entry((a, b)).or_insert_with(|| {
                    let p = hm.pos(a, b);
                    positions.push([p[0], y, p[1]]);
                    (positions.len() - 1) as u32
                });
            }
            indices.extend_from_slice(&[ids[0], ids[2], ids[1], ids[1], ids[2], ids[3]]);
        }
    }
    if positions.is_empty() {
        return;
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let uvs: Vec<[f32; 2]> = positions.iter().map(|p| [p[0], p[2]]).collect();
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices));
    commands.spawn((
        Name::new("Water"),
        crate::render::centred(&mut meshes, mesh),
        MeshMaterial3d(materials.add(water_material())),
        NotShadowCaster,
    ));
}
