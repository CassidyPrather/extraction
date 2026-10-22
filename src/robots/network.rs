//! The machines' own network, turned against them.
//!
//! What a relay (a running pylon, a spotter's winch, a sentry, the
//! airship) sees goes to the director as a report (`Director::queue_report`),
//! and a response team follows. A player can work on that:
//!
//! - **Cut a pylon's cable** ([`PylonCable`], bolt cutters): it goes
//!   silent, so its reports don't go out. Its tell is the silence itself.
//!   After [`SILENCE_NOTICED`] the machines send a mender to it, and once
//!   its cabinet is welded whole (the cut tore into it: [`CUT_HARM`]) it's
//!   wired up again.
//! - **Splice a salvaged radio into a relay** (let go of a `radio` against
//!   a running pylon's cabinet or a spotter's winch, [`SPLICE_REACH`]): it
//!   reports contact somewhere you aren't ([`decoy_place`]: a named place
//!   [`DECOY_FROM`] or more from you, the one nearest the relay), and the
//!   team goes there. The radio is gone into it. A second false report
//!   from the same relay inside [`DISTRUST_WITHIN`] isn't believed: the
//!   relay is distrusted from then on (none of its reports, true or false,
//!   go out), and a walker is sent to look it over.
//! - **A jammer** ([`Jammer`], powered: `gadgets::Powered`) blocks every
//!   report from a relay within its radius. But a relay gone quiet in a
//!   jam is itself a sign: after [`JAM_NOTICED`] a team is sent to find
//!   it, to roughly where it is ([`JAM_GUESS`]).
//!
//! Logic only: runs headless.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use super::director::Director;
use super::pylon::{Pylon, PylonState};
use super::{PartRole, Robot, RobotPart};
use crate::world::{Layer, Zone};

/// A silent pylon is noticed, and a mender sent, after this long (s).
pub const SILENCE_NOTICED: f32 = 45.0;
/// Cutting the cable tears into the cabinet this much (hp): a mender's
/// work to put right.
pub const CUT_HARM: f32 = 60.0;
/// A radio let go within this of a relay's cabinet is spliced in (m).
pub const SPLICE_REACH: f32 = 1.2;
/// A false report names a place at least this far from you (m)...
pub const DECOY_FROM: f32 = 150.0;
/// ...and goes out after this long (s).
pub const DECOY_DELAY: f32 = 20.0;
/// A second false report from one relay inside this long (s) isn't
/// believed.
pub const DISTRUST_WITHIN: f32 = 900.0;
/// A relay in a jam this long (s) gets a team sent to find it...
pub const JAM_NOTICED: f32 = 40.0;
/// ...to within this of the jammer (m)...
pub const JAM_GUESS: f32 = 25.0;
/// ...and not again for this long (s).
pub const JAM_AGAIN: f32 = 180.0;
/// A jammer's reach when nothing says otherwise (m).
pub const JAM_RADIUS: f32 = 60.0;

/// What the director's network knows of itself this tick.
#[derive(Debug, Default, Clone)]
pub struct RelayNet {
    /// Powered jammers: where, and how far they reach.
    pub jams: Vec<(Vec3, f32)>,
    /// Relays that have gone silent (a cut cable).
    pub silenced: HashSet<Entity>,
    /// Relays it no longer believes.
    pub distrusted: HashSet<Entity>,
}

impl RelayNet {
    /// Whether a report sent from `at` is jammed.
    pub fn jammed(&self, at: Vec3) -> bool {
        self.jams.iter().any(|(j, r)| j.distance(at) < *r)
    }
}

impl Director {
    /// A relay (`relay`, at `from`) reports someone at `at`: unless it's
    /// silent, distrusted or jammed, a response follows in `delay`. Says
    /// whether it went out.
    pub fn queue_report(&mut self, relay: Entity, from: Vec3, delay: f32, at: Vec3) -> bool {
        if self.net.silenced.contains(&relay)
            || self.net.distrusted.contains(&relay)
            || self.net.jammed(from)
        {
            return false;
        }
        self.queue_response(delay, at);
        true
    }
}

/// The cable a running pylon talks down: cut it and it's silent.
#[derive(Component, Debug, Clone, Copy)]
pub struct PylonCable {
    pub pylon: Entity,
}

/// A pylon gone silent: since when, and whether a mender's been sent.
#[derive(Component, Debug, Clone, Copy)]
pub struct Silenced {
    pub since: f32,
    pub sent: bool,
}

/// A relay's record of false reports.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct Relay {
    pub false_reports: u32,
    pub last_false: f32,
}

/// A jammer: blocks relays' reports within `radius` while powered.
#[derive(Component, Debug, Clone, Copy)]
pub struct Jammer {
    pub radius: f32,
}

/// Where a pylon's cable runs: down its cabinet's side to the ground.
pub fn cable_at(pylon: Vec3) -> Vec3 {
    pylon + Vec3::new(0.62, 0.35, 0.0)
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(name_jammers).add_systems(
            FixedUpdate,
            (
                jams,
                wire_pylons,
                cut_cables,
                mend_silence,
                splice,
                notice_jams,
            )
                .chain(),
        );
    }
}

/// A `jammer` item is a jammer.
fn name_jammers(
    add: On<Add, crate::items::Item>,
    items: Query<&crate::items::Item>,
    catalog: Option<Res<crate::items::Catalog>>,
    mut commands: Commands,
) {
    let (Ok(item), Some(catalog)) = (items.get(add.entity), catalog) else {
        return;
    };
    if catalog.get(item.kind()).id == "jammer" {
        commands
            .entity(add.entity)
            .try_insert(Jammer { radius: JAM_RADIUS });
    }
}

/// The powered jammers, into the director's picture.
fn jams(
    director: Option<ResMut<Director>>,
    jammers: Query<(&Jammer, &GlobalTransform), With<crate::gadgets::Powered>>,
) {
    let Some(mut director) = director else {
        return;
    };
    director.net.jams = jammers
        .iter()
        .map(|(j, g)| (g.translation(), j.radius))
        .collect();
}

/// A pylon gets its cable (and a cable whose pylon has gone goes too).
fn wire_pylons(
    mut commands: Commands,
    pylons: Query<(Entity, &Transform), (Added<Pylon>, Without<Silenced>)>,
    cables: Query<(Entity, &PylonCable)>,
    alive: Query<(), With<Pylon>>,
) {
    for (e, t) in &pylons {
        spawn_cable(&mut commands, e, t.translation);
    }
    for (c, cable) in &cables {
        if !alive.contains(cable.pylon) {
            commands.entity(c).despawn();
        }
    }
}

fn spawn_cable(commands: &mut Commands, pylon: Entity, at: Vec3) {
    commands.spawn((
        Name::new("Pylon cable"),
        PylonCable { pylon },
        crate::destruction::hooks::Cuttable { needs: 0.0 },
        // What the cutters' jaws find: a thin overlap-only box down the
        // cabinet's side, which nothing collides with or shoots.
        Sensor,
        Collider::cuboid(0.06, 0.7, 0.06),
        CollisionLayers::new(Layer::Hand, LayerMask::NONE),
        Transform::from_translation(cable_at(at)),
        Visibility::default(),
    ));
}

/// Cut: the pylon's silent, and its cabinet torn.
fn cut_cables(
    mut commands: Commands,
    mut cuts: MessageReader<crate::destruction::hooks::Cut>,
    cables: Query<&PylonCable>,
    mut parts: Query<&mut RobotPart>,
    director: Option<ResMut<Director>>,
    time: Res<Time>,
) {
    let Some(mut director) = director else {
        cuts.clear();
        return;
    };
    for c in cuts.read() {
        let Ok(cable) = cables.get(c.target) else {
            continue;
        };
        let p = cable.pylon;
        director.net.silenced.insert(p);
        commands.entity(p).try_insert(Silenced {
            since: time.elapsed_secs(),
            sent: false,
        });
        for mut part in &mut parts {
            if part.robot == p && part.role == PartRole::Core && part.alive() {
                part.hp = (part.hp - CUT_HARM).max(1.0);
            }
        }
        info!("network: a pylon's cable cut, it's silent");
    }
}

/// A silent pylon is noticed: a mender is sent (if none is out), and
/// once its cabinet's whole again it's wired up.
#[allow(clippy::too_many_arguments)]
fn mend_silence(
    mut commands: Commands,
    mut silent: Query<(Entity, &mut Silenced, &Transform, &Robot), With<Pylon>>,
    parts: Query<&RobotPart>,
    menders: Query<(), With<super::mender::Mender>>,
    director: Option<ResMut<Director>>,
    zone: Option<Res<Zone>>,
    time: Res<Time>,
) {
    let (Some(mut director), Some(zone)) = (director, zone) else {
        return;
    };
    let now = time.elapsed_secs();
    let live: HashSet<Entity> = silent.iter().map(|(e, ..)| e).collect();
    director.net.silenced.retain(|e| live.contains(e));
    for (e, mut s, t, robot) in &mut silent {
        if robot.wrecked {
            continue;
        }
        let whole = parts
            .iter()
            .filter(|p| p.robot == e && p.role == PartRole::Core)
            .all(|p| p.hp >= p.max_hp);
        if whole {
            commands.entity(e).remove::<Silenced>();
            director.net.silenced.remove(&e);
            spawn_cable(&mut commands, e, t.translation);
            info!("network: a pylon's wired up again");
            continue;
        }
        if !s.sent && now - s.since >= SILENCE_NOTICED && director.enabled {
            s.sent = true;
            if menders.is_empty() {
                let gate = super::director::gate(&zone);
                super::mender::spawn_mender(
                    &mut commands,
                    gate + Vec3::Y * extraction_core::mender::CRUISE,
                    gate + Vec3::Y * 6.0,
                );
            }
            info!("network: a silent pylon noticed; a mender sent");
        }
    }
}

/// Where a false report from `relay` sends them: of the named places at
/// least [`DECOY_FROM`] from `you`, the one nearest the relay (else a
/// point that far from you, beyond the relay).
pub fn decoy_place(zone: &Zone, relay: Vec3, you: Vec3) -> (Vec3, Option<String>) {
    let named = zone
        .map
        .markers
        .iter()
        .filter(|m| !m.name.is_empty())
        .map(|m| (zone.on_ground(m.at[0], m.at[1]), m.name.clone()))
        .filter(|(p, _)| p.xz().distance(you.xz()) >= DECOY_FROM)
        .min_by(|a, b| a.0.distance(relay).total_cmp(&b.0.distance(relay)));
    match named {
        Some((p, n)) => (p, Some(n)),
        None => {
            let away = (relay - you).with_y(0.0).normalize_or(Vec3::X);
            let p = you + away * DECOY_FROM;
            (zone.on_ground(p.x, p.z), None)
        }
    }
}

/// A radio let go against a relay's cabinet is spliced in: a false report.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn splice(
    mut commands: Commands,
    mut events: MessageReader<crate::interact::InteractEvent>,
    items: Query<(&crate::items::Item, &GlobalTransform)>,
    catalog: Res<crate::items::Catalog>,
    pylons: Query<(Entity, &Pylon, &Transform, &Robot)>,
    spotters: Query<(Entity, &super::spotter::Spotter, &Robot)>,
    mut relays: Query<&mut Relay>,
    players: Query<&GlobalTransform, With<crate::player::Player>>,
    director: Option<ResMut<Director>>,
    zone: Res<Zone>,
    time: Res<Time>,
) {
    let Some(mut director) = director else {
        events.clear();
        return;
    };
    let now = time.elapsed_secs();
    for ev in events.read() {
        let crate::interact::InteractEvent::Released { item, .. } = *ev else {
            continue;
        };
        let Ok((it, gt)) = items.get(item) else {
            continue;
        };
        if catalog.get(it.kind()).id != "radio" {
            continue;
        }
        let at = gt.translation();
        let relay = pylons
            .iter()
            .filter(|(_, p, _, r)| !r.wrecked && matches!(p.state, PylonState::Running { .. }))
            .map(|(e, _, t, _)| (e, t.translation + Vec3::Y * 0.6))
            .chain(
                spotters
                    .iter()
                    .filter(|(_, _, r)| !r.wrecked)
                    .map(|(e, s, _)| (e, s.winch)),
            )
            .filter(|(_, p)| p.distance(at) < SPLICE_REACH + 0.6)
            .min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)));
        let Some((relay, from)) = relay else {
            continue;
        };
        // Wired in: the radio's gone into it.
        commands.entity(item).despawn();
        let you = players
            .iter()
            .map(|g| g.translation())
            .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
            .unwrap_or(at);
        let mut record = relays.get_mut(relay).map(|r| *r).unwrap_or_default();
        let twice = record.false_reports > 0 && now - record.last_false < DISTRUST_WITHIN;
        record.false_reports += 1;
        record.last_false = now;
        commands.entity(relay).try_insert(record);
        if twice {
            director.net.distrusted.insert(relay);
            // Someone goes to look it over.
            let exit = super::director::gate(&zone);
            let post = zone.on_ground(from.x + 3.0, from.z + 3.0);
            super::walker::spawn_walker(
                &mut commands,
                &zone,
                exit,
                super::Job {
                    kind: super::JobKind::Watch,
                    site: post,
                    duration: 240.0,
                    exit,
                },
            );
            info!("network: a second false report from one relay; it's distrusted");
            continue;
        }
        let (place, name) = decoy_place(&zone, from, you);
        let sent = director.queue_report(relay, from, DECOY_DELAY, place);
        info!(
            "network: a spliced radio reports contact at {} ({})",
            name.as_deref().unwrap_or("nowhere named"),
            if sent { "sent" } else { "blocked" }
        );
    }
}

/// A relay gone quiet in a jam: after a while they come to find it.
#[allow(clippy::type_complexity)]
fn notice_jams(
    jammers: Query<(Entity, &Jammer, &GlobalTransform), With<crate::gadgets::Powered>>,
    pylons: Query<(&Transform, &Pylon, &Robot)>,
    spotters: Query<(&super::spotter::Spotter, &Robot)>,
    sentries: Query<(&super::sentry::Sentry, &Robot)>,
    director: Option<ResMut<Director>>,
    time: Res<Time>,
    mut quiet: Local<HashMap<Entity, (f32, f32)>>,
) {
    let Some(mut director) = director else {
        return;
    };
    let dt = time.delta_secs();
    quiet.retain(|e, _| jammers.contains(*e));
    let relays: Vec<Vec3> = pylons
        .iter()
        .filter(|(_, p, r)| !r.wrecked && matches!(p.state, PylonState::Running { .. }))
        .map(|(t, ..)| t.translation)
        .chain(
            spotters
                .iter()
                .filter(|(_, r)| !r.wrecked)
                .map(|(s, _)| s.winch),
        )
        .chain(
            sentries
                .iter()
                .filter(|(_, r)| !r.wrecked)
                .map(|(s, _)| s.foot),
        )
        .collect();
    for (e, j, g) in &jammers {
        let at = g.translation();
        let (held, again) = quiet.entry(e).or_insert((0.0, 0.0));
        *again -= dt;
        if relays.iter().any(|r| r.distance(at) < j.radius) {
            *held += dt;
        } else {
            *held = 0.0;
        }
        if *held >= JAM_NOTICED && *again <= 0.0 && director.enabled {
            *held = 0.0;
            *again = JAM_AGAIN;
            // Roughly where: a guess off it, the same guess for the same
            // jammer.
            let a = (e.to_bits() % 628) as f32 * 0.01;
            let guess = at + Vec3::new(a.cos(), 0.0, a.sin()) * JAM_GUESS * 0.6;
            let delay = director.rng.range(10.0, 20.0);
            director.queue_response(delay, guess);
            info!("network: a jam noticed; a team sent to find it");
        }
    }
}
