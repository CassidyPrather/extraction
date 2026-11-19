//! The injector pen (#102): two slots, each taking a cartridge (an item
//! with `mount: Cartridge`), and one jab gives what's in both
//! (`items::usage`). Half heal, half stim is a REGEN in one slot and a
//! STIM in the other: what you put in is the mix.
//!
//! A cartridge's grade is its `dose:` tag (`dose:regen:60`: 60 HP when
//! full; `dose:stim:6`: six real seconds of slowed world when full), and
//! how full it is is its condition (0..1): a part-used one gives its share,
//! and a used one is left in the slot, empty, till it's taken out.
//!
//! A pen comes with a REGEN R-2 in its first slot ([`prime_pens`]), once
//! (one found lying about, an R-1: [`FOUND_WITH`], so a found pen keeps to
//! the medical table's grades, the R-2 out from raid 2):
//! its `rounds` (a pen has none) records that it has (`1`), so a pen saved
//! empty loads empty. An old save's single-use injector loads as a new
//! pen, R-2 and all.
//!
//! Taken up, a pen turns about its length so its wells (its top, +Y) face
//! the eyes ([`wells_to`]): how full each cartridge is reads at a glance.
//! Its needle points where it did as it was taken.

use bevy::prelude::*;

use super::{Catalog, Item, SpawnItem, spawn_item};
use crate::interact::{Socket, SocketKind, SocketOwner};
use extraction_core::items::{ItemDef, Mount};

/// The pen's id (the injector's, kept so saves and kits carry over).
pub const PEN: &str = "regen_injector";
/// What a new pen comes with.
pub const PRIMED_WITH: &str = "regen_r2";
/// What a pen found lying about comes with (`loot::Found`).
pub const FOUND_WITH: &str = "regen_r1";
/// The slots, in the pen's frame (it lies along Z, the needle at -Z): two
/// wells side by side in its body, open on top (each a cartridge's centre
/// line).
pub const SLOTS: [Vec3; 2] = [
    Vec3::new(0.0075, 0.002, 0.025),
    Vec3::new(-0.0075, 0.002, 0.025),
];
/// How near a slot a cartridge let go seats in it (m).
pub const SLOT_REACH: f32 = 0.06;

/// One of a pen's slots (0 or 1).
#[derive(Component, Clone, Copy, Debug)]
pub struct PenSlot(pub usize);

/// What a cartridge gives, full.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dose {
    /// HP.
    Regen(f32),
    /// Real seconds of slowed world.
    Stim(f32),
}

impl Dose {
    /// A cartridge's grade, from its `dose:` tag.
    pub fn of(def: &ItemDef) -> Option<Dose> {
        def.tags.iter().find_map(|t| {
            let mut parts = t.strip_prefix("dose:")?.split(':');
            let (kind, amount) = (parts.next()?, parts.next()?.parse::<f32>().ok()?);
            match kind {
                "regen" => Some(Dose::Regen(amount)),
                "stim" => Some(Dose::Stim(amount)),
                _ => None,
            }
        })
    }

    /// The share a cartridge `fill` full gives.
    pub fn times(self, fill: f32) -> Dose {
        let k = fill.clamp(0.0, 1.0);
        match self {
            Dose::Regen(a) => Dose::Regen(a * k),
            Dose::Stim(a) => Dose::Stim(a * k),
        }
    }
}

/// What one jab gives: HP back, and seconds of stim (each summed over the
/// slots).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Jab {
    pub heal: f32,
    pub stim: f32,
}

impl Jab {
    pub fn is_empty(&self) -> bool {
        self.heal <= 0.0 && self.stim <= 0.0
    }
}

/// The sum of what's in `cartridges` (each a grade and how full it is).
pub fn jab_of(cartridges: impl IntoIterator<Item = (Dose, f32)>) -> Jab {
    let mut j = Jab::default();
    for (dose, fill) in cartridges {
        match dose.times(fill) {
            Dose::Regen(hp) => j.heal += hp,
            Dose::Stim(s) => j.stim += s,
        }
    }
    j
}

/// The player in a stim: seconds of it left, of how many (piece 3 of #102
/// slows the world while it's on).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Stimmed {
    pub left: f32,
    pub total: f32,
}

pub struct PenPlugin;

impl Plugin for PenPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(pen_slots)
            .add_observer(wells_up)
            .add_systems(Update, prime_pens);
    }
}

/// A pen gets its two slots.
fn pen_slots(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).id != PEN {
        return;
    }
    for (i, at) in SLOTS.iter().enumerate() {
        commands.spawn((
            Name::new("Pen slot"),
            PenSlot(i),
            Socket {
                kind: SocketKind::Mount(Mount::Cartridge),
                occupant: None,
                radius: SLOT_REACH,
                seat: Transform::IDENTITY,
            },
            SocketOwner(add.entity),
            Transform::from_translation(*at),
            Visibility::default(),
            ChildOf(add.entity),
        ));
    }
}

/// A pen held at `offset` (in its hand's frame), turned about its length
/// so its wells (+Y) face `eye` (in the hand's frame too).
pub fn wells_to(offset: Transform, eye: Vec3) -> Transform {
    let to = offset.rotation.inverse() * (eye - offset.translation);
    if to.x.abs() + to.y.abs() < 1e-6 {
        return offset;
    }
    // +Y turned by `a` about Z is (-sin a, cos a).
    let a = (-to.x).atan2(to.y);
    offset.with_rotation(offset.rotation * Quat::from_rotation_z(a))
}

/// A pen taken up turns its wells to the eyes.
fn wells_up(
    add: On<Add, crate::interact::Held>,
    mut held: Query<(&mut crate::interact::Held, &Item)>,
    hands: Query<&crate::interact::Hand>,
    heads: Query<(&GlobalTransform, &ChildOf), With<crate::rig::RigHead>>,
    catalog: Res<Catalog>,
) {
    let Ok((mut h, item)) = held.get_mut(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).id != PEN {
        return;
    }
    let Ok(hand) = hands.get(h.hand) else {
        return;
    };
    let Some(eye) = heads
        .iter()
        .find(|(_, p)| p.parent() == h.player)
        .map(|(g, _)| g.translation())
    else {
        return;
    };
    let eye = hand.world.compute_affine().inverse().transform_point3(eye);
    h.offset = wells_to(h.offset, eye);
}

/// A new pen (`rounds` 0) gets its R-2, once; one found, an R-1.
fn prime_pens(
    mut commands: Commands,
    mut pens: Query<(Entity, &mut Item, Has<super::loot::Found>)>,
    catalog: Res<Catalog>,
) {
    for (pen, mut item, found) in &mut pens {
        if item.0.rounds != 0 || catalog.get(item.kind()).id != PEN {
            continue;
        }
        let Some(kind) = catalog.kind(if found { FOUND_WITH } else { PRIMED_WITH }) else {
            continue;
        };
        item.0.rounds = 1;
        let c = spawn_item(
            &mut commands,
            &catalog,
            SpawnItem {
                kind,
                transform: Transform::default(),
                condition: 1.0,
            },
        );
        commands.queue(move |world: &mut World| {
            crate::persist::seat_in_mount(world, pen, c);
        });
    }
}

/// The well a cartridge goes in, loaded by hand (flat mode's combine key,
/// the bot's): the first empty one, else the first whose cartridge is
/// spent (to swap it out). `wells` are each well's index and what's in it,
/// as `Some(how full)`. None if both hold something still good.
pub fn well_for(wells: &[(usize, Option<f32>)]) -> Option<usize> {
    let mut w = wells.to_vec();
    w.sort_by_key(|(i, _)| *i);
    w.iter()
        .find(|(_, c)| c.is_none())
        .or_else(|| w.iter().find(|(_, c)| c.is_some_and(|f| f <= 0.0)))
        .map(|(i, _)| *i)
}

/// A held cartridge `cart` loaded into a held `pen` without bringing hand
/// to well (flat mode, the bot): it goes in the well [`well_for`] picks,
/// and a spent one there comes out into the hand the new one left. Both
/// wells full of good ones: nothing happens. True if it went in.
pub fn load_by_hand(world: &mut World, pen: Entity, cart: Entity) -> bool {
    use crate::interact::{Hand, HandAction, Held, InSocket, InteractEvent, Supported};
    use avian3d::prelude::{ColliderDisabled, RigidBodyDisabled};
    // A spent one isn't worth loading (and, swapped out onto the pen, one
    // would swap back in for the other spent one, and back, every touch).
    if world
        .get::<Item>(cart)
        .is_none_or(|it| it.0.condition <= 0.0)
    {
        return false;
    }
    // Already in one of this pen's wells: it stays.
    if world
        .get::<InSocket>(cart)
        .and_then(|s| world.get::<crate::interact::SocketOwner>(s.socket))
        .is_some_and(|o| o.0 == pen)
    {
        return false;
    }
    let Some(children) = world.get::<Children>(pen).map(|c| c.to_vec()) else {
        return false;
    };
    let wells: Vec<(Entity, usize, Option<Entity>)> = children
        .iter()
        .filter_map(|s| {
            let i = world.get::<PenSlot>(*s)?.0;
            Some((*s, i, world.get::<Socket>(*s)?.occupant))
        })
        .collect();
    let fills: Vec<(usize, Option<f32>)> = wells
        .iter()
        .map(|(_, i, o)| {
            (
                *i,
                o.map(|c| world.get::<Item>(c).map_or(0.0, |it| it.0.condition)),
            )
        })
        .collect();
    let Some(want) = well_for(&fills) else {
        return false;
    };
    let Some(&(slot, _, spent)) = wells.iter().find(|(_, i, _)| *i == want) else {
        return false;
    };
    // Out of wherever it was: a pocket (one-handed, the pen touched to
    // one on you), the pack.
    if let Some(from) = world.get::<InSocket>(cart).map(|s| s.socket) {
        if let Some(mut s) = world.get_mut::<Socket>(from) {
            if s.occupant == Some(cart) {
                s.occupant = None;
            }
        }
        world.write_message(InteractEvent::Unsocketed {
            item: cart,
            socket: from,
        });
    }
    if let Some(bag) = world
        .get::<crate::interact::body::InBackpack>(cart)
        .map(|b| b.bag)
    {
        if let Some(mut b) = world.get_mut::<crate::interact::body::Backpack>(bag) {
            b.contents.retain(|e| *e != cart);
        }
        world
            .entity_mut(cart)
            .remove::<crate::interact::body::InBackpack>();
    }
    // Out of its hand.
    let held = world.get::<Held>(cart).copied();
    if let Some(h) = held {
        if let Some(mut hand) = world.get_mut::<Hand>(h.hand) {
            hand.holding = None;
        }
    }
    let at = world
        .get::<GlobalTransform>(cart)
        .map(|g| g.compute_transform())
        .unwrap_or_default();
    // A spent one out, where the new one was.
    if let Some(old) = spent {
        if let Some(mut s) = world.get_mut::<Socket>(slot) {
            s.occupant = None;
        }
        world
            .entity_mut(old)
            .remove::<(ChildOf, InSocket, RigidBodyDisabled, ColliderDisabled)>()
            .insert((
                at,
                avian3d::prelude::Position(at.translation),
                avian3d::prelude::Rotation(at.rotation),
                avian3d::prelude::LinearVelocity::ZERO,
            ));
        world.write_message(InteractEvent::Unsocketed {
            item: old,
            socket: slot,
        });
        if let Some(h) = held {
            world.write_message(HandAction::Grab {
                player: h.player,
                side: h.side,
                item: old,
                point: None,
            });
        }
    }
    // The new one in.
    let seat = {
        let Some(mut s) = world.get_mut::<Socket>(slot) else {
            return false;
        };
        s.occupant = Some(cart);
        s.seat
    };
    world
        .entity_mut(cart)
        .remove::<(Held, Supported)>()
        .insert((
            InSocket { socket: slot },
            RigidBodyDisabled,
            ColliderDisabled,
            ChildOf(slot),
            seat,
        ));
    world.write_message(InteractEvent::Socketed {
        item: cart,
        socket: slot,
    });
    true
}

/// The cartridges in a pen's slots: each, its grade, how full.
pub fn cartridges_in(
    pen: Entity,
    children: &Query<&Children>,
    slots: &Query<&Socket, With<PenSlot>>,
    items: &Query<&Item>,
    catalog: &Catalog,
) -> Vec<(Entity, Dose, f32)> {
    children
        .get(pen)
        .into_iter()
        .flat_map(|c| c.iter())
        .filter_map(|s| slots.get(s).ok()?.occupant)
        .filter_map(|c| {
            let it = items.get(c).ok()?;
            Some((c, Dose::of(catalog.get(it.kind()))?, it.0.condition))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cartridge's grade is its tag; a part-used one gives its share; a
    /// regen and a stim in one pen give both.
    /// By hand, a cartridge goes in the first empty well, else in place of
    /// a spent one; two good ones and it doesn't go.
    #[test]
    fn a_cartridge_by_hand_takes_an_empty_well_then_a_spent_one() {
        assert_eq!(well_for(&[(0, Some(1.0)), (1, None)]), Some(1));
        assert_eq!(well_for(&[(1, None), (0, None)]), Some(0));
        assert_eq!(well_for(&[(0, Some(0.5)), (1, Some(0.0))]), Some(1));
        assert_eq!(well_for(&[(0, Some(0.0)), (1, Some(0.0))]), Some(0));
        assert_eq!(well_for(&[(0, Some(1.0)), (1, Some(0.3))]), None);
    }

    #[test]
    fn a_jab_gives_what_is_in_both_slots() {
        let catalog =
            extraction_core::items::ItemCatalog::from_ron(extraction_core::items::BUILTIN_CATALOG)
                .unwrap();
        let def = |id: &str| catalog.get(catalog.kind(id).unwrap()).clone();
        assert_eq!(Dose::of(&def("regen_r1")), Some(Dose::Regen(30.0)));
        assert_eq!(Dose::of(&def("regen_r2")), Some(Dose::Regen(60.0)));
        assert_eq!(Dose::of(&def("regen_r3")), Some(Dose::Regen(100.0)));
        assert_eq!(Dose::of(&def("stim_s1")), Some(Dose::Stim(6.0)));
        assert_eq!(Dose::of(&def("stim_s2")), Some(Dose::Stim(12.0)));
        assert_eq!(Dose::of(&def(PEN)), None, "the pen itself gives nothing");
        // Half heal, half stim: an R-2 half used, an S-1 full.
        let j = jab_of([(Dose::Regen(60.0), 0.5), (Dose::Stim(6.0), 1.0)]);
        assert_eq!(
            j,
            Jab {
                heal: 30.0,
                stim: 6.0
            }
        );
        // Two regens add up; empties give nothing.
        let j = jab_of([(Dose::Regen(30.0), 1.0), (Dose::Regen(60.0), 0.0)]);
        assert_eq!(
            j,
            Jab {
                heal: 30.0,
                stim: 0.0
            }
        );
        assert!(jab_of([]).is_empty());
        // Turned to the eyes about its length: its top faces them, its
        // needle where it was.
        let held = Transform::from_xyz(0.0, 0.0, -0.05)
            .with_rotation(Quat::from_rotation_x(0.7) * Quat::from_rotation_z(2.5));
        let eye = Vec3::new(0.1, 0.3, 0.4);
        let t = wells_to(held, eye);
        let to_eye = (eye - t.translation).normalize();
        let along = t.rotation * Vec3::Z;
        let off_axis = (to_eye - along * to_eye.dot(along)).normalize();
        assert!(
            (t.rotation * Vec3::Y).dot(off_axis) > 0.999,
            "its top to the eyes"
        );
        assert!(
            along.dot(held.rotation * Vec3::Z) > 0.9999,
            "its needle unmoved"
        );
        assert_eq!(t.translation, held.translation);
        // Every cartridge fits the pen's slots.
        for id in ["regen_r1", "regen_r2", "regen_r3", "stim_s1", "stim_s2"] {
            assert_eq!(def(id).fits_mount(), Some(Mount::Cartridge), "{id}");
        }
    }
}
