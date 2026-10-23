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
}

/// The check's verdict on one building: in view, near enough, and drawing
/// nothing is a failure.
pub fn missing(d: Drawn) -> bool {
    d.in_view == 0
}

#[allow(clippy::type_complexity)]
pub fn check_buildings(
    opts: Res<crate::launch::LaunchOptions>,
    loaded: Option<Res<Loaded>>,
    cams: Query<(&Camera, &GlobalTransform), With<crate::render::FreeCamera>>,
    buildings: Query<(Entity, &Building, &GlobalTransform)>,
    children: Query<&Children>,
    meshes: Query<(&InheritedVisibility, &ViewVisibility), With<Mesh3d>>,
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
            if let Ok((inherited, view)) = meshes.get(x) {
                count.meshes += 1;
                count.visible += inherited.get() as usize;
                count.in_view += view.get() as usize;
            }
            if let Ok(c) = children.get(x) {
                stack.extend(c.iter());
            }
        }
        if missing(count) {
            bad += 1;
            error!(
                "check-buildings: {} at ({:.0}, {:.0}), {d:.0} m off and in view, draws nothing: \
                 {} meshes, {} visible, {} in view",
                b.prefab, p.x, p.z, count.meshes, count.visible, count.in_view
            );
        } else {
            info!(
                "check-buildings: {} at {d:.0} m: {} of {} meshes drawn",
                b.prefab, count.in_view, count.meshes
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
        };
        assert!(missing(none));
        assert!(missing(Drawn::default()));
        assert!(!missing(Drawn {
            meshes: 40,
            visible: 40,
            in_view: 3,
        }));
    }
}
