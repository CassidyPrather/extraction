//! The driver's jobs. A card by the tailgate says what the driver wants
//! this time: a few of something from the stash (a fetch), a recorder
//! brought home from a building out there (a recover), or a look at a
//! place (a scout). Press the card at base with it done: the driver tells
//! you where they hid a cache of their own somewhere in the Zone, and the
//! next job goes on the card. Nothing is bought: the reward is still out
//! there to be found, carried and got home. The jobs come in tiers, and
//! each tier done gets something fitted to the truck (`progression`).

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::noise::Rng;
use extraction_core::progression::{Job, LEDGER_ITEM, LETTER_ITEM, RECOVER_ITEM, TIERS};

use super::truck::{FLOOR, Pressable, Stash, Truck};
use super::{Phase, Raid};
use crate::interact::body::Backpack;
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::items::loot::Loot;
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::progression::{Progression, lines};
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::world::{Layer, Zone};

/// Caches go at least this far from the truck: worth the walk.
const CACHE_AWAY: f32 = 150.0;

/// How many jobs have been paid for, and where the last cache was left.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Requests {
    /// Jobs paid for (seeds each cache).
    pub number: u32,
    /// Where the driver's cache is, once told.
    pub cache: Option<String>,
    /// And on the map: the valley, and (x, z) there (pencilled on the
    /// field map).
    pub cache_at: Option<(String, [f32; 2])>,
}

#[derive(Component)]
pub struct RequestCard;

pub struct RequestsPlugin;

impl Plugin for RequestsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Requests>().add_systems(
            Update,
            (spawn_card, press_card, find_cache, card_text)
                .chain()
                .after(super::leave_base),
        );
    }
}

fn spawn_card(mut commands: Commands, trucks: Query<Entity, Added<Truck>>) {
    for truck in &trucks {
        commands.spawn((
            Name::new("Request card"),
            RequestCard,
            Pressable::Request,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            Collider::cuboid(0.3, 0.26, 0.03),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            ScreenText {
                text: String::new(),
                size: Vec2::new(0.3, 0.26),
                style: ScreenStyle::Card,
            },
            // By the logbook, nearer the tailgate.
            Transform::from_xyz(-1.12, FLOOR + 1.66, 1.32)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            ChildOf(truck),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn press_card(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    cards: Query<(), With<RequestCard>>,
    mut requests: ResMut<Requests>,
    mut progression: ResMut<Progression>,
    mut raid: ResMut<Raid>,
    mut stashes: Query<&mut Backpack, With<Stash>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    loot: Option<Res<Loot>>,
    spots: Option<Res<crate::world::buildings::BuildingLoot>>,
    zone: Res<Zone>,
    opts: Res<crate::launch::LaunchOptions>,
    mut done: MessageWriter<crate::progression::JobDone>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        if !cards.contains(*item) {
            continue;
        }
        let Some(job) = progression.0.job.clone() else {
            raid.say(lines::NO_JOB);
            continue;
        };
        if raid.phase != Phase::AtBase {
            raid.say(lines::NOT_AT_BASE);
            continue;
        }
        if progression.0.last_raid == Some(raid.raids) {
            raid.say(lines::ONE_A_RAID);
            continue;
        }
        if opts.map == crate::camp::TRAINING_YARD {
            raid.say(lines::IN_THE_YARD);
            continue;
        }
        let Some(mut stash) = stashes.iter_mut().next() else {
            continue;
        };
        let in_stash = |id: &str, n: u32, stash: &Backpack| -> Vec<Entity> {
            stash
                .contents
                .iter()
                .copied()
                .filter(|e| {
                    items.get(*e).is_ok_and(|i| {
                        extraction_core::progression::fetch_takes(id, &catalog.get(i.kind()).id)
                    })
                })
                .take(n as usize)
                .collect()
        };
        let handed: Vec<Entity> = match &job {
            Job::Fetch { item: id, n } => {
                let have = in_stash(id, *n, &stash);
                if (have.len() as u32) < *n {
                    raid.say(lines::fetch_short(
                        have.len(),
                        *n,
                        &crate::progression::want_name(&catalog, id),
                    ));
                    continue;
                }
                have
            }
            Job::Recover { .. } => {
                let have = in_stash(RECOVER_ITEM, 1, &stash);
                if have.is_empty() {
                    raid.say(lines::NO_RECORDER);
                    continue;
                }
                have
            }
            Job::Letter => {
                let have = in_stash(LETTER_ITEM, 1, &stash);
                if have.is_empty() {
                    raid.say(lines::NO_LETTER);
                    continue;
                }
                have
            }
            Job::Ledger => {
                let have = in_stash(LEDGER_ITEM, 1, &stash);
                if have.is_empty() {
                    raid.say(lines::NO_LEDGER);
                    continue;
                }
                have
            }
            Job::Scout { place, seen, .. } => {
                if !seen {
                    raid.say(lines::scout_not_seen(place));
                    continue;
                }
                Vec::new()
            }
        };
        stash.contents.retain(|e| !handed.contains(e));
        for e in handed {
            commands.entity(e).despawn();
        }
        requests.number += 1;
        progression.0.last_raid = Some(raid.raids);
        let tier = progression.0.tier;
        let mut rng = Rng::new(opts.seed ^ 0xCAC4E ^ requests.number as u64);
        let placed = match (&loot, &spots) {
            (Some(loot), Some(spots)) => {
                let order = progression.0.order_def();
                place_cache(
                    &mut commands,
                    &zone,
                    spots,
                    loot,
                    &catalog,
                    &mut rng,
                    tier,
                    order,
                )
            }
            _ => None,
        };
        let thanks = match &job {
            Job::Scout { place, .. } => lines::thanks(Some(place)),
            _ => lines::thanks(None),
        };
        if job == Job::Letter {
            raid.say(lines::LETTER_HOME);
        }
        if job == Job::Ledger {
            raid.say(lines::LEDGER_HOME);
        }
        match placed {
            Some((where_, at)) => {
                raid.say(lines::paid(&thanks, &where_));
                requests.cache = Some(where_);
                requests.cache_at = Some((opts.map.clone(), [at.x, at.z]));
            }
            None => raid.say(lines::paid_nothing(&thanks)),
        }
        done.write(crate::progression::JobDone);
    }
}

/// Leaves the driver's cache at a building loot spot well away from the
/// truck, and says where.
#[allow(clippy::too_many_arguments)]
fn place_cache(
    commands: &mut Commands,
    zone: &Zone,
    spots: &crate::world::buildings::BuildingLoot,
    loot: &Loot,
    catalog: &Catalog,
    rng: &mut Rng,
    tier: u32,
    order: &extraction_core::progression::Order,
) -> Option<(String, Vec3)> {
    let base = zone
        .marker(&MarkerKind::TruckStop)
        .map(|m| Vec2::from_array(m.at))
        .unwrap_or(Vec2::ZERO);
    let far: Vec<Vec3> = spots
        .0
        .iter()
        .map(|s| s.at)
        .filter(|p| p.xz().distance(base) > CACHE_AWAY)
        .collect();
    if far.is_empty() {
        return None;
    }
    let at = far[rng.below(far.len())];
    // From the third tier on the driver has more to give: a second pile;
    // past the ladder, a third, of what you ordered.
    let piles = match tier {
        0 | 1 => 1,
        2 | 3 => 2,
        _ => 3,
    };
    let drops: Vec<_> = (0..piles * order.rolls)
        .flat_map(|_| loot.0.roll(order.table, catalog, rng))
        .collect();
    for (i, drop) in drops.into_iter().enumerate() {
        let off = Vec3::new(
            (i as f32 * 1.7).sin() * 0.2,
            0.08 + 0.06 * i as f32,
            (i as f32 * 1.7).cos() * 0.2,
        );
        spawn_item(
            commands,
            catalog,
            SpawnItem {
                kind: drop.kind,
                transform: Transform::from_translation(at + off),
                condition: drop.condition,
            },
        );
    }
    Some((describe_place(zone, at), at))
}

/// "in the barn by the Low farm."
pub fn describe_place(zone: &Zone, at: Vec3) -> String {
    let here = [at.x, at.z];
    // Below ground: the site it's in ("down in the mine gallery by ...").
    let below = (at.y < zone.ground(at.x, at.z) - 1.5)
        .then(|| {
            zone.map
                .placements
                .iter()
                .filter(|p| extraction_core::geom::dist(p.at, here) < 60.0)
                .find_map(|p| extraction_core::underground::layout(&p.prefab))
        })
        .flatten();
    let building = zone
        .map
        .placements
        .iter()
        .min_by(|a, b| {
            extraction_core::geom::dist(a.at, here)
                .total_cmp(&extraction_core::geom::dist(b.at, here))
        })
        .filter(|p| extraction_core::geom::dist(p.at, here) < 20.0)
        .map(|p| building_name(&p.prefab));
    let landmark = zone
        .map
        .markers
        .iter()
        .filter(|m| {
            matches!(
                m.kind,
                MarkerKind::Poi | MarkerKind::Rendezvous | MarkerKind::Offering
            ) && !m.name.is_empty()
        })
        .min_by(|a, b| {
            extraction_core::geom::dist(a.at, here)
                .total_cmp(&extraction_core::geom::dist(b.at, here))
        });
    let place = match below {
        Some(l) => format!("the {}", l.name.to_lowercase()),
        None => building.map_or("a spot".to_owned(), |b| format!("the {b}")),
    };
    let into = if below.is_some() { "down in" } else { "in" };
    match landmark {
        Some(m) => {
            let d = extraction_core::geom::dist(m.at, here);
            let rel = if d < 60.0 { "by" } else { "out past" };
            format!("{into} {place} {rel} the {}.", m.name)
        }
        None => format!("{into} {place}."),
    }
}

fn building_name(prefab: &str) -> String {
    match prefab {
        "house_plank" => "plank house".into(),
        "house_brick" => "brick house".into(),
        "cabin_log" => "log cabin".into(),
        "pump_station" => "pump house".into(),
        "rail_shed" => "rail shed".into(),
        "mast_hut" => "hut under the mast".into(),
        "guard_booth" => "guard booth".into(),
        "ranger_tower" => "ranger tower".into(),
        other => other.replace('_', " "),
    }
}

/// Walking up to the cache takes the cross off the map; a Surge sweeping
/// the Zone takes the cache.
fn find_cache(
    mut requests: ResMut<Requests>,
    mut raid: ResMut<Raid>,
    opts: Res<crate::launch::LaunchOptions>,
    players: Query<&GlobalTransform, With<crate::rig::LocalPlayer>>,
    mut cycle: Local<Option<u32>>,
) {
    let was = cycle.replace(raid.cycle);
    if was.is_some_and(|c| c != raid.cycle) && requests.cache_at.is_some() {
        requests.cache_at = None;
        requests.cache = None;
        raid.say("Whatever was left in the driver's cache, the Surge has it now.");
        return;
    }
    let Some((map, at)) = requests.cache_at.clone() else {
        return;
    };
    if map != opts.map {
        return;
    }
    let near = players.iter().any(|p| {
        Vec2::new(p.translation().x, p.translation().z).distance(Vec2::from_array(at)) < 3.0
    });
    if near {
        requests.cache_at = None;
        requests.cache = None;
        raid.say("The driver's cache. Just where he said.");
    }
}

fn card_text(
    requests: Res<Requests>,
    progression: Res<Progression>,
    catalog: Res<Catalog>,
    opts: Res<crate::launch::LaunchOptions>,
    mut cards: Query<&mut ScreenText, With<RequestCard>>,
    added: Query<(), Added<RequestCard>>,
    mut last: Local<String>,
) {
    let p = &progression.0;
    let tier = p.tier_def();
    let mut text = if (p.tier as usize) < TIERS.len() {
        format!(
            "{} - JOB {} OF {}\n",
            tier.name.to_uppercase(),
            p.done_in_tier + 1,
            tier.jobs
        )
    } else {
        "DRIVER WANTS\n".to_owned()
    };
    match &p.job {
        Some(job) => text += &crate::progression::job_text(job, &catalog, &opts.map),
        None => text += lines::NOTHING_YET,
    }
    if let Some(u) = p.next_fitting().first() {
        if p.done_in_tier + 1 == tier.jobs {
            text += "\n";
            text += &lines::promise(u.name());
        }
    }
    if let Some(c) = &requests.cache {
        text += &format!("\n\nMY CACHE: {c}");
    }
    // Fitting the text is not free: only when it changes.
    if *last == text && added.is_empty() {
        return;
    }
    *last = text.clone();
    for mut c in &mut cards {
        let text = crate::render::screen::wrap_to_fit(&text, c.size, c.style);
        if c.text != text {
            c.text = text.clone();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::progression::Progression;

    pub(crate) fn app() -> App {
        let opts = crate::LaunchOptions {
            robots: false,
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

    pub(crate) fn one<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, F>()
            .iter(app.world())
            .next()
            .expect("there is one")
    }

    /// Puts `n` new `id`s in the truck's stash.
    pub(crate) fn stash(app: &mut App, id: &str, n: u32) {
        let catalog = app.world().resource::<Catalog>().clone();
        let stash = one::<With<Stash>>(app);
        for _ in 0..n {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut c = Commands::new(&mut queue, app.world());
                spawn_item(
                    &mut c,
                    &catalog,
                    SpawnItem {
                        kind: catalog.kind(id).unwrap(),
                        transform: Transform::default(),
                        condition: 1.0,
                    },
                )
            };
            queue.apply(app.world_mut());
            app.world_mut()
                .get_mut::<Backpack>(stash)
                .unwrap()
                .contents
                .push(e);
            app.world_mut().entity_mut(e).insert((
                crate::interact::body::InBackpack { bag: stash },
                ChildOf(stash),
                RigidBodyDisabled,
                ColliderDisabled,
            ));
        }
        app.update();
    }

    pub(crate) fn press_card(app: &mut App) {
        let card = one::<With<RequestCard>>(app);
        let player = one::<With<crate::rig::LocalPlayer>>(app);
        app.world_mut()
            .write_message(InteractEvent::Pressed { item: card, player });
        app.update();
        app.update();
    }

    /// Walks the player to (x, z) on the ground.
    pub(crate) fn go(app: &mut App, at: [f32; 2]) {
        let player = one::<With<crate::rig::LocalPlayer>>(app);
        let zone = app.world().resource::<crate::world::Zone>().clone();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = zone.on_ground(at[0], at[1]) + Vec3::Y * 0.1;
        for _ in 0..3 {
            app.update();
        }
    }

    /// Back in the truck bed at base, the raid over.
    pub(crate) fn home(app: &mut App) {
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let bed = super::super::truck::base_pose(&zone).transform_point(Vec3::new(0.3, FLOOR, 0.6));
        go(app, [bed.x, bed.z]);
        app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
        app.update();
    }

    fn job(app: &App) -> Job {
        app.world()
            .resource::<Progression>()
            .0
            .job
            .clone()
            .expect("a job on the card")
    }

    #[test]
    fn a_job_done_pays_a_cache_out_there() {
        let mut app = app();
        // The first job: go and look at somewhere near.
        let Job::Scout { at, place, .. } = job(&app) else {
            panic!("the first job is a scout: {:?}", job(&app));
        };
        // Pressing before going: not done.
        press_card(&mut app);
        assert!(matches!(job(&app), Job::Scout { seen: false, .. }));
        assert_eq!(app.world().resource::<Progression>().0.done, 0);
        // Out to the place, and home.
        go(&mut app, at);
        assert!(
            app.world().resource::<Raid>().phase != Phase::AtBase,
            "walking off starts the raid"
        );
        assert!(
            matches!(job(&app), Job::Scout { seen: true, .. }),
            "seen the {place}"
        );
        home(&mut app);
        let items_before = app.world_mut().query::<&Item>().iter(app.world()).count();
        press_card(&mut app);
        let r = app.world().resource::<Requests>().clone();
        assert_eq!(app.world().resource::<Progression>().0.done, 1, "done");
        assert_eq!(r.number, 1);
        let note = r.cache.clone().expect("told where the cache is");
        assert!(note.starts_with("in the "), "{note}");
        let items_after = app.world_mut().query::<&Item>().iter(app.world()).count();
        assert!(
            items_after > items_before,
            "the cache is out there ({items_before} -> {items_after})"
        );
        // The next job is on the card straight away.
        app.update();
        assert!(app.world().resource::<Progression>().0.job.is_some());
        // Walking up to the cache takes the cross off.
        go(&mut app, r.cache_at.clone().unwrap().1);
        assert!(
            app.world().resource::<Requests>().cache_at.is_none(),
            "found it"
        );

        // A fetch: what's wanted goes from the stash.
        home(&mut app);
        app.world_mut().resource_mut::<Progression>().0.job = Some(Job::Fetch {
            item: "canned_stew".into(),
            n: 2,
        });
        let stew = |app: &mut App| {
            let catalog = app.world().resource::<Catalog>().clone();
            app.world_mut()
                .query::<&Item>()
                .iter(app.world())
                .filter(|i| catalog.get(i.kind()).id == "canned_stew")
                .count()
        };
        stash(&mut app, "canned_stew", 1);
        press_card(&mut app);
        assert_eq!(
            app.world().resource::<Progression>().0.done,
            1,
            "one's not two"
        );
        stash(&mut app, "canned_stew", 1);
        let before = stew(&mut app);
        press_card(&mut app);
        assert_eq!(app.world().resource::<Progression>().0.done, 2);
        assert_eq!(stew(&mut app), before - 2, "handed over");
    }
}
