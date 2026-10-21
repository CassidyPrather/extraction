//! The cassette player in the world: an item you hold, pocket or set
//! down. Open its door (stop, then stop again: the eject), put a tape in
//! the right way round (open edge down, the side you want facing out),
//! shut the door by pushing it, and press the keys on top with a
//! fingertip. It runs on a power cell in its back, and sags flat as the
//! cell dies. The rules are `extraction_core::music::deck`; this is the
//! item, its sockets and keys, and what the robots hear.

use bevy::prelude::*;
use extraction_core::items::Mount;
use extraction_core::music::deck::{Deck, Key, Mode, Pressed, Side};
use extraction_core::music::tapes;
use std::f32::consts::FRAC_PI_2;

use crate::interact::poke::PokeButton;
use crate::interact::{InSocket, InteractEvent, Socket, SocketKind, SocketOwner, physics_on};
use crate::items::usage::UseItem;
use crate::items::{Catalog, Item};

/// The player's body, centred on its origin (m): width, height, depth.
pub const BODY: Vec3 = Vec3::new(0.150, 0.090, 0.040);
/// The keys along the top, left to right.
pub const KEYS: [(Key, f32); 4] = [
    (Key::Rewind, -0.051),
    (Key::FastForward, -0.017),
    (Key::Play, 0.017),
    (Key::Stop, 0.051),
];
/// The cassette's well, behind the door.
pub const WELL: Vec3 = Vec3::new(0.0, 0.0, 0.004);
/// How far the door swings open (rad).
pub const DOOR_OPEN: f32 = 1.2;
/// How far a tape let go of near the open well is taken in (m).
pub const WELL_REACH: f32 = 0.08;
/// While it plays, how far off a robot hears it (m), and how often it's
/// told (s). Past [`crate::robots`]' wake radius at full volume: a tape
/// set down playing on a table wakes the house.
pub const HEARD_AT: f32 = 26.0;
pub const NOISE_EVERY: f32 = 3.0;

/// A cassette player: its mechanism, and how loud it's turned up (0..1).
#[derive(Component, Debug)]
pub struct TapeDeck {
    pub deck: Deck,
    pub volume: f32,
    /// The door's swing, 0 shut to 1 open (eased, so it springs).
    pub door: f32,
    /// The cassette in its well.
    pub tape: Option<Entity>,
    /// Seconds until robots are next told it's playing.
    pub noise_in: f32,
}

impl Default for TapeDeck {
    fn default() -> Self {
        Self {
            deck: Deck::default(),
            volume: 1.0,
            door: 0.0,
            tape: None,
            noise_in: 0.0,
        }
    }
}

/// A key on a cassette player (a child with its [`PokeButton`]).
#[derive(Component, Debug, Clone, Copy)]
pub struct DeckKey {
    pub deck: Entity,
    pub key: Key,
}

/// The door's push plate: pushed while open, it shuts.
#[derive(Component, Debug, Clone, Copy)]
pub struct DeckDoor {
    pub deck: Entity,
}

/// The tape well's socket.
#[derive(Component, Debug, Clone, Copy)]
pub struct TapeWell;

/// The battery compartment's socket.
#[derive(Component, Debug, Clone, Copy)]
pub struct DeckCell;

/// A cassette: how far its tape is wound (it keeps that between players).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Tape {
    pub wound: f32,
}

/// What a cassette player just did, for its sounds.
#[derive(Message, Clone, Copy, Debug)]
pub struct DeckEvent {
    pub deck: Entity,
    pub what: DeckWhat,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeckWhat {
    /// A key engaged the mechanism.
    Engaged(Mode),
    /// A key went down and did nothing.
    Dead,
    /// The door sprang open; pushed shut.
    DoorOpened,
    DoorShut,
    /// A tape went in, came out, or wouldn't go in that way round.
    TapeIn(Side),
    TapeOut,
    WrongWay,
    /// It stopped itself: the end of the side (or a flat cell).
    Stalled,
}

pub(super) fn deck_parts(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    let e = add.entity;
    let id = catalog.get(item.kind()).id.as_str();
    if tapes::get(id).is_some() {
        commands.entity(e).insert(Tape::default());
        return;
    }
    if id != "cassette_player" {
        return;
    }
    commands.entity(e).insert(TapeDeck::default());
    // The well: shut, it takes nothing (radius 0 until the door opens).
    commands.spawn((
        Name::new("Tape well"),
        TapeWell,
        Socket {
            kind: SocketKind::Mount(Mount::Cassette),
            occupant: None,
            radius: 0.0,
            seat: Transform::IDENTITY,
        },
        SocketOwner(e),
        Transform::from_translation(WELL),
        Visibility::default(),
        ChildOf(e),
    ));
    // A cell in the back, lying along the width.
    commands.spawn((
        Name::new("Cell mount"),
        DeckCell,
        Socket {
            kind: SocketKind::Mount(Mount::Cell),
            occupant: None,
            radius: 0.09,
            seat: Transform::from_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        },
        SocketOwner(e),
        Transform::from_xyz(0.0, -0.02, -BODY.z * 0.5 + 0.012),
        Visibility::default(),
        ChildOf(e),
    ));
    // The keys on top: each a push button facing up.
    for (key, x) in KEYS {
        commands.spawn((
            Name::new(format!("{key:?} key")),
            DeckKey { deck: e, key },
            Transform::from_xyz(x, BODY.y * 0.5, 0.0)
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            Visibility::default(),
            ChildOf(e),
        ));
    }
    commands.spawn((
        Name::new("Door plate"),
        DeckDoor { deck: e },
        door_plate(0.0),
        Visibility::default(),
        ChildOf(e),
    ));
}

/// The door's push plate, where the open door's inside face is; far out
/// of reach while it's shut (a shut door has nothing to push).
pub fn door_plate(open: f32) -> Transform {
    if open < 0.5 {
        return Transform::from_xyz(0.0, -50.0, 0.0);
    }
    let a = DOOR_OPEN * open;
    let hinge = Vec3::new(0.0, -BODY.y * 0.5, BODY.z * 0.5);
    let swing = Quat::from_rotation_x(a);
    // Its middle, and its inside face (+Z of the plate) turned up.
    Transform::from_translation(hinge + swing * Vec3::new(0.0, BODY.y * 0.45, 0.0))
        .with_rotation(swing * Quat::from_rotation_y(std::f32::consts::PI))
}

/// Keys and the door's plate get their push buttons. Kept off until
/// their parts exist so a button never targets nothing.
pub(super) fn mount_buttons(
    mut commands: Commands,
    keys: Query<Entity, (With<DeckKey>, Without<PokeButton>)>,
    doors: Query<Entity, (With<DeckDoor>, Without<PokeButton>)>,
) {
    for e in keys.iter().chain(doors.iter()) {
        commands.entity(e).insert(PokeButton {
            target: e,
            depth: 0.0,
            down: false,
        });
    }
}

/// A cell's share of its full charge.
fn charge_of(item: &Item, catalog: &Catalog) -> f32 {
    let full = catalog.get(item.kind()).battery_wh;
    if full <= 0.0 {
        0.0
    } else {
        (item.0.charge_wh / full).clamp(0.0, 1.0)
    }
}

fn cell_in(
    deck: Entity,
    sockets: &Query<(&Socket, &SocketOwner), With<DeckCell>>,
) -> Option<Entity> {
    sockets
        .iter()
        .find(|(_, o)| o.0 == deck)
        .and_then(|(s, _)| s.occupant)
}

/// Keys pressed (a fingertip in VR; the use button while held, flat or
/// bot, is play/stop) and the door pushed shut.
pub(super) fn press_keys(
    mut events: MessageReader<InteractEvent>,
    mut uses: MessageReader<UseItem>,
    keys: Query<&DeckKey>,
    doors: Query<&DeckDoor>,
    mut decks: Query<&mut TapeDeck>,
    cells: Query<(&Socket, &SocketOwner), With<DeckCell>>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<DeckEvent>,
) {
    let charge = |deck: Entity| {
        cell_in(deck, &cells)
            .and_then(|c| items.get(c).ok())
            .map_or(0.0, |i| charge_of(i, &catalog))
    };
    let mut press = |deck: Entity, key: Key, decks: &mut Query<&mut TapeDeck>| {
        let c = charge(deck);
        let Ok(mut d) = decks.get_mut(deck) else {
            return;
        };
        let what = match d.deck.press(key, c) {
            Pressed::Engaged(m) => DeckWhat::Engaged(m),
            Pressed::Ejected => DeckWhat::DoorOpened,
            Pressed::Nothing => DeckWhat::Dead,
        };
        out.write(DeckEvent { deck, what });
    };
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        if let Ok(k) = keys.get(*item) {
            press(k.deck, k.key, &mut decks);
        } else if let Ok(door) = doors.get(*item) {
            if let Ok(mut d) = decks.get_mut(door.deck) {
                if d.deck.door_open {
                    d.deck.close_door();
                    out.write(DeckEvent {
                        deck: door.deck,
                        what: DeckWhat::DoorShut,
                    });
                }
            }
        }
    }
    for u in uses.read() {
        let Ok(d) = decks.get(u.item) else {
            continue;
        };
        let key = if d.deck.mode == Mode::Stopped {
            Key::Play
        } else {
            Key::Stop
        };
        press(u.item, key, &mut decks);
    }
}

/// Which side faces out, if the tape is the right way round for the well
/// (its open edge, local -Y, down into the deck): the cassette's own
/// pose against the well's.
pub fn fits(tape: &GlobalTransform, well: &GlobalTransform) -> Option<Side> {
    let (_, t, _) = tape.to_scale_rotation_translation();
    let (_, w, _) = well.to_scale_rotation_translation();
    let down = (t * Vec3::NEG_Y).dot(w * Vec3::NEG_Y);
    if down < 0.5 {
        return None;
    }
    Some(if (t * Vec3::Z).dot(w * Vec3::Z) >= 0.0 {
        Side::A
    } else {
        Side::B
    })
}

/// Tapes into and out of wells. The pose it was let go at is still in its
/// global transform the frame it's seated (the seat applies after), and
/// that says which way round it went. The wrong way, it won't go: it
/// drops out in front.
pub(super) fn load_tapes(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    mut wells: Query<(&mut Socket, &SocketOwner, &GlobalTransform), With<TapeWell>>,
    mut decks: Query<&mut TapeDeck>,
    mut tapes_q: Query<(&mut Tape, &Item, &GlobalTransform)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<DeckEvent>,
) {
    for ev in events.read() {
        match *ev {
            InteractEvent::Socketed { item, socket } => {
                let Ok((mut well, owner, wgt)) = wells.get_mut(socket) else {
                    continue;
                };
                let Ok((tape, it, tgt)) = tapes_q.get_mut(item) else {
                    continue;
                };
                let Ok(mut d) = decks.get_mut(owner.0) else {
                    continue;
                };
                let id = catalog.get(it.kind()).id.as_str();
                let side = fits(tgt, wgt);
                let inserted = match (side, tapes::get(id)) {
                    (Some(side), Some(def)) => {
                        let len = tapes::side_length(def);
                        d.deck.insert(len, side, tape.wound).then_some(side)
                    }
                    _ => None,
                };
                if let Some(side) = inserted {
                    d.tape = Some(item);
                    out.write(DeckEvent {
                        deck: owner.0,
                        what: DeckWhat::TapeIn(side),
                    });
                } else {
                    // It won't go that way: out it drops, in front.
                    well.occupant = None;
                    let at = wgt.transform_point(Vec3::new(0.0, 0.02, 0.06));
                    let (_, rot, _) = tgt.to_scale_rotation_translation();
                    physics_on(
                        &mut commands,
                        item,
                        Transform::from_translation(at).with_rotation(rot),
                        Vec3::ZERO,
                        Vec3::ZERO,
                        true,
                    );
                    out.write(DeckEvent {
                        deck: owner.0,
                        what: DeckWhat::WrongWay,
                    });
                }
            }
            InteractEvent::Unsocketed { item, socket } => {
                let Ok((_, owner, _)) = wells.get(socket) else {
                    continue;
                };
                let Ok(mut d) = decks.get_mut(owner.0) else {
                    continue;
                };
                if d.tape != Some(item) {
                    continue;
                }
                // The door is open (the well takes nothing otherwise).
                d.deck.door_open = true;
                let wound = d.deck.remove().unwrap_or(0.0);
                d.tape = None;
                if let Ok((mut tape, ..)) = tapes_q.get_mut(item) {
                    tape.wound = wound;
                }
                out.write(DeckEvent {
                    deck: owner.0,
                    what: DeckWhat::TapeOut,
                });
            }
            _ => {}
        }
    }
}

/// The mechanism runs, the cell drains, the door swings, and the well
/// opens to a tape only while the door does.
pub(super) fn run_decks(
    mut decks: Query<(Entity, &mut TapeDeck)>,
    mut wells: Query<(&mut Socket, &SocketOwner), (With<TapeWell>, Without<DeckCell>)>,
    cells: Query<(&Socket, &SocketOwner), With<DeckCell>>,
    mut items: Query<&mut Item>,
    mut plates: Query<(&DeckDoor, &mut Transform)>,
    in_socket: Query<&InSocket>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut out: MessageWriter<DeckEvent>,
) {
    let dt = time.delta_secs();
    for (e, mut d) in &mut decks {
        // A tape taken out some other way (it was looted from the well by
        // hand, which unsockets it) is handled in `load_tapes`; one that
        // vanished is just gone.
        if let Some(t) = d.tape {
            if in_socket.get(t).is_err() {
                d.tape = None;
                d.deck.door_open = true;
                d.deck.remove();
                d.deck.door_open = false;
            }
        }
        let cell = cell_in(e, &cells);
        let charge = cell
            .and_then(|c| items.get(c).ok())
            .map_or(0.0, |i| charge_of(i, &catalog));
        let watts = d.deck.watts();
        if d.deck.step(dt, charge) {
            out.write(DeckEvent {
                deck: e,
                what: DeckWhat::Stalled,
            });
        }
        if let Some(Ok(mut it)) = cell.map(|c| items.get_mut(c)) {
            it.0.charge_wh = (it.0.charge_wh - watts * dt / 3600.0).max(0.0);
        }
        let want = if d.deck.door_open { 1.0 } else { 0.0 };
        // Springs open, pushed shut.
        let rate = if d.deck.door_open { 8.0 } else { 14.0 };
        d.door += (want - d.door) * (dt * rate).min(1.0);
        if (d.door - want).abs() < 0.01 {
            d.door = want;
        }
        for (mut well, owner) in &mut wells {
            if owner.0 == e {
                well.radius = if d.deck.door_open { WELL_REACH } else { 0.0 };
            }
        }
        for (plate, mut t) in &mut plates {
            if plate.deck == e {
                *t = door_plate(d.door);
            }
        }
    }
}

/// While a tape plays, the robots near it hear it: a lure, set down on a
/// table and left running.
pub(super) fn deck_noise(
    mut decks: Query<(Entity, &mut TapeDeck, &GlobalTransform)>,
    time: Res<Time>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    let dt = time.delta_secs();
    for (_, mut d, gt) in &mut decks {
        if d.deck.mode != Mode::Playing || d.volume <= 0.0 {
            d.noise_in = 0.0;
            continue;
        }
        d.noise_in -= dt;
        if d.noise_in > 0.0 {
            continue;
        }
        d.noise_in = NOISE_EVERY;
        noise.write(crate::robots::Noise {
            at: gt.translation(),
            radius: HEARD_AT * d.volume.sqrt(),
            source: None,
        });
    }
}
