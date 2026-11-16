//! Where a thing let go of goes: a body slot, an open pack, back into the
//! hand (a slot in reach it doesn't fit), or down. One answer, asked by
//! the release (`apply_hand_actions`) and by what shows it beforehand
//! (`render::pockets`), so the sphere that lights is where it goes.
//!
//! [`let_go`] is the whole of it, in the order the game takes it:
//!
//! 1. Let go over a shoulder (above it, behind the neck): stowed, the
//!    best slot that takes it ([`shoulder_stow`]), else the worn pack.
//! 2. Else where it is ([`stow_target`]): the nearest socket in reach
//!    that takes it, else an open pack it's inside, else back in the hand
//!    if a slot in reach won't fit it, else down.
//! 3. Down, a gun on its sling is caught by it (`weapons::carry`).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use extraction_core::inventory::SlotKind;
use extraction_core::items::{ItemDef, SizeClass};

use super::body::{self, Backpack, BodySlot};
use super::{Socket, SocketKind, SocketOwner, accepts_item, fits_body_slot};
use crate::items::{Catalog, Item};

/// Where a thing let go of at a point goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StowTarget {
    /// Into this socket.
    Socket(Entity),
    /// Into this open pack.
    Pack(Entity),
    /// Back into the hand, with a tick: a body slot in reach it doesn't
    /// fit, and nothing else to take it. `pack_full`: it was also inside an
    /// open pack with no room.
    Refused { pack_full: Option<Entity> },
    /// Let fall (or thrown). `pack_full` as for `Refused`.
    Drop { pack_full: Option<Entity> },
}

/// A socket, as the stow sees it.
#[derive(Clone, Debug)]
pub struct SlotView {
    pub entity: Entity,
    pub at: Vec3,
    pub kind: SocketKind,
    pub radius: f32,
    pub occupied: bool,
    pub owner: Option<Entity>,
    /// A sling's shoulder: let go there, a long gun goes onto it too.
    pub shoulder: Option<Vec3>,
}

/// A pack, as the stow sees it.
#[derive(Clone, Copy, Debug)]
pub struct PackView {
    pub entity: Entity,
    pub owner: Entity,
    pub open: bool,
    /// On its owner's back (or hanging off a hand): what a stow over the
    /// shoulder goes into.
    pub carried: bool,
    /// World to the pack's own frame.
    pub inverse: bevy::math::Affine3A,
    pub half: Vec3,
    /// Room in it for the thing let go.
    pub room: bool,
}

/// How far out of a pack's half extents a thing let go still goes in.
pub const PACK_MOUTH: f32 = 1.3;

impl SlotView {
    pub fn new(
        entity: Entity,
        global: &GlobalTransform,
        socket: &Socket,
        owner: Option<&SocketOwner>,
        place: Option<&Transform>,
    ) -> Self {
        let shoulder = (socket.kind == SocketKind::Body(SlotKind::Sling))
            .then_some(place)
            .flatten()
            .map(|local| global.transform_point(body::sling_shoulder(local.translation)));
        Self {
            entity,
            at: global.translation(),
            kind: socket.kind.clone(),
            radius: socket.radius,
            occupied: socket.occupant.is_some(),
            owner: owner.map(|o| o.0),
            shoulder,
        }
    }

    /// How near `pos` is to taking it (its distance, its catch radius):
    /// a sling's shoulder counts as the sling.
    pub fn reach(&self, pos: Vec3) -> (f32, f32) {
        let d = self.at.distance(pos);
        match self.shoulder {
            Some(s) => (d.min(s.distance(pos)), self.radius.max(body::SLING_REACH)),
            None => (d, self.radius),
        }
    }

    /// Whether it takes this thing at all (empty or not).
    pub fn takes(&self, item: Entity, def: Option<&ItemDef>) -> bool {
        if self.owner == Some(item) {
            return false;
        }
        let size = def.map_or(SizeClass::Medium, |d| d.size);
        let family = def.and_then(|d| d.magazine.as_ref().map(|m| m.fits.as_str()));
        let is_gun = def.is_some_and(|d| d.firearm.is_some());
        let mount = def.and_then(|d| d.fits_mount());
        accepts_item(&self.kind, size, family, is_gun, mount)
            && def.is_none_or(|d| fits_body_slot(&self.kind, d))
    }
}

/// Where `item` (`def`), let go at `pos` by `player`, goes.
pub fn stow_target(
    item: Entity,
    def: Option<&ItemDef>,
    pos: Vec3,
    player: Entity,
    slots: impl IntoIterator<Item = SlotView>,
    packs: impl IntoIterator<Item = PackView>,
) -> StowTarget {
    // A free socket in reach that takes it, the nearest; a body slot in
    // reach it doesn't fit, noted.
    let mut target: Option<(f32, Entity)> = None;
    let mut refused = false;
    for s in slots {
        if s.occupied || s.owner == Some(item) {
            continue;
        }
        let size = def.map_or(SizeClass::Medium, |d| d.size);
        let family = def.and_then(|d| d.magazine.as_ref().map(|m| m.fits.as_str()));
        let is_gun = def.is_some_and(|d| d.firearm.is_some());
        if !accepts_item(
            &s.kind,
            size,
            family,
            is_gun,
            def.and_then(|d| d.fits_mount()),
        ) {
            continue;
        }
        if def.is_some_and(|d| !fits_body_slot(&s.kind, d)) {
            refused |= s.at.distance(pos) < s.radius;
            continue;
        }
        let (d, radius) = s.reach(pos);
        if d < radius && target.is_none_or(|(bd, _)| d < bd) {
            target = Some((d, s.entity));
        }
    }
    if let Some((_, e)) = target {
        return StowTarget::Socket(e);
    }
    // Into an open pack of theirs it's inside (or at the mouth of).
    let mut pack_full = None;
    for p in packs {
        if !p.open || p.owner != player {
            continue;
        }
        let local = p.inverse.transform_point3(pos);
        if local.abs().cmple(p.half * PACK_MOUTH).all() {
            if def.is_some() {
                if p.room {
                    return StowTarget::Pack(p.entity);
                }
                pack_full = Some(p.entity);
            }
            break;
        }
    }
    if refused {
        StowTarget::Refused { pack_full }
    } else {
        StowTarget::Drop { pack_full }
    }
}

/// A hand at `hand` is over a shoulder (above it, behind the neck), the
/// torso's frame `torso` (world to torso).
pub fn over_shoulder(torso: bevy::math::Affine3A, hand: Vec3) -> bool {
    let local = torso.transform_point3(hand);
    local.y > 0.05 && local.z > 0.0 && local.x.abs() < 0.45
}

/// Which free slot a put-away item goes to first (over the shoulder,
/// flat mode's stow). The pouches are kept for magazines (where a reload
/// reaches, and where a one-handed one takes them from, `seat_from_body`):
/// anything else goes to a pocket first, a pouch only when nothing else
/// on the body is free.
pub fn slot_rank(kind: &SocketKind, magazine: bool) -> u8 {
    match (kind, magazine) {
        (SocketKind::Body(SlotKind::Pouch), true) => 0,
        (SocketKind::Body(SlotKind::Pocket), _) => 1,
        (SocketKind::Body(SlotKind::Mount), _) => 2,
        (SocketKind::Body(SlotKind::Holster), _) => 3,
        (SocketKind::Body(SlotKind::Sling), _) => 4,
        (SocketKind::Body(SlotKind::Pouch), false) => 5,
        _ => 9,
    }
}

/// Put away without a place: the first of `player`'s free body slots that
/// takes it ([`slot_rank`]), else their worn pack if there's room in it;
/// else it stays in the hand (`pack_full` if the pack had no room).
pub fn shoulder_stow(
    def: &ItemDef,
    player: Entity,
    slots: impl IntoIterator<Item = SlotView>,
    packs: impl IntoIterator<Item = PackView>,
) -> StowTarget {
    let best = slots
        .into_iter()
        .filter(|s| {
            s.owner == Some(player)
                && !s.occupied
                && accepts_item(&s.kind, def.size, None, def.firearm.is_some(), None)
                && fits_body_slot(&s.kind, def)
        })
        // The first of the best rank, in the order they come.
        .fold(None::<SlotView>, |best, s| {
            let rank = |v: &SlotView| slot_rank(&v.kind, def.magazine.is_some());
            match best {
                Some(b) if rank(&b) <= rank(&s) => Some(b),
                _ => Some(s),
            }
        });
    if let Some(s) = best {
        return StowTarget::Socket(s.entity);
    }
    for p in packs {
        if p.owner != player || !p.carried {
            continue;
        }
        return if p.room {
            StowTarget::Pack(p.entity)
        } else {
            StowTarget::Refused {
                pack_full: Some(p.entity),
            }
        };
    }
    StowTarget::Refused { pack_full: None }
}

/// A thing in a hand, about to be let go.
#[derive(Clone, Copy, Debug)]
pub struct LetGo<'a> {
    pub item: Entity,
    pub def: Option<&'a ItemDef>,
    /// The thing's middle.
    pub at: Vec3,
    /// The hand holding it.
    pub hand: Vec3,
    pub player: Entity,
    /// World to the player's torso, if they have one.
    pub torso: Option<bevy::math::Affine3A>,
    /// The sling it's on (`weapons::carry::OnSling`), if any.
    pub sling: Option<Entity>,
}

/// Where a thing let go of gently goes (see the module's list).
pub fn let_go(
    l: &LetGo,
    slots: impl IntoIterator<Item = SlotView> + Clone,
    packs: impl IntoIterator<Item = PackView>,
) -> StowTarget {
    if let (Some(torso), Some(def)) = (l.torso, l.def)
        && over_shoulder(torso, l.hand)
    {
        return shoulder_stow(def, l.player, slots, packs);
    }
    let t = stow_target(l.item, l.def, l.at, l.player, slots.clone(), packs);
    if let (StowTarget::Drop { .. }, Some(sling)) = (t, l.sling)
        && slots.into_iter().any(|s| s.entity == sling && !s.occupied)
    {
        return StowTarget::Socket(sling);
    }
    t
}

/// The world as the stow sees it, read-only.
#[derive(SystemParam)]
pub struct StowQuery<'w, 's> {
    sockets: Query<
        'w,
        's,
        (
            Entity,
            &'static GlobalTransform,
            &'static Socket,
            Option<&'static SocketOwner>,
        ),
    >,
    places: Query<'w, 's, &'static Transform, With<BodySlot>>,
    packs: Query<'w, 's, (Entity, &'static GlobalTransform, &'static Backpack)>,
    items: Query<'w, 's, &'static Item>,
    catalog: Res<'w, Catalog>,
    torsos: Query<'w, 's, (&'static GlobalTransform, &'static ChildOf), With<body::Torso>>,
    held: Query<'w, 's, (&'static super::Held, &'static GlobalTransform)>,
    hands: Query<'w, 's, &'static super::Hand>,
    slung: Query<'w, 's, &'static crate::weapons::carry::OnSling>,
}

impl StowQuery<'_, '_> {
    pub fn def(&self, item: Entity) -> Option<&ItemDef> {
        self.items
            .get(item)
            .ok()
            .map(|i| self.catalog.get(i.kind()))
    }

    pub fn slots(&self) -> impl Iterator<Item = SlotView> + '_ {
        self.sockets
            .iter()
            .map(|(e, g, s, o)| SlotView::new(e, g, s, o, self.places.get(e).ok()))
    }

    pub fn packs(&self, def: Option<&ItemDef>) -> impl Iterator<Item = PackView> + '_ {
        let (size, bulk) = def.map_or((SizeClass::Medium, 0.0), |d| (d.size, d.bulk));
        self.packs.iter().map(move |(e, g, bag)| {
            let kinds: Vec<_> = bag
                .contents
                .iter()
                .filter_map(|c| self.items.get(*c).ok().map(|i| i.kind()))
                .collect();
            let used = bag.used(&self.catalog, &kinds);
            PackView {
                entity: e,
                owner: bag.owner,
                open: bag.open,
                carried: bag.carried,
                inverse: g.affine().inverse(),
                half: bag.half,
                room: bag.fits(used, size, bulk),
            }
        })
    }

    /// World to `player`'s torso.
    pub fn torso(&self, player: Entity) -> Option<bevy::math::Affine3A> {
        self.torsos
            .iter()
            .find(|(_, p)| p.parent() == player)
            .map(|(g, _)| g.affine().inverse())
    }

    /// Where `item`, in a hand now, would go if that hand let it go
    /// gently: None if it isn't held.
    pub fn target(&self, item: Entity) -> Option<StowTarget> {
        let (held, g) = self.held.get(item).ok()?;
        let hand = self.hands.get(held.hand).ok()?.world.translation;
        let def = self.def(item);
        let l = LetGo {
            item,
            def,
            at: g.translation(),
            hand,
            player: held.player,
            torso: self.torso(held.player),
            sling: self
                .slung
                .get(item)
                .ok()
                .filter(|s| s.player == held.player)
                .map(|s| s.socket),
        };
        let slots: Vec<SlotView> = self.slots().collect();
        Some(let_go(&l, slots, self.packs(def)))
    }
}
