//! Children attached to a tree that's already in place get their
//! `GlobalTransform`, every time.
//!
//! Bevy 0.19's static-scene optimisation (`StaticTransformOptimizations`)
//! only propagates transforms down trees whose `TransformTreeChanged` is
//! marked. `mark_dirty_trees` marks the way up from each changed entity
//! but stops early at an ancestor already marked. Its multi-threaded
//! version splits that walk across workers that exit early together. A
//! child attached under a tree that's been still for a while can be left
//! unmarked, and its `GlobalTransform` never leaves `[0, 0, 0]`.
//! Cinder Works' blast furnace drew under the map that way: its meshes,
//! dressed in after the building stood, sat at the origin while its lamp
//! (spawned with it) stood at its top. Whether it hit depended on the
//! build's system order and the raid's roll.
//!
//! The guard: between `mark_dirty_trees` and the propagation, every entity
//! attached this frame (or newly given a transform) marks every ancestor's
//! `TransformTreeChanged`, all the way up, with no early exit. It costs
//! only on frames where something's attached. The optimisation stays on.

use bevy::prelude::*;
use bevy::transform::components::TransformTreeChanged;
use bevy::transform::systems::{mark_dirty_trees, propagate_parent_transforms};

pub struct TransformGuardPlugin;

impl Plugin for TransformGuardPlugin {
    fn build(&self, app: &mut App) {
        // For showing what it guards against: off, the bug's back.
        if std::env::var_os("EXTRACTION_NO_TRANSFORM_GUARD").is_some() {
            return;
        }
        app.add_systems(
            PostUpdate,
            mark_attached_trees
                .after(mark_dirty_trees)
                .before(propagate_parent_transforms)
                .in_set(TransformSystems::Propagate),
        );
    }
}

/// Marks the whole way up from everything attached or newly placed this
/// frame, so propagation walks down to it.
#[allow(clippy::type_complexity)]
pub fn mark_attached_trees(
    attached: Query<Entity, Or<(Added<ChildOf>, Changed<ChildOf>, Added<GlobalTransform>)>>,
    parents: Query<&ChildOf>,
    mut trees: Query<&mut TransformTreeChanged>,
) {
    for e in &attached {
        let mut at = e;
        loop {
            if let Ok(mut tree) = trees.get_mut(at) {
                tree.set_changed();
            }
            match parents.get(at) {
                Ok(p) => at = p.parent(),
                Err(_) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::transform::TransformPlugin;

    fn app(guard: bool) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        if guard {
            app.add_plugins(TransformGuardPlugin);
        }
        app
    }

    /// Every child, attached in each of the ways the game does (to a still
    /// tree, a deep one, two at once, while another tree moves, a frame
    /// apart), ends up where its parent puts it. (A small app like this
    /// one doesn't lose them without the guard either: the race needs the
    /// game's full schedule. The game-level proof is
    /// `EXTRACTION_NO_TRANSFORM_GUARD=1` against Cinder's dusk landmark,
    /// docs/CAPTURE.md.)
    fn attach_patterns(guard: bool) -> Vec<String> {
        let mut bad = Vec::new();
        let mut app = app(guard);
        let w = app.world_mut();
        // Still trees, some deep, as the valley's buildings are.
        let roots: Vec<Entity> = (0..40)
            .map(|i| {
                w.spawn(Transform::from_xyz(i as f32 * 10.0, 5.0, -3.0))
                    .id()
            })
            .collect();
        let mut mids = Vec::new();
        for (i, r) in roots.iter().enumerate() {
            let m = w
                .spawn((Transform::from_xyz(1.0, i as f32, 0.0), ChildOf(*r)))
                .id();
            mids.push(m);
        }
        let mover = w.spawn(Transform::default()).id();
        for _ in 0..5 {
            app.update();
        }
        let mut kids = Vec::new();
        for round in 0..6 {
            {
                let w = app.world_mut();
                // Something else in the world moving this frame.
                w.get_mut::<Transform>(mover).unwrap().translation.x += 1.0;
                for (i, (r, m)) in roots.iter().zip(&mids).enumerate() {
                    if (i + round) % 3 != 0 {
                        continue;
                    }
                    let at = Transform::from_xyz(0.5, 2.0 + round as f32, 0.25);
                    // Under the root, and under its child, in one frame.
                    kids.push(w.spawn((at, ChildOf(*r))).id());
                    kids.push(w.spawn((at, ChildOf(*m))).id());
                }
            }
            app.update();
            if round % 2 == 0 {
                app.update();
            }
        }
        app.update();
        let w = app.world_mut();
        for k in kids {
            let parent = w.get::<ChildOf>(k).unwrap().parent();
            let want = w
                .get::<GlobalTransform>(parent)
                .unwrap()
                .mul_transform(*w.get::<Transform>(k).unwrap());
            let got = *w.get::<GlobalTransform>(k).unwrap();
            if got.translation().distance(want.translation()) > 1e-4 {
                bad.push(format!(
                    "{k:?}: at {} not {}",
                    got.translation(),
                    want.translation()
                ));
            }
        }
        bad
    }

    #[test]
    fn children_attached_to_still_trees_are_placed() {
        let bad = attach_patterns(true);
        assert!(
            bad.is_empty(),
            "children left unplaced:\n{}",
            bad.join("\n")
        );
    }
}
