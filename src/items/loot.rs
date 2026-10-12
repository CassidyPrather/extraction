//! Loot: tables loaded from `assets/data/loot.ron`, rolled at the map's
//! loot markers when a raid starts, plus the developer cache.

use bevy::prelude::*;
use extraction_core::loot::{BUILTIN_LOOT, LootTables};
use extraction_core::map::MarkerKind;
use extraction_core::noise::Rng;

use super::{Catalog, Item, SpawnItem, spawn_item};
use crate::launch::LaunchOptions;
use crate::world::Zone;

#[derive(Resource, Clone)]
pub struct Loot(pub std::sync::Arc<LootTables>);

pub struct LootPlugin;

impl Plugin for LootPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (load_tables, spawn_marker_loot, spawn_dev_cache).chain(),
        );
    }
}

fn load_tables(mut commands: Commands, catalog: Res<Catalog>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/data/loot.ron");
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| BUILTIN_LOOT.to_owned());
    let tables = LootTables::from_ron(&text, &catalog).unwrap_or_else(|e| {
        error!("loot.ron: {e}");
        LootTables::from_ron(BUILTIN_LOOT, &catalog).expect("built-in loot parses")
    });
    commands.insert_resource(Loot(std::sync::Arc::new(tables)));
}

fn spawn_marker_loot(
    mut commands: Commands,
    zone: Res<Zone>,
    loot: Res<Loot>,
    catalog: Res<Catalog>,
    opts: Res<LaunchOptions>,
) {
    let mut rng = Rng::new(opts.seed ^ 0x100);
    for m in &zone.map.markers {
        let MarkerKind::Loot { table } = &m.kind else {
            continue;
        };
        let base = zone.on_ground(m.at[0], m.at[1]) + Vec3::Y * (m.y + 0.1);
        for (i, drop) in loot
            .0
            .roll(table, &catalog, &mut rng)
            .into_iter()
            .enumerate()
        {
            let offset = Vec3::new(
                (i as f32 * 0.37).sin() * 0.25,
                0.05 * i as f32,
                (i as f32 * 0.37).cos() * 0.25,
            );
            let e = spawn_item(
                &mut commands,
                &catalog,
                SpawnItem {
                    kind: drop.kind,
                    transform: Transform::from_translation(base + offset),
                    condition: drop.condition,
                },
            );
            let _ = e;
        }
    }
}

/// Every gun, loaded magazines, ammunition and the maintenance kit on the
/// ground by the insertion point. A testing convenience while the truck
/// and its stash don't exist yet.
fn spawn_dev_cache(
    mut commands: Commands,
    zone: Res<Zone>,
    catalog: Res<Catalog>,
    opts: Res<LaunchOptions>,
) {
    let Some(ins) = zone.marker(&MarkerKind::Insertion) else {
        return;
    };
    let _ = opts;
    let origin = Vec2::new(ins.at[0] + 2.5, ins.at[1] - 3.0);
    let list: &[(&str, u32)] = &[
        ("pm_pistol", 0),
        ("ak74", 0),
        ("mosin", 0),
        ("toz_double", 0),
        ("pm_mag", 8),
        ("pm_mag", 8),
        ("ak_mag", 30),
        ("ak_mag", 30),
        ("ammo_9x18", 16),
        ("ammo_545", 30),
        ("ammo_762r", 10),
        ("ammo_12ga", 10),
        ("cleaning_kit", 0),
        ("spare_parts", 0),
        ("flashlight", 0),
        ("canned_stew", 0),
        ("art_featherstone", 0),
    ];
    for (i, (id, rounds)) in list.iter().enumerate() {
        let Some(kind) = catalog.kind(id) else {
            continue;
        };
        let col = (i % 6) as f32;
        let row = (i / 6) as f32;
        let x = origin.x + col * 0.7;
        let z = origin.y - row * 0.9;
        let y = zone.ground(x, z) + 0.25;
        let e = spawn_item(
            &mut commands,
            &catalog,
            SpawnItem {
                kind,
                transform: Transform::from_xyz(x, y, z)
                    .with_rotation(Quat::from_rotation_y(0.4 * col)),
                condition: 0.85,
            },
        );
        if *rounds > 0 {
            commands.entity(e).queue(move |mut ent: EntityWorldMut| {
                if let Some(mut it) = ent.get_mut::<Item>() {
                    it.rounds = rounds.to_owned();
                }
            });
        }
    }
}
