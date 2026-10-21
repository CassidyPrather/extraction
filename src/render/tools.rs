//! How the hand tools move as you work them. The bolt cutters'
//! handles spread to follow your hands (`art/items/tools.py`: each handle
//! and jaw is its own node, turning about Y on its pivot), and the jaws
//! open with them.

use bevy::prelude::*;

use crate::interact::tools::cutters::{Cutters, LEFT};
use crate::items::ItemModelRoot;

/// The handles from their pivots to the grips (m), and how far the jaws
/// open for each radian the handles spread (the lever's ratio).
const HANDLE: f32 = 0.4;
const JAW_RATIO: f32 = 0.4;
/// Furthest a handle swings out, radians.
const SPREAD: f32 = 0.5;

pub struct ToolVisualsPlugin;

impl Plugin for ToolVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            swing_cutters.before(TransformSystems::Propagate),
        );
    }
}

/// A part of the cutters that swings, and its pose as modelled (shut).
#[derive(Component, Clone, Copy)]
struct Swings {
    rest: Quat,
    /// Handles turn by the spread; jaws by the spread over the lever.
    jaw: bool,
    /// Which way it turns about Y as the tool opens.
    sign: f32,
}

fn swing_cutters(
    mut commands: Commands,
    cutters: Query<(&Cutters, &Children)>,
    roots: Query<&Children, With<ItemModelRoot>>,
    mut parts: Query<(Entity, &Name, &mut Transform, Option<&Swings>)>,
) {
    for (c, children) in &cutters {
        // Each handle swings out to where its hand is: half the hands'
        // spread from the middle, a handle's length from its pivot.
        let out = ((c.spread * 0.5 - LEFT.x.abs()) / HANDLE).clamp(0.0, SPREAD.sin());
        let handle = out.asin();
        for root in children.iter().filter_map(|c| roots.get(c).ok()) {
            for part in root.iter() {
                let Ok((e, name, mut t, swings)) = parts.get_mut(part) else {
                    continue;
                };
                let s = match swings {
                    Some(s) => *s,
                    None => {
                        let (jaw, sign) = match name.as_str() {
                            "handle_left" => (false, -1.0),
                            "handle_right" => (false, 1.0),
                            // The jaws open the other way (checked on the
                            // model): each is worked by the far handle.
                            "jaw_left" => (true, 1.0),
                            "jaw_right" => (true, -1.0),
                            _ => continue,
                        };
                        let s = Swings {
                            rest: t.rotation,
                            jaw,
                            sign,
                        };
                        commands.entity(e).insert(s);
                        s
                    }
                };
                let a = if s.jaw { handle * JAW_RATIO } else { handle };
                t.rotation = s.rest * Quat::from_rotation_y(a * s.sign);
            }
        }
    }
}
