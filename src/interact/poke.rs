//! Push buttons, pressed with a fingertip (VR). Every card that does
//! something when pressed (a comfort setting, the route, the driver's
//! request, a terminal's next page, the camp's choices) is the label
//! plate of a round button mounted at its right: put your finger on the
//! button and push. Gripping the card still works, and is what flat mode
//! does.
//!
//! The first headset session asked for physical controls over cards you
//! squeeze: a squeezed card gives no sense of having pressed anything.

use bevy::prelude::*;

use super::{Hand, InteractEvent};
use crate::raid::bench::ScreenText;
use crate::raid::truck::Pressable;
use crate::rig::{LocalPlayer, RigDriver, RigHand, RigInput};

/// The pointing index finger's tip in the hand's frame (the glove's index,
/// on the thumb side, in `render::props::POINTING`; a test there keeps the
/// modelled finger ending here). The hand's origin is the controller's grip,
/// in the middle of the closed fist, so the tip is a finger's length ahead.
pub const FINGERTIP: Vec3 = Vec3::new(0.0, 0.03, -0.0862);
const FINGER_R: f32 = 0.008;
/// The button's cap: radius, how proud of its base it stands, how far it
/// travels.
pub const BUTTON_R: f32 = 0.022;
pub const CAP: f32 = 0.014;
pub const TRAVEL: f32 = 0.01;
/// Gap between a card's edge and its button's centre.
const BESIDE: f32 = 0.04;

/// A button: presses `target` when pushed in. Its `+Z` faces out of the
/// wall.
#[derive(Component, Debug)]
pub struct PokeButton {
    pub target: Entity,
    /// How far the cap is pushed in (m), for its visual.
    pub depth: f32,
    pub down: bool,
}

/// A button with a look of its own (a modelled key that moves itself, a
/// crank): no generic collar and cap (`render::props::dress_buttons`),
/// which stood over the cassette deck's keys and hid them (playtest
/// 2026-10-28, F4). Every button but a card's has one.
#[derive(Component, Debug)]
pub struct OwnLook;

/// Cards that do nothing (the camp's title, the career sheet) get none.
pub(super) fn mount_button(
    add: On<Add, Pressable>,
    cards: Query<(&Pressable, &ScreenText, Option<&crate::camp::CampCard>)>,
    mut commands: Commands,
) {
    let Ok((kind, screen, camp)) = cards.get(add.entity) else {
        return;
    };
    let control = matches!(
        kind,
        Pressable::Setting
            | Pressable::Destination
            | Pressable::Request
            | Pressable::Terminal
            | Pressable::Camp
            // Gripped, it comes up to be read (`read`): its button turns
            // the page.
            | Pressable::Logbook
    ) && !matches!(
        camp,
        Some(crate::camp::CampCard::Title | crate::camp::CampCard::Career)
    );
    if !control {
        return;
    }
    commands.spawn((
        Name::new("Button"),
        PokeButton {
            target: add.entity,
            depth: 0.0,
            down: false,
        },
        Transform::from_xyz(screen.size.x * 0.5 + BESIDE, 0.0, 0.0),
        Visibility::default(),
        ChildOf(add.entity),
    ));
}

/// Fingertips push caps in; past most of the travel it clicks (once, until
/// the finger lets it back out).
pub(super) fn poke_buttons(
    players: Query<(Entity, &RigInput, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
    mut buttons: Query<(&GlobalTransform, &mut PokeButton)>,
    targets: Query<&Visibility>,
    driver: Res<RigDriver>,
    mut events: MessageWriter<InteractEvent>,
) {
    if *driver != RigDriver::Xr {
        return;
    }
    // Fingers that are pointing: an empty hand, trigger off.
    let mut tips = Vec::new();
    for (player, input, children) in &players {
        for child in children.iter() {
            let Ok((rh, hand)) = hands.get(child) else {
                continue;
            };
            let h = input.hand(rh.side);
            if h.tracked && h.trigger < 0.5 && hand.holding.is_none() && hand.handle.is_none() {
                tips.push((player, hand.world.transform_point(FINGERTIP)));
            }
        }
    }
    for (gt, mut button) in &mut buttons {
        // A card put out of sight (the training yard folds what it isn't
        // teaching yet) has nothing to push.
        if targets.get(button.target) == Ok(&Visibility::Hidden) {
            button.depth = 0.0;
            button.down = false;
            continue;
        }
        let to_local = gt.affine().inverse();
        let mut depth = 0.0f32;
        let mut by = None;
        for (player, tip) in &tips {
            let p = to_local.transform_point3(*tip);
            if p.truncate().length() > BUTTON_R + FINGER_R || p.z < -0.05 {
                continue;
            }
            let d = (CAP + FINGER_R - p.z).clamp(0.0, TRAVEL);
            if d > depth {
                depth = d;
                by = Some(*player);
            }
        }
        button.depth = depth;
        if !button.down && depth > TRAVEL * 0.75 {
            button.down = true;
            if let Some(player) = by {
                events.write(InteractEvent::Pressed {
                    item: button.target,
                    player,
                });
            }
        } else if button.down && depth < TRAVEL * 0.3 {
            button.down = false;
        }
    }
}
