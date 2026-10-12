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

/// The Startup systems that roll the Zone's loot (the save loader runs
/// before them, and they stand down when it restores a Zone instead).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct LootSpawnSystems;

impl Plugin for LootPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                load_tables.before(LootSpawnSystems),
                (spawn_zone_loot, spawn_dev_cache)
                    .chain()
                    .in_set(LootSpawnSystems),
            )
                .after(crate::world::buildings::spawn_buildings),
        );
    }
}

/// The loot seed for a Surge cycle: each reset rolls a different Zone.
fn cycle_seed(opts: &LaunchOptions, cycle: u32) -> u64 {
    opts.seed ^ (cycle as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

fn spawn_zone_loot(world: &mut World) {
    if world
        .get_resource::<crate::persist::Restore>()
        .is_some_and(|r| r.zone)
    {
        return;
    }
    roll_zone_loot(world);
}

/// Rolls every loot marker and building loot spot for the current cycle.
pub fn roll_zone_loot(world: &mut World) {
    let opts = world.resource::<LaunchOptions>().clone();
    let cycle = world.resource::<crate::raid::Raid>().cycle;
    let seed = cycle_seed(&opts, cycle);
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let zone = world.resource::<Zone>();
        let loot = world.resource::<Loot>();
        let catalog = world.resource::<Catalog>();
        let spots = world.get_resource::<crate::world::buildings::BuildingLoot>();
        let mut commands =
            Commands::new_from_entities(&mut queue, world.entity_allocator(), world.entities());
        spawn_marker_loot(&mut commands, zone, loot, catalog, seed);
        if let Some(spots) = spots {
            spawn_building_loot(&mut commands, zone, spots, loot, catalog, seed);
        }
    }
    queue.apply(world);
    crate::anomaly::roll_artifacts(world);
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
    commands: &mut Commands,
    zone: &Zone,
    loot: &Loot,
    catalog: &Catalog,
    seed: u64,
) {
    let mut rng = Rng::new(seed ^ 0x100);
    for m in &zone.map.markers {
        let MarkerKind::Loot { table } = &m.kind else {
            continue;
        };
        let base = zone.on_ground(m.at[0], m.at[1]) + Vec3::Y * (m.y + 0.1);
        let rolls = rolls_for(zone.map.richness_at(m.at), &mut rng);
        let drops: Vec<_> = (0..rolls)
            .flat_map(|_| loot.0.roll(table, catalog, &mut rng))
            .collect();
        for (i, drop) in drops.into_iter().enumerate() {
            let offset = Vec3::new(
                (i as f32 * 0.37).sin() * 0.25,
                0.05 * i as f32,
                (i as f32 * 0.37).cos() * 0.25,
            );
            let e = spawn_item(
                commands,
                catalog,
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

/// Loot rolls for a spot of this richness: the whole part, and the
/// fraction as a chance of one more.
fn rolls_for(richness: f32, rng: &mut Rng) -> usize {
    let whole = richness.max(0.0).floor();
    whole as usize + usize::from(rng.range(0.0, 1.0) < richness - whole)
}

/// Rolls every building loot spot (shelves, tables, crates).
fn spawn_building_loot(
    commands: &mut Commands,
    zone: &Zone,
    spots: &crate::world::buildings::BuildingLoot,
    loot: &Loot,
    catalog: &Catalog,
    seed: u64,
) {
    let mut rng = Rng::new(seed ^ 0xB11D);
    let mut n = 0;
    for spot in &spots.0 {
        let rolls = rolls_for(zone.map.richness_at([spot.at.x, spot.at.z]), &mut rng);
        let drops: Vec<_> = (0..rolls)
            .flat_map(|_| loot.0.roll(spot.table, catalog, &mut rng))
            .collect();
        for (i, drop) in drops.into_iter().enumerate() {
            let off = Vec3::new(
                (i as f32 * 1.7).sin() * 0.18,
                0.04 + i as f32 * 0.06,
                (i as f32 * 1.7).cos() * 0.12,
            );
            spawn_item(
                commands,
                catalog,
                SpawnItem {
                    kind: drop.kind,
                    transform: Transform::from_translation(spot.at + off).with_rotation(
                        Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU)),
                    ),
                    condition: drop.condition,
                },
            );
            n += 1;
        }
    }
    info!(
        "{n} items rolled into {} building loot spots",
        spots.0.len()
    );
}

/// Every gun, loaded magazines, ammunition and the maintenance kit on the
/// ground by the insertion point: a testing convenience for sessions that
/// don't save (scenarios, screenshots, `--scene`). A saved game starts with
/// a kit in the truck's stash instead.
fn spawn_dev_cache(
    mut commands: Commands,
    zone: Res<Zone>,
    catalog: Res<Catalog>,
    slot: Res<crate::persist::SaveSlot>,
) {
    if slot.0.is_some() {
        return;
    }
    let Some(ins) = zone.marker(&MarkerKind::Insertion) else {
        return;
    };
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
        ("side_rail", 0),
        ("weapon_light", 0),
        ("laser_module", 0),
        ("pu_scope", 0),
        ("power_cell", 0),
        ("power_cell", 0),
        ("emp_grenade", 0),
        ("compass", 0),
        ("map_case", 0),
        ("binoculars", 0),
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
