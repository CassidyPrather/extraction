//! What a frame asks of the GPU, counted: `EXTRACTION_RENDER_STATS=1`
//! prints, once the scene has settled, how many meshes the camera draws,
//! their triangles, how many distinct meshes that is (instancing batches
//! the repeats), and what the sun's shadow pass draws. The screenshots
//! render in software, so this is the measure of a view's load that can be
//! taken without a GPU (see `docs/PLAYTEST.md` for the in-headset one).

use bevy::camera::visibility::ViewVisibility;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub struct RenderStatsPlugin;

impl Plugin for RenderStatsPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("EXTRACTION_RENDER_STATS").is_some() {
            app.init_resource::<Triangles>()
                .add_systems(Last, (count_triangles, report).chain());
        }
    }
}

/// Triangles per mesh, counted as each arrives (once a mesh is handed to
/// the render world its data is gone from this one).
#[derive(Resource, Default)]
struct Triangles(HashMap<AssetId<Mesh>, usize>);

fn count_triangles(
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Res<Assets<Mesh>>,
    mut tris: ResMut<Triangles>,
) {
    for e in events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = e {
            if let Some(mesh) = meshes.get(*id) {
                let n = match mesh.try_indices_option() {
                    Ok(Some(i)) => i.len() / 3,
                    Ok(None) => mesh.count_vertices() / 3,
                    Err(_) => continue,
                };
                tris.0.insert(*id, n);
            }
        }
    }
}

/// The sun's shadows reach this far (see `world::sky`).
const SHADOW_REACH: f32 = 90.0;

fn report(
    mut frame: Local<u32>,
    cameras: Query<
        (&GlobalTransform, &bevy::camera::visibility::VisibleEntities),
        (With<Camera3d>, Without<super::OffscreenCamera>),
    >,
    drawn: Query<(
        Entity,
        &Mesh3d,
        &ViewVisibility,
        &GlobalTransform,
        Has<NotShadowCaster>,
        Option<&bevy::camera::primitives::Aabb>,
    )>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    tri_counts: Res<Triangles>,
    points: Query<&PointLight>,
    spots: Query<&SpotLight>,
    ranged: Query<
        (&GlobalTransform, Option<&bevy::camera::primitives::Aabb>),
        (
            With<Mesh3d>,
            With<bevy::camera::visibility::VisibilityRange>,
        ),
    >,
) {
    *frame += 1;
    if *frame != 30 {
        return;
    }
    // What this camera itself draws (`ViewVisibility` is any camera's:
    // the screens' 2D cameras included).
    let Some((eye, seen)) = cameras.iter().next().map(|(g, v)| {
        let seen: HashSet<Entity> = v
            .get(std::any::TypeId::of::<Mesh3d>())
            .iter()
            .copied()
            .collect();
        (g.translation(), seen)
    }) else {
        return;
    };
    let tris = |m: &Mesh3d| tri_counts.0.get(&m.0.id()).copied().unwrap_or(0);
    let (mut n, mut t, mut kinds) = (0usize, 0usize, HashSet::new());
    let (mut sn, mut st) = (0usize, 0usize);
    // What the triangles are: by the nearest named ancestor.
    let mut by: HashMap<String, (usize, usize)> = HashMap::new();
    let mut shadow_by: HashMap<String, (usize, usize)> = HashMap::new();
    let label = |mut e: Entity| {
        for _ in 0..6 {
            if let Ok(n) = names.get(e) {
                return n.as_str().split(' ').next().unwrap_or("").to_string();
            }
            match parents.get(e) {
                Ok(p) => e = p.parent(),
                Err(_) => break,
            }
        }
        "(unnamed)".to_string()
    };
    for (e, m, _vis, gt, no_shadow, aabb) in &drawn {
        // Merged meshes sit at the origin: measure to their bounds.
        let near = aabb.map_or(gt.translation().distance(eye), |b| {
            let c = gt.transform_point(b.center.into());
            (c.distance(eye) - Vec3::from(b.half_extents).length()).max(0.0)
        });
        if seen.contains(&e) {
            n += 1;
            t += tris(m);
            kinds.insert(m.0.id());
            let entry = by.entry(label(e)).or_default();
            entry.0 += 1;
            entry.1 += tris(m);
        }
        if !no_shadow && near < SHADOW_REACH {
            sn += 1;
            st += tris(m);
            let entry = shadow_by.entry(label(e)).or_default();
            entry.0 += 1;
            entry.1 += tris(m);
        }
    }
    // The GPU's LOD dither measures from a mesh's origin, not its bounds:
    // a ranged mesh left at the world origin fades by the camera's distance
    // from the map centre (see `render::centred`).
    let at_origin = ranged
        .iter()
        .filter(|(g, _)| g.translation().length() < 1e-3)
        .count();
    let unbounded = ranged.iter().filter(|(_, b)| b.is_none()).count();
    if at_origin + unbounded > 0 {
        println!(
            "render stats WARNING: {at_origin} LOD-ranged meshes at the world origin, \
             {unbounded} with no bounds"
        );
    }
    let shadowed_points = points.iter().filter(|p| p.shadow_maps_enabled).count();
    let shadowed_spots = spots.iter().filter(|p| p.shadow_maps_enabled).count();
    println!(
        "render stats at {eye:.0}: {n} meshes drawn ({} distinct), {:.0}k triangles; \
         sun shadows {sn} meshes, {:.0}k triangles; shadowed lights: {shadowed_points} point, \
         {shadowed_spots} spot",
        kinds.len(),
        t as f64 / 1000.0,
        st as f64 / 1000.0,
    );
    for (title, by) in [("drawn", by), ("shadows", shadow_by)] {
        let mut by: Vec<_> = by.into_iter().collect();
        by.sort_by(|a, b| b.1.1.cmp(&a.1.1));
        for (what, (count, tris)) in by.iter().take(6) {
            println!(
                "  {title}: {what}: {count} meshes, {:.0}k triangles",
                *tris as f64 / 1000.0
            );
        }
    }
}
