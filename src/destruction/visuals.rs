//! Client: the glass and the pieces.
//!
//! A building's panes are one mesh (a draw for all its windows): each
//! whole pane a thin slab, each broken one the teeth its break left in
//! the frame (`fracture::shatter`, the same seed as when it broke, so a
//! restored save shows the same teeth). It's rebuilt when a pane breaks
//! or the Surge mends them. Pieces in flight are their own small slabs,
//! shrinking away at the end of their life.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::math::Affine3A;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use extraction_core::fracture::{self, P2};
use std::collections::HashMap;

use super::debris::{Debris, Kind};
use super::glass::{Pane, THICK, cells_for};
use crate::render::materials::{Mat, MaterialLibrary};

pub struct DestructionVisualsPlugin;

impl Plugin for DestructionVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (glaze_buildings, dress_debris, fade_debris));
    }
}

/// A convex outline extruded `thick` along z, centred on z 0: faces,
/// flat normals, UVs in metres.
pub fn slab(poly: &[P2], thick: f32) -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    append_slab(
        &mut pos,
        &mut nrm,
        &mut uv,
        &mut idx,
        poly,
        thick,
        Affine3A::IDENTITY,
    );
    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    m.insert_indices(Indices::U32(idx));
    m
}

fn append_slab(
    pos: &mut Vec<[f32; 3]>,
    nrm: &mut Vec<[f32; 3]>,
    uv: &mut Vec<[f32; 2]>,
    idx: &mut Vec<u32>,
    poly: &[P2],
    thick: f32,
    xf: Affine3A,
) {
    let n = poly.len();
    if n < 3 {
        return;
    }
    let h = thick * 0.5;
    let mut push = |p: Vec3, normal: Vec3, t: [f32; 2]| {
        pos.push(xf.transform_point3(p).to_array());
        nrm.push(xf.transform_vector3(normal).normalize_or_zero().to_array());
        uv.push(t);
        (pos.len() - 1) as u32
    };
    // Front (+z) and back (-z), fans (the outline is convex, CCW).
    for (z, normal) in [(h, Vec3::Z), (-h, Vec3::NEG_Z)] {
        let first = pos_len_after(&mut push, poly, z, normal);
        for i in 1..n as u32 - 1 {
            if z > 0.0 {
                idx.extend([first, first + i, first + i + 1]);
            } else {
                idx.extend([first, first + i + 1, first + i]);
            }
        }
    }
    // The edges.
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let e = Vec3::new(b[0] - a[0], b[1] - a[1], 0.0);
        let normal = Vec3::new(e.y, -e.x, 0.0).normalize_or_zero();
        let l = e.length();
        let v0 = push(Vec3::new(a[0], a[1], -h), normal, [0.0, 0.0]);
        let v1 = push(Vec3::new(b[0], b[1], -h), normal, [l, 0.0]);
        let v2 = push(Vec3::new(b[0], b[1], h), normal, [l, thick]);
        let v3 = push(Vec3::new(a[0], a[1], h), normal, [0.0, thick]);
        idx.extend([v0, v1, v2, v0, v2, v3]);
    }
}

fn pos_len_after(
    push: &mut impl FnMut(Vec3, Vec3, [f32; 2]) -> u32,
    poly: &[P2],
    z: f32,
    normal: Vec3,
) -> u32 {
    let mut first = None;
    for p in poly {
        let i = push(Vec3::new(p[0], p[1], z), normal, *p);
        first.get_or_insert(i);
    }
    first.unwrap_or(0)
}

/// The glass of a building's windows, drawn as one.
#[derive(Component)]
pub struct WindowGlass;

/// Rebuild the glass of each building whose panes changed.
fn glaze_buildings(
    mut commands: Commands,
    changed: Query<&Pane, Changed<Pane>>,
    panes: Query<(&Pane, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    mut drawn: Local<HashMap<Entity, Entity>>,
) {
    let mut dirty: Vec<Entity> = changed.iter().map(|p| p.building).collect();
    if dirty.is_empty() {
        return;
    }
    dirty.sort();
    dirty.dedup();
    let mut by: HashMap<Entity, (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>)> =
        dirty.iter().map(|b| (*b, Default::default())).collect();
    for (p, local) in &panes {
        let Some((pos, nrm, uv, idx)) = by.get_mut(&p.building) else {
            continue;
        };
        let xf = local.compute_affine();
        match p.broken {
            None => {
                let h = p.half;
                let rect = [[-h.x, -h.y], [h.x, -h.y], [h.x, h.y], [-h.x, h.y]];
                append_slab(pos, nrm, uv, idx, &rect, THICK, xf);
            }
            Some(b) => {
                let s = fracture::shatter(
                    p.half.to_array(),
                    b.impact.to_array(),
                    b.seed,
                    cells_for(p.area()),
                );
                for k in &s.kept {
                    let at = xf * Affine3A::from_translation(Vec3::new(k.at[0], k.at[1], 0.0));
                    append_slab(pos, nrm, uv, idx, &k.poly, THICK, at);
                }
            }
        }
    }
    let glass = library.get(Mat::Glass, &server, &mut materials);
    for (b, (pos, nrm, uv, idx)) in by {
        if commands.get_entity(b).is_err() {
            continue;
        }
        let mut m = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        m.insert_indices(Indices::U32(idx));
        let handle = meshes.add(m);
        match drawn.get(&b).copied() {
            Some(e) if commands.get_entity(e).is_ok() => {
                commands.entity(e).insert(Mesh3d(handle));
            }
            _ => {
                let e = commands
                    .spawn((
                        Name::new("Window glass"),
                        WindowGlass,
                        Mesh3d(handle),
                        MeshMaterial3d(glass.clone()),
                        Transform::IDENTITY,
                        NotShadowCaster,
                        crate::render::lod::near_range(),
                        ChildOf(b),
                    ))
                    .id();
                drawn.insert(b, e);
            }
        }
    }
}

/// A piece's slab, as it's thrown.
fn dress_debris(
    mut commands: Commands,
    added: Query<(Entity, &Debris), Added<Debris>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for (e, d) in &added {
        let mat = match d.kind {
            Kind::Glass => Mat::Glass,
            Kind::Wood(m) => m,
        };
        let material = library.get(mat, &server, &mut materials);
        // On a child, so shrinking it never touches the body.
        commands.spawn((
            DebrisMesh,
            Mesh3d(meshes.add(slab(&d.poly, d.thick))),
            MeshMaterial3d(material),
            NotShadowCaster,
            Transform::IDENTITY,
            ChildOf(e),
        ));
    }
}

#[derive(Component)]
pub struct DebrisMesh;

/// The last half second of a piece's life, it shrinks away.
fn fade_debris(
    pieces: Query<&Debris>,
    mut drawn: Query<(&ChildOf, &mut Transform), With<DebrisMesh>>,
) {
    for (parent, mut t) in &mut drawn {
        let Ok(d) = pieces.get(parent.parent()) else {
            continue;
        };
        let left = d.life - d.age;
        if left < 0.5 {
            t.scale = Vec3::splat((left / 0.5).clamp(0.05, 1.0));
        }
    }
}
