//! Items in the world: the catalog as a resource, spawning physical items,
//! and their placeholder visuals.

pub mod loot;
pub mod usage;

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::items::{
    BUILTIN_CATALOG, Category, ItemCatalog, ItemDef, ItemKind, ItemState,
};
use std::sync::Arc;

use crate::interact::{GrabPoint, Grabbable};
use crate::world::Layer;

/// The item catalog, shared.
#[derive(Resource, Clone, Deref)]
pub struct Catalog(pub Arc<ItemCatalog>);

impl Catalog {
    pub fn load() -> Self {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/data/items.ron");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|_| BUILTIN_CATALOG.to_owned());
        let catalog = ItemCatalog::from_ron(&text).unwrap_or_else(|e| {
            error!("items.ron: {e}; falling back to the built-in catalog");
            ItemCatalog::from_ron(BUILTIN_CATALOG).expect("built-in catalog parses")
        });
        Self(Arc::new(catalog))
    }
}

/// A physical item. Its per-instance state (condition, rounds, charge).
#[derive(Component, Clone, Debug, Deref, DerefMut)]
pub struct Item(pub ItemState);

impl Item {
    pub fn kind(&self) -> ItemKind {
        self.0.kind
    }
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Catalog::load())
            .add_plugins((loot::LootPlugin, usage::UsagePlugin));
    }
}

/// Request to spawn an item; resolved by [`spawn_item`].
pub struct SpawnItem {
    pub kind: ItemKind,
    pub transform: Transform,
    pub condition: f32,
}

/// Spawns a free, physical item and returns its entity. Firearms and
/// magazines get their mechanism components from the weapons module's
/// `On<Add, Item>` observer.
pub fn spawn_item(commands: &mut Commands, catalog: &ItemCatalog, req: SpawnItem) -> Entity {
    let def = catalog.get(req.kind);
    let state = ItemState::fresh(catalog, req.kind, req.condition);
    commands
        .spawn((
            Name::new(def.name.clone()),
            Item(state),
            req.transform,
            Visibility::default(),
            Grabbable {
                points: default_grab_points(def),
            },
        ))
        .insert(physical_bundle(def))
        .id()
}

/// Physics for a loose item.
pub fn physical_bundle(def: &ItemDef) -> impl Bundle {
    let [x, y, z] = def.size_m();
    (
        RigidBody::Dynamic,
        Collider::cuboid(x.max(0.01), y.max(0.01), z.max(0.01)),
        Mass(def.mass),
        NoAutoMass,
        CollisionLayers::new(Layer::Item, [Layer::World, Layer::Item, Layer::Robot]),
        Friction::new(0.7),
        Restitution::new(0.15),
        LinearDamping(0.05),
        AngularDamping(0.1),
        SweptCcd::default(),
    )
}

/// Where hands hold an item, by default. Guns override with their grips.
fn default_grab_points(def: &ItemDef) -> Vec<GrabPoint> {
    match def.category {
        // Free grab: the hand keeps whatever relative pose it grabbed at.
        _ => vec![GrabPoint::free()],
    }
}

/// Category colours for placeholder visuals.
fn placeholder_color(cat: Category) -> Color {
    match cat {
        Category::Firearm => Color::srgb(0.12, 0.12, 0.13),
        Category::Magazine => Color::srgb(0.25, 0.12, 0.15),
        Category::Ammo => Color::srgb(0.55, 0.42, 0.18),
        Category::Attachment => Color::srgb(0.15, 0.15, 0.16),
        Category::Gadget => Color::srgb(0.25, 0.3, 0.22),
        Category::Medical => Color::srgb(0.7, 0.68, 0.62),
        Category::Food => Color::srgb(0.5, 0.42, 0.3),
        Category::Tool => Color::srgb(0.35, 0.3, 0.25),
        Category::Material => Color::srgb(0.38, 0.36, 0.33),
        Category::RobotPart => Color::srgb(0.82, 0.83, 0.85),
        Category::Artifact => Color::srgb(0.55, 0.4, 0.75),
        Category::Curio => Color::srgb(0.45, 0.35, 0.25),
    }
}

/// Marker for items whose visuals are handled by a specialised builder
/// (guns), so the generic placeholder skips them.
#[derive(Component)]
pub struct CustomVisual;

pub struct ItemVisualsPlugin;

impl Plugin for ItemVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, placeholder_visuals);
    }
}

fn placeholder_visuals(
    mut commands: Commands,
    added: Query<(Entity, &Item), (Added<Item>, Without<CustomVisual>, Without<Mesh3d>)>,
    custom: Query<(), With<CustomVisual>>,
    catalog: Res<Catalog>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<std::collections::HashMap<ItemKind, (Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    for (e, item) in &added {
        if custom.contains(e) {
            continue;
        }
        let def = catalog.get(item.kind());
        let (mesh, mat) = cache
            .entry(item.kind())
            .or_insert_with(|| {
                let [x, y, z] = def.size_m();
                let emissive = if def.category == Category::Artifact {
                    LinearRgba::rgb(0.6, 0.35, 1.2)
                } else {
                    LinearRgba::BLACK
                };
                (
                    meshes.add(Cuboid::new(x, y, z)),
                    materials.add(StandardMaterial {
                        base_color: placeholder_color(def.category),
                        perceptual_roughness: 0.6,
                        emissive,
                        ..default()
                    }),
                )
            })
            .clone();
        commands
            .entity(e)
            .insert((Mesh3d(mesh), MeshMaterial3d(mat)));
    }
}
