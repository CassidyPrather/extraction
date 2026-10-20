//! The kit you wear (`art/kit/rig.py`): the chest rig and belt the body
//! slots hang from, a holster, magazine pouches, hip pockets and the
//! chest mount, each slot's flap closed while it's empty and open while
//! something's in it; and the backpack, its lid open while it hangs open.
//!
//! Only the look: slot positions, seats and snap radii are gameplay
//! (`interact::body`) and stay as they are. Until the models load, or if
//! they don't, the slots and the pack keep their boxes. Everything here
//! rides on the torso, so `props::body_casts_no_shadow` keeps it out of
//! the shadow maps.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;
use extraction_core::inventory::SlotKind;

use crate::interact::body::{Backpack, BodySlot, Torso};
use crate::interact::{Socket, SocketKind};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes, spawn_primitives};
use crate::raid::Remains;
use crate::raid::truck::Stash;
use crate::render::materials::MaterialLibrary;

const RIG: &str = "models/kit/rig.glb";
const RIG_OPEN: &str = "models/kit/rig_open.glb";
const PACK: &str = "models/kit/backpack.glb";
const PACK_OPEN: &str = "models/kit/backpack_open.glb";

pub struct KitVisualsPlugin;

impl Plugin for KitVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (dress_slots, dress_pack, mark_torsos),
                wear_kit,
                (open_flaps, open_lids),
            )
                .chain(),
        );
    }
}

/// A slot's model piece (`None`: the slings, which carry what's slung and
/// draw nothing of their own).
fn piece(kind: SlotKind) -> Option<&'static str> {
    match kind {
        SlotKind::Sling => None,
        SlotKind::Holster => Some("holster"),
        SlotKind::Pouch => Some("pouch"),
        SlotKind::Pocket => Some("pocket"),
        SlotKind::Mount => Some("mount"),
    }
}

/// Waiting for the kit's models, with the boxes worn meanwhile.
#[derive(Component)]
struct Undressed {
    boxes: Vec<Entity>,
}

/// Which flap a child is: shown while its slot (or bag) is empty
/// (closed) or full (open).
#[derive(Component)]
struct Flap {
    open: bool,
}

/// A torso still to get its harness.
#[derive(Component)]
struct WantsHarness;

fn mark_torsos(mut commands: Commands, torsos: Query<Entity, Added<Torso>>) {
    for t in &torsos {
        commands.entity(t).insert(WantsHarness);
    }
}

/// Boxes on the slots until the models are in (and for good if they
/// never are).
fn dress_slots(
    mut commands: Commands,
    slots: Query<(Entity, &Socket), Added<BodySlot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut look: Local<Option<Handle<StandardMaterial>>>,
) {
    for (e, socket) in &slots {
        let SocketKind::Body(kind) = socket.kind else {
            continue;
        };
        let size = match kind {
            SlotKind::Sling => continue,
            SlotKind::Holster => Vec3::new(0.05, 0.16, 0.06),
            SlotKind::Pouch => Vec3::new(0.07, 0.11, 0.035),
            SlotKind::Pocket | SlotKind::Mount => Vec3::new(0.06, 0.07, 0.03),
        };
        let m = look
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.24, 0.26, 0.19),
                    perceptual_roughness: 0.95,
                    ..default()
                })
            })
            .clone();
        let b = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(m),
                Transform::from_xyz(0.0, -0.04, 0.0),
                ChildOf(e),
            ))
            .id();
        commands.entity(e).insert(Undressed { boxes: vec![b] });
    }
}

/// The worn pack: a canvas box with a lid until its model is in.
fn dress_pack(
    mut commands: Commands,
    bags: Query<(Entity, &Backpack), (Added<Backpack>, Without<Stash>, Without<Remains>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, bag) in &bags {
        if !bag.carried {
            continue;
        }
        let canvas = materials.add(StandardMaterial {
            base_color: Color::srgb(0.27, 0.29, 0.20),
            perceptual_roughness: 0.95,
            ..default()
        });
        let body = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(0.34, 0.48, 0.2))),
                MeshMaterial3d(canvas.clone()),
                ChildOf(e),
            ))
            .id();
        let lid = commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(0.35, 0.04, 0.22))),
                MeshMaterial3d(canvas),
                Transform::from_xyz(0.0, 0.25, 0.01),
                ChildOf(e),
            ))
            .id();
        commands.entity(e).insert(Undressed {
            boxes: vec![body, lid],
        });
    }
}

/// A model file's nodes: `None` while loading, `Some(None)` if it's missing
/// or broken (keep the boxes).
fn nodes_of(
    path: &str,
    models: &mut Models,
    server: &AssetServer,
    gltfs: &Assets<Gltf>,
    nodes: &Assets<GltfNode>,
    meshes: &Assets<GltfMesh>,
) -> Option<Option<Vec<ModelNode>>> {
    let Some(h) = models.request(path, server) else {
        return Some(None);
    };
    match Models::state(&h, server, gltfs) {
        ModelState::Loading => None,
        ModelState::Failed => {
            warn!("{path} failed to load; the kit keeps its boxes");
            Some(None)
        }
        ModelState::Ready(g) => Some(Some(model_nodes(g, nodes, meshes))),
    }
}

/// Swaps the boxes for the models once they're in. Each piece's mesh hangs
/// on its own slot (the model's node transforms only lay the rig out in
/// one place for the art checks).
#[allow(clippy::too_many_arguments)]
fn wear_kit(
    mut commands: Commands,
    slots: Query<(Entity, &Socket, &Undressed), With<BodySlot>>,
    bags: Query<(Entity, &Undressed), With<Backpack>>,
    torsos: Query<Entity, With<WantsHarness>>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if slots.is_empty() && bags.is_empty() && torsos.is_empty() {
        return;
    }
    let mut load = |path| nodes_of(path, &mut models, &server, &gltfs, &nodes, &gltf_meshes);
    let (Some(rig), Some(rig_open), Some(pack), Some(pack_open)) =
        (load(RIG), load(RIG_OPEN), load(PACK), load(PACK_OPEN))
    else {
        return;
    };
    let mut mat = |name: Option<&str>| {
        model_mats.get(name.unwrap_or("Canvas"), &mut lib, &server, &mut materials)
    };
    let mut hang =
        |commands: &mut Commands, parent: Entity, node: &ModelNode, flap: Option<bool>| {
            let at = commands
                .spawn((Transform::IDENTITY, Visibility::default(), ChildOf(parent)))
                .id();
            if let Some(open) = flap {
                commands.entity(at).insert((
                    Flap { open },
                    if open {
                        Visibility::Hidden
                    } else {
                        Visibility::Inherited
                    },
                ));
            }
            spawn_primitives(commands, at, node, &mut mat);
        };
    // The harness on every torso.
    for t in &torsos {
        commands.entity(t).remove::<WantsHarness>();
        if let Some(h) = rig.as_deref().and_then(|r| ModelNode::find(r, "harness")) {
            hang(&mut commands, t, h, None);
        }
    }
    // Each slot: its piece and both flaps, or its box if the model lacks it.
    for (e, socket, undressed) in &slots {
        commands.entity(e).remove::<Undressed>();
        let SocketKind::Body(kind) = socket.kind else {
            continue;
        };
        let (Some(name), Some(rig)) = (piece(kind), rig.as_deref()) else {
            continue;
        };
        let Some(body) = ModelNode::find(rig, name) else {
            continue;
        };
        hang(&mut commands, e, body, None);
        let closed = format!("{name}_flap_closed");
        let open = format!("{name}_flap_open");
        if let Some(n) = ModelNode::find(rig, &closed) {
            hang(&mut commands, e, n, Some(false));
        }
        if let Some(n) = rig_open.as_deref().and_then(|r| ModelNode::find(r, &open)) {
            hang(&mut commands, e, n, Some(true));
        }
        for b in &undressed.boxes {
            commands.entity(*b).despawn();
        }
    }
    // The pack: its bag, the lid shut, and the lid thrown back.
    for (e, undressed) in &bags {
        commands.entity(e).remove::<Undressed>();
        let (Some(pack), Some(pack_open)) = (pack.as_deref(), pack_open.as_deref()) else {
            continue;
        };
        let (Some(bag), Some(shut), Some(back)) = (
            ModelNode::find(pack, "pack"),
            ModelNode::find(pack, "pack_lid_closed"),
            ModelNode::find(pack_open, "pack_lid_open"),
        ) else {
            continue;
        };
        hang(&mut commands, e, bag, None);
        hang(&mut commands, e, shut, Some(false));
        hang(&mut commands, e, back, Some(true));
        for b in &undressed.boxes {
            commands.entity(*b).despawn();
        }
    }
}

/// A slot's flap is shut while it's empty and thrown open while it holds
/// something.
fn open_flaps(
    slots: Query<(&Socket, &Children), With<BodySlot>>,
    mut flaps: Query<(&Flap, &mut Visibility)>,
) {
    for (socket, children) in &slots {
        let full = socket.occupant.is_some();
        for c in children.iter() {
            if let Ok((flap, mut v)) = flaps.get_mut(c) {
                v.set_if_neq(if flap.open == full {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }
    }
}

/// The pack's lid is thrown back while the pack hangs open.
fn open_lids(bags: Query<(&Backpack, &Children)>, mut flaps: Query<(&Flap, &mut Visibility)>) {
    for (bag, children) in &bags {
        for c in children.iter() {
            if let Ok((flap, mut v)) = flaps.get_mut(c) {
                v.set_if_neq(if flap.open == bag.open {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The node names of a .glb.
    fn glb_nodes(path: &str) -> Vec<String> {
        let path = crate::asset_root_path().join(path);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (run art/build.py rig)", path.display()));
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
        json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["mesh"].is_u64())
            .filter_map(|n| n["name"].as_str().map(str::to_owned))
            .collect()
    }

    /// Every slot kind that draws something has its piece and both flaps,
    /// the torso its harness, the pack its bag and both lids: a missing
    /// node leaves that slot on its box, or a flap that never opens.
    #[test]
    fn every_worn_piece_is_modelled() {
        let rig = glb_nodes(RIG);
        let open = glb_nodes(RIG_OPEN);
        let mut want: Vec<(String, &Vec<String>)> = vec![("harness".into(), &rig)];
        for kind in extraction_core::inventory::DEFAULT_SLOTS {
            let Some(name) = piece(*kind) else {
                continue;
            };
            want.push((name.into(), &rig));
            if name != "mount" {
                want.push((format!("{name}_flap_closed"), &rig));
                want.push((format!("{name}_flap_open"), &open));
            }
        }
        let pack = glb_nodes(PACK);
        let pack_open = glb_nodes(PACK_OPEN);
        want.push(("pack".into(), &pack));
        want.push(("pack_lid_closed".into(), &pack));
        want.push(("pack_lid_open".into(), &pack_open));
        for (name, file) in want {
            assert!(file.contains(&name), "no {name} in the kit models");
        }
    }
}
