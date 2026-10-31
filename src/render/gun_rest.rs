//! A gun nobody is handling is drawn as one mesh per material.
//!
//! A gun's model is one entity per glTF primitive: its `static` body (one
//! primitive per material, joined in Blender), then each moving part
//! (bolt, selector, trigger, hammer...) and a seated magazine's own,
//! with every round in a drum or clip three more. An AK-74 with its
//! magazine is 19 draws, a PPSh with its drum 55, a Nagant 49. Those
//! parts move only in hand. So a gun lying in the truck, on a shelf, on
//! the ground (not held or steadied, not on the player) is drawn as its
//! parts' meshes merged per material, in the pose it was left in: a child
//! of the gun, so it still moves and fades with it. The moment a hand
//! takes it, or anything under it is taken off or put on, it is its parts
//! again. Node names, part entities and their transforms are untouched;
//! only the leaf meshes are hidden while the merged ones stand in.
//!
//! `EXTRACTION_GUN_REST=0` leaves every gun as its parts.

use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;

pub struct GunRestPlugin;

impl Plugin for GunRestPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("EXTRACTION_GUN_REST").is_ok_and(|v| v == "0") {
            return;
        }
        app.init_resource::<RestCache>().add_systems(
            PostUpdate,
            (wake_handled, rest_idle, finish_building)
                .chain()
                .after(TransformSystems::Propagate)
                .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
    }
}

/// Frames a gun must sit unhandled, its meshes unchanged, before it rests
/// (a model still loading, a magazine just seated, a gun just dropped).
const SETTLE: u32 = 30;
/// Guns put to rest a frame, at most (a valley's loot comes in at once).
const PER_FRAME: usize = 8;

/// On a gun drawn merged: the leaf meshes hidden (with the visibility
/// each had) and the merged meshes standing in.
#[derive(Component)]
pub struct AtRest {
    /// Every leaf under the gun when it was put to rest (sorted): a
    /// change in these wakes it.
    all: Vec<Entity>,
    leaves: Vec<(Entity, Visibility)>,
    merged: Vec<Entity>,
}

/// Merged meshes by what went into them: guns left alike share one.
#[derive(Resource, Default)]
struct RestCache {
    meshes: std::collections::HashMap<u64, Handle<Mesh>>,
}

/// Whether a gun is being handled: held or steadied, or anywhere on the
/// player (slung, holstered, in a pocket).
fn handled(
    gun: Entity,
    hands: &Query<
        (),
        Or<(
            With<crate::interact::Held>,
            With<crate::interact::Supported>,
        )>,
    >,
    players: &Query<(), With<crate::player::Player>>,
    parents: &Query<&ChildOf>,
) -> bool {
    if hands.contains(gun) {
        return true;
    }
    let mut at = gun;
    for _ in 0..24 {
        if players.contains(at) {
            return true;
        }
        match parents.get(at) {
            Ok(p) => at = p.parent(),
            Err(_) => return false,
        }
    }
    false
}

/// Every mesh under `gun` drawn with a standard material, and whether it
/// is shown.
fn leaves_of(
    gun: Entity,
    children: &Query<&Children>,
    meshes: &Query<
        (
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &InheritedVisibility,
        ),
        Without<bevy::mesh::skinning::SkinnedMesh>,
    >,
    merged: &Query<(), With<RestMesh>>,
    names: &Query<&Name>,
) -> Vec<Entity> {
    let live = |e: Entity| names.get(e).is_ok_and(|n| LIVE.contains(&n.as_str()));
    let mut out = Vec::new();
    let mut stack = vec![gun];
    while let Some(e) = stack.pop() {
        if merged.contains(e) || live(e) {
            continue;
        }
        // A lens (whatever holds a vignette) stays live too.
        if children.get(e).is_ok_and(|kids| {
            kids.iter()
                .any(|k| names.get(k).is_ok_and(|n| n.as_str() == "Vignette"))
        }) {
            continue;
        }
        if e != gun && meshes.contains(e) {
            out.push(e);
        }
        if let Ok(kids) = children.get(e) {
            stack.extend(kids.iter());
        }
    }
    out.sort();
    out
}

/// What changes look on a gun nobody holds, kept out of the merge (and
/// so drawn as it is):
/// - the muzzle crown fouls and nicks (a rod down the bore, oil touched
///   to a gun on a table: `weapons::wear`);
/// - a scope's vignette follows the eye through a rifle lying on a sill
///   (`weapons::optics`);
/// - a cold scope's lens fogs and clears (`weapons::weather`).
///
/// (Rain and grime on a gun lying out give its parts their own
/// materials once, which wakes it, see `wake_handled`; after that their
/// tint changes in the material the merged meshes share, and shows.)
const LIVE: [&str; 3] = ["Muzzle crown", "Vignette", "Lens fog"];

/// What a merged mesh shares: its material and how it casts the sun's
/// shadow.
#[derive(Clone, PartialEq)]
struct Shade {
    mat: Handle<StandardMaterial>,
    reach: Option<u32>,
    caster: bool,
}

/// A merged stand-in mesh (so a rest isn't merged into the next).
#[derive(Component)]
pub struct RestMesh;

/// A gun that's picked up, put on the player, or has something taken off
/// or put on it is its parts again.
#[allow(clippy::type_complexity)]
fn wake_handled(
    mut commands: Commands,
    resting: Query<(Entity, &AtRest)>,
    hands: Query<
        (),
        Or<(
            With<crate::interact::Held>,
            With<crate::interact::Supported>,
        )>,
    >,
    players: Query<(), With<crate::player::Player>>,
    parents: Query<&ChildOf>,
    moved: Query<(), Or<(Changed<ChildOf>, Changed<Children>)>>,
    mut unparented: RemovedComponents<ChildOf>,
    children: Query<&Children>,
    meshes: Query<
        (
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &InheritedVisibility,
        ),
        Without<bevy::mesh::skinning::SkinnedMesh>,
    >,
    merged: Query<(), With<RestMesh>>,
    names: Query<&Name>,
    rematerialled: Query<Entity, (Changed<MeshMaterial3d<StandardMaterial>>, Without<RestMesh>)>,
    mut vis: Query<&mut Visibility>,
) {
    // A merged part given a material of its own (a gun weathering): the
    // merged meshes still wear the old one.
    let rematerialled: std::collections::HashSet<Entity> = rematerialled.iter().collect();
    // (Something put on, taken off, or gone: removing a parent isn't a
    // change to one.)
    let reparented = !moved.is_empty() | (unparented.read().count() > 0);
    for (gun, rest) in &resting {
        let mut wake = handled(gun, &hands, &players, &parents);
        if !wake && !rematerialled.is_empty() {
            wake = rest.leaves.iter().any(|(e, _)| rematerialled.contains(e));
        }
        if !wake && reparented {
            // Something came or went: the leaves under it now aren't the
            // ones merged.
            wake = leaves_of(gun, &children, &meshes, &merged, &names) != rest.all;
        }
        if !wake {
            continue;
        }
        for (e, v) in &rest.leaves {
            if let Ok(mut cur) = vis.get_mut(*e) {
                *cur = *v;
            }
        }
        for m in &rest.merged {
            commands.entity(*m).try_despawn();
        }
        commands.entity(gun).remove::<AtRest>();
    }
}

/// Whether a mesh can be merged (`world_props::for_merge` takes it):
/// without building the merge, which generates tangents and cost a frame
/// its 31 ms a gun when asked every time. A mesh whose data went to the
/// GPU only (`RENDER_WORLD` usage) can't be: asked for its attributes it
/// panicked, a valley's first frames in (`part_rest`).
pub(crate) fn mergeable(m: &Mesh) -> bool {
    let has = |a| m.try_attribute_option(a).is_ok_and(|v| v.is_some());
    m.primitive_topology() == bevy::mesh::PrimitiveTopology::TriangleList
        && has(Mesh::ATTRIBUTE_POSITION)
        && has(Mesh::ATTRIBUTE_NORMAL)
        && has(Mesh::ATTRIBUTE_UV_0)
        && m.try_indices_option().is_ok_and(|i| i.is_some())
}

/// A gun whose merged meshes are being built off the main thread (a
/// merge was a 31 ms frame on it): put to rest when they're done, if it
/// is still lying as it was.
#[derive(Component)]
struct Building {
    task: bevy::tasks::Task<Vec<(u64, Option<Mesh>)>>,
    groups: Vec<(Shade, u64)>,
    all: Vec<Entity>,
    taken: Vec<(Entity, Visibility)>,
    range: Option<VisibilityRange>,
}

fn group_key(parts: &[(AssetId<Mesh>, Mat4)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (id, m) in parts {
        id.hash(&mut h);
        for f in m.to_cols_array() {
            ((f * 1e4).round() as i64).hash(&mut h);
        }
    }
    h.finish()
}

/// Puts a gun to rest with its merged meshes: stand-ins spawned, leaves
/// hidden.
fn put_to_rest(
    commands: &mut Commands,
    gun: Entity,
    groups: &[(Shade, Handle<Mesh>)],
    all: Vec<Entity>,
    taken: Vec<(Entity, Visibility)>,
    range: Option<VisibilityRange>,
) {
    let mut stand_ins = Vec::new();
    for (shade, handle) in groups {
        let mut ec = commands.spawn((
            Name::new("Gun at rest"),
            RestMesh,
            Mesh3d(handle.clone()),
            MeshMaterial3d(shade.mat.clone()),
            Transform::IDENTITY,
            ChildOf(gun),
        ));
        if let Some(r) = &range {
            ec.insert(r.clone());
        }
        match shade.reach {
            Some(bits) => {
                ec.insert(super::budget::ShadowReach(f32::from_bits(bits)));
            }
            None if !shade.caster => {
                ec.insert(bevy::light::NotShadowCaster);
            }
            None => {}
        }
        stand_ins.push(ec.id());
    }
    for (e, _) in &taken {
        commands.entity(*e).insert(Visibility::Hidden);
    }
    commands.entity(gun).insert(AtRest {
        all,
        leaves: taken,
        merged: stand_ins,
    });
}

/// A gun's merged meshes done building: put to rest, if nobody took it
/// and nothing under it changed meanwhile.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn finish_building(
    mut commands: Commands,
    mut building: Query<(Entity, &mut Building)>,
    mut cache: ResMut<RestCache>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    hands: Query<
        (),
        Or<(
            With<crate::interact::Held>,
            With<crate::interact::Supported>,
        )>,
    >,
    players: Query<(), With<crate::player::Player>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    meshes: Query<
        (
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &InheritedVisibility,
        ),
        Without<bevy::mesh::skinning::SkinnedMesh>,
    >,
    merged: Query<(), With<RestMesh>>,
    names: Query<&Name>,
) {
    use bevy::tasks::{block_on, futures_lite::future};
    for (gun, mut b) in &mut building {
        let Some(built) = block_on(future::poll_once(&mut b.task)) else {
            continue;
        };
        commands.entity(gun).remove::<Building>();
        for (key, mesh) in built {
            if let Some(mesh) = mesh {
                let h = mesh_assets.add(mesh);
                cache.meshes.insert(key, h);
            }
        }
        if handled(gun, &hands, &players, &parents)
            || leaves_of(gun, &children, &meshes, &merged, &names) != b.all
        {
            continue;
        }
        let groups: Option<Vec<(Shade, Handle<Mesh>)>> = b
            .groups
            .iter()
            .map(|(shade, key)| cache.meshes.get(key).map(|h| (shade.clone(), h.clone())))
            .collect();
        let Some(groups) = groups else {
            continue;
        };
        put_to_rest(
            &mut commands,
            gun,
            &groups,
            std::mem::take(&mut b.all),
            std::mem::take(&mut b.taken),
            b.range.take(),
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn rest_idle(
    mut commands: Commands,
    mut settling: Local<std::collections::HashMap<Entity, (usize, u32)>>,
    // Guns not worth merging, by how many leaves they had then: asked
    // again only when that changes.
    mut declined: Local<std::collections::HashMap<Entity, usize>>,
    cache: Res<RestCache>,
    mesh_assets: Res<Assets<Mesh>>,
    guns: Query<
        (Entity, &GlobalTransform, &InheritedVisibility),
        (
            With<crate::weapons::Gun>,
            Without<AtRest>,
            Without<Building>,
        ),
    >,
    hands: Query<
        (),
        Or<(
            With<crate::interact::Held>,
            With<crate::interact::Supported>,
        )>,
    >,
    players: Query<(), With<crate::player::Player>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    meshes: Query<
        (
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &InheritedVisibility,
        ),
        Without<bevy::mesh::skinning::SkinnedMesh>,
    >,
    merged: Query<(), With<RestMesh>>,
    names: Query<&Name>,
    leaf_info: Query<(
        &GlobalTransform,
        &Visibility,
        Option<&VisibilityRange>,
        Option<&super::budget::ShadowReach>,
        Has<bevy::light::NotShadowCaster>,
    )>,
) {
    settling.retain(|e, _| guns.contains(*e));
    declined.retain(|e, _| guns.contains(*e));
    let mut done = 0;
    for (gun, gun_gt, shown) in &guns {
        if done >= PER_FRAME {
            break;
        }
        if !shown.get() || handled(gun, &hands, &players, &parents) {
            settling.remove(&gun);
            continue;
        }
        let leaves = leaves_of(gun, &children, &meshes, &merged, &names);
        if declined.get(&gun) == Some(&leaves.len()) {
            continue;
        }
        let all = leaves.clone();
        let entry = settling.entry(gun).or_insert((usize::MAX, 0));
        if entry.0 != leaves.len() {
            *entry = (leaves.len(), 0);
            continue;
        }
        entry.1 += 1;
        if entry.1 < SETTLE || leaves.is_empty() {
            continue;
        }
        settling.remove(&gun);
        done += 1;
        // Per material: the shown leaves' meshes, in the gun's space.
        if gun_gt.compute_transform().scale.abs().min_element() < 1e-6 {
            continue;
        }
        let to_gun = gun_gt.affine().inverse();
        // By material and by how the leaf casts the sun's shadow (small
        // parts only near the eye, `budget::small_shadows`): merged, a
        // drum's rounds would cast at any distance.
        let mut groups: Vec<(Shade, Vec<(AssetId<Mesh>, Mat4)>)> = Vec::new();
        let mut taken: Vec<(Entity, Visibility)> = Vec::new();
        let mut range = None;
        for e in leaves {
            let Ok((mesh, mat, inherited)) = meshes.get(e) else {
                continue;
            };
            if !inherited.get() {
                continue;
            }
            let Ok((gt, v, r, reach, no_shadow)) = leaf_info.get(e) else {
                continue;
            };
            let Some(m) = mesh_assets.get(&mesh.0) else {
                continue;
            };
            if !mergeable(m) {
                continue;
            }
            if range.is_none() {
                range = r.cloned();
            }
            let local = Mat4::from(to_gun * gt.affine());
            // (A part shrunk to nothing to hide it draws nothing: left be.)
            let (scale, _, _) = local.to_scale_rotation_translation();
            if !scale.is_finite() || scale.abs().min_element() < 1e-6 {
                continue;
            }
            let shade = Shade {
                mat: mat.0.clone(),
                reach: reach.map(|r| r.0.to_bits()),
                // (A small part's flag is `ShadowReach`'s to set.)
                caster: reach.is_some() || !no_shadow,
            };
            match groups.iter_mut().find(|(k, _)| *k == shade) {
                Some((_, l)) => l.push((mesh.0.id(), local)),
                None => groups.push((shade, vec![(mesh.0.id(), local)])),
            }
            taken.push((e, *v));
        }
        // Not worth it for a gun that's a draw or two already.
        if taken.len() <= groups.len() {
            declined.insert(gun, all.len());
            continue;
        }
        let keyed: Vec<(Shade, u64, Vec<(AssetId<Mesh>, Mat4)>)> = groups
            .into_iter()
            .map(|(shade, parts)| {
                let key = group_key(&parts);
                (shade, key, parts)
            })
            .collect();
        // Guns left alike share merged meshes: when every group's is made,
        // rest now; else build the rest off the main thread.
        let hits: Option<Vec<(Shade, Handle<Mesh>)>> = keyed
            .iter()
            .map(|(shade, key, _)| cache.meshes.get(key).map(|h| (shade.clone(), h.clone())))
            .collect();
        if let Some(hits) = hits {
            put_to_rest(&mut commands, gun, &hits, all, taken, range);
            continue;
        }
        let work: Vec<(u64, Vec<(Mesh, Mat4)>)> = keyed
            .iter()
            .filter(|(_, key, _)| !cache.meshes.contains_key(key))
            .map(|(_, key, parts)| {
                let srcs = parts
                    .iter()
                    .filter_map(|(id, m)| mesh_assets.get(*id).map(|src| (src.clone(), *m)))
                    .collect();
                (*key, srcs)
            })
            .collect();
        let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
            work.into_iter()
                .map(|(key, srcs)| {
                    let mut out: Option<Mesh> = None;
                    for (src, m) in srcs {
                        let Some(part) = crate::render::world_props::for_merge(&src) else {
                            continue;
                        };
                        let part = part.transformed_by(Transform::from_matrix(m));
                        match &mut out {
                            Some(o) => {
                                let _ = o.merge(&part);
                            }
                            None => out = Some(part),
                        }
                    }
                    (key, out)
                })
                .collect()
        });
        commands.entity(gun).insert(Building {
            task,
            groups: keyed
                .into_iter()
                .map(|(shade, key, _)| (shade, key))
                .collect(),
            all,
            taken,
            range,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the guns in a fresh valley are doing: resting, handled, or
    /// neither (and why).
    fn census(app: &mut App) -> (usize, usize, usize, usize) {
        let w = app.world_mut();
        let guns: Vec<Entity> = w
            .query_filtered::<Entity, With<crate::weapons::Gun>>()
            .iter(w)
            .collect();
        let resting = w.query_filtered::<(), With<AtRest>>().iter(w).count();
        let mut on_player = 0;
        let mut hidden = 0;
        for g in &guns {
            let mut at = *g;
            for _ in 0..24 {
                if w.get::<crate::player::Player>(at).is_some() {
                    on_player += 1;
                    break;
                }
                match w.get::<ChildOf>(at) {
                    Some(p) => at = p.parent(),
                    None => break,
                }
            }
            if w.get::<InheritedVisibility>(*g).is_some_and(|v| !v.get()) {
                hidden += 1;
            }
        }
        (guns.len(), resting, on_player, hidden)
    }

    /// The valley in (`settle`: no new meshes for a while), the guns'
    /// time to settle, and every merge built (`Building`). The merges run
    /// on Bevy's task pools, which are the process's: beside the rest of
    /// the gauge tier, other valleys' loads queue ahead of them, and a
    /// fixed count of frames found 16 of 17 guns resting, or none.
    fn settled(app: &mut App) {
        super::super::budget::tests::settle(app);
        for frame in 0..6000 {
            app.update();
            let w = app.world_mut();
            let building = w.query_filtered::<(), With<Building>>().iter(w).count();
            if frame >= SETTLE as usize * 3 + 60 && building == 0 {
                return;
            }
        }
        panic!("guns' merges still building after 6000 frames");
    }

    /// Guns lying in a valley rest; the drawn meshes under a resting gun
    /// are its merged ones only.
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_guns_lying_about_rest() {
        // Counts: guns at rest, their meshes.
        let _counting = super::super::budget::tests::counted();
        let mut app = super::super::budget::tests::client_threads("marrow_creek", "clearing", None);
        settled(&mut app);
        let (guns, resting, on_player, hidden) = census(&mut app);
        println!("{guns} guns: {resting} resting, {on_player} on the player, {hidden} hidden");
        assert!(guns > 0);
        assert!(resting > 0, "no gun rests");
        assert_eq!(
            resting,
            guns - on_player - hidden,
            "every gun lying in the valley rests"
        );
        // Resting, a gun's own leaves are hidden and its merged meshes
        // stand in: fewer of them than the leaves.
        let w = app.world_mut();
        let rest: Vec<(Entity, usize, usize)> = w
            .query::<(Entity, &AtRest)>()
            .iter(w)
            .map(|(e, r)| (e, r.leaves.len(), r.merged.len()))
            .collect();
        for (gun, leaves, merged) in &rest {
            assert!(
                merged < leaves,
                "{gun:?}: {merged} merged for {leaves} leaves"
            );
        }
        let (took, merged): (usize, usize) =
            rest.iter().fold((0, 0), |a, r| (a.0 + r.1, a.1 + r.2));
        println!("resting guns: {took} leaf meshes drawn as {merged}");
    }

    fn a_resting_gun(app: &mut App) -> (Entity, Vec<Entity>) {
        let w = app.world_mut();
        let (gun, rest) = w
            .query::<(Entity, &AtRest)>()
            .iter(w)
            .next()
            .expect("a resting gun");
        (gun, rest.leaves.iter().map(|(e, _)| *e).collect())
    }

    fn awake(app: &mut App, gun: Entity, leaves: &[Entity], case: &str) {
        let w = app.world();
        assert!(w.get::<AtRest>(gun).is_none(), "{case}: still resting");
        for l in leaves {
            assert_ne!(
                w.get::<Visibility>(*l),
                Some(&Visibility::Hidden),
                "a leaf left hidden"
            );
        }
        assert_eq!(
            app.world_mut()
                .query_filtered::<&ChildOf, With<RestMesh>>()
                .iter(app.world())
                .filter(|c| c.parent() == gun)
                .count(),
            0,
            "merged meshes left behind"
        );
    }

    /// A hand taking a resting gun, a magazine pushed into one lying on a
    /// table (a hand holds the magazine, not the gun), and a part taken
    /// off one: each wakes it to its parts, as they were.
    #[test]
    fn a_resting_gun_wakes() {
        let _one_at_a_time = super::super::budget::tests::heavy();
        let mut app = super::super::budget::tests::client_threads("marrow_creek", "clearing", None);
        settled(&mut app);
        // Taken in hand.
        let (gun, leaves) = a_resting_gun(&mut app);
        let hand = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(gun)
            .insert(crate::interact::Held {
                player: hand,
                hand,
                side: crate::rig::Side::Right,
                point: 0,
                offset: Transform::IDENTITY,
            });
        app.update();
        awake(&mut app, gun, &leaves, "taken in hand");
        app.world_mut()
            .entity_mut(gun)
            .remove::<crate::interact::Held>();
        // Something pushed in.
        let (gun, leaves) = a_resting_gun(&mut app);
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(0.05, 0.2, 0.03));
        let mat = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let mag = app
            .world_mut()
            .spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::IDENTITY))
            .id();
        app.update();
        app.world_mut().entity_mut(mag).insert(ChildOf(gun));
        app.update();
        app.update();
        awake(&mut app, gun, &leaves, "pushed in");
        // A part given a material of its own (a gun weathering).
        let (gun, leaves) = a_resting_gun(&mut app);
        let own = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        app.world_mut()
            .entity_mut(leaves[0])
            .insert(MeshMaterial3d(own));
        app.update();
        awake(&mut app, gun, &leaves, "weathered");
        // Something taken off.
        let (gun, leaves) = a_resting_gun(&mut app);
        let off = leaves[0];
        app.world_mut().entity_mut(off).remove::<ChildOf>();
        app.update();
        app.update();
        awake(&mut app, gun, &leaves[1..], "taken off");
    }
}
