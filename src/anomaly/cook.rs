//! Cooking: a wreck's part, or any loose thing, left in a burner's coals
//! cooks through. What cooked means for salvage is Heavy's
//! (`robots::salvage`, which reads [`Cooked`]): holds burnt through and
//! off by hand, the part heat-struck, a live cell spent. A trade: a
//! burner is a fast, lossy way to strip a wreck.
//!
//! Stones aren't cooked: a stone in the coals is hot (`harvest::Hot`).

use bevy::prelude::*;
use extraction_core::anomaly::AnomalyKind;
use extraction_core::items::Category;

use super::Anomaly;
use crate::interact::Held;
use crate::items::{Catalog, Item};

/// Seconds in the coals to cook through.
pub const COOK_SECS: f32 = 6.0;
/// Seconds a cooked thing smoulders after (its look: `visuals`).
pub const SMOULDER_SECS: f32 = 20.0;

/// Cooking: seconds in the coals so far (it keeps them out of the coals,
/// and goes on from there if it's put back).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Cooking(pub f32);

/// Cooked through by a burner's coals; `smoulder` counts the seconds
/// since, for its smoke. Salvage reads it (Heavy).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Cooked {
    pub smoulder: f32,
}

/// A thing has just cooked through (for salvage, sound, a line).
#[derive(Message, Clone, Copy, Debug)]
pub struct PartCooked {
    pub entity: Entity,
    pub at: Vec3,
}

/// What lies in a burner's coals: inside its radius, and no more than
/// this over its ground.
const IN_COALS_UP: f32 = 1.2;

/// Cooks what lies in the coals: an item (not a stone, not one in a
/// hand), a dead machine's plate, a wreck itself.
#[allow(clippy::type_complexity)]
pub fn cook(
    mut commands: Commands,
    burners: Query<(&Anomaly, &GlobalTransform)>,
    things: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&Item>,
            Option<&mut Cooking>,
            Has<Cooked>,
        ),
        (
            Without<Held>,
            Or<(
                With<Item>,
                With<crate::robots::salvage::HealthAtDeath>,
                (
                    With<crate::robots::Robot>,
                    With<crate::robots::wreck::Broken>,
                ),
            )>,
        ),
    >,
    mut smoulders: Query<&mut Cooked>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut cooked: MessageWriter<PartCooked>,
) {
    let dt = time.delta_secs();
    for mut c in &mut smoulders {
        c.smoulder += dt;
    }
    let coals: Vec<(Vec3, f32)> = burners
        .iter()
        .filter(|(a, _)| a.kind == AnomalyKind::Burner)
        .map(|(a, g)| (g.translation(), a.radius))
        .collect();
    if coals.is_empty() {
        return;
    }
    let mut things = things;
    for (e, gt, item, cooking, done) in &mut things {
        if done || item.is_some_and(|i| catalog.get(i.kind()).category == Category::Artifact) {
            continue;
        }
        let p = gt.translation();
        let in_coals = coals.iter().any(|(c, r)| {
            let rel = p - *c;
            Vec2::new(rel.x, rel.z).length() < *r && rel.y < IN_COALS_UP
        });
        if !in_coals {
            continue;
        }
        match cooking {
            None => {
                commands.entity(e).try_insert(Cooking(dt));
            }
            Some(mut c) => {
                c.0 += dt;
                if c.0 >= COOK_SECS {
                    // Charred for good (`robots::salvage::Charred`: its look,
                    // kept through a save), whatever it was; salvage reads
                    // the message for what cooking does to a part.
                    commands
                        .entity(e)
                        .remove::<Cooking>()
                        .try_insert((Cooked::default(), crate::robots::salvage::Charred));
                    cooked.write(PartCooked { entity: e, at: p });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A spare part left in a burner cooks through in six seconds; one
    /// beside it doesn't, nor does a stone in it.
    #[test]
    fn what_lies_in_the_coals_cooks() {
        let mut app = crate::headless_app(
            crate::launch::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(0.0, 30.0);
        super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Burner, at, 2.0, None);
        let grow = |app: &mut App, id: &str, p: Vec3| {
            let e = crate::anomaly::surge::grow(app.world_mut(), id, p).unwrap();
            app.world_mut()
                .entity_mut(e)
                .insert(avian3d::prelude::RigidBody::Static);
            e
        };
        let part = grow(&mut app, "spare_parts", at + Vec3::Y * 0.2);
        let beside = grow(&mut app, "spare_parts", at + Vec3::new(4.0, 0.2, 0.0));
        let stone = grow(&mut app, "art_hearth", at + Vec3::new(0.5, 0.2, 0.0));
        for _ in 0..(90.0 * (COOK_SECS + 0.5)) as usize {
            app.update();
        }
        assert!(app.world().get::<Cooked>(part).is_some(), "cooked through");
        assert!(
            app.world().get::<Cooked>(beside).is_none(),
            "not out of the coals"
        );
        assert!(
            app.world().get::<Cooked>(stone).is_none(),
            "a stone's only hot"
        );
    }

    /// Cooked stays charred through a save (Heavy's `SavedItem.charred`).
    #[test]
    fn a_cooked_thing_is_saved_charred() {
        let mut app = crate::headless_app(
            crate::launch::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(0.0, 30.0);
        super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Burner, at, 2.0, None);
        let e = crate::anomaly::surge::grow(app.world_mut(), "spare_parts", at + Vec3::Y * 0.2)
            .unwrap();
        app.world_mut()
            .entity_mut(e)
            .insert(avian3d::prelude::RigidBody::Static);
        for _ in 0..(90.0 * (COOK_SECS + 0.5)) as usize {
            app.update();
        }
        assert!(
            app.world()
                .get::<crate::robots::salvage::Charred>(e)
                .is_some(),
            "charred"
        );
        let saved = crate::persist::snapshot_item(app.world(), e, Vec3::ZERO).unwrap();
        assert!(saved.charred, "saved charred");
    }
}
