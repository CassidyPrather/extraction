//! Every headset-style eye has an eye's projection from its first frame.
//!
//! `bevy_mod_xr`'s `XrProjection` starts as the identity matrix and the
//! backend fills it only once the runtime has located the views (a session
//! can sit for a minute between made and running). Bevy takes an identity
//! matrix for an orthographic view a unit deep: its lights were clustered
//! from a first slice at -1 (`Clusters::near`), the GPU clustering's
//! slices spanning all of every light, seconds a frame on a software GPU,
//! and its frustum culled for a box that isn't the eye's. The proxy eye
//! (`warmup::ProxyEye`) had it all the time it stood in.

use bevy::camera::{CameraProjection, CameraUpdateSystems};
use bevy::prelude::*;
use bevy_mod_xr::camera::{Fov, XrProjection, calculate_projection};

pub struct EyeProjectionPlugin;

impl Plugin for EyeProjectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, fill_projections.before(CameraUpdateSystems));
    }
}

/// A Beyond 2 eye's, square and symmetric (`legibility::EYE_TAN_SPAN`):
/// what the runtime will give, near enough to cull and cluster with.
pub fn headset_matrix(near: f32) -> Mat4 {
    let half = (super::legibility::EYE_TAN_SPAN / 2.0).atan();
    calculate_projection(
        near,
        Fov {
            angle_left: -half,
            angle_right: half,
            angle_down: -half,
            angle_up: half,
        },
    )
}

/// Whether a projection is a perspective one (Bevy's own test, for
/// clustering and culling: `w_axis.w` 1 is orthographic).
pub fn is_perspective(clip_from_view: Mat4) -> bool {
    clip_from_view.w_axis.w == 0.0 && clip_from_view.z_axis.w != 0.0
}

/// Any `XrProjection` not yet a perspective one gets the headset's, at its
/// own near plane; the backend's views replace it once located.
fn fill_projections(mut cameras: Query<&mut Projection>) {
    for mut projection in &mut cameras {
        let Projection::Custom(custom) = &*projection else {
            continue;
        };
        let Some(p) = custom.get::<XrProjection>() else {
            continue;
        };
        if is_perspective(p.get_clip_from_view()) {
            continue;
        }
        let near = p.near;
        if let Projection::Custom(custom) = &mut *projection {
            if let Some(p) = custom.get_mut::<XrProjection>() {
                p.projection_matrix = headset_matrix(near);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An eye as the backend spawns it, and the proxy eye as the warm-up
    /// does: both are perspective by the time cameras are updated, with a
    /// first cluster slice in front of the eye.
    #[test]
    fn eyes_are_perspective_before_their_views_are_located() {
        let mut app = App::new();
        app.add_plugins(EyeProjectionPlugin);
        let eye = app
            .world_mut()
            .spawn(Projection::custom(XrProjection::default()))
            .id();
        let proxy = app
            .world_mut()
            .spawn(Projection::custom(XrProjection {
                near: super::super::NEAR_CLIP,
                ..default()
            }))
            .id();
        app.update();
        for e in [eye, proxy] {
            let projection = app.world().get::<Projection>(e).unwrap();
            let m = projection.get_clip_from_view();
            assert!(is_perspective(m), "{e:?}: {m:?}");
            // Something a metre ahead is inside the frustum.
            let clip = m * Vec4::new(0.0, 0.0, -1.0, 1.0);
            assert!(clip.w > 0.0 && (clip.x / clip.w).abs() < 1.0);
        }
    }

    /// The backend's own matrix, once it has one, is left alone.
    #[test]
    fn a_located_view_is_kept() {
        let mut app = App::new();
        app.add_plugins(EyeProjectionPlugin);
        let located = calculate_projection(
            0.05,
            Fov {
                angle_left: -0.9,
                angle_right: 0.7,
                angle_down: -0.8,
                angle_up: 0.75,
            },
        );
        let e = app
            .world_mut()
            .spawn(Projection::custom(XrProjection {
                near: 0.05,
                projection_matrix: located,
            }))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .get::<Projection>(e)
                .unwrap()
                .get_clip_from_view(),
            located
        );
    }
}
