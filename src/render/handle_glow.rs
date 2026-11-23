//! A gun clicked on an empty chamber with rounds in its feed
//! (`weapons::unchambered::NeedsWorking`): its action's handle (bolt,
//! slide, charging handle, forend) glows the glove's hover green, breathing
//! a little, till it's worked. An affordance answering a mistake ("this is
//! what to work"), not a warning, and never lit otherwise.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::interact::Grabbable;
use crate::weapons::unchambered::{NeedsWorking, handle_point};

pub struct HandleGlowPlugin;

impl Plugin for HandleGlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_glows);
    }
}

/// The glow's size (m across) and how it breathes (share, Hz).
pub const GLOW_SIZE: f32 = 0.035;
pub const GLOW_BREATH: (f32, f32) = (0.15, 1.5);

/// A glow, on its gun.
#[derive(Component)]
pub struct HandleGlow;

#[allow(clippy::type_complexity)]
fn handle_glows(
    mut commands: Commands,
    marked: Query<(Entity, &Grabbable), With<NeedsWorking>>,
    mut glows: Query<(Entity, &ChildOf, &mut Transform), With<HandleGlow>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    let breath =
        1.0 + GLOW_BREATH.0 * (time.elapsed_secs() * GLOW_BREATH.1 * std::f32::consts::TAU).sin();
    let mut have = std::collections::HashSet::new();
    for (e, parent, mut t) in &mut glows {
        if marked.contains(parent.parent()) {
            have.insert(parent.parent());
            t.scale = Vec3::splat(GLOW_SIZE * breath);
        } else {
            commands.entity(e).despawn();
        }
    }
    for (gun, grab) in &marked {
        if have.contains(&gun) {
            continue;
        }
        let Some(p) = handle_point(grab) else {
            continue;
        };
        let (mesh, mat) = look
            .get_or_insert_with(|| {
                (
                    meshes.add(Sphere::new(0.5)),
                    materials.add(StandardMaterial {
                        // The glove's hover green (`render::props::tint_gloves`),
                        // bright enough to read on a small part.
                        base_color: Color::srgba(0.3, 0.8, 0.4, 0.45),
                        emissive: LinearRgba::rgb(0.3, 0.8, 0.4),
                        unlit: true,
                        alpha_mode: AlphaMode::Add,
                        ..default()
                    }),
                )
            })
            .clone();
        commands.spawn((
            Name::new("Handle glow"),
            HandleGlow,
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            NotShadowCaster,
            Transform::from_translation(p.local.translation).with_scale(Vec3::splat(GLOW_SIZE)),
            ChildOf(gun),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::{GrabPoint, GripRole};

    /// A marked gun's handle glows, where its handle is; worked (the mark
    /// gone), the glow goes; a gun never marked has none.
    #[test]
    fn a_handle_glows_while_its_gun_wants_working() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_plugins(HandleGlowPlugin);
        let at = Vec3::new(0.03, 0.02, 0.1);
        let grab = || Grabbable {
            points: vec![
                GrabPoint::new(Transform::IDENTITY, GripRole::Primary, 0.08),
                GrabPoint::new(Transform::from_translation(at), GripRole::Handle(0), 0.07),
            ],
        };
        let gun = app.world_mut().spawn((grab(), NeedsWorking)).id();
        let other = app.world_mut().spawn(grab()).id();
        app.update();
        let glows = |app: &mut App| -> Vec<(Entity, Vec3)> {
            let mut q = app
                .world_mut()
                .query_filtered::<(&ChildOf, &Transform), With<HandleGlow>>();
            q.iter(app.world())
                .map(|(c, t)| (c.parent(), t.translation))
                .collect()
        };
        assert_eq!(glows(&mut app), vec![(gun, at)], "one glow, at the handle");
        app.update();
        assert_eq!(glows(&mut app).len(), 1, "still one");
        let _ = other;
        app.world_mut().entity_mut(gun).remove::<NeedsWorking>();
        app.update();
        assert!(glows(&mut app).is_empty(), "worked: gone");
    }
}
