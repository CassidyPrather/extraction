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

/// An underground site's rooms lie within this of its entrance.
const UNDERGROUND_REACH: f32 = 60.0;

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
        app.add_plugins(safehouse::SafehousePlugin);
        app.init_resource::<Progression>()
            .add_message::<JobDone>()
            .add_systems(
                Update,
                (
                    count_raids,
                    upkeep,
                    spawn_order_card,
                    press_order_card,
                    order_card_text,
                    job_done,
                    offer_job,
                    place_recorder,
                    place_letter,
                    place_ledger,
                    scout,
                    contracts::place_bring,
                    contracts::watch_plant,
                    contracts::photograph,
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

/// An item's name, or what a fetch of several kinds calls them.
pub fn want_name(catalog: &Catalog, id: &str) -> String {
    if id == extraction_core::progression::ANY_PARTS {
        return "robot parts (any)".to_owned();
    }
    catalog
        .kind(id)
        .map(|k| catalog.get(k).name.clone())
        .unwrap_or(id.to_owned())
}

/// The job as the card says it.
pub fn job_text(job: &Job, catalog: &Catalog, map: &str, epilogue: bool) -> String {
    let elsewhere = |m: &str| {
        if m == map {
            String::new()
        } else {
            lines::elsewhere(&valley_name(m))
        }
    };
    match job {
        Job::Fetch { item, n } => {
            let name = want_name(catalog, item);
            let why = epilogue
                .then(|| lines::why_after(item))
                .flatten()
                .unwrap_or_else(|| extraction_core::progression::why(item));
            lines::ask_fetch(*n, &name, why)
        }
        Job::Recover {
            map: m,
            region,
            place,
            overnight,
            ..
        } => {
            let where_ = place
                .clone()
                .unwrap_or_else(|| format!("somewhere in the {region}."));
            if *overnight {
                elsewhere(m) + &lines::ask_expedition(&where_)
            } else {
                elsewhere(m) + &lines::ask_recover(&where_)
            }
        }
        Job::Bring {
            map: m,
            place,
            item,
            ..
        } => elsewhere(m) + &lines::ask_bring(&want_name(catalog, item), place),
        Job::Plant {
            map: m,
            place,
            item,
            planted,
            ..
        } => {
            if *planted {
                lines::planted(place) + "\nCome home and press here."
            } else {
                elsewhere(m) + &lines::ask_plant(&want_name(catalog, item), place)
            }
        }
        Job::Photograph { subject, taken } => {
            if *taken {
                lines::photo_taken(subject.name()) + "\nGet it home, press here."
            } else {
                lines::ask_photo(subject.name())
            }
        }
        Job::Letter => elsewhere(extraction_core::progression::LETTER_MAP) + lines::ASK_LETTER,
        Job::Ledger => elsewhere(extraction_core::progression::LEDGER_MAP) + lines::ASK_LEDGER,
        Job::Scout {
            map: m,
            place,
            seen,
            ..
        } => {
            if *seen {
                lines::scout_seen(place)
            } else {
                elsewhere(m) + &lines::ask_scout(place)
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
            let name = want_name(&catalog, &item);
            raid.say(lines::fetch_swapped(&name));
        }
        progression.0.swaps += 1;
    }
}

/// What the driver leaves in the stash along with a fitting: (item id,
/// how many, condition). Ids the catalog doesn't know are skipped.
pub fn gifts(u: Upgrade) -> &'static [(&'static str, u32, f32)] {
    match u {
        // The end of the second tier: the driver's own old carbine.
        Upgrade::BenchVice => &[
            ("sks", 1, 0.6),
            ("sks_clip", 2, 1.0),
            ("ammo_762x39", 1, 1.0),
        ],
        // The end of the third: a sub-machine gun from his man in town.
        Upgrade::Aerial => &[
            ("ppsh", 1, 0.55),
            ("ppsh_drum", 1, 1.0),
            ("ammo_762x25", 1, 1.0),
        ],
        _ => &[],
    }
}

/// Home from a raid: fittings due their upkeep take it from the stash, or
/// lie up until they can (`Progress::upkeep`).
#[allow(clippy::too_many_arguments)]
fn upkeep(
    mut commands: Commands,
    mut raid: ResMut<Raid>,
    opts: Res<LaunchOptions>,
    mut progression: ResMut<Progression>,
    mut stashes: Query<&mut Backpack, With<Stash>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut was_out: Local<bool>,
) {
    let out = raid.phase != Phase::AtBase;
    let home = !out && *was_out;
    *was_out = out;
    if !home || opts.map == crate::camp::TRAINING_YARD {
        return;
    }
    let Some(mut stash) = stashes.iter_mut().next() else {
        return;
    };
    let raids = raid.raids;
    let said = progression.0.upkeep(raids, |parts| {
        let mut taken: Vec<Entity> = Vec::new();
        for (id, n) in parts {
            let have: Vec<Entity> = stash
                .contents
                .iter()
                .copied()
                .filter(|e| !taken.contains(e))
                .filter(|e| items.get(*e).is_ok_and(|i| catalog.get(i.kind()).id == *id))
                .take(*n as usize)
                .collect();
            if (have.len() as u32) < *n {
                return false;
            }
            taken.extend(have);
        }
        stash.contents.retain(|e| !taken.contains(e));
        for e in taken {
            commands.entity(e).despawn();
        }
        true
    });
    for (u, how) in said {
        let parts = lines::parts(u.upkeep(), |id| want_name(&catalog, id).to_lowercase());
        raid.say(match how {
            extraction_core::progression::Upkept::Paid => lines::upkeep_paid(u.short(), &parts),
            extraction_core::progression::Upkept::LaidUp => lines::laid_up(u.short(), &parts),
            extraction_core::progression::Upkept::Back => lines::back_in_order(u.short()),
        });
    }
}

/// The order card, under the job: what the driver's caches lean to, once
/// the ladder's done.
#[derive(Component)]
pub struct OrderCard;

fn spawn_order_card(mut commands: Commands, trucks: Query<Entity, Added<Truck>>) {
    for truck in &trucks {
        commands.spawn((
            Name::new("Order card"),
            OrderCard,
            crate::raid::truck::Pressable::Setting,
            crate::interact::Grabbable {
                points: vec![crate::interact::GrabPoint::free()],
            },
            Collider::cuboid(0.3, 0.12, 0.03),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            crate::render::legibility::TextSource::Request,
            crate::raid::bench::ScreenText {
                text: String::new(),
                size: Vec2::new(0.3, 0.12),
                style: crate::raid::bench::ScreenStyle::Card,
            },
            // Under the request card.
            Transform::from_xyz(-1.12, FLOOR + 1.42, 1.32)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Visibility::Hidden,
            ChildOf(truck),
        ));
    }
}

fn press_order_card(
    mut events: MessageReader<crate::interact::InteractEvent>,
    cards: Query<(), With<OrderCard>>,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
) {
    for ev in events.read() {
        let crate::interact::InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        if !cards.contains(*item) {
            continue;
        }
        if !progression.0.past_ladder() {
            raid.say(lines::ORDER_LATER);
            continue;
        }
        progression.0.next_order();
        let name = progression.0.order_def().name;
        raid.say(lines::ordered(
            &name[..1]
                .to_uppercase()
                .chars()
                .chain(name[1..].to_lowercase().chars())
                .collect::<String>(),
        ));
    }
}

fn order_card_text(
    progression: Res<Progression>,
    mut cards: Query<(&mut crate::raid::bench::ScreenText, &mut Visibility), With<OrderCard>>,
) {
    let open = progression.0.past_ladder();
    let text = lines::order_card(progression.0.order_def().name);
    for (mut c, mut vis) in &mut cards {
        let want = if open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if open && c.text != text {
            c.text = text.clone();
        }
    }
}

/// A job handed in: count it, and fit what a finished tier earns.
fn job_done(
    mut commands: Commands,
    mut done: MessageReader<JobDone>,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
    mut save: MessageWriter<crate::persist::SaveNow>,
    mut handover: MessageWriter<crate::raid::lev::HandOver>,
    catalog: Res<Catalog>,
) {
    for _ in done.read() {
        let (fitted, finished) = progression.0.complete_job();
        if let Some(c) = finished {
            contracts::pay(c, &mut raid, &mut handover);
        }
        for u in fitted {
            raid.say(lines::fitted(*u));
            // He passes them back through the cab's hatch (`raid::lev`).
            let mut gave = false;
            for (id, n, condition) in gifts(*u) {
                if catalog.kind(id).is_none() {
                    continue;
                }
                for _ in 0..*n {
                    handover.write(crate::raid::lev::HandOver {
                        id: id.to_string(),
                        condition: *condition,
                    });
                    gave = true;
                }
            }
            if gave {
                raid.say(lines::gift(*u));
            }
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
fn offer_job(
    opts: Res<LaunchOptions>,
    // Lev has been into the depot with you (Story's epilogue): the job
    // card's asks ease off.
    epilogue: Res<crate::raid::epilogue::Epilogue>,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
    mut handover: MessageWriter<crate::raid::lev::HandOver>,
) {
    if progression.0.quiet != epilogue.done {
        progression.0.quiet = epilogue.done;
    }
    if progression.0.job.is_some() {
        return;
    }
    let here = here(&opts);
    let valleys: Vec<Valley> = valleys()
        .iter()
        .map(|(id, map)| Valley { id, map })
        .collect();
    let roll = |p: &Progress| extraction_core::progression::roll_job(p, opts.seed, &here, &valleys);
    if opts.sim_run.is_some() && progression.0.done == 0 {
        // A sim bot takes a scout job (the run's seed picks the tier), to
        // measure what a job pulls: the telemetry's `end` says whether it
        // was seen and got home.
        // (Tiers 1-3 and the deep jobs after the ladder; tier 4's are in
        // another valley, which a single sim raid can't go to.)
        let tier =
            [0, 1, 2, extraction_core::progression::TIERS.len() as u32][(opts.seed % 4) as usize];
        progression.0.tier = tier;
        progression.0.done = 1 + 3 * tier;
        for _ in 0..40 {
            let job = roll(&progression.0);
            if matches!(job, Job::Scout { .. }) {
                progression.0.job = Some(job);
                break;
            }
            progression.0.swaps += 1;
        }
        info!("sim job: {:?}", progression.0.job);
        return;
    }
    // A contract the driver has for you now starts with its kit.
    if let Some(c) = progression.0.start_contract() {
        info!("contract: {}", c.id);
        contracts::hand_kit(c, &mut raid, &mut handover);
    }
    let job = extraction_core::contracts::contract_job(&progression.0, &valleys)
        .unwrap_or_else(|| roll(&progression.0));
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
        ..
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
    // An underground site's name: its rooms (below ground, near its
    // entrance); else a region's.
    let site = extraction_core::progression::underground_site(&zone.map, region);
    let in_region = |p: Vec3| match &site {
        Some((_, at, _)) => {
            p.xz().distance(Vec2::from_array(*at)) < UNDERGROUND_REACH
                && p.y < zone.ground(p.x, p.z) - 1.5
        }
        // A region's: above ground (a site under it is its own job).
        None => {
            p.y >= zone.ground(p.x, p.z) - 1.5
                && zone
                    .map
                    .region_at([p.x, p.z])
                    .is_some_and(|r| r.name == *region)
        }
    };
    let mut candidates: Vec<Vec3> = spots
        .iter()
        .flat_map(|s| s.0.iter().map(|s| s.at))
        .filter(|p| in_region(*p))
        .collect();
    if candidates.is_empty() && site.is_none() {
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

/// The letter job: Hana's letter lies on the table in the house by the
/// well until it's taken home (put back if the Surge or the convoy takes
/// it).
fn place_letter(
    mut commands: Commands,
    progression: Res<Progression>,
    opts: Res<LaunchOptions>,
    zone: Res<Zone>,
    spatial: SpatialQuery,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut tried: Local<u32>,
) {
    use extraction_core::progression::{LETTER_AT, LETTER_ITEM, LETTER_MAP};
    if progression.0.job != Some(Job::Letter) || opts.map != LETTER_MAP {
        return;
    }
    let Some(kind) = catalog.kind(LETTER_ITEM) else {
        return;
    };
    if items.iter().any(|i| i.kind() == kind) {
        return;
    }
    // Only so often: the house may still be going up.
    *tried += 1;
    if *tried % 30 != 1 {
        return;
    }
    // Onto the table: down from under the roof to the first thing hit.
    let ground = zone.on_ground(LETTER_AT[0], LETTER_AT[1]);
    let from = ground + Vec3::Y * 1.6;
    let top = spatial
        .cast_ray(
            from,
            Dir3::NEG_Y,
            3.0,
            true,
            &SpatialQueryFilter::from_mask(Layer::World),
        )
        .map_or(ground, |hit| from - Vec3::Y * hit.distance);
    spawn_item(
        &mut commands,
        &catalog,
        SpawnItem {
            kind,
            transform: Transform::from_translation(top + Vec3::Y * 0.02),
            condition: 0.9,
        },
    );
    info!("Hana's letter on the table at {top:?}");
}

/// The ledger job: the intake ledger lies in the Mareks' bay in the
/// depot until it's taken home (put back if the Surge or the convoy takes
/// it).
fn place_ledger(
    mut commands: Commands,
    progression: Res<Progression>,
    opts: Res<LaunchOptions>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut tried: Local<u32>,
) {
    use extraction_core::progression::{LEDGER_AT, LEDGER_ITEM, LEDGER_MAP};
    if progression.0.job != Some(Job::Ledger) || opts.map != LEDGER_MAP {
        return;
    }
    let Some(kind) = catalog.kind(LEDGER_ITEM) else {
        return;
    };
    if items.iter().any(|i| i.kind() == kind) {
        return;
    }
    // Only so often: the depot may still be going up.
    *tried += 1;
    if *tried % 30 != 1 {
        return;
    }
    let at = Vec3::from_array(LEDGER_AT) + Vec3::Y * 0.05;
    spawn_item(
        &mut commands,
        &catalog,
        SpawnItem {
            kind,
            transform: Transform::from_translation(at),
            condition: 0.9,
        },
    );
    info!("the intake ledger in bay K-14 at {at:?}");
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
        raid.say(lines::scout_forgotten(place));
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
        raid.say(lines::scout_noticed(place));
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
    if !progression.0.works(Upgrade::Aerial) || raid.phase == Phase::AtBase {
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
        raid.say(lines::AERIAL_SURGE);
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
    let taught = extraction_core::contracts::recipes(&progression.0);
    for mut b in &mut benches {
        if b.tier != tier {
            b.tier = tier;
        }
        if b.taught != taught {
            b.taught = taught.clone();
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
        || !progression.0.works(Upgrade::MedCabinet)
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
    epilogue: bool,
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
    let laid: Vec<String> = Upgrade::ALL
        .into_iter()
        .filter(|u| progress.has(*u) && !progress.works(*u))
        .map(|u| u.short().to_owned())
        .collect();
    if !laid.is_empty() {
        lines.push(format!("Laid up: {}", laid.join(", ")));
    }
    if progress.past_ladder() {
        lines.push(format!(
            "Caches: {}",
            progress.order_def().name.to_lowercase()
        ));
    }
    // The contracts: the one under way, and the trust they've earned.
    if let Some(c) = progress
        .contract
        .as_ref()
        .and_then(|a| extraction_core::contracts::contract(&a.id).map(|c| (a, c)))
    {
        lines.push(format!(
            "Contract: {}, {} of {}",
            c.1.name,
            c.0.step + 1,
            c.1.steps.len()
        ));
    }
    if !progress.contracts_done.is_empty() {
        lines.push(format!(
            "Trust: {} ({} contracts done)",
            progress.trust,
            progress.contracts_done.len()
        ));
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
    if epilogue {
        lines.push(lines::CAREER_AFTER.to_owned());
    }
    lines.join("\n")
}

pub mod contracts;
pub mod lines;
pub mod safehouse;

#[cfg(test)]
mod tests;
