//! Far stand-ins drawn merged, a block at a time, while every one of them
//! would be drawn whole.
//!
//! Past `lod::FAR_BUILDING` a building is one vertex-coloured mesh of its
//! shell in one shared material (`lod::StandIn`); past the forest's far
//! LOD a chunk's trunks and crowns are too (`trees::FarForest`). Each is
//! still a draw: at Marrow Creek's south gate, 47 buildings' and 18 chunks'
//! an eye. Here every far stand-in in a [`BLOCK`]-metre block that shares
//! a material (and vertex layout) is merged into one mesh, which stands in
//! for them; theirs are only hidden.
//!
//! A stand-in fades in past its near meshes and out at the fog, each by its
//! own distance (its `VisibilityRange`, with crossfades). A merged block
//! can't, so it's shown only while every stand-in in it is, for every eye,
//! inside its own range clear of both crossfades: there each would be
//! drawn whole, as the merged mesh draws it. Otherwise they're their own,
//! each to fade as it did. Ranges are read as they are each frame (the
//! forest's far end moves with the weather).
//!
//! A stand-in that comes, goes (a building redressed) or moves (a far
//! robot walking) undoes its block until the block's stand-ins have stood
//! still [`SETTLE`] frames, when it's merged afresh.
//!
//! A stand-in's mesh is kept on the GPU alone (`RenderAssetUsages::
//! RENDER_WORLD`): its data is gone from here once it's drawn. So each is
//! copied when first seen, the frame it comes, while it's still here
//! (see [`Copies`]); one never seen readable draws as itself.
//!
//! `EXTRACTION_FAR_REST=0` leaves every stand-in on its own.

use bevy::camera::visibility::VisibilityRange;
use bevy::mesh::Indices;
use bevy::prelude::*;
use std::collections::HashMap;

pub struct FarRestPlugin;

impl Plugin for FarRestPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("EXTRACTION_FAR_REST").is_ok_and(|v| v == "0") {
            return;
        }
        app.init_resource::<Blocks>()
            .init_resource::<Copies>()
            .add_systems(
                PostUpdate,
                (find_stand_ins, merge_blocks, show_blocks)
                    .chain()
                    .after(TransformSystems::Propagate)
                    .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            );
    }
}

/// A block's side, metres.
pub const BLOCK: f32 = 192.0;
/// Frames a block's stand-ins stand unchanged before it's merged.
const SETTLE: u32 = 20;
/// Blocks merged a frame, at most.
const PER_FRAME: usize = 2;
/// How far inside its range, past each crossfade, a stand-in must be for
/// its block to be shown merged (metres).
const CLEAR: f32 = 1.0;

/// A merged stand-in for a block's far stand-ins of one material.
#[derive(Component)]
pub struct FarRestMesh;

#[derive(Default)]
struct Block {
    /// Its stand-ins, sorted.
    members: Vec<Entity>,
    /// Frames its stand-ins have stood as they are.
    quiet: u32,
    merged: Option<Merged>,
}

struct Merged {
    meshes: Vec<Entity>,
    /// The stand-ins the merged meshes hold, and where each's bounds are
    /// centred (as its range measures).
    hidden: Vec<(Entity, Vec3)>,
    /// Its stand-ins that were whole when it was merged (some may be
    /// alone in their material, and not in it).
    whole: Vec<Entity>,
    shown: bool,
    /// Frames since it was merged: shown only from its second (a mesh
    /// spawned now has no transform propagated until the next frame).
    age: u32,
}

/// A copy of each far stand-in's mesh kept only on the GPU, taken the
/// frame the stand-in came (before its data is sent away), to merge from.
/// Dropped when the mesh is; one changed is copied again if it can be.
#[derive(Resource, Default)]
pub(crate) struct Copies {
    by: HashMap<AssetId<Mesh>, Mesh>,
}

impl Copies {
    /// Bytes held, roughly (positions, normals and so on, and indices).
    pub(crate) fn bytes(&self) -> usize {
        self.by
            .values()
            .map(|m| {
                m.count_vertices() * m.get_vertex_size() as usize
                    + m.try_indices().map_or(0, |i| i.len() * 4)
            })
            .sum()
    }
}

/// A mesh readable to merge: its own data, or the copy of it.
fn readable<'a>(m: &'a Mesh, id: AssetId<Mesh>, copies: &'a Copies) -> Option<&'a Mesh> {
    if m.try_indices().is_ok() {
        Some(m)
    } else {
        copies.by.get(&id)
    }
}

/// Copied if it's kept on the GPU alone and still readable now.
fn copy(copies: &mut Copies, meshes: &Assets<Mesh>, id: AssetId<Mesh>) {
    if copies.by.contains_key(&id) {
        return;
    }
    let Some(m) = meshes.get(id) else {
        return;
    };
    if m.asset_usage == bevy::asset::RenderAssetUsages::RENDER_WORLD && m.try_indices().is_ok() {
        copies.by.insert(id, m.clone());
    }
}

#[derive(Resource, Default)]
struct Blocks {
    by: HashMap<(i32, i32), Block>,
}

fn block_of(p: Vec3) -> (i32, i32) {
    ((p.x / BLOCK).floor() as i32, (p.z / BLOCK).floor() as i32)
}

/// A far stand-in: it fades in past something nearer (its range starts
/// past zero), and casts no shadow.
fn is_stand_in(range: &VisibilityRange) -> bool {
    range.start_margin.end > 0.0 && range.use_aabb
}

/// Which stand-ins are in which block; a block whose stand-ins changed is
/// stand-ins again.
#[allow(clippy::type_complexity)]
fn find_stand_ins(
    mut commands: Commands,
    mut blocks: ResMut<Blocks>,
    mut copies: ResMut<Copies>,
    meshes: Res<Assets<Mesh>>,
    added: Query<(), Added<Mesh3d>>,
    fresh: Query<(&Mesh3d, &VisibilityRange), Changed<Mesh3d>>,
    mut gone: RemovedComponents<Mesh3d>,
    stand_ins: Query<
        (Entity, &GlobalTransform, &VisibilityRange),
        (
            With<Mesh3d>,
            With<MeshMaterial3d<StandardMaterial>>,
            With<bevy::light::NotShadowCaster>,
            Without<FarRestMesh>,
        ),
    >,
    moved: Query<
        Entity,
        (
            Changed<GlobalTransform>,
            With<bevy::light::NotShadowCaster>,
            With<VisibilityRange>,
            Without<FarRestMesh>,
        ),
    >,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    swapped: Query<Entity, (Changed<Mesh3d>, With<bevy::light::NotShadowCaster>)>,
    member_meshes: Query<&Mesh3d>,
    mut vis: Query<&mut Visibility>,
) {
    // A stand-in whose mesh is swapped or changed is held wrong by a
    // merged mesh too: its block is undone the same way.
    let mut modified: std::collections::HashSet<AssetId<Mesh>> = Default::default();
    for ev in mesh_events.read() {
        match ev {
            AssetEvent::Modified { id } => {
                modified.insert(*id);
                // A copy changed is copied again, if it still can be.
                if copies.by.remove(id).is_some() {
                    copy(&mut copies, &meshes, *id);
                }
            }
            AssetEvent::Removed { id } | AssetEvent::Unused { id } => {
                copies.by.remove(id);
            }
            _ => {}
        }
    }
    // Stand-ins come and meshes swapped are copied while they're still
    // here (see `Copies`).
    for (mesh, range) in &fresh {
        if is_stand_in(range) {
            copy(&mut copies, &meshes, mesh.0.id());
        }
    }
    if !modified.is_empty() || !swapped.is_empty() {
        let swapped: std::collections::HashSet<Entity> = swapped.iter().collect();
        for block in blocks.by.values_mut() {
            let hit = block.members.iter().any(|e| {
                swapped.contains(e)
                    || member_meshes
                        .get(*e)
                        .is_ok_and(|m| modified.contains(&m.0.id()))
            });
            if hit {
                if let Some(m) = block.merged.take() {
                    undo(&mut commands, &m, &mut vis);
                }
                block.quiet = 0;
            }
        }
    }
    // A stand-in that moves (a far robot walking) can't be held in a
    // merged mesh, which would keep it where it was: its block is undone,
    // and waits until all of its stand-ins stand still again.
    if !moved.is_empty() {
        let moved: std::collections::HashSet<Entity> = moved.iter().collect();
        for block in blocks.by.values_mut() {
            if block.members.iter().any(|e| moved.contains(e)) {
                if let Some(m) = block.merged.take() {
                    undo(&mut commands, &m, &mut vis);
                }
                block.quiet = 0;
            }
        }
    }
    if !added.is_empty() || gone.read().count() > 0 {
        let mut now: HashMap<(i32, i32), Vec<Entity>> = HashMap::new();
        for (e, gt, range) in &stand_ins {
            if is_stand_in(range) {
                now.entry(block_of(gt.translation())).or_default().push(e);
            }
        }
        for list in now.values_mut() {
            list.sort();
        }
        let mut keys: Vec<(i32, i32)> = blocks.by.keys().chain(now.keys()).copied().collect();
        keys.sort_unstable();
        keys.dedup();
        for k in keys {
            let list = now.remove(&k).unwrap_or_default();
            let block = blocks.by.entry(k).or_default();
            if block.members == list {
                continue;
            }
            if let Some(m) = block.merged.take() {
                undo(&mut commands, &m, &mut vis);
            }
            block.members = list;
            block.quiet = 0;
        }
        blocks.by.retain(|_, b| !b.members.is_empty());
    }
    for block in blocks.by.values_mut() {
        if block.merged.is_none() {
            block.quiet = block.quiet.saturating_add(1);
        }
    }
}

fn undo(commands: &mut Commands, m: &Merged, vis: &mut Query<&mut Visibility>) {
    for e in &m.meshes {
        commands.entity(*e).try_despawn();
    }
    for (e, _) in &m.hidden {
        if let Ok(mut v) = vis.get_mut(*e) {
            *v = Visibility::Inherited;
        }
    }
}

/// What can be merged into one mesh: the same vertex layout, attribute
/// by attribute and format by format (a merge of unlike ones fails part
/// way, its mesh holding some of the other's triangles).
fn layout(m: &Mesh) -> Vec<u64> {
    let mut ids: Vec<u64> = m
        .attributes()
        .map(|(a, _)| {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            a.id.hash(&mut h);
            format!("{:?}", a.format).hash(&mut h);
            h.finish()
        })
        .collect();
    ids.sort_unstable();
    ids
}

/// `m` with 32-bit indices: merged, a block's vertices pass 65 535, which
/// 16-bit indices would wrap.
fn wide(mut m: Mesh) -> Mesh {
    if let Some(Indices::U16(i)) = m.indices() {
        let wide: Vec<u32> = i.iter().map(|&x| x as u32).collect();
        m.insert_indices(Indices::U32(wide));
    }
    m
}

/// `m` as drawn by `world` (its whole affine: a child turned under a
/// parent scaled unevenly is sheared, which a translation, rotation and
/// scale can't hold, so `Mesh::transformed_by` put it metres out), in the
/// frame of a block centred at `centre`.
fn into_frame(mut m: Mesh, world: bevy::math::Affine3A, centre: Vec3) -> Mesh {
    use bevy::mesh::VertexAttributeValues as V;
    let lin = bevy::math::Mat3::from(world.matrix3);
    let normal = lin.inverse().transpose();
    if let Some(V::Float32x3(p)) = m.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        for v in p.iter_mut() {
            *v = (world.transform_point3(Vec3::from(*v)) - centre).to_array();
        }
    }
    if let Some(V::Float32x3(n)) = m.attribute_mut(Mesh::ATTRIBUTE_NORMAL) {
        for v in n.iter_mut() {
            *v = (normal * Vec3::from(*v)).normalize_or_zero().to_array();
        }
    }
    if let Some(V::Float32x4(t)) = m.attribute_mut(Mesh::ATTRIBUTE_TANGENT) {
        for v in t.iter_mut() {
            let d = (lin * Vec3::new(v[0], v[1], v[2])).normalize_or_zero();
            *v = [d.x, d.y, d.z, v[3]];
        }
    }
    // A mirroring transform turns triangles inside out: wound back.
    if lin.determinant() < 0.0 {
        if let Some(Indices::U32(i)) = m.indices_mut() {
            for t in i.chunks_mut(3) {
                t.swap(1, 2);
            }
        }
    }
    m
}

/// A block's stand-ins, settled, merged per material and layout; a group
/// of one is left as it is.
fn merge_blocks(
    mut commands: Commands,
    mut blocks: ResMut<Blocks>,
    stand_ins: Query<(
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
        &GlobalTransform,
        Option<&bevy::camera::primitives::Aabb>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    copies: Res<Copies>,
    materials: Res<Assets<StandardMaterial>>,
    cams: crate::render::puff::Eyes,
    ranges: Query<&VisibilityRange>,
) {
    let eyes = eyes(&cams);
    if eyes.is_empty() {
        return;
    }
    let mut done = 0;
    for (k, block) in blocks.by.iter_mut() {
        if done >= PER_FRAME {
            break;
        }
        if block.merged.is_some() || block.quiet < SETTLE {
            continue;
        }
        let centre = Vec3::new((k.0 as f32 + 0.5) * BLOCK, 0.0, (k.1 as f32 + 0.5) * BLOCK);
        type Group = (
            Handle<StandardMaterial>,
            Vec<u64>,
            Vec<(Entity, Mesh, Vec3)>,
        );
        let mut groups: Vec<Group> = Vec::new();
        let mut ready = true;
        let mut whole = Vec::new();
        for e in &block.members {
            let Ok((mesh, mat, gt, aabb)) = stand_ins.get(*e) else {
                ready = false;
                break;
            };
            let (Some(m), Some(aabb)) = (meshes.get(&mesh.0), aabb) else {
                ready = false;
                break;
            };
            let at = gt.transform_point(Vec3::from(aabb.center));
            // Only what's drawn whole now (the rest draw as themselves;
            // the block is merged again when that changes). Noted before
            // anything else rules it out, as `show_blocks` sees it.
            if !ranges.get(*e).is_ok_and(|r| all_whole(r, at, &eyes)) {
                continue;
            }
            whole.push(*e);
            // (Blended things are sorted one by one: merged, they'd draw in
            // another order.)
            let blended = materials
                .get(&mat.0)
                .is_none_or(|m| matches!(m.alpha_mode, AlphaMode::Blend));
            // (A mesh whose data went to the GPU alone, never copied,
            // can't be read to merge: it draws as itself.)
            let Some(m) = readable(m, mesh.0.id(), &copies) else {
                continue;
            };
            if blended || m.primitive_topology() != bevy::mesh::PrimitiveTopology::TriangleList {
                continue;
            }
            let lay = layout(m);
            let moved = into_frame(wide(m.clone()), gt.affine(), centre);
            match groups
                .iter_mut()
                .find(|(gm, gl, _)| *gm == mat.0 && *gl == lay)
            {
                Some((_, _, list)) => list.push((*e, moved, at)),
                None => groups.push((mat.0.clone(), lay, vec![(*e, moved, at)])),
            }
        }
        if !ready {
            continue;
        }
        done += 1;
        let mut merged = Merged {
            meshes: Vec::new(),
            hidden: Vec::new(),
            whole,
            shown: false,
            age: 0,
        };
        for (mat, _, list) in groups {
            if list.len() < 2 {
                continue;
            }
            let mut it = list.into_iter();
            let (first, mut into, at) = it.next().unwrap();
            let mut took = vec![(first, at)];
            for (e, m, at) in it {
                if into.merge(&m).is_ok() {
                    took.push((e, at));
                }
            }
            if took.len() < 2 {
                continue;
            }
            let id = commands
                .spawn((
                    Name::new("Far stand-ins"),
                    FarRestMesh,
                    Mesh3d(meshes.add(into)),
                    MeshMaterial3d(mat),
                    bevy::light::NotShadowCaster,
                    Transform::from_translation(centre),
                    GlobalTransform::from_translation(centre),
                    Visibility::Hidden,
                ))
                .id();
            merged.meshes.push(id);
            merged.hidden.extend(took);
        }
        block.merged = Some(merged);
    }
}

/// Whether a stand-in `d` metres away is drawn whole by its range: past
/// its fade in, short of its fade out.
fn whole(range: &VisibilityRange, d: f32) -> bool {
    d >= range.start_margin.end + CLEAR && d <= range.end_margin.start - CLEAR
}

/// Whole for every eye.
fn all_whole(range: &VisibilityRange, at: Vec3, eyes: &[Vec3]) -> bool {
    eyes.iter().all(|e| whole(range, e.distance(at)))
}

fn eyes(cams: &crate::render::puff::Eyes) -> Vec<Vec3> {
    cams.iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .collect()
}

/// Each merged block shown, the stand-ins it holds hidden, while each of
/// them is still drawn whole for every eye. When one isn't (the eye came
/// near enough for it to fade to its near meshes, or the fog closed in),
/// or one that wasn't now is, the block is its stand-ins again and is
/// merged afresh, of those whole then.
fn show_blocks(
    mut commands: Commands,
    mut blocks: ResMut<Blocks>,
    cams: crate::render::puff::Eyes,
    ranges: Query<&VisibilityRange>,
    centres: Query<(&GlobalTransform, Option<&bevy::camera::primitives::Aabb>)>,
    mut vis: Query<&mut Visibility>,
    copies: Res<Copies>,
    mut said: Local<(usize, usize)>,
) {
    let eyes = eyes(&cams);
    // `EXTRACTION_FAR_REST_LOG=1`: how many merged blocks are shown, and
    // how many stand-ins they hold, when that changes.
    if std::env::var_os("EXTRACTION_FAR_REST_LOG").is_some() {
        let shown: Vec<&Merged> = blocks
            .by
            .values()
            .filter_map(|b| b.merged.as_ref())
            .filter(|m| m.shown)
            .collect();
        let now = (
            shown.iter().map(|m| m.meshes.len()).sum(),
            shown.iter().map(|m| m.hidden.len()).sum(),
        );
        if now != *said {
            eprintln!(
                "far_rest: {} merged meshes shown, holding {} stand-ins; {} meshes copied, {} KiB",
                now.0,
                now.1,
                copies.by.len(),
                copies.bytes() / 1024
            );
            *said = now;
        }
    }
    for block in blocks.by.values_mut() {
        let Some(m) = &mut block.merged else {
            continue;
        };
        m.age = m.age.saturating_add(1);
        // Which of its stand-ins are whole now.
        let whole_now = |e: Entity| {
            let Ok((gt, aabb)) = centres.get(e) else {
                return false;
            };
            let at = aabb.map_or(gt.translation(), |a| {
                gt.transform_point(Vec3::from(a.center))
            });
            !eyes.is_empty() && ranges.get(e).is_ok_and(|r| all_whole(r, at, &eyes))
        };
        let held: std::collections::HashSet<Entity> = m.whole.iter().copied().collect();
        let changed = block
            .members
            .iter()
            .any(|e| held.contains(e) != whole_now(*e));
        if changed {
            let m = block.merged.take().unwrap();
            undo(&mut commands, &m, &mut vis);
            continue;
        }
        if m.meshes.is_empty() || m.shown || m.age < 2 {
            continue;
        }
        m.shown = true;
        for e in &m.meshes {
            if let Ok(mut v) = vis.get_mut(*e) {
                *v = Visibility::Inherited;
            }
        }
        for (e, _) in &m.hidden {
            if let Ok(mut v) = vis.get_mut(*e) {
                *v = Visibility::Hidden;
            }
        }
    }
}

/// What a stand-in is doing, for the gauges.
#[cfg(test)]
pub(crate) fn state_of(w: &World, e: Entity) -> &'static str {
    let Some(blocks) = w.get_resource::<Blocks>() else {
        return "off";
    };
    for b in blocks.by.values() {
        if !b.members.contains(&e) {
            continue;
        }
        return match &b.merged {
            Some(m) if m.hidden.iter().any(|(h, _)| *h == e) => "merged",
            Some(m) if m.whole.contains(&e) => "alone in its material",
            Some(_) => "not whole",
            None => "settling",
        };
    }
    "no block"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            TransformPlugin,
            bevy::camera::visibility::VisibilityPlugin,
            FarRestPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
        .init_resource::<Sent>()
        .add_systems(Last, send_to_gpu);
        app
    }

    /// Each mesh kept on the GPU alone, as it was before its data went.
    #[derive(Resource, Default)]
    struct Sent(HashMap<AssetId<Mesh>, Mesh>);

    /// What the render world's extraction does at the end of a frame: a
    /// mesh kept on the GPU alone has its data taken, unnoticed here.
    fn send_to_gpu(mut meshes: ResMut<Assets<Mesh>>, mut sent: ResMut<Sent>) {
        let ids: Vec<AssetId<Mesh>> = meshes.ids().collect();
        for id in ids {
            let Some(m) = meshes.get_mut_untracked(id) else {
                continue;
            };
            if m.asset_usage == bevy::asset::RenderAssetUsages::RENDER_WORLD
                && m.try_indices().is_ok()
            {
                sent.0.insert(id, m.clone());
                let _ = m.take_gpu_data();
            }
        }
    }

    /// A far stand-in as `lod::StandIn` leaves one: vertex-coloured, in
    /// the shared stand-in material, ranged past its near meshes, casting
    /// nothing.
    fn stand_in(app: &mut App, at: Vec3, mat: &Handle<StandardMaterial>) -> Entity {
        let w = app.world_mut();
        let mut mesh = Mesh::from(Cuboid::new(6.0, 4.0, 8.0));
        let n = mesh.count_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.3, 0.25, 0.2, 1.0]; n]);
        // Kept on the GPU alone, as `StandIn::build` makes them.
        mesh.asset_usage = bevy::asset::RenderAssetUsages::RENDER_WORLD;
        let mesh = w.resource_mut::<Assets<Mesh>>().add(mesh);
        w.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat.clone()),
            bevy::light::NotShadowCaster,
            crate::render::lod::far_range(),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_y(at.x * 0.01)),
        ))
        .id()
    }

    fn shown(app: &App, e: Entity) -> bool {
        *app.world().get::<Visibility>(e).unwrap() != Visibility::Hidden
    }

    fn merged(app: &mut App) -> Vec<Entity> {
        let w = app.world_mut();
        w.query_filtered::<Entity, With<FarRestMesh>>()
            .iter(w)
            .collect()
    }

    /// World-space triangles of a mesh entity, each corner to a centimetre,
    /// sorted.
    fn triangles(app: &App, e: Entity) -> Vec<[[i64; 3]; 3]> {
        let w = app.world();
        let g = w.get::<GlobalTransform>(e).unwrap().affine();
        let id = w.get::<Mesh3d>(e).unwrap().0.id();
        let m = w.resource::<Assets<Mesh>>().get(id).unwrap();
        let m = if m.try_indices().is_ok() {
            m
        } else {
            &w.resource::<Sent>().0[&id]
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
            m.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        let idx: Vec<usize> = m.indices().unwrap().iter().collect();
        idx.chunks(3)
            .map(|t| {
                let mut c = [0, 1, 2].map(|k| {
                    let v = g.transform_point3(Vec3::from(p[t[k]]));
                    [v.x, v.y, v.z].map(|x| (x * 100.0).round() as i64)
                });
                c.sort();
                c
            })
            .collect()
    }

    /// A block's stand-ins in the shared material: seen from far off,
    /// where each is drawn whole, they're one merged mesh holding exactly
    /// their triangles; from where one is fading in (its crossfade) or has
    /// gone to its near meshes, that one is its own and the merged mesh
    /// holds exactly the rest; one taken away (a building redressed) undoes
    /// the block until it's settled, when it's merged again.
    #[test]
    fn a_blocks_far_stand_ins_draw_merged_only_while_each_is_whole() {
        let mut app = app();
        let mat = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let ats = [
            Vec3::new(20.0, 0.0, 30.0),
            Vec3::new(60.0, 0.0, 40.0),
            Vec3::new(100.0, 0.0, 150.0),
            Vec3::new(150.0, 0.0, 90.0),
        ];
        let mut members: Vec<Entity> = ats.iter().map(|a| stand_in(&mut app, *a, &mat)).collect();
        // One turned under a parent scaled unevenly (an ivy card on a
        // stretched building): its transform is sheared.
        let parent = app
            .world_mut()
            .spawn((
                Transform::from_xyz(120.0, 0.0, 120.0).with_scale(Vec3::new(3.0, 1.0, 0.5)),
                Visibility::default(),
            ))
            .id();
        let sheared = stand_in(&mut app, Vec3::ZERO, &mat);
        app.world_mut().entity_mut(sheared).insert((
            Transform::from_rotation(Quat::from_rotation_y(0.7)),
            ChildOf(parent),
        ));
        members.push(sheared);
        // And a blended one among them (a haze card): never merged, drawn
        // as itself, and no reason for the block to be undone.
        let haze = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
        let blended = stand_in(&mut app, Vec3::new(80.0, 0.0, 80.0), &haze);
        // And one whose mesh's data went to the GPU before it was ever seen
        // (all of them go, as `RenderAssetUsages::RENDER_WORLD` leaves
        // them; the rest were copied the frame they came): it can't be
        // read, so it draws as itself, and nothing panics reading it.
        let gpu_only = {
            let w = app.world_mut();
            let mut mesh = Mesh::from(Cuboid::new(3.0, 3.0, 3.0));
            let n = mesh.count_vertices();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.3, 0.25, 0.2, 1.0]; n]);
            let _ = mesh.take_gpu_data();
            let mesh = w.resource_mut::<Assets<Mesh>>().add(mesh);
            w.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat.clone()),
                bevy::light::NotShadowCaster,
                crate::render::lod::far_range(),
                Transform::from_xyz(70.0, 0.0, 100.0),
            ))
            .id()
        };
        let eye = app
            .world_mut()
            .spawn((Camera3d::default(), Transform::from_xyz(90.0, 1.7, -400.0)))
            .id();
        for _ in 0..SETTLE + 5 {
            app.update();
        }
        let m = merged(&mut app);
        assert_eq!(m.len(), 1, "one material: one merged mesh");
        assert!(shown(&app, m[0]), "far: merged");
        assert!(shown(&app, blended), "a blended stand-in is its own");
        assert!(
            shown(&app, gpu_only),
            "one kept on the GPU alone is its own"
        );
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(
            merged(&mut app),
            m,
            "merged and left be, not merged again each frame"
        );
        assert!(
            members.iter().all(|e| !shown(&app, *e)),
            "far: stand-ins hidden"
        );
        let mut want: Vec<_> = members.iter().flat_map(|e| triangles(&app, *e)).collect();
        let mut got = triangles(&app, m[0]);
        want.sort();
        got.sort();
        assert_eq!(got, want, "the merged mesh draws what the stand-ins did");
        // One stand-in in its crossfade (125 m off), then near: it's its
        // own, the rest still merged, exactly theirs.
        for z in [ats[0].z - 125.0, ats[0].z - 20.0] {
            app.world_mut()
                .get_mut::<Transform>(eye)
                .unwrap()
                .translation = Vec3::new(ats[0].x, 1.7, z);
            for _ in 0..3 {
                app.update();
            }
            assert!(shown(&app, members[0]), "{z}: the one not whole is its own");
            holds_exactly_the_hidden(&mut app, &members);
        }
        // Far again: all merged. Then one goes: undone, and merged afresh.
        app.world_mut()
            .get_mut::<Transform>(eye)
            .unwrap()
            .translation = Vec3::new(90.0, 1.7, -400.0);
        for _ in 0..3 {
            app.update();
        }
        assert!(
            members.iter().all(|e| !shown(&app, *e)),
            "far again: all merged"
        );
        holds_exactly_the_hidden(&mut app, &members);
        // One moves (a far robot walking): undone at once, every stand-in
        // its own; still again, merged where it now is.
        app.world_mut()
            .get_mut::<Transform>(members[1])
            .unwrap()
            .translation += Vec3::new(5.0, 0.0, 3.0);
        app.update();
        assert!(merged(&mut app).is_empty(), "one moved: undone");
        assert!(
            members.iter().all(|e| shown(&app, *e)),
            "one moved: their own"
        );
        for _ in 0..SETTLE + 5 {
            app.update();
        }
        assert!(
            members.iter().all(|e| !shown(&app, *e)),
            "still again: merged"
        );
        holds_exactly_the_hidden(&mut app, &members);
        app.world_mut().entity_mut(members[3]).despawn();
        let rest: Vec<Entity> = members
            .iter()
            .copied()
            .filter(|e| *e != members[3])
            .collect();
        let held = app.world().resource::<Copies>().by.len();
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Copies>().by.len(),
            held - 1,
            "its copy goes with it"
        );
        assert!(merged(&mut app).is_empty(), "one gone: undone");
        assert!(rest.iter().all(|e| shown(&app, *e)));
        for _ in 0..SETTLE + 5 {
            app.update();
        }
        assert_eq!(merged(&mut app).len(), 1, "settled: merged again");
        assert!(rest.iter().all(|e| !shown(&app, *e)));
        holds_exactly_the_hidden(&mut app, &rest);
    }

    /// The merged meshes shown hold exactly the triangles of the stand-ins
    /// hidden, no more and no fewer.
    fn holds_exactly_the_hidden(app: &mut App, members: &[Entity]) {
        let m: Vec<Entity> = merged(app).into_iter().filter(|e| shown(app, *e)).collect();
        let mut want: Vec<_> = members
            .iter()
            .filter(|e| !shown(app, **e))
            .flat_map(|e| triangles(app, *e))
            .collect();
        let mut got: Vec<_> = m.iter().flat_map(|e| triangles(app, *e)).collect();
        want.sort();
        got.sort();
        assert_eq!(
            got, want,
            "the merged meshes draw what the hidden stand-ins did"
        );
    }

    /// A frame of `tris` (world space, each with its material) from `eye`
    /// looking at `at`: each pixel's nearest depth and material.
    fn frame(tris: &[([Vec3; 3], u64)], eye: Vec3, at: Vec3) -> Vec<Option<(f32, u64)>> {
        use crate::render::overdraw::{RES, raster_with};
        let view = Transform::from_translation(eye)
            .looking_at(at, Vec3::Y)
            .compute_affine()
            .inverse();
        let mut px: Vec<Option<(f32, u64)>> = vec![None; RES * RES];
        for (t, m) in tris {
            let v = t.map(|p| view.transform_point3(p));
            raster_with(&v, 0.05, 1.0, true, |i, d| {
                if px[i].is_none_or(|(o, _)| d < o) {
                    px[i] = Some((d, *m));
                }
                true
            });
        }
        px
    }

    /// Every mesh entity's world-space triangles, with its material.
    fn world_tris(w: &World, list: &[Entity]) -> Vec<([Vec3; 3], u64)> {
        use std::hash::{Hash, Hasher};
        let mut out = Vec::new();
        for e in list {
            let g = w.get::<GlobalTransform>(*e).unwrap().affine();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            w.get::<MeshMaterial3d<StandardMaterial>>(*e)
                .unwrap()
                .0
                .id()
                .hash(&mut h);
            let mat = h.finish();
            let m = w
                .resource::<Assets<Mesh>>()
                .get(&w.get::<Mesh3d>(*e).unwrap().0)
                .unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
                m.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let idx: Vec<usize> = m.indices().unwrap().iter().collect();
            for t in idx.chunks(3) {
                out.push((
                    [0, 1, 2].map(|k| g.transform_point3(Vec3::from(p[t[k]]))),
                    mat,
                ));
            }
        }
        out
    }

    /// How far apart two triangles' corners are, matched up whichever way
    /// round fits best (sorting corners by coordinate isn't enough: corners
    /// a hair apart in x sort either way).
    fn apart(a: &[Vec3; 3], b: &[Vec3; 3]) -> f32 {
        const ORDERS: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        ORDERS
            .iter()
            .map(|o| (0..3).map(|k| a[k].distance(b[o[k]])).fold(0f32, f32::max))
            .fold(f32::INFINITY, f32::min)
    }

    /// The furthest any triangle of `a` is from its nearest in `b` in its
    /// material (corners matched whichever way round fits), searched among
    /// those whose middles are within a centimetre-grid cell of its own.
    fn furthest(a: &[([Vec3; 3], u64)], b: &[([Vec3; 3], u64)]) -> f32 {
        let cell = |t: &[Vec3; 3]| {
            let c = (t[0] + t[1] + t[2]) / 3.0 * 100.0;
            (c.x.floor() as i64, c.y.floor() as i64, c.z.floor() as i64)
        };
        let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, t) in b.iter().enumerate() {
            grid.entry(cell(&t.0)).or_default().push(i);
        }
        let mut worst = 0f32;
        for t in a {
            let (x, y, z) = cell(&t.0);
            let mut best = f32::INFINITY;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        for &i in grid.get(&(x + dx, y + dy, z + dz)).into_iter().flatten() {
                            if b[i].1 == t.1 {
                                best = best.min(apart(&t.0, &b[i].0));
                            }
                        }
                    }
                }
            }
            worst = worst.max(best);
        }
        worst
    }

    /// Whether two lists hold the same triangles (corners within 1 mm, in
    /// any order) in the same materials: as many of each, every one of
    /// either within a millimetre of one of the other's.
    fn same_triangles(a: &[([Vec3; 3], u64)], b: &[([Vec3; 3], u64)]) -> bool {
        a.len() == b.len() && furthest(a, b) < 1e-3 && furthest(b, a) < 1e-3
    }

    /// How two triangle lists differ, for the report.
    fn how_unlike(a: &[([Vec3; 3], u64)], b: &[([Vec3; 3], u64)]) -> String {
        format!(
            "{} triangles against {}; the furthest of the first from the second {:.2e} m, \
             of the second from the first {:.2e} m",
            a.len(),
            b.len(),
            furthest(a, b),
            furthest(b, a)
        )
    }

    /// Every valley: each block's merged stand-ins hold exactly its
    /// stand-ins' triangles, and each stand-in, seen from 200 m off, is
    /// drawn the same merged as on its own, pixel for pixel (nearest
    /// depth, material). A pixel where one surface passes another may
    /// round either way (a vertex moved into the block's frame on the
    /// CPU), at most one in 10 000.
    #[test]
    #[ignore = "gauge: loads every valley"]
    fn gauge_every_far_stand_in_draws_merged_as_its_own() {
        let mut bad = Vec::new();
        for (map, _) in extraction_core::map::VALLEYS {
            let mut app =
                crate::render::budget::tests::client_flash(map, "clear", None, false, None);
            for _ in 0..400 {
                app.update();
            }
            let blocks: Vec<(Vec<Entity>, Vec<Entity>)> = app
                .world()
                .resource::<Blocks>()
                .by
                .values()
                .filter_map(|b| b.merged.as_ref())
                .filter(|m| !m.meshes.is_empty())
                .map(|m| (m.meshes.clone(), m.hidden.iter().map(|(e, _)| *e).collect()))
                .collect();
            let w = app.world();
            let (mut members, mut unlike, mut differ, mut covered) = (0, 0, 0, 0);
            for (merged, hidden) in &blocks {
                members += hidden.len();
                let own = world_tris(w, hidden);
                let as_merged = world_tris(w, merged);
                if !same_triangles(&own, &as_merged) {
                    unlike += 1;
                    eprintln!("  unlike: {}", how_unlike(&own, &as_merged));
                }
                for e in hidden {
                    let at = w.get::<GlobalTransform>(*e).unwrap().translation();
                    let eye = at + Vec3::new(140.0, 30.0, 140.0);
                    let a = frame(&own, eye, at);
                    let m = frame(&as_merged, eye, at);
                    for (p, q) in a.iter().zip(&m) {
                        match (p, q) {
                            (None, None) => {}
                            (Some((d0, m0)), Some((d1, m1))) => {
                                covered += 1;
                                if (d0 - d1).abs() > 1e-3 * d0.max(1.0) || m0 != m1 {
                                    differ += 1;
                                }
                            }
                            _ => {
                                covered += 1;
                                differ += 1;
                            }
                        }
                    }
                }
            }
            eprintln!(
                "{map}: {} merged blocks ({unlike} not the same triangles), {members} stand-ins; \
                 {covered} pixels drawn, {differ} differ",
                blocks.len()
            );
            assert!(members > 0, "{map}: nothing merged");
            if unlike > 0 || differ * 10_000 > covered {
                bad.push(format!(
                    "{map}: {unlike} blocks not the same triangles; {differ} of {covered} pixels differ"
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "merged stand-ins drawn differently:\n{}",
            bad.join("\n")
        );
    }
}
