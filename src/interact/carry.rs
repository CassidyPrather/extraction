//! Carrying the heavy and the bulky: the catalog's `SizeClass::Bulky` that
//! aren't guns (a car door, a bus panel, a wheel, a heavy's claw).
//!
//! - **Two hands carry it.** It has a grab point at each end of its long
//!   side; the second hand on a held one takes the other end, as a gun's
//!   handguard is taken (`Supported`). It swings: its speed changes no
//!   faster than two arms can push its mass ([`ARMS`]), so it carries on
//!   when you stop and comes round late when you turn. You walk slower by
//!   its mass ([`haul_of`]) and can't run.
//! - **One hand drags it.** It keeps its weight and is pulled by the point
//!   you hold ([`PULL`], [`DRAG_SPEED`]): the far end on the ground,
//!   scraping ([`Dragged`]: Audio voices it, robots hear it). You walk at
//!   half pace.
//! - **Let go of it moving and it's shoved, not thrown**: what an arm puts
//!   into it is [`SHOVE`] joules at most (`interact::apply_hand_actions`).
//! - **A wreck is dragged by a limb** ([`Draggable`]): a broken robot's hull
//!   (`robots::wreck::Broken`), taken by a leg or an arm (its parts are
//!   found by the grip, `interact::find_grab`), in one hand or both, never
//!   lifted; slowly, as its mass allows ([`drag_speed`]). Let go, it
//!   settles again.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::items::SizeClass;

use super::touch::Stuff;
use super::{GrabPoint, Grabbable, GripRole, Hand, Held, InteractSystems, Supported};
use crate::items::{Catalog, Item};
use crate::player::Body;
use crate::world::Layer;

/// How hard two arms can push a carried thing about, newtons.
pub const ARMS: f32 = 500.0;
/// How hard one arm pulls a dragged thing, newtons, and how fast the held
/// point can be drawn along, m/s.
pub const PULL: f32 = 450.0;
pub const DRAG_SPEED: f32 = 1.4;
/// One arm lifts at most this share of a bulky thing's weight: enough to
/// raise the end it holds (half the weight), not to pick it up.
pub const LIFT_ONE: f32 = 0.6;
/// The most a shove puts into a bulky thing, joules.
pub const SHOVE: f32 = 40.0;
/// Walking with a bulky thing in both hands: this much slower per kg (a
/// 16 kg car door: about a third slower), and dragging one, half pace.
pub const CARRY_SLOW: f32 = 0.022;
pub const DRAG_SLOW: f32 = 0.5;
/// A drag at full speed is heard this far, metres.
pub const DRAG_HEARD: f32 = 14.0;

/// Bulky, not a gun: carried in two hands, dragged in one.
#[derive(Component, Debug)]
pub struct Bulky;

/// Too heavy to carry, however many hands: only dragged (a robot's wreck).
/// Also [`Bulky`], so a second hand takes hold of it too.
#[derive(Component, Debug)]
pub struct Draggable;

/// How fast a thing of this mass can be dragged at most, m/s: [`DRAG_SPEED`]
/// up to 30 kg, slower the heavier (a walker's hull, about half that).
pub fn drag_speed(mass: f32) -> f32 {
    DRAG_SPEED * (30.0 / mass.max(1.0)).min(1.0).sqrt()
}

/// Something bulky dragged along the ground, every tick it slides (a
/// scrape for Audio: one loop per item, stopping when these stop).
#[derive(Message, Clone, Copy, Debug)]
pub struct Dragged {
    pub item: Entity,
    pub at: Vec3,
    /// m/s; full at [`DRAG_SPEED`].
    pub speed: f32,
    pub stuff: Stuff,
    /// It rolls (a wheel) rather than scrapes.
    pub rolling: bool,
}

/// A carried thing's velocity last tick, for its swing.
#[derive(Component, Default)]
struct Swing {
    lin: Vec3,
    ang: Vec3,
}

pub struct CarryPlugin;

impl Plugin for CarryPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Dragged>()
            .add_observer(mark_bulky)
            .add_observer(wreck_draggable)
            .add_observer(settle_let_go)
            .add_observer(unsettle_taken)
            .add_systems(
                FixedUpdate,
                (drag, swing, haul)
                    .chain()
                    .in_set(InteractSystems::Follow)
                    .after(super::follow_hands),
            );
    }
}

/// Is this catalog item carried as bulky?
pub fn is_bulky(def: &extraction_core::items::ItemDef) -> bool {
    def.size == SizeClass::Bulky && def.firearm.is_none()
}

/// A grab point at each end of a thing's long side (its size, metres),
/// a little in from the end.
pub fn end_points(size: Vec3) -> Vec<GrabPoint> {
    let axis = if size.x >= size.y && size.x >= size.z {
        Vec3::X
    } else if size.y >= size.z {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let reach = (size.dot(axis) / 2.0 - 0.06).max(0.05);
    [1.0, -1.0]
        .map(|s| GrabPoint {
            local: Transform::from_translation(axis * reach * s),
            role: GripRole::Free,
            radius: (size.max_element() / 2.0).max(0.3),
        })
        .to_vec()
}

fn mark_bulky(
    add: On<Add, Item>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut commands: Commands,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    let def = catalog.get(item.kind());
    if !is_bulky(def) {
        return;
    }
    commands.entity(add.entity).try_insert((
        Bulky,
        Grabbable {
            points: end_points(Vec3::from_array(def.size_m())),
        },
    ));
}

/// A robot come apart: its hull is dragged by a limb.
fn wreck_draggable(add: On<Add, crate::robots::wreck::Broken>, mut commands: Commands) {
    commands.entity(add.entity).try_insert((
        Bulky,
        Draggable,
        Grabbable {
            points: vec![GrabPoint::free()],
        },
    ));
}

/// A wreck taken hold of stops settling: it'd be made static in the hand.
fn unsettle_taken(add: On<Add, Held>, wrecks: Query<(), With<Draggable>>, mut commands: Commands) {
    if wrecks.contains(add.entity) {
        commands
            .entity(add.entity)
            .try_remove::<crate::robots::wreck::Settling>();
    }
}

/// A wreck let go settles again (and goes static, costing nothing).
fn settle_let_go(
    remove: On<Remove, Held>,
    wrecks: Query<(), With<Draggable>>,
    mut commands: Commands,
) {
    if wrecks.contains(remove.entity) {
        commands
            .entity(remove.entity)
            .try_insert(crate::robots::wreck::Settling::default());
    }
}

/// What a carried thing is to the hand and the ear.
fn stuff_of(def: &extraction_core::items::ItemDef) -> Stuff {
    let has = |t: &str| def.tags.iter().any(|x| x == t);
    if has("glass") {
        Stuff::Glass
    } else if has("metal") || has("steel") {
        Stuff::Metal
    } else {
        Stuff::Wood
    }
}

/// The speed a velocity may change to from `was`, given the force and the
/// mass behind it.
pub fn swung(was: Vec3, want: Vec3, force: f32, mass: f32, dt: f32) -> Vec3 {
    was + (want - was).clamp_length_max(force / mass.max(0.1) * dt)
}

/// One hand on a bulky thing: it keeps its weight, and the point held is
/// drawn toward the hand by an arm's pull.
#[allow(clippy::type_complexity)]
fn drag(
    mut commands: Commands,
    mut things: Query<
        (
            Entity,
            &Held,
            Option<&Item>,
            &Position,
            &Rotation,
            &mut LinearVelocity,
            &mut AngularVelocity,
            Option<&mut GravityScale>,
            Option<&Mass>,
            Option<&Supported>,
            Has<Draggable>,
            Option<&Grabbable>,
        ),
        With<Bulky>,
    >,
    hands: Query<&Hand>,
    catalog: Res<Catalog>,
    spatial: SpatialQuery,
    mut dragged: MessageWriter<Dragged>,
    mut noise: MessageWriter<crate::robots::Noise>,
    time: Res<Time>,
    mut heard: Local<f32>,
) {
    let dt = time.delta_secs().max(1e-4);
    *heard -= dt;
    let mut loudest: Option<crate::robots::Noise> = None;
    for (e, held, item, pos, rot, mut lin, mut ang, gravity, mass, support, wreck, grab) in
        &mut things
    {
        let two = support.is_some() && !wreck;
        let want_gravity = if two { 0.0 } else { 1.0 };
        match gravity {
            Some(mut g) if g.0 != want_gravity => g.0 = want_gravity,
            Some(_) => {}
            None => {
                commands.entity(e).insert(GravityScale(want_gravity));
            }
        }
        if two {
            continue;
        }
        let def = item.map(|i| catalog.get(i.kind()));
        let m = mass.map_or(def.map_or(80.0, |d| d.mass), |m| m.0).max(0.5);
        let size = def.map_or(Vec3::splat(0.8), |d| Vec3::from_array(d.size_m()));
        let inertia =
            m / 12.0 * (Vec3::splat(size.length_squared()) - size * size).max(Vec3::splat(0.01));
        // Each hand on it pulls at the point it holds: the first where it
        // took hold, the second (on a wreck) at its grab point.
        let mut pulls = vec![];
        if let Ok(hand) = hands.get(held.hand) {
            let grip = held
                .offset
                .compute_affine()
                .inverse()
                .transform_point3(Vec3::ZERO);
            pulls.push((hand.world.translation, grip));
        }
        if let (Some(s), true) = (support, wreck)
            && let Ok(hand) = hands.get(s.hand)
        {
            let grip = grab
                .and_then(|g| g.points.get(s.point))
                .map_or(Vec3::ZERO, |p| p.local.translation);
            pulls.push((hand.world.translation, grip));
        }
        let top = drag_speed(m);
        for (hand_at, grip) in pulls {
            let r = rot.0 * grip;
            let at = pos.0 + r;
            let point_v = lin.0 + ang.0.cross(r);
            let want = ((hand_at - at) * 10.0).clamp_length_max(top);
            // An arm's impulse at the held point, toward the speed wanted
            // there: it moves the thing and turns it about its middle.
            let j = ((want - point_v) * m).clamp_length_max(PULL * dt);
            // One arm raises the end it holds, never the whole thing: up,
            // it pulls with at most part of the thing's weight.
            let j = j.with_y(j.y.min(LIFT_ONE * m * 9.81 * dt));
            lin.0 += j / m;
            let local = rot.0.inverse() * r.cross(j);
            ang.0 += rot.0 * (local / inertia);
        }
        // Sliding on the ground: a scrape.
        let speed = Vec2::new(lin.0.x, lin.0.z).length();
        if speed > 0.1 {
            let reach = size.max_element() / 2.0 + 0.08;
            let filter = SpatialQueryFilter::from_mask(Layer::World);
            let down = spatial.cast_ray(pos.0, Dir3::NEG_Y, reach, true, &filter);
            if let Some(hit) = down {
                let ground = pos.0 - Vec3::Y * hit.distance;
                dragged.write(Dragged {
                    item: e,
                    at: ground,
                    speed,
                    stuff: def.map_or(Stuff::Metal, stuff_of),
                    rolling: def.is_some_and(|d| d.id.contains("wheel")),
                });
                let radius = DRAG_HEARD * (speed / top).clamp(0.3, 1.0);
                if loudest.as_ref().is_none_or(|n| n.radius < radius) {
                    loudest = Some(crate::robots::Noise {
                        at: ground,
                        radius,
                        source: Some(held.player),
                    });
                }
            }
        }
    }
    // Robots hear a drag twice a second, as they hear footsteps.
    if let Some(n) = loudest
        && *heard <= 0.0
    {
        *heard = 0.5;
        noise.write(n);
    }
}

/// Two hands on a bulky thing: `follow_hands` has asked for the speed that
/// would put it in the hands; it gets there only as fast as arms can push
/// its mass, so it swings.
#[allow(clippy::type_complexity)]
fn swing(
    mut commands: Commands,
    mut things: Query<
        (
            Entity,
            &mut LinearVelocity,
            &mut AngularVelocity,
            Option<&Mass>,
            Option<&mut Swing>,
        ),
        (With<Bulky>, With<Held>, With<Supported>),
    >,
    loose: Query<Entity, (With<Swing>, Or<(Without<Held>, Without<Supported>)>)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    for (e, mut lin, mut ang, mass, was) in &mut things {
        let m = mass.map_or(1.0, |m| m.0);
        let Some(mut was) = was else {
            commands.entity(e).insert(Swing {
                lin: lin.0,
                ang: ang.0,
            });
            continue;
        };
        lin.0 = swung(was.lin, lin.0, ARMS, m, dt);
        // Turning it: the arms are half a metre out either side.
        ang.0 = swung(was.ang, ang.0, ARMS * 0.5, m * 0.25, dt);
        was.lin = lin.0;
        was.ang = ang.0;
    }
    for e in &loose {
        commands.entity(e).remove::<Swing>();
    }
}

/// How much slower you walk with a bulky thing: carried (its mass, both
/// hands) or dragged (half pace). 0: not at all.
pub fn haul_of(mass: f32, two_hands: bool) -> f32 {
    if two_hands {
        (mass * CARRY_SLOW).min(0.6)
    } else {
        // Dragging: half pace, slower still for more than you could carry.
        (DRAG_SLOW + (mass - 40.0).max(0.0) * 0.004).min(0.75)
    }
}

fn haul(
    things: Query<(&Held, Has<Supported>, Option<&Mass>, Has<Draggable>), With<Bulky>>,
    mut bodies: Query<(Entity, &mut Body)>,
) {
    for (player, mut body) in &mut bodies {
        let slow = things
            .iter()
            .filter(|(h, ..)| h.player == player)
            .map(|(_, two, m, wreck)| haul_of(m.map_or(1.0, |m| m.0), two && !wreck))
            .fold(0.0, f32::max);
        if body.hauling != slow {
            body.hauling = slow;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A car door's ends are along its width, a hand's reach in from each
    /// edge; a wheel's across its face.
    #[test]
    fn the_grab_points_are_at_the_long_ends() {
        let door = end_points(Vec3::new(1.0, 0.95, 0.12));
        assert_eq!(door.len(), 2);
        assert!((door[0].local.translation - Vec3::X * 0.44).length() < 1e-4);
        assert!((door[1].local.translation + Vec3::X * 0.44).length() < 1e-4);
    }

    /// A heavy thing's speed comes round slowly; a light one's at once.
    #[test]
    fn heavy_things_swing() {
        let dt = 1.0 / 64.0;
        let stop = swung(Vec3::X * 2.0, Vec3::ZERO, ARMS, 16.0, dt);
        assert!(stop.x > 1.4, "a car door carries on: {stop}");
        let light = swung(Vec3::X * 2.0, Vec3::ZERO, ARMS, 0.5, dt);
        assert!(light.length() < 1e-4, "a magazine stops dead: {light}");
    }

    /// Carried, a door slows you by about a third, a panel by an eighth;
    /// dragged, anything halves your pace.
    #[test]
    fn hauling_slows_you_by_the_load() {
        assert!((haul_of(16.0, true) - 0.35).abs() < 0.02);
        assert!((haul_of(6.0, true) - 0.13).abs() < 0.02);
        assert_eq!(haul_of(6.0, false), DRAG_SLOW);
        assert!(haul_of(70.0, false) > DRAG_SLOW, "a wreck, slower");
    }

    /// A walker's hull drags at most at about two thirds of a light thing's
    /// pace.
    #[test]
    fn the_heavier_the_slower_it_drags() {
        assert_eq!(drag_speed(16.0), DRAG_SPEED);
        let hull = drag_speed(70.0);
        assert!(hull < DRAG_SPEED * 0.7 && hull > DRAG_SPEED * 0.5, "{hull}");
    }
}
