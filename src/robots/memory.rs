//! The Zone remembers you across raids (`extraction_core::zone_memory`):
//! what each raid shows of your habits, and what the machines make of it
//! when you next step out.
//!
//! **Traced** while you're out ([`trace_raid`]): the 30 m cells you walk,
//! the machines wrecked, the loot spots you clear, and the rendezvous you
//! get out from. Folded into the valley's memory when the raid ends (out,
//! or down), and saved with the valley (`persist`).
//!
//! **At the next raid's start** ([`raid_turns`]), gently, and capped:
//!
//! - **Patrols** lean toward your corridors: the director's survey drones
//!   and work-site walkers pick places near your usual ways up to
//!   [`PATROL_LEAN`] times as often ([`lean_pick`]).
//! - **A sentry's eye**: the one nearest your favourite rendezvous turns
//!   its sweep toward it, by at most a quarter of its arc
//!   ([`SENTRY_LEAN`]), from where it was set.
//! - **A cleared building is found shut**: up to [`SHUT_BUILDINGS`] you
//!   cleared lately have their doors shut, and, once it's a habit
//!   ([`WEDGE_AT`]), one wedged if there's another way in (never the only
//!   door).
//! - **A watcher**: where you've wrecked them more than once lately
//!   ([`WATCHER_AT`]), one of its kind (an unarmed walker, which calls
//!   the others; or a drone) is left watching for the start of the raid.
//!
//! **Said**: the driver remarks on the strongest of these as you set off,
//! and the radio, if you carry one, says when you're picked up on your
//! usual way ("contact again near the mill"), once a raid.
//!
//! All of it is scaled by `zone_memory::MEMORY` (the one knob); a fresh
//! career (a blank memory) changes nothing, the director's draws
//! included.

use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::noise::Rng;
use extraction_core::zone_memory::{RaidTrace, ZoneMemory};

use super::{Breed, PartDestroyed, PartRole, Robot, RobotKind};
use crate::raid::{Phase, Raid};
use crate::rig::LocalPlayer;
use crate::world::Zone;

/// A place on your usual ways is picked up to this many times as often.
pub const PATROL_LEAN: f32 = 1.5;
/// A sentry's sweep turns toward your rendezvous by at most this share of
/// its arc...
pub const SENTRY_LEAN: f32 = 0.25;
/// ...if it's within this of it (m).
pub const SENTRY_REACH: f32 = 220.0;
/// At most this many buildings you cleared are found shut.
pub const SHUT_BUILDINGS: usize = 2;
/// A remembered thing counts for this much (0..1, after the knob) or more.
pub const NOTABLE: f32 = 0.3;
/// A watcher is left only where you've killed lately and more than once
/// (two raids running, about): this strong.
pub const WATCHER_AT: f32 = 0.7;
/// One door of a cleared building is wedged only once it's a habit (three
/// raids running, about): this strong.
pub const WEDGE_AT: f32 = 0.9;
/// How long a watcher watches (s).
pub const WATCH_FOR: f32 = 240.0;
/// You're on your usual way where it's this habitual (0..1).
pub const USUAL_WAY: f32 = 0.6;
/// Where you are is traced this often (s).
pub const TRACE_EVERY: f32 = 1.0;

/// This valley's memory of you, and the raid being traced.
#[derive(Resource, Default, Debug)]
pub struct ZoneMind {
    pub memory: ZoneMemory,
    pub trace: RaidTrace,
}

/// A sentry's sweep as its marker set it (memory leans it from here).
#[derive(Component, Clone, Copy, Debug)]
pub struct HomeCentre(pub f32);

/// A door memory wedged shut (it's let go at the next raid's start).
#[derive(Component, Debug)]
pub struct ShutByMemory;

pub struct MemoryPlugin;

impl Plugin for MemoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ZoneMind>()
            .add_systems(FixedUpdate, trace_raid.after(super::perceive))
            .add_systems(Update, raid_turns);
    }
}

/// An index into `at`, picked as `rng.below` would, leaning toward places
/// near your usual ways. With nothing remembered there it's exactly
/// `rng.below(at.len())`: the same draw as before there was a memory.
pub fn lean_pick(rng: &mut Rng, at: &[[f32; 2]], mind: Option<&ZoneMind>) -> usize {
    let weights: Vec<f32> = at
        .iter()
        .map(|p| {
            1.0 + (PATROL_LEAN - 1.0) * mind.map_or(0.0, |m| m.memory.corridor_near(p[0], p[1]))
        })
        .collect();
    if weights.iter().all(|w| *w == 1.0) {
        return rng.below(at.len());
    }
    let total: f32 = weights.iter().sum();
    let mut t = rng.range(0.0, total);
    for (i, w) in weights.iter().enumerate() {
        if t < *w {
            return i;
        }
        t -= w;
    }
    at.len() - 1
}

fn out(phase: &Phase) -> bool {
    matches!(
        phase,
        Phase::Out | Phase::TruckCalled { .. } | Phase::TruckWaiting { .. }
    )
}

/// What this raid shows of you (see the module docs).
#[allow(clippy::too_many_arguments)]
fn trace_raid(
    mut mind: ResMut<ZoneMind>,
    raid: Res<Raid>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
    mut wrecked: MessageReader<PartDestroyed>,
    robots: Query<(&Robot, Option<&Breed>)>,
    mut grabs: MessageReader<crate::interact::InteractEvent>,
    places: Query<&GlobalTransform>,
    loot: Option<Res<crate::world::buildings::BuildingLoot>>,
    time: Res<Time>,
    mut next: Local<f32>,
) {
    if !out(&raid.phase) {
        wrecked.clear();
        grabs.clear();
        return;
    }
    let now = time.elapsed_secs();
    if now >= *next {
        *next = now + TRACE_EVERY;
        for gt in &players {
            let p = gt.translation();
            mind.trace.walked(p.x, p.z);
        }
    }
    for w in wrecked.read().filter(|w| w.role == PartRole::Core) {
        let kind = robots
            .get(w.robot)
            .map_or("Walker".to_string(), |(r, b)| super::robot_name(r.kind, b));
        mind.trace.kills.push(([w.at.x, w.at.z], kind));
    }
    if let Some(loot) = &loot {
        for ev in grabs.read() {
            let crate::interact::InteractEvent::Grabbed { item, .. } = *ev else {
                continue;
            };
            let Ok(p) = places.get(item).map(|g| g.translation()) else {
                continue;
            };
            if let Some(spot) = loot
                .0
                .iter()
                .find(|s| s.at.distance(p) < super::disturbed::LOOT_HAND)
            {
                mind.trace.looted.push([spot.at.x, spot.at.z]);
            }
        }
    }
}

/// How far a memory line looks for a place to name.
const MEMORY_PLACE_NEAR: f32 = 150.0;

fn place_near(map: &extraction_core::map::MapDef, at: [f32; 2]) -> Option<String> {
    place_within(map, at, MEMORY_PLACE_NEAR)
}

/// A place's name for a line: the nearest named place within `reach` ("near
/// the mill"), never a note's or a terminal's, or none. Every line that
/// names where something is goes through here.
pub(crate) fn place_within(
    map: &extraction_core::map::MapDef,
    at: [f32; 2],
    reach: f32,
) -> Option<String> {
    map.markers
        .iter()
        .filter(|m| m.kind.names_a_place() && !m.name.is_empty())
        .map(|m| {
            let d = Vec2::from(m.at).distance(Vec2::from(at));
            (m, d)
        })
        .filter(|(_, d)| *d < reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(m, _)| m.name.clone())
}

/// The `n`th way of saying a memory line, about `place` if it has one.
fn line(with: &[&str], without: &[&str], place: Option<&str>, n: u32) -> String {
    use crate::raid::lines::{fill, pick};
    match place {
        Some(p) => fill(pick(with, n), p, 0.0, ""),
        None => fill(pick(without, n), "", 0.0, ""),
    }
}

/// The raid's turns: its start (the machines lean on what they remember)
/// and its end (this raid goes into the memory).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn raid_turns(
    mut commands: Commands,
    mut mind: ResMut<ZoneMind>,
    mut raid: ResMut<Raid>,
    zone: Res<Zone>,
    (director, entrances): (
        Option<Res<super::director::Director>>,
        Option<Res<super::doors::Entrances>>,
    ),
    mut sentries: Query<(Entity, &mut super::sentry::Sentry, Option<&HomeCentre>)>,
    mut doors: Query<&mut crate::world::buildings::Door>,
    shut: Query<Entity, With<ShutByMemory>>,
    (mut noticed, players, radios, catalog): (
        MessageReader<super::Noticed>,
        Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
        Query<(
            Entity,
            &crate::items::Item,
            &crate::anomaly::artifacts::CarriedBy,
        )>,
        Res<crate::items::Catalog>,
    ),
    mut chatter: MessageWriter<crate::raid::chatter::RobotChatter>,
    (mut last, mut said): (Local<Option<Phase>>, Local<bool>),
) {
    let now = raid.phase.clone();
    let before = last.replace(now.clone());
    // Out at a rendezvous: remember which.
    if let (Some(Phase::TruckWaiting { rv, .. }), Phase::Extracting { .. }) = (&before, &now) {
        if let Some(m) = zone.map.markers_of(&MarkerKind::Rendezvous).nth(*rv) {
            mind.trace.rv = Some(m.at);
        }
    }
    // The end: out or down, the raid goes in.
    let ended = before.as_ref().is_some_and(out)
        && matches!(now, Phase::Extracting { .. } | Phase::Down { .. });
    if ended {
        let trace = std::mem::take(&mut mind.trace);
        mind.memory.fold(&trace);
    }
    // Picked up on your usual way: the radio says so, once a raid.
    if out(&now) && !*said {
        for n in noticed.read() {
            let Some((player, me)) = players
                .iter()
                .map(|(e, g)| (e, g.translation()))
                .min_by(|a, b| a.1.distance(n.at).total_cmp(&b.1.distance(n.at)))
            else {
                continue;
            };
            if mind.memory.corridor_near(me.x, me.z) < USUAL_WAY {
                continue;
            }
            let Some(radio) = radios
                .iter()
                .find(|(_, item, by)| by.0 == player && catalog.get(item.kind()).id == "radio")
                .map(|(e, ..)| e)
            else {
                continue;
            };
            *said = true;
            let place = place_near(&zone.map, [me.x, me.z]);
            raid.say(line(
                crate::raid::lines::MEMORY_CONTACT,
                crate::raid::lines::MEMORY_CONTACT_AWAY,
                place.as_deref(),
                mind.memory.raids,
            ));
            chatter.write(crate::raid::chatter::RobotChatter {
                radio,
                at: n.at,
                close: true,
            });
            break;
        }
    } else {
        noticed.clear();
    }
    // The start: stepping out of the truck.
    if !(matches!(before, Some(Phase::AtBase)) && now == Phase::Out) {
        return;
    }
    *said = false;
    // Last raid's doors are let go.
    for e in &shut {
        commands
            .entity(e)
            .remove::<(super::doors::Wedged, ShutByMemory)>();
    }
    let machines = director.as_ref().is_some_and(|d| d.enabled) && !raid.recalled;
    let mut remarks: Vec<String> = vec![];
    // A sentry's eye on your rendezvous.
    for (e, s, home) in &sentries {
        if home.is_none() {
            commands.entity(e).insert(HomeCentre(s.centre));
        }
    }
    let rv = mind.memory.favourite_rv().filter(|(_, s)| *s >= NOTABLE);
    if let Some((at, strength)) = rv {
        let nearest = sentries
            .iter_mut()
            .filter(|(_, s, _)| s.powered)
            .map(|(e, s, h)| {
                let d = Vec2::new(s.foot.x, s.foot.z).distance(Vec2::from(at));
                (e, s, h, d)
            })
            .filter(|t| t.3 < SENTRY_REACH)
            .min_by(|a, b| a.3.total_cmp(&b.3));
        if let Some((_, mut s, home, _)) = nearest {
            let home = home.map_or(s.centre, |h| h.0);
            let toward = extraction_core::sentry::bearing([s.foot.x, s.foot.z], at);
            let off = extraction_core::sentry::wrap_deg(toward - home);
            let most = s.arc * SENTRY_LEAN;
            s.centre =
                extraction_core::sentry::wrap_deg(home + (off * strength).clamp(-most, most));
        }
        if let Some(m) = zone
            .map
            .markers_of(&MarkerKind::Rendezvous)
            .find(|m| m.at == at)
        {
            remarks.push(line(
                crate::raid::lines::MEMORY_RV,
                crate::raid::lines::MEMORY_RV,
                Some(&m.name),
                mind.memory.raids,
            ));
        }
    }
    // Buildings you cleared, found shut.
    if let Some(entrances) = &entrances {
        let mut done: Vec<Entity> = vec![];
        for (at, strength) in mind.memory.looted() {
            if done.len() >= SHUT_BUILDINGS || strength < NOTABLE {
                break;
            }
            let p = zone.on_ground(at[0], at[1]) + Vec3::Y * 0.5;
            let Some(b) = entrances.building_at(p).filter(|b| !done.contains(b)) else {
                continue;
            };
            done.push(b);
            let ways: Vec<Entity> = entrances
                .doors
                .iter()
                .filter(|d| d.building == b)
                .map(|d| d.door)
                .collect();
            for (k, d) in ways.iter().enumerate() {
                if let Ok(mut door) = doors.get_mut(*d) {
                    door.open = false;
                }
                // One wedged, if there's another way in.
                if k == 0 && ways.len() > 1 && strength >= WEDGE_AT {
                    commands
                        .entity(*d)
                        .insert((super::doors::Wedged(*d), ShutByMemory));
                }
            }
            if done.len() == 1 {
                if let Some(place) = place_near(&zone.map, at) {
                    remarks.push(line(
                        crate::raid::lines::MEMORY_SHUT,
                        crate::raid::lines::MEMORY_SHUT,
                        Some(&place),
                        mind.memory.raids,
                    ));
                }
            }
        }
    }
    // A watcher where you wrecked one of theirs.
    if let Some((at, kind, strength)) = mind
        .memory
        .kill_spot()
        .filter(|k| k.2 >= WATCHER_AT && machines)
    {
        let site = zone.on_ground(at[0], at[1]);
        let exit = super::director::gate(&zone);
        let job = super::Job {
            kind: super::JobKind::Watch,
            site,
            duration: WATCH_FOR * (0.5 + 0.5 * strength),
            exit,
        };
        if kind.eq_ignore_ascii_case(&format!("{:?}", RobotKind::Drone)) {
            super::drone::spawn_drone(
                &mut commands,
                site + Vec3::Y * 15.0,
                super::Job {
                    kind: super::JobKind::Survey,
                    ..job
                },
            );
        } else {
            // It watches and calls the others; it doesn't fight you itself.
            super::walker::spawn_walker_with(&mut commands, &zone, site, job, false, 130.0);
        }
        remarks.insert(
            0,
            line(
                crate::raid::lines::MEMORY_WATCHER,
                crate::raid::lines::MEMORY_WATCHER_AWAY,
                place_near(&zone.map, at).as_deref(),
                mind.memory.raids,
            ),
        );
    }
    if remarks.is_empty() && mind.memory.heat.iter().any(|h| h.1 >= 1.5) {
        remarks.push(line(
            crate::raid::lines::MEMORY_WAYS,
            crate::raid::lines::MEMORY_WAYS,
            None,
            mind.memory.raids,
        ));
    }
    // The strongest, said once.
    if let Some(r) = remarks.into_iter().next() {
        raid.say(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With nothing remembered (no memory, or a blank one), the director
    /// draws exactly as it did before there was a memory.
    #[test]
    fn a_fresh_career_draws_as_before() {
        let places: Vec<[f32; 2]> = (0..7).map(|i| [i as f32 * 100.0, 50.0]).collect();
        let blank = ZoneMind::default();
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let mut c = Rng::new(42);
        for _ in 0..200 {
            let before = a.below(places.len());
            assert_eq!(lean_pick(&mut b, &places, None), before);
            assert_eq!(lean_pick(&mut c, &places, Some(&blank)), before);
        }
    }

    /// A place on your usual way is picked more often, but never more than
    /// PATROL_LEAN times as often as another.
    #[test]
    fn patrols_lean_toward_your_ways_but_only_so_far() {
        let places = [[45.0, 45.0], [1045.0, 45.0]];
        let mut mind = ZoneMind::default();
        for _ in 0..6 {
            let mut t = RaidTrace::default();
            t.walked(45.0, 45.0);
            mind.memory.fold(&t);
        }
        let mut rng = Rng::new(7);
        let mut hits = [0u32; 2];
        for _ in 0..4000 {
            hits[lean_pick(&mut rng, &places, Some(&mind))] += 1;
        }
        let ratio = hits[0] as f32 / hits[1] as f32;
        assert!(
            ratio > 1.0 + (PATROL_LEAN - 1.0) * 0.6,
            "it leans your way: {hits:?}"
        );
        assert!(ratio < PATROL_LEAN * 1.15, "but only so far: {hits:?}");
    }

    /// A memory line names a place, never a note or a terminal lying
    /// somewhere: anywhere in every valley, the name it says is a place's.
    #[test]
    fn memory_lines_name_places_not_notes() {
        use extraction_core::map::{MapDef, VALLEYS};
        for (id, text) in VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let things: Vec<&str> = map
                .markers
                .iter()
                .filter(|m| m.kind.text().is_some())
                .map(|m| m.name.as_str())
                .collect();
            let mut said = 0;
            for x in (-300..=300).step_by(10) {
                for z in (-300..=300).step_by(10) {
                    if let Some(name) = place_near(&map, [x as f32, z as f32]) {
                        said += 1;
                        let place = map
                            .markers
                            .iter()
                            .any(|m| m.kind.names_a_place() && m.name == name);
                        assert!(
                            place && !things.contains(&name.as_str()),
                            "{id}: a line at ({x}, {z}) names {name:?}, which isn't a place"
                        );
                    }
                }
            }
            assert!(said > 0, "{id}: no place is ever named");
        }
    }
}
