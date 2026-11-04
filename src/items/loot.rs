//! Loot: tables loaded from `assets/data/loot.ron`, rolled at the map's
//! loot markers when a raid starts, plus the developer cache.

use bevy::prelude::*;
use extraction_core::loot::{BUILTIN_LOOT, LootTables};
use extraction_core::map::MarkerKind;
use extraction_core::noise::Rng;

use super::{Catalog, Item, SpawnItem, spawn_item};
use crate::launch::LaunchOptions;
use crate::world::Zone;

/// The tables, and the raid of the career they roll for (`None`: no
/// career, every family; kept by `follow_the_career`).
#[derive(Resource, Clone)]
pub struct Loot(pub std::sync::Arc<LootTables>, pub Option<u32>);

impl Loot {
    /// Rolls `table` for this raid of the career: a family not out yet
    /// stays out, whatever rolls it (a wreck, a cargo pod, a crate, the
    /// driver's cache), as at the zone's markers.
    pub fn roll(
        &self,
        table: &str,
        catalog: &extraction_core::items::ItemCatalog,
        rng: &mut Rng,
    ) -> Vec<extraction_core::loot::Drop> {
        self.0.roll_at(table, None, self.1, catalog, rng)
    }
}

/// Keeps `Loot`'s raid the career's.
fn follow_the_career(
    loot: Option<ResMut<Loot>>,
    opts: Res<LaunchOptions>,
    progression: Option<Res<crate::progression::Progression>>,
) {
    let Some(mut loot) = loot else {
        return;
    };
    let raid = career_raid_of(&opts, progression.as_deref());
    if loot.1 != raid {
        loot.1 = raid;
    }
}

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
        )
        .add_systems(PreUpdate, follow_the_career);
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

/// Which raid of a career this is, for the loot's `from_raid` (the first is
/// raid 0: raids set off before it, every valley's), or `None` with no
/// career in play: a sim, a test's world, a gallery (every entry rolls, as
/// before; a test app counted as a career's raid 0 rolled other loot, and
/// the gate failed on two bugs that loot showed up).
fn career_raid(world: &World, opts: &LaunchOptions) -> Option<u32> {
    career_raid_of(
        opts,
        world.get_resource::<crate::progression::Progression>(),
    )
}

/// [`career_raid`], from the career as a system has it.
pub fn career_raid_of(
    opts: &LaunchOptions,
    progression: Option<&crate::progression::Progression>,
) -> Option<u32> {
    if opts.sim_run.is_some() || crate::persist::career_path(opts).is_none() {
        return None;
    }
    Some(progression?.0.raids_by_map.iter().map(|(_, n)| *n).sum())
}

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
    let raid = career_raid(world, &opts);
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let zone = world.resource::<Zone>();
        let loot = world.resource::<Loot>();
        let catalog = world.resource::<Catalog>();
        let mut commands =
            Commands::new_from_entities(&mut queue, world.entity_allocator(), world.entities());
        match step {
            0 => spawn_marker_loot(&mut commands, zone, loot, catalog, seed, raid),
            1 => crate::world::scenes::spawn_things(
                &mut commands,
                &opts.map,
                zone,
                catalog,
                crate::world::scenes::gates(world, &opts.map),
                &|id| arrived(&loot.0, raid, id),
            ),
            2 => {
                if let Some(spots) = world.get_resource::<crate::world::buildings::BuildingLoot>() {
                    spawn_building_loot(&mut commands, zone, spots, loot, catalog, seed, raid);
                }
            }
            _ => crate::radio::places::lay_tapes(&mut commands, zone, catalog, &opts.map, &|id| {
                arrived(&loot.0, raid, id)
            }),
        }
    }
    queue.apply(world);
}
/// Whether kind `id` is out yet on career raid `raid` (`None`: no career,
/// everything is): the raid any table first drops it, for the sources
/// that aren't tables (a scene's things). A kind no table drops is out.
pub fn arrived(loot: &LootTables, raid: Option<u32>, id: &str) -> bool {
    raid.is_none_or(|r| loot.arrives(id).is_none_or(|a| a <= r))
}

fn load_tables(mut commands: Commands, catalog: Res<Catalog>) {
    let path = crate::asset_root_path().join("data/loot.ron");
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| BUILTIN_LOOT.to_owned());
    let tables = LootTables::from_ron(&text, &catalog).unwrap_or_else(|e| {
        error!("loot.ron: {e}");
        LootTables::from_ron(BUILTIN_LOOT, &catalog).expect("built-in loot parses")
    });
    commands.insert_resource(Loot(std::sync::Arc::new(tables), None));
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
    raid: Option<u32>,
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
            .flat_map(|_| {
                loot.0
                    .roll_at(table, Some(&valley), raid, catalog, &mut rng)
            })
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
    raid: Option<u32>,
) {
    let mut rng = Rng::new(seed ^ 0xB11D);
    let mut n = 0;
    let notes = notes_lying(zone);
    let valley = extraction_core::loot::valley_key(&zone.map.name);
    for spot in &spots.0 {
        let rolls = rolls_for(zone.map.richness_at([spot.at.x, spot.at.z]), &mut rng);
        let drops: Vec<_> = (0..rolls)
            .flat_map(|_| {
                loot.0
                    .roll_at(spot.table, Some(&valley), raid, catalog, &mut rng)
            })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// The kinds lying in the zone, and how many of each.
    fn kinds(app: &mut App) -> BTreeMap<String, u32> {
        let cat = app.world().resource::<Catalog>().clone();
        let mut out = BTreeMap::new();
        let mut q = app.world_mut().query::<&Item>();
        for it in q.iter(app.world()) {
            *out.entry(cat.0.get(it.kind).id.clone()).or_default() += 1;
        }
        out
    }

    /// Marrow on career raid `r`: everything rolled as a raid starts
    /// (`roll_zone_loot`: markers, scenes, buildings, tapes, artifacts),
    /// with nothing else lying about.
    pub(crate) fn rolled_on(app: &mut App, r: u32) -> BTreeMap<String, u32> {
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .raids_by_map = vec![("marrow_creek".into(), r)];
        let items: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<Item>>()
            .iter(app.world())
            .collect();
        for e in items {
            if let Ok(e) = app.world_mut().get_entity_mut(e) {
                e.despawn();
            }
        }
        roll_zone_loot(app.world_mut());
        app.update();
        kinds(app)
    }

    pub(crate) fn career_app() -> App {
        let save = std::env::temp_dir().join(format!("loot_career_{}.ron", std::process::id()));
        let _ = std::fs::remove_file(&save);
        let opts = crate::LaunchOptions {
            robots: false,
            map: "marrow_creek".into(),
            save: Some(save),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        app
    }

    /// Every source a raid's loot is rolled from waits for a kind's
    /// `from_raid`, not only the tables: the Farm barricade laid an AK
    /// magazine on a first raid, four raids before any AK (the designer's
    /// "too many item types at once").
    #[test]
    fn nothing_turns_up_before_its_raid() {
        let mut app = career_app();
        let loot = app.world().resource::<Loot>().0.clone();
        let mut early = Vec::new();
        for r in 0..6 {
            for id in rolled_on(&mut app, r).keys() {
                if !arrived(&loot, Some(r), id) {
                    early.push(format!("{id} on raid {r}, out from {:?}", loot.arrives(id)));
                }
            }
        }
        assert!(early.is_empty(), "{early:#?}");
    }

    /// Whatever else rolls a table (a robot's wreck, a cargo pod, a
    /// packed crate, the driver's cache) rolls for the career's raid
    /// too: on a first raid, every table only gives what's out.
    #[test]
    fn every_roll_follows_the_career() {
        let mut app = career_app();
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .raids_by_map = vec![("marrow_creek".into(), 0)];
        app.update();
        let loot = app.world().resource::<Loot>().clone();
        assert_eq!(loot.1, Some(0));
        let cat = app.world().resource::<Catalog>().clone();
        let mut rng = Rng::new(5);
        let mut early = std::collections::BTreeSet::new();
        let names: Vec<String> = loot.0.names().cloned().collect();
        for t in &names {
            for _ in 0..40 {
                for d in loot.roll(t, &cat.0, &mut rng) {
                    let id = &cat.0.get(d.kind).id;
                    if !arrived(&loot.0, Some(0), id) {
                        early.insert(format!("{t}: {id}"));
                    }
                }
            }
        }
        assert!(early.is_empty(), "{early:#?}");
    }
}
