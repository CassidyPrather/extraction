//! The disturbed world as evidence: what you leave changed tells a patrol
//! someone was here.
//!
//! One registry, [`Signs`]: each [`Sign`] is where, what ([`SignKind`])
//! and since when. Whatever changes the world feeds it, by writing a
//! [`Disturb`] or with a line in [`feed`]: a door a player opened, a fence
//! or padlock cut, a crowbar's marks, glass or a crate broken, a wreck
//! dragged off, a loot spot cleared. A new kind is a variant and a line.
//!
//! A robot that can't see you, passing within sight of a sign it hasn't
//! seen yet ([`SIGN_SIGHT`], less as the light or weather close in):
//!
//! - **Fresh** (younger than [`SIGN_FRESH`]), it goes to look
//!   ([`SIGN_SUSPICION`]): roughly there ([`SIGN_UNCERTAINTY`]).
//! - **Older** (up to [`SIGN_LIFE`]), it's only wary ([`OLD_SUSPICION`]):
//!   it turns to look, and it's quicker to look into the next thing.
//! - **Several in a line**: two or more it's seen lately
//!   ([`ROUTE_WITHIN`]), [`ROUTE_APART`] or more apart, are a route: it
//!   guesses you went on from the newest the way they run, [`ROUTE_LEAD`]
//!   further, and goes to look there ([`ROUTE_UNCERTAINTY`]).
//!
//! Cost: at most one look a robot every [`LOOK_EVERY`] seconds, a scan of
//! at most [`MAX_SIGNS`] signs, and a ray for each sign in reach and view.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

use super::{Awareness, Robot, Senses};
use crate::world::Layer;

/// What was disturbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignKind {
    /// A door a player opened (and may have left open).
    DoorOpened,
    /// A fence's wire or a padlock cut through.
    Cut,
    /// A crowbar levered at something: a door, a lid, a car's door.
    Pried,
    /// Glass broken: a window, a windscreen.
    Glass,
    /// Something wooden broken open: a crate, a door, a shutter.
    Splintered,
    /// A wreck or a carcass dragged from where it fell.
    Dragged,
    /// A loot spot cleared: a shelf, a table, a locker left bare.
    Looted,
}

/// A disturbance: where, what, and since when (the robots' clock).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sign {
    pub at: Vec3,
    pub kind: SignKind,
    pub since: f32,
}

/// Something in the world disturbed, now, here: for any system to write.
#[derive(Message, Clone, Copy, Debug)]
pub struct Disturb {
    pub at: Vec3,
    pub kind: SignKind,
}

/// At most this many signs are kept (the oldest go).
pub const MAX_SIGNS: usize = 256;
/// The same kind of sign within this of another (m) is the same sign,
/// made again (a door swinging, a wreck dragged on): it's renewed.
pub const SAME_SIGN: f32 = 2.0;

/// Every sign in the Zone.
#[derive(Resource, Default, Debug)]
pub struct Signs(pub Vec<Sign>);

impl Signs {
    /// Leave a sign of `kind` at `at`, now: the same one nearby is renewed
    /// (and moved, for a thing dragged along) rather than doubled.
    pub fn leave(&mut self, at: Vec3, kind: SignKind, now: f32) {
        if let Some(s) = self
            .0
            .iter_mut()
            .find(|s| s.kind == kind && s.at.distance(at) < SAME_SIGN)
        {
            s.at = at;
            s.since = now;
            return;
        }
        self.0.push(Sign {
            at,
            kind,
            since: now,
        });
        if self.0.len() > MAX_SIGNS {
            let oldest = (0..self.0.len())
                .min_by(|a, b| self.0[*a].since.total_cmp(&self.0[*b].since))
                .unwrap();
            self.0.swap_remove(oldest);
        }
    }
}

/// It makes a sign out at this far in clear daylight (m)...
pub const SIGN_SIGHT: f32 = 16.0;
/// ...looking every this many seconds.
pub const LOOK_EVERY: f32 = 0.5;
/// A sign younger than this (s) sends it to look...
pub const SIGN_FRESH: f32 = 180.0;
/// ...raising its suspicion to this...
pub const SIGN_SUSPICION: f32 = super::search::INVESTIGATE_AT + 0.1;
/// ...unsure of where you were by this (m).
pub const SIGN_UNCERTAINTY: f32 = 4.0;
/// An older one, up to this (s), only makes it wary: this suspicious.
pub const SIGN_LIFE: f32 = 900.0;
pub const OLD_SUSPICION: f32 = super::search::INVESTIGATE_AT * 0.7;
/// Signs it saw within this long (s) of each other...
pub const ROUTE_WITHIN: f32 = 300.0;
/// ...and at least this far apart (m) are a route...
pub const ROUTE_APART: f32 = 4.0;
/// ...that it reads on this far past the newest (m)...
pub const ROUTE_LEAD: f32 = 20.0;
/// ...this unsure (m).
pub const ROUTE_UNCERTAINTY: f32 = 6.0;
/// A player opening a door is within this of it (m).
pub const DOOR_HAND: f32 = 2.5;
/// A thing picked up within this of a loot spot (m) cleared it.
pub const LOOT_HAND: f32 = 0.6;

pub struct DisturbedPlugin;

impl Plugin for DisturbedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Signs>()
            .add_message::<Disturb>()
            .add_systems(
                FixedUpdate,
                (feed, notice_signs).chain().after(super::perceive),
            );
    }
}

/// What feeds the registry: one line a kind (see the module docs).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn feed(
    mut signs: ResMut<Signs>,
    mut disturbed: MessageReader<Disturb>,
    (mut cuts, mut pries, mut broke, mut drags, mut grabs): (
        MessageReader<crate::destruction::hooks::Cut>,
        MessageReader<crate::destruction::hooks::Pried>,
        MessageReader<crate::destruction::Broke>,
        MessageReader<crate::interact::carry::Dragged>,
        MessageReader<crate::interact::InteractEvent>,
    ),
    doors: Query<
        (&crate::world::buildings::Door, &GlobalTransform),
        Changed<crate::world::buildings::Door>,
    >,
    players: Query<(Entity, &GlobalTransform), With<crate::player::Player>>,
    wrecks: Query<(), With<crate::interact::carry::Draggable>>,
    places: Query<&GlobalTransform>,
    loot: Option<Res<crate::world::buildings::BuildingLoot>>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let by_player = |e: Entity| players.contains(e);
    let at = |e: Entity| places.get(e).ok().map(|g| g.translation());
    let mut left: Vec<(Vec3, SignKind)> = disturbed.read().map(|d| (d.at, d.kind)).collect();
    left.extend(
        cuts.read()
            .filter(|c| by_player(c.by))
            .filter_map(|c| Some((at(c.target)?, SignKind::Cut))),
    );
    left.extend(
        pries
            .read()
            .filter(|p| by_player(p.by))
            .filter_map(|p| Some((at(p.target)?, SignKind::Pried))),
    );
    left.extend(broke.read().map(|b| {
        let kind = match b.stuff {
            crate::destruction::Stuff::Glass => SignKind::Glass,
            crate::destruction::Stuff::Wood => SignKind::Splintered,
        };
        (b.at, kind)
    }));
    left.extend(
        drags
            .read()
            .filter(|d| wrecks.contains(d.item))
            .map(|d| (d.at, SignKind::Dragged)),
    );
    left.extend(
        doors
            .iter()
            .filter(|(d, gt)| {
                d.open
                    && players
                        .iter()
                        .any(|(_, p)| p.translation().distance(gt.translation()) < DOOR_HAND)
            })
            .map(|(_, gt)| (gt.translation(), SignKind::DoorOpened)),
    );
    if let Some(loot) = &loot {
        for ev in grabs.read() {
            let crate::interact::InteractEvent::Grabbed { item, .. } = *ev else {
                continue;
            };
            let Some(p) = at(item) else {
                continue;
            };
            if let Some(spot) = loot.0.iter().find(|s| s.at.distance(p) < LOOT_HAND) {
                left.push((spot.at, SignKind::Looted));
            }
        }
    }
    for (at, kind) in left {
        signs.leave(at, kind, now);
    }
}

/// Where a robot that has seen `seen` (newest last: when it saw each,
/// where, and since when it was there) guesses you went: on along the
/// line of the two newest made far enough apart, if any.
pub fn route(seen: &[Sign]) -> Option<Vec3> {
    let newest = seen.iter().max_by(|a, b| a.since.total_cmp(&b.since))?;
    let older = seen
        .iter()
        .filter(|s| {
            newest.since - s.since <= ROUTE_WITHIN
                && s.since < newest.since
                && s.at.xz().distance(newest.at.xz()) >= ROUTE_APART
        })
        .max_by(|a, b| a.since.total_cmp(&b.since))?;
    let dir = (newest.at - older.at).with_y(0.0).normalize_or_zero();
    Some(newest.at + dir * ROUTE_LEAD)
}

#[allow(clippy::type_complexity)]
fn notice_signs(
    mut robots: Query<(Entity, &Robot, &Senses, &GlobalTransform, &mut Awareness)>,
    signs: Res<Signs>,
    conditions: Res<crate::world::weather::Conditions>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut seen: Local<HashMap<Entity, (f32, Vec<Sign>)>>,
) {
    if signs.0.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let reach = conditions.sight(SIGN_SIGHT);
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    seen.retain(|e, _| robots.contains(*e));
    for (e, robot, senses, gt, mut aware) in &mut robots {
        if robot.wrecked || aware.can_see_target {
            continue;
        }
        let (last, mine) = seen.entry(e).or_insert((f32::NEG_INFINITY, Vec::new()));
        if now - *last < LOOK_EVERY {
            continue;
        }
        *last = now;
        mine.retain(|s| now - s.since < SIGN_LIFE);
        let eye = gt.transform_point(senses.eye);
        let forward = gt.forward().as_vec3();
        let new: Vec<Sign> = signs
            .0
            .iter()
            .filter(|s| now - s.since < SIGN_LIFE)
            .filter(|s| {
                !mine.iter().any(|m| {
                    m.kind == s.kind && m.at.distance(s.at) < SAME_SIGN && m.since >= s.since
                })
            })
            .filter(|s| {
                let to = s.at + Vec3::Y * 0.5 - eye;
                let d = to.length();
                if d > reach || d < 0.01 {
                    return false;
                }
                if forward.angle_between(to / d).to_degrees() > senses.fov_deg * 0.5 {
                    return false;
                }
                // (Short of it: a door or a fence is the thing in the way.)
                Dir3::new(to).ok().is_some_and(|dir| {
                    spatial
                        .cast_ray(eye, dir, (d - 1.0).max(0.0), true, &world)
                        .is_none()
                })
            })
            .copied()
            .collect();
        if new.is_empty() {
            continue;
        }
        for s in &new {
            mine.retain(|m| !(m.kind == s.kind && m.at.distance(s.at) < SAME_SIGN));
            mine.push(*s);
        }
        let freshest = new
            .iter()
            .max_by(|a, b| a.since.total_cmp(&b.since))
            .unwrap();
        if now - freshest.since >= SIGN_FRESH {
            aware.suspicion = aware.suspicion.max(OLD_SUSPICION);
            aware.news = true;
            continue;
        }
        let fresh: Vec<Sign> = mine
            .iter()
            .filter(|s| now - s.since < SIGN_FRESH)
            .copied()
            .collect();
        let (guess, uncertainty) = match route(&fresh) {
            Some(on) => (on, ROUTE_UNCERTAINTY),
            None => (freshest.at, SIGN_UNCERTAINTY),
        };
        aware.suspicion = aware.suspicion.max(SIGN_SUSPICION);
        aware.last_known = Some(guess);
        aware.uncertainty = uncertainty;
        aware.news = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same sign made again nearby is renewed, not doubled; the
    /// registry keeps the newest MAX_SIGNS.
    #[test]
    fn a_sign_made_again_is_renewed_and_the_oldest_go() {
        let mut s = Signs::default();
        s.leave(Vec3::ZERO, SignKind::Dragged, 1.0);
        s.leave(Vec3::X, SignKind::Dragged, 2.0);
        assert_eq!(s.0.len(), 1);
        assert_eq!(s.0[0].since, 2.0);
        assert_eq!(s.0[0].at, Vec3::X);
        // Another kind there is another sign.
        s.leave(Vec3::X, SignKind::Glass, 3.0);
        assert_eq!(s.0.len(), 2);
        for i in 0..MAX_SIGNS + 10 {
            s.leave(
                Vec3::new(i as f32 * 10.0, 0.0, 50.0),
                SignKind::Cut,
                10.0 + i as f32,
            );
        }
        assert_eq!(s.0.len(), MAX_SIGNS);
        assert!(s.0.iter().all(|x| x.since > 10.0), "the oldest went");
    }

    /// Two signs far enough apart read on past the newer, the way they
    /// run; one alone, or two on top of each other, don't.
    #[test]
    fn signs_in_a_line_read_on_past_the_newest() {
        let sign = |x: f32, since: f32| Sign {
            at: Vec3::new(x, 0.0, 0.0),
            kind: SignKind::DoorOpened,
            since,
        };
        let on = route(&[sign(0.0, 10.0), sign(8.0, 40.0)]).unwrap();
        assert!((on.x - (8.0 + ROUTE_LEAD)).abs() < 0.01, "{on}");
        // Newest first in the list: the same.
        let on = route(&[sign(8.0, 40.0), sign(0.0, 10.0)]).unwrap();
        assert!(on.x > 8.0);
        assert!(route(&[sign(0.0, 10.0)]).is_none());
        assert!(route(&[sign(0.0, 10.0), sign(1.0, 20.0)]).is_none());
        // Too long between them: not one route.
        assert!(route(&[sign(0.0, 10.0), sign(8.0, 10.0 + ROUTE_WITHIN + 1.0)]).is_none());
    }
}
