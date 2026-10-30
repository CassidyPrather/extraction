//! A blink over the eyes when the comfort setting moves you at once
//! (`interact::hand_climb::ClimbedOver` with `teleport`): the view goes
//! black and opens again on where you are, rather than jumping.
//!
//! The eyes also close while the head is inside something solid (leant
//! into a wall, a bench's top, a crate): the room never moves to keep the
//! head out (`player`), so what's inside isn't seen instead.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::interact::hand_climb::ClimbedOver;
use crate::rig::{LocalPlayer, RigHead};

/// How long the view takes to open again, seconds.
const OPEN: f32 = 0.4;
/// How long it takes to close as the head goes into something, seconds.
const CLOSE: f32 = 0.12;
/// The head's size against solid things (m).
const HEAD_R: f32 = 0.09;

pub struct BlinkPlugin;

impl Plugin for BlinkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (add_blink, blink).chain());
    }
}

#[derive(Component)]
struct Blink {
    material: Handle<StandardMaterial>,
    /// 1 shut, 0 open.
    shut: f32,
}

fn add_blink(
    mut commands: Commands,
    heads: Query<(Entity, &ChildOf), Added<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (head, parent) in &heads {
        if !locals.contains(parent.parent()) {
            continue;
        }
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            fog_enabled: false,
            ..default()
        });
        // Just inside the vignette (at 0.07), and wider than any view.
        commands.entity(head).with_child((
            Blink {
                material: material.clone(),
                shut: 0.0,
            },
            Mesh3d(meshes.add(Rectangle::new(0.6, 0.6))),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, 0.0, -0.065),
            NotShadowCaster,
            Visibility::Hidden,
        ));
    }
}

/// Shut at once on a comfort move of the local player's, then open.
fn blink(
    mut blinks: Query<(&mut Blink, &mut Visibility, &ChildOf)>,
    heads: Query<&GlobalTransform, With<RigHead>>,
    mut over: MessageReader<ClimbedOver>,
    locals: Query<(), With<LocalPlayer>>,
    spatial: avian3d::prelude::SpatialQuery,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    let moved = over.read().any(|o| o.teleport && locals.contains(o.player));
    let dt = time.delta_secs();
    for (mut b, mut vis, head) in &mut blinks {
        let inside = heads
            .get(head.parent())
            .is_ok_and(|h| head_in_solid(&spatial, h.translation()));
        // Open and staying open: nothing to write (a material written
        // re-prepares every frame).
        if !moved && !inside && b.shut <= 0.0 {
            continue;
        }
        b.shut = if moved {
            1.0
        } else if inside {
            (b.shut + dt / CLOSE).min(1.0)
        } else {
            (b.shut - dt / OPEN).max(0.0)
        };
        vis.set_if_neq(if b.shut > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if let Some(mut m) = materials.get_mut(&b.material) {
            m.base_color = Color::srgba(0.0, 0.0, 0.0, b.shut);
        }
    }
}

/// Whether a head at `at` is inside the world's solid things.
pub fn head_in_solid(spatial: &avian3d::prelude::SpatialQuery, at: Vec3) -> bool {
    use avian3d::prelude::*;
    let filter = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
    !spatial
        .shape_intersections(&Collider::sphere(HEAD_R), at, Quat::IDENTITY, &filter)
        .is_empty()
}
