//! The cassette player through the headset's path: hands posed and
//! squeezed, fingertips pushed onto keys (`vr_harness`), never messages
//! sent straight to the mechanism.

use bevy::prelude::*;
use extraction_core::music::deck::{Key, Mode, Side as TapeSide};

use super::deck::{DeckDoor, DeckKey, DeckWhat, TapeDeck, TapeWell};
use crate::interact::poke::{CAP, FINGERTIP};
use crate::interact::{InSocket, Socket};
use crate::rig::Side;
use crate::vr_harness::Vr;

/// Messages of a kind seen so far.
#[derive(Resource, Default)]
struct Log(Vec<DeckWhat>, Vec<crate::robots::Noise>);

fn log(
    mut deck: MessageReader<super::deck::DeckEvent>,
    mut noise: MessageReader<crate::robots::Noise>,
    mut l: ResMut<Log>,
) {
    l.0.extend(deck.read().map(|e| e.what));
    l.1.extend(noise.read().cloned());
}

fn setup() -> (Vr, Entity, Vec3) {
    let mut vr = Vr::new("training_yard");
    vr.app.init_resource::<Log>().add_systems(Update, log);
    let feet = vr
        .app
        .world()
        .resource::<crate::world::Zone>()
        .on_ground(0.0, 30.0);
    vr.stand(feet, 0.0);
    // The player in the left hand, held level in front of the chest.
    let at = feet + Vec3::new(-0.15, 1.2, -0.35);
    let deck = vr.spawn("cassette_player", Transform::from_translation(at));
    vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
    vr.step(2);
    vr.grip(Side::Left, 1.0);
    vr.step(3);
    assert_eq!(vr.hand_state(Side::Left).holding, Some(deck), "held");
    (vr, deck, feet)
}

fn deck_of(vr: &Vr, deck: Entity) -> extraction_core::music::deck::Deck {
    vr.app.world().get::<TapeDeck>(deck).unwrap().deck.clone()
}

/// Where a child of the deck with `C` is (a key, the door's plate, the
/// well).
fn part<C: Component>(vr: &mut Vr, pick: impl Fn(&C) -> bool) -> GlobalTransform {
    *vr.app
        .world_mut()
        .query::<(&C, &GlobalTransform)>()
        .iter(vr.app.world())
        .find(|(c, _)| pick(c))
        .map(|(_, gt)| gt)
        .expect("the part")
}

/// The right forefinger pushes a button in along its face and takes it
/// back out.
fn poke(vr: &mut Vr, button: GlobalTransform) {
    let normal = button.rotation() * Vec3::Z;
    let rot = Quat::from_rotation_arc(Vec3::NEG_Z, -normal);
    let tip_at = |d: f32| button.translation() + normal * d - rot * FINGERTIP;
    vr.hand(Side::Right, tip_at(0.06), rot, 0.0, 0.0);
    vr.step(2);
    for k in 0..8 {
        vr.move_hand(Side::Right, tip_at(0.06 - k as f32 * 0.006));
        vr.step(1);
    }
    for k in 0..6 {
        vr.move_hand(Side::Right, tip_at(CAP - k as f32 * 0.003));
        vr.step(1);
    }
    vr.move_hand(Side::Right, tip_at(0.08));
    vr.step(3);
}

fn press(vr: &mut Vr, key: Key) {
    let gt = part::<DeckKey>(vr, |k| k.key == key);
    poke(vr, gt);
}

/// Takes a cassette in the right hand turned `rot`, brings it to the well
/// and lets go there.
fn put_tape(vr: &mut Vr, id: &str, rot: Quat) -> Entity {
    let well = part::<TapeWell>(vr, |_| true);
    let start = well.translation() + Vec3::new(0.25, 0.1, 0.1);
    let tape = vr.spawn(id, Transform::from_translation(start).with_rotation(rot));
    vr.hand(Side::Right, start, Quat::IDENTITY, 0.0, 0.0);
    vr.step(2);
    vr.grip(Side::Right, 1.0);
    vr.step(3);
    assert_eq!(vr.hand_state(Side::Right).holding, Some(tape), "tape held");
    for k in 1..=10 {
        let p = start.lerp(well.translation() + Vec3::Z * 0.01, k as f32 / 10.0);
        vr.move_hand(Side::Right, p);
        vr.step(1);
    }
    vr.grip(Side::Right, 0.0);
    vr.step(3);
    vr.move_hand(Side::Right, well.translation() + Vec3::new(0.2, 0.15, 0.2));
    vr.step(2);
    tape
}

/// A cell into the back, full.
fn put_cell(vr: &mut Vr, deck: Entity) {
    vr.app
        .world_mut()
        .entity_mut(deck)
        .insert(super::deck::WithCell(1.0));
    vr.step(3);
}

/// Stop twice opens the door; a tape the right way round goes in, side A
/// out; pushing the door shuts it; play plays, and the robots hear it.
#[test]
fn load_a_tape_and_play_it() {
    let (mut vr, deck, _) = setup();
    put_cell(&mut vr, deck);
    press(&mut vr, Key::Stop);
    assert!(deck_of(&vr, deck).door_open, "stop while stopped ejects");
    // Let the door swing open.
    vr.step(20);
    let tape = put_tape(&mut vr, "tape_practice", Quat::IDENTITY);
    assert!(vr.app.world().get::<InSocket>(tape).is_some(), "seated");
    assert_eq!(deck_of(&vr, deck).tape.map(|t| t.1), Some(TapeSide::A));
    let plate = part::<DeckDoor>(&mut vr, |d| d.deck == deck);
    poke(&mut vr, plate);
    assert!(!deck_of(&vr, deck).door_open, "pushed shut");
    vr.step(10);
    press(&mut vr, Key::Play);
    assert_eq!(deck_of(&vr, deck).mode, Mode::Playing);
    let before = deck_of(&vr, deck).position();
    vr.step(120);
    assert!(deck_of(&vr, deck).position() > before, "the tape moves");
    let log = vr.app.world().resource::<Log>();
    assert!(
        log.1.iter().any(|n| n.radius > 20.0 && n.source.is_none()),
        "robots hear it"
    );
    press(&mut vr, Key::Stop);
    assert_eq!(deck_of(&vr, deck).mode, Mode::Stopped);
}

/// Turned over, it plays side B; upside down (the open edge up) it won't
/// go in and drops out.
#[test]
fn the_right_way_round() {
    let (mut vr, deck, _) = setup();
    press(&mut vr, Key::Stop);
    vr.step(20);
    let tape = put_tape(
        &mut vr,
        "tape_hymns",
        Quat::from_rotation_z(std::f32::consts::PI),
    );
    assert!(vr.app.world().get::<InSocket>(tape).is_none(), "refused");
    assert!(
        vr.app
            .world()
            .resource::<Log>()
            .0
            .contains(&DeckWhat::WrongWay),
        "it said why"
    );
    assert!(deck_of(&vr, deck).tape.is_none());
    let flipped = put_tape(
        &mut vr,
        "tape_lodge",
        Quat::from_rotation_y(std::f32::consts::PI),
    );
    assert!(vr.app.world().get::<InSocket>(flipped).is_some(), "seated");
    assert_eq!(deck_of(&vr, deck).tape.map(|t| t.1), Some(TapeSide::B));
}

/// No cell, no music: play does nothing; the key still goes down.
#[test]
fn no_cell_no_music() {
    let (mut vr, deck, _) = setup();
    press(&mut vr, Key::Stop);
    vr.step(20);
    put_tape(&mut vr, "tape_practice", Quat::IDENTITY);
    let plate = part::<DeckDoor>(&mut vr, |d| d.deck == deck);
    poke(&mut vr, plate);
    press(&mut vr, Key::Play);
    assert_eq!(deck_of(&vr, deck).mode, Mode::Stopped);
    assert!(vr.app.world().resource::<Log>().0.contains(&DeckWhat::Dead));
}
