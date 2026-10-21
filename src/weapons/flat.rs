//! Flat-screen gun handling: the VR motions as timed macros.
//!
//! | Key | Does |
//! | --- | --- |
//! | V | Work the action: tap-rack the slide / charging handle (drop the magazine first on a double feed), cycle the bolt, break the shotgun, thumb a revolver's hammer |
//! | R | Reload from what you carry: swap magazines, or thumb rounds in |
//! | X | Selector / safety |
//! | C | Combine the items in your two hands (load a magazine from a box, clean a gun...) |
//!
//! The macros call the same mechanism operations a VR hand does, step by
//! step with human-ish timing, so flat play exercises the real mechanics.

use bevy::prelude::*;
use extraction_core::firearm::{ActionKind, Malfunction};
use extraction_core::items::Category;
use std::collections::VecDeque;

use super::{DropMagazine, Ejection, Gun, GunEvent, Mag, emit};
use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{Hand, InSocket, InteractEvent, Socket, SocketOwner, seat_in_socket};
use crate::items::{Catalog, Item};
use crate::rig::{LocalPlayer, RigHand, Side};

pub struct FlatGunPlugin;

impl Plugin for FlatGunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, start_macros);
    }
}

#[derive(Clone, Debug)]
pub enum Step {
    Bolt(f32),
    ReleaseBolt,
    Lift(f32),
    Break(bool),
    DropMag,
    InsertBestMag,
    SlideStop,
    LoadRound,
    Mode,
    /// Slap the magazine home.
    TapMag,
    /// Thumb the hammer back.
    Cock,
    /// Open or shut a revolver's loading gate.
    Gate(bool),
    /// Turn a revolver's cylinder a chamber by hand.
    Rotate,
    /// Push the case at the gate out with the rod.
    EjectGate,
    /// Slap a stuck turn-bolt's handle up (a palm under it).
    Slap,
}

/// A player's queued gun-handling motions (flat input and bots both fill it).
#[derive(Component, Default)]
pub struct GunMacro {
    gun: Option<Entity>,
    steps: VecDeque<(f32, Step)>,
    wait: f32,
}

impl GunMacro {
    pub fn busy(&self) -> bool {
        !self.steps.is_empty()
    }

    pub fn start(&mut self, gun: Entity, steps: Vec<(f32, Step)>) {
        self.gun = Some(gun);
        self.steps = steps.into();
        self.wait = 0.0;
    }
}

/// What a macro is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroKind {
    Rack,
    Reload,
    Mode,
}

/// The motions for a macro on this gun.
pub fn macro_steps(kind: MacroKind, gun: &Gun) -> Vec<(f32, Step)> {
    let action = gun.firearm.def.action;
    let mut steps: Vec<(f32, Step)> = Vec::new();
    match kind {
        MacroKind::Rack => {
            if action == ActionKind::BreakAction {
                let open = gun.firearm.bolt_travel < 0.5;
                steps.push((0.1, Step::Break(open)));
            } else if gun.firearm.malfunction == Some(Malfunction::DoubleFeed) {
                // Strip the magazine, rack the pair out, reload, rack.
                steps.push((0.05, Step::DropMag));
                steps.extend(rack_steps(action).into_iter().map(|(t, s)| (t + 0.25, s)));
                steps.push((0.5, Step::InsertBestMag));
                steps.extend(rack_steps(action).into_iter().map(|(t, s)| (t + 0.05, s)));
            } else if action == ActionKind::SemiAuto {
                // Tap-rack: harmless when nothing is wrong.
                steps.push((0.05, Step::TapMag));
                steps.extend(rack_steps(action).into_iter().map(|(t, s)| (t + 0.1, s)));
            } else {
                steps = rack_steps(action);
            }
        }
        MacroKind::Mode => steps.push((0.0, Step::Mode)),
        MacroKind::Reload => match action {
            ActionKind::SemiAuto if gun.firearm.def.magazine_family.is_none() => {
                // A fixed magazine (an SKS): hold the bolt open, thumb
                // rounds in, let it run home.
                steps.push((0.1, Step::Bolt(1.0)));
                for _ in gun.firearm.internal_rounds..gun.firearm.def.internal_capacity {
                    steps.push((0.3, Step::LoadRound));
                }
                let home = if gun.firearm.locked_back {
                    Step::SlideStop
                } else {
                    Step::ReleaseBolt
                };
                steps.push((0.15, home));
            }
            ActionKind::SemiAuto => {
                steps.push((0.05, Step::DropMag));
                steps.push((0.6, Step::InsertBestMag));
                if gun.firearm.locked_back {
                    steps.push((0.3, Step::SlideStop));
                } else {
                    steps.extend(rack_steps(action).into_iter().map(|(t, s)| (t + 0.05, s)));
                }
            }
            ActionKind::BoltAction => {
                steps.push((0.1, Step::Lift(1.0)));
                steps.push((0.1, Step::Bolt(1.0)));
                for _ in 0..gun.firearm.def.internal_capacity {
                    steps.push((0.35, Step::LoadRound));
                }
                steps.push((0.15, Step::Bolt(0.0)));
                steps.push((0.1, Step::Lift(0.0)));
            }
            ActionKind::BreakAction => {
                steps.push((0.1, Step::Break(true)));
                for _ in 0..gun.firearm.def.barrels {
                    steps.push((0.5, Step::LoadRound));
                }
                steps.push((0.3, Step::Break(false)));
            }
            ActionKind::Revolver => {
                // Round the cylinder at the gate: rod out the case, thumb a
                // round in, turn.
                steps.push((0.1, Step::Gate(true)));
                for _ in 0..gun.firearm.chambers.len() {
                    steps.push((0.25, Step::EjectGate));
                    steps.push((0.3, Step::LoadRound));
                    steps.push((0.15, Step::Rotate));
                }
                steps.push((0.2, Step::Gate(false)));
            }
        },
    }
    steps
}

fn rack_steps(action: ActionKind) -> Vec<(f32, Step)> {
    match action {
        ActionKind::SemiAuto => vec![
            (0.05, Step::Bolt(0.4)),
            (0.05, Step::Bolt(1.0)),
            (0.08, Step::ReleaseBolt),
        ],
        ActionKind::BoltAction => vec![
            // A stuck bolt takes a slap first (nothing if it isn't).
            (0.0, Step::Slap),
            (0.08, Step::Lift(1.0)),
            (0.08, Step::Bolt(0.5)),
            (0.06, Step::Bolt(1.0)),
            (0.12, Step::Bolt(0.4)),
            (0.06, Step::Bolt(0.0)),
            (0.08, Step::Lift(0.0)),
        ],
        ActionKind::BreakAction => vec![(0.1, Step::Break(true))],
        ActionKind::Revolver => vec![(0.1, Step::Cock)],
    }
}

fn start_macros(
    keys: Res<ButtonInput<KeyCode>>,
    mut players: Query<(&Children, &mut GunMacro), (With<LocalPlayer>, Without<crate::bot::Bot>)>,
    hands: Query<(&RigHand, &Hand)>,
    guns: Query<&Gun>,
    mut combine: MessageWriter<Combine>,
) {
    let Some((children, mut mac)) = players.iter_mut().next() else {
        return;
    };
    let mut right = None;
    let mut left = None;
    for c in children.iter() {
        if let Ok((rh, h)) = hands.get(c) {
            match rh.side {
                Side::Right => right = h.holding,
                Side::Left => left = h.holding,
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyT) {
        if let (Some(a), Some(b)) = (right, left) {
            combine.write(Combine { a, b });
        }
    }
    let Some(gun_e) = right else {
        return;
    };
    let Ok(gun) = guns.get(gun_e) else {
        return;
    };
    if mac.busy() {
        return;
    }
    let kind = if keys.just_pressed(KeyCode::KeyV) {
        MacroKind::Rack
    } else if keys.just_pressed(KeyCode::KeyX) {
        MacroKind::Mode
    } else if keys.just_pressed(KeyCode::KeyR) {
        MacroKind::Reload
    } else {
        return;
    };
    mac.start(gun_e, macro_steps(kind, gun));
}

fn run_macros(
    mut commands: Commands,
    mut players: Query<(Entity, &mut GunMacro)>,
    time: Res<Time>,
    mut guns: Query<(Entity, &mut Gun, &GlobalTransform, &Item)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<Ejection>,
    mut drop: MessageWriter<DropMagazine>,
    mut events: MessageWriter<InteractEvent>,
    owners: Query<(Entity, &SocketOwner)>,
    mut sockets: Query<&mut Socket>,
    mut mags: Query<
        (
            Entity,
            &mut Item,
            &Mag,
            Option<&InSocket>,
            Option<&InBackpack>,
        ),
        Without<Gun>,
    >,
    mut bags: Query<&mut Backpack>,
) {
    for (player_e, mut mac) in &mut players {
        let player = Some(player_e);
        let Some(gun_e) = mac.gun else {
            continue;
        };
        mac.wait -= time.delta_secs();
        if mac.wait > 0.0 {
            continue;
        }
        let Some((delay, step)) = mac.steps.pop_front() else {
            mac.gun = None;
            continue;
        };
        if let Some((next, _)) = mac.steps.front() {
            mac.wait = *next;
        }
        let _ = delay;
        let Ok((e, mut gun, gt, item)) = guns.get_mut(gun_e) else {
            mac.steps.clear();
            continue;
        };
        let name = catalog.get(item.kind()).id.clone();
        debug!("macro step {step:?} on {gun_e}");
        let ev = match step {
            Step::Bolt(t) => gun.firearm.set_bolt_travel(t),
            Step::ReleaseBolt => gun.firearm.release_bolt(),
            Step::Lift(t) => gun.firearm.set_handle_lift(t),
            Step::Slap => gun.firearm.free_bolt(),
            Step::Break(open) => gun.firearm.set_break_open(open),
            Step::SlideStop => gun.firearm.release_slide_stop(),
            Step::Mode => gun.firearm.cycle_mode(),
            Step::TapMag => gun.firearm.tap_magazine(),
            Step::Cock => gun.firearm.cock_hammer(),
            Step::Gate(open) => gun.firearm.set_gate(open),
            Step::Rotate => gun.firearm.rotate_cylinder(),
            Step::EjectGate => gun.firearm.eject_at_gate(),
            Step::DropMag => {
                drop.write(DropMagazine { gun: e });
                Vec::new()
            }
            Step::InsertBestMag => {
                // The fullest fitting magazine on the body or in the bag.
                let family = gun.firearm.def.magazine_family.clone().unwrap_or_default();
                let mine = |m: &(
                    Entity,
                    Mut<Item>,
                    &Mag,
                    Option<&InSocket>,
                    Option<&InBackpack>,
                )| {
                    let on_body = m.3.is_some_and(|s| {
                        owners.get(s.socket).is_ok_and(|(_, o)| Some(o.0) == player)
                    });
                    let in_bag = m.4.is_some();
                    on_body || in_bag
                };
                let mut best: Option<(u32, Entity)> = None;
                for m in &mut mags {
                    if m.2.family != family || !mine(&m) {
                        continue;
                    }
                    if best.is_none_or(|b| m.1.rounds > b.0) {
                        best = Some((m.1.rounds, m.0));
                    }
                }
                if let (Some((_, mag_e)), Some(well)) = (best, gun.magwell) {
                    if let Ok((_, _, _, in_socket, in_bag)) = mags.get(mag_e) {
                        if let Some(s) = in_socket {
                            if let Ok(mut socket) = sockets.get_mut(s.socket) {
                                socket.occupant = None;
                            }
                        }
                        if in_bag.is_some() {
                            for mut bag in &mut bags {
                                bag.contents.retain(|c| *c != mag_e);
                            }
                        }
                    }
                    commands.entity(mag_e).remove::<(InSocket, InBackpack)>();
                    if let Ok(mut socket) = sockets.get_mut(well) {
                        if socket.occupant.is_none() {
                            seat_in_socket(&mut commands, mag_e, well, &mut socket);
                            events.write(InteractEvent::Socketed {
                                item: mag_e,
                                socket: well,
                            });
                        }
                    }
                }
                Vec::new()
            }
            Step::LoadRound => {
                // Done after the step, with world access: see `load_one_round`.
                Vec::new()
            }
        };
        emit(e, &name, &mut gun, gt, ev, &mut out, &mut ejected);
        if matches!(step, Step::LoadRound) {
            commands.queue(move |world: &mut World| load_one_round(world, player_e, gun_e));
        }
    }
}

/// Moves one round from carried ammunition into the gun.
fn load_one_round(world: &mut World, player: Entity, gun_e: Entity) {
    let Some(cartridge) = world
        .get::<Gun>(gun_e)
        .map(|g| g.firearm.def.cartridge.clone())
    else {
        return;
    };
    let catalog = world.resource::<Catalog>().clone();
    // Candidate ammo: in our sockets or our backpack.
    let mut q = world.query::<(Entity, &Item, Option<&InSocket>, Option<&InBackpack>)>();
    let mut candidates = Vec::new();
    for (e, it, s, b) in q.iter(world) {
        let def = catalog.get(it.kind());
        if def.category != Category::Ammo
            || def.cartridge.as_deref() != Some(cartridge.as_str())
            || it.rounds == 0
        {
            continue;
        }
        if s.is_some() || b.is_some() {
            candidates.push((e, it.rounds, s.map(|s| s.socket), b.map(|b| b.bag)));
        }
    }
    let mine: Vec<_> = candidates
        .into_iter()
        .filter(|(_, _, s, b)| {
            s.is_some_and(|s| world.get::<SocketOwner>(s).is_some_and(|o| o.0 == player))
                || b.is_some_and(|b| {
                    world
                        .get::<Backpack>(b)
                        .is_some_and(|bag| bag.owner == player)
                })
        })
        .collect();
    // Loose rounds first, then the emptiest box.
    let Some((ammo_e, _, socket, bag)) = mine.into_iter().min_by_key(|(_, r, _, _)| *r) else {
        return;
    };
    let load = {
        let base = catalog.get(world.get::<Item>(ammo_e).unwrap().kind()).load;
        let it = world.get::<Item>(ammo_e).unwrap();
        let mut left = it.loads.clone();
        let load = left.pop_or(it.rounds, base);
        (load, left)
    };
    let loaded = world
        .get_mut::<Gun>(gun_e)
        .is_some_and(|mut g| g.firearm.load_round_of(load.0));
    if !loaded {
        return;
    }
    let empty = {
        let mut it = world.get_mut::<Item>(ammo_e).expect("ammo exists");
        it.loads = load.1;
        it.rounds -= 1;
        it.rounds == 0
    };
    let loose = catalog
        .get(world.get::<Item>(ammo_e).unwrap().kind())
        .rounds
        == 1;
    if empty && loose {
        if let Some(s) = socket {
            if let Some(mut sock) = world.get_mut::<Socket>(s) {
                sock.occupant = None;
            }
        }
        if let Some(b) = bag {
            if let Some(mut bag) = world.get_mut::<Backpack>(b) {
                bag.contents.retain(|c| *c != ammo_e);
            }
        }
        world.despawn(ammo_e);
    }
}

/// Two held items brought together: load a magazine from ammo, clean or
/// repair a gun.
#[derive(Message, Clone, Copy, Debug)]
pub struct Combine {
    pub a: Entity,
    pub b: Entity,
}

pub fn apply_combines(
    mut commands: Commands,
    mut combines: MessageReader<Combine>,
    mut items: Query<(&mut Item, Option<&Mag>)>,
    mut guns: Query<&mut Gun>,
    catalog: Res<Catalog>,
    hands: Query<(Entity, &Hand)>,
) {
    for c in combines.read() {
        for (x, y) in [(c.a, c.b), (c.b, c.a)] {
            let (Ok((ix, _)), Ok((iy, my))) = (items.get(x), items.get(y)) else {
                continue;
            };
            let dx = catalog.get(ix.kind()).clone();
            let dy = catalog.get(iy.kind()).clone();
            let my = my.cloned();
            // Ammo into a magazine.
            if let (Some(cart), Some(mag)) = (&dx.cartridge, &dy.magazine) {
                if dx.category == Category::Ammo && *cart == mag.cartridge {
                    let room = mag.capacity.saturating_sub(iy.rounds);
                    let moved = room.min(ix.rounds);
                    // A round at a time off the ammo, on top of what's in.
                    let mut taken = Vec::new();
                    if let Ok((mut a, _)) = items.get_mut(x) {
                        for i in 0..moved {
                            let n = a.rounds - i;
                            taken.push(a.loads.pop_or(n, dx.load));
                        }
                        a.rounds -= moved;
                    }
                    if let Ok((mut b, _)) = items.get_mut(y) {
                        for l in taken {
                            let have = b.rounds;
                            b.loads.push(have, l);
                            b.rounds += 1;
                        }
                    }
                    let _ = my;
                    if moved > 0 && dx.rounds == 1 && items.get(x).is_ok_and(|(a, _)| a.rounds == 0)
                    {
                        release_and_despawn(&mut commands, x, &hands);
                    }
                    break;
                }
            }
            // Cleaning kit / gun oil / spare parts on a gun.
            if let Ok(mut gun) = guns.get_mut(y) {
                match dx.id.as_str() {
                    "cleaning_kit" => {
                        gun.firearm.clean(1.0);
                        if let Ok((mut kit, _)) = items.get_mut(x) {
                            kit.condition = (kit.condition - 0.1).max(0.0);
                            if kit.condition <= 0.0 {
                                release_and_despawn(&mut commands, x, &hands);
                            }
                        }
                    }
                    "gun_oil" => {
                        gun.firearm.clean(0.3);
                        if let Ok((mut oil, _)) = items.get_mut(x) {
                            oil.condition = (oil.condition - 0.25).max(0.0);
                            if oil.condition <= 0.0 {
                                release_and_despawn(&mut commands, x, &hands);
                            }
                        }
                    }
                    "spare_parts" => {
                        // Parts are consumed: a resource sink.
                        gun.firearm.repair(0.35, 0.95);
                        release_and_despawn(&mut commands, x, &hands);
                    }
                    _ => continue,
                }
                break;
            }
        }
    }
}

/// How close (m) a box of rounds and a magazine held in your two hands
/// have to come for the rounds to go in.
pub const TOUCH: f32 = 0.12;
/// How close a cleaning kit, oil or parts must come to a gun's receiver.
pub const TOUCH_GUN: f32 = 0.2;

/// How near two things must come to combine, if they go together at all:
/// a box of rounds and its magazine; a cleaning kit, oil or spare parts
/// and a gun's receiver.
pub fn combine_reach(
    a: &extraction_core::items::ItemDef,
    b: &extraction_core::items::ItemDef,
) -> Option<f32> {
    let loads = |ammo: &extraction_core::items::ItemDef, mag: &extraction_core::items::ItemDef| {
        ammo.category == Category::Ammo
            && matches!((&ammo.cartridge, &mag.magazine), (Some(c), Some(m)) if *c == m.cartridge)
    };
    let tends = |tool: &extraction_core::items::ItemDef, gun: &extraction_core::items::ItemDef| {
        gun.firearm.is_some()
            && matches!(tool.id.as_str(), "cleaning_kit" | "gun_oil" | "spare_parts")
    };
    if loads(a, b) || loads(b, a) {
        Some(TOUCH)
    } else if tends(a, b) || tends(b, a) {
        Some(TOUCH_GUN)
    } else {
        None
    }
}

/// VR: bring a box of rounds to a magazine, one in each hand, and the rounds
/// go in (what flat mode's C does); bring a cleaning kit, oil or spare
/// parts to a gun's receiver and it's tended. Without it a headset could never load a
/// magazine: the yard's sign said to, and nothing listened.
///
/// One-handed (`Comfort::one_handed`), the other thing needn't be in a
/// hand: it can be set down (on a table, the ground) or in one of your own
/// pouches or pockets. A one-handed bot plays this way too.
#[allow(clippy::too_many_arguments)]
fn touch_to_combine(
    driver: Res<crate::rig::RigDriver>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    hands: Query<(&RigHand, &Hand, &ChildOf)>,
    items: Query<(Entity, &Item, &GlobalTransform)>,
    held: Query<(), With<crate::interact::Held>>,
    in_socket: Query<&InSocket>,
    owners: Query<&SocketOwner>,
    bots: Query<(), With<crate::bot::Bot>>,
    catalog: Res<Catalog>,
    mut combine: MessageWriter<Combine>,
    mut touching: Local<std::collections::HashSet<(Entity, Entity)>>,
) {
    let one_handed = comfort.is_some_and(|c| c.one_handed);
    let xr = *driver == crate::rig::RigDriver::Xr;
    if !xr && !(one_handed && !bots.is_empty()) {
        return;
    }
    let mut now = std::collections::HashSet::new();
    let mut touch = |a: Entity, b: Entity, ga: &GlobalTransform, gb: &GlobalTransform, reach| {
        if ga.translation().distance(gb.translation()) < reach {
            now.insert((a, b));
            if !touching.contains(&(a, b)) {
                combine.write(Combine { a, b });
            }
        }
    };
    for (rh, h, owner) in &hands {
        // Bots play on the flat driver, and play by hand.
        if !xr && !bots.contains(owner.parent()) {
            continue;
        }
        let Some(a) = h.holding else { continue };
        let Ok((_, ia, ga)) = items.get(a) else {
            continue;
        };
        let da = catalog.get(ia.kind());
        if one_handed {
            // Whatever's to hand: set down, or on your own body.
            let player = owner.parent();
            for (b, ib, gb) in &items {
                if b == a
                    || held.contains(b)
                    || gb.translation().distance(ga.translation()) > TOUCH_GUN
                    || in_socket
                        .get(b)
                        .is_ok_and(|s| owners.get(s.socket).map_or(true, |o| o.0 != player))
                {
                    continue;
                }
                if let Some(reach) = combine_reach(da, catalog.get(ib.kind())) {
                    touch(a, b, ga, gb, reach);
                }
            }
            continue;
        }
        if rh.side != Side::Right {
            continue;
        }
        let other = hands
            .iter()
            .find(|(o, _, p)| o.side == Side::Left && p.parent() == owner.parent())
            .and_then(|(_, oh, _)| oh.holding);
        let Some(b) = other else { continue };
        let Ok((_, ib, gb)) = items.get(b) else {
            continue;
        };
        if let Some(reach) = combine_reach(da, catalog.get(ib.kind())) {
            touch(a, b, ga, gb, reach);
        }
    }
    *touching = now;
}

fn release_and_despawn(commands: &mut Commands, item: Entity, hands: &Query<(Entity, &Hand)>) {
    for (h, hand) in hands.iter() {
        if hand.holding == Some(item) {
            commands.entity(h).queue(|mut e: EntityWorldMut| {
                if let Some(mut hand) = e.get_mut::<Hand>() {
                    hand.holding = None;
                }
            });
        }
    }
    commands.entity(item).try_despawn();
}

/// Logic half of gun handling: running macros and combining items. Runs
/// headless (bots use it); the key bindings are in [`FlatGunPlugin`].
pub struct CombinePlugin;

impl Plugin for CombinePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Combine>().add_systems(
            FixedUpdate,
            (
                run_macros.after(super::triggers),
                (touch_to_combine, apply_combines).chain(),
            ),
        );
    }
}
