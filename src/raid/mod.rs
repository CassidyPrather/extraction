//! The raid: the truck, the clock, getting out.
//!
//! There are no menus between raids. You start in the back of the truck,
//! parked at the south gate. Step out and the raid clock starts. Use the
//! radio to call the truck to a rendezvous on the far side; it takes a while
//! to drive round. Climb into the back and bang on the cab (or press the
//! dash button) and you're out: the truck drives home and everything you
//! carry is yours. Go down and the memorial convoy comes for you instead.
//! Stay too long and the Surge comes for everyone.

pub mod truck;

use bevy::prelude::*;
use extraction_core::map::MarkerKind;

use crate::health::{Downed, Health};
use crate::player::Player;
use crate::rig::LocalPlayer;
use crate::world::Zone;

/// Seconds from leaving the truck until the Surge.
pub const SURGE_AFTER: f32 = 45.0 * 60.0;
/// Seconds of warning before it.
pub const SURGE_WARNING: f32 = 5.0 * 60.0;
/// Seconds for the truck to drive round to a rendezvous.
pub const TRUCK_TRANSIT: f32 = 120.0;
/// Seconds the truck waits at a rendezvous before giving up.
pub const TRUCK_WAIT: f32 = 300.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// In or by the truck at base; the clock isn't running.
    AtBase,
    /// Out in the Zone.
    Out,
    /// Truck called to rendezvous `rv`; arrives in `eta` seconds.
    TruckCalled { rv: usize, eta: f32 },
    /// Truck at rendezvous `rv`, waiting `left` more seconds.
    TruckWaiting { rv: usize, left: f32 },
    /// Extracted: fading out, driving home.
    Extracting { left: f32 },
    /// Down. The convoy is coming.
    Down { left: f32 },
}

#[derive(Resource, Debug)]
pub struct Raid {
    pub phase: Phase,
    /// Seconds since stepping out of the truck.
    pub clock: f32,
    pub raids: u32,
    pub extractions: u32,
    pub deaths: u32,
    /// Messages for the HUD / radio chatter: (text, seconds left to show).
    pub log: Vec<(String, f32)>,
}

impl Raid {
    pub fn say(&mut self, text: impl Into<String>) {
        let t = text.into();
        info!("raid: {t}");
        self.log.push((t, 8.0));
    }

    pub fn surge_in(&self) -> f32 {
        SURGE_AFTER - self.clock
    }
}

/// The local player used the radio (flat: U with the radio in hand; VR:
/// primary button while holding it).
#[derive(Message, Clone, Copy, Debug)]
pub struct RadioCall {
    pub player: Entity,
}

/// The player asked to leave (in the truck bed: the dash button).
#[derive(Message, Clone, Copy, Debug)]
pub struct DepartRequest {
    pub player: Entity,
}

pub struct RaidPlugin;

impl Plugin for RaidPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Raid {
            phase: Phase::AtBase,
            clock: 0.0,
            raids: 0,
            extractions: 0,
            deaths: 0,
            log: Vec::new(),
        })
        .add_message::<RadioCall>()
        .add_message::<DepartRequest>()
        .add_message::<truck::DropEverything>()
        .add_plugins(truck::TruckPlugin)
        .add_systems(
            Update,
            (leave_base, radio_calls, tick_raid, depart, downed).chain(),
        )
        .add_systems(PostUpdate, drop_everything);
    }
}

/// Stepping out of the truck's pull-in starts the clock.
fn leave_base(
    mut raid: ResMut<Raid>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
    trucks: Query<&GlobalTransform, With<truck::Truck>>,
) {
    if raid.phase != Phase::AtBase {
        return;
    }
    let (Some(p), Some(t)) = (players.iter().next(), trucks.iter().next()) else {
        return;
    };
    if p.translation().distance(t.translation()) > 14.0 {
        raid.phase = Phase::Out;
        raid.clock = 0.0;
        raid.raids += 1;
        raid.say("Out of the truck. The clock is running.");
    }
}

fn rendezvous(zone: &Zone) -> Vec<(String, [f32; 2])> {
    zone.map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| (m.name.clone(), m.at))
        .collect()
}

/// The radio calls the truck to the rendezvous nearest the caller.
fn radio_calls(
    mut calls: MessageReader<RadioCall>,
    mut raid: ResMut<Raid>,
    zone: Res<Zone>,
    players: Query<&GlobalTransform>,
) {
    for call in calls.read() {
        match raid.phase {
            Phase::Out => {
                let Ok(p) = players.get(call.player) else {
                    continue;
                };
                let here = [p.translation().x, p.translation().z];
                let rvs = rendezvous(&zone);
                let Some((i, (name, _))) = rvs.iter().enumerate().min_by(|a, b| {
                    extraction_core::geom::dist(a.1.1, here)
                        .total_cmp(&extraction_core::geom::dist(b.1.1, here))
                }) else {
                    continue;
                };
                raid.say(format!(
                    "Truck: \"Copy. Heading for the {name}. Give me two minutes.\""
                ));
                raid.phase = Phase::TruckCalled {
                    rv: i,
                    eta: TRUCK_TRANSIT,
                };
            }
            Phase::TruckCalled { eta, .. } => {
                raid.say(format!("Truck: \"Still on my way. {:.0} seconds.\"", eta))
            }
            Phase::TruckWaiting { rv, .. } => {
                let name = rendezvous(&zone)
                    .get(rv)
                    .map(|r| r.0.clone())
                    .unwrap_or_default();
                raid.say(format!("Truck: \"I'm at the {name}. Get in the back.\""));
            }
            _ => {}
        }
    }
}

fn tick_raid(
    mut raid: ResMut<Raid>,
    time: Res<Time>,
    zone: Res<Zone>,
    mut healths: Query<&mut Health, With<LocalPlayer>>,
) {
    let dt = time.delta_secs();
    for l in &mut raid.log {
        l.1 -= dt;
    }
    raid.log.retain(|l| l.1 > 0.0);
    match raid.phase.clone() {
        Phase::AtBase | Phase::Extracting { .. } | Phase::Down { .. } => return,
        Phase::TruckCalled { rv, eta } => {
            let eta = eta - dt;
            if eta <= 0.0 {
                let name = rendezvous(&zone)
                    .get(rv)
                    .map(|r| r.0.clone())
                    .unwrap_or_default();
                raid.say(format!(
                    "Truck: \"At the {name}. I'll wait five minutes, no more.\""
                ));
                raid.phase = Phase::TruckWaiting {
                    rv,
                    left: TRUCK_WAIT,
                };
            } else {
                raid.phase = Phase::TruckCalled { rv, eta };
            }
        }
        Phase::TruckWaiting { rv, left } => {
            let left = left - dt;
            if left <= 0.0 {
                raid.say("Truck: \"Can't stay. Call me again when you're close.\"");
                raid.phase = Phase::Out;
            } else {
                raid.phase = Phase::TruckWaiting { rv, left };
            }
        }
        Phase::Out => {}
    }
    let before = raid.surge_in();
    raid.clock += dt;
    let after = raid.surge_in();
    if before > SURGE_WARNING && after <= SURGE_WARNING {
        raid.say("The sky is going the wrong colour. Five minutes until the Surge.");
    }
    if before > 60.0 && after <= 60.0 {
        raid.say("One minute. Get out, or get under something.");
    }
    if after <= 0.0 {
        for mut h in &mut healths {
            // The Surge is not survivable in the open.
            h.hp -= 15.0 * dt;
        }
    }
}

/// Leaving: allowed from the truck bed when the truck is at a rendezvous
/// (or back at base, for a short exfil that costs fuel — later).
fn depart(
    mut requests: MessageReader<DepartRequest>,
    mut raid: ResMut<Raid>,
    players: Query<&GlobalTransform, With<Player>>,
    trucks: Query<(&GlobalTransform, &truck::Truck)>,
) {
    for req in requests.read() {
        let Ok(p) = players.get(req.player) else {
            continue;
        };
        let in_bed = trucks
            .iter()
            .any(|(t, truck)| truck.in_bed(t, p.translation()));
        if !in_bed {
            continue;
        }
        if let Phase::TruckWaiting { .. } = raid.phase {
            raid.extractions += 1;
            raid.say("Truck: \"Hold on to something.\" You're out.");
            raid.phase = Phase::Extracting { left: 4.0 };
        }
    }
}

fn downed(
    mut events: MessageReader<Downed>,
    mut raid: ResMut<Raid>,
    locals: Query<(), With<LocalPlayer>>,
) {
    for ev in events.read() {
        if locals.contains(ev.player) {
            raid.deaths += 1;
            raid.say("Everything goes grey. Somewhere, a collection order is filed.");
            raid.phase = Phase::Down { left: 6.0 };
        }
    }
}

/// What a downed player leaves behind: a bag of everything they carried.
#[derive(Component)]
pub struct Remains {
    pub owner: Entity,
}

/// Moves everything a player carried into a remains bag where they fell.
pub(crate) fn drop_everything(world: &mut World) {
    let requests: Vec<truck::DropEverything> = world
        .resource_mut::<Messages<truck::DropEverything>>()
        .drain()
        .collect();
    for req in requests {
        let player = req.player;
        let mut items: Vec<Entity> = Vec::new();
        // Held.
        let mut q = world.query::<(Entity, &crate::interact::Held)>();
        items.extend(
            q.iter(world)
                .filter(|(_, h)| h.player == player)
                .map(|(e, _)| e),
        );
        // Body slots.
        let mut q = world.query::<(&mut crate::interact::Socket, &crate::interact::SocketOwner)>();
        for (mut s, o) in q.iter_mut(world) {
            if o.0 == player {
                if let Some(e) = s.occupant.take() {
                    items.push(e);
                }
            }
        }
        // Worn backpack.
        let mut q = world.query::<&mut crate::interact::body::Backpack>();
        for mut b in q.iter_mut(world) {
            if b.owner == player && b.carried {
                items.append(&mut b.contents);
                b.open = false;
            }
        }
        let mut q = world.query::<&mut crate::interact::Hand>();
        for mut h in q.iter_mut(world) {
            h.holding = h.holding.filter(|e| !items.contains(e));
            h.supporting = None;
            h.handle = None;
        }
        let bag = world
            .spawn((
                Name::new("Remains"),
                Remains { owner: player },
                crate::interact::body::Backpack {
                    owner: player,
                    open: true,
                    model: extraction_core::inventory::Backpack {
                        capacity: 10_000.0,
                        mass: 0.0,
                    },
                    contents: items.clone(),
                    carried: false,
                    half: Vec3::new(0.4, 0.2, 0.9),
                },
                Transform::from_translation(req.at + Vec3::Y * 0.25),
                Visibility::default(),
            ))
            .id();
        for (i, item) in items.iter().enumerate() {
            let a = i as f32 * 0.9;
            let local = Vec3::new(a.cos() * 0.25, 0.0, (i as f32 * 0.17) % 0.8 - 0.4);
            world
                .entity_mut(*item)
                .remove::<(
                    crate::interact::Held,
                    crate::interact::Supported,
                    crate::interact::InSocket,
                )>()
                .insert((
                    crate::interact::body::InBackpack { bag },
                    ChildOf(bag),
                    Transform::from_translation(local),
                    avian3d::prelude::RigidBodyDisabled,
                    avian3d::prelude::ColliderDisabled,
                    Visibility::Inherited,
                ));
        }
        info!("left {} items in remains at {:.0}", items.len(), req.at);
    }
}
