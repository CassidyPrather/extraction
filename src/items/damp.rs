//! Damp rounds, readable before they're loaded (#103). A round that got
//! wet (`weapons::weather`) misfires, hangs and now and then squibs
//! (`extraction_core::firearm`), so a box, a magazine or a loose round with
//! one in it says so: it looks it (its card or brass darkened and glossed,
//! as wet things are) and, taken in a hand, a callout beside it counts
//! them. The bench dries them (`raid::bench`, "Dry and re-prime").

use bevy::prelude::*;
use extraction_core::ammo::Load;
use extraction_core::items::Category;

use super::{Catalog, Item, ItemModelRoot};
use crate::interact::InteractEvent;
use crate::raid::{Raid, Show};

/// How many of a box's, a magazine's or a loose round's rounds are damp
/// (none for anything else). A stack's unlisted rounds are its own load,
/// which is never damp.
pub fn damp_rounds(item: &Item, category: Category) -> u32 {
    if !matches!(category, Category::Ammo | Category::Magazine) {
        return 0;
    }
    item.loads.0.iter().filter(|l| **l == Load::Damp).count() as u32
}

/// The callout for a thing with `damp` of its `rounds` damp.
pub fn damp_line(damp: u32, rounds: u32) -> String {
    if rounds <= 1 {
        "Damp. It may click, hang or squib. Dry it at the bench.".into()
    } else {
        format!(
            "{damp} of {rounds} rounds damp. They may click, hang or squib. Dry them at the bench."
        )
    }
}

pub struct DampPlugin;

impl Plugin for DampPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, say_damp);
    }
}

/// A thing with damp rounds taken in a hand: a callout beside it says so.
fn say_damp(
    mut events: MessageReader<InteractEvent>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    raid: Option<ResMut<Raid>>,
) {
    let Some(mut raid) = raid else {
        events.clear();
        return;
    };
    for ev in events.read() {
        let InteractEvent::Grabbed { item, .. } = ev else {
            continue;
        };
        let Ok(it) = items.get(*item) else {
            continue;
        };
        let damp = damp_rounds(it, catalog.get(it.kind()).category);
        if damp > 0 {
            raid.tell(damp_line(damp, it.rounds), Show::Item(*item));
        }
    }
}

pub struct DampVisualsPlugin;

/// Where a damp look is made: after a box's rounds are painted
/// (`items::rounds`).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DampVisualsSet;

impl Plugin for DampVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, damp_looks.in_set(DampVisualsSet));
    }
}

/// How a damp thing's materials change: darker, as wet card and brass
/// go, and glossier.
const DAMP_DARKEN: f32 = 0.55;
const DAMP_ROUGH: f32 = 0.35;

/// A damp thing's own darkened copies of its materials, and what each mesh
/// wore before (put back once it's dried).
#[derive(Component)]
pub struct DampLook(Vec<(Entity, Handle<StandardMaterial>)>);

impl DampLook {
    /// Its meshes back as they were.
    pub fn undo(&self, meshes: &mut Query<&mut MeshMaterial3d<StandardMaterial>>) {
        for (m, h) in &self.0 {
            if let Ok(mut mat) = meshes.get_mut(*m) {
                mat.0 = h.clone();
            }
        }
    }
}

fn damp_looks(
    mut commands: Commands,
    items: Query<(Entity, &Item, &Children, Option<&DampLook>)>,
    roots: Query<(), With<ItemModelRoot>>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    catalog: Res<Catalog>,
) {
    for (e, item, kids, look) in &items {
        let damp = damp_rounds(item, catalog.get(item.kind()).category) > 0;
        match (damp, look) {
            (true, None) => {
                let Some(root) = kids.iter().find(|c| roots.contains(*c)) else {
                    continue;
                };
                let mut was = Vec::new();
                for m in children.iter_descendants(root) {
                    let Ok(mut mat) = meshes.get_mut(m) else {
                        continue;
                    };
                    let Some(src) = materials.get(&mat.0).cloned() else {
                        continue;
                    };
                    was.push((m, mat.0.clone()));
                    let base = src.base_color.to_linear();
                    let wet = StandardMaterial {
                        base_color: Color::LinearRgba(LinearRgba::new(
                            base.red * DAMP_DARKEN,
                            base.green * DAMP_DARKEN,
                            base.blue * DAMP_DARKEN,
                            base.alpha,
                        )),
                        perceptual_roughness: src.perceptual_roughness * DAMP_ROUGH,
                        ..src
                    };
                    mat.0 = materials.add(wet);
                }
                if !was.is_empty() {
                    commands.entity(e).insert(DampLook(was));
                }
            }
            (false, Some(look)) => {
                for (m, h) in &look.0 {
                    if let Ok(mut mat) = meshes.get_mut(*m) {
                        mat.0 = h.clone();
                    }
                }
                commands.entity(e).remove::<DampLook>();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damp_rounds_are_counted_and_said() {
        let catalog = Catalog::load();
        let kind = catalog.kind("ammo_545").unwrap();
        let mut it = Item(extraction_core::items::ItemState::fresh(
            &catalog, kind, 1.0,
        ));
        assert_eq!(it.rounds, 30);
        assert_eq!(damp_rounds(&it, Category::Ammo), 0, "dry");
        assert!(it.loads.spoil(30, Load::Standard, 0));
        assert!(it.loads.spoil(30, Load::Standard, 5));
        assert_eq!(damp_rounds(&it, Category::Ammo), 2);
        assert_eq!(damp_rounds(&it, Category::Gadget), 0);
        assert!(damp_line(2, 30).starts_with("2 of 30 rounds damp."));
        assert!(damp_line(1, 1).starts_with("Damp."));
    }
}
