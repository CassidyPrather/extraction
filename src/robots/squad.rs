//! Machines on one contact work it together, and say so.
//!
//! Two or more machines fighting the same player within [`SQUAD_RADIUS`]
//! of them share a plan ([`plan_squads`]), each with a role
//! ([`SquadRole`]):
//!
//! - **Pin**: the first walker holds you from cover.
//! - **Flank**: the next works round to your side ([`walker::FLANK_ANGLE`]
//!   off the pinner's line), and any after the third, the other side.
//! - **Cut off**: the third makes for the way to your rendezvous (the one
//!   the valley knows you favour, else the nearest) and holds it.
//! - **Spot**: a drone hangs over the flanker's way in and lights it.
//!
//! And a relay that reports you while they're on you **calls** it in.
//!
//! A machine **changing post holds its fire** until it's there
//! ([`repositioning`]): a flanker going round, a cut-off on its way, the
//! spotter climbing to its station. That's the window a listener gets.
//!
//! Every change of role is said aloud ([`SquadCall`]): a chirp between
//! the machines (Audio), a line on a radio you carry when it's about you
//! and near ("one going round to your east"), and the spotter's light.
//! A player who listens knows where the flanker's coming from and which
//! way is being shut. One switch ([`Coordination`]) turns all of it off
//! for the sims: each machine then fights alone.

use bevy::prelude::*;

use super::{Awareness, Brain, Robot, RobotKind};
use crate::raid::{Phase, Raid};
use crate::world::Zone;
use extraction_core::map::MarkerKind;

/// Machines fighting one player within this of them (m) are a squad.
pub const SQUAD_RADIUS: f32 = 60.0;
/// The plan is looked at again this often (s).
pub const PLAN_EVERY: f32 = 0.5;
/// The cut-off stands this far out from you toward your rendezvous (m),
/// or halfway, if it's nearer.
pub const CUT_OFF: f32 = 30.0;
/// The rendezvous it guards is the favourite the valley knows if that's
/// within this of you (m); else the nearest.
pub const FAVOURITE_REACH: f32 = 400.0;
/// A spotting drone hangs this high over the flanker's way in (m).
pub const SPOT_HEIGHT: f32 = 14.0;
/// A radio says at most one squad call this often (s)...
pub const RADIO_EVERY: f32 = 2.0;
/// ...and one that has waited this long (s) goes unsaid.
pub const RADIO_STALE: f32 = 8.0;
/// Squad calls about you are on your radio within this (m).
pub const RADIO_REACH: f32 = crate::raid::chatter::CLOSE;
/// A machine changing post holds its fire until it's this near the post
/// (m): a flanker or a cut-off on its way, the spotter climbing to its
/// station. "One going round" is one gun fewer, for as long as it takes.
pub const ON_POST: f32 = 4.0;
/// The spotter's station counts as reached this near it (m: it hovers).
pub const ON_STATION: f32 = 6.0;

/// Whether a machine at `at` with `role` is on its way to its post, and so
/// holds fire.
pub fn repositioning(role: Option<&SquadRole>, at: Vec3) -> bool {
    match role {
        Some(r) if matches!(r.role, Role::Flank | Role::CutOff) => {
            Vec2::new(r.post.x - at.x, r.post.z - at.z).length() > ON_POST
        }
        Some(r) if r.role == Role::Spot => r.post.distance(at) > ON_STATION,
        _ => false,
    }
}

/// A flanker's or the spotter's line names a place within this of where
/// it's going (m).
pub const PLACE_NEAR: f32 = 40.0;
/// Their chirps between machines carry this far (m): near enough, you
/// hear them without a radio.
pub const CHIRP_REACH: f32 = 40.0;
/// The scenario counter for calls made.
pub const CALLS: &str = "squad_calls";

/// A machine's part in the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Pin,
    Flank,
    CutOff,
    Spot,
    /// A relay, calling a response in.
    Call,
}

/// A machine's role on one contact, and where that puts it.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SquadRole {
    pub role: Role,
    pub target: Entity,
    /// Where the role puts it (the pinner: where it is).
    pub post: Vec3,
}

/// A machine took a new role: for its chirp, the radio, and the bots.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct SquadCall {
    pub robot: Entity,
    pub role: Role,
    /// Where it is.
    pub at: Vec3,
    /// Where it's going.
    pub post: Vec3,
    /// Whom it's about.
    pub target: Entity,
}

/// The calls a radio has said, and when (s): what a player with one has
/// heard, for bots that act on it. The last [`AIRED_KEPT`].
#[derive(Resource, Default, Debug)]
pub struct Aired(pub Vec<(SquadCall, f32)>);

pub const AIRED_KEPT: usize = 32;

/// Whether machines share a plan at all (the sims' other arm).
#[derive(Resource, Clone, Copy, Debug)]
pub struct Coordination {
    pub on: bool,
}

impl Default for Coordination {
    fn default() -> Self {
        Self { on: true }
    }
}

pub struct SquadPlugin;

impl Plugin for SquadPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SquadCall>()
            .init_resource::<Coordination>()
            .init_resource::<Aired>()
            .add_systems(
                FixedPostUpdate,
                (plan_squads, relays_call, count_calls).chain(),
            )
            .add_systems(Update, (radio_calls, light_spotters));
    }
}

/// One machine in a squad, for the plan.
#[derive(Clone, Copy, Debug)]
pub struct Member {
    pub e: Entity,
    pub at: Vec3,
    pub drone: bool,
    pub seed: u64,
    /// The role it holds already, if any: a plan keeps its parts where it
    /// can, rather than swapping them as machines join.
    pub holds: Option<Role>,
}

/// Who's first for a part: whoever holds it already, then by seed.
fn order(m: &Member) -> (u8, u64, Entity) {
    let held = match m.holds {
        Some(Role::Pin) => 0,
        Some(Role::Flank) => 1,
        Some(Role::CutOff) => 2,
        Some(Role::Spot) => 0,
        _ => 3,
    };
    (held, m.seed, m.e)
}

/// The plan for one contact at `tp`, guarding `rv` (if there is one):
/// each member's role and post. Fewer than two, no plan.
pub fn plan(members: &[Member], tp: Vec3, rv: Option<Vec3>) -> Vec<(Entity, Role, Vec3)> {
    if members.len() < 2 {
        return vec![];
    }
    let mut walkers: Vec<&Member> = members.iter().filter(|m| !m.drone).collect();
    walkers.sort_by_key(|m| order(m));
    let mut drones: Vec<&Member> = members.iter().filter(|m| m.drone).collect();
    drones.sort_by_key(|m| order(m));
    let mut out = vec![];
    let Some(pinner) = walkers.first() else {
        // Drones alone: each fights as it does.
        return out;
    };
    out.push((pinner.e, Role::Pin, pinner.at));
    let mut flanks = 0;
    // A cut-off that holds its part keeps it; else the third walker.
    let cutter = walkers
        .iter()
        .skip(1)
        .position(|w| w.holds == Some(Role::CutOff))
        .map_or(2, |k| k + 1);
    for (k, w) in walkers.iter().enumerate().skip(1) {
        if k == cutter {
            if let Some(rv) = rv {
                out.push((w.e, Role::CutOff, cut_off_post(tp, rv, w.at)));
                continue;
            }
        }
        let side = if flanks % 2 == 0 { 1.0 } else { -1.0 };
        flanks += 1;
        out.push((w.e, Role::Flank, flank_post(pinner.at, tp, side, w.at)));
    }
    // The first flanker's way in, lit.
    let lit = out.iter().find(|o| o.1 == Role::Flank).map(|(e, _, post)| {
        let from = walkers.iter().find(|w| w.e == *e).map_or(*post, |w| w.at);
        (from + *post) * 0.5
    });
    if let (Some(lit), Some(d)) = (lit, drones.first()) {
        out.push((
            d.e,
            Role::Spot,
            Vec3::new(lit.x, lit.y + SPOT_HEIGHT, lit.z),
        ));
    }
    out
}

/// Round to your side: [`super::walker::FLANK_ANGLE`] off the pinner's
/// line to you, at its range (held to [`super::walker::FLANK_RANGE`]).
pub fn flank_post(pinner: Vec3, tp: Vec3, side: f32, y_from: Vec3) -> Vec3 {
    use super::walker::{FLANK_ANGLE, FLANK_RANGE};
    let from = Vec2::new(pinner.x - tp.x, pinner.z - tp.z);
    let d = from.length().clamp(FLANK_RANGE.0, FLANK_RANGE.1);
    let dir = Vec2::from_angle(FLANK_ANGLE * side).rotate(from.normalize_or(Vec2::X));
    Vec3::new(tp.x + dir.x * d, y_from.y, tp.z + dir.y * d)
}

/// Between you and your rendezvous: [`CUT_OFF`] out toward it, or
/// halfway if it's nearer.
pub fn cut_off_post(tp: Vec3, rv: Vec3, y_from: Vec3) -> Vec3 {
    let to = Vec2::new(rv.x - tp.x, rv.z - tp.z);
    let d = to.length();
    let out = CUT_OFF.min(d * 0.5);
    let dir = to.normalize_or(Vec2::X);
    Vec3::new(tp.x + dir.x * out, y_from.y, tp.z + dir.y * out)
}

/// The rendezvous you'd make for from `tp`: the valley's favourite if
/// it's within reach, else the nearest.
pub fn your_rendezvous(
    zone: &Zone,
    mind: Option<&super::memory::ZoneMind>,
    tp: Vec3,
) -> Option<Vec3> {
    let near = |at: [f32; 2]| Vec2::from(at).distance(Vec2::new(tp.x, tp.z));
    let favourite = mind
        .and_then(|m| m.memory.favourite_rv())
        .filter(|(at, _)| near(*at) < FAVOURITE_REACH)
        .map(|(at, _)| at);
    let at = favourite.or_else(|| {
        zone.map
            .markers_of(&MarkerKind::Rendezvous)
            .map(|m| m.at)
            .min_by(|a, b| near(*a).total_cmp(&near(*b)))
    })?;
    Some(zone.on_ground(at[0], at[1]))
}

/// The squads: who's on whom, and what each does.
#[allow(clippy::type_complexity)]
fn plan_squads(
    mut commands: Commands,
    on: Res<Coordination>,
    robots: Query<(
        Entity,
        &Robot,
        &Brain,
        &Awareness,
        &GlobalTransform,
        Option<&super::senses::MindSeed>,
        (
            Has<super::hound::Hound>,
            Has<super::indoor::Crawler>,
            Has<super::grappler::Grappler>,
            Has<super::heavy::Heavy>,
        ),
    )>,
    roles: Query<(Entity, &SquadRole)>,
    targets: Query<&GlobalTransform>,
    (zone, mind): (Option<Res<Zone>>, Option<Res<super::memory::ZoneMind>>),
    mut calls: MessageWriter<SquadCall>,
    time: Res<Time>,
    mut next: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + PLAN_EVERY;
    let mut squads: Vec<(Entity, Vec3, Vec<Member>)> = vec![];
    if on.on {
        for (e, robot, brain, aware, gt, seed, (hound, crawler, grappler, heavy)) in &robots {
            let kind_ok = match robot.kind {
                RobotKind::Walker => !hound && !crawler && !grappler && !heavy,
                RobotKind::Drone => true,
                _ => false,
            };
            if robot.wrecked || !kind_ok || !matches!(brain, Brain::Engage) {
                continue;
            }
            let Some(target) = aware.target else {
                continue;
            };
            let Ok(tp) = targets.get(target).map(|g| g.translation()) else {
                continue;
            };
            let at = gt.translation();
            if at.distance(tp) > SQUAD_RADIUS {
                continue;
            }
            let m = Member {
                e,
                at,
                drone: robot.kind == RobotKind::Drone,
                seed: seed.map_or(e.to_bits(), |s| s.0),
                holds: roles
                    .get(e)
                    .ok()
                    .filter(|(_, r)| r.target == target)
                    .map(|(_, r)| r.role),
            };
            match squads.iter_mut().find(|s| s.0 == target) {
                Some(s) => s.2.push(m),
                None => squads.push((target, tp, vec![m])),
            }
        }
    }
    let mut given: Vec<Entity> = vec![];
    for (target, tp, members) in &squads {
        let rv = zone
            .as_deref()
            .and_then(|z| your_rendezvous(z, mind.as_deref(), *tp));
        for (e, role, post) in plan(members, *tp, rv) {
            given.push(e);
            let was = roles.get(e).ok().map(|(_, r)| *r);
            let new = SquadRole {
                role,
                target: *target,
                post,
            };
            if was != Some(new) {
                commands.entity(e).try_insert(new);
            }
            // Said when the role is new, not every time its post moves.
            if was.is_none_or(|w| w.role != role || w.target != *target) {
                let at = members.iter().find(|m| m.e == e).map_or(post, |m| m.at);
                calls.write(SquadCall {
                    robot: e,
                    role,
                    at,
                    post,
                    target: *target,
                });
            }
        }
    }
    for (e, r) in &roles {
        if r.role != Role::Call && !given.contains(&e) {
            commands.entity(e).try_remove::<SquadRole>();
        }
    }
}

/// A relay that reports someone the machines are already on calls it in.
fn relays_call(
    on: Res<Coordination>,
    relays: Query<
        (Entity, &super::recall::Filed, &Awareness, &GlobalTransform),
        Changed<super::recall::Filed>,
    >,
    engaged: Query<&SquadRole>,
    mut calls: MessageWriter<SquadCall>,
) {
    if !on.on {
        return;
    }
    for (e, _, aware, gt) in &relays {
        let (Some(target), Some(at)) = (aware.target, aware.last_known) else {
            continue;
        };
        if !engaged.iter().any(|r| r.target == target) {
            continue;
        }
        calls.write(SquadCall {
            robot: e,
            role: Role::Call,
            at: gt.translation(),
            post: at,
            target,
        });
    }
}

fn count_calls(
    mut calls: MessageReader<SquadCall>,
    counters: Option<ResMut<crate::bot::Counters>>,
) {
    let n = calls.read().count() as u32;
    if let (Some(mut c), true) = (counters, n > 0) {
        *c.0.entry(CALLS).or_insert(0) += n;
    }
}

/// What a call says on your radio.
/// `place`: the rendezvous a cut-off makes for, or a named place by where
/// a flanker or the spotter is going.
pub fn call_line(call: &SquadCall, me: Vec3, place: Option<&str>, n: u32) -> String {
    use crate::raid::lines::{
        SQUAD_CALL, SQUAD_CUT_OFF, SQUAD_CUT_OFF_AWAY, SQUAD_FLANK, SQUAD_FLANK_BY, SQUAD_PIN,
        SQUAD_SPOT, SQUAD_SPOT_BY, bearing_word, fill, pick,
    };
    let way = bearing_word(Vec2::new(call.post.x - me.x, call.post.z - me.z));
    let (pool, rv) = match (call.role, place) {
        (Role::Pin, _) => (SQUAD_PIN, ""),
        (Role::Flank, Some(p)) => (SQUAD_FLANK_BY, p),
        (Role::Flank, None) => (SQUAD_FLANK, ""),
        (Role::CutOff, Some(rv)) => (SQUAD_CUT_OFF, rv),
        (Role::CutOff, None) => (SQUAD_CUT_OFF_AWAY, ""),
        (Role::Spot, Some(p)) => (SQUAD_SPOT_BY, p),
        (Role::Spot, None) => (SQUAD_SPOT, ""),
        (Role::Call, _) => (SQUAD_CALL, ""),
    };
    fill(pick(pool, n), rv, 0.0, way)
}

/// Calls about you, near, on a radio you carry: the line, and the static.
#[allow(clippy::too_many_arguments)]
fn radio_calls(
    mut calls: MessageReader<SquadCall>,
    raid: Option<ResMut<Raid>>,
    players: Query<(Entity, &GlobalTransform), With<crate::rig::LocalPlayer>>,
    radios: Query<(
        Entity,
        &crate::items::Item,
        &crate::anomaly::artifacts::CarriedBy,
    )>,
    catalog: Res<crate::items::Catalog>,
    zone: Option<Res<Zone>>,
    mut chatter: MessageWriter<crate::raid::chatter::RobotChatter>,
    time: Res<Time>,
    (mut said, mut waiting, mut aired): (
        Local<(f32, u32)>,
        Local<Vec<(SquadCall, f32)>>,
        ResMut<Aired>,
    ),
) {
    let Some(mut raid) = raid else {
        calls.clear();
        return;
    };
    if raid.phase == Phase::AtBase {
        calls.clear();
        waiting.clear();
        return;
    }
    let now = time.elapsed_secs();
    // Calls wait their turn on the air; the newest word from each machine
    // replaces its last, and stale ones go unsaid.
    for call in calls.read() {
        waiting.retain(|(w, _)| w.robot != call.robot);
        waiting.push((*call, now));
    }
    waiting.retain(|(_, t)| now - t < RADIO_STALE);
    if waiting.is_empty() || (said.0 > 0.0 && now - said.0 < RADIO_EVERY) {
        return;
    }
    // The most use to you first: the way being shut, where the flanker's
    // coming, the light, the relay, then the one holding you.
    let rank = |r: Role| match r {
        Role::CutOff => 0,
        Role::Flank => 1,
        Role::Spot => 2,
        Role::Call => 3,
        Role::Pin => 4,
    };
    let Some(i) = (0..waiting.len()).min_by_key(|&i| (rank(waiting[i].0.role), i)) else {
        return;
    };
    let (call, _) = waiting.remove(i);
    let call = &call;
    {
        let Ok((player, gt)) = players.get(call.target) else {
            return;
        };
        let me = gt.translation();
        if call.at.distance(me) > RADIO_REACH {
            return;
        }
        let Some(radio) = radios
            .iter()
            .find(|(_, item, by)| by.0 == player && catalog.get(item.kind()).id == "radio")
            .map(|(e, ..)| e)
        else {
            return;
        };
        // A name to go with the way: the rendezvous a cut-off makes for;
        // for the rest, a named place by where they're going, if there is
        // one (a compass word alone goes stale; "by the mill" doesn't).
        let place = zone.as_deref().and_then(|z| {
            let post = Vec2::new(call.post.x, call.post.z);
            let d = |m: &&extraction_core::map::Marker| Vec2::from(m.at).distance(post);
            let named = z.map.markers.iter().filter(|m| !m.name.is_empty());
            match call.role {
                Role::CutOff => z
                    .map
                    .markers_of(&MarkerKind::Rendezvous)
                    .filter(|m| !m.name.is_empty())
                    .min_by(|a, b| d(a).total_cmp(&d(b))),
                _ => named
                    .filter(|m| d(m) < PLACE_NEAR)
                    .min_by(|a, b| d(a).total_cmp(&d(b))),
            }
            .map(|m| m.name.clone())
        });
        raid.say(call_line(call, me, place.as_deref(), said.1));
        aired.0.push((*call, now));
        if aired.0.len() > AIRED_KEPT {
            aired.0.remove(0);
        }
        said.0 = now;
        said.1 += 1;
        chatter.write(crate::raid::chatter::RobotChatter {
            radio,
            at: call.post,
            close: true,
        });
    }
}

/// A spotting drone's light, down on the flanker's way in.
#[derive(Component, Debug)]
pub struct SpotterLight {
    pub drone: Entity,
}

/// The spotting drones' lights: on while spotting, off after.
#[allow(clippy::type_complexity)]
fn light_spotters(
    mut commands: Commands,
    drones: Query<(Entity, &SquadRole, &GlobalTransform), With<super::drone::Drone>>,
    mut lights: Query<(Entity, &SpotterLight, &mut Transform)>,
) {
    for (light, l, mut t) in &mut lights {
        match drones.get(l.drone) {
            Ok((_, r, gt)) if r.role == Role::Spot => {
                let ground = Vec3::new(r.post.x, r.post.y - SPOT_HEIGHT, r.post.z);
                let from = gt.translation() - Vec3::Y * 0.3;
                if from.distance(ground) > 0.5 {
                    *t = Transform::from_translation(from).looking_at(ground, Vec3::X);
                }
            }
            _ => commands.entity(light).despawn(),
        }
    }
    for (e, r, gt) in &drones {
        if r.role != Role::Spot || lights.iter().any(|(_, l, _)| l.drone == e) {
            continue;
        }
        commands.spawn((
            Name::new("Spotter light"),
            SpotterLight { drone: e },
            SpotLight {
                intensity: 400_000.0,
                range: 45.0,
                outer_angle: 0.28,
                inner_angle: 0.18,
                shadow_maps_enabled: false,
                color: Color::srgb(0.85, 0.92, 1.0),
                ..default()
            },
            Transform::from_translation(gt.translation()),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(bits: u32, at: Vec3, drone: bool, seed: u64) -> Member {
        Member {
            e: Entity::from_raw_u32(bits).unwrap(),
            at,
            drone,
            seed,
            holds: None,
        }
    }

    /// One alone has no plan; two walkers pin and flank; a third cuts off
    /// the way to your rendezvous; a drone lights the flanker's way.
    #[test]
    fn the_plan_holds_flanks_and_cuts_off() {
        let tp = Vec3::ZERO;
        let rv = Vec3::new(0.0, 0.0, -200.0);
        let a = m(1, Vec3::new(0.0, 0.0, 20.0), false, 1);
        let b = m(2, Vec3::new(5.0, 0.0, 22.0), false, 2);
        let c = m(3, Vec3::new(-5.0, 0.0, 22.0), false, 3);
        let d = m(4, Vec3::new(0.0, 20.0, 15.0), true, 0);
        assert!(plan(&[a], tp, Some(rv)).is_empty());
        let two = plan(&[b, a], tp, Some(rv));
        assert_eq!(two[0], (a.e, Role::Pin, a.at), "the lowest seed pins");
        assert_eq!(two[1].1, Role::Flank);
        let off = Vec2::new(two[1].2.x, two[1].2.z)
            .angle_to(Vec2::new(a.at.x, a.at.z))
            .abs();
        assert!(
            (off - super::super::walker::FLANK_ANGLE).abs() < 0.01,
            "round to your side"
        );
        let all = plan(&[c, d, b, a], tp, Some(rv));
        let role = |e: Entity| all.iter().find(|o| o.0 == e).map(|o| o.1);
        assert_eq!(role(c.e), Some(Role::CutOff));
        let cut = all.iter().find(|o| o.1 == Role::CutOff).unwrap().2;
        assert!(
            (cut - Vec3::new(0.0, 0.0, -CUT_OFF)).length() < 0.01,
            "between you and the rendezvous: {cut}"
        );
        assert_eq!(role(d.e), Some(Role::Spot));
        let spot = all.iter().find(|o| o.1 == Role::Spot).unwrap().2;
        let flank = all.iter().find(|o| o.1 == Role::Flank).unwrap().2;
        assert!(
            Vec2::new(spot.x, spot.z).distance(Vec2::new(flank.x + b.at.x, flank.z + b.at.z) * 0.5)
                < 0.01,
            "over the flanker's way in"
        );
        // No rendezvous: the third flanks the other side.
        let no_rv = plan(&[a, b, c], tp, None);
        assert_eq!(no_rv.iter().filter(|o| o.1 == Role::Flank).count(), 2);
        // Drones alone: no plan.
        assert!(plan(&[d, m(5, Vec3::Y * 20.0, true, 1)], tp, Some(rv)).is_empty());
    }

    /// A plan keeps its parts as machines join: a newcomer with a lower
    /// seed doesn't take the pinner's or the flanker's.
    #[test]
    fn parts_are_kept_as_machines_join() {
        let tp = Vec3::ZERO;
        let rv = Vec3::new(0.0, 0.0, -200.0);
        let mut a = m(1, Vec3::new(0.0, 0.0, 20.0), false, 5);
        let mut b = m(2, Vec3::new(5.0, 0.0, 22.0), false, 6);
        a.holds = Some(Role::Pin);
        b.holds = Some(Role::Flank);
        let late = m(3, Vec3::new(-5.0, 0.0, 22.0), false, 0);
        let p = plan(&[late, b, a], tp, Some(rv));
        let role = |e: Entity| p.iter().find(|o| o.0 == e).map(|o| o.1);
        assert_eq!(role(a.e), Some(Role::Pin));
        assert_eq!(role(b.e), Some(Role::Flank));
        assert_eq!(role(late.e), Some(Role::CutOff));
    }

    /// On its way it holds fire; at its post (or with no part to go to) it
    /// doesn't. The spotter's station is in the air.
    #[test]
    fn on_its_way_to_a_post_it_holds_fire() {
        let r = |role, post| SquadRole {
            role,
            target: Entity::PLACEHOLDER,
            post,
        };
        let post = Vec3::new(20.0, 0.0, 0.0);
        assert!(repositioning(Some(&r(Role::Flank, post)), Vec3::ZERO));
        assert!(repositioning(Some(&r(Role::CutOff, post)), Vec3::ZERO));
        assert!(!repositioning(
            Some(&r(Role::Flank, post)),
            post + Vec3::X * 2.0
        ));
        assert!(!repositioning(Some(&r(Role::Pin, post)), Vec3::ZERO));
        assert!(!repositioning(None, Vec3::ZERO));
        let station = Vec3::new(0.0, 14.0, 0.0);
        assert!(repositioning(Some(&r(Role::Spot, station)), Vec3::ZERO));
        assert!(!repositioning(
            Some(&r(Role::Spot, station)),
            station + Vec3::Y
        ));
    }

    /// Near your rendezvous, the cut-off stands halfway to it.
    #[test]
    fn a_near_rendezvous_is_cut_off_halfway() {
        let p = cut_off_post(Vec3::ZERO, Vec3::new(20.0, 0.0, 0.0), Vec3::ZERO);
        assert!((p - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-4);
    }

    /// The radio says which way: the flanker's post, as a compass word.
    #[test]
    fn a_call_says_which_way() {
        let call = SquadCall {
            robot: Entity::PLACEHOLDER,
            role: Role::Flank,
            at: Vec3::ZERO,
            post: Vec3::new(30.0, 0.0, 0.0),
            target: Entity::PLACEHOLDER,
        };
        assert!(call_line(&call, Vec3::ZERO, None, 0).contains("east"));
        let cut = SquadCall {
            role: Role::CutOff,
            ..call
        };
        assert!(call_line(&cut, Vec3::ZERO, Some("the mill"), 0).contains("mill"));
        let by = call_line(&call, Vec3::ZERO, Some("Rail shed"), 0);
        assert!(by.contains("east") && by.contains("rail shed"), "{by}");
    }
}
