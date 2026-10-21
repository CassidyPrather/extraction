//! Blades in tracked hands: a knife, and the bayonets on the Mosin (always
//! fixed) and the SKS (folding; A on the gun hand swings it out or back).
//!
//! Each tick the blade's point is swept from where it was to where it is;
//! what it meets is struck with [`extraction_core::melee::strike`] from
//! the point's velocity: a thrust along the blade, a slash across its
//! edge, nothing for a slow poke. Robots' soft spots take more: eyes
//! (sensors) and joints (legs, rotors); the core and armour shrug most of
//! it off. A strike is a [`Hit`], so robots take it, impacts show it and
//! the hand feels it (a hit is a kick).
//!
//! A long gun's butt is a blunt weapon too: swung or driven butt-first,
//! it strikes from its speed and the gun's weight ([`strike_butts`]).
//! Joints and optics feel a butt most. A part made to be beaten or cut (a
//! heavy's bolts) carries its own [`WeakPoint`] multiplier. Each strike's
//! `Hit` says what kind of blow it was (point, edge or blunt), so wood can
//! weigh a butt over a knife.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::melee::{MeleeDef, strike};

use super::Gun;
use super::ballistics::Hit;
use crate::interact::{GrabPoint, Grabbable, GripRole, Held};
use crate::items::{Catalog, Item};
use crate::player::Player;
use crate::rig::RigInput;
use crate::world::Layer;

/// A knife's points in its own space: the palm on the handle, the point,
/// the blade along -Z with its edge down (-Y).
pub const KNIFE_GRIP: Vec3 = Vec3::new(0.0, 0.0, 0.07);
pub const KNIFE_TIP: Vec3 = Vec3::new(0.0, 0.0, -0.125);

/// The point of a fixed bayonet in gun space (Kit's `pt_bayonet_tip`).
pub fn bayonet_tip(gun: &str) -> Option<Vec3> {
    match gun {
        "mosin" => Some(Vec3::new(0.0, 0.024, -1.425)),
        "sks" => Some(Vec3::new(0.0, 0.0265, -0.975)),
        _ => None,
    }
}

/// The bayonet each gun's blade is, as a blade.
fn bayonet_def() -> MeleeDef {
    MeleeDef {
        min_speed: 1.8,
        damage: 45.0,
        blade: 0.3,
    }
}

/// How much a part feels a blade.
pub fn softness(role: crate::robots::PartRole) -> f32 {
    use crate::robots::PartRole::*;
    match role {
        Sensor => 2.0,
        Leg(_) | Rotor(_) | GasCell(_) => 1.5,
        Weapon => 0.6,
        Core => 0.4,
        #[allow(unreachable_patterns)]
        _ => 0.3,
    }
}

/// How much a part feels a blunt blow (a rifle butt): joints and optics
/// most, a gas cell's skin little, plate and the core's shell hardly.
pub fn blunt_softness(role: crate::robots::PartRole) -> f32 {
    use crate::robots::PartRole::*;
    match role {
        Sensor | Leg(_) | Rotor(_) => 1.5,
        Weapon => 1.0,
        GasCell(_) => 0.5,
        Core => 0.25,
        #[allow(unreachable_patterns)]
        _ => 0.3,
    }
}

/// A part a melee blow is meant for (a heavy's bolts, a seam): its own
/// multiplier, in place of its role's, for blades and butts alike.
#[derive(Component, Debug, Clone, Copy)]
pub struct WeakPoint(pub f32);

/// A long gun's butt as a blunt weapon: where it was last tick, and what
/// it struck lately.
#[derive(Component, Debug, Default)]
pub struct ButtStroke {
    prev: Option<Vec3>,
    lately: Vec<(Entity, f32)>,
}

/// A gun's bayonet: whether it's out.
#[derive(Component, Debug, Clone, Copy)]
pub struct Bayonet {
    pub fixed: bool,
}

/// A blade's point last tick, and who it struck lately (one strike a
/// target per thrust).
#[derive(Component, Debug, Default)]
pub struct Blade {
    prev: Option<Vec3>,
    lately: Vec<(Entity, f32)>,
}

/// One strike against one thing, at most this often, seconds.
const RESTRIKE: f32 = 0.35;

pub struct MeleePlugin;

impl Plugin for MeleePlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(setup_blades)
            .add_observer(setup_bayonets)
            .add_systems(
                FixedUpdate,
                (fold_bayonets, strike_blades, strike_butts)
                    .chain()
                    .after(crate::interact::InteractSystems::Follow)
                    .before(super::ballistics::BallisticsSystems),
            );
    }
}

fn setup_blades(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).melee.is_none() {
        return;
    }
    commands.entity(add.entity).insert((
        Blade::default(),
        Grabbable {
            points: vec![GrabPoint::new(
                Transform::from_translation(KNIFE_GRIP),
                GripRole::Primary,
                0.1,
            )],
        },
    ));
}

fn setup_bayonets(
    add: On<Add, Gun>,
    mut commands: Commands,
    items: Query<&Item>,
    guns: Query<&Gun>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    let id = &catalog.get(item.kind()).id;
    if let Ok(gun) = guns.get(add.entity) {
        if super::feel::butt(gun).is_some() {
            commands.entity(add.entity).insert(ButtStroke::default());
        }
    }
    if bayonet_tip(id).is_some() {
        commands.entity(add.entity).insert((
            Bayonet {
                fixed: id == "mosin",
            },
            Blade::default(),
        ));
    }
}

/// A on the gun hand swings an SKS's bayonet out or back (it has no
/// magazine to drop).
fn fold_bayonets(
    players: Query<&RigInput, With<Player>>,
    mut guns: Query<(Entity, &mut Bayonet, &Held, &Item)>,
    catalog: Res<Catalog>,
    mut last: Local<std::collections::HashMap<Entity, bool>>,
) {
    for (e, mut b, held, item) in &mut guns {
        if catalog.get(item.kind()).id != "sks" {
            continue;
        }
        let Ok(input) = players.get(held.player) else {
            continue;
        };
        let down = input.hand(held.side).primary;
        let was = last.insert(e, down).unwrap_or(false);
        if down && !was {
            b.fixed = !b.fixed;
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn strike_blades(
    mut blades: Query<(
        Entity,
        &mut Blade,
        &GlobalTransform,
        &Item,
        Option<&Held>,
        Option<&Bayonet>,
    )>,
    parts: Query<&crate::robots::RobotPart>,
    weak: Query<&WeakPoint>,
    parents: Query<&ChildOf>,
    hands: Query<&crate::interact::Hand>,
    catalog: Res<Catalog>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
) {
    let dt = time.delta_secs().max(1e-4);
    let now = time.elapsed_secs();
    for (e, mut blade, gt, item, held, bayonet) in &mut blades {
        blade.lately.retain(|(_, t)| now - *t < RESTRIKE);
        // Only a blade in a hand strikes; a bayonet only when it's out.
        let (Some(held), true) = (held, bayonet.is_none_or(|b| b.fixed)) else {
            blade.prev = None;
            continue;
        };
        let def = catalog.get(item.kind());
        let (tip_local, mdef) = match (&def.melee, bayonet_tip(&def.id)) {
            (Some(m), _) => (KNIFE_TIP, m.clone()),
            (None, Some(tip)) => (tip, bayonet_def()),
            _ => continue,
        };
        // Where the hand drives the blade, not where it came to rest against
        // what it hit: the strike is the hand's.
        let pose = hands
            .get(held.hand)
            .map(|h| h.world.mul_transform(held.offset))
            .unwrap_or_else(|_| gt.compute_transform());
        let tip = pose.transform_point(tip_local);
        let Some(prev) = blade.prev.replace(tip) else {
            continue;
        };
        let step = tip - prev;
        let Ok(dir) = Dir3::new(step) else {
            continue;
        };
        // The blade's own items and the one holding it don't count.
        let own = |h: Entity| {
            let mut cur = h;
            for _ in 0..6 {
                if cur == e || cur == held.player {
                    return true;
                }
                match parents.get(cur) {
                    Ok(c) => cur = c.parent(),
                    Err(_) => return false,
                }
            }
            false
        };
        let filter = SpatialQueryFilter::from_mask([Layer::Robot, Layer::World, Layer::Item]);
        let Some(contact) =
            spatial.cast_ray_predicate(prev, dir, step.length(), true, &filter, &|h| !own(h))
        else {
            continue;
        };
        if blade.lately.iter().any(|(t, _)| *t == contact.entity) {
            continue;
        }
        let v = step / dt;
        let along = v.dot(pose.rotation * Vec3::NEG_Z);
        let across = v.dot(pose.rotation * Vec3::NEG_Y);
        let Some((harm, blow)) = strike(&mdef, along, across) else {
            continue;
        };
        let soft = weak
            .get(contact.entity)
            .map(|w| w.0)
            .or_else(|_| parts.get(contact.entity).map(|p| softness(p.role)))
            .unwrap_or(1.0);
        blade.lately.push((contact.entity, now));
        hits.write(Hit {
            entity: contact.entity,
            point: prev + *dir * contact.distance,
            normal: contact.normal,
            velocity: v,
            damage: harm * soft,
            shooter: Some(held.player),
            mass_g: 0.0,
            load: Default::default(),
            blow: Some(blow),
        });
        haptics.write(crate::rig::haptics::Haptic {
            side: held.side,
            amplitude: (0.6 + harm / 100.0).min(1.0),
            seconds: 0.05,
        });
    }
}

/// A long gun swung butt-first: its butt plate, swept tick to tick, strikes
/// what it meets blunt, from its speed into it and the gun's weight
/// ([`extraction_core::melee::blunt`]). Glass breaks on its own; wood and
/// robots take the [`Hit`].
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn strike_butts(
    mut guns: Query<(Entity, &mut ButtStroke, &Gun, &Item, Option<&Held>)>,
    parts: Query<&crate::robots::RobotPart>,
    weak: Query<&WeakPoint>,
    parents: Query<&ChildOf>,
    hands: Query<&crate::interact::Hand>,
    catalog: Res<Catalog>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
) {
    let dt = time.delta_secs().max(1e-4);
    let now = time.elapsed_secs();
    for (e, mut stroke, gun, item, held) in &mut guns {
        stroke.lately.retain(|(_, t)| now - *t < RESTRIKE);
        let (Some(held), Some(butt)) = (held, super::feel::butt(gun)) else {
            stroke.prev = None;
            continue;
        };
        let Ok(hand) = hands.get(held.hand) else {
            continue;
        };
        // Where the hand drives it, as with a blade.
        let pose = hand.world.mul_transform(held.offset);
        let at = pose.transform_point(butt);
        let Some(prev) = stroke.prev.replace(at) else {
            continue;
        };
        let step = at - prev;
        let Ok(dir) = Dir3::new(step) else {
            continue;
        };
        let own = |h: Entity| {
            let mut cur = h;
            for _ in 0..6 {
                if cur == e || cur == held.player {
                    return true;
                }
                match parents.get(cur) {
                    Ok(c) => cur = c.parent(),
                    Err(_) => return false,
                }
            }
            false
        };
        let filter = SpatialQueryFilter::from_mask([Layer::Robot, Layer::World, Layer::Item]);
        let Some(contact) =
            spatial.cast_ray_predicate(prev, dir, step.length(), true, &filter, &|h| !own(h))
        else {
            continue;
        };
        // Starting inside it is drawing out of it, not a blow.
        if contact.distance <= 1e-4 || stroke.lately.iter().any(|(t, _)| *t == contact.entity) {
            continue;
        }
        let v = step / dt;
        let into = v.dot(-contact.normal).max(0.0);
        let mass = catalog.get(item.kind()).mass;
        let Some(harm) = extraction_core::melee::blunt(mass, into) else {
            continue;
        };
        let soft = weak
            .get(contact.entity)
            .map(|w| w.0)
            .or_else(|_| parts.get(contact.entity).map(|p| blunt_softness(p.role)))
            .unwrap_or(1.0);
        stroke.lately.push((contact.entity, now));
        hits.write(Hit {
            entity: contact.entity,
            point: prev + *dir * contact.distance,
            normal: contact.normal,
            velocity: v,
            damage: harm * soft,
            shooter: Some(held.player),
            mass_g: 0.0,
            load: Default::default(),
            blow: Some(extraction_core::melee::Blow::Blunt),
        });
        haptics.write(crate::rig::haptics::Haptic {
            side: held.side,
            amplitude: (0.7 + harm / 150.0).min(1.0),
            seconds: 0.08,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Side;
    use crate::robots::{PartRole, RobotPart};
    use crate::vr_harness::Vr;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    /// A knife in the right hand, and a robot's sensor and core standing
    /// in front of it.
    fn setup() -> (Vr, Vec3, Entity, Entity) {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let at = feet + Vec3::new(0.2, 1.2, -0.3);
        let knife = vr.spawn("knife", Transform::from_translation(at));
        vr.app
            .world_mut()
            .entity_mut(knife)
            .insert(RigidBody::Static);
        let grip = vr.point(knife, |r| *r == GripRole::Primary);
        vr.hand(Side::Right, grip, Quat::IDENTITY, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(knife)
            .insert(RigidBody::Dynamic);
        vr.step(10);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(knife));
        let part = |vr: &mut Vr, role, x: f32| {
            let robot = vr.app.world_mut().spawn_empty().id();
            vr.app
                .world_mut()
                .spawn((
                    RobotPart {
                        robot,
                        role,
                        hp: 100.0,
                        max_hp: 100.0,
                    },
                    Transform::from_translation(feet + Vec3::new(x, 1.2, -0.75)),
                    RigidBody::Static,
                    Collider::cuboid(0.2, 0.2, 0.2),
                    CollisionLayers::new(Layer::Robot, LayerMask::ALL),
                ))
                .id()
        };
        let eye = part(&mut vr, PartRole::Sensor, 0.2);
        let core = part(&mut vr, PartRole::Core, 0.9);
        vr.step(2);
        (vr, feet, eye, core)
    }

    fn hp(vr: &Vr, e: Entity) -> f32 {
        vr.app.world().get::<RobotPart>(e).unwrap().hp
    }

    /// The hand drives the knife forward at `speed` m/s along -Z for
    /// `metres`, from `x`, then draws it back.
    fn thrust(vr: &mut Vr, feet: Vec3, x: f32, speed: f32, metres: f32) {
        let start = feet + Vec3::new(x, 1.2, -0.3);
        vr.move_hand(Side::Right, start);
        vr.step(10);
        let steps = (metres / (speed / 90.0)).ceil() as usize;
        for i in 1..=steps {
            vr.move_hand(Side::Right, start + Vec3::NEG_Z * (speed / 90.0) * i as f32);
            vr.step(1);
        }
        vr.move_hand(Side::Right, start);
        vr.step(20);
    }

    /// Pressed slowly into a robot's eye, the knife does nothing; driven
    /// in hard, it hurts.
    #[test]
    fn a_slow_poke_doesnt_hurt_a_hard_thrust_does() {
        let (mut vr, feet, eye, _) = setup();
        thrust(&mut vr, feet, 0.2, 0.6, 0.35);
        assert_eq!(hp(&vr, eye), 100.0, "a slow poke");
        thrust(&mut vr, feet, 0.2, 5.0, 0.35);
        assert!(hp(&vr, eye) < 60.0, "a hard thrust: {}", hp(&vr, eye));
    }

    /// The same thrust into the eye does several times what it does to
    /// the core's plate.
    #[test]
    fn the_soft_spots_take_more() {
        let (mut vr, feet, eye, core) = setup();
        thrust(&mut vr, feet, 0.2, 4.0, 0.35);
        thrust(&mut vr, feet, 0.9, 4.0, 0.35);
        let (to_eye, to_core) = (100.0 - hp(&vr, eye), 100.0 - hp(&vr, core));
        assert!(to_core > 0.0, "the core felt it");
        assert!(to_eye > to_core * 3.0, "eye {to_eye}, core {to_core}");
    }

    /// An AK in the right hand, held level, and a robot's part of `role`
    /// standing just behind its butt.
    fn butt_setup(role: PartRole) -> (Vr, Entity, Entity, Vec3) {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        let at = feet + Vec3::new(0.3, 1.2, -0.6);
        let gun = vr.spawn("ak74", Transform::from_translation(at));
        vr.app.world_mut().entity_mut(gun).insert(RigidBody::Static);
        let grip = vr.point(gun, |r| *r == GripRole::Primary);
        let rake = vr.app.world().get::<Gun>(gun).unwrap().layout.grip.rotation;
        vr.hand(Side::Right, grip, rake, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(gun)
            .insert(RigidBody::Dynamic);
        vr.step(20);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(gun));
        let butt = {
            let g = vr.app.world().get::<Gun>(gun).unwrap();
            let b = super::super::feel::butt(g).unwrap();
            vr.app
                .world()
                .get::<GlobalTransform>(gun)
                .unwrap()
                .transform_point(b)
        };
        let robot = vr.app.world_mut().spawn_empty().id();
        let part = vr
            .app
            .world_mut()
            .spawn((
                RobotPart {
                    robot,
                    role,
                    hp: 100.0,
                    max_hp: 100.0,
                },
                Transform::from_translation(butt + Vec3::new(0.0, 0.0, 0.2)),
                RigidBody::Static,
                Collider::cuboid(0.2, 0.2, 0.2),
                CollisionLayers::new(Layer::Robot, LayerMask::ALL),
            ))
            .id();
        vr.step(2);
        let hand = vr_hand(&mut vr);
        (vr, gun, part, hand)
    }

    fn vr_hand(vr: &mut Vr) -> Vec3 {
        vr.hand_state(Side::Right).world.translation
    }

    /// The hand drives the gun butt-first (back, +Z) at `speed` m/s for
    /// `metres`, then brings it back.
    fn butt_stroke(vr: &mut Vr, from: Vec3, speed: f32, metres: f32) {
        let steps = (metres / (speed / 90.0)).ceil() as usize;
        for i in 1..=steps {
            vr.move_hand(Side::Right, from + Vec3::Z * (speed / 90.0) * i as f32);
            vr.step(1);
        }
        // Back at a walk.
        let end = from + Vec3::Z * (speed / 90.0) * steps as f32;
        for i in 1..=30 {
            vr.move_hand(Side::Right, end.lerp(from, i as f32 / 30.0));
            vr.step(1);
        }
        vr.step(10);
    }

    /// Nudged butt-first into a robot's eye, an AK does nothing; driven
    /// back hard, its butt hurts, and the blow is a blunt one.
    #[test]
    fn a_butt_stroke_hurts_a_nudge_doesnt() {
        #[derive(Resource, Default)]
        struct Blows(Vec<Option<extraction_core::melee::Blow>>);
        let (mut vr, _, eye, hand) = butt_setup(PartRole::Sensor);
        vr.app.init_resource::<Blows>().add_systems(
            FixedPostUpdate,
            |mut hits: MessageReader<Hit>, mut b: ResMut<Blows>| {
                b.0.extend(hits.read().map(|h| h.blow))
            },
        );
        butt_stroke(&mut vr, hand, 1.0, 0.3);
        assert_eq!(hp(&vr, eye), 100.0, "a nudge");
        butt_stroke(&mut vr, hand, 6.0, 0.35);
        assert!(hp(&vr, eye) < 50.0, "a butt stroke: {}", hp(&vr, eye));
        let blows = &vr.app.world().resource::<Blows>().0;
        assert!(
            blows.contains(&Some(extraction_core::melee::Blow::Blunt)),
            "{blows:?}"
        );
    }

    /// The same stroke does several times to an eye what it does to the
    /// core's shell; a part marked a weak point takes its own multiplier.
    #[test]
    fn a_butt_finds_the_weak_points() {
        let damage = |role, weak: Option<f32>| {
            let (mut vr, _, part, hand) = butt_setup(role);
            if let Some(w) = weak {
                vr.app.world_mut().entity_mut(part).insert(WeakPoint(w));
            }
            butt_stroke(&mut vr, hand, 6.0, 0.35);
            100.0 - hp(&vr, part)
        };
        let eye = damage(PartRole::Sensor, None);
        let core = damage(PartRole::Core, None);
        let bolt = damage(PartRole::Armour(10), Some(1.5));
        assert!(core > 0.0, "the core felt it");
        assert!(eye > core * 4.0, "eye {eye}, core {core}");
        assert!(
            (bolt - eye).abs() < eye * 0.05,
            "a weak point: {bolt} (eye {eye})"
        );
    }
}
