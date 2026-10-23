//! `--check-buildings 1`: once everything's in, every building whose middle
//! is in front of the camera and within [`REACH`] must draw at least one
//! of its meshes. A capture that shoots a valley with a building missing
//! fails loudly instead of passing as a picture of an empty field. (Cinder
//! Works' whole blast-furnace group once drew nothing at dusk, its lamp
//! hanging in the sky over where it should have stood.)
//!
//! "Draws" is Bevy's `ViewVisibility`: visible, inside the view frustum and
//! inside its `VisibilityRange`. So a hero landmark's near model and its
//! lighter stand-ins (`buildings::dress_landmark`) count, whichever is in
//! range.

use bevy::camera::visibility::{InheritedVisibility, ViewVisibility};
use bevy::prelude::*;

use crate::render::timeline::Loaded;
use crate::world::buildings::Building;

/// How far off a building in view must draw, metres.
pub const REACH: f32 = 150.0;
/// Frames after everything's in before checking (visibility settles a
/// frame or two after spawns).
const AFTER: u32 = 10;

/// A building's meshes, counted three ways.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Drawn {
    pub meshes: usize,
    pub visible: usize,
    pub in_view: usize,
    /// Meshes with a position, normal or colour that isn't a finite number:
    /// drawn, and still not seen (lavapipe drops them).
    pub broken: usize,
    /// Meshes whose data is in the render world only, so not checked.
    pub unread: usize,
}

/// The check's verdict on one building: in view, near enough, and drawing
/// nothing (or drawing broken meshes) is a failure.
pub fn missing(d: Drawn) -> bool {
    d.in_view == 0 || d.broken > 0
}

/// Whether every position, normal and colour of a mesh is a finite number;
/// None for a mesh whose data has gone to the render world only.
pub fn finite(mesh: &Mesh) -> Option<bool> {
    use bevy::mesh::VertexAttributeValues as V;
    let mut ok = true;
    for a in [
        Mesh::ATTRIBUTE_POSITION,
        Mesh::ATTRIBUTE_NORMAL,
        Mesh::ATTRIBUTE_COLOR,
    ] {
        ok &= match mesh.try_attribute_option(a).ok()? {
            Some(V::Float32x3(v)) => v.iter().flatten().all(|x| x.is_finite()),
            Some(V::Float32x4(v)) => v.iter().flatten().all(|x| x.is_finite()),
            _ => true,
        };
    }
    Some(ok)
}

#[allow(clippy::type_complexity)]
pub fn check_buildings(
    opts: Res<crate::launch::LaunchOptions>,
    loaded: Option<Res<Loaded>>,
    cams: Query<(&Camera, &GlobalTransform), With<crate::render::FreeCamera>>,
    buildings: Query<(Entity, &Building, &GlobalTransform)>,
    children: Query<&Children>,
    meshes: Query<(&InheritedVisibility, &ViewVisibility, &Mesh3d)>,
    assets: Res<Assets<Mesh>>,
    mut since: Local<u32>,
    mut done: Local<bool>,
) {
    if !opts.capture.check_buildings || *done || loaded.is_none() {
        return;
    }
    *since += 1;
    if *since < AFTER {
        return;
    }
    let Ok((cam, cam_at)) = cams.single() else {
        return;
    };
    *done = true;
    let eye = cam_at.translation();
    let mut bad = 0;
    for (e, b, at) in &buildings {
        let p = at.translation();
        let d = p.distance(eye);
        if d > REACH {
            continue;
        }
        // In front and inside the frame (its footing or 5 m up).
        let seen = [p, p + Vec3::Y * 5.0].iter().any(|q| {
            cam.world_to_ndc(cam_at, *q)
                .is_some_and(|n| n.z > 0.0 && n.x.abs() <= 1.0 && n.y.abs() <= 1.0)
        });
        if !seen {
            continue;
        }
        let mut count = Drawn::default();
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            if let Ok((inherited, view, mesh)) = meshes.get(x) {
                count.meshes += 1;
                count.visible += inherited.get() as usize;
                count.in_view += view.get() as usize;
                match assets.get(&mesh.0).map(finite) {
                    Some(Some(false)) => count.broken += 1,
                    Some(None) | None => count.unread += 1,
                    _ => {}
                }
            }
            if let Ok(c) = children.get(x) {
                stack.extend(c.iter());
            }
        }
        if missing(count) {
            bad += 1;
            error!(
                "check-buildings: {} at ({:.0}, {:.0}), {d:.0} m off and in view, draws nothing: \
                 {} meshes, {} visible, {} in view, {} with non-finite vertices ({} not readable)",
                b.prefab,
                p.x,
                p.z,
                count.meshes,
                count.visible,
                count.in_view,
                count.broken,
                count.unread
            );
        } else {
            info!(
                "check-buildings: {} at {d:.0} m: {} of {} meshes drawn ({} not readable)",
                b.prefab, count.in_view, count.meshes, count.unread
            );
        }
    }
    if bad == 0 {
        info!("check-buildings: every building in view draws");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_building_in_view_drawing_nothing_is_missing() {
        let none = Drawn {
            meshes: 40,
            visible: 40,
            in_view: 0,
            broken: 0,
            unread: 0,
        };
        assert!(missing(none));
        assert!(missing(Drawn::default()));
        let fine = Drawn {
            meshes: 40,
            visible: 40,
            in_view: 3,
            broken: 0,
            unread: 0,
        };
        assert!(!missing(fine));
        assert!(missing(Drawn { broken: 1, ..fine }));
    }

    #[test]
    fn a_nan_vertex_is_broken() {
        let mut m = Mesh::from(Cuboid::default());
        assert_eq!(finite(&m), Some(true));
        let n = m.count_vertices();
        let mut colours = vec![[1.0f32, 1.0, 1.0, 1.0]; n];
        colours[3][0] = f32::NAN;
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        assert_eq!(finite(&m), Some(false));
    }
}
