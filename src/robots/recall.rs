//! What the machines remember of you, and how you can tell they do.
//!
//! Every memory a robot keeps between one look at you and the next, or
//! the valley keeps between one raid and the next, is a row in one table
//! ([`rule`]), like the devices' (`devices::TABLE`): how it fades
//! ([`Decay`]) and what shows it ([`Tell`]). The numbers the rest of the
//! robots use come from here (`senses::SUSPICION_DECAY`,
//! `senses::BORED_FOR`, `disturbed::SIGN_LIFE`, `walker::KNOCKED_AGAIN`,
//! `drone::LEAVING_FIGHT`, `network::DISTRUST_WITHIN`). A memory with no
//! rule or no tell fails the tests. So does a robot-keyed `Local` map in
//! `robots/`, or a field of the structs memories live in (`Awareness`,
//! `RelayNet`, `Relay`, `ZoneMemory`), that the table doesn't name.
//!
//! Two tells are this module's own:
//!
//! - **Search marks** ([`SearchMark`]): a walker that searches somewhere
//!   sprays a ring and a cross on the ground where it stood. Bright while
//!   it's fresh (while that walker won't answer a lure there,
//!   `senses::BORED_FOR`), duller after. The valley keeps where they were
//!   (`ZoneMemory::searched`), so last raid's marks are there when you
//!   next step out, faded by a raid, and gone in five.
//! - **A relay's tally** ([`Filed`]): a relay that has reported you blinks
//!   red, once per report (up to three), then a pause; one that's no
//!   longer believed pulses amber. It stops when the memory does
//!   ([`FILED_FOR`]). Audio and Robot bodies can read [`Filed`] for the
//!   chirp and the look on relays this module doesn't draw a lamp on.
//!
//! Logic runs headless; the marks' and lamps' looks only where there are
//! meshes to make.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::network::Relay;
use super::{Robot, RobotKind};
use crate::raid::{Phase, Raid};
use crate::world::{Layer, Zone};

/// How a memory fades.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Decay {
    /// It falls by this much a second (it's 0..1).
    PerSecond(f32),
    /// It's let go this long after it was last fed (s).
    After(f32),
    /// Each raid it fades to `keep` of itself; under `forget` it's gone.
    PerRaid { keep: f32, forget: f32 },
    /// It lasts only while its cause does (what that is).
    While(&'static str),
}

/// What the player can see or hear of a memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tell {
    /// It says so: a voiced turn of mind (`search::MindShift`), the wrist.
    Voice,
    /// It comes and looks round where it last had you.
    Searches,
    /// Paint on the ground where it searched ([`SearchMark`]).
    Marks,
    /// A relay's tally lamp ([`Filed`]).
    Lamp,
    /// Someone is sent: a mender, an inspector, a team.
    Sent,
    /// It turns its scan on the thing, then its gun (`devices::Hunting`).
    Aims,
    /// It breaks off: backs away, climbs out (`walker::Retreating`).
    BreaksOff,
    /// What it does leans your way: patrols, a sentry's sweep, and the
    /// driver says so (`raid::lines::MEMORY_*`).
    Leans,
    /// A building you cleared is found shut.
    Shut,
    /// A watcher is left where you wrecked one of theirs.
    Watcher,
}

/// Where a memory is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// A field of a struct (`Awareness`, `RelayNet`, `ZoneMemory`, `Relay`).
    Field(&'static str, &'static str),
    /// A system's `Local` map, keyed by robot, in this file of `robots/`.
    Local(&'static str),
    /// A component of its own.
    Component(&'static str),
}

/// One memory's row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub what: &'static str,
    pub kept: Kept,
    pub decay: Option<Decay>,
    pub tell: Option<Tell>,
}

/// Everything a machine remembers of you.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Memory {
    /// How sure it is someone's there.
    Suspicion,
    /// Where it last had you, and how well.
    LastSeen,
    /// A place it searched and found nothing: it won't answer a lure there.
    Lure,
    /// The signs of you it has seen (`disturbed`).
    Sign,
    /// The devices of yours it has seen running.
    Device,
    /// The device it's going to shoot.
    Hunt,
    /// It was knocked down lately.
    Knocked,
    /// You went under a roof on a drone.
    Roofed,
    /// A relay has reported you.
    Reported,
    /// A relay fed a false report.
    FalseReport,
    /// A relay the network no longer believes.
    Distrust,
    /// A relay gone silent.
    Silence,
    /// A relay quiet in a jam.
    Jam,
    /// The valley: your usual ways.
    Ways,
    /// The valley: the rendezvous you favour.
    Rendezvous,
    /// The valley: where you wrecked its machines.
    Kills,
    /// The valley: buildings you cleared.
    Looted,
    /// The valley: where its walkers searched for you.
    Searched,
}

/// Every memory (an exhaustive match: a new one won't build until it's
/// here).
pub fn every_memory() -> Vec<Memory> {
    use Memory::*;
    let all = vec![
        Suspicion,
        LastSeen,
        Lure,
        Sign,
        Device,
        Hunt,
        Knocked,
        Roofed,
        Reported,
        FalseReport,
        Distrust,
        Silence,
        Jam,
        Ways,
        Rendezvous,
        Kills,
        Looted,
        Searched,
    ];
    for m in &all {
        match m {
            Suspicion | LastSeen | Lure | Sign | Device | Hunt | Knocked | Roofed | Reported
            | FalseReport | Distrust | Silence | Jam | Ways | Rendezvous | Kills | Looted
            | Searched => {}
        }
    }
    all
}

/// A relay that has reported you shows it this long after its last
/// report (s).
pub const FILED_FOR: f32 = 600.0;
/// A false report is held against its relay this long (s); a relay
/// distrusted for a second one is believed again this long after it.
const FALSE_FOR: f32 = 900.0;

const RAID: Decay = Decay::PerRaid {
    keep: extraction_core::zone_memory::DECAY,
    forget: extraction_core::zone_memory::FORGET,
};

/// The table: each memory's rule.
pub const fn rule(m: Memory) -> Rule {
    use Decay::*;
    use Kept::*;
    use Memory::*;
    let (what, kept, decay, tell) = match m {
        Suspicion => (
            "how sure it is",
            Field("Awareness", "suspicion"),
            PerSecond(0.05),
            Tell::Voice,
        ),
        LastSeen => (
            "where it last had you: it looks there, then gives up",
            Field("Awareness", "last_known"),
            After(super::search::INVESTIGATE_TIME + super::search::SEARCH_TIME),
            Tell::Searches,
        ),
        Lure => (
            "a place it searched for nothing: sounds nobody made there go unanswered",
            Local("senses.rs"),
            After(120.0),
            Tell::Marks,
        ),
        Sign => (
            "a sign of you it has seen",
            Local("disturbed.rs"),
            After(900.0),
            Tell::Searches,
        ),
        Device => (
            "a device of yours it has seen",
            Local("devices.rs"),
            While("the device runs"),
            Tell::Aims,
        ),
        Hunt => (
            "the device it's going to shoot",
            Component("Hunting"),
            While("the device is whole and in reach"),
            Tell::Aims,
        ),
        Knocked => (
            "knocked down lately: again and it backs off",
            Local("walker.rs"),
            After(10.0),
            Tell::BreaksOff,
        ),
        Roofed => (
            "you went under a roof: it leaves the fight",
            Local("drone.rs"),
            After(8.0),
            Tell::BreaksOff,
        ),
        Reported => (
            "a relay's reports of you",
            Component("Filed"),
            After(FILED_FOR),
            Tell::Lamp,
        ),
        FalseReport => (
            "a relay's false reports",
            Field("Relay", "false_reports"),
            After(FALSE_FOR),
            Tell::Sent,
        ),
        Distrust => (
            "a relay no longer believed",
            Field("RelayNet", "distrusted"),
            After(FALSE_FOR),
            Tell::Lamp,
        ),
        Silence => (
            "a relay gone silent",
            Field("RelayNet", "silenced"),
            While("its cabinet is torn"),
            Tell::Sent,
        ),
        Jam => (
            "a relay quiet in a jam",
            Local("network.rs"),
            While("a relay is in the jammer's reach"),
            Tell::Sent,
        ),
        Ways => (
            "the cells you walk",
            Field("ZoneMemory", "heat"),
            RAID,
            Tell::Leans,
        ),
        Rendezvous => (
            "the rendezvous you get out from",
            Field("ZoneMemory", "rvs"),
            RAID,
            Tell::Leans,
        ),
        Kills => (
            "where you wrecked its machines",
            Field("ZoneMemory", "kills"),
            RAID,
            Tell::Watcher,
        ),
        Looted => (
            "buildings you cleared",
            Field("ZoneMemory", "looted"),
            RAID,
            Tell::Shut,
        ),
        Searched => (
            "where its walkers searched",
            Field("ZoneMemory", "searched"),
            RAID,
            Tell::Marks,
        ),
    };
    Rule {
        what,
        kept,
        decay: Some(decay),
        tell: Some(tell),
    }
}

/// A memory's rate (`Decay::PerSecond`), for the code that keeps it.
pub const fn per_second(m: Memory) -> f32 {
    match rule(m).decay {
        Some(Decay::PerSecond(r)) => r,
        _ => panic!("not a per-second memory"),
    }
}

/// How long a memory lasts (`Decay::After`), for the code that keeps it.
pub const fn lasts(m: Memory) -> f32 {
    match rule(m).decay {
        Some(Decay::After(s)) => s,
        _ => panic!("not a timed memory"),
    }
}

/// Why a row isn't a memory the player can live with: no rule to fade
/// by, a rule that never fades, or nothing to show it.
pub fn check(m: Memory, r: &Rule) -> Result<(), String> {
    let Some(decay) = r.decay else {
        return Err(format!("{m:?} never fades: no decay rule"));
    };
    let fades = match decay {
        Decay::PerSecond(x) | Decay::After(x) => x.is_finite() && x > 0.0,
        Decay::PerRaid { keep, forget } => {
            (0.0..1.0).contains(&keep) && forget > 0.0 && forget < 1.0
        }
        Decay::While(cause) => !cause.is_empty(),
    };
    if !fades {
        return Err(format!("{m:?} never fades: {decay:?}"));
    }
    if r.tell.is_none() {
        return Err(format!("{m:?} has no tell: nothing shows it"));
    }
    Ok(())
}

// --- Search marks ------------------------------------------------------

/// Paint where a walker searched. `strength` is 1 this raid; last raid's
/// are as strong as the valley still remembers them.
#[derive(Component, Debug, Clone, Copy)]
pub struct SearchMark {
    pub at: Vec3,
    pub since: f32,
    pub strength: f32,
}

impl SearchMark {
    /// Still fresh: the walker that made it won't answer a lure here.
    pub fn fresh(&self, now: f32) -> bool {
        self.strength >= 1.0 && now - self.since < lasts(Memory::Lure)
    }

    /// How bright its paint is, 0..1.
    pub fn paint(&self, now: f32) -> f32 {
        if self.fresh(now) {
            1.0
        } else {
            0.7 * self.strength
        }
    }
}

/// A searcher closer than this to a mark (m) renews it, not another.
pub const SAME_MARK: f32 = 3.0;
/// At most this many marks this raid (the oldest go).
pub const MAX_MARKS: usize = 40;

// --- Relays ------------------------------------------------------------

/// A relay that has reported you: how often, and when it last did.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Filed {
    pub count: u32,
    pub last: f32,
}

/// What a relay's tally shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tally {
    /// Nothing to show.
    Dark,
    /// It's reported you this many times.
    Filed(u32),
    /// It's not believed.
    Distrusted,
}

/// How bright a tally is at `t` (0..1): red blinks, one per report up to
/// three, then a pause; amber pulsing for distrust.
pub fn tally(state: Tally, t: f32) -> f32 {
    match state {
        Tally::Dark => 0.0,
        Tally::Filed(n) => {
            let n = n.clamp(1, 3) as f32;
            let p = t.rem_euclid(n * 0.5 + 1.5);
            if p < n * 0.5 && p % 0.5 < 0.25 {
                1.0
            } else {
                0.0
            }
        }
        Tally::Distrusted => 0.5 + 0.5 * (t * std::f32::consts::TAU * 0.5).sin(),
    }
}

/// A pylon's tally lamp.
#[derive(Component, Debug)]
pub struct TallyLamp {
    pub relay: Entity,
}

/// Where the lamp sits on a pylon's cabinet (from its foot).
const LAMP_AT: Vec3 = Vec3::new(0.35, 1.25, 0.35);

pub struct RecallPlugin;

impl Plugin for RecallPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            (file_reports, forgive, mark_searches).chain(),
        )
        .add_systems(Update, (marks_turn, paint_marks, light_tallies));
    }
}

/// Reports that went out: the relay's tally goes up; one quiet this long
/// is let go.
fn file_reports(
    mut commands: Commands,
    director: Option<ResMut<super::director::Director>>,
    mut filed: Query<(Entity, &mut Filed)>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    for (e, f) in &filed {
        if now - f.last >= lasts(Memory::Reported) {
            commands.entity(e).remove::<Filed>();
        }
    }
    let Some(mut director) = director else {
        return;
    };
    for relay in std::mem::take(&mut director.net.filed) {
        match filed.get_mut(relay) {
            Ok((_, mut f)) => {
                f.count += 1;
                f.last = now;
            }
            Err(_) => {
                commands.entity(relay).try_insert(Filed {
                    count: 1,
                    last: now,
                });
            }
        }
    }
}

/// A relay distrusted is believed again once its false report is old
/// enough; a gone one is let go.
fn forgive(
    director: Option<ResMut<super::director::Director>>,
    relays: Query<Option<&Relay>>,
    time: Res<Time>,
) {
    let Some(mut director) = director else {
        return;
    };
    if director.net.distrusted.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    director.net.distrusted.retain(|e| match relays.get(*e) {
        Ok(Some(r)) => now - r.last_false < lasts(Memory::Distrust),
        // (Its record lands a tick after the distrust.)
        Ok(None) => true,
        Err(_) => false,
    });
}

/// A walker searching paints where it stands, and the valley remembers.
fn mark_searches(
    mut commands: Commands,
    mut searching: MessageReader<super::search::Searching>,
    robots: Query<&Robot>,
    mut marks: Query<(Entity, &mut SearchMark)>,
    mind: Option<ResMut<super::memory::ZoneMind>>,
    zone: Option<Res<Zone>>,
    spatial: SpatialQuery,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let mut mind = mind;
    for s in searching.read() {
        if !robots
            .get(s.robot)
            .is_ok_and(|r| r.kind == RobotKind::Walker)
        {
            continue;
        }
        let at = ground_at(s.at, zone.as_deref(), &spatial);
        if let Some(mind) = mind.as_mut() {
            mind.trace.searched.push([at.x, at.z]);
        }
        if let Some((_, mut m)) = marks
            .iter_mut()
            .find(|(_, m)| m.at.distance(at) < SAME_MARK)
        {
            *m = SearchMark {
                at: m.at,
                since: now,
                strength: 1.0,
            };
            continue;
        }
        let count = marks.iter().count();
        if count >= MAX_MARKS {
            if let Some((oldest, _)) = marks.iter().min_by(|a, b| a.1.since.total_cmp(&b.1.since)) {
                commands.entity(oldest).despawn();
            }
        }
        spawn_mark(&mut commands, at, now, 1.0);
    }
}

/// Where paint lands under `at`: the floor it stands on.
fn ground_at(at: Vec3, zone: Option<&Zone>, spatial: &SpatialQuery) -> Vec3 {
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    if let Some(hit) = spatial.cast_ray(at + Vec3::Y * 1.0, Dir3::NEG_Y, 4.0, true, &world) {
        return at + Vec3::Y * (1.0 - hit.distance);
    }
    zone.map_or(at, |z| z.on_ground(at.x, at.z))
}

fn spawn_mark(commands: &mut Commands, at: Vec3, now: f32, strength: f32) {
    commands.spawn((
        Name::new("Search mark"),
        SearchMark {
            at,
            since: now,
            strength,
        },
        Transform::from_translation(at + Vec3::Y * 0.02),
        Visibility::default(),
    ));
}

/// Stepping out: last raid's marks are where they were, as faded as the
/// valley's memory of them.
fn marks_turn(
    mut commands: Commands,
    raid: Option<Res<Raid>>,
    mind: Option<Res<super::memory::ZoneMind>>,
    marks: Query<Entity, With<SearchMark>>,
    zone: Option<Res<Zone>>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut last: Local<Option<Phase>>,
) {
    let Some(raid) = raid else {
        return;
    };
    let before = last.replace(raid.phase.clone());
    if !(matches!(before, Some(Phase::AtBase)) && raid.phase == Phase::Out) {
        return;
    }
    for e in &marks {
        commands.entity(e).despawn();
    }
    let Some(mind) = mind else {
        return;
    };
    let now = time.elapsed_secs();
    for (at, strength) in mind.memory.searched() {
        let Some(z) = zone.as_deref() else {
            break;
        };
        let p = ground_at(z.on_ground(at[0], at[1]), Some(z), &spatial);
        // Not fresh: older than a lure's welcome.
        spawn_mark(&mut commands, p, now - lasts(Memory::Lure), strength);
    }
}

/// The look of the marks: a sprayed ring and a cross, brighter fresh.
#[allow(clippy::type_complexity)]
fn paint_marks(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    new: Query<(Entity, &SearchMark), Without<MeshMaterial3d<StandardMaterial>>>,
    drawn: Query<(&SearchMark, &Children)>,
    paint: Query<&MeshMaterial3d<StandardMaterial>>,
    time: Res<Time>,
    mut shapes: Local<Option<(Handle<Mesh>, Handle<Mesh>)>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let now = time.elapsed_secs();
    let (ring, bar) = shapes
        .get_or_insert_with(|| {
            (
                meshes.add(Annulus::new(0.42, 0.52)),
                meshes.add(Rectangle::new(1.0, 0.08)),
            )
        })
        .clone();
    for (e, m) in &new {
        let mat = materials.add(StandardMaterial {
            base_color: paint_colour(m.paint(now)),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.9,
            depth_bias: 50.0,
            ..default()
        });
        let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        commands
            .entity(e)
            .insert(MeshMaterial3d(mat.clone()))
            .with_children(|c| {
                c.spawn((
                    Mesh3d(ring.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_rotation(flat),
                ));
                for a in [0.785f32, -0.785] {
                    c.spawn((
                        Mesh3d(bar.clone()),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_rotation(Quat::from_rotation_y(a) * flat),
                    ));
                }
            });
    }
    for (m, children) in &drawn {
        let Some(h) = children.iter().find_map(|c| paint.get(c).ok()) else {
            continue;
        };
        let want = paint_colour(m.paint(now));
        if let Some(mut mat) = materials.get_mut(&h.0) {
            if mat.base_color != want {
                mat.base_color = want;
            }
        }
    }
}

fn paint_colour(paint: f32) -> Color {
    Color::srgba(1.0, 0.42, 0.05, 0.85 * paint)
}

/// The pylons' tallies: a lamp on each cabinet, lit by what it remembers.
#[allow(clippy::type_complexity)]
fn light_tallies(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    pylons: Query<Entity, (With<super::pylon::Pylon>, Without<HasLamp>)>,
    lamps: Query<(&TallyLamp, &MeshMaterial3d<StandardMaterial>)>,
    filed: Query<&Filed>,
    director: Option<Res<super::director::Director>>,
    time: Res<Time>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    for p in &pylons {
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.06, 0.05),
            ..default()
        });
        let lamp = commands
            .spawn((
                Name::new("Tally lamp"),
                TallyLamp { relay: p },
                Mesh3d(meshes.add(Sphere::new(0.05))),
                MeshMaterial3d(mat),
                Transform::from_translation(LAMP_AT),
                ChildOf(p),
            ))
            .id();
        commands.entity(p).insert(HasLamp(lamp));
    }
    let t = time.elapsed_secs();
    for (lamp, mat) in &lamps {
        let distrusted = director
            .as_ref()
            .is_some_and(|d| d.net.distrusted.contains(&lamp.relay));
        let state = if distrusted {
            Tally::Distrusted
        } else if let Ok(f) = filed.get(lamp.relay) {
            Tally::Filed(f.count)
        } else {
            Tally::Dark
        };
        let glow = tally(state, t);
        let colour = match state {
            Tally::Distrusted => LinearRgba::rgb(4.0, 1.6, 0.0),
            _ => LinearRgba::rgb(5.0, 0.2, 0.1),
        };
        let want = colour * glow;
        if let Some(mut m) = materials.get_mut(&mat.0) {
            if m.emissive != want {
                m.emissive = want;
            }
        }
    }
}

/// A pylon with its tally lamp.
#[derive(Component, Debug)]
pub struct HasLamp(pub Entity);

#[cfg(test)]
mod tests {
    use super::*;

    /// Every memory fades by a rule and shows by a tell.
    #[test]
    fn every_memory_fades_and_shows() {
        for m in every_memory() {
            if let Err(e) = check(m, &rule(m)) {
                panic!("{e}");
            }
        }
    }

    /// The check turns away a memory with no rule, one that never fades,
    /// and one with nothing to show it.
    #[test]
    fn a_memory_without_a_rule_or_a_tell_fails() {
        let good = rule(Memory::Suspicion);
        assert!(check(Memory::Suspicion, &good).is_ok());
        let no_decay = Rule {
            decay: None,
            ..good
        };
        assert!(check(Memory::Suspicion, &no_decay).is_err());
        let forever = Rule {
            decay: Some(Decay::After(f32::INFINITY)),
            ..good
        };
        assert!(check(Memory::Suspicion, &forever).is_err());
        let never = Rule {
            decay: Some(Decay::PerRaid {
                keep: 1.0,
                forget: 0.1,
            }),
            ..good
        };
        assert!(check(Memory::Suspicion, &never).is_err());
        let unseen = Rule { tell: None, ..good };
        assert!(check(Memory::Suspicion, &unseen).is_err());
    }

    /// The numbers the robots use are the table's.
    #[test]
    fn the_robots_use_the_tables_numbers() {
        assert_eq!(
            super::super::senses::SUSPICION_DECAY,
            per_second(Memory::Suspicion)
        );
        assert_eq!(super::super::senses::BORED_FOR, lasts(Memory::Lure));
        assert_eq!(super::super::disturbed::SIGN_LIFE, lasts(Memory::Sign));
        assert_eq!(super::super::walker::KNOCKED_AGAIN, lasts(Memory::Knocked));
        assert_eq!(super::super::drone::LEAVING_FIGHT, lasts(Memory::Roofed));
        assert_eq!(
            super::super::network::DISTRUST_WITHIN,
            lasts(Memory::FalseReport)
        );
        assert_eq!(
            rule(Memory::Ways).decay,
            Some(Decay::PerRaid {
                keep: extraction_core::zone_memory::DECAY,
                forget: extraction_core::zone_memory::FORGET,
            })
        );
    }

    /// The fields of a struct's `Debug` (its default's): `Name { a: .., b: .. }`.
    fn fields(debug: &str) -> Vec<String> {
        let inner = &debug[debug.find('{').unwrap() + 1..debug.rfind('}').unwrap()];
        let mut out = vec![];
        let (mut depth, mut start) = (0, 0);
        let bytes: Vec<char> = inner.chars().collect();
        for (i, c) in bytes.iter().enumerate() {
            match c {
                '{' | '[' | '(' => depth += 1,
                '}' | ']' | ')' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(bytes[start..i].iter().collect::<String>());
                    start = i + 1;
                }
                _ => {}
            }
        }
        out.push(bytes[start..].iter().collect::<String>());
        out.iter()
            .filter_map(|f| f.split(':').next())
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect()
    }

    /// What's kept in these structs that isn't a memory of you, and why.
    const NOT_MEMORY: &[(&str, &str, &str)] = &[
        ("Awareness", "target", "who it's on: goes with last_known"),
        ("Awareness", "uncertainty", "how well it knows last_known"),
        (
            "Awareness",
            "unseen",
            "how long since: goes with last_known",
        ),
        ("Awareness", "can_see_target", "this tick"),
        ("Awareness", "exposure", "this tick"),
        ("Awareness", "news", "this tick"),
        ("RelayNet", "jams", "what's running now"),
        ("RelayNet", "blocked", "a count for a scenario"),
        ("RelayNet", "filed", "this tick's reports, for Filed"),
        ("Relay", "last_false", "when: goes with false_reports"),
        ("ZoneMemory", "raids", "a count"),
    ];

    /// Every field of the structs memories are kept in is a memory in the
    /// table, or said not to be.
    #[test]
    fn every_kept_field_is_in_the_table() {
        let structs = [
            (
                "Awareness",
                format!("{:?}", super::super::Awareness::default()),
            ),
            (
                "RelayNet",
                format!("{:?}", super::super::network::RelayNet::default()),
            ),
            ("Relay", format!("{:?}", Relay::default())),
            (
                "ZoneMemory",
                format!("{:?}", extraction_core::zone_memory::ZoneMemory::default()),
            ),
        ];
        let rows: Vec<Rule> = every_memory().into_iter().map(rule).collect();
        for (name, debug) in &structs {
            for f in fields(debug) {
                let row = rows
                    .iter()
                    .any(|r| matches!(r.kept, Kept::Field(s, n) if s == *name && n == f));
                let excused = NOT_MEMORY.iter().any(|(s, n, _)| s == name && *n == f);
                assert!(!(row && excused), "{name}.{f} is in the table: not excused");
                assert!(
                    row || excused,
                    "{name}.{f}: a memory with no row in recall::rule"
                );
            }
        }
        for r in &rows {
            if let Kept::Field(s, f) = r.kept {
                let (_, debug) = structs.iter().find(|(n, _)| *n == s).unwrap();
                assert!(
                    fields(debug).iter().any(|x| x == f),
                    "{s}.{f}: no such field"
                );
            }
        }
    }

    /// The robots' `Local` maps that aren't memories of you, by file.
    const NOT_MEMORY_LOCALS: &[(&str, &str)] = &[
        (
            "devices.rs",
            "how long each device has run (its, not a robot's)",
        ),
        ("drone.rs", "how long it's been firing: its aim settling"),
        ("fx.rs", "an effect's count"),
        ("fx.rs", "an effect's next puff"),
        ("mod.rs", "a wreck's loot timer"),
        ("search.rs", "a call's cooldown"),
        ("search.rs", "the wrist's line pacing"),
        ("search.rs", "the brain it had last tick"),
        ("search.rs", "where it was last looking, for LostYou"),
        ("senses.rs", "a door's last state, to see it open"),
        ("tracks.rs", "how often it looks for prints"),
        ("visuals.rs", "a material made"),
        ("visuals.rs", "a lamp's flicker"),
        ("walker.rs", "a burst through a wall"),
        ("walker.rs", "how long it's been firing: its aim settling"),
    ];

    /// Every robot-keyed `Local` map in `robots/` is a memory in the table
    /// or said not to be: a new one fails here until it has a rule and a
    /// tell, or a reason.
    #[test]
    fn every_robot_keyed_local_is_in_the_table() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/robots");
        let mut found: Vec<(String, usize)> = vec![];
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".rs") || name == "mind_tests.rs" {
                continue;
            }
            let text = std::fs::read_to_string(e.path()).unwrap();
            let n = locals_outside_tests(&text);
            if n > 0 {
                found.push((name, n));
            }
        }
        found.sort();
        let rows: Vec<Rule> = every_memory().into_iter().map(rule).collect();
        let mut files: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r.kept {
                Kept::Local(f) => Some(f),
                _ => None,
            })
            .chain(NOT_MEMORY_LOCALS.iter().map(|(f, _)| *f))
            .collect();
        files.sort();
        let mut want: Vec<(String, usize)> = vec![];
        for f in files {
            match want.last_mut() {
                Some((g, n)) if g == f => *n += 1,
                _ => want.push((f.to_string(), 1)),
            }
        }
        assert_eq!(
            found, want,
            "robot-keyed Locals by file (found, then the table's and the excused)"
        );
    }

    /// Lines with a `Local` map keyed by entity, outside test modules.
    fn locals_outside_tests(text: &str) -> usize {
        let mut n = 0;
        let mut lines = text.lines().peekable();
        while let Some(l) = lines.next() {
            if l.trim() == "#[cfg(test)]"
                && lines.peek().is_some_and(|m| {
                    m.trim_start().starts_with("mod ") && m.trim_end().ends_with('{')
                })
            {
                let mut depth = 0i32;
                for m in lines.by_ref() {
                    depth += m.matches('{').count() as i32 - m.matches('}').count() as i32;
                    if depth <= 0 && m.contains('}') {
                        break;
                    }
                }
                continue;
            }
            if l.contains("Local<")
                && ["HashMap<Entity", "HashSet<Entity", "BTreeMap<Entity"]
                    .iter()
                    .any(|k| l.contains(k))
            {
                n += 1;
            }
        }
        n
    }

    /// A report blinks once a pause; three, three; distrust pulses; dark
    /// is dark.
    #[test]
    fn a_tally_blinks_its_count() {
        let blinks = |state: Tally, period: f32| {
            let mut n = 0;
            let mut was = false;
            for k in 0..(period * 100.0) as usize {
                let on = tally(state, k as f32 * 0.01) > 0.5;
                if on && !was {
                    n += 1;
                }
                was = on;
            }
            n
        };
        assert_eq!(blinks(Tally::Filed(1), 2.0), 1);
        assert_eq!(blinks(Tally::Filed(3), 3.0), 3);
        assert_eq!(blinks(Tally::Filed(9), 3.0), 3, "three at most");
        assert_eq!(blinks(Tally::Dark, 3.0), 0);
        assert!(blinks(Tally::Distrusted, 2.0) >= 1);
    }
}
