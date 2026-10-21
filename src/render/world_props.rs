//! The world props from `art/props/` (see `docs/ASSETS.md`, "Models"):
//! furniture, crates, door leaves, windows, signposts, fence posts and the
//! camp. Client-side only: logic keeps its colliders and loot spots, and
//! every prop keeps a box blockout that stays when its model is missing or
//! broken.
//!
//! Two ways in. Buildings merge their props into the building's own
//! per-material meshes (`world::buildings`), with copies from
//! [`PropMeshes`], so a furnished house is still a handful of draws.
//! Anything else spawns its blockout children marked [`Blockout`] and puts
//! a [`SwapIn`] on itself: once the models are loaded the blockout goes
//! and the models' meshes (shared, so repeats instance) take its place.

use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::collections::HashMap;

use crate::models::{ModelMaterials, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;

pub struct WorldPropsPlugin;

impl Plugin for WorldPropsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PropMeshes>().add_systems(
            Update,
            (
                swap_in,
                crate::world::overgrowth::setup_ivy,
                crate::world::overgrowth::blow_ivy,
                crate::world::decay::setup_stains,
                crate::world::buildings::dress_buildings
                    .after(crate::world::buildings::building_visuals),
            ),
        );
    }
}

/// A prop model's path under `assets/`.
pub fn path(name: &str) -> String {
    format!("models/props/{name}.glb")
}

/// A blockout part to remove once its entity's [`SwapIn`] models are in.
#[derive(Component)]
pub struct Blockout;

/// Models (by name under `models/props/`, placed in the entity's space)
/// to replace this entity's [`Blockout`] children with, once all of them
/// have loaded. If any is missing or broken, the blockout stays.
#[derive(Component)]
pub struct SwapIn(pub Vec<(&'static str, Transform)>);

impl SwapIn {
    pub fn one(name: &'static str) -> Self {
        Self(vec![(name, Transform::IDENTITY)])
    }
}

/// Where a model is: still loading, not there (or broken), or loaded.
pub enum Resolved {
    Loading,
    Missing,
    Ready,
}

/// What it takes to look a prop model up.
#[derive(SystemParam)]
pub struct PropModels<'w> {
    pub models: ResMut<'w, Models>,
    pub server: Res<'w, AssetServer>,
    pub gltfs: Res<'w, Assets<Gltf>>,
    pub nodes: Res<'w, Assets<GltfNode>>,
    pub gltf_meshes: Res<'w, Assets<GltfMesh>>,
    pub model_mats: ResMut<'w, ModelMaterials>,
    pub lib: ResMut<'w, MaterialLibrary>,
}

impl PropModels<'_> {
    pub fn resolve(&mut self, name: &str) -> Resolved {
        match self.models.request(&path(name), &self.server) {
            None => Resolved::Missing,
            Some(h) => match Models::state(&h, &self.server, &self.gltfs) {
                ModelState::Loading => Resolved::Loading,
                ModelState::Failed => {
                    warn!("prop model {name} failed to load; keeping its blockout");
                    Resolved::Missing
                }
                ModelState::Ready(_) => Resolved::Ready,
            },
        }
    }

    /// Of several models: loading if any is, missing if any is, else ready.
    pub fn resolve_all<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) -> Resolved {
        let mut out = Resolved::Ready;
        for n in names {
            match self.resolve(n) {
                Resolved::Loading => return Resolved::Loading,
                Resolved::Missing => out = Resolved::Missing,
                Resolved::Ready => {}
            }
        }
        out
    }

    /// The model's primitives (mesh, material name) in prop space, if it
    /// has loaded.
    pub fn primitives(&mut self, name: &str) -> Vec<(Handle<Mesh>, Transform, String)> {
        let Some(h) = self.models.request(&path(name), &self.server) else {
            return Vec::new();
        };
        let ModelState::Ready(g) = Models::state(&h, &self.server, &self.gltfs) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        fn walk(
            n: &crate::models::ModelNode,
            parent: Transform,
            out: &mut Vec<(Handle<Mesh>, Transform, String)>,
        ) {
            let t = parent * n.transform;
            for p in &n.primitives {
                out.push((
                    p.mesh.clone(),
                    t,
                    p.material.clone().unwrap_or_else(|| "Timber".into()),
                ));
            }
            for c in &n.children {
                walk(c, t, out);
            }
        }
        for n in model_nodes(g, &self.nodes, &self.gltf_meshes) {
            walk(&n, Transform::IDENTITY, &mut out);
        }
        out
    }

    pub fn material(
        &mut self,
        name: &str,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.model_mats
            .get(name, &mut self.lib, &self.server, materials)
    }
}

/// Copies of prop meshes made ready to merge into a building's (only
/// position, normal, UV and tangent, 32-bit indices), by model name.
#[derive(Resource, Default)]
pub struct PropMeshes(HashMap<String, Vec<(String, Mesh)>>);

impl PropMeshes {
    /// Whether `name`'s meshes are in (merged, if empty: a model with
    /// nothing mergeable is ready too).
    pub fn has(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    /// The model's primitives as mergeable meshes, in prop space; empty if
    /// it isn't loaded.
    pub fn get(
        &mut self,
        name: &str,
        props: &mut PropModels,
        meshes: &Assets<Mesh>,
    ) -> &[(String, Mesh)] {
        if !self.0.contains_key(name) {
            let prims = props.primitives(name);
            if prims.is_empty() {
                return &[];
            }
            // A model's glTF can be ready a frame or two before its
            // meshes are: wait for them all (caching it half-loaded would
            // drop the rest for good).
            if prims.iter().any(|(h, _, _)| meshes.get(h).is_none()) {
                return &[];
            }
            let mut out = Vec::new();
            for (h, t, mat) in prims {
                if let Some(m) = meshes.get(&h).and_then(for_merge) {
                    out.push((mat, m.transformed_by(t)));
                }
            }
            self.0.insert(name.to_owned(), out);
        }
        &self.0[name]
    }
}

/// A copy of a mesh with just the attributes the buildings' boxes have, so
/// the two merge.
pub fn for_merge(m: &Mesh) -> Option<Mesh> {
    if m.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let pos = m.try_attribute(Mesh::ATTRIBUTE_POSITION).ok()?.clone();
    let nrm = m.try_attribute(Mesh::ATTRIBUTE_NORMAL).ok()?.clone();
    let uv = m.try_attribute(Mesh::ATTRIBUTE_UV_0).ok()?.clone();
    let idx: Vec<u32> = m.try_indices().ok()?.iter().map(|i| i as u32).collect();
    let mut out = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx));
    match m.try_attribute(Mesh::ATTRIBUTE_TANGENT) {
        Ok(t) => out.insert_attribute(Mesh::ATTRIBUTE_TANGENT, t.clone()),
        Err(_) => out.generate_tangents().ok()?,
    }
    // The model's baked ambient occlusion (white if it has none), as
    // 32-bit floats so it merges with the buildings' own (`world::ao`).
    let n = out.count_vertices();
    let col: Vec<[f32; 4]> = match m.try_attribute(Mesh::ATTRIBUTE_COLOR) {
        Ok(bevy::mesh::VertexAttributeValues::Float32x4(c)) if c.len() == n => c.clone(),
        Ok(bevy::mesh::VertexAttributeValues::Float32x3(c)) if c.len() == n => {
            c.iter().map(|c| [c[0], c[1], c[2], 1.0]).collect()
        }
        _ => vec![[1.0; 4]; n],
    };
    let col = if crate::world::ao::enabled() {
        col
    } else {
        vec![[1.0; 4]; n]
    };
    out.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    Some(out)
}

/// Swaps blockouts for models once they've loaded.
fn swap_in(
    mut commands: Commands,
    waiting: Query<(Entity, &SwapIn, Option<&Children>)>,
    blockouts: Query<(), With<Blockout>>,
    mut props: PropModels,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, swap, children) in &waiting {
        match props.resolve_all(swap.0.iter().map(|(n, _)| *n)) {
            Resolved::Loading => continue,
            Resolved::Missing => {
                commands.entity(e).remove::<SwapIn>();
                continue;
            }
            Resolved::Ready => {}
        }
        commands.entity(e).remove::<SwapIn>();
        for c in children.into_iter().flatten() {
            if blockouts.contains(*c) {
                commands.entity(*c).despawn();
            }
        }
        for (name, at) in &swap.0 {
            for (mesh, t, mat) in props.primitives(name) {
                let material = props.material(&mat, &mut materials);
                commands.spawn((Mesh3d(mesh), MeshMaterial3d(material), *at * t, ChildOf(e)));
            }
        }
    }
}
