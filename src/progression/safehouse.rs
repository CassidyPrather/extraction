//! The valley's safehouse in the game: finding its cabin, claiming it (clear
//! it, wedge its door, light its stove), its crate, sleeping a night in its
//! bed, its radio, and what a loud raid or the Surge does to it. Rules and
//! state are in [`extraction_core::safehouse`].

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::safehouse::{CLEAR_RADIUS, NIGHT, SafehouseSave, safehouse};

use super::lines;
use crate::anomaly::Attention;
use crate::anomaly::artifacts::CarriedBy;
use crate::interact::body::Backpack;
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::items::{Catalog, Item};
use crate::launch::LaunchOptions;
use crate::raid::truck::Pressable;
use crate::raid::{Phase, Raid, TRUCK_WAIT};
use crate::rig::LocalPlayer;
use crate::robots::Robot;
use crate::world::buildings::{Building, DoorLeaf};
use crate::world::{Layer, Zone};

/// This valley's safehouse: its state (saved with the Zone) and the things
/// in the world that make it up.
#[derive(Resource, Debug, Default, Clone)]
pub struct Safehouse {
    pub state: SafehouseSave,
    /// The cabin, its door, and what claiming it brings to life.
    pub cabin: Option<Entity>,
    pub door: Option<Entity>,
    pub crate_bag: Option<Entity>,
    /// The truck's been told to wait the night (the radio).
    pub truck_waits: bool,
    /// The loudest the Zone has felt this raid (`Attention::feeling`).
    pub peak: u8,
    /// Nights slept this raid (for the logbook).
    pub nights_out: u32,
}

/// Steps the hour on from the cycle's own, one per night slept
/// (`world::weather` reads it); back to the cycle's when the Surge comes.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct HourShift(pub u8);

/// A night was slept (the client fades the view out and back).
#[derive(Message, Clone, Copy, Debug)]
pub struct Slept;

/// The safehouse's stove, bed and radio: pressed to light, sleep, call.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SafehouseThing {
    Stove,
    Bed,
    Radio,
}

/// The safehouse's own crate (its contents are saved with it, not the
/// truck's stash).
#[derive(Component)]
pub struct SafehouseCrate;

/// Cabin space (`world::buildings::cabin`): floor height, stove, bed, and
/// where claiming puts the crate and the radio.
const FLOOR: f32 = 0.4;
const STOVE: Vec3 = Vec3::new(2.6, FLOOR + 0.9, 2.3);
const BED: Vec3 = Vec3::new(-2.8, FLOOR + 0.55, 1.5);
const CRATE: Vec3 = Vec3::new(-2.6, FLOOR + 0.3, -1.6);
const RADIO: Vec3 = Vec3::new(0.0, FLOOR + 0.8, 0.5);
/// Inside: this close to the cabin's middle.
const INSIDE: f32 = 3.2;

pub struct SafehousePlugin;

impl Plugin for SafehousePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Slept>()
            .init_resource::<Safehouse>()
            .init_resource::<HourShift>()
            .add_systems(
                Update,
                (
                    find_cabin,
                    surge_ends_claim,
                    clear,
                    bar,
                    press,
                    claim,
                    watch_attention,
                    homecoming,
                )
                    .chain(),
            )
            .add_systems(Update, furnish);
    }
}

/// The cabin and its door, once the buildings are up; and its stove and bed
/// as things to press.
fn find_cabin(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    mut house: ResMut<Safehouse>,
    cabins: Query<(Entity, &Building, &GlobalTransform)>,
    doors: Query<(Entity, &ChildOf), With<DoorLeaf>>,
) {
    if house.cabin.is_some() {
        return;
    }
    let Some(at) = safehouse(&opts.map) else {
        return;
    };
    let Some((cabin, _, _)) = cabins.iter().find(|(_, b, gt)| {
        b.prefab == "cabin_log" && gt.translation().xz().distance(Vec2::from_array(at)) < 1.0
    }) else {
        return;
    };
    house.cabin = Some(cabin);
    house.door = doors
        .iter()
        .find(|(_, c)| c.parent() == cabin)
        .map(|(e, _)| e);
    for (thing, at, size) in [
        (SafehouseThing::Stove, STOVE, Vec3::new(0.7, 0.2, 0.6)),
        (SafehouseThing::Bed, BED, Vec3::new(0.9, 0.2, 1.9)),
        (SafehouseThing::Radio, RADIO, Vec3::new(0.25, 0.15, 0.15)),
    ] {
        commands.spawn((
            Name::new(format!("Safehouse {thing:?}")),
            thing,
            Pressable::Setting,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            Collider::cuboid(size.x, size.y, size.z),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            Transform::from_translation(at),
            Visibility::default(),
            ChildOf(cabin),
        ));
    }
}

/// The Surge ends a claim, and empties the crate with it.
fn surge_ends_claim(
    mut commands: Commands,
    raid: Res<Raid>,
    mut house: ResMut<Safehouse>,
    mut shift: ResMut<HourShift>,
    mut bags: Query<&mut Backpack, With<SafehouseCrate>>,
) {
    if house.state.cycle == raid.cycle {
        return;
    }
    let was = house.state.claimed();
    house.state.surge(raid.cycle);
    shift.0 = 0;
    if let Some(bag) = house.crate_bag.take() {
        if let Ok(mut b) = bags.get_mut(bag) {
            for e in b.contents.drain(..) {
                commands.entity(e).despawn();
            }
        }
        commands.entity(bag).despawn();
    }
    if was {
        info!("the Surge ended the safehouse's claim");
    }
}

fn player_at(players: &Query<&GlobalTransform, With<LocalPlayer>>) -> Option<Vec3> {
    players.iter().next().map(|p| p.translation())
}

/// Inside, with no machine close: cleared.
fn clear(
    mut house: ResMut<Safehouse>,
    mut raid: ResMut<Raid>,
    cabins: Query<&GlobalTransform>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
    robots: Query<&GlobalTransform, With<Robot>>,
) {
    if house.state.cleared || house.state.broken {
        return;
    }
    let (Some(cabin), Some(me)) = (house.cabin, player_at(&players)) else {
        return;
    };
    let Ok(c) = cabins.get(cabin) else {
        return;
    };
    let centre = c.translation();
    if me.xz().distance(centre.xz()) > INSIDE {
        return;
    }
    if robots
        .iter()
        .any(|r| r.translation().distance(centre) < CLEAR_RADIUS)
    {
        return;
    }
    house.state.cleared = true;
    raid.say(lines::SAFE_CLEARED);
}

/// A wedge in its door: barred.
fn bar(
    mut house: ResMut<Safehouse>,
    mut raid: ResMut<Raid>,
    wedged: Query<(), With<crate::robots::doors::Wedged>>,
) {
    if house.state.barred || house.state.broken {
        return;
    }
    if house.door.is_some_and(|d| wedged.contains(d)) {
        house.state.barred = true;
        raid.say(lines::SAFE_BARRED);
    }
}

/// The stove (with matches on you), the bed, the radio.
#[allow(clippy::too_many_arguments)]
fn press(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    things: Query<&SafehouseThing>,
    mut house: ResMut<Safehouse>,
    mut raid: ResMut<Raid>,
    mut shift: ResMut<HourShift>,
    carried: Query<(Entity, &Item, &CarriedBy)>,
    catalog: Res<Catalog>,
    wedged: Query<(), With<crate::robots::doors::Wedged>>,
    mut attention: Query<&mut Attention>,
    mut director: Option<ResMut<crate::robots::director::Director>>,
    doors: Query<&GlobalTransform, With<DoorLeaf>>,
    mut slept: MessageWriter<Slept>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, player } = ev else {
            continue;
        };
        let Ok(thing) = things.get(*item) else {
            continue;
        };
        match thing {
            SafehouseThing::Stove if house.state.lit => raid.say(lines::SAFE_STOVE_LIT),
            SafehouseThing::Stove => {
                let matches = carried
                    .iter()
                    .find(|(_, i, by)| by.0 == *player && catalog.get(i.kind()).id == "matches");
                match matches {
                    Some((e, ..)) => {
                        commands.entity(e).despawn();
                        house.state.lit = true;
                        raid.say(lines::SAFE_LIT);
                    }
                    None => raid.say(lines::SAFE_NO_MATCHES),
                }
            }
            SafehouseThing::Radio if !house.state.claimed() => raid.say(lines::SAFE_NOT_YET),
            SafehouseThing::Radio => {
                house.truck_waits = true;
                raid.say(lines::SAFE_TRUCK_WAITS);
            }
            SafehouseThing::Bed => {
                if !house.state.claimed() {
                    raid.say(lines::SAFE_NOT_YET);
                    continue;
                }
                if raid.phase == Phase::AtBase {
                    continue;
                }
                if !house.door.is_some_and(|d| wedged.contains(d)) {
                    raid.say(lines::SAFE_WEDGE_FIRST);
                    continue;
                }
                if raid.surge_in() < NIGHT + 120.0 {
                    raid.say(lines::SAFE_SURGE_CLOSE);
                    continue;
                }
                let feeling = attention
                    .get(*player)
                    .map_or(0, |a| a.feeling() as u8)
                    .max(house.peak.min(2));
                sleep(&mut raid, &mut house, &mut shift);
                slept.write(Slept);
                if feeling >= 2 {
                    // They've followed you: something at the door before
                    // light.
                    raid.say(lines::SAFE_NIGHT_VISIT);
                    let at = house
                        .door
                        .and_then(|d| doors.get(d).ok())
                        .map(|g| g.translation());
                    if let (Some(d), Some(at)) = (director.as_mut(), at) {
                        d.queue_response(20.0, at);
                    }
                } else {
                    raid.say(lines::SAFE_WOKE);
                }
                // A night behind a wedged door: the Zone forgets you a little.
                if let Ok(mut a) = attention.get_mut(*player) {
                    a.noise = 0.0;
                }
            }
        }
    }
}

/// A night passes: the clock, the hour, the truck.
pub fn sleep(raid: &mut Raid, house: &mut Safehouse, shift: &mut HourShift) {
    raid.clock += NIGHT;
    shift.0 = shift.0.wrapping_add(1);
    house.state.nights += 1;
    house.nights_out += 1;
    // The truck waits the night if it was told to; otherwise it's gone.
    match raid.phase {
        Phase::TruckWaiting { rv, .. } | Phase::TruckCalled { rv, .. } if house.truck_waits => {
            raid.phase = Phase::TruckWaiting {
                rv,
                left: TRUCK_WAIT,
            };
        }
        Phase::TruckWaiting { .. } | Phase::TruckCalled { .. } => {
            raid.phase = Phase::Out;
            raid.say(lines::SAFE_TRUCK_LEFT);
        }
        _ => {}
    }
    house.truck_waits = false;
}

/// All three done: claimed. The crate and the radio come with it.
fn claim(mut commands: Commands, mut house: ResMut<Safehouse>, mut raid: ResMut<Raid>) {
    if !house.state.claimed() || house.crate_bag.is_some() {
        return;
    }
    let Some(cabin) = house.cabin else {
        return;
    };
    let bag = spawn_crate(&mut commands, cabin);
    house.crate_bag = Some(bag);
    if house.state.stash.is_empty() && house.state.nights == 0 {
        raid.say(lines::SAFE_CLAIMED);
    }
}

fn spawn_crate(commands: &mut Commands, cabin: Entity) -> Entity {
    commands
        .spawn((
            Name::new("Safehouse crate"),
            SafehouseCrate,
            Backpack {
                owner: Entity::PLACEHOLDER,
                open: true,
                model: extraction_core::inventory::Backpack {
                    capacity: 60.0,
                    mass: 0.0,
                },
                contents: Vec::new(),
                carried: false,
                half: Vec3::new(0.35, 0.2, 0.25),
            },
            Transform::from_translation(CRATE),
            Visibility::default(),
            ChildOf(cabin),
        ))
        .id()
}

/// Fill a crate just put down with what was saved in it (once).
fn furnish(world: &mut World) {
    let Some(bag) = world.resource::<Safehouse>().crate_bag else {
        return;
    };
    if world.get::<Backpack>(bag).is_none() {
        return;
    }
    let stash = std::mem::take(&mut world.resource_mut::<Safehouse>().state.stash);
    if stash.is_empty() {
        return;
    }
    let player = world
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(world)
        .next();
    if let (Some(p), Some(mut b)) = (player, world.get_mut::<Backpack>(bag)) {
        b.owner = p;
    }
    for saved in &stash {
        if let Some(e) = crate::persist::restore_item(world, saved, Transform::IDENTITY) {
            crate::persist::put_in_bag(world, e, bag, Vec3::from_array(saved.local));
        }
    }
}

/// How loud the raid has got, for whether they find the safehouse.
fn watch_attention(
    raid: Res<Raid>,
    mut house: ResMut<Safehouse>,
    attention: Query<&Attention, With<LocalPlayer>>,
) {
    if raid.phase == Phase::AtBase {
        return;
    }
    let now = attention
        .iter()
        .map(|a| a.feeling() as u8)
        .max()
        .unwrap_or(0);
    if now > house.peak {
        house.peak = now;
    }
}

/// Home: a loud raid has led them to the safehouse. Its door's broken and
/// its crate tipped out over the floor.
fn homecoming(
    mut commands: Commands,
    mut raid: ResMut<Raid>,
    mut house: ResMut<Safehouse>,
    mut bags: Query<&mut Backpack, With<SafehouseCrate>>,
    places: Query<&GlobalTransform>,
    mut was_out: Local<bool>,
) {
    let out = raid.phase != Phase::AtBase;
    let home = !out && *was_out;
    *was_out = out;
    if !home {
        return;
    }
    let peak = std::mem::take(&mut house.peak);
    house.nights_out = 0;
    house.truck_waits = false;
    if !house.state.claimed() || peak < extraction_core::safehouse::FOUND_AT {
        return;
    }
    // Tip the crate: its contents fall loose where they were.
    if let Some(bag) = house.crate_bag.take() {
        if let Ok(mut b) = bags.get_mut(bag) {
            for e in b.contents.drain(..) {
                let at = places
                    .get(e)
                    .map_or(Transform::IDENTITY, |g| g.compute_transform());
                commands
                    .entity(e)
                    .remove::<(
                        ChildOf,
                        crate::interact::body::InBackpack,
                        RigidBodyDisabled,
                        ColliderDisabled,
                    )>()
                    .insert((
                        at.with_translation(at.translation + Vec3::Y * 0.3),
                        Visibility::Inherited,
                    ));
            }
        }
        commands.entity(bag).despawn();
    }
    house.state.found(peak);
    raid.say(lines::SAFE_FOUND);
}

#[cfg(test)]
mod tests;
