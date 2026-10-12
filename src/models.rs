//! The procedural models from `art/` (see `docs/ASSETS.md`, "Models"):
//! glTF files under `assets/models/`, read node by node so visuals can give
//! each named object its own behaviour (a gun's bolt, a drone's rotor), and
//! with their placeholder materials swapped by name for the game's own.
//!
//! Client-side only. Callers keep their blockout as a fallback: [`Models::request`]
//! returns `None` when the file isn't there, and [`Models::state`] reports a
//! failed load, so a missing or broken model never costs the game its visuals.

use bevy::gltf::{Gltf, GltfMaterial, GltfMesh, GltfNode};
use bevy::prelude::*;
use std::collections::HashMap;

use crate::render::materials::{Mat, MaterialLibrary};

/// Loaded (or loading) model files, by path under `assets/`.
#[derive(Resource, Default)]
pub struct Models {
    handles: HashMap<String, Option<Handle<Gltf>>>,
}

pub enum ModelState<'a> {
    Loading,
    Failed,
    Ready(&'a Gltf),
}

/// One mesh primitive of a node and the name of its material.
pub struct ModelPrimitive {
    pub mesh: Handle<Mesh>,
    pub material: Option<String>,
}

/// A node of a model: its name, local transform, primitives and children.
pub struct ModelNode {
    pub name: String,
    pub transform: Transform,
    pub primitives: Vec<ModelPrimitive>,
    pub children: Vec<ModelNode>,
}

impl ModelNode {
    pub fn find<'a>(nodes: &'a [ModelNode], name: &str) -> Option<&'a ModelNode> {
        nodes.iter().find(|n| n.name == name)
    }
}

impl Models {
    /// Starts loading `path` (relative to `assets/`) once; `None` if there is
    /// no such file, so the caller should use its blockout.
    pub fn request(&mut self, path: &str, server: &AssetServer) -> Option<Handle<Gltf>> {
        self.handles
            .entry(path.to_owned())
            .or_insert_with(|| {
                let on_disk = crate::asset_root_path().join(path);
                if on_disk.exists() {
                    Some(server.load(path.to_owned()))
                } else {
                    None
                }
            })
            .clone()
    }

    pub fn state<'a>(
        handle: &Handle<Gltf>,
        server: &AssetServer,
        gltfs: &'a Assets<Gltf>,
    ) -> ModelState<'a> {
        if let Some(g) = gltfs.get(handle) {
            if server.is_loaded_with_dependencies(handle) {
                return ModelState::Ready(g);
            }
        }
        if server.get_load_state(handle).is_some_and(|s| s.is_failed())
            || server
                .get_recursive_dependency_load_state(handle)
                .is_some_and(|s| s.is_failed())
        {
            return ModelState::Failed;
        }
        ModelState::Loading
    }
}

/// The scene's root nodes, with mesh primitives and material names resolved.
pub fn model_nodes(
    gltf: &Gltf,
    nodes: &Assets<GltfNode>,
    meshes: &Assets<GltfMesh>,
) -> Vec<ModelNode> {
    let material_names: HashMap<AssetId<GltfMaterial>, String> = gltf
        .named_materials
        .iter()
        .map(|(name, h)| (h.id(), name.to_string()))
        .collect();
    let is_child: std::collections::HashSet<AssetId<GltfNode>> = gltf
        .nodes
        .iter()
        .filter_map(|h| nodes.get(h))
        .flat_map(|n| n.children.iter().map(|c| c.id()))
        .collect();
    gltf.nodes
        .iter()
        .filter(|h| !is_child.contains(&h.id()))
        .filter_map(|h| convert(h, nodes, meshes, &material_names))
        .collect()
}

fn convert(
    handle: &Handle<GltfNode>,
    nodes: &Assets<GltfNode>,
    meshes: &Assets<GltfMesh>,
    material_names: &HashMap<AssetId<GltfMaterial>, String>,
) -> Option<ModelNode> {
    let node = nodes.get(handle)?;
    let primitives = node
        .mesh
        .as_ref()
        .and_then(|m| meshes.get(m))
        .map(|m| {
            m.primitives
                .iter()
                .map(|p| ModelPrimitive {
                    mesh: p.mesh.clone(),
                    material: p
                        .material
                        .as_ref()
                        .and_then(|h| material_names.get(&h.id()).cloned()),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ModelNode {
        name: node.name.clone(),
        transform: node.transform,
        primitives,
        children: node
            .children
            .iter()
            .filter_map(|c| convert(c, nodes, meshes, material_names))
            .collect(),
    })
}

/// The game's materials for the model material names: the scanned PBR sets
/// from the [`MaterialLibrary`] where they exist, flat colours otherwise.
#[derive(Resource, Default)]
pub struct ModelMaterials {
    cache: HashMap<String, Handle<StandardMaterial>>,
}

impl ModelMaterials {
    pub fn get(
        &mut self,
        name: &str,
        lib: &mut MaterialLibrary,
        server: &AssetServer,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        if let Some(h) = self.cache.get(name) {
            return h.clone();
        }
        let mut derive = |base: Mat, f: &dyn Fn(&mut StandardMaterial)| {
            let h = lib.get(base, server, materials);
            let mut m = materials.get(&h).cloned().unwrap_or_default();
            f(&mut m);
            materials.add(m)
        };
        let h = match name {
            "Blued" => lib.get(Mat::GunMetal, server, materials),
            "Wood" => lib.get(Mat::GunWood, server, materials),
            "Polymer" => lib.get(Mat::Polymer, server, materials),
            "Shell" => lib.get(Mat::RobotShell, server, materials),
            // Bright steel: the gun-metal scan's relief, without its dark albedo.
            "Steel" => derive(Mat::GunMetal, &|m| {
                m.base_color_texture = None;
                m.base_color = Color::srgb(0.46, 0.46, 0.47);
                m.metallic = 1.0;
            }),
            // Phenolic grips: the polymer's relief in a deep red-brown.
            "Bakelite" => derive(Mat::Polymer, &|m| {
                m.base_color_texture = None;
                m.base_color = Color::srgb(0.2, 0.075, 0.04);
                m.metallic = 0.0;
            }),
            "Black" => derive(Mat::GunMetal, &|m| {
                m.base_color = Color::srgb(0.45, 0.45, 0.45);
                m.perceptual_roughness = m.perceptual_roughness.max(0.6);
            }),
            "Brass" => materials.add(StandardMaterial {
                base_color: Color::srgb(0.72, 0.52, 0.24),
                metallic: 1.0,
                perceptual_roughness: 0.35,
                ..default()
            }),
            _ => materials.add(StandardMaterial {
                base_color: Color::srgb(0.3, 0.3, 0.3),
                perceptual_roughness: 0.6,
                ..default()
            }),
        };
        self.cache.insert(name.to_owned(), h.clone());
        h
    }
}

/// Spawns a node's primitives as children of `parent`, materials picked by
/// `material(name)`.
pub fn spawn_primitives(
    commands: &mut Commands,
    parent: Entity,
    node: &ModelNode,
    mut material: impl FnMut(Option<&str>) -> Handle<StandardMaterial>,
) {
    for p in &node.primitives {
        commands.spawn((
            Mesh3d(p.mesh.clone()),
            MeshMaterial3d(material(p.material.as_deref())),
            Transform::IDENTITY,
            ChildOf(parent),
        ));
    }
}

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Models>()
            .init_resource::<ModelMaterials>();
    }
}
