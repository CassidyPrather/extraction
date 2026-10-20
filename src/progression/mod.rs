//! The long loop in the game: the driver's jobs and what the driver fits
//! to the truck for them. Rules (tiers, upgrades, rolling jobs) are in
//! [`extraction_core::progression`]; `docs/PROGRESSION.md` is the design.
//!
//! - A job is always on the card (`raid/requests.rs` shows it and takes
//!   it in). A Recover job's recorder is put in its building here, a Scout
//!   job's place is noticed here.
//! - What's fitted takes effect here: the stash's room, the bench's
//!   recipes, the aerial's early warning, the cabinet's dressing. Each
//!   fitting is a prop in the truck bed (`TruckUpgradeProp`, dressed by
//!   `render::upgrades`).
//! - The career page on the logbook (`career_page`).

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MapDef;
use extraction_core::progression::{Job, Progress, RECOVER_ITEM, SCOUT_RADIUS, Upgrade, Valley};

use crate::anomaly::artifacts::CarriedBy;
use crate::interact::body::Backpack;
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::launch::LaunchOptions;
use crate::raid::truck::{FLOOR, Stash, Truck};
use crate::raid::{Phase, Raid};
use crate::rig::LocalPlayer;
use crate::world::{Layer, Zone};

/// The career's jobs and fittings (saved with the career).
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Progression(pub Progress);

/// A job was handed in at the card (`raid/requests.rs`).
#[derive(Message, Clone, Copy, Debug)]
pub struct JobDone;

/// One fitting in the truck bed, a child of the truck (truck space).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TruckUpgradeProp(pub Upgrade);

/// The aerial warns of the Surge this many seconds out.
pub const AERIAL_WARNING: f32 = 10.0 * 60.0;
/// The cabinet holds at most this many dressings.
const CABINET_HOLDS: usize = 2;
/// The cabinet's floor, truck space (on the front wall), where its
/// dressing is left.
pub const CABINET_AT: Vec3 = Vec3::new(0.6, FLOOR + 0.74, -1.70);
/// From the cabinet's floor up to its centre (the model's origin).
const CABINET_SHELF_BELOW: Vec3 = Vec3::new(0.0, 0.21, -0.04);

pub struct ProgressionPlugin;

impl Plugin for ProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Progression>()
            .add_message::<JobDone>()
            .add_systems(
                Update,
                (
                    count_raids,
                    job_done,
                    offer_job,
                    place_recorder,
                    scout,
                    aerial_warning,
                    fit_props,
                    fit_stash,
                    fit_bench,
                    stock_cabinet,
                )
                    .chain(),
            );
    }
}

/// The three valleys, as jobs see them.
pub fn valleys() -> &'static [(&'static str, MapDef)] {
    static V: std::sync::OnceLock<Vec<(&'static str, MapDef)>> = std::sync::OnceLock::new();
    V.get_or_init(|| {
        use extraction_core::map::{CINDER_WORKS, KETTLE_PASS, MARROW_CREEK};
        [
            ("marrow_creek", MARROW_CREEK),
            ("kettle_pass", KETTLE_PASS),
            ("cinder_works", CINDER_WORKS),
        ]
        .into_iter()
        .filter_map(|(id, text)| MapDef::from_ron(text).ok().map(|m| (id, m)))
        .collect()
    })
}

fn valley_name(id: &str) -> String {
    valleys()
        .iter()
        .find(|(v, _)| *v == id)
        .map_or(id.replace('_', " "), |(_, m)| m.name.clone())
}

/// The valley the jobs are about: the truck's, or from the training yard
/// the one it drives on to.
fn here(opts: &LaunchOptions) -> String {
    if opts.map == crate::camp::TRAINING_YARD {
        crate::persist::next_valley(opts)
    } else {
        opts.map.clone()
    }
}

/// The job as the card says it.
pub fn job_text(job: &Job, catalog: &Catalog, map: &str) -> String {
    let elsewhere = |m: &str| {
        if m == map {
            String::new()
        } else {
            format!("In {}: ", valley_name(m))
        }
    };
    match job {
        Job::Fetch { item, n } => {
            let name = catalog
                .kind(item)
                .map(|k| catalog.get(k).name.clone())
                .unwrap_or(item.clone());
            let them = if *n == 1 { "it" } else { "them" };
            format!(
                "{n} x {name}\n\"{}\"\nStash {them}, press here.",
                extraction_core::progression::why(item)
            )
        }
        Job::Recover {
            map: m,
            region,
            place,
            ..
        } => {
            let where_ = place
                .clone()
                .unwrap_or_else(|| format!("somewhere in the {region}."));
            format!(
                "{}\"A surveyor's recorder, left {where_} Bring it home.\"\nStash it, press here.",
                elsewhere(m)
            )
        }
        Job::Scout {
            map: m,
            place,
            seen,
            ..
        } => {
            if *seen {
                format!("The {place}: seen.\nGet home, press here.")
            } else {
                format!(
                    "{}\"Walk up to the {place} and have a look round. Come back and tell me.\"",
                    elsewhere(m)
                )
            }
        }
    }
}

fn count_raids(
    mut raid: ResMut<Raid>,
    opts: Res<LaunchOptions>,
    mut progression: ResMut<Progression>,
    catalog: Res<Catalog>,
    mut was_out: Local<bool>,
) {
    let out = raid.phase != Phase::AtBase;
    let set_off = out && !*was_out;
    let home = !out && *was_out;
    *was_out = out;
    if opts.map == crate::camp::TRAINING_YARD {
        return;
    }
    if set_off {
        progression.0.count_raid(&opts.map);
    }
    // Back from a raid with a fetch the Zone isn't giving up: something
    // else instead (rolled by `offer_job`).
    if home && progression.0.fetch_gone_stale(raid.raids) {
        if let Some(Job::Fetch { item, .. }) = progression.0.job.take() {
            let name = catalog
                .kind(&item)
                .map(|k| catalog.get(k).name.clone())
                .unwrap_or(item);
            raid.say(format!(
                "Driver: \"Forget the {name}. I'll get it elsewhere. New job on the card.\""
            ));
        }
        progression.0.swaps += 1;
    }
}

/// A job handed in: count it, and fit what a finished tier earns.
fn job_done(
    mut commands: Commands,
    mut done: MessageReader<JobDone>,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
    mut save: MessageWriter<crate::persist::SaveNow>,
) {
    for _ in done.read() {
        let fitted = progression.0.complete();
        for u in fitted {
            raid.say(format!(
                "Driver: \"And I've put up {}. Have a look.\"",
                u.name()
            ));
        }
        if !fitted.is_empty() {
            commands.write_message(crate::rig::haptics::StepDone {
                at: None,
                station: true,
            });
        }
        save.write(crate::persist::SaveNow);
    }
}

/// There's always a job on the card.
fn offer_job(opts: Res<LaunchOptions>, raid: Res<Raid>, mut progression: ResMut<Progression>) {
    if progression.0.job.is_some() {
        return;
    }
    let here = here(&opts);
    let valleys: Vec<Valley> = valleys()
        .iter()
        .map(|(id, map)| Valley { id, map })
        .collect();
    let job = extraction_core::progression::roll_job(&progression.0, opts.seed, &here, &valleys);
    info!("job {}: {job:?}", progression.0.done + 1);
    progression.0.job = Some(job);
    progression.0.job_raid = raid.raids;
}

/// A Recover job in this valley with no recorder anywhere (not yet put
/// out, swept by the Surge, taken by the convoy): the driver's contact
/// leaves one in a building in the job's region.
fn place_recorder(
    mut commands: Commands,
    mut progression: ResMut<Progression>,
    opts: Res<LaunchOptions>,
    zone: Res<Zone>,
    spots: Option<Res<crate::world::buildings::BuildingLoot>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut tried: Local<u32>,
) {
    let Some(Job::Recover {
        map,
        region,
        at,
        place,
    }) = &mut progression.0.job
    else {
        return;
    };
    if *map != opts.map {
        return;
    }
    let Some(kind) = catalog.kind(RECOVER_ITEM) else {
        return;
    };
    if items.iter().any(|i| i.kind() == kind) {
        return;
    }
    // Only so often: the world may still be loading.
    *tried += 1;
    if *tried % 30 != 1 {
        return;
    }
    let in_region = |p: Vec3| {
        zone.map
            .region_at([p.x, p.z])
            .is_some_and(|r| r.name == *region)
    };
    let mut candidates: Vec<Vec3> = spots
        .iter()
        .flat_map(|s| s.0.iter().map(|s| s.at))
        .filter(|p| in_region(*p))
        .collect();
    if candidates.is_empty() {
        // No building spots loaded: the region's point of interest.
        candidates = zone
            .map
            .markers
            .iter()
            .filter(|m| in_region(Vec3::new(m.at[0], 0.0, m.at[1])))
            .map(|m| zone.on_ground(m.at[0], m.at[1]) + Vec3::Y * 0.1)
            .collect();
    }
    if candidates.is_empty() {
        warn!("no place for the recorder in {region}");
        return;
    }
    // The same spot again if it was settled, else one by the job's number.
    let p = match at {
        Some(a) => candidates
            .iter()
            .copied()
            .min_by(|x, y| {
                x.xz()
                    .distance(Vec2::from_array(*a))
                    .total_cmp(&y.xz().distance(Vec2::from_array(*a)))
            })
            .unwrap(),
        None => {
            let n = progression_seed(&opts, candidates.len());
            candidates[n]
        }
    };
    spawn_item(
        &mut commands,
        &catalog,
        SpawnItem {
            kind,
            transform: Transform::from_translation(p + Vec3::Y * 0.06),
            condition: 0.8,
        },
    );
    *at = Some([p.x, p.z]);
    *place = Some(crate::raid::requests::describe_place(&zone, p));
    info!("recorder left at {p:?} ({region})");
}

fn progression_seed(opts: &LaunchOptions, n: usize) -> usize {
    let mut rng = extraction_core::noise::Rng::new(opts.seed ^ 0x5EC0_4DE5);
    rng.below(n.max(1))
}

/// A Scout job's place, reached: noted. Going down forgets it.
fn scout(
    mut commands: Commands,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
    opts: Res<LaunchOptions>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
) {
    let down = matches!(raid.phase, Phase::Down { .. });
    let out = raid.phase != Phase::AtBase;
    let Some(Job::Scout {
        map,
        place,
        at,
        seen,
    }) = &mut progression.0.job
    else {
        return;
    };
    if *seen && down {
        *seen = false;
        raid.say(format!("What you saw at the {place} goes with you."));
        return;
    }
    if *seen || !out || down || *map != opts.map {
        return;
    }
    let near = players
        .iter()
        .any(|p| p.translation().xz().distance(Vec2::from_array(*at)) < SCOUT_RADIUS);
    if near {
        *seen = true;
        raid.say(format!("The {place}: noted. Now get it home."));
        crate::rig::haptics::step_done(&mut commands, None);
    }
}

/// With the aerial up, the truck radios a Surge warning early, to anyone
/// carrying a radio.
fn aerial_warning(
    mut raid: ResMut<Raid>,
    progression: Res<Progression>,
    players: Query<Entity, With<LocalPlayer>>,
    radios: Query<(&Item, &CarriedBy)>,
    catalog: Res<Catalog>,
    mut last: Local<f32>,
) {
    let now = raid.surge_in();
    let before = std::mem::replace(&mut *last, now);
    if !progression.0.has(Upgrade::Aerial) || raid.phase == Phase::AtBase {
        return;
    }
    if !(before > AERIAL_WARNING && now <= AERIAL_WARNING) {
        return;
    }
    let carried = players.iter().any(|p| {
        radios
            .iter()
            .any(|(i, by)| by.0 == p && catalog.get(i.kind()).id == "radio")
    });
    if carried {
        raid.say("Truck: \"Aerial's picking up the Surge. Ten minutes, maybe less.\"");
    }
}

/// Every fitting is in the truck bed, once.
fn fit_props(
    mut commands: Commands,
    progression: Res<Progression>,
    trucks: Query<Entity, With<Truck>>,
    props: Query<&TruckUpgradeProp>,
) {
    let Some(truck) = trucks.iter().next() else {
        return;
    };
    for u in Upgrade::ALL {
        if !progression.0.has(u) || props.iter().any(|p| p.0 == u) {
            continue;
        }
        let at = prop_at(u);
        let mut e = commands.spawn((
            Name::new(format!("Fitting: {}", u.id())),
            TruckUpgradeProp(u),
            Transform::from_translation(at),
            Visibility::default(),
            ChildOf(truck),
        ));
        if u == Upgrade::MedCabinet {
            // The cabinet's shelf takes what's put on it.
            e.with_child((
                Collider::cuboid(0.38, 0.02, 0.12),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                Transform::from_translation(CABINET_AT - at - Vec3::Y * 0.01),
            ));
        }
    }
}

/// Where each fitting sits, truck space: its model's origin
/// (`art/props/truck_kit.py`, `models/props/<id>.glb`).
pub fn prop_at(u: Upgrade) -> Vec3 {
    match u {
        // The stash crate's centre: posts at its corners.
        Upgrade::StashRack => Vec3::new(-0.55, FLOOR + 0.65, -1.0),
        // On the left wall by the bench end, clear of the crate.
        Upgrade::StashRack2 => Vec3::new(-0.95, FLOOR + 0.45, 0.15),
        // On the bench top at its front end.
        Upgrade::BenchVice => Vec3::new(0.72, FLOOR + 0.8, -0.45),
        // On the cab roof, right rear corner.
        Upgrade::Aerial => Vec3::new(0.95, 2.6, -2.1),
        // On the front wall, clear of the memorial note and the bell pull.
        Upgrade::MedCabinet => CABINET_AT + CABINET_SHELF_BELOW,
    }
}

/// The stash crate's room follows the racks. A career saved with more in
/// it than the room (from before there were racks) keeps what it has.
fn fit_stash(
    progression: Res<Progression>,
    mut stashes: Query<&mut Backpack, With<Stash>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let want = progression.0.stash_capacity();
    for mut s in &mut stashes {
        let used: f32 = s
            .contents
            .iter()
            .filter_map(|e| items.get(*e).ok())
            .map(|i| catalog.get(i.kind()).bulk)
            .sum();
        let cap = want.max(used.ceil());
        if (s.model.capacity - cap).abs() > 0.01 && (progression.is_changed() || s.is_added()) {
            s.model.capacity = cap;
        }
    }
}

fn fit_bench(progression: Res<Progression>, mut benches: Query<&mut crate::raid::bench::Bench>) {
    let tier = progression.0.bench_tier();
    for mut b in &mut benches {
        if b.tier != tier {
            b.tier = tier;
        }
    }
}

/// Home from a raid with the cabinet fitted: the driver leaves a dressing
/// on its shelf (it holds two).
fn stock_cabinet(
    mut commands: Commands,
    raid: Res<Raid>,
    mut progression: ResMut<Progression>,
    trucks: Query<(&Truck, &GlobalTransform)>,
    items: Query<(&Item, &GlobalTransform)>,
    catalog: Res<Catalog>,
) {
    if raid.phase != Phase::AtBase
        || !progression.0.has(Upgrade::MedCabinet)
        || progression.0.cabinet_raid >= raid.raids.max(1)
    {
        return;
    }
    let Some((_, truck)) = trucks.iter().find(|(t, _)| t.at_base) else {
        return;
    };
    let Some(kind) = catalog.kind("field_dressing") else {
        return;
    };
    let shelf = truck.transform_point(CABINET_AT);
    let on_shelf = items
        .iter()
        .filter(|(i, gt)| i.kind() == kind && gt.translation().distance(shelf) < 0.3)
        .count();
    progression.0.cabinet_raid = raid.raids.max(1);
    if on_shelf >= CABINET_HOLDS {
        return;
    }
    let off = Vec3::new(-0.1 + 0.12 * on_shelf as f32, 0.04, 0.0);
    spawn_item(
        &mut commands,
        &catalog,
        SpawnItem {
            kind,
            transform: Transform::from_translation(truck.transform_point(CABINET_AT + off)),
            condition: 1.0,
        },
    );
}

/// The career page, for the logbook's second side.
pub fn career_page(
    progress: &Progress,
    raid: &Raid,
    memorial: &extraction_core::save::Memorial,
    keys_home: usize,
) -> String {
    let mut lines = vec![
        "CAREER".to_owned(),
        format!(
            "{} raids: {} home, {} lost",
            raid.raids, raid.extractions, raid.deaths
        ),
    ];
    let valleys: Vec<String> = progress
        .raids_by_map
        .iter()
        .map(|(m, n)| format!("{} {n}", valley_name(m)))
        .collect();
    if !valleys.is_empty() {
        lines.push(valleys.join(", "));
    }
    let tier = progress.tier_def();
    if tier.jobs > 0 {
        lines.push(format!(
            "Driver: {}, {} of {} done",
            tier.name, progress.done_in_tier, tier.jobs
        ));
        if let Some(u) = tier.fits.first() {
            lines.push(format!("Next: {}", u.name()));
        }
    } else {
        lines.push(format!("Driver: {} jobs done", progress.done));
    }
    let fitted: Vec<&str> = Upgrade::ALL
        .into_iter()
        .filter(|u| progress.has(*u))
        .map(|u| u.does())
        .collect();
    if !fitted.is_empty() {
        lines.push(format!("Fitted: {}", fitted.join("; ")));
    }
    lines.push(if raid.recalled {
        "The machines are recalled.".to_owned()
    } else {
        format!("Keys home: {keys_home} of 3")
    });
    if memorial.convoys > 0 {
        lines.push(format!(
            "Convoys: {} ({} things)",
            memorial.convoys,
            memorial.lost.len()
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests;
