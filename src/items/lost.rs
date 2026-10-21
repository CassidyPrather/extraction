//! Items that leave the world come back.
//!
//! Something that tunnels through the ground (or a floor), or ends up
//! somewhere non-finite, would otherwise fall forever: lost to the player,
//! skipped by the save, and simulated every tick. So a loose item that is
//! clearly out of the world is put back where it last lay at rest (or on
//! the ground over where it is, if it never rested), stopped.
//!
//! "Out of the world": more than [`BELOW`] under the ground, unless it's
//! over an underground site's floor (sites are under the ground on
//! purpose), or not finite at all. See `docs/SOAK.md`.
//!
//! Before it gets that far, two backstops catch nearly all of it. A loose
//! item moving fast enough to cover [`STRIDE`] in a tick has its middle's
//! path checked against the static world: a middle never touches a
//! surface (resting on one, it stays this side), so a path through one is
//! tunnelling, and the item goes back to where it struck, bouncing off.
//! (Avian's swept CCD misses it for small, fast things: one 5.45 round in
//! four thrown at a plank wall at 50 m/s went through.) And the ground's
//! skin catches what sinks slowly: thin
//! things (a cleaning rod, a loose round) thrown hard at the terrain slip
//! under its heightfield, which has no thickness to push them back out
//! (a third of rods thrown at 25 m/s did). A moving item whose middle is
//! more than [`SKIN`] under the terrain, and not down a site, is lifted
//! back onto it at once.
//!
//! And new items settle gently: spawned overlapping one another or the
//! world (a heap of loot rolled onto one spot, a find half in a wall), the
//! solver pushes them apart, and in a pile those pushes add up: a PPSh
//! drum, one of ten spawned together, left at 12 m/s. For [`SETTLE`]
//! seconds a new item moves no faster than [`GENTLE`] (or than it was
//! spawned moving, if faster); half a second of falling is slower than
//! that, so a drop is untouched.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::underground as ug;

use super::Item;
use crate::world::Zone;

/// How far under the ground (or a site's floor) an item may be before it
/// counts as lost.
pub const BELOW: f32 = 1.0;
/// How far an item's middle may be under the terrain's surface before it
/// is under the ground. (A building's floor may be up to 0.1 m under the
/// ground beside it: `fit`'s `THROUGH`.)
pub const SKIN: f32 = 0.15;
/// A tick's move longer than this has its path checked, metres (4.5 m/s).
pub const STRIDE: f32 = 0.05;
/// How often items are looked at, seconds.
const EVERY: f32 = 0.25;
/// Seconds a new item settles for.
pub const SETTLE: f32 = 0.5;
/// How fast a settling item may move, m/s.
pub const GENTLE: f32 = 5.0;

/// A new item, settling: seconds left, and its speed limit meanwhile.
#[derive(Component, Clone, Copy, Debug)]
pub struct Settling {
    pub left: f32,
    pub cap: f32,
}

/// Where a loose item's middle was after a tick, its speed then, and
/// which tick (a trail older than the last tick is stale: the item slept,
/// or was held or put away, in between).
#[derive(Component, Clone, Copy, Debug)]
pub struct Trail(pub Vec3, pub f32, pub u64);

/// Where a loose item last lay still, in the world.
#[derive(Component, Clone, Copy, Debug)]
pub struct LastRest(pub Vec3);

pub struct LostItemsPlugin;

impl Plugin for LostItemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            (
                settle_new_items,
                no_tunnelling,
                ground_skin,
                recover_lost_items,
            )
                .chain()
                .after(PhysicsSystems::Last),
        );
    }
}

/// New items spread out of a heap (or out of the wall they spawned in)
/// at a walk, not a launch.
fn settle_new_items(
    mut commands: Commands,
    time: Res<Time>,
    new: Query<(Entity, Option<&LinearVelocity>), (Added<Item>, Without<Settling>)>,
    mut settling: Query<(Entity, &mut Settling, &mut LinearVelocity)>,
) {
    for (e, v) in &new {
        let cap = v.map_or(0.0, |v| v.0.length()).max(GENTLE);
        commands
            .entity(e)
            .try_insert(Settling { left: SETTLE, cap });
    }
    let dt = time.delta_secs();
    for (e, mut s, mut v) in &mut settling {
        let speed = v.0.length();
        if speed > s.cap {
            v.0 *= s.cap / speed;
        }
        s.left -= dt;
        if s.left <= 0.0 {
            commands.entity(e).remove::<Settling>();
        }
    }
}

/// Whether a world point is down an underground site: over (or less than
/// `below` under) a site's floor near there, and under its ceiling (a
/// stair's open to the sky).
fn down_a_site(zone: &Zone, p: Vec3, below: f32) -> bool {
    for site in ug::placed(&zone.map) {
        let Some(r) = ug::raster(&site.id) else {
            continue;
        };
        let footing = zone.hm.footing(&site.id, site.at, site.rot);
        // A wall's raster cell may be rock where an item lies against it.
        for (dx, dz) in [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let local = site.to_site([p.x + dx, p.z + dz]);
            let Some(f) = r.floor_at(local) else {
                continue;
            };
            let ceil = r
                .cell_of(local)
                .map(|(i, j)| r.ceil[j * r.nx + i])
                .filter(|c| *c > 0.0)
                .unwrap_or(f32::INFINITY);
            if p.y > footing + f - below && p.y < footing + f + ceil + 0.5 {
                return true;
            }
        }
    }
    false
}

/// Fast items whose middle went through something static this tick go
/// back to where they struck it, a little off its face, bouncing.
fn no_tunnelling(
    mut commands: Commands,
    catalog: Res<super::Catalog>,
    time: Res<Time>,
    mut tick: Local<u64>,
    of: Query<&ColliderOf>,
    bodies: Query<&RigidBody>,
    // The spatial query reads every collider's position; the items'
    // are written after it's done.
    mut world: ParamSet<(
        SpatialQuery,
        Query<
            (
                Entity,
                &Item,
                &RigidBody,
                &mut Position,
                &Rotation,
                &mut Transform,
                &mut LinearVelocity,
                Option<&mut Trail>,
            ),
            (
                With<Item>,
                Without<RigidBodyDisabled>,
                Without<Sleeping>,
                Without<crate::interact::Held>,
            ),
        >,
    )>,
) {
    *tick += 1;
    let dt = time.delta_secs();
    // The ticks' moves worth checking: (item, from, to).
    let mut moves = Vec::new();
    for (e, _, body, pos, _, _, lin, trail) in &mut world.p1() {
        if *body != RigidBody::Dynamic || !pos.0.is_finite() {
            continue;
        }
        let Some(mut trail) = trail else {
            commands
                .entity(e)
                .insert(Trail(pos.0, lin.0.length(), *tick));
            continue;
        };
        let (a, b) = (trail.0, pos.0);
        let fresh = trail.2 + 1 == *tick;
        let speed = trail.1.max(lin.0.length());
        *trail = Trail(b, lin.0.length(), *tick);
        let len = a.distance(b);
        // A move the item's speed (before the tick or after: a strike
        // stops it) doesn't account for is somebody putting it somewhere
        // (a teleport), not a flight.
        let flew = len < speed * dt * 2.0 + 0.1;
        if fresh && flew && a.is_finite() && len >= STRIDE {
            moves.push((e, a, b));
        }
    }
    if moves.is_empty() {
        return;
    }
    let filter = SpatialQueryFilter::from_mask(crate::world::Layer::World);
    let is_static = |e: Entity| {
        of.get(e)
            .ok()
            .and_then(|c| bodies.get(c.body).ok())
            .is_some_and(|b| *b == RigidBody::Static)
    };
    let spatial = world.p0();
    let hits: Vec<(Entity, Vec3, Vec3)> = moves
        .into_iter()
        .filter_map(|(e, a, b)| {
            let dir = Dir3::new(b - a).ok()?;
            // From a point inside something (spawned there) the hit is at
            // 0: that's for the solver to push out, not this.
            let hit = spatial
                .ray_hits(a, dir, a.distance(b), 4, true, &filter)
                .into_iter()
                .filter(|h| h.distance > 1e-3)
                .find(|h| is_static(h.entity))?;
            Some((e, a + *dir * hit.distance, hit.normal.normalize_or(-*dir)))
        })
        .collect();
    let mut items = world.p1();
    for (e, at, n) in hits {
        let Ok((_, item, _, mut pos, rot, mut tf, mut lin, trail)) = items.get_mut(e) else {
            continue;
        };
        // Off the face by the item's half thickness along it.
        let [x, y, z] = catalog.get(item.kind()).size_m();
        let local = (rot.0.inverse() * n).abs();
        let half = (Vec3::new(x, y, z).max(Vec3::splat(0.01)) * 0.5).dot(local);
        let back = at + n * (half + 0.005);
        pos.0 = back;
        tf.translation = back;
        if let Some(mut t) = trail {
            t.0 = back;
        }
        // Off the face with what restitution would leave of it, and
        // friction half its slide.
        let into = lin.0.dot(n);
        let slide = lin.0 - n * into;
        lin.0 = slide * 0.5 + n * into.abs() * 0.15;
    }
}

/// Moving items under the terrain's skin go back up onto it.
fn ground_skin(
    zone: Res<Zone>,
    mut items: Query<
        (
            &RigidBody,
            &mut Position,
            &mut Transform,
            &mut LinearVelocity,
            Option<&ColliderAabb>,
            Option<&mut Trail>,
        ),
        (
            With<Item>,
            Without<RigidBodyDisabled>,
            Without<Sleeping>,
            Without<crate::interact::Held>,
        ),
    >,
) {
    for (body, mut pos, mut tf, mut lin, aabb, trail) in &mut items {
        if *body != RigidBody::Dynamic || !pos.0.is_finite() {
            continue;
        }
        let g = zone.ground(pos.0.x, pos.0.z);
        if pos.0.y > g - SKIN || pos.0.y < g - BELOW || down_a_site(&zone, pos.0, BELOW) {
            continue;
        }
        let half = aabb.map_or(0.05, |a| (a.max.y - a.min.y) * 0.5);
        pos.0.y = g + half + 0.01;
        tf.translation = pos.0;
        if let Some(mut t) = trail {
            t.0 = pos.0;
        }
        lin.0 = Vec3::new(lin.0.x * 0.5, 0.0, lin.0.z * 0.5);
    }
}

/// Whether a world point is somewhere an item can be: over the ground
/// (less than [`BELOW`] under it), or over an underground site's floor
/// near there.
pub fn in_world(zone: &Zone, p: Vec3) -> bool {
    if !p.is_finite() {
        return false;
    }
    p.y > zone.ground(p.x, p.z) - BELOW || down_a_site(zone, p, BELOW)
}

fn recover_lost_items(
    mut commands: Commands,
    time: Res<Time>,
    mut since: Local<f32>,
    zone: Res<Zone>,
    mut items: Query<
        (
            Entity,
            &RigidBody,
            &mut Position,
            &mut Rotation,
            &mut Transform,
            &mut LinearVelocity,
            &mut AngularVelocity,
            Option<&LastRest>,
            Option<&mut Trail>,
        ),
        (
            With<Item>,
            Without<RigidBodyDisabled>,
            Without<crate::interact::Held>,
        ),
    >,
) {
    *since += time.delta_secs();
    if *since < EVERY {
        return;
    }
    *since = 0.0;
    for (e, body, mut pos, mut rot, mut tf, mut lin, mut ang, rest, trail) in &mut items {
        if *body != RigidBody::Dynamic {
            continue;
        }
        if in_world(&zone, pos.0) {
            if lin.0.length_squared() < 0.01 && rest.is_none_or(|r| r.0.distance(pos.0) > 0.05) {
                commands.entity(e).insert(LastRest(pos.0));
            }
            continue;
        }
        let back = rest
            .map(|r| r.0)
            .filter(|r| in_world(&zone, *r))
            .unwrap_or_else(|| {
                let (x, z) = if pos.0.is_finite() {
                    (pos.0.x, pos.0.z)
                } else {
                    (0.0, 0.0)
                };
                zone.on_ground(x, z) + Vec3::Y * 0.3
            });
        warn!(
            "item {e} fell out of the world at {}; put back at {back}",
            pos.0
        );
        pos.0 = back;
        tf.translation = back;
        if let Some(mut t) = trail {
            t.0 = back;
        }
        if !rot.0.is_finite() {
            rot.0 = Quat::IDENTITY;
            tf.rotation = Quat::IDENTITY;
        }
        lin.0 = Vec3::ZERO;
        ang.0 = Vec3::ZERO;
    }
}
