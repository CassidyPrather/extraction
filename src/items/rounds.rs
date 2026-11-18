//! Boxes of rounds (#103, `docs/shots/ammo_boxes/PROPOSAL.md`): what a box
//! shows and how its rounds come out.
//!
//! - **Shows** (client, [`RoundsVisualsPlugin`]): each round in a box's
//!   open end is its own node (`round_<i>`, `art/items/ammo.py`), the
//!   first the last to go: a box shows its share of what it holds, the top
//!   row (or the front) going first. Each round's point (and the band
//!   behind it, and a shell's hull) is painted for its load, read from the
//!   box's per-round [`Loads`]: a mixed box shows its mix. A box holding a
//!   load its factory box doesn't is *mixed*: it wears a hatched band and
//!   MIX in place of its type's band. A shut box shows its seal.
//! - **Shut and open**: a box comes shut and full, and doesn't spill. Torn
//!   open (used in the hand, or the other hand's trigger pinched at its
//!   end), it stays open; one that isn't full is open.
//! - **Out** (logic, [`RoundsPlugin`]), with a box held open in one hand:
//!   - the other hand's trigger pinched at its mouth takes a round; held,
//!     it gathers one more every [`GATHER_EVERY`] up to [`HANDFUL`] (a
//!     handful is a loose round with more than one in it, its loads kept);
//!   - tipped mouth-down, it pours a round every [`POUR_EVERY`]: into the
//!     other hand when that's under the mouth (up to [`POUR_HAND`]), else
//!     onto whatever's below;
//!   - a handful brought to its mouth with the trigger pinched goes back
//!     in, a round every [`BACK_EVERY`]: a box filled from another is mixed.
//! - Loading a magazine or a gun from a box or a handful is as it was
//!   (`weapons::flat`, `weapons::feed_by_hand`).

use std::collections::HashMap;

use bevy::prelude::*;
use extraction_core::ammo::{Load, Loads};
use extraction_core::items::{Category, ItemDef};

use super::{Catalog, Item, ItemModelRoot, SpawnItem, spawn_item};
use crate::interact::HandAction;
use crate::rig::{RigHand, Side};

/// A round taken, then one more each this many seconds while the pinch
/// holds, up to [`HANDFUL`].
pub const GATHER_EVERY: f32 = 0.2;
pub const HANDFUL: u32 = 5;
/// A box mouth-down pours a round each this many seconds; into a hand, up
/// to [`POUR_HAND`].
pub const POUR_EVERY: f32 = 0.1;
pub const POUR_HAND: u32 = 10;
/// A handful at a box's mouth goes in a round each this many seconds.
pub const BACK_EVERY: f32 = 0.15;
/// How near (m) a hand must be to a box's mouth to take from it or put
/// back, and how far under it to catch a pour.
pub const MOUTH_REACH: f32 = 0.07;
pub const CATCH_REACH: f32 = 0.14;
/// Tipped this far (the mouth's way against straight down, as a cosine),
/// a box pours.
pub const POUR_TIP: f32 = 0.55;

/// A box torn open (it stays open). A box that isn't full is open anyway.
#[derive(Component, Clone, Copy, Debug)]
pub struct Opened;

/// What happens at a box, for sound.
#[derive(Message, Clone, Copy, Debug)]
pub enum BoxEvent {
    Torn {
        item: Entity,
        at: Vec3,
    },
    /// A round out (taken, gathered or poured).
    Out {
        item: Entity,
        at: Vec3,
    },
    /// A round back in.
    In {
        item: Entity,
        at: Vec3,
    },
}

/// Which way a box's mouth faces, in its own space: where its rounds show
/// and come out (`art/items/ammo.py`). None for things that aren't boxes.
pub fn mouth_dir(def: &ItemDef) -> Option<Vec3> {
    if def.category != Category::Ammo || def.rounds <= 1 || def.cartridge.is_none() {
        return None;
    }
    let id = def.id.as_str();
    Some(
        if id.starts_with("ammo_545") || id.starts_with("ammo_762x38r") {
            Vec3::NEG_Z
        } else if id.starts_with("ammo_762r") {
            Vec3::Z
        } else {
            Vec3::Y
        },
    )
}

/// The middle of a box's mouth, in its own space: its box's face that way.
pub fn mouth_at(def: &ItemDef) -> Option<Vec3> {
    let d = mouth_dir(def)?;
    let half = Vec3::from(def.size_m()) * 0.5;
    Some(d * half)
}

/// Open: torn, or not full.
pub fn is_open(item: &Item, def: &ItemDef, opened: bool) -> bool {
    opened || item.rounds < def.rounds
}

/// The load of the `i`-th round from the bottom of a stack of `count`
/// whose unlisted rounds are `base`.
pub fn load_at(loads: &Loads, count: u32, base: Load, i: u32) -> Load {
    let listed = loads.0.len().min(count as usize) as u32;
    let unlisted = count - listed;
    if i < unlisted {
        base
    } else {
        let k = (loads.0.len() as u32 - listed) + (i - unlisted);
        loads.0.get(k as usize).copied().unwrap_or(base)
    }
}

/// The loads its factory box holds.
fn factory(def: &ItemDef) -> [Option<Load>; 2] {
    [
        Some(def.load),
        (def.tracer_every > 0).then_some(Load::Tracer),
    ]
}

/// A box holding a load its factory box doesn't (a damp round counts as
/// what it was).
pub fn is_mixed(item: &Item, def: &ItemDef) -> bool {
    let f = factory(def);
    item.loads.0.iter().any(|l| {
        let l = if *l == Load::Damp { def.load } else { *l };
        !f.contains(&Some(l))
    })
}

/// How many of `slots` round nodes show for `rounds` of `capacity`: its
/// share, at least one while any are left.
pub fn shown(slots: u32, rounds: u32, capacity: u32) -> u32 {
    if rounds == 0 || slots == 0 {
        return 0;
    }
    let c = capacity.max(rounds).max(1);
    (slots * rounds).div_ceil(c).clamp(1, slots)
}

/// A round's paint: (the band behind its point, its point, a shell's
/// hull), as model material names.
pub fn paint(load: Load, cartridge: &str) -> (&'static str, &'static str, &'static str) {
    match load {
        Load::Ap if matches!(cartridge, "7.62x39" | "7.62x54R") => ("Red", "Black", "Red"),
        Load::Ap => ("Copper", "Black", "Red"),
        Load::Tracer => ("Copper", "Leaf", "Red"),
        Load::Subsonic => ("Leaf", "Black", "Red"),
        Load::Slug => ("Copper", "Copper", "Blue"),
        _ => ("Copper", "Copper", "Red"),
    }
}

pub struct RoundsPlugin;

impl Plugin for RoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BoxEvent>()
            .add_systems(Update, (tear_on_use, hands_at_boxes).chain());
    }
}

/// Used in the hand, a shut box tears open.
fn tear_on_use(
    mut commands: Commands,
    mut uses: MessageReader<crate::items::usage::UseItem>,
    items: Query<(&Item, &GlobalTransform, Has<Opened>)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<BoxEvent>,
) {
    for u in uses.read() {
        let Ok((it, gt, opened)) = items.get(u.item) else {
            continue;
        };
        let def = catalog.get(it.kind());
        if mouth_dir(def).is_none() || is_open(it, def, opened) {
            continue;
        }
        commands.entity(u.item).insert(Opened);
        out.write(BoxEvent::Torn {
            item: u.item,
            at: gt.translation(),
        });
    }
}

/// A hand's work at a box held in the other: its pinch held since, and
/// when it last moved a round.
#[derive(Default)]
struct Work {
    trigger_was: HashMap<Entity, bool>,
    /// A pinch that took a round (it gathers while held) or put one back.
    pinch: HashMap<Entity, (Entity, bool)>,
    next: HashMap<Entity, f32>,
    /// A handful made and waiting to be put in this hand.
    handing: Vec<(Entity, Side, Entity, Entity)>,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn hands_at_boxes(
    mut commands: Commands,
    hands: Query<(Entity, &RigHand, &crate::interact::Hand, &ChildOf)>,
    mut items: Query<(&mut Item, &GlobalTransform, Has<Opened>)>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut actions: MessageWriter<HandAction>,
    mut out: MessageWriter<BoxEvent>,
    mut work: Local<Work>,
) {
    let now = time.elapsed_secs();
    // Handfuls made last tick go into their hands now, once spawned.
    for (player, side, hand, item) in std::mem::take(&mut work.handing) {
        if items.contains(item)
            && hands
                .get(hand)
                .is_ok_and(|(_, _, h, _)| h.holding.is_none())
        {
            actions.write(HandAction::Grab {
                player,
                side,
                item,
                point: None,
            });
        }
    }
    let all: Vec<_> = hands
        .iter()
        .map(|(e, rh, h, p)| (e, rh.side, h, p.parent()))
        .collect();
    for &(hand_e, side, hand, player) in &all {
        let trigger = hand.trigger_down;
        let was = work.trigger_was.insert(hand_e, trigger).unwrap_or(false);
        if !trigger {
            work.pinch.remove(&hand_e);
        }
        // The other hand, holding an open box.
        let Some(&(_, _, other, _)) = all
            .iter()
            .find(|(e, s, _, p)| *e != hand_e && *s != side && *p == player)
        else {
            continue;
        };
        let Some(box_e) = other.holding else {
            continue;
        };
        let Ok((bx, bgt, opened)) = items.get(box_e) else {
            continue;
        };
        let def = catalog.get(bx.kind()).clone();
        let (Some(dir), Some(at)) = (mouth_dir(&def), mouth_at(&def)) else {
            continue;
        };
        let cartridge = def.cartridge.clone().unwrap_or_default();
        let open = is_open(bx, &def, opened);
        let mouth = bgt.transform_point(at);
        let out_dir = bgt.affine().transform_vector3(dir).normalize_or_zero();
        let near = hand.world.translation.distance(mouth) < MOUTH_REACH;
        let pressed = trigger && !was;
        // In this hand: nothing, or a loose round or handful of the box's
        // calibre.
        let in_hand = hand.holding.and_then(|h| {
            let (it, _, _) = items.get(h).ok()?;
            let d = catalog.get(it.kind());
            (d.category == Category::Ammo
                && d.rounds <= 1
                && d.magazine.is_none()
                && d.cartridge.as_deref() == Some(cartridge.as_str()))
            .then_some(h)
        });
        if hand.holding.is_some() && in_hand.is_none() {
            continue;
        }
        // A shut box: the pinch at its end tears it.
        if !open {
            if pressed && near {
                commands.entity(box_e).insert(Opened);
                out.write(BoxEvent::Torn {
                    item: box_e,
                    at: mouth,
                });
            }
            continue;
        }
        let due = work.next.get(&hand_e).is_none_or(|t| now >= *t);
        // Back in: a handful at the mouth, pinched.
        if let Some(h) = in_hand
            && near
            && trigger
            && (pressed || work.pinch.get(&hand_e) == Some(&(box_e, false)))
        {
            work.pinch.insert(hand_e, (box_e, false));
            if !due {
                continue;
            }
            let room = items
                .get(box_e)
                .is_ok_and(|(b, _, _)| b.rounds < def.rounds);
            if !room {
                continue;
            }
            let Ok([(mut b, _, _), (mut r, _, _)]) = items.get_many_mut([box_e, h]) else {
                continue;
            };
            let rbase = catalog.get(r.kind()).load;
            let n = r.rounds;
            let load = r.loads.pop_or(n, rbase);
            r.rounds -= 1;
            let bn = b.rounds;
            b.loads.push(bn, load);
            b.rounds += 1;
            if r.rounds == 0 {
                commands.entity(h).despawn();
            }
            work.next.insert(hand_e, now + BACK_EVERY);
            out.write(BoxEvent::In {
                item: box_e,
                at: mouth,
            });
            continue;
        }
        let rounds_left = items.get(box_e).map(|(b, _, _)| b.rounds).unwrap_or(0);
        if rounds_left == 0 {
            continue;
        }
        // Out: pinched at the mouth (and gathering while held), or poured.
        let pinching = near
            && trigger
            && (pressed && in_hand.is_none() || work.pinch.get(&hand_e) == Some(&(box_e, true)));
        let pouring = out_dir.dot(Vec3::NEG_Y) > POUR_TIP;
        if !(pinching || pouring) || !due {
            continue;
        }
        let under = {
            let to = hand.world.translation - mouth;
            to.length() < CATCH_REACH && to.normalize_or_zero().dot(Vec3::NEG_Y) > 0.5
        };
        let cap = if pinching { HANDFUL } else { POUR_HAND };
        let into_hand = pinching || under;
        let have = in_hand
            .and_then(|h| items.get(h).ok().map(|(r, _, _)| r.rounds))
            .unwrap_or(0);
        if into_hand && have >= cap {
            continue;
        }
        if pinching {
            work.pinch.insert(hand_e, (box_e, true));
        }
        let Ok((mut b, _, _)) = items.get_mut(box_e) else {
            continue;
        };
        let n = b.rounds;
        let load = b.loads.pop_or(n, def.load);
        b.rounds -= 1;
        work.next.insert(
            hand_e,
            now + if pinching { GATHER_EVERY } else { POUR_EVERY },
        );
        out.write(BoxEvent::Out {
            item: box_e,
            at: mouth,
        });
        match (into_hand, in_hand) {
            (true, Some(h)) => {
                if let Ok((mut r, _, _)) = items.get_mut(h) {
                    let rn = r.rounds;
                    r.loads.push(rn, load);
                    r.rounds += 1;
                }
            }
            _ => {
                let Some(kind) = crate::weapons::loose_round(&catalog, &cartridge, load) else {
                    continue;
                };
                let at = if into_hand {
                    hand.world.translation
                } else {
                    mouth + out_dir * 0.03
                };
                let e = spawn_item(
                    &mut commands,
                    &catalog,
                    SpawnItem {
                        kind,
                        transform: Transform::from_translation(at),
                        condition: 1.0,
                    },
                );
                // A round of another load than its kind's (none such
                // exists for that calibre) keeps its load.
                if catalog.get(kind).load != load {
                    commands.entity(e).queue(move |mut w: EntityWorldMut| {
                        if let Some(mut it) = w.get_mut::<Item>() {
                            it.loads = Loads(vec![load]);
                        }
                    });
                }
                if into_hand {
                    work.handing.push((player, side, hand_e, e));
                }
            }
        }
    }
}

pub struct RoundsVisualsPlugin;

impl Plugin for RoundsVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_rounds.before(super::damp::DampVisualsSet));
    }
}

/// A box's model's working parts, found once its model is in.
#[derive(Component)]
struct BoxParts {
    rounds: Vec<RoundNode>,
    band: Option<Entity>,
    band_mix: Option<Entity>,
    seal: Option<Entity>,
    print_mix: Option<Entity>,
    shown: Option<(u32, Loads, bool, bool)>,
}

struct RoundNode {
    node: Entity,
    /// Its painted meshes: the band behind the point, the point, a hull.
    band: Vec<Entity>,
    tip: Vec<Entity>,
    hull: Vec<Entity>,
}

#[allow(clippy::type_complexity)]
fn show_rounds(
    mut commands: Commands,
    mut boxes: Query<(
        Entity,
        &Item,
        &Children,
        Has<Opened>,
        Option<&mut BoxParts>,
        Option<&super::damp::DampLook>,
    )>,
    roots: Query<&Children, With<ItemModelRoot>>,
    names: Query<(&Name, Option<&Children>)>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut vis: Query<&mut Visibility>,
    catalog: Res<Catalog>,
    model_mats: Option<ResMut<crate::models::ModelMaterials>>,
    mut lib: Option<ResMut<crate::render::materials::MaterialLibrary>>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut model_mats) = model_mats else {
        return;
    };
    let Some(lib) = lib.as_mut() else {
        return;
    };
    let mut mat = |name: &str| model_mats.get(name, lib, &server, &mut materials);
    let (tip_m, band_m, hull_m) = (mat("Tip"), mat("TipBand"), mat("Hull"));
    for (e, item, kids, opened, parts, damp) in &mut boxes {
        let def = catalog.get(item.kind());
        if mouth_dir(def).is_none() {
            continue;
        }
        let mut parts = match parts {
            Some(p) => p,
            None => {
                let Some(root) = kids.iter().find_map(|c| roots.get(c).ok()) else {
                    continue;
                };
                let mut found = BoxParts {
                    rounds: Vec::new(),
                    band: None,
                    band_mix: None,
                    seal: None,
                    print_mix: None,
                    shown: None,
                };
                let mut slots: Vec<(u32, Entity)> = Vec::new();
                for node in root.iter() {
                    let Ok((name, _)) = names.get(node) else {
                        continue;
                    };
                    match name.as_str() {
                        "band" => found.band = Some(node),
                        "band_mix" => found.band_mix = Some(node),
                        "seal" => found.seal = Some(node),
                        "print_mix" => found.print_mix = Some(node),
                        n => {
                            if let Some(i) = n.strip_prefix("round_").and_then(|i| i.parse().ok()) {
                                slots.push((i, node));
                            }
                        }
                    }
                }
                if slots.is_empty() && found.seal.is_none() {
                    continue;
                }
                slots.sort();
                for (_, node) in slots {
                    let mut r = RoundNode {
                        node,
                        band: Vec::new(),
                        tip: Vec::new(),
                        hull: Vec::new(),
                    };
                    let mut stack = vec![node];
                    while let Some(n) = stack.pop() {
                        if let Ok(m) = meshes.get(n) {
                            if m.0 == tip_m {
                                r.tip.push(n);
                            } else if m.0 == band_m {
                                r.band.push(n);
                            } else if m.0 == hull_m {
                                r.hull.push(n);
                            }
                        }
                        if let Ok((_, Some(c))) = names.get(n) {
                            stack.extend(c.iter());
                        } else if let Ok(c) = roots.get(n) {
                            stack.extend(c.iter());
                        }
                    }
                    found.rounds.push(r);
                }
                commands.entity(e).insert(found);
                continue;
            }
        };
        let open = is_open(item, def, opened);
        let mixed = is_mixed(item, def);
        let now = (item.rounds, item.loads.clone(), open, mixed);
        if parts.shown.as_ref() == Some(&now) {
            continue;
        }
        // Its damp look is put back first, and made again over the new
        // paint (`items::damp`).
        if let Some(d) = damp {
            d.undo(&mut meshes);
            commands.entity(e).remove::<super::damp::DampLook>();
        }
        let set = |vis: &mut Query<&mut Visibility>, e: Option<Entity>, on: bool| {
            if let Some(e) = e
                && let Ok(mut v) = vis.get_mut(e)
            {
                *v = if on {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        };
        set(&mut vis, parts.seal, !open);
        set(&mut vis, parts.band, !mixed);
        set(&mut vis, parts.band_mix, mixed);
        set(&mut vis, parts.print_mix, mixed);
        let k = parts.rounds.len() as u32;
        let show = shown(k, item.rounds, def.rounds);
        let cartridge = def.cartridge.clone().unwrap_or_default();
        for (i, r) in parts.rounds.iter().enumerate() {
            let i = i as u32;
            set(&mut vis, Some(r.node), i < show);
            if i >= show {
                continue;
            }
            // The round this node stands for, counted from the bottom.
            let at = (i * item.rounds) / show.max(1);
            let load = load_at(&item.loads, item.rounds, def.load, at);
            let (b, t, h) = paint(load, &cartridge);
            let (b, t, h) = (mat(b), mat(t), mat(h));
            for (list, m) in [(&r.band, &b), (&r.tip, &t), (&r.hull, &h)] {
                for n in list {
                    if let Ok(mut mm) = meshes.get_mut(*n) {
                        mm.0 = m.clone();
                    }
                }
            }
        }
        parts.shown = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_shows_its_share_top_row_first() {
        assert_eq!(shown(12, 30, 30), 12);
        assert_eq!(shown(12, 12, 30), 5);
        assert_eq!(shown(12, 1, 30), 1, "one left still shows");
        assert_eq!(shown(12, 0, 30), 0);
        assert_eq!(shown(20, 20, 20), 20);
    }

    #[test]
    fn a_rounds_load_is_read_from_the_bottom() {
        // Three below unlisted (ball), then AP, tracer on top.
        let loads = Loads(vec![Load::Ap, Load::Tracer]);
        assert_eq!(load_at(&loads, 5, Load::Standard, 0), Load::Standard);
        assert_eq!(load_at(&loads, 5, Load::Standard, 3), Load::Ap);
        assert_eq!(load_at(&loads, 5, Load::Standard, 4), Load::Tracer);
    }

    #[test]
    fn a_box_with_a_load_its_factory_lacks_is_mixed() {
        let catalog = Catalog::load();
        let fresh = |id: &str| {
            let kind = catalog.kind(id).unwrap();
            Item(extraction_core::items::ItemState::fresh(
                &catalog, kind, 1.0,
            ))
        };
        let ball = catalog.get(catalog.kind("ammo_545").unwrap()).clone();
        let mut b = fresh("ammo_545");
        assert!(!is_mixed(&b, &ball));
        let n = b.rounds;
        b.loads.push(n - 1, Load::Ap);
        assert!(is_mixed(&b, &ball), "an AP round in a ball box");
        // The one-in-four box is its own factory mix.
        let t4 = catalog.get(catalog.kind("ammo_545_t4").unwrap()).clone();
        let b = fresh("ammo_545_t4");
        assert!(!is_mixed(&b, &t4));
        // A damp round is what it was.
        let mut b = fresh("ammo_545");
        let n = b.rounds;
        assert!(b.loads.spoil(n, Load::Standard, 0));
        assert!(!is_mixed(&b, &ball));
        assert!(mouth_dir(&ball).is_some());
        assert!(mouth_dir(catalog.get(catalog.kind("round_545").unwrap())).is_none());
    }

    #[test]
    fn rounds_are_painted_by_load() {
        assert_eq!(paint(Load::Ap, "5.45x39").1, "Black");
        assert_eq!(paint(Load::Ap, "7.62x39").0, "Red");
        assert_eq!(paint(Load::Tracer, "5.45x39").1, "Leaf");
        assert_eq!(paint(Load::Slug, "12ga").2, "Blue");
        assert_eq!(paint(Load::Standard, "12ga").2, "Red");
    }
}
