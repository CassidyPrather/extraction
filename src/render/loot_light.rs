//! Loot lit as the spot in the room it lies in: an item indoors takes
//! the building's sky light where it is (`world::ao::SkyField`, the
//! light its surfaces and furniture are baked with), when its model
//! comes in and again when it comes to rest somewhere new; in a hand
//! it's lit as the hands are. Its materials swap for copies with their
//! base colour scaled, in a few steps shared by every item, so items
//! still batch. `capture::checks::loot_sights` gauges what this shows.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::interact::Held;
use crate::items::{Item, ItemModelRoot};
use crate::world::buildings::RoomLight;

pub struct LootLightPlugin;

impl Plugin for LootLightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tints>()
            .add_systems(PostUpdate, tint_loot);
    }
}

/// Steps the light is shared out in (of before the field).
const STEP: f32 = 0.125;
/// How far an item moves before it's lit afresh (m).
const MOVED: f32 = 0.25;

/// Where an item was last lit, and how.
#[derive(Component)]
struct Lit {
    at: Vec3,
    step: u16,
}

/// A primitive's own material, before any tint.
#[derive(Component)]
struct OwnMaterial(Handle<StandardMaterial>);

/// Tinted copies of materials: (material, step) to the copy.
#[derive(Resource, Default)]
struct Tints(HashMap<(AssetId<StandardMaterial>, u16), Handle<StandardMaterial>>);

/// The light at a world point, as a share of before the field: the
/// building it's in, if any, and its field there (1 out of doors).
pub fn light_at(
    p: Vec3,
    entrances: &crate::robots::doors::Entrances,
    rooms: &Query<(&RoomLight, &Transform)>,
) -> f32 {
    let Some(b) = entrances.building_at(p) else {
        return 1.0;
    };
    let Ok((room, xf)) = rooms.get(b) else {
        return 1.0;
    };
    let Some(field) = &room.0.light else {
        return 1.0;
    };
    let l = xf.compute_affine().inverse().transform_point3(p);
    crate::world::ao::sky_light(field.at(l + Vec3::Y * 0.15)).max(crate::world::ao::SKY_MIN)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tint_loot(
    mut commands: Commands,
    items: Query<(Entity, &GlobalTransform, Has<Held>, Option<&Lit>, &Children), With<Item>>,
    roots: Query<(), With<ItemModelRoot>>,
    children: Query<&Children>,
    mut mats: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&OwnMaterial>)>,
    rooms: Query<(&RoomLight, &Transform)>,
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    velocity: Query<&avian3d::prelude::LinearVelocity>,
    mut tints: ResMut<Tints>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(entrances) = entrances else {
        return;
    };
    for (e, gt, held, lit, kids) in &items {
        // Its model in.
        let Some(root) = kids.iter().find(|c| roots.contains(*c)) else {
            continue;
        };
        let p = gt.translation();
        let step = if held {
            // Lit as the hands are.
            (1.0 / STEP) as u16
        } else {
            // At rest, or just in: not while it flies.
            let moving = velocity.get(e).is_ok_and(|v| v.0.length() > 0.2);
            if lit.is_some_and(|l| moving || l.at.distance(p) < MOVED) {
                continue;
            }
            (light_at(p, &entrances, &rooms) / STEP).round().max(1.0) as u16
        };
        if lit.is_some_and(|l| l.step == step) {
            if !held {
                commands.entity(e).insert(Lit { at: p, step });
            }
            continue;
        }
        commands.entity(e).insert(Lit { at: p, step });
        let k = step as f32 * STEP;
        for d in std::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok((mut m, own)) = mats.get_mut(d) else {
                continue;
            };
            let base = match own {
                Some(o) => o.0.clone(),
                None => {
                    commands.entity(d).insert(OwnMaterial(m.0.clone()));
                    m.0.clone()
                }
            };
            if (k - 1.0).abs() < 1e-3 {
                m.0 = base;
                continue;
            }
            let copy = tints
                .0
                .entry((base.id(), step))
                .or_insert_with(|| {
                    let mut c = materials.get(&base).cloned().unwrap_or_default();
                    let l = c.base_color.to_linear();
                    c.base_color = Color::linear_rgba(l.red * k, l.green * k, l.blue * k, l.alpha);
                    materials.add(c)
                })
                .clone();
            m.0 = copy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::buildings;
    use bevy::ecs::system::RunSystemOnce;

    /// In a plank house, what's put down by its front windows is lit
    /// brighter than what's put down in its back corner, and outside it's
    /// as the day.
    #[test]
    fn loot_takes_the_light_of_the_room_it_lies_in() {
        let mut app = App::new();
        let id = "house_plank";
        let geo = buildings::prefab(id).unwrap();
        let f = extraction_core::map::prefab_footprint(id);
        let occ = buildings::occluders(&geo).within(Vec2::new(f[0], f[2]) * 0.5);
        // (Made the first time it's asked for.)
        occ.sky_view(Vec3::Y, Vec3::Y);
        let baked = buildings::Baked {
            meshes: Vec::new(),
            light: occ.into_field(),
        };
        let xf = Transform::from_xyz(40.0, 2.0, -10.0);
        let b = app
            .world_mut()
            .spawn((RoomLight(std::sync::Arc::new(baked)), xf))
            .id();
        let mut entrances = crate::robots::doors::Entrances::default();
        entrances
            .shells
            .push(crate::robots::doors::Shell::of(b, id, xf));
        app.insert_resource(entrances);
        let at = move |x: f32, z: f32| xf.transform_point(Vec3::new(x, 0.9, z));
        let (front, corner, outside) = app
            .world_mut()
            .run_system_once(
                move |e: Res<crate::robots::doors::Entrances>,
                      rooms: Query<(&RoomLight, &Transform)>| {
                    (
                        light_at(at(1.5, -2.9), &e, &rooms),
                        light_at(at(-3.5, 3.0), &e, &rooms),
                        light_at(at(9.0, 0.0), &e, &rooms),
                    )
                },
            )
            .unwrap();
        assert!(front > corner * 1.5, "front {front}, corner {corner}");
        assert_eq!(outside, 1.0);
    }
}
