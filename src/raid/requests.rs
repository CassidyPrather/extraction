//! The driver's requests. A card by the tailgate says what the driver
//! wants this time — two robot eyes, a few cans of stew, some scrap. Put
//! them in the stash and press the card at base: they're gone (a sink),
//! and the driver tells you where they hid a cache of their own somewhere
//! in the Zone. Nothing is bought: the reward is still out there to be
//! found, carried and got home. A new request comes with the next raid.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::noise::Rng;

use super::truck::{FLOOR, Pressable, Stash, Truck};
use super::{Phase, Raid};
use crate::interact::body::Backpack;
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::items::loot::Loot;
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::world::{Layer, Zone};

/// What the driver asks for: item id, how many (min, max), and why.
const WANTS: [(&str, u32, u32, &str); 10] = [
    (
        "sensor_eye",
        1,
        2,
        "Robot eyes. Someone in town pays in diesel.",
    ),
    ("scrap_metal", 3, 5, "Scrap. The tailgate hinge is going."),
    ("electronics", 2, 3, "Boards, any. The radio's dying."),
    ("canned_stew", 2, 3, "Something hot that isn't my cooking."),
    ("field_dressing", 2, 3, "Dressings. Not for me. Don't ask."),
    (
        "robot_cell",
        1,
        1,
        "One of their cells. I want to see inside.",
    ),
    ("gun_oil", 1, 1, "Gun oil. The winch is seizing."),
    ("matches", 2, 3, "Matches. Out again."),
    ("servo", 1, 2, "A servo or two. I'm making a thing."),
    ("tea_tin", 1, 1, "Tea. Real tea."),
];

/// Caches go at least this far from the truck: worth the walk.
const CACHE_AWAY: f32 = 150.0;

/// The request on the card, and where the last cache was left.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Requests {
    /// How many requests there have been.
    pub number: u32,
    /// What's wanted now (item id, count), if anything.
    pub want: Option<(String, u32)>,
    /// Where the driver's cache is, once told.
    pub cache: Option<String>,
    /// And on the map: the valley, and (x, z) there (pencilled on the
    /// field map).
    pub cache_at: Option<(String, [f32; 2])>,
}

impl Requests {
    /// The next request, rolled from the career seed and its number.
    pub fn next(&mut self, seed: u64) {
        self.number += 1;
        let mut rng = Rng::new(seed ^ 0x2E9_0E57 ^ (self.number as u64).wrapping_mul(0x9E37));
        let (id, lo, hi, _) = WANTS[rng.below(WANTS.len())];
        let n = lo + rng.below((hi - lo + 1) as usize) as u32;
        self.want = Some((id.to_owned(), n));
    }
}

fn why(id: &str) -> &'static str {
    WANTS
        .iter()
        .find(|w| w.0 == id)
        .map_or("You know what for.", |w| w.3)
}

#[derive(Component)]
pub struct RequestCard;

pub struct RequestsPlugin;

impl Plugin for RequestsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Requests>().add_systems(
            Update,
            (spawn_card, new_request, press_card, find_cache, card_text)
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

/// A request to start with, and a new one each raid after the last was
/// met.
fn new_request(
    raid: Res<Raid>,
    opts: Res<crate::launch::LaunchOptions>,
    mut requests: ResMut<Requests>,
    mut was_out: Local<bool>,
) {
    let out = raid.phase != Phase::AtBase;
    let set_off = out && !*was_out;
    *was_out = out;
    if requests.want.is_none() && (requests.number == 0 || set_off) {
        requests.next(opts.seed);
    }
}

#[allow(clippy::too_many_arguments)]
fn press_card(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    cards: Query<(), With<RequestCard>>,
    mut requests: ResMut<Requests>,
    mut raid: ResMut<Raid>,
    mut stashes: Query<&mut Backpack, With<Stash>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    loot: Option<Res<Loot>>,
    spots: Option<Res<crate::world::buildings::BuildingLoot>>,
    zone: Res<Zone>,
    opts: Res<crate::launch::LaunchOptions>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        if !cards.contains(*item) {
            continue;
        }
        let Some((id, n)) = requests.want.clone() else {
            raid.say("Driver: \"Nothing else for now. Go on.\"");
            continue;
        };
        if raid.phase != Phase::AtBase {
            raid.say("Driver: \"Back at base. I'm not counting tins out here.\"");
            continue;
        }
        let name = catalog
            .kind(&id)
            .map(|k| catalog.get(k).name.clone())
            .unwrap_or(id.clone());
        let Some(mut stash) = stashes.iter_mut().next() else {
            continue;
        };
        let have: Vec<Entity> = stash
            .contents
            .iter()
            .copied()
            .filter(|e| items.get(*e).is_ok_and(|i| catalog.get(i.kind()).id == id))
            .take(n as usize)
            .collect();
        if (have.len() as u32) < n {
            raid.say(format!(
                "Driver: \"That's {} of {n} {name}. In the stash, mind.\"",
                have.len()
            ));
            continue;
        }
        stash.contents.retain(|e| !have.contains(e));
        for e in have {
            commands.entity(e).despawn();
        }
        let mut rng = Rng::new(opts.seed ^ 0xCAC4E ^ requests.number as u64);
        let placed = match (&loot, &spots) {
            (Some(loot), Some(spots)) => {
                place_cache(&mut commands, &zone, spots, loot, &catalog, &mut rng)
            }
            _ => None,
        };
        requests.want = None;
        match placed {
            Some((where_, at)) => {
                raid.say(format!(
                    "Driver: \"Ta. My cache: {where_} It's on your map.\""
                ));
                requests.cache = Some(where_);
                requests.cache_at = Some((opts.map.clone(), [at.x, at.z]));
            }
            None => raid.say("Driver: \"Ta. I owe you one.\""),
        }
    }
}

/// Leaves the driver's cache at a building loot spot well away from the
/// truck, and says where.
fn place_cache(
    commands: &mut Commands,
    zone: &Zone,
    spots: &crate::world::buildings::BuildingLoot,
    loot: &Loot,
    catalog: &Catalog,
    rng: &mut Rng,
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
    for (i, drop) in loot
        .0
        .roll("driver_cache", catalog, rng)
        .into_iter()
        .enumerate()
    {
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
    let place = building.map_or("a spot".to_owned(), |b| format!("the {b}"));
    match landmark {
        Some(m) => {
            let d = extraction_core::geom::dist(m.at, here);
            let rel = if d < 60.0 { "by" } else { "out past" };
            format!("in {place} {rel} the {}.", m.name)
        }
        None => format!("in {place}."),
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
    catalog: Res<Catalog>,
    mut cards: Query<&mut ScreenText, With<RequestCard>>,
) {
    let mut text = String::from("DRIVER WANTS\n");
    match &requests.want {
        Some((id, n)) => {
            let name = catalog
                .kind(id)
                .map(|k| catalog.get(k).name.clone())
                .unwrap_or(id.clone());
            let them = if *n == 1 { "it" } else { "them" };
            text += &format!("{n} x {name}\n\"{}\"\nStash {them}, press here.", why(id));
        }
        None => text += "Nothing yet.",
    }
    if let Some(c) = &requests.cache {
        text += &format!("\n\nMY CACHE: {c}");
    }
    let text = crate::render::screen::wrap_lines(&text, 24);
    for mut c in &mut cards {
        if c.text != text {
            c.text = text.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handing_over_a_request_leaves_a_cache_out_there() {
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
        let (id, n) = app
            .world()
            .resource::<Requests>()
            .want
            .clone()
            .expect("a first request");
        let card = app
            .world_mut()
            .query_filtered::<Entity, With<RequestCard>>()
            .iter(app.world())
            .next()
            .expect("the card is in the truck");
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let items_before = app.world_mut().query::<&Item>().iter(app.world()).count();
        // Pressing with nothing to give: a "that's 0 of" from the driver.
        app.world_mut()
            .write_message(InteractEvent::Pressed { item: card, player });
        app.update();
        assert!(app.world().resource::<Requests>().want.is_some());
        // Put what's wanted in the stash.
        let catalog = app.world().resource::<Catalog>().clone();
        let stash = app
            .world_mut()
            .query_filtered::<Entity, With<Stash>>()
            .iter(app.world())
            .next()
            .unwrap();
        for _ in 0..n {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut c = Commands::new(&mut queue, app.world());
                spawn_item(
                    &mut c,
                    &catalog,
                    SpawnItem {
                        kind: catalog.kind(&id).unwrap(),
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
        app.world_mut()
            .write_message(InteractEvent::Pressed { item: card, player });
        app.update();
        app.update();
        let r = app.world().resource::<Requests>().clone();
        assert!(r.want.is_none(), "handed over");
        let note = r.cache.expect("told where the cache is");
        assert!(r.cache_at.is_some(), "and it's on the map");
        // Walking up to it takes the cross off.
        let at = r.cache_at.clone().unwrap().1;
        let zone = app.world().resource::<crate::world::Zone>().clone();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = zone.on_ground(at[0], at[1]) + Vec3::Y * 0.1;
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world().resource::<Requests>().cache_at.is_none(),
            "found it"
        );
        assert!(note.starts_with("in the "), "{note}");
        let items_after = app.world_mut().query::<&Item>().iter(app.world()).count();
        assert!(
            items_after > items_before,
            "the cache is out there ({items_before} -> {items_after})"
        );
        // The next raid brings the next request.
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.update();
        let r = app.world().resource::<Requests>();
        assert_eq!(r.number, 2);
        assert!(r.want.is_some());
    }
}
