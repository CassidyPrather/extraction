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

/// Every material name [`ModelMaterials::get`] knows; any other name renders
/// a flat grey. `art/common.py` must list the same names (a test checks it,
/// and that every exported model uses only these).
// One name a line, so that branches adding names merge without a fight.
#[rustfmt::skip]
pub const MATERIAL_NAMES: &[&str] = &[
    "Blued",
    "Wood",
    "Polymer",
    "Shell",
    "Steel",
    "Bakelite",
    "Black",
    "Brass",
    "Paint",
    "Canvas",
    "Planks",
    "Boards",
    "Iron",
    "Glass",
    "Rubber",
    "Rope",
    "Yellow",
    "Orange",
    "Blue",
    "Leaf",
    "Enamel",
    "Worn",
    "Red",
    "Pebble",
    "Char",
    "Ember",
    "Cream",
    "Paper",
    "Olive",
    "Copper",
    "Dark",
    "Rotor",
    "Lens",
    "Clear",
    "Lamp",
    "LampRed",
    "Glow",
    "Warn",
    "Eye",
    "Strange",
    "Beacon",
    // World props (art/props).
    "Timber",
    "Rusty",
    "Painted",
    "Plaster",
    "Concrete",
    "Cast",
    "Wool",
    "Ticking",
    "Zinc",
    "GreyGreen",
    "Straw",
    "Brick",
    "Corrugated",
    "Shingles",
    // The gloves (art/hands).
    "Glove",
    "Leather",
    "Padding",
    // People (Lev): weathered skin, a dark wool coat.
    "Skin",
    "DarkWool",
    // Re-packed ammunition's tarnished brass.
    "Tarnish",
];

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
            // The truck, gadgets and the rest.
            // Army olive: the peeling-paint scan's relief and wear, its
            // own green and rust left out.
            "Paint" => derive(Mat::PaintedMetal, &|m| {
                m.base_color_texture = None;
                m.base_color = Color::srgb(0.22, 0.25, 0.12);
                m.metallic = 0.3;
            }),
            "Canvas" => lib.get(Mat::Canvas, server, materials),
            "Planks" => lib.get(Mat::Planks, server, materials),
            "Boards" => lib.get(Mat::WoodFloor, server, materials),
            "Iron" => lib.get(Mat::DarkMetal, server, materials),
            "Glass" => lib.get(Mat::Glass, server, materials),
            // World props (art/props).
            "Timber" => lib.get(Mat::Timber, server, materials),
            "Rusty" => lib.get(Mat::RustyMetal, server, materials),
            "Painted" => lib.get(Mat::PaintedMetal, server, materials),
            "Plaster" => lib.get(Mat::Plaster, server, materials),
            "Concrete" => lib.get(Mat::Concrete, server, materials),
            "Brick" => lib.get(Mat::Brick, server, materials),
            "Corrugated" => lib.get(Mat::Corrugated, server, materials),
            "Shingles" => lib.get(Mat::Shingles, server, materials),
            // Cast iron: the gun-metal scan's relief, dull and dark.
            "Cast" => derive(Mat::GunMetal, &|m| {
                m.base_color = Color::srgb(0.32, 0.31, 0.3);
                m.perceptual_roughness = m.perceptual_roughness.max(0.75);
            }),
            "Wool" => flat(materials, Color::srgb(0.24, 0.23, 0.2), 0.0, 0.95),
            "DarkWool" => flat(materials, Color::srgb(0.14, 0.14, 0.15), 0.0, 0.95),
            "Skin" => flat(materials, Color::srgb(0.6, 0.44, 0.36), 0.0, 0.6),
            "Tarnish" => flat(materials, Color::srgb(0.42, 0.31, 0.15), 1.0, 0.55),
            "Ticking" => flat(materials, Color::srgb(0.55, 0.53, 0.48), 0.0, 0.9),
            // Instrument paint: the grey-green of survey and signals kit.
            "GreyGreen" => flat(materials, Color::srgb(0.36, 0.4, 0.34), 0.0, 0.6),
            "Zinc" => flat(materials, Color::srgb(0.5, 0.52, 0.53), 0.9, 0.45),
            "Straw" => flat(materials, Color::srgb(0.5, 0.42, 0.24), 0.0, 0.95),
            // The gloves: the canvas scan's weave, fine, under olive-drab
            // nylon, a tan synthetic leather palm and charcoal padding. Dark
            // as worn gloves are, but never black (ART_DIRECTION: values).
            "Glove" | "Leather" | "Padding" => {
                let (c, rough) = match name {
                    "Glove" => (Color::srgb(0.30, 0.29, 0.23), 0.9),
                    "Leather" => (Color::srgb(0.33, 0.27, 0.2), 0.7),
                    _ => (Color::srgb(0.17, 0.17, 0.16), 0.8),
                };
                derive(Mat::Canvas, &move |m| {
                    m.base_color_texture = None;
                    m.base_color = c;
                    m.metallic = 0.0;
                    m.metallic_roughness_texture = None;
                    m.perceptual_roughness = rough;
                    m.uv_transform = bevy::math::Affine2::from_scale(Vec2::splat(6.0));
                })
            }
            "Rubber" => flat(materials, Color::srgb(0.07, 0.07, 0.07), 0.0, 0.88),
            "Rope" => flat(materials, Color::srgb(0.55, 0.47, 0.32), 0.0, 0.9),
            // Printed and painted colours. A true yellow (yolk, butter, the
            // 12-gauge carton), an orange for carrots and hazard paint.
            "Yellow" => flat(materials, Color::srgb(0.9, 0.74, 0.16), 0.0, 0.55),
            "Orange" => flat(materials, Color::srgb(0.86, 0.38, 0.08), 0.0, 0.6),
            "Blue" => flat(materials, Color::srgb(0.1, 0.24, 0.55), 0.0, 0.5),
            "Leaf" => flat(materials, Color::srgb(0.2, 0.4, 0.1), 0.0, 0.7),
            // White enamel: mugs, bowls, the kettle, labels' white print, and the
            // houses' stoves and sinks.
            "Enamel" => flat(materials, Color::srgb(0.88, 0.88, 0.85), 0.0, 0.3),
            // Bare steel showing through worn bluing, on gun edges.
            "Worn" => flat(materials, Color::srgb(0.5, 0.5, 0.5), 1.0, 0.45),
            "Red" => flat(materials, Color::srgb(0.5, 0.06, 0.04), 0.0, 0.6),
            "Pebble" => flat(materials, Color::srgb(0.42, 0.42, 0.44), 0.0, 0.45),
            "Char" => flat(materials, Color::srgb(0.1, 0.092, 0.085), 0.0, 0.9),
            "Ember" => glow(materials, Color::srgb(0.9, 0.35, 0.08), (6.0, 1.8, 0.25)),
            "Cream" => flat(materials, Color::srgb(0.82, 0.78, 0.66), 0.0, 0.6),
            "Paper" => flat(materials, Color::srgb(0.85, 0.83, 0.76), 0.0, 0.9),
            "Olive" => flat(materials, Color::srgb(0.22, 0.24, 0.15), 0.0, 0.8),
            "Copper" => flat(materials, Color::srgb(0.78, 0.44, 0.28), 1.0, 0.32),
            "Dark" => flat(materials, Color::srgb(0.08, 0.085, 0.09), 0.7, 0.4),
            "Rotor" => flat(materials, Color::srgb(0.06, 0.06, 0.065), 0.3, 0.45),
            "Lens" => materials.add(StandardMaterial {
                base_color: Color::srgb(0.05, 0.07, 0.1),
                metallic: 0.3,
                perceptual_roughness: 0.05,
                reflectance: 0.9,
                ..default()
            }),
            "Clear" => materials.add(StandardMaterial {
                base_color: Color::srgba(0.85, 0.88, 0.85, 0.16),
                perceptual_roughness: 0.15,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            "Lamp" => glow(materials, Color::srgb(0.9, 0.85, 0.7), (3.0, 2.6, 1.8)),
            "LampRed" => glow(materials, Color::srgb(0.5, 0.03, 0.02), (1.6, 0.08, 0.03)),
            "Glow" => glow(materials, Color::srgb(0.1, 0.4, 0.5), (0.5, 3.0, 4.0)),
            "Warn" => glow(materials, Color::srgb(0.9, 0.6, 0.1), (3.0, 1.6, 0.2)),
            // A robot's beacon (robots blink theirs; see robots::visuals).
            "Beacon" => glow(materials, Color::srgb(1.0, 0.75, 0.4), (6.0, 3.6, 1.2)),
            "Eye" => glow(materials, Color::srgb(0.1, 0.02, 0.02), (9.0, 1.2, 0.6)),
            "Strange" => materials.add(StandardMaterial {
                base_color: Color::srgb(0.45, 0.35, 0.7),
                emissive: LinearRgba::rgb(0.6, 0.35, 1.2),
                metallic: 0.6,
                perceptual_roughness: 0.15,
                clearcoat: 1.0,
                ..default()
            }),
            _ => {
                warn!("model material {name:?} is not mapped; drawing it grey");
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.3, 0.3, 0.3),
                    perceptual_roughness: 0.6,
                    ..default()
                })
            }
        };
        self.cache.insert(name.to_owned(), h.clone());
        h
    }
}

impl ModelMaterials {
    /// A variant of `base` made once under `key` by `f` (a robot's lit
    /// shell): cached like the named materials.
    pub fn derived(
        &mut self,
        key: &str,
        materials: &mut Assets<StandardMaterial>,
        base: &Handle<StandardMaterial>,
        f: impl FnOnce(&mut StandardMaterial),
    ) -> Handle<StandardMaterial> {
        if let Some(h) = self.cache.get(key) {
            return h.clone();
        }
        let mut m = materials.get(base).cloned().unwrap_or_default();
        f(&mut m);
        let h = materials.add(m);
        self.cache.insert(key.to_owned(), h.clone());
        h
    }
}

fn flat(
    materials: &mut Assets<StandardMaterial>,
    c: Color,
    metal: f32,
    rough: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        metallic: metal,
        perceptual_roughness: rough,
        ..default()
    })
}

fn glow(
    materials: &mut Assets<StandardMaterial>,
    c: Color,
    (r, g, b): (f32, f32, f32),
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        emissive: LinearRgba::rgb(r, g, b),
        ..default()
    })
}

/// Spawns a node's primitives as children of `parent`, materials picked by
/// `material(name)`.
pub fn spawn_primitives(
    commands: &mut Commands,
    parent: Entity,
    node: &ModelNode,
    material: impl FnMut(Option<&str>) -> Handle<StandardMaterial>,
) {
    spawn_primitives_ranged(commands, parent, node, None, material);
}

/// [`spawn_primitives`], each mesh drawn only within `range` if given.
pub fn spawn_primitives_ranged(
    commands: &mut Commands,
    parent: Entity,
    node: &ModelNode,
    range: Option<bevy::camera::visibility::VisibilityRange>,
    mut material: impl FnMut(Option<&str>) -> Handle<StandardMaterial>,
) {
    for p in &node.primitives {
        let mut e = commands.spawn((
            Mesh3d(p.mesh.clone()),
            MeshMaterial3d(material(p.material.as_deref())),
            Transform::IDENTITY,
            ChildOf(parent),
        ));
        if let Some(r) = range.clone() {
            e.insert(r);
        }
    }
}

/// Where an item model with a far stand-in swaps to it (metres from the
/// camera), cross-fading over a metre.
pub const ITEM_LOD: f32 = 6.0;

/// The draw range of a node in an item model that has `<name>_lod1`
/// stand-ins (`has_lods`): the stand-ins only past [`ITEM_LOD`], the rest
/// only nearer. None (always drawn) in a model without them.
pub fn item_lod_range(
    name: &str,
    has_lods: bool,
) -> Option<bevy::camera::visibility::VisibilityRange> {
    use bevy::camera::visibility::VisibilityRange;
    if !has_lods {
        return None;
    }
    let fade = (ITEM_LOD - 0.5)..(ITEM_LOD + 0.5);
    Some(if name.ends_with("_lod1") {
        VisibilityRange {
            start_margin: fade,
            end_margin: 1.0e4..1.0e4,
            use_aabb: false,
        }
    } else {
        VisibilityRange {
            start_margin: 0.0..0.0,
            end_margin: fade,
            use_aabb: false,
        }
    })
}

pub struct ModelsPlugin;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Models>()
            .init_resource::<ModelMaterials>();
    }
}

#[cfg(test)]
mod tests {
    use super::MATERIAL_NAMES;

    /// A model's far stand-in takes over where its full mesh stops; a
    /// model without stand-ins is drawn at any distance.
    #[test]
    fn item_lods_hand_over_at_one_distance() {
        use super::{ITEM_LOD, item_lod_range};
        assert!(item_lod_range("vest", false).is_none());
        let near = item_lod_range("vest", true).unwrap();
        let far = item_lod_range("vest_lod1", true).unwrap();
        assert_eq!(near.end_margin, far.start_margin);
        assert!(near.end_margin.contains(&ITEM_LOD));
        assert_eq!(near.start_margin, 0.0..0.0);
    }
    use std::path::Path;

    /// The material names in a .glb's JSON chunk.
    fn glb_materials(path: &Path) -> Vec<String> {
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(&bytes[0..4], b"glTF", "{} is not a glb", path.display());
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
        json["materials"]
            .as_array()
            .map(|ms| {
                ms.iter()
                    .filter_map(|m| m["name"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every material a model uses is one the game maps: an unknown name
    /// renders flat grey (how "Yellow" once came out orange and "Glow"
    /// came out dark went unnoticed).
    #[test]
    fn every_model_material_is_mapped() {
        let root = crate::asset_root_path().join("models");
        let mut checked = 0;
        let mut bad = Vec::new();
        for dir in std::fs::read_dir(&root).unwrap() {
            let dir = dir.unwrap().path();
            if !dir.is_dir() {
                continue;
            }
            for f in std::fs::read_dir(&dir).unwrap() {
                let f = f.unwrap().path();
                if f.extension().is_some_and(|e| e == "glb") {
                    checked += 1;
                    for m in glb_materials(&f) {
                        if !MATERIAL_NAMES.contains(&m.as_str()) {
                            bad.push(format!("{}: {m}", f.display()));
                        }
                    }
                }
            }
        }
        assert!(checked > 50, "only {checked} models found under {root:?}");
        assert!(
            bad.is_empty(),
            "unmapped model materials:\n{}",
            bad.join("\n")
        );
    }

    /// A model's bounds in its own space: every mesh node's POSITION
    /// accessor bounds, through the node's translation, rotation and scale.
    fn glb_bounds(path: &Path) -> Option<(bevy::math::Vec3, bevy::math::Vec3)> {
        use bevy::math::{Quat, Vec3};
        let bytes = std::fs::read(path).unwrap();
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
        let f = |v: &serde_json::Value, i: usize, d: f32| {
            v.get(i).and_then(|x| x.as_f64()).map_or(d, |x| x as f32)
        };
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for node in json["nodes"].as_array()? {
            let Some(mesh) = node["mesh"].as_u64() else {
                continue;
            };
            let t = &node["translation"];
            let r = &node["rotation"];
            let sc = &node["scale"];
            let at = Vec3::new(f(t, 0, 0.0), f(t, 1, 0.0), f(t, 2, 0.0));
            let rot = Quat::from_xyzw(f(r, 0, 0.0), f(r, 1, 0.0), f(r, 2, 0.0), f(r, 3, 1.0));
            let scale = Vec3::new(f(sc, 0, 1.0), f(sc, 1, 1.0), f(sc, 2, 1.0));
            for prim in json["meshes"][mesh as usize]["primitives"].as_array()? {
                let acc = &json["accessors"][prim["attributes"]["POSITION"].as_u64()? as usize];
                let (a, b) = (&acc["min"], &acc["max"]);
                for k in 0..8 {
                    let c = Vec3::new(
                        if k & 1 == 0 {
                            f(a, 0, 0.0)
                        } else {
                            f(b, 0, 0.0)
                        },
                        if k & 2 == 0 {
                            f(a, 1, 0.0)
                        } else {
                            f(b, 1, 0.0)
                        },
                        if k & 4 == 0 {
                            f(a, 2, 0.0)
                        } else {
                            f(b, 2, 0.0)
                        },
                    );
                    let w = at + rot * (scale * c);
                    lo = lo.min(w);
                    hi = hi.max(w);
                }
            }
        }
        (lo.x <= hi.x).then_some((lo, hi))
    }

    /// Every item model fits the box its collider is (`dims`, or a cube of
    /// its bulk): it doesn't stick out of it, rests on its floor rather
    /// than floating above or sinking into the ground, and a flat or long
    /// thing has `dims` of its own instead of a cube it rattles about in
    /// (a photograph once lay in a 2 cm cube as a postage stamp).
    #[test]
    fn every_item_model_fits_its_box() {
        use extraction_core::items::{BUILTIN_CATALOG, Category, ItemCatalog};
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        let root = crate::asset_root_path().join("models");
        let mut bad = Vec::new();
        let mut checked = 0;
        for (_, def) in cat.iter() {
            let dir = match def.category {
                // Guns are placed by their layouts, not their boxes.
                Category::Firearm => continue,
                Category::Magazine => "mags",
                _ => "items",
            };
            let path = root.join(dir).join(format!("{}.glb", def.id));
            let Some((lo, hi)) = path.exists().then(|| glb_bounds(&path)).flatten() else {
                continue;
            };
            checked += 1;
            let size = bevy::math::Vec3::from(def.size_m());
            let ext = hi - lo;
            let id = &def.id;
            // Attachments are drawn from their mount point, their clamps
            // hanging below it; only the size rule applies loosely.
            if def.category != Category::Attachment {
                for (i, axis) in ["x", "y", "z"].iter().enumerate() {
                    if ext[i] > size[i] * 1.2 + 0.006 {
                        bad.push(format!(
                            "{id}: {axis} {:.3} m in a {:.3} m box",
                            ext[i], size[i]
                        ));
                    }
                }
                let gap = lo.y + size.y / 2.0;
                if gap.abs() > 0.006 {
                    let how = if gap > 0.0 { "floats" } else { "sinks" };
                    bad.push(format!(
                        "{id}: {how} {:.3} m off its box's floor",
                        gap.abs()
                    ));
                }
            }
            if def.dims.is_none() && ext.min_element() < 0.5 * ext.max_element() {
                bad.push(format!(
                    "{id}: {:.3} x {:.3} x {:.3} m in a cube: give it dims",
                    ext.x, ext.y, ext.z
                ));
            }
        }
        assert!(checked > 80, "only {checked} item models found");
        assert!(
            bad.is_empty(),
            "item models that don't fit their boxes:\n{}",
            bad.join("\n")
        );
    }

    /// The art kit's preview palette names the same materials as the game.
    #[test]
    fn art_palette_matches_the_game() {
        let common = crate::asset_root_path()
            .parent()
            .unwrap()
            .join("art/common.py");
        let Ok(src) = std::fs::read_to_string(&common) else {
            return; // a packaged build has no art/
        };
        let start = src.find("MATERIALS = {").expect("MATERIALS in common.py");
        let end = start + src[start..].find("\n}").unwrap();
        let mut art: Vec<&str> = src[start..end]
            .lines()
            .filter_map(|l| l.trim().strip_prefix('"')?.split('"').next())
            .collect();
        let mut game: Vec<&str> = MATERIAL_NAMES.to_vec();
        art.sort();
        game.sort();
        assert_eq!(
            art, game,
            "art/common.py MATERIALS vs src/models.rs MATERIAL_NAMES"
        );
    }
}
