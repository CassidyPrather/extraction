//! What's in your hand reads in the dimmest room. In a room the eyes are
//! set for the room, and a gun at 30 cm in the Rime Fell mill came out a
//! black shape (8 of 255, the yard's 30: `scripts/held_check.py`): no
//! slide, no grip, nowhere to seat the magazine.
//!
//! A held item and the gloves keep [`SHARE`] of the stops the room cuts
//! (`render::fog`'s lighting patch, in a room only: out of doors nothing
//! changes). Only they do: a near fill by distance from the eye lit a wall
//! you stood beside as pale as fog. They're told apart by their material:
//! [`MARK`] in `attenuation_distance`, read only by transmission, which
//! nothing held or worn uses. A held item's materials are swapped for
//! marked copies (one per original, made once) while a hand holds it, and
//! back when it's let go; the gloves' own are marked.
//!
//! `EXTRACTION_NEAR_FILL=0` marks nothing (the A/B for shots and the
//! headset's frame log).

use bevy::prelude::*;
use std::collections::HashMap;

/// The mark (the shader's `XS_HELD_MARK`).
pub const MARK: f32 = 3.0e33;
/// The share of the room's cut a held thing keeps, in stops (the
/// shader's `XS_HELD_SHARE`). At 0.75 the PM looked lit; at 0.5 its slide
/// and grip read (38 of 255) and it still sits in the room.
pub const SHARE: f32 = 0.5;

pub struct HeldLightPlugin;

impl Plugin for HeldLightPlugin {
    fn build(&self, app: &mut App) {
        if !enabled() {
            return;
        }
        app.init_resource::<Marked>()
            .add_systems(PostUpdate, (mark_held, unmark_let_go, mark_gloves));
    }
}

/// Whether held things get their light: `EXTRACTION_NEAR_FILL=0` leaves it
/// out.
pub fn enabled() -> bool {
    std::env::var_os("EXTRACTION_NEAR_FILL").is_none_or(|v| v != "0")
}

/// On a thing the gallery puts at arm's length (`--gallery pm_pistol,0,0.3`):
/// lit as if a hand held it, for the shots and `scripts/held_check.py`.
#[derive(Component)]
pub struct ShownInHand;

type InHand = Or<(With<crate::interact::Held>, With<ShownInHand>)>;

/// The marked copy of each material a held thing has worn.
#[derive(Resource, Default)]
struct Marked(HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>);

/// On a mesh of a held thing: the material it had before it was marked.
#[derive(Component)]
struct WasMaterial(Handle<StandardMaterial>);

/// A material's marked copy.
pub fn marked(m: &StandardMaterial) -> StandardMaterial {
    StandardMaterial {
        attenuation_distance: MARK,
        ..m.clone()
    }
}

/// Every mesh of a held thing wears its marked material: on the frame it's
/// taken, and any part seated while it's held (a magazine).
fn mark_held(
    mut commands: Commands,
    held: Query<Entity, InHand>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>, Without<WasMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut marked_ones: ResMut<Marked>,
) {
    for item in &held {
        for e in std::iter::once(item).chain(children.iter_descendants(item)) {
            let Ok(mut m) = meshes.get_mut(e) else {
                continue;
            };
            let was = m.0.clone();
            let copy = match marked_ones.0.get(&was.id()) {
                Some(h) => h.clone(),
                None => {
                    let Some(orig) = materials.get(&was).map(marked) else {
                        continue;
                    };
                    let h = materials.add(orig);
                    marked_ones.0.insert(was.id(), h.clone());
                    h
                }
            };
            m.0 = copy;
            commands.entity(e).insert(WasMaterial(was));
        }
    }
}

/// Let go (or the part taken off): back to its own material.
fn unmark_let_go(
    mut commands: Commands,
    mut worn: Query<(Entity, &WasMaterial, &mut MeshMaterial3d<StandardMaterial>)>,
    parents: Query<&ChildOf>,
    held: Query<(), InHand>,
) {
    for (e, was, mut m) in &mut worn {
        let in_hand = held.contains(e) || parents.iter_ancestors(e).any(|a| held.contains(a));
        if !in_hand {
            m.0 = was.0.clone();
            commands.entity(e).remove::<WasMaterial>();
        }
    }
}

/// The gloves are always in hand: their own materials are marked.
fn mark_gloves(
    gloves: Query<&crate::render::props::GloveMaterial>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for h in gloves.iter().flat_map(|g| g.0.iter()) {
        let unmarked = materials
            .get(h)
            .is_some_and(|mat| mat.attenuation_distance != MARK);
        if unmarked {
            if let Some(mut mat) = materials.get_mut(h) {
                mat.attenuation_distance = MARK;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shader's mark and share are these.
    #[test]
    fn the_shader_reads_the_same_mark_and_share() {
        let src = crate::render::fog::wet_wgsl_for_tests();
        assert!(src.contains(&format!("const XS_HELD_MARK: f32 = {:.1e};", MARK)));
        assert!(src.contains(&format!("const XS_HELD_SHARE: f32 = {SHARE:.1};")));
    }

    /// A held thing's material keeps everything but the mark.
    #[test]
    fn a_marked_copy_differs_only_by_the_mark() {
        let m = StandardMaterial {
            base_color: Color::srgb(0.1, 0.12, 0.15),
            metallic: 1.0,
            perceptual_roughness: 0.3,
            ..default()
        };
        let c = marked(&m);
        assert_eq!(c.attenuation_distance, MARK);
        assert_eq!(c.base_color, m.base_color);
        assert_eq!(
            (c.metallic, c.perceptual_roughness),
            (m.metallic, m.perceptual_roughness)
        );
        assert_eq!(m.attenuation_distance, f32::INFINITY);
    }
}
