//! What a frame asks of the GPU, counted: `EXTRACTION_RENDER_STATS=1`
//! prints, once the scene has settled, how many meshes the camera draws,
//! their triangles, how many distinct meshes that is (instancing batches
//! the repeats), and what the sun's shadow cascades draw for it (see
//! `budget::ViewLoad`). The screenshots
//! render in software, so this is the measure of a view's load that can be
//! taken without a GPU (see `docs/PLAYTEST.md` for the in-headset one).

use bevy::camera::visibility::VisibleEntities;
use bevy::prelude::*;

use super::budget::LoadCounter;

pub struct RenderStatsPlugin;

impl Plugin for RenderStatsPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("EXTRACTION_RENDER_STATS").is_some() {
            app.add_systems(Last, report.after(super::budget::count_triangles));
        }
    }
}

fn report(
    mut frame: Local<u32>,
    cameras: Query<
        (Entity, &GlobalTransform, &VisibleEntities),
        (With<Camera3d>, Without<super::OffscreenCamera>),
    >,
    counter: LoadCounter,
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
    // the screens' 2D cameras included), and its own shadow cascades.
    let Some((cam, eye, seen)) = cameras.iter().next() else {
        return;
    };
    let eye = eye.translation();
    let load = counter.measure(cam, seen);
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
    println!("render stats at {eye:.0}: {}", load.summary());
    for (title, by) in [("drawn", &load.by), ("shadows", &load.shadow_by)] {
        for (what, count, tris) in by.iter().take(6) {
            println!(
                "  {title}: {what}: {count} meshes, {:.0}k triangles",
                *tris as f64 / 1000.0
            );
        }
    }
}
