//! Breakable parts nobody is near are drawn merged, a cell at a time.
//!
//! A building's shutters, boards and crates, a wreck's panels, a window's
//! sash leaves and a counter's drawers are each drawn on their own
//! (`destruction::wood::Drawn`), so a crowbar can lever one off. Marrow
//! Creek's gate view drew about 250 of them an eye. Parts rarely change,
//! so once every part in a 16 m cell has stood unchanged for a while,
//! the cell's meshes are merged per material and stand in for them. The
//! parts keep their entities, colliders and everything else; only their
//! meshes are hidden.
//!
//! The moment anything could change a part, its whole cell is parts
//! again. Destruction's list of what that means:
//! - it moves, or anything under it does (a hit, a hinge, a press, a
//!   robot opening a door, the Surge);
//! - a mesh under it is swapped (a pane breaking and showing its teeth);
//! - anything under it comes or goes;
//! - it is hit (its timber's hp) or a crowbar's in its seam;
//! - it's gone (broken, stripped, the last wheel nut off);
//! - a hand, or anything held, comes within [`NEAR`].
//!
//! The cell waits until all of its parts are quiet again before it's
//! merged again. Window glass, and anything blended, isn't merged.
//!
//! `EXTRACTION_PART_REST=0` leaves every part on its own.

use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use bevy::tasks::Task;
use std::collections::{HashMap, HashSet};

use crate::destruction::wood::{Drawn, Timber};

pub struct PartRestPlugin;

impl Plugin for PartRestPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("EXTRACTION_PART_REST").is_ok_and(|v| v == "0") {
            return;
        }
        app.init_resource::<Cells>().add_systems(
            PostUpdate,
            (wake_parts, rest_cells, finish_cells)
                .chain()
                .after(TransformSystems::Propagate)
                .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
    }
}

/// A cell's side, metres.
const CELL: f32 = 16.0;
/// Frames every part in a cell stands unchanged before it's merged.
const SETTLE: u32 = 30;
/// A hand, or anything held, this close to a part (metres, from its
/// origin, past the part's own half-size) wakes it.
pub const NEAR: f32 = 1.5;
/// A cell of fewer parts than this isn't worth merging.
const MIN_PARTS: usize = 3;
/// Cells merged a frame, at most (a valley's parts all settle at once).
const PER_FRAME: usize = 4;

/// A merged stand-in mesh.
#[derive(Component)]
pub struct PartRestMesh;

/// What a merged mesh shares: its material, its range and how it casts
/// the sun's shadow.
#[derive(Clone, PartialEq)]
struct Shade {
    mat: Handle<StandardMaterial>,
    range: Option<(u32, u32)>,
    reach: Option<u32>,
    caster: bool,
}

#[derive(Default)]
struct Cell {
    /// Its parts, and how many frames each has been quiet.
    quiet: HashMap<Entity, u32>,
    rest: Option<Rest>,
    building: Option<Building>,
}

/// A merged cell: the leaves hidden (with the visibility each had) and
/// the merged meshes standing in.
struct Rest {
    leaves: Vec<(Entity, Visibility)>,
    merged: Vec<Entity>,
}

struct Building {
    task: Task<Vec<(Shade, Option<Mesh>)>>,
    leaves: Vec<(Entity, Visibility)>,
}

#[derive(Resource, Default)]
struct Cells {
    cells: HashMap<(i32, i32), Cell>,
    /// Each part's cell.
    part_cell: HashMap<Entity, (i32, i32)>,
    /// Each leaf (a mesh under a part, or the part's own) and its part:
    /// a change to one wakes the other.
    leaf_part: HashMap<Entity, Entity>,
}

fn cell_of(p: Vec3) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32)
}

impl Cells {
    /// Wakes `part`: its cell is parts again (and waits to settle).
    fn wake(&mut self, part: Entity, vis: &mut Query<&mut Visibility>, commands: &mut Commands) {
        let Some(key) = self.part_cell.get(&part).copied() else {
            return;
        };
        let Some(cell) = self.cells.get_mut(&key) else {
            return;
        };
        if let Some(q) = cell.quiet.get_mut(&part) {
            *q = 0;
        }
        cell.building = None;
        if let Some(rest) = cell.rest.take() {
            for (e, v) in rest.leaves {
                if let Ok(mut cur) = vis.get_mut(e) {
                    *cur = v;
                }
            }
            for m in rest.merged {
                commands.entity(m).try_despawn();
            }
        }
    }
}

/// Whatever could change a part wakes its cell.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn wake_parts(
    mut commands: Commands,
    mut cells: ResMut<Cells>,
    parts: Query<(Entity, &GlobalTransform, &Drawn)>,
    moved: Query<Entity, Changed<GlobalTransform>>,
    reshaped: Query<Entity, Changed<Mesh3d>>,
    regrown: Query<Entity, Changed<Children>>,
    mut gone: RemovedComponents<ChildOf>,
    mut undrawn: RemovedComponents<Drawn>,
    hit: Query<Entity, Changed<Timber>>,
    prised: Query<
        (Entity, &crate::destruction::hooks::Pryable),
        Changed<crate::destruction::hooks::Pryable>,
    >,
    near: Query<&GlobalTransform, Or<(With<crate::interact::Hand>, With<crate::interact::Held>)>>,
    mut vis: Query<&mut Visibility>,
) {
    let mut woken: HashSet<Entity> = HashSet::new();
    let part_of = |e: Entity, cells: &Cells| cells.leaf_part.get(&e).copied();
    for e in moved.iter().chain(reshaped.iter()).chain(regrown.iter()) {
        if let Some(p) = part_of(e, &cells) {
            woken.insert(p);
        }
    }
    for e in gone.read() {
        if let Some(p) = part_of(e, &cells) {
            woken.insert(p);
        }
    }
    woken.extend(hit.iter().filter(|e| cells.part_cell.contains_key(e)));
    woken.extend(
        prised
            .iter()
            .filter(|(e, p)| p.held > 0.0 && cells.part_cell.contains_key(e))
            .map(|(e, _)| e),
    );
    // Gone: broken, stripped, despawned.
    let undrawn: Vec<Entity> = undrawn.read().collect();
    for e in &undrawn {
        woken.insert(*e);
    }
    // Hands and held things: the parts in the cells round each.
    for g in &near {
        let at = g.translation();
        let (cx, cz) = cell_of(at);
        for dx in -1..=1 {
            for dz in -1..=1 {
                let Some(cell) = cells.cells.get(&(cx + dx, cz + dz)) else {
                    continue;
                };
                for part in cell.quiet.keys() {
                    if let Ok((_, pg, d)) = parts.get(*part) {
                        let reach = NEAR + d.size.max_element() * 0.5;
                        if pg.translation().distance_squared(at) < reach * reach {
                            woken.insert(*part);
                        }
                    }
                }
            }
        }
    }
    for part in woken {
        cells.wake(part, &mut vis, &mut commands);
    }
    // Parts gone for good leave their cells.
    for e in undrawn {
        if let Some(key) = cells.part_cell.remove(&e) {
            if let Some(cell) = cells.cells.get_mut(&key) {
                cell.quiet.remove(&e);
            }
        }
        cells.leaf_part.retain(|_, p| *p != e);
    }
}

/// Every mesh under a part (the part's own too) drawn with a standard
/// material, not glass, not blended.
fn leaves_of(
    part: Entity,
    children: &Query<&Children>,
    meshes: &Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>), Without<PartRestMesh>>,
    glass: &Query<(), With<crate::destruction::visuals::WindowGlass>>,
) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack = vec![part];
    while let Some(e) = stack.pop() {
        if glass.contains(e) {
            continue;
        }
        if meshes.contains(e) {
            out.push(e);
        }
        if let Ok(kids) = children.get(e) {
            stack.extend(kids.iter());
        }
    }
    out
}

/// Parts join their cells; quiet cells are merged (off the main thread).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn rest_cells(
    mut cells: ResMut<Cells>,
    parts: Query<(Entity, &GlobalTransform), With<Drawn>>,
    children: Query<&Children>,
    meshes: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>), Without<PartRestMesh>>,
    glass: Query<(), With<crate::destruction::visuals::WindowGlass>>,
    materials: Res<Assets<StandardMaterial>>,
    mesh_assets: Res<Assets<Mesh>>,
    leaf_info: Query<(
        &GlobalTransform,
        &Visibility,
        &InheritedVisibility,
        Option<&VisibilityRange>,
        Option<&super::budget::ShadowReach>,
        Has<bevy::light::NotShadowCaster>,
    )>,
) {
    let cells = &mut *cells;
    // New parts join (a valley's, a mended Surge's), quiet from now.
    for (e, g) in &parts {
        if cells.part_cell.contains_key(&e) {
            continue;
        }
        let key = cell_of(g.translation());
        cells.part_cell.insert(e, key);
        cells.leaf_part.insert(e, e);
        cells.cells.entry(key).or_default().quiet.insert(e, 0);
    }
    let mut started = 0;
    let keys: Vec<(i32, i32)> = cells.cells.keys().copied().collect();
    for key in keys {
        let cell = cells.cells.get_mut(&key).unwrap();
        for q in cell.quiet.values_mut() {
            *q = q.saturating_add(1);
        }
        if cell.rest.is_some()
            || cell.building.is_some()
            || cell.quiet.len() < MIN_PARTS
            || cell.quiet.values().any(|q| *q < SETTLE)
            || started >= PER_FRAME
        {
            continue;
        }
        // Every shown leaf, by what it shares, in the world.
        let mut groups: Vec<(Shade, Vec<(Mesh, Mat4)>)> = Vec::new();
        let mut taken: Vec<(Entity, Visibility)> = Vec::new();
        let mut owners: Vec<(Entity, Entity)> = Vec::new();
        for part in cell.quiet.keys() {
            for leaf in leaves_of(*part, &children, &meshes, &glass) {
                owners.push((leaf, *part));
                let Ok((mesh, mat)) = meshes.get(leaf) else {
                    continue;
                };
                let Ok((gt, v, shown, range, reach, no_shadow)) = leaf_info.get(leaf) else {
                    continue;
                };
                if !shown.get() {
                    continue;
                }
                if materials.get(&mat.0).is_none_or(|m| {
                    matches!(
                        m.alpha_mode,
                        AlphaMode::Blend
                            | AlphaMode::Premultiplied
                            | AlphaMode::Add
                            | AlphaMode::Multiply
                    )
                }) {
                    continue;
                }
                let Some(m) = mesh_assets.get(&mesh.0) else {
                    continue;
                };
                if !super::gun_rest::mergeable(m) {
                    continue;
                }
                let shade = Shade {
                    mat: mat.0.clone(),
                    range: range
                        .map(|r| (r.end_margin.start.to_bits(), r.end_margin.end.to_bits())),
                    reach: reach.map(|r| r.0.to_bits()),
                    caster: reach.is_some() || !no_shadow,
                };
                let world = Mat4::from(gt.affine());
                match groups.iter_mut().find(|(k, _)| *k == shade) {
                    Some((_, l)) => l.push((m.clone(), world)),
                    None => groups.push((shade, vec![(m.clone(), world)])),
                }
                taken.push((leaf, *v));
            }
        }
        for (leaf, part) in owners {
            cells.leaf_part.insert(leaf, part);
        }
        // Not worth it for a cell that's a few draws already.
        if taken.len() <= groups.len() + 1 {
            continue;
        }
        started += 1;
        let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move {
            groups
                .into_iter()
                .map(|(shade, srcs)| {
                    let mut out: Option<Mesh> = None;
                    for (src, m) in srcs {
                        let Some(part) = super::world_props::for_merge(&src) else {
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
                    (shade, out)
                })
                .collect()
        });
        cell.building = Some(Building {
            task,
            leaves: taken,
        });
    }
}

/// A cell's merged meshes done: they stand in, its leaves hide (unless
/// something woke it meanwhile, which dropped the build).
fn finish_cells(
    mut commands: Commands,
    mut cells: ResMut<Cells>,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    mut vis: Query<&mut Visibility>,
) {
    use bevy::tasks::{block_on, futures_lite::future};
    for cell in cells.cells.values_mut() {
        let Some(b) = cell.building.as_mut() else {
            continue;
        };
        let Some(built) = block_on(future::poll_once(&mut b.task)) else {
            continue;
        };
        let b = cell.building.take().unwrap();
        let mut merged = Vec::new();
        for (shade, mesh) in built {
            let Some(mesh) = mesh else {
                continue;
            };
            let mut e = commands.spawn((
                Name::new("Parts at rest"),
                PartRestMesh,
                super::centred(&mut mesh_assets, mesh),
                MeshMaterial3d(shade.mat.clone()),
            ));
            if let Some((a, b)) = shade.range {
                e.insert(VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: f32::from_bits(a)..f32::from_bits(b),
                    use_aabb: true,
                });
            }
            match shade.reach {
                Some(bits) => {
                    e.insert(super::budget::ShadowReach(f32::from_bits(bits)));
                }
                None if !shade.caster => {
                    e.insert(bevy::light::NotShadowCaster);
                }
                None => {}
            }
            merged.push(e.id());
        }
        for (leaf, _) in &b.leaves {
            if let Ok(mut v) = vis.get_mut(*leaf) {
                *v = Visibility::Hidden;
            }
        }
        debug!(
            "parts at rest: {} meshes drawn as {}",
            b.leaves.len(),
            merged.len()
        );
        cell.rest = Some(Rest {
            leaves: b.leaves,
            merged,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::destruction::hooks::Pryable;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            TransformPlugin,
            bevy::camera::visibility::VisibilityPlugin,
            PartRestPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        app
    }

    /// The library's planks: parts share their materials.
    #[derive(Resource)]
    struct Planks(Handle<StandardMaterial>);

    /// A part as `dress_crates` leaves it: a mesh under it.
    fn part(app: &mut App, at: Vec3) -> (Entity, Entity) {
        let w = app.world_mut();
        let mesh = w
            .resource_mut::<Assets<Mesh>>()
            .add(crate::render::materials::metric_box(Vec3::new(
                0.6, 1.2, 0.05,
            )));
        if !w.contains_resource::<Planks>() {
            let mat = w
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial::default());
            w.insert_resource(Planks(mat));
        }
        let mat = w.resource::<Planks>().0.clone();
        let p = w
            .spawn((
                Drawn {
                    model: None,
                    mat: crate::render::materials::Mat::Planks,
                    at: Transform::IDENTITY,
                    size: Vec3::new(0.6, 1.2, 0.05),
                    range: 50.0..60.0,
                    shadow: false,
                },
                Timber {
                    hp: 10.0,
                    max: 10.0,
                    key: [0; 3],
                    push: Vec3::ZERO,
                },
                Pryable {
                    needs: 1.0,
                    seam: Vec3::ZERO,
                    along: Vec3::Y,
                    held: 0.0,
                },
                Transform::from_translation(at),
                Visibility::default(),
            ))
            .id();
        let leaf = w
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::IDENTITY,
                ChildOf(p),
            ))
            .id();
        (p, leaf)
    }

    fn resting(app: &mut App) -> usize {
        let w = app.world_mut();
        w.query_filtered::<(), With<PartRestMesh>>().iter(w).count()
    }

    fn shown(app: &App, leaf: Entity) -> bool {
        app.world().get::<Visibility>(leaf) != Some(&Visibility::Hidden)
    }

    /// Four shutters along a wall, settled: merged, the parts hidden.
    fn settled() -> (App, Vec<(Entity, Entity)>) {
        let mut app = app();
        let parts: Vec<_> = (0..4)
            .map(|i| part(&mut app, Vec3::new(1.0 + i as f32 * 2.0, 1.0, 2.0)))
            .collect();
        for _ in 0..(SETTLE + 20) {
            app.update();
        }
        assert_eq!(resting(&mut app), 1, "one material: one merged mesh");
        for (_, leaf) in &parts {
            assert!(!shown(&app, *leaf), "a merged part's own mesh is hidden");
        }
        (app, parts)
    }

    /// Whatever wakes a part shows its cell's parts on their own that
    /// frame, with nothing merged left.
    fn wakes(case: &str, act: impl FnOnce(&mut App, &[(Entity, Entity)])) {
        let (mut app, parts) = settled();
        act(&mut app, &parts);
        app.update();
        assert_eq!(resting(&mut app), 0, "{case}: merged meshes left");
        for (p, leaf) in &parts {
            if app.world().get_entity(*p).is_ok() {
                assert!(shown(&app, *leaf), "{case}: a part left hidden");
            }
        }
    }

    #[test]
    fn a_hand_near_a_part_wakes_its_cell() {
        wakes("a hand", |app, parts| {
            let at = app
                .world()
                .get::<Transform>(parts[0].0)
                .unwrap()
                .translation;
            app.world_mut().spawn((
                crate::interact::Hand::default(),
                Transform::from_translation(at + Vec3::Z * 0.8),
            ));
        });
    }

    #[test]
    fn a_crowbar_in_the_seam_wakes_its_cell() {
        wakes("a crowbar", |app, parts| {
            app.world_mut().get_mut::<Pryable>(parts[1].0).unwrap().held = 0.1;
        });
    }

    #[test]
    fn a_hit_wakes_its_cell() {
        wakes("a hit", |app, parts| {
            app.world_mut().get_mut::<Timber>(parts[2].0).unwrap().hp -= 3.0;
        });
    }

    /// A window leaf swinging (a hand, a press, a robot at a door: the
    /// part moves).
    #[test]
    fn a_swinging_leaf_wakes_its_cell() {
        wakes("a swing", |app, parts| {
            app.world_mut()
                .get_mut::<Transform>(parts[3].0)
                .unwrap()
                .rotate_y(0.3);
        });
    }

    /// A pane breaking under a resting leaf: its mesh is swapped.
    #[test]
    fn a_mesh_swapped_under_a_part_wakes_its_cell() {
        wakes("a pane", |app, parts| {
            let w = app.world_mut();
            let mesh = w
                .resource_mut::<Assets<Mesh>>()
                .add(crate::render::materials::metric_box(Vec3::splat(0.3)));
            w.entity_mut(parts[0].1).insert(Mesh3d(mesh));
        });
    }

    /// The last nut off takes its wheel: no merged ghost of it left, and
    /// the cell merges again without it.
    #[test]
    fn a_part_gone_leaves_no_ghost() {
        wakes("gone", |app, parts| {
            app.world_mut().entity_mut(parts[0].0).despawn();
        });
        let (mut app, parts) = settled();
        app.world_mut().entity_mut(parts[0].0).despawn();
        for _ in 0..(SETTLE + 20) {
            app.update();
        }
        assert_eq!(resting(&mut app), 1, "the rest merged again");
    }

    /// A part drawn with a mesh whose data went to the GPU only can't be
    /// merged: it's left as it is, the rest of its cell merged.
    #[test]
    fn a_mesh_on_the_gpu_only_is_left_be() {
        let mut app = app();
        let parts: Vec<_> = (0..4)
            .map(|i| part(&mut app, Vec3::new(1.0 + i as f32 * 2.0, 1.0, 2.0)))
            .collect();
        {
            let w = app.world_mut();
            let mut gone = crate::render::materials::metric_box(Vec3::splat(0.4));
            let _ = gone.take_gpu_data();
            let mesh = w.resource_mut::<Assets<Mesh>>().add(gone);
            w.entity_mut(parts[0].1).insert(Mesh3d(mesh));
        }
        for _ in 0..(SETTLE + 20) {
            app.update();
        }
        assert_eq!(resting(&mut app), 1, "the other three merged");
        assert!(shown(&app, parts[0].1), "the GPU-only one drawn as it is");
    }

    /// Woken and left alone again, a cell merges again.
    #[test]
    fn a_woken_cell_rests_again() {
        let (mut app, parts) = settled();
        app.world_mut().get_mut::<Timber>(parts[0].0).unwrap().hp -= 1.0;
        app.update();
        assert_eq!(resting(&mut app), 0);
        for _ in 0..(SETTLE + 20) {
            app.update();
        }
        assert_eq!(resting(&mut app), 1);
    }
}
