//! Where to pry: while a hand holds the crowbar, every seam it can be put
//! to within [`SHOW_WITHIN`] shows a small ring, in the grab ring's green,
//! where the claw seats (`interact::tools::crowbar::Pryable::seam`). A
//! nailed board end, a shutter's hinge edge, a latch: nothing on the
//! models says "here" from a few steps (`docs/LEGIBILITY.md`,
//! affordances), and painting them would be on things everyone sees all
//! the time; with the bar in hand is when it matters.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::interact::Hand;
use crate::interact::tools::crowbar::{Crowbar, Pryable};

pub struct PryCuePlugin;

impl Plugin for PryCuePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, pry_cues);
    }
}

/// How near a seam must be to the bar's hand to show (m).
pub const SHOW_WITHIN: f32 = 3.0;
/// The ring's size at a seam (m), before its pulse.
pub const SEAM_RING: f32 = 0.1;

#[derive(Component)]
struct PryCue;

#[allow(clippy::type_complexity)]
fn pry_cues(
    mut commands: Commands,
    hands: Query<&Hand>,
    bars: Query<(), With<Crowbar>>,
    seams: Query<(Entity, &GlobalTransform, &Pryable)>,
    mut cues: Query<(&mut Transform, &mut Visibility), With<PryCue>>,
    mut made: Local<std::collections::HashMap<Entity, Entity>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    let holders: Vec<Vec3> = hands
        .iter()
        .filter(|h| h.holding.is_some_and(|i| bars.contains(i)))
        .map(|h| h.world.translation)
        .collect();
    let pulse = 1.0 + 0.1 * (time.elapsed_secs() * 5.0).sin();
    let mut shown = std::collections::HashSet::new();
    for (e, gt, p) in &seams {
        let at = gt.transform_point(p.seam);
        let Some(hand) = holders
            .iter()
            .copied()
            .filter(|h| h.distance(at) <= SHOW_WITHIN)
            .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
        else {
            continue;
        };
        shown.insert(e);
        let cue = *made.entry(e).or_insert_with(|| {
            let (mesh, mat) = look
                .get_or_insert_with(|| {
                    (
                        meshes.add(Torus::new(0.85, 1.0)),
                        materials.add(StandardMaterial {
                            base_color: Color::srgba(0.3, 0.9, 0.4, 0.5),
                            emissive: LinearRgba::rgb(0.3, 0.9, 0.4),
                            unlit: true,
                            alpha_mode: AlphaMode::Add,
                            ..default()
                        }),
                    )
                })
                .clone();
            commands
                .spawn((
                    Name::new("Pry cue"),
                    PryCue,
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    NotShadowCaster,
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .id()
        });
        if let Ok((mut t, mut v)) = cues.get_mut(cue) {
            t.translation = at;
            t.rotation = Quat::from_rotation_arc(Vec3::Y, (hand - at).normalize_or(Vec3::Y));
            t.scale = Vec3::splat(SEAM_RING * pulse);
            *v = Visibility::Visible;
        }
    }
    for (seam, cue) in made.iter() {
        if !shown.contains(seam) {
            if let Ok((_, mut v)) = cues.get_mut(*cue) {
                *v = Visibility::Hidden;
            }
        }
    }
    // Seams that are gone take their cue along.
    made.retain(|seam, cue| {
        let keep = seams.contains(*seam);
        if !keep {
            commands.entity(*cue).despawn();
        }
        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the bar in hand, the seam a step away shows its ring and the
    /// one across the yard doesn't; put the bar down and it goes.
    #[test]
    fn the_seams_near_a_crowbar_show_where_it_goes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_plugins(PryCuePlugin);
        let bar = app.world_mut().spawn(Crowbar::default()).id();
        let hand = app.world_mut().spawn(Hand::default()).id();
        {
            let mut h = app.world_mut().get_mut::<Hand>(hand).unwrap();
            h.holding = Some(bar);
            h.world = Transform::from_xyz(0.0, 1.2, 0.0);
        }
        let seam = |at: Vec3| {
            Pryable {
                needs: 50.0,
                seam: Vec3::new(0.0, 0.5, 0.0),
                along: Vec3::X,
                held: 0.0,
            }
            .clone_with(at)
        };
        let near = app.world_mut().spawn(seam(Vec3::new(1.5, 0.5, 0.0))).id();
        let far = app.world_mut().spawn(seam(Vec3::new(9.0, 0.5, 0.0))).id();
        app.update();
        app.update();
        let shown = |app: &mut App| -> Vec<Vec3> {
            let w = app.world_mut();
            w.query_filtered::<(&Transform, &Visibility), With<PryCue>>()
                .iter(w)
                .filter(|(_, v)| **v == Visibility::Visible)
                .map(|(t, _)| t.translation)
                .collect()
        };
        let s = shown(&mut app);
        assert_eq!(s.len(), 1, "{s:?}");
        assert!(
            s[0].distance(Vec3::new(1.5, 1.0, 0.0)) < 1e-3,
            "at the seam: {s:?}"
        );
        let _ = (near, far);
        app.world_mut().get_mut::<Hand>(hand).unwrap().holding = None;
        app.update();
        assert!(shown(&mut app).is_empty());
    }

    trait At {
        fn clone_with(self, at: Vec3) -> (Pryable, Transform, GlobalTransform);
    }

    impl At for Pryable {
        fn clone_with(self, at: Vec3) -> (Pryable, Transform, GlobalTransform) {
            let t = Transform::from_translation(at);
            (self, t, GlobalTransform::from(t))
        }
    }
}
