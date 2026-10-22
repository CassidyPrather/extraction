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
                supply_crates,
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
    for i in 0..ROLL_STEPS {
        roll_zone_loot_step(world, i);
    }
}

/// The roll's parts, for doing it over several frames (the Surge's reset
/// at base, `persist::ZoneReset`: at once it was a frame's 7 ms and the
/// next frame's setting up of all that came). The same seed and order,
/// so the same loot.
pub const ROLL_STEPS: usize = 5;

/// One part of `roll_zone_loot`: the markers' loot, the scenes' things,
/// the buildings' loot, the tapes, the artifacts.
pub fn roll_zone_loot_step(world: &mut World, step: usize) {
    if step == 4 {
        crate::anomaly::roll_artifacts(world);
        return;
    }
    let opts = world.resource::<LaunchOptions>().clone();
    let cycle = world.resource::<crate::raid::Raid>().cycle;
    let seed = cycle_seed(&opts, cycle);
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let zone = world.resource::<Zone>();
        let loot = world.resource::<Loot>();
        let catalog = world.resource::<Catalog>();
        let mut commands =
            Commands::new_from_entities(&mut queue, world.entity_allocator(), world.entities());
        match step {
            0 => spawn_marker_loot(&mut commands, zone, loot, catalog, seed),
            1 => crate::world::scenes::spawn_things(
                &mut commands,
                &opts.map,
                zone,
                catalog,
                crate::world::scenes::gates(world, &opts.map),
            ),
            2 => {
                if let Some(spots) = world.get_resource::<crate::world::buildings::BuildingLoot>() {
                    spawn_building_loot(&mut commands, zone, spots, loot, catalog, seed);
                }
            }
            _ => crate::radio::places::lay_tapes(&mut commands, zone, catalog, &opts.map),
        }
    }
    queue.apply(world);
}
fn load_tables(mut commands: Commands, catalog: Res<Catalog>) {
    let path = crate::asset_root_path().join("data/loot.ron");
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| BUILTIN_LOOT.to_owned());
    let tables = LootTables::from_ron(&text, &catalog).unwrap_or_else(|e| {
        error!("loot.ron: {e}");
        LootTables::from_ron(BUILTIN_LOOT, &catalog).expect("built-in loot parses")
    });
    commands.insert_resource(Loot(std::sync::Arc::new(tables)));
}

/// A crate set down under loot in the open (dressed by `render::props`).
#[derive(Component)]
pub struct SupplyCrate;

/// Its size: the lid at a standing reach.
pub const SUPPLY_CRATE: Vec3 = Vec3::new(0.9, 0.78, 0.6);

/// Where each note lies (as `world::notes` lays it) and which way it
/// turns: loot rolled beside one is kept off its words.
pub fn notes_lying(zone: &Zone) -> Vec<(Vec3, Quat)> {
    zone.map
        .markers
        .iter()
        .filter(|m| matches!(m.kind, MarkerKind::Note { .. }))
        .map(|m| {
            let y = crate::world::notes::rest_height(zone, m.at[0], m.at[1], 2.2 + m.y);
            (
                Vec3::new(m.at[0], y, m.at[1]),
                Quat::from_rotation_y(-m.rot.to_radians()),
            )
        })
        .collect()
}

/// How far a rolled item's middle stays from a note's paper.
pub const CLEAR_OF_NOTE: f32 = 0.1;

/// Where an item rolled at `at` lands, moved the least way off any note it
/// would lie on (`docs/LEGIBILITY.md`: a torch on the pump log hid half of
/// it).
pub fn clear_of_notes(at: Vec3, notes: &[(Vec3, Quat)]) -> Vec3 {
    let half = crate::world::notes::NOTE * 0.5 + Vec2::splat(CLEAR_OF_NOTE);
    let mut at = at;
    for (note, rot) in notes {
        let p = rot.inverse() * (at - *note);
        if p.y.abs() > 0.4 || p.x.abs() >= half.x || p.z.abs() >= half.y {
            continue;
        }
        let mut q = p;
        if half.x - p.x.abs() < half.y - p.z.abs() {
            q.x = half.x.copysign(p.x);
        } else {
            q.z = half.y.copysign(p.z);
        }
        at = *note + *rot * q;
    }
    at
}

/// The table a supply crate was packed from (broken open, it spills a
/// roll of it: `destruction::wood`).
#[derive(Component, Clone, Debug)]
pub struct CrateLoot(pub String);

/// The supply crates under the loot markers in the open. They're part of
/// the place, not the loot: there whether the Zone was rolled or
/// restored, and not doubled when the Surge rolls it again.
pub fn spawn_supply_crates(commands: &mut Commands, zone: &Zone) {
    for m in &zone.map.markers {
        let MarkerKind::Loot { table } = &m.kind else {
            continue;
        };
        if m.y >= 0.3 {
            continue;
        }
        let ground = zone.on_ground(m.at[0], m.at[1]);
        let yaw = (m.at[0] * 0.37 + m.at[1] * 0.11).sin() * 3.0;
        commands.spawn((
            Name::new(format!("Supply crate ({})", m.name)),
            SupplyCrate,
            CrateLoot(table.clone()),
            Transform::from_translation(ground + Vec3::Y * SUPPLY_CRATE.y * 0.5)
                .with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(SUPPLY_CRATE.x, SUPPLY_CRATE.y, SUPPLY_CRATE.z),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::World,
                avian3d::prelude::LayerMask::ALL,
            ),
        ));
    }
}

fn supply_crates(mut commands: Commands, zone: Res<Zone>) {
    spawn_supply_crates(&mut commands, &zone);
}

fn spawn_marker_loot(
    commands: &mut Commands,
    zone: &Zone,
    loot: &Loot,
    catalog: &Catalog,
    seed: u64,
) {
    let mut rng = Rng::new(seed ^ 0x100);
    let notes = notes_lying(zone);
    let valley = extraction_core::loot::valley_key(&zone.map.name);
    for m in &zone.map.markers {
        let MarkerKind::Loot { table } = &m.kind else {
            continue;
        };
        let ground = zone.on_ground(m.at[0], m.at[1]);
        // Loot isn't left on the ground for you to stoop for: a marker in
        // the open gets a supply crate, and the loot sits on its lid.
        let base = if m.y < 0.3 {
            ground + Vec3::Y * (SUPPLY_CRATE.y + 0.06)
        } else {
            ground + Vec3::Y * (m.y + 0.1)
        };
        // A marker is a place someone meant to leave something: never bare.
        let rolls = rolls_for(zone.map.richness_at(m.at), &mut rng).max(1);
        let drops: Vec<_> = (0..rolls)
            .flat_map(|_| loot.0.roll_in(table, Some(&valley), catalog, &mut rng))
            .collect();
        for (i, drop) in drops.into_iter().enumerate() {
            let offset = marker_roll(i);
            let e = spawn_item(
                commands,
                catalog,
                SpawnItem {
                    kind: drop.kind,
                    transform: Transform::from_translation(clear_of_notes(base + offset, &notes)),
                    condition: drop.condition,
                },
            );
            let _ = e;
        }
    }
}

/// Where a building spot's `i`th roll lands, from the spot.
pub fn building_roll(i: usize) -> Vec3 {
    Vec3::new(
        (i as f32 * 1.7).sin() * 0.18,
        0.04 + i as f32 * 0.06,
        (i as f32 * 1.7).cos() * 0.12,
    )
}

/// Where a loot marker's `i`th roll lands, from its base.
pub fn marker_roll(i: usize) -> Vec3 {
    Vec3::new(
        (i as f32 * 0.37).sin() * 0.25,
        0.05 * i as f32,
        (i as f32 * 0.37).cos() * 0.18,
    )
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
    let notes = notes_lying(zone);
    let valley = extraction_core::loot::valley_key(&zone.map.name);
    for spot in &spots.0 {
        let rolls = rolls_for(zone.map.richness_at([spot.at.x, spot.at.z]), &mut rng);
        let drops: Vec<_> = (0..rolls)
            .flat_map(|_| loot.0.roll_in(spot.table, Some(&valley), catalog, &mut rng))
            .collect();
        for (i, drop) in drops.into_iter().enumerate() {
            let at = clear_of_notes(spot.at + building_roll(i), &notes);
            spawn_item(
                commands,
                catalog,
                SpawnItem {
                    kind: drop.kind,
                    transform: Transform::from_translation(at).with_rotation(
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
    opts: Res<crate::launch::LaunchOptions>,
) {
    // (Not in the yard: it has its own kit, in the crate.)
    if slot.0.is_some() || opts.map == crate::camp::TRAINING_YARD {
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
        // (Last, so nothing above moves.)
        ("sks", 0),
        ("ammo_762x39", 20),
        ("sks_clip", 10),
        ("nagant", 0),
        ("ammo_762x38r", 14),
        ("ppsh", 0),
        ("ppsh_drum", 71),
        ("ammo_762x25", 70),
        ("mp133", 0),
        ("svd", 0),
        ("svd_mag", 10),
        ("toz8", 0),
        ("ammo_22lr", 50),
        ("rpk74", 0),
        ("rpk_mag", 45),
        ("flare_pistol", 0),
        ("flare_white", 1),
        ("flare_red", 1),
        ("crossbow", 0),
        ("bolt_bundle", 5),
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
