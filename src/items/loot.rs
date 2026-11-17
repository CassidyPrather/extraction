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

/// Found lying about (rolled loot, a wreck's, a pod's, the driver's
/// cache): a gun found so comes up dirty for its wear (`found_dirt`), so
/// the bench and the cleaning kit are plainly worth it (Gunsmith). Not the
/// kit, a gift, the armoury, or a sealed crate's: those come clean.
#[derive(Component)]
pub struct Found;

/// What a found gun of `id` in `condition` carries: (grime, fouling).
/// Nothing above condition 0.6, where rust goes plain and the action drags
/// (so it looks dirty when it handles dirty), and never so much that a
/// found gun jams past about 10% before it's cleaned (Weapon feel's
/// limits: 0.06 a grime, 0.03 a fouling, on wear's 7.4% at 0.3). The PM,
/// a newcomer's own gun, rolled in raid-0 houses, comes clean.
pub fn found_dirt(id: &str, condition: f32) -> (f32, f32) {
    if id == "pm_pistol" {
        return (0.0, 0.0);
    }
    let dirt = ((0.6 - condition) / 0.3).clamp(0.0, 1.0);
    (0.28 * dirt, 0.28 * dirt)
}

/// The most a found gun jams before it's cleaned (Weapon feel).
pub const FOUND_JAMS: f32 = 0.10;

/// How much of `found_dirt` a gun can take and stay under [`FOUND_JAMS`]
/// (0..1): a very worn one jams near it from wear alone, and takes less.
pub fn jam_room(gun: &extraction_core::firearm::Firearm) -> f32 {
    let mut g = gun.clone();
    g.grime = 0.0;
    g.fouling = 0.0;
    let clean = g.malfunction_chance();
    let (grime, fouling) = found_dirt("", g.condition);
    if grime + fouling <= 0.0 {
        return 1.0;
    }
    g.grime = grime;
    g.fouling = fouling;
    let per = g.malfunction_chance() - clean;
    if per <= 0.0 {
        return 1.0;
    }
    ((FOUND_JAMS - clean).max(0.0) / per).min(1.0)
}

/// `spawn_item`, for something found lying about ([`Found`]).
pub fn spawn_found(commands: &mut Commands, catalog: &Catalog, item: SpawnItem) -> Entity {
    let e = spawn_item(commands, catalog, item);
    commands.entity(e).insert(Found);
    e
}

/// A found gun's dirt, once it's a gun (`weapons::setup_gun_items`).
fn dirty_found_guns(
    mut commands: Commands,
    mut guns: Query<(Entity, &Item, Option<&mut crate::weapons::Gun>), With<Found>>,
    catalog: Res<Catalog>,
) {
    for (e, item, gun) in &mut guns {
        let def = catalog.get(item.kind());
        if def.firearm.is_none() {
            commands.entity(e).remove::<Found>();
            continue;
        }
        let Some(mut gun) = gun else {
            continue;
        };
        let (grime, fouling) = found_dirt(&def.id, gun.firearm.condition);
        let scale = jam_room(&gun.firearm);
        gun.firearm.grime = gun.firearm.grime.max(grime * scale);
        gun.firearm.fouling = gun.firearm.fouling.max(fouling * scale);
        commands.entity(e).remove::<Found>();
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
        .add_systems(PreUpdate, follow_the_career)
        .add_systems(Update, dirty_found_guns);
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
                    #[cfg(test)]
                    let cluster = world.get_resource::<OldPiles>().is_none();
                    #[cfg(not(test))]
                    let cluster = true;
                    spawn_building_loot(
                        &mut commands,
                        zone,
                        spots,
                        loot,
                        catalog,
                        seed,
                        raid,
                        cluster,
                    );
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
        // Its own stream (`place_key`): a marker added elsewhere changes
        // nothing here.
        let mut rng = Rng::new(seed ^ 0x100 ^ place_key(Vec3::new(m.at[0], m.y, m.at[1])));
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
            let e = spawn_found(
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

/// Where a first raid's small thing lies on its spot, the `i`th laid
/// there: a tidy cluster, a hand's width apart, all on the board. In a
/// pile (`building_roll`) the 9x18 box, matches, bolt and cell climbed on
/// each other and read as one muddy heap.
pub fn cluster_roll(i: usize) -> Vec3 {
    const SLOTS: [(f32, f32); 4] = [(-0.5, -0.5), (0.5, 0.5), (0.5, -0.5), (-0.5, 0.5)];
    let (x, z) = SLOTS[i % SLOTS.len()];
    Vec3::new(
        x * CLUSTER_GAP,
        0.04 + (i / SLOTS.len()) as f32 * 0.06,
        z * CLUSTER_GAP,
    )
}

/// Where a first raid's junk lies on its spot, the `i`th there (#104):
/// round the real things' cluster, a ring clear of it, so set dressing
/// never sits in or on what's worth taking.
pub fn dressing_roll(i: usize) -> Vec3 {
    const AROUND: usize = 6;
    let a = (i % AROUND) as f32 / AROUND as f32 * std::f32::consts::TAU + 0.3;
    let r = DRESSING_RING + (i / AROUND) as f32 * 0.07;
    Vec3::new(a.cos() * r, 0.04, a.sin() * r)
}

/// How far round a first raid's cluster its junk lies (m, from the spot).
pub const DRESSING_RING: f32 = 0.2;

/// Between neighbours in a first raid's cluster (m, centre to centre).
pub const CLUSTER_GAP: f32 = 0.12;

/// Lays a first raid's small things in the old climbing pile, for a test
/// to compare against the cluster.
#[cfg(test)]
#[derive(Resource)]
pub(crate) struct OldPiles;

/// The building loot spot a thing was laid at (by index), for a test: a
/// tall pile's top is nearer the board above than its own.
#[cfg(test)]
#[derive(Component)]
pub(crate) struct LaidAt(pub usize);

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
/// A career's first raids put a small thing where it's seen (Models'
/// shots of Marrow's first houses: the 9x18 box, matches, bolt and cell
/// were specks on a low shelf in drizzle light): from waist to eye height,
/// a tiny one on a flat top (or the high shelf), in the same building.
pub const PLACED_RAIDS: u32 = 2;

/// From waist to eye height (m above the floor).
pub const SEEN_FROM: f32 = 0.75;
pub const SEEN_TO: f32 = 1.6;

/// Whether a thing of `size` sits well at `spot` for a newcomer.
pub fn sits_well(
    spot: &crate::world::buildings::LootSpot,
    size: extraction_core::items::SizeClass,
) -> bool {
    use extraction_core::items::SizeClass;
    let Some(lift) = spot.lift else {
        return !matches!(size, SizeClass::Tiny | SizeClass::Small);
    };
    let seen = (SEEN_FROM..=SEEN_TO).contains(&lift);
    match size {
        // A shelf under another board is shaded and edge-on; the high
        // one's near the eye.
        SizeClass::Tiny => seen && (spot.top || lift >= 1.0),
        SizeClass::Small => seen,
        _ => true,
    }
}

/// The most things a first raid moves onto one spot: a pile any higher
/// hides what's behind it, the thing the move is for.
pub const MOVED_PILE: usize = 4;

/// A spot in `building` a thing of `size` sits well at and that isn't
/// piled up yet: flat tops first, the emptiest of them.
fn better_spot(
    spots: &[crate::world::buildings::LootSpot],
    building: usize,
    size: extraction_core::items::SizeClass,
    piled: &std::collections::HashMap<usize, usize>,
    rng: &mut Rng,
) -> Option<usize> {
    let pile = |k: usize| piled.get(&k).copied().unwrap_or(0);
    let good: Vec<usize> = (0..spots.len())
        .filter(|&k| {
            spots[k].building == building && sits_well(&spots[k], size) && pile(k) < MOVED_PILE
        })
        .collect();
    let tops: Vec<usize> = good.iter().copied().filter(|&k| spots[k].top).collect();
    let pick = if tops.is_empty() { good } else { tops };
    let least = pick.iter().map(|&k| pile(k)).min()?;
    let pick: Vec<usize> = pick.into_iter().filter(|&k| pile(k) == least).collect();
    Some(pick[rng.below(pick.len())])
}

/// A place's own stream key: its position to the centimetre. A spot's or
/// marker's draws come from its own stream, so a prop placed (or taken
/// away) elsewhere in the valley doesn't change what's rolled here: from
/// one stream walked in spot order, Map design's jeep on Marrow (its spots
/// new in the list) reshuffled every spot's roll after it, the bot went
/// somewhere new and second_hour turned red (7 November).
pub(crate) fn place_key(at: Vec3) -> u64 {
    let q = |v: f32| (v * 100.0).round() as i64 as u64;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in [q(at.x), q(at.y), q(at.z)] {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

#[allow(clippy::too_many_arguments)]
fn spawn_building_loot(
    commands: &mut Commands,
    zone: &Zone,
    spots: &crate::world::buildings::BuildingLoot,
    loot: &Loot,
    catalog: &Catalog,
    seed: u64,
    raid: Option<u32>,
    cluster: bool,
) {
    let mut n = 0;
    let notes = notes_lying(zone);
    let valley = extraction_core::loot::valley_key(&zone.map.name);
    let placed = raid.is_some_and(|r| r < PLACED_RAIDS);
    // How many things each spot has (a pile climbs: `building_roll`).
    let mut piled: std::collections::HashMap<usize, usize> = Default::default();
    // A first raid's small things that sit badly, moved once every spot's
    // own roll is in (so a spot's pile counts what's rolled there too).
    let mut moving = Vec::new();
    // A first raid's junk on each spot, laid round its cluster.
    let mut dressed: std::collections::HashMap<usize, usize> = Default::default();
    let mut real: std::collections::HashMap<usize, usize> = Default::default();
    let mut lay = |to: usize,
                   drop: extraction_core::loot::Drop,
                   piled: &mut std::collections::HashMap<usize, usize>,
                   rng: &mut Rng| {
        // On a first raid a spot's junk (#104) lies round its cluster, on
        // a ring of its own, and isn't counted in it: the real things'
        // cluster is laid as if it weren't there.
        let junk = catalog.get(drop.kind).category == extraction_core::items::Category::Junk;
        // (Every thing counts toward a spot's load, which decides where a
        // badly sitting one moves to; only its slot leaves junk out.)
        let pile = piled.entry(to).or_default();
        *pile += 1;
        let i = if cluster && placed {
            let n = if junk {
                dressed.entry(to).or_default()
            } else {
                real.entry(to).or_default()
            };
            *n += 1;
            *n - 1
        } else {
            *pile - 1
        };
        // On a first raid a small thing joins its spot's cluster; the rest
        // (and every later raid's) pile as rolled.
        let small = matches!(
            catalog.get(drop.kind).size,
            extraction_core::items::SizeClass::Tiny | extraction_core::items::SizeClass::Small
        );
        let off = if cluster && placed && junk {
            dressing_roll(i)
        } else if cluster && placed && small {
            cluster_roll(i)
        } else {
            building_roll(i)
        };
        let at = clear_of_notes(spots.0[to].at + off, &notes);
        let _e = spawn_found(
            commands,
            catalog,
            SpawnItem {
                kind: drop.kind,
                transform: Transform::from_translation(at)
                    .with_rotation(Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU))),
                condition: drop.condition,
            },
        );
        #[cfg(test)]
        commands.entity(_e).insert(LaidAt(to));
    };
    for (j, spot) in spots.0.iter().enumerate() {
        // This spot's own stream (`place_key`).
        let mut rng = Rng::new(seed ^ 0xB11D ^ place_key(spot.at));
        let rolls = rolls_for(zone.map.richness_at([spot.at.x, spot.at.z]), &mut rng);
        let drops: Vec<_> = (0..rolls)
            .flat_map(|_| {
                loot.0
                    .roll_at(spot.table, Some(&valley), raid, catalog, &mut rng)
            })
            .collect();
        for (k, drop) in drops.into_iter().enumerate() {
            n += 1;
            if placed && !sits_well(spot, catalog.get(drop.kind).size) {
                moving.push((j, k, drop));
            } else {
                lay(j, drop, &mut piled, &mut rng);
            }
        }
    }
    // A moved thing's own stream too: its spot's, and which of the spot's
    // drops it was (not how many other spots moved theirs first).
    for (j, k, drop) in moving {
        let mut rng = Rng::new(
            seed ^ 0xB11E
                ^ place_key(spots.0[j].at)
                ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15),
        );
        let size = catalog.get(drop.kind).size;
        let to = better_spot(&spots.0, spots.0[j].building, size, &piled, &mut rng).unwrap_or(j);
        lay(to, drop, &mut piled, &mut rng);
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
/// Whether a session gets the dev cache: only one that doesn't save, and
/// not the yard (it has its own kit, in the crate) nor the arcade (it lays
/// out its own sweep).
fn wants_dev_cache(saves: bool, opts: &crate::launch::LaunchOptions) -> bool {
    !saves
        && opts.map != crate::camp::TRAINING_YARD
        && opts.scene != crate::launch::SceneChoice::Arcade
}

fn spawn_dev_cache(
    mut commands: Commands,
    zone: Res<Zone>,
    catalog: Res<Catalog>,
    slot: Res<crate::persist::SaveSlot>,
    opts: Res<crate::launch::LaunchOptions>,
) {
    if !wants_dev_cache(slot.0.is_some(), &opts) {
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
pub(crate) mod tests {
    /// The dev cache is for a session that doesn't save, in a valley: not
    /// the yard, not the arcade.
    #[test]
    fn the_dev_cache_is_not_in_the_yard_nor_the_arcade() {
        use crate::launch::{LaunchOptions, SceneChoice};
        let raid = LaunchOptions::default();
        assert!(super::wants_dev_cache(false, &raid));
        assert!(!super::wants_dev_cache(true, &raid));
        let yard = LaunchOptions {
            map: crate::camp::TRAINING_YARD.into(),
            ..Default::default()
        };
        assert!(!super::wants_dev_cache(false, &yard));
        let arcade = LaunchOptions {
            scene: SceneChoice::Arcade,
            ..Default::default()
        };
        assert!(!super::wants_dev_cache(false, &arcade));
    }

    use super::*;

    /// A prop placed in a valley doesn't change what's rolled anywhere
    /// else in it: Marrow's building loot rolled as it is, and again with a
    /// new building's spots put in the middle of the list (a jeep wreck
    /// placed, as Map design did), every one of the valley's own spots has
    /// the same things, byte for byte. From one stream walked in spot order
    /// the new spots took draws, and every spot after them rolled anew.
    #[test]
    fn a_prop_placed_rolls_nothing_new_elsewhere() {
        let opts = LaunchOptions {
            robots: false,
            map: "marrow_creek".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let spots: Vec<crate::world::buildings::LootSpot> = app
            .world()
            .resource::<crate::world::buildings::BuildingLoot>()
            .0
            .clone();
        assert!(spots.len() > 50, "Marrow's spots: {}", spots.len());
        // The new building's spots: six of the valley's own, moved a
        // kilometre off (their things are told apart by where they land).
        let far = Vec3::new(1000.0, 0.0, 0.0);
        let new: Vec<_> = spots
            .iter()
            .step_by(spots.len() / 6)
            .take(6)
            .cloned()
            .enumerate()
            .map(|(i, mut s)| {
                s.at += far;
                // A building of its own, as a new prop is.
                s.building = 100_000 + i;
                s
            })
            .collect();
        let mut with = spots.clone();
        let mid = with.len() / 2;
        with.splice(mid..mid, new);
        let roll = |app: &mut App, list: Vec<crate::world::buildings::LootSpot>| -> Vec<String> {
            let world = app.world_mut();
            let before: std::collections::BTreeSet<Entity> = world
                .query_filtered::<Entity, With<Item>>()
                .iter(world)
                .collect();
            let opts = world.resource::<LaunchOptions>().clone();
            let seed = cycle_seed(&opts, 0);
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let zone = world.resource::<Zone>();
                let loot = world.resource::<Loot>();
                let catalog = world.resource::<Catalog>();
                let mut commands = Commands::new_from_entities(
                    &mut queue,
                    world.entity_allocator(),
                    world.entities(),
                );
                let spots = crate::world::buildings::BuildingLoot(list);
                spawn_building_loot(
                    &mut commands,
                    zone,
                    &spots,
                    loot,
                    catalog,
                    seed,
                    Some(3),
                    true,
                );
            }
            queue.apply(world);
            let catalog = world.resource::<Catalog>().clone();
            let mut rows: Vec<(Entity, f32, String)> = world
                .query::<(Entity, &Item, &Transform)>()
                .iter(world)
                .filter(|(e, ..)| !before.contains(e))
                .map(|(e, it, t)| {
                    (
                        e,
                        t.translation.x,
                        format!(
                            "{} {:.3?} {:.3?} {:.3}",
                            catalog.get(it.0.kind).id,
                            t.translation,
                            t.rotation,
                            it.0.condition
                        ),
                    )
                })
                .collect();
            for (e, ..) in &rows {
                world.despawn(*e);
            }
            // The far spots' things (a kilometre off, the valley being
            // some 400 m across) left out.
            let mut out: Vec<String> = rows
                .drain(..)
                .filter(|(_, x, _)| *x < 600.0)
                .map(|(.., r)| r)
                .collect();
            out.sort();
            out
        };
        let alone = roll(&mut app, spots);
        let placed = roll(&mut app, with);
        let lost: Vec<_> = alone.iter().filter(|r| !placed.contains(r)).collect();
        let gained: Vec<_> = placed.iter().filter(|r| !alone.contains(r)).collect();
        assert!(
            lost.is_empty() && gained.is_empty(),
            "{} of the valley's {} things rolled differently with a prop placed:\n- {}\n+ {}",
            lost.len(),
            alone.len(),
            lost.iter()
                .take(5)
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n- "),
            gained
                .iter()
                .take(5)
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n+ ")
        );
    }
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
        rolled_on_map(app, "marrow_creek", r)
    }

    /// [`rolled_on`] in valley `map` (the app's own).
    fn rolled_on_map(app: &mut App, map: &str, r: u32) -> BTreeMap<String, u32> {
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .raids_by_map = vec![(map.into(), r)];
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

    /// Junk dresses a place on a stream of its own (#104): with its
    /// dressing, a place's real roll is the same thing for thing, and the
    /// stream it was rolled from is left as it was; the real roll never
    /// yields junk, and the dressing yields nothing else.
    #[test]
    fn junk_dresses_a_place_without_touching_its_real_roll() {
        use extraction_core::items::Category;
        let mut app = career_app();
        app.update();
        let loot = app.world().resource::<Loot>().clone();
        let cat = app.world().resource::<Catalog>().clone();
        let mut junk = 0;
        for raid in [Some(0), Some(8), None] {
            for name in ["kitchen", "house", "food_common"] {
                for seed in 0..300 {
                    let (mut a, mut b) = (Rng::new(seed), Rng::new(seed));
                    let all = loot.0.roll_at(name, None, raid, &cat.0, &mut a);
                    let real = loot.0.roll_real(name, None, raid, &cat.0, &mut b);
                    let kept: Vec<_> = all
                        .iter()
                        .filter(|d| cat.get(d.kind).category != Category::Junk)
                        .copied()
                        .collect();
                    assert_eq!(kept, real, "{name} {raid:?} seed {seed}");
                    assert_eq!(
                        a.next_u64(),
                        b.next_u64(),
                        "{name}: the stream is as it was"
                    );
                    assert!(
                        real.iter()
                            .all(|d| cat.get(d.kind).category != Category::Junk)
                    );
                    junk += all.len() - kept.len();
                }
            }
        }
        assert!(junk > 0, "the dressing dresses");
        // The dressing tables hold junk and nothing else.
        for t in ["junk_pantry", "junk_kitchen", "junk_house"] {
            let mut rng = Rng::new(9);
            for _ in 0..200 {
                for d in loot.0.roll_real(t, None, None, &cat.0, &mut rng.clone()) {
                    panic!("{t} gave a real {}", cat.get(d.kind).id);
                }
                rng.next_u64();
            }
        }
    }

    /// Cooking isn't shown early (#104, the user: "I do not want it to be
    /// presented early on"): no table rolls a cooking stage, and neither the
    /// yard nor a valley puts up a kitchen station.
    #[test]
    fn nothing_early_shows_cooking() {
        let app = career_app();
        let loot = app.world().resource::<Loot>().clone();
        let cat = app.world().resource::<Catalog>().clone();
        for (_, def) in cat.0.iter() {
            if def.id.starts_with("cook_") {
                assert_eq!(loot.0.arrives(&def.id), None, "{} is rolled", def.id);
            }
        }
        for map in [crate::camp::TRAINING_YARD, "marrow_creek"] {
            let mut app = crate::headless_app(
                crate::LaunchOptions {
                    robots: false,
                    map: map.into(),
                    ..Default::default()
                },
                false,
            );
            app.finish();
            app.cleanup();
            for _ in 0..60 {
                app.update();
            }
            let mut q = app.world_mut().query::<&crate::raid::kitchen::Cook>();
            assert_eq!(q.iter(app.world()).count(), 0, "{map}: a kitchen station");
            let mut q = app.world_mut().query::<&Item>();
            for i in q.iter(app.world()) {
                let id = &cat.get(i.kind()).id;
                assert!(!id.starts_with("cook_"), "{map}: {id} lies about");
            }
        }
    }

    /// `cargo test -- --ignored item_census --nocapture`: every item in
    /// each valley once it's settled, by kind, junk (food and the like:
    /// its `food:` tag, or the Junk category) apart from the rest (#104).
    #[test]
    #[ignore]
    fn item_census() {
        for map in [
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "sallow_reservoir",
            "rime_fell",
        ] {
            let dir = std::env::temp_dir().join(format!("census_{map}_{}", std::process::id()));
            let opts = crate::LaunchOptions {
                robots: false,
                map: map.into(),
                data_dir: Some(dir.clone()),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..120 {
                app.update();
            }
            let cat = app.world().resource::<Catalog>().clone();
            let raid = app.world().resource::<Loot>().1;
            let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
            let mut q = app.world_mut().query::<&Item>();
            for i in q.iter(app.world()) {
                *kinds.entry(cat.get(i.kind()).id.clone()).or_default() += 1;
            }
            let junk = |id: &str| {
                let d = cat.get(cat.kind(id).unwrap());
                d.tags.iter().any(|t| t.starts_with("food:"))
                    || id.starts_with("grenade_pin")
                    || id.starts_with("grenade_spoon")
            };
            let (j, r): (Vec<_>, Vec<_>) = kinds.iter().partition(|(k, _)| junk(k));
            let sum = |v: &[(&String, &usize)]| v.iter().map(|(_, n)| **n).sum::<usize>();
            println!(
                "census {map} (raid {raid:?}): real {} in {} kinds, junk {} in {} kinds",
                sum(&r),
                r.len(),
                sum(&j),
                j.len()
            );
            println!("  real: {r:?}");
            println!("  junk: {j:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    pub(crate) fn career_app() -> App {
        career_app_on("marrow_creek")
    }

    fn career_app_on(map: &str) -> App {
        let save =
            std::env::temp_dir().join(format!("loot_career_{map}_{}.ron", std::process::id()));
        let _ = std::fs::remove_file(&save);
        let opts = crate::LaunchOptions {
            robots: false,
            map: map.into(),
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
            let rolled = rolled_on(&mut app, r);
            // Hana's tape is there from the first, by her letter.
            assert!(
                rolled.contains_key("tape_practice"),
                "raid {r}: no tape_practice"
            );
            for id in rolled.keys() {
                // A laid story beat (Radio's, by Story's ask).
                if crate::radio::places::FROM_THE_START.contains(&id.as_str()) {
                    continue;
                }
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

    /// On a career's first raids a small thing lies where it's seen: from
    /// waist to eye height, a tiny one on a flat top or the high shelf
    /// (Models' shots of Marrow's first houses: the 9x18 box, matches,
    /// bolt and cell were specks on a low shelf), wherever its building has
    /// such a spot. Later raids leave loot where it's rolled.
    #[test]
    fn a_first_raids_small_things_lie_in_sight() {
        use extraction_core::items::SizeClass;
        let mut app = career_app();
        let spots = app
            .world()
            .resource::<crate::world::buildings::BuildingLoot>()
            .0
            .clone();
        let cat = app.world().resource::<Catalog>().clone();
        for r in 0..PLACED_RAIDS {
            rolled_on(&mut app, r);
            // Each small thing in a building, by the spot it was laid at
            // (piles climb a little), and how many things each spot has.
            let mut laid = Vec::new();
            let mut per = vec![0usize; spots.len()];
            let mut q = app.world_mut().query::<(&Item, &GlobalTransform)>();
            for (item, gt) in q.iter(app.world()) {
                let at = gt.translation();
                let Some((k, _)) = spots
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| {
                        s.at.xz().distance(at.xz()) < 0.4 && (at.y - s.at.y).abs() < 1.0
                    })
                    .min_by(|a, b| a.1.at.distance(at).total_cmp(&b.1.at.distance(at)))
                else {
                    continue;
                };
                per[k] += 1;
                let def = cat.0.get(item.kind);
                if matches!(def.size, SizeClass::Tiny | SizeClass::Small) {
                    laid.push((def.id.clone(), def.size, k));
                }
            }
            // Out of sight only where every spot in sight in its building
            // is piled up already.
            let room = |b: usize, size: SizeClass| {
                (0..spots.len()).any(|k| {
                    spots[k].building == b && sits_well(&spots[k], size) && per[k] < MOVED_PILE
                })
            };
            let badly: Vec<String> = laid
                .iter()
                .filter(|(_, size, k)| {
                    !sits_well(&spots[*k], *size) && room(spots[*k].building, *size)
                })
                .map(|(id, _, k)| format!("{id} at {:?} (lift {:?})", spots[*k].at, spots[*k].lift))
                .collect();
            assert!(
                laid.len() > 10,
                "raid {r}: only {} small things in buildings",
                laid.len()
            );
            assert!(
                badly.is_empty(),
                "raid {r}: {} of {} small things out of sight: {badly:#?}",
                badly.len(),
                laid.len()
            );
        }
    }

    /// What lies at each building loot spot: its kinds and how many of
    /// each, and where its small things lie.
    #[allow(clippy::type_complexity)]
    fn by_spot(
        app: &mut App,
        spots: &[crate::world::buildings::LootSpot],
    ) -> (Vec<BTreeMap<String, u32>>, Vec<Vec<Vec3>>) {
        use extraction_core::items::SizeClass;
        let cat = app.world().resource::<Catalog>().clone();
        let mut kinds = vec![BTreeMap::new(); spots.len()];
        let mut small = vec![Vec::new(); spots.len()];
        let mut q = app
            .world_mut()
            .query::<(&Item, &GlobalTransform, &LaidAt)>();
        for (item, gt, laid) in q.iter(app.world()) {
            let (at, k) = (gt.translation(), laid.0);
            let def = cat.0.get(item.kind);
            *kinds[k].entry(def.id.clone()).or_default() += 1;
            // (Junk lies round the cluster, on a ring of its own.)
            if matches!(def.size, SizeClass::Tiny | SizeClass::Small)
                && def.category != extraction_core::items::Category::Junk
            {
                small[k].push(at);
            }
        }
        (kinds, small)
    }

    /// A first raid's small things lie as a tidy cluster on their spot,
    /// not a climbing pile (Progression: the 9x18 box, matches, bolt and
    /// cell read as one heap), and only where they lie changes: in
    /// Marrow's and Kettle's first two raids every spot holds the same
    /// kinds, as many of each, as in the pile. A spot's small things lie
    /// side by side on the board, a hand's width apart. The raid after
    /// is laid as before.
    #[test]
    fn a_first_raids_small_things_lie_as_a_cluster() {
        for map in ["marrow_creek", "kettle_pass"] {
            let mut app = career_app_on(map);
            let spots = app
                .world()
                .resource::<crate::world::buildings::BuildingLoot>()
                .0
                .clone();
            let mut clusters = 0;
            for r in 0..=PLACED_RAIDS {
                app.world_mut().insert_resource(OldPiles);
                rolled_on_map(&mut app, map, r);
                let (pile, pile_at) = by_spot(&mut app, &spots);
                app.world_mut().remove_resource::<OldPiles>();
                rolled_on_map(&mut app, map, r);
                let (laid, laid_at) = by_spot(&mut app, &spots);
                for k in 0..spots.len() {
                    assert_eq!(pile[k], laid[k], "{map} raid {r}: spot {:?}", spots[k].at);
                }
                if r == PLACED_RAIDS {
                    assert_eq!(pile_at, laid_at, "{map} raid {r}: laid otherwise");
                    continue;
                }
                for (k, at) in laid_at.iter().enumerate() {
                    if at.len() < 2 || at.len() > 4 {
                        continue;
                    }
                    clusters += 1;
                    for (a, p) in at.iter().enumerate() {
                        for q in &at[a + 1..] {
                            assert!(
                                (p.y - q.y).abs() < 0.02,
                                "{map} raid {r}: spot {:?}: stacked {p} {q}",
                                spots[k].at
                            );
                            let d = p.xz().distance(q.xz());
                            assert!(
                                d > CLUSTER_GAP * 0.9 && d < CLUSTER_GAP * 1.5,
                                "{map} raid {r}: spot {:?}: {d:.3} m apart",
                                spots[k].at
                            );
                        }
                    }
                }
            }
            assert!(clusters > 5, "{map}: only {clusters} clusters");
        }
    }

    /// A gun found lying about comes up dirty for its wear, so cleaning is
    /// plainly worth it (Gunsmith), within Weapon feel's limits: nothing
    /// above condition 0.6, the PM clean, and no found gun jamming past
    /// FOUND_JAMS before it's cleaned. A gun that isn't found (the kit's,
    /// a gift) is untouched.
    #[test]
    fn a_found_gun_comes_up_dirty_for_its_wear() {
        let mut app = crate::raid::requests::tests::app();
        let catalog = app.world().resource::<Catalog>().clone();
        let lay = |app: &mut App, id: &str, condition: f32, found: bool| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut c = Commands::new(&mut queue, app.world());
                let item = SpawnItem {
                    kind: catalog.kind(id).unwrap(),
                    transform: Transform::from_xyz(0.0, 50.0, 0.0),
                    condition,
                };
                if found {
                    spawn_found(&mut c, &catalog, item)
                } else {
                    spawn_item(&mut c, &catalog, item)
                }
            };
            queue.apply(app.world_mut());
            for _ in 0..3 {
                app.update();
            }
            let g = &app.world().get::<crate::weapons::Gun>(e).unwrap().firearm;
            (g.grime, g.fouling, g.malfunction_chance())
        };
        for id in [
            "toz8",
            "toz_double",
            "mp133",
            "sks",
            "mosin",
            "ak74",
            "ppsh",
            "svd",
        ] {
            for c in [0.25, 0.3, 0.45] {
                let (grime, fouling, jams) = lay(&mut app, id, c, true);
                assert!(grime > 0.0 && fouling > 0.0, "{id} at {c}: clean");
                let wear_only = lay(&mut app, id, c, false).2;
                assert!(
                    jams <= FOUND_JAMS.max(wear_only) + 1e-4,
                    "{id} at {c}: jams {jams:.3}"
                );
            }
            let (grime, fouling, _) = lay(&mut app, id, 0.7, true);
            assert_eq!((grime, fouling), (0.0, 0.0), "{id} at 0.7");
            let (grime, fouling, _) = lay(&mut app, id, 0.3, false);
            assert_eq!((grime, fouling), (0.0, 0.0), "{id}, not found");
        }
        let (grime, fouling, _) = lay(&mut app, "pm_pistol", 0.3, true);
        assert_eq!((grime, fouling), (0.0, 0.0), "the PM comes clean");
    }
}
