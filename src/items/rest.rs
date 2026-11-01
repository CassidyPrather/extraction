//! Items lying still go to sleep.
//!
//! Avian keeps a resting contact's point only while its gap is under how
//! far the bodies close in a step (`dt` times their relative speed). An
//! item at rest closes on nothing, so a corner lifted a hair by the
//! solver loses its point (the 5 mm contact tolerance only finds it). If
//! enough of a resting item's points go, it falls a step, lands with a
//! spin of a few rad/s, and its sleep timer starts again. Some items lie
//! like that for good, on a gentle slope of bare terrain, a millimetre
//! up or down and never more, awake every frame (Performance's
//! `perf::tests::gauge_items_fall_asleep`).
//!
//! So a loose item that has stayed within [`STILL_MOVE`] and [`STILL_TURN`]
//! of where it was for [`STILL_FOR`] is put to sleep, if everything in its
//! contact island is too (Avian sleeps whole islands). Anything that
//! wakes it then (a touch, a shot, a hand) works as before, and it has to
//! lie still for [`STILL_FOR`] again.

use avian3d::dynamics::solver::islands::{BodyIslandNode, SleepBody};
use avian3d::prelude::*;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

use super::{Catalog, Item};

/// How far an item may drift and still be lying still, metres.
pub const STILL_MOVE: f32 = 0.002;
/// How far it may turn and still be lying still, radians (2 degrees).
pub const STILL_TURN: f32 = 0.035;
/// How long an item lies still before it's put to sleep, seconds.
pub const STILL_FOR: f32 = 1.0;

/// Where an awake loose item came to lie, and when.
#[derive(Component, Clone, Copy, Debug)]
pub struct Lying {
    at: Vec3,
    rot: Quat,
    since: f32,
}

pub struct RestPlugin;

impl Plugin for RestPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedPostUpdate, lie_still.after(PhysicsSystems::Last));
    }
}

#[allow(clippy::type_complexity)]
fn lie_still(
    mut commands: Commands,
    time: Res<Time>,
    catalog: Res<Catalog>,
    mut items: Query<(
        Entity,
        &Item,
        &RigidBody,
        &Position,
        &Rotation,
        Option<&mut Lying>,
        Has<Sleeping>,
        Has<crate::interact::Held>,
        Has<RigidBodyDisabled>,
        Has<ChildOf>,
    )>,
    bodies: Query<(Entity, &BodyIslandNode), Without<Sleeping>>,
) {
    let now = time.elapsed_secs();
    let mut still = HashSet::new();
    for (e, item, body, pos, rot, lying, asleep, held, disabled, child) in &mut items {
        let loose = body.is_dynamic()
            && !asleep
            && !held
            && !disabled
            && !child
            // A Featherstone hovers on a force, which wakes it every frame.
            && catalog.get(item.kind()).lift_kg <= 0.0;
        if !loose {
            if lying.is_some() {
                commands.entity(e).remove::<Lying>();
            }
            continue;
        }
        let here = Lying {
            at: pos.0,
            rot: rot.0,
            since: now,
        };
        match lying {
            // (Measured from where it came to lie, not from the last
            // frame, so a slow creep adds up and counts as moving.)
            Some(l)
                if l.at.distance(pos.0) <= STILL_MOVE
                    && l.rot.angle_between(rot.0) <= STILL_TURN =>
            {
                if now - l.since >= STILL_FOR {
                    still.insert(e);
                }
            }
            Some(mut l) => *l = here,
            None => {
                commands.entity(e).insert(here);
            }
        }
    }
    if still.is_empty() {
        return;
    }
    // An island sleeps whole: only one that's all still items.
    let mut islands: HashMap<u32, bool> = HashMap::new();
    for (e, node) in &bodies {
        let ok = islands.entry(node.island_id().0).or_insert(true);
        *ok &= still.contains(&e);
    }
    let mut done = HashSet::new();
    for (e, node) in &bodies {
        let id = node.island_id().0;
        if still.contains(&e) && islands.get(&id) == Some(&true) && done.insert(id) {
            commands.queue(SleepBody(e));
        }
    }
}
