//! Worn armour: a plate carrier (the vest) with a plate in its front or
//! back pocket, and a helmet. A round that strikes the player where a
//! plate or the helmet covers goes through
//! [`extraction_core::firearm_terminal::armour_strike`]: stopped (a bruise),
//! through (slowed, so weaker), or turned; and every stop wears the plate.
//!
//! - The vest goes on by letting it go at your chest (the `Vest` body
//!   slot), the helmet at your head (`Head`), a plate at either of the
//!   vest's pockets. The pocket decides front or back, so one plate serves
//!   both.
//! - Blasts and blades (a hit with no round's mass) aren't stopped by
//!   plates.
//! - [`Armour::filter`] is what the health system calls for a hit on a
//!   body.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use extraction_core::firearm_terminal::{ArmourArea, Outcome, armour_strike};
use extraction_core::inventory::SlotKind;
use extraction_core::items::Mount;
use extraction_core::noise::Rng;

use super::ballistics::Hit;
use crate::interact::body::Torso;
use crate::interact::{InSocket, Socket, SocketKind, SocketOwner};
use crate::items::{Catalog, Item};
use crate::rig::{LocalPlayer, RigHead};

/// A vest's plate pocket, and which way it faces.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlatePocket(pub ArmourArea);

/// The vest's pockets in its own space, which worn is the torso's (front
/// toward -Z): where the worn model draws its plates.
pub const POCKET_FRONT: Vec3 = Vec3::new(0.0, -0.15, -0.09);
pub const POCKET_BACK: Vec3 = Vec3::new(0.0, -0.15, 0.111);
/// What a plate covers on the torso, in the torso's frame (origin
/// mid-chest): half its width, and its bottom and top (a 25 x 30 cm
/// plate round the pockets).
pub const PLATE_HALF_WIDTH: f32 = 0.125;
pub const PLATE_SPAN: (f32, f32) = (-0.30, 0.0);
/// How far below the eyes the helmet still covers, metres.
pub const HELMET_BELOW_EYES: f32 = 0.06;
/// A stopped round still bruises: its share of the damage.
pub const BLUNT: f32 = 0.05;

/// A round met armour (for the clang, and the plate's wear).
#[derive(Message, Clone, Copy, Debug)]
pub struct ArmourStruck {
    pub player: Entity,
    pub armour: Entity,
    pub outcome: Outcome,
    pub at: Vec3,
}

pub struct ArmourPlugin;

impl Plugin for ArmourPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ArmourStruck>()
            .add_observer(vest_pockets)
            .add_systems(Update, hide_own_helmet);
    }
}

/// A vest comes with its two plate pockets.
fn vest_pockets(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).fits_mount() != Some(Mount::Vest) {
        return;
    }
    for (at, area) in [
        (POCKET_FRONT, ArmourArea::Front),
        (POCKET_BACK, ArmourArea::Back),
    ] {
        commands.spawn((
            Name::new(format!("{area:?} plate pocket")),
            Socket {
                kind: SocketKind::Mount(Mount::Plate),
                occupant: None,
                radius: 0.12,
                seat: Transform::IDENTITY,
            },
            PlatePocket(area),
            SocketOwner(add.entity),
            Transform::from_translation(at),
            Visibility::default(),
            ChildOf(add.entity),
        ));
    }
}

/// Your own helmet is on your head, not in front of your eyes.
fn hide_own_helmet(
    mut items: Query<(&InSocket, &mut Visibility), With<Item>>,
    sockets: Query<(&Socket, &SocketOwner)>,
    locals: Query<(), With<LocalPlayer>>,
) {
    for (in_socket, mut vis) in &mut items {
        let Ok((socket, owner)) = sockets.get(in_socket.socket) else {
            continue;
        };
        if socket.kind == SocketKind::Body(SlotKind::Head)
            && locals.contains(owner.0)
            && *vis != Visibility::Hidden
        {
            *vis = Visibility::Hidden;
        }
    }
}

/// What the health system needs to weigh a hit against what's worn.
#[derive(SystemParam)]
pub struct Armour<'w, 's> {
    sockets: Query<
        'w,
        's,
        (
            Entity,
            &'static Socket,
            &'static SocketOwner,
            Option<&'static PlatePocket>,
        ),
    >,
    items: Query<'w, 's, &'static mut Item>,
    torsos: Query<'w, 's, (&'static GlobalTransform, &'static ChildOf), With<Torso>>,
    heads: Query<'w, 's, (&'static GlobalTransform, &'static ChildOf), With<RigHead>>,
    catalog: Res<'w, Catalog>,
    struck: MessageWriter<'w, ArmourStruck>,
}

impl Armour<'_, '_> {
    /// What's worn in `player`'s `slot`.
    fn worn(&self, player: Entity, slot: SlotKind) -> Option<Entity> {
        self.sockets
            .iter()
            .find(|(_, s, o, _)| o.0 == player && s.kind == SocketKind::Body(slot))
            .and_then(|(_, s, ..)| s.occupant)
    }

    /// The damage `hit` does to `player` after whatever armour it met,
    /// wearing that armour as it goes.
    pub fn filter(&mut self, player: Entity, hit: &Hit) -> f32 {
        if hit.mass_g <= 0.0 {
            return hit.damage;
        }
        let speed = hit.velocity.length();
        let dir = hit.velocity / speed.max(1e-3);
        let head = self
            .heads
            .iter()
            .find(|(_, p)| p.parent() == player)
            .map(|(g, _)| g.translation());
        let torso = self
            .torsos
            .iter()
            .find(|(_, p)| p.parent() == player)
            .map(|(g, _)| *g);
        // The helmet: anything struck at the head.
        let (piece, cos_in) = if head.is_some_and(|h| hit.point.y > h.y - HELMET_BELOW_EYES) {
            let n = (hit.point - head.unwrap()).normalize_or(Vec3::Y);
            (self.worn(player, SlotKind::Head), (-dir).dot(n).abs())
        } else if let Some(torso) = torso {
            // A plate: the chest's front or back, where a plate covers.
            let local = torso.affine().inverse().transform_point3(hit.point);
            let covered =
                local.x.abs() < PLATE_HALF_WIDTH && (PLATE_SPAN.0..PLATE_SPAN.1).contains(&local.y);
            let fwd = torso.rotation() * Vec3::NEG_Z;
            // Travelling against the chest's facing, it strikes the front.
            let area = if dir.dot(fwd) < 0.0 {
                ArmourArea::Front
            } else {
                ArmourArea::Back
            };
            let plate = covered
                .then(|| self.worn(player, SlotKind::Vest))
                .flatten()
                .and_then(|vest| {
                    self.sockets
                        .iter()
                        .find(|(_, s, o, p)| {
                            o.0 == vest && p.is_some_and(|p| p.0 == area) && s.occupant.is_some()
                        })
                        .and_then(|(_, s, ..)| s.occupant)
                });
            (plate, dir.dot(fwd).abs())
        } else {
            (None, 1.0)
        };
        let Some(piece) = piece else {
            return hit.damage;
        };
        let Ok(mut item) = self.items.get_mut(piece) else {
            return hit.damage;
        };
        let Some(def) = self.catalog.get(item.kind()).armour.clone() else {
            return hit.damage;
        };
        let roll =
            Rng::new(u64::from(hit.point.x.to_bits()) << 32 | u64::from(hit.point.z.to_bits()))
                .f32();
        let (outcome, wear) = armour_strike(&def, item.condition, hit.mass_g, speed, cos_in, roll);
        item.condition = (item.condition - wear).max(0.0);
        self.struck.write(ArmourStruck {
            player,
            armour: piece,
            outcome,
            at: hit.point,
        });
        match outcome {
            Outcome::Stop => hit.damage * BLUNT,
            Outcome::Ricochet { .. } => 0.0,
            Outcome::Through { speed: out } => hit.damage * (out / speed.max(1e-3)).powi(2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::Health;
    use crate::rig::Side;
    use crate::vr_harness::Vr;
    use crate::weapons::Gun;
    use crate::weapons::ballistics::Shot;
    use avian3d::prelude::RigidBody;

    const OPEN: Vec3 = Vec3::new(0.0, 0.0, 30.0);

    fn torso(vr: &mut Vr) -> GlobalTransform {
        let mut q = vr
            .app
            .world_mut()
            .query_filtered::<&GlobalTransform, With<Torso>>();
        *q.iter(vr.app.world()).next().expect("a torso")
    }

    /// Take `item` from where it lies and let it go at `to`.
    fn put(vr: &mut Vr, item: Entity, to: Vec3) {
        vr.app
            .world_mut()
            .entity_mut(item)
            .insert(RigidBody::Static);
        let grip = vr.point(item, |_| true);
        vr.hand(Side::Right, grip, Quat::IDENTITY, 1.0, 0.0);
        vr.step(3);
        vr.app
            .world_mut()
            .entity_mut(item)
            .insert(RigidBody::Dynamic);
        vr.step(5);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(item));
        let from = vr.hand_state(Side::Right).world.translation;
        // The hand to where the item should go (it's held off the hand).
        let off = vr
            .app
            .world()
            .get::<GlobalTransform>(item)
            .unwrap()
            .translation()
            - from;
        for i in 1..=20 {
            vr.move_hand(Side::Right, from.lerp(to - off, i as f32 / 20.0));
            vr.step(1);
        }
        vr.grip(Side::Right, 0.0);
        vr.step(5);
    }

    fn worn_in(vr: &Vr, item: Entity) -> Option<SocketKind> {
        vr.app
            .world()
            .get::<InSocket>(item)
            .and_then(|s| vr.app.world().get::<Socket>(s.socket))
            .map(|s| s.kind.clone())
    }

    /// A round from `gun` from 8 m in front, at the middle of the chest
    /// plate; the HP it took.
    fn shoot_chest(vr: &mut Vr, gun: Entity, seed: u64) -> f32 {
        let t = torso(vr);
        let target = t.transform_point(Vec3::new(0.0, -0.15, 0.0));
        let fwd = t.rotation() * Vec3::NEG_Z;
        let def = vr.app.world().get::<Gun>(gun).unwrap().firearm.def.clone();
        let before = vr.app.world().get::<Health>(vr.player).unwrap().hp;
        vr.app.world_mut().write_message(Shot {
            gun,
            shooter: None,
            origin: target + fwd * 8.0,
            direction: -fwd,
            speed: def.muzzle_velocity,
            damage: def.damage,
            projectiles: 1,
            spread: 0.0,
            recoil: 0.0,
            seed,
        });
        vr.step(8);
        let after = vr.app.world().get::<Health>(vr.player).unwrap().hp;
        before - after
    }

    /// Put the vest on at your chest and a plate in its front pocket: a
    /// pistol round to the chest bruises and wears the plate; without the
    /// plate it wounds; a worn plate lets a rifle round through.
    #[test]
    fn a_plate_in_a_worn_vest_stops_a_pistol_round() {
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(OPEN.x, OPEN.z);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        let far = Transform::from_translation(feet + Vec3::new(5.0, 0.5, 5.0));
        let pm = vr.spawn("pm_pistol", far);
        let mosin = vr.spawn("mosin", far.with_translation(far.translation + Vec3::X));
        for g in [pm, mosin] {
            vr.app.world_mut().entity_mut(g).insert(RigidBody::Static);
        }

        let bare = shoot_chest(&mut vr, pm, 1);
        assert!(bare > 10.0, "a pistol round to a bare chest wounds: {bare}");
        vr.app.world_mut().get_mut::<Health>(vr.player).unwrap().hp = 100.0;

        let at = feet + Vec3::new(0.3, 1.1, -0.4);
        let vest = vr.spawn("vest", Transform::from_translation(at));
        let t = torso(&mut vr);
        put(
            &mut vr,
            vest,
            t.transform_point(Vec3::new(0.0, -0.08, -0.02)),
        );
        assert_eq!(
            worn_in(&vr, vest),
            Some(SocketKind::Body(SlotKind::Vest)),
            "the vest is worn"
        );
        let empty = shoot_chest(&mut vr, pm, 2);
        assert!(empty > 10.0, "an empty vest stops nothing: {empty}");
        vr.app.world_mut().get_mut::<Health>(vr.player).unwrap().hp = 100.0;

        let plate = vr.spawn("steel_plate", Transform::from_translation(at));
        let pocket = vr
            .app
            .world()
            .get::<GlobalTransform>(vest)
            .unwrap()
            .transform_point(POCKET_FRONT);
        put(&mut vr, plate, pocket);
        assert_eq!(
            worn_in(&vr, plate),
            Some(SocketKind::Mount(Mount::Plate)),
            "the plate is in its pocket"
        );
        let stopped = shoot_chest(&mut vr, pm, 3);
        assert!(
            stopped < bare * 0.2,
            "the plate stops it: {stopped} against {bare}"
        );
        let worn = vr.app.world().get::<Item>(plate).unwrap().condition;
        assert!(worn < 1.0, "the stop wears the plate: {worn}");

        vr.app.world_mut().get_mut::<Item>(plate).unwrap().condition = 0.3;
        vr.app.world_mut().get_mut::<Health>(vr.player).unwrap().hp = 100.0;
        let rifle = shoot_chest(&mut vr, mosin, 4);
        assert!(
            rifle > 10.0,
            "a worn plate lets a rifle round through: {rifle}"
        );
    }
}
