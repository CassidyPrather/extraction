//! The raid: the truck, the clock, getting out.
//!
//! There are no menus between raids. You start in the back of the truck,
//! parked at the south gate. Step out and the raid clock starts. Use the
//! radio to call the truck to a rendezvous on the far side; it takes a while
//! to drive round. Climb into the back and bang on the cab (or press the
//! dash button) and you're out: the truck drives home and everything you
//! carry is yours. Go down and the memorial convoy comes for you instead.
//! Stay too long and the Surge comes for everyone.

pub mod bench;
pub mod board;
pub mod captions;
pub mod chatter;
pub mod comfort_board;
pub mod epilogue;
pub mod kitchen;
pub mod lev;
pub mod lev_walk;
pub mod lines;
pub mod logbook;
pub mod requests;
pub mod route;
pub mod slowed;
pub mod truck;

use bevy::prelude::*;
use extraction_core::map::MarkerKind;

use crate::health::{Downed, Health};
use crate::player::Player;
use crate::rig::LocalPlayer;
use crate::world::Zone;

/// Seconds of Zone time (it only runs while you're out) between Surges:
/// two or three raids' worth.
pub const SURGE_AFTER: f32 = 60.0 * 60.0;
/// How long the Surge itself rages.
pub const SURGE_LENGTH: f32 = 120.0;
/// At base with less than this to go, pulling the bell sleeps it out.
pub const SLEEP_WINDOW: f32 = 15.0 * 60.0;
/// Seconds of warning before it.
pub const SURGE_WARNING: f32 = 5.0 * 60.0;
/// The last call before the Surge.
pub const SURGE_LAST: f32 = 60.0;
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
    /// The machines were recalled (the capstone): no more robots, in any
    /// valley.
    pub recalled: bool,
    /// Surge cycles survived by this Zone (each one re-rolls the loot).
    pub cycle: u32,
    pub extractions: u32,
    pub deaths: u32,
    /// Messages for the HUD / radio chatter: (text, seconds left to show).
    pub log: Vec<(String, f32)>,
    /// The truck's answer to a call, said once its drive is settled (the
    /// yard and the bots shorten it after the call): the line with its
    /// `{t}` still to fill, and whether a frame has passed.
    pub pending_call: Option<(String, bool)>,
    /// What was said lately, as [`claim`]s, with seconds left before it
    /// may be said again.
    pub said_lately: Vec<(String, f32)>,
}

/// How long a thing said is held back from being said again.
pub const REPEAT_HOLD: f32 = 30.0;

/// What a line claims, for telling a repeat: its last sentence when that
/// says something by itself (four words or more: "Next: the lamp by the
/// pen."), so two ways into the same news are one claim; else the whole
/// line ("Copy." alone isn't news, the line around it is).
pub fn claim(line: &str) -> String {
    let t = line.trim().trim_end_matches(['"', '\'']).trim();
    let body = t.trim_end_matches(['.', '!', '?']);
    let last = body
        .rfind(['.', '!', '?'])
        .map_or(body, |i| &body[i + 1..])
        .trim();
    let words = last.split_whitespace().count();
    let key = if words >= 4 { last } else { t };
    key.to_lowercase()
}

impl Raid {
    /// Say a line on the radio and the wrist. The same news twice inside
    /// [`REPEAT_HOLD`] is said once: the second time adds nothing and
    /// reads as the game stuttering.
    pub fn say(&mut self, text: impl Into<String>) {
        let t = text.into();
        let key = claim(&t);
        if self.said_lately.iter().any(|(k, _)| *k == key) {
            debug!("raid (said lately): {t}");
            return;
        }
        info!("raid: {t}");
        self.said_lately.push((key, REPEAT_HOLD));
        self.log.push((t, 8.0));
    }

    pub fn surge_in(&self) -> f32 {
        SURGE_AFTER - self.clock
    }

    /// How much of the Surge is in the air, 0..=1, for sky, light and
    /// sound: a creeping wrongness through the warning, all of it while it
    /// rages, an aftertaste until the Zone resets.
    pub fn surge_level(&self) -> f32 {
        if self.phase == Phase::AtBase {
            return 0.0;
        }
        let t = self.surge_in();
        if t > SURGE_WARNING {
            0.0
        } else if t > 0.0 {
            0.4 * (1.0 - t / SURGE_WARNING).powf(1.5)
        } else if t > -SURGE_LENGTH {
            1.0
        } else {
            let after = -SURGE_LENGTH - t;
            0.12 + 0.88 * (-after / 30.0).exp()
        }
    }

    /// The Surge is raging right now.
    pub fn surging(&self) -> bool {
        let t = self.surge_in();
        self.phase != Phase::AtBase && t <= 0.0 && t > -SURGE_LENGTH
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
            recalled: false,
            cycle: 0,
            extractions: 0,
            deaths: 0,
            pending_call: None,
            said_lately: Vec::new(),
            log: Vec::new(),
        })
        .add_message::<RadioCall>()
        .add_message::<DepartRequest>()
        .add_message::<truck::DropEverything>()
        .add_plugins(slowed::SlowedPlugin)
        .add_plugins((
            truck::TruckPlugin,
            bench::BenchPlugin,
            kitchen::KitchenPlugin,
            route::RoutePlugin,
            comfort_board::ComfortBoardPlugin,
            crate::camp::CampPlugin,
            crate::capstone::CapstonePlugin,
            chatter::ChatterPlugin,
            captions::CaptionsPlugin,
            logbook::LogbookPlugin,
            requests::RequestsPlugin,
            board::BoardPlugin,
            lev::LevPlugin,
            epilogue::EpiloguePlugin,
        ))
        .add_systems(
            Update,
            (leave_base, radio_calls, tick_raid, depart, downed).chain(),
        )
        .add_systems(PostUpdate, drop_everything)
        .add_systems(PostStartup, dev_surge_clock);
    }
}

/// `--surge-in N`: wind the Zone clock for testing the Surge.
fn dev_surge_clock(opts: Res<crate::launch::LaunchOptions>, mut raid: ResMut<Raid>) {
    if let Some(n) = opts.surge_in {
        raid.clock = SURGE_AFTER - n;
    }
}

/// Stepping out of the truck's pull-in starts the clock.
fn leave_base(
    mut raid: ResMut<Raid>,
    // Their own poses, not the propagated ones: waking in the truck after
    // going down moves both in the same frame, and the stale global poses
    // read the player as still out where they fell (so the raid restarted
    // with you sitting in the truck).
    players: Query<&Transform, With<LocalPlayer>>,
    trucks: Query<&Transform, With<truck::Truck>>,
) {
    if raid.phase != Phase::AtBase {
        return;
    }
    let (Some(p), Some(t)) = (players.iter().next(), trucks.iter().next()) else {
        return;
    };
    if p.translation.distance(t.translation) > 14.0 {
        raid.phase = Phase::Out;
        raid.raids += 1;
        let mins = (raid.surge_in() / 60.0).max(0.0).ceil();
        raid.say(format!(
            "Out of the truck. The Surge is due in about {mins:.0} minutes."
        ));
    }
}

fn rendezvous(zone: &Zone) -> Vec<(String, [f32; 2])> {
    zone.map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| (m.name.clone(), m.at))
        .collect()
}

/// The radio calls the truck to the rendezvous nearest the caller.
/// Rendezvous index meaning "back at the base": the short exfil.
pub const SHORT_EXFIL: usize = usize::MAX;

fn radio_calls(
    mut commands: Commands,
    mut calls: MessageReader<RadioCall>,
    mut raid: ResMut<Raid>,
    zone: Res<Zone>,
    players: Query<&GlobalTransform>,
    mut stashes: Query<&mut crate::interact::body::Backpack, With<truck::Stash>>,
    items: Query<&crate::items::Item>,
    catalog: Res<crate::items::Catalog>,
    progression: Option<Res<crate::progression::Progression>>,
) {
    // A career's first job comes home from the gate it went out by, free:
    // the way back is the way it came, not across the valley to a
    // rendezvous (`docs/FIRST_HOUR.md`).
    let first_job = progression.is_some_and(|p| p.0.done == 0);
    for call in calls.read() {
        match raid.phase {
            Phase::Out => {
                let Ok(p) = players.get(call.player) else {
                    continue;
                };
                let here = [p.translation().x, p.translation().z];
                // Near the gate you came in by, the truck can come straight
                // back for you, if there's fuel in the stash to burn.
                let at_short = zone
                    .map
                    .markers_of(&MarkerKind::ShortExfil)
                    .any(|m| extraction_core::geom::dist(m.at, here) < 40.0);
                if at_short {
                    let mut paid = first_job;
                    for mut stash in &mut stashes {
                        let can = stash.contents.iter().copied().find(|e| {
                            items
                                .get(*e)
                                .is_ok_and(|i| catalog.get(i.kind()).has_tag("fuel"))
                        });
                        if let Some(can) = can.filter(|_| !paid) {
                            stash.contents.retain(|e| *e != can);
                            commands.entity(can).despawn();
                            paid = true;
                            break;
                        }
                    }
                    if paid {
                        let n = raid.clock as u32;
                        raid.say(if first_job {
                            lines::FIRST_SHORT_RUN
                        } else {
                            lines::pick(lines::SHORT_RUN, n)
                        });
                        raid.phase = Phase::TruckWaiting {
                            rv: SHORT_EXFIL,
                            left: TRUCK_WAIT,
                        };
                        continue;
                    }
                    raid.say(lines::NO_FUEL);
                }
                let rvs = rendezvous(&zone);
                let Some((i, (name, _))) = rvs.iter().enumerate().min_by(|a, b| {
                    extraction_core::geom::dist(a.1.1, here)
                        .total_cmp(&extraction_core::geom::dist(b.1.1, here))
                }) else {
                    continue;
                };
                let n = raid.clock as u32;
                let b = lines::way(here, rvs[i].1);
                // Said when the drive is known (`tick_raid`): how long is
                // its own `{t}`.
                raid.pending_call = Some((
                    lines::fill(lines::pick(lines::CALLED, n), name, 0.0, &b),
                    false,
                ));
                raid.phase = Phase::TruckCalled {
                    rv: i,
                    eta: TRUCK_TRANSIT,
                };
            }
            Phase::TruckCalled { eta, .. } => {
                let n = raid.clock as u32;
                raid.say(lines::fill_time(lines::pick(lines::ON_THE_WAY, n), eta))
            }
            Phase::TruckWaiting { rv, .. } => {
                let (name, at) = rendezvous(&zone).get(rv).cloned().unwrap_or_else(|| {
                    let gate = zone
                        .map
                        .markers_of(&MarkerKind::ShortExfil)
                        .next()
                        .map_or([0.0, 0.0], |m| m.at);
                    ("gate".into(), gate)
                });
                let b = players
                    .get(call.player)
                    .map(|p| lines::way([p.translation().x, p.translation().z], at))
                    .unwrap_or_default();
                let n = raid.clock as u32;
                raid.say(lines::fill(lines::pick(lines::WAITING, n), &name, 0.0, &b));
            }
            _ => {}
        }
    }
}

pub(crate) fn tick_raid(
    mut raid: ResMut<Raid>,
    time: Res<Time>,
    zone: Res<Zone>,
    mut healths: Query<
        (
            &mut Health,
            &GlobalTransform,
            Option<&crate::anomaly::artifacts::ArtifactEffects>,
        ),
        With<LocalPlayer>,
    >,
    spatial: avian3d::prelude::SpatialQuery,
) {
    let dt = time.delta_secs();
    for l in &mut raid.log {
        l.1 -= dt;
    }
    raid.log.retain(|l| l.1 > 0.0);
    for l in &mut raid.said_lately {
        l.1 -= dt;
    }
    raid.said_lately.retain(|l| l.1 > 0.0);
    match raid.phase.clone() {
        Phase::AtBase | Phase::Extracting { .. } | Phase::Down { .. } => {
            raid.pending_call = None;
            return;
        }
        Phase::TruckCalled { rv, eta } => {
            // The call's answer, a frame on: whatever shortens the drive
            // has, and it says the time the truck will take.
            if let Some((line, aged)) = raid.pending_call.take() {
                if aged {
                    raid.say(lines::fill_time(&line, eta));
                } else {
                    raid.pending_call = Some((line, true));
                }
            }
            let eta = eta - dt;
            if eta <= 0.0 {
                let name = rendezvous(&zone)
                    .get(rv)
                    .map(|r| r.0.clone())
                    .unwrap_or_default();
                let n = raid.clock as u32;
                raid.say(lines::fill_time(
                    &lines::fill(lines::pick(lines::ARRIVED, n), &name, 0.0, ""),
                    TRUCK_WAIT,
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
                let n = raid.clock as u32;
                raid.say(lines::pick(lines::GAVE_UP, n));
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
        raid.say(format!(
            "The sky is going the wrong colour. {} until the Surge.",
            lines::time_words(SURGE_WARNING)
        ));
    }
    if before > SURGE_LAST && after <= SURGE_LAST {
        raid.say(format!(
            "{}. Get out, or get under something.",
            lines::time_words(SURGE_LAST)
        ));
    }
    if before > 0.0 && after <= 0.0 {
        raid.say("The Surge is here.");
    }
    if before > -SURGE_LENGTH && after <= -SURGE_LENGTH {
        raid.say("It's passing. The air tastes of pennies.");
    }
    if after <= 0.0 && after > -SURGE_LENGTH {
        for (mut h, gt, fx) in &mut healths {
            // Not survivable in the open; under a roof, barely. A
            // stormglass carried live is a roof of its own.
            let head = gt.translation() + Vec3::Y * 1.7;
            let roof = spatial
                .cast_ray(
                    head,
                    Dir3::Y,
                    25.0,
                    true,
                    &avian3d::prelude::SpatialQueryFilter::from_mask(crate::world::Layer::World),
                )
                .is_some()
                || fx.is_some_and(|f| f.stormglass);
            h.hp -= if roof { 0.6 } else { 15.0 } * dt;
        }
    }
}

/// The bell in the truck bed: leave from a rendezvous; at base, sleep out
/// the Surge, or drive to the valley on the route card.
fn depart(
    mut requests: MessageReader<DepartRequest>,
    mut raid: ResMut<Raid>,
    players: Query<(&GlobalTransform, &crate::rig::RigInput), With<Player>>,
    trucks: Query<(&GlobalTransform, &truck::Truck)>,
    dest: Option<Res<route::Destination>>,
    opts: Res<crate::launch::LaunchOptions>,
    mut relocate: MessageWriter<route::Relocate>,
) {
    for req in requests.read() {
        let Ok((p, input)) = players.get(req.player) else {
            continue;
        };
        let at = crate::player::standing_at(p, input) + Vec3::Y * 0.1;
        let in_bed = trucks.iter().any(|(t, truck)| truck.in_bed(t, at));
        if !in_bed {
            continue;
        }
        match raid.phase {
            Phase::TruckWaiting { .. } => {
                raid.extractions += 1;
                let n = raid.extractions;
                raid.say(lines::pick(lines::PULLING_OUT, n));
                raid.phase = Phase::Extracting { left: 4.0 };
            }
            // Another valley on the card: drive there through the night.
            Phase::AtBase if dest.as_ref().is_some_and(|d| d.id() != opts.map) => {
                let d = dest.as_ref().unwrap();
                raid.say(format!(
                    "Driver: \"{}, then. Long drive. Get some sleep.\"",
                    d.name()
                ));
                relocate.write(route::Relocate {
                    to: d.id().to_owned(),
                });
            }
            Phase::AtBase if raid.surge_in() < SLEEP_WINDOW => {
                // Sleep it out in the cab: the Surge comes and goes, and
                // `persist` resets the Zone.
                raid.say("You bed down in the cab and wait for the Surge to pass.");
                raid.clock = SURGE_AFTER + SURGE_LENGTH;
            }
            Phase::AtBase => {
                let mins = raid.surge_in() / 60.0;
                raid.say(format!(
                    "Driver: \"Surge isn't due for {mins:.0} minutes. Go on, get out there.\""
                ));
            }
            _ => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_short_exfil_burns_a_can_of_fuel() {
        assert!(short_exfil_burns_the_can(1), "the can was burnt");
    }

    #[test]
    fn the_first_jobs_short_run_is_on_the_house() {
        assert!(!short_exfil_burns_the_can(0), "the can was kept");
    }

    /// Calls the truck to the short exfil with a can in the stash, `done` jobs
    /// into the career; true when the can went.
    fn short_exfil_burns_the_can(done: u32) -> bool {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let stash = app
            .world_mut()
            .query_filtered::<Entity, With<truck::Stash>>()
            .iter(app.world())
            .next()
            .unwrap();
        let kind = catalog.kind("fuel_can").unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let can = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::IDENTITY,
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.world_mut()
            .get_mut::<crate::interact::body::Backpack>(stash)
            .unwrap()
            .contents
            .push(can);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let zone = app.world().resource::<Zone>().clone();
        let m = zone.marker(&MarkerKind::ShortExfil).unwrap().at;
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = zone.on_ground(m[0], m[1] - 20.0);
        app.update();
        app.update();
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .done = done;
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.world_mut().write_message(RadioCall { player });
        app.update();
        assert!(matches!(
            app.world().resource::<Raid>().phase,
            Phase::TruckWaiting {
                rv: SHORT_EXFIL,
                ..
            }
        ));
        app.world().get_entity(can).is_err()
    }

    /// The truck says how long it'll be, and that's how long it is: called
    /// far from the gate, a drive cut short after the call (as the yard and
    /// the bots cut it) is what the radio says, not the usual two minutes.
    #[test]
    fn the_truck_says_its_real_time() {
        for (cut, want) in [(None, "Two minutes"), (Some(10.0), "Ten seconds")] {
            let opts = crate::LaunchOptions {
                robots: false,
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            if let Some(cut) = cut {
                app.add_systems(Update, move |mut raid: ResMut<Raid>| {
                    if let Phase::TruckCalled { eta, .. } = &mut raid.phase {
                        *eta = eta.min(cut);
                    }
                });
            }
            app.finish();
            app.cleanup();
            app.update();
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            // Out in the valley, far from the gate's short run.
            let zone = app.world().resource::<Zone>().clone();
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = zone.on_ground(0.0, -150.0);
            app.update();
            app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
            app.world_mut().write_message(RadioCall { player });
            for _ in 0..4 {
                app.update();
            }
            let raid = app.world().resource::<Raid>();
            let said: Vec<&str> = raid
                .log
                .iter()
                .map(|(l, _)| l.as_str())
                .filter(|l| l.starts_with("Truck:"))
                .collect();
            assert_eq!(said.len(), 1, "{said:?}");
            assert!(said[0].contains(want), "{cut:?}: {said:?}");
        }
    }

    /// The same news twice in a row is said once: two ways into one claim
    /// ("Good. Next: the lamp." then "Off you go. Next: the lamp.") are
    /// one line; it can be said again once REPEAT_HOLD has passed; and a
    /// line that differs only in a word too short to be news ("Copy.") is
    /// still the same line, so it's held too.
    #[test]
    fn the_same_news_is_said_once() {
        assert_eq!(
            claim("Driver: \"Good. Next: the lamp by the pen.\""),
            claim("Driver: \"Off you go. Next: the lamp by the pen.\"")
        );
        assert_ne!(
            claim("Truck: \"Copy. Ten seconds.\""),
            claim("Truck: \"Ten seconds.\"")
        );
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let said = |app: &App| {
            app.world()
                .resource::<Raid>()
                .log
                .iter()
                .filter(|(l, _)| l.contains("Next: the lamp"))
                .count()
        };
        let before = app.world().resource::<Raid>().log.len();
        let mut raid = app.world_mut().resource_mut::<Raid>();
        raid.say("Driver: \"Good. Next: the lamp by the pen.\"");
        raid.say("Driver: \"Off you go. Next: the lamp by the pen.\"");
        assert_eq!(said(&app), 1, "said once");
        assert!(app.world().resource::<Raid>().log.len() > before);
        // Near the end of the hold, a second of the game runs it out.
        for l in &mut app.world_mut().resource_mut::<Raid>().said_lately {
            l.1 = l.1.min(0.5);
        }
        for _ in 0..60 {
            app.update();
        }
        let mut raid = app.world_mut().resource_mut::<Raid>();
        raid.log.clear();
        raid.say("Driver: \"Off you go. Next: the lamp by the pen.\"");
        assert_eq!(said(&app), 1, "and again, later");
    }
}
